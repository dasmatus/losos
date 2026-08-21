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
use losos_ctl::installer_io::{options_from_env, run_install};
use losos_ctl::model::Mode;
use losos_ctl::overrides::validate_apply;
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

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
    /// The losos auto-installer. Destructive: it repartitions every target disk.
    Install(InstallArgs),
}

/// Flags for the installer. Runs locally; never touches the bus.
#[derive(Args)]
struct InstallArgs {
    /// Use TPM2 (a passphrase is asked once, at format time).
    #[arg(long)]
    tpm: bool,
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

fn main() -> ExitCode {
    let cli = Cli::parse();

    // The installer is local and prints its own progress as it goes; it does
    // not produce a JSON document to relay.
    if let Command::Install(args) = &cli.command {
        // Line-buffering matters when stdout is a pipe: block buffering would
        // hold the installer's own progress lines until exit, after the output
        // of the subprocesses they announce.
        let opts = options_from_env(
            args.tpm,
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

fn run(cli: Cli) -> Result<(), BackendFailure> {
    let json = match cli.command {
        Command::State { .. } => call_backend("State", &())?,
        Command::Status { .. } => call_backend("Status", &())?,
        Command::Settings { .. } => call_backend("Settings", &())?,
        Command::Change { mode } => call_backend("Change", &(mode.as_str(),))?,
        Command::FactoryReset => call_backend("FactoryReset", &())?,
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
        // Handled before the bus is ever touched.
        Command::Install(_) => unreachable!("install is dispatched in main"),
    };
    println!("{json}");
    Ok(())
}
