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
    /// Of that, in use: what `GET /api/storage` reports as `usedBytes`.
    pub persist_used_bytes: u64,
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
    /// What the fake `occ status --output=json` prints and exits with. The
    /// default is an installed, serving instance; a test that wants the
    /// first-boot window sets `installed: false` or a non-zero exit.
    pub status_exit: i32,
    pub status_stdout: String,
    pub status_stderr: String,
    /// Make the status probe fail to spawn, like `occ_spawn_fails`.
    pub status_spawn_fails: bool,
    /// Every readiness probe that ran, with the target it was aimed at.
    pub status_probed: Vec<crate::setup::Target>,
    /// What `crictl logs` of the latest container would print; `None` is
    /// "no container has ever run".
    pub last_log: Option<String>,
    /// How often the last log was asked for — a running pod must never be.
    pub last_log_asked: usize,
    /// The password the fake Nextcloud accepts for the admin account, or
    /// `None` for a Nextcloud that cannot be asked at all (the pod not up
    /// yet). A sign-in attempt compares against this; nothing else does.
    pub accepted_password: Option<String>,
    /// Model Nextcloud's brute-force protection having shut the door.
    pub login_throttled: bool,
    /// Every sign-in probe that was made: the account asked about and the
    /// client address forwarded, in order. A test reads this to prove the
    /// probe names the one admin account and carries the browser's address.
    pub login_asked: Vec<(String, String)>,

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
    /// The live custom domains last handed to LosOS cloud; `None` until the
    /// first write.
    pub public_names: Option<Vec<String>>,
    /// The last relay pass written: `None` never written, `Some(None)`
    /// removed.
    pub relay_pass: Option<Option<String>>,

    // ── Finding an edge proxy ───────────────────────────────────────────
    /// What the fake scanner last found. The default has one LAN edge in
    /// reach, so the happy path is the default and a test of the gate puts
    /// an empty scan here on purpose.
    pub edge: crate::edge::EdgeStatus,
    /// How often the status was asked for: the gate must read it on every
    /// turn-on, and a read-only command must not.
    pub edge_asked: usize,
    // ── The owner's look ────────────────────────────────────────────────
    /// Stands in for `look.json`.
    pub look: crate::look::Look,
    /// Stands in for the uploaded picture's file; `None` is no file.
    pub background: Option<Vec<u8>>,

    // ── The option document and the configuration repository ───────────
    /// Stands in for `/etc/losos/options.json`; `None` is a box without it.
    pub options_doc: Option<crate::options::OptionsDoc>,
    /// Where the repository is published; `None` is the feature off.
    pub repo: Option<crate::config_repo::RepoConfig>,
    pub branch: String,
    /// The local history, oldest first. A fresh fake has the installer's
    /// one commit.
    pub commits: Vec<FakeCommit>,
    /// Whether the working tree has changes a commit would pick up. Set by
    /// `write_overrides`, cleared by `config_commit`.
    pub dirty: bool,
    /// The remote branch's history, oldest first, by sha. Shas that are not
    /// in `commits` are commits made elsewhere (a push from a clone).
    pub remote_commits: Vec<String>,
    /// What `git show <remote head>:modules/overrides.nix` prints.
    pub remote_overrides: Option<String>,
    /// Model a remote that cannot be reached.
    pub fetch_fails: bool,
    /// Every push made, by URL.
    pub pushed: Vec<String>,
    /// Every fast-forward made, by target sha.
    pub fast_forwarded: Vec<String>,
    /// LosOS Git's answers by `METHOD path`; a route with no entry models a
    /// LosOS Git that is not up.
    pub forgejo_routes: std::collections::BTreeMap<String, (u16, String)>,
    /// Every request made, as `METHOD path` and the password it carried.
    pub forgejo_calls: Vec<(String, Option<String>)>,
    pub sync_report: Option<crate::config_repo::SyncReport>,
    /// Numbers the next commit the fake makes.
    pub commit_seq: u32,

    // ── Backups and erasing the box ─────────────────────────────────────
    /// Stands in for `/var/secrets/losos-backup.json`.
    pub backup_target: Option<crate::backup::Target>,
    /// Every backup or restore unit started, by unit name, in order.
    pub units_started: Vec<String>,
    /// Every one stopped.
    pub units_stopped: Vec<String>,
    /// What polling any job's unit answers. Running by default.
    pub unit_poll: crate::supervisor::Poll,
    /// The last line of the backup log.
    pub backup_log: String,
    pub backup_report: Option<crate::backup::Report>,
    /// Stands in for the restore unit's code file.
    pub restore_code: Option<String>,
    /// What a restore staged as `overrides.nix`.
    pub restored_overrides: Option<String>,
    /// Unix seconds; tests move it by hand.
    pub clock: u64,
    /// The erase countdown, 15 minutes like the default option.
    pub grace_secs: u64,
    /// Whether the box was told to wipe and reboot.
    pub rebooted: bool,
    pub erase_report: Option<crate::erase::Outside>,
}

