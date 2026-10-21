//! The dispatcher: from a push or a pull request to runs, jobs and checks.
//!
//! An event names a commit. The dispatcher asks LosOS Git for the commit's
//! files, reads `.github/workflows/`, makes a run of every workflow the
//! event starts and plans the jobs whose `needs` are met. A job goes to the
//! edge's queue (`mesh`), where any box of the mesh that lends its runner
//! may take it, or stays on this box (`box`): when it says `self-hosted`,
//! when the repository is private and private repositories stay home, and
//! when there is no edge in reach. [`Ci::tick`] then follows the jobs,
//! copies their logs, plans what their outputs unblock and writes each
//! job's state to the commit as a status, the pull request's Checks box.

use super::db::{self, Job, NewRun, Run, Store};
use super::net::{Claimed, EdgeSpec, Forge, Mesh, Report};
use super::workflow::{self, Event, RunInfo, Workflow};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

/// The largest commit tree a job carries: the edge's own cap.
pub const MAX_ARCHIVE_BYTES: usize = 24 * 1024 * 1024;
/// The most of a job's log the box keeps.
pub const MAX_LOG_BYTES: u64 = 4 * 1024 * 1024;
/// Finished runs are kept this long.
pub const KEEP_RUNS_SECS: i64 = 30 * 24 * 3600;

/// When this box lends its runner to the rest of the mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lend {
    Always,
    /// Inside the compute window (`losos.cluster.computeWindow`).
    Window,
    Never,
}

impl Lend {
    pub fn parse(s: &str) -> Option<Lend> {
        match s {
            "always" => Some(Lend::Always),
            "window" => Some(Lend::Window),
            "never" => Some(Lend::Never),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Lend::Always => "always",
            Lend::Window => "window",
            Lend::Never => "never",
        }
    }
}

