//! Unified API error type. Response body shape matches what the React frontend
//! expects (`api.ts`): `{ "message": "...", "errors": ["..."] }`.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
    pub errors: Vec<String>,
}

pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self { status, message: message.into(), errors: Vec::new() }
    }
    pub fn with_errors(status: StatusCode, message: impl Into<String>, errors: Vec<String>) -> Self {
        Self { status, message: message.into(), errors }
    }
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }
    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "Authentication required")
    }
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, message)
    }
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }
    pub fn bad_gateway(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_GATEWAY, message)
    }
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.status, self.message)
    }
}
impl std::error::Error for ApiError {}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut body = json!({ "message": self.message, "code": self.status.as_u16() });
        if !self.errors.is_empty() {
            body["errors"] = json!(self.errors);
        }
        (self.status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!(error = %e, "database error");
        ApiError::internal("Database error")
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!(error = %e, "internal error");
        ApiError::internal("Internal error")
    }
}
