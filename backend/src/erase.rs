//! Backups, restores, and the full erase, as commands over [`Losos`].
//!
//! # Two kinds of reset
//!
//! `factory-reset` ([`crate::losos::cmd_factory_reset`]) puts the settings
//! back and keeps everything else: files, apps, the owner. This module adds
//! the other kind, an **erase**: the box ends up as it was on its first boot,
//! with no files, no accounts, no settings and no owner, waiting for the
//! setup wizard. Only the system itself and the installer's own facts (which
//! drives, which firmware, how the disk unlocks) stay, so it boots.
//!
//! # The order, and where it can still be stopped
//!
//! ```text
//!   backingUp ──► waiting ──► leaving ──► resetting ──► restarting ──► (wiped at boot)
//!       │            │
//!       └── cancel ──┘      nothing outside the box has changed yet
//! ```
//!
//!  1. **backingUp** (when asked for): a fresh backup to the owner's bucket
//!     (`crate::backup`). If it fails, the erase stops in `failed` and
//!     nothing is touched: an erase that was meant to come with a backup
//!     never goes ahead without one.
//!  2. **waiting**: a countdown, `losos.reset.graceMinutes` long. Cancel is
//!     one click from any screen of the admin UI. Nothing has happened yet
//!     that cannot be taken back.
//!  3. **leaving**: the box gives up what it holds outside itself, while it
//!     still has the credentials to: its custom domains on the edge, its open
//!     market listings, and its place in the edge's registry. Best effort and
//!     counted ([`Outside`]); a box with no edge skips it.
//!  4. **resetting**: the default settings are written and committed, and
//!     the box rebuilds into them, so the system the wipe boots into is the
//!     factory one.
//!  5. **restarting**: a marker is left on `/persist` and the box reboots.
//!     `losos-factory-wipe.service` (`modules/backup.nix`) finds the marker
//!     early in the next boot, before any service that owns data has
//!     started, and deletes the data, the secrets and this file with them.
//!
//! The countdown sits *before* the edge is told anything on purpose: a
//! cancelled erase must leave the box exactly as it was, domains and all.
//!
//! # The tick
//!
//! Every step past the first is driven by [`tick`], which lososd calls on a
//! short cadence from its own thread and which reads everything it needs out
//! of `state.json`. A rebuild restarts lososd halfway through step 4; the
//! new process picks the erase up where the file says it is.

use crate::backup::{Busy, Invalid, Job, JobState, Kind};
use crate::losos::Losos;
use crate::market::{Op, Outcome};
use crate::model::RebuildState;
use crate::supervisor::{Poll, MAX_UNKNOWN_POLLS};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// How often lososd drives [`tick`].
pub const TICK_EVERY: std::time::Duration = std::time::Duration::from_secs(2);

/// Rebuild message for the erase's settings reset.
const MSG_ERASE_RESET: &str = "erase: resetting settings";
/// Rebuild message for applying the settings a restore brought back.
const MSG_RESTORED: &str = "restore: applying the restored settings";

/// Where an erase is. See the module docs for the order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    BackingUp,
    Waiting,
    Leaving,
    Resetting,
    Restarting,
    Failed,
}

impl Phase {
    /// Whether cancelling is still possible: nothing outside the box, and
    /// nothing on it, has changed yet.
    #[must_use]
    pub fn cancellable(self) -> bool {
        matches!(self, Phase::BackingUp | Phase::Waiting | Phase::Failed)
    }
}

/// What the box gave up outside itself, as counts. Kept past the wipe
/// (`/var/lib/losos-erase`) so the next owner of the box, or the same one,
/// can see it went; counts only, because the domain names belonged to the
/// previous owner.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outside {
    /// Whether an edge was configured at all. When false the counts are 0
    /// because there was nothing to do, not because something failed.
    pub had_edge: bool,
    pub domains_removed: u32,
    pub listings_closed: u32,
    /// Taken off the edge's registry.
    pub left_edge: bool,
    /// Steps the edge did not confirm, by name (`domains`, `listings`,
    /// `registry`). The erase goes on regardless: the box is going away, and
    /// the owner sees what is left to tidy by hand.
    #[serde(default)]
    pub problems: Vec<String>,
    /// Unix seconds, set when the erase reached the wipe.
    #[serde(default)]
    pub erased_at: u64,
}

