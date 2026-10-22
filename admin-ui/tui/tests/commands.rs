//! Every command, run as a script runs it, against a box and an edge on
//! loopback that open and seal the envelope with losos-access's server half.

mod common;

use std::sync::Arc;

use common::{fake_bin, mock, owner, Client, TOKEN};
use losos::keychain::SecretTool;
use losos_access::UserKey;
use serde_json::{json, Value};

fn box_handler() -> Box<common::Handler> {
    Box::new(|method, path, _body| match (method, path) {
        ("GET", "/api/state") => (200, json!({ "mode": "local" })),
        ("POST", "/api/fail") => (500, json!({ "error": "it broke" })),
        ("GET", "/api/credentials") => (
            200,
            json!({
                "key": "tpm2",
                "credentials": [
                    { "name": "admin-token", "app": "admin", "source": "box", "set": true,
                      "updated": 1_760_000_000, "services": ["lososd"], "rotate": false,
                      "adopted": true, "remade": false },
                    { "name": "authelia-session", "app": "login", "source": "box", "set": true,
                      "updated": 1_760_000_000, "services": ["authelia-losos"], "rotate": true,
                      "adopted": false, "remade": false },
                    { "name": "backup-bucket", "app": "backup", "source": "owner", "set": false,
                      "updated": null, "services": [], "rotate": false,
                      "adopted": false, "remade": false },
                ],
            }),
        ),
        ("POST", "/api/credentials/authelia-session/rotate") => (
            200,
            json!({ "name": "authelia-session", "set": true, "services": ["authelia-losos"] }),
        ),
        ("POST", "/api/credentials/admin-token/rotate") => (
            409,
            json!({ "error": "admin-token cannot be made new here" }),
        ),
        _ => (404, json!({ "error": "no such route" })),
    })
}

fn edge_handler() -> Box<common::Handler> {
    Box::new(|method, path, body| match (method, path) {
        ("GET", "/overview") => (200, json!({ "tenants": [] })),
        ("GET", "/secrets") => (
            200,
            json!([
                { "name": "STRIPE_KEY", "sealed": true, "updated": 1_760_000_000 },
                { "name": "stripe-webhook-secret", "sealed": false, "updated": null },
            ]),
        ),
        ("POST", "/secrets") => (200, json!({ "sealed": body["name"] })),
        ("POST", "/reconcile") => (200, Value::Null),
        ("GET", "/escrow/releaser") => (
            200,
            json!({
                "address": "0xabc",
                "networks": [
                    { "network": "base", "escrow": "0xe5", "balanceWei": "1500000000000000000", "escrowAgrees": true, "error": null },
                    { "network": "arbitrum", "escrow": "0xe6", "balanceWei": null, "escrowAgrees": null, "error": "rpc down" },
                ],
            }),
        ),
        ("POST", "/escrow/releaser") => (200, json!({ "address": "0xdef" })),
        _ => (404, json!({ "error": "no such route" })),
    })
}

fn path(file: &tempfile::NamedTempFile) -> &str {
    file.path().to_str().unwrap()
}

#[test]
fn prints_its_version_and_its_commands() {
    let client = Client::new();
    let r = client.run(&["--version"], "");
    assert_eq!(r.code, 0);
    assert!(r.out.starts_with("losos "), "{}", r.out);
    let r = client.run(&["--help"], "");
    assert_eq!(r.code, 0);
    for command in ["login", "logout", "hosts", "key", "call", "edge", "ci"] {
        assert!(
            r.out.contains(&format!("  {command} ")),
            "{command} missing from {}",
            r.out
        );
    }
    let r = client.run(&["--help", "--lang", "sk"], "");
    assert!(r.out.contains("Príkazy"), "{}", r.out);
    let r = client.run(&["call"], "");
    assert_eq!(r.code, 2, "a usage error");
    assert!(r.err.contains("<METHOD>"), "{}", r.err);
}

