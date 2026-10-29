//! `losos-registrar serve` — the edge registration HTTP API + the reconciler.
//!
//! **This API is on the public internet.** `modules/edge.nix` gives Traefik a
//! `Host(register.<domain>)` router with no path rule and no middleware in
//! front of it, so every route below answers anyone who can resolve that name,
//! and the unit runs as root. Treat everything here as hostile input.
//!
//! The API is the only thing that mutates the registry; the reconciler is the
//! only thing that writes config files and decides the routes Traefik
//! discovers through the provider listener (`provider`). A `tokio::sync::Notify` lets the API
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
//! tunnel service provisioned with an empty token.
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

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
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
    desired_config_with_hosts, uplink_config, vm_routes, TenantView, UplinkService, UplinkTarget,
};
use crate::domains::{self, Doh, DomainError, Domains, DomainsView, Vouched};
use crate::error::ApiError;
use crate::gateway::Gateway;
use crate::hardware::{Catalogue, Line};
use crate::market::{
    AccountView, CheckoutView, HardwareCheckoutView, Market, MarketError, NewListing, NewOrder,
    OnboardView, Provision, PublicListing, Sharing,
};
use crate::official;
use crate::onramp::{Onramp, OnrampError, OrderView};
use crate::opts::ServeOpts;
use crate::registry::{Registry, Shared};
use crate::relay::{
    self, Enrolment, MainMember, RelayRefused, RelayReq, RelayResp, RelayRoute, UplinkFile,
};
use crate::routes::{self, Bindings, PassKey, PlanInput, RouteRow, TableFile};
use crate::secondary::{self, ClusterFile, Peers, SyncMsg};
use crate::stripe_gate::{GATE_TIMEOUT, MAX_GATE_CALLS_PER_REQUEST};
use crate::window::{self, valid_hhmm, valid_tz, ComputeWindow};
use crate::zone::{self, ZoneNames};

mod ci;
mod llm;
mod manage;
mod operator;
mod provider;
mod storage;
mod tang;
mod vm;

pub use provider::Discovery;

/// The zone file is public DNS data, read by the `knot` user.
const ZONE_FILE_MODE: u32 = 0o644;

/// Files that hold nothing secret (the hub's routes beside the registry).
const PUBLIC_FILE_MODE: u32 = 0o644;
/// The tunnel's files carry service tokens → owner-only.
const TUNNEL_FILE_MODE: u32 = 0o600;
/// The box ids `losos-hidden-sync` reads, as root: the ids are on the
/// public routes anyway, but nothing else needs them.
const HIDDEN_SERVICES_FILE_MODE: u32 = 0o600;

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

/// The coin routes ask the edge's own node several things in a row (the
/// chain, a descriptor, an import) and may wait on both price sources, so
/// they get more than [`REQUEST_TIMEOUT`] and still stay under the box's
/// 25 s relay.
const COIN_ROUTE_TIMEOUT: Duration = Duration::from_secs(20);
const COIN_ROUTES: [&str; 3] = [
    "/market/coins/key",
    "/market/coins/pay",
    // Reads every tap of a meter back from the chain.
    "/market/taps/confirm",
];

/// Budget for `/operator/tokens*`, which asks GitHub who the caller is
/// before it looks at the list: one round trip to api.github.com, bounded
/// by [`GITHUB_TIMEOUT`], plus the file write.
const GITHUB_ROUTE_TIMEOUT: Duration = Duration::from_secs(GITHUB_TIMEOUT.as_secs() + 3);
/// `POST /passport/attest` waits its turn at the TPM
/// ([`crate::gateway::TPM_WAIT`]) and then runs half a dozen TPM commands.
const PASSPORT_ATTEST_ROUTE: &str = "/passport/attest";
const PASSPORT_ATTEST_TIMEOUT: Duration =
    Duration::from_secs(crate::gateway::TPM_WAIT.as_secs() + 20);
/// `POST /operator/call` may seal a secret, which runs `systemd-creds` and
/// restarts a unit (`crate::operator`); the script has 20 s of it.
const OPERATOR_CALL_ROUTE: &str = "/operator/call";
const OPERATOR_CALL_TIMEOUT: Duration = Duration::from_secs(25);
/// How long the edge waits on GitHub's `GET /user` for one operator call.
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
    /// Binance's on-ramp (`crate::onramp`); `None` without
    /// `--onramp-binance-api`.
    onramp: Option<Arc<Onramp>>,
    /// AI models across lending boxes; `None` without `--inference-models`.
    inference: Option<Arc<crate::inference::Inference>>,
    /// Chat requests in flight (`llm::MAX_CHATS`), which run outside
    /// [`guard`].
    chats: Arc<Semaphore>,
    /// An open gateway's passport and TPM (`crate::gateway`); `None` (no
    /// `--gateway-key-file`) makes every `/passport*` route a 404.
    gateway: Option<Arc<Gateway>>,
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
    /// Machine image transfers in flight (`vm::MAX_TRANSFERS`), which run
    /// outside [`guard`] and so outside [`AppState::limiter`].
    transfers: Arc<Semaphore>,
    /// The backup door on bought storage (`storage`), which also runs
    /// outside [`guard`].
    storage: Arc<storage::Door>,
    /// The terminal (`crate::operator`); `None` without `--access-dir`.
    operator: Option<Arc<crate::operator::Operator>>,
    /// The page boxes are managed from (`manage`); `None` without
    /// `--manage-key-file`.
    manage: manage::Shared,
    /// Call media for this edge's boxes (`crate::calls_relay`); `None`
    /// without `--calls-relay-listen`.
    calls_relay: Option<Arc<crate::calls_relay::Relay>>,
    /// Mesh CI's queue (`crate::ci`); `None` without `--ci-dir`.
    ci: Option<Arc<crate::ci::Ci>>,
    /// The boxes' Tang keys (`crate::tang`). `None` unless `--tang-dir`
    /// was given, and every `/tang/*` route answers 404.
    tang: Option<Arc<crate::tang::Tang>>,
    /// The secondary cluster's peer table (`crate::secondary`); `None`
    /// without `--secondary-file`.
    secondary: Option<Arc<Secondary>>,
    /// The main cluster as this edge sees it from `/relay`: every gateway
    /// that relays through it, by (site, gateway). Live state.
    main: Arc<std::sync::Mutex<BTreeMap<(String, String), MainRow>>>,
    /// What the hub the uplink points at said about itself last
    /// (`crate::official`): its official cluster and the official edges it
    /// lists, which is where the uplink goes if the hub is banned.
    hub: Arc<std::sync::Mutex<Option<HubView>>>,
    /// The routes Traefik's HTTP provider is served (`provider`).
    discovery: Discovery,
    /// The ECH key the zone publishes (`crate::ech`); `None` without
    /// `--ech-key-file`.
    ech: Option<Arc<crate::ech::EchKey>>,
}

/// A gateway of this gateway's secondary cluster, and the ids it was warned
/// about once.
struct Secondary {
    opts: secondary::SecondaryOpts,
    peers: Peers,
    /// Boxes a peer listed with a token other than ours: logged once each.
    conflicts: std::sync::Mutex<HashSet<String>>,
}

