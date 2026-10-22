//! Call media for the boxes this edge serves, over QUIC.
//!
//! A box behind this edge opens no port, so the browsers in its calls send
//! their WebRTC datagrams here instead. Each box keeps one QUIC connection
//! to this relay; the relay gives it a UDP port of its own from
//! `--calls-relay-media-ports`, tells it which addresses browsers reach that
//! port on, and from then on hands every datagram that arrives there down
//! the connection as a QUIC DATAGRAM, and sends out whatever the box hands
//! back. The datagrams are DTLS-SRTP between the browser and the box's SFU
//! around frames the browsers encrypted end to end, so this edge reads
//! nothing of a call. The box's side, and the protocol, are in
//! `backend/src/calls/relay.rs`.
//!
//! Two rules keep the relay from being anyone's UDP cannon:
//!
//! * a box is who it says only with the token it registers with
//!   (`server::authenticate`, the same check as `/register`), and gets one
//!   port, which the next connection with that id takes over;
//! * the relay sends from a box's port only to an address that sent to
//!   that port in the last [`PEER_TTL`], so a box cannot use the edge to
//!   send to a host that never asked.

use miette::{miette, IntoDiagnostic, Result, WrapErr};
use quinn::crypto::rustls::QuicServerConfig;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;

/// The ALPN both sides insist on.
pub const ALPN: &[u8] = b"losos-calls-relay/1";

/// Largest packet carried. WebRTC keeps its datagrams near 1200 bytes.
pub const MAX_PACKET: usize = 2048;

/// How long an address that sent to a box's port may be sent to.
pub const PEER_TTL: Duration = Duration::from_secs(60);

/// Addresses remembered per box; past this, the stale ones go first.
const MAX_PEERS: usize = 1024;

/// What `--calls-relay-*` set.
#[derive(Debug, Clone)]
pub struct CallsRelayOpts {
    /// Where boxes reach the relay (UDP, QUIC).
    pub listen: SocketAddr,
    /// The UDP ports handed to boxes, one each, inclusive.
    pub media_ports: (u16, u16),
    /// `(bind, offered)`: an address to take media on, and the address
    /// browsers are told, which differ behind a 1:1 NAT. Empty: every
    /// address of every interface that is up, as itself.
    pub addresses: Vec<(IpAddr, IpAddr)>,
}

/// How the relay asks whether `(id, token)` is a box of this edge.
pub type Auth =
    Arc<dyn Fn(String, String) -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync>;

/// The relay's certificate, made at start and never written down: a
/// restart makes a new one, and boxes read its hash again when they
/// reconnect.
pub struct Relay {
    opts: CallsRelayOpts,
    cert: CertificateDer<'static>,
    key: Vec<u8>,
    sha256: String,
}

/// `GET /calls/relay`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub port: u16,
    pub cert_sha256: String,
}

impl Relay {
    pub fn new(opts: CallsRelayOpts) -> Result<Self> {
        let made = rcgen::generate_simple_self_signed(vec!["losos-calls-relay".to_string()])
            .into_diagnostic()
            .wrap_err("make the calls relay's certificate")?;
        let cert = made.cert.der().clone();
        let digest = ring::digest::digest(&ring::digest::SHA256, cert.as_ref());
        let sha256 = digest.as_ref().iter().map(|b| format!("{b:02x}")).collect();
        Ok(Self {
            opts,
            cert,
            key: made.signing_key.serialize_der(),
            sha256,
        })
    }

    pub fn info(&self) -> Info {
        Info {
            port: self.opts.listen.port(),
            cert_sha256: self.sha256.clone(),
        }
    }

    fn server_config(&self) -> Result<quinn::ServerConfig> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut tls = rustls::ServerConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13])
            .into_diagnostic()?
            .with_no_client_auth()
            .with_single_cert(
                vec![self.cert.clone()],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key.clone())),
            )
            .into_diagnostic()?;
        tls.alpn_protocols = vec![ALPN.to_vec()];
        let crypto = QuicServerConfig::try_from(tls).into_diagnostic()?;
        let mut config = quinn::ServerConfig::with_crypto(Arc::new(crypto));
        let mut transport = quinn::TransportConfig::default();
        transport.keep_alive_interval(Some(Duration::from_secs(5)));
        transport.max_idle_timeout(Some(
            quinn::IdleTimeout::try_from(Duration::from_secs(20)).into_diagnostic()?,
        ));
        // Media only travels on datagrams and one-packet streams; the
        // hello is the only bidirectional stream.
        transport.max_concurrent_bidi_streams(1u32.into());
        transport.max_concurrent_uni_streams(64u32.into());
        config.transport_config(Arc::new(transport));
        Ok(config)
    }

    /// The `(bind, offered)` pairs, discovered when none were given.
    fn addresses(&self) -> Vec<(IpAddr, IpAddr)> {
        if !self.opts.addresses.is_empty() {
            return self.opts.addresses.clone();
        }
        host_addresses().into_iter().map(|ip| (ip, ip)).collect()
    }
}

