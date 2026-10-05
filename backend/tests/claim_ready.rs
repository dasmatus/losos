//! The first-run claim waits for Nextcloud instead of failing into a 500.
//!
//! `crictl ps --state Running` is true from the first line of the pod's
//! entrypoint, and that entrypoint runs `occ maintenance:install` before it
//! serves anything — minutes on a mini-PC, most of an hour under emulation.
//! The recorded install demo caught what that meant for the owner: a correct
//! password typed into step 2 of the wizard answered "command failed; see the
//! lososd journal", on a box with no shell to read a journal from. These
//! tests pin the replacement: `GET /api/setup/claim` says whether the box is
//! ready and why not, and `POST` refuses with a typed, retryable error while
//! it is not — running no `occ`, staging no secret, changing nothing.

use losos_ctl::fake::FakeLosos;
use losos_ctl::losos::{cmd_claim, cmd_claim_state, nextcloud_readiness, Losos};
use losos_ctl::setup::{
    interpret_status, plan_status, NcMode, NotReady, OccOutcome, Readiness, Target,
    IMAGE_OCC_STATUS, NATIVE_OCC,
};

const PASSWORD: &str = "zqx-marmalade-77-parapet";
const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn installed() -> OccOutcome {
    OccOutcome {
        code: 0,
        stdout: r#"{"installed":true,"version":"34.0.2.1","versionstring":"34.0.2","edition":"","maintenance":false,"needsDbUpgrade":false,"productname":"Nextcloud","extendedSupport":false}"#.to_string(),
        stderr: String::new(),
    }
}

// ── The probe's argv ─────────────────────────────────────────────────────────

#[test]
fn the_container_probe_execs_the_status_wrapper_in_the_located_container() {
    let target = Target::Container {
        socket: "unix:///run/containerd/containerd.sock".to_string(),
        id: "c0ffee1234".to_string(),
    };
    let argv = plan_status(&target);
    assert_eq!(
        argv,
        vec![
            "crictl",
            "--runtime-endpoint",
            "unix:///run/containerd/containerd.sock",
            "exec",
            "--sync",
            "c0ffee1234",
            IMAGE_OCC_STATUS,
        ]
    );
    // Same flag-order trap as set-password: the endpoint is a global flag and
    // means something else after the subcommand.
    assert!(
        argv.iter().position(|a| a == "--runtime-endpoint") < argv.iter().position(|a| a == "exec")
    );
}

#[test]
fn the_native_probe_asks_the_host_wrapper_for_json() {
    assert_eq!(
        plan_status(&Target::Native),
        vec![NATIVE_OCC, "status", "--output=json"]
    );
}

// ── Reading the answer ───────────────────────────────────────────────────────

#[test]
fn an_installed_serving_instance_is_ready() {
    assert_eq!(interpret_status(&installed()), Ok(()));
}

#[test]
fn installed_false_is_the_first_boot_window() {
    let out = OccOutcome {
        code: 0,
        stdout: r#"{"installed":false,"version":"34.0.2.1","versionstring":"34.0.2","edition":"","maintenance":false,"needsDbUpgrade":false,"productname":"Nextcloud","extendedSupport":false}"#.to_string(),
        stderr: String::new(),
    };
    let why = interpret_status(&out).unwrap_err();
    assert!(why.contains("still installing"), "{why}");
}

#[test]
fn a_not_installed_exit_is_the_same_window() {
    // What occ says when config.php is still being written by the installer.
    let out = OccOutcome {
        code: 1,
        stdout: String::new(),
        stderr: "Nextcloud is not installed - only a limited number of commands are available"
            .to_string(),
    };
    let why = interpret_status(&out).unwrap_err();
    assert!(why.contains("still installing"), "{why}");
}

