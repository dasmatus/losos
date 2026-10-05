//! Fixtures: a host built on a store the test can see into.
//!
//! The tests run the real router through the real wrapper. The store is the
//! crate's own `MemStore`, which two hosts can share by cloning, so a test can
//! stand up a "second instance" without a database; the Postgres store has its
//! own, gated, test in `tests/postgres.rs`.
#![allow(dead_code, unreachable_pub)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::Method;
use http_body_util::BodyExt;
use losos_edge_vercel::{Host, Settings, Store, TenantSpec};
use tower::ServiceExt;

/// A 64-hex-character token, the shape the docs describe for a real appliance.
pub const GOOD_TOKEN: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0";

pub struct TempDir {
    pub path: PathBuf,
}

impl TempDir {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("lev-{}-{nanos}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// The settings every test host runs with: one whitelisted tenant,
/// `demo-box` → `demo.losos.example`, a two-minute TTL, no database (the
/// store is handed to [`host`] directly).
pub fn settings(dir: &TempDir) -> Settings {
    let mut tenants = std::collections::BTreeMap::new();
    tenants.insert(
        "demo-box".to_string(),
        TenantSpec {
            hostname: "demo.losos.example".to_string(),
            token: GOOD_TOKEN.to_string(),
            cluster: false,
            market: false,
        },
    );
    Settings {
        tenants,
        bootstrap_token: "test-bootstrap-0123456789abcdef0123".to_string(),
        heartbeat_ttl: "120s".to_string(),
        port_range: "50000-50100".to_string(),
        state_dir: dir.path.clone(),
        store: None,
    }
}

/// A host on the given store (or none, meaning memory for this instance only).
pub async fn host(dir: &TempDir, store: Option<Box<dyn Store>>) -> Arc<Host> {
    Arc::new(
        Host::with_store(&settings(dir), store)
            .await
            .expect("build host"),
    )
}

/// Drive one request through the router, in-process, exactly as Vercel's
/// bridge would hand it over. Returns the status and the body as text.
pub async fn call(
    host: &Arc<Host>,
    method: Method,
    path: &str,
    body: Option<&str>,
) -> (u16, String) {
    let mut builder = axum::http::Request::builder().method(method).uri(path);
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    let request = builder
        .body(axum::body::Body::from(body.unwrap_or("").to_string()))
        .expect("request");
    let response = host
        .router()
        .oneshot(request)
        .await
        .expect("router is infallible");
    let status = response.status().as_u16();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

pub fn register_body(id: &str, hostname: &str, token: &str) -> String {
    serde_json::json!({ "appliance_id": id, "token": token, "hostname": hostname }).to_string()
}

pub fn heartbeat_body(id: &str, token: &str, idle: bool) -> String {
    serde_json::json!({ "appliance_id": id, "token": token, "idle": idle }).to_string()
}
