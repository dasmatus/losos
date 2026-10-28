//! The edge as a way out to the internet (`losos.edge.exit.*`,
//! `modules/edge-exit.nix`, `handbook/docs/in-depth/edge-exit.md`).
//!
//! A tenant the operator lets out (`losos.edge.tenants.<id>.exit`) enrols a
//! WireGuard key with `POST /exit` and gets an address on the edge's
//! `losos-exit` interface back. Its traffic then leaves from the edge's
//! public address, the way a VPN's does. LosOS Desktop's network machine is
//! the caller: it picks an edge the way a box does (`losos-edge-path`) and
//! sends the computer's traffic through the one it picked.
//!
//! The same enrolment opens a second door for a computer whose traffic goes
//! through Tor first: a SOCKS5 server on the edge's own onion service
//! (`exit-socks`), with the tenant's id and a password the enrolment hands
//! out. Tor carries the connection to the edge and the edge makes it, so the
//! site sees the edge and the edge sees only Tor.
//!
//! The registrar keeps no WireGuard state. It writes the enrolled peers to
//! `<dir>/peers.json`; `losos-exit-sync` (root) puts them on the interface,
//! and `exit-socks` reads the same file for the passwords, which are kept as
//! SHA-256 digests only.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use rand::RngCore as _;
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq as _;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

use crate::action::Action;

/// The file the registrar writes in `--exit-dir`.
pub const PEERS_FILE: &str = "peers.json";

/// `serve --exit-dir DIR --exit-subnet CIDR --exit-endpoint HOST:PORT
/// --exit-public-key-file FILE [--exit-onion-file FILE] [--exit-onion-port N]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitOpts {
    pub dir: String,
    /// The interface's network; the edge is its first address.
    pub subnet: (Ipv4Addr, u8),
    /// Where a peer dials the edge's WireGuard port, as the peer writes it in
    /// its `Endpoint =`: the edge's public name or address.
    pub endpoint: String,
    /// `wg pubkey` of the interface's private key, which the registrar never
    /// sees.
    pub public_key_file: String,
    /// Tor's `hostname` file for the onion service `exit-socks` answers on;
    /// `None` when the edge offers no exit over Tor.
    pub onion_file: Option<String>,
    /// The port on that onion address.
    pub onion_port: u16,
}

/// One enrolled computer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Peer {
    /// Its WireGuard public key, base64.
    pub public_key: String,
    /// Its one address on the interface.
    pub address: Ipv4Addr,
    /// SHA-256 of its SOCKS password, hex.
    pub secret_sha256: String,
}

/// `peers.json`: tenant id → peer.
pub type Peers = BTreeMap<String, Peer>;

#[derive(Debug, thiserror::Error)]
pub enum ExitError {
    /// The edge offers no exit, or this tenant may not use it.
    #[error("this edge is no exit for this id")]
    Unconfigured,
    #[error("{0}")]
    Invalid(&'static str),
    /// Every address of the subnet is taken.
    #[error("no address left on this exit")]
    Full,
    #[error("exit store: {0}")]
    Store(String),
}

impl From<std::io::Error> for ExitError {
    fn from(e: std::io::Error) -> Self {
        ExitError::Store(e.to_string())
    }
}

impl From<serde_json::Error> for ExitError {
    fn from(e: serde_json::Error) -> Self {
        ExitError::Store(e.to_string())
    }
}

/// What a computer needs to use the exit, as `POST /exit` answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Enrolled {
    /// Its address, with the subnet's prefix for the interface.
    pub address: String,
    pub server_public_key: String,
    pub endpoint: String,
    /// What to route into the tunnel: everything.
    pub allowed_ips: Vec<String>,
    /// The edge's onion address and port for the SOCKS5 exit, when it has
    /// one, and the credentials it asks for there.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onion: Option<String>,
    pub onion_port: u16,
    pub socks_username: String,
    pub socks_password: String,
}

/// The exit's store: one writer at a time, through the registrar.
#[derive(Debug)]
pub struct Exit {
    opts: ExitOpts,
    lock: Mutex<()>,
}

