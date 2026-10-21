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
//!     TXT record naming its registrar API. The box already runs
//!     systemd-resolved with MulticastDNS on for its own `.local` name, so
//!     the browse is two Varlink calls on resolved's socket
//!     (`io.systemd.Resolve.BrowseServices`, then `ResolveService` per
//!     instance): no extra binary on the unit path, and nothing the box was
//!     not already running.
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
//! nothing here touches LosOS Cloud, Forgejo or the box's own disk.
//!
//! This module is the pure half: the record shapes, the Varlink framing and
//! what resolved's replies mean, and the gate rule, all unit-tested. The
//! socket and the subprocesses are in `io_backend.rs` behind
//! [`crate::losos::Losos::edge_status`]. The probes shell out to curl for the
//! reasons in `catalogue.rs`; the browse speaks Varlink itself, because it is
//! a NUL-terminated JSON message on a local socket, which std and serde_json
//! already cover, and resolved ships no command that browses and exits.

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

/// Ceiling for one whole LAN browse, resolving included: what a resolved
/// that accepts the call and then never answers can cost a scan.
pub const BROWSE_TIMEOUT_SECS: u64 = 10;

/// How long the browse listens before it takes what it has. resolved's
/// browser answers from its cache at once and sends its first mDNS query
/// straight away, and an edge on the LAN answers within a second (Avahi's
/// `--terminate` settled in about that); the rest covers the second query,
/// which resolved sends a second after the first, and its answers. resolved
/// sends *nothing* while nothing is found, so a quiet LAN costs exactly this
/// long.
pub const BROWSE_SETTLE: Duration = Duration::from_secs(3);

/// systemd-resolved's Varlink socket. Any local user may browse and resolve
/// on it, so lososd needs no extra privilege for it.
pub const RESOLVE_SOCKET: &str = "/run/systemd/resolve/io.systemd.Resolve";

/// mDNS's domain, the only one an edge advertises on.
pub const DOMAIN: &str = "local";

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
    /// Whether the LAN could be searched at all. False means resolved's
    /// socket could not be opened or it refused the browse (resolved down,
    /// too old to browse), which the UI says in so many words rather than
    /// claiming the network is empty.
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
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "no edge proxy is reachable from this box, so {setting} cannot be turned on: \
     storage can only be shared through an edge. Local use keeps working."
)]
pub struct EdgeRequired {
    /// The setting that was refused, in its `losos.*` spelling.
    pub setting: &'static str,
}

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

/// Refused: an `overrides.nix` that has the box join the mesh when it cannot
/// enrol. These are the two assertions of `modules/cluster.nix`, checked at
/// Apply so the owner reads the reason on the pane instead of in a rebuild
/// that fails before it builds anything.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MeshNotReady {
    /// `losos.cluster.enable` without `losos.proxy.enable`: the join signs
    /// in with the tunnel's token, which only the master proxy sets up.
    #[error(
        "joining the mesh needs \"Reachable from outside your home\" on in Network: \
         the box signs in to the edge with the same key as the tunnel"
    )]
    ProxyOff,
    /// The node name is empty or the stock `mattbox` every box ships with,
    /// which the edge refuses for the second box that joins.
    #[error(
        "joining the mesh needs a name of this box's own: every box starts as \
         \"{STOCK_NAME}\", and the edge turns away a second box by the same name. \
         Rename it in Network first"
    )]
    StockName,
}

/// The host name every box ships with (`losos.hostName`'s default).
pub const STOCK_NAME: &str = "mattbox";

/// The name this box joins the mesh under, as `modules/options.nix` derives
/// it: `losos.cluster.nodeName`, else `losos.proxy.applianceId`, else
/// `losos.hostName`.
#[must_use]
pub fn mesh_node_name(overrides: &str) -> String {
    let unquote = |v: String| v.trim().trim_matches('"').to_string();
    crate::overrides::lookup_nix("cluster.nodeName", overrides)
        .or_else(|| crate::overrides::lookup_nix("proxy.applianceId", overrides))
        .or_else(|| crate::overrides::lookup_nix("hostName", overrides))
        .map_or_else(|| STOCK_NAME.to_string(), unquote)
}

/// The mesh half of an Apply: `Ok` unless `overrides` joins the mesh on a
/// box the edge cannot enrol. Checked whenever the join is on, not only when
/// it is turned on: switching the proxy off under a joined box fails the
/// same assertion.
///
/// # Errors
/// The first thing the box is missing.
pub fn check_mesh(overrides: &str) -> Result<(), MeshNotReady> {
    let desired = crate::overrides::parse_settings(overrides);
    if !desired.cluster_enable {
        return Ok(());
    }
    if !desired.proxy_enable {
        return Err(MeshNotReady::ProxyOff);
    }
    let name = mesh_node_name(overrides);
    if name.is_empty() || name == STOCK_NAME {
        return Err(MeshNotReady::StockName);
    }
    Ok(())
}

