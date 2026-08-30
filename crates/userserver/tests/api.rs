//! End-to-end route tests driven through the axum `Router` over an in-memory DB.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use bscp_common::config::UserServerConfig;
use serde_json::{json, Value};
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

struct TestServer {
    app: axum::Router,
}

async fn server() -> TestServer {
    let dir = std::env::temp_dir().join(format!("bscp-test-{}", bscp_common::uuid()));
    std::fs::create_dir_all(&dir).unwrap();

    let cfg = UserServerConfig {
        port: 0,
        domain: "localhost:5000".into(),
        db_path: dir.join("db.sqlite"),
        secret_key: "test-secret-key-value".into(),
        cache_dir: dir.join("cache"),
        cache_time: 3600,
        upload_dir: dir.join("uploads"),
        static_dir: dir.join("static"),
        vapid_public_key: String::new(),
        vapid_private_key: String::new(),
        vapid_contact: "mailto:test@localhost".into(),
        vapid_keys_file: dir.join("vapid.json"),
        oidc_keys_file: dir.join("oidc.json"),
        ice_public_ip: None,
        rtc_port_min: 0,
        rtc_port_max: 0,
        public_url: "http://localhost:5000".into(),
        oidc_access_ttl: 3600,
        oidc_refresh_ttl: 2_592_000,
    };
    std::fs::create_dir_all(&cfg.cache_dir).unwrap();
    std::fs::create_dir_all(&cfg.upload_dir).unwrap();

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    bscp_userserver::MIGRATOR.run(&pool).await.unwrap();

    let state = bscp_userserver::make_state(cfg, pool).unwrap();
    TestServer { app: bscp_userserver::routes::build(state) }
}

impl TestServer {
    async fn call(&self, req: Request<Body>) -> (StatusCode, Value) {
        let resp = self.app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, value)
    }

    async fn call_raw(&self, req: Request<Body>) -> (StatusCode, Vec<String>, Value) {
        let resp = self.app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let cookies: Vec<String> = resp
            .headers()
            .get_all("set-cookie")
            .iter()
            .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_string())
            .collect();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, cookies, value)
    }
}

