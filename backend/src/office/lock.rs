//! WOPI locks.
//!
//! coolwsd takes a lock on a document when the first person opens it, keeps
//! it fresh while anyone edits, and lets go when the last one leaves. Every
//! save carries the lock, so a save from a session that lost it (a second
//! coolwsd, a crashed one, a stale tab) is refused instead of overwriting
//! the live document. The rules are MS-WOPI's: a lock is an opaque string,
//! held per file, and runs out after 30 minutes unless refreshed.
//!
//! Locks are kept in memory. A restart of the host drops them, which is what
//! a lock that times out would do anyway, and coolwsd takes them again on its
//! next refresh.

use std::collections::HashMap;
use std::sync::Mutex;

/// How long a lock lasts without a refresh.
pub const LOCK_TTL_MS: u64 = 30 * 60 * 1000;

#[derive(Debug, Default)]
pub struct Locks {
    held: Mutex<HashMap<String, Held>>,
}

#[derive(Debug, Clone)]
struct Held {
    lock: String,
    expires_ms: u64,
}

/// What a lock operation came to. `Conflict` carries the lock that is held
/// now, or an empty string for none, which WOPI returns in `X-WOPI-Lock`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Ok,
    Conflict(String),
}

impl Locks {
    /// The lock on `file`, if one is held at `now_ms`.
    pub fn current(&self, file: &str, now_ms: u64) -> Option<String> {
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        live(&mut held, file, now_ms).map(|h| h.lock.clone())
    }

    /// LOCK: take the lock, or refresh it if it is already ours.
    pub fn lock(&self, file: &str, lock: &str, now_ms: u64) -> Outcome {
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        match live(&mut held, file, now_ms) {
            Some(h) if h.lock != lock => Outcome::Conflict(h.lock.clone()),
            _ => {
                held.insert(file.into(), fresh(lock, now_ms));
                Outcome::Ok
            }
        }
    }

    /// LOCK with `X-WOPI-OldLock`: swap `old` for `new` in one step.
    pub fn unlock_and_relock(&self, file: &str, old: &str, new: &str, now_ms: u64) -> Outcome {
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        match live(&mut held, file, now_ms) {
            Some(h) if h.lock == old => {
                held.insert(file.into(), fresh(new, now_ms));
                Outcome::Ok
            }
            Some(h) => Outcome::Conflict(h.lock.clone()),
            None => Outcome::Conflict(String::new()),
        }
    }

    /// REFRESH_LOCK: push the expiry out, if the lock is ours.
    pub fn refresh(&self, file: &str, lock: &str, now_ms: u64) -> Outcome {
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        match live(&mut held, file, now_ms) {
            Some(h) if h.lock == lock => {
                h.expires_ms = now_ms + LOCK_TTL_MS;
                Outcome::Ok
            }
            Some(h) => Outcome::Conflict(h.lock.clone()),
            None => Outcome::Conflict(String::new()),
        }
    }

    /// UNLOCK: let go, if the lock is ours.
    pub fn unlock(&self, file: &str, lock: &str, now_ms: u64) -> Outcome {
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        match live(&mut held, file, now_ms) {
            Some(h) if h.lock == lock => {
                held.remove(file);
                Outcome::Ok
            }
            Some(h) => Outcome::Conflict(h.lock.clone()),
            None => Outcome::Conflict(String::new()),
        }
    }

    /// Whether a write that presents `lock` (or none) may go ahead. A held
    /// lock must match. With none held, a write that names a lock takes it,
    /// so a session whose lock ran out over a long pause can still save;
    /// a write that names none is let through only for an empty file, the
    /// MS-WOPI rule for files made outside the editor.
    pub fn may_write(&self, file: &str, lock: Option<&str>, size: u64, now_ms: u64) -> Outcome {
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        match (live(&mut held, file, now_ms), lock) {
            (Some(h), Some(l)) if h.lock == l => Outcome::Ok,
            (Some(h), _) => Outcome::Conflict(h.lock.clone()),
            (None, Some(l)) => {
                held.insert(file.into(), fresh(l, now_ms));
                Outcome::Ok
            }
            (None, None) if size == 0 => Outcome::Ok,
            (None, None) => Outcome::Conflict(String::new()),
        }
    }

