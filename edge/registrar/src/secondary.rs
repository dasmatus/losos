//! Secondary clusters: the gateways on one LAN that serve the same boxes.
//!
//! LosOS has three tiers of connection ([`Tier`],
//! handbook/docs/in-depth/edge-federation.md, "Three tiers"):
//!
//! * **primary**: the main cluster, edges joined to edges: every local
//!   gateway with an uplink and the official edges it relays through
//!   ([`crate::relay`]);
//! * **secondary**: a secondary cluster, boxes joined to the local gateways
//!   of their LAN;
//! * **tertiary**: a box talking to an official edge directly, which a box
//!   does only when no gateway of its own cluster answers.
//!
//! This module is the secondary tier. A LAN may run several gateways. Those
//! that carry the same cluster name and secret (`losos-edge cluster create`
//! on the first, `losos-edge cluster join` on the others, both writing the
//! runtime [`ClusterFile`]) behave as one: every pass each gateway POSTs a
//! [`SyncMsg`] to its peers and reads theirs back. Two things travel:
//!
//! * **the enrolled boxes**, with their tokens, so a box that enrolled with
//!   one gateway is let in by every other gateway of the cluster when its
//!   own gateway goes away, without a second first contact;
//! * **when each gateway last heard each box**, so exactly one gateway, the
//!   one that heard the box most recently, relays it to the main cluster
//!   ([`Peers::holds`]). A box talks to one gateway at a time, so the
//!   freshest heartbeat names that gateway, and the hub's route for the box
//!   moves with it instead of two gateways fighting over one service.
//!
//! Ages, not timestamps, cross the wire for the second part, so the rule does
//! not depend on the gateways' clocks agreeing. The message as a whole is
//! signed with HMAC-SHA256 under the cluster secret and carries the sender's
//! clock ([`fresh`]), which bounds a replay to a few minutes. A box's token
//! crosses the LAN in the clear here exactly as it does on every heartbeat
//! the box sends; the secret itself never does.
//!
//! What lives here is the pure half and the peer table. The loop, the route
//! and the enrolment of a peer's boxes are in `server.rs`, beside the state
//! they need.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::{Duration, Instant};

use ring::hmac;
use serde::{Deserialize, Serialize};

use crate::identity::{from_hex, to_hex};
use crate::relay::{dns_label, dns_name, MAX_RELAYED};

/// Where a peer gateway is synced with.
pub const SYNC_ROUTE: &str = "/cluster/secondary/sync";
/// The header carrying a sync message's MAC, on the request and the answer.
pub const MAC_HEADER: &str = "x-losos-cluster-mac";
/// Most peers one gateway keeps. A LAN has a handful of gateways; past this
/// a list is a bug or somebody filling the table, and the rest is ignored.
pub const MAX_PEERS: usize = 16;
/// How far a message's clock may be from ours.
pub const MAX_SKEW_SECS: u64 = 300;
/// Shortest cluster secret accepted: 128 bits as hex.
pub const MIN_SECRET_LEN: usize = 32;
/// Domain separation for the MAC. Bump with the message shape.
const MAC_CONTEXT: &[u8] = b"losos-secondary-v1\n";

/// The runtime file `losos-edge cluster create|join` writes: the cluster this
/// gateway belongs to, its own name in it, the shared secret and the peers it
/// was told about. Absent means the gateway is in no secondary cluster, and
/// nothing is synced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterFile {
    pub name: String,
    /// This gateway's name in the cluster (its host name).
    pub member: String,
    pub secret: String,
    /// Registrar URLs of the gateways this one was joined through. The
    /// founder's list is empty: it learns the others when they first sync.
    #[serde(default)]
    pub peers: Vec<String>,
}

impl ClusterFile {
    /// Why the file cannot be used, or `None`.
    #[must_use]
    pub fn fault(&self) -> Option<&'static str> {
        if !dns_label(&self.name) {
            Some("the cluster name is not a DNS label")
        } else if !dns_label(&self.member) {
            Some("the member name is not a DNS label")
        } else if self.secret.len() < MIN_SECRET_LEN
            || !self.secret.bytes().all(|b| b.is_ascii_hexdigit())
        {
            Some("the secret is not at least 32 hex characters")
        } else if self.peers.iter().any(|p| !plain_url(p)) {
            Some("a peer is not an http(s) URL")
        } else {
            None
        }
    }
}

