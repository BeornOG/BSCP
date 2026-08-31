//! BSCP channel server — hosts federated guilds (text + voice channels, roles &
//! permissions), channel webhooks, and a minimal operator console.

mod auth;
mod call_ws;
mod models;
mod oidc_client;
mod perms;
mod routes;
mod state;
mod webui;

use bscp_common::config::{Args, ChannelServerConfig};
use state::AppState;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            if cfg!(debug_assertions) {
                "info,bscp_channelserver=debug,bscp_common=debug".into()
            } else {
                "info".into()
            }
        }))
        .init();

    let args = Args::parse();
    let cfg = ChannelServerConfig::load(&args)?;

    let pool = bscp_common::db::connect(&cfg.database_url()).await?;
    MIGRATOR.run(&pool).await?;

    let state = AppState::new(pool, &cfg);
    let app = routes::router(state);

    let listener = bscp_common::net::listen(cfg.port).await?;
    tracing::info!("Channel server listening on :{} (dual-stack)", cfg.port);
    axum::serve(listener, app).await?;
    Ok(())
}
