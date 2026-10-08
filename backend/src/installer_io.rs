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
use std::net::ToSocketAddrs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// Default upstream to install from. Overridable with `LOSOS_FLAKE_URL`.
pub const DEFAULT_FLAKE_URL: &str = "https://github.com/dasmatus/losos.git";
/// Where the flake is cloned to. Overridable with `LOSOS_FLAKE_WORK`.
pub const DEFAULT_FLAKE_WORK: &str = "/tmp/losos-flake";
/// The LUKS keyfile for the unattended path. Overridable with `LOSOS_KEYFILE`.
pub const DEFAULT_KEYFILE: &str = "/etc/keys/persist-keyfile";
/// Where the target file goes inside the clone.
pub const DEFAULT_TARGET_REL: &str = "modules/install-target.nix";
/// Where `--disko-script` writes the rendered target.
pub const DEFAULT_EMIT_FILE: &str = "/tmp/losos-install-target.nix";
/// Random bytes drawn for a generated keyfile. The file itself is their hex
/// spelling, so it is twice this long; see `keyfile_text`.
const KEYFILE_RANDOM_BYTES: usize = 2048;
/// How long to wait for the flake's host to resolve before giving up, in
/// seconds. Overridable with `LOSOS_NETWORK_TIMEOUT`.
const DEFAULT_NETWORK_TIMEOUT_SECS: u64 = 600;
/// Attempts at the clone itself once the host resolves.
const CLONE_ATTEMPTS: u32 = 3;

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

