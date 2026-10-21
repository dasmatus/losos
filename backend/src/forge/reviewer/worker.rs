//! The queue's one worker: a thread of the losos-git process that takes the
//! oldest waiting review, asks the model, and posts the review as the bot.
//!
//! One review at a time, one question at a time. The box runs LosOS Cloud
//! and Git and its owner's other apps on as little as two cores; a second
//! reviewer would only make both slower and hold a second answer in
//! Ollama's short queue, where a build from the App builder waits too.
//! Progress is written after each file, to the queue row (the pull
//! request's page reads it) and as the head commit's "LosOS Reviewer"
//! status (the Checks box).

use super::super::db::{NewLineComment, ReviewState, Side, Store};
use super::super::events::Event;
use super::super::server::State;
use super::super::statuses::{self, NewStatus};
use super::jobs::{Job, Reason};
use super::{model, patch, review};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

/// The bot account the reviews are posted as.
pub const BOT_LOGIN: &str = "losos-reviewer";
pub const BOT_NAME: &str = "LosOS Reviewer";
/// The commit status context the Checks box shows.
pub const STATUS_CONTEXT: &str = "LosOS Reviewer";

/// How long to wait before asking again when the model is still being
/// downloaded or the box was short of memory.
const RETRY: Duration = Duration::from_secs(300);

/// The reviewer's part of the server's shared state: where the model is,
/// and the bell that wakes the worker.
#[derive(Debug, Default)]
pub struct Handle {
    pub config: Option<model::Config>,
    bell: Mutex<bool>,
    rung: Condvar,
}

impl Handle {
    pub fn new(config: Option<model::Config>) -> Handle {
        Handle {
            config,
            ..Handle::default()
        }
    }

    pub fn from_env() -> Handle {
        Handle::new(model::Config::from_env())
    }

    /// The box has a code model to review with (Build with AI is on).
    pub fn available(&self) -> bool {
        self.config.is_some()
    }

    pub fn wake(&self) {
        *self.bell.lock().unwrap_or_else(|p| p.into_inner()) = true;
        self.rung.notify_all();
    }

    fn wait(&self, at_most: Duration) {
        let guard = self.bell.lock().unwrap_or_else(|p| p.into_inner());
        let (mut guard, _) = self
            .rung
            .wait_timeout_while(guard, at_most, |rung| !*rung)
            .unwrap_or_else(|p| p.into_inner());
        *guard = false;
    }
}

/// A pull request was opened, reopened, pushed to, closed or merged.
fn on_event(state: &State, event: &Event) {
    let Event::PullRequest {
        action,
        owner,
        repo,
        pr,
        ..
    } = event
    else {
        return;
    };
    let result = (|| -> anyhow::Result<()> {
        let Some(repo) = state.store.repo(owner, repo)? else {
            return Ok(());
        };
        let Some(issue) = state.store.issue(repo.id, pr.number)? else {
            return Ok(());
        };
        let (repo_id, issue_id, head) = (repo.id, issue.id, pr.head_sha.as_str());
        match action.as_str() {
            "closed" | "merged" => {
                state.store.drop_reviews(issue_id)?;
                return Ok(());
            }
            _ if !state.reviewer.available() || !state.store.ai_review(repo_id)? => return Ok(()),
            _ if !statuses::is_sha(head) => return Ok(()),
            "opened" | "reopened" => {
                state
                    .store
                    .queue_review(repo_id, issue_id, head, None, Reason::Opened)?;
            }
            "synchronize" => {
                let since = state.store.last_reviewed_head(issue_id)?;
                state.store.queue_review(
                    repo_id,
                    issue_id,
                    head,
                    since.as_deref(),
                    Reason::Pushed,
                )?;
            }
            _ => return Ok(()),
        }
        state.reviewer.wake();
        Ok(())
    })();
    if let Err(e) = result {
        tracing::warn!(error = ?e, "the reviewer could not take a pull request event");
    }
}

