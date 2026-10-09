//! Edge federation: the pure half of relaying a site's boxes through a hub,
//! and the on-disk store of boxes a LAN edge enrolled on first contact.
//!
//! handbook/docs/in-depth/edge-federation.md is the design. In one paragraph: a
//! user-hosted edge (a **spoke**) is an ordinary tenant of an official edge (a
//! **hub**) whose whitelist row carries a `relay_zone`. The spoke POSTs
//! `/relay` with the boxes it currently serves; the hub keeps a `<spoke>.<box>`
//! tenant for each, routes `Host(<box hostname>)` into a rathole service of
//! that name, and the spoke's uplink rathole client
//! (`crate::config::uplink_config`) delivers that service onto the box's own
//! tunnel. Hostname authority stays with the hub operator: a relayed hostname
//! must be exactly one label under the zone they wrote.
//!
//! What lives here is testable without a socket: the wire shapes, the name
//! rules, the uplink file a spoke reads its hub from, and [`Enrolment`], the
//! trust-on-first-use store a gateway keeps for boxes on its LAN. The handler,
//! the reconciler's relayed half and the uplink loop are in `server.rs`,
//! where the state they need lives.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::action::Action;
use crate::fsutil;

/// Most boxes one spoke may relay at once. A site is a handful of boxes; a
/// list longer than this is a bug or an attempt to drain the hub's port
/// range, and the whole call is refused rather than truncated so the spoke's
/// log says so.
pub const MAX_RELAYED: usize = 64;

/// The body a spoke POSTs to `/relay`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayReq {
    /// The spoke's own tenant id on the hub.
    pub appliance_id: String,
    pub token: String,
    /// The boxes the spoke serves right now, each with the public hostname
    /// it registered under.
    pub tenants: Vec<RelayTenant>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayTenant {
    pub id: String,
    pub hostname: String,
    /// The relay pass the box got from this hub and gave its spoke
    /// (`crate::routes`): the box's own word that it is behind this spoke,
    /// which is what lets its custom domains route here. Absent for a box
    /// that has none (no custom domains, or a hub without a route table).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pass: Option<String>,
}

/// The hub's answer: what it will route, and what it refused and why. A
/// refusal is per box, never the whole call, so one misnamed box does not
/// take a site offline.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelayResp {
    pub accepted: Vec<crate::registry::Relayed>,
    pub refused: Vec<RelayRefused>,
    /// The custom domains the hub routes to this spoke's boxes, from its
    /// route table (`crate::routes`). Informational: the hub's
    /// traffic for them arrives on the box's relayed service like the box's
    /// own hostname, so the spoke routes nothing by name.
    #[serde(default)]
    pub routes: Vec<RelayRoute>,
}

/// One row of the hub's route table, as the spoke is told it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayRoute {
    /// The box id as the spoke knows it.
    pub id: String,
    pub domain: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayRefused {
    pub id: String,
    pub reason: String,
}

/// A DNS label as a box id or a spoke id must be: 1–63 lowercase
/// alphanumerics and inner hyphens, no dots. The relayed key is
/// `<spoke>.<box>`, so a dot in either half would make the key ambiguous.
#[must_use]
pub fn dns_label(s: &str) -> bool {
    (1..=63).contains(&s.len())
        && !s.starts_with('-')
        && !s.ends_with('-')
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A hostname made of DNS labels, at most 253 characters.
#[must_use]
pub fn dns_name(s: &str) -> bool {
    s.len() <= 253 && !s.is_empty() && s.split('.').all(dns_label)
}

/// Whether `hostname` is exactly one label under `zone`
/// (`mattbox.acme.losos.cfd` under `acme.losos.cfd`). One label, not any
/// depth: the zone is what the operator delegated, and a relayed
/// `www.evil.acme.losos.cfd` would still be inside it, but a certificate per
/// arbitrary depth is not what they agreed to issue.
#[must_use]
pub fn hostname_in_zone(hostname: &str, zone: &str) -> bool {
    let zone = zone.trim().trim_end_matches('.');
    if !dns_name(zone) || !dns_name(hostname) {
        return false;
    }
    match hostname.strip_suffix(zone) {
        Some(rest) => rest
            .strip_suffix('.')
            .is_some_and(|label| !label.is_empty() && dns_label(label)),
        None => false,
    }
}

/// Why one listed box is refused, or `None` when it may be relayed under
/// `zone`. `whitelisted` says whether the would-be key `<spoke>.<id>` is a
/// direct tenant of the hub, which a relay must never shadow.
#[must_use]
pub fn refusal(tenant: &RelayTenant, zone: &str, whitelisted: bool) -> Option<&'static str> {
    if !dns_label(&tenant.id) {
        Some("id is not a DNS label")
    } else if !hostname_in_zone(&tenant.hostname, zone) {
        Some("hostname is not one label under the spoke's relay zone")
    } else if whitelisted {
        Some("the relayed key names a direct tenant of this edge")
    } else {
        None
    }
}

