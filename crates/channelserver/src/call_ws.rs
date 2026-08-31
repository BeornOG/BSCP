//! `/calls/manager/ws` — a member's user server attaches to a voice room. The
//! channel server relays SDP/ICE/roster between participant servers; it never
//! touches media. Mirrors the user server's DM-call manager WS.

use crate::state::AppState;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use bscp_common::call::manager::ParticipantServer;
use bscp_common::call::SignalMsg;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;

pub fn router() -> Router<AppState> {
    Router::new().route("/calls/manager/ws", get(manager_ws))
}

#[derive(Deserialize)]
struct Params {
    call_id: String,
    token: String,
    server: String,
}

async fn manager_ws(State(state): State<AppState>, Query(p): Query<Params>, ws: WebSocketUpgrade) -> Response {
    if !state.calls.verify_token(&p.call_id, &p.server, &p.token) {
        return (axum::http::StatusCode::UNAUTHORIZED, "bad call token").into_response();
    }
    ws.on_upgrade(move |socket| peer_loop(socket, state, p))
}

async fn peer_loop(socket: WebSocket, state: AppState, p: Params) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<SignalMsg>();

    let pump = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let txt = serde_json::to_string(&msg).unwrap_or_else(|_| "{}".into());
            if ws_tx.send(Message::Text(txt)).await.is_err() {
                break;
            }
        }
    });

    let mut joined = false;
    while let Some(Ok(frame)) = ws_rx.next().await {
        let Message::Text(t) = frame else {
            if matches!(frame, Message::Close(_)) {
                break;
            }
            continue;
        };
        let Ok(sig) = serde_json::from_str::<SignalMsg>(&t) else { continue };
        match sig {
            SignalMsg::Join { members, .. } if !joined => {
                joined = true;
                let _ = state.calls.join(
                    &p.call_id,
                    ParticipantServer { domain: p.server.clone(), members, muted: vec![], sink: tx.clone() },
                );
            }
            SignalMsg::Sdp { .. } | SignalMsg::Ice { .. } | SignalMsg::Mute { .. } => {
                state.calls.route(sig);
            }
            SignalMsg::Leave { .. } | SignalMsg::Hangup { .. } => break,
            _ => {}
        }
    }

    pump.abort();
    // a room persists even when empty
    state.calls.leave(&p.call_id, &p.server);
}
