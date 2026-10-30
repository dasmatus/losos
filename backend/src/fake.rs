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

    // ── Installing an app ───────────────────────────────────────────────
    /// `None` models a box with no local cluster.
    pub apps: Option<crate::apps::Config>,
    /// What a chart fetch hands back; `None` is a chart that cannot be had.
    pub chart_files: Option<crate::apps::ChartFiles>,
    pub app_records: Vec<crate::apps::Record>,
    /// What `save_app_source` was handed, by app.
    pub app_sources: std::collections::BTreeMap<String, (String, Vec<(String, String)>)>,
    /// Every job started, in order.
    pub app_jobs: Vec<(String, crate::apps::Action)>,
    /// Apps whose job the fake reports as still running.
    pub app_jobs_active: Vec<String>,

    // ── The market ──────────────────────────────────────────────────────
    /// Registrar answers by `METHOD path`, as `(status, body)`. A route with no
    /// entry models a box with no registrar configured.
    pub market_routes: std::collections::BTreeMap<String, (u16, String)>,
    /// The edge `market_config` reports; `None`, no master proxy.
    pub market_config: Option<crate::market::Config>,
    /// Every operation asked for, in order.
    pub market_ops: Vec<crate::market::Op>,
    /// The `market-escrow-key` credential's bytes, if the box has one.
    pub escrow_seed: Option<[u8; 32]>,
    /// The live custom domains last handed to LosOS Cloud; `None` until the
    /// first write.
    pub public_names: Option<Vec<String>>,
    /// What `public_tls` answers.
    pub public_tls: crate::edge::PublicTls,
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
    /// Stands in for `user-apps.json`.
    pub user_apps: crate::user_apps::UserApps,
    /// Stands in for `user-apps/<id>.json`, by app id.
    pub app_data: std::collections::BTreeMap<String, crate::user_apps::AppData>,
    /// Every app version, local and pushed, by sha: one object store for
    /// the box and the fake LosOS Git, as fetching would make it.
    pub app_commits: std::collections::BTreeMap<String, FakeAppCommit>,
    /// Each app's branch, by app id.
    pub app_heads: std::collections::BTreeMap<String, String>,
    /// The fake LosOS Git's branch of each app repository, by URL.
    pub app_remotes: std::collections::BTreeMap<String, String>,
    /// Every push of an app repository, by URL.
    pub app_pushes: Vec<String>,
    /// The fake LosOS Git does not answer fetches and pushes.
    pub app_remote_down: bool,
    // ── The pin picker ──────────────────────────────────────────────────
    /// LosOS Cloud's next answers, in order; empty is "not answering".
    pub cloud_answers: std::collections::VecDeque<(u16, Vec<u8>)>,
    /// Every request made of LosOS Cloud: `mint <user> <device>`,
    /// `list <user> <path>`, `search <user> <query>` or `arrive <user>`.
    pub cloud_calls: Vec<String>,

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
    pub git_routes: std::collections::BTreeMap<String, (u16, String)>,
    /// Every request made, as `METHOD path`.
    pub git_calls: Vec<String>,
    pub sync_report: Option<crate::config_repo::SyncReport>,
    /// Where the box follows the published tree from; `None` is off.
    pub upstream: Option<crate::config_repo::Upstream>,
    /// The published branch's head; `None` models a source that cannot be
    /// reached.
    pub upstream_head: Option<String>,
    /// `upstream_base` per published commit.
    pub upstream_bases: std::collections::HashMap<String, i64>,
    /// What the last follow recorded.
    pub upstream_mark: Option<crate::config_repo::FollowMark>,
    /// Whether files outside the box's own were changed since the mark.
    pub edited_outside: bool,
    pub upstream_report: Option<crate::config_repo::SyncReport>,
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

/// One app version in the fake's object store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeAppCommit {
    pub parent: Option<String>,
    pub files: std::collections::BTreeMap<String, String>,
    pub subject: String,
    pub body: String,
}

impl FakeLosos {
    /// Make an app version in the store and return its sha.
    pub fn app_commit_object(
        &mut self,
        parent: Option<String>,
        files: &[(&str, String)],
        subject: &str,
        body: &str,
    ) -> String {
        let sha = format!("{:040x}", 0xa000_0000_u64 + self.app_commits.len() as u64);
        self.app_commits.insert(
            sha.clone(),
            FakeAppCommit {
                parent,
                files: files
                    .iter()
                    .map(|(n, t)| (n.to_string(), t.clone()))
                    .collect(),
                subject: subject.to_string(),
                body: body.to_string(),
            },
        );
        sha
    }

