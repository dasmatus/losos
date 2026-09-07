//! `losos-registrar serve` — the edge registration HTTP API + the reconciler.
//!
//! **This API is on the public internet.** `modules/edge.nix` gives Traefik a
//! `Host(register.<domain>)` router with no path rule and no middleware in
//! front of it, so every route below answers anyone who can resolve that name,
//! and the unit runs as root. Treat everything here as hostile input.
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
//! a `tenants.json` the NixOS module generates from `losos.edge.tenants`. The
//! whitelist is also the *hostname* authority: [`reconcile_once`] builds every
//! Traefik router from `tenants.json`, never from the registry's stored copy,
//! so an operator hostname change lands on the next tick and a tampered
//! `registry.json` cannot mint a router (or an ACME request) for a hostname
//! the operator never listed.
//!
//! Token files are hand-placed `/var/secrets/*` — nothing in the flake creates
//! them — so [`token_fault`] treats a missing, empty, short or
//! control-character-bearing token file as an operator fault and refuses that
//! tenant loudly. Without that check a zero-byte token file authenticates a
//! request carrying `"token": ""`, which is a complete tenant takeover: the
//! caller gets the victim's Traefik router, its Let's Encrypt cert, and a
//! rathole service provisioned with an empty token.
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
use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use miette::{miette, Context, IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, Notify, Semaphore};

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

/// Hard cap on a request body. Every route takes a three-field JSON object;
/// 16 KiB is orders of magnitude more than any of them needs. Enforced at the
/// router layer so an oversized body is never materialised.
const MAX_BODY_BYTES: usize = 16 * 1024;

/// Shortest on-disk token the registrar will honour. The appliance token the
/// docs describe is 64 hex characters; anything under 32 is either a
/// truncated write, a placeholder, or an empty file — none of which should be
/// able to authenticate a tenant on an internet-facing route.
const MIN_TOKEN_LEN: usize = 32;

/// Wall-clock budget for one request, end to end. Without it a slow-loris
/// client holds a connection (and a concurrency permit) indefinitely.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Requests allowed in flight at once. Every authenticated *and*
/// unauthenticated request costs a `tenants.json` stat and (for a known id) a
/// token-file read, so an unbounded arrival rate is an unbounded IO rate on
/// the edge's root filesystem. Excess is shed with 503 rather than queued —
/// queueing under a flood just converts a CPU problem into a memory one.
const MAX_INFLIGHT: usize = 64;

/// A value no real token can equal, used to make the unknown-id branch do the
/// same compare the known-id branch does. See [`decoy_probe`].
const DECOY_TOKEN: &str = "\0decoy\0";

#[derive(Clone)]
struct AppState {
    reg: Shared,
    opts: Arc<ServeOpts>,
    notify: Arc<Notify>,
    tenants: Arc<TenantCache>,
    limiter: Arc<Semaphore>,
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

/// Bind `opts.listen` and serve until SIGTERM/SIGINT.
pub async fn run(opts: ServeOpts) -> Result<()> {
    let listener = TcpListener::bind(&opts.listen)
        .await
        .into_diagnostic()
        .with_context(|| format!("bind {}", opts.listen))?;
    tracing::info!(target: Action::Bind.target(), "listening on {}", opts.listen);
    serve(listener, opts, shutdown_signal()).await
}

/// Serve on an already-bound listener until `shutdown` resolves.
///
/// Split out from [`run`] so the caller owns both the socket and the stop
/// condition: production passes a signal future, tests pass an ephemeral
/// listener (port 0) and a oneshot. In-flight requests finish before the
/// listener closes — a registrar killed mid-`/register` would otherwise leave
/// the appliance to time out and retry against a `registry.json` that may or
/// may not have been written.
pub async fn serve<F>(listener: TcpListener, opts: ServeOpts, shutdown: F) -> Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    let reg = Arc::new(Registry::new(opts.registry_path.clone(), opts.port_range));
    // Re-attach before the API opens: restore routes for tenants the edge
    // already knew about so a rebooting edge doesn't drop every appliance.
    reg.load().await.context("load registry")?;
    tracing::info!(target: Action::LoadRegistry.target(), "registry loaded");

