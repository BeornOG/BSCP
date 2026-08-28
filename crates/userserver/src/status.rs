//! Presence/status derivation — port of `routes/__init__.py::get_user_status`.

use bscp_common::models::{User, UserSession};
use bscp_common::now_ts;

const ONLINE: f64 = 5.0 * 60.0;
const AWAY: f64 = 60.0 * 60.0;
const INACTIVE_SESSION: f64 = 6.0 * 60.0 * 60.0;

pub fn status_from_type(status_type: i64) -> &'static str {
    match status_type {
        0 => "online",
        2 => "away",
        3 => "dnd",
        _ => "offline",
    }
}

pub fn user_status(user: &User, sessions: &[UserSession]) -> &'static str {
    let now = now_ts();
    let active: Vec<&UserSession> = sessions
        .iter()
        .filter(|s| s.expires_at > now && (now - s.last_active) <= INACTIVE_SESSION)
        .collect();

    if active.is_empty() {
        return "offline";
    }
    if user.status_type == 2 || user.status_type == 3 {
        return status_from_type(user.status_type);
    }
    if user.status_type == 1 {
        return "offline";
    }

    let last_active = active.iter().map(|s| s.last_active).fold(f64::MIN, f64::max);
    let idle = now - last_active;
    if idle <= ONLINE {
        "online"
    } else if idle <= AWAY {
        "away"
    } else {
        "offline"
    }
}
