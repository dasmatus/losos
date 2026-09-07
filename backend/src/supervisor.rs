//! Rebuild supervision: spawn a transient unit, watch it, record the outcome.
//!
//! A rebuild runs as a `systemd-run` transient unit rather than a child
//! process, because `nixos-rebuild switch` restarts `lososd` itself during
//! activation. A child would die with the daemon mid-rebuild; a transient unit
//! survives, and [`start_supervisor`] re-attaches a watcher to it when the new
//! daemon process comes up.
//!
//! **Do not add `--collect`.** The unit's `ExecMainStatus` has to stay readable
//! after it exits, which is how the exit code is recovered. Dead units are
//! cleaned up by the reboot anyway — the appliance root is a tmpfs.

use crate::io_backend::{read_state, write_state, IoLosos, Paths};
use crate::losos::unit_outcome;
use crate::model::RebuildState;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::process::Command;
use std::thread::JoinHandle;
use std::time::Duration;

/// How long a log line may be before the UI gets an unreadable wall of text.
pub const LOG_LINE_CAP: usize = 240;
/// Bytes of the tail of the rebuild log to consider. The log grows to many
/// megabytes; only the last line is ever used.
pub const LOG_WINDOW: u64 = 4096;
/// Gap between unit polls.
const POLL_INTERVAL: Duration = Duration::from_secs(2);
/// How many consecutive unreadable polls to tolerate before declaring the unit
/// gone. At [`POLL_INTERVAL`] this is a 30-second grace window, which covers
/// the race where `systemd-run` has returned but the unit is not yet visible,
/// and equally a `systemctl` that has stopped executing at all.
pub const MAX_UNKNOWN_POLLS: u32 = 15;

/// The transient unit name for a job.
pub fn unit_name(job: &str) -> String {
    format!("losos-rebuild-{job}")
}

/// Last non-empty line of `lines`, trimmed and capped.
///
/// Pure, so both the real log reader and the test fake share it.
pub fn last_log_line(lines: &[String]) -> String {
    lines
        .iter()
        .map(|l| l.trim())
        .rfind(|l| !l.is_empty())
        .map(|l| l.chars().take(LOG_LINE_CAP).collect())
        .unwrap_or_default()
}

/// Last non-empty line of the rebuild log.
///
/// Only the final [`LOG_WINDOW`] bytes are read. A multi-byte character can be
/// sliced in half at the window boundary, but that only ever corrupts the
/// *first* line in the window, which is discarded — the last line is intact.
pub fn log_tail(path: &Path) -> String {
    let Ok(mut f) = std::fs::File::open(path) else {
        return String::new();
    };
    let Ok(meta) = f.metadata() else {
        return String::new();
    };
    let size = meta.len();
    if size > LOG_WINDOW && f.seek(SeekFrom::Start(size - LOG_WINDOW)).is_err() {
        return String::new();
    }
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() {
        return String::new();
    }
    let text = String::from_utf8_lossy(&buf);
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    last_log_line(&lines)
}

/// What one poll of the unit told us.
#[derive(Debug, PartialEq, Eq)]
pub enum Poll {
    /// Still running (or `systemctl` hiccuped) — keep waiting.
    Wait,
    /// Could not make sense of the unit's state.
    Unknown,
    /// Finished with this exit code.
    Done(i32),
}

/// Interpret `systemctl show --value -p ActiveState -p ExecMainStatus` output.
///
/// Split out from the subprocess call so it can be unit-tested.
pub fn classify_poll(active_state: &str, exec_main_status: &str) -> Poll {
    let code = exec_main_status.trim().parse::<i32>().ok();
    match active_state.trim() {
        "active" | "activating" | "deactivating" | "reloading" => Poll::Wait,
        // A failed unit always yields a verdict; assume 1 when the status is
        // unreadable, so a failure is never mistaken for success.
        "failed" => Poll::Done(code.unwrap_or(1)),
        "inactive" => match code {
            Some(c) => Poll::Done(c),
            None => Poll::Unknown,
        },
        _ => Poll::Unknown,
    }
}

/// Interpret one `systemctl show --value -p ActiveState -p ExecMainStatus`
/// invocation. `None` is an invocation that could not be run at all.
///
/// A `systemctl` that will not execute is [`Poll::Unknown`], not
/// [`Poll::Wait`]: `Wait` resets the grace counter in [`watch_unit`], so an
/// exec failure classed as a wait would hold a permanently broken box at
/// "building" for ever, never reaching [`MAX_UNKNOWN_POLLS`]. `Unknown` is
/// bounded, and ends in a recorded failure the operator can act on.
pub fn classify_show(stdout: Option<&str>) -> Poll {
    let Some(stdout) = stdout else {
        return Poll::Unknown;
    };
    let mut lines = stdout.lines();
    match (lines.next(), lines.next()) {
        (Some(active), Some(status)) => classify_poll(active, status),
        _ => Poll::Unknown,
    }
}

