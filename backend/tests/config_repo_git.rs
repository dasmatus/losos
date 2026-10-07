//! The configuration repository against a real `git`.
//!
//! `backend/src/losos.rs` tests the sync decision over the fake's two-sided
//! history; this file holds the real [`Losos`] implementation to the same
//! contract with an actual repository, a bare remote and a clone that pushes
//! — the arrangement the box and LosOS Git are in. No Forgejo: the remote is
//! a path, which is the `$LOSOS_CONFIG_REMOTE_URL` seam `tests/admin-vm.nix`
//! uses too.

use assert_fs::TempDir;
use losos_ctl::io_backend::{IoLosos, Paths};
use losos_ctl::losos::Losos;
use std::path::{Path, PathBuf};
use std::process::Command;

/// `git` in `dir`, with an identity, asserting success.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=test", "-c", "user.email=test@localhost"])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// An appliance whose `/etc/nixos` is a one-commit repository on `main`, and
/// an empty bare remote beside it.
fn appliance(dir: &TempDir) -> (IoLosos, PathBuf) {
    let config_dir = dir.path().join("etc-nixos");
    std::fs::create_dir_all(config_dir.join("modules")).unwrap();
    std::fs::write(
        config_dir.join("modules/overrides.nix"),
        losos_ctl::overrides::DEFAULT_OVERRIDES_NIX,
    )
    .unwrap();
    std::fs::write(config_dir.join("flake.nix"), "{ }\n").unwrap();
    git(&config_dir, &["init", "-q", "-b", "main"]);
    git(&config_dir, &["add", "-A"]);
    git(&config_dir, &["commit", "-q", "-m", "losos install"]);

    let bare = dir.path().join("remote.git");
    git(
        dir.path(),
        &["init", "-q", "--bare", "-b", "main", "remote.git"],
    );

    let paths = Paths {
        state_dir: dir.path().join("state"),
        overrides_file: config_dir.join("modules/overrides.nix"),
        flake_ref: "/etc/nixos#install".to_string(),
        config_dir,
        options_file: dir.path().join("no-such-options.json"),
    };
    (IoLosos::new(paths), bare)
}

#[test]
fn commits_fetches_pushes_and_fast_forwards_like_the_fake_says() {
    let dir = TempDir::new().unwrap();
    let (mut l, bare) = appliance(&dir);
    let bare_url = bare.to_string_lossy().to_string();

    let head = l.config_head().unwrap().expect("the installer committed");
    assert_eq!(head.branch, "main");
    assert_eq!(head.sha.len(), 40);

    // A clean tree has nothing to commit.
    assert_eq!(l.config_commit("nothing", "").unwrap(), None);

    // An Apply writes the file; the commit carries subject and body.
    l.write_overrides("{ ... }:\n{\n  losos.hostName = \"box2\";\n}\n")
        .unwrap();
    let sha = l
        .config_commit("Change hostName", "losos.hostName: \"mattbox\" -> \"box2\"")
        .unwrap()
        .expect("a change was committed");
    assert_eq!(l.config_head().unwrap().unwrap().sha, sha);
    let log = l.config_log(5).unwrap();
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].sha, sha);
    assert_eq!(log[0].subject, "Change hostName");
    assert!(log[0].when.contains('T'), "RFC 3339: {}", log[0].when);
    assert_eq!(log[1].subject, "losos install");
    let body = git(&l.paths.config_dir, &["log", "-1", "--format=%b"]);
    assert_eq!(body, "losos.hostName: \"mattbox\" -> \"box2\"");

    // An empty remote has no branch; the first push makes it.
    assert_eq!(l.config_fetch(&bare_url, "main").unwrap(), None);
    l.config_push(&bare_url, "main").unwrap();
    assert_eq!(
        l.config_fetch(&bare_url, "main").unwrap(),
        Some(sha.clone())
    );

    // A clone pushes a commit on top: the box's head is in its history.
    let clone = dir.path().join("clone");
    git(
        dir.path(),
        &["clone", "-q", &bare_url, &clone.to_string_lossy()],
    );
    std::fs::write(
        clone.join("modules/overrides.nix"),
        "{ ... }:\n{\n  losos.hostName = \"pushed\";\n}\n",
    )
    .unwrap();
    git(&clone, &["commit", "-qam", "pushed from a laptop"]);
    git(&clone, &["push", "-q", "origin", "main"]);

    let remote = l.config_fetch(&bare_url, "main").unwrap().unwrap();
    assert_ne!(remote, sha);
    assert!(l.config_is_ancestor(&sha, &remote).unwrap());
    assert!(!l.config_is_ancestor(&remote, &sha).unwrap());
    assert_eq!(
        l.config_show(&remote, "modules/overrides.nix")
            .unwrap()
            .as_deref(),
        Some("{ ... }:\n{\n  losos.hostName = \"pushed\";\n}\n")
    );
    assert_eq!(l.config_show(&remote, "modules/absent.nix").unwrap(), None);

    // Taking it moves the branch and the working tree, which is what the
    // rebuild and `settings` then read.
    l.config_fast_forward(&remote).unwrap();
    assert_eq!(l.config_head().unwrap().unwrap().sha, remote);
    assert!(l.read_overrides().unwrap().contains("\"pushed\""));
    assert_eq!(l.config_commit("nothing", "").unwrap(), None);
    assert_eq!(l.config_log(1).unwrap()[0].subject, "pushed from a laptop");
}

#[test]
fn a_remote_that_cannot_be_reached_is_not_up_rather_than_a_fault() {
    let dir = TempDir::new().unwrap();
    let (mut l, _) = appliance(&dir);
    let err = l
        .config_fetch(&dir.path().join("absent.git").to_string_lossy(), "main")
        .unwrap_err();
    assert!(
        err.downcast_ref::<losos_ctl::config_repo::NotUp>()
            .is_some(),
        "{err:#}"
    );
}

#[test]
fn a_directory_that_is_no_repository_has_no_head_and_an_empty_log() {
    let dir = TempDir::new().unwrap();
    let plain = dir.path().join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    let mut l = IoLosos::new(Paths {
        state_dir: dir.path().join("state"),
        overrides_file: plain.join("overrides.nix"),
        flake_ref: "/etc/nixos#install".to_string(),
        config_dir: plain,
        options_file: dir.path().join("no-such-options.json"),
    });
    assert_eq!(l.config_head().unwrap(), None);
    assert!(l.config_log(5).unwrap().is_empty());
    // And a box without the option document has none, rather than an error.
    assert!(l.read_options_doc().unwrap().is_none());
}

#[test]
fn the_sync_report_round_trips_through_its_file() {
    let dir = TempDir::new().unwrap();
    let (mut l, _) = appliance(&dir);
    assert_eq!(l.load_sync_report().unwrap(), None);
    let report = losos_ctl::config_repo::SyncReport::new("ok", "Pushed.");
    l.save_sync_report(&report).unwrap();
    assert_eq!(l.load_sync_report().unwrap(), Some(report));
}
