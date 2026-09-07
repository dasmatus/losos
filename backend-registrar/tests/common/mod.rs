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
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use losos_registrar::opts::ServeOpts;
use tokio::sync::oneshot;

/// A 64-hex-character token, the shape the docs describe for a real appliance.
pub const GOOD_TOKEN: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0";
/// A second valid token, distinct from [`GOOD_TOKEN`].
pub const OTHER_TOKEN: &str = "abad1deaabad1deaabad1deaabad1deaabad1deaabad1deaabad1deaabad1dea";
/// The rathole `default_token` the edge is configured with.
pub const BOOTSTRAP_TOKEN: &str =
    "b00757241pb00757241pb00757241pb00757241pb00757241pb00757241pb007";

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
/// claim, and the contents of its token file (`None` writes no file at all —
/// the "operator forgot to place the secret" case).
pub struct TenantSpec {
    pub id: &'static str,
    pub hostname: String,
    pub token: Option<String>,
}

impl TenantSpec {
    pub fn new(id: &'static str, hostname: &str, token: &str) -> Self {
        Self {
            id,
            hostname: hostname.to_string(),
            token: Some(token.to_string()),
        }
    }

    /// A tenant whose token file is never created.
    pub fn without_token_file(id: &'static str, hostname: &str) -> Self {
        Self {
            id,
            hostname: hostname.to_string(),
            token: None,
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
        let dir = TempDir::new(tag);
        std::fs::create_dir_all(dir.join("traefik")).expect("create traefik dir");
        std::fs::write(dir.join("bootstrap.token"), BOOTSTRAP_TOKEN).expect("write bootstrap");
        write_tenants(&dir, tenants);

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
            }),
        );
    }
    let json = serde_json::to_vec_pretty(&map).expect("serialize tenants.json");
    std::fs::write(dir.join("tenants.json"), json).expect("write tenants.json");
}
