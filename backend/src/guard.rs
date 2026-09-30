//! Admin-API abuse controls: a per-address throttle on failed authentication
//! and an append-only audit log of admin-plane mutations.
//!
//! Both are keyed on the *remote address* and both are wired in at one place,
//! `http::guarded`, so no route can be added that has one without the other.
//! Time is passed in rather than read, so the throttle is tested without
//! sleeping.

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Failures a source may make before it is delayed. Generous enough that an
/// owner fumbling a pasted token is never slowed.
const FREE_FAILURES: u32 = 10;
/// Ceiling on the delay, so a typo storm is never a lockout measured in hours.
const MAX_DELAY: Duration = Duration::from_secs(300);
/// Entries kept. A LAN attacker can spoof addresses, so the map must not grow
/// without bound; past this, the oldest entries are dropped.
const MAX_TRACKED: usize = 1024;
/// A source quiet for this long is forgotten.
const FORGET_AFTER: Duration = Duration::from_secs(3600);

/// Where the audit log lives. A constant, deliberately not read from the
/// environment: `/var` is bind-mounted from `/persist`, so this survives the
/// nightly reboot, and no input can steer where the daemon appends.
pub const AUDIT_LOG: &str = "/var/lib/losos/audit.log";

struct Record {
    failures: u32,
    last_failure: Instant,
}

/// Exponential delay per remote address once [`FREE_FAILURES`] are spent.
#[derive(Default)]
pub struct Throttle {
    sources: Mutex<HashMap<String, Record>>,
}

impl Throttle {
    /// `Err(retry_after)` while `addr` is inside its penalty window.
    ///
    /// A blocked attempt is refused even with the right token — otherwise the
    /// throttle would only slow down guesses that were already wrong — and is
    /// not counted, so hammering cannot push the window out indefinitely.
    pub fn check(&self, addr: &str, now: Instant) -> Result<(), Duration> {
        let sources = self.sources.lock().unwrap_or_else(|e| e.into_inner());
        let Some(r) = sources.get(addr) else {
            return Ok(());
        };
        let until = r.last_failure + delay_for(r.failures);
        if now < until {
            Err(until - now)
        } else {
            Ok(())
        }
    }

    pub fn record_failure(&self, addr: &str, now: Instant) {
        let mut sources = self.sources.lock().unwrap_or_else(|e| e.into_inner());
        if sources.len() >= MAX_TRACKED && !sources.contains_key(addr) {
            sources.retain(|_, r| now.duration_since(r.last_failure) < FORGET_AFTER);
            if sources.len() >= MAX_TRACKED {
                if let Some(oldest) = sources
                    .iter()
                    .min_by_key(|(_, r)| r.last_failure)
                    .map(|(k, _)| k.clone())
                {
                    sources.remove(&oldest);
                }
            }
        }
        let r = sources.entry(addr.to_string()).or_insert(Record {
            failures: 0,
            last_failure: now,
        });
        r.failures = r.failures.saturating_add(1);
        r.last_failure = now;
    }

    /// A correct token clears the source's history: no permanent lockout.
    pub fn record_success(&self, addr: &str) {
        self.sources
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(addr);
    }
}

fn delay_for(failures: u32) -> Duration {
    if failures <= FREE_FAILURES {
        return Duration::ZERO;
    }
    let exp = (failures - FREE_FAILURES - 1).min(16);
    Duration::from_secs(1u64 << exp).min(MAX_DELAY)
}

/// Whole seconds for a `Retry-After` header, rounded up and at least 1.
pub fn retry_after_secs(d: Duration) -> u64 {
    (d.as_secs() + u64::from(d.subsec_nanos() > 0)).max(1)
}

/// Append-only JSON-lines log at [`AUDIT_LOG`]:
/// `{"ts","route","outcome","remote"}`.
///
/// The token, request bodies and passwords are never passed in, so they cannot
/// be logged. Mode 0600, `O_APPEND`. A write failure is reported to the
/// journal and does not fail the request: a full disk must not stop the owner
/// administering the box. It is not tamper-evident.
///
/// There is deliberately no way to point this at another path: the file name is
/// the constant above and nothing else reaches the code that opens it.
pub struct Audit;

