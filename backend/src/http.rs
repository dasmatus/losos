//! The loopback admin HTTP API.
//!
//! Bound to `127.0.0.1` only; Nginx proxies `/api/*` here for the admin UI.
//! Every route except `/api/health` requires `Authorization: Bearer <token>`,
//! where the token is the file at `$LOSOS_ADMIN_TOKEN_FILE` — except the
//! three that exist to *get* one: the first-run claim (`/api/setup/claim`)
//! and the password sign-in (`/api/sign-in`, `crate::signin`), which answer
//! with the token when the owner has proved themselves, and the claim-state
//! read the page needs before it knows which of the two to show.
//!
//! Health is deliberately open so the dashboard can show whether the daemon is
//! reachable before anyone has pasted a token.

use crate::guard::{retry_after_secs, Audit, Throttle};
use crate::io_backend::{atomic_write_secret, IoLosos};
use crate::losos::{
    cmd_apply, cmd_apps_search, cmd_change, cmd_config, cmd_config_sync, cmd_edge,
    cmd_factory_reset, cmd_grow, cmd_options, cmd_recovery, cmd_set_password, cmd_settings,
    cmd_sign_in, cmd_state, cmd_status, cmd_storage,
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

/// Shared handler state.
struct Api {
    backend: IoLosos,
    token: String,
    throttle: Throttle,
    audit: Audit,
    /// The last claim answered, so a reply lost to a proxy timeout can be
    /// given again to the same password (`crate::receipt`).
    claims: std::sync::Mutex<crate::receipt::Receipts>,
    /// The terminal client's keys and tokens (`crate::access`).
    access: crate::access::Access,
    /// This listener's own port, which a terminal call is run against.
    port: u16,
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
            e.downcast_ref::<crate::look::Invalid>()
                .map(ToString::to_string)
        })
        .or_else(|| {
            e.downcast_ref::<crate::apps::Invalid>()
                .map(ToString::to_string)
        })
        .or_else(|| {
            e.downcast_ref::<crate::backup::Invalid>()
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
    // The first boot's one expected failure: Nextcloud is still installing
    // itself, so the first password cannot be set *yet*. 503 with the
    // reason and a Retry-After, so the wizard waits and says why, instead
    // of the opaque 500 that once sent owners to re-image a working box.
    if let Some(crate::setup::NotReady(why)) = e.downcast_ref() {
        tracing::info!(reason = why.as_str(), "claim refused: not ready yet");
        return HttpResponse::build(StatusCode::SERVICE_UNAVAILABLE)
            .insert_header(("Retry-After", "10"))
            .json(serde_json::json!({ "error": why, "ready": false, "waitingFor": why }));
    }
    // A password sign-in that LosOS cloud turned down: 401, which the
    // dialog shows as "try again" and the caller counts as a failure for
    // the per-address throttle, exactly like a wrong token.
    if let Some(why) = e.downcast_ref::<crate::signin::WrongPassword>() {
        tracing::info!("sign-in refused: wrong password");
        return err(StatusCode::UNAUTHORIZED, &why.to_string());
    }
    // LosOS cloud's own brute-force protection shut the door; the page
    // says "wait" rather than "wrong".
    if let Some(why) = e.downcast_ref::<crate::signin::Throttled>() {
        tracing::warn!("sign-in refused: LosOS cloud is throttling this address");
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
    // Trading asked of a box whose edges are all a company's own: the
    // sentence, as a 409, with a flag the Market pane can key on.
    if let Some(why) = e.downcast_ref::<crate::edge::OfficialEdgeRequired>() {
        tracing::warn!("refused: no official edge reachable, trading is off");
        return HttpResponse::build(StatusCode::CONFLICT).json(serde_json::json!({
            "error": why.to_string(),
            "officialEdgeRequired": true,
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
    let remote = remote_addr(req);
    let audit = |outcome: &str| {
        if mutating {
            api.audit.record(route, outcome, &remote);
        }
    };
    if let Err(wait) = api.throttle.check(&remote, Instant::now()) {
        audit("throttled");
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
    if !authorized(api, req) {
        api.throttle.record_failure(&remote, Instant::now());
        audit("unauthorized");
        return err(StatusCode::UNAUTHORIZED, "unauthorized");
    }
    api.throttle.record_success(&remote);
    let resp = f();
    let status = resp.status();
    audit(if status.is_success() {
        "ok"
    } else if status.is_client_error() {
        "rejected"
    } else {
        "error"
    });
    resp
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

/// Replace the Nextcloud admin password.
///
/// The body is parsed by hand rather than through actix's JSON extractor, for
/// the same reason `post_apply` takes raw bytes: the extractor's rejection
/// handler renders the offending payload into its error, and this payload is a
/// password.
///
/// The 400s below name the field that was wrong and never echo what was sent.
/// The command's own errors go through [`run`], which keeps the context chain
/// out of the response and puts it in the journal — where, by construction, it
/// cannot carry the password either: it is not a field of any `OccAction` and
/// never reaches an argv.
/// `GET /api/setup/claim` — has this box got an owner yet? No token required.
///
/// The page has to ask this before it can decide whether to show the wizard or
/// the sign-in dialog, and on an unclaimed box there is no token to ask with.
async fn get_claim_state(api: web::Data<Api>) -> HttpResponse {
    run(&api, crate::losos::cmd_claim_state)
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
    const SHAPE: &str = r#"body must be JSON: {"user": "..." (optional), "password": "..."}"#;
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
    let token = api.token.clone();
    let claims = api.clone();
    run(&api, move |l| {
        // Held across the occ run on purpose: two claims racing each other
        // would otherwise both see an unclaimed box.
        let mut receipts = claims
            .claims
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        crate::losos::cmd_claim(l, &user, password, &token, &mut receipts)
    })
}

/// `POST /api/sign-in` — unlock the admin pages with the owner's password.
///
/// The second of the two routes that hand out the admin token
/// ([`post_claim`] is the first), and the one every later visit uses: the
/// password is checked by Nextcloud over loopback ([`crate::losos::cmd_sign_in`],
/// `crate::signin`), and a correct one is answered with the same token the
/// claim released. No Bearer, by nature — it is how a tab gets one — so it
/// gets the claim's three same-box checks ([`claim_fault`]) against a page in
/// a LAN browser, and the daemon's own per-address throttle counts a wrong
/// password exactly as it counts a wrong token: a few free tries, then
/// `429` with `Retry-After` for a growing window. Nextcloud throttles the
/// forwarded client address on top of that.
async fn post_sign_in(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    const SHAPE: &str = r#"body must be JSON: {"password": "..."}"#;
    let header = |name: &str| req.headers().get(name).and_then(|v| v.to_str().ok());
    let hostname = std::fs::read_to_string("/proc/sys/kernel/hostname").unwrap_or_default();
    if let Some(why) = claim_fault(
        header("content-type"),
        header("host"),
        header("origin"),
        hostname.trim(),
    ) {
        tracing::warn!(reason = why, "refused a sign-in request");
        return err(StatusCode::FORBIDDEN, why);
    }
    let remote = remote_addr(&req);
    if let Err(wait) = api.throttle.check(&remote, Instant::now()) {
        api.audit.record("/api/sign-in", "throttled", &remote);
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
    let Ok(doc) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return err(StatusCode::BAD_REQUEST, SHAPE);
    };
    let Some(password) = doc.get("password").and_then(|p| p.as_str()) else {
        return err(StatusCode::BAD_REQUEST, SHAPE);
    };
    if let Err(e) = crate::signin::validate_candidate(password) {
        return err(StatusCode::BAD_REQUEST, &e);
    }
    let token = api.token.clone();
    let client = remote.clone();
    let resp = run(&api, move |l| cmd_sign_in(l, password, &token, &client));
    let outcome = if resp.status().is_success() {
        api.throttle.record_success(&remote);
        "ok"
    } else if resp.status() == StatusCode::UNAUTHORIZED {
        api.throttle.record_failure(&remote, Instant::now());
        "unauthorized"
    } else if resp.status().is_client_error() {
        "rejected"
    } else {
        "error"
    };
    api.audit.record("/api/sign-in", outcome, &remote);
    resp
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
    guarded(&api, &req, "/api/set-password", true, || {
        post_set_password_inner(&api, &body)
    })
}

fn post_set_password_inner(api: &Api, body: &[u8]) -> HttpResponse {
    const SHAPE: &str = r#"body must be JSON: {"user": "...", "password": "..."}"#;
    let Ok(doc) = serde_json::from_slice::<serde_json::Value>(body) else {
        return err(StatusCode::BAD_REQUEST, SHAPE);
    };
    // `user` is optional: the appliance installs exactly one Nextcloud admin,
    // and the wizard should not have to know its name to reset it.
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
    run(api, |b| cmd_set_password(b, &user, &password))
}

/// `POST /api/devices/cloud` and `POST /api/devices/git`: a credential of
/// one computer's own for LosOS cloud or LosOS Git (`crate::devices`).
///
/// Bearer-gated like every other route, and on top of that the owner's
/// password, which Nextcloud and Forgejo both insist on before they make a
/// credential. A wrong password is a 403 with `wrongPassword`, not the 401
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
    let result = match kind {
        "cloud" => api
            .backend
            .serialized(|l| crate::losos::cmd_device_cloud(l, password, device, client)),
        "git" => api
            .backend
            .serialized(|l| crate::losos::cmd_device_git(l, password, device, client)),
        _ => return err(StatusCode::NOT_FOUND, "no such device credential"),
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

/// `GET /api/cloud/text?q=` — files in LosOS cloud whose text matches.
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
    guarded(&api, &req, "/api/market", false, || {
        run(&api, crate::losos::cmd_market)
    })
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
    guarded(&api, &req, route, true, || {
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        match build(&doc) {
            Some(op) => run(&api, |b| crate::losos::cmd_market_op(b, &op)),
            None => err(
                StatusCode::BAD_REQUEST,
                "the request body is missing a required field",
            ),
        }
    })
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

// ── The widget builder ────────────────────────────────────────────────────
// Relayed to the edge like the market (`crate::market`): the edge holds the
// Anthropic key and the balance, this box only its proxy token.

/// `GET /api/builder` — balance, packs, price and recent builds, or
/// `{"available": false}` when no official edge offers the builder.
async fn get_builder(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/builder", false, || {
        run(&api, crate::losos::cmd_builder)
    })
}

/// `POST /api/builder/credits` `{"amount": ...}` — a Stripe Checkout for one
/// credit pack.
async fn post_builder_credit(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    post_market(api, req, body, "/api/builder/credits", |d| {
        Some(crate::market::Op::BuilderCredit {
            amount: field_u64(d, "amount")?,
        })
    })
    .await
}

/// `POST /api/builder/builds` `{"prompt": ..., "base"?: [files], "lang"?: ...}`
/// — start a build. `base` is the widget to change, as the files the editor
/// holds; a single string is read as its `index.html`.
async fn post_builder_start(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    post_market(api, req, body, "/api/builder/builds", |d| {
        Some(crate::market::Op::BuilderStart {
            prompt: field_str(d, "prompt")?,
            base: match d.get("base") {
                None | Some(serde_json::Value::Null) => None,
                Some(serde_json::Value::String(s)) if s.trim().is_empty() => None,
                Some(serde_json::Value::String(s)) => Some(vec![crate::look::WidgetFile::entry(s)]),
                Some(files) => Some(crate::look::parse_files(files)?),
            },
            lang: field_str(d, "lang"),
        })
    })
    .await
}

/// `GET /api/builder/builds/{id}` — one build, with its source once done.
async fn get_builder_build(
    api: web::Data<Api>,
    req: HttpRequest,
    path: web::Path<String>,
) -> HttpResponse {
    let build_id = path.into_inner();
    guarded(&api, &req, "/api/builder/builds", false, || {
        let op = crate::market::Op::BuilderBuild { build_id };
        run(&api, |b| crate::losos::cmd_market_op(b, &op))
    })
}

// ── Virtual machines ──────────────────────────────────────────────────────
// The market's third kind (`backend-registrar/src/vms.rs`), on its own page
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

// ── The owner's look ──────────────────────────────────────────────────────
// A background picture and the widgets written by hand (`crate::look`).
// HTTP-only, like the market relay: a CLI for a wallpaper would be surface
// with no caller. Every change is Bearer-authed and audited; the one read
// that is not is the picture itself, for the reason given in look.rs.

/// `GET /api/look` — the document the page paints from.
async fn get_look(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/look", false, || {
        run(&api, crate::look::cmd_look)
    })
}

/// `POST /api/look` — the background by choice (none, or a shipped picture
/// by name) and the veil over it. Each field optional, so one knob at a time.
async fn post_look(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    guarded(&api, &req, "/api/look", true, || {
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        match crate::look::parse_patch(&doc) {
            Some(patch) => run(&api, |b| crate::look::cmd_patch_look(b, &patch)),
            None => err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"background": {"kind": "none|shipped", "name": "..."}, "veil": 20-90}"#,
            ),
        }
    })
}

/// `PUT /api/look/background` — the picture, raw. The type is read off the
/// bytes, never off the request; the size cap is `PayloadConfig` below plus
/// the command's own check, so an oversized body is refused before it is
/// read in full.
async fn put_background(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    guarded(&api, &req, "/api/look/background", true, || {
        run(&api, |b| crate::look::cmd_upload_background(b, &body))
    })
}

/// `DELETE /api/look/background` — back to the plain ground colour.
async fn delete_background(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/look/background", true, || {
        let patch = crate::look::LookPatch {
            background: Some(crate::look::BackgroundChoice::None),
            veil: None,
        };
        run(&api, |b| crate::look::cmd_patch_look(b, &patch))
    })
}

/// `GET /api/look/background?v=N` — the uploaded picture, **without a
/// token**. A CSS `background-image` cannot carry one (look.rs says why
/// this is the accepted shape). Cached hard, because a replacement is a new
/// `?v=` and therefore a new URL. 404 when the look has no upload.
async fn get_background(api: web::Data<Api>) -> HttpResponse {
    match api.backend.serialized(crate::look::cmd_read_background) {
        Ok(Some((content_type, bytes))) => HttpResponse::Ok()
            .content_type(content_type)
            .insert_header(("Cache-Control", "private, max-age=31536000, immutable"))
            .insert_header(("X-Content-Type-Options", "nosniff"))
            .body(bytes),
        Ok(None) => err(StatusCode::NOT_FOUND, "no picture"),
        Err(e) => {
            tracing::error!(error = ?e, "reading the background picture failed");
            err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "command failed; see the lososd journal",
            )
        }
    }
}

