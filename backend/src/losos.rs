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
use crate::setup::Readiness;
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

    // ── Setting the Nextcloud admin password ────────────────────────────
    // Split look / plan / do the same way growth is, and for a second reason:
    // the password is an argument to `run_occ` rather than a field of the
    // action, so it cannot end up in an argv or in a serialized plan.
    /// Which mode Nextcloud is *running* in, from `$LOSOS_NEXTCLOUD_MODE`.
    fn nextcloud_mode(&mut self) -> anyhow::Result<crate::setup::NcMode>;
    /// Locate the `occ` entry point: in container mode the single running
    /// workload container, in native mode the host wrapper.
    fn nextcloud_target(
        &mut self,
        mode: crate::setup::NcMode,
    ) -> anyhow::Result<crate::setup::Target>;
    /// Execute one planned step.
    ///
    /// `Err` means the step could not be run at all — a missing binary, an
    /// unreachable CRI socket, an unwritable staging directory. A command that
    /// ran and exited non-zero is `Ok(Some(outcome))`, so which failures mean
    /// what is decided by [`crate::setup::interpret_occ`], purely.
    fn run_occ(
        &mut self,
        action: &crate::setup::OccAction,
        secret: &crate::setup::Secret,
    ) -> anyhow::Result<Option<crate::setup::OccOutcome>>;
    /// Ask the located Nextcloud whether it is ready for `occ`: the argv is
    /// [`crate::setup::plan_status`], the answer is read by
    /// [`crate::setup::interpret_status`]. `Err` means the probe could not be
    /// run at all; a non-zero exit is data, as for [`Losos::run_occ`].
    fn nextcloud_status(
        &mut self,
        target: &crate::setup::Target,
    ) -> anyhow::Result<crate::setup::OccOutcome>;
    /// The last few log lines of the most recent Nextcloud container in any
    /// state, when [`Losos::nextcloud_target`] found no running one: the argv
    /// pair is [`crate::setup::last_container_argv`] then
    /// [`crate::setup::container_log_argv`], and the text is read by
    /// [`crate::setup::describe_stopped`]. `Ok(None)` when there is no such
    /// container or no log; native mode has neither.
    fn nextcloud_last_log(&mut self, mode: crate::setup::NcMode) -> anyhow::Result<Option<String>>;
    /// Ask the running Nextcloud whether `secret` is `user`'s password, on
    /// behalf of the browser at `client` (an IP, forwarded so Nextcloud's own
    /// brute-force protection counts the right party). The request is
    /// [`crate::signin::login_request`] and the answer is read by
    /// [`crate::signin::interpret_login`]; `Err` means the question could
    /// not even be sent, which [`cmd_sign_in`] reports as "not now" rather
    /// than as "wrong".
    fn nextcloud_login(
        &mut self,
        user: &str,
        secret: &crate::setup::Secret,
        client: &str,
    ) -> anyhow::Result<crate::signin::LoginOutcome>;

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

    // ── The market ──────────────────────────────────────────────────────
    /// Relay one [`crate::market::Op`] to the edge's market, with this
    /// appliance's credentials. `Ok(Unavailable)` when there is no registrar
    /// configured or the edge does not offer the market to this box; a
    /// [`crate::market::Refused`] error when the owner can act on the answer;
    /// any other error is a fault for the journal.
    fn market_request(&mut self, op: &crate::market::Op) -> anyhow::Result<crate::market::Outcome>;

    // ── Finding an edge proxy ───────────────────────────────────────────
    /// What the last scan for an edge proxy found (`crate::edge`). The real
    /// daemon answers from its scanner's cache; the fake answers what a test
    /// put there. Never an error for "nothing found": that is a status.
    fn edge_status(&mut self) -> anyhow::Result<crate::edge::EdgeStatus>;

    /// Record the custom domains that are live for this box on its edge, for
    /// LosOS cloud to trust (`trusted_domains`) and to build its links from
    /// when a request arrives on one. The edge verified each of them; this is
    /// only the box learning the answer. Sorted, lowercase, deduplicated.
    fn write_public_names(&mut self, names: &[String]) -> anyhow::Result<()>;
    /// Keep the relay pass the official edge issued (`None`: forget it), for
    /// the announce loop to hand a local edge (backend-registrar's
    /// `routes.rs`). Root-only: it lets a local edge route this box's
    /// custom domains for a few hours.
    fn write_relay_pass(&mut self, pass: Option<&str>) -> anyhow::Result<()>;
    // ── The owner's look ────────────────────────────────────────────────
    // A background picture and the widgets written by hand
    // (`crate::look`). Appliance state beside `state.json`, never a
    // rebuild; the commands are pure over these five effects.
    /// The look document, or [`crate::look::Look::default`] when there is
    /// none yet or it does not parse — lenient the way `load_state` is.
    fn load_look(&mut self) -> anyhow::Result<crate::look::Look>;
    fn save_look(&mut self, look: &crate::look::Look) -> anyhow::Result<()>;
    /// Keep the uploaded picture's bytes beside the document.
    fn write_background(&mut self, bytes: &[u8]) -> anyhow::Result<()>;
    /// The uploaded picture, or `None` when there is none on disk.
    fn read_background(&mut self) -> anyhow::Result<Option<Vec<u8>>>;
    /// Delete the uploaded picture. Not an error when there is none.
    fn remove_background(&mut self) -> anyhow::Result<()>;

    // ── The option document and the configuration repository ───────────
    // `crate::options` is the gate every Apply goes through; `crate::
    // config_repo` is the git history every change lands in. The git and
    // Forgejo effects are one method each, named by what they do, so the
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
    /// One request to LosOS Git's API as the bot administrator: status and
    /// body. The password an operation carries travels in `secret`, never in
    /// an argument vector. [`crate::config_repo::NotUp`] when LosOS Git
    /// cannot be asked at all.
    fn forgejo_request(
        &mut self,
        op: &crate::config_repo::ForgejoOp,
        secret: Option<&crate::setup::Secret>,
    ) -> anyhow::Result<(u16, String)>;
    /// A random password for an account nobody will type into.
    fn mint_secret(&mut self) -> anyhow::Result<crate::setup::Secret>;
    fn load_sync_report(&mut self) -> anyhow::Result<Option<crate::config_repo::SyncReport>>;
    fn save_sync_report(&mut self, report: &crate::config_repo::SyncReport) -> anyhow::Result<()>;

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
    let before = l.read_overrides()?;
    let lines: Vec<String> = before.lines().map(str::to_string).collect();
    let mut body = crate::overrides::inject_line(sharing, &lines).join("\n");
    body.push('\n');
    l.write_overrides(&body)?;
    let commit = commit_settings(l, "Change", &before, &body);
    let job = l.next_job_id()?;
    let mut s = l.load_state()?;
    s.mode = mode;
    s.sharing = sharing;
    s.rebuild = Some(building(&job, MSG_STARTED));
    l.save_state(&s)?;
    l.spawn_rebuild(&job)?;
    Ok(json!({ "job": job, "commit": commit }))
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
    crate::edge::check_gate(&now, &want, &l.edge_status()?)?;
    l.write_overrides(nix_code)?;
    let commit = commit_settings(l, "Change", &before, nix_code);
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

