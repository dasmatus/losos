//! The command core: a [`Losos`] effect trait plus the six commands written
//! once against it.
//!
//! Nothing here knows about paths, systemd, D-Bus or HTTP. That is the whole
//! point — `lososd` runs these functions against the real filesystem while the
//! test suite runs the *same* functions against [`crate::fake::FakeLosos`],
//! so the state machine is exercised with no filesystem and no subprocesses.
//!
//! The trait is synchronous. Every method is a small file read/write or a
//! process spawn, and keeping it sync means the fake stays trivially pure and
//! the command layer never acquires an async colour; the async transports call
//! in through `web::block` / `spawn_blocking`.

use crate::model::{Mode, Rebuild, RebuildState, State};
use crate::overrides::{parse_settings, DEFAULT_OVERRIDES_NIX};
use serde_json::{json, Value};

/// Anything the commands need from the outside world.
pub trait Losos {
    /// Current persisted state. Missing or corrupt input yields
    /// [`State::default`] rather than an error.
    fn load_state(&mut self) -> anyhow::Result<State>;
    fn save_state(&mut self, s: &State) -> anyhow::Result<()>;
    /// Replace `overrides.nix` (`LOSOS_OVERRIDES`) wholesale. The one file
    /// the box evaluates for a runtime setting; `change` line-patches it
    /// through this too, after reading it back.
    fn write_overrides(&mut self, body: &str) -> anyhow::Result<()>;
    /// `overrides.nix` as it is on disk, or the committed defaults when the
    /// file is absent (a fresh appliance). A read *error* is an error.
    fn read_overrides(&mut self) -> anyhow::Result<String>;
    /// Start a rebuild for `job` and arrange for its completion to be recorded.
    fn spawn_rebuild(&mut self, job: &str) -> anyhow::Result<()>;
    /// Last non-empty line of the rebuild log, or `""`.
    fn rebuild_log_tail(&mut self) -> anyhow::Result<String>;
    fn next_job_id(&mut self) -> anyhow::Result<String>;

    // ── Online growth of /persist ───────────────────────────────────────
    // Split into "look" and "do" so the ordering of the destructive half is
    // asserted against the plan, the same way the installer does it.
    /// Unallocated extents in `persist-vg`.
    fn vg_free(&mut self) -> anyhow::Result<crate::grow::VgFree>;
    /// Execute one planned step.
    fn run_grow(&mut self, action: &crate::grow::GrowAction) -> anyhow::Result<()>;
    /// Total bytes of the `/persist` filesystem, for before/after reporting.
    fn persist_bytes(&mut self) -> anyhow::Result<u64>;
    /// Bytes of the `/persist` filesystem in use, for the Storage pane.
    fn persist_used_bytes(&mut self) -> anyhow::Result<u64>;
    /// Key file `cryptsetup resize` should authenticate with, if any.
    /// `None` means "rely on the volume key being in the kernel keyring",
    /// which is the TPM path; the keyfile path must supply one or the
    /// resize prompts on a stdin the daemon does not have.
    fn luks_key_file(&mut self) -> anyhow::Result<Option<String>>;

    // ── The appliance recovery code ─────────────────────────────────────
    /// The appliance's recovery code, minting one only if there is none.
    ///
    /// One method rather than the look/plan/do split above because there is no
    /// destructive ordering to assert: the whole of the decision is
    /// [`crate::recovery::plan_ensure`], which is pure and tested on its own.
    /// What this method carries is the *idempotence* — every call after the
    /// first must return the same code and write nothing.
    fn recovery_code(&mut self) -> anyhow::Result<crate::recovery::Recovery>;

    // ── Searching the app catalogue ─────────────────────────────────────
    /// Rows matching `query`, from the catalogue in [`crate::catalogue`].
    ///
    /// The query arrives validated — [`cmd_apps_search`] refuses a bad one
    /// before this is reached — so an `Err` here means the search could not be
    /// made or did not come back: no route off the box, a catalogue that is
    /// down, a body that is not JSON. All three are worth retrying, which is
    /// why the screen distinguishes them from "this box does not serve the
    /// route" (a 404) and offers the field again.
    fn search_apps(&mut self, query: &str) -> anyhow::Result<Vec<crate::catalogue::App>>;

    // ── Installing an app from the catalogue ────────────────────────────
    // `crate::apps` holds the rules; these are the effects.
    /// `None` when this box runs no local cluster (both apps native) or the
    /// build carries no install job: nothing can be installed then.
    fn apps_config(&mut self) -> Option<crate::apps::Config>;
    /// The chart's `values.yaml` and schema, fetched by the box. An `Err` is
    /// a chart that could not be fetched, which the dialog offers to retry.
    fn chart_files(
        &mut self,
        chart: &crate::apps::ChartRef,
    ) -> anyhow::Result<crate::apps::ChartFiles>;
    /// Every installed app's record, by name.
    fn load_app_records(&mut self) -> anyhow::Result<Vec<crate::apps::Record>>;
    fn save_app_record(&mut self, record: &crate::apps::Record) -> anyhow::Result<()>;
    /// A Compose app's file and the files beside it, where the job reads
    /// them (`<apps dir>/<release>/source/`), replacing what was there.
    fn save_app_source(
        &mut self,
        release: &str,
        compose: &str,
        files: &[(String, String)],
    ) -> anyhow::Result<()>;
    /// Start the job that installs or removes `release`, from its record.
    fn start_app_job(&mut self, release: &str, action: crate::apps::Action) -> anyhow::Result<()>;
    /// Whether a job for `release` is running.
    fn app_job_active(&mut self, release: &str) -> bool;

    // ── The market ──────────────────────────────────────────────────────
    /// Relay one [`crate::market::Op`] to the edge's market, with this
    /// appliance's credentials. `Ok(Unavailable)` when there is no registrar
    /// configured or the edge does not offer the market to this box; a
    /// [`crate::market::Refused`] error when the owner can act on the answer;
    /// any other error is a fault for the journal.
    fn market_request(&mut self, op: &crate::market::Op) -> anyhow::Result<crate::market::Outcome>;

    /// This box's escrow keys, from the `market-escrow-key` credential, or
    /// `None` on a box that has none (an older build, a test).
    fn escrow_keys(&mut self) -> anyhow::Result<Option<crate::escrow_key::Keys>>;

    /// The edge this box trades through and its id there, or `None` with no
    /// master proxy configured.
    fn market_config(&mut self) -> Option<crate::market::Config> {
        crate::market::Config::from_env()
    }

    // ── Finding an edge proxy ───────────────────────────────────────────
    /// What the last scan for an edge proxy found (`crate::edge`). The real
    /// daemon answers from its scanner's cache; the fake answers what a test
    /// put there. Never an error for "nothing found": that is a status.
    fn edge_status(&mut self) -> anyhow::Result<crate::edge::EdgeStatus>;
    /// Make `cluster` the secondary cluster this box belongs to, or (`None`)
    /// let it settle anew, and choose its path again. Returns the status
    /// with the new path.
    fn set_edge_cluster(
        &mut self,
        cluster: Option<&str>,
    ) -> anyhow::Result<crate::edge::EdgeStatus>;
    /// The box's own TLS on its public names (`crate::edge::PublicTls`):
    /// what modules/public-tls.nix wrote. Absent files are a status.
    fn public_tls(&mut self) -> anyhow::Result<crate::edge::PublicTls>;

    /// Record the custom domains that are live for this box on its edge, for
    /// LosOS Cloud to trust (`trusted_domains`) and to build its links from
    /// when a request arrives on one. The edge verified each of them; this is
    /// only the box learning the answer. Sorted, lowercase, deduplicated.
    fn write_public_names(&mut self, names: &[String]) -> anyhow::Result<()>;
    /// Keep the relay pass the official edge issued (`None`: forget it), for
    /// the announce loop to hand a local edge (edge/registrar's
    /// `routes.rs`). Root-only: it lets a local edge route this box's
    /// custom domains for a few hours.
    fn write_relay_pass(&mut self, pass: Option<&str>) -> anyhow::Result<()>;
    // ── The owner's own apps ────────────────────────────────────────────
    // `crate::user_apps`: the list of apps beside `state.json`, and each
    // app's data in a document of its own.
    /// The apps, or an empty list when there is none yet or it does not
    /// parse.
    fn load_user_apps(&mut self) -> anyhow::Result<crate::user_apps::UserApps>;
    fn save_user_apps(&mut self, apps: &crate::user_apps::UserApps) -> anyhow::Result<()>;
    /// One app's data, or nothing kept yet.
    fn load_app_data(&mut self, id: &str) -> anyhow::Result<crate::user_apps::AppData>;
    fn save_app_data(&mut self, id: &str, data: &crate::user_apps::AppData) -> anyhow::Result<()>;
    /// Delete one app's data. Not an error when there is none.
    fn remove_app_data(&mut self, id: &str) -> anyhow::Result<()>;
    // `crate::app_repo`: each app's versions in a bare repository of its
    // own beside the list, kept in step with LosOS Git.
    /// The commit the app's branch names, `None` before the first.
    fn app_repo_head(&mut self, id: &str) -> anyhow::Result<Option<String>>;
    /// Commit `files` (and nothing else) as the next version on the app's
    /// branch, making the repository on first use. `None` when the head
    /// already holds exactly these files.
    fn app_repo_commit(
        &mut self,
        id: &str,
        files: &[(&str, String)],
        subject: &str,
        body: &str,
    ) -> anyhow::Result<Option<String>>;
    /// Fetch the branch from `url`; its commit, or `None` when the remote
    /// has no such branch yet. [`crate::config_repo::NotUp`] when the remote
    /// cannot be reached.
    fn app_repo_fetch(&mut self, id: &str, url: &str) -> anyhow::Result<Option<String>>;
    /// Whether `ancestor` is `of` or an ancestor of it.
    fn app_repo_is_ancestor(&mut self, id: &str, ancestor: &str, of: &str) -> anyhow::Result<bool>;
    /// One file of a fetched commit, `None` when it has no such file.
    fn app_repo_show(&mut self, id: &str, rev: &str, path: &str) -> anyhow::Result<Option<String>>;
    /// Move the app's branch to a fetched commit.
    fn app_repo_set_head(&mut self, id: &str, sha: &str) -> anyhow::Result<()>;
    /// Push the app's branch to `url`.
    fn app_repo_push(&mut self, id: &str, url: &str) -> anyhow::Result<()>;
    /// Delete the app's repository beside the list. Not an error when there
    /// is none.
    fn app_repo_remove(&mut self, id: &str) -> anyhow::Result<()>;

    // ── The pin picker ──────────────────────────────────────────────────
    // `crate::pins`: what the picker offers. The pins themselves are the
    // pins plugin's.
    /// One request to LosOS Cloud over its socket
    /// ([`crate::pins::request_bytes`]): its status and body. `Err` when it
    /// could not be asked at all.
    fn cloud_request(
        &mut self,
        request: &crate::pins::CloudRequest<'_>,
    ) -> anyhow::Result<(u16, Vec<u8>)>;

    // ── The option document and the configuration repository ───────────
    // `crate::options` is the gate every Apply goes through; `crate::
    // config_repo` is the git history every change lands in. The git and
    // LosOS Git effects are one method each, named by what they do, so the
    // sync in [`cmd_config_sync`] reads as the plan it is and the fake can
    // model a two-sided history in a few lines.
    /// The document `modules/config-repo.nix` wrote, or `None` when this
    /// box has none (an older build). A document that is there but
    /// unreadable is an error: the gate must not silently vanish.
    fn read_options_doc(&mut self) -> anyhow::Result<Option<crate::options::OptionsDoc>>;
    /// Where the configuration repository is published, or `None` when it
    /// is not (the box still commits locally).
    fn config_repo(&mut self) -> Option<crate::config_repo::RepoConfig>;
    /// The branch head of `/etc/nixos`, or `None` when nothing is committed.
    fn config_head(&mut self) -> anyhow::Result<Option<crate::config_repo::Head>>;
    /// Stage everything and commit. `None` when the tree was clean.
    fn config_commit(&mut self, subject: &str, body: &str) -> anyhow::Result<Option<String>>;
    /// The remote branch's head after fetching it, `None` when the remote
    /// has no such branch yet. A remote that cannot be reached is a
    /// [`crate::config_repo::NotUp`] error.
    fn config_fetch(&mut self, url: &str, branch: &str) -> anyhow::Result<Option<String>>;
    /// Whether `ancestor` is in `of`'s history.
    fn config_is_ancestor(&mut self, ancestor: &str, of: &str) -> anyhow::Result<bool>;
    /// Move the branch and the working tree to `to`, fast-forward only.
    fn config_fast_forward(&mut self, to: &str) -> anyhow::Result<()>;
    fn config_push(&mut self, url: &str, branch: &str) -> anyhow::Result<()>;
    /// A file as it is at `rev`, or `None` when that commit has no such file.
    fn config_show(&mut self, rev: &str, path: &str) -> anyhow::Result<Option<String>>;
    /// The last `n` commits, newest first.
    fn config_log(&mut self, n: usize) -> anyhow::Result<Vec<crate::config_repo::LogEntry>>;
    /// One request to LosOS Git's API as its administrator: status and
    /// body. [`crate::config_repo::NotUp`] when LosOS Git cannot be asked at
    /// all.
    fn git_request(&mut self, op: &crate::config_repo::GitOp) -> anyhow::Result<(u16, String)>;
    fn load_sync_report(&mut self) -> anyhow::Result<Option<crate::config_repo::SyncReport>>;
    fn save_sync_report(&mut self, report: &crate::config_repo::SyncReport) -> anyhow::Result<()>;
    /// Where `/etc/nixos` follows the published tree from, `None` when it
    /// does not (`crate::config_repo::Upstream`).
    fn upstream(&mut self) -> Option<crate::config_repo::Upstream>;
    /// The published branch's head after a shallow fetch. A source that
    /// cannot be reached is a [`crate::config_repo::NotUp`] error.
    fn upstream_fetch(&mut self, up: &crate::config_repo::Upstream) -> anyhow::Result<String>;
    /// What the last follow recorded, `None` before the first.
    fn upstream_mark(&mut self) -> anyhow::Result<Option<crate::config_repo::FollowMark>>;
    /// When the main a fetched or marked published commit was cut from was
    /// made ([`crate::config_repo::base_of`]), `None` when the repository
    /// does not hold that commit.
    fn upstream_base(&mut self, sha: &str) -> anyhow::Result<Option<i64>>;
    /// Whether the head's tree differs from `base`'s outside
    /// [`crate::config_repo::BOX_FILES`]. `None` compares with the first
    /// commit, the tree the installer laid down.
    fn config_edited_outside(&mut self, base: Option<&str>) -> anyhow::Result<bool>;
    /// Commit `upstream`'s tree with the box's own copies of
    /// [`crate::config_repo::BOX_FILES`] on top of the head, fast-forward to
    /// it and record the mark. `None` when the tree came out the same (the
    /// mark is recorded either way).
    fn config_follow(
        &mut self,
        upstream: &str,
        subject: &str,
        body: &str,
    ) -> anyhow::Result<Option<String>>;
    fn load_upstream_report(&mut self) -> anyhow::Result<Option<crate::config_repo::SyncReport>>;
    fn save_upstream_report(
        &mut self,
        report: &crate::config_repo::SyncReport,
    ) -> anyhow::Result<()>;

    // ── Backups and erasing the box (`crate::backup`, `crate::erase`) ───
    // The copying is done by scripts in transient units; these are the
    // handles lososd holds them by, and the few files it reads back.
    /// The bucket backups go to, or `None` when none is set.
    fn backup_target(&mut self) -> anyhow::Result<Option<crate::backup::Target>>;
    /// Keep the bucket (0600, with the units' environment file beside it),
    /// or with `None` forget it.
    fn write_backup_target(&mut self, target: Option<&crate::backup::Target>)
        -> anyhow::Result<()>;
    /// Start the transient unit that runs `kind`'s script for `job`.
    fn start_backup_job(&mut self, kind: crate::backup::Kind, job: &str) -> anyhow::Result<()>;
    /// What that unit is doing.
    fn poll_backup_job(&mut self, kind: crate::backup::Kind, job: &str) -> crate::supervisor::Poll;
    /// Stop it, for a cancelled erase.
    fn stop_backup_job(&mut self, kind: crate::backup::Kind, job: &str) -> anyhow::Result<()>;
    /// Last non-empty line of the backup and restore log, or `""`.
    fn backup_log_tail(&mut self) -> String;
    /// What the last successful backup recorded, if there was one.
    fn backup_report(&mut self) -> anyhow::Result<Option<crate::backup::Report>>;
    /// Hand the restore unit the recovery code that opens the backup, in a
    /// 0600 file on `/run`; `None` removes it.
    fn write_restore_code(&mut self, code: Option<&str>) -> anyhow::Result<()>;
    /// The `overrides.nix` a finished restore staged, taken (the staged copy
    /// is removed), or `None` when there is none.
    fn take_restored_overrides(&mut self) -> anyhow::Result<Option<String>>;
    /// Unix seconds.
    fn now(&mut self) -> u64;
    /// How long an erase counts down before it starts changing anything
    /// (`losos.reset.graceMinutes`).
    fn erase_grace_secs(&mut self) -> u64;
    /// Leave the marker the boot-time wipe looks for, and reboot.
    fn wipe_and_reboot(&mut self) -> anyhow::Result<()>;
    /// Keep what the last erase gave up outside the box where the wipe leaves
    /// it, and read it back.
    fn write_erase_report(&mut self, report: &crate::erase::Outside) -> anyhow::Result<()>;
    fn read_erase_report(&mut self) -> anyhow::Result<Option<crate::erase::Outside>>;
}

/// Message stamped on a rebuild the moment it is queued.
const MSG_STARTED: &str = "rebuild started";
/// The factory-reset variant, kept distinct so the UI can tell them apart.
const MSG_RESET_STARTED: &str = "factory reset: rebuild started";

/// A freshly queued rebuild record.
fn building(job: &str, message: &str) -> Rebuild {
    Rebuild {
        job: job.to_string(),
        state: RebuildState::Building,
        progress: 0,
        message: message.to_string(),
    }
}

/// Current mode and sharing flag.
///
/// Deliberately omits `rebuild`, even though the persisted state carries it —
/// `status` is the endpoint for that.
pub fn cmd_state<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    let s = l.load_state()?;
    Ok(json!({ "mode": s.mode.as_str(), "sharing": s.sharing }))
}

/// The user-tunable `losos.*` options, parsed out of `overrides.nix`.
pub fn cmd_settings<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    let content = l.read_overrides()?;
    Ok(parse_settings(&content).to_json())
}

/// Switch sharing posture and rebuild.
///
/// `sharing` is derived from the mode and is never passed in separately.
///
/// This patches the `losos.sharingMyStorage` line of `overrides.nix` — the
/// same file `apply` writes whole and the admin UI's Storage toggle goes
/// through — and leaves every other line alone. It used to patch
/// `/etc/nixos/defaults.nix`, a path that exists on no installed box (the
/// module is `modules/defaults.nix`, and the option is not assigned there
/// anyway), and a missing file was logged as a warning and reported as
/// success: the CLI and the D-Bus method recorded `mode: mesh` in state.json,
/// spawned a rebuild that changed nothing, and answered OK. Only the web UI,
/// which never called this, actually moved the box. Now all three roads
/// write the one file the flake evaluates.
pub fn cmd_change<L: Losos>(l: &mut L, mode: Mode) -> anyhow::Result<Value> {
    let sharing = mode == Mode::Mesh;
    // The gate (`crate::edge`): mesh mode shares this box's storage through
    // an edge, so asking for it with no edge in reach is refused with the
    // reason, before anything is written. Unconditional, unlike `apply`'s
    // transition rule: `change --mode mesh` *is* the request to share, not a
    // document that happens to carry the setting. Going local is never gated.
    if sharing && !l.edge_status()?.reachable {
        return Err(crate::edge::EdgeRequired {
            setting: "losos.sharingMyStorage",
        }
        .into());
    }
    // The owner chose: a pause the box made is theirs to keep or undo now,
    // and turning sharing on just passed the gate, so an edge answered.
    let (job, commit) = set_sharing(l, sharing, "Change", |s| {
        s.sharing_paused_at = None;
        s.sharing_edge_seen = sharing;
    })?;
    Ok(json!({ "job": job, "commit": commit }))
}

/// Write `losos.sharingMyStorage`, record the mode, and rebuild: the one
/// road for the owner's mode change and the box's own pause and resume.
/// `mark` adjusts the state saved with the rebuild.
fn set_sharing<L: Losos>(
    l: &mut L,
    sharing: bool,
    title: &str,
    mark: impl FnOnce(&mut State),
) -> anyhow::Result<(String, Value)> {
    let before = l.read_overrides()?;
    let lines: Vec<String> = before.lines().map(str::to_string).collect();
    let mut body = crate::overrides::inject_line(sharing, &lines).join("\n");
    body.push('\n');
    l.write_overrides(&body)?;
    let commit = commit_settings(l, title, &before, &body);
    let job = l.next_job_id()?;
    let mut s = l.load_state()?;
    s.mode = if sharing { Mode::Mesh } else { Mode::Local };
    s.sharing = sharing;
    s.rebuild = Some(building(&job, MSG_STARTED));
    mark(&mut s);
    l.save_state(&s)?;
    l.spawn_rebuild(&job)?;
    Ok((job, commit))
}

