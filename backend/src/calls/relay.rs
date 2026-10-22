//! Call media through the edge, over one QUIC connection.
//!
//! A box that tunnels through an edge (`losos.proxy`) opens no port of its
//! own, and calls keep it that way: the edge takes the browsers' WebRTC
//! datagrams on a UDP port it allocates to this box and hands each one down
//! this connection as a QUIC DATAGRAM, and the SFU's answers go back the
//! same way. The browsers are told the edge's addresses as ICE candidates.
//! What crosses the edge is DTLS-SRTP around frames the browsers encrypted
//! end to end, so the edge reads no more of a call than the box does.
//!
//! The edge side is `edge/registrar/src/calls_relay.rs`. The protocol:
//!
//! 1. QUIC to the edge's relay port, ALPN [`ALPN`]. The edge's certificate
//!    is self-signed and pinned by its SHA-256, which the box reads from
//!    the registrar's `GET /calls/relay` over the same URL its tunnel and
//!    announce already trust.
//! 2. The box opens one bidirectional stream and writes
//!    `{"id": <appliance id>, "token": <its edge token>}`; the edge answers
//!    `{"addresses": ["ip:port", ...]}`, the addresses browsers send to, or
//!    `{"error": ...}` and closes.
//! 3. Every datagram, either way, is a [`frame`]: which of those addresses
//!    (one byte), the browser's address, then the packet. A packet larger
//!    than the connection's datagram limit goes as one frame on a
//!    unidirectional stream of its own instead.

use anyhow::Context;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// The ALPN both sides insist on.
pub const ALPN: &[u8] = b"losos-calls-relay/1";

/// The name the box asks for. The certificate is pinned by its hash, so the
/// name only has to be well formed.
const SERVER_NAME: &str = "losos-calls-relay";

/// Largest packet carried. WebRTC keeps its datagrams near 1200 bytes.
pub const MAX_PACKET: usize = 2048;

/// The front of a frame: which offered address, then the browser's address.
pub fn frame(index: u8, peer: SocketAddr, packet: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(packet.len() + 20);
    out.push(index);
    match peer.ip() {
        IpAddr::V4(ip) => {
            out.push(4);
            out.extend_from_slice(&ip.octets());
        }
        IpAddr::V6(ip) => {
            out.push(6);
            out.extend_from_slice(&ip.octets());
        }
    }
    out.extend_from_slice(&peer.port().to_be_bytes());
    out.extend_from_slice(packet);
    out
}

/// The parts of a frame, or `None` for anything malformed.
pub fn unframe(frame: &[u8]) -> Option<(u8, SocketAddr, &[u8])> {
    let (&index, rest) = frame.split_first()?;
    let (&family, rest) = rest.split_first()?;
    let (ip, rest) = match family {
        4 => {
            let (ip, rest) = rest.split_first_chunk::<4>()?;
            (IpAddr::V4(Ipv4Addr::from(*ip)), rest)
        }
        6 => {
            let (ip, rest) = rest.split_first_chunk::<16>()?;
            (IpAddr::V6(Ipv6Addr::from(*ip)), rest)
        }
        _ => return None,
    };
    let (port, packet) = rest.split_first_chunk::<2>()?;
    if packet.is_empty() || packet.len() > MAX_PACKET {
        return None;
    }
    Some((
        index,
        SocketAddr::new(ip, u16::from_be_bytes(*port)),
        packet,
    ))
}

#[derive(Serialize)]
struct Hello<'a> {
    id: &'a str,
    token: &'a str,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Answer {
    Welcome { addresses: Vec<SocketAddr> },
    Refused { error: String },
}

/// A packet from a browser, as the edge took it.
pub struct Incoming {
    pub index: u8,
    pub peer: SocketAddr,
    pub packet: Vec<u8>,
}

/// The connection to the edge, and the addresses it took for this box.
#[derive(Clone)]
pub struct Link {
    conn: quinn::Connection,
    addresses: Vec<SocketAddr>,
    // Held so the endpoint lives as long as any clone of the link.
    _endpoint: quinn::Endpoint,
}

/// The edge's relay port as `GET /calls/relay` gives it.
pub struct Edge<'a> {
    /// `host:port` of the relay.
    pub addr: &'a str,
    /// SHA-256 of the relay's certificate.
    pub cert_sha256: [u8; 32],
    /// The box's appliance id and edge token, the pair it registers with.
    pub id: &'a str,
    pub token: &'a str,
}

