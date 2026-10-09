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
//! `/cluster/join` is the mesh half and reuses all of the above: the same
//! per-appliance token, the same [`authenticate`], the same body cap and
//! concurrency guard. It mints no second credential — the appliance's existing
//! `/var/secrets/losos-proxy-token` is what proves it may enrol — and it adds
//! exactly one authorisation bit, `losos.edge.tenants.<id>.cluster`, because a
//! mesh node runs a kubelet on the edge's cluster while a proxy tenant only
//! gets HTTP forwarded to it. Before handing back the node token it deletes the
//! caller's stale `Node` object and node-password `Secret`: `factory-reset` and
//! the reinstall ISO wipe `/persist`, the box regenerates
//! `/etc/rancher/node/password`, and rke2 then refuses the rejoin *permanently*
//! as a duplicate hostname — on an appliance with no shell to diagnose it from.
//!
//! Handler errors use the concrete [`ApiError`] enum (thiserror), mapped to
//! HTTP status codes at the boundary — `?` converts io/serde/registry via
//! `#[from]`. The reconciler orchestration (`reconcile_once`) returns
//! `miette::Result`: `ApiError`/`RegistryError` implement `miette::Diagnostic`
//! so `?` converts them, and io/serde errors are lifted via `.into_diagnostic()`
//! before `.context()`/`.with_context()` (miette's `Context` is only impl'd for
//! `Result<T, E: Diagnostic>`, not for plain `std::error::Error`). It logs and
//! continues rather than killing the server.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::{HeaderMap, StatusCode};
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
use crate::config::{
    desired_config_with_hosts, uplink_config, EdgeOpts, TenantView, UplinkService, UplinkTarget,
};
use crate::domains::{self, Doh, DomainError, Domains, DomainsView, Vouched};
use crate::error::ApiError;
use crate::hardware::{Catalogue, Line};
use crate::identity::Identity;
use crate::market::{
    AccountView, CheckoutView, HardwareCheckoutView, Market, MarketError, NewListing, NewOrder,
    OnboardView, Provision, PublicListing, Sharing,
};
use crate::opts::ServeOpts;
use crate::registry::{Registry, Shared};
use crate::relay::{self, Enrolment, RelayRefused, RelayReq, RelayResp, RelayRoute, UplinkFile};
use crate::routes::{self, Bindings, PassKey, PlanInput, RouteRow, TableFile};
use crate::stripe_gate::{GATE_TIMEOUT, MAX_GATE_CALLS_PER_REQUEST};
use crate::window::{self, valid_hhmm, valid_tz, ComputeWindow};
use crate::zone::{self, ZoneNames};

/// The zone file is public DNS data, read by the `knot` user.
const ZONE_FILE_MODE: u32 = 0o644;

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

/// Stripe events (`account.updated` especially) outgrow the 16 KiB every other
/// route is held to. Still bounded: the body is buffered before it is verified.
const MAX_WEBHOOK_BYTES: usize = 256 * 1024;

/// Wall-clock budget for one request, end to end. Without it a slow-loris
/// client holds a connection (and a concurrency permit) indefinitely.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Budget for the two market routes that wait on Stripe through the gate:
/// onboarding (up to [`MAX_GATE_CALLS_PER_REQUEST`] sequential round trips) and
/// ordering (two). Held to [`REQUEST_TIMEOUT`] they returned 504 on a slow but
/// healthy Stripe after the account or session had already been made, and no
/// link came back. Two seconds on top cover the store writes between calls.
/// Still bounded, and still under the same in-flight cap.
const STRIPE_ROUTE_TIMEOUT: Duration =
    Duration::from_secs(GATE_TIMEOUT.as_secs() * MAX_GATE_CALLS_PER_REQUEST as u64 + 2);

/// The routes [`STRIPE_ROUTE_TIMEOUT`] applies to.
const STRIPE_ROUTES: [&str; 3] = [
    "/market/seller/onboard",
    "/market/orders",
    "/market/hardware/checkout",
];

/// Budget for `POST /identity/cert`, which asks GitHub who the pusher is
/// before it looks at the list: one round trip to api.github.com, bounded
/// by [`GITHUB_TIMEOUT`], plus the file write.
const IDENTITY_PUSH_TIMEOUT: Duration = Duration::from_secs(GITHUB_TIMEOUT.as_secs() + 3);
const IDENTITY_PUSH_ROUTE: &str = "/identity/cert";
/// How long the edge waits on GitHub's `GET /user` for one push.
const GITHUB_TIMEOUT: Duration = Duration::from_secs(10);

/// Requests allowed in flight at once. Every authenticated *and*
/// unauthenticated request costs a `tenants.json` stat and (for a known id) a
/// token-file read, so an unbounded arrival rate is an unbounded IO rate on
/// the edge's root filesystem. Excess is shed with 503 rather than queued —
/// queueing under a flood just converts a CPU problem into a memory one.
const MAX_INFLIGHT: usize = 64;

/// A value no real token can equal, used to make the unknown-id branch do the
/// same compare the known-id branch does. See [`decoy_probe`].
const DECOY_TOKEN: &str = "\0decoy\0";

/// The compute-window file carries node names and clock times, no secrets, but
/// it lives inside the registrar's `StateDirectory` (0700 root) and only the
/// edge's `losos-mesh-taint.service` — also root — reads it. Owner-only costs
/// nothing here and keeps the narrow default.
const COMPUTE_WINDOWS_FILE_MODE: u32 = 0o600;

/// `relay-routes.json` names boxes, their local edges and their domains, the
/// same operator-only inventory `registry.json` holds.
const ROUTE_TABLE_FILE_MODE: u32 = 0o600;

/// Budget for one request to the mesh apiserver. Two of these (the `Node`
/// delete and the node-password `Secret` delete) plus a TLS handshake have to
/// fit inside [`REQUEST_TIMEOUT`], or the join comes back as the guard's 504
/// instead of this module's 503 — same retry for the client, but a far less
/// useful line in the journal. The apiserver is on loopback, so 2s is already
/// generous.
const KUBE_TIMEOUT: Duration = Duration::from_secs(2);

/// Longest node name the cleanup will build a URL from. Kubernetes object
/// names are DNS subdomains, capped at 253 characters.
const MAX_NODE_NAME_LEN: usize = 253;

/// An official edge's route table for boxes behind local edges.
struct RouteTable {
    /// `relay-routes.json`, rewritten when the table changes.
    file: PathBuf,
    key: PassKey,
    /// Which spoke vouched for which box, as `/relay` verified it.
    bindings: std::sync::Mutex<Bindings>,
    /// The table the last reconcile pass planned.
    last: std::sync::Mutex<BTreeMap<String, RouteRow>>,
}

#[derive(Clone)]
struct AppState {
    reg: Shared,
    opts: Arc<ServeOpts>,
    notify: Arc<Notify>,
    tenants: Arc<TenantCache>,
    limiter: Arc<Semaphore>,
    /// When [`guard`] last shed a request for want of a permit. While that is
    /// within one `heartbeat_ttl`, [`reconcile_once`] does not prune: see
    /// there.
    last_shed: Arc<std::sync::Mutex<Option<std::time::Instant>>>,
    /// `None` unless `--market-gate-socket` was given.
    market: Option<Arc<Market>>,
    /// This edge's identity key and, once pushed, its certificate
    /// (`crate::identity`); `None` (no `--identity-key-file`) makes every
    /// `/identity*` route a 404, i.e. an edge that cannot become official.
    identity: Option<Arc<Identity>>,
    /// One push at a time: the handler holds this across its GitHub round
    /// trip, so a flood of bogus pushes costs GitHub one request at a time
    /// rather than [`MAX_INFLIGHT`] of them.
    push_lock: Arc<Mutex<()>>,
    /// The client the push handler asks GitHub with.
    http: reqwest::Client,
    /// `None` unless `--dns-zone` was given (see `crate::domains`).
    domains: Option<Arc<Domains>>,
    /// The DNS-over-HTTPS client the domain checks look records up with.
    doh: Option<Doh>,
    /// Boxes this edge accepted on first contact (`--enrol-dir`,
    /// `losos.edge.lan.openEnrolment`); `None` keeps enrolment closed.
    enrolment: Option<Arc<Enrolment>>,
    /// The enrolled file, memoised like the whitelist.
    enrolled: Arc<TenantCache>,
    /// `None` unless `--relay-routes` was given (see `crate::routes`).
    routes: Option<Arc<RouteTable>>,
    /// The relay passes this edge's own boxes handed it, by box id, for the
    /// uplink to forward to the hub. Live state only: every box repeats its
    /// pass on each heartbeat.
    passes: Arc<std::sync::Mutex<HashMap<String, String>>>,
}

#[derive(Debug, Deserialize)]
struct RegisterReq {
    appliance_id: String,
    token: String,
    hostname: String,
    /// The box's relay pass from its official edge, for a spoke to forward
    /// (`crate::routes`). Optional: most boxes have none.
    #[serde(default)]
    relay_pass: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HeartbeatReq {
    appliance_id: String,
    token: String,
    /// Whether the appliance was idle when it sent this.
    ///
    /// Optional so an appliance that predates idle reporting still heartbeats;
    /// absent is recorded as BUSY, never as idle. The failure that costs an
    /// owner is lending their box out while they are using it, so every
    /// unknown resolves that way.
    #[serde(default)]
    idle: Option<bool>,
    /// As on [`RegisterReq`].
    #[serde(default)]
    relay_pass: Option<String>,
}

#[derive(Debug, Serialize)]
struct RegisterResp {
    rathole_port: u16,
}

/// A `/cluster/join` body. `share_compute` and the two window bounds default
/// rather than being required so an older appliance — one whose
/// `losos-mesh-join.service` predates the compute-window flags — still enrols,
/// with sharing off, which is the safe direction.
#[derive(Debug, Deserialize)]
struct ClusterJoinReq {
    appliance_id: String,
    token: String,
    node_name: String,
    #[serde(default)]
    share_compute: bool,
    #[serde(default = "default_window_start")]
    window_start: String,
    #[serde(default = "default_window_end")]
    window_end: String,
    /// The IANA zone the two bounds are wall-clock times in — the appliance's
    /// own time.timeZone. Defaulted rather than required so a client that
    /// predates the field still joins; see window.rs for why UTC is the right
    /// default rather than the edge's guess at the owner's zone.
    #[serde(default = "default_window_tz")]
    window_tz: String,
}

fn default_window_start() -> String {
    "23:00".to_string()
}

fn default_window_end() -> String {
    "07:00".to_string()
}

fn default_window_tz() -> String {
    "UTC".to_string()
}

/// What an accepted join gets back: where to register, the node token to
/// present, and the name the edge expects it under. `node_name` is echoed
/// rather than assumed so the appliance-side client logs what the edge agreed
/// to, not what it asked for.
#[derive(Debug, Serialize)]
struct ClusterJoinResp {
    server_addr: String,
    token: String,
    node_name: String,
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
    let app = build(opts).await?;
    let recon = tokio::spawn(reconciler(app.state.clone()));
    // The uplink is a loop of its own: it talks to another machine and must
    // never hold up the local reconcile.
    let uplink = app
        .state
        .opts
        .uplink
        .is_some()
        .then(|| tokio::spawn(uplink_loop(app.state.clone())));

    // axum::serve returns when the listener errors or `shutdown` resolves.
    axum::serve(listener, app.router)
        .with_graceful_shutdown(shutdown)
        .await
        .into_diagnostic()?;
    tracing::info!(target: Action::Serve.target(), "http server stopped");

