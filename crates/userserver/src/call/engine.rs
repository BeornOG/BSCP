//! Audio media relay — each user server is a tiny SFU.
//!
//! Per call this server holds:
//!   * one **client** `RTCPeerConnection` per local browser (browser ↔ this server)
//!   * one **mesh** `RTCPeerConnection` per remote participant server
//!
//! Inbound RTP from every peer is tagged with an *origin* key and published on a
//! per-call broadcast bus; each peer's outbound track re-writes every packet whose
//! origin is not its own. For a 1:1 call that is a clean two-node bridge; the same
//! rule extends to groups.

use crate::state::AppState;
use bscp_common::call::SignalMsg;
use bscp_common::models::User;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::broadcast;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::setting_engine::SettingEngine;
use webrtc::api::{APIBuilder, API};
use webrtc::ice::udp_network::{EphemeralUDP, UDPNetwork};
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_candidate_type::RTCIceCandidateType;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::{TrackLocal, TrackLocalWriter};

type RtpPacket = webrtc::rtp::packet::Packet;

// ── global webrtc API (media engine + ICE settings) ─────────────────────

static API: OnceLock<Arc<API>> = OnceLock::new();

fn api(state: &AppState) -> Arc<API> {
    API.get_or_init(|| {
        let mut se = SettingEngine::default();
        let (lo, hi) = (state.cfg.rtc_port_min, state.cfg.rtc_port_max);
        if lo > 0 && hi >= lo {
            if let Ok(eph) = EphemeralUDP::new(lo, hi) {
                se.set_udp_network(UDPNetwork::Ephemeral(eph));
            }
        }
        if let Some(ip) = &state.cfg.ice_public_ip {
            se.set_nat_1to1_ips(vec![ip.clone()], RTCIceCandidateType::Host);
        }
        let mut me = MediaEngine::default();
        me.register_default_codecs().expect("codecs");
        let mut reg = Registry::new();
        reg = register_default_interceptors(reg, &mut me).expect("interceptors");
        Arc::new(
            APIBuilder::new()
                .with_media_engine(me)
                .with_setting_engine(se)
                .with_interceptor_registry(reg)
                .build(),
        )
    })
    .clone()
}

// ── per-call state ─────────────────────────────────────────────────────

struct Peer {
    pc: Arc<RTCPeerConnection>,
    /// The origin key for audio this peer produces (`user@domain` or a server domain).
    #[allow(dead_code)]
    origin: String,
}

struct Session {
    call_id: String,
    my_domain: String,
    bus: broadcast::Sender<(String, RtpPacket)>,
    clients: Mutex<HashMap<String, Peer>>, // key: user_id
    mesh: Mutex<HashMap<String, Peer>>,    // key: peer server domain
}

static SESSIONS: OnceLock<Mutex<HashMap<String, Arc<Session>>>> = OnceLock::new();

fn sessions() -> &'static Mutex<HashMap<String, Arc<Session>>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn session(state: &AppState, call_id: &str) -> Arc<Session> {
    let mut map = sessions().lock().unwrap();
    map.entry(call_id.to_string())
        .or_insert_with(|| {
            let (bus, _) = broadcast::channel(1024);
            Arc::new(Session {
                call_id: call_id.to_string(),
                my_domain: state.calls.domain.clone(),
                bus,
                clients: Mutex::new(HashMap::new()),
                mesh: Mutex::new(HashMap::new()),
            })
        })
        .clone()
}

// ── helpers ───────────────────────────────────────────────────────────

async fn new_local_track(label: &str) -> Arc<TrackLocalStaticRTP> {
    Arc::new(TrackLocalStaticRTP::new(
        RTCRtpCodecCapability { mime_type: "audio/opus".to_string(), ..Default::default() },
        format!("audio-{label}"),
        format!("bscp-{label}"),
    ))
}