/// Connect, say who this box is, and start taking packets.
pub async fn connect(edge: Edge<'_>) -> anyhow::Result<(Link, mpsc::Receiver<Incoming>)> {
    let target = tokio::net::lookup_host(edge.addr)
        .await
        .with_context(|| format!("resolving {}", edge.addr))?
        .next()
        .with_context(|| format!("{} has no address", edge.addr))?;

    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut tls = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pinned {
            sha256: edge.cert_sha256,
            provider,
        }))
        .with_no_client_auth();
    tls.alpn_protocols = vec![ALPN.to_vec()];
    let mut client = quinn::ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(tls)?,
    ));
    client.transport_config(Arc::new(transport()));

    let bind: SocketAddr = if target.is_ipv6() {
        (Ipv6Addr::UNSPECIFIED, 0).into()
    } else {
        (Ipv4Addr::UNSPECIFIED, 0).into()
    };
    let mut endpoint = quinn::Endpoint::client(bind)?;
    endpoint.set_default_client_config(client);
    let conn = endpoint
        .connect(target, SERVER_NAME)?
        .await
        .with_context(|| format!("QUIC to the edge at {target}"))?;

    let (mut send, mut recv) = conn.open_bi().await?;
    send.write_all(&serde_json::to_vec(&Hello {
        id: edge.id,
        token: edge.token,
    })?)
    .await?;
    send.finish()?;
    let answer = tokio::time::timeout(Duration::from_secs(10), recv.read_to_end(64 * 1024))
        .await
        .context("the edge did not answer")??;
    let addresses = match serde_json::from_slice(&answer).context("the edge's answer")? {
        Answer::Welcome { addresses } if !addresses.is_empty() => addresses,
        Answer::Welcome { .. } => anyhow::bail!("the edge offered no address"),
        Answer::Refused { error } => anyhow::bail!("the edge refused this box: {error}"),
    };
    anyhow::ensure!(
        addresses.len() <= 256,
        "the edge offered too many addresses"
    );

    let (tx, rx) = mpsc::channel(4096);
    let datagrams = conn.clone();
    let to_sfu = tx.clone();
    tokio::spawn(async move {
        while let Ok(d) = datagrams.read_datagram().await {
            if let Some((index, peer, packet)) = unframe(&d) {
                let incoming = Incoming {
                    index,
                    peer,
                    packet: packet.to_vec(),
                };
                // A full queue drops the packet, as the network would.
                if let Err(mpsc::error::TrySendError::Closed(_)) = to_sfu.try_send(incoming) {
                    return;
                }
            }
        }
    });
    let streams = conn.clone();
    tokio::spawn(async move {
        while let Ok(mut s) = streams.accept_uni().await {
            let tx = tx.clone();
            tokio::spawn(async move {
                if let Ok(f) = s.read_to_end(MAX_PACKET + 20).await {
                    if let Some((index, peer, packet)) = unframe(&f) {
                        let _ = tx.try_send(Incoming {
                            index,
                            peer,
                            packet: packet.to_vec(),
                        });
                    }
                }
            });
        }
    });

    Ok((
        Link {
            conn,
            addresses,
            _endpoint: endpoint,
        },
        rx,
    ))
}

/// Keep-alives well inside the idle timeout, so a quiet call holds the
/// connection and a dead edge is noticed within the timeout.
fn transport() -> quinn::TransportConfig {
    let mut t = quinn::TransportConfig::default();
    t.keep_alive_interval(Some(Duration::from_secs(5)));
    t.max_idle_timeout(Some(
        quinn::IdleTimeout::try_from(Duration::from_secs(20))
            .expect("20 s is a valid idle timeout"),
    ));
    t
}

impl Link {
    /// The addresses browsers send to, in the order frames name them.
    pub fn addresses(&self) -> &[SocketAddr] {
        &self.addresses
    }

    /// Send `packet` to `peer` from the edge's address `index`. Never
    /// blocks: a packet that cannot go now is dropped, as UDP would.
    pub fn send(&self, index: u8, peer: SocketAddr, packet: &[u8]) {
        let f = frame(index, peer, packet);
        if self
            .conn
            .max_datagram_size()
            .is_some_and(|max| f.len() <= max)
        {
            let _ = self.conn.send_datagram(f.into());
            return;
        }
        let conn = self.conn.clone();
        tokio::spawn(async move {
            if let Ok(mut s) = conn.open_uni().await {
                if s.write_all(&f).await.is_ok() {
                    let _ = s.finish();
                }
            }
        });
    }

    /// Resolves when the connection is gone, with why.
    pub async fn closed(&self) -> quinn::ConnectionError {
        self.conn.closed().await
    }
}

/// Accepts exactly the certificate whose SHA-256 the registrar published,
/// and checks the handshake signature against it as any client would.
#[derive(Debug)]
struct Pinned {
    sha256: [u8; 32],
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let got = ring::digest::digest(&ring::digest::SHA256, end_entity.as_ref());
        if got.as_ref() == self.sha256 {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::InvalidCertificate(
                rustls::CertificateError::ApplicationVerificationFailure,
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// `--relay-cert`: 64 hex characters.
pub fn parse_sha256(s: &str) -> Result<[u8; 32], String> {
    let s = s.trim();
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("expected 64 hex characters".into());
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_for_both_families() {
        for peer in ["192.0.2.7:51234", "[2001:db8::7]:40000"] {
            let peer: SocketAddr = peer.parse().unwrap();
            let f = frame(3, peer, b"stun");
            assert_eq!(unframe(&f), Some((3, peer, &b"stun"[..])));
        }
    }

    #[test]
    fn malformed_frames_are_dropped() {
        let peer: SocketAddr = "192.0.2.7:1".parse().unwrap();
        let f = frame(0, peer, b"x");
        // Every truncation before the packet, an unknown family, no packet.
        for n in 0..f.len() {
            assert_eq!(unframe(&f[..n]), None, "cut at {n}");
        }
        let mut bad = f.clone();
        bad[1] = 5;
        assert_eq!(unframe(&bad), None);
        assert_eq!(unframe(&frame(0, peer, &[0; MAX_PACKET + 1])), None);
    }

    #[test]
    fn the_pin_is_hex() {
        let hex = "00ff".repeat(16);
        assert_eq!(parse_sha256(&hex).unwrap()[..2], [0x00, 0xff]);
        assert!(parse_sha256("abc").is_err());
        assert!(parse_sha256(&"zz".repeat(32)).is_err());
    }
}
