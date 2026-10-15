//! `SecretTool` against a stand-in for libsecret's secret-tool with the same
//! arguments, output and exit statuses (tests/fake-bin/secret-tool).

mod common;

use common::fake_bin;
use losos::keychain::{Keychain, SecretTool};

const KEY: &str =
    "-----BEGIN PGP PRIVATE KEY BLOCK-----\n\nxVgEZ\nabc=\n-----END PGP PRIVATE KEY BLOCK-----\n";

// One test, so the environment the stand-in reads is set once.
#[test]
fn stores_looks_up_searches_and_clears() {
    let store = tempfile::tempdir().unwrap();
    std::env::set_var("FAKE_SECRET_STORE", store.path());
    std::env::remove_var("FAKE_SECRET_DOWN");
    let k = SecretTool::new(fake_bin("secret-tool"));
    let key = [
        ("service", "losos"),
        ("kind", "gpg-key"),
        ("fingerprint", "AB"),
    ];
    let host = [
        ("service", "losos"),
        ("kind", "host"),
        ("url", "https://box.local"),
    ];

    k.probe().unwrap();
    assert_eq!(k.lookup(&key).unwrap(), None);
    assert!(k.search(&[("service", "losos")]).unwrap().is_empty());

    k.store(&key, "losos gpg-key Owner <owner@example.org>", KEY)
        .unwrap();
    k.store(
        &host,
        "losos box https://box.local",
        "{\"url\":\"https://box.local\"}",
    )
    .unwrap();
    assert_eq!(k.lookup(&key).unwrap().as_deref(), Some(KEY));

    // A secret that spans lines, then the next item.
    let all = k.search(&[("service", "losos")]).unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].secret, KEY.trim_end());
    assert_eq!(all[0].label, "losos gpg-key Owner <owner@example.org>");
    assert_eq!(all[1].secret, "{\"url\":\"https://box.local\"}");

    // Stored again under the same attributes, it replaces the first.
    k.store(&host, "losos box https://box.local", "{\"v\":2}")
        .unwrap();
    let hosts = k.search(&[("service", "losos"), ("kind", "host")]).unwrap();
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].secret, "{\"v\":2}");

    k.clear(&host).unwrap();
    assert_eq!(k.lookup(&host).unwrap(), None);
    assert_eq!(k.search(&[("service", "losos")]).unwrap().len(), 1);

    // No Secret Service: one sentence naming what secret-tool said.
    std::env::set_var("FAKE_SECRET_DOWN", "1");
    let problem = k.probe().unwrap_err();
    assert!(
        problem.message.contains("org.freedesktop.secrets"),
        "{}",
        problem.message
    );
    assert!(k.store(&key, "x", "y").is_err());
    std::env::remove_var("FAKE_SECRET_DOWN");

    // Not installed at all.
    let missing = SecretTool::new("/nonexistent/secret-tool")
        .probe()
        .unwrap_err();
    assert!(missing.message.contains("libsecret"), "{}", missing.message);
}
