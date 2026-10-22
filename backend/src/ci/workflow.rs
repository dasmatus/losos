//! GitHub Actions workflows: reading `.github/workflows/*.yml`, deciding
//! which ones an event starts, and turning one job into the jobs that run
//! (one per matrix combination).
//!
//! What is read follows GitHub's workflow syntax. The parts that need
//! GitHub's own services are said plainly where they come up: reusable
//! workflows (`jobs.<id>.uses`) and `paths` filters are refused or ignored
//! with a note, `secrets` is empty, `schedule` never fires.

use super::expr::{self, Scope};
use super::glob;
use serde_json::{json, Map, Value};

/// The events a workflow can be started by on LosOS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Push {
        r#ref: String,
        before: String,
        after: String,
        pusher: String,
    },
    PullRequest {
        action: String,
        number: i64,
        title: String,
        head_ref: String,
        head_sha: String,
        base_ref: String,
        author: String,
    },
    /// The Run button: `workflow_dispatch`.
    Dispatch {
        r#ref: String,
        sha: String,
        actor: String,
    },
}

impl Event {
    /// `github.event_name`.
    pub fn name(&self) -> &'static str {
        match self {
            Event::Push { .. } => "push",
            Event::PullRequest { .. } => "pull_request",
            Event::Dispatch { .. } => "workflow_dispatch",
        }
    }

    /// The commit the jobs check out.
    pub fn sha(&self) -> &str {
        match self {
            Event::Push { after, .. } => after,
            Event::PullRequest { head_sha, .. } => head_sha,
            Event::Dispatch { sha, .. } => sha,
        }
    }

    /// `github.ref`. A pull request's jobs run on its head commit, so the
    /// ref names the head (`refs/pull/7/head`); LosOS makes no merge
    /// commit to test.
    pub fn git_ref(&self) -> String {
        match self {
            Event::Push { r#ref, .. } | Event::Dispatch { r#ref, .. } => r#ref.clone(),
            Event::PullRequest { number, .. } => format!("refs/pull/{number}/head"),
        }
    }

    pub fn actor(&self) -> &str {
        match self {
            Event::Push { pusher, .. } => pusher,
            Event::PullRequest { author, .. } => author,
            Event::Dispatch { actor, .. } => actor,
        }
    }

    /// One line for the run's list: the commit's ref or the PR's title.
    pub fn title(&self) -> String {
        match self {
            Event::Push { r#ref, .. } | Event::Dispatch { r#ref, .. } => short_ref(r#ref).into(),
            Event::PullRequest { number, title, .. } => format!("#{number} {title}"),
        }
    }
}

/// `refs/heads/main` → `main`.
pub fn short_ref(r: &str) -> &str {
    r.strip_prefix("refs/heads/")
        .or_else(|| r.strip_prefix("refs/tags/"))
        .unwrap_or(r)
}

/// One trigger of `on:` with its filters.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Trigger {
    pub event: String,
    pub branches: Option<Vec<String>>,
    pub branches_ignore: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub tags_ignore: Option<Vec<String>>,
    pub types: Option<Vec<String>>,
}

/// A parsed workflow file.
#[derive(Debug, Clone)]
pub struct Workflow {
    /// `.github/workflows/ci.yml`.
    pub file: String,
    /// `name:`, or the file's path as GitHub shows it.
    pub name: String,
    pub on: Vec<Trigger>,
    /// The whole document as JSON, for the planner.
    pub doc: Map<String, Value>,
    /// The jobs in file order.
    pub jobs: Vec<String>,
}

