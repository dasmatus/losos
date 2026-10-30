//! The loopback admin HTTP API.
//!
//! Bound to `127.0.0.1` only; Nginx proxies `/api/*` here for the admin UI.
//! A call is made by one of two principals ([`Principal`]):
//!
//!   * **a person**, signed in by Authelia (`modules/identity.nix`). nginx
//!     asks Authelia about every `/api/` request and passes the answer on as
//!     `Remote-User`, overwriting whatever the client sent; lososd looks the
//!     name up in the people file (`crate::people`) and lets the person
//!     through to the sections their groups open (`crate::sections`);
//!   * **the spare key**, `Authorization: Bearer <token>` with the file at
//!     `$LOSOS_ADMIN_TOKEN_FILE`. It opens everything, and nginx lets it past
//!     Authelia so the box can still be run when Authelia cannot;
//!   * **the LosOS companion** in someone's AI tools, `Authorization: Bearer`
//!     with an access token LosOS Login issued to it (`crate::companion`).
//!     It reads a few routes as the person who approved it, and nothing more.
//!
//! The public routes in `backend/sections.json` take neither: health, the
//! first-run claim and its state read, and the terminal's two routes, whose
//! envelope carries its own proof.

use crate::guard::{retry_after_secs, Audit, Throttle};
use crate::io_backend::{atomic_write_secret, IoLosos};
use crate::losos::{
    cmd_apply, cmd_apps_search, cmd_change, cmd_config, cmd_config_sync, cmd_edge,
    cmd_factory_reset, cmd_grow, cmd_options, cmd_recovery, cmd_set_password, cmd_settings,
    cmd_state, cmd_status, cmd_storage,
};
use crate::model::Mode;
use crate::overrides::validate_apply;
use actix_web::http::StatusCode;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use anyhow::Context;
use std::path::Path;
use std::time::Instant;

/// Default loopback port; overridden by `$LOSOS_ADMIN_PORT`.
const DEFAULT_PORT: u16 = 8082;
/// Default token location; overridden by `$LOSOS_ADMIN_TOKEN_FILE`.
const DEFAULT_TOKEN_FILE: &str = "/var/secrets/losos-admin-token";
/// Bytes of entropy behind the admin token. Hex-encoded, this is the 64
/// characters the VM test asserts on.
const TOKEN_BYTES: usize = 32;
/// Length of the hex-encoded token, and the only length this daemon accepts.
const TOKEN_HEX_LEN: usize = TOKEN_BYTES * 2;

/// Most handlers run their command on the worker's own thread, under the
/// state lock, so a slow command (crictl, vgs, a call to the edge) holds that
/// worker. actix starts one worker per core, a single one on a small box, and
/// answers 408 to any request whose head it has not read within five
/// seconds. A pane fires a dozen reads at once, so one slow command turned
/// the rest into 408s. More workers keep the reads from queueing behind it,
/// and a longer head timeout keeps a queued one from failing: only nginx
/// reaches this loopback port, and it gives up on slow clients itself.
const HTTP_WORKERS: usize = 8;
const HEAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Shared handler state.
struct Api {
    backend: IoLosos,
    token: String,
    throttle: Throttle,
    /// Wrong passwords at device setup, per address (`post_device`).
    passwords: Throttle,
    audit: Audit,
    /// The last claim answered, so a reply lost to a proxy timeout can be
    /// given again to the same password (`crate::receipt`).
    claims: std::sync::Mutex<crate::receipt::Receipts>,
    /// The terminal client's keys and tokens (`crate::access`).
    access: crate::access::Access,
    /// This listener's own port, which a terminal call is run against.
    port: u16,
    /// The widget builder (`crate::builder`).
    builder: crate::builder::Builder,
    /// The people Authelia signs in (`crate::people`).
    people: crate::people::People,
    /// LosOS Login's word on companion tokens (`crate::companion`).
    companion: crate::companion::Companion,
    /// LosOS Desktop computers joined to this box (`crate::directory`).
    directory: crate::directory::Directory,
    /// LosOS Login's rules and second factors (`crate::login`).
    login: crate::login::Login,
    /// LosOS Login names on LosOS Cloud's share (`crate::cloud_share`).
    share: crate::cloud_share::Share,
    /// LosOS Chat's homeserver, as far as lososd runs it (`crate::chat`).
    chat: crate::chat::Chat,
    /// The chat rooms lososd made for LosOS Git repositories
    /// (`crate::git_rooms`).
    git_rooms: crate::git_rooms::Rooms,
    /// LosOS Cloud runs here, and with it LosOS Contacts' address books
    /// and LosOS Calendar (`LOSOS_CLOUD_CONTACTS`, `modules/cloud.nix`).
    contacts: bool,
    /// LosOS Mail (`crate::mail`).
    mail: crate::mail::Mail,
    /// People's OpenPGP keys for LosOS Mail (`crate::mail_pgp`), while mail
    /// is live.
    mail_keys: Option<crate::mail_pgp::Keys>,
    /// The box's credential store (`crate::credentials`).
    credentials: crate::credentials::Credentials,
    /// Each person's picture, banner and the rest of their profile
    /// (`crate::profiles`).
    profiles: crate::profiles::Profiles,
    /// Custom roles (`crate::roles`).
    roles: crate::roles::Presets,
    /// Reports, mutes, suspensions and their log (`crate::moderation`).
    moderation: crate::moderation::Moderation,
    /// Who sees which pictures blurred, and the age marks (`crate::pictures`).
    pictures: crate::pictures::Pictures,
    /// Turns at decoding an upload (`IMAGE_JOBS`).
    image_jobs: tokio::sync::Semaphore,
    /// The plugins that answer every route not registered below
    /// (`crate::plugin`).
    plugins: crate::plugin::Runtime,
    /// The owner's schizo mode and the computers it reaches
    /// (`crate::schizo`).
    schizo: crate::schizo::Schizo,
}

/// Who made a call.
pub(crate) enum Principal {
    /// The admin token: the claim's reply, the printed spare key, or this
    /// daemon replaying a terminal call made with a spare-key token.
    Spare,
    /// Someone Authelia signed in, or a terminal call made with their token.
    Person(crate::people::Person),
    /// The company edge the owner let manage this box, through the box's
    /// own tunnel (`crate::edge::manager_active`). It may use the sections
    /// `sections.json` lists under `edge`, and nothing admin.
    Edge,
    /// The LosOS companion in someone's AI tools, as the person who signed
    /// it in: GET on `losos_access::COMPANION_ROUTES` only, and within
    /// those what the person may read.
    Companion(crate::people::Person),
    /// A computer's backups, as the person LosOS Login signed them in as
    /// (`losos_access::BACKUPS_CLIENT_ID`): `POST /api/devices/backups`
    /// only, which makes that person's own credentials and nobody else's.
    Backups(crate::people::Person),
    /// A LosOS Desktop computer joined to this box, reading the people list
    /// (`crate::directory`): `GET /api/directory/people` only. Nobody.
    Directory(crate::directory::Computer),
}

impl Principal {
    fn audit_name(&self) -> &str {
        match self {
            Principal::Spare => "spare-key",
            Principal::Person(p) | Principal::Companion(p) | Principal::Backups(p) => &p.name,
            Principal::Edge => "edge",
            Principal::Directory(c) => &c.device,
        }
    }

    fn is_admin(&self) -> bool {
        match self {
            Principal::Spare => true,
            Principal::Person(p) => p.is_admin(),
            Principal::Edge
            | Principal::Companion(_)
            | Principal::Backups(_)
            | Principal::Directory(_) => false,
        }
    }

    fn person(&self) -> Option<&crate::people::Person> {
        match self {
            Principal::Spare | Principal::Edge | Principal::Directory(_) => None,
            Principal::Person(p) | Principal::Companion(p) | Principal::Backups(p) => Some(p),
        }
    }
}

/// `{"error": "..."}` at the given status — the shape the SPA expects.
fn err(status: actix_web::http::StatusCode, msg: &str) -> HttpResponse {
    HttpResponse::build(status).json(serde_json::json!({ "error": msg }))
}

/// Run a command, or report it as a 500.
///
/// The client is told only that the command failed. The context chain names
/// filesystem paths and half the appliance's layout, and it goes to the journal
/// instead — the operator can read that, an HTTP caller has no business with it.
fn run(
    api: &Api,
    f: impl FnOnce(&mut IoLosos) -> anyhow::Result<serde_json::Value>,
) -> HttpResponse {
    match api.backend.serialized(f) {
        Ok(v) => HttpResponse::Ok().json(v),
        Err(e) => error_response(e),
    }
}

/// The answer to a command that failed: a typed refusal keeps its words and
/// its status, anything else is a 500 whose detail goes to the journal.
fn error_response(e: anyhow::Error) -> HttpResponse {
    // A market answer the owner can act on keeps its words; it is the
    // registrar's own sentence, vetted by `market::classify`.
    // A change to the people the Users pane cannot make: who and why.
    if let Some(r) = e.downcast_ref::<crate::people::Refused>() {
        return err(
            StatusCode::from_u16(r.status()).unwrap_or(StatusCode::CONFLICT),
            &r.to_string(),
        );
    }
    if let Some(r) = e.downcast_ref::<crate::schizo::Refused>() {
        return err(
            StatusCode::from_u16(r.status()).unwrap_or(StatusCode::BAD_REQUEST),
            &r.to_string(),
        );
    }
    if let Some(r) = e.downcast_ref::<crate::moderation::Refused>() {
        return err(
            StatusCode::from_u16(r.status).unwrap_or(StatusCode::BAD_REQUEST),
            &r.message,
        );
    }
    if let Some(r) = e.downcast_ref::<crate::market::Refused>() {
        return err(
            StatusCode::from_u16(r.status).unwrap_or(StatusCode::BAD_REQUEST),
            &r.message,
        );
    }
    // Sentences the owner can act on, as 400s: an apply the option document
    // turned down (the pane puts it beside the field), a look request that is
    // not one, an install with a bad name or a chart the box will not run, a
    // bucket or recovery code that is not one.
    if let Some(why) = e
        .downcast_ref::<crate::options::Rejected>()
        .map(ToString::to_string)
        .or_else(|| {
            e.downcast_ref::<crate::apps::Invalid>()
                .map(ToString::to_string)
        })
        .or_else(|| {
            e.downcast_ref::<crate::pins::Invalid>()
                .map(ToString::to_string)
        })
        .or_else(|| {
            e.downcast_ref::<crate::user_apps::Invalid>()
                .map(ToString::to_string)
        })
        .or_else(|| {
            e.downcast_ref::<crate::backup::Invalid>()
                .map(ToString::to_string)
        })
        .or_else(|| {
            e.downcast_ref::<crate::people::BadDetails>()
                .map(ToString::to_string)
        })
    {
        return err(StatusCode::BAD_REQUEST, &why);
    }
    // Things that cannot happen right now, as 409s: an install while the
    // shared side is locked or a job still runs, a backup, restore or erase
    // that would collide with one running.
    if let Some(why) = e
        .downcast_ref::<crate::apps::Conflict>()
        .map(ToString::to_string)
        .or_else(|| {
            e.downcast_ref::<crate::backup::Busy>()
                .map(ToString::to_string)
        })
    {
        return err(StatusCode::CONFLICT, &why);
    }
    // Something the request needs is still starting (LosOS Cloud, LosOS
    // Git). 503 with the reason and a Retry-After, so the page waits and
    // says why, instead of an opaque 500.
    if let Some(crate::setup::NotReady(why)) = e.downcast_ref() {
        tracing::info!(reason = why.as_str(), "refused: not ready yet");
        return HttpResponse::build(StatusCode::SERVICE_UNAVAILABLE)
            .insert_header(("Retry-After", "10"))
            .json(serde_json::json!({ "error": why, "ready": false, "waitingFor": why }));
    }
    // A password sign-in that LosOS Cloud turned down: 401, which the
    // dialog shows as "try again" and the caller counts as a failure for
    // the per-address throttle, exactly like a wrong token.
    if let Some(why) = e.downcast_ref::<crate::signin::WrongPassword>() {
        tracing::info!("sign-in refused: wrong password");
        return err(StatusCode::UNAUTHORIZED, &why.to_string());
    }
    // Too many wrong passwords from this address; the page says "wait"
    // rather than "wrong".
    if let Some(why) = e.downcast_ref::<crate::signin::Throttled>() {
        tracing::warn!("password refused: this address is waiting out its wrong guesses");
        let mut resp = err(StatusCode::TOO_MANY_REQUESTS, &why.to_string());
        resp.headers_mut().insert(
            actix_web::http::header::RETRY_AFTER,
            "30".parse().expect("digits"),
        );
        return resp;
    }
    // Sharing turned on with no edge proxy in reach (`crate::edge`):
    // 409 with the reason and a flag the Mesh pane keys on, so the switch
    // can say why it did not take rather than "command failed".
    if let Some(why) = e.downcast_ref::<crate::edge::EdgeRequired>() {
        tracing::warn!(setting = why.setting, "refused: no edge proxy reachable");
        return HttpResponse::build(StatusCode::CONFLICT).json(serde_json::json!({
            "error": why.to_string(),
            "edgeRequired": true,
            "setting": why.setting,
        }));
    }
    // Sharing with this network turned on with no local gateway in reach.
    if let Some(why) = e.downcast_ref::<crate::edge::LocalGatewayRequired>() {
        tracing::warn!("refused: no local gateway reachable");
        return HttpResponse::build(StatusCode::CONFLICT).json(serde_json::json!({
            "error": why.to_string(),
            "localGatewayRequired": true,
            "setting": "losos.sharingLocally",
        }));
    }
    // The mesh join on a box the edge cannot enrol (`crate::edge::
    // check_mesh`): the sentence, as a 409 with its own flag, rather than a
    // rebuild that fails on the same rule.
    if let Some(why) = e.downcast_ref::<crate::edge::MeshNotReady>() {
        tracing::warn!(reason = %why, "refused: the mesh join cannot enrol");
        return HttpResponse::build(StatusCode::CONFLICT).json(serde_json::json!({
            "error": why.to_string(),
            "meshNotReady": true,
        }));
    }
    // Trading asked of a box whose edges are all a company's own: the
    // sentence, as a 409, with a flag the Market pane can key on.
    if let Some(why) = e.downcast_ref::<crate::edge::OfficialEdgeRequired>() {
        tracing::warn!("refused: no official edge reachable, trading is off");
        return HttpResponse::build(StatusCode::CONFLICT).json(serde_json::json!({
            "error": why.to_string(),
            "officialEdgeRequired": true,
        }));
    }
    // A move to a secondary cluster with no gateway in reach, or one a
    // custom domain holds the box against.
    if let Some(why) = e.downcast_ref::<crate::edge::ClusterNotInReach>() {
        return err(StatusCode::CONFLICT, &why.to_string());
    }
    if let Some(why) = e.downcast_ref::<crate::edge::HeldByDomain>() {
        return HttpResponse::build(StatusCode::CONFLICT).json(serde_json::json!({
            "error": why.to_string(),
            "heldByDomain": true,
        }));
    }
    // Management switched on with no company edge in use (`crate::edge::
    // manageable`): the sentence, as a 409.
    if let Some(why) = e.downcast_ref::<crate::edge::NotManageable>() {
        return HttpResponse::build(StatusCode::CONFLICT).json(serde_json::json!({
            "error": why.to_string(),
            "notManageable": true,
        }));
    }
    // A second owner, or the first one again after the grace window:
    // the sentence, as a 409 the wizard already knows how to show.
    if let Some(why) = e.downcast_ref::<crate::setup::AlreadyClaimed>() {
        tracing::warn!("claim refused: the box already has an owner");
        return err(StatusCode::CONFLICT, &why.to_string());
    }
    tracing::error!(error = ?e, "admin API command failed");
    err(
        StatusCode::INTERNAL_SERVER_ERROR,
        "command failed; see the lososd journal",
    )
}

/// Compare two secrets without an early exit on the first differing byte.
///
/// The length check does short-circuit, which leaks only the length of what was
/// presented — the token's own length is fixed and public.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// The credentials out of an `Authorization: Bearer <token>` header.
///
/// RFC 7235 makes the scheme case-insensitive, so `bearer` is as valid as
/// `Bearer`; the old exact match on `"Bearer {token}"` rejected it.
fn bearer_credentials(header: &str) -> Option<&str> {
    let (scheme, credentials) = header.split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then(|| credentials.trim_start_matches(' '))
}

/// Who made the request, if anyone this daemon knows.
///
/// The admin token is the spare key, unless it comes with `X-Losos-As`:
/// that is this daemon replaying a terminal call for the person whose token
/// it was ([`crate::access::forward`]), and only a holder of the admin token
/// can say it. Without the token, `Remote-User` names the person nginx had
/// Authelia sign in. That header is trusted because only root and nginx can
/// connect to this port (the owner rule in `modules/daemon.nix`) and nginx
/// sets it on every location that proxies here, empty when nobody signed in.
fn identify(api: &Api, req: &HttpRequest) -> Option<Principal> {
    let header = |name: &str| {
        req.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|v| !v.is_empty())
    };
    let lookup = |name: &str| match api.people.get(name) {
        Ok(found) => found.map(Principal::Person),
        Err(e) => {
            tracing::error!(error = ?e, "the people file could not be read");
            None
        }
    };
    // nginx's `losos-manage.invalid` server, which takes only loopback, where
    // the box's tunnel to its edge lands (modules/proxy.nix), sets
    // this on every request and every other location clears it. Such a
    // request is the edge or nobody: no spare key, no person.
    let from_loopback = req.peer_addr().is_some_and(|a| a.ip().is_loopback());
    if header("x-losos-via-edge").is_some() {
        return (from_loopback
            && edge_token_ok(header("x-losos-edge"))
            && api.backend.edge_manager_active())
        .then_some(Principal::Edge);
    }
    if authorized(api, req) {
        return match header("x-losos-as") {
            Some(name) => lookup(name),
            None => Some(Principal::Spare),
        };
    }
    if let Some(token) = header("authorization").and_then(bearer_credentials) {
        // A joined computer's own token, which only this box made and
        // only lososd checks.
        if crate::directory::is_directory_token(token) {
            return api
                .directory
                .check(token, losos_access::now_secs())
                .map(Principal::Directory);
        }
        // nginx passes a Bearer header on without asking Authelia; a token
        // of LosOS Login's is checked with LosOS Login, at the address the
        // request came to, which is where it was issued.
        if !(from_loopback
            && api.people.headers
            && crate::companion::Companion::is_login_token(token))
        {
            return None;
        }
        let host = header("x-forwarded-host").or_else(|| header("host"))?;
        let grant = api.companion.check(token, host)?;
        return match (lookup(&grant.person)?, grant.client) {
            (Principal::Person(p), crate::companion::Client::Companion) => {
                Some(Principal::Companion(p))
            }
            (Principal::Person(p), crate::companion::Client::Backups) => {
                Some(Principal::Backups(p))
            }
            _ => None,
        };
    }
    match header("remote-user") {
        Some(name) if from_loopback && api.people.headers => lookup(name),
        _ => None,
    }
}

/// Whether `presented` is this box's proxy token (`$LOSOS_PROXY_TOKEN_FILE`),
/// the secret the box enrolled with its edge by: only the box and that edge
/// hold it. Read per call, like the market relay does.
fn edge_token_ok(presented: Option<&str>) -> bool {
    let Some(presented) = presented else {
        return false;
    };
    let Some(file) = std::env::var("LOSOS_PROXY_TOKEN_FILE")
        .ok()
        .filter(|v| !v.is_empty())
    else {
        return false;
    };
    let Ok(token) = std::fs::read_to_string(file) else {
        return false;
    };
    let token = token.trim();
    !token.is_empty() && constant_time_eq(presented.as_bytes(), token.as_bytes())
}

/// Whether the request carries the admin token.
fn authorized(api: &Api, req: &HttpRequest) -> bool {
    req.headers()
        .get(actix_web::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(bearer_credentials)
        .is_some_and(|presented| constant_time_eq(presented.as_bytes(), api.token.as_bytes()))
}

/// The address to attribute a request to.
///
/// The listener is loopback-only, so the peer is always Nginx and the real
/// client is whatever Nginx put in `X-Real-IP` (it overwrites any value the
/// client sent). Anything else — the VM tests, curl on the box — is the peer.
fn remote_addr(req: &HttpRequest) -> String {
    let peer = req.peer_addr().map(|a| a.ip());
    if peer.is_some_and(|ip| ip.is_loopback()) {
        if let Some(ip) = req
            .headers()
            .get("x-real-ip")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<std::net::IpAddr>().ok())
        {
            return ip.to_string();
        }
    }
    peer.map_or_else(|| "unknown".to_string(), |ip| ip.to_string())
}

/// Authenticate, throttle and audit one request, then run `f`.
///
/// Failed authentication is counted per remote address; past a small free
/// budget the address is answered `429` with `Retry-After` for an
/// exponentially growing (capped) window, and a correct token clears the
/// count. `mutating` routes additionally append one audit record per attempt:
/// the outcome of the command, or why the attempt never got that far.
fn guarded(
    api: &Api,
    req: &HttpRequest,
    route: &str,
    mutating: bool,
    f: impl FnOnce() -> HttpResponse,
) -> HttpResponse {
    guarded_as(api, req, route, mutating, |_| f())
}

/// [`guarded`], for a handler that needs to know who called.
///
/// After authentication the route's section (`crate::sections`) is checked
/// against the person's groups: a person outside it gets `403` with the
/// section named, so the page can say what is missing. The spare key passes
/// every section.
fn guarded_as(
    api: &Api,
    req: &HttpRequest,
    route: &str,
    mutating: bool,
    f: impl FnOnce(&Principal) -> HttpResponse,
) -> HttpResponse {
    guarded_at(api, req, route, None, mutating, f)
}

/// [`guarded_as`] for a route actix did not match itself, a plugin's: the
/// section is looked up under `pattern`, the manifest's route.
fn guarded_at(
    api: &Api,
    req: &HttpRequest,
    route: &str,
    pattern: Option<&str>,
    mutating: bool,
    f: impl FnOnce(&Principal) -> HttpResponse,
) -> HttpResponse {
    let who = match gate_at(api, req, route, pattern, mutating) {
        Ok(who) => who,
        Err(refused) => return *refused,
    };
    let resp = f(&who);
    if mutating {
        audit_outcome(api, req, route, &resp, &who);
    }
    resp
}

/// Record a mutating call's outcome by its status.
fn audit_outcome(api: &Api, req: &HttpRequest, route: &str, resp: &HttpResponse, who: &Principal) {
    let status = resp.status();
    let outcome = if status.is_success() {
        "ok"
    } else if status.is_client_error() {
        "rejected"
    } else {
        "error"
    };
    api.audit
        .record_as(route, outcome, &remote_addr(req), Some(who.audit_name()));
}

/// The checks of [`guarded_at`] before the handler runs: the caller, or the
/// answer that refuses them, audited when `mutating`.
fn gate_at(
    api: &Api,
    req: &HttpRequest,
    route: &str,
    pattern: Option<&str>,
    mutating: bool,
) -> Result<Principal, Box<HttpResponse>> {
    let remote = remote_addr(req);
    let audit = |outcome: &str, who: Option<&str>| {
        if mutating {
            api.audit.record_as(route, outcome, &remote, who);
        }
    };
    if let Err(wait) = api.throttle.check(&remote, Instant::now()) {
        audit("throttled", None);
        let mut resp = err(
            StatusCode::TOO_MANY_REQUESTS,
            "too many failed attempts; slow down",
        );
        resp.headers_mut().insert(
            actix_web::http::header::RETRY_AFTER,
            retry_after_secs(wait).to_string().parse().expect("digits"),
        );
        return Err(Box::new(resp));
    }
    let Some(who) = identify(api, req) else {
        // Authelia let someone through whom the people file no longer
        // lists (removed or disabled since they signed in). A 401 would
        // send the page back to Authelia, which would send it straight
        // back here; a 403 stops that loop.
        let named = req
            .headers()
            .get("remote-user")
            .and_then(|v| v.to_str().ok())
            .filter(|v| !v.is_empty());
        if let (true, Some(name)) = (api.people.headers, named) {
            audit("no account", None);
            if let Some(hold) = api.moderation.suspended(name, crate::login::now()) {
                return Err(Box::new(HttpResponse::Forbidden().json(
                    serde_json::json!({
                        "error": "a moderator has suspended your account",
                        "noAccount": true,
                        "suspended": hold,
                    }),
                )));
            }
            return Err(Box::new(HttpResponse::Forbidden().json(
                serde_json::json!({
                    "error": "your account is not on this box any more",
                    "noAccount": true,
                }),
            )));
        }
        api.throttle.record_failure(&remote, Instant::now());
        audit("unauthorized", None);
        return Err(Box::new(err(StatusCode::UNAUTHORIZED, "unauthorized")));
    };
    api.throttle.record_success(&remote);
    let method = req.method().as_str();
    // Schizo mode takes the owner off the market (`crate::schizo`). A read
    // is the market's own "not offered here", which every pane already
    // shows; anything else is refused.
    if crate::schizo::market_route(req.path()) && api.schizo.on() {
        audit("schizo", Some(who.audit_name()));
        return Err(Box::new(if method == "GET" {
            HttpResponse::Ok().json(serde_json::json!({ "available": false, "schizo": true }))
        } else {
            HttpResponse::Forbidden().json(serde_json::json!({
                "error": "schizo mode is on: nothing is bought or sold with this box",
                "schizo": true,
            }))
        }));
    }
    let pattern = pattern
        .map(str::to_string)
        .or_else(|| req.match_pattern())
        .unwrap_or_else(|| req.path().to_string());
    let section = crate::sections::section(method, &pattern);
    if let Principal::Companion(person) = &who {
        let asked = match req.query_string() {
            "" => req.path().to_string(),
            q => format!("{}?{q}", req.path()),
        };
        if !losos_access::companion_route(method, &asked) {
            audit("forbidden", Some(&person.name));
            return Err(Box::new(HttpResponse::Forbidden().json(
                serde_json::json!({
                    "error": "the LosOS companion reads files, repositories, mail and the box's health, nothing else",
                    "section": section,
                }),
            )));
        }
    }
    if let Principal::Backups(person) = &who {
        if !losos_access::backups_route(method, req.path()) {
            audit("forbidden", Some(&person.name));
            return Err(Box::new(HttpResponse::Forbidden().json(
                serde_json::json!({
                    "error": "a computer's backups only fetch their own credentials",
                    "section": section,
                }),
            )));
        }
    }
    if let Principal::Directory(computer) = &who {
        if !crate::directory::route_allowed(method, req.path()) {
            audit("forbidden", Some(&computer.device));
            return Err(Box::new(HttpResponse::Forbidden().json(
                serde_json::json!({
                    "error": "a joined computer only reads the people list",
                    "section": section,
                }),
            )));
        }
    }
    if let Principal::Person(person) | Principal::Companion(person) | Principal::Backups(person) =
        &who
    {
        // A rule of LosOS Login holds them to their own account until they
        // comply (`crate::login`): that, and what every signed-in page
        // reads, is all they reach.
        if person.groups.iter().any(|g| g == crate::people::LOCKED)
            && !matches!(section, "account" | "signed-in" | "public")
        {
            audit("locked", Some(&person.name));
            return Err(Box::new(HttpResponse::Forbidden().json(
                serde_json::json!({
                    "error": "finish what LosOS Login asks of your account first",
                    "locked": true,
                    "section": section,
                }),
            )));
        }
        let locked_to_account =
            section == "account" && person.groups.iter().any(|g| g == crate::people::LOCKED);
        if !locked_to_account && !crate::sections::allows(person, method, section) {
            audit("forbidden", Some(&person.name));
            return Err(Box::new(HttpResponse::Forbidden().json(
                serde_json::json!({
                    "error": "your account cannot use this part of the box",
                    "section": section,
                }),
            )));
        }
        if person.groups.iter().any(|g| g == crate::roles::GUESTS)
            && !crate::sections::guest_allows(method, &pattern, section)
        {
            audit("forbidden", Some(&person.name));
            return Err(Box::new(HttpResponse::Forbidden().json(
                serde_json::json!({
                    "error": "guests cannot do this on this box",
                    "guest": true,
                    "section": section,
                }),
            )));
        }
        if crate::sections::speech(method, &pattern) {
            if let Some(hold) = api.moderation.muted(&person.name, crate::login::now()) {
                audit("muted", Some(&person.name));
                return Err(Box::new(HttpResponse::Forbidden().json(
                    serde_json::json!({
                        "error": "a moderator has muted you",
                        "muted": hold,
                    }),
                )));
            }
        }
    }
    if matches!(who, Principal::Edge) && !crate::sections::edge_allows(method, section) {
        audit("forbidden", Some("edge"));
        return Err(Box::new(HttpResponse::Forbidden().json(
            serde_json::json!({
                "error": "the edge may manage only LosOS Cloud, LosOS Git and the apps on this box",
                "section": section,
            }),
        )));
    }
    Ok(who)
}

/// The apps on the shared side that are live on this box now, by name:
/// mail, chat and Mesh CI run only while the box shares its storage, and the admin
/// UI draws their entries only when they are listed here. The spare key
/// and an edge are nobody, so they get none.
fn shared_apps(api: &Api) -> Vec<&'static str> {
    let mut apps = Vec::new();
    if api.mail.open().is_some() {
        apps.push("mail");
    }
    if api.chat.live() {
        apps.push("chat");
    }
    // Not shared-side apps: LosOS Contacts and LosOS Calendar are LosOS
    // Cloud's address books and calendars, on every box that runs LosOS
    // Cloud. Listed here because the sidebar draws the app entries this
    // list names.
    if api.contacts {
        apps.push("contacts");
        apps.push("calendar");
    }
    // modules/ci.nix names the socket only while Mesh CI runs.
    if std::env::var_os("LOSOS_CI_SOCKET").is_some() {
        apps.push("ci");
    }
    apps
}