    let state = AppState {
        reg,
        opts: Arc::new(opts),
        notify: Arc::new(Notify::new()),
        tenants: Arc::new(TenantCache::default()),
        limiter: Arc::new(Semaphore::new(MAX_INFLIGHT)),
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
        // Bound at the router so an oversized body never materialises.
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        // Added last, so outermost: the timeout and the concurrency cap cover
        // body reading, routing and 404s, not just handler bodies.
        .layer(middleware::from_fn_with_state(state.clone(), guard))
        .with_state(state.clone());

    // axum::serve returns when the listener errors or `shutdown` resolves.
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
        .into_diagnostic()?;
    tracing::info!(target: Action::Serve.target(), "http server stopped");

    // Structured shutdown: stop the reconciler and await its result so a
    // panic inside it surfaces instead of being silently detached.
    recon.abort();
    let _ = recon.await;
    Ok(())
}

/// Resolve on SIGTERM (systemd's stop signal) or SIGINT.
async fn shutdown_signal() {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                tracing::warn!(target: Action::Serve.target(), "no SIGTERM handler: {e}");
                // Never resolve, so the ctrl_c arm stays the live one.
                std::future::pending::<()>().await;
            }
        }
    };
    tokio::select! {
        () = interrupt => {}
        () = terminate => {}
    }
    tracing::info!(target: Action::Serve.target(), "shutdown signal received");
}

/// Outermost middleware: shed load past [`MAX_INFLIGHT`], then bound whatever
/// runs inside by [`REQUEST_TIMEOUT`].
async fn guard(State(st): State<AppState>, req: Request, next: Next) -> Response {
    let Ok(_permit) = Arc::clone(&st.limiter).try_acquire_owned() else {
        tracing::warn!(target: Action::Serve.target(), "shedding request: {MAX_INFLIGHT} in flight");
        return (StatusCode::SERVICE_UNAVAILABLE, "busy; retry later").into_response();
    };
    match tokio::time::timeout(REQUEST_TIMEOUT, next.run(req)).await {
        Ok(response) => response,
        Err(_) => {
            tracing::warn!(target: Action::Serve.target(), "request exceeded {REQUEST_TIMEOUT:?}");
            (StatusCode::GATEWAY_TIMEOUT, "request timed out").into_response()
        }
    }
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
    let port = st.reg.register(&req.appliance_id, &tenant.hostname).await?;
    tracing::info!(
        target: Action::Register.target(),
        "registered {} -> {} on port {port}",
        req.appliance_id,
        tenant.hostname,
    );
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
    tracing::info!(target: Action::Deregister.target(), "deregistered {}", req.appliance_id);
    st.notify.notify_one();
    Ok(StatusCode::NO_CONTENT)
}

/// One tenant of the operator whitelist (`tenants.json`): the hostname it is
/// allowed to claim and the path of the file holding its token. The path is
/// public (it lives in the nix store); the *contents* are the secret.
#[derive(Debug, Clone, Deserialize)]
struct TenantEntry {
    hostname: String,
    token_file: String,
}

/// `tenants.json` memoised behind an mtime+size check.
///
/// The file is regenerated by a NixOS switch, not per request, but it was
/// being read and JSON-parsed on *every* request — including every
/// unauthenticated one, which made an unauthenticated flood a filesystem
/// amplifier. Stat-and-reuse keeps the "operator can rotate tenants without a
/// restart" property while making the steady-state cost one `statx`.
#[derive(Debug, Default)]
struct TenantCache {
    cached: Mutex<Option<Cached>>,
}

#[derive(Debug)]
struct Cached {
    stamp: Stamp,
    tenants: Arc<HashMap<String, TenantEntry>>,
}

/// The cheap identity of a file: modification time and length. A nix store
/// path swap changes both; an in-place edit changes at least one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    mtime: Option<SystemTime>,
    len: u64,
}

impl TenantCache {
    async fn load(&self, path: &str) -> Result<Arc<HashMap<String, TenantEntry>>, ApiError> {
        let meta = tokio::fs::metadata(path).await?;
        let stamp = Stamp {
            mtime: meta.modified().ok(),
            len: meta.len(),
        };
        let mut slot = self.cached.lock().await;
        if let Some(hit) = slot.as_ref().filter(|c| c.stamp == stamp) {
            return Ok(Arc::clone(&hit.tenants));
        }
        let bytes = tokio::fs::read(path).await?;
        let tenants: Arc<HashMap<String, TenantEntry>> = Arc::new(serde_json::from_slice(&bytes)?);
        tracing::info!(
            target: Action::Authenticate.target(),
            "loaded tenant whitelist: {} tenant(s)",
            tenants.len(),
        );
        *slot = Some(Cached {
            stamp,
            tenants: Arc::clone(&tenants),
        });
        Ok(tenants)
    }
}

