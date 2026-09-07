//! The command core: a [`Losos`] effect trait plus the six commands written
//! once against it.
//!
//! Nothing here knows about paths, systemd, D-Bus or HTTP. That is the whole
//! point — `lososd` runs these functions against the real filesystem while the
//! test suite runs the *same* functions against [`crate::fake::FakeLosos`],
//! so the state machine is exercised with no filesystem and no subprocesses.
//!
//! The trait is synchronous. Every method is a small file read/write or a
//! process spawn, and keeping it sync means the fake stays trivially pure and
//! the command layer never acquires an async colour; the async transports call
//! in through `web::block` / `spawn_blocking`.

use crate::model::{Mode, Rebuild, RebuildState, State};
use crate::overrides::{parse_settings, DEFAULT_OVERRIDES_NIX};
use serde_json::{json, Value};

/// Anything the commands need from the outside world.
pub trait Losos {
    /// Current persisted state. Missing or corrupt input yields
    /// [`State::default`] rather than an error.
    fn load_state(&mut self) -> anyhow::Result<State>;
    fn save_state(&mut self, s: &State) -> anyhow::Result<()>;
    /// Patch `losos.sharingMyStorage` in `defaults.nix` (`LOSOS_CONFIG`).
    fn rewrite_config(&mut self, sharing: bool) -> anyhow::Result<()>;
    /// Replace `overrides.nix` (`LOSOS_OVERRIDES`) wholesale.
    fn write_overrides(&mut self, body: &str) -> anyhow::Result<()>;
    fn read_overrides(&mut self) -> anyhow::Result<String>;
    /// Start a rebuild for `job` and arrange for its completion to be recorded.
    fn spawn_rebuild(&mut self, job: &str) -> anyhow::Result<()>;
    /// Last non-empty line of the rebuild log, or `""`.
    fn rebuild_log_tail(&mut self) -> anyhow::Result<String>;
    fn next_job_id(&mut self) -> anyhow::Result<String>;
}

/// Message stamped on a rebuild the moment it is queued.
const MSG_STARTED: &str = "rebuild started";
/// The factory-reset variant, kept distinct so the UI can tell them apart.
const MSG_RESET_STARTED: &str = "factory reset: rebuild started";

/// A freshly queued rebuild record.
fn building(job: &str, message: &str) -> Rebuild {
    Rebuild {
        job: job.to_string(),
        state: RebuildState::Building,
        progress: 0,
        message: message.to_string(),
    }
}

/// Current mode and sharing flag.
///
/// Deliberately omits `rebuild`, even though the persisted state carries it —
/// `status` is the endpoint for that.
pub fn cmd_state<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    let s = l.load_state()?;
    Ok(json!({ "mode": s.mode.as_str(), "sharing": s.sharing }))
}

/// The user-tunable `losos.*` options, parsed out of `overrides.nix`.
pub fn cmd_settings<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    let content = l.read_overrides()?;
    Ok(parse_settings(&content).to_json())
}

/// Switch sharing posture and rebuild.
///
/// `sharing` is derived from the mode and is never passed in separately. Note
/// this writes `defaults.nix`, not `overrides.nix`.
pub fn cmd_change<L: Losos>(l: &mut L, mode: Mode) -> anyhow::Result<Value> {
    let sharing = mode == Mode::Mesh;
    l.rewrite_config(sharing)?;
    let job = l.next_job_id()?;
    let mut s = l.load_state()?;
    s.mode = mode;
    s.sharing = sharing;
    s.rebuild = Some(building(&job, MSG_STARTED));
    l.save_state(&s)?;
    l.spawn_rebuild(&job)?;
    Ok(json!({ "job": job }))
}

/// Overwrite `overrides.nix` with an already-validated body and rebuild.
///
/// Mode and sharing are left exactly as they were: applying settings is not a
/// posture change.
pub fn cmd_apply<L: Losos>(l: &mut L, nix_code: &str) -> anyhow::Result<Value> {
    l.write_overrides(nix_code)?;
    let job = l.next_job_id()?;
    let mut s = l.load_state()?;
    s.rebuild = Some(building(&job, MSG_STARTED));
    l.save_state(&s)?;
    l.spawn_rebuild(&job)?;
    Ok(json!({ "job": job }))
}

/// Soft factory reset: restore the committed defaults and rebuild.
///
/// Unlike the other write commands this does not load the existing state
/// first — it writes [`State::default`] wholesale, which is the reset. The
/// destructive tier (wipe the disks) is the installer ISO, not this.
pub fn cmd_factory_reset<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    l.write_overrides(DEFAULT_OVERRIDES_NIX)?;
    let job = l.next_job_id()?;
    let s = State {
        rebuild: Some(building(&job, MSG_RESET_STARTED)),
        ..State::default()
    };
    l.save_state(&s)?;
    l.spawn_rebuild(&job)?;
    Ok(json!({ "job": job, "reset": true }))
}