impl Exit {
    #[must_use]
    pub fn new(opts: ExitOpts) -> Arc<Self> {
        Arc::new(Exit {
            opts,
            lock: Mutex::new(()),
        })
    }

    fn peers_path(&self) -> PathBuf {
        Path::new(&self.opts.dir).join(PEERS_FILE)
    }

    /// Enrols `id`'s key, or moves it to a new key, keeping its address. A
    /// new SOCKS password every time: the one it had is gone with the
    /// computer that asked again.
    pub async fn enrol(&self, id: &str, public_key: &str) -> Result<Enrolled, ExitError> {
        if !wireguard_key(public_key) {
            return Err(ExitError::Invalid("public_key is not a WireGuard key"));
        }
        let server_public_key = tokio::fs::read_to_string(&self.opts.public_key_file)
            .await
            .map_err(|e| ExitError::Store(format!("{}: {e}", self.opts.public_key_file)))?
            .trim()
            .to_string();
        if !wireguard_key(&server_public_key) {
            return Err(ExitError::Store(format!(
                "{} holds no WireGuard key",
                self.opts.public_key_file
            )));
        }
        let onion = match &self.opts.onion_file {
            // Tor writes the name once the service is up; until then the
            // exit over Tor is simply not offered yet.
            Some(f) => tokio::fs::read_to_string(f)
                .await
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| s.ends_with(".onion")),
            None => None,
        };

        let mut secret = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut secret);
        let password = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(secret);

        let _guard = self.lock.lock().await;
        let mut peers = load(&self.peers_path()).await?;
        // A key already enrolled for another id would put two ids on one
        // WireGuard peer, and the second would take the first's address.
        if peers
            .iter()
            .any(|(other, p)| other != id && p.public_key == public_key)
        {
            return Err(ExitError::Invalid("public_key is enrolled for another id"));
        }
        let address = match peers.get(id) {
            Some(p) => p.address,
            None => allocate(&peers, self.opts.subnet).ok_or(ExitError::Full)?,
        };
        peers.insert(
            id.to_string(),
            Peer {
                public_key: public_key.to_string(),
                address,
                secret_sha256: sha256_hex(password.as_bytes()),
            },
        );
        store(&self.peers_path(), &peers).await?;
        tracing::info!(target: Action::Exit.target(), "{id} enrolled at {address}");
        Ok(Enrolled {
            address: format!("{address}/32"),
            server_public_key,
            endpoint: self.opts.endpoint.clone(),
            allowed_ips: vec!["0.0.0.0/0".into(), "::/0".into()],
            onion,
            onion_port: self.opts.onion_port,
            socks_username: id.to_string(),
            socks_password: password,
        })
    }

    /// Takes `id` off the exit. Leaving twice is no error.
    pub async fn leave(&self, id: &str) -> Result<(), ExitError> {
        let _guard = self.lock.lock().await;
        let mut peers = load(&self.peers_path()).await?;
        if peers.remove(id).is_some() {
            store(&self.peers_path(), &peers).await?;
            tracing::info!(target: Action::Exit.target(), "{id} left");
        }
        Ok(())
    }
}

/// `peers.json`, or none when it was never written.
pub async fn load(path: &Path) -> Result<Peers, ExitError> {
    match tokio::fs::read(path).await {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Peers::new()),
        Err(e) => Err(e.into()),
    }
}

async fn store(path: &Path, peers: &Peers) -> Result<(), ExitError> {
    let bytes = serde_json::to_vec_pretty(peers)?;
    // The sync unit and exit-socks read it; neither needs more than the
    // registrar's group.
    crate::fsutil::atomic_write(path, &bytes, 0o640).await?;
    Ok(())
}

/// The lowest free address of the subnet, past the edge's own first one.
#[must_use]
pub fn allocate(peers: &Peers, (network, prefix): (Ipv4Addr, u8)) -> Option<Ipv4Addr> {
    let base = u32::from(network);
    let size = 1u64 << (32 - u32::from(prefix.min(30)));
    let taken: std::collections::HashSet<Ipv4Addr> = peers.values().map(|p| p.address).collect();
    // 0 is the network, 1 the edge, size-1 the broadcast.
    (2..size - 1)
        .map(|i| Ipv4Addr::from(base.wrapping_add(i as u32)))
        .find(|a| !taken.contains(a))
}

