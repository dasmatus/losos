//! Finding an edge proxy, and refusing to share storage without one.
//!
//! The mesh is other people's boxes behind an edge: the rke2 server that
//! Longhorn replicates to, the rathole tunnel that makes the box reachable,
//! the registrar that enrols it. Every one of those lives on the edge, so a
//! box with no edge in reach cannot share anything, whatever its settings
//! say — the agent's kubelet never starts, the announce loop retries forever,
//! and the owner sees a switch that is on and a mesh that is not there.
//!
//! So lososd *looks* for an edge, on two roads:
//!
//!   * **the LAN**, by DNS-SD. The edge publishes `_losos-edge._tcp` over
//!     mDNS (`losos.edge.lan.advertise` in `modules/edge.nix`), with a `url=`
//!     TXT record naming its registrar API. The box already runs Avahi for its
//!     own `.local` name, so `avahi-browse` is the one extra binary on the
//!     unit path, and the browse costs nothing the box was not already paying.
//!   * **the configured URL**, `losos.proxy.registrarUrl`, which on a box with
//!     internet is the public edge. It is probed whether or not the master
//!     proxy is switched on, because the question here is "is there an edge
//!     anywhere", not "is this box enrolled".
//!
//! A candidate from either road counts only once its `/health` answers: a TXT
//! record is a claim anyone on the LAN can make, and a configured name may
//! resolve to a host that is down. The scan runs every [`SCAN_INTERVAL`] in a
//! daemon thread ([`start_scanner`]) and the latest result is what
//! `GET /api/edge` serves and what the gate reads.
//!
//! The **gate** is [`check_gate`]: a `change --mode mesh`, or an `apply` that
//! turns on `losos.sharingMyStorage` or `losos.cluster.enable`, is refused
//! with [`EdgeRequired`] while nothing is reachable. It refuses *turning on*
//! only. A box that was sharing when its edge went away keeps its settings —
//! the mesh has already paused itself, and an owner changing the hostname
//! during an outage must not be told off about storage — and the admin UI
//! says what the daemon knows instead. Local use keeps working throughout:
//! nothing here touches Nextcloud, Forgejo or the box's own disk.
//!
//! This module is the pure half: the record shapes, the `avahi-browse` output
//! parser and the gate rule, all unit-tested. The subprocesses are in
//! `io_backend.rs` behind [`crate::losos::Losos::edge_status`], for the
//! reasons in `catalogue.rs` (lososd shells out rather than linking a client).

use std::fmt;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::model::Settings;

/// The DNS-SD service type an edge advertises on the LAN.
pub const SERVICE_TYPE: &str = "_losos-edge._tcp";

/// How often the daemon looks again. Short enough that an edge unplugged
/// during a demo is noticed inside a minute; long enough that a browse plus
/// two probes per cycle is nothing.
pub const SCAN_INTERVAL: Duration = Duration::from_secs(20);

/// The rathole port an edge binds unless its advert says otherwise
/// (`losos.edge.ratholeBindPort`'s default). Used for a LAN edge whose
/// advert predates the `rathole=` record.
pub const DEFAULT_RATHOLE_PORT: u16 = 2333;

/// The two units modules/proxy.nix runs against the chosen edge, in the
/// order they are restarted: the tunnel first, then the announce that asks
/// the edge to route to it.
pub const PATH_UNITS: [&str; 2] = [
    "losos-rathole-client.service",
    "losos-registrar-announce.service",
];

/// Budget for one `avahi-browse --terminate`. The browse itself settles in
/// about a second; the ceiling is for an Avahi that is not answering at all.
pub const BROWSE_TIMEOUT_SECS: u64 = 10;

/// Budget for one `/health` probe. Loopback or LAN answers in milliseconds;
/// the configured public edge in well under a second. Anything slower is
/// not an edge this box can share through.
pub const PROBE_TIMEOUT_SECS: u64 = 5;

/// Where a candidate came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Found by DNS-SD on the local network.
    Lan,
    /// `losos.proxy.registrarUrl`.
    Configured,
}

/// One edge proxy that answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    /// The advertised instance name (LAN) or the URL's host (configured).
    pub name: String,
    /// Base URL of its registrar API, no trailing slash.
    pub url: String,
    pub source: Source,
    /// The edge proved an identity the LosOS root key signed (see the
    /// `identity` section below). Only official edges may process trading;
    /// a company's own edge is `false` here and still shares storage.
    #[serde(default)]
    pub official: bool,
    /// Where this edge's rathole server listens, `host:port`: the advert's
    /// `rathole=` record (or its host on the default port) for a LAN edge,
    /// `losos.proxy.edgeRatholeEndpoint` for the configured one. `None` when
    /// the box was given no endpoint for it, in which case it can be found
    /// but not tunnelled to.
    #[serde(default)]
    pub rathole: Option<String>,
}

/// The edge this box uses, out of everything the scan found: the path
/// (handbook/docs/in-depth/edge-federation.md, "The box"). A LAN edge first,
/// the configured official one when the LAN has none, and `None` when neither
/// answered, which is what switches every edge-dependent feature off.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgePath {
    pub name: String,
    pub url: String,
    pub source: Source,
    /// `host:port` the tunnel dials. Always present: an edge with no
    /// endpoint is never chosen as the path.
    pub rathole: String,
}

