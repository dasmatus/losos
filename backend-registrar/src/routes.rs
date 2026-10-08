//! Custom domains for boxes behind a local edge: the route table an official
//! edge keeps, and the relay pass that binds a box to its local edge.
//!
//! [`crate::domains`] routes an owner's domain to their box when the box
//! registered with the official edge itself. A box on a site with its own
//! gateway registers with that gateway instead, and reaches the official
//! edge only as a relayed tenant `<spoke>.<box>` ([`crate::relay`]). This
//! module lets its domains follow it there, under three rules:
//!
//! * **Only an official edge routes a domain.** The claims, the DNS checks
//!   and the certificates stay where [`crate::domains`] put them; a local
//!   edge never serves a zone or asks Let's Encrypt for anything. It carries
//!   the traffic the official edge hands it to the box, as it already does
//!   for the box's relayed hostname.
//! * **The box says which local edge it is behind.** A spoke is anyone's
//!   machine, so its word that "my box `mattbox` is your tenant `mattbox`"
//!   is worth nothing on its own: it would let any site take another box's
//!   domains (and the certificates the official edge would issue for them).
//!   The box therefore fetches a [relay pass](PassKey) from the official edge
//!   over its own authenticated `/domains/list`, and hands it to its local
//!   edge on every heartbeat; the local edge forwards it in `POST /relay`.
//!   The pass is an HMAC over the box id and the hour, under a key only the
//!   official edge holds, so a spoke can present it but not make one, and
//!   one that stops receiving it (the box moved on) loses the route within
//!   [`PASS_EPOCHS_VALID`] hours.
//! * **Only a box in the mesh is routed.** The box must have joined this
//!   edge's mesh (`/cluster/join`, which records its compute window under
//!   its own id): a box the operator let into the cluster, not merely one
//!   with a tunnel.
//!
//! The table is the registrar's own state, like `registry.json`: the
//! bindings `/relay` verified and the rows they produce live in memory and
//! in one file beside the registry (`relay-routes.json`, [`TableFile`]),
//! which the reconciler rewrites when the table changes and reads back on
//! start, so a restart keeps routing without waiting for every spoke's next
//! `/relay`. The rows in the file are for the operator to read; only the
//! bindings are loaded, and the table is planned again from them. The
//! local edge gets its own rows back in the `/relay` answer
//! ([`crate::relay::RelayResp::routes`]).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use ring::hmac;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};

use crate::fsutil::atomic_write;

/// One relay pass covers the hour it was issued in.
pub const PASS_EPOCH_SECS: u64 = 3600;
/// Hours a pass stays good: the one it was issued in and the two before
/// now. A box refreshes its pass every few minutes, so this is slack for a
/// box or an edge that was down for a while, and also the longest a spoke
/// the box has left can keep presenting an old one.
pub const PASS_EPOCHS_VALID: u64 = 3;
/// Version prefix of a pass. Bump with the HMAC's domain separation string.
pub const PASS_PREFIX: &str = "v1";
/// Longest pass accepted on the wire: `v1.` + an epoch + `.` + 64 hex.
pub const MAX_PASS_LEN: usize = 96;
/// The pass key file holds a secret.
const PASS_KEY_FILE_MODE: u32 = 0o600;

/// The official edge's relay-pass key.
pub struct PassKey(hmac::Key);

impl std::fmt::Debug for PassKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PassKey(..)")
    }
}