/// One resolved DNS-SD instance, out of a `ResolveService` reply
/// ([`advert_from_resolved`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advert {
    pub name: String,
    /// The SRV record's target host, without its final dot.
    pub host: String,
    /// One address of that host, as text: IPv4 when it has one.
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

// ── the LAN browse, over resolved's Varlink ─────────────────────────────────
//
// Varlink is JSON over a stream socket, each message ended by a NUL byte. A
// call is `{"method", "parameters", "more"?}`; a reply is `{"parameters",
// "continues"?}` or `{"error", "parameters"?}`. The field names below are
// systemd 261's `io.systemd.Resolve` interface
// (src/shared/varlink-io.systemd.Resolve.c), what resolved itself checks
// replies against. io_backend.rs owns the socket; everything it reads comes
// through here, so a reply of the wrong shape is a test case, not a panic.

/// Upper bound on one Varlink message. A browse reply is a few hundred bytes
/// per instance and a resolve reply a few hundred more; anything near this is
/// not resolved talking, and the reader stops rather than buffer it.
pub const VARLINK_MAX_MESSAGE: u64 = 1024 * 1024;

/// One Varlink call: its JSON and the NUL that ends a message. `more` asks
/// for a stream of replies, which `BrowseServices` requires (resolved
/// answers anything else with `org.varlink.service.ExpectedMore`).
#[must_use]
pub fn varlink_call(method: &str, parameters: Value, more: bool) -> Vec<u8> {
    let mut call = json!({ "method": method, "parameters": parameters });
    if more {
        call["more"] = Value::Bool(true);
    }
    let mut bytes = call.to_string().into_bytes();
    bytes.push(0);
    bytes
}

/// One Varlink reply that was not an error.
#[derive(Debug, Clone, PartialEq)]
pub struct VarlinkReply {
    /// `Null` when the reply carried none: sd-varlink leaves an empty
    /// parameters object out of the message altogether.
    pub parameters: Value,
    /// More replies follow on this call.
    pub continues: bool,
}

/// Parse one message, its NUL already removed. A Varlink error comes back
/// as `Err` with its name and any parameters, which is what the journal
/// wants to say about it (`io.systemd.Resolve.NoSuchResourceRecord`,
/// `org.varlink.service.MethodNotFound` from a resolved too old to browse).
///
/// # Errors
/// The message is not a JSON object, or it is a Varlink error.
pub fn parse_varlink_reply(message: &[u8]) -> Result<VarlinkReply, String> {
    let value: Value =
        serde_json::from_slice(message).map_err(|e| format!("not a Varlink message: {e}"))?;
    if !value.is_object() {
        return Err("not a Varlink message: not a JSON object".into());
    }
    let parameters = value.get("parameters").cloned().unwrap_or(Value::Null);
    if let Some(error) = value.get("error") {
        let name = error.as_str().unwrap_or("an unnamed Varlink error");
        return Err(match parameters {
            Value::Null => name.to_string(),
            p => format!("{name} {p}"),
        });
    }
    Ok(VarlinkReply {
        parameters,
        continues: value.get("continues").and_then(Value::as_bool) == Some(true),
    })
}

/// The parameters of the one `BrowseServices` call a scan makes: every link
/// (`ifindex` 0) and flags 0, which resolved widens to every protocol it has,
/// mDNS included (it refuses a browse without mDNS).
#[must_use]
pub fn browse_parameters() -> Value {
    json!({ "domain": DOMAIN, "type": SERVICE_TYPE, "ifindex": 0, "flags": 0 })
}

/// One DNS-SD instance on one link: what `ResolveService` is asked about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    pub name: String,
    pub service_type: String,
    pub domain: String,
    /// The link it was seen on. Resolving on that link asks the edge there,
    /// which is the address this box can reach it at.
    pub ifindex: u32,
}

/// The instances present so far, each with the mDNS family it was seen over
/// (resolved runs one mDNS scope per link and family and reports each), in
/// the order they were first seen.
pub type Present = Vec<(Instance, i64)>;

