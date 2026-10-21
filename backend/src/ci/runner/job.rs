//! One job's steps, the way GitHub's runner does them: `run:` scripts in
//! the job's shell, `uses:` actions (JavaScript, composite and container
//! ones, from the repository, from GitHub or `docker://`), `if:` with the
//! status functions, `env`, `with`, step outputs, `continue-on-error`,
//! `timeout-minutes`, the file commands and the workflow commands, post
//! steps, and the job's `outputs` at the end.
//!
//! Three actions need GitHub's own services and are answered here instead:
//! `actions/checkout` (the commit's files are in the workspace already),
//! `actions/cache` and `actions/upload-artifact` / `download-artifact`
//! (there is no cache or artifact store on the mesh, so they do nothing
//! and say so).

use super::commands::{self, Command};
use super::sandbox::{Dirs, Exec, Exit, Sandbox};
use crate::ci::expr::{self, Scope};
use crate::ci::workflow::yaml_to_json;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Fetch `owner/repo` at `ref` from GitHub into a directory.
pub type Fetch<'a> = &'a dyn Fn(&str, &str, &Path) -> anyhow::Result<()>;

pub struct Job<'a> {
    pub payload: &'a Value,
    pub dirs: &'a Dirs,
    /// This box's name, `runner.name`.
    pub runner_name: &'a str,
    pub sandbox: &'a mut dyn Sandbox,
    pub fetch: Fetch<'a>,
    pub stop: &'a dyn Fn() -> bool,
    pub log: &'a mut dyn FnMut(&str),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// success, failure or cancelled.
    pub state: &'static str,
    pub outputs: Map<String, Value>,
}

/// A step's result, as `steps.<id>` holds it.
#[derive(Debug, Clone)]
struct StepResult {
    outcome: &'static str,
    outputs: Map<String, Value>,
}

/// What runs after the main steps: a JavaScript action's `post`.
struct Post {
    name: String,
    argv: Vec<String>,
    env: BTreeMap<String, String>,
    condition: String,
    state: Map<String, Value>,
}

struct Engine<'j, 'a> {
    job: &'j mut Job<'a>,
    /// `$GITHUB_ENV` so far.
    env: BTreeMap<String, String>,
    /// `$GITHUB_PATH` so far, newest first.
    path: Vec<String>,
    masks: Vec<String>,
    failed: bool,
    posts: Vec<Post>,
    counter: u32,
    parser: commands::Parser,
}

fn text(v: Option<&Value>) -> String {
    v.map(expr::to_text).unwrap_or_default()
}

/// GitHub's `hashFiles`: SHA-256 over the SHA-256 of each matching file,
/// in path order; empty when nothing matches.
fn hash_files(workspace: &Path, patterns: &[String]) -> String {
    let mut files: Vec<(String, PathBuf)> = walkdir::WalkDir::new(workspace)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let rel = e
                .path()
                .strip_prefix(workspace)
                .ok()?
                .to_string_lossy()
                .into_owned();
            Some((rel, e.path().to_path_buf()))
        })
        .filter(|(rel, _)| {
            let (inc, exc): (Vec<_>, Vec<_>) = patterns.iter().partition(|p| !p.starts_with('!'));
            inc.iter()
                .any(|p| crate::ci::glob::matches(p.trim_start_matches("./"), rel))
                && !exc
                    .iter()
                    .any(|p| crate::ci::glob::matches(p[1..].trim_start_matches("./"), rel))
        })
        .collect();
    if files.is_empty() {
        return String::new();
    }
    files.sort();
    let mut all = ring::digest::Context::new(&ring::digest::SHA256);
    for (_, path) in files {
        if let Ok(bytes) = std::fs::read(path) {
            all.update(ring::digest::digest(&ring::digest::SHA256, &bytes).as_ref());
        }
    }
    all.finish()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `name<<DELIMITER` pairs and plain ones, from a file command's file.
fn read_pairs(path: &Path, log: &mut dyn FnMut(&str)) -> Vec<(String, String)> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    match commands::parse_pairs(&text) {
        Ok(pairs) => pairs,
        Err(e) => {
            log(&format!("##[error]{}: {e}", path.display()));
            Vec::new()
        }
    }
}

/// An action's reference: `owner/repo[/path]@ref`.
fn split_uses(uses: &str) -> Option<(String, String, String)> {
    let (what, r) = uses.split_once('@')?;
    let mut parts = what.splitn(3, '/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    let sub = parts.next().unwrap_or("");
    let ok = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
            && s != ".."
    };
    let ref_ok = !r.is_empty()
        && r.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./".contains(c))
        && !r.contains("..");
    if !ok(owner) || !ok(repo) || !ref_ok || sub.split('/').any(|s| s == "..") {
        return None;
    }
    Some((format!("{owner}/{repo}"), r.to_string(), sub.to_string()))
}

impl<'j, 'a> Engine<'j, 'a> {
    fn log(&mut self, line: &str) {
        let masked = commands::mask(line, &self.masks);
        (self.job.log)(&masked);
    }

    fn file(&self, name: &str) -> (PathBuf, String) {
        (
            self.job.dirs.temp.join(name),
            format!("{}/{name}", self.job.sandbox.inside().temp),
        )
    }