#[test]
fn takes_global_options_before_the_command_and_refuses_a_url_there() {
    let client = Client::new();
    let r = client.run(&["--lang", "sk", "-v", "hosts"], "");
    assert_eq!(r.code, 0, "{}", r.err);
    let r = client.run(&["--theme", "light", "key", "list"], "");
    assert_eq!(r.code, 0, "{}", r.err);
    let r = client.run(&["--lang", "en", "call", "GET"], "");
    assert_eq!(r.code, 2, "a usage error");
    assert!(r.err.contains("<PATH>"), "{}", r.err);
    let r = client.run(&["mattbox.local", "hosts"], "");
    assert_eq!(r.code, 2, "a usage error");
    assert!(r.err.contains("--host mattbox.local"), "{}", r.err);
}

#[test]
fn imports_a_key_lists_it_and_refuses_a_public_key() {
    let client = Client::new();
    let me = owner();
    let r = client.run(&["key", "import", path(&me.file)], "");
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.contains("owner@example.org"));
    let r = client.run(&["key", "list"], "");
    assert!(r.out.contains("owner@example.org"));

    let mut public = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut public, me.key.armored_public().as_bytes()).unwrap();
    let r = client.run(&["key", "import", path(&public)], "");
    assert_eq!(r.code, 1);
    assert!(r.err.contains("losos::key"), "{}", r.err);

    let r = client.run(&["key", "import", "/nonexistent/key.asc"], "");
    assert_eq!(r.code, 1);
}

#[test]
fn logs_in_to_a_box_from_a_script_and_calls_it() {
    let me = owner();
    let host = mock(false, me.key.armored_public(), box_handler());
    let client = Client::new();
    client.run(&["key", "import", path(&me.file)], "");

    // A script must confirm the fingerprint on the command line.
    let r = client.run(&["login", &host.url], TOKEN);
    assert_eq!(r.code, 1);
    assert!(r.err.contains("--fingerprint"), "{}", r.err);

    let r = client.run(&["login", &host.url, "--fingerprint", "AAAA"], TOKEN);
    assert_eq!(r.code, 2, "a fingerprint that does not match");

    let r = client.run(
        &["login", &host.url, "--fingerprint", &host.fingerprint()],
        "not a token\n",
    );
    assert_eq!(r.code, 1);

    let r = client.run(
        &["login", &host.url, "--fingerprint", &host.fingerprint()],
        &format!("{TOKEN}\n"),
    );
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.contains(&host.url));
    assert!(
        !r.out.contains("TLS"),
        "plain HTTP pins no certificate: {}",
        r.out
    );

    let r = client.run(&["hosts"], "");
    assert!(r.out.contains(&host.url), "{}", r.out);

    let r = client.run(&["call", "GET", "/api/state"], "");
    assert_eq!(r.code, 0, "{}", r.err);
    assert_eq!(r.err.trim(), "200");
    let body: Value = serde_json::from_str(&r.out).unwrap();
    assert_eq!(body["mode"], "local");

    let r = client.run(&["call", "post", "/api/fail", "{\"a\":1}"], "");
    assert_eq!(r.code, 1, "the inner status decides");
    assert_eq!(r.err.trim(), "500");
    assert_eq!(
        host.calls.lock().unwrap().last().unwrap().2,
        json!({ "a": 1 })
    );

    let r = client.run(&["call", "GET", "/api/state", "{oops"], "");
    assert_eq!(r.code, 1);

    let r = client.run(&["logout", &host.url], "");
    assert_eq!(r.code, 0);
    let r = client.run(&["call", "GET", "/api/state"], "");
    assert_eq!(r.code, 1, "no host left");
}

#[test]
fn a_plain_refusal_is_shown_as_not_sealed() {
    let me = owner();
    // The box seeded someone else's key, so it cannot answer sealed.
    let stranger = HostKeyPublic::new();
    let host = mock(false, &stranger.0, box_handler());
    let client = Client::new();
    client.run(&["key", "import", path(&me.file)], "");
    let r = client.run(
        &["login", &host.url, "--fingerprint", &host.fingerprint()],
        TOKEN,
    );
    assert_eq!(r.code, 0, "{}", r.err);
    let r = client.run(&["call", "GET", "/api/state"], "");
    assert_eq!(r.code, 1);
    let note = losos::i18n::template("tui.call.notSealed", &[], losos::i18n::Locale::En);
    assert_eq!(r.err.trim(), format!("401, {note}"));
    assert!(r.out.contains("error"), "{}", r.out);
}

struct HostKeyPublic(String);