/// Apply one `BrowseServices` reply to `present`: each `browserServiceData`
/// entry is `{updateFlag: "added"|"removed", family, name, type, domain,
/// ifindex}`. An entry with no name (resolved allows that for a plain SRV
/// service, and an edge always names itself), of another type, or of any
/// other shape is skipped: the browse goes on without it.
pub fn apply_browse(present: &mut Present, parameters: &Value) {
    let Some(entries) = parameters
        .get("browserServiceData")
        .and_then(Value::as_array)
    else {
        return;
    };
    for entry in entries {
        let seen = (|| {
            let instance = Instance {
                name: entry.get("name")?.as_str()?.to_string(),
                service_type: entry.get("type")?.as_str()?.to_string(),
                domain: entry.get("domain")?.as_str()?.to_string(),
                ifindex: u32::try_from(entry.get("ifindex")?.as_u64()?).ok()?,
            };
            let family = entry.get("family")?.as_i64()?;
            let added = match entry.get("updateFlag")?.as_str()? {
                "added" => true,
                "removed" => false,
                _ => return None,
            };
            Some((instance, family, added))
        })();
        let Some((instance, family, added)) = seen else {
            continue;
        };
        if instance.name.is_empty() || instance.service_type != SERVICE_TYPE {
            continue;
        }
        let key = (instance, family);
        let known = present.iter().position(|k| *k == key);
        match (added, known) {
            (true, None) => present.push(key),
            (false, Some(i)) => {
                present.remove(i);
            }
            _ => {}
        }
    }
}

/// The instances to resolve: one per name and link, in the order first
/// seen. An edge reachable over both families on one link is one lookup,
/// because `ResolveService` with no family returns its IPv4 and IPv6
/// addresses together.
#[must_use]
pub fn to_resolve(present: &Present) -> Vec<Instance> {
    let mut out: Vec<Instance> = Vec::new();
    for (instance, _) in present {
        if !out.contains(instance) {
            out.push(instance.clone());
        }
    }
    out
}

/// The parameters of the `ResolveService` call for one instance. `family` 0
/// (AF_UNSPEC) asks for both address families, and `flags` 0 keeps the TXT
/// and address lookups resolved does by default.
#[must_use]
pub fn resolve_parameters(instance: &Instance) -> Value {
    json!({
        "name": instance.name,
        "type": instance.service_type,
        "domain": instance.domain,
        "ifindex": instance.ifindex,
        "family": 0,
        "flags": 0,
    })
}

/// Read a `ResolveService` reply's parameters into the [`Advert`] for the
/// instance `name`. The reply is `{services: [{priority, weight, port,
/// hostname, canonicalName?, addresses?: [{ifindex?, family, address: [byte,
/// …]}]}], txt?: [string], canonical, flags}`; the first service is the one,
/// since an edge publishes a single SRV record.
///
/// IPv4 is preferred: a dual-stack edge resolves to both, and the box's own
/// network is IPv4-first. Without one, a routable IPv6 address comes before a
/// link-local one, which means nothing in a URL without its interface.
/// `None` for anything malformed (no service, a port that is missing or 0,
/// no hostname, no usable address, a `txt` that is not a list), so a broken
/// advert is skipped rather than dialled.
#[must_use]
pub fn advert_from_resolved(name: &str, parameters: &Value) -> Option<Advert> {
    use std::net::IpAddr;

    let service = parameters.get("services")?.as_array()?.first()?;
    let port = u16::try_from(service.get("port")?.as_u64()?)
        .ok()
        .filter(|p| *p != 0)?;
    let host = service
        .get("hostname")?
        .as_str()?
        .trim_end_matches('.')
        .to_string();
    if host.is_empty() {
        return None;
    }
    let addresses: Vec<IpAddr> = service
        .get("addresses")?
        .as_array()?
        .iter()
        .filter_map(|a| {
            let bytes: Vec<u8> = a
                .get("address")?
                .as_array()?
                .iter()
                .map(|b| b.as_u64().and_then(|b| u8::try_from(b).ok()))
                .collect::<Option<_>>()?;
            // The family must agree with the length, or the entry is not
            // one resolved wrote: AF_INET is 2 and AF_INET6 10 on Linux.
            match (a.get("family")?.as_i64()?, bytes.len()) {
                (2, 4) => Some(IpAddr::from(<[u8; 4]>::try_from(bytes).ok()?)),
                (10, 16) => Some(IpAddr::from(<[u8; 16]>::try_from(bytes).ok()?)),
                _ => None,
            }
        })
        .collect();
    let link_local =
        |ip: &IpAddr| matches!(ip, IpAddr::V6(v6) if v6.segments()[0] & 0xffc0 == 0xfe80);
    let address = addresses
        .iter()
        .find(|ip| ip.is_ipv4())
        .or_else(|| addresses.iter().find(|ip| !link_local(ip)))
        .or_else(|| addresses.first())?
        .to_string();
    // resolved leaves `txt` out when the service has none.
    let txt: Vec<&str> = match parameters.get("txt") {
        None | Some(Value::Null) => Vec::new(),
        Some(t) => t.as_array()?.iter().filter_map(Value::as_str).collect(),
    };
    // DNS-SD keys are case-insensitive (RFC 6763 section 6.4). A value
    // resolved had to escape keeps its backslashes and fails the URL and
    // endpoint checks downstream.
    let record = |key: &str| {
        txt.iter()
            .filter_map(|t| t.split_once('='))
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.to_string())
    };
    Some(Advert {
        name: name.to_string(),
        host,
        address,
        port,
        url: record("url"),
        rathole: record("rathole"),
    })
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