/// The spoke's hub, as a file (`--uplink-file`). Paths, not secrets, so the
/// NixOS module may render it into the store; the gateway image points the
/// flag at a runtime path instead so one image serves every site, and the
/// registrar re-reads it on every uplink pass: writing the file is enough,
/// no restart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UplinkFile {
    /// The hub's registrar API base URL (`https://register.<domain>`).
    pub registrar_url: String,
    /// The hub's rathole server, `host:port`.
    pub rathole_endpoint: String,
    /// This spoke's tenant id on the hub.
    pub id: String,
    /// 0600 file holding this spoke's tenant token on the hub.
    pub token_file: String,
    /// 0600 file holding the hub's rathole bootstrap token. Optional: every
    /// relayed service carries the spoke's own token, so rathole's
    /// `default_token` is never consulted, and the spoke token stands in
    /// when no bootstrap token was handed out.
    #[serde(default)]
    pub bootstrap_token_file: Option<String>,
    /// Where the hub's Noise public key is pinned. Absent means plain TCP.
    /// When the file does not exist yet the uplink loop fetches the key once
    /// from the hub's `/noise-public-key` and writes it here (trust on first
    /// contact, as a box pins its edge); from then on the file is authority
    /// and a hub whose key changed is refused by rathole, not re-pinned.
    #[serde(default)]
    pub noise_public_key_file: Option<String>,
}

/// The uplink half of `serve`'s options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UplinkOpts {
    /// Where the [`UplinkFile`] is. Absent at runtime means no uplink, which
    /// the gateway image relies on before its owner has entered one.
    pub file: String,
    /// The rathole client config the uplink loop is the sole writer of.
    pub rathole_config: String,
    /// How often the spoke posts `/relay` when nothing changed. Must be well
    /// under the hub's heartbeat TTL, like a box's heartbeat.
    pub interval: std::time::Duration,
}

/// The whitelist-shaped file [`Enrolment`] keeps, keyed by appliance id.
/// Same keys as `tenants.json` so the server's one `TenantEntry` reader
/// parses both: `cluster` and `market` are never written and so deserialize
/// to `false`, which is the only thing an enrolled box may ever be.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct EnrolledFile(BTreeMap<String, EnrolledEntry>);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EnrolledEntry {
    hostname: String,
    token_file: String,
}

/// Boxes a LAN edge accepted on first contact (`losos.edge.lan.openEnrolment`).
///
/// A directory, `0700`: `tenants.json` in the whitelist shape plus one
/// `<id>.token` (0600) per box. Trust on first use: the first `/register`
/// for an unknown id fixes that id's token; every later request must match
/// it, and the only way to change it is for the owner to forget the id. The
/// hostname is recorded at the same time and may be changed by a later
/// authenticated `/register`, since the box proved the token. Writes are
/// serialised by one lock and land atomically, like the registry.
#[derive(Debug)]
pub struct Enrolment {
    dir: PathBuf,
    lock: Mutex<()>,
}

impl Enrolment {
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            lock: Mutex::new(()),
        }
    }

    /// The whitelist-shaped file the server reads enrolled tenants from.
    #[must_use]
    pub fn tenants_file(&self) -> PathBuf {
        self.dir.join("tenants.json")
    }

    fn token_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.token"))
    }

    async fn read(&self) -> std::io::Result<EnrolledFile> {
        match tokio::fs::read(self.tenants_file()).await {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(std::io::Error::other),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(EnrolledFile::default()),
            Err(e) => Err(e),
        }
    }

    async fn write(&self, file: &EnrolledFile) -> std::io::Result<()> {
        tokio::fs::create_dir_all(&self.dir).await?;
        let bytes = serde_json::to_vec_pretty(file).map_err(std::io::Error::other)?;
        fsutil::atomic_write(&self.tenants_file(), &bytes, 0o600).await
    }

    /// Record `id` with `hostname` and `token`. Refuses (returns `false`,
    /// writes nothing) when the id is already enrolled: a second first
    /// contact is somebody else, and the first box keeps its name.
    pub async fn enrol(&self, id: &str, hostname: &str, token: &str) -> std::io::Result<bool> {
        let _guard = self.lock.lock().await;
        let mut file = self.read().await?;
        if file.0.contains_key(id) {
            return Ok(false);
        }
        tokio::fs::create_dir_all(&self.dir).await?;
        let token_path = self.token_path(id);
        fsutil::atomic_write(&token_path, format!("{token}\n").as_bytes(), 0o600).await?;
        file.0.insert(
            id.to_string(),
            EnrolledEntry {
                hostname: hostname.to_string(),
                token_file: token_path.to_string_lossy().into_owned(),
            },
        );
        self.write(&file).await?;
        tracing::info!(
            target: Action::Enrol.target(),
            "enrolled {id} as {hostname} on first contact",
        );
        Ok(true)
    }

    /// Point an enrolled id at a new hostname. `false` when the id is not
    /// enrolled or already has that name (no write either way).
    pub async fn rehost(&self, id: &str, hostname: &str) -> std::io::Result<bool> {
        let _guard = self.lock.lock().await;
        let mut file = self.read().await?;
        match file.0.get_mut(id) {
            Some(entry) if entry.hostname != hostname => entry.hostname = hostname.to_string(),
            _ => return Ok(false),
        }
        self.write(&file).await?;
        Ok(true)
    }

    /// Drop an enrolled id and its token file: what `losos-edge forget` does.
    pub async fn forget(&self, id: &str) -> std::io::Result<bool> {
        let _guard = self.lock.lock().await;
        let mut file = self.read().await?;
        if file.0.remove(id).is_none() {
            return Ok(false);
        }
        self.write(&file).await?;
        match tokio::fs::remove_file(self.token_path(id)).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        Ok(true)
    }

    /// The enrolled ids with their hostnames, sorted.
    pub async fn list(&self) -> std::io::Result<Vec<(String, String)>> {
        Ok(self
            .read()
            .await?
            .0
            .into_iter()
            .map(|(id, e)| (id, e.hostname))
            .collect())
    }

    /// Whether `path` is inside this store, so a caller can tell an enrolled
    /// token file from a whitelist one.
    #[must_use]
    pub fn owns(&self, path: &Path) -> bool {
        path.starts_with(&self.dir)
    }
}

