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
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::http::{Method, StatusCode, Uri};
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
/// Uniquified by pid + a monotonic clock reading so `cargo test`'s parallel
/// threads (and two concurrent `cargo test` runs) never share one.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "losos-registrar-test-{tag}-{}-{nanos}",
            std::process::id()
        ));
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
}

impl TenantSpec {
    pub fn new(id: &'static str, hostname: &str, token: &str) -> Self {
        Self {
            id,
            hostname: hostname.to_string(),
            token: Some(token.to_string()),
            cluster: false,
        }
    }

    /// A tenant whose token file is never created.
    pub fn without_token_file(id: &'static str, hostname: &str) -> Self {
        Self {
            id,
            hostname: hostname.to_string(),
            token: None,
            cluster: false,
        }
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

/// A running registrar: its temp state directory, its base URL, and the
/// handles needed to stop it.
pub struct Edge {
    pub dir: TempDir,
    pub base: String,
    pub client: reqwest::Client,
    stop: Option<oneshot::Sender<()>>,
    join: Option<tokio::task::JoinHandle<miette::Result<()>>>,
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
        let dir = TempDir::new(tag);
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

        let opts = ServeOpts {
            listen: "127.0.0.1:0".to_string(),
            registry_path: dir.path_str("registry.json"),
            traefik_dir: dir.path_str("traefik"),
            rathole_config: dir.path_str("server.toml"),
            rathole_bind_addr: "0.0.0.0".to_string(),
            rathole_bind_port: 2333,
            port_range: (50000, 50100),
            bootstrap_token_file: dir.path_str("bootstrap.token"),
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
        };

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral port");
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
    pub async fn shutdown(mut self) {
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
    /// What to answer with. 404 is the apiserver's "no such node", which the
    /// handler must treat as a clean result.
    status: StatusCode,
    /// The bearer token every request is expected to carry.
    expect_bearer: Option<String>,
}

impl KubeStub {
    /// A stub that answers every request with `status`.
    pub async fn start(status: u16) -> Self {
        Self::start_inner(status, None).await
    }

    /// A stub that also asserts the `Authorization` header, so a join that
    /// forgot the ServiceAccount token fails loudly instead of passing because
    /// the stub did not care.
    pub async fn expecting_bearer(status: u16, token: &str) -> Self {
        Self::start_inner(status, Some(token.to_string())).await
    }

    async fn start_inner(status: u16, expect_bearer: Option<String>) -> Self {
        let state = StubState {
            seen: Arc::new(Mutex::new(Vec::new())),
            status: StatusCode::from_u16(status).expect("a valid status code"),
            expect_bearer,
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
) -> StatusCode {
    state
        .seen
        .lock()
        .expect("the stub's request log is not poisoned")
        .push(format!("{method} {}", uri.path()));
    if let Some(expected) = &state.expect_bearer {
        let supplied = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        if supplied != format!("Bearer {expected}") {
            return StatusCode::UNAUTHORIZED;
        }
    }
    state.status
}
