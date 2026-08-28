//! In-memory registry of active calls + signaling relay.

use super::signal::{RosterEntry, SignalMsg};
use super::{CallId, CallKind, ParticipantId, ServerId};
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::mpsc;

#[derive(Debug, thiserror::Error)]
pub enum CallError {
    #[error("call not found")]
    NotFound,
    #[error("invalid token")]
    BadToken,
}

/// One participant server attached to a call. `sink` delivers frames toward that
/// server (an in-process channel for the manager's own server, or a WebSocket
/// forwarder for a remote server).
pub struct ParticipantServer {
    pub domain: ServerId,
    pub members: Vec<ParticipantId>,
    pub muted: Vec<ParticipantId>,
    pub sink: mpsc::UnboundedSender<SignalMsg>,
}

pub struct Call {
    pub id: CallId,
    pub kind: CallKind,
    pub manager_domain: ServerId,
    pub created_at: f64,
    participants: Vec<ParticipantServer>,
    /// One-time tokens minted per peer domain for the manager-WS handshake.
    tokens: HashMap<ServerId, String>,
}

impl Call {
    fn roster(&self) -> Vec<RosterEntry> {
        self.participants
            .iter()
            .map(|p| RosterEntry {
                server: p.domain.clone(),
                members: p.members.clone(),
                muted: p.muted.clone(),
            })
            .collect()
    }

    fn broadcast(&self, msg: &SignalMsg) {
        for p in &self.participants {
            let _ = p.sink.send(msg.clone());
        }
    }
}

pub struct CallManager {
    manager_domain: ServerId,
    calls: Mutex<HashMap<CallId, Call>>,
    /// `channel_path` → stable `CallId` for persistent rooms.
    rooms: Mutex<HashMap<String, CallId>>,
}

impl CallManager {
    pub fn new(manager_domain: impl Into<String>) -> Self {
        Self {
            manager_domain: manager_domain.into(),
            calls: Mutex::new(HashMap::new()),
            rooms: Mutex::new(HashMap::new()),
        }
    }

    pub fn manager_domain(&self) -> &str {
        &self.manager_domain
    }

    /// Create an ephemeral `Direct` call. Returns its id and the handshake token
    /// the invited peer domain must present on the manager WS.
    pub fn open_direct(&self, invited_domain: &str) -> (CallId, String) {
        let id = crate::uuid();
        let token = crate::random_token(24);
        let mut call = Call {
            id: id.clone(),
            kind: CallKind::Direct,
            manager_domain: self.manager_domain.clone(),
            created_at: crate::now_ts(),
            participants: Vec::new(),
            tokens: HashMap::new(),
        };
        call.tokens.insert(invited_domain.to_string(), token.clone());
        self.calls.lock().unwrap().insert(id.clone(), call);
        (id, token)
    }

    /// Mint (or return) a token for another domain to join an existing call.
    pub fn mint_token(&self, call_id: &str, domain: &str) -> Option<String> {
        let mut calls = self.calls.lock().unwrap();
        let call = calls.get_mut(call_id)?;
        Some(call.tokens.entry(domain.to_string()).or_insert_with(|| crate::random_token(24)).clone())
    }

    /// Get-or-create the persistent room for `channel_path`.
    pub fn room(&self, channel_path: &str) -> CallId {
        let mut rooms = self.rooms.lock().unwrap();
        if let Some(id) = rooms.get(channel_path) {
            return id.clone();
        }
        let id = crate::uuid();
        rooms.insert(channel_path.to_string(), id.clone());
        self.calls.lock().unwrap().insert(
            id.clone(),
            Call {
                id: id.clone(),
                kind: CallKind::ChannelRoom { channel_path: channel_path.to_string() },
                manager_domain: self.manager_domain.clone(),
                created_at: crate::now_ts(),
                participants: Vec::new(),
                tokens: HashMap::new(),
            },
        );
        id
    }

    pub fn verify_token(&self, call_id: &str, domain: &str, token: &str) -> bool {
        self.calls
            .lock()
            .unwrap()
            .get(call_id)
            .and_then(|c| c.tokens.get(domain))
            .map(|t| t == token)
            .unwrap_or(false)
    }

    pub fn kind(&self, call_id: &str) -> Option<CallKind> {
        self.calls.lock().unwrap().get(call_id).map(|c| c.kind.clone())
    }

