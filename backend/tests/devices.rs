//! Credentials for one of the owner's computers (`crate::devices`).
//!
//! `losos cloud setup` and `losos git setup` ask the box for an app password
//! and an access token of the computer's own. These tests pin what that
//! rests on: the cloud's password is judged by Nextcloud and a wrong one is
//! the typed refusal that makes nothing; LosOS Git's token is made for the
//! owner's account (the HTTP layer checked the password against the people
//! file), replaces one of the same name, and a box without LosOS Git, or
//! with one still starting, says so.

use losos_ctl::devices::{GitOff, GIT_SCOPES};
use losos_ctl::fake::FakeLosos;
use losos_ctl::losos::{cmd_device_cloud, cmd_device_git};
use losos_ctl::setup::NotReady;
use losos_ctl::signin::{Throttled, WrongPassword};

const PASSWORD: &str = "Zqx-marmalade-77-parapet";
const CLIENT: &str = "192.168.122.1";
const CREATE: &str = "POST /api/v1/admin/users/notshared/tokens";

fn cloud_box() -> FakeLosos {
    let mut f = FakeLosos::new();
    f.state.claimed = true;
    f.accepted_password = Some(PASSWORD.to_string());
    f.app_password_answer = Some((
        200,
        r#"{"ocs":{"meta":{"status":"ok"},"data":{"apppassword":"AbCd-EfGh-1234"}}}"#.to_string(),
    ));
    f
}

fn git_box() -> FakeLosos {
    let mut f = FakeLosos::new().with_config_repo();
    f.state.claimed = true;
    f.accepted_password = Some(PASSWORD.to_string());
    f.git_routes.insert(
        CREATE.to_string(),
        (
            201,
            r#"{"id":3,"name":"LosOS on laptop","token":"9f8e7d6c5b4a","last_eight":"6c5b4a"}"#
                .to_string(),
        ),
    );
    f
}

#[test]
fn the_cloud_answer_has_everything_a_computer_needs_to_mount_the_files() {
    let mut f = cloud_box();
    let out = cmd_device_cloud(&mut f, PASSWORD, "laptop", CLIENT).unwrap();
    assert_eq!(out["user"], "notshared");
    assert_eq!(out["appPassword"], "AbCd-EfGh-1234");
    assert_eq!(out["path"], "/nextcloud/remote.php/dav/files/notshared/");
    assert_eq!(
        f.app_password_asked,
        vec![(
            "notshared".to_string(),
            "laptop".to_string(),
            CLIENT.to_string()
        )]
    );
}

#[test]
fn a_wrong_password_makes_no_cloud_credential() {
    let mut f = cloud_box();
    let err = cmd_device_cloud(&mut f, "not-the-password", "laptop", CLIENT).unwrap_err();
    assert!(err.downcast_ref::<WrongPassword>().is_some(), "{err:#}");
}

#[test]
fn a_cloud_that_is_down_or_throttling_is_not_a_wrong_password() {
    let mut f = cloud_box();
    f.app_password_answer = None;
    let err = cmd_device_cloud(&mut f, PASSWORD, "laptop", CLIENT).unwrap_err();
    assert!(err.downcast_ref::<NotReady>().is_some(), "{err:#}");

    let mut f = cloud_box();
    f.login_throttled = true;
    let err = cmd_device_cloud(&mut f, PASSWORD, "laptop", CLIENT).unwrap_err();
    assert!(err.downcast_ref::<Throttled>().is_some(), "{err:#}");
}

#[test]
fn a_bad_device_name_is_refused_before_anything_is_asked() {
    let mut f = cloud_box();
    assert!(cmd_device_cloud(&mut f, PASSWORD, "a/b", CLIENT).is_err());
    assert!(cmd_device_cloud(&mut f, PASSWORD, "", CLIENT).is_err());
    assert!(f.app_password_asked.is_empty());
}

#[test]
fn the_git_token_is_made_for_the_owners_account() {
    let mut f = git_box();
    let out = cmd_device_git(&mut f, "laptop").unwrap();
    assert_eq!(out["user"], "notshared");
    assert_eq!(out["token"], "9f8e7d6c5b4a");
    assert_eq!(out["path"], "/git/");
    assert_eq!(
        f.git_calls,
        vec!["PUT /api/v1/admin/users/notshared".to_string(), CREATE.to_string()],
        "the account is made sure of first"
    );
    let op = losos_ctl::config_repo::GitOp::CreateToken {
        login: "notshared".into(),
        name: "LosOS on laptop".into(),
    };
    let body: serde_json::Value = serde_json::from_str(&op.body().unwrap()).unwrap();
    assert_eq!(body["scopes"], serde_json::json!(GIT_SCOPES));
    assert_eq!(body["replace"], true, "a second setup replaces the token");
}

#[test]
fn a_bad_device_name_never_reaches_losos_git() {
    let mut f = git_box();
    assert!(cmd_device_git(&mut f, "a/b").is_err());
    assert!(f.git_calls.is_empty());
}

#[test]
fn a_box_without_losos_git_says_so() {
    let mut f = cloud_box();
    let err = cmd_device_git(&mut f, "laptop").unwrap_err();
    assert!(err.downcast_ref::<GitOff>().is_some(), "{err:#}");
}

#[test]
fn losos_git_that_is_not_up_is_not_now() {
    let mut f = git_box();
    f.git_routes.clear();
    let err = cmd_device_git(&mut f, "laptop").unwrap_err();
    assert!(err.downcast_ref::<NotReady>().is_some(), "{err:#}");
}

#[test]
fn a_refused_token_is_not_now_rather_than_wrong() {
    let mut f = git_box();
    f.git_routes.insert(
        CREATE.to_string(),
        (500, r#"{"message":"LosOS Git could not answer."}"#.to_string()),
    );
    let err = cmd_device_git(&mut f, "laptop").unwrap_err();
    assert!(err.downcast_ref::<NotReady>().is_some(), "{err:#}");
}
