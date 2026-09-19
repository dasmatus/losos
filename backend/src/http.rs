//! The loopback admin HTTP API.
//!
//! Bound to `127.0.0.1` only; Nginx proxies `/api/*` here for the admin UI.
//! Every route except `/api/health` requires `Authorization: Bearer <token>`,
//! where the token is the file at `$LOSOS_ADMIN_TOKEN_FILE`.
//!
//! Health is deliberately open so the dashboard can show whether the daemon is
//! reachable before anyone has pasted a token.

use crate::io_backend::{atomic_write_secret, IoLosos};
use crate::losos::{
    cmd_apply, cmd_apps_search, cmd_change, cmd_factory_reset, cmd_grow, cmd_recovery,
    cmd_set_password, cmd_settings, cmd_state, cmd_status,
};
use crate::model::Mode;
use crate::overrides::validate_apply;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use anyhow::Context;
use std::path::Path;

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
        Err(e) => {
            tracing::error!(error = ?e, "admin API command failed");
            err(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "command failed; see the lososd journal",
            )
        }
    }
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

/// Reject unauthenticated requests, with the body the SPA looks for.
fn gate(api: &Api, req: &HttpRequest) -> Option<HttpResponse> {
    if authorized(api, req) {
        None
    } else {
        Some(err(
            actix_web::http::StatusCode::UNAUTHORIZED,
            "unauthorized",
        ))
    }
}

async fn health() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({ "ok": true }))
}

async fn get_state(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    gate(&api, &req).unwrap_or_else(|| run(&api, cmd_state))
}

async fn get_settings(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    gate(&api, &req).unwrap_or_else(|| run(&api, cmd_settings))
}

async fn get_status(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    gate(&api, &req).unwrap_or_else(|| run(&api, cmd_status))
}

async fn post_change(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    if let Some(r) = gate(&api, &req) {
        return r;
    }
    let mode = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("mode").and_then(|m| m.as_str()).map(str::to_string));
    let Some(mode) = mode else {
        return err(
            actix_web::http::StatusCode::BAD_REQUEST,
            r#"body must be JSON: {"mode": "local|mesh"}"#,
        );
    };
    let Some(mode) = Mode::parse(&mode) else {
        return err(
            actix_web::http::StatusCode::BAD_REQUEST,
            "mode must be 'local' or 'mesh'",
        );
    };
    run(&api, |b| cmd_change(b, mode))
}

async fn post_apply(api: web::Data<Api>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    if let Some(r) = gate(&api, &req) {
        return r;
    }
    let Ok(text) = String::from_utf8(body.to_vec()) else {
        return err(
            actix_web::http::StatusCode::BAD_REQUEST,
            "body must be UTF-8 Nix code",
        );
    };
    match validate_apply(&text) {
        Err(e) => err(actix_web::http::StatusCode::BAD_REQUEST, e),
        Ok(code) => {
            let code = code.to_string();
            run(&api, |b| cmd_apply(b, &code))
        }
    }
}

async fn post_factory_reset(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    gate(&api, &req).unwrap_or_else(|| run(&api, cmd_factory_reset))
}

/// Extend `/persist` into the volume group's free extents.
///
/// Synchronous, unlike every other POST here: it is three short-lived commands
/// rather than a supervised rebuild, so it returns the measured before/after
/// sizes instead of a job id to poll.
async fn post_grow(api: web::Data<Api>, req: HttpRequest) -> HttpResponse {
    gate(&api, &req).unwrap_or_else(|| run(&api, cmd_grow))
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
async fn post_claim(api: web::Data<Api>, body: web::Bytes) -> HttpResponse {
    const SHAPE: &str = r#"body must be JSON: {"user": "..." (optional), "password": "..."}"#;
    let Ok(doc) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return err(actix_web::http::StatusCode::BAD_REQUEST, SHAPE);
    };
    let user = match doc.get("user") {
        None | Some(serde_json::Value::Null) => crate::setup::DEFAULT_ADMIN_USER.to_string(),
        Some(serde_json::Value::String(u)) => u.clone(),
        Some(_) => return err(actix_web::http::StatusCode::BAD_REQUEST, SHAPE),
    };
    let Some(password) = doc.get("password").and_then(|p| p.as_str()) else {
        return err(actix_web::http::StatusCode::BAD_REQUEST, SHAPE);
    };
    let token = api.token.clone();
    run(&api, move |l| {
        crate::losos::cmd_claim(l, &user, password, &token)
    })
}

