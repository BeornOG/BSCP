//! Call signaling WebSockets:
//! - `GET /api/calls/ws`      — a browser ↔ its own user server
//! - `GET /calls/manager/ws`  — a peer user server ↔ this server's call manager

use crate::auth::{load_user_by_token, SESSION_COOKIE};
use crate::call::{engine, PendingInvite};
use crate::profile::get_profile;
use crate::routes::chats::{store_and_deliver, OutgoingMessage};
use crate::state::AppState;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::PrivateCookieJar;
use bscp_common::call::manager::ParticipantServer;
use bscp_common::call::{CallKind, SignalMsg};
use bscp_common::models::User;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;

fn ws_json(msg: &SignalMsg) -> Message {
    Message::Text(serde_json::to_string(msg).unwrap_or_else(|_| "{}".into()))
}

fn err_to(state: &AppState, user_id: &str, message: &str) {
    state.calls.notify_user(user_id, &SignalMsg::Error { message: message.into() });
}

// ════════════════════════════ browser side ════════════════════════════

#[derive(Deserialize)]
pub struct WsAuthQuery {
    token: Option<String>,
}

/// Resolve the session token from `?token=`, the private session cookie, or the
/// `X-Session-Token` header (browsers send the cookie automatically on the
/// same-origin WS handshake; other clients pass `?token=`).
pub async fn browser_ws(
    State(state): State<AppState>,
    Query(q): Query<WsAuthQuery>,
    jar: PrivateCookieJar,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    let token = q
        .token
        .or_else(|| jar.get(SESSION_COOKIE).map(|c| c.value().to_string()))
        .or_else(|| headers.get("x-session-token").and_then(|v| v.to_str().ok()).map(String::from));

    let Some(user) = (match token {
        Some(t) => load_user_by_token(&state, &t).await,
        None => None,
    }) else {
        return (axum::http::StatusCode::UNAUTHORIZED, "authentication required").into_response();
    };

    ws.on_upgrade(move |socket| browser_loop(socket, state, user))
}

async fn browser_loop(socket: WebSocket, state: AppState, user: User) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<SignalMsg>();
    let conn_id = state.calls.register_browser(&user.id, tx);

    let pump = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_tx.send(ws_json(&msg)).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(frame)) = ws_rx.next().await {
        let text = match frame {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue,
        };
        let Ok(sig) = serde_json::from_str::<SignalMsg>(&text) else { continue };
        match sig {
            SignalMsg::StartCall { to } => start_direct_call(&state, &user, &to).await,
            SignalMsg::Accept { call_id } => accept_call(&state, &user, &call_id).await,
            SignalMsg::Reject { call_id } => reject_call(&state, &user, &call_id).await,
            SignalMsg::Hangup { call_id } | SignalMsg::Leave { call_id } => {
                leave_call(&state, &user, &call_id).await
            }
            SignalMsg::JoinRoom { .. } => err_to(&state, &user.id, "channel voice rooms are not available yet"),
            SignalMsg::Sdp { .. } | SignalMsg::Ice { .. } => engine::on_client_signal(&state, &user, sig).await,
            SignalMsg::Mute { muted, .. } => {
                if let Some(cid) = state.calls.user_call(&user.id) {
                    let member = state.full_id(&user.username);
                    submit_to_manager(&state, &cid, SignalMsg::Mute { call_id: cid.clone(), member, muted });
                }
            }
            _ => {}
        }
    }

    pump.abort();
    state.calls.unregister_browser(&user.id, conn_id);
    if !state.calls.user_has_socket(&user.id) {
        if let Some(call_id) = state.calls.user_call(&user.id) {
            leave_call(&state, &user, &call_id).await;
        }
    }
}

/// Route a control frame into the call manager — directly if we are the manager,
/// otherwise over the peer link.
pub fn submit_to_manager(state: &AppState, call_id: &str, frame: SignalMsg) {
    if let Some(link) = state.calls.manager_link(call_id) {
        let _ = link.send(frame);
        return;
    }
    match frame {
        SignalMsg::Sdp { .. } | SignalMsg::Ice { .. } | SignalMsg::Mute { .. } => state.calls.manager.route(frame),
        SignalMsg::Hangup { call_id } => state.calls.manager.end(&call_id, "hangup"),
        SignalMsg::Leave { call_id } => {
            let dom = state.calls.domain.clone();
            state.calls.manager.leave(&call_id, &dom);
        }
        _ => {}
    }
}

