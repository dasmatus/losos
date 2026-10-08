//! `losos-registrar lab`: LosOS Lab's guests under libvirt.
//!
//! The Lab draws a setup in the browser and boots one small x86_64 guest per
//! LosOS device and per piece of network gear. In the hosted copy those
//! guests run in the tab under qemu-wasm, three at most and slowly. This
//! helper runs them under the libvirt of the machine the browser is on
//! instead (KVM when the machine has it), and the Lab falls back to
//! qemu-wasm for any guest it cannot start here.
//!
//! libvirt is reached only through the `virsh` CLI ([`virsh`]). Each guest
//! is a *transient* domain (`virsh create`, never `define`) named
//! `losos-lab-<key>`, so nothing outlives the helper: it destroys every
//! guest it made on SIGTERM or Ctrl-C, sweeps `losos-lab-*` leftovers of a
//! killed predecessor when it starts, and destroys a guest no browser has
//! watched for `--idle` (a closed tab sends no DELETE).
//!
//! The browser stays the switch fabric. A guest's serial port is a TCP
//! client of the helper and each network card a UDP tunnel the helper owns;
//! both are bridged to WebSockets, so the Lab moves a libvirt guest's frames
//! between cables exactly as it moves a qemu-wasm guest's, and the two kinds
//! can share a segment. No guest is on a libvirt network or a bridge.
//!
//! Routes, all under `/lab/v1/`:
//!   * `GET hello`: version, libvirt URI, whether virsh answered, `kvm` or
//!     `qemu`, which images are present, the limits.
//!   * `GET guests`, `POST guests` ([`spec::GuestRequest`]), `DELETE
//!     guests/{key}`.
//!   * `GET guests/{key}/console` and `GET guests/{key}/nic/{n}`:
//!     WebSockets. Binary messages carry serial bytes and whole Ethernet
//!     frames. They authenticate with the per-guest ticket the create
//!     answered with, sent as the `ticket.<hex>` WebSocket subprotocol (a
//!     browser cannot set an Authorization header on a WebSocket, and a
//!     query string ends up in access logs).
//!
//! Who may call: a request with an `Origin` must come from one of the
//! `--origin` values or from the same origin it is addressed to; preflights
//! answer Private Network Access. Without `--token-file` the helper answers
//! only requests addressed to a loopback name, which stops a DNS-rebinding
//! page from driving it, and refuses to listen beyond loopback at all. With
//! one, every HTTP route but the two WebSockets wants `Authorization: Bearer`
//! (the box's lososd relays with the admin token).

pub mod spec;
pub mod virsh;

use std::collections::HashMap;
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{DefaultBodyLimit, Path as UrlPath, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get};
use axum::{Json, Router};
use miette::{miette, IntoDiagnostic, Result, WrapErr};
use ring::rand::{SecureRandom, SystemRandom};
use serde_json::json;
use subtle::ConstantTimeEq;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::{broadcast, mpsc, watch};
use tokio::task::JoinHandle;

use spec::{DomainType, Nic, Role};
use virsh::{Virsh, PREFIX};

const TARGET: &str = crate::action::Action::Lab.target();
/// Serial output kept for a console that attaches late, or attaches again.
const BACKLOG: usize = 64 * 1024;
/// Largest keystroke burst forwarded to a serial port in one message.
const MAX_INPUT: usize = 4096;
/// The WebSocket subprotocol the helper answers with.
const PROTOCOL: &str = "losos-lab";

/// How the guests run: hardware virtualisation, plain emulation, or
/// whichever libvirt offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtType {
    Auto,
    Kvm,
    Qemu,
}

/// `losos-registrar lab` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabOpts {
    pub listen: String,
    pub connect: String,
    pub images: PathBuf,
    pub origins: Vec<String>,
    pub token_file: Option<String>,
    pub virsh: String,
    pub max_guests: usize,
    pub memory_mib: u32,
    pub virt_type: VirtType,
    pub idle: Duration,
}