/// Attach a fresh peer connection: adds our outbound audio track (fed from the
/// bus, skipping `origin`), pipes inbound RTP onto the bus, and trickles ICE.
async fn build_peer(
    state: &AppState,
    sess: &Arc<Session>,
    origin: String,
    on_ice: impl Fn(String) + Send + Sync + 'static,
) -> anyhow::Result<Arc<RTCPeerConnection>> {
    let pc = Arc::new(api(state).new_peer_connection(RTCConfiguration::default()).await?);

    // outbound track (what this peer will hear)
    let out_track = new_local_track(&origin).await;
    let sender = pc.add_track(out_track.clone() as Arc<dyn TrackLocal + Send + Sync>).await?;
    tokio::spawn(async move {
        let mut buf = vec![0u8; 1500];
        while sender.read(&mut buf).await.is_ok() {}
    });

    // bus → outbound track
    {
        let mut rx = sess.bus.subscribe();
        let mine = origin.clone();
        tokio::spawn(async move {
            while let Ok((src, pkt)) = rx.recv().await {
                if src != mine {
                    let _ = out_track.write_rtp(&pkt).await;
                }
            }
        });
    }

    // inbound track → bus
    {
        let bus = sess.bus.clone();
        let src = origin.clone();
        pc.on_track(Box::new(move |track, _, _| {
            let bus = bus.clone();
            let src = src.clone();
            Box::pin(async move {
                tokio::spawn(async move {
                    while let Ok((pkt, _)) = track.read_rtp().await {
                        let _ = bus.send((src.clone(), pkt));
                    }
                });
            })
        }));
    }

    // local ICE → callback
    pc.on_ice_candidate(Box::new(move |c| {
        let on_ice = &on_ice;
        if let Some(c) = c {
            if let Ok(init) = c.to_json() {
                if let Ok(js) = serde_json::to_string(&init) {
                    on_ice(js);
                }
            }
        }
        Box::pin(async {})
    }));

    let cid = sess.call_id.clone();
    pc.on_peer_connection_state_change(Box::new(move |s| {
        tracing::debug!(call = %cid, state = ?s, "call PC state");
        Box::pin(async {})
    }));

    Ok(pc)
}

fn parse_ice(candidate: &str) -> Option<RTCIceCandidateInit> {
    serde_json::from_str(candidate).ok()
}

// ── browser (client PC) ───────────────────────────────────────────────

pub async fn on_client_signal(state: &AppState, user: &User, sig: SignalMsg) {
    let Some(call_id) = state.calls.user_call(&user.id) else { return };
    let sess = session(state, &call_id);

    match sig {
        SignalMsg::Sdp { sdp, answer, .. } if !answer => {
            let origin = state.full_id(&user.username);
            let (st, uid, cid) = (state.clone(), user.id.clone(), call_id.clone());
            let on_ice = move |js: String| {
                st.calls.notify_user(
                    &uid,
                    &SignalMsg::Ice { call_id: cid.clone(), from: st.calls.domain.clone(), to: "browser".into(), candidate: js },
                );
            };
            let pc = match build_peer(state, &sess, origin.clone(), on_ice).await {
                Ok(pc) => pc,
                Err(e) => return tracing::warn!(error = %e, "client PC build failed"),
            };
            if let Err(e) = negotiate_answer(&pc, &sdp).await {
                return tracing::warn!(error = %e, "client PC answer failed");
            }
            let ldesc = pc.local_description().await.map(|d| d.sdp).unwrap_or_default();
            sess.clients.lock().unwrap().insert(user.id.clone(), Peer { pc, origin });
            state.calls.notify_user(
                &user.id,
                &SignalMsg::Sdp {
                    call_id,
                    from: state.calls.domain.clone(),
                    to: "browser".into(),
                    sdp: ldesc,
                    answer: true,
                },
            );
        }
        SignalMsg::Ice { candidate, .. } => {
            let pc = sess.clients.lock().unwrap().get(&user.id).map(|p| p.pc.clone());
            if let (Some(init), Some(pc)) = (parse_ice(&candidate), pc) {
                let _ = pc.add_ice_candidate(init).await;
            }
        }
        _ => {}
    }
}

async fn negotiate_answer(pc: &Arc<RTCPeerConnection>, offer_sdp: &str) -> anyhow::Result<()> {
    pc.set_remote_description(RTCSessionDescription::offer(offer_sdp.to_string())?).await?;
    let answer = pc.create_answer(None).await?;
    pc.set_local_description(answer).await?;
    Ok(())
}

// ── mesh (server ↔ server PC) ─────────────────────────────────────────

