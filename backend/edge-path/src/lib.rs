//! Which edge proxy to use: the path rule a LosOS box goes by, as a crate
//! of its own so that anything else that picks an edge picks it the same
//! way. lososd (`backend/src/edge.rs`) does the I/O around it: the LAN
//! browse over resolved, the `/health` and `/cluster/secondary` probes, the
//! gateway checks, and the units it drives. LosOS Desktop's network machine
//! runs the same rule to choose the edge its traffic leaves through.
//!
//! Everything here is pure: what an edge advertises ([`Advert`]), what a
//! scan found ([`assemble_with`], [`EdgeStatus`]), and which edge is the
//! path ([`choose_path`]). The handbook's In depth chapter,
//! `edge-federation.md`, is the long form of the rule.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// The tunnel port an edge binds unless its advert says otherwise
/// (`losos.edge.tunnelBindPort`'s default). Used for a LAN edge whose
/// advert carries no `tunnel=` record.
pub const DEFAULT_TUNNEL_PORT: u16 = 2333;

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
    /// The edge passed every gateway check (see the `gateways` section
    /// below): its TPM shows a published LosOS gateway image with its
    /// owner's signed passport and LosOS's fee addresses. Only such an edge
    /// may process trading; any other edge still shares storage.
    #[serde(default)]
    pub official: bool,
    /// What the gateway checks concluded, when the edge has a passport to
    /// check. `None` for an edge that shows none and on a box that ships no
    /// anchors to check one against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway: Option<Gateway>,
    /// Where this edge's tunnel server listens, `host:port`: the advert's
    /// `tunnel=` record (or its host on the default port) for a LAN edge,
    /// `losos.proxy.edgeTunnelEndpoint` for the configured one. `None` when
    /// the box was given no endpoint for it, in which case it can be found
    /// but not tunnelled to.
    #[serde(default)]
    pub tunnel: Option<String>,
    /// Both the registrar and the tunnel endpoint resolve only to addresses
    /// on a network this box is directly attached to ([`on_own_network`]). Only
    /// such an edge may manage the box; one reached through a router, or
    /// advertised on the LAN with addresses elsewhere, never may.
    #[serde(default, rename = "onNetwork")]
    pub on_network: bool,
    /// The tier a link from this box to this edge would be ([`Tier`]):
    /// secondary to a local gateway, tertiary to an official edge (any edge
    /// that does not say it is a gateway).
    #[serde(default = "Tier::secondary")]
    pub tier: Tier,
    /// The secondary cluster the edge belongs to, as it said
    /// (`GET /cluster/secondary`), when it is a gateway in one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cluster: Option<String>,
    /// The edge's name in that cluster.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<String>,
    /// The official cluster an official edge sits in: the load balancer in
    /// front of it, which a ban may name as a whole ([`Bans`]).
    #[serde(
        default,
        rename = "officialCluster",
        skip_serializing_if = "Option::is_none"
    )]
    pub official_cluster: Option<String>,
}

/// The tier of a connection (handbook/docs/in-depth/edge-federation.md,
/// "Three tiers"). A box's own link is secondary or tertiary; primary links
/// join edges to edges, and a box only ever sees them reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// Edge to edge: the main cluster.
    Primary,
    /// Box to a local gateway: a secondary cluster.
    Secondary,
    /// Box straight to an official edge.
    Tertiary,
}

impl Tier {
    fn secondary() -> Self {
        Tier::Secondary
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Primary => "primary",
            Tier::Secondary => "secondary",
            Tier::Tertiary => "tertiary",
        }
    }
}

/// What an edge says about itself at `GET /cluster/secondary`
/// (`edge/registrar/src/secondary.rs`, `Info`), the part the box reads.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Described {
    /// It says it is a local gateway.
    pub gateway: bool,
    pub cluster: Option<String>,
    pub member: Option<String>,
    /// An official edge's official cluster.
    pub official_cluster: Option<String>,
    /// The official edges it lists, its own cluster's and the others'.
    pub officials: Vec<OfficialPeer>,
}

/// One official edge as another lists it (`edge/registrar/src/official.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialPeer {
    pub cluster: String,
    pub url: String,
    pub tunnel: String,
}

/// Most official edges read off one answer.
pub const MAX_OFFICIALS: usize = 64;

