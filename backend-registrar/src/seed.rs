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