/// One gateway in [`AppState::main`].
#[derive(Debug, Clone)]
struct MainRow {
    cluster: Option<String>,
    boxes: usize,
    last: std::time::Instant,
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
    /// The memory the box lends to AI models now, MiB. Absent is none.
    #[serde(default)]
    llm_mib: Option<u64>,
    /// As on [`RegisterReq`].
    #[serde(default)]
    relay_pass: Option<String>,
    /// The box's owner lets this edge manage it (`manage`). Absent is no.
    #[serde(default)]
    managed: bool,
    /// The names the box ends TLS for itself (`crate::passthrough`); absent
    /// leaves its TLS to this edge, as before.
    #[serde(default)]
    tls: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct RegisterResp {
    tunnel_port: u16,
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
    /// Whether the box hosts virtual machines (`crate::vms`). Off for a
    /// client from before machines, which is the safe direction.
    #[serde(default)]
    host_vms: bool,
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

/// Bind `opts.listen` and the provider listener, and serve until
/// SIGTERM/SIGINT.
pub async fn run(opts: ServeOpts) -> Result<()> {
    let listener = TcpListener::bind(&opts.listen)
        .await
        .into_diagnostic()
        .with_context(|| format!("bind {}", opts.listen))?;
    tracing::info!(target: Action::Bind.target(), "listening on {}", opts.listen);
    let provider = TcpListener::bind(&opts.traefik_provider_listen)
        .await
        .into_diagnostic()
        .with_context(|| format!("bind {}", opts.traefik_provider_listen))?;
    tracing::info!(
        target: Action::Bind.target(),
        "serving Traefik's routes on {}",
        opts.traefik_provider_listen,
    );
    serve(listener, Some(provider), opts, shutdown_signal()).await
}

/// Serve on an already-bound listener until `shutdown` resolves.
///
/// Split out from [`run`] so the caller owns both the socket and the stop
/// condition: production passes a signal future, tests pass an ephemeral
/// listener (port 0) and a oneshot. In-flight requests finish before the
/// listener closes — a registrar killed mid-`/register` would otherwise leave
/// the appliance to time out and retry against a `registry.json` that may or
/// may not have been written. `provider` is the listener Traefik polls for
/// the routes; `None` serves the API alone.
pub async fn serve<F>(
    listener: TcpListener,
    provider: Option<TcpListener>,
    opts: ServeOpts,
    shutdown: F,
) -> Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    serve_app(build(opts).await?, listener, provider, shutdown).await
}

/// [`serve`] for an [`App`] the caller built, so it can keep a handle on
/// what the app publishes ([`App::discovery`]).
pub async fn serve_app<F>(
    app: App,
    listener: TcpListener,
    provider: Option<TcpListener>,
    shutdown: F,
) -> Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    // Traefik keeps the routes it has while this listener is down, and only
    // reloads when the document changes, so it needs no graceful stop.
    let provider = match provider {
        Some(listener) => {
            let addr = listener.local_addr().into_diagnostic()?;
            app.state
                .discovery
                .set_offline_url(format!("http://{addr}"));
            let routes = provider::router(app.state.discovery.clone());
            Some(tokio::spawn(async move {
                if let Err(e) = axum::serve(listener, routes).await {
                    tracing::error!(target: Action::Serve.target(), "provider listener: {e}");
                }
            }))
        }
        None => None,
    };
    let recon = tokio::spawn(reconciler(app.state.clone()));
    // The Statistics page's readings of Traefik (`crate::stats`).
    let scrape = app
        .state
        .manage
        .clone()
        .map(|m| tokio::spawn(manage::scrape_loop(m)));
    // The uplink is a loop of its own: it talks to another machine and must
    // never hold up the local reconcile.
    let uplink = app
        .state
        .opts
        .uplink
        .is_some()
        .then(|| tokio::spawn(uplink_loop(app.state.clone())));
    // The secondary cluster's sync talks to the LAN's other gateways; like
    // the uplink it must never hold up the local reconcile.
    let sync = app
        .state
        .secondary
        .is_some()
        .then(|| tokio::spawn(secondary_loop(app.state.clone())));
    // The calls relay authenticates a box exactly as /register does.
    let calls = app.state.calls_relay.clone().map(|relay| {
        let st = app.state.clone();
        let auth: crate::calls_relay::Auth = Arc::new(move |id, token| {
            let st = st.clone();
            Box::pin(async move { authenticate(&st, &id, &token).await.is_ok() })
        });
        tokio::spawn(async move {
            if let Err(e) = crate::calls_relay::run(relay, auth).await {
                tracing::error!(target: Action::Serve.target(), "calls relay: {e:?}");
            }
        })
    });

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
    if let Some(scrape) = scrape {
        scrape.abort();
        let _ = scrape.await;
    }
    if let Some(calls) = calls {
        calls.abort();
        let _ = calls.await;
    }
    if let Some(uplink) = uplink {
        uplink.abort();
        let _ = uplink.await;
    }
    if let Some(sync) = sync {
        sync.abort();
        let _ = sync.await;
    }
    if let Some(provider) = provider {
        provider.abort();
        let _ = provider.await;
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

    /// The routes this app publishes for Traefik.
    #[must_use]
    pub fn discovery(&self) -> Discovery {
        self.state.discovery.clone()
    }

    /// One reconciler pass, as the timer would run it: prune, decide the
    /// Traefik routes, rewrite the tunnel file if anything changed, publish the compute
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
    let onramp = match &opts.onramp {
        Some(o) => Some(Arc::new(
            Onramp::open(o.clone())
                .await
                .map_err(|e| miette!("open the onramp store: {e}"))?,
        )),
        None => None,
    };

    let inference = match &opts.inference {
        None => None,
        Some(i) => {
            if opts.market.as_ref().and_then(|m| m.usdc.as_ref()).is_none() {
                return Err(miette!(
                    "--inference-models needs the market with USDC (--market-usdc): models are paid for through it"
                ));
            }
            let state_file = Path::new(&opts.registry_path).with_file_name("inference.json");
            let heads = Arc::new(crate::inference::SystemdHeads::new(i));
            Some(Arc::new(
                crate::inference::Inference::open(i.clone(), opts.port_range, state_file, heads)
                    .await
                    .map_err(|e| miette!("open the model catalogue: {e}"))?,
            ))
        }
    };

    // Opened before the API accepts: the edge key is made here on the first
    // start if the file is missing, so the private half never travels.
    let gateway = match &opts.gateway {
        Some(g) => Some(Arc::new(
            Gateway::open(g.clone()).wrap_err("open the gateway's edge key")?,
        )),
        None => None,
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

    // The manage page's key, made on the first start like the terminal's.
    // The certificate is made here so `GET /calls/relay` can name it before
    // `serve` binds the relay.
    let calls_relay = match &opts.calls_relay {
        Some(c) => Some(Arc::new(crate::calls_relay::Relay::new(c.clone())?)),
        None => None,
    };
    let manage = match &opts.manage {
        None => None,
        Some(m) => Some(Arc::new(manage::Manage::load(m)?)),
    };

    // Opened before the API accepts: the key is made on the first start,
    // and a seeded key that does not parse stops the edge here.
    let operator = match &opts.operator {
        None => None,
        Some(o) => {
            let hostname = std::fs::read_to_string("/proc/sys/kernel/hostname").unwrap_or_default();
            let op = crate::operator::Operator::open(o, hostname.trim())?;
            tracing::info!(
                target: Action::Authenticate.target(),
                "terminal access ready: key {}, {} seeded key(s){}",
                op.host.fingerprint(),
                op.ring.keys().len(),
                if op.can_seal() { "" } else { ", no seal script" }
            );
            Some(Arc::new(op))
        }
    };

    let ci = match &opts.ci_dir {
        None => None,
        Some(dir) => Some(Arc::new(
            crate::ci::Ci::open(Path::new(dir))
                .await
                .into_diagnostic()
                .wrap_err("open the CI queue")?,
        )),
    };

    let tang = match &opts.tang {
        None => None,
        Some(t) => Some(Arc::new(
            crate::tang::Tang::open(t.clone())
                .await
                .into_diagnostic()
                .wrap_err("open the Tang key directory")?,
        )),
    };

    // Made on the first start like the manage key, so the zone's first
    // render already carries it.
    let ech = match &opts.ech {
        None => None,
        Some(e) => Some(Arc::new(crate::ech::load_or_make(
            Path::new(&e.key_file),
            &e.public_name,
        )?)),
    };

    let opts_secondary = opts.secondary.clone();
    let state = AppState {
        reg,
        opts: Arc::new(opts),
        notify: Arc::new(Notify::new()),
        tenants: Arc::new(TenantCache::default()),
        limiter: Arc::new(Semaphore::new(MAX_INFLIGHT)),
        last_shed: Arc::new(std::sync::Mutex::new(None)),
        market,
        onramp,
        inference,
        chats: llm::chats(),
        gateway,
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
        transfers: Arc::new(Semaphore::new(vm::MAX_TRANSFERS)),
        storage: Arc::new(
            storage::Door::new()
                .into_diagnostic()
                .wrap_err("build the storage client")?,
        ),
        operator,
        manage,
        calls_relay,
        ci,
        tang,
        secondary: opts_secondary.map(|opts| {
            Arc::new(Secondary {
                opts,
                peers: Peers::default(),
                conflicts: std::sync::Mutex::new(HashSet::new()),
            })
        }),
        main: Arc::new(std::sync::Mutex::new(BTreeMap::new())),
        hub: Arc::new(std::sync::Mutex::new(None)),
        discovery: Discovery::default(),
        ech,
    };

    // Generate config from whatever we just loaded, so the box is serving
    // before any heartbeat arrives.
    reconcile_once(&state).await?;
    // The machines' routes come from the ledger, which is open already: in
    // the first document too, so a restart does not drop them until the
    // market's first pass.
    if let Some(market) = &state.market {
        if market.vm_opts().is_some_and(|o| o.domain.is_some()) {
            state
                .discovery
                .set_vms(vm_routes(&market.vm_routes().await));
        }
    }

    let router = Router::new()
        // The edge as its boxes' Tang server (`tang`).
        .merge(tang::routes())
        .route("/health", get(health))
        .route("/tunnel-pin", get(tunnel_pin))
        .route("/calls/relay", get(calls_relay_info))
        .route("/calls/guest-link", post(calls_guest_link))
        // An open gateway's passport (`crate::gateway`): unauthenticated, a
        // box asks before it trusts anything. The settings route is
        // authenticated by the owner's OpenPGP signature over the body.
        .route("/passport", get(passport_route))
        .route("/passport/edge-key", get(passport_edge_key))
        .route("/passport/attest", post(passport_attest))
        .route("/passport/settings", post(passport_settings))
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
        // Secondary clusters (crate::secondary): what a box reads to learn
        // which tier its link to this edge is, and the gateways' sync.
        .route("/cluster/secondary", get(secondary_info))
        .route(secondary::SYNC_ROUTE, post(secondary_sync))
        // The optional Stripe Connect market. Every route answers 503 unless
        // the edge was started with `--market-gate-socket`. Browsing is
        // anonymous and shows no seller identity; everything else takes the
        // appliance token like the routes above; the webhook is authenticated
        // by Stripe's signature instead and needs a larger body than the rest.
        .route("/market/listings", get(market_browse).post(market_list))
        .route("/market/listings/close", post(market_close))
        .route("/market/seller/onboard", post(market_onboard))
        .route("/market/orders", post(market_order))
        .route("/market/payout", post(market_payout))
        .route("/market/sales/release", post(market_release))
        .route("/market/confirm", post(market_confirm))
        .route("/market/coins/key", post(market_coin_key))
        .route("/market/coins/pay", post(market_coin_pay))
        .route("/market/fees/pay", post(market_fees_pay))
        .route("/market/rpc", post(market_rpc))
        .route("/market/account", post(market_account))
        .route("/market/storage-door", post(storage::set_door))
        // Pay as you go for AI models (`llm`). The chat route is merged
        // below, outside the guard.
        .route("/market/llm/models", post(llm::overview))
        .route("/market/taps", post(llm::quote))
        .route("/market/taps/confirm", post(llm::confirm))
        .route("/market/taps/stop", post(llm::stop))
        .route("/market/hardware", get(market_hardware))
        .route("/market/hardware/checkout", post(market_hardware_checkout))
        // Virtual machines (crate::vms): the catalogue is anonymous like the
        // shelf, the rest take the appliance token. The two file transfers
        // are merged below, outside the body cap and the timeout.
        .route("/market/vm-images", get(vm::catalogue))
        .route("/market/vm-images/ticket", post(vm::ticket))
        .route("/market/vm-images/remove", post(vm::remove))
        .route("/market/vms/status", post(vm::status))
        .route(
            "/market/webhook",
            post(market_webhook).layer(DefaultBodyLimit::max(MAX_WEBHOOK_BYTES)),
        )
        // Euros to USDC at Binance (crate::onramp): 503 unless the edge has
        // a Binance partner. The two box routes take the appliance token;
        // the webhook is authenticated by Binance's signature.
        .route("/market/onramp", post(onramp_create))
        .route("/market/onramp/status", post(onramp_status))
        .route(
            "/market/onramp/webhook",
            post(onramp_webhook).layer(DefaultBodyLimit::max(MAX_WEBHOOK_BYTES)),
        )
        // Custom domains (crate::domains). 503 unless the edge serves a zone
        // and is a claimed gateway (crate::gateway); appliance token like
        // the market. None of them looks anything up in DNS: an add kicks
        // the reconciler, which does, outside the request's 5 s budget.
        .route("/domains/list", post(domains_list))
        .route("/domains/add", post(domains_add))
        .route("/domains/remove", post(domains_remove))
        .route("/domains/wallet", post(domains_wallet))
        // The terminal (crate::operator, docs/edge-terminal.md). 404 unless
        // the edge was started with `--access-dir`. The call is authenticated
        // by its envelope; the token routes by a GitHub account, like the
        // certificate push.
        .route("/operator/host-key", get(operator::host_key))
        .route(
            OPERATOR_CALL_ROUTE,
            post(operator::call).layer(DefaultBodyLimit::max(crate::operator::MAX_ENVELOPE_BYTES)),
        )
        .route("/operator/tokens", post(operator::create_token))
        .route("/operator/tokens/list", post(operator::list_tokens))
        .route("/operator/tokens/revoke", post(operator::revoke_token))
        // Bound at the router so an oversized body never materialises.
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        // Added last, so outermost: the timeout and the concurrency cap cover
        // body reading, routing and 404s, not just handler bodies.
        .layer(middleware::from_fn_with_state(state.clone(), guard))
        .with_state(state.clone())
        // Merged after the layers above, so none of them applies: a disk
        // image is gigabytes and takes minutes. `vm::transfer_router` has
        // its own cap, idle timeout and size limit.
        .merge(vm::transfer_router(state.clone()))
        // A box's backup on the storage it bought: packs of megabytes, many
        // of them (`storage`), with its own cap.
        .merge(storage::router(state.clone()))
        // The same for the manage page: an app install on a box outlasts
        // the API's budget (`manage`).
        .merge(manage::router(state.clone()))
        .merge(llm::chat_router(state.clone()))
        // Mesh CI: a claim waits for work and a submit carries an archive
        // (`ci`), so both run outside the guard with caps of their own.
        .merge(ci::router(state.clone()));

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
    } else if COIN_ROUTES.contains(&req.uri().path()) {
        COIN_ROUTE_TIMEOUT
    } else if req.uri().path().starts_with("/operator/tokens") {
        GITHUB_ROUTE_TIMEOUT
    } else if req.uri().path() == PASSPORT_ATTEST_ROUTE {
        PASSPORT_ATTEST_TIMEOUT
    } else if req.uri().path() == OPERATOR_CALL_ROUTE {
        OPERATOR_CALL_TIMEOUT
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

/// SHA-256 of the tunnel's certificate (`crate::tunnel`), for appliances to
/// pin on first start.
///
/// Unauthenticated on purpose: it names a public certificate, and the
/// appliance fetches it before it holds any credential, over the TLS the
/// edge is fronted with. 404 when the tunnel has not made its certificate yet.
async fn tunnel_pin(State(st): State<AppState>) -> Response {
    let Some(path) = &st.opts.tunnel_pin_file else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match tokio::fs::read_to_string(path).await {
        Ok(key) => key.trim().to_string().into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Where this edge takes call media for its boxes, and the hash of the
/// relay's certificate for them to pin (`crate::calls_relay`). Public: a box
/// asks before it connects, over the URL it registers with, and the token it
/// then proves itself with never travels here. 404 when the relay is off.
/// Body of `POST /calls/guest-link`.
#[derive(Debug, Deserialize)]
struct GuestLinkReq {
    appliance_id: String,
    token: String,
    call: String,
    invite: String,
}

/// `POST /calls/guest-link` — the link a guest opens to join one call on a
/// box behind this edge, with no account on the box. Only an official edge
/// makes one, and only on the box's own hostname, so a link always names
/// the box it leads to. The edge sees the call and the invite: enough to
/// knock, not to hear. The secret that lets the guest and the members trust
/// each other's keys is added in the inviting browser, after the `#`, and
/// never reaches the edge or the box.
async fn calls_guest_link(
    State(st): State<AppState>,
    Json(req): Json<GuestLinkReq>,
) -> Result<Response, ApiError> {
    let tenant = authenticate(&st, &req.appliance_id, &req.token).await?;
    if !claimed_gateway(&st) {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "notOfficial" })),
        )
            .into_response());
    }
    let hex32 =
        |s: &str| s.len() == 32 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    if !hex32(&req.call) || !hex32(&req.invite) {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "badInvite" })),
        )
            .into_response());
    }
    tracing::info!("calls: {} made a guest link", req.appliance_id);
    Ok(Json(serde_json::json!({
        "url": format!(
            "https://{}/calls/guest/?call={}&invite={}",
            tenant.hostname, req.call, req.invite
        )
    }))
    .into_response())
}

