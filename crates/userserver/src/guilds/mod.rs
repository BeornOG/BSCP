//! Guild / channel-server integration on the user server side: mint federation
//! assertions for local users, and (phase 3) a constrained gateway that proxies
//! guild traffic to channel servers on the user's behalf.

pub mod assert;