/// Interfaces no browser reaches: the mesh's overlays, container bridges.
const SKIP_INTERFACES: &[&str] = &[
    "cali", "vxlan", "tunl", "flannel", "cni", "kube", "docker", "podman", "veth", "virbr", "lo",
];

fn host_addresses() -> Vec<IpAddr> {
    let mut v: Vec<IpAddr> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| !i.is_loopback() && !i.is_link_local() && i.is_oper_up())
        .filter(|i| !SKIP_INTERFACES.iter().any(|p| i.name.starts_with(p)))
        .map(|i| i.ip())
        .collect();
    v.sort();
    v.dedup();
    v
}

/// The front of a frame: which offered address, then the browser's.
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

#[derive(Deserialize)]
struct Hello {
    id: String,
    token: String,
}

/// Which box holds which media port, and its connection, so a new
/// connection with the same id can close the old one.
///
/// A port stays taken until the connection that held it has closed its
/// sockets, so a box that reconnects gets the next port while its old
/// connection winds down.
#[derive(Default)]
struct Boxes {
    by_id: HashMap<String, quinn::Connection>,
    ports: BTreeSet<u16>,
}

impl Boxes {
    fn claim(&mut self, id: &str, conn: &quinn::Connection, range: (u16, u16)) -> Option<u16> {
        if let Some(old) = self.by_id.remove(id) {
            old.close(2u32.into(), b"replaced");
        }
        let port = (range.0..=range.1).find(|p| !self.ports.contains(p))?;
        self.ports.insert(port);
        self.by_id.insert(id.to_string(), conn.clone());
        Some(port)
    }

    fn release(&mut self, id: &str, conn: &quinn::Connection, port: u16) {
        self.ports.remove(&port);
        if self
            .by_id
            .get(id)
            .is_some_and(|c| c.stable_id() == conn.stable_id())
        {
            self.by_id.remove(id);
        }
    }
}

/// Accept boxes until the endpoint fails.
pub async fn run(relay: Arc<Relay>, auth: Auth) -> Result<()> {
    let endpoint = quinn::Endpoint::server(relay.server_config()?, relay.opts.listen)
        .into_diagnostic()
        .wrap_err_with(|| format!("bind the calls relay on {}", relay.opts.listen))?;
    tracing::info!("calls relay on udp {}", relay.opts.listen);
    let boxes = Arc::new(Mutex::new(Boxes::default()));
    while let Some(incoming) = endpoint.accept().await {
        let relay = relay.clone();
        let auth = auth.clone();
        let boxes = boxes.clone();
        tokio::spawn(async move {
            let conn = match incoming.await {
                Ok(c) => c,
                Err(e) => {
                    tracing::debug!("calls relay handshake: {e}");
                    return;
                }
            };
            if let Err(e) = serve_box(&relay, &auth, &boxes, &conn).await {
                tracing::info!("calls relay, {}: {e}", conn.remote_address());
            }
        });
    }
    Err(miette!("the calls relay's endpoint closed"))
}

