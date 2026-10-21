//! `losos cloud setup`, `losos git setup` and git's credential helper,
//! against a box on loopback with its own certificate: what lands in the
//! keychain, in the file manager's bookmarks and in git's configuration.

mod common;

use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};

use common::{mock_tls, owner, Cert, Client, Mock, Owner, TOKEN};
use losos::computer::{pinned_public_key, subject_public_key_info};
use losos::keychain::{Keychain, MemoryKeychain};
use serde_json::{json, Value};

const PASSWORD: &str = "Zqx-marmalade-77-parapet";

/// The box's two device routes; every body they were sent is kept.
fn device_handler(seen: Arc<Mutex<Vec<Value>>>) -> Box<common::Handler> {
    Box::new(move |method, path, body| {
        seen.lock().unwrap().push(body.clone());
        let right = body["password"] == json!(PASSWORD);
        match (method, path) {
            ("POST", "/api/devices/cloud") if right => (
                200,
                json!({ "user": "notshared", "appPassword": "AbCd-EfGh-1234", "path": "/nextcloud/remote.php/dav/files/notshared/" }),
            ),
            ("POST", "/api/devices/git") if right => (
                200,
                json!({ "user": "notshared", "token": "9f8e7d6c5b4a", "path": "/git/" }),
            ),
            ("POST", "/api/devices/cloud" | "/api/devices/git") => (
                403,
                json!({ "error": "that password was not accepted", "wrongPassword": true }),
            ),
            _ => (404, json!({ "error": "no such route" })),
        }
    })
}

struct Setup {
    _me: Owner,
    cert: Cert,
    host: Mock,
    keychain: Arc<MemoryKeychain>,
    client: Client,
    home: tempfile::TempDir,
    seen: Arc<Mutex<Vec<Value>>>,
}

/// A box with its own certificate, logged in to.
fn logged_in() -> Setup {
    let me = owner();
    let cert = Cert::new();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let host = mock_tls(
        false,
        me.key.armored_public(),
        device_handler(seen.clone()),
        &cert,
    );
    let keychain = Arc::new(MemoryKeychain::new());
    let home = tempfile::tempdir().unwrap();
    let mut client = Client::with(keychain.clone());
    client
        .set(
            "XDG_CONFIG_HOME",
            &home.path().join("config").display().to_string(),
        )
        .set("HOME", &home.path().display().to_string())
        .set(
            "GIT_CONFIG_GLOBAL",
            &home.path().join("gitconfig").display().to_string(),
        )
        .set("LOSOS_SELF", "/opt/losos/bin/losos")
        .set("LOSOS_DEVICE", "work pc");
    client.run(&["key", "import", me.file.path().to_str().unwrap()], "");
    let r = client.run(
        &["login", &host.url, "--fingerprint", &host.fingerprint()],
        TOKEN,
    );
    assert_eq!(r.code, 0, "{}", r.err);
    Setup {
        _me: me,
        cert,
        host,
        keychain,
        client,
        home,
        seen,
    }
}

