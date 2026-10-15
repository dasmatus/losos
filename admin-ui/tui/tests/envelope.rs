//! The client's half of the envelope, without a server: what it seals is
//! what the box opens, and it takes an answer only when the pinned host key
//! signed it for this very request.

use losos::envelope::Session;
use losos::http::{Http, Trust};
use losos::vault::{HostKind, HostRecord};
use losos_access::{open, seal, HostKey, Keyring, Refusal, Replay, UserKey};
use serde_json::json;

const NONCE: &str = "00112233445566778899aabbccddeeff";

struct Pair {
    host: HostKey,
    user: HostKey,
    session: Session,
}

fn pair() -> Pair {
    let host = HostKey::generate("box").unwrap();
    let user = HostKey::generate("Owner <owner@example.org>").unwrap();
    let record = HostRecord {
        url: "http://box.test".into(),
        kind: HostKind::Box,
        fingerprint: host.fingerprint().to_owned(),
        public_key: host.armored_public().to_owned(),
        token: "losos_pat_0123abcd_00112233445566778899aabbccddeeff00112233".into(),
        key_fingerprint: user.fingerprint().to_owned(),
        tls_pin: None,
    };
    let unlocked = UserKey::from_armored(&user.armored_secret().unwrap())
        .unwrap()
        .unlock("")
        .unwrap();
    let session = Session::new(&record, unlocked, Http::new(Trust::Public)).unwrap();
    Pair {
        host,
        user,
        session,
    }
}

fn ring(key: &HostKey) -> Keyring {
    Keyring::parse(key.armored_public()).unwrap()
}

#[test]
fn the_box_opens_what_the_client_seals() {
    let p = pair();
    let now = losos_access::now_secs();
    let sealed = p
        .session
        .seal("get", "/api/state", json!(null), now, NONCE)
        .unwrap();
    let opened = open(
        &p.host,
        &ring(&p.user),
        sealed.as_bytes(),
        &Replay::default(),
        now,
    )
    .unwrap();
    assert_eq!(opened.request.method, "GET");
    assert_eq!(opened.request.path, "/api/state");
    assert_eq!(opened.request.nonce, NONCE);
    assert!(opened
        .request
        .host
        .eq_ignore_ascii_case(p.host.fingerprint()));
}

#[test]
fn the_box_refuses_another_signer_a_stale_call_and_a_replay() {
    let p = pair();
    let now = losos_access::now_secs();
    let sealed = p
        .session
        .seal("GET", "/api/state", json!(null), now, NONCE)
        .unwrap();

    let stranger = HostKey::generate("stranger").unwrap();
    let refused = open(
        &p.host,
        &ring(&stranger),
        sealed.as_bytes(),
        &Replay::default(),
        now,
    );
    assert!(matches!(refused, Err(Refusal::Unsigned)), "{refused:?}");

    let late = now + losos_access::MAX_SKEW_SECS + 1;
    let refused = open(
        &p.host,
        &ring(&p.user),
        sealed.as_bytes(),
        &Replay::default(),
        late,
    );
    assert!(matches!(refused, Err(Refusal::Stale)), "{refused:?}");

    let replay = Replay::default();
    open(&p.host, &ring(&p.user), sealed.as_bytes(), &replay, now).unwrap();
    let again = open(&p.host, &ring(&p.user), sealed.as_bytes(), &replay, now);
    assert!(again.is_err(), "a nonce was taken twice");
}

#[test]
fn takes_an_answer_signed_by_the_pinned_key_for_this_nonce() {
    let p = pair();
    let user = p.user.as_seeded().unwrap();
    let answer = seal(&p.host, &user, NONCE, 200, json!({"mode": "local"})).unwrap();
    let (status, body) = p.session.open(&answer, NONCE).unwrap();
    assert_eq!(status, 200);
    assert_eq!(body, json!({"mode": "local"}));
}

#[test]
fn refuses_an_answer_to_another_request() {
    let p = pair();
    let user = p.user.as_seeded().unwrap();
    let answer = seal(&p.host, &user, NONCE, 200, json!({})).unwrap();
    let other = "ffeeddccbbaa99887766554433221100";
    let problem = p.session.open(&answer, other).unwrap_err();
    assert_eq!(problem.code, "losos::envelope");
}

#[test]
fn refuses_an_answer_signed_by_another_key() {
    let p = pair();
    let user = p.user.as_seeded().unwrap();
    let impostor = HostKey::generate("impostor").unwrap();
    let answer = seal(&impostor, &user, NONCE, 200, json!({})).unwrap();
    let problem = p.session.open(&answer, NONCE).unwrap_err();
    assert_eq!(problem.code, "losos::envelope");
}

#[test]
fn refuses_a_pin_whose_key_does_not_match_its_fingerprint() {
    let host = HostKey::generate("box").unwrap();
    let other = HostKey::generate("other").unwrap();
    let user = HostKey::generate("owner").unwrap();
    let record = HostRecord {
        url: "http://box.test".into(),
        kind: HostKind::Box,
        fingerprint: other.fingerprint().to_owned(),
        public_key: host.armored_public().to_owned(),
        token: String::new(),
        key_fingerprint: user.fingerprint().to_owned(),
        tls_pin: None,
    };
    let unlocked = UserKey::from_armored(&user.armored_secret().unwrap())
        .unwrap()
        .unlock("")
        .unwrap();
    let refused = Session::new(&record, unlocked, Http::new(Trust::Public));
    assert_eq!(refused.err().unwrap().code, "losos::pin");
}
