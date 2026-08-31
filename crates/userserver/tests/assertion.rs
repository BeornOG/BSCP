//! Federation-assertion mint + issuer-callback round-trip.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use bscp_common::assertion::AssertionClaims;
use bscp_common::config::UserServerConfig;
use bscp_common::models::User;
use serde_json::{json, Value};
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

async fn setup() -> (axum::Router, bscp_userserver::state::AppState) {
    let dir = std::env::temp_dir().join(format!("bscp-assert-{}", bscp_common::uuid()));
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
    let pool = SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    bscp_userserver::MIGRATOR.run(&pool).await.unwrap();
    let state = bscp_userserver::make_state(cfg, pool).unwrap();
    (bscp_userserver::routes::build(state.clone()), state)
}

async fn make_user(state: &bscp_userserver::state::AppState) -> User {
    sqlx::query(
        "INSERT INTO users (id, username, password_hash, otp_secret, created_at) VALUES (?, 'alice', 'x', 'x', ?)",
    )
    .bind(bscp_common::uuid())
    .bind(bscp_common::now_ts())
    .execute(&state.pool)
    .await
    .unwrap();
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = 'alice'")
        .fetch_one(&state.pool)
        .await
        .unwrap()
}

async fn verify_via_callback(app: &axum::Router, token: &str) -> Value {
    let req = Request::builder()
        .method("POST")
        .uri("/federation/assert/verify")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "token": token }).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn mint_then_verify_roundtrip() {
    let (app, state) = setup().await;
    let user = make_user(&state).await;

    let token = bscp_userserver::guilds::assert::assertion_for(&state, &user, "chan.example")
        .await
        .unwrap();

    // claims look right
    let claims: AssertionClaims = state.oidc.verify(&token, Some("chan.example")).unwrap();
    assert_eq!(claims.iss, "http://localhost:5000");
    assert_eq!(claims.sub, "alice@localhost:5000");
    assert_eq!(claims.aud, "chan.example");

    // issuer callback confirms it
    let v = verify_via_callback(&app, &token).await;
    assert_eq!(v["valid"], json!(true), "{v:?}");
    assert_eq!(v["name"], json!("alice"));

    // a forged token (unknown jti) is rejected
    let forged = state
        .oidc
        .sign(&AssertionClaims {
            iss: "http://localhost:5000".into(),
            sub: "alice@localhost:5000".into(),
            aud: "chan.example".into(),
            exp: (bscp_common::now_ts() + 100.0) as i64,
            iat: bscp_common::now_ts() as i64,
            jti: "not-a-real-jti".into(),
            name: None,
            picture: None,
        })
        .unwrap();
    let v = verify_via_callback(&app, &forged).await;
    assert_eq!(v["valid"], json!(false));

    // garbage token
    let v = verify_via_callback(&app, "garbage.token.value").await;
    assert_eq!(v["valid"], json!(false));
}

#[tokio::test]
async fn assertion_is_cached() {
    let (_app, state) = setup().await;
    let user = make_user(&state).await;
    let a = bscp_userserver::guilds::assert::assertion_for(&state, &user, "chan.example").await.unwrap();
    let b = bscp_userserver::guilds::assert::assertion_for(&state, &user, "chan.example").await.unwrap();
    assert_eq!(a, b, "same token returned from cache");
    let c = bscp_userserver::guilds::assert::assertion_for(&state, &user, "other.example").await.unwrap();
    assert_ne!(a, c, "different aud → different token");
    let _ = StatusCode::OK;
}
