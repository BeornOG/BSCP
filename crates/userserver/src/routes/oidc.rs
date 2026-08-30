//! OpenID Connect provider endpoints. Local users only — `sub` is always
//! `localuser@thisdomain`.

use crate::auth::{load_user_by_token, SESSION_COOKIE};
use crate::oidc::{clients, consent, scope_has, sha256_hex, tokens};
use crate::state::AppState;
use axum::extract::{Form, Path, Query, RawQuery, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::PrivateCookieJar;
use bscp_common::models::User;
use bscp_common::{now_ts, ApiError};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/.well-known/openid-configuration", get(discovery))
        .route("/oauth/jwks", get(jwks))
        .route("/oauth/register", post(register))
        .route("/oauth/register/:client_id", axum::routing::delete(delete_registration))
        .route("/oauth/authorize", get(authorize_get).post(authorize_post))
        .route("/oauth/token", post(token))
        .route("/oauth/userinfo", get(userinfo).post(userinfo))
        .route("/oauth/revoke", post(revoke))
}

async fn oidc_enabled(state: &AppState) -> bool {
    sqlx::query_scalar::<_, i64>("SELECT COALESCE((SELECT oidc_enabled FROM server_config WHERE id = 1), 1)")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(1)
        != 0
}

async fn session_user(state: &AppState, jar: &PrivateCookieJar, headers: &HeaderMap) -> Option<(User, String)> {
    let token = jar
        .get(SESSION_COOKIE)
        .map(|c| c.value().to_string())
        .or_else(|| headers.get("x-session-token").and_then(|v| v.to_str().ok()).map(String::from))?;
    let user = load_user_by_token(state, &token).await?;
    Some((user, token))
}

