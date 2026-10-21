//! `losos-ctl set-password`: the rules a new password must meet, and the
//! command against the fake.
//!
//! The password itself lives in the people file (`crate::people`), written by
//! the HTTP and D-Bus layers. The command validates both arguments before any
//! effect and keeps LosOS Git's account in step for the owner.

use losos_ctl::fake::FakeLosos;
use losos_ctl::losos::cmd_set_password;
use losos_ctl::setup::{validate_password, validate_user, MIN_PASSWORD_CHARS};

/// Distinctive enough that a substring search cannot match it by accident, and
/// long enough to clear the minimum.
const PASSWORD: &str = "Zqx-marmalade-77-parapet";

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
}

#[test]
fn a_good_password_reports_the_user_and_never_the_password() {
    let mut f = FakeLosos::new();
    let out = cmd_set_password(&mut f, "notshared", PASSWORD).unwrap();
    assert_eq!(out["user"], "notshared");
    assert_eq!(out["changed"], true);
    assert!(!out.to_string().contains(PASSWORD));
}

#[test]
fn the_minimum_is_counted_in_characters_not_bytes() {
    // 11 characters that are 22 bytes. Counting bytes would accept it and
    // report a floor the owner did not actually clear.
    let eleven = "Α1-αααααααα";
    assert_eq!(eleven.chars().count(), 11);
    assert!(validate_password(eleven).is_err());
    assert!(validate_password("Α1-ααααααααα").is_ok());
}

#[test]
fn the_four_composition_rules_each_name_what_is_missing() {
    // One password opens the admin pages and LosOS Cloud, so a new one wants
    // both cases, a digit and a symbol as well as the length. Each refusal
    // names the one rule it is about, because the wizard shows that sentence.
    let err = validate_password("zqx-marmalade-77-parapet").unwrap_err();
    assert!(err.contains("upper-case"), "unhelpful: {err}");
    let err = validate_password("ZQX-MARMALADE-77-PARAPET").unwrap_err();
    assert!(err.contains("lower-case"), "unhelpful: {err}");
    let err = validate_password("Zqx-marmalade-parapet").unwrap_err();
    assert!(err.contains("digit"), "unhelpful: {err}");
    let err = validate_password("Zqx marmalade 77 parapet").unwrap_err();
    assert!(err.contains("symbol"), "unhelpful: {err}");
    assert!(validate_password("Zqx-marmalade-77-parapet").is_ok());
    // Unicode letters count for their case: a Slovak or German keyboard
    // must not be told its capital letter is not one. And a symbol is any
    // non-alphanumeric, non-space character, whatever the keyboard.
    assert!(validate_password("žltý kôň 7 ÚPEL!").is_ok());
    assert!(validate_password("straße über 9 ÖL€").is_ok());
    assert!(validate_password("Zqx…marmalade 77 parapet").is_ok());
}

#[test]
fn a_password_containing_a_newline_is_refused() {
    // Not pedantry. The people file is YAML and a sign-in form is one line:
    // an embedded newline would be a password nobody can type back.
    let err = validate_password("Correct horse\nbattery-1").unwrap_err();
    assert!(err.contains("control character"), "unhelpful: {err}");
    assert!(validate_password("Tab\there-too-1").is_err());
    // A space is not a control character and must survive.
    assert!(validate_password("Correct horse-battery 1").is_ok());
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
    // A user name reaches LosOS Git's admin calls; one that reads as a flag
    // could change nobody's password and still look like success.
    let err = validate_user("--version").unwrap_err();
    assert!(err.contains("flag"), "unhelpful: {err}");
    assert!(validate_user("").is_err());
    assert!(validate_user("has space").is_err());
    assert!(validate_user("semi;colon").is_err());
    assert!(validate_user("notshared").is_ok());
    assert!(validate_user("admin.user_1@example").is_ok());
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
