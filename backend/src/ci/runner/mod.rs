//! The runner: `losos-ci runner`, as the `shared` user. It asks the
//! daemon for work over the runner socket, unpacks the commit into a job
//! directory, runs the steps ([`job`]) in a rootless Podman pod
//! ([`sandbox`]) and reports the log and the result as it goes.

pub mod commands;
pub mod job;
pub mod sandbox;

use super::net::{unix_http, Report};
use anyhow::Context;
use sandbox::{Dirs, Host, Limits, Podman, Sandbox};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How the runner is set up (`modules/ci.nix`).
#[derive(Debug, Clone)]
pub struct Config {
    pub socket: PathBuf,
    /// Job directories go here, one per job, removed after it.
    pub work: PathBuf,
    /// This box, `runner.name`.
    pub name: String,
    /// `podman` or `host`.
    pub sandbox: String,
    /// The image a job runs in when its `runs-on` names no other.
    pub default_image: String,
    /// The image's archive, loaded once.
    pub default_image_archive: Option<PathBuf>,
    /// `runs-on` label to image.
    pub images: BTreeMap<String, String>,
    pub limits: Limits,
    pub node: Option<String>,
}

/// The most of a log one report carries; the edge takes 512 KiB.
const MAX_CHUNK: usize = 256 * 1024;

fn post(socket: &Path, path: &str, body: &Value, wait: Duration) -> anyhow::Result<(u16, Vec<u8>)> {
    unix_http(
        socket,
        "POST",
        path,
        Some("application/json"),
        &serde_json::to_vec(body)?,
        40 * 1024 * 1024,
        wait,
    )
}

/// A job as the daemon hands it over.
struct Work {
    kind: String,
    id: String,
    origin: String,
    name: String,
    payload: Value,
    timeout: Duration,
    archive: Vec<u8>,
}

fn claim(socket: &Path) -> anyhow::Result<Option<Work>> {
    use base64::Engine as _;
    let (code, body) = post(
        socket,
        "/runner/claim",
        &json!({ "wait": 20 }),
        Duration::from_secs(90),
    )?;
    match code {
        204 => Ok(None),
        200 => {
            let v: Value = serde_json::from_slice(&body)?;
            let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
            Ok(Some(Work {
                kind: s("kind"),
                id: s("id"),
                origin: s("origin"),
                name: s("name"),
                payload: v["payload"].clone(),
                timeout: Duration::from_secs(v["timeoutSecs"].as_u64().unwrap_or(3600).max(60)),
                archive: base64::engine::general_purpose::STANDARD
                    .decode(s("archive").as_bytes())
                    .context("the archive is not base64")?,
            }))
        }
        _ => anyhow::bail!(
            "the daemon answered {code}: {}",
            String::from_utf8_lossy(&body)
        ),
    }
}

fn report(socket: &Path, kind: &str, id: &str, r: &Report) -> anyhow::Result<bool> {
    let mut body = serde_json::to_value(r)?;
    body["kind"] = json!(kind);
    body["id"] = json!(id);
    let (code, raw) = post(socket, "/runner/report", &body, Duration::from_secs(60))?;
    if code != 200 {
        anyhow::bail!(
            "the daemon answered {code}: {}",
            String::from_utf8_lossy(&raw)
        );
    }
    let v: Value = serde_json::from_slice(&raw)?;
    Ok(v["stop"].as_bool().unwrap_or(false))
}

/// Unpack a gzipped tar into `dir`, refusing paths that leave it.
pub fn unpack(archive: &[u8], dir: &Path, strip_first: bool) -> anyhow::Result<()> {
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    tar.set_preserve_permissions(true);
    for entry in tar.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let mut parts = path.components();
        if strip_first {
            parts.next();
        }
        let rel: PathBuf = parts.collect();
        if rel.as_os_str().is_empty() {
            continue;
        }
        if rel
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            anyhow::bail!(
                "the archive names a path outside its directory: {}",
                path.display()
            );
        }
        let to = dir.join(&rel);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        entry.unpack(&to)?;
    }
    Ok(())
}

