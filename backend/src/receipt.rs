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
    /// Note that `password` was just set for `user`, as of now. Without a
    /// salt from the kernel nothing is remembered: the claim itself has
    /// already succeeded, and losing the replay is the safer failure than a
    /// digest salted with something guessable.
    pub fn remember(&mut self, user: &str, password: &str) {
        match urandom::<16>() {
            Some(salt) => self.remember_at(user, password, salt, Instant::now()),
            None => {
                tracing::warn!("no salt from /dev/urandom; this claim cannot be replayed");
                self.last = None;
            }
        }
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

/// `N` bytes from `/dev/urandom`, or `None` when they cannot be read. No
/// fallback to the clock or a fixed value: a predictable salt is the one
/// thing this must not be, and the caller can do without a receipt.
fn urandom<const N: usize>() -> Option<[u8; N]> {
    use std::io::Read;
    let mut buf = Vec::with_capacity(N);
    std::fs::File::open("/dev/urandom")
        .ok()?
        .take(N as u64)
        .read_to_end(&mut buf)
        .ok()?;
    buf.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every salt and password here is drawn at run time rather than written
    // out: the tests are about equality and the clock, not about any value.
    fn salt() -> [u8; 16] {
        urandom().expect("/dev/urandom")
    }

    fn password() -> String {
        String::from_utf8_lossy(&urandom::<32>().expect("/dev/urandom")).into_owned()
    }

    #[test]
    fn the_same_password_inside_the_window_gets_the_account_back() {
        let t0 = Instant::now();
        let pw = password();
        let mut r = Receipts::default();
        assert_eq!(
            r.replay_at(&password(), t0),
            None,
            "nothing to replay before a claim"
        );
        r.remember_at("notshared", &pw, salt(), t0);
        assert_eq!(
            r.replay_at(&pw, t0 + Duration::from_secs(90)),
            Some("notshared")
        );
    }

    #[test]
    fn another_password_or_a_late_one_is_refused() {
        let t0 = Instant::now();
        let pw = password();
        let mut r = Receipts::default();
        r.remember_at("notshared", &pw, salt(), t0);
        assert_eq!(r.replay_at(&password(), t0), None);
        assert_eq!(r.replay_at(&pw[..0], t0), None);
        assert_eq!(r.replay_at(&pw, t0 + GRACE), Some("notshared"));
        assert_eq!(r.replay_at(&pw, t0 + GRACE + Duration::from_secs(1)), None);
    }

    #[test]
    fn remember_salts_from_the_kernel_and_replays() {
        let pw = password();
        let mut r = Receipts::default();
        r.remember("notshared", &pw);
        assert_eq!(r.replay(&pw), Some("notshared"));
    }

    #[test]
    fn the_salt_is_in_the_digest() {
        let pw = password();
        let s = salt();
        assert_ne!(digest(&s, &pw), digest(&salt(), &pw));
        assert_eq!(digest(&s, &pw), digest(&s, &pw));
    }
}