/// The secondary half of `serve`'s options (`--secondary-file`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecondaryOpts {
    /// Where the [`ClusterFile`] is. Re-read every pass, so writing or
    /// removing it needs no restart.
    pub file: String,
    /// This gateway's registrar URL as its peers and boxes reach it
    /// (`losos.edge.lan.url`).
    pub self_url: String,
    /// Where this gateway's tunnel listens (`losos.edge.lan.ratholeEndpoint`).
    pub self_tunnel: String,
    /// How often peers are synced. Well under the heartbeat TTL.
    pub interval: Duration,
}

/// One box in a [`SyncMsg`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncBox {
    pub id: String,
    pub hostname: String,
    /// The token the box enrolled with. Empty for a box the sender knows
    /// from its operator's whitelist rather than from enrolment: that row is
    /// the sender's operator's, not the cluster's.
    #[serde(default)]
    pub token: String,
    /// Seconds since the sender last heard the box, when it is live there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seen_ago_secs: Option<u64>,
}

/// What gateways of one cluster tell each other every pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncMsg {
    pub cluster: String,
    pub member: String,
    /// The sender's registrar URL.
    pub url: String,
    /// The sender's tunnel endpoint, `host:port`.
    pub tunnel: String,
    /// The sender's clock, Unix seconds.
    pub at: u64,
    pub boxes: Vec<SyncBox>,
    /// Every peer URL the sender knows, so a gateway joined through one
    /// peer meets the rest.
    #[serde(default)]
    pub peers: Vec<String>,
}

impl SyncMsg {
    /// Why a received message is not taken, or `None`.
    #[must_use]
    pub fn fault(&self, own: &ClusterFile, now: u64) -> Option<&'static str> {
        if self.cluster != own.name {
            Some("it is for another cluster")
        } else if self.member == own.member {
            Some("it carries this gateway's own name")
        } else if !dns_label(&self.member) {
            Some("the member name is not a DNS label")
        } else if !plain_url(&self.url) {
            Some("the sender's URL is not an http(s) URL")
        } else if !fresh(self.at, now) {
            Some("its clock is more than five minutes from this gateway's")
        } else if self.boxes.len() > MAX_RELAYED {
            Some("it lists more boxes than a gateway serves")
        } else {
            None
        }
    }
}

/// The tier of one connection. Which one a link is follows from the two
/// ends: edge to edge is primary, box to a local gateway secondary, box to an
/// official edge (any edge that is not a gateway) tertiary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Primary,
    Secondary,
    Tertiary,
}

/// What an edge is to the boxes that reach it, from [`Info::role`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// A local gateway: it enrols boxes on its LAN or belongs to a
    /// secondary cluster. A box's link to it is secondary.
    Gateway,
    /// Any other edge, an official one above all. A box's link to it is
    /// tertiary; a gateway's uplink to it is primary.
    #[default]
    Hub,
}

impl Role {
    /// The tier of a box's link to an edge of this role.
    #[must_use]
    pub fn box_tier(self) -> Tier {
        match self {
            Role::Gateway => Tier::Secondary,
            Role::Hub => Tier::Tertiary,
        }
    }
}

/// What a box (or a person with curl) reads from `GET /cluster/secondary`.
/// No secret, no token.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Info {
    /// What this edge is to a box, and so which tier the box's link is.
    pub role: Role,
    /// The tier of the box links this edge serves (`role.box_tier()`), given
    /// so a reader need not know the rule.
    pub tier: Option<Tier>,
    /// The cluster this gateway belongs to; `None` when it is in none.
    pub cluster: Option<String>,
    /// This gateway's name in it.
    pub member: Option<String>,
    /// The gateways of the cluster that are answering, this one first.
    pub gateways: Vec<GatewayView>,
    /// Whether this gateway relays to a hub, i.e. holds a primary link into
    /// the main cluster.
    pub main: bool,
    /// The official cluster an official edge sits in (`crate::official`).
    #[serde(
        default,
        rename = "officialCluster",
        skip_serializing_if = "Option::is_none"
    )]
    pub official_cluster: Option<String>,
    /// The official edges it lists, its own cluster's and the others'.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub officials: Vec<crate::official::OfficialPeer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayView {
    pub member: String,
    pub url: String,
    pub tunnel: String,
}