/// Ask `systemctl` what the unit is doing.
fn poll_unit(job: &str) -> Poll {
    let stdout = Command::new("systemctl")
        .args([
            "show",
            &unit_name(job),
            "--value",
            "-p",
            "ActiveState",
            "-p",
            "ExecMainStatus",
        ])
        .output()
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned());
    classify_show(stdout.as_deref())
}

/// Start the transient unit for `job`. `Err` carries the detail that goes into
/// the message the admin UI shows.
fn launch_unit(paths: &Paths, job: &str) -> Result<(), String> {
    let log_str = paths.rebuild_log().to_string_lossy().into_owned();
    let status = Command::new("systemd-run")
        .arg(format!("--unit={}", unit_name(job)))
        .arg(format!("--description=losos rebuild {job}"))
        .arg(format!("--property=StandardOutput=append:{log_str}"))
        .arg(format!("--property=StandardError=append:{log_str}"))
        .args(["nixos-rebuild", "switch", "--flake", &paths.flake_ref])
        .status();
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("systemd-run exited {s}")),
        Err(e) => Err(e.to_string()),
    }
}

/// Everything a rebuild needs from systemd, behind one seam.
///
/// [`Systemd`] is the shipped implementation and the only one the daemon ever
/// builds. Tests script this trait instead, which is what lets the watcher loop
/// be exercised whole — no subprocess, no systemd, and no waiting on a clock.
pub trait Units: Send + 'static {
    /// Launch the transient unit for `job`.
    fn launch(&mut self, paths: &Paths, job: &str) -> Result<(), String>;
    /// Ask what `job`'s unit is doing.
    fn poll(&mut self, job: &str) -> Poll;
    /// Sit out the gap between two polls.
    fn pause(&mut self);
}

/// The shipped [`Units`]: `systemd-run`, `systemctl show`, and a real sleep.
pub struct Systemd;

impl Units for Systemd {
    fn launch(&mut self, paths: &Paths, job: &str) -> Result<(), String> {
        launch_unit(paths, job)
    }

    fn poll(&mut self, job: &str) -> Poll {
        poll_unit(job)
    }

