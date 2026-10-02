//! `losos-ctl` — the facade CLI.
//!
//! Most subcommands relay to `lososd` over the system bus and print the JSON it
//! replies with, followed by a newline. The daemon does the privileged work;
//! this binary needs nothing but the right to send on the bus.
//!
//! `install` is the exception: it runs entirely locally. It is the installer
//! ISO's entry point, where there is no daemon and no bus to talk to.
//!
//! `--json` is accepted on the read-only subcommands and does nothing: output
//! has always been JSON, and the flag exists so older callers keep working.

use clap::{Args, Parser, Subcommand};
use losos_ctl::facade::{call_backend, BackendFailure};
use losos_ctl::installer_io::{booted_in_bios, options_from_env, run_install};
use losos_ctl::model::Mode;
use losos_ctl::overrides::validate_apply;
use std::io::{IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(
    name = "losos-ctl",
    about = "losos appliance control facade (D-Bus client for lososd)",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the current mode and sharing flag.
    State {
        /// Accepted for compatibility; output is always JSON.
        #[arg(long)]
        json: bool,
    },
    /// Apply a new mode and trigger a rebuild.
    Change {
        /// Sharing posture: local (private) or mesh (contribute storage).
        #[arg(long, value_parser = parse_mode)]
        mode: Mode,
    },
    /// Print rebuild progress.
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Print the current losos.* settings from overrides.nix.
    Settings {
        #[arg(long)]
        json: bool,
    },
    /// Apply Nix config read from stdin (rewrites overrides.nix, then rebuilds).
    Apply,
    /// Soft factory reset: restore defaults and rebuild.
    ///
    /// The destructive reset — wiping the disks — is the installer ISO, not
    /// this subcommand.
    FactoryReset,
    /// Grow /persist into the volume group's unallocated extents.
    ///
    /// Online: nothing is unmounted and no rebuild is triggered. The space
    /// comes from the margin modules/disko.nix leaves at install time
    /// (losos.storage.fillPercent); when that is exhausted, add a disk with
    /// pvcreate and vgextend and run this again.
    Grow,
    /// Replace the Nextcloud admin password, read from stdin.
    ///
    /// Replaces it; there is no way to read the existing one out. The password
    /// generated at install time by modules/nextcloud-common.nix is 0600 and
    /// shown nowhere, which is exactly the problem this solves.
    ///
    /// Stdin and not a flag: /proc/<pid>/cmdline is world-readable, so
    /// `--password hunter2` would publish it to every process on the box.
    ///
    ///   printf %s 'correct horse battery staple' | losos-ctl set-password
    SetPassword {
        /// Account to reset. Defaults to the appliance's Nextcloud admin.
        #[arg(long, default_value = losos_ctl::setup::DEFAULT_ADMIN_USER)]
        user: String,
    },
    /// Print the appliance recovery code, minting one on first use.
    ///
    /// Stable for the life of the installation: the same code comes back on
    /// every later call, and there is no subcommand that replaces it. That is
    /// the point — a code that could be rotated is a code the owner's written
    /// copy stops matching.
    ///
    /// It survives `factory-reset` (which rewrites state.json and
    /// overrides.nix, not /var/secrets) and is destroyed only by a reinstall,
    /// which is the one event it exists for.
    Recovery {
        #[arg(long, hide = true)]
        json: bool,
    },
    /// The losos auto-installer. Destructive: it repartitions every target disk.
    Install(InstallArgs),
}

/// Flags for the installer. Runs locally; never touches the bus.
#[derive(Args)]
struct InstallArgs {
    /// Use TPM2 (a passphrase is asked once, at format time).
    #[arg(long)]
    tpm: bool,
    /// Install for legacy BIOS (GRUB). Default: whatever firmware booted this
    /// medium.
    #[arg(long, conflicts_with = "uefi")]
    bios: bool,
    /// Install for UEFI (systemd-boot); requires this medium to boot in UEFI mode.
    #[arg(long)]
    uefi: bool,
    /// Override drive auto-detection with a comma-separated list.
    #[arg(long, value_name = "A,/dev/b,...", value_parser = parse_drives)]
    drives: Option<Vec<String>>,
    /// Stop after disko (format and mount); skip nixos-install.
    #[arg(long)]
    no_install: bool,
    /// Run a prebuilt diskoScript instead of `disko --flake`.
    #[arg(long, value_name = "PATH")]
    disko_script: Option<PathBuf>,
    /// Write install-target.nix to FILE and exit, touching no disks.
    #[arg(long, value_name = "FILE")]
    emit_target: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FirmwareMode {
    Bios,
    Uefi,
    Autodetect,
}

impl FirmwareMode {
    fn parse(choice: &str) -> Option<Self> {
        match choice.trim() {
            "1" => Some(Self::Bios),
            "2" => Some(Self::Uefi),
            "" | "3" => Some(Self::Autodetect),
            _ => None,
        }
    }

    fn bios_override(self) -> Option<bool> {
        match self {
            Self::Bios => Some(true),
            Self::Uefi => Some(false),
            Self::Autodetect => None,
        }
    }
}

/// Split `--drives a,b,c`, rejecting a list that is empty once trimmed.
fn parse_drives(s: &str) -> Result<Vec<String>, String> {
    let ds: Vec<String> = s
        .split(',')
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty())
        .collect();
    if ds.is_empty() {
        Err("no drives given".to_string())
    } else {
        Ok(ds)
    }
}

/// Reject an invalid mode while parsing, so no bad value ever reaches the bus.
fn parse_mode(s: &str) -> Result<Mode, String> {
    Mode::parse(s).ok_or_else(|| "mode must be 'local' or 'mesh'".to_string())
}

/// How long the ISO's firmware menu waits before taking the default.
///
/// The installer medium is set-and-forget: insert it, boot, walk away. A menu
/// that waits forever would park an unattended reinstall at the prompt, and
/// the CI gate `tests/iso-boot.py`, which never types anything, would time out
/// waiting for the installer's DNS lookup. So no answer means autodetect.
const FIRMWARE_MENU_TIMEOUT: Duration = Duration::from_secs(30);

/// One answer to the firmware prompt, or why there is none.
#[derive(Debug)]
enum MenuInput {
    Line(String),
    Timeout,
    Eof,
}

fn select_firmware(explicit: Option<bool>, interactive: bool) -> std::io::Result<Option<bool>> {
    if let Some(bios) = explicit {
        return Ok(Some(bios));
    }
    if !interactive {
        return Ok(None);
    }
    prompt_firmware(
        read_stdin_line,
        FIRMWARE_MENU_TIMEOUT,
        &mut std::io::stdout(),
    )
}

/// Read one line from stdin, giving up after `timeout`.
///
/// poll(2) rather than a reader thread: a thread left blocked in read_line
/// after a timeout would keep reading the terminal for the rest of the
/// install, and a `--tpm` install hands that terminal to disko to collect the
/// LUKS passphrase. A stray reader would swallow it and leave disko waiting.
/// The terminal is in canonical mode, so a readable stdin holds one whole
/// line and read_line does not buffer past it.
fn read_stdin_line(timeout: Duration) -> std::io::Result<MenuInput> {
    use rustix::event::{poll, PollFd, PollFlags, Timespec};
    let stdin = std::io::stdin();
    let deadline = Instant::now() + timeout;
    let ready = loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let wait = Timespec::try_from(left).unwrap_or(Timespec {
            tv_sec: i64::MAX,
            tv_nsec: 0,
        });
        let mut fds = [PollFd::new(&stdin, PollFlags::IN)];
        match poll(&mut fds, Some(&wait)) {
            // A signal is not an answer: wait out the rest of the deadline.
            Err(rustix::io::Errno::INTR) => continue,
            other => break other?,
        }
    };
    match ready {
        0 => Ok(MenuInput::Timeout),
        _ => {
            let mut line = String::new();
            if stdin.read_line(&mut line)? == 0 {
                Ok(MenuInput::Eof)
            } else {
                Ok(MenuInput::Line(line))
            }
        }
    }
}

