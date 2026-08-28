//! Transport-agnostic call coordination shared by the user server (manager for
//! DM / group-DM calls) and the channel server (manager for channel rooms).
//!
//! The manager owns call state + the participant roster and relays signaling
//! (SDP / ICE / control) between participant **servers**. It never touches media.

pub mod manager;
pub mod signal;

pub use manager::{Call, CallError, CallManager, ParticipantServer};
pub use signal::{RosterEntry, SignalMsg};

/// `<uuid>` — unique per call.
pub type CallId = String;
/// `user@domain`
pub type ParticipantId = String;
/// `domain` (a user server or channel server host).
pub type ServerId = String;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum CallKind {
    /// Ephemeral 1:1 / group-DM call. Torn down when the roster empties.
    Direct,
    /// Persistent Discord-style voice room keyed by a channel path. Survives an
    /// empty roster.
    ChannelRoom { channel_path: String },
}

impl CallKind {
    pub fn is_persistent(&self) -> bool {
        matches!(self, CallKind::ChannelRoom { .. })
    }
}
