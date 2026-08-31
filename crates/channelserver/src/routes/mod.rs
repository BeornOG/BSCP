pub mod channels;
pub mod guilds;
pub mod invites;
pub mod legacy;
pub mod members;
pub mod messages;
pub mod roles;
pub mod voice;
pub mod webhooks;

use crate::state::AppState;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use serde_json::json;

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(guilds::router())
        .merge(channels::router())
        .merge(roles::router())
        .merge(members::router())
        .merge(messages::router())
        .merge(invites::router())
        .merge(voice::router())
        .merge(webhooks::router())
        .merge(legacy::router())
        .merge(crate::webui::router())
        .merge(crate::call_ws::router())
        .with_state(state)
}

/// JSON error helper.
pub struct ApiErr(pub StatusCode, pub String);
impl ApiErr {
    pub fn new(code: StatusCode, msg: impl Into<String>) -> Self {
        Self(code, msg.into())
    }
    pub fn not_found(m: impl Into<String>) -> Self {
        Self(StatusCode::NOT_FOUND, m.into())
    }
    pub fn bad(m: impl Into<String>) -> Self {
        Self(StatusCode::BAD_REQUEST, m.into())
    }
    pub fn forbidden(m: impl Into<String>) -> Self {
        Self(StatusCode::FORBIDDEN, m.into())
    }
}
impl IntoResponse for ApiErr {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}
impl From<sqlx::Error> for ApiErr {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!(error = %e, "db error");
        Self(StatusCode::INTERNAL_SERVER_ERROR, "database error".into())
    }
}
impl From<crate::auth::AuthError> for ApiErr {
    fn from(e: crate::auth::AuthError) -> Self {
        Self(e.0, e.1.into())
    }
}

pub type ApiResult<T> = Result<T, ApiErr>;

/// Confirm a guild exists; returns its owner.
pub async fn guild_owner(state: &AppState, gid: &str) -> ApiResult<String> {
    sqlx::query_scalar::<_, String>("SELECT owner FROM guilds WHERE id = ?")
        .bind(gid)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiErr::not_found("guild not found"))
}

/// Look up a channel's guild id + path.
pub async fn channel_ctx(state: &AppState, cid: &str) -> ApiResult<(String, String, String)> {
    sqlx::query_as::<_, (String, String, String)>(
        "SELECT guild_id, path, kind FROM channels WHERE id = ?",
    )
    .bind(cid)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiErr::not_found("channel not found"))
}
