//! In-memory [`Losos`] implementation for tests.
//!
//! The counterpart of the Haskell `TestM` interpreter: it lets the command
//! functions run with no filesystem, no `systemd-run` and no clock, so the
//! state machine is tested deterministically.
//!
//! One simplification is inherited from `TestM` and worth stating plainly:
//! [`FakeLosos::config`] stands in for *both* Nix files. In production
//! `config` stands in for `overrides.nix`, the one file every command that
//! changes a setting writes: `apply` and `factory-reset` replace it, `change`
//! line-patches it through `read_overrides` + `write_overrides`.

use crate::losos::Losos;
use crate::model::State;
use crate::overrides::DEFAULT_OVERRIDES_NIX;

/// A fake world: everything the commands can observe or mutate.
#[derive(Debug, Clone)]
pub struct FakeLosos {
    pub state: State,
    /// Stands in for `overrides.nix` (see the module docs).
    pub config: Vec<String>,
    /// Lines the "rebuild log" would contain.
    pub log: Vec<String>,
    pub job_counter: u32,
    pub spawned: bool,
    /// Unallocated extents the fake volume group reports.
    pub vg_free_extents: u64,
    /// Size of the fake `/persist`, in bytes.
    pub persist_bytes: u64,
    /// Every grow step that was executed, in order. The ordering is the thing
    /// under test, so the fake records rather than simulates.
    pub grow_ran: Vec<crate::grow::GrowAction>,
    /// Key file handed to `cryptsetup resize`; None models the TPM path.
    pub luks_key_file: Option<String>,

    // ── Setting the Nextcloud admin password ────────────────────────────
    /// Mode the fake appliance reports running in.
    pub nextcloud_mode: crate::setup::NcMode,
    /// Container id `crictl ps` would return. `None` models a pod that is not
    /// running yet — the common case on a box in its first few minutes.
    pub nextcloud_container: Option<String>,
    /// Every `occ` step that was executed, in order. Recorded rather than
    /// simulated, so a test can assert the order and — the point of the whole
    /// module — inspect every argv for the password.
    pub occ_ran: Vec<crate::setup::OccAction>,
    /// Content of the staged secret file, or `None` once it has been cleared.
    /// A test reads this to prove the password travelled by file rather than by
    /// argv, and that it does not survive the command.
    pub staged_secret: Option<String>,
    /// Exit code the fake `occ` reports.
    pub occ_exit: i32,
    pub occ_stdout: String,
    pub occ_stderr: String,
    /// Make `run_occ` fail to *spawn* the exec step, modelling a missing
    /// `crictl` or an unreachable CRI socket — which is what the cleanup path
    /// has to survive.
    pub occ_spawn_fails: bool,

    // ── The appliance recovery code ─────────────────────────────────────
    /// In-memory stand-in for `/var/secrets/losos-recovery-code`. Public so a
    /// test can seed it with [`crate::recovery::MemoryStore::with_file`] or
    /// count its `writes` to prove the code is minted exactly once.
    pub recovery: crate::recovery::MemoryStore,

    // ── Searching the app catalogue ─────────────────────────────────────
    /// Body a fake catalogue hands back. `None` models the fetch not happening
    /// at all — no route off the box, DNS unanswered, the catalogue down —
    /// which is the case the screen has to keep working through.
    pub catalogue_body: Option<String>,
    /// Every query the fake was asked for, in order. A test reads this to
    /// prove the command trimmed before searching rather than after.
    pub catalogue_queries: Vec<String>,

    // ── The market ──────────────────────────────────────────────────────
    /// Registrar answers by `METHOD path`, as `(status, body)`. A route with no
    /// entry models a box with no registrar configured.
    pub market_routes: std::collections::BTreeMap<String, (u16, String)>,
    /// Every operation asked for, in order.
    pub market_ops: Vec<crate::market::Op>,
}