/// Why an on-disk token is unusable, or `None` if it is fit to authenticate
/// with. `token` must already be trimmed.
///
/// Nothing in the flake writes these files — they are placed by hand under
/// `/var/secrets/` — so every failure mode here is an operator mistake that
/// must be loud and must *not* fall through to a comparison:
///   * empty (a zero-byte or whitespace-only file) would otherwise match a
///     request that supplies `"token": ""`;
///   * short means a truncated write or a placeholder;
///   * a control character would additionally be written into `server.toml`,
///     where it is only safe because [`crate::config`] escapes it.
fn token_fault(token: &str) -> Option<&'static str> {
    if token.is_empty() {
        Some("empty or whitespace-only")
    } else if token.chars().count() < MIN_TOKEN_LEN {
        Some("shorter than the 32-character minimum")
    } else if token.chars().any(char::is_control) {
        Some("carrying a control character")
    } else {
        None
    }
}

/// Validate a request's `(appliance_id, token)` against the whitelist.
///
/// Every rejection returns the same [`ApiError::Unauthorized`] and — as far
/// as is practical — costs the same work: one whitelist load plus one
/// token-file read plus one constant-time compare. The unknown-id branch used
/// to return after the whitelist load alone, which is a far louder oracle
/// than any byte-compare timing: it let an unauthenticated caller enumerate
/// which appliance ids exist. [`decoy_probe`] pays the missing read.
async fn authenticate(st: &AppState, id: &str, token: &str) -> Result<TenantEntry, ApiError> {
    let tenants = st.tenants.load(&st.opts.tenants_file).await?;

    // Independent of the id, so this leaks nothing: no tenant may ever
    // authenticate with a blank token, whatever its token file says.
    let supplied = token.trim();
    if supplied.is_empty() {
        tracing::warn!(target: Action::Authenticate.target(), "rejected blank token for {id:?}");
        return Err(ApiError::Unauthorized);
    }

    let Some(entry) = tenants.get(id) else {
        decoy_probe(&tenants).await;
        tracing::warn!(target: Action::Authenticate.target(), "unknown appliance id {id:?}");
        return Err(ApiError::Unauthorized);
    };

    let expected = match tokio::fs::read_to_string(&entry.token_file).await {
        Ok(content) => content,
        Err(e) => {
            // Not a 500: a caller must not be able to tell "your token is
            // wrong" from "this tenant's secret is missing on the edge".
            tracing::error!(
                target: Action::Authenticate.target(),
                "tenant {id}: cannot read token file {}: {e}",
                entry.token_file,
            );
            return Err(ApiError::Unauthorized);
        }
    };
    let expected = expected.trim();
    if let Some(fault) = token_fault(expected) {
        tracing::error!(
            target: Action::Authenticate.target(),
            "tenant {id}: token file {} is {fault} — refusing all requests for this tenant until an operator fixes it",
            entry.token_file,
        );
        return Err(ApiError::Unauthorized);
    }
    if !ct_eq(supplied, expected) {
        tracing::warn!(target: Action::Authenticate.target(), "token mismatch for {id:?}");
        return Err(ApiError::Unauthorized);
    }
    Ok(entry.clone())
}

/// Do the token read + compare the known-id path does, and throw the answer
/// away, so an unknown id costs the same syscalls as a known one.
///
/// Any tenant's file will do — the point is the work, not the value.
/// `black_box` stops the optimiser from noticing the result is unused.
async fn decoy_probe(tenants: &HashMap<String, TenantEntry>) {
    let Some(entry) = tenants.values().next() else {
        return;
    };
    let expected = tokio::fs::read_to_string(&entry.token_file)
        .await
        .unwrap_or_default();
    let _ = std::hint::black_box(ct_eq(DECOY_TOKEN, expected.trim()));
}