    /// Every context but `env` and `steps`, which change per step.
    fn base_contexts(&self) -> Map<String, Value> {
        let mut ctx = self
            .job
            .payload
            .get("contexts")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let inside = self.job.sandbox.inside();
        ctx.insert(
            "runner".into(),
            json!({
                "name": self.job.runner_name,
                "os": "Linux",
                "arch": "X64",
                "temp": inside.temp,
                "tool_cache": format!("{}/toolcache", inside.home),
                "debug": "",
                "environment": "self-hosted",
            }),
        );
        ctx.insert(
            "job".into(),
            json!({
                "status": if self.failed { "failure" } else { "success" },
                "container": { "id": "", "network": "" },
                "services": {},
            }),
        );
        ctx
    }

    fn github(&self) -> Value {
        self.job
            .payload
            .pointer("/contexts/github")
            .cloned()
            .unwrap_or(Value::Null)
    }

    /// The variables every step gets, GitHub's `GITHUB_*` and `RUNNER_*`.
    fn default_env(&self) -> BTreeMap<String, String> {
        let g = self.github();
        let s = |k: &str| text(g.get(k));
        let inside = self.job.sandbox.inside();
        let mut e = BTreeMap::new();
        let mut put = |k: &str, v: String| {
            e.insert(k.to_string(), v);
        };
        put("CI", "true".into());
        put("GITHUB_ACTIONS", "true".into());
        put("LOSOS_CI", "true".into());
        put("HOME", inside.home.clone());
        put("GITHUB_WORKSPACE", inside.workspace.clone());
        put("GITHUB_EVENT_PATH", format!("{}/_event.json", inside.temp));
        for (k, key) in [
            ("GITHUB_EVENT_NAME", "event_name"),
            ("GITHUB_SHA", "sha"),
            ("GITHUB_REF", "ref"),
            ("GITHUB_REF_NAME", "ref_name"),
            ("GITHUB_REF_TYPE", "ref_type"),
            ("GITHUB_HEAD_REF", "head_ref"),
            ("GITHUB_BASE_REF", "base_ref"),
            ("GITHUB_REPOSITORY", "repository"),
            ("GITHUB_REPOSITORY_OWNER", "repository_owner"),
            ("GITHUB_RUN_ID", "run_id"),
            ("GITHUB_RUN_NUMBER", "run_number"),
            ("GITHUB_RUN_ATTEMPT", "run_attempt"),
            ("GITHUB_ACTOR", "actor"),
            ("GITHUB_TRIGGERING_ACTOR", "triggering_actor"),
            ("GITHUB_WORKFLOW", "workflow"),
            ("GITHUB_WORKFLOW_REF", "workflow_ref"),
            ("GITHUB_JOB", "job"),
            ("GITHUB_SERVER_URL", "server_url"),
            ("GITHUB_API_URL", "api_url"),
            ("GITHUB_GRAPHQL_URL", "graphql_url"),
        ] {
            put(k, s(key));
        }
        put("RUNNER_OS", "Linux".into());
        put("RUNNER_ARCH", "X64".into());
        put("RUNNER_NAME", self.job.runner_name.to_string());
        put("RUNNER_TEMP", inside.temp.clone());
        put("RUNNER_TOOL_CACHE", format!("{}/toolcache", inside.home));
        put("RUNNER_WORKSPACE", inside.workspace.clone());
        e
    }

    /// `env` for a step: the job's, `$GITHUB_ENV`'s, the step's own.
    fn step_env(
        &self,
        step_env: Option<&Value>,
        outer: &BTreeMap<String, String>,
        ctx: &Map<String, Value>,
        failed: bool,
    ) -> Result<BTreeMap<String, String>, String> {
        let mut env = outer.clone();
        for (k, v) in &self.env {
            env.insert(k.clone(), v.clone());
        }
        if let Some(Value::Object(m)) = step_env {
            let mut c = ctx.clone();
            c.insert("env".into(), json!(env));
            let workspace = self.job.dirs.workspace.clone();
            let hasher = move |p: &[String]| hash_files(&workspace, p);
            let scope = Scope {
                failed,
                cancelled: (self.job.stop)(),
                hash_files: Some(&hasher),
                ..Scope::new(&c)
            };
            for (k, v) in m {
                let v = expr::interpolate_value(v, &scope).map_err(|e| format!("env.{k}: {e}"))?;
                env.insert(k.clone(), expr::to_text(&v));
            }
        }
        Ok(env)
    }

    fn path_value(&self) -> String {
        let mut parts = self.path.clone();
        parts.push(self.job.sandbox.base_path());
        parts.join(":")
    }

