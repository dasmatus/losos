//! Finding an edge proxy, and refusing to share storage without one.
//!
//! The mesh is other people's boxes behind an edge: the rke2 server that
//! Longhorn replicates to, the tunnel that makes the box reachable,
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
//! only: an owner changing the hostname during an outage must not be told
//! off about storage. Local use keeps working throughout: nothing here
//! touches LosOS Cloud, LosOS Git or the box's own disk.
//!
//! **Pausing** is [`sharing_step`]: a box that shares through an edge and
//! then finds none for `losos.proxy.pauseSharingAfter` seconds turns sharing
//! off by itself, the way the owner's switch would, and remembers that it
//! did (`State::sharing_pause`). Once an edge has answered for
//! [`RESUME_AFTER`] it turns sharing back on. Only a pause it made itself is
//! undone: the owner switching sharing on or off in the meantime forgets the
//! pause. A box that never reached an edge while sharing (sharing set in its
//! configuration with no edge anywhere, as the VM tests of the shared side
//! do) is never paused, because there is no outage to speak of.
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

// The path rule and the shapes it reads and writes are their own crate,
// so that LosOS Desktop's network machine chooses an edge the same way.
pub use losos_edge_path::*;

/// The DNS-SD service type an edge advertises on the LAN.
pub const SERVICE_TYPE: &str = "_losos-edge._tcp";

/// How often the daemon looks again. Short enough that an edge unplugged
/// during a demo is noticed inside a minute; long enough that a browse plus
/// two probes per cycle is nothing.
pub const SCAN_INTERVAL: Duration = Duration::from_secs(20);

/// The two units modules/proxy.nix runs against the chosen edge, in the
/// order they are restarted: the tunnel first, then the announce that asks
/// the edge to route to it.
pub const PATH_UNITS: [&str; 2] = [
    "losos-tunnel-client.service",
    "losos-registrar-announce.service",
];

/// The mesh units modules/cluster.nix runs, restarted in this order when the
/// box moves to another secondary cluster: the join that fetches the new
/// cluster's token, then the agent that dials it.
pub const MESH_UNITS: [&str; 2] = ["losos-mesh-join.service", "rke2-agent.service"];

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

/// Refused: `losos.sharingLocally` was being turned on with no local gateway
/// in reach. Sharing as `kindashared` goes to the gateways of the box's own
/// network and nowhere else, so an official edge does not open it. Rendered
/// as a 409 by the HTTP layer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "no local gateway is reachable from this box, so losos.sharingLocally cannot be \
     turned on: sharing with this network goes only through a gateway on it, never \
     through an official edge. Local use keeps working."
)]
pub struct LocalGatewayRequired;

/// The local half of the gate: `Ok` unless `desired` turns
/// `losos.sharingLocally` on while no local gateway answers. Settings already
/// on pass, like [`check_gate`]'s, so a box can always switch it off.
///
/// # Errors
/// [`LocalGatewayRequired`].
pub fn check_local_gate(
    current: &Settings,
    desired: &Settings,
    edge: &EdgeStatus,
) -> Result<(), LocalGatewayRequired> {
    if desired.sharing_locally && !current.sharing_locally && !edge.local_gateway() {
        return Err(LocalGatewayRequired);
    }
    Ok(())
}

/// How long an edge must have answered before a paused box shares again:
/// three scans, so an edge that flaps does not cost a rebuild each way.
pub const RESUME_AFTER: u64 = 60;

/// The default of `losos.proxy.pauseSharingAfter`: ten minutes without an
/// edge, long enough that an edge's own reboot or a router's does not pause
/// anything, short enough that a box whose edge is gone stops waiting on it
/// within the hour.
pub const PAUSE_AFTER_DEFAULT: u64 = 600;

/// Since when the scans have answered one way, kept by the scanner thread
/// between scans. Both start empty, so a daemon restart counts again from
/// its first scan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OutageClock {
    unreachable_since: Option<u64>,
    reachable_since: Option<u64>,
}

impl OutageClock {
    /// Take one scan's answer at `now` (Unix seconds).
    pub fn observe(&mut self, reachable: bool, now: u64) {
        if reachable {
            self.unreachable_since = None;
            self.reachable_since.get_or_insert(now);
        } else {
            self.reachable_since = None;
            self.unreachable_since.get_or_insert(now);
        }
    }

    fn unreachable_for(&self, now: u64) -> Option<u64> {
        self.unreachable_since.map(|t| now.saturating_sub(t))
    }

    fn reachable_for(&self, now: u64) -> Option<u64> {
        self.reachable_since.map(|t| now.saturating_sub(t))
    }
}

/// What the box's sharing looks like to [`sharing_step`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharingNow {
    /// `losos.sharingMyStorage` in `overrides.nix`.
    pub on: bool,
    /// The box paused sharing itself and has not undone it yet.
    pub paused: bool,
    /// An edge has answered at least once since sharing was turned on.
    pub edge_seen: bool,
    /// A rebuild is running: nothing is written over it.
    pub building: bool,
}

/// What the scanner does about sharing after a scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharingStep {
    Nothing,
    /// Sharing is on and an edge answered: remember that one did.
    SeenEdge,
    /// No edge for too long: turn sharing off and remember it was us.
    Pause,
    /// An edge is back: turn sharing on again.
    Resume,
    /// The owner switched sharing on during the pause: it is theirs again.
    Forget,
}

/// The rule, with `pause_after` 0 meaning never pause. Call it after
/// [`OutageClock::observe`].
pub fn sharing_step(
    sharing: SharingNow,
    clock: &OutageClock,
    now: u64,
    pause_after: u64,
) -> SharingStep {
    if sharing.paused && sharing.on {
        return SharingStep::Forget;
    }
    if sharing.building {
        return SharingStep::Nothing;
    }
    if sharing.paused {
        return match clock.reachable_for(now) {
            Some(up) if up >= RESUME_AFTER => SharingStep::Resume,
            _ => SharingStep::Nothing,
        };
    }
    if !sharing.on {
        return SharingStep::Nothing;
    }
    if clock.reachable_for(now).is_some() {
        return if sharing.edge_seen {
            SharingStep::Nothing
        } else {
            SharingStep::SeenEdge
        };
    }
    match clock.unreachable_for(now) {
        Some(down) if pause_after > 0 && sharing.edge_seen && down >= pause_after => {
            SharingStep::Pause
        }
        _ => SharingStep::Nothing,
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
        tunnel: record("tunnel"),
    })
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
pub fn path_env(path: &EdgePath, tunnel_pin: Option<&str>) -> String {
    let mut out = format!(
        "LOSOS_EDGE_PATH_URL={}\nLOSOS_EDGE_PATH_TUNNEL={}\nLOSOS_EDGE_PATH_SOURCE={}\nLOSOS_EDGE_PATH_TIER={}\n",
        path.url,
        path.tunnel,
        match path.source {
            Source::Lan => "lan",
            Source::Configured => "configured",
        },
        path.tier.as_str(),
    );
    // A DNS label, checked when it was read off the edge.
    if let Some(cluster) = &path.cluster {
        out.push_str(&format!("LOSOS_EDGE_PATH_CLUSTER={cluster}\n"));
    }
    if let Some(cluster) = &path.official_cluster {
        out.push_str(&format!("LOSOS_EDGE_PATH_OFFICIAL_CLUSTER={cluster}\n"));
    }
    if let Some(pin) = tunnel_pin {
        out.push_str(&format!("LOSOS_EDGE_PATH_TUNNEL_PIN={pin}\n"));
    }
    out
}

/// The path an env file [`path_env`] wrote describes, as far as it says:
/// what a box held by a custom domain keeps dialling. `None` for a file
/// without a URL and a tunnel endpoint.
#[must_use]
pub fn path_from_env(env: &str) -> Option<EdgePath> {
    let url = env_value(env, "LOSOS_EDGE_PATH_URL")?.to_string();
    let tunnel = env_value(env, "LOSOS_EDGE_PATH_TUNNEL")?.to_string();
    if url.is_empty() || !plain_endpoint(&tunnel) {
        return None;
    }
    let source = match env_value(env, "LOSOS_EDGE_PATH_SOURCE") {
        Some("configured") => Source::Configured,
        _ => Source::Lan,
    };
    let tier = match env_value(env, "LOSOS_EDGE_PATH_TIER") {
        Some("tertiary") => Tier::Tertiary,
        Some("secondary") => Tier::Secondary,
        _ if source == Source::Configured => Tier::Tertiary,
        _ => Tier::Secondary,
    };
    Some(EdgePath {
        name: bracketed_host(&url),
        cluster: env_value(env, "LOSOS_EDGE_PATH_CLUSTER")
            .filter(|c| cluster_name(c))
            .map(str::to_string),
        member: None,
        official_cluster: env_value(env, "LOSOS_EDGE_PATH_OFFICIAL_CLUSTER")
            .filter(|c| dns_label(c))
            .map(str::to_string),
        url,
        source,
        tunnel,
        tier,
    })
}