/// Rebuild progress. Polled by the admin UI roughly every two seconds.
///
/// While a rebuild is running the stored message is replaced by the live log
/// tail when there is one, which is what makes the UI's progress line move.
/// `progress` is never derived from the log — it stays 0 until the unit
/// reaches a terminal state.
/// `job` identifies which rebuild the document describes, so a client can tell
/// its own rebuild's outcome from a previous one's. Without it, a status poll
/// that lands before the daemon has recorded the new job reads the *previous*
/// job's terminal state and the UI declares "complete" on a rebuild that is
/// still starting. `cmd_apply` happens to persist `building` before it returns
/// the ack, which closes that window today — but that ordering is an
/// implementation detail of one call path, not a promise, and the admin UI
/// should not have to depend on it. Absent when idle: there is no job.
pub fn cmd_status<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    let s = l.load_state()?;
    let Some(rb) = s.rebuild else {
        return Ok(json!({ "state": "idle", "progress": 0, "message": "" }));
    };
    if rb.state == RebuildState::Building {
        let tail = l.rebuild_log_tail()?;
        let message = if tail.is_empty() { rb.message } else { tail };
        return Ok(json!({
            "state": RebuildState::Building.as_str(),
            "progress": rb.progress,
            "message": message,
            "job": rb.job,
        }));
    }
    Ok(json!({
        "state": rb.state.as_str(),
        "progress": rb.progress,
        "message": rb.message,
        "job": rb.job,
    }))
}