/// `http://` or `https://`, a host, nothing else a client would read as
/// something else. Peer URLs come off the LAN.
#[must_use]
pub fn plain_url(url: &str) -> bool {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"));
    rest.is_some_and(|rest| {
        !rest.is_empty()
            && rest.len() <= 253
            && rest
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b":.-_[]/".contains(&b))
    })
}

fn key(secret: &str) -> hmac::Key {
    hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes())
}

/// The MAC of a message body under the cluster secret, as hex.
#[must_use]
pub fn mac(secret: &str, body: &[u8]) -> String {
    let mut ctx = hmac::Context::with_key(&key(secret));
    ctx.update(MAC_CONTEXT);
    ctx.update(body);
    to_hex(ctx.sign().as_ref())
}

/// Whether `mac_hex` is the MAC of `body` under `secret`. Constant time.
#[must_use]
pub fn verify_mac(secret: &str, body: &[u8], mac_hex: &str) -> bool {
    let Some(tag) = from_hex(mac_hex.trim()) else {
        return false;
    };
    let mut msg = Vec::with_capacity(MAC_CONTEXT.len() + body.len());
    msg.extend_from_slice(MAC_CONTEXT);
    msg.extend_from_slice(body);
    hmac::verify(&key(secret), &msg, &tag).is_ok()
}

/// Whether a message stamped `at` may be taken at `now`.
#[must_use]
pub fn fresh(at: u64, now: u64) -> bool {
    at.abs_diff(now) <= MAX_SKEW_SECS
}

/// A box listed in a message, if it is well formed enough to enrol: a
/// DNS-label id, a DNS hostname, a token. A box with no token is only
/// timing information.
#[must_use]
pub fn enrollable(b: &SyncBox) -> bool {
    dns_label(&b.id) && dns_name(&b.hostname) && !b.token.trim().is_empty()
}

/// The rke2 server token of a cluster's control plane, derived from the
/// cluster secret so every gateway of the cluster arrives at the same one
/// without another value to hand around. `losos-edge cluster` writes it for
/// rke2; this is the same derivation, for the tests to pin.
#[must_use]
pub fn rke2_token(secret: &str) -> String {
    let mut ctx = hmac::Context::with_key(&key(secret));
    ctx.update(b"losos-secondary-rke2-token");
    to_hex(ctx.sign().as_ref())
}

#[derive(Debug, Clone)]
struct PeerState {
    url: String,
    tunnel: String,
    last: Instant,
    /// When the peer last heard each box, as of its last message.
    seen: HashMap<String, Instant>,
}

/// The peers of this gateway's cluster, as their messages describe them.
/// In memory only: a restarted gateway learns them again on the first pass
/// (the founder from its joiners' first sync, a joiner from its file).
#[derive(Debug, Default)]
pub struct Peers {
    inner: std::sync::Mutex<PeersInner>,
}

#[derive(Debug, Default)]
struct PeersInner {
    /// The cluster the table belongs to; a change of cluster empties it.
    cluster: Option<String>,
    by_member: BTreeMap<String, PeerState>,
    /// Peer URLs learned from messages, beside the file's own list.
    learned: BTreeSet<String>,
}

impl Peers {
    fn lock(&self) -> std::sync::MutexGuard<'_, PeersInner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Take a peer's message, received at `now`. `self_url` keeps this
    /// gateway out of its own peer list.
    pub fn record(&self, msg: &SyncMsg, self_url: &str, now: Instant) {
        let mut inner = self.lock();
        if inner.cluster.as_deref() != Some(msg.cluster.as_str()) {
            *inner = PeersInner {
                cluster: Some(msg.cluster.clone()),
                ..PeersInner::default()
            };
        }
        if !inner.by_member.contains_key(&msg.member) && inner.by_member.len() >= MAX_PEERS {
            return;
        }
        let seen = msg
            .boxes
            .iter()
            .filter_map(|b| {
                let ago = Duration::from_secs(b.seen_ago_secs?);
                Some((b.id.clone(), now.checked_sub(ago).unwrap_or(now)))
            })
            .collect();
        inner.by_member.insert(
            msg.member.clone(),
            PeerState {
                url: msg.url.clone(),
                tunnel: msg.tunnel.clone(),
                last: now,
                seen,
            },
        );
        for url in std::iter::once(&msg.url).chain(&msg.peers) {
            if url != self_url && plain_url(url) && inner.learned.len() < MAX_PEERS {
                inner.learned.insert(url.clone());
            }
        }
    }

