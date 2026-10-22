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
    /// LosOS Login's rules and second factors (`crate::login`).
    login: crate::login::Login,
    /// LosOS Login names on LosOS Cloud's share (`crate::cloud_share`).
    share: crate::cloud_share::Share,
    /// LosOS Chat's homeserver, as far as lososd runs it (`crate::chat`).
    chat: crate::chat::Chat,
    /// LosOS Mail (`crate::mail`).
    mail: crate::mail::Mail,
    /// The box's credential store (`crate::credentials`).
    credentials: crate::credentials::Credentials,
    /// Each person's picture, banner and the rest of their profile
    /// (`crate::profiles`).
    profiles: crate::profiles::Profiles,
    /// Turns at decoding an upload (`IMAGE_JOBS`).
    image_jobs: tokio::sync::Semaphore,
    /// The plugins that answer every route not registered below
    /// (`crate::plugin`).
    plugins: crate::plugin::Runtime,
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
}

impl Principal {
    fn audit_name(&self) -> &str {
        match self {
            Principal::Spare => "spare-key",
            Principal::Person(p) | Principal::Companion(p) => &p.name,
            Principal::Edge => "edge",
        }
    }

    fn is_admin(&self) -> bool {
        match self {
            Principal::Spare => true,
            Principal::Person(p) => p.is_admin(),
            Principal::Edge | Principal::Companion(_) => false,
        }
    }

    fn person(&self) -> Option<&crate::people::Person> {
        match self {
            Principal::Spare | Principal::Edge => None,
            Principal::Person(p) | Principal::Companion(p) => Some(p),
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
        return match lookup(&grant.person)? {
            Principal::Person(p) => Some(Principal::Companion(p)),
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
        if api.people.headers
            && req
                .headers()
                .get("remote-user")
                .is_some_and(|v| !v.is_empty())
        {
            audit("no account", None);
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
    if let Principal::Person(person) | Principal::Companion(person) = &who {
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
/// mail and chat run only while the box shares its storage, and the admin
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
                "admin": true,
                "spareKey": true,
                "sections": all(),
                "standalone": api.people.headers,
            })),
            Principal::Person(p) | Principal::Companion(p) => {
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
                        groups: Vec::new(),
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
        }
    })
}

// ── People ────────────────────────────────────────────────────────────────

/* The Users pane: everyone who may sign in to LosOS Login on this box, and
 * what they may do. Only while the box signs people in itself
 * (`People::headers`, `LOSOS_IDENTITY=authelia`); a box whose sign-in runs
 * elsewhere lists nobody and changes nothing here. */

/// The groups the Users pane may hand out: administrator, viewer, and one
/// per section of `backend/sections.json`.
fn grantable_groups() -> Vec<String> {
    let mut all = vec![
        crate::people::ADMINS.to_string(),
        crate::people::VIEWERS.to_string(),
    ];
    all.extend(
        crate::sections::names()
            .into_iter()
            .map(|s| format!("losos-{s}")),
    );
    all
}