/// One variable out of an env file [`path_env`] wrote.
#[must_use]
pub fn env_value<'a>(env: &'a str, key: &str) -> Option<&'a str> {
    env.lines()
        .filter_map(|l| l.split_once('='))
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v)
}

/// Whether a new path moves the box to another secondary cluster: both the
/// old and the new path run through one, and not the same. That is when the
/// mesh join runs again (the box joins the new cluster's control plane); a
/// failover inside a cluster, or out to an official edge and back, is not.
#[must_use]
pub fn moved_cluster(prev_env: Option<&str>, next_env: Option<&str>) -> bool {
    fn cluster(env: Option<&str>) -> Option<&str> {
        env.and_then(|e| env_value(e, "LOSOS_EDGE_PATH_CLUSTER"))
    }
    matches!((cluster(prev_env), cluster(next_env)), (Some(a), Some(b)) if a != b)
}

/// The file name a LAN edge's tunnel certificate is pinned under: its registrar's
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
    format!("{}.pin", safe.trim_matches('_'))
}

/// Refused: the owner asked to move the box to a secondary cluster none of
/// whose gateways is in reach.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("no gateway of the cluster {0} answers on this network, so the box cannot move there")]
pub struct ClusterNotInReach(pub String);

/// Refused: a live custom domain holds the box to the edge that serves it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("this box has a custom domain served through {0}, so it stays on that edge; remove the domain first to move the box")]
pub struct HeldByDomain(pub String);

/// Check a move to `cluster` against the last scan.
///
/// # Errors
/// [`HeldByDomain`] while a custom domain holds the box, and
/// [`ClusterNotInReach`] when no gateway of `cluster` answered.
pub fn check_cluster_move(status: &EdgeStatus, cluster: &str) -> anyhow::Result<()> {
    if let Some(url) = &status.held {
        return Err(HeldByDomain(url.clone()).into());
    }
    if status.clusters.iter().any(|c| c.name == cluster) {
        Ok(())
    } else {
        Err(ClusterNotInReach(cluster.to_string()).into())
    }
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

// ── gateways: an edge that proves what it runs ────────────────────────────
//
// Anyone may run an edge. A box trusts one with trading, and calls it
// `official`, only when the edge proves with its TPM that it runs a LosOS
// gateway image this box knows, under its owner's signed OpenPGP passport,
// paying LosOS's share to the addresses this box ships, with nothing changed
// since it booted (backend/access/src/gateway/check.rs for the checks,
// edge/registrar/src/gateway.rs for the gateway's half). Two round trips:
// `GET /passport?nonce=` and `POST /passport/attest`.
//
// A gateway that proves it was tampered with is disabled on this box for
// good, by its URL, its TPM and its owner's key, in
// `/var/lib/losos/gateways.json`. Every other refusal (an image or a TPM
// maker the box does not know yet, changed payout addresses) is shown and
// asked again on the next check. A trusted gateway is not asked again for
// [`TRUST_SECS`].

pub use losos_access::gateway::check::Anchors;
use losos_access::gateway::check::{self as gwcheck, AttestAnswer, Hello, Refusal};

/// How long a trusted gateway is believed without asking again.
pub const TRUST_SECS: u64 = 600;
/// The most a gateway's answer may weigh: the event log is the large part.
pub const GATEWAY_BODY_MAX: usize = 1 << 20;

/// One gateway this box will not use again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisabledGateway {
    pub url: String,
    /// The TPM's endorsement key name ([`gwcheck::Introduced::ek_id`]).
    pub ek_id: String,
    /// The owner's OpenPGP fingerprint.
    pub fingerprint: String,
    pub code: String,
    pub reason: String,
    pub since: u64,
}

/// The disabled gateways, as `/var/lib/losos/gateways.json` keeps them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    #[serde(default)]
    pub disabled: Vec<DisabledGateway>,
}

impl Ledger {
    /// The entry that disables `url`, or the TPM or owner key a gateway at
    /// another URL presents.
    #[must_use]
    pub fn find(
        &self,
        url: &str,
        ek_id: Option<&str>,
        fingerprint: Option<&str>,
    ) -> Option<&DisabledGateway> {
        self.disabled.iter().find(|d| {
            d.url == url
                || ek_id.is_some_and(|e| e == d.ek_id)
                || fingerprint.is_some_and(|f| f.eq_ignore_ascii_case(&d.fingerprint))
        })
    }
}

impl From<&DisabledGateway> for Gateway {
    fn from(d: &DisabledGateway) -> Self {
        Gateway::Disabled {
            code: d.code.clone(),
            reason: d.reason.clone(),
            since: d.since,
        }
    }
}

/// The two requests, as the I/O layer makes them.
pub trait Wire {
    /// `GET {url}/passport?nonce={nonce}`: `Ok(None)` when the edge has no
    /// passport to show (404), the body when it answered.
    ///
    /// # Errors
    /// The request failed: the edge is not judged this time.
    fn hello(&self, url: &str, nonce: &str) -> Result<Option<String>, String>;

    /// `POST {url}/passport/attest` with `body`.
    ///
    /// # Errors
    /// The request failed or the edge answered an error.
    fn attest(&self, url: &str, body: &str) -> Result<String, String>;
}

/// The box's gateway checks across scans: what it trusts (`anchors`, `None`
/// when it ships none, and then no edge is asked anything), what it
/// disabled, and whom it trusted lately.
pub struct Vetter {
    pub anchors: Option<Anchors>,
    pub ledger: Ledger,
    trusted: std::collections::HashMap<String, (Gateway, u64)>,
}

impl Vetter {
    #[must_use]
    pub fn new(anchors: Option<Anchors>, ledger: Ledger) -> Self {
        Vetter {
            anchors,
            ledger,
            trusted: std::collections::HashMap::new(),
        }
    }

    /// Check the edge at `url`. Returns what to show, and whether the ledger
    /// changed, which the caller then writes down.
    pub fn vet(
        &mut self,
        url: &str,
        wire: &impl Wire,
        nonce: &str,
        now: u64,
        rng: &mut (impl rand::CryptoRng + rand::RngCore),
    ) -> (Option<Gateway>, bool) {
        if let Some(d) = self.ledger.find(url, None, None) {
            return (Some(d.into()), false);
        }
        let Some(anchors) = &self.anchors else {
            return (None, false);
        };
        if let Some((g, at)) = self.trusted.get(url) {
            if now.saturating_sub(*at) < TRUST_SECS {
                return (Some(g.clone()), false);
            }
        }
        self.trusted.remove(url);

        let body = match wire.hello(url, nonce) {
            Ok(Some(body)) => body,
            Ok(None) => return (None, false),
            Err(e) => {
                tracing::info!(url, error = %e, "the gateway's passport could not be fetched");
                return (None, false);
            }
        };
        let refused = |r: &Refusal| Gateway::Refused {
            code: r.code().to_string(),
            reason: r.to_string(),
        };
        let hello: Hello = match serde_json::from_str(&body) {
            Ok(h) => h,
            Err(e) => return (Some(refused(&Refusal::Malformed(e.to_string()))), false),
        };
        let intro = match gwcheck::introduce(
            url,
            nonce,
            &hello,
            anchors,
            i64::try_from(now).unwrap_or(i64::MAX),
        ) {
            Ok(i) => i,
            Err(r) => return (Some(refused(&r)), false),
        };
        if let Some(d) = self
            .ledger
            .find(url, Some(&intro.ek_id), Some(&intro.owner.fingerprint))
        {
            return (Some(d.into()), false);
        }
        let (challenge, req) = match gwcheck::challenge(&intro, nonce, rng) {
            Ok(c) => c,
            Err(r) => return (Some(refused(&r)), false),
        };
        let req = serde_json::to_string(&req).unwrap_or_default();
        let answer: AttestAnswer = match wire
            .attest(url, &req)
            .and_then(|b| serde_json::from_str(&b).map_err(|e| e.to_string()))
        {
            Ok(a) => a,
            Err(e) => {
                tracing::info!(url, error = %e, "the gateway did not attest");
                return (None, false);
            }
        };
        match gwcheck::judge(
            &intro,
            nonce,
            &challenge,
            &answer,
            &hello.tpm.ak_public,
            anchors,
        ) {
            Ok(verdict) => {
                let g = Gateway::Trusted {
                    owner: intro.owner.user_id.clone(),
                    fingerprint: intro.owner.fingerprint.clone(),
                    image: verdict.image,
                };
                self.trusted.insert(url.to_string(), (g.clone(), now));
                (Some(g), false)
            }
            Err(r) if r.tampered() => {
                tracing::warn!(url, reason = %r, "the gateway is tampered with; disabled on this box");
                let d = DisabledGateway {
                    url: url.to_string(),
                    ek_id: intro.ek_id.clone(),
                    fingerprint: intro.owner.fingerprint.clone(),
                    code: r.code().to_string(),
                    reason: r.to_string(),
                    since: now,
                };
                let g = Gateway::from(&d);
                self.ledger.disabled.push(d);
                (Some(g), true)
            }
            Err(r) => (Some(refused(&r)), false),
        }
    }
}