/// `10.153.0.0/16`, with no host bits set.
pub fn parse_subnet(s: &str) -> Option<(Ipv4Addr, u8)> {
    let (addr, prefix) = s.split_once('/')?;
    let addr: Ipv4Addr = addr.parse().ok()?;
    let prefix: u8 = prefix.parse().ok()?;
    if !(8..=30).contains(&prefix) {
        return None;
    }
    let mask = u32::MAX << (32 - prefix);
    (u32::from(addr) & !mask == 0).then_some((addr, prefix))
}

/// A WireGuard public key: 32 bytes, in standard base64 with padding.
#[must_use]
pub fn wireguard_key(s: &str) -> bool {
    s.len() == 44
        && base64::engine::general_purpose::STANDARD
            .decode(s)
            .is_ok_and(|k| k.len() == 32)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
    digest.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

// ── exit-socks: the exit over Tor ─────────────────────────────────────────

/// `exit-socks --listen ADDR --peers FILE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocksOpts {
    /// Loopback: Tor's onion service delivers here, and nothing else should.
    pub listen: String,
    pub peers_file: String,
}

/// How long a client has to finish its handshake, and a destination to
/// answer.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(60);

/// Serves the SOCKS5 exit until the process is stopped.
pub async fn run_socks(opts: SocksOpts) -> miette::Result<()> {
    let listener = TcpListener::bind(&opts.listen)
        .await
        .map_err(|e| miette::miette!("listening on {}: {e}", opts.listen))?;
    tracing::info!(target: Action::Exit.target(), "SOCKS5 exit on {}", opts.listen);
    let peers_file = Arc::new(PathBuf::from(opts.peers_file));
    loop {
        let (client, _) = match listener.accept().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(target: Action::Exit.target(), "accept: {e}");
                continue;
            }
        };
        let peers_file = peers_file.clone();
        tokio::spawn(async move {
            if let Err(e) = serve_one(client, &peers_file, &public_destination).await {
                tracing::debug!(target: Action::Exit.target(), "connection closed: {e}");
            }
        });
    }
}

/// One SOCKS5 connection: a password from an enrolled peer, then a CONNECT
/// to a public address. `allowed` is the destination rule, a parameter so
/// the tests can reach a listener on loopback.
pub async fn serve_one(
    mut client: TcpStream,
    peers_file: &Path,
    allowed: &(dyn Fn(IpAddr) -> bool + Sync),
) -> std::io::Result<()> {
    let target = tokio::time::timeout(HANDSHAKE_TIMEOUT, async {
        let who = greet(&mut client, peers_file).await?;
        let target = request(&mut client).await?;
        Ok::<_, std::io::Error>((who, target))
    })
    .await
    .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "handshake"))??;
    let (who, target) = target;

    let addrs: Vec<SocketAddr> = match &target {
        Target::Addr(a) => vec![*a],
        Target::Name(host, port) => tokio::net::lookup_host((host.as_str(), *port))
            .await
            .map(Iterator::collect)
            .unwrap_or_default(),
    };
    // Every address a name resolves to must be public, not only the first:
    // a name that answers with one public and one private address must not
    // reach the edge's own network on the second try.
    if addrs.is_empty() {
        reply(&mut client, 4).await?;
        return Ok(());
    }
    if !addrs.iter().all(|a| allowed(a.ip())) {
        tracing::warn!(target: Action::Exit.target(), "{who}: refused {target:?}, not a public address");
        reply(&mut client, 2).await?;
        return Ok(());
    }
    let upstream =
        match tokio::time::timeout(HANDSHAKE_TIMEOUT, TcpStream::connect(&addrs[..])).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                let code = match e.kind() {
                    std::io::ErrorKind::ConnectionRefused => 5,
                    _ => 4,
                };
                reply(&mut client, code).await?;
                return Ok(());
            }
            Err(_) => {
                reply(&mut client, 6).await?;
                return Ok(());
            }
        };
    reply(&mut client, 0).await?;
    let _ = upstream.set_nodelay(true);
    let mut upstream = upstream;
    tokio::io::copy_bidirectional(&mut client, &mut upstream).await?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Target {
    Addr(SocketAddr),
    Name(String, u16),
}

