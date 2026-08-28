//! `/api/invites` — invite code management (admin only).

use crate::auth::AdminUser;
use crate::state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use bscp_common::models::InviteCode;
use bscp_common::{now_ts, ApiError};
use serde_json::{json, Value};

const INVITE_TTL_SECS: f64 = 7.0 * 24.0 * 60.0 * 60.0;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/invites", get(list))
        .route("/api/invites/", get(list))
        .route("/api/invites/generate", post(generate))
}

fn serialize(i: &InviteCode) -> Value {
    json!({
        "id": i.id,
        "code": i.code,
        "status": if i.used_by.is_some() { "Used" } else { "Active" },
        "created_at": i.created_at,
        "expires_at": i.expires_at,
        "used_by": i.used_by,
    })
}

async fn list(State(state): State<AppState>, _admin: AdminUser) -> Result<Json<Value>, ApiError> {
    let invites = sqlx::query_as::<_, InviteCode>("SELECT * FROM invite_codes").fetch_all(&state.pool).await?;
    Ok(Json(json!(invites.iter().map(serialize).collect::<Vec<_>>())))
}

async fn generate(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let code = bscp_common::random_hex(8);
    let now = now_ts();
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO invite_codes (code, created_by, created_at, expires_at) VALUES (?, ?, ?, ?) RETURNING id",
    )
    .bind(&code)
    .bind(&admin.0.id)
    .bind(now)
    .bind(now + INVITE_TTL_SECS)
    .fetch_one(&state.pool)
    .await?;

    let invite = sqlx::query_as::<_, InviteCode>("SELECT * FROM invite_codes WHERE id = ?")
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    Ok((StatusCode::CREATED, Json(serialize(&invite))))
}