/// What the last scan found. The JSON shape is the `GET /api/edge` contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeStatus {
    /// At least one edge answered. The one bit the gate reads.
    pub reachable: bool,
    /// At least one *official* edge answered. The one bit the market reads.
    #[serde(default)]
    pub official: bool,
    /// Every edge that answered, LAN first.
    pub edges: Vec<Edge>,
    /// Whether the LAN could be searched at all. False means `avahi-browse`
    /// did not run (missing from the unit path, Avahi down), which the UI
    /// says in so many words rather than claiming the network is empty.
    pub lan_searched: bool,
    /// The configured URL, when there is one, so the UI can name what was
    /// tried when nothing answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configured_url: Option<String>,
    /// Unix seconds of the scan this describes. `None` before the first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<u64>,
    /// The edge the box uses, derived from `edges` by [`choose_path`].
    #[serde(default)]
    pub path: Option<EdgePath>,
}

/// The path rule: the first LAN edge that answered
/// and can be tunnelled to, else the configured edge, else none. `edges` is
/// LAN first as [`assemble`] builds it, so the first edge with an endpoint
/// in list order is the answer.
#[must_use]
pub fn choose_path(edges: &[Edge]) -> Option<EdgePath> {
    let pick = |source: Source| {
        edges
            .iter()
            .find(|e| e.source == source && e.rathole.is_some())
    };
    pick(Source::Lan)
        .or_else(|| pick(Source::Configured))
        .map(|e| EdgePath {
            name: e.name.clone(),
            url: e.url.clone(),
            source: e.source,
            rathole: e.rathole.clone().unwrap_or_default(),
        })
}

impl EdgeStatus {
    /// Nothing found, nothing searched: the state before the first scan.
    #[must_use]
    pub fn unknown() -> Self {
        EdgeStatus {
            reachable: false,
            official: false,
            edges: Vec::new(),
            lan_searched: false,
            configured_url: None,
            checked_at: None,
            path: None,
        }
    }

    /// A scan that found `edges`. `reachable` is derived, never set apart
    /// from the list, so the two cannot disagree.
    #[must_use]
    pub fn found(edges: Vec<Edge>, lan_searched: bool, configured_url: Option<String>) -> Self {
        EdgeStatus {
            reachable: !edges.is_empty(),
            official: edges.iter().any(|e| e.official),
            path: choose_path(&edges),
            edges,
            lan_searched,
            configured_url,
            checked_at: None,
        }
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "reachable": self.reachable,
            "official": self.official,
            "edges": self.edges,
            "lanSearched": self.lan_searched,
            "configuredUrl": self.configured_url,
            "checkedAt": self.checked_at,
            "path": self.path,
        })
    }
}

/// Refused: a network-dependent sharing setting was being turned on with no
/// edge in reach. Rendered as a 409 by the HTTP layer; the message is the
/// sentence the owner reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeRequired {
    /// The setting that was refused, in its `losos.*` spelling.
    pub setting: &'static str,
}

impl fmt::Display for EdgeRequired {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "no edge proxy is reachable from this box, so {} cannot be turned on: \
             storage can only be shared through an edge. Local use keeps working.",
            self.setting
        )
    }
}

impl std::error::Error for EdgeRequired {}

/// The sharing settings that need an edge, and the rule for when one is
/// being *turned on*.
///
/// `cluster.shareCompute` is not listed: the mesh pane already refuses it
/// without `cluster.enable`, and gating the parent gates the child.
pub fn turning_on(current: &Settings, desired: &Settings) -> Option<&'static str> {
    if desired.sharing_my_storage && !current.sharing_my_storage {
        return Some("losos.sharingMyStorage");
    }
    if desired.cluster_enable && !current.cluster_enable {
        return Some("losos.cluster.enable");
    }
    None
}

/// The gate: `Ok` when `desired` turns nothing network-dependent on, or an
/// edge is reachable; [`EdgeRequired`] otherwise.
///
/// # Errors
/// Names the first setting that cannot be turned on.
pub fn check_gate(
    current: &Settings,
    desired: &Settings,
    edge: &EdgeStatus,
) -> Result<(), EdgeRequired> {
    match turning_on(current, desired) {
        Some(setting) if !edge.reachable => Err(EdgeRequired { setting }),
        _ => Ok(()),
    }
}

/// A resolved DNS-SD record out of `avahi-browse --parsable --resolve`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advert {
    pub name: String,
    pub host: String,
    pub address: String,
    pub port: u16,
    /// The `url=` TXT value, when the edge published one.
    pub url: Option<String>,
    /// The `rathole=` TXT value, when the edge published one.
    pub rathole: Option<String>,
}

impl Advert {
    /// The registrar URL this advert points at: the published `url=` when it
    /// is plain http(s), else `http://<address>:<port>`. IPv6 literals get
    /// their brackets.
    #[must_use]
    pub fn registrar_url(&self) -> String {
        if let Some(url) = &self.url {
            if plain_http_url(url) {
                return url.trim_end_matches('/').to_string();
            }
        }
        if self.address.contains(':') {
            format!("http://[{}]:{}", self.address, self.port)
        } else {
            format!("http://{}:{}", self.address, self.port)
        }
    }
}

/// `http://` or `https://`, a host, and nothing a browser would read as
/// something else. The URL goes to curl as an argument and into the admin
/// page as text; a `javascript:` or a `file:` is refused here.
#[must_use]
pub fn plain_http_url(url: &str) -> bool {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"));
    match rest {
        Some(rest) => {
            !rest.is_empty()
                && rest.len() <= 253
                && rest
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b":.-_[]/%".contains(&b))
        }
        None => false,
    }
}

