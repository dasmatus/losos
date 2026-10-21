//! The reviewer's records: which repositories asked for it, and the queue of
//! reviews, one row per review with how far it got.
//!
//! The queue is in the database rather than in memory so a restart (every
//! `nixos-rebuild switch` restarts losos-git) picks up where it was: a
//! review that was running starts again from the beginning, and one that
//! was waiting keeps its place.

use super::super::db::Store;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

/// The table this module owns, and the repository setting. Appended to
/// `db::MIGRATIONS`.
pub const MIGRATION: &str = "
    ALTER TABLE repos ADD COLUMN ai_review INTEGER NOT NULL DEFAULT 0;
    CREATE TABLE review_jobs (
        id          INTEGER PRIMARY KEY,
        repo_id     INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
        issue_id    INTEGER NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
        head        TEXT NOT NULL,
        since       TEXT,
        reason      TEXT NOT NULL,
        state       TEXT NOT NULL,
        files_total INTEGER NOT NULL DEFAULT 0,
        files_done  INTEGER NOT NULL DEFAULT 0,
        comments    INTEGER NOT NULL DEFAULT 0,
        review_id   INTEGER,
        error       TEXT,
        created     TEXT NOT NULL,
        started     TEXT,
        finished    TEXT
    );
    CREATE INDEX review_jobs_waiting ON review_jobs (state, id);
    CREATE INDEX review_jobs_issue ON review_jobs (issue_id, id);
";

/// Why a review was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Reason {
    /// The pull request was opened or reopened: the whole of it.
    Opened,
    /// New commits: what they changed, when the old head is still in the
    /// branch's history.
    Pushed,
    /// Someone asked the reviewer again: the whole of it.
    Requested,
}

impl Reason {
    fn as_str(self) -> &'static str {
        match self {
            Reason::Opened => "opened",
            Reason::Pushed => "pushed",
            Reason::Requested => "requested",
        }
    }
    fn parse(s: &str) -> Reason {
        match s {
            "pushed" => Reason::Pushed,
            "requested" => Reason::Requested,
            _ => Reason::Opened,
        }
    }
}

/// Where a review is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    Queued,
    Reviewing,
    Done,
    Failed,
    /// A newer push or a closed pull request made it pointless.
    Superseded,
}