/// `GET /api/me`: who is signed in, and which sections they can use. The
/// admin UI shows the panes these open and hides the rest.
async fn get_me(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded_as(&api, &req, "/api/me", false, |who| {
        let all = || {
            crate::sections::usable(&crate::people::Person {
                name: String::new(),
                display_name: String::new(),
                groups: vec![crate::people::ADMINS.to_string()],
            })
        };
        match who {
            Principal::Spare => HttpResponse::Ok().json(serde_json::json!({
                "name": null,
                "displayName": null,
                "groups": [],
                "role": "admin",
                "reports": open_reports(&api),
                "admin": true,
                "spareKey": true,
                "sections": all(),
                "standalone": api.people.headers,
            })),
            Principal::Person(p) | Principal::Companion(p) | Principal::Backups(p) => {
                // What holds them now, not at their last request: a page
                // that has just added a passkey asks again to be let go.
                let locked = if api.people.headers {
                    reconcile_login(&api);
                    login_standing(&api, &p.name, api.login.rules(), crate::login::now()).2
                } else {
                    Vec::new()
                };
                let sections = if locked.is_empty() {
                    crate::sections::usable(p)
                } else {
                    vec!["account"]
                };
                let shown = crate::profiles::render(
                    &crate::people::Listed {
                        name: p.name.clone(),
                        display_name: p.display_name.clone(),
                        email: String::new(),
                        groups: p.groups.clone(),
                        disabled: false,
                    },
                    &api.profiles.get(&p.name).unwrap_or_default(),
                );
                HttpResponse::Ok().json(serde_json::json!({
                    "name": p.name,
                    "displayName": p.display_name,
                    "picture": shown["picture"],
                    "accent": shown["accent"],
                    "groups": p.groups,
                    "role": shown["role"],
                    "roleName": role_name(&api, &p.groups),
                    "reports": if sections.contains(&"moderation") {
                        open_reports(&api)
                    } else {
                        0
                    },
                    "admin": who.is_admin() && locked.is_empty(),
                    "spareKey": false,
                    "companion": matches!(who, Principal::Companion(_)),
                    "sections": sections,
                    "standalone": api.people.headers,
                    "locked": locked,
                    "shared": if locked.is_empty() { shared_apps(&api) } else { Vec::new() },
                }))
            }
            Principal::Edge => HttpResponse::Ok().json(serde_json::json!({
                "name": null,
                "displayName": null,
                "groups": [],
                "admin": false,
                "spareKey": false,
                "edge": true,
                "sections": crate::sections::names()
                    .into_iter()
                    .filter(|s| crate::sections::edge_allows("POST", s))
                    .collect::<Vec<_>>(),
                "standalone": api.people.headers,
            })),
            // The gate lets a joined computer read the people list alone.
            Principal::Directory(_) => err(StatusCode::FORBIDDEN, "a joined computer is nobody"),
        }
    })
}

// ── People ────────────────────────────────────────────────────────────────

/* The Users pane: everyone who may sign in to LosOS Login on this box, and
 * what they may do. Only while the box signs people in itself
 * (`People::headers`, `LOSOS_IDENTITY=authelia`); a box whose sign-in runs
 * elsewhere lists nobody and changes nothing here. */

/// The groups the Users pane may hand out: administrator, viewer, guest,
/// one per section of `backend/sections.json`, and one per custom role.
fn grantable_groups(
    presets: &std::collections::BTreeMap<String, crate::roles::Preset>,
) -> Vec<String> {
    let mut all = vec![
        crate::people::ADMINS.to_string(),
        crate::people::VIEWERS.to_string(),
        crate::roles::GUESTS.to_string(),
    ];
    all.extend(
        crate::sections::names()
            .into_iter()
            .map(|s| format!("losos-{s}")),
    );
    all.extend(
        presets
            .keys()
            .map(|id| format!("{}{id}", crate::roles::CUSTOM)),
    );
    all
}

/// `groups` from a request body, each one a group the pane may hand out,
/// as the role they name bundles them (`crate::roles::normalize`).
fn requested_groups(api: &Api, doc: &serde_json::Value) -> Result<Vec<String>, String> {
    let Some(list) = doc.get("groups").and_then(|g| g.as_array()) else {
        return Err("groups must be a list".into());
    };
    let presets = api.roles.all();
    let allowed = grantable_groups(&presets);
    let mut out = Vec::new();
    for g in list {
        let Some(g) = g.as_str() else {
            return Err("groups must be a list of names".into());
        };
        if !allowed.iter().any(|a| a == g) {
            return Err(format!("{g} is not a group this box hands out"));
        }
        if !out.iter().any(|o| o == g) {
            out.push(g.to_string());
        }
    }
    crate::roles::normalize(&out, &presets)
}

/// The name of the custom role `groups` hold, if any.
fn role_name(api: &Api, groups: &[String]) -> Option<String> {
    let id = crate::roles::role_of(groups).custom?;
    api.roles.all().remove(&id).map(|p| p.name)
}

/// How many reports wait for a moderator.
fn open_reports(api: &Api) -> usize {
    api.moderation
        .reports()
        .iter()
        .filter(|r| r.state == crate::moderation::State::Open)
        .count()
}

/// A role as the log says it: "moderator", or a custom role's name.
fn role_words(api: &Api, groups: &[String]) -> String {
    role_name(api, groups)
        .unwrap_or_else(|| crate::roles::role_of(groups).role.as_str().to_string())
}

fn not_standalone() -> HttpResponse {
    HttpResponse::Conflict().json(serde_json::json!({
        "error": "this box does not run its own sign-in",
        "standalone": false,
    }))
}

/// `GET /api/people`: everyone in the people file, and the sections they
/// can be given.
async fn get_people(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded_as(&api, &req, "/api/people", false, |who| {
        let people = if api.people.headers {
            match api.people.list() {
                Ok(p) => p,
                Err(e) => return error_response(e),
            }
        } else {
            Vec::new()
        };
        let rules = api.login.rules();
        let now = crate::login::now();
        let profiles = api.profiles.all().unwrap_or_default();
        let presets = api.roles.all();
        let holds = api.moderation.holds();
        let marks = api.pictures.marks().unwrap_or_default();
        let people: Vec<serde_json::Value> = people
            .into_iter()
            .map(|p| {
                let (factors, set_at, reasons) = login_standing(&api, &p.name, rules, now);
                let mut row = serde_json::to_value(&p).unwrap_or_default();
                row["twoFactor"] = serde_json::json!(factors.any());
                row["passwordSetAt"] = serde_json::json!(set_at);
                row["locked"] = serde_json::json!(reasons);
                let shown = crate::profiles::render(
                    &p,
                    &profiles.get(&p.name).cloned().unwrap_or_default(),
                );
                row["picture"] = shown["picture"].clone();
                row["accent"] = shown["accent"].clone();
                row["banner"] = shown["banner"].clone();
                row["role"] = shown["role"].clone();
                row["roleName"] = serde_json::json!(crate::roles::role_of(&p.groups)
                    .custom
                    .and_then(|id| presets.get(&id).map(|r| r.name.clone())));
                let held = holds.get(&p.name).cloned().unwrap_or_default();
                row["muted"] = serde_json::json!(held
                    .muted
                    .filter(|_| api.moderation.muted(&p.name, now).is_some()));
                row["suspended"] = serde_json::json!(held.suspended);
                row["age"] = serde_json::json!(marks.get(&p.name));
                row
            })
            .collect();
        HttpResponse::Ok().json(serde_json::json!({
            "standalone": api.people.headers,
            "you": who.person().map(|p| p.name.clone()),
            "sections": crate::sections::names(),
            "roles": presets_json(&presets),
            "rules": rules,
            "people": people,
        }))
    })
}

/// `POST /api/people`: add someone. Body `{"name", "displayName",
/// "password", "groups"}`; `email` is optional and defaults to the name at
/// the box's `.local` address, as the owner's does.
async fn post_person(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    const SHAPE: &str = r#"body must be JSON: {"name": "...", "displayName": "...", "password": "...", "groups": [...]}"#;
    guarded(&api, &req, "/api/people", true, || {
        if !api.people.headers {
            return not_standalone();
        }
        let Ok(doc) = serde_json::from_slice::<serde_json::Value>(&body) else {
            return err(StatusCode::BAD_REQUEST, SHAPE);
        };
        let field = |k: &str| doc.get(k).and_then(|v| v.as_str()).map(str::trim);
        let (Some(name), Some(password)) =
            (field("name"), doc.get("password").and_then(|p| p.as_str()))
        else {
            return err(StatusCode::BAD_REQUEST, SHAPE);
        };
        if let Err(e) = crate::people::validate_name(name) {
            return err(StatusCode::BAD_REQUEST, &e);
        }
        let secret = match crate::setup::validate_password(password) {
            Ok(s) => s,
            Err(e) => return err(StatusCode::BAD_REQUEST, &e),
        };
        let groups = match requested_groups(&api, &doc) {
            Ok(g) => g,
            Err(e) => return err(StatusCode::BAD_REQUEST, &e),
        };
        let email = match field("email").filter(|e| !e.is_empty()) {
            Some(e) if e.contains('@') && !e.contains(char::is_whitespace) => e.to_string(),
            Some(_) => return err(StatusCode::BAD_REQUEST, "that is not an email address"),
            None => {
                let host = std::fs::read_to_string("/proc/sys/kernel/hostname").unwrap_or_default();
                format!("{name}@{}.local", host.trim())
            }
        };
        let display = field("displayName").unwrap_or("");
        let refs: Vec<&str> = groups.iter().map(String::as_str).collect();
        match api.people.add(name, display, &email, &secret, &refs) {
            Ok(()) => {
                note_password(&api, name);
                HttpResponse::Ok().json(serde_json::json!({ "name": name }))
            }
            Err(e) => error_response(e),
        }
    })
}

/// The `name` in a request body.
fn named(body: &[u8]) -> Option<(String, serde_json::Value)> {
    let doc = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    let name = doc.get("name")?.as_str()?.trim().to_string();
    Some((name, doc))
}

/// `POST /api/people/access`: `{"name", "groups"}` in place of what they
/// had. Nobody changes their own, and the owner's stays.
async fn post_person_access(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded_as(&api, &req, "/api/people/access", true, |who| {
        if !api.people.headers {
            return not_standalone();
        }
        let Some((name, doc)) = named(&body) else {
            return err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"name": "...", "groups": [...]}"#,
            );
        };
        let groups = match requested_groups(&api, &doc) {
            Ok(g) => g,
            Err(e) => return err(StatusCode::BAD_REQUEST, &e),
        };
        let by = who.person().map(|p| p.name.as_str());
        let before = api
            .people
            .list()
            .ok()
            .and_then(|all| all.into_iter().find(|p| p.name == name))
            .map(|p| role_words(&api, &p.groups));
        match api.people.set_access(&name, &groups, by) {
            Ok(()) => {
                let after = role_words(&api, &groups);
                if before.as_deref() != Some(after.as_str()) {
                    api.moderation.note(
                        crate::login::now(),
                        who.audit_name(),
                        "role",
                        Some(&name),
                        &after,
                    );
                }
                HttpResponse::Ok().json(serde_json::json!({ "name": name, "groups": groups }))
            }
            Err(e) => error_response(e),
        }
    })
}

/// `POST /api/people/password`: `{"name", "password"}`, for someone who
/// forgot theirs. The owner changes their own on the Account menu, which
/// keeps their LosOS Cloud password in step.
async fn post_person_password(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded(&api, &req, "/api/people/password", true, || {
        if !api.people.headers {
            return not_standalone();
        }
        let Some((name, doc)) = named(&body) else {
            return err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"name": "...", "password": "..."}"#,
            );
        };
        let secret = match doc
            .get("password")
            .and_then(|p| p.as_str())
            .ok_or_else(|| "a password is needed".to_string())
            .and_then(crate::setup::validate_password)
        {
            Ok(s) => s,
            Err(e) => return err(StatusCode::BAD_REQUEST, &e),
        };
        let owner = api
            .people
            .list()
            .ok()
            .and_then(|all| all.into_iter().find(|p| p.name == name))
            .is_some_and(|p| p.groups.iter().any(|g| g == crate::people::OWNER));
        if owner {
            return error_response(crate::people::Refused::Owner(name).into());
        }
        match api.people.set_password(&name, &secret) {
            Ok(()) => {
                note_password(&api, &name);
                HttpResponse::Ok().json(serde_json::json!({ "name": name, "changed": true }))
            }
            Err(e) => error_response(e),
        }
    })
}

/// `POST /api/people/remove`: `{"name"}`. They can no longer sign in; their
/// accounts in LosOS Cloud and LosOS Git, and the files in them, stay.
async fn post_person_remove(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded_as(&api, &req, "/api/people/remove", true, |who| {
        if !api.people.headers {
            return not_standalone();
        }
        let Some((name, _)) = named(&body) else {
            return err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"name": "..."}"#,
            );
        };
        let by = who.person().map(|p| p.name.as_str());
        match api.people.remove(&name, by) {
            Ok(()) => {
                if let Err(e) = api.login.forget(&name) {
                    tracing::warn!(error = ?e, "a removed person's password date stayed");
                }
                if let Err(e) = api.profiles.forget(&name) {
                    tracing::warn!(error = ?e, "a removed person's profile stayed");
                }
                // Every chat device they had, signed out (`crate::chat`).
                match api.chat.close(&name) {
                    Ok(()) | Err(crate::chat::ChatError::Off) => {}
                    Err(e) => {
                        tracing::warn!(error = %e, person = %name, "a removed person's chat devices stayed signed in")
                    }
                }
                if let Err(e) = api.moderation.forget(&name) {
                    tracing::warn!(error = ?e, "a removed person's mutes stayed");
                }
                if let Err(e) = api.pictures.forget(&name) {
                    tracing::warn!(error = ?e, "a removed person's picture settings stayed");
                }
                api.moderation.note(
                    crate::login::now(),
                    who.audit_name(),
                    "remove",
                    Some(&name),
                    "",
                );
                HttpResponse::Ok().json(serde_json::json!({ "name": name, "removed": true }))
            }
            Err(e) => error_response(e),
        }
    })
}

// ── Schizo mode and the computers it reaches (`crate::schizo`) ───────────

/// The owner, when `who` is them signed in. Schizo mode is the owner's own
/// setting: not an administrator's, not the spare key's.
fn owner_of<'a>(api: &Api, who: &'a Principal) -> Option<&'a crate::people::Person> {
    use crate::losos::Losos;
    let person = who.person()?;
    let owner = api.backend.serialized(|l| l.load_state()).ok()?.owner?;
    (person.name == owner && person.groups.iter().any(|g| g == crate::people::OWNER))
        .then_some(person)
}

/// `GET /api/schizo`: whether schizo mode is on, and the computers it
/// reaches, for the owner.
async fn get_schizo(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded_as(&api, &req, "/api/schizo", false, |who| {
        if owner_of(&api, who).is_none() {
            return err(StatusCode::FORBIDDEN, "schizo mode is the owner's to see");
        }
        match api.schizo.status() {
            Ok(v) => HttpResponse::Ok().json(v),
            Err(e) => error_response(e),
        }
    })
}

/// `POST /api/schizo`: `{"on", "full", "duress"?}`, the owner signed in.
/// Turning it on closes the box's open market listings.
async fn post_schizo(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    const SHAPE: &str = r#"body must be JSON: {"on": bool, "full": bool, "duress"?: "..."}"#;
    guarded_as(&api, &req, "/api/schizo", true, |who| {
        let Some(owner) = owner_of(&api, who) else {
            return err(
                StatusCode::FORBIDDEN,
                "only the owner turns schizo mode on or off",
            );
        };
        let Ok(doc) = serde_json::from_slice::<serde_json::Value>(&body) else {
            return err(StatusCode::BAD_REQUEST, SHAPE);
        };
        let (Some(on), full) = (
            doc.get("on").and_then(serde_json::Value::as_bool),
            doc.get("full")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
        ) else {
            return err(StatusCode::BAD_REQUEST, SHAPE);
        };
        let duress = match doc.get("duress").and_then(serde_json::Value::as_str) {
            None => None,
            Some(code) => match crate::signin::validate_candidate(code) {
                Ok(code) => Some(code),
                Err(e) => return err(StatusCode::BAD_REQUEST, &e),
            },
        };
        let was_on = api.schizo.on();
        let is_password = |code: &crate::setup::Secret| {
            api.people.check_password(&owner.name, code).unwrap_or(true)
        };
        if let Err(e) = api
            .schizo
            .set(on, full, duress.as_ref(), is_password, crate::login::now())
        {
            return error_response(e);
        }
        if on && !was_on {
            close_listings(&api);
            match api.backend.serialized(crate::losos::cmd_schizo_tor) {
                Ok(Some(job)) => {
                    tracing::info!(job = %job, "schizo mode: giving the box a Tor address")
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "schizo mode could not give the box a Tor address")
                }
            }
        }
        match api.schizo.status() {
            Ok(v) => HttpResponse::Ok().json(v),
            Err(e) => error_response(e),
        }
    })
}

/// The box's open listings, closed on its edge in the background: schizo
/// mode sells nothing, and a listing left open would.
fn close_listings(api: &web::Data<Api>) {
    let api = api.clone();
    std::thread::spawn(move || {
        let closed = api
            .backend
            .serialized(|l| Ok::<_, anyhow::Error>(crate::erase::close_listings(l)));
        match closed {
            Ok((n, true)) => tracing::info!(closed = n, "schizo mode closed the box's listings"),
            Ok((n, false)) => {
                tracing::warn!(closed = n, "schizo mode could not close every listing")
            }
            Err(e) => tracing::warn!(error = %e, "schizo mode could not close the listings"),
        }
    });
}

/// The joined computer behind a directory token (`crate::directory`), its
/// id and name, or the answer that refuses the call. These routes are open
/// beyond the LAN and past LosOS Login (`modules/identity.nix`), so the
/// token is the whole of the check, and a wrong one counts against the
/// address as a wrong spare key does.
fn device_call(api: &Api, req: &HttpRequest) -> Result<(String, String), Box<HttpResponse>> {
    let remote = remote_addr(req);
    if let Err(wait) = api.throttle.check(&remote, Instant::now()) {
        let mut resp = err(
            StatusCode::TOO_MANY_REQUESTS,
            "too many failed attempts; slow down",
        );
        resp.headers_mut().insert(
            actix_web::http::header::RETRY_AFTER,
            retry_after_secs(wait).to_string().parse().expect("digits"),
        );
        return Err(Box::new(resp));
    }
    let token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(bearer_credentials)
        .filter(|t| crate::directory::is_directory_token(t));
    let now = i64::try_from(crate::login::now()).unwrap_or(i64::MAX);
    match token.and_then(|t| api.directory.check(t, now)) {
        Some(computer) => {
            api.throttle.record_success(&remote);
            Ok((computer.id, computer.device))
        }
        None => {
            api.throttle.record_failure(&remote, Instant::now());
            Err(Box::new(err(StatusCode::UNAUTHORIZED, "unauthorized")))
        }
    }
}

/// `POST /api/login-devices/self`: `{"onion"?}`. A computer checks in,
/// with its own Tor address once it has one, and is told whether schizo
/// mode is on, the duress code's hash while the full form is, a wipe order
/// it has not confirmed, and, while schizo mode is on, the Tor addresses of
/// the box and of the owner's other computers.
async fn post_login_device_self(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    let (id, name) = match device_call(&api, &req) {
        Ok(found) => found,
        Err(refused) => return *refused,
    };
    let onion = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|d| d.get("onion")?.as_str().map(str::to_string));
    let box_onion = crate::schizo::box_onion(std::path::Path::new(HIDDEN_ADDRESSES));
    match api.schizo.policy(
        &id,
        &name,
        onion.as_deref(),
        box_onion.as_deref(),
        crate::login::now(),
    ) {
        Ok(p) => HttpResponse::Ok().json(p),
        Err(e) => error_response(e),
    }
}

/// The box's Tor and I2P addresses (modules/hidden-lib.nix).
const HIDDEN_ADDRESSES: &str = "/var/lib/losos-hidden/addresses.json";

/// `POST /api/login-devices/self/ack`: `{"wipe"}`. The computer heard the
/// order and is carrying it out.
async fn post_login_device_ack(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    let (id, _) = match device_call(&api, &req) {
        Ok(found) => found,
        Err(refused) => return *refused,
    };
    let Some(wipe) = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|d| d.get("wipe")?.as_str().map(str::to_string))
    else {
        return err(
            StatusCode::BAD_REQUEST,
            r#"body must be JSON: {"wipe": "..."}"#,
        );
    };
    match api.schizo.ack(&id, &wipe, crate::login::now()) {
        Ok(known) => HttpResponse::Ok().json(serde_json::json!({ "wipe": wipe, "known": known })),
        Err(e) => error_response(e),
    }
}

/// `POST /api/login-devices/self/duress`: `{"code"}`. Someone typed this at
/// the computer's login. The box checks it against its own hash, and a
/// match orders every other computer of the owner's to wipe itself.
async fn post_login_device_duress(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    let (id, _) = match device_call(&api, &req) {
        Ok(found) => found,
        Err(refused) => return *refused,
    };
    let Some(code) = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|d| d.get("code")?.as_str().map(str::to_string))
        .and_then(|c| crate::signin::validate_candidate(&c).ok())
    else {
        return err(
            StatusCode::BAD_REQUEST,
            r#"body must be JSON: {"code": "..."}"#,
        );
    };
    match api.schizo.report(&id, &code, crate::login::now()) {
        Ok(crate::schizo::Reported::Wipe) => {
            // Every other computer of the owner's hears of it at its next
            // check-in. What the box itself removes of the owner's is not
            // decided yet; until it is, the box keeps everything.
            tracing::warn!(device = %id, "a duress code was entered: the owner's computers are told to wipe");
            HttpResponse::Ok().json(serde_json::json!({ "wipe": true }))
        }
        Ok(crate::schizo::Reported::Already) => {
            HttpResponse::Ok().json(serde_json::json!({ "wipe": true }))
        }
        // The same answer as a right code that came too late: whoever holds
        // the token learns nothing about the code from it.
        Ok(crate::schizo::Reported::Wrong) => {
            api.throttle
                .record_failure(&remote_addr(&req), Instant::now());
            HttpResponse::Ok().json(serde_json::json!({ "wipe": false }))
        }
        Err(e) => error_response(e),
    }
}

// ── LosOS Login: rules, second factors, own account ──────────────────────

/* What `crate::login` says about each person, carried into the people file
 * as two marks Authelia's access rules read: `losos-two-factor` (has a
 * second factor, so it is asked for) and `losos-locked` (a rule holds them
 * to their own account). Kept in step every few seconds, since a person
 * adds a passkey on LosOS Login's own page and a password runs out on its
 * own, and at once after every change made here. */

/// A person's standing under the rules, at `now`.
fn login_standing(
    api: &Api,
    name: &str,
    rules: crate::login::Rules,
    now: u64,
) -> (crate::login::Factors, u64, Vec<crate::login::Reason>) {
    let factors = api.login.factors(name);
    let set_at = api.login.password_set(name, now);
    let reasons = crate::login::reasons(rules, &factors, set_at, now);
    (factors, set_at, reasons)
}

/// `name` has just set a password: the date the rule counts from, then the
/// marks, so a person held for an old password is let go at once.
fn note_password(api: &Api, name: &str) {
    if let Err(e) = api.login.record_password(name, crate::login::now()) {
        tracing::warn!(error = ?e, "could not record when a password was set");
    }
    reconcile_login(api);
}

/// Bring every person's marks in step with the rules and their factors.
fn reconcile_login(api: &Api) {
    if !api.people.headers {
        return;
    }
    let people = match api.people.list() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = ?e, "the people file could not be read");
            return;
        }
    };
    let rules = api.login.rules();
    let now = crate::login::now();
    let mut marks = std::collections::BTreeMap::new();
    for person in people {
        let (factors, _, reasons) = login_standing(api, &person.name, rules, now);
        let mut want = Vec::new();
        if factors.any() {
            want.push(crate::people::TWO_FACTOR);
        }
        if !reasons.is_empty() {
            want.push(crate::people::LOCKED);
        }
        marks.insert(person.name, want);
    }
    if let Err(e) = api.people.set_marks(&marks) {
        tracing::warn!(error = ?e, "LosOS Login's marks could not be written");
    }
}

/// End the mutes and suspensions whose time has come. A suspended person
/// never reaches this daemon (LosOS Login turns them away), so it is a
/// timer that lets them back in.
fn lift_due(api: &Api) {
    let now = crate::login::now();
    if let Err(e) = api
        .moderation
        .lift_due(now, |name| api.people.set_disabled(name, false, None))
    {
        tracing::warn!(error = ?e, "the moderation file could not be read");
    }
}

/// The thread that keeps the marks in step.
fn start_login_marks(api: web::Data<Api>) {
    if !api.people.headers {
        return;
    }
    std::thread::spawn(move || loop {
        reconcile_login(&api);
        lift_due(&api);
        std::thread::sleep(std::time::Duration::from_secs(10));
    });
}

/// Give LosOS Cloud and LosOS Git LosOS Login's name and address for
/// everyone (`only` none) or one person, in the background: either app may
/// still be starting, so whoever is left is asked again for a while.
fn sync_names_later(api: web::Data<Api>, only: Option<String>) {
    if !api.people.headers {
        return;
    }
    std::thread::spawn(move || {
        let mut backend = api.backend.clone();
        let mut done = std::collections::BTreeSet::new();
        for round in 0..40 {
            if round > 0 {
                std::thread::sleep(std::time::Duration::from_secs(30));
            }
            let Ok(people) = api.people.list() else {
                continue;
            };
            let mut left = false;
            for person in people {
                if only.as_ref().is_some_and(|n| *n != person.name) || done.contains(&person.name) {
                    continue;
                }
                if crate::app_names::sync(&mut backend, &crate::app_names::profile(&person)) {
                    done.insert(person.name);
                } else {
                    left = true;
                }
            }
            if !left {
                return;
            }
        }
    });
}

/// `GET /api/account`: the caller's own account: name, email, second
/// factors, when their password runs out, and what holds them.
async fn get_account(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded_as(&api, &req, "/api/account", false, |who| {
        let Some(person) = who.person() else {
            return err(
                StatusCode::CONFLICT,
                "the spare key has no account of its own",
            );
        };
        if !api.people.headers {
            return not_standalone();
        }
        reconcile_login(&api);
        let listed = match api.people.list() {
            Ok(all) => all.into_iter().find(|p| p.name == person.name),
            Err(e) => return error_response(e),
        };
        let Some(listed) = listed else {
            return error_response(crate::people::Refused::Unknown(person.name.clone()).into());
        };
        let rules = api.login.rules();
        let (factors, set_at, reasons) =
            login_standing(&api, &person.name, rules, crate::login::now());
        HttpResponse::Ok().json(serde_json::json!({
            "name": listed.name,
            "displayName": listed.display_name,
            "email": listed.email,
            "factors": factors,
            "passwordSetAt": set_at,
            "passwordExpiresAt": crate::login::expires_at(rules, set_at),
            "rules": rules,
            "locked": reasons,
        }))
    })
}

/// `POST /api/account`: `{"displayName", "email"}`, the caller's own.
async fn post_account(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    const SHAPE: &str = r#"body must be JSON: {"displayName": "...", "email": "..."}"#;
    guarded_as(&api, &req, "/api/account", true, |who| {
        let Some(person) = who.person() else {
            return err(
                StatusCode::CONFLICT,
                "the spare key has no account of its own",
            );
        };
        if !api.people.headers {
            return not_standalone();
        }
        let Ok(doc) = serde_json::from_slice::<serde_json::Value>(&body) else {
            return err(StatusCode::BAD_REQUEST, SHAPE);
        };
        let field = |k: &str| doc.get(k).and_then(|v| v.as_str());
        let (Some(display), Some(email)) = (field("displayName"), field("email")) else {
            return err(StatusCode::BAD_REQUEST, SHAPE);
        };
        match api.people.set_details(&person.name, display, email) {
            Ok(()) => {
                sync_names_later(api.clone(), Some(person.name.clone()));
                HttpResponse::Ok().json(serde_json::json!({
                "name": person.name,
                "displayName": display.trim(),
                "email": email.trim(),
                }))
            }
            Err(e) => error_response(e),
        }
    })
}

// ── Profiles ──────────────────────────────────────────────────────────────

/* Each person's picture, banner, colour, pronouns and a line about them
 * (`crate::profiles`, contract in docs/profiles.md). Anyone signed in may
 * read everyone's, which is what makes a picture useful beside a name; only
 * the person changes their own, and an administrator may take a picture
 * down. */

/// Everyone the people file lists and LosOS Login lets in, by name.
fn shown_people(api: &Api) -> anyhow::Result<Vec<serde_json::Value>> {
    if !api.people.headers {
        return Ok(Vec::new());
    }
    let profiles = api.profiles.all()?;
    let presets = api.roles.all();
    Ok(api
        .people
        .list()?
        .into_iter()
        .filter(|p| !p.disabled)
        .map(|p| {
            let profile = profiles.get(&p.name).cloned().unwrap_or_default();
            let mut shown = crate::profiles::render(&p, &profile);
            shown["roleName"] = serde_json::json!(crate::roles::role_of(&p.groups)
                .custom
                .and_then(|id| presets.get(&id).map(|r| r.name.clone())));
            shown
        })
        .collect())
}

/// `GET /api/profiles`: everyone's profile.
async fn get_profiles(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/profiles", false, || {
        match shown_people(&api) {
            Ok(people) => HttpResponse::Ok().json(serde_json::json!({ "people": people })),
            Err(e) => error_response(e),
        }
    })
}

/// `GET /api/profiles/{name}`: one person's profile, 404 for nobody.
async fn get_profile(
    api: web::Data<Api>,
    req: HttpRequest,
    path: web::Path<String>,
) -> HttpResponse {
    let name = path.into_inner();
    guarded(&api, &req, "/api/profiles", false, || {
        match shown_people(&api) {
            Ok(people) => match people.into_iter().find(|p| p["name"] == name.as_str()) {
                Some(p) => HttpResponse::Ok().json(p),
                None => err(StatusCode::NOT_FOUND, &format!("{name} has no account")),
            },
            Err(e) => error_response(e),
        }
    })
}