impl LabOpts {
    /// The defaults, as `losos-registrar lab` with no flags sees them.
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            listen: "127.0.0.1:8095".to_string(),
            connect: "qemu:///session".to_string(),
            images: PathBuf::from("guest"),
            // serve.py's address, so the copy build.sh makes works as it is.
            origins: vec![
                "http://localhost:8080".to_string(),
                "http://127.0.0.1:8080".to_string(),
            ],
            token_file: None,
            virsh: std::env::var("LOSOS_LAB_VIRSH")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "virsh".to_string()),
            max_guests: 8,
            memory_mib: 96,
            virt_type: VirtType::Auto,
            idle: Duration::from_secs(60),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn random_hex(bytes: usize) -> Result<String> {
    let mut raw = vec![0u8; bytes];
    SystemRandom::new()
        .fill(&mut raw)
        .map_err(|_| miette!("no randomness"))?;
    Ok(raw.iter().map(|b| format!("{b:02x}")).collect())
}

/// A guest's serial port: the bytes it printed so far, a fan-out to every
/// console watching, and the way back in.
struct Console {
    state: Mutex<(Vec<u8>, broadcast::Sender<Bytes>)>,
    input: mpsc::Sender<Bytes>,
}

impl Console {
    fn push(&self, bytes: &[u8]) {
        let mut st = lock(&self.state);
        st.0.extend_from_slice(bytes);
        if st.0.len() > BACKLOG {
            let cut = st.0.len() - BACKLOG;
            st.0.drain(..cut);
        }
        let _ = st.1.send(Bytes::copy_from_slice(bytes));
    }

    /// The backlog and a receiver for what follows it, taken under one lock
    /// so no byte is missed or doubled between the two.
    fn subscribe(&self) -> (Vec<u8>, broadcast::Receiver<Bytes>) {
        let st = lock(&self.state);
        (st.0.clone(), st.1.subscribe())
    }
}

/// One network card: the helper's end of the UDP tunnel, QEMU's address,
/// and the WebSocket currently attached (a newer one replaces it).
struct NicPort {
    sock: UdpSocket,
    qemu: SocketAddr,
    out: Mutex<Option<(u64, mpsc::Sender<Bytes>)>>,
    generation: AtomicUsize,
}

struct Guest {
    key: String,
    domain: String,
    ticket: String,
    name: String,
    role: Role,
    console: Arc<Console>,
    nics: Vec<Arc<NicPort>>,
    watchers: AtomicUsize,
    idle_since: Mutex<Instant>,
    gone: watch::Sender<bool>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
}

impl Guest {
    fn watch(self: &Arc<Self>) -> Watching {
        self.watchers.fetch_add(1, Ordering::SeqCst);
        Watching(self.clone())
    }

    fn finish(&self) {
        let _ = self.gone.send(true);
        for t in lock(&self.tasks).drain(..) {
            t.abort();
        }
    }
}

/// A console or NIC socket attached to a guest; the idle clock starts when
/// the last one goes.
struct Watching(Arc<Guest>);

impl Drop for Watching {
    fn drop(&mut self) {
        if self.0.watchers.fetch_sub(1, Ordering::SeqCst) == 1 {
            *lock(&self.0.idle_since) = Instant::now();
        }
    }
}

#[derive(Clone)]
struct Probe {
    uri: std::result::Result<String, String>,
    domain_type: DomainType,
}

struct Lab {
    opts: LabOpts,
    images: PathBuf,
    virsh: Virsh,
    token: Option<String>,
    guests: Mutex<HashMap<String, Arc<Guest>>>,
    create_lock: tokio::sync::Mutex<()>,
    probe: tokio::sync::Mutex<Option<(Instant, Probe)>>,
}

/// A refusal before any work: the status and its sentence.
type Denied = (StatusCode, &'static str);

fn answer(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({ "error": msg }))).into_response()
}