// ── the official edge's domain ─────────────────────────────────────────────
//
// A gateway that passes every check above is official only at a name that
// is DNSSEC-signed and carries its Encrypted Client Hello key: the HTTPS
// record an official edge's zone writes at every name in it
// (edge/registrar/src/zone.rs). The passport names the URL, so the name is
// the one the owner signed for; the lookup is `delv`'s, which validates from
// a trust anchor itself rather than believing a resolver's AD bit.

/// What a validating lookup of a name's HTTPS record showed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct DomainCheck {
    /// The answer, or the proof that there is none, validated from a
    /// trust anchor.
    pub signed: bool,
    /// A validated HTTPS record carries an `ech=` key.
    pub ech: bool,
}

impl DomainCheck {
    /// Both hold.
    #[must_use]
    pub fn passed(self) -> bool {
        self.signed && self.ech
    }
}

/// Read `delv <name> HTTPS`'s output. delv marks a validated answer
/// `; fully validated` and a validated absence
/// `; negative response, fully validated`; anything else (`; unsigned
/// answer`, a resolution failure) is not signed.
#[must_use]
pub fn parse_delv(out: &str) -> DomainCheck {
    let signed = out.lines().map(str::trim).any(|l| {
        l.starts_with(';') && l.ends_with("fully validated") && !l.contains("not fully validated")
    });
    let ech = signed
        && out.lines().map(str::trim).any(|l| {
            !l.starts_with(';')
                && l.split_whitespace().nth(3) == Some("HTTPS")
                && l.contains("ech=")
        });
    DomainCheck { signed, ech }
}

/// The DNS name an edge's URL is at: its host without a port, lower case.
/// `None` for a plain `http://` URL, an address, or a name with no dot,
/// none of which can carry a signed record.
#[must_use]
pub fn domain_of(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let host = rest.split('/').next().unwrap_or(rest);
    if host.starts_with('[') {
        return None;
    }
    let name = host
        .split(':')
        .next()
        .unwrap_or(host)
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let label_ok = |l: &str| {
        !l.is_empty()
            && l.len() <= 63
            && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            && !l.starts_with('-')
            && !l.ends_with('-')
    };
    let ok = name.len() <= 253
        && name.contains('.')
        && name.parse::<std::net::IpAddr>().is_err()
        && name.split('.').all(label_ok);
    ok.then_some(name)
}

/// Hold a gateway that passed every other check to its domain: it stays
/// trusted only when its URL is an `https://` name whose HTTPS record
/// validates under DNSSEC and carries an ECH key. Anything else is refused,
/// not disabled: a DS record that is not published yet is the operator's
/// to fix, and the next scan asks again. `check` is asked only for a
/// trusted gateway.
pub fn hold_to_domain(
    url: &str,
    gateway: Option<Gateway>,
    check: impl FnOnce(&str) -> DomainCheck,
) -> Option<Gateway> {
    if !matches!(gateway, Some(Gateway::Trusted { .. })) {
        return gateway;
    }
    let refused = |code: &str, reason: String| {
        Some(Gateway::Refused {
            code: code.to_string(),
            reason,
        })
    };
    let Some(name) = domain_of(url) else {
        return refused(
            "domainNotHttps",
            format!("an official edge is reached at an https:// name, and {url} is not one"),
        );
    };
    let c = check(&name);
    if !c.signed {
        return refused(
            "domainUnsigned",
            format!("{name} does not validate under DNSSEC"),
        );
    }
    if !c.ech {
        return refused(
            "domainNoEch",
            format!("{name} has no signed HTTPS record with an Encrypted Client Hello key"),
        );
    }
    gateway
}

/// Refused: trading was asked of a box with no checked gateway in reach.
/// Rendered as a 409 by the HTTP layer; the message is the sentence the
/// owner reads.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "no edge this box could check is in reach, so trading is off: only a \
     gateway that proves it runs a published LosOS image and pays LosOS's \
     share may process the market. Sharing storage through any edge keeps \
     working."
)]
pub struct OfficialEdgeRequired;

/// The market gate: trading needs a checked gateway.
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

// ── the box's own TLS on its public names ──────────────────────────────
//
// modules/public-tls.nix gets a certificate for each public name and writes
// two files: the names the box ends TLS for itself (what the heartbeat asks
// the edge to pass through), and what its hourly look at each name from
// outside found. `GET /api/edge` carries both as `publicTls`, so the Mesh
// pane can say whether the edge can read the box's traffic.

/// What a look at one public name from outside found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TlsSeen {
    /// The box's own certificate answered: the edge passes TLS through.
    Ok,
    /// Nothing answered, or not with TLS.
    Unreachable,
    /// A certificate the box did not make: something between the visitor
    /// and the box ends its TLS.
    Foreign,
}

/// `publicTls` in `GET /api/edge`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicTls {
    /// The names this box ends TLS for itself. `None`: the edge still ends
    /// it for every name (no certificate yet, or passthrough is off).
    pub own: Option<Vec<String>>,
    /// When the last look from outside ran (RFC 3339), if one has.
    pub checked_at: Option<String>,
    /// What it found, by name.
    pub seen: std::collections::BTreeMap<String, TlsSeen>,
}

/// [`PublicTls`] from the two files' bodies (`None`: no such file). Names
/// that are not DNS names, verdicts this build does not know and a probe
/// that is not JSON are left out rather than failing the whole status.
#[must_use]
pub fn public_tls_from(ready: Option<&str>, probe: Option<&str>) -> PublicTls {
    let own = ready.map(|body| {
        body.lines()
            .map(str::trim)
            .filter(|n| public_name(n))
            .map(str::to_string)
            .collect()
    });
    let probe: Option<Value> = probe.and_then(|p| serde_json::from_str(p).ok());
    let checked_at = probe
        .as_ref()
        .and_then(|p| p["checkedAt"].as_str())
        .map(str::to_string);
    let seen = probe
        .as_ref()
        .and_then(|p| p["names"].as_object())
        .map(|names| {
            names
                .iter()
                .filter(|(n, _)| public_name(n))
                .filter_map(|(n, v)| {
                    serde_json::from_value::<TlsSeen>(v.clone())
                        .ok()
                        .map(|v| (n.clone(), v))
                })
                .collect()
        })
        .unwrap_or_default();
    PublicTls {
        own,
        checked_at,
        seen,
    }
}

/// A lowercase DNS name of two labels or more.
fn public_name(n: &str) -> bool {
    n.len() <= 253 && n.contains('.') && n.split('.').all(dns_label)
}

// ── management: a company's own edge looking after this box ───────────────
//
// A local edge's web page can change this box's LosOS Cloud, LosOS Git and
// app settings, but only once the owner switched it on for that edge
// (`State::managed_by`, `POST /api/edge/manage`). The edge calls lososd
// through the box's own tunnel, which is pinned to the edge's certificate, and
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
    /// The edge in use was found on the LAN but its addresses are not on a
    /// network this box is attached to.
    #[error("the edge in use is not on this box's own network, so it may not manage the box")]
    OutsideNetwork,
}