/// `GET /api/profiles/{name}/{kind}?v=N`: the picture or the banner. Cached
/// hard, because a new one is a new `?v=`; any other `?v=` is a 404, so a
/// cache never keeps bytes under a URL that names others.
async fn get_profile_image(
    api: web::Data<Api>,
    req: HttpRequest,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (name, kind) = path.into_inner();
    let version =
        web::Query::<std::collections::HashMap<String, String>>::from_query(req.query_string())
            .ok()
            .and_then(|q| q.get("v").and_then(|v| v.parse::<u64>().ok()));
    guarded(&api, &req, "/api/profiles", false, || {
        let Some(kind) = crate::profiles::Kind::parse(&kind) else {
            return err(
                StatusCode::NOT_FOUND,
                "a profile has a picture and a banner",
            );
        };
        if crate::people::validate_name(&name).is_err() {
            return err(StatusCode::NOT_FOUND, "no picture");
        }
        let listed = api
            .people
            .list()
            .map(|all| all.into_iter().any(|p| p.name == name && !p.disabled));
        if !matches!(listed, Ok(true)) {
            return err(StatusCode::NOT_FOUND, "no picture");
        }
        let Some(version) = version else {
            return err(StatusCode::NOT_FOUND, "no picture");
        };
        match api.profiles.read_image(&name, kind, version) {
            Ok(Some((content_type, bytes))) => HttpResponse::Ok()
                .content_type(content_type)
                .insert_header(("Cache-Control", "private, max-age=31536000, immutable"))
                .insert_header(("X-Content-Type-Options", "nosniff"))
                .body(bytes),
            Ok(None) => err(StatusCode::NOT_FOUND, "no picture"),
            Err(e) => error_response(e),
        }
    })
}

/// The caller's own profile as every page shows it.
fn own_profile(api: &Api, person: &crate::people::Person) -> HttpResponse {
    let listed = match api.people.list() {
        Ok(all) => all.into_iter().find(|p| p.name == person.name),
        Err(e) => return error_response(e),
    };
    let Some(listed) = listed else {
        return error_response(crate::people::Refused::Unknown(person.name.clone()).into());
    };
    match api.profiles.get(&person.name) {
        Ok(profile) => HttpResponse::Ok().json(crate::profiles::render(&listed, &profile)),
        Err(e) => error_response(e),
    }
}

/// `POST /api/account/profile`: `{"pronouns", "about", "accent"}`, each
/// optional, the caller's own.
async fn post_account_profile(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded_as(&api, &req, "/api/account/profile", true, |who| {
        let Some(person) = who.person() else {
            return err(
                StatusCode::CONFLICT,
                "the spare key has no account of its own",
            );
        };
        if !api.people.headers {
            return not_standalone();
        }
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        let details = match crate::profiles::parse_details(&doc) {
            Ok(d) => d,
            Err(why) => return err(StatusCode::BAD_REQUEST, &why),
        };
        match api
            .profiles
            .set_details(&person.name, &details, || still_listed(&api, &person.name))
        {
            Ok(_) => own_profile(&api, person),
            Err(e) => error_response(e),
        }
    })
}

/// Whether `name` is still in the people file, asked by the profile store
/// under its lock so a change racing the person's removal is refused.
fn still_listed(api: &Api, name: &str) -> anyhow::Result<bool> {
    Ok(api.people.get(name)?.is_some())
}

fn own_image_route(kind: crate::profiles::Kind) -> &'static str {
    match kind {
        crate::profiles::Kind::Picture => "/api/account/picture",
        crate::profiles::Kind::Banner => "/api/account/banner",
    }
}

/// How many uploads are decoded and encoded at once. The work runs on the
/// blocking pool, never on a worker that answers requests, and a few
/// people uploading 8 MiB photos together wait their turn rather than take
/// every core of a small box.
const IMAGE_JOBS: usize = 2;

/// `PUT /api/account/{picture,banner}`: the caller's own. The body is the
/// picture's bytes; their type is read off the bytes.
async fn put_own_image(
    api: web::Data<Api>,
    req: HttpRequest,
    kind: crate::profiles::Kind,
    body: web::Bytes,
) -> HttpResponse {
    // The guard first, so nobody unsigned makes the box decode anything.
    let mut caller = None;
    let refused = guarded_as(&api, &req, own_image_route(kind), true, |who| {
        let Some(person) = who.person() else {
            return err(
                StatusCode::CONFLICT,
                "the spare key has no account of its own",
            );
        };
        if !api.people.headers {
            return not_standalone();
        }
        caller = Some(person.clone());
        HttpResponse::Ok().finish()
    });
    let Some(person) = caller else {
        return refused;
    };
    let normalized = {
        let Ok(_turn) = api.image_jobs.acquire().await else {
            return err(StatusCode::SERVICE_UNAVAILABLE, "lososd is stopping");
        };
        match web::block(move || crate::profiles::normalize(kind, &body)).await {
            Ok(done) => done,
            Err(e) => return error_response(anyhow::anyhow!("decoding the picture: {e}")),
        }
    };
    let stored = normalized.and_then(|picture| {
        api.profiles
            .put_normalized(&person.name, kind, picture, || {
                still_listed(&api, &person.name)
            })
    });
    match stored {
        Ok(_) => own_profile(&api, &person),
        Err(e) => error_response(e),
    }
}

/// `DELETE /api/account/{picture,banner}`: the caller's own.
fn delete_own_image(api: &Api, req: &HttpRequest, kind: crate::profiles::Kind) -> HttpResponse {
    guarded_as(api, req, own_image_route(kind), true, |who| {
        let Some(person) = who.person() else {
            return err(
                StatusCode::CONFLICT,
                "the spare key has no account of its own",
            );
        };
        if !api.people.headers {
            return not_standalone();
        }
        match api.profiles.remove_image(&person.name, kind) {
            Ok(_) => own_profile(api, person),
            Err(e) => error_response(e),
        }
    })
}

async fn put_account_picture(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    put_own_image(api, req, crate::profiles::Kind::Picture, body).await
}

async fn delete_account_picture(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    delete_own_image(&api, &req, crate::profiles::Kind::Picture)
}

async fn put_account_banner(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    put_own_image(api, req, crate::profiles::Kind::Banner, body).await
}

async fn delete_account_banner(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    delete_own_image(&api, &req, crate::profiles::Kind::Banner)
}

// ── Pictures in Chat and Mail: blur, trusted people, age marks ───────────
// (`crate::pictures`). The blur and the nudity warning are drawn by the
// person's browser; these routes only keep and answer the settings.

/// A refusal from the picture settings, with the word the page keys on.
fn pictures_error(e: anyhow::Error) -> HttpResponse {
    match e.downcast_ref::<crate::pictures::Refused>() {
        Some(r) => HttpResponse::Forbidden()
            .json(serde_json::json!({ "error": r.to_string(), "code": r.code() })),
        None => error_response(e),
    }
}

fn own_pictures(
    who: &Principal,
    f: impl FnOnce(&crate::people::Person) -> anyhow::Result<crate::pictures::Settings>,
) -> HttpResponse {
    let Some(person) = who.person() else {
        return err(
            StatusCode::CONFLICT,
            "the spare key has no account of its own",
        );
    };
    match f(person) {
        Ok(s) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(crate::pictures::render(&s)),
        Err(e) => pictures_error(e),
    }
}

/// `GET /api/pictures`: the caller's own picture settings.
async fn get_pictures(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded_as(&api, &req, "/api/pictures", false, |who| {
        own_pictures(who, |p| api.pictures.get(&p.name))
    })
}

/// `POST /api/pictures`: `{"blur": "everyone|strangers|nobody",
/// "nudityCheck": bool}`, each optional. 403 `adultRequired` or
/// `childAccount` when the age mark does not allow it.
async fn post_pictures(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    guarded_as(&api, &req, "/api/pictures", true, |who| {
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        let change = match crate::pictures::parse_change(&doc) {
            Ok(c) => c,
            Err(why) => return err(StatusCode::BAD_REQUEST, &why),
        };
        own_pictures(who, |p| api.pictures.set(&p.name, &change))
    })
}

/// `POST /api/pictures/trust`: `{"id": "@bo:box.local" | "bo@example.org",
/// "trusted": bool}`. A trusted sender's pictures are never blurred.
async fn post_pictures_trust(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded_as(&api, &req, "/api/pictures/trust", true, |who| {
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        let (Some(id), Some(trusted)) = (
            doc.get("id").and_then(serde_json::Value::as_str),
            doc.get("trusted").and_then(serde_json::Value::as_bool),
        ) else {
            return err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"id": "...", "trusted": true}"#,
            );
        };
        own_pictures(who, |p| api.pictures.trust(&p.name, id, trusted))
    })
}

/// `GET /api/pictures/sender?id=`: whether pictures from this chat ID or
/// address show at once for the caller. `{"shown": bool, "because":
/// "trusted|known|setting"|null, "nudityCheck": bool}`. "known" means on
/// this box or in the caller's contacts, asked only for `strangers`.
async fn get_pictures_sender(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    let mut person = None;
    let refused = guarded_as(&api, &req, "/api/pictures/sender", false, |who| {
        match who.person() {
            Some(p) => {
                person = Some(p.clone());
                HttpResponse::Ok().finish()
            }
            None => err(
                StatusCode::CONFLICT,
                "the spare key has no account of its own",
            ),
        }
    });
    let Some(person) = person else {
        return refused;
    };
    let Some(sender) = query_map(&req)
        .get("id")
        .and_then(|id| crate::pictures::valid_id(id))
    else {
        return err(
            StatusCode::BAD_REQUEST,
            "id is a chat ID such as @ana:box.local or a mail address",
        );
    };
    let worker = api.clone();
    let answer = web::block(move || -> anyhow::Result<serde_json::Value> {
        let settings = worker.pictures.get(&person.name)?;
        let because = crate::pictures::shown_because(&settings, &sender, || {
            sender_known(&worker, &person, &sender)
        });
        Ok(serde_json::json!({
            "shown": because.is_some(),
            "because": because,
            "nudityCheck": settings.nudity_check(),
        }))
    })
    .await;
    match answer {
        Ok(Ok(v)) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(v),
        Ok(Err(e)) => error_response(e),
        Err(_) => HttpResponse::ServiceUnavailable().finish(),
    }
}

/// Whether `sender` is a person on this box or in `person`'s contacts.
fn sender_known(api: &Api, person: &crate::people::Person, sender: &str) -> bool {
    let same = |s: &str| s.eq_ignore_ascii_case(sender);
    if contacts_neighbours(api).iter().any(|n| {
        n.name != person.name
            && (n.matrix_id.as_deref().is_some_and(same) || n.email.as_deref().is_some_and(same))
    }) {
        return true;
    }
    if !api.contacts {
        return false;
    }
    let login = if person.groups.iter().any(|g| g == crate::people::OWNER) {
        crate::app_names::OWNER_ACCOUNT.to_string()
    } else {
        person.name.clone()
    };
    let want = if sender.starts_with('@') {
        crate::cloud::contacts::Want::Matrix
    } else {
        crate::cloud::contacts::Want::Email
    };
    let book = crate::contacts::book_matches(&mut api.backend.clone(), &login, sender, want);
    book.known_matrix_ids.iter().any(|id| same(id))
        || book.matches.iter().any(|r| {
            r.suggestion.matrix_id.as_deref().is_some_and(same)
                || r.suggestion.emails.iter().any(|e| same(e))
        })
}

/// `POST /api/people/age`: `{"name", "age": "adult" | "child" | null}`.
/// An administrator marks a person as an adult (who may then show every
/// picture at once) or as a child (whose nudity warning stays on).
async fn post_people_age(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    guarded(&api, &req, "/api/people/age", true, || {
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        let name = doc.get("name").and_then(serde_json::Value::as_str);
        let age = match doc.get("age") {
            Some(serde_json::Value::Null) => Some(None),
            Some(v) => v.as_str().and_then(crate::pictures::Age::parse).map(Some),
            None => None,
        };
        let (Some(name), Some(age)) = (name, age) else {
            return err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"name": "...", "age": "adult" | "child" | null}"#,
            );
        };
        match api.people.get(name) {
            Ok(Some(_)) => {}
            Ok(None) => {
                return error_response(crate::people::Refused::Unknown(name.to_string()).into())
            }
            Err(e) => return error_response(e),
        }
        match api.pictures.set_age(name, age) {
            Ok(s) => HttpResponse::Ok().json(serde_json::json!({ "name": name, "age": s.age })),
            Err(e) => pictures_error(e),
        }
    })
}

/// `POST /api/people/profile/clear`: `{"name", "kind": "picture|banner"}`.
/// An administrator takes down a picture someone else put up. The rest of
/// the profile stays theirs.
async fn post_people_profile_clear(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    const SHAPE: &str = r#"body must be JSON: {"name": "...", "kind": "picture|banner"}"#;
    guarded_as(&api, &req, "/api/people/profile/clear", true, |who| {
        if !api.people.headers {
            return not_standalone();
        }
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        let (Some(name), Some(kind)) = (
            doc.get("name").and_then(|v| v.as_str()),
            doc.get("kind")
                .and_then(|v| v.as_str())
                .and_then(crate::profiles::Kind::parse),
        ) else {
            return err(StatusCode::BAD_REQUEST, SHAPE);
        };
        match api.people.list() {
            Ok(all) if all.iter().any(|p| p.name == name) => {}
            Ok(_) => {
                return error_response(crate::people::Refused::Unknown(name.to_string()).into())
            }
            Err(e) => return error_response(e),
        }
        match api.profiles.remove_image(name, kind) {
            Ok(_) => {
                api.moderation.note(
                    crate::login::now(),
                    who.audit_name(),
                    "take down",
                    Some(name),
                    kind.as_str(),
                );
                HttpResponse::Ok().json(
                    serde_json::json!({ "name": name, "kind": kind.as_str(), "cleared": true }),
                )
            }
            Err(e) => error_response(e),
        }
    })
}

// ── Roles and moderation ─────────────────────────────────────────────────

/* Roles are bundles of the groups sections.json reads (`crate::roles`);
 * moderation is reports, mutes, suspensions and a log (`crate::moderation`).
 * Both stay in this daemon, beside the people file they write and the
 * sign-in they decide. */

/// The custom roles as `GET /api/people` lists them.
fn presets_json(
    presets: &std::collections::BTreeMap<String, crate::roles::Preset>,
) -> Vec<serde_json::Value> {
    presets
        .iter()
        .map(|(id, p)| serde_json::json!({ "id": id, "name": p.name, "sections": p.sections }))
        .collect()
}

/// Put everyone who holds custom role `id` in `groups` in place of what
/// they had.
fn regroup(api: &Api, id: &str, groups: &[String]) -> anyhow::Result<()> {
    let mark = format!("{}{id}", crate::roles::CUSTOM);
    for person in api.people.list()? {
        if person.groups.contains(&mark) {
            api.people.set_access(&person.name, groups, None)?;
        }
    }
    Ok(())
}

/// `POST /api/roles`: `{"id"?, "name", "sections"}`. Save a custom role,
/// and move everyone who holds it to its new sections.
async fn post_roles(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    const SHAPE: &str = r#"body must be JSON: {"id"?: "...", "name": "...", "sections": [...]}"#;
    guarded_as(&api, &req, "/api/roles", true, |who| {
        if !api.people.headers {
            return not_standalone();
        }
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        let (Some(name), Some(list)) = (
            doc.get("name").and_then(|v| v.as_str()),
            doc.get("sections").and_then(|v| v.as_array()),
        ) else {
            return err(StatusCode::BAD_REQUEST, SHAPE);
        };
        let sections: Vec<String> = list
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        let id = doc.get("id").and_then(|v| v.as_str());
        let id = match api.roles.put(id, name, &sections) {
            Ok(id) => id,
            Err(e) => return error_response(e),
        };
        let presets = api.roles.all();
        if let Some(preset) = presets.get(&id) {
            if let Err(e) = regroup(&api, &id, &crate::roles::preset_groups(&id, preset)) {
                return error_response(e);
            }
            api.moderation.note(
                crate::login::now(),
                who.audit_name(),
                "role saved",
                None,
                &preset.name,
            );
        }
        HttpResponse::Ok().json(serde_json::json!({ "id": id, "roles": presets_json(&presets) }))
    })
}

/// `POST /api/roles/remove`: `{"id"}`. Everyone who held the role keeps
/// only their own account.
async fn post_roles_remove(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded_as(&api, &req, "/api/roles/remove", true, |who| {
        if !api.people.headers {
            return not_standalone();
        }
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        let Some(id) = doc.get("id").and_then(|v| v.as_str()) else {
            return err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"id": "..."}"#,
            );
        };
        let Some(preset) = api.roles.all().remove(id) else {
            return error_response(crate::people::Refused::Unknown(id.to_string()).into());
        };
        if let Err(e) = regroup(&api, id, &["losos-account".to_string()]) {
            return error_response(e);
        }
        if let Err(e) = api.roles.remove(id) {
            return error_response(e);
        }
        api.moderation.note(
            crate::login::now(),
            who.audit_name(),
            "role removed",
            None,
            &preset.name,
        );
        HttpResponse::Ok().json(serde_json::json!({ "roles": presets_json(&api.roles.all()) }))
    })
}

/// `POST /api/reports`: `{"app", "ref", "reason", "about"?, "note"?}` from
/// anyone signed in. The report names the thing; it never carries it.
async fn post_report(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    guarded_as(&api, &req, "/api/reports", true, |who| {
        let Some(person) = who.person() else {
            return err(
                StatusCode::CONFLICT,
                "the spare key is nobody; sign in with LosOS Login to report something",
            );
        };
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        let new = match crate::moderation::parse_report(&doc) {
            Ok(n) => n,
            Err(e) => return error_response(e),
        };
        match api
            .moderation
            .report(&person.name, new, crate::login::now())
        {
            Ok(id) => HttpResponse::Ok().json(serde_json::json!({ "id": id })),
            Err(e) => error_response(e),
        }
    })
}

/// The caller's role, for what they may do to whom. The spare key acts as
/// an administrator.
fn actor_role(who: &Principal) -> crate::roles::Role {
    who.person().map_or(crate::roles::Role::Admin, |p| {
        crate::roles::role_of(&p.groups).role
    })
}

/// `name`'s entry, if the caller may moderate them, or the answer that
/// says why not.
fn moderated(
    api: &Api,
    who: &Principal,
    name: &str,
) -> Result<crate::people::Listed, Box<HttpResponse>> {
    if !api.people.headers {
        return Err(Box::new(not_standalone()));
    }
    let listed = match api.people.list() {
        Ok(all) => all.into_iter().find(|p| p.name == name),
        Err(e) => return Err(Box::new(error_response(e))),
    };
    let Some(listed) = listed else {
        return Err(Box::new(error_response(
            crate::people::Refused::Unknown(name.to_string()).into(),
        )));
    };
    if who.person().is_some_and(|p| p.name == name) {
        return Err(Box::new(err(
            StatusCode::CONFLICT,
            "you cannot mute or suspend yourself",
        )));
    }
    let target = crate::roles::role_of(&listed.groups).role;
    if !crate::roles::may_moderate(actor_role(who), target) {
        return Err(Box::new(err(
            StatusCode::FORBIDDEN,
            "moderators act on members and guests; an administrator can help with the rest",
        )));
    }
    Ok(listed)
}

/// `GET /api/moderation`: the reports, everyone with what holds them, and
/// the newest lines of the log.
async fn get_moderation(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded_as(&api, &req, "/api/moderation", false, |who| {
        let now = crate::login::now();
        let actor = actor_role(who);
        let you = who.person().map(|p| p.name.clone());
        let people = if api.people.headers {
            api.people.list().unwrap_or_default()
        } else {
            Vec::new()
        };
        let profiles = api.profiles.all().unwrap_or_default();
        let presets = api.roles.all();
        let holds = api.moderation.holds();
        let people: Vec<serde_json::Value> = people
            .into_iter()
            .map(|p| {
                let profile = profiles.get(&p.name).cloned().unwrap_or_default();
                let mut row = crate::profiles::render(&p, &profile);
                let standing = crate::roles::role_of(&p.groups);
                row["roleName"] = serde_json::json!(standing
                    .custom
                    .and_then(|id| presets.get(&id).map(|r| r.name.clone())));
                let held = holds.get(&p.name).cloned().unwrap_or_default();
                row["muted"] = serde_json::json!(held
                    .muted
                    .filter(|_| api.moderation.muted(&p.name, now).is_some()));
                row["suspended"] = serde_json::json!(held.suspended);
                row["disabled"] = serde_json::json!(p.disabled);
                row["actionable"] = serde_json::json!(
                    you.as_deref() != Some(p.name.as_str())
                        && crate::roles::may_moderate(actor, standing.role)
                );
                row
            })
            .collect();
        HttpResponse::Ok().json(serde_json::json!({
            "standalone": api.people.headers,
            "you": you,
            "admin": who.is_admin(),
            "hides": if api.chat.live() {
                serde_json::json!(["mail", "chat", "profile"])
            } else {
                serde_json::json!(["mail", "profile"])
            },
            "reports": api.moderation.reports(),
            "people": people,
            "log": api.moderation.log(crate::moderation::LOG_SHOWN),
        }))
    })
}

/// `POST /api/moderation/reports/{id}`: `{"action": "hide" | "release" |
/// "dismiss" | "resolve"}`. Hiding holds a mail in the recipient's
/// Quarantine, redacts a chat message as its sender, or takes a picture
/// down, without anything of it reaching the moderator.
async fn post_moderation_report(
    api: web::Data<Api>,
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
) -> HttpResponse {
    use crate::moderation::{Act, App};
    guarded_as(&api, &req, "/api/moderation/reports", true, |who| {
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        let Some(act) = doc
            .get("action")
            .and_then(|v| v.as_str())
            .and_then(Act::parse)
        else {
            return err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"action": "hide|release|dismiss|resolve"}"#,
            );
        };
        let Some(report) = api.moderation.find(&path) else {
            return err(StatusCode::NOT_FOUND, "no such report");
        };
        if let Err(r) = crate::moderation::check_act(&report, act) {
            return error_response(r.into());
        }
        match (report.app, act) {
            (App::Mail, Act::Hide | Act::Release) => {
                let Ok(Some(owner)) = api.people.get(&report.by) else {
                    return err(
                        StatusCode::CONFLICT,
                        "the person who reported it has no account on this box now",
                    );
                };
                let hold = act == Act::Hide;
                let moved = if hold {
                    api.mail.hold(&owner, &report.reference)
                } else {
                    api.mail.release(&owner, &report.reference)
                };
                if let Err(r) = moved {
                    return mail_refused(&r);
                }
                if let Err(e) = api
                    .moderation
                    .hold_mail(&owner.name, &report.reference, hold)
                {
                    return error_response(e);
                }
            }
            (App::Chat, Act::Hide) => {
                let Some((room, event)) = crate::chat::event_ref(&report.reference) else {
                    return err(
                        StatusCode::CONFLICT,
                        "the report does not say which room the message is in",
                    );
                };
                let Some(about) = report.about.as_deref() else {
                    return err(
                        StatusCode::CONFLICT,
                        "the message is from someone on another box; leave the room or ask its admins",
                    );
                };
                if let Err(resp) = moderated(&api, who, about) {
                    return *resp;
                }
                if let Err(e) = api.chat.redact(about, room, event, "Hidden by a moderator") {
                    return chat_refused(&e);
                }
            }
            (App::Profile, Act::Hide) => {
                let Some(about) = report.about.as_deref() else {
                    return err(StatusCode::CONFLICT, "the report names nobody's profile");
                };
                let Some(kind) = crate::profiles::Kind::parse(&report.reference) else {
                    return err(
                        StatusCode::CONFLICT,
                        "a picture or a banner can be taken down; the rest of a profile is its owner's",
                    );
                };
                if let Err(resp) = moderated(&api, who, about) {
                    return *resp;
                }
                if let Err(e) = api.profiles.remove_image(about, kind) {
                    return error_response(e);
                }
            }
            _ => {}
        }
        match api
            .moderation
            .settle(&report.id, act, who.audit_name(), crate::login::now())
        {
            Ok(done) => HttpResponse::Ok().json(done),
            Err(e) => error_response(e),
        }
    })
}

/// A hold's `minutes` (absent or null: until lifted) and `reason`.
fn hold_of(doc: &serde_json::Value, by: &str, now: u64) -> Result<crate::moderation::Hold, String> {
    /// A year, the longest a hold runs by itself.
    const MAX_MINUTES: u64 = 366 * 24 * 60;
    let until = match doc.get("minutes") {
        None | Some(serde_json::Value::Null) => None,
        Some(v) => match v.as_u64() {
            Some(m) if (1..=MAX_MINUTES).contains(&m) => Some(now + m * 60),
            _ => {
                return Err(format!(
                    "minutes is 1 to {MAX_MINUTES}, or null until lifted"
                ))
            }
        },
    };
    Ok(crate::moderation::Hold {
        at: now,
        by: by.to_string(),
        until,
        reason: crate::moderation::clean_reason(
            doc.get("reason").and_then(|v| v.as_str()).unwrap_or(""),
        ),
    })
}

/// The `name` of a moderation request, the person it names, and the body.
fn moderation_target(
    api: &Api,
    who: &Principal,
    body: &[u8],
) -> Result<(String, serde_json::Value), Box<HttpResponse>> {
    let Some((name, doc)) = named(body) else {
        return Err(Box::new(err(
            StatusCode::BAD_REQUEST,
            r#"body must be JSON: {"name": "...", "minutes"?: n, "reason"?: "..."}"#,
        )));
    };
    moderated(api, who, &name)?;
    Ok((name, doc))
}

/// `POST /api/moderation/mute`: `{"name", "minutes"?, "reason"?}`. They
/// keep reading; they cannot send mail or change their profile.
async fn post_moderation_mute(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded_as(&api, &req, "/api/moderation/mute", true, |who| {
        let (name, doc) = match moderation_target(&api, who, &body) {
            Ok(t) => t,
            Err(resp) => return *resp,
        };
        let hold = match hold_of(&doc, who.audit_name(), crate::login::now()) {
            Ok(h) => h,
            Err(e) => return err(StatusCode::BAD_REQUEST, &e),
        };
        match api.moderation.mute(&name, hold.clone()) {
            Ok(()) => HttpResponse::Ok().json(serde_json::json!({ "name": name, "muted": hold })),
            Err(e) => error_response(e),
        }
    })
}

/// `POST /api/moderation/unmute`: `{"name"}`.
async fn post_moderation_unmute(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded_as(&api, &req, "/api/moderation/unmute", true, |who| {
        let (name, _) = match moderation_target(&api, who, &body) {
            Ok(t) => t,
            Err(resp) => return *resp,
        };
        match api
            .moderation
            .unmute(&name, who.audit_name(), crate::login::now())
        {
            Ok(()) => HttpResponse::Ok().json(serde_json::json!({ "name": name, "muted": null })),
            Err(e) => error_response(e),
        }
    })
}

/// `POST /api/moderation/suspend`: `{"name", "minutes"?, "reason"?}`. LosOS
/// Login stops letting them in at their next click; nothing of theirs is
/// deleted.
async fn post_moderation_suspend(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded_as(&api, &req, "/api/moderation/suspend", true, |who| {
        let (name, doc) = match moderation_target(&api, who, &body) {
            Ok(t) => t,
            Err(resp) => return *resp,
        };
        let hold = match hold_of(&doc, who.audit_name(), crate::login::now()) {
            Ok(h) => h,
            Err(e) => return err(StatusCode::BAD_REQUEST, &e),
        };
        let by = who.person().map(|p| p.name.as_str());
        if let Err(e) = api.people.set_disabled(&name, true, by) {
            return error_response(e);
        }
        match api.moderation.suspend(&name, hold.clone()) {
            Ok(()) => {
                HttpResponse::Ok().json(serde_json::json!({ "name": name, "suspended": hold }))
            }
            Err(e) => error_response(e),
        }
    })
}

/// `POST /api/moderation/unsuspend`: `{"name"}`.
async fn post_moderation_unsuspend(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded_as(&api, &req, "/api/moderation/unsuspend", true, |who| {
        let (name, _) = match moderation_target(&api, who, &body) {
            Ok(t) => t,
            Err(resp) => return *resp,
        };
        if let Err(e) = api.people.set_disabled(&name, false, None) {
            return error_response(e);
        }
        match api
            .moderation
            .unsuspend(&name, who.audit_name(), crate::login::now())
        {
            Ok(()) => {
                HttpResponse::Ok().json(serde_json::json!({ "name": name, "suspended": null }))
            }
            Err(e) => error_response(e),
        }
    })
}

/// `POST /api/account/code`: `{"password"}`. The one-time code LosOS Login
/// last sent the caller, which it asks for before they add or remove an
/// authenticator app or a passkey. The box has no mail, so this is where
/// the code arrives, behind their password.
async fn post_account_code(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    let remote = remote_addr(&req);
    guarded_as(&api, &req, "/api/account/code", true, |who| {
        let Some(person) = who.person() else {
            return err(
                StatusCode::CONFLICT,
                "the spare key has no account of its own",
            );
        };
        if !api.people.headers {
            return not_standalone();
        }
        let Some(password) = serde_json::from_slice::<serde_json::Value>(&body)
            .ok()
            .and_then(|d| {
                d.get("password")
                    .and_then(|p| p.as_str())
                    .map(str::to_string)
            })
        else {
            return err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"password": "..."}"#,
            );
        };
        let secret = match crate::signin::validate_candidate(&password) {
            Ok(s) => s,
            Err(e) => return err(StatusCode::BAD_REQUEST, &e),
        };
        match api.people.check_password(&person.name, &secret) {
            Ok(true) => {}
            Ok(false) => {
                api.throttle.record_failure(&remote, Instant::now());
                return HttpResponse::Forbidden().json(serde_json::json!({
                    "error": "that is not your password",
                    "wrongPassword": true,
                }));
            }
            Err(e) => return error_response(e),
        }
        let email = api
            .people
            .list()
            .ok()
            .and_then(|all| all.into_iter().find(|p| p.name == person.name))
            .map(|p| p.email)
            .unwrap_or_default();
        match api.login.code_for(&email) {
            Some(code) => HttpResponse::Ok().json(serde_json::json!({ "code": code })),
            None => HttpResponse::NotFound().json(serde_json::json!({
                "error": "LosOS Login has not made a code for you yet",
                "noCode": true,
            })),
        }
    })
}

