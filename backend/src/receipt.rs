//! The claim's reply, kept in memory so it can be given a second time.
//!
//! `POST /api/setup/claim` sets the owner's first password and hands back the
//! admin key — once, to the caller who set it, because after that call the
//! route refuses everything ([`crate::losos::cmd_claim`]). That "once" has a
//! hole the recorded install demo of 2026-10-05 (take 6) fell straight into:
//! on a box still warming up, `occ user:resetpassword` took longer than the
//! proxy in front of lososd was willing to wait, the browser got a 504 and the
//! connection was gone when the reply was ready. lososd had done its work —
//! password set, box claimed — and the one copy of the admin key went to
//! nobody. On a box with no shell that owner is locked out of the admin UI
//! for good, with a password that works.
//!
//! So the daemon remembers the claim it just answered: a salted digest of
//! the password, the account it set, and when. A second claim within
//! [`GRACE`] that presents the *same* password gets the same reply again.
//! Nothing else does: a different password, or the same one later, is refused
//! exactly as before. What this hands out is bounded by what the caller
//! already holds — a password that signs them in to everything on the box —
//! and by the clock. The digest lives only in this process, never on disk.

use std::time::{Duration, Instant};

/// How long after a claim the same password can ask for the reply again.
/// Long enough for a browser to retry a request that timed out, by hand or
/// on its own; short enough that it is over before anyone wonders about it.
pub const GRACE: Duration = Duration::from_secs(15 * 60);

/// The claim lososd last answered.
#[derive(Debug, Clone)]
struct Receipt {
    salt: [u8; 16],
    digest: [u8; 32],
    user: String,
    at: Instant,
}

/// The daemon's memory of its last successful claim. One, not a list: a box
/// is claimed once.
#[derive(Debug, Default)]
pub struct Receipts {
    last: Option<Receipt>,
}

impl Receipts {
    /// Note that `password` was just set for `user`, as of now.
    pub fn remember(&mut self, user: &str, password: &str) {
        self.remember_at(user, password, fresh_salt(), Instant::now());
    }

    /// The account the claim set, when `password` is the one it set and the
    /// window is still open; `None` otherwise. Pure in `now` for the tests.
    pub fn replay(&self, password: &str) -> Option<&str> {
        self.replay_at(password, Instant::now())
    }

    pub fn remember_at(&mut self, user: &str, password: &str, salt: [u8; 16], now: Instant) {
        self.last = Some(Receipt {
            salt,
            digest: digest(&salt, password),
            user: user.to_string(),
            at: now,
        });
    }

    pub fn replay_at(&self, password: &str, now: Instant) -> Option<&str> {
        let r = self.last.as_ref()?;
        if now.saturating_duration_since(r.at) > GRACE {
            return None;
        }
        if !constant_time_eq(&digest(&r.salt, password), &r.digest) {
            return None;
        }
        Some(&r.user)
    }
}

fn digest(salt: &[u8; 16], password: &str) -> [u8; 32] {
    let mut data = Vec::with_capacity(salt.len() + password.len());
    data.extend_from_slice(salt);
    data.extend_from_slice(password.as_bytes());
    crate::boxid::sha256(&data)
}

/// Equal without an early exit, so the comparison's timing says nothing
/// about how many leading bytes matched.
fn constant_time_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Sixteen bytes from `/dev/urandom`; if that cannot be read, the digest is
/// salted with the clock instead. The salt only keeps two receipts for the
/// same password from sharing a digest, and the receipt never leaves RAM,
/// so a weak salt costs less than a claim refused for want of one.
fn fresh_salt() -> [u8; 16] {
    use std::io::Read;
    let mut salt = [0u8; 16];
    let read = std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut salt));
    if read.is_err() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        salt = nanos.to_le_bytes();
    }
    salt
}

#[cfg(test)]
mod tests {
    use super::*;

    const SALT: [u8; 16] = [7; 16];

    #[test]
    fn the_same_password_inside_the_window_gets_the_account_back() {
        let t0 = Instant::now();
        let mut r = Receipts::default();
        assert_eq!(
            r.replay_at("anything", t0),
            None,
            "nothing to replay before a claim"
        );
        r.remember_at("notshared", "zqx-marmalade-77-parapet", SALT, t0);
        assert_eq!(
            r.replay_at("zqx-marmalade-77-parapet", t0 + Duration::from_secs(90)),
            Some("notshared")
        );
    }

    #[test]
    fn another_password_or_a_late_one_is_refused() {
        let t0 = Instant::now();
        let mut r = Receipts::default();
        r.remember_at("notshared", "zqx-marmalade-77-parapet", SALT, t0);
        assert_eq!(r.replay_at("zqx-marmalade-77-parapeT", t0), None);
        assert_eq!(r.replay_at("", t0), None);
        assert_eq!(
            r.replay_at("zqx-marmalade-77-parapet", t0 + GRACE),
            Some("notshared")
        );
        assert_eq!(
            r.replay_at(
                "zqx-marmalade-77-parapet",
                t0 + GRACE + Duration::from_secs(1)
            ),
            None
        );
    }

    #[test]
    fn the_salt_is_in_the_digest() {
        assert_ne!(digest(&[1; 16], "pw"), digest(&[2; 16], "pw"));
        assert_eq!(digest(&SALT, "pw"), digest(&SALT, "pw"));
    }
}