    // Structured shutdown: stop the reconciler and await its result so a
    // panic inside it surfaces instead of being silently detached.
    recon.abort();
    let _ = recon.await;
    if let Some(uplink) = uplink {
        uplink.abort();
        let _ = uplink.await;
    }
    Ok(())
}

/// A built registrar that nothing is serving yet: the router, and the handles
/// a host needs to drive the reconciler itself.
///
/// [`serve`] is the self-hosted shape — one long-lived process that owns a
/// listener and ticks the reconciler on a timer. A host without either (a
/// serverless function, which is handed each request by its platform and may
/// not outlive it) builds one of these instead and calls [`App::reconcile`]
/// when it sees fit. The router is the same router with the same auth, body
/// cap, timeout and concurrency guard; nothing about the request surface is
/// decided by who serves it.
pub struct App {
    state: AppState,
    router: Router,
}

impl App {
    /// The registrar's router: every route [`serve`] exposes, nothing more.
    pub fn router(&self) -> Router {
        self.router.clone()
    }

    /// The live registry, for a host that persists it somewhere other than
    /// `--registry` (see [`Registry::export`]).
    #[must_use]
    pub fn registry(&self) -> &Shared {
        &self.state.reg
    }

    /// The options this app was built with.
    #[must_use]
    pub fn opts(&self) -> &ServeOpts {
        &self.state.opts
    }