/// Attach this server (and `user`) to a call — as a fresh participant if this is
/// the first local member, otherwise adding a member to the existing entry.
fn ensure_local_participant(state: &AppState, call_id: &str, user: &User) {
    let me_full = state.full_id(&user.username);
    let first = !state.calls.manager.has_participant(call_id, &state.calls.domain);

    state.calls.add_local(call_id, &user.id);
    state.calls.set_user_call(&user.id, call_id);

    if !first {
        state.calls.manager.add_member(call_id, &state.calls.domain, &me_full);
        return;
    }

    let (mtx, mut mrx) = mpsc::unbounded_channel::<SignalMsg>();
    let _ = state.calls.manager.join(
        call_id,
        ParticipantServer {
            domain: state.calls.domain.clone(),
            members: vec![me_full],
            muted: vec![],
            sink: mtx,
        },
    );

    let st = state.clone();
    let cid = call_id.to_string();
    tokio::spawn(async move {
        while let Some(msg) = mrx.recv().await {
            if handle_manager_frame(&st, &cid, msg).await {
                break;
            }
        }
    });
}

/// Process one frame coming from the call manager toward this server. Returns
/// `true` when the call has ended and the pump should stop.
async fn handle_manager_frame(st: &AppState, cid: &str, msg: SignalMsg) -> bool {
    match &msg {
        SignalMsg::Sdp { .. } | SignalMsg::Ice { .. } => {
            engine::on_mesh_signal(st, cid, msg).await;
        }
        SignalMsg::Roster { participants, .. } => {
            let servers: Vec<String> = participants.iter().map(|p| p.server.clone()).collect();
            st.calls.notify_call_locals(cid, &msg);
            engine::on_roster(st, cid, &servers).await;
        }
        SignalMsg::CallEnded { .. } => {
            st.calls.notify_call_locals(cid, &msg);
            st.calls.drain_locals(cid);
            st.calls.drop_manager_link(cid);
            engine::teardown(st, cid).await;
            return true;
        }
        _ => st.calls.notify_call_locals(cid, &msg),
    }
    false
}

async fn start_direct_call(state: &AppState, user: &User, to: &str) {
    if !to.contains('@') {
        return err_to(state, &user.id, "invalid callee");
    }
    if state.calls.user_call(&user.id).is_some() {
        return err_to(state, &user.id, "already in a call");
    }
    match get_profile(state, to).await {
        Ok(Some(_)) => {}
        Ok(None) => return err_to(state, &user.id, "user not found"),
        Err(_) => return err_to(state, &user.id, "cannot reach that server"),
    }

    let to_domain = to.rsplit('@').next().unwrap_or_default().to_string();
    let (call_id, token) = state.calls.manager.open_direct(&to_domain);
    let me_full = state.full_id(&user.username);

    ensure_local_participant(state, &call_id, user);

    let manager_ws_url = format!("ws://{}/calls/manager/ws", state.domain());
    let meta = json!({ "call_id": call_id, "manager_ws_url": manager_ws_url, "token": token });

    let out = OutgoingMessage {
        sender: me_full,
        receiver: to.to_string(),
        target: to.to_string(),
        text: format!("{} started a call", user.username),
        kind: "call_invite".into(),
        metadata: Some(meta.to_string()),
    };
    if let Err(e) = store_and_deliver(state, out, Some(&user.username), Some(&user.id)).await {
        tracing::warn!(error = %e, "failed to send call invite");
        state.calls.manager.end(&call_id, "invite-failed");
        state.calls.remove_local(&call_id, &user.id);
        state.calls.clear_user_call(&user.id);
    }
}

async fn accept_call(state: &AppState, user: &User, call_id: &str) {
    let Some(inv) = state.calls.take_pending(call_id) else {
        return err_to(state, &user.id, "call is no longer available");
    };
    if inv.for_user_id != user.id {
        return;
    }

    // Same-server call: we already run the manager — just attach locally.
    if state.calls.manager.exists(&inv.call_id) {
        ensure_local_participant(state, &inv.call_id, user);
        return;
    }

    let me_full = state.full_id(&user.username);
    let url = format!(
        "{}?call_id={}&token={}&server={}",
        inv.manager_ws_url, inv.call_id, inv.token, state.domain()
    );
    let (stream, _) = match tokio_tungstenite::connect_async(url.as_str()).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, url, "could not reach call manager");
            return err_to(state, &user.id, "could not reach call manager");
        }
    };
    let (mut sink, mut src) = stream.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<SignalMsg>();

    let _ = out_tx.send(SignalMsg::Join {
        call_id: inv.call_id.clone(),
        server: state.domain().to_string(),
        members: vec![me_full],
    });

    tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            let txt = serde_json::to_string(&msg).unwrap_or_default();
            if sink.send(tokio_tungstenite::tungstenite::Message::Text(txt)).await.is_err() {
                break;
            }
        }
    });

    state.calls.set_manager_link(&inv.call_id, out_tx);
    state.calls.add_local(&inv.call_id, &user.id);
    state.calls.set_user_call(&user.id, &inv.call_id);

    let st = state.clone();
    let cid = inv.call_id.clone();
    tokio::spawn(async move {
        while let Some(Ok(m)) = src.next().await {
            let tokio_tungstenite::tungstenite::Message::Text(t) = m else { continue };
            let Ok(sig) = serde_json::from_str::<SignalMsg>(&t) else { continue };
            if handle_manager_frame(&st, &cid, sig).await {
                break;
            }
        }
        st.calls.drop_manager_link(&cid);
    });
}

