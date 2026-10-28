//! Official clusters, load balancing between them, and bans.
//!
//! The official edges behind one load balancer form an **official cluster**,
//! named by its operator (`--official-cluster NAME`). Every official edge
//! lists the official edges it knows of, its own cluster's and the others'
//! (`--official-peer CLUSTER,URL,TUNNEL`), at `GET /cluster/secondary`
//! (`crate::secondary::Info::officials`). That list is how a gateway's
//! uplink, and a box going tertiary, spread over the official edges rather
//! than all landing on the one they were given ([`pick`]), and where they go
//! when theirs is banned.
//!
//! A ban names edges or whole official clusters ([`Bans`]). Whoever is on a
//! banned edge moves by itself to an official edge outside every ban: a
//! gateway's uplink here, a box's path in lososd (backend/src/edge.rs). A
//! box held to its edge by a custom domain moves too; a ban outranks the
//! hold. The ban list itself is read from a file (`--bans-file`) its source
//! writes after checking it.

use ring::digest;
use serde::{Deserialize, Serialize};

use crate::relay::dns_label;
use crate::secondary::plain_url;

/// Most official peers one edge lists. Past this a list is a mistake.
pub const MAX_OFFICIALS: usize = 64;

/// One official edge, as another official edge lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfficialPeer {
    /// Its official cluster: the load balancer it sits behind.
    pub cluster: String,
    /// Its registrar API.
    pub url: String,
    /// `host:port` of its tunnel server.
    pub tunnel: String,
}

impl OfficialPeer {
    /// Parse `CLUSTER,URL,TUNNEL`, the `--official-peer` form.
    ///
    /// # Errors
    /// A sentence naming what is wrong.
    pub fn parse(s: &str) -> Result<Self, String> {
        let mut parts = s.splitn(3, ',');
        let (Some(cluster), Some(url), Some(tunnel)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(format!("{s:?} is not CLUSTER,URL,TUNNEL"));
        };
        let peer = OfficialPeer {
            cluster: cluster.trim().to_string(),
            url: url.trim().trim_end_matches('/').to_string(),
            tunnel: tunnel.trim().to_string(),
        };
        if !peer.well_formed() {
            return Err(format!(
                "{s:?}: the cluster must be a DNS label, the URL plain http(s) and the tunnel host:port"
            ));
        }
        Ok(peer)
    }

    /// Everything a reader writes into a config file is plain.
    #[must_use]
    pub fn well_formed(&self) -> bool {
        dns_label(&self.cluster) && plain_url(&self.url) && plain_endpoint(&self.tunnel)
    }
}

/// `host:port` with a port that is a number above zero, and nothing a
/// config file would read as something else.
#[must_use]
pub fn plain_endpoint(s: &str) -> bool {
    let Some((host, port)) = s.rsplit_once(':') else {
        return false;
    };
    !host.is_empty()
        && host.len() <= 253
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'[' | b']' | b':'))
        && port.parse::<u16>().is_ok_and(|p| p > 0)
}

/// What is banned: single edges by registrar URL, and whole official
/// clusters by name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bans {
    #[serde(default)]
    pub edges: Vec<String>,
    #[serde(default)]
    pub clusters: Vec<String>,
}

impl Bans {
    /// Read the ban file. Absent or unreadable is no bans: the file is the
    /// only thing that can move anyone, and a bad one must not scatter them.
    pub async fn read(path: Option<&str>) -> Self {
        let Some(path) = path else {
            return Bans::default();
        };
        match tokio::fs::read(path).await {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "the ban file {path} does not parse; nothing is banned");
                Bans::default()
            }),
            Err(_) => Bans::default(),
        }
    }

    /// Whether the edge at `url`, in official cluster `cluster`, is banned.
    #[must_use]
    pub fn banned(&self, url: &str, cluster: Option<&str>) -> bool {
        let url = url.trim_end_matches('/');
        self.edges.iter().any(|e| e.trim_end_matches('/') == url)
            || cluster.is_some_and(|c| self.clusters.iter().any(|b| b == c))
    }
}