/// `HH:MM` to minutes after midnight.
fn minutes(hhmm: &str) -> Option<u32> {
    let (h, m) = hhmm.trim().split_once(':')?;
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// Whether `now` (minutes after local midnight) is inside `start`..`end`,
/// a window that may run over midnight.
pub fn in_window(start: &str, end: &str, now: u32) -> bool {
    match (minutes(start), minutes(end)) {
        (Some(s), Some(e)) if s <= e => s <= now && now < e,
        (Some(s), Some(e)) => now >= s || now < e,
        _ => false,
    }
}

/// The parsed event LosOS Git sends (`crate::forge::events`).
#[derive(Debug, Clone, PartialEq)]
pub struct Incoming {
    pub owner: String,
    pub repo: String,
    pub private: bool,
    pub event: Event,
}

/// Read one event body. `None` for anything Mesh CI does not run on.
pub fn parse_event(v: &Value) -> Option<Incoming> {
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let event = match v.get("kind")?.as_str()? {
        "push" => {
            let after = s("after");
            if !super::is_sha(&after) || after.bytes().all(|b| b == b'0') {
                return None;
            }
            Event::Push {
                r#ref: s("ref"),
                before: s("before"),
                after,
                pusher: s("pusher"),
            }
        }
        "pull_request" => {
            let head_sha = s("headSha");
            if !super::is_sha(&head_sha) {
                return None;
            }
            Event::PullRequest {
                action: s("action"),
                number: v.get("number")?.as_i64()?,
                title: s("title"),
                head_ref: s("headRef"),
                head_sha,
                base_ref: s("baseRef"),
                author: s("author"),
            }
        }
        _ => return None,
    };
    let (owner, repo) = (s("owner"), s("repo"));
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some(Incoming {
        owner,
        repo,
        private: v.get("private").and_then(Value::as_bool).unwrap_or(true),
        event,
    })
}

fn event_to_json(e: &Event) -> Value {
    match e {
        Event::Push {
            r#ref,
            before,
            after,
            pusher,
        } => {
            json!({ "kind": "push", "ref": r#ref, "before": before, "after": after, "pusher": pusher })
        }
        Event::PullRequest {
            action,
            number,
            title,
            head_ref,
            head_sha,
            base_ref,
            author,
        } => json!({
            "kind": "pull_request", "action": action, "number": number, "title": title,
            "headRef": head_ref, "headSha": head_sha, "baseRef": base_ref, "author": author,
        }),
        Event::Dispatch { r#ref, sha, actor } => {
            json!({ "kind": "workflow_dispatch", "ref": r#ref, "sha": sha, "actor": actor })
        }
    }
}

fn event_from_json(v: &Value) -> Option<Event> {
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    Some(match v.get("kind")?.as_str()? {
        "push" => Event::Push {
            r#ref: s("ref"),
            before: s("before"),
            after: s("after"),
            pusher: s("pusher"),
        },
        "pull_request" => Event::PullRequest {
            action: s("action"),
            number: v.get("number")?.as_i64()?,
            title: s("title"),
            head_ref: s("headRef"),
            head_sha: s("headSha"),
            base_ref: s("baseRef"),
            author: s("author"),
        },
        "workflow_dispatch" => Event::Dispatch {
            r#ref: s("ref"),
            sha: s("sha"),
            actor: s("actor"),
        },
        _ => return None,
    })
}

/// The words a new run is made of, borrowed while it is written.
struct Texts<'a> {
    file: &'a str,
    text: &'a str,
    name: &'a str,
    event_json: &'a str,
    git_ref: &'a str,
    title: &'a str,
}

impl<'a> Texts<'a> {
    fn run(self, inc: &'a Incoming) -> NewRun<'a> {
        NewRun {
            owner: &inc.owner,
            repo: &inc.repo,
            private: inc.private,
            workflow_file: self.file,
            workflow_name: self.name,
            workflow_text: self.text,
            event: inc.event.name(),
            event_json: self.event_json,
            sha: inc.event.sha(),
            git_ref: self.git_ref,
            title: self.title,
            actor: inc.event.actor(),
        }
    }
}

/// A job for this box's runner, from this box's own queue or the edge's.
#[derive(Debug, Clone)]
pub struct Work {
    /// `box`: this box's queue, `id` is the job's row. `mesh`: the edge's,
    /// `id` is the edge's job id.
    pub kind: &'static str,
    pub id: String,
    /// The box the job came from.
    pub origin: String,
    pub name: String,
    pub payload: Value,
    pub timeout_secs: u32,
    pub archive: Vec<u8>,
}

pub struct Ci {
    pub store: Store,
    pub mesh: Box<dyn Mesh>,
    pub forge: Box<dyn Forge>,
    /// Logs and archives live here.
    pub dir: PathBuf,
    /// This box's id on the edge.
    pub box_id: String,
    /// `https://<box>/git`, the workflows' `github.server_url`.
    pub server_url: String,
    /// Private repositories' jobs may go to the mesh.
    pub private_on_mesh: bool,
    /// The compute window, for [`Lend::Window`].
    pub window: Option<(String, String)>,
    /// Box jobs whose run was cancelled while they ran.
    cancel: Mutex<HashSet<i64>>,
    /// The status last written for each job, so a tick writes only changes.
    posted: Mutex<HashMap<i64, String>>,
    /// One tick at a time.
    ticking: Mutex<()>,
}

/// The state a job's status takes on the commit.
fn status_state(job_state: &str) -> &'static str {
    match job_state {
        "running" => "running",
        "success" | "skipped" => "success",
        "failure" => "failure",
        "cancelled" => "cancelled",
        "error" => "error",
        _ => "pending",
    }
}

fn duration(job: &Job) -> String {
    let (Some(a), Some(b)) = (job.started, job.finished) else {
        return String::new();
    };
    let secs = (b - a).max(0);
    if secs < 60 {
        format!("{secs}s")
    } else {
        format!("{}m {}s", secs / 60, secs % 60)
    }
}

impl Ci {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: Store,
        mesh: Box<dyn Mesh>,
        forge: Box<dyn Forge>,
        dir: PathBuf,
        box_id: String,
        server_url: String,
        private_on_mesh: bool,
        window: Option<(String, String)>,
    ) -> Ci {
        Ci {
            store,
            mesh,
            forge,
            dir,
            box_id,
            server_url,
            private_on_mesh,
            window,
            cancel: Mutex::new(HashSet::new()),
            posted: Mutex::new(HashMap::new()),
            ticking: Mutex::new(()),
        }
    }

    // ── settings ──────────────────────────────────────────────────────────

    pub fn lend(&self) -> Lend {
        self.store
            .setting("lend")
            .ok()
            .flatten()
            .and_then(|s| Lend::parse(&s))
            .unwrap_or(Lend::Window)
    }

    pub fn set_lend(&self, lend: Lend) -> anyhow::Result<()> {
        self.store.set_setting("lend", lend.as_str())
    }

    /// Whether the runner takes other boxes' jobs right now.
    pub fn lending_now(&self) -> bool {
        match self.lend() {
            Lend::Always => true,
            Lend::Never => false,
            Lend::Window => {
                use chrono::Timelike;
                let now = chrono::Local::now();
                self.window
                    .as_ref()
                    .is_some_and(|(s, e)| in_window(s, e, now.hour() * 60 + now.minute()))
            }
        }
    }

    // ── files ─────────────────────────────────────────────────────────────

    pub fn log_path(&self, job: i64) -> PathBuf {
        self.dir.join("logs").join(format!("{job}.log"))
    }

    fn archive_path(&self, run: &Run) -> PathBuf {
        self.dir.join("archives").join(format!("{}.tar.gz", run.id))
    }

    /// Add to a job's log, up to [`MAX_LOG_BYTES`].
    pub fn append_log(&self, job: i64, text: &str) {
        if text.is_empty() {
            return;
        }
        let path = self.log_path(job);
        let have = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        if have >= MAX_LOG_BYTES {
            return;
        }
        let _ = std::fs::create_dir_all(self.dir.join("logs"));
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let room = (MAX_LOG_BYTES - have) as usize;
            let bytes = text.as_bytes();
            let _ = f.write_all(&bytes[..bytes.len().min(room)]);
            if bytes.len() > room {
                let _ = f.write_all(b"\n[the log is cut here: it reached its limit]\n");
            }
        }
    }

    /// A job's log from byte `from`.
    pub fn read_log(&self, job: i64, from: u64) -> String {
        use std::io::{Read, Seek, SeekFrom};
        let Ok(mut f) = std::fs::File::open(self.log_path(job)) else {
            return String::new();
        };
        let _ = f.seek(SeekFrom::Start(from));
        let mut buf = Vec::new();
        let _ = f.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    }

    // ── events ────────────────────────────────────────────────────────────

    /// A push or a pull request: a run for every workflow it starts.
    pub fn on_event(&self, inc: &Incoming) -> anyhow::Result<Vec<i64>> {
        if let Event::PullRequest { action, .. } = &inc.event {
            if !matches!(action.as_str(), "opened" | "synchronize" | "reopened") {
                return Ok(Vec::new());
            }
        }
        let sha = inc.event.sha().to_string();
        let archive = self.forge.archive(&inc.owner, &inc.repo, &sha)?;
        self.start_runs(inc, &archive, None)
    }

    /// `workflow_dispatch`: the Run button on one workflow at one ref.
    pub fn dispatch(
        &self,
        owner: &str,
        repo: &str,
        file: &str,
        git_ref: &str,
        actor: &str,
    ) -> anyhow::Result<i64> {
        let info = self.forge.info(owner, repo, actor, git_ref)?;
        let sha = info
            .sha
            .clone()
            .ok_or_else(|| anyhow::anyhow!("{git_ref} names no commit"))?;
        let archive = self.forge.archive(owner, repo, &sha)?;
        let inc = Incoming {
            owner: owner.into(),
            repo: repo.into(),
            private: info.private,
            event: Event::Dispatch {
                r#ref: info.r#ref.clone(),
                sha,
                actor: actor.into(),
            },
        };
        let runs = self.start_runs(&inc, &archive, Some(file))?;
        runs.into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("{file} has no workflow_dispatch trigger"))
    }

    /// The workflows a commit's tree holds that the Run button may start.
    pub fn dispatchable(archive: &[u8]) -> Vec<(String, String)> {
        workflow::files_in_archive(archive)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(file, text)| workflow::parse(&file, &text).ok())
            .filter(Workflow::dispatchable)
            .map(|wf| (wf.file, wf.name))
            .collect()
    }

    fn start_runs(
        &self,
        inc: &Incoming,
        archive: &[u8],
        only: Option<&str>,
    ) -> anyhow::Result<Vec<i64>> {
        let files = workflow::files_in_archive(archive)?;
        let event_json = event_to_json(&inc.event).to_string();
        let git_ref = inc.event.git_ref();
        let title = inc.event.title();
        let mut runs = Vec::new();
        for (file, text) in files {
            if only.is_some_and(|f| f != file) {
                continue;
            }
            let new = |name| {
                Texts {
                    file: &file,
                    text: &text,
                    name,
                    event_json: &event_json,
                    git_ref: &git_ref,
                    title: &title,
                }
                .run(inc)
            };
            let wf = match workflow::parse(&file, &text) {
                Ok(wf) => wf,
                Err(why) => {
                    // A broken workflow is said once, on the commit, the way
                    // GitHub fails a run that never started.
                    let name = file.rsplit('/').next().unwrap_or(&file).to_string();
                    let run = self.store.create_run(&new(&name), "error", &why)?;
                    let _ = self.forge.status(
                        &inc.owner,
                        &inc.repo,
                        inc.event.sha(),
                        &json!({
                            "state": "error",
                            "context": format!("{name} ({})", inc.event.name()),
                            "description": why.chars().take(900).collect::<String>(),
                            "targetUrl": format!("/ci/runs/{run}"),
                        }),
                    );
                    runs.push(run);
                    continue;
                }
            };
            let wanted = match &inc.event {
                Event::Dispatch { .. } => wf.dispatchable(),
                e => wf.started_by(e),
            };
            if !wanted {
                continue;
            }
            let run = self.store.create_run(&new(&wf.name), "queued", "")?;
            let _ = std::fs::create_dir_all(self.dir.join("archives"));
            if let Some(r) = self.store.run(run)? {
                std::fs::write(self.archive_path(&r), archive)?;
            }
            runs.push(run);
        }
        for &run in &runs {
            if let Err(e) = self.advance(run) {
                tracing::warn!(target: "ci", "run {run}: {e:#}");
            }
        }
        Ok(runs)
    }

    /// A run again, from the same commit and the same event.
    pub fn rerun(&self, id: i64, actor: &str) -> anyhow::Result<i64> {
        let run = self
            .store
            .run(id)?
            .ok_or_else(|| anyhow::anyhow!("there is no run {id}"))?;
        let event: Value = serde_json::from_str(&run.event_json)?;
        let mut event = event_from_json(&event)
            .ok_or_else(|| anyhow::anyhow!("run {id}'s event cannot be read"))?;
        if let Event::Dispatch { actor: a, .. } = &mut event {
            *a = actor.to_string();
        }
        let inc = Incoming {
            owner: run.owner.clone(),
            repo: run.repo.clone(),
            private: run.private,
            event,
        };
        let archive = self.forge.archive(&run.owner, &run.repo, &run.sha)?;
        let only = Some(run.workflow_file.as_str());
        let runs = self.start_runs_forced(&inc, &archive, only)?;
        runs.into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("the workflow is gone from the commit"))
    }

    /// [`Self::start_runs`] for one file whatever its triggers say, as a
    /// re-run must.
    fn start_runs_forced(
        &self,
        inc: &Incoming,
        archive: &[u8],
        only: Option<&str>,
    ) -> anyhow::Result<Vec<i64>> {
        let files = workflow::files_in_archive(archive)?;
        let Some((file, text)) = files.into_iter().find(|(f, _)| Some(f.as_str()) == only) else {
            return Ok(Vec::new());
        };
        let wf = workflow::parse(&file, &text).map_err(|e| anyhow::anyhow!(e))?;
        let event_json = event_to_json(&inc.event).to_string();
        let git_ref = inc.event.git_ref();
        let title = inc.event.title();
        let run = self.store.create_run(
            &Texts {
                file: &file,
                text: &text,
                name: &wf.name,
                event_json: &event_json,
                git_ref: &git_ref,
                title: &title,
            }
            .run(inc),
            "queued",
            "",
        )?;
        let _ = std::fs::create_dir_all(self.dir.join("archives"));
        if let Some(r) = self.store.run(run)? {
            std::fs::write(self.archive_path(&r), archive)?;
        }
        self.advance(run)?;
        Ok(vec![run])
    }

    /// Stop a run: queued jobs end, running ones are asked to stop.
    pub fn cancel_run(&self, id: i64, by: &str) -> anyhow::Result<()> {
        let Some(run) = self.store.run(id)? else {
            anyhow::bail!("there is no run {id}");
        };
        if db::finished(&run.state) {
            return Ok(());
        }
        for job in self.store.jobs(id)? {
            if db::finished(&job.state) {
                continue;
            }
            match (job.placement.as_str(), job.state.as_str()) {
                ("box", "running") => {
                    self.cancel
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(job.id);
                }
                ("mesh", _) => {
                    if let Some(edge) = &job.edge_id {
                        if let Err(e) = self.mesh.cancel(edge) {
                            tracing::warn!(target: "ci", "cancelling {edge} on the edge: {e:#}");
                        }
                    }
                    self.store
                        .set_job_state(job.id, "cancelled", None, None, None)?;
                }
                _ => self
                    .store
                    .set_job_state(job.id, "cancelled", None, None, None)?,
            }
            self.append_log(job.id, &format!("\nCancelled by {by}.\n"));
        }
        self.store
            .set_run_state(id, "cancelled", &format!("Cancelled by {by}."))?;
        self.post_statuses(&run)?;
        Ok(())
    }

    // ── planning ──────────────────────────────────────────────────────────

    fn github(&self, run: &Run, wf: &Workflow, event: &Event) -> Value {
        workflow::github_context(&RunInfo {
            owner: &run.owner,
            repo: &run.repo,
            run_id: run.id,
            run_number: run.number,
            workflow: wf,
            event,
            server_url: &self.server_url,
        })
    }

    /// Where a planned job runs.
    fn placement(&self, run: &Run, self_hosted: bool) -> &'static str {
        if self_hosted || (run.private && !self.private_on_mesh) || !self.mesh.reachable() {
            "box"
        } else {
            "mesh"
        }
    }

    /// Plan what can be planned, hand mesh jobs to the edge, and close the
    /// run when every job is over.
    pub fn advance(&self, run_id: i64) -> anyhow::Result<()> {
        let Some(run) = self.store.run(run_id)? else {
            return Ok(());
        };
        if db::finished(&run.state) {
            return Ok(());
        }
        let wf = workflow::parse(&run.workflow_file, &run.workflow_text)
            .map_err(|e| anyhow::anyhow!(e))?;
        let event: Value = serde_json::from_str(&run.event_json)?;
        let event = event_from_json(&event).ok_or_else(|| anyhow::anyhow!("unreadable event"))?;
        let github = self.github(&run, &wf, &event);

        loop {
            let jobs = self.store.jobs(run_id)?;
            let mut planned_any = false;
            for key in &wf.jobs {
                if jobs.iter().any(|j| &j.job_key == key) {
                    continue;
                }
                let needs = workflow::needs_of(wf.job(key));
                let ready = needs.iter().all(|n| {
                    let rows: Vec<&Job> = jobs.iter().filter(|j| &j.job_key == n).collect();
                    !rows.is_empty() && rows.iter().all(|j| db::finished(&j.state))
                });
                if !ready {
                    continue;
                }
                let mut ctx = Map::new();
                let mut needs_failed = false;
                for n in &needs {
                    let rows: Vec<&Job> = jobs.iter().filter(|j| &j.job_key == n).collect();
                    let result = need_result(&rows);
                    if result != "success" {
                        needs_failed = true;
                    }
                    let mut outputs = Map::new();
                    for r in &rows {
                        for (k, v) in &r.outputs {
                            outputs.insert(k.clone(), v.clone());
                        }
                    }
                    ctx.insert(n.clone(), json!({ "result": result, "outputs": outputs }));
                }
                planned_any = true;
                match workflow::plan_job(&wf, key, &github, &ctx, needs_failed, false) {
                    Ok(None) => {
                        self.store.add_job(
                            run_id,
                            key,
                            key,
                            &Map::new(),
                            "skipped",
                            "",
                            &Value::Null,
                            false,
                        )?;
                    }
                    Err(why) => {
                        let job = self.store.add_job(
                            run_id,
                            key,
                            key,
                            &Map::new(),
                            "error",
                            "",
                            &Value::Null,
                            false,
                        )?;
                        self.append_log(job, &format!("{why}\n"));
                    }
                    Ok(Some(list)) => {
                        for p in list {
                            let placement = self.placement(&run, p.self_hosted);
                            let job = self.store.add_job(
                                run_id,
                                key,
                                &p.name,
                                &p.matrix,
                                "queued",
                                placement,
                                &p.payload,
                                p.continue_on_error,
                            )?;
                            if placement == "mesh" {
                                self.submit(&run, job, &p.name, &p.payload, p.timeout_minutes);
                            } else if p.self_hosted {
                                self.append_log(
                                    job,
                                    "Waiting for this box's runner (runs-on: self-hosted).\n",
                                );
                            } else {
                                self.append_log(job, "Waiting for this box's runner.\n");
                            }
                        }
                    }
                }
            }
            if !planned_any {
                break;
            }
        }

        let jobs = self.store.jobs(run_id)?;
        self.fail_fast(&wf, &jobs)?;
        let jobs = self.store.jobs(run_id)?;
        let all_planned = wf.jobs.iter().all(|k| jobs.iter().any(|j| &j.job_key == k));
        let all_done = jobs.iter().all(|j| db::finished(&j.state));
        if all_planned && all_done {
            let failed = jobs
                .iter()
                .any(|j| !j.continue_on_error && matches!(j.state.as_str(), "failure" | "error"));
            let cancelled = jobs.iter().any(|j| j.state == "cancelled");
            let state = if failed {
                "failure"
            } else if cancelled {
                "cancelled"
            } else {
                "success"
            };
            self.store.set_run_state(run_id, state, "")?;
            let _ = std::fs::remove_file(self.archive_path(&run));
        } else if !all_planned && all_done && !jobs.is_empty() {
            // Nothing runs and nothing more can be planned: a cycle the
            // parser would have refused, or a need that names nothing.
            self.store
                .set_run_state(run_id, "error", "Some jobs could not be planned.")?;
        } else if jobs.iter().any(|j| j.state == "running") && run.state == "queued" {
            self.store.set_run_state(run_id, "running", "")?;
        }
        self.post_statuses(&run)?;
        Ok(())
    }

    /// GitHub's `fail-fast` (on unless the strategy says otherwise): a
    /// failed combination of a matrix cancels the ones still to finish.
    fn fail_fast(&self, wf: &Workflow, jobs: &[Job]) -> anyhow::Result<()> {
        for key in &wf.jobs {
            let rows: Vec<&Job> = jobs.iter().filter(|j| &j.job_key == key).collect();
            if rows.len() < 2 {
                continue;
            }
            let fail_fast = rows[0]
                .payload
                .pointer("/contexts/strategy/fail-fast")
                .map(super::expr::truthy)
                .unwrap_or(true);
            let tripped = rows
                .iter()
                .any(|j| !j.continue_on_error && matches!(j.state.as_str(), "failure" | "error"));
            if !(fail_fast && tripped) {
                continue;
            }
            for j in rows.iter().filter(|j| !db::finished(&j.state)) {
                match (j.placement.as_str(), j.state.as_str(), &j.edge_id) {
                    ("box", "running", _) => {
                        self.cancel
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .insert(j.id);
                    }
                    ("mesh", _, Some(edge)) => {
                        let _ = self.mesh.cancel(edge);
                        self.store
                            .set_job_state(j.id, "cancelled", None, None, None)?;
                    }
                    _ => self
                        .store
                        .set_job_state(j.id, "cancelled", None, None, None)?,
                }
                self.append_log(
                    j.id,
                    "\nCancelled: another job of the matrix failed (fail-fast).\n",
                );
            }
        }
        Ok(())
    }

    /// Hand a job to the edge. When the edge does not take it, it runs on
    /// this box instead.
    fn submit(&self, run: &Run, job: i64, name: &str, payload: &Value, timeout_minutes: u32) {
        let archive = match std::fs::read(self.archive_path(run)) {
            Ok(a) => a,
            Err(e) => {
                let _ = self.store.set_job_state(job, "error", None, None, None);
                self.append_log(job, &format!("The commit's files are gone: {e}\n"));
                return;
            }
        };
        let spec = EdgeSpec {
            name: format!("{} / {name}", run.workflow_name)
                .chars()
                .take(200)
                .collect(),
            payload: payload.clone(),
            timeout_secs: timeout_minutes.saturating_mul(60).clamp(60, 6 * 3600),
            repo: format!("{}/{}", run.owner, run.repo),
            commit: run.sha.clone(),
        };
        match self.mesh.submit(&spec, &archive) {
            Ok(edge_id) => {
                let _ = self.store.set_job_edge_id(job, &edge_id);
                self.append_log(job, "Queued on the mesh; waiting for a box to take it.\n");
            }
            Err(e) => {
                let _ = self.store.set_placement(job, "box");
                self.append_log(
                    job,
                    &format!("The mesh did not take the job ({e:#}); it runs on this box.\n"),
                );
            }
        }
    }

    // ── following the jobs ────────────────────────────────────────────────

    /// One pass: the edge's word on every mesh job, then every active run
    /// advanced. Called every few seconds by the daemon.
    pub fn tick(&self) -> anyhow::Result<()> {
        let _one = self.ticking.lock().unwrap_or_else(|e| e.into_inner());
        let open = self.store.open_mesh_jobs()?;
        if !open.is_empty() && self.mesh.reachable() {
            let wanted: Vec<(String, i64)> = open
                .iter()
                .filter_map(|j| j.edge_id.clone().map(|e| (e, j.log_len)))
                .collect();
            match self.mesh.jobs(&wanted) {
                Ok(views) => {
                    for v in views {
                        if let Some(job) = open.iter().find(|j| j.edge_id.as_deref() == Some(&v.id))
                        {
                            self.apply_edge_view(job, &v)?;
                        }
                    }
                }
                Err(e) => tracing::warn!(target: "ci", "asking the edge about jobs: {e:#}"),
            }
        }
        for run in self.store.active_runs()? {
            if let Err(e) = self.advance(run.id) {
                tracing::warn!(target: "ci", "run {}: {e:#}", run.id);
                self.store
                    .set_run_state(run.id, "error", &format!("{e:#}"))?;
            }
        }
        Ok(())
    }

    fn apply_edge_view(&self, job: &Job, v: &super::net::EdgeJob) -> anyhow::Result<()> {
        if !v.log.is_empty() {
            self.append_log(job.id, &v.log);
        }
        if v.log_len > job.log_len {
            self.store.set_log_len(job.id, v.log_len)?;
        }
        let state = match v.state.as_str() {
            "lost" => {
                self.append_log(job.id, "\nThe edge no longer knows this job.\n");
                "error"
            }
            s @ ("queued" | "running" | "success" | "failure" | "error" | "cancelled") => s,
            _ => return Ok(()),
        };
        if state != job.state || v.runner.as_deref().is_some_and(|r| r != job.runner) {
            self.store.set_job_state(
                job.id,
                state,
                v.runner.as_deref(),
                v.exit_code,
                v.outputs.as_ref(),
            )?;
        }
        Ok(())
    }

    /// Write each job's state to the commit, where it changed.
    fn post_statuses(&self, run: &Run) -> anyhow::Result<()> {
        let event = run.event.as_str();
        for job in self.store.jobs(run.id)? {
            let state = status_state(&job.state);
            let description = match job.state.as_str() {
                "queued" if job.placement == "mesh" => "Waiting for a box of the mesh".to_string(),
                "queued" => "Waiting for this box's runner".to_string(),
                "running" if !job.runner.is_empty() => format!("Running on {}", job.runner),
                "running" => "Running".to_string(),
                "success" => format!("Passed in {} on {}", duration(&job), job.runner),
                "failure" => format!("Failed after {} on {}", duration(&job), job.runner),
                "skipped" => "Skipped".to_string(),
                "cancelled" => "Cancelled".to_string(),
                _ => "Could not run".to_string(),
            };
            let key = format!("{state}|{description}");
            {
                let mut posted = self.posted.lock().unwrap_or_else(|e| e.into_inner());
                if posted.get(&job.id) == Some(&key) {
                    continue;
                }
                posted.insert(job.id, key);
            }
            let body = json!({
                "state": state,
                "context": format!("{} / {} ({event})", run.workflow_name, job.name),
                "description": description,
                "targetUrl": format!("/ci/runs/{}#job-{}", run.id, job.id),
            });
            if let Err(e) = self.forge.status(&run.owner, &run.repo, &run.sha, &body) {
                tracing::warn!(target: "ci", "status for job {}: {e:#}", job.id);
                self.posted
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&job.id);
            }
        }
        Ok(())
    }

    // ── this box's runner ─────────────────────────────────────────────────

    /// Work for the runner: this box's queue first, then the edge's, for up
    /// to `wait_secs`.
    pub fn claim(&self, wait_secs: u32) -> anyhow::Result<Option<Work>> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(wait_secs.into());
        loop {
            if let Some(job) = self.store.take_box_job(&self.box_id)? {
                let run = self
                    .store
                    .run(job.run_id)?
                    .ok_or_else(|| anyhow::anyhow!("job {} has no run", job.id))?;
                let archive = std::fs::read(self.archive_path(&run)).unwrap_or_default();
                self.append_log(job.id, &format!("Taken by this box ({}).\n", self.box_id));
                let _ = self.post_statuses(&run);
                let timeout = job
                    .payload
                    .get("timeoutMinutes")
                    .and_then(Value::as_u64)
                    .unwrap_or(360) as u32;
                return Ok(Some(Work {
                    kind: "box",
                    id: job.id.to_string(),
                    origin: self.box_id.clone(),
                    name: format!("{} / {}", run.workflow_name, job.name),
                    payload: job.payload,
                    timeout_secs: timeout * 60,
                    archive,
                }));
            }
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if self.mesh.reachable() {
                let wait = left.as_secs().min(5) as u32;
                match self.mesh.claim(self.lending_now(), wait) {
                    Ok(Some(Claimed {
                        id,
                        origin,
                        job,
                        archive,
                    })) => {
                        return Ok(Some(Work {
                            kind: "mesh",
                            id,
                            origin,
                            name: job.name,
                            payload: job.payload,
                            timeout_secs: job.timeout_secs,
                            archive,
                        }))
                    }
                    Ok(None) => {}
                    Err(e) => {
                        tracing::warn!(target: "ci", "claiming from the edge: {e:#}");
                        std::thread::sleep(std::time::Duration::from_secs(1).min(left));
                    }
                }
            } else {
                std::thread::sleep(std::time::Duration::from_secs(1).min(left));
            }
            if std::time::Instant::now() >= deadline {
                return Ok(None);
            }
        }
    }

    /// The runner's report on a job it took. Returns whether to stop it.
    pub fn report(&self, kind: &str, id: &str, report: &Report) -> anyhow::Result<bool> {
        match kind {
            "mesh" => self.mesh.update(id, report),
            "box" => {
                let id: i64 = id.parse()?;
                let Some(job) = self.store.job(id)? else {
                    return Ok(true);
                };
                self.append_log(id, &report.log);
                if let Some(state) = &report.state {
                    let state = match state.as_str() {
                        s @ ("success" | "failure" | "cancelled" | "error") => s,
                        _ => "error",
                    };
                    self.store.set_job_state(
                        id,
                        state,
                        None,
                        report.exit_code,
                        report.outputs.as_ref(),
                    )?;
                    self.cancel
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .remove(&id);
                    if let Err(e) = self.advance(job.run_id) {
                        tracing::warn!(target: "ci", "run {}: {e:#}", job.run_id);
                    }
                    return Ok(false);
                }
                Ok(job.state == "cancelled"
                    || self
                        .cancel
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .contains(&id))
            }
            _ => anyhow::bail!("unknown job kind {kind}"),
        }
    }

    /// After a restart: a box job the runner had is lost with the runner,
    /// so it ends as an error rather than reading `running` forever.
    pub fn recover(&self) -> anyhow::Result<()> {
        for run in self.store.active_runs()? {
            for job in self.store.jobs(run.id)? {
                if job.placement == "box" && job.state == "running" {
                    self.append_log(job.id, "\nThis box restarted while the job ran.\n");
                    self.store
                        .set_job_state(job.id, "error", None, None, None)?;
                }
            }
        }
        Ok(())
    }

    /// Housekeeping: old runs and their logs go.
    pub fn sweep(&self) -> anyhow::Result<()> {
        for job in self.store.forget_before(db::now() - KEEP_RUNS_SECS)? {
            let _ = std::fs::remove_file(self.log_path(job));
        }
        Ok(())
    }
}

