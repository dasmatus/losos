//! Where a job's steps run. [`Podman`] is the real one: a rootless pod per
//! job, as the `shared` user, with the job's container and its services in
//! it. [`Host`] runs the steps as plain processes in the job's directory,
//! for the tests and for a box without Podman.

use anyhow::Context;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// The job's directories on the host. Each is mounted into the sandbox.
#[derive(Debug, Clone)]
pub struct Dirs {
    pub root: PathBuf,
    pub workspace: PathBuf,
    pub temp: PathBuf,
    pub actions: PathBuf,
    pub home: PathBuf,
}

impl Dirs {
    pub fn new(root: &Path) -> std::io::Result<Dirs> {
        let d = Dirs {
            root: root.to_path_buf(),
            workspace: root.join("workspace"),
            temp: root.join("temp"),
            actions: root.join("actions"),
            home: root.join("home"),
        };
        for p in [&d.workspace, &d.temp, &d.actions, &d.home] {
            std::fs::create_dir_all(p)?;
        }
        Ok(d)
    }
}

/// The same directories as the steps see them.
#[derive(Debug, Clone)]
pub struct Inside {
    pub workspace: String,
    pub temp: String,
    pub actions: String,
    pub home: String,
}

/// How a process ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Code(i32),
    TimedOut,
    Stopped,
}

/// A program to run in the sandbox.
pub struct Exec<'a> {
    pub argv: &'a [String],
    pub env: &'a BTreeMap<String, String>,
    pub cwd: &'a str,
    pub timeout: Duration,
}

pub trait Sandbox {
    fn inside(&self) -> &Inside;
    /// `$PATH` in the sandbox before the steps add to it.
    fn base_path(&self) -> String;
    /// The node binary JavaScript actions run with, if there is one.
    fn node(&self) -> Option<String>;
    fn exec(
        &mut self,
        e: &Exec<'_>,
        stop: &dyn Fn() -> bool,
        out: &mut dyn FnMut(&str),
    ) -> anyhow::Result<Exit>;
    /// A container action: `image` run once with the job's directories.
    fn run_image(
        &mut self,
        image: &str,
        entrypoint: Option<&str>,
        e: &Exec<'_>,
        stop: &dyn Fn() -> bool,
        out: &mut dyn FnMut(&str),
    ) -> anyhow::Result<Exit>;
    /// Build an action's Dockerfile; the image's name.
    fn build_image(
        &mut self,
        context: &Path,
        dockerfile: &str,
        out: &mut dyn FnMut(&str),
    ) -> anyhow::Result<String>;
    /// Take everything down.
    fn finish(&mut self);
}

/// Run `cmd`, handing each line of its output (both streams) to `out`
/// until it ends, `timeout` passes or `stop` says so.
pub fn stream(
    mut cmd: Command,
    timeout: Duration,
    stop: &dyn Fn() -> bool,
    out: &mut dyn FnMut(&str),
) -> anyhow::Result<Exit> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("starting the step")?;
    let (tx, rx) = mpsc::channel::<String>();
    let mut readers = Vec::new();
    let pipes: [Option<Box<dyn Read + Send>>; 2] = [
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    ];
    for pipe in pipes.into_iter().flatten() {
        let tx = tx.clone();
        readers.push(std::thread::spawn(move || {
            let mut r = BufReader::new(pipe);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match r.read_until(b'\n', &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let line = String::from_utf8_lossy(&buf);
                        let line = line.trim_end_matches(['\n', '\r']).to_string();
                        if tx.send(line).is_err() {
                            break;
                        }
                    }
                }
            }
        }));
    }
    drop(tx);
    let deadline = Instant::now() + timeout;
    let mut ended: Option<Exit> = None;
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => out(&line),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if ended.is_none() {
            if stop() {
                ended = Some(Exit::Stopped);
                let _ = child.kill();
            } else if Instant::now() >= deadline {
                ended = Some(Exit::TimedOut);
                let _ = child.kill();
            }
        }
    }
    for r in readers {
        let _ = r.join();
    }
    let status = child.wait()?;
    Ok(ended.unwrap_or(Exit::Code(status.code().unwrap_or(-1))))
}

// ── the host ──────────────────────────────────────────────────────────────

pub struct Host {
    inside: Inside,
}

impl Host {
    pub fn new(dirs: &Dirs) -> Host {
        let s = |p: &Path| p.to_string_lossy().into_owned();
        Host {
            inside: Inside {
                workspace: s(&dirs.workspace),
                temp: s(&dirs.temp),
                actions: s(&dirs.actions),
                home: s(&dirs.home),
            },
        }
    }
}

impl Sandbox for Host {
    fn inside(&self) -> &Inside {
        &self.inside
    }