/// Read an edge's `GET /cluster/secondary` answer. `None` for anything that
/// is not the shape the registrar writes, which leaves the edge classified
/// by where it was found. Names are kept only when they are DNS labels: they
/// go into an env file and onto the admin page.
#[must_use]
pub fn parse_described(body: &str) -> Option<Described> {
    let v: Value = serde_json::from_str(body).ok()?;
    let role = v.get("role")?.as_str()?;
    let label = |key: &str| {
        v.get(key)
            .and_then(Value::as_str)
            .filter(|s| dns_label(s))
            .map(str::to_string)
    };
    let officials = v
        .get("officials")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|o| {
            let peer = OfficialPeer {
                cluster: o.get("cluster")?.as_str()?.to_string(),
                url: o.get("url")?.as_str()?.trim_end_matches('/').to_string(),
                tunnel: o.get("tunnel")?.as_str()?.to_string(),
            };
            (dns_label(&peer.cluster) && plain_http_url(&peer.url) && plain_endpoint(&peer.tunnel))
                .then_some(peer)
        })
        .take(MAX_OFFICIALS)
        .collect();
    Some(Described {
        gateway: role == "gateway",
        cluster: label("cluster"),
        member: label("member"),
        official_cluster: label("officialCluster"),
        officials,
    })
}

/// 1 to 63 lowercase letters, digits and inner hyphens.
#[must_use]
pub fn dns_label(s: &str) -> bool {
    (1..=63).contains(&s.len())
        && !s.starts_with('-')
        && !s.ends_with('-')
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A name [`cluster_key`] can give: a DNS label, or the host of a gateway
/// that names no cluster (`gw.local`, `[fe80::1]`). What the owner may send
/// to `POST /api/edge/cluster`; the move is then checked against the scan.
#[must_use]
pub fn cluster_name(s: &str) -> bool {
    (1..=253).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b':' | b'[' | b']'))
}

/// One secondary cluster in reach, as the Mesh pane lists them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterView {
    pub name: String,
    /// Its gateways that answered, by name.
    pub gateways: Vec<String>,
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
    pub tunnel: String,
    /// Secondary through a gateway of the box's cluster, tertiary straight
    /// to an official edge.
    pub tier: Tier,
    /// The secondary cluster the path runs through; `None` on a tertiary one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cluster: Option<String>,
    /// The gateway's name in it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<String>,
    /// The official cluster of an official edge.
    #[serde(
        default,
        rename = "officialCluster",
        skip_serializing_if = "Option::is_none"
    )]
    pub official_cluster: Option<String>,
}

impl EdgePath {
    pub fn of(e: &Edge) -> Self {
        EdgePath {
            name: e.name.clone(),
            url: e.url.clone(),
            source: e.source,
            tunnel: e.tunnel.clone().unwrap_or_default(),
            tier: e.tier,
            cluster: (e.tier == Tier::Secondary).then(|| cluster_key(e)),
            member: e.member.clone(),
            official_cluster: e.official_cluster.clone(),
        }
    }
}

/// What is banned (`edge/registrar/src/official.rs`, the same file shape):
/// single edges by registrar URL, whole official clusters by name. Read from
/// `$LOSOS_EDGE_BANS_FILE`, which the ban list's source writes once it has
/// checked it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bans {
    #[serde(default)]
    pub edges: Vec<String>,
    #[serde(default)]
    pub clusters: Vec<String>,
}

impl Bans {
    /// Whether the edge at `url`, in official cluster `cluster`, is banned.
    #[must_use]
    pub fn banned(&self, url: &str, cluster: Option<&str>) -> bool {
        let url = url.trim_end_matches('/');
        self.edges.iter().any(|e| e.trim_end_matches('/') == url)
            || cluster.is_some_and(|c| self.clusters.iter().any(|b| b == c))
    }
}

/// Rendezvous weight of `url` for `seed` (this box's name): the same seed
/// ranks the same edges the same way everywhere, and an edge going away
/// moves only the boxes that were on it.
#[must_use]
pub fn weight(seed: &str, url: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    hasher.update(b"\0");
    hasher.update(url.as_bytes());
    hasher.finalize().into()
}

/// What [`choose_path`] goes by besides the edges in reach.
#[derive(Debug, Clone, Copy, Default)]
pub struct PathRule<'a> {
    /// The secondary cluster the box belongs to.
    pub member_of: Option<&'a str>,
    /// The registrar URL the box uses now.
    pub current: Option<&'a str>,
    /// The path a live custom domain holds the box to.
    pub hold: Option<&'a EdgePath>,
    /// This box's name, which spreads boxes over the official edges.
    pub seed: &'a str,
    pub bans: Option<&'a Bans>,
}