/// One erase, persisted in `state.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Erase {
    pub phase: Phase,
    /// Whether a backup was asked for.
    pub backup: bool,
    pub started_at: u64,
    /// End of the countdown, set when it starts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline: Option<u64>,
    /// The backup job in `backingUp`, the rebuild job in `resetting`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job: Option<String>,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub outside: Outside,
}

// ── Backups and restores ────────────────────────────────────────────────

/// Start `kind`'s unit for a fresh job and record it as running.
fn start_job<L: Losos>(l: &mut L, kind: Kind) -> anyhow::Result<String> {
    let job = l.next_job_id()?;
    let now = l.now();
    let mut s = l.load_state()?;
    s.backup_job = Some(Job {
        kind,
        job: job.clone(),
        state: JobState::Running,
        started_at: now,
        finished_at: None,
        message: String::new(),
        unknown_polls: 0,
    });
    l.save_state(&s)?;
    if let Err(e) = l.start_backup_job(kind, &job) {
        let mut s = l.load_state()?;
        if let Some(j) = s.backup_job.as_mut() {
            j.state = JobState::Failed;
            j.finished_at = Some(now);
            j.message = format!("{} did not start", kind.as_str());
        }
        l.save_state(&s)?;
        return Err(e.context(format!("starting the {} unit", kind.as_str())));
    }
    Ok(job)
}

/// Refuse a new job while anything that conflicts with it is going on.
fn ensure_idle<L: Losos>(l: &mut L) -> anyhow::Result<()> {
    let s = l.load_state()?;
    if s.erase.is_some() {
        return Err(Busy("the box is being erased").into());
    }
    if s.backup_job.as_ref().is_some_and(Job::running) {
        return Err(Busy("a backup or restore is already running").into());
    }
    if s.rebuild
        .as_ref()
        .is_some_and(|r| r.state == RebuildState::Building)
    {
        return Err(Busy("a change is being applied; try again when it is done").into());
    }
    Ok(())
}

/// `GET /api/backup`: the bucket (without its secret), the last backup, the
/// current job, and any erase under way.
pub fn cmd_backup<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    let target = l.backup_target()?;
    let last = l.backup_report()?;
    let s = l.load_state()?;
    let now = l.now();
    Ok(json!({
        "target": target.as_ref().map(crate::backup::Target::public),
        "last": last,
        "job": s.backup_job,
        "erase": s.erase.as_ref().map(|e| erase_view(e, now)),
        "lastErase": l.read_erase_report()?,
        "graceSeconds": l.erase_grace_secs(),
    }))
}

/// `POST /api/backup/target`: set the bucket. The secret may be left out to
/// keep the stored one.
pub fn cmd_backup_target<L: Losos>(
    l: &mut L,
    input: crate::backup::TargetInput,
) -> anyhow::Result<Value> {
    if l.load_state()?
        .backup_job
        .as_ref()
        .is_some_and(Job::running)
    {
        return Err(
            Busy("a backup or restore is running; change the bucket when it is done").into(),
        );
    }
    let stored = l.backup_target()?;
    let target = input.into_target(stored.as_ref())?;
    l.write_backup_target(Some(&target))?;
    Ok(json!({ "target": target.public() }))
}

/// `DELETE /api/backup/target`: forget the bucket. The backups in it stay
/// where they are; only this box stops writing there.
pub fn cmd_backup_target_clear<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    let s = l.load_state()?;
    if s.backup_job.as_ref().is_some_and(Job::running) {
        return Err(
            Busy("a backup or restore is running; change the bucket when it is done").into(),
        );
    }
    if s.erase.as_ref().is_some_and(|e| e.backup) {
        return Err(Busy("the erase under way is backing up to this bucket").into());
    }
    l.write_backup_target(None)?;
    Ok(json!({ "target": null }))
}

/// `POST /api/backup/run`: back up now.
pub fn cmd_backup_run<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    ensure_idle(l)?;
    if l.backup_target()?.is_none() {
        return Err(Invalid("set up a bucket first").into());
    }
    // The repository password is the recovery code; make sure there is one
    // before the script goes looking for it.
    l.recovery_code()?;
    let job = start_job(l, Kind::Backup)?;
    Ok(json!({ "job": job }))
}

