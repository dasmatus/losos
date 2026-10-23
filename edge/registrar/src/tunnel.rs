//! The tunnel between an edge and the boxes behind it, over QUIC.
//!
//! A box opens no port. It keeps one QUIC connection to its edge, and the
//! edge carries every TCP connection meant for the box (a browser's request
//! after Traefik, the model head reaching a lending box's memory) as one
//! stream of that connection. Streams are independent, so one slow download
//! holds up nothing else on the link; a new request costs no handshake,
//! because the connection is already up; and the connection follows the box
//! when its address changes, which a home connection's address does.
//!
//! Two halves of one binary:
//!
//! * [`serve`] runs on the edge (`losos-registrar tunnel-server`). It listens
//!   for boxes on UDP and, for every service in the file the registrar keeps
//!   ([`ServerFile`]), on a loopback TCP port Traefik forwards to. It re-reads
//!   that file every second, so a box that registers is reachable within a
//!   second of the registrar's pass, and a restart of the registrar costs no
//!   tunnel.
//! * [`run_client`] runs on the box, and on an edge that relays its boxes to
//!   a hub (`losos-registrar tunnel-client`). It dials the edge, names the
//!   services it serves with their tokens, and connects every stream the edge
//!   opens to the service's local address.
//!
//! The protocol, ALPN [`ALPN`], TLS 1.3:
//!
//! 1. The edge's certificate is self-signed, made once on its first start
//!    and kept ([`load_or_make_identity`]). The client accepts exactly the
//!    certificate whose SHA-256 it holds (its pin), which a box reads from the
//!    registrar's `GET /tunnel-pin` the first time and keeps from then on.
//! 2. The client opens one bidirectional stream and writes a [`Hello`]: the
//!    services it serves, each with its token. The edge answers a
//!    [`Welcome`] naming which it took, and from then on is the holder of
//!    each; a later connection that proves the same token takes a service
//!    over. A connection that took none is closed.
//! 3. For each TCP connection on a service's port the edge opens a
//!    bidirectional stream, writes the service's name (one length byte, then
//!    the name) and then the bytes both ways until either side closes.
//!
//! Where UDP does not get through, the client reaches the same port over
//! TCP: TLS 1.3 with the same certificate, the same hello, and HTTP/2
//! streams in place of QUIC's (see "TCP fallback" below). It keeps trying
//! QUIC and moves back when that works. A link another one took over keeps
//! its open streams for half a minute before the edge closes it, so a box
//! that reconnects drops no download in flight.

