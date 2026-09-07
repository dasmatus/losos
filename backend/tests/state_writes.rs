//! What the appliance's own files are worth when two writers meet.
//!
//! The box has no SSH and no shell logins, so a config left truncated by a
//! crash or by an interleaved write is not a bug someone logs in to fix — it is
//! a box that fails every rebuild from then on, including the nightly
//! auto-upgrade. These tests hold `atomic_write` and the job-id generator to
//! that standard.

use assert_fs::TempDir;
use losos_ctl::io_backend::{atomic_write, IoLosos, Paths};
use losos_ctl::losos::Losos;
use std::collections::HashSet;

/// Payload size per writer. Big enough that a write is not one instruction, so
/// two writers sharing a temp file have a wide window to land on each other.
const PAYLOAD_BYTES: usize = 256 * 1024;
/// Writes per writer thread.
const ROUNDS: usize = 24;

fn paths_in(dir: &TempDir) -> Paths {
    Paths {
        state_dir: dir.path().join("state"),
        config_file: dir.path().join("defaults.nix"),
        overrides_file: dir.path().join("overrides.nix"),
        flake_ref: "/etc/nixos#install".to_string(),
    }
}

/// Names in `dir` other than the ones passed as `expected`.
fn strays(dir: &TempDir, expected: &[&str]) -> Vec<String> {
    std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| !expected.contains(&name.as_str()))
        .collect()
}

#[test]
fn concurrent_writers_each_land_a_whole_payload() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("state.json");

    // Four payloads of the same length: only the content tells them apart, so a
    // mixed result is a mixture and not merely a short read.
    let payloads: Vec<Vec<u8>> = (0..4u8).map(|i| vec![b'a' + i; PAYLOAD_BYTES]).collect();

    // No shared flag and nothing waiting on anything: a writer that fails
    // panics, the rest run to completion, and the scope surfaces it at the
    // join. The temp name used to be a fixed `<name>.tmp`, so writers truncated
    // and filled each other's file and renamed it out from under one another —
    // which shows up here as a failed write, deterministically.
    std::thread::scope(|scope| {
        let writers: Vec<_> = payloads
            .iter()
            .map(|payload| {
                let target = &target;
                scope.spawn(move || {
                    for _ in 0..ROUNDS {
                        atomic_write(target, payload).unwrap();
                    }
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap();
        }
    });

    let landed = std::fs::read(&target).unwrap();
    assert!(
        payloads.contains(&landed),
        "the file is not one whole payload: {} bytes, {} distinct byte values",
        landed.len(),
        landed.iter().collect::<HashSet<_>>().len()
    );
    assert!(
        strays(&dir, &["state.json"]).is_empty(),
        "files left behind: {:?}",
        strays(&dir, &["state.json"])
    );
}

#[test]
fn a_write_that_cannot_land_leaves_the_previous_content_intact() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("state.json");
    atomic_write(&target, b"the good state").unwrap();

    // A directory where the file should be: the rename cannot replace it.
    let blocked = dir.path().join("blocked");
    std::fs::create_dir(&blocked).unwrap();
    assert!(atomic_write(&blocked, b"nope").is_err());

    assert_eq!(std::fs::read(&target).unwrap(), b"the good state");
    let left = strays(&dir, &["state.json", "blocked"]);
    assert!(left.is_empty(), "temp file left behind: {left:?}");
}

#[test]
fn job_ids_queued_in_the_same_second_are_distinct() {
    let dir = TempDir::new().unwrap();
    let mut backend = IoLosos::new(paths_in(&dir));

    let ids: Vec<String> = (0..64).map(|_| backend.next_job_id().unwrap()).collect();

    let unique: HashSet<&String> = ids.iter().collect();
    assert_eq!(
        unique.len(),
        ids.len(),
        "duplicate job id: two rebuilds would fight over one systemd unit name"
    );

    // ...and they were genuinely in the same second, so it is the counter doing
    // the work rather than the clock. "%Y%m%d%H%M%S" is 14 characters.
    let seconds: HashSet<&str> = ids.iter().map(|id| &id[..14]).collect();
    assert!(
        seconds.len() < ids.len(),
        "the clock separated every id; this test proved nothing"
    );
}

#[test]
fn job_ids_are_shared_across_clones_of_one_backend() {
    let dir = TempDir::new().unwrap();
    let backend = IoLosos::new(paths_in(&dir));
    // The transports each hold a clone; a per-instance counter would hand the
    // same id to a bus call and an HTTP call.
    let mut one = backend.clone();
    let mut two = backend.clone();
    assert_ne!(one.next_job_id().unwrap(), two.next_job_id().unwrap());
}

#[test]
fn defaults_nix_is_rewritten_whole_or_not_at_all() {
    let dir = TempDir::new().unwrap();
    let paths = paths_in(&dir);
    std::fs::write(
        &paths.config_file,
        "{ ... }:\n{\n  losos.sharingMyStorage = true;\n}\n",
    )
    .unwrap();
    let mut backend = IoLosos::new(paths.clone());

    backend.rewrite_config(false).unwrap();

    let out = std::fs::read_to_string(&paths.config_file).unwrap();
    assert!(out.contains("losos.sharingMyStorage = false;"), "{out}");
    // The rewrite goes through a temp file and a rename, like every other write
    // here, so nothing is left half-written next to it.
    let left = strays(&dir, &["defaults.nix"]);
    assert!(left.is_empty(), "files left behind: {left:?}");
}

#[test]
fn a_missing_defaults_nix_is_a_warning_but_an_unreadable_one_is_not() {
    let dir = TempDir::new().unwrap();
    let mut backend = IoLosos::new(paths_in(&dir));
    // Absent: the appliance may be running from a flake laid out differently.
    assert!(backend.rewrite_config(true).is_ok());

    // A directory in its place stands in for any read error that is not
    // "absent". Reporting a mode change that never reached the disk is worse
    // than failing.
    let paths = paths_in(&dir);
    std::fs::create_dir(&paths.config_file).unwrap();
    let mut backend = IoLosos::new(paths);
    assert!(backend.rewrite_config(true).is_err());
}

#[test]
fn settings_never_paper_over_an_unreadable_overrides_file() {
    let dir = TempDir::new().unwrap();
    let paths = paths_in(&dir);

    // Absent means a fresh appliance: the committed defaults are the answer.
    let mut backend = IoLosos::new(paths.clone());
    assert!(backend
        .read_overrides()
        .unwrap()
        .contains("losos.sharingMyStorage"));

    // A read error must not look like "absent". If it did, the settings page
    // would paint defaults the box is not running and the next Apply would
    // write them over the real config.
    std::fs::create_dir(&paths.overrides_file).unwrap();
    let mut backend = IoLosos::new(paths);
    assert!(backend.read_overrides().is_err());
}