/// `POST /api/backup/restore`: pull the latest backup back, opened with the
/// recovery code of the box that made it.
pub fn cmd_restore<L: Losos>(l: &mut L, code: &str) -> anyhow::Result<Value> {
    let Some(code) = crate::backup::normalise_code(code) else {
        return Err(
            Invalid("a recovery code looks like 0f8fad5b-d9cb-469f-a165-70867728950e").into(),
        );
    };
    ensure_idle(l)?;
    if l.backup_target()?.is_none() {
        return Err(Invalid("set up the bucket the backup is in first").into());
    }
    l.write_restore_code(Some(&code))?;
    match start_job(l, Kind::Restore) {
        Ok(job) => Ok(json!({ "job": job })),
        Err(e) => {
            l.write_restore_code(None)?;
            Err(e)
        }
    }
}

/// Follow the running job, if any, to its outcome.
fn poll_job<L: Losos>(l: &mut L) -> anyhow::Result<()> {
    let mut s = l.load_state()?;
    let Some(job) = s.backup_job.as_mut() else {
        return Ok(());
    };
    if !job.running() {
        return Ok(());
    }
    let (kind, id) = (job.kind, job.job.clone());
    let now = l.now();
    let outcome = match l.poll_backup_job(kind, &id) {
        Poll::Wait => {
            if job.unknown_polls != 0 {
                job.unknown_polls = 0;
                l.save_state(&s)?;
            }
            return Ok(());
        }
        // The unit is not visible yet, or systemctl hiccuped: give it the
        // same grace a rebuild gets, counted in the job so a restart of
        // lososd does not reset it.
        Poll::Unknown => {
            job.unknown_polls += 1;
            if job.unknown_polls < MAX_UNKNOWN_POLLS {
                l.save_state(&s)?;
                return Ok(());
            }
            Err("lost track of the job".to_string())
        }
        Poll::Done(0) => Ok(()),
        Poll::Done(code) => {
            let tail = l.backup_log_tail();
            Err(crate::backup::explain_failure(kind, code, &tail))
        }
    };
    let job = s.backup_job.as_mut().expect("checked above");
    job.finished_at = Some(now);
    match outcome {
        Ok(()) => job.state = JobState::Done,
        Err(message) => {
            job.state = JobState::Failed;
            job.message = message;
        }
    }
    let done = job.state == JobState::Done;
    l.save_state(&s)?;
    if kind == Kind::Restore {
        l.write_restore_code(None)?;
        if done {
            apply_restored_settings(l)?;
        }
    }
    Ok(())
}

/// Put back the settings a restore brought with it, through the same gates
/// an Apply passes, and rebuild into them. A backup made without settings,
/// or one whose settings this build does not accept, leaves the box on its
/// current ones and says so on the job.
fn apply_restored_settings<L: Losos>(l: &mut L) -> anyhow::Result<()> {
    let Some(body) = l.take_restored_overrides()? else {
        return Ok(());
    };
    let checked = check_restored(l, &body);
    let code = match checked {
        Ok(code) => code,
        Err(e) => {
            tracing::warn!(error = %e, "the restored settings were not applied");
            let mut s = l.load_state()?;
            if let Some(j) = s.backup_job.as_mut() {
                j.message = "the files are back; the settings in the backup did not fit this version and were left as they are".to_string();
            }
            return l.save_state(&s);
        }
    };
    let before = l.read_overrides()?;
    if before == code {
        return Ok(());
    }
    l.write_overrides(&code)?;
    crate::losos::commit_settings(l, "Restore", &before, &code);
    crate::losos::queue_rebuild(l, MSG_RESTORED)?;
    Ok(())
}

/// The gates an Apply passes, for a body that came out of a backup.
fn check_restored<L: Losos>(l: &mut L, body: &str) -> anyhow::Result<String> {
    let code = crate::overrides::validate_apply(body)
        .map_err(|e| anyhow::anyhow!(e))?
        .to_string();
    if let Some(doc) = l.read_options_doc()? {
        crate::options::check_body(&doc, &code)?;
    }
    Ok(code)
}

// ── The erase ───────────────────────────────────────────────────────────

