//! Shared fixtures for the registrar's integration tests.
//!
//! The tests drive the real thing: a real `ServeOpts`, a real `axum` router on
//! a real socket, real files on disk. Nothing here stubs the registrar's own
//! behaviour — the only thing this module fakes is the *operator*, standing in
//! for the NixOS module that would otherwise generate `tenants.json` and for
//! the hand-placed `/var/secrets` token files.
//!
//! Each test binary that includes this module uses a different slice of it, so
//! the unused-item warning is expected rather than informative.
#![allow(dead_code, unreachable_pub)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::http::{Method, StatusCode, Uri};
use axum::response::IntoResponse;
use axum::Router;
use losos_registrar::opts::ServeOpts;
use tokio::sync::oneshot;

/// A 64-hex-character token, the shape the docs describe for a real appliance.
pub const GOOD_TOKEN: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0";
/// A second valid token, distinct from [`GOOD_TOKEN`].
pub const OTHER_TOKEN: &str = "abad1deaabad1deaabad1deaabad1deaabad1deaabad1deaabad1deaabad1dea";
/// The rathole `default_token` the edge is configured with.
pub const BOOTSTRAP_TOKEN: &str =
    "b00757241pb00757241pb00757241pb00757241pb00757241pb00757241pb007";
/// The shape of a real rke2 node token: `K10<ca hash>::server:<password>`.
/// Long, and carrying `:` — which `token_fault` allows and a control-character
/// check must not be tightened into rejecting.
pub const MESH_AGENT_TOKEN: &str =
    "K10bb0dcafebb0dcafebb0dcafebb0dcafebb0dcafebb0dcafe::server:meshpassword0123456789";
/// The ServiceAccount bearer token the registrar presents to the apiserver.
pub const KUBE_TOKEN: &str = "eyJhbGciOiJSUzI1NiIsImtpZCI6Imxvc29zLXJlZ2lzdHJhciJ9.stub";

