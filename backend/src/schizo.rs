//! Schizo mode, as the owner's account carries it to every computer LosOS
//! Login manages the logins of.
//!
//! On LosOS Desktop schizo mode sends every connection through Tor. Here
//! it is a setting of the owner's, kept by this daemon and handed to every
//! computer joined to this box, so turning it on once turns it on on all
//! of them. On the box itself it takes the owner off the market: no
//! listing, no order, no payout, nothing bought or sold with this box's
//! identity, and the box's open listings are closed.
//!
//! Its full form also arms a duress code, as GrapheneOS has one. Typed in
//! place of a password at a computer's login or lock screen, it wipes that
//! computer to its factory state, and the computer tells this box, which
//! hands the same order to each of the owner's other computers the next
//! time it checks in. An order waits for a computer that is off or away
//! for as long as that takes, because the box keeps it.
//!
//! # Computers
//!
//! The computers are the ones joined to this box's directory
//! (`crate::directory`), with the token they read the people list with.
//! The same token makes three more calls here, and nothing else does: check
//! in, confirm a wipe, and report the duress code. The calls are open beyond
//! the LAN (`modules/identity.nix`), since a computer away from the office,
//! or one whose connections all go through Tor, still has to hear of a
//! wipe. Reporting the duress code needs the code, which the box checks
//! against its own hash: a stolen token can read whether schizo mode is
//! on, and cannot start a wipe.
//!
//! Each computer is given the duress code's argon2id hash only while the
//! full form is on; turning the full form off takes it back at the next
//! check-in.
//!
//! The document lives beside lososd's state, root's only, in
//! `/var/lib/losos/schizo.json`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::setup::Secret;

const DEFAULT_FILE: &str = "/var/lib/losos/schizo.json";
/// Shortest duress code. GrapheneOS asks for a PIN at least as long as the
/// device's own; six is the floor its lock screen sets.
pub const MIN_DURESS: usize = 6;
const MAX_DURESS: usize = 128;

/// What this box knows of one joined computer, by its directory id, from
/// the first time it checked in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// Its name in the directory.
    #[serde(default)]
    pub name: String,
    /// When it last checked in, seconds since the epoch.
    #[serde(default)]
    pub seen: Option<u64>,
    /// When it confirmed the wipe order, which it carries out next.
    #[serde(default)]
    pub wiped: Option<u64>,
    /// Its own Tor address, when it has one, which its check-ins carry.
    #[serde(default)]
    pub onion: Option<String>,
}

/// A duress code was entered somewhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wipe {
    /// Names this order, so a computer that confirmed it once is not asked
    /// again by a later one.
    pub id: String,
    /// When it was entered.
    pub at: u64,
    /// The computer it was entered on.
    pub from: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Document {
    #[serde(default)]
    on: bool,
    /// The full form: the duress code is armed.
    #[serde(default)]
    full: bool,
    /// When it last changed.
    #[serde(default)]
    since: u64,
    /// The duress code's argon2id hash, while `full`.
    #[serde(default)]
    duress: Option<String>,
    #[serde(default)]
    devices: BTreeMap<String, Device>,
    #[serde(default)]
    wipe: Option<Wipe>,
}

/// Why a change was refused, in words the page can show.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Refused {
    #[error(
        "the duress code must be {} to {} characters long",
        MIN_DURESS,
        MAX_DURESS
    )]
    DuressLength,
    #[error("the duress code cannot be the owner's password")]
    DuressIsPassword,
    #[error("the full form needs a duress code")]
    DuressMissing,
}

impl Refused {
    pub fn status(&self) -> u16 {
        400
    }
}

/// What a computer that checked in is told.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    pub on: bool,
    pub full: bool,
    pub since: u64,
    /// The duress code's hash, while the full form is on.
    pub duress: Option<String>,
    /// A wipe order this computer has not confirmed yet.
    pub wipe: Option<String>,
    /// While schizo mode is on, where the box and every other computer of
    /// the owner's can be reached on Tor.
    pub peers: Vec<Peer>,
}

/// One device's Tor address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Peer {
    /// `box`, or the computer's directory id.
    pub id: String,
    pub name: String,
    pub onion: String,
}