async fn reject_call(state: &AppState, user: &User, call_id: &str) {
    let Some(inv) = state.calls.take_pending(call_id) else { return };
    let out = OutgoingMessage {
        sender: state.full_id(&user.username),
        receiver: inv.from.clone(),
        target: inv.from.clone(),
        text: "Call declined".into(),
        kind: "call_end".into(),
        metadata: Some(json!({ "call_id": call_id, "outcome": "rejected" }).to_string()),
    };
    let _ = store_and_deliver(state, out, None, None).await;
}

async fn leave_call(state: &AppState, user: &User, call_id: &str) {
    let remaining = state.calls.remove_local(call_id, &user.id);
    state.calls.clear_user_call(&user.id);

    if remaining > 0 {
        // other local members remain — just drop this member from the roster
        state.calls.manager.remove_member(call_id, &state.calls.domain, &state.full_id(&user.username));
        return;
    }

    let is_room = matches!(state.calls.manager.kind(call_id), Some(CallKind::ChannelRoom { .. }));
    let frame = if is_room {
        SignalMsg::Leave { call_id: call_id.to_string() }
    } else {
        SignalMsg::Hangup { call_id: call_id.to_string() }
    };
    submit_to_manager(state, call_id, frame);
    state.calls.drop_manager_link(call_id);
    engine::teardown(state, call_id).await;
}

// ════════════════════════════ manager side ════════════════════════════

#[derive(Deserialize)]
pub struct ManagerParams {
    call_id: String,
    token: String,
    server: String,
}

pub async fn manager_ws(
    State(state): State<AppState>,
    Query(p): Query<ManagerParams>,
    ws: WebSocketUpgrade,
) -> Response {
    if !state.calls.manager.verify_token(&p.call_id, &p.server, &p.token) {
        return (axum::http::StatusCode::UNAUTHORIZED, "bad call token").into_response();
    }
    ws.on_upgrade(move |socket| manager_peer_loop(socket, state, p))
}

async fn manager_peer_loop(socket: WebSocket, state: AppState, p: ManagerParams) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<SignalMsg>();

    let pump = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_tx.send(ws_json(&msg)).await.is_err() {
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
                let _ = state.calls.manager.join(
                    &p.call_id,
                    ParticipantServer { domain: p.server.clone(), members, muted: vec![], sink: tx.clone() },
                );
            }
            SignalMsg::Sdp { .. } | SignalMsg::Ice { .. } | SignalMsg::Mute { .. } => state.calls.manager.route(sig),
            SignalMsg::Hangup { .. } => {
                state.calls.manager.end(&p.call_id, "hangup");
                break;
            }
            SignalMsg::Leave { .. } => break,
            _ => {}
        }
    }

    pump.abort();
    state.calls.manager.leave(&p.call_id, &p.server);
}

// ══════════════════════ inbound call_* message hooks ══════════════════════

pub fn on_call_invite(state: &AppState, recipient_user_id: &str, from: &str, metadata_json: &str) {
    let Ok(m) = serde_json::from_str::<serde_json::Value>(metadata_json) else { return };
    let (Some(call_id), Some(url), Some(token)) = (
        m.get("call_id").and_then(|v| v.as_str()),
        m.get("manager_ws_url").and_then(|v| v.as_str()),
        m.get("token").and_then(|v| v.as_str()),
    ) else {
        return;
    };
    state.calls.add_pending(PendingInvite {
        call_id: call_id.to_string(),
        from: from.to_string(),
        manager_ws_url: url.to_string(),
        token: token.to_string(),
        for_user_id: recipient_user_id.to_string(),
    });
    state.calls.notify_user(
        recipient_user_id,
        &SignalMsg::IncomingCall {
            call_id: call_id.to_string(),
            from: from.to_string(),
            manager_ws_url: url.to_string(),
            token: token.to_string(),
        },
    );
}

pub fn on_call_end(state: &AppState, recipient_user_id: &str, metadata_json: &str) {
    let Ok(m) = serde_json::from_str::<serde_json::Value>(metadata_json) else { return };
    let Some(call_id) = m.get("call_id").and_then(|v| v.as_str()) else { return };
    let outcome = m.get("outcome").and_then(|v| v.as_str()).unwrap_or("ended").to_string();

    state.calls.take_pending(call_id);
    if state.calls.manager.exists(call_id) {
        state.calls.manager.end(call_id, &outcome);
    }
    state.calls.notify_user(
        recipient_user_id,
        &SignalMsg::CallEnded { call_id: call_id.to_string(), reason: outcome },
    );
    state.calls.remove_local(call_id, recipient_user_id);
    state.calls.clear_user_call(recipient_user_id);
    state.calls.drop_manager_link(call_id);
}