    fn base_path(&self) -> String {
        std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into())
    }

    fn node(&self) -> Option<String> {
        std::env::var("LOSOS_CI_NODE").ok()
    }

    fn exec(
        &mut self,
        e: &Exec<'_>,
        stop: &dyn Fn() -> bool,
        out: &mut dyn FnMut(&str),
    ) -> anyhow::Result<Exit> {
        let (program, args) = e.argv.split_first().context("nothing to run")?;
        let mut cmd = Command::new(program);
        cmd.args(args).env_clear().envs(e.env).current_dir(e.cwd);
        stream(cmd, e.timeout, stop, out)
    }

    fn run_image(
        &mut self,
        _image: &str,
        _entrypoint: Option<&str>,
        _e: &Exec<'_>,
        _stop: &dyn Fn() -> bool,
        _out: &mut dyn FnMut(&str),
    ) -> anyhow::Result<Exit> {
        anyhow::bail!("container actions need Podman, and this runner has none")
    }

    fn build_image(
        &mut self,
        _context: &Path,
        _dockerfile: &str,
        _out: &mut dyn FnMut(&str),
    ) -> anyhow::Result<String> {
        anyhow::bail!("container actions need Podman, and this runner has none")
    }

    fn finish(&mut self) {}
}

// ── Podman ────────────────────────────────────────────────────────────────

/// What a job may use of the box that runs it.
#[derive(Debug, Clone)]
pub struct Limits {
    /// `--memory`, e.g. `2g`.
    pub memory: String,
    /// `--cpus`, e.g. `1.5`.
    pub cpus: String,
}

pub struct Podman {
    pod: String,
    job: String,
    dirs: Dirs,
    inside: Inside,
    path: String,
    node: Option<String>,
    limits: Limits,
    hosts: Vec<String>,
    steps: u32,
}

/// Split a container's `options:` string the way a shell would, quotes
/// and all, without running anything.
pub fn words(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut any = false;
    for c in s.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                any = true;
            }
            (None, c) if c.is_whitespace() => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The container options a workflow may pass on: health checks, the user
/// and plain environment. Anything else (`--privileged`, volumes, devices,
/// the network) is left out and said in the log.
pub fn allowed_options(options: &str, log: &mut dyn FnMut(&str)) -> Vec<String> {
    let w = words(options);
    let mut out = Vec::new();
    let mut i = 0;
    while i < w.len() {
        let opt = &w[i];
        let (name, inline) = match opt.split_once('=') {
            Some((n, v)) => (n.to_string(), Some(v.to_string())),
            None => (opt.clone(), None),
        };
        let ok = name.starts_with("--health-")
            || matches!(
                name.as_str(),
                "--user" | "-u" | "--env" | "-e" | "--workdir" | "-w"
            );
        let value = match inline {
            Some(v) => Some(v),
            None if i + 1 < w.len() && !w[i + 1].starts_with('-') => {
                i += 1;
                Some(w[i].clone())
            }
            None => None,
        };
        if ok {
            out.push(name);
            if let Some(v) = value {
                out.push(v);
            }
        } else {
            log(&format!(
                "Leaving out the container option {name}: LosOS does not pass it on."
            ));
        }
        i += 1;
    }
    out
}

fn podman() -> Command {
    Command::new("podman")
}

