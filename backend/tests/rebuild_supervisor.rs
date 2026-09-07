//! What happens to a rebuild nobody is watching any more.
//!
//! `nixos-rebuild switch` restarts `lososd` during activation, so the watcher
//! thread that started a rebuild dies partway through it. The re-attach in
//! `start_supervisor` is the only thing that stops the state file saying
//! `building` for ever, and the job comparison in `finish` is the only thing
//! that stops a stale watcher stamping its outcome on the rebuild that replaced
//! it. Both are one line, and neither used to be covered.
//!
//! Nothing here sleeps or waits on a clock: the watcher's view of systemd is a
//! scripted [`Units`], and a watcher that asks for one verdict more than the
//! script holds panics rather than spinning — a hang would burn CI's whole cap
//! with nothing to show for it.

use assert_fs::TempDir;
use losos_ctl::io_backend::{read_state, write_state, IoLosos, Paths};
use losos_ctl::model::{Mode, Rebuild, RebuildState, State};
use losos_ctl::supervisor::{
    classify_show, spawn_rebuild_with, start_supervisor_with, watch_unit, Poll, Units,
    MAX_UNKNOWN_POLLS,
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

/// A job id shaped like the real ones (`%Y%m%d%H%M%S-<pid>-<counter>`).
const JOB: &str = "20260907120000-4242-0";
/// What `cmd_change` stamps on a rebuild the moment it is queued.
const QUEUED: &str = "rebuild started";

/// What the scripted systemd was asked to do, and what it answered.
#[derive(Default)]
struct Tape {
    /// Verdicts still to hand back, oldest first.
    polls: VecDeque<Poll>,
    /// The job each `poll` asked about, in order.
    polled: Vec<String>,
    /// The job each `launch` was asked to start.
    launched: Vec<String>,
    /// When set, `launch` refuses with this detail instead of starting a unit.
    launch_error: Option<String>,
    /// Inter-poll pauses taken. No wall-clock time passes for any of them.
    pauses: u32,
}

/// A scripted stand-in for systemd, shared between the test and the watcher
/// thread that consumes it.
#[derive(Clone, Default)]
struct Script {
    tape: Arc<Mutex<Tape>>,
}

impl Script {
    /// A systemd that starts anything and then answers with `polls` in order.
    fn polling(polls: impl IntoIterator<Item = Poll>) -> Self {
        let script = Script::default();
        script.tape().polls = polls.into_iter().collect();
        script
    }

    /// A systemd that cannot start a unit at all.
    fn refusing_to_launch(detail: &str) -> Self {
        let script = Script::default();
        script.tape().launch_error = Some(detail.to_string());
        script
    }

    /// The tape, recovering from the poison a panicking watcher leaves behind
    /// so the assertions still get to run and say why.
    fn tape(&self) -> MutexGuard<'_, Tape> {
        self.tape.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl Units for Script {
    fn launch(&mut self, _paths: &Paths, job: &str) -> Result<(), String> {
        let mut tape = self.tape();
        tape.launched.push(job.to_string());
        match &tape.launch_error {
            Some(detail) => Err(detail.clone()),
            None => Ok(()),
        }
    }

    fn poll(&mut self, job: &str) -> Poll {
        let mut tape = self.tape();
        tape.polled.push(job.to_string());
        // Running off the end is a watcher that should have stopped, not a
        // verdict to invent. The panic surfaces at the join.
        tape.polls
            .pop_front()
            .unwrap_or_else(|| panic!("the watcher polled {job} past the end of the script"))
    }

    fn pause(&mut self) {
        self.tape().pauses += 1;
    }
}

/// An appliance whose every path lives inside `dir`.
fn appliance(dir: &TempDir) -> IoLosos {
    IoLosos::new(Paths {
        state_dir: dir.path().join("state"),
        config_file: dir.path().join("defaults.nix"),
        overrides_file: dir.path().join("overrides.nix"),
        flake_ref: "/etc/nixos#install".to_string(),
    })
}

/// Put `job` on record in `state.json`, exactly as a queued rebuild leaves it.
fn record(backend: &IoLosos, job: &str, state: RebuildState) {
    write_state(
        &backend.paths.state_file(),
        &State {
            mode: Mode::Local,
            sharing: false,
            rebuild: Some(Rebuild {
                job: job.to_string(),
                state,
                progress: 0,
                message: QUEUED.to_string(),
            }),
        },
    )
    .unwrap();
}

/// The rebuild `state.json` currently tracks.
fn tracked(backend: &IoLosos) -> Rebuild {
    read_state(&backend.paths.state_file())
        .rebuild
        .expect("the state file lost its rebuild record")
}

#[test]
fn a_restarted_daemon_reattaches_to_the_rebuild_in_flight() {
    let dir = TempDir::new().unwrap();
    let backend = appliance(&dir);
    record(&backend, JOB, RebuildState::Building);

    // A fresh daemon process comes up with the rebuild still running.
    let script = Script::polling([Poll::Wait, Poll::Done(0)]);
    let watcher = start_supervisor_with(&backend, script.clone())
        .expect("a rebuild recorded as building must get a fresh watcher");
    watcher.join().unwrap();

    // It attached to the job the state file named, not to some other unit.
    assert_eq!(script.tape().polled, [JOB, JOB]);

    let rb = tracked(&backend);
    assert_eq!(rb.job, JOB);
    assert_eq!(
        rb.state,
        RebuildState::Done,
        "without the re-attach the admin UI spins on 'building' for ever"
    );
    assert_eq!(rb.progress, 100);
    assert_eq!(rb.message, "rebuild complete");
}

#[test]
fn a_settled_rebuild_is_not_reattached_to() {
    // Nothing is in flight in any of these, so starting a watcher would poll a
    // unit that is long gone and overwrite a finished record.
    for state in [RebuildState::Idle, RebuildState::Done, RebuildState::Failed] {
        let dir = TempDir::new().unwrap();
        let backend = appliance(&dir);
        record(&backend, JOB, state);

        let script = Script::polling([Poll::Done(0)]);
        assert!(
            start_supervisor_with(&backend, script.clone()).is_none(),
            "{state:?} spawned a watcher"
        );
        assert!(script.tape().polled.is_empty(), "{state:?} was polled");
        assert_eq!(tracked(&backend).state, state);
        assert_eq!(tracked(&backend).message, QUEUED);
    }
}

#[test]
fn a_fresh_appliance_has_nothing_to_reattach_to() {
    let dir = TempDir::new().unwrap();
    // No state file at all: first boot, or the file was never written.
    let backend = appliance(&dir);

    let script = Script::polling([Poll::Done(0)]);
    assert!(start_supervisor_with(&backend, script.clone()).is_none());
    assert!(script.tape().polled.is_empty());
}

#[test]
fn a_reattached_watcher_reports_why_the_rebuild_failed() {
    let dir = TempDir::new().unwrap();
    let backend = appliance(&dir);
    record(&backend, JOB, RebuildState::Building);
    std::fs::write(
        backend.paths.rebuild_log(),
        "building '/nix/store/aaaa-etc.drv'\nerror: attribute 'foo' missing\n",
    )
    .unwrap();

    start_supervisor_with(&backend, Script::polling([Poll::Done(1)]))
        .unwrap()
        .join()
        .unwrap();

    let rb = tracked(&backend);
    assert_eq!(rb.state, RebuildState::Failed);
    assert_eq!(rb.progress, 0);
    // "exit 1" on its own tells nobody which expression blew up.
    assert!(rb.message.contains("exit 1"), "{}", rb.message);
    assert!(
        rb.message.contains("error: attribute 'foo' missing"),
        "{}",
        rb.message
    );
}

#[test]
fn a_superseded_watcher_cannot_overwrite_the_rebuild_that_replaced_it() {
    let dir = TempDir::new().unwrap();
    let backend = appliance(&dir);
    // An apply came in after job-a was queued, so job-b is what is on record.
    record(&backend, "job-b", RebuildState::Building);

    // job-a's watcher only now notices its unit finished. It is stale.
    watch_unit(&backend, "job-a", &mut Script::polling([Poll::Done(0)]));

    let rb = tracked(&backend);
    assert_eq!(rb.job, "job-b");
    assert_eq!(
        rb.state,
        RebuildState::Building,
        "a stale watcher marked the rebuild that replaced it complete"
    );
    assert_eq!(rb.progress, 0);
    assert_eq!(rb.message, QUEUED);
}

#[test]
fn the_watcher_of_the_tracked_job_does_record_its_outcome() {
    // The control for the test above: same setup, matching job, and the
    // outcome lands. Without this, dropping the record entirely would pass.
    let dir = TempDir::new().unwrap();
    let backend = appliance(&dir);
    record(&backend, "job-b", RebuildState::Building);

    watch_unit(&backend, "job-b", &mut Script::polling([Poll::Done(0)]));

    let rb = tracked(&backend);
    assert_eq!(rb.job, "job-b");
    assert_eq!(rb.state, RebuildState::Done);
    assert_eq!(rb.progress, 100);
}

#[test]
fn an_unreadable_unit_is_declared_lost_once_the_grace_window_runs_out() {
    let dir = TempDir::new().unwrap();
    let backend = appliance(&dir);
    record(&backend, JOB, RebuildState::Building);

    // One verdict more than the window tolerates, and not one to spare: the
    // script panics if the watcher asks again, so this pins the bound as well
    // as the outcome.
    let script = Script::polling((0..=MAX_UNKNOWN_POLLS).map(|_| Poll::Unknown));
    watch_unit(&backend, JOB, &mut script.clone());

    assert!(
        script.tape().polls.is_empty(),
        "the watcher gave up before the grace window ran out"
    );
    assert_eq!(script.tape().pauses, MAX_UNKNOWN_POLLS);

    let rb = tracked(&backend);
    assert_eq!(rb.state, RebuildState::Failed);
    assert!(rb.message.contains("vanished"), "{}", rb.message);
    assert!(rb.message.contains("apply again"), "{}", rb.message);
}

#[test]
fn one_readable_poll_resets_the_grace_window() {
    // This is why an exec failure must not be classed as a wait: a single
    // `Wait` wipes the counter, so a `systemctl` that never runs again would
    // never reach the end of the window and the rebuild would stay "building".
    let dir = TempDir::new().unwrap();
    let backend = appliance(&dir);
    record(&backend, JOB, RebuildState::Building);

    let script = Script::polling(
        (0..MAX_UNKNOWN_POLLS)
            .map(|_| Poll::Unknown)
            .chain(std::iter::once(Poll::Wait))
            .chain((0..=MAX_UNKNOWN_POLLS).map(|_| Poll::Unknown)),
    );
    watch_unit(&backend, JOB, &mut script.clone());

    assert!(
        script.tape().polls.is_empty(),
        "the watcher gave up without the counter having been reset"
    );
    assert_eq!(tracked(&backend).state, RebuildState::Failed);
}

#[test]
fn a_systemctl_that_will_not_run_is_unknown_rather_than_a_wait() {
    // `None` is "the command could not be executed at all". Classed as `Wait`
    // it resets the grace window on every poll, so a permanently broken
    // `systemctl` holds the rebuild at "building" until the box reboots.
    assert_eq!(classify_show(None), Poll::Unknown);
}

#[test]
fn the_shapes_systemctl_actually_prints() {
    // `systemctl show <unit> --value -p ActiveState -p ExecMainStatus`, as
    // observed against a running systemd.
    assert_eq!(classify_show(Some("active\n0\n")), Poll::Wait);
    assert_eq!(classify_show(Some("failed\n1\n")), Poll::Done(1));
    assert_eq!(classify_show(Some("failed\n100\n")), Poll::Done(100));
    // A transient unit that exits 0 is garbage-collected immediately, and
    // `systemctl show` answers for a unit it has never heard of exactly as it
    // does for one that finished cleanly. Both really are a rebuild that ended
    // in 0 here, which is why this is `Done` and not `Unknown` — and why the
    // module refuses `--collect`, which would erase the failures too.
    assert_eq!(classify_show(Some("inactive\n0\n")), Poll::Done(0));
    // The property list cut short, or no output at all.
    assert_eq!(classify_show(Some("active\n")), Poll::Unknown);
    assert_eq!(classify_show(Some("")), Poll::Unknown);
}

#[test]
fn a_rebuild_that_will_not_start_is_recorded_failed_at_once() {
    let dir = TempDir::new().unwrap();
    let backend = appliance(&dir);
    record(&backend, JOB, RebuildState::Building);

    let script = Script::refusing_to_launch("No such file or directory (os error 2)");
    let err = spawn_rebuild_with(&backend, JOB, script.clone()).unwrap_err();

    assert!(err.to_string().contains("failed to start rebuild"), "{err}");
    assert_eq!(script.tape().launched, [JOB]);
    assert!(
        script.tape().polled.is_empty(),
        "no unit was started, so there is nothing to watch"
    );

    let rb = tracked(&backend);
    assert_eq!(
        rb.state,
        RebuildState::Failed,
        "the UI would sit at 'building' with no watcher on the way"
    );
    assert!(rb.message.contains("No such file"), "{}", rb.message);
}

#[test]
fn a_started_rebuild_gets_a_watcher_on_its_own_unit() {
    let dir = TempDir::new().unwrap();
    let backend = appliance(&dir);
    record(&backend, JOB, RebuildState::Building);

    let script = Script::polling([Poll::Wait, Poll::Wait, Poll::Done(0)]);
    spawn_rebuild_with(&backend, JOB, script.clone())
        .unwrap()
        .join()
        .unwrap();

    assert_eq!(script.tape().launched, [JOB]);
    assert_eq!(script.tape().polled, [JOB, JOB, JOB]);
    assert_eq!(tracked(&backend).state, RebuildState::Done);
}