fn get(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

fn post_json(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn with_token(mut req: Request<Body>, token: &str) -> Request<Body> {
    req.headers_mut().insert("x-session-token", token.parse().unwrap());
    req
}

async fn setup_admin(s: &TestServer) -> String {
    let (status, _) = s
        .call(post_json(
            "/api/auth/setup",
            json!({ "username": "admin", "password": "secret1", "password_confirm": "secret1" }),
        ))
        .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, v) = s
        .call(post_json("/api/auth/login", json!({ "user": "admin", "password": "secret1" })))
        .await;
    assert_eq!(status, StatusCode::OK);
    v["session_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn setup_gate() {
    let s = server().await;
    let (_, v) = s.call(get("/api/auth/setup")).await;
    assert_eq!(v["needs_setup"], json!(true));

    setup_admin(&s).await;

    let (_, v) = s.call(get("/api/auth/setup")).await;
    assert_eq!(v["needs_setup"], json!(false));

    let (status, v) = s
        .call(post_json(
            "/api/auth/setup",
            json!({ "username": "x", "password": "secret1", "password_confirm": "secret1" }),
        ))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(v["message"], json!("Setup already complete"));
}

#[tokio::test]
async fn login_and_me() {
    let s = server().await;
    let token = setup_admin(&s).await;

    let (status, v) = s.call(with_token(get("/api/users/me"), &token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["username"], json!("admin@localhost:5000"));
    assert_eq!(v["is_admin"], json!(true));

    let (status, _) = s.call(get("/api/users/me")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (_, v) = s
        .call(post_json("/api/auth/login", json!({ "user": "admin", "password": "wrong" })))
        .await;
    assert_eq!(v["success"], json!(false));
    assert_eq!(v["error"], json!("Invalid username or password"));
}

#[tokio::test]
async fn invites_and_registration() {
    let s = server().await;
    let admin = setup_admin(&s).await;

    let (status, invite) = s
        .call(with_token(
            Request::builder().method("POST").uri("/api/invites/generate").body(Body::empty()).unwrap(),
            &admin,
        ))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let code = invite["code"].as_str().unwrap().to_string();

    let (status, _) = s
        .call(post_json(
            "/api/auth/register",
            json!({ "username": "bob", "password": "secret1", "password_confirm": "secret1", "invite_code": code }),
        ))
        .await;
    assert_eq!(status, StatusCode::CREATED);

    // Reused invite is rejected.
    let (status, v) = s
        .call(post_json(
            "/api/auth/register",
            json!({ "username": "eve", "password": "secret1", "password_confirm": "secret1", "invite_code": code }),
        ))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(v["errors"][0], json!("Invite code already used"));

    // Non-admin cannot list invites.
    let (_, bob) = s
        .call(post_json("/api/auth/login", json!({ "user": "bob", "password": "secret1" })))
        .await;
    let bob = bob["session_token"].as_str().unwrap().to_string();
    let (status, _) = s.call(with_token(get("/api/invites/"), &bob)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn send_list_delete_message() {
    let s = server().await;
    let token = setup_admin(&s).await;

    let (status, msg) = s
        .call(with_token(
            post_json("/api/chats/admin%40localhost%3A5000/messages", json!({ "text": "note to self" })),
            &token,
        ))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = msg["id"].as_str().unwrap().to_string();

    let (status, list) = s
        .call(with_token(get("/api/chats/admin%40localhost%3A5000/messages"), &token))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["text"], json!("note to self"));

    let del = with_token(
        Request::builder()
            .method("DELETE")
            .uri(format!("/api/chats/admin%40localhost%3A5000/messages/{}", urlencoding(&id)))
            .body(Body::empty())
            .unwrap(),
        &token,
    );
    let (status, _, _) = s.call_raw(del).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u32),
        })
        .collect()
}

#[tokio::test]
async fn upload_quota_enforced() {
    let s = server().await;
    let admin = setup_admin(&s).await;

    // Register a normal user and shrink their quota to 1 MB.
    let (_, invite) = s
        .call(with_token(
            Request::builder().method("POST").uri("/api/invites/generate").body(Body::empty()).unwrap(),
            &admin,
        ))
        .await;
    let code = invite["code"].as_str().unwrap().to_string();
    s.call(post_json(
        "/api/auth/register",
        json!({ "username": "carol", "password": "secret1", "password_confirm": "secret1", "invite_code": code }),
    ))
    .await;

    let patch = with_token(
        Request::builder()
            .method("PATCH")
            .uri("/api/admin/users/carol/storage")
            .header("content-type", "application/json")
            .body(Body::from(json!({ "storage_limit_mb": 1 }).to_string()))
            .unwrap(),
        &admin,
    );
    let (status, _) = s.call(patch).await;
    assert_eq!(status, StatusCode::OK);

    let (_, carol) = s
        .call(post_json("/api/auth/login", json!({ "user": "carol", "password": "secret1" })))
        .await;
    let carol = carol["session_token"].as_str().unwrap().to_string();

    let big = vec![b'x'; 2 * 1024 * 1024];
    let boundary = "----bscptest";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"big.bin\"\r\n");
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(&big);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let req = with_token(
        Request::builder()
            .method("POST")
            .uri("/api/upload/")
            .header("content-type", format!("multipart/form-data; boundary={boundary}"))
            .body(Body::from(body))
            .unwrap(),
        &carol,
    );
    let (status, _) = s.call(req).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn two_factor_round_trip() {
    let s = server().await;
    let token = setup_admin(&s).await;

    let (status, cookies, setup) = s
        .call_raw(with_token(
            Request::builder().method("POST").uri("/api/users/me/2fa/setup").body(Body::empty()).unwrap(),
            &token,
        ))
        .await;
    assert_eq!(status, StatusCode::OK);
    let secret = setup["secret"].as_str().unwrap().to_string();
    let pending = cookies.iter().find(|c| c.starts_with("bscp_pending_2fa=")).cloned().unwrap();

    let code = bscp_common::totp::current_code(&secret).unwrap();
    let mut enable = with_token(post_json("/api/users/me/2fa/enable", json!({ "otp": code })), &token);
    enable.headers_mut().insert("cookie", pending.parse().unwrap());
    let (status, v) = s.call(enable).await;
    assert_eq!(status, StatusCode::OK, "{v:?}");
    assert_eq!(v["success"], json!(true));

    // Login now needs a second factor.
    let (_, _, login) = s
        .call_raw(post_json("/api/auth/login", json!({ "user": "admin", "password": "secret1" })))
        .await;
    assert_eq!(login["requires_2fa"], json!(true));
}