impl Lab {
    fn new(opts: LabOpts) -> Result<Self> {
        let images = std::fs::canonicalize(&opts.images).unwrap_or_else(|_| {
            std::env::current_dir()
                .map(|d| d.join(&opts.images))
                .unwrap_or_else(|_| opts.images.clone())
        });
        let token = match &opts.token_file {
            None => None,
            Some(f) => {
                let t = std::fs::read_to_string(f)
                    .into_diagnostic()
                    .wrap_err_with(|| format!("reading --token-file {f}"))?
                    .trim()
                    .to_string();
                if t.len() < 16 {
                    return Err(miette!("--token-file {f} holds fewer than 16 characters"));
                }
                Some(t)
            }
        };
        Ok(Self {
            virsh: Virsh {
                bin: opts.virsh.clone(),
                uri: opts.connect.clone(),
            },
            images,
            token,
            opts,
            guests: Mutex::new(HashMap::new()),
            create_lock: tokio::sync::Mutex::new(()),
            probe: tokio::sync::Mutex::new(None),
        })
    }

    /// What libvirt offers, asked again after a minute (or five seconds,
    /// while it is not answering).
    async fn probe(&self) -> Probe {
        let mut cached = self.probe.lock().await;
        if let Some((at, p)) = cached.as_ref() {
            let fresh = if p.uri.is_ok() {
                Duration::from_secs(60)
            } else {
                Duration::from_secs(5)
            };
            if at.elapsed() < fresh {
                return p.clone();
            }
        }
        let uri = self.virsh.uri().await;
        let domain_type = match self.opts.virt_type {
            VirtType::Kvm => DomainType::Kvm,
            VirtType::Qemu => DomainType::Qemu,
            VirtType::Auto if uri.is_ok() && self.virsh.kvm().await => DomainType::Kvm,
            VirtType::Auto => DomainType::Qemu,
        };
        let p = Probe { uri, domain_type };
        *cached = Some((Instant::now(), p.clone()));
        p
    }

    fn authorize(&self, headers: &HeaderMap) -> std::result::Result<(), Denied> {
        let Some(token) = &self.token else {
            return Ok(());
        };
        let given = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or("");
        if bool::from(given.as_bytes().ct_eq(token.as_bytes())) {
            Ok(())
        } else {
            Err((StatusCode::UNAUTHORIZED, "unauthorized"))
        }
    }

    fn guest(&self, key: &str) -> Option<Arc<Guest>> {
        lock(&self.guests).get(key).cloned()
    }

    /// The guest a WebSocket names, if its ticket is right.
    fn ticketed(&self, key: &str, headers: &HeaderMap) -> std::result::Result<Arc<Guest>, Denied> {
        let Some(g) = self.guest(key) else {
            return Err((StatusCode::NOT_FOUND, "no such guest"));
        };
        let ticket = ticket_of(headers).unwrap_or("");
        if bool::from(ticket.as_bytes().ct_eq(g.ticket.as_bytes())) {
            Ok(g)
        } else {
            Err((StatusCode::UNAUTHORIZED, "wrong or missing ticket"))
        }
    }

    /// Stop one guest: end its sockets and tasks, then destroy the domain.
    async fn end(&self, key: &str) -> bool {
        let Some(g) = lock(&self.guests).remove(key) else {
            return false;
        };
        g.finish();
        match self.virsh.destroy(&g.domain).await {
            Ok(()) => tracing::info!(target: TARGET, domain = g.domain, "destroyed"),
            Err(e) => tracing::warn!(target: TARGET, domain = g.domain, "destroy: {e}"),
        }
        true
    }

    async fn end_all(&self) {
        let keys: Vec<String> = lock(&self.guests).keys().cloned().collect();
        for k in keys {
            self.end(&k).await;
        }
    }

    /// Destroy `losos-lab-*` domains a killed helper left running.
    async fn sweep(&self) {
        match self.virsh.ours().await {
            Ok(names) => {
                for n in names {
                    match self.virsh.destroy(&n).await {
                        Ok(()) => tracing::info!(target: TARGET, domain = n, "swept a leftover"),
                        Err(e) => tracing::warn!(target: TARGET, domain = n, "sweep: {e}"),
                    }
                }
            }
            Err(e) => tracing::warn!(target: TARGET, "libvirt did not answer yet: {e}"),
        }
    }

    fn has(&self, file: &str) -> bool {
        self.images.join(file).is_file()
    }

