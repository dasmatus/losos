//! The loopback admin HTTP API.
//!
//! Bound to `127.0.0.1` only; Nginx proxies `/api/*` here for the admin UI.
//! Every route except `/api/health` requires `Authorization: Bearer <token>`,
//! where the token is the file at `$LOSOS_ADMIN_TOKEN_FILE`.
//!
//! Health is deliberately open so the dashboard can show whether the daemon is
//! reachable before anyone has pasted a token.

use crate::io_backend::{IoLosos, Paths};
use crate::losos::{cmd_apply, cmd_change, cmd_factory_reset, cmd_settings, cmd_state, cmd_status};
use crate::model::Mode;
use crate::overrides::validate_apply;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use std::sync::{Arc, Mutex};

/// Default loopback port; overridden by `$LOSOS_ADMIN_PORT`.
const DEFAULT_PORT: u16 = 8082;
/// Default token location; overridden by `$LOSOS_ADMIN_TOKEN_FILE`.
const DEFAULT_TOKEN_FILE: &str = "/var/secrets/losos-admin-token";
/// Bytes of entropy behind the admin token. Hex-encoded, this is the 64
/// characters the VM test asserts on.
const TOKEN_BYTES: usize = 32;

/// Shared handler state.
struct Api {
    backend: Arc<Mutex<IoLosos>>,
    token: String,
}

/// `{"error": "..."}` at the given status — the shape the SPA expects.
fn err(status: actix_web::http::StatusCode, msg: &str) -> HttpResponse {
    HttpResponse::build(status).json(serde_json::json!({ "error": msg }))
}

/// Run a command, or report it as a 500.
fn run(
    api: &Api,
    f: impl FnOnce(&mut IoLosos) -> anyhow::Result<serde_json::Value>,
) -> HttpResponse {
    let Ok(mut guard) = api.backend.lock() else {
        return err(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            "control state poisoned",
        );
    };
    match f(&mut guard) {
        Ok(v) => HttpResponse::Ok().json(v),
        Err(e) => err(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            &format!("{e:#}"),
        ),
    }
}

/// Whether the request carries the admin token.
fn authorized(api: &Api, req: &HttpRequest) -> bool {
    req.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == format!("Bearer {}", api.token))
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

async fn not_found() -> HttpResponse {
    err(actix_web::http::StatusCode::NOT_FOUND, "not found")
}

/// Read the admin token, creating it on first start.
///
/// An existing file is left alone, so the token survives restarts; rotating it
/// means deleting the file and restarting `lososd`. Mode 0600 matters — the
/// token is equivalent to root on this box.
pub fn ensure_token(path: &std::path::Path) -> anyhow::Result<String> {
    if let Ok(existing) = std::fs::read_to_string(path) {
        return Ok(existing.trim().to_string());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut buf = [0u8; TOKEN_BYTES];
    {
        use std::io::Read;
        let mut urandom = std::fs::File::open("/dev/urandom")?;
        urandom.read_exact(&mut buf)?;
    }
    let token: String = buf.iter().map(|b| format!("{b:02x}")).collect();
    std::fs::write(path, &token)?;
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    tracing::info!(path = %path.display(), "minted admin token");
    Ok(token)
}

/// Serve the API. Blocking: runs its own actix `System` on the calling thread.
pub fn serve(paths: Paths) -> anyhow::Result<()> {
    let port: u16 = std::env::var("LOSOS_ADMIN_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let token_file =
        std::env::var("LOSOS_ADMIN_TOKEN_FILE").unwrap_or_else(|_| DEFAULT_TOKEN_FILE.to_string());
    let token = ensure_token(std::path::Path::new(&token_file))?;

    let backend = Arc::new(Mutex::new(IoLosos { paths }));

    actix_web::rt::System::new().block_on(async move {
        let api = web::Data::new(Api { backend, token });
        tracing::info!(port, "admin API listening on loopback");
        HttpServer::new(move || {
            App::new()
                .app_data(api.clone())
                .route("/api/health", web::get().to(health))
                .route("/api/state", web::get().to(get_state))
                .route("/api/settings", web::get().to(get_settings))
                .route("/api/status", web::get().to(get_status))
                .route("/api/change", web::post().to(post_change))
                .route("/api/apply", web::post().to(post_apply))
                .route("/api/factory-reset", web::post().to(post_factory_reset))
                .default_service(web::route().to(not_found))
        })
        .bind(("127.0.0.1", port))?
        .run()
        .await
    })?;
    Ok(())
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