/// One commit in the fake's history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeCommit {
    pub sha: String,
    pub subject: String,
    pub body: String,
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
            persist_used_bytes: 8 * 1024 * 1024 * 1024,
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
            status_exit: 0,
            status_stdout: r#"{"installed":true,"version":"34.0.2.1","versionstring":"34.0.2","edition":"","maintenance":false,"needsDbUpgrade":false,"productname":"Nextcloud","extendedSupport":false}"#.to_string(),
            status_stderr: String::new(),
            status_spawn_fails: false,
            status_probed: Vec::new(),
            last_log: None,
            last_log_asked: 0,
            // A reachable Nextcloud whose password nobody knows yet, like a
            // fresh box: a sign-in has to fail until a test sets one.
            accepted_password: None,
            login_throttled: false,
            login_asked: Vec::new(),
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
            public_names: None,
            relay_pass: None,
            edge: crate::edge::EdgeStatus::found(
                vec![crate::edge::Edge {
                    name: "edge".to_string(),
                    url: "http://edge.local:8443".to_string(),
                    source: crate::edge::Source::Lan,
                    official: true,
                    rathole: Some("edge.local:2333".to_string()),
                }],
                true,
                None,
            ),
            edge_asked: 0,
            look: crate::look::Look::default(),
            background: None,
            options_doc: None,
            repo: None,
            branch: "main".to_string(),
            commits: vec![FakeCommit {
                sha: Self::sha(0),
                subject: "losos install".to_string(),
                body: String::new(),
            }],
            dirty: false,
            remote_commits: Vec::new(),
            remote_overrides: None,
            fetch_fails: false,
            pushed: Vec::new(),
            fast_forwarded: Vec::new(),
            forgejo_routes: std::collections::BTreeMap::new(),
            forgejo_calls: Vec::new(),
            sync_report: None,
            commit_seq: 1,
            backup_target: None,
            units_started: Vec::new(),
            units_stopped: Vec::new(),
            unit_poll: crate::supervisor::Poll::Wait,
            backup_log: String::new(),
            backup_report: None,
            restore_code: None,
            restored_overrides: None,
            clock: 1_700_000_000,
            grace_secs: 900,
            rebooted: false,
            erase_report: None,
        }
    }

    /// A fake whose scan found nothing: the box with no edge in reach.
    #[must_use]
    pub fn without_edge(mut self) -> Self {
        self.edge = crate::edge::EdgeStatus::found(Vec::new(), true, None);
        self
    }

    /// An edge in reach that proved no LosOS identity: a company's own.
    /// Sharing is allowed through it, trading is not.
    #[must_use]
    pub fn with_company_edge(mut self) -> Self {
        for e in &mut self.edge.edges {
            e.official = false;
        }
        self.edge.official = false;
        self
    }

    /// A readable 40-character sha for commit number `n`.
    pub fn sha(n: u32) -> String {
        format!("{n:040x}")
    }

    /// The commits a sha has behind it (and itself), from whichever side of
    /// the fake knows it; empty for a sha nobody has.
    fn history(&self, sha: &str) -> Vec<String> {
        if let Some(i) = self.commits.iter().position(|c| c.sha == sha) {
            return self.commits[..=i].iter().map(|c| c.sha.clone()).collect();
        }
        if let Some(i) = self.remote_commits.iter().position(|c| c == sha) {
            return self.remote_commits[..=i].to_vec();
        }
        Vec::new()
    }

    /// The configuration repository switched on, pointing at a LosOS Git
    /// that answers "exists" for the owner and the repository.
    pub fn with_config_repo(mut self) -> Self {
        self.repo = Some(crate::config_repo::RepoConfig {
            owner: "notshared".to_string(),
            name: "losos-config".to_string(),
            forgejo_url: "http://127.0.0.1:3000".to_string(),
            token_file: "/var/lib/forgejo/.losos-token".to_string(),
            remote_url: None,
        });
        self.forgejo_routes.insert(
            "GET /api/v1/users/notshared".to_string(),
            (200, r#"{"id": 2, "login": "notshared"}"#.to_string()),
        );
        self.forgejo_routes.insert(
            "GET /api/v1/repos/notshared/losos-config".to_string(),
            (200, r#"{"id": 1, "name": "losos-config"}"#.to_string()),
        );
        self.forgejo_routes.insert(
            "PATCH /api/v1/admin/users/notshared".to_string(),
            (200, r#"{"id": 2}"#.to_string()),
        );
        self
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
        let lines: Vec<String> = body.lines().map(str::to_string).collect();
        // Like a working tree: rewriting a file with the same bytes is not a
        // change, and a commit of it would be nothing to commit.
        if lines != self.config {
            self.dirty = true;
        }
        self.config = lines;
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
    fn nextcloud_login(
        &mut self,
        user: &str,
        secret: &crate::setup::Secret,
        client: &str,
    ) -> anyhow::Result<crate::signin::LoginOutcome> {
        use crate::signin::LoginOutcome;
        self.login_asked
            .push((user.to_string(), client.to_string()));
        if self.login_throttled {
            return Ok(LoginOutcome::Throttled);
        }
        match &self.accepted_password {
            None => anyhow::bail!("connection refused (the fake Nextcloud is not up)"),
            Some(p) if p == secret.expose() => Ok(LoginOutcome::Accepted),
            Some(_) => Ok(LoginOutcome::Rejected),
        }
    }

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
    fn edge_status(&mut self) -> anyhow::Result<crate::edge::EdgeStatus> {
        self.edge_asked += 1;
        Ok(self.edge.clone())
    }

    fn market_request(&mut self, op: &crate::market::Op) -> anyhow::Result<crate::market::Outcome> {
        self.market_ops.push(op.clone());
        let (method, path) = op.route();
        match self.market_routes.get(&format!("{method} {path}")) {
            None => Ok(crate::market::Outcome::Unavailable),
            Some((status, body)) => crate::market::classify(*status, body),
        }
    }

    fn write_relay_pass(&mut self, pass: Option<&str>) -> anyhow::Result<()> {
        self.relay_pass = Some(pass.map(str::to_string));
        Ok(())
    }

    fn write_public_names(&mut self, names: &[String]) -> anyhow::Result<()> {
        self.public_names = Some(names.to_vec());
        Ok(())
    }

    fn load_look(&mut self) -> anyhow::Result<crate::look::Look> {
        Ok(self.look.clone())
    }

    fn save_look(&mut self, look: &crate::look::Look) -> anyhow::Result<()> {
        self.look = look.clone();
        Ok(())
    }

    fn write_background(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        self.background = Some(bytes.to_vec());
        Ok(())
    }

    fn read_background(&mut self) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(self.background.clone())
    }

    fn remove_background(&mut self) -> anyhow::Result<()> {
        self.background = None;
        Ok(())
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

    fn persist_used_bytes(&mut self) -> anyhow::Result<u64> {
        Ok(self.persist_used_bytes)
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

    fn nextcloud_status(
        &mut self,
        target: &crate::setup::Target,
    ) -> anyhow::Result<crate::setup::OccOutcome> {
        self.status_probed.push(target.clone());
        if self.status_spawn_fails {
            anyhow::bail!("crictl: No such file or directory");
        }
        Ok(crate::setup::OccOutcome {
            code: self.status_exit,
            stdout: self.status_stdout.clone(),
            stderr: self.status_stderr.clone(),
        })
    }

    fn nextcloud_last_log(
        &mut self,
        _mode: crate::setup::NcMode,
    ) -> anyhow::Result<Option<String>> {
        self.last_log_asked += 1;
        Ok(self.last_log.clone())
    }

    fn read_options_doc(&mut self) -> anyhow::Result<Option<crate::options::OptionsDoc>> {
        Ok(self.options_doc.clone())
    }

    fn config_repo(&mut self) -> Option<crate::config_repo::RepoConfig> {
        self.repo.clone()
    }

    fn config_head(&mut self) -> anyhow::Result<Option<crate::config_repo::Head>> {
        Ok(self.commits.last().map(|c| crate::config_repo::Head {
            sha: c.sha.clone(),
            branch: self.branch.clone(),
        }))
    }

    fn config_commit(&mut self, subject: &str, body: &str) -> anyhow::Result<Option<String>> {
        if !self.dirty && !self.commits.is_empty() {
            return Ok(None);
        }
        let sha = Self::sha(self.commit_seq);
        self.commit_seq += 1;
        self.commits.push(FakeCommit {
            sha: sha.clone(),
            subject: subject.to_string(),
            body: body.to_string(),
        });
        self.dirty = false;
        Ok(Some(sha))
    }

    fn config_fetch(&mut self, _url: &str, _branch: &str) -> anyhow::Result<Option<String>> {
        if self.fetch_fails {
            return Err(crate::config_repo::NotUp("the fake remote is not up".to_string()).into());
        }
        Ok(self.remote_commits.last().cloned())
    }

    fn config_is_ancestor(&mut self, ancestor: &str, of: &str) -> anyhow::Result<bool> {
        Ok(self.history(of).iter().any(|s| s == ancestor))
    }

    fn config_fast_forward(&mut self, to: &str) -> anyhow::Result<()> {
        let history = self.history(to);
        if history.is_empty() {
            anyhow::bail!("unknown commit {to}");
        }
        let mut commits: Vec<FakeCommit> = Vec::new();
        for sha in history {
            match self.commits.iter().find(|c| c.sha == sha) {
                Some(c) => commits.push(c.clone()),
                None => commits.push(FakeCommit {
                    sha,
                    subject: "(pushed from a clone)".to_string(),
                    body: String::new(),
                }),
            }
        }
        self.commits = commits;
        // The one effect a later step reads back: the fast-forward writes
        // the pushed overrides.nix into the working tree.
        if let Some(body) = &self.remote_overrides {
            self.config = body.lines().map(str::to_string).collect();
        }
        self.dirty = false;
        self.fast_forwarded.push(to.to_string());
        Ok(())
    }

    fn config_push(&mut self, url: &str, _branch: &str) -> anyhow::Result<()> {
        self.remote_commits = self.commits.iter().map(|c| c.sha.clone()).collect();
        self.pushed.push(url.to_string());
        Ok(())
    }

    fn config_show(&mut self, rev: &str, path: &str) -> anyhow::Result<Option<String>> {
        if path != "modules/overrides.nix" || !self.remote_commits.iter().any(|s| s == rev) {
            return Ok(None);
        }
        Ok(self.remote_overrides.clone())
    }

    fn config_log(&mut self, n: usize) -> anyhow::Result<Vec<crate::config_repo::LogEntry>> {
        Ok(self
            .commits
            .iter()
            .rev()
            .take(n)
            .map(|c| crate::config_repo::LogEntry {
                sha: c.sha.clone(),
                when: "2026-10-07T12:00:00Z".to_string(),
                subject: c.subject.clone(),
            })
            .collect())
    }

    fn forgejo_request(
        &mut self,
        op: &crate::config_repo::ForgejoOp,
        secret: Option<&crate::setup::Secret>,
    ) -> anyhow::Result<(u16, String)> {
        let (method, path) = op.route();
        let key = format!("{method} {path}");
        self.forgejo_calls
            .push((key.clone(), secret.map(|s| s.expose().to_string())));
        match self.forgejo_routes.get(&key) {
            Some((status, body)) => Ok((*status, body.clone())),
            None => {
                Err(crate::config_repo::NotUp("the fake LosOS Git is not up".to_string()).into())
            }
        }
    }

    fn mint_secret(&mut self) -> anyhow::Result<crate::setup::Secret> {
        Ok(crate::setup::Secret::new("minted-fake-password-0123"))
    }

    fn load_sync_report(&mut self) -> anyhow::Result<Option<crate::config_repo::SyncReport>> {
        Ok(self.sync_report.clone())
    }

    fn save_sync_report(&mut self, report: &crate::config_repo::SyncReport) -> anyhow::Result<()> {
        self.sync_report = Some(report.clone());
        Ok(())
    }

    fn backup_target(&mut self) -> anyhow::Result<Option<crate::backup::Target>> {
        Ok(self.backup_target.clone())
    }

    fn write_backup_target(
        &mut self,
        target: Option<&crate::backup::Target>,
    ) -> anyhow::Result<()> {
        self.backup_target = target.cloned();
        Ok(())
    }

    fn start_backup_job(&mut self, kind: crate::backup::Kind, job: &str) -> anyhow::Result<()> {
        self.units_started.push(kind.unit(job));
        Ok(())
    }

    fn poll_backup_job(
        &mut self,
        _kind: crate::backup::Kind,
        _job: &str,
    ) -> crate::supervisor::Poll {
        self.unit_poll.clone()
    }

    fn stop_backup_job(&mut self, kind: crate::backup::Kind, job: &str) -> anyhow::Result<()> {
        self.units_stopped.push(kind.unit(job));
        Ok(())
    }

    fn backup_log_tail(&mut self) -> String {
        self.backup_log.clone()
    }

    fn backup_report(&mut self) -> anyhow::Result<Option<crate::backup::Report>> {
        Ok(self.backup_report.clone())
    }

    fn write_restore_code(&mut self, code: Option<&str>) -> anyhow::Result<()> {
        self.restore_code = code.map(str::to_string);
        Ok(())
    }

    fn take_restored_overrides(&mut self) -> anyhow::Result<Option<String>> {
        Ok(self.restored_overrides.take())
    }

    fn now(&mut self) -> u64 {
        self.clock
    }

    fn erase_grace_secs(&mut self) -> u64 {
        self.grace_secs
    }

    fn wipe_and_reboot(&mut self) -> anyhow::Result<()> {
        self.rebooted = true;
        Ok(())
    }

    fn write_erase_report(&mut self, report: &crate::erase::Outside) -> anyhow::Result<()> {
        self.erase_report = Some(report.clone());
        Ok(())
    }

    fn read_erase_report(&mut self) -> anyhow::Result<Option<crate::erase::Outside>> {
        Ok(self.erase_report.clone())
    }
}
