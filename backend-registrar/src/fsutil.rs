//! Atomic file-write helper shared by the registry store and the config
//! writer.
//!
//! The temp + fsync + rename sequence is a *filesystem* concern, not an async
//! one, so it runs on `spawn_blocking` with `std::fs`. A concurrent reader or a
//! crash (or a future cancelled at an `.await`) sees either the old complete
//! file or the new one — never a torn or missing path.
//!
//! The temp name is unique per write (`.{stem}.{pid}.{counter}.tmp`) and
//! opened `create_new`, so two writers racing the same target never share a
//! scratch file. The earlier `path.with_extension("tmp")` gave every writer of
//! `registry.json` the same `registry.tmp` opened `truncate` — one writer
//! could rename another's half-written buffer over the target. Callers should
//! still serialise their own writes (see `Registry::persist`); this is the
//! second line of defence, not the first.

use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Distinguishes concurrent writes from the same process. The pid
/// distinguishes writes from different processes.
static TMP_CTR: AtomicU64 = AtomicU64::new(0);

/// Atomically write `bytes` to `path` with filesystem mode `mode`.
///
/// Writes a uniquely-named sibling temp file (same directory, so the rename is
/// atomic on the same filesystem), fsyncs it, renames over the target, then
/// fsyncs the *directory* so the rename itself survives a power cut — an
/// fsync of the file alone does not make its new name durable.
pub(crate) async fn atomic_write(path: &Path, bytes: &[u8], mode: u32) -> std::io::Result<()> {
    let path: PathBuf = path.to_path_buf();
    let bytes = bytes.to_vec();
    // Ownership moves into the blocking task; no async guard crosses the await.
    tokio::task::spawn_blocking(move || atomic_write_sync(&path, &bytes, mode))
        .await
        .map_err(std::io::Error::other)?
}

fn atomic_write_sync(path: &Path, bytes: &[u8], mode: u32) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let (tmp, mut f) = create_temp(path, dir, mode)?;

    // The temp file is unlinked on any failure below so a crashed write does
    // not leave scratch files (with token content) lying next to the target.
    let write = (|| -> std::io::Result<()> {
        // Explicit mode: `.mode()` above is masked by the process umask, which
        // can only clear bits — this restores the intended mode (e.g. 0644 for
        // the Traefik config under a 0077 umask).
        std::fs::set_permissions(&tmp, PermissionsExt::from_mode(mode))?;
        f.write_all(bytes)?;
        f.flush()?;
        f.sync_all()
    })();
    drop(f);
    if let Err(e) = write {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    // Durability of the rename is a property of the directory, not the file.
    // A missing/unopenable directory here is not worth failing an otherwise
    // complete write over — the data is already fsynced and in place.
    if let Ok(d) = std::fs::File::open(dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

/// Open a fresh temp file next to `path`. `create_new` means an existing name
/// is never reused, so a racing writer's scratch file is never clobbered; the
/// counter advances until an unused name is found.
fn create_temp(path: &Path, dir: &Path, mode: u32) -> std::io::Result<(PathBuf, std::fs::File)> {
    let stem = path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("losos");
    let pid = std::process::id();
    loop {
        let n = TMP_CTR.fetch_add(1, Ordering::Relaxed);
        let tmp = dir.join(format!(".{stem}.{pid}.{n}.tmp"));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&tmp)
        {
            Ok(f) => return Ok((tmp, f)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
}