/// Undo `avahi-browse --parsable`'s escaping: a byte outside the printable
/// range is written as a backslash and three decimal digits (`\032` is a
/// space), and a literal backslash as `\\`.
fn unescape(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut chars = field.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            Some('\\') => {
                chars.next();
                out.push('\\');
            }
            Some(d) if d.is_ascii_digit() => {
                let digits: String = (0..3)
                    .filter_map(|_| chars.next_if(char::is_ascii_digit))
                    .collect();
                match digits.parse::<u8>() {
                    Ok(b) if digits.len() == 3 => out.push(char::from(b)),
                    _ => {
                        out.push('\\');
                        out.push_str(&digits);
                    }
                }
            }
            _ => out.push('\\'),
        }
    }
    out
}

/// Parse `avahi-browse --parsable --resolve --terminate` output into the
/// resolved records.
///
/// Only `=` lines (resolved) count; `+` (seen) and `-` (gone) lines carry no
/// address. The columns are
/// `=;iface;proto;name;type;domain;host;address;port;txt`, with the TXT
/// records as space-separated quoted strings. IPv4 records are preferred:
/// Avahi reports a dual-stack edge twice, and the box's own network is
/// IPv4-first, so the IPv6 copy is kept only when there is no IPv4 one for
/// the same instance.
#[must_use]
pub fn parse_browse(output: &str) -> Vec<Advert> {
    let mut v4: Vec<Advert> = Vec::new();
    let mut v6: Vec<Advert> = Vec::new();
    for line in output.lines() {
        let cols: Vec<&str> = line.split(';').collect();
        if cols.len() < 9 || cols[0] != "=" || cols[4] != SERVICE_TYPE {
            continue;
        }
        let Ok(port) = cols[8].trim().parse::<u16>() else {
            continue;
        };
        let txt = cols.get(9).copied().unwrap_or("");
        let record = |key: &str| {
            txt.split('"')
                .filter(|s| !s.trim().is_empty())
                .find_map(|s| s.strip_prefix(key).map(str::to_string))
        };
        let advert = Advert {
            name: unescape(cols[3]),
            host: cols[6].to_string(),
            address: cols[7].to_string(),
            port,
            url: record("url="),
            rathole: record("rathole="),
        };
        if cols[2] == "IPv4" {
            v4.push(advert);
        } else {
            v6.push(advert);
        }
    }
    for a in v6 {
        if !v4.iter().any(|b| b.name == a.name) {
            v4.push(a);
        }
    }
    v4
}

/// Reduce a list of adverts to one candidate per registrar URL, in the order
/// they were seen. Two edges on one LAN are two candidates; one edge seen
/// over two interfaces is one.
#[must_use]
pub fn candidates(adverts: &[Advert]) -> Vec<Edge> {
    let mut out: Vec<Edge> = Vec::new();
    for a in adverts {
        let url = a.registrar_url();
        if out.iter().any(|e| e.url == url) {
            continue;
        }
        // The advertised tunnel endpoint when it is well formed, else the
        // registrar's host on the default port: an edge from before the
        // record existed still has a rathole there.
        let rathole = a
            .rathole
            .as_deref()
            .filter(|r| plain_endpoint(r))
            .map(str::to_string)
            .unwrap_or_else(|| format!("{}:{DEFAULT_RATHOLE_PORT}", bracketed_host(&url)));
        out.push(Edge {
            name: a.name.clone(),
            url,
            source: Source::Lan,
            official: false,
            rathole: Some(rathole),
        });
    }
    out
}

/// `host:port` as rathole takes it: a hostname or IP literal (IPv6 in
/// brackets) and a port, nothing a shell or a TOML string would read as
/// something else. The value goes into client.toml between quotes.
#[must_use]
pub fn plain_endpoint(s: &str) -> bool {
    let Some((host, port)) = s.rsplit_once(':') else {
        return false;
    };
    let host = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    !host.is_empty()
        && host.len() <= 253
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b":.-_".contains(&b))
        && port.parse::<u16>().is_ok_and(|p| p != 0)
}

/// The host of a URL with its port removed and an IPv6 literal kept in its
/// brackets, so a port can be appended to it.
#[must_use]
pub fn bracketed_host(url: &str) -> String {
    let hostport = host_of(url);
    if let Some(end) = hostport.strip_prefix('[').and_then(|h| h.find(']')) {
        return hostport[..end + 2].to_string();
    }
    match hostport.rsplit_once(':') {
        Some((host, port)) if port.parse::<u16>().is_ok() => host.to_string(),
        _ => hostport,
    }
}

/// The host part of a URL, for naming the configured edge.
#[must_use]
pub fn host_of(url: &str) -> String {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    rest.split('/').next().unwrap_or(rest).to_string()
}

/// One scan's inputs, as the subprocess layer hands them in: what the browse
/// said and which candidate URLs answered `/health`. Pure, so the whole
/// decision is testable without Avahi or curl.
pub fn assemble(
    browse: Option<&str>,
    configured_url: Option<&str>,
    answered: impl Fn(&str) -> bool,
    trust: &Trust<'_>,
    prove: impl Fn(&str, &str) -> Option<String>,
    now: u64,
) -> EdgeStatus {
    assemble_with(browse, configured_url, None, answered, trust, prove, now)
}

