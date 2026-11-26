//! A box's sign-in is LosOS Login: the device code, a refresh when the
//! access token has run out, and a logout that hands both tokens back.

mod common;

use std::sync::Arc;

use common::{mock, owner, Client, TOKEN};
use losos::keychain::MemoryKeychain;
use losos::vault::{HostKind, HostRecord, Vault};

fn handler() -> Box<common::Handler> {
    Box::new(|method, path, _body| match (method, path) {
        ("GET", "/api/me") => (
            200,
            serde_json::json!({
                "name": "matus",
                "admin": true,
                "role": "owner",
                "sections": ["storage", "backup", "apps", "network"],
                "locked": [],
            }),
        ),
        ("GET", "/api/state") => (200, serde_json::json!({ "mode": "local" })),
        _ => (404, serde_json::json!({ "error": "no such route" })),
    })
}

fn legacy(url: &str) -> HostRecord {
    HostRecord {
        url: url.to_string(),
        kind: HostKind::Box,
        fingerprint: "AB".into(),
        public_key: String::new(),
        token: TOKEN.into(),
        key_fingerprint: "CD".into(),
        tls_pin: None,
        edge_url: None,
        access_token: None,
        refresh_token: None,
        expires_at: None,
    }
}

#[test]
fn sign_in_uses_losos_login_and_logout_revokes_it() {
    let me = owner();
    let host = mock(false, me.key.armored_public(), handler());
    let keychain = Arc::new(MemoryKeychain::new());
    let client = Client::with(keychain.clone());

    let r = client.run(&["login", &host.url], "");
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.contains("LosOS Login"), "{}", r.out);
    assert!(r.out.contains("WDJBMJHT"), "{}", r.out);
    assert!(
        r.out.contains("matus") && r.out.contains("owner"),
        "{}",
        r.out
    );

    let record = Vault::new(keychain.clone())
        .get_host(&host.url)
        .unwrap()
        .unwrap();
    assert!(record.login_session());
    assert!(!record.legacy_box());
    assert!(record.token.is_empty(), "no access token of the old kind");

    let r = client.run(&["call", "GET", "/api/state"], "");
    assert_eq!(r.code, 0, "{}", r.err);
    let shown = host.login.lock().unwrap().shown.clone();
    assert_eq!(
        shown.last().unwrap(),
        record.access_token.as_deref().unwrap()
    );

    let r = client.run(&["logout", &host.url], "");
    assert_eq!(r.code, 0, "{}", r.err);
    let login = host.login.lock().unwrap();
    assert!(login.revoked.iter().any(|t| t.starts_with("authelia_at_")));
    assert!(login.revoked.iter().any(|t| t.starts_with("authelia_rt_")));
    assert!(login.access.is_empty());
    drop(login);
    assert!(Vault::new(keychain).get_host(&host.url).unwrap().is_none());
    let r = client.run(&["call", "GET", "/api/state"], "");
    assert_eq!(r.code, 1, "signed out");
}

#[test]
fn an_expired_access_token_is_refreshed_and_a_dead_refresh_ends_the_sign_in() {
    let me = owner();
    let host = mock(false, me.key.armored_public(), handler());
    let keychain = Arc::new(MemoryKeychain::new());
    let client = Client::with(keychain.clone());
    assert_eq!(client.run(&["login", &host.url], "").code, 0);

    let vault = Vault::new(keychain.clone());
    let mut record = vault.get_host(&host.url).unwrap().unwrap();
    let first = record.access_token.clone().unwrap();
    record.expires_at = Some(0);
    vault.put_host(&record).unwrap();
    host.login.lock().unwrap().expire();

    let r = client.run(&["call", "GET", "/api/state"], "");
    assert_eq!(r.code, 0, "{}", r.err);
    let login = host.login.lock().unwrap();
    assert_eq!(login.refreshes, 1);
    assert_ne!(login.access.last().unwrap(), &first);
    let renewed = login.access.last().unwrap().clone();
    drop(login);
    assert_eq!(
        Vault::new(keychain.clone())
            .get_host(&host.url)
            .unwrap()
            .unwrap()
            .access_token
            .as_deref(),
        Some(renewed.as_str())
    );

    let mut record = Vault::new(keychain.clone())
        .get_host(&host.url)
        .unwrap()
        .unwrap();
    record.expires_at = Some(0);
    Vault::new(keychain).put_host(&record).unwrap();
    host.login.lock().unwrap().refresh = None;
    host.login.lock().unwrap().expire();
    let r = client.run(&["call", "GET", "/api/state"], "");
    assert_eq!(r.code, 1, "{}", r.out);
    assert!(
        r.err.contains("losos login"),
        "a dead refresh asks for a new sign-in: {}",
        r.err
    );
}

#[test]
fn a_denied_approval_stores_nothing() {
    let me = owner();
    let host = mock(false, me.key.armored_public(), handler());
    host.login.lock().unwrap().deny = true;
    let keychain = Arc::new(MemoryKeychain::new());
    let client = Client::with(keychain.clone());
    let r = client.run(&["login", &host.url], "");
    assert_eq!(r.code, 1, "{}", r.out);
    assert!(
        r.err.contains("denied") || r.err.contains("Denied"),
        "{}",
        r.err
    );
    assert!(Vault::new(keychain).get_host(&host.url).unwrap().is_none());
}

#[test]
fn an_old_access_token_is_not_a_way_in() {
    let me = owner();
    let host = mock(false, me.key.armored_public(), handler());
    let keychain = Arc::new(MemoryKeychain::new());
    Vault::new(keychain.clone())
        .put_host(&legacy(&host.url))
        .unwrap();
    let client = Client::with(keychain.clone());

    let r = client.run(&["call", "GET", "/api/state"], "");
    assert_eq!(r.code, 1, "{}", r.out);
    assert!(r.err.contains("LosOS Login"), "{}", r.err);
    assert!(host.login.lock().unwrap().shown.is_empty());
    assert!(host.calls.lock().unwrap().is_empty());

    let r = client.run(&["hosts"], "");
    assert!(r.out.contains("old access token"), "{}", r.out);

    let r = client.run(&["logout", &host.url], "");
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.contains("old access-token"), "{}", r.out);
    assert!(Vault::new(keychain.clone())
        .get_host(&host.url)
        .unwrap()
        .is_none());

    let r = client.run(&["login", &host.url], "");
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(Vault::new(keychain)
        .get_host(&host.url)
        .unwrap()
        .unwrap()
        .login_session());
}
