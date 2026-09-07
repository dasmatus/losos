//! The real [`Losos`] implementation: files, environment, `systemd-run`.
//!
//! Every path is environment-overridable so the CLI can be driven against a
//! throwaway directory in tests without touching `/etc` or `/var`.
//!
//! Every file this module owns is written through [`atomic_write`] — a unique
//! temp file, fsynced, then renamed over the target. `state.json`,
//! `overrides.nix` and `defaults.nix` all take that route: the appliance has no
//! shell, so a config truncated by a crash or by two concurrent writers would
//! fail every later rebuild with nobody able to log in and repair it.

use crate::losos::Losos;
use crate::model::State;
use crate::overrides::{inject_line, DEFAULT_OVERRIDES_NIX};
use crate::supervisor;
use anyhow::Context;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// Where everything lives. Resolved once from the environment.
#[derive(Debug, Clone)]
pub struct Paths {
    /// `$LOSOS_STATE_DIR`, holding `state.json` and `rebuild.log`.
    pub state_dir: PathBuf,
    /// `$LOSOS_CONFIG` — patched by `change --mode`.
    pub config_file: PathBuf,
    /// `$LOSOS_OVERRIDES` — replaced by `apply` / `factory-reset`.
    pub overrides_file: PathBuf,
    /// `$LOSOS_FLAKE`, the flake reference rebuilds are made from.
    pub flake_ref: String,
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

impl Paths {
    /// Resolve from the environment, falling back to the appliance defaults.
    pub fn from_env() -> Self {
        Paths {
            state_dir: PathBuf::from(env_or("LOSOS_STATE_DIR", "/var/lib/losos")),
            config_file: PathBuf::from(env_or("LOSOS_CONFIG", "/etc/nixos/defaults.nix")),
            overrides_file: PathBuf::from(env_or(
                "LOSOS_OVERRIDES",
                "/etc/nixos/modules/overrides.nix",
            )),
            flake_ref: env_or("LOSOS_FLAKE", "/etc/nixos#install"),
        }
    }

    pub fn state_file(&self) -> PathBuf {
        self.state_dir.join("state.json")
    }

    pub fn rebuild_log(&self) -> PathBuf {
        self.state_dir.join("rebuild.log")
    }
}

/// Distinguishes the temp files of concurrent writers within one process.
static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);
/// Distinguishes rebuild jobs queued within the same second. Process-lifetime,
/// so it survives every clone of [`IoLosos`].
static JOB_SEQ: AtomicU64 = AtomicU64::new(0);

/// A temp path nobody else will pick: the pid separates processes, the counter
/// separates threads within one.
///
/// The old fixed `<name>.tmp` was the bug — two writers truncated and filled
/// the *same* temp file, then renamed it in turn, so the survivor could be a
/// blend of both payloads.
fn temp_path(path: &Path) -> PathBuf {
    let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "losos".to_string());
    path.with_file_name(format!(".{name}.{}.{seq}.tmp", std::process::id()))
}

/// Fill a fresh temp file and flush it to the disk itself.
///
/// `create_new` is what keeps `mode` honest: the file carries its permissions
/// from the moment it exists, instead of being created wide and narrowed after.
fn fill_temp(tmp: &Path, content: &[u8], mode: u32) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(tmp)?;
    file.write_all(content)?;
    // Without this the rename can land before the bytes do, and a power cut
    // between the two leaves a correctly named, empty config.
    file.sync_all()
}

/// Write `content` to `path` atomically, with `mode` on the resulting file and
/// `dir_mode` on any parent directory this call has to create.
fn write_atomically(path: &Path, content: &[u8], mode: u32, dir_mode: u32) -> anyhow::Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(dir_mode)
            .create(dir)
            .with_context(|| format!("creating {}", dir.display()))?;
    }

    let tmp = temp_path(path);
    if let Err(e) = fill_temp(&tmp, content, mode) {
        let _ = std::fs::remove_file(&tmp);
        return Err(anyhow::Error::new(e).context(format!("writing {}", tmp.display())));
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(anyhow::Error::new(e).context(format!(
            "renaming {} to {}",
            tmp.display(),
            path.display()
        )));
    }

    // The rename is only durable once the directory entry is on disk too. A
    // failure here means the new content is live but might not survive a power
    // cut — worth a line in the journal, not worth failing a completed write.
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        match std::fs::File::open(dir).and_then(|d| d.sync_all()) {
            Ok(()) => {}
            Err(e) => {
                tracing::debug!(dir = %dir.display(), error = %e, "could not fsync directory")
            }
        }
    }
    Ok(())
}