/// The caller's Windows sign-in, for `GET` and `POST /api/windows`.
fn windows_answer(api: &Api, name: &str) -> HttpResponse {
    let host = std::fs::read_to_string("/proc/sys/kernel/hostname").unwrap_or_default();
    match api.people.windows_status(name) {
        Ok(None) => HttpResponse::Ok().json(serde_json::json!({ "on": false })),
        Ok(Some(status)) => HttpResponse::Ok().json(serde_json::json!({
            "on": true,
            "ready": status.ready,
            "share": status.share,
            "host": host.trim(),
        })),
        Err(e) => error_response(e),
    }
}

/// `GET /api/windows`: whether the box's Windows shares are on
/// (`crate::windows`), and whether Windows takes the caller's LosOS
/// password yet.
async fn get_windows(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded_as(&api, &req, "/api/windows", false, |who| {
        match who.person() {
            Some(person) => windows_answer(&api, &person.name),
            None => HttpResponse::Ok().json(serde_json::json!({ "on": api.people.windows_on() })),
        }
    })
}

/// `POST /api/windows`: `{"password"}`. A person whose password this daemon
/// has not seen in the clear since the shares came on types it once, and
/// Windows takes it from then on.
async fn post_windows(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    let remote = remote_addr(&req);
    guarded_as(&api, &req, "/api/windows", true, |who| {
        let Some(person) = who.person() else {
            return err(
                StatusCode::CONFLICT,
                "the spare key has no account of its own",
            );
        };
        if !api.people.windows_on() {
            return err(StatusCode::CONFLICT, "the Windows share is off");
        }
        let Some(password) = serde_json::from_slice::<serde_json::Value>(&body)
            .ok()
            .and_then(|d| {
                d.get("password")
                    .and_then(|p| p.as_str())
                    .map(str::to_string)
            })
        else {
            return err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"password": "..."}"#,
            );
        };
        let secret = match crate::signin::validate_candidate(&password) {
            Ok(s) => s,
            Err(e) => return err(StatusCode::BAD_REQUEST, &e),
        };
        match api.people.check_password(&person.name, &secret) {
            Ok(true) => windows_answer(&api, &person.name),
            Ok(false) => {
                api.throttle.record_failure(&remote, Instant::now());
                HttpResponse::Forbidden().json(serde_json::json!({
                    "error": "that is not your password",
                    "wrongPassword": true,
                }))
            }
            Err(e) => error_response(e),
        }
    })
}

/// `POST /api/people/rules`: `{"twoFactor": bool, "passwordDays": n}`.
async fn post_people_rules(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded_as(&api, &req, "/api/people/rules", true, |who| {
        if !api.people.headers {
            return not_standalone();
        }
        let Ok(rules) = serde_json::from_slice::<crate::login::Rules>(&body) else {
            return err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"twoFactor": true|false, "passwordDays": 0..1200}"#,
            );
        };
        if rules.password_days > crate::login::MAX_PASSWORD_DAYS {
            return err(
                StatusCode::BAD_REQUEST,
                &format!(
                    "a password can be kept for at most {} days",
                    crate::login::MAX_PASSWORD_DAYS
                ),
            );
        }
        // Whoever turns two-factor sign-in on has a second factor already,
        // or the rule would hold them to their own account the moment it
        // took. The spare key has no account to hold.
        if let Some(person) = who.person() {
            if rules.two_factor
                && !api.login.rules().two_factor
                && !api.login.factors(&person.name).any()
            {
                return HttpResponse::Conflict().json(serde_json::json!({
                    "error": "add an authenticator app or a passkey to your own account first",
                    "needsOwnFactor": true,
                }));
            }
        }
        match api.login.set_rules(rules) {
            Ok(()) => {
                reconcile_login(&api);
                HttpResponse::Ok().json(rules)
            }
            Err(e) => error_response(e),
        }
    })
}

/// `POST /api/people/two-factor/reset`: `{"name"}`. Remove the person's
/// authenticator app and passkeys, for someone who lost the phone or key.
async fn post_person_factors_reset(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded(&api, &req, "/api/people/two-factor/reset", true, || {
        if !api.people.headers {
            return not_standalone();
        }
        let Some((name, _)) = named(&body) else {
            return err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"name": "..."}"#,
            );
        };
        let known = api
            .people
            .list()
            .is_ok_and(|all| all.iter().any(|p| p.name == name));
        if !known {
            return error_response(crate::people::Refused::Unknown(name).into());
        }
        match api.login.reset_factors(&name) {
            Ok(()) => {
                reconcile_login(&api);
                HttpResponse::Ok().json(serde_json::json!({ "name": name, "reset": true }))
            }
            Err(e) => error_response(e),
        }
    })
}

/// `GET /internal/cloud-share`: whose request to LosOS Cloud's share this
/// is, for nginx (`crate::cloud_share`). Always a 200; the person, if any,
/// in the headers nginx reads from Authelia's answers too. Only with
/// LosOS Login on: without it nginx does not ask, and nobody is named.
async fn get_cloud_share(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    if !api.people.headers {
        return HttpResponse::Ok().finish();
    }
    let authorization = req
        .headers()
        .get(actix_web::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let api = api.clone();
    let answer = web::block(move || {
        let mut backend = api.backend.clone();
        api.share.answer(
            &api.people,
            &mut backend,
            authorization.as_deref(),
            Instant::now(),
        )
    })
    .await;
    match answer {
        Ok(crate::cloud_share::Answer::Person(p)) => HttpResponse::Ok()
            .insert_header(("Remote-User", p.name.as_str()))
            .insert_header(("Remote-Groups", p.groups.join(",")))
            .insert_header(("Remote-Name", p.display_name.as_str()))
            .finish(),
        _ => HttpResponse::Ok().finish(),
    }
}

// ── LosOS Chat (crate::chat) ─────────────────────────────────────────────

/// A refusal from chat, as `{"error": ...}` at its own status.
fn chat_refused(e: &crate::chat::ChatError) -> HttpResponse {
    if !matches!(
        e,
        crate::chat::ChatError::NotForYou(_) | crate::chat::ChatError::Off
    ) {
        tracing::warn!(error = %e, "chat");
    }
    err(
        StatusCode::from_u16(e.status()).unwrap_or(StatusCode::BAD_GATEWAY),
        &e.to_string(),
    )
}

/// `GET /api/chat`: whether the box runs chat, and its server name.
async fn get_chat(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    let mut asked = false;
    let refused = guarded(&api, &req, "/api/chat", false, || {
        asked = true;
        HttpResponse::Ok().finish()
    });
    if !asked {
        return refused;
    }
    if !api.chat.live() {
        return chat_refused(&crate::chat::ChatError::Off);
    }
    // Every call below waits on the homeserver: off the server's workers, or
    // a slow answer holds up every request on the same worker, nginx's
    // /internal/chat/authz included.
    let api = api.clone();
    match web::block(move || api.chat.view()).await {
        Ok(view) => HttpResponse::Ok().json(view),
        Err(_) => HttpResponse::ServiceUnavailable().finish(),
    }
}

/// The person LosOS Login signed in, for the chat routes; the refusal
/// otherwise (chat is a person's, never the spare key's or the edge's).
fn chat_person(
    api: &web::Data<Api>,
    req: &HttpRequest,
    route: &str,
    writes: bool,
) -> Result<crate::people::Person, Box<HttpResponse>> {
    let mut person = None;
    let refused = guarded_as(api, req, route, writes, |who| match who.person() {
        Some(p) => {
            person = Some(p.clone());
            HttpResponse::Ok().finish()
        }
        None => chat_refused(&crate::chat::ChatError::NotForYou(
            "sign in with LosOS Login to use chat".to_string(),
        )),
    });
    person.ok_or_else(|| Box::new(refused))
}

/// `GET /api/chat/people`: everyone on the box chat can reach, each with
/// an account made, for "Message someone" and invites. A person only.
async fn get_chat_people(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    if let Err(refused) = chat_person(&api, &req, "/api/chat/people", false) {
        return *refused;
    }
    let api = api.clone();
    let answer = web::block(move || {
        let everyone = api.people.list().map_err(|e| e.to_string())?;
        Ok::<_, String>(api.chat.people(&everyone))
    })
    .await;
    match answer {
        Ok(Ok(Ok(list))) => HttpResponse::Ok().json(serde_json::json!({ "people": list })),
        Ok(Ok(Err(e))) => chat_refused(&e),
        Ok(Err(e)) => HttpResponse::InternalServerError().json(serde_json::json!({ "error": e })),
        Err(_) => HttpResponse::ServiceUnavailable().finish(),
    }
}

/// `POST /api/chat/session` `{"device"?: "...", "deviceName"?: "..."}`:
/// a device signed in to chat for the person LosOS Login signed in. Never
/// for the spare key or the edge: chat is a person's.
async fn post_chat_session(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    let person = match chat_person(&api, &req, "/api/chat/session", true) {
        Ok(p) => p,
        Err(refused) => return *refused,
    };
    let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
    let field = |k: &str| {
        doc.get(k)
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    };
    let (device, device_name) = (field("device"), field("deviceName"));
    let api = api.clone();
    let made = web::block(move || {
        api.chat
            .session(&person, device.as_deref(), device_name.as_deref())
    })
    .await;
    match made {
        Ok(Ok(s)) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(serde_json::json!({
                "userId": s.user_id,
                "accessToken": s.access_token,
                "deviceId": s.device_id,
                "homeserver": "/",
            })),
        Ok(Err(e)) => chat_refused(&e),
        Err(_) => HttpResponse::ServiceUnavailable().finish(),
    }
}

/// `POST /api/chat/openid`: a Matrix OpenID token for the person LosOS Login
/// signed in, for another box to learn their chat address from
/// (`crate::chat::Chat::openid`). The chat page's "tell that box who you
/// are" asks for it; never the spare key or the edge.
async fn post_chat_openid(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    let person = match chat_person(&api, &req, "/api/chat/openid", true) {
        Ok(p) => p,
        Err(refused) => return *refused,
    };
    let api = api.clone();
    match web::block(move || api.chat.openid(&person)).await {
        Ok(Ok(token)) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(token),
        Ok(Err(e)) => chat_refused(&e),
        Err(_) => HttpResponse::ServiceUnavailable().finish(),
    }
}

/// `POST /api/chat/git-room` `{"owner": "...", "repo": "..."}`: give a
/// LosOS Git repository a chat room (`crate::git_rooms`). Only an
/// administrator of the repository, asked of LosOS Git as that person. A
/// repository that has its room gets the same one back.
async fn post_chat_git_room(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    let person = match chat_person(&api, &req, "/api/chat/git-room", true) {
        Ok(p) => p,
        Err(refused) => return *refused,
    };
    let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
    let field = |k: &str| {
        doc.get(k)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let (owner, repo) = (field("owner"), field("repo"));
    if !crate::git_rooms::plain_name(&owner) || !crate::git_rooms::plain_name(&repo) {
        return err(
            StatusCode::BAD_REQUEST,
            r#"body must be JSON: {"owner": "...", "repo": "..."}"#,
        );
    }
    if !api.chat.live() {
        return chat_refused(&crate::chat::ChatError::Off);
    }
    let api = api.clone();
    let made = web::block(move || git_room(&api, &person, &owner, &repo)).await;
    match made {
        Ok(Ok(room)) => HttpResponse::Ok().json(serde_json::json!({ "room": room })),
        Ok(Err((status, message))) => err(status, &message),
        Err(_) => HttpResponse::ServiceUnavailable().finish(),
    }
}

fn git_room(
    api: &Api,
    person: &crate::people::Person,
    owner: &str,
    repo: &str,
) -> Result<String, (StatusCode, String)> {
    use crate::config_repo::{classify, Answer, GitOp};
    use crate::losos::Losos;
    let login = if person.groups.iter().any(|g| g == crate::people::OWNER) {
        crate::app_names::OWNER_ACCOUNT.to_string()
    } else {
        person.name.clone()
    };
    let not_up = |e: anyhow::Error| {
        tracing::warn!(error = ?e, "LosOS Git for a chat room");
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "LosOS Git is not answering; try again in a moment.".to_string(),
        )
    };
    let mut backend = api.backend.clone();
    let (status, body) = backend
        .git_request(&GitOp::RepoAs {
            login: login.clone(),
            owner: owner.to_string(),
            name: repo.to_string(),
        })
        .map_err(not_up)?;
    let doc = match classify(status, &body) {
        Answer::Ok(doc) => doc,
        Answer::Missing => {
            return Err((StatusCode::NOT_FOUND, "No such repository.".to_string()));
        }
        Answer::Exists | Answer::Refused { .. } => {
            return Err((
                StatusCode::BAD_GATEWAY,
                format!("LosOS Git answered {status}."),
            ));
        }
    };
    if doc["permissions"]["admin"] != serde_json::json!(true) {
        return Err((
            StatusCode::FORBIDDEN,
            "Only an administrator of the repository can give it a chat room.".to_string(),
        ));
    }
    let linked = doc["chat_room"].as_str().unwrap_or("");
    let room = match api.git_rooms.get(owner, repo) {
        Some(room) if room == linked => return Ok(room),
        // Made before, unlinked since: link the same room again.
        Some(room) => room,
        None => {
            let server = api.chat.server_name().unwrap_or_default();
            let made = api.chat.git_room(
                person,
                &crate::chat::RepoRoom {
                    name: repo.to_string(),
                    url: format!("https://{server}/git/{owner}/{repo}"),
                    public: doc["visibility"] == "public",
                },
            );
            let room = made.map_err(|e| {
                (
                    StatusCode::from_u16(e.status()).unwrap_or(StatusCode::BAD_GATEWAY),
                    e.to_string(),
                )
            })?;
            api.git_rooms.set(owner, repo, &room).map_err(|e| {
                tracing::warn!(error = ?e, "the chat room of a repository was not kept");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "The room was made but could not be kept.".to_string(),
                )
            })?;
            room
        }
    };
    let (status, body) = backend
        .git_request(&GitOp::SetChatRoom {
            login,
            owner: owner.to_string(),
            name: repo.to_string(),
            room: room.clone(),
        })
        .map_err(not_up)?;
    match classify(status, &body) {
        Answer::Ok(_) => Ok(room),
        _ => Err((
            StatusCode::BAD_GATEWAY,
            format!("LosOS Git would not link the room ({status})."),
        )),
    }
}

/// Take LosOS Git's events and post those of a repository with a room
/// there (`crate::git_rooms`). Only where chat runs and `modules/chat.nix`
/// names the socket.
fn listen_for_git_events(api: &web::Data<Api>) {
    let Some(path) = std::env::var_os("LOSOS_GIT_CHAT_EVENTS_SOCKET") else {
        return;
    };
    if !api.chat.live() {
        return;
    }
    let api = api.clone();
    let started = crate::git_rooms::listen(std::path::Path::new(&path), move |event| {
        git_event(&api, &event);
    });
    if let Err(e) = started {
        tracing::warn!(error = ?e, "LosOS Git's events will not reach chat rooms");
    }
}

fn git_event(api: &Api, event: &serde_json::Value) {
    let text = |k: &str| {
        event
            .get(k)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
    };
    let room = text("chatRoom");
    if room.is_empty() || api.git_rooms.get(text("owner"), text("repo")).as_deref() != Some(room) {
        return;
    }
    let Some(server) = api.chat.server_name() else {
        return;
    };
    let owner = api
        .people
        .list()
        .ok()
        .and_then(|all| {
            all.into_iter()
                .find(|p| p.groups.iter().any(|g| g == crate::people::OWNER))
        })
        .map(|p| format!("@{}:{server}", p.name))
        .unwrap_or_default();
    let who = |login: &str| {
        crate::git_rooms::address(login, server, crate::app_names::OWNER_ACCOUNT, &owner)
    };
    let Some(notice) = crate::git_rooms::notice(event, &format!("https://{server}"), &who) else {
        return;
    };
    if let Err(e) = api.chat.git_notice(room, &notice) {
        tracing::warn!(error = %e, room, "a LosOS Git notice was not posted");
    }
}

/// `GET /internal/chat/authz`: nginx's question before every request to the
/// homeserver's client and media APIs. 200 goes on, 401 is "sign in with
/// LosOS Login", 403 is "not yours". nginx alone asks it (`modules/chat.nix`).
async fn get_chat_authz(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    let header = |k: &str| {
        req.headers()
            .get(k)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    let asked = crate::chat::Asked {
        cookie: header("cookie"),
        original_url: header("x-original-url").unwrap_or_default(),
        original_method: header("x-original-method").unwrap_or_default(),
        forwarded_for: header("x-forwarded-for").unwrap_or_default(),
    };
    let token = crate::chat::token_of(header("authorization").as_deref(), &asked.original_url);
    let api = api.clone();
    let verdict = web::block(move || {
        let now = crate::login::now();
        let muted = |name: &str| api.moderation.muted(name, now).is_some();
        api.chat
            .authorize(&api.people, &asked, token.as_deref(), &muted)
    })
    .await;
    match verdict {
        Ok(Ok(crate::chat::Verdict::Allow)) => HttpResponse::Ok().finish(),
        Ok(Ok(crate::chat::Verdict::SignIn)) => HttpResponse::Unauthorized().finish(),
        Ok(Ok(crate::chat::Verdict::Refuse)) => HttpResponse::Forbidden().finish(),
        Ok(Err(crate::chat::ChatError::Off)) => HttpResponse::NotFound().finish(),
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "chat authz");
            HttpResponse::ServiceUnavailable().finish()
        }
        Err(_) => HttpResponse::ServiceUnavailable().finish(),
    }
}

// ── LosOS Contacts' people pickers (crate::contacts) ──────────────────────
//
// The book is LosOS Cloud's (`/cloud/api/contacts/`, crate::cloud::contacts);
// lososd answers only the pickers, which also offer the people on the box.
// `signed-in` in sections.json: the spare key and the edge have no book.

/// Everyone on this box, as people pickers offer them: their chat ID where
/// chat runs, their address where mail does.
fn contacts_neighbours(api: &Api) -> Vec<crate::contacts::Neighbour> {
    let Ok(everyone) = api.people.list() else {
        return Vec::new();
    };
    let chat = api.chat.live();
    everyone
        .into_iter()
        .filter(|p| !p.disabled && !p.groups.iter().any(|g| g == crate::people::LOCKED))
        .map(|p| {
            let shown = crate::profiles::render(&p, &api.profiles.get(&p.name).unwrap_or_default());
            crate::contacts::Neighbour {
                email: api.mail.address(&p.name),
                picture: shown["picture"].as_str().map(str::to_string),
                matrix_id: chat.then(|| api.chat.user_id(&p.name).ok()).flatten(),
                display_name: p.display_name,
                name: p.name,
            }
        })
        .collect()
}

/// `GET /api/contacts/suggest?q=&want=any|email|matrix|mesh|box&limit=8`:
/// the people a picker offers, the caller's contacts (from LosOS Cloud)
/// and the people on this box, best match first. `{"contacts": [...]}`.
async fn get_contacts_suggest(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    let mut person = None;
    let refused = guarded_as(
        &api,
        &req,
        "/api/contacts/suggest",
        false,
        |who| match who.person() {
            Some(p) => {
                person = Some(p.clone());
                HttpResponse::Ok().finish()
            }
            None => err(
                StatusCode::FORBIDDEN,
                "sign in with LosOS Login to use your contacts",
            ),
        },
    );
    let Some(person) = person else {
        return refused;
    };
    let q = query_map(&req);
    let text = q.get("q").cloned().unwrap_or_default();
    let want = crate::cloud::contacts::Want::parse(q.get("want").map_or("", String::as_str));
    let limit = q
        .get("limit")
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(8);
    let worker = api.clone();
    let found = web::block(move || {
        let book = if worker.contacts {
            // The owner's LosOS Cloud account is the owner's own
            // (`modules/app-sign-in.nix`); everyone else's is their name.
            let login = if person.groups.iter().any(|g| g == crate::people::OWNER) {
                crate::app_names::OWNER_ACCOUNT.to_string()
            } else {
                person.name.clone()
            };
            crate::contacts::book_matches(&mut worker.backend.clone(), &login, &text, want)
        } else {
            crate::cloud::contacts::Matches::default()
        };
        let neighbours = contacts_neighbours(&worker);
        crate::contacts::suggest(book, &neighbours, &person.name, &text, want, limit)
    })
    .await;
    match found {
        Ok(found) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(serde_json::json!({ "contacts": found })),
        Err(_) => HttpResponse::ServiceUnavailable().finish(),
    }
}

async fn health() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({ "ok": true }))
}

async fn get_state(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/state", false, || run(&api, cmd_state))
}

async fn get_settings(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/settings", false, || {
        run(&api, cmd_settings)
    })
}

async fn get_status(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/status", false, || run(&api, cmd_status))
}

/// What the daemon found when it last looked for an edge proxy.
async fn get_edge(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/edge", false, || run(&api, cmd_edge))
}

/// The owner lets the company edge in use manage LosOS Cloud, LosOS Git and
/// the apps, or stops it: `{"enabled": bool}`.
async fn post_edge_manage(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    guarded(&api, &req, "/api/edge/manage", true, || {
        let Some(enabled) = serde_json::from_slice::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("enabled").and_then(serde_json::Value::as_bool))
        else {
            return err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"enabled": true|false}"#,
            );
        };
        let resp = run(&api, |l| crate::losos::cmd_edge_manage(l, enabled));
        api.backend.publish_managed();
        resp
    })
}

/// The owner moves the box to a secondary cluster in reach, or lets it settle
/// anew: `{"cluster": "office"}` or `{"cluster": null}`.
async fn post_edge_cluster(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded(&api, &req, "/api/edge/cluster", true, || {
        let parsed = serde_json::from_slice::<serde_json::Value>(&body).ok();
        let cluster = match parsed.as_ref().and_then(|v| v.get("cluster")) {
            Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(c)) if crate::edge::cluster_name(c) => Some(c.clone()),
            _ => {
                return err(
                    StatusCode::BAD_REQUEST,
                    r#"body must be JSON: {"cluster": "<name>"|null}"#,
                )
            }
        };
        run(&api, |l| {
            crate::losos::cmd_edge_cluster(l, cluster.as_deref())
        })
    })
}

async fn post_change(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    guarded(&api, &req, "/api/change", true, || {
        post_change_inner(&api, &body)
    })
}

fn post_change_inner(api: &Api, body: &[u8]) -> HttpResponse {
    let mode = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("mode").and_then(|m| m.as_str()).map(str::to_string));
    let Some(mode) = mode else {
        return err(
            StatusCode::BAD_REQUEST,
            r#"body must be JSON: {"mode": "local|mesh"}"#,
        );
    };
    let Some(mode) = Mode::parse(&mode) else {
        return err(StatusCode::BAD_REQUEST, "mode must be 'local' or 'mesh'");
    };
    run(api, |b| cmd_change(b, mode))
}

async fn post_apply(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    guarded(&api, &req, "/api/apply", true, || {
        post_apply_inner(&api, &body)
    })
}

fn post_apply_inner(api: &Api, body: &[u8]) -> HttpResponse {
    let Ok(text) = String::from_utf8(body.to_vec()) else {
        return err(StatusCode::BAD_REQUEST, "body must be UTF-8 Nix code");
    };
    match validate_apply(&text) {
        Err(e) => err(StatusCode::BAD_REQUEST, e),
        Ok(code) => {
            let code = code.to_string();
            run(api, |b| cmd_apply(b, &code))
        }
    }
}

async fn post_factory_reset(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/factory-reset", true, || {
        run(&api, cmd_factory_reset)
    })
}

// ── Backups and erasing the box (crate::backup, crate::erase) ──────────

/// `GET /api/backup`: the bucket without its secret, the last backup, the
/// job running now, and an erase under way.
async fn get_backup(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/backup", false, || {
        run(&api, crate::erase::cmd_backup)
    })
}

/// `GET /api/credentials`: every credential the box keeps, without one
/// value among them (`crate::credentials`).
async fn get_credentials(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/credentials", false, || {
        match (api.credentials.key(), api.credentials.list()) {
            (Ok(key), Ok(list)) => HttpResponse::Ok().json(serde_json::json!({
                "key": key,
                "credentials": list,
            })),
            (Err(e), _) | (_, Err(e)) => {
                tracing::error!(error = %e, "listing the credentials");
                err(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "the credential store could not be read",
                )
            }
        }
    })
}

/// `POST /api/credentials/{name}/rotate`: make a new value for a credential
/// the box makes itself, and restart what reads it. The value is made and
/// sealed here; the answer carries the row, never the value.
async fn post_credential_rotate(
    api: web::Data<Api>,
    req: HttpRequest,
    path: web::Path<String>,
) -> HttpResponse {
    let name = path.into_inner();
    guarded(
        &api,
        &req,
        "/api/credentials/{name}/rotate",
        true,
        || match api.credentials.rotate(&name) {
            Ok(Ok(row)) => {
                tracing::info!(credential = %name, "made a new value for a credential");
                HttpResponse::Ok().json(serde_json::json!({ "credential": row }))
            }
            Ok(Err(refused @ crate::credentials::Refused::NotFound)) => {
                err(StatusCode::NOT_FOUND, &refused.to_string())
            }
            Ok(Err(refused)) => err(StatusCode::CONFLICT, &refused.to_string()),
            Err(e) => {
                tracing::error!(error = %e, credential = %name, "making a new credential value");
                err(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "the new value could not be sealed",
                )
            }
        },
    )
}

/// `POST /api/backup/target`: set the bucket.
///
/// Raw bytes rather than actix's JSON extractor, for the reason
/// `post_set_password` gives: the extractor's rejection would render the
/// body, and this body carries the bucket's secret key.
async fn post_backup_target(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded(
        &api,
        &req,
        "/api/backup/target",
        true,
        || match serde_json::from_slice::<crate::backup::TargetInput>(&body) {
            Ok(input) => run(&api, |b| crate::erase::cmd_backup_target(b, input)),
            Err(_) => err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"endpoint", "bucket", "prefix", "region", "accessKeyId", "secretAccessKey"}"#,
            ),
        },
    )
}

/// `POST /api/backup/target/bought`: back up to storage this box bought on
/// the mesh, `{"order": "ord_…"}`. Relayed: the door's key is set at the
/// edge first.
async fn post_backup_target_bought(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
    match field_str(&doc, "order") {
        Some(order) => {
            relayed(&api, &req, "/api/backup/target/bought", true, move |b| {
                crate::erase::cmd_backup_target_bought(b, &order)
            })
            .await
        }
        None => guarded(&api, &req, "/api/backup/target/bought", true, || {
            err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"order": "ord_…"}"#,
            )
        }),
    }
}

/// `DELETE /api/backup/target`: forget the bucket; the backups in it stay.
async fn delete_backup_target(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/backup/target", true, || {
        run(&api, crate::erase::cmd_backup_target_clear)
    })
}

/// `POST /api/backup/run`: back up now.
async fn post_backup_run(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/backup/run", true, || {
        run(&api, crate::erase::cmd_backup_run)
    })
}

/// `POST /api/backup/restore` `{"code": "<recovery code>"}`: pull the
/// latest backup back. Raw bytes for the same reason as the target.
async fn post_backup_restore(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded(&api, &req, "/api/backup/restore", true, || {
        let code = serde_json::from_slice::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("code").and_then(|c| c.as_str()).map(str::to_string));
        match code {
            Some(code) => run(&api, |b| crate::erase::cmd_restore(b, &code)),
            None => err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"code": "<recovery code>"}"#,
            ),
        }
    })
}

/// `POST /api/erase` `{"backup": true|false}`: start erasing the box.
async fn post_erase(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    guarded(&api, &req, "/api/erase", true, || {
        let backup = serde_json::from_slice::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("backup").and_then(serde_json::Value::as_bool));
        match backup {
            Some(backup) => run(&api, |b| crate::erase::cmd_erase(b, backup)),
            None => err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"backup": true|false}"#,
            ),
        }
    })
}

/// `POST /api/erase/cancel`: stop an erase before it reaches the edge.
async fn post_erase_cancel(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/erase/cancel", true, || {
        run(&api, crate::erase::cmd_erase_cancel)
    })
}

/// Extend `/persist` into the volume group's free extents.
///
/// Synchronous, unlike every other POST here: it is three short-lived commands
/// rather than a supervised rebuild, so it returns the measured before/after
/// sizes instead of a job id to poll.
async fn get_storage(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/storage", false, || run(&api, cmd_storage))
}

async fn post_grow(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/grow", true, || run(&api, cmd_grow))
}

/// `GET /api/setup/claim` — has this box got an owner yet? No token required.
///
/// The page has to ask this before it can decide whether to show the wizard or
/// the sign-in dialog, and on an unclaimed box there is no token to ask with.
///
/// `people` says whether anyone can sign in as an administrator yet. A box
/// claimed before sign-in went through Authelia has nobody, and its owner
/// makes the first account with the spare key ([`post_claim`]).
async fn get_claim_state(api: web::Data<Api>) -> HttpResponse {
    let people = api.people.has_admin().unwrap_or(false);
    run(&api, move |l| {
        let mut v = crate::losos::cmd_claim_state(l)?;
        v["people"] = serde_json::Value::Bool(people);
        Ok(v)
    })
}

