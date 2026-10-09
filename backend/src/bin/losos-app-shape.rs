//! `losos-app-shape` — the Helm post-renderer behind an app install.
//!
//! Helm hands its rendered chart to this binary through the `losos-shape`
//! plugin (`modules/apps.nix`), which first turns the YAML into one JSON
//! document per line with `yq`. The rules are [`losos_ctl::apps::shape`];
//! this file is the plumbing around them:
//!
//!   * the shaped documents go back to Helm on stdout;
//!   * the directories the chart's claims became are made here, owned by the
//!     app's user, because the pods mount them with `type: Directory` and the
//!     kubelet would otherwise refuse to start them;
//!   * what the job needs afterwards (the port to forward, or the reason the
//!     chart was refused) goes to `--report` as JSON, since Helm drops a
//!     post-renderer's stderr and says only that it failed.

use clap::Parser;
use losos_ctl::apps::{render, shape, RunAs, ShapeCtx};
use std::io::{BufRead, Write};
use std::os::unix::fs::{chown, DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(version, about = "Shape a rendered Helm chart to run on a LosOS box")]
struct Cli {
    #[arg(long)]
    release: String,
    /// notshared or shared.
    #[arg(long)]
    run_as: String,
    /// Ports the box keeps for itself, comma-separated.
    #[arg(long, default_value = "")]
    reserved: String,
    /// Ports other apps listen on, as `port=app`, comma-separated.
    #[arg(long, default_value = "")]
    taken: String,
    /// Where to write what the install job needs to know.
    #[arg(long)]
    report: PathBuf,
}

fn ports(raw: &str) -> Vec<u16> {
    raw.split(',')
        .filter_map(|p| p.trim().parse().ok())
        .collect()
}

fn taken(raw: &str) -> Vec<(u16, String)> {
    raw.split(',')
        .filter_map(|pair| {
            let (port, app) = pair.split_once('=')?;
            Some((port.trim().parse().ok()?, app.trim().to_string()))
        })
        .collect()
}

fn report(path: &Path, value: &serde_json::Value) {
    if let Err(e) = std::fs::write(path, format!("{value}\n")) {
        eprintln!("losos-app-shape: writing {}: {e}", path.display());
    }
}

/// Make `dir` and every directory between the user's apps root and it, each
/// owned by `uid` and closed to everyone else. Only that span: the data
/// domain above it belongs to the system.
fn make_owned(apps_root: &Path, dir: &Path, uid: u32) -> std::io::Result<()> {
    let rel = dir.strip_prefix(apps_root).map_err(|_| {
        std::io::Error::other(format!("{} is outside the apps root", dir.display()))
    })?;
    let mut at = apps_root.to_path_buf();
    let mut chain = vec![at.clone()];
    for part in rel.components() {
        at.push(part);
        chain.push(at.clone());
    }
    for d in chain {
        match std::fs::DirBuilder::new().mode(0o750).create(&d) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        chown(&d, Some(uid), Some(uid))?;
        std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o750))?;
    }
    Ok(())
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    let Some(run_as) = RunAs::parse(&cli.run_as) else {
        eprintln!("losos-app-shape: --run-as must be notshared or shared");
        return std::process::ExitCode::from(2);
    };

    let mut docs = Vec::new();
    for line in std::io::stdin().lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("losos-app-shape: reading the chart: {e}");
                return std::process::ExitCode::FAILURE;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<serde_json::Value>(&line) {
            Ok(serde_json::Value::Null) => {}
            Ok(doc) => docs.push(doc),
            Err(e) => {
                eprintln!("losos-app-shape: a document that is not JSON: {e}");
                return std::process::ExitCode::FAILURE;
            }
        }
    }

    let ctx = ShapeCtx {
        release: cli.release.clone(),
        run_as,
        reserved: ports(&cli.reserved),
        taken: taken(&cli.taken),
    };
    let shaped = match shape(docs, &ctx) {
        Ok(s) => s,
        Err(refusal) => {
            report(
                &cli.report,
                &serde_json::json!({ "ok": false, "refusal": refusal.0 }),
            );
            eprintln!("losos-app-shape: {refusal}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let apps_root = PathBuf::from(run_as.apps_root());
    for dir in &shaped.dirs {
        if let Err(e) = make_owned(&apps_root, Path::new(dir), run_as.uid()) {
            let why = format!(
                "The app's folder could not be made in the {} side ({e}).",
                run_as.as_str()
            );
            report(
                &cli.report,
                &serde_json::json!({ "ok": false, "refusal": why }),
            );
            eprintln!("losos-app-shape: {why}");
            return std::process::ExitCode::FAILURE;
        }
    }

    report(
        &cli.report,
        &serde_json::json!({
            "ok": true,
            "port": shaped.primary,
            "ports": shaped.ports,
            "dirs": shaped.dirs,
        }),
    );
    let mut out = std::io::stdout().lock();
    if out.write_all(render(&shaped.docs).as_bytes()).is_err() {
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}
