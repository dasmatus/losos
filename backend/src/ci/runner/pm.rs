//! A job's containers, through pm (github.com/losos-project/pm).
//!
//! The runner starts and stops nothing itself. Every container of a job is
//! described in a compose file and handed to `pm container`, which asks
//! Mesh CI's pm plugin (backend/plugins/pm-ci/) what to run and runs it, as
//! the `shared` user, rootless. The plugin answers only for containers named
//! `lci-…`, so this is the whole of what the runner's pm can touch.
//!
//! One job is one compose file, `lci-<job>.yml`:
//!
//! * `job`, the container the steps run in, on a network of its own. Its
//!   entrypoint is [`AGENT`], a small shell loop that runs the steps the
//!   runner hands over in a directory both can see, one at a time, writing
//!   each one's output and exit code beside it. Nothing else has to reach
//!   into the container, so pm needs no request beyond starting and
//!   stopping it.
//! * one container per service, in the job container's network namespace:
//!   a step reaches `postgres` by name and on `localhost`, as GitHub's own
//!   runners allow. Health checks are passed on and waited for.
//!
//! A container action (`docker://…` or a Dockerfile) is a compose file of its
//! own, `lci-<job>-a<n>.action.yml`, in the job container's network, run
//! once; the plugin removes the image built for it when it is taken down.

use super::sandbox::{stream, words, Dirs, Exec, Exit, Inside, Limits, Sandbox};
use anyhow::Context;
use serde_json::{json, Map, Value};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// pm and the configuration directory that holds Mesh CI's plugin and the
/// key it is signed with (nix/ci-pm-plugin.nix).
#[derive(Debug, Clone)]
pub struct Tool {
    pub bin: PathBuf,
    pub config: PathBuf,
}

impl Tool {
    /// pm with `config` as its configuration directory.
    pub fn command(&self, config: &Path) -> Command {
        let mut c = Command::new(&self.bin);
        c.env("XDG_CONFIG_HOME", config)
            .env("RUST_LOG", "warn")
            .stdin(Stdio::null());
        c
    }

    /// `pm container <args>`, answered by Mesh CI's plugin.
    pub fn container(&self, args: &[&str]) -> Command {
        let mut c = self.command(&self.config);
        c.arg("container").args(args);
        c
    }

