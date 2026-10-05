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
    let lines: Vec<String> = l.read_overrides()?.lines().map(str::to_string).collect();
    let mut body = crate::overrides::inject_line(sharing, &lines).join("\n");
    body.push('\n');
    l.write_overrides(&body)?;
    let job = l.next_job_id()?;
    let mut s = l.load_state()?;
    s.mode = mode;
    s.sharing = sharing;
    s.rebuild = Some(building(&job, MSG_STARTED));
    l.save_state(&s)?;
    l.spawn_rebuild(&job)?;
    Ok(json!({ "job": job }))
}

/// Overwrite `overrides.nix` with an already-validated body and rebuild.
///
/// Mode and sharing are left exactly as they were: applying settings is not a
/// posture change.
pub fn cmd_apply<L: Losos>(l: &mut L, nix_code: &str) -> anyhow::Result<Value> {
    l.write_overrides(nix_code)?;
    let job = l.next_job_id()?;
    let mut s = l.load_state()?;
    s.rebuild = Some(building(&job, MSG_STARTED));
    l.save_state(&s)?;
    l.spawn_rebuild(&job)?;
    Ok(json!({ "job": job }))
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
    l.write_overrides(DEFAULT_OVERRIDES_NIX)?;
    let job = l.next_job_id()?;
    // `claimed` is carried over rather than reset, and this is the one field of
    // State that a factory reset must not touch. Resetting the settings an
    // owner chose is not the same as forgetting that the box has an owner: the
    // claim route is unauthenticated while `claimed` is false, so re-opening it
    // here would mean a reset performed from the admin UI hands the next person
    // on that LAN an unguarded password prompt. The destructive tier that truly
    // makes the box unowned is the installer ISO, which wipes /persist and takes
    // state.json with it.
    let claimed = l.load_state().map(|s| s.claimed).unwrap_or(false);
    let s = State {
        rebuild: Some(building(&job, MSG_RESET_STARTED)),
        claimed,
        ..State::default()
    };
    l.save_state(&s)?;
    l.spawn_rebuild(&job)?;
    Ok(json!({ "job": job, "reset": true }))
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
    let target = match l.nextcloud_mode().and_then(|m| l.nextcloud_target(m)) {
        Ok(t) => t,
        Err(e) => {
            tracing::info!(error = ?e, "Nextcloud not located yet");
            return Readiness::NotYet(
                "Nextcloud has not started yet. On a new box this takes a few minutes.".to_string(),
            );
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
pub fn cmd_claim<L: Losos>(
    l: &mut L,
    user: &str,
    password: &str,
    token: &str,
) -> anyhow::Result<Value> {
    // Fail closed: if the state cannot be read, nobody gets to claim the box.
    let mut state = l
        .load_state()
        .map_err(|e| e.context("reading the state before a claim"))?;
    if state.claimed {
        anyhow::bail!("this box has already been set up");
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

    Ok(json!({
        "claimed": true,
        "user": out.get("user").cloned().unwrap_or(Value::Null),
        "token": token,
    }))
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
            message: "the market is not offered to this appliance".to_string(),
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
        }

        let mut inert = Inert(FakeLosos::new());
        let out = cmd_grow(&mut inert).unwrap();
        assert_eq!(out["grew"], false, "a no-op grow must not report success");
        assert_eq!(out["beforeBytes"], out["afterBytes"]);
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
}
