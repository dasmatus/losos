//! The first-run claim: one shot while the box is unclaimed, never after,
//! and a reply lost in transit can be asked for again.
//!
//! Nothing has to start before the first password is set any more (LosOS
//! Cloud believes the person nginx names, and the password lives in LosOS
//! Login's people file), so an unclaimed box is ready at once.

use losos_ctl::fake::FakeLosos;
use losos_ctl::losos::{cmd_claim, cmd_claim_state, Losos};
use losos_ctl::receipt::Receipts;
use losos_ctl::setup::AlreadyClaimed;

const PASSWORD: &str = "Zqx-marmalade-77-parapet";
const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// A password that is not [`PASSWORD`], drawn at run time rather than
/// written out.
fn another_password() -> String {
    use std::io::Read;
    let mut buf = Vec::new();
    std::fs::File::open("/dev/urandom")
        .and_then(|f| f.take(32).read_to_end(&mut buf))
        .expect("/dev/urandom");
    String::from_utf8_lossy(&buf).into_owned()
}

#[test]
fn an_unclaimed_box_is_ready_at_once() {
    let mut l = FakeLosos::default();
    let v = cmd_claim_state(&mut l).unwrap();
    assert_eq!(v["claimed"], false);
    assert_eq!(v["ready"], true);
    assert!(v["waitingFor"].is_null());
}

#[test]
fn a_claim_takes_the_box_and_hands_back_the_token() {
    let mut l = FakeLosos::default();
    let v = cmd_claim(
        &mut l,
        "notshared",
        PASSWORD,
        TOKEN,
        &mut Receipts::default(),
    )
    .unwrap();
    assert_eq!(v["claimed"], true);
    assert_eq!(v["token"], TOKEN);
    assert_eq!(v["user"], "notshared");
    assert!(l.load_state().unwrap().claimed);
    let v = cmd_claim_state(&mut l).unwrap();
    assert_eq!(v["claimed"], true);
}

#[test]
fn a_password_the_rules_refuse_leaves_the_box_claimable() {
    let mut l = FakeLosos::default();
    let err = cmd_claim(
        &mut l,
        "notshared",
        "short",
        TOKEN,
        &mut Receipts::default(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("at least"), "{err}");
    assert!(!l.load_state().unwrap().claimed);
}

// ── A reply lost in transit can be asked for again ───────────────────────────

#[test]
fn the_same_password_asked_again_gets_the_same_reply() {
    let mut l = FakeLosos::default();
    let mut receipts = Receipts::default();
    let first = cmd_claim(&mut l, "notshared", PASSWORD, TOKEN, &mut receipts).unwrap();

    let again = cmd_claim(&mut l, "notshared", PASSWORD, TOKEN, &mut receipts).unwrap();
    assert_eq!(again["claimed"], true);
    assert_eq!(again["token"], first["token"]);
    assert_eq!(again["user"], first["user"]);
    assert_eq!(
        again["replayed"], true,
        "the page can tell a replay from a first claim"
    );
}

#[test]
fn a_different_password_on_a_claimed_box_is_a_typed_conflict() {
    let mut l = FakeLosos::default();
    let mut receipts = Receipts::default();
    cmd_claim(&mut l, "notshared", PASSWORD, TOKEN, &mut receipts).unwrap();

    let err = cmd_claim(
        &mut l,
        "notshared",
        &another_password(),
        TOKEN,
        &mut receipts,
    )
    .unwrap_err();
    assert!(
        err.downcast_ref::<AlreadyClaimed>().is_some(),
        "the HTTP layer keys its 409 on this type: {err}"
    );
}

#[test]
fn a_claimed_box_with_no_receipt_in_memory_refuses_every_password() {
    // The daemon restarted since the claim (nixos-rebuild switch restarts
    // lososd), so the receipt is gone: the right password gets no key.
    let mut l = FakeLosos::default();
    cmd_claim(
        &mut l,
        "notshared",
        PASSWORD,
        TOKEN,
        &mut Receipts::default(),
    )
    .unwrap();
    let err = cmd_claim(
        &mut l,
        "notshared",
        PASSWORD,
        TOKEN,
        &mut Receipts::default(),
    )
    .unwrap_err();
    assert!(err.downcast_ref::<AlreadyClaimed>().is_some(), "{err}");
}