async fn calls_relay_info(State(st): State<AppState>) -> Response {
    match &st.calls_relay {
        Some(relay) => Json(relay.info()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// A gateway whose owner has claimed it: the settings were measured at this
/// boot. What lets this edge offer what only a trusted edge may; each box
/// checks for itself.
fn claimed_gateway(st: &AppState) -> bool {
    st.gateway.as_ref().is_some_and(|g| g.measured().is_some())
}

#[derive(Deserialize)]
struct PassportQuery {
    #[serde(default)]
    nonce: String,
}

/// `GET /passport?nonce=<hex>` — the measured settings, the edge key's
/// signature over the nonce, and the TPM's keys (`crate::gateway`). 404 on
/// an edge that is not a gateway or not claimed yet, 400 for a nonce it will
/// not sign.
async fn passport_route(
    State(st): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<PassportQuery>,
) -> Response {
    let Some(gw) = &st.gateway else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !crate::gateway::nonce_ok(&q.nonce) {
        return (
            StatusCode::BAD_REQUEST,
            "nonce must be 32 to 128 hex characters",
        )
            .into_response();
    }
    match gw.hello(&q.nonce) {
        Ok(Some(hello)) => Json(hello).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!(target: Action::Passport.target(), "the TPM files: {e}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "the TPM identity is not ready",
            )
                .into_response()
        }
    }
}

/// `GET /passport/edge-key` — the Ed25519 public key a passport must name,
/// 64 hex characters. 404 on an edge that is not a gateway.
async fn passport_edge_key(State(st): State<AppState>) -> Response {
    match &st.gateway {
        Some(gw) => format!("{}\n", gw.public_key()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// `POST /passport/attest` — open the box's credential and quote PCRs 4, 11
/// and 15 (`crate::gateway::Gateway::attest`). One at a time; 503 when the
/// TPM stays busy.
async fn passport_attest(
    State(st): State<AppState>,
    Json(req): Json<losos_access::gateway::check::AttestRequest>,
) -> Response {
    let Some(gw) = &st.gateway else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if gw.measured().is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    match gw.attest(req).await {
        Ok(answer) => Json(answer).into_response(),
        Err(e @ crate::gateway::AttestError::BadRequest(_)) => {
            (StatusCode::BAD_REQUEST, e.to_string()).into_response()
        }
        Err(e @ crate::gateway::AttestError::Busy) => {
            (StatusCode::SERVICE_UNAVAILABLE, e.to_string()).into_response()
        }
        Err(e) => {
            tracing::error!(target: Action::Passport.target(), "attestation: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// `POST /passport/settings` — new settings, signed by the owner with
/// OpenPGP (`crate::gateway::Claim`). On success the file is written and
/// the machine reboots to measure it, so the answer is the last thing this
/// boot says.
async fn passport_settings(
    State(st): State<AppState>,
    Json(claim): Json<crate::gateway::Claim>,
) -> Response {
    let Some(gw) = &st.gateway else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let now = i64::try_from(crate::market::now_secs()).unwrap_or(i64::MAX);
    match gw.claim(&claim, now) {
        Ok(owner) => Json(serde_json::json!({
            "accepted": true,
            "owner": owner,
            "rebooting": true,
        }))
        .into_response(),
        // JSON like the manage page's other answers: it shows `error`.
        Err(e @ crate::gateway::ClaimError::NotTheOwner) => (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
        Err(e @ crate::gateway::ClaimError::Write(_)) => {
            tracing::error!(target: Action::Passport.target(), "settings: {e}");
            (
                StatusCode::INSUFFICIENT_STORAGE,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// The GitHub gate of the operator routes (`/operator/tokens*`): the `Authorization: Bearer` header is a GitHub
/// token, GitHub says whose it is, and the account's numeric id must be on
/// the allowlist compiled into this binary (`operators.json`). `what` names
/// the request in the journal. The caller holds [`AppState::push_lock`], so
/// one round trip to GitHub is in flight at a time.
async fn github_operator(
    st: &AppState,
    headers: &HeaderMap,
    what: &str,
) -> std::result::Result<(crate::provision::GithubUser, crate::provision::Operator), Box<Response>>
{
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|t| !t.is_empty() && t.len() <= 512 && t.bytes().all(|b| b.is_ascii_graphic()));
    let Some(token) = token else {
        return Err(Box::new(
            (
                StatusCode::UNAUTHORIZED,
                "Authorization: Bearer <GitHub token> is required",
            )
                .into_response(),
        ));
    };
    let operators = match crate::provision::Operators::committed() {
        Ok(o) => o,
        Err(e) => {
            tracing::error!(target: Action::Passport.target(), "operators.json: {e}");
            return Err(Box::new(StatusCode::INTERNAL_SERVER_ERROR.into_response()));
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
            tracing::warn!(target: Action::Passport.target(), "{what} refused: {e}");
            return Err(Box::new((status, format!("GitHub: {e}")).into_response()));
        }
    };
    let Some(operator) = operators.find(user.id).cloned() else {
        tracing::warn!(
            target: Action::Passport.target(),
            "{what} refused: {} (GitHub id {}) is not on the operator allowlist",
            user.login, user.id
        );
        return Err(Box::new(
            (
                StatusCode::FORBIDDEN,
                format!(
                    "{} (GitHub id {}) is not on the operator allowlist",
                    user.login, user.id
                ),
            )
                .into_response(),
        ));
    };
    Ok((user, operator))
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
    Ok(Json(RegisterResp { tunnel_port: port }))
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
) -> Result<Response, ApiError> {
    authenticate(&st, &req.appliance_id, &req.token).await?;
    // Recorded before the liveness check so the two cannot disagree about
    // which heartbeat this was.
    st.reg
        .record_idle(&req.appliance_id, req.idle.unwrap_or(false))
        .await;
    st.reg
        .record_llm(&req.appliance_id, req.llm_mib.unwrap_or(0))
        .await;
    if st.reg.heartbeat(&req.appliance_id).await {
        remember_pass(&st, &req.appliance_id, req.relay_pass.as_deref());
        if let Some(manage) = &st.manage {
            manage.record(&req.appliance_id, req.managed);
        }
        // A box that ends TLS itself: its routes change with what it holds,
        // so the reconciler runs now rather than at its next tick.
        match st.reg.record_tls(&req.appliance_id, req.tls.clone()).await {
            Ok(true) => st.notify.notify_one(),
            Ok(false) => {}
            Err(e) => tracing::warn!(
                target: Action::Heartbeat.target(),
                "could not keep what {} said about its TLS: {e}",
                req.appliance_id,
            ),
        }
        let mut answer = serde_json::Map::new();
        // The box's Tor and I2P addresses on this edge, when it has any
        // (crate::hidden); the box shows them and lets LosOS Login answer
        // on them. An older box reads any 2xx as a heartbeat taken.
        if let Some(h) = &st.opts.hidden {
            let addresses = crate::hidden::read_addresses(Path::new(&h.addresses_file)).await;
            if let Some(a) = addresses.get(&req.appliance_id) {
                answer.insert("hidden".into(), serde_json::json!(a));
            }
        }
        // Every public name this edge routes to a box that ends TLS itself,
        // so it can get a certificate for each (crate::passthrough).
        if req.tls.is_some() {
            let names = st.discovery.names_of(&req.appliance_id);
            if !names.is_empty() {
                answer.insert("names".into(), serde_json::json!(names));
            }
        }
        if answer.is_empty() {
            return Ok(StatusCode::NO_CONTENT.into_response());
        }
        Ok((StatusCode::OK, Json(serde_json::Value::Object(answer))).into_response())
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
    if !tenant.cluster && !secondary_member(&st, &tenant).await {
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
        host_vms: req.host_vms,
    };
    if st.reg.set_compute_window(&req.node_name, window).await? {
        // Kick the reconciler so the taint timer sees the new window on its
        // next five-minute tick rather than up to `reconcile_interval` later.
        st.notify.notify_one();
    }

    tracing::info!(
        target: Action::Join.target(),
        "enrolled {} as mesh node {} (share_compute={}, host_vms={})",
        req.appliance_id,
        req.node_name,
        req.share_compute,
        req.host_vms,
    );
    Ok(Json(ClusterJoinResp {
        server_addr: server_addr.clone(),
        token: agent_token.to_string(),
        node_name: req.node_name,
    }))
}

/// Whether `tenant` is a box of this gateway's secondary cluster: enrolled
/// here (or at a peer and synced here) on a gateway that belongs to one.
/// Such a box may join the cluster's mesh, which is what a secondary cluster
/// is for. An enrolled box on a gateway in no cluster stays proxy-only, as
/// open enrolment always was, and a whitelist row keeps its own `cluster` bit.
async fn secondary_member(st: &AppState, tenant: &TenantEntry) -> bool {
    let (Some(sec), Some(enrolment)) = (&st.secondary, &st.enrolment) else {
        return false;
    };
    enrolment.owns(Path::new(&tenant.token_file))
        && read_cluster_file(&sec.opts.file).await.is_some()
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
    let relayed = st
        .reg
        .relay(&req.appliance_id, req.gateway.as_deref(), &accepted)
        .await?;
    // What each relayed box said through its spoke about ending TLS itself
    // (crate::passthrough), kept like a direct box's heartbeat says it.
    let mut tls_changed = false;
    for r in &relayed {
        let tls = req
            .tenants
            .iter()
            .find(|t| t.id == r.id)
            .and_then(|t| t.tls.clone());
        let key = format!("{}.{}", req.appliance_id, r.id);
        tls_changed |= st.reg.record_tls(&key, tls).await?;
    }
    if tls_changed {
        st.notify.notify_one();
    }
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
    let members = record_main_member(&st, &req, relayed.len()).await;
    st.notify.notify_one();
    Ok(Json(RelayResp {
        accepted: relayed,
        refused,
        routes,
        members,
    }))
}

/// Note a gateway's `/relay` in the main cluster's member table, drop the
/// gateways that went quiet for a heartbeat TTL, keep `main-cluster.json`
/// beside the registry for the operator, and answer with the site's own
/// gateways: a site sees its primary links, never another site's.
async fn record_main_member(st: &AppState, req: &RelayReq, boxes: usize) -> Vec<MainMember> {
    let now = std::time::Instant::now();
    let ttl = st.opts.heartbeat_ttl;
    let (site_rows, all_rows) = {
        let mut main = st
            .main
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        main.insert(
            (
                req.appliance_id.clone(),
                req.gateway.clone().unwrap_or_default(),
            ),
            MainRow {
                cluster: req.cluster.clone(),
                boxes,
                last: now,
            },
        );
        main.retain(|_, row| now.duration_since(row.last) < ttl);
        let member = |(site, gateway): &(String, String), row: &MainRow| MainMember {
            site: site.clone(),
            gateway: gateway.clone(),
            cluster: row.cluster.clone(),
            boxes: row.boxes,
            seen_ago_secs: now.duration_since(row.last).as_secs(),
        };
        let site: Vec<MainMember> = main
            .iter()
            .filter(|((site, _), _)| *site == req.appliance_id)
            .map(|(k, v)| member(k, v))
            .collect();
        // The file leaves the ages out, so it changes when the cluster does
        // and not on every call.
        let all: Vec<MainMember> = main
            .iter()
            .map(|(k, v)| MainMember {
                seen_ago_secs: 0,
                ..member(k, v)
            })
            .collect();
        (site, all)
    };
    write_main_cluster(st, &all_rows).await;
    site_rows
}

/// `main-cluster.json` beside the registry: the gateways of the main cluster
/// as this edge knows them, every link in it primary. On a hub that is every
/// site's gateways; on a gateway, its own site's, as the hub answered.
async fn write_main_cluster(st: &AppState, members: &[MainMember]) {
    let path = Path::new(&st.opts.registry_path).with_file_name("main-cluster.json");
    let rows: Vec<serde_json::Value> = members
        .iter()
        .map(|m| {
            serde_json::json!({
                "site": m.site,
                "gateway": m.gateway,
                "cluster": m.cluster,
                "boxes": m.boxes,
                "tier": secondary::Tier::Primary,
            })
        })
        .collect();
    let mut text = serde_json::to_string_pretty(&rows).expect("members serialize");
    text.push('\n');
    match write_if_changed(&path, &text, PUBLIC_FILE_MODE).await {
        Ok(true) => tracing::info!(
            target: Action::Relay.target(),
            "main cluster: {} gateway(s) on primary links",
            rows.len(),
        ),
        Ok(false) => {}
        Err(e) => tracing::warn!(target: Action::Relay.target(), "{}: {e}", path.display()),
    }
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
    market.check_mode().await?;
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
    let mut view = market.account(&req.appliance_id, &sharing).await?;
    if let (Some(onramp), Some(usdc)) = (&st.onramp, view.usdc.as_mut()) {
        if !usdc.test
            && usdc.chains.iter().any(|c| c.network == "base")
            && onramp.configured().await
        {
            usdc.onramp = Some("binance");
        }
    }
    Ok(Json(view))
}

#[derive(Debug, Deserialize)]
struct OnrampReq {
    #[serde(flatten)]
    auth: MarketAuth,
    address: String,
    euros: String,
}

#[derive(Debug, Deserialize)]
struct OnrampStatusReq {
    #[serde(flatten)]
    auth: MarketAuth,
    order_id: String,
}

/// The on-ramp, for an authenticated box. Any tenant may buy: the USDC goes
/// to the box's own wallet, not to another tenant.
async fn onramp_of(st: &AppState, auth: &MarketAuth) -> Result<Arc<Onramp>, ApiError> {
    authenticate(st, &auth.appliance_id, &auth.token).await?;
    st.onramp
        .clone()
        .ok_or(ApiError::Onramp(OnrampError::Unconfigured))
}

/// `POST /market/onramp`: a Binance order for `euros` of USDC on Base to
/// `address`, with the checkout link the buyer opens.
async fn onramp_create(
    State(st): State<AppState>,
    Json(req): Json<OnrampReq>,
) -> Result<(StatusCode, Json<OrderView>), ApiError> {
    let onramp = onramp_of(&st, &req.auth).await?;
    let view = onramp
        .create(&req.auth.appliance_id, &req.address, &req.euros)
        .await?;
    Ok((StatusCode::CREATED, Json(view)))
}

/// `POST /market/onramp/status`: where one of the caller's orders is.
async fn onramp_status(
    State(st): State<AppState>,
    Json(req): Json<OnrampStatusReq>,
) -> Result<Json<OrderView>, ApiError> {
    let onramp = onramp_of(&st, &req.auth).await?;
    Ok(Json(
        onramp.status(&req.auth.appliance_id, &req.order_id).await?,
    ))
}

/// `POST /market/onramp/webhook`: Binance pushes an order's new status. The
/// body is verified as it arrived, so it is taken as bytes. Binance wants a
/// 200 with `returnCode` back.
async fn onramp_webhook(
    State(st): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    let onramp = st
        .onramp
        .as_ref()
        .ok_or(ApiError::Onramp(OnrampError::Unconfigured))?;
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .ok_or(ApiError::Onramp(OnrampError::BadSignature))
    };
    onramp
        .webhook(
            header("X-BN-Connect-For")?,
            header("X-BN-Connect-Timestamp")?,
            header("X-BN-Connect-Signature")?,
            &body,
        )
        .await?;
    Ok(Json(serde_json::json!({
        "returnCode": "SUCCESS",
        "returnMessage": null,
    })))
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

/// The seller asks to be paid for every sale whose slot is over.
async fn market_release(
    State(st): State<AppState>,
    Json(auth): Json<MarketAuth>,
) -> Result<Json<crate::slots::PayoutView>, ApiError> {
    let market = market_tenant(&st, &auth).await?;
    let view = market.release_sales(&auth.appliance_id).await?;
    st.notify.notify_one();
    Ok(Json(view))
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

#[derive(Debug, Deserialize)]
struct MarketPayoutReq {
    #[serde(flatten)]
    auth: MarketAuth,
    address: String,
    signature: String,
    /// `evm` when absent, which is what a box built before Solana sends.
    #[serde(default)]
    family: Option<String>,
}

/// `POST /market/payout`: the seller's wallet signs the payout address.
async fn market_payout(
    State(st): State<AppState>,
    Json(req): Json<MarketPayoutReq>,
) -> Result<StatusCode, ApiError> {
    let market = market_tenant(&st, &req.auth).await?;
    let family = match req.family.as_deref() {
        None | Some("evm") => crate::stablecoin::Family::Evm,
        Some("solana") => crate::stablecoin::Family::Solana,
        Some(_) => return Err(MarketError::Invalid("family is evm or solana").into()),
    };
    market
        .set_payout(&req.auth.appliance_id, &req.address, &req.signature, family)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct MarketConfirmReq {
    #[serde(flatten)]
    auth: MarketAuth,
    order_id: String,
    tx: String,
    /// The network `tx` is on; the edge's first when absent.
    #[serde(default)]
    network: Option<String>,
}

/// `POST /market/confirm`: a mined USDC payment for an order this caller made.
async fn market_confirm(
    State(st): State<AppState>,
    Json(req): Json<MarketConfirmReq>,
) -> Result<StatusCode, ApiError> {
    let market = market_tenant(&st, &req.auth).await?;
    let confirmed = market
        .confirm_usdc(
            &req.auth.appliance_id,
            &req.order_id,
            &req.tx,
            req.network.as_deref(),
        )
        .await;
    // A held payment, paid or refused, has an escrow call waiting: the
    // reconciler sends it now rather than on its next tick.
    st.notify.notify_one();
    confirmed?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct MarketCoinKeyReq {
    #[serde(flatten)]
    auth: MarketAuth,
    /// `btc`, `xmr` or `zec`.
    coin: String,
    /// Bitcoin: an extended public key or a ranged output descriptor.
    /// Monero: the wallet's primary address. Zcash: the wallet's unified
    /// viewing key, full (`uview…`) or incoming (`uivk…`).
    key: String,
    /// Monero: the wallet's private view key. Zcash takes none.
    #[serde(default)]
    view_key: Option<String>,
}

/// `POST /market/coins/key`: the watch-only key this seller is paid to in a
/// coin other than USDC.
async fn market_coin_key(
    State(st): State<AppState>,
    Json(req): Json<MarketCoinKeyReq>,
) -> Result<StatusCode, ApiError> {
    let market = market_tenant(&st, &req.auth).await?;
    market
        .set_coin_key(
            &req.auth.appliance_id,
            &req.coin,
            &req.key,
            req.view_key.as_deref(),
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct MarketCoinPayReq {
    #[serde(flatten)]
    auth: MarketAuth,
    order_id: String,
    coin: String,
}

/// `POST /market/coins/pay`: an address and amount to pay this caller's
/// order in a coin, or what has reached it so far.
async fn market_coin_pay(
    State(st): State<AppState>,
    Json(req): Json<MarketCoinPayReq>,
) -> Result<Json<crate::market::CoinView>, ApiError> {
    let market = market_tenant(&st, &req.auth).await?;
    let view = market
        .pay_with_coin(&req.auth.appliance_id, &req.order_id, &req.coin)
        .await?;
    Ok(Json(view))
}

#[derive(Debug, Deserialize)]
struct MarketFeesPayReq {
    #[serde(flatten)]
    auth: MarketAuth,
    /// A USDC transfer from the seller's payout address to the treasury.
    tx: String,
    /// The EVM network it is on; the edge's first when absent.
    #[serde(default)]
    network: Option<String>,
}

/// `POST /market/fees/pay`: the seller settled platform fees its coin sales
/// owe with a USDC transfer to the treasury.
async fn market_fees_pay(
    State(st): State<AppState>,
    Json(req): Json<MarketFeesPayReq>,
) -> Result<Json<crate::market::FeePaymentView>, ApiError> {
    let market = market_tenant(&st, &req.auth).await?;
    let view = market
        .pay_coin_fees(&req.auth.appliance_id, &req.tx, req.network.as_deref())
        .await?;
    Ok(Json(view))
}

#[derive(Debug, Deserialize)]
struct MarketRpcReq {
    #[serde(flatten)]
    auth: MarketAuth,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
    #[serde(default = "rpc_id")]
    id: serde_json::Value,
    /// A network the edge settles on, or `ens` for name lookups; the edge's
    /// first network when absent.
    #[serde(default)]
    network: Option<String>,
}

fn rpc_id() -> serde_json::Value {
    serde_json::json!(1)
}

/// `POST /market/rpc`: one allowlisted chain call. The body the caller gets
/// is the JSON-RPC object, so a wallet can use this as its transport. The
/// RPC URL is not in it.
async fn market_rpc(
    State(st): State<AppState>,
    Json(req): Json<MarketRpcReq>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let market = market_tenant(&st, &req.auth).await?;
    let params = if req.params.is_null() {
        serde_json::json!([])
    } else {
        req.params
    };
    Ok(Json(
        market
            .forward_rpc(req.network.as_deref(), &req.method, &params, &req.id)
            .await?,
    ))
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
/// serves a zone and is a claimed gateway. Whether to trust it is the
/// boxes' decision (they check its passport, TPM and fees, and refuse to
/// relay domain requests to an edge that fails); this is the edge's own half.
fn domains_of(st: &AppState) -> Result<&Arc<Domains>, ApiError> {
    let domains = st
        .domains
        .as_ref()
        .ok_or(ApiError::Domains(DomainError::Unconfigured))?;
    if !claimed_gateway(st) {
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
async fn domains_tenant(st: &AppState, auth: &MarketAuth) -> Result<DomainsTenant, ApiError> {
    let entry = authenticate(st, &auth.appliance_id, &auth.token).await?;
    let store = Arc::clone(domains_of(st)?);
    let vouched = vouched_for(st, &auth.appliance_id).await;
    Ok(DomainsTenant {
        store,
        vouched,
        hostname: entry.hostname,
    })
}

/// An authenticated box on an edge that hands out names.
struct DomainsTenant {
    store: Arc<Domains>,
    vouched: Option<Vouched>,
    /// Its name on this edge.
    hostname: String,
}

impl DomainsTenant {
    fn view(&self, st: &AppState, state: &domains::DomainsState, tenant: &str) -> DomainsView {
        with_pass(
            st,
            tenant,
            domains::view(
                state,
                tenant,
                Some(&self.hostname),
                self.vouched.as_ref(),
                &self.store.opts,
            ),
        )
    }
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
    let tenant = domains_tenant(&st, &req).await?;
    let state = tenant.store.snapshot().await;
    Ok(Json(tenant.view(&st, &state, &req.appliance_id)))
}

async fn domains_add(
    State(st): State<AppState>,
    Json(req): Json<DomainReq>,
) -> Result<(StatusCode, Json<DomainsView>), ApiError> {
    let tenant = domains_tenant(&st, &req.auth).await?;
    let store = &tenant.store;
    if tenant.vouched.is_none() {
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
        Json(tenant.view(&st, &state, &req.auth.appliance_id)),
    ))
}

async fn domains_remove(
    State(st): State<AppState>,
    Json(req): Json<DomainReq>,
) -> Result<Json<DomainsView>, ApiError> {
    let tenant = domains_tenant(&st, &req.auth).await?;
    let store = &tenant.store;
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
    Ok(Json(tenant.view(&st, &state, &req.auth.appliance_id)))
}

/// Body of `POST /domains/wallet`: an address and its wallet's signature of
/// [`domains::wallet_message`] to bind, or neither to unbind.
#[derive(Debug, Deserialize)]
struct WalletReq {
    #[serde(flatten)]
    auth: MarketAuth,
    #[serde(default)]
    address: Option<String>,
    #[serde(default)]
    signature: Option<String>,
}

/// `POST /domains/wallet`: publish the box's wallet at its names in this
/// edge's zone, or take it off. The zone follows at the next reconcile, which
/// this asks for now.
async fn domains_wallet(
    State(st): State<AppState>,
    Json(req): Json<WalletReq>,
) -> Result<Json<DomainsView>, ApiError> {
    let tenant = domains_tenant(&st, &req.auth).await?;
    let id = &req.auth.appliance_id;
    let now = crate::market::now_secs();
    let state = match (&req.address, &req.signature) {
        (Some(address), Some(signature)) => {
            let state = tenant
                .store
                .update(|s| {
                    domains::bind_wallet(s, &tenant.store.opts, id, address, signature, now)
                })
                .await?;
            tracing::info!(
                target: Action::Domains.target(),
                "{id} bound its names to {}",
                state.wallets.get(id.as_str()).map_or("", |w| w.address.as_str()),
            );
            state
        }
        (None, None) => {
            let state = tenant
                .store
                .update(|s| Ok(domains::unbind_wallet(s, id)))
                .await?;
            tracing::info!(target: Action::Domains.target(), "{id} took its wallet off its names");
            state
        }
        _ => {
            return Err(ApiError::Domains(DomainError::Invalid(
                "send the address with its signature, or neither to unbind",
            )))
        }
    };
    st.notify.notify_one();
    Ok(Json(tenant.view(&st, &state, id)))
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
    // Enrolled boxes too: they sign in like listed ones and may bind a wallet.
    let tenants = match all_tenants(st).await {
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
    check_claims(st, store, doh, now).await;
    check_published(st, store, doh, &tenants, now).await;
}

/// The claims half of [`check_domains`].
async fn check_claims(st: &AppState, store: &domains::Domains, doh: &domains::Doh, now: u64) {
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
        let wallet = snapshot
            .wallets
            .get(&claim.tenant)
            .map(|w| w.address.clone());
        // No Stripe data: nothing to look up, the answer is already known.
        let observed = if vouched.is_some() {
            doh.observe(&claim.domain, wallet.is_some()).await
        } else {
            domains::Observed {
                txt: Ok(Vec::new()),
                addrs: Ok(Vec::new()),
                wallet: None,
            }
        };
        results.push((claim, vouched, wallet, observed));
    }
    let outcome = store
        .update(|s| {
            let mut next = s.clone();
            for (claim, vouched, wallet, observed) in &results {
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
                    // The owner rebound (or unbound) while this lookup ran: what
                    // it saw was checked against the old address. Keep the
                    // routing verdict, drop the wallet one; the next pass
                    // looks again for the new address.
                    let current = next.wallets.get(&claim.tenant).map(|w| w.address.clone());
                    let stale = current != *wallet;
                    let mut observed = observed.clone();
                    if stale {
                        observed.wallet = None;
                    }
                    let checked_at = slot.checked_at;
                    *slot = domains::evaluate(
                        slot,
                        vouched.as_ref(),
                        current.as_deref(),
                        &store.opts,
                        taken,
                        &observed,
                        now,
                    );
                    if stale {
                        // Still due, so the new address is looked up next pass.
                        slot.checked_at = checked_at;
                    }
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

/// Ask public DNS for the wallet record at the names of every box with a
/// wallet bound ([`domains::Published`]), at most
/// [`domains::CHECKS_PER_PASS`] boxes a pass. Like the claims, the lookups
/// run with no lock held and are dropped if the wallet changed meanwhile.
async fn check_published(
    st: &AppState,
    store: &domains::Domains,
    doh: &domains::Doh,
    tenants: &HashMap<String, TenantEntry>,
    now: u64,
) {
    let snapshot = store.snapshot().await;
    let mut due = Vec::new();
    for (tenant, wallet) in &snapshot.wallets {
        if due.len() == domains::CHECKS_PER_PASS {
            break;
        }
        let Some(entry) = tenants.get(tenant) else {
            continue;
        };
        let vouched = vouched_for(st, tenant).await;
        let names = domains::box_names(&store.opts, Some(&entry.hostname), vouched.as_ref());
        if !names.is_empty()
            && domains::Published::due(snapshot.published.get(tenant), &wallet.address, &names, now)
        {
            due.push((tenant.clone(), wallet.address.clone(), names));
        }
    }
    if due.is_empty() {
        return;
    }
    let mut results = Vec::with_capacity(due.len());
    for (tenant, address, names) in due {
        let seen = doh.published(&names, &address).await;
        results.push((tenant, address, seen));
    }
    let outcome = store
        .update(|s| {
            let mut next = s.clone();
            for (tenant, address, seen) in &results {
                if next
                    .wallets
                    .get(tenant)
                    .is_none_or(|w| w.address != *address)
                {
                    continue;
                }
                let before = next.published.get(tenant).map(|p| p.names.clone());
                if before.as_ref() != Some(seen) {
                    tracing::info!(
                        target: Action::Domains.target(),
                        "{tenant}'s names in public DNS: {seen:?}",
                    );
                }
                next.published.insert(
                    tenant.clone(),
                    domains::Published {
                        address: address.clone(),
                        checked_at: now,
                        names: seen.clone(),
                    },
                );
            }
            Ok(next)
        })
        .await;
    if let Err(e) = outcome {
        tracing::error!(target: Action::Domains.target(), "recording name checks failed: {e}");
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
/// Machine orders are fulfilled by `vm::fulfil` first, on the same pass.
/// Which of `watched` (sellers with a running, held slot) are online, and
/// whether this pass could tell at all. Online is the seller's node `Ready`
/// in the mesh, which is what serves what they sold; when the mesh apiserver
/// cannot be asked, a heartbeat within the TTL. While the API is shedding
/// load a missed heartbeat proves nothing, so such a pass counts no offline
/// time (`crate::slots::observe`), the same rule the prune keeps.
async fn seller_presence(st: &AppState, watched: &BTreeSet<String>) -> (BTreeSet<String>, bool) {
    if watched.is_empty() {
        return (BTreeSet::new(), true);
    }
    let ttl = st.opts.heartbeat_ttl;
    let shedding = st
        .last_shed
        .lock()
        .map(|last| last.is_some_and(|t| t.elapsed() < ttl))
        .unwrap_or(true);
    let heard = st.reg.alive(ttl).await;
    let kube = if st.opts.kube_token_file.is_some() {
        kube_access(st).await.ok()
    } else {
        None
    };
    let mut online = BTreeSet::new();
    let mut can_tell = !shedding;
    for seller in watched {
        let ready = match &kube {
            Some(kube) if kube_name_fault(seller).is_none() => node_ready(kube, seller).await,
            _ => None,
        };
        match ready {
            Some(true) => {
                online.insert(seller.clone());
            }
            Some(false) => {}
            None => {
                if heard.contains(seller) {
                    online.insert(seller.clone());
                } else if kube.is_some() {
                    // Neither the apiserver nor the box answered: the edge
                    // cannot tell which side is down.
                    can_tell = false;
                }
            }
        }
    }
    (online, can_tell)
}

/// Whether node `name` is `Ready`: `Some(false)` when it is not, or is not
/// in the mesh at all; `None` when the apiserver did not answer.
async fn node_ready(kube: &KubeAccess, name: &str) -> Option<bool> {
    let url = format!("{}/api/v1/nodes/{name}", kube.api);
    let response = kube
        .client
        .get(&url)
        .bearer_auth(&kube.token)
        .send()
        .await
        .ok()?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Some(false);
    }
    if !response.status().is_success() {
        return None;
    }
    let node: serde_json::Value = response.json().await.ok()?;
    Some(node_is_ready(&node))
}

/// A Node object's `Ready` condition is `True`.
fn node_is_ready(node: &serde_json::Value) -> bool {
    node["status"]["conditions"]
        .as_array()
        .is_some_and(|conditions| {
            conditions
                .iter()
                .any(|c| c["type"] == "Ready" && c["status"] == "True")
        })
}

async fn fulfil_market(st: &AppState) {
    let Some(market) = &st.market else {
        return;
    };
    // Which ledger to work on follows the gate's key. Once a request has
    // told, the pass keeps to that ledger: its orders are still owed their
    // volumes even if the key changes before the next request notices.
    if market.mode().await.is_none() && market.check_mode().await.is_err() {
        return;
    }
    // A coin payment seen in time completes its order even after the hold
    // ran out, so read the payments before expiring anything.
    market.settle_coins().await;
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
    let watched = market.watched_sellers().await;
    let (online, can_tell) = seller_presence(st, &watched).await;
    let max_gap = st
        .opts
        .reconcile_interval
        .as_secs()
        .saturating_mul(3)
        .max(300);
    market.slots_pass(&online, can_tell, max_gap).await;
    market.settle_escrow().await;
    if let Some(inference) = &st.inference {
        let lenders = llm::lenders_of(st).await;
        let in_use = market.groups_in_use().await;
        for b in inference.pass(&lenders, |g| in_use.contains(g)).await {
            tracing::warn!(
                target: Action::Market.target(),
                "model group {} stopped: {}",
                b.group,
                b.reason.message(),
            );
            market.end_group(&b.group, b.reason).await;
        }
    }
    market.taps_pass().await;
    market.settle_cards().await;
    vm::fulfil(st, market).await;
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
    ensure_namespace(kube, &p.namespace).await?;
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

/// Create a buyer's `market-<id>` namespace; one that exists is fine.
async fn ensure_namespace(kube: &KubeAccess, namespace: &str) -> Result<(), ApiError> {
    kube_create(
        kube,
        &format!("{}/api/v1/namespaces", kube.api),
        &serde_json::json!({
            "apiVersion": "v1",
            "kind": "Namespace",
            "metadata": {
                "name": namespace,
                "labels": { "losos.market/managed": "true" },
            },
        }),
    )
    .await
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
/// below removed every router and tunnel service on the edge. An attacker
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

    let mut enriched: Vec<TenantView> = Vec::with_capacity(views.len());
    let mut stale: Vec<String> = Vec::new();
    let mut rehome: Vec<(String, String)> = Vec::new();
    for v in &views {
        // Tenants removed from the whitelist are dropped here and pruned from
        // the registry so their tunnel service + Traefik router disappear.
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
            tunnel_port: v.tunnel_port,
            token: token.to_string(),
            llm_port: None,
            tls: v.tls.clone(),
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
            tunnel_port: r.tunnel_port,
            token,
            llm_port: None,
            tls: r.tls,
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

    // The domain half: a name in the edge's zone for each box Stripe vouches
    // for, every proved custom domain, and the zone file itself. Computed
    // before the Traefik routes so both describe the same moment.
    let (extra_hosts, mut zone_names) = domain_hosts(st, &enriched, &tenants).await;
    // On a spoke, the names the hub routes to a box behind it: the box needs
    // a certificate for those too when it ends TLS itself, though this edge
    // routes nothing by them. `record_hub_routes` keeps them; no file is
    // none.
    let mut hub_hosts: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let hub_routes = Path::new(&st.opts.registry_path).with_file_name("hub-routes.json");
    if let Ok(text) = tokio::fs::read(&hub_routes).await {
        let rows: Vec<RelayRoute> = serde_json::from_slice(&text).unwrap_or_default();
        for r in rows {
            hub_hosts.entry(r.id).or_default().push(r.domain);
        }
    }
    // Every public name of each live box, for the heartbeat's answer to a
    // box that ends TLS itself (crate::passthrough), and no HTTPS record at
    // those names: the edge can offer neither ECH nor HTTP/3 there.
    let zone = domains_of(st).ok().map(|store| store.opts.zone.clone());
    let mut box_names: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for t in &enriched {
        let mut names = vec![t.hostname.clone()];
        names.extend(extra_hosts.get(&t.id).into_iter().flatten().cloned());
        let names = crate::passthrough::clean(names);
        if t.tls.is_some() {
            if let Some(zone) = &zone {
                for name in &names {
                    zone_names.skip_https(name, zone);
                }
            }
        }
        let mut told = names;
        told.extend(hub_hosts.get(&t.id).into_iter().flatten().cloned());
        box_names.insert(t.id.clone(), crate::passthrough::clean(told));
    }
    st.discovery.set_names(box_names);
    // A box lending memory to AI models gets its second tunnel service.
    if let Some(inference) = &st.inference {
        let lending: HashSet<String> = llm::lenders_of(st)
            .await
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        for t in &mut enriched {
            if lending.contains(&t.id) {
                t.llm_port = inference.llm_port(t.tunnel_port);
            }
        }
    }
    let files = desired_config_with_hosts(&enriched, &extra_hosts);

    // Traefik polls the provider listener and picks this up within its poll
    // interval; nothing is written for it.
    let changed_traefik = st.discovery.set_tenants(files.traefik.clone());
    let changed_tunnel = write_if_changed(
        Path::new(&st.opts.tunnel_config),
        &files.tunnel_json,
        TUNNEL_FILE_MODE,
    )
    .await
    .with_context(|| format!("write {}", st.opts.tunnel_config))?;

    // Tor and I2P (crate::hidden): never fatal, like the zone below. A box
    // whose address is not made yet is routed on a later tick.
    let changed_hidden = match &st.opts.hidden {
        None => false,
        Some(h) => match reconcile_hidden(h, &st.discovery, &enriched).await {
            Ok(changed) => changed,
            Err(e) => {
                tracing::error!(target: Action::Reconcile.target(), "Tor and I2P routes: {e:?}");
                false
            }
        },
    };

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
        || changed_tunnel
        || changed_hidden
        || dropped_windows
        || changed_windows
        || changed_zone
    {
        tracing::info!(
            target: Action::Reconcile.target(),
            "reconciled {} tenant(s), {} compute window(s); traefik={} tunnel={} windows={}",
            enriched.len(),
            windows.len(),
            changed_traefik,
            changed_tunnel,
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
    let mut names = ZoneNames {
        ech: st.ech.as_ref().map(|k| k.config_list_base64()),
        ..ZoneNames::default()
    };
    let Ok(store) = domains_of(st) else {
        return (hosts, names);
    };
    let zone = &store.opts.zone;
    let state = store.snapshot().await;
    // The wallet record of a box that bound one, at each of its names.
    let record = |tenant: &str| {
        state
            .wallets
            .get(tenant)
            .map(|w| domains::ens_record(&w.address))
    };
    // Tenant hostnames inside the zone: every whitelisted one, live or not,
    // so a box that reboots keeps its name and only its router comes and goes.
    for (tenant, entry) in tenants.iter() {
        names.add_fqdn(&entry.hostname, zone);
        if let Some(r) = record(tenant) {
            names.add_txt(&entry.hostname, zone, &r);
        }
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
                if let Some(r) = record(&tenant) {
                    names.add_txt(&target, zone, &r);
                }
                hosts.entry(tenant).or_default().push(target);
            }
        }
    }
    for (tenant, domains) in domains::live_routes(&state) {
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

/// The secondary cluster this gateway belongs to, from the runtime file
/// the manage page writes when a cluster is started or joined. `None` when
/// there is no file, and also when it does not parse or fails
/// [`ClusterFile::fault`], which is logged: a broken file must not sync
/// with anyone.
async fn read_cluster_file(path: &str) -> Option<ClusterFile> {
    let bytes = match tokio::fs::read(path).await {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            tracing::warn!(target: Action::Secondary.target(), "read {path}: {e}");
            return None;
        }
    };
    match serde_json::from_slice::<ClusterFile>(&bytes) {
        Ok(file) => match file.fault() {
            None => Some(file),
            Some(fault) => {
                tracing::warn!(target: Action::Secondary.target(), "{path}: {fault}; not syncing");
                None
            }
        },
        Err(e) => {
            tracing::warn!(target: Action::Secondary.target(), "{path}: {e}; not syncing");
            None
        }
    }
}

/// This gateway's side of a sync: every box enrolled here with its token,
/// every box live here with how long ago it was heard, and the peers this
/// gateway knows.
async fn own_sync_msg(st: &AppState, sec: &Secondary, file: &ClusterFile) -> Result<SyncMsg> {
    let ages = st.reg.export().await.tenants;
    let tenants = all_tenants(st).await?;
    let mut boxes: Vec<secondary::SyncBox> = Vec::new();
    for (id, entry) in tenants.iter() {
        let seen_ago_secs = ages
            .get(id)
            .filter(|t| t.via.is_none())
            .map(|t| t.seen_ago_secs);
        let token = match &st.enrolment {
            Some(e) if e.owns(Path::new(&entry.token_file)) => {
                e.token(id).await.into_diagnostic()?.unwrap_or_default()
            }
            _ => String::new(),
        };
        if token.is_empty() && seen_ago_secs.is_none() {
            continue;
        }
        boxes.push(secondary::SyncBox {
            id: id.clone(),
            hostname: entry.hostname.trim().to_string(),
            token,
            seen_ago_secs,
        });
    }
    boxes.sort_by(|a, b| a.id.cmp(&b.id));
    boxes.truncate(relay::MAX_RELAYED);
    Ok(SyncMsg {
        cluster: file.name.clone(),
        member: file.member.clone(),
        url: sec.opts.self_url.clone(),
        tunnel: sec.opts.self_tunnel.clone(),
        at: crate::market::now_secs(),
        boxes,
        peers: sec.peers.urls(file, &sec.opts.self_url),
    })
}

/// Take a peer's message: note when it heard which box, learn the peers it
/// knows, and enrol here every box it enrolled that this gateway does not
/// know yet, with the box's own token. A box this gateway enrolled with a
/// different token keeps ours, and the clash is logged once.
async fn take_peer_msg(st: &AppState, sec: &Secondary, msg: &SyncMsg) {
    sec.peers
        .record(msg, &sec.opts.self_url, std::time::Instant::now());
    let Some(enrolment) = &st.enrolment else {
        return;
    };
    let Ok(tenants) = st.tenants.load(&st.opts.tenants_file).await else {
        return;
    };
    for b in msg.boxes.iter().filter(|b| secondary::enrollable(b)) {
        let token = b.token.trim();
        if token_fault(token).is_some() {
            continue;
        }
        let known = match lookup_tenant(st, &tenants, &b.id).await {
            Ok(known) => known,
            Err(e) => {
                tracing::warn!(target: Action::Secondary.target(), "look up {}: {e}", b.id);
                continue;
            }
        };
        match known {
            None => {
                if let Err(e) = enrolment
                    .adopt(&b.id, &b.hostname, token, &msg.member)
                    .await
                {
                    tracing::warn!(target: Action::Secondary.target(), "enrol {} from {}: {e}", b.id, msg.member);
                }
            }
            Some(entry) if enrolment.owns(Path::new(&entry.token_file)) => {
                let ours = enrolment
                    .token(&b.id)
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                if !ct_eq(&ours, token)
                    && sec
                        .conflicts
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .insert(b.id.clone())
                {
                    tracing::warn!(
                        target: Action::Secondary.target(),
                        "{} enrolled {} with another token than this gateway did; keeping this gateway's (forget {} on one of their manage pages)",
                        msg.member,
                        b.id,
                        b.id,
                    );
                }
            }
            // A whitelist row is the operator's, never the cluster's.
            Some(_) => {}
        }
    }
}

/// `GET /cluster/secondary`: what this edge is to a box (a gateway or a hub,
/// and so which tier the box's link is), the secondary cluster it belongs
/// to and its gateways that answer, and whether it holds a primary link.
/// Public: names and URLs a LAN advert already shows, nothing else.
async fn secondary_info(State(st): State<AppState>) -> Json<secondary::Info> {
    let file = match &st.secondary {
        Some(sec) => read_cluster_file(&sec.opts.file).await,
        None => None,
    };
    let role = if st.enrolment.is_some() || file.is_some() {
        secondary::Role::Gateway
    } else {
        secondary::Role::Hub
    };
    let main = match &st.opts.uplink {
        Some(up) => tokio::fs::metadata(&up.file).await.is_ok(),
        None => false,
    };
    let mut info = secondary::Info {
        role,
        tier: Some(role.box_tier()),
        main,
        official_cluster: st.opts.official_cluster.clone(),
        officials: st.opts.official_peers.clone(),
        ..secondary::Info::default()
    };
    if let (Some(sec), Some(file)) = (&st.secondary, file) {
        info.gateways.push(secondary::GatewayView {
            member: file.member.clone(),
            url: sec.opts.self_url.clone(),
            tunnel: sec.opts.self_tunnel.clone(),
        });
        info.gateways.extend(
            sec.peers
                .alive(sec.opts.interval * 3, std::time::Instant::now()),
        );
        info.cluster = Some(file.name);
        info.member = Some(file.member);
    }
    Json(info)
}

/// `POST /cluster/secondary/sync`: a peer gateway's half of a sync. The MAC
/// over the body under the cluster secret is the authentication; the answer
/// is this gateway's half, signed the same way.
async fn secondary_sync(State(st): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(sec) = st.secondary.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(file) = read_cluster_file(&sec.opts.file).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mac = headers
        .get(secondary::MAC_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if !secondary::verify_mac(&file.secret, &body, mac) {
        tracing::warn!(target: Action::Secondary.target(), "refused a sync that is not signed with this cluster's secret");
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let msg: SyncMsg = match serde_json::from_slice(&body) {
        Ok(m) => m,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };
    if let Some(fault) = msg.fault(&file, crate::market::now_secs()) {
        tracing::warn!(target: Action::Secondary.target(), "refused a sync from {}: {fault}", msg.member);
        return (StatusCode::BAD_REQUEST, fault).into_response();
    }
    take_peer_msg(&st, &sec, &msg).await;
    let answer = match own_sync_msg(&st, &sec, &file).await {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!(target: Action::Secondary.target(), "answer {}: {e}", msg.member);
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
    };
    let bytes = serde_json::to_vec(&answer).expect("sync message serializes");
    (
        [
            (secondary::MAC_HEADER, secondary::mac(&file.secret, &bytes)),
            ("content-type", "application/json".to_string()),
        ],
        bytes,
    )
        .into_response()
}

/// The secondary cluster's sync: every `interval`, read the cluster file and
/// exchange a [`SyncMsg`] with each peer. A peer that does not answer is
/// tried again next pass; its last word stands until then, so the boxes it
/// held stay its own until one of them is heard here instead.
async fn secondary_loop(st: AppState) {
    let Some(sec) = st.secondary.clone() else {
        return;
    };
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(3))
        .user_agent(concat!("losos-registrar/", env!("CARGO_PKG_VERSION")))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(target: Action::Secondary.target(), "cannot build the sync client: {e}");
            return;
        }
    };
    loop {
        if let Err(e) = secondary_once(&st, &sec, &client).await {
            tracing::warn!(target: Action::Secondary.target(), "sync pass failed: {e}");
        }
        tokio::time::sleep(sec.opts.interval).await;
    }
}

async fn secondary_once(st: &AppState, sec: &Secondary, client: &reqwest::Client) -> Result<()> {
    let Some(file) = read_cluster_file(&sec.opts.file).await else {
        sec.peers.clear();
        return Ok(());
    };
    let own = own_sync_msg(st, sec, &file).await?;
    let body = serde_json::to_vec(&own).into_diagnostic()?;
    let mac = secondary::mac(&file.secret, &body);
    for url in &own.peers {
        let target = format!("{}{}", url.trim_end_matches('/'), secondary::SYNC_ROUTE);
        let reply = client
            .post(&target)
            .header(secondary::MAC_HEADER, &mac)
            .header("content-type", "application/json")
            .body(body.clone())
            .send()
            .await;
        let reply = match reply {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                tracing::info!(target: Action::Secondary.target(), "peer {url} answered {}", r.status());
                continue;
            }
            Err(e) => {
                tracing::debug!(target: Action::Secondary.target(), "peer {url}: {e}");
                continue;
            }
        };
        let their_mac = reply
            .headers()
            .get(secondary::MAC_HEADER)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let Ok(bytes) = reply.bytes().await else {
            continue;
        };
        if !secondary::verify_mac(&file.secret, &bytes, &their_mac) {
            tracing::warn!(target: Action::Secondary.target(), "peer {url} answered without this cluster's secret");
            continue;
        }
        match serde_json::from_slice::<SyncMsg>(&bytes) {
            Ok(msg) => match msg.fault(&file, crate::market::now_secs()) {
                None => take_peer_msg(st, sec, &msg).await,
                Some(fault) => {
                    tracing::warn!(target: Action::Secondary.target(), "peer {url}: {fault}")
                }
            },
            Err(e) => tracing::warn!(target: Action::Secondary.target(), "peer {url}: {e}"),
        }
    }
    Ok(())
}

/// The spoke's uplink (crate::relay): every `interval`, and whenever the
/// reconciler was kicked, read the uplink file, tell the hub which boxes are
/// live here, and rewrite the uplink's tunnel client file.
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
/// The hub the uplink points at, as it described itself last.
#[derive(Debug, Clone)]
struct HubView {
    url: String,
    cluster: Option<String>,
    officials: Vec<official::OfficialPeer>,
}

/// Ask the hub what it is (`GET /cluster/secondary`) and keep its official
/// cluster and peers. A hub that does not answer keeps what it said before:
/// a banned hub may well stop answering, and its list is the way out.
async fn learn_hub(st: &AppState, client: &reqwest::Client, url: &str) {
    let answer = client
        .get(format!("{url}/cluster/secondary"))
        .timeout(Duration::from_secs(5))
        .send()
        .await;
    let Ok(resp) = answer else { return };
    let Ok(info) = resp.json::<secondary::Info>().await else {
        return;
    };
    let view = HubView {
        url: url.to_string(),
        cluster: info.official_cluster.filter(|c| crate::relay::dns_label(c)),
        officials: info
            .officials
            .into_iter()
            .filter(official::OfficialPeer::well_formed)
            .take(official::MAX_OFFICIALS)
            .collect(),
    };
    *st.hub
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(view);
}

/// Move the uplink off a banned hub. With the hub (or its whole official
/// cluster) on the ban list, the uplink goes to the official edge the hub
/// listed that [`official::pick`] gives this gateway outside every ban, and
/// the uplink file says so from then on. The site's token is the same on
/// every official edge, so nothing else changes; the tunnel pin is dropped so
/// the new hub's certificate is pinned afresh. With no such edge known the uplink
/// stays where it is and says why.
async fn leave_banned_hub(
    st: &AppState,
    up: &relay::UplinkOpts,
    client: &reqwest::Client,
    target: UplinkFile,
) -> UplinkFile {
    learn_hub(st, client, &target.registrar_url).await;
    let bans = official::Bans::read(st.opts.bans_file.as_deref()).await;
    let view = st
        .hub
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let hub_cluster = view
        .as_ref()
        .filter(|v| v.url == target.registrar_url)
        .and_then(|v| v.cluster.clone());
    if !bans.banned(&target.registrar_url, hub_cluster.as_deref()) {
        return target;
    }
    let officials = view.map(|v| v.officials).unwrap_or_default();
    let member = match &st.secondary {
        Some(sec) => read_cluster_file(&sec.opts.file)
            .await
            .map(|f| f.member)
            .unwrap_or_default(),
        None => String::new(),
    };
    let seed = format!("{}:{member}", target.id);
    let Some(next) = official::pick(&officials, &seed, None, &bans) else {
        tracing::warn!(
            target: Action::Uplink.target(),
            "the hub {} is banned and it listed no official edge outside the ban; the uplink stays",
            target.registrar_url,
        );
        return target;
    };
    let mut moved = target.clone();
    moved.registrar_url = next.url.clone();
    moved.tunnel_endpoint = next.tunnel.clone();
    if let Err(e) = remove_if_exists(Path::new(&target.pin_file)).await {
        tracing::warn!(target: Action::Uplink.target(), "{e:?}");
    }
    let mut text = serde_json::to_string_pretty(&moved).expect("uplink serializes");
    text.push('\n');
    if let Err(e) = crate::fsutil::atomic_write(Path::new(&up.file), text.as_bytes(), 0o600).await {
        tracing::warn!(
            target: Action::Uplink.target(),
            "could not write the moved uplink to {}: {e}",
            up.file,
        );
    }
    tracing::info!(
        target: Action::Uplink.target(),
        "the hub {} is banned{}; the uplink moves to {} (official cluster {})",
        target.registrar_url,
        hub_cluster.map_or(String::new(), |c| format!(" (official cluster {c})")),
        next.url,
        next.cluster,
    );
    moved
}

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
            if remove_if_exists(Path::new(&up.tunnel_config)).await? {
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
    let target = leave_banned_hub(st, up, client, target).await;
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
    // What is live here: the direct tenants whose token file reads, i.e.
    // exactly the set the local reconcile rendered. Hostnames come from the
    // whitelist (or the enrolled file), the same authority the local Traefik
    // router uses, so the hub sees the names this edge actually serves.
    let tenants = all_tenants(st).await?;
    // With other gateways of the same secondary cluster on the LAN, only the
    // boxes this one heard last are its to relay (`secondary::Peers::holds`);
    // the rest are a peer's, and listing them too would have the hub's route
    // for a box flip between the two uplinks.
    let cluster = match &st.secondary {
        Some(sec) => read_cluster_file(&sec.opts.file)
            .await
            .map(|file| (Arc::clone(sec), file)),
        None => None,
    };
    let ages = st.reg.export().await.tenants;
    let now = std::time::Instant::now();
    let mut listed: Vec<relay::RelayTenant> = Vec::new();
    let mut services: Vec<UplinkService> = Vec::new();
    for v in st.reg.views().await {
        let Some(entry) = tenants.get(&v.id) else {
            continue;
        };
        if let Some((sec, file)) = &cluster {
            let ago = ages.get(&v.id).map_or(0, |t| t.seen_ago_secs);
            let mine = now.checked_sub(Duration::from_secs(ago)).unwrap_or(now);
            if !sec.peers.holds(&file.member, &v.id, mine) {
                continue;
            }
        }
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
            tls: v.tls.clone(),
        });
        services.push(UplinkService {
            name: format!("{}.{}", target.id, v.id),
            local_port: v.tunnel_port,
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
        gateway: cluster.as_ref().map(|(_, file)| file.member.clone()),
        cluster: cluster.as_ref().map(|(_, file)| file.name.clone()),
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
            write_main_cluster(st, &resp.members).await;
            resp.accepted.into_iter().map(|a| a.id).collect()
        }
        Ok(r) => {
            return Err(miette!("hub {url} answered {}", r.status()));
        }
        Err(e) => return Err(miette!("hub {url}: {e}")),
    };
    // Only what the hub accepted gets a client service: a service the hub
    // has no matching service for is refused on every reconnect, for a box
    // the hub told us it will not route anyway.
    let prefix = format!("{}.", target.id);
    services.retain(|s| {
        s.name
            .strip_prefix(&prefix)
            .is_some_and(|id| accepted.contains(id))
    });
    let Some(pin) = pin_hub_tunnel(client, &target.registrar_url, &target.pin_file).await else {
        return Err(miette!(
            "no pin for the hub's tunnel yet; {} stays as it is",
            up.tunnel_config
        ));
    };
    let rendered = uplink_config(
        &UplinkTarget {
            tunnel_endpoint: target.tunnel_endpoint.clone(),
            pin,
            token,
        },
        &services,
    );
    if write_if_changed(Path::new(&up.tunnel_config), &rendered, TUNNEL_FILE_MODE).await? {
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
    match write_if_changed(&path, &text, PUBLIC_FILE_MODE).await {
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

/// The pin of the hub's tunnel for the uplink: the pinned file when it
/// exists, else one fetch of the hub's `/tunnel-pin`, written to the file
/// (0600, atomically) so every later pass reads the same bytes. Any failure
/// is logged and the uplink waits for the next pass rather than rendering a
/// file that would connect unpinned.
async fn pin_hub_tunnel(
    client: &reqwest::Client,
    registrar_url: &str,
    path: &str,
) -> Option<String> {
    match tokio::fs::read_to_string(path).await {
        Ok(k) => {
            let k = k.trim().to_string();
            if crate::tunnel::parse_pin(&k).is_some() {
                return Some(k);
            }
            tracing::warn!(target: Action::Uplink.target(), "{path} does not hold a pin");
            return None;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            tracing::warn!(target: Action::Uplink.target(), "read {path}: {e}");
            return None;
        }
    }
    let url = format!("{}/tunnel-pin", registrar_url.trim_end_matches('/'));
    let pin = match client.get(&url).send().await {
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
    if crate::tunnel::parse_pin(&pin).is_none() {
        tracing::warn!(target: Action::Uplink.target(), "{url}: not a pin");
        return None;
    }
    if let Some(parent) = Path::new(path).parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    match crate::fsutil::atomic_write(Path::new(path), pin.as_bytes(), 0o600).await {
        Ok(()) => {
            tracing::info!(
                target: Action::Uplink.target(),
                "pinned the hub's tunnel from {url} into {path}",
            );
            Some(pin)
        }
        Err(e) => {
            tracing::warn!(target: Action::Uplink.target(), "write {path}: {e}");
            None
        }
    }
}

/// The live boxes for `losos-hidden-sync`, and Traefik's routes for the
/// addresses it made (`crate::hidden`). Whether either changed.
async fn reconcile_hidden(
    h: &crate::hidden::HiddenOpts,
    discovery: &Discovery,
    tenants: &[TenantView],
) -> Result<bool> {
    let ids: Vec<&str> = tenants.iter().map(|t| t.id.as_str()).collect();
    let services = Path::new(&h.dir).join(crate::hidden::SERVICES_FILE);
    let changed_services = write_if_changed(
        &services,
        &crate::hidden::services_json(&ids, h),
        HIDDEN_SERVICES_FILE_MODE,
    )
    .await
    .with_context(|| format!("write {}", services.display()))?;
    let addresses = crate::hidden::read_addresses(Path::new(&h.addresses_file)).await;
    let routes: Vec<crate::config::HiddenRoute> = tenants
        .iter()
        .filter_map(|t| {
            addresses.get(&t.id).map(|a| crate::config::HiddenRoute {
                id: t.id.clone(),
                tunnel_port: t.tunnel_port,
                hosts: a.hosts(),
            })
        })
        .collect();
    let changed_routes = discovery.set_hidden(crate::config::hidden_routes(&routes));
    Ok(changed_services || changed_routes)
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
/// if it was already absent.
async fn remove_if_exists(path: &Path) -> Result<bool> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e)
            .into_diagnostic()
            .with_context(|| format!("remove {}", path.display())),
    }
}
