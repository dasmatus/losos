//! Atomic file-write helper shared by the registry store and the config
//! writer.
//!
//! The temp + fsync + rename sequence is a *filesystem* concern, not an async
//! one, so it runs on `spawn_blocking` with `std::fs`. A concurrent reader or a
//! crash (or a future cancelled at an `.await`) sees either the old complete
//! file or the new one — never a torn or missing path.

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Atomically write `bytes` to `path` with filesystem mode `mode`.
///
/// Writes a sibling temp file (same directory, so the rename is atomic on the
/// same filesystem), flushes + fsyncs it, sets the mode explicitly (beats
/// umask), then renames over the target. The temp file uses `with_extension`
/// so it lives next to the target — never on a different mount.
pub(crate) async fn atomic_write(
    path: &Path,
    bytes: &[u8],
    mode: u32,
) -> std::io::Result<()> {
    let path: PathBuf = path.to_path_buf();
    let bytes = bytes.to_vec();
    // Ownership moves into the blocking task; no async guard crosses the await.
    tokio::task::spawn_blocking(move || atomic_write_sync(&path, &bytes, mode))
        .await
        .map_err(std::io::Error::other)?
}

fn atomic_write_sync(path: &Path, bytes: &[u8], mode: u32) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp)?;
    // Explicit mode: don't let the service umask widen 0600 -> 0644 on a
    // file that carries tokens.
    std::fs::set_permissions(&tmp, PermissionsExt::from_mode(mode))?;
    f.write_all(bytes)?;
    f.flush()?;
    f.sync_all()?;
    drop(f);
    std::fs::rename(&tmp, path)?;
    Ok(())
}