use bytes::Bytes;
use miette::{miette, IntoDiagnostic, Result, WrapErr};
use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{ready, Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::client::TlsStream as TlsClientStream;
use tokio_rustls::{TlsAcceptor, TlsConnector};

/// The ALPN both sides insist on.
pub const ALPN: &[u8] = b"losos-tunnel/1";

/// The name the client asks for. The certificate is pinned by its hash, so
/// the name only has to be well formed.
const SERVER_NAME: &str = "losos-tunnel";

/// Largest hello either side reads. A spoke relaying many boxes names each.
const MAX_HELLO: usize = 1 << 20;

/// Most services one connection may name.
pub const MAX_SERVICES: usize = 4096;

/// A service name is one length byte long at most.
const MAX_NAME: usize = 255;

/// How long a peer has to say its hello, or a stream its header.
const HANDSHAKE: Duration = Duration::from_secs(10);

/// How often the server re-reads its file and the client its own.
const SERVER_RELOAD: Duration = Duration::from_secs(1);
const CLIENT_RELOAD: Duration = Duration::from_secs(2);

/// Streams the edge may have open to one box at a time: concurrent requests
/// to it, each a browser connection Traefik holds open.
const MAX_STREAMS: u32 = 4096;

/// The file the registrar writes and the server reads: every service the
/// edge carries, keyed by name. Absent means none yet.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerFile {
    #[serde(default)]
    pub services: BTreeMap<String, ServerService>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerService {
    /// What a client proves to take the service.
    pub token: String,
    /// The loopback TCP port Traefik (or the model head) connects to.
    pub bind: SocketAddr,
}

/// The client's file: where the edge is, its pin, and what this side serves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientFile {
    /// The edge's tunnel, `host:port` (UDP).
    pub remote: String,
    /// SHA-256 of the edge's certificate, 64 hex characters.
    pub pin: String,
    #[serde(default)]
    pub services: BTreeMap<String, ClientService>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientService {
    pub token: String,
    /// Where a stream for this service is delivered, `host:port`.
    pub local: String,
}

/// What a client names when it connects.
#[derive(Debug, Serialize, Deserialize)]
pub struct Hello {
    pub services: Vec<Claim>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claim {
    pub name: String,
    pub token: String,
}

/// The edge's answer to a [`Hello`].
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Welcome {
    pub accepted: Vec<String>,
    pub refused: Vec<String>,
}

// ── Identity ────────────────────────────────────────────────────────────

/// The edge's certificate and key.
pub struct Identity {
    cert: CertificateDer<'static>,
    key: Vec<u8>,
}

impl Identity {
    /// SHA-256 of the certificate, the pin clients hold.
    pub fn pin(&self) -> String {
        pin_of(self.cert.as_ref())
    }
}

/// The lowercase hex SHA-256 of `der`.
pub fn pin_of(der: &[u8]) -> String {
    hex(ring::digest::digest(&ring::digest::SHA256, der).as_ref())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A pin as 32 bytes, or `None` for anything but 64 hex characters.
pub fn parse_pin(s: &str) -> Option<[u8; 32]> {
    let s = s.trim();
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

/// The certificate in `dir`, or a new one written there on first start:
/// `cert.der`, `key.der` (0600) and `pin` (0644, the hex the registrar
/// serves). Kept across restarts on purpose: every box holds the pin, and a
/// new certificate would cut each of them off until someone cleared it.
pub fn load_or_make_identity(dir: &Path) -> Result<Identity> {
    let cert_path = dir.join("cert.der");
    let key_path = dir.join("key.der");
    let identity = match (std::fs::read(&cert_path), std::fs::read(&key_path)) {
        (Ok(cert), Ok(key)) => Identity {
            cert: CertificateDer::from(cert),
            key,
        },
        _ => {
            let made = rcgen::generate_simple_self_signed(vec![SERVER_NAME.to_string()])
                .into_diagnostic()
                .wrap_err("make the tunnel's certificate")?;
            std::fs::create_dir_all(dir)
                .into_diagnostic()
                .wrap_err_with(|| format!("create {}", dir.display()))?;
            let identity = Identity {
                cert: made.cert.der().clone(),
                key: made.signing_key.serialize_der(),
            };
            write_mode(&key_path, &identity.key, 0o600)?;
            write_mode(&cert_path, identity.cert.as_ref(), 0o644)?;
            tracing::info!("made the tunnel's certificate in {}", dir.display());
            identity
        }
    };
    write_mode(
        &dir.join("pin"),
        format!("{}\n", identity.pin()).as_bytes(),
        0o644,
    )?;
    Ok(identity)
}

fn write_mode(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let tmp = path.with_extension("new");
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(&tmp)
        .into_diagnostic()
        .wrap_err_with(|| format!("write {}", tmp.display()))?;
    std::io::Write::write_all(&mut f, bytes).into_diagnostic()?;
    f.sync_all().into_diagnostic()?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode)).into_diagnostic()?;
    std::fs::rename(&tmp, path)
        .into_diagnostic()
        .wrap_err_with(|| format!("rename into {}", path.display()))
}

// ── Transport ───────────────────────────────────────────────────────────

/// Settings both ends share. The box sends keep-alives well inside the idle
/// timeout, so a quiet box holds its NAT binding and a dead edge is noticed
/// within half a minute.
fn transport(incoming_bidi: u32) -> quinn::TransportConfig {
    let mut t = quinn::TransportConfig::default();
    t.keep_alive_interval(Some(Duration::from_secs(10)));
    t.max_idle_timeout(Some(
        quinn::IdleTimeout::try_from(Duration::from_secs(30)).expect("30 s is a valid timeout"),
    ));
    t.max_concurrent_bidi_streams(incoming_bidi.into());
    t.max_concurrent_uni_streams(0u32.into());
    t
}

fn server_config(id: &Identity) -> Result<quinn::ServerConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut tls = rustls::ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .into_diagnostic()?
        .with_no_client_auth()
        .with_single_cert(
            vec![id.cert.clone()],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(id.key.clone())),
        )
        .into_diagnostic()?;
    tls.alpn_protocols = vec![ALPN.to_vec()];
    let crypto = QuicServerConfig::try_from(tls).into_diagnostic()?;
    let mut config = quinn::ServerConfig::with_crypto(Arc::new(crypto));
    // The client opens one stream, its hello; every other stream is the
    // edge's.
    config.transport_config(Arc::new(transport(1)));
    Ok(config)
}

/// The client's TLS: TLS 1.3, the tunnel's ALPN, and the pinned
/// certificate. Both transports use it.
fn pinned_tls(pin: [u8; 32]) -> Result<rustls::ClientConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut tls = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .into_diagnostic()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pinned {
            sha256: pin,
            provider,
        }))
        .with_no_client_auth();
    tls.alpn_protocols = vec![ALPN.to_vec()];
    Ok(tls)
}

fn client_config(pin: [u8; 32]) -> Result<quinn::ClientConfig> {
    let tls = pinned_tls(pin)?;
    let mut client =
        quinn::ClientConfig::new(Arc::new(QuicClientConfig::try_from(tls).into_diagnostic()?));
    client.transport_config(Arc::new(transport(MAX_STREAMS)));
    Ok(client)
}

/// Accepts exactly the certificate whose SHA-256 is pinned, and checks the
/// handshake signature against it as any client would.
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
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        // Both transports are TLS 1.3 only.
        Err(rustls::Error::PeerIncompatible(
            rustls::PeerIncompatible::Tls12NotOffered,
        ))
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

// ── Stream header ───────────────────────────────────────────────────────

/// The front of every stream the edge opens: which service it is for.
pub fn header(name: &str) -> Option<Vec<u8>> {
    if name.is_empty() || name.len() > MAX_NAME {
        return None;
    }
    let mut out = Vec::with_capacity(name.len() + 1);
    out.push(name.len() as u8);
    out.extend_from_slice(name.as_bytes());
    Some(out)
}

async fn read_header<R: AsyncRead + Unpin>(recv: &mut R) -> Option<String> {
    let mut len = [0u8; 1];
    recv.read_exact(&mut len).await.ok()?;
    if len[0] == 0 {
        return None;
    }
    let mut name = vec![0u8; usize::from(len[0])];
    recv.read_exact(&mut name).await.ok()?;
    String::from_utf8(name).ok()
}

/// A stream of either transport, as one byte pipe.
trait Duplex: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Duplex for T {}

/// Carry bytes both ways between a TCP connection and a stream until both
/// directions have ended. Each side's end is passed on as the other's
/// (TCP FIN, QUIC FIN, HTTP/2 END_STREAM), so a client that half-closes
/// still gets its answer.
async fn splice(tcp: TcpStream, mut stream: impl Duplex) {
    let _ = tcp.set_nodelay(true);
    let mut tcp = tcp;
    let _ = tokio::io::copy_bidirectional(&mut tcp, &mut stream).await;
}