impl HostKeyPublic {
    fn new() -> HostKeyPublic {
        HostKeyPublic(
            losos_access::HostKey::generate("Someone <someone@example.org>")
                .unwrap()
                .armored_public()
                .to_string(),
        )
    }
}

#[test]
fn refuses_loudly_when_a_pinned_host_shows_another_key() {
    let me = owner();
    let host = mock(false, me.key.armored_public(), box_handler());
    let client = Client::new();
    client.run(&["key", "import", path(&me.file)], "");
    let first = host.fingerprint();
    let r = client.run(&["login", &host.url, "--fingerprint", &first], TOKEN);
    assert_eq!(r.code, 0, "{}", r.err);

    *host.host.lock().unwrap() = losos_access::HostKey::generate("LosOS box <other>").unwrap();
    let r = client.run(
        &["login", &host.url, "--fingerprint", &host.fingerprint()],
        TOKEN,
    );
    assert_eq!(r.code, 2);
    assert!(
        r.err.contains(&losos::vault::group_fingerprint(&first)),
        "{}",
        r.err
    );

    // Calls stop too: the answer is no longer signed by the pinned key.
    let r = client.run(&["call", "GET", "/api/state"], "");
    assert_eq!(r.code, 1);
}

#[test]
fn logs_in_to_an_edge_and_runs_the_edge_commands() {
    let me = owner();
    let edge = mock(true, me.key.armored_public(), edge_handler());
    let client = Client::new();
    client.run(&["key", "import", path(&me.file)], "");
    let r = client.run(
        &["login", &edge.url, "--fingerprint", &edge.fingerprint()],
        TOKEN,
    );
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.contains("edge"), "{}", r.out);

    let r = client.run(&["edge", "secret", "list"], "");
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(
        r.out.contains("STRIPE_KEY") && r.out.contains("2025-10-09T08:53:20Z"),
        "{}",
        r.out
    );

    let r = client.run(
        &["edge", "secret", "set", "stripe-webhook-secret"],
        "whsec_secret\n",
    );
    assert_eq!(r.code, 0, "{}", r.err);
    let calls = edge.calls.lock().unwrap().clone();
    assert_eq!(
        calls.last().unwrap().2,
        json!({ "name": "stripe-webhook-secret", "value": "whsec_secret" })
    );
    assert!(!r.out.contains("whsec_secret"));

    let r = client.run(&["edge", "secret", "set", "stripe-webhook-secret"], "");
    assert_eq!(r.code, 1, "an empty value is refused");

    let r = client.run(&["edge", "reconcile"], "");
    assert_eq!(r.code, 0, "{}", r.err);

    let r = client.run(&["edge", "escrow", "releaser"], "");
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(
        r.out.contains("0xabc") && r.out.contains("1.5 ETH") && r.out.contains("rpc down"),
        "{}",
        r.out
    );

    let r = client.run(&["edge", "escrow", "releaser", "--replace"], "");
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.contains("0xdef"));
    assert_eq!(
        edge.calls.lock().unwrap().last().unwrap().2,
        json!({ "replace": true })
    );

    let r = client.run(
        &["edge", "secret", "list", "--host", "https://not.logged.in"],
        "",
    );
    assert_eq!(r.code, 1);
}

#[test]
fn lists_the_credentials_of_a_box_and_makes_one_new() {
    let me = owner();
    let host = mock(false, me.key.armored_public(), box_handler());
    let client = Client::new();
    client.run(&["key", "import", path(&me.file)], "");
    client.run(
        &["login", &host.url, "--fingerprint", &host.fingerprint()],
        TOKEN,
    );

    let r = client.run(&["credentials", "list"], "");
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.starts_with("Sealed to this box's TPM."), "{}", r.out);
    let row = |name: &str| {
        r.out
            .lines()
            .find(|l| l.starts_with(name))
            .unwrap_or_else(|| panic!("no row for {name}: {}", r.out))
            .to_string()
    };
    assert!(row("admin-token").contains("made by the box"));
    assert!(row("admin-token").contains("2025-10-09"));
    assert!(row("admin-token").contains("read by lososd"));
    assert!(row("admin-token").contains("moved from a plain file"));
    assert!(row("backup-bucket").contains("not set"));
    assert!(
        r.out.contains("makes a new value for: authelia-session\n"),
        "{}",
        r.out
    );

    let r = client.run(&["credentials", "rotate", "authelia-session"], "");
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.contains("restarted authelia-losos"), "{}", r.out);

    let r = client.run(&["credentials", "rotate", "admin-token"], "");
    assert_eq!(r.code, 1, "the box's refusal is the answer");
    assert!(r.err.contains("cannot be made new"), "{}", r.err);

    // Never sent: a name that is not a name would walk the path.
    let calls = host.calls.lock().unwrap().len();
    let r = client.run(&["credentials", "rotate", "../setup/claim"], "");
    assert_eq!(r.code, 1);
    assert_eq!(host.calls.lock().unwrap().len(), calls);
}