/// Called when the roster changes: the lexicographically-smaller domain dials.
pub async fn on_roster(state: &AppState, call_id: &str, servers: &[String]) {
    let sess = session(state, call_id);
    let me = state.calls.domain.clone();
    for peer in servers {
        if peer.as_str() <= me.as_str() {
            continue;
        }
        let already = sess.mesh.lock().unwrap().contains_key(peer);
        if already {
            continue;
        }
        let (st, cid, to) = (state.clone(), call_id.to_string(), peer.clone());
        let on_ice = move |js: String| {
            crate::call::ws::submit_to_manager(
                &st,
                &cid,
                SignalMsg::Ice { call_id: cid.clone(), from: st.calls.domain.clone(), to: to.clone(), candidate: js },
            );
        };
        let pc = match build_peer(state, &sess, peer.clone(), on_ice).await {
            Ok(pc) => pc,
            Err(e) => {
                tracing::warn!(error = %e, "mesh PC build failed");
                continue;
            }
        };
        let offer = match pc.create_offer(None).await {
            Ok(o) => o,
            Err(e) => {
                tracing::warn!(error = %e, "mesh offer failed");
                continue;
            }
        };
        let _ = pc.set_local_description(offer.clone()).await;
        sess.mesh.lock().unwrap().insert(peer.clone(), Peer { pc, origin: peer.clone() });
        crate::call::ws::submit_to_manager(
            state,
            call_id,
            SignalMsg::Sdp { call_id: call_id.to_string(), from: me.clone(), to: peer.clone(), sdp: offer.sdp, answer: false },
        );
    }
}

pub async fn on_mesh_signal(state: &AppState, call_id: &str, sig: SignalMsg) {
    let sess = session(state, call_id);
    match sig {
        SignalMsg::Sdp { from, sdp, answer, .. } => {
            if answer {
                let pc = sess.mesh.lock().unwrap().get(&from).map(|p| p.pc.clone());
                if let (Some(pc), Ok(desc)) = (pc, RTCSessionDescription::answer(sdp)) {
                    let _ = pc.set_remote_description(desc).await;
                }
                return;
            }
            // inbound offer → build + answer
            let (st, cid, to) = (state.clone(), call_id.to_string(), from.clone());
            let on_ice = move |js: String| {
                crate::call::ws::submit_to_manager(
                    &st,
                    &cid,
                    SignalMsg::Ice { call_id: cid.clone(), from: st.calls.domain.clone(), to: to.clone(), candidate: js },
                );
            };
            let pc = match build_peer(state, &sess, from.clone(), on_ice).await {
                Ok(pc) => pc,
                Err(e) => return tracing::warn!(error = %e, "mesh answer PC failed"),
            };
            if let Err(e) = negotiate_answer(&pc, &sdp).await {
                return tracing::warn!(error = %e, "mesh answer failed");
            }
            let ldesc = pc.local_description().await.map(|d| d.sdp).unwrap_or_default();
            sess.mesh.lock().unwrap().insert(from.clone(), Peer { pc, origin: from.clone() });
            crate::call::ws::submit_to_manager(
                state,
                call_id,
                SignalMsg::Sdp {
                    call_id: call_id.to_string(),
                    from: state.calls.domain.clone(),
                    to: from,
                    sdp: ldesc,
                    answer: true,
                },
            );
        }
        SignalMsg::Ice { from, candidate, .. } => {
            let pc = sess.mesh.lock().unwrap().get(&from).map(|p| p.pc.clone());
            if let (Some(init), Some(pc)) = (parse_ice(&candidate), pc) {
                let _ = pc.add_ice_candidate(init).await;
            }
        }
        _ => {}
    }
}

// ── teardown ──────────────────────────────────────────────────────────

pub async fn teardown(_state: &AppState, call_id: &str) {
    let Some(sess) = sessions().lock().unwrap().remove(call_id) else { return };
    let clients: Vec<_> = sess.clients.lock().unwrap().drain().map(|(_, p)| p.pc).collect();
    let mesh: Vec<_> = sess.mesh.lock().unwrap().drain().map(|(_, p)| p.pc).collect();
    for pc in clients.into_iter().chain(mesh) {
        let _ = pc.close().await;
    }
    tracing::debug!(call = %sess.call_id, domain = %sess.my_domain, "call media torn down");
}