/// [`assemble`] with the configured edge's tunnel endpoint
/// (`losos.proxy.edgeRatholeEndpoint`), which is what lets that edge be the
/// path when the LAN has none.
pub fn assemble_with(
    browse: Option<&str>,
    configured_url: Option<&str>,
    configured_rathole: Option<&str>,
    answered: impl Fn(&str) -> bool,
    trust: &Trust<'_>,
    prove: impl Fn(&str, &str) -> Option<String>,
    now: u64,
) -> EdgeStatus {
    let mut edges: Vec<Edge> = Vec::new();
    if let Some(output) = browse {
        for edge in candidates(&parse_browse(output)) {
            if answered(&edge.url) {
                edges.push(edge);
            }
        }
    }
    let configured_url = configured_url
        .map(|u| u.trim().trim_end_matches('/').to_string())
        .filter(|u| plain_http_url(u));
    if let Some(url) = &configured_url {
        if !edges.iter().any(|e| &e.url == url) && answered(url) {
            edges.push(Edge {
                name: host_of(url),
                url: url.clone(),
                source: Source::Configured,
                official: false,
                rathole: configured_rathole
                    .map(str::trim)
                    .filter(|r| plain_endpoint(r))
                    .map(str::to_string),
            });
        }
    }
    // Every edge that answered is asked who it is. With no root key there
    // is no one to vouch, so nothing is official and no request is made.
    if trust.root_public.is_some() {
        for edge in &mut edges {
            edge.official = prove(&edge.url, trust.nonce)
                .is_some_and(|body| verify_answer(trust, &body, &edge.url, now).is_ok());
        }
    }
    let mut status = EdgeStatus::found(edges, browse.is_some(), configured_url);
    status.checked_at = Some(now);
    status
}

// ── the path, as the tunnel units read it ──────────────────────────────────
//
// lososd tells modules/proxy.nix's two units which edge to dial through one
// file on /run, in systemd EnvironmentFile= syntax, and drives the units when
// it changes. The rendering and the decision are here so they can be tested
// without a systemd; the file writes and `systemctl` calls are in
// io_backend.rs.

/// The env file for a chosen path. Every value is one the box constructed
/// from a validated URL or endpoint (`plain_http_url`, `plain_endpoint`) or
/// a path it named itself, so none needs quoting under systemd's rules.
#[must_use]
pub fn path_env(path: &EdgePath, noise_pin: Option<&str>) -> String {
    let mut out = format!(
        "LOSOS_EDGE_PATH_URL={}\nLOSOS_EDGE_PATH_RATHOLE={}\nLOSOS_EDGE_PATH_SOURCE={}\n",
        path.url,
        path.rathole,
        match path.source {
            Source::Lan => "lan",
            Source::Configured => "configured",
        }
    );
    if let Some(pin) = noise_pin {
        out.push_str(&format!("LOSOS_EDGE_PATH_NOISE_PUB={pin}\n"));
    }
    out
}

/// The file name a LAN edge's Noise key is pinned under: its registrar's
/// host and port with anything outside `[A-Za-z0-9.-]` replaced, so two
/// edges never share a pin and the name is safe in a shell.
#[must_use]
pub fn pin_file_name(url: &str) -> String {
    let host = host_of(url);
    let safe: String = host
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{}.pub", safe.trim_matches('_'))
}

/// What to do to the tunnel units after a scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drive {
    /// A path is set and it is new, or the units were stopped: start them
    /// against it (a restart starts a stopped unit too).
    Restart,
    /// No edge in reach and the units have not been told yet: stop them.
    Stop,
    /// Nothing changed since the last scan.
    Nothing,
}

/// The decision, from what the previous scan left on disk (`prev_env`, the
/// env file's content if it existed; `had_none`, whether the marker was
/// there) and what this scan rendered (`next_env`, `None` for no path).
#[must_use]
pub fn drive(prev_env: Option<&str>, had_none: bool, next_env: Option<&str>) -> Drive {
    match next_env {
        Some(next) if had_none || prev_env != Some(next) => Drive::Restart,
        Some(_) => Drive::Nothing,
        None if had_none => Drive::Nothing,
        None => Drive::Stop,
    }
}

// ── identity: an official edge, or a company's own ─────────────────────────
//
// Any company may run an edge beside its boxes; only edges LosOS itself runs
// may process trading. The box tells them apart with one root key whose
// public half it carries (`losos.proxy.officialRootKeyFile`): an official
// edge holds a *certificate* — its name, URL, Ed25519 public key and expiry,
// signed by the root — and proves on every scan that it holds the certified
// key by signing the nonce the box sends (`GET /identity?nonce=…`). Four
// checks: the root's signature, the URL the box is talking to, the clock,
// the nonce signature. The formats are the contract with
// `backend-registrar/src/identity.rs`, byte for byte; the tests pin them.

/// Domain separators, versioned. Must match the registrar's.
pub const CERT_PREFIX: &str = "losos-edge-identity-v1";
pub const NONCE_PREFIX: &str = "losos-edge-nonce-v1";

/// What the box trusts for one scan: the root public key (hex) and the nonce
/// it chose for this round.
pub struct Trust<'a> {
    pub root_public: Option<&'a str>,
    pub nonce: &'a str,
}

/// A certificate as the edge serves it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cert {
    pub name: String,
    pub url: String,
    pub public_key: String,
    pub not_after: u64,
    pub signature: String,
}

/// The edge's answer to the challenge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer {
    pub cert: Cert,
    pub nonce_signature: String,
}

/// Why an answer was not accepted. Logged, never shown as a failure: a
/// company edge answering 404 is the ordinary case and is not an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejected {
    NotJson,
    RootSignature,
    Url,
    Expired,
    EdgeSignature,
}