// ── TCP fallback ────────────────────────────────────────────────────────
//
// Some networks pass no UDP at all (an office firewall, a hotel's). There
// the client reaches the same port over TCP instead: TLS 1.3 with the same
// certificate and ALPN, then the same hello and welcome, each as a 4-byte
// length and the JSON, then HTTP/2 over the rest of the connection. The edge
// is HTTP/2's client there, because it is the side that opens streams; each
// stream is one request whose body carries the header and the visitor's
// bytes, and whose response body carries the box's. HTTP/2 shares one TCP
// connection, so a lost packet stalls every stream on it, which is the price
// of getting through at all; the client keeps trying QUIC and moves back
// when it works.

/// How long the client gives QUIC before it tries TCP.
const QUIC_TRY: Duration = Duration::from_secs(5);

/// How often a client on TCP tries QUIC again.
const QUIC_RETRY: Duration = Duration::from_secs(300);

/// How long a link another one took over keeps carrying the streams it
/// already had, before it is closed.
const GRACE: Duration = Duration::from_secs(30);

/// How often the client pings the edge on TCP, and how long it waits for
/// the answer before it calls the link dead. QUIC does this itself.
const TCP_PING: Duration = Duration::from_secs(10);
const TCP_PING_WAIT: Duration = Duration::from_secs(20);

/// Per-stream and per-connection HTTP/2 windows. The defaults (64 KiB) cap
/// one download at 64 KiB per round trip, a few hundred KiB/s to a box an
/// ocean away.
const H2_STREAM_WINDOW: u32 = 4 << 20;
const H2_CONN_WINDOW: u32 = 32 << 20;

fn io_err(e: h2::Error) -> std::io::Error {
    if e.is_io() {
        e.into_io()
            .unwrap_or_else(|| std::io::ErrorKind::Other.into())
    } else {
        std::io::Error::other(e)
    }
}

async fn write_frame<W: AsyncWrite + Unpin>(w: &mut W, body: &[u8]) -> std::io::Result<()> {
    let len = u32::try_from(body.len()).map_err(std::io::Error::other)?;
    w.write_all(&len.to_be_bytes()).await?;
    w.write_all(body).await?;
    w.flush().await
}

async fn read_frame<R: AsyncRead + Unpin>(r: &mut R) -> std::io::Result<Vec<u8>> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len).await?;
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_HELLO {
        return Err(std::io::Error::other("frame too large"));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body).await?;
    Ok(body)
}

/// Where the read half of an HTTP/2 stream is: the edge's waits for the
/// box's response headers first.
enum H2Recv {
    Waiting(h2::client::ResponseFuture),
    Open(h2::RecvStream),
    Done,
}

/// One HTTP/2 stream as a byte pipe, with HTTP/2's flow control honoured
/// both ways.
struct H2Io {
    send: h2::SendStream<Bytes>,
    recv: H2Recv,
    /// What the last DATA frame held beyond what the reader took.
    rest: Bytes,
    ended: bool,
}

impl H2Io {
    fn new(send: h2::SendStream<Bytes>, recv: H2Recv) -> Self {
        Self {
            send,
            recv,
            rest: Bytes::new(),
            ended: false,
        }
    }
}

impl AsyncRead for H2Io {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = &mut *self;
        loop {
            if !this.rest.is_empty() {
                let n = this.rest.len().min(buf.remaining());
                buf.put_slice(&this.rest.split_to(n));
                return Poll::Ready(Ok(()));
            }
            match &mut this.recv {
                H2Recv::Done => return Poll::Ready(Ok(())),
                H2Recv::Waiting(fut) => match ready!(Pin::new(fut).poll(cx)) {
                    Ok(resp) => this.recv = H2Recv::Open(resp.into_body()),
                    Err(e) => {
                        this.recv = H2Recv::Done;
                        return Poll::Ready(Err(io_err(e)));
                    }
                },
                H2Recv::Open(body) => match ready!(body.poll_data(cx)) {
                    Some(Ok(data)) => {
                        let _ = body.flow_control().release_capacity(data.len());
                        this.rest = data;
                    }
                    Some(Err(e)) => {
                        this.recv = H2Recv::Done;
                        return Poll::Ready(Err(io_err(e)));
                    }
                    None => this.recv = H2Recv::Done,
                },
            }
        }
    }
}

impl AsyncWrite for H2Io {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }
        let this = &mut *self;
        this.send.reserve_capacity(buf.len());
        loop {
            let room = this.send.capacity();
            if room > 0 {
                let n = room.min(buf.len());
                this.send
                    .send_data(Bytes::copy_from_slice(&buf[..n]), false)
                    .map_err(io_err)?;
                return Poll::Ready(Ok(n));
            }
            match ready!(this.send.poll_capacity(cx)) {
                Some(Ok(_)) => {}
                Some(Err(e)) => return Poll::Ready(Err(io_err(e))),
                None => return Poll::Ready(Err(std::io::ErrorKind::BrokenPipe.into())),
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        if !self.ended {
            self.ended = true;
            // A stream the peer already reset has nothing left to end.
            let _ = self.send.send_data(Bytes::new(), true);
        }
        Poll::Ready(Ok(()))
    }
}

