//! `losos-ctl set-password` — the planner and the command, against the fake.
//!
//! The headline property is the first test: **no argv this command can produce
//! ever contains the password**. `/proc/<pid>/cmdline` is world-readable and
//! nothing in `modules/` sets `hidepid`, so an `occ user:resetpassword
//! --password …` would hand the appliance's Nextcloud admin credentials to
//! every process on the box, including the two data domains' own workloads.
//! The plan is data precisely so that property can be checked rather than
//! reasoned about.

use losos_ctl::fake::FakeLosos;
use losos_ctl::losos::cmd_set_password;
use losos_ctl::setup::{
    action_argv, interpret_occ, parse_container_ids, plan_set_password, validate_password,
    validate_user, NcMode, OccAction, OccOutcome, SecretChannel, Target, IMAGE_OCC_SETPASS,
    MIN_PASSWORD_CHARS, NATIVE_OCC, NEXTCLOUD_UID, STAGED_SECRET,
};

/// Distinctive enough that a substring search cannot match it by accident, and
/// long enough to clear the minimum.
const PASSWORD: &str = "zqx-marmalade-77-parapet";

fn container() -> Target {
    Target::Container {
        socket: "unix:///run/containerd/containerd.sock".to_string(),
        id: "c0ffee1234".to_string(),
    }
}

/// Every argv a plan renders, flattened.
fn rendered_argv(plan: &[OccAction]) -> Vec<String> {
    plan.iter()
        .filter_map(action_argv)
        .flat_map(|argv| argv.iter().cloned())
        .collect()
}

// ── The property this module exists for ─────────────────────────────────────

#[test]
fn the_password_never_reaches_an_argv_in_either_mode() {
    for target in [container(), Target::Native] {
        let plan = plan_set_password(&target, "notshared").unwrap();
        for word in rendered_argv(&plan) {
            assert!(
                !word.contains(PASSWORD),
                "{:?} mode put the password in an argv: {word:?}",
                target.mode()
            );
        }
    }
}

#[test]
fn the_password_is_not_a_field_of_the_plan_at_all() {
    // Stronger than the argv check, and the reason it holds: OccAction has no
    // password field, so serializing a plan into a debug log cannot leak one
    // either. If someone later adds `password` to an action, this fails.
    for target in [container(), Target::Native] {
        let plan = plan_set_password(&target, "notshared").unwrap();
        let encoded = serde_json::to_string(&plan).unwrap();
        assert!(
            !encoded.contains(PASSWORD),
            "a serialized plan carried the password: {encoded}"
        );
    }
}

#[test]
fn nothing_the_command_executes_carries_the_password_either() {
    // The same property one level down: not just the plan, but every action the
    // command actually ran, in both modes.
    for mode in [NcMode::Container, NcMode::Native] {
        let mut f = FakeLosos::new();
        f.nextcloud_mode = mode;
        cmd_set_password(&mut f, "notshared", PASSWORD).unwrap();

        assert!(!f.occ_ran.is_empty(), "{mode:?} mode ran nothing");
        for word in rendered_argv(&f.occ_ran) {
            assert!(
                !word.contains(PASSWORD),
                "{mode:?} mode executed an argv holding the password: {word:?}"
            );
        }
    }
}

// ── Server-side validation ──────────────────────────────────────────────────

#[test]
fn a_too_short_password_is_refused_before_anything_runs() {
    let mut f = FakeLosos::new();
    let err = cmd_set_password(&mut f, "notshared", "short")
        .unwrap_err()
        .to_string();

    assert!(
        err.contains(&MIN_PASSWORD_CHARS.to_string()),
        "the message must say what the minimum is: {err}"
    );
    // Refused *before* any effect: nothing exec'd, and no plaintext password
    // left staged in /var/lib/nextcloud for the next process to find.
    assert!(f.occ_ran.is_empty(), "a refused password still ran steps");
    assert_eq!(f.staged_secret, None);
}

#[test]
fn the_minimum_is_counted_in_characters_not_bytes() {
    // 11 characters that are 22 bytes. Counting bytes would accept it and
    // report a floor the owner did not actually clear.
    let eleven = "ααααααααααα";
    assert_eq!(eleven.chars().count(), 11);
    assert!(validate_password(eleven).is_err());
    assert!(validate_password("αααααααααααα").is_ok());
}

