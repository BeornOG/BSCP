//! Server-level moderation helpers. The federated-domain blocklist (issue #8)
//! lives in `bscp_common::moderation` since the channel server shares it.

pub use bscp_common::moderation::{is_domain_blocked, normalize_domain};
