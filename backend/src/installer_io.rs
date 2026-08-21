//! The real [`Install`] implementation, and the `losos-ctl install` entry point.
//!
//! Every external command is invoked with an argument list, never a shell
//! string, so there is no quoting to get wrong on a path that formats disks.
//!
//! Long-running commands (`disko`, `nixos-install`, a prebuilt disko script)
//! run with **inherited stdio**. Capturing them would leave the installer
//! console frozen for the entire install and would swallow the LUKS passphrase
//! prompt in TPM mode.

use crate::installer::{execute, plan_install, BlockDev, Install, LsblkOutput, Options};
use crate::io_backend::atomic_write;
use anyhow::{bail, Context};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Default upstream to install from. Overridable with `LOSOS_FLAKE_URL`.
pub const DEFAULT_FLAKE_URL: &str = "https://codeberg.org/dasmatus/losos.git";
/// Where the flake is cloned to. Overridable with `LOSOS_FLAKE_WORK`.
pub const DEFAULT_FLAKE_WORK: &str = "/tmp/losos-flake";
/// The LUKS keyfile for the unattended path. Overridable with `LOSOS_KEYFILE`.
pub const DEFAULT_KEYFILE: &str = "/etc/keys/persist-keyfile";
/// Where the target file goes inside the clone.
pub const DEFAULT_TARGET_REL: &str = "modules/install-target.nix";
/// Where `--disko-script` writes the rendered target.
pub const DEFAULT_EMIT_FILE: &str = "/tmp/losos-install-target.nix";
/// Bytes of key material for a generated keyfile.
const KEYFILE_BYTES: usize = 4096;

/// The production installer.
pub struct IoInstall;

/// Run a command with inherited stdio, failing on a non-zero exit.
fn run_inherit(exe: &str, args: &[&str]) -> anyhow::Result<()> {
    let status = Command::new(exe)
        .args(args)
        .status()
        .with_context(|| format!("could not start {exe}"))?;
    if !status.success() {
        bail!(
            "{exe} {} failed ({status}); see the output above",
            args.join(" ")
        );
    }
    Ok(())
}

/// Run a command quietly, surfacing stderr only if it fails.
fn run_quiet(exe: &str, args: &[&str]) -> anyhow::Result<()> {
    let out = Command::new(exe)
        .args(args)
        .output()
        .with_context(|| format!("could not start {exe}"))?;
    if !out.status.success() {
        bail!(
            "{exe} {} failed ({}):\n{}",
            args.join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(())
}

/// Make a directory owner-only (0700).
fn owner_only_dir(p: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(p)?;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o700))?;
    Ok(())
}