#[test]
fn a_password_containing_a_newline_is_refused() {
    // Not pedantry. In container mode the password is staged in a file that the
    // in-image wrapper reads with `$(cat …)`, so an embedded newline would be
    // silently truncated there: the owner would believe they set one password
    // and find a different one in force, locked out of the only admin account.
    let err = validate_password("correct horse\nbattery").unwrap_err();
    assert!(err.contains("control character"), "unhelpful: {err}");
    assert!(validate_password("tab\there-too").is_err());
    // A space is not a control character and must survive.
    assert!(validate_password("correct horse battery").is_ok());
}

#[test]
fn validation_errors_never_quote_the_password() {
    // These strings reach the journal, a D-Bus error name and the SPA.
    for bad in ["short", "has\na newline in it somewhere"] {
        let err = validate_password(bad).unwrap_err();
        assert!(!err.contains(bad), "the error echoed the password: {err}");
    }
}

#[test]
fn a_user_that_would_be_read_as_a_flag_is_refused() {
    // `occ user:resetpassword --version --password-from-env` parses, and would
    // exit 0 having changed nobody's password.
    let err = validate_user("--version").unwrap_err();
    assert!(err.contains("flag"), "unhelpful: {err}");
    assert!(validate_user("").is_err());
    assert!(validate_user("has space").is_err());
    assert!(validate_user("semi;colon").is_err());
    assert!(validate_user("notshared").is_ok());
    assert!(validate_user("admin.user_1@example").is_ok());
}

#[test]
fn a_bad_user_is_refused_by_the_planner_not_just_by_its_caller() {
    // The planner is the function that puts this value into an argv, so it
    // re-checks rather than trusting whoever called it.
    assert!(plan_set_password(&container(), "--version").is_err());
    assert!(plan_set_password(&Target::Native, "--version").is_err());
}

// ── The plans ───────────────────────────────────────────────────────────────

#[test]
fn the_container_plan_is_stage_then_exec_then_clear() {
    let plan = plan_set_password(&container(), "notshared").unwrap();
    assert_eq!(
        plan,
        vec![
            OccAction::StageSecret {
                path: STAGED_SECRET.to_string(),
                uid: NEXTCLOUD_UID,
            },
            OccAction::RunOcc {
                argv: vec![
                    "crictl".into(),
                    "--runtime-endpoint".into(),
                    "unix:///run/containerd/containerd.sock".into(),
                    "exec".into(),
                    "--sync".into(),
                    "c0ffee1234".into(),
                    IMAGE_OCC_SETPASS.into(),
                    "notshared".into(),
                ],
                secret: SecretChannel::File {
                    path: STAGED_SECRET.to_string()
                },
            },
            OccAction::ClearSecret {
                path: STAGED_SECRET.to_string()
            },
        ]
    );
}

#[test]
fn the_endpoint_flag_comes_before_the_subcommand() {
    // `-r` is --runtime-endpoint globally, --resolve-image-path after `ps` and
    // --transport after `exec`. Putting the endpoint after the subcommand does
    // not error; it silently talks to whichever socket crictl probes first,
    // which on this box is a real second containerd (the mesh rke2 agent's).
    let plan = plan_set_password(&container(), "notshared").unwrap();
    let argv = action_argv(&plan[1]).unwrap();
    let endpoint = argv.iter().position(|w| w == "--runtime-endpoint").unwrap();
    let exec = argv.iter().position(|w| w == "exec").unwrap();
    assert!(
        endpoint < exec,
        "endpoint flag after the subcommand: {argv:?}"
    );

    let resolve = losos_ctl::setup::resolve_argv("unix:///run/containerd/containerd.sock");
    let endpoint = resolve
        .iter()
        .position(|w| w == "--runtime-endpoint")
        .unwrap();
    let ps = resolve.iter().position(|w| w == "ps").unwrap();
    assert!(
        endpoint < ps,
        "endpoint flag after the subcommand: {resolve:?}"
    );
}