/// One scan's inputs, as the I/O layer hands them in: what the browse found
/// (`None` when the LAN could not be searched) and which candidate URLs
/// answered `/health`. Pure, so the whole decision is testable without
/// resolved or curl.
pub fn assemble(
    browse: Option<&[Advert]>,
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
    browse: Option<&[Advert]>,
    configured_url: Option<&str>,
    configured_rathole: Option<&str>,
    answered: impl Fn(&str) -> bool,
    trust: &Trust<'_>,
    prove: impl Fn(&str, &str) -> Option<String>,
    now: u64,
) -> EdgeStatus {
    let mut edges: Vec<Edge> = Vec::new();
    if let Some(adverts) = browse {
        for edge in candidates(adverts) {
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
// `edge/registrar/src/identity.rs`, byte for byte; the tests pin them.

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
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "no official LosOS edge is reachable from this box, so trading is off: \
     only edges LosOS runs may process the market. Sharing storage through \
     your own edge keeps working."
)]
pub struct OfficialEdgeRequired;

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

// ── management: a company's own edge looking after this box ───────────────
//
// A local edge's web page can change this box's LosOS Cloud, LosOS Git and
// app settings, but only once the owner switched it on for that edge
// (`State::managed_by`, `POST /api/edge/manage`). The edge calls lososd
// through the box's own tunnel, which is pinned to the edge's Noise key, and
// nginx's `losos-manage.invalid` server, which takes only loopback (where
// the tunnel lands); the call carries the box's proxy token, which only this
// box and the edge it enrolled with hold. lososd then lets it use the
// sections in `sections.json`'s `edge` list and nothing else.

/// Why the switch could not be turned on. Rendered as a 409.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NotManageable {
    /// The box reaches no edge, or only through the configured one.
    #[error("this box is not joined to an edge on its own network")]
    NoLocalEdge,
    /// The edge it is joined to is an official LosOS edge, which never
    /// looks after anyone's box.
    #[error("an official LosOS edge does not manage boxes; only a company's own edge does")]
    Official,
}

/// The edge that may manage this box right now: the path, when it is an edge
/// on the LAN that is not official. Official edges never manage boxes.
///
/// # Errors
/// What stands in the way.
pub fn manageable(status: &EdgeStatus) -> Result<&EdgePath, NotManageable> {
    let path = status
        .path
        .as_ref()
        .filter(|p| p.source == Source::Lan)
        .ok_or(NotManageable::NoLocalEdge)?;
    let official = status.edges.iter().any(|e| e.url == path.url && e.official);
    if official {
        return Err(NotManageable::Official);
    }
    Ok(path)
}