    pub fn exists(&self, call_id: &str) -> bool {
        self.calls.lock().unwrap().contains_key(call_id)
    }

    /// Attach a participant server. Sends it the current `Roster`, and tells the
    /// others `ParticipantJoined`.
    pub fn join(&self, call_id: &str, ps: ParticipantServer) -> Result<(), CallError> {
        let mut calls = self.calls.lock().unwrap();
        let call = calls.get_mut(call_id).ok_or(CallError::NotFound)?;

        call.participants.retain(|p| p.domain != ps.domain);
        call.broadcast(&SignalMsg::ParticipantJoined {
            call_id: call_id.to_string(),
            server: ps.domain.clone(),
            members: ps.members.clone(),
        });

        call.participants.push(ps);
        let roster = SignalMsg::Roster { call_id: call_id.to_string(), participants: call.roster() };
        call.broadcast(&roster);
        Ok(())
    }

    /// Add a member to an already-attached participant server and re-broadcast
    /// the roster (used when a second local user joins the same server's call).
    pub fn add_member(&self, call_id: &str, domain: &str, member: &str) {
        let mut calls = self.calls.lock().unwrap();
        let Some(call) = calls.get_mut(call_id) else { return };
        if let Some(p) = call.participants.iter_mut().find(|p| p.domain == domain) {
            if !p.members.iter().any(|m| m == member) {
                p.members.push(member.to_string());
            }
        }
        call.broadcast(&SignalMsg::ParticipantJoined {
            call_id: call_id.to_string(),
            server: domain.to_string(),
            members: vec![member.to_string()],
        });
        call.broadcast(&SignalMsg::Roster { call_id: call_id.to_string(), participants: call.roster() });
    }

    /// Remove one member from a participant server. Returns the number of members
    /// still on that server (so the caller can `leave` when it hits zero).
    pub fn remove_member(&self, call_id: &str, domain: &str, member: &str) -> usize {
        let mut calls = self.calls.lock().unwrap();
        let Some(call) = calls.get_mut(call_id) else { return 0 };
        let Some(p) = call.participants.iter_mut().find(|p| p.domain == domain) else { return 0 };
        p.members.retain(|m| m != member);
        p.muted.retain(|m| m != member);
        let left = p.members.len();
        call.broadcast(&SignalMsg::Roster { call_id: call_id.to_string(), participants: call.roster() });
        left
    }

    /// True when this server already has a participant entry for the call.
    pub fn has_participant(&self, call_id: &str, domain: &str) -> bool {
        self.calls
            .lock()
            .unwrap()
            .get(call_id)
            .map(|c| c.participants.iter().any(|p| p.domain == domain))
            .unwrap_or(false)
    }

    /// Detach a participant server. Ephemeral calls with no participants left are
    /// dropped; persistent rooms are kept.
    pub fn leave(&self, call_id: &str, domain: &str) {
        let mut calls = self.calls.lock().unwrap();
        let Some(call) = calls.get_mut(call_id) else { return };

        let before = call.participants.len();
        call.participants.retain(|p| p.domain != domain);
        if call.participants.len() == before {
            return;
        }

        call.broadcast(&SignalMsg::ParticipantLeft { call_id: call_id.to_string(), server: domain.to_string() });
        call.broadcast(&SignalMsg::Roster { call_id: call_id.to_string(), participants: call.roster() });

        if call.participants.is_empty() && !call.kind.is_persistent() {
            calls.remove(call_id);
        }
    }

    /// End a call for everyone.
    pub fn end(&self, call_id: &str, reason: &str) {
        let mut calls = self.calls.lock().unwrap();
        let Some(call) = calls.get(call_id) else { return };
        call.broadcast(&SignalMsg::CallEnded { call_id: call_id.to_string(), reason: reason.to_string() });
        if let CallKind::ChannelRoom { channel_path } = &call.kind {
            self.rooms.lock().unwrap().remove(channel_path);
        }
        calls.remove(call_id);
    }