/// What a reported duress code did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reported {
    /// It was not the code; nothing happened.
    Wrong,
    /// It was, and this is the first report: the order goes out now.
    Wipe,
    /// It was, and an order was already out.
    Already,
}

/// The document and one lock around every read-modify-write.
pub struct Schizo {
    path: PathBuf,
    lock: Mutex<()>,
}

impl Schizo {
    pub fn new(path: impl Into<PathBuf>) -> Schizo {
        Schizo {
            path: path.into(),
            lock: Mutex::new(()),
        }
    }

    /// `$LOSOS_SCHIZO_FILE`, or `/var/lib/losos/schizo.json`.
    pub fn from_env() -> Schizo {
        Schizo::new(std::env::var("LOSOS_SCHIZO_FILE").unwrap_or_else(|_| DEFAULT_FILE.into()))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn read(&self) -> anyhow::Result<Document> {
        match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| anyhow::anyhow!("reading {}: {e}", self.path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Document::default()),
            Err(e) => Err(anyhow::anyhow!("reading {}: {e}", self.path.display())),
        }
    }

    fn write(&self, doc: &Document) -> anyhow::Result<()> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("tmp");
        {
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp)?;
            f.write_all(&serde_json::to_vec_pretty(doc)?)?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    fn change<T>(&self, f: impl FnOnce(&mut Document) -> anyhow::Result<T>) -> anyhow::Result<T> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let before = doc.clone();
        let out = f(&mut doc)?;
        if doc != before {
            self.write(&doc)?;
        }
        Ok(out)
    }

    /// Whether schizo mode is on. Read on every market call, and a file
    /// that cannot be read counts as on: the market is the side to fail to.
    pub fn on(&self) -> bool {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        self.read().map(|d| d.on).unwrap_or(true)
    }

    /// The setting and the computers, for the owner's pages. Never a hash.
    pub fn status(&self) -> anyhow::Result<Value> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let doc = self.read()?;
        Ok(json!({
            "on": doc.on,
            "full": doc.full,
            "since": doc.since,
            "devices": doc.devices.iter().map(|(id, d)| json!({
                "id": id,
                "name": d.name,
                "seen": d.seen,
                "wiped": d.wiped,
                "onion": d.onion,
            })).collect::<Vec<_>>(),
            "wipe": doc.wipe.as_ref().map(|w| json!({ "id": w.id, "at": w.at })),
        }))
    }

    /// Turn schizo mode on or off, with the duress code for the full form.
    /// `is_password` says whether a candidate is the owner's password, which
    /// the code may not be: typing their password under duress must log them
    /// in, not wipe everything.
    pub fn set(
        &self,
        on: bool,
        full: bool,
        duress: Option<&Secret>,
        is_password: impl FnOnce(&Secret) -> bool,
        now: u64,
    ) -> anyhow::Result<()> {
        let hash = match (on && full, duress) {
            (true, Some(code)) => Some(duress_hash(code, is_password)?),
            (true, None) => return Err(Refused::DuressMissing.into()),
            (false, _) => None,
        };
        self.change(|doc| {
            // Off takes the full form, and the code, with it.
            let full = on && full;
            if doc.on != on || doc.full != full || hash.is_some() {
                doc.since = now;
            }
            doc.on = on;
            doc.full = full;
            doc.duress = hash;
            Ok(())
        })
    }

    /// What computer `id` (`name` in the directory) is told when it checks
    /// in, which also counts as seeing it. `onion` is its own Tor address,
    /// if it has one by now; `box_onion` the box's
    /// (`/var/lib/losos-hidden/addresses.json`).
    pub fn policy(
        &self,
        id: &str,
        name: &str,
        onion: Option<&str>,
        box_onion: Option<&str>,
        now: u64,
    ) -> anyhow::Result<Policy> {
        let onion = onion.filter(|o| valid_onion(o));
        self.change(|doc| {
            let wipe = doc.wipe.as_ref().map(|w| w.id.clone());
            let device = doc.devices.entry(id.to_string()).or_default();
            device.name = name.to_string();
            device.seen = Some(now);
            if let Some(onion) = onion {
                device.onion = Some(onion.to_string());
            }
            let wipe = wipe.filter(|_| device.wiped.is_none());
            // The addresses only while schizo mode is on: off, each computer
            // goes back to the network it is on, and nobody needs them.
            let mut peers = Vec::new();
            if doc.on {
                if let Some(onion) = box_onion.filter(|o| valid_onion(o)) {
                    peers.push(Peer {
                        id: "box".into(),
                        name: "box".into(),
                        onion: onion.to_string(),
                    });
                }
                peers.extend(
                    doc.devices
                        .iter()
                        .filter(|(other, _)| *other != id)
                        .filter_map(|(other, d)| {
                            Some(Peer {
                                id: other.clone(),
                                name: d.name.clone(),
                                onion: d.onion.clone()?,
                            })
                        }),
                );
            }
            Ok(Policy {
                on: doc.on,
                full: doc.full,
                since: doc.since,
                duress: doc.duress.clone().filter(|_| doc.full),
                wipe,
                peers,
            })
        })
    }

    /// `id` confirms order `wipe` and is about to carry it out. False when
    /// that is not the order out, or `id` never checked in to hear it.
    pub fn ack(&self, id: &str, wipe: &str, now: u64) -> anyhow::Result<bool> {
        self.change(|doc| {
            if doc.wipe.as_ref().map(|w| w.id.as_str()) != Some(wipe) {
                return Ok(false);
            }
            Ok(match doc.devices.get_mut(id) {
                Some(device) => {
                    device.wiped.get_or_insert(now);
                    true
                }
                None => false,
            })
        })
    }

    /// `id` reports that `code` was typed at its login. A match starts the
    /// wipe order, whichever computer it came from, and that computer counts
    /// as having confirmed it: it is already wiping itself.
    pub fn report(&self, id: &str, code: &Secret, now: u64) -> anyhow::Result<Reported> {
        self.change(|doc| {
            let matches = doc.full
                && doc
                    .duress
                    .as_deref()
                    .is_some_and(|h| crate::people::verify_password(h, code));
            if !matches {
                return Ok(Reported::Wrong);
            }
            let first = doc.wipe.is_none();
            if first {
                doc.wipe = Some(Wipe {
                    id: format!("wipe_{}", random_hex(8)),
                    at: now,
                    from: id.to_string(),
                });
                for device in doc.devices.values_mut() {
                    device.wiped = None;
                }
            }
            doc.devices
                .entry(id.to_string())
                .or_default()
                .wiped
                .get_or_insert(now);
            Ok(if first {
                Reported::Wipe
            } else {
                Reported::Already
            })
        })
    }

    /// Computers that have checked in and not confirmed the order yet, by
    /// name.
    pub fn waiting(&self) -> anyhow::Result<Vec<String>> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let doc = self.read()?;
        Ok(match doc.wipe {
            None => Vec::new(),
            Some(_) => doc
                .devices
                .values()
                .filter(|d| d.wiped.is_none())
                .map(|d| d.name.clone())
                .collect(),
        })
    }
}