/// Whether the edge the owner switched management on for (`managed_by`, its
/// registrar URL) is the one this box uses now. A box that moves to another
/// edge is not managed by it until the owner says so again.
#[must_use]
pub fn manager_active(managed_by: Option<&str>, status: &EdgeStatus) -> bool {
    match (managed_by, manageable(status)) {
        (Some(url), Ok(path)) => path.url == url,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lan_edge(url: &str, official: bool) -> Edge {
        Edge {
            name: "office".into(),
            url: url.into(),
            source: Source::Lan,
            official,
            rathole: Some("192.168.1.2:2333".into()),
        }
    }

    #[test]
    fn only_a_local_unofficial_edge_can_manage() {
        let none = EdgeStatus::found(vec![], true, None);
        assert_eq!(manageable(&none), Err(NotManageable::NoLocalEdge));

        let configured = EdgeStatus::found(
            vec![Edge {
                source: Source::Configured,
                ..lan_edge("https://register.example", false)
            }],
            true,
            None,
        );
        assert_eq!(manageable(&configured), Err(NotManageable::NoLocalEdge));

        let official =
            EdgeStatus::found(vec![lan_edge("http://office.local:8443", true)], true, None);
        assert_eq!(manageable(&official), Err(NotManageable::Official));

        let company = EdgeStatus::found(
            vec![lan_edge("http://office.local:8443", false)],
            true,
            None,
        );
        assert_eq!(
            manageable(&company).map(|p| p.url.as_str()),
            Ok("http://office.local:8443")
        );
    }

    #[test]
    fn management_follows_the_edge_it_was_switched_on_for() {
        let office = EdgeStatus::found(
            vec![lan_edge("http://office.local:8443", false)],
            true,
            None,
        );
        assert!(manager_active(Some("http://office.local:8443"), &office));
        assert!(!manager_active(None, &office));

        let other = EdgeStatus::found(vec![lan_edge("http://other.local:8443", false)], true, None);
        assert!(!manager_active(Some("http://office.local:8443"), &other));

        let gone = EdgeStatus::found(vec![], true, None);
        assert!(!manager_active(Some("http://office.local:8443"), &gone));
    }

    /// No root key: nothing can be official and no challenge is sent.
    const NO_TRUST: Trust<'static> = Trust {
        root_public: None,
        nonce: "00",
    };

    /// The browse of a LAN with one dual-stack edge on one link, as resolved
    /// sends it: the instance over its IPv6 and its IPv4 mDNS scope.
    const BROWSE_REPLY: &[u8] = br#"{"parameters":{"browserServiceData":[{"updateFlag":"added","family":10,"name":"edge demo","type":"_losos-edge._tcp","domain":"local","ifindex":2},{"updateFlag":"added","family":2,"name":"edge demo","type":"_losos-edge._tcp","domain":"local","ifindex":2}]},"continues":true}"#;

    /// resolved's answer to `ResolveService` for that instance: the SRV
    /// record, the host's link-local IPv6 and its IPv4 address, the TXT
    /// strings.
    const RESOLVE_REPLY: &[u8] = br#"{"parameters":{"services":[{"priority":0,"weight":0,"port":8443,"hostname":"edge.local","addresses":[{"ifindex":2,"family":10,"address":[254,128,0,0,0,0,0,0,80,84,0,255,254,18,52,86]},{"ifindex":2,"family":2,"address":[192,168,100,1]}]}],"txt":["url=http://edge.local:8443","txtvers=1"],"canonical":{"name":"edge demo","type":"_losos-edge._tcp","domain":"local"},"flags":0}}"#;

    /// A `ResolveService` reply's parameters for one SRV record on `host`,
    /// with `addresses` as resolved encodes them and `txt` left out when
    /// empty, as resolved leaves it out.
    fn resolved(host: &str, port: u16, addresses: &[&str], txt: &[&str]) -> Value {
        let addresses: Vec<Value> = addresses
            .iter()
            .map(|a| match a.parse::<std::net::IpAddr>().unwrap() {
                std::net::IpAddr::V4(v4) => {
                    json!({"ifindex": 2, "family": 2, "address": v4.octets()})
                }
                std::net::IpAddr::V6(v6) => {
                    json!({"ifindex": 2, "family": 10, "address": v6.octets()})
                }
            })
            .collect();
        let mut reply = json!({
            "services": [{"priority": 0, "weight": 0, "port": port,
                          "hostname": host, "addresses": addresses}],
            "canonical": {"name": "x", "type": SERVICE_TYPE, "domain": "local"},
            "flags": 0
        });
        if !txt.is_empty() {
            reply["txt"] = json!(txt);
        }
        reply
    }

    /// The whole pure path of a browse: the browse reply, the instances it
    /// leaves to resolve, and each one's resolve reply, as io_backend.rs
    /// runs them.
    fn browse() -> Vec<Advert> {
        let mut present = Present::new();
        apply_browse(
            &mut present,
            &parse_varlink_reply(BROWSE_REPLY).unwrap().parameters,
        );
        let resolve = parse_varlink_reply(RESOLVE_REPLY).unwrap().parameters;
        to_resolve(&present)
            .iter()
            .filter_map(|i| advert_from_resolved(&i.name, &resolve))
            .collect()
    }

    #[test]
    fn the_mesh_join_needs_the_proxy_and_a_name_of_its_own() {
        let body = |lines: &str| format!("{{ ... }}:\n{{\n{lines}}}\n");
        // Not joining: nothing to check, stock name and all.
        assert_eq!(check_mesh(&body("  losos.proxy.enable = false;\n")), Ok(()));
        // A fresh box: proxy off, stock name.
        assert_eq!(
            check_mesh(&body("  losos.cluster.enable = true;\n")),
            Err(MeshNotReady::ProxyOff)
        );
        assert_eq!(
            check_mesh(&body(
                "  losos.hostName = \"mattbox\";\n  losos.proxy.enable = true;\n  losos.cluster.enable = true;\n"
            )),
            Err(MeshNotReady::StockName)
        );
        assert_eq!(
            check_mesh(&body(
                "  losos.hostName = \"kitchen\";\n  losos.proxy.enable = true;\n  losos.cluster.enable = true;\n"
            )),
            Ok(())
        );
        // The appliance id and the node name come before the host name,
        // as in modules/options.nix.
        assert_eq!(
            check_mesh(&body(
                "  losos.proxy.enable = true;\n  losos.cluster.enable = true;\n  losos.proxy.applianceId = \"kitchen\";\n"
            )),
            Ok(())
        );
        assert_eq!(
            check_mesh(&body(
                "  losos.hostName = \"kitchen\";\n  losos.proxy.enable = true;\n  losos.cluster.enable = true;\n  losos.cluster.nodeName = \"\";\n"
            )),
            Err(MeshNotReady::StockName)
        );
    }

    #[test]
    fn the_ipv4_address_wins_and_one_link_over_two_families_is_one_advert() {
        let adverts = browse();
        assert_eq!(adverts.len(), 1, "{adverts:?}");
        let a = &adverts[0];
        // resolved hands the name over as text; there is nothing to unescape.
        assert_eq!(a.name, "edge demo");
        assert_eq!(a.host, "edge.local");
        assert_eq!(a.address, "192.168.100.1");
        assert_eq!(a.port, 8443);
        assert_eq!(a.url.as_deref(), Some("http://edge.local:8443"));
        assert_eq!(a.registrar_url(), "http://edge.local:8443");
        // Without IPv4, a routable IPv6 address before a link-local one.
        let v6 = resolved("edge.local.", 8443, &["fe80::1", "fd00::2"], &[]);
        let a = advert_from_resolved("edge", &v6).unwrap();
        assert_eq!(a.address, "fd00::2");
        assert_eq!(a.host, "edge.local");
        let ll = resolved("edge.local", 8443, &["fe80::1"], &[]);
        assert_eq!(
            advert_from_resolved("edge", &ll).unwrap().address,
            "fe80::1"
        );
    }

    #[test]
    fn an_edge_without_a_url_record_is_reached_by_address_and_port() {
        let reply = resolved("edge.local", 8443, &["10.0.0.2"], &[]);
        assert!(reply.get("txt").is_none());
        let a = advert_from_resolved("edge", &reply).unwrap();
        assert_eq!(a.url, None);
        assert_eq!(a.registrar_url(), "http://10.0.0.2:8443");
        let reply6 = resolved("edge.local", 8443, &["fd00::2"], &[]);
        assert_eq!(
            advert_from_resolved("edge", &reply6)
                .unwrap()
                .registrar_url(),
            "http://[fd00::2]:8443"
        );
    }

    #[test]
    fn a_url_record_that_is_not_http_is_ignored() {
        let reply = resolved(
            "edge.local",
            8443,
            &["10.0.0.2"],
            &["url=javascript:alert(1)"],
        );
        assert_eq!(
            advert_from_resolved("edge", &reply)
                .unwrap()
                .registrar_url(),
            "http://10.0.0.2:8443"
        );
        assert!(!plain_http_url("ftp://x"));
        assert!(!plain_http_url("http://"));
        assert!(!plain_http_url("http://a b"));
        assert!(plain_http_url("https://losos-edge.dasmat.us"));
        assert!(plain_http_url("http://[fd00::2]:8443/"));
    }

    #[test]
    fn a_malformed_resolve_reply_is_no_advert() {
        let good = resolved("edge.local", 8443, &["10.0.0.2"], &["url=http://e:1"]);
        assert!(advert_from_resolved("edge", &good).is_some());
        let without = |pointer: &str, key: &str| {
            let mut r = good.clone();
            r.pointer_mut(pointer)
                .and_then(Value::as_object_mut)
                .unwrap()
                .remove(key);
            r
        };
        let with = |pointer: &str, value: Value| {
            let mut r = good.clone();
            *r.pointer_mut(pointer).unwrap() = value;
            r
        };
        for (why, reply) in [
            ("no services", without("", "services")),
            ("no service", with("/services", json!([]))),
            ("services not a list", with("/services", json!({}))),
            ("no port", without("/services/0", "port")),
            ("port 0", with("/services/0/port", json!(0))),
            ("port past 65535", with("/services/0/port", json!(70000))),
            ("port as text", with("/services/0/port", json!("8443"))),
            ("no hostname", without("/services/0", "hostname")),
            ("empty hostname", with("/services/0/hostname", json!("."))),
            ("no addresses", without("/services/0", "addresses")),
            ("no address", with("/services/0/addresses", json!([]))),
            (
                "family and length disagree",
                with(
                    "/services/0/addresses",
                    json!([{"family": 10, "address": [10, 0, 0, 2]}]),
                ),
            ),
            (
                "a byte past 255",
                with(
                    "/services/0/addresses",
                    json!([{"family": 2, "address": [10, 0, 0, 256]}]),
                ),
            ),
            ("txt not a list", with("/txt", json!("url=http://e:1"))),
            ("not an object", json!("services")),
        ] {
            assert_eq!(advert_from_resolved("edge", &reply), None, "{why}");
        }
        // One bad address beside a good one costs only itself.
        let mixed = with(
            "/services/0/addresses",
            json!([{"family": 2, "address": [1, 2]},
                   {"family": 2, "address": [10, 0, 0, 3]}]),
        );
        assert_eq!(
            advert_from_resolved("edge", &mixed).unwrap().address,
            "10.0.0.3"
        );
        // TXT keys are case-insensitive, and an entry that is not a string
        // is skipped.
        let txt = with("/txt", json!([7, "URL=http://e:2", "Rathole=e:2333"]));
        let a = advert_from_resolved("edge", &txt).unwrap();
        assert_eq!(a.url.as_deref(), Some("http://e:2"));
        assert_eq!(a.rathole.as_deref(), Some("e:2333"));
    }

    #[test]
    fn other_service_types_unnamed_and_malformed_entries_are_skipped() {
        let mut present = Present::new();
        apply_browse(
            &mut present,
            &json!({"browserServiceData": [
                {"updateFlag": "added", "family": 2, "name": "printer",
                 "type": "_ipp._tcp", "domain": "local", "ifindex": 2},
                // resolved leaves the name out of a plain SRV service.
                {"updateFlag": "added", "family": 2,
                 "type": "_losos-edge._tcp", "domain": "local", "ifindex": 2},
                {"updateFlag": "added", "family": 2, "name": "edge",
                 "type": "_losos-edge._tcp", "domain": "local", "ifindex": -1},
                {"updateFlag": "changed", "family": 2, "name": "edge",
                 "type": "_losos-edge._tcp", "domain": "local", "ifindex": 2},
                {"updateFlag": "added", "family": "2", "name": "edge",
                 "type": "_losos-edge._tcp", "domain": "local", "ifindex": 2},
                "added"
            ]}),
        );
        assert!(present.is_empty(), "{present:?}");
        // A reply of another shape altogether changes nothing either.
        apply_browse(&mut present, &json!({"browserServiceData": {}}));
        apply_browse(&mut present, &Value::Null);
        assert!(to_resolve(&present).is_empty());
    }

    #[test]
    fn an_instance_stays_until_its_last_family_is_removed() {
        let entry = |flag: &str, family: i64, ifindex: u32| {
            json!({"updateFlag": flag, "family": family, "name": "edge",
                   "type": SERVICE_TYPE, "domain": "local", "ifindex": ifindex})
        };
        let mut present = Present::new();
        apply_browse(
            &mut present,
            &json!({"browserServiceData": [
                entry("added", 2, 2), entry("added", 10, 2), entry("added", 2, 3),
                // resolved repeats itself after a cache revisit; still one.
                entry("added", 2, 2)
            ]}),
        );
        let links: Vec<u32> = to_resolve(&present).iter().map(|i| i.ifindex).collect();
        assert_eq!(links, [2, 3]);
        apply_browse(
            &mut present,
            &json!({"browserServiceData": [entry("removed", 10, 2), entry("removed", 2, 3)]}),
        );
        let links: Vec<u32> = to_resolve(&present).iter().map(|i| i.ifindex).collect();
        assert_eq!(links, [2]);
        apply_browse(
            &mut present,
            &json!({"browserServiceData": [entry("removed", 2, 2)]}),
        );
        assert!(to_resolve(&present).is_empty());
        // Removing what was never added is not an error.
        apply_browse(
            &mut present,
            &json!({"browserServiceData": [entry("removed", 2, 9)]}),
        );
        assert!(present.is_empty());
    }

    #[test]
    fn one_edge_seen_on_two_interfaces_is_one_candidate() {
        let url = ["url=http://edge.local:8443"];
        let adverts: Vec<Advert> = [
            ("edge", resolved("edge.local", 8443, &["10.0.0.2"], &url)),
            ("edge", resolved("edge.local", 8443, &["10.0.1.2"], &url)),
            ("other", resolved("other.local", 8443, &["10.0.0.3"], &[])),
        ]
        .iter()
        .filter_map(|(name, reply)| advert_from_resolved(name, reply))
        .collect();
        let c = candidates(&adverts);
        assert_eq!(c.len(), 2, "{c:?}");
        assert_eq!(c[0].url, "http://edge.local:8443");
        assert_eq!(c[1].url, "http://10.0.0.3:8443");
        assert!(c.iter().all(|e| e.source == Source::Lan));
    }

    #[test]
    fn a_candidate_counts_only_when_its_health_answers() {
        let status = assemble(
            Some(&browse()),
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
            Some(&browse()),
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
            Some(&browse()),
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
            Some(&browse()),
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
    fn a_varlink_call_is_json_and_a_nul() {
        let bytes = varlink_call(
            "io.systemd.Resolve.BrowseServices",
            browse_parameters(),
            true,
        );
        assert_eq!(bytes.last(), Some(&0));
        assert_eq!(bytes.iter().filter(|b| **b == 0).count(), 1);
        let call: Value = serde_json::from_slice(&bytes[..bytes.len() - 1]).unwrap();
        assert_eq!(call["method"], "io.systemd.Resolve.BrowseServices");
        assert_eq!(
            call["parameters"],
            json!({"domain": "local", "type": "_losos-edge._tcp", "ifindex": 0, "flags": 0})
        );
        assert_eq!(call["more"], true);

        let instance = Instance {
            name: "edge demo".into(),
            service_type: SERVICE_TYPE.into(),
            domain: "local".into(),
            ifindex: 2,
        };
        let once = varlink_call(
            "io.systemd.Resolve.ResolveService",
            resolve_parameters(&instance),
            false,
        );
        let call: Value = serde_json::from_slice(&once[..once.len() - 1]).unwrap();
        assert!(call.get("more").is_none());
        assert_eq!(
            call["parameters"],
            json!({"name": "edge demo", "type": "_losos-edge._tcp", "domain": "local",
                   "ifindex": 2, "family": 0, "flags": 0})
        );
    }

    #[test]
    fn varlink_replies_errors_and_garbage() {
        let reply = parse_varlink_reply(BROWSE_REPLY).unwrap();
        assert!(reply.continues);
        assert_eq!(reply.parameters["browserServiceData"][1]["family"], 2);
        let last = parse_varlink_reply(br#"{"parameters":{"a":1}}"#).unwrap();
        assert!(!last.continues);
        // sd-varlink leaves an empty parameters object out of the message.
        let empty = parse_varlink_reply(br#"{"continues":true}"#).unwrap();
        assert_eq!(empty.parameters, Value::Null);
        assert_eq!(
            parse_varlink_reply(br#"{"error":"io.systemd.Resolve.NoSuchResourceRecord"}"#),
            Err("io.systemd.Resolve.NoSuchResourceRecord".to_string())
        );
        assert_eq!(
            parse_varlink_reply(
                br#"{"error":"org.varlink.service.InvalidParameter","parameters":{"parameter":"type"}}"#
            ),
            Err(r#"org.varlink.service.InvalidParameter {"parameter":"type"}"#.to_string())
        );
        assert!(parse_varlink_reply(b"not json").is_err());
        assert!(parse_varlink_reply(b"[1]").is_err());
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
            Some(&browse()),
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
            Some(&browse()),
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

    /// A gateway edge's resolve reply, with the `rathole=` record and the
    /// other TXT strings `losos.edge.lan.advertise` publishes.
    fn gateway(rathole: &str) -> Vec<Advert> {
        let reply = resolved(
            "gw.local",
            8443,
            &["192.168.1.2"],
            &[
                "url=http://gw.local:8443",
                &format!("rathole={rathole}"),
                "enrol=open",
                "txtvers=1",
            ],
        );
        advert_from_resolved("gateway", &reply)
            .into_iter()
            .collect()
    }

    #[test]
    fn the_advert_carries_the_tunnel_endpoint_and_an_old_one_gets_the_default_port() {
        let c = candidates(&gateway("gw.local:2333"));
        assert_eq!(c[0].rathole.as_deref(), Some("gw.local:2333"));
        let c = candidates(&browse());
        assert_eq!(c[0].rathole.as_deref(), Some("edge.local:2333"));
        // A malformed record is ignored, not dialled.
        let c = candidates(&gateway("gw.local:2333;rm"));
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
            Some(&gateway("gw.local:2333")),
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
            Some(&gateway("gw.local:2333")),
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

        let none = assemble(Some(&browse()), None, |_| false, &NO_TRUST, |_, _| None, 1);
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
