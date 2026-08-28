//! Media relay for calls — each user server is a tiny audio SFU.
//!
//! v1 stub: signaling is wired end-to-end but no RTP is forwarded yet. The
//! `on_*` hooks are where `webrtc` peer connections will be driven.

use crate::state::AppState;
use bscp_common::call::SignalMsg;
use bscp_common::models::User;

/// SDP/ICE from a local browser for its client PC (browser ↔ this server).
pub async fn on_client_signal(_state: &AppState, _user: &User, _sig: SignalMsg) {
    // TODO(v1-media): feed into the per-user client RTCPeerConnection.
}

/// SDP/ICE for a mesh PC (this server ↔ a peer participant server).
pub async fn on_mesh_signal(_state: &AppState, _call_id: &str, _sig: SignalMsg) {
    // TODO(v1-media): feed into the per-peer mesh RTCPeerConnection.
}

/// Release any media resources held for `call_id` on this server.
pub async fn teardown(_state: &AppState, _call_id: &str) {
    // TODO(v1-media): close client + mesh peer connections for this call.
}