fn refused(msg: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::PermissionDenied, msg)
}

/// RFC 1928's greeting with RFC 1929's username and password, the only
/// method this exit takes. Answers with the tenant id it let in.
async fn greet(client: &mut TcpStream, peers_file: &Path) -> std::io::Result<String> {
    let mut head = [0u8; 2];
    client.read_exact(&mut head).await?;
    if head[0] != 5 {
        return Err(refused("not SOCKS5"));
    }
    let mut methods = vec![0u8; usize::from(head[1])];
    client.read_exact(&mut methods).await?;
    if !methods.contains(&2) {
        client.write_all(&[5, 0xff]).await?;
        return Err(refused("no password offered"));
    }
    client.write_all(&[5, 2]).await?;

    let mut version = [0u8; 1];
    client.read_exact(&mut version).await?;
    let username = field(client).await?;
    let password = field(client).await?;
    // Read for every attempt, so a peer that left or enrolled a new key is
    // let in or out at once. The file is small.
    let peers = load(peers_file).await.unwrap_or_default();
    let ok = version[0] == 1
        && std::str::from_utf8(&username)
            .ok()
            .and_then(|id| peers.get(id))
            .is_some_and(|p| {
                sha256_hex(&password)
                    .as_bytes()
                    .ct_eq(p.secret_sha256.as_bytes())
                    .into()
            });
    client.write_all(&[1, u8::from(!ok)]).await?;
    if !ok {
        return Err(refused("wrong username or password"));
    }
    Ok(String::from_utf8_lossy(&username).into_owned())
}

async fn field(client: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut len = [0u8; 1];
    client.read_exact(&mut len).await?;
    let mut value = vec![0u8; usize::from(len[0])];
    client.read_exact(&mut value).await?;
    Ok(value)
}

/// RFC 1928's request; CONNECT only.
async fn request(client: &mut TcpStream) -> std::io::Result<Target> {
    let mut head = [0u8; 4];
    client.read_exact(&mut head).await?;
    if head[0] != 5 || head[1] != 1 {
        reply(client, 7).await?;
        return Err(refused("not a CONNECT"));
    }
    let target = match head[3] {
        1 => {
            let mut b = [0u8; 6];
            client.read_exact(&mut b).await?;
            let ip = Ipv4Addr::new(b[0], b[1], b[2], b[3]);
            Target::Addr(SocketAddr::new(ip.into(), u16::from_be_bytes([b[4], b[5]])))
        }
        4 => {
            let mut b = [0u8; 18];
            client.read_exact(&mut b).await?;
            let mut ip = [0u8; 16];
            ip.copy_from_slice(&b[..16]);
            Target::Addr(SocketAddr::new(
                Ipv6Addr::from(ip).into(),
                u16::from_be_bytes([b[16], b[17]]),
            ))
        }
        3 => {
            let name = field(client).await?;
            let mut port = [0u8; 2];
            client.read_exact(&mut port).await?;
            let name = String::from_utf8(name).map_err(|_| refused("a name that is not text"))?;
            Target::Name(name, u16::from_be_bytes(port))
        }
        _ => {
            reply(client, 8).await?;
            return Err(refused("unknown address type"));
        }
    };
    Ok(target)
}

async fn reply(client: &mut TcpStream, code: u8) -> std::io::Result<()> {
    client.write_all(&[5, code, 0, 1, 0, 0, 0, 0, 0, 0]).await
}