    /// Forward a point-to-point frame (`Sdp` / `Ice`) to its target server.
    pub fn route(&self, frame: SignalMsg) {
        let (Some(call_id), Some(target)) =
            (frame.call_id().map(str::to_string), frame.route_target().map(str::to_string))
        else {
            return;
        };
        let calls = self.calls.lock().unwrap();
        if let Some(call) = calls.get(&call_id) {
            if let Some(p) = call.participants.iter().find(|p| p.domain == target) {
                let _ = p.sink.send(frame);
            }
        }
    }

    pub fn set_muted(&self, call_id: &str, domain: &str, member: &str, muted: bool) {
        let mut calls = self.calls.lock().unwrap();
        let Some(call) = calls.get_mut(call_id) else { return };
        if let Some(p) = call.participants.iter_mut().find(|p| p.domain == domain) {
            p.muted.retain(|m| m != member);
            if muted {
                p.muted.push(member.to_string());
            }
        }
        call.broadcast(&SignalMsg::Mute {
            call_id: call_id.to_string(),
            member: member.to_string(),
            muted,
        });
    }

    pub fn roster(&self, call_id: &str) -> Option<Vec<RosterEntry>> {
        self.calls.lock().unwrap().get(call_id).map(|c| c.roster())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn participant(domain: &str) -> (ParticipantServer, mpsc::UnboundedReceiver<SignalMsg>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            ParticipantServer {
                domain: domain.to_string(),
                members: vec![format!("u@{domain}")],
                muted: vec![],
                sink: tx,
            },
            rx,
        )
    }

    #[test]
    fn direct_call_tears_down_when_empty() {
        let mgr = CallManager::new("a.example");
        let (id, token) = mgr.open_direct("b.example");
        assert!(mgr.verify_token(&id, "b.example", &token));

        let (pa, _ra) = participant("a.example");
        let (pb, _rb) = participant("b.example");
        mgr.join(&id, pa).unwrap();
        mgr.join(&id, pb).unwrap();
        assert_eq!(mgr.roster(&id).unwrap().len(), 2);

        mgr.leave(&id, "a.example");
        assert!(mgr.exists(&id));
        mgr.leave(&id, "b.example");
        assert!(!mgr.exists(&id), "empty Direct call should be removed");
    }

    #[test]
    fn channel_room_persists_when_empty() {
        let mgr = CallManager::new("chan.example");
        let id = mgr.room("chan.example#general");
        assert_eq!(mgr.room("chan.example#general"), id, "same path → same room id");

        let (p, _r) = participant("x.example");
        mgr.join(&id, p).unwrap();
        mgr.leave(&id, "x.example");
        assert!(mgr.exists(&id), "empty ChannelRoom should persist");
    }

    #[test]
    fn join_notifies_existing_and_new() {
        let mgr = CallManager::new("a.example");
        let (id, _) = mgr.open_direct("b.example");

        let (pa, mut ra) = participant("a.example");
        mgr.join(&id, pa).unwrap();
        // a: its own Roster
        assert!(matches!(ra.try_recv(), Ok(SignalMsg::Roster { .. })));

        let (pb, mut rb) = participant("b.example");
        mgr.join(&id, pb).unwrap();
        // a hears b joined, then a fresh roster
        assert!(matches!(ra.try_recv(), Ok(SignalMsg::ParticipantJoined { server, .. }) if server == "b.example"));
        assert!(matches!(ra.try_recv(), Ok(SignalMsg::Roster { .. })));
        // b gets a roster of size 2
        match rb.try_recv() {
            Ok(SignalMsg::Roster { participants, .. }) => assert_eq!(participants.len(), 2),
            other => panic!("expected roster, got {other:?}"),
        }
    }

    #[test]
    fn routes_sdp_to_target_only() {
        let mgr = CallManager::new("a.example");
        let (id, _) = mgr.open_direct("b.example");
        let (pa, mut ra) = participant("a.example");
        let (pb, mut rb) = participant("b.example");
        mgr.join(&id, pa).unwrap();
        mgr.join(&id, pb).unwrap();
        while ra.try_recv().is_ok() {}
        while rb.try_recv().is_ok() {}

        mgr.route(SignalMsg::Sdp {
            call_id: id.clone(),
            from: "a.example".into(),
            to: "b.example".into(),
            sdp: "v=0".into(),
            answer: false,
        });
        assert!(matches!(rb.try_recv(), Ok(SignalMsg::Sdp { .. })));
        assert!(ra.try_recv().is_err(), "sender must not receive its own routed frame");
    }
}