    /// Run a program for a step, read its workflow and file commands.
    #[allow(clippy::too_many_arguments)]
    fn exec(
        &mut self,
        argv: Vec<String>,
        env: &BTreeMap<String, String>,
        cwd: &str,
        timeout: Duration,
        image: Option<(&str, Option<&str>)>,
        outputs: &mut Map<String, Value>,
        state: &mut Map<String, Value>,
    ) -> Result<Exit, String> {
        self.counter += 1;
        let n = self.counter;
        let (env_host, env_in) = self.file(&format!("_env_{n}"));
        let (out_host, out_in) = self.file(&format!("_output_{n}"));
        let (path_host, path_in) = self.file(&format!("_path_{n}"));
        let (state_host, state_in) = self.file(&format!("_state_{n}"));
        let (sum_host, sum_in) = self.file(&format!("_summary_{n}"));
        for p in [&env_host, &out_host, &path_host, &state_host, &sum_host] {
            std::fs::write(p, b"").map_err(|e| e.to_string())?;
        }
        let mut env = env.clone();
        env.insert("GITHUB_ENV".into(), env_in);
        env.insert("GITHUB_OUTPUT".into(), out_in);
        env.insert("GITHUB_PATH".into(), path_in);
        env.insert("GITHUB_STATE".into(), state_in);
        env.insert("GITHUB_STEP_SUMMARY".into(), sum_in);
        env.entry("PATH".into())
            .or_insert_with(|| self.path_value());

        let mut lines: Vec<Command> = Vec::new();
        let exit = {
            let parser = &mut self.parser;
            let masks = &mut self.masks;
            let log = &mut self.job.log;
            let mut handle = |line: &str| {
                let c = parser.line(line);
                match &c {
                    Command::Line(l) => (log)(&commands::mask(l, masks)),
                    Command::Mask(m) => {
                        if !m.is_empty() {
                            masks.push(m.clone());
                        }
                    }
                    Command::Group(g) => (log)(&format!("##[group]{}", commands::mask(g, masks))),
                    Command::EndGroup => (log)("##[endgroup]"),
                    Command::Annotation(kind, m) => {
                        (log)(&format!("##[{kind}]{}", commands::mask(m, masks)))
                    }
                    Command::Debug(_) => {}
                    Command::Ignored(why) if !why.is_empty() => (log)(&format!("##[warning]{why}")),
                    _ => {}
                }
                lines.push(c);
            };
            let e = Exec {
                argv: &argv,
                env: &env,
                cwd,
                timeout,
            };
            let stop = self.job.stop;
            match image {
                Some((img, entry)) => self
                    .job
                    .sandbox
                    .run_image(img, entry, &e, stop, &mut handle),
                None => self.job.sandbox.exec(&e, stop, &mut handle),
            }
            .map_err(|e| format!("{e:#}"))?
        };
        for c in lines {
            match c {
                Command::Output(k, v) => {
                    outputs.insert(k, json!(v));
                }
                Command::State(k, v) => {
                    state.insert(k, json!(v));
                }
                _ => {}
            }
        }
        let mut log = |l: &str| (self.job.log)(l);
        for (k, v) in read_pairs(&env_host, &mut log) {
            if k == "NODE_OPTIONS" && v.contains("--require") {
                continue;
            }
            self.env.insert(k, v);
        }
        for (k, v) in read_pairs(&out_host, &mut log) {
            outputs.insert(k, json!(v));
        }
        for (k, v) in read_pairs(&state_host, &mut log) {
            state.insert(k, json!(v));
        }
        let added = std::fs::read_to_string(&path_host).unwrap_or_default();
        for p in added.lines().map(str::trim).filter(|p| !p.is_empty()) {
            self.path.insert(0, p.to_string());
        }
        let summary = std::fs::read_to_string(&sum_host).unwrap_or_default();
        if !summary.trim().is_empty() {
            self.log("##[group]Step summary");
            for l in summary.lines() {
                self.log(l);
            }
            self.log("##[endgroup]");
        }
        Ok(exit)
    }