/// What [`choose_path`] decided.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Chosen {
    pub path: Option<EdgePath>,
    /// The secondary cluster the box belongs to afterwards.
    pub cluster: Option<String>,
    /// The banned edge the box left, when it was on one.
    pub left_banned: Option<String>,
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
    /// The secondary clusters in reach.
    #[serde(default)]
    pub clusters: Vec<ClusterView>,
    /// The secondary cluster this box belongs to: the one its owner chose,
    /// or the one it settled in first. Kept while none of its gateways
    /// answers, so a box never moves its own membership.
    #[serde(default)]
    pub cluster: Option<String>,
    /// The registrar URL a live custom domain holds this box to, if any. A
    /// box with a domain sticks to the edge that serves it and fails over to
    /// no other gateway or tier, unless that edge or its official cluster is
    /// banned; the hold then follows the box to the edge it moved to.
    #[serde(default)]
    pub held: Option<String>,
    /// The banned edge this box moved off on the last choice, if it did.
    #[serde(default)]
    pub left_banned: Option<String>,
}

/// The cluster an edge is counted in: the one it named, or for a gateway
/// that names none (one from before secondary clusters), a cluster of its
/// own named by its host.
#[must_use]
pub fn cluster_key(edge: &Edge) -> String {
    match &edge.cluster {
        Some(c) => c.clone(),
        None => bracketed_host(&edge.url),
    }
}