/// One record as a JSON line, newline included. Pure, so it is unit-tested
/// without touching the filesystem.
pub fn audit_line(ts: &str, route: &str, outcome: &str, remote: &str) -> String {
    format!(
        "{}\n",
        serde_json::json!({
            "ts": ts,
            "route": route,
            "outcome": outcome,
            "remote": remote,
        })
    )
}

impl Audit {
    pub fn record(&self, route: &str, outcome: &str, remote: &str) {
        let ts = chrono::Utc::now().to_rfc3339();
        if let Err(e) = Self::append(&audit_line(&ts, route, outcome, remote)) {
            tracing::error!(error = ?e, path = AUDIT_LOG, "audit log write failed");
        }
    }

    fn append(line: &str) -> std::io::Result<()> {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::create_dir_all(Path::new(AUDIT_LOG).parent().unwrap_or(Path::new("/")))?;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(AUDIT_LOG)?
            .write_all(line.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "192.168.1.9";

    #[test]
    fn free_failures_are_not_delayed_then_delay_grows() {
        let t = Throttle::default();
        let now = Instant::now();
        for _ in 0..FREE_FAILURES {
            assert!(t.check(A, now).is_ok());
            t.record_failure(A, now);
        }
        // Sixth failure is the first to be penalised: 1s, then 2s.
        assert_eq!(t.check(A, now), Ok(()));
        t.record_failure(A, now);
        assert_eq!(t.check(A, now), Err(Duration::from_secs(1)));
        assert!(t.check(A, now + Duration::from_secs(1)).is_ok());
        t.record_failure(A, now + Duration::from_secs(1));
        assert_eq!(
            t.check(A, now + Duration::from_secs(1)),
            Err(Duration::from_secs(2))
        );
    }

    #[test]
    fn delay_is_capped_and_other_sources_are_unaffected() {
        let t = Throttle::default();
        let now = Instant::now();
        for _ in 0..40 {
            t.record_failure(A, now);
        }
        assert_eq!(t.check(A, now), Err(MAX_DELAY));
        assert!(t.check("192.168.1.10", now).is_ok());
    }

    #[test]
    fn success_clears_the_history() {
        let t = Throttle::default();
        let now = Instant::now();
        for _ in 0..=FREE_FAILURES {
            t.record_failure(A, now);
        }
        assert!(t.check(A, now).is_err());
        t.record_success(A);
        assert!(t.check(A, now).is_ok());
    }

    #[test]
    fn tracked_sources_are_bounded() {
        let t = Throttle::default();
        let now = Instant::now();
        for i in 0..MAX_TRACKED + 50 {
            t.record_failure(&format!("10.0.{}.{}", i / 256, i % 256), now);
        }
        assert!(t.sources.lock().unwrap().len() <= MAX_TRACKED);
    }

    #[test]
    fn retry_after_rounds_up() {
        assert_eq!(retry_after_secs(Duration::from_millis(200)), 1);
        assert_eq!(retry_after_secs(Duration::from_secs(3)), 3);
        assert_eq!(retry_after_secs(Duration::from_millis(2500)), 3);
    }

    #[test]
    fn audit_line_is_one_json_object_per_line_without_extras() {
        let line = audit_line(
            "2026-09-30T00:00:00+00:00",
            "/api/apply",
            "ok",
            "192.168.1.9",
        );
        assert!(line.ends_with('\n') && line.matches('\n').count() == 1);
        let v: serde_json::Value = serde_json::from_str(line.trim_end()).unwrap();
        assert_eq!(v["route"], "/api/apply");
        assert_eq!(v["outcome"], "ok");
        assert_eq!(v["remote"], "192.168.1.9");
        assert_eq!(v.as_object().unwrap().len(), 4);
    }
}