/// Write `content` to `path` atomically: a unique temp file, fsynced, then
/// renamed over the target.
///
/// The rename is atomic on POSIX, so a reader either sees the whole old file or
/// the whole new one — never a truncated config, and never a blend of two
/// concurrent writers.
pub fn atomic_write(path: &Path, content: &[u8]) -> anyhow::Result<()> {
    write_atomically(path, content, 0o644, 0o755)
}

/// [`atomic_write`] for a secret: mode 0600 from creation, in a 0700 directory.
///
/// Used for the admin token, which is equivalent to root on this appliance.
/// Creating it world-readable and chmodding afterwards leaves a window in which
/// any local process can read it, and a token that was ever readable is a token
/// to rotate.
pub fn atomic_write_secret(path: &Path, content: &[u8]) -> anyhow::Result<()> {
    write_atomically(path, content, 0o600, 0o700)
}

/// Read the persisted state.
///
/// A missing file means a fresh appliance. A *corrupt* file is treated the same
/// way rather than raising: the admin UI going blank is a worse failure than
/// silently resetting to defaults, and the next write repairs the file.
pub fn read_state(path: &Path) -> State {
    let Ok(bytes) = std::fs::read(path) else {
        return State::default();
    };
    match serde_json::from_slice::<State>(&bytes) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "unreadable state file; using defaults");
            State::default()
        }
    }
}

/// Persist the state atomically.
pub fn write_state(path: &Path, s: &State) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec(s).context("encoding state")?;
    atomic_write(path, &bytes)
}

/// The production backend.
///
/// Every clone shares one lock, and that lock is the whole serialisation story
/// for `state.json`. There used to be two: `dbus` and `http` each built their
/// own, so a D-Bus `Change` and a `POST /api/change` raced, and the rebuild
/// watcher recorded outcomes under no lock at all — which could drop a
/// finished rebuild, or clobber the record of the one that replaced it.
///
/// Cloning is cheap and carries no state: the paths are immutable and the lock
/// is shared, so a clone is a second handle to the same appliance.
#[derive(Debug, Clone)]
pub struct IoLosos {
    pub paths: Paths,
    state_lock: Arc<Mutex<()>>,
}

impl IoLosos {
    /// A backend and a fresh lock. Call this **once** per process and clone the
    /// result; two separately constructed backends do not serialise each other.
    pub fn new(paths: Paths) -> Self {
        IoLosos {
            paths,
            state_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn from_env() -> Self {
        Self::new(Paths::from_env())
    }

    /// Take the state lock.
    ///
    /// Poisoning is recovered from on purpose. The mutex guards no in-memory
    /// invariant — the state is a file, and a corrupt one already reads as the
    /// default — so honouring the poison would turn one panicked command into a
    /// permanently dead admin surface on a box with no shell to repair it.
    pub(crate) fn lock_state(&self) -> MutexGuard<'_, ()> {
        self.state_lock.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("state lock was poisoned by a panicking command; recovering");
            poisoned.into_inner()
        })
    }

    /// Run one command with the state lock held from the first effect to the
    /// last.
    ///
    /// Per-effect locking would not do: a command is a read-modify-write spread
    /// over several trait calls (`load_state`, mutate, `save_state`,
    /// `spawn_rebuild`), and two of them interleaving is exactly the race this
    /// prevents.
    ///
    /// Not reentrant — never call it from inside `f`.
    pub fn serialized<T>(
        &self,
        f: impl FnOnce(&mut IoLosos) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let _guard = self.lock_state();
        let mut backend = self.clone();
        f(&mut backend)
    }
}

impl Default for IoLosos {
    fn default() -> Self {
        Self::from_env()
    }
}

impl Losos for IoLosos {
    fn load_state(&mut self) -> anyhow::Result<State> {
        Ok(read_state(&self.paths.state_file()))
    }

    fn save_state(&mut self, s: &State) -> anyhow::Result<()> {
        write_state(&self.paths.state_file(), s)
    }

