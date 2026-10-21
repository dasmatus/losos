//! `GET /internal/cloud-share`: the LosOS Cloud share opened with a LosOS
//! Login name and password.
//!
//! A file manager opens LosOS Cloud's WebDAV share
//! (`/remote.php/webdav/`, announced on the LAN by `modules/cloud.nix`)
//! and asks for a name and a password, which it
//! then sends as Basic auth on every request. LosOS Cloud itself knows the
//! owner only as `notshared` and the other people by no password at all, so
//! nginx asks this route about every request to the share from the LAN
//! (`modules/identity.nix`, `/internal/losos-cloud-share`) and gets one of
//! two answers, both a 200:
//!
//!   * a person, in `Remote-User`, `Remote-Groups` and `Remote-Name`, when
//!     the request carries a LosOS Login name and that person's password.
//!     nginx then drops the `Authorization` header and names the person to
//!     LosOS Cloud in the sign-in headers, as it does for a browser
//!     (`modules/cloud.nix`): the owner arrives as `notshared`;
//!   * nobody, for everything else: no credentials, an app password, a
//!     name LosOS Login does not know, a wrong password. nginx passes the
//!     request on untouched and LosOS Cloud answers it with its own
//!     accounts and app passwords, as before.
//!
//! So nothing that worked stops working, and a wrong LosOS password is a
//! 401 from LosOS Cloud, which also counts it in its own throttle.
//!
//! Clients send the password on every request and an argon2id check costs
//! 64 MiB and a fraction of a second, so a name and password that checked
//! out are remembered for [`REMEMBER`] by their SHA-256 together with the
//! stored hash they matched: a changed password or a disabled person stops
//! matching at once. One check runs at a time. App passwords (72 letters and
//! digits, or five groups of five) never reach argon2 at all, so the
//! desktop and phone apps cost a header parse per request. Failures are
//! counted per name, and past the throttle's budget a name is not checked
//! until its window ends.
//!
//! LosOS Cloud makes a person's account the first time LosOS Login sends
//! them, and only on its sign-in page. A person who opens the share before
//! they ever opened LosOS Cloud in a browser would get a 401 there, so the
//! first time this daemon vouches for someone it signs them in once on
//! loopback the way nginx would ([`crate::pins::CloudRequest::Arrive`]).

use std::collections::{BTreeSet, HashMap};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::guard::Throttle;
use crate::losos::Losos;
use crate::people::{People, Person, OWNER};
use crate::setup::Secret;

/// How long a name and password that checked out are taken on trust.
pub const REMEMBER: Duration = Duration::from_secs(300);
/// Entries remembered at most; past this the oldest goes.
const MAX_REMEMBERED: usize = 256;

/// What nginx is told.
#[derive(Debug, PartialEq)]
pub enum Answer {
    /// Pass the request on as it came.
    Nobody,
    /// The request is this person's.
    Person(Person),
}

#[derive(Default)]
pub struct Share {
    /// SHA-256 of name, password and stored hash, and when it checked out.
    remembered: Mutex<HashMap<[u8; 32], Instant>>,
    failures: Throttle,
    checking: Mutex<()>,
    /// People LosOS Cloud has been sent once since this daemon started.
    arrived: Mutex<BTreeSet<String>>,
}

impl Share {
    /// The answer for a request carrying `authorization`, at `now`.
    pub fn answer(
        &self,
        people: &People,
        backend: &mut impl Losos,
        authorization: Option<&str>,
        now: Instant,
    ) -> Answer {
        let Some((name, password)) = authorization.and_then(basic_credentials) else {
            return Answer::Nobody;
        };
        if is_app_password(&password) {
            return Answer::Nobody;
        }
        let (person, hash) = match people.with_hash(&name) {
            Ok(Some(found)) => found,
            Ok(None) => return Answer::Nobody,
            Err(e) => {
                tracing::error!(error = ?e, "the people file could not be read");
                return Answer::Nobody;
            }
        };
        let key = fingerprint(&name, &password, &hash);
        if !self.recall(&key, now) {
            if self.failures.check(&name, now).is_err() {
                return Answer::Nobody;
            }
            let ok = {
                let _one = self.checking.lock().unwrap_or_else(|e| e.into_inner());
                crate::people::verify_password(&hash, &Secret::new(&password))
            };
            if !ok {
                self.failures.record_failure(&name, now);
                return Answer::Nobody;
            }
            self.failures.record_success(&name);
            self.remember(key, now);
        }
        self.arrive(backend, &person);
        Answer::Person(person)
    }

    fn recall(&self, key: &[u8; 32], now: Instant) -> bool {
        let mut m = self.remembered.lock().unwrap_or_else(|e| e.into_inner());
        match m.get(key) {
            Some(at) if now.duration_since(*at) < REMEMBER => true,
            Some(_) => {
                m.remove(key);
                false
            }
            None => false,
        }
    }

