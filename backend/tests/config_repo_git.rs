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

/// The published repository: a work tree with a flake, the default
/// overrides, and one more file, on `main`.
fn published(dir: &TempDir) -> (PathBuf, String) {
    let src = dir.path().join("published");
    std::fs::create_dir_all(src.join("modules")).unwrap();
    std::fs::write(src.join("flake.nix"), "{ version = 1; }\n").unwrap();
    std::fs::write(
        src.join("modules/overrides.nix"),
        losos_ctl::overrides::DEFAULT_OVERRIDES_NIX,
    )
    .unwrap();
    std::fs::write(src.join("modules/options.nix"), "{ v1 = true; }\n").unwrap();
    git(&src, &["init", "-q", "-b", "main"]);
    git(&src, &["add", "-A"]);
    git(&src, &["commit", "-q", "-m", "published v1"]);
    let url = format!("file://{}", src.display());
    (src, url)
}

fn tracked(dir: &Path) -> Vec<String> {
    git(dir, &["ls-files"])
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn a_follow_takes_the_published_tree_and_keeps_the_boxs_own_files() {
    let dir = TempDir::new().unwrap();
    let (mut l, _) = appliance(&dir);
    let cfg = l.paths.config_dir.clone();
    // What the installer writes beside the published tree.
    std::fs::write(cfg.join("modules/install-target.nix"), "{ drives = 1; }\n").unwrap();
    git(&cfg, &["add", "-A"]);
    git(&cfg, &["commit", "-q", "-m", "install target"]);
    // An Apply.
    let applied = "{ ... }:\n{\n  losos.hostName = \"box2\";\n}\n";
    l.write_overrides(applied).unwrap();
    l.config_commit("Change hostName", "").unwrap();
    let before = l.config_head().unwrap().unwrap().sha;

    let (src, url) = published(&dir);
    let up = losos_ctl::config_repo::Upstream {
        url,
        branch: "main".into(),
    };
    assert_eq!(l.upstream_mark().unwrap(), None);
    // Nothing outside the box's own files changed since the install.
    assert!(!l.config_edited_outside(None).unwrap());

    let fetched = l.upstream_fetch(&up).unwrap();
    assert_eq!(fetched, git(&src, &["rev-parse", "HEAD"]));
    let made = l
        .config_follow(&fetched, "Update to published main", "Upstream: x")
        .unwrap()
        .expect("the tree changed");

    let head = l.config_head().unwrap().unwrap();
    assert_eq!(head.sha, made);
    assert_eq!(head.branch, "main");
    // One new commit on the box's own history, not a merge.
    assert_eq!(git(&cfg, &["rev-parse", "HEAD^"]), before);
    assert_eq!(
        l.config_log(1).unwrap()[0].subject,
        "Update to published main"
    );
    // The published files arrived, in the working tree too.
    assert_eq!(
        std::fs::read_to_string(cfg.join("flake.nix")).unwrap(),
        "{ version = 1; }\n"
    );
    assert!(cfg.join("modules/options.nix").exists());
    // The box's own two files are the box's.
    assert_eq!(l.read_overrides().unwrap(), applied);
    assert_eq!(
        std::fs::read_to_string(cfg.join("modules/install-target.nix")).unwrap(),
        "{ drives = 1; }\n"
    );
    assert_eq!(
        tracked(&cfg),
        [
            "flake.nix",
            "modules/install-target.nix",
            "modules/options.nix",
            "modules/overrides.nix"
        ]
    );
    // Clean: nothing for the next commit to pick up.
    assert_eq!(l.config_commit("nothing", "").unwrap(), None);
    assert_eq!(
        l.upstream_mark().unwrap(),
        Some(losos_ctl::config_repo::FollowMark {
            upstream: fetched.clone(),
            followed: made.clone(),
        })
    );
    assert!(!l.config_edited_outside(Some(&made)).unwrap());

    // The same published commit again: no new commit.
    assert_eq!(l.config_follow(&fetched, "again", "").unwrap(), None);
    assert_eq!(l.config_head().unwrap().unwrap().sha, made);

    // The project moves on, removing a file and changing overrides.nix;
    // the box takes the first and keeps its own copy of the second.
    git(&src, &["rm", "-q", "modules/options.nix"]);
    std::fs::write(src.join("flake.nix"), "{ version = 2; }\n").unwrap();
    std::fs::write(src.join("modules/overrides.nix"), "{ published = 2; }\n").unwrap();
    git(&src, &["commit", "-qam", "published v2"]);
    // After an Apply on the box in the meantime.
    let applied2 = "{ ... }:\n{\n  losos.hostName = \"box3\";\n}\n";
    l.write_overrides(applied2).unwrap();
    l.config_commit("Change hostName", "").unwrap();
    assert!(!l.config_edited_outside(Some(&made)).unwrap());

    let fetched2 = l.upstream_fetch(&up).unwrap();
    assert_ne!(fetched2, fetched);
    l.config_follow(&fetched2, "Update to published main", "")
        .unwrap()
        .expect("the tree changed");
    assert_eq!(
        std::fs::read_to_string(cfg.join("flake.nix")).unwrap(),
        "{ version = 2; }\n"
    );
    assert!(!cfg.join("modules/options.nix").exists());
    assert_eq!(l.read_overrides().unwrap(), applied2);
}

#[test]
fn an_edit_outside_the_boxs_own_files_is_seen() {
    let dir = TempDir::new().unwrap();
    let (mut l, _) = appliance(&dir);
    let cfg = l.paths.config_dir.clone();
    // An edit to the box's own file is not one.
    l.write_overrides("{ x = 1; }\n").unwrap();
    l.config_commit("Change", "").unwrap();
    assert!(!l.config_edited_outside(None).unwrap());
    // A pushed edit to the flake is.
    std::fs::write(cfg.join("flake.nix"), "{ mine = true; }\n").unwrap();
    l.config_commit("pushed from a laptop", "").unwrap();
    assert!(l.config_edited_outside(None).unwrap());
}

#[test]
fn a_published_branch_that_cannot_be_fetched_is_not_up() {
    let dir = TempDir::new().unwrap();
    let (mut l, _) = appliance(&dir);
    let up = losos_ctl::config_repo::Upstream {
        url: format!("file://{}", dir.path().join("absent.git").display()),
        branch: "main".into(),
    };
    let err = l.upstream_fetch(&up).unwrap_err();
    assert!(
        err.downcast_ref::<losos_ctl::config_repo::NotUp>()
            .is_some(),
        "{err:#}"
    );
    assert_eq!(l.load_upstream_report().unwrap(), None);
    let report = losos_ctl::config_repo::SyncReport::new("unavailable", "no network");
    l.save_upstream_report(&report).unwrap();
    assert_eq!(l.load_upstream_report().unwrap(), Some(report));
}