/// An action from GitHub: the tarball of `repo` at `git_ref`, by curl.
fn fetch_action(repo: &str, git_ref: &str, dest: &Path) -> anyhow::Result<()> {
    let url = format!("https://codeload.github.com/{repo}/tar.gz/{git_ref}");
    let out = std::process::Command::new("curl")
        .args(["--silent", "--show-error", "--fail", "--location"])
        .args(["--proto", "=https", "--max-time", "300"])
        .args(["--max-filesize", &(200 * 1024 * 1024).to_string()])
        .arg(&url)
        .stdin(std::process::Stdio::null())
        .output()
        .context("running curl")?;
    if !out.status.success() {
        anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    std::fs::create_dir_all(dest)?;
    unpack(&out.stdout, dest, true)
}

/// Load the default image from its archive unless Podman has it.
fn load_default_image(cfg: &Config) {
    let Some(archive) = &cfg.default_image_archive else {
        return;
    };
    let exists = std::process::Command::new("podman")
        .args(["image", "exists", &cfg.default_image])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if exists {
        return;
    }
    tracing::info!(image = %cfg.default_image, "loading the default image");
    let ok = std::process::Command::new("podman")
        .args(["load", "--quiet", "--input"])
        .arg(archive)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        tracing::warn!("could not load {}", archive.display());
    }
}

/// The image for a job: its `container:`, else the first `runs-on` label
/// the box has an image for, else the default.
fn image_for(cfg: &Config, payload: &Value) -> String {
    let container = &payload["container"];
    let from_container = match container {
        Value::String(s) => Some(s.clone()),
        Value::Object(m) => m.get("image").and_then(Value::as_str).map(str::to_string),
        _ => None,
    };
    if let Some(img) = from_container.filter(|s| !s.is_empty()) {
        return img;
    }
    payload["runsOn"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .find_map(|l| cfg.images.get(&l.to_ascii_lowercase()).cloned())
        .unwrap_or_else(|| cfg.default_image.clone())
}

/// One job, start to end. The log goes to the daemon every two seconds.
fn run_one(cfg: &Config, work: Work) {
    let buf = Arc::new(Mutex::new(String::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));
    let flusher = {
        let (buf, stop, done) = (buf.clone(), stop.clone(), done.clone());
        let (socket, kind, id) = (cfg.socket.clone(), work.kind.clone(), work.id.clone());
        std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_secs(2));
                let chunk = take_chunk(&buf);
                match report(
                    &socket,
                    &kind,
                    &id,
                    &Report {
                        log: chunk,
                        ..Report::default()
                    },
                ) {
                    Ok(true) => stop.store(true, Ordering::Relaxed),
                    Ok(false) => {}
                    Err(e) => tracing::warn!("reporting the log: {e:#}"),
                }
            }
        })
    };
    let say = |line: &str| {
        let mut b = buf.lock().unwrap_or_else(|e| e.into_inner());
        b.push_str(line);
        b.push('\n');
    };
    say(&format!(
        "Running {} from {} on {}.",
        work.name, work.origin, cfg.name
    ));

    let root = cfg.work.join(format!(
        "{}-{}",
        work.kind,
        work.id.replace(['/', '.'], "_")
    ));
    let _ = std::fs::remove_dir_all(&root);
    let outcome = (|| -> anyhow::Result<job::Outcome> {
        let dirs = Dirs::new(&root)?;
        unpack(&work.archive, &dirs.workspace, false).context("unpacking the commit")?;
        let mut sandbox: Box<dyn Sandbox> = if cfg.sandbox == "host" {
            Box::new(Host::new(&dirs))
        } else {
            let image = image_for(cfg, &work.payload);
            say(&format!("##[group]Starting the job's container ({image})"));
            let mut log = |l: &str| say(l);
            let p = Podman::start(
                &format!("{}-{}", work.kind, work.id),
                &dirs,
                &image,
                &work.payload["container"],
                &work.payload["services"],
                cfg.limits.clone(),
                cfg.node.clone(),
                &mut log,
            );
            say("##[endgroup]");
            Box::new(p?)
        };
        let deadline = Instant::now() + work.timeout;
        let stop_now = || stop.load(Ordering::Relaxed) || Instant::now() >= deadline;
        let mut log = |l: &str| say(l);
        let mut j = job::Job {
            payload: &work.payload,
            dirs: &dirs,
            runner_name: &cfg.name,
            sandbox: sandbox.as_mut(),
            fetch: &fetch_action,
            stop: &stop_now,
            log: &mut log,
        };
        let mut out = job::run(&mut j);
        sandbox.finish();
        if Instant::now() >= deadline && !stop.load(Ordering::Relaxed) {
            say("##[error]The job ran past its timeout-minutes.");
            out.state = "failure";
        }
        Ok(out)
    })();
    let _ = std::fs::remove_dir_all(&root);
    let (state, outputs) = match outcome {
        Ok(o) => (o.state, o.outputs),
        Err(e) => {
            say(&format!("##[error]{e:#}"));
            ("error", Map::new())
        }
    };
    say(&format!("The job ended: {state}."));
    done.store(true, Ordering::Relaxed);
    let _ = flusher.join();
    // What is left of the log, then the end.
    loop {
        let chunk = take_chunk(&buf);
        let last = buf.lock().map(|b| b.is_empty()).unwrap_or(true);
        let r = Report {
            log: chunk,
            state: last.then(|| state.to_string()),
            exit_code: last.then_some(if state == "success" { 0 } else { 1 }),
            outputs: last.then(|| outputs.clone()),
        };
        let mut tries = 0;
        while let Err(e) = report(&cfg.socket, &work.kind, &work.id, &r) {
            tries += 1;
            tracing::warn!("reporting the end: {e:#}");
            if tries >= 10 {
                break;
            }
            std::thread::sleep(Duration::from_secs(3));
        }
        if last {
            break;
        }
    }
}

