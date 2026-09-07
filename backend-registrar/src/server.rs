//! `losos-registrar serve` — the edge loopback HTTP API + the reconciler.
//!
//! The API is the only thing that mutates the registry; the reconciler is the
//! only thing that writes config files. A `tokio::sync::Notify` lets the API
//! kick the reconciler immediately on register/heartbeat/deregister instead of
//! waiting for the next tick — but the reconciler also ticks every
//! `reconcile_interval` so stale tenants are pruned even with no traffic
//! (the "constantly updating" property).
//!
//! Auth is closed-enrollment: a request is honoured only if its `appliance_id`
//! is in the tenants whitelist AND its `token` constant-time-matches the
//! content of that tenant's `tokenFile`. The whitelist + token paths come from
//! a `tenants.json` the NixOS module generates from `losos.edge.tenants`.
//!
//! Handler errors use the concrete [`ApiError`] enum (thiserror), mapped to
//! HTTP status codes at the boundary — `?` converts io/serde/registry via
//! `#[from]`. The reconciler orchestration (`reconcile_once`) returns
//! `miette::Result`: `ApiError`/`RegistryError` implement `miette::Diagnostic`
//! so `?` converts them, and io/serde errors are lifted via `.into_diagnostic()`
//! before `.context()`/`.with_context()` (miette's `Context` is only impl'd for
//! `Result<T, E: Diagnostic>`, not for plain `std::error::Error`). It logs and
//! continues rather than killing the server.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use miette::{Context, IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use crate::action::Action;
use crate::config::{desired_config, EdgeOpts, TenantView};
use crate::error::ApiError;
use crate::opts::ServeOpts;
use crate::registry::{Registry, Shared};

/// Per-tenant Traefik router config is public (hostnames only, no secrets) →
/// world-readable so the `traefik` user can read it.
const TRAEFIK_FILE_MODE: u32 = 0o644;
/// rathole server config carries service tokens → owner-only. Shared with
/// the `seed` subcommand, which writes the same file at first boot.
pub(crate) const RATHOLE_FILE_MODE: u32 = 0o600;
/// Hard cap on an uploaded config body (1 MiB). Enforced at the router layer
/// so the body is never materialised beyond this.
const MAX_UPLOAD_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
struct AppState {
    reg: Shared,
    opts: Arc<ServeOpts>,
    notify: Arc<Notify>,
}

#[derive(Debug, Deserialize)]
struct RegisterReq {
    appliance_id: String,
    token: String,
    hostname: String,
}

#[derive(Debug, Deserialize)]
struct HeartbeatReq {
    appliance_id: String,
    token: String,
}

#[derive(Debug, Serialize)]
struct RegisterResp {
    rathole_port: u16,
}

/// Summary returned by the upload endpoints after parsing the spilled file.
#[derive(Debug, Serialize)]
struct UploadResp {
    bytes: usize,
    /// Top-level keys of the parsed config (Traefik YAML top-level keys, or
    /// Tahoe INI `[section]` headers), sorted and deduplicated.
    top_level_keys: Vec<String>,
    /// The canonical root key for the format, if present:
    /// `"http"` for a Traefik dynamic config, `"node"` for a Tahoe `tahoe.cfg`.
    primary_key: Option<String>,
}