    /// One reconciler pass, as the timer would run it: prune, rewrite the
    /// Traefik and rathole files if anything changed, publish the compute
    /// windows, then fulfil paid market orders.
    pub async fn reconcile(&self) -> Result<()> {
        check_domains(&self.state).await;
        reconcile_once(&self.state).await?;
        fulfil_market(&self.state).await;
        Ok(())
    }
}

/// Load the registry, open the market store if configured, run the first
/// reconcile pass and build the router — everything [`serve`] does before it
/// starts accepting, with no socket and no background task.
pub async fn build(opts: ServeOpts) -> Result<App> {
    let reg = Arc::new(Registry::new(opts.registry_path.clone(), opts.port_range));
    // Re-attach before the API opens: restore routes for tenants the edge
    // already knew about so a rebooting edge doesn't drop every appliance.
    reg.load().await.context("load registry")?;
    tracing::info!(target: Action::LoadRegistry.target(), "registry loaded");

    // Opened before the API accepts: an unreadable `market.json` stops the
    // edge from starting rather than serving a market that has forgotten who
    // paid for what.
    let market = match &opts.market {
        Some(m) => Some(Arc::new(
            Market::open((**m).clone())
                .await
                .map_err(|e| miette!("open market store: {e}"))?,
        )),
        None => None,
    };

    // Opened before the API accepts: the key is made here on the first start
    // if the file is missing (the private half never travels), and the
    // certificate, if one has been pushed, is checked to be for that key.
    let identity = match (&opts.identity_key_file, &opts.identity_cert_file) {
        (Some(key), Some(cert)) => Some(Arc::new(
            Identity::open(Path::new(key), Path::new(cert)).wrap_err("open the edge identity")?,
        )),
        (None, None) => None,
        _ => {
            return Err(miette!(
                "--identity-key-file and --identity-cert-file go together; one was given without the other"
            ))
        }
    };

    // Opened before the API accepts, like the market: an unreadable
    // `domains.json` stops the edge rather than dropping every live domain.
    let (domains, doh) = match &opts.domains {
        Some(d) => (
            Some(Arc::new(
                Domains::open((**d).clone())
                    .await
                    .map_err(|e| miette!("open domains store: {e}"))?,
            )),
            Some(
                Doh::new(&d.doh_url)
                    .into_diagnostic()
                    .wrap_err("build the DNS-over-HTTPS client")?,
            ),
        ),
        None => (None, None),
    };
    let enrolment = opts
        .enrol_dir
        .as_deref()
        .map(|dir| Arc::new(Enrolment::new(dir)));
    let routes = match &opts.routes {
        None => None,
        Some(_) if domains.is_none() => {
            return Err(miette!(
                "--relay-routes needs --dns-zone: only an edge that routes custom domains keeps the route table"
            ))
        }
        Some(r) => {
            let file = PathBuf::from(&r.table_file);
            // A table file that does not parse costs the routes until each
            // spoke's next /relay, not the edge: start empty and say so.
            let bindings = TableFile::load_bindings(&file).await.unwrap_or_else(|e| {
                tracing::warn!(
                    target: Action::Relay.target(),
                    "route table {}: {e}; starting empty, spokes refill it on their next /relay",
                    file.display(),
                );
                Bindings::default()
            });
            Some(Arc::new(RouteTable {
                file,
                key: PassKey::load_or_create(Path::new(&r.pass_key_file))
                    .await
                    .into_diagnostic()
                    .with_context(|| format!("relay pass key {}", r.pass_key_file))?,
                bindings: std::sync::Mutex::new(bindings),
                last: std::sync::Mutex::new(BTreeMap::new()),
            }))
        }
    };

    let state = AppState {
        reg,
        opts: Arc::new(opts),
        notify: Arc::new(Notify::new()),
        tenants: Arc::new(TenantCache::default()),
        limiter: Arc::new(Semaphore::new(MAX_INFLIGHT)),
        last_shed: Arc::new(std::sync::Mutex::new(None)),
        market,
        identity,
        push_lock: Arc::new(Mutex::new(())),
        http: reqwest::Client::builder()
            .timeout(GITHUB_TIMEOUT)
            .user_agent(concat!("losos-registrar/", env!("CARGO_PKG_VERSION")))
            .build()
            .into_diagnostic()
            .wrap_err("build the HTTP client")?,
        domains,
        doh,
        enrolment,
        enrolled: Arc::new(TenantCache::default()),
        routes,
        passes: Arc::new(std::sync::Mutex::new(HashMap::new())),
    };

    // Generate config from whatever we just loaded, so the box is serving
    // before any heartbeat arrives.
    reconcile_once(&state).await?;

    let router = Router::new()
        .route("/health", get(health))
        .route("/noise-public-key", get(noise_public_key))
        .route("/identity", get(identity_route))
        // The identity's other half: the public key an operator signs, and
        // the push that installs the signed certificate. The push is the one
        // route authenticated by a GitHub account rather than an appliance
        // token: see `identity_push`.
        .route("/identity/public-key", get(identity_public_key))
        .route(IDENTITY_PUSH_ROUTE, post(identity_push))
        .route("/register", post(register))
        .route("/heartbeat", post(heartbeat))
        .route("/deregister", post(deregister))
        // Same operation under a clearer name; the announce client never
        // calls either, so there is no wire-compat constraint to honour.
        .route("/unregister", post(deregister))
        // Inside the body cap, the timeout and the concurrency semaphore, like
        // every other route: Traefik's `register.<domain>` router has no path
        // rule, so this one is on the public internet too.
        .route("/cluster/join", post(cluster_join))
        // Federation (crate::relay): a spoke lists the boxes it serves.
        .route("/relay", post(relay_route))
        // The optional Stripe Connect market. Every route answers 503 unless
        // the edge was started with `--market-gate-socket`. Browsing is
        // anonymous and shows no seller identity; everything else takes the
        // appliance token like the routes above; the webhook is authenticated
        // by Stripe's signature instead and needs a larger body than the rest.
        .route("/market/listings", get(market_browse).post(market_list))
        .route("/market/listings/close", post(market_close))
        .route("/market/seller/onboard", post(market_onboard))
        .route("/market/orders", post(market_order))
        .route("/market/account", post(market_account))
        .route("/market/hardware", get(market_hardware))
        .route("/market/hardware/checkout", post(market_hardware_checkout))
        .route(
            "/market/webhook",
            post(market_webhook).layer(DefaultBodyLimit::max(MAX_WEBHOOK_BYTES)),
        )
        // Custom domains (crate::domains). 503 unless the edge serves a zone
        // and holds an installed identity certificate; appliance token like
        // the market. None of them looks anything up in DNS: an add kicks
        // the reconciler, which does, outside the request's 5 s budget.
        .route("/domains/list", post(domains_list))
        .route("/domains/add", post(domains_add))
        .route("/domains/remove", post(domains_remove))
        // Bound at the router so an oversized body never materialises.
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        // Added last, so outermost: the timeout and the concurrency cap cover
        // body reading, routing and 404s, not just handler bodies.
        .layer(middleware::from_fn_with_state(state.clone(), guard))
        .with_state(state.clone());

    Ok(App { state, router })
}

/// Resolve on SIGTERM (systemd's stop signal) or SIGINT.
pub(crate) async fn shutdown_signal() {
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
/// runs inside by [`REQUEST_TIMEOUT`], or [`STRIPE_ROUTE_TIMEOUT`] on the
/// routes that wait on Stripe.
async fn guard(State(st): State<AppState>, req: Request, next: Next) -> Response {
    let Ok(_permit) = Arc::clone(&st.limiter).try_acquire_owned() else {
        tracing::warn!(target: Action::Serve.target(), "shedding request: {MAX_INFLIGHT} in flight");
        if let Ok(mut last) = st.last_shed.lock() {
            *last = Some(std::time::Instant::now());
        }
        return (StatusCode::SERVICE_UNAVAILABLE, "busy; retry later").into_response();
    };
    let budget = if STRIPE_ROUTES.contains(&req.uri().path()) {
        STRIPE_ROUTE_TIMEOUT
    } else if req.uri().path() == IDENTITY_PUSH_ROUTE {
        IDENTITY_PUSH_TIMEOUT
    } else {
        REQUEST_TIMEOUT
    };
    match tokio::time::timeout(budget, next.run(req)).await {
        Ok(response) => response,
        Err(_) => {
            tracing::warn!(target: Action::Serve.target(), "request exceeded {budget:?}");
            (StatusCode::GATEWAY_TIMEOUT, "request timed out").into_response()
        }
    }
}

async fn health() -> &'static str {
    "ok"
}

/// The tunnel's Noise public key, for appliances to pin on first start.
///
/// Unauthenticated on purpose: it is a public key, and the appliance fetches it
/// before it holds any credential, over the TLS the edge is fronted with.
/// 404 when Noise is off or the key has not been generated yet.
async fn noise_public_key(State(st): State<AppState>) -> Response {
    let Some(path) = &st.opts.noise_public_key_file else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match tokio::fs::read_to_string(path).await {
        Ok(key) => key.trim().to_string().into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// `GET /identity?nonce=<hex>` — who this edge is, and proof it holds the
/// certified key (`crate::identity`). Unauthenticated: a box asks before it
/// trusts anything, and nothing here is secret. 404 for an edge without an
/// identity key, or with a key but no certificate yet (a company edge, or
/// one whose certificate has not been pushed), 400 for a nonce the edge
/// will not sign.
async fn identity_route(
    State(st): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<IdentityQuery>,
) -> Response {
    let Some(id) = &st.identity else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !crate::identity::nonce_ok(&q.nonce) {
        return (
            StatusCode::BAD_REQUEST,
            "nonce must be 32 to 128 hex characters",
        )
            .into_response();
    }
    match id.answer(&q.nonce) {
        Some(answer) => Json(answer).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(Deserialize)]
struct IdentityQuery {
    #[serde(default)]
    nonce: String,
}

/// `GET /identity/public-key` — this edge's Ed25519 public key, 64 hex
/// characters, the thing the root signs. Unauthenticated, like
/// `/noise-public-key`: a public key is public, and `provision edge` reads
/// it before it can issue anything. 404 for an edge with no identity key.
async fn identity_public_key(State(st): State<AppState>) -> Response {
    match &st.identity {
        Some(id) => id.public_key().to_string().into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// `POST /identity/cert` — install the certificate the root signed for this
/// edge's key. The body is the certificate JSON (`crate::identity::Cert`);
/// the `Authorization: Bearer` header is a **GitHub token**, not an
/// appliance token: the edge asks GitHub whose it is and admits the push
/// only if that account's numeric id is on the operator allowlist compiled
/// into this binary (`operators.json`, the same list `provision` checks on
/// the operator's machine). The checks run cheapest first, so a flood of
/// unauthenticated or mis-addressed pushes never reaches GitHub: the body
/// must parse and name *this* key before the token is looked at, and one
/// push holds [`AppState::push_lock`] across the round trip.
///
/// Answers: 200 with `{installed, name, url, not_after, operator}`; 400 a
/// body that is not a certificate for this key; 401 no token, or one GitHub
/// does not accept; 403 an account that is not listed; 404 no identity
/// key on this edge; 502 GitHub unreachable; 507 the file could not be
/// written. The root's signature is not checked here (the edge need not
/// hold the root public key): the boxes check it, and `provision edge`
/// probes `/identity` right after the push and says so.
async fn identity_push(State(st): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(id) = &st.identity else {
        return (
            StatusCode::NOT_FOUND,
            "this edge has no identity key (losos.edge.identity.keyFile)",
        )
            .into_response();
    };
    let cert: crate::identity::Cert = match serde_json::from_slice(&body) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("the body is not a certificate: {e}"),
            )
                .into_response()
        }
    };
    if cert.public_key != id.public_key() {
        return (
            StatusCode::BAD_REQUEST,
            crate::identity::Pushed::OtherKey.to_string(),
        )
            .into_response();
    }
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|t| !t.is_empty() && t.len() <= 512 && t.bytes().all(|b| b.is_ascii_graphic()));
    let Some(token) = token else {
        return (
            StatusCode::UNAUTHORIZED,
            "Authorization: Bearer <GitHub token> is required",
        )
            .into_response();
    };

    let _one_at_a_time = st.push_lock.lock().await;
    let operators = match crate::provision::Operators::committed() {
        Ok(o) => o,
        Err(e) => {
            tracing::error!(target: Action::Identity.target(), "operators.json: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let gh = crate::provision::Github {
        oauth_url: String::new(),
        api_url: st.opts.github_api_url.clone(),
        client_id: String::new(),
    };
    let user = match crate::provision::whoami(&st.http, &gh, token).await {
        Ok(u) => u,
        Err(e) => {
            // `whoami` wraps GitHub's refusal of the token in its own words;
            // tell a bad token from an unreachable GitHub by the cause.
            let text = format!("{e:?}");
            let status = if text.contains("did not accept the token") {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::BAD_GATEWAY
            };
            tracing::warn!(target: Action::Identity.target(), "push refused: {e}");
            return (status, format!("GitHub: {e}")).into_response();
        }
    };
    let Some(operator) = operators.find(user.id) else {
        tracing::warn!(
            target: Action::Identity.target(),
            "push refused: {} (GitHub id {}) is not on the operator allowlist",
            user.login, user.id
        );
        return (
            StatusCode::FORBIDDEN,
            format!(
                "{} (GitHub id {}) is not on the operator allowlist",
                user.login, user.id
            ),
        )
            .into_response();
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    match id.install(cert.clone(), now) {
        Ok(()) => {
            tracing::info!(
                target: Action::Identity.target(),
                "certificate installed by {} (GitHub id {}, listed as {}): {} at {}, until {}",
                user.login, user.id, operator.github_login, cert.name, cert.url, cert.not_after
            );
            Json(serde_json::json!({
                "installed": true,
                "name": cert.name,
                "url": cert.url,
                "not_after": cert.not_after,
                "operator": user.login,
            }))
            .into_response()
        }
        Err(e @ crate::identity::Pushed::Write(_)) => {
            tracing::error!(target: Action::Identity.target(), "push by {}: {e}", user.login);
            (StatusCode::INSUFFICIENT_STORAGE, e.to_string()).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn register(
    State(st): State<AppState>,
    Json(req): Json<RegisterReq>,
) -> Result<Json<RegisterResp>, ApiError> {
    enrol_on_first_contact(&st, &req).await?;
    let tenant = authenticate(&st, &req.appliance_id, &req.token).await?;
    // The registered hostname must match the whitelisted one — a tenant can't
    // claim an arbitrary public hostname, only its own. A box this edge
    // enrolled itself has no operator row; it proved its token just now, so
    // its new name is recorded instead.
    if req.hostname != tenant.hostname {
        let enrolled_here = match &st.enrolment {
            Some(e) if e.owns(Path::new(&tenant.token_file)) => {
                relay::dns_name(&req.hostname) && e.rehost(&req.appliance_id, &req.hostname).await?
            }
            _ => false,
        };
        if !enrolled_here {
            return Err(ApiError::HostnameForbidden);
        }
    }
    let port = st.reg.register(&req.appliance_id, &req.hostname).await?;
    remember_pass(&st, &req.appliance_id, req.relay_pass.as_deref());
    tracing::info!(
        target: Action::Register.target(),
        "registered {} -> {} on port {port}",
        req.appliance_id,
        tenant.hostname,
    );
    st.notify.notify_one();
    Ok(Json(RegisterResp { rathole_port: port }))
}

/// Open enrolment (`losos.edge.lan.openEnrolment`): a `/register` for an id
/// nobody listed, on an edge that enrols on first contact, creates the
/// tenant with the token it presented. Validated like a whitelist row would
/// be — a DNS-label id, a DNS hostname, a token that [`token_fault`] would
/// accept — and never more than proxy membership. A known id, whitelisted or
/// already enrolled, falls through to the ordinary check, so a second box
/// claiming an enrolled id is simply unauthorized.
async fn enrol_on_first_contact(st: &AppState, req: &RegisterReq) -> Result<(), ApiError> {
    let Some(enrolment) = &st.enrolment else {
        return Ok(());
    };
    let tenants = st.tenants.load(&st.opts.tenants_file).await?;
    if lookup_tenant(st, &tenants, &req.appliance_id)
        .await?
        .is_some()
    {
        return Ok(());
    }
    let token = req.token.trim();
    if !relay::dns_label(&req.appliance_id) || !relay::dns_name(&req.hostname) {
        tracing::warn!(
            target: Action::Enrol.target(),
            "refusing to enrol {:?} as {:?}: not a DNS label and a DNS name",
            req.appliance_id,
            req.hostname,
        );
        return Err(ApiError::Unauthorized);
    }
    if let Some(fault) = token_fault(token) {
        tracing::warn!(
            target: Action::Enrol.target(),
            "refusing to enrol {:?}: its token is {fault}",
            req.appliance_id,
        );
        return Err(ApiError::Unauthorized);
    }
    enrolment
        .enrol(&req.appliance_id, &req.hostname, token)
        .await?;
    Ok(())
}

/// Keep (or forget) the relay pass a box of this edge just sent, for the
/// uplink. Only a pass-shaped string is kept, so a heartbeat cannot park
/// anything else in memory; this edge cannot check the pass itself, only the
/// hub that issued it can.
fn remember_pass(st: &AppState, id: &str, pass: Option<&str>) {
    let mut passes = st
        .passes
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match pass.map(str::trim).filter(|p| routes::well_formed_pass(p)) {
        Some(p) => {
            passes.insert(id.to_string(), p.to_string());
        }
        None => {
            passes.remove(id);
        }
    }
}

async fn heartbeat(
    State(st): State<AppState>,
    Json(req): Json<HeartbeatReq>,
) -> Result<StatusCode, ApiError> {
    authenticate(&st, &req.appliance_id, &req.token).await?;
    // Recorded before the liveness check so the two cannot disagree about
    // which heartbeat this was.
    st.reg
        .record_idle(&req.appliance_id, req.idle.unwrap_or(false))
        .await;
    if st.reg.heartbeat(&req.appliance_id).await {
        remember_pass(&st, &req.appliance_id, req.relay_pass.as_deref());
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

/// Enrol this appliance in the edge's mesh rke2 cluster.
///
/// Four gates, in order, then a cleanup, then the token — and the order is the
/// point of the route:
///
/// 1. [`authenticate`]: the existing closed-enrollment path, unmodified.
/// 2. `cluster`: proxy membership does not imply mesh membership.
/// 3. `node_name == appliance_id`: this is what makes the delete below safe.
///    The only node object a caller can ever remove is the one named after
///    itself, so no tenant can evict a neighbour from the cluster.
/// 4. the window bounds are `HH:MM`. A browser is not a trust boundary and
///    neither is lososd: this body arrives from anything holding the token.
///
/// The cleanup runs *before* the token is read, and a cleanup failure returns
/// early. Handing out a node token after a failed cleanup reproduces exactly
/// the permanent lockout this route exists to prevent: the agent starts, rke2
/// sees a node of that name it already has a different password for, and
/// rejects it for good.
async fn cluster_join(
    State(st): State<AppState>,
    Json(req): Json<ClusterJoinReq>,
) -> Result<Json<ClusterJoinResp>, ApiError> {
    let tenant = authenticate(&st, &req.appliance_id, &req.token).await?;
    if !tenant.cluster {
        return Err(ApiError::ClusterForbidden);
    }
    if req.node_name != req.appliance_id {
        return Err(ApiError::NodeNameForbidden);
    }
    if !valid_hhmm(&req.window_start) || !valid_hhmm(&req.window_end) {
        return Err(ApiError::InvalidWindow);
    }
    // A zone the edge's `date` cannot resolve is silently treated as UTC, so an
    // unvalidated value reintroduces the exact drift this field exists to fix —
    // only one layer further down, where nothing logs it.
    if !valid_tz(&req.window_tz) {
        return Err(ApiError::InvalidWindow);
    }

    // Presence is checked here so an edge with `losos.edge.cluster.enable =
    // false` answers 503 before touching the apiserver or the filesystem; the
    // token itself is read after the cleanup.
    let (Some(agent_token_file), Some(server_addr)) =
        (&st.opts.mesh_agent_token_file, &st.opts.mesh_server_addr)
    else {
        return Err(ApiError::MeshUnconfigured);
    };

    cleanup_stale_node(&st, &req.node_name).await?;

    let agent_token = match tokio::fs::read_to_string(agent_token_file).await {
        Ok(content) => content,
        Err(e) => {
            tracing::error!(
                target: Action::Join.target(),
                "cannot read mesh agent token file {agent_token_file}: {e}",
            );
            return Err(ApiError::MeshUnconfigured);
        }
    };
    let agent_token = agent_token.trim();
    // The same operator-fault check tenant tokens get: a truncated or empty
    // file would otherwise be written to `losos.cluster.tokenFile` on the
    // appliance, where rke2 fails with a TLS error that names nothing.
    if let Some(fault) = token_fault(agent_token) {
        tracing::error!(
            target: Action::Join.target(),
            "mesh agent token file {agent_token_file} is {fault} — refusing every join until an operator fixes it",
        );
        return Err(ApiError::MeshUnconfigured);
    }

    let window = ComputeWindow {
        share_compute: req.share_compute,
        window_start: req.window_start,
        window_end: req.window_end,
        tz: req.window_tz,
    };
    if st.reg.set_compute_window(&req.node_name, window).await? {
        // Kick the reconciler so the taint timer sees the new window on its
        // next five-minute tick rather than up to `reconcile_interval` later.
        st.notify.notify_one();
    }

    tracing::info!(
        target: Action::Join.target(),
        "enrolled {} as mesh node {} (share_compute={})",
        req.appliance_id,
        req.node_name,
        req.share_compute,
    );
    Ok(Json(ClusterJoinResp {
        server_addr: server_addr.clone(),
        token: agent_token.to_string(),
        node_name: req.node_name,
    }))
}

/// `POST /relay`: a spoke replaces the list of boxes it relays through this
/// hub. Authenticated as the spoke's own tenant, which must carry a
/// `relay_zone`; every listed box is then checked against that zone
/// ([`relay::refusal`]) and the accepted ones become `<spoke>.<box>` tenants
/// ([`Registry::relay`]). Refusals are per box and come back in the body, so
/// the spoke can log which of its boxes is misnamed while the rest route.
async fn relay_route(
    State(st): State<AppState>,
    Json(req): Json<RelayReq>,
) -> Result<Json<RelayResp>, ApiError> {
    let spoke = authenticate(&st, &req.appliance_id, &req.token).await?;
    let Some(zone) = spoke
        .relay_zone
        .as_deref()
        .map(str::trim)
        .filter(|z| !z.is_empty())
    else {
        return Err(ApiError::RelayForbidden);
    };
    if !relay::dns_label(&req.appliance_id) {
        // The spoke's id is half of every relayed key; a dot in it would make
        // `<spoke>.<box>` ambiguous. An operator fault, loud in the log.
        tracing::error!(
            target: Action::Relay.target(),
            "tenant {:?} has a relay zone but its id is not a DNS label; refusing to relay for it",
            req.appliance_id,
        );
        return Err(ApiError::RelayForbidden);
    }
    if req.tenants.len() > relay::MAX_RELAYED {
        tracing::warn!(
            target: Action::Relay.target(),
            "spoke {} listed {} boxes, more than the {} one spoke may relay",
            req.appliance_id,
            req.tenants.len(),
            relay::MAX_RELAYED,
        );
        return Err(ApiError::RelayTooMany);
    }
    let tenants = st.tenants.load(&st.opts.tenants_file).await?;
    let mut accepted: Vec<(String, String)> = Vec::new();
    let mut refused: Vec<RelayRefused> = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    for t in &req.tenants {
        if !seen.insert(t.id.as_str()) {
            refused.push(RelayRefused {
                id: t.id.clone(),
                reason: "listed twice".to_string(),
            });
            continue;
        }
        let key = format!("{}.{}", req.appliance_id, t.id);
        match relay::refusal(t, zone, tenants.contains_key(&key)) {
            Some(reason) => refused.push(RelayRefused {
                id: t.id.clone(),
                reason: reason.to_string(),
            }),
            None => accepted.push((t.id.clone(), t.hostname.clone())),
        }
    }
    for r in &refused {
        tracing::warn!(
            target: Action::Relay.target(),
            "spoke {}: not relaying {}: {}",
            req.appliance_id,
            r.id,
            r.reason,
        );
    }
    let relayed = st.reg.relay(&req.appliance_id, &accepted).await?;
    let routes = match &st.routes {
        None => Vec::new(),
        Some(rt) => {
            let now = crate::market::now_secs();
            let accepted_ids: HashSet<&str> = accepted.iter().map(|(id, _)| id.as_str()).collect();
            let vouched: HashMap<String, u64> = req
                .tenants
                .iter()
                .filter(|t| accepted_ids.contains(t.id.as_str()))
                .filter_map(|t| {
                    let pass = t.pass.as_deref()?;
                    match rt.key.verify(&t.id, pass, now) {
                        Some(epoch) => Some((t.id.clone(), epoch)),
                        None => {
                            tracing::info!(
                                target: Action::Relay.target(),
                                "spoke {}: the relay pass for {} is not good here (expired, or not this edge's)",
                                req.appliance_id,
                                t.id,
                            );
                            None
                        }
                    }
                })
                .collect();
            rt.bindings
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .record(&req.appliance_id, &vouched);
            rt.last
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .values()
                .filter(|row| row.spoke == req.appliance_id)
                .map(|row| RelayRoute {
                    id: row.tenant.clone(),
                    domain: row.domain.clone(),
                })
                .collect()
        }
    };
    tracing::debug!(
        target: Action::Relay.target(),
        "spoke {} relays {} box(es), {} refused",
        req.appliance_id,
        relayed.len(),
        refused.len(),
    );
    st.notify.notify_one();
    Ok(Json(RelayResp {
        accepted: relayed,
        refused,
        routes,
    }))
}

/// Body of every authenticated market route; the route-specific fields sit
/// beside the credentials.
#[derive(Debug, Deserialize)]
struct MarketAuth {
    appliance_id: String,
    token: String,
}

#[derive(Debug, Deserialize)]
struct MarketListReq {
    #[serde(flatten)]
    auth: MarketAuth,
    #[serde(flatten)]
    listing: NewListing,
}

#[derive(Debug, Deserialize)]
struct MarketCloseReq {
    #[serde(flatten)]
    auth: MarketAuth,
    listing_id: String,
}

#[derive(Debug, Deserialize)]
struct MarketOrderReq {
    #[serde(flatten)]
    auth: MarketAuth,
    #[serde(flatten)]
    order: NewOrder,
}

#[derive(Debug, Serialize)]
struct ListingCreated {
    listing_id: String,
}

async fn market_of(st: &AppState) -> Result<&Arc<Market>, ApiError> {
    let market = st
        .market
        .as_ref()
        .ok_or(ApiError::Market(MarketError::Unconfigured))?;
    market.validate_secrets().await?;
    Ok(market)
}

/// Authenticate, then require both a configured market and the tenant's
/// `market` bit. Authentication comes first so an unauthenticated caller
/// learns nothing about whether the market exists for anyone else.
async fn market_tenant(st: &AppState, auth: &MarketAuth) -> Result<Arc<Market>, ApiError> {
    let tenant = authenticate(st, &auth.appliance_id, &auth.token).await?;
    let market = market_of(st).await?;
    if !tenant.market {
        return Err(ApiError::Market(MarketError::Forbidden));
    }
    Ok(Arc::clone(market))
}

/// What each appliance may sell right now: what it already contributes to the
/// mesh, and only while the operator still has its tenant's `market` bit on.
/// Re-read on every call, so revoking a seller's opt-in pulls their listings
/// from the shelf and refuses new orders for them without a restart; the
/// seller's own routes are refused separately by [`market_tenant`].
async fn sharing_of(st: &AppState) -> Result<Sharing, ApiError> {
    let tenants = st.tenants.load(&st.opts.tenants_file).await?;
    Ok(Sharing::from_windows(&st.reg.compute_windows().await)
        .only_sellers(|seller| tenants.get(seller).is_some_and(|t| t.market)))
}

async fn market_browse(State(st): State<AppState>) -> Result<Json<Vec<PublicListing>>, ApiError> {
    let market = market_of(&st).await?;
    let sharing = sharing_of(&st).await?;
    Ok(Json(market.browse(&sharing).await?))
}

async fn market_account(
    State(st): State<AppState>,
    Json(req): Json<MarketAuth>,
) -> Result<Json<AccountView>, ApiError> {
    let market = market_tenant(&st, &req).await?;
    let sharing = sharing_of(&st).await?;
    Ok(Json(market.account(&req.appliance_id, &sharing).await?))
}

/// `box_uuid` is the box's own UUID, written onto its Stripe account. Optional
/// so a box that predates it still onboards.
#[derive(Debug, Deserialize)]
struct MarketOnboardReq {
    #[serde(flatten)]
    auth: MarketAuth,
    #[serde(default)]
    box_uuid: Option<String>,
}

async fn market_onboard(
    State(st): State<AppState>,
    Json(req): Json<MarketOnboardReq>,
) -> Result<Json<OnboardView>, ApiError> {
    let market = market_tenant(&st, &req.auth).await?;
    Ok(Json(
        market
            .onboard(&req.auth.appliance_id, req.box_uuid.as_deref())
            .await?,
    ))
}

async fn market_list(
    State(st): State<AppState>,
    Json(req): Json<MarketListReq>,
) -> Result<(StatusCode, Json<ListingCreated>), ApiError> {
    let market = market_tenant(&st, &req.auth).await?;
    let sharing = sharing_of(&st).await?;
    let listing_id = market
        .create_listing(&req.auth.appliance_id, req.listing, &sharing)
        .await?;
    Ok((StatusCode::CREATED, Json(ListingCreated { listing_id })))
}

async fn market_close(
    State(st): State<AppState>,
    Json(req): Json<MarketCloseReq>,
) -> Result<StatusCode, ApiError> {
    let market = market_tenant(&st, &req.auth).await?;
    market
        .close_listing(&req.auth.appliance_id, &req.listing_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn market_order(
    State(st): State<AppState>,
    Json(req): Json<MarketOrderReq>,
) -> Result<(StatusCode, Json<CheckoutView>), ApiError> {
    let market = market_tenant(&st, &req.auth).await?;
    let sharing = sharing_of(&st).await?;
    let checkout = market
        .create_order(&req.auth.appliance_id, req.order, &sharing)
        .await?;
    Ok((StatusCode::CREATED, Json(checkout)))
}

/// `GET /market/hardware`: what the operator sells, anonymous like the shelf.
/// 503 unless the edge runs the market and was given a catalogue.
async fn market_hardware(State(st): State<AppState>) -> Result<Json<Catalogue>, ApiError> {
    let market = market_of(&st).await?;
    Ok(Json(market.hardware()?))
}

#[derive(Debug, Deserialize)]
struct HardwareCheckoutReq {
    #[serde(flatten)]
    auth: MarketAuth,
    items: Vec<Line>,
}

/// `POST /market/hardware/checkout`: a Stripe Checkout for boxes and
/// gateways. Any tenant may buy; the `market` bit is about trading with
/// other tenants, and this is a sale by the operator.
async fn market_hardware_checkout(
    State(st): State<AppState>,
    Json(req): Json<HardwareCheckoutReq>,
) -> Result<(StatusCode, Json<HardwareCheckoutView>), ApiError> {
    authenticate(&st, &req.auth.appliance_id, &req.auth.token).await?;
    let market = market_of(&st).await?;
    let checkout = market
        .hardware_checkout(&req.auth.appliance_id, req.items)
        .await?;
    Ok((StatusCode::CREATED, Json(checkout)))
}

/// Body of the domain routes that name one domain.
#[derive(Debug, Deserialize)]
struct DomainReq {
    #[serde(flatten)]
    auth: MarketAuth,
    domain: String,
}

/// The domain store, but only on an edge that may hand out names: one that
/// serves a zone and holds an unexpired identity certificate an operator
/// pushed (`POST /identity/cert`, gated by the GitHub allowlist). The root
/// signature is checked by the boxes, which also refuse to relay domain
/// requests to an edge that is not official; this is the edge's own half.
fn domains_of(st: &AppState) -> Result<&Arc<Domains>, ApiError> {
    let domains = st
        .domains
        .as_ref()
        .ok_or(ApiError::Domains(DomainError::Unconfigured))?;
    let official = st
        .identity
        .as_ref()
        .and_then(|i| i.cert())
        .is_some_and(|c| c.not_after > crate::market::now_secs());
    if !official {
        return Err(ApiError::Domains(DomainError::Unconfigured));
    }
    Ok(domains)
}

/// The tenant's Stripe data as the domain module reads it: `None` on an edge
/// with no market, for a box with no account, and for one Stripe has not
/// finished checking.
async fn vouched_for(st: &AppState, tenant: &str) -> Option<Vouched> {
    let market = st.market.as_ref()?;
    Vouched::from_seller(market.seller(tenant).await.as_ref())
}

/// Authenticate first, so a caller without a token learns nothing about
/// whether this edge offers domains.
async fn domains_tenant(
    st: &AppState,
    auth: &MarketAuth,
) -> Result<(Arc<Domains>, Option<Vouched>), ApiError> {
    authenticate(st, &auth.appliance_id, &auth.token).await?;
    let domains = Arc::clone(domains_of(st)?);
    let vouched = vouched_for(st, &auth.appliance_id).await;
    Ok((domains, vouched))
}

/// A box's view with its relay pass, on an edge that keeps a route table.
/// The box hands the pass to a local edge, if it is behind one, and that is
/// how its domains follow it there (`crate::routes`).
fn with_pass(st: &AppState, tenant: &str, mut view: DomainsView) -> DomainsView {
    if let Some(rt) = &st.routes {
        view.relay_pass = Some(rt.key.issue(tenant, crate::market::now_secs()));
    }
    view
}

async fn domains_list(
    State(st): State<AppState>,
    Json(req): Json<MarketAuth>,
) -> Result<Json<DomainsView>, ApiError> {
    let (store, vouched) = domains_tenant(&st, &req).await?;
    let state = store.snapshot().await;
    Ok(Json(with_pass(
        &st,
        &req.appliance_id,
        domains::view(&state, &req.appliance_id, vouched.as_ref(), &store.opts),
    )))
}

async fn domains_add(
    State(st): State<AppState>,
    Json(req): Json<DomainReq>,
) -> Result<(StatusCode, Json<DomainsView>), ApiError> {
    let (store, vouched) = domains_tenant(&st, &req.auth).await?;
    if vouched.is_none() {
        return Err(ApiError::Domains(DomainError::Conflict(
            "a custom domain needs this box's Stripe account, checked by Stripe; finish it on the Market pane first",
        )));
    }
    let domain = domains::normalize_domain(&req.domain, &store.opts)?;
    let now = crate::market::now_secs();
    let state = store
        .update(|s| domains::add(s, &req.auth.appliance_id, &domain, now))
        .await?;
    tracing::info!(
        target: Action::Domains.target(),
        "{} claimed {domain}",
        req.auth.appliance_id,
    );
    // Look it up now rather than at the next tick.
    st.notify.notify_one();
    Ok((
        StatusCode::CREATED,
        Json(with_pass(
            &st,
            &req.auth.appliance_id,
            domains::view(
                &state,
                &req.auth.appliance_id,
                vouched.as_ref(),
                &store.opts,
            ),
        )),
    ))
}

async fn domains_remove(
    State(st): State<AppState>,
    Json(req): Json<DomainReq>,
) -> Result<Json<DomainsView>, ApiError> {
    let (store, vouched) = domains_tenant(&st, &req.auth).await?;
    // Not normalised through the full check: a claim made under rules that
    // have since tightened must still be removable.
    let domain = req.domain.trim().trim_end_matches('.').to_ascii_lowercase();
    let state = store
        .update(|s| domains::remove(s, &req.auth.appliance_id, &domain))
        .await?;
    tracing::info!(
        target: Action::Domains.target(),
        "{} released {domain}",
        req.auth.appliance_id,
    );
    // Its router goes at once.
    st.notify.notify_one();
    Ok(Json(with_pass(
        &st,
        &req.auth.appliance_id,
        domains::view(
            &state,
            &req.auth.appliance_id,
            vouched.as_ref(),
            &store.opts,
        ),
    )))
}

/// Look up the claims that are due ([`domains::Claim::due`]), at most
/// [`domains::CHECKS_PER_PASS`] of them, and record what DNS said.
///
/// The lookups run with no lock held; the results are applied afterwards to
/// whatever the store holds then, so a claim removed meanwhile stays removed.
/// Never fatal: a failure here costs domains, not tunnels.
async fn check_domains(st: &AppState) {
    let (Some(store), Some(doh)) = (&st.domains, &st.doh) else {
        return;
    };
    let now = crate::market::now_secs();
    let tenants = match st.tenants.load(&st.opts.tenants_file).await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(target: Action::Domains.target(), "not checking domains: {e}");
            return;
        }
    };
    // Prune first: removed tenants and week-old waiting claims.
    if let Err(e) = store
        .update(|s| {
            let mut next = s.clone();
            domains::prune(&mut next, |t| tenants.contains_key(t), now);
            Ok(next)
        })
        .await
    {
        tracing::error!(target: Action::Domains.target(), "pruning domains failed: {e}");
    }
    let snapshot = store.snapshot().await;
    let due: Vec<domains::Claim> = snapshot
        .claims
        .iter()
        .filter(|c| c.due(now))
        .take(domains::CHECKS_PER_PASS)
        .cloned()
        .collect();
    if due.is_empty() {
        return;
    }
    let mut results = Vec::with_capacity(due.len());
    for claim in due {
        let vouched = vouched_for(st, &claim.tenant).await;
        // No Stripe data: nothing to look up, the answer is already known.
        let observed = if vouched.is_some() {
            doh.observe(&claim.domain).await
        } else {
            domains::Observed {
                txt: Ok(Vec::new()),
                addrs: Ok(Vec::new()),
            }
        };
        results.push((claim, vouched, observed));
    }
    let outcome = store
        .update(|s| {
            let mut next = s.clone();
            for (claim, vouched, observed) in &results {
                let taken = next
                    .claims
                    .iter()
                    .any(|c| c.domain == claim.domain && c.tenant != claim.tenant && c.live());
                if let Some(slot) = next
                    .claims
                    .iter_mut()
                    .find(|c| c.domain == claim.domain && c.tenant == claim.tenant)
                {
                    let was_live = slot.live();
                    *slot = domains::evaluate(
                        slot,
                        vouched.as_ref(),
                        &store.opts,
                        taken,
                        observed,
                        now,
                    );
                    if slot.live() != was_live {
                        tracing::info!(
                            target: Action::Domains.target(),
                            "{} for {}: {}",
                            slot.domain,
                            slot.tenant,
                            if slot.live() { "live" } else { "offline" },
                        );
                    }
                }
            }
            Ok(next)
        })
        .await;
    if let Err(e) = outcome {
        tracing::error!(target: Action::Domains.target(), "recording domain checks failed: {e}");
    }
}

/// Stripe's event delivery. No appliance token: the signature over the raw body
/// is the credential, so the body is taken as bytes and never re-serialised.
async fn market_webhook(
    State(st): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    let market = market_of(&st).await?;
    let signature = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError::Market(MarketError::BadSignature))?;
    market.webhook(signature, &body).await?;
    // A payment may just have become a volume to provision.
    st.notify.notify_one();
    Ok(StatusCode::OK)
}

/// Expire pending orders whose hold has run out, free the units of lapsed
/// storage orders whose claim an operator has deleted, then provision the
/// volume for every paid storage order that lacks one.
///
/// Runs after every reconcile pass, and the webhook kicks the reconciler, so a
/// purchase is usually fulfilled within a second and a failed attempt is
/// retried every `--reconcile-interval`. It is idempotent: a `409` from the
/// apiserver means a previous attempt created the object and died before the
/// order was marked, and counts as success. Nothing is ever deleted — a lapsed
/// order stops being reported as an entitlement but its volume is the buyer's
/// data, and removing it is an operator decision.
///
/// Compute orders have nothing to provision: they are a ledger credit.
async fn fulfil_market(st: &AppState) {
    let Some(market) = &st.market else {
        return;
    };
    // Which ledger to work on follows the gate's key. Once a request has
    // told, the pass keeps to that ledger: its orders are still owed their
    // volumes even if the key changes before the next request notices.
    if market.mode().await.is_none() && market.validate_secrets().await.is_err() {
        return;
    }
    match market.expire_stale().await {
        Ok(expired) => {
            for id in expired {
                tracing::info!(
                    target: Action::Market.target(),
                    "order {id} expired: no payment arrived while its units were held",
                );
            }
        }
        Err(e) => tracing::error!(
            target: Action::Market.target(),
            "could not record stale orders as expired: {e}",
        ),
    }
    let pending = market.pending_provisions().await;
    let lapsed = market.lapsed_volumes().await;
    if pending.is_empty() && lapsed.is_empty() {
        return;
    }
    let kube = match kube_access(st).await {
        Ok(kube) => kube,
        Err(e) => {
            tracing::error!(
                target: Action::Market.target(),
                "{} paid storage order(s) cannot be fulfilled and {} lapsed volume(s) cannot be checked: {e}",
                pending.len(),
                lapsed.len(),
            );
            return;
        }
    };
    reclaim_lapsed(&kube, market, lapsed).await;
    for p in pending {
        if let Err(e) = market.mark_claimed(&p.order_id, &p.namespace, &p.pvc).await {
            tracing::error!(
                target: Action::Market.target(),
                "order {} cannot be fulfilled until its claim is recorded: {e}",
                p.order_id,
            );
            continue;
        }
        match provision_volume(&kube, market.storage_class(), &p).await {
            Ok(()) => {
                if let Err(e) = market
                    .mark_provisioned(&p.order_id, &p.namespace, &p.pvc)
                    .await
                {
                    tracing::error!(
                        target: Action::Market.target(),
                        "volume for order {} exists but could not be recorded: {e}",
                        p.order_id,
                    );
                } else {
                    tracing::info!(
                        target: Action::Market.target(),
                        "provisioned {}/{} ({} GiB) for order {}",
                        p.namespace,
                        p.pvc,
                        p.gib,
                        p.order_id,
                    );
                }
            }
            Err(e) => tracing::error!(
                target: Action::Market.target(),
                "provisioning order {}: {e}",
                p.order_id,
            ),
        }
    }
}

/// Return a lapsed order's units to its listing once its claim is gone.
///
/// Nothing on the edge deletes a buyer's volume: it holds their data, and
/// removing it is the operator's decision. Until they do, Longhorn keeps its
/// GiB, so the order keeps them reserved too. This notices the deletion.
async fn reclaim_lapsed(
    kube: &KubeAccess,
    market: &crate::market::Market,
    lapsed: Vec<(String, String)>,
) {
    for (order_id, volume) in lapsed {
        let Some((namespace, pvc)) = volume.split_once('/') else {
            continue;
        };
        let url = format!(
            "{}/api/v1/namespaces/{namespace}/persistentvolumeclaims/{pvc}",
            kube.api,
        );
        match kube_claim_exists(kube, &url).await {
            Ok(true) => {}
            Ok(false) => match market.mark_reclaimed(&order_id).await {
                Ok(()) => tracing::info!(
                    target: Action::Market.target(),
                    "claim {volume} of lapsed order {order_id} is gone; its units are for sale again",
                ),
                Err(e) => tracing::error!(
                    target: Action::Market.target(),
                    "claim {volume} is gone but order {order_id} could not be updated: {e}",
                ),
            },
            Err(e) => tracing::error!(
                target: Action::Market.target(),
                "checking claim {volume} of lapsed order {order_id}: {e}",
            ),
        }
    }
}

/// Whether the claim at `url` exists. Only a `404` means it does not; any
/// other failure is an error, so an apiserver hiccup never frees units that
/// are still in use.
async fn kube_claim_exists(kube: &KubeAccess, url: &str) -> Result<bool, ApiError> {
    let response = kube
        .client
        .get(url)
        .bearer_auth(&kube.token)
        .send()
        .await
        .map_err(|e| ApiError::KubeApi(format!("GET {url}: {e}")))?;
    match response.status() {
        reqwest::StatusCode::NOT_FOUND => Ok(false),
        status if status.is_success() => Ok(true),
        status => Err(ApiError::KubeApi(format!("GET {url} -> {status}"))),
    }
}

/// POST `body` to `url`; `201 Created` and `409 Already Exists` are success.
async fn kube_create(
    kube: &KubeAccess,
    url: &str,
    body: &serde_json::Value,
) -> Result<(), ApiError> {
    let response = kube
        .client
        .post(url)
        .bearer_auth(&kube.token)
        .json(body)
        .send()
        .await
        .map_err(|e| ApiError::KubeApi(format!("POST {url}: {e}")))?;
    let status = response.status();
    if status.is_success() || status == StatusCode::CONFLICT {
        return Ok(());
    }
    Err(ApiError::KubeApi(format!("POST {url} -> {status}")))
}

/// The claim's `status.phase`, e.g. `Pending` or `Bound`.
async fn kube_claim_phase(kube: &KubeAccess, url: &str) -> Result<String, ApiError> {
    let response = kube
        .client
        .get(url)
        .bearer_auth(&kube.token)
        .send()
        .await
        .map_err(|e| ApiError::KubeApi(format!("GET {url}: {e}")))?;
    let status = response.status();
    if !status.is_success() {
        return Err(ApiError::KubeApi(format!("GET {url} -> {status}")));
    }
    let claim: serde_json::Value = response
        .json()
        .await
        .map_err(|e| ApiError::KubeApi(format!("GET {url}: {e}")))?;
    Ok(claim["status"]["phase"]
        .as_str()
        .unwrap_or("Unknown")
        .to_string())
}

/// Ensure the buyer's namespace, then the order's claim inside it, and succeed
/// only once the claim is `Bound`.
///
/// A `201` only means the apiserver stored the object. With no such storage
/// class, or one out of capacity, the claim sits `Pending` forever, and an
/// order marked provisioned at that point would never be retried and would
/// report a volume that does not exist. So a claim that is not bound yet is an
/// error here, and the next reconcile pass checks again (the create is a `409`
/// by then). This assumes an `Immediate`-binding class, which Longhorn's
/// default is; a `WaitForFirstConsumer` class stays `Pending` until something
/// mounts the claim, which nothing on the edge does yet.
async fn provision_volume(
    kube: &KubeAccess,
    storage_class: &str,
    p: &Provision,
) -> Result<(), ApiError> {
    kube_create(
        kube,
        &format!("{}/api/v1/namespaces", kube.api),
        &serde_json::json!({
            "apiVersion": "v1",
            "kind": "Namespace",
            "metadata": {
                "name": p.namespace,
                "labels": { "losos.market/managed": "true" },
            },
        }),
    )
    .await?;
    let claims = format!(
        "{}/api/v1/namespaces/{}/persistentvolumeclaims",
        kube.api, p.namespace,
    );
    kube_create(
        kube,
        &claims,
        &serde_json::json!({
            "apiVersion": "v1",
            "kind": "PersistentVolumeClaim",
            "metadata": {
                "name": p.pvc,
                "namespace": p.namespace,
                "labels": {
                    "losos.market/managed": "true",
                    "losos.market/order": p.pvc,
                },
            },
            "spec": {
                "accessModes": ["ReadWriteOnce"],
                "storageClassName": storage_class,
                "resources": { "requests": { "storage": format!("{}Gi", p.gib) } },
            },
        }),
    )
    .await?;
    match kube_claim_phase(kube, &format!("{claims}/{}", p.pvc))
        .await?
        .as_str()
    {
        "Bound" => Ok(()),
        phase => Err(ApiError::KubeApi(format!(
            "claim {}/{} is {phase}, not Bound yet; will check again",
            p.namespace, p.pvc,
        ))),
    }
}

/// An authenticated handle on the mesh apiserver: a client whose root store is
/// pinned to the cluster CA, the ServiceAccount bearer token, and the base URL.
struct KubeAccess {
    client: reqwest::Client,
    token: String,
    api: String,
}

/// Build the mesh apiserver client from `--kube-*`. Every failure is an edge
/// *configuration* fault: loud in the journal, opaque to the caller. See
/// [`cleanup_stale_node`] for what the CA pin buys.
async fn kube_access(st: &AppState) -> Result<KubeAccess, ApiError> {
    let Some(token_file) = &st.opts.kube_token_file else {
        tracing::error!(
            target: Action::Join.target(),
            "--kube-token-file was not supplied; cannot reach the mesh apiserver",
        );
        return Err(ApiError::MeshUnconfigured);
    };
    let pinned = st.opts.kube_api.starts_with("https://");
    if !pinned {
        tracing::warn!(
            target: Action::Join.target(),
            "--kube-api {} is not https; the ServiceAccount token crosses in cleartext and the cluster CA is not pinned",
            st.opts.kube_api,
        );
    }
    let ca_file = match (&st.opts.kube_ca_file, pinned) {
        (Some(path), _) => Some(path),
        (None, false) => None,
        (None, true) => {
            tracing::error!(
                target: Action::Join.target(),
                "--kube-ca-file was not supplied for an https apiserver; refusing to trust the public roots",
            );
            return Err(ApiError::MeshUnconfigured);
        }
    };
    let ca = match ca_file {
        Some(path) => {
            let pem = tokio::fs::read(path)
                .await
                .map_err(|e| ApiError::KubeApi(format!("read CA {path}: {e}")))?;
            Some(
                reqwest::Certificate::from_pem(&pem)
                    .map_err(|e| ApiError::KubeApi(format!("parse CA {path}: {e}")))?,
            )
        }
        None => None,
    };
    let token = tokio::fs::read_to_string(token_file)
        .await
        .map_err(|e| ApiError::KubeApi(format!("read kube token {token_file}: {e}")))?;
    let token = token.trim();
    if token.is_empty() {
        tracing::error!(
            target: Action::Join.target(),
            "kube token file {token_file} is empty; losos-mesh-rbac.service has not run",
        );
        return Err(ApiError::MeshUnconfigured);
    }

    // Built per request rather than cached in `AppState`. Joins are rare (one
    // per appliance per boot), and a fresh client picks up a rotated
    // ServiceAccount token or a re-issued cluster CA without restarting the
    // registrar — which would drop every tenant's tunnel for the restart.
    let mut builder = reqwest::Client::builder()
        .timeout(KUBE_TIMEOUT)
        .user_agent(concat!("losos-registrar/", env!("CARGO_PKG_VERSION")));
    if let Some(ca) = ca {
        builder = builder
            .tls_built_in_root_certs(false)
            .add_root_certificate(ca);
    }
    let client = builder
        .build()
        .map_err(|e| ApiError::KubeApi(format!("build kube client: {e}")))?;
    Ok(KubeAccess {
        client,
        token: token.to_string(),
        api: st.opts.kube_api.clone(),
    })
}

/// Delete the caller's `Node` object and its node-password `Secret` from the
/// mesh cluster, so a reinstalled box can rejoin under the same name.
///
/// 404 is success: it means this is a first join and there was nothing to
/// clean. Any other non-2xx, and any transport failure, is a 503 — never a
/// success, because the whole reason the route deletes anything is that
/// proceeding without the delete is what bricks the rejoin.
///
/// Transport is the crate's existing reqwest+rustls, one bearer token, no
/// kubeconfig parsing and no client certificates. The root store is *pinned*
/// to the cluster CA (`tls_built_in_root_certs(false)`): the apiserver's
/// certificate is issued by rke2's own CA, so trusting the public webpki roots
/// here would only widen who can impersonate it.
///
/// `--kube-ca-file` is required only for an `https://` apiserver, mirroring
/// `announce`'s conditional `https_only`: the VM tests point `--kube-api` at a
/// plain-HTTP stub on loopback, and demanding a PEM there would mean the join
/// path could only ever be exercised on a box with a real cluster on it.
/// `modules/edge.nix` always generates `https://127.0.0.1:6443`, so in
/// production the pin is on — and a plain-HTTP value logs a warning naming what
/// it costs, since the ServiceAccount bearer token then crosses in cleartext.
async fn cleanup_stale_node(st: &AppState, node_name: &str) -> Result<(), ApiError> {
    // The name is interpolated into a request path. It reaches here having
    // already been proved equal to a whitelist key, so this is an edge
    // *configuration* fault, not an attack — but a key carrying a slash would
    // let the path escape the collection it is meant to address, and a name
    // Kubernetes cannot hold is one the agent could never register under
    // either. Loud in the journal, opaque 503 to the caller.
    if let Some(fault) = kube_name_fault(node_name) {
        tracing::error!(
            target: Action::Join.target(),
            "appliance id {node_name:?} is {fault}, so it cannot be a Kubernetes node name",
        );
        return Err(ApiError::KubeApi(format!("unusable node name: {fault}")));
    }

    let kube = kube_access(st).await?;
    let targets = [
        format!("{}/api/v1/nodes/{node_name}", kube.api),
        format!(
            "{}/api/v1/namespaces/kube-system/secrets/{node_name}.node-password.rke2",
            kube.api,
        ),
    ];
    for url in targets {
        let response = kube
            .client
            .delete(&url)
            .bearer_auth(&kube.token)
            .send()
            .await
            .map_err(|e| ApiError::KubeApi(format!("DELETE {url}: {e}")))?;
        let status = response.status();
        // 404 is the first-join case: nothing to clean is a clean result.
        if status.is_success() || status.as_u16() == 404 {
            tracing::debug!(target: Action::Join.target(), "DELETE {url} -> {status}");
            continue;
        }
        return Err(ApiError::KubeApi(format!("DELETE {url} -> {status}")));
    }
    Ok(())
}

/// Why `name` cannot be a Kubernetes object name, or `None` if it can.
///
/// Kubernetes names are DNS subdomains: lowercase alphanumerics, `-` and `.`,
/// at most 253 characters. Checking the charset rather than only rejecting `/`
/// keeps this a whitelist — the point is that nothing which is not a plain
/// path segment ever reaches the URL builder.
fn kube_name_fault(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        Some("empty")
    } else if name.len() > MAX_NODE_NAME_LEN {
        Some("longer than 253 characters")
    } else if !name
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
    {
        Some("not lowercase alphanumeric, '-' and '.' only")
    } else {
        None
    }
}

/// One tenant of the operator whitelist (`tenants.json`): the hostname it is
/// allowed to claim, the path of the file holding its token, and whether the
/// operator has cleared it for the mesh cluster. The paths are public (they
/// live in the nix store); the *contents* are the secret.
///
/// `cluster` is `#[serde(default)]` so a `tenants.json` generated before the
/// mesh existed still parses — and defaults to `false`, which is the safe
/// direction. Note that `modules/edge.nix` has to be taught to emit the field
/// at all: its `tenantsJson` builds the attribute set by hand, and an
/// unmodified one silently drops the flag and 403s every join.
#[derive(Debug, Clone, Deserialize)]
struct TenantEntry {
    hostname: String,
    token_file: String,
    #[serde(default)]
    cluster: bool,
    /// May buy and sell on the market. A third, separate bit: publishing a
    /// website or lending compute does not mean the operator agreed to settle
    /// money with this box.
    #[serde(default)]
    market: bool,
    /// The DNS zone this tenant may relay other boxes under (`crate::relay`):
    /// `Some` makes it a spoke of this hub. A fourth bit, absent for a plain
    /// box, and like the others it reaches the registrar only because
    /// `modules/edge.nix` renders `relayZone` into tenants.json.
    #[serde(default)]
    relay_zone: Option<String>,
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
    /// As [`TenantCache::load`], with an absent file read as an empty list:
    /// the enrolled file does not exist until the first box arrives.
    async fn load_optional(
        &self,
        path: &str,
    ) -> Result<Arc<HashMap<String, TenantEntry>>, ApiError> {
        match self.load(path).await {
            Ok(map) => Ok(map),
            Err(ApiError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(Arc::new(HashMap::new()))
            }
            Err(e) => Err(e),
        }
    }

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

    let Some(entry) = lookup_tenant(st, &tenants, id).await? else {
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
    Ok(entry)
}

/// The whitelist row for `id`, or the enrolled one when this edge enrols
/// boxes on first contact. The whitelist wins: an operator row for an id a
/// box also enrolled under is the operator's decision about that id.
async fn lookup_tenant(
    st: &AppState,
    tenants: &HashMap<String, TenantEntry>,
    id: &str,
) -> Result<Option<TenantEntry>, ApiError> {
    if let Some(entry) = tenants.get(id) {
        return Ok(Some(entry.clone()));
    }
    let Some(enrolment) = &st.enrolment else {
        return Ok(None);
    };
    let enrolled = st
        .enrolled
        .load_optional(&enrolment.tenants_file().to_string_lossy())
        .await?;
    Ok(enrolled.get(id).cloned())
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
        check_domains(&st).await;
        if let Err(e) = reconcile_once(&st).await {
            // Log and continue — the reconciler must not kill the server on a
            // single failed pass; the next tick retries.
            tracing::error!(target: Action::Reconcile.target(), "reconcile failed: {e}");
        }
        fulfil_market(&st).await;
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
///
/// It also publishes the mesh compute windows, but only *after* both proxy
/// config files are on disk, and it logs rather than propagating a failure
/// there. `serve` calls this once before the API opens and treats an error as
/// fatal, so a mesh-side write fault must never be able to stop an edge whose
/// operator has not enabled the cluster at all from booting.
///
/// And it does not prune while the API is shedding load. A missed heartbeat
/// is only evidence that an appliance went away if this registrar was able
/// to hear it: under a flood of slow anonymous requests every `/heartbeat`
/// was answered 503, the prune then took each tenant for dead, and the pass
/// below removed every router and rathole service on the edge. An attacker
/// with no token at all could take every appliance's public site down for as
/// long as the flood lasted. A dead appliance kept a little longer costs a
/// router that answers 502.
async fn reconcile_once(st: &AppState) -> Result<()> {
    let ttl = st.opts.heartbeat_ttl;
    let shedding = st
        .last_shed
        .lock()
        .map(|last| last.is_some_and(|t| t.elapsed() < ttl))
        .unwrap_or(true);
    let pruned = if shedding {
        tracing::warn!(
            target: Action::Serve.target(),
            "not pruning: requests were shed within the last {ttl:?}, so missed heartbeats prove nothing",
        );
        false
    } else {
        st.reg.prune(ttl).await
    };
    let views = st.reg.views().await;
    let tenants = all_tenants(st).await.context("load tenants whitelist")?;
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
    // The relayed half (crate::relay): a `<spoke>.<box>` tenant stays only
    // while its spoke is still whitelisted with a relay zone the hostname
    // fits, and it is rendered with the *spoke's* token, read from the
    // spoke's file like any direct tenant's. The zone is re-checked here and
    // not only at `/relay` so an operator narrowing or removing a zone takes
    // effect on the next tick, the same property the whitelist hostname has.
    let mut spoke_tokens: HashMap<String, Option<String>> = HashMap::new();
    for r in st.reg.relayed().await {
        let fits = tenants.get(&r.via).and_then(|spoke| {
            spoke
                .relay_zone
                .as_deref()
                .filter(|zone| relay::hostname_in_zone(&r.hostname, zone))
                .map(|_| spoke)
        });
        let Some(spoke) = fits else {
            tracing::info!(
                target: Action::Reconcile.target(),
                "dropping relayed {}: its spoke {} no longer relays {}",
                r.key,
                r.via,
                r.hostname,
            );
            stale.push(r.key.clone());
            continue;
        };
        let token = match spoke_tokens.get(&r.via) {
            Some(cached) => cached.clone(),
            None => {
                let read = match tokio::fs::read_to_string(&spoke.token_file).await {
                    Ok(content) => match token_fault(content.trim()) {
                        None => Some(content.trim().to_string()),
                        Some(fault) => {
                            tracing::error!(
                                target: Action::Reconcile.target(),
                                "skipping every box of spoke {}: token file {} is {fault}",
                                r.via,
                                spoke.token_file,
                            );
                            None
                        }
                    },
                    Err(e) => {
                        tracing::warn!(
                            target: Action::Reconcile.target(),
                            "skipping every box of spoke {}: cannot read token file {}: {e}",
                            r.via,
                            spoke.token_file,
                        );
                        None
                    }
                };
                spoke_tokens.insert(r.via.clone(), read.clone());
                read
            }
        };
        let Some(token) = token else {
            continue;
        };
        enriched.push(TenantView {
            id: r.key,
            hostname: r.hostname,
            rathole_port: r.rathole_port,
            token,
        });
    }
    for id in &stale {
        // best-effort; a failure just means it lingers until next prune
        if let Err(e) = st.reg.deregister(id).await {
            tracing::warn!(target: Action::Reconcile.target(), "dropping stale {id} failed: {e}");
        }
    }
    // The whitelist is the authority for compute windows too: a tenant the
    // operator has deleted must stop steering the taint on its node.
    let allowed: HashSet<&str> = tenants.keys().map(String::as_str).collect();
    let mut dropped_windows = false;
    match st.reg.retain_windows(&allowed).await {
        Ok(changed) => dropped_windows = changed,
        Err(e) => {
            tracing::warn!(target: Action::Reconcile.target(), "pruning compute windows failed: {e}");
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
        noise_private_key: match &st.opts.noise_private_key_file {
            None => None,
            Some(f) => Some(
                tokio::fs::read_to_string(f)
                    .await
                    .into_diagnostic()
                    .with_context(|| format!("read noise private key file {f}"))?
                    .trim()
                    .to_string(),
            ),
        },
    };
    // The domain half: a name in the edge's zone for each box Stripe vouches
    // for, every proved custom domain, and the zone file itself. Computed
    // before the Traefik file so both describe the same moment.
    let (extra_hosts, zone_names) = domain_hosts(st, &enriched, &tenants).await;
    let files = desired_config_with_hosts(&enriched, &opts, &extra_hosts);

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
    // Never fatal, like the windows below: a zone that cannot be written
    // costs DNS answers, not tunnels.
    let changed_zone = match write_zone(st, &zone_names).await {
        Ok(changed) => changed,
        Err(e) => {
            tracing::error!(target: Action::Domains.target(), "writing the zone failed: {e:?}");
            false
        }
    };

    // Published last and never fatal: this is the mesh half. `write_if_changed`
    // creates the parent directory, so on an edge that has never seen a join
    // the steady state is one small `{"nodes": []}` — a truthful statement that
    // no window is recorded, which is what the taint timer needs to read in
    // order to remove a taint it set earlier.
    let windows = st.reg.compute_windows().await;
    // Live, and deliberately not persisted: an edge that has just restarted
    // knows nothing about who is idle, and "nobody" is the safe answer until
    // each box says otherwise on its next heartbeat.
    let idle = st.reg.idle_nodes(st.opts.heartbeat_ttl).await;
    let changed_windows = match write_if_changed(
        Path::new(&st.opts.compute_windows_file),
        &window::render(&windows, &idle),
        COMPUTE_WINDOWS_FILE_MODE,
    )
    .await
    {
        Ok(changed) => changed,
        Err(e) => {
            tracing::error!(
                target: Action::Reconcile.target(),
                "writing {} failed: {e:?}",
                st.opts.compute_windows_file,
            );
            false
        }
    };

    if pruned
        || repaired
        || changed_traefik
        || changed_rathole
        || dropped_windows
        || changed_windows
        || changed_zone
    {
        tracing::info!(
            target: Action::Reconcile.target(),
            "reconciled {} tenant(s), {} compute window(s); traefik={} rathole={} windows={}",
            enriched.len(),
            windows.len(),
            changed_traefik,
            changed_rathole,
            changed_windows,
        );
    }
    Ok(())
}

/// Extra Traefik hostnames per live tenant, and the names for the zone.
///
/// Empty on an edge with no zone, and on one that is not (or no longer)
/// official: an edge that loses its certificate stops serving the names it
/// handed out, the same way its boxes stop trusting it.
async fn domain_hosts(
    st: &AppState,
    live: &[TenantView],
    tenants: &HashMap<String, TenantEntry>,
) -> (BTreeMap<String, Vec<String>>, ZoneNames) {
    let mut hosts: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut names = ZoneNames::default();
    let Ok(store) = domains_of(st) else {
        return (hosts, names);
    };
    let zone = &store.opts.zone;
    // Tenant hostnames inside the zone: every whitelisted one, live or not,
    // so a box that reboots keeps its name and only its router comes and goes.
    for entry in tenants.values() {
        names.add_fqdn(&entry.hostname, zone);
    }
    // And the registrar's own name, when the zone is the whole public domain.
    if !store.opts.public_domain.is_empty() {
        names.add_fqdn(&format!("register.{}", store.opts.public_domain), zone);
    }
    // One name per vouched box. Also whitelisted-only: a seller the operator
    // has removed gets nothing.
    if let Some(market) = &st.market {
        for (tenant, seller) in market.sellers().await {
            if !tenants.contains_key(&tenant) {
                continue;
            }
            if let Some(v) = Vouched::from_seller(Some(&seller)) {
                let target = store.opts.target_for(&v.box_uuid);
                names.add_fqdn(&target, zone);
                hosts.entry(tenant).or_default().push(target);
            }
        }
    }
    for (tenant, domains) in domains::live_routes(&store.snapshot().await) {
        hosts.entry(tenant).or_default().extend(domains);
    }
    // A box behind a local edge: its names go to its relayed tenant, as the
    // route table says (crate::routes).
    if let Some(rt) = &st.routes {
        for row in relayed_routes(st, rt, live, &hosts).await {
            hosts.entry(row.service).or_default().push(row.domain);
        }
    }
    // Routers only for tenants that are live right now.
    let live_ids: HashSet<&str> = live.iter().map(|t| t.id.as_str()).collect();
    hosts.retain(|id, _| live_ids.contains(id.as_str()));
    (hosts, names)
}

/// Plan the route table, keep it, and return the rows to route. The file
/// beside the registry follows the table; failing to write it is logged and
/// costs nothing until the next restart, which would then start from older
/// bindings.
async fn relayed_routes(
    st: &AppState,
    rt: &RouteTable,
    live: &[TenantView],
    hosts: &BTreeMap<String, Vec<String>>,
) -> Vec<RouteRow> {
    let live_ids: HashSet<String> = live.iter().map(|t| t.id.clone()).collect();
    let relayed_live: HashSet<String> = st
        .reg
        .relayed()
        .await
        .into_iter()
        .map(|r| r.key)
        .filter(|k| live_ids.contains(k))
        .collect();
    let direct_live: HashSet<String> = live_ids.difference(&relayed_live).cloned().collect();
    let mesh: HashSet<String> = st.reg.compute_windows().await.into_keys().collect();
    let (table, text) = {
        let bindings = rt
            .bindings
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let table = routes::plan(&PlanInput {
            hosts,
            direct_live: &direct_live,
            relayed_live: &relayed_live,
            mesh: &mesh,
            bindings: &bindings,
            now: crate::market::now_secs(),
        });
        let text = TableFile::render(&bindings, &table);
        (table, text)
    };
    {
        let mut last = rt
            .last
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for key in last.keys().filter(|k| !table.contains_key(*k)) {
            tracing::info!(target: Action::Domains.target(), "route table: removing {key}");
        }
        for (key, row) in &table {
            if last.get(key) != Some(row) {
                tracing::info!(
                    target: Action::Domains.target(),
                    "route table: {} goes to {} through {}",
                    row.domain,
                    row.tenant,
                    row.spoke,
                );
            }
        }
        last.clone_from(&table);
    }
    if let Err(e) = write_if_changed(&rt.file, &text, ROUTE_TABLE_FILE_MODE).await {
        tracing::warn!(
            target: Action::Domains.target(),
            "route table {} not written: {e:#}",
            rt.file.display(),
        );
    }
    table.into_values().collect()
}

/// Render the zone and replace the file when it says something new.
async fn write_zone(st: &AppState, names: &ZoneNames) -> Result<bool> {
    let Some(store) = &st.domains else {
        return Ok(false);
    };
    let path = Path::new(&store.opts.zone_file);
    let existing = match tokio::fs::read_to_string(path).await {
        Ok(s) => Some(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return Err(e)
                .into_diagnostic()
                .with_context(|| format!("read {}", path.display()))
        }
    };
    match zone::next_zone(
        &store.opts,
        names,
        existing.as_deref(),
        crate::market::now_secs(),
    ) {
        None => Ok(false),
        // World-readable: Knot runs as its own user and the zone is public.
        Some(text) => write_if_changed(path, &text, ZONE_FILE_MODE).await,
    }
}

/// Write `content` to `path` only if it differs from the current content.
/// Atomic temp+fsync+rename via the shared blocking helper, with the given
/// filesystem mode. Returns `true` if the file was (re)written.
/// The whitelist plus, on an edge that enrols on first contact, the enrolled
/// boxes: one map, the whitelist winning on a shared id, as
/// [`lookup_tenant`] decides per request.
async fn all_tenants(st: &AppState) -> Result<Arc<HashMap<String, TenantEntry>>> {
    let tenants = st
        .tenants
        .load(&st.opts.tenants_file)
        .await
        .map_err(|e| miette!("{e}"))?;
    let Some(enrolment) = &st.enrolment else {
        return Ok(tenants);
    };
    let enrolled = st
        .enrolled
        .load_optional(&enrolment.tenants_file().to_string_lossy())
        .await
        .map_err(|e| miette!("{e}"))?;
    if enrolled.is_empty() {
        return Ok(tenants);
    }
    let mut merged: HashMap<String, TenantEntry> = (*enrolled).clone();
    for (id, entry) in tenants.iter() {
        merged.insert(id.clone(), entry.clone());
    }
    Ok(Arc::new(merged))
}

/// The spoke's uplink (crate::relay): every `interval`, and whenever the
/// reconciler was kicked, read the uplink file, tell the hub which boxes are
/// live here, and rewrite the uplink rathole client config.
///
/// The file is re-read on every pass so the gateway image's owner can write
/// it (or remove it) with no restart; while it is absent the uplink config is
/// removed and nothing is sent. A hub that refuses the call, or a box it
/// refuses, is logged and tried again next pass: the hub is another
/// machine, and nothing about this edge's own LAN waits on it.
async fn uplink_loop(st: AppState) {
    let Some(up) = st.opts.uplink.clone() else {
        return;
    };
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .user_agent(concat!("losos-registrar/", env!("CARGO_PKG_VERSION")))
        .build();
    let client = match client {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(target: Action::Uplink.target(), "cannot build the uplink client: {e}");
            return;
        }
    };
    loop {
        if let Err(e) = uplink_once(&st, &up, &client).await {
            tracing::warn!(target: Action::Uplink.target(), "uplink pass failed: {e}");
        }
        let tick = tokio::time::sleep(up.interval);
        tokio::pin!(tick);
        tokio::select! {
            () = st.notify.notified() => {}
            () = &mut tick => {}
        }
    }
}

/// One uplink pass. Returns what was accepted, for the log and the tests.
async fn uplink_once(
    st: &AppState,
    up: &relay::UplinkOpts,
    client: &reqwest::Client,
) -> Result<()> {
    let target = match tokio::fs::read(&up.file).await {
        Ok(bytes) => serde_json::from_slice::<UplinkFile>(&bytes)
            .into_diagnostic()
            .with_context(|| format!("parse uplink file {}", up.file))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if remove_if_exists(Path::new(&up.rathole_config)).await? {
                tracing::info!(
                    target: Action::Uplink.target(),
                    "no uplink file at {}; the uplink is off",
                    up.file,
                );
            }
            return Ok(());
        }
        Err(e) => {
            return Err(e)
                .into_diagnostic()
                .with_context(|| format!("read uplink file {}", up.file))
        }
    };
    let token = tokio::fs::read_to_string(&target.token_file)
        .await
        .into_diagnostic()
        .with_context(|| format!("read uplink token file {}", target.token_file))?;
    let token = token.trim().to_string();
    if let Some(fault) = token_fault(&token) {
        return Err(miette!(
            "uplink token file {} is {fault}",
            target.token_file
        ));
    }
    let bootstrap = match &target.bootstrap_token_file {
        Some(path) => tokio::fs::read_to_string(path)
            .await
            .into_diagnostic()
            .with_context(|| format!("read uplink bootstrap token file {path}"))?
            .trim()
            .to_string(),
        None => token.clone(),
    };
    let noise_public_key = match &target.noise_public_key_file {
        Some(path) => pin_hub_noise_key(client, &target.registrar_url, path).await,
        None => None,
    };

    // What is live here: the direct tenants whose token file reads, i.e.
    // exactly the set the local reconcile rendered. Hostnames come from the
    // whitelist (or the enrolled file), the same authority the local Traefik
    // router uses, so the hub sees the names this edge actually serves.
    let tenants = all_tenants(st).await?;
    let mut listed: Vec<relay::RelayTenant> = Vec::new();
    let mut services: Vec<UplinkService> = Vec::new();
    for v in st.reg.views().await {
        let Some(entry) = tenants.get(&v.id) else {
            continue;
        };
        let pass = st
            .passes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&v.id)
            .cloned();
        listed.push(relay::RelayTenant {
            id: v.id.clone(),
            hostname: entry.hostname.trim().to_string(),
            pass,
        });
        services.push(UplinkService {
            name: format!("{}.{}", target.id, v.id),
            local_port: v.rathole_port,
        });
    }
    if listed.len() > relay::MAX_RELAYED {
        tracing::warn!(
            target: Action::Uplink.target(),
            "{} boxes here but a hub relays at most {}; the rest wait",
            listed.len(),
            relay::MAX_RELAYED,
        );
        listed.truncate(relay::MAX_RELAYED);
    }

    let url = format!("{}/relay", target.registrar_url.trim_end_matches('/'));
    let body = RelayReq {
        appliance_id: target.id.clone(),
        token: token.clone(),
        tenants: listed,
    };
    let accepted: HashSet<String> = match client.post(&url).json(&body).send().await {
        Ok(r) if r.status().is_success() => {
            let resp: RelayResp = r.json().await.into_diagnostic().context("parse /relay")?;
            for refused in &resp.refused {
                tracing::warn!(
                    target: Action::Uplink.target(),
                    "the hub does not relay {}: {}",
                    refused.id,
                    refused.reason,
                );
            }
            record_hub_routes(st, &resp.routes).await;
            resp.accepted.into_iter().map(|a| a.id).collect()
        }
        Ok(r) => {
            return Err(miette!("hub {url} answered {}", r.status()));
        }
        Err(e) => return Err(miette!("hub {url}: {e}")),
    };
    // Only what the hub accepted gets a client service: a service the hub
    // has no matching server entry for makes rathole log an auth failure
    // every retry, for a box the hub told us it will not route anyway.
    let prefix = format!("{}.", target.id);
    services.retain(|s| {
        s.name
            .strip_prefix(&prefix)
            .is_some_and(|id| accepted.contains(id))
    });
    let rendered = uplink_config(
        &UplinkTarget {
            rathole_endpoint: target.rathole_endpoint.clone(),
            bootstrap_token: bootstrap,
            token,
            noise_public_key,
        },
        &services,
    );
    if write_if_changed(Path::new(&up.rathole_config), &rendered, RATHOLE_FILE_MODE).await? {
        tracing::info!(
            target: Action::Uplink.target(),
            "uplink to {} carries {} box(es)",
            target.registrar_url,
            services.len(),
        );
    }
    Ok(())
}

/// Keep the hub's word on which custom domains it routes to our boxes in
/// `hub-routes.json` beside the registry, so the site's owner can see it on
/// the gateway, and say so in the log when it changes. The hub's traffic for
/// these names arrives on each box's relayed service, so nothing here routes
/// by them.
async fn record_hub_routes(st: &AppState, routes: &[RelayRoute]) {
    let mut sorted = routes.to_vec();
    sorted.sort_by(|a, b| (&a.id, &a.domain).cmp(&(&b.id, &b.domain)));
    let path = Path::new(&st.opts.registry_path).with_file_name("hub-routes.json");
    let mut text = serde_json::to_string_pretty(&sorted).expect("routes serialize");
    text.push('\n');
    match write_if_changed(&path, &text, TRAEFIK_FILE_MODE).await {
        Ok(true) => {
            for r in &sorted {
                tracing::info!(
                    target: Action::Uplink.target(),
                    "the hub routes {} to {} through this edge",
                    r.domain,
                    r.id,
                );
            }
            if sorted.is_empty() {
                tracing::info!(target: Action::Uplink.target(), "the hub routes no custom domain through this edge");
            }
        }
        Ok(false) => {}
        Err(e) => tracing::warn!(target: Action::Uplink.target(), "{}: {e}", path.display()),
    }
}

/// The hub's Noise public key for the uplink: the pinned file when it
/// exists, else one fetch of the hub's `/noise-public-key`, written to the
/// file (0600, atomically) so the next pass and rathole read the same
/// bytes. A hub that answers 404 runs plain TCP, and nothing is written, so
/// a hub that turns Noise on later is pinned on the first pass after. Any
/// other failure is logged and the uplink waits for the next pass rather
/// than rendering a config that would connect unpinned.
async fn pin_hub_noise_key(
    client: &reqwest::Client,
    registrar_url: &str,
    path: &str,
) -> Option<String> {
    match tokio::fs::read_to_string(path).await {
        Ok(k) => {
            let k = k.trim().to_string();
            return if k.is_empty() { None } else { Some(k) };
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            tracing::warn!(target: Action::Uplink.target(), "read {path}: {e}");
            return None;
        }
    }
    let url = format!("{}/noise-public-key", registrar_url.trim_end_matches('/'));
    let key = match client.get(&url).send().await {
        Ok(r) if r.status() == reqwest::StatusCode::NOT_FOUND => return None,
        Ok(r) if r.status().is_success() => match r.text().await {
            Ok(t) => t.trim().to_string(),
            Err(e) => {
                tracing::warn!(target: Action::Uplink.target(), "{url}: {e}");
                return None;
            }
        },
        Ok(r) => {
            tracing::warn!(target: Action::Uplink.target(), "{url} answered {}", r.status());
            return None;
        }
        Err(e) => {
            tracing::warn!(target: Action::Uplink.target(), "{url}: {e}");
            return None;
        }
    };
    if key.is_empty() || key.len() > 128 || !key.chars().all(|c| c.is_ascii_graphic()) {
        tracing::warn!(target: Action::Uplink.target(), "{url}: not a Noise public key");
        return None;
    }
    if let Some(parent) = Path::new(path).parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    match crate::fsutil::atomic_write(Path::new(path), key.as_bytes(), 0o600).await {
        Ok(()) => {
            tracing::info!(
                target: Action::Uplink.target(),
                "pinned the hub's Noise public key from {url} into {path}",
            );
            Some(key)
        }
        Err(e) => {
            tracing::warn!(target: Action::Uplink.target(), "write {path}: {e}");
            None
        }
    }
}

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