/// The secondary clusters among `edges`, by name, each with its gateways.
#[must_use]
pub fn clusters(edges: &[Edge]) -> Vec<ClusterView> {
    let mut out: Vec<ClusterView> = Vec::new();
    for e in edges.iter().filter(|e| e.tier == Tier::Secondary) {
        let key = cluster_key(e);
        let name = e.member.clone().unwrap_or_else(|| e.name.clone());
        match out.iter_mut().find(|c| c.name == key) {
            Some(c) => c.gateways.push(name),
            None => out.push(ClusterView {
                name: key,
                gateways: vec![name],
            }),
        }
    }
    for c in &mut out {
        c.gateways.sort();
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// The path rule (handbook/docs/in-depth/edge-federation.md, "The box"):
///
/// 1. **secondary**, through a gateway of the box's own cluster (`member_of`,
///    the one its owner chose or it settled in). With no cluster yet, the
///    cluster in reach with the most gateways, ties by name, and that becomes
///    the box's cluster. Inside the cluster the box stays on the gateway it
///    uses now (`current`, its registrar URL) while that one answers, else
///    the first by name: a failover moves to another gateway of the same
///    cluster, where the box is already known.
/// 2. **tertiary**, straight to an official edge, when no gateway of the
///    box's cluster answers. Another cluster's gateway is never taken on the
///    box's own: membership moves only when the owner moves it. The official
///    edges that answer share the boxes between them: a box stays on the one
///    it uses while it answers, else takes the one its name weighs heaviest
///    on ([`weight`]), so boxes spread over every load balancer and an edge
///    going away moves only its own boxes. With no official edge, the
///    configured edge.
/// 3. none.
///
/// A box with a live custom domain is held to `hold`, the path it had when
/// the domain went live. It stays on that edge while it answers, and keeps
/// dialling it while it does not, rather than move: the domain's route and
/// every connection over it run through that one edge.
///
/// Above everything, a banned edge, or any edge of a banned official
/// cluster, is never chosen ([`Bans`]). A box on one moves by itself under
/// the rules above, a held box included: a ban outranks the hold. Nor is
/// an edge this box disabled for tampering ([`Edge::disabled`]).
#[must_use]
pub fn choose_path(edges: &[Edge], rule: &PathRule<'_>) -> Chosen {
    let no_bans = Bans::default();
    let bans = rule.bans.unwrap_or(&no_bans);
    let open: Vec<&Edge> = edges
        .iter()
        .filter(|e| e.tunnel.is_some() && !e.disabled())
        .filter(|e| !bans.banned(&e.url, e.official_cluster.as_deref()))
        .collect();
    let current_banned = rule.current.filter(|cur| {
        edges
            .iter()
            .find(|e| e.url == *cur)
            .map_or(bans.banned(cur, None), |e| {
                bans.banned(&e.url, e.official_cluster.as_deref())
            })
    });
    let mut left_banned = current_banned.map(str::to_string);
    let current = rule.current.filter(|_| current_banned.is_none());

    if let Some(held) = rule.hold {
        if bans.banned(&held.url, held.official_cluster.as_deref()) {
            left_banned = Some(held.url.clone());
        } else {
            let path = open
                .iter()
                .find(|e| e.url == held.url)
                .map_or_else(|| held.clone(), |e| EdgePath::of(e));
            let cluster = rule
                .member_of
                .map(str::to_string)
                .or_else(|| path.cluster.clone());
            return Chosen {
                path: Some(path),
                cluster,
                left_banned: None,
            };
        }
    }

    let secondary: Vec<&Edge> = open
        .iter()
        .copied()
        .filter(|e| e.tier == Tier::Secondary)
        .collect();
    let membership = match rule.member_of {
        Some(c) => Some(c.to_string()),
        None => clusters(edges)
            .into_iter()
            .filter(|c| secondary.iter().any(|e| cluster_key(e) == c.name))
            .max_by(|a, b| {
                a.gateways
                    .len()
                    .cmp(&b.gateways.len())
                    .then_with(|| b.name.cmp(&a.name))
            })
            .map(|c| c.name),
    };
    let stay = |candidates: &[&Edge]| -> Option<EdgePath> {
        candidates
            .iter()
            .find(|e| Some(e.url.as_str()) == current)
            .map(|e| EdgePath::of(e))
    };
    if let Some(m) = &membership {
        let mine: Vec<&Edge> = secondary
            .iter()
            .copied()
            .filter(|e| cluster_key(e) == *m)
            .collect();
        let first = mine
            .iter()
            .min_by_key(|e| e.member.clone().unwrap_or_else(|| e.name.clone()))
            .map(|e| EdgePath::of(e));
        if let Some(path) = stay(&mine).or(first) {
            return Chosen {
                path: Some(path),
                cluster: membership,
                left_banned,
            };
        }
    }
    let tertiary: Vec<&Edge> = open
        .iter()
        .copied()
        .filter(|e| e.tier == Tier::Tertiary)
        .collect();
    let officials: Vec<&Edge> = tertiary.iter().copied().filter(|e| e.official).collect();
    let path = if officials.is_empty() {
        let configured: Vec<&Edge> = tertiary
            .iter()
            .copied()
            .filter(|e| e.source == Source::Configured)
            .collect();
        let pool = if configured.is_empty() {
            &tertiary
        } else {
            &configured
        };
        stay(pool).or_else(|| pool.first().map(|e| EdgePath::of(e)))
    } else {
        stay(&officials).or_else(|| {
            officials
                .iter()
                .max_by_key(|e| weight(rule.seed, &e.url))
                .map(|e| EdgePath::of(e))
        })
    };
    Chosen {
        path,
        cluster: membership,
        left_banned,
    }
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
            clusters: Vec::new(),
            cluster: None,
            held: None,
            left_banned: None,
        }
    }

    /// A scan that found `edges`. `reachable` is derived, never set apart
    /// from the list, so the two cannot disagree.
    #[must_use]
    pub fn found(edges: Vec<Edge>, lan_searched: bool, configured_url: Option<String>) -> Self {
        let Chosen { path, cluster, .. } = choose_path(&edges, &PathRule::default());
        EdgeStatus {
            reachable: !edges.is_empty(),
            official: edges.iter().any(|e| e.official),
            path,
            clusters: clusters(&edges),
            cluster,
            held: None,
            left_banned: None,
            edges,
            lan_searched,
            configured_url,
            checked_at: None,
        }
    }

    /// Choose the path again by `rule` ([`choose_path`]). A hold a ban
    /// broke follows the box: `held` names the edge it moved to.
    pub fn route(&mut self, rule: &PathRule<'_>) {
        let chosen = choose_path(&self.edges, rule);
        let hold_broken = rule
            .hold
            .is_some_and(|h| chosen.left_banned.as_deref() == Some(h.url.as_str()));
        self.held = match rule.hold {
            Some(_) if hold_broken => chosen.path.as_ref().map(|p| p.url.clone()),
            Some(h) => Some(h.url.clone()),
            None => None,
        };
        self.path = chosen.path;
        self.cluster = chosen.cluster;
        self.left_banned = chosen.left_banned;
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
            "clusters": self.clusters,
            "cluster": self.cluster,
            "held": self.held,
            "leftBanned": self.left_banned,
        })
    }
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
    /// The `tunnel=` TXT value, when the edge published one.
    pub tunnel: Option<String>,
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
        // registrar's host on the default port.
        let tunnel = a
            .tunnel
            .as_deref()
            .filter(|r| plain_endpoint(r))
            .map(str::to_string)
            .unwrap_or_else(|| format!("{}:{DEFAULT_TUNNEL_PORT}", bracketed_host(&url)));
        out.push(Edge {
            name: a.name.clone(),
            url,
            source: Source::Lan,
            official: false,
            gateway: None,
            tunnel: Some(tunnel),
            on_network: false,
            tier: Tier::Secondary,
            cluster: None,
            member: None,
            official_cluster: None,
        });
    }
    out
}