    fn origin_allowed(&self, origin: &str, host: &str) -> bool {
        let origin = origin.trim_end_matches('/');
        self.opts
            .origins
            .iter()
            .any(|o| o.trim_end_matches('/').eq_ignore_ascii_case(origin))
            || authority(origin).is_some_and(|a| !host.is_empty() && a.eq_ignore_ascii_case(host))
    }
}

/// `host[:port]` of an `http(s)://` origin; `None` for `null` and the rest.
pub(crate) fn authority(origin: &str) -> Option<&str> {
    let rest = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))?;
    (!rest.is_empty() && !rest.contains('/')).then_some(rest)
}

/// Is a Host header a loopback name: `localhost`, `127.x.y.z` or `[::1]`,
/// with or without a port?
#[must_use]
pub fn loopback_host(host: &str) -> bool {
    let name = if let Some(rest) = host.strip_prefix('[') {
        match rest.split_once(']') {
            Some((inside, _)) => inside,
            None => return false,
        }
    } else {
        host.rsplit_once(':').map_or(host, |(h, _)| h)
    };
    name.eq_ignore_ascii_case("localhost")
        || name.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// The `ticket.<hex>` entry of `Sec-WebSocket-Protocol`.
fn ticket_of(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(header::SEC_WEBSOCKET_PROTOCOL)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .find_map(|p| p.strip_prefix("ticket."))
}

/// Origin, Host and CORS for every route.
async fn gate(State(lab): State<Arc<Lab>>, req: Request, next: Next) -> Response {
    let headers = req.headers();
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if lab.token.is_none() && !loopback_host(&host) {
        return answer(
            StatusCode::FORBIDDEN,
            "without --token-file this helper answers only requests addressed to 127.0.0.1 or localhost",
        );
    }
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    if let Some(o) = &origin {
        if !lab.origin_allowed(o, &host) {
            return answer(
                StatusCode::FORBIDDEN,
                "this page's origin is not one the helper serves; start it with --origin and the page's address",
            );
        }
    }
    let private = headers
        .get("access-control-request-private-network")
        .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"true"));
    let mut resp = if req.method() == Method::OPTIONS {
        let mut r = StatusCode::NO_CONTENT.into_response();
        let h = r.headers_mut();
        h.insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static("GET, POST, DELETE"),
        );
        h.insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static("authorization, content-type"),
        );
        h.insert(
            header::ACCESS_CONTROL_MAX_AGE,
            HeaderValue::from_static("600"),
        );
        if private {
            h.insert(
                "access-control-allow-private-network",
                HeaderValue::from_static("true"),
            );
        }
        r
    } else {
        next.run(req).await
    };
    let h = resp.headers_mut();
    // The hosted Lab is cross-origin isolated (COEP require-corp).
    h.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("cross-origin"),
    );
    if let Some(o) = origin.and_then(|o| HeaderValue::from_str(&o).ok()) {
        h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, o);
        h.insert(header::VARY, HeaderValue::from_static("Origin"));
    }
    resp
}

async fn hello(State(lab): State<Arc<Lab>>, headers: HeaderMap) -> Response {
    if let Err((status, why)) = lab.authorize(&headers) {
        return answer(status, why);
    }
    let p = lab.probe().await;
    let kernel = lab.has(spec::KERNEL);
    let rootfs = lab.has(spec::Image::Rootfs.file());
    let reason = match &p.uri {
        Err(e) => Some(format!("libvirt did not answer: {e}")),
        Ok(_) if !kernel || !rootfs => Some(format!(
            "{} has no bzImage and rootfs.bin; copy the hosted Lab's guest/ folder there",
            lab.images.display()
        )),
        Ok(_) => None,
    };
    Json(json!({
        "available": reason.is_none(),
        "version": env!("CARGO_PKG_VERSION"),
        "uri": p.uri.as_ref().map_or(lab.opts.connect.as_str(), String::as_str),
        "virsh": p.uri.is_ok(),
        "domainType": p.domain_type.as_str(),
        "label": p.domain_type.label(),
        "images": { "kernel": kernel, "rootfs": rootfs, "gear": lab.has(spec::Image::Gear.file()) },
        "maxGuests": lab.opts.max_guests,
        "memoryMiB": lab.opts.memory_mib,
        "guests": lock(&lab.guests).len(),
        "reason": reason,
    }))
    .into_response()
}

