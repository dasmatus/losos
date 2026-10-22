//! Mesh CI's queue: the jobs boxes in this edge's mesh hand it, and the
//! boxes that lend CI time take them (`/ci/*`, `server::ci`).
//!
//! A box runs LosOS Git and `losos-ci` (backend/src/ci/). When someone
//! pushes, its `losos-ci` reads the repository's `.github/workflows/`,
//! expands them into jobs (a matrix is one job per combination), and sends each one here with a gzipped tar of the commit. Any
//! box of the mesh that lends CI time asks for work (`claim`, a long poll),
//! runs the job as its `shared` user in rootless Podman, and streams the log
//! back (`update`). The box that sent the job reads the log and the result
//! (`jobs`) and posts them to LosOS Git as checks on the commit. A job is
//! one job of a GitHub Actions workflow, planned by the box that sent it.
//!
//! The edge is the meeting point because it is the one machine every box of
//! the mesh already reaches: boxes sit behind home routers and dial out, and
//! none of them can be dialled. Nothing runs here: the edge keeps the queue,
//! the archives and the logs, in memory and in one directory, and forgets
//! them a few hours after a job ends. A restart forgets everything; the
//! boxes see their jobs as `lost` and say so on the commit.
//!
//! Who may: every route takes the appliance token, and the tenant must have
//! the operator's `cluster` bit (`losos.edge.tenants.<id>.cluster`), the
//! same bit that lets a box join the mesh. A box reads and cancels only the
//! jobs it sent, and reports only on the jobs it took.
//!
//! Who gets a job: the oldest job a box may take. A box takes other boxes'
//! jobs only when it says it lends now (`foreign`), and takes its own jobs
//! only after [`ORIGIN_GRACE`], so while another box is waiting for work
//! the job goes there and the box that pushed stays free for its owner.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, Notify, Semaphore};
use tokio::time::Instant;

/// The largest gzipped tar of one commit, in bytes.
pub const MAX_ARCHIVE_BYTES: usize = 24 * 1024 * 1024;
/// Jobs one box may have open (queued or running) at once.
pub const MAX_OPEN_PER_BOX: usize = 64;
/// Jobs the edge keeps at once, finished ones included.
pub const MAX_JOBS: usize = 4096;
/// The log kept per job; past it the rest is dropped and the log says so.
pub const MAX_LOG_BYTES: usize = 1024 * 1024;
/// The longest a box may go without a word about a job it runs before the
/// job goes back in the queue.
pub const LEASE: Duration = Duration::from_secs(90);
/// How long a job waits for another box before the box that sent it may
/// take it itself.
pub const ORIGIN_GRACE: Duration = Duration::from_secs(10);
/// How many boxes a job is handed to before it fails for good.
pub const MAX_ATTEMPTS: u32 = 2;
/// How long a finished job's log stays.
pub const KEEP_FINISHED: Duration = Duration::from_secs(6 * 3600);
/// How long a job may wait for a box at all.
pub const KEEP_QUEUED: Duration = Duration::from_secs(24 * 3600);
/// The longest one `claim` waits for work.
pub const MAX_WAIT: Duration = Duration::from_secs(25);
/// The longest a job may run.
pub const MAX_TIMEOUT_SECS: u32 = 6 * 3600;
/// Requests on `/ci/*` answered at once. The routes run outside the API's
/// guard (a claim waits, a submit carries an archive), so this stands in
/// for its cap.
pub const MAX_INFLIGHT: usize = 128;

/// The largest job description, serialised.
pub const MAX_PAYLOAD_BYTES: usize = 256 * 1024;

/// What one job is. The edge reads only the name, the timeout and where it
/// came from; `payload` is the job as the sending box planned it from the
/// repository's GitHub Actions workflow (backend/src/ci/), for the runner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSpec {
    /// The check's name on the commit, `CI / test (ubuntu, 20)`.
    pub name: String,
    pub payload: serde_json::Value,
    pub timeout_secs: u32,
    /// `owner/name`, for the runner's log and page.
    #[serde(default)]
    pub repo: String,
    /// The commit, 40 hex characters.
    #[serde(default)]
    pub commit: String,
}

