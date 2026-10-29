//! Credentials for one of the owner's computers (`crate::devices`).
//!
//! `losos cloud setup` and `losos git setup` ask the box for an app password
//! and an access token of the computer's own. These tests pin what that
//! rests on: the HTTP layer has judged the owner's password against the
//! people file (and throttled the address) before either command runs; a
//! wrong one is the typed refusal and makes nothing; LosOS Cloud mints the
//! app password over its socket for the owner it is told; LosOS Git's token
//! is made for the owner's account and replaces one of the same name, and a
//! box without LosOS Git, or with one still starting, says so.

use losos_ctl::devices::{GitOff, GIT_SCOPES};
use losos_ctl::fake::FakeLosos;
use losos_ctl::losos::{cmd_device_backups, cmd_device_cloud, cmd_device_git};
use losos_ctl::people::{Person, OWNER};
use losos_ctl::setup::NotReady;
use losos_ctl::signin::WrongPassword;

const CREATE: &str = "POST /api/v1/admin/users/notshared/tokens";

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
fn the_git_token_is_made_for_the_owners_account() {
    let mut f = git_box();
    let out = cmd_device_git(&mut f, true, "laptop").unwrap();
    assert_eq!(out["user"], "notshared");
    assert_eq!(out["token"], "9f8e7d6c5b4a");
    assert_eq!(out["path"], "/git/");
    assert_eq!(
        f.git_calls,
        vec![
            "PUT /api/v1/admin/users/notshared".to_string(),
            CREATE.to_string()
        ],
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
    assert!(cmd_device_git(&mut f, true, "a/b").is_err());
    assert!(f.git_calls.is_empty());
}

#[test]
fn a_wrong_password_never_reaches_losos_git() {
    let mut f = git_box();
    let err = cmd_device_git(&mut f, false, "laptop").unwrap_err();
    assert!(err.downcast_ref::<WrongPassword>().is_some(), "{err:#}");
    assert!(f.git_calls.is_empty());
}

#[test]
fn a_box_without_losos_git_says_so() {
    let mut f = cloud_box();
    let err = cmd_device_git(&mut f, true, "laptop").unwrap_err();
    assert!(err.downcast_ref::<GitOff>().is_some(), "{err:#}");
}

#[test]
fn losos_git_that_is_not_up_is_not_now() {
    let mut f = git_box();
    f.git_routes.clear();
    let err = cmd_device_git(&mut f, true, "laptop").unwrap_err();
    assert!(err.downcast_ref::<NotReady>().is_some(), "{err:#}");
}

#[test]
fn a_refused_token_is_not_now_rather_than_wrong() {
    let mut f = git_box();
    f.git_routes.insert(
        CREATE.to_string(),
        (
            500,
            r#"{"message":"LosOS Git could not answer."}"#.to_string(),
        ),
    );
    let err = cmd_device_git(&mut f, true, "laptop").unwrap_err();
    assert!(err.downcast_ref::<NotReady>().is_some(), "{err:#}");
}

fn person(name: &str, groups: &[&str]) -> Person {
    Person {
        name: name.to_string(),
        display_name: String::new(),
        groups: groups.iter().map(|g| g.to_string()).collect(),
    }
}

#[test]
fn the_owners_backups_get_both_credentials_of_the_owners_accounts() {
    let mut f = git_box();
    f.cloud_answers = cloud_box().cloud_answers;
    let out = cmd_device_backups(&mut f, &person("matus", &[OWNER]), "laptop").unwrap();
    assert_eq!(out["cloud"]["user"], "notshared");
    assert_eq!(out["cloud"]["appPassword"], "AbCd-EfGh-1234");
    assert_eq!(out["git"]["user"], "notshared");
    assert_eq!(out["git"]["token"], "9f8e7d6c5b4a");
    assert!(out.get("problems").is_none(), "{out}");
    // The owner's cloud account is always there: no arrival.
    assert_eq!(f.cloud_calls, vec!["mint notshared LosOS on laptop"]);
    assert!(
        f.git_calls.contains(&CREATE.to_string()),
        "{:?}",
        f.git_calls
    );
}

#[test]
fn someone_elses_backups_get_their_own_accounts() {
    let mut f = git_box();
    f.cloud_answers.push_back((200, b"{}".to_vec()));
    f.cloud_answers.extend(cloud_box().cloud_answers);
    f.git_routes.insert(
        "PUT /api/v1/admin/users/ada".to_string(),
        (200, r#"{"login":"ada"}"#.to_string()),
    );
    f.git_routes.insert(
        "POST /api/v1/admin/users/ada/tokens".to_string(),
        (
            201,
            r#"{"id":4,"name":"LosOS on laptop","token":"ada-token"}"#.to_string(),
        ),
    );
    let out = cmd_device_backups(&mut f, &person("ada", &[]), "laptop").unwrap();
    assert_eq!(out["cloud"]["user"], "ada");
    assert_eq!(out["git"]["user"], "ada");
    assert_eq!(out["git"]["token"], "ada-token");
    assert_eq!(
        f.cloud_calls,
        vec!["arrive ada", "mint ada LosOS on laptop"]
    );
    assert!(!f.git_calls.contains(&CREATE.to_string()));
}

#[test]
fn backups_on_a_box_without_losos_git_get_the_cloud_alone() {
    let mut f = cloud_box();
    let out = cmd_device_backups(&mut f, &person("matus", &[OWNER]), "laptop").unwrap();
    assert_eq!(out["cloud"]["user"], "notshared");
    assert!(out["git"].is_null());
    assert_eq!(out["gitOff"], true);
}

#[test]
fn backups_with_neither_app_up_are_not_now() {
    let mut f = FakeLosos::new().with_config_repo();
    f.state.claimed = true;
    let err = cmd_device_backups(&mut f, &person("matus", &[OWNER]), "laptop").unwrap_err();
    assert!(err.downcast_ref::<NotReady>().is_some(), "{err:#}");
    let err = cmd_device_backups(&mut f, &person("matus", &[OWNER]), "../x").unwrap_err();
    assert!(err.downcast_ref::<NotReady>().is_none(), "{err:#}");
}
