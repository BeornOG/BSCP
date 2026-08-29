//! Proves the webrtc-rs audio-forwarding primitive the call engine relies on:
//! a sender PC's RTP reaches a receiver PC's `on_track` after a normal
//! offer/answer, over real ICE/DTLS on loopback.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::{TrackLocal, TrackLocalWriter};

async fn new_pc() -> Arc<webrtc::peer_connection::RTCPeerConnection> {
    let mut m = MediaEngine::default();
    m.register_default_codecs().unwrap();
    let api = APIBuilder::new().with_media_engine(m).build();
    Arc::new(api.new_peer_connection(RTCConfiguration::default()).await.unwrap())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rtp_is_forwarded_between_peers() {
    let sender = new_pc().await;
    let receiver = new_pc().await;

    let track = Arc::new(TrackLocalStaticRTP::new(
        RTCRtpCodecCapability { mime_type: "audio/opus".into(), ..Default::default() },
        "audio".into(),
        "bscp".into(),
    ));
    let rtp_sender = sender.add_track(track.clone() as Arc<dyn TrackLocal + Send + Sync>).await.unwrap();
    tokio::spawn(async move {
        let mut b = vec![0u8; 1500];
        while rtp_sender.read(&mut b).await.is_ok() {}
    });

    let received = Arc::new(AtomicU64::new(0));
    let r2 = received.clone();
    receiver.on_track(Box::new(move |t, _, _| {
        let r = r2.clone();
        Box::pin(async move {
            tokio::spawn(async move {
                while let Ok((_pkt, _)) = t.read_rtp().await {
                    r.fetch_add(1, Ordering::Relaxed);
                }
            });
        })
    }));

    // trickle ICE both ways
    {
        let rx = receiver.clone();
        sender.on_ice_candidate(Box::new(move |c| {
            let rx = rx.clone();
            Box::pin(async move {
                if let Some(c) = c {
                    let _ = rx.add_ice_candidate(c.to_json().unwrap()).await;
                }
            })
        }));
        let tx = sender.clone();
        receiver.on_ice_candidate(Box::new(move |c| {
            let tx = tx.clone();
            Box::pin(async move {
                if let Some(c) = c {
                    let _ = tx.add_ice_candidate(c.to_json().unwrap()).await;
                }
            })
        }));
    }

    let offer = sender.create_offer(None).await.unwrap();
    sender.set_local_description(offer.clone()).await.unwrap();
    receiver.set_remote_description(RTCSessionDescription::offer(offer.sdp).unwrap()).await.unwrap();
    let answer = receiver.create_answer(None).await.unwrap();
    receiver.set_local_description(answer.clone()).await.unwrap();
    sender.set_remote_description(RTCSessionDescription::answer(answer.sdp).unwrap()).await.unwrap();

    // pump synthetic RTP for a bit
    let writer = tokio::spawn(async move {
        let mut seq = 0u16;
        let mut ts = 0u32;
        for _ in 0..100 {
            let pkt = webrtc::rtp::packet::Packet {
                header: webrtc::rtp::header::Header {
                    version: 2,
                    payload_type: 111,
                    sequence_number: seq,
                    timestamp: ts,
                    ssrc: 12345,
                    ..Default::default()
                },
                payload: bytes::Bytes::from_static(&[0x80, 0, 0, 0, 0, 0, 0, 0]),
            };
            let _ = track.write_rtp(&pkt).await;
            seq = seq.wrapping_add(1);
            ts = ts.wrapping_add(960);
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });

    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if received.load(Ordering::Relaxed) > 5 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("no RTP forwarded within 15s");

    writer.abort();
    assert!(received.load(Ordering::Relaxed) > 5, "expected forwarded RTP packets");
    let _ = sender.close().await;
    let _ = receiver.close().await;
}