/// Replace the Nextcloud admin password.
///
/// Replaces, never reveals. The install-time password from
/// `modules/nextcloud-common.nix` is 0600 and displayed nowhere; reading it
/// back out over this API would make a root-only file API-readable and save the
/// owner nothing, because they have to type a password they have chosen either
/// way.
///
/// Both validations run before any effect, so a rejected password stages
/// nothing and execs nothing. `changed` is derived from occ's exit status
/// rather than asserted: see [`crate::setup::interpret_occ`] for why that
/// status is worth trusting here and is not in `grow`.
pub fn cmd_set_password<L: Losos>(l: &mut L, user: &str, password: &str) -> anyhow::Result<Value> {
    use crate::setup::{
        interpret_occ, plan_set_password, validate_password, validate_user, OccAction,
    };

    let user = validate_user(user).map_err(|e| anyhow::anyhow!(e))?;
    let secret = validate_password(password).map_err(|e| anyhow::anyhow!(e))?;

    let mode = l.nextcloud_mode()?;
    let target = l.nextcloud_target(mode)?;
    // The mode that actually ran, taken from the located target rather than
    // from the env var, so the two cannot disagree in the reply.
    let ran_mode = target.mode();
    let plan = plan_set_password(&target, user).map_err(|e| anyhow::anyhow!(e))?;

    let mut outcome = None;
    let mut failed_at = None;
    for (i, action) in plan.iter().enumerate() {
        match l.run_occ(action, &secret) {
            Ok(Some(o)) => outcome = Some(o),
            Ok(None) => {}
            Err(e) => {
                failed_at = Some((i, e));
                break;
            }
        }
    }

    // The staged file holds a plaintext password, so it gets removed whatever
    // happened. Anything before the failure point has already run; this is the
    // tail that did not.
    let resume = failed_at.as_ref().map_or(plan.len(), |(i, _)| i + 1);
    for action in plan.iter().skip(resume) {
        if matches!(action, OccAction::ClearSecret { .. }) {
            if let Err(e) = l.run_occ(action, &secret) {
                // Warned about, not raised: it must not mask the failure that
                // got us here, and the next attempt overwrites the file anyway.
                tracing::warn!(error = ?e, "could not remove the staged Nextcloud password file");
            }
        }
    }

    if let Some((_, e)) = failed_at {
        return Err(e);
    }
    let Some(outcome) = outcome else {
        anyhow::bail!("set-password executed no occ command; plan_set_password is broken");
    };
    let message = interpret_occ(&outcome).map_err(|e| anyhow::anyhow!(e))?;
    if outcome.code == 0 {
        sync_owner_password(l, user, &secret);
    }

    Ok(json!({
        "user": user,
        "mode": ran_mode.as_str(),
        "changed": outcome.code == 0,
        "message": message,
    }))
}