fn erase_view(e: &Erase, now: u64) -> Value {
    let mut v = serde_json::to_value(e).unwrap_or(Value::Null);
    if let (Some(obj), Some(deadline)) = (v.as_object_mut(), e.deadline) {
        obj.insert(
            "secondsLeft".to_string(),
            json!(deadline.saturating_sub(now)),
        );
    }
    if let Some(obj) = v.as_object_mut() {
        obj.insert("cancellable".to_string(), json!(e.phase.cancellable()));
    }
    v
}

/// `POST /api/erase`: start an erase, with or without a backup first.
pub fn cmd_erase<L: Losos>(l: &mut L, backup: bool) -> anyhow::Result<Value> {
    ensure_idle(l)?;
    let now = l.now();
    let erase = if backup {
        if l.backup_target()?.is_none() {
            return Err(Invalid("set up a bucket first, or erase without a backup").into());
        }
        l.recovery_code()?;
        let job = start_job(l, Kind::Backup)?;
        Erase {
            phase: Phase::BackingUp,
            backup,
            started_at: now,
            deadline: None,
            job: Some(job),
            message: String::new(),
            outside: Outside::default(),
        }
    } else {
        Erase {
            phase: Phase::Waiting,
            backup,
            started_at: now,
            deadline: Some(now + l.erase_grace_secs()),
            job: None,
            message: String::new(),
            outside: Outside::default(),
        }
    };
    let mut s = l.load_state()?;
    s.erase = Some(erase.clone());
    l.save_state(&s)?;
    tracing::warn!(backup, "erase started");
    Ok(json!({ "erase": erase_view(&erase, now) }))
}

/// `POST /api/erase/cancel`: stop an erase that has not reached the edge
/// yet, or dismiss one that failed.
pub fn cmd_erase_cancel<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    let mut s = l.load_state()?;
    let Some(e) = s.erase.clone() else {
        return Ok(json!({ "erase": null }));
    };
    if !e.phase.cancellable() {
        return Err(Busy(
            "the box has already started leaving its edge and can no longer be stopped",
        )
        .into());
    }
    if e.phase == Phase::BackingUp {
        if let Some(job) = s.backup_job.as_mut().filter(|j| j.running()) {
            let (kind, id) = (job.kind, job.job.clone());
            job.state = JobState::Cancelled;
            job.finished_at = Some(l.now());
            if let Err(err) = l.stop_backup_job(kind, &id) {
                tracing::warn!(error = ?err, "could not stop the backup unit");
            }
        }
    }
    s.erase = None;
    l.save_state(&s)?;
    tracing::warn!("erase cancelled");
    Ok(json!({ "erase": null }))
}

/// Give up what the box holds on its edge: domains, listings, the registry
/// entry. Every step is tried whatever the one before it did.
fn leave_edge<L: Losos>(l: &mut L) -> Outside {
    let mut out = Outside::default();
    match l.market_request(&Op::Domains) {
        Ok(Outcome::Reply(view)) => {
            out.had_edge = true;
            let names: Vec<String> = view["domains"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|d| d["domain"].as_str())
                .filter(|d| crate::market::valid_domain(d))
                .map(str::to_string)
                .collect();
            for domain in names {
                match l.market_request(&Op::DomainRemove { domain }) {
                    Ok(Outcome::Reply(_)) => out.domains_removed += 1,
                    _ => push_once(&mut out.problems, "domains"),
                }
            }
        }
        Ok(Outcome::Unavailable) => {}
        Err(_) => push_once(&mut out.problems, "domains"),
    }
    match l.market_request(&Op::Account) {
        Ok(Outcome::Reply(account)) => {
            out.had_edge = true;
            let open: Vec<String> = account["listings"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|x| x["active"].as_bool() == Some(true))
                .filter_map(|x| x["id"].as_str().map(str::to_string))
                .collect();
            for listing_id in open {
                match l.market_request(&Op::Close { listing_id }) {
                    Ok(Outcome::Reply(_)) => out.listings_closed += 1,
                    _ => push_once(&mut out.problems, "listings"),
                }
            }
        }
        Ok(Outcome::Unavailable) => {}
        Err(_) => push_once(&mut out.problems, "listings"),
    }
    match l.market_request(&Op::Deregister) {
        Ok(Outcome::Reply(_)) => {
            out.had_edge = true;
            out.left_edge = true;
        }
        Ok(Outcome::Unavailable) => {}
        Err(_) => {
            out.had_edge = true;
            push_once(&mut out.problems, "registry");
        }
    }
    out
}

