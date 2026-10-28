//! The calls relay (`calls_relay`), driven as a box and a browser would:
//! a real `serve`, a real QUIC connection, real UDP.

mod common;

use common::{Edge, TenantSpec, GOOD_TOKEN, OTHER_TOKEN};
use losos_registrar::calls_relay::{frame, unframe, CallsRelayOpts, ALPN};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;

fn free_udp_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn edge_with_relay(tag: &str, media: (u16, u16)) -> (Edge, SocketAddr) {
    let listen: SocketAddr = format!("127.0.0.1:{}", free_udp_port()).parse().unwrap();
    let edge = Edge::start_custom(
        tag,
        &[TenantSpec::new("mattbox", "mattbox.losos.cfd", GOOD_TOKEN)],
        move |opts, _| {
            opts.calls_relay = Some(CallsRelayOpts {
                listen,
                media_ports: media,
                addresses: vec![("127.0.0.1".parse().unwrap(), "127.0.0.1".parse().unwrap())],
            });
        },
    )
    .await;
    (edge, listen)
}

async fn pin(edge: &Edge) -> [u8; 32] {
    let info: serde_json::Value = edge
        .client
        .get(format!("{}/calls/relay", edge.base))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let hex = info["certSha256"].as_str().unwrap();
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap();
    }
    out
}

#[derive(Debug)]
struct Pinned([u8; 32], Arc<CryptoProvider>);

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        cert: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let got = ring::digest::digest(&ring::digest::SHA256, cert.as_ref());
        if got.as_ref() == self.0 {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::InvalidCertificate(
                rustls::CertificateError::ApplicationVerificationFailure,
            ))
        }
    }
    fn verify_tls12_signature(
        &self,
        m: &[u8],
        c: &CertificateDer<'_>,
        d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(m, c, d, &self.1.signature_verification_algorithms)
    }
    fn verify_tls13_signature(
        &self,
        m: &[u8],
        c: &CertificateDer<'_>,
        d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(m, c, d, &self.1.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.1.signature_verification_algorithms.supported_schemes()
    }
}

/// The box's side: connect with the pin, then say who it is. Returns the
/// connection and the edge's answer.
async fn connect(
    relay: SocketAddr,
    pin: [u8; 32],
    id: &str,
    token: &str,
) -> Result<(quinn::Connection, serde_json::Value), String> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut tls = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .unwrap()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pinned(pin, provider)))
        .with_no_client_auth();
    tls.alpn_protocols = vec![ALPN.to_vec()];
    let config = quinn::ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(tls).unwrap(),
    ));
    let mut endpoint = quinn::Endpoint::client("127.0.0.1:0".parse().unwrap()).unwrap();
    endpoint.set_default_client_config(config);
    let conn = endpoint
        .connect(relay, "losos-calls-relay")
        .unwrap()
        .await
        .map_err(|e| e.to_string())?;
    let (mut send, mut recv) = conn.open_bi().await.map_err(|e| e.to_string())?;
    let hello = serde_json::json!({ "id": id, "token": token });
    send.write_all(hello.to_string().as_bytes()).await.unwrap();
    send.finish().unwrap();
    let answer = recv.read_to_end(4096).await.map_err(|e| e.to_string())?;
    Ok((
        conn,
        serde_json::from_slice(&answer).map_err(|e| e.to_string())?,
    ))
}

async fn next_frame(conn: &quinn::Connection) -> Vec<u8> {
    let datagram = conn.read_datagram();
    let stream = async {
        let mut s = conn.accept_uni().await.unwrap();
        s.read_to_end(4096).await.unwrap()
    };
    tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! {
            d = datagram => d.unwrap().to_vec(),
            s = stream => s,
        }
    })
    .await
    .expect("a frame from the edge")
}

#[tokio::test]
async fn an_edge_without_the_relay_answers_404() {
    let edge = Edge::start(
        "calls-relay-off",
        &[TenantSpec::new("mattbox", "mattbox.losos.cfd", GOOD_TOKEN)],
    )
    .await;
    let r = edge
        .client
        .get(format!("{}/calls/relay", edge.base))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);
}

/// The whole path: the box gets a port, a browser's packet reaches the box
/// with the browser's address, and the box's answer reaches the browser,
/// small packets as datagrams and large ones on a stream.
#[tokio::test]
async fn media_goes_both_ways_through_the_relay() {
    let (edge, relay) = edge_with_relay("calls-relay-path", (46100, 46109)).await;
    let (conn, answer) = connect(relay, pin(&edge).await, "mattbox", GOOD_TOKEN)
        .await
        .unwrap();
    assert_eq!(answer["addresses"], serde_json::json!(["127.0.0.1:46100"]));

    let browser = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let me = browser.local_addr().unwrap();
    for size in [100, 1500] {
        let packet = vec![size as u8; size];
        browser.send_to(&packet, "127.0.0.1:46100").await.unwrap();
        let f = next_frame(&conn).await;
        assert_eq!(unframe(&f), Some((0, me, &packet[..])), "{size} bytes in");

        let back = vec![7u8; size];
        let f = frame(0, me, &back);
        if conn.max_datagram_size().is_some_and(|m| f.len() <= m) {
            conn.send_datagram(f.into()).unwrap();
        } else {
            let mut s = conn.open_uni().await.unwrap();
            s.write_all(&f).await.unwrap();
            s.finish().unwrap();
        }
        let mut buf = [0u8; 4096];
        let (n, from) = tokio::time::timeout(Duration::from_secs(5), browser.recv_from(&mut buf))
            .await
            .expect("the answer reaches the browser")
            .unwrap();
        assert_eq!(from.to_string(), "127.0.0.1:46100");
        assert_eq!(&buf[..n], &back[..], "{size} bytes out");
    }
}