/// A duress code's argon2id hash, Authelia's parameters, as the people file
/// keeps passwords.
fn duress_hash(code: &Secret, is_password: impl FnOnce(&Secret) -> bool) -> anyhow::Result<String> {
    let len = code.expose().chars().count();
    if !(MIN_DURESS..=MAX_DURESS).contains(&len) {
        return Err(Refused::DuressLength.into());
    }
    if is_password(code) {
        return Err(Refused::DuressIsPassword.into());
    }
    crate::people::hash_password(code)
}

/// A v3 onion address: 56 base32 letters and `.onion`.
pub fn valid_onion(name: &str) -> bool {
    name.strip_suffix(".onion").is_some_and(|label| {
        label.len() == 56
            && label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b))
    })
}

/// The box's own Tor address, as modules/hidden-lib.nix lists it.
pub fn box_onion(addresses: &Path) -> Option<String> {
    let doc: Value = serde_json::from_slice(&std::fs::read(addresses).ok()?).ok()?;
    doc.pointer("/front/tor")
        .and_then(Value::as_str)
        .filter(|o| valid_onion(o))
        .map(str::to_string)
}

/// `overrides.nix` with `losos.hidden.tor.enable = true;`, or `None` when it
/// says so already. Replaces an assignment in place, otherwise goes in
/// before the closing brace, as `crate::overrides::inject_line` does.
pub fn with_tor(overrides: &str) -> Option<String> {
    const LINE: &str = "  losos.hidden.tor.enable = true;";
    let mut lines: Vec<String> = overrides.lines().map(str::to_string).collect();
    let assign = |l: &String| l.contains("losos.hidden.tor.enable") && l.contains('=');
    match lines.iter().position(assign) {
        Some(i) if lines[i].trim() == LINE.trim() => return None,
        Some(i) => lines[i] = LINE.to_string(),
        None => match lines.iter().rposition(|l| l.contains('}')) {
            Some(i) => lines.insert(i, LINE.to_string()),
            None => lines.push(LINE.to_string()),
        },
    }
    let mut body = lines.join("\n");
    body.push('\n');
    Some(body)
}