    fn remember(&self, key: [u8; 32], now: Instant) {
        let mut m = self.remembered.lock().unwrap_or_else(|e| e.into_inner());
        m.retain(|_, at| now.duration_since(*at) < REMEMBER);
        if m.len() >= MAX_REMEMBERED {
            if let Some(oldest) = m.iter().min_by_key(|(_, at)| **at).map(|(k, _)| *k) {
                m.remove(&oldest);
            }
        }
        m.insert(key, now);
    }

    /// Send `person` to LosOS Cloud's sign-in once, so their account
    /// exists. The owner's always does. A failure is only logged: the
    /// request goes on and LosOS Cloud answers it.
    fn arrive(&self, backend: &mut impl Losos, person: &Person) {
        if person.groups.iter().any(|g| g == OWNER) {
            return;
        }
        {
            let arrived = self.arrived.lock().unwrap_or_else(|e| e.into_inner());
            if arrived.contains(&person.name) {
                return;
            }
        }
        let request = crate::pins::CloudRequest::Arrive {
            user: &person.name,
            display_name: &person.display_name,
        };
        match backend.cloud_request(&request) {
            // A redirect onwards; anything else is LosOS Cloud's error page.
            Ok((status, _)) if (200..400).contains(&status) => {
                self.arrived
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(person.name.clone());
            }
            Ok((status, _)) => {
                tracing::warn!(status, person = %person.name, "LosOS Cloud did not take the person");
            }
            Err(e) => tracing::warn!(error = %e, "could not reach LosOS Cloud"),
        }
    }
}

/// Name and password out of `Authorization: Basic …`, when it is that and
/// the name is one LosOS Login could have.
pub fn basic_credentials(header: &str) -> Option<(String, String)> {
    let (scheme, encoded) = header.trim().split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("basic") {
        return None;
    }
    let decoded = String::from_utf8(base64_decode(encoded.trim())?).ok()?;
    let (name, password) = decoded.split_once(':')?;
    crate::people::validate_name(name).ok()?;
    (!password.is_empty()).then(|| (name.to_string(), password.to_string()))
}

/// Whether `password` has the shape of one LosOS Cloud made: 72 letters and
/// digits from a login flow, or five dash-joined groups of five from its
/// security settings. A LosOS password of either shape is still taken by
/// LosOS Cloud's own check if its owner made it an app password.
pub fn is_app_password(password: &str) -> bool {
    let flow = password.len() == 72 && password.bytes().all(|b| b.is_ascii_alphanumeric());
    let typed = password.split('-').count() == 5
        && password
            .split('-')
            .all(|g| g.len() == 5 && g.bytes().all(|b| b.is_ascii_alphanumeric()));
    flow || typed
}