/// `POST /api/setup/claim` — set the first password and take ownership.
///
/// Unauthenticated **while the box is unclaimed, and never after**. See
/// [`crate::losos::cmd_claim`] for why this window exists at all: the admin
/// token is minted into a 0600 file on a box with no shell, so without it a
/// fresh appliance cannot be administered by anyone.
///
/// The token is handed back in the reply so the page can continue
/// authenticated. That is the same secret the gate checks, released exactly
/// once, to the caller who just proved they were on the LAN during the window
/// and set the owner password. After that this route refuses every call, so
/// the token cannot be re-fetched by a later visitor.
async fn post_claim(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    const SHAPE: &str =
        r#"body must be JSON: {"name": "...", "password": "...", "user": "..." (optional)}"#;
    let header = |name: &str| req.headers().get(name).and_then(|v| v.to_str().ok());
    let hostname = std::fs::read_to_string("/proc/sys/kernel/hostname").unwrap_or_default();
    if let Some(why) = claim_fault(
        header("content-type"),
        header("host"),
        header("origin"),
        hostname.trim(),
    ) {
        tracing::warn!(reason = why, "refused a claim request");
        return err(StatusCode::FORBIDDEN, why);
    }
    let Ok(doc) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return err(StatusCode::BAD_REQUEST, SHAPE);
    };
    let user = match doc.get("user") {
        None | Some(serde_json::Value::Null) => crate::setup::DEFAULT_ADMIN_USER.to_string(),
        Some(serde_json::Value::String(u)) => u.clone(),
        Some(_) => return err(StatusCode::BAD_REQUEST, SHAPE),
    };
    let Some(password) = doc.get("password").and_then(|p| p.as_str()) else {
        return err(StatusCode::BAD_REQUEST, SHAPE);
    };
    // The first person's sign-in name. Checked before anything runs, so a
    // bad one costs nothing.
    let name = match doc.get("name") {
        None | Some(serde_json::Value::Null) => "admin".to_string(),
        Some(serde_json::Value::String(n)) => n.trim().to_string(),
        Some(_) => return err(StatusCode::BAD_REQUEST, SHAPE),
    };
    if let Err(e) = crate::people::validate_name(&name) {
        return err(StatusCode::BAD_REQUEST, &e);
    }
    if let Err(e) = crate::setup::validate_password(password) {
        return err(StatusCode::BAD_REQUEST, &e);
    }
    let email = format!("{name}@{}.local", hostname.trim());
    // A box claimed before people signed in through Authelia: the spare key
    // makes the first account, once.
    if authorized(&api, &req) {
        return match api.people.has_admin() {
            Ok(true) => err(
                StatusCode::CONFLICT,
                "this box already has an administrator",
            ),
            Ok(false) => {
                let secret = crate::setup::validate_password(password).expect("checked above");
                let people = api.clone();
                run(&api, move |l| {
                    people.people.add(
                        &name,
                        &name,
                        &email,
                        &secret,
                        &[crate::people::ADMINS, crate::people::OWNER],
                    )?;
                    remember_owner(l, &name)?;
                    Ok(serde_json::json!({ "name": name }))
                })
            }
            Err(e) => error_response(e),
        };
    }
    let token = api.token.clone();
    let claims = api.clone();
    run(&api, move |l| {
        // Held across the claim on purpose: two claims racing each other
        // would otherwise both see an unclaimed box.
        let mut receipts = claims
            .claims
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut out = crate::losos::cmd_claim(l, &user, password, &token, &mut receipts)?;
        if out.get("replayed").is_none() {
            // The first person on a box is its administrator. The password
            // is the one LosOS Cloud just took, so one password opens both.
            let secret =
                crate::setup::validate_password(password).map_err(|e| anyhow::anyhow!(e))?;
            claims.people.add(
                &name,
                &name,
                &email,
                &secret,
                &[crate::people::ADMINS, crate::people::OWNER],
            )?;
            remember_owner(l, &name)?;
        }
        out["name"] = serde_json::Value::String(name);
        Ok(out)
    })
}

/// Record which person owns the box's LosOS Cloud account, so their
/// password changes change that one too.
fn remember_owner(l: &mut IoLosos, name: &str) -> anyhow::Result<()> {
    use crate::losos::Losos;
    let mut state = l.load_state()?;
    state.owner = Some(name.to_string());
    l.save_state(&state)
}

/// Why a claim request must be refused before it is even parsed, if it must.
///
/// The claim route is the one state-changing route with no token, so the
/// LAN-only guard in nginx is all that stands in front of it, and that guard
/// cannot tell a person on the LAN from a web page *running in the browser of*
/// a person on the LAN. Three checks close the two ways a page can do this:
///
///   * **Content-Type must be `application/json`.** Without it a page can POST
///     the JSON body as `text/plain`, a "simple" request the browser sends
///     without a CORS preflight, and set the owner password on a box it has
///     never seen. Requiring the JSON type forces a preflight, and lososd
///     answers no CORS, so the browser never sends the real request.
///   * **Host must be this box**: its hostname, `<hostname>.local`, or an IP
///     literal. Under DNS rebinding the page's own domain resolves to the box,
///     the request is same-origin to the browser, and it could read the reply,
///     which carries the admin token. The Host header still names the
///     attacker's domain, and that is what this refuses.
///   * **Origin, when present, must match Host.** Belt and braces for the two
///     above: a browser always sends it on a cross-origin POST.
///
/// Requests with no Origin (curl, the VM tests) pass the third check; they
/// cannot come from a page.
fn claim_fault(
    content_type: Option<&str>,
    host: Option<&str>,
    origin: Option<&str>,
    hostname: &str,
) -> Option<&'static str> {
    let is_json = content_type
        .and_then(|ct| ct.split(';').next())
        .is_some_and(|essence| essence.trim().eq_ignore_ascii_case("application/json"));
    if !is_json {
        return Some("claim requests must be sent as application/json");
    }
    let Some(host) = host.map(host_part).filter(|h| !h.is_empty()) else {
        return Some("claim requests must name this box in Host");
    };
    let own_name = !hostname.is_empty()
        && (host.eq_ignore_ascii_case(hostname)
            || host
                .strip_suffix(".local")
                .is_some_and(|h| h.eq_ignore_ascii_case(hostname)));
    if !own_name && host.parse::<std::net::IpAddr>().is_err() {
        return Some("claim requests must name this box in Host");
    }
    if let Some(origin) = origin {
        let authority = origin
            .strip_prefix("https://")
            .or_else(|| origin.strip_prefix("http://"));
        let same = authority.is_some_and(|a| host_part(a).eq_ignore_ascii_case(host));
        if !same {
            return Some("claim requests must come from this box's own page");
        }
    }
    None
}

/// The host of a `Host` header or an origin's authority: no port, no
/// brackets around an IPv6 literal, no trailing dot.
fn host_part(authority: &str) -> &str {
    let a = authority.trim();
    if let Some(rest) = a.strip_prefix('[') {
        return rest.split(']').next().unwrap_or("");
    }
    let a = match a.rsplit_once(':') {
        Some((h, port)) if port.bytes().all(|b| b.is_ascii_digit()) => h,
        _ => a,
    };
    a.strip_suffix('.').unwrap_or(a)
}

async fn post_set_password(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded_as(&api, &req, "/api/set-password", true, |who| match who {
        Principal::Person(p) => post_own_password(&api, p, &body),
        Principal::Spare => post_set_password_inner(&api, &body),
        Principal::Edge => err(StatusCode::FORBIDDEN, "the edge has no password here"),
        Principal::Companion(_) => err(StatusCode::FORBIDDEN, "the LosOS companion only reads"),
        Principal::Backups(_) => err(StatusCode::FORBIDDEN, "backups do not change passwords"),
        Principal::Directory(_) => err(StatusCode::FORBIDDEN, "a joined computer only reads"),
    })
}

/// A person changing their own password: the one LosOS Login checks, and
/// for the person who owns the box (`State::owner`), their LosOS Git
/// account's too, so one password keeps opening both.
fn post_own_password(api: &Api, person: &crate::people::Person, body: &[u8]) -> HttpResponse {
    const SHAPE: &str = r#"body must be JSON: {"password": "..."}"#;
    let Some(password) = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|d| {
            d.get("password")
                .and_then(|p| p.as_str())
                .map(str::to_string)
        })
    else {
        return err(StatusCode::BAD_REQUEST, SHAPE);
    };
    let secret = match crate::setup::validate_password(&password) {
        Ok(s) => s,
        Err(e) => return err(StatusCode::BAD_REQUEST, &e),
    };
    if api
        .people
        .check_password(&person.name, &secret)
        .unwrap_or(false)
    {
        return err(
            StatusCode::BAD_REQUEST,
            "that is the password you have now; choose a new one",
        );
    }
    let name = person.name.clone();
    run(api, move |l| {
        use crate::losos::Losos;
        api.people.set_password(&name, &secret)?;
        note_password(api, &name);
        // The owner's LosOS Git account keeps the same password.
        if l.load_state()?.owner.as_deref() == Some(name.as_str()) {
            cmd_set_password(l, crate::setup::DEFAULT_ADMIN_USER, &password)?;
        }
        Ok(serde_json::json!({ "name": name, "changed": true }))
    })
}

fn post_set_password_inner(api: &Api, body: &[u8]) -> HttpResponse {
    const SHAPE: &str = r#"body must be JSON: {"user": "...", "password": "..."}"#;
    let Ok(doc) = serde_json::from_slice::<serde_json::Value>(body) else {
        return err(StatusCode::BAD_REQUEST, SHAPE);
    };
    // `user` is optional: the box has one owner account, and the wizard
    // should not have to know its name to reset it.
    let user = match doc.get("user") {
        None | Some(serde_json::Value::Null) => crate::setup::DEFAULT_ADMIN_USER.to_string(),
        Some(serde_json::Value::String(u)) => u.clone(),
        Some(_) => return err(StatusCode::BAD_REQUEST, SHAPE),
    };
    let Some(password) = doc.get("password").and_then(|p| p.as_str()) else {
        return err(StatusCode::BAD_REQUEST, SHAPE);
    };
    // Validated a second time inside the command; doing it here as well is what
    // turns "too short" into a 400 the wizard can show next to the field
    // instead of a 500 that says to read the journal.
    if let Err(e) = crate::setup::validate_password(password) {
        return err(StatusCode::BAD_REQUEST, &e);
    }
    if let Err(e) = crate::setup::validate_user(&user) {
        return err(StatusCode::BAD_REQUEST, &e);
    }
    let password = password.to_string();
    run(api, |b| {
        use crate::losos::Losos;
        let mut out = cmd_set_password(b, &user, &password)?;
        // The owner signs in to LosOS Login with it. A box claimed before
        // sign-in went through LosOS Login has no owner there yet; its
        // first account is made by the claim with the spare key.
        let owner = if user == crate::setup::DEFAULT_ADMIN_USER {
            b.load_state()?.owner
        } else {
            None
        };
        match owner {
            Some(owner) => {
                let secret =
                    crate::setup::validate_password(&password).map_err(|e| anyhow::anyhow!(e))?;
                api.people.set_password(&owner, &secret)?;
                note_password(api, &owner);
            }
            None => out["changed"] = serde_json::Value::Bool(false),
        }
        Ok(out)
    })
}

/// `POST /api/devices/backups`: LosOS Cloud and LosOS Git credentials of one
/// computer's own, for the backups LosOS Login signed in as a person
/// (`crate::losos::cmd_device_backups`). No password: the token is the
/// sign-in, with the person's second factor and consent behind it. A person
/// signed in through LosOS Login in the browser may ask too, for their own.
async fn post_device_backups(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    const SHAPE: &str = r#"body must be JSON: {"device": "..."}"#;
    guarded_as(&api, &req, "/api/devices/backups", true, |who| {
        let (Principal::Backups(person) | Principal::Person(person)) = who else {
            return err(
                StatusCode::FORBIDDEN,
                "backups sign in as a person, through LosOS Login",
            );
        };
        let Some(device) = serde_json::from_slice::<serde_json::Value>(&body)
            .ok()
            .and_then(|d| d.get("device")?.as_str().map(str::to_string))
        else {
            return err(StatusCode::BAD_REQUEST, SHAPE);
        };
        if let Err(e) = crate::devices::validate_device(&device) {
            return err(StatusCode::BAD_REQUEST, &e);
        }
        // A signed-in route lets a locked account through to its own pages;
        // credentials wait until LosOS Login's rules are met.
        if person.groups.iter().any(|g| g == crate::people::LOCKED) {
            return HttpResponse::Forbidden().json(serde_json::json!({
                "error": "finish what LosOS Login asks of your account first",
                "locked": true,
            }));
        }
        match api
            .backend
            .serialized(|l| crate::losos::cmd_device_backups(l, person, &device))
        {
            Ok(v) => HttpResponse::Ok().json(v),
            Err(e) => error_response(e),
        }
    })
}

fn directory_error(e: crate::directory::DirectoryError) -> HttpResponse {
    use crate::directory::DirectoryError as E;
    match e {
        E::Invalid(why) => err(StatusCode::BAD_REQUEST, &why),
        E::Full => err(StatusCode::CONFLICT, &E::Full.to_string()),
        E::NotFound => err(StatusCode::NOT_FOUND, &E::NotFound.to_string()),
        E::Io(e) => error_response(e),
    }
}

/// `POST /api/devices/directory` — `{"device"}`: join a LosOS Desktop
/// computer to this box (`crate::directory`). An administrator asks, from
/// the computer's `losos-login-bridge join` with their LosOS Login session,
/// and the answer carries the computer's token, once.
async fn post_device_directory(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded_as(&api, &req, "/api/devices/directory", true, |who| {
        let Some(device) = serde_json::from_slice::<serde_json::Value>(&body)
            .ok()
            .and_then(|d| d.get("device")?.as_str().map(str::to_string))
        else {
            return err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"device": "..."}"#,
            );
        };
        match api.directory.join(
            &device,
            who.person().map(|p| p.name.as_str()),
            losos_access::now_secs(),
        ) {
            Ok((token, computer)) => HttpResponse::Ok().json(serde_json::json!({
                "token": token,
                "computer": computer,
            })),
            Err(e) => directory_error(e),
        }
    })
}

/// `GET /api/devices/directory`: the joined computers, without tokens.
async fn get_device_directory(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded_as(&api, &req, "/api/devices/directory", false, |_| {
        match api.directory.list() {
            Ok(computers) => HttpResponse::Ok().json(serde_json::json!({ "computers": computers })),
            Err(e) => directory_error(e),
        }
    })
}

/// `DELETE /api/devices/directory/{id}`: take a computer out. Its token
/// stops at once; the accounts already on it stay, as they would offline.
async fn delete_device_directory(
    api: web::Data<Api>,
    req: HttpRequest,
    id: web::Path<String>,
) -> HttpResponse {
    guarded_as(
        &api,
        &req,
        "/api/devices/directory/{id}",
        true,
        |_| match api.directory.remove(&id) {
            Ok(()) => HttpResponse::Ok().json(serde_json::json!({ "removed": true })),
            Err(e) => directory_error(e),
        },
    )
}

/// `GET /api/directory/people`: the people list a joined computer reads to
/// lock the accounts this box removed or disabled (`crate::directory`).
/// The computer's own token, or an administrator.
async fn get_directory_people(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded_as(&api, &req, crate::directory::PEOPLE_ROUTE, false, |_| {
        if !api.people.headers {
            return err(
                StatusCode::CONFLICT,
                "LosOS Login does not run on this box, so it has no people list",
            );
        }
        match api.people.list() {
            Ok(people) => HttpResponse::Ok().json(crate::directory::people(people)),
            Err(e) => error_response(e),
        }
    })
}

/// Where `losos-ci` answers lososd (`backend/src/ci/api.rs`'s page socket).
fn ci_socket() -> std::path::PathBuf {
    std::env::var_os("LOSOS_CI_SOCKET")
        .map(Into::into)
        .unwrap_or_else(|| "/run/losos-ci/api.sock".into())
}

/// `POST /api/directory/jobs/claim` and `/report`: a joined computer's
/// Mesh CI jobs (`runs-on: [losos-desktop, <computer>]`), relayed to
/// `losos-ci` as that computer. Only the computer's own token: the
/// computer's name comes from the token lososd checked, never from the
/// request. A claim waits up to 15 s for work, off lososd's workers.
async fn post_directory_jobs(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    let (route, target) = if req.path() == crate::directory::JOBS_CLAIM_ROUTE {
        (crate::directory::JOBS_CLAIM_ROUTE, "/ci/desktop/claim")
    } else {
        (crate::directory::JOBS_REPORT_ROUTE, "/ci/desktop/report")
    };
    let computer = match gate_at(&api, &req, route, None, false) {
        Ok(Principal::Directory(computer)) => computer.device,
        Ok(_) => {
            return err(
                StatusCode::FORBIDDEN,
                "only a joined computer asks for its own jobs",
            )
        }
        Err(refused) => return *refused,
    };
    let body = if target == "/ci/desktop/claim" {
        serde_json::json!({ "wait": 15 }).to_string().into_bytes()
    } else {
        body.to_vec()
    };
    let answer = web::block(move || {
        crate::ci::net::unix_http_with(
            &ci_socket(),
            "POST",
            target,
            &[
                ("Content-Type", "application/json"),
                ("X-Losos-Desktop", &computer),
            ],
            &body,
            1024 * 1024,
            std::time::Duration::from_secs(40),
        )
    })
    .await;
    match answer {
        Ok(Ok((204, _))) => HttpResponse::NoContent().finish(),
        Ok(Ok((code, body))) => {
            HttpResponse::build(StatusCode::from_u16(code).unwrap_or(StatusCode::BAD_GATEWAY))
                .content_type("application/json")
                .body(body)
        }
        // No socket: this box runs no Mesh CI, so there is never work.
        Ok(Err(e)) => {
            tracing::debug!("losos-ci did not answer: {e:#}");
            HttpResponse::NoContent().finish()
        }
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// `POST /api/devices/cloud` and `POST /api/devices/git`: a credential of
/// one computer's own for LosOS Cloud or LosOS Git (`crate::devices`).
///
/// Bearer-gated like every other route, and on top of that the owner's
/// password, checked against LosOS Login's people file before either app
/// is asked. A wrong password is a 403 with `wrongPassword`, not the 401
/// of a bad token: the caller holds a good token, and a client that reads
/// 401 as "sign in again" would throw it away.
async fn post_device(
    api: web::Data<Api>,
    req: HttpRequest,
    kind: web::Path<String>,
    body: web::Bytes,
) -> HttpResponse {
    let route = format!("/api/devices/{kind}");
    guarded(&api, &req, &route, true, || {
        post_device_inner(&api, &kind, &remote_addr(&req), &body)
    })
}

fn post_device_inner(api: &Api, kind: &str, client: &str, body: &[u8]) -> HttpResponse {
    const SHAPE: &str = r#"body must be JSON: {"password": "...", "device": "..."}"#;
    let Ok(doc) = serde_json::from_slice::<serde_json::Value>(body) else {
        return err(StatusCode::BAD_REQUEST, SHAPE);
    };
    let (Some(password), Some(device)) = (
        doc.get("password").and_then(|p| p.as_str()),
        doc.get("device").and_then(|d| d.as_str()),
    ) else {
        return err(StatusCode::BAD_REQUEST, SHAPE);
    };
    if let Err(e) = crate::signin::validate_candidate(password) {
        return err(StatusCode::BAD_REQUEST, &e);
    }
    if let Err(e) = crate::devices::validate_device(device) {
        return err(StatusCode::BAD_REQUEST, &e);
    }
    if !matches!(kind, "cloud" | "git") {
        return err(StatusCode::NOT_FOUND, "no such device credential");
    }
    // The owner's password, as LosOS Login knows it. Wrong guesses are
    // counted per address, and past the budget nothing is checked until
    // the address's window ends.
    let now = std::time::Instant::now();
    if api.passwords.check(client, now).is_err() {
        return error_response(crate::signin::Throttled.into());
    }
    let password_ok = match owner_password_ok(api, password) {
        Ok(ok) => ok,
        Err(e) => return error_response(e),
    };
    if password_ok {
        api.passwords.record_success(client);
    } else {
        // Counted from when the answer is known: the argon2 check takes a
        // while, and a window started before it could end before the reply.
        api.passwords
            .record_failure(client, std::time::Instant::now());
    }
    let result = match kind {
        "cloud" => api
            .backend
            .serialized(|l| crate::losos::cmd_device_cloud(l, password_ok, device)),
        _ => api
            .backend
            .serialized(|l| crate::losos::cmd_device_git(l, password_ok, device)),
    };
    match result {
        Ok(v) => HttpResponse::Ok().json(v),
        Err(e) if e.downcast_ref::<crate::signin::WrongPassword>().is_some() => {
            tracing::info!(kind, "device credential refused: wrong password");
            HttpResponse::build(StatusCode::FORBIDDEN).json(serde_json::json!({
                "error": e.to_string(),
                "wrongPassword": true,
            }))
        }
        Err(e) if e.downcast_ref::<crate::devices::GitOff>().is_some() => {
            HttpResponse::build(StatusCode::CONFLICT).json(serde_json::json!({
                "error": e.to_string(),
                "gitOff": true,
            }))
        }
        Err(e) => error_response(e),
    }
}

/// Whether `password` is the owner's, as LosOS Login's people file has it.
/// A box claimed before sign-in went through LosOS Login has no owner
/// there yet, and nothing to check against: not now, rather than wrong.
fn owner_password_ok(api: &Api, password: &str) -> anyhow::Result<bool> {
    use crate::losos::Losos;
    let secret = crate::signin::validate_candidate(password).map_err(|e| anyhow::anyhow!(e))?;
    let owner = api.backend.serialized(|l| l.load_state())?.owner;
    let Some(owner) = owner else {
        return Err(crate::setup::NotReady(
            "Nobody signs in to this box yet. Make the first account on the Users pane with the spare key."
                .to_string(),
        )
        .into());
    };
    api.people.check_password(&owner, &secret)
}

/// The appliance recovery code.
///
/// `GET`, and there is no route that rotates it — see [`cmd_recovery`] for why
/// replacing the code is not an operation this API offers.
///
/// Gated by the same Bearer token as everything else and no harder, which is a
/// decision rather than an oversight: the token that would be needed to read
/// this already authorises `POST /api/apply`, which writes arbitrary Nix and
/// runs `nixos-rebuild switch` as root. A second factor in front of the code
/// would cost the owner a step and cost an attacker who already holds the token
/// nothing. The reasoning is written out in `backend/src/recovery.rs`.
async fn get_recovery(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/recovery", false, || {
        run(&api, cmd_recovery)
    })
}

/// Search the app catalogue: `GET /api/apps/search?q=<query>`.
///
/// `GET` with the term in the query string, because that is what
/// `admin-ui/app/src/screens/settings/catalogue.ts` sends and because the call
/// is a read — it installs nothing and changes nothing on this box.
///
/// A malformed or missing `q` is a 400 with the reason in it, so the screen can
/// put the sentence next to the field. It must not be a 404: the SPA treats a
/// 404 as "this box does not serve the route", latches it, and never asks
/// again for the rest of the session — so answering a bad query that way would
/// disable the search until a reload.
///
/// Everything past validation is [`run`], which means a search that could not
/// be made is a 500 with `command failed; see the lososd journal` and the real
/// cause — curl's own diagnosis — in the journal. The screen shows that as a
/// retryable failure, which is right: no route off the box and a catalogue
/// having a bad afternoon are both things that come back.
async fn get_apps_search(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/apps/search", false, || {
        get_apps_search_inner(&api, &req)
    })
}

fn get_apps_search_inner(api: &Api, req: &HttpRequest) -> HttpResponse {
    let query =
        web::Query::<std::collections::HashMap<String, String>>::from_query(req.query_string())
            .ok()
            .and_then(|q| q.get("q").cloned())
            .unwrap_or_default();
    // Validated here as well as in the command, for the same reason
    // `set-password` is: it turns "too short" into a 400 beside the field
    // rather than a 500 that says to read the journal.
    if let Err(e) = crate::catalogue::validate_query(&query) {
        return err(StatusCode::BAD_REQUEST, &e);
    }
    run(api, |b| cmd_apps_search(b, &query))
}

// ── Text inside files, for the palette ───────────────────────────────────
// `crate::textsearch` holds the searches; they run in their own units as the
// apps' own users (modules/search.nix), and lososd only relays. HTTP-only,
// like /api/apps/search: the palette is the only caller.

/// `GET /api/cloud/text?q=` — files in LosOS Cloud whose text matches.
async fn get_cloud_text(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/cloud/text", false, || {
        text_search::<crate::textsearch::CloudHit>(&req, "LOSOS_SEARCH_CLOUD_SOCKET", "off")
    })
}

/// `GET /api/git/text?q=` — lines on the repositories' default branches
/// that match.
async fn get_git_text(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/git/text", false, || {
        text_search::<crate::textsearch::GitHit>(&req, "LOSOS_SEARCH_GIT_SOCKET", "noGit")
    })
}

/// How long lososd waits for a search before it calls it failed. The Git
/// search answers with what it has after four seconds.
const TEXT_SEARCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(6);

/// Relay one text search. Not offered (the socket variable unset: the search
/// is off, or the box runs no LosOS Git) is 200 with `available: false` and
/// the reason, never 404, which the palette latches; neither is a search
/// unit that is not up yet. A bad `q` is 400 with the reason.
fn text_search<T: serde::de::DeserializeOwned + serde::Serialize>(
    req: &HttpRequest,
    socket_var: &str,
    off: &str,
) -> HttpResponse {
    let params =
        web::Query::<std::collections::HashMap<String, String>>::from_query(req.query_string())
            .map(web::Query::into_inner)
            .unwrap_or_default();
    let q = match crate::textsearch::validate_query(params.get("q").map_or("", String::as_str)) {
        Ok(q) => q,
        Err(e) => return err(StatusCode::BAD_REQUEST, &e),
    };
    let limit = params
        .get("limit")
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(8)
        .clamp(1, crate::textsearch::MAX_HITS);
    let Some(socket) = std::env::var_os(socket_var) else {
        return HttpResponse::Ok().json(serde_json::json!({ "available": false, "reason": off }));
    };
    match crate::textsearch::ask::<T>(Path::new(&socket), &q, limit, TEXT_SEARCH_TIMEOUT) {
        Ok(answer) => HttpResponse::Ok().json(serde_json::json!({
            "available": true,
            "partial": answer.partial,
            "hits": answer.hits,
        })),
        Err(crate::textsearch::AskError::NotUp(_)) => {
            HttpResponse::Ok().json(serde_json::json!({ "available": false, "reason": "notUp" }))
        }
        Err(crate::textsearch::AskError::Failed(e)) => {
            tracing::warn!(error = %e, "text search failed");
            err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "search failed; see the lososd journal",
            )
        }
    }
}

// ── Installing an app from the catalogue ─────────────────────────────────
// `crate::apps` holds the rules. HTTP-only, like the market: the page is the
// only caller.

/// `GET /api/apps` — what is installed, and whether anything can be.
async fn get_apps(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/apps", false, || {
        run(&api, crate::apps::cmd_apps)
    })
}

/// `GET /api/apps/chart?repo=&name=&version=` — the chart's values and
/// schema, for the install dialog.
///
/// Not under the state lock: fetching a chart can take as long as the
/// network does, and it touches no state. Holding the lock would stall every
/// other request to the box for that long.
async fn get_app_chart(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/apps/chart", false, || {
        let q =
            web::Query::<std::collections::HashMap<String, String>>::from_query(req.query_string())
                .map(web::Query::into_inner)
                .unwrap_or_default();
        let field = |k: &str| q.get(k).map(|v| v.trim().to_string()).unwrap_or_default();
        let chart = crate::apps::ChartRef {
            repo: field("repo"),
            name: field("name"),
            version: field("version"),
        };
        let mut backend = api.backend.clone();
        match crate::apps::cmd_app_chart(&mut backend, &chart) {
            Ok(v) => HttpResponse::Ok().json(v),
            Err(e) => error_response(e),
        }
    })
}

/// `POST /api/apps/install` `{release, chart: {repo, name, version}, runAs,
/// values?, valuesYaml?}` — install an app, or change one installed under the
/// same name.
async fn post_app_install(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    guarded(&api, &req, "/api/apps/install", true, || {
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        match crate::apps::InstallRequest::parse(&doc) {
            Ok(request) => run(&api, |b| crate::apps::cmd_app_install(b, &request)),
            Err(e) => error_response(e),
        }
    })
}

/// `POST /api/apps/compose` `{release, title?, compose, files?}` — install an
/// app from a Compose file (the LosOS app format), or change one installed
/// that way under the same name.
async fn post_app_compose(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    guarded(&api, &req, "/api/apps/compose", true, || {
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        match crate::apps::ComposeRequest::parse(&doc) {
            Ok(request) => run(&api, |b| crate::apps::cmd_app_compose(b, &request)),
            Err(e) => error_response(e),
        }
    })
}

/// `POST /api/apps/remove` `{release}`.
async fn post_app_remove(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    guarded(&api, &req, "/api/apps/remove", true, || {
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        match field_str(&doc, "release") {
            Some(release) => run(&api, |b| crate::apps::cmd_app_remove(b, &release)),
            None => err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"release": "<name>"}"#,
            ),
        }
    })
}

/// Where the Lab's libvirt helper listens, from `$LOSOS_LAB_URL`; `None`
/// when the box runs none (`losos.lab.libvirt.enable` off).
fn lab_helper() -> Option<std::net::SocketAddr> {
    std::env::var("LOSOS_LAB_URL")
        .ok()
        .and_then(|u| crate::lab::helper_addr(&u))
}

/// Pass the helper's answer on: its status and its JSON, untouched.
fn lab_answer(status: u16, body: Vec<u8>) -> HttpResponse {
    HttpResponse::build(StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY))
        .content_type("application/json")
        .body(body)
}

const LAB_NOT_RUNNING: &str = "the Lab's libvirt helper is not running on this box";

/// Relay one call to the Lab's helper and pass its answer on; a 503 when the
/// helper is off or does not answer. A 204 stays bodiless.
fn lab_relay(api: &Api, method: &str, path: &str, body: &[u8]) -> HttpResponse {
    let Some(addr) = lab_helper() else {
        return err(StatusCode::SERVICE_UNAVAILABLE, LAB_NOT_RUNNING);
    };
    match crate::lab::relay(addr, method, path, &api.token, body) {
        Ok((204, _)) => HttpResponse::NoContent().finish(),
        Ok((status, body)) => lab_answer(status, body),
        Err(_) => err(StatusCode::SERVICE_UNAVAILABLE, LAB_NOT_RUNNING),
    }
}

/// `GET /api/lab/hello` — can this box run the Lab's guests under libvirt?
/// A 200 either way; `available: false` names why not. See `crate::lab`.
async fn get_lab_hello(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/lab/hello", false, || {
        let Some(addr) = lab_helper() else {
            return HttpResponse::Ok()
                .json(serde_json::json!({ "available": false, "reason": "off" }));
        };
        match crate::lab::relay(addr, "GET", "/lab/v1/hello", &api.token, b"") {
            Ok((200, body)) => lab_answer(200, body),
            Ok((status, _)) => {
                tracing::warn!(status, "lab helper answered hello with an error");
                HttpResponse::Ok()
                    .json(serde_json::json!({ "available": false, "reason": "helperError" }))
            }
            Err(e) => {
                tracing::info!("lab helper not reachable: {e}");
                HttpResponse::Ok()
                    .json(serde_json::json!({ "available": false, "reason": "notRunning" }))
            }
        }
    })
}