/// Whether this box has an owner yet. Unauthenticated, on purpose.
///
/// The admin UI asks this before it decides what to show, so it has to answer
/// without a token — the whole point is that on an unclaimed box nobody has
/// one. It reveals a single bit about a machine the caller has already reached
/// on the LAN, and `modules/containers.nix`'s `lanOnly` guard is what keeps
/// "on the LAN" meaningful.
///
/// `ready` and `waitingFor` say whether the first password can be set *now*.
/// On a fresh box the Nextcloud pod spends its first minutes in
/// `occ maintenance:install`, and a claim sent before that finishes fails;
/// the wizard polls this instead and lets the owner proceed when it is true.
/// Only probed while unclaimed — afterwards there is nothing to wait for, and
/// an unauthenticated route should not spawn a process per request forever.
pub fn cmd_claim_state<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    let claimed = l.load_state().map(|s| s.claimed).unwrap_or(false);
    if claimed {
        return Ok(json!({ "claimed": true, "ready": true, "waitingFor": Value::Null }));
    }
    match nextcloud_readiness(l) {
        Readiness::Ready => {
            Ok(json!({ "claimed": false, "ready": true, "waitingFor": Value::Null }))
        }
        Readiness::NotYet(why) => {
            Ok(json!({ "claimed": false, "ready": false, "waitingFor": why }))
        }
    }
}