fn push_once(v: &mut Vec<String>, what: &str) {
    if !v.iter().any(|x| x == what) {
        v.push(what.to_string());
    }
}

fn save_erase<L: Losos>(l: &mut L, e: &Erase) -> anyhow::Result<()> {
    let mut s = l.load_state()?;
    s.erase = Some(e.clone());
    l.save_state(&s)
}

/// Move an erase one step on, if its time has come.
fn drive_erase<L: Losos>(l: &mut L) -> anyhow::Result<()> {
    let s = l.load_state()?;
    let Some(mut e) = s.erase.clone() else {
        return Ok(());
    };
    let now = l.now();
    match e.phase {
        Phase::BackingUp => {
            let job = s
                .backup_job
                .as_ref()
                .filter(|j| Some(&j.job) == e.job.as_ref());
            match job.map(|j| j.state) {
                Some(JobState::Running) => {}
                Some(JobState::Done) => {
                    e.phase = Phase::Waiting;
                    e.deadline = Some(now + l.erase_grace_secs());
                    save_erase(l, &e)?;
                }
                other => {
                    let why = job.map(|j| j.message.clone()).unwrap_or_default();
                    e.phase = Phase::Failed;
                    e.message = if other == Some(JobState::Cancelled) || why.is_empty() {
                        "the backup did not finish, so nothing was erased".to_string()
                    } else {
                        format!("the backup did not finish, so nothing was erased: {why}")
                    };
                    save_erase(l, &e)?;
                }
            }
        }
        Phase::Waiting => {
            if e.deadline.is_some_and(|d| now >= d) {
                e.phase = Phase::Leaving;
                save_erase(l, &e)?;
                leave_and_reset(l, e)?;
            }
        }
        // lososd stopped partway through leaving: every step is safe to
        // repeat (a removed domain is not listed again), so do it again.
        Phase::Leaving => leave_and_reset(l, e)?,
        Phase::Resetting => {
            let building = s.rebuild.as_ref().is_some_and(|r| {
                Some(&r.job) == e.job.as_ref() && r.state == RebuildState::Building
            });
            if !building {
                // A failed rebuild does not hold the erase back: the data
                // goes either way, and the defaults are already written, so
                // the next rebuild (the nightly one at the latest) reaches
                // them.
                e.phase = Phase::Restarting;
                e.outside.erased_at = now;
                save_erase(l, &e)?;
                if let Err(err) = l.write_erase_report(&e.outside) {
                    tracing::warn!(error = ?err, "could not keep the erase report");
                }
                tracing::warn!("erase: restarting into the wipe");
                if let Err(err) = l.wipe_and_reboot() {
                    e.message =
                        "the box could not restart itself; switch it off and on again to finish"
                            .to_string();
                    save_erase(l, &e)?;
                    return Err(err.context("restarting into the wipe"));
                }
            }
        }
        Phase::Restarting | Phase::Failed => {}
    }
    Ok(())
}

fn leave_and_reset<L: Losos>(l: &mut L, mut e: Erase) -> anyhow::Result<()> {
    e.outside = leave_edge(l);
    e.phase = Phase::Resetting;
    save_erase(l, &e)?;
    let before = l.read_overrides()?;
    l.write_overrides(crate::overrides::DEFAULT_OVERRIDES_NIX)?;
    crate::losos::commit_settings(l, "Erase", &before, crate::overrides::DEFAULT_OVERRIDES_NIX);
    let job = crate::losos::queue_rebuild(l, MSG_ERASE_RESET)?;
    e.job = Some(job);
    save_erase(l, &e)
}