/// `host:port` as the tunnel takes it: a hostname or IP literal (IPv6 in
/// brackets) and a port, nothing a shell or a JSON string would read as
/// something else. The value goes into the tunnel client's file.
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
    vet: impl FnMut(&str) -> Option<Gateway>,
    now: u64,
) -> EdgeStatus {
    assemble_with(browse, configured_url, None, answered, |_| None, vet, now)
}

/// [`assemble`] with the configured edge's tunnel endpoint
/// (`losos.proxy.edgeTunnelEndpoint`), which is what lets that edge be the
/// path when the LAN has none, and `describe`, each answering edge's word on
/// what it is (`GET /cluster/secondary`, [`parse_described`]). An edge that
/// says nothing is a gateway when it was found on the LAN and an official
/// edge when it is the configured one; an official edge is never a gateway.
#[allow(clippy::too_many_arguments)]
pub fn assemble_with(
    browse: Option<&[Advert]>,
    configured_url: Option<&str>,
    configured_tunnel: Option<&str>,
    answered: impl Fn(&str) -> bool,
    describe: impl Fn(&str) -> Option<Described>,
    mut vet: impl FnMut(&str) -> Option<Gateway>,
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
                gateway: None,
                tunnel: configured_tunnel
                    .map(str::trim)
                    .filter(|r| plain_endpoint(r))
                    .map(str::to_string),
                on_network: false,
                tier: Tier::Tertiary,
                cluster: None,
                member: None,
                official_cluster: None,
            });
        }
    }
    // Each edge is asked what it is. An official edge also names the
    // official edges it knows of, behind its own load balancer and the
    // others': those that answer are in reach too, for the path rule to
    // spread boxes over and to move to when one is banned.
    let mut listed: Vec<OfficialPeer> = Vec::new();
    for edge in &mut edges {
        if let Some(d) = describe(&edge.url) {
            edge.tier = if d.gateway {
                Tier::Secondary
            } else {
                Tier::Tertiary
            };
            edge.cluster = d.cluster;
            edge.member = d.member;
            edge.official_cluster = d.official_cluster;
            if !d.gateway {
                listed.extend(d.officials);
            }
        }
    }
    for peer in listed.into_iter().take(MAX_OFFICIALS) {
        if edges.iter().any(|e| e.url == peer.url) || !answered(&peer.url) {
            continue;
        }
        edges.push(Edge {
            name: host_of(&peer.url),
            url: peer.url,
            source: Source::Configured,
            official: false,
            gateway: None,
            tunnel: Some(peer.tunnel),
            on_network: false,
            tier: Tier::Tertiary,
            cluster: None,
            member: None,
            official_cluster: Some(peer.cluster),
        });
    }
    // Every edge that answered is checked as a gateway ([`Vetter::vet`]).
    // Only one that passed may name the official cluster it sits in: a
    // ban may name that cluster, and an edge that proves nothing could
    // otherwise pass itself off as one of its members.
    for edge in &mut edges {
        edge.gateway = vet(&edge.url);
        edge.official = matches!(edge.gateway, Some(Gateway::Trusted { .. }));
        if !edge.official {
            edge.official_cluster = None;
        }
    }
    let mut status = EdgeStatus::found(edges, browse.is_some(), configured_url);
    status.checked_at = Some(now);
    status
}

/// What the box concluded about one edge's passport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Gateway {
    /// Every check passed.
    Trusted {
        /// The owner's OpenPGP user id and key fingerprint.
        owner: String,
        fingerprint: String,
        /// The published image it runs.
        image: String,
    },
    /// Not trusted now; asked again next time. `code` is the code of
    /// `losos_access`'s gateway check refusal.
    Refused { code: String, reason: String },
    /// Proved tampered, on this check or an earlier one. Never trusted or
    /// tunnelled through again.
    Disabled {
        code: String,
        reason: String,
        since: u64,
    },
}

impl Edge {
    /// This box disabled the edge for tampering.
    #[must_use]
    pub fn disabled(&self) -> bool {
        matches!(self.gateway, Some(Gateway::Disabled { .. }))
    }
}