fn git_config(home: &Path, name: &str) -> Vec<String> {
    let out = Command::new("git")
        .args(["config", "--file"])
        .arg(home.join("gitconfig"))
        .args(["--get-all", name])
        .output()
        .unwrap();
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn cloud_setup_puts_the_files_in_the_file_manager_with_no_password_to_type() {
    let s = logged_in();
    let r = s.client.run(&["cloud", "setup"], PASSWORD);
    assert_eq!(r.code, 0, "{}", r.err);
    let port = url::Url::parse(&s.host.url).unwrap().port().unwrap();
    let location =
        format!("davs://notshared@127.0.0.1:{port}/nextcloud/remote.php/dav/files/notshared/");
    assert!(r.out.contains("as work pc"), "{}", r.out);
    assert!(r.out.contains(&location), "{}", r.out);
    assert!(r.out.contains("certificate"), "{}", r.out);
    assert!(r.out.contains("LosOS on work pc"), "{}", r.out);

    // The box was asked with the password and this computer's name.
    let sent = s.seen.lock().unwrap().last().cloned().unwrap();
    assert_eq!(sent, json!({ "password": PASSWORD, "device": "work pc" }));

    // GVfs finds the app password by exactly these attributes before it
    // asks: the realm the WebDAV challenge names, the port filled in.
    let port = port.to_string();
    let found = s
        .keychain
        .lookup(&[
            ("xdg:schema", "org.gnome.keyring.NetworkPassword"),
            ("user", "notshared"),
            ("server", "127.0.0.1"),
            ("protocol", "http"),
            ("object", "LosOS cloud"),
            ("authtype", "basic"),
            ("port", &port),
        ])
        .unwrap();
    assert_eq!(found.as_deref(), Some("AbCd-EfGh-1234"));
    let own = s
        .keychain
        .lookup(&[
            ("service", "losos"),
            ("kind", "cloud"),
            ("url", &s.host.url),
        ])
        .unwrap()
        .unwrap();
    assert!(own.contains("AbCd-EfGh-1234"), "{own}");

    let bookmarks = s.home.path().join("config/gtk-3.0/bookmarks");
    assert_eq!(
        std::fs::read_to_string(&bookmarks).unwrap(),
        format!("{location} LosOS cloud\n")
    );
    // Again: one bookmark still, and the credential replaced.
    let r = s.client.run(&["cloud", "setup"], PASSWORD);
    assert_eq!(r.code, 0, "{}", r.err);
    assert_eq!(
        std::fs::read_to_string(&bookmarks).unwrap().lines().count(),
        1
    );
}

#[test]
fn a_wrong_password_sets_up_nothing() {
    let s = logged_in();
    let before = s.keychain.len();
    let r = s.client.run(&["cloud", "setup"], "not-the-password");
    assert_eq!(r.code, 1);
    assert!(
        r.err.contains("not the password of LosOS cloud"),
        "{}",
        r.err
    );
    assert!(r.err.contains("losos::password"), "{}", r.err);
    let r = s.client.run(&["git", "setup"], "not-the-password");
    assert_eq!(r.code, 1);
    assert_eq!(s.keychain.len(), before, "nothing was stored");
    assert!(!s.home.path().join("config/gtk-3.0/bookmarks").exists());
    assert!(!s.home.path().join("gitconfig").exists());
}

#[test]
fn git_setup_makes_this_program_git_s_credential_helper_for_the_box() {
    let s = logged_in();
    let r = s
        .client
        .run(&["git", "setup", "--device", "laptop"], PASSWORD);
    assert_eq!(r.code, 0, "{}", r.err);
    let git_url = format!("{}/git", s.host.url);
    assert!(
        r.out
            .contains(&format!("git clone {git_url}/notshared/<repository>.git")),
        "{}",
        r.out
    );
    let sent = s.seen.lock().unwrap().last().cloned().unwrap();
    assert_eq!(sent["device"], "laptop");

    let home = s.home.path();
    assert_eq!(
        git_config(home, &format!("credential.{git_url}.helper")),
        vec![
            String::new(),
            "!/opt/losos/bin/losos git credential".to_string()
        ]
    );
    assert_eq!(
        git_config(home, &format!("credential.{git_url}.username")),
        vec!["notshared"]
    );
    // The box's own certificate: git checks its key, as this client checks
    // the certificate, whatever name the box was reached by.
    assert_eq!(
        git_config(home, &format!("http.{git_url}.pinnedPubkey")),
        vec![pinned_public_key(&s.cert.der).unwrap()]
    );
    assert_eq!(
        git_config(home, &format!("http.{git_url}.sslVerify")),
        vec!["false"]
    );

    // git asks the helper, the helper asks the keychain.
    let authority = s.host.url.trim_start_matches("https://").to_string();
    let r = s.client.run(
        &["git", "credential", "get"],
        &format!("protocol=https\nhost={authority}\npath=git/notshared/notes.git\n"),
    );
    assert_eq!(r.code, 0, "{}", r.err);
    assert_eq!(r.out, "username=notshared\npassword=9f8e7d6c5b4a\n");
    // Another host gets nothing, and store and erase are taken quietly.
    let r = s.client.run(
        &["git", "credential", "get"],
        "protocol=https\nhost=other.example\n",
    );
    assert_eq!((r.code, r.out.as_str()), (0, ""));
    let r = s
        .client
        .run(&["git", "credential", "erase"], "protocol=https\n");
    assert_eq!((r.code, r.out.as_str()), (0, ""));

    // Setting up again leaves one helper, not two.
    let r = s.client.run(&["git", "setup"], PASSWORD);
    assert_eq!(r.code, 0, "{}", r.err);
    assert_eq!(
        git_config(home, &format!("credential.{git_url}.helper")).len(),
        2
    );
}

#[test]
fn the_pinned_key_is_the_certificate_s_own() {
    let cert = Cert::new();
    assert_eq!(subject_public_key_info(&cert.der), Some(&cert.spki[..]));
    let pin = pinned_public_key(&cert.der).unwrap();
    assert!(
        pin.starts_with("sha256//") && pin.len() == "sha256//".len() + 44,
        "{pin}"
    );
    assert_eq!(subject_public_key_info(b"\x30\x03\x02\x01"), None);
}