/// After each edge scan: pause sharing when no edge has answered for
/// `pause_after` seconds, and share again once one is back
/// (`crate::edge::sharing_step` is the rule). Returns what it did, for the
/// journal. `clock` must already hold this scan.
///
/// # Errors
/// Reading the state or the settings, or queueing the rebuild, failed.
pub fn sharing_tick<L: Losos>(
    l: &mut L,
    clock: &crate::edge::OutageClock,
    now: u64,
    pause_after: u64,
) -> anyhow::Result<crate::edge::SharingStep> {
    use crate::edge::SharingStep;
    let s = l.load_state()?;
    // An erase owns the settings until it is done. And a box nobody has
    // claimed yet has no state file, which is what keeps the claim route
    // open (`State::claimed`): writing one here, when its default sharing
    // first sees an edge, locked the owner out of their own new box.
    if s.erase.is_some() || !s.claimed {
        return Ok(SharingStep::Nothing);
    }
    let sharing = crate::edge::SharingNow {
        on: parse_settings(&l.read_overrides()?).sharing_my_storage,
        paused: s.sharing_paused_at.is_some(),
        edge_seen: s.sharing_edge_seen,
        building: s
            .rebuild
            .as_ref()
            .is_some_and(|rb| rb.state == RebuildState::Building),
    };
    let step = crate::edge::sharing_step(sharing, clock, now, pause_after);
    match step {
        SharingStep::Nothing => {}
        SharingStep::SeenEdge => {
            let mut s = s;
            s.sharing_edge_seen = true;
            l.save_state(&s)?;
        }
        SharingStep::Forget => {
            let mut s = s;
            s.sharing_paused_at = None;
            l.save_state(&s)?;
        }
        SharingStep::Pause => {
            set_sharing(l, false, "Pause sharing: no edge answers", |s| {
                s.sharing_paused_at = Some(now);
            })?;
        }
        SharingStep::Resume => {
            set_sharing(l, true, "Resume sharing: an edge answers again", |s| {
                s.sharing_paused_at = None;
                s.sharing_edge_seen = true;
            })?;
        }
    }
    Ok(step)
}

/// [`sharing_tick`] for `losos.sharingLocally`, the `kindashared` side:
/// pause when no local gateway has answered for `pause_after` seconds, share
/// again once one is back. `clock` observes [`crate::edge::EdgeStatus::
/// local_gateway`], not plain reachability: an official edge answering does
/// not keep this kind of sharing on.
///
/// # Errors
/// Reading the state or the settings, or queueing the rebuild, failed.
pub fn local_sharing_tick<L: Losos>(
    l: &mut L,
    clock: &crate::edge::OutageClock,
    now: u64,
    pause_after: u64,
) -> anyhow::Result<crate::edge::SharingStep> {
    use crate::edge::SharingStep;
    let s = l.load_state()?;
    if s.erase.is_some() || !s.claimed {
        return Ok(SharingStep::Nothing);
    }
    let sharing = crate::edge::SharingNow {
        on: parse_settings(&l.read_overrides()?).sharing_locally,
        paused: s.local_sharing_paused_at.is_some(),
        edge_seen: s.local_sharing_edge_seen,
        building: s
            .rebuild
            .as_ref()
            .is_some_and(|rb| rb.state == RebuildState::Building),
    };
    let step = crate::edge::sharing_step(sharing, clock, now, pause_after);
    match step {
        SharingStep::Nothing => {}
        SharingStep::SeenEdge => {
            let mut s = s;
            s.local_sharing_edge_seen = true;
            l.save_state(&s)?;
        }
        SharingStep::Forget => {
            let mut s = s;
            s.local_sharing_paused_at = None;
            l.save_state(&s)?;
        }
        SharingStep::Pause => {
            set_local_sharing(
                l,
                false,
                "Pause local sharing: no local gateway answers",
                |s| {
                    s.local_sharing_paused_at = Some(now);
                },
            )?;
        }
        SharingStep::Resume => {
            set_local_sharing(
                l,
                true,
                "Resume local sharing: a local gateway answers again",
                |s| {
                    s.local_sharing_paused_at = None;
                    s.local_sharing_edge_seen = true;
                },
            )?;
        }
    }
    Ok(step)
}

/// Write `losos.sharingLocally` and rebuild: the box's own pause and resume
/// of sharing with its local gateways. Unlike [`set_sharing`] it leaves
/// `mode`, which is about the mesh, alone.
fn set_local_sharing<L: Losos>(
    l: &mut L,
    on: bool,
    title: &str,
    mark: impl FnOnce(&mut State),
) -> anyhow::Result<(String, Value)> {
    let before = l.read_overrides()?;
    let lines: Vec<String> = before.lines().map(str::to_string).collect();
    let mut body = crate::overrides::inject_bool("sharingLocally", on, &lines).join("\n");
    body.push('\n');
    l.write_overrides(&body)?;
    let commit = commit_settings(l, title, &before, &body);
    let job = l.next_job_id()?;
    let mut s = l.load_state()?;
    s.rebuild = Some(building(&job, MSG_STARTED));
    mark(&mut s);
    l.save_state(&s)?;
    l.spawn_rebuild(&job)?;
    Ok((job, commit))
}

/// Record a change to `overrides.nix` in the configuration repository
/// (`crate::config_repo`), with a message naming the settings that moved.
///
/// Best effort, on purpose: the change is already on disk and the rebuild
/// that makes it real is queued next, so a commit that fails — git missing
/// from the unit path, a repository the installer never made — is a line in
/// the journal and a `null` in the reply, not a refused Apply. The sync
/// (`cmd_config_sync`) commits whatever is uncommitted before it pushes, so
/// the history catches up on the next run.
pub(crate) fn commit_settings<L: Losos>(
    l: &mut L,
    title: &str,
    before: &str,
    after: &str,
) -> Value {
    let changes = crate::overrides::describe_changes(before, after);
    let (subject, body) = crate::overrides::commit_message(title, &changes);
    match l.config_commit(&subject, &body) {
        Ok(Some(sha)) => Value::String(sha),
        Ok(None) => Value::Null,
        Err(e) => {
            tracing::warn!(
                error = ?e,
                "the change is applied but could not be committed to the configuration repository"
            );
            Value::Null
        }
    }
}

/// Queue a rebuild: a fresh job id, `building` in the state, the unit.
pub(crate) fn queue_rebuild<L: Losos>(l: &mut L, message: &str) -> anyhow::Result<String> {
    let job = l.next_job_id()?;
    let mut s = l.load_state()?;
    s.rebuild = Some(building(&job, message));
    l.save_state(&s)?;
    l.spawn_rebuild(&job)?;
    Ok(job)
}

/// Schizo mode was turned on: give the box its Tor address
/// (`losos.hidden.tor.enable`, modules/hidden-services.nix), so the owner's
/// computers, whose connections all go through Tor now, can reach it there.
/// `None` when it already has one, or when a rebuild is under way, which
/// this does not queue behind: the owner then turns it on in Network.
pub fn cmd_schizo_tor<L: Losos>(l: &mut L) -> anyhow::Result<Option<String>> {
    let s = l.load_state()?;
    if s.erase.is_some()
        || s.rebuild
            .as_ref()
            .is_some_and(|rb| rb.state == RebuildState::Building)
    {
        return Ok(None);
    }
    let before = l.read_overrides()?;
    let Some(body) = crate::schizo::with_tor(&before) else {
        return Ok(None);
    };
    l.write_overrides(&body)?;
    commit_settings(l, "Schizo mode: a Tor address", &before, &body);
    queue_rebuild(l, MSG_STARTED).map(Some)
}

/// Overwrite `overrides.nix` with an already-validated body and rebuild.
///
/// Mode and sharing are left exactly as they were: applying settings is not a
/// posture change.
///
/// "Already validated" means [`crate::overrides::validate_apply`], which every
/// transport runs first. The second gate is here, so it travels with the
/// command: when the box carries its option document
/// (`crate::options`), every `losos.<key>` line is checked against the
/// declared type and refused by name when it does not fit — typed as
/// [`crate::options::Rejected`], which the HTTP layer answers as a 400 with
/// the sentence. A box without the document (an older build) keeps the
/// first gate only.
///
/// The body it replaces is read first, so the commit can say what changed.
pub fn cmd_apply<L: Losos>(l: &mut L, nix_code: &str) -> anyhow::Result<Value> {
    if let Some(doc) = l.read_options_doc()? {
        crate::options::check_body(&doc, nix_code)?;
    }
    let before = l.read_overrides()?;
    // The gate (`crate::edge`): an apply that turns storage sharing or the
    // mesh join on needs an edge in reach. Settings already on, and anything
    // that is not sharing, pass whatever the network looks like.
    let now = parse_settings(&before);
    let want = parse_settings(nix_code);
    let edge = l.edge_status()?;
    crate::edge::check_gate(&now, &want, &edge)?;
    // Sharing as `kindashared` needs more than any edge: a local gateway.
    crate::edge::check_local_gate(&now, &want, &edge)?;
    crate::edge::check_mesh(nix_code)?;
    let body = crate::overrides::drop_retired(nix_code);
    l.write_overrides(&body)?;
    let commit = commit_settings(l, "Change", &before, &body);
    if now.sharing_my_storage != want.sharing_my_storage {
        // The owner flipped sharing: as in `cmd_change`, a pause the box
        // made is forgotten.
        let mut s = l.load_state()?;
        s.sharing_paused_at = None;
        s.sharing_edge_seen = want.sharing_my_storage;
        l.save_state(&s)?;
    }
    if now.sharing_locally != want.sharing_locally {
        // The same for sharing with the local gateways: the gate above just
        // saw one answer if it was turned on.
        let mut s = l.load_state()?;
        s.local_sharing_paused_at = None;
        s.local_sharing_edge_seen = want.sharing_locally;
        l.save_state(&s)?;
    }
    let job = queue_rebuild(l, MSG_STARTED)?;
    Ok(json!({ "job": job, "commit": commit }))
}

/// Soft factory reset: restore the committed defaults and rebuild.
///
/// Writes [`State::default`] rather than editing the existing state, with one
/// field carried across: `claimed`. Resetting the settings an owner chose is
/// not the same as forgetting that the box has an owner, and the claim route is
/// unauthenticated precisely while `claimed` is false — see [`cmd_claim`]. The
/// destructive tier (wipe the disks, and with them state.json) is the installer
/// ISO, not this.
pub fn cmd_factory_reset<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    // An erase (`crate::erase`) resets the settings itself, at its own step;
    // a second reset racing it would queue a rebuild the erase is not
    // watching.
    let current = l.load_state()?;
    if current.erase.is_some() {
        return Err(crate::backup::Busy("the box is being erased").into());
    }
    let before = l.read_overrides()?;
    l.write_overrides(DEFAULT_OVERRIDES_NIX)?;
    let commit = commit_settings(l, "Reset", &before, DEFAULT_OVERRIDES_NIX);
    let job = l.next_job_id()?;
    // `claimed` is carried over rather than reset, and this is the one field of
    // State that a factory reset must not touch. Resetting the settings an
    // owner chose is not the same as forgetting that the box has an owner: the
    // claim route is unauthenticated while `claimed` is false, so re-opening it
    // here would mean a reset performed from the admin UI hands the next person
    // on that LAN an unguarded password prompt. The destructive tier that truly
    // makes the box unowned is the installer ISO, which wipes /persist and takes
    // state.json with it.
    //
    // The backup job record is carried over too: it is not a setting, and a
    // backup that is running when the settings are reset keeps running.
    let claimed = current.claimed;
    let s = State {
        rebuild: Some(building(&job, MSG_RESET_STARTED)),
        claimed,
        backup_job: current.backup_job,
        owner: current.owner,
        ..State::default()
    };
    l.save_state(&s)?;
    l.spawn_rebuild(&job)?;
    Ok(json!({ "job": job, "reset": true, "commit": commit }))
}

/// Extend `/persist` into the volume group's free extents, online.
///
/// Reports measured before/after sizes rather than "ok", because every way
/// this goes wrong goes wrong *quietly*: `resize2fs` run against a mapping
/// that has not been resized prints "Nothing to do!" and exits 0. `grew` is
/// the only field worth trusting.
pub fn cmd_grow<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    let vg = l.vg_free()?;
    let key_file = l.luks_key_file()?;
    let plan = crate::grow::plan_grow(vg, key_file.as_deref()).map_err(|e| anyhow::anyhow!(e))?;
    let before = l.persist_bytes()?;
    for action in &plan {
        l.run_grow(action)?;
    }
    let after = l.persist_bytes()?;
    Ok(json!({
        "grew": after > before,
        "beforeBytes": before,
        "afterBytes": after,
        "claimedBytes": vg.free_bytes(),
    }))
}

/// What the Storage pane shows: the size of `/persist`, how much of it is in
/// use, and the reserve a grow would claim.
///
/// Read-only, and it is what keeps "Use reserve" honest after a reload: the
/// pane used to learn the reserve only from a grow's reply, so a fresh tab
/// offered to claim space that was already claimed. Each reading stands
/// alone; one the box cannot take is `null`, never a guess, so a box without
/// the LVM layout still reports its filesystem.
pub fn cmd_storage<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    fn reading(what: &str, r: anyhow::Result<u64>) -> Value {
        match r {
            Ok(n) => json!(n),
            Err(e) => {
                tracing::warn!(error = ?e, "storage: no {what} reading");
                Value::Null
            }
        }
    }
    let total = l.persist_bytes();
    let used = l.persist_used_bytes();
    let reserve = l.vg_free().map(crate::grow::VgFree::free_bytes);
    Ok(json!({
        "totalBytes": reading("size", total),
        "usedBytes": reading("usage", used),
        "reserveBytes": reading("reserve", reserve),
    }))
}

/// The owner's new password, checked.
///
/// The people file is written by the HTTP layer, which holds it
/// (`crate::people`), and no app keeps a copy of its own: LosOS Cloud and
/// LosOS Git take the person from LosOS Login. Both validations run before
/// the HTTP layer writes anything.
pub fn cmd_set_password<L: Losos>(_l: &mut L, user: &str, password: &str) -> anyhow::Result<Value> {
    use crate::setup::{validate_password, validate_user};

    let user = validate_user(user).map_err(|e| anyhow::anyhow!(e))?;
    validate_password(password).map_err(|e| anyhow::anyhow!(e))?;
    Ok(json!({ "user": user, "changed": true }))
}

/// Whether this box has an owner yet. Unauthenticated, on purpose.
///
/// The admin UI asks this before it decides what to show, so it has to answer
/// without a token — the whole point is that on an unclaimed box nobody has
/// one. It reveals a single bit about a machine the caller has already reached
/// on the LAN, and `modules/containers.nix`'s `lanOnly` guard is what keeps
/// "on the LAN" meaningful.
///
/// `ready` and `waitingFor` are kept for the wizard, which waits while
/// `ready` is false. Nothing has to start before the first password any
/// more, so an unclaimed box is always ready.
pub fn cmd_claim_state<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    let claimed = l.load_state().map(|s| s.claimed).unwrap_or(false);
    if claimed {
        return Ok(json!({ "claimed": true, "ready": true, "waitingFor": Value::Null }));
    }
    Ok(json!({ "claimed": false, "ready": true, "waitingFor": Value::Null }))
}

/// Claim an unowned box: set the first password, and record that it has an
/// owner. One shot, and unauthenticated **only while unclaimed**.
///
/// This exists because the alternative did not work at all. `losos.admin.
/// tokenFile` is 64 random hex characters minted by lososd into a 0600 file on
/// first start, and this appliance has no SSH and no shell logins — so the
/// admin key is unreadable by the person who just unboxed the machine. The UI
/// that asked them to "paste the admin key, it is printed on the box" was
/// describing a sticker nothing in this tree prints. A fresh appliance was
/// therefore unadministrable: not inconvenient, impossible.
///
/// So the first boot has a claim window, the same shape every LAN appliance
/// uses. While `State::claimed` is false this route needs no token; the first
/// successful call sets the owner's password, flips the flag, and hands back
/// the admin token so the page can carry on authenticated without a human ever
/// seeing it. Every later call is refused, whatever it presents.
///
/// What guards the window is source address, not a secret: `/api` is LAN-only
/// (`lanOnly` in `modules/containers.nix`, which deliberately denies loopback
/// so master-proxy tunnel traffic cannot reach it either). The exposure is
/// bounded by that guard and by the window closing on first use, and it is
/// written down in `docs/security-model.md` rather than left implicit.
///
/// The password is validated before anything is staged or executed, exactly as
/// in [`cmd_set_password`], and the flag is only set after the password change
/// actually succeeded — a failed claim leaves the box claimable, or the owner
/// would be locked out by their own typo.
///
/// `receipts` is the daemon's memory of the claim it last answered
/// ([`crate::receipt`]): a claimed box still answers, with the same reply,
/// a caller who presents the password that claimed it within the grace
/// window. That is for the reply that was lost in transit — the proxy timed
/// out while the password was still being set, and the admin key in the
/// reply reached nobody — and it changes nothing: no password is set, the
/// state is not rewritten.
pub fn cmd_claim<L: Losos>(
    l: &mut L,
    user: &str,
    password: &str,
    token: &str,
    receipts: &mut crate::receipt::Receipts,
) -> anyhow::Result<Value> {
    // Fail closed: if the state cannot be read, nobody gets to claim the box.
    let mut state = l
        .load_state()
        .map_err(|e| e.context("reading the state before a claim"))?;
    if state.claimed {
        if let Some(user) = receipts.replay(password) {
            tracing::info!("claim answered again: same password, inside the grace window");
            return Ok(json!({
                "claimed": true,
                "user": user,
                "token": token,
                "replayed": true,
            }));
        }
        return Err(crate::setup::AlreadyClaimed.into());
    }
    // Reuses the set-password path whole, so the two cannot drift.
    let out = cmd_set_password(l, user, password)?;

    state.claimed = true;
    l.save_state(&state)?;
    let user = out.get("user").cloned().unwrap_or(Value::Null);
    receipts.remember(user.as_str().unwrap_or(""), password);

    Ok(json!({
        "claimed": true,
        "user": user,
        "token": token,
    }))
}

/// `POST /api/devices/cloud`: an app password for LosOS Cloud, for one of
/// the owner's computers.
///
/// `password_ok` says whether the password the owner typed is theirs, as
/// LosOS Login's people file has it (the HTTP layer holds that file and
/// counts wrong guesses); a wrong one is [`crate::signin::WrongPassword`].
/// Then LosOS Cloud makes the app password for the owner's account over its
/// socket, as nginx would ask it for a signed-in person. The answer carries
/// the user, the app password and the WebDAV path below the box's address,
/// everything a computer needs to mount the files.
pub fn cmd_device_cloud<L: Losos>(
    l: &mut L,
    password_ok: bool,
    device: &str,
) -> anyhow::Result<Value> {
    use crate::devices::{dav_path, interpret_app_password, validate_device, Minted};
    use crate::pins::CloudRequest;

    let device = validate_device(device).map_err(|e| anyhow::anyhow!(e))?;
    if !password_ok {
        return Err(crate::signin::WrongPassword.into());
    }
    let user = crate::setup::DEFAULT_ADMIN_USER;
    let name = crate::devices::credential_name(&device);
    let minted = match l.cloud_request(&CloudRequest::MintKey {
        user,
        device: &name,
    }) {
        Ok((status, body)) => interpret_app_password(status, &String::from_utf8_lossy(&body)),
        Err(e) => {
            tracing::info!(error = ?e, "the app password request could not reach LosOS Cloud");
            Minted::Unavailable(
                "LosOS Cloud could not be reached. It may still be starting.".to_string(),
            )
        }
    };
    match minted {
        Minted::Credential(app_password) => {
            tracing::info!(device = device.as_str(), "LosOS Cloud app password made");
            Ok(json!({ "user": user, "appPassword": app_password, "path": dav_path(user) }))
        }
        other => Err(mint_refusal(other)),
    }
}

/// `POST /api/devices/git`: an access token for LosOS Git, for one of the
/// owner's computers.
///
/// The HTTP layer has judged the owner's password against LosOS Login's
/// people file by now (`password_ok`, as for [`cmd_device_cloud`]); LosOS
/// Git keeps none. The token replaces one of the same name, from an earlier
/// setup of the same computer: that is the token this computer is replacing.
pub fn cmd_device_git<L: Losos>(
    l: &mut L,
    password_ok: bool,
    device: &str,
) -> anyhow::Result<Value> {
    use crate::devices::{
        credential_name, interpret_token, validate_device, GitOff, Minted, GIT_PATH,
    };

    let device = validate_device(device).map_err(|e| anyhow::anyhow!(e))?;
    let Some(repo) = l.config_repo().filter(|r| r.via_server()) else {
        return Err(GitOff.into());
    };
    if !password_ok {
        return Err(crate::signin::WrongPassword.into());
    }
    let not_up = |e: anyhow::Error| -> anyhow::Error {
        if e.downcast_ref::<crate::config_repo::NotUp>().is_some() {
            crate::setup::NotReady(e.to_string()).into()
        } else {
            e
        }
    };
    ensure_owner(l, &repo).map_err(not_up)?;
    let (status, body) = l
        .git_request(&crate::config_repo::GitOp::CreateToken {
            login: repo.owner.clone(),
            name: credential_name(&device),
        })
        .map_err(not_up)?;
    match interpret_token(status, &body) {
        Minted::Credential(token) => {
            tracing::info!(device = device.as_str(), "LosOS Git token made");
            Ok(json!({ "user": repo.owner, "token": token, "path": GIT_PATH }))
        }
        other => Err(mint_refusal(other)),
    }
}

