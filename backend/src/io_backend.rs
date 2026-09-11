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

/// The real [`crate::recovery::CodeStore`]: one 0600 file plus `/dev/urandom`.
///
/// Separate from [`IoLosos`] rather than folded into it because the recovery
/// code is the one secret here that must outlive a factory reset, so its path
/// comes from the unit's environment (`LOSOS_RECOVERY_FILE`) and not from
/// [`Paths`], whose entries all live under directories a reset clears.
#[derive(Debug, Clone)]
pub struct FileCodeStore {
    path: PathBuf,
}

impl FileCodeStore {
    /// The path `modules/recovery.nix` puts in the unit's environment, falling
    /// back to [`crate::recovery::DEFAULT_RECOVERY_FILE`].
    pub fn from_env() -> Self {
        Self {
            path: crate::recovery::code_file(),
        }
    }
}

impl Default for FileCodeStore {
    fn default() -> Self {
        Self::from_env()
    }
}

impl crate::recovery::CodeStore for FileCodeStore {
    fn read_code(&mut self) -> anyhow::Result<Option<String>> {
        match std::fs::read_to_string(&self.path) {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            // Must be an error rather than `None`: `ensure_code` mints over a
            // `None`, so reporting an EIO or an EACCES that way would destroy a
            // code the owner is still holding on paper. `tests/recovery.rs`
            // asserts the propagation.
            Err(e) => Err(anyhow::Error::new(e).context(format!(
                "reading the recovery code at {}",
                self.path.display()
            ))),
        }
    }

    fn write_code(&mut self, code: &str) -> anyhow::Result<()> {
        // Trailing newline so `cat` of the file in a rescue shell prints
        // cleanly; `is_well_formed` trims, so the round trip is exact.
        atomic_write_secret(&self.path, format!("{code}\n").as_bytes())
            .with_context(|| format!("writing the recovery code to {}", self.path.display()))
    }