fn random_hex(bytes: usize) -> String {
    use ring::rand::SecureRandom;
    let mut buf = vec![0u8; bytes];
    ring::rand::SystemRandom::new()
        .fill(&mut buf)
        .expect("the OS has randomness");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Whether a route is the market's, which schizo mode closes: everything
/// bought or sold with the box's identity, storage, compute, machines and
/// model time alike.
pub fn market_route(path: &str) -> bool {
    ["/api/market", "/api/vms"].iter().any(|p| {
        path == *p
            || path
                .strip_prefix(p)
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, Schizo) {
        let dir = tempfile::tempdir().unwrap();
        let s = Schizo::new(dir.path().join("schizo.json"));
        (dir, s)
    }

    fn code(s: &str) -> Secret {
        Secret::new(s)
    }

    fn check_in(s: &Schizo, id: &str, now: u64) -> Policy {
        s.policy(id, id, None, None, now).unwrap()
    }

    #[test]
    fn off_by_default_and_nothing_to_hand_out() {
        let (_d, s) = store();
        assert!(!s.on());
        let p = check_in(&s, "a", 2);
        assert_eq!((p.on, p.full, p.duress, p.wipe), (false, false, None, None));
    }

    #[test]
    fn an_unreadable_file_closes_the_market() {
        let (d, s) = store();
        std::fs::write(d.path().join("schizo.json"), "not json").unwrap();
        assert!(s.on());
    }

    #[test]
    fn the_full_form_needs_a_code_that_is_not_the_password() {
        let (_d, s) = store();
        let err = s.set(true, true, None, |_| false, 1).unwrap_err();
        assert_eq!(err.downcast_ref(), Some(&Refused::DuressMissing));
        let err = s
            .set(true, true, Some(&code("12345")), |_| false, 1)
            .unwrap_err();
        assert_eq!(err.downcast_ref(), Some(&Refused::DuressLength));
        let err = s
            .set(
                true,
                true,
                Some(&code("hunter22")),
                |c| c.expose() == "hunter22",
                1,
            )
            .unwrap_err();
        assert_eq!(err.downcast_ref(), Some(&Refused::DuressIsPassword));
        assert!(!s.on(), "nothing changed");
        s.set(true, true, Some(&code("482916")), |_| false, 5)
            .unwrap();
        assert!(s.on());
    }

    #[test]
    fn every_computer_hears_of_it_and_off_takes_the_code_back() {
        let (_d, s) = store();
        s.set(true, true, Some(&code("482916")), |_| false, 5)
            .unwrap();
        for id in ["a", "b"] {
            let p = check_in(&s, id, 6);
            assert!(p.on && p.full);
            let hash = p.duress.expect("the hash, while full");
            assert!(crate::people::verify_password(&hash, &code("482916")));
        }
        s.set(true, false, None, |_| false, 7).unwrap();
        let p = check_in(&s, "a", 8);
        assert!(p.on && !p.full && p.duress.is_none());
        s.set(false, true, Some(&code("482916")), |_| false, 9)
            .unwrap();
        let p = check_in(&s, "a", 10);
        assert!(
            !p.on && !p.full && p.duress.is_none(),
            "off is off, code or not"
        );
    }

    #[test]
    fn a_wrong_code_does_nothing() {
        let (_d, s) = store();
        s.set(true, true, Some(&code("482916")), |_| false, 5)
            .unwrap();
        check_in(&s, "a", 5);
        assert_eq!(s.report("a", &code("000000"), 6).unwrap(), Reported::Wrong);
        assert_eq!(check_in(&s, "a", 7).wipe, None);
    }

    #[test]
    fn without_the_full_form_the_code_starts_nothing() {
        let (_d, s) = store();
        s.set(true, true, Some(&code("482916")), |_| false, 5)
            .unwrap();
        s.set(true, false, None, |_| false, 6).unwrap();
        assert_eq!(s.report("a", &code("482916"), 7).unwrap(), Reported::Wrong);
    }

    #[test]
    fn the_order_waits_for_every_other_computer_until_it_confirms() {
        let (_d, s) = store();
        s.set(true, true, Some(&code("482916")), |_| false, 5)
            .unwrap();
        for id in ["a", "b", "c"] {
            check_in(&s, id, 5);
        }
        assert_eq!(s.report("a", &code("482916"), 6).unwrap(), Reported::Wipe);
        assert_eq!(
            s.report("b", &code("482916"), 6).unwrap(),
            Reported::Already
        );
        // a typed it and b reported it too: both are wiping already.
        assert_eq!(check_in(&s, "a", 7).wipe, None);
        assert_eq!(check_in(&s, "b", 7).wipe, None);
        // c was off. Days later it checks in and hears of it.
        let order = check_in(&s, "c", 900_000).wipe.expect("the order waited");
        assert_eq!(s.waiting().unwrap(), vec!["c".to_string()]);
        assert!(
            !s.ack("c", "wipe_other", 900_001).unwrap(),
            "only this order"
        );
        assert!(s.ack("c", &order, 900_001).unwrap());
        assert_eq!(check_in(&s, "c", 900_002).wipe, None);
        assert!(s.waiting().unwrap().is_empty());
        // d joined later and never confirmed: it hears of it too.
        assert!(check_in(&s, "d", 900_003).wipe.is_some());
    }

    #[test]
    fn onion_addresses_go_round_while_it_is_on() {
        let (_d, s) = store();
        let onion_a = format!("{}.onion", "a".repeat(56));
        let onion_box = format!("{}.onion", "b".repeat(56));
        let p = s
            .policy("a", "a", Some(&onion_a), Some(&onion_box), 2)
            .unwrap();
        assert!(p.peers.is_empty());
        s.set(true, false, None, |_| false, 3).unwrap();
        let p = s
            .policy("b", "b", Some("not an onion"), Some(&onion_box), 4)
            .unwrap();
        let peers: Vec<_> = p
            .peers
            .iter()
            .map(|p| (p.id.as_str(), p.onion.as_str()))
            .collect();
        assert_eq!(
            peers,
            [("box", onion_box.as_str()), ("a", onion_a.as_str())]
        );
        let p = check_in(&s, "a", 5);
        assert!(
            p.peers.is_empty(),
            "b gave no address, and the box none this time"
        );
    }

    #[test]
    fn tor_goes_into_the_overrides_once() {
        let body = "{ ... }:\n{\n  losos.hostName = \"box\";\n}\n";
        let with = with_tor(body).unwrap();
        assert_eq!(
            with,
            "{ ... }:\n{\n  losos.hostName = \"box\";\n  losos.hidden.tor.enable = true;\n}\n"
        );
        assert_eq!(with_tor(&with), None);
        let off = with.replace("= true", "= false");
        assert_eq!(with_tor(&off).unwrap(), with);
    }

    #[test]
    fn onions() {
        assert!(valid_onion(&format!("{}.onion", "a2".repeat(28))));
        assert!(!valid_onion(&format!("{}.onion", "A".repeat(56))));
        assert!(!valid_onion("short.onion"));
    }

    #[test]
    fn market_routes() {
        for p in [
            "/api/market",
            "/api/market/orders",
            "/api/vms",
            "/api/vms/images",
        ] {
            assert!(market_route(p), "{p}");
        }
        for p in ["/api/marketing", "/api/vmsx", "/api/machines", "/api/me"] {
            assert!(!market_route(p), "{p}");
        }
    }
}
