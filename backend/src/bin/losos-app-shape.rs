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
//!
//! With `--podman` it is not a post-renderer: it makes a `notshared` app's
//! Podman Compose project ([`losos_ctl::compose`]) instead, from a chart
//! `helm template` rendered, or with `--compose` from the app's own Compose
//! file. The project goes to its directory under
//! [`losos_ctl::compose::PODMAN_ROOT`], owned by the app's user, and nothing
//! goes to stdout.

use clap::Parser;
use losos_ctl::apps::{render, shape, RunAs, ShapeCtx};
use losos_ctl::compose::{self, Project};
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
    /// Make a Podman Compose project rather than shaping for Helm.
    #[arg(long)]
    podman: bool,
    /// With --podman: stdin is the app's Compose file, not a rendered chart.
    #[arg(long, requires = "podman")]
    compose: bool,
    /// With --compose: the folder of files the app ships beside it.
    #[arg(long, requires = "compose")]
    source: Option<PathBuf>,
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

/// Every file under `root`, relative to it.
fn walk(root: &Path) -> std::io::Result<Vec<String>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };
        for entry in entries {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                stack.push(entry.path());
            } else if kind.is_file() {
                if let Ok(rel) = entry.path().strip_prefix(root) {
                    out.push(rel.to_string_lossy().into_owned());
                }
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Give `path` and everything under it to `uid`, closed to everyone else.
/// Symlinks are not followed: the tree is the box's own, but it never hurts.
fn own_tree(path: &Path, uid: u32) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return Ok(());
    }
    std::os::unix::fs::lchown(path, Some(uid), Some(uid))?;
    let mode = if meta.is_dir() { 0o700 } else { 0o600 };
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    if meta.is_dir() {
        for entry in std::fs::read_dir(path)? {
            own_tree(&entry?.path(), uid)?;
        }
    }
    Ok(())
}

/// Write the project into a fresh directory, then swap it in for the old
/// one. The data folders are made in place and never removed.
fn write_project(
    project: &Project,
    run_as: RunAs,
    release: &str,
    source: Option<&Path>,
) -> Result<(), String> {
    let uid = run_as.uid();
    let dir = PathBuf::from(compose::project_dir(run_as, release));
    let staged = dir.with_extension("new");
    let io = |what: &str, e: std::io::Error| format!("{what} ({e})");
    if staged.exists() {
        std::fs::remove_dir_all(&staged).map_err(|e| io("clearing a half-made project", e))?;
    }
    let files = staged.join(compose::FILES_DIR);
    std::fs::create_dir_all(&files).map_err(|e| io("making the project", e))?;
    if let Some(source) = source {
        for rel in walk(source).map_err(|e| io("reading the app's files", e))? {
            let to = files.join(&rel);
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent).map_err(|e| io("copying the app's files", e))?;
            }
            std::fs::copy(source.join(&rel), &to).map_err(|e| io("copying the app's files", e))?;
        }
    }
    for f in &project.files {
        let to = files.join(&f.path);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io("writing the app's settings", e))?;
        }
        std::fs::write(&to, &f.bytes).map_err(|e| io("writing the app's settings", e))?;
    }
    for d in &project.scratch {
        let rel = Path::new(d)
            .strip_prefix(&dir)
            .map_err(|_| format!("{d} is outside the project"))?;
        std::fs::create_dir_all(staged.join(rel)).map_err(|e| io("making scratch space", e))?;
    }
    let body = serde_json::to_vec_pretty(&project.compose).expect("a Value always encodes");
    std::fs::write(staged.join(compose::COMPOSE_FILE), body)
        .map_err(|e| io("writing compose.json", e))?;
    let images: String = project.images.iter().map(|i| format!("{i}\n")).collect();
    std::fs::write(staged.join(compose::IMAGES_FILE), images)
        .map_err(|e| io("writing images.txt", e))?;
    own_tree(&staged, uid).map_err(|e| io("handing the project to its user", e))?;

    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| io("clearing the old project", e))?;
    }
    std::fs::rename(&staged, &dir).map_err(|e| io("putting the project in place", e))?;

    let apps_root = PathBuf::from(run_as.apps_root());
    for d in &project.dirs {
        make_owned(&apps_root, Path::new(d), uid).map_err(|e| {
            format!(
                "The app's folder could not be made in the {} side ({e}).",
                run_as.as_str()
            )
        })?;
    }
    Ok(())
}

fn podman(
    cli: &Cli,
    run_as: RunAs,
    ctx: &ShapeCtx,
    docs: Vec<serde_json::Value>,
) -> std::process::ExitCode {
    let fail = |why: String| {
        report(
            &cli.report,
            &serde_json::json!({ "ok": false, "refusal": why }),
        );
        eprintln!("losos-app-shape: {why}");
        std::process::ExitCode::FAILURE
    };
    let project = if cli.compose {
        let Some(doc) = docs.into_iter().next() else {
            return fail("The compose file is empty.".into());
        };
        let shipped = match cli.source.as_deref().map(walk).transpose() {
            Ok(list) => list.unwrap_or_default(),
            Err(e) => return fail(format!("The app's files could not be read ({e}).")),
        };
        compose::shape_compose(doc, ctx, &shipped)
    } else {
        shape(docs, ctx).and_then(|shaped| compose::from_kube(shaped, ctx))
    };
    let project = match project {
        Ok(p) => p,
        Err(refusal) => return fail(refusal.0),
    };
    if let Err(why) = write_project(&project, run_as, &cli.release, cli.source.as_deref()) {
        return fail(why);
    }
    report(
        &cli.report,
        &serde_json::json!({
            "ok": true,
            "port": project.primary,
            "ports": project.ports,
            "dirs": project.dirs,
            "images": project.images,
        }),
    );
    std::process::ExitCode::SUCCESS
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
    if cli.podman {
        return podman(&cli, run_as, &ctx, docs);
    }
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