/// Make a file owner-only (0600). It holds a LUKS key.
fn owner_only_file(p: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

impl Install for IoInstall {
    fn write_file_atomic(&mut self, path: &Path, text: &str) -> anyhow::Result<()> {
        atomic_write(path, text.as_bytes())
    }

    /// Generate the LUKS keyfile unless one already exists.
    fn ensure_keyfile(&mut self, path: &Path) -> anyhow::Result<()> {
        if path.exists() {
            return Ok(());
        }
        println!("losos-install: generating {}", path.display());
        if let Some(dir) = path.parent() {
            owner_only_dir(dir)?;
        }
        let mut buf = vec![0u8; KEYFILE_BYTES];
        {
            use std::io::Read;
            std::fs::File::open("/dev/urandom")?.read_exact(&mut buf)?;
        }
        std::fs::write(path, &buf)?;
        owner_only_file(path)
    }

    fn run_disko_script(&mut self, path: &Path) -> anyhow::Result<()> {
        run_inherit(&path.to_string_lossy(), &[])
    }

    /// Clone the flake, then drop the clone's `.git`.
    ///
    /// This is not tidiness. Nix's git fetcher exposes only *tracked* files to
    /// flake evaluation, and `install-target.nix` is written into the clone
    /// after this returns — with `.git` present it would be invisible to
    /// `disko --flake` and `nixos-install --flake`, the `builtins.pathExists`
    /// guard in flake.nix would read false, and the build would fall back to
    /// the default target drive and TPM mode. That means formatting the wrong
    /// disk. Without `.git` the work dir is a plain path flake and every file
    /// in it is visible.
    fn clone_flake(&mut self, url: &str, work: &Path) -> anyhow::Result<()> {
        if work.exists() {
            std::fs::remove_dir_all(work)
                .with_context(|| format!("clearing {}", work.display()))?;
        }
        run_quiet(
            "git",
            &["clone", "--depth", "1", url, &work.to_string_lossy()],
        )?;
        let git_dir = work.join(".git");
        if git_dir.exists() {
            std::fs::remove_dir_all(&git_dir)
                .with_context(|| format!("removing {}", git_dir.display()))?;
        }
        Ok(())
    }

    /// Format and mount.
    ///
    /// `--yes-wipe-all-disks` is required: disko's combined destroy mode
    /// otherwise prompts for confirmation on stdin, and an unattended run reads
    /// EOF and aborts. This *is* the destructive unattended installer, so the
    /// confirmation is answered up front.
    fn run_disko(&mut self, work: &Path) -> anyhow::Result<()> {
        let flake = format!("{}#install", work.to_string_lossy());
        run_inherit(
            "disko",
            &[
                "--mode",
                "destroy,format,mount",
                "--yes-wipe-all-disks",
                "--flake",
                &flake,
            ],
        )
    }

    /// Bind `src` (under the mounted `/mnt/persist`) onto `dst` (under the
    /// `/mnt` tmpfs), creating both first.
    fn bind_mount(&mut self, src: &Path, dst: &Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(src)?;
        std::fs::create_dir_all(dst)?;
        run_quiet(
            "mount",
            &["--bind", &src.to_string_lossy(), &dst.to_string_lossy()],
        )
    }

    fn run_nixos_install(&mut self, work: &Path) -> anyhow::Result<()> {
        let flake = format!("{}#install", work.to_string_lossy());
        run_inherit("nixos-install", &["--flake", &flake, "--no-root-passwd"])
    }

    /// Copy the flake to `/persist/etc/nixos` and make it a git repo, so the
    /// `git+file:///etc/nixos` auto-upgrade has something to track.
    fn lay_flake(&mut self, work: &Path, dest: &Path) -> anyhow::Result<()> {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if dest.exists() {
            std::fs::remove_dir_all(dest)?;
        }
        run_quiet(
            "cp",
            &["-a", &work.to_string_lossy(), &dest.to_string_lossy()],
        )?;
        run_quiet("chmod", &["-R", "u+w", &dest.to_string_lossy()])?;

        let d = dest.to_string_lossy().to_string();
        let git = |args: &[&str]| -> anyhow::Result<()> {
            let mut full = vec!["-C", &d];
            full.extend_from_slice(args);
            run_quiet("git", &full)
        };
        git(&["init", "-q"])?;
        git(&["add", "-A"])?;
        git(&[
            "-c",
            "user.email=losos@local",
            "-c",
            "user.name=losos-install",
            "commit",
            "-qm",
            "losos install",
        ])
    }

    fn copy_keyfile(&mut self, src: &Path, dst: &Path) -> anyhow::Result<()> {
        if let Some(dir) = dst.parent() {
            owner_only_dir(dir)?;
        }
        std::fs::copy(src, dst)
            .with_context(|| format!("copying {} to {}", src.display(), dst.display()))?;
        owner_only_file(dst)
    }

    fn log_info(&mut self, msg: &str) {
        println!("{msg}");
    }
}

/// Enumerate block devices with `lsblk --json --bytes`.
fn read_block_devices() -> anyhow::Result<Vec<BlockDev>> {
    let out = Command::new("lsblk")
        .args([
            "--json",
            "--bytes",
            "-o",
            "NAME,SIZE,RM,TYPE,MOUNTPOINTS,PKNAME",
        ])
        .output()
        .context("could not run lsblk")?;
    if !out.status.success() {
        bail!(
            "lsblk failed ({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let parsed: LsblkOutput = serde_json::from_slice(&out.stdout).with_context(|| {
        format!(
            "could not parse lsblk output: {}",
            String::from_utf8_lossy(&out.stdout)
                .chars()
                .take(200)
                .collect::<String>()
        )
    })?;
    Ok(parsed.blockdevices)
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Fill in the environment-overridable defaults around the parsed CLI flags.
pub fn options_from_env(
    tpm: bool,
    drives: Option<Vec<String>>,
    no_install: bool,
    disko_script: Option<PathBuf>,
    emit_target: Option<PathBuf>,
) -> Options {
    Options {
        tpm,
        drives,
        no_install,
        disko_script,
        emit_target,
        flake_url: env_or("LOSOS_FLAKE_URL", DEFAULT_FLAKE_URL),
        flake_work: PathBuf::from(env_or("LOSOS_FLAKE_WORK", DEFAULT_FLAKE_WORK)),
        keyfile: PathBuf::from(env_or("LOSOS_KEYFILE", DEFAULT_KEYFILE)),
        target_rel: PathBuf::from(DEFAULT_TARGET_REL),
        emit_file: PathBuf::from(DEFAULT_EMIT_FILE),
    }
}

/// Plan and run an install.
///
/// Devices are only enumerated when auto-detecting, so `--drives` works on a
/// machine where `lsblk` would fail or is absent.
pub fn run_install(opts: &Options) -> anyhow::Result<()> {
    let devs = match opts.drives {
        Some(_) => Vec::new(),
        None => read_block_devices()?,
    };
    let acts = plan_install(opts, &devs).map_err(|e| anyhow::anyhow!(e))?;
    execute(&mut IoInstall, &acts)
}
