//! Database row types for the user server. Time columns are stored as REAL
//! (fractional unix seconds), matching Python's `datetime.timestamp()`.

use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct User {
    pub id: String,
    pub username: String,
    #[serde(skip)]
    pub password_hash: String,
    pub email: Option<String>,
    #[serde(skip)]
    pub otp_secret: String,
    pub is_2fa_enabled: bool,
    pub is_admin: bool,
    pub is_primary_admin: bool,
    pub is_deleted: bool,
    pub storage_limit_mb: i64,
    pub display_name: Option<String>,
    pub theme: String,
    pub accent_color: String,
    pub bio: Option<String>,
    pub profile_pic: Option<String>,
    pub status_text: Option<String>,
    pub status_type: i64,
    pub created_at: f64,
}

#[derive(Debug, Clone, FromRow)]
pub struct UserSession {
    pub id: String,
    pub user_id: String,
    pub token: String,
    pub device_info: Option<String>,
    pub last_active: f64,
    pub expires_at: f64,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Message {
    pub id: String,
    pub sender: String,
    pub receiver: String,
    pub text: String,
    #[serde(skip)]
    pub validation_key: Option<String>,
    pub timestamp: f64,
    pub is_read: bool,
}

#[derive(Debug, Clone, FromRow)]
pub struct PushSubscription {
    pub id: String,
    pub user_id: String,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub created_at: f64,
    pub updated_at: f64,
}

#[derive(Debug, Clone, FromRow)]
pub struct Upload {
    pub id: String,
    pub filename: String,
    pub mimetype: String,
    pub size_bytes: i64,
    pub uploaded_by: String,
    pub created_at: f64,
}

#[derive(Debug, Clone, FromRow)]
pub struct ServerConfig {
    pub id: i64,
    pub storage_limit_mb: i64,
    pub updated_at: f64,
}

#[derive(Debug, Clone, FromRow)]
pub struct InviteCode {
    pub id: i64,
    pub code: String,
    pub created_by: String,
    pub used_by: Option<String>,
    pub created_at: f64,
    pub used_at: Option<f64>,
    pub expires_at: Option<f64>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Webhook {
    pub id: String,
    pub user_id: String,
    pub channel_id: Option<String>,
    pub name: String,
    pub token: String,
    pub profile_pic: Option<String>,
    pub created_at: f64,
    pub last_used: Option<f64>,
}