/// `POST /api/look/widgets` — add a widget written by hand, or replace the
/// one whose id the body carries.
async fn post_widget(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    guarded(&api, &req, "/api/look/widgets", true, || {
        let doc = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
        match crate::look::parse_draft(&doc) {
            Some(draft) => run(&api, |b| crate::look::cmd_put_widget(b, &draft)),
            None => err(
                StatusCode::BAD_REQUEST,
                r#"body must be JSON: {"id": "..." (optional), "name": "...", "span": "half|full", "source": "..."}"#,
            ),
        }
    })
}

/// `DELETE /api/look/widgets/{id}`.
async fn delete_widget(
    api: web::Data<Api>,
    req: HttpRequest,
    id: web::Path<String>,
) -> HttpResponse {
    guarded(&api, &req, "/api/look/widgets", true, || {
        run(&api, |b| crate::look::cmd_delete_widget(b, &id))
    })
}

// ── Every option, and the configuration repository ───────────────────────

/// `GET /api/options` — every `losos.*` option the box declares, with its
/// editor kind and what `overrides.nix` sets (`crate::options`). The
/// Advanced pane is drawn from this. A box without the document answers
/// `available: false`, deliberately not 404, which the SPA latches.
async fn get_options(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/options", false, || run(&api, cmd_options))
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
async fn get_access(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    guarded(&api, &req, "/api/access", false, || {
        match access_ready(&api) {
            Ok(ready) => match crate::access::overview(ready) {
                Ok(v) => HttpResponse::Ok().json(v),
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
    guarded(&api, &req, "/api/access/tokens", true, || {
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
    guarded(&api, &req, "/api/access/tokens/revoke", true, || {
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
    let token_id = match crate::access::authorize(ready, &opened, now) {
        Ok(id) => id,
        Err(r) => return refuse(r),
    };
    api.throttle.record_success(remote);
    let r = &opened.request;
    let route_only = r.path.split('?').next().unwrap_or(&r.path);
    let audit_route = format!("{ROUTE} {token_id} {} {route_only}", r.method);
    let (status, answer) = match crate::access::forward(
        api.port, &api.token, remote, &r.method, &r.path, &r.body,
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

async fn not_found() -> HttpResponse {
    err(StatusCode::NOT_FOUND, "not found")
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

/// Serve the API. Blocking: runs its own actix `System` on the calling thread.
///
/// Returns only on failure or on the server stopping, and the caller is
/// expected to treat either as fatal: an admin API that failed to bind while
/// the daemon stays up is invisible to systemd.
pub fn serve(backend: IoLosos) -> anyhow::Result<()> {
    let port = admin_port()?;
    let token_file =
        std::env::var("LOSOS_ADMIN_TOKEN_FILE").unwrap_or_else(|_| DEFAULT_TOKEN_FILE.to_string());
    let token = ensure_token(Path::new(&token_file))?;

    actix_web::rt::System::new().block_on(async move {
        let api = web::Data::new(Api {
            backend,
            token,
            throttle: Throttle::default(),
            audit: Audit,
            claims: std::sync::Mutex::new(crate::receipt::Receipts::default()),
            access: crate::access::Access::from_env(),
            port,
        });
        let server = HttpServer::new(move || {
            App::new()
                .app_data(api.clone())
                // actix reads at most 256 KiB into `web::Bytes` by default,
                // which is plenty for every JSON body here and a quarter of
                // a wallpaper. The cap is the picture limit plus room for a
                // widget's JSON; look.rs refuses anything over its own
                // limits with a sentence.
                .app_data(web::PayloadConfig::new(
                    crate::look::MAX_IMAGE_BYTES + 256 * 1024,
                ))
                .route("/api/health", web::get().to(health))
                .route("/api/state", web::get().to(get_state))
                .route("/api/settings", web::get().to(get_settings))
                .route("/api/status", web::get().to(get_status))
                .route("/api/edge", web::get().to(get_edge))
                .route("/api/change", web::post().to(post_change))
                .route("/api/apply", web::post().to(post_apply))
                .route("/api/factory-reset", web::post().to(post_factory_reset))
                .route("/api/backup", web::get().to(get_backup))
                .route("/api/backup/target", web::post().to(post_backup_target))
                .route("/api/backup/target", web::delete().to(delete_backup_target))
                .route("/api/backup/run", web::post().to(post_backup_run))
                .route("/api/backup/restore", web::post().to(post_backup_restore))
                .route("/api/erase", web::post().to(post_erase))
                .route("/api/erase/cancel", web::post().to(post_erase_cancel))
                .route("/api/storage", web::get().to(get_storage))
                .route("/api/grow", web::post().to(post_grow))
                .route("/api/set-password", web::post().to(post_set_password))
                .route("/api/recovery", web::get().to(get_recovery))
                .route("/api/devices/{kind}", web::post().to(post_device))
                .route("/api/apps/search", web::get().to(get_apps_search))
                .route("/api/cloud/text", web::get().to(get_cloud_text))
                .route("/api/git/text", web::get().to(get_git_text))
                .route("/api/apps", web::get().to(get_apps))
                .route("/api/apps/chart", web::get().to(get_app_chart))
                .route("/api/apps/install", web::post().to(post_app_install))
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
                .route("/api/builder", web::get().to(get_builder))
                .route("/api/builder/credits", web::post().to(post_builder_credit))
                .route("/api/builder/builds", web::post().to(post_builder_start))
                .route("/api/builder/builds/{id}", web::get().to(get_builder_build))
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
                .route("/api/options", web::get().to(get_options))
                .route("/api/config", web::get().to(get_config))
                .route("/api/config/sync", web::post().to(post_config_sync))
                .route("/api/look", web::get().to(get_look))
                .route("/api/look", web::post().to(post_look))
                .route("/api/look/background", web::put().to(put_background))
                .route("/api/look/background", web::delete().to(delete_background))
                // The one read without a token: the wallpaper, see look.rs.
                .route("/api/look/background", web::get().to(get_background))
                .route("/api/look/widgets", web::post().to(post_widget))
                .route("/api/look/widgets/{id}", web::delete().to(delete_widget))
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
                // The third: the password sign-in, which answers with the
                // token on a claimed box and is guarded like the claim.
                .route("/api/sign-in", web::post().to(post_sign_in))
                .default_service(web::route().to(not_found))
        })
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