async fn list(State(lab): State<Arc<Lab>>, headers: HeaderMap) -> Response {
    if let Err((status, why)) = lab.authorize(&headers) {
        return answer(status, why);
    }
    let guests: Vec<_> = lock(&lab.guests)
        .values()
        .map(|g| json!({ "guest": g.key, "domain": g.domain, "name": g.name, "role": g.role.as_str() }))
        .collect();
    Json(json!({ "guests": guests })).into_response()
}

/// A loopback UDP port nobody holds right now, for QEMU to bind.
fn free_udp_port(not: u16) -> std::io::Result<u16> {
    loop {
        let port = std::net::UdpSocket::bind("127.0.0.1:0")?
            .local_addr()?
            .port();
        if port != not {
            return Ok(port);
        }
    }
}

async fn create(State(lab): State<Arc<Lab>>, headers: HeaderMap, body: Bytes) -> Response {
    if let Err((status, why)) = lab.authorize(&headers) {
        return answer(status, why);
    }
    let Ok(req) = serde_json::from_slice::<spec::GuestRequest>(&body) else {
        return answer(
            StatusCode::BAD_REQUEST,
            "the body must be {id, name, role, cmdline, macs} and nothing else",
        );
    };
    let c = match spec::check(req) {
        Ok(c) => c,
        Err(why) => return answer(StatusCode::BAD_REQUEST, &why),
    };
    let p = lab.probe().await;
    if let Err(e) = &p.uri {
        return answer(
            StatusCode::SERVICE_UNAVAILABLE,
            &format!("libvirt did not answer: {e}"),
        );
    }
    let image = c.role.image().file();
    for f in [spec::KERNEL, image] {
        if !lab.has(f) {
            return answer(
                StatusCode::SERVICE_UNAVAILABLE,
                &format!("{f} is not in the helper's images folder"),
            );
        }
    }
    let _one_at_a_time = lab.create_lock.lock().await;
    if lock(&lab.guests).len() >= lab.opts.max_guests {
        return answer(
            StatusCode::CONFLICT,
            &format!(
                "{} guests are running, as many as this helper starts",
                lab.opts.max_guests
            ),
        );
    }
    match start(&lab, &c, p.domain_type, image).await {
        Ok(g) => (
            StatusCode::CREATED,
            Json(json!({
                "guest": g.key,
                "domain": g.domain,
                "ticket": g.ticket,
                "nics": g.nics.len(),
                "domainType": p.domain_type.as_str(),
                "label": p.domain_type.label(),
            })),
        )
            .into_response(),
        Err(Refusal::Libvirt(e)) => answer(
            StatusCode::BAD_GATEWAY,
            &format!("libvirt refused the guest: {e}"),
        ),
        Err(Refusal::Local(e)) => {
            tracing::warn!(target: TARGET, "creating a guest: {e:?}");
            answer(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the helper could not set the guest up",
            )
        }
    }
}

enum Refusal {
    Libvirt(String),
    Local(miette::Report),
}