fn fingerprint(name: &str, password: &str, hash: &str) -> [u8; 32] {
    let mut ctx = ring::digest::Context::new(&ring::digest::SHA256);
    for part in [name, password, hash] {
        ctx.update(&(part.len() as u64).to_be_bytes());
        ctx.update(part.as_bytes());
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(ctx.finish().as_ref());
    out
}

/// Standard base64 with padding, as Basic auth uses.
fn base64_decode(text: &str) -> Option<Vec<u8>> {
    fn value(c: u8) -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    }
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for (i, chunk) in bytes.chunks(4).enumerate() {
        let last = i == bytes.len() / 4 - 1;
        let pad = chunk.iter().rev().take_while(|b| **b == b'=').count();
        if pad > 2 || (pad > 0 && !last) {
            return None;
        }
        let mut n = 0u32;
        for &c in &chunk[..4 - pad] {
            n = (n << 6) | value(c)?;
        }
        n <<= 6 * pad as u32;
        let all = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&all[..3 - pad]);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeLosos;
    use crate::people::ADMINS;

    fn basic(name: &str, password: &str) -> String {
        format!(
            "Basic {}",
            crate::signin::base64(format!("{name}:{password}").as_bytes())
        )
    }

    fn people() -> (assert_fs::TempDir, People) {
        let dir = assert_fs::TempDir::new().unwrap();
        let p = People::new(dir.path().join("users.yml"), None);
        p.add(
            "matus",
            "Matus",
            "",
            &Secret::new("Owner-Long-Password-1"),
            &[ADMINS, OWNER],
        )
        .unwrap();
        p.add(
            "eva",
            "Eva",
            "",
            &Secret::new("Eva-Long-Password-3"),
            &["losos-viewers"],
        )
        .unwrap();
        (dir, p)
    }

    #[test]
    fn base64_round_trips_with_and_without_padding() {
        for text in ["", "a", "ab", "abc", "matus:pass word:with colon"] {
            let encoded = crate::signin::base64(text.as_bytes());
            assert_eq!(
                base64_decode(&encoded).unwrap(),
                text.as_bytes(),
                "{text:?}"
            );
        }
        assert_eq!(base64_decode("YQ"), None);
        assert_eq!(base64_decode("Y=Q="), None);
        assert_eq!(base64_decode("YQ*="), None);
    }

    #[test]
    fn only_basic_with_a_sign_in_name_is_read() {
        assert_eq!(
            basic_credentials(&basic("eva", "a:b")),
            Some(("eva".into(), "a:b".into()))
        );
        assert_eq!(basic_credentials(&basic("Eva", "x")), None);
        assert_eq!(basic_credentials(&basic("notshared", "x")), None);
        assert_eq!(basic_credentials(&basic("eva", "")), None);
        assert_eq!(basic_credentials("Bearer abc"), None);
        assert_eq!(basic_credentials("Basic !!!!"), None);
    }

    #[test]
    fn app_passwords_are_recognised_by_shape() {
        assert!(is_app_password(&"aB3".repeat(24)));
        assert!(is_app_password("abcde-FGHIJ-12345-klmno-PQRST"));
        assert!(!is_app_password("Eva-Long-Password-3"));
        assert!(!is_app_password(&"a".repeat(71)));
        assert!(!is_app_password("abcde-FGHIJ-12345-klmno"));
    }

    #[test]
    fn the_owner_is_vouched_for_without_a_trip_to_losos_cloud() {
        let (_d, p) = people();
        let share = Share::default();
        let mut b = FakeLosos::default();
        let now = Instant::now();
        let auth = basic("matus", "Owner-Long-Password-1");
        match share.answer(&p, &mut b, Some(&auth), now) {
            Answer::Person(person) => assert_eq!(person.name, "matus"),
            other => panic!("{other:?}"),
        }
        assert!(b.cloud_calls.is_empty(), "{:?}", b.cloud_calls);
    }

    #[test]
    fn anyone_else_arrives_in_losos_cloud_once() {
        let (_d, p) = people();
        let share = Share::default();
        let mut b = FakeLosos::default();
        b.cloud_answers.push_back((303, Vec::new()));
        let now = Instant::now();
        let auth = basic("eva", "Eva-Long-Password-3");
        assert!(matches!(
            share.answer(&p, &mut b, Some(&auth), now),
            Answer::Person(_)
        ));
        assert!(matches!(
            share.answer(&p, &mut b, Some(&auth), now),
            Answer::Person(_)
        ));
        assert_eq!(b.cloud_calls, ["arrive eva"]);
    }

    #[test]
    fn everything_else_goes_on_to_losos_cloud_as_it_came() {
        let (_d, p) = people();
        let share = Share::default();
        let mut b = FakeLosos::default();
        let now = Instant::now();
        for auth in [
            None,
            Some(basic("eva", "wrong password")),
            Some(basic("nobody", "Eva-Long-Password-3")),
            Some(basic("eva", &"aB3".repeat(24))),
            Some("Bearer token".to_string()),
        ] {
            assert_eq!(
                share.answer(&p, &mut b, auth.as_deref(), now),
                Answer::Nobody,
                "{auth:?}"
            );
        }
    }

    #[test]
    fn a_remembered_password_stops_working_when_it_is_changed() {
        let (_d, p) = people();
        let share = Share::default();
        let mut b = FakeLosos::default();
        let now = Instant::now();
        let auth = basic("matus", "Owner-Long-Password-1");
        assert!(matches!(
            share.answer(&p, &mut b, Some(&auth), now),
            Answer::Person(_)
        ));
        p.set_password("matus", &Secret::new("Owner-New-Password-2"))
            .unwrap();
        assert_eq!(share.answer(&p, &mut b, Some(&auth), now), Answer::Nobody);
        let new = basic("matus", "Owner-New-Password-2");
        assert!(matches!(
            share.answer(&p, &mut b, Some(&new), now),
            Answer::Person(_)
        ));
    }

    #[test]
    fn a_name_guessed_at_is_not_checked_until_its_window_ends() {
        let (_d, p) = people();
        let share = Share::default();
        let mut b = FakeLosos::default();
        let now = Instant::now();
        for _ in 0..12 {
            share.answer(&p, &mut b, Some(&basic("matus", "guess")), now);
        }
        let right = basic("matus", "Owner-Long-Password-1");
        assert_eq!(share.answer(&p, &mut b, Some(&right), now), Answer::Nobody);
        let later = now + Duration::from_secs(600);
        assert!(matches!(
            share.answer(&p, &mut b, Some(&right), later),
            Answer::Person(_)
        ));
    }
}