fn tls_server(id: &Identity) -> Result<TlsAcceptor> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut tls = rustls::ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .into_diagnostic()?
        .with_no_client_auth()
        .with_single_cert(
            vec![id.cert.clone()],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(id.key.clone())),
        )
        .into_diagnostic()?;
    tls.alpn_protocols = vec![ALPN.to_vec()];
    Ok(TlsAcceptor::from(Arc::new(tls)))
}

fn tls_client(pin: [u8; 32]) -> Result<TlsConnector> {
    Ok(TlsConnector::from(Arc::new(pinned_tls(pin)?)))
}

// ── Server ──────────────────────────────────────────────────────────────

/// What `tunnel-server` was given.
#[derive(Debug, Clone)]
pub struct ServerOpts {
    /// Where boxes reach the tunnel: this UDP port for QUIC, and the same
    /// TCP port for the fallback.
    pub listen: SocketAddr,
    /// Where the certificate lives ([`load_or_make_identity`]).
    pub identity_dir: PathBuf,
    /// The registrar's [`ServerFile`].
    pub config: PathBuf,
}

/// A box's connection, either transport.
#[derive(Clone)]
enum Link {
    Quic(quinn::Connection),
    Tcp(Arc<TcpLink>),
}

struct TcpLink {
    peer: SocketAddr,
    opener: h2::client::SendRequest<Bytes>,
    /// The task driving the HTTP/2 connection; aborting it closes the link.
    driver: tokio::task::AbortHandle,
    /// Turns true, or loses its sender, when the connection has ended.
    done: tokio::sync::watch::Receiver<bool>,
}