/// YAML to JSON, every key a string (`on: 3:` and the like).
pub fn yaml_to_json(y: &serde_yaml::Value) -> Value {
    use serde_yaml::Value as Y;
    match y {
        Y::Null => Value::Null,
        Y::Bool(b) => Value::Bool(*b),
        Y::Number(n) => {
            if let Some(i) = n.as_i64() {
                json!(i)
            } else if let Some(u) = n.as_u64() {
                json!(u)
            } else {
                n.as_f64()
                    .and_then(serde_json::Number::from_f64)
                    .map(Value::Number)
                    .unwrap_or(Value::Null)
            }
        }
        Y::String(s) => Value::String(s.clone()),
        Y::Sequence(items) => Value::Array(items.iter().map(yaml_to_json).collect()),
        Y::Mapping(m) => Value::Object(
            m.iter()
                .map(|(k, v)| {
                    let key = match k {
                        Y::String(s) => s.clone(),
                        Y::Bool(b) => b.to_string(),
                        other => expr::to_text(&yaml_to_json(other)),
                    };
                    (key, yaml_to_json(v))
                })
                .collect(),
        ),
        Y::Tagged(t) => yaml_to_json(&t.value),
    }
}

fn strings(v: Option<&Value>) -> Option<Vec<String>> {
    match v? {
        Value::String(s) => Some(vec![s.clone()]),
        Value::Array(items) => Some(items.iter().map(expr::to_text).collect()),
        _ => None,
    }
}

fn triggers(on: &Value) -> Result<Vec<Trigger>, String> {
    let one = |event: &str| Trigger {
        event: event.to_string(),
        ..Trigger::default()
    };
    Ok(match on {
        Value::String(e) => vec![one(e)],
        Value::Array(items) => items.iter().map(|e| one(&expr::to_text(e))).collect(),
        Value::Object(m) => m
            .iter()
            .map(|(event, cfg)| {
                let get = |k: &str| strings(cfg.get(k));
                Trigger {
                    event: event.clone(),
                    branches: get("branches"),
                    branches_ignore: get("branches-ignore"),
                    tags: get("tags"),
                    tags_ignore: get("tags-ignore"),
                    types: get("types"),
                }
            })
            .collect(),
        _ => return Err("`on:` names the events that start the workflow".into()),
    })
}

/// Read one workflow file.
pub fn parse(file: &str, text: &str) -> Result<Workflow, String> {
    let yaml: serde_yaml::Value = serde_yaml::from_str(text).map_err(|e| format!("{file}: {e}"))?;
    let Value::Object(doc) = yaml_to_json(&yaml) else {
        return Err(format!("{file}: a workflow is a mapping"));
    };
    let on = doc
        .get("on")
        .or_else(|| doc.get("true"))
        .ok_or_else(|| format!("{file}: there is no `on:`"))?;
    let on = triggers(on).map_err(|e| format!("{file}: {e}"))?;
    let Some(Value::Object(jobs)) = doc.get("jobs") else {
        return Err(format!("{file}: there is no `jobs:`"));
    };
    if jobs.is_empty() {
        return Err(format!("{file}: `jobs:` is empty"));
    }
    for (id, job) in jobs {
        if !job.is_object() {
            return Err(format!("{file}: job {id} is not a mapping"));
        }
        if job.get("uses").is_some() {
            return Err(format!(
                "{file}: job {id} calls a reusable workflow, which LosOS does not run yet"
            ));
        }
        if job.get("steps").and_then(Value::as_array).is_none() {
            return Err(format!("{file}: job {id} has no steps"));
        }
        for need in needs_of(job) {
            if !jobs.contains_key(&need) {
                return Err(format!(
                    "{file}: job {id} needs {need}, which is not a job here"
                ));
            }
        }
    }
    let order: Vec<String> = jobs.keys().cloned().collect();
    cycle_free(jobs, &order).map_err(|e| format!("{file}: {e}"))?;
    let name = doc
        .get("name")
        .map(expr::to_text)
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| file.to_string());
    Ok(Workflow {
        file: file.to_string(),
        name,
        on,
        doc,
        jobs: order,
    })
}

/// `needs:` of one job.
pub fn needs_of(job: &Value) -> Vec<String> {
    strings(job.get("needs")).unwrap_or_default()
}

