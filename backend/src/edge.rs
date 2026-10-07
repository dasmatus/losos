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
}

/// What the last scan found. The JSON shape is the `GET /api/edge` contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeStatus {
    /// At least one edge answered. The one bit the gate reads.
    pub reachable: bool,
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
}

impl EdgeStatus {
    /// Nothing found, nothing searched: the state before the first scan.
    #[must_use]
    pub fn unknown() -> Self {
        EdgeStatus {
            reachable: false,
            edges: Vec::new(),
            lan_searched: false,
            configured_url: None,
            checked_at: None,
        }
    }

    /// A scan that found `edges`. `reachable` is derived, never set apart
    /// from the list, so the two cannot disagree.
    #[must_use]
    pub fn found(edges: Vec<Edge>, lan_searched: bool, configured_url: Option<String>) -> Self {
        EdgeStatus {
            reachable: !edges.is_empty(),
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
            "edges": self.edges,
            "lanSearched": self.lan_searched,
            "configuredUrl": self.configured_url,
            "checkedAt": self.checked_at,
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
        let url = txt
            .split('"')
            .filter(|s| !s.trim().is_empty())
            .find_map(|s| s.strip_prefix("url=").map(str::to_string));
        let advert = Advert {
            name: unescape(cols[3]),
            host: cols[6].to_string(),
            address: cols[7].to_string(),
            port,
            url,
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
        out.push(Edge {
            name: a.name.clone(),
            url,
            source: Source::Lan,
        });
    }
    out
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
            });
        }
    }
    let mut status = EdgeStatus::found(edges, browse.is_some(), configured_url);
    status.checked_at = Some(now);
    status
}

#[cfg(test)]
mod tests {
    use super::*;

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
            1,
        );
        assert_eq!(status.edges.len(), 2);
        assert_eq!(status.edges[1].name, "losos-edge.example");
        assert_eq!(status.edges[1].source, Source::Configured);
        // The same edge on both roads is one edge.
        let status = assemble(Some(BROWSE), Some("http://edge.local:8443/"), |_| true, 1);
        assert_eq!(status.edges.len(), 1);
    }

    #[test]
    fn a_lan_that_could_not_be_searched_says_so() {
        let status = assemble(None, None, |_| true, 1);
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
}