    fn pause(&mut self) {
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// Record a terminal outcome for `job`, if it is still the tracked job.
///
/// A completion event for a job that a newer `change`/`apply`/`factory-reset`
/// has already superseded is dropped: otherwise a slow watcher could overwrite
/// the state of the rebuild that replaced it.
///
/// **The caller must hold the state lock.** This is a read-modify-write of
/// `state.json`, and running it unlocked is how a finishing rebuild used to
/// clobber the record of the one queued a moment earlier.
fn finish_locked(paths: &Paths, job: &str, st: RebuildState, progress: i64, message: String) {
    let mut state = read_state(&paths.state_file());
    match state.rebuild.as_mut() {
        Some(rb) if rb.job == job => {
            rb.state = st;
            rb.progress = progress;
            rb.message = message;
        }
        _ => {
            tracing::debug!(
                job,
                "outcome dropped: a newer rebuild has replaced this one"
            );
            return;
        }
    }
    tracing::info!(job, state = st.as_str(), "rebuild outcome recorded");
    if let Err(e) = write_state(&paths.state_file(), &state) {
        tracing::warn!(job, error = %e, "could not record rebuild outcome");
    }
}

/// [`finish_locked`] for a watcher thread, which owns no lock yet.
fn finish(backend: &IoLosos, job: &str, st: RebuildState, progress: i64, message: String) {
    let _guard = backend.lock_state();
    finish_locked(&backend.paths, job, st, progress, message);
}

/// Watch a transient unit to completion and record the result. Blocking: call
/// it on its own thread.
pub fn watch_unit(backend: &IoLosos, job: &str, units: &mut dyn Units) {
    let mut unknowns = 0u32;
    loop {
        match units.poll(job) {
            Poll::Wait => {
                unknowns = 0;
                units.pause();
            }
            Poll::Unknown if unknowns < MAX_UNKNOWN_POLLS => {
                unknowns += 1;
                units.pause();
            }
            Poll::Unknown => {
                finish(
                    backend,
                    job,
                    RebuildState::Failed,
                    0,
                    "rebuild unit vanished (reboot or manual stop mid-rebuild) — system state unknown; apply again to retry".to_string(),
                );
                return;
            }
            Poll::Done(code) => {
                let tail = log_tail(&backend.paths.rebuild_log());
                let (st, progress, message) = unit_outcome(code, &tail);
                finish(backend, job, st, progress, message);
                return;
            }
        }
    }
}

/// Start a rebuild for `job` and watch it in the background.
///
/// If `systemd-run` cannot be started at all, the tracked rebuild is flipped
/// straight to failed — otherwise the UI would sit at "building" forever with
/// no watcher on the way.
pub fn spawn_rebuild(backend: &IoLosos, job: &str) -> anyhow::Result<()> {
    // The handle is dropped, which detaches the watcher: it outlives the
    // command that queued the rebuild by design.
    spawn_rebuild_with(backend, job, Systemd).map(|_| ())
}

/// [`spawn_rebuild`] over an explicit [`Units`].
///
/// Returns the watcher's handle, so a caller that wants the outcome can join it
/// rather than wait on a clock.
///
/// Runs inside the state lock in production, hence `finish_locked` on the
/// failure path: the lock is not reentrant.
pub fn spawn_rebuild_with<U: Units>(
    backend: &IoLosos,
    job: &str,
    mut units: U,
) -> anyhow::Result<JoinHandle<()>> {
    if let Err(detail) = units.launch(&backend.paths, job) {
        finish_locked(
            &backend.paths,
            job,
            RebuildState::Failed,
            0,
            format!("failed to start rebuild: {detail}"),
        );
        anyhow::bail!("failed to start rebuild: {detail}")
    }
    let backend = backend.clone();
    let job = job.to_string();
    Ok(std::thread::spawn(move || {
        watch_unit(&backend, &job, &mut units)
    }))
}

/// Re-attach a watcher to an in-flight rebuild at daemon startup.
///
/// Load-bearing: `nixos-rebuild switch` restarts `lososd` during activation,
/// killing the watcher thread that started the rebuild. Without this, a state
/// file that says `building` would stay that way forever and the admin UI
/// would spin indefinitely.
pub fn start_supervisor(backend: &IoLosos) {
    // Detached on purpose: the daemon's job here is to get a watcher running,
    // not to wait for the rebuild.
    let _ = start_supervisor_with(backend, Systemd);
}

/// [`start_supervisor`] over an explicit [`Units`].
///
/// `Some` carries the re-attached watcher's handle; `None` means the state file
/// recorded no rebuild in flight, so nothing was spawned.
pub fn start_supervisor_with<U: Units>(backend: &IoLosos, mut units: U) -> Option<JoinHandle<()>> {
    let rb = read_state(&backend.paths.state_file()).rebuild?;
    if rb.state != RebuildState::Building {
        return None;
    }
    tracing::info!(job = %rb.job, "re-attaching to in-flight rebuild");
    let backend = backend.clone();
    Some(std::thread::spawn(move || {
        watch_unit(&backend, &rb.job, &mut units)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_line_skips_blanks_and_trims() {
        let lines = vec![
            "first".to_string(),
            "  ".to_string(),
            "  last line  ".to_string(),
            String::new(),
        ];
        assert_eq!(last_log_line(&lines), "last line");
    }

    #[test]
    fn last_line_is_empty_for_an_empty_log() {
        assert_eq!(last_log_line(&[]), "");
        assert_eq!(last_log_line(&["".to_string(), "   ".to_string()]), "");
    }

    #[test]
    fn last_line_is_capped() {
        let long = "x".repeat(500);
        assert_eq!(last_log_line(&[long]).chars().count(), LOG_LINE_CAP);
    }

    #[test]
    fn running_states_mean_wait() {
        for s in ["active", "activating", "deactivating", "reloading"] {
            assert_eq!(classify_poll(s, "0"), Poll::Wait);
        }
    }

    #[test]
    fn failed_unit_defaults_to_exit_one_when_status_is_unreadable() {
        assert_eq!(classify_poll("failed", ""), Poll::Done(1));
        assert_eq!(classify_poll("failed", "3"), Poll::Done(3));
    }

    #[test]
    fn inactive_needs_a_readable_status() {
        assert_eq!(classify_poll("inactive", "0"), Poll::Done(0));
        assert_eq!(classify_poll("inactive", "junk"), Poll::Unknown);
    }

    #[test]
    fn unrecognised_state_is_unknown() {
        assert_eq!(classify_poll("", ""), Poll::Unknown);
        assert_eq!(classify_poll("something-else", "0"), Poll::Unknown);
    }

    #[test]
    fn unit_name_matches_the_shipped_contract() {
        assert_eq!(unit_name("20260821-1"), "losos-rebuild-20260821-1");
    }
}