/// Constant-time string compare, so a token check leaks no byte-position
/// information through timing. Delegates to `subtle`, which exists to stop
/// the optimiser turning a hand-rolled XOR-accumulate loop back into an
/// early-exit `memcmp`.
fn ct_eq(a: &str, b: &str) -> bool {
    bool::from(a.as_bytes().ct_eq(b.as_bytes()))
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

/// One reconciliation pass: prune, resolve the live registry against the
/// operator whitelist, and rewrite the two config files if anything changed.
///
/// Two properties this function is responsible for:
///
/// * **The whitelist is the hostname authority.** Each `TenantView` is built
///   from `entry.hostname` (`tenants.json`), never from the registry's stored
///   hostname, and a registry entry that disagrees is repaired. Otherwise an
///   operator hostname change sat inert until the appliance happened to
///   re-register, and any hostname that reached `registry.json` — a restored
///   backup, a hand-edit, a torn write — became a live router and an ACME
///   request for a name the operator never approved.
///
/// * **One bad tenant does not stop the pass.** A token file that cannot be
///   read (or that [`token_fault`] rejects) drops *that* tenant and logs;
///   it used to `?`-propagate, which aborted the pass before either file was
///   written, and since the next tick hit the same file it aborted identically
///   every `reconcile_interval` forever. One tenant's missing secret froze
///   registration, hostname changes and pruning for every tenant on the edge.
///   The bootstrap token stays fatal: it is not per-tenant, and rathole's
///   `[server]` block cannot be rendered without it.
async fn reconcile_once(st: &AppState) -> Result<()> {
    let pruned = st.reg.prune(st.opts.heartbeat_ttl).await;
    let views = st.reg.views().await;
    let tenants = st
        .tenants
        .load(&st.opts.tenants_file)
        .await
        .context("load tenants whitelist")?;
    let bootstrap = tokio::fs::read_to_string(&st.opts.bootstrap_token_file)
        .await
        .into_diagnostic()
        .context("read bootstrap token file")?;
    let bootstrap = bootstrap.trim();
    if let Some(fault) = token_fault(bootstrap) {
        return Err(miette!(
            "bootstrap token file {} is {fault}",
            st.opts.bootstrap_token_file
        ));
    }

    let mut enriched: Vec<TenantView> = Vec::with_capacity(views.len());
    let mut stale: Vec<String> = Vec::new();
    let mut rehome: Vec<(String, String)> = Vec::new();
    for v in &views {
        // Tenants removed from the whitelist are dropped here and pruned from
        // the registry so their rathole service + Traefik router disappear.
        let Some(entry) = tenants.get(&v.id) else {
            stale.push(v.id.clone());
            continue;
        };
        let hostname = entry.hostname.trim();
        if hostname.is_empty() {
            tracing::error!(
                target: Action::Reconcile.target(),
                "skipping tenant {}: whitelist hostname is empty",
                v.id,
            );
            continue;
        }
        if hostname != v.hostname {
            tracing::warn!(
                target: Action::Reconcile.target(),
                "tenant {}: registry hostname {:?} disagrees with the whitelist's {hostname:?}; the whitelist wins",
                v.id,
                v.hostname,
            );
            rehome.push((v.id.clone(), hostname.to_string()));
        }
        // The on-disk secret is the single source of truth for the token, so
        // it is read here rather than carried through the registry.
        let token = match tokio::fs::read_to_string(&entry.token_file).await {
            Ok(content) => content,
            Err(e) => {
                tracing::warn!(
                    target: Action::Reconcile.target(),
                    "skipping tenant {}: cannot read token file {}: {e}",
                    v.id,
                    entry.token_file,
                );
                continue;
            }
        };
        let token = token.trim();
        if let Some(fault) = token_fault(token) {
            tracing::error!(
                target: Action::Reconcile.target(),
                "skipping tenant {}: token file {} is {fault}",
                v.id,
                entry.token_file,
            );
            continue;
        }
        enriched.push(TenantView {
            id: v.id.clone(),
            hostname: hostname.to_string(),
            rathole_port: v.rathole_port,
            token: token.to_string(),
        });
    }
    for id in &stale {
        // best-effort; a failure just means it lingers until next prune
        if let Err(e) = st.reg.deregister(id).await {
            tracing::warn!(target: Action::Reconcile.target(), "dropping stale {id} failed: {e}");
        }
    }
    let mut repaired = false;
    for (id, hostname) in &rehome {
        match st.reg.rehost(id, hostname).await {
            Ok(changed) => repaired |= changed,
            Err(e) => {
                tracing::warn!(target: Action::Reconcile.target(), "rehosting {id} failed: {e}");
            }
        }
    }

    let opts = EdgeOpts {
        rathole_bind_addr: st.opts.rathole_bind_addr.clone(),
        rathole_bind_port: st.opts.rathole_bind_port,
        bootstrap_token: bootstrap.to_string(),
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
    if pruned || repaired || changed_traefik || changed_rathole {
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