/// `groups` from a request body, each one a group the pane may hand out.
fn requested_groups(doc: &serde_json::Value) -> Result<Vec<String>, String> {
    let Some(list) = doc.get("groups").and_then(|g| g.as_array()) else {
        return Err("groups must be a list".into());
    };
    let allowed = grantable_groups();
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
    Ok(out)
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
                row
            })
            .collect();
        HttpResponse::Ok().json(serde_json::json!({
            "standalone": api.people.headers,
            "you": who.person().map(|p| p.name.clone()),
            "sections": crate::sections::names(),
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
        let groups = match requested_groups(&doc) {
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
        let groups = match requested_groups(&doc) {
            Ok(g) => g,
            Err(e) => return err(StatusCode::BAD_REQUEST, &e),
        };
        let by = who.person().map(|p| p.name.as_str());
        match api.people.set_access(&name, &groups, by) {
            Ok(()) => {
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
                HttpResponse::Ok().json(serde_json::json!({ "name": name, "removed": true }))
            }
            Err(e) => error_response(e),
        }
    })
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

/// The thread that keeps the marks in step.
fn start_login_marks(api: web::Data<Api>) {
    if !api.people.headers {
        return;
    }
    std::thread::spawn(move || loop {
        reconcile_login(&api);
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
    Ok(api
        .people
        .list()?
        .into_iter()
        .filter(|p| !p.disabled)
        .map(|p| {
            let profile = profiles.get(&p.name).cloned().unwrap_or_default();
            crate::profiles::render(&p, &profile)
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

/// `POST /api/people/profile/clear`: `{"name", "kind": "picture|banner"}`.
/// An administrator takes down a picture someone else put up. The rest of
/// the profile stays theirs.
async fn post_people_profile_clear(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    const SHAPE: &str = r#"body must be JSON: {"name": "...", "kind": "picture|banner"}"#;
    guarded(&api, &req, "/api/people/profile/clear", true, || {
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
            Ok(_) => HttpResponse::Ok()
                .json(serde_json::json!({ "name": name, "kind": kind.as_str(), "cleared": true })),
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
    let verdict =
        web::block(move || api.chat.authorize(&api.people, &asked, token.as_deref())).await;
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

/// `GET /internal/chat/threads/{room}`: Matrix's thread list, which Dendrite
/// lacks. nginx sends `GET /_matrix/client/v1/rooms/{room}/threads` here,
/// after `/internal/chat/authz` let it through; the caller's own Matrix
/// token decides what it sees (`crate::chat::Chat::threads`).
async fn get_chat_threads(
    api: web::Data<Api>,
    req: HttpRequest,
    room: web::Path<String>,
) -> HttpResponse {
    let token = crate::chat::token_of(
        req.headers()
            .get(actix_web::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok()),
        &req.uri().to_string(),
    );
    let Some(token) = token else {
        return HttpResponse::Unauthorized().json(serde_json::json!({
            "errcode": "M_MISSING_TOKEN",
            "error": "Missing access token",
        }));
    };
    let query = crate::chat::ThreadsQuery::parse(&query_map(&req));
    let room = room.into_inner();
    let api = api.clone();
    match web::block(move || api.chat.threads(&token, &room, &query)).await {
        Ok(Ok(reply)) => HttpResponse::build(
            StatusCode::from_u16(reply.status).unwrap_or(StatusCode::BAD_GATEWAY),
        )
        .json(reply.body),
        Ok(Err(e)) => {
            HttpResponse::build(StatusCode::from_u16(e.status()).unwrap_or(StatusCode::BAD_GATEWAY))
                .json(serde_json::json!({ "errcode": "M_UNKNOWN", "error": e.to_string() }))
        }
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
        api.passwords.record_failure(client, now);
    }
    let result = match kind {
        "cloud" => api
            .backend
            .serialized(|l| crate::losos::cmd_device_cloud(l, password_ok, device)),
        _ => api
            .backend
            .serialized(|l| crate::losos::cmd_device_git(l, password, password_ok, device)),
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
    let who = match gate_at(api, req, route, None, mutating) {
        Ok(who) => who,
        Err(refused) => return *refused,
    };
    let worker = api.clone();
    let resp = match web::block(move || work(&mut worker.backend.clone())).await {
        Ok(Ok(v)) => HttpResponse::Ok().json(v),
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
/// USDC on Base to this box's wallet, with Binance's checkout link.
async fn post_market_onramp(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    post_market(api, req, body, "/api/market/onramp", |d| {
        Some(crate::market::Op::Onramp {
            address: field_str(d, "address")?,
            euros: field_str(d, "euros")?,
        })
    })
    .await
}

/// `POST /api/market/onramp/status` — `{"order_id"}`: where that order is.
async fn post_market_onramp_status(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    post_market(api, req, body, "/api/market/onramp/status", |d| {
        Some(crate::market::Op::OnrampStatus {
            order_id: field_str(d, "order_id")?,
        })
    })
    .await
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

/// `POST /api/builder/builds` `{"prompt": ..., "base"?: [files], "lang"?: ...}`
/// — start a build. `base` is the widget to change, as the files the editor
/// holds; a single string is read as its `index.html`.
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
        |who| match mail_person(&api, who).map(|p| {
            let by_person = !matches!(who, Principal::Companion(_));
            api.mail.message(p, &path, by_person)
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
        |who| match mail_person(&api, who).map(|p| api.mail.html(p, &path)) {
            Ok(Ok(html)) => HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .insert_header(("Content-Security-Policy", MAIL_FRAME_CSP))
                .insert_header(("X-Content-Type-Options", "nosniff"))
                .insert_header(("Referrer-Policy", "no-referrer"))
                .insert_header(("Cache-Control", "private, no-store"))
                .body(html),
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
        let done = mail_person(&api, who)
            .map(|p| crate::mail::parse_draft(&doc).and_then(|d| api.mail.send(p, &d)));
        match done {
            Ok(Ok(())) => HttpResponse::Ok().json(serde_json::json!({ "ok": true })),
            Ok(Err(r)) => mail_refused(&r),
            Err(resp) => *resp,
        }
    })
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
        Some(Principal::Companion(_)) => t::Principal::Anonymous,
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
                resp.insert_header((k.as_str(), v.as_str()));
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
/// "forgejo.site.landingPage": null}}`: each value a Nix literal, or null to
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
/// pin on first contact. Public by nature; LAN-only like all of `/api/`.
async fn get_access_host_key(api: web::Data<Api>) -> HttpResponse {
    match access_ready(&api) {
        Ok(ready) => HttpResponse::Ok().json(serde_json::json!({
            "fingerprint": ready.host.fingerprint(),
            "publicKey": ready.host.armored_public(),
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
            login,
            share: crate::cloud_share::Share::default(),
            chat: crate::chat::Chat::from_env(),
            mail: crate::mail::Mail::from_env(),
            credentials: crate::credentials::Credentials::from_env(),
            profiles: crate::profiles::Profiles::from_env(),
            image_jobs: tokio::sync::Semaphore::new(IMAGE_JOBS),
            plugins,
        });
        api.plugins
            .set_authorize(std::sync::Arc::new(authorize_caller));
        let payload_cap = api
            .plugins
            .max_body()
            .max(crate::profiles::MAX_UPLOAD_BYTES)
            .max(1024 * 1024);
        owner_in_owner_group(&api);
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
                .route("/internal/chat/authz", web::get().to(get_chat_authz))
                .route(
                    "/internal/chat/threads/{room}",
                    web::get().to(get_chat_threads),
                )
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