/// A box cannot use the edge to send to an address that never sent to it.
#[tokio::test]
async fn the_relay_sends_only_to_addresses_that_asked() {
    let (edge, relay) = edge_with_relay("calls-relay-reflect", (46110, 46119)).await;
    let (conn, _) = connect(relay, pin(&edge).await, "mattbox", GOOD_TOKEN)
        .await
        .unwrap();
    let stranger = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let target = stranger.local_addr().unwrap();
    conn.send_datagram(frame(0, target, b"unasked").into())
        .unwrap();
    let mut buf = [0u8; 64];
    let got = tokio::time::timeout(Duration::from_millis(700), stranger.recv_from(&mut buf)).await;
    assert!(got.is_err(), "the stranger got a packet it never asked for");
}

#[tokio::test]
async fn a_wrong_token_gets_no_port() {
    let (edge, relay) = edge_with_relay("calls-relay-token", (46120, 46129)).await;
    let pin = pin(&edge).await;
    for (id, token) in [
        ("mattbox", OTHER_TOKEN),
        ("nobody", GOOD_TOKEN),
        ("mattbox", ""),
    ] {
        // Either the answer says so, or the edge closed before it was read.
        if let Ok((_, answer)) = connect(relay, pin, id, token).await {
            assert_eq!(answer["error"], "unauthorized", "{id}");
        }
    }
    // Nothing is bound for a refused box.
    assert!(std::net::UdpSocket::bind("127.0.0.1:46120").is_ok());
}

/// The certificate is the one the registrar named, or no connection.
#[tokio::test]
async fn a_box_refuses_a_relay_it_did_not_pin() {
    let (_edge, relay) = edge_with_relay("calls-relay-pin", (46130, 46139)).await;
    assert!(connect(relay, [0u8; 32], "mattbox", GOOD_TOKEN)
        .await
        .is_err());
}

/// The box's next connection takes its port over, and the old one is shut.
#[tokio::test]
async fn a_reconnecting_box_replaces_its_old_connection() {
    let (edge, relay) = edge_with_relay("calls-relay-again", (46140, 46149)).await;
    let pin = pin(&edge).await;
    let (old, first) = connect(relay, pin, "mattbox", GOOD_TOKEN).await.unwrap();
    let (_new, second) = connect(relay, pin, "mattbox", GOOD_TOKEN).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), old.closed())
        .await
        .expect("the old connection closes");
    // The old port is free again a moment later, so the new one may have
    // taken the next; either way, it has one.
    assert!(first["addresses"].is_array() && second["addresses"].is_array());
}

const CALL: &str = "0123456789abcdef0123456789abcdef";
const INVITE: &str = "fedcba9876543210fedcba9876543210";

fn guest_link(id: &str, token: &str, call: &str, invite: &str) -> serde_json::Value {
    serde_json::json!({ "appliance_id": id, "token": token, "call": call, "invite": invite })
}

#[tokio::test]
async fn only_an_official_edge_makes_guest_links_and_only_on_the_boxs_own_name() {
    let tenants = [
        TenantSpec::new("mattbox", "mattbox.losos.cfd", GOOD_TOKEN),
        TenantSpec::new("otherbox", "otherbox.losos.cfd", OTHER_TOKEN),
    ];
    // Not official: no link.
    let plain = Edge::start_custom("guest-plain", &tenants, |_, _| {}).await;
    let (status, body) = plain
        .post(
            "/calls/guest-link",
            guest_link("mattbox", GOOD_TOKEN, CALL, INVITE),
        )
        .await;
    assert_eq!(status, 403, "{body}");
    assert!(body.contains("notOfficial"), "{body}");
    plain.shutdown().await;

    let scratch = std::env::temp_dir().join(format!("losos-guest-link-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let gateway = common::claimed_gateway(&scratch);
    let edge = Edge::start_custom("guest-official", &tenants, move |opts, _| {
        opts.gateway = Some(gateway);
    })
    .await;
    let (status, body) = edge
        .post(
            "/calls/guest-link",
            guest_link("mattbox", GOOD_TOKEN, CALL, INVITE),
        )
        .await;
    assert_eq!(status, 200, "{body}");
    let url: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        url["url"],
        format!("https://mattbox.losos.cfd/calls/guest/?call={CALL}&invite={INVITE}")
    );
    // Another box's token makes no link for this one, and a wrong token none.
    let (status, _) = edge
        .post(
            "/calls/guest-link",
            guest_link("mattbox", OTHER_TOKEN, CALL, INVITE),
        )
        .await;
    assert_eq!(status, 401);
    let (status, body) = edge
        .post(
            "/calls/guest-link",
            guest_link("otherbox", OTHER_TOKEN, CALL, INVITE),
        )
        .await;
    assert_eq!(status, 200);
    assert!(body.contains("https://otherbox.losos.cfd/"), "{body}");
    // Only ids the box made: nothing that could bend the URL.
    for (call, invite) in [
        (CALL, "x/../../evil"),
        ("not-hex", INVITE),
        (CALL, "ABCDEF0123456789ABCDEF0123456789"),
    ] {
        let (status, _) = edge
            .post(
                "/calls/guest-link",
                guest_link("mattbox", GOOD_TOKEN, call, invite),
            )
            .await;
        assert_eq!(status, 400, "{call} {invite}");
    }
    edge.shutdown().await;
    let _ = std::fs::remove_dir_all(&scratch);
}
