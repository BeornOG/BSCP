//! Row types for the guild schema.

use serde::Serialize;
use sqlx::FromRow;

#[derive(FromRow, Serialize, Clone)]
pub struct Guild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub owner: String,
    pub created_at: f64,
}

#[derive(FromRow, Serialize, Clone)]
pub struct Role {
    pub id: String,
    pub guild_id: String,
    pub name: String,
    pub color: Option<String>,
    pub position: i64,
    pub permissions: i64,
    pub is_everyone: bool,
}

#[derive(FromRow, Serialize, Clone)]
pub struct Channel {
    pub id: String,
    pub guild_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub kind: String,
    pub topic: Option<String>,
    pub position: i64,
    pub path: String,
}