/// `needs.<id>.result` over the rows of one job (one per combination).
fn need_result(rows: &[&Job]) -> &'static str {
    if rows
        .iter()
        .any(|j| !j.continue_on_error && matches!(j.state.as_str(), "failure" | "error"))
    {
        "failure"
    } else if rows.iter().any(|j| j.state == "cancelled") {
        "cancelled"
    } else if rows.iter().all(|j| j.state == "skipped") {
        "skipped"
    } else {
        "success"
    }
}

#[cfg(test)]
mod tests {
    use super::super::net::{EdgeJob, RepoInfo};
    use super::*;
    use std::sync::Arc;

    /// id, spec, state, outputs.
    type EdgeRow = (String, EdgeSpec, String, Option<Map<String, Value>>);

    /// An edge that keeps jobs in memory, and lets the test play the box
    /// that runs them.
    #[derive(Default)]
    struct FakeMesh {
        up: bool,
        jobs: Mutex<Vec<EdgeRow>>,
    }

    impl Mesh for Arc<FakeMesh> {
        fn reachable(&self) -> bool {
            self.up
        }
        fn submit(&self, spec: &EdgeSpec, _archive: &[u8]) -> anyhow::Result<String> {
            let mut jobs = self.jobs.lock().unwrap();
            let id = format!("e{}", jobs.len() + 1);
            jobs.push((id.clone(), spec.clone(), "queued".into(), None));
            Ok(id)
        }
        fn jobs(&self, wanted: &[(String, i64)]) -> anyhow::Result<Vec<EdgeJob>> {
            let jobs = self.jobs.lock().unwrap();
            Ok(wanted
                .iter()
                .filter_map(|(id, _)| jobs.iter().find(|j| &j.0 == id))
                .map(|(id, _, state, outputs)| EdgeJob {
                    id: id.clone(),
                    state: state.clone(),
                    runner: Some("box-2".into()),
                    exit_code: Some(if state == "failure" { 1 } else { 0 }),
                    outputs: outputs.clone(),
                    log: format!("[{state}]\n"),
                    log_len: 1,
                })
                .collect())
        }
        fn cancel(&self, id: &str) -> anyhow::Result<()> {
            for j in self.jobs.lock().unwrap().iter_mut() {
                if j.0 == id {
                    j.2 = "cancelled".into();
                }
            }
            Ok(())
        }
        fn claim(&self, _foreign: bool, _wait: u32) -> anyhow::Result<Option<Claimed>> {
            Ok(None)
        }
        fn update(&self, _id: &str, _report: &Report) -> anyhow::Result<bool> {
            Ok(false)
        }
    }