/// Up to [`MAX_CHUNK`] bytes off the front of the buffer, on a character
/// boundary.
fn take_chunk(buf: &Mutex<String>) -> String {
    let mut b = buf.lock().unwrap_or_else(|e| e.into_inner());
    if b.len() <= MAX_CHUNK {
        return std::mem::take(&mut *b);
    }
    let mut cut = MAX_CHUNK;
    while !b.is_char_boundary(cut) {
        cut -= 1;
    }
    let rest = b.split_off(cut);
    std::mem::replace(&mut *b, rest)
}

/// Take jobs one at a time, forever.
pub fn run(cfg: Config) -> anyhow::Result<()> {
    std::fs::create_dir_all(&cfg.work)?;
    if cfg.sandbox != "host" {
        load_default_image(&cfg);
    }
    loop {
        match claim(&cfg.socket) {
            Ok(Some(work)) => run_one(&cfg, work),
            Ok(None) => {}
            Err(e) => {
                tracing::warn!("asking for work: {e:#}");
                std::thread::sleep(Duration::from_secs(5));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_and_images() {
        let buf = Mutex::new("é".repeat(MAX_CHUNK));
        let first = take_chunk(&buf);
        assert!(first.len() <= MAX_CHUNK);
        assert_eq!(first.len() + take_chunk(&buf).len(), MAX_CHUNK * 2);
        let cfg = Config {
            socket: PathBuf::new(),
            work: PathBuf::new(),
            name: "b".into(),
            sandbox: "host".into(),
            default_image: "losos-ci-base".into(),
            default_image_archive: None,
            images: [(
                "ubuntu-latest".to_string(),
                "docker.io/library/ubuntu:24.04".to_string(),
            )]
            .into_iter()
            .collect(),
            limits: Limits {
                memory: "2g".into(),
                cpus: "1".into(),
            },
            node: None,
        };
        assert_eq!(
            image_for(&cfg, &json!({ "runsOn": ["ubuntu-latest"] })),
            "docker.io/library/ubuntu:24.04"
        );
        assert_eq!(
            image_for(&cfg, &json!({ "runsOn": ["self-hosted"] })),
            "losos-ci-base"
        );
        assert_eq!(
            image_for(&cfg, &json!({ "container": "node:20", "runsOn": [] })),
            "node:20"
        );
        assert_eq!(
            image_for(&cfg, &json!({ "container": { "image": "x:1" } })),
            "x:1"
        );
    }

    #[test]
    fn unpacking_refuses_escapes() {
        let mut raw = Vec::new();
        {
            let gz = flate2::write::GzEncoder::new(&mut raw, flate2::Compression::fast());
            let mut tar = tar::Builder::new(gz);
            let mut h = tar::Header::new_gnu();
            h.set_size(2);
            h.set_mode(0o644);
            // Header::set_path refuses `..`, so write the name by hand.
            let name = b"top/../../evil";
            h.as_old_mut().name[..name.len()].copy_from_slice(name);
            h.set_cksum();
            tar.append(&h, &b"hi"[..]).unwrap();
            tar.into_inner().unwrap().finish().unwrap();
        }
        let dir = std::env::temp_dir().join(format!("losos-ci-unpack-{}", std::process::id()));
        assert!(unpack(&raw, &dir, true).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