/// One step of everything this module drives: the running job, then the
/// erase. lososd calls it every [`TICK_EVERY`].
pub fn tick<L: Losos>(l: &mut L) -> anyhow::Result<()> {
    poll_job(l)?;
    drive_erase(l)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeLosos;

    const CODE: &str = "0f8fad5b-d9cb-469f-a165-70867728950e";

    fn with_bucket() -> FakeLosos {
        let mut f = FakeLosos::new();
        f.state.claimed = true;
        f.backup_target = Some(
            crate::backup::TargetInput {
                endpoint: "https://s3.example.org".into(),
                bucket: "box-backups".into(),
                access_key_id: "AKIDEXAMPLE".into(),
                secret_access_key: Some("secretEXAMPLE".into()),
                ..Default::default()
            }
            .into_target(None)
            .unwrap(),
        );
        f
    }

    fn phase(f: &FakeLosos) -> Option<Phase> {
        f.state.erase.as_ref().map(|e| e.phase)
    }

    #[test]
    fn a_backup_runs_as_its_own_unit_and_is_followed_to_the_end() {
        let mut f = with_bucket();
        let out = cmd_backup_run(&mut f).unwrap();
        assert_eq!(out["job"], "job-0");
        assert_eq!(f.units_started, vec!["losos-backup-job-0".to_string()]);
        tick(&mut f).unwrap();
        assert!(f.state.backup_job.as_ref().unwrap().running());
        f.unit_poll = Poll::Done(0);
        tick(&mut f).unwrap();
        assert_eq!(f.state.backup_job.as_ref().unwrap().state, JobState::Done);
        // The repository password exists before the script needs it.
        assert!(f.recovery.writes > 0);
    }

    #[test]
    fn no_bucket_no_backup() {
        let mut f = FakeLosos::new();
        let err = cmd_backup_run(&mut f).unwrap_err();
        assert!(err.downcast_ref::<Invalid>().is_some());
        assert!(f.units_started.is_empty());
    }

    #[test]
    fn one_job_at_a_time() {
        let mut f = with_bucket();
        cmd_backup_run(&mut f).unwrap();
        let err = cmd_backup_run(&mut f).unwrap_err();
        assert!(err.downcast_ref::<Busy>().is_some());
        let err = cmd_restore(&mut f, CODE).unwrap_err();
        assert!(err.downcast_ref::<Busy>().is_some());
    }

    #[test]
    fn a_failed_backup_says_why_in_plain_words() {
        let mut f = with_bucket();
        cmd_backup_run(&mut f).unwrap();
        f.backup_log = "Fatal: unable to open config file: Stat: Access Denied.".into();
        f.unit_poll = Poll::Done(1);
        tick(&mut f).unwrap();
        let job = f.state.backup_job.as_ref().unwrap();
        assert_eq!(job.state, JobState::Failed);
        assert_eq!(job.message, "the bucket refused the keys");
    }

    #[test]
    fn a_unit_that_never_shows_up_fails_after_the_grace() {
        let mut f = with_bucket();
        cmd_backup_run(&mut f).unwrap();
        f.unit_poll = Poll::Unknown;
        for _ in 0..MAX_UNKNOWN_POLLS - 1 {
            tick(&mut f).unwrap();
            assert!(f.state.backup_job.as_ref().unwrap().running());
        }
        tick(&mut f).unwrap();
        assert_eq!(f.state.backup_job.as_ref().unwrap().state, JobState::Failed);
    }

    #[test]
    fn a_restore_takes_the_code_by_file_and_applies_the_settings_it_brought() {
        let mut f = with_bucket();
        let err = cmd_restore(&mut f, "nope").unwrap_err();
        assert!(err.downcast_ref::<Invalid>().is_some());
        cmd_restore(&mut f, &CODE.to_uppercase()).unwrap();
        assert_eq!(f.restore_code.as_deref(), Some(CODE));
        assert_eq!(f.units_started, vec!["losos-restore-job-0".to_string()]);
        f.restored_overrides = Some(crate::overrides::DEFAULT_OVERRIDES_NIX.replace(
            "losos.hostName = \"mattbox\";",
            "losos.hostName = \"kitchen\";",
        ));
        f.unit_poll = Poll::Done(0);
        tick(&mut f).unwrap();
        assert_eq!(f.state.backup_job.as_ref().unwrap().state, JobState::Done);
        // The code does not outlive the job.
        assert_eq!(f.restore_code, None);
        // The settings came back through a rebuild.
        assert!(f.config.iter().any(|l| l.contains("\"kitchen\"")));
        assert!(f.spawned);
        assert_eq!(
            f.state.rebuild.as_ref().unwrap().state,
            RebuildState::Building
        );
    }

    #[test]
    fn a_failed_restore_drops_the_code_too() {
        let mut f = with_bucket();
        cmd_restore(&mut f, CODE).unwrap();
        f.backup_log = "Fatal: wrong password or no key found".into();
        f.unit_poll = Poll::Done(1);
        tick(&mut f).unwrap();
        let job = f.state.backup_job.as_ref().unwrap();
        assert_eq!(job.message, "the recovery code does not open this backup");
        assert_eq!(f.restore_code, None);
        assert!(!f.spawned);
    }

    #[test]
    fn the_secret_is_kept_when_the_form_leaves_it_out() {
        let mut f = with_bucket();
        let out = cmd_backup_target(
            &mut f,
            crate::backup::TargetInput {
                endpoint: "https://s3.example.org".into(),
                bucket: "renamed".into(),
                access_key_id: "AKIDEXAMPLE".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out["target"]["bucket"], "renamed");
        assert_eq!(out["target"]["hasSecret"], true);
        assert!(!out.to_string().contains("secretEXAMPLE"));
        assert_eq!(
            f.backup_target.as_ref().unwrap().secret_access_key,
            "secretEXAMPLE"
        );
    }

    #[test]
    fn an_erase_with_a_backup_waits_for_it_then_counts_down() {
        let mut f = with_bucket();
        f.clock = 1000;
        cmd_erase(&mut f, true).unwrap();
        assert_eq!(phase(&f), Some(Phase::BackingUp));
        tick(&mut f).unwrap();
        assert_eq!(phase(&f), Some(Phase::BackingUp));
        f.clock = 1300;
        f.unit_poll = Poll::Done(0);
        tick(&mut f).unwrap();
        assert_eq!(phase(&f), Some(Phase::Waiting));
        // The countdown starts when the backup is safe, not when asked.
        assert_eq!(f.state.erase.as_ref().unwrap().deadline, Some(1300 + 900));
        f.clock = 1300 + 899;
        tick(&mut f).unwrap();
        assert_eq!(phase(&f), Some(Phase::Waiting));
        assert!(
            f.market_ops.is_empty(),
            "nothing outside changes during the countdown"
        );
        assert!(!f.rebooted);
    }

    #[test]
    fn a_failed_backup_erases_nothing() {
        let mut f = with_bucket();
        cmd_erase(&mut f, true).unwrap();
        f.unit_poll = Poll::Done(3);
        tick(&mut f).unwrap();
        assert_eq!(phase(&f), Some(Phase::Failed));
        for _ in 0..5 {
            f.clock += 10_000;
            tick(&mut f).unwrap();
        }
        assert_eq!(phase(&f), Some(Phase::Failed));
        assert!(f.market_ops.is_empty());
        assert!(!f.rebooted && !f.spawned);
        // Dismissing it clears it.
        cmd_erase_cancel(&mut f).unwrap();
        assert_eq!(phase(&f), None);
    }

    #[test]
    fn an_erase_without_a_bucket_must_say_so() {
        let mut f = FakeLosos::new();
        assert!(cmd_erase(&mut f, true)
            .unwrap_err()
            .downcast_ref::<Invalid>()
            .is_some());
        cmd_erase(&mut f, false).unwrap();
        assert_eq!(phase(&f), Some(Phase::Waiting));
    }

    #[test]
    fn cancelling_during_the_backup_stops_the_unit_and_leaves_no_trace() {
        let mut f = with_bucket();
        cmd_erase(&mut f, true).unwrap();
        cmd_erase_cancel(&mut f).unwrap();
        assert_eq!(phase(&f), None);
        assert_eq!(f.units_stopped, vec!["losos-backup-job-0".to_string()]);
        assert_eq!(
            f.state.backup_job.as_ref().unwrap().state,
            JobState::Cancelled
        );
        tick(&mut f).unwrap();
        assert!(f.market_ops.is_empty() && !f.rebooted);
    }

    fn edge_with_one_of_everything(f: &mut FakeLosos) {
        f.market_routes.insert(
            "POST /domains/list".into(),
            (
                200,
                r#"{"eligible":true,"domains":[{"domain":"cloud.example.org"},{"domain":"git.example.org"}]}"#.into(),
            ),
        );
        f.market_routes
            .insert("POST /domains/remove".into(), (200, "{}".into()));
        f.market_routes.insert(
            "POST /market/account".into(),
            (
                200,
                r#"{"listings":[{"id":"lst_aa","active":true},{"id":"lst_bb","active":false}]}"#
                    .into(),
            ),
        );
        f.market_routes
            .insert("POST /market/listings/close".into(), (204, String::new()));
        f.market_routes
            .insert("POST /deregister".into(), (204, String::new()));
    }

    #[test]
    fn after_the_countdown_the_box_leaves_its_edge_resets_and_restarts() {
        let mut f = with_bucket();
        edge_with_one_of_everything(&mut f);
        f.config = vec!["losos.hostName = \"kitchen\";".into()];
        cmd_erase(&mut f, false).unwrap();
        f.clock += 900;
        tick(&mut f).unwrap();
        assert_eq!(phase(&f), Some(Phase::Resetting));
        let e = f.state.erase.clone().unwrap();
        assert!(e.outside.had_edge && e.outside.left_edge);
        assert_eq!(e.outside.domains_removed, 2);
        assert_eq!(e.outside.listings_closed, 1, "only the open listing");
        assert!(e.outside.problems.is_empty());
        // Deregistering comes last, after everything else that names this
        // box on the edge.
        assert_eq!(f.market_ops.last(), Some(&Op::Deregister));
        // The defaults were written and are being built.
        assert_eq!(
            f.config.join("\n").trim(),
            crate::overrides::DEFAULT_OVERRIDES_NIX.trim()
        );
        assert!(f.spawned);
        assert!(!f.rebooted, "not before the rebuild has finished");
        tick(&mut f).unwrap();
        assert!(!f.rebooted);
        f.state.rebuild.as_mut().unwrap().state = RebuildState::Done;
        tick(&mut f).unwrap();
        assert_eq!(phase(&f), Some(Phase::Restarting));
        assert!(f.rebooted);
        assert_eq!(f.erase_report.as_ref().unwrap().domains_removed, 2);
        // And it cannot be stopped any more.
        assert!(cmd_erase_cancel(&mut f)
            .unwrap_err()
            .downcast_ref::<Busy>()
            .is_some());
    }

    #[test]
    fn an_edge_that_does_not_answer_is_noted_not_fatal() {
        let mut f = FakeLosos::new();
        f.market_routes
            .insert("POST /domains/list".into(), (500, String::new()));
        f.market_routes
            .insert("POST /deregister".into(), (502, String::new()));
        cmd_erase(&mut f, false).unwrap();
        f.clock += 900;
        tick(&mut f).unwrap();
        let e = f.state.erase.clone().unwrap();
        assert_eq!(e.phase, Phase::Resetting);
        assert_eq!(e.outside.problems, vec!["domains", "registry"]);
        assert!(!e.outside.left_edge);
    }

    #[test]
    fn a_box_with_no_edge_skips_leaving() {
        let mut f = FakeLosos::new();
        cmd_erase(&mut f, false).unwrap();
        f.clock += 900;
        tick(&mut f).unwrap();
        let e = f.state.erase.clone().unwrap();
        assert!(!e.outside.had_edge);
        assert!(e.outside.problems.is_empty());
    }

    #[test]
    fn a_failed_settings_rebuild_does_not_hold_the_erase_back() {
        let mut f = FakeLosos::new();
        cmd_erase(&mut f, false).unwrap();
        f.clock += 900;
        tick(&mut f).unwrap();
        f.state.rebuild.as_mut().unwrap().state = RebuildState::Failed;
        tick(&mut f).unwrap();
        assert!(f.rebooted);
    }

    #[test]
    fn nothing_starts_while_an_erase_runs() {
        let mut f = with_bucket();
        cmd_erase(&mut f, false).unwrap();
        for err in [
            cmd_erase(&mut f, false).unwrap_err(),
            cmd_backup_run(&mut f).unwrap_err(),
            cmd_restore(&mut f, CODE).unwrap_err(),
        ] {
            assert!(err.downcast_ref::<Busy>().is_some());
        }
    }

    #[test]
    fn the_view_shows_the_countdown_and_hides_the_secret() {
        let mut f = with_bucket();
        f.clock = 50;
        cmd_erase(&mut f, false).unwrap();
        f.clock = 150;
        let v = cmd_backup(&mut f).unwrap();
        assert_eq!(v["erase"]["phase"], "waiting");
        assert_eq!(v["erase"]["secondsLeft"], 800);
        assert_eq!(v["erase"]["cancellable"], true);
        assert_eq!(v["graceSeconds"], 900);
        assert!(!v.to_string().contains("secretEXAMPLE"));
    }
}
