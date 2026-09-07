//! `losos-registrar seed` — the first-boot rathole config.
//!
//! rathole must be able to start before the registrar's reconciler has run
//! once, so the seed writes the zero-tenant `[server]` base. It calls the same
//! `desired_config` the reconciler calls, which is the property worth pinning:
//! a byte-literal here would let the two drift apart and only show up as a
//! spurious rewrite (and tunnel churn) on every edge's first reconcile.

mod common;

use std::os::unix::fs::PermissionsExt;

use common::TempDir;
use losos_registrar::opts::SeedOpts;
use losos_registrar::{desired_config, EdgeOpts};

const BOOTSTRAP: &str = "b00757241pb00757241pb00757241pb00757241pb00757241pb00757241pb007";

fn seed_opts(dir: &TempDir, rathole_config: &str) -> SeedOpts {
    SeedOpts {
        rathole_config: rathole_config.to_string(),
        rathole_bind_addr: "0.0.0.0".to_string(),
        rathole_bind_port: 2333,
        bootstrap_token_file: dir.path_str("bootstrap.token"),
    }
}

#[tokio::test]
async fn the_seed_matches_what_the_reconciler_would_write() {
    let dir = TempDir::new("seed-match");
    std::fs::write(dir.join("bootstrap.token"), format!("{BOOTSTRAP}\n"))
        .expect("write bootstrap token");
    let rathole_path = dir.join("rathole").join("server.toml");

    losos_registrar::seed::run(seed_opts(&dir, &rathole_path.to_string_lossy()))
        .await
        .expect("seed succeeds on a fresh path");

    let written = std::fs::read_to_string(&rathole_path).expect("seed wrote server.toml");
    let expected = desired_config(
        &[],
        &EdgeOpts {
            rathole_bind_addr: "0.0.0.0".to_string(),
            rathole_bind_port: 2333,
            bootstrap_token: BOOTSTRAP.to_string(),
        },
    )
    .rathole_toml;
    assert_eq!(written, expected);
}

/// `server.toml` carries every tenant's tunnel token, so the seed must land it
/// 0600 inside a 0700 directory regardless of the unit's umask.
#[tokio::test]
async fn the_seed_writes_owner_only_permissions() {
    let dir = TempDir::new("seed-perms");
    std::fs::write(dir.join("bootstrap.token"), BOOTSTRAP).expect("write bootstrap token");
    let rathole_path = dir.join("rathole").join("server.toml");

    losos_registrar::seed::run(seed_opts(&dir, &rathole_path.to_string_lossy()))
        .await
        .expect("seed succeeds");

    let file_mode = std::fs::metadata(&rathole_path)
        .expect("stat server.toml")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(file_mode, 0o600, "server.toml is world-readable");
    let dir_mode = std::fs::metadata(rathole_path.parent().expect("parent dir"))
        .expect("stat rathole dir")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(dir_mode, 0o700);
}

/// After first boot the running `serve` reconciler owns the file, so the seed
/// must never clobber it — including on a restart, where the seed unit runs
/// again before the registrar.
#[tokio::test]
async fn the_seed_never_overwrites_an_existing_file() {
    let dir = TempDir::new("seed-noclobber");
    std::fs::write(dir.join("bootstrap.token"), BOOTSTRAP).expect("write bootstrap token");
    let rathole_path = dir.join("server.toml");
    let sentinel = "# owned by the running registrar\n";
    std::fs::write(&rathole_path, sentinel).expect("pre-place server.toml");

    losos_registrar::seed::run(seed_opts(&dir, &rathole_path.to_string_lossy()))
        .await
        .expect("an existing file is a no-op, not an error");

    assert_eq!(
        std::fs::read_to_string(&rathole_path).expect("read server.toml"),
        sentinel
    );
}

/// A missing bootstrap token is fatal here: rathole's `[server]` block cannot
/// be rendered without one, and starting rathole with a wrong `default_token`
/// is worse than not starting it.
#[tokio::test]
async fn a_missing_bootstrap_token_fails_the_seed() {
    let dir = TempDir::new("seed-nobootstrap");
    let rathole_path = dir.join("server.toml");
    let result = losos_registrar::seed::run(seed_opts(&dir, &rathole_path.to_string_lossy())).await;
    assert!(result.is_err(), "seed must not invent a bootstrap token");
    assert!(!rathole_path.exists(), "a failed seed must write nothing");
}