async fn start(
    lab: &Arc<Lab>,
    c: &spec::Checked,
    domain_type: DomainType,
    image: &str,
) -> std::result::Result<Arc<Guest>, Refusal> {
    let local = |e: std::io::Error| Refusal::Local(miette!("{e}"));
    let serial = TcpListener::bind("127.0.0.1:0").await.map_err(local)?;
    let serial_port = serial.local_addr().map_err(local)?.port();
    let mut nics = Vec::new();
    let mut socks = Vec::new();
    for mac in &c.macs {
        let sock = UdpSocket::bind("127.0.0.1:0").await.map_err(local)?;
        let helper_port = sock.local_addr().map_err(local)?.port();
        let qemu_port = free_udp_port(helper_port).map_err(local)?;
        nics.push(Nic {
            mac: mac.clone(),
            qemu_port,
            helper_port,
        });
        socks.push(sock);
    }
    let key = format!("{}-{}", c.id, random_hex(3).map_err(Refusal::Local)?);
    let domain = format!("{PREFIX}{key}");
    let ticket = random_hex(16).map_err(Refusal::Local)?;
    let title = format!("LosOS Lab: {}", c.name);
    let kernel = lab.images.join(spec::KERNEL);
    let disk = lab.images.join(image);
    let xml = spec::domain_xml(&spec::Domain {
        name: &domain,
        title: &title,
        domain_type,
        memory_mib: lab.opts.memory_mib,
        kernel: &kernel,
        disk: &disk,
        cmdline: &c.cmdline,
        serial_port,
        nics: &nics,
    });
    lab.virsh.create(&xml).await.map_err(Refusal::Libvirt)?;
    tracing::info!(target: TARGET, domain, role = c.role.as_str(), "created");

    let (input_tx, input_rx) = mpsc::channel(64);
    let console = Arc::new(Console {
        state: Mutex::new((Vec::new(), broadcast::channel(256).0)),
        input: input_tx,
    });
    let ports: Vec<Arc<NicPort>> = socks
        .into_iter()
        .zip(&nics)
        .map(|(sock, n)| {
            Arc::new(NicPort {
                sock,
                qemu: SocketAddr::from(([127, 0, 0, 1], n.qemu_port)),
                out: Mutex::new(None),
                generation: AtomicUsize::new(0),
            })
        })
        .collect();
    let mut tasks = vec![tokio::spawn(serial_bridge(
        serial,
        console.clone(),
        input_rx,
    ))];
    tasks.extend(ports.iter().map(|p| tokio::spawn(nic_bridge(p.clone()))));
    let g = Arc::new(Guest {
        key: key.clone(),
        domain,
        ticket,
        name: c.name.clone(),
        role: c.role,
        console,
        nics: ports,
        watchers: AtomicUsize::new(0),
        idle_since: Mutex::new(Instant::now()),
        gone: watch::channel(false).0,
        tasks: Mutex::new(tasks),
    });
    lock(&lab.guests).insert(key, g.clone());
    Ok(g)
}

/// QEMU connects its serial port here once; bytes go both ways until it
/// hangs up.
async fn serial_bridge(
    listener: TcpListener,
    console: Arc<Console>,
    mut input: mpsc::Receiver<Bytes>,
) {
    let stream = match tokio::time::timeout(Duration::from_secs(60), listener.accept()).await {
        Ok(Ok((s, peer))) if peer.ip().is_loopback() => s,
        _ => {
            console.push(b"\r\n[lab] the guest's serial port never connected\r\n");
            return;
        }
    };
    drop(listener);
    let (mut rd, mut wr) = stream.into_split();
    let to_guest = async {
        while let Some(b) = input.recv().await {
            if wr.write_all(&b).await.is_err() {
                break;
            }
        }
    };
    let from_guest = async {
        let mut buf = vec![0u8; 4096];
        loop {
            match rd.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => console.push(&buf[..n]),
            }
        }
    };
    tokio::select! {
        () = to_guest => {}
        () = from_guest => {}
    }
    console.push(b"\r\n[lab] the guest stopped\r\n");
}

/// Frames QEMU sends from its end of the tunnel go to the attached socket;
/// anything from another address is dropped.
async fn nic_bridge(port: Arc<NicPort>) {
    let mut buf = vec![0u8; 65536];
    loop {
        let Ok((n, from)) = port.sock.recv_from(&mut buf).await else {
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        };
        if from != port.qemu {
            continue;
        }
        let out = lock(&port.out).as_ref().map(|(_, tx)| tx.clone());
        if let Some(tx) = out {
            let _ = tx.try_send(Bytes::copy_from_slice(&buf[..n]));
        }
    }
}

async fn remove(
    State(lab): State<Arc<Lab>>,
    headers: HeaderMap,
    UrlPath(key): UrlPath<String>,
) -> Response {
    if let Err((status, why)) = lab.authorize(&headers) {
        return answer(status, why);
    }
    if lab.end(&key).await {
        StatusCode::NO_CONTENT.into_response()
    } else {
        answer(StatusCode::NOT_FOUND, "no such guest")
    }
}

