//! Server-rendered operator console (no build step).

use crate::auth::{Operator, OP_COOKIE};
use crate::oidc_client;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use axum_extra::extract::cookie::{Cookie, PrivateCookieJar, SameSite};
use bscp_common::{now_ts, uuid};
use serde::Deserialize;

const SESSION_TTL: f64 = 7.0 * 24.0 * 3600.0;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/oauth/callback", get(callback))
        .route("/admin/guild-creators", post(guild_creators))
        .route("/invite/:code", get(invite_landing))
        .route("/invite/:code/go", get(invite_go))
}

#[derive(Deserialize)]
struct GoQuery {
    idp: String,
}

async fn invite_landing(State(state): State<AppState>, Path(code): Path<String>) -> Response {
    let row: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT g.name, g.icon FROM guild_invites i JOIN guilds g ON g.id = i.guild_id WHERE i.code = ?",
    )
    .bind(&code)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();
    let Some((name, icon)) = row else {
        return page("Invite", "<div class=card>This invite link is not valid.</div>").into_response();
    };
    let icon_html = icon
        .filter(|s| !s.is_empty())
        .map(|u| format!("<img src='{u}' width=64 height=64 style='border-radius:16px'>"))
        .unwrap_or_default();
    page(
        &format!("Join {name}"),
        &format!(
            "<div class=card>{icon_html}<h2>You're invited to <b>{name}</b></h2>\
             <p>Accepting happens through your own BSCP server.</p>\
             <form method=get action='/invite/{code}/go'>\
             <input name=idp placeholder='you@your-server.example' size=30 required> \
             <button>Accept invite</button></form></div>"
        ),
    )
    .into_response()
}

async fn invite_go(State(state): State<AppState>, Path(code): Path<String>, Query(q): Query<GoQuery>) -> Response {
    let host = q.idp.rsplit('@').next().unwrap_or(&q.idp).trim();
    let invite_url = format!("{}/invite/{}", state.public_url.trim_end_matches('/'), code);
    let scheme = if host.starts_with("localhost") || host.starts_with("127.") { "http" } else { "https" };
    let target = format!(
        "{scheme}://{host}/join?invite={}",
        urlencoding(&invite_url)
    );
    Redirect::to(&target).into_response()
}