#[test]
fn the_native_plan_uses_the_environment_and_stages_nothing() {
    let plan = plan_set_password(&Target::Native, "notshared").unwrap();
    assert_eq!(
        plan,
        vec![OccAction::RunOcc {
            argv: vec![
                NATIVE_OCC.into(),
                "user:resetpassword".into(),
                "notshared".into(),
                "--password-from-env".into(),
            ],
            secret: SecretChannel::Env,
        }]
    );
    // No staged file to leak or to forget to clean up: /proc/<pid>/environ is
    // mode 0400 owner-only, so the environment is a channel container mode
    // would use too if `crictl exec` had a flag for it.
    assert!(!plan
        .iter()
        .any(|a| matches!(a, OccAction::StageSecret { .. })));
}

#[test]
fn occ_is_always_told_to_read_the_password_from_the_environment() {
    // Without --password-from-env, occ takes the password interactively and a
    // daemon has no terminal. With --password it would take it from an argv.
    for target in [container(), Target::Native] {
        let plan = plan_set_password(&target, "notshared").unwrap();
        let words = rendered_argv(&plan);
        assert!(
            !words.iter().any(|w| w == "--password"),
            "{:?} mode reached for the argv form",
            target.mode()
        );
    }
    let native = plan_set_password(&Target::Native, "notshared").unwrap();
    assert!(rendered_argv(&native)
        .iter()
        .any(|w| w == "--password-from-env"));
}

// ── Locating the container ──────────────────────────────────────────────────

#[test]
fn zero_or_two_matching_containers_is_an_error_not_a_guess() {
    assert_eq!(parse_container_ids("  abc123 \n").unwrap(), "abc123");

    let none = parse_container_ids("\n  \n").unwrap_err();
    // The common cause is a box in its first few minutes, and the message has
    // to say so — there is no shell to go and look from.
    assert!(none.contains("maintenance:install"), "unhelpful: {none}");

    let many = parse_container_ids("abc\ndef\n").unwrap_err();
    assert!(many.contains("refusing to guess"), "unhelpful: {many}");
}

#[test]
fn a_pod_that_is_not_running_fails_without_staging_anything() {
    let mut f = FakeLosos::new();
    f.nextcloud_container = None;
    let err = cmd_set_password(&mut f, "notshared", PASSWORD)
        .unwrap_err()
        .to_string();
    assert!(err.contains("no running container"), "unhelpful: {err}");
    assert!(f.occ_ran.is_empty());
    assert_eq!(f.staged_secret, None);
}

// ── The executor ────────────────────────────────────────────────────────────

#[test]
fn the_success_path_reports_the_user_the_mode_that_ran_and_occs_message() {
    let mut f = FakeLosos::new();
    f.occ_stdout = "Successfully reset password for notshared".to_string();

    let out = cmd_set_password(&mut f, "notshared", PASSWORD).unwrap();

    assert_eq!(out["user"], "notshared");
    assert_eq!(out["mode"], "container");
    assert_eq!(out["changed"], true);
    assert_eq!(out["message"], "Successfully reset password for notshared");
    // The reply must not carry the password back in any field.
    assert!(!out.to_string().contains(PASSWORD));
}

#[test]
fn the_reported_mode_is_the_one_that_actually_ran() {
    let mut f = FakeLosos::new();
    f.nextcloud_mode = NcMode::Native;
    let out = cmd_set_password(&mut f, "notshared", PASSWORD).unwrap();
    assert_eq!(out["mode"], "native");
}

#[test]
fn the_secret_travels_by_file_in_container_mode_and_is_gone_afterwards() {
    let mut f = FakeLosos::new();
    cmd_set_password(&mut f, "notshared", PASSWORD).unwrap();

    // It did go through the staged file — otherwise the assertion below that it
    // is cleared would pass trivially.
    assert!(f
        .occ_ran
        .iter()
        .any(|a| matches!(a, OccAction::StageSecret { .. })));
    assert_eq!(
        f.staged_secret, None,
        "the staged plaintext password outlived the command"
    );
}