    /// A file moved to a new id (a rename in a store whose ids follow names):
    /// its lock goes with it.
    pub fn moved(&self, from: &str, to: &str) {
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(h) = held.remove(from) {
            held.insert(to.into(), h);
        }
    }
}

fn fresh(lock: &str, now_ms: u64) -> Held {
    Held {
        lock: lock.into(),
        expires_ms: now_ms + LOCK_TTL_MS,
    }
}

/// The lock on `file` if it has not run out; one that has is dropped.
fn live<'a>(held: &'a mut HashMap<String, Held>, file: &str, now_ms: u64) -> Option<&'a mut Held> {
    if held.get(file).is_some_and(|h| h.expires_ms <= now_ms) {
        held.remove(file);
    }
    held.get_mut(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Outcome::*;

    #[test]
    fn one_lock_per_file_and_a_second_holder_sees_the_first() {
        let l = Locks::default();
        assert_eq!(l.lock("a", "L1", 0), Ok);
        assert_eq!(l.lock("a", "L1", 1), Ok, "taking it again refreshes it");
        assert_eq!(l.lock("a", "L2", 2), Conflict("L1".into()));
        assert_eq!(l.lock("b", "L2", 2), Ok, "locks are per file");
        assert_eq!(l.current("a", 3).as_deref(), Some("L1"));
    }

    #[test]
    fn a_lock_runs_out_unless_refreshed() {
        let l = Locks::default();
        l.lock("a", "L1", 0);
        assert_eq!(l.refresh("a", "L1", LOCK_TTL_MS - 1), Ok);
        assert_eq!(l.current("a", LOCK_TTL_MS + 1).as_deref(), Some("L1"));
        assert_eq!(l.current("a", 2 * LOCK_TTL_MS), None);
        assert_eq!(l.lock("a", "L2", 2 * LOCK_TTL_MS), Ok);
    }

    #[test]
    fn unlock_and_refresh_need_the_held_lock() {
        let l = Locks::default();
        assert_eq!(l.unlock("a", "L1", 0), Conflict(String::new()));
        assert_eq!(l.refresh("a", "L1", 0), Conflict(String::new()));
        l.lock("a", "L1", 0);
        assert_eq!(l.unlock("a", "L2", 0), Conflict("L1".into()));
        assert_eq!(l.refresh("a", "L2", 0), Conflict("L1".into()));
        assert_eq!(l.unlock("a", "L1", 0), Ok);
        assert_eq!(l.current("a", 0), None);
    }

    #[test]
    fn unlock_and_relock_swaps_only_from_the_old_lock() {
        let l = Locks::default();
        assert_eq!(
            l.unlock_and_relock("a", "L1", "L2", 0),
            Conflict(String::new())
        );
        l.lock("a", "L1", 0);
        assert_eq!(
            l.unlock_and_relock("a", "X", "L2", 0),
            Conflict("L1".into())
        );
        assert_eq!(l.unlock_and_relock("a", "L1", "L2", 0), Ok);
        assert_eq!(l.current("a", 0).as_deref(), Some("L2"));
    }

    #[test]
    fn writes_follow_the_lock() {
        let l = Locks::default();
        assert_eq!(
            l.may_write("a", None, 0, 0),
            Ok,
            "an empty file needs no lock"
        );
        assert_eq!(l.may_write("a", None, 10, 0), Conflict(String::new()));
        assert_eq!(
            l.may_write("a", Some("L1"), 10, 0),
            Ok,
            "an unheld lock is taken"
        );
        assert_eq!(l.current("a", 0).as_deref(), Some("L1"));
        assert_eq!(l.may_write("a", Some("L2"), 10, 0), Conflict("L1".into()));
        assert_eq!(l.may_write("a", None, 0, 0), Conflict("L1".into()));
        assert_eq!(l.may_write("a", Some("L1"), 10, 0), Ok);
    }

    #[test]
    fn a_moved_file_keeps_its_lock() {
        let l = Locks::default();
        l.lock("a", "L1", 0);
        l.moved("a", "b");
        assert_eq!(l.current("a", 0), None);
        assert_eq!(l.current("b", 0).as_deref(), Some("L1"));
    }
}