/// The menu itself, over an injectable line source, so the timeout and the
/// retry loop are unit-testable. A timeout or EOF picks autodetect; the
/// deadline is overall, so retyping garbage cannot extend it.
fn prompt_firmware(
    mut read_line: impl FnMut(Duration) -> std::io::Result<MenuInput>,
    timeout: Duration,
    out: &mut impl Write,
) -> std::io::Result<Option<bool>> {
    let deadline = Instant::now() + timeout;
    loop {
        writeln!(out, "Choose the firmware mode for the installed system:")?;
        writeln!(out, "  1) BIOS")?;
        writeln!(
            out,
            "  2) UEFI (requires this installer to be booted in UEFI mode)"
        )?;
        writeln!(
            out,
            "  3) Autodetect (use the firmware that booted this installer)"
        )?;
        write!(
            out,
            "Selection [3, chosen automatically after {}s]: ",
            timeout.as_secs()
        )?;
        out.flush()?;

        let left = deadline.saturating_duration_since(Instant::now());
        match read_line(left)? {
            MenuInput::Line(line) => match FirmwareMode::parse(&line) {
                Some(mode) => return Ok(mode.bios_override()),
                None => writeln!(out, "Choose 1, 2, or 3.")?,
            },
            MenuInput::Timeout => {
                writeln!(out)?;
                writeln!(out, "No choice made; autodetecting.")?;
                return Ok(None);
            }
            MenuInput::Eof => return Ok(None),
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // The installer is local and prints its own progress as it goes; it does
    // not produce a JSON document to relay.
    if let Command::Install(args) = &cli.command {
        // Line-buffering matters when stdout is a pipe: block buffering would
        // hold the installer's own progress lines until exit, after the output
        // of the subprocesses they announce.
        let explicit_bios = match (args.bios, args.uefi) {
            (true, _) => Some(true),
            (_, true) => Some(false),
            _ => None,
        };
        let on_installer_iso = std::env::var_os("LOSOS_INSTALLER_ISO").is_some();
        let interactive = on_installer_iso
            && std::io::stdin().is_terminal()
            && std::io::stdout().is_terminal()
            && args.emit_target.is_none();
        let bios = match select_firmware(explicit_bios, interactive) {
            Ok(bios) => bios,
            Err(e) => {
                eprintln!("losos-install: cannot read firmware selection: {e}");
                return ExitCode::FAILURE;
            }
        };
        if let Err(e) = validate_firmware_choice(bios, booted_in_bios()) {
            eprintln!("losos-install: {e}");
            return ExitCode::FAILURE;
        }
        let opts = options_from_env(
            args.tpm,
            bios,
            args.drives.clone(),
            args.no_install,
            args.disko_script.clone(),
            args.emit_target.clone(),
        );
        return match run_install(&opts) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("losos-install: {e:#}");
                ExitCode::FAILURE
            }
        };
    }

    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn validate_firmware_choice(
    target_bios: Option<bool>,
    installer_bios: bool,
) -> Result<(), &'static str> {
    if installer_bios && target_bios == Some(false) {
        Err("cannot install for UEFI when the installer was booted in BIOS mode; reboot the installer in UEFI mode")
    } else {
        Ok(())
    }
}

