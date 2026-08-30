//! End-to-end OIDC provider flow (dynamic registration + PKCE) against the
//! in-process router, plus discovery shape and negative cases.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine as _;
use bscp_common::config::UserServerConfig;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

const B64: base64::engine::general_purpose::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;

async fn app() -> axum::Router {
    let dir = std::env::temp_dir().join(format!("bscp-oidc-{}", bscp_common::uuid()));
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

async fn body_bytes(resp: axum::response::Response) -> Vec<u8> {
    axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap().to_vec()
}

async fn json_of(resp: axum::response::Response) -> Value {
    serde_json::from_slice(&body_bytes(resp).await).unwrap_or(Value::Null)
}

fn post_json(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn post_form(uri: &str, form: &[(&str, &str)], cookie: Option<&str>) -> Request<Body> {
    let body = form
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding(v)))
        .collect::<Vec<_>>()
        .join("&");
    let mut b = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/x-www-form-urlencoded");
    if let Some(c) = cookie {
        b = b.header("cookie", c);
    }
    b.body(Body::from(body)).unwrap()
}

fn urlencoding(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

async fn setup_and_login(app: &axum::Router) -> String {
    let r = app
        .clone()
        .oneshot(post_json(
            "/api/auth/setup",
            json!({ "username": "admin", "password": "secret1", "password_confirm": "secret1" }),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CREATED);

    let r = app
        .clone()
        .oneshot(post_json("/api/auth/login", json!({ "user": "admin", "password": "secret1" })))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    r.headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn discovery_document_is_well_formed() {
    let app = app().await;
    let doc = json_of(app.clone().oneshot(Request::get("/.well-known/openid-configuration").body(Body::empty()).unwrap()).await.unwrap()).await;
    assert_eq!(doc["issuer"], "http://localhost:5000");
    assert_eq!(doc["authorization_endpoint"], "http://localhost:5000/oauth/authorize");
    assert_eq!(doc["code_challenge_methods_supported"][0], "S256");
    assert_eq!(doc["id_token_signing_alg_values_supported"][0], "RS256");

    let wk = json_of(app.oneshot(Request::get("/.well-known/BSCP/userserver").body(Body::empty()).unwrap()).await.unwrap()).await;
    assert_eq!(wk["oidc"]["issuer"], "http://localhost:5000");
    assert_eq!(wk["capabilities"]["oidc"], true);
}

#[tokio::test]
async fn full_auth_code_pkce_flow() {
    let app = app().await;
    let cookie = setup_and_login(&app).await;

    // 1. dynamic registration (public client)
    let reg = json_of(
        app.clone()
            .oneshot(post_json(
                "/oauth/register",
                json!({
                    "redirect_uris": ["https://app.example/cb"],
                    "client_name": "Test App",
                    "token_endpoint_auth_method": "none"
                }),
            ))
            .await
            .unwrap(),
    )
    .await;
    let client_id = reg["client_id"].as_str().unwrap().to_string();
    assert!(reg["client_secret"].is_null());

    // 2. PKCE
    let verifier = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ012345";
    let challenge = B64.encode(Sha256::digest(verifier.as_bytes()));

    // 3. authorize → consent page
    let authz = format!(
        "/oauth/authorize?response_type=code&client_id={}&redirect_uri={}&scope=openid+profile&state=xyz&nonce=n1&code_challenge={}&code_challenge_method=S256",
        client_id,
        urlencoding("https://app.example/cb"),
        challenge
    );
    let resp = app
        .clone()
        .oneshot(Request::get(&authz).header("cookie", &cookie).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let html = String::from_utf8(body_bytes(resp).await).unwrap();
    let csrf = between(&html, r#"name="csrf" value=""#, r#"""#).expect("csrf field");

    // 4. approve consent
    let resp = app
        .clone()
        .oneshot(post_form(
            "/oauth/authorize",
            &[
                ("client_id", &client_id),
                ("redirect_uri", "https://app.example/cb"),
                ("response_type", "code"),
                ("scope", "openid profile"),
                ("state", "xyz"),
                ("nonce", "n1"),
                ("code_challenge", &challenge),
                ("code_challenge_method", "S256"),
                ("csrf", &csrf),
                ("decision", "approve"),
            ],
            Some(&cookie),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let loc = resp.headers().get("location").unwrap().to_str().unwrap().to_string();
    assert!(loc.starts_with("https://app.example/cb?code="));
    assert!(loc.contains("state=xyz"));
    let code = between(&loc, "code=", "&").expect("code param");

    // 5. token exchange
    let tok = json_of(
        app.clone()
            .oneshot(post_form(
                "/oauth/token",
                &[
                    ("grant_type", "authorization_code"),
                    ("code", &code),
                    ("redirect_uri", "https://app.example/cb"),
                    ("code_verifier", verifier),
                    ("client_id", &client_id),
                ],
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    let id_token = tok["id_token"].as_str().expect("id_token");
    let access = tok["access_token"].as_str().unwrap().to_string();
    assert_eq!(tok["token_type"], "Bearer");

    // 6. verify id_token against JWKS
    let jwks = json_of(app.clone().oneshot(Request::get("/oauth/jwks").body(Body::empty()).unwrap()).await.unwrap()).await;
    let n = jwks["keys"][0]["n"].as_str().unwrap();
    let e = jwks["keys"][0]["e"].as_str().unwrap();
    let dk = jsonwebtoken::DecodingKey::from_rsa_components(n, e).unwrap();
    let mut val = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
    val.set_audience(&[&client_id]);
    let data = jsonwebtoken::decode::<Value>(id_token, &dk, &val).unwrap();
    assert_eq!(data.claims["sub"], "admin@localhost:5000");
    assert_eq!(data.claims["nonce"], "n1");
    assert_eq!(data.claims["iss"], "http://localhost:5000");
    assert_eq!(data.claims["preferred_username"], "admin@localhost:5000");

    // 7. userinfo
    let ui = json_of(
        app.clone()
            .oneshot(
                Request::get("/oauth/userinfo")
                    .header("authorization", format!("Bearer {access}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(ui["sub"], "admin@localhost:5000");
    assert_eq!(ui["name"], "admin");

    // 8. replayed code is rejected
    let replay = app
        .clone()
        .oneshot(post_form(
            "/oauth/token",
            &[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("redirect_uri", "https://app.example/cb"),
                ("code_verifier", verifier),
                ("client_id", &client_id),
            ],
            None,
        ))
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rejects_unregistered_redirect_uri() {
    let app = app().await;
    let cookie = setup_and_login(&app).await;
    let reg = json_of(
        app.clone()
            .oneshot(post_json(
                "/oauth/register",
                json!({ "redirect_uris": ["https://app.example/cb"], "token_endpoint_auth_method": "none" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    let client_id = reg["client_id"].as_str().unwrap();

    let authz = format!(
        "/oauth/authorize?response_type=code&client_id={}&redirect_uri={}&scope=openid&code_challenge=x&code_challenge_method=S256",
        client_id,
        urlencoding("https://evil.example/steal")
    );
    let resp = app
        .oneshot(Request::get(&authz).header("cookie", &cookie).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

fn between(haystack: &str, start: &str, end: &str) -> Option<String> {
    let i = haystack.find(start)? + start.len();
    let rest = &haystack[i..];
    let j = rest.find(end).unwrap_or(rest.len());
    Some(rest[..j].to_string())
}
