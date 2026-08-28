//! `/api/upload` — file uploads, storage quota, listing.

use crate::auth::AuthUser;
use crate::state::AppState;
use crate::util::{secure_filename, take_file_field};
use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use bscp_common::models::Upload;
use bscp_common::{now_ts, uuid, ApiError};
use serde_json::{json, Value};

const VIDEO_MIMETYPES: &[&str] = &[
    "video/mp4", "video/webm", "video/ogg", "video/quicktime", "video/x-msvideo", "video/x-flv",
    "video/x-matroska",
];

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/upload", post(upload))
        .route("/api/upload/", post(upload))
        .route("/api/upload/user/list", get(list))
        .route("/api/upload/:upload_id", axum::routing::delete(delete_upload))
}

async fn upload(
    State(state): State<AppState>,
    auth: AuthUser,
    multipart: Multipart,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let (orig, ct, data) = take_file_field(multipart, "No file provided").await?;
    if orig.is_empty() {
        return Err(ApiError::bad_request("No filename"));
    }
    let mimetype = if ct.is_empty() { "application/octet-stream".to_string() } else { ct };
    let file_size = data.len() as i64;

    if !auth.user.is_primary_admin {
        let limit_bytes = auth.user.storage_limit_mb * 1024 * 1024;
        if file_size > limit_bytes {
            return Err(ApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("File exceeds size limit of {}MB", auth.user.storage_limit_mb),
            ));
        }
        let total: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(size_bytes), 0) FROM uploads WHERE uploaded_by = ?",
        )
        .bind(&auth.user.id)
        .fetch_one(&state.pool)
        .await?;
        if total + file_size > limit_bytes {
            return Err(ApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                format!(
                    "Insufficient storage. Used: {}MB / {}MB",
                    total / (1024 * 1024),
                    auth.user.storage_limit_mb
                ),
            ));
        }
    }

    let filename = secure_filename(&format!("{}_{}", uuid(), orig));
    let path = state.cfg.upload_dir.join(&filename);
    tokio::fs::write(&path, &data).await.map_err(|e| ApiError::internal(format!("write failed: {e}")))?;

    sqlx::query(
        "INSERT INTO uploads (id, filename, mimetype, size_bytes, uploaded_by, created_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(uuid())
    .bind(&filename)
    .bind(&mimetype)
    .bind(file_size)
    .bind(&auth.user.id)
    .bind(now_ts())
    .execute(&state.pool)
    .await?;

    let file_url = format!("http://{}/uploads/{}", state.domain(), filename);
    let markdown = if VIDEO_MIMETYPES.contains(&mimetype.as_str()) {
        file_url.clone()
    } else {
        format!("![image]({file_url})")
    };

    Ok((StatusCode::CREATED, Json(json!({ "url": file_url, "mimetype": mimetype, "markdown": markdown }))))
}

async fn delete_upload(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(upload_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let upload = sqlx::query_as::<_, Upload>("SELECT * FROM uploads WHERE id = ?")
        .bind(&upload_id)
        .fetch_optional(&state.pool)
        .await?;
    let Some(upload) = upload else {
        return Err(ApiError::not_found("Upload not found"));
    };
    if upload.uploaded_by != auth.user.id {
        return Err(ApiError::forbidden("Cannot delete other user's uploads"));
    }

    let path = state.cfg.upload_dir.join(&upload.filename);
    let _ = tokio::fs::remove_file(&path).await;

    sqlx::query("DELETE FROM uploads WHERE id = ?").bind(&upload_id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list(State(state): State<AppState>, auth: AuthUser) -> Result<Json<Value>, ApiError> {
    let uploads = sqlx::query_as::<_, Upload>("SELECT * FROM uploads WHERE uploaded_by = ?")
        .bind(&auth.user.id)
        .fetch_all(&state.pool)
        .await?;
    let total: i64 = uploads.iter().map(|u| u.size_bytes).sum();
    let limit_bytes = auth.user.storage_limit_mb * 1024 * 1024;

    let items: Vec<Value> = uploads
        .iter()
        .map(|u| {
            json!({
                "id": u.id,
                "filename": u.filename,
                "mimetype": u.mimetype,
                "size_bytes": u.size_bytes,
                "created_at": u.created_at,
            })
        })
        .collect();

    Ok(Json(json!({ "uploads": items, "total_size_bytes": total, "limit_bytes": limit_bytes })))
}