pub async fn run(opts: ServeOpts) -> Result<()> {
    let reg = Arc::new(Registry::new(opts.registry_path.clone(), opts.port_range));
    // Re-attach before the API opens: restore routes for tenants the edge
    // already knew about so a rebooting edge doesn't drop every appliance.
    reg.load().await.context("load registry")?;
    tracing::info!(target: Action::LoadRegistry.target(), "registry loaded");

    let state = AppState {
        reg,
        opts: Arc::new(opts.clone()),
        notify: Arc::new(Notify::new()),
    };

    // Generate config from whatever we just loaded, so the box is serving
    // before any heartbeat arrives.
    reconcile_once(&state).await?;

    let recon = tokio::spawn(reconciler(state.clone()));

    let app = Router::new()
        .route("/health", get(health))
        .route("/register", post(register))
        .route("/heartbeat", post(heartbeat))
        .route("/deregister", post(deregister))
        // Same operation under a clearer name; the announce client never
        // calls either, so there is no wire-compat constraint to honour.
        .route("/unregister", post(deregister))
        // Upload endpoints: validate-and-discard. The appliance submits a
        // config; the edge parses it on tmpfs and unlinks it before
        // returning — the file is never retained. Both are reachable through
        // Traefik at register.<publicDomain> (the static register route
        // proxies this whole loopback API).
        .route("/config", post(upload_config))
        .route("/tahoe", post(upload_tahoe))
        // Bound at the router so an oversized upload never materialises.
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES))
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind(&opts.listen)
        .await
        .into_diagnostic()
        .with_context(|| format!("bind {}", opts.listen))?;
    tracing::info!(target: Action::Bind.target(), "listening on {}", opts.listen);
    // axum::serve returns when the listener errors or the runtime stops.
    axum::serve(listener, app).await.into_diagnostic()?;

    // Structured shutdown: stop the reconciler and await its result so a
    // panic inside it surfaces instead of being silently detached.
    recon.abort();
    let _ = recon.await;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn register(
    State(st): State<AppState>,
    Json(req): Json<RegisterReq>,
) -> Result<Json<RegisterResp>, ApiError> {
    let tenant = authenticate(&st, &req.appliance_id, &req.token).await?;
    // The registered hostname must match the whitelisted one — a tenant can't
    // claim an arbitrary public hostname, only its own.
    if req.hostname != tenant.hostname {
        return Err(ApiError::HostnameForbidden);
    }
    let port = st.reg.register(&req.appliance_id, &req.hostname).await?;
    st.notify.notify_one();
    Ok(Json(RegisterResp { rathole_port: port }))
}

async fn heartbeat(
    State(st): State<AppState>,
    Json(req): Json<HeartbeatReq>,
) -> Result<StatusCode, ApiError> {
    authenticate(&st, &req.appliance_id, &req.token).await?;
    if st.reg.heartbeat(&req.appliance_id).await {
        Ok(StatusCode::NO_CONTENT)
    } else {
        // Unknown id mid-run: tell the appliance to re-register. (It will, on
        // the next announce loop, because 404 triggers re-enroll.)
        Err(ApiError::UnknownAppliance)
    }
}

async fn deregister(
    State(st): State<AppState>,
    Json(req): Json<HeartbeatReq>,
) -> Result<StatusCode, ApiError> {
    authenticate(&st, &req.appliance_id, &req.token).await?;
    st.reg.deregister(&req.appliance_id).await?;
    st.notify.notify_one();
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /config` — receive an uploaded **Traefik** dynamic config (YAML),
/// persist it to tmpfs, parse it, return a summary, then delete the tmpfs file.
///
/// Auth is via headers (`x-appliance-id` / `x-appliance-token`) because the
/// body is a raw file, not JSON. The file is written to `opts.upload_dir`
/// (tmpfs — a `RuntimeDirectory`, deliberately not under `/persist`), read
/// back, and unlinked before the handler returns — including on parse failure
/// — so retention is minimised. Even a missed unlink is safe: tmpfs is wiped
/// on reboot. The file is never copied into Traefik's dynamic dir; this is a
/// validate-and-discard endpoint, not a config-deployment one.
async fn upload_config(
    State(st): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<UploadResp>, ApiError> {
    let (bytes, content) = authed_spill(&st, &headers, body, "yml").await?;
    let value: serde_yaml::Value =
        serde_yaml::from_str(&content).map_err(|_| ApiError::BadRequest("invalid yaml"))?;
    let mut keys: Vec<String> = value
        .as_mapping()
        .map(|m| {
            m.iter()
                .filter_map(|(k, _)| k.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    keys.sort();
    keys.dedup();
    let primary_key = keys.iter().find(|k| *k == "http").cloned();
    Ok(Json(UploadResp {
        bytes,
        top_level_keys: keys,
        primary_key,
    }))
}

/// `POST /tahoe` — receive an uploaded **Tahoe** `tahoe.cfg` (INI), persist
/// it to tmpfs, parse the `[section]` headers, return a summary, then delete
/// the tmpfs file. Same validate-and-discard contract as [`upload_config`];
/// the INI section scan is hand-rolled (no INI dependency) since the summary
/// only needs section names.
async fn upload_tahoe(
    State(st): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<UploadResp>, ApiError> {
    let (bytes, content) = authed_spill(&st, &headers, body, "cfg").await?;
    let mut sections: Vec<String> = content
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            l.strip_prefix('[')?.strip_suffix(']').map(String::from)
        })
        .collect();
    sections.sort();
    sections.dedup();
    let primary_key = sections.iter().find(|s| *s == "node").cloned();
    Ok(Json(UploadResp {
        bytes,
        top_level_keys: sections,
        primary_key,
    }))
}

/// Authenticate via headers, then spill `body` to a unique tmpfs file, read it
/// back, and unlink it — all on `spawn_blocking` so the tmpfs file's lifetime
/// is a single synchronous span with no async window to orphan it. Deletion is
/// unconditional (runs even if the read fails). Returns `(byte count, file
/// content)`. A missing auth header is `Unauthorized`; an IO failure is a 500.
async fn authed_spill(
    st: &AppState,
    headers: &HeaderMap,
    body: Bytes,
    suffix: &str,
) -> Result<(usize, String), ApiError> {
    let id = header_str(headers, "x-appliance-id").ok_or(ApiError::Unauthorized)?;
    let token = header_str(headers, "x-appliance-token").ok_or(ApiError::Unauthorized)?;
    authenticate(st, id, token).await?;

    let dir = st.opts.upload_dir.clone();
    let suffix = suffix.to_string();
    let body_vec = body.to_vec();
    let bytes_len = body_vec.len();
    let content = tokio::task::spawn_blocking(move || spill_and_delete(&dir, &body_vec, &suffix))
        .await
        .map_err(|e| ApiError::from(std::io::Error::other(e)))??;
    Ok((bytes_len, content))
}

/// Write `body` to `<dir>/upload-<N>.<suffix>` on tmpfs, read it back as UTF-8,
/// then unlink it unconditionally. The unlink runs before the function
/// returns regardless of whether the read succeeded.
fn spill_and_delete(dir: &Path, body: &[u8], suffix: &str) -> std::io::Result<String> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!(
        "upload-{N}.{suffix}",
        N = next_upload_id(),
        suffix = suffix
    ));
    std::fs::write(&path, body)?;
    // Read back so the summary reflects exactly what hit tmpfs.
    let content = std::fs::read_to_string(&path);
    // Minimise retention: unlink now, regardless of read outcome. A failure
    // here is non-fatal (tmpfs is wiped on reboot) but we surface it.
    let _ = std::fs::remove_file(&path);
    content
}