#[must_use]
pub fn cert_message(name: &str, url: &str, public_key: &str, not_after: u64) -> Vec<u8> {
    format!("{CERT_PREFIX}\n{name}\n{url}\n{public_key}\n{not_after}\n").into_bytes()
}

#[must_use]
pub fn nonce_message(nonce: &str) -> Vec<u8> {
    format!("{NONCE_PREFIX}\n{nonce}\n").into_bytes()
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// The LosOS root **public** key as the box ships it: one line of 64 hex
/// characters, in a file that may also carry `#` comment lines and blank
/// lines, so the committed default can explain itself. `None` when the file
/// holds no key (the shipped default is empty: no root, nothing is official,
/// the market stays off), or more than one, or anything that is not a key.
#[must_use]
pub fn parse_root_key_file(text: &str) -> Option<String> {
    let mut keys = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'));
    let key = keys.next()?;
    if keys.next().is_some() {
        return None;
    }
    (key.len() == 64 && key.bytes().all(|b| b.is_ascii_hexdigit())).then(|| key.to_lowercase())
}

/// Ed25519 verification; any malformed input is a plain `false`.
#[must_use]
pub fn verify(public_hex: &str, msg: &[u8], sig_hex: &str) -> bool {
    let (Some(public), Some(sig)) = (from_hex(public_hex.trim()), from_hex(sig_hex.trim())) else {
        return false;
    };
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public)
        .verify(msg, &sig)
        .is_ok()
}

/// The four checks over an `/identity` answer body.
///
/// # Errors
/// The first check that failed.
pub fn verify_answer(
    trust: &Trust<'_>,
    body: &str,
    url: &str,
    now: u64,
) -> Result<Answer, Rejected> {
    let Some(root) = trust.root_public else {
        return Err(Rejected::RootSignature);
    };
    let answer: Answer = serde_json::from_str(body).map_err(|_| Rejected::NotJson)?;
    let c = &answer.cert;
    if !verify(
        root,
        &cert_message(&c.name, &c.url, &c.public_key, c.not_after),
        &c.signature,
    ) {
        return Err(Rejected::RootSignature);
    }
    if c.url.trim_end_matches('/') != url.trim().trim_end_matches('/') {
        return Err(Rejected::Url);
    }
    if now >= c.not_after {
        return Err(Rejected::Expired);
    }
    if !verify(
        &c.public_key,
        &nonce_message(trust.nonce),
        &answer.nonce_signature,
    ) {
        return Err(Rejected::EdgeSignature);
    }
    Ok(answer)
}

/// Refused: trading was asked of a box with no official edge in reach.
/// Rendered as a 409 by the HTTP layer; the message is the sentence the
/// owner reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialEdgeRequired;

impl fmt::Display for OfficialEdgeRequired {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            "no official LosOS edge is reachable from this box, so trading is off: \
             only edges LosOS runs may process the market. Sharing storage through \
             your own edge keeps working.",
        )
    }
}

impl std::error::Error for OfficialEdgeRequired {}