fn cycle_free(jobs: &Map<String, Value>, order: &[String]) -> Result<(), String> {
    fn visit(
        id: &str,
        jobs: &Map<String, Value>,
        seen: &mut Vec<String>,
        done: &mut Vec<String>,
    ) -> Result<(), String> {
        if done.iter().any(|d| d == id) {
            return Ok(());
        }
        if seen.iter().any(|s| s == id) {
            return Err(format!("jobs need each other in a circle ({id})"));
        }
        seen.push(id.to_string());
        for n in needs_of(&jobs[id]) {
            visit(&n, jobs, seen, done)?;
        }
        seen.pop();
        done.push(id.to_string());
        Ok(())
    }
    let mut done = Vec::new();
    for id in order {
        visit(id, jobs, &mut Vec::new(), &mut done)?;
    }
    Ok(())
}

fn branch_or_tag(trigger: &Trigger, git_ref: &str) -> bool {
    let (name, wanted, ignored, other_wanted, other_ignored) =
        if let Some(b) = git_ref.strip_prefix("refs/heads/") {
            (
                b,
                &trigger.branches,
                &trigger.branches_ignore,
                &trigger.tags,
                &trigger.tags_ignore,
            )
        } else if let Some(t) = git_ref.strip_prefix("refs/tags/") {
            (
                t,
                &trigger.tags,
                &trigger.tags_ignore,
                &trigger.branches,
                &trigger.branches_ignore,
            )
        } else {
            return false;
        };
    match (wanted, ignored) {
        (Some(w), _) => glob::filter(w, name),
        (None, Some(i)) => !i.iter().any(|p| glob::matches(p, name)),
        // Only filters for the other kind of ref: this kind never runs.
        (None, None) => other_wanted.is_none() && other_ignored.is_none(),
    }
}

impl Workflow {
    /// Whether `event` starts this workflow.
    pub fn started_by(&self, event: &Event) -> bool {
        self.on.iter().any(|t| match event {
            Event::Push { r#ref, .. } => t.event == "push" && branch_or_tag(t, r#ref),
            Event::PullRequest {
                action, base_ref, ..
            } => {
                let pr = t.event == "pull_request" || t.event == "pull_request_target";
                let types = t.types.clone().unwrap_or_else(|| {
                    vec!["opened".into(), "synchronize".into(), "reopened".into()]
                });
                let base = short_ref(base_ref);
                let branch_ok = match (&t.branches, &t.branches_ignore) {
                    (Some(w), _) => glob::filter(w, base),
                    (None, Some(i)) => !i.iter().any(|p| glob::matches(p, base)),
                    (None, None) => true,
                };
                pr && types.iter().any(|ty| ty == action) && branch_ok
            }
            Event::Dispatch { .. } => t.event == "workflow_dispatch",
        })
    }

    /// Whether the Run button may start it.
    pub fn dispatchable(&self) -> bool {
        self.on.iter().any(|t| t.event == "workflow_dispatch")
    }

    pub fn job(&self, id: &str) -> &Value {
        self.doc
            .get("jobs")
            .and_then(|j| j.get(id))
            .unwrap_or(&Value::Null)
    }
}

/// What every job of a run knows about it: `github`, the base the
/// runner's contexts start from.
pub struct RunInfo<'a> {
    pub owner: &'a str,
    pub repo: &'a str,
    pub run_id: i64,
    pub run_number: i64,
    pub workflow: &'a Workflow,
    pub event: &'a Event,
    /// `https://<box>/git`, `github.server_url`.
    pub server_url: &'a str,
}