/// Someone asked a reviewer for a review; the reviewer answers when it is
/// the one asked.
pub fn review_requested(state: &State, repo_id: i64, issue_id: i64, head: &str, user_id: i64) {
    let result = (|| -> anyhow::Result<()> {
        let bot = state.store.ensure_bot(BOT_LOGIN, BOT_NAME)?;
        if user_id != bot.id || !state.reviewer.available() || !state.store.ai_review(repo_id)? {
            return Ok(());
        }
        state
            .store
            .queue_review(repo_id, issue_id, head, None, Reason::Requested)?;
        state.reviewer.wake();
        Ok(())
    })();
    if let Err(e) = result {
        tracing::warn!(error = ?e, "the reviewer could not take a review request");
    }
}

/// Listen for pull request events and start the worker thread, on a box
/// with a code model. What was running when the process stopped is queued
/// again first.
pub fn start(state: actix_web::web::Data<State>) -> std::io::Result<()> {
    if !state.reviewer.available() {
        tracing::info!("no code model on this box (Build with AI is off): LosOS Reviewer is off");
        return Ok(());
    }
    let listening = state.clone();
    super::super::events::subscribe(Box::new(move |e| on_event(&listening, e)));
    std::thread::Builder::new()
        .name("reviewer".into())
        .spawn(move || {
            if let Err(e) = state
                .store
                .ensure_bot(BOT_LOGIN, BOT_NAME)
                .and_then(|_| state.store.requeue_reviews())
            {
                tracing::error!(error = ?e, "the reviewer could not start");
                return;
            }
            loop {
                match state.store.next_review_job() {
                    Ok(Some(job)) => {
                        if let Some(wait) = work(&state, &job) {
                            state.reviewer.wait(wait);
                        }
                    }
                    Ok(None) => state.reviewer.wait(Duration::from_secs(60)),
                    Err(e) => {
                        tracing::error!(error = ?e, "the review queue could not be read");
                        state.reviewer.wait(Duration::from_secs(60));
                    }
                }
            }
        })?;
    Ok(())
}

/// Do one review. Returns how long to rest when it had to be put back.
fn work(state: &State, job: &Job) -> Option<Duration> {
    // The row links to the pull request's changed files, where the
    // comments are.
    let target_url = match (
        state.store.repo_by_id(job.repo_id),
        state.store.issue_number(job.issue_id),
    ) {
        (Ok(Some(repo)), Ok(Some(number))) => {
            format!("/git/{}/{}/pulls/{number}/files", repo.owner, repo.name)
        }
        _ => String::new(),
    };
    let status = |s: &str, description: &str| {
        let status = NewStatus {
            state: s.to_string(),
            context: STATUS_CONTEXT.to_string(),
            description: description.to_string(),
            target_url: target_url.clone(),
        };
        if let Err(e) = statuses::set(&state.store, job.repo_id, &job.head, &status, BOT_LOGIN) {
            tracing::debug!(error = ?e, "the reviewer could not write its status");
        }
    };
    let Some(config) = state.reviewer.config.as_ref() else {
        let _ = state
            .store
            .fail_review_job(job.id, "Build with AI is off on this box.");
        return None;
    };
    match model::has_model(config) {
        Ok(true) => {}
        Ok(false) | Err(_) => {
            tracing::info!(model = %config.model, "the code model is not there yet; the review waits");
            status("pending", "Waiting for the box's code model to download");
            let _ = state.store.put_back_review(job.id);
            return Some(RETRY);
        }
    }
    status("pending", "Reading the changes");
    match review_job(state, config, job, &status) {
        Ok(Done::Posted(comments)) => {
            status(
                "success",
                &match comments {
                    0 => "Found nothing to comment on".to_string(),
                    1 => "Left 1 comment".to_string(),
                    n => format!("Left {n} comments"),
                },
            );
            None
        }
        Ok(Done::Stopped) => None,
        Err(e) => {
            let said = format!("{e:#}");
            tracing::warn!(error = %said, job = job.id, "a review failed");
            if said.contains("memory") {
                status("pending", "Waiting for memory: the box is busy");
                let _ = state.store.put_back_review(job.id);
                return Some(RETRY);
            }
            let _ = state.store.fail_review_job(job.id, &said);
            status("error", "Could not finish the review");
            None
        }
    }
}