impl Link {
    fn same(&self, other: &Link) -> bool {
        match (self, other) {
            (Link::Quic(a), Link::Quic(b)) => a.stable_id() == b.stable_id(),
            (Link::Tcp(a), Link::Tcp(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }

    fn peer(&self) -> String {
        match self {
            Link::Quic(c) => format!("{} (quic)", c.remote_address()),
            Link::Tcp(t) => format!("{} (tcp)", t.peer),
        }
    }

    fn close(&self, why: &'static [u8]) {
        match self {
            Link::Quic(c) => c.close(2u32.into(), why),
            Link::Tcp(t) => t.driver.abort(),
        }
    }

    async fn closed(&self) {
        match self {
            Link::Quic(c) => {
                c.closed().await;
            }
            Link::Tcp(t) => {
                let mut done = t.done.clone();
                while !*done.borrow_and_update() {
                    if done.changed().await.is_err() {
                        break;
                    }
                }
            }
        }
    }

    /// A new stream to the box, its header already written.
    async fn open(&self, name: &str) -> Option<Box<dyn Duplex>> {
        let front = Bytes::from(header(name)?);
        match self {
            Link::Quic(conn) => {
                let (mut send, recv) = conn.open_bi().await.ok()?;
                send.write_all(&front).await.ok()?;
                Some(Box::new(tokio::io::join(recv, send)))
            }
            Link::Tcp(t) => {
                let mut opener = t.opener.clone().ready().await.ok()?;
                let req = http::Request::post("https://losos-tunnel/").body(()).ok()?;
                let (resp, mut send) = opener.send_request(req, false).ok()?;
                send.send_data(front, false).ok()?;
                Some(Box::new(H2Io::new(send, H2Recv::Waiting(resp))))
            }
        }
    }
}

/// One service on this edge: its token, its port, and the link that holds
/// it now.
struct Service {
    token: String,
    bind: SocketAddr,
    listener: tokio::task::AbortHandle,
    holder: Option<Link>,
}

type Services = Arc<Mutex<HashMap<String, Service>>>;

/// The edge's sockets: QUIC on a UDP port and the fallback on the same TCP
/// port.
pub struct Bound {
    quic: Option<quinn::Endpoint>,
    tcp: TcpListener,
    tls: TlsAcceptor,
}

impl Bound {
    /// The address both listen on.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.tcp.local_addr().into_diagnostic()
    }

    /// Stop answering QUIC, as on a network that drops UDP. For tests.
    pub fn without_quic(mut self) -> Self {
        if let Some(e) = self.quic.take() {
            e.close(0u32.into(), b"");
        }
        self
    }
}

/// Run the edge's side until the endpoint fails.
pub async fn serve(opts: ServerOpts) -> Result<()> {
    let id = load_or_make_identity(&opts.identity_dir)?;
    let bound = bind_server(opts.listen, &id)?;
    tracing::info!(
        "tunnel on udp and tcp {}, certificate {}",
        opts.listen,
        id.pin()
    );
    serve_on(bound, opts.config).await
}

/// The edge's sockets on `listen`, presenting `id`. Port 0 picks one port
/// free for both.
pub fn bind_server(listen: SocketAddr, id: &Identity) -> Result<Bound> {
    let tls = tls_server(id)?;
    let mut tries = 0;
    loop {
        let quic = quinn::Endpoint::server(server_config(id)?, listen)
            .into_diagnostic()
            .wrap_err_with(|| format!("bind the tunnel on udp {listen}"))?;
        let port = quic.local_addr().into_diagnostic()?.port();
        let at = SocketAddr::new(listen.ip(), port);
        match std::net::TcpListener::bind(at) {
            Ok(tcp) => {
                tcp.set_nonblocking(true).into_diagnostic()?;
                return Ok(Bound {
                    quic: Some(quic),
                    tcp: TcpListener::from_std(tcp).into_diagnostic()?,
                    tls,
                });
            }
            // The UDP port was free and the same TCP port is not: pick again.
            Err(_) if listen.port() == 0 && tries < 20 => tries += 1,
            Err(e) => {
                return Err(e)
                    .into_diagnostic()
                    .wrap_err_with(|| format!("bind the tunnel on tcp {at}"))
            }
        }
    }
}

/// [`serve`] on sockets already bound, for tests.
pub async fn serve_on(bound: Bound, config: PathBuf) -> Result<()> {
    let services: Services = Arc::default();
    tokio::spawn(reload_loop(config, services.clone()));
    tokio::spawn(accept_tcp(bound.tcp, bound.tls, services.clone()));
    let Some(endpoint) = bound.quic else {
        std::future::pending::<()>().await;
        unreachable!()
    };
    while let Some(incoming) = endpoint.accept().await {
        let services = services.clone();
        tokio::spawn(async move {
            let conn = match incoming.await {
                Ok(c) => c,
                Err(e) => {
                    tracing::debug!("tunnel handshake: {e}");
                    return;
                }
            };
            let peer = conn.remote_address();
            if let Err(e) = serve_quic(&services, conn).await {
                tracing::info!("tunnel, {peer}: {e}");
            }
        });
    }
    Err(miette!("the tunnel's endpoint closed"))
}

/// Re-read the file every second and bring the listeners in line with it.
async fn reload_loop(config: PathBuf, services: Services) {
    let mut last: Option<Vec<u8>> = None;
    loop {
        let bytes = match tokio::fs::read(&config).await {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => b"{}".to_vec(),
            Err(e) => {
                tracing::warn!("read {}: {e}", config.display());
                tokio::time::sleep(SERVER_RELOAD).await;
                continue;
            }
        };
        if last.as_ref() != Some(&bytes) {
            match serde_json::from_slice::<ServerFile>(&bytes) {
                Ok(file) => {
                    apply(&services, file);
                    last = Some(bytes);
                }
                Err(e) => tracing::warn!("{} is not a tunnel file: {e}", config.display()),
            }
        }
        tokio::time::sleep(SERVER_RELOAD).await;
    }
}

/// Bring the services in line with `file`: new ones get a listener, gone
/// ones lose theirs, and a changed token or port drops the holder, which
/// must prove the new token on its next connection.
fn apply(services: &Services, file: ServerFile) {
    let mut map = services.lock().expect("services");
    map.retain(|name, s| {
        let keep = file
            .services
            .get(name)
            .is_some_and(|f| f.token == s.token && f.bind == s.bind);
        if !keep {
            s.listener.abort();
            tracing::info!("tunnel: {name} is gone or changed");
        }
        keep
    });
    for (name, f) in file.services {
        if map.contains_key(&name) || header(&name).is_none() {
            continue;
        }
        let listener = tokio::spawn(listen(name.clone(), f.bind, services.clone())).abort_handle();
        map.insert(
            name,
            Service {
                token: f.token,
                bind: f.bind,
                listener,
                holder: None,
            },
        );
    }
}

/// Take TCP connections on a service's port and hand each to its holder.
async fn listen(name: String, bind: SocketAddr, services: Services) {
    let listener = loop {
        match TcpListener::bind(bind).await {
            Ok(l) => break l,
            Err(e) => {
                tracing::warn!("tunnel: cannot bind {bind} for {name}: {e}");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    };
    loop {
        let Ok((tcp, _)) = listener.accept().await else {
            continue;
        };
        let holder = services
            .lock()
            .expect("services")
            .get(&name)
            .and_then(|s| s.holder.clone());
        // Nobody holds the service: the connection closes, and Traefik
        // answers 502 as it would for a box that is down.
        let Some(link) = holder else {
            continue;
        };
        let name = name.clone();
        tokio::spawn(async move {
            if let Some(stream) = link.open(&name).await {
                splice(tcp, stream).await;
            }
        });
    }
}

/// Read a QUIC client's hello, hand it the services it proves, and hold
/// them until the connection goes.
async fn serve_quic(services: &Services, conn: quinn::Connection) -> Result<()> {
    let (mut send, mut recv) = tokio::time::timeout(HANDSHAKE, conn.accept_bi())
        .await
        .into_diagnostic()?
        .into_diagnostic()?;
    let hello = tokio::time::timeout(HANDSHAKE, recv.read_to_end(MAX_HELLO))
        .await
        .into_diagnostic()?
        .into_diagnostic()?;
    let hello = match parse_hello(&hello) {
        Ok(h) => h,
        Err(why) => {
            conn.close(1u32.into(), why.as_bytes());
            return Err(miette!("{why}"));
        }
    };
    let welcome = verify(services, &hello);
    let link = Link::Quic(conn.clone());
    take(services, &link, &welcome.accepted);
    send.write_all(&serde_json::to_vec(&welcome).into_diagnostic()?)
        .await
        .into_diagnostic()?;
    send.finish().into_diagnostic()?;
    if welcome.accepted.is_empty() {
        let _ = send.stopped().await;
        conn.close(1u32.into(), b"unauthorized");
        return Err(miette!("refused {:?}", welcome.refused));
    }
    hold(services, link, &welcome).await;
    Ok(())
}

/// Take TLS connections on the TCP port: the fallback.
async fn accept_tcp(listener: TcpListener, tls: TlsAcceptor, services: Services) {
    loop {
        let Ok((tcp, peer)) = listener.accept().await else {
            continue;
        };
        let (tls, services) = (tls.clone(), services.clone());
        tokio::spawn(async move {
            if let Err(e) = serve_tcp(&services, tcp, peer, tls).await {
                tracing::info!("tunnel, {peer} (tcp): {e}");
            }
        });
    }
}

async fn serve_tcp(
    services: &Services,
    tcp: TcpStream,
    peer: SocketAddr,
    tls: TlsAcceptor,
) -> Result<()> {
    let _ = tcp.set_nodelay(true);
    let mut stream = tokio::time::timeout(HANDSHAKE, tls.accept(tcp))
        .await
        .into_diagnostic()?
        .into_diagnostic()?;
    let hello = tokio::time::timeout(HANDSHAKE, read_frame(&mut stream))
        .await
        .into_diagnostic()?
        .into_diagnostic()?;
    let hello = parse_hello(&hello).map_err(|why| miette!("{why}"))?;
    let welcome = verify(services, &hello);
    write_frame(
        &mut stream,
        &serde_json::to_vec(&welcome).into_diagnostic()?,
    )
    .await
    .into_diagnostic()?;
    if welcome.accepted.is_empty() {
        let _ = stream.shutdown().await;
        return Err(miette!("refused {:?}", welcome.refused));
    }
    let (opener, conn) = h2::client::Builder::new()
        .initial_window_size(H2_STREAM_WINDOW)
        .initial_connection_window_size(H2_CONN_WINDOW)
        .handshake::<_, Bytes>(stream)
        .await
        .into_diagnostic()?;
    let (done_tx, done) = tokio::sync::watch::channel(false);
    let driver = tokio::spawn(async move {
        let _ = conn.await;
        let _ = done_tx.send(true);
    })
    .abort_handle();
    let link = Link::Tcp(Arc::new(TcpLink {
        peer,
        opener,
        driver,
        done,
    }));
    take(services, &link, &welcome.accepted);
    hold(services, link, &welcome).await;
    Ok(())
}

fn parse_hello(bytes: &[u8]) -> std::result::Result<Hello, &'static str> {
    let hello = serde_json::from_slice::<Hello>(bytes).map_err(|_| "bad hello")?;
    if hello.services.len() > MAX_SERVICES {
        return Err("too many services");
    }
    Ok(hello)
}

/// Which services in `hello` carry their token.
fn verify(services: &Services, hello: &Hello) -> Welcome {
    let map = services.lock().expect("services");
    let mut welcome = Welcome::default();
    for c in &hello.services {
        let ok = map.get(&c.name).is_some_and(|s| {
            subtle::ConstantTimeEq::ct_eq(s.token.as_bytes(), c.token.as_bytes()).into()
        });
        if ok {
            welcome.accepted.push(c.name.clone());
        } else {
            welcome.refused.push(c.name.clone());
        }
    }
    welcome
}

/// Make `link` the holder of `names`. A link that loses its last service to
/// a newer one is the same box's old link, and the box has moved on; it
/// keeps the streams it carries for [`GRACE`] and is then closed.
fn take(services: &Services, link: &Link, names: &[String]) {
    let mut displaced: Vec<Link> = Vec::new();
    {
        let mut map = services.lock().expect("services");
        for name in names {
            let Some(s) = map.get_mut(name) else {
                continue;
            };
            if let Some(old) = s.holder.replace(link.clone()) {
                if !old.same(link) && !displaced.iter().any(|d| d.same(&old)) {
                    displaced.push(old);
                }
            }
        }
    }
    for old in displaced {
        let services = services.clone();
        tokio::spawn(async move {
            tokio::time::sleep(GRACE).await;
            let still = services
                .lock()
                .expect("services")
                .values()
                .any(|s| s.holder.as_ref().is_some_and(|h| h.same(&old)));
            if !still {
                old.close(b"replaced");
            }
        });
    }
}

/// Hold `link`'s services until it goes, then let go of them.
async fn hold(services: &Services, link: Link, welcome: &Welcome) {
    tracing::info!(
        "tunnel: {} holds {}",
        link.peer(),
        welcome.accepted.join(", ")
    );
    link.closed().await;
    let mut map = services.lock().expect("services");
    for s in map.values_mut() {
        if s.holder.as_ref().is_some_and(|h| h.same(&link)) {
            s.holder = None;
        }
    }
    drop(map);
    tracing::info!("tunnel: {} left", link.peer());
}

// ── Client ──────────────────────────────────────────────────────────────

/// A client's live link to its edge: the task carrying the edge's streams,
/// and how to close it.
pub struct Up {
    task: tokio::task::JoinHandle<String>,
    quic: Option<(quinn::Endpoint, quinn::Connection)>,
}

impl std::fmt::Debug for Up {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.is_tcp() { "Up(tcp)" } else { "Up(quic)" })
    }
}

impl Up {
    /// Whether this link is the TCP fallback.
    pub fn is_tcp(&self) -> bool {
        self.quic.is_none()
    }