    /// What a clone pushing to `url` does: a version on top of the
    /// remote's branch with `files` changed.
    pub fn push_from_clone(&mut self, url: &str, changed: &[(&str, String)]) -> String {
        let parent = self.app_remotes.get(url).cloned();
        let mut files: Vec<(String, String)> = parent
            .as_ref()
            .and_then(|p| self.app_commits.get(p))
            .map(|c| c.files.clone().into_iter().collect())
            .unwrap_or_default();
        for (name, text) in changed {
            files.retain(|(n, _)| n != name);
            files.push((name.to_string(), text.clone()));
        }
        let refs: Vec<(&str, String)> =
            files.iter().map(|(n, t)| (n.as_str(), t.clone())).collect();
        let sha = self.app_commit_object(parent, &refs, "Pushed from a clone", "");
        self.app_remotes.insert(url.to_string(), sha.clone());
        sha
    }

    fn app_history(&self, of: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut at = Some(of.to_string());
        while let Some(sha) = at {
            at = self.app_commits.get(&sha).and_then(|c| c.parent.clone());
            out.push(sha);
        }
        out
    }

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
            apps: Some(crate::apps::Config {
                ports: (30000, 30099),
                kube: true,
                builtin: Vec::new(),
            }),
            app_sources: std::collections::BTreeMap::new(),
            chart_files: None,
            app_records: Vec::new(),
            app_jobs: Vec::new(),
            app_jobs_active: Vec::new(),
            market_routes: std::collections::BTreeMap::new(),
            market_config: None,
            market_ops: Vec::new(),
            escrow_seed: Some([7u8; 32]),
            public_names: None,
            public_tls: crate::edge::PublicTls::default(),
            relay_pass: None,
            edge: crate::edge::EdgeStatus::found(
                vec![crate::edge::Edge {
                    name: "edge".to_string(),
                    url: "http://edge.local:8443".to_string(),
                    source: crate::edge::Source::Lan,
                    official: true,
                    gateway: None,
                    tunnel: Some("edge.local:2333".to_string()),
                    on_network: true,
                    tier: crate::edge::Tier::Tertiary,
                    cluster: None,
                    member: None,
                    official_cluster: None,
                }],
                true,
                None,
            ),
            edge_asked: 0,
            user_apps: crate::user_apps::UserApps::default(),
            app_data: std::collections::BTreeMap::new(),
            app_commits: std::collections::BTreeMap::new(),
            app_heads: std::collections::BTreeMap::new(),
            app_remotes: std::collections::BTreeMap::new(),
            app_pushes: Vec::new(),
            app_remote_down: false,
            cloud_answers: std::collections::VecDeque::new(),
            cloud_calls: Vec::new(),
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
            git_routes: std::collections::BTreeMap::new(),
            git_calls: Vec::new(),
            sync_report: None,
            upstream: None,
            upstream_head: None,
            upstream_bases: std::collections::HashMap::new(),
            upstream_mark: None,
            edited_outside: false,
            upstream_report: None,
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