/// `POST /api/lab/guests` — start a guest; the helper checks the body.
async fn post_lab_guest(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    guarded(&api, &req, "/api/lab/guests", true, || {
        if body.len() > crate::lab::MAX_BODY {
            return err(
                StatusCode::PAYLOAD_TOO_LARGE,
                "the guest request is too big",
            );
        }
        lab_relay(&api, "POST", "/lab/v1/guests", &body)
    })
}

/// `POST /api/lab/virt-ticket` — a single-use ticket for the libvirt relay
/// socket at `/api/lab/virt`, which nginx proxies to the helper directly.
/// The ticket opens libvirt at the helper's uid, so it wants the admin token
/// like a create, and the request is audited. No body.
async fn post_lab_virt_ticket(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/lab/virt-ticket", true, || {
        lab_relay(&api, "POST", "/lab/v1/virt-ticket", b"")
    })
}

/// `DELETE /api/lab/guests/{key}` — stop a guest.
async fn delete_lab_guest(
    api: web::Data<Api>,
    req: HttpRequest,
    key: web::Path<String>,
) -> HttpResponse {
    guarded(&api, &req, "/api/lab/guests", true, || {
        if !crate::lab::valid_key(&key) {
            return err(StatusCode::BAD_REQUEST, "that is not a guest key");
        }
        lab_relay(&api, "DELETE", &format!("/lab/v1/guests/{key}"), b"")
    })
}

/// `losos.lab.ordering.enable`, which modules/daemon.nix passes as
/// `LOSOS_LAB_ORDERING=1`.
fn lab_ordering() -> bool {
    std::env::var("LOSOS_LAB_ORDERING").is_ok_and(|v| v == "1")
}

/// `GET /api/lab/order` — whether the Lab shows its order button, and the
/// official edge's catalogue when it does. A 200 in every case.
async fn get_lab_order(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/lab/order", false, || {
        run(&api, |b| crate::losos::cmd_lab_order(b, lab_ordering()))
    })
}

/// `POST /api/lab/order` — `{"items":[{"sku","quantity"}]}` to a Stripe
/// Checkout on the official edge.
async fn post_lab_order(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    guarded(&api, &req, "/api/lab/order", true, || {
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        let items: Option<Vec<(String, u64)>> = doc
            .get("items")
            .and_then(serde_json::Value::as_array)
            .and_then(|lines| {
                lines
                    .iter()
                    .map(|l| Some((field_str(l, "sku")?, field_u64(l, "quantity")?)))
                    .collect()
            });
        match items {
            Some(items) => run(&api, |b| {
                crate::losos::cmd_lab_order_op(b, lab_ordering(), items)
            }),
            None => err(
                StatusCode::BAD_REQUEST,
                "the request body is missing a required field",
            ),
        }
    })
}

/// `GET /api/market` — the market as this box sees it: the shelf and its own
/// account, or `{"available": false}` when it is not offered here.
///
/// Relayed to the edge because the pages cannot call it themselves; see
/// `backend/src/market.rs`. Unavailable is a 200, not a 404, because the SPA
/// latches a 404 as "this box does not serve the route" for the whole session.
async fn get_market(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    relayed(&api, &req, "/api/market", false, crate::losos::cmd_market).await
}

/// A call the box relays to its edge, run on a blocking thread and outside the
/// state lock. It only reads the box's edge status and asks the edge, and the
/// edge can take its time (a tap confirm reads the chain): held under the lock
/// on a request thread, it kept every other call waiting until actix gave
/// them a 408.
async fn relayed(
    api: &web::Data<Api>,
    req: &HttpRequest,
    route: &str,
    mutating: bool,
    work: impl FnOnce(&mut IoLosos) -> anyhow::Result<serde_json::Value> + Send + 'static,
) -> HttpResponse {
    relayed_then(api, req, route, mutating, work, |_, _, _| {}).await
}

/// [`relayed`], with `then` shown the caller and the edge's answer before
/// it goes back.
async fn relayed_then(
    api: &web::Data<Api>,
    req: &HttpRequest,
    route: &str,
    mutating: bool,
    work: impl FnOnce(&mut IoLosos) -> anyhow::Result<serde_json::Value> + Send + 'static,
    then: impl FnOnce(&Api, &Principal, &serde_json::Value),
) -> HttpResponse {
    let who = match gate_at(api, req, route, None, mutating) {
        Ok(who) => who,
        Err(refused) => return *refused,
    };
    let worker = api.clone();
    let resp = match web::block(move || work(&mut worker.backend.clone())).await {
        Ok(Ok(v)) => {
            then(api, &who, &v);
            HttpResponse::Ok().json(v)
        }
        Ok(Err(e)) => error_response(e),
        Err(e) => {
            tracing::error!(error = %e, "the edge call did not finish");
            err(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    };
    if mutating {
        audit_outcome(api, req, route, &resp, &who);
    }
    resp
}

/// The `POST /api/market/*` actions. The body is parsed by hand and the 400s
/// name the field, never echoing what was sent.
async fn post_market(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
    route: &'static str,
    build: fn(&serde_json::Value) -> Option<crate::market::Op>,
) -> HttpResponse {
    let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
    match build(&doc) {
        Some(op) => {
            relayed(&api, &req, route, true, move |b| {
                crate::losos::cmd_market_op(b, &op)
            })
            .await
        }
        None => guarded(&api, &req, route, true, || {
            err(
                StatusCode::BAD_REQUEST,
                "the request body is missing a required field",
            )
        }),
    }
}

fn field_u64(doc: &serde_json::Value, key: &str) -> Option<u64> {
    doc.get(key).and_then(serde_json::Value::as_u64)
}

fn field_str(doc: &serde_json::Value, key: &str) -> Option<String> {
    doc.get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

async fn post_market_onboard(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    post_market(api, req, body, "/api/market/onboard", |_| {
        Some(crate::market::Op::Onboard { box_uuid: None })
    })
    .await
}

async fn post_market_listing(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    post_market(api, req, body, "/api/market/listings", |d| {
        Some(crate::market::Op::List {
            kind: field_str(d, "kind")?,
            unit_price: field_u64(d, "unit_price")?,
            capacity: field_u64(d, "capacity")?,
        })
    })
    .await
}

async fn post_market_close(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    post_market(api, req, body, "/api/market/listings/close", |d| {
        Some(crate::market::Op::Close {
            listing_id: field_str(d, "listing_id")?,
        })
    })
    .await
}

async fn post_market_release(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    post_market(api, req, body, "/api/market/release", |_| {
        Some(crate::market::Op::Release)
    })
    .await
}

async fn post_market_order(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    post_market(api, req, body, "/api/market/orders", |d| {
        Some(crate::market::Op::Order {
            listing_id: field_str(d, "listing_id")?,
            quantity: field_u64(d, "quantity")?,
        })
    })
    .await
}

async fn post_market_payout(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    post_market(api, req, body, "/api/market/payout", |d| {
        Some(crate::market::Op::Payout {
            address: field_str(d, "address")?,
            signature: field_str(d, "signature")?,
            family: field_str(d, "family"),
        })
    })
    .await
}

async fn post_market_confirm(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    post_market(api, req, body, "/api/market/confirm", |d| {
        Some(crate::market::Op::Confirm {
            order_id: field_str(d, "order_id")?,
            tx: field_str(d, "tx")?,
            network: field_str(d, "network"),
        })
    })
    .await
}

async fn post_market_fees_pay(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    post_market(api, req, body, "/api/market/fees/pay", |d| {
        Some(crate::market::Op::FeePay {
            tx: field_str(d, "tx")?,
            network: field_str(d, "network"),
        })
    })
    .await
}

async fn post_market_coin_key(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    post_market(api, req, body, "/api/market/coins/key", |d| {
        Some(crate::market::Op::CoinKey {
            coin: field_str(d, "coin")?,
            key: field_str(d, "key")?,
            view_key: field_str(d, "view_key"),
        })
    })
    .await
}

async fn post_market_coin_pay(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    post_market(api, req, body, "/api/market/coins/pay", |d| {
        Some(crate::market::Op::CoinPay {
            order_id: field_str(d, "order_id")?,
            coin: field_str(d, "coin")?,
        })
    })
    .await
}

/// `POST /api/market/onramp` — `{"address","euros"}`: a Binance order for
/// USDC on Base to this box's wallet, with Binance's checkout link. The
/// order is noted against the person who started it, so a finished
/// purchase can mark them as an adult (`crate::pictures`).
async fn post_market_onramp(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
    let (Some(address), Some(euros)) = (field_str(&doc, "address"), field_str(&doc, "euros"))
    else {
        return post_market(api, req, body, "/api/market/onramp", |_| None).await;
    };
    let op = crate::market::Op::Onramp { address, euros };
    relayed_then(
        &api,
        &req,
        "/api/market/onramp",
        true,
        move |b| crate::losos::cmd_market_op(b, &op),
        |api, who, order| {
            let (Some(person), Some(id)) = (who.person(), order_id_of(order)) else {
                return;
            };
            if let Err(e) = api.pictures.order_started(&person.name, id) {
                tracing::warn!(error = ?e, "a Binance order was not noted for the adult mark");
            }
        },
    )
    .await
}

/// `POST /api/market/onramp/status` — `{"order_id"}`: where that order is.
/// A finished purchase marks whoever started it as an adult.
async fn post_market_onramp_status(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
    let Some(order_id) = field_str(&doc, "order_id") else {
        return post_market(api, req, body, "/api/market/onramp/status", |_| None).await;
    };
    let op = crate::market::Op::OnrampStatus { order_id };
    relayed_then(
        &api,
        &req,
        "/api/market/onramp/status",
        true,
        move |b| crate::losos::cmd_market_op(b, &op),
        |api, _, order| {
            let (Some(id), Some(stage)) = (
                order_id_of(order),
                order.get("stage").and_then(serde_json::Value::as_str),
            ) else {
                return;
            };
            match api.pictures.order_stage(id, stage) {
                Ok(Some(name)) => tracing::info!(person = %name, "marked as an adult by a finished Binance purchase"),
                Ok(None) => {}
                Err(e) => tracing::warn!(error = ?e, "a finished Binance order did not reach the adult mark"),
            }
        },
    )
    .await
}

/// The order ID in the edge's answer about a Binance order, when it is one.
fn order_id_of(order: &serde_json::Value) -> Option<&str> {
    order
        .get("order_id")
        .and_then(serde_json::Value::as_str)
        .filter(|id| id.len() == 32 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
}

async fn post_market_rpc(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    post_market(api, req, body, "/api/market/rpc", |d| {
        let method = field_str(d, "method")?;
        let params = d.get("params")?.to_string();
        Some(crate::market::Op::Rpc {
            method,
            params,
            network: field_str(d, "network"),
        })
    })
    .await
}

// ── AI models on lent memory ──────────────────────────────────────────────
// Pay as you go through the edge (`edge/registrar/src/server/llm.rs`): the
// pane quotes a meter, the owner's wallet opens its taps in the escrow, and
// each answer is billed per token. Field names are the edge's own.

/// `POST /api/market/llm/models` `{network?}` — the models, this box's
/// meters, balances and lending, or `{"available": false}`.
async fn post_market_llm_models(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
    let network = field_str(&doc, "network");
    relayed(&api, &req, "/api/market/llm/models", false, move |b| {
        crate::losos::cmd_llm_models(b, network)
    })
    .await
}

/// A whole number of USDC base units, as a JSON number or decimal text.
fn field_units(doc: &serde_json::Value, key: &str) -> Option<u64> {
    let v = doc.get(key)?;
    v.as_u64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
}

/// `POST /api/market/taps` `{model, network?, spendPerHour}` — quote a meter.
async fn post_market_taps(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    post_market(api, req, body, "/api/market/taps", |d| {
        Some(crate::market::Op::TapQuote {
            model: field_str(d, "model")?,
            network: field_str(d, "network"),
            spend_per_hour: field_units(d, "spendPerHour")?,
        })
    })
    .await
}

/// `POST /api/market/taps/confirm` `{meter, wallet}` — `wallet` opened the
/// taps.
async fn post_market_taps_confirm(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    post_market(api, req, body, "/api/market/taps/confirm", |d| {
        Some(crate::market::Op::TapConfirm {
            meter: field_str(d, "meter")?,
            wallet: field_str(d, "wallet")?,
        })
    })
    .await
}

/// `POST /api/market/taps/stop` `{meter, wallet?}` — stop a meter.
async fn post_market_taps_stop(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    post_market(api, req, body, "/api/market/taps/stop", |d| {
        let wallet = match d.get("wallet") {
            None | Some(serde_json::Value::Null) => None,
            Some(w) => Some(w.as_str()?.to_string()),
        };
        Some(crate::market::Op::TapStop {
            meter: field_str(d, "meter")?,
            wallet,
        })
    })
    .await
}

/// The chat body as the page sends it, or `None` when a field is missing or
/// of the wrong kind. The limits are checked by `ChatAsk::validate`.
fn chat_ask(doc: &serde_json::Value) -> Option<crate::market::ChatAsk> {
    let messages = doc
        .get("messages")?
        .as_array()?
        .iter()
        .map(|m| Some((field_str(m, "role")?, field_str(m, "content")?)))
        .collect::<Option<Vec<_>>>()?;
    let number = |key: &str| match doc.get(key) {
        None | Some(serde_json::Value::Null) => Some(None),
        Some(v) => v.as_f64().map(Some),
    };
    let stop = match doc.get("stop") {
        None | Some(serde_json::Value::Null) => Vec::new(),
        Some(serde_json::Value::String(s)) => vec![s.clone()],
        Some(serde_json::Value::Array(a)) => a
            .iter()
            .map(|s| s.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()?,
        Some(_) => return None,
    };
    let max_tokens = match doc.get("maxTokens") {
        None | Some(serde_json::Value::Null) => None,
        Some(v) => Some(v.as_u64()?),
    };
    Some(crate::market::ChatAsk {
        meter: field_str(doc, "meter")?,
        messages,
        max_tokens,
        temperature: number("temperature")?,
        top_p: number("topP")?,
        stop,
    })
}

/// `POST /api/market/llm/chat` — one request to a running model, billed on
/// its meter: `{meter, messages: [{role, content}], maxTokens?,
/// temperature?, topP?, stop?}`.
///
/// An answer can take a quarter of an hour, so this runs off the HTTP worker
/// and without the state lock every other command takes: it touches no
/// state of the box, and holding the lock would stall the whole API while a
/// model thinks. The audit record is written when the answer is back.
async fn post_market_llm_chat(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    const ROUTE: &str = "/api/market/llm/chat";
    let mut who: Option<String> = None;
    let gate = guarded_as(&api, &req, ROUTE, false, |p| {
        who = Some(p.audit_name().to_string());
        HttpResponse::Ok().finish()
    });
    let Some(who) = who else {
        return gate;
    };
    let remote = remote_addr(&req);
    let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
    let resp = match chat_ask(&doc) {
        None => err(
            StatusCode::BAD_REQUEST,
            "the request body is missing a required field",
        ),
        Some(ask) => {
            let worker = api.clone();
            let answer = web::block(move || {
                let mut backend = worker.backend.clone();
                crate::losos::cmd_market_op(&mut backend, &crate::market::Op::LlmChat(ask))
            })
            .await;
            match answer {
                Ok(Ok(v)) => HttpResponse::Ok().json(v),
                Ok(Err(e)) => error_response(e),
                Err(e) => {
                    tracing::error!(error = %e, "the model request did not finish");
                    err(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
                }
            }
        }
    };
    let status = resp.status();
    let outcome = if status.is_success() {
        "ok"
    } else if status.is_client_error() {
        "rejected"
    } else {
        "error"
    };
    api.audit.record_as(ROUTE, outcome, &remote, Some(&who));
    resp
}

// ── The widget builder ────────────────────────────────────────────────────
// A model on this box writes a widget from the owner's description
// (`crate::builder`). It has its own lock: a build takes minutes and must
// not hold the backend's.

/// `GET /api/builder` — whether the model is ready, and the recent builds.
async fn get_builder(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/builder", false, || {
        HttpResponse::Ok().json(api.builder.view())
    })
}

/// A refusal from the builder, as `{"error": ...}` at its own status.
fn builder_refused(r: &crate::builder::Refused) -> HttpResponse {
    err(
        StatusCode::from_u16(r.status).unwrap_or(StatusCode::BAD_REQUEST),
        &r.message,
    )
}

/// `POST /api/builder/builds` `{"prompt": ..., "kind"?: "widget"|"app",
/// "base"?: ..., "lang"?: ...}` — start a build. `base` is the widget to
/// change, as the files the editor holds (a single string is read as its
/// `index.html`), or the source of the app to change.
async fn post_builder_start(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded(&api, &req, "/api/builder/builds", true, || {
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        match crate::builder::parse_request(&doc).and_then(|r| api.builder.start(r)) {
            Ok(build) => HttpResponse::Ok().json(build),
            Err(r) => builder_refused(&r),
        }
    })
}

/// `GET /api/builder/builds/{id}` — one build, with its files once done.
async fn get_builder_build(
    api: web::Data<Api>,
    req: HttpRequest,
    path: web::Path<String>,
) -> HttpResponse {
    guarded(&api, &req, "/api/builder/builds", false, || {
        match api.builder.build(&path) {
            Some(build) => HttpResponse::Ok().json(build),
            None => err(StatusCode::NOT_FOUND, "no such build"),
        }
    })
}

/// `POST /api/builder/builds/{id}/tried` — what the page found when it
/// tried the app the build asked it to (`crate::builder::TryReport`).
async fn post_builder_tried(
    api: web::Data<Api>,
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
) -> HttpResponse {
    guarded(&api, &req, "/api/builder/builds", true, || {
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        match crate::builder::parse_report(&doc)
            .and_then(|(trial, report)| api.builder.tried(&path, &trial, report))
        {
            Ok(v) => HttpResponse::Ok().json(v),
            Err(r) => builder_refused(&r),
        }
    })
}

/// `DELETE /api/builder/builds/{id}` — stop a running build.
async fn delete_builder_build(
    api: web::Data<Api>,
    req: HttpRequest,
    path: web::Path<String>,
) -> HttpResponse {
    guarded(&api, &req, "/api/builder/builds", true, || {
        match api.builder.stop(&path) {
            Ok(build) => HttpResponse::Ok().json(build),
            Err(r) => builder_refused(&r),
        }
    })
}

// ── LosOS Mail ────────────────────────────────────────────────────────────
// A mailbox per person (`crate::mail`). Every route is `signed-in` in
// sections.json: whoever LosOS Login signed in reads their own mail, and
// only their own, because each call is made as them. The spare key and a
// managing edge are nobody, so they have no mailbox.

/// The policy a message's HTML is shown under, by lososd and by nginx's
/// header map alike (`modules/containers.nix`, the `mailFrame` arm): a
/// sandbox with no scripts and no same origin, inline style, and nothing
/// loaded from anywhere, so a remote picture cannot tell the sender the
/// message was read. Links open in a new tab, which is all
/// `allow-popups` is for.
pub const MAIL_FRAME_CSP: &str = "sandbox allow-popups allow-popups-to-escape-sandbox; default-src 'none'; style-src 'unsafe-inline'; img-src data:; font-src data:; frame-ancestors 'self'; base-uri 'none'; form-action 'none'";

/// [`MAIL_FRAME_CSP`] for a message whose pictures stay hidden: no picture
/// of any kind loads, in an `<img>` or in its CSS (`crate::pictures`).
pub const MAIL_FRAME_CSP_NO_PICTURES: &str = "sandbox allow-popups allow-popups-to-escape-sandbox; default-src 'none'; style-src 'unsafe-inline'; img-src 'none'; font-src data:; frame-ancestors 'self'; base-uri 'none'; form-action 'none'";

/// Drawn where a hidden picture would be, so the layout keeps its holes
/// instead of broken-image icons.
const MAIL_HIDDEN_PICTURE_STYLE: &str = "<style>img{color:transparent;background:repeating-linear-gradient(45deg,#8881 0 8px,#8882 8px 16px);min-width:24px;min-height:24px}</style>";

fn mail_refused(r: &crate::mail::Refused) -> HttpResponse {
    err(
        StatusCode::from_u16(r.status).unwrap_or(StatusCode::BAD_GATEWAY),
        &r.message,
    )
}

/// The person a mail call is made as, or the answer for a caller who is
/// nobody's mailbox: the spare key, an edge, or a person LosOS Login is
/// holding to their own account.
fn mail_person<'a>(
    api: &Api,
    who: &'a Principal,
) -> Result<&'a crate::people::Person, Box<HttpResponse>> {
    if api.mail.open().is_none() {
        return Err(Box::new(err(
            StatusCode::NOT_FOUND,
            "there is no mail on this box",
        )));
    }
    match who.person() {
        Some(p) if p.groups.iter().any(|g| g == crate::people::LOCKED) => Err(Box::new(
            HttpResponse::Forbidden().json(serde_json::json!({
                "error": "finish what LosOS Login asks of your account first",
                "locked": true,
            })),
        )),
        Some(p) => Ok(p),
        None => Err(Box::new(err(
            StatusCode::CONFLICT,
            "the spare key has no mailbox; sign in with LosOS Login to read mail",
        ))),
    }
}

/// The answer for a message a moderator holds (`crate::moderation`): it
/// neither opens nor moves until they let it go. Deleting it is the
/// person's own choice and stays open.
fn not_held<'a>(
    api: &Api,
    person: &'a crate::people::Person,
    id: &str,
) -> Result<&'a crate::people::Person, Box<HttpResponse>> {
    if api.moderation.is_held(&person.name, id) {
        return Err(Box::new(HttpResponse::Forbidden().json(
            serde_json::json!({
                "error": "a moderator is holding this message",
                "held": true,
            }),
        )));
    }
    Ok(person)
}

/// `GET /api/mail` — the person's address, their folders, and the people
/// on this box to write to. 404 when mail is not live here: off, or the box
/// not sharing its storage, in which case the page does not exist either.
async fn get_mail(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded_as(&api, &req, "/api/mail", false, |who| {
        let person = match mail_person(&api, who) {
            Ok(p) => p,
            Err(resp) => return *resp,
        };
        let people = api.people.list().unwrap_or_default();
        match api.mail.mailboxes(person) {
            Ok(mailboxes) => HttpResponse::Ok().json(serde_json::json!({
                "address": api.mail.address(&person.name),
                "name": person.display_name,
                "mailboxes": mailboxes,
                "people": api.mail.neighbours(&people),
            })),
            Err(r) => mail_refused(&r),
        }
    })
}

/// `GET /api/mail/messages?mailbox=<id>&q=<words>&position=<n>` — a page of
/// a folder, newest first, optionally only what matches `q`.
async fn get_mail_messages(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded_as(&api, &req, "/api/mail/messages", false, |who| {
        let person = match mail_person(&api, who) {
            Ok(p) => p,
            Err(resp) => return *resp,
        };
        let query =
            web::Query::<std::collections::HashMap<String, String>>::from_query(req.query_string())
                .map(web::Query::into_inner)
                .unwrap_or_default();
        let Some(mailbox) = query.get("mailbox") else {
            return err(StatusCode::BAD_REQUEST, "which folder?");
        };
        let position = query
            .get("position")
            .and_then(|p| p.parse().ok())
            .unwrap_or(0);
        match api.mail.list(
            person,
            mailbox,
            query.get("q").map(String::as_str),
            position,
        ) {
            Ok(page) => HttpResponse::Ok().json(page),
            Err(r) => mail_refused(&r),
        }
    })
}

/// `GET /api/mail/messages/{id}` — open a message, which marks it read,
/// unless the LosOS companion opened it for an AI tool.
async fn get_mail_message(
    api: web::Data<Api>,
    req: HttpRequest,
    path: web::Path<String>,
) -> HttpResponse {
    guarded_as(
        &api,
        &req,
        "/api/mail/messages",
        false,
        |who| match mail_person(&api, who)
            .and_then(|p| not_held(&api, p, &path))
            .map(|p| {
                let by_person = !matches!(who, Principal::Companion(_));
                let m = api.mail.message(p, &path, by_person);
                if let (Ok(m), Some(keys), true) = (&m, &api.mail_keys, by_person) {
                    learn_autocrypt(&api, keys, p, m);
                }
                m
            }) {
            Ok(Ok(m)) => HttpResponse::Ok().json(m),
            Ok(Err(r)) => mail_refused(&r),
            Err(resp) => *resp,
        },
    )
}

/// `GET /api/mail/messages/{id}/html` — the message's HTML, as a document
/// of its own under [`MAIL_FRAME_CSP`], for the page's sandboxed frame.
async fn get_mail_html(
    api: web::Data<Api>,
    req: HttpRequest,
    path: web::Path<String>,
) -> HttpResponse {
    guarded_as(
        &api,
        &req,
        "/api/mail/messages/html",
        false,
        |who| match mail_person(&api, who)
            .and_then(|p| not_held(&api, p, &path))
            .map(|p| api.mail.html(p, &path))
        {
            Ok(Ok(html)) => {
                // `?pictures=1` once the person chose to see them; hidden
                // otherwise, whatever the message says.
                let shown = query_map(&req).get("pictures").is_some_and(|v| v == "1");
                let (csp, html) = if shown {
                    (MAIL_FRAME_CSP, html)
                } else {
                    (
                        MAIL_FRAME_CSP_NO_PICTURES,
                        format!("{MAIL_HIDDEN_PICTURE_STYLE}{html}"),
                    )
                };
                HttpResponse::Ok()
                    .content_type("text/html; charset=utf-8")
                    .insert_header(("Content-Security-Policy", csp))
                    .insert_header(("X-Content-Type-Options", "nosniff"))
                    .insert_header(("Referrer-Policy", "no-referrer"))
                    .insert_header(("Cache-Control", "private, no-store"))
                    .body(html)
            }
            Ok(Err(r)) => mail_refused(&r),
            Err(resp) => *resp,
        },
    )
}

/// `POST /api/mail/messages/{id}` `{"seen"?, "flagged"?, "mailbox"?}`.
async fn post_mail_message(
    api: web::Data<Api>,
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
) -> HttpResponse {
    guarded_as(&api, &req, "/api/mail/messages", true, |who| {
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        let done = mail_person(&api, who)
            .and_then(|p| not_held(&api, p, &path))
            .map(|p| crate::mail::parse_change(&doc).and_then(|c| api.mail.change(p, &path, &c)));
        match done {
            Ok(Ok(())) => HttpResponse::Ok().json(serde_json::json!({ "ok": true })),
            Ok(Err(r)) => mail_refused(&r),
            Err(resp) => *resp,
        }
    })
}

/// `DELETE /api/mail/messages/{id}` — to the bin, or out of it for good.
async fn delete_mail_message(
    api: web::Data<Api>,
    req: HttpRequest,
    path: web::Path<String>,
) -> HttpResponse {
    guarded_as(
        &api,
        &req,
        "/api/mail/messages",
        true,
        |who| match mail_person(&api, who).map(|p| api.mail.delete(p, &path)) {
            Ok(Ok(())) => HttpResponse::Ok().json(serde_json::json!({ "ok": true })),
            Ok(Err(r)) => mail_refused(&r),
            Err(resp) => *resp,
        },
    )
}

/// `POST /api/mail/send` `{"to", "cc"?, "subject", "text", "inReplyTo"?,
/// "attachments"?: [{"name", "type", "data": base64}]}`.
async fn post_mail_send(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    guarded_as(&api, &req, "/api/mail/send", true, |who| {
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        let done = mail_person(&api, who).map(|p| {
            let autocrypt = api.mail_keys.as_ref().and_then(|k| {
                let address = api.mail.address(&p.name)?;
                let key = k.public(&p.name)?;
                Some(crate::mail_pgp::autocrypt_header(&address, &key.binary()))
            });
            crate::mail::parse_draft(&doc).and_then(|d| api.mail.send(p, &d, autocrypt.as_deref()))
        });
        match done {
            Ok(Ok(())) => HttpResponse::Ok().json(serde_json::json!({ "ok": true })),
            Ok(Err(r)) => mail_refused(&r),
            Err(resp) => *resp,
        }
    })
}

/// Learn the key in the `Autocrypt:` header of a message its person opened.
fn learn_autocrypt(
    api: &Api,
    keys: &crate::mail_pgp::Keys,
    person: &crate::people::Person,
    m: &crate::mail::Message,
) {
    let (Some(header), Some(from)) = (&m.autocrypt, m.summary.from.first()) else {
        return;
    };
    let own = api.mail.address(&person.name).unwrap_or_default();
    let sent = m.sent_at.as_deref().unwrap_or(&m.summary.received_at);
    keys.learn(&person.name, &own, &from.email, sent, header);
}

/// The vCards in `person`'s LosOS Contacts, every address book, asked of
/// LosOS Cloud over its socket as them. Nothing when Cloud is not there.
fn contact_cards(api: &Api, person: &crate::people::Person) -> Vec<String> {
    let user = if person.groups.iter().any(|g| g == crate::people::OWNER) {
        crate::app_names::OWNER_ACCOUNT
    } else {
        person.name.as_str()
    };
    let asked = api.backend.serialized(|b| {
        use crate::losos::Losos as _;
        let (status, body) = b.cloud_request(&crate::pins::CloudRequest::AddressBooks { user })?;
        anyhow::ensure!(status == 207, "LosOS Cloud answered {status}");
        let mut cards = Vec::new();
        for href in crate::pins::parse_address_books(&String::from_utf8_lossy(&body), user) {
            let (status, body) =
                b.cloud_request(&crate::pins::CloudRequest::Cards { user, href: &href })?;
            if status == 207 {
                cards.extend(crate::pins::parse_cards(&String::from_utf8_lossy(&body)));
            }
        }
        Ok(cards)
    });
    asked.unwrap_or_else(|e| {
        tracing::debug!(error = %e, "no contacts to look for keys in");
        Vec::new()
    })
}

fn no_keys() -> HttpResponse {
    err(StatusCode::NOT_FOUND, "there is no mail on this box")
}