enum Done {
    Posted(usize),
    Stopped,
}

fn review_job(
    state: &State,
    config: &model::Config,
    job: &Job,
    status: &dyn Fn(&str, &str),
) -> anyhow::Result<Done> {
    let store: &Store = &state.store;
    let repo = store
        .repo_by_id(job.repo_id)?
        .ok_or_else(|| anyhow::anyhow!("the repository is gone"))?;
    let number = store
        .issue_number(job.issue_id)?
        .ok_or_else(|| anyhow::anyhow!("the pull request is gone"))?;
    let pull = store
        .issue(repo.id, number)?
        .ok_or_else(|| anyhow::anyhow!("the pull request is gone"))?;
    let path = state.config.repo_path(&repo.owner, &repo.name);
    let base = pull
        .base_branch
        .clone()
        .unwrap_or(repo.default_branch.clone());
    let git = |args: &[&str]| state.git.run(&path, args);

    // A push is reviewed from the last reviewed head when that head is still
    // in the branch's history; after a force push, from where the branch
    // left its base.
    let (from, since_last) = match job.since.as_deref() {
        Some(since) if git(&["merge-base", "--is-ancestor", since, &job.head]).is_ok() => {
            (since.to_string(), true)
        }
        _ => (
            git(&["merge-base", &format!("refs/heads/{base}"), &job.head])?
                .trim()
                .to_string(),
            false,
        ),
    };
    let diff = git(&[
        "diff",
        "--no-color",
        "--no-ext-diff",
        "-M",
        "-U3",
        &from,
        &job.head,
    ])?;
    let mut files = patch::files(&diff);
    for f in &mut files {
        let Some(p) = f.path.as_deref() else { continue };
        if patch::language(p).contains("Script") {
            f.head = git(&["show", &format!("{}:{p}", job.head)]).ok();
        }
    }

    let total_files = files.iter().filter(|f| f.path.is_some()).count();
    let mut progress = |done: usize, total: usize| {
        let going = store
            .review_progress(job.id, done as i64, total as i64)
            .unwrap_or(false)
            && store.ai_review(repo.id).unwrap_or(false);
        if going {
            status(
                "running",
                &format!(
                    "Reviewing: {done} of {total} {} done",
                    if total == 1 { "file" } else { "files" }
                ),
            );
        }
        going
    };
    let outcome = review::run(config, &pull.title, &files, &mut progress)?;
    if outcome.stopped {
        return Ok(Done::Stopped);
    }
    // Still the review to post: a push while the last answer came in
    // superseded it.
    if store.review_job(job.id)?.map(|j| j.state) != Some(super::jobs::JobState::Reviewing) {
        return Ok(Done::Stopped);
    }
    let bot = store.ensure_bot(BOT_LOGIN, BOT_NAME)?;
    let comments: Vec<NewLineComment> = outcome
        .comments
        .iter()
        .map(|c| NewLineComment {
            path: c.path.clone(),
            line: i64::from(c.line),
            side: Side::New,
            body: c.body.clone(),
        })
        .collect();
    let body = outcome.body(&config.model, since_last);
    let posted = store.create_review(
        job.issue_id,
        bot.id,
        ReviewState::Commented,
        &body,
        &job.head,
        &comments,
    )?;
    store.drop_review_request(job.issue_id, bot.id)?;
    store.finish_review_job(job.id, Some(posted.id), comments.len() as i64)?;
    tracing::info!(
        repo = %format!("{}/{}", repo.owner, repo.name),
        number,
        files = total_files,
        comments = comments.len(),
        "posted a review"
    );
    Ok(Done::Posted(comments.len()))
}
