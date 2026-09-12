//! Is this box busy doing its owner's work right now?
//!
//! The compute window says which hours an owner is *willing* to lend the
//! machine out. It cannot say whether they are using it: a window is a guess
//! about a routine, and routines break. Someone who set 23:00–07:00 and then
//! stays up editing photos has told the mesh their box is free while they are
//! sitting at it, and the first thing they notice is the box getting slower.
//!
//! So the window is permission and this is reality, and the mesh gets the
//! machine only when both agree. The two compose in one direction only: idle
//! can WITHDRAW availability inside the window, and can never grant it outside
//! one. An owner who set a window meant it.
//!
//! ## Why load average, and what it does not see
//!
//! `/proc/loadavg`'s one-minute figure divided by the CPU count, against a
//! threshold. That is a deliberately unclever measure, chosen because the
//! clever ones are wrong here:
//!
//!   * Per-process accounting would have to know which processes are "the
//!     owner's". On this box that is Nextcloud, Forgejo, Postgres, Redis and
//!     two Kubernetes instances, all of which run whether or not anybody is
//!     awake. There is no session to attribute work to; there is no session at
//!     all, because there are no logins.
//!   * Network activity would catch a phone syncing photos and miss a long
//!     video transcode, which is exactly backwards.
//!   * Anything sampling faster than the load average already does would make
//!     the signal flap, and every flap is a pod evicted and rescheduled
//!     somewhere else in the mesh.
//!
//! The one-minute average is also already smoothed, which matters more than
//! precision: this value is read at most once per heartbeat and a single noisy
//! sample would cost the mesh a migration.
//!
//! What it cannot see is a box that is busy in a way that does not load the
//! CPU — a large upload filling the disk, say. That is an honest gap rather
//! than a defect to paper over: the mesh workload it would be sharing with is
//! CPU work, so CPU pressure is the contention that matters.

use std::fs;

/// Default: the box is considered free below a quarter of a core's worth of
/// one-minute load per CPU.
///
/// Not zero, and not 1.0. A completely idle losos box does not sit at 0.00 —
/// two kubelets, containerd, Postgres, Redis and a PHP-FPM pool all tick over
/// — so a threshold near zero would mean "never idle" and the feature would
/// silently never fire. And 1.0 per core is fully committed, by which point
/// the owner is already waiting on their own machine.
pub const DEFAULT_LOAD_THRESHOLD: f64 = 0.25;

/// One-minute load average, as a fraction of one core.
///
/// Separated from the decision so the threshold comparison is testable without
/// a filesystem, which is the same split the rest of this crate uses.
pub fn parse_loadavg(contents: &str, cpus: usize) -> Option<f64> {
    let one_minute: f64 = contents.split_whitespace().next()?.parse().ok()?;
    if !one_minute.is_finite() || one_minute < 0.0 {
        return None;
    }
    let cpus = cpus.max(1) as f64;
    Some(one_minute / cpus)
}

/// Whether `load` (per core) counts as idle at `threshold`.
///
/// Strictly less than, so a threshold of 0 means "never idle" rather than
/// "idle when the load happens to read exactly zero". An operator who sets 0
/// is asking to withdraw the box from compute sharing, and should get that
/// rather than a coin flip.
pub fn is_idle(load: f64, threshold: f64) -> bool {
    load < threshold
}

/// Read the box's current per-core one-minute load.
///
/// `None` when /proc/loadavg cannot be read or parsed. Callers must treat that
/// as **busy**, never as idle: an unreadable load is an unknown state, and the
/// failure that costs the owner is lending the machine out while they are
/// using it, not declining to lend it out.
pub fn current_load() -> Option<f64> {
    let contents = fs::read_to_string("/proc/loadavg").ok()?;
    parse_loadavg(&contents, num_cpus())
}

/// CPU count, as the kernel reports it online.
///
/// `std::thread::available_parallelism` rather than a crate: it already
/// respects cgroup CPU limits, which matters because both Kubernetes
/// instances run under one.
fn num_cpus() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loadavg_is_divided_by_the_cpu_count() {
        // The kernel reports absolute load; a four-core box at 2.00 is half
        // committed, not "twice as busy as a threshold of 1".
        let l = parse_loadavg("2.00 1.50 1.20 2/512 1234", 4).unwrap();
        assert!((l - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn a_zero_cpu_count_does_not_divide_by_zero() {
        // available_parallelism should never return 0, but the arithmetic here
        // is the kind that produces inf and then compares as "idle".
        let l = parse_loadavg("1.00 1.00 1.00 1/1 1", 0).unwrap();
        assert!(l.is_finite());
        assert!((l - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn junk_is_none_rather_than_a_number() {
        assert!(parse_loadavg("", 1).is_none());
        assert!(parse_loadavg("not-a-number 1 1", 1).is_none());
        assert!(parse_loadavg("-1.0 1 1", 1).is_none());
        assert!(parse_loadavg("nan 1 1", 1).is_none());
        assert!(parse_loadavg("inf 1 1", 1).is_none());
    }

    #[test]
    fn the_threshold_is_exclusive_so_zero_means_never() {
        assert!(!is_idle(0.0, 0.0), "threshold 0 must mean never idle");
        assert!(is_idle(0.0, 0.01));
        assert!(is_idle(0.24, DEFAULT_LOAD_THRESHOLD));
        assert!(!is_idle(0.25, DEFAULT_LOAD_THRESHOLD));
        assert!(!is_idle(4.0, DEFAULT_LOAD_THRESHOLD));
    }
}