/// Rendezvous weight of `url` for `seed`: the same seed ranks the same
/// edges the same way everywhere, and losing one edge moves only the
/// boxes that were on it.
#[must_use]
pub fn weight(seed: &str, url: &str) -> [u8; 32] {
    let mut ctx = digest::Context::new(&digest::SHA256);
    ctx.update(seed.as_bytes());
    ctx.update(b"\0");
    ctx.update(url.as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(ctx.finish().as_ref());
    out
}

/// Where `seed` (a box or a gateway) should be among `candidates`, none of
/// them banned: on `current` while it is one of them, so nobody moves while
/// their edge is fine, else the highest [`weight`]. `None` with no
/// candidate left.
#[must_use]
pub fn pick<'a>(
    candidates: &'a [OfficialPeer],
    seed: &str,
    current: Option<&str>,
    bans: &Bans,
) -> Option<&'a OfficialPeer> {
    let open: Vec<&OfficialPeer> = candidates
        .iter()
        .filter(|p| !bans.banned(&p.url, Some(&p.cluster)))
        .collect();
    if let Some(cur) = current {
        if let Some(p) = open.iter().find(|p| p.url == cur.trim_end_matches('/')) {
            return Some(p);
        }
    }
    open.into_iter().max_by_key(|p| weight(seed, &p.url))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(cluster: &str, n: u8) -> OfficialPeer {
        OfficialPeer {
            cluster: cluster.into(),
            url: format!("https://edge{n}.{cluster}.example"),
            tunnel: format!("edge{n}.{cluster}.example:2333"),
        }
    }

    #[test]
    fn a_peer_parses_and_a_bad_one_does_not() {
        let p = OfficialPeer::parse("eu,https://edge1.example/,edge1.example:2333").unwrap();
        assert_eq!(p.url, "https://edge1.example");
        assert!(OfficialPeer::parse("eu,https://edge1.example").is_err());
        assert!(OfficialPeer::parse("EU!,https://edge1.example,edge1.example:2333").is_err());
        assert!(OfficialPeer::parse("eu,ftp://x,edge1.example:2333").is_err());
        assert!(OfficialPeer::parse("eu,https://x,edge1.example:0").is_err());
    }

    #[test]
    fn boxes_spread_stay_and_leave_only_a_banned_edge() {
        let peers = vec![peer("eu", 1), peer("eu", 2), peer("us", 1), peer("us", 2)];
        let none = Bans::default();
        // Many boxes land on more than one edge.
        let mut used = std::collections::BTreeSet::new();
        for n in 0..64 {
            used.insert(
                pick(&peers, &format!("box{n}"), None, &none)
                    .unwrap()
                    .url
                    .clone(),
            );
        }
        assert!(used.len() >= 3, "{used:?}");
        // A box stays on its edge while it is fine, whatever the weights say.
        for p in &peers {
            assert_eq!(pick(&peers, "box1", Some(&p.url), &none).unwrap(), p);
        }
        // Its edge banned: it moves, and only it.
        let first = pick(&peers, "box1", None, &none).unwrap().clone();
        let bans = Bans {
            edges: vec![first.url.clone()],
            clusters: vec![],
        };
        let moved = pick(&peers, "box1", Some(&first.url), &bans).unwrap();
        assert_ne!(moved.url, first.url);
        // The whole cluster banned: off every edge behind that balancer.
        let bans = Bans {
            edges: vec![],
            clusters: vec!["eu".into()],
        };
        for n in 0..32 {
            let p = pick(
                &peers,
                &format!("box{n}"),
                Some("https://edge1.eu.example"),
                &bans,
            )
            .unwrap();
            assert_eq!(p.cluster, "us");
        }
        let all = Bans {
            edges: vec![],
            clusters: vec!["eu".into(), "us".into()],
        };
        assert_eq!(pick(&peers, "box1", None, &all), None);
        assert!(all.banned("https://edge9.us.example/", Some("us")));
        assert!(!all.banned("https://edge9.us.example/", None));
    }
}