    /// Forget everything: the gateway left its cluster.
    pub fn clear(&self) {
        *self.lock() = PeersInner::default();
    }

    /// Every peer URL to sync with: the file's and the learned ones, without
    /// this gateway's own, sorted and capped.
    #[must_use]
    pub fn urls(&self, file: &ClusterFile, self_url: &str) -> Vec<String> {
        let mut inner = self.lock();
        if inner.cluster.as_deref() != Some(file.name.as_str()) {
            *inner = PeersInner {
                cluster: Some(file.name.clone()),
                ..PeersInner::default()
            };
        }
        let mut all: BTreeSet<String> = inner.learned.clone();
        all.extend(file.peers.iter().cloned());
        all.remove(self_url);
        all.into_iter().take(MAX_PEERS).collect()
    }

    /// The peers heard from within `within`, by member name.
    #[must_use]
    pub fn alive(&self, within: Duration, now: Instant) -> Vec<GatewayView> {
        self.lock()
            .by_member
            .iter()
            .filter(|(_, p)| now.duration_since(p.last) < within)
            .map(|(member, p)| GatewayView {
                member: member.clone(),
                url: p.url.clone(),
                tunnel: p.tunnel.clone(),
            })
            .collect()
    }

    /// Whether this gateway (`member`), which last heard `box_id` at
    /// `mine`, is the one that relays it: no peer heard it more recently,
    /// and on a tie the lower name wins.
    #[must_use]
    pub fn holds(&self, member: &str, box_id: &str, mine: Instant) -> bool {
        self.lock().by_member.iter().all(|(peer, p)| {
            p.seen
                .get(box_id)
                .is_none_or(|&theirs| holder_is_mine((mine, member), (theirs, peer)))
        })
    }

    /// Known peers' URLs with the boxes they hold, for `losos-edge cluster
    /// show` (through `GET /cluster/secondary`) and the tests.
    #[must_use]
    pub fn members(&self) -> Vec<String> {
        self.lock().by_member.keys().cloned().collect()
    }
}