    /// The steps of the job or of a composite action. `steps` is the
    /// `steps` context they fill.
    #[allow(clippy::too_many_arguments)]
    fn run_steps(
        &mut self,
        list: &[Value],
        ctx: &Map<String, Value>,
        outer_env: &BTreeMap<String, String>,
        defaults: &Value,
        steps: &mut Map<String, Value>,
        composite: bool,
    ) -> bool {
        let mut failed = false;
        for (i, step) in list.iter().enumerate() {
            let id = step
                .get("id")
                .map(expr::to_text)
                .unwrap_or_else(|| format!("__{i}"));
            let mut c = ctx.clone();
            c.insert("steps".into(), Value::Object(steps.clone()));
            if !composite {
                c.insert("job".into(), self.base_contexts()["job"].clone());
            }
            let any_failed = failed || (!composite && self.failed);
            let env = match self.step_env(step.get("env"), outer_env, &c, any_failed) {
                Ok(e) => e,
                Err(why) => {
                    self.log(&format!("##[error]{why}"));
                    failed = true;
                    steps.insert(
                        id,
                        json!({ "outcome": "failure", "conclusion": "failure", "outputs": {} }),
                    );
                    continue;
                }
            };
            c.insert("env".into(), json!(env));
            let workspace = self.job.dirs.workspace.clone();
            let hasher = move |p: &[String]| hash_files(&workspace, p);
            let scope = Scope {
                failed: any_failed,
                cancelled: (self.job.stop)(),
                hash_files: Some(&hasher),
                ..Scope::new(&c)
            };
            let name = match step.get("name") {
                Some(n) => expr::interpolate(&expr::to_text(n), &scope)
                    .unwrap_or_else(|_| expr::to_text(n)),
                None => match (step.get("run"), step.get("uses")) {
                    (Some(r), _) => format!(
                        "Run {}",
                        expr::to_text(r).lines().next().unwrap_or("").trim()
                    ),
                    (None, Some(u)) => format!("Run {}", expr::to_text(u)),
                    _ => format!("Step {}", i + 1),
                },
            };
            let cond = step.get("if").map(expr::to_text);
            let run_it = match expr::condition(cond.as_deref(), &scope) {
                Ok(b) => b,
                Err(e) => {
                    self.log(&format!("##[error]{name}: if: {e}"));
                    false
                }
            };
            if !run_it {
                self.log(&format!("##[group]{name}"));
                self.log("##[skipped]");
                self.log("##[endgroup]");
                steps.insert(
                    id,
                    json!({ "outcome": "skipped", "conclusion": "skipped", "outputs": {} }),
                );
                continue;
            }
            self.log(&format!("##[group]{name}"));
            let started = std::time::Instant::now();
            let result = self.run_step(step, &c, &env, defaults, &scope_owned(&scope));
            let continue_on_error = step
                .get("continue-on-error")
                .map(|v| {
                    expr::interpolate_value(v, &scope)
                        .map(|v| expr::truthy(&v))
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            let conclusion = if result.outcome == "failure" && continue_on_error {
                "success"
            } else {
                result.outcome
            };
            self.log(&format!(
                "##[result]{} {}s",
                result.outcome,
                started.elapsed().as_secs()
            ));
            self.log("##[endgroup]");
            if conclusion == "failure" {
                failed = true;
                if !composite {
                    self.failed = true;
                }
            }
            steps.insert(
                id,
                json!({ "outcome": result.outcome, "conclusion": conclusion, "outputs": result.outputs }),
            );
        }
        failed
    }

    fn run_step(
        &mut self,
        step: &Value,
        ctx: &Map<String, Value>,
        env: &BTreeMap<String, String>,
        defaults: &Value,
        scope: &ScopeParts,
    ) -> StepResult {
        let fail = |outcome| StepResult {
            outcome,
            outputs: Map::new(),
        };
        let workspace = self.job.dirs.workspace.clone();
        let hasher = move |p: &[String]| hash_files(&workspace, p);
        let scope = Scope {
            failed: scope.failed,
            cancelled: scope.cancelled,
            hash_files: Some(&hasher),
            ..Scope::new(ctx)
        };
        let timeout = step
            .get("timeout-minutes")
            .and_then(|v| expr::interpolate_value(v, &scope).ok())
            .and_then(|v| expr::to_text(&v).trim().parse::<f64>().ok())
            .map(|m| Duration::from_secs_f64((m * 60.0).clamp(1.0, 6.0 * 3600.0)))
            .unwrap_or(Duration::from_secs(6 * 3600));
        let inside_ws = self.job.sandbox.inside().workspace.clone();
        let wd = step
            .get("working-directory")
            .or_else(|| defaults.pointer("/run/working-directory"))
            .map(|v| expr::interpolate(&expr::to_text(v), &scope).unwrap_or_default());
        let cwd = match wd {
            Some(d) if d.starts_with('/') => d,
            Some(d) if !d.is_empty() => format!("{inside_ws}/{d}"),
            _ => inside_ws.clone(),
        };

        if let Some(run) = step.get("run") {
            let script = match expr::interpolate(&expr::to_text(run), &scope) {
                Ok(s) => s,
                Err(e) => {
                    self.log(&format!("##[error]run: {e}"));
                    return fail("failure");
                }
            };
            let shell = step
                .get("shell")
                .or_else(|| defaults.pointer("/run/shell"))
                .map(|v| expr::interpolate(&expr::to_text(v), &scope).unwrap_or_default())
                .unwrap_or_default();
            let (template, ext) = match shell.trim() {
                "" | "bash" => ("bash --noprofile --norc -eo pipefail {0}", "sh"),
                "sh" => ("sh -e {0}", "sh"),
                "python" => ("python {0}", "py"),
                "pwsh" | "powershell" | "cmd" => {
                    self.log(&format!(
                        "##[error]The {shell} shell is not on LosOS runners."
                    ));
                    return fail("failure");
                }
                custom if custom.contains("{0}") => (custom, "sh"),
                other => {
                    self.log(&format!(
                        "##[error]{other}: a custom shell names its script with {{0}}"
                    ));
                    return fail("failure");
                }
            };
            self.counter += 1;
            let file = format!("_step_{}.{ext}", self.counter);
            let (host, inside) = self.file(&file);
            if let Err(e) = std::fs::write(&host, script.as_bytes()) {
                self.log(&format!("##[error]{e}"));
                return fail("failure");
            }
            for l in script.lines() {
                self.log(&format!("##[command]{l}"));
            }
            let argv: Vec<String> = super::sandbox::words(template)
                .into_iter()
                .map(|w| w.replace("{0}", &inside))
                .collect();
            let mut outputs = Map::new();
            let mut state = Map::new();
            return match self.exec(argv, env, &cwd, timeout, None, &mut outputs, &mut state) {
                Ok(exit) => self.finish(exit, outputs),
                Err(e) => {
                    self.log(&format!("##[error]{e}"));
                    fail("failure")
                }
            };
        }

        let Some(uses) = step.get("uses").map(expr::to_text) else {
            self.log("##[error]A step has run: or uses:.");
            return fail("failure");
        };
        let with: Map<String, Value> = match step.get("with") {
            Some(w) => match expr::interpolate_value(w, &scope) {
                Ok(Value::Object(m)) => m,
                Ok(_) => Map::new(),
                Err(e) => {
                    self.log(&format!("##[error]with: {e}"));
                    return fail("failure");
                }
            },
            None => Map::new(),
        };
        self.uses(&uses, &with, ctx, env, &cwd, timeout)
    }

    fn finish(&mut self, exit: Exit, outputs: Map<String, Value>) -> StepResult {
        let outcome = match exit {
            Exit::Code(0) => "success",
            Exit::Code(code) => {
                self.log(&format!("##[error]The step exited with {code}."));
                "failure"
            }
            Exit::TimedOut => {
                self.log("##[error]The step ran past its timeout-minutes.");
                "failure"
            }
            Exit::Stopped => {
                self.log("##[error]The job was cancelled.");
                "cancelled"
            }
        };
        StepResult { outcome, outputs }
    }

    #[allow(clippy::too_many_arguments)]
    fn uses(
        &mut self,
        uses: &str,
        with: &Map<String, Value>,
        ctx: &Map<String, Value>,
        env: &BTreeMap<String, String>,
        cwd: &str,
        timeout: Duration,
    ) -> StepResult {
        let ok = StepResult {
            outcome: "success",
            outputs: Map::new(),
        };
        let fail = StepResult {
            outcome: "failure",
            outputs: Map::new(),
        };
        if let Some(image) = uses.strip_prefix("docker://") {
            let args = super::sandbox::words(&text(with.get("args")));
            let entry = with.get("entrypoint").map(expr::to_text);
            let mut env = env.clone();
            for (k, v) in with {
                if k != "args" && k != "entrypoint" {
                    env.insert(input_var(k), expr::to_text(v));
                }
            }
            let ws = self.job.sandbox.inside().workspace.clone();
            let mut outputs = Map::new();
            let mut state = Map::new();
            return match self.exec(
                args,
                &env,
                &ws,
                timeout,
                Some((image, entry.as_deref())),
                &mut outputs,
                &mut state,
            ) {
                Ok(exit) => self.finish(exit, outputs),
                Err(e) => {
                    self.log(&format!("##[error]{e}"));
                    fail
                }
            };
        }

        // The actions that stand for GitHub's own services.
        let name = uses.split('@').next().unwrap_or("").to_ascii_lowercase();
        match name.as_str() {
            "actions/checkout" => {
                if with
                    .get("repository")
                    .is_some_and(|r| !expr::to_text(r).is_empty())
                {
                    self.log("##[warning]actions/checkout of another repository is not supported here; the job's own commit is in the workspace.");
                } else {
                    self.log("The commit's files are already in the workspace.");
                }
                return ok;
            }
            "actions/cache" | "actions/cache/restore" | "actions/cache/save" => {
                self.log("There is no cache store on the mesh, so nothing is restored or saved.");
                let mut outputs = Map::new();
                outputs.insert("cache-hit".into(), json!("false"));
                return StepResult {
                    outcome: "success",
                    outputs,
                };
            }
            "actions/upload-artifact" | "actions/download-artifact" => {
                self.log("There is no artifact store on the mesh; this step does nothing.");
                return ok;
            }
            _ => {}
        }

        // Find the action's directory, on the host and inside.
        let inside = self.job.sandbox.inside().clone();
        let (host_dir, inside_dir) = if let Some(rel) = uses.strip_prefix("./") {
            if rel.split('/').any(|s| s == "..") {
                self.log("##[error]A local action stays inside the workspace.");
                return fail;
            }
            // Relative to the workspace, inside a composite action too.
            (
                self.job.dirs.workspace.join(rel),
                format!("{}/{}", inside.workspace, rel.trim_end_matches('/')),
            )
        } else {
            let Some((repo, git_ref, sub)) = split_uses(uses) else {
                self.log(&format!(
                    "##[error]{uses} is not owner/repo@ref, ./path or docker://image."
                ));
                return fail;
            };
            let dir = format!("{repo}/{git_ref}");
            let host = self.job.dirs.actions.join(&dir);
            if !host.exists() {
                self.log(&format!("Downloading {repo}@{git_ref} from GitHub"));
                if let Err(e) = (self.job.fetch)(&repo, &git_ref, &host) {
                    let _ = std::fs::remove_dir_all(&host);
                    self.log(&format!(
                        "##[error]Could not download {repo}@{git_ref}: {e:#}"
                    ));
                    return fail;
                }
            }
            let (h, i) = if sub.is_empty() {
                (host, format!("{}/{dir}", inside.actions))
            } else {
                (host.join(&sub), format!("{}/{dir}/{sub}", inside.actions))
            };
            (h, i)
        };
        let manifest = ["action.yml", "action.yaml"]
            .iter()
            .map(|f| host_dir.join(f))
            .find(|p| p.is_file());
        let Some(manifest) = manifest else {
            self.log(&format!("##[error]{uses} has no action.yml."));
            return fail;
        };
        let action = match std::fs::read_to_string(&manifest)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_yaml::from_str::<serde_yaml::Value>(&t).map_err(|e| e.to_string()))
        {
            Ok(y) => yaml_to_json(&y),
            Err(e) => {
                self.log(&format!("##[error]{}: {e}", manifest.display()));
                return fail;
            }
        };

        // Inputs: what `with` says, else the action's defaults.
        let mut inputs = Map::new();
        let scope_ctx = ctx.clone();
        let scope = Scope::new(&scope_ctx);
        if let Some(declared) = action.get("inputs").and_then(Value::as_object) {
            for (k, spec) in declared {
                let value = match with.get(k) {
                    Some(v) => expr::to_text(v),
                    None => match spec.get("default") {
                        Some(d) => expr::interpolate(&expr::to_text(d), &scope).unwrap_or_default(),
                        None => {
                            if spec.get("required").is_some_and(expr::truthy) {
                                self.log(&format!(
                                    "##[warning]Input required and not supplied: {k}"
                                ));
                            }
                            String::new()
                        }
                    },
                };
                inputs.insert(k.clone(), json!(value));
            }
        }
        for (k, v) in with {
            inputs
                .entry(k.clone())
                .or_insert_with(|| json!(expr::to_text(v)));
        }
        let mut env = env.clone();
        for (k, v) in &inputs {
            env.insert(input_var(k), expr::to_text(v));
        }
        env.insert("GITHUB_ACTION_PATH".into(), inside_dir.clone());
        env.insert(
            "GITHUB_ACTION_REPOSITORY".into(),
            uses.split('@').next().unwrap_or("").into(),
        );
        env.insert(
            "GITHUB_ACTION_REF".into(),
            uses.split('@').nth(1).unwrap_or("").into(),
        );

        let runs = action.get("runs").cloned().unwrap_or(Value::Null);
        let using = text(runs.get("using")).to_ascii_lowercase();
        if using.starts_with("node") {
            let Some(node) = self.job.sandbox.node() else {
                self.log("##[error]JavaScript actions need node, and this runner has none.");
                return fail;
            };
            let main = text(runs.get("main"));
            let mut outputs = Map::new();
            let mut state = Map::new();
            if let Some(pre) = runs.get("pre").map(expr::to_text) {
                let argv = vec![node.clone(), format!("{inside_dir}/{pre}")];
                match self.exec(argv, &env, cwd, timeout, None, &mut Map::new(), &mut state) {
                    Ok(Exit::Code(0)) => {}
                    Ok(exit) => return self.finish(exit, Map::new()),
                    Err(e) => {
                        self.log(&format!("##[error]{e}"));
                        return fail;
                    }
                }
            }
            let argv = vec![node.clone(), format!("{inside_dir}/{main}")];
            let result = match self.exec(argv, &env, cwd, timeout, None, &mut outputs, &mut state) {
                Ok(exit) => self.finish(exit, outputs),
                Err(e) => {
                    self.log(&format!("##[error]{e}"));
                    fail
                }
            };
            if let Some(post) = runs.get("post").map(expr::to_text) {
                self.posts.push(Post {
                    name: format!("Post {uses}"),
                    argv: vec![node, format!("{inside_dir}/{post}")],
                    env,
                    condition: runs
                        .get("post-if")
                        .map(expr::to_text)
                        .unwrap_or_else(|| "always()".into()),
                    state,
                });
            }
            return result;
        }
        if using == "composite" {
            let list = runs
                .get("steps")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut c = ctx.clone();
            c.insert("inputs".into(), Value::Object(inputs));
            let mut inner = Map::new();
            let mut outer_env = env.clone();
            outer_env.retain(|k, _| !k.starts_with("INPUT_"));
            let failed = self.run_steps(&list, &c, &outer_env, &json!({}), &mut inner, true);
            c.insert("steps".into(), Value::Object(inner));
            let scope = Scope::new(&c);
            let mut outputs = Map::new();
            if let Some(decl) = action.get("outputs").and_then(Value::as_object) {
                for (k, spec) in decl {
                    let v = spec
                        .get("value")
                        .map(|v| expr::interpolate(&expr::to_text(v), &scope).unwrap_or_default())
                        .unwrap_or_default();
                    outputs.insert(k.clone(), json!(v));
                }
            }
            return StepResult {
                outcome: if failed { "failure" } else { "success" },
                outputs,
            };
        }
        if using == "docker" {
            let image = text(runs.get("image"));
            let image = match image.strip_prefix("docker://") {
                Some(i) => i.to_string(),
                None => {
                    let mut out = |l: &str| (self.job.log)(l);
                    match self.job.sandbox.build_image(&host_dir, &image, &mut out) {
                        Ok(tag) => tag,
                        Err(e) => {
                            self.log(&format!("##[error]{e:#}"));
                            return fail;
                        }
                    }
                }
            };
            let mut c = ctx.clone();
            c.insert("inputs".into(), Value::Object(inputs));
            let scope = Scope::new(&c);
            let args: Vec<String> = runs
                .get("args")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|v| expr::interpolate(&expr::to_text(v), &scope).unwrap_or_default())
                        .collect()
                })
                .unwrap_or_default();
            if let Some(Value::Object(extra)) = runs.get("env") {
                for (k, v) in extra {
                    env.insert(
                        k.clone(),
                        expr::interpolate(&expr::to_text(v), &scope).unwrap_or_default(),
                    );
                }
            }
            let entry = runs.get("entrypoint").map(expr::to_text);
            let ws = self.job.sandbox.inside().workspace.clone();
            let mut outputs = Map::new();
            let mut state = Map::new();
            return match self.exec(
                args,
                &env,
                &ws,
                timeout,
                Some((&image, entry.as_deref())),
                &mut outputs,
                &mut state,
            ) {
                Ok(exit) => self.finish(exit, outputs),
                Err(e) => {
                    self.log(&format!("##[error]{e}"));
                    fail
                }
            };
        }
        self.log(&format!(
            "##[error]{uses}: runs.using {using} is not supported."
        ));
        fail
    }
}