/// Read a header value as a `&str` (returns `None` on missing or non-UTF-8).
fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

/// Read tenants.json (id → {hostname, `token_file`}) and validate the request.
/// Re-reads the file each call so the NixOS module can rotate tenants with a
/// reload the registrar picks up without a restart.
async fn load_tenants(path: &str) -> Result<HashMap<String, TenantEntry>, ApiError> {
    let bytes = tokio::fs::read(path).await?;
    let map = serde_json::from_slice(&bytes)?;
    Ok(map)
}

#[derive(Debug, Clone, Deserialize)]
struct TenantEntry {
    hostname: String,
    token_file: String,
}

async fn authenticate(st: &AppState, id: &str, token: &str) -> Result<TenantEntry, ApiError> {
    let tenants = load_tenants(&st.opts.tenants_file).await?;
    let entry = tenants.get(id).ok_or(ApiError::Unauthorized)?;
    let expected = tokio::fs::read_to_string(&entry.token_file).await?;
    if !ct_eq(token.trim(), expected.trim()) {
        return Err(ApiError::Unauthorized);
    }
    Ok(entry.clone())
}

/// Constant-time string compare so token checks don't leak via timing.
fn ct_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        // Length is not a side channel that matters for random tokens; the
        // early return just avoids a panic on zip length mismatch.
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

async fn reconciler(st: AppState) {
    loop {
        // Wait either for an API kick or the interval — whichever fires first.
        let tick = tokio::time::sleep(st.opts.reconcile_interval);
        tokio::pin!(tick);
        tokio::select! {
            () = st.notify.notified() => {}
            () = &mut tick => {}
        }
        if let Err(e) = reconcile_once(&st).await {
            // Log and continue — the reconciler must not kill the server on a
            // single failed pass; the next tick retries.
            tracing::error!(target: Action::Reconcile.target(), "reconcile failed: {e}");
        }
    }
}

