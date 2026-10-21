//! Credentials for one of the owner's computers (`crate::devices`).
//!
//! `losos cloud setup` and `losos git setup` ask the box for an app password
//! and an access token of the computer's own. These tests pin what that
//! rests on: the HTTP layer has judged the owner's password against the
//! people file (and throttled the address) before either command runs; a
//! wrong one is the typed refusal and makes nothing; LosOS Cloud mints the
//! app password over its socket for the owner it is told; LosOS Git gets the
//! same password before Forgejo is asked, a token of the same name is
//! replaced rather than refused, and a box without LosOS Git says so.

use losos_ctl::devices::{GitOff, GIT_SCOPES};
use losos_ctl::fake::FakeLosos;
use losos_ctl::losos::{cmd_device_cloud, cmd_device_git};
use losos_ctl::setup::NotReady;
use losos_ctl::signin::WrongPassword;

const PASSWORD: &str = "Zqx-marmalade-77-parapet";
const DELETE: &str = "DELETE /api/v1/users/notshared/tokens/LosOS%20on%20laptop";
const CREATE: &str = "POST /api/v1/users/notshared/tokens";

fn cloud_box() -> FakeLosos {
    let mut f = FakeLosos::new();
    f.state.claimed = true;
    f.cloud_answers.push_back((
        200,
        br#"{"ocs":{"meta":{"status":"ok"},"data":{"apppassword":"AbCd-EfGh-1234"}}}"#.to_vec(),
    ));
    f
}

fn git_box() -> FakeLosos {
    let mut f = FakeLosos::new().with_config_repo();
    f.state.claimed = true;
    f.owner_forgejo_routes.insert(
        DELETE.to_string(),
        (404, r#"{"message":"not found"}"#.to_string()),
    );
    f.owner_forgejo_routes.insert(
        CREATE.to_string(),
        (
            201,
            r#"{"id":3,"name":"LosOS on laptop","sha1":"9f8e7d6c5b4a","token_last_eight":"6c5b4a"}"#
                .to_string(),
        ),
    );
    f
}

#[test]
fn the_cloud_answer_has_everything_a_computer_needs_to_mount_the_files() {
    let mut f = cloud_box();
    let out = cmd_device_cloud(&mut f, true, "laptop").unwrap();
    assert_eq!(out["user"], "notshared");
    assert_eq!(out["appPassword"], "AbCd-EfGh-1234");
    assert_eq!(out["path"], "/remote.php/dav/files/notshared/");
    assert_eq!(f.cloud_calls, vec!["mint notshared LosOS on laptop"]);
}

#[test]
fn a_wrong_password_makes_no_cloud_credential() {
    let mut f = cloud_box();
    let err = cmd_device_cloud(&mut f, false, "laptop").unwrap_err();
    assert!(err.downcast_ref::<WrongPassword>().is_some(), "{err:#}");
    assert!(f.cloud_calls.is_empty());
}

#[test]
fn a_cloud_that_is_down_is_not_a_wrong_password() {
    let mut f = cloud_box();
    f.cloud_answers.clear();
    let err = cmd_device_cloud(&mut f, true, "laptop").unwrap_err();
    assert!(err.downcast_ref::<NotReady>().is_some(), "{err:#}");
}

#[test]
fn a_bad_device_name_is_refused_before_anything_is_asked() {
    let mut f = cloud_box();
    assert!(cmd_device_cloud(&mut f, true, "a/b").is_err());
    assert!(cmd_device_cloud(&mut f, true, "").is_err());
    assert!(f.cloud_calls.is_empty());
}

#[test]
fn the_git_token_is_made_as_the_owner_with_the_password() {
    let mut f = git_box();
    let out = cmd_device_git(&mut f, PASSWORD, true, "laptop").unwrap();
    assert_eq!(out["user"], "notshared");
    assert_eq!(out["token"], "9f8e7d6c5b4a");
    assert_eq!(out["path"], "/forgejo/");

    // LosOS Git was given the password first, so a password changed inside
    // LosOS Cloud is the one Forgejo checks.
    let synced = f
        .forgejo_calls
        .iter()
        .position(|(k, _)| k == "PATCH /api/v1/admin/users/notshared")
        .expect("the password was kept in step");
    assert_eq!(
        f.forgejo_calls[synced].1.as_deref(),
        Some(PASSWORD),
        "the bot set the owner's password"
    );

    let calls: Vec<&str> = f
        .owner_forgejo_calls
        .iter()
        .map(|(k, _, _)| k.as_str())
        .collect();
    assert_eq!(calls, vec![DELETE, CREATE], "the old name is freed first");
    for (_, password, _) in &f.owner_forgejo_calls {
        assert_eq!(password, PASSWORD);
    }
    let body: serde_json::Value =
        serde_json::from_str(f.owner_forgejo_calls[1].2.as_deref().unwrap()).unwrap();
    assert_eq!(body["name"], "LosOS on laptop");
    assert_eq!(body["scopes"], serde_json::json!(GIT_SCOPES));
}

#[test]
fn setting_up_the_same_computer_again_replaces_its_token() {
    let mut f = git_box();
    f.owner_forgejo_routes
        .insert(DELETE.to_string(), (204, String::new()));
    let out = cmd_device_git(&mut f, PASSWORD, true, "laptop").unwrap();
    assert_eq!(out["token"], "9f8e7d6c5b4a");
}

#[test]
fn a_wrong_password_never_reaches_losos_git() {
    let mut f = git_box();
    let err = cmd_device_git(&mut f, "not-the-password", false, "laptop").unwrap_err();
    assert!(err.downcast_ref::<WrongPassword>().is_some(), "{err:#}");
    assert!(f.owner_forgejo_calls.is_empty());
    assert!(
        !f.forgejo_calls.iter().any(|(k, _)| k.starts_with("PATCH")),
        "a wrong password was given to LosOS Git"
    );
}

#[test]
fn a_box_without_losos_git_says_so() {
    let mut f = cloud_box();
    let err = cmd_device_git(&mut f, PASSWORD, true, "laptop").unwrap_err();
    assert!(err.downcast_ref::<GitOff>().is_some(), "{err:#}");
}

#[test]
fn losos_git_that_is_not_up_is_not_now() {
    let mut f = git_box();
    f.owner_forgejo_routes.clear();
    let err = cmd_device_git(&mut f, PASSWORD, true, "laptop").unwrap_err();
    assert!(err.downcast_ref::<NotReady>().is_some(), "{err:#}");
}

#[test]
fn a_forgejo_out_of_step_is_not_now_rather_than_wrong() {
    let mut f = git_box();
    f.owner_forgejo_routes.insert(
        DELETE.to_string(),
        (401, r#"{"message":"unauthorized"}"#.to_string()),
    );
    let err = cmd_device_git(&mut f, PASSWORD, true, "laptop").unwrap_err();
    assert!(err.downcast_ref::<NotReady>().is_some(), "{err:#}");
}