fn quiet(mut cmd: Command) -> anyhow::Result<String> {
    let out = cmd
        .stdin(Stdio::null())
        .output()
        .context("running podman")?;
    if !out.status.success() {
        anyhow::bail!(
            "podman: {}",
            String::from_utf8_lossy(&out.stderr)
                .trim()
                .lines()
                .last()
                .unwrap_or("")
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Pull `image` unless it is here, saying so in the log.
fn ensure_image(image: &str, log: &mut dyn FnMut(&str)) -> anyhow::Result<()> {
    let mut exists = podman();
    exists.args(["image", "exists", image]);
    if exists.status().map(|s| s.success()).unwrap_or(false) {
        return Ok(());
    }
    log(&format!("Pulling {image}"));
    let mut pull = podman();
    pull.args(["pull", "--quiet", image]);
    match stream(pull, Duration::from_secs(1800), &|| false, log)? {
        Exit::Code(0) => Ok(()),
        _ => anyhow::bail!("could not pull {image}"),
    }
}

impl Podman {
    /// The job's pod: its services, then its own container, idle until
    /// the steps are run in it with `podman exec`.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        id: &str,
        dirs: &Dirs,
        image: &str,
        container: &serde_json::Value,
        services: &serde_json::Value,
        limits: Limits,
        node: Option<String>,
        log: &mut dyn FnMut(&str),
    ) -> anyhow::Result<Podman> {
        let safe: String = id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let pod = format!("losos-ci-{safe}");
        let inside = Inside {
            workspace: "/github/workspace".into(),
            temp: "/github/runner_temp".into(),
            actions: "/github/actions".into(),
            home: "/github/home".into(),
        };
        let mut me = Podman {
            job: format!("{pod}-job"),
            pod,
            dirs: dirs.clone(),
            inside,
            path: String::new(),
            node,
            limits,
            hosts: Vec::new(),
            steps: 0,
        };
        let mut create = podman();
        create.args(["pod", "create", "--name", &me.pod, "--infra=true"]);
        quiet(create).context("making the job's pod")?;

        if let Some(services) = services.as_object() {
            for (name, svc) in services {
                let Some(img) = svc
                    .get("image")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                else {
                    continue;
                };
                ensure_image(img, log)?;
                let mut run = podman();
                run.args(["run", "--detach", "--pod", &me.pod])
                    .args(["--name", &format!("{}-svc-{name}", me.pod)])
                    .args([
                        "--memory",
                        "1g",
                        "--cpu-shares",
                        "256",
                        "--pids-limit",
                        "2048",
                    ]);
                if let Some(env) = svc.get("env").and_then(|v| v.as_object()) {
                    for (k, v) in env {
                        run.args(["--env", &format!("{k}={}", super::super::expr::to_text(v))]);
                    }
                }
                if let Some(opts) = svc.get("options").and_then(|v| v.as_str()) {
                    run.args(allowed_options(opts, log));
                }
                run.arg(img);
                quiet(run).with_context(|| format!("starting the service {name}"))?;
                me.hosts.push(name.clone());
                log(&format!("Service {name} started from {img}"));
            }
            me.wait_healthy(services, log)?;
        }

        ensure_image(image, log)?;
        let mut run = podman();
        run.args(["run", "--detach", "--pod", &me.pod, "--name", &me.job])
            .args(["--memory", &me.limits.memory, "--cpus", &me.limits.cpus])
            .args(["--cpu-shares", "256", "--pids-limit", "8192"])
            .args(me.mounts())
            .args(["--env", &format!("HOME={}", me.inside.home)])
            .args(["--workdir", &me.inside.workspace]);
        for host in &me.hosts {
            run.args(["--add-host", &format!("{host}:127.0.0.1")]);
        }
        if let Some(env) = container.get("env").and_then(|v| v.as_object()) {
            for (k, v) in env {
                run.args(["--env", &format!("{k}={}", super::super::expr::to_text(v))]);
            }
        }
        if let Some(opts) = container.get("options").and_then(|v| v.as_str()) {
            run.args(allowed_options(opts, log));
        }
        run.args(["--entrypoint", "tail", image, "-f", "/dev/null"]);
        quiet(run).context("starting the job's container")?;
        let mut path = podman();
        path.args(["exec", &me.job, "sh", "-c", "printf %s \"$PATH\""]);
        me.path = quiet(path).unwrap_or_else(|_| {
            "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".into()
        });
        Ok(me)
    }

    fn mounts(&self) -> Vec<String> {
        let m = |host: &Path, inside: &str| {
            vec![
                "--volume".to_string(),
                format!("{}:{inside}", host.display()),
            ]
        };
        let mut v = Vec::new();
        v.extend(m(&self.dirs.workspace, &self.inside.workspace));
        v.extend(m(&self.dirs.temp, &self.inside.temp));
        v.extend(m(&self.dirs.actions, &self.inside.actions));
        v.extend(m(&self.dirs.home, &self.inside.home));
        if self.node.is_some() {
            // JavaScript actions run with the box's own node, whatever the
            // image has: the store is read-only and holds no secret.
            v.push("--volume".into());
            v.push("/nix/store:/nix/store:ro".into());
        }
        v
    }

    /// GitHub waits for services with a health check to be healthy.
    fn wait_healthy(
        &self,
        services: &serde_json::Map<String, serde_json::Value>,
        log: &mut dyn FnMut(&str),
    ) -> anyhow::Result<()> {
        for (name, svc) in services {
            let checked = svc
                .get("options")
                .and_then(|v| v.as_str())
                .is_some_and(|o| o.contains("--health-cmd"));
            if !checked {
                continue;
            }
            let container = format!("{}-svc-{name}", self.pod);
            let deadline = Instant::now() + Duration::from_secs(300);
            log(&format!("Waiting for {name} to be healthy"));
            loop {
                let mut inspect = podman();
                inspect.args([
                    "inspect",
                    "--format",
                    "{{.State.Health.Status}}",
                    &container,
                ]);
                let status = quiet(inspect).unwrap_or_default();
                if status == "healthy" {
                    break;
                }
                if Instant::now() >= deadline {
                    anyhow::bail!("the service {name} never became healthy ({status})");
                }
                let mut check = podman();
                check.args(["healthcheck", "run", &container]);
                let _ = check.stdout(Stdio::null()).stderr(Stdio::null()).status();
                std::thread::sleep(Duration::from_secs(2));
            }
        }
        Ok(())
    }
}

impl Sandbox for Podman {
    fn inside(&self) -> &Inside {
        &self.inside
    }

    fn base_path(&self) -> String {
        self.path.clone()
    }

    fn node(&self) -> Option<String> {
        self.node.clone()
    }

    fn exec(
        &mut self,
        e: &Exec<'_>,
        stop: &dyn Fn() -> bool,
        out: &mut dyn FnMut(&str),
    ) -> anyhow::Result<Exit> {
        let mut cmd = podman();
        cmd.args(["exec", "--workdir", e.cwd]);
        for (k, v) in e.env {
            cmd.args(["--env", &format!("{k}={v}")]);
        }
        cmd.arg(&self.job).args(e.argv);
        stream(cmd, e.timeout, stop, out)
    }

    fn run_image(
        &mut self,
        image: &str,
        entrypoint: Option<&str>,
        e: &Exec<'_>,
        stop: &dyn Fn() -> bool,
        out: &mut dyn FnMut(&str),
    ) -> anyhow::Result<Exit> {
        ensure_image(image, out)?;
        self.steps += 1;
        let mut cmd = podman();
        cmd.args(["run", "--rm", "--pod", &self.pod])
            .args(["--name", &format!("{}-step-{}", self.pod, self.steps)])
            .args(["--memory", &self.limits.memory, "--cpus", &self.limits.cpus])
            .args(["--cpu-shares", "256", "--pids-limit", "8192"])
            .args(self.mounts())
            .args(["--workdir", e.cwd]);
        for host in &self.hosts {
            cmd.args(["--add-host", &format!("{host}:127.0.0.1")]);
        }
        for (k, v) in e.env {
            if k != "PATH" {
                cmd.args(["--env", &format!("{k}={v}")]);
            }
        }
        if let Some(ep) = entrypoint {
            cmd.args(["--entrypoint", ep]);
        }
        cmd.arg(image).args(e.argv);
        stream(cmd, e.timeout, stop, out)
    }

    fn build_image(
        &mut self,
        context: &Path,
        dockerfile: &str,
        out: &mut dyn FnMut(&str),
    ) -> anyhow::Result<String> {
        self.steps += 1;
        let tag = format!("localhost/{}-action-{}", self.pod, self.steps);
        let mut cmd = podman();
        cmd.args(["build", "--quiet", "--tag", &tag, "--file"])
            .arg(context.join(dockerfile))
            .arg(context);
        match stream(cmd, Duration::from_secs(1800), &|| false, out)? {
            Exit::Code(0) => Ok(tag),
            _ => anyhow::bail!("the action's image did not build"),
        }
    }

    fn finish(&mut self) {
        let mut rm = podman();
        rm.args(["pod", "rm", "--force", "--time", "5", &self.pod]);
        let _ = rm.stdout(Stdio::null()).stderr(Stdio::null()).status();
        let mut prune = podman();
        prune.args([
            "image",
            "prune",
            "--force",
            "--filter",
            &format!("reference=localhost/{}-action-*", self.pod),
        ]);
        let _ = prune.stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_are_split_and_filtered() {
        assert_eq!(
            words(r#"--health-cmd "pg_isready -U x" --health-interval=10s"#),
            vec!["--health-cmd", "pg_isready -U x", "--health-interval=10s"]
        );
        let mut said = Vec::new();
        let kept = allowed_options(
            r#"--privileged --health-cmd "redis-cli ping" -v /:/host --user 1000"#,
            &mut |l| said.push(l.to_string()),
        );
        assert_eq!(
            kept,
            vec!["--health-cmd", "redis-cli ping", "--user", "1000"]
        );
        assert_eq!(said.len(), 2);
    }

    #[test]
    fn the_host_streams_and_times_out() {
        let mut lines = Vec::new();
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "echo one; echo two >&2; exit 3"]);
        let exit = stream(cmd, Duration::from_secs(10), &|| false, &mut |l| {
            lines.push(l.to_string())
        })
        .unwrap();
        assert_eq!(exit, Exit::Code(3));
        lines.sort();
        assert_eq!(lines, vec!["one", "two"]);
        let mut cmd = Command::new("sleep");
        cmd.arg("10");
        let exit = stream(cmd, Duration::from_millis(300), &|| false, &mut |_| {}).unwrap();
        assert_eq!(exit, Exit::TimedOut);
    }
}