/// The market gate: trading needs an official edge.
///
/// # Errors
/// [`OfficialEdgeRequired`] when none is reachable.
pub fn check_market_gate(edge: &EdgeStatus) -> Result<(), OfficialEdgeRequired> {
    if edge.official {
        Ok(())
    } else {
        Err(OfficialEdgeRequired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No root key: nothing can be official and no challenge is sent.
    const NO_TRUST: Trust<'static> = Trust {
        root_public: None,
        nonce: "00",
    };

    const BROWSE: &str = "\
+;eth0;IPv6;edge\\032demo;_losos-edge._tcp;local
+;eth0;IPv4;edge\\032demo;_losos-edge._tcp;local
=;eth0;IPv6;edge\\032demo;_losos-edge._tcp;local;edge.local;fe80::5054:ff:fe12:3456;8443;\"url=http://edge.local:8443\" \"txtvers=1\"
=;eth0;IPv4;edge\\032demo;_losos-edge._tcp;local;edge.local;192.168.100.1;8443;\"url=http://edge.local:8443\" \"txtvers=1\"
";

    #[test]
    fn resolved_ipv4_record_wins_and_the_name_is_unescaped() {
        let adverts = parse_browse(BROWSE);
        assert_eq!(adverts.len(), 1, "{adverts:?}");
        let a = &adverts[0];
        assert_eq!(a.name, "edge demo");
        assert_eq!(a.address, "192.168.100.1");
        assert_eq!(a.port, 8443);
        assert_eq!(a.url.as_deref(), Some("http://edge.local:8443"));
        assert_eq!(a.registrar_url(), "http://edge.local:8443");
    }

    #[test]
    fn an_edge_without_a_url_record_is_reached_by_address_and_port() {
        let out = "=;eth0;IPv4;edge;_losos-edge._tcp;local;edge.local;10.0.0.2;8443;\n";
        let adverts = parse_browse(out);
        assert_eq!(adverts[0].registrar_url(), "http://10.0.0.2:8443");
        let out6 = "=;eth0;IPv6;edge;_losos-edge._tcp;local;edge.local;fd00::2;8443;\n";
        assert_eq!(
            parse_browse(out6)[0].registrar_url(),
            "http://[fd00::2]:8443"
        );
    }

    #[test]
    fn a_url_record_that_is_not_http_is_ignored() {
        let out = "=;eth0;IPv4;edge;_losos-edge._tcp;local;edge.local;10.0.0.2;8443;\"url=javascript:alert(1)\"\n";
        assert_eq!(parse_browse(out)[0].registrar_url(), "http://10.0.0.2:8443");
        assert!(!plain_http_url("ftp://x"));
        assert!(!plain_http_url("http://"));
        assert!(!plain_http_url("http://a b"));
        assert!(plain_http_url("https://losos-edge.dasmat.us"));
        assert!(plain_http_url("http://[fd00::2]:8443/"));
    }

    #[test]
    fn other_service_types_and_unresolved_lines_are_skipped() {
        let out = "\
=;eth0;IPv4;printer;_ipp._tcp;local;p.local;10.0.0.9;631;
+;eth0;IPv4;edge;_losos-edge._tcp;local
-;eth0;IPv4;edge;_losos-edge._tcp;local
";
        assert!(parse_browse(out).is_empty());
    }

    #[test]
    fn one_edge_seen_on_two_interfaces_is_one_candidate() {
        let out = "\
=;eth0;IPv4;edge;_losos-edge._tcp;local;edge.local;10.0.0.2;8443;\"url=http://edge.local:8443\"
=;eth1;IPv4;edge;_losos-edge._tcp;local;edge.local;10.0.1.2;8443;\"url=http://edge.local:8443\"
=;eth0;IPv4;other;_losos-edge._tcp;local;other.local;10.0.0.3;8443;
";
        let c = candidates(&parse_browse(out));
        assert_eq!(c.len(), 2, "{c:?}");
        assert_eq!(c[0].url, "http://edge.local:8443");
        assert_eq!(c[1].url, "http://10.0.0.3:8443");
        assert!(c.iter().all(|e| e.source == Source::Lan));
    }

    #[test]
    fn a_candidate_counts_only_when_its_health_answers() {
        let status = assemble(
            Some(BROWSE),
            Some("https://losos-edge.example/"),
            |_| false,
            &NO_TRUST,
            |_, _| None,
            7,
        );
        assert!(!status.reachable);
        assert!(status.edges.is_empty());
        assert!(status.lan_searched);
        assert_eq!(
            status.configured_url.as_deref(),
            Some("https://losos-edge.example")
        );
        assert_eq!(status.checked_at, Some(7));

        let status = assemble(
            Some(BROWSE),
            Some("https://losos-edge.example"),
            |url| url == "http://edge.local:8443",
            &NO_TRUST,
            |_, _| None,
            8,
        );
        assert!(status.reachable);
        assert_eq!(status.edges.len(), 1);
        assert_eq!(status.edges[0].name, "edge demo");
        assert_eq!(status.edges[0].source, Source::Lan);
    }

    #[test]
    fn the_configured_edge_is_named_by_its_host_and_listed_after_the_lan() {
        let status = assemble(
            Some(BROWSE),
            Some("https://losos-edge.example"),
            |_| true,
            &NO_TRUST,
            |_, _| None,
            1,
        );
        assert_eq!(status.edges.len(), 2);
        assert_eq!(status.edges[1].name, "losos-edge.example");
        assert_eq!(status.edges[1].source, Source::Configured);
        // The same edge on both roads is one edge.
        let status = assemble(
            Some(BROWSE),
            Some("http://edge.local:8443/"),
            |_| true,
            &NO_TRUST,
            |_, _| None,
            1,
        );
        assert_eq!(status.edges.len(), 1);
    }

    #[test]
    fn a_lan_that_could_not_be_searched_says_so() {
        let status = assemble(None, None, |_| true, &NO_TRUST, |_, _| None, 1);
        assert!(!status.lan_searched);
        assert!(!status.reachable);
        assert_eq!(status.configured_url, None);
        let doc = status.to_json();
        assert_eq!(doc["lanSearched"], false);
        assert_eq!(doc["configuredUrl"], Value::Null);
    }

    fn settings(sharing: bool, cluster: bool) -> Settings {
        Settings {
            sharing_my_storage: sharing,
            cluster_enable: cluster,
            ..Settings::default()
        }
    }

    #[test]
    fn the_gate_refuses_turning_sharing_on_without_an_edge() {
        let none = EdgeStatus::found(Vec::new(), true, None);
        let err = check_gate(&settings(false, false), &settings(true, false), &none).unwrap_err();
        assert_eq!(err.setting, "losos.sharingMyStorage");
        assert!(err.to_string().contains("Local use keeps working"));
        let err = check_gate(&settings(false, false), &settings(false, true), &none).unwrap_err();
        assert_eq!(err.setting, "losos.cluster.enable");
    }

    #[test]
    fn the_gate_lets_an_unrelated_change_through_while_sharing_stays_on() {
        let none = EdgeStatus::found(Vec::new(), true, None);
        // Already on: not being turned on, so not refused.
        assert!(check_gate(&settings(true, true), &settings(true, true), &none).is_ok());
        // Turning off is always allowed.
        assert!(check_gate(&settings(true, true), &settings(false, false), &none).is_ok());
    }

    #[test]
    fn the_gate_opens_when_an_edge_answers() {
        let some = EdgeStatus::found(
            vec![Edge {
                name: "edge".into(),
                url: "http://edge.local:8443".into(),
                source: Source::Lan,
                official: false,
                rathole: Some("edge.local:2333".into()),
            }],
            true,
            None,
        );
        assert!(check_gate(&settings(false, false), &settings(true, true), &some).is_ok());
    }

    #[test]
    fn unescape_handles_avahi_escapes() {
        assert_eq!(unescape("a\\032b"), "a b");
        assert_eq!(unescape("a\\\\b"), "a\\b");
        assert_eq!(unescape("plain"), "plain");
        assert_eq!(unescape("bad\\9"), "bad\\9");
    }

    // ── identity ───────────────────────────────────────────────────────────

    struct Signer(ring::signature::Ed25519KeyPair, String);

    fn signer() -> Signer {
        use ring::signature::KeyPair;
        let rng = ring::rand::SystemRandom::new();
        let pkcs8 = ring::signature::Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let pair = ring::signature::Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
        let public = pair
            .public_key()
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Signer(pair, public)
    }

    fn hex_sig(pair: &ring::signature::Ed25519KeyPair, msg: &[u8]) -> String {
        pair.sign(msg)
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// A certificate `root` issued to `edge` for `url`, and its answer to
    /// `nonce`, as the registrar would serialise them.
    fn answer_json(root: &Signer, edge: &Signer, url: &str, not_after: u64, nonce: &str) -> String {
        let cert = Cert {
            name: "losos edge one".into(),
            url: url.into(),
            public_key: edge.1.clone(),
            not_after,
            signature: hex_sig(
                &root.0,
                &cert_message("losos edge one", url, &edge.1, not_after),
            ),
        };
        serde_json::to_string(&Answer {
            cert,
            nonce_signature: hex_sig(&edge.0, &nonce_message(nonce)),
        })
        .unwrap()
    }

    const NONCE: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    #[test]
    fn the_root_key_file_may_carry_comments_but_only_one_key() {
        let key = "ab".repeat(32);
        assert_eq!(parse_root_key_file(""), None);
        assert_eq!(parse_root_key_file("# no key yet\n\n"), None);
        assert_eq!(
            parse_root_key_file(&format!("# the root\n{}\n", key.to_uppercase())),
            Some(key.clone())
        );
        assert_eq!(parse_root_key_file(&format!("{key}\n{key}\n")), None);
        assert_eq!(parse_root_key_file("not a key"), None);
    }

    #[test]
    fn the_message_formats_are_the_contract_with_the_registrar() {
        assert_eq!(
            cert_message("n", "u", "k", 7),
            b"losos-edge-identity-v1\nn\nu\nk\n7\n".to_vec()
        );
        assert_eq!(nonce_message("ab"), b"losos-edge-nonce-v1\nab\n".to_vec());
    }

    #[test]
    fn an_edge_the_root_vouched_for_is_official_and_every_other_is_not() {
        let root = signer();
        let edge = signer();
        let url = "http://edge.local:8443";
        let trust = Trust {
            root_public: Some(&root.1),
            nonce: NONCE,
        };
        let good = answer_json(&root, &edge, url, 2_000_000_000, NONCE);
        assert!(verify_answer(&trust, &good, url, 1_900_000_000).is_ok());

        // Signed by someone else's root.
        let other = signer();
        let forged = answer_json(&other, &edge, url, 2_000_000_000, NONCE);
        assert_eq!(
            verify_answer(&trust, &forged, url, 1_900_000_000),
            Err(Rejected::RootSignature)
        );
        // A real certificate, replayed from another edge's address.
        assert_eq!(
            verify_answer(&trust, &good, "http://10.0.0.9:8443", 1_900_000_000),
            Err(Rejected::Url)
        );
        // Expired.
        assert_eq!(
            verify_answer(&trust, &good, url, 2_000_000_000),
            Err(Rejected::Expired)
        );
        // A stranger holding the certificate but not the key: the nonce
        // signature does not verify under the certified key.
        let stranger = signer();
        let mut a: Answer = serde_json::from_str(&good).unwrap();
        a.nonce_signature = hex_sig(&stranger.0, &nonce_message(NONCE));
        assert_eq!(
            verify_answer(
                &trust,
                &serde_json::to_string(&a).unwrap(),
                url,
                1_900_000_000
            ),
            Err(Rejected::EdgeSignature)
        );
        // Yesterday's answer, replayed under today's nonce.
        let stale = Trust {
            root_public: Some(&root.1),
            nonce: "ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100",
        };
        assert_eq!(
            verify_answer(&stale, &good, url, 1_900_000_000),
            Err(Rejected::EdgeSignature)
        );
        assert_eq!(
            verify_answer(&trust, "not json", url, 1_900_000_000),
            Err(Rejected::NotJson)
        );
    }

    #[test]
    fn the_scan_marks_official_edges_and_asks_nothing_without_a_root_key() {
        let root = signer();
        let edge = signer();
        let url = "http://edge.local:8443";
        let good = answer_json(&root, &edge, url, 2_000_000_000, NONCE);
        let trust = Trust {
            root_public: Some(&root.1),
            nonce: NONCE,
        };
        // The LAN edge proves itself; the configured one answers 404.
        let status = assemble(
            Some(BROWSE),
            Some("https://losos-edge.example"),
            |_| true,
            &trust,
            |u, n| (u == url && n == NONCE).then(|| good.clone()),
            1_900_000_000,
        );
        assert!(status.official);
        assert!(status.edges[0].official);
        assert!(!status.edges[1].official);
        assert_eq!(status.to_json()["official"], true);
        assert_eq!(status.to_json()["edges"][0]["official"], true);
        assert!(check_market_gate(&status).is_ok());

        // No root key on this box: nothing is asked and nothing is official.
        let asked = std::cell::Cell::new(0);
        let status = assemble(
            Some(BROWSE),
            None,
            |_| true,
            &NO_TRUST,
            |_, _| {
                asked.set(asked.get() + 1);
                Some(good.clone())
            },
            1_900_000_000,
        );
        assert_eq!(asked.get(), 0);
        assert!(status.reachable && !status.official);
        assert_eq!(check_market_gate(&status), Err(OfficialEdgeRequired));
        assert!(OfficialEdgeRequired
            .to_string()
            .contains("only edges LosOS runs may process the market"));
    }

    const BROWSE_WITH_RATHOLE: &str = "\
=;eth0;IPv4;gateway;_losos-edge._tcp;local;gw.local;192.168.1.2;8443;\"url=http://gw.local:8443\" \"rathole=gw.local:2333\" \"enrol=open\" \"txtvers=1\"
";

    #[test]
    fn the_advert_carries_the_tunnel_endpoint_and_an_old_one_gets_the_default_port() {
        let c = candidates(&parse_browse(BROWSE_WITH_RATHOLE));
        assert_eq!(c[0].rathole.as_deref(), Some("gw.local:2333"));
        let c = candidates(&parse_browse(BROWSE));
        assert_eq!(c[0].rathole.as_deref(), Some("edge.local:2333"));
        // A malformed record is ignored, not dialled.
        let bad = BROWSE_WITH_RATHOLE.replace("rathole=gw.local:2333", "rathole=gw.local:2333;rm");
        let c = candidates(&parse_browse(&bad));
        assert_eq!(c[0].rathole.as_deref(), Some("gw.local:2333"));
        assert!(plain_endpoint("[fe80::1]:2333"));
        assert!(!plain_endpoint("gw.local"));
        assert!(!plain_endpoint("gw.local:0"));
        assert!(!plain_endpoint("gw local:2333"));
        assert_eq!(bracketed_host("http://[fe80::1]:8443"), "[fe80::1]");
        assert_eq!(bracketed_host("https://edge.example"), "edge.example");
    }

    #[test]
    fn the_path_is_the_lan_edge_then_the_configured_one_then_none() {
        let both = assemble_with(
            Some(BROWSE_WITH_RATHOLE),
            Some("https://losos-edge.example"),
            Some("edge.losos.cfd:2333"),
            |_| true,
            &NO_TRUST,
            |_, _| None,
            1,
        );
        let path = both.path.clone().expect("a path");
        assert_eq!(path.source, Source::Lan);
        assert_eq!(path.url, "http://gw.local:8443");
        assert_eq!(path.rathole, "gw.local:2333");
        assert_eq!(both.to_json()["path"]["rathole"], "gw.local:2333");

        let official_only = assemble_with(
            Some(BROWSE_WITH_RATHOLE),
            Some("https://losos-edge.example"),
            Some("edge.losos.cfd:2333"),
            |url| url.starts_with("https://"),
            &NO_TRUST,
            |_, _| None,
            1,
        );
        let path = official_only.path.expect("a path");
        assert_eq!(path.source, Source::Configured);
        assert_eq!(path.rathole, "edge.losos.cfd:2333");

        // A configured edge the box has no tunnel endpoint for can be found
        // but is not a path.
        let no_endpoint = assemble(
            None,
            Some("https://losos-edge.example"),
            |_| true,
            &NO_TRUST,
            |_, _| None,
            1,
        );
        assert!(no_endpoint.reachable);
        assert_eq!(no_endpoint.path, None);

        let none = assemble(Some(BROWSE), None, |_| false, &NO_TRUST, |_, _| None, 1);
        assert_eq!(none.path, None);
        assert_eq!(none.to_json()["path"], Value::Null);
    }

    #[test]
    fn the_env_file_and_the_drive_decision() {
        let path = EdgePath {
            name: "gateway".into(),
            url: "http://gw.local:8443".into(),
            source: Source::Lan,
            rathole: "gw.local:2333".into(),
        };
        let env = path_env(
            &path,
            Some("/var/secrets/losos-edge-pins/gw.local_8443.pub"),
        );
        assert_eq!(
            env,
            "LOSOS_EDGE_PATH_URL=http://gw.local:8443\n\
             LOSOS_EDGE_PATH_RATHOLE=gw.local:2333\n\
             LOSOS_EDGE_PATH_SOURCE=lan\n\
             LOSOS_EDGE_PATH_NOISE_PUB=/var/secrets/losos-edge-pins/gw.local_8443.pub\n"
        );
        assert!(!path_env(&path, None).contains("NOISE"));
        assert_eq!(pin_file_name("http://gw.local:8443"), "gw.local_8443.pub");
        assert_eq!(pin_file_name("http://[fe80::1]:8443"), "fe80__1__8443.pub");

        // First scan after boot with a path: start the units against it.
        assert_eq!(drive(None, false, Some(&env)), Drive::Restart);
        // Same path again (a daemon restart mid-rebuild included): nothing.
        assert_eq!(drive(Some(&env), false, Some(&env)), Drive::Nothing);
        // The path moved: restart.
        assert_eq!(drive(Some("other"), false, Some(&env)), Drive::Restart);
        // The edge went away: stop once, then nothing while it stays away.
        assert_eq!(drive(Some(&env), false, None), Drive::Stop);
        assert_eq!(drive(None, true, None), Drive::Nothing);
        // It came back: the units were stopped, so start them.
        assert_eq!(drive(None, true, Some(&env)), Drive::Restart);
    }
}