/// `INPUT_<NAME>`, as GitHub names an input's variable.
fn input_var(name: &str) -> String {
    format!("INPUT_{}", name.replace(' ', "_").to_ascii_uppercase())
}

/// The parts of a [`Scope`] a step carries over to [`Engine::run_step`].
struct ScopeParts {
    failed: bool,
    cancelled: bool,
}

fn scope_owned(s: &Scope<'_>) -> ScopeParts {
    ScopeParts {
        failed: s.failed,
        cancelled: s.cancelled,
    }
}

/// Run the job. The workspace already holds the commit's files.
pub fn run(job: &mut Job<'_>) -> Outcome {
    let mut e = Engine {
        job,
        env: BTreeMap::new(),
        path: Vec::new(),
        masks: Vec::new(),
        failed: false,
        posts: Vec::new(),
        counter: 0,
        parser: commands::Parser::default(),
    };
    let github = e.github();
    let _ = std::fs::write(
        e.job.dirs.temp.join("_event.json"),
        serde_json::to_vec_pretty(github.get("event").unwrap_or(&json!({}))).unwrap_or_default(),
    );
    let ctx = e.base_contexts();

    // The job's env: the workflow's and the job's, interpolated once.
    let mut env = e.default_env();
    {
        let scope = Scope::new(&ctx);
        if let Some(Value::Object(m)) = e.job.payload.get("env") {
            for (k, v) in m {
                match expr::interpolate_value(v, &scope) {
                    Ok(v) => {
                        env.insert(k.clone(), expr::to_text(&v));
                    }
                    Err(err) => {
                        e.log(&format!("##[error]env.{k}: {err}"));
                        e.failed = true;
                    }
                }
            }
        }
    }
    let defaults = e.job.payload.get("defaults").cloned().unwrap_or(json!({}));
    let list = e
        .job
        .payload
        .get("steps")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut steps = Map::new();
    if !e.failed {
        e.run_steps(&list, &ctx, &env, &defaults, &mut steps, false);
    }

    // Post steps, last first.
    let posts = std::mem::take(&mut e.posts);
    for post in posts.into_iter().rev() {
        let mut c = ctx.clone();
        c.insert("steps".into(), Value::Object(steps.clone()));
        let scope = Scope {
            failed: e.failed,
            cancelled: (e.job.stop)(),
            ..Scope::new(&c)
        };
        if !expr::condition(Some(&post.condition), &scope).unwrap_or(true) {
            continue;
        }
        e.log(&format!("##[group]{}", post.name));
        let mut env = post.env.clone();
        for (k, v) in &post.state {
            env.insert(format!("STATE_{k}"), expr::to_text(v));
        }
        let ws = e.job.sandbox.inside().workspace.clone();
        let mut state = Map::new();
        let exit = e.exec(
            post.argv,
            &env,
            &ws,
            Duration::from_secs(3600),
            None,
            &mut Map::new(),
            &mut state,
        );
        if !matches!(exit, Ok(Exit::Code(0))) {
            e.log("##[warning]The post step did not succeed.");
        }
        e.log("##[endgroup]");
    }

    // The job's outputs, from its steps.
    let mut c = ctx.clone();
    c.insert("steps".into(), Value::Object(steps));
    let scope = Scope::new(&c);
    let mut outputs = Map::new();
    if let Some(Value::Object(m)) = e.job.payload.get("outputs") {
        for (k, v) in m {
            let value = expr::interpolate(&expr::to_text(v), &scope).unwrap_or_default();
            outputs.insert(k.clone(), json!(commands::mask(&value, &e.masks)));
        }
    }
    let state = if (e.job.stop)() {
        "cancelled"
    } else if e.failed {
        "failure"
    } else {
        "success"
    };
    Outcome { state, outputs }
}