async fn post_set_password(
    api: web::Data<Api>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    if let Some(r) = gate(&api, &req) {
        return r;
    }
    const SHAPE: &str = r#"body must be JSON: {"user": "...", "password": "..."}"#;
    let Ok(doc) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return err(actix_web::http::StatusCode::BAD_REQUEST, SHAPE);
    };
    // `user` is optional: the appliance installs exactly one Nextcloud admin,
    // and the wizard should not have to know its name to reset it.
    let user = match doc.get("user") {
        None | Some(serde_json::Value::Null) => crate::setup::DEFAULT_ADMIN_USER.to_string(),
        Some(serde_json::Value::String(u)) => u.clone(),
        Some(_) => return err(actix_web::http::StatusCode::BAD_REQUEST, SHAPE),
    };
    let Some(password) = doc.get("password").and_then(|p| p.as_str()) else {
        return err(actix_web::http::StatusCode::BAD_REQUEST, SHAPE);
    };
    // Validated a second time inside the command; doing it here as well is what
    // turns "too short" into a 400 the wizard can show next to the field
    // instead of a 500 that says to read the journal.
    if let Err(e) = crate::setup::validate_password(password) {
        return err(actix_web::http::StatusCode::BAD_REQUEST, &e);
    }
    if let Err(e) = crate::setup::validate_user(&user) {
        return err(actix_web::http::StatusCode::BAD_REQUEST, &e);
    }
    let password = password.to_string();
    run(&api, |b| cmd_set_password(b, &user, &password))
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
    gate(&api, &req).unwrap_or_else(|| run(&api, cmd_recovery))
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
    if let Some(r) = gate(&api, &req) {
        return r;
    }
    let query =
        web::Query::<std::collections::HashMap<String, String>>::from_query(req.query_string())
            .ok()
            .and_then(|q| q.get("q").cloned())
            .unwrap_or_default();
    // Validated here as well as in the command, for the same reason
    // `set-password` is: it turns "too short" into a 400 beside the field
    // rather than a 500 that says to read the journal.
    if let Err(e) = crate::catalogue::validate_query(&query) {
        return err(actix_web::http::StatusCode::BAD_REQUEST, &e);
    }
    run(&api, |b| cmd_apps_search(b, &query))
}

async fn not_found() -> HttpResponse {
    err(actix_web::http::StatusCode::NOT_FOUND, "not found")
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
    let mut buf = [0u8; TOKEN_BYTES];
    {
        use std::io::Read;
        std::fs::File::open("/dev/urandom")
            .and_then(|mut urandom| urandom.read_exact(&mut buf))
            .context("reading /dev/urandom")?;
    }
    let token: String = buf.iter().map(|b| format!("{b:02x}")).collect();
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
        let api = web::Data::new(Api { backend, token });
        let server = HttpServer::new(move || {
            App::new()
                .app_data(api.clone())
                .route("/api/health", web::get().to(health))
                .route("/api/state", web::get().to(get_state))
                .route("/api/settings", web::get().to(get_settings))
                .route("/api/status", web::get().to(get_status))
                .route("/api/change", web::post().to(post_change))
                .route("/api/apply", web::post().to(post_apply))
                .route("/api/factory-reset", web::post().to(post_factory_reset))
                .route("/api/grow", web::post().to(post_grow))
                .route("/api/set-password", web::post().to(post_set_password))
                .route("/api/recovery", web::get().to(get_recovery))
                .route("/api/apps/search", web::get().to(get_apps_search))
                // The two unauthenticated routes, and the only ones besides
                // /api/health. Both are first-run only: the state read is a
                // single bit, and the claim refuses once the box has an owner.
                .route("/api/setup/claim", web::get().to(get_claim_state))
                .route("/api/setup/claim", web::post().to(post_claim))
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
