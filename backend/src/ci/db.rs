//! What `losos-ci` remembers: runs, their jobs and the box's runner
//! setting, in one SQLite file. A job's log is a file beside it
//! (`logs/<job>.log`), appended to as the lines arrive.
//!
//! One connection behind a mutex, as LosOS Git keeps its records
//! (`crate::forge::db`). The schema grows by appending to [`MIGRATIONS`].

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use serde_json::{Map, Value};
use std::path::Path;
use std::sync::Mutex;

const MIGRATIONS: &[&str] = &["
    CREATE TABLE runs (
        id            INTEGER PRIMARY KEY,
        owner         TEXT NOT NULL,
        repo          TEXT NOT NULL,
        private       INTEGER NOT NULL,
        workflow_file TEXT NOT NULL,
        workflow_name TEXT NOT NULL,
        workflow_text TEXT NOT NULL,
        number        INTEGER NOT NULL,
        event         TEXT NOT NULL,
        event_json    TEXT NOT NULL,
        sha           TEXT NOT NULL,
        git_ref       TEXT NOT NULL,
        title         TEXT NOT NULL,
        actor         TEXT NOT NULL,
        state         TEXT NOT NULL,
        message       TEXT NOT NULL DEFAULT '',
        created       INTEGER NOT NULL,
        started       INTEGER,
        finished      INTEGER
    );
    CREATE INDEX runs_repo ON runs (owner, repo, id);
    CREATE INDEX runs_state ON runs (state);
    CREATE TABLE jobs (
        id                INTEGER PRIMARY KEY,
        run_id            INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
        job_key           TEXT NOT NULL,
        name              TEXT NOT NULL,
        matrix_json       TEXT NOT NULL DEFAULT '{}',
        state             TEXT NOT NULL,
        placement         TEXT NOT NULL DEFAULT '',
        edge_id           TEXT,
        runner            TEXT NOT NULL DEFAULT '',
        exit_code         INTEGER,
        outputs_json      TEXT NOT NULL DEFAULT '{}',
        payload_json      TEXT NOT NULL DEFAULT 'null',
        continue_on_error INTEGER NOT NULL DEFAULT 0,
        log_len           INTEGER NOT NULL DEFAULT 0,
        created           INTEGER NOT NULL,
        started           INTEGER,
        finished          INTEGER
    );
    CREATE INDEX jobs_run ON jobs (run_id);
    CREATE INDEX jobs_state ON jobs (state);
    CREATE TABLE settings (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
"];

/// Unix seconds.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A state is over: nothing more will happen to the run or job.
pub fn finished(state: &str) -> bool {
    matches!(
        state,
        "success" | "failure" | "cancelled" | "skipped" | "error"
    )
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: i64,
    pub owner: String,
    pub repo: String,
    pub private: bool,
    pub workflow_file: String,
    pub workflow_name: String,
    #[serde(skip)]
    pub workflow_text: String,
    pub number: i64,
    pub event: String,
    #[serde(skip)]
    pub event_json: String,
    pub sha: String,
    pub git_ref: String,
    pub title: String,
    pub actor: String,
    pub state: String,
    pub message: String,
    pub created: i64,
    pub started: Option<i64>,
    pub finished: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: i64,
    pub run_id: i64,
    pub job_key: String,
    pub name: String,
    pub matrix: Map<String, Value>,
    pub state: String,
    /// `box`: this box's own runner. `mesh`: the edge's queue, any box.
    /// `desktop:<computer>`: a LosOS Desktop computer joined to this box.
    pub placement: String,
    #[serde(skip)]
    pub edge_id: Option<String>,
    /// The box that ran it, as the edge said; this box's id for `box`.
    pub runner: String,
    pub exit_code: Option<i64>,
    pub outputs: Map<String, Value>,
    #[serde(skip)]
    pub payload: Value,
    pub continue_on_error: bool,
    pub log_len: i64,
    pub created: i64,
    pub started: Option<i64>,
    pub finished: Option<i64>,
}

/// A run as it is made.
pub struct NewRun<'a> {
    pub owner: &'a str,
    pub repo: &'a str,
    pub private: bool,
    pub workflow_file: &'a str,
    pub workflow_name: &'a str,
    pub workflow_text: &'a str,
    pub event: &'a str,
    pub event_json: &'a str,
    pub sha: &'a str,
    pub git_ref: &'a str,
    pub title: &'a str,
    pub actor: &'a str,
}

pub struct Store {
    conn: Mutex<Connection>,
}

const RUN_COLUMNS: &str = "id, owner, repo, private, workflow_file, workflow_name, workflow_text, \
     number, event, event_json, sha, git_ref, title, actor, state, message, created, started, finished";
const JOB_COLUMNS: &str =
    "id, run_id, job_key, name, matrix_json, state, placement, edge_id, runner, \
     exit_code, outputs_json, payload_json, continue_on_error, log_len, created, started, finished";

fn run_row(r: &Row<'_>) -> rusqlite::Result<Run> {
    Ok(Run {
        id: r.get(0)?,
        owner: r.get(1)?,
        repo: r.get(2)?,
        private: r.get(3)?,
        workflow_file: r.get(4)?,
        workflow_name: r.get(5)?,
        workflow_text: r.get(6)?,
        number: r.get(7)?,
        event: r.get(8)?,
        event_json: r.get(9)?,
        sha: r.get(10)?,
        git_ref: r.get(11)?,
        title: r.get(12)?,
        actor: r.get(13)?,
        state: r.get(14)?,
        message: r.get(15)?,
        created: r.get(16)?,
        started: r.get(17)?,
        finished: r.get(18)?,
    })
}

fn object(text: String) -> Map<String, Value> {
    match serde_json::from_str(&text) {
        Ok(Value::Object(m)) => m,
        _ => Map::new(),
    }
}

fn job_row(r: &Row<'_>) -> rusqlite::Result<Job> {
    Ok(Job {
        id: r.get(0)?,
        run_id: r.get(1)?,
        job_key: r.get(2)?,
        name: r.get(3)?,
        matrix: object(r.get(4)?),
        state: r.get(5)?,
        placement: r.get(6)?,
        edge_id: r.get(7)?,
        runner: r.get(8)?,
        exit_code: r.get(9)?,
        outputs: object(r.get(10)?),
        payload: serde_json::from_str(&r.get::<_, String>(11)?).unwrap_or(Value::Null),
        continue_on_error: r.get(12)?,
        log_len: r.get(13)?,
        created: r.get(14)?,
        started: r.get(15)?,
        finished: r.get(16)?,
    })
}

impl Store {
    pub fn open(path: &Path) -> anyhow::Result<Store> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::migrate(conn)
    }

    pub fn in_memory() -> anyhow::Result<Store> {
        Self::migrate(Connection::open_in_memory()?)
    }

    fn migrate(mut conn: Connection) -> anyhow::Result<Store> {
        conn.pragma_update(None, "foreign_keys", true)?;
        let have: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(have.max(0) as usize) {
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", i as i64 + 1)?;
            tx.commit()?;
        }
        Ok(Store {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A new run, numbered after the last one of the same workflow.
    pub fn create_run(&self, n: &NewRun<'_>, state: &str, message: &str) -> anyhow::Result<i64> {
        let conn = self.conn();
        let number: i64 = conn.query_row(
            "SELECT COALESCE(MAX(number), 0) + 1 FROM runs
             WHERE owner = ?1 AND repo = ?2 AND workflow_file = ?3",
            params![n.owner, n.repo, n.workflow_file],
            |r| r.get(0),
        )?;
        let now = now();
        let done = finished(state).then_some(now);
        conn.execute(
            "INSERT INTO runs (owner, repo, private, workflow_file, workflow_name, workflow_text,
               number, event, event_json, sha, git_ref, title, actor, state, message, created, finished)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                n.owner,
                n.repo,
                n.private,
                n.workflow_file,
                n.workflow_name,
                n.workflow_text,
                number,
                n.event,
                n.event_json,
                n.sha,
                n.git_ref,
                n.title,
                n.actor,
                state,
                message,
                now,
                done
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn run(&self, id: i64) -> anyhow::Result<Option<Run>> {
        Ok(self
            .conn()
            .query_row(
                &format!("SELECT {RUN_COLUMNS} FROM runs WHERE id = ?1"),
                params![id],
                run_row,
            )
            .optional()?)
    }

    /// The newest runs first, `before` an id for the next page.
    pub fn runs(
        &self,
        repo: Option<(&str, &str)>,
        before: Option<i64>,
        limit: u32,
    ) -> anyhow::Result<Vec<Run>> {
        let conn = self.conn();
        let before = before.unwrap_or(i64::MAX);
        let mut out = Vec::new();
        match repo {
            Some((owner, name)) => {
                let mut stmt = conn.prepare(&format!(
                    "SELECT {RUN_COLUMNS} FROM runs WHERE owner = ?1 AND repo = ?2 AND id < ?3
                     ORDER BY id DESC LIMIT ?4"
                ))?;
                for r in stmt.query_map(params![owner, name, before, limit], run_row)? {
                    out.push(r?);
                }
            }
            None => {
                let mut stmt = conn.prepare(&format!(
                    "SELECT {RUN_COLUMNS} FROM runs WHERE id < ?1 ORDER BY id DESC LIMIT ?2"
                ))?;
                for r in stmt.query_map(params![before, limit], run_row)? {
                    out.push(r?);
                }
            }
        }
        Ok(out)
    }

    /// Runs that are not over yet, oldest first.
    pub fn active_runs(&self) -> anyhow::Result<Vec<Run>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {RUN_COLUMNS} FROM runs WHERE state IN ('queued', 'running') ORDER BY id"
        ))?;
        let rows = stmt.query_map([], run_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn set_run_state(&self, id: i64, state: &str, message: &str) -> anyhow::Result<()> {
        let now = now();
        self.conn().execute(
            "UPDATE runs SET state = ?2, message = ?3,
               started = CASE WHEN ?2 = 'running' THEN COALESCE(started, ?4) ELSE started END,
               finished = CASE WHEN ?2 IN ('success','failure','cancelled','skipped','error')
                          THEN COALESCE(finished, ?4) ELSE NULL END
             WHERE id = ?1",
            params![id, state, message, now],
        )?;
        Ok(())
    }

    /// A job of `run`, in the state it starts in.
    #[allow(clippy::too_many_arguments)]
    pub fn add_job(
        &self,
        run: i64,
        key: &str,
        name: &str,
        matrix: &Map<String, Value>,
        state: &str,
        placement: &str,
        payload: &Value,
        continue_on_error: bool,
    ) -> anyhow::Result<i64> {
        let conn = self.conn();
        let now = now();
        conn.execute(
            "INSERT INTO jobs (run_id, job_key, name, matrix_json, state, placement, payload_json,
               continue_on_error, created, finished)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                run,
                key,
                name,
                serde_json::to_string(matrix)?,
                state,
                placement,
                serde_json::to_string(payload)?,
                continue_on_error,
                now,
                finished(state).then_some(now)
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn job(&self, id: i64) -> anyhow::Result<Option<Job>> {
        Ok(self
            .conn()
            .query_row(
                &format!("SELECT {JOB_COLUMNS} FROM jobs WHERE id = ?1"),
                params![id],
                job_row,
            )
            .optional()?)
    }

    pub fn jobs(&self, run: i64) -> anyhow::Result<Vec<Job>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {JOB_COLUMNS} FROM jobs WHERE run_id = ?1 ORDER BY id"
        ))?;
        let rows = stmt.query_map(params![run], job_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The oldest job waiting for this box's own runner, marked running in
    /// the same statement so two runners never take one job.
    pub fn take_box_job(&self, runner: &str) -> anyhow::Result<Option<Job>> {
        let conn = self.conn();
        let id: Option<i64> = conn
            .query_row(
                "UPDATE jobs SET state = 'running', runner = ?1, started = ?2
                 WHERE id = (SELECT id FROM jobs WHERE state = 'queued' AND placement = 'box'
                             ORDER BY id LIMIT 1)
                 RETURNING id",
                params![runner, now()],
                |r| r.get(0),
            )
            .optional()?;
        drop(conn);
        match id {
            Some(id) => self.job(id),
            None => Ok(None),
        }
    }

    /// The oldest queued job placed `placement`, now running on `runner`.
    pub fn take_placed_job(&self, placement: &str, runner: &str) -> anyhow::Result<Option<Job>> {
        let conn = self.conn();
        let id: Option<i64> = conn
            .query_row(
                "UPDATE jobs SET state = 'running', runner = ?2, started = ?3
                 WHERE id = (SELECT id FROM jobs WHERE state = 'queued' AND placement = ?1
                             ORDER BY id LIMIT 1)
                 RETURNING id",
                params![placement, runner, now()],
                |r| r.get(0),
            )
            .optional()?;
        drop(conn);
        match id {
            Some(id) => self.job(id),
            None => Ok(None),
        }
    }

    /// What the edge or the runner said about a job.
    pub fn set_job_state(
        &self,
        id: i64,
        state: &str,
        runner: Option<&str>,
        exit_code: Option<i64>,
        outputs: Option<&Map<String, Value>>,
    ) -> anyhow::Result<()> {
        let now = now();
        let outputs = outputs.map(serde_json::to_string).transpose()?;
        self.conn().execute(
            "UPDATE jobs SET state = ?2,
               runner = COALESCE(?3, runner),
               exit_code = COALESCE(?4, exit_code),
               outputs_json = COALESCE(?5, outputs_json),
               started = CASE WHEN ?2 = 'running' THEN COALESCE(started, ?6) ELSE started END,
               finished = CASE WHEN ?2 IN ('success','failure','cancelled','skipped','error')
                          THEN COALESCE(finished, ?6) ELSE NULL END
             WHERE id = ?1",
            params![id, state, runner, exit_code, outputs, now],
        )?;
        Ok(())
    }

    pub fn set_job_edge_id(&self, id: i64, edge_id: &str) -> anyhow::Result<()> {
        self.conn().execute(
            "UPDATE jobs SET edge_id = ?2 WHERE id = ?1",
            params![id, edge_id],
        )?;
        Ok(())
    }

    pub fn set_placement(&self, id: i64, placement: &str) -> anyhow::Result<()> {
        self.conn().execute(
            "UPDATE jobs SET placement = ?2 WHERE id = ?1",
            params![id, placement],
        )?;
        Ok(())
    }

    /// Jobs on the edge's queue that are not over, whatever their run.
    pub fn open_mesh_jobs(&self) -> anyhow::Result<Vec<Job>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {JOB_COLUMNS} FROM jobs WHERE placement = 'mesh' AND edge_id IS NOT NULL
               AND state IN ('queued', 'running') ORDER BY id"
        ))?;
        let rows = stmt.query_map([], job_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn set_log_len(&self, id: i64, len: i64) -> anyhow::Result<()> {
        self.conn().execute(
            "UPDATE jobs SET log_len = ?2 WHERE id = ?1",
            params![id, len],
        )?;
        Ok(())
    }

    pub fn setting(&self, key: &str) -> anyhow::Result<Option<String>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> anyhow::Result<()> {
        self.conn().execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Runs over and older than `before`, with their jobs, for the
    /// housekeeping pass. Returns the job ids, whose logs go too.
    pub fn forget_before(&self, before: i64) -> anyhow::Result<Vec<i64>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT jobs.id FROM jobs JOIN runs ON runs.id = jobs.run_id
             WHERE runs.finished IS NOT NULL AND runs.finished < ?1",
        )?;
        let ids = stmt
            .query_map(params![before], |r| r.get(0))?
            .collect::<Result<Vec<i64>, _>>()?;
        conn.execute(
            "DELETE FROM runs WHERE finished IS NOT NULL AND finished < ?1",
            params![before],
        )?;
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_run<'a>() -> NewRun<'a> {
        NewRun {
            owner: "matus",
            repo: "site",
            private: false,
            workflow_file: ".github/workflows/ci.yml",
            workflow_name: "CI",
            workflow_text: "",
            event: "push",
            event_json: "{}",
            sha: "a",
            git_ref: "refs/heads/main",
            title: "main",
            actor: "matus",
        }
    }

    #[test]
    fn runs_are_numbered_per_workflow() {
        let s = Store::in_memory().unwrap();
        let a = s.create_run(&new_run(), "queued", "").unwrap();
        let b = s.create_run(&new_run(), "queued", "").unwrap();
        assert_eq!(s.run(a).unwrap().unwrap().number, 1);
        assert_eq!(s.run(b).unwrap().unwrap().number, 2);
        assert_eq!(s.active_runs().unwrap().len(), 2);
        s.set_run_state(a, "success", "").unwrap();
        let a = s.run(a).unwrap().unwrap();
        assert!(a.finished.is_some());
        assert_eq!(s.active_runs().unwrap().len(), 1);
        let page = s.runs(Some(("matus", "site")), None, 1).unwrap();
        assert_eq!(page[0].id, b);
        assert_eq!(s.runs(None, Some(b), 10).unwrap()[0].id, a.id);
    }

    #[test]
    fn a_box_job_is_taken_once() {
        let s = Store::in_memory().unwrap();
        let run = s.create_run(&new_run(), "running", "").unwrap();
        let m = Map::new();
        s.add_job(run, "a", "a", &m, "queued", "mesh", &Value::Null, false)
            .unwrap();
        let j = s
            .add_job(run, "b", "b", &m, "queued", "box", &Value::Null, false)
            .unwrap();
        let taken = s.take_box_job("box-1").unwrap().unwrap();
        assert_eq!(taken.id, j);
        assert_eq!(taken.state, "running");
        assert!(s.take_box_job("box-1").unwrap().is_none());
        let mut outs = Map::new();
        outs.insert("v".into(), "1".into());
        s.set_job_state(j, "success", None, Some(0), Some(&outs))
            .unwrap();
        let done = s.job(j).unwrap().unwrap();
        assert_eq!(done.outputs["v"], "1");
        assert_eq!(done.runner, "box-1");
        assert!(done.finished.is_some());
        s.set_run_state(run, "success", "").unwrap();
        let gone = s.forget_before(now() + 1).unwrap();
        assert_eq!(gone.len(), 2);
        assert!(s.run(run).unwrap().is_none());
    }
}
