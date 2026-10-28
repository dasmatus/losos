//! The two demonstration routes, and the view they serve.
//!
//! `/status` is what the self-hosted edge deliberately does not have: a public
//! list of who is registered. On a VPS the registrar's only observers are
//! Traefik and rathole, reading files, and the operator, reading the journal.
//! A demonstration needs a window, so this host adds one — read-only, hostnames
//! and liveness only, never a token — and the page in `public/` draws it.
//! `/status/traefik` shows the routes the reconciler just decided, i.e.
//! exactly the document a real edge's Traefik would be reading from the
//! registrar's provider listener right now.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use losos_registrar::{ComputeWindow, Snapshot};
use serde::Serialize;

use crate::host::Host;

/// What a self-hosted edge runs that this host cannot. Listed in the status
/// body so the page can say so rather than leave the audience to infer it.
pub const NOT_ON_THIS_HOST: [&str; 4] = [
    "rathole tunnel server (raw TCP, long-lived connections)",
    "Traefik with per-tenant Let's Encrypt certificates",
    "rke2 mesh control plane",
    "Stripe gate (Unix socket, systemd credentials)",
];

#[derive(Debug, Serialize)]
pub struct Status {
    /// Always `"vercel"`; lets a page tell this host from a real edge.
    pub host: &'static str,
    /// `"postgres"` or `"memory"`.
    pub store: &'static str,
    pub heartbeat_ttl_secs: u64,
    pub generated_at_unix: u64,
    pub tenants: Vec<TenantStatus>,
    pub compute_windows: BTreeMap<String, ComputeWindow>,
    pub not_on_this_host: [&'static str; 4],
}

#[derive(Debug, Serialize)]
pub struct TenantStatus {
    pub id: String,
    pub hostname: String,
    pub rathole_port: u16,
    pub seen_ago_secs: u64,
    /// The box's last idle report, if one is younger than the TTL. Stale is
    /// not idle — the same rule the mesh taint applies.
    pub idle: Option<bool>,
}

impl Status {
    pub(crate) fn from_snapshot(snapshot: Snapshot, ttl: Duration, store: &'static str) -> Self {
        let tenants = snapshot
            .tenants
            .into_iter()
            .map(|(id, t)| TenantStatus {
                id,
                hostname: t.hostname,
                rathole_port: t.rathole_port,
                seen_ago_secs: t.seen_ago_secs,
                idle: t
                    .idle
                    .filter(|(_, ago)| Duration::from_secs(*ago) < ttl)
                    .map(|(idle, _)| idle),
            })
            .collect();
        Self {
            host: "vercel",
            store,
            heartbeat_ttl_secs: ttl.as_secs(),
            generated_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
            tenants,
            compute_windows: snapshot.compute_windows,
            not_on_this_host: NOT_ON_THIS_HOST,
        }
    }
}

/// The routes, with the host as their state. Merged into the registrar's
/// router by [`Host::router`], which also puts them under the same per-request
/// load/reconcile/save wrapper, so `/status` reads the registry as it is after
/// this request's reconcile pass — pruned, like the real edge's files.
pub(crate) fn routes(host: Arc<Host>) -> Router {
    Router::new()
        .route("/status", get(status))
        .route("/status/traefik", get(traefik))
        .with_state(host)
}

async fn status(State(host): State<Arc<Host>>) -> Json<Status> {
    Json(host.status().await)
}

async fn traefik(State(host): State<Arc<Host>>) -> Response {
    // Pretty-printed for the page; Traefik reads the compact form. `{}` is the
    // zero-tenant document on a real edge too.
    let doc = host.traefik_config();
    let body = serde_json::from_str::<serde_json::Value>(&doc)
        .and_then(|v| serde_json::to_string_pretty(&v))
        .unwrap_or(doc);
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        body,
    )
        .into_response()
}
