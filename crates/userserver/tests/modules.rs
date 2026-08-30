//! Out-of-process module lifecycle: registration, a signed event delivery to a
//! mock module, and the account-linking round trip.

use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use bscp_common::config::UserServerConfig;
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tower::ServiceExt;

type HmacSha256 = Hmac<Sha256>;

fn hmac_hex(secret: &str, body: &[u8]) -> String {
    let mut m = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    m.update(body);
    m.finalize().into_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Clone, Default)]
struct MockState {
    events: Arc<Mutex<Vec<Value>>>,
}

async fn mock_manifest() -> axum::Json<Value> {
    axum::Json(json!({
        "name": "mock",
        "version": "1.0",
        "description": "test module",
        "events": ["user.registered", "message.sent"],
        "link_providers": [{ "id": "github", "name": "GitHub" }]
    }))
}

async fn mock_events(State(st): State<MockState>, body: axum::body::Bytes) -> StatusCode {
    if let Ok(v) = serde_json::from_slice::<Value>(&body) {
        st.events.lock().unwrap().push(v);
    }
    StatusCode::OK
}

/// Spawn the mock module on an ephemeral port; returns `(base_url, events)`.
async fn spawn_mock() -> (String, Arc<Mutex<Vec<Value>>>) {
    let st = MockState::default();
    let events = st.events.clone();
    let app = axum::Router::new()
        .route("/.well-known/bscp-module", get(mock_manifest))
        .route("/events", post(mock_events))
        .with_state(st);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), events)
}

async fn user_app() -> axum::Router {
    let dir = std::env::temp_dir().join(format!("bscp-mod-{}", bscp_common::uuid()));
    std::fs::create_dir_all(&dir).unwrap();
    let cfg = UserServerConfig {
        port: 0,
        domain: "localhost:5000".into(),
        db_path: dir.join("db.sqlite"),
        secret_key: "test-secret-key-value".into(),
        cache_dir: dir.join("c"),
        cache_time: 3600,
        upload_dir: dir.join("u"),
        static_dir: dir.join("s"),
        vapid_public_key: String::new(),
        vapid_private_key: String::new(),
        vapid_contact: "mailto:t@localhost".into(),
        vapid_keys_file: dir.join("v.json"),
        oidc_keys_file: dir.join("oidc.json"),
        ice_public_ip: None,
        rtc_port_min: 0,
        rtc_port_max: 0,
        public_url: "http://localhost:5000".into(),
        oidc_access_ttl: 3600,
        oidc_refresh_ttl: 2_592_000,
    };
    std::fs::create_dir_all(&cfg.cache_dir).unwrap();
    let pool = SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    bscp_userserver::MIGRATOR.run(&pool).await.unwrap();
    let state = bscp_userserver::make_state(cfg, pool).unwrap();
    bscp_userserver::routes::build(state)
}

fn post_json(uri: &str, body: Value, cookie: Option<&str>) -> Request<Body> {
    let mut b = Request::builder().method("POST").uri(uri).header("content-type", "application/json");
    if let Some(c) = cookie {
        b = b.header("cookie", c);
    }
    b.body(Body::from(body.to_string())).unwrap()
}

async fn json_of(resp: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

async fn admin_cookie(app: &axum::Router) -> String {
    app.clone()
        .oneshot(post_json(
            "/api/auth/setup",
            json!({ "username": "admin", "password": "secret1", "password_confirm": "secret1" }),
            None,
        ))
        .await
        .unwrap();
    let r = app
        .clone()
        .oneshot(post_json("/api/auth/login", json!({ "user": "admin", "password": "secret1" }), None))
        .await
        .unwrap();
    r.headers().get("set-cookie").unwrap().to_str().unwrap().split(';').next().unwrap().to_string()
}

#[tokio::test]
async fn module_registration_events_and_linking() {
    let (mock_url, events) = spawn_mock().await;
    let app = user_app().await;
    let cookie = admin_cookie(&app).await;

    // register the module
    let reg = json_of(
        app.clone()
            .oneshot(post_json("/api/admin/modules", json!({ "base_url": mock_url }), Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    let secret = reg["secret"].as_str().unwrap().to_string();
    assert_eq!(reg["name"], "mock");

    // an invite + registration should fire `user.registered` at the module
    let invite = json_of(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/invites/generate")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let code = invite["code"].as_str().unwrap();
    app.clone()
        .oneshot(post_json(
            "/api/auth/register",
            json!({ "username": "bob", "password": "secret1", "password_confirm": "secret1", "invite_code": code }),
            None,
        ))
        .await
        .unwrap();

    // wait for the fire-and-forget delivery
    for _ in 0..50 {
        if !events.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let ev = events.lock().unwrap().clone();
    assert!(!ev.is_empty(), "module received no events");
    assert_eq!(ev[0]["type"], "user.registered");
    assert_eq!(ev[0]["data"]["user"], "bob@localhost:5000");

    // ── account linking ──
    let start = app
        .clone()
        .oneshot(
            Request::get("/api/modules/mock/link/github/start")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(start.status(), StatusCode::SEE_OTHER);
    let loc = start.headers().get("location").unwrap().to_str().unwrap().to_string();
    let ticket = between(&loc, "ticket=", "&").expect("ticket");

    // the module reports the completed link (HMAC-signed)
    let body = json!({ "ticket": ticket, "external_id": "42", "display_name": "octocat",
                       "profile_url": "https://github.com/octocat" })
    .to_string();
    let sig = format!("sha256={}", hmac_hex(&secret, body.as_bytes()));
    let cb = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/modules/mock/links")
                .header("content-type", "application/json")
                .header("x-bscp-signature", sig)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cb.status(), StatusCode::OK, "{:?}", json_of(cb).await);

    // it shows up
    let links = json_of(
        app.clone()
            .oneshot(Request::get("/api/users/me/links").header("cookie", &cookie).body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(links[0]["provider"], "github");
    assert_eq!(links[0]["display_name"], "octocat");

    let providers = json_of(
        app.clone()
            .oneshot(Request::get("/api/modules/providers").header("cookie", &cookie).body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(providers[0]["linked"], true);

    // unlink
    let del = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/modules/mock/links/github")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(del.status(), StatusCode::NO_CONTENT);

    let links = json_of(
        app.clone()
            .oneshot(Request::get("/api/users/me/links").header("cookie", &cookie).body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(links.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn link_callback_rejects_bad_signature() {
    let (mock_url, _) = spawn_mock().await;
    let app = user_app().await;
    let cookie = admin_cookie(&app).await;
    app.clone()
        .oneshot(post_json("/api/admin/modules", json!({ "base_url": mock_url }), Some(&cookie)))
        .await
        .unwrap();

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/modules/mock/links")
                .header("content-type", "application/json")
                .header("x-bscp-signature", "sha256=deadbeef")
                .body(Body::from(json!({ "ticket": "x" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

fn between(haystack: &str, start: &str, end: &str) -> Option<String> {
    let i = haystack.find(start)? + start.len();
    let rest = &haystack[i..];
    let j = rest.find(end).unwrap_or(rest.len());
    Some(rest[..j].to_string())
}