    /// What `pm container <args>` printed, or why it failed.
    fn quiet(&self, args: &[&str]) -> anyhow::Result<String> {
        let out = self
            .container(args)
            .output()
            .with_context(|| format!("running pm container {}", args.join(" ")))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!(
                "pm container {}: {}",
                args.first().copied().unwrap_or(""),
                err.trim().lines().last().unwrap_or("failed")
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// Take down whatever an earlier runner left behind: every `lci-…`
    /// container there is.
    pub fn sweep(&self) {
        let Ok(names) = self.quiet(&["list"]) else {
            return;
        };
        for name in names
            .lines()
            .map(str::trim)
            .filter(|n| n.starts_with("lci-"))
        {
            let _ = self.quiet(&["remove", name]);
        }
    }
}

/// The agent in every job container. It is the container's first process:
/// a step is ended with everything it started by signalling every other
/// process in the container (`kill -1`), and `done` ends the agent itself.
pub const AGENT: &str = r#"# LosOS Mesh CI: runs the steps the runner hands over, one at a time.
d=/github/losos-ci
trap 'exit 0' TERM INT
printf %s "$PATH" > "$d/path.new" && mv "$d/path.new" "$d/path"
nap() { sleep 0.2 2>/dev/null || sleep 1; }
n=1
while [ ! -e "$d/done" ]; do
  s="$d/steps/$n"
  if [ ! -e "$s/ready" ]; then nap; continue; fi
  ( /bin/sh "$s/run.sh" < /dev/null > "$s/log" 2>&1
    printf %s "$?" > "$s/code.new" && mv "$s/code.new" "$s/code" ) &
  pid=$!
  while [ ! -e "$s/code" ]; do
    if [ -e "$s/cancel" ]; then
      kill -TERM -1 2>/dev/null
      i=0
      while [ ! -e "$s/code" ] && [ $i -lt 25 ]; do nap; i=$((i+1)); done
      kill -KILL -1 2>/dev/null
      break
    fi
    nap
  done
  wait "$pid" 2>/dev/null
  [ -e "$s/code" ] || { printf 143 > "$s/code.new" && mv "$s/code.new" "$s/code"; }
  n=$((n+1))
done
"#;

/// Where the agent's directory is, inside.
const AGENT_DIR: &str = "/github/losos-ci";

/// A short, stable name for a job: `lci-` and ten hex digits of its id.
pub fn job_name(id: &str) -> String {
    // FNV-1a: no dependency, and the same id always gives the same name.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in id.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("lci-{:010x}", h & 0xff_ffff_ffff)
}

/// A service's label as part of a container name.
fn label(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .take(30)
        .collect();
    if s.is_empty() {
        "service".into()
    } else {
        s
    }
}

/// The container options a workflow may pass on, as compose keys: health
/// checks, the user, plain environment and the working directory. Anything
/// else (`--privileged`, volumes, devices, the network) is left out and said
/// in the log.
pub fn compose_options(options: &str, log: &mut dyn FnMut(&str)) -> Map<String, Value> {
    let w = words(options);
    let mut out = Map::new();
    let mut health = Map::new();
    let mut env = Map::new();
    let mut i = 0;
    while i < w.len() {
        let (name, inline) = match w[i].split_once('=') {
            Some((n, v)) => (n.to_string(), Some(v.to_string())),
            None => (w[i].clone(), None),
        };
        let value = match inline {
            Some(v) => Some(v),
            None if i + 1 < w.len() && !w[i + 1].starts_with('-') => {
                i += 1;
                Some(w[i].clone())
            }
            None => None,
        };
        let v = value.clone().unwrap_or_default();
        match name.as_str() {
            "--health-cmd" => {
                health.insert("test".into(), json!(["CMD-SHELL", v]));
            }
            "--health-interval" => {
                health.insert("interval".into(), json!(v));
            }
            "--health-timeout" => {
                health.insert("timeout".into(), json!(v));
            }
            "--health-start-period" => {
                health.insert("start_period".into(), json!(v));
            }
            "--health-retries" => {
                health.insert("retries".into(), json!(v.parse::<u32>().unwrap_or(3)));
            }
            "--user" | "-u" => {
                out.insert("user".into(), json!(v));
            }
            "--workdir" | "-w" => {
                out.insert("working_dir".into(), json!(v));
            }
            "--env" | "-e" => {
                if let Some((k, val)) = v.split_once('=') {
                    env.insert(k.to_string(), json!(val));
                }
            }
            _ => log(&format!(
                "Leaving out the container option {name}: LosOS does not pass it on."
            )),
        }
        i += 1;
    }
    if health.contains_key("test") {
        out.insert("healthcheck".into(), Value::Object(health));
    }
    if !env.is_empty() {
        out.insert("environment".into(), Value::Object(env));
    }
    out
}

/// A workflow's `env:` map as compose's.
fn env_of(v: Option<&Value>) -> Map<String, Value> {
    v.and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .map(|(k, v)| (k.clone(), json!(super::super::expr::to_text(v))))
                .collect()
        })
        .unwrap_or_default()
}

/// Merge `extra`'s environment into `into`'s.
fn merge_env(into: &mut Map<String, Value>, extra: Map<String, Value>) {
    let slot = into
        .entry("environment")
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(m) = slot.as_object_mut() {
        m.extend(extra);
    }
}