fn urlencoding(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn page(title: &str, body: &str) -> Html<String> {
    Html(format!(
        "<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'>\
         <title>{title}</title>\
         <style>body{{font:15px/1.5 system-ui,sans-serif;max-width:760px;margin:3rem auto;padding:0 1rem;color:#e8eaed;background:#0f0f11}}\
         h1,h2{{font-weight:600}} a{{color:#7eafff}} input,button{{font:inherit;padding:.5rem;border-radius:6px;border:1px solid #333;background:#1a1a1d;color:inherit}}\
         button{{cursor:pointer;background:#2b3b57;border-color:#3a4a66}} table{{border-collapse:collapse;width:100%;margin:1rem 0}}\
         td,th{{border-bottom:1px solid #262626;padding:.5rem;text-align:left}} form.inline{{display:inline}}\
         .card{{background:#151517;border:1px solid #232529;border-radius:12px;padding:1.25rem;margin:1rem 0}}</style>\
         <h1>BSCP channel server</h1>{body}"
    ))
}

async fn index(state: State<AppState>, op: Result<Operator, crate::auth::AuthError>) -> Response {
    match op {
        Ok(op) => console(state, op).await.into_response(),
        Err(_) => signin_page().into_response(),
    }
}

fn signin_page() -> Html<String> {
    page(
        "Operator sign-in",
        "<div class=card><h2>Operator sign-in</h2>\
         <p>Sign in with your BSCP account. The first person to sign in claims this server.</p>\
         <form method=post action=/login>\
         <input name=idp placeholder='you@your-server.example' size=32 required> \
         <button>Sign in with BSCP</button></form></div>",
    )
}

#[derive(Deserialize)]
struct LoginForm {
    idp: String,
}

async fn login(State(state): State<AppState>, Form(f): Form<LoginForm>) -> Response {
    oidc_client::gc_states(&state).await;
    match oidc_client::begin(&state, f.idp.trim()).await {
        Ok(url) => Redirect::to(&url).into_response(),
        Err(e) => page("Sign-in error", &format!("<div class=card><p>Could not start sign-in: {e}</p><p><a href=/>back</a></p></div>"))
            .into_response(),
    }
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

async fn callback(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Query(q): Query<CallbackQuery>,
) -> Response {
    if let Some(e) = q.error {
        return page("Sign-in cancelled", &format!("<div class=card><p>{e}</p><a href=/>back</a></div>")).into_response();
    }
    let (Some(code), Some(st)) = (q.code, q.state) else {
        return page("Sign-in error", "<div class=card>missing code</div>").into_response();
    };

    let ident = match oidc_client::complete(&state, &code, &st).await {
        Ok(i) => i,
        Err(e) => {
            return page("Sign-in error", &format!("<div class=card><p>{e}</p><a href=/>back</a></div>")).into_response()
        }
    };

    // first sign-in claims the operator role
    let current: Option<String> =
        sqlx::query_scalar::<_, Option<String>>("SELECT operator_sub FROM operator_config WHERE id = 1")
            .fetch_optional(&state.pool)
            .await
            .ok()
            .flatten()
            .flatten()
            .filter(|s| !s.is_empty());
    match current {
        None => {
            sqlx::query("UPDATE operator_config SET operator_sub = ? WHERE id = 1")
                .bind(&ident.sub)
                .execute(&state.pool)
                .await
                .ok();
        }
        Some(sub) if sub != ident.sub => {
            return page("Not authorised", "<div class=card>This server already has an operator.</div>")
                .into_response();
        }
        _ => {}
    }

    let sid = uuid();
    sqlx::query("INSERT INTO operator_sessions (id, sub, expires_at) VALUES (?, ?, ?)")
        .bind(&sid)
        .bind(&ident.sub)
        .bind(now_ts() + SESSION_TTL)
        .execute(&state.pool)
        .await
        .ok();
    let mut cookie = Cookie::new(OP_COOKIE, sid);
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_path("/");
    (jar.add(cookie), Redirect::to("/")).into_response()
}

async fn logout(State(state): State<AppState>, jar: PrivateCookieJar) -> Response {
    if let Some(c) = jar.get(OP_COOKIE) {
        sqlx::query("DELETE FROM operator_sessions WHERE id = ?").bind(c.value()).execute(&state.pool).await.ok();
    }
    let mut gone = Cookie::from(OP_COOKIE);
    gone.set_path("/");
    (jar.remove(gone), Redirect::to("/")).into_response()
}

#[derive(Deserialize)]
struct CreatorForm {
    action: String,
    user_id: String,
}

async fn guild_creators(State(state): State<AppState>, _op: Operator, Form(f): Form<CreatorForm>) -> Response {
    let uid = f.user_id.trim().to_lowercase();
    if !uid.is_empty() && uid.contains('@') {
        match f.action.as_str() {
            "add" => {
                sqlx::query("INSERT OR IGNORE INTO guild_creators (user_id) VALUES (?)")
                    .bind(&uid)
                    .execute(&state.pool)
                    .await
                    .ok();
            }
            "remove" => {
                sqlx::query("DELETE FROM guild_creators WHERE user_id = ?").bind(&uid).execute(&state.pool).await.ok();
            }
            _ => {}
        }
    }
    Redirect::to("/").into_response()
}

async fn console(State(state): State<AppState>, op: Operator) -> Html<String> {
    let creators: Vec<String> = sqlx::query_scalar("SELECT user_id FROM guild_creators ORDER BY user_id")
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();
    let guilds: Vec<(String, String, String, i64, i64, f64)> = sqlx::query_as(
        "SELECT g.id, g.name, g.owner, \
           (SELECT COUNT(*) FROM guild_members m WHERE m.guild_id = g.id), \
           (SELECT COUNT(*) FROM channels c WHERE c.guild_id = g.id), g.created_at \
         FROM guilds g ORDER BY g.created_at DESC",
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let creator_rows: String = creators
        .iter()
        .map(|u| {
            format!(
                "<tr><td>{u}</td><td><form class=inline method=post action=/admin/guild-creators>\
                 <input type=hidden name=action value=remove><input type=hidden name=user_id value='{u}'>\
                 <button>remove</button></form></td></tr>"
            )
        })
        .collect();

    let guild_rows: String = guilds
        .iter()
        .map(|(id, name, owner, members, chans, created)| {
            format!(
                "<tr><td>{name}</td><td>{owner}</td><td>{members}</td><td>{chans}</td>\
                 <td>{}</td><td><code>{}</code></td></tr>",
                fmt_date(*created),
                &id[..id.len().min(8)]
            )
        })
        .collect();

    page(
        "Operator console",
        &format!(
            "<p>Signed in as <b>{}</b> · <form class=inline method=post action=/logout><button>sign out</button></form></p>\
             <div class=card><h2>Guild creators</h2>\
             <table><tr><th>user@domain</th><th></th></tr>{creator_rows}</table>\
             <form method=post action=/admin/guild-creators>\
             <input type=hidden name=action value=add>\
             <input name=user_id placeholder='user@domain' size=28 required> <button>allow</button></form></div>\
             <div class=card><h2>Guilds</h2>\
             <table><tr><th>name</th><th>owner</th><th>members</th><th>channels</th><th>created</th><th>id</th></tr>\
             {guild_rows}</table></div>",
            op.sub
        ),
    )
}

fn fmt_date(ts: f64) -> String {
    // cheap YYYY-MM-DD from unix seconds
    let days = (ts / 86400.0) as i64;
    let mut y = 1970i64;
    let mut d = days;
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
        let dy = if leap { 366 } else { 365 };
        if d < dy {
            break;
        }
        d -= dy;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let months = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m = 0;
    while d >= months[m] {
        d -= months[m];
        m += 1;
    }
    format!("{y:04}-{:02}-{:02}", m + 1, d + 1)
}