    /// A local gateway in reach and nothing else: the edge a box on a
    /// company network finds, its link secondary.
    #[must_use]
    pub fn with_local_gateway(mut self) -> Self {
        for e in &mut self.edge.edges {
            e.official = false;
            e.tier = crate::edge::Tier::Secondary;
        }
        self.edge = crate::edge::EdgeStatus::found(self.edge.edges.clone(), true, None);
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
            git_url: "http://127.0.0.1:3000".to_string(),
            token_file: "/var/lib/losos-git/admin-token".to_string(),
            remote_url: None,
        });
        self.git_routes.insert(
            "PUT /api/v1/admin/users/notshared".to_string(),
            (200, r#"{"id": 2, "login": "notshared"}"#.to_string()),
        );
        self.git_routes.insert(
            "PUT /api/v1/admin/users/notshared/repos/losos-config".to_string(),
            (200, r#"{"id": 1, "name": "losos-config"}"#.to_string()),
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

    fn apps_config(&mut self) -> Option<crate::apps::Config> {
        self.apps.clone()
    }

    fn chart_files(
        &mut self,
        _chart: &crate::apps::ChartRef,
    ) -> anyhow::Result<crate::apps::ChartFiles> {
        self.chart_files
            .clone()
            .ok_or_else(|| anyhow::anyhow!("the chart could not be fetched"))
    }

    fn load_app_records(&mut self) -> anyhow::Result<Vec<crate::apps::Record>> {
        Ok(self.app_records.clone())
    }

    fn save_app_source(
        &mut self,
        release: &str,
        compose: &str,
        files: &[(String, String)],
    ) -> anyhow::Result<()> {
        self.app_sources
            .insert(release.to_string(), (compose.to_string(), files.to_vec()));
        Ok(())
    }

    fn save_app_record(&mut self, record: &crate::apps::Record) -> anyhow::Result<()> {
        match self
            .app_records
            .iter_mut()
            .find(|r| r.release == record.release)
        {
            Some(old) => *old = record.clone(),
            None => self.app_records.push(record.clone()),
        }
        Ok(())
    }

    fn start_app_job(&mut self, release: &str, action: crate::apps::Action) -> anyhow::Result<()> {
        self.app_jobs.push((release.to_string(), action));
        self.app_jobs_active.push(release.to_string());
        Ok(())
    }

    fn app_job_active(&mut self, release: &str) -> bool {
        self.app_jobs_active.iter().any(|r| r == release)
    }

    /// The real classifier, run against a scripted registrar.
    fn edge_status(&mut self) -> anyhow::Result<crate::edge::EdgeStatus> {
        self.edge_asked += 1;
        Ok(self.edge.clone())
    }

    /// The real path rule, over the scripted scan.
    fn set_edge_cluster(
        &mut self,
        cluster: Option<&str>,
    ) -> anyhow::Result<crate::edge::EdgeStatus> {
        let current = self.edge.path.as_ref().map(|p| p.url.clone());
        self.edge.route(&crate::edge::PathRule {
            member_of: cluster,
            current: current.as_deref(),
            seed: "box",
            ..crate::edge::PathRule::default()
        });
        Ok(self.edge.clone())
    }

    fn market_config(&mut self) -> Option<crate::market::Config> {
        self.market_config.clone()
    }

    fn market_request(&mut self, op: &crate::market::Op) -> anyhow::Result<crate::market::Outcome> {
        self.market_ops.push(op.clone());
        let (method, path) = op.route();
        match self.market_routes.get(&format!("{method} {path}")) {
            None => Ok(crate::market::Outcome::Unavailable),
            Some((status, body)) => op.classify(*status, body),
        }
    }

    fn escrow_keys(&mut self) -> anyhow::Result<Option<crate::escrow_key::Keys>> {
        self.escrow_seed
            .map(|s| crate::escrow_key::Keys::from_seed(&s))
            .transpose()
    }

    fn write_relay_pass(&mut self, pass: Option<&str>) -> anyhow::Result<()> {
        self.relay_pass = Some(pass.map(str::to_string));
        Ok(())
    }

    fn write_public_names(&mut self, names: &[String]) -> anyhow::Result<()> {
        self.public_names = Some(names.to_vec());
        Ok(())
    }

    fn public_tls(&mut self) -> anyhow::Result<crate::edge::PublicTls> {
        Ok(self.public_tls.clone())
    }

    fn cloud_request(
        &mut self,
        request: &crate::pins::CloudRequest<'_>,
    ) -> anyhow::Result<(u16, Vec<u8>)> {
        self.cloud_calls.push(match request {
            crate::pins::CloudRequest::MintKey { user, device } => format!("mint {user} {device}"),
            crate::pins::CloudRequest::List { user, path } => format!("list {user} {path}"),
            crate::pins::CloudRequest::Search { user, query } => format!("search {user} {query}"),
            crate::pins::CloudRequest::Arrive { user, .. } => format!("arrive {user}"),
            crate::pins::CloudRequest::AddressBooks { user } => format!("books {user}"),
            crate::pins::CloudRequest::Cards { user, href } => format!("cards {user} {href}"),
            crate::pins::CloudRequest::ContactMatches { user, query, want } => {
                format!("contacts {user} {want} {query}")
            }
        });
        self.cloud_answers
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("the fake LosOS Cloud is not answering"))
    }

    fn load_user_apps(&mut self) -> anyhow::Result<crate::user_apps::UserApps> {
        Ok(self.user_apps.clone())
    }

    fn save_user_apps(&mut self, apps: &crate::user_apps::UserApps) -> anyhow::Result<()> {
        self.user_apps = apps.clone();
        Ok(())
    }

    fn load_app_data(&mut self, id: &str) -> anyhow::Result<crate::user_apps::AppData> {
        Ok(self.app_data.get(id).cloned().unwrap_or_default())
    }

    fn save_app_data(&mut self, id: &str, data: &crate::user_apps::AppData) -> anyhow::Result<()> {
        self.app_data.insert(id.to_string(), data.clone());
        Ok(())
    }

    fn remove_app_data(&mut self, id: &str) -> anyhow::Result<()> {
        self.app_data.remove(id);
        Ok(())
    }

    fn app_repo_head(&mut self, id: &str) -> anyhow::Result<Option<String>> {
        Ok(self.app_heads.get(id).cloned())
    }

    fn app_repo_commit(
        &mut self,
        id: &str,
        files: &[(&str, String)],
        subject: &str,
        body: &str,
    ) -> anyhow::Result<Option<String>> {
        let head = self.app_heads.get(id).cloned();
        let mut tree = head
            .as_ref()
            .and_then(|h| self.app_commits.get(h))
            .map(|c| c.files.clone())
            .unwrap_or_default();
        let before = tree.clone();
        for (name, text) in files {
            tree.insert(name.to_string(), text.clone());
        }
        if head.is_some() && tree == before {
            return Ok(None);
        }
        let all: Vec<(&str, String)> = tree.iter().map(|(n, t)| (n.as_str(), t.clone())).collect();
        let sha = self.app_commit_object(head, &all, subject, body);
        self.app_heads.insert(id.to_string(), sha.clone());
        Ok(Some(sha))
    }

    fn app_repo_fetch(&mut self, _id: &str, url: &str) -> anyhow::Result<Option<String>> {
        if self.app_remote_down {
            return Err(
                crate::config_repo::NotUp("the fake LosOS Git is not up".to_string()).into(),
            );
        }
        Ok(self.app_remotes.get(url).cloned())
    }

    fn app_repo_is_ancestor(
        &mut self,
        _id: &str,
        ancestor: &str,
        of: &str,
    ) -> anyhow::Result<bool> {
        Ok(self.app_history(of).iter().any(|s| s == ancestor))
    }

    fn app_repo_show(
        &mut self,
        _id: &str,
        rev: &str,
        path: &str,
    ) -> anyhow::Result<Option<String>> {
        Ok(self
            .app_commits
            .get(rev)
            .and_then(|c| c.files.get(path).cloned()))
    }

    fn app_repo_set_head(&mut self, id: &str, sha: &str) -> anyhow::Result<()> {
        anyhow::ensure!(self.app_commits.contains_key(sha), "unknown commit {sha}");
        self.app_heads.insert(id.to_string(), sha.to_string());
        Ok(())
    }

    fn app_repo_push(&mut self, id: &str, url: &str) -> anyhow::Result<()> {
        if self.app_remote_down {
            return Err(
                crate::config_repo::NotUp("the fake LosOS Git is not up".to_string()).into(),
            );
        }
        let head = self
            .app_heads
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("nothing to push"))?;
        if let Some(remote) = self.app_remotes.get(url) {
            anyhow::ensure!(
                self.app_history(&head).contains(remote),
                "rejected: not a fast-forward"
            );
        }
        self.app_remotes.insert(url.to_string(), head);
        self.app_pushes.push(url.to_string());
        Ok(())
    }

    fn app_repo_remove(&mut self, id: &str) -> anyhow::Result<()> {
        self.app_heads.remove(id);
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

    fn git_request(&mut self, op: &crate::config_repo::GitOp) -> anyhow::Result<(u16, String)> {
        let (method, path) = op.route();
        let key = format!("{method} {path}");
        self.git_calls.push(key.clone());
        match self.git_routes.get(&key) {
            Some((status, body)) => Ok((*status, body.clone())),
            None => {
                Err(crate::config_repo::NotUp("the fake LosOS Git is not up".to_string()).into())
            }
        }
    }

    fn load_sync_report(&mut self) -> anyhow::Result<Option<crate::config_repo::SyncReport>> {
        Ok(self.sync_report.clone())
    }

    fn save_sync_report(&mut self, report: &crate::config_repo::SyncReport) -> anyhow::Result<()> {
        self.sync_report = Some(report.clone());
        Ok(())
    }

    fn upstream(&mut self) -> Option<crate::config_repo::Upstream> {
        self.upstream.clone()
    }

    fn upstream_fetch(&mut self, _up: &crate::config_repo::Upstream) -> anyhow::Result<String> {
        self.upstream_head.clone().ok_or_else(|| {
            crate::config_repo::NotUp("the fake published branch is not up".to_string()).into()
        })
    }

    fn upstream_mark(&mut self) -> anyhow::Result<Option<crate::config_repo::FollowMark>> {
        Ok(self.upstream_mark.clone())
    }

    fn upstream_base(&mut self, sha: &str) -> anyhow::Result<Option<i64>> {
        Ok(self.upstream_bases.get(sha).copied())
    }

    fn config_edited_outside(&mut self, _base: Option<&str>) -> anyhow::Result<bool> {
        Ok(self.edited_outside)
    }

    fn config_follow(
        &mut self,
        upstream: &str,
        subject: &str,
        body: &str,
    ) -> anyhow::Result<Option<String>> {
        let sha = Self::sha(self.commit_seq);
        self.commit_seq += 1;
        self.commits.push(FakeCommit {
            sha: sha.clone(),
            subject: subject.to_string(),
            body: body.to_string(),
        });
        self.upstream_mark = Some(crate::config_repo::FollowMark {
            upstream: upstream.to_string(),
            followed: sha.clone(),
        });
        Ok(Some(sha))
    }

    fn load_upstream_report(&mut self) -> anyhow::Result<Option<crate::config_repo::SyncReport>> {
        Ok(self.upstream_report.clone())
    }

    fn save_upstream_report(
        &mut self,
        report: &crate::config_repo::SyncReport,
    ) -> anyhow::Result<()> {
        self.upstream_report = Some(report.clone());
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