/// The `github` context of a run (the job's id is filled in per job).
pub fn github_context(r: &RunInfo<'_>) -> Value {
    let full = format!("{}/{}", r.owner, r.repo);
    let repository = json!({
        "name": r.repo,
        "full_name": full,
        "owner": { "login": r.owner, "name": r.owner },
        "html_url": format!("{}/{full}", r.server_url),
        "clone_url": format!("{}/{full}.git", r.server_url),
    });
    let git_ref = r.event.git_ref();
    let (payload, head_ref, base_ref) = match r.event {
        Event::Push {
            r#ref,
            before,
            after,
            pusher,
        } => (
            json!({
                "ref": r#ref, "before": before, "after": after,
                "pusher": { "name": pusher },
                "head_commit": { "id": after },
                "repository": repository,
                "sender": { "login": pusher },
            }),
            String::new(),
            String::new(),
        ),
        Event::PullRequest {
            action,
            number,
            title,
            head_ref,
            head_sha,
            base_ref,
            author,
        } => (
            json!({
                "action": action,
                "number": number,
                "pull_request": {
                    "number": number, "title": title,
                    "head": { "ref": short_ref(head_ref), "sha": head_sha },
                    "base": { "ref": short_ref(base_ref) },
                    "user": { "login": author },
                },
                "repository": repository,
                "sender": { "login": author },
            }),
            short_ref(head_ref).to_string(),
            short_ref(base_ref).to_string(),
        ),
        Event::Dispatch { r#ref, actor, .. } => (
            json!({ "ref": r#ref, "inputs": {}, "repository": repository, "sender": { "login": actor } }),
            String::new(),
            String::new(),
        ),
    };
    let ref_type = if git_ref.starts_with("refs/tags/") {
        "tag"
    } else {
        "branch"
    };
    let ref_name = match r.event {
        Event::PullRequest { number, .. } => format!("{number}/merge"),
        _ => short_ref(&git_ref).to_string(),
    };
    json!({
        "event_name": r.event.name(),
        "event": payload,
        "sha": r.event.sha(),
        "ref": git_ref,
        "ref_name": ref_name,
        "ref_type": ref_type,
        "ref_protected": false,
        "head_ref": head_ref,
        "base_ref": base_ref,
        "repository": full,
        "repository_owner": r.owner,
        "repositoryUrl": format!("{}/{full}.git", r.server_url),
        "actor": r.event.actor(),
        "triggering_actor": r.event.actor(),
        "run_id": r.run_id.to_string(),
        "run_number": r.run_number.to_string(),
        "run_attempt": "1",
        "workflow": r.workflow.name,
        "workflow_ref": format!("{full}/{}@{git_ref}", r.workflow.file),
        "server_url": r.server_url,
        "api_url": format!("{}/api/v1", r.server_url),
        "graphql_url": "",
        "workspace": "/github/workspace",
        "action": "",
        "token": "",
        "retention_days": "0",
        "secret_source": "None",
    })
}

/// One combination of a job's matrix, with what the planner worked out.
#[derive(Debug, Clone, PartialEq)]
pub struct Planned {
    /// The check's name: `test (20, ubuntu)`.
    pub name: String,
    pub matrix: Map<String, Value>,
    /// What the runner gets (`exec`).
    pub payload: Value,
    pub timeout_minutes: u32,
    /// `runs-on` names `self-hosted`: the job stays on this box.
    pub self_hosted: bool,
    /// `continue-on-error`: a failure does not fail the run.
    pub continue_on_error: bool,
}

/// GitHub's matrix: the product of the lists, less `exclude`, plus
/// `include`.
pub fn expand_matrix(matrix: &Value) -> Result<Vec<Map<String, Value>>, String> {
    let Some(m) = matrix.as_object() else {
        return if matrix.is_null() {
            Ok(vec![Map::new()])
        } else {
            Err("strategy.matrix is a mapping".into())
        };
    };
    let mut combos: Vec<Map<String, Value>> = vec![Map::new()];
    let mut keys = Vec::new();
    for (k, v) in m {
        if k == "include" || k == "exclude" {
            continue;
        }
        let Value::Array(values) = v else {
            return Err(format!("matrix.{k} is a list"));
        };
        keys.push(k.clone());
        let mut next = Vec::new();
        for c in &combos {
            for v in values {
                let mut c = c.clone();
                c.insert(k.clone(), v.clone());
                next.push(c);
            }
        }
        combos = next;
    }
    if keys.is_empty() {
        combos.clear();
    }
    let same = |a: &Value, b: &Value| a == b || expr::to_text(a) == expr::to_text(b);
    if let Some(Value::Array(ex)) = m.get("exclude") {
        for e in ex.iter().filter_map(Value::as_object) {
            combos.retain(|c| {
                !e.iter()
                    .all(|(k, v)| c.get(k).is_some_and(|cv| same(cv, v)))
            });
        }
    }
    if let Some(Value::Array(inc)) = m.get("include") {
        let base = combos.len();
        for i in inc.iter().filter_map(Value::as_object) {
            let mut added = false;
            for c in combos.iter_mut().take(base) {
                let fits = i
                    .iter()
                    .filter(|(k, _)| keys.contains(k))
                    .all(|(k, v)| c.get(k).is_some_and(|cv| same(cv, v)));
                if fits {
                    for (k, v) in i {
                        if !keys.contains(k) {
                            c.insert(k.clone(), v.clone());
                        }
                    }
                    added = true;
                }
            }
            if !added {
                combos.push(i.clone());
            }
        }
    }
    if combos.is_empty() {
        return Err("the matrix has no combinations".into());
    }
    if combos.len() > 256 {
        return Err("a matrix has at most 256 combinations".into());
    }
    Ok(combos)
}

fn labels(runs_on: &Value) -> Vec<String> {
    match runs_on {
        Value::Object(m) => strings(m.get("labels")).unwrap_or_default(),
        other => strings(Some(other)).unwrap_or_default(),
    }
}

/// Merge `defaults:` of the workflow and of the job, the job's winning.
fn defaults(workflow: &Map<String, Value>, job: &Value) -> Value {
    let mut run = workflow
        .get("defaults")
        .and_then(|d| d.get("run"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(job_run) = job
        .get("defaults")
        .and_then(|d| d.get("run"))
        .and_then(Value::as_object)
    {
        for (k, v) in job_run {
            run.insert(k.clone(), v.clone());
        }
    }
    json!({ "run": run })
}

/// Plan job `id` now that its needs are done: decide its `if`, expand its
/// matrix, and work out everything that may be worked out before it runs.
/// `Ok(None)`: its `if` says no (skipped).
pub fn plan_job(
    wf: &Workflow,
    id: &str,
    github: &Value,
    needs: &Map<String, Value>,
    needs_failed: bool,
    cancelled: bool,
) -> Result<Option<Vec<Planned>>, String> {
    let job = wf.job(id);
    let mut contexts = Map::new();
    contexts.insert("github".into(), github.clone());
    contexts.insert("needs".into(), Value::Object(needs.clone()));
    contexts.insert("vars".into(), json!({}));
    contexts.insert("inputs".into(), json!({}));
    let scope = Scope {
        failed: needs_failed,
        cancelled,
        ..Scope::new(&contexts)
    };
    let cond = job.get("if").map(expr::to_text);
    if !expr::condition(cond.as_deref(), &scope).map_err(|e| format!("jobs.{id}.if: {e}"))? {
        return Ok(None);
    }
    let strategy = match job.get("strategy") {
        Some(s) => {
            expr::interpolate_value(s, &scope).map_err(|e| format!("jobs.{id}.strategy: {e}"))?
        }
        None => Value::Null,
    };
    let combos = expand_matrix(strategy.get("matrix").unwrap_or(&Value::Null))
        .map_err(|e| format!("jobs.{id}: {e}"))?;
    let total = combos.len();
    let fail_fast = strategy.get("fail-fast").map(expr::truthy).unwrap_or(true);
    let mut out = Vec::new();
    for (index, matrix) in combos.into_iter().enumerate() {
        let mut ctx = contexts.clone();
        ctx.insert("matrix".into(), Value::Object(matrix.clone()));
        ctx.insert(
            "strategy".into(),
            json!({ "fail-fast": fail_fast, "job-index": index, "job-total": total, "max-parallel": total }),
        );
        let s = Scope::new(&ctx);
        let at = |what: &str, e: String| format!("jobs.{id}.{what}: {e}");
        let name = match job.get("name") {
            Some(n) => expr::interpolate(&expr::to_text(n), &s).map_err(|e| at("name", e))?,
            None if matrix.is_empty() => id.to_string(),
            None => format!(
                "{id} ({})",
                matrix
                    .values()
                    .map(|v| match v {
                        Value::Object(_) | Value::Array(_) =>
                            serde_json::to_string(v).unwrap_or_default(),
                        _ => expr::to_text(v),
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        let runs_on = expr::interpolate_value(job.get("runs-on").unwrap_or(&Value::Null), &s)
            .map_err(|e| at("runs-on", e))?;
        let labels = labels(&runs_on);
        if labels.is_empty() && job.get("container").is_none() {
            return Err(format!("jobs.{id}: runs-on names no runner"));
        }
        let container = match job.get("container") {
            Some(c) => expr::interpolate_value(c, &s).map_err(|e| at("container", e))?,
            None => Value::Null,
        };
        let services = match job.get("services") {
            Some(c) => expr::interpolate_value(c, &s).map_err(|e| at("services", e))?,
            None => Value::Null,
        };
        let timeout = match job.get("timeout-minutes") {
            Some(t) => expr::interpolate_value(t, &s).map_err(|e| at("timeout-minutes", e))?,
            None => json!(360),
        };
        let timeout_minutes = expr::to_text(&timeout)
            .trim()
            .parse::<f64>()
            .unwrap_or(360.0);
        let timeout_minutes = timeout_minutes.clamp(1.0, 360.0) as u32;
        let continue_on_error = match job.get("continue-on-error") {
            Some(c) => expr::truthy(
                &expr::interpolate_value(c, &s).map_err(|e| at("continue-on-error", e))?,
            ),
            None => false,
        };
        let mut env = wf
            .doc
            .get("env")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        if let Some(job_env) = job.get("env").and_then(Value::as_object) {
            for (k, v) in job_env {
                env.insert(k.clone(), v.clone());
            }
        }
        let mut github = github.clone();
        github["job"] = json!(id);
        let payload = json!({
            "workflow": wf.name,
            "workflowFile": wf.file,
            "jobId": id,
            "name": name,
            "runsOn": labels,
            "container": container,
            "services": services,
            "env": env,
            "defaults": defaults(&wf.doc, job),
            "steps": job.get("steps").cloned().unwrap_or(json!([])),
            "outputs": job.get("outputs").cloned().unwrap_or(json!({})),
            "timeoutMinutes": timeout_minutes,
            "contexts": {
                "github": github,
                "needs": needs,
                "matrix": matrix,
                "strategy": ctx["strategy"],
                "vars": {},
                "inputs": {},
                "secrets": { "GITHUB_TOKEN": "" },
            },
        });
        out.push(Planned {
            self_hosted: labels.iter().any(|l| l.eq_ignore_ascii_case("self-hosted")),
            name,
            matrix,
            payload,
            timeout_minutes,
            continue_on_error,
        });
    }
    Ok(Some(out))
}

/// Every workflow file in a commit's archive (`.github/workflows/*.yml`
/// and `*.yaml`, not in subdirectories), with its text.
pub fn files_in_archive(archive: &[u8]) -> std::io::Result<Vec<(String, String)>> {
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    let mut out = Vec::new();
    for entry in tar.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_string_lossy().to_string();
        let path = path.trim_start_matches("./").to_string();
        let Some(name) = path.strip_prefix(".github/workflows/") else {
            continue;
        };
        if name.contains('/') || !(name.ends_with(".yml") || name.ends_with(".yaml")) {
            continue;
        }
        if entry.header().entry_type() != tar::EntryType::Regular || entry.size() > 512 * 1024 {
            continue;
        }
        let mut text = String::new();
        use std::io::Read;
        if entry.read_to_string(&mut text).is_ok() {
            out.push((path, text));
        }
    }
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CI: &str = r#"
name: CI
on:
  push:
    branches: [main, 'release/**']
  pull_request:
  workflow_dispatch:
env:
  GREETING: hello
jobs:
  build:
    runs-on: ubuntu-latest
    outputs:
      version: ${{ steps.v.outputs.version }}
    steps:
      - uses: actions/checkout@v4
      - id: v
        run: echo "version=1.2" >> "$GITHUB_OUTPUT"
  test:
    needs: build
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, self-hosted]
        node: [18, 20]
        exclude:
          - os: self-hosted
            node: 18
        include:
          - node: 20
            experimental: true
          - os: windows-latest
            node: 22
    steps:
      - run: echo ${{ matrix.node }}
  deploy:
    needs: [build, test]
    if: github.ref == 'refs/heads/main'
    runs-on: ubuntu-latest
    steps:
      - run: echo ${{ needs.build.outputs.version }}
"#;

    fn push(r: &str) -> Event {
        Event::Push {
            r#ref: r.into(),
            before: "0".repeat(40),
            after: "a".repeat(40),
            pusher: "matus".into(),
        }
    }

    fn pr(action: &str, base: &str) -> Event {
        Event::PullRequest {
            action: action.into(),
            number: 7,
            title: "Fix".into(),
            head_ref: "refs/heads/fix".into(),
            head_sha: "b".repeat(40),
            base_ref: base.into(),
            author: "matus".into(),
        }
    }

    #[test]
    fn triggers_and_filters() {
        let wf = parse(".github/workflows/ci.yml", CI).unwrap();
        assert_eq!(wf.name, "CI");
        assert_eq!(wf.jobs, ["build", "test", "deploy"]);
        assert!(wf.started_by(&push("refs/heads/main")));
        assert!(wf.started_by(&push("refs/heads/release/1.0")));
        assert!(!wf.started_by(&push("refs/heads/dev")));
        assert!(!wf.started_by(&push("refs/tags/v1")));
        assert!(wf.started_by(&pr("opened", "refs/heads/dev")));
        assert!(!wf.started_by(&pr("closed", "refs/heads/main")));
        assert!(wf.dispatchable());

        let tags = parse(
            "t.yml",
            "on:\n  push:\n    tags: ['v*']\njobs:\n  a:\n    runs-on: x\n    steps: [{run: x}]\n",
        )
        .unwrap();
        assert!(tags.started_by(&push("refs/tags/v1.0")));
        assert!(!tags.started_by(&push("refs/heads/main")));
        let plain = parse(
            "p.yml",
            "on: [push]\njobs:\n  a:\n    runs-on: x\n    steps: [{run: x}]\n",
        )
        .unwrap();
        assert!(plain.started_by(&push("refs/heads/anything")));
        assert!(plain.started_by(&push("refs/tags/v1")));
    }

    #[test]
    fn broken_workflows_say_why() {
        let e = parse(
            "a.yml",
            "on: push\njobs:\n  a:\n    needs: b\n    runs-on: x\n    steps: [{run: x}]\n",
        )
        .unwrap_err();
        assert!(e.contains("needs b"), "{e}");
        let e = parse("a.yml", "on: push\njobs:\n  a:\n    needs: b\n    runs-on: x\n    steps: [{run: x}]\n  b:\n    needs: a\n    runs-on: x\n    steps: [{run: x}]\n").unwrap_err();
        assert!(e.contains("circle"), "{e}");
        let e = parse(
            "a.yml",
            "on: push\njobs:\n  a:\n    uses: o/r/.github/workflows/x.yml@v1\n",
        )
        .unwrap_err();
        assert!(e.contains("reusable"), "{e}");
        assert!(parse("a.yml", "on: [push\n").is_err());
    }

    #[test]
    fn matrix_include_and_exclude() {
        let m = json!({
            "os": ["ubuntu-latest", "self-hosted"],
            "node": [18, 20],
            "exclude": [{ "os": "self-hosted", "node": 18 }],
            "include": [{ "node": 20, "experimental": true }, { "os": "windows-latest", "node": 22 }],
        });
        let c = expand_matrix(&m).unwrap();
        assert_eq!(c.len(), 4);
        assert_eq!(
            c[0],
            json!({"os": "ubuntu-latest", "node": 18})
                .as_object()
                .unwrap()
                .clone()
        );
        assert_eq!(c[1]["experimental"], json!(true));
        assert_eq!(c[2]["experimental"], json!(true));
        assert_eq!(
            c[3],
            json!({"os": "windows-latest", "node": 22})
                .as_object()
                .unwrap()
                .clone()
        );
        let only = expand_matrix(&json!({ "include": [{ "a": 1 }, { "a": 2 }] })).unwrap();
        assert_eq!(only.len(), 2);
    }

    #[test]
    fn planning_follows_needs_and_if() {
        let wf = parse("ci.yml", CI).unwrap();
        let ev = push("refs/heads/main");
        let run = RunInfo {
            owner: "matus",
            repo: "site",
            run_id: 3,
            run_number: 1,
            workflow: &wf,
            event: &ev,
            server_url: "https://mattbox.local/git",
        };
        let gh = github_context(&run);
        assert_eq!(gh["ref_name"], "main");
        let build = plan_job(&wf, "build", &gh, &Map::new(), false, false)
            .unwrap()
            .unwrap();
        assert_eq!(build.len(), 1);
        assert_eq!(build[0].name, "build");
        assert_eq!(build[0].payload["env"]["GREETING"], "hello");

        let needs = json!({ "build": { "result": "success", "outputs": { "version": "1.2" } } });
        let test = plan_job(&wf, "test", &gh, needs.as_object().unwrap(), false, false)
            .unwrap()
            .unwrap();
        assert_eq!(test.len(), 4);
        assert_eq!(test[0].name, "test (ubuntu-latest, 18)");
        assert!(test[2].self_hosted);
        assert_eq!(test[2].payload["runsOn"], json!(["self-hosted"]));
        assert_eq!(test[3].payload["runsOn"], json!(["windows-latest"]));

        // A failed need skips a job without a status check in its if.
        assert!(
            plan_job(&wf, "deploy", &gh, needs.as_object().unwrap(), true, false)
                .unwrap()
                .is_none()
        );
        let other = push("refs/heads/release/2");
        let gh2 = github_context(&RunInfo {
            event: &other,
            ..run
        });
        assert!(plan_job(
            &wf,
            "deploy",
            &gh2,
            needs.as_object().unwrap(),
            false,
            false
        )
        .unwrap()
        .is_none());
        assert!(
            plan_job(&wf, "deploy", &gh, needs.as_object().unwrap(), false, false)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn workflows_come_out_of_the_archive() {
        let mut tar = tar::Builder::new(Vec::new());
        for (path, body) in [
            (".github/workflows/ci.yml", "on: push"),
            (".github/workflows/sub/x.yml", "nope"),
            ("README.md", "hi"),
        ] {
            let mut h = tar::Header::new_gnu();
            h.set_size(body.len() as u64);
            h.set_mode(0o644);
            h.set_cksum();
            tar.append_data(&mut h, path, body.as_bytes()).unwrap();
        }
        let raw = tar.into_inner().unwrap();
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        use std::io::Write;
        gz.write_all(&raw).unwrap();
        let files = files_in_archive(&gz.finish().unwrap()).unwrap();
        assert_eq!(
            files,
            vec![(
                ".github/workflows/ci.yml".to_string(),
                "on: push".to_string()
            )]
        );
    }
}