    fn fresh_bytes(&mut self) -> anyhow::Result<[u8; 16]> {
        use std::io::Read;
        let mut buf = [0u8; 16];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut buf))
            .context("reading 16 bytes from /dev/urandom")?;
        Ok(buf)
    }
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

    // ── Online growth of /persist ───────────────────────────────────────
    fn vg_free(&mut self) -> anyhow::Result<crate::grow::VgFree> {
        // `vgs --units b --nosuffix --noheadings -o vg_free_count,vg_extent_size`
        // gives the two numbers with no locale formatting to reparse. The
        // extent count is already a count, so --units only affects the size.
        let out = std::process::Command::new("vgs")
            .args([
                "--noheadings",
                "--nosuffix",
                "--units",
                "b",
                "-o",
                "vg_free_count,vg_extent_size",
                crate::grow::VG,
            ])
            .output()
            .with_context(|| format!("running vgs against {}", crate::grow::VG))?;
        if !out.status.success() {
            anyhow::bail!(
                "vgs failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let mut fields = text.split_whitespace();
        let free_extents: u64 = fields
            .next()
            .and_then(|f| f.parse().ok())
            .with_context(|| format!("no free-extent count in vgs output: {text:?}"))?;
        let extent_bytes: u64 = fields
            .next()
            .and_then(|f| f.parse().ok())
            .with_context(|| format!("no extent size in vgs output: {text:?}"))?;
        Ok(crate::grow::VgFree {
            free_extents,
            extent_bytes,
        })
    }

    fn luks_key_file(&mut self) -> anyhow::Result<Option<String>> {
        // `$LOSOS_LUKS_KEYFILE`, set by modules/daemon.nix on the no-TPM path.
        //
        // Unset means the TPM path, where the volume key is in the kernel
        // keyring and `cryptsetup resize` finds it there. Where it is neither
        // set nor in the keyring, cryptsetup falls back to prompting on stdin
        // — and a daemon has none, so the resize dies with "Nothing to read on
        // input." *after* lvextend has already grown the logical volume. That
        // is not theoretical: it is how tests/resize.nix failed first.
        Ok(std::env::var("LOSOS_LUKS_KEYFILE")
            .ok()
            .filter(|p| !p.is_empty()))
    }

    fn run_grow(&mut self, action: &crate::grow::GrowAction) -> anyhow::Result<()> {
        let argv = crate::grow::action_argv(action);
        let (cmd, args) = argv.split_first().expect("action_argv is never empty");
        let out = std::process::Command::new(cmd)
            .args(args)
            .output()
            .with_context(|| format!("running {}", argv.join(" ")))?;
        if !out.status.success() {
            anyhow::bail!(
                "{} failed: {}",
                argv.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(())
    }

    fn persist_bytes(&mut self) -> anyhow::Result<u64> {
        // `df -B1`, not `df -h`: this number is compared before and after to
        // decide whether the grow did anything, so a value rounded to "1.6T"
        // would report no change for the first 50 GB of growth.
        //
        // A subprocess rather than statvfs(2) because the alternative is a new
        // crate dependency, and backend/Cargo.toml declares its whole set up
        // front specifically so Cargo.lock — and the cargoHash pinned in
        // flake/packages.nix — settles once. The other three steps here are
        // subprocesses anyway.
        let out = std::process::Command::new("df")
            .args(["-B1", "--output=size", "/persist"])
            .output()
            .context("running df against /persist")?;
        if !out.status.success() {
            anyhow::bail!("df failed: {}", String::from_utf8_lossy(&out.stderr).trim());
        }
        let text = String::from_utf8_lossy(&out.stdout);
        // Line 1 is the "1B-blocks" header; line 2 is the number.
        text.lines()
            .nth(1)
            .and_then(|l| l.trim().parse().ok())
            .with_context(|| format!("no size in df output: {text:?}"))
    }

    // ── Setting the Nextcloud admin password ────────────────────────────
    fn nextcloud_mode(&mut self) -> anyhow::Result<crate::setup::NcMode> {
        // No default. Guessing "container" would run `crictl` against a socket
        // that does not exist on a native box, and guessing "native" would run
        // `nextcloud-occ`, which is not even in the closure of a container-mode
        // box — and both failures read as "the command is broken" rather than
        // "the daemon was not told". modules/daemon.nix sets this from
        // config.losos.nextcloud.mode.
        let raw = match std::env::var("LOSOS_NEXTCLOUD_MODE") {
            Ok(raw) => raw,
            Err(std::env::VarError::NotPresent) => anyhow::bail!(
                "LOSOS_NEXTCLOUD_MODE is not set, so lososd cannot tell whether \
                 Nextcloud is running as a k3s workload or natively. \
                 modules/daemon.nix must set it from config.losos.nextcloud.mode."
            ),
            Err(e) => return Err(anyhow::Error::new(e).context("LOSOS_NEXTCLOUD_MODE")),
        };
        crate::setup::NcMode::parse(raw.trim()).with_context(|| {
            format!("LOSOS_NEXTCLOUD_MODE must be 'container' or 'native': {raw:?}")
        })
    }

    fn nextcloud_target(
        &mut self,
        mode: crate::setup::NcMode,
    ) -> anyhow::Result<crate::setup::Target> {
        if mode == crate::setup::NcMode::Native {
            return Ok(crate::setup::Target::Native);
        }
        let socket = std::env::var("LOSOS_CRI_SOCKET")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| crate::setup::DEFAULT_CRI_SOCKET.to_string());
        let argv = crate::setup::resolve_argv(&socket);
        let (cmd, args) = argv.split_first().context("resolve_argv is never empty")?;
        let out = std::process::Command::new(cmd)
            .args(args)
            .output()
            .with_context(|| {
                format!(
                    "running {} (is pkgs.cri-tools on lososd's unit path?)",
                    argv.join(" ")
                )
            })?;
        if !out.status.success() {
            anyhow::bail!(
                "{} failed: {}",
                argv.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        let id = crate::setup::parse_container_ids(&String::from_utf8_lossy(&out.stdout))
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(crate::setup::Target::Container { socket, id })
    }

    fn run_occ(
        &mut self,
        action: &crate::setup::OccAction,
        secret: &crate::setup::Secret,
    ) -> anyhow::Result<Option<crate::setup::OccOutcome>> {
        use crate::setup::{OccAction, OccOutcome, SecretChannel};
        match action {
            OccAction::StageSecret { path, uid } => {
                let path = Path::new(path);
                // No trailing newline: the in-image wrapper reads this with
                // `$(cat …)`, which strips trailing newlines, so writing one
                // would make the two sides agree only by accident.
                atomic_write_secret(path, secret.expose().as_bytes())
                    .with_context(|| format!("staging the new password at {}", path.display()))?;
                // 0600 is root-only until this lands, and the pod's process is
                // uid 1002. The chown is what makes the file readable by
                // exactly one account and no group.
                std::os::unix::fs::chown(path, Some(*uid), Some(*uid)).with_context(|| {
                    format!(
                        "giving {} to uid {uid} so the pod can read it",
                        path.display()
                    )
                })?;
                Ok(None)
            }
            OccAction::ClearSecret { path } => {
                match std::fs::remove_file(path) {
                    Ok(()) => {}
                    // Already gone is the goal, not a failure.
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => {
                        return Err(anyhow::Error::new(e)
                            .context(format!("removing the staged password at {path}")))
                    }
                }
                Ok(None)
            }
            OccAction::RunOcc { argv, secret: chan } => {
                let (cmd, args) = argv.split_first().context("RunOcc argv is never empty")?;
                let mut child = std::process::Command::new(cmd);
                child.args(args);
                if let SecretChannel::Env = chan {
                    // /proc/<pid>/environ is mode 0400 owner-only and this
                    // child is root, unlike /proc/<pid>/cmdline.
                    child.env("OC_PASS", secret.expose());
                    // ResetPassword.php reads `getenv('NC_PASS') ?: getenv('OC_PASS')`,
                    // so an NC_PASS inherited from anywhere would silently win
                    // over the password the owner just typed.
                    child.env_remove("NC_PASS");
                    // The nixpkgs nextcloud-occ wrapper tests `$USER` under
                    // `set -u`, and systemd does not export USER to a root
                    // service with no User=. Unset, the wrapper aborts with
                    // "USER: unbound variable" before occ ever starts.
                    child.env("USER", "root");
                }
                let out = child.output().with_context(|| {
                    format!(
                        "running {} (is the occ wrapper on lososd's unit path?)",
                        // The argv is safe to quote: the password is never in it.
                        argv.join(" ")
                    )
                })?;
                Ok(Some(OccOutcome {
                    // A signalled child has no code; -1 is not a status occ can
                    // return, so it cannot be mistaken for one.
                    code: out.status.code().unwrap_or(-1),
                    stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                    stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
                }))
            }
        }
    }

    fn recovery_code(&mut self) -> anyhow::Result<crate::recovery::Recovery> {
        crate::recovery::ensure_code(&mut FileCodeStore::from_env())
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
            claimed: true,
        };
        write_state(&p, &s).unwrap();
        assert_eq!(read_state(&p), s);
        std::fs::remove_file(&p).ok();
    }
}