/// Map a finished unit's exit code to the state the UI should show.
///
/// On failure the last log line is appended, because "exit 1" alone tells the
/// user nothing about which Nix expression blew up.
pub fn unit_outcome(code: i32, log_tail: &str) -> (RebuildState, i64, String) {
    if code == 0 {
        return (RebuildState::Done, 100, "rebuild complete".to_string());
    }
    let mut msg = format!("rebuild failed (exit {code})");
    if !log_tail.is_empty() {
        msg.push_str(": ");
        msg.push_str(log_tail);
    }
    (RebuildState::Failed, 0, msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeLosos;

    /// Every assertion below mirrors a case from the Haskell `test/Spec.hs`,
    /// so the port is checked against the behaviour that shipped.

    #[test]
    fn change_mesh_flips_sharing_marks_building_and_spawns() {
        let mut f = FakeLosos::new();
        let out = cmd_change(&mut f, Mode::Mesh).unwrap();
        assert_eq!(out["job"], "job-0");
        let s = f.state.clone();
        assert_eq!(s.mode, Mode::Mesh);
        assert!(s.sharing);
        let rb = s.rebuild.unwrap();
        assert_eq!(rb.state, RebuildState::Building);
        assert_eq!(rb.progress, 0);
        assert!(f.spawned);
    }

    #[test]
    fn change_local_rewrites_the_config_line_to_false() {
        let mut f = FakeLosos::new();
        cmd_change(&mut f, Mode::Local).unwrap();
        assert!(f
            .config
            .iter()
            .any(|l| l.contains("losos.sharingMyStorage = false;")));
    }

    #[test]
    fn status_reports_idle_then_building() {
        let mut f = FakeLosos::new();
        assert_eq!(cmd_status(&mut f).unwrap()["state"], "idle");
        cmd_change(&mut f, Mode::Mesh).unwrap();
        assert_eq!(cmd_status(&mut f).unwrap()["state"], "building");
    }

    #[test]
    fn status_while_building_shows_the_live_log_tail() {
        let mut f = FakeLosos::new();
        f.log = vec!["copying path '/nix/store/abc-bash'".to_string()];
        cmd_change(&mut f, Mode::Mesh).unwrap();
        let out = cmd_status(&mut f).unwrap();
        assert_eq!(out["message"], "copying path '/nix/store/abc-bash'");
        // the log tail must not fabricate progress
        assert_eq!(out["progress"], 0);
    }

    #[test]
    fn status_carries_the_job_id_so_a_client_can_tell_rebuilds_apart() {
        let mut f = FakeLosos::new();
        // Idle: no job to report.
        assert!(cmd_status(&mut f).unwrap().get("job").is_none());

        let ack = cmd_change(&mut f, Mode::Mesh).unwrap();
        let job = ack["job"].as_str().expect("change acks with a job id");
        let building = cmd_status(&mut f).unwrap();
        assert_eq!(building["job"], job, "status must name the running job");

        // And after it settles, so a client polling late still learns which
        // rebuild the terminal state belongs to.
        let (state, progress, message) = unit_outcome(0, "");
        let mut s = f.load_state().unwrap();
        s.rebuild = Some(Rebuild {
            job: job.to_string(),
            state,
            progress,
            message,
        });
        f.save_state(&s).unwrap();
        assert_eq!(cmd_status(&mut f).unwrap()["job"], job);
    }

    #[test]
    fn status_falls_back_to_the_stored_message_with_an_empty_log() {
        let mut f = FakeLosos::new();
        cmd_change(&mut f, Mode::Mesh).unwrap();
        assert_eq!(cmd_status(&mut f).unwrap()["message"], MSG_STARTED);
    }

    #[test]
    fn unit_outcome_maps_success_and_failure() {
        assert_eq!(
            unit_outcome(0, ""),
            (RebuildState::Done, 100, "rebuild complete".to_string())
        );
        let (st, pct, msg) = unit_outcome(1, "error: evaluation failed");
        assert_eq!(st, RebuildState::Failed);
        assert_eq!(pct, 0);
        assert!(msg.contains("evaluation failed"));
        // no log tail => no trailing colon
        assert_eq!(unit_outcome(2, "").2, "rebuild failed (exit 2)");
    }

    #[test]
    fn settings_reports_the_committed_defaults() {
        let mut f = FakeLosos::new();
        let out = cmd_settings(&mut f).unwrap();
        assert_eq!(out["sharingMyStorage"], true);
        assert_eq!(out["nextcloudMode"], "container");
        assert_eq!(out["forgejoMode"], "container");
        assert_eq!(out["hostName"], "mattbox");
        assert_eq!(out["apachePort"], 11000);
        assert_eq!(out["proxyEnable"], false);
    }

    #[test]
    fn apply_rewrites_overrides_spawns_and_returns_a_job() {
        let mut f = FakeLosos::new();
        let code =
            "{ ... }:\n{\n  losos.sharingMyStorage = false;\n  losos.hostName = \"box2\";\n}\n";
        let out = cmd_apply(&mut f, code).unwrap();
        assert_eq!(out["job"], "job-0");
        assert!(f
            .config
            .iter()
            .any(|l| l.contains("losos.hostName = \"box2\";")));
        assert!(!f
            .config
            .iter()
            .any(|l| l.contains("losos.sharingMyStorage = true;")));
        assert!(f.spawned);
    }

    #[test]
    fn settings_reflects_an_applied_body() {
        let mut f = FakeLosos::new();
        let code = "{ ... }:\n{\n  losos.sharingMyStorage = false;\n  losos.hostName = \"box2\";\n  losos.nextcloud.mode = \"native\";\n  losos.nextcloud.apachePort = 12345;\n  losos.proxy.enable = true;\n}\n";
        cmd_apply(&mut f, code).unwrap();
        let out = cmd_settings(&mut f).unwrap();
        assert_eq!(out["hostName"], "box2");
        assert_eq!(out["nextcloudMode"], "native");
        assert_eq!(out["apachePort"], 12345);
        assert_eq!(out["proxyEnable"], true);
    }

    #[test]
    fn apply_then_settings_honours_the_legacy_aliases() {
        let mut f = FakeLosos::new();
        cmd_apply(&mut f, "{ ... }:\n{\n  losos.aio.apachePort = 12345;\n}\n").unwrap();
        assert_eq!(cmd_settings(&mut f).unwrap()["apachePort"], 12345);

        let mut g = FakeLosos::new();
        cmd_apply(
            &mut g,
            "{ ... }:\n{\n  losos.nextcloud.mode = \"aio\";\n}\n",
        )
        .unwrap();
        assert_eq!(cmd_settings(&mut g).unwrap()["nextcloudMode"], "container");
    }

    #[test]
    fn factory_reset_restores_defaults_resets_state_and_acks() {
        let mut f = FakeLosos::new();
        cmd_apply(
            &mut f,
            "{ ... }:\n{\n  losos.sharingMyStorage = false;\n  losos.hostName = \"box2\";\n}\n",
        )
        .unwrap();
        let out = cmd_factory_reset(&mut f).unwrap();

        assert_eq!(out["reset"], true);
        // the job counter keeps running across commands in one session
        assert_eq!(out["job"], "job-1");
        assert!(f
            .config
            .iter()
            .any(|l| l.contains("losos.hostName = \"mattbox\";")));
        assert!(!f.config.iter().any(|l| l.contains("box2")));
        assert_eq!(f.state.mode, Mode::Local);
        assert!(!f.state.sharing);
        assert!(f.spawned);
        assert_eq!(f.state.rebuild.as_ref().unwrap().message, MSG_RESET_STARTED);
    }

    #[test]
    fn state_response_never_carries_the_rebuild_record() {
        let mut f = FakeLosos::new();
        cmd_change(&mut f, Mode::Mesh).unwrap();
        let out = cmd_state(&mut f).unwrap();
        assert!(out.get("rebuild").is_none());
        assert_eq!(out["mode"], "mesh");
        assert_eq!(out["sharing"], true);
    }
}