impl JobSpec {
    /// Why this job is refused, or `None`.
    pub fn fault(&self) -> Option<&'static str> {
        let payload = serde_json::to_vec(&self.payload)
            .map(|v| v.len())
            .unwrap_or(usize::MAX);
        if self.name.trim().is_empty() || self.name.len() > 200 || self.name.contains('\n') {
            Some("a job's name is one line of at most 200 characters")
        } else if !self.payload.is_object() || payload > MAX_PAYLOAD_BYTES {
            Some("a job is a JSON object of at most 256 KiB")
        } else if self.timeout_secs == 0 || self.timeout_secs > MAX_TIMEOUT_SECS {
            Some("the timeout is between 1 second and 6 hours")
        } else if self.repo.len() > 200
            || !(self.commit.is_empty()
                || (self.commit.len() == 40 && self.commit.chars().all(|c| c.is_ascii_hexdigit())))
        {
            Some("the commit is 40 hex characters")
        } else {
            None
        }
    }
}

/// Where a job is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    Queued,
    Running,
    Success,
    Failure,
    Error,
    Cancelled,
}

impl JobState {
    pub fn finished(self) -> bool {
        !matches!(self, JobState::Queued | JobState::Running)
    }
}

/// Why a request was refused.
#[derive(Debug, thiserror::Error)]
pub enum CiError {
    #[error("this edge does not run CI for its mesh")]
    Unconfigured,
    #[error("CI is for boxes in this edge's mesh")]
    Forbidden,
    #[error("{0}")]
    Invalid(&'static str),
    #[error("no such job")]
    NotFound,
    #[error("{0}")]
    Full(&'static str),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

struct Job {
    origin: String,
    spec: JobSpec,
    state: JobState,
    runner: Option<String>,
    attempts: u32,
    created: Instant,
    touched: Instant,
    created_at: u64,
    started_at: Option<u64>,
    finished_at: Option<u64>,
    exit_code: Option<i32>,
    outputs: Option<serde_json::Map<String, serde_json::Value>>,
    log: String,
    cut: bool,
    cancel: bool,
}

/// A job handed to a runner.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Claimed {
    pub id: String,
    pub origin: String,
    pub job: JobSpec,
    /// The gzipped tar of the commit.
    #[serde(skip)]
    pub archive: Vec<u8>,
}

/// One job as the box that sent it sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    pub id: String,
    /// `lost` for a job this edge does not know (any more).
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// When the edge took the job, Unix seconds; 0 for a lost one.
    pub created_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<u64>,
    /// The job's outputs (`jobs.<id>.outputs`), once it is over.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<serde_json::Map<String, serde_json::Value>>,
    /// The log from the offset asked for.
    pub log: String,
    /// Where the log ends now: the offset to ask from next time.
    pub log_len: usize,
}

/// What a runner reports about a job it took.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// More of the log.
    #[serde(default)]
    pub log: String,
    /// The end, once the job is over: success, failure, error or
    /// cancelled.
    #[serde(default)]
    pub state: Option<JobState>,
    #[serde(default)]
    pub exit_code: Option<i32>,
    /// The job's outputs, with the end.
    #[serde(default)]
    pub outputs: Option<serde_json::Map<String, serde_json::Value>>,
}

/// The largest set of outputs one job may hand on, serialised.
pub const MAX_OUTPUTS_BYTES: usize = 64 * 1024;