impl PassKey {
    /// A key from 32 raw bytes.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(hmac::Key::new(hmac::HMAC_SHA256, bytes))
    }

    /// Read the key file (64 hex characters), making it on first start.
    ///
    /// # Errors
    /// The file exists but is not 64 hex characters, or cannot be read or
    /// written.
    pub async fn load_or_create(path: &Path) -> std::io::Result<Self> {
        match tokio::fs::read_to_string(path).await {
            Ok(text) => {
                let text = text.trim();
                match unhex(text) {
                    Some(bytes) if bytes.len() == 32 => Ok(Self::from_bytes(&bytes)),
                    _ => Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("{} is not 64 hex characters", path.display()),
                    )),
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let mut bytes = [0u8; 32];
                SystemRandom::new()
                    .fill(&mut bytes)
                    .map_err(|_| std::io::Error::other("the system RNG failed"))?;
                atomic_write(path, hex(&bytes).as_bytes(), PASS_KEY_FILE_MODE).await?;
                Ok(Self::from_bytes(&bytes))
            }
            Err(e) => Err(e),
        }
    }

    fn mac(&self, box_id: &str, epoch: u64) -> hmac::Tag {
        hmac::sign(
            &self.0,
            format!("losos-relay-pass-v1\n{box_id}\n{epoch}\n").as_bytes(),
        )
    }

    /// The pass for `box_id` at `now` (Unix seconds).
    #[must_use]
    pub fn issue(&self, box_id: &str, now: u64) -> String {
        let epoch = now / PASS_EPOCH_SECS;
        format!(
            "{PASS_PREFIX}.{epoch}.{}",
            hex(self.mac(box_id, epoch).as_ref())
        )
    }

    /// The pass's epoch when it is this key's pass for `box_id` and still
    /// good at `now`; `None` for anything else.
    #[must_use]
    pub fn verify(&self, box_id: &str, pass: &str, now: u64) -> Option<u64> {
        if !well_formed_pass(pass) {
            return None;
        }
        let mut parts = pass.splitn(3, '.');
        let (_, epoch, tag) = (parts.next()?, parts.next()?, parts.next()?);
        let epoch: u64 = epoch.parse().ok()?;
        let current = now / PASS_EPOCH_SECS;
        if epoch > current || current - epoch >= PASS_EPOCHS_VALID {
            return None;
        }
        let tag = unhex(tag)?;
        hmac::verify(
            &self.0,
            format!("losos-relay-pass-v1\n{box_id}\n{epoch}\n").as_bytes(),
            &tag,
        )
        .ok()
        .map(|()| epoch)
    }
}

/// Whether `s` has a pass's shape: `v1.<digits>.<64 hex>`. Checked before a
/// spoke keeps a box's pass in memory, so a heartbeat cannot park arbitrary
/// bytes there.
#[must_use]
pub fn well_formed_pass(s: &str) -> bool {
    if s.len() > MAX_PASS_LEN {
        return false;
    }
    let mut parts = s.splitn(3, '.');
    let (Some(v), Some(epoch), Some(tag)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    v == PASS_PREFIX
        && !epoch.is_empty()
        && epoch.len() <= 20
        && epoch.bytes().all(|b| b.is_ascii_digit())
        && tag.len() == 64
        && tag
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// Which local edge vouched for a box, and with a pass from which hour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub spoke: String,
    pub epoch: u64,
}

/// The bindings `/relay` has verified, keyed by box id. Kept in
/// [`TableFile`] across restarts; one that outlived its pass is ignored by
/// [`Bindings::get`] and dropped by the next [`Bindings::record`] of its
/// spoke.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Bindings(BTreeMap<String, Binding>);

impl Bindings {
    /// Record what spoke `spoke` just listed: `vouched` maps each box it
    /// relays with a good pass to that pass's epoch. A box this spoke held
    /// and now lists without a good pass, or no longer lists, is released.
    /// A box another spoke holds moves only to a pass at least as new, so a
    /// spoke the box has left cannot take it back with an old pass.
    pub fn record(&mut self, spoke: &str, vouched: &HashMap<String, u64>) {
        self.0
            .retain(|id, b| b.spoke != spoke || vouched.contains_key(id));
        for (id, &epoch) in vouched {
            match self.0.get_mut(id) {
                Some(b) if b.spoke == spoke => b.epoch = b.epoch.max(epoch),
                Some(b) if b.epoch > epoch => {}
                _ => {
                    self.0.insert(
                        id.clone(),
                        Binding {
                            spoke: spoke.to_string(),
                            epoch,
                        },
                    );
                }
            }
        }
    }

    /// The box's binding, if one is still within [`PASS_EPOCHS_VALID`].
    #[must_use]
    pub fn get(&self, box_id: &str, now: u64) -> Option<&Binding> {
        let current = now / PASS_EPOCH_SECS;
        self.0
            .get(box_id)
            .filter(|b| current.saturating_sub(b.epoch) < PASS_EPOCHS_VALID)
    }
}

/// One route: `domain` is served by the relayed tenant `service`
/// (`<spoke>.<tenant>`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteRow {
    pub domain: String,
    pub tenant: String,
    pub spoke: String,
    pub service: String,
}

/// The table's key of a route: `<spoke>/<box>/<domain>`.
#[must_use]
pub fn row_key(row: &RouteRow) -> String {
    format!("{}/{}/{}", row.spoke, row.tenant, row.domain)
}