#[test]
fn edge_commands_refuse_a_box() {
    let me = owner();
    let host = mock(false, me.key.armored_public(), box_handler());
    let client = Client::new();
    client.run(&["key", "import", path(&me.file)], "");
    client.run(
        &["login", &host.url, "--fingerprint", &host.fingerprint()],
        TOKEN,
    );
    let r = client.run(&["edge", "reconcile", "--host", &host.url], "");
    assert_eq!(r.code, 1);
    assert!(
        r.err.contains("not an edge") || r.err.contains("edge"),
        "{}",
        r.err
    );
}

fn gh_log(log: &std::path::Path) -> Vec<(String, String)> {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    text.split(">>> ")
        .filter(|s| !s.is_empty())
        .map(|block| {
            let (args, rest) = block.split_once('\n').unwrap();
            (
                args.to_string(),
                rest.trim_end_matches("\n<<<\n").to_string(),
            )
        })
        .collect()
}

#[test]
fn sets_up_ci_through_gh_and_ci_can_call_the_edge() {
    let me = owner();
    let edge = mock(true, me.key.armored_public(), edge_handler());
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("gh.log");
    let mut client = Client::new();
    client
        .set("LOSOS_GH", &fake_bin("gh"))
        .set("FAKE_GH_LOG", log.to_str().unwrap());
    client.run(&["key", "import", path(&me.file)], "");
    client.run(
        &["login", &edge.url, "--fingerprint", &edge.fingerprint()],
        TOKEN,
    );

    let r = client.run(
        &["ci", "setup", "--edge", &edge.url, "--repo", "owner/repo"],
        &format!("{TOKEN}\n"),
    );
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.contains("BEGIN PGP PUBLIC KEY BLOCK"), "{}", r.out);
    assert!(
        !r.out.contains("PRIVATE KEY"),
        "the CI key's secret half is never printed"
    );

    let calls = gh_log(&log);
    let names: Vec<&str> = calls.iter().map(|(a, _)| a.as_str()).collect();
    assert_eq!(
        names,
        [
            "secret set LOSOS_CI_GPG_KEY --repo owner/repo",
            "secret set LOSOS_EDGE_TOKEN --repo owner/repo",
            "variable set LOSOS_EDGE_URL --repo owner/repo",
            "variable set LOSOS_EDGE_HOST_KEY --repo owner/repo",
        ]
    );
    let value = |name: &str| {
        calls
            .iter()
            .find(|(a, _)| a.contains(name))
            .map(|(_, v)| v.clone())
            .unwrap()
    };
    assert_eq!(value("LOSOS_EDGE_TOKEN"), TOKEN);
    assert_eq!(value("LOSOS_EDGE_URL"), edge.url);

    // The CI key is the client's own, separate from the owner's, and kept.
    let ci_key = UserKey::from_armored(&value("LOSOS_CI_GPG_KEY")).unwrap();
    assert_ne!(ci_key.fingerprint(), me.key.fingerprint());
    assert!(!ci_key.needs_passphrase());
    let r = client.run(&["key", "list"], "");
    assert!(r.out.contains("LosOS CI <owner/repo>"), "{}", r.out);

    // Once the edge seeds it, a runner with only the four values gets in,
    // with an empty keychain.
    edge.seed(&ci_key.armored_public().unwrap());
    let mut runner = Client::new();
    runner
        .set("LOSOS_CI", "1")
        .set("LOSOS_CI_GPG_KEY", &value("LOSOS_CI_GPG_KEY"))
        .set("LOSOS_EDGE_TOKEN", &value("LOSOS_EDGE_TOKEN"))
        .set("LOSOS_EDGE_URL", &value("LOSOS_EDGE_URL"))
        .set("LOSOS_EDGE_HOST_KEY", &value("LOSOS_EDGE_HOST_KEY"));
    let r = runner.run(&["edge", "reconcile"], "");
    assert_eq!(r.code, 0, "{}", r.err);
    let r = runner.run(&["call", "GET", "/overview"], "");
    assert_eq!(r.code, 0, "{}", r.err);

    runner.env.remove("LOSOS_EDGE_TOKEN");
    let r = runner.run(&["edge", "reconcile"], "");
    assert_eq!(r.code, 1);
    assert!(r.err.contains("LOSOS_EDGE_TOKEN"), "{}", r.err);
}

