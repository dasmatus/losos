//! Custom domains for boxes behind a local edge: the route table an official
//! edge keeps in etcd, and the relay pass that binds a box to its local edge.
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
//! The table itself lives in etcd, one key per route,
//! `<prefix>/<spoke>/<box>/<domain>`, so the official edge's routing is a
//! keyspace an operator can read with `etcdctl get --prefix /losos/routes/`
//! and that several official edges can share later. The reconciler writes
//! what it computed, reads the prefix back, and renders the Traefik routers
//! from what etcd holds, so what routes is what the table says. The local
//! edge gets its own rows back in the `/relay` answer
//! ([`crate::relay::RelayResp::routes`]); it never talks to etcd.
//!
//! etcd is reached through its v3 JSON gateway (`/v3/kv/*`, base64 keys and
//! values) over plain HTTP on loopback, which keeps this a few `reqwest`
//! calls instead of a gRPC stack.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

use ring::hmac;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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
/// Default etcd key prefix of the route table.
pub const DEFAULT_PREFIX: &str = "/losos/routes";
/// One etcd request. The gateway is on loopback.
pub const ETCD_TIMEOUT: Duration = Duration::from_secs(3);
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub spoke: String,
    pub epoch: u64,
}

/// The bindings `/relay` has verified, keyed by box id. Live state only:
/// an official edge that restarts relearns them from the next `/relay` of
/// each spoke, at most one uplink interval later.
#[derive(Debug, Default)]
pub struct Bindings(HashMap<String, Binding>);

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

/// The etcd key of a route.
#[must_use]
pub fn row_key(prefix: &str, row: &RouteRow) -> String {
    format!(
        "{}/{}/{}/{}",
        prefix.trim_end_matches('/'),
        row.spoke,
        row.tenant,
        row.domain
    )
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

/// The route table, keyed by etcd key. Pure.
///
/// A tenant that is live here itself is routed directly and gets no row:
/// the direct path is always preferred. Otherwise a row per host name when
/// the box has a fresh binding, the spoke it names relays it right now, and
/// the box is in the mesh.
#[must_use]
pub fn plan(prefix: &str, input: &PlanInput<'_>) -> BTreeMap<String, RouteRow> {
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
            rows.insert(row_key(prefix, &row), row);
        }
    }
    rows
}

/// What to change in etcd to get from `current` to `desired`.
#[must_use]
pub fn diff(
    current: &BTreeMap<String, RouteRow>,
    desired: &BTreeMap<String, RouteRow>,
) -> (Vec<(String, RouteRow)>, Vec<String>) {
    let puts = desired
        .iter()
        .filter(|(k, v)| current.get(*k) != Some(*v))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let deletes = current
        .keys()
        .filter(|k| !desired.contains_key(*k))
        .cloned()
        .collect();
    (puts, deletes)
}

/// A client of etcd's v3 JSON gateway.
#[derive(Debug, Clone)]
pub struct Etcd {
    base: String,
    http: reqwest::Client,
}

/// Why an etcd call failed.
#[derive(Debug, thiserror::Error)]
pub enum EtcdError {
    #[error("etcd at {0}: {1}")]
    Http(String, String),
    #[error("etcd answered something that is not a v3 gateway reply: {0}")]
    Shape(String),
}