/// The `enrol` subcommand (`crate::opts::EnrolOpts`).
pub async fn run(opts: crate::opts::EnrolOpts) -> miette::Result<()> {
    use crate::opts::EnrolOpts;
    use miette::{miette, IntoDiagnostic};
    match opts {
        EnrolOpts::List { dir } => {
            let store = Enrolment::new(dir);
            for (id, hostname) in store.list().await.into_diagnostic()? {
                println!("{id} {hostname}");
            }
            Ok(())
        }
        EnrolOpts::Forget { dir, id } => {
            if !dns_label(&id) {
                return Err(miette!("{id} is not an appliance id"));
            }
            let store = Enrolment::new(dir);
            if store.forget(&id).await.into_diagnostic()? {
                println!("forgot {id}");
                Ok(())
            } else {
                Err(miette!("{id} is not enrolled here"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_names() {
        assert!(dns_label("mattbox"));
        assert!(dns_label("box-01"));
        assert!(!dns_label("MattBox"));
        assert!(!dns_label("a.b"));
        assert!(!dns_label("-a"));
        assert!(!dns_label(""));
        assert!(dns_name("mattbox.acme.losos.cfd"));
        assert!(!dns_name("mattbox..acme"));
    }

    #[test]
    fn one_label_under_the_zone_and_no_more() {
        assert!(hostname_in_zone("mattbox.acme.losos.cfd", "acme.losos.cfd"));
        assert!(hostname_in_zone(
            "mattbox.acme.losos.cfd",
            "acme.losos.cfd."
        ));
        assert!(!hostname_in_zone("acme.losos.cfd", "acme.losos.cfd"));
        assert!(!hostname_in_zone("a.b.acme.losos.cfd", "acme.losos.cfd"));
        assert!(!hostname_in_zone("mattboxacme.losos.cfd", "acme.losos.cfd"));
        assert!(!hostname_in_zone(
            "mattbox.other.losos.cfd",
            "acme.losos.cfd"
        ));
        assert!(!hostname_in_zone("mattbox.acme.losos.cfd", ""));
    }

    #[test]
    fn refusals_name_the_rule() {
        let ok = RelayTenant {
            id: "mattbox".into(),
            hostname: "mattbox.acme.losos.cfd".into(),
            pass: None,
        };
        assert_eq!(refusal(&ok, "acme.losos.cfd", false), None);
        assert!(refusal(&ok, "acme.losos.cfd", true)
            .unwrap()
            .contains("direct tenant"));
        let bad_id = RelayTenant {
            id: "Matt.Box".into(),
            ..ok.clone()
        };
        assert!(refusal(&bad_id, "acme.losos.cfd", false)
            .unwrap()
            .contains("DNS label"));
        let bad_host = RelayTenant {
            hostname: "mattbox.losos.cfd".into(),
            ..ok
        };
        assert!(refusal(&bad_host, "acme.losos.cfd", false)
            .unwrap()
            .contains("relay zone"));
    }

    #[tokio::test]
    async fn enrolment_is_first_come_and_forgettable() {
        let dir = std::env::temp_dir().join(format!(
            "losos-enrol-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        let e = Enrolment::new(&dir);
        assert!(e.list().await.unwrap().is_empty());
        assert!(e
            .enrol("mattbox", "mattbox.acme.losos.cfd", "t0ken")
            .await
            .unwrap());
        assert!(!e.enrol("mattbox", "other", "another").await.unwrap());
        let token = std::fs::read_to_string(dir.join("mattbox.token")).unwrap();
        assert_eq!(token.trim(), "t0ken");
        assert!(e.rehost("mattbox", "mb.acme.losos.cfd").await.unwrap());
        assert!(!e.rehost("mattbox", "mb.acme.losos.cfd").await.unwrap());
        assert_eq!(
            e.list().await.unwrap(),
            vec![("mattbox".to_string(), "mb.acme.losos.cfd".to_string())]
        );
        assert!(e.owns(&dir.join("mattbox.token")));
        assert!(e.forget("mattbox").await.unwrap());
        assert!(!e.forget("mattbox").await.unwrap());
        assert!(!dir.join("mattbox.token").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
