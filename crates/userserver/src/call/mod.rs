//! Call subsystem for the user server: browser signaling WS, peer/manager WS,
//! and the in-memory state tying them together. (Media relay lives in `engine`.)

pub mod engine;
pub mod ws;

use bscp_common::call::{CallId, CallManager, ParticipantId, SignalMsg};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tokio::sync::mpsc;

/// An unaccepted `call_invite` this server received for a local user.
#[derive(Clone)]
pub struct PendingInvite {
    pub call_id: CallId,
    pub from: ParticipantId,
    pub manager_ws_url: String,
    pub token: String,
    pub for_user_id: String,
}

type Sink = mpsc::UnboundedSender<SignalMsg>;

#[derive(Default)]
struct Browsers {
    /// `user_id` → open browser signaling sockets (one per tab), each with a conn id.
    tabs: HashMap<String, Vec<(u64, Sink)>>,
}

pub struct CallState {
    pub domain: String,
    /// This server acting as the manager for calls its users started.
    pub manager: CallManager,
    next_conn_id: AtomicU64,
    browsers: Mutex<Browsers>,
    /// `call_id` → pending invite awaiting the local user's accept/reject.
    pending: Mutex<HashMap<CallId, PendingInvite>>,
    /// `call_id` → sink toward the manager for calls this server participates in
    /// (in-process when we are the manager; the peer WS otherwise).
    to_manager: Mutex<HashMap<CallId, Sink>>,
    /// `user_id` → the call the user is currently in.
    user_call: Mutex<HashMap<String, CallId>>,
    /// `call_id` → local `user_id`s currently in the call (this server's members).
    call_locals: Mutex<HashMap<CallId, Vec<String>>>,
}

impl CallState {
    pub fn new(domain: impl Into<String>) -> Self {
        let domain = domain.into();
        Self {
            manager: CallManager::new(domain.clone()),
            domain,
            next_conn_id: AtomicU64::new(1),
            browsers: Mutex::new(Browsers::default()),
            pending: Mutex::new(HashMap::new()),
            to_manager: Mutex::new(HashMap::new()),
            user_call: Mutex::new(HashMap::new()),
            call_locals: Mutex::new(HashMap::new()),
        }
    }

    // ── browser registry ────────────────────────────────────────────────

    pub fn register_browser(&self, user_id: &str, sink: Sink) -> u64 {
        let id = self.next_conn_id.fetch_add(1, Ordering::Relaxed);
        self.browsers.lock().unwrap().tabs.entry(user_id.to_string()).or_default().push((id, sink));
        id
    }

    pub fn unregister_browser(&self, user_id: &str, conn_id: u64) {
        let mut b = self.browsers.lock().unwrap();
        if let Some(v) = b.tabs.get_mut(user_id) {
            v.retain(|(id, _)| *id != conn_id);
            if v.is_empty() {
                b.tabs.remove(user_id);
            }
        }
    }

    /// Fan a frame out to every open tab of `user_id`. Returns how many got it.
    pub fn notify_user(&self, user_id: &str, msg: &SignalMsg) -> usize {
        let b = self.browsers.lock().unwrap();
        let Some(tabs) = b.tabs.get(user_id) else { return 0 };
        let mut n = 0;
        for (_, sink) in tabs {
            if sink.send(msg.clone()).is_ok() {
                n += 1;
            }
        }
        n
    }

    pub fn user_has_socket(&self, user_id: &str) -> bool {
        self.browsers.lock().unwrap().tabs.contains_key(user_id)
    }

    // ── pending invites ─────────────────────────────────────────────────

    pub fn add_pending(&self, inv: PendingInvite) {
        self.pending.lock().unwrap().insert(inv.call_id.clone(), inv);
    }
    pub fn take_pending(&self, call_id: &str) -> Option<PendingInvite> {
        self.pending.lock().unwrap().remove(call_id)
    }
    pub fn peek_pending(&self, call_id: &str) -> Option<PendingInvite> {
        self.pending.lock().unwrap().get(call_id).cloned()
    }

    // ── manager link + membership ───────────────────────────────────────

    pub fn set_manager_link(&self, call_id: &str, sink: Sink) {
        self.to_manager.lock().unwrap().insert(call_id.to_string(), sink);
    }
    pub fn manager_link(&self, call_id: &str) -> Option<Sink> {
        self.to_manager.lock().unwrap().get(call_id).cloned()
    }
    pub fn drop_manager_link(&self, call_id: &str) {
        self.to_manager.lock().unwrap().remove(call_id);
    }

    pub fn set_user_call(&self, user_id: &str, call_id: &str) {
        self.user_call.lock().unwrap().insert(user_id.to_string(), call_id.to_string());
    }
    pub fn clear_user_call(&self, user_id: &str) -> Option<CallId> {
        self.user_call.lock().unwrap().remove(user_id)
    }
    pub fn user_call(&self, user_id: &str) -> Option<CallId> {
        self.user_call.lock().unwrap().get(user_id).cloned()
    }

    // ── local members of a call ─────────────────────────────────────────

    pub fn add_local(&self, call_id: &str, user_id: &str) {
        let mut m = self.call_locals.lock().unwrap();
        let v = m.entry(call_id.to_string()).or_default();
        if !v.iter().any(|u| u == user_id) {
            v.push(user_id.to_string());
        }
    }
    /// Remove a local member; returns how many local members remain in the call.
    pub fn remove_local(&self, call_id: &str, user_id: &str) -> usize {
        let mut m = self.call_locals.lock().unwrap();
        let Some(v) = m.get_mut(call_id) else { return 0 };
        v.retain(|u| u != user_id);
        let n = v.len();
        if n == 0 {
            m.remove(call_id);
        }
        n
    }
    /// Fan a frame to every local member of the call.
    pub fn notify_call_locals(&self, call_id: &str, msg: &SignalMsg) {
        let uids = self.call_locals.lock().unwrap().get(call_id).cloned().unwrap_or_default();
        for uid in uids {
            self.notify_user(&uid, msg);
        }
    }
    pub fn call_has_locals(&self, call_id: &str) -> bool {
        self.call_locals.lock().unwrap().get(call_id).map(|v| !v.is_empty()).unwrap_or(false)
    }
    /// Remove and return every local member of a call, clearing their `user_call`
    /// pointers. Used when a call ends for everyone.
    pub fn drain_locals(&self, call_id: &str) -> Vec<String> {
        let uids = self.call_locals.lock().unwrap().remove(call_id).unwrap_or_default();
        let mut uc = self.user_call.lock().unwrap();
        for uid in &uids {
            if uc.get(uid).map(|c| c == call_id).unwrap_or(false) {
                uc.remove(uid);
            }
        }
        uids
    }
}