fn run(cli: Cli) -> Result<(), BackendFailure> {
    let json = match cli.command {
        Command::State { .. } => call_backend("State", &())?,
        Command::Status { .. } => call_backend("Status", &())?,
        Command::Settings { .. } => call_backend("Settings", &())?,
        Command::Change { mode } => call_backend("Change", &(mode.as_str(),))?,
        Command::FactoryReset => call_backend("FactoryReset", &())?,
        Command::Grow => call_backend("Grow", &())?,
        Command::Recovery { .. } => call_backend("Recovery", &())?,
        Command::Apply => {
            let mut input = String::new();
            std::io::stdin()
                .read_to_string(&mut input)
                .map_err(|e| BackendFailure(format!("losos-ctl apply: cannot read stdin — {e}")))?;
            // Validate locally so an obviously bad payload fails immediately
            // without a round trip. lososd validates again on its side — this
            // is a convenience, never the security boundary.
            let code = validate_apply(&input)
                .map_err(|e| BackendFailure(format!("losos-ctl apply: {e}")))?;
            call_backend("Apply", &(code,))?
        }
        Command::SetPassword { user } => {
            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input).map_err(|e| {
                BackendFailure(format!("losos-ctl set-password: cannot read stdin — {e}"))
            })?;
            // Exactly one trailing newline, not `trim`: `echo` and every heredoc
            // add one, and a password is allowed to end in a space. Trimming
            // would silently set a different password from the one that was
            // typed, on the one command where nobody can check afterwards.
            let password = input.strip_suffix('\n').unwrap_or(&input);
            let password = password.strip_suffix('\r').unwrap_or(password);
            // Checked locally so an obviously bad password fails without a round
            // trip; lososd validates again, and that is the boundary.
            losos_ctl::setup::validate_password(password)
                .map_err(|e| BackendFailure(format!("losos-ctl set-password: {e}")))?;
            call_backend("SetPassword", &(user.as_str(), password))?
        }
        // Handled before the bus is ever touched.
        Command::Install(_) => unreachable!("install is dispatched in main"),
    };
    println!("{json}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{prompt_firmware, validate_firmware_choice, FirmwareMode, MenuInput};
    use std::time::Duration;

    #[test]
    fn firmware_choices_resolve_as_expected() {
        assert_eq!(FirmwareMode::parse("1"), Some(FirmwareMode::Bios));
        assert_eq!(FirmwareMode::parse("2"), Some(FirmwareMode::Uefi));
        assert_eq!(FirmwareMode::parse("3"), Some(FirmwareMode::Autodetect));
        assert_eq!(FirmwareMode::parse(""), Some(FirmwareMode::Autodetect));
        assert_eq!(FirmwareMode::parse("4"), None);

        assert_eq!(FirmwareMode::Bios.bios_override(), Some(true));
        assert_eq!(FirmwareMode::Uefi.bios_override(), Some(false));
        assert_eq!(FirmwareMode::Autodetect.bios_override(), None);
    }

    #[test]
    fn uefi_install_requires_uefi_booted_installer() {
        assert!(validate_firmware_choice(Some(false), true).is_err());
        assert!(validate_firmware_choice(Some(false), false).is_ok());
        assert!(validate_firmware_choice(Some(true), true).is_ok());
        assert!(validate_firmware_choice(None, true).is_ok());
    }

    /// A scripted line source: each call pops the next answer.
    fn script(mut answers: Vec<MenuInput>) -> impl FnMut(Duration) -> std::io::Result<MenuInput> {
        answers.reverse();
        move |_| Ok(answers.pop().unwrap_or(MenuInput::Eof))
    }

    #[test]
    fn firmware_menu_retries_then_takes_a_valid_choice() {
        let mut out = Vec::new();
        let read = script(vec![
            MenuInput::Line("x\n".into()),
            MenuInput::Line("1\n".into()),
        ]);
        let got = prompt_firmware(read, Duration::from_secs(5), &mut out).unwrap();
        assert_eq!(got, Some(true));
        assert!(String::from_utf8(out)
            .unwrap()
            .contains("Choose 1, 2, or 3."));
    }

    #[test]
    fn firmware_menu_autodetects_on_timeout_and_on_eof() {
        // Unattended boot: nobody types, so the menu must not block the install.
        let mut out = Vec::new();
        let got = prompt_firmware(script(vec![MenuInput::Timeout]), Duration::ZERO, &mut out);
        assert_eq!(got.unwrap(), None);
        assert!(String::from_utf8(out).unwrap().contains("autodetecting"));

        let got = prompt_firmware(script(vec![]), Duration::from_secs(5), &mut Vec::new());
        assert_eq!(got.unwrap(), None);
    }

    #[test]
    fn firmware_menu_deadline_is_overall() {
        // Each re-prompt gets only what is left, never a fresh 30 s.
        let mut budgets = Vec::new();
        let mut first = true;
        let read = |left: Duration| {
            budgets.push(left);
            if std::mem::take(&mut first) {
                std::thread::sleep(Duration::from_millis(20));
                Ok(MenuInput::Line("nope\n".into()))
            } else {
                Ok(MenuInput::Timeout)
            }
        };
        prompt_firmware(read, Duration::from_millis(200), &mut Vec::new()).unwrap();
        assert!(budgets[1] < budgets[0], "{budgets:?}");
    }
}