async fn console_ws(
    State(lab): State<Arc<Lab>>,
    UrlPath(key): UrlPath<String>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    match lab.ticketed(&key, &headers) {
        Ok(g) => ws
            .protocols([PROTOCOL])
            .on_upgrade(move |s| console_session(g, s)),
        Err((status, why)) => answer(status, why),
    }
}

async fn console_session(g: Arc<Guest>, mut ws: WebSocket) {
    let _watching = g.watch();
    let mut gone = g.gone.subscribe();
    if *gone.borrow_and_update() {
        return;
    }
    let (backlog, mut rx) = g.console.subscribe();
    if !backlog.is_empty() && ws.send(Message::Binary(backlog.into())).await.is_err() {
        return;
    }
    loop {
        tokio::select! {
            m = rx.recv() => match m {
                Ok(b) => if ws.send(Message::Binary(b)).await.is_err() { break },
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            },
            m = ws.recv() => match m {
                Some(Ok(Message::Binary(b))) if b.len() <= MAX_INPUT => { let _ = g.console.input.send(b).await; }
                Some(Ok(Message::Text(t))) if t.len() <= MAX_INPUT => {
                    let _ = g.console.input.send(Bytes::copy_from_slice(t.as_str().as_bytes())).await;
                }
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            _ = gone.changed() => break,
        }
    }
}

async fn nic_ws(
    State(lab): State<Arc<Lab>>,
    UrlPath((key, n)): UrlPath<(String, usize)>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    match lab.ticketed(&key, &headers) {
        Ok(g) if n < g.nics.len() => ws
            .protocols([PROTOCOL])
            .on_upgrade(move |s| nic_session(g, n, s)),
        Ok(_) => answer(StatusCode::NOT_FOUND, "no such network card"),
        Err((status, why)) => answer(status, why),
    }
}

async fn nic_session(g: Arc<Guest>, n: usize, mut ws: WebSocket) {
    let _watching = g.watch();
    let mut gone = g.gone.subscribe();
    if *gone.borrow_and_update() {
        return;
    }
    let port = g.nics[n].clone();
    let (tx, mut rx) = mpsc::channel::<Bytes>(512);
    let mine = port.generation.fetch_add(1, Ordering::SeqCst) as u64 + 1;
    // A newer socket replaces an older one: the old sender drops, and the
    // old session's receiver ends.
    *lock(&port.out) = Some((mine, tx));
    loop {
        tokio::select! {
            f = rx.recv() => match f {
                Some(b) => if ws.send(Message::Binary(b)).await.is_err() { break },
                None => break,
            },
            m = ws.recv() => match m {
                Some(Ok(Message::Binary(b))) if (14..=65535).contains(&b.len()) => {
                    let _ = port.sock.send_to(&b, port.qemu).await;
                }
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            _ = gone.changed() => break,
        }
    }
    let mut out = lock(&port.out);
    if out.as_ref().is_some_and(|(g, _)| *g == mine) {
        *out = None;
    }
}

/// Destroy guests no console or NIC socket has watched for `--idle`.
async fn reaper(lab: Arc<Lab>) {
    let mut tick = tokio::time::interval(Duration::from_secs(5).min(lab.opts.idle));
    loop {
        tick.tick().await;
        let stale: Vec<String> = lock(&lab.guests)
            .values()
            .filter(|g| {
                g.watchers.load(Ordering::SeqCst) == 0
                    && lock(&g.idle_since).elapsed() >= lab.opts.idle
            })
            .map(|g| g.key.clone())
            .collect();
        for k in stale {
            tracing::info!(target: TARGET, guest = k, "nobody watched it; destroying");
            lab.end(&k).await;
        }
    }
}

fn router(lab: Arc<Lab>) -> Router {
    Router::new()
        .route("/lab/v1/hello", get(hello))
        .route("/lab/v1/guests", get(list).post(create))
        .route("/lab/v1/guests/{key}", delete(remove))
        .route("/lab/v1/guests/{key}/console", get(console_ws))
        .route("/lab/v1/guests/{key}/nic/{n}", get(nic_ws))
        .layer(DefaultBodyLimit::max(8 * 1024))
        .layer(middleware::from_fn_with_state(lab.clone(), gate))
        .with_state(lab)
}

/// Is `listen` a loopback address?
fn loopback_listen(listen: &str) -> bool {
    listen
        .parse::<SocketAddr>()
        .is_ok_and(|a| a.ip().is_loopback())
}

/// Bind `--listen` and serve until SIGTERM or Ctrl-C, then destroy every
/// guest.
pub async fn run(opts: LabOpts) -> Result<()> {
    if opts.token_file.is_none() && !loopback_listen(&opts.listen) {
        return Err(miette!(
            "--listen {} is not loopback; give --token-file to listen there",
            opts.listen
        ));
    }
    let listener = TcpListener::bind(&opts.listen)
        .await
        .into_diagnostic()
        .wrap_err_with(|| format!("bind {}", opts.listen))?;
    tracing::info!(target: TARGET, "listening on {}, libvirt {}", opts.listen, opts.connect);
    serve(listener, opts, crate::server::shutdown_signal()).await
}

/// Serve on a bound listener until `shutdown` resolves; every guest is
/// destroyed before this returns, whether the server stopped or failed.
pub async fn serve<F>(listener: TcpListener, opts: LabOpts, shutdown: F) -> Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    let lab = Arc::new(Lab::new(opts)?);
    if !lab.images.join(spec::KERNEL).is_file() {
        tracing::warn!(target: TARGET, "no bzImage in {}; the Lab will use qemu-wasm", lab.images.display());
    }
    lab.sweep().await;
    let reap = tokio::spawn(reaper(lab.clone()));
    let closing = lab.clone();
    // The guests go first: an open WebSocket would otherwise hold the
    // graceful shutdown open.
    let stop = async move {
        shutdown.await;
        closing.end_all().await;
    };
    let served = axum::serve(listener, router(lab.clone()))
        .with_graceful_shutdown(stop)
        .await
        .into_diagnostic();
    reap.abort();
    lab.end_all().await;
    tracing::info!(target: TARGET, "stopped; no guest left running");
    served
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_hosts() {
        for h in [
            "127.0.0.1:8095",
            "localhost:8095",
            "LOCALHOST",
            "[::1]:8095",
            "127.1.2.3",
            "[::1]",
        ] {
            assert!(loopback_host(h), "{h}");
        }
        for h in [
            "",
            "evil.example:8095",
            "192.168.1.5:8095",
            "localhost.evil.example",
            "[::2]:1",
            "[::1",
        ] {
            assert!(!loopback_host(h), "{h}");
        }
    }

    #[test]
    fn origins_are_matched_exactly_or_against_the_host() {
        let lab = Lab::new(LabOpts {
            origins: vec!["https://lab.example.org".into()],
            ..LabOpts::defaults()
        })
        .expect("lab");
        assert!(lab.origin_allowed("https://lab.example.org", "127.0.0.1:8095"));
        assert!(lab.origin_allowed("https://lab.example.org/", "127.0.0.1:8095"));
        assert!(lab.origin_allowed("http://mattbox.local", "mattbox.local"));
        assert!(!lab.origin_allowed("https://lab.example.org.evil", "127.0.0.1:8095"));
        assert!(!lab.origin_allowed("http://localhost:8080", "127.0.0.1:8095"));
        assert!(!lab.origin_allowed("null", "null"));
        assert!(!lab.origin_allowed("http://a/b", "a"));
    }

    #[test]
    fn tickets_come_from_the_subprotocol_header() {
        let mut h = HeaderMap::new();
        h.insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("losos-lab, ticket.00ff"),
        );
        assert_eq!(ticket_of(&h), Some("00ff"));
        assert_eq!(ticket_of(&HeaderMap::new()), None);
    }

    #[test]
    fn listening_beyond_loopback_needs_a_token() {
        assert!(loopback_listen("127.0.0.1:8095"));
        assert!(loopback_listen("[::1]:8095"));
        assert!(!loopback_listen("0.0.0.0:8095"));
        assert!(!loopback_listen("localhost:8095"));
    }
}