/// `POST /api/devices/backups`: both credentials a computer's backups need,
/// for the person LosOS Login signed the backups in as.
///
/// LosOS Desktop's backups sign in with the device code flow as
/// [`losos_access::BACKUPS_CLIENT_ID`], so LosOS Login asked for the
/// person's password, their second factor when they have one, and their
/// consent; the HTTP layer has checked the token with LosOS Login by now
/// and passes the person it names. Nobody's password is here.
///
/// The person's own account in each app, as `crate::app_names` names it
/// (the owner's is `notshared`): a LosOS Cloud app password, and a LosOS
/// Git token that replaces one of the same name from an earlier sign-in of
/// the same computer. A box without LosOS Git answers `git: null` with
/// `gitOff`, and the backups put repositories in LosOS Cloud instead. When
/// neither app can be asked, it is not now.
pub fn cmd_device_backups<L: Losos>(
    l: &mut L,
    person: &crate::people::Person,
    device: &str,
) -> anyhow::Result<Value> {
    use crate::devices::{
        credential_name, dav_path, interpret_app_password, interpret_token, validate_device,
        Minted, GIT_PATH,
    };
    use crate::pins::CloudRequest;

    let device = validate_device(device).map_err(|e| anyhow::anyhow!(e))?;
    let owner = person.groups.iter().any(|g| g == crate::people::OWNER);
    let account = if owner {
        crate::app_names::OWNER_ACCOUNT.to_string()
    } else {
        person.name.clone()
    };
    let name = credential_name(&device);

    // LosOS Cloud makes an account the first time LosOS Login sends the
    // person, as `crate::cloud_share` does for the share; the owner's is
    // always there.
    if !owner {
        let arrive = CloudRequest::Arrive {
            user: &account,
            display_name: &person.display_name,
        };
        if let Err(e) = l.cloud_request(&arrive) {
            tracing::info!(error = ?e, "LosOS Cloud could not be reached for a new account");
        }
    }
    let cloud = match l.cloud_request(&CloudRequest::MintKey {
        user: &account,
        device: &name,
    }) {
        Ok((status, body)) => interpret_app_password(status, &String::from_utf8_lossy(&body)),
        Err(e) => {
            tracing::info!(error = ?e, "the app password request could not reach LosOS Cloud");
            Minted::Unavailable(
                "LosOS Cloud could not be reached. It may still be starting.".to_string(),
            )
        }
    };

    // The owner's LosOS Git account is the configuration repository's
    // owner, as `cmd_device_git` makes its tokens for.
    let mut git_account = account.clone();
    let git = match l.config_repo().filter(|r| r.via_server()) {
        None => None,
        Some(repo) => Some({
            if owner {
                git_account = repo.owner.clone();
            }
            let made = if owner {
                ensure_owner(l, &repo)
            } else {
                let profile = crate::app_names::Profile {
                    account: account.clone(),
                    name: match person.display_name.trim() {
                        "" => person.name.clone(),
                        shown => shown.to_string(),
                    },
                    email: String::new(),
                };
                crate::app_names::git(l, &profile).map(|_| ())
            };
            match made {
                Err(e) => {
                    tracing::info!(error = ?e, "LosOS Git could not make the account");
                    Minted::Unavailable(
                        "LosOS Git could not be reached. It may still be starting.".to_string(),
                    )
                }
                Ok(()) => match l.git_request(&crate::config_repo::GitOp::CreateToken {
                    login: git_account.clone(),
                    name: name.clone(),
                }) {
                    Ok((status, body)) => interpret_token(status, &body),
                    Err(e) => {
                        tracing::info!(error = ?e, "the token request could not reach LosOS Git");
                        Minted::Unavailable(
                            "LosOS Git could not be reached. It may still be starting.".to_string(),
                        )
                    }
                },
            }
        }),
    };

    let mut out = json!({ "cloud": null, "git": null });
    let mut why = Vec::new();
    match cloud {
        Minted::Credential(app_password) => {
            out["cloud"] = json!({
                "user": account,
                "appPassword": app_password,
                "path": dav_path(&account),
            });
        }
        other => why.push(refusal_sentence(other)),
    }
    match git {
        None => out["gitOff"] = Value::Bool(true),
        Some(Minted::Credential(token)) => {
            out["git"] = json!({ "user": git_account, "token": token, "path": GIT_PATH });
        }
        Some(other) => why.push(refusal_sentence(other)),
    }
    if out["cloud"].is_null() && out["git"].is_null() {
        return Err(crate::setup::NotReady(why.join(" ")).into());
    }
    if !why.is_empty() {
        out["problems"] = json!(why);
    }
    tracing::info!(
        device = device.as_str(),
        person = person.name.as_str(),
        "backup credentials made"
    );
    Ok(out)
}

/// What a refused mint says to the person whose backups asked for it.
fn refusal_sentence(m: crate::devices::Minted) -> String {
    use crate::devices::Minted;
    match m {
        Minted::Rejected => "The app refused the account.".to_string(),
        Minted::Throttled => "The app is making this box wait; try again in a while.".to_string(),
        Minted::Unavailable(why) => why,
        Minted::Credential(_) => String::new(),
    }
}

/// A mint that did not come back with a credential, as the error the HTTP
/// layer knows how to answer.
fn mint_refusal(m: crate::devices::Minted) -> anyhow::Error {
    use crate::devices::Minted;
    match m {
        Minted::Rejected => crate::signin::WrongPassword.into(),
        Minted::Throttled => crate::signin::Throttled.into(),
        Minted::Unavailable(why) => crate::setup::NotReady(why).into(),
        Minted::Credential(_) => anyhow::anyhow!("a credential is not a refusal"),
    }
}

/// The appliance's recovery code, minted on the first call and stable after.
///
/// A read, not a rotation. There is deliberately no way to ask for a *new*
/// code over this API: the value's only job is to still match what the owner
/// wrote down before a reinstall wiped the box, and an endpoint that replaces
/// it is an endpoint that invalidates their paper copy — from the admin UI, in
/// one click, with nothing to undo it.
///
/// `minted` says whether *this* call created the code, so the wizard can say
/// "write this down now" once and "here it is again" on every later visit.
///
/// What this does **not** do is recover anything yet. Nothing on the edge
/// consumes the code — see `modules/recovery.nix` and the tail of
/// `backend/src/recovery.rs` for the registrar half that does not exist. Until
/// it does, the code proves ownership at the wizard and nowhere else, and the
/// UI must not imply otherwise.
pub fn cmd_recovery<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    let r = l.recovery_code()?;
    Ok(json!({ "code": r.code, "minted": r.minted }))
}

/// Search the app catalogue.
///
/// Two fields out, both of which `admin-ui/app/src/screens/settings/
/// catalogue.ts` parses: `results`, the rows, and `sources`, the catalogues
/// they were drawn from. `sources` is sent even when `results` is empty —
/// "nothing matched on Artifact Hub" and "nothing matched" are different
/// sentences, and only the first one is true.
///
/// The query is validated here rather than only at the HTTP boundary so the
/// rule travels with the command: every caller of this function gets the same
/// refusal, and the D-Bus surface cannot grow a laxer one by accident.
pub fn cmd_apps_search<L: Losos>(l: &mut L, query: &str) -> anyhow::Result<Value> {
    let query = crate::catalogue::validate_query(query).map_err(|e| anyhow::anyhow!(e))?;
    let results = l.search_apps(query)?;
    Ok(json!({ "sources": [crate::catalogue::SOURCE], "results": results }))
}

/// Everything the Market pane shows, in one round trip.
///
/// `available: false` — with nothing else — when the edge runs no market, this
/// box was not cleared for it, or the box has no registrar. That is the normal
/// state of most appliances and must not look like a failure.
pub fn cmd_market<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    use crate::market::{Op, Outcome};
    // The official-edge gate (`crate::edge`): trading goes through edges LosOS
    // runs and no other. With none in reach the market is simply not
    // available here, with the reason, and the registrar is not even asked.
    if crate::edge::check_market_gate(&l.edge_status()?).is_err() {
        return Ok(json!({ "available": false, "reason": "noOfficialEdge" }));
    }
    let Outcome::Reply(listings) = l.market_request(&Op::Browse)? else {
        return Ok(json!({ "available": false }));
    };
    let Outcome::Reply(account) = l.market_request(&Op::Account)? else {
        return Ok(json!({ "available": false }));
    };
    Ok(json!({ "available": true, "listings": listings, "account": account }))
}

/// The AI models the edge runs on lent memory, and this box's meters, escrow
/// balances and lending: `POST /api/market/llm/models`.
///
/// A view, like [`cmd_market`]: an edge that runs no models, a box not
/// cleared for the market, or no official edge in reach is
/// `{"available": false}` rather than an error, so the pane can say so
/// quietly.
pub fn cmd_llm_models<L: Losos>(l: &mut L, network: Option<String>) -> anyhow::Result<Value> {
    use crate::market::{Op, Outcome, Refused};
    if crate::edge::check_market_gate(&l.edge_status()?).is_err() {
        return Ok(json!({ "available": false, "reason": "noOfficialEdge" }));
    }
    let op = Op::LlmModels { network };
    op.validate().map_err(|e| {
        anyhow::Error::from(Refused {
            status: 400,
            message: e.to_string(),
        })
    })?;
    let Outcome::Reply(mut reply) = l.market_request(&op)? else {
        return Ok(json!({ "available": false }));
    };
    if let Some(obj) = reply.as_object_mut() {
        obj.insert("available".to_string(), json!(true));
    }
    Ok(reply)
}

/// Why a listing is refused on a box that shares only with its local
/// gateways ([`cmd_market_op`]).
pub const LOCAL_ONLY_NOT_SOLD: &str = "this box shares only with the gateways of its own network, and nothing shared that way is sold: turn on sharing with the mesh to sell room or time";

/// One market action, validated here as well as at the HTTP boundary so the
/// rule travels with the command.
///
/// The reply is the registrar's, with `available: true`. A Checkout URL that
/// is not plain `https` is dropped rather than handed to the browser.
pub fn cmd_market_op<L: Losos>(l: &mut L, op: &crate::market::Op) -> anyhow::Result<Value> {
    use crate::market::{Op, Outcome, Refused};
    // Nothing kindashared holds is ever sold: sharing with this network's
    // gateways is not the mesh, and a box that shares only that way has no
    // room or time on the market to offer. Buying stays the owner's own
    // business and is not touched.
    if matches!(op, Op::List { .. }) {
        let settings = parse_settings(&l.read_overrides()?);
        if settings.sharing_locally && !settings.sharing_my_storage {
            return Err(Refused {
                status: 409,
                message: LOCAL_ONLY_NOT_SOLD.to_string(),
            }
            .into());
        }
    }
    // Same gate as `cmd_market`, as a refusal: an action, unlike a view, has
    // to say why it did not happen.
    crate::edge::check_market_gate(&l.edge_status()?)?;
    // Onboarding names this box to Stripe by a UUID derived one-way from the
    // recovery code — the code itself is a credential and never leaves.
    let tagged;
    let op = if matches!(op, Op::Onboard { .. }) {
        let code = l.recovery_code()?;
        tagged = Op::Onboard {
            box_uuid: Some(crate::boxid::box_uuid(&code.code)),
        };
        &tagged
    } else {
        op
    };
    op.validate().map_err(|e| {
        anyhow::Error::from(Refused {
            status: 400,
            message: e.to_string(),
        })
    })?;
    // An escrowed payment names this box's escrow key, which the edge has
    // from the box itself: tell it once more right before the order, so a
    // box that just came up does not have to wait for its signing pass.
    let keys = if matches!(op, Op::Order { .. }) {
        let keys = l.escrow_keys()?;
        if let Some(k) = &keys {
            let register = Op::EscrowDecisions {
                evm: k.evm_address(),
                solana: k.solana_address(),
            };
            if let Err(e) = l.market_request(&register) {
                tracing::warn!(error = ?e, "could not register the escrow keys before an order");
            }
        }
        keys
    } else {
        None
    };
    let Outcome::Reply(mut reply) = l.market_request(op)? else {
        return Err(Refused {
            status: 409,
            message: "the market is not offered to this appliance".to_string(),
        }
        .into());
    };
    if let Some(k) = &keys {
        k.check_invoice(&reply["stablecoin"])?;
    }
    // A chain call is a JSON-RPC object. Wrapping it, or treating a missing
    // checkout URL as a Stripe page, would make the wallet's transport fail.
    if matches!(op, Op::Rpc { .. }) {
        return Ok(reply);
    }
    // Binance's checkout is the one page an on-ramp order sends the owner
    // to; nothing else in its answer is a page.
    if matches!(op, Op::Onramp { .. } | Op::OnrampStatus { .. }) {
        if reply
            .get("link")
            .and_then(Value::as_str)
            .is_some_and(|link| !crate::market::binance_hosted_url(link))
        {
            anyhow::bail!("the registrar returned a link that is not a binance.com page");
        }
        return Ok(reply);
    }
    for key in ["checkout_url", "url"] {
        if let Some(url) = reply.get(key).and_then(Value::as_str) {
            if !crate::market::stripe_hosted_url(url) {
                anyhow::bail!("the registrar returned a {key} that is not a Stripe-hosted page");
            }
        }
    }
    if let Some(obj) = reply.as_object_mut() {
        obj.insert("available".to_string(), json!(true));
    }
    Ok(reply)
}

/// One pass of this box over the escrowed payments it is party to: tell the
/// edge its escrow keys, read what the edge waits on it to agree to, and
/// sign what pays this box's owner (`crate::escrow_key`). lososd runs it
/// every [`ESCROW_PASS_EVERY`]; the count is what it signed.
///
/// # Errors
/// A fault reaching the edge. A box with no market, no official edge or no
/// escrow key signs nothing and is not an error.
pub fn cmd_escrow_pass<L: Losos>(l: &mut L) -> anyhow::Result<usize> {
    use crate::market::{Op, Outcome};
    if crate::edge::check_market_gate(&l.edge_status()?).is_err() {
        return Ok(0);
    }
    let Some(keys) = l.escrow_keys()? else {
        return Ok(0);
    };
    let register = Op::EscrowDecisions {
        evm: keys.evm_address(),
        solana: keys.solana_address(),
    };
    let Outcome::Reply(decisions) = l.market_request(&register)? else {
        return Ok(0);
    };
    let decisions = decisions.as_array().cloned().unwrap_or_default();
    if decisions.is_empty() {
        return Ok(0);
    }
    // Which trades are this box's, from the box's own account view.
    let Outcome::Reply(account) = l.market_request(&Op::Account)? else {
        return Ok(0);
    };
    let mine = crate::escrow_key::Mine::from_account(&account);
    let mut signed = 0;
    for d in &decisions {
        match keys.sign(d, &mine) {
            Ok(body) => match l.market_request(&Op::EscrowSign(body)) {
                Ok(_) => signed += 1,
                Err(e) => tracing::warn!(error = ?e, "the edge did not take an escrow signature"),
            },
            Err(e) => tracing::warn!(error = %e, "an escrow decision was not signed"),
        }
    }
    Ok(signed)
}

/// How often the box looks for escrow decisions to sign. A release waits on
/// it at most this long after the edge decides.
pub const ESCROW_PASS_EVERY: std::time::Duration = std::time::Duration::from_secs(60);

/// Why a machine action was refused before anything left the box: this box
/// does not share its storage, and virtual machines are offered only to a box
/// that does.
pub const VMS_NEED_SHARING: &str =
    "virtual machines are offered only while this box shares its storage";

/// The Virtual machines page: `GET /api/vms`.
///
/// Machines are a sharing feature: a box that keeps its storage to itself is
/// told so (`reason: "notSharing"`) and no edge is asked. Past that the same
/// official-edge gate as the market applies, then the edge's own answer: an
/// edge that runs no machines is `notOffered`. The replicas' status comes
/// from the mesh; a mesh that does not answer leaves the list empty rather
/// than taking the page down.
pub fn cmd_vms<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    use crate::market::{Op, Outcome};
    if !l.load_state()?.sharing {
        return Ok(json!({ "available": false, "reason": "notSharing" }));
    }
    if crate::edge::check_market_gate(&l.edge_status()?).is_err() {
        return Ok(json!({ "available": false, "reason": "noOfficialEdge" }));
    }
    let Outcome::Reply(catalogue) = l.market_request(&Op::VmImages)? else {
        return Ok(json!({ "available": false, "reason": "notOffered" }));
    };
    let Outcome::Reply(listings) = l.market_request(&Op::Browse)? else {
        return Ok(json!({ "available": false, "reason": "notOffered" }));
    };
    let Outcome::Reply(account) = l.market_request(&Op::Account)? else {
        return Ok(json!({ "available": false, "reason": "notOffered" }));
    };
    let machines = match l.market_request(&Op::VmStatus) {
        Ok(Outcome::Reply(v)) if v.is_array() => v,
        Ok(_) => json!([]),
        Err(e) => {
            tracing::warn!(error = ?e, "machine status unavailable");
            json!([])
        }
    };
    let listings: Vec<Value> = listings
        .as_array()
        .map(|all| {
            all.iter()
                .filter(|l| l.get("kind").and_then(Value::as_str) == Some("vm"))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    Ok(json!({
        "available": true,
        "catalogue": catalogue,
        "listings": listings,
        "account": account,
        "machines": machines,
    }))
}

/// One machine action (order, list, close, an image ticket or removal):
/// refused unless this box shares its storage, then relayed exactly as a
/// market action is.
pub fn cmd_vm_op<L: Losos>(l: &mut L, op: &crate::market::Op) -> anyhow::Result<Value> {
    use crate::market::Refused;
    if !l.load_state()?.sharing {
        return Err(Refused {
            status: 409,
            message: VMS_NEED_SHARING.to_string(),
        }
        .into());
    }
    cmd_market_op(l, op)
}

/// LosOS Lab's order button: `GET /api/lab/order`. `enabled` is
/// `losos.lab.ordering.enable`; while it is off the answer says only that,
/// and no edge is asked. On, the catalogue comes from the official edge.
pub fn cmd_lab_order<L: Losos>(l: &mut L, enabled: bool) -> anyhow::Result<Value> {
    use crate::market::{Op, Outcome};
    if !enabled {
        return Ok(json!({ "enabled": false }));
    }
    if crate::edge::check_market_gate(&l.edge_status()?).is_err() {
        return Ok(json!({ "enabled": true, "available": false, "reason": "noOfficialEdge" }));
    }
    match l.market_request(&Op::Hardware)? {
        Outcome::Reply(catalogue) => {
            Ok(json!({ "enabled": true, "available": true, "catalogue": catalogue }))
        }
        Outcome::Unavailable => {
            Ok(json!({ "enabled": true, "available": false, "reason": "notSold" }))
        }
    }
}

/// Check out the Lab's cart: `POST /api/lab/order`. The reply carries the
/// Stripe-hosted `checkout_url`, checked like every other one.
pub fn cmd_lab_order_op<L: Losos>(
    l: &mut L,
    enabled: bool,
    items: Vec<(String, u64)>,
) -> anyhow::Result<Value> {
    if !enabled {
        return Err(crate::market::Refused {
            status: 409,
            message: "ordering is not switched on for this box".to_string(),
        }
        .into());
    }
    cmd_market_op(l, &crate::market::Op::HardwareCheckout { items })
}

/// This box's custom domains: `GET /api/domains`.
///
/// Gated like the market, on an official edge in reach, because only an
/// official edge hands out names and this box's proxy token should go to no
/// other. `{"available": false}` (with a reason when there is one) is the
/// common case and a 200.
pub fn cmd_domains<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    use crate::market::{Op, Outcome};
    if crate::edge::check_market_gate(&l.edge_status()?).is_err() {
        return Ok(json!({ "available": false, "reason": "noOfficialEdge" }));
    }
    match l.market_request(&Op::Domains)? {
        Outcome::Reply(mut view) => {
            record_view(l, &mut view);
            if let Some(obj) = view.as_object_mut() {
                obj.insert("available".to_string(), json!(true));
            }
            Ok(view)
        }
        Outcome::Unavailable => Ok(json!({ "available": false })),
    }
}

/// Add or remove a custom domain, or bind the box's wallet to its names.
/// The reply is the edge's whole view, so the page redraws from one answer.
pub fn cmd_domain_op<L: Losos>(l: &mut L, op: &crate::market::Op) -> anyhow::Result<Value> {
    use crate::market::{Op, Outcome, Refused};
    if !matches!(
        op,
        Op::DomainAdd { .. } | Op::DomainRemove { .. } | Op::DomainWallet { .. }
    ) {
        anyhow::bail!("not a domain operation");
    }
    crate::edge::check_market_gate(&l.edge_status()?)?;
    op.validate().map_err(|e| {
        anyhow::Error::from(Refused {
            status: 400,
            message: e.to_string(),
        })
    })?;
    let Outcome::Reply(mut view) = l.market_request(op)? else {
        return Err(Refused {
            status: 409,
            message: "the edge does not offer custom domains to this box".to_string(),
        }
        .into());
    };
    record_view(l, &mut view);
    if let Some(obj) = view.as_object_mut() {
        obj.insert("available".to_string(), json!(true));
    }
    Ok(view)
}