#[test]
fn the_staged_secret_is_cleared_even_when_the_exec_cannot_run() {
    // Modelling a missing crictl or an unreachable CRI socket: the failure
    // lands between staging and clearing, which is exactly when a plaintext
    // password would be left behind in /var/lib/nextcloud.
    let mut f = FakeLosos::new();
    f.occ_spawn_fails = true;

    let err = cmd_set_password(&mut f, "notshared", PASSWORD).unwrap_err();

    assert!(err.to_string().contains("No such file"));
    assert_eq!(
        f.staged_secret, None,
        "a failed set-password left the password staged on disk"
    );
    assert!(
        f.occ_ran
            .iter()
            .any(|a| matches!(a, OccAction::ClearSecret { .. })),
        "the cleanup step never ran: {:?}",
        f.occ_ran
    );
}

#[test]
fn a_non_zero_occ_exit_is_an_error_not_a_cheerful_report() {
    let mut f = FakeLosos::new();
    f.occ_exit = 1;
    f.occ_stdout = String::new();
    f.occ_stderr = "User does not exist".to_string();

    let err = cmd_set_password(&mut f, "notshared", PASSWORD)
        .unwrap_err()
        .to_string();

    assert!(err.contains("exit 1"), "no exit status in: {err}");
    assert!(err.contains("User does not exist"), "no detail in: {err}");
    assert!(!err.contains(PASSWORD));
    // And it still cleaned up.
    assert_eq!(f.staged_secret, None);
}

// ── Reading occ's output ────────────────────────────────────────────────────

#[test]
fn a_pod_that_is_running_but_not_installed_yet_says_so() {
    // `crictl ps --state Running` goes true long before Nextcloud is usable:
    // the entrypoint's `occ maintenance:install` takes minutes on first boot.
    // Reported as a generic failure this reads as a broken appliance.
    let err = interpret_occ(&OccOutcome {
        code: 1,
        stdout: "Nextcloud is not installed - only a limited number of commands are available"
            .to_string(),
        stderr: String::new(),
    })
    .unwrap_err();
    assert!(err.contains("not installed yet"), "unhelpful: {err}");
    assert!(err.contains("try again"), "no way forward in: {err}");
}

#[test]
fn an_empty_password_environment_blames_the_staging_channel() {
    // ResetPassword.php prints this when neither NC_PASS nor OC_PASS arrived.
    // In container mode that means the staged file never reached the pod, which
    // is a mount problem, not an empty password — the daemon refuses those.
    let err = interpret_occ(&OccOutcome {
        code: 1,
        stdout: "--password-from-env given, but NC_PASS/OC_PASS is empty!".to_string(),
        stderr: String::new(),
    })
    .unwrap_err();
    assert!(err.contains(STAGED_SECRET), "unhelpful: {err}");
}

#[test]
fn a_silent_success_still_produces_a_message() {
    let msg = interpret_occ(&OccOutcome {
        code: 0,
        stdout: String::new(),
        stderr: String::new(),
    })
    .unwrap();
    assert_eq!(msg, "password updated");
}

#[test]
fn a_killed_occ_is_reported_as_a_failure() {
    // A signalled child has no exit code; io_backend reports -1, which occ
    // cannot return, so it must not be mistaken for success.
    let err = interpret_occ(&OccOutcome {
        code: -1,
        stdout: String::new(),
        stderr: String::new(),
    })
    .unwrap_err();
    assert!(err.contains("exit -1"), "unhelpful: {err}");
}

// ── The mode itself ─────────────────────────────────────────────────────────

#[test]
fn the_mode_parse_is_exact_with_no_lenient_fallback() {
    // Unlike model::Mode, a wrong guess here does not degrade gracefully: it
    // runs crictl on a box with no local cluster, or nextcloud-occ on a box
    // whose Nextcloud only exists inside an image.
    assert_eq!(NcMode::parse("container"), Some(NcMode::Container));
    assert_eq!(NcMode::parse("native"), Some(NcMode::Native));
    assert_eq!(NcMode::parse("Container"), None);
    assert_eq!(NcMode::parse("aio"), None);
    assert_eq!(NcMode::parse(""), None);
    assert_eq!(NcMode::Native.as_str(), "native");
}

#[test]
fn a_secret_does_not_print_itself() {
    // crate::http::run logs failures as `error = ?e`, and dbus::reply renders
    // the whole context chain into the error message.
    let secret = validate_password(PASSWORD).unwrap();
    let shown = format!("{secret:?}");
    assert!(
        !shown.contains(PASSWORD),
        "Debug leaked the password: {shown}"
    );
}