async fn serve_box(
    relay: &Relay,
    auth: &Auth,
    boxes: &Mutex<Boxes>,
    conn: &quinn::Connection,
) -> Result<()> {
    let wait = Duration::from_secs(10);
    let (mut send, mut recv) = tokio::time::timeout(wait, conn.accept_bi())
        .await
        .into_diagnostic()?
        .into_diagnostic()?;
    let hello = tokio::time::timeout(wait, recv.read_to_end(4096))
        .await
        .into_diagnostic()?
        .into_diagnostic()?;
    let refuse = |why: &'static str| {
        conn.close(1u32.into(), why.as_bytes());
        miette!("refused: {why}")
    };
    let Ok(hello) = serde_json::from_slice::<Hello>(&hello) else {
        return Err(refuse("bad hello"));
    };
    if !auth(hello.id.clone(), hello.token).await {
        let _ = send.write_all(br#"{"error":"unauthorized"}"#).await;
        let _ = send.finish();
        let _ = send.stopped().await;
        return Err(refuse("unauthorized"));
    }
    let id = hello.id;

    let claimed = boxes
        .lock()
        .expect("boxes")
        .claim(&id, conn, relay.opts.media_ports);
    let Some(port) = claimed else {
        let _ = send.write_all(br#"{"error":"no media port free"}"#).await;
        let _ = send.finish();
        let _ = send.stopped().await;
        return Err(refuse("no media port free"));
    };
    let result = forward(relay, conn, port, &id, &mut send).await;
    boxes.lock().expect("boxes").release(&id, conn, port);
    result
}

/// Bind the box's port, tell it the addresses, and carry packets both ways
/// until the connection goes.
async fn forward(
    relay: &Relay,
    conn: &quinn::Connection,
    port: u16,
    id: &str,
    hello: &mut quinn::SendStream,
) -> Result<()> {
    let mut sockets = Vec::new();
    let mut offered = Vec::new();
    for (bind, shown) in relay.addresses().into_iter().take(256) {
        match UdpSocket::bind((bind, port)).await {
            Ok(s) => {
                sockets.push(Arc::new(s));
                offered.push(SocketAddr::new(shown, port));
            }
            Err(e) => tracing::warn!("calls relay: cannot bind {bind}:{port} for {id}: {e}"),
        }
    }
    if sockets.is_empty() {
        let _ = hello.write_all(br#"{"error":"no media address"}"#).await;
        let _ = hello.finish();
        let _ = hello.stopped().await;
        return Err(miette!("no address to take {id}'s media on"));
    }
    let welcome =
        serde_json::to_vec(&serde_json::json!({ "addresses": offered })).into_diagnostic()?;
    hello.write_all(&welcome).await.into_diagnostic()?;
    hello.finish().into_diagnostic()?;
    tracing::info!("calls relay: {id} takes media on {offered:?}");

    let peers: Arc<Mutex<HashMap<(u8, SocketAddr), Instant>>> = Arc::default();
    let mut tasks = tokio::task::JoinSet::new();
    for (index, socket) in sockets.iter().enumerate() {
        let socket = socket.clone();
        let conn = conn.clone();
        let peers = peers.clone();
        let index = index as u8;
        tasks.spawn(async move {
            let mut buf = vec![0u8; MAX_PACKET + 1];
            loop {
                let Ok((n, from)) = socket.recv_from(&mut buf).await else {
                    continue;
                };
                if n == 0 || n > MAX_PACKET {
                    continue;
                }
                remember(&peers, index, from);
                let f = frame(index, from, &buf[..n]);
                if conn.max_datagram_size().is_some_and(|max| f.len() <= max) {
                    if let Err(quinn::SendDatagramError::ConnectionLost(_)) =
                        conn.send_datagram(f.into())
                    {
                        return;
                    }
                } else {
                    let conn = conn.clone();
                    tokio::spawn(async move {
                        if let Ok(mut s) = conn.open_uni().await {
                            if s.write_all(&f).await.is_ok() {
                                let _ = s.finish();
                            }
                        }
                    });
                }
            }
        });
    }

    let out = |f: &[u8]| {
        let Some((index, peer, packet)) = unframe(f) else {
            return;
        };
        let Some(socket) = sockets.get(usize::from(index)) else {
            return;
        };
        let fresh = peers
            .lock()
            .expect("peers")
            .get(&(index, peer))
            .is_some_and(|seen| seen.elapsed() < PEER_TTL);
        if fresh {
            let _ = socket.try_send_to(packet, peer);
        }
    };
    let (tx, mut from_streams) = tokio::sync::mpsc::channel::<Vec<u8>>(256);
    let accept = conn.clone();
    tasks.spawn(async move {
        while let Ok(mut s) = accept.accept_uni().await {
            let tx = tx.clone();
            tokio::spawn(async move {
                if let Ok(f) = s.read_to_end(MAX_PACKET + 20).await {
                    let _ = tx.send(f).await;
                }
            });
        }
    });
    // Datagrams and one-packet streams, until the connection goes.
    loop {
        tokio::select! {
            d = conn.read_datagram() => match d {
                Ok(d) => out(&d),
                Err(e) => {
                    tasks.shutdown().await;
                    tracing::info!("calls relay: {id} left: {e}");
                    return Ok(());
                }
            },
            Some(f) = from_streams.recv() => out(&f),
        }
    }
}

/// Note that `peer` sent to the box's address `index`, and forget the
/// stalest addresses once there are too many.
fn remember(peers: &Mutex<HashMap<(u8, SocketAddr), Instant>>, index: u8, peer: SocketAddr) {
    let mut peers = peers.lock().expect("peers");
    let now = Instant::now();
    if peers.len() >= MAX_PEERS && !peers.contains_key(&(index, peer)) {
        peers.retain(|_, seen| now.duration_since(*seen) < PEER_TTL);
        if peers.len() >= MAX_PEERS {
            if let Some(oldest) = peers.iter().min_by_key(|(_, seen)| **seen).map(|(k, _)| *k) {
                peers.remove(&oldest);
            }
        }
    }
    peers.insert((index, peer), now);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_for_both_families() {
        for peer in ["192.0.2.7:51234", "[2001:db8::7]:40000"] {
            let peer: SocketAddr = peer.parse().unwrap();
            assert_eq!(
                unframe(&frame(9, peer, b"dtls")),
                Some((9, peer, &b"dtls"[..]))
            );
        }
    }

    #[test]
    fn malformed_frames_are_dropped() {
        let peer: SocketAddr = "192.0.2.7:1".parse().unwrap();
        let f = frame(0, peer, b"x");
        for n in 0..f.len() {
            assert_eq!(unframe(&f[..n]), None, "cut at {n}");
        }
        assert_eq!(unframe(&frame(0, peer, &[0; MAX_PACKET + 1])), None);
    }

    #[test]
    fn the_peer_list_stays_bounded() {
        let peers = Mutex::new(HashMap::new());
        for port in 0..(MAX_PEERS as u16 + 10) {
            remember(&peers, 0, SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port));
        }
        assert_eq!(peers.lock().unwrap().len(), MAX_PEERS);
    }
}
