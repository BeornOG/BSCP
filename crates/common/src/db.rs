//! SQLite pool helpers.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

/// Open (creating if needed) a SQLite pool with sane pragmas for a small server.
pub async fn connect(database_url: &str) -> anyhow::Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_secs(10))
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(opts)
        .await?;
    Ok(pool)
}