impl FakeLosos {
    /// A fresh appliance: default state, the committed overrides body, no log.
    pub fn new() -> Self {
        FakeLosos {
            state: State::default(),
            config: DEFAULT_OVERRIDES_NIX.lines().map(str::to_string).collect(),
            log: Vec::new(),
            job_counter: 0,
            spawned: false,
            // 512 extents of the lvm2 default 4 MiB = 2 GiB of headroom, on a
            // 20 GiB filesystem. Both are arbitrary; what matters is that they
            // are non-zero so the happy path is the default.
            vg_free_extents: 512,
            persist_bytes: 20 * 1024 * 1024 * 1024,
            grow_ran: Vec::new(),
            luks_key_file: None,
            // Container is the appliance's default (losos.nextcloud.mode), so
            // it is the default here too: the mode most tests should exercise
            // is the one most boxes run.
            nextcloud_mode: crate::setup::NcMode::Container,
            nextcloud_container: Some("c0ffee1234".to_string()),
            occ_ran: Vec::new(),
            staged_secret: None,
            occ_exit: 0,
            occ_stdout: "Successfully reset password for notshared".to_string(),
            occ_stderr: String::new(),
            occ_spawn_fails: false,
            // Empty, so the default fake is a fresh appliance that has never
            // minted a code — the state the first-run wizard actually meets.
            recovery: crate::recovery::MemoryStore::empty(),
            // One row, so the happy path is the default. Deliberately carries
            // an organisation rather than only a repository name, because that
            // is the field the row must be attributable by.
            catalogue_body: Some(
                r#"{"packages":[{"package_id":"a1b2","name":"nextcloud",
                   "normalized_name":"nextcloud","display_name":"Nextcloud",
                   "description":"A safe home for all your data","version":"6.6.10",
                   "repository":{"name":"nextcloud","organization_display_name":"Nextcloud GmbH"}}]}"#
                    .to_string(),
            ),
            catalogue_queries: Vec::new(),
            market_routes: std::collections::BTreeMap::new(),
            market_ops: Vec::new(),
        }
    }
}

impl Default for FakeLosos {
    fn default() -> Self {
        Self::new()
    }
}

impl Losos for FakeLosos {
    fn load_state(&mut self) -> anyhow::Result<State> {
        Ok(self.state.clone())
    }

    fn save_state(&mut self, s: &State) -> anyhow::Result<()> {
        self.state = s.clone();
        Ok(())
    }

    fn write_overrides(&mut self, body: &str) -> anyhow::Result<()> {
        self.config = body.lines().map(str::to_string).collect();
        Ok(())
    }

    fn read_overrides(&mut self) -> anyhow::Result<String> {
        let mut s = self.config.join("\n");
        s.push('\n');
        Ok(s)
    }

    fn spawn_rebuild(&mut self, _job: &str) -> anyhow::Result<()> {
        // No watcher is simulated: the Building record was already written by
        // the command itself before it got here.
        self.spawned = true;
        Ok(())
    }

    fn rebuild_log_tail(&mut self) -> anyhow::Result<String> {
        Ok(crate::supervisor::last_log_line(&self.log))
    }

    fn next_job_id(&mut self) -> anyhow::Result<String> {
        let id = format!("job-{}", self.job_counter);
        self.job_counter += 1;
        Ok(id)
    }

    /// The real decision, run against the in-memory store.
    ///
    /// Deliberately [`crate::recovery::ensure_code`] rather than a hand-written
    /// stub: the mint-once behaviour is the thing under test, and a fake that
    /// reimplemented it would prove only that the fake agrees with itself.
    fn recovery_code(&mut self) -> anyhow::Result<crate::recovery::Recovery> {
        crate::recovery::ensure_code(&mut self.recovery)
    }

