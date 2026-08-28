//! Call signaling WebSocket routes.

use crate::call::ws;
use crate::state::AppState;
use axum::routing::get;
use axum::Router;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/calls/ws", get(ws::browser_ws))
        .route("/calls/manager/ws", get(ws::manager_ws))
}