#[test]
fn maintenance_and_db_upgrade_are_named_not_lumped_in() {
    let mk = |maint: bool, upg: bool| OccOutcome {
        code: 0,
        stdout: format!(r#"{{"installed":true,"maintenance":{maint},"needsDbUpgrade":{upg}}}"#),
        stderr: String::new(),
    };
    assert!(interpret_status(&mk(true, false))
        .unwrap_err()
        .contains("maintenance"));
    assert!(interpret_status(&mk(false, true))
        .unwrap_err()
        .contains("upgrading"));
}

#[test]
fn framing_before_the_json_is_tolerated_but_no_json_is_not() {
    let noisy = OccOutcome {
        code: 0,
        stdout: format!(
            "PHP Deprecated: something in a bundled app\n{}",
            installed().stdout
        ),
        stderr: String::new(),
    };
    assert_eq!(interpret_status(&noisy), Ok(()));
    let garbage = OccOutcome {
        code: 0,
        stdout: "Nextcloud or so".to_string(),
        stderr: String::new(),
    };
    assert!(interpret_status(&garbage).unwrap_err().contains("JSON"));
}

// ── The command against the fake ─────────────────────────────────────────────

#[test]
fn a_pod_that_is_not_running_reads_as_not_yet_without_probing() {
    let mut l = FakeLosos {
        nextcloud_container: None,
        ..FakeLosos::default()
    };
    let Readiness::NotYet(why) = nextcloud_readiness(&mut l) else {
        panic!("a missing pod is not ready");
    };
    assert!(why.contains("not started yet"), "{why}");
    assert!(l.status_probed.is_empty(), "nothing to exec into yet");
}

#[test]
fn a_probe_that_cannot_run_reads_as_not_yet_rather_than_as_an_error() {
    let mut l = FakeLosos {
        status_spawn_fails: true,
        ..FakeLosos::default()
    };
    assert!(matches!(nextcloud_readiness(&mut l), Readiness::NotYet(_)));
}

#[test]
fn claim_state_carries_ready_and_the_reason_while_unclaimed() {
    let mut l = FakeLosos::default();
    let v = cmd_claim_state(&mut l).unwrap();
    assert_eq!(v["claimed"], false);
    assert_eq!(v["ready"], true);
    assert!(v["waitingFor"].is_null());

    l.status_stdout = r#"{"installed":false}"#.to_string();
    let v = cmd_claim_state(&mut l).unwrap();
    assert_eq!(v["ready"], false);
    assert!(v["waitingFor"]
        .as_str()
        .unwrap()
        .contains("still installing"));
}

#[test]
fn a_claimed_box_is_not_probed_on_every_anonymous_request() {
    let mut l = FakeLosos {
        status_stdout: r#"{"installed":false}"#.to_string(),
        ..FakeLosos::default()
    };
    cmd_claim(&mut l, "notshared", PASSWORD, TOKEN).unwrap_err();
    l.status_stdout = installed().stdout;
    cmd_claim(&mut l, "notshared", PASSWORD, TOKEN).unwrap();
    let probes_before = l.status_probed.len();
    let v = cmd_claim_state(&mut l).unwrap();
    assert_eq!(v["claimed"], true);
    assert_eq!(v["ready"], true);
    assert_eq!(
        l.status_probed.len(),
        probes_before,
        "claimed: nothing to wait for"
    );
}

#[test]
fn a_claim_before_nextcloud_is_ready_is_refused_typed_and_changes_nothing() {
    let mut l = FakeLosos {
        status_stdout: r#"{"installed":false}"#.to_string(),
        ..FakeLosos::default()
    };
    let err = cmd_claim(&mut l, "notshared", PASSWORD, TOKEN).unwrap_err();
    let not_ready = err
        .downcast_ref::<NotReady>()
        .expect("the HTTP layer keys its 503 on this type");
    assert!(not_ready.0.contains("still installing"), "{not_ready}");
    assert!(
        l.occ_ran.is_empty(),
        "no occ step may run before the probe says ready"
    );
    assert!(
        l.staged_secret.is_none(),
        "no secret may be staged for a claim that was refused"
    );
    assert!(!l.load_state().unwrap().claimed, "the box stays claimable");
}

#[test]
fn the_same_claim_succeeds_once_the_probe_says_ready() {
    let mut l = FakeLosos {
        nextcloud_mode: NcMode::Container,
        ..FakeLosos::default()
    };
    let v = cmd_claim(&mut l, "notshared", PASSWORD, TOKEN).unwrap();
    assert_eq!(v["claimed"], true);
    assert_eq!(v["token"], TOKEN);
    assert_eq!(
        l.status_probed.len(),
        1,
        "probed exactly once, before the plan ran"
    );
    assert!(!l.occ_ran.is_empty());
    assert!(l.load_state().unwrap().claimed);
}