fn err_page(msg: &str) -> Response {
    (StatusCode::BAD_REQUEST, Html(format!("<h3>Authorization error</h3><p>{}</p>", html_escape(msg)))).into_response()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

// ── discovery / jwks ─────────────────────────────────────────────────

async fn discovery(State(state): State<AppState>) -> Json<Value> {
    let b = &state.cfg.public_url;
    Json(json!({
        "issuer": b,
        "authorization_endpoint": format!("{b}/oauth/authorize"),
        "token_endpoint": format!("{b}/oauth/token"),
        "userinfo_endpoint": format!("{b}/oauth/userinfo"),
        "jwks_uri": format!("{b}/oauth/jwks"),
        "registration_endpoint": format!("{b}/oauth/register"),
        "revocation_endpoint": format!("{b}/oauth/revoke"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "scopes_supported": ["openid", "profile", "email", "bscp:links"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post", "none"],
        "code_challenge_methods_supported": ["S256"],
        "claims_supported": ["sub", "iss", "aud", "exp", "iat", "auth_time", "nonce",
            "name", "preferred_username", "picture", "email", "email_verified", "bscp_links"]
    }))
}

async fn jwks(State(state): State<AppState>) -> Json<Value> {
    Json(state.oidc.jwks())
}

// ── dynamic registration ─────────────────────────────────────────────

async fn register(
    State(state): State<AppState>,
    Json(body): Json<clients::RegisterRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if !oidc_enabled(&state).await {
        return Err(ApiError::forbidden("OIDC is disabled on this server"));
    }
    let out = clients::register(&state, body).await?;
    Ok((StatusCode::CREATED, Json(out)))
}

async fn delete_registration(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(client_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let bearer = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(ApiError::unauthorized)?;
    clients::delete_registered(&state, &client_id, bearer).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── authorize ────────────────────────────────────────────────────────

#[derive(Deserialize, Clone)]
struct AuthorizeParams {
    client_id: String,
    redirect_uri: String,
    #[serde(default)]
    response_type: String,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    nonce: Option<String>,
    #[serde(default)]
    code_challenge: Option<String>,
    #[serde(default)]
    code_challenge_method: Option<String>,
    #[serde(default)]
    prompt: Option<String>,
}

fn redirect_err(redirect_uri: &str, error: &str, desc: &str, state: Option<&str>) -> Response {
    let mut u = format!("{redirect_uri}?error={error}&error_description={}", urlenc(desc));
    if let Some(s) = state {
        u.push_str(&format!("&state={}", urlenc(s)));
    }
    Redirect::to(&u).into_response()
}

fn urlenc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Filter a requested scope string to what this provider + client allow.
fn effective_scope(requested: &str, allowed: &str) -> String {
    let allow: Vec<&str> = allowed.split_whitespace().collect();
    let mut out: Vec<&str> = requested
        .split_whitespace()
        .filter(|s| crate::oidc::known_scopes().contains(s) && allow.contains(s))
        .collect();
    if !out.contains(&"openid") {
        out.insert(0, "openid");
    }
    out.dedup();
    out.join(" ")
}

async fn authorize_get(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    headers: HeaderMap,
    Query(p): Query<AuthorizeParams>,
    RawQuery(raw): RawQuery,
) -> Response {
    if !oidc_enabled(&state).await {
        return err_page("OIDC is disabled on this server");
    }
    let client = match clients::resolve(&state, &p.client_id).await {
        Ok(c) => c,
        Err(e) => return err_page(&e),
    };
    if !client.allows_redirect(&p.redirect_uri) {
        return err_page("redirect_uri is not registered for this client");
    }
    // redirect_uri is trusted from here on.
    if p.response_type != "code" {
        return redirect_err(&p.redirect_uri, "unsupported_response_type", "only code is supported", p.state.as_deref());
    }
    if client.is_public && p.code_challenge.is_none() {
        return redirect_err(&p.redirect_uri, "invalid_request", "PKCE (code_challenge) is required", p.state.as_deref());
    }
    if p.code_challenge.is_some() && p.code_challenge_method.as_deref() != Some("S256") {
        return redirect_err(&p.redirect_uri, "invalid_request", "code_challenge_method must be S256", p.state.as_deref());
    }
    let scope = effective_scope(&p.scope, &client.allowed_scope);

    let Some((user, token)) = session_user(&state, &jar, &headers).await else {
        let raw = raw.unwrap_or_default();
        let next = format!("/oauth/authorize?{raw}");
        let mut login = reqwest::Url::parse(&state.cfg.public_url).unwrap();
        login.set_path("/login");
        login.query_pairs_mut().append_pair("next", &next);
        return Redirect::to(login.as_str()).into_response();
    };

    let skip_consent = client.registered
        && p.prompt.as_deref() != Some("consent")
        && has_consent(&state, &user.id, &client.client_id, &scope).await;

    if skip_consent {
        return issue_and_redirect(&state, &client, &user, &p, &scope).await;
    }

    // render consent
    let hidden = hidden_fields(&p, &scope);
    let csrf = sha256_hex(&format!("{token}:{}", client.client_id));
    let subject = state.full_id(&user.username);
    let scope_list: Vec<&str> = scope.split_whitespace().collect();
    Html(consent::page(
        &client.name,
        client.logo_url.as_deref(),
        &subject,
        &scope_list,
        &hidden,
        &csrf,
        &format!("{}/oauth/authorize", state.cfg.public_url),
    ))
    .into_response()
}

fn hidden_fields(p: &AuthorizeParams, scope: &str) -> String {
    let mut f = String::new();
    let mut add = |k: &str, v: &str| {
        f.push_str(&format!(
            "<input type=\"hidden\" name=\"{}\" value=\"{}\">",
            k,
            v.replace('"', "&quot;")
        ));
    };
    add("client_id", &p.client_id);
    add("redirect_uri", &p.redirect_uri);
    add("response_type", "code");
    add("scope", scope);
    if let Some(s) = &p.state {
        add("state", s);
    }
    if let Some(n) = &p.nonce {
        add("nonce", n);
    }
    if let Some(c) = &p.code_challenge {
        add("code_challenge", c);
    }
    if let Some(m) = &p.code_challenge_method {
        add("code_challenge_method", m);
    }
    f
}

async fn has_consent(state: &AppState, user_id: &str, client_id: &str, scope: &str) -> bool {
    let existing: Option<String> =
        sqlx::query_scalar("SELECT scope FROM oauth_consents WHERE user_id = ? AND client_id = ?")
            .bind(user_id)
            .bind(client_id)
            .fetch_optional(&state.pool)
            .await
            .ok()
            .flatten();
    match existing {
        Some(granted) => scope.split_whitespace().all(|s| scope_has(&granted, s)),
        None => false,
    }
}

async fn issue_and_redirect(
    state: &AppState,
    client: &clients::ResolvedClient,
    user: &User,
    p: &AuthorizeParams,
    scope: &str,
) -> Response {
    match tokens::issue_code(
        state,
        &client.client_id,
        &user.id,
        &p.redirect_uri,
        scope,
        p.nonce.as_deref(),
        p.code_challenge.as_deref(),
        p.code_challenge_method.as_deref(),
        now_ts(),
    )
    .await
    {
        Ok(code) => {
            let mut u = format!("{}?code={}", p.redirect_uri, urlenc(&code));
            if let Some(s) = &p.state {
                u.push_str(&format!("&state={}", urlenc(s)));
            }
            Redirect::to(&u).into_response()
        }
        Err(e) => e.into_response(),
    }
}

async fn authorize_post(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    let get = |k: &str| form.get(k).cloned().unwrap_or_default();
    let p = AuthorizeParams {
        client_id: get("client_id"),
        redirect_uri: get("redirect_uri"),
        response_type: "code".into(),
        scope: get("scope"),
        state: form.get("state").cloned(),
        nonce: form.get("nonce").cloned(),
        code_challenge: form.get("code_challenge").cloned(),
        code_challenge_method: form.get("code_challenge_method").cloned(),
        prompt: None,
    };

    let client = match clients::resolve(&state, &p.client_id).await {
        Ok(c) => c,
        Err(e) => return err_page(&e),
    };
    if !client.allows_redirect(&p.redirect_uri) {
        return err_page("redirect_uri is not registered for this client");
    }
    let Some((user, token)) = session_user(&state, &jar, &headers).await else {
        return err_page("session expired — start again");
    };
    let expected_csrf = sha256_hex(&format!("{token}:{}", client.client_id));
    if get("csrf") != expected_csrf {
        return err_page("invalid form token");
    }

    if get("decision") != "approve" {
        return redirect_err(&p.redirect_uri, "access_denied", "user denied the request", p.state.as_deref());
    }

    let scope = effective_scope(&p.scope, &client.allowed_scope);
    let _ = sqlx::query(
        "INSERT INTO oauth_consents (user_id, client_id, scope, created_at) VALUES (?, ?, ?, ?) \
         ON CONFLICT(user_id, client_id) DO UPDATE SET scope = excluded.scope, created_at = excluded.created_at",
    )
    .bind(&user.id)
    .bind(&client.client_id)
    .bind(&scope)
    .bind(now_ts())
    .execute(&state.pool)
    .await;

    issue_and_redirect(&state, &client, &user, &p, &scope).await
}

// ── token ────────────────────────────────────────────────────────────

fn token_err(status: StatusCode, error: &str, desc: &str) -> Response {
    (status, Json(json!({ "error": error, "error_description": desc }))).into_response()
}

async fn token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    let get = |k: &str| form.get(k).map(String::as_str).unwrap_or_default();

    // client authentication
    let (basic_id, basic_secret) = parse_basic_auth(&headers);
    let client_id = if !basic_id.is_empty() { basic_id.clone() } else { get("client_id").to_string() };
    let client_secret = if !basic_secret.is_empty() { basic_secret } else { get("client_secret").to_string() };
    if client_id.is_empty() {
        return token_err(StatusCode::UNAUTHORIZED, "invalid_client", "client_id is required");
    }
    let client = match clients::resolve(&state, &client_id).await {
        Ok(c) => c,
        Err(_) => return token_err(StatusCode::UNAUTHORIZED, "invalid_client", "unknown client"),
    };
    if let Some(hash) = &client.secret_hash {
        if sha256_hex(&client_secret) != *hash {
            return token_err(StatusCode::UNAUTHORIZED, "invalid_client", "bad client secret");
        }
    }

    match get("grant_type") {
        "authorization_code" => {
            let code = get("code");
            let redirect_uri = get("redirect_uri");
            let verifier = get("code_verifier");
            let row = match tokens::consume_code(&state, code).await {
                Ok(Some(r)) => r,
                _ => return token_err(StatusCode::BAD_REQUEST, "invalid_grant", "code is invalid or expired"),
            };
            if row.client_id != client_id || row.redirect_uri != redirect_uri {
                return token_err(StatusCode::BAD_REQUEST, "invalid_grant", "code does not match this request");
            }
            if let Some(challenge) = &row.code_challenge {
                if verifier.is_empty()
                    || !tokens::verify_pkce(challenge, row.code_challenge_method.as_deref(), verifier)
                {
                    return token_err(StatusCode::BAD_REQUEST, "invalid_grant", "PKCE verification failed");
                }
            } else if client.is_public {
                return token_err(StatusCode::BAD_REQUEST, "invalid_grant", "PKCE required");
            }

            let Ok(Some(user)) = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
                .bind(&row.user_id)
                .fetch_optional(&state.pool)
                .await
            else {
                return token_err(StatusCode::BAD_REQUEST, "invalid_grant", "user no longer exists");
            };

            match tokens::issue_tokens(&state, &client_id, &user, &row.scope, row.nonce.as_deref(), row.auth_time).await {
                Ok(t) => token_json(t).into_response(),
                Err(e) => e.into_response(),
            }
        }
        "refresh_token" => {
            let rt = get("refresh_token");
            match tokens::rotate_refresh(&state, rt, &client_id).await {
                Ok(Some(t)) => token_json(t).into_response(),
                Ok(None) => token_err(StatusCode::BAD_REQUEST, "invalid_grant", "refresh token is invalid"),
                Err(e) => e.into_response(),
            }
        }
        other => token_err(StatusCode::BAD_REQUEST, "unsupported_grant_type", other),
    }
}

fn token_json(t: tokens::TokenSet) -> Json<Value> {
    Json(json!({
        "access_token": t.access,
        "token_type": "Bearer",
        "expires_in": t.expires_in,
        "refresh_token": t.refresh,
        "id_token": t.id_token,
        "scope": t.scope,
    }))
}

fn parse_basic_auth(headers: &HeaderMap) -> (String, String) {
    let Some(v) = headers.get("authorization").and_then(|v| v.to_str().ok()) else {
        return (String::new(), String::new());
    };
    let Some(b64) = v.strip_prefix("Basic ") else { return (String::new(), String::new()) };
    use base64::Engine;
    let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(b64) else {
        return (String::new(), String::new());
    };
    let s = String::from_utf8_lossy(&decoded);
    match s.split_once(':') {
        Some((id, secret)) => (
            urldecode(id),
            urldecode(secret),
        ),
        None => (String::new(), String::new()),
    }
}

fn urldecode(s: &str) -> String {
    percent_decode(s)
}
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(b);
                    i += 3;
                    continue;
                }
                out.push(bytes[i]);
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ── userinfo / revoke ────────────────────────────────────────────────

async fn userinfo(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(token) = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    else {
        return (StatusCode::UNAUTHORIZED, [("www-authenticate", "Bearer")], "missing bearer token").into_response();
    };
    let Ok(Some((user, scope))) = tokens::user_from_access(&state, token).await else {
        return (StatusCode::UNAUTHORIZED, [("www-authenticate", "Bearer error=\"invalid_token\"")], "invalid token")
            .into_response();
    };
    let mut claims = serde_json::Map::new();
    claims.insert("sub".into(), json!(state.full_id(&user.username)));
    tokens::add_profile_claims(&mut claims, &state, &user, &scope);
    if scope_has(&scope, "bscp:links") {
        claims.insert("bscp_links".into(), crate::modules::links::links_claim(&state, &user.id).await);
    }
    Json(Value::Object(claims)).into_response()
}

async fn revoke(State(state): State<AppState>, Form(form): Form<HashMap<String, String>>) -> StatusCode {
    if let Some(t) = form.get("token") {
        let _ = tokens::revoke(&state, t).await;
    }
    StatusCode::OK
}