/// What the reconciler knows when it plans the table.
#[derive(Debug)]
pub struct PlanInput<'a> {
    /// Every host name a tenant should answer on: its name in the edge's
    /// zone and its live custom domains, keyed by tenant id.
    pub hosts: &'a BTreeMap<String, Vec<String>>,
    /// Ids of tenants that registered here themselves and are live.
    pub direct_live: &'a HashSet<String>,
    /// Keys (`<spoke>.<box>`) of relayed tenants that are live.
    pub relayed_live: &'a HashSet<String>,
    /// Ids of boxes in this edge's mesh.
    pub mesh: &'a HashSet<String>,
    pub bindings: &'a Bindings,
    pub now: u64,
}

/// The route table, keyed by [`row_key`]. Pure.
///
/// A tenant that is live here itself is routed directly and gets no row:
/// the direct path is always preferred. Otherwise a row per host name when
/// the box has a fresh binding, the spoke it names relays it right now, and
/// the box is in the mesh.
#[must_use]
pub fn plan(input: &PlanInput<'_>) -> BTreeMap<String, RouteRow> {
    let mut rows = BTreeMap::new();
    for (tenant, names) in input.hosts {
        if input.direct_live.contains(tenant) || !input.mesh.contains(tenant) {
            continue;
        }
        let Some(binding) = input.bindings.get(tenant, input.now) else {
            continue;
        };
        let service = format!("{}.{tenant}", binding.spoke);
        if !input.relayed_live.contains(&service) {
            continue;
        }
        for domain in names {
            let row = RouteRow {
                domain: domain.clone(),
                tenant: tenant.clone(),
                spoke: binding.spoke.clone(),
                service: service.clone(),
            };
            rows.insert(row_key(&row), row);
        }
    }
    rows
}

/// What `relay-routes.json` holds: the bindings, which a restart reloads,
/// and the rows they produced when the file was written, which nothing
/// reads back.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TableFile {
    pub bindings: Bindings,
    pub routes: Vec<RouteRow>,
}

impl TableFile {
    /// The file's text for `bindings` and `table`: pretty JSON, rows in key
    /// order, so it changes only when the table does.
    #[must_use]
    pub fn render(bindings: &Bindings, table: &BTreeMap<String, RouteRow>) -> String {
        let file = Self {
            bindings: bindings.clone(),
            routes: table.values().cloned().collect(),
        };
        let mut text = serde_json::to_string_pretty(&file).expect("the route table serializes");
        text.push('\n');
        text
    }