#[cfg(test)]
mod tests {
    use super::super::sandbox::Host;
    use super::*;

    fn temp() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let d = std::env::temp_dir().join(format!(
            "losos-ci-job-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn payload(steps: &str, outputs: Value) -> Value {
        let steps: serde_yaml::Value = serde_yaml::from_str(steps).unwrap();
        json!({
            "name": "t",
            "env": { "GREETING": "hi ${{ matrix.n }}" },
            "defaults": { "run": {} },
            "steps": yaml_to_json(&steps),
            "outputs": outputs,
            "contexts": {
                "github": { "sha": "a", "event": {}, "repository": "matus/site" },
                "matrix": { "n": 2 },
                "needs": {},
                "secrets": {},
                "vars": {},
                "inputs": {},
            },
        })
    }

    fn go(p: &Value, setup: impl FnOnce(&Dirs)) -> (Outcome, String) {
        let root = temp();
        let dirs = Dirs::new(&root).unwrap();
        setup(&dirs);
        let mut host = Host::new(&dirs);
        let mut lines = Vec::new();
        let mut log = |l: &str| lines.push(l.to_string());
        let fetch = |_: &str, _: &str, _: &Path| -> anyhow::Result<()> { anyhow::bail!("offline") };
        let mut job = Job {
            payload: p,
            dirs: &dirs,
            runner_name: "box-1",
            sandbox: &mut host,
            fetch: &fetch,
            stop: &|| false,
            log: &mut log,
        };
        let out = run(&mut job);
        let _ = std::fs::remove_dir_all(root);
        (out, lines.join("\n"))
    }

    #[test]
    fn steps_env_outputs_and_masks() {
        let p = payload(
            r#"
- id: one
  run: |
    echo "$GREETING"
    echo "word=hello" >> "$GITHUB_OUTPUT"
    echo "LATER=from-env" >> "$GITHUB_ENV"
    echo "::add-mask::s3cret"
    echo "the s3cret is safe"
- id: two
  if: steps.one.outputs.word == 'hello'
  run: echo "$LATER ${{ steps.one.outputs.word }}"
- run: exit 3
  continue-on-error: true
- name: never
  if: failure()
  run: echo should not run
- run: echo "after"
"#,
            json!({ "word": "${{ steps.one.outputs.word }}" }),
        );
        let (out, log) = go(&p, |_| {});
        assert_eq!(out.state, "success", "{log}");
        assert_eq!(out.outputs["word"], "hello");
        assert!(log.contains("hi 2"), "{log}");
        assert!(log.contains("from-env hello"), "{log}");
        assert!(log.contains("the *** is safe"), "{log}");
        assert!(!log.contains("should not run"));
        assert!(log.contains("##[group]never\n##[skipped]"), "{log}");
    }

    #[test]
    fn a_failure_skips_the_rest_but_always_runs() {
        let p = payload(
            r#"
- run: "false"
- run: echo skipped
- if: always()
  run: echo cleanup
- if: failure()
  run: echo "status ${{ job.status }}"
"#,
            json!({}),
        );
        let (out, log) = go(&p, |_| {});
        assert_eq!(out.state, "failure");
        assert!(log.contains("cleanup"));
        assert!(log.contains("status failure"), "{log}");
        assert!(!log.contains("\nskipped"));
    }

    #[test]
    fn composite_and_local_actions() {
        let p = payload(
            r#"
- uses: actions/checkout@v4
- id: greet
  uses: ./.github/actions/greet
  with:
    who: mesh
- run: echo "said ${{ steps.greet.outputs.said }}"
- uses: actions/cache@v4
  id: cache
- run: echo "hit ${{ steps.cache.outputs.cache-hit }}"
"#,
            json!({}),
        );
        let (out, log) = go(&p, |d| {
            let a = d.workspace.join(".github/actions/greet");
            std::fs::create_dir_all(&a).unwrap();
            std::fs::write(
                a.join("action.yml"),
                r#"
name: greet
inputs:
  who: { required: true }
  how: { default: hello }
outputs:
  said: { value: "${{ steps.s.outputs.text }}" }
runs:
  using: composite
  steps:
    - id: s
      shell: bash
      run: |
        echo "${{ inputs.how }} $INPUT_WHO"
        echo "text=${{ inputs.how }} ${{ inputs.who }}" >> "$GITHUB_OUTPUT"
"#,
            )
            .unwrap();
        });
        assert_eq!(out.state, "success", "{log}");
        assert!(log.contains("hello mesh"), "{log}");
        assert!(log.contains("said hello mesh"), "{log}");
        assert!(log.contains("hit false"), "{log}");
        assert!(log.contains("already in the workspace"));
    }

    #[test]
    fn path_and_timeouts() {
        let p = payload(
            r#"
- run: |
    mkdir -p "$HOME/bin"
    printf '#!/bin/sh\necho tool ran\n' > "$HOME/bin/mytool"
    chmod +x "$HOME/bin/mytool"
    echo "$HOME/bin" >> "$GITHUB_PATH"
- run: mytool
- run: sleep 5
  timeout-minutes: 0.01
"#,
            json!({}),
        );
        let (out, log) = go(&p, |_| {});
        assert!(log.contains("tool ran"), "{log}");
        assert!(log.contains("past its timeout-minutes"), "{log}");
        assert_eq!(out.state, "failure");
    }

    #[test]
    fn hashing_files() {
        let root = temp();
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::write(root.join("a/Cargo.lock"), "x").unwrap();
        let h = hash_files(&root, &["**/Cargo.lock".into()]);
        assert_eq!(h.len(), 64);
        assert_eq!(hash_files(&root, &["*.none".into()]), "");
        let _ = std::fs::remove_dir_all(root);
        assert_eq!(
            split_uses("actions/setup-node@v4"),
            Some(("actions/setup-node".into(), "v4".into(), String::new()))
        );
        assert_eq!(
            split_uses("github/codeql-action/init@v3"),
            Some(("github/codeql-action".into(), "v3".into(), "init".into()))
        );
        assert!(split_uses("../x@v1").is_none());
    }
}