/// `GET /api/mail/keys` — the person's own key: public, and private locked
/// with their passphrase; `{"key": null}` before they set one.
async fn get_mail_keys(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded_as(&api, &req, "/api/mail/keys", false, |who| {
        let person = match mail_person(&api, who) {
            Ok(p) => p,
            Err(resp) => return *resp,
        };
        let Some(keys) = &api.mail_keys else {
            return no_keys();
        };
        match keys.mine(&person.name) {
            Ok(mine) => HttpResponse::Ok()
                .insert_header(("Cache-Control", "private, no-store"))
                .json(serde_json::json!({ "key": mine })),
            Err(r) => mail_refused(&r),
        }
    })
}

/// `PUT /api/mail/keys` `{"publicKey", "lockedKey"}` — set the person's key.
/// The private key must be locked with a passphrase; the box seals it again
/// in its credential store and publishes the public key in its WKD.
async fn put_mail_keys(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    guarded_as(&api, &req, "/api/mail/keys", true, |who| {
        let person = match mail_person(&api, who) {
            Ok(p) => p,
            Err(resp) => return *resp,
        };
        let Some(keys) = &api.mail_keys else {
            return no_keys();
        };
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        let field = |k: &str| doc.get(k).and_then(serde_json::Value::as_str).unwrap_or("");
        let address = api.mail.address(&person.name).unwrap_or_default();
        let sealed_to = api.credentials.key().ok().flatten();
        let done = crate::mail_pgp::check_own(field("publicKey"), field("lockedKey"), &address)
            .and_then(|own| keys.set(&person.name, &own, sealed_to))
            .and_then(|()| keys.mine(&person.name));
        match done {
            Ok(mine) => HttpResponse::Ok().json(serde_json::json!({ "key": mine })),
            Err(r) => mail_refused(&r),
        }
    })
}

/// `DELETE /api/mail/keys` — forget the person's key and unpublish it.
async fn delete_mail_keys(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded_as(&api, &req, "/api/mail/keys", true, |who| {
        let person = match mail_person(&api, who) {
            Ok(p) => p,
            Err(resp) => return *resp,
        };
        let Some(keys) = &api.mail_keys else {
            return no_keys();
        };
        match keys.remove(&person.name) {
            Ok(()) => HttpResponse::Ok().json(serde_json::json!({ "key": null })),
            Err(r) => mail_refused(&r),
        }
    })
}

/// `GET /api/mail/keys/find?email=<address>` — the keys mail to that
/// address can be encrypted with, and its signatures checked against, best
/// first, each with where it was found.
async fn find_mail_keys(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded_as(&api, &req, "/api/mail/keys/find", false, |who| {
        let person = match mail_person(&api, who) {
            Ok(p) => p,
            Err(resp) => return *resp,
        };
        let Some(keys) = &api.mail_keys else {
            return no_keys();
        };
        let email =
            web::Query::<std::collections::HashMap<String, String>>::from_query(req.query_string())
                .ok()
                .and_then(|q| q.get("email").cloned())
                .unwrap_or_default();
        match keys.find(&person.name, &email, || contact_cards(&api, person)) {
            Ok(found) => HttpResponse::Ok().json(
                serde_json::json!({ "email": email.trim().to_ascii_lowercase(), "keys": found }),
            ),
            Err(r) => mail_refused(&r),
        }
    })
}

/// `GET /api/mail/messages/{id}/raw` — the whole message, for the page to
/// decrypt or check a signature on. Never shown as a document.
async fn get_mail_raw(
    api: web::Data<Api>,
    req: HttpRequest,
    path: web::Path<String>,
) -> HttpResponse {
    guarded_as(
        &api,
        &req,
        "/api/mail/messages/raw",
        false,
        |who| match mail_person(&api, who).map(|p| api.mail.raw(p, &path)) {
            Ok(Ok(bytes)) => HttpResponse::Ok()
                .content_type("application/octet-stream")
                .insert_header((
                    "Content-Disposition",
                    "attachment; filename=\"message.eml\"",
                ))
                .insert_header(("X-Content-Type-Options", "nosniff"))
                .insert_header(("Cache-Control", "private, no-store"))
                .body(bytes),
            Ok(Err(r)) => mail_refused(&r),
            Err(resp) => *resp,
        },
    )
}

/// `GET /api/mail/attachments/{blob}?name=<file name>` — an attachment, as
/// a download. Never shown inline: a file from a stranger is not opened
/// under this origin.
async fn get_mail_attachment(
    api: web::Data<Api>,
    req: HttpRequest,
    path: web::Path<String>,
) -> HttpResponse {
    guarded_as(&api, &req, "/api/mail/attachments", false, |who| {
        let name =
            web::Query::<std::collections::HashMap<String, String>>::from_query(req.query_string())
                .ok()
                .and_then(|q| q.get("name").cloned())
                .unwrap_or_else(|| "attachment".into());
        // Only what cannot break the header or the file system.
        let name: String = name
            .chars()
            .filter(|c| !c.is_control() && !matches!(c, '"' | '\\' | '/'))
            .take(120)
            .collect();
        let name = if name.trim().is_empty() {
            "attachment".to_string()
        } else {
            name
        };
        match mail_person(&api, who).map(|p| api.mail.download(p, &path)) {
            Ok(Ok((_, bytes))) => HttpResponse::Ok()
                .content_type("application/octet-stream")
                .insert_header((
                    "Content-Disposition",
                    format!(
                        "attachment; filename=\"{}\"; filename*=UTF-8''{}",
                        name.replace(|c: char| !c.is_ascii(), "_"),
                        percent(&name)
                    ),
                ))
                .insert_header(("X-Content-Type-Options", "nosniff"))
                .insert_header(("Cache-Control", "private, no-store"))
                .body(bytes),
            Ok(Err(r)) => mail_refused(&r),
            Err(resp) => *resp,
        }
    })
}

/// RFC 5987 percent-encoding for a `filename*` parameter.
fn percent(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

// ── Virtual machines ──────────────────────────────────────────────────────
// The market's third kind (`edge/registrar/src/vms.rs`), on its own page
// and its own routes, because every one of them is refused unless this box
// shares its storage (`crate::losos::cmd_vm_op`). Onboarding is the market's.

/// `GET /api/vms` — the catalogue, the machine listings, this box's account
/// and its machines' replicas; `{"available": false, "reason": ...}` when
/// the box does not share its storage or no official edge runs machines.
async fn get_vms(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/vms", false, || {
        run(&api, crate::losos::cmd_vms)
    })
}

async fn post_vm(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
    route: &'static str,
    build: fn(&serde_json::Value) -> Option<crate::market::Op>,
) -> HttpResponse {
    guarded(&api, &req, route, true, || {
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        match build(&doc) {
            Some(op) => run(&api, |b| crate::losos::cmd_vm_op(b, &op)),
            None => err(
                StatusCode::BAD_REQUEST,
                "the request body is missing a required field",
            ),
        }
    })
}

/// `POST /api/vms/orders` `{listing_id, quantity, image, name, user_data?}`.
async fn post_vm_order(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    post_vm(api, req, body, "/api/vms/orders", |d| {
        Some(crate::market::Op::VmOrder {
            listing_id: field_str(d, "listing_id")?,
            quantity: field_u64(d, "quantity")?,
            image: field_str(d, "image")?,
            name: field_str(d, "name")?,
            user_data: field_str(d, "user_data").filter(|u| !u.trim().is_empty()),
        })
    })
    .await
}

/// `POST /api/vms/listings` `{unit_price, capacity}`: offer replicas of
/// machines on this box, priced per replica-month.
async fn post_vm_listing(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    post_vm(api, req, body, "/api/vms/listings", |d| {
        Some(crate::market::Op::List {
            kind: "vm".to_string(),
            unit_price: field_u64(d, "unit_price")?,
            capacity: field_u64(d, "capacity")?,
        })
    })
    .await
}

async fn post_vm_listing_close(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    post_vm(api, req, body, "/api/vms/listings/close", |d| {
        Some(crate::market::Op::Close {
            listing_id: field_str(d, "listing_id")?,
        })
    })
    .await
}

/// `POST /api/vms/images/remove` `{upload_id}`.
async fn post_vm_image_remove(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    post_vm(api, req, body, "/api/vms/images/remove", |d| {
        Some(crate::market::Op::VmRemove {
            upload_id: field_str(d, "upload_id")?,
        })
    })
    .await
}

#[derive(serde::Deserialize)]
struct VmImageQuery {
    name: String,
    #[serde(default)]
    efi: Option<String>,
}

/// `PUT /api/vms/images?name=<name>&efi=1` with the QCOW2 as the body.
///
/// The ticket is taken under the backend lock like any market call; the
/// file itself streams to the edge outside it (`crate::vm_upload`), so a
/// long upload holds up nothing else. The edge checks the file (a QCOW2,
/// under its size limit) and its refusal comes back with its own words.
async fn put_vm_image(
    api: web::Data<Api>,
    req: HttpRequest,
    query: web::Query<VmImageQuery>,
    payload: web::Payload,
) -> HttpResponse {
    let op = crate::market::Op::VmTicket {
        name: query.name.clone(),
        efi: matches!(query.efi.as_deref(), Some("1" | "true")),
    };
    let mut ticket = None;
    let refused = guarded(&api, &req, "/api/vms/images", true, || {
        match api.backend.serialized(|b| crate::losos::cmd_vm_op(b, &op)) {
            Ok(v) => {
                ticket = Some(v);
                HttpResponse::Ok().finish()
            }
            Err(e) => error_response(e),
        }
    });
    let Some(ticket) = ticket else {
        return refused;
    };
    let (Some(path), Some(config)) = (
        ticket
            .get("upload_path")
            .and_then(serde_json::Value::as_str),
        crate::market::Config::from_env(),
    ) else {
        tracing::error!("the edge's upload ticket named no path, or the registrar is unset");
        return err(StatusCode::BAD_GATEWAY, "the edge did not take the image");
    };
    match crate::vm_upload::stream_to_edge(&config.registrar_url, path, payload).await {
        Ok(a) if (200..300).contains(&a.status) => {
            match serde_json::from_str::<serde_json::Value>(&a.body) {
                Ok(v) => HttpResponse::Ok().json(v),
                Err(_) => err(StatusCode::BAD_GATEWAY, "the edge did not take the image"),
            }
        }
        Ok(a) => {
            let status = match a.status {
                408 => StatusCode::REQUEST_TIMEOUT,
                409 => StatusCode::CONFLICT,
                413 => StatusCode::PAYLOAD_TOO_LARGE,
                415 => StatusCode::UNSUPPORTED_MEDIA_TYPE,
                400 | 404 => StatusCode::BAD_REQUEST,
                other => {
                    tracing::error!(status = other, "the edge refused an image upload");
                    return err(StatusCode::BAD_GATEWAY, "the edge did not take the image");
                }
            };
            err(status, &crate::market::public_message(&a.body))
        }
        Err(e) => {
            tracing::error!(error = ?e, "image upload to the edge failed");
            err(StatusCode::BAD_GATEWAY, "the upload did not reach the edge")
        }
    }
}

/// `GET /api/domains` — this box's own domains on the edge, and the records
/// to create for them; `{"available": false}` when no official edge offers
/// them. Relayed like the market, for the same reasons.
async fn get_domains(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/domains", false, || {
        run(&api, crate::losos::cmd_domains)
    })
}

async fn post_domain(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
    route: &'static str,
    build: fn(String) -> crate::market::Op,
) -> HttpResponse {
    guarded(&api, &req, route, true, || {
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        // Lowercased and trimmed here so "Cloud.Example.org " is not a 400.
        match field_str(&doc, "domain") {
            Some(d) => {
                let d = d.trim().trim_end_matches('.').to_ascii_lowercase();
                let op = build(d);
                run(&api, |b| crate::losos::cmd_domain_op(b, &op))
            }
            None => err(
                StatusCode::BAD_REQUEST,
                "the request body is missing a required field",
            ),
        }
    })
}

/// `POST /api/domains` `{"domain": ...}` — claim a domain for this box.
async fn post_domain_add(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    post_domain(api, req, body, "/api/domains", |domain| {
        crate::market::Op::DomainAdd { domain }
    })
    .await
}

/// `POST /api/domains/remove` `{"domain": ...}` — give one up.
async fn post_domain_remove(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    post_domain(api, req, body, "/api/domains/remove", |domain| {
        crate::market::Op::DomainRemove { domain }
    })
    .await
}

/// `POST /api/domains/wallet` `{"address": "0x…", "signature": "0x…"}` —
/// publish the box's wallet at its names on the edge, so wallets that send
/// to ENS names pay it by name; `{}` takes it off.
async fn post_domain_wallet(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded(&api, &req, "/api/domains/wallet", true, || {
        // `{}` unbinds, so anything that is not an object of strings is a
        // 400 rather than read as one.
        const SHAPE: &str =
            r#"body must be JSON: {"address": "0x…", "signature": "0x…"} to bind, or {} to unbind"#;
        let Ok(serde_json::Value::Object(doc)) = serde_json::from_slice(&body) else {
            return err(StatusCode::BAD_REQUEST, SHAPE);
        };
        let text = |key: &str| match doc.get(key) {
            None => Ok(None),
            Some(serde_json::Value::String(v)) => Ok(Some(v.trim().to_string())),
            Some(_) => Err(()),
        };
        let (Ok(address), Ok(signature)) = (text("address"), text("signature")) else {
            return err(StatusCode::BAD_REQUEST, SHAPE);
        };
        let op = crate::market::Op::DomainWallet {
            address: address.map(|a| a.to_ascii_lowercase()),
            signature,
        };
        run(&api, |b| crate::losos::cmd_domain_op(b, &op))
    })
}

// ── Plugins ───────────────────────────────────────────────────────────────
// Every route not registered above is a plugin's (`crate::plugin`): the
// manifest names it, this daemon authenticates the caller and checks the
// section exactly as for its own routes, and the plugin answers.

/// The section check for a call a plugin dispatches (`dispatch.call`),
/// the same rule [`guarded_at`] holds an arriving call to.
fn authorize_caller(
    caller: &crate::plugin::bindings::losos::plugin::types::Caller,
    method: &str,
    route: &str,
) -> Result<(), String> {
    use crate::plugin::bindings::losos::plugin::types::Principal as P;
    let section = crate::sections::section(method, route);
    match &caller.principal {
        P::Spare | P::Host => Ok(()),
        P::Anonymous if section == "public" => Ok(()),
        P::Anonymous => Err("unauthorized".to_string()),
        P::Edge if crate::sections::edge_allows(method, section) => Ok(()),
        P::Edge => Err("the edge may not use this part of the box".to_string()),
        P::Person(p) => {
            let person = crate::people::Person {
                name: p.name.clone(),
                display_name: p.display_name.clone(),
                groups: p.groups.clone(),
            };
            let locked = person.groups.iter().any(|g| g == crate::people::LOCKED);
            if locked && !matches!(section, "account" | "signed-in" | "public") {
                return Err("finish what LosOS Login asks of your account first".to_string());
            }
            if !(locked && section == "account")
                && !crate::sections::allows(&person, method, section)
            {
                return Err("your account cannot use this part of the box".to_string());
            }
            Ok(())
        }
    }
}

/// A plugin's `dispatch.call` to one of this daemon's own routes: replayed
/// on the loopback API as the caller, the way a terminal call is
/// ([`crate::access::forward_raw`]). A person goes as `X-Losos-As` under the
/// admin token, the spare key and the host as the spare key, and nobody
/// with no credential at all, so the route's own checks decide as they
/// would for the page. The edge never reaches the host this way.
fn host_call(port: u16, token: String) -> std::sync::Arc<crate::plugin::HostCall> {
    use crate::plugin::bindings::losos::plugin::types::{Principal as P, Response};
    std::sync::Arc::new(move |request| {
        let refused = |status: u16, why: &str| Response {
            status,
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            body: serde_json::to_vec(&serde_json::json!({ "error": why })).unwrap_or_default(),
        };
        let (credential, person) = match &request.caller.principal {
            P::Anonymous => (None, None),
            P::Spare | P::Host => (Some(token.as_str()), None),
            P::Person(p) => (Some(token.as_str()), Some(p.name.as_str())),
            P::Edge => return Some(refused(403, "the edge may not use this part of the box")),
        };
        let path = if request.query.is_empty() {
            request.path.clone()
        } else {
            format!("{}?{}", request.path, request.query)
        };
        let content_type = request
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.as_str());
        match crate::access::forward_raw(
            port,
            credential,
            &request.caller.remote,
            person,
            &request.method,
            &path,
            content_type,
            &request.body,
        ) {
            Ok(answer) => Some(Response {
                status: answer.status,
                headers: answer
                    .content_type
                    .map(|v| vec![("content-type".to_string(), v)])
                    .unwrap_or_default(),
                body: answer.body,
            }),
            Err(e) => {
                tracing::error!(error = %e, path = %request.path, "a plugin's call to the host failed");
                Some(refused(502, "the call failed on the box"))
            }
        }
    })
}

/// The caller as a plugin sees it.
fn plugin_caller(
    who: Option<&Principal>,
    remote: String,
    section: &str,
) -> crate::plugin::bindings::losos::plugin::types::Caller {
    use crate::plugin::bindings::losos::plugin::types as t;
    let principal = match who {
        None => t::Principal::Anonymous,
        Some(Principal::Spare) => t::Principal::Spare,
        Some(Principal::Edge) => t::Principal::Edge,
        // No plugin route is a companion route (`guarded_at` refuses it
        // before a plugin is asked), and a plugin's own calls would run
        // with this caller: nobody, then.
        Some(Principal::Companion(_) | Principal::Backups(_) | Principal::Directory(_)) => {
            t::Principal::Anonymous
        }
        Some(Principal::Person(p)) => t::Principal::Person(t::Person {
            name: p.name.clone(),
            display_name: p.display_name.clone(),
            groups: p.groups.clone(),
            admin: p.is_admin(),
        }),
    };
    t::Caller {
        principal,
        remote,
        section: section.to_string(),
    }
}

/// Headers a plugin never sees: the credentials this daemon already spent.
fn passed_header(name: &str) -> bool {
    !matches!(name, "authorization" | "cookie" | "x-losos-edge")
}

/// Headers of a plugin's answer that are the transport's business.
fn kept_header(name: &str) -> bool {
    !matches!(
        name,
        "content-length" | "transfer-encoding" | "connection" | "keep-alive" | "upgrade"
    )
}

async fn plugin_route(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    let method = req.method().as_str().to_ascii_uppercase();
    let Some(matched) = api.plugins.route(&method, req.path()) else {
        return err(StatusCode::NOT_FOUND, "not found");
    };
    if body.len() > matched.route.max_body {
        return err(
            StatusCode::PAYLOAD_TOO_LARGE,
            "the request body is too large",
        );
    }
    let pattern = matched.route.path.clone();
    let section = crate::sections::section(&method, &pattern);
    let call = |who: Option<&Principal>| {
        use crate::plugin::bindings::losos::plugin::types as t;
        let request = t::Request {
            method: method.clone(),
            path: req.path().to_string(),
            route: pattern.clone(),
            params: matched.params.clone(),
            query: req.query_string().to_string(),
            headers: req
                .headers()
                .iter()
                .filter(|(k, _)| passed_header(k.as_str()))
                .map(|(k, v)| {
                    (
                        k.as_str().to_string(),
                        String::from_utf8_lossy(v.as_bytes()).into_owned(),
                    )
                })
                .collect(),
            body: body.to_vec(),
            caller: plugin_caller(who, remote_addr(&req), section),
        };
        let answer = api.plugins.handle(&matched.plugin, &request);
        let mut resp = HttpResponse::build(
            StatusCode::from_u16(answer.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        );
        for (k, v) in &answer.headers {
            if kept_header(&k.to_ascii_lowercase()) {
                resp.append_header((k.as_str(), v.as_str()));
            }
        }
        resp.body(answer.body)
    };
    // nginx still asks LosOS Login about an unguarded route; this daemon
    // does not, because the browser cannot send it a token there.
    if matched.route.unguarded || section == "public" {
        return call(None);
    }
    guarded_at(
        &api,
        &req,
        &pattern,
        Some(&pattern),
        method != "GET",
        |who| call(Some(who)),
    )
}

// ── The owner's own apps ──────────────────────────────────────────────────
// Apps the builder wrote or the owner typed (`crate::user_apps`), and the
// data each one keeps. Making and changing them is the apps section's;
// opening one and its data is anyone's who is signed in, since the people
// on a box share its apps the way a family shares a list.

fn user_app_missing() -> HttpResponse {
    err(
        StatusCode::NOT_FOUND,
        "there is no app with that id on this box",
    )
}

/// `GET /api/user-apps`.
async fn get_user_apps(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/user-apps", false, || {
        run(&api, crate::user_apps::cmd_list)
    })
}

/// `GET /api/user-apps/{id}` — one app, with its source and its script.
async fn get_user_app(
    api: web::Data<Api>,
    req: HttpRequest,
    id: web::Path<String>,
) -> HttpResponse {
    guarded(&api, &req, "/api/user-apps", false, || {
        match api
            .backend
            .serialized(|b| crate::user_apps::cmd_get(b, &id))
        {
            Ok(Some(v)) => HttpResponse::Ok().json(v),
            Ok(None) => user_app_missing(),
            Err(e) => error_response(e),
        }
    })
}

/// `POST /api/user-apps` `{"id"?, "name", "summary"?, "prompt"?, "source"}`
/// — add an app, or save a new version of one.
async fn post_user_app(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    guarded(&api, &req, "/api/user-apps", true, || {
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        match crate::user_apps::parse_draft(&doc) {
            Some(draft) => run(&api, |b| {
                let saved = crate::user_apps::cmd_put(b, &draft);
                if saved.is_ok() {
                    crate::config_repo::nudge();
                }
                saved
            }),
            None => err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"id": "..." (optional), "name": "...", "source": "..."}"#,
            ),
        }
    })
}

/// `POST /api/user-apps/check` `{"source"}` — check and compile, saving
/// nothing: the code editor's preview.
async fn post_user_app_check(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded(&api, &req, "/api/user-apps/check", false, || {
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        match doc.get("source").and_then(serde_json::Value::as_str) {
            Some(source) => match crate::user_apps::cmd_check(source) {
                Ok(v) => HttpResponse::Ok().json(v),
                Err(e) => error_response(e),
            },
            None => err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"source": "..."}"#,
            ),
        }
    })
}

/// `DELETE /api/user-apps/{id}` — the app and its data.
async fn delete_user_app(
    api: web::Data<Api>,
    req: HttpRequest,
    id: web::Path<String>,
) -> HttpResponse {
    guarded(&api, &req, "/api/user-apps", true, || {
        run(&api, |b| crate::user_apps::cmd_delete(b, &id))
    })
}

/// `GET /api/user-apps/{id}/data` — what the app keeps.
async fn get_user_app_data(
    api: web::Data<Api>,
    req: HttpRequest,
    id: web::Path<String>,
) -> HttpResponse {
    guarded(&api, &req, "/api/user-apps/data", false, || {
        match api
            .backend
            .serialized(|b| crate::user_apps::cmd_data(b, &id))
        {
            Ok(Some(v)) => HttpResponse::Ok().json(v),
            Ok(None) => user_app_missing(),
            Err(e) => error_response(e),
        }
    })
}

/// `POST /api/user-apps/{id}/data` `{"op": ...}` — one write to it.
async fn post_user_app_data(
    api: web::Data<Api>,
    req: HttpRequest,
    id: web::Path<String>,
    body: web::Bytes,
) -> HttpResponse {
    guarded(&api, &req, "/api/user-apps/data", true, || {
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        let Some(op) = crate::user_apps::parse_op(&doc) else {
            return err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"op": "add"|"update"|"remove"|"replace"|"set", ...}"#,
            );
        };
        match api
            .backend
            .serialized(|b| crate::user_apps::cmd_data_op(b, &id, &op))
        {
            Ok(Some(v)) => HttpResponse::Ok().json(v),
            Ok(None) => user_app_missing(),
            Err(e) => error_response(e),
        }
    })
}

// ── The pin picker ────────────────────────────────────────────────────────
// The two lists the sidebar's picker pins from, LosOS Cloud's files and
// LosOS Git's repositories (`crate::pins`). The pins themselves are the
// pins plugin's (`plugins/pins/`).

/// `GET /api/cloud/files?path=/...` — one folder of LosOS Cloud, or with
/// `q=...` the files and folders anywhere whose name holds the text. The
/// pin picker reads it; so can anything else on the page that looks files up.
async fn get_cloud_files(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/cloud/files", false, || {
        let q = query_map(&req);
        let path = q.get("path").cloned().unwrap_or_else(|| "/".to_string());
        run(&api, |b| {
            crate::pins::cmd_cloud_files(b, &path, q.get("q").map(String::as_str))
        })
    })
}

/// `GET /api/git/repos[?q=...]` — the owner's repositories on LosOS Git.
async fn get_git_repos(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/git/repos", false, || {
        let q = query_map(&req);
        run(&api, |b| {
            crate::pins::cmd_git_repos(b, q.get("q").map(String::as_str))
        })
    })
}

fn query_map(req: &HttpRequest) -> std::collections::HashMap<String, String> {
    web::Query::<std::collections::HashMap<String, String>>::from_query(req.query_string())
        .map(web::Query::into_inner)
        .unwrap_or_default()
}

// ── Every option, and the configuration repository ───────────────────────

/// `GET /api/options` — every `losos.*` option the box declares, with its
/// editor kind and what `overrides.nix` sets (`crate::options`). The
/// Advanced pane is drawn from this. A box without the document answers
/// `available: false`, deliberately not 404, which the SPA latches.
async fn get_options(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/options", false, || run(&api, cmd_options))
}

/// `GET /api/site` — the settings of LosOS Cloud and LosOS Git only
/// (`crate::site`), for the apps section and a managing edge, which may not
/// read every option.
async fn get_site(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/site", false, || {
        run(&api, crate::losos::cmd_site)
    })
}

/// `POST /api/site` — `{"set": {"cloud.trashDays": "30",
/// "git.site.pushCreate": null}}`: each value a Nix literal, or null to
/// go back to the default. Only those lines of `overrides.nix` change; the
/// rest is an apply.
async fn post_site(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    guarded(&api, &req, "/api/site", true, || {
        let set = serde_json::from_slice::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("set").and_then(|s| s.as_object()).cloned());
        let Some(set) = set else {
            return err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"set": {"<option>": "<Nix literal>" | null}}"#,
            );
        };
        let mut changes = Vec::with_capacity(set.len());
        for (name, value) in set {
            match value {
                serde_json::Value::Null => changes.push((name, None)),
                serde_json::Value::String(raw) => changes.push((name, Some(raw))),
                _ => {
                    return err(
                        StatusCode::BAD_REQUEST,
                        "each value must be a Nix literal as a string, or null",
                    )
                }
            }
        }
        run(&api, |l| crate::losos::cmd_site_set(l, &changes))
    })
}

/// `GET /api/config` — the configuration repository as the History pane
/// shows it (`crate::config_repo`).
async fn get_config(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/config", false, || run(&api, cmd_config))
}

/// `POST /api/config/sync` — one sync with LosOS Git, now, then the same
/// document as the read. Mutating (it can push, fast-forward and rebuild),
/// so audited.
async fn post_config_sync(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/config/sync", true, || {
        run(&api, cmd_config_sync)
    })
}

// ── Terminal access (crate::access, docs/terminal-access.md) ─────────────────

/// The access state, or the sentence that says why there is none.
fn access_ready(api: &Api) -> Result<&crate::access::Ready, &str> {
    match &api.access {
        crate::access::Access::Ready(r) => Ok(r),
        crate::access::Access::Broken(why) => Err(why),
    }
}

/// The 503 for a box whose terminal access is off.
fn access_off(why: &str) -> HttpResponse {
    err(StatusCode::SERVICE_UNAVAILABLE, why)
}

/// `GET /api/access` — the box's key, the seeded keys and the tokens.
///
/// A person sees their own tokens; an administrator sees everyone's.
async fn get_access(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded_as(&api, &req, "/api/access", false, |who| {
        match access_ready(&api) {
            Ok(ready) => match crate::access::overview(ready) {
                Ok(mut v) => {
                    if !who.is_admin() {
                        let me = who.person().map(|p| p.name.as_str());
                        if let Some(tokens) = v.get_mut("tokens").and_then(|t| t.as_array_mut()) {
                            tokens.retain(|t| t.get("owner").and_then(|o| o.as_str()) == me);
                        }
                    }
                    HttpResponse::Ok().json(v)
                }
                Err(e) => error_response(e),
            },
            Err(why) => access_off(why),
        }
    })
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NewToken {
    name: String,
    fingerprint: String,
    scope: losos_access::Scope,
    expires_in_days: Option<u32>,
}

fn token_error(e: losos_access::TokenError) -> HttpResponse {
    use losos_access::TokenError;
    match e {
        TokenError::Invalid(why) => err(StatusCode::BAD_REQUEST, &why),
        TokenError::Full => err(StatusCode::CONFLICT, &TokenError::Full.to_string()),
        TokenError::NotFound => err(StatusCode::NOT_FOUND, &TokenError::NotFound.to_string()),
        e @ TokenError::Io { .. } => error_response(anyhow::Error::new(e)),
    }
}

/// `POST /api/access/tokens` — make a token; the answer is the only time
/// the token itself leaves the box.
async fn post_access_token(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded_as(&api, &req, "/api/access/tokens", true, |who| {
        let ready = match access_ready(&api) {
            Ok(r) => r,
            Err(why) => return access_off(why),
        };
        let new: NewToken = match serde_json::from_slice(&body) {
            Ok(n) => n,
            Err(_) => {
                return err(
                    StatusCode::BAD_REQUEST,
                    r#"body must be JSON: {"name", "fingerprint", "scope": "read"|"full", "expiresInDays": number|null}"#,
                )
            }
        };
        match ready.tokens.create(
            &ready.ring,
            &new.name,
            &new.fingerprint,
            new.scope,
            new.expires_in_days,
            // The token acts as whoever made it.
            who.person().map(|p| p.name.as_str()),
            losos_access::now_secs(),
        ) {
            Ok((token, record)) => {
                HttpResponse::Ok().json(serde_json::json!({ "token": token, "record": record }))
            }
            Err(e) => token_error(e),
        }
    })
}

/// `POST /api/access/tokens/revoke` — `{"id"}`.
async fn post_access_revoke(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    guarded_as(&api, &req, "/api/access/tokens/revoke", true, |who| {
        let ready = match access_ready(&api) {
            Ok(r) => r,
            Err(why) => return access_off(why),
        };
        let id = serde_json::from_slice::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("id").and_then(|i| i.as_str()).map(str::to_string));
        let Some(id) = id else {
            return err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"id": "..."}"#,
            );
        };
        // A person revokes their own tokens; an administrator anyone's.
        if !who.is_admin() {
            let me = who.person().map(|p| p.name.clone());
            let theirs = ready
                .tokens
                .list()
                .map(|all| all.iter().any(|t| t.id == id && t.owner == me))
                .unwrap_or(false);
            if !theirs {
                return token_error(losos_access::TokenError::NotFound);
            }
        }
        match ready.tokens.revoke(&id) {
            Ok(()) => HttpResponse::Ok().json(serde_json::json!({ "revoked": true })),
            Err(e) => token_error(e),
        }
    })
}

