//! The owner's password unlocks the admin pages, and Nextcloud is the judge.
//!
//! `POST /api/sign-in` (backend/src/signin.rs) exists so the wizard's
//! password is the one credential an owner needs: the admin key is a spare on
//! the printed sheet, no longer the only way back in. These tests pin what
//! that rests on — the password is checked by asking Nextcloud, never by a
//! copy lososd keeps; a wrong one is a typed 401 and an unreachable
//! Nextcloud a typed 503, never a bare error; the probe names the one admin
//! account and forwards the browser's address; and the request that carries
//! the password has it in the Basic header and nowhere else.

use losos_ctl::fake::FakeLosos;
use losos_ctl::losos::cmd_sign_in;
use losos_ctl::setup::{validate_password, NotReady, DEFAULT_ADMIN_USER};
use losos_ctl::signin::{
    base64, login_request, parse_login_url, validate_candidate, Throttled, WrongPassword,
    DEFAULT_LOGIN_URL,
};

const PASSWORD: &str = "Zqx-marmalade-77-parapet";
const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const CLIENT: &str = "192.168.122.1";

fn claimed_box() -> FakeLosos {
    let mut f = FakeLosos::new();
    f.state.claimed = true;
    f.accepted_password = Some(PASSWORD.to_string());
    f
}

#[test]
fn the_right_password_is_answered_with_the_admin_token() {
    let mut f = claimed_box();
    let out = cmd_sign_in(&mut f, PASSWORD, TOKEN, CLIENT).unwrap();
    assert_eq!(out["token"], TOKEN);
    assert_eq!(out["user"], DEFAULT_ADMIN_USER);
    // The question went to Nextcloud, about the one admin account, on behalf
    // of the browser that asked.
    assert_eq!(
        f.login_asked,
        vec![(DEFAULT_ADMIN_USER.to_string(), CLIENT.to_string())]
    );
}

#[test]
fn a_wrong_password_is_a_typed_refusal_and_no_token() {
    let mut f = claimed_box();
    let err = cmd_sign_in(&mut f, "Zqx-marmalade-78-parapet", TOKEN, CLIENT).unwrap_err();
    assert!(
        err.downcast_ref::<WrongPassword>().is_some(),
        "not the typed error: {err:#}"
    );
    assert!(
        !format!("{err:#}").contains(TOKEN),
        "the refusal leaked the token"
    );
}

#[test]
fn an_unreachable_nextcloud_is_not_now_rather_than_wrong() {
    // The pod is not up: the password cannot be checked, which is a 503 the
    // dialog turns into "use the spare key", not a 401 that reads as a typo.
    let mut f = claimed_box();
    f.accepted_password = None;
    let err = cmd_sign_in(&mut f, PASSWORD, TOKEN, CLIENT).unwrap_err();
    let not_ready = err
        .downcast_ref::<NotReady>()
        .unwrap_or_else(|| panic!("not the typed error: {err:#}"));
    assert!(
        not_ready.0.contains("LosOS cloud"),
        "unhelpful: {}",
        not_ready.0
    );
}

#[test]
fn nextclouds_own_throttle_is_reported_as_wait_not_wrong() {
    let mut f = claimed_box();
    f.login_throttled = true;
    let err = cmd_sign_in(&mut f, PASSWORD, TOKEN, CLIENT).unwrap_err();
    assert!(err.downcast_ref::<Throttled>().is_some(), "{err:#}");
}

#[test]
fn an_old_password_that_fails_todays_rules_still_signs_in() {
    // The rules for a *new* password grew; the one already set did not. An
    // owner must never be locked out by a rule added after they chose it.
    let old = "correct horse battery staple";
    assert!(
        validate_password(old).is_err(),
        "the premise: this fails today's rules"
    );
    assert!(validate_candidate(old).is_ok());
    let mut f = claimed_box();
    f.accepted_password = Some(old.to_string());
    assert_eq!(
        cmd_sign_in(&mut f, old, TOKEN, CLIENT).unwrap()["token"],
        TOKEN
    );
}

#[test]
fn a_malformed_candidate_never_reaches_nextcloud() {
    let mut f = claimed_box();
    for bad in ["", "line\nbreak"] {
        assert!(cmd_sign_in(&mut f, bad, TOKEN, CLIENT).is_err());
    }
    assert!(f.login_asked.is_empty(), "a malformed password was sent on");
}

#[test]
fn the_probe_carries_the_password_in_the_basic_header_and_nowhere_else() {
    let endpoint = parse_login_url(DEFAULT_LOGIN_URL).unwrap();
    let secret = validate_candidate(PASSWORD).unwrap();
    let bytes = login_request(&endpoint, "localhost", DEFAULT_ADMIN_USER, &secret, CLIENT);
    let text = String::from_utf8(bytes).unwrap();
    let expected = format!(
        "Authorization: Basic {}\r\n",
        base64(format!("{DEFAULT_ADMIN_USER}:{PASSWORD}").as_bytes())
    );
    assert!(text.contains(&expected), "no Basic header:\n{text}");
    assert!(
        !text.contains(PASSWORD),
        "the password is on the wire in clear:\n{text}"
    );
    assert!(text.starts_with("GET /nextcloud/ocs/v2.php/cloud/user HTTP/1.1\r\n"));
    assert!(text.contains("Host: localhost\r\n"));
    assert!(text.contains("OCS-APIRequest: true\r\n"));
    assert!(text.contains(&format!("X-Forwarded-For: {CLIENT}\r\n")));
    assert!(text.ends_with("Connection: close\r\n\r\n"));
}

#[test]
fn a_client_address_that_is_not_an_ip_is_not_forwarded() {
    // `X-Real-IP` is whatever nginx put there; if it is not an address it
    // must not become a header Nextcloud keys its throttle on.
    let endpoint = parse_login_url(DEFAULT_LOGIN_URL).unwrap();
    let secret = validate_candidate(PASSWORD).unwrap();
    let bytes = login_request(&endpoint, "local\r\nhost", "notshared", &secret, "unknown");
    let text = String::from_utf8(bytes).unwrap();
    assert!(!text.contains("X-Forwarded-For"), "{text}");
    // And a Host with a line break in it cannot start a header of its own.
    assert!(text.contains("Host: localhost\r\n"), "{text}");
}