/// The edge that may manage this box right now: the path, when it is an edge
/// found on the LAN whose registrar and tunnel endpoint are both on a network
/// this box is directly attached to. Checked again on every call the edge
/// makes ([`manager_active`]), so an edge whose name starts resolving
/// elsewhere loses access with the next scan. A gateway that passed its
/// checks is somebody's own edge like any other.
///
/// # Errors
/// What stands in the way.
pub fn manageable(status: &EdgeStatus) -> Result<&EdgePath, NotManageable> {
    let path = status
        .path
        .as_ref()
        .filter(|p| p.source == Source::Lan)
        .ok_or(NotManageable::NoLocalEdge)?;
    if status
        .edges
        .iter()
        .any(|e| e.url == path.url && e.on_network)
    {
        Ok(path)
    } else {
        Err(NotManageable::OutsideNetwork)
    }
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

    // ── public_tls_from ─────────────────────────────────────────────────

    #[test]
    fn no_ready_file_means_the_edge_still_ends_tls() {
        let t = public_tls_from(None, None);
        assert_eq!(t.own, None);
        assert_eq!(t.checked_at, None);
        assert!(t.seen.is_empty());
    }

    #[test]
    fn the_box_reports_its_own_names_and_what_it_saw_from_outside() {
        let t = public_tls_from(
            Some("mattbox.losos.cfd\nNot A Name\nfiles.example.org\n"),
            Some(
                r#"{"checkedAt":"2026-10-28T21:00:00Z","names":{
                    "mattbox.losos.cfd":"ok","files.example.org":"foreign",
                    "../etc":"ok","x.example":"sideways"}}"#,
            ),
        );
        assert_eq!(
            t.own.as_deref(),
            Some(
                &[
                    "mattbox.losos.cfd".to_string(),
                    "files.example.org".to_string()
                ][..]
            )
        );
        assert_eq!(t.checked_at.as_deref(), Some("2026-10-28T21:00:00Z"));
        assert_eq!(t.seen.len(), 2);
        assert_eq!(t.seen["mattbox.losos.cfd"], TlsSeen::Ok);
        assert_eq!(t.seen["files.example.org"], TlsSeen::Foreign);
        // An empty file is still the box ending TLS: it lapsed, and the
        // edge does not take it back.
        assert_eq!(
            public_tls_from(Some(""), Some("not json")).own,
            Some(vec![])
        );
    }

    // ── sharing_step ────────────────────────────────────────────────────

    fn sharing(on: bool, paused: bool) -> SharingNow {
        SharingNow {
            on,
            paused,
            edge_seen: true,
            building: false,
        }
    }

    fn clock(reachable: bool, since: u64) -> OutageClock {
        let mut c = OutageClock::default();
        c.observe(reachable, since);
        c
    }

    #[test]
    fn sharing_pauses_only_after_the_whole_wait() {
        let down = clock(false, 100);
        assert_eq!(
            sharing_step(sharing(true, false), &down, 699, 600),
            SharingStep::Nothing
        );
        assert_eq!(
            sharing_step(sharing(true, false), &down, 700, 600),
            SharingStep::Pause
        );
        // 0 is never.
        assert_eq!(
            sharing_step(sharing(true, false), &down, 99_999, 0),
            SharingStep::Nothing
        );
    }

    #[test]
    fn one_answer_restarts_the_wait() {
        let mut c = clock(false, 0);
        c.observe(true, 500);
        c.observe(false, 520);
        assert_eq!(
            sharing_step(sharing(true, false), &c, 700, 600),
            SharingStep::Nothing
        );
        assert_eq!(
            sharing_step(sharing(true, false), &c, 1120, 600),
            SharingStep::Pause
        );
    }

    #[test]
    fn a_paused_box_resumes_after_a_minute_of_answers() {
        let up = clock(true, 1000);
        assert_eq!(
            sharing_step(sharing(false, true), &up, 1059, 600),
            SharingStep::Nothing
        );
        assert_eq!(
            sharing_step(sharing(false, true), &up, 1060, 600),
            SharingStep::Resume
        );
        let down = clock(false, 1000);
        assert_eq!(
            sharing_step(sharing(false, true), &down, 9000, 600),
            SharingStep::Nothing
        );
    }

    #[test]
    fn a_rebuild_in_flight_is_never_written_over() {
        let mut now = sharing(true, false);
        now.building = true;
        assert_eq!(
            sharing_step(now, &clock(false, 0), 10_000, 600),
            SharingStep::Nothing
        );
        let mut paused = sharing(false, true);
        paused.building = true;
        assert_eq!(
            sharing_step(paused, &clock(true, 0), 10_000, 600),
            SharingStep::Nothing
        );
    }

    #[test]
    fn the_owners_switch_ends_a_pause_and_unseen_edges_never_start_one() {
        assert_eq!(
            sharing_step(sharing(true, true), &clock(true, 0), 0, 600),
            SharingStep::Forget
        );
        let mut unseen = sharing(true, false);
        unseen.edge_seen = false;
        assert_eq!(
            sharing_step(unseen, &clock(false, 0), 10_000, 600),
            SharingStep::Nothing
        );
        assert_eq!(
            sharing_step(unseen, &clock(true, 0), 0, 600),
            SharingStep::SeenEdge
        );
        // Sharing off and never paused: the owner's choice, left alone.
        assert_eq!(
            sharing_step(sharing(false, false), &clock(true, 0), 10_000, 600),
            SharingStep::Nothing
        );
    }

    use super::*;
    use std::net::IpAddr;

    fn lan_edge(url: &str, official: bool) -> Edge {
        Edge {
            name: "office".into(),
            url: url.into(),
            source: Source::Lan,
            official,
            gateway: None,
            tunnel: Some("192.168.1.2:2333".into()),
            on_network: true,
            tier: Tier::Secondary,
            cluster: None,
            member: None,
            official_cluster: None,
        }
    }

    #[test]
    fn only_a_local_edge_can_manage_checked_or_not() {
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

        let checked =
            EdgeStatus::found(vec![lan_edge("http://office.local:8443", true)], true, None);
        assert_eq!(
            manageable(&checked).map(|p| p.url.as_str()),
            Ok("http://office.local:8443")
        );

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
    fn an_edge_outside_the_boxs_network_may_not_manage_it() {
        let mut outside = lan_edge("http://office.local:8443", false);
        outside.on_network = false;
        let status = EdgeStatus::found(vec![outside], true, None);
        assert_eq!(manageable(&status), Err(NotManageable::OutsideNetwork));
        assert!(!manager_active(Some("http://office.local:8443"), &status));
    }

    const ROUTE: &str =
        "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\t\tMTU\tWindow\tIRTT\n\
eth0\t00000000\t0101A8C0\t0003\t0\t0\t100\t00000000\t0\t0\t0\n\
eth0\t0001A8C0\t00000000\t0001\t0\t0\t100\t00FFFFFF\t0\t0\t0\n\
lo\t0000007F\t00000000\t0001\t0\t0\t0\t000000FF\t0\t0\t0\n";

    const INET6: &str = "fe80000000000000505400fffe123456 02 40 20 80     eth0\n\
00000000000000000000000000000001 01 80 10 80       lo\n\
fd00000000000000505400fffe123456 02 40 00 00     eth0\n";

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn the_boxs_own_networks_come_from_proc() {
        let v4 = parse_proc_route(ROUTE);
        assert_eq!(v4.len(), 1, "{v4:?}");
        assert!(v4[0].contains(ip("192.168.1.2")));
        assert!(!v4[0].contains(ip("192.168.2.2")));
        let v6 = parse_if_inet6(INET6);
        assert_eq!(v6.len(), 2, "{v6:?}");
        assert!(v6.iter().any(|n| n.contains(ip("fd00::1"))));
        assert!(v6.iter().any(|n| n.contains(ip("fe80::1"))));
    }

    #[test]
    fn only_private_addresses_on_the_boxs_links_count_as_its_network() {
        let nets = parse_proc_route(ROUTE);
        assert!(on_own_network(&[ip("192.168.1.2")], &nets));
        // Another private network, routed to: not this box's link.
        assert!(!on_own_network(&[ip("192.168.2.2")], &nets));
        // A public address on a link is still somebody else's network.
        let public = vec![Net {
            addr: ip("203.0.113.0"),
            len: 24,
        }];
        assert!(!on_own_network(&[ip("203.0.113.5")], &public));
        // A global IPv6 prefix on the box's link is that link.
        let v6 = vec![Net {
            addr: ip("2001:db8:1::"),
            len: 64,
        }];
        assert!(on_own_network(&[ip("2001:db8:1::2")], &v6));
        assert!(!on_own_network(&[ip("2001:db8:2::2")], &v6));
        // Nothing resolved, or one address elsewhere, is not the network.
        assert!(!on_own_network(&[], &nets));
        assert!(!on_own_network(&[ip("192.168.1.2"), ip("10.0.0.2")], &nets));
    }

    #[test]
    fn an_edge_counts_as_on_the_network_when_registrar_and_tunnel_resolve_there() {
        let nets = parse_proc_route(ROUTE);
        let resolve = |h: &str| -> Vec<IpAddr> {
            match h {
                "office.local" | "192.168.1.2" => vec![ip("192.168.1.2")],
                "far.local" => vec![ip("192.168.2.2")],
                _ => vec![],
            }
        };
        let mut far_tunnel = lan_edge("http://office.local:8443", false);
        far_tunnel.tunnel = Some("far.local:2333".into());
        let mut configured = lan_edge("http://office.local:8443", false);
        configured.source = Source::Configured;
        let mut status = EdgeStatus::found(
            vec![
                lan_edge("http://office.local:8443", false),
                lan_edge("http://far.local:8443", false),
                far_tunnel,
                configured,
            ],
            true,
            None,
        );
        status.mark_on_network(&nets, resolve);
        let marks: Vec<bool> = status.edges.iter().map(|e| e.on_network).collect();
        assert_eq!(marks, [true, false, false, false]);
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
        let txt = with("/txt", json!([7, "URL=http://e:2", "Tunnel=e:2333"]));
        let a = advert_from_resolved("edge", &txt).unwrap();
        assert_eq!(a.url.as_deref(), Some("http://e:2"));
        assert_eq!(a.tunnel.as_deref(), Some("e:2333"));
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
            |_| None,
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
            |_| None,
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
            |_| None,
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
            |_| None,
            1,
        );
        assert_eq!(status.edges.len(), 1);
    }

    #[test]
    fn a_lan_that_could_not_be_searched_says_so() {
        let status = assemble(None, None, |_| true, |_| None, 1);
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

    fn locally(on: bool) -> Settings {
        Settings {
            sharing_locally: on,
            ..Settings::default()
        }
    }

    fn edge_of(tier: Tier) -> Edge {
        Edge {
            name: "gw".into(),
            url: "http://gw.local:8443".into(),
            source: Source::Lan,
            official: false,
            gateway: None,
            tunnel: Some("gw.local:2333".into()),
            on_network: true,
            tier,
            cluster: None,
            member: None,
            official_cluster: None,
        }
    }

    #[test]
    fn sharing_locally_needs_a_local_gateway_not_any_edge() {
        let none = EdgeStatus::found(Vec::new(), true, None);
        let official = EdgeStatus::found(vec![edge_of(Tier::Tertiary)], true, None);
        let gateway = EdgeStatus::found(vec![edge_of(Tier::Secondary)], true, None);
        assert!(!none.local_gateway());
        assert!(!official.local_gateway());
        assert!(gateway.local_gateway());
        assert_eq!(official.to_json()["localGateway"], false);
        assert_eq!(gateway.to_json()["localGateway"], true);

        let err = check_local_gate(&locally(false), &locally(true), &none).unwrap_err();
        assert!(err.to_string().contains("no local gateway is reachable"));
        // An official edge answers, and it is not enough.
        assert!(check_local_gate(&locally(false), &locally(true), &official).is_err());
        assert!(check_local_gate(&locally(false), &locally(true), &gateway).is_ok());
        // Already on, or being turned off, passes whatever answers.
        assert!(check_local_gate(&locally(true), &locally(true), &none).is_ok());
        assert!(check_local_gate(&locally(true), &locally(false), &none).is_ok());
        // The other kind of sharing is not this gate's business.
        assert!(check_local_gate(&settings(false, false), &settings(true, false), &none).is_ok());
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
                gateway: None,
                tunnel: Some("edge.local:2333".into()),
                on_network: true,
                tier: Tier::Secondary,
                cluster: None,
                member: None,
                official_cluster: None,
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

    // ── gateways ───────────────────────────────────────────────────────────
    //
    // The checks themselves are losos-access's (and the gateway VM test's);
    // these are what the box does around them.

    /// A wire that answers what it is told and counts the hellos.
    struct FakeWire {
        hello: Result<Option<String>, String>,
        asked: std::cell::Cell<u32>,
    }

    impl FakeWire {
        fn new(hello: Result<Option<String>, String>) -> Self {
            FakeWire {
                hello,
                asked: std::cell::Cell::new(0),
            }
        }
    }

    impl Wire for FakeWire {
        fn hello(&self, _url: &str, _nonce: &str) -> Result<Option<String>, String> {
            self.asked.set(self.asked.get() + 1);
            self.hello.clone()
        }
        fn attest(&self, _url: &str, _body: &str) -> Result<String, String> {
            Err("not asked in these tests".into())
        }
    }

    const GW: &str = "http://gw.local:8443";

    fn disabled_entry() -> DisabledGateway {
        DisabledGateway {
            url: GW.into(),
            ek_id: "000b".to_string() + &"aa".repeat(32),
            fingerprint: "ABCDEF0123456789ABCDEF0123456789ABCDEF01".into(),
            code: "tainted".into(),
            reason: "the gateway's settings or state changed after it booted (PCR 15)".into(),
            since: 1_900_000_000,
        }
    }

    #[test]
    fn a_box_with_no_anchors_asks_no_edge_anything() {
        let wire = FakeWire::new(Ok(Some("{}".into())));
        let mut v = Vetter::new(None, Ledger::default());
        assert_eq!(
            v.vet(GW, &wire, "00", 1, &mut rand::thread_rng()),
            (None, false)
        );
        assert_eq!(wire.asked.get(), 0);
    }

    #[test]
    fn an_edge_with_no_passport_or_out_of_reach_is_not_judged() {
        let mut v = Vetter::new(Some(Anchors::default()), Ledger::default());
        for answer in [Ok(None), Err("timed out".to_string())] {
            let wire = FakeWire::new(answer);
            assert_eq!(
                v.vet(GW, &wire, "00", 1, &mut rand::thread_rng()),
                (None, false)
            );
            assert_eq!(wire.asked.get(), 1);
        }
    }

    #[test]
    fn a_passport_that_does_not_hold_is_refused_and_asked_again() {
        let wire = FakeWire::new(Ok(Some("{\"settings\":1}".into())));
        let mut v = Vetter::new(Some(Anchors::default()), Ledger::default());
        for round in 1..=2 {
            let (g, changed) = v.vet(GW, &wire, "00", round, &mut rand::thread_rng());
            assert!(!changed);
            match g {
                Some(Gateway::Refused { code, .. }) => assert_eq!(code, "malformed"),
                other => panic!("{other:?}"),
            }
            assert_eq!(wire.asked.get(), u32::try_from(round).unwrap());
        }
        assert!(v.ledger.disabled.is_empty());
    }

    #[test]
    fn a_disabled_gateway_is_not_asked_and_is_never_the_path() {
        let wire = FakeWire::new(Ok(Some("{}".into())));
        let ledger = Ledger {
            disabled: vec![disabled_entry()],
        };
        let mut v = Vetter::new(Some(Anchors::default()), ledger.clone());
        let (g, changed) = v.vet(GW, &wire, "00", 5, &mut rand::thread_rng());
        assert!(!changed);
        assert_eq!(wire.asked.get(), 0);
        assert!(matches!(g, Some(Gateway::Disabled { ref code, .. }) if code == "tainted"));

        // The same TPM or owner key behind another URL is the same gateway.
        let e = disabled_entry();
        assert!(ledger
            .find("http://moved.local:8443", Some(&e.ek_id), None)
            .is_some());
        assert!(ledger
            .find(
                "http://moved.local:8443",
                None,
                Some(&e.fingerprint.to_lowercase())
            )
            .is_some());
        assert!(ledger
            .find("http://other.local:8443", Some("000b00"), Some("00"))
            .is_none());

        // The scan shows it, never tunnels through it, and falls back to the
        // configured edge.
        let status = assemble_with(
            Some(&gateway("gw.local:2333")),
            Some("https://losos-edge.example"),
            Some("edge.losos.cfd:2333"),
            |_| true,
            |_| None,
            |url| (url == GW).then(|| Gateway::from(&disabled_entry())),
            9,
        );
        assert!(status.edges[0].disabled());
        assert!(!status.official);
        assert_eq!(
            status.path.as_ref().expect("a path").source,
            Source::Configured
        );
        let doc = status.to_json();
        assert_eq!(doc["edges"][0]["gateway"]["state"], "disabled");
        assert_eq!(doc["edges"][0]["gateway"]["code"], "tainted");
        assert!(doc["edges"][1].get("gateway").is_none());
    }

    #[test]
    fn the_ledger_file_keeps_its_shape() {
        let ledger = Ledger {
            disabled: vec![disabled_entry()],
        };
        let text = serde_json::to_string(&ledger).unwrap();
        assert!(text.contains("\"ekId\""), "{text}");
        assert_eq!(serde_json::from_str::<Ledger>(&text).unwrap(), ledger);
        assert_eq!(
            serde_json::from_str::<Ledger>("{}").unwrap(),
            Ledger::default()
        );
    }

    #[test]
    fn a_trusted_gateway_is_official_and_opens_the_market() {
        let trusted = Gateway::Trusted {
            owner: "Ada Gateway <ada@example.net>".into(),
            fingerprint: "ABCDEF".into(),
            image: "losos-edge-gateway 0.6.0".into(),
        };
        let status = assemble(
            Some(&gateway("gw.local:2333")),
            Some("https://losos-edge.example"),
            |_| true,
            |url| (url == GW).then(|| trusted.clone()),
            1,
        );
        assert!(status.official);
        assert!(status.edges[0].official && !status.edges[1].official);
        assert!(check_market_gate(&status).is_ok());
        let doc = status.to_json();
        assert_eq!(doc["official"], true);
        assert_eq!(doc["edges"][0]["gateway"]["state"], "trusted");
        assert_eq!(
            doc["edges"][0]["gateway"]["owner"],
            "Ada Gateway <ada@example.net>"
        );

        let status = assemble(Some(&gateway("gw.local:2333")), None, |_| true, |_| None, 1);
        assert!(status.reachable && !status.official);
        assert_eq!(check_market_gate(&status), Err(OfficialEdgeRequired));
        assert!(OfficialEdgeRequired
            .to_string()
            .contains("proves it runs a published LosOS image"));
    }

    /// A gateway edge's resolve reply, with the `tunnel=` record and the
    /// other TXT strings `losos.edge.lan.advertise` publishes.
    fn gateway(tunnel: &str) -> Vec<Advert> {
        let reply = resolved(
            "gw.local",
            8443,
            &["192.168.1.2"],
            &[
                "url=http://gw.local:8443",
                &format!("tunnel={tunnel}"),
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
        assert_eq!(c[0].tunnel.as_deref(), Some("gw.local:2333"));
        let c = candidates(&browse());
        assert_eq!(c[0].tunnel.as_deref(), Some("edge.local:2333"));
        // A malformed record is ignored, not dialled.
        let c = candidates(&gateway("gw.local:2333;rm"));
        assert_eq!(c[0].tunnel.as_deref(), Some("gw.local:2333"));
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
            |_| None,
            |_| None,
            1,
        );
        let path = both.path.clone().expect("a path");
        assert_eq!(path.source, Source::Lan);
        assert_eq!(path.url, "http://gw.local:8443");
        assert_eq!(path.tunnel, "gw.local:2333");
        assert_eq!(both.to_json()["path"]["tunnel"], "gw.local:2333");

        let official_only = assemble_with(
            Some(&gateway("gw.local:2333")),
            Some("https://losos-edge.example"),
            Some("edge.losos.cfd:2333"),
            |url| url.starts_with("https://"),
            |_| None,
            |_| None,
            1,
        );
        let path = official_only.path.expect("a path");
        assert_eq!(path.source, Source::Configured);
        assert_eq!(path.tunnel, "edge.losos.cfd:2333");

        // A configured edge the box has no tunnel endpoint for can be found
        // but is not a path.
        let no_endpoint = assemble(
            None,
            Some("https://losos-edge.example"),
            |_| true,
            |_| None,
            1,
        );
        assert!(no_endpoint.reachable);
        assert_eq!(no_endpoint.path, None);

        let none = assemble(Some(&browse()), None, |_| false, |_| None, 1);
        assert_eq!(none.path, None);
        assert_eq!(none.to_json()["path"], Value::Null);
    }

    fn route(
        edges: &[Edge],
        member_of: Option<&str>,
        current: Option<&str>,
        hold: Option<&EdgePath>,
    ) -> (Option<EdgePath>, Option<String>) {
        let chosen = choose_path(
            edges,
            &PathRule {
                member_of,
                current,
                hold,
                seed: "box",
                bans: None,
            },
        );
        (chosen.path, chosen.cluster)
    }

    fn gw(cluster: &str, member: &str) -> Edge {
        Edge {
            name: member.into(),
            url: format!("http://{member}.local:8443"),
            source: Source::Lan,
            official: false,
            gateway: None,
            tunnel: Some(format!("{member}.local:2333")),
            on_network: true,
            tier: Tier::Secondary,
            cluster: Some(cluster.into()),
            member: Some(member.into()),
            official_cluster: None,
        }
    }

    fn official_edge() -> Edge {
        Edge {
            name: "edge".into(),
            url: "https://edge.losos.cfd".into(),
            source: Source::Configured,
            official: true,
            gateway: None,
            tunnel: Some("edge.losos.cfd:2333".into()),
            on_network: true,
            tier: Tier::Tertiary,
            cluster: None,
            member: None,
            official_cluster: None,
        }
    }

    #[test]
    fn an_edge_says_what_it_is() {
        let d = parse_described(
            r#"{"role":"gateway","tier":"secondary","cluster":"office","member":"gw-a","gateways":[],"main":true}"#,
        )
        .unwrap();
        assert_eq!(
            d,
            Described {
                gateway: true,
                cluster: Some("office".into()),
                member: Some("gw-a".into()),
                ..Described::default()
            }
        );
        let hub = parse_described(r#"{"role":"hub","tier":"tertiary"}"#).unwrap();
        assert!(!hub.gateway);
        // A name that is not a DNS label is dropped, not written to the env
        // file.
        let odd = parse_described(r#"{"role":"gateway","cluster":"a\nB=1"}"#).unwrap();
        assert_eq!(odd.cluster, None);
        assert_eq!(parse_described("<html>"), None);
        assert_eq!(parse_described("{}"), None);
        assert!(cluster_name("office"));
        assert!(cluster_name("gw.local"));
        assert!(cluster_name("[fe80::1]"));
        assert!(!cluster_name(""));
        assert!(!cluster_name("a b"));
        assert!(!cluster_name("a\nb"));
    }

    #[test]
    fn the_configured_edge_is_tertiary_and_a_gateway_that_says_so_is_secondary() {
        let status = assemble_with(
            Some(&gateway("gw.local:2333")),
            Some("https://losos-edge.example"),
            Some("edge.losos.cfd:2333"),
            |_| true,
            |url| {
                url.contains("gw.local").then(|| Described {
                    gateway: true,
                    cluster: Some("office".into()),
                    member: Some("gw-a".into()),
                    ..Described::default()
                })
            },
            |_| None,
            1,
        );
        let lan = status
            .edges
            .iter()
            .find(|e| e.source == Source::Lan)
            .unwrap();
        assert_eq!(lan.tier, Tier::Secondary);
        assert_eq!(lan.cluster.as_deref(), Some("office"));
        let configured = status
            .edges
            .iter()
            .find(|e| e.source == Source::Configured)
            .unwrap();
        assert_eq!(configured.tier, Tier::Tertiary);
        let path = status.path.unwrap();
        assert_eq!(path.tier, Tier::Secondary);
        assert_eq!(path.cluster.as_deref(), Some("office"));
        assert_eq!(path.member.as_deref(), Some("gw-a"));
        assert_eq!(status.cluster.as_deref(), Some("office"));
        assert_eq!(
            status.clusters,
            vec![ClusterView {
                name: "office".into(),
                gateways: vec!["gw-a".into()],
            }]
        );
        let json = EdgeStatus::found(status.edges.clone(), true, None).to_json();
        assert_eq!(json["path"]["tier"], "secondary");
        assert_eq!(json["edges"][1]["tier"], "tertiary");
    }

    #[test]
    fn a_box_settles_in_the_biggest_cluster_and_stays_on_its_gateway() {
        let edges = vec![
            gw("lab", "gw-z"),
            gw("office", "gw-b"),
            gw("office", "gw-a"),
            official_edge(),
        ];
        // No cluster yet: the one with the most gateways, the first gateway.
        let (path, cluster) = route(&edges, None, None, None);
        let path = path.unwrap();
        assert_eq!(cluster.as_deref(), Some("office"));
        assert_eq!(path.member.as_deref(), Some("gw-a"));
        assert_eq!(path.tier, Tier::Secondary);
        // On gw-b now: stays there while it answers.
        let (path, _) = route(&edges, Some("office"), Some("http://gw-b.local:8443"), None);
        assert_eq!(path.unwrap().member.as_deref(), Some("gw-b"));
        // The owner put it in lab: lab, even though office is bigger.
        let (path, cluster) = route(&edges, Some("lab"), Some("http://gw-b.local:8443"), None);
        assert_eq!(path.unwrap().member.as_deref(), Some("gw-z"));
        assert_eq!(cluster.as_deref(), Some("lab"));
        // A tie goes by name.
        let tie = vec![gw("b-site", "gw-1"), gw("a-site", "gw-2")];
        assert_eq!(route(&tie, None, None, None).1.as_deref(), Some("a-site"));
    }

    #[test]
    fn a_failover_stays_in_the_cluster_and_then_goes_tertiary() {
        // gw-a went quiet: the box moves to gw-b of the same cluster.
        let edges = vec![gw("office", "gw-b"), gw("lab", "gw-z"), official_edge()];
        let (path, cluster) = route(&edges, Some("office"), Some("http://gw-a.local:8443"), None);
        let path = path.unwrap();
        assert_eq!(path.member.as_deref(), Some("gw-b"));
        assert_eq!(cluster.as_deref(), Some("office"));
        // No gateway of office answers: straight to the official edge, never
        // to lab's gateway, and the box is still in office.
        let edges = vec![gw("lab", "gw-z"), official_edge()];
        let (path, cluster) = route(&edges, Some("office"), Some("http://gw-b.local:8443"), None);
        let path = path.unwrap();
        assert_eq!(path.tier, Tier::Tertiary);
        assert_eq!(path.url, "https://edge.losos.cfd");
        assert_eq!(path.cluster, None);
        assert_eq!(cluster.as_deref(), Some("office"));
        // Nothing at all.
        let (path, cluster) = route(&[gw("lab", "gw-z")], Some("office"), None, None);
        assert_eq!(path, None);
        assert_eq!(cluster.as_deref(), Some("office"));
        // Back in reach: back to secondary.
        let (path, _) = route(
            &[gw("office", "gw-a"), official_edge()],
            Some("office"),
            Some("https://edge.losos.cfd"),
            None,
        );
        assert_eq!(path.unwrap().tier, Tier::Secondary);
    }

    #[test]
    fn a_custom_domain_holds_the_box_to_its_edge() {
        let edges = vec![gw("office", "gw-a"), gw("office", "gw-b"), official_edge()];
        // The domain went live while the box used gw-b.
        let (on_b, _) = route(&edges, Some("office"), Some("http://gw-b.local:8443"), None);
        let held = path_from_env(&path_env(&on_b.unwrap(), None)).unwrap();
        assert_eq!(held.url, "http://gw-b.local:8443");
        assert_eq!(held.tunnel, "gw-b.local:2333");
        assert_eq!(held.tier, Tier::Secondary);
        assert_eq!(held.cluster.as_deref(), Some("office"));
        // While gw-b answers: gw-b, even with gw-a first by name.
        let (path, cluster) = route(&edges, Some("office"), None, Some(&held));
        assert_eq!(path.unwrap().member.as_deref(), Some("gw-b"));
        assert_eq!(cluster.as_deref(), Some("office"));
        // gw-b is gone: no failover to gw-a, no tertiary; it keeps dialling gw-b.
        let without_b = vec![gw("office", "gw-a"), official_edge()];
        let (path, _) = route(
            &without_b,
            Some("office"),
            Some("http://gw-b.local:8443"),
            Some(&held),
        );
        assert_eq!(path.unwrap().url, "http://gw-b.local:8443");
        // Held to the official edge (a tertiary path): a gateway coming up
        // on the LAN does not pull the box off it.
        let tertiary = path_from_env(
            "LOSOS_EDGE_PATH_URL=https://edge.losos.cfd\n\
             LOSOS_EDGE_PATH_TUNNEL=edge.losos.cfd:2333\n\
             LOSOS_EDGE_PATH_SOURCE=configured\n",
        )
        .unwrap();
        assert_eq!(tertiary.tier, Tier::Tertiary);
        let (path, _) = route(&edges, None, None, Some(&tertiary));
        assert_eq!(path.unwrap().tier, Tier::Tertiary);
        let mut status = EdgeStatus::found(edges, true, None);
        status.route(&PathRule {
            member_of: Some("office"),
            hold: Some(&held),
            ..PathRule::default()
        });
        assert_eq!(status.held.as_deref(), Some("http://gw-b.local:8443"));
        assert!(check_cluster_move(&status, "office")
            .unwrap_err()
            .downcast_ref::<HeldByDomain>()
            .is_some());
        assert_eq!(status.to_json()["held"], "http://gw-b.local:8443");
        assert_eq!(path_from_env("LOSOS_EDGE_PATH_URL=x\n"), None);
    }

    fn official(cluster: &str, n: u8) -> Edge {
        Edge {
            name: format!("edge{n}.{cluster}"),
            url: format!("https://edge{n}.{cluster}.example"),
            source: Source::Configured,
            official: true,
            gateway: None,
            tunnel: Some(format!("edge{n}.{cluster}.example:2333")),
            on_network: true,
            tier: Tier::Tertiary,
            cluster: None,
            member: None,
            official_cluster: Some(cluster.into()),
        }
    }

    fn rule<'a>(seed: &'a str, current: Option<&'a str>, bans: Option<&'a Bans>) -> PathRule<'a> {
        PathRule {
            seed,
            current,
            bans,
            ..PathRule::default()
        }
    }

    #[test]
    fn boxes_spread_over_the_official_edges_and_stay_on_theirs() {
        let edges = vec![
            official("eu", 1),
            official("eu", 2),
            official("us", 1),
            official("us", 2),
        ];
        let mut used = std::collections::BTreeSet::new();
        for n in 0..64 {
            let seed = format!("box{n}");
            let path = choose_path(&edges, &rule(&seed, None, None)).path.unwrap();
            assert_eq!(path.tier, Tier::Tertiary);
            used.insert(path.url);
        }
        assert!(used.len() >= 3, "boxes did not spread: {used:?}");
        // A box stays on the edge it uses while it answers.
        for e in &edges {
            let path = choose_path(&edges, &rule("box1", Some(&e.url), None))
                .path
                .unwrap();
            assert_eq!(path.url, e.url);
        }
        // An edge going away moves only its own boxes.
        let gone = &edges[0].url;
        let rest: Vec<Edge> = edges.iter().filter(|e| &e.url != gone).cloned().collect();
        for n in 0..64 {
            let seed = format!("box{n}");
            let before = choose_path(&edges, &rule(&seed, None, None))
                .path
                .unwrap()
                .url;
            let after = choose_path(&rest, &rule(&seed, Some(&before), None))
                .path
                .unwrap()
                .url;
            if &before != gone {
                assert_eq!(after, before, "{seed} moved though its edge stayed");
            }
        }
    }

    #[test]
    fn a_ban_moves_the_box_off_its_edge_and_off_a_whole_official_cluster() {
        let edges = vec![official("eu", 1), official("eu", 2), official("us", 1)];
        let on = "https://edge1.eu.example";
        let one = Bans {
            edges: vec![on.into()],
            clusters: vec![],
        };
        let chosen = choose_path(&edges, &rule("box1", Some(on), Some(&one)));
        assert_ne!(chosen.path.as_ref().unwrap().url, on);
        assert_eq!(chosen.left_banned.as_deref(), Some(on));
        let all_eu = Bans {
            edges: vec![],
            clusters: vec!["eu".into()],
        };
        for n in 0..32 {
            let seed = format!("box{n}");
            let chosen = choose_path(&edges, &rule(&seed, Some(on), Some(&all_eu)));
            assert_eq!(chosen.path.unwrap().official_cluster.as_deref(), Some("us"));
        }
        // Everything banned: no path, rather than a banned one.
        let all = Bans {
            edges: vec![],
            clusters: vec!["eu".into(), "us".into()],
        };
        assert_eq!(
            choose_path(&edges, &rule("box1", Some(on), Some(&all))).path,
            None
        );
        // A gateway of the box's cluster is still taken over any official edge.
        let mut with_gw = edges.clone();
        with_gw.push(gw("office", "gw-a"));
        let chosen = choose_path(&with_gw, &rule("box1", None, Some(&all_eu)));
        assert_eq!(chosen.path.unwrap().tier, Tier::Secondary);
    }

    #[test]
    fn a_ban_outranks_a_custom_domain_and_the_hold_follows_the_box() {
        let edges = vec![official("eu", 1), official("us", 1)];
        let held = EdgePath::of(&edges[0]);
        // Not banned: held, even with the other edge weighing more.
        let mut status = EdgeStatus::found(edges.clone(), true, None);
        status.route(&PathRule {
            hold: Some(&held),
            seed: "box1",
            ..PathRule::default()
        });
        assert_eq!(status.path.as_ref().unwrap().url, held.url);
        assert_eq!(status.held.as_deref(), Some(held.url.as_str()));
        // Its whole official cluster banned: the box moves, and the hold
        // names the edge it moved to.
        let bans = Bans {
            edges: vec![],
            clusters: vec!["eu".into()],
        };
        status.route(&PathRule {
            hold: Some(&held),
            current: Some(&held.url),
            seed: "box1",
            bans: Some(&bans),
            ..PathRule::default()
        });
        assert_eq!(
            status.path.as_ref().unwrap().url,
            "https://edge1.us.example"
        );
        assert_eq!(status.held.as_deref(), Some("https://edge1.us.example"));
        assert_eq!(status.left_banned.as_deref(), Some(held.url.as_str()));
        assert_eq!(status.to_json()["leftBanned"], held.url);
        // The official cluster rides the env file, so a held box knows it.
        let env = path_env(&held, None);
        assert!(
            env.contains("LOSOS_EDGE_PATH_OFFICIAL_CLUSTER=eu\n"),
            "{env}"
        );
        assert_eq!(
            path_from_env(&env).unwrap().official_cluster.as_deref(),
            Some("eu")
        );
    }

    #[test]
    fn an_official_edge_lists_the_others_and_those_that_answer_are_in_reach() {
        let d = parse_described(
            r#"{"role":"hub","officialCluster":"eu","officials":[
                {"cluster":"eu","url":"https://edge2.eu.example/","tunnel":"edge2.eu.example:2333"},
                {"cluster":"us","url":"https://edge1.us.example","tunnel":"edge1.us.example:2333"},
                {"cluster":"bad name","url":"https://x.example","tunnel":"x.example:2333"},
                {"cluster":"us","url":"ftp://y.example","tunnel":"y.example:2333"}]}"#,
        )
        .unwrap();
        assert_eq!(d.official_cluster.as_deref(), Some("eu"));
        assert_eq!(d.officials.len(), 2, "{:?}", d.officials);
        assert_eq!(d.officials[0].url, "https://edge2.eu.example");
        let status = assemble_with(
            None,
            Some("https://edge1.eu.example"),
            Some("edge1.eu.example:2333"),
            |url| url != "https://edge1.us.example",
            |url| (url == "https://edge1.eu.example").then(|| d.clone()),
            |_| None,
            1,
        );
        let urls: Vec<&str> = status.edges.iter().map(|e| e.url.as_str()).collect();
        assert_eq!(
            urls,
            ["https://edge1.eu.example", "https://edge2.eu.example"]
        );
        assert_eq!(
            status.edges[1].tunnel.as_deref(),
            Some("edge2.eu.example:2333")
        );
        assert!(status.edges.iter().all(|e| e.tier == Tier::Tertiary));
    }

    #[test]
    fn only_a_move_between_clusters_runs_the_mesh_join_again() {
        let env = |cluster: Option<&str>, url: &str| {
            path_env(
                &EdgePath {
                    name: "gw".into(),
                    url: url.into(),
                    source: Source::Lan,
                    tunnel: "gw.local:2333".into(),
                    tier: if cluster.is_some() {
                        Tier::Secondary
                    } else {
                        Tier::Tertiary
                    },
                    cluster: cluster.map(str::to_string),
                    member: None,
                    official_cluster: None,
                },
                None,
            )
        };
        let a = env(Some("office"), "http://gw-a.local:8443");
        let b = env(Some("office"), "http://gw-b.local:8443");
        let lab = env(Some("lab"), "http://gw-z.local:8443");
        let hub = env(None, "https://edge.losos.cfd");
        assert!(a.contains("LOSOS_EDGE_PATH_CLUSTER=office\n"));
        assert_eq!(env_value(&a, "LOSOS_EDGE_PATH_TIER"), Some("secondary"));
        assert_eq!(env_value(&hub, "LOSOS_EDGE_PATH_TIER"), Some("tertiary"));
        assert!(!moved_cluster(Some(&a), Some(&b)), "failover inside office");
        assert!(!moved_cluster(Some(&a), Some(&hub)), "out to tertiary");
        assert!(!moved_cluster(Some(&hub), Some(&a)), "back from tertiary");
        assert!(!moved_cluster(None, Some(&a)), "first path after boot");
        assert!(moved_cluster(Some(&a), Some(&lab)), "the owner moved it");
    }

    #[test]
    fn a_move_needs_a_gateway_of_that_cluster_in_reach() {
        let status = EdgeStatus::found(vec![gw("office", "gw-a"), official_edge()], true, None);
        assert!(check_cluster_move(&status, "office").is_ok());
        assert_eq!(
            check_cluster_move(&status, "lab")
                .unwrap_err()
                .downcast_ref::<ClusterNotInReach>(),
            Some(&ClusterNotInReach("lab".into()))
        );
    }

    #[test]
    fn the_env_file_and_the_drive_decision() {
        let path = EdgePath {
            name: "gateway".into(),
            url: "http://gw.local:8443".into(),
            source: Source::Lan,
            tunnel: "gw.local:2333".into(),
            tier: Tier::Secondary,
            cluster: None,
            member: None,
            official_cluster: None,
        };
        let env = path_env(
            &path,
            Some("/var/secrets/losos-edge-pins/gw.local_8443.pin"),
        );
        assert_eq!(
            env,
            "LOSOS_EDGE_PATH_URL=http://gw.local:8443\n\
             LOSOS_EDGE_PATH_TUNNEL=gw.local:2333\n\
             LOSOS_EDGE_PATH_SOURCE=lan\n\
             LOSOS_EDGE_PATH_TIER=secondary\n\
             LOSOS_EDGE_PATH_TUNNEL_PIN=/var/secrets/losos-edge-pins/gw.local_8443.pin\n"
        );
        assert!(!path_env(&path, None).contains("PIN"));
        assert_eq!(pin_file_name("http://gw.local:8443"), "gw.local_8443.pin");
        assert_eq!(pin_file_name("http://[fe80::1]:8443"), "fe80__1__8443.pin");

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

    const DELV_ECH: &str = "; fully validated\n\
        mattbox.boxes.example.test. 300\tIN\tHTTPS\t1 . alpn=\"h3,h2\" ech=\"AEX+DQBBAQAgACA=\"\n\
        mattbox.boxes.example.test. 300\tIN\tRRSIG\tHTTPS 13 4 300 20261111000000 20261028000000 1234 boxes.example.test. abc=\n";

    #[test]
    fn delv_reads_a_signed_ech_record() {
        let c = parse_delv(DELV_ECH);
        assert!(c.signed && c.ech && c.passed(), "{c:?}");
    }

    #[test]
    fn delv_unsigned_or_failed_is_not_signed() {
        let unsigned = DELV_ECH.replace("; fully validated", "; unsigned answer");
        assert_eq!(parse_delv(&unsigned), DomainCheck::default());
        assert_eq!(
            parse_delv(";; resolution failed: SERVFAIL\n"),
            DomainCheck::default()
        );
        assert_eq!(parse_delv(""), DomainCheck::default());
    }

    #[test]
    fn delv_signed_without_ech_is_not_enough() {
        let no_record = "; negative response, fully validated\n\
            ; mattbox.boxes.example.test. 300 IN \\-HTTPS ;-$NXRRSET\n";
        let c = parse_delv(no_record);
        assert!(c.signed && !c.ech, "{c:?}");
        // An HTTPS record without a key, signed.
        let plain = DELV_ECH.replace(" ech=\"AEX+DQBBAQAgACA=\"", "");
        let c = parse_delv(&plain);
        assert!(c.signed && !c.ech, "{c:?}");
        // `ech=` in an RRSIG's or a comment's text is not a record.
        let rrsig_only =
            "; fully validated\nx.test. 300 IN RRSIG HTTPS 13 2 300 1 2 3 x.test. ech=\n";
        assert!(!parse_delv(rrsig_only).ech);
    }

    #[test]
    fn the_domain_is_the_https_urls_host() {
        assert_eq!(
            domain_of("https://Register.Example.Test:8443/x").as_deref(),
            Some("register.example.test")
        );
        assert_eq!(
            domain_of("https://register.example.test.").as_deref(),
            Some("register.example.test")
        );
        for url in [
            "http://register.example.test",
            "https://192.0.2.1",
            "https://[2001:db8::1]:443",
            "https://localhost",
            "https://-bad.example.test",
            "https://a_b.example.test",
        ] {
            assert_eq!(domain_of(url), None, "{url}");
        }
    }

    fn trusted() -> Option<Gateway> {
        Some(Gateway::Trusted {
            owner: "Owner <o@example.test>".into(),
            fingerprint: "AB".into(),
            image: "gateway".into(),
        })
    }

    fn code(g: Option<Gateway>) -> String {
        match g {
            Some(Gateway::Refused { code, .. }) => code,
            other => panic!("not refused: {other:?}"),
        }
    }

    #[test]
    fn a_trusted_gateway_keeps_its_trust_only_at_a_signed_ech_name() {
        let pass = DomainCheck {
            signed: true,
            ech: true,
        };
        let url = "https://register.example.test";
        let mut asked = None;
        let held = hold_to_domain(url, trusted(), |n| {
            asked = Some(n.to_string());
            pass
        });
        assert_eq!(held, trusted());
        assert_eq!(asked.as_deref(), Some("register.example.test"));

        let unsigned = hold_to_domain(url, trusted(), |_| DomainCheck::default());
        assert_eq!(code(unsigned), "domainUnsigned");
        let no_ech = hold_to_domain(url, trusted(), |_| DomainCheck {
            signed: true,
            ech: false,
        });
        assert_eq!(code(no_ech), "domainNoEch");
        let plain = hold_to_domain("http://register.example.test", trusted(), |_| pass);
        assert_eq!(code(plain), "domainNotHttps");
    }

    #[test]
    fn only_a_trusted_gateway_is_looked_up() {
        let refused = Some(Gateway::Refused {
            code: "unknownImage".into(),
            reason: "r".into(),
        });
        for g in [None, refused] {
            let out = hold_to_domain("https://register.example.test", g.clone(), |_| {
                panic!("looked up")
            });
            assert_eq!(out, g);
        }
    }
}
