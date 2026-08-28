//! BSCP channel server — stores messages for `domain#channel#sub` paths and
//! exposes channel webhooks. Port of the old `channelserver.py`.

mod routes;

use axum::Router;
use bscp_common::config::{Args, ChannelServerConfig};
use bscp_common::discovery::Discovery;
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub domain: String,
    pub discovery: Arc<Discovery>,
}

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                if cfg!(debug_assertions) { "info,bscp_channelserver=debug".into() } else { "info".into() }
            }),
        )
        .init();

    let args = Args::parse();
    let cfg = ChannelServerConfig::load(&args)?;

    let pool = bscp_common::db::connect(&cfg.database_url()).await?;
    MIGRATOR.run(&pool).await?;

    let state = AppState { pool, domain: cfg.domain.clone(), discovery: Arc::new(Discovery::new()) };
    let app: Router = routes::router(state);

    let listener = bscp_common::net::listen(cfg.port).await?;
    tracing::info!("Channel server listening on :{} (dual-stack)", cfg.port);
    axum::serve(listener, app).await?;
    Ok(())
}