#[test]
fn prints_the_four_names_and_no_secret_when_gh_is_signed_out() {
    let me = owner();
    let edge = mock(true, me.key.armored_public(), edge_handler());
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("gh.log");
    let mut client = Client::new();
    client
        .set("LOSOS_GH", &fake_bin("gh"))
        .set("FAKE_GH_LOG", log.to_str().unwrap())
        .set("FAKE_GH_SIGNED_OUT", "1");
    client.run(&["key", "import", path(&me.file)], "");
    client.run(
        &["login", &edge.url, "--fingerprint", &edge.fingerprint()],
        TOKEN,
    );

    let r = client.run(&["ci", "setup", "--edge", &edge.url], TOKEN);
    assert_eq!(r.code, 0, "{}", r.err);
    for name in [
        "LOSOS_CI_GPG_KEY",
        "LOSOS_EDGE_TOKEN",
        "LOSOS_EDGE_URL",
        "LOSOS_EDGE_HOST_KEY",
    ] {
        assert!(r.out.contains(name), "{name}: {}", r.out);
    }
    assert!(!r.out.contains(TOKEN) && !r.out.contains("PRIVATE KEY"));
    assert!(gh_log(&log).is_empty());

    let r = client.run(&["ci", "setup", "--edge", &edge.url, "--print"], TOKEN);
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.contains(TOKEN) && r.out.contains("PRIVATE KEY"));

    let r = client.run(&["ci", "setup", "--edge", "https://not.logged.in"], TOKEN);
    assert_eq!(r.code, 1);
}

#[test]
fn removes_a_key_and_names_the_hosts_left_without_one() {
    let me = owner();
    let host = mock(false, me.key.armored_public(), box_handler());
    let client = Client::new();
    client.run(&["key", "import", path(&me.file)], "");
    client.run(
        &["login", &host.url, "--fingerprint", &host.fingerprint()],
        TOKEN,
    );
    let spaced = losos::vault::group_fingerprint(me.key.fingerprint());
    let r = client.run(&["key", "remove", &spaced], "");
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.contains(&host.url), "{}", r.out);
    let r = client.run(&["call", "GET", "/api/state"], "");
    assert_eq!(r.code, 1, "the host's key is gone");
}

#[test]
fn works_through_secret_tool() {
    let store = tempfile::tempdir().unwrap();
    let me = owner();
    let host = mock(false, me.key.armored_public(), box_handler());
    let mut client = Client::with(Arc::new(SecretTool::new(fake_bin("secret-tool"))));
    // The fake reads its store from the environment of the test process.
    std::env::set_var("FAKE_SECRET_STORE", store.path());
    client.set("FAKE_SECRET_STORE", store.path().to_str().unwrap());
    let r = client.run(&["key", "import", path(&me.file)], "");
    assert_eq!(r.code, 0, "{}", r.err);
    let r = client.run(
        &["login", &host.url, "--fingerprint", &host.fingerprint()],
        TOKEN,
    );
    assert_eq!(r.code, 0, "{}", r.err);
    let r = client.run(&["call", "GET", "/api/state"], "");
    assert_eq!(r.code, 0, "{}", r.err);
    let r = client.run(&["hosts"], "");
    assert!(r.out.contains(&host.url));
}

#[test]
fn says_so_in_one_sentence_when_there_is_no_secret_service() {
    let client = Client::with(Arc::new(SecretTool::new("/nonexistent/secret-tool")));
    let r = client.run(&["hosts"], "");
    assert_eq!(r.code, 1);
    assert!(r.err.contains("secret-tool"), "{}", r.err);
}