// ── the box's own network ──────────────────────────────────────────────
//
// Management is for an edge beside the box, never one out on the internet,
// even one that tunnels the box's traffic. The LAN advert alone does not
// show that: its `url=` and `tunnel=` records may name any host. So the
// scan resolves both, and the edge counts as on the box's network only when
// every address is inside a prefix the box itself has on an interface
// (`/proc/net/route`, `/proc/net/if_inet6`), that is, reached without a
// router in between, and every IPv4 one is private (RFC 1918, link-local).

/// A prefix the box is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Net {
    pub addr: IpAddr,
    pub len: u8,
}

impl Net {
    #[must_use]
    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self.addr, ip) {
            (IpAddr::V4(n), IpAddr::V4(a)) if self.len <= 32 => {
                let mask = u32::MAX.checked_shl(32 - u32::from(self.len)).unwrap_or(0);
                u32::from(n) & mask == u32::from(a) & mask
            }
            (IpAddr::V6(n), IpAddr::V6(a)) if self.len <= 128 => {
                let mask = u128::MAX
                    .checked_shl(128 - u32::from(self.len))
                    .unwrap_or(0);
                u128::from(n) & mask == u128::from(a) & mask
            }
            _ => false,
        }
    }
}

/// The IPv4 prefixes of `/proc/net/route` that have no gateway: the
/// networks on the box's own links. The default route and loopback are
/// left out.
#[must_use]
pub fn parse_proc_route(text: &str) -> Vec<Net> {
    let hex = |s: &str| u32::from_str_radix(s, 16).ok().map(u32::from_be);
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            let (iface, dest, gw, mask) = (
                f.first()?,
                hex(f.get(1)?)?,
                hex(f.get(2)?)?,
                hex(f.get(7)?)?,
            );
            let len = u8::try_from(mask.count_ones()).ok()?;
            (*iface != "lo" && gw == 0 && len > 0).then(|| Net {
                addr: IpAddr::V4(Ipv4Addr::from(dest)),
                len,
            })
        })
        .collect()
}

/// The IPv6 prefixes of `/proc/net/if_inet6` (address, prefix length per
/// interface), loopback left out.
#[must_use]
pub fn parse_if_inet6(text: &str) -> Vec<Net> {
    text.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            let (addr, len, iface) = (f.first()?, f.get(2)?, f.get(5)?);
            let addr = u128::from_str_radix(addr, 16).ok()?;
            let len = u8::from_str_radix(len, 16).ok()?;
            (*iface != "lo" && len > 0).then(|| Net {
                addr: IpAddr::V6(Ipv6Addr::from(addr)),
                len,
            })
        })
        .collect()
}

/// An IPv4 address only a local network uses. IPv6 needs no such check: a
/// global prefix on the box's own link is still that link, and a LAN with
/// SLAAC from its provider names its machines by such addresses.
#[must_use]
pub fn private_v4(ip: Ipv4Addr) -> bool {
    ip.is_private() || ip.is_link_local()
}

/// Every one of `addrs` is on one of the box's own prefixes, and an IPv4
/// one is private besides: a box on a public subnet shares it with
/// strangers.
#[must_use]
pub fn on_own_network(addrs: &[IpAddr], nets: &[Net]) -> bool {
    !addrs.is_empty()
        && addrs.iter().all(|a| {
            let local = match a {
                IpAddr::V4(v4) => private_v4(*v4),
                IpAddr::V6(_) => true,
            };
            local && nets.iter().any(|n| n.contains(*a))
        })
}

impl EdgeStatus {
    /// Mark the LAN edges whose registrar and tunnel endpoint resolve onto
    /// the box's own networks. `resolve` takes a host (no port, no brackets)
    /// and gives its addresses.
    pub fn mark_on_network(&mut self, nets: &[Net], resolve: impl Fn(&str) -> Vec<IpAddr>) {
        for e in &mut self.edges {
            let hosts = std::iter::once(bracketed_host(&e.url))
                .chain(e.tunnel.as_deref().map(bracketed_host_of_endpoint));
            e.on_network = e.source == Source::Lan
                && hosts.into_iter().all(|h| {
                    on_own_network(
                        &resolve(h.trim_start_matches('[').trim_end_matches(']')),
                        nets,
                    )
                });
        }
    }
}

/// The host of a `host:port` endpoint, an IPv6 literal kept in brackets.
fn bracketed_host_of_endpoint(endpoint: &str) -> String {
    endpoint
        .rsplit_once(':')
        .map_or(endpoint, |(h, _)| h)
        .to_string()
}