    /// The real parser, run against a recorded body — same reasoning as
    /// `recovery_code` above: a fake that reimplemented the mapping would
    /// prove only that the fake agrees with itself.
    fn search_apps(&mut self, query: &str) -> anyhow::Result<Vec<crate::catalogue::App>> {
        self.catalogue_queries.push(query.to_string());
        let body = self
            .catalogue_body
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("the catalogue could not be reached"))?;
        crate::catalogue::parse_results(body)
    }

    /// The real classifier, run against a scripted registrar.
    fn market_request(&mut self, op: &crate::market::Op) -> anyhow::Result<crate::market::Outcome> {
        self.market_ops.push(op.clone());
        let (method, path) = op.route();
        match self.market_routes.get(&format!("{method} {path}")) {
            None => Ok(crate::market::Outcome::Unavailable),
            Some((status, body)) => crate::market::classify(*status, body),
        }
    }

    fn vg_free(&mut self) -> anyhow::Result<crate::grow::VgFree> {
        Ok(crate::grow::VgFree {
            free_extents: self.vg_free_extents,
            extent_bytes: 4 * 1024 * 1024,
        })
    }

    fn run_grow(&mut self, action: &crate::grow::GrowAction) -> anyhow::Result<()> {
        // Recorded, not simulated — except for the one effect the command
        // actually reads back. `ResizeFs` is what makes the filesystem bigger,
        // so only that step moves `persist_bytes`; a plan that ran the steps in
        // the wrong order would still "work" here, which is precisely why the
        // ordering is asserted against the plan in grow.rs rather than here.
        self.grow_ran.push(action.clone());
        if matches!(action, crate::grow::GrowAction::ResizeFs) {
            let claimed = self.vg_free_extents.saturating_mul(4 * 1024 * 1024);
            self.persist_bytes = self.persist_bytes.saturating_add(claimed);
            self.vg_free_extents = 0;
        }
        Ok(())
    }

    fn persist_bytes(&mut self) -> anyhow::Result<u64> {
        Ok(self.persist_bytes)
    }

    fn luks_key_file(&mut self) -> anyhow::Result<Option<String>> {
        Ok(self.luks_key_file.clone())
    }

    fn nextcloud_mode(&mut self) -> anyhow::Result<crate::setup::NcMode> {
        Ok(self.nextcloud_mode)
    }

    fn nextcloud_target(
        &mut self,
        mode: crate::setup::NcMode,
    ) -> anyhow::Result<crate::setup::Target> {
        match mode {
            crate::setup::NcMode::Native => Ok(crate::setup::Target::Native),
            crate::setup::NcMode::Container => {
                // Runs the real parser over what `crictl ps --quiet` would have
                // printed, so "zero or two matches is an error" is genuinely
                // covered instead of stubbed into always succeeding.
                let stdout = self.nextcloud_container.clone().unwrap_or_default();
                let id = crate::setup::parse_container_ids(&stdout)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                Ok(crate::setup::Target::Container {
                    socket: crate::setup::DEFAULT_CRI_SOCKET.to_string(),
                    id,
                })
            }
        }
    }

    fn run_occ(
        &mut self,
        action: &crate::setup::OccAction,
        secret: &crate::setup::Secret,
    ) -> anyhow::Result<Option<crate::setup::OccOutcome>> {
        use crate::setup::OccAction;
        self.occ_ran.push(action.clone());
        match action {
            // The one effect a later step reads back: the secret has to be in
            // the file before the exec, and gone after the clear.
            OccAction::StageSecret { .. } => {
                self.staged_secret = Some(secret.expose().to_string());
                Ok(None)
            }
            OccAction::ClearSecret { .. } => {
                self.staged_secret = None;
                Ok(None)
            }
            OccAction::RunOcc { .. } if self.occ_spawn_fails => {
                Err(anyhow::anyhow!("crictl: No such file or directory"))
            }
            OccAction::RunOcc { .. } => Ok(Some(crate::setup::OccOutcome {
                code: self.occ_exit,
                stdout: self.occ_stdout.clone(),
                stderr: self.occ_stderr.clone(),
            })),
        }
    }
}