/// Can Nextcloud take an `occ user:resetpassword` right now?
///
/// Three things have to be true, and each has its own sentence for the
/// owner: the mode is known, the `occ` entry point can be found (in container
/// mode, that the pod is running at all), and `occ status` says installed and
/// not in maintenance. A probe that cannot even be run reads as "not yet"
/// rather than as an error, because on a box with no shell the only thing the
/// owner can do about either is wait.
pub fn nextcloud_readiness<L: Losos>(l: &mut L) -> Readiness {
    const NOT_STARTED: &str =
        "Nextcloud has not started yet. On a new box this takes a few minutes.";
    let mode = match l.nextcloud_mode() {
        Ok(m) => m,
        Err(e) => {
            tracing::info!(error = ?e, "Nextcloud mode unknown");
            return Readiness::NotYet(NOT_STARTED.to_string());
        }
    };
    let target = match l.nextcloud_target(mode) {
        Ok(t) => t,
        Err(e) => {
            tracing::info!(error = ?e, "Nextcloud not located yet");
            // No running container. Before saying "not yet", look at whether
            // one *was* running and what it said as it died: a pod in
            // CrashLoopBackOff is "not yet" forever, and its last log line
            // is the only thing an owner with no shell can act on.
            let why = match l.nextcloud_last_log(mode) {
                Ok(Some(tail)) => crate::setup::describe_stopped(&tail),
                Ok(None) => None,
                Err(e) => {
                    tracing::info!(error = ?e, "Nextcloud last log could not be read");
                    None
                }
            };
            return Readiness::NotYet(why.unwrap_or_else(|| NOT_STARTED.to_string()));
        }
    };
    match l.nextcloud_status(&target) {
        Ok(outcome) => match crate::setup::interpret_status(&outcome) {
            Ok(()) => Readiness::Ready,
            Err(why) => Readiness::NotYet(why),
        },
        Err(e) => {
            tracing::info!(error = ?e, "Nextcloud status probe could not run");
            Readiness::NotYet(
                "Nextcloud is starting but not answering yet. On a new box this takes a few minutes."
                    .to_string(),
            )
        }
    }
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
/// out while occ was still running, and the admin key in the reply reached
/// nobody — and it changes nothing: no occ runs, the state is not rewritten.
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
    // Asked before anything is staged: the wizard shows this sentence and
    // keeps waiting, where the 500 a failed occ would produce told the owner
    // nothing. Typed, so the HTTP layer answers 503 rather than 500.
    if let Readiness::NotYet(why) = nextcloud_readiness(l) {
        return Err(crate::setup::NotReady(why).into());
    }

    // Reuses the set-password path whole, so the two cannot drift on the thing
    // that matters most here: the password never reaching an argv.
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

/// Unlock the admin pages with the owner's password.
///
/// The password is checked by Nextcloud, not by this daemon — see the header
/// of [`crate::signin`] for why the credential has to live in one place. A
/// correct one is answered with the admin token, the same secret the claim
/// released, so the page carries on exactly as it did with a pasted key.
///
/// Only the *shape* of the candidate is checked here
/// ([`crate::signin::validate_candidate`]), never the rules for a new
/// password: an owner whose password predates a stricter rule set still has
/// to get in with it.
///
/// Unauthenticated by nature — it is how a tab gets its token — and so
/// guarded by the HTTP layer the way the claim is (same-box `Host`, JSON
/// only, matching `Origin`) and throttled per address like a bad token.
pub fn cmd_sign_in<L: Losos>(
    l: &mut L,
    password: &str,
    token: &str,
    client: &str,
) -> anyhow::Result<Value> {
    use crate::signin::{validate_candidate, LoginOutcome, Throttled, WrongPassword};

    let secret = validate_candidate(password).map_err(|e| anyhow::anyhow!(e))?;
    let user = crate::setup::DEFAULT_ADMIN_USER;
    let outcome = match l.nextcloud_login(user, &secret, client) {
        Ok(o) => o,
        Err(e) => {
            tracing::info!(error = ?e, "the sign-in probe could not reach LosOS cloud");
            LoginOutcome::Unavailable(
                "LosOS cloud could not be reached, so the password could not be checked. \
                 It may still be starting."
                    .to_string(),
            )
        }
    };
    match outcome {
        LoginOutcome::Accepted => {
            // The owner just proved this password to LosOS cloud; LosOS Git
            // gets the same one, so one password opens both. Best effort —
            // LosOS Git may still be starting — and never a reason to refuse
            // a sign-in that LosOS cloud accepted.
            sync_owner_password(l, user, &secret);
            Ok(json!({ "user": user, "token": token }))
        }
        LoginOutcome::Rejected => Err(WrongPassword.into()),
        LoginOutcome::Throttled => Err(Throttled.into()),
        // Typed as the claim's "not yet", so the HTTP layer answers 503 with
        // the sentence and the dialog can offer the spare key.
        LoginOutcome::Unavailable(why) => Err(crate::setup::NotReady(why).into()),
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

/// One market action, validated here as well as at the HTTP boundary so the
/// rule travels with the command.
///
/// The reply is the registrar's, with `available: true`. A Checkout URL that
/// is not plain `https` is dropped rather than handed to the browser.
pub fn cmd_market_op<L: Losos>(l: &mut L, op: &crate::market::Op) -> anyhow::Result<Value> {
    use crate::market::{Op, Outcome, Refused};
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
    let Outcome::Reply(mut reply) = l.market_request(op)? else {
        return Err(Refused {
            status: 409,
            message: if op.is_builder() {
                "the widget builder is not offered to this appliance"
            } else {
                "the market is not offered to this appliance"
            }
            .to_string(),
        }
        .into());
    };
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

/// The widget builder as this box sees it: `GET /api/builder`.
///
/// The edge's account view (balance, packs, price, recent builds) with
/// `available: true`, or `available: false` and a reason. Gated like the
/// market: the proxy token goes to an official edge and no other, and the
/// builder is paid through the market's Stripe account.
pub fn cmd_builder<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    use crate::market::{Op, Outcome};
    if crate::edge::check_market_gate(&l.edge_status()?).is_err() {
        return Ok(json!({ "available": false, "reason": "noOfficialEdge" }));
    }
    match l.market_request(&Op::BuilderAccount)? {
        Outcome::Reply(mut view) => {
            if let Some(obj) = view.as_object_mut() {
                obj.insert("available".to_string(), json!(true));
            }
            Ok(view)
        }
        Outcome::Unavailable => Ok(json!({ "available": false })),
    }
}

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

/// Add or remove a custom domain. The reply is the edge's whole view, so the
/// page redraws from one answer.
pub fn cmd_domain_op<L: Losos>(l: &mut L, op: &crate::market::Op) -> anyhow::Result<Value> {
    use crate::market::{Op, Outcome, Refused};
    if !matches!(op, Op::DomainAdd { .. } | Op::DomainRemove { .. }) {
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

/// The live domains in an edge's domains view, as LosOS cloud should trust
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

/// Hand the live names to LosOS cloud and the relay pass to the announce
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

/// The shape backend-registrar's `routes::well_formed_pass` accepts:
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
    Ok(l.edge_status()?.to_json())
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
    Ok(json!({
        "enabled": repo.is_some(),
        "repository": repo.map(|r| json!({
            "owner": r.owner,
            "name": r.name,
            "url": r.page_url(),
            "clone": r.remote(),
            "viaForgejo": r.via_forgejo(),
        })),
        "head": head,
        "log": log,
        "sync": sync,
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
///  2. on Forgejo, the owner's account and the private repository are made
///     if missing (idempotent: "exists" counts as done);
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

    if repo.via_forgejo() {
        ensure_owner(l, repo, None)?;
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
    crate::edge::check_gate(&now, &parse_settings(body), &edge).map_err(|e| e.to_string())
}

fn ask<L: Losos>(
    l: &mut L,
    op: &crate::config_repo::ForgejoOp,
    secret: Option<&crate::setup::Secret>,
) -> anyhow::Result<crate::config_repo::Answer> {
    let (status, body) = l.forgejo_request(op, secret)?;
    Ok(crate::config_repo::classify(status, &body))
}

/// The owner's account on LosOS Git: made if missing (a site administrator,
/// with `secret` or a minted password), and given `secret` as its password
/// when one is passed.
fn ensure_owner<L: Losos>(
    l: &mut L,
    repo: &crate::config_repo::RepoConfig,
    secret: Option<&crate::setup::Secret>,
) -> anyhow::Result<()> {
    use crate::config_repo::{Answer, ForgejoOp};
    let login = repo.owner.clone();
    match ask(
        l,
        &ForgejoOp::GetUser {
            login: login.clone(),
        },
        None,
    )? {
        Answer::Ok(_) | Answer::Exists => {
            if let Some(secret) = secret {
                match ask(
                    l,
                    &ForgejoOp::SetPassword {
                        login: login.clone(),
                    },
                    Some(secret),
                )? {
                    Answer::Ok(_) => {}
                    other => anyhow::bail!("LosOS Git did not take {login}'s password: {other:?}"),
                }
            }
        }
        Answer::Missing => {
            let minted;
            let password = match secret {
                Some(s) => s,
                None => {
                    minted = l.mint_secret()?;
                    &minted
                }
            };
            match ask(
                l,
                &ForgejoOp::CreateUser {
                    login: login.clone(),
                },
                Some(password),
            )? {
                Answer::Ok(_) | Answer::Exists => {}
                Answer::Missing => anyhow::bail!("LosOS Git has no admin API at the expected path"),
                Answer::Refused { status, message } => {
                    anyhow::bail!("LosOS Git refused to create {login} ({status}): {message}")
                }
            }
            if let Answer::Refused { status, message } = ask(
                l,
                &ForgejoOp::MakeAdmin {
                    login: login.clone(),
                },
                None,
            )? {
                tracing::warn!(
                    status,
                    message,
                    "{login} was created on LosOS Git but not made an administrator"
                );
            }
        }
        Answer::Refused { status, message } => {
            anyhow::bail!("LosOS Git refused to look up {login} ({status}): {message}")
        }
    }
    Ok(())
}

/// The private configuration repository under the owner, made if missing.
fn ensure_repo<L: Losos>(
    l: &mut L,
    repo: &crate::config_repo::RepoConfig,
    branch: &str,
) -> anyhow::Result<()> {
    use crate::config_repo::{Answer, ForgejoOp};
    let (owner, name) = (repo.owner.clone(), repo.name.clone());
    match ask(
        l,
        &ForgejoOp::GetRepo {
            owner: owner.clone(),
            name: name.clone(),
        },
        None,
    )? {
        Answer::Ok(_) | Answer::Exists => return Ok(()),
        Answer::Missing => {}
        Answer::Refused { status, message } => {
            anyhow::bail!("LosOS Git refused to look up {owner}/{name} ({status}): {message}")
        }
    }
    match ask(
        l,
        &ForgejoOp::CreateRepo {
            owner: owner.clone(),
            name: name.clone(),
            branch: branch.to_string(),
        },
        None,
    )? {
        Answer::Ok(_) | Answer::Exists => Ok(()),
        Answer::Missing => anyhow::bail!("LosOS Git has no admin API at the expected path"),
        Answer::Refused { status, message } => {
            anyhow::bail!("LosOS Git refused to create {owner}/{name} ({status}): {message}")
        }
    }
}

/// Give the owner's LosOS Git account the password they just proved.
///
/// Called after a successful claim, set-password and sign-in. Best effort:
/// LosOS Git may be starting, or off, and neither is a reason to refuse the
/// thing that just succeeded. Only the configured owner's account is
/// touched; a password set for any other Nextcloud user is none of LosOS
/// Git's business.
pub fn sync_owner_password<L: Losos>(l: &mut L, user: &str, secret: &crate::setup::Secret) {
    let Some(repo) = l.config_repo() else {
        return;
    };
    if !repo.via_forgejo() || repo.owner != user {
        return;
    }
    match ensure_owner(l, &repo, Some(secret)) {
        Ok(()) => tracing::info!(user, "LosOS Git password kept in step"),
        Err(e) if e.downcast_ref::<crate::config_repo::NotUp>().is_some() => {
            tracing::info!(reason = %e, "LosOS Git password not updated: not up yet")
        }
        Err(e) => tracing::warn!(error = ?e, "LosOS Git password could not be updated"),
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
        assert_eq!(out["nextcloudMode"], "container");
        assert_eq!(out["forgejoMode"], "container");
        assert_eq!(out["hostName"], "mattbox");
        assert_eq!(out["apachePort"], 11000);
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
        let code = "{ ... }:\n{\n  losos.sharingMyStorage = false;\n  losos.hostName = \"box2\";\n  losos.nextcloud.mode = \"native\";\n  losos.nextcloud.apachePort = 12345;\n  losos.proxy.enable = true;\n}\n";
        cmd_apply(&mut f, code).unwrap();
        let out = cmd_settings(&mut f).unwrap();
        assert_eq!(out["hostName"], "box2");
        assert_eq!(out["nextcloudMode"], "native");
        assert_eq!(out["apachePort"], 12345);
        assert_eq!(out["proxyEnable"], true);
    }

    #[test]
    fn apply_then_settings_honours_the_legacy_aliases() {
        let mut f = FakeLosos::new();
        cmd_apply(&mut f, "{ ... }:\n{\n  losos.aio.apachePort = 12345;\n}\n").unwrap();
        assert_eq!(cmd_settings(&mut f).unwrap()["apachePort"], 12345);

        let mut g = FakeLosos::new();
        cmd_apply(
            &mut g,
            "{ ... }:\n{\n  losos.nextcloud.mode = \"aio\";\n}\n",
        )
        .unwrap();
        assert_eq!(cmd_settings(&mut g).unwrap()["nextcloudMode"], "container");
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
            fn nextcloud_mode(&mut self) -> anyhow::Result<crate::setup::NcMode> {
                self.0.nextcloud_mode()
            }
            fn nextcloud_target(
                &mut self,
                mode: crate::setup::NcMode,
            ) -> anyhow::Result<crate::setup::Target> {
                self.0.nextcloud_target(mode)
            }
            fn run_occ(
                &mut self,
                action: &crate::setup::OccAction,
                secret: &crate::setup::Secret,
            ) -> anyhow::Result<Option<crate::setup::OccOutcome>> {
                self.0.run_occ(action, secret)
            }
            fn nextcloud_status(
                &mut self,
                target: &crate::setup::Target,
            ) -> anyhow::Result<crate::setup::OccOutcome> {
                self.0.nextcloud_status(target)
            }
            fn nextcloud_last_log(
                &mut self,
                mode: crate::setup::NcMode,
            ) -> anyhow::Result<Option<String>> {
                self.0.nextcloud_last_log(mode)
            }
            fn nextcloud_login(
                &mut self,
                user: &str,
                secret: &crate::setup::Secret,
                client: &str,
            ) -> anyhow::Result<crate::signin::LoginOutcome> {
                self.0.nextcloud_login(user, secret, client)
            }
            fn recovery_code(&mut self) -> anyhow::Result<crate::recovery::Recovery> {
                self.0.recovery_code()
            }
            fn search_apps(&mut self, query: &str) -> anyhow::Result<Vec<crate::catalogue::App>> {
                self.0.search_apps(query)
            }
            fn market_request(
                &mut self,
                op: &crate::market::Op,
            ) -> anyhow::Result<crate::market::Outcome> {
                self.0.market_request(op)
            }
            fn edge_status(&mut self) -> anyhow::Result<crate::edge::EdgeStatus> {
                self.0.edge_status()
            }
            fn write_public_names(&mut self, names: &[String]) -> anyhow::Result<()> {
                self.0.write_public_names(names)
            }
            fn write_relay_pass(&mut self, pass: Option<&str>) -> anyhow::Result<()> {
                self.0.write_relay_pass(pass)
            }
            fn load_look(&mut self) -> anyhow::Result<crate::look::Look> {
                self.0.load_look()
            }
            fn save_look(&mut self, look: &crate::look::Look) -> anyhow::Result<()> {
                self.0.save_look(look)
            }
            fn write_background(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
                self.0.write_background(bytes)
            }
            fn read_background(&mut self) -> anyhow::Result<Option<Vec<u8>>> {
                self.0.read_background()
            }
            fn remove_background(&mut self) -> anyhow::Result<()> {
                self.0.remove_background()
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
            fn forgejo_request(
                &mut self,
                op: &crate::config_repo::ForgejoOp,
                s: Option<&crate::setup::Secret>,
            ) -> anyhow::Result<(u16, String)> {
                self.0.forgejo_request(op, s)
            }
            fn mint_secret(&mut self) -> anyhow::Result<crate::setup::Secret> {
                self.0.mint_secret()
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
    fn the_builder_view_is_the_edges_account_or_unavailable() {
        use crate::market::Op;
        let mut f = FakeLosos::new();
        assert_eq!(
            cmd_builder(&mut f).unwrap(),
            serde_json::json!({ "available": false })
        );
        f.market_routes.insert(
            "POST /builder/account".to_string(),
            (200, r#"{"currency":"eur","balance":380}"#.to_string()),
        );
        let out = cmd_builder(&mut f).unwrap();
        assert_eq!(out["available"], true);
        assert_eq!(out["balance"], 380);

        let mut f = FakeLosos::new().with_company_edge();
        assert_eq!(
            cmd_builder(&mut f).unwrap(),
            serde_json::json!({ "available": false, "reason": "noOfficialEdge" })
        );
        assert!(cmd_market_op(&mut f, &Op::BuilderCredit { amount: 500 }).is_err());
        assert!(f.market_ops.is_empty(), "no token to a company edge");
    }

    #[test]
    fn a_top_up_is_sent_only_to_a_stripe_hosted_page() {
        use crate::market::Op;
        let mut f = FakeLosos::new();
        f.market_routes.insert(
            "POST /builder/credits".to_string(),
            (
                201,
                r#"{"order_id":"cr_1","checkout_url":"https://evil.example/pay"}"#.to_string(),
            ),
        );
        assert!(cmd_market_op(&mut f, &Op::BuilderCredit { amount: 500 }).is_err());
        f.market_routes.insert(
            "POST /builder/credits".to_string(),
            (
                201,
                r#"{"order_id":"cr_1","checkout_url":"https://checkout.stripe.com/c/pay/cs_1"}"#
                    .to_string(),
            ),
        );
        let out = cmd_market_op(&mut f, &Op::BuilderCredit { amount: 500 }).unwrap();
        assert_eq!(out["order_id"], "cr_1");
        // A builder the edge does not run says so in its own words.
        let err = cmd_market_op(
            &mut f,
            &Op::BuilderBuild {
                build_id: "bld_1".to_string(),
            },
        )
        .unwrap_err();
        assert_eq!(
            err.downcast_ref::<crate::market::Refused>()
                .unwrap()
                .message,
            "the widget builder is not offered to this appliance"
        );
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
        let err = cmd_apply(&mut f, "{ losos.forgejo.enable = \"yes\"; }").unwrap_err();
        let rejected = err
            .downcast_ref::<crate::options::Rejected>()
            .expect("typed, so the HTTP layer answers 400");
        assert_eq!(rejected.0, "losos.forgejo.enable must be true or false");
        assert!(!f.spawned, "a refused apply rebuilds nothing");
        assert_eq!(f.commits.len(), 1, "and commits nothing");

        let err = cmd_apply(&mut f, "{ losos.tpm.enable = false; }").unwrap_err();
        assert!(err.downcast_ref::<crate::options::Rejected>().is_some());

        cmd_apply(&mut f, "{ losos.forgejo.enable = false; }").unwrap();
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
        cmd_apply(&mut f, "{ losos.forgejo.enable = false; }").unwrap();
        let out = cmd_options(&mut f).unwrap();
        assert_eq!(out["available"], true);
        let opts = out["options"].as_array().unwrap();
        let fe = opts.iter().find(|o| o["name"] == "forgejo.enable").unwrap();
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
        assert_eq!(out["repository"]["url"], "/forgejo/notshared/losos-config");
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
            "{ ... }:\n{\n  losos.sharingMyStorage = true;\n  losos.forgejo.enable = false;\n}\n"
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
            .contains("losos.forgejo.enable = false;"));
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
        f.remote_overrides = Some("{ losos.forgejo.enable = \"yes\"; }".to_string());
        let out = cmd_config_sync(&mut f).unwrap();
        assert_eq!(out["sync"]["state"], "refused");
        assert!(out["sync"]["detail"]
            .as_str()
            .unwrap()
            .contains("losos.forgejo.enable must be true or false"));
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
        f.remote_overrides = Some("{ losos.forgejo.enable = false; }".to_string());
        f.state.rebuild = Some(building("job-9", MSG_STARTED));
        let out = cmd_config_sync(&mut f).unwrap();
        assert_eq!(out["sync"]["state"], "waiting");
        assert!(f.fast_forwarded.is_empty());
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
    fn sync_creates_the_owner_and_the_repository_when_missing() {
        let mut f = FakeLosos::new().with_config_repo();
        f.forgejo_routes.insert(
            "GET /api/v1/users/notshared".into(),
            (404, r#"{"message":"user does not exist"}"#.into()),
        );
        f.forgejo_routes.insert(
            "POST /api/v1/admin/users".into(),
            (201, r#"{"id": 2}"#.into()),
        );
        f.forgejo_routes.insert(
            "GET /api/v1/repos/notshared/losos-config".into(),
            (404, r#"{"message":"not found"}"#.into()),
        );
        f.forgejo_routes.insert(
            "POST /api/v1/admin/users/notshared/repos".into(),
            (201, r#"{"id": 1}"#.into()),
        );
        let out = cmd_config_sync(&mut f).unwrap();
        assert_eq!(out["sync"]["state"], "ok", "{}", out["sync"]["detail"]);
        let calls: Vec<&str> = f.forgejo_calls.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            calls,
            vec![
                "GET /api/v1/users/notshared",
                "POST /api/v1/admin/users",
                "PATCH /api/v1/admin/users/notshared",
                "GET /api/v1/repos/notshared/losos-config",
                "POST /api/v1/admin/users/notshared/repos",
            ]
        );
        // The account was made with a minted password nobody will type, and
        // nothing but the create carried one.
        assert_eq!(
            f.forgejo_calls[1].1.as_deref(),
            Some("minted-fake-password-0123")
        );
        assert!(f.forgejo_calls.iter().filter(|(_, p)| p.is_some()).count() == 1);
        assert_eq!(f.pushed.len(), 1);
        // "Exists" is as good as made: a repeat creates nothing twice.
        f.forgejo_routes.insert(
            "POST /api/v1/admin/users".into(),
            (
                422,
                r#"{"message":"user already exists [name: notshared]"}"#.into(),
            ),
        );
        f.forgejo_routes.insert(
            "POST /api/v1/admin/users/notshared/repos".into(),
            (
                409,
                r#"{"message":"The repository with the same name already exists."}"#.into(),
            ),
        );
        let out = cmd_config_sync(&mut f).unwrap();
        assert_eq!(out["sync"]["state"], "ok");
    }

    #[test]
    fn a_refusal_from_losos_git_is_reported_not_raised() {
        let mut f = FakeLosos::new().with_config_repo();
        f.forgejo_routes.insert(
            "GET /api/v1/users/notshared".into(),
            (
                403,
                r#"{"message":"token does not have at least one of required scope(s)"}"#.into(),
            ),
        );
        let out = cmd_config_sync(&mut f).unwrap();
        assert_eq!(out["sync"]["state"], "error");
        assert!(out["sync"]["detail"]
            .as_str()
            .unwrap()
            .contains("required scope"));
    }

    #[test]
    fn a_sign_in_gives_losos_git_the_same_password() {
        let mut f = FakeLosos::new().with_config_repo();
        f.accepted_password = Some("correct horse battery staple".to_string());
        cmd_sign_in(&mut f, "correct horse battery staple", "tok", "192.168.1.9").unwrap();
        let set = f
            .forgejo_calls
            .iter()
            .find(|(k, _)| k == "PATCH /api/v1/admin/users/notshared")
            .expect("the password was set");
        assert_eq!(set.1.as_deref(), Some("correct horse battery staple"));
        // A wrong password sets nothing.
        f.forgejo_calls.clear();
        assert!(cmd_sign_in(&mut f, "wrong horse", "tok", "192.168.1.9").is_err());
        assert!(f.forgejo_calls.is_empty());
    }

    #[test]
    fn set_password_gives_losos_git_the_same_password() {
        let mut f = FakeLosos::new().with_config_repo();
        cmd_set_password(&mut f, "notshared", "Correct-Horse-Battery-9").unwrap();
        let set = f
            .forgejo_calls
            .iter()
            .find(|(k, _)| k == "PATCH /api/v1/admin/users/notshared")
            .expect("the password was set");
        assert_eq!(set.1.as_deref(), Some("Correct-Horse-Battery-9"));
        // Another Nextcloud user is none of LosOS Git's business.
        f.forgejo_calls.clear();
        cmd_set_password(&mut f, "someone", "Correct-Horse-Battery-9").unwrap();
        assert!(f.forgejo_calls.is_empty());
    }

    #[test]
    fn a_sign_in_still_succeeds_while_losos_git_is_down() {
        let mut f = FakeLosos::new();
        f.repo = FakeLosos::new().with_config_repo().repo;
        f.accepted_password = Some("correct horse battery staple".to_string());
        let out =
            cmd_sign_in(&mut f, "correct horse battery staple", "tok", "192.168.1.9").unwrap();
        assert_eq!(out["token"], "tok");
    }
}