    impl FakeMesh {
        fn finish(&self, name_part: &str, state: &str, outputs: Option<Map<String, Value>>) {
            for j in self.jobs.lock().unwrap().iter_mut() {
                if j.1.name.contains(name_part) {
                    j.2 = state.into();
                    j.3 = outputs.clone();
                }
            }
        }
        fn names(&self) -> Vec<String> {
            self.jobs
                .lock()
                .unwrap()
                .iter()
                .map(|j| j.1.name.clone())
                .collect()
        }
    }

    struct FakeForge {
        archive: Vec<u8>,
        statuses: Mutex<Vec<Value>>,
    }

    impl Forge for Arc<FakeForge> {
        fn info(&self, _o: &str, _r: &str, _u: &str, git_ref: &str) -> anyhow::Result<RepoInfo> {
            Ok(RepoInfo {
                private: false,
                default_branch: "main".into(),
                permission: "admin".into(),
                r#ref: if git_ref.is_empty() {
                    "refs/heads/main".into()
                } else {
                    git_ref.into()
                },
                sha: Some("c".repeat(40)),
            })
        }
        fn archive(&self, _o: &str, _r: &str, _s: &str) -> anyhow::Result<Vec<u8>> {
            Ok(self.archive.clone())
        }
        fn status(&self, _o: &str, _r: &str, _s: &str, status: &Value) -> anyhow::Result<()> {
            self.statuses.lock().unwrap().push(status.clone());
            Ok(())
        }
    }

    fn archive(files: &[(&str, &str)]) -> Vec<u8> {
        let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::fast(),
        ));
        for (path, text) in files {
            let mut h = tar::Header::new_gnu();
            h.set_size(text.len() as u64);
            h.set_mode(0o644);
            h.set_cksum();
            tar.append_data(&mut h, path, text.as_bytes()).unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap()
    }

    const WF: &str = "