/// The live domains in an edge's domains view, as LosOS Cloud should trust
/// them: lowercase, a plain host name each, sorted, no duplicates. Anything
/// that is not a host name is dropped rather than trusted, whatever the edge
/// sent.
#[must_use]
pub fn live_public_names(view: &Value) -> Vec<String> {
    let mut names: Vec<String> = view["domains"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| d["status"] == "live")
        .filter_map(|d| d["domain"].as_str())
        .map(str::to_ascii_lowercase)
        .filter(|d| crate::market::valid_domain(d))
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Hand the live names to LosOS Cloud and the relay pass to the announce
/// loop, and take the pass out of the view: the admin page has no use for
/// it. A failure is logged, not returned: the owner asked to see or change
/// their domains, and that worked.
fn record_view<L: Losos>(l: &mut L, view: &mut Value) {
    if let Err(e) = l.write_public_names(&live_public_names(view)) {
        tracing::warn!(error = %e, "could not record this box's live custom domains");
    }
    let pass = view
        .as_object_mut()
        .and_then(|o| o.remove("relay_pass"))
        .and_then(|p| p.as_str().map(str::to_string))
        .filter(|p| valid_relay_pass(p));
    if let Err(e) = l.write_relay_pass(pass.as_deref()) {
        tracing::warn!(error = %e, "could not record this box's relay pass");
    }
}

/// The shape edge/registrar's `routes::well_formed_pass` accepts:
/// `v1.<digits>.<64 lowercase hex>`. Anything else from the edge is dropped
/// rather than written to a file the announce loop sends on.
#[must_use]
pub fn valid_relay_pass(s: &str) -> bool {
    let mut parts = s.splitn(3, '.');
    let (Some(v), Some(epoch), Some(tag)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    v == "v1"
        && (1..=20).contains(&epoch.len())
        && epoch.bytes().all(|b| b.is_ascii_digit())
        && tag.len() == 64
        && tag
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// What the daemon knows about edge proxies: `GET /api/edge`.
pub fn cmd_edge<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    let status = l.edge_status()?;
    let state = l.load_state()?;
    let mut out = status.to_json();
    out["manage"] = manage_json(state.managed_by.as_deref(), &status);
    // Sharing the box turned off itself while no edge answered, and turns
    // back on once one does (`sharing_tick`).
    out["sharingPausedAt"] = json!(state.sharing_paused_at);
    // The same for sharing with the local gateways (`local_sharing_tick`).
    out["localSharingPausedAt"] = json!(state.local_sharing_paused_at);
    // Whether the edge can read what passes through it.
    out["publicTls"] = serde_json::to_value(l.public_tls()?)?;
    Ok(out)
}

/// The owner moves the box to another secondary cluster in reach, or lets it
/// settle anew (`POST /api/edge/cluster`, `{"cluster": "office"}` or
/// `{"cluster": null}`). The box then takes a gateway of that cluster as its
/// path, and its mesh joins that cluster's control plane.
///
/// # Errors
/// [`crate::edge::ClusterNotInReach`] for a cluster none of whose gateways
/// answered the last scan.
pub fn cmd_edge_cluster<L: Losos>(l: &mut L, cluster: Option<&str>) -> anyhow::Result<Value> {
    let status = l.edge_status()?;
    match cluster {
        Some(c) => crate::edge::check_cluster_move(&status, c)?,
        None => {
            if let Some(url) = &status.held {
                return Err(crate::edge::HeldByDomain(url.clone()).into());
            }
        }
    }
    let status = l.set_edge_cluster(cluster)?;
    Ok(json!({ "cluster": status.cluster, "path": status.path }))
}

/// The management switch as `GET /api/edge` shows it: which edge could
/// manage the box now (or why none can), which one the owner switched it on
/// for, and whether that one is the edge in use.
fn manage_json(managed_by: Option<&str>, status: &crate::edge::EdgeStatus) -> Value {
    let (candidate, refused) = match crate::edge::manageable(status) {
        Ok(path) => (json!({ "name": path.name, "url": path.url }), Value::Null),
        Err(e) => (
            Value::Null,
            json!(match e {
                crate::edge::NotManageable::NoLocalEdge => "noLocalEdge",
                crate::edge::NotManageable::OutsideNetwork => "outsideNetwork",
            }),
        ),
    };
    json!({
        "candidate": candidate,
        "refused": refused,
        "managedBy": managed_by,
        "active": crate::edge::manager_active(managed_by, status),
    })
}

/// Let the company edge this box uses manage its LosOS Cloud, LosOS Git and
/// app settings, or stop it (`POST /api/edge/manage`). Switching on names the
/// edge in use, so a box that later moves to another edge is not managed by
/// that one. Switching off always works.
///
/// # Errors
/// [`crate::edge::NotManageable`] when switching on with no company edge in use.
pub fn cmd_edge_manage<L: Losos>(l: &mut L, enabled: bool) -> anyhow::Result<Value> {
    let status = l.edge_status()?;
    let mut s = l.load_state()?;
    s.managed_by = if enabled {
        Some(crate::edge::manageable(&status)?.url.clone())
    } else {
        None
    };
    l.save_state(&s)?;
    Ok(json!({ "manage": manage_json(s.managed_by.as_deref(), &status) }))
}

/// Every `losos.*` option the box declares, with what `overrides.nix` sets
/// (`GET /api/options`; see `crate::options`).
///
/// `available: false` when the box has no option document — an older build
/// — so the Advanced pane can say so rather than draw nothing. A 200, not a
/// 404, because the SPA latches a 404 as "route not served".
pub fn cmd_options<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    match l.read_options_doc()? {
        None => Ok(json!({
            "available": false,
            "version": 0,
            "options": [],
            "stray": [],
            "excluded": {},
        })),
        Some(doc) => {
            let content = l.read_overrides()?;
            Ok(crate::options::join(&doc, &content))
        }
    }
}

/// The settings of LosOS Cloud and LosOS Git (`GET /api/site`; see
/// `crate::site`), in the shape of `GET /api/options`.
pub fn cmd_site<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    match l.read_options_doc()? {
        None => cmd_options(l),
        Some(doc) => Ok(crate::site::view(&doc, &l.read_overrides()?)),
    }
}

/// Change some of them (`POST /api/site`): each name is checked, the lines
/// are patched, and the file goes through [`cmd_apply`] like any other.
pub fn cmd_site_set<L: Losos>(
    l: &mut L,
    changes: &[(String, Option<String>)],
) -> anyhow::Result<Value> {
    let Some(doc) = l.read_options_doc()? else {
        return Err(crate::options::Rejected(
            "this box has no option document, so its settings can only be changed by an administrator"
                .to_string(),
        )
        .into());
    };
    if changes.is_empty() {
        return Err(crate::options::Rejected("nothing to change".to_string()).into());
    }
    for (name, raw) in changes {
        crate::site::check(&doc, name, raw.as_deref())?;
    }
    let body = crate::site::patch(&l.read_overrides()?, changes)?;
    cmd_apply(l, &body)
}

/// The configuration repository as the History pane shows it: where it is
/// on LosOS Git, the branch head, the last commits, and how the last sync
/// went (`GET /api/config`; see `crate::config_repo`).
pub fn cmd_config<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    let repo = l.config_repo();
    let head = l.config_head()?;
    let log = l.config_log(40)?;
    let sync = match (&repo, l.load_sync_report()?) {
        (None, _) => crate::config_repo::SyncReport {
            state: "off".to_string(),
            detail: "LosOS Git is off on this box; changes are still committed here.".to_string(),
            synced_at: None,
            remote_head: None,
        },
        (Some(_), Some(r)) => r,
        (Some(_), None) => crate::config_repo::SyncReport {
            state: "pending".to_string(),
            detail: "Not synced with LosOS Git yet.".to_string(),
            synced_at: None,
            remote_head: None,
        },
    };
    let upstream = match (l.upstream(), l.load_upstream_report()?) {
        (None, _) => json!({
            "url": null,
            "branch": null,
            "report": crate::config_repo::SyncReport {
                state: "off".to_string(),
                detail: "This box does not follow a published branch; it rebuilds only from its own configuration.".to_string(),
                synced_at: None,
                remote_head: None,
            },
        }),
        (Some(up), report) => json!({
            "url": up.url,
            "branch": up.branch,
            "report": report.unwrap_or_else(|| crate::config_repo::SyncReport {
                state: "pending".to_string(),
                detail: "Not checked yet; the box looks before the nightly update.".to_string(),
                synced_at: None,
                remote_head: None,
            }),
        }),
    };
    Ok(json!({
        "enabled": repo.is_some(),
        "repository": repo.map(|r| json!({
            "owner": r.owner,
            "name": r.name,
            "url": r.page_url(),
            "clone": r.remote(),
            "viaServer": r.via_server(),
        })),
        "head": head,
        "log": log,
        "sync": sync,
        "upstream": upstream,
    }))
}

/// Bring the box and LosOS Git in step, once, and report
/// (`POST /api/config/sync`, and the daemon's reconciler every half minute).
///
/// The decision is [`crate::config_repo::plan_sync`] over the two branch
/// heads; what happens on each answer is written out in [`try_sync`]. Every
/// way this can fail is a *report*, not an error: LosOS Git not up yet is the
/// normal state of a box in its first minutes, and the owner reads the
/// outcome on the History pane either way.
pub fn cmd_config_sync<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    let report = sync_once(l);
    l.save_sync_report(&report)?;
    cmd_config(l)
}

fn sync_once<L: Losos>(l: &mut L) -> crate::config_repo::SyncReport {
    use crate::config_repo::{NotUp, SyncReport};
    let Some(repo) = l.config_repo() else {
        return SyncReport::new(
            "off",
            "LosOS Git is off on this box; changes are still committed here.",
        );
    };
    match try_sync(l, &repo) {
        Ok(report) => report,
        Err(e) if e.downcast_ref::<NotUp>().is_some() => {
            tracing::info!(reason = %e, "configuration sync: LosOS Git is not up");
            SyncReport::new("unavailable", e.to_string())
        }
        Err(e) => {
            tracing::warn!(error = ?e, "configuration sync failed");
            SyncReport::new("error", format!("{e:#}"))
        }
    }
}

/// One sync, with every effect spelled out in order.
///
///  1. anything uncommitted in `/etc/nixos` is committed, so the push below
///     carries it (and so a box the installer left with no commit gets one);
///  2. on LosOS Git, the owner's account and the private repository are
///     made if missing (both calls are idempotent);
///  3. the remote branch is fetched and the plan decided;
///  4. ahead → push. Behind → the pushed `overrides.nix` goes through the
///     same two gates an Apply does, then the branch is fast-forwarded and a
///     rebuild queued, unless one is running, in which case it waits.
///     Diverged → reported; this box never force-pushes over the owner's
///     commits and never merges on its own.
fn try_sync<L: Losos>(
    l: &mut L,
    repo: &crate::config_repo::RepoConfig,
) -> anyhow::Result<crate::config_repo::SyncReport> {
    use crate::config_repo::{plan_sync, SyncPlan, SyncReport};

    if l.config_head()?.is_none() {
        l.config_commit("Configuration as installed", "")?;
    } else {
        // A hand edit, or a change applied while git was unavailable.
        l.config_commit("Uncommitted changes", "")?;
    }
    let Some(head) = l.config_head()? else {
        return Ok(SyncReport::new(
            "error",
            "the configuration directory is not a git repository",
        ));
    };

    if repo.via_server() {
        ensure_owner(l, repo)?;
        ensure_repo(l, repo, &head.branch)?;
    }

    let url = repo.remote();
    let remote = l.config_fetch(&url, &head.branch)?;
    let (local_in_remote, remote_in_local) = match remote.as_deref() {
        Some(r) if r != head.sha => (
            l.config_is_ancestor(&head.sha, r)?,
            l.config_is_ancestor(r, &head.sha)?,
        ),
        _ => (false, false),
    };
    let short = |sha: &str| sha.chars().take(10).collect::<String>();
    let mut report = match plan_sync(
        Some(&head.sha),
        remote.as_deref(),
        local_in_remote,
        remote_in_local,
    ) {
        SyncPlan::Nothing => SyncReport::new("ok", "LosOS Git has this box's configuration."),
        SyncPlan::Push => {
            l.config_push(&url, &head.branch)?;
            let mut r = SyncReport::new("ok", format!("Pushed {} to LosOS Git.", short(&head.sha)));
            r.remote_head = Some(head.sha.clone());
            return Ok(r);
        }
        SyncPlan::FastForward => {
            let r = remote.as_deref().expect("a fast-forward has a remote head");
            let building = l
                .load_state()?
                .rebuild
                .is_some_and(|rb| rb.state == RebuildState::Building);
            if building {
                SyncReport::new(
                    "waiting",
                    "LosOS Git has a newer configuration; it is picked up when the running rebuild finishes.",
                )
            } else {
                match l.config_show(r, "modules/overrides.nix")? {
                    None => SyncReport::new(
                        "refused",
                        format!(
                            "The pushed commit {} has no modules/overrides.nix, so it was not taken.",
                            short(r)
                        ),
                    ),
                    Some(body) => match gate_overrides(l, &body) {
                        Err(why) => SyncReport::new(
                            "refused",
                            format!(
                                "The pushed configuration {} was not taken: {why}. Fix it and push again.",
                                short(r)
                            ),
                        ),
                        Ok(()) => {
                            l.config_fast_forward(r)?;
                            queue_rebuild(l, MSG_STARTED)?;
                            SyncReport::new(
                                "ok",
                                format!("Took {} from LosOS Git; rebuilding.", short(r)),
                            )
                        }
                    },
                }
            }
        }
        SyncPlan::Diverged => SyncReport::new(
            "diverged",
            "LosOS Git and this box have each moved on, so the box keeps its own configuration. \
             Reset the branch on LosOS Git to the box's commit, or push again from a fresh clone.",
        ),
    };
    report.remote_head = remote;
    Ok(report)
}

/// The gates an Apply goes through, over a pushed `overrides.nix`: the
/// shape, the option document, and the edge gate (`crate::edge`), so a
/// commit that turns sharing on with no edge in reach is refused the same
/// way the Apply button is.
fn gate_overrides<L: Losos>(l: &mut L, body: &str) -> Result<(), String> {
    crate::overrides::validate_apply(body).map_err(str::to_string)?;
    match l.read_options_doc() {
        Ok(Some(doc)) => crate::options::check_body(&doc, body).map_err(|e| e.0)?,
        Ok(None) => {}
        Err(e) => return Err(format!("the option document could not be read ({e})")),
    }
    let now = parse_settings(&l.read_overrides().map_err(|e| e.to_string())?);
    let edge = l.edge_status().map_err(|e| e.to_string())?;
    crate::edge::check_gate(&now, &parse_settings(body), &edge).map_err(|e| e.to_string())?;
    crate::edge::check_mesh(body).map_err(|e| e.to_string())
}

/// Move `/etc/nixos` to the published branch's newest tree, keeping this
/// box's own files, once, and report (`losos-ctl config --update`, which
/// `nixos-upgrade.service` runs right before the nightly build, so the 03:00
/// rebuild is the one that takes it). It queues no rebuild of its own.
///
/// Like a sync, every way this can fail is a report: a box with no internet
/// keeps building its current tree, which is what it did before this existed.
pub fn cmd_config_update<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    let report = follow_once(l);
    l.save_upstream_report(&report)?;
    cmd_config(l)
}

fn follow_once<L: Losos>(l: &mut L) -> crate::config_repo::SyncReport {
    use crate::config_repo::{NotUp, SyncReport};
    let Some(up) = l.upstream() else {
        return SyncReport::new(
            "off",
            "This box does not follow a published branch; it rebuilds only from its own configuration.",
        );
    };
    match try_follow(l, &up) {
        Ok(report) => report,
        Err(e) if e.downcast_ref::<NotUp>().is_some() => {
            tracing::info!(reason = %e, "update check: the published branch is not reachable");
            SyncReport::new("unavailable", e.to_string())
        }
        Err(e) => {
            tracing::warn!(error = ?e, "update check failed");
            SyncReport::new("error", format!("{e:#}"))
        }
    }
}

/// One follow, in order: wait out a running rebuild (it is reading the
/// tree), commit anything uncommitted so the comparison below sees it, fetch,
/// decide ([`crate::config_repo::plan_follow`]), and take the tree.
fn try_follow<L: Losos>(
    l: &mut L,
    up: &crate::config_repo::Upstream,
) -> anyhow::Result<crate::config_repo::SyncReport> {
    use crate::config_repo::{plan_follow, FollowPlan, SyncReport};

    let building = l
        .load_state()?
        .rebuild
        .is_some_and(|rb| rb.state == RebuildState::Building);
    if building {
        return Ok(SyncReport::new(
            "waiting",
            "A rebuild is running; the published branch is checked again at the next update.",
        ));
    }
    if l.config_head()?.is_none() {
        l.config_commit("Configuration as installed", "")?;
    } else {
        l.config_commit("Uncommitted changes", "")?;
    }
    if l.config_head()?.is_none() {
        return Ok(SyncReport::new(
            "error",
            "the configuration directory is not a git repository",
        ));
    }

    let fetched = l.upstream_fetch(up)?;
    let mark = l.upstream_mark()?;
    let edited = l.config_edited_outside(mark.as_ref().map(|m| m.followed.as_str()))?;
    // Only a different commit can be older; an unknown time is never older,
    // so a box whose mark predates the channels follows as it always did.
    let bases = match &mark {
        Some(m) if m.upstream != fetched => l
            .upstream_base(&fetched)?
            .zip(l.upstream_base(&m.upstream)?),
        _ => None,
    };
    let older = bases.is_some_and(|(new, taken)| new < taken);
    let short: String = fetched.chars().take(10).collect();
    let label = up.label();
    let mut report = match plan_follow(&fetched, mark.as_ref(), edited, older) {
        FollowPlan::UpToDate => SyncReport::new("ok", format!("This box has {label} at {short}.")),
        FollowPlan::Holding => {
            let made = |secs: i64| {
                chrono::DateTime::from_timestamp(secs, 0)
                    .map(|t| t.format("%Y-%m-%d").to_string())
                    .unwrap_or_default()
            };
            let (new, taken) = bases.unwrap_or_default();
            SyncReport::new(
                "holding",
                format!(
                    "{label} is at a version from {}, older than the one from {} this box runs, \
                     so the box keeps its version until the channel passes it.",
                    made(new),
                    made(taken)
                ),
            )
        }
        FollowPlan::Kept => SyncReport::new(
            "kept",
            format!(
                "Files other than overrides.nix and install-target.nix were changed in this box's \
                 configuration, so it keeps them and does not take {label} at {short}. \
                 Put those files back on LosOS Git to follow again."
            ),
        ),
        FollowPlan::Follow => {
            let subject = format!("Update to {label} {short}");
            let body = format!("Upstream: {} {fetched}", up.url);
            match l.config_follow(&fetched, &subject, &body)? {
                Some(_) => SyncReport::new(
                    "ok",
                    format!("Took {label} at {short}; the box builds it at the next update."),
                ),
                None => SyncReport::new("ok", format!("This box has {label} at {short}.")),
            }
        }
    };
    report.remote_head = Some(fetched);
    Ok(report)
}

fn ask<L: Losos>(
    l: &mut L,
    op: &crate::config_repo::GitOp,
) -> anyhow::Result<crate::config_repo::Answer> {
    let (status, body) = l.git_request(op)?;
    Ok(crate::config_repo::classify(status, &body))
}

/// The owner's account on LosOS Git, made if missing, and an administrator.
/// The apps' repositories (`crate::user_apps::cmd_sync`) need it too.
pub(crate) fn ensure_owner<L: Losos>(
    l: &mut L,
    repo: &crate::config_repo::RepoConfig,
) -> anyhow::Result<()> {
    use crate::config_repo::{Answer, GitOp};
    let login = repo.owner.clone();
    match ask(
        l,
        &GitOp::EnsureUser {
            login: login.clone(),
            full_name: String::new(),
            email: String::new(),
            admin: Some(true),
        },
    )? {
        Answer::Ok(_) | Answer::Exists => Ok(()),
        Answer::Missing => anyhow::bail!("LosOS Git has no admin API at the expected path"),
        Answer::Refused { status, message } => {
            anyhow::bail!("LosOS Git refused to make {login} ({status}): {message}")
        }
    }
}

/// The private configuration repository under the owner, made if missing.
fn ensure_repo<L: Losos>(
    l: &mut L,
    repo: &crate::config_repo::RepoConfig,
    branch: &str,
) -> anyhow::Result<()> {
    use crate::config_repo::{Answer, GitOp};
    let (owner, name) = (repo.owner.clone(), repo.name.clone());
    match ask(
        l,
        &GitOp::EnsureRepo {
            owner: owner.clone(),
            name: name.clone(),
            branch: branch.to_string(),
        },
    )? {
        Answer::Ok(_) | Answer::Exists => Ok(()),
        Answer::Missing => anyhow::bail!("LosOS Git has no admin API at the expected path"),
        Answer::Refused { status, message } => {
            anyhow::bail!("LosOS Git refused to make {owner}/{name} ({status}): {message}")
        }
    }
}

/// Rebuild progress. Polled by the admin UI roughly every two seconds.
///
/// While a rebuild is running the stored message is replaced by the live log
/// tail when there is one, which is what makes the UI's progress line move.
/// `progress` is never derived from the log — it stays 0 until the unit
/// reaches a terminal state.
/// `job` identifies which rebuild the document describes, so a client can tell
/// its own rebuild's outcome from a previous one's. Without it, a status poll
/// that lands before the daemon has recorded the new job reads the *previous*
/// job's terminal state and the UI declares "complete" on a rebuild that is
/// still starting. `cmd_apply` happens to persist `building` before it returns
/// the ack, which closes that window today — but that ordering is an
/// implementation detail of one call path, not a promise, and the admin UI
/// should not have to depend on it. Absent when idle: there is no job.
pub fn cmd_status<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    let s = l.load_state()?;
    let Some(rb) = s.rebuild else {
        return Ok(json!({ "state": "idle", "progress": 0, "message": "" }));
    };
    if rb.state == RebuildState::Building {
        let tail = l.rebuild_log_tail()?;
        let message = if tail.is_empty() { rb.message } else { tail };
        return Ok(json!({
            "state": RebuildState::Building.as_str(),
            "progress": rb.progress,
            "message": message,
            "job": rb.job,
        }));
    }
    Ok(json!({
        "state": rb.state.as_str(),
        "progress": rb.progress,
        "message": rb.message,
        "job": rb.job,
    }))
}