/// The queue.
pub struct Ci {
    dir: PathBuf,
    jobs: Mutex<HashMap<String, Job>>,
    wake: Notify,
    counter: std::sync::atomic::AtomicU64,
    /// [`MAX_INFLIGHT`].
    pub inflight: Semaphore,
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Append `more` to `log`, keeping it under [`MAX_LOG_BYTES`].
fn append(log: &mut String, cut: &mut bool, more: &str) {
    if *cut || more.is_empty() {
        return;
    }
    let room = MAX_LOG_BYTES.saturating_sub(log.len());
    if more.len() <= room {
        log.push_str(more);
        return;
    }
    let mut end = room;
    while !more.is_char_boundary(end) {
        end -= 1;
    }
    log.push_str(&more[..end]);
    log.push_str("\n[The log is cut here: it is longer than 1 MiB.]\n");
    *cut = true;
}

impl Ci {
    /// Open the queue's directory. Archives left from before a restart
    /// belong to jobs nobody remembers, so they go.
    pub async fn open(dir: &Path) -> std::io::Result<Ci> {
        tokio::fs::create_dir_all(dir).await?;
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).await?;
        }
        let mut entries = tokio::fs::read_dir(dir).await?;
        while let Some(e) = entries.next_entry().await? {
            if e.file_name().to_string_lossy().ends_with(".tar.gz") {
                let _ = tokio::fs::remove_file(e.path()).await;
            }
        }
        Ok(Ci {
            dir: dir.to_path_buf(),
            jobs: Mutex::new(HashMap::new()),
            wake: Notify::new(),
            counter: std::sync::atomic::AtomicU64::new(0),
            inflight: Semaphore::new(MAX_INFLIGHT),
        })
    }

    fn archive_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.tar.gz"))
    }

    fn new_id(&self) -> String {
        let n = self
            .counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let salt = crate::identity::random_hex(4).unwrap_or_default();
        format!("{:x}{n:04x}{salt}", unix_now())
    }

    /// Take a job from `origin`. Returns its id.
    pub async fn submit(
        &self,
        origin: &str,
        spec: JobSpec,
        archive: Vec<u8>,
    ) -> Result<String, CiError> {
        if let Some(why) = spec.fault() {
            return Err(CiError::Invalid(why));
        }
        if archive.len() > MAX_ARCHIVE_BYTES {
            return Err(CiError::Invalid(
                "the commit's archive is larger than 24 MiB",
            ));
        }
        let id = self.new_id();
        {
            let mut jobs = self.jobs.lock().await;
            self.sweep(&mut jobs);
            if jobs.len() >= MAX_JOBS {
                return Err(CiError::Full(
                    "the edge holds as many jobs as it can; try again later",
                ));
            }
            let open = jobs
                .values()
                .filter(|j| j.origin == origin && !j.state.finished())
                .count();
            if open >= MAX_OPEN_PER_BOX {
                return Err(CiError::Full(
                    "this box has 64 jobs waiting or running already",
                ));
            }
            // Written under the lock, so a claim never sees a job whose
            // archive is not there yet.
            tokio::fs::write(self.archive_path(&id), &archive).await?;
            let now = Instant::now();
            jobs.insert(
                id.clone(),
                Job {
                    origin: origin.to_string(),
                    spec,
                    state: JobState::Queued,
                    runner: None,
                    attempts: 0,
                    created: now,
                    touched: now,
                    created_at: unix_now(),
                    started_at: None,
                    finished_at: None,
                    exit_code: None,
                    outputs: None,
                    log: String::new(),
                    cut: false,
                    cancel: false,
                },
            );
        }
        self.wake.notify_waiters();
        Ok(id)
    }

    /// The oldest job `runner` may take now.
    fn pick(
        jobs: &HashMap<String, Job>,
        runner: &str,
        foreign: bool,
        now: Instant,
    ) -> Option<String> {
        jobs.iter()
            .filter(|(_, j)| j.state == JobState::Queued)
            .filter(|(_, j)| {
                if j.origin == runner {
                    now.duration_since(j.created) >= ORIGIN_GRACE
                } else {
                    foreign
                }
            })
            .min_by_key(|(id, j)| (j.created, (*id).clone()))
            .map(|(id, _)| id.clone())
    }

    /// Hand `runner` a job, waiting up to `wait` for one. `foreign`: the
    /// box lends now, so it takes other boxes' jobs too.
    pub async fn claim(
        &self,
        runner: &str,
        foreign: bool,
        wait: Duration,
    ) -> Result<Option<Claimed>, CiError> {
        let deadline = Instant::now() + wait.min(MAX_WAIT);
        loop {
            let woken = self.wake.notified();
            tokio::pin!(woken);
            woken.as_mut().enable();
            let picked = {
                let mut jobs = self.jobs.lock().await;
                let now = Instant::now();
                self.sweep(&mut jobs);
                match Self::pick(&jobs, runner, foreign, now) {
                    Some(id) => {
                        let job = jobs.get_mut(&id).ok_or(CiError::NotFound)?;
                        job.state = JobState::Running;
                        job.runner = Some(runner.to_string());
                        job.attempts += 1;
                        job.touched = now;
                        job.started_at = Some(unix_now());
                        Some((id, job.origin.clone(), job.spec.clone()))
                    }
                    None => None,
                }
            };
            if let Some((id, origin, job)) = picked {
                let archive = match tokio::fs::read(self.archive_path(&id)).await {
                    Ok(a) => a,
                    Err(e) => {
                        self.finish(
                            &id,
                            JobState::Error,
                            None,
                            "The edge lost this job's archive.",
                        )
                        .await;
                        return Err(e.into());
                    }
                };
                tracing::info!(target: "ci", "job {id} from {origin} goes to {runner}");
                return Ok(Some(Claimed {
                    id,
                    origin,
                    job,
                    archive,
                }));
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            // A box's own job becomes its own after the grace without
            // anyone waking it, so look again within a second.
            let nap = (deadline - now).min(Duration::from_secs(1));
            let _ = tokio::time::timeout(nap, woken).await;
        }
    }

    async fn finish(&self, id: &str, state: JobState, exit: Option<i32>, note: &str) {
        let mut jobs = self.jobs.lock().await;
        if let Some(job) = jobs.get_mut(id) {
            job.state = state;
            job.exit_code = exit;
            job.finished_at = Some(unix_now());
            append(&mut job.log, &mut job.cut, note);
        }
        drop(jobs);
        let _ = tokio::fs::remove_file(self.archive_path(id)).await;
    }

    /// A runner's word about a job it took. Answers whether the box that
    /// sent it wants it stopped.
    pub async fn update(&self, runner: &str, id: &str, report: Report) -> Result<bool, CiError> {
        let mut jobs = self.jobs.lock().await;
        let job = jobs.get_mut(id).ok_or(CiError::NotFound)?;
        if job.runner.as_deref() != Some(runner) || job.state != JobState::Running {
            // Another box has it now, or it is over: this box stops.
            return Ok(true);
        }
        append(&mut job.log, &mut job.cut, &report.log);
        job.touched = Instant::now();
        let cancel = job.cancel;
        if let Some(state) = report.state {
            if !state.finished() {
                return Err(CiError::Invalid(
                    "a job ends in success, failure, error or cancelled",
                ));
            }
            if let Some(outputs) = &report.outputs {
                let size = serde_json::to_vec(outputs)
                    .map(|v| v.len())
                    .unwrap_or(usize::MAX);
                if size > MAX_OUTPUTS_BYTES {
                    return Err(CiError::Invalid("a job's outputs are at most 64 KiB"));
                }
            }
            job.state = state;
            job.exit_code = report.exit_code;
            job.outputs = report.outputs;
            job.finished_at = Some(unix_now());
            drop(jobs);
            let _ = tokio::fs::remove_file(self.archive_path(id)).await;
        }
        Ok(cancel)
    }

    /// The jobs `origin` asks about, each with its log from the offset
    /// given.
    pub async fn jobs(&self, origin: &str, wanted: &[(String, usize)]) -> Vec<JobView> {
        let mut jobs = self.jobs.lock().await;
        self.sweep(&mut jobs);
        wanted
            .iter()
            .take(256)
            .map(
                |(id, from)| match jobs.get(id).filter(|j| j.origin == origin) {
                    None => JobView {
                        id: id.clone(),
                        state: "lost".into(),
                        runner: None,
                        exit_code: None,
                        outputs: None,
                        created_at: 0,
                        started_at: None,
                        finished_at: None,
                        log: String::new(),
                        log_len: 0,
                    },
                    Some(j) => {
                        let mut from = (*from).min(j.log.len());
                        while !j.log.is_char_boundary(from) {
                            from -= 1;
                        }
                        JobView {
                            id: id.clone(),
                            state: serde_json::to_value(j.state)
                                .ok()
                                .and_then(|v| v.as_str().map(str::to_string))
                                .unwrap_or_default(),
                            runner: j.runner.clone(),
                            exit_code: j.exit_code,
                            outputs: j.outputs.clone(),
                            created_at: j.created_at,
                            started_at: j.started_at,
                            finished_at: j.finished_at,
                            log: j.log[from..].to_string(),
                            log_len: j.log.len(),
                        }
                    }
                },
            )
            .collect()
    }

    /// Stop a job `origin` sent: a queued one at once, a running one at
    /// its runner's next report.
    pub async fn cancel(&self, origin: &str, id: &str) -> Result<(), CiError> {
        let mut jobs = self.jobs.lock().await;
        let job = jobs
            .get_mut(id)
            .filter(|j| j.origin == origin)
            .ok_or(CiError::NotFound)?;
        match job.state {
            JobState::Queued => {
                job.state = JobState::Cancelled;
                job.finished_at = Some(unix_now());
                drop(jobs);
                let _ = tokio::fs::remove_file(self.archive_path(id)).await;
            }
            JobState::Running => job.cancel = true,
            _ => {}
        }
        Ok(())
    }

    /// Requeue jobs whose box went quiet, fail jobs nobody took, forget
    /// old ones.
    fn sweep(&self, jobs: &mut HashMap<String, Job>) {
        let now = Instant::now();
        let mut requeued = false;
        let mut gone = Vec::new();
        for (id, job) in jobs.iter_mut() {
            match job.state {
                JobState::Running if now.duration_since(job.touched) > LEASE => {
                    let who = job.runner.take().unwrap_or_default();
                    if job.attempts < MAX_ATTEMPTS {
                        append(
                            &mut job.log,
                            &mut job.cut,
                            &format!("\n[{who} stopped answering; the job goes to another box.]\n"),
                        );
                        job.state = JobState::Queued;
                        job.cancel = false;
                        requeued = true;
                    } else {
                        append(
                            &mut job.log,
                            &mut job.cut,
                            &format!("\n[{who} stopped answering, and no box finished the job.]\n"),
                        );
                        job.state = JobState::Error;
                        job.finished_at = Some(unix_now());
                        gone.push((id.clone(), false));
                    }
                    job.runner = Some(who);
                }
                JobState::Queued if now.duration_since(job.created) > KEEP_QUEUED => {
                    append(
                        &mut job.log,
                        &mut job.cut,
                        "[No box took this job in a day.]\n",
                    );
                    job.state = JobState::Error;
                    job.finished_at = Some(unix_now());
                    gone.push((id.clone(), false));
                }
                s if s.finished() && now.duration_since(job.touched) > KEEP_FINISHED => {
                    gone.push((id.clone(), true));
                }
                _ => {}
            }
        }
        for (id, forget) in gone {
            let _ = std::fs::remove_file(self.archive_path(&id));
            if forget {
                jobs.remove(&id);
            }
        }
        if requeued {
            self.wake.notify_waiters();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(name: &str) -> JobSpec {
        JobSpec {
            name: name.into(),
            payload: serde_json::json!({ "steps": [{ "run": "make test" }] }),
            timeout_secs: 600,
            repo: "matus/site".into(),
            commit: "a".repeat(40),
        }
    }

    struct Dir(PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    async fn queue() -> (Ci, Dir) {
        let dir = std::env::temp_dir().join(format!(
            "losos-ci-{}-{}",
            std::process::id(),
            crate::identity::random_hex(8).unwrap()
        ));
        (Ci::open(&dir).await.unwrap(), Dir(dir))
    }

    #[tokio::test(start_paused = true)]
    async fn another_box_gets_the_job_first() {
        let (ci, _d) = queue().await;
        let id = ci
            .submit("boxa", spec("build"), b"tar".to_vec())
            .await
            .unwrap();
        // The box that sent it waits out the grace.
        assert!(ci
            .claim("boxa", true, Duration::ZERO)
            .await
            .unwrap()
            .is_none());
        // A box that lends takes it at once, with the archive.
        let got = ci
            .claim("boxb", true, Duration::ZERO)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got.id, id);
        assert_eq!(got.origin, "boxa");
        assert_eq!(got.archive, b"tar");
        // A box that does not lend now never sees it.
        let id2 = ci
            .submit("boxa", spec("test"), b"t".to_vec())
            .await
            .unwrap();
        assert!(ci
            .claim("boxc", false, Duration::ZERO)
            .await
            .unwrap()
            .is_none());
        // After the grace the sender's own runner may take it.
        tokio::time::advance(ORIGIN_GRACE).await;
        let own = ci
            .claim("boxa", false, Duration::ZERO)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(own.id, id2);
    }

    #[tokio::test(start_paused = true)]
    async fn a_waiting_box_is_woken_by_a_new_job() {
        let (ci, _d) = queue().await;
        let ci = std::sync::Arc::new(ci);
        let waiter = {
            let ci = ci.clone();
            tokio::spawn(async move { ci.claim("boxb", true, Duration::from_secs(20)).await })
        };
        tokio::task::yield_now().await;
        let id = ci.submit("boxa", spec("build"), vec![1]).await.unwrap();
        let got = waiter.await.unwrap().unwrap().unwrap();
        assert_eq!(got.id, id);
    }

    #[tokio::test(start_paused = true)]
    async fn the_log_and_the_end_reach_the_sender() {
        let (ci, _d) = queue().await;
        let id = ci.submit("boxa", spec("build"), vec![1]).await.unwrap();
        ci.claim("boxb", true, Duration::ZERO)
            .await
            .unwrap()
            .unwrap();
        let report = |log: &str, state: Option<JobState>| Report {
            log: log.into(),
            state,
            exit_code: state.map(|_| 0),
            outputs: state.map(|_| {
                serde_json::json!({ "size": "12" })
                    .as_object()
                    .unwrap()
                    .clone()
            }),
        };
        assert!(!ci.update("boxb", &id, report("one\n", None)).await.unwrap());
        // Another box cannot report on it: it is told to stop.
        assert!(ci.update("boxc", &id, report("x", None)).await.unwrap());
        let v = ci.jobs("boxa", &[(id.clone(), 0)]).await;
        assert_eq!(v[0].state, "running");
        assert_eq!(v[0].runner.as_deref(), Some("boxb"));
        assert_eq!(v[0].log, "one\n");
        ci.update("boxb", &id, report("two\n", Some(JobState::Success)))
            .await
            .unwrap();
        let v = ci.jobs("boxa", &[(id.clone(), 4)]).await;
        assert_eq!(v[0].state, "success");
        assert_eq!(v[0].log, "two\n");
        assert_eq!(v[0].log_len, 8);
        assert_eq!(v[0].exit_code, Some(0));
        assert_eq!(v[0].outputs.as_ref().unwrap()["size"], "12");
        // Only the sender sees it.
        assert_eq!(ci.jobs("boxc", &[(id.clone(), 0)]).await[0].state, "lost");
    }

    #[tokio::test(start_paused = true)]
    async fn a_quiet_box_loses_the_job_to_another() {
        let (ci, _d) = queue().await;
        let id = ci.submit("boxa", spec("build"), vec![1]).await.unwrap();
        ci.claim("boxb", true, Duration::ZERO)
            .await
            .unwrap()
            .unwrap();
        tokio::time::advance(LEASE + Duration::from_secs(1)).await;
        let again = ci
            .claim("boxc", true, Duration::ZERO)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(again.id, id);
        // The first box's late word stops it.
        assert!(ci.update("boxb", &id, Report::default()).await.unwrap());
        tokio::time::advance(LEASE + Duration::from_secs(1)).await;
        // Two boxes went quiet: it fails.
        assert!(ci
            .claim("boxd", true, Duration::ZERO)
            .await
            .unwrap()
            .is_none());
        let v = ci.jobs("boxa", &[(id, 0)]).await;
        assert_eq!(v[0].state, "error");
        assert!(v[0].log.contains("no box finished the job"));
    }

    #[tokio::test(start_paused = true)]
    async fn cancel_stops_queued_and_running_jobs() {
        let (ci, _d) = queue().await;
        let queued = ci.submit("boxa", spec("a"), vec![1]).await.unwrap();
        ci.cancel("boxa", &queued).await.unwrap();
        assert!(ci
            .claim("boxb", true, Duration::ZERO)
            .await
            .unwrap()
            .is_none());
        let running = ci.submit("boxa", spec("b"), vec![1]).await.unwrap();
        ci.claim("boxb", true, Duration::ZERO)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            ci.cancel("boxc", &running).await,
            Err(CiError::NotFound)
        ));
        ci.cancel("boxa", &running).await.unwrap();
        assert!(ci
            .update("boxb", &running, Report::default())
            .await
            .unwrap());
    }

    #[tokio::test(start_paused = true)]
    async fn limits_hold() {
        let (ci, _d) = queue().await;
        let mut bad = spec("x");
        bad.payload = serde_json::json!("steps");
        assert!(matches!(
            ci.submit("boxa", bad, vec![]).await,
            Err(CiError::Invalid(_))
        ));
        let mut bad = spec("x");
        bad.payload = serde_json::json!({ "run": "x".repeat(MAX_PAYLOAD_BYTES) });
        assert!(matches!(
            ci.submit("boxa", bad, vec![]).await,
            Err(CiError::Invalid(_))
        ));
        let mut bad = spec("x");
        bad.commit = "main".into();
        assert!(matches!(
            ci.submit("boxa", bad, vec![]).await,
            Err(CiError::Invalid(_))
        ));
        assert!(matches!(
            ci.submit("boxa", spec("x"), vec![0; MAX_ARCHIVE_BYTES + 1])
                .await,
            Err(CiError::Invalid(_))
        ));
        for i in 0..MAX_OPEN_PER_BOX {
            ci.submit("boxa", spec(&format!("j{i}")), vec![])
                .await
                .unwrap();
        }
        assert!(matches!(
            ci.submit("boxa", spec("y"), vec![]).await,
            Err(CiError::Full(_))
        ));
        // Another box is not held to boxa's count.
        ci.submit("boxb", spec("y"), vec![]).await.unwrap();
    }

    #[test]
    fn the_log_is_capped() {
        let mut log = String::new();
        let mut cut = false;
        append(&mut log, &mut cut, &"é".repeat(MAX_LOG_BYTES));
        assert!(cut);
        assert!(log.len() < MAX_LOG_BYTES + 100);
        append(&mut log, &mut cut, "more");
        assert!(!log.ends_with("more"));
    }
}