impl JobState {
    fn as_str(self) -> &'static str {
        match self {
            JobState::Queued => "queued",
            JobState::Reviewing => "reviewing",
            JobState::Done => "done",
            JobState::Failed => "failed",
            JobState::Superseded => "superseded",
        }
    }
    fn parse(s: &str) -> JobState {
        match s {
            "reviewing" => JobState::Reviewing,
            "done" => JobState::Done,
            "failed" => JobState::Failed,
            "superseded" => JobState::Superseded,
            _ => JobState::Queued,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: i64,
    pub repo_id: i64,
    pub issue_id: i64,
    pub head: String,
    pub since: Option<String>,
    pub reason: Reason,
    pub state: JobState,
    pub files_total: i64,
    pub files_done: i64,
    pub comments: i64,
    pub review_id: Option<i64>,
    pub error: Option<String>,
    pub created: String,
    pub started: Option<String>,
    pub finished: Option<String>,
}

const JOB_COLS: &str = "id, repo_id, issue_id, head, since, reason, state, files_total,
    files_done, comments, review_id, error, created, started, finished";

fn job_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Job> {
    Ok(Job {
        id: r.get(0)?,
        repo_id: r.get(1)?,
        issue_id: r.get(2)?,
        head: r.get(3)?,
        since: r.get(4)?,
        reason: Reason::parse(&r.get::<_, String>(5)?),
        state: JobState::parse(&r.get::<_, String>(6)?),
        files_total: r.get(7)?,
        files_done: r.get(8)?,
        comments: r.get(9)?,
        review_id: r.get(10)?,
        error: r.get(11)?,
        created: r.get(12)?,
        started: r.get(13)?,
        finished: r.get(14)?,
    })
}

impl Store {
    /// Whether the repository asked for AI review. Off until someone with
    /// admin rights on it turns it on.
    pub fn ai_review(&self, repo_id: i64) -> anyhow::Result<bool> {
        Ok(self
            .conn()
            .query_row(
                "SELECT ai_review FROM repos WHERE id = ?1",
                [repo_id],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(false))
    }

    /// Turn AI review on or off. Off also drops every review still waiting.
    pub fn set_ai_review(&self, repo_id: i64, on: bool) -> anyhow::Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE repos SET ai_review = ?2 WHERE id = ?1",
            params![repo_id, on],
        )?;
        if !on {
            conn.execute(
                "UPDATE review_jobs SET state = 'superseded', finished = ?2
                 WHERE repo_id = ?1 AND state IN ('queued', 'reviewing')",
                params![repo_id, super::super::now()],
            )?;
        }
        Ok(())
    }

    /// Put a review in the queue. Whatever was waiting or running for the
    /// same pull request is superseded: only the newest head is worth the
    /// box's time.
    pub fn queue_review(
        &self,
        repo_id: i64,
        issue_id: i64,
        head: &str,
        since: Option<&str>,
        reason: Reason,
    ) -> anyhow::Result<Job> {
        let now = super::super::now();
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE review_jobs SET state = 'superseded', finished = ?2
             WHERE issue_id = ?1 AND state IN ('queued', 'reviewing')",
            params![issue_id, now],
        )?;
        tx.execute(
            "INSERT INTO review_jobs (repo_id, issue_id, head, since, reason, state, created)
             VALUES (?1, ?2, ?3, ?4, ?5, 'queued', ?6)",
            params![repo_id, issue_id, head, since, reason.as_str(), now],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        drop(conn);
        self.review_job(id)?
            .ok_or_else(|| anyhow::anyhow!("the queued review vanished"))
    }

    /// The pull request was closed or merged: nothing for it waits any more.
    pub fn drop_reviews(&self, issue_id: i64) -> anyhow::Result<()> {
        self.conn().execute(
            "UPDATE review_jobs SET state = 'superseded', finished = ?2
             WHERE issue_id = ?1 AND state IN ('queued', 'reviewing')",
            params![issue_id, super::super::now()],
        )?;
        Ok(())
    }

    pub fn review_job(&self, id: i64) -> anyhow::Result<Option<Job>> {
        Ok(self
            .conn()
            .query_row(
                &format!("SELECT {JOB_COLS} FROM review_jobs WHERE id = ?1"),
                [id],
                job_row,
            )
            .optional()?)
    }

    /// The newest review of a pull request, whatever its state.
    pub fn last_review_job(&self, issue_id: i64) -> anyhow::Result<Option<Job>> {
        Ok(self
            .conn()
            .query_row(
                &format!(
                    "SELECT {JOB_COLS} FROM review_jobs WHERE issue_id = ?1
                     ORDER BY id DESC LIMIT 1"
                ),
                [issue_id],
                job_row,
            )
            .optional()?)
    }

    /// The head the reviewer last finished reviewing, for a push to be
    /// reviewed from there.
    pub fn last_reviewed_head(&self, issue_id: i64) -> anyhow::Result<Option<String>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT head FROM review_jobs WHERE issue_id = ?1 AND state = 'done'
                 ORDER BY id DESC LIMIT 1",
                [issue_id],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// How many reviews wait before this one.
    pub fn reviews_ahead(&self, job_id: i64) -> anyhow::Result<i64> {
        Ok(self.conn().query_row(
            "SELECT COUNT(*) FROM review_jobs
             WHERE id < ?1 AND state IN ('queued', 'reviewing')",
            [job_id],
            |r| r.get(0),
        )?)
    }

    /// Take the oldest waiting review and mark it running.
    pub fn next_review_job(&self) -> anyhow::Result<Option<Job>> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let job = tx
            .query_row(
                &format!(
                    "SELECT {JOB_COLS} FROM review_jobs WHERE state = 'queued'
                     ORDER BY id LIMIT 1"
                ),
                [],
                job_row,
            )
            .optional()?;
        let Some(mut job) = job else {
            return Ok(None);
        };
        let now = super::super::now();
        tx.execute(
            "UPDATE review_jobs SET state = 'reviewing', started = ?2, files_done = 0,
             comments = 0, error = NULL WHERE id = ?1",
            params![job.id, now],
        )?;
        tx.commit()?;
        job.state = JobState::Reviewing;
        job.started = Some(now);
        Ok(Some(job))
    }

    /// After a restart: what was running starts again.
    pub fn requeue_reviews(&self) -> anyhow::Result<usize> {
        Ok(self.conn().execute(
            "UPDATE review_jobs SET state = 'queued', started = NULL
             WHERE state = 'reviewing'",
            [],
        )?)
    }

    /// How far a running review is. Returns false when it was superseded
    /// meanwhile, so the worker stops asking.
    pub fn review_progress(
        &self,
        job_id: i64,
        files_done: i64,
        files_total: i64,
    ) -> anyhow::Result<bool> {
        let n = self.conn().execute(
            "UPDATE review_jobs SET files_done = ?2, files_total = ?3
             WHERE id = ?1 AND state = 'reviewing'",
            params![job_id, files_done, files_total],
        )?;
        Ok(n > 0)
    }

    pub fn finish_review_job(
        &self,
        job_id: i64,
        review_id: Option<i64>,
        comments: i64,
    ) -> anyhow::Result<()> {
        self.conn().execute(
            "UPDATE review_jobs SET state = 'done', review_id = ?2, comments = ?3,
             files_done = files_total, finished = ?4 WHERE id = ?1 AND state = 'reviewing'",
            params![job_id, review_id, comments, super::super::now()],
        )?;
        Ok(())
    }

    /// Back in the queue, at its old place: the model was not ready.
    pub fn put_back_review(&self, job_id: i64) -> anyhow::Result<()> {
        self.conn().execute(
            "UPDATE review_jobs SET state = 'queued', started = NULL
             WHERE id = ?1 AND state = 'reviewing'",
            [job_id],
        )?;
        Ok(())
    }

    /// The number a pull request has in its repository.
    pub fn issue_number(&self, issue_id: i64) -> anyhow::Result<Option<i64>> {
        Ok(self
            .conn()
            .query_row("SELECT number FROM issues WHERE id = ?1", [issue_id], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn fail_review_job(&self, job_id: i64, error: &str) -> anyhow::Result<()> {
        self.conn().execute(
            "UPDATE review_jobs SET state = 'failed', error = ?2, finished = ?3
             WHERE id = ?1 AND state = 'reviewing'",
            params![job_id, error, super::super::now()],
        )?;
        Ok(())
    }
}
