//! Fixtures: a stand-in for Upstash, and a host built against it.
//!
//! The fake speaks the two commands the store uses, in Upstash's REST shape,
//! and nothing else — the point is to drive the real `Store` over real HTTP,
//! not to model Redis.
#![allow(dead_code, unreachable_pub)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::{Method, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use http_body_util::BodyExt;
use losos_edge_vercel::{Host, Settings, StoreSettings, TenantSpec};
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

/// The fake's whole state: the key space.
pub type Kv = Arc<Mutex<HashMap<String, String>>>;

/// Start a fake Upstash on an ephemeral port. Returns its base URL and the
/// key space, so a test can seed or inspect what the store saved.
pub async fn fake_upstash() -> (String, Kv) {
    let kv: Kv = Arc::default();
    let app = Router::new()
        .route("/", post(command))
        .with_state(Arc::clone(&kv));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fake upstash");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    // The store insists on https for a real endpoint; the test endpoint is
    // loopback, so the settings are built directly rather than from env.
    (format!("http://{addr}"), kv)
}

async fn command(
    State(kv): State<Kv>,
    Json(argv): Json<Vec<serde_json::Value>>,
) -> (StatusCode, Json<serde_json::Value>) {
    let words: Vec<String> = argv
        .iter()
        .map(|v| match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .collect();
    let mut kv = kv.lock().expect("kv lock");
    match words.first().map(String::as_str) {
        Some("GET") if words.len() == 2 => {
            let value = kv.get(&words[1]).cloned();
            (StatusCode::OK, Json(serde_json::json!({ "result": value })))
        }
        Some("SET") if words.len() == 3 => {
            kv.insert(words[1].clone(), words[2].clone());
            (StatusCode::OK, Json(serde_json::json!({ "result": "OK" })))
        }
        _ => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("unsupported command {words:?}") })),
        ),
    }
}

/// A host with one whitelisted tenant, `demo-box` → `demo.losos.example`, a
/// two-minute TTL, and the given store (or none).
pub async fn host(dir: &TempDir, store_url: Option<&str>) -> Arc<Host> {
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
    let settings = Settings {
        tenants,
        bootstrap_token: "test-bootstrap-0123456789abcdef0123".to_string(),
        heartbeat_ttl: "120s".to_string(),
        port_range: "50000-50100".to_string(),
        state_dir: dir.path.clone(),
        store: store_url.map(|url| StoreSettings {
            url: url.to_string(),
            token: "test-token".to_string(),
            key: "losos:test".to_string(),
        }),
    };
    Arc::new(Host::new(&settings).await.expect("build host"))
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