/// Map a finished unit's exit code to the state the UI should show.
///
/// On failure the last log line is appended, because "exit 1" alone tells the
/// user nothing about which Nix expression blew up.
pub fn unit_outcome(code: i32, log_tail: &str) -> (RebuildState, i64, String) {
    if code == 0 {
        return (RebuildState::Done, 100, "rebuild complete".to_string());
    }
    let mut msg = format!("rebuild failed (exit {code})");
    if !log_tail.is_empty() {
        msg.push_str(": ");
        msg.push_str(log_tail);
    }
    (RebuildState::Failed, 0, msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeLosos;

    /// Every assertion below mirrors a case from the Haskell `test/Spec.hs`,
    /// so the port is checked against the behaviour that shipped.

    #[test]
    fn change_mesh_flips_sharing_marks_building_and_spawns() {
        let mut f = FakeLosos::new();
        let out = cmd_change(&mut f, Mode::Mesh).unwrap();
        assert_eq!(out["job"], "job-0");
        let s = f.state.clone();
        assert_eq!(s.mode, Mode::Mesh);
        assert!(s.sharing);
        let rb = s.rebuild.unwrap();
        assert_eq!(rb.state, RebuildState::Building);
        assert_eq!(rb.progress, 0);
        assert!(f.spawned);
    }

    #[test]
    fn change_local_rewrites_the_config_line_to_false() {
        let mut f = FakeLosos::new();
        cmd_change(&mut f, Mode::Local).unwrap();
        assert!(f
            .config
            .iter()
            .any(|l| l.contains("losos.sharingMyStorage = false;")));
    }

    /// `change` and `apply` must agree on the file, or the Storage toggle in
    /// the admin UI (apply) and the CLI/D-Bus road (change) each think they
    /// own the box. The fake holds one `overrides.nix`; a change must be
    /// visible to `settings`, which reads that file, and must leave every
    /// other assignment where `apply` put it.
    #[test]
    fn change_patches_the_overrides_file_that_settings_reads() {
        let mut f = FakeLosos::new();
        cmd_apply(
            &mut f,
            "{ ... }:\n{\n  losos.sharingMyStorage = false;\n  losos.hostName = \"kept\";\n}\n",
        )
        .unwrap();
        cmd_change(&mut f, Mode::Mesh).unwrap();
        let settings = cmd_settings(&mut f).unwrap();
        assert_eq!(settings["sharingMyStorage"], true);
        assert_eq!(settings["hostName"], "kept");
        assert_eq!(
            f.config.len(),
            5,
            "change rewrote more than one line: {:?}",
            f.config
        );
    }

    #[test]
    fn status_reports_idle_then_building() {
        let mut f = FakeLosos::new();
        assert_eq!(cmd_status(&mut f).unwrap()["state"], "idle");
        cmd_change(&mut f, Mode::Mesh).unwrap();
        assert_eq!(cmd_status(&mut f).unwrap()["state"], "building");
    }

    #[test]
    fn status_while_building_shows_the_live_log_tail() {
        let mut f = FakeLosos::new();
        f.log = vec!["copying path '/nix/store/abc-bash'".to_string()];
        cmd_change(&mut f, Mode::Mesh).unwrap();
        let out = cmd_status(&mut f).unwrap();
        assert_eq!(out["message"], "copying path '/nix/store/abc-bash'");
        // the log tail must not fabricate progress
        assert_eq!(out["progress"], 0);
    }

    #[test]
    fn status_carries_the_job_id_so_a_client_can_tell_rebuilds_apart() {
        let mut f = FakeLosos::new();
        // Idle: no job to report.
        assert!(cmd_status(&mut f).unwrap().get("job").is_none());

        let ack = cmd_change(&mut f, Mode::Mesh).unwrap();
        let job = ack["job"].as_str().expect("change acks with a job id");
        let building = cmd_status(&mut f).unwrap();
        assert_eq!(building["job"], job, "status must name the running job");

        // And after it settles, so a client polling late still learns which
        // rebuild the terminal state belongs to.
        let (state, progress, message) = unit_outcome(0, "");
        let mut s = f.load_state().unwrap();
        s.rebuild = Some(Rebuild {
            job: job.to_string(),
            state,
            progress,
            message,
        });
        f.save_state(&s).unwrap();
        assert_eq!(cmd_status(&mut f).unwrap()["job"], job);
    }

    #[test]
    fn status_falls_back_to_the_stored_message_with_an_empty_log() {
        let mut f = FakeLosos::new();
        cmd_change(&mut f, Mode::Mesh).unwrap();
        assert_eq!(cmd_status(&mut f).unwrap()["message"], MSG_STARTED);
    }

    #[test]
    fn unit_outcome_maps_success_and_failure() {
        assert_eq!(
            unit_outcome(0, ""),
            (RebuildState::Done, 100, "rebuild complete".to_string())
        );
        let (st, pct, msg) = unit_outcome(1, "error: evaluation failed");
        assert_eq!(st, RebuildState::Failed);
        assert_eq!(pct, 0);
        assert!(msg.contains("evaluation failed"));
        // no log tail => no trailing colon
        assert_eq!(unit_outcome(2, "").2, "rebuild failed (exit 2)");
    }

    #[test]
    fn settings_reports_the_committed_defaults() {
        let mut f = FakeLosos::new();
        let out = cmd_settings(&mut f).unwrap();
        assert_eq!(out["sharingMyStorage"], true);
        assert_eq!(out["hostName"], "mattbox");
        assert!(out.get("nextcloudMode").is_none());
        assert_eq!(out["proxyEnable"], false);
    }

    #[test]
    fn apply_rewrites_overrides_spawns_and_returns_a_job() {
        let mut f = FakeLosos::new();
        let code =
            "{ ... }:\n{\n  losos.sharingMyStorage = false;\n  losos.hostName = \"box2\";\n}\n";
        let out = cmd_apply(&mut f, code).unwrap();
        assert_eq!(out["job"], "job-0");
        assert!(f
            .config
            .iter()
            .any(|l| l.contains("losos.hostName = \"box2\";")));
        assert!(!f
            .config
            .iter()
            .any(|l| l.contains("losos.sharingMyStorage = true;")));
        assert!(f.spawned);
    }

    #[test]
    fn settings_reflects_an_applied_body() {
        let mut f = FakeLosos::new();
        let code = "{ ... }:\n{\n  losos.sharingMyStorage = false;\n  losos.hostName = \"box2\";\n  losos.proxy.enable = true;\n}\n";
        cmd_apply(&mut f, code).unwrap();
        let out = cmd_settings(&mut f).unwrap();
        assert_eq!(out["hostName"], "box2");
        assert_eq!(out["proxyEnable"], true);
    }

    #[test]
    fn apply_honours_the_proxy_alias_and_drops_what_nextcloud_took_with_it() {
        let mut f = FakeLosos::new();
        cmd_apply(
            &mut f,
            "{ ... }:\n{\n  losos.cfd.enable = true;\n  losos.nextcloud.mode = \"container\";\n  losos.keyring.enable = true;\n}\n",
        )
        .unwrap();
        assert_eq!(cmd_settings(&mut f).unwrap()["proxyEnable"], true);
        let file = f.read_overrides().unwrap();
        assert!(file.contains("losos.cfd.enable = true;"), "{file}");
        assert!(!file.contains("nextcloud"), "{file}");
        assert!(!file.contains("keyring"), "{file}");
    }

    #[test]
    fn factory_reset_restores_defaults_resets_state_and_acks() {
        let mut f = FakeLosos::new();
        cmd_apply(
            &mut f,
            "{ ... }:\n{\n  losos.sharingMyStorage = false;\n  losos.hostName = \"box2\";\n}\n",
        )
        .unwrap();
        let out = cmd_factory_reset(&mut f).unwrap();

        assert_eq!(out["reset"], true);
        // the job counter keeps running across commands in one session
        assert_eq!(out["job"], "job-1");
        assert!(f
            .config
            .iter()
            .any(|l| l.contains("losos.hostName = \"mattbox\";")));
        assert!(!f.config.iter().any(|l| l.contains("box2")));
        assert_eq!(f.state.mode, Mode::Local);
        assert!(!f.state.sharing);
        assert!(f.spawned);
        assert_eq!(f.state.rebuild.as_ref().unwrap().message, MSG_RESET_STARTED);
    }

    #[test]
    fn grow_runs_the_three_steps_in_order_and_reports_measured_sizes() {
        use crate::grow::GrowAction;
        let mut f = FakeLosos::new();
        let before = f.persist_bytes;

        let out = cmd_grow(&mut f).unwrap();

        assert_eq!(
            f.grow_ran,
            vec![
                GrowAction::ExtendLv { extents: 512 },
                GrowAction::ResizeLuks { key_file: None },
                GrowAction::ResizeFs,
            ]
        );
        // Reported from two measurements, not from "the commands exited 0".
        assert_eq!(out["grew"], true);
        assert_eq!(out["beforeBytes"], before);
        assert_eq!(out["afterBytes"], f.persist_bytes);
        assert_eq!(out["claimedBytes"], 512u64 * 4 * 1024 * 1024);
    }

    #[test]
    fn storage_reports_the_reserve_and_reads_zero_after_a_grow() {
        // The Storage pane disables "Use reserve" from this reading, so it
        // must say 0 once a grow has spent the reserve, in any tab.
        let mut f = FakeLosos::new();
        let out = cmd_storage(&mut f).unwrap();
        assert_eq!(out["reserveBytes"], 512u64 * 4 * 1024 * 1024);
        assert_eq!(out["totalBytes"], f.persist_bytes);
        assert_eq!(out["usedBytes"], f.persist_used_bytes);
        cmd_grow(&mut f).unwrap();
        let out = cmd_storage(&mut f).unwrap();
        assert_eq!(out["reserveBytes"], 0);
        assert_eq!(out["totalBytes"], f.persist_bytes);
    }

    #[test]
    fn grow_refuses_when_there_is_nothing_to_grow_into() {
        let mut f = FakeLosos::new();
        f.vg_free_extents = 0;
        let err = cmd_grow(&mut f).unwrap_err().to_string();
        assert!(err.contains("no free extents"), "unhelpful: {err}");
        // And nothing was run — a failed grow must not leave the LV extended
        // with the filesystem unaware of it.
        assert!(f.grow_ran.is_empty());
    }

    #[test]
    fn grow_reports_no_growth_rather_than_claiming_success() {
        // The failure this guards is real: resize2fs against a mapping that was
        // never resized prints "Nothing to do!" and exits 0. A command that
        // reported the exit status would call that a success.
        struct Inert(FakeLosos);
        impl Losos for Inert {
            fn load_state(&mut self) -> anyhow::Result<State> {
                self.0.load_state()
            }
            fn save_state(&mut self, s: &State) -> anyhow::Result<()> {
                self.0.save_state(s)
            }
            fn write_overrides(&mut self, b: &str) -> anyhow::Result<()> {
                self.0.write_overrides(b)
            }
            fn read_overrides(&mut self) -> anyhow::Result<String> {
                self.0.read_overrides()
            }
            fn spawn_rebuild(&mut self, j: &str) -> anyhow::Result<()> {
                self.0.spawn_rebuild(j)
            }
            fn rebuild_log_tail(&mut self) -> anyhow::Result<String> {
                self.0.rebuild_log_tail()
            }
            fn next_job_id(&mut self) -> anyhow::Result<String> {
                self.0.next_job_id()
            }
            fn vg_free(&mut self) -> anyhow::Result<crate::grow::VgFree> {
                self.0.vg_free()
            }
            /// Every step "succeeds" and nothing actually grows.
            fn run_grow(&mut self, _: &crate::grow::GrowAction) -> anyhow::Result<()> {
                Ok(())
            }
            fn persist_bytes(&mut self) -> anyhow::Result<u64> {
                self.0.persist_bytes()
            }
            fn persist_used_bytes(&mut self) -> anyhow::Result<u64> {
                self.0.persist_used_bytes()
            }
            fn luks_key_file(&mut self) -> anyhow::Result<Option<String>> {
                self.0.luks_key_file()
            }
            fn recovery_code(&mut self) -> anyhow::Result<crate::recovery::Recovery> {
                self.0.recovery_code()
            }
            fn search_apps(&mut self, query: &str) -> anyhow::Result<Vec<crate::catalogue::App>> {
                self.0.search_apps(query)
            }
            fn apps_config(&mut self) -> Option<crate::apps::Config> {
                self.0.apps_config()
            }
            fn chart_files(
                &mut self,
                chart: &crate::apps::ChartRef,
            ) -> anyhow::Result<crate::apps::ChartFiles> {
                self.0.chart_files(chart)
            }
            fn load_app_records(&mut self) -> anyhow::Result<Vec<crate::apps::Record>> {
                self.0.load_app_records()
            }
            fn save_app_source(
                &mut self,
                release: &str,
                compose: &str,
                files: &[(String, String)],
            ) -> anyhow::Result<()> {
                self.0.save_app_source(release, compose, files)
            }
            fn save_app_record(&mut self, record: &crate::apps::Record) -> anyhow::Result<()> {
                self.0.save_app_record(record)
            }
            fn start_app_job(
                &mut self,
                release: &str,
                action: crate::apps::Action,
            ) -> anyhow::Result<()> {
                self.0.start_app_job(release, action)
            }
            fn app_job_active(&mut self, release: &str) -> bool {
                self.0.app_job_active(release)
            }
            fn market_request(
                &mut self,
                op: &crate::market::Op,
            ) -> anyhow::Result<crate::market::Outcome> {
                self.0.market_request(op)
            }
            fn escrow_keys(&mut self) -> anyhow::Result<Option<crate::escrow_key::Keys>> {
                self.0.escrow_keys()
            }
            fn edge_status(&mut self) -> anyhow::Result<crate::edge::EdgeStatus> {
                self.0.edge_status()
            }
            fn public_tls(&mut self) -> anyhow::Result<crate::edge::PublicTls> {
                self.0.public_tls()
            }
            fn set_edge_cluster(
                &mut self,
                cluster: Option<&str>,
            ) -> anyhow::Result<crate::edge::EdgeStatus> {
                self.0.set_edge_cluster(cluster)
            }
            fn write_public_names(&mut self, names: &[String]) -> anyhow::Result<()> {
                self.0.write_public_names(names)
            }
            fn write_relay_pass(&mut self, pass: Option<&str>) -> anyhow::Result<()> {
                self.0.write_relay_pass(pass)
            }
            fn load_user_apps(&mut self) -> anyhow::Result<crate::user_apps::UserApps> {
                self.0.load_user_apps()
            }
            fn save_user_apps(&mut self, a: &crate::user_apps::UserApps) -> anyhow::Result<()> {
                self.0.save_user_apps(a)
            }
            fn load_app_data(&mut self, id: &str) -> anyhow::Result<crate::user_apps::AppData> {
                self.0.load_app_data(id)
            }
            fn save_app_data(
                &mut self,
                id: &str,
                d: &crate::user_apps::AppData,
            ) -> anyhow::Result<()> {
                self.0.save_app_data(id, d)
            }
            fn remove_app_data(&mut self, id: &str) -> anyhow::Result<()> {
                self.0.remove_app_data(id)
            }
            fn app_repo_head(&mut self, id: &str) -> anyhow::Result<Option<String>> {
                self.0.app_repo_head(id)
            }
            fn app_repo_commit(
                &mut self,
                id: &str,
                f: &[(&str, String)],
                s: &str,
                b: &str,
            ) -> anyhow::Result<Option<String>> {
                self.0.app_repo_commit(id, f, s, b)
            }
            fn app_repo_fetch(&mut self, id: &str, u: &str) -> anyhow::Result<Option<String>> {
                self.0.app_repo_fetch(id, u)
            }
            fn app_repo_is_ancestor(&mut self, id: &str, a: &str, o: &str) -> anyhow::Result<bool> {
                self.0.app_repo_is_ancestor(id, a, o)
            }
            fn app_repo_show(
                &mut self,
                id: &str,
                r: &str,
                p: &str,
            ) -> anyhow::Result<Option<String>> {
                self.0.app_repo_show(id, r, p)
            }
            fn app_repo_set_head(&mut self, id: &str, sha: &str) -> anyhow::Result<()> {
                self.0.app_repo_set_head(id, sha)
            }
            fn app_repo_push(&mut self, id: &str, u: &str) -> anyhow::Result<()> {
                self.0.app_repo_push(id, u)
            }
            fn app_repo_remove(&mut self, id: &str) -> anyhow::Result<()> {
                self.0.app_repo_remove(id)
            }
            fn read_options_doc(&mut self) -> anyhow::Result<Option<crate::options::OptionsDoc>> {
                self.0.read_options_doc()
            }
            fn config_repo(&mut self) -> Option<crate::config_repo::RepoConfig> {
                self.0.config_repo()
            }
            fn config_head(&mut self) -> anyhow::Result<Option<crate::config_repo::Head>> {
                self.0.config_head()
            }
            fn config_commit(&mut self, s: &str, b: &str) -> anyhow::Result<Option<String>> {
                self.0.config_commit(s, b)
            }
            fn config_fetch(&mut self, u: &str, b: &str) -> anyhow::Result<Option<String>> {
                self.0.config_fetch(u, b)
            }
            fn config_is_ancestor(&mut self, a: &str, o: &str) -> anyhow::Result<bool> {
                self.0.config_is_ancestor(a, o)
            }
            fn config_fast_forward(&mut self, t: &str) -> anyhow::Result<()> {
                self.0.config_fast_forward(t)
            }
            fn config_push(&mut self, u: &str, b: &str) -> anyhow::Result<()> {
                self.0.config_push(u, b)
            }
            fn config_show(&mut self, r: &str, p: &str) -> anyhow::Result<Option<String>> {
                self.0.config_show(r, p)
            }
            fn config_log(
                &mut self,
                n: usize,
            ) -> anyhow::Result<Vec<crate::config_repo::LogEntry>> {
                self.0.config_log(n)
            }
            fn git_request(
                &mut self,
                op: &crate::config_repo::GitOp,
            ) -> anyhow::Result<(u16, String)> {
                self.0.git_request(op)
            }
            fn cloud_request(
                &mut self,
                r: &crate::pins::CloudRequest<'_>,
            ) -> anyhow::Result<(u16, Vec<u8>)> {
                self.0.cloud_request(r)
            }
            fn load_sync_report(
                &mut self,
            ) -> anyhow::Result<Option<crate::config_repo::SyncReport>> {
                self.0.load_sync_report()
            }
            fn save_sync_report(
                &mut self,
                r: &crate::config_repo::SyncReport,
            ) -> anyhow::Result<()> {
                self.0.save_sync_report(r)
            }
            fn upstream(&mut self) -> Option<crate::config_repo::Upstream> {
                self.0.upstream()
            }
            fn upstream_fetch(
                &mut self,
                u: &crate::config_repo::Upstream,
            ) -> anyhow::Result<String> {
                self.0.upstream_fetch(u)
            }
            fn upstream_mark(&mut self) -> anyhow::Result<Option<crate::config_repo::FollowMark>> {
                self.0.upstream_mark()
            }
            fn upstream_base(&mut self, sha: &str) -> anyhow::Result<Option<i64>> {
                self.0.upstream_base(sha)
            }
            fn config_edited_outside(&mut self, b: Option<&str>) -> anyhow::Result<bool> {
                self.0.config_edited_outside(b)
            }
            fn config_follow(
                &mut self,
                u: &str,
                s: &str,
                b: &str,
            ) -> anyhow::Result<Option<String>> {
                self.0.config_follow(u, s, b)
            }
            fn load_upstream_report(
                &mut self,
            ) -> anyhow::Result<Option<crate::config_repo::SyncReport>> {
                self.0.load_upstream_report()
            }
            fn save_upstream_report(
                &mut self,
                r: &crate::config_repo::SyncReport,
            ) -> anyhow::Result<()> {
                self.0.save_upstream_report(r)
            }
            fn backup_target(&mut self) -> anyhow::Result<Option<crate::backup::Target>> {
                self.0.backup_target()
            }
            fn write_backup_target(
                &mut self,
                t: Option<&crate::backup::Target>,
            ) -> anyhow::Result<()> {
                self.0.write_backup_target(t)
            }
            fn start_backup_job(&mut self, k: crate::backup::Kind, j: &str) -> anyhow::Result<()> {
                self.0.start_backup_job(k, j)
            }
            fn poll_backup_job(
                &mut self,
                k: crate::backup::Kind,
                j: &str,
            ) -> crate::supervisor::Poll {
                self.0.poll_backup_job(k, j)
            }
            fn stop_backup_job(&mut self, k: crate::backup::Kind, j: &str) -> anyhow::Result<()> {
                self.0.stop_backup_job(k, j)
            }
            fn backup_log_tail(&mut self) -> String {
                self.0.backup_log_tail()
            }
            fn backup_report(&mut self) -> anyhow::Result<Option<crate::backup::Report>> {
                self.0.backup_report()
            }
            fn write_restore_code(&mut self, c: Option<&str>) -> anyhow::Result<()> {
                self.0.write_restore_code(c)
            }
            fn take_restored_overrides(&mut self) -> anyhow::Result<Option<String>> {
                self.0.take_restored_overrides()
            }
            fn now(&mut self) -> u64 {
                self.0.now()
            }
            fn erase_grace_secs(&mut self) -> u64 {
                self.0.erase_grace_secs()
            }
            fn wipe_and_reboot(&mut self) -> anyhow::Result<()> {
                self.0.wipe_and_reboot()
            }
            fn write_erase_report(&mut self, r: &crate::erase::Outside) -> anyhow::Result<()> {
                self.0.write_erase_report(r)
            }
            fn read_erase_report(&mut self) -> anyhow::Result<Option<crate::erase::Outside>> {
                self.0.read_erase_report()
            }
        }

        let mut inert = Inert(FakeLosos::new());
        let out = cmd_grow(&mut inert).unwrap();
        assert_eq!(out["grew"], false, "a no-op grow must not report success");
        assert_eq!(out["beforeBytes"], out["afterBytes"]);
    }

    // ── The edge gate (crate::edge) ─────────────────────────────────────

    #[test]
    fn mesh_mode_is_refused_with_no_edge_in_reach_and_nothing_is_written() {
        let mut fake = FakeLosos::new().without_edge();
        let before = fake.config.clone();
        let err = cmd_change(&mut fake, Mode::Mesh).unwrap_err();
        let why = err
            .downcast_ref::<crate::edge::EdgeRequired>()
            .expect("the refusal is typed, so the HTTP layer can map it to a 409");
        assert_eq!(why.setting, "losos.sharingMyStorage");
        assert_eq!(fake.config, before, "overrides.nix must be untouched");
        assert_eq!(fake.state.mode, Mode::Local);
        assert!(!fake.spawned, "no rebuild may start for a refused change");
        assert_eq!(fake.edge_asked, 1);
    }

    // ── Pausing sharing while no edge answers (crate::edge::sharing_step) ──

    /// A box sharing through an edge it has seen, with the rebuild that
    /// turned sharing on finished.
    fn sharing_fake() -> FakeLosos {
        let mut fake = FakeLosos::new();
        fake.state.claimed = true;
        cmd_change(&mut fake, Mode::Mesh).unwrap();
        fake.state.rebuild.as_mut().unwrap().state = RebuildState::Done;
        fake.spawned = false;
        fake
    }

    fn sharing_line(fake: &FakeLosos) -> bool {
        parse_settings(&fake.config.join("\n")).sharing_my_storage
    }

    #[test]
    fn an_outage_pauses_sharing_once_and_an_edge_brings_it_back() {
        use crate::edge::{OutageClock, SharingStep};
        let mut fake = sharing_fake();
        assert!(
            fake.state.sharing_edge_seen,
            "the gate passed, so an edge answered"
        );
        let mut clock = OutageClock::default();

        clock.observe(false, 1000);
        assert_eq!(
            sharing_tick(&mut fake, &clock, 1000, 600).unwrap(),
            SharingStep::Nothing
        );
        clock.observe(false, 1599);
        assert_eq!(
            sharing_tick(&mut fake, &clock, 1599, 600).unwrap(),
            SharingStep::Nothing
        );
        assert!(sharing_line(&fake) && !fake.spawned);

        clock.observe(false, 1600);
        assert_eq!(
            sharing_tick(&mut fake, &clock, 1600, 600).unwrap(),
            SharingStep::Pause
        );
        assert!(!sharing_line(&fake), "overrides.nix says sharing is off");
        assert_eq!(fake.state.mode, Mode::Local);
        assert_eq!(fake.state.sharing_paused_at, Some(1600));
        assert!(
            fake.spawned,
            "the pause is a rebuild like the owner's switch"
        );

        // Nothing more while the rebuild runs, or while the edge stays away.
        fake.spawned = false;
        clock.observe(false, 2000);
        assert_eq!(
            sharing_tick(&mut fake, &clock, 2000, 600).unwrap(),
            SharingStep::Nothing
        );
        fake.state.rebuild.as_mut().unwrap().state = RebuildState::Done;
        assert_eq!(
            sharing_tick(&mut fake, &clock, 2000, 600).unwrap(),
            SharingStep::Nothing
        );
        assert!(!fake.spawned);

        // The edge answers; a minute later sharing is back.
        clock.observe(true, 2020);
        assert_eq!(
            sharing_tick(&mut fake, &clock, 2020, 600).unwrap(),
            SharingStep::Nothing
        );
        clock.observe(true, 2080);
        assert_eq!(
            sharing_tick(&mut fake, &clock, 2080, 600).unwrap(),
            SharingStep::Resume
        );
        assert!(sharing_line(&fake));
        assert_eq!(fake.state.mode, Mode::Mesh);
        assert_eq!(fake.state.sharing_paused_at, None);
        assert!(fake.state.sharing_edge_seen);
        assert!(fake.spawned);
    }

    #[test]
    fn the_owner_going_local_during_a_pause_is_not_undone() {
        use crate::edge::{OutageClock, SharingStep};
        let mut fake = sharing_fake();
        let mut clock = OutageClock::default();
        clock.observe(false, 0);
        clock.observe(false, 600);
        assert_eq!(
            sharing_tick(&mut fake, &clock, 600, 600).unwrap(),
            SharingStep::Pause
        );
        fake.state.rebuild.as_mut().unwrap().state = RebuildState::Done;

        cmd_change(&mut fake, Mode::Local).unwrap();
        fake.state.rebuild.as_mut().unwrap().state = RebuildState::Done;
        assert_eq!(fake.state.sharing_paused_at, None);
        clock.observe(true, 700);
        clock.observe(true, 800);
        assert_eq!(
            sharing_tick(&mut fake, &clock, 800, 600).unwrap(),
            SharingStep::Nothing
        );
        assert!(!sharing_line(&fake));
    }

    #[test]
    fn a_box_that_never_reached_an_edge_is_never_paused() {
        use crate::edge::{OutageClock, SharingStep};
        // Sharing set in the configuration with no edge anywhere, the way
        // the shared side's VM tests boot.
        let mut fake = FakeLosos::new().without_edge();
        fake.state.claimed = true;
        fake.config = crate::overrides::inject_line(true, &fake.config);
        let mut clock = OutageClock::default();
        clock.observe(false, 0);
        clock.observe(false, 10_000);
        assert_eq!(
            sharing_tick(&mut fake, &clock, 10_000, 600).unwrap(),
            SharingStep::Nothing
        );
        assert!(sharing_line(&fake) && !fake.spawned);
    }

    #[test]
    fn an_unclaimed_box_that_sees_an_edge_writes_no_state() {
        use crate::edge::{OutageClock, SharingStep};
        // A fresh box: sharing on by default, no state file yet. A state
        // file is what marks a box claimed, so writing one here would shut
        // the claim route on its owner.
        let mut fake = FakeLosos::new();
        assert!(!fake.state.claimed && sharing_line(&fake));
        let mut clock = OutageClock::default();
        clock.observe(true, 100);
        assert_eq!(
            sharing_tick(&mut fake, &clock, 100, 600).unwrap(),
            SharingStep::Nothing
        );
        assert_eq!(fake.state, State::default(), "nothing was saved");
    }

    // ── Sharing with the local gateways (kindashared) ───────────────────

    fn locally_body(fake: &FakeLosos, on: bool) -> String {
        crate::overrides::inject_bool("sharingLocally", on, &fake.config).join("\n")
    }

    fn local_line(fake: &FakeLosos) -> bool {
        parse_settings(&fake.config.join("\n")).sharing_locally
    }

    #[test]
    fn sharing_locally_is_refused_through_an_official_edge_alone() {
        // The default fake's edge is official and tertiary: any edge, but
        // not a local gateway.
        let mut fake = FakeLosos::new();
        let before = fake.config.clone();
        let body = locally_body(&fake, true);
        let err = cmd_apply(&mut fake, &body).unwrap_err();
        assert!(
            err.downcast_ref::<crate::edge::LocalGatewayRequired>()
                .is_some(),
            "{err:?}"
        );
        assert_eq!(fake.config, before, "overrides.nix must be untouched");
        assert!(!fake.spawned);

        let mut fake = FakeLosos::new().with_local_gateway();
        let body = locally_body(&fake, true);
        cmd_apply(&mut fake, &body).unwrap();
        assert!(local_line(&fake));
        assert!(fake.state.local_sharing_edge_seen);
        // Sharing through the official edges is a separate switch, untouched.
        assert_eq!(fake.state.mode, Mode::Local);
    }

    #[test]
    fn no_local_gateway_pauses_local_sharing_and_one_brings_it_back() {
        use crate::edge::{OutageClock, SharingStep};
        let mut fake = FakeLosos::new().with_local_gateway();
        fake.state.claimed = true;
        let body = locally_body(&fake, true);
        cmd_apply(&mut fake, &body).unwrap();
        fake.state.rebuild.as_mut().unwrap().state = RebuildState::Done;
        fake.spawned = false;
        let mesh_before = parse_settings(&fake.config.join("\n")).sharing_my_storage;

        let mut clock = OutageClock::default();
        clock.observe(false, 1000);
        clock.observe(false, 1599);
        assert_eq!(
            local_sharing_tick(&mut fake, &clock, 1599, 600).unwrap(),
            SharingStep::Nothing
        );
        clock.observe(false, 1600);
        assert_eq!(
            local_sharing_tick(&mut fake, &clock, 1600, 600).unwrap(),
            SharingStep::Pause
        );
        assert!(!local_line(&fake));
        assert_eq!(fake.state.local_sharing_paused_at, Some(1600));
        assert_eq!(
            parse_settings(&fake.config.join("\n")).sharing_my_storage,
            mesh_before,
            "the pause leaves sharing through the mesh alone"
        );
        assert!(fake.spawned);
        assert_eq!(
            cmd_edge(&mut fake).unwrap()["localSharingPausedAt"],
            json!(1600)
        );

        fake.state.rebuild.as_mut().unwrap().state = RebuildState::Done;
        clock.observe(true, 2000);
        clock.observe(true, 2060);
        assert_eq!(
            local_sharing_tick(&mut fake, &clock, 2060, 600).unwrap(),
            SharingStep::Resume
        );
        assert!(local_line(&fake));
        assert_eq!(fake.state.local_sharing_paused_at, None);
    }

    #[test]
    fn edge_status_says_when_sharing_was_paused() {
        let mut fake = FakeLosos::new();
        assert_eq!(cmd_edge(&mut fake).unwrap()["sharingPausedAt"], Value::Null);
        fake.state.sharing_paused_at = Some(42);
        assert_eq!(cmd_edge(&mut fake).unwrap()["sharingPausedAt"], json!(42));
    }

    #[test]
    fn edge_status_says_whether_the_edge_can_read_the_traffic() {
        let mut fake = FakeLosos::new();
        assert_eq!(
            cmd_edge(&mut fake).unwrap()["publicTls"],
            json!({ "own": null, "checkedAt": null, "seen": {} })
        );
        fake.public_tls = crate::edge::public_tls_from(
            Some("box.losos.cfd\n"),
            Some(r#"{"checkedAt":"2026-10-28T21:00:00Z","names":{"box.losos.cfd":"ok"}}"#),
        );
        assert_eq!(
            cmd_edge(&mut fake).unwrap()["publicTls"],
            json!({
                "own": ["box.losos.cfd"],
                "checkedAt": "2026-10-28T21:00:00Z",
                "seen": { "box.losos.cfd": "ok" },
            })
        );
    }

    #[test]
    fn going_local_never_asks_about_the_edge() {
        let mut fake = FakeLosos::new().without_edge();
        fake.state.mode = Mode::Mesh;
        fake.state.sharing = true;
        cmd_change(&mut fake, Mode::Local).unwrap();
        assert_eq!(fake.state.mode, Mode::Local);
        assert!(fake.spawned);
        assert_eq!(fake.edge_asked, 0);
    }

    #[test]
    fn an_apply_that_joins_the_mesh_is_refused_without_an_edge() {
        let mut fake = FakeLosos::new().without_edge();
        let body = DEFAULT_OVERRIDES_NIX
            .replace(
                "losos.cluster.enable = false;",
                "losos.cluster.enable = true;",
            )
            .replace(
                "losos.sharingMyStorage = true;",
                "losos.sharingMyStorage = false;",
            );
        // The committed default body has sharing ON, so set the current file
        // to a box that shares nothing, then try to join.
        fake.config = body
            .replace(
                "losos.cluster.enable = true;",
                "losos.cluster.enable = false;",
            )
            .lines()
            .map(str::to_string)
            .collect();
        let err = cmd_apply(&mut fake, &body).unwrap_err();
        let why = err
            .downcast_ref::<crate::edge::EdgeRequired>()
            .expect("typed");
        assert_eq!(why.setting, "losos.cluster.enable");
        assert!(!fake.spawned);
        assert!(fake.state.rebuild.is_none());
    }

    #[test]
    fn joining_the_mesh_without_the_proxy_or_a_name_is_refused_before_the_rebuild() {
        // The edge is in reach, so the edge gate passes; what is missing is
        // on the box itself, and modules/cluster.nix would fail the rebuild.
        let mut fake = FakeLosos::new();
        let join = DEFAULT_OVERRIDES_NIX.replace(
            "losos.cluster.enable = false;",
            "losos.cluster.enable = true;",
        );
        let err = cmd_apply(&mut fake, &join).unwrap_err();
        assert_eq!(
            err.downcast_ref::<crate::edge::MeshNotReady>(),
            Some(&crate::edge::MeshNotReady::ProxyOff)
        );
        let proxied = join.replace("losos.proxy.enable = false;", "losos.proxy.enable = true;");
        let err = cmd_apply(&mut fake, &proxied).unwrap_err();
        assert_eq!(
            err.downcast_ref::<crate::edge::MeshNotReady>(),
            Some(&crate::edge::MeshNotReady::StockName)
        );
        assert!(!fake.spawned);
        let named = proxied.replace(
            "losos.hostName = \"mattbox\";",
            "losos.hostName = \"kitchen\";",
        );
        assert!(cmd_apply(&mut fake, &named).unwrap()["job"].is_string());
        assert!(fake.spawned);
    }

    #[test]
    fn an_apply_that_leaves_sharing_on_passes_while_the_edge_is_away() {
        // The default body has sharingMyStorage = true already: a hostname
        // change during an edge outage is not a sharing decision.
        let mut fake = FakeLosos::new().without_edge();
        let body = DEFAULT_OVERRIDES_NIX.replace(
            "losos.hostName = \"mattbox\";",
            "losos.hostName = \"salmon\";",
        );
        let out = cmd_apply(&mut fake, &body).unwrap();
        assert!(out["job"].is_string());
        assert!(fake.spawned);
        assert_eq!(cmd_settings(&mut fake).unwrap()["hostName"], "salmon");
    }

    #[test]
    fn with_an_edge_in_reach_mesh_mode_goes_through_as_before() {
        let mut fake = FakeLosos::new();
        cmd_change(&mut fake, Mode::Mesh).unwrap();
        assert_eq!(fake.state.mode, Mode::Mesh);
        assert!(fake.spawned);
    }

    #[test]
    fn the_edge_document_names_what_was_found_and_what_was_tried() {
        let mut fake = FakeLosos::new();
        let doc = cmd_edge(&mut fake).unwrap();
        assert_eq!(doc["reachable"], true);
        assert_eq!(doc["edges"][0]["name"], "edge");
        assert_eq!(doc["edges"][0]["url"], "http://edge.local:8443");
        assert_eq!(doc["edges"][0]["source"], "lan");
        assert_eq!(doc["lanSearched"], true);

        let mut fake = FakeLosos::new().without_edge();
        let doc = cmd_edge(&mut fake).unwrap();
        assert_eq!(doc["reachable"], false);
        assert_eq!(doc["edges"], serde_json::json!([]));
    }

    #[test]
    fn only_a_lan_edge_in_use_can_be_let_manage_the_box() {
        // The fake's edge passed the gateway checks, and a checked gateway
        // manages the boxes its owner lets it like any other LAN edge.
        let mut fake = FakeLosos::new();
        cmd_edge_manage(&mut fake, true).unwrap();
        assert_eq!(
            fake.state.managed_by.as_deref(),
            Some("http://edge.local:8443")
        );
        cmd_edge_manage(&mut fake, false).unwrap();
        assert_eq!(fake.state.managed_by, None);

        let mut fake = fake.with_company_edge();
        let doc = cmd_edge_manage(&mut fake, true).unwrap();
        assert_eq!(
            fake.state.managed_by.as_deref(),
            Some("http://edge.local:8443")
        );
        assert_eq!(doc["manage"]["active"], true);
        let doc = cmd_edge(&mut fake).unwrap();
        assert_eq!(doc["manage"]["managedBy"], "http://edge.local:8443");
        assert_eq!(doc["manage"]["candidate"]["url"], "http://edge.local:8443");

        // Switching off works with or without an edge.
        let mut gone = fake.without_edge();
        assert_eq!(cmd_edge(&mut gone).unwrap()["manage"]["active"], false);
        cmd_edge_manage(&mut gone, false).unwrap();
        assert_eq!(gone.state.managed_by, None);
        assert_eq!(
            cmd_edge_manage(&mut gone, true)
                .unwrap_err()
                .downcast_ref::<crate::edge::NotManageable>(),
            Some(&crate::edge::NotManageable::NoLocalEdge)
        );
    }

    #[test]
    fn the_owner_moves_the_box_between_secondary_clusters() {
        use crate::edge::{Edge, EdgeStatus, Source, Tier};
        let gw = |cluster: &str, member: &str| Edge {
            name: member.into(),
            url: format!("http://{member}.local:8443"),
            source: Source::Lan,
            official: false,
            gateway: None,
            tunnel: Some(format!("{member}.local:2333")),
            on_network: true,
            tier: Tier::Secondary,
            cluster: Some(cluster.into()),
            member: Some(member.into()),
            official_cluster: None,
        };
        let mut fake = FakeLosos::new();
        fake.edge = EdgeStatus::found(
            vec![
                gw("office", "gw-a"),
                gw("office", "gw-b"),
                gw("lab", "gw-z"),
            ],
            true,
            None,
        );
        let doc = cmd_edge(&mut fake).unwrap();
        assert_eq!(doc["cluster"], "office");
        assert_eq!(doc["path"]["tier"], "secondary");
        assert_eq!(doc["path"]["member"], "gw-a");
        assert_eq!(doc["clusters"][1]["gateways"][1], "gw-b");

        let doc = cmd_edge_cluster(&mut fake, Some("lab")).unwrap();
        assert_eq!(doc["cluster"], "lab");
        assert_eq!(doc["path"]["member"], "gw-z");
        let e = cmd_edge_cluster(&mut fake, Some("garage")).unwrap_err();
        assert_eq!(
            e.downcast_ref::<crate::edge::ClusterNotInReach>(),
            Some(&crate::edge::ClusterNotInReach("garage".into()))
        );
        assert_eq!(fake.edge.cluster.as_deref(), Some("lab"));

        // A custom domain holds the box where it is: no move, no settling anew.
        fake.edge.held = Some("http://gw-z.local:8443".into());
        for target in [Some("office"), None] {
            let e = cmd_edge_cluster(&mut fake, target).unwrap_err();
            assert!(
                e.downcast_ref::<crate::edge::HeldByDomain>().is_some(),
                "{e}"
            );
        }
        assert_eq!(
            cmd_edge(&mut fake).unwrap()["held"],
            "http://gw-z.local:8443"
        );
    }

    #[test]
    fn state_response_never_carries_the_rebuild_record() {
        let mut f = FakeLosos::new();
        cmd_change(&mut f, Mode::Mesh).unwrap();
        let out = cmd_state(&mut f).unwrap();
        assert!(out.get("rebuild").is_none());
        assert_eq!(out["mode"], "mesh");
        assert_eq!(out["sharing"], true);
    }

    // ── The market ──────────────────────────────────────────────────────

    fn market_fake() -> FakeLosos {
        let mut f = FakeLosos::new();
        f.market_routes.insert(
            "GET /market/listings".to_string(),
            (200, r#"[{"id":"lst_1","kind":"storage"}]"#.to_string()),
        );
        f.market_routes.insert(
            "POST /market/account".to_string(),
            (200, r#"{"seller_ready":false}"#.to_string()),
        );
        f
    }

    #[test]
    fn the_escrow_pass_signs_only_what_pays_this_boxs_owner() {
        use crate::market::Op;
        let mut f = market_fake();
        let keys = f.escrow_keys().unwrap().unwrap();
        let decision = |refund: &str, order: &str| {
            serde_json::json!({
                "network": "base", "family": "evm", "chainId": 8453,
                "escrow": "0x5615deb798bb3e4dfa0139dfa1b3d433cc23b72f", "deposit": 4,
                "side": "seller", "step": "release", "refund": refund, "gross": "100",
                "orderId": order,
            })
        };
        f.market_routes.insert(
            "POST /market/escrow/decisions".to_string(),
            (
                200,
                serde_json::json!([
                    decision("0", "ord_sold"),
                    // The edge asks this box, the buyer, to release: refused.
                    decision("0", "ord_bought"),
                    decision("100", "ord_bought"),
                ])
                .to_string(),
            ),
        );
        f.market_routes.insert(
            "POST /market/account".to_string(),
            (
                200,
                r#"{"purchases":[{"id":"ord_bought"}],"sales":[{"id":"ord_sold"}]}"#.to_string(),
            ),
        );
        f.market_routes
            .insert("POST /market/escrow/sign".to_string(), (204, String::new()));
        assert_eq!(cmd_escrow_pass(&mut f).unwrap(), 2);
        assert_eq!(
            f.market_ops[0],
            Op::EscrowDecisions {
                evm: keys.evm_address(),
                solana: keys.solana_address(),
            }
        );
        let signed: Vec<_> = f
            .market_ops
            .iter()
            .filter_map(|op| match op {
                Op::EscrowSign(body) => Some(body["refund"].as_u64().unwrap()),
                _ => None,
            })
            .collect();
        assert_eq!(signed, vec![0, 100]);

        // No escrow key, or no official edge: nothing is asked at all.
        let mut f = market_fake();
        f.escrow_seed = None;
        assert_eq!(cmd_escrow_pass(&mut f).unwrap(), 0);
        assert!(f.market_ops.is_empty());
        let mut f = FakeLosos::new().with_company_edge();
        assert_eq!(cmd_escrow_pass(&mut f).unwrap(), 0);
        assert!(f.market_ops.is_empty());
    }

    #[test]
    fn an_order_registers_the_escrow_keys_and_refuses_an_invoice_naming_others() {
        use crate::market::Op;
        let mut f = market_fake();
        let keys = f.escrow_keys().unwrap().unwrap();
        let order = Op::Order {
            listing_id: "lst_1".to_string(),
            quantity: 2,
        };
        f.market_routes.insert(
            "POST /market/escrow/decisions".to_string(),
            (200, "[]".to_string()),
        );
        f.market_routes.insert(
            "POST /market/orders".to_string(),
            (
                201,
                serde_json::json!({ "order_id": "ord_1", "stablecoin": { "chains": [
                    { "network": "base", "buyer_box": keys.evm_address() },
                    { "network": "solana", "buyer_box": keys.solana_address() },
                ]}})
                .to_string(),
            ),
        );
        assert_eq!(cmd_market_op(&mut f, &order).unwrap()["order_id"], "ord_1");
        assert!(matches!(f.market_ops[0], Op::EscrowDecisions { .. }));
        f.market_routes.insert(
            "POST /market/orders".to_string(),
            (
                201,
                serde_json::json!({ "order_id": "ord_2", "stablecoin": { "chains": [
                    { "network": "base", "buyer_box": "0x00000000000000000000000000000000000000b1" },
                ]}})
                .to_string(),
            ),
        );
        let err = cmd_market_op(&mut f, &order).unwrap_err().to_string();
        assert!(err.contains("another key"), "{err}");
    }

    #[test]
    fn the_market_pane_gets_the_shelf_and_the_account_together() {
        let mut f = market_fake();
        let out = cmd_market(&mut f).unwrap();
        assert_eq!(out["available"], true);
        assert_eq!(out["listings"][0]["id"], "lst_1");
        assert_eq!(out["account"]["seller_ready"], false);
    }

    #[test]
    fn the_ai_models_are_a_quiet_view_when_the_edge_runs_none() {
        // No registrar configured, or an edge that runs no models.
        let mut f = FakeLosos::new();
        assert_eq!(
            cmd_llm_models(&mut f, None).unwrap(),
            serde_json::json!({ "available": false })
        );
        f.market_routes.insert(
            "POST /market/llm/models".to_string(),
            (503, "market not configured on this edge".to_string()),
        );
        assert_eq!(
            cmd_llm_models(&mut f, None).unwrap(),
            serde_json::json!({ "available": false })
        );
        // Through an edge the root never vouched for, the registrar is not
        // even asked.
        let mut f = FakeLosos::new().with_company_edge();
        assert_eq!(
            cmd_llm_models(&mut f, None).unwrap(),
            serde_json::json!({ "available": false, "reason": "noOfficialEdge" })
        );
        assert!(f.market_ops.is_empty());
    }

    #[test]
    fn the_ai_models_carry_the_edges_answer_and_a_loading_model_is_a_refusal() {
        use crate::market::{ChatAsk, Op, Refused};
        let mut f = FakeLosos::new();
        f.market_routes.insert(
            "POST /market/llm/models".to_string(),
            (
                200,
                r#"{"network":"base","models":[{"id":"llama-70b","needMib":45000}]}"#.to_string(),
            ),
        );
        let out = cmd_llm_models(&mut f, Some("base".to_string())).unwrap();
        assert_eq!(out["available"], true);
        assert_eq!(out["models"][0]["needMib"], 45000);
        assert_eq!(
            f.market_ops,
            vec![Op::LlmModels {
                network: Some("base".to_string())
            }]
        );
        assert!(cmd_llm_models(&mut f, Some("../x".to_string()))
            .unwrap_err()
            .downcast_ref::<Refused>()
            .is_some());

        f.market_routes.insert(
            "POST /market/llm/chat".to_string(),
            (503, "the model is still loading on its boxes".to_string()),
        );
        let chat = Op::LlmChat(ChatAsk {
            meter: "mtr_00ff".to_string(),
            messages: vec![("user".to_string(), "hi".to_string())],
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: Vec::new(),
        });
        let err = cmd_market_op(&mut f, &chat).unwrap_err();
        let r = err.downcast_ref::<Refused>().expect("a refusal");
        assert_eq!(r.status, 503);
        f.market_routes.insert(
            "POST /market/llm/chat".to_string(),
            (
                200,
                r#"{"content":"Hello.","tokensIn":3,"tokensOut":2,"costE6":"91000"}"#.to_string(),
            ),
        );
        let out = cmd_market_op(&mut f, &chat).unwrap();
        assert_eq!(out["content"], "Hello.");
        assert_eq!(out["available"], true);
    }

    #[test]
    fn a_box_without_a_market_is_unavailable_not_failed() {
        // No registrar configured.
        let mut f = FakeLosos::new();
        assert_eq!(
            cmd_market(&mut f).unwrap(),
            serde_json::json!({ "available": false })
        );
        // The edge runs a market but this box was not cleared for it.
        let mut f = market_fake();
        f.market_routes
            .insert("POST /market/account".to_string(), (403, String::new()));
        assert_eq!(
            cmd_market(&mut f).unwrap(),
            serde_json::json!({ "available": false })
        );
    }

    #[test]
    fn a_company_edge_gets_sharing_but_never_the_market() {
        use crate::market::Op;
        // The same market fixture, reached through an edge the root never
        // vouched for: the view says why, the action is refused.
        let mut f = market_fake().with_company_edge();
        assert_eq!(
            cmd_market(&mut f).unwrap(),
            serde_json::json!({ "available": false, "reason": "noOfficialEdge" })
        );
        let err = cmd_market_op(
            &mut f,
            &Op::Order {
                listing_id: "lst_1".to_string(),
                quantity: 1,
            },
        )
        .unwrap_err();
        assert!(err
            .downcast_ref::<crate::edge::OfficialEdgeRequired>()
            .is_some());
        assert!(
            f.market_ops.is_empty(),
            "nothing went to the edge: {:?}",
            f.market_ops
        );
        // Sharing through that edge is still allowed.
        assert!(crate::edge::check_gate(
            &crate::model::Settings::default(),
            &crate::model::Settings {
                sharing_my_storage: true,
                ..Default::default()
            },
            &f.edge
        )
        .is_ok());
    }

    #[test]
    fn a_purchase_returns_the_checkout_url_and_nothing_unsafe() {
        use crate::market::Op;
        let order = Op::Order {
            listing_id: "lst_1".to_string(),
            quantity: 2,
        };
        let mut f = market_fake();
        f.market_routes.insert(
            "POST /market/orders".to_string(),
            (
                201,
                r#"{"order_id":"ord_1","checkout_url":"https://checkout.stripe.com/c/1"}"#
                    .to_string(),
            ),
        );
        let out = cmd_market_op(&mut f, &order).unwrap();
        assert_eq!(out["checkout_url"], "https://checkout.stripe.com/c/1");
        assert_eq!(out["available"], true);

        f.market_routes.insert(
            "POST /market/orders".to_string(),
            (201, r#"{"checkout_url":"javascript:alert(1)"}"#.to_string()),
        );
        assert!(cmd_market_op(&mut f, &order).is_err());
    }

    #[test]
    fn a_binance_order_hands_on_binances_checkout_and_nothing_else() {
        use crate::market::Op;
        let buy = Op::Onramp {
            address: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266".to_string(),
            euros: "50".to_string(),
        };
        let mut f = market_fake();
        f.market_routes.insert(
            "POST /market/onramp".to_string(),
            (
                201,
                r#"{"order_id":"0123456789abcdef0123456789abcdef","link":"https://app.binance.com/uni-qr/qccnt?x=1","stage":"open"}"#
                    .to_string(),
            ),
        );
        let out = cmd_market_op(&mut f, &buy).unwrap();
        assert_eq!(out["link"], "https://app.binance.com/uni-qr/qccnt?x=1");
        assert_eq!(out["stage"], "open");

        // An edge cannot send the owner's browser anywhere but binance.com.
        for link in ["https://binance.com.evil.example/", "javascript:alert(1)"] {
            f.market_routes.insert(
                "POST /market/onramp".to_string(),
                (201, format!(r#"{{"order_id":"x","link":"{link}"}}"#)),
            );
            assert!(cmd_market_op(&mut f, &buy).is_err(), "{link}");
        }

        // The edge's daily cap is the owner's to wait out, not a fault.
        f.market_routes.insert(
            "POST /market/onramp".to_string(),
            (
                429,
                "this box started too many purchases today; try again tomorrow".to_string(),
            ),
        );
        let refused = cmd_market_op(&mut f, &buy).unwrap_err();
        assert!(
            refused.downcast_ref::<crate::market::Refused>().is_some(),
            "{refused}"
        );
    }

    #[test]
    fn onboarding_sends_the_derived_box_uuid_and_never_the_recovery_code() {
        use crate::market::Op;
        let mut f = market_fake();
        f.market_routes.insert(
            "POST /market/seller/onboard".to_string(),
            (
                200,
                r#"{"ready":false,"url":"https://connect.stripe.com/x"}"#.to_string(),
            ),
        );
        let code = f.recovery_code().unwrap().code;
        cmd_market_op(&mut f, &Op::Onboard { box_uuid: None }).unwrap();
        let Some(Op::Onboard {
            box_uuid: Some(sent),
        }) = f.market_ops.last().cloned()
        else {
            panic!("onboard went out without a box uuid: {:?}", f.market_ops);
        };
        assert_eq!(sent, crate::boxid::box_uuid(&code));
        assert_ne!(sent, code);
    }

    #[test]
    fn a_caller_cannot_name_a_different_box_uuid() {
        use crate::market::Op;
        let mut f = market_fake();
        f.market_routes.insert(
            "POST /market/seller/onboard".to_string(),
            (200, r#"{"ready":true}"#.to_string()),
        );
        let forged = Op::Onboard {
            box_uuid: Some("0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d".to_string()),
        };
        cmd_market_op(&mut f, &forged).unwrap();
        assert_ne!(f.market_ops.last(), Some(&forged));
    }

    #[test]
    fn the_lab_order_button_asks_nothing_while_it_is_off() {
        let mut f = market_fake();
        f.market_routes.insert(
            "GET /market/hardware".to_string(),
            (200, r#"{"currency":"eur","items":[]}"#.to_string()),
        );
        assert_eq!(
            cmd_lab_order(&mut f, false).unwrap(),
            serde_json::json!({ "enabled": false })
        );
        let e = cmd_lab_order_op(&mut f, false, vec![("box".to_string(), 1)]).unwrap_err();
        assert_eq!(
            e.downcast_ref::<crate::market::Refused>().unwrap().status,
            409
        );
        assert!(f.market_ops.is_empty());

        let out = cmd_lab_order(&mut f, true).unwrap();
        assert_eq!(out["available"], true);
        assert_eq!(out["catalogue"]["currency"], "eur");
        let mut f = market_fake();
        assert_eq!(cmd_lab_order(&mut f, true).unwrap()["reason"], "notSold");
        let mut f = market_fake().with_company_edge();
        assert_eq!(
            cmd_lab_order(&mut f, true).unwrap()["reason"],
            "noOfficialEdge"
        );
    }

    #[test]
    fn a_lab_checkout_follows_only_a_stripe_hosted_page() {
        let mut f = market_fake();
        f.market_routes.insert(
            "POST /market/hardware/checkout".to_string(),
            (
                201,
                r#"{"order_id":"hw_1","checkout_url":"https://checkout.stripe.com/c/pay/cs_1","amount":89800,"currency":"eur"}"#
                    .to_string(),
            ),
        );
        let out = cmd_lab_order_op(&mut f, true, vec![("box".to_string(), 2)]).unwrap();
        assert_eq!(
            out["checkout_url"],
            "https://checkout.stripe.com/c/pay/cs_1"
        );
        f.market_routes.insert(
            "POST /market/hardware/checkout".to_string(),
            (
                201,
                r#"{"checkout_url":"https://evil.example/pay"}"#.to_string(),
            ),
        );
        assert!(cmd_lab_order_op(&mut f, true, vec![("box".to_string(), 2)]).is_err());
    }

    // ── Virtual machines ────────────────────────────────────────────────

    fn vm_fake() -> FakeLosos {
        let mut f = market_fake();
        f.state.sharing = true;
        f.market_routes.insert(
            "GET /market/listings".to_string(),
            (
                200,
                r#"[{"id":"lst_1","kind":"storage"},{"id":"lst_2","kind":"vm"}]"#.to_string(),
            ),
        );
        f.market_routes.insert(
            "GET /market/vm-images".to_string(),
            (
                200,
                r#"{"images":[{"id":"losos"}],"fee_bps":5000}"#.to_string(),
            ),
        );
        f.market_routes.insert(
            "POST /market/vms/status".to_string(),
            (200, r#"[{"order_id":"ord_1","replicas":[]}]"#.to_string()),
        );
        f
    }

    #[test]
    fn machines_are_offered_only_while_the_box_shares_its_storage() {
        use crate::market::{Op, Refused};
        let mut f = vm_fake();
        f.state.sharing = false;
        assert_eq!(
            cmd_vms(&mut f).unwrap(),
            serde_json::json!({ "available": false, "reason": "notSharing" })
        );
        let e = cmd_vm_op(&mut f, &Op::VmStatus).unwrap_err();
        let r = e.downcast_ref::<Refused>().unwrap();
        assert_eq!(r.status, 409);
        assert_eq!(r.message, VMS_NEED_SHARING);
        assert!(f.market_ops.is_empty(), "no edge was asked");
        assert_eq!(f.edge_asked, 0);
    }

    #[test]
    fn the_machines_page_gets_the_catalogue_its_listings_and_the_replicas() {
        let mut f = vm_fake();
        let out = cmd_vms(&mut f).unwrap();
        assert_eq!(out["available"], true);
        assert_eq!(out["catalogue"]["fee_bps"], 5000);
        assert_eq!(
            out["listings"],
            serde_json::json!([{ "id": "lst_2", "kind": "vm" }])
        );
        assert_eq!(out["machines"][0]["order_id"], "ord_1");
        // An edge with no machines says so.
        let mut f = vm_fake();
        f.market_routes.remove("GET /market/vm-images");
        assert_eq!(cmd_vms(&mut f).unwrap()["reason"], "notOffered");
        // A mesh that does not answer costs the replicas, not the page.
        let mut f = vm_fake();
        f.market_routes
            .insert("POST /market/vms/status".to_string(), (500, String::new()));
        assert_eq!(cmd_vms(&mut f).unwrap()["machines"], serde_json::json!([]));
        // A company edge gets no machines either.
        let mut f = vm_fake().with_company_edge();
        assert_eq!(cmd_vms(&mut f).unwrap()["reason"], "noOfficialEdge");
    }

    #[test]
    fn a_machine_order_carries_its_image_name_and_cloud_init() {
        use crate::market::Op;
        let mut f = vm_fake();
        f.market_routes.insert(
            "POST /market/orders".to_string(),
            (
                201,
                r#"{"order_id":"ord_1","checkout_url":"https://checkout.stripe.com/c/pay/x"}"#
                    .to_string(),
            ),
        );
        let order = |quantity: u64, image: &str, name: &str, user_data: Option<&str>| Op::VmOrder {
            listing_id: "lst_2".to_string(),
            quantity,
            image: image.to_string(),
            name: name.to_string(),
            user_data: user_data.map(str::to_string),
        };
        let op = order(
            2,
            "ubuntu-24.04",
            "web",
            Some("#cloud-config\npackages: [nginx]\n"),
        );
        let out = cmd_vm_op(&mut f, &op).unwrap();
        assert_eq!(out["available"], true);
        let body: Value = serde_json::from_str(&op.body("box", "tok").unwrap()).unwrap();
        assert_eq!(body["image"], "ubuntu-24.04");
        assert_eq!(body["quantity"], 2);
        assert!(body["user_data"]
            .as_str()
            .unwrap()
            .starts_with("#cloud-config"));
        for bad in [
            order(11, "ubuntu-24.04", "web", None),
            order(1, "../etc", "web", None),
            order(1, "ubuntu-24.04", "", None),
            order(1, "ubuntu-24.04", "web", Some("rm -rf /")),
        ] {
            assert!(bad.validate().is_err(), "{bad:?}");
        }
        assert!(order(1, "upl_0123abcd", "own", None).validate().is_ok());
        assert!(Op::VmRemove {
            upload_id: "upl_zz".to_string()
        }
        .validate()
        .is_err());
        assert!(Op::List {
            kind: "vm".to_string(),
            unit_price: 900,
            capacity: 4
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn a_bad_market_request_never_leaves_the_box() {
        use crate::market::{Op, Refused};
        let mut f = market_fake();
        let e = cmd_market_op(
            &mut f,
            &Op::Order {
                listing_id: "../x".to_string(),
                quantity: 1,
            },
        )
        .unwrap_err();
        assert_eq!(e.downcast_ref::<Refused>().unwrap().status, 400);
        assert!(f.market_ops.is_empty());
    }

    #[test]
    fn a_refusal_from_the_registrar_reaches_the_owner_in_its_own_words() {
        use crate::market::{Op, Refused};
        let mut f = market_fake();
        f.market_routes.insert(
            "POST /market/listings".to_string(),
            (
                409,
                "only an appliance sharing its compute on the mesh can sell it".to_string(),
            ),
        );
        let e = cmd_market_op(
            &mut f,
            &Op::List {
                kind: "compute".to_string(),
                unit_price: 100,
                capacity: 4,
            },
        )
        .unwrap_err();
        let r = e.downcast_ref::<Refused>().unwrap();
        assert_eq!(r.status, 409);
        assert!(r.message.contains("sharing its compute"));
    }

    #[test]
    fn nothing_shared_only_with_local_gateways_is_sold() {
        use crate::market::{Op, Refused};
        let list = || Op::List {
            kind: "storage".to_string(),
            unit_price: 100,
            capacity: 4,
        };
        let mut f = market_fake();
        f.config = crate::overrides::inject_bool("sharingMyStorage", false, &f.config);
        f.config = crate::overrides::inject_bool("sharingLocally", true, &f.config);
        let e = cmd_market_op(&mut f, &list()).unwrap_err();
        let r = e.downcast_ref::<Refused>().unwrap();
        assert_eq!(r.status, 409);
        assert_eq!(r.message, LOCAL_ONLY_NOT_SOLD);
        assert!(f.market_ops.is_empty(), "the refused listing left the box");
        // Buying is the owner's own and still goes out.
        f.market_routes.insert(
            "POST /market/orders".to_string(),
            (200, r#"{"id":"ord_1"}"#.to_string()),
        );
        cmd_market_op(
            &mut f,
            &Op::Order {
                listing_id: "lst_1".to_string(),
                quantity: 1,
            },
        )
        .unwrap();
        // With the mesh shared too, the mesh's room is for sale again.
        f.config = crate::overrides::inject_bool("sharingMyStorage", true, &f.config);
        f.market_routes.insert(
            "POST /market/listings".to_string(),
            (200, r#"{"id":"lst_2"}"#.to_string()),
        );
        cmd_market_op(&mut f, &list()).unwrap();
    }

    // ── Custom domains ──────────────────────────────────────────────────

    #[test]
    fn the_domains_view_comes_from_the_edge_and_says_available() {
        let mut f = FakeLosos::new();
        f.market_routes.insert(
            "POST /domains/list".to_string(),
            (
                200,
                r#"{"eligible":true,"target":"3f9a1c0e7b2d4a55.boxes.losos.cfd","domains":[]}"#
                    .to_string(),
            ),
        );
        let out = cmd_domains(&mut f).unwrap();
        assert_eq!(out["available"], true);
        assert_eq!(out["target"], "3f9a1c0e7b2d4a55.boxes.losos.cfd");
        assert_eq!(f.public_names, Some(Vec::new()));
        // An edge with no zone, or one that is not official yet, is a 503
        // there and "not available" here.
        f.market_routes
            .insert("POST /domains/list".to_string(), (503, String::new()));
        assert_eq!(
            cmd_domains(&mut f).unwrap(),
            serde_json::json!({ "available": false })
        );
        // An edge that did not answer leaves the last answer in place: the
        // names stop reaching the box when the edge stops routing them.
        assert_eq!(f.public_names, Some(Vec::new()));
    }

    #[test]
    fn only_live_domains_reach_the_files_app_and_never_a_non_name() {
        let mut f = FakeLosos::new();
        f.market_routes.insert(
            "POST /domains/list".to_string(),
            (
                200,
                r#"{"eligible":true,"domains":[
                    {"domain":"Cloud.Example.org","status":"live"},
                    {"domain":"cloud.example.org","status":"live"},
                    {"domain":"git.example.org","status":"waiting"},
                    {"domain":"a.example.org","status":"live"},
                    {"domain":"evil'];phpinfo();//","status":"live"},
                    {"status":"live"}
                ]}"#
                .to_string(),
            ),
        );
        cmd_domains(&mut f).unwrap();
        assert_eq!(
            f.public_names,
            Some(vec![
                "a.example.org".to_string(),
                "cloud.example.org".to_string()
            ])
        );
        // An add answers with the whole view, which is recorded the same way.
        f.market_routes.insert(
            "POST /domains/add".to_string(),
            (201, r#"{"eligible":true,"domains":[]}"#.to_string()),
        );
        cmd_domain_op(
            &mut f,
            &crate::market::Op::DomainAdd {
                domain: "new.example.org".to_string(),
            },
        )
        .unwrap();
        assert_eq!(f.public_names, Some(Vec::new()));
    }

    #[test]
    fn the_relay_pass_goes_to_its_file_and_never_to_the_page() {
        let pass = format!("v1.494712.{}", "ab".repeat(32));
        let mut f = FakeLosos::new();
        f.market_routes.insert(
            "POST /domains/list".to_string(),
            (
                200,
                format!(r#"{{"eligible":true,"domains":[],"relay_pass":"{pass}"}}"#),
            ),
        );
        let view = cmd_domains(&mut f).unwrap();
        assert!(view.get("relay_pass").is_none(), "{view}");
        assert_eq!(f.relay_pass, Some(Some(pass)));
        // An edge without a route table sends none, and the file goes.
        f.market_routes.insert(
            "POST /domains/list".to_string(),
            (200, r#"{"eligible":true,"domains":[]}"#.to_string()),
        );
        cmd_domains(&mut f).unwrap();
        assert_eq!(f.relay_pass, Some(None));
        // Nothing that is not a pass is written.
        f.market_routes.insert(
            "POST /domains/list".to_string(),
            (
                200,
                r#"{"eligible":true,"domains":[],"relay_pass":"v1.1.x\nrm -rf"}"#.to_string(),
            ),
        );
        cmd_domains(&mut f).unwrap();
        assert_eq!(f.relay_pass, Some(None));
    }

    #[test]
    fn only_an_official_edge_is_asked_about_domains() {
        use crate::market::Op;
        let mut f = FakeLosos::new().with_company_edge();
        assert_eq!(
            cmd_domains(&mut f).unwrap(),
            serde_json::json!({ "available": false, "reason": "noOfficialEdge" })
        );
        let err = cmd_domain_op(
            &mut f,
            &Op::DomainAdd {
                domain: "cloud.example.org".to_string(),
            },
        )
        .unwrap_err();
        assert!(err
            .downcast_ref::<crate::edge::OfficialEdgeRequired>()
            .is_some());
        assert!(f.market_ops.is_empty(), "{:?}", f.market_ops);
    }

    #[test]
    fn adding_a_domain_passes_the_edges_refusal_through_in_its_words() {
        use crate::market::{Op, Refused};
        let mut f = FakeLosos::new();
        f.market_routes.insert(
            "POST /domains/add".to_string(),
            (
                409,
                "a custom domain needs this box's Stripe account, checked by Stripe; finish it on the Market pane first"
                    .to_string(),
            ),
        );
        let add = Op::DomainAdd {
            domain: "cloud.example.org".to_string(),
        };
        let e = cmd_domain_op(&mut f, &add).unwrap_err();
        let r = e.downcast_ref::<Refused>().unwrap();
        assert_eq!(r.status, 409);
        assert!(r.message.contains("Stripe"));
        // A bad name never leaves the box, and a non-domain op is refused.
        let mut f = FakeLosos::new();
        let e = cmd_domain_op(
            &mut f,
            &Op::DomainAdd {
                domain: "not a name".to_string(),
            },
        )
        .unwrap_err();
        assert_eq!(e.downcast_ref::<Refused>().unwrap().status, 400);
        assert!(cmd_domain_op(&mut f, &Op::Account).is_err());
        assert!(f.market_ops.is_empty());
    }

    // ── Searching the app catalogue ─────────────────────────────────────

    #[test]
    fn a_search_answers_in_the_two_fields_the_settings_screen_parses() {
        let mut f = FakeLosos::new();
        let out = cmd_apps_search(&mut f, "nextcloud").unwrap();

        assert_eq!(out["sources"][0], crate::catalogue::SOURCE);
        let rows = out["results"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        // The three the SPA requires on every row.
        assert_eq!(rows[0]["name"], "Nextcloud");
        assert_eq!(rows[0]["source"], "Nextcloud GmbH");
        assert!(rows[0]["id"].is_string());
    }

    /// `sources` is not conditional on there being rows: "nothing matched on
    /// Artifact Hub" and "nothing matched" are different sentences, and the
    /// screen can only write the first one if the box says where it looked.
    #[test]
    fn a_search_that_matched_nothing_still_says_where_it_looked() {
        let mut f = FakeLosos::new();
        f.catalogue_body = Some(r#"{"packages":[]}"#.to_string());
        let out = cmd_apps_search(&mut f, "nothing-matches-this").unwrap();

        assert_eq!(out["sources"][0], crate::catalogue::SOURCE);
        assert!(out["results"].as_array().unwrap().is_empty());
    }

    /// The command trims before it searches, so a trailing space the owner
    /// typed is not part of the term the catalogue is asked for.
    #[test]
    fn the_query_reaches_the_catalogue_trimmed() {
        let mut f = FakeLosos::new();
        cmd_apps_search(&mut f, "  nextcloud  ").unwrap();
        assert_eq!(f.catalogue_queries, vec!["nextcloud".to_string()]);
    }

    /// Refused *before* the effect, not after it. A query the box will not act
    /// on must not become a request to somebody else's server.
    #[test]
    fn a_refused_query_never_reaches_the_catalogue() {
        for bad in ["n", "", "   ", "next\ncloud"] {
            let mut f = FakeLosos::new();
            assert!(
                cmd_apps_search(&mut f, bad).is_err(),
                "{bad:?} was accepted"
            );
            assert!(
                f.catalogue_queries.is_empty(),
                "{bad:?} was sent to the catalogue anyway"
            );
        }
    }

    /// A box with no route out is an `Err`, which the HTTP layer turns into a
    /// 500 and the screen offers to retry — deliberately not an empty result,
    /// which would read as "there is no such app".
    #[test]
    fn a_catalogue_that_cannot_be_reached_is_an_error_not_an_empty_list() {
        let mut f = FakeLosos::new();
        f.catalogue_body = None;
        assert!(cmd_apps_search(&mut f, "nextcloud").is_err());
    }

    // ── The option document and the configuration repository ───────────

    fn with_doc(f: FakeLosos) -> FakeLosos {
        FakeLosos {
            options_doc: Some(crate::options::tests::sample()),
            ..f
        }
    }

    /// A document declaring every line of the committed overrides, plus two
    /// settings of the apps, so an apply of the default file passes the gate.
    fn site_doc() -> crate::options::OptionsDoc {
        let mut options: Vec<Value> = crate::overrides::assignments(DEFAULT_OVERRIDES_NIX)
            .into_iter()
            .map(|(k, v)| {
                let editor = match v.as_str() {
                    "true" | "false" => json!({ "kind": "bool" }),
                    _ if v.parse::<i64>().is_ok() => json!({ "kind": "int" }),
                    _ => json!({ "kind": "str" }),
                };
                json!({ "name": k, "group": "general", "editor": editor, "nixType": "x" })
            })
            .collect();
        options.push(json!({ "name": "cloud.trashDays", "group": "cloud",
            "editor": { "kind": "int", "min": 1 }, "nixType": "int" }));
        options.push(json!({ "name": "git.site.defaultPrivate", "group": "git",
            "editor": { "kind": "enum", "values": ["last", "public"] }, "nixType": "enum" }));
        serde_json::from_value(json!({ "version": 1, "options": options })).unwrap()
    }

    #[test]
    fn the_apps_settings_change_alone_and_rebuild() {
        let mut f = FakeLosos {
            options_doc: Some(site_doc()),
            ..FakeLosos::new()
        };
        let view = cmd_site(&mut f).unwrap();
        let names: Vec<&str> = view["options"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["cloud.trashDays", "git.site.defaultPrivate"]);

        let out = cmd_site_set(
            &mut f,
            &[
                ("cloud.trashDays".into(), Some("30".into())),
                ("git.site.defaultPrivate".into(), Some("\"public\"".into())),
            ],
        )
        .unwrap();
        assert!(out["job"].is_string());
        let body = f.read_overrides().unwrap();
        assert_eq!(
            crate::overrides::lookup_nix("cloud.trashDays", &body).as_deref(),
            Some("30")
        );
        assert_eq!(
            crate::overrides::lookup_nix("hostName", &body).as_deref(),
            Some("\"mattbox\"")
        );
        assert!(f.commits.last().unwrap().subject.contains("trashDays"));

        // Anything else is refused by name, and nothing is written.
        for (name, raw) in [
            ("hostName", Some("\"evil\"")),
            ("sharingMyStorage", Some("false")),
            ("cloud.trashDays", Some("0")),
            ("cloud.trashDays", Some("30;\n  losos.hostName = \"x\"")),
        ] {
            let e = cmd_site_set(&mut f, &[(name.into(), raw.map(str::to_string))]).unwrap_err();
            assert!(
                e.downcast_ref::<crate::options::Rejected>().is_some(),
                "{name}: {e}"
            );
            assert_eq!(f.read_overrides().unwrap(), body);
        }

        cmd_site_set(&mut f, &[("cloud.trashDays".into(), None)]).unwrap();
        assert_eq!(
            crate::overrides::lookup_nix("cloud.trashDays", &f.read_overrides().unwrap()),
            None
        );
    }

    #[test]
    fn apply_commits_with_a_message_naming_the_setting() {
        let mut f = FakeLosos::new();
        let out = cmd_apply(
            &mut f,
            "{ ... }:\n{\n  losos.sharingMyStorage = true;\n  losos.hostName = \"box2\";\n}\n",
        )
        .unwrap();
        assert_eq!(f.commits.len(), 2, "{:?}", f.commits);
        let c = f.commits.last().unwrap();
        assert_eq!(out["commit"], c.sha);
        assert!(
            c.subject.starts_with("Change hostName"),
            "the subject names the setting: {}",
            c.subject
        );
        assert!(
            c.body.contains("losos.hostName: \"mattbox\" -> \"box2\""),
            "{}",
            c.body
        );
        assert!(!f.dirty);
    }

    #[test]
    fn change_and_reset_commit_too() {
        let mut f = FakeLosos::new();
        cmd_change(&mut f, Mode::Local).unwrap();
        assert_eq!(f.commits.last().unwrap().subject, "Change sharingMyStorage");
        let out = cmd_factory_reset(&mut f).unwrap();
        assert_eq!(f.commits.last().unwrap().subject, "Reset sharingMyStorage");
        assert_eq!(out["commit"], f.commits.last().unwrap().sha);
        // A reset of a box already at the defaults has nothing to commit and
        // says so, rather than failing.
        let out = cmd_factory_reset(&mut f).unwrap();
        assert_eq!(out["commit"], Value::Null);
    }

    #[test]
    fn a_failed_commit_does_not_refuse_the_apply() {
        // The fake cannot fail a commit; the real one can (git missing from the
        // unit path). `commit_settings` logs and answers null — assert the
        // contract on the one path the fake does model: nothing to commit.
        let mut f = FakeLosos::new();
        f.dirty = false;
        let body = f.read_overrides().unwrap();
        let out = cmd_apply(&mut f, &body).unwrap();
        assert_eq!(out["commit"], Value::Null);
        assert!(f.spawned, "the rebuild still runs");
    }

    #[test]
    fn apply_is_gated_by_the_option_document() {
        let mut f = with_doc(FakeLosos::new());
        let err = cmd_apply(&mut f, "{ losos.git.enable = \"yes\"; }").unwrap_err();
        let rejected = err
            .downcast_ref::<crate::options::Rejected>()
            .expect("typed, so the HTTP layer answers 400");
        assert_eq!(rejected.0, "losos.git.enable must be true or false");
        assert!(!f.spawned, "a refused apply rebuilds nothing");
        assert_eq!(f.commits.len(), 1, "and commits nothing");

        let err = cmd_apply(&mut f, "{ losos.tpm.enable = false; }").unwrap_err();
        assert!(err.downcast_ref::<crate::options::Rejected>().is_some());

        cmd_apply(&mut f, "{ losos.git.enable = false; }").unwrap();
        assert!(f.spawned);
    }

    #[test]
    fn a_box_without_the_document_keeps_the_first_gate_only() {
        let mut f = FakeLosos::new();
        cmd_apply(&mut f, "{ losos.anything = \"goes\"; }").unwrap();
        assert!(f.spawned);
        let out = cmd_options(&mut f).unwrap();
        assert_eq!(out["available"], false);
        assert_eq!(out["options"], json!([]));
    }

    #[test]
    fn options_joins_what_overrides_sets() {
        let mut f = with_doc(FakeLosos::new());
        cmd_apply(&mut f, "{ losos.git.enable = false; }").unwrap();
        let out = cmd_options(&mut f).unwrap();
        assert_eq!(out["available"], true);
        let opts = out["options"].as_array().unwrap();
        let fe = opts.iter().find(|o| o["name"] == "git.enable").unwrap();
        assert_eq!(fe["set"], "false");
        assert_eq!(fe["editor"]["kind"], "bool");
        let hn = opts.iter().find(|o| o["name"] == "hostName").unwrap();
        assert_eq!(hn["set"], Value::Null);
    }

    #[test]
    fn config_reports_off_with_no_repository_configured() {
        let mut f = FakeLosos::new();
        let out = cmd_config(&mut f).unwrap();
        assert_eq!(out["enabled"], false);
        assert_eq!(out["sync"]["state"], "off");
        assert_eq!(out["head"]["sha"], FakeLosos::sha(0));
        assert_eq!(out["log"][0]["subject"], "losos install");
        let out = cmd_config_sync(&mut f).unwrap();
        assert_eq!(out["sync"]["state"], "off");
        assert!(f.pushed.is_empty());
    }

    #[test]
    fn sync_pushes_when_the_box_is_ahead() {
        let mut f = FakeLosos::new().with_config_repo();
        // The remote has nothing yet: first push.
        let out = cmd_config_sync(&mut f).unwrap();
        assert_eq!(out["sync"]["state"], "ok", "{}", out["sync"]["detail"]);
        assert_eq!(
            f.pushed,
            vec!["http://127.0.0.1:3000/notshared/losos-config.git"]
        );
        assert_eq!(f.remote_commits, vec![FakeLosos::sha(0)]);
        assert_eq!(out["repository"]["url"], "/git/notshared/losos-config");
        assert_eq!(out["sync"]["remoteHead"], FakeLosos::sha(0));
        // In step: nothing to do.
        cmd_config_sync(&mut f).unwrap();
        assert_eq!(f.pushed.len(), 1);
        // An Apply moves the box ahead: pushed on the next run.
        cmd_apply(&mut f, "{ losos.hostName = \"box2\"; }").unwrap();
        let out = cmd_config_sync(&mut f).unwrap();
        assert_eq!(f.pushed.len(), 2);
        assert_eq!(f.remote_commits.len(), 2);
        assert!(out["sync"]["detail"]
            .as_str()
            .unwrap()
            .starts_with("Pushed "));
    }

    #[test]
    fn sync_commits_what_was_left_uncommitted_before_pushing() {
        let mut f = FakeLosos::new().with_config_repo();
        f.dirty = true;
        cmd_config_sync(&mut f).unwrap();
        assert_eq!(f.commits.len(), 2);
        assert_eq!(f.commits.last().unwrap().subject, "Uncommitted changes");
        assert_eq!(f.remote_commits.len(), 2);
    }

    /// A clone pushed a commit on top of the box's: it is fetched, gated,
    /// taken, and the box rebuilds from it.
    #[test]
    fn sync_takes_a_pushed_commit_through_the_gates_and_rebuilds() {
        let mut f = with_doc(FakeLosos::new().with_config_repo());
        cmd_config_sync(&mut f).unwrap();
        let pushed = "f".repeat(40);
        f.remote_commits.push(pushed.clone());
        f.remote_overrides = Some(
            "{ ... }:\n{\n  losos.sharingMyStorage = true;\n  losos.git.enable = false;\n}\n"
                .to_string(),
        );
        let out = cmd_config_sync(&mut f).unwrap();
        assert_eq!(out["sync"]["state"], "ok", "{}", out["sync"]["detail"]);
        assert!(out["sync"]["detail"]
            .as_str()
            .unwrap()
            .starts_with("Took ffffffffff"));
        assert_eq!(f.fast_forwarded, vec![pushed.clone()]);
        assert_eq!(f.commits.last().unwrap().sha, pushed);
        assert!(f.spawned, "the pushed configuration is rebuilt");
        assert_eq!(
            f.state.rebuild.as_ref().unwrap().state,
            RebuildState::Building
        );
        // The working tree now carries the pushed overrides, which is what
        // `settings` reads.
        assert_eq!(cmd_settings(&mut f).unwrap()["sharingMyStorage"], true);
        assert!(f
            .read_overrides()
            .unwrap()
            .contains("losos.git.enable = false;"));
        // Nothing was pushed back: the remote already has it.
        assert_eq!(f.pushed.len(), 1);
    }

    #[test]
    fn sync_refuses_a_pushed_overrides_that_fails_the_gates() {
        let mut f = with_doc(FakeLosos::new().with_config_repo());
        cmd_config_sync(&mut f).unwrap();
        let before = f.commits.last().unwrap().sha.clone();
        f.remote_commits.push("e".repeat(40));
        // A hostname that would brick the box: the first gate.
        f.remote_overrides = Some("{ losos.hostName = \"my box\"; }".to_string());
        let out = cmd_config_sync(&mut f).unwrap();
        assert_eq!(out["sync"]["state"], "refused");
        assert!(out["sync"]["detail"].as_str().unwrap().contains("hostName"));
        assert!(f.fast_forwarded.is_empty());
        assert!(!f.spawned);
        assert_eq!(f.commits.last().unwrap().sha, before);
        // A value of the wrong type: the second gate.
        f.remote_overrides = Some("{ losos.git.enable = \"yes\"; }".to_string());
        let out = cmd_config_sync(&mut f).unwrap();
        assert_eq!(out["sync"]["state"], "refused");
        assert!(out["sync"]["detail"]
            .as_str()
            .unwrap()
            .contains("losos.git.enable must be true or false"));
        // No overrides.nix at all in the pushed commit.
        f.remote_overrides = None;
        let out = cmd_config_sync(&mut f).unwrap();
        assert_eq!(out["sync"]["state"], "refused");
        assert!(f.fast_forwarded.is_empty());
    }

    #[test]
    fn sync_waits_for_a_running_rebuild_before_taking_a_push() {
        let mut f = with_doc(FakeLosos::new().with_config_repo());
        cmd_config_sync(&mut f).unwrap();
        f.remote_commits.push("d".repeat(40));
        f.remote_overrides = Some("{ losos.git.enable = false; }".to_string());
        f.state.rebuild = Some(building("job-9", MSG_STARTED));
        let out = cmd_config_sync(&mut f).unwrap();
        assert_eq!(out["sync"]["state"], "waiting");
        assert!(f.fast_forwarded.is_empty());
    }

    fn following() -> FakeLosos {
        let mut f = FakeLosos::new();
        f.upstream = Some(crate::config_repo::Upstream {
            url: "https://github.com/dasmatus/losos.git".into(),
            branch: "main".into(),
        });
        f
    }

    #[test]
    fn update_is_off_without_an_upstream_and_reports_it() {
        let mut f = FakeLosos::new();
        let commits = f.commits.len();
        let out = cmd_config_update(&mut f).unwrap();
        assert_eq!(out["upstream"]["report"]["state"], "off");
        assert!(out["upstream"]["url"].is_null());
        assert_eq!(f.commits.len(), commits);
        // And a box that follows but never looked says so.
        let out = cmd_config(&mut following()).unwrap();
        assert_eq!(out["upstream"]["report"]["state"], "pending");
        assert_eq!(out["upstream"]["branch"], "main");
    }

    #[test]
    fn update_takes_a_new_published_commit_once_and_queues_no_rebuild() {
        let mut f = following();
        f.upstream_head = Some("a".repeat(40));
        let out = cmd_config_update(&mut f).unwrap();
        assert_eq!(out["upstream"]["report"]["state"], "ok");
        assert_eq!(out["upstream"]["report"]["remoteHead"], "a".repeat(40));
        let last = f.commits.last().unwrap();
        assert_eq!(last.subject, "Update to dasmatus/losos main aaaaaaaaaa");
        assert!(last.body.ends_with(&"a".repeat(40)), "{}", last.body);
        // The nightly rebuild takes it; the update itself starts nothing.
        assert!(!f.spawned);

        // Nothing new: nothing committed.
        let commits = f.commits.len();
        let out = cmd_config_update(&mut f).unwrap();
        assert_eq!(out["upstream"]["report"]["state"], "ok");
        assert_eq!(f.commits.len(), commits);

        f.upstream_head = Some("b".repeat(40));
        cmd_config_update(&mut f).unwrap();
        assert_eq!(f.commits.len(), commits + 1);
        assert_eq!(f.upstream_mark.as_ref().unwrap().upstream, "b".repeat(40));
    }

    #[test]
    fn update_holds_on_a_slower_channel_until_it_passes_the_box() {
        let mut f = following();
        let daily = "d".repeat(40);
        let weekly = "w".repeat(40);
        let later = "e".repeat(40);
        f.upstream_bases.insert(daily.clone(), 1_761_600_000); // 2025-10-27
        f.upstream_bases.insert(weekly.clone(), 1_761_000_000); // 2025-10-20
        f.upstream_bases.insert(later.clone(), 1_762_200_000); // 2025-11-03
        f.upstream_head = Some(daily.clone());
        cmd_config_update(&mut f).unwrap();
        let commits = f.commits.len();

        // The owner picks weekly; its last cut is older than what the box runs.
        f.upstream_head = Some(weekly);
        let out = cmd_config_update(&mut f).unwrap();
        assert_eq!(out["upstream"]["report"]["state"], "holding");
        let detail = out["upstream"]["report"]["detail"].as_str().unwrap();
        assert!(
            detail.contains("2025-10-20") && detail.contains("2025-10-27"),
            "{detail}"
        );
        assert_eq!(f.commits.len(), commits);
        assert_eq!(f.upstream_mark.as_ref().unwrap().upstream, daily);

        // The next weekly cut is newer: the box moves.
        f.upstream_head = Some(later.clone());
        let out = cmd_config_update(&mut f).unwrap();
        assert_eq!(out["upstream"]["report"]["state"], "ok");
        assert_eq!(f.commits.len(), commits + 1);
        assert_eq!(f.upstream_mark.as_ref().unwrap().upstream, later);
    }

    #[test]
    fn update_follows_when_a_commit_has_no_known_time() {
        // A box whose mark the repository no longer holds follows as before.
        let mut f = following();
        f.upstream_head = Some("a".repeat(40));
        cmd_config_update(&mut f).unwrap();
        f.upstream_bases.insert("b".repeat(40), 1);
        f.upstream_head = Some("b".repeat(40));
        let out = cmd_config_update(&mut f).unwrap();
        assert_eq!(out["upstream"]["report"]["state"], "ok");
        assert_eq!(f.upstream_mark.as_ref().unwrap().upstream, "b".repeat(40));
    }

    #[test]
    fn update_commits_a_pending_change_before_it_looks() {
        let mut f = following();
        f.upstream_head = Some("a".repeat(40));
        f.dirty = true;
        cmd_config_update(&mut f).unwrap();
        let subjects: Vec<_> = f.commits.iter().map(|c| c.subject.as_str()).collect();
        assert_eq!(
            subjects[subjects.len() - 2..],
            [
                "Uncommitted changes",
                "Update to dasmatus/losos main aaaaaaaaaa"
            ]
        );
    }

    #[test]
    fn update_keeps_files_someone_changed_and_says_so() {
        let mut f = following();
        f.upstream_head = Some("a".repeat(40));
        f.edited_outside = true;
        let commits = f.commits.len();
        let out = cmd_config_update(&mut f).unwrap();
        assert_eq!(out["upstream"]["report"]["state"], "kept");
        assert_eq!(f.commits.len(), commits);
        assert!(f.upstream_mark.is_none());
    }

    #[test]
    fn update_waits_for_a_rebuild_and_survives_no_network() {
        let mut f = following();
        f.upstream_head = Some("a".repeat(40));
        f.state.rebuild = Some(building("job-9", MSG_STARTED));
        let out = cmd_config_update(&mut f).unwrap();
        assert_eq!(out["upstream"]["report"]["state"], "waiting");
        assert!(f.upstream_mark.is_none());

        let mut f = following();
        let commits = f.commits.len();
        let out = cmd_config_update(&mut f).unwrap();
        assert_eq!(out["upstream"]["report"]["state"], "unavailable");
        assert_eq!(f.commits.len(), commits);
        assert!(f.upstream_report.is_some(), "the report is kept");
    }

    #[test]
    fn sync_reports_diverged_and_touches_neither_side() {
        let mut f = FakeLosos::new().with_config_repo();
        cmd_config_sync(&mut f).unwrap();
        cmd_apply(&mut f, "{ losos.hostName = \"box2\"; }").unwrap();
        f.remote_commits.push("c".repeat(40));
        let out = cmd_config_sync(&mut f).unwrap();
        assert_eq!(out["sync"]["state"], "diverged");
        assert_eq!(f.pushed.len(), 1, "no force-push");
        assert!(f.fast_forwarded.is_empty());
        assert_eq!(f.remote_commits.len(), 2);
    }

    #[test]
    fn sync_reports_unavailable_while_losos_git_is_not_up() {
        // No routes at all: the pod is still starting.
        let mut f = FakeLosos::new();
        f.repo = FakeLosos::new().with_config_repo().repo;
        let out = cmd_config_sync(&mut f).unwrap();
        assert_eq!(out["sync"]["state"], "unavailable");
        assert!(f.pushed.is_empty());
        // The API answers but the remote does not.
        let mut f = FakeLosos::new().with_config_repo();
        f.fetch_fails = true;
        let out = cmd_config_sync(&mut f).unwrap();
        assert_eq!(out["sync"]["state"], "unavailable");
        assert!(f.pushed.is_empty());
        assert!(f.sync_report.is_some(), "the report is kept for the pane");
    }

    #[test]
    fn sync_makes_sure_of_the_owner_and_the_repository_first() {
        let mut f = FakeLosos::new().with_config_repo();
        let out = cmd_config_sync(&mut f).unwrap();
        assert_eq!(out["sync"]["state"], "ok", "{}", out["sync"]["detail"]);
        let calls: Vec<&str> = f.git_calls.iter().map(String::as_str).collect();
        assert_eq!(
            calls,
            vec![
                "PUT /api/v1/admin/users/notshared",
                "PUT /api/v1/admin/users/notshared/repos/losos-config",
            ]
        );
        assert_eq!(f.pushed.len(), 1);
        // Both calls are "ensure": a repeat asks again and makes nothing twice.
        f.git_routes.insert(
            "PUT /api/v1/admin/users/notshared/repos/losos-config".into(),
            (
                409,
                r#"{"message":"There is already a repository."}"#.into(),
            ),
        );
        let out = cmd_config_sync(&mut f).unwrap();
        assert_eq!(out["sync"]["state"], "ok");
    }

    #[test]
    fn a_refusal_from_losos_git_is_reported_not_raised() {
        let mut f = FakeLosos::new().with_config_repo();
        f.git_routes.insert(
            "PUT /api/v1/admin/users/notshared".into(),
            (
                403,
                r#"{"message":"Only the box's administrator may do this."}"#.into(),
            ),
        );
        let out = cmd_config_sync(&mut f).unwrap();
        assert_eq!(out["sync"]["state"], "error");
        assert!(out["sync"]["detail"]
            .as_str()
            .unwrap()
            .contains("administrator"));
    }
}
