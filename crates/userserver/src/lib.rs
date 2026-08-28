//! BSCP user server library — federated DMs, media proxy, webhooks, 2FA, admin.
//! Rust port of the Flask `app.py` + `web.py` + `routes/*` + `federation.py`.

pub mod auth;
pub mod media;
#[cfg(debug_assertions)]
pub mod openapi;
pub mod profile;
pub mod routes;
pub mod state;
pub mod status;
pub mod tasks;
pub mod util;

use bscp_common::config::{Args, UserServerConfig};
use bscp_common::discovery::Discovery;
use sqlx::SqlitePool;
use state::{derive_cookie_key, AppState};
use std::sync::Arc;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Build the fully-wired application state from a config and an open pool.
pub fn make_state(cfg: UserServerConfig, pool: SqlitePool) -> AppState {
    let vapid = bscp_common::push::load_or_generate(
        &cfg.vapid_private_key,
        &cfg.vapid_public_key,
        &cfg.vapid_contact,
        &cfg.vapid_keys_file,
    );
    let cookie_key = derive_cookie_key(&cfg.secret_key);
    AppState {
        pool,
        cfg: Arc::new(cfg),
        discovery: Arc::new(Discovery::new()),
        vapid: Arc::new(vapid),
        cookie_key,
    }
}

/// Entry point used by the binary.
pub async fn run() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            if cfg!(debug_assertions) {
                "info,bscp_userserver=debug,bscp_common=debug,tower_http=debug".into()
            } else {
                "info".into()
            }
        }))
        .init();

    let args = Args::parse();
    let cfg = UserServerConfig::load(&args)?;
    let port = cfg.port;

    let pool = bscp_common::db::connect(&cfg.database_url()).await?;
    MIGRATOR.run(&pool).await?;

    let state = make_state(cfg, pool);
    tasks::spawn_cache_cleanup(state.cfg.clone());

    let app = routes::build(state);
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("BSCP user server listening on {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