/// A directory under the system temp dir, removed when the guard drops.
///
/// Uniquified by pid, the wall clock and a per-process counter, so neither
/// `cargo test`'s parallel threads nor two concurrent `cargo test` runs ever
/// share one.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(_tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        // The clock alone can repeat across threads (a coarse clock, or two
        // calls inside one tick); the counter cannot within one process.
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("l-{}-{nanos}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    pub fn path_str(&self, name: &str) -> String {
        self.join(name).to_string_lossy().into_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// One row of the operator whitelist: an appliance id, the hostname it may
/// claim, the contents of its token file (`None` writes no file at all — the
/// "operator forgot to place the secret" case), and whether the operator has
/// cleared it for the mesh cluster.
pub struct TenantSpec {
    pub id: &'static str,
    pub hostname: String,
    pub token: Option<String>,
    pub cluster: bool,
    pub market: bool,
    /// `losos.edge.tenants.<id>.relayZone` — this tenant is a spoke.
    pub relay_zone: Option<String>,
}

impl TenantSpec {
    pub fn new(id: &'static str, hostname: &str, token: &str) -> Self {
        Self {
            id,
            hostname: hostname.to_string(),
            token: Some(token.to_string()),
            cluster: false,
            market: false,
            relay_zone: None,
        }
    }

    /// `losos.edge.tenants.<id>.relayZone = zone` — may relay boxes under it.
    #[must_use]
    pub fn with_relay_zone(mut self, zone: &str) -> Self {
        self.relay_zone = Some(zone.to_string());
        self
    }

    /// A tenant whose token file is never created.
    pub fn without_token_file(id: &'static str, hostname: &str) -> Self {
        Self {
            id,
            hostname: hostname.to_string(),
            token: None,
            cluster: false,
            market: false,
            relay_zone: None,
        }
    }

    /// `losos.edge.tenants.<id>.market = true` — may buy and sell.
    #[must_use]
    pub fn with_market(mut self) -> Self {
        self.market = true;
        self
    }

    /// `losos.edge.tenants.<id>.cluster = true` — cleared for mesh enrolment.
    #[must_use]
    pub fn with_cluster(mut self) -> Self {
        self.cluster = true;
        self
    }
}

/// The `serve` flags that turn the mesh half on.
///
/// Every test that predates the mesh starts an [`Edge`] with
/// [`MeshFixture::default`], i.e. none of these flags — which is exactly the
/// argument list `modules/edge.nix` generates for
/// `losos.edge.cluster.enable = false`. Those tests therefore keep asserting
/// that the master-proxy half is untouched by the join route existing.
#[derive(Default)]
pub struct MeshFixture {
    /// Written to `mesh-agent.token`; enables `/cluster/join`.
    pub agent_token: Option<String>,
    /// `--mesh-server-addr`, the `https://<addr>:9345` an agent registers on.
    pub server_addr: Option<String>,
    /// `--kube-api`. A plain-HTTP stub in these tests: the registrar only
    /// demands a pinned CA for an https apiserver.
    pub kube_api: Option<String>,
    /// Written to `kube.token`; `--kube-token-file`.
    pub kube_token: Option<String>,
}

impl MeshFixture {
    /// A fully configured mesh edge pointed at `kube_api`.
    pub fn enabled(kube_api: &str) -> Self {
        Self {
            agent_token: Some(MESH_AGENT_TOKEN.to_string()),
            server_addr: Some("https://198.51.100.7:9345".to_string()),
            kube_api: Some(kube_api.to_string()),
            kube_token: Some(KUBE_TOKEN.to_string()),
        }
    }
}

/// The Stripe secrets the market fixture writes, and what the stub answers.
pub const STRIPE_KEY: &str = "sk_test_0123456789abcdef";
pub const WEBHOOK_SECRET: &str = "whsec_0123456789abcdef";
pub const RETURN_URL: &str = "https://losos.example/market";
/// The zone and address the domains fixture serves.
pub const DNS_ZONE: &str = "boxes.example.test";
pub const EDGE_IPV4: &str = "203.0.113.7";

/// A last-minute change to the options, for flags with no fixture of their own.
pub type Tweak = Box<dyn FnOnce(&mut ServeOpts, &TempDir) + Send>;

/// A running registrar: its temp state directory, its base URL, and the
/// handles needed to stop it.
pub struct Edge {
    pub dir: TempDir,
    pub base: String,
    pub client: reqwest::Client,
    stop: Option<oneshot::Sender<()>>,
    join: Option<tokio::task::JoinHandle<miette::Result<()>>>,
    /// The Stripe gate the market talks to, when the market is on. The
    /// registrar itself never sees the key files this writes.
    gate_stop: Option<oneshot::Sender<()>>,
    gate_join: Option<tokio::task::JoinHandle<()>>,
}

impl Edge {
    /// Write the whitelist + token files, then start `serve` on an ephemeral
    /// port. `reconcile_interval` is deliberately short so a test can observe
    /// a steady-state pass without waiting on production's 15s tick.
    pub async fn start(tag: &str, tenants: &[TenantSpec]) -> Self {
        Self::start_with_mesh(tag, tenants, MeshFixture::default()).await
    }

    /// As [`Edge::start`], with the mesh half configured.
    pub async fn start_with_mesh(tag: &str, tenants: &[TenantSpec], mesh: MeshFixture) -> Self {
        Self::start_inner(tag, tenants, mesh, None, None).await
    }

    /// As [`Edge::start`], with the Stripe market pointed at `stripe_api` and
    /// `seller-box` already enrolled in the mesh with compute sharing on —
    /// the market only sells what an appliance is sharing.
    pub async fn start_with_market(tag: &str, tenants: &[TenantSpec], stripe_api: &str) -> Self {
        Self::start_market(
            tag,
            tenants,
            stripe_api,
            MeshFixture::default(),
            &[("seller-box", true)],
        )
        .await
    }

    /// As [`Edge::start_with_market`], with the mesh half configured too, so
    /// a paid storage order has an apiserver to be fulfilled against.
    pub async fn start_with_market_and_mesh(
        tag: &str,
        tenants: &[TenantSpec],
        stripe_api: &str,
        mesh: MeshFixture,
    ) -> Self {
        Self::start_market(tag, tenants, stripe_api, mesh, &[("seller-box", true)]).await
    }

    /// The general form: `enrolled` lists `(appliance id, share_compute)` for
    /// every appliance that has already joined the mesh, written into
    /// `registry.json` before the edge starts.
    pub async fn start_market(
        tag: &str,
        tenants: &[TenantSpec],
        stripe_api: &str,
        mesh: MeshFixture,
        enrolled: &[(&str, bool)],
    ) -> Self {
        Self::start_inner_seeded(
            tag,
            tenants,
            mesh,
            None,
            Some(stripe_api.to_string()),
            enrolled,
            None,
            None,
        )
        .await
    }

    /// As [`Edge::start_market`], with `market.json` written before the edge
    /// starts, for states a test cannot reach in real time (an entitlement
    /// that lapsed a month ago).
    pub async fn start_market_with_state(
        tag: &str,
        tenants: &[TenantSpec],
        stripe_api: &str,
        mesh: MeshFixture,
        enrolled: &[(&str, bool)],
        market_state: &serde_json::Value,
    ) -> Self {
        Self::start_inner_seeded(
            tag,
            tenants,
            mesh,
            None,
            Some(stripe_api.to_string()),
            enrolled,
            Some(market_state),
            None,
        )
        .await
    }

    /// As [`Edge::start`], serving `public_key` at `/noise-public-key`.
    pub async fn start_with_noise_public_key(
        tag: &str,
        tenants: &[TenantSpec],
        public_key: &str,
    ) -> Self {
        Self::start_inner(tag, tenants, MeshFixture::default(), Some(public_key), None).await
    }

    /// An official edge (identity key and certificate in place) with a
    /// market store seeded from `market_state` and the domains half on,
    /// looking records up at `doh_url`. The market's gate points nowhere:
    /// domains read the store and never ask Stripe.
    pub async fn start_with_domains(
        tag: &str,
        tenants: &[TenantSpec],
        market_state: &serde_json::Value,
        identity: Option<(&str, &str)>,
        doh_url: &str,
    ) -> Self {
        Self::start_general(
            tag,
            tenants,
            MeshFixture::default(),
            None,
            Some("http://127.0.0.1:9".to_string()),
            &[],
            Some(market_state),
            identity,
            None,
            None,
            Some(doh_url),
            None,
        )
        .await
    }

    /// An official hub for the relayed-domain tests: as
    /// [`Edge::start_with_domains`], with `enrolled` boxes already in the
    /// mesh and `tweak` applied last (how a test turns `--relay-routes` on).
    #[allow(clippy::too_many_arguments)]
    pub async fn start_official_hub(
        tag: &str,
        tenants: &[TenantSpec],
        market_state: &serde_json::Value,
        identity: (&str, &str),
        doh_url: &str,
        enrolled: &[(&str, bool)],
        tweak: impl FnOnce(&mut ServeOpts, &TempDir) + Send + 'static,
    ) -> Self {
        Self::start_general(
            tag,
            tenants,
            MeshFixture::default(),
            None,
            Some("http://127.0.0.1:9".to_string()),
            enrolled,
            Some(market_state),
            Some(identity),
            None,
            None,
            Some(doh_url),
            Some(Box::new(tweak)),
        )
        .await
    }

    async fn start_inner(
        tag: &str,
        tenants: &[TenantSpec],
        mesh: MeshFixture,
        noise_public_key: Option<&str>,
        stripe_api: Option<String>,
    ) -> Self {
        Self::start_inner_seeded(
            tag,
            tenants,
            mesh,
            noise_public_key,
            stripe_api,
            &[],
            None,
            None,
        )
        .await
    }

    /// As [`Edge::start`], with `--identity-key-file` / `--identity-cert-file`
    /// pointing at files a test (or `provision edge`) already wrote, so the
    /// edge answers `GET /identity`. `listener` lets the test choose the port
    /// before it starts, because a certificate names the URL it is for.
    pub async fn start_with_identity_on(
        tag: &str,
        tenants: &[TenantSpec],
        key_file: &str,
        cert_file: &str,
        listener: Option<tokio::net::TcpListener>,
    ) -> Self {
        Self::start_general(
            tag,
            tenants,
            MeshFixture::default(),
            None,
            None,
            &[],
            None,
            Some((key_file, cert_file)),
            listener,
            None,
            None,
            None,
        )
        .await
    }

    /// As [`Edge::start_with_identity_on`], with `POST /identity/cert`
    /// resolving GitHub tokens against `github_api_url` (a test's fake).
    pub async fn start_with_identity_and_github(
        tag: &str,
        key_file: &str,
        cert_file: &str,
        listener: Option<tokio::net::TcpListener>,
        github_api_url: &str,
    ) -> Self {
        Self::start_general(
            tag,
            &[],
            MeshFixture::default(),
            None,
            None,
            &[],
            None,
            Some((key_file, cert_file)),
            listener,
            Some(github_api_url),
            None,
            None,
        )
        .await
    }

    /// As [`Edge::start`], with `tweak` applied to the options just before
    /// `serve` starts: how the federation tests turn on the uplink and open
    /// enrolment, whose flags have no fixture of their own.
    pub async fn start_custom(
        tag: &str,
        tenants: &[TenantSpec],
        tweak: impl FnOnce(&mut ServeOpts, &TempDir) + Send + 'static,
    ) -> Self {
        Self::start_inner_seeded(
            tag,
            tenants,
            MeshFixture::default(),
            None,
            None,
            &[],
            None,
            Some(Box::new(tweak)),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn start_inner_seeded(
        tag: &str,
        tenants: &[TenantSpec],
        mesh: MeshFixture,
        noise_public_key: Option<&str>,
        stripe_api: Option<String>,
        enrolled: &[(&str, bool)],
        market_state: Option<&serde_json::Value>,
        tweak: Option<Tweak>,
    ) -> Self {
        Self::start_general(
            tag,
            tenants,
            mesh,
            noise_public_key,
            stripe_api,
            enrolled,
            market_state,
            None,
            None,
            None,
            None,
            tweak,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn start_general(
        tag: &str,
        tenants: &[TenantSpec],
        mesh: MeshFixture,
        noise_public_key: Option<&str>,
        stripe_api: Option<String>,
        enrolled: &[(&str, bool)],
        market_state: Option<&serde_json::Value>,
        identity: Option<(&str, &str)>,
        listener: Option<tokio::net::TcpListener>,
        github_api_url: Option<&str>,
        doh_url: Option<&str>,
        tweak: Option<Tweak>,
    ) -> Self {
        let dir = TempDir::new(tag);
        if let Some(market_state) = market_state {
            std::fs::write(
                dir.join("market.json"),
                serde_json::to_vec(market_state).expect("serialize market state"),
            )
            .expect("seed market.json");
        }
        if !enrolled.is_empty() {
            let windows: serde_json::Map<String, serde_json::Value> = enrolled
                .iter()
                .map(|(id, share)| {
                    (
                        (*id).to_string(),
                        serde_json::json!({
                            "share_compute": share,
                            "window_start": "23:00",
                            "window_end": "07:00",
                            "tz": "UTC",
                        }),
                    )
                })
                .collect();
            std::fs::write(
                dir.join("registry.json"),
                serde_json::to_vec(
                    &serde_json::json!({ "tenants": {}, "compute_windows": windows }),
                )
                .expect("serialize registry"),
            )
            .expect("seed registry.json");
        }
        std::fs::create_dir_all(dir.join("traefik")).expect("create traefik dir");
        std::fs::write(dir.join("bootstrap.token"), BOOTSTRAP_TOKEN).expect("write bootstrap");
        write_tenants(&dir, tenants);

        let mesh_agent_token_file = mesh.agent_token.map(|token| {
            std::fs::write(dir.join("mesh-agent.token"), token).expect("write mesh agent token");
            dir.path_str("mesh-agent.token")
        });
        let kube_token_file = mesh.kube_token.map(|token| {
            std::fs::write(dir.join("kube.token"), token).expect("write kube token");
            dir.path_str("kube.token")
        });

        let noise_public_key_file = noise_public_key.map(|key| {
            std::fs::write(dir.join("noise.pub"), format!("{key}\n")).expect("write noise pub");
            dir.path_str("noise.pub")
        });

        let mut gate_stop = None;
        let mut gate_join = None;
        let market = stripe_api.map(|stripe_api| {
            std::fs::write(dir.join("stripe.key"), STRIPE_KEY).expect("write stripe key");
            std::fs::write(dir.join("webhook.secret"), WEBHOOK_SECRET).expect("write webhook");
            let gate = losos_registrar::stripe_gate::GateOpts {
                socket: dir.path_str("gate.sock"),
                stripe_key_file: dir.path_str("stripe.key"),
                webhook_secret_file: dir.path_str("webhook.secret"),
                stripe_api,
                currency: "eur".to_string(),
                fee_bps: Some(losos_registrar::market::DEFAULT_FEE_BPS),
                return_url: Some(RETURN_URL.to_string()),
            };
            let listener = losos_registrar::stripe_gate::bind(&gate.socket).expect("bind gate");
            let (stop, rx) = oneshot::channel::<()>();
            gate_stop = Some(stop);
            gate_join = Some(tokio::spawn(losos_registrar::stripe_gate::serve(
                listener,
                gate,
                async move {
                    let _ = rx.await;
                },
            )));
            Box::new(losos_registrar::market::MarketOpts {
                state_file: dir.path_str("market.json"),
                gate_socket: dir.path_str("gate.sock"),
                return_url: RETURN_URL.to_string(),
                currency: "eur".to_string(),
                fee_bps: losos_registrar::market::DEFAULT_FEE_BPS,
                storage_class: losos_registrar::market::DEFAULT_STORAGE_CLASS.to_string(),
            })
        });

        let mut opts = ServeOpts {
            listen: "127.0.0.1:0".to_string(),
            registry_path: dir.path_str("registry.json"),
            traefik_dir: dir.path_str("traefik"),
            rathole_config: dir.path_str("server.toml"),
            rathole_bind_addr: "0.0.0.0".to_string(),
            rathole_bind_port: 2333,
            port_range: (50000, 50100),
            bootstrap_token_file: dir.path_str("bootstrap.token"),
            noise_private_key_file: None,
            noise_public_key_file,
            identity_key_file: identity.map(|(k, _)| k.to_string()),
            identity_cert_file: identity.map(|(_, c)| c.to_string()),
            tenants_file: dir.path_str("tenants.json"),
            reconcile_interval: Duration::from_millis(50),
            heartbeat_ttl: Duration::from_secs(300),
            mesh_agent_token_file,
            mesh_server_addr: mesh.server_addr,
            // The default is the production `https://127.0.0.1:6443`, which
            // would make every cleanup demand a pinned CA — so a test that
            // supplies no stub gets a URL that cannot connect, which is the
            // honest shape of "this edge has no cluster".
            kube_api: mesh
                .kube_api
                .unwrap_or_else(|| "https://127.0.0.1:6443".to_string()),
            kube_token_file,
            kube_ca_file: None,
            compute_windows_file: dir.path_str("compute-windows.json"),
            market,
            github_api_url: github_api_url
                .unwrap_or(losos_registrar::provision::GITHUB_API_URL)
                .to_string(),
            domains: doh_url.map(|doh_url| {
                Box::new(losos_registrar::domains::DomainsOpts {
                    state_file: dir.path_str("domains.json"),
                    zone: DNS_ZONE.to_string(),
                    zone_file: dir.path_str("dns/boxes.example.test.zone"),
                    nameservers: vec![format!("ns1.{DNS_ZONE}")],
                    hostmaster: format!("hostmaster.{DNS_ZONE}"),
                    ipv4: vec![EDGE_IPV4.parse().expect("edge ipv4")],
                    ipv6: Vec::new(),
                    doh_url: doh_url.to_string(),
                    public_domain: "example.test".to_string(),
                })
            }),
            enrol_dir: None,
            uplink: None,
            routes: None,
        };
        if let Some(tweak) = tweak {
            tweak(&mut opts, &dir);
        }

        let listener = match listener {
            Some(l) => l,
            None => tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind ephemeral port"),
        };
        let port = listener.local_addr().expect("local_addr").port();
        let (stop, rx) = oneshot::channel::<()>();
        let join = tokio::spawn(async move {
            losos_registrar::server::serve(listener, opts, async move {
                let _ = rx.await;
            })
            .await
        });

        let edge = Self {
            dir,
            base: format!("http://127.0.0.1:{port}"),
            client: reqwest::Client::new(),
            stop: Some(stop),
            join: Some(join),
            gate_stop,
            gate_join,
        };
        // `serve` does its registry load and first reconcile pass *before* it
        // starts accepting, so a successful /health is proof both finished —
        // without it a test could read the config files before they exist and
        // fail for a reason that has nothing to do with what it asserts.
        edge.wait_until_ready().await;
        edge
    }

    /// Poll `/health` until the server accepts, or give up after 5s.
    async fn wait_until_ready(&self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let responded = self
                .client
                .get(format!("{}/health", self.base))
                .send()
                .await
                .is_ok_and(|r| r.status().is_success());
            if responded {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("the registrar never became ready");
    }

    pub fn token_path(&self, id: &str) -> PathBuf {
        self.dir.join(&format!("{id}.token"))
    }

    pub fn tenants_path(&self) -> PathBuf {
        self.dir.join("tenants.json")
    }

    pub fn traefik_yaml(&self) -> Option<String> {
        std::fs::read_to_string(self.dir.join("traefik").join("losos.yml")).ok()
    }

    pub fn rathole_toml(&self) -> Option<String> {
        std::fs::read_to_string(self.dir.join("server.toml")).ok()
    }

    pub fn registry_json(&self) -> Option<String> {
        std::fs::read_to_string(self.dir.join("registry.json")).ok()
    }

    /// The file `losos-mesh-taint.service` reads on the real edge.
    pub fn compute_windows(&self) -> Option<serde_json::Value> {
        let text = std::fs::read_to_string(self.dir.join("compute-windows.json")).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Rewrite the whitelist while the registrar is running, the way an
    /// operator switch would.
    pub fn rewrite_tenants(&self, tenants: &[TenantSpec]) {
        write_tenants(&self.dir, tenants);
    }

    /// `POST <base><path>` with a JSON body. Returns the status and body.
    pub async fn post(&self, path: &str, body: serde_json::Value) -> (u16, String) {
        let response = self
            .client
            .post(format!("{}{path}", self.base))
            .json(&body)
            .send()
            .await
            .expect("request reaches the registrar");
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        (status, text)
    }

    pub async fn register(&self, id: &str, hostname: &str, token: &str) -> (u16, String) {
        self.post(
            "/register",
            serde_json::json!({ "appliance_id": id, "token": token, "hostname": hostname }),
        )
        .await
    }

    pub async fn heartbeat(&self, id: &str, token: &str) -> (u16, String) {
        self.post(
            "/heartbeat",
            serde_json::json!({ "appliance_id": id, "token": token }),
        )
        .await
    }

    /// A `/cluster/join` with the default 23:00-07:00 window.
    pub async fn join(&self, id: &str, node_name: &str, token: &str) -> (u16, String) {
        self.post(
            "/cluster/join",
            serde_json::json!({
                "appliance_id": id,
                "token": token,
                "node_name": node_name,
                "share_compute": true,
                "window_start": "23:00",
                "window_end": "07:00",
            }),
        )
        .await
    }

    /// Poll `check` until it holds or the deadline passes. Returns whether it
    /// held, so the caller can assert with a message carrying the final state.
    pub async fn wait_for<F: FnMut() -> bool>(&self, mut check: F) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if check() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        check()
    }

    /// Stop the server and wait for `serve` to return, so a panic inside it
    /// fails the test instead of vanishing with the runtime.
    /// Stop the Stripe gate while the registrar keeps running: the edge whose
    /// key was never sealed, or whose gate could not unseal it.
    pub async fn stop_gate(&mut self) {
        if let Some(stop) = self.gate_stop.take() {
            let _ = stop.send(());
        }
        if let Some(join) = self.gate_join.take() {
            let _ = tokio::time::timeout(Duration::from_secs(5), join).await;
        }
    }

    pub async fn shutdown(mut self) {
        self.stop_gate().await;
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(join) = self.join.take() {
            let outcome = tokio::time::timeout(Duration::from_secs(5), join).await;
            match outcome {
                Ok(Ok(Ok(()))) => {}
                Ok(Ok(Err(e))) => panic!("serve returned an error: {e:?}"),
                Ok(Err(e)) => panic!("serve task panicked: {e}"),
                Err(_) => panic!("serve did not stop within 5s of the shutdown signal"),
            }
        }
    }
}

/// Write `tenants.json` plus one token file per tenant that has token content.
fn write_tenants(dir: &TempDir, tenants: &[TenantSpec]) {
    let mut map: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    for spec in tenants {
        let token_path = dir.join(&format!("{}.token", spec.id));
        match &spec.token {
            Some(token) => std::fs::write(&token_path, token).expect("write token file"),
            None => {
                let _ = std::fs::remove_file(&token_path);
            }
        }
        map.insert(
            spec.id.to_string(),
            serde_json::json!({
                "hostname": spec.hostname,
                "token_file": token_path.to_string_lossy(),
                "cluster": spec.cluster,
                "market": spec.market,
                "relay_zone": spec.relay_zone,
            }),
        );
    }
    let json = serde_json::to_vec_pretty(&map).expect("serialize tenants.json");
    std::fs::write(dir.join("tenants.json"), json).expect("write tenants.json");
}

/// A stand-in for the mesh apiserver.
///
/// It exists because the property under test is *what the join handler does
/// before it hands out a token*: it must delete the caller's `Node` object and
/// its node-password `Secret`, and it must refuse the join outright if that
/// fails. Asserting that against a real rke2 cluster belongs in the VM tests;
/// asserting it here needs something that records the requests and can be told
/// to fail on demand.
///
/// Plain HTTP on loopback, which the registrar accepts for a non-`https`
/// `--kube-api` (see `cleanup_stale_node`). Nothing secret crosses it.
pub struct KubeStub {
    pub base: String,
    state: StubState,
    stop: Option<oneshot::Sender<()>>,
    join: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Clone)]
struct StubState {
    /// The `METHOD path` of every request the stub saw, in order.
    seen: Arc<Mutex<Vec<String>>>,
    /// The JSON body of every request that had one, with its `METHOD path`.
    bodies: Arc<Mutex<Vec<(String, serde_json::Value)>>>,
    /// What to answer with. 404 is the apiserver's "no such node", which the
    /// handler must treat as a clean result.
    status: StatusCode,
    /// The bearer token every request is expected to carry.
    expect_bearer: Option<String>,
    /// The `status.phase` a `GET` of a PersistentVolumeClaim reports.
    claim_phase: Arc<Mutex<String>>,
}

impl KubeStub {
    /// A stub that answers every request with `status`.
    pub async fn start(status: u16) -> Self {
        Self::start_inner(status, None, "Bound").await
    }

    /// A stub whose claims report `phase` until told otherwise.
    pub async fn start_with_claim_phase(status: u16, phase: &str) -> Self {
        Self::start_inner(status, None, phase).await
    }

    /// A stub that also asserts the `Authorization` header, so a join that
    /// forgot the ServiceAccount token fails loudly instead of passing because
    /// the stub did not care.
    pub async fn expecting_bearer(status: u16, token: &str) -> Self {
        Self::start_inner(status, Some(token.to_string()), "Bound").await
    }

    async fn start_inner(status: u16, expect_bearer: Option<String>, phase: &str) -> Self {
        let state = StubState {
            seen: Arc::new(Mutex::new(Vec::new())),
            bodies: Arc::new(Mutex::new(Vec::new())),
            status: StatusCode::from_u16(status).expect("a valid status code"),
            expect_bearer,
            claim_phase: Arc::new(Mutex::new(phase.to_string())),
        };
        let app = Router::new()
            .fallback(stub_handler)
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind the kube stub");
        let port = listener.local_addr().expect("local_addr").port();
        let (stop, rx) = oneshot::channel::<()>();
        let join = tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = rx.await;
                })
                .await;
        });
        Self {
            base: format!("http://127.0.0.1:{port}"),
            state,
            stop: Some(stop),
            join: Some(join),
        }
    }

    /// Every request the stub saw, as `METHOD path`.
    pub fn seen(&self) -> Vec<String> {
        self.state
            .seen
            .lock()
            .expect("the stub's request log is not poisoned")
            .clone()
    }

    /// The JSON body of every request that carried one, as
    /// `(METHOD path, body)`.
    pub fn bodies(&self) -> Vec<(String, serde_json::Value)> {
        self.state
            .bodies
            .lock()
            .expect("the stub's body log is not poisoned")
            .clone()
    }

    /// What a `GET` of any claim reports from now on (`Bound` by default).
    pub fn set_claim_phase(&self, phase: &str) {
        *self
            .state
            .claim_phase
            .lock()
            .expect("the stub's claim phase is not poisoned") = phase.to_string();
    }

    pub async fn shutdown(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(join) = self.join.take() {
            let _ = tokio::time::timeout(Duration::from_secs(5), join).await;
        }
    }
}

async fn stub_handler(
    axum::extract::State(state): axum::extract::State<StubState>,
    headers: axum::http::HeaderMap,
    method: Method,
    uri: Uri,
    body: axum::body::Bytes,
) -> axum::response::Response {
    state
        .seen
        .lock()
        .expect("the stub's request log is not poisoned")
        .push(format!("{method} {}", uri.path()));
    if let Ok(json) = serde_json::from_slice(&body) {
        state
            .bodies
            .lock()
            .expect("the stub's body log is not poisoned")
            .push((format!("{method} {}", uri.path()), json));
    }
    if let Some(expected) = &state.expect_bearer {
        let supplied = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        if supplied != format!("Bearer {expected}") {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }
    if method == Method::GET && uri.path().contains("/persistentvolumeclaims/") {
        let phase = state
            .claim_phase
            .lock()
            .expect("the stub's claim phase is not poisoned")
            .clone();
        // "Gone" stands for a claim an operator has deleted.
        if phase == "Gone" {
            return StatusCode::NOT_FOUND.into_response();
        }
        return axum::Json(serde_json::json!({ "status": { "phase": phase } })).into_response();
    }
    state.status.into_response()
}
