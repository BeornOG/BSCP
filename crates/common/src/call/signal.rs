//! Wire frames for call signaling. One enum is reused on every hop:
//! browser ↔ its user server, and user server ↔ call manager.

use super::{CallId, ParticipantId, ServerId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RosterEntry {
    pub server: ServerId,
    pub members: Vec<ParticipantId>,
    #[serde(default)]
    pub muted: Vec<ParticipantId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalMsg {
    // ── browser → its own user server ─────────────────────────────────────
    /// Start a DM call with `to` (`user@domain`). The user server becomes the manager.
    StartCall { to: ParticipantId },
    /// Accept / reject a `call_invite` the browser was shown.
    Accept { call_id: CallId },
    Reject { call_id: CallId },
    /// Join / leave a persistent channel voice room.
    JoinRoom { channel_path: String },
    Leave { call_id: CallId },
    /// End the call for everyone (initiator / any participant).
    Hangup { call_id: CallId },

    // ── user server ↔ manager ────────────────────────────────────────────
    /// A participant server attaches to the manager.
    Join {
        call_id: CallId,
        server: ServerId,
        members: Vec<ParticipantId>,
    },
    /// SDP offer (`answer=false`) or answer (`answer=true`) for the mesh PC
    /// between two participant servers, or the client PC between browser & server.
    Sdp {
        call_id: CallId,
        from: ServerId,
        to: ServerId,
        sdp: String,
        answer: bool,
    },
    /// Trickled ICE candidate for the same PC.
    Ice {
        call_id: CallId,
        from: ServerId,
        to: ServerId,
        candidate: String,
    },
    /// Per-member mute state change.
    Mute {
        call_id: CallId,
        member: ParticipantId,
        muted: bool,
    },

    // ── manager / server → browser & peer servers ────────────────────────
    IncomingCall {
        call_id: CallId,
        from: ParticipantId,
        manager_ws_url: String,
        token: String,
    },
    Roster {
        call_id: CallId,
        participants: Vec<RosterEntry>,
    },
    ParticipantJoined {
        call_id: CallId,
        server: ServerId,
        members: Vec<ParticipantId>,
    },
    ParticipantLeft {
        call_id: CallId,
        server: ServerId,
    },
    CallEnded {
        call_id: CallId,
        reason: String,
    },
    Error {
        message: String,
    },
}

impl SignalMsg {
    /// `call_id` for frames that carry one.
    pub fn call_id(&self) -> Option<&str> {
        match self {
            SignalMsg::Accept { call_id }
            | SignalMsg::Reject { call_id }
            | SignalMsg::Leave { call_id }
            | SignalMsg::Hangup { call_id }
            | SignalMsg::Join { call_id, .. }
            | SignalMsg::Sdp { call_id, .. }
            | SignalMsg::Ice { call_id, .. }
            | SignalMsg::Mute { call_id, .. }
            | SignalMsg::IncomingCall { call_id, .. }
            | SignalMsg::Roster { call_id, .. }
            | SignalMsg::ParticipantJoined { call_id, .. }
            | SignalMsg::ParticipantLeft { call_id, .. }
            | SignalMsg::CallEnded { call_id, .. } => Some(call_id),
            _ => None,
        }
    }

    /// Destination participant server for point-to-point frames (`Sdp` / `Ice`).
    pub fn route_target(&self) -> Option<&str> {
        match self {
            SignalMsg::Sdp { to, .. } | SignalMsg::Ice { to, .. } => Some(to),
            _ => None,
        }
    }
}