impl Etcd {
    /// A client for the gateway at `base` (`http://127.0.0.1:2379`).
    ///
    /// # Errors
    /// The HTTP client cannot be built.
    pub fn new(base: &str) -> Result<Self, reqwest::Error> {
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            http: reqwest::Client::builder().timeout(ETCD_TIMEOUT).build()?,
        })
    }

    async fn call(&self, path: &str, body: Value) -> Result<Value, EtcdError> {
        let url = format!("{}{path}", self.base);
        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| EtcdError::Http(url.clone(), e.to_string()))?;
        let status = resp.status();
        let value: Value = resp
            .json()
            .await
            .map_err(|e| EtcdError::Http(url.clone(), e.to_string()))?;
        if !status.is_success() {
            return Err(EtcdError::Http(url, format!("{status}: {value}")));
        }
        Ok(value)
    }

    /// Every key under `prefix`, with its value.
    ///
    /// # Errors
    /// etcd is unreachable or answers something else.
    pub async fn range_prefix(&self, prefix: &str) -> Result<BTreeMap<String, Vec<u8>>, EtcdError> {
        let reply = self
            .call(
                "/v3/kv/range",
                json!({
                    "key": b64(prefix.as_bytes()),
                    "range_end": b64(&prefix_end(prefix.as_bytes())),
                }),
            )
            .await?;
        let mut out = BTreeMap::new();
        // An empty range has no `kvs` at all.
        for kv in reply["kvs"].as_array().into_iter().flatten() {
            let key = kv["key"]
                .as_str()
                .and_then(unb64)
                .and_then(|k| String::from_utf8(k).ok())
                .ok_or_else(|| EtcdError::Shape(kv.to_string()))?;
            // A key set to an empty value has no `value` field.
            let value = match kv["value"].as_str() {
                Some(v) => unb64(v).ok_or_else(|| EtcdError::Shape(kv.to_string()))?,
                None => Vec::new(),
            };
            out.insert(key, value);
        }
        Ok(out)
    }

    /// Set `key` to `value`.
    ///
    /// # Errors
    /// etcd is unreachable or refuses.
    pub async fn put(&self, key: &str, value: &[u8]) -> Result<(), EtcdError> {
        self.call(
            "/v3/kv/put",
            json!({ "key": b64(key.as_bytes()), "value": b64(value) }),
        )
        .await
        .map(|_| ())
    }

    /// Remove `key`.
    ///
    /// # Errors
    /// etcd is unreachable or refuses.
    pub async fn delete(&self, key: &str) -> Result<(), EtcdError> {
        self.call("/v3/kv/deleterange", json!({ "key": b64(key.as_bytes()) }))
            .await
            .map(|_| ())
    }

    /// The route rows under `prefix`, and the keys under it that hold
    /// something else. A value that does not parse as a row, or whose key is
    /// not where the row says it belongs, is never routed: [`Etcd::sync`]
    /// deletes it.
    ///
    /// # Errors
    /// As [`Etcd::range_prefix`].
    pub async fn rows(
        &self,
        prefix: &str,
    ) -> Result<(BTreeMap<String, RouteRow>, Vec<String>), EtcdError> {
        let dir = format!("{}/", prefix.trim_end_matches('/'));
        let mut rows = BTreeMap::new();
        let mut junk = Vec::new();
        for (key, value) in self.range_prefix(&dir).await? {
            match serde_json::from_slice::<RouteRow>(&value) {
                Ok(row) if row_key(prefix, &row) == key => {
                    rows.insert(key, row);
                }
                _ => junk.push(key),
            }
        }
        Ok((rows, junk))
    }

    /// Make the table under `prefix` equal `desired` and return what etcd
    /// holds afterwards, read back rather than assumed.
    ///
    /// # Errors
    /// Any etcd call failed. Changes already made stay made; the next pass
    /// repeats the rest.
    pub async fn sync(
        &self,
        prefix: &str,
        desired: &BTreeMap<String, RouteRow>,
    ) -> Result<BTreeMap<String, RouteRow>, EtcdError> {
        let (current, junk) = self.rows(prefix).await?;
        let (puts, mut deletes) = diff(&current, desired);
        deletes.extend(junk);
        if puts.is_empty() && deletes.is_empty() {
            return Ok(current);
        }
        for key in &deletes {
            tracing::info!(target: crate::action::Action::Domains.target(), "etcd: removing route {key}");
            self.delete(key).await?;
        }
        for (key, row) in &puts {
            tracing::info!(
                target: crate::action::Action::Domains.target(),
                "etcd: {} goes to {} through {}",
                row.domain,
                row.tenant,
                row.spoke,
            );
            let value = serde_json::to_vec(row).expect("a route row serializes");
            self.put(key, &value).await?;
        }
        Ok(self.rows(prefix).await?.0)
    }
}