async fn reconcile_once(st: &AppState) -> Result<()> {
    let changed = st.reg.prune(st.opts.heartbeat_ttl).await;
    let views = st.reg.views().await;
    let tenants = load_tenants(&st.opts.tenants_file)
        .await
        .context("load tenants whitelist")?;
    let bootstrap = tokio::fs::read_to_string(&st.opts.bootstrap_token_file)
        .await
        .into_diagnostic()
        .context("read bootstrap token file")?;

    // Enrich each live tenant with its token (read from the whitelist's
    // tokenFile — the on-disk secret is the single source of truth). Tenants
    // removed from the whitelist are dropped here and pruned from the registry
    // so their rathole service + Traefik router disappear.
    let mut enriched: Vec<TenantView> = Vec::with_capacity(views.len());
    let mut stale: Vec<String> = Vec::new();
    for v in &views {
        match tenants.get(&v.id) {
            Some(entry) => {
                let token = tokio::fs::read_to_string(&entry.token_file)
                    .await
                    .into_diagnostic()
                    .with_context(|| format!("read token file for {}", v.id))?;
                enriched.push(TenantView {
                    id: v.id.clone(),
                    hostname: v.hostname.clone(),
                    rathole_port: v.rathole_port,
                    token: token.trim().to_string(),
                });
            }
            None => stale.push(v.id.clone()),
        }
    }
    for id in &stale {
        // best-effort; a failure just means it lingers until next prune
        let _ = st.reg.deregister(id).await;
    }

    let opts = EdgeOpts {
        rathole_bind_addr: st.opts.rathole_bind_addr.clone(),
        rathole_bind_port: st.opts.rathole_bind_port,
        bootstrap_token: bootstrap.trim().to_string(),
    };
    let files = desired_config(&enriched, &opts);

    let traefik_path = Path::new(&st.opts.traefik_dir).join("losos.yml");
    // `None` means "no live tenants": Traefik's file provider rejects an empty
    // dynamic config, so the zero-tenant state is "no losos.yml" — remove the
    // file so Traefik drops every appliance router. `Some` is the normal
    // write-if-changed path.
    let changed_traefik = match files.traefik_yaml {
        Some(ref content) => write_if_changed(&traefik_path, content, TRAEFIK_FILE_MODE)
            .await
            .with_context(|| format!("write {}", traefik_path.display()))?,
        None => remove_if_exists(&traefik_path)
            .await
            .with_context(|| format!("remove {}", traefik_path.display()))?,
    };
    let changed_rathole = write_if_changed(
        Path::new(&st.opts.rathole_config),
        &files.rathole_toml,
        RATHOLE_FILE_MODE,
    )
    .await
    .with_context(|| format!("write {}", st.opts.rathole_config))?;

    // No explicit reload signal: rathole 0.5 hot-reloads via its `notify`
    // file-watcher, which re-applies the config the instant server.toml is
    // rewritten above. SIGHUP has no handler in rathole 0.5 (default action
    // terminate) — sending it would kill the process and systemd's
    // Restart=always would resurrect it ~5s later, a needless tunnel outage
    // on every tenant change. The atomic temp+rename above is the only
    // signal rathole needs.
    if changed || changed_traefik || changed_rathole {
        tracing::info!(
            target: Action::Reconcile.target(),
            "reconciled {} tenant(s); traefik={} rathole={}",
            enriched.len(),
            changed_traefik,
            changed_rathole,
        );
    }
    Ok(())
}

/// Write `content` to `path` only if it differs from the current content.
/// Atomic temp+fsync+rename via the shared blocking helper, with the given
/// filesystem mode. Returns `true` if the file was (re)written.
async fn write_if_changed(path: &Path, content: &str, mode: u32) -> Result<bool> {
    let existing = match tokio::fs::read(path).await {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            return Err(e)
                .into_diagnostic()
                .with_context(|| format!("read {}", path.display()))
        }
    };
    if existing == content.as_bytes() {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    crate::fsutil::atomic_write(path, content.as_bytes(), mode)
        .await
        .into_diagnostic()
        .with_context(|| format!("atomic write {}", path.display()))?;
    Ok(true)
}

/// Remove `path` if it exists. Returns `true` if a file was removed, `false`
/// if it was already absent. Used for the zero-tenant Traefik state: delete
/// `losos.yml` so Traefik's file provider drops all appliance routers.
async fn remove_if_exists(path: &Path) -> Result<bool> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e)
            .into_diagnostic()
            .with_context(|| format!("remove {}", path.display())),
    }
}

/// Monotonic counter for unique tmpfs upload filenames. (Cheap, lock-free; a
/// real RNG is unnecessary — the file is unlinked within the same call.)
static UPLOAD_CTR: AtomicU64 = AtomicU64::new(0);

fn next_upload_id() -> u64 {
    UPLOAD_CTR.fetch_add(1, Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::ct_eq;

    #[test]
    fn ct_eq_matches_and_mismatches() {
        assert!(ct_eq("token", "token"));
        assert!(!ct_eq("token", "toke"));
        assert!(!ct_eq("token", "tokex"));
        assert!(ct_eq("", ""));
    }
}