    fn rewrite_config(&mut self, sharing: bool) -> anyhow::Result<()> {
        let path = &self.paths.config_file;
        let contents = match std::fs::read_to_string(path) {
            Ok(c) => c,
            // Absent config is a warning, not a failure: the appliance may be
            // running from a flake laid out differently, and refusing to change
            // mode over it would be worse than carrying on.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!(path = %path.display(), "config missing; not rewriting");
                return Ok(());
            }
            // Anything else — a permission error, a bad sector — is a real
            // failure. Carrying on would report a mode change that never
            // reached the disk.
            Err(e) => {
                return Err(anyhow::Error::new(e).context(format!("reading {}", path.display())))
            }
        };
        let lines: Vec<String> = contents.lines().map(str::to_string).collect();
        let mut out = inject_line(sharing, &lines).join("\n");
        out.push('\n');
        // Atomic, like every other file here: a truncated defaults.nix fails
        // every future rebuild, including the nightly auto-upgrade.
        atomic_write(path, out.as_bytes()).with_context(|| format!("rewriting {}", path.display()))
    }

    fn write_overrides(&mut self, body: &str) -> anyhow::Result<()> {
        atomic_write(&self.paths.overrides_file, body.as_bytes())
    }

    fn read_overrides(&mut self) -> anyhow::Result<String> {
        match std::fs::read_to_string(&self.paths.overrides_file) {
            Ok(body) => Ok(body),
            // Absent means a fresh appliance, and the committed defaults are
            // the honest answer. A read *error* is not the same thing: reporting
            // defaults would have the settings page paint values the box is not
            // running, and the next Apply would write them over the real config.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(DEFAULT_OVERRIDES_NIX.to_string())
            }
            Err(e) => Err(anyhow::Error::new(e)
                .context(format!("reading {}", self.paths.overrides_file.display()))),
        }
    }

    fn spawn_rebuild(&mut self, job: &str) -> anyhow::Result<()> {
        supervisor::spawn_rebuild(self, job)
    }

    fn rebuild_log_tail(&mut self) -> anyhow::Result<String> {
        Ok(supervisor::log_tail(&self.paths.rebuild_log()))
    }

    fn next_job_id(&mut self) -> anyhow::Result<String> {
        // The timestamp and the pid are both constant within one second of one
        // long-lived daemon, so they alone let two jobs collide — and a
        // collision means `systemd-run --unit=` fails with "Unit already
        // exists" *after* the command has already rewritten overrides.nix,
        // while `supervisor::finish` can no longer tell the two rebuilds apart.
        let seq = JOB_SEQ.fetch_add(1, Ordering::Relaxed);
        Ok(format!(
            "{}-{}-{}",
            chrono::Utc::now().format("%Y%m%d%H%M%S"),
            std::process::id(),
            seq
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Mode, Rebuild, RebuildState};

    fn tmpdir() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "losos-io-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn atomic_write_creates_parents_and_leaves_no_temp_file() {
        let dir = tmpdir().join("nested/deeper");
        let target = dir.join("state.json");
        atomic_write(&target, b"hello").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "hello");
        let strays: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains("tmp"))
            .collect();
        assert!(strays.is_empty(), "temp file left behind: {strays:?}");
        std::fs::remove_dir_all(tmpdir()).ok();
    }

    #[test]
    fn missing_state_file_reads_as_default() {
        let p = tmpdir().join("does-not-exist.json");
        assert_eq!(read_state(&p), State::default());
    }

    #[test]
    fn corrupt_state_file_reads_as_default_rather_than_failing() {
        let p = tmpdir().join("corrupt.json");
        std::fs::write(&p, b"{ this is not json").unwrap();
        assert_eq!(read_state(&p), State::default());
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn state_round_trips_through_the_file() {
        let p = tmpdir().join("round-trip.json");
        let s = State {
            mode: Mode::Mesh,
            sharing: true,
            rebuild: Some(Rebuild {
                job: "job-7".into(),
                state: RebuildState::Building,
                progress: 0,
                message: "rebuild started".into(),
            }),
        };
        write_state(&p, &s).unwrap();
        assert_eq!(read_state(&p), s);
        std::fs::remove_file(&p).ok();
    }
}