/// etcd's `range_end` for "every key starting with `prefix`": the prefix
/// with its last byte incremented, dropping trailing 0xff bytes.
#[must_use]
pub fn prefix_end(prefix: &[u8]) -> Vec<u8> {
    let mut end = prefix.to_vec();
    while let Some(last) = end.pop() {
        if last < 0xff {
            end.push(last + 1);
            return end;
        }
    }
    // Every byte was 0xff (or the prefix was empty): the whole keyspace.
    vec![0]
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding, as the gateway's protobuf JSON mapping
/// writes `bytes` fields.
#[must_use]
pub fn b64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = match chunk.len() {
            3 => (u32::from(chunk[0]) << 16) | (u32::from(chunk[1]) << 8) | u32::from(chunk[2]),
            2 => (u32::from(chunk[0]) << 16) | (u32::from(chunk[1]) << 8),
            _ => u32::from(chunk[0]) << 16,
        };
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(B64[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Decode standard base64 (padding optional); `None` for anything else.
#[must_use]
pub fn unb64(s: &str) -> Option<Vec<u8>> {
    let s = s.trim_end_matches('=');
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0u32;
    for c in s.bytes() {
        let v = B64.iter().position(|&b| b == c)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xff) as u8);
        }
    }
    Some(out)
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
        let rows = plan(
            DEFAULT_PREFIX,
            &PlanInput {
                hosts: &hosts,
                direct_live: &set(&["direct"]),
                relayed_live: &set(&["acme.mattbox", "acme.nomesh", "acme.direct", "acme.unbound"]),
                mesh: &set(&["mattbox", "direct", "gone", "unbound"]),
                bindings: &bindings,
                now,
            },
        );
        let keys: Vec<&str> = rows.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "/losos/routes/acme/mattbox/0123456789abcdef.boxes.example.test",
                "/losos/routes/acme/mattbox/cloud.example.org",
            ]
        );
        let row = &rows["/losos/routes/acme/mattbox/cloud.example.org"];
        assert_eq!(row.service, "acme.mattbox");
        assert_eq!(row.tenant, "mattbox");
    }

    #[test]
    fn the_diff_puts_what_changed_and_deletes_what_went() {
        let row = |d: &str| RouteRow {
            domain: d.to_string(),
            tenant: "mattbox".to_string(),
            spoke: "acme".to_string(),
            service: "acme.mattbox".to_string(),
        };
        let current: BTreeMap<String, RouteRow> = [("k/a", row("a")), ("k/b", row("b"))]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        let mut moved = row("b");
        moved.spoke = "home".to_string();
        let desired: BTreeMap<String, RouteRow> = [("k/b", moved.clone()), ("k/c", row("c"))]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        let (puts, deletes) = diff(&current, &desired);
        assert_eq!(
            puts,
            vec![("k/b".to_string(), moved), ("k/c".to_string(), row("c"))]
        );
        assert_eq!(deletes, vec!["k/a".to_string()]);
    }

    #[test]
    fn base64_round_trips_and_matches_the_standard_alphabet() {
        for (raw, enc) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("/losos/routes/", "L2xvc29zL3JvdXRlcy8="),
        ] {
            assert_eq!(b64(raw.as_bytes()), enc);
            assert_eq!(unb64(enc).unwrap(), raw.as_bytes());
        }
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(unb64(&b64(&all)).unwrap(), all);
        assert!(unb64("not base64!").is_none());
    }

    #[test]
    fn the_range_end_covers_exactly_the_prefix() {
        assert_eq!(prefix_end(b"/losos/routes/"), b"/losos/routes0");
        assert_eq!(prefix_end(&[b'a', 0xff]), b"b");
        assert_eq!(prefix_end(&[0xff]), vec![0]);
    }
}