/// `'s'`, for sh.
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// A name sh takes for a variable.
fn is_var(k: &str) -> bool {
    let mut c = k.chars();
    c.next()
        .is_some_and(|f| f.is_ascii_alphabetic() || f == '_')
        && c.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The script the agent runs for one step.
pub fn step_script(e: &Exec<'_>) -> String {
    let mut s = String::from("cd -- ");
    s.push_str(&quote(e.cwd));
    s.push_str(" || exit 1\n");
    for (k, v) in e.env {
        if is_var(k) {
            s.push_str(&format!("export {k}={}\n", quote(v)));
        }
    }
    s.push_str("exec");
    for a in e.argv {
        s.push(' ');
        s.push_str(&quote(a));
    }
    s.push('\n');
    s
}

/// Lines out of a container's k8s-file log: `<time> <stream> <P|F> <text>`,
/// where P is part of a line that goes on in the next.
fn k8s_lines(raw: &str, partial: &mut String, out: &mut dyn FnMut(&str)) {
    for line in raw.lines() {
        let mut parts = line.splitn(4, ' ');
        let (_, _, tag) = (parts.next(), parts.next(), parts.next());
        let text = parts.next().unwrap_or("");
        partial.push_str(text);
        if tag != Some("P") {
            out(partial);
            partial.clear();
        }
    }
}

/// Podman's own chatter that says nothing to a workflow's author: the IDs of
/// the containers it made.
fn noise(line: &str) -> bool {
    line.len() == 64 && line.bytes().all(|b| b.is_ascii_hexdigit())
}

pub struct Pm {
    tool: Tool,
    name: String,
    dirs: Dirs,
    /// The agent's directory on the host.
    agent: PathBuf,
    /// The compose files.
    files: PathBuf,
    inside: Inside,
    path: String,
    node: Option<String>,
    limits: Limits,
    steps: u32,
    actions: u32,
    /// A Dockerfile action's image, built when it is run.
    pending: Option<(String, PathBuf, String)>,
}

impl Pm {
    /// The job's containers: its own and its services', up and healthy,
    /// with the agent waiting for the first step.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        tool: Tool,
        id: &str,
        dirs: &Dirs,
        image: &str,
        container: &Value,
        services: &Value,
        limits: Limits,
        node: Option<String>,
        log: &mut dyn FnMut(&str),
    ) -> anyhow::Result<Pm> {
        let name = job_name(id);
        let agent = dirs.root.join("agent");
        let files = dirs.root.join("lci");
        std::fs::create_dir_all(agent.join("steps"))?;
        std::fs::create_dir_all(&files)?;
        // The container's user may not be root, and only this job's
        // directory (0700, the shared user's) leads here.
        set_mode(&agent, 0o777)?;
        set_mode(&agent.join("steps"), 0o777)?;
        std::fs::write(agent.join("agent.sh"), AGENT)?;
        let mut me = Pm {
            tool,
            name,
            dirs: dirs.clone(),
            agent,
            files,
            inside: Inside {
                workspace: "/github/workspace".into(),
                temp: "/github/runner_temp".into(),
                actions: "/github/actions".into(),
                home: "/github/home".into(),
            },
            path: String::new(),
            node,
            limits,
            steps: 0,
            actions: 0,
            pending: None,
        };
        let compose = me.job_compose(image, container, services, log);
        let file = me.files.join(format!("{}.yml", me.name));
        std::fs::write(&file, serde_json::to_vec_pretty(&compose)?)?;
        let mut up = me.tool.container(&["up"]);
        up.arg(&file);
        let quiet = |l: &str| {
            if !noise(l) {
                log(l)
            }
        };
        let mut quiet = quiet;
        match stream(up, Duration::from_secs(1800), &|| false, &mut quiet)? {
            Exit::Code(0) => {}
            _ => anyhow::bail!("the job's containers did not start"),
        }
        me.wait_agent()?;
        if let Some(services) = services.as_object() {
            me.wait_healthy(services, log)?;
        }
        me.path = std::fs::read_to_string(me.agent.join("path")).unwrap_or_else(|_| {
            "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".into()
        });
        Ok(me)
    }

    fn container_name(&self, what: &str) -> String {
        format!("{}-{what}", self.name)
    }

    fn mounts(&self) -> Vec<Value> {
        let m = |host: &Path, inside: &str| json!(format!("{}:{inside}", host.display()));
        let mut v = vec![
            m(&self.dirs.workspace, &self.inside.workspace),
            m(&self.dirs.temp, &self.inside.temp),
            m(&self.dirs.actions, &self.inside.actions),
            m(&self.dirs.home, &self.inside.home),
        ];
        if self.node.is_some() {
            // JavaScript actions run with the box's own node, whatever the
            // image has: the store is read-only and holds no secret.
            v.push(json!("/nix/store:/nix/store:ro"));
        }
        v
    }

    /// The job's compose file.
    pub fn job_compose(
        &self,
        image: &str,
        container: &Value,
        services: &Value,
        log: &mut dyn FnMut(&str),
    ) -> Value {
        let labels: Vec<(String, &Value)> = services
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(_, s)| {
                s.get("image")
                    .and_then(Value::as_str)
                    .is_some_and(|i| !i.is_empty())
            })
            .map(|(k, s)| (k.clone(), s))
            .collect();

        let mut job = Map::new();
        job.insert("image".into(), json!(image));
        job.insert("container_name".into(), json!(self.container_name("job")));
        job.insert(
            "entrypoint".into(),
            json!(["/bin/sh", format!("{AGENT_DIR}/agent.sh")]),
        );
        job.insert("working_dir".into(), json!(self.inside.workspace));
        let mut volumes = self.mounts();
        volumes.push(json!(format!("{}:{AGENT_DIR}", self.agent.display())));
        job.insert("volumes".into(), Value::Array(volumes));
        job.insert("mem_limit".into(), json!(self.limits.memory));
        job.insert("cpus".into(), json!(self.limits.cpus));
        job.insert("cpu_shares".into(), json!(256));
        job.insert("pids_limit".into(), json!(8192));
        if !labels.is_empty() {
            job.insert(
                "extra_hosts".into(),
                Value::Array(
                    labels
                        .iter()
                        .map(|(l, _)| json!(format!("{l}:127.0.0.1")))
                        .collect(),
                ),
            );
        }
        let mut env = Map::new();
        env.insert("HOME".into(), json!(self.inside.home));
        job.insert("environment".into(), Value::Object(env));
        merge_env(&mut job, env_of(container.get("env")));
        if let Some(opts) = container.get("options").and_then(Value::as_str) {
            let o = compose_options(opts, log);
            for (k, v) in o {
                if k == "environment" {
                    if let Value::Object(m) = v {
                        merge_env(&mut job, m);
                    }
                } else {
                    job.insert(k, v);
                }
            }
        }

        let mut all = Map::new();
        all.insert("job".into(), Value::Object(job));
        for (name, svc) in labels {
            let mut s = Map::new();
            s.insert("image".into(), svc["image"].clone());
            s.insert(
                "container_name".into(),
                json!(self.container_name(&format!("s-{}", label(&name)))),
            );
            s.insert("network_mode".into(), json!("service:job"));
            s.insert("depends_on".into(), json!(["job"]));
            s.insert("mem_limit".into(), json!("1g"));
            s.insert("cpu_shares".into(), json!(256));
            s.insert("pids_limit".into(), json!(2048));
            let env = env_of(svc.get("env"));
            if !env.is_empty() {
                s.insert("environment".into(), Value::Object(env));
            }
            if let Some(opts) = svc.get("options").and_then(Value::as_str) {
                for (k, v) in compose_options(opts, log) {
                    if k == "environment" {
                        if let Value::Object(m) = v {
                            merge_env(&mut s, m);
                        }
                    } else {
                        s.insert(k, v);
                    }
                }
            }
            all.insert(format!("s-{}", label(&name)), Value::Object(s));
        }
        json!({
            "name": self.name,
            "x-podman": { "in_pod": false },
            "services": all,
        })
    }

    /// The state the plugin prints: `running`, `exited 1`, `running healthy`.
    fn status(&self, container: &str) -> anyhow::Result<String> {
        Ok(self
            .tool
            .quiet(&["status", container])?
            .lines()
            .last()
            .unwrap_or("")
            .to_string())
    }

    /// The agent says where `$PATH` is once it is up.
    fn wait_agent(&self) -> anyhow::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(120);
        let job = self.container_name("job");
        let mut checked = Instant::now();
        while !self.agent.join("path").exists() {
            if Instant::now() >= deadline {
                anyhow::bail!("the job's container never started its steps");
            }
            if checked.elapsed() >= Duration::from_secs(3) {
                checked = Instant::now();
                let state = self.status(&job).unwrap_or_default();
                if state.starts_with("exited") {
                    anyhow::bail!("the job's container stopped ({state}): its image needs /bin/sh");
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        Ok(())
    }

    /// GitHub waits for services with a health check to be healthy.
    fn wait_healthy(
        &self,
        services: &Map<String, Value>,
        log: &mut dyn FnMut(&str),
    ) -> anyhow::Result<()> {
        for (name, svc) in services {
            let checked = svc
                .get("options")
                .and_then(Value::as_str)
                .is_some_and(|o| o.contains("--health-cmd"));
            if !checked {
                continue;
            }
            let container = self.container_name(&format!("s-{}", label(name)));
            let deadline = Instant::now() + Duration::from_secs(300);
            log(&format!("Waiting for {name} to be healthy"));
            loop {
                let state = self.status(&container).unwrap_or_default();
                match state.split_whitespace().last() {
                    Some("healthy") => break,
                    _ if state.starts_with("exited") => {
                        anyhow::bail!("the service {name} stopped ({state})")
                    }
                    _ if Instant::now() >= deadline => {
                        anyhow::bail!("the service {name} never became healthy ({state})")
                    }
                    _ => std::thread::sleep(Duration::from_secs(2)),
                }
            }
            log(&format!("{name} is healthy"));
        }
        Ok(())
    }

    /// Hand a step to the agent and follow its log until it ends.
    fn agent_step(
        &mut self,
        e: &Exec<'_>,
        stop: &dyn Fn() -> bool,
        out: &mut dyn FnMut(&str),
    ) -> anyhow::Result<Exit> {
        self.steps += 1;
        let dir = self.agent.join("steps").join(self.steps.to_string());
        std::fs::create_dir_all(&dir)?;
        set_mode(&dir, 0o777)?;
        std::fs::write(dir.join("run.sh"), step_script(e))?;
        std::fs::write(dir.join("ready"), b"")?;

        let deadline = Instant::now() + e.timeout;
        let job = self.container_name("job");
        let mut tail = Tail::new(dir.join("log"));
        let mut ended: Option<(Exit, Instant)> = None;
        let mut checked = Instant::now();
        loop {
            tail.read(out);
            if let Ok(code) = std::fs::read_to_string(dir.join("code")) {
                tail.read(out);
                tail.flush(out);
                return Ok(match ended {
                    Some((why, _)) => why,
                    None => Exit::Code(code.trim().parse().unwrap_or(-1)),
                });
            }
            match ended {
                None => {
                    let why = if stop() {
                        Some(Exit::Stopped)
                    } else if Instant::now() >= deadline {
                        Some(Exit::TimedOut)
                    } else {
                        None
                    };
                    if let Some(why) = why {
                        std::fs::write(dir.join("cancel"), b"")?;
                        ended = Some((why, Instant::now()));
                    }
                }
                Some((why, since)) if since.elapsed() > Duration::from_secs(20) => {
                    tail.flush(out);
                    return Ok(why);
                }
                Some(_) => {}
            }
            if checked.elapsed() >= Duration::from_secs(5) {
                checked = Instant::now();
                let state = self.status(&job).unwrap_or_default();
                if !state.is_empty() && !state.starts_with("running") {
                    tail.flush(out);
                    anyhow::bail!("the job's container stopped ({state})");
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}

/// A log file read as it grows.
struct Tail {
    path: PathBuf,
    at: u64,
    partial: String,
}

impl Tail {
    fn new(path: PathBuf) -> Tail {
        Tail {
            path,
            at: 0,
            partial: String::new(),
        }
    }

    fn read(&mut self, out: &mut dyn FnMut(&str)) {
        let Ok(mut f) = std::fs::File::open(&self.path) else {
            return;
        };
        if f.seek(SeekFrom::Start(self.at)).is_err() {
            return;
        }
        let mut buf = Vec::new();
        if f.read_to_end(&mut buf).is_err() {
            return;
        }
        self.at += buf.len() as u64;
        self.partial.push_str(&String::from_utf8_lossy(&buf));
        while let Some(i) = self.partial.find('\n') {
            let line: String = self.partial.drain(..=i).collect();
            out(line.trim_end_matches(['\n', '\r']));
        }
    }

    fn flush(&mut self, out: &mut dyn FnMut(&str)) {
        if !self.partial.is_empty() {
            out(&std::mem::take(&mut self.partial));
        }
    }

    /// The text of a k8s-file log since the last call, as lines.
    fn read_k8s(&mut self, partial: &mut String, out: &mut dyn FnMut(&str)) {
        let mut raw = Vec::new();
        self.read(&mut |l| raw.push(l.to_string()));
        k8s_lines(&raw.join("\n"), partial, out);
    }
}

fn set_mode(p: &Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode))
}

impl Sandbox for Pm {
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
        self.agent_step(e, stop, out)
    }

    fn run_image(
        &mut self,
        image: &str,
        entrypoint: Option<&str>,
        e: &Exec<'_>,
        stop: &dyn Fn() -> bool,
        out: &mut dyn FnMut(&str),
    ) -> anyhow::Result<Exit> {
        self.actions += 1;
        let n = self.actions;
        let container = self.container_name(&format!("a{n}"));
        let log_file = self.agent.join(format!("a{n}.log"));
        let mut svc = Map::new();
        svc.insert("image".into(), json!(image));
        if let Some((tag, context, dockerfile)) = self.pending.take() {
            if tag == image {
                svc.insert(
                    "build".into(),
                    json!({ "context": context, "dockerfile": dockerfile }),
                );
            }
        }
        svc.insert("container_name".into(), json!(container));
        svc.insert(
            "network_mode".into(),
            json!(format!("container:{}", self.container_name("job"))),
        );
        if let Some(ep) = entrypoint.filter(|s| !s.is_empty()) {
            svc.insert("entrypoint".into(), json!([ep]));
        }
        if !e.argv.is_empty() {
            svc.insert("command".into(), json!(e.argv));
        }
        svc.insert("working_dir".into(), json!(e.cwd));
        let env: Map<String, Value> = e
            .env
            .iter()
            .filter(|(k, _)| k.as_str() != "PATH")
            .map(|(k, v)| (k.clone(), json!(v)))
            .collect();
        svc.insert("environment".into(), Value::Object(env));
        svc.insert("volumes".into(), Value::Array(self.mounts()));
        svc.insert("mem_limit".into(), json!(self.limits.memory));
        svc.insert("cpus".into(), json!(self.limits.cpus));
        svc.insert("cpu_shares".into(), json!(256));
        svc.insert("pids_limit".into(), json!(8192));
        svc.insert(
            "logging".into(),
            json!({ "driver": "k8s-file", "options": { "path": log_file } }),
        );
        let compose = json!({
            "name": container,
            "x-podman": { "in_pod": false },
            "services": { "action": svc },
        });
        let file = self.files.join(format!("{container}.action.yml"));
        std::fs::write(&file, serde_json::to_vec_pretty(&compose)?)?;

        let mut up = self.tool.container(&["up"]);
        up.arg(&file);
        let mut quiet = |l: &str| {
            if !noise(l) {
                out(l)
            }
        };
        let started = stream(up, Duration::from_secs(1800), stop, &mut quiet)?;
        let result = (|| -> anyhow::Result<Exit> {
            match started {
                Exit::Code(0) => {}
                Exit::Code(_) => anyhow::bail!("the action's container did not start"),
                other => return Ok(other),
            }
            let deadline = Instant::now() + e.timeout;
            let mut tail = Tail::new(log_file.clone());
            let mut partial = String::new();
            let mut ended: Option<Exit> = None;
            loop {
                tail.read_k8s(&mut partial, out);
                let state = self.status(&container).unwrap_or_default();
                if let Some(code) = state.strip_prefix("exited ") {
                    tail.read_k8s(&mut partial, out);
                    if !partial.is_empty() {
                        out(&partial);
                    }
                    let code = code.split_whitespace().next().unwrap_or("-1");
                    return Ok(ended.unwrap_or(Exit::Code(code.parse().unwrap_or(-1))));
                }
                if ended.is_none() {
                    let why = if stop() {
                        Some(Exit::Stopped)
                    } else if Instant::now() >= deadline {
                        Some(Exit::TimedOut)
                    } else {
                        None
                    };
                    if let Some(why) = why {
                        ended = Some(why);
                        let _ = self.tool.quiet(&["stop", &container]);
                    }
                } else if state.is_empty() {
                    return Ok(ended.unwrap_or(Exit::Stopped));
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        })();
        let mut down = self.tool.container(&["down"]);
        down.arg(&file);
        let _ = down.stdout(Stdio::null()).stderr(Stdio::null()).status();
        result
    }

    fn build_image(
        &mut self,
        context: &Path,
        dockerfile: &str,
        out: &mut dyn FnMut(&str),
    ) -> anyhow::Result<String> {
        // Built by the compose file that runs it, so the image never
        // outlives the step.
        let tag = format!(
            "localhost/{}",
            self.container_name(&format!("a{}", self.actions + 1))
        );
        out(&format!("Building {tag} from {dockerfile}"));
        self.pending = Some((tag.clone(), context.to_path_buf(), dockerfile.to_string()));
        Ok(tag)
    }

    fn finish(&mut self) {
        let _ = std::fs::write(self.agent.join("done"), b"");
        let mut down = self.tool.container(&["down"]);
        down.arg(self.files.join(format!("{}.yml", self.name)));
        let _ = down.stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
}

/// `uses: losos-project/pm@<ref>`: build or explain a pm recipe from the
/// workspace, on the host as the shared user, in pm's own jail. The recipe
/// must be signed by one of the keys the step trusts.
pub struct Recipe<'a> {
    pub tool: &'a Tool,
    pub dirs: &'a Dirs,
    /// `build` or `explain`.
    pub command: String,
    /// The recipe, relative to the workspace.
    pub recipe: String,
    pub permissive: bool,
    /// Public keys, as hex.
    pub trust: Vec<String>,
}

impl Recipe<'_> {
    /// Run it; the exit and the archives it made, relative to the workspace.
    pub fn run(
        &self,
        timeout: Duration,
        stop: &dyn Fn() -> bool,
        out: &mut dyn FnMut(&str),
    ) -> anyhow::Result<(Exit, Vec<String>)> {
        if !matches!(self.command.as_str(), "build" | "explain") {
            anyhow::bail!("command is build or explain, not {}", self.command);
        }
        let rel = Path::new(&self.recipe);
        if self.recipe.is_empty()
            || rel.is_absolute()
            || rel
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            anyhow::bail!("recipe is a path inside the workspace");
        }
        let config = self.dirs.root.join("pm-config");
        let trusted = config.join("pm").join("trusted");
        std::fs::create_dir_all(&trusted)?;
        for key in &self.trust {
            let key = key.trim().to_ascii_lowercase();
            if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
                anyhow::bail!("{key} is not a public key: 64 hex digits");
            }
            std::fs::write(trusted.join(format!("{key}.pub")), format!("{key}\n"))?;
        }
        let before = archives(&self.dirs.workspace);
        let mut cmd = self.tool.command(&config);
        cmd.arg("--no-plugins").arg(&self.command).arg(rel);
        if self.permissive {
            cmd.arg("--permissive");
        }
        cmd.current_dir(&self.dirs.workspace)
            .env("RUST_LOG", "info")
            .env("HOME", &self.dirs.home)
            .env("TMPDIR", &self.dirs.temp)
            .env("NO_COLOR", "1");
        let exit = stream(cmd, timeout, stop, out)?;
        let made = archives(&self.dirs.workspace)
            .into_iter()
            .filter(|a| !before.contains(a))
            .collect();
        Ok((exit, made))
    }
}

/// Every `.cpkg` under `dir`, relative to it.
fn archives(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            let Ok(t) = e.file_type() else { continue };
            if t.is_dir() {
                todo.push(p);
            } else if p.extension().is_some_and(|x| x == "cpkg") {
                if let Ok(rel) = p.strip_prefix(dir) {
                    found.push(rel.to_string_lossy().into_owned());
                }
            }
        }
    }
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn pm(root: &Path) -> Pm {
        let dirs = Dirs::new(root).unwrap();
        Pm {
            tool: Tool {
                bin: "pm".into(),
                config: "/nix/store/x".into(),
            },
            name: job_name("job-1"),
            agent: root.join("agent"),
            files: root.join("lci"),
            dirs,
            inside: Inside {
                workspace: "/github/workspace".into(),
                temp: "/github/runner_temp".into(),
                actions: "/github/actions".into(),
                home: "/github/home".into(),
            },
            path: String::new(),
            node: None,
            limits: Limits {
                memory: "2g".into(),
                cpus: "1.5".into(),
            },
            steps: 0,
            actions: 0,
            pending: None,
        }
    }

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("losos-ci-pm-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn names_are_short_stable_and_ours() {
        let a = job_name("job-0123-abc");
        assert_eq!(a, job_name("job-0123-abc"));
        assert_ne!(a, job_name("job-0123-abd"));
        assert!(a.starts_with("lci-") && a.len() == 14);
        assert_eq!(label("Postgres DB!"), "postgres-db-");
    }

    #[test]
    fn options_become_compose_keys() {
        let mut said = Vec::new();
        let o = compose_options(
            r#"--privileged --health-cmd "redis-cli ping" --health-interval 5s -v /:/host --user 1000 -e A=b"#,
            &mut |l| said.push(l.to_string()),
        );
        assert_eq!(o["user"], "1000");
        assert_eq!(
            o["healthcheck"]["test"],
            json!(["CMD-SHELL", "redis-cli ping"])
        );
        assert_eq!(o["healthcheck"]["interval"], "5s");
        assert_eq!(o["environment"]["A"], "b");
        assert_eq!(said.len(), 2, "{said:?}");
    }

    #[test]
    fn the_job_and_its_services_share_a_network() {
        let root = temp("compose");
        let p = pm(&root);
        let services = json!({
            "postgres": {
                "image": "postgres:16",
                "env": { "POSTGRES_PASSWORD": "x" },
                "options": "--health-cmd pg_isready --health-retries 5"
            },
            "nothing": { "image": "" }
        });
        let c = p.job_compose(
            "localhost/losos-ci-base:abc",
            &json!({ "env": { "CI": true } }),
            &services,
            &mut |_| {},
        );
        let name = job_name("job-1");
        assert_eq!(c["name"], json!(name));
        assert_eq!(c["x-podman"]["in_pod"], false);
        let job = &c["services"]["job"];
        assert_eq!(job["container_name"], json!(format!("{name}-job")));
        assert_eq!(
            job["entrypoint"],
            json!(["/bin/sh", "/github/losos-ci/agent.sh"])
        );
        assert_eq!(job["environment"]["CI"], "true");
        assert_eq!(job["environment"]["HOME"], "/github/home");
        assert_eq!(job["extra_hosts"], json!(["postgres:127.0.0.1"]));
        assert_eq!(job["mem_limit"], "2g");
        assert!(job["volumes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str().unwrap().ends_with(":/github/losos-ci")));
        let db = &c["services"]["s-postgres"];
        assert_eq!(db["network_mode"], "service:job");
        assert_eq!(db["container_name"], json!(format!("{name}-s-postgres")));
        assert_eq!(db["healthcheck"]["retries"], 5);
        assert_eq!(db["environment"]["POSTGRES_PASSWORD"], "x");
        assert!(c["services"].get("s-nothing").is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_step_is_a_script_with_its_env_quoted() {
        let env: BTreeMap<String, String> = [
            ("A".to_string(), "it's".to_string()),
            ("bad name".to_string(), "x".to_string()),
        ]
        .into_iter()
        .collect();
        let argv = vec![
            "bash".to_string(),
            "-e".to_string(),
            "/x/step 1.sh".to_string(),
        ];
        let s = step_script(&Exec {
            argv: &argv,
            env: &env,
            cwd: "/github/workspace",
            timeout: Duration::from_secs(1),
        });
        assert_eq!(
            s,
            "cd -- '/github/workspace' || exit 1\nexport A='it'\\''s'\nexec 'bash' '-e' '/x/step 1.sh'\n"
        );
    }

    #[test]
    fn k8s_file_lines_join_their_parts() {
        let mut got = Vec::new();
        let mut partial = String::new();
        k8s_lines(
            "2026-10-21T10:00:00Z stdout P hel\n2026-10-21T10:00:00Z stdout F lo\n2026-10-21T10:00:01Z stderr F two words",
            &mut partial,
            &mut |l| got.push(l.to_string()),
        );
        assert_eq!(got, ["hello", "two words"]);
        assert!(noise(&"a".repeat(64)));
        assert!(!noise("Pulling postgres:16"));
    }

    /// The agent, run by a real sh against a real directory: steps in order,
    /// output and codes beside them, a cancelled step ended.
    #[test]
    fn the_agent_runs_steps_and_cancels_them() {
        let root = temp("agent");
        let d = root.join("losos-ci");
        std::fs::create_dir_all(d.join("steps/1")).unwrap();
        std::fs::create_dir_all(d.join("steps/2")).unwrap();
        let script = AGENT.replace("d=/github/losos-ci", &format!("d={}", d.display()));
        // `kill -1` would reach every process of the test's user; the agent
        // is pid 1 of its container only in a container.
        let script = script
            .replace("kill -TERM -1", "kill -TERM \"$pid\"")
            .replace("kill -KILL -1", "kill -KILL \"$pid\"");
        std::fs::write(d.join("agent.sh"), script).unwrap();
        let mut agent = Command::new("sh").arg(d.join("agent.sh")).spawn().unwrap();
        std::fs::write(d.join("steps/1/run.sh"), "echo one; echo two >&2; exit 3\n").unwrap();
        std::fs::write(d.join("steps/1/ready"), "").unwrap();
        let wait = |p: PathBuf| {
            let t = Instant::now();
            while !p.exists() {
                assert!(t.elapsed() < Duration::from_secs(20), "{}", p.display());
                std::thread::sleep(Duration::from_millis(50));
            }
        };
        wait(d.join("steps/1/code"));
        assert_eq!(
            std::fs::read_to_string(d.join("steps/1/code")).unwrap(),
            "3"
        );
        assert_eq!(
            std::fs::read_to_string(d.join("steps/1/log")).unwrap(),
            "one\ntwo\n"
        );
        assert!(d.join("path").exists());
        std::fs::write(d.join("steps/2/run.sh"), "exec sleep 30\n").unwrap();
        std::fs::write(d.join("steps/2/ready"), "").unwrap();
        std::thread::sleep(Duration::from_millis(500));
        std::fs::write(d.join("steps/2/cancel"), "").unwrap();
        wait(d.join("steps/2/code"));
        assert_ne!(
            std::fs::read_to_string(d.join("steps/2/code")).unwrap(),
            "0"
        );
        std::fs::write(d.join("done"), "").unwrap();
        let t = Instant::now();
        while agent.try_wait().unwrap().is_none() {
            assert!(t.elapsed() < Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn recipes_stay_in_the_workspace_and_keys_are_hex() {
        let root = temp("recipe");
        let dirs = Dirs::new(&root).unwrap();
        let tool = Tool {
            bin: "/nonexistent/pm".into(),
            config: root.clone(),
        };
        let r = |recipe: &str, trust: &[&str], command: &str| Recipe {
            tool: &tool,
            dirs: &dirs,
            command: command.into(),
            recipe: recipe.into(),
            permissive: false,
            trust: trust.iter().map(|s| s.to_string()).collect(),
        };
        let go = |x: Recipe<'_>| x.run(Duration::from_secs(1), &|| false, &mut |_| {});
        assert!(go(r("../etc/x.rhai", &[], "build")).is_err());
        assert!(go(r("/etc/x.rhai", &[], "build")).is_err());
        assert!(go(r("x.rhai", &["zz"], "build")).is_err());
        assert!(go(r("x.rhai", &[], "install")).is_err());
        // A good request gets as far as starting pm.
        let key = "ab".repeat(32);
        let e = go(r("pkg/build.rhai", &[&key], "build")).unwrap_err();
        assert!(format!("{e:#}").contains("starting the step"), "{e:#}");
        assert!(root
            .join("pm-config/pm/trusted")
            .join(format!("{key}.pub"))
            .exists());
        std::fs::write(dirs.workspace.join("a.cpkg"), "").unwrap();
        assert_eq!(archives(&dirs.workspace), ["a.cpkg"]);
        let _ = std::fs::remove_dir_all(root);
    }
}