    /// The bindings in the file at `path`; none when there is no file yet.
    ///
    /// # Errors
    /// The file cannot be read or is not a table file.
    pub async fn load_bindings(path: &Path) -> std::io::Result<Bindings> {
        match tokio::fs::read(path).await {
            Ok(bytes) => serde_json::from_slice::<Self>(&bytes)
                .map(|f| f.bindings)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Bindings::default()),
            Err(e) => Err(e),
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: u64 = PASS_EPOCH_SECS;

    fn key() -> PassKey {
        PassKey::from_bytes(&[7u8; 32])
    }

    #[test]
    fn a_pass_is_good_for_its_box_for_three_hours_and_nothing_else() {
        let k = key();
        let now = 1000 * HOUR + 17;
        let pass = k.issue("mattbox", now);
        assert!(well_formed_pass(&pass), "{pass}");
        assert_eq!(k.verify("mattbox", &pass, now), Some(1000));
        assert_eq!(k.verify("mattbox", &pass, now + 2 * HOUR), Some(1000));
        assert_eq!(k.verify("mattbox", &pass, now + 3 * HOUR), None, "expired");
        assert_eq!(
            k.verify("mattbox", &pass, now - HOUR),
            None,
            "from the future"
        );
        assert_eq!(k.verify("otherbox", &pass, now), None, "another box");
        assert_eq!(
            PassKey::from_bytes(&[8u8; 32]).verify("mattbox", &pass, now),
            None,
            "another edge's key"
        );
        // An edited epoch does not carry the old tag with it.
        let forged = pass.replacen(".1000.", ".1001.", 1);
        assert_eq!(k.verify("mattbox", &forged, now + HOUR), None);
    }

    #[test]
    fn only_a_pass_shaped_string_is_kept() {
        let tag = "a".repeat(64);
        assert!(well_formed_pass(&format!("v1.12.{tag}")));
        assert!(!well_formed_pass(&format!("v2.12.{tag}")));
        assert!(!well_formed_pass(&format!("v1..{tag}")));
        assert!(!well_formed_pass(&format!("v1.1x.{tag}")));
        assert!(!well_formed_pass(&format!("v1.12.{}", "A".repeat(64))));
        assert!(!well_formed_pass(&format!("v1.12.{}", "a".repeat(63))));
        assert!(!well_formed_pass(&format!("v1.12.{tag}\n")));
        assert!(!well_formed_pass(""));
    }

    #[tokio::test]
    async fn the_key_file_is_made_once_and_read_back() {
        let dir = std::env::temp_dir().join(format!("losos-pass-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay-pass.key");
        let first = PassKey::load_or_create(&path).await.unwrap();
        let pass = first.issue("mattbox", 5 * HOUR);
        let again = PassKey::load_or_create(&path).await.unwrap();
        assert_eq!(again.verify("mattbox", &pass, 5 * HOUR), Some(5));
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        std::fs::write(&path, "not hex").unwrap();
        assert!(PassKey::load_or_create(&path).await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_spoke_the_box_left_cannot_take_it_back_with_an_older_pass() {
        let mut b = Bindings::default();
        let now = 100 * HOUR;
        b.record("acme", &HashMap::from([("mattbox".to_string(), 99)]));
        assert_eq!(b.get("mattbox", now).unwrap().spoke, "acme");
        // The box moved to another site and got a newer pass there.
        b.record("home", &HashMap::from([("mattbox".to_string(), 100)]));
        assert_eq!(b.get("mattbox", now).unwrap().spoke, "home");
        // The old site replays what it was given.
        b.record("acme", &HashMap::from([("mattbox".to_string(), 99)]));
        assert_eq!(b.get("mattbox", now).unwrap().spoke, "home");
        // A spoke that stops vouching releases only what it held.
        b.record("acme", &HashMap::new());
        assert_eq!(b.get("mattbox", now).unwrap().spoke, "home");
        b.record("home", &HashMap::new());
        assert!(b.get("mattbox", now).is_none());
        // And a binding ages out with its pass.
        b.record("home", &HashMap::from([("mattbox".to_string(), 100)]));
        assert!(b.get("mattbox", now + 2 * HOUR).is_some());
        assert!(b.get("mattbox", now + 3 * HOUR).is_none());
    }

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn only_a_mesh_box_bound_to_a_live_spoke_and_not_live_here_gets_rows() {
        let now = 100 * HOUR;
        let mut bindings = Bindings::default();
        bindings.record(
            "acme",
            &HashMap::from([
                ("mattbox".to_string(), 100),
                ("nomesh".to_string(), 100),
                ("direct".to_string(), 100),
                ("gone".to_string(), 100),
            ]),
        );
        let hosts: BTreeMap<String, Vec<String>> = [
            (
                "mattbox",
                vec!["cloud.example.org", "0123456789abcdef.boxes.example.test"],
            ),
            ("nomesh", vec!["nomesh.example.org"]),
            ("direct", vec!["direct.example.org"]),
            ("gone", vec!["gone.example.org"]),
            ("unbound", vec!["unbound.example.org"]),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.into_iter().map(String::from).collect()))
        .collect();
        let rows = plan(&PlanInput {
            hosts: &hosts,
            direct_live: &set(&["direct"]),
            relayed_live: &set(&["acme.mattbox", "acme.nomesh", "acme.direct", "acme.unbound"]),
            mesh: &set(&["mattbox", "direct", "gone", "unbound"]),
            bindings: &bindings,
            now,
        });
        let keys: Vec<&str> = rows.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "acme/mattbox/0123456789abcdef.boxes.example.test",
                "acme/mattbox/cloud.example.org",
            ]
        );
        let row = &rows["acme/mattbox/cloud.example.org"];
        assert_eq!(row.service, "acme.mattbox");
        assert_eq!(row.tenant, "mattbox");
    }

    #[tokio::test]
    async fn a_restart_reloads_the_bindings_from_the_table_file() {
        let dir = std::env::temp_dir().join(format!("losos-routes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay-routes.json");
        assert_eq!(
            TableFile::load_bindings(&path).await.unwrap(),
            Bindings::default(),
            "no file yet"
        );
        let mut bindings = Bindings::default();
        bindings.record("acme", &HashMap::from([("mattbox".to_string(), 100)]));
        let row = RouteRow {
            domain: "cloud.example.org".to_string(),
            tenant: "mattbox".to_string(),
            spoke: "acme".to_string(),
            service: "acme.mattbox".to_string(),
        };
        let table = BTreeMap::from([(row_key(&row), row)]);
        let text = TableFile::render(&bindings, &table);
        std::fs::write(&path, &text).unwrap();
        assert_eq!(TableFile::load_bindings(&path).await.unwrap(), bindings);
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["bindings"]["mattbox"]["spoke"], "acme");
        assert_eq!(v["routes"][0]["service"], "acme.mattbox");
        std::fs::write(&path, "not json").unwrap();
        assert!(TableFile::load_bindings(&path).await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