/// Whether the exit may connect to `ip`: an address on the internet, never
/// the edge itself, its LAN, the exit's own subnet or anything else private.
#[must_use]
pub fn public_destination(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => public_v4(v4),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => public_v4(v4),
            None => {
                let s = v6.segments();
                !(v6.is_loopback()
                    || v6.is_unspecified()
                    || v6.is_multicast()
                    // fc00::/7 unique local, fe80::/10 link-local
                    || (s[0] & 0xfe00) == 0xfc00
                    || (s[0] & 0xffc0) == 0xfe80
                    // 2001:db8::/32 documentation, 64:ff9b::/96 NAT64
                    || (s[0] == 0x2001 && s[1] == 0x0db8)
                    || (s[0] == 0x0064 && s[1] == 0xff9b))
            }
        },
    }
}

fn public_v4(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified()
        || ip.is_multicast()
        || o[0] == 0
        // 100.64.0.0/10, carrier-grade NAT
        || (o[0] == 100 && (o[1] & 0xc0) == 64)
        // 192.0.0.0/24, 198.18.0.0/15, 240.0.0.0/4
        || (o[0] == 192 && o[1] == 0 && o[2] == 0)
        || (o[0] == 198 && (o[1] & 0xfe) == 18)
        || o[0] >= 240)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_A: &str = "YAnzHL7JRWo/x93FMEVY1BOFd8DOFGW8pVf6xLYhJ3M=";
    const KEY_B: &str = "HIgo9xNzJMWLKASShiTqIybxZ0U3wGLiUeJ1PKf8ykw=";

    /// A fresh directory for one test.
    fn scratch() -> PathBuf {
        let mut n = [0u8; 8];
        rand::thread_rng().fill_bytes(&mut n);
        let d = std::env::temp_dir().join(format!("losos-exit-{}", u64::from_le_bytes(n)));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn opts(dir: &Path) -> ExitOpts {
        std::fs::write(dir.join("public.key"), format!("{KEY_B}\n")).unwrap();
        ExitOpts {
            dir: dir.to_string_lossy().into_owned(),
            subnet: parse_subnet("10.153.0.0/29").unwrap(),
            endpoint: "edge.example:51820".into(),
            public_key_file: dir.join("public.key").to_string_lossy().into_owned(),
            onion_file: None,
            onion_port: 1080,
        }
    }

    #[test]
    fn reads_a_subnet() {
        assert_eq!(
            parse_subnet("10.153.0.0/16"),
            Some((Ipv4Addr::new(10, 153, 0, 0), 16))
        );
        assert_eq!(parse_subnet("10.153.0.1/16"), None);
        assert_eq!(parse_subnet("10.153.0.0/31"), None);
        assert_eq!(parse_subnet("10.153.0.0"), None);
    }

    #[test]
    fn knows_a_wireguard_key() {
        assert!(wireguard_key(KEY_A));
        assert!(!wireguard_key("not a key"));
        assert!(!wireguard_key(&KEY_A[..43]));
    }

    #[tokio::test]
    async fn enrols_keeps_an_address_and_runs_out() {
        let dir = scratch();
        let exit = Exit::new(opts(&dir));
        let a = exit.enrol("desk-a", KEY_A).await.unwrap();
        assert_eq!(a.address, "10.153.0.2/32");
        assert_eq!(a.server_public_key, KEY_B);
        assert_eq!(a.socks_username, "desk-a");

        // The same id again, a new key: the same address, a new password.
        let again = exit.enrol("desk-a", KEY_B).await.unwrap();
        assert_eq!(again.address, a.address);
        assert_ne!(again.socks_password, a.socks_password);
        // Another id may not take that key.
        assert!(matches!(
            exit.enrol("desk-b", KEY_B).await,
            Err(ExitError::Invalid(_))
        ));

        // A /29 has .2 to .6 for peers.
        for (i, id) in ["b", "c", "d", "e"].iter().enumerate() {
            let mut key = [0u8; 32];
            key[0] = i as u8 + 1;
            let key = base64::engine::general_purpose::STANDARD.encode(key);
            exit.enrol(id, &key).await.unwrap();
        }
        let mut key = [7u8; 32];
        key[1] = 9;
        let key = base64::engine::general_purpose::STANDARD.encode(key);
        assert!(matches!(exit.enrol("f", &key).await, Err(ExitError::Full)));

        exit.leave("c").await.unwrap();
        exit.leave("c").await.unwrap();
        assert_eq!(
            exit.enrol("f", &key).await.unwrap().address,
            "10.153.0.4/32"
        );
    }

    #[test]
    fn lets_out_only_to_the_internet() {
        for ok in ["1.1.1.1", "93.184.215.14", "2606:4700::1111"] {
            assert!(public_destination(ok.parse().unwrap()), "{ok}");
        }
        for no in [
            "127.0.0.1",
            "10.153.0.1",
            "192.168.1.1",
            "172.16.0.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "224.0.0.251",
            "::1",
            "fe80::1",
            "fd00::1",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
        ] {
            assert!(!public_destination(no.parse().unwrap()), "{no}");
        }
    }

    /// The SOCKS5 exit end to end on loopback: a wrong password is turned
    /// away, the right one reaches the destination, and a destination the
    /// rule refuses is refused.
    #[tokio::test]
    async fn socks_lets_an_enrolled_peer_out() {
        let dir = scratch();
        let exit = Exit::new(opts(&dir));
        let enrolled = exit.enrol("desk-a", KEY_A).await.unwrap();
        let peers = dir.join(PEERS_FILE);

        let site = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let site_addr = site.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut s, _) = site.accept().await.unwrap();
            let mut b = [0u8; 4];
            s.read_exact(&mut b).await.unwrap();
            s.write_all(&b.to_ascii_uppercase()).await.unwrap();
        });

        let socks = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let socks_addr = socks.local_addr().unwrap();
        let peers2 = peers.clone();
        tokio::spawn(async move {
            loop {
                let (c, _) = socks.accept().await.unwrap();
                let p = peers2.clone();
                tokio::spawn(async move {
                    let only_the_site = move |ip: IpAddr| ip == site_addr.ip();
                    let _ = serve_one(c, &p, &only_the_site).await;
                });
            }
        });

        async fn login(addr: SocketAddr, user: &str, pass: &str) -> (TcpStream, u8) {
            let mut c = TcpStream::connect(addr).await.unwrap();
            c.write_all(&[5, 1, 2]).await.unwrap();
            let mut m = [0u8; 2];
            c.read_exact(&mut m).await.unwrap();
            assert_eq!(m, [5, 2]);
            let mut req = vec![1, user.len() as u8];
            req.extend_from_slice(user.as_bytes());
            req.push(pass.len() as u8);
            req.extend_from_slice(pass.as_bytes());
            c.write_all(&req).await.unwrap();
            let mut s = [0u8; 2];
            c.read_exact(&mut s).await.unwrap();
            (c, s[1])
        }
        async fn connect(c: &mut TcpStream, to: SocketAddr) -> u8 {
            let IpAddr::V4(ip) = to.ip() else { panic!() };
            let mut req = vec![5, 1, 0, 1];
            req.extend_from_slice(&ip.octets());
            req.extend_from_slice(&to.port().to_be_bytes());
            c.write_all(&req).await.unwrap();
            let mut r = [0u8; 10];
            c.read_exact(&mut r).await.unwrap();
            r[1]
        }

        let (_, status) = login(socks_addr, "desk-a", "wrong").await;
        assert_eq!(status, 1);

        let (mut c, status) = login(socks_addr, "desk-a", &enrolled.socks_password).await;
        assert_eq!(status, 0);
        assert_eq!(connect(&mut c, "127.0.0.2:9".parse().unwrap()).await, 2);

        let (mut c, _) = login(socks_addr, "desk-a", &enrolled.socks_password).await;
        assert_eq!(connect(&mut c, site_addr).await, 0);
        c.write_all(b"ping").await.unwrap();
        let mut got = [0u8; 4];
        c.read_exact(&mut got).await.unwrap();
        assert_eq!(&got, b"PING");

        // Gone from the exit, gone from the SOCKS door.
        exit.leave("desk-a").await.unwrap();
        let (_, status) = login(socks_addr, "desk-a", &enrolled.socks_password).await;
        assert_eq!(status, 1);
    }
}