/// `GET /api/access/host-key` — the box's public key, for the terminal to
/// pin on first contact, and where it reaches the box through the edge when
/// that is open to it (`edgeUrl`). Public by nature; LAN-only like all of
/// `/api/` unless losos.access.throughEdge lets the tunnel through to it.
async fn get_access_host_key(api: web::Data<Api>) -> HttpResponse {
    match access_ready(&api) {
        Ok(ready) => HttpResponse::Ok().json(serde_json::json!({
            "fingerprint": ready.host.fingerprint(),
            "publicKey": ready.host.armored_public(),
            "edgeUrl": ready.edge_url,
        })),
        Err(why) => access_off(why),
    }
}

/// `POST /api/access/call` — one terminal call. No Bearer: the envelope
/// carries the token and the signature, and every refusal before the call
/// runs counts against the same per-address throttle as a wrong Bearer.
async fn post_access_call(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    const ROUTE: &str = "/api/access/call";
    let remote = remote_addr(&req);
    if let Err(wait) = api.throttle.check(&remote, Instant::now()) {
        api.audit.record(ROUTE, "throttled", &remote);
        let mut resp = err(
            StatusCode::TOO_MANY_REQUESTS,
            "too many failed attempts; slow down",
        );
        resp.headers_mut().insert(
            actix_web::http::header::RETRY_AFTER,
            retry_after_secs(wait).to_string().parse().expect("digits"),
        );
        return resp;
    }
    if body.len() > crate::access::MAX_ENVELOPE_BYTES {
        return err(
            StatusCode::PAYLOAD_TOO_LARGE,
            "a terminal call is at most 1 MiB",
        );
    }
    if let Err(why) = access_ready(&api) {
        return access_off(why);
    }
    // Off the worker: the crypto is cheap but not free, and the inner call
    // comes back into this same server, which on a one-core box has one
    // worker to answer it with.
    let worker = api.clone();
    match web::block(move || call(&worker, &remote, &body)).await {
        Ok(Called {
            status,
            sealed,
            body,
        }) => HttpResponse::build(
            StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        )
        .content_type(if sealed {
            "application/pgp-encrypted"
        } else {
            "application/json"
        })
        .body(body),
        Err(e) => {
            tracing::error!(error = %e, "terminal call did not finish");
            err(StatusCode::INTERNAL_SERVER_ERROR, "the call did not finish")
        }
    }
}

/// A terminal call's answer, built off the worker. `HttpResponse` is not
/// `Send`, so the handler turns this into one.
struct Called {
    status: u16,
    /// An armoured answer for the client's key, or a plain JSON refusal.
    sealed: bool,
    body: String,
}

impl Called {
    fn plain(status: u16, msg: &str) -> Called {
        Called {
            status,
            sealed: false,
            body: serde_json::json!({ "error": msg }).to_string(),
        }
    }
}

fn call(api: &Api, remote: &str, body: &[u8]) -> Called {
    const ROUTE: &str = "/api/access/call";
    let crate::access::Access::Ready(ready) = &api.access else {
        return Called::plain(503, "terminal access is off on this box");
    };
    let now = losos_access::now_secs();
    let refuse = |r: losos_access::Refusal| {
        api.throttle.record_failure(remote, Instant::now());
        api.audit.record(ROUTE, "unauthorized", remote);
        Called::plain(r.status(), &r.to_string())
    };
    let opened = match losos_access::open(&ready.host, &ready.ring, body, &ready.replay, now) {
        Ok(o) => o,
        Err(r) => return refuse(r),
    };
    let token = match crate::access::authorize(ready, &opened, now) {
        Ok(t) => t,
        Err(r) => return refuse(r),
    };
    let token_id = &token.id;
    api.throttle.record_success(remote);
    let r = &opened.request;
    let route_only = r.path.split('?').next().unwrap_or(&r.path);
    let audit_route = format!("{ROUTE} {token_id} {} {route_only}", r.method);
    let (status, answer) = match crate::access::forward(
        api.port,
        &api.token,
        remote,
        token.owner.as_deref(),
        &r.method,
        &r.path,
        &r.body,
    ) {
        Ok(inner) => (inner.status, inner.body),
        Err(e) => {
            tracing::error!(error = %e, path = route_only, "terminal call: the inner request failed");
            (
                500,
                serde_json::json!({ "error": "the call failed on the box" }),
            )
        }
    };
    match losos_access::seal(&ready.host, &opened.signer, &r.nonce, status, answer) {
        Ok(sealed) => {
            let outcome = match status {
                0..=399 => "ok",
                400..=499 => "rejected",
                _ => "error",
            };
            api.audit.record(&audit_route, outcome, remote);
            Called {
                status,
                sealed: true,
                body: sealed,
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "terminal call: sealing the answer failed");
            api.audit.record(&audit_route, "error", remote);
            Called::plain(500, "the answer could not be sealed")
        }
    }
}

/// Whether `candidate` is a token this daemon would have minted: exactly
/// [`TOKEN_HEX_LEN`] lowercase hex characters.
fn is_well_formed(candidate: &str) -> bool {
    candidate.len() == TOKEN_HEX_LEN
        && candidate
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// Read the admin token, minting one when the file is missing or unusable.
///
/// A *well-formed* existing file is left alone, so the token survives restarts;
/// rotating it means deleting the file and restarting `lososd`.
///
/// Anything else is replaced rather than trusted. The old code returned the
/// file's contents verbatim, which made whatever happened to be there the
/// shared secret: a one-character file authenticated `Bearer x`, and a write
/// interrupted by the nightly reboot could leave exactly that. Validating costs
/// nothing and the failure mode it removes is remote root — `POST /api/apply`
/// writes Nix and runs `nixos-rebuild switch`.
pub fn ensure_token(path: &Path) -> anyhow::Result<String> {
    match std::fs::read_to_string(path) {
        Ok(existing) => {
            let candidate = existing.trim();
            if is_well_formed(candidate) {
                return Ok(candidate.to_string());
            }
            tracing::error!(
                path = %path.display(),
                bytes = existing.len(),
                "admin token file is not {TOKEN_HEX_LEN} lowercase hex characters; \
                 discarding it and minting a new token — any client holding the old \
                 one must be re-pointed at the new file"
            );
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(anyhow::Error::new(e).context(format!("reading {}", path.display()))),
    }
    mint_token(path)
}

/// Generate a token and write it out at 0600, atomically.
fn mint_token(path: &Path) -> anyhow::Result<String> {
    let token = crate::io_backend::hex(&crate::io_backend::random_bytes::<TOKEN_BYTES>()?);
    atomic_write_secret(path, token.as_bytes())
        .with_context(|| format!("writing {}", path.display()))?;
    tracing::info!(path = %path.display(), "minted admin token");
    Ok(token)
}

/// The port to bind, from `$LOSOS_ADMIN_PORT`.
///
/// A malformed value is fatal on purpose. Falling back to the default would
/// bind a port Nginx is not proxying to and report nothing, leaving an admin UI
/// that is simply dead with no clue anywhere as to why.
fn admin_port() -> anyhow::Result<u16> {
    let raw = match std::env::var("LOSOS_ADMIN_PORT") {
        Ok(raw) => raw,
        Err(std::env::VarError::NotPresent) => return Ok(DEFAULT_PORT),
        Err(e) => return Err(anyhow::Error::new(e).context("LOSOS_ADMIN_PORT")),
    };
    let port: u16 = raw
        .trim()
        .parse()
        .with_context(|| format!("LOSOS_ADMIN_PORT is not a TCP port number: {raw:?}"))?;
    if port == 0 {
        anyhow::bail!("LOSOS_ADMIN_PORT is 0; nothing could reach an ephemeral port");
    }
    Ok(port)
}

/// Put the person who claimed the box in `losos-owner`, which a box claimed
/// before the group existed lacks. nginx signs that group in to LosOS Cloud
/// and LosOS Git as the owner's own accounts (`modules/app-sign-in.nix`).
/// A box with nobody yet, or no people file, has nothing to do.
fn owner_in_owner_group(api: &Api) {
    use crate::losos::Losos;
    let owner = match api.backend.serialized(|l| l.load_state()) {
        Ok(state) => state.owner,
        Err(e) => {
            tracing::warn!(error = %e, "could not read the owner from state.json");
            return;
        }
    };
    let Some(owner) = owner else { return };
    if !api.people.path().exists() {
        return;
    }
    if let Err(e) = api.people.ensure_group(&owner, crate::people::OWNER) {
        tracing::warn!(error = %e, owner = %owner, "could not put the owner in losos-owner");
    }
}

/// Serve the API. Blocking: runs its own actix `System` on the calling thread.
///
/// Returns only on failure or on the server stopping, and the caller is
/// expected to treat either as fatal: an admin API that failed to bind while
/// the daemon stays up is invisible to systemd.
pub fn serve(backend: IoLosos, plugins: crate::plugin::Runtime) -> anyhow::Result<()> {
    let port = admin_port()?;
    let token_file =
        std::env::var("LOSOS_ADMIN_TOKEN_FILE").unwrap_or_else(|_| DEFAULT_TOKEN_FILE.to_string());
    let token = ensure_token(Path::new(&token_file))?;
    crate::io_backend::scrub_backup_target_at_start();

    actix_web::rt::System::new().block_on(async move {
        let people = crate::people::People::from_env();
        // The Windows shares' accounts live on /run: written again at
        // every start.
        if let Err(e) = people.sync_windows() {
            tracing::warn!(error = ?e, "the Windows shares could not be written");
        }
        let login = crate::login::Login::from_env(people.path());
        let api = web::Data::new(Api {
            backend,
            token,
            throttle: Throttle::default(),
            passwords: Throttle::default(),
            audit: Audit,
            claims: std::sync::Mutex::new(crate::receipt::Receipts::default()),
            access: crate::access::Access::from_env(),
            port,
            builder: crate::builder::Builder::from_env(),
            people,
            companion: crate::companion::Companion::from_env(),
            directory: crate::directory::Directory::from_env(),
            login,
            share: crate::cloud_share::Share::default(),
            chat: crate::chat::Chat::from_env(),
            git_rooms: crate::git_rooms::Rooms::from_env(),
            contacts: std::env::var("LOSOS_CLOUD_CONTACTS").is_ok_and(|v| v == "1"),
            mail: crate::mail::Mail::from_env(),
            mail_keys: crate::mail::Config::from_env().map(|c| {
                let keys = crate::mail_pgp::Keys::from_env(&c.domain);
                keys.republish();
                keys
            }),
            credentials: crate::credentials::Credentials::from_env(),
            profiles: crate::profiles::Profiles::from_env(),
            roles: crate::roles::Presets::from_env(),
            moderation: crate::moderation::Moderation::from_env(),
            pictures: crate::pictures::Pictures::from_env(),
            image_jobs: tokio::sync::Semaphore::new(IMAGE_JOBS),
            plugins,
            schizo: crate::schizo::Schizo::from_env(),
        });
        api.plugins
            .set_authorize(std::sync::Arc::new(authorize_caller));
        api.plugins.set_host(host_call(api.port, api.token.clone()));
        let payload_cap = api
            .plugins
            .max_body()
            .max(crate::profiles::MAX_UPLOAD_BYTES)
            .max(crate::mail::MAX_REQUEST);
        owner_in_owner_group(&api);
        listen_for_git_events(&api);
        start_login_marks(api.clone());
        sync_names_later(api.clone(), None);
        let server = HttpServer::new(move || {
            App::new()
                .app_data(api.clone())
                // actix reads at most 256 KiB into `web::Bytes` by default.
                // The cap is the largest `maxBody` a plugin route asks for,
                // or a profile picture's; each plugin route's own limit is
                // checked in `plugin_route`, and profiles.rs refuses a
                // picture over its limit with a sentence.
                .app_data(web::PayloadConfig::new(payload_cap))
                .route("/api/health", web::get().to(health))
                .route("/api/mail", web::get().to(get_mail))
                // A message carries its attachments, so this one route takes
                // more than the cap above.
                .service(
                    web::resource("/api/mail/send")
                        .app_data(web::PayloadConfig::new(crate::mail::MAX_SEND_BYTES))
                        .route(web::post().to(post_mail_send)),
                )
                .route("/api/mail/messages", web::get().to(get_mail_messages))
                .route("/api/mail/messages/{id}", web::get().to(get_mail_message))
                .route("/api/mail/messages/{id}", web::post().to(post_mail_message))
                .route(
                    "/api/mail/messages/{id}",
                    web::delete().to(delete_mail_message),
                )
                .route("/api/mail/messages/{id}/html", web::get().to(get_mail_html))
                .route("/api/mail/messages/{id}/raw", web::get().to(get_mail_raw))
                .route("/api/mail/keys", web::get().to(get_mail_keys))
                .route("/api/mail/keys", web::put().to(put_mail_keys))
                .route("/api/mail/keys", web::delete().to(delete_mail_keys))
                .route("/api/mail/keys/find", web::get().to(find_mail_keys))
                .route(
                    "/api/mail/attachments/{blob}",
                    web::get().to(get_mail_attachment),
                )
                .route("/api/me", web::get().to(get_me))
                .route("/api/state", web::get().to(get_state))
                .route("/api/settings", web::get().to(get_settings))
                .route("/api/status", web::get().to(get_status))
                .route("/api/edge", web::get().to(get_edge))
                .route("/api/edge/manage", web::post().to(post_edge_manage))
                .route("/api/edge/cluster", web::post().to(post_edge_cluster))
                .route("/api/change", web::post().to(post_change))
                .route("/api/apply", web::post().to(post_apply))
                .route("/api/factory-reset", web::post().to(post_factory_reset))
                .route("/api/backup", web::get().to(get_backup))
                .route("/api/credentials", web::get().to(get_credentials))
                .route(
                    "/api/credentials/{name}/rotate",
                    web::post().to(post_credential_rotate),
                )
                .route("/api/backup/target", web::post().to(post_backup_target))
                .route("/api/backup/target", web::delete().to(delete_backup_target))
                .route(
                    "/api/backup/target/bought",
                    web::post().to(post_backup_target_bought),
                )
                .route("/api/backup/run", web::post().to(post_backup_run))
                .route("/api/backup/restore", web::post().to(post_backup_restore))
                .route("/api/erase", web::post().to(post_erase))
                .route("/api/erase/cancel", web::post().to(post_erase_cancel))
                .route("/api/storage", web::get().to(get_storage))
                .route("/api/grow", web::post().to(post_grow))
                .route("/api/set-password", web::post().to(post_set_password))
                .route("/api/people", web::get().to(get_people))
                .route("/api/people", web::post().to(post_person))
                .route("/api/people/access", web::post().to(post_person_access))
                .route("/api/people/password", web::post().to(post_person_password))
                .route("/api/people/remove", web::post().to(post_person_remove))
                .route("/api/people/rules", web::post().to(post_people_rules))
                .route("/api/roles", web::post().to(post_roles))
                .route("/api/roles/remove", web::post().to(post_roles_remove))
                .route("/api/reports", web::post().to(post_report))
                .route("/api/moderation", web::get().to(get_moderation))
                .route(
                    "/api/moderation/reports/{id}",
                    web::post().to(post_moderation_report),
                )
                .route("/api/moderation/mute", web::post().to(post_moderation_mute))
                .route(
                    "/api/moderation/unmute",
                    web::post().to(post_moderation_unmute),
                )
                .route(
                    "/api/moderation/suspend",
                    web::post().to(post_moderation_suspend),
                )
                .route(
                    "/api/moderation/unsuspend",
                    web::post().to(post_moderation_unsuspend),
                )
                .route(
                    "/api/people/two-factor/reset",
                    web::post().to(post_person_factors_reset),
                )
                .route("/api/account", web::get().to(get_account))
                .route("/api/account", web::post().to(post_account))
                .route("/api/account/code", web::post().to(post_account_code))
                .route("/api/windows", web::get().to(get_windows))
                .route("/api/windows", web::post().to(post_windows))
                .route("/api/account/profile", web::post().to(post_account_profile))
                .route("/api/pictures", web::get().to(get_pictures))
                .route("/api/pictures", web::post().to(post_pictures))
                .route("/api/pictures/trust", web::post().to(post_pictures_trust))
                .route("/api/pictures/sender", web::get().to(get_pictures_sender))
                .route("/api/people/age", web::post().to(post_people_age))
                .route("/api/account/picture", web::put().to(put_account_picture))
                .route(
                    "/api/account/picture",
                    web::delete().to(delete_account_picture),
                )
                .route("/api/account/banner", web::put().to(put_account_banner))
                .route(
                    "/api/account/banner",
                    web::delete().to(delete_account_banner),
                )
                .route("/api/profiles", web::get().to(get_profiles))
                .route("/api/profiles/{name}", web::get().to(get_profile))
                .route(
                    "/api/profiles/{name}/{kind}",
                    web::get().to(get_profile_image),
                )
                .route(
                    "/api/people/profile/clear",
                    web::post().to(post_people_profile_clear),
                )
                .route("/api/recovery", web::get().to(get_recovery))
                .route("/api/schizo", web::get().to(get_schizo))
                .route("/api/schizo", web::post().to(post_schizo))
                .route(
                    "/api/login-devices/self",
                    web::post().to(post_login_device_self),
                )
                .route(
                    "/api/login-devices/self/ack",
                    web::post().to(post_login_device_ack),
                )
                .route(
                    "/api/login-devices/self/duress",
                    web::post().to(post_login_device_duress),
                )
                .route("/api/devices/backups", web::post().to(post_device_backups))
                .route(
                    "/api/devices/directory",
                    web::get().to(get_device_directory),
                )
                .route(
                    "/api/devices/directory",
                    web::post().to(post_device_directory),
                )
                .route(
                    "/api/devices/directory/{id}",
                    web::delete().to(delete_device_directory),
                )
                .route("/api/directory/people", web::get().to(get_directory_people))
                // Written out, as every route here is, so the sections test
                // reads them; `directory::JOBS_*_ROUTE` are the same strings.
                .route(
                    "/api/directory/jobs/claim",
                    web::post().to(post_directory_jobs),
                )
                .route(
                    "/api/directory/jobs/report",
                    web::post().to(post_directory_jobs),
                )
                .route("/api/devices/{kind}", web::post().to(post_device))
                .route("/api/apps/search", web::get().to(get_apps_search))
                .route("/api/cloud/text", web::get().to(get_cloud_text))
                .route("/api/git/text", web::get().to(get_git_text))
                .route("/api/apps", web::get().to(get_apps))
                .route("/api/apps/chart", web::get().to(get_app_chart))
                .route("/api/apps/install", web::post().to(post_app_install))
                .route("/api/apps/compose", web::post().to(post_app_compose))
                .route("/api/apps/remove", web::post().to(post_app_remove))
                .route("/api/market", web::get().to(get_market))
                .route("/api/market/onboard", web::post().to(post_market_onboard))
                .route("/api/market/listings", web::post().to(post_market_listing))
                .route(
                    "/api/market/listings/close",
                    web::post().to(post_market_close),
                )
                .route("/api/market/orders", web::post().to(post_market_order))
                .route("/api/market/payout", web::post().to(post_market_payout))
                .route("/api/market/release", web::post().to(post_market_release))
                .route("/api/market/confirm", web::post().to(post_market_confirm))
                .route("/api/market/fees/pay", web::post().to(post_market_fees_pay))
                .route(
                    "/api/market/coins/key",
                    web::post().to(post_market_coin_key),
                )
                .route(
                    "/api/market/coins/pay",
                    web::post().to(post_market_coin_pay),
                )
                .route("/api/market/rpc", web::post().to(post_market_rpc))
                .route("/api/user-apps", web::get().to(get_user_apps))
                .route("/api/user-apps", web::post().to(post_user_app))
                .route("/api/user-apps/check", web::post().to(post_user_app_check))
                .route("/api/user-apps/{id}", web::get().to(get_user_app))
                .route("/api/user-apps/{id}", web::delete().to(delete_user_app))
                .route("/api/user-apps/{id}/data", web::get().to(get_user_app_data))
                .route(
                    "/api/user-apps/{id}/data",
                    web::post().to(post_user_app_data),
                )
                .route("/api/market/onramp", web::post().to(post_market_onramp))
                .route(
                    "/api/market/onramp/status",
                    web::post().to(post_market_onramp_status),
                )
                .route(
                    "/api/market/llm/models",
                    web::post().to(post_market_llm_models),
                )
                .route("/api/market/llm/chat", web::post().to(post_market_llm_chat))
                .route("/api/market/taps", web::post().to(post_market_taps))
                .route(
                    "/api/market/taps/confirm",
                    web::post().to(post_market_taps_confirm),
                )
                .route(
                    "/api/market/taps/stop",
                    web::post().to(post_market_taps_stop),
                )
                .route("/api/builder", web::get().to(get_builder))
                .route("/api/builder/builds", web::post().to(post_builder_start))
                .route("/api/builder/builds/{id}", web::get().to(get_builder_build))
                .route(
                    "/api/builder/builds/{id}",
                    web::delete().to(delete_builder_build),
                )
                .route(
                    "/api/builder/builds/{id}/tried",
                    web::post().to(post_builder_tried),
                )
                .route("/api/vms", web::get().to(get_vms))
                .route("/api/vms/orders", web::post().to(post_vm_order))
                .route("/api/vms/listings", web::post().to(post_vm_listing))
                .route(
                    "/api/vms/listings/close",
                    web::post().to(post_vm_listing_close),
                )
                .route("/api/vms/images", web::put().to(put_vm_image))
                .route(
                    "/api/vms/images/remove",
                    web::post().to(post_vm_image_remove),
                )
                .route("/api/lab/hello", web::get().to(get_lab_hello))
                .route("/api/lab/order", web::get().to(get_lab_order))
                .route("/api/lab/order", web::post().to(post_lab_order))
                .route("/api/lab/guests", web::post().to(post_lab_guest))
                .route("/api/lab/guests/{key}", web::delete().to(delete_lab_guest))
                .route("/api/lab/virt-ticket", web::post().to(post_lab_virt_ticket))
                .route("/api/domains", web::get().to(get_domains))
                .route("/api/domains", web::post().to(post_domain_add))
                .route("/api/domains/remove", web::post().to(post_domain_remove))
                .route("/api/domains/wallet", web::post().to(post_domain_wallet))
                .route("/api/options", web::get().to(get_options))
                .route("/api/site", web::get().to(get_site))
                .route("/api/site", web::post().to(post_site))
                .route("/api/config", web::get().to(get_config))
                .route("/api/config/sync", web::post().to(post_config_sync))
                .route("/api/cloud/files", web::get().to(get_cloud_files))
                .route("/api/git/repos", web::get().to(get_git_repos))
                // The web UI's side of terminal access (crate::access):
                // Bearer-authed like every route above.
                .route("/api/access", web::get().to(get_access))
                .route("/api/access/tokens", web::post().to(post_access_token))
                .route(
                    "/api/access/tokens/revoke",
                    web::post().to(post_access_revoke),
                )
                // The terminal's side: no Bearer, by nature. The host key is
                // public, and a call carries its own two credentials inside
                // the envelope (crate::access, docs/terminal-access.md).
                .route("/api/access/host-key", web::get().to(get_access_host_key))
                .route("/api/access/call", web::post().to(post_access_call))
                // The unauthenticated routes besides /api/health. These two
                // are first-run only: the state read is a single bit, and
                // the claim refuses once the box has an owner.
                .route("/api/setup/claim", web::get().to(get_claim_state))
                .route("/api/setup/claim", web::post().to(post_claim))
                // Asked by nginx alone, never proxied to a client
                // (crate::cloud_share, modules/identity.nix).
                .route("/internal/cloud-share", web::get().to(get_cloud_share))
                .route("/api/chat", web::get().to(get_chat))
                .route("/api/chat/session", web::post().to(post_chat_session))
                .route("/api/chat/people", web::get().to(get_chat_people))
                .route("/api/chat/openid", web::post().to(post_chat_openid))
                .route("/api/chat/git-room", web::post().to(post_chat_git_room))
                .route("/api/contacts/suggest", web::get().to(get_contacts_suggest))
                .route("/internal/chat/authz", web::get().to(get_chat_authz))
                // Everything else is a plugin's, or nobody's (crate::plugin).
                .default_service(web::route().to(plugin_route))
        })
        .workers(HTTP_WORKERS)
        .client_request_timeout(HEAD_TIMEOUT)
        .bind(("127.0.0.1", port))
        .with_context(|| format!("binding 127.0.0.1:{port}"))?;
        tracing::info!(port, "admin API listening on loopback");
        server.run().await.context("admin HTTP server")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: Option<&str> = Some("application/json");

    #[test]
    fn a_chat_body_is_read_field_by_field_and_a_wrong_kind_is_missing() {
        let doc = serde_json::json!({
            "meter": "mtr_ab",
            "messages": [{ "role": "user", "content": "hi" }],
            "maxTokens": 32,
            "temperature": 0.2,
            "stop": "END",
        });
        let ask = chat_ask(&doc).expect("a chat");
        assert_eq!(ask.meter, "mtr_ab");
        assert_eq!(ask.messages, vec![("user".to_string(), "hi".to_string())]);
        assert_eq!(ask.max_tokens, Some(32));
        assert_eq!(ask.temperature, Some(0.2));
        assert_eq!(ask.top_p, None);
        assert_eq!(ask.stop, vec!["END".to_string()]);
        for bad in [
            serde_json::json!({ "messages": [] }),
            serde_json::json!({ "meter": "mtr_ab", "messages": "hi" }),
            serde_json::json!({ "meter": "mtr_ab", "messages": [{ "role": "user" }] }),
            serde_json::json!({ "meter": "mtr_ab", "messages": [], "maxTokens": -1 }),
            serde_json::json!({ "meter": "mtr_ab", "messages": [], "temperature": "hot" }),
            serde_json::json!({ "meter": "mtr_ab", "messages": [], "stop": [1] }),
        ] {
            assert!(chat_ask(&bad).is_none(), "{bad}");
        }
        assert_eq!(
            field_units(
                &serde_json::json!({ "spendPerHour": "250000" }),
                "spendPerHour"
            ),
            Some(250_000)
        );
        assert_eq!(
            field_units(
                &serde_json::json!({ "spendPerHour": 250_000 }),
                "spendPerHour"
            ),
            Some(250_000)
        );
        assert_eq!(
            field_units(
                &serde_json::json!({ "spendPerHour": "2.5" }),
                "spendPerHour"
            ),
            None
        );
    }

    #[test]
    fn claim_from_the_box_own_page_passes() {
        assert_eq!(
            claim_fault(
                JSON,
                Some("mattbox.local"),
                Some("http://mattbox.local"),
                "mattbox"
            ),
            None
        );
        assert_eq!(
            claim_fault(
                Some("application/json; charset=utf-8"),
                Some("MattBox.local:443"),
                Some("https://mattbox.local"),
                "mattbox"
            ),
            None
        );
        assert_eq!(
            claim_fault(
                JSON,
                Some("192.168.1.20"),
                Some("http://192.168.1.20"),
                "mattbox"
            ),
            None
        );
        assert_eq!(
            claim_fault(JSON, Some("[fe80::1]:80"), None, "mattbox"),
            None
        );
        // curl on the box, or a VM test talking to lososd directly.
        assert_eq!(
            claim_fault(JSON, Some("127.0.0.1:8082"), None, "mattbox"),
            None
        );
    }

    #[test]
    fn claim_as_a_simple_request_is_refused() {
        // What a cross-site page sends to skip the CORS preflight.
        assert!(claim_fault(Some("text/plain"), Some("mattbox.local"), None, "mattbox").is_some());
        assert!(claim_fault(None, Some("mattbox.local"), None, "mattbox").is_some());
        assert!(claim_fault(
            Some("application/x-www-form-urlencoded"),
            Some("mattbox.local"),
            None,
            "mattbox"
        )
        .is_some());
    }

    #[test]
    fn claim_under_dns_rebinding_is_refused() {
        // Same-origin to the browser, but the Host is the attacker's name.
        assert!(claim_fault(
            JSON,
            Some("evil.example:80"),
            Some("http://evil.example"),
            "mattbox"
        )
        .is_some());
        assert!(claim_fault(JSON, Some("evil.example"), None, "mattbox").is_some());
        assert!(claim_fault(JSON, Some("mattbox.evil.example"), None, "mattbox").is_some());
        assert!(claim_fault(JSON, None, None, "mattbox").is_some());
        // An unreadable hostname leaves only IP literals.
        assert!(claim_fault(JSON, Some(".local"), None, "").is_some());
    }

    #[test]
    fn claim_from_another_origin_is_refused() {
        assert!(claim_fault(
            JSON,
            Some("mattbox.local"),
            Some("https://evil.example"),
            "mattbox"
        )
        .is_some());
        assert!(claim_fault(JSON, Some("mattbox.local"), Some("null"), "mattbox").is_some());
    }

    #[test]
    fn token_is_64_hex_chars_mode_600_and_stable_across_calls() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("losos-token-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("token");
        let _ = std::fs::remove_file(&path);

        let first = ensure_token(&path).unwrap();
        assert_eq!(first.len(), 64, "the VM test asserts exactly 64 chars");
        assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(first.chars().all(|c| !c.is_ascii_uppercase()));

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        // No trailing newline: the file content is exactly the token.
        assert_eq!(std::fs::read_to_string(&path).unwrap(), first);

        // An existing token is reused, never regenerated.
        assert_eq!(ensure_token(&path).unwrap(), first);

        std::fs::remove_dir_all(&dir).ok();
    }
}
