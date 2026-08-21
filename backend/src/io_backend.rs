//! The real [`Losos`] implementation: files, environment, `systemd-run`.
//!
//! Every path is environment-overridable so the CLI can be driven against a
//! throwaway directory in tests without touching `/etc` or `/var`.
//!
//! Note the deliberate asymmetry in how the two Nix files are written:
//! `overrides.nix` and `state.json` go through [`atomic_write`] (temp file plus
//! rename, so a crash can never leave a half-written config), while
//! `defaults.nix` is rewritten in place by [`Losos::rewrite_config`]. That
//! matches the behaviour that shipped and is left as-is.

use crate::losos::Losos;
use crate::model::State;
use crate::overrides::{inject_line, DEFAULT_OVERRIDES_NIX};
use crate::supervisor;
use anyhow::Context;
use std::path::{Path, PathBuf};

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

/// Write `content` to `path` atomically: a sibling temp file, then a rename.
///
/// The rename is atomic on POSIX, so a reader either sees the whole old file or
/// the whole new one — never a truncated config.
pub fn atomic_write(path: &Path, content: &[u8]) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let tmp = path.with_extension(format!(
        "{}tmp",
        path.extension()
            .map(|e| format!("{}.", e.to_string_lossy()))
            .unwrap_or_default()
    ));
    std::fs::write(&tmp, content).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("renaming {} to {}", tmp.display(), path.display()))?;
    Ok(())
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
#[derive(Debug, Clone)]
pub struct IoLosos {
    pub paths: Paths,
}

impl IoLosos {
    pub fn from_env() -> Self {
        IoLosos {
            paths: Paths::from_env(),
        }
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
        let Ok(contents) = std::fs::read_to_string(path) else {
            // Absent config is a warning, not a failure: the appliance may be
            // running from a flake laid out differently, and refusing to change
            // mode over it would be worse than carrying on.
            eprintln!(
                "losos-ctl: warning: config {} missing; not rewriting",
                path.display()
            );
            return Ok(());
        };
        let lines: Vec<String> = contents.lines().map(str::to_string).collect();
        let mut out = inject_line(sharing, &lines).join("\n");
        out.push('\n');
        std::fs::write(path, out).with_context(|| format!("rewriting {}", path.display()))
    }

    fn write_overrides(&mut self, body: &str) -> anyhow::Result<()> {
        atomic_write(&self.paths.overrides_file, body.as_bytes())
    }

    fn read_overrides(&mut self) -> anyhow::Result<String> {
        Ok(std::fs::read_to_string(&self.paths.overrides_file)
            .unwrap_or_else(|_| DEFAULT_OVERRIDES_NIX.to_string()))
    }

    fn spawn_rebuild(&mut self, job: &str) -> anyhow::Result<()> {
        supervisor::spawn_rebuild(&self.paths, job)
    }

    fn rebuild_log_tail(&mut self) -> anyhow::Result<String> {
        Ok(supervisor::log_tail(&self.paths.rebuild_log()))
    }

    fn next_job_id(&mut self) -> anyhow::Result<String> {
        Ok(format!(
            "{}-{}",
            chrono::Utc::now().format("%Y%m%d%H%M%S"),
            std::process::id()
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