/// The keyfile's contents for `random` bytes of entropy: their lowercase hex
/// spelling, no newline.
///
/// The same file is read twice by two different readers, and they must see
/// the same bytes. At boot, stage 1 feeds `/crypto_keyfile.bin` to cryptsetup
/// as is, and so do `systemd-cryptenroll --unlock-key-file` (the TPM path's
/// enrolment) and `cryptsetup resize`. At format time, disko's luks `passwordFile` becomes
/// `<(echo -n "$(cat file)")` (lib/types/luks.nix), and bash's command
/// substitution drops every NUL byte and any trailing newlines. With raw
/// `/dev/urandom` output the volume was therefore formatted with a key that
/// was *not* the file's contents (4096 random bytes lose about 16 NULs), and
/// the first boot of a fresh install sat at "Please enter passphrase for disk
/// persist" with no passphrase anyone could type. Hex is NUL-free and
/// newline-free, so it survives the substitution byte for byte;
/// `tests/install.nix` asserts the generated file opens the volume.
fn keyfile_text(random: &[u8]) -> String {
    use std::fmt::Write;
    random
        .iter()
        .fold(String::with_capacity(random.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
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
        let mut buf = vec![0u8; KEYFILE_RANDOM_BYTES];
        {
            use std::io::Read;
            std::fs::File::open("/dev/urandom")?.read_exact(&mut buf)?;
        }
        std::fs::write(path, keyfile_text(&buf))?;
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
    ///
    /// The autorun ISO starts this from tty1's autologin, which getty reaches
    /// long before NetworkManager has a DHCP lease, so the first clone used to
    /// die with "Could not resolve host" and drop the owner to a root shell.
    /// It now waits for the flake's host to resolve, then retries the clone a
    /// couple of times for a flaky first connection. Nothing destructive has
    /// run by this point, so waiting is always safe.
    fn clone_flake(&mut self, url: &str, work: &Path) -> anyhow::Result<()> {
        if let Some(host) = flake_host(url) {
            let secs = std::env::var("LOSOS_NETWORK_TIMEOUT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_NETWORK_TIMEOUT_SECS);
            wait_for_host(&host, Duration::from_secs(secs))?;
        }
        let mut attempt = 1;
        loop {
            if work.exists() {
                std::fs::remove_dir_all(work)
                    .with_context(|| format!("clearing {}", work.display()))?;
            }
            match run_quiet(
                "git",
                &["clone", "--depth", "1", url, &work.to_string_lossy()],
            ) {
                Ok(()) => break,
                Err(e) if attempt < CLONE_ATTEMPTS => {
                    println!("losos-install: clone attempt {attempt} of {CLONE_ATTEMPTS} failed, retrying: {e:#}");
                    std::thread::sleep(Duration::from_secs(5 * u64::from(attempt)));
                    attempt += 1;
                }
                Err(e) => return Err(e),
            }
        }
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

    /// Seal a LUKS2 token to the TPM2, unattended.
    ///
    /// `--unlock-key-file` authenticates with the keyfile the volume was just
    /// formatted with, so nothing is typed. `--tpm2-pcrs=` (empty) binds the
    /// token to no PCRs, on purpose and for the same reason modules/keyring.nix
    /// gives: this box updates its firmware, bootloader and kernel unattended
    /// and has no shell to recover from, so a token bound to PCR 0 or 7 would
    /// lock the owner out on the first firmware update. What the chip buys
    /// without PCRs is that the disk alone (pulled, cloned, imaged) is
    /// unreadable; a thief who takes the whole box keeps the chip and is
    /// outside this threat model, as docs/security-model.md says.
    ///
    /// Inherited stdio: the tool prints what it sealed, and that belongs on
    /// the installer console.
    fn enroll_tpm(&mut self, device: &Path, keyfile: &Path) -> anyhow::Result<()> {
        let unlock = format!("--unlock-key-file={}", keyfile.display());
        run_inherit(
            "systemd-cryptenroll",
            &[
                "--tpm2-device=auto",
                "--tpm2-pcrs=",
                &unlock,
                &device.to_string_lossy(),
            ],
        )
    }

    fn log_info(&mut self, msg: &str) {
        println!("{msg}");
    }
}

/// The host a flake URL is fetched from, or `None` for a local path.
///
/// Covers the spellings `LOSOS_FLAKE_URL` realistically takes: `https://…`,
/// `git+https://…`, `ssh://user@host/…` and scp-style `user@host:path`.
fn flake_host(url: &str) -> Option<String> {
    let url = url.strip_prefix("git+").unwrap_or(url);
    let rest = match url.split_once("://") {
        Some(("file", _)) => return None,
        Some((_, rest)) => rest,
        // scp-style `git@host:owner/repo`; a bare path has no `:` before `/`.
        None => {
            let (head, _) = url.split_once(':')?;
            if head.contains('/') {
                return None;
            }
            head
        }
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host = authority.rsplit('@').next().unwrap_or(authority);
    let host = match host.rsplit_once(':') {
        Some((h, port)) if port.chars().all(|c| c.is_ascii_digit()) => h,
        _ => host,
    };
    (!host.is_empty()).then(|| host.to_string())
}

/// Block until `host` resolves, printing progress, or fail after `timeout`.
///
/// Resolution is the test because it is what the clone failed on, and it
/// needs a lease and a working resolver: exactly what is missing while
/// NetworkManager is still coming up. Ctrl-C ends the wait (the login
/// wrapper then drops to a shell), which is how the owner gets to `nmtui` to
/// join Wi-Fi before running `losos-install` again.
fn wait_for_host(host: &str, timeout: Duration) -> anyhow::Result<()> {
    let start = Instant::now();
    let mut announced = false;
    loop {
        if (host, 443)
            .to_socket_addrs()
            .is_ok_and(|mut a| a.next().is_some())
        {
            if announced {
                println!("losos-install: network is up, {host} resolves");
            }
            return Ok(());
        }
        let waited = start.elapsed();
        if waited >= timeout {
            bail!(
                "no network: {host} did not resolve within {}s. Plug in a network \
                 cable, or run nmtui to join Wi-Fi, then run losos-install again",
                timeout.as_secs()
            );
        }
        if !announced || waited.as_secs() % 30 < 2 {
            println!(
                "losos-install: waiting for network ({host} does not resolve yet, {}s so far). \
                 Plug in a cable; for Wi-Fi press Ctrl-C, run nmtui, then run losos-install",
                waited.as_secs()
            );
            announced = true;
        }
        std::thread::sleep(Duration::from_secs(2));
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

/// Whether the installer medium itself was booted in legacy BIOS mode.
///
/// The kernel only creates `/sys/firmware/efi` when the firmware handed it EFI
/// tables, so its absence is the standard test. The installed system has to
/// boot the way the machine did, because that is the only firmware proven to
/// work on it.
pub fn booted_in_bios() -> bool {
    !Path::new("/sys/firmware/efi").exists()
}

/// The EFI variable that says whether the firmware enforced Secure Boot
/// when it started this medium's loader: four attribute bytes, then one
/// byte, 1 for enabled.
const SECURE_BOOT_VAR: &str =
    "/sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c";

/// What the firmware did about Secure Boot before this medium ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecureBoot {
    /// UEFI, SecureBoot = 1: the firmware verified the medium's loader
    /// against its `db` before starting it, so the kernel, initrd and
    /// command line now running are the ones that were signed.
    Enabled,
    /// UEFI, SecureBoot = 0 (or no variable): nothing was verified.
    Disabled,
    /// Legacy BIOS: the feature does not exist there.
    Bios,
}

impl SecureBoot {
    /// Decode the variable's contents as the kernel exposes them in efivarfs.
    pub fn from_efivar(bios: bool, var: Option<&[u8]>) -> Self {
        if bios {
            return SecureBoot::Bios;
        }
        match var {
            Some(bytes) if bytes.len() >= 5 && bytes[4] == 1 => SecureBoot::Enabled,
            _ => SecureBoot::Disabled,
        }
    }

    /// The line the installer prints above its firmware menu, so the person
    /// at the screen sees whether the medium they booted was verified.
    pub fn banner(self) -> &'static str {
        match self {
            SecureBoot::Enabled => {
                "Secure Boot: enabled. The firmware verified this medium's signature."
            }
            SecureBoot::Disabled => {
                "Secure Boot: disabled. The firmware did not check this medium's signature."
            }
            SecureBoot::Bios => "Secure Boot: not available (legacy BIOS boot).",
        }
    }
}

/// Read the firmware's Secure Boot state for this boot.
pub fn secure_boot_state() -> SecureBoot {
    let var = std::fs::read(SECURE_BOOT_VAR).ok();
    SecureBoot::from_efivar(booted_in_bios(), var.as_deref())
}

/// Whether the installer medium can see a TPM2 chip.
///
/// `/dev/tpmrm0` is the kernel's resource-managed interface, which is what
/// systemd-cryptenroll and the initrd's systemd-cryptsetup open; `/dev/tpm0`
/// is the raw one, present whenever the driver bound at all. Either means the
/// hardware is there; the enrolment itself is what proves the chip works, and
/// it runs before anything slow.
pub fn tpm_present() -> bool {
    Path::new("/dev/tpmrm0").exists() || Path::new("/dev/tpm0").exists()
}

/// Fill in the environment-overridable defaults around the parsed CLI flags.
pub fn options_from_env(
    tpm: bool,
    bios: Option<bool>,
    drives: Option<Vec<String>>,
    no_install: bool,
    disko_script: Option<PathBuf>,
    emit_target: Option<PathBuf>,
) -> Options {
    Options {
        tpm,
        bios: bios.unwrap_or_else(booted_in_bios),
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

#[cfg(test)]
mod tests {
    use super::{flake_host, keyfile_text, wait_for_host, IoInstall, KEYFILE_RANDOM_BYTES};
    use crate::installer::Install;
    use std::time::Duration;

    #[test]
    fn keyfile_survives_bash_command_substitution() {
        // Every byte value, including NUL and newline, which bash's
        // `$(cat file)` would drop; the written text must carry none.
        let random: Vec<u8> = (0..=255u8).cycle().take(KEYFILE_RANDOM_BYTES).collect();
        let text = keyfile_text(&random);
        assert_eq!(text.len(), 2 * KEYFILE_RANDOM_BYTES);
        assert!(text.bytes().all(|b| b.is_ascii_hexdigit()), "{text}");
        // What `echo -n "$(cat file)"` hands cryptsetup at format time.
        let as_bash_sees_it: String = text.chars().filter(|&c| c != '\0').collect();
        assert_eq!(as_bash_sees_it.trim_end_matches('\n'), text);
        // And it is the random bytes, not a constant.
        assert_ne!(keyfile_text(&[1, 2, 3]), keyfile_text(&[3, 2, 1]));
    }

    #[test]
    fn a_generated_keyfile_is_hex_owner_only_and_left_alone_when_present() {
        use std::os::unix::fs::PermissionsExt;
        // No tempfile crate in this crate's dev-dependencies (a Cargo.lock
        // change means a new cargoHash); a pid-named dir under $TMPDIR does.
        let dir = std::env::temp_dir().join(format!("losos-keyfile-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("keys").join("persist-keyfile");
        IoInstall.ensure_keyfile(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.len(), 2 * KEYFILE_RANDOM_BYTES);
        assert!(text.bytes().all(|b| b.is_ascii_hexdigit()), "{text}");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        // A second run keeps the key the volume was formatted with.
        IoInstall.ensure_keyfile(&path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn secure_boot_state_is_read_from_the_variable_or_absent_under_bios() {
        use super::SecureBoot;
        // Four attribute bytes (NV+BS+RT), then the value.
        let on = [7, 0, 0, 0, 1];
        let off = [7, 0, 0, 0, 0];
        assert_eq!(
            SecureBoot::from_efivar(false, Some(&on)),
            SecureBoot::Enabled
        );
        assert_eq!(
            SecureBoot::from_efivar(false, Some(&off)),
            SecureBoot::Disabled
        );
        // No variable at all (firmware without Secure Boot support) is off.
        assert_eq!(SecureBoot::from_efivar(false, None), SecureBoot::Disabled);
        // A truncated variable never counts as enabled.
        assert_eq!(
            SecureBoot::from_efivar(false, Some(&[7, 0])),
            SecureBoot::Disabled
        );
        // Under BIOS the variable cannot exist and the answer is neither.
        assert_eq!(SecureBoot::from_efivar(true, Some(&on)), SecureBoot::Bios);
        assert!(SecureBoot::Enabled
            .banner()
            .starts_with("Secure Boot: enabled"));
        assert!(SecureBoot::Disabled
            .banner()
            .starts_with("Secure Boot: disabled"));
        assert!(SecureBoot::Bios.banner().contains("BIOS"));
    }

    #[test]
    fn wait_for_host_gives_up_with_a_clear_message() {
        // .invalid is reserved (RFC 6761) and never resolves.
        let err = wait_for_host("losos.invalid", Duration::ZERO).unwrap_err();
        assert!(err.to_string().contains("no network"), "{err}");
    }

    #[test]
    fn wait_for_host_returns_at_once_for_a_literal_address() {
        wait_for_host("127.0.0.1", Duration::ZERO).unwrap();
    }

    #[test]
    fn flake_host_covers_the_url_spellings() {
        let h = |u| flake_host(u);
        assert_eq!(
            h("https://github.com/dasmatus/losos.git").as_deref(),
            Some("github.com")
        );
        assert_eq!(
            h("git+https://github.com/dasmatus/losos").as_deref(),
            Some("github.com")
        );
        assert_eq!(
            h("https://example.org:8443/x.git").as_deref(),
            Some("example.org")
        );
        assert_eq!(
            h("ssh://git@example.org:2222/x.git").as_deref(),
            Some("example.org")
        );
        assert_eq!(
            h("git@github.com:dasmatus/losos.git").as_deref(),
            Some("github.com")
        );
        assert_eq!(
            h("https://user:pw@example.org/x").as_deref(),
            Some("example.org")
        );
        assert_eq!(h("file:///srv/losos"), None);
        assert_eq!(h("/srv/losos"), None);
        assert_eq!(h("./losos"), None);
    }
}