    async fn close(self) {
        match self.quic {
            Some((endpoint, conn)) => {
                conn.close(0u32.into(), b"reconfigured");
                endpoint.wait_idle().await;
            }
            None => self.task.abort(),
        }
    }
}

/// Keep a link to the edge in `config` for as long as this runs: reconnect
/// when it drops, with a backoff up to half a minute, and reconnect at once
/// when the file changes. On TCP, try QUIC every five minutes and move back
/// to it when it works.
pub async fn run_client(config: PathBuf) -> Result<()> {
    let mut backoff = Duration::from_secs(1);
    'connect: loop {
        let (file, bytes) = match read_client_file(&config).await {
            Ok(x) => x,
            Err(e) => {
                tracing::warn!("{e:?}");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };
        match connect(&file).await {
            Ok(mut up) => {
                backoff = Duration::from_secs(1);
                loop {
                    tokio::select! {
                        why = &mut up.task => {
                            let why = why.unwrap_or_else(|e| e.to_string());
                            tracing::warn!("tunnel to {} dropped: {why}", file.remote);
                            break;
                        }
                        () = changed(&config, &bytes) => {
                            tracing::info!("{} changed; reconnecting", config.display());
                            up.close().await;
                            continue 'connect;
                        }
                        quic = quic_returns(&file), if up.is_tcp() => {
                            tracing::info!("QUIC gets through to {} again; moving the tunnel back", file.remote);
                            let old = std::mem::replace(&mut up, quic);
                            tokio::spawn(async move {
                                tokio::time::sleep(GRACE).await;
                                old.close().await;
                            });
                        }
                    }
                }
            }
            Err(e) => tracing::warn!("tunnel to {}: {e:?}", file.remote),
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }
}

async fn quic_returns(file: &ClientFile) -> Up {
    loop {
        tokio::time::sleep(QUIC_RETRY).await;
        if let Ok(Ok(up)) = tokio::time::timeout(QUIC_TRY, connect_quic(file)).await {
            return up;
        }
    }
}

async fn read_client_file(path: &Path) -> Result<(ClientFile, Vec<u8>)> {
    let bytes = tokio::fs::read(path)
        .await
        .into_diagnostic()
        .wrap_err_with(|| format!("read {}", path.display()))?;
    let file = serde_json::from_slice(&bytes)
        .into_diagnostic()
        .wrap_err_with(|| format!("{} is not a tunnel client file", path.display()))?;
    Ok((file, bytes))
}

/// Resolves once the file at `path` no longer holds `bytes`.
async fn changed(path: &Path, bytes: &[u8]) {
    loop {
        tokio::time::sleep(CLIENT_RELOAD).await;
        match tokio::fs::read(path).await {
            Ok(now) if now == bytes => {}
            Ok(_) => return,
            // A missing file is a reconfiguration in progress, not a change.
            Err(_) => {}
        }
    }
}

/// The client's side of an edge that answered: QUIC refused us, so TCP
/// would be refused too.
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("the edge took none of this side's services")]
struct Refused;

/// Dial the edge, over QUIC or else over TCP, say the hello, and return the
/// link once the edge took at least one service.
pub async fn connect(file: &ClientFile) -> Result<Up> {
    if file.services.len() > MAX_SERVICES {
        return Err(miette!("more than {MAX_SERVICES} services"));
    }
    let quic = match tokio::time::timeout(QUIC_TRY, connect_quic(file)).await {
        Ok(Ok(up)) => return Ok(up),
        Ok(Err(e)) if e.downcast_ref::<Refused>().is_some() => return Err(e),
        Ok(Err(e)) => e,
        Err(_) => miette!("no QUIC answer within {}s", QUIC_TRY.as_secs()),
    };
    match connect_tcp(file).await {
        Ok(up) => {
            tracing::info!("QUIC to {} failed ({quic}); on TCP", file.remote);
            Ok(up)
        }
        Err(tcp) => Err(miette!("QUIC: {quic:?}\nTCP: {tcp:?}")),
    }
}

fn pin_of_file(file: &ClientFile) -> Result<[u8; 32]> {
    parse_pin(&file.pin).ok_or_else(|| miette!("the pin is not 64 hex characters"))
}

async fn resolve(file: &ClientFile) -> Result<SocketAddr> {
    tokio::net::lookup_host(&file.remote)
        .await
        .into_diagnostic()
        .wrap_err_with(|| format!("resolve {}", file.remote))?
        .next()
        .ok_or_else(|| miette!("{} has no address", file.remote))
}

fn hello_of(file: &ClientFile) -> Hello {
    Hello {
        services: file
            .services
            .iter()
            .map(|(name, s)| Claim {
                name: name.clone(),
                token: s.token.clone(),
            })
            .collect(),
    }
}

fn welcomed(answer: &[u8], target: SocketAddr, how: &str) -> Result<()> {
    let welcome: Welcome = serde_json::from_slice(answer)
        .into_diagnostic()
        .wrap_err("the edge's answer")?;
    for name in &welcome.refused {
        tracing::warn!("the edge refused {name}: unknown service or wrong token");
    }
    if welcome.accepted.is_empty() {
        return Err(Refused.into());
    }
    tracing::info!(
        "tunnel to {target} over {how} carries {}",
        welcome.accepted.join(", ")
    );
    Ok(())
}

async fn connect_quic(file: &ClientFile) -> Result<Up> {
    let pin = pin_of_file(file)?;
    let target = resolve(file).await?;
    let bind: SocketAddr = if target.is_ipv6() {
        (Ipv6Addr::UNSPECIFIED, 0).into()
    } else {
        (Ipv4Addr::UNSPECIFIED, 0).into()
    };
    let mut endpoint = quinn::Endpoint::client(bind).into_diagnostic()?;
    endpoint.set_default_client_config(client_config(pin)?);
    let conn = endpoint
        .connect(target, SERVER_NAME)
        .into_diagnostic()?
        .await
        .into_diagnostic()
        .wrap_err_with(|| format!("QUIC to {target}"))?;

    let (mut send, mut recv) = conn.open_bi().await.into_diagnostic()?;
    send.write_all(&serde_json::to_vec(&hello_of(file)).into_diagnostic()?)
        .await
        .into_diagnostic()?;
    send.finish().into_diagnostic()?;
    let answer = tokio::time::timeout(HANDSHAKE, recv.read_to_end(MAX_HELLO))
        .await
        .into_diagnostic()
        .wrap_err("the edge did not answer")?
        .into_diagnostic()
        .wrap_err("the edge closed the tunnel")?;
    welcomed(&answer, target, "QUIC")?;
    let services = Arc::new(file.services.clone());
    let task = tokio::spawn({
        let conn = conn.clone();
        async move { quic_streams(&conn, services).await.to_string() }
    });
    Ok(Up {
        task,
        quic: Some((endpoint, conn)),
    })
}

async fn connect_tcp(file: &ClientFile) -> Result<Up> {
    let pin = pin_of_file(file)?;
    let target = resolve(file).await?;
    let tcp = tokio::time::timeout(HANDSHAKE, TcpStream::connect(target))
        .await
        .into_diagnostic()
        .wrap_err_with(|| format!("TCP to {target} timed out"))?
        .into_diagnostic()
        .wrap_err_with(|| format!("TCP to {target}"))?;
    let _ = tcp.set_nodelay(true);
    let name = ServerName::try_from(SERVER_NAME).into_diagnostic()?;
    let mut stream = tokio::time::timeout(HANDSHAKE, tls_client(pin)?.connect(name, tcp))
        .await
        .into_diagnostic()
        .wrap_err("TLS timed out")?
        .into_diagnostic()
        .wrap_err_with(|| format!("TLS to {target}"))?;
    write_frame(
        &mut stream,
        &serde_json::to_vec(&hello_of(file)).into_diagnostic()?,
    )
    .await
    .into_diagnostic()?;
    let answer = tokio::time::timeout(HANDSHAKE, read_frame(&mut stream))
        .await
        .into_diagnostic()
        .wrap_err("the edge did not answer")?
        .into_diagnostic()
        .wrap_err("the edge closed the tunnel")?;
    welcomed(&answer, target, "TCP")?;
    let conn = h2::server::Builder::new()
        .initial_window_size(H2_STREAM_WINDOW)
        .initial_connection_window_size(H2_CONN_WINDOW)
        .max_concurrent_streams(MAX_STREAMS)
        .handshake::<_, Bytes>(stream)
        .await
        .into_diagnostic()?;
    let services = Arc::new(file.services.clone());
    let task = tokio::spawn(h2_streams(conn, services));
    Ok(Up { task, quic: None })
}

/// Deliver a stream from the edge to its service's local address.
async fn deliver(mut stream: impl Duplex, services: Arc<BTreeMap<String, ClientService>>) {
    let name = match tokio::time::timeout(HANDSHAKE, read_header(&mut stream)).await {
        Ok(Some(n)) => n,
        _ => return,
    };
    let Some(s) = services.get(&name) else {
        tracing::warn!("the edge opened a stream for {name}, which this side does not serve");
        return;
    };
    match tokio::time::timeout(HANDSHAKE, TcpStream::connect(&s.local)).await {
        Ok(Ok(tcp)) => splice(tcp, stream).await,
        Ok(Err(e)) => tracing::warn!("{name}: connect {}: {e}", s.local),
        Err(_) => tracing::warn!("{name}: connect {} timed out", s.local),
    }
}

/// Every stream the edge opens on QUIC, until the connection goes.
async fn quic_streams(
    conn: &quinn::Connection,
    services: Arc<BTreeMap<String, ClientService>>,
) -> quinn::ConnectionError {
    loop {
        let (send, recv) = match conn.accept_bi().await {
            Ok(s) => s,
            Err(e) => return e,
        };
        tokio::spawn(deliver(tokio::io::join(recv, send), services.clone()));
    }
}

/// Every stream the edge opens on TCP, until the connection goes or stops
/// answering pings.
async fn h2_streams(
    mut conn: h2::server::Connection<TlsClientStream<TcpStream>, Bytes>,
    services: Arc<BTreeMap<String, ClientService>>,
) -> String {
    let mut pings = conn.ping_pong();
    let pinger = async move {
        let Some(pings) = pings.as_mut() else {
            return "no ping handle".to_string();
        };
        loop {
            tokio::time::sleep(TCP_PING).await;
            match tokio::time::timeout(TCP_PING_WAIT, pings.ping(h2::Ping::opaque())).await {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => return e.to_string(),
                Err(_) => return "the edge stopped answering".to_string(),
            }
        }
    };
    let accept = async {
        while let Some(next) = conn.accept().await {
            let (req, mut respond) = match next {
                Ok(x) => x,
                Err(e) => return e.to_string(),
            };
            let Ok(send) = respond.send_response(http::Response::new(()), false) else {
                continue;
            };
            let io = H2Io::new(send, H2Recv::Open(req.into_body()));
            tokio::spawn(deliver(io, services.clone()));
        }
        "the edge closed the tunnel".to_string()
    };
    tokio::select! {
        why = accept => why,
        why = pinger => why,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers_carry_names_up_to_255_bytes() {
        assert_eq!(header("a").unwrap(), b"\x01a");
        assert!(header("").is_none());
        assert!(header(&"x".repeat(256)).is_none());
        assert_eq!(header(&"x".repeat(255)).unwrap().len(), 256);
    }

    #[test]
    fn pins_are_64_hex_characters() {
        let hex = "00ff".repeat(16);
        assert_eq!(parse_pin(&hex).unwrap()[..2], [0x00, 0xff]);
        assert_eq!(parse_pin(&format!("{hex}\n")).unwrap()[1], 0xff);
        assert!(parse_pin("abc").is_none());
        assert!(parse_pin(&"zz".repeat(32)).is_none());
    }

    #[test]
    fn the_identity_is_made_once_and_kept() {
        let dir = std::env::temp_dir().join(format!("losos-tunnel-id-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let first = load_or_make_identity(&dir).unwrap().pin();
        let again = load_or_make_identity(&dir).unwrap().pin();
        assert_eq!(first, again);
        let written = std::fs::read_to_string(dir.join("pin")).unwrap();
        assert_eq!(written.trim(), first);
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.join("key.der"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
