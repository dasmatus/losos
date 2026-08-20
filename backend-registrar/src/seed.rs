//! `losos-registrar seed` — the declarative-boot rathole config seed.
//!
//! Writes the rathole server config's zero-tenant `[server]` base so rathole
//! can start before `serve`'s reconciler has run once. Idempotent: if
//! `rathole_config` already exists this is a no-op — after first boot the
//! running `serve` reconciler is the file's sole writer.
//!
//! Reuses [`crate::config::desired_config`] with an empty tenant slice — the
//! exact function `serve`'s reconciler calls (see `server::reconcile_once`)
//! — so the seed can never drift from the reconciler's zero-tenant output;
//! there is no second rendering to keep in sync. This subcommand replaces
//! the hand-written shell heredoc `modules/edge.nix` used to seed the same
//! file, which had to be kept byte-identical to this crate's output by hand.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use miette::{Context, IntoDiagnostic, Result};

use crate::config::{desired_config, EdgeOpts};
use crate::opts::SeedOpts;

use crate::server::RATHOLE_FILE_MODE;

/// Parent directory mode: owner-only, matching the declarative seed this
/// subcommand replaces (`install -d -m 0700`).
const RATHOLE_DIR_MODE: u32 = 0o700;

pub async fn run(opts: SeedOpts) -> Result<()> {
    let path = Path::new(&opts.rathole_config);

    // Idempotent: after first boot the running `serve` reconciler owns this
    // file, so a pre-existing file (of any content) is left untouched.
    match tokio::fs::metadata(path).await {
        Ok(_) => return Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(e)
                .into_diagnostic()
                .with_context(|| format!("stat {}", path.display()))
        }
    }

    let bootstrap = tokio::fs::read_to_string(&opts.bootstrap_token_file)
        .await
        .into_diagnostic()
        .with_context(|| format!("read bootstrap token file {}", opts.bootstrap_token_file))?;

    // Same `desired_config` call the reconciler makes for the zero-tenant
    // case (see `server::reconcile_once`), so this is wired to produce
    // exactly the reconciler's first-run output, not a parallel rendering.
    let edge_opts = EdgeOpts {
        rathole_bind_addr: opts.rathole_bind_addr,
        rathole_bind_port: opts.rathole_bind_port,
        bootstrap_token: bootstrap.trim().to_string(),
    };
    let files = desired_config(&[], &edge_opts);

    if let Some(parent) = path.parent() {
        let parent_exists = match tokio::fs::metadata(parent).await {
            Ok(_) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => {
                return Err(e)
                    .into_diagnostic()
                    .with_context(|| format!("stat {}", parent.display()))
            }
        };
        if !parent_exists {
            tokio::fs::create_dir_all(parent)
                .await
                .into_diagnostic()
                .with_context(|| format!("create {}", parent.display()))?;
            tokio::fs::set_permissions(parent, std::fs::Permissions::from_mode(RATHOLE_DIR_MODE))
                .await
                .into_diagnostic()
                .with_context(|| format!("chmod {}", parent.display()))?;
        }
    }

    crate::fsutil::atomic_write(path, files.rathole_toml.as_bytes(), RATHOLE_FILE_MODE)
        .await
        .into_diagnostic()
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// A fresh, unique directory under the system tmp dir so parallel test
    /// threads never collide.
    fn unique_tmp_dir(tag: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "losos-registrar-seed-test-{tag}-{}-{nanos}",
            std::process::id()
        ))
    }

    /// Pins the wiring, not the bytes: `run` must produce exactly what
    /// `desired_config` (the same function `serve`'s reconciler calls) would
    /// render for the equivalent `EdgeOpts` and zero tenants. If a future
    /// edit made `seed` render its own TOML instead of calling
    /// `desired_config`, this test — not a byte-literal — would catch the
    /// drift.
    #[tokio::test]
    async fn seed_output_matches_reconciler_for_zero_tenants() {
        let dir = unique_tmp_dir("match");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let token_path = dir.join("bootstrap-token");
        tokio::fs::write(&token_path, b"BOOT\n").await.unwrap();
        let rathole_path = dir.join("rathole").join("server.toml");

        let opts = SeedOpts {
            rathole_config: rathole_path.to_string_lossy().into_owned(),
            rathole_bind_addr: "0.0.0.0".to_string(),
            rathole_bind_port: 2333,
            bootstrap_token_file: token_path.to_string_lossy().into_owned(),
        };
        run(opts).await.expect("seed succeeds on a fresh path");

        let written = tokio::fs::read_to_string(&rathole_path).await.unwrap();
        let expected = desired_config(
            &[],
            &EdgeOpts {
                rathole_bind_addr: "0.0.0.0".to_string(),
                rathole_bind_port: 2333,
                bootstrap_token: "BOOT".to_string(),
            },
        )
        .rathole_toml;
        assert_eq!(written, expected);

        let file_mode = tokio::fs::metadata(&rathole_path)
            .await
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600);
        let dir_mode = tokio::fs::metadata(rathole_path.parent().unwrap())
            .await
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700);

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn seed_refuses_to_overwrite_existing_file() {
        let dir = unique_tmp_dir("noclobber");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let token_path = dir.join("bootstrap-token");
        tokio::fs::write(&token_path, b"BOOT\n").await.unwrap();
        let rathole_path = dir.join("server.toml");
        let sentinel = b"# owned by the running registrar\n".as_slice();
        tokio::fs::write(&rathole_path, sentinel).await.unwrap();

        let opts = SeedOpts {
            rathole_config: rathole_path.to_string_lossy().into_owned(),
            rathole_bind_addr: "0.0.0.0".to_string(),
            rathole_bind_port: 2333,
            bootstrap_token_file: token_path.to_string_lossy().into_owned(),
        };
        run(opts)
            .await
            .expect("an existing file is a no-op, not an error");

        let content = tokio::fs::read(&rathole_path).await.unwrap();
        assert_eq!(content, sentinel);

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
}