/// The holder rule between two gateways that both heard a box: the fresher
/// hearing wins; on an exact tie the lower member name, so the two agree.
#[must_use]
pub fn holder_is_mine(mine: (Instant, &str), theirs: (Instant, &str)) -> bool {
    match mine.0.cmp(&theirs.0) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => mine.1 <= theirs.1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    fn file(member: &str, peers: &[&str]) -> ClusterFile {
        ClusterFile {
            name: "office".into(),
            member: member.into(),
            secret: SECRET.into(),
            peers: peers.iter().map(|p| (*p).to_string()).collect(),
        }
    }

    fn msg(member: &str, url: &str, boxes: Vec<SyncBox>, at: u64) -> SyncMsg {
        SyncMsg {
            cluster: "office".into(),
            member: member.into(),
            url: url.into(),
            tunnel: format!("{member}.local:2333"),
            at,
            boxes,
            peers: vec![],
        }
    }

    fn seen(id: &str, ago: u64) -> SyncBox {
        SyncBox {
            id: id.into(),
            hostname: format!("{id}.acme.losos.cfd"),
            token: "t".repeat(40),
            seen_ago_secs: Some(ago),
        }
    }

    #[test]
    fn the_file_must_name_a_cluster_a_member_and_a_real_secret() {
        assert_eq!(file("gw-a", &["http://gw-b.local:8443"]).fault(), None);
        let mut f = file("gw-a", &[]);
        f.secret = "short".into();
        assert!(f.fault().unwrap().contains("secret"));
        let mut f = file("gw-a", &[]);
        f.name = "Office Two".into();
        assert!(f.fault().unwrap().contains("cluster name"));
        assert!(file("gw-a", &["javascript:alert(1)"])
            .fault()
            .unwrap()
            .contains("peer"));
    }

    #[test]
    fn a_message_is_taken_only_with_the_secret_and_a_close_clock() {
        let body = br#"{"cluster":"office"}"#;
        let m = mac(SECRET, body);
        assert!(verify_mac(SECRET, body, &m));
        assert!(!verify_mac(SECRET, br#"{"cluster":"other"}"#, &m));
        assert!(!verify_mac(&"f".repeat(64), body, &m));
        assert!(!verify_mac(SECRET, body, "not hex"));

        let own = file("gw-a", &[]);
        let now = 1_000_000;
        assert_eq!(
            msg("gw-b", "http://gw-b.local:8443", vec![], now).fault(&own, now),
            None
        );
        assert!(msg("gw-b", "http://gw-b.local:8443", vec![], now - 301)
            .fault(&own, now)
            .unwrap()
            .contains("clock"));
        assert!(msg("gw-a", "http://gw-a.local:8443", vec![], now)
            .fault(&own, now)
            .unwrap()
            .contains("own name"));
        let mut other = msg("gw-b", "http://gw-b.local:8443", vec![], now);
        other.cluster = "lab".into();
        assert!(other.fault(&own, now).unwrap().contains("another cluster"));
    }

    #[test]
    fn the_gateway_that_heard_a_box_last_relays_it() {
        let peers = Peers::default();
        let now = Instant::now();
        // gw-b heard box1 2 s ago and box2 60 s ago.
        peers.record(
            &msg(
                "gw-b",
                "http://gw-b.local:8443",
                vec![seen("box1", 2), seen("box2", 60)],
                0,
            ),
            "http://gw-a.local:8443",
            now,
        );
        let ten_ago = now - Duration::from_secs(10);
        // gw-a heard both 10 s ago: box2 is gw-a's, box1 is gw-b's.
        assert!(!peers.holds("gw-a", "box1", ten_ago));
        assert!(peers.holds("gw-a", "box2", ten_ago));
        // A box no peer has heard is held by whoever has it.
        assert!(peers.holds("gw-a", "box3", ten_ago));
        // A tie goes to the lower name, from either side.
        assert!(holder_is_mine((now, "gw-a"), (now, "gw-b")));
        assert!(!holder_is_mine((now, "gw-b"), (now, "gw-a")));
    }

    #[test]
    fn peers_are_learned_from_messages_and_never_include_this_gateway() {
        let peers = Peers::default();
        let own = file("gw-b", &["http://gw-a.local:8443"]);
        let self_url = "http://gw-b.local:8443";
        assert_eq!(peers.urls(&own, self_url), ["http://gw-a.local:8443"]);
        let mut m = msg("gw-a", "http://gw-a.local:8443", vec![], 0);
        m.peers = vec![
            "http://gw-c.local:8443".into(),
            self_url.into(),
            "file:///etc/passwd".into(),
        ];
        peers.record(&m, self_url, Instant::now());
        assert_eq!(
            peers.urls(&own, self_url),
            ["http://gw-a.local:8443", "http://gw-c.local:8443"]
        );
        assert_eq!(peers.members(), ["gw-a"]);
        let alive = peers.alive(Duration::from_secs(30), Instant::now());
        assert_eq!(alive.len(), 1);
        assert_eq!(alive[0].tunnel, "gw-a.local:2333");
        // Another cluster's file starts the table over.
        let mut moved = own.clone();
        moved.name = "lab".into();
        assert_eq!(peers.urls(&moved, self_url), ["http://gw-a.local:8443"]);
        assert!(peers.members().is_empty());
    }

    #[test]
    fn only_a_box_with_a_name_and_a_token_is_enrolled_from_a_peer() {
        assert!(enrollable(&seen("box1", 1)));
        let mut b = seen("box1", 1);
        b.token = String::new();
        assert!(!enrollable(&b));
        let mut b = seen("Box.One", 1);
        b.token = "x".repeat(40);
        assert!(!enrollable(&b));
    }

    #[test]
    fn the_tier_follows_from_the_two_ends() {
        assert_eq!(Role::Gateway.box_tier(), Tier::Secondary);
        assert_eq!(Role::Hub.box_tier(), Tier::Tertiary);
        assert_eq!(
            serde_json::to_string(&Tier::Primary).unwrap(),
            "\"primary\""
        );
        let info: Info = serde_json::from_str(r#"{"role":"gateway","tier":"secondary","cluster":null,"member":null,"gateways":[],"main":false}"#).unwrap();
        assert_eq!(info.role, Role::Gateway);
    }

    #[test]
    fn every_gateway_derives_the_same_rke2_token() {
        let t = rke2_token(SECRET);
        assert_eq!(t.len(), 64);
        assert_eq!(t, rke2_token(SECRET));
        assert_ne!(t, rke2_token(&"f".repeat(64)));
    }
}