name: CI
on: [push, pull_request, workflow_dispatch]
jobs:
  build:
    runs-on: ubuntu-latest
    outputs:
      version: ${{ steps.v.outputs.version }}
    steps:
      - id: v
        run: echo version=1.2 >> $GITHUB_OUTPUT
  test:
    needs: build
    runs-on: ubuntu-latest
    strategy:
      matrix:
        part: [1, 2]
    steps:
      - run: echo ${{ needs.build.outputs.version }} ${{ matrix.part }}
  local:
    runs-on: [self-hosted]
    steps:
      - run: echo here
";

    fn setup(up: bool) -> (Ci, Arc<FakeMesh>, Arc<FakeForge>, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "losos-ci-test-{}-{}",
            std::process::id(),
            rand_suffix()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let mesh = Arc::new(FakeMesh {
            up,
            ..FakeMesh::default()
        });
        let forge = Arc::new(FakeForge {
            archive: archive(&[(".github/workflows/ci.yml", WF), ("README", "hi")]),
            statuses: Mutex::new(Vec::new()),
        });
        let ci = Ci::new(
            Store::in_memory().unwrap(),
            Box::new(mesh.clone()),
            Box::new(forge.clone()),
            dir.clone(),
            "box-1".into(),
            "https://box-1.local/git".into(),
            false,
            Some(("23:00".into(), "07:00".into())),
        );
        (ci, mesh, forge, dir)
    }

    fn rand_suffix() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        N.fetch_add(1, Ordering::Relaxed)
    }

    fn push() -> Incoming {
        Incoming {
            owner: "matus".into(),
            repo: "site".into(),
            private: false,
            event: Event::Push {
                r#ref: "refs/heads/main".into(),
                before: "0".repeat(40),
                after: "a".repeat(40),
                pusher: "matus".into(),
            },
        }
    }

    #[test]
    fn a_push_runs_on_the_mesh_and_needs_carry_outputs() {
        let (ci, mesh, forge, dir) = setup(true);
        let runs = ci.on_event(&push()).unwrap();
        assert_eq!(runs.len(), 1);
        let run = runs[0];
        // build goes to the mesh, local stays on this box, test waits.
        assert_eq!(mesh.names(), ["CI / build"]);
        let jobs = ci.store.jobs(run).unwrap();
        assert_eq!(jobs.len(), 2);
        let local = jobs.iter().find(|j| j.job_key == "local").unwrap();
        assert_eq!(local.placement, "box");

        let mut outs = Map::new();
        outs.insert("version".into(), json!("1.2"));
        mesh.finish("build", "success", Some(outs));
        ci.tick().unwrap();
        // The matrix is planned with the build's output.
        assert_eq!(mesh.names().len(), 3);
        let test = ci
            .store
            .jobs(run)
            .unwrap()
            .into_iter()
            .find(|j| j.name == "test (1)")
            .unwrap();
        assert_eq!(
            test.payload
                .pointer("/contexts/needs/build/outputs/version"),
            Some(&json!("1.2"))
        );

        // This box's runner takes the self-hosted job and passes it.
        let work = ci.claim(0).unwrap().unwrap();
        assert_eq!(work.kind, "box");
        assert!(!work.archive.is_empty());
        let stop = ci
            .report(
                "box",
                &work.id,
                &Report {
                    log: "here\n".into(),
                    state: Some("success".into()),
                    exit_code: Some(0),
                    outputs: None,
                },
            )
            .unwrap();
        assert!(!stop);
        mesh.finish("test", "success", None);
        ci.tick().unwrap();
        let run = ci.store.run(run).unwrap().unwrap();
        assert_eq!(run.state, "success");
        let statuses = forge.statuses.lock().unwrap();
        assert!(statuses
            .iter()
            .any(|s| s["context"] == "CI / test (2) (push)" && s["state"] == "success"));
        assert!(statuses
            .iter()
            .any(|s| s["context"] == "CI / build (push)" && s["state"] == "pending"));
        assert!(ci.read_log(local.id, 0).contains("here"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn fail_fast_cancels_the_rest_of_the_matrix() {
        let (ci, mesh, _forge, dir) = setup(true);
        let run = ci.on_event(&push()).unwrap()[0];
        mesh.finish("build", "success", None);
        ci.tick().unwrap();
        mesh.finish("test (1)", "failure", None);
        ci.tick().unwrap();
        let jobs = ci.store.jobs(run).unwrap();
        let two = jobs.iter().find(|j| j.name == "test (2)").unwrap();
        assert_eq!(two.state, "cancelled");
        let work = ci.claim(0).unwrap().unwrap();
        ci.report(
            "box",
            &work.id,
            &Report {
                state: Some("success".into()),
                ..Report::default()
            },
        )
        .unwrap();
        assert_eq!(ci.store.run(run).unwrap().unwrap().state, "failure");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn without_an_edge_everything_stays_home() {
        let (ci, mesh, _forge, dir) = setup(false);
        let run = ci.on_event(&push()).unwrap()[0];
        assert!(mesh.names().is_empty());
        assert!(ci
            .store
            .jobs(run)
            .unwrap()
            .iter()
            .all(|j| j.placement == "box"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn cancelling_stops_running_box_jobs() {
        let (ci, _mesh, _forge, dir) = setup(false);
        let run = ci.on_event(&push()).unwrap()[0];
        let work = ci.claim(0).unwrap().unwrap();
        ci.cancel_run(run, "matus").unwrap();
        assert!(ci.report("box", &work.id, &Report::default()).unwrap());
        assert_eq!(ci.store.run(run).unwrap().unwrap().state, "cancelled");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_broken_workflow_is_said_on_the_commit() {
        let (mut ci, _mesh, _forge, dir) = setup(false);
        let forge = Arc::new(FakeForge {
            archive: archive(&[(".github/workflows/bad.yml", "on: push\njobs: {}\n")]),
            statuses: Mutex::new(Vec::new()),
        });
        ci.forge = Box::new(forge.clone());
        let run = ci.on_event(&push()).unwrap()[0];
        let run = ci.store.run(run).unwrap().unwrap();
        assert_eq!(run.state, "error");
        assert_eq!(forge.statuses.lock().unwrap()[0]["state"], "error");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn events_and_windows() {
        let ev = json!({
            "kind": "pull_request", "action": "opened", "owner": "matus", "repo": "site",
            "private": true, "number": 3, "title": "T", "headRef": "refs/heads/x",
            "headSha": "b".repeat(40), "baseRef": "refs/heads/main", "author": "eva",
        });
        let inc = parse_event(&ev).unwrap();
        assert!(inc.private);
        assert_eq!(inc.event.git_ref(), "refs/pull/3/head");
        assert_eq!(event_from_json(&event_to_json(&inc.event)), Some(inc.event));
        let deleted = json!({ "kind": "push", "owner": "a", "repo": "b", "after": "0".repeat(40) });
        assert!(parse_event(&deleted).is_none());
        assert!(in_window("23:00", "07:00", 23 * 60 + 30));
        assert!(in_window("23:00", "07:00", 60));
        assert!(!in_window("23:00", "07:00", 12 * 60));
        assert!(in_window("09:00", "17:00", 9 * 60));
        assert!(!in_window("09:00", "17:00", 17 * 60));
    }
}
