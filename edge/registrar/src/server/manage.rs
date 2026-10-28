//! The page a company's admins look after their boxes from.
//!
//! A company's own edge (a gateway on the office LAN) lists the boxes that
//! enrolled with it, and for each box whose owner switched it on, shows and
//! changes that box's LosOS Cloud, LosOS Git and app settings. The box
//! decides, not the edge: the owner's switch lives on the box
//! (`backend/src/edge.rs`, "management"), the box says on its heartbeats
//! that it is on (`crate::announce`, `managed`), and every call this page
//! makes is relayed into the box through the box's own tunnel, where lososd
//! checks the switch, the box's proxy token and the sections the edge may
//! use (`backend/sections.json`, `edge`) again. An official edge never runs
//! this: the box refuses to be managed by one.
//!
//! Routes, outside the API's 16 KiB body cap and 5 s budget because an app
//! install or a store search on the box takes longer:
//!
//! ```text
//! GET  /manage/                         the page (--manage-ui), no sign-in,
//!                                       with the admin UI's /assets/ and /theme-boot.js
//! GET  /manage/api/boxes                Bearer <key>: every box, and whether it is managed
//! GET|POST /manage/api/boxes/{id}/api/… Bearer <key>: relayed to the box's lososd
//! …      /manage/api/gateway…           Bearer <key>: the gateway's own settings
//!                                       (`gateway`)
//! ```
//!
//! The key is 64 hex characters in `--manage-key-file`, made on the first
//! start if missing; the gateway's screen shows it, since the gateway has no
//! shell to read it from (`modules/edge-gateway.nix`).

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, Path as UrlPath, RawQuery, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{Json, Router};
use miette::{miette, Context, IntoDiagnostic, Result};
use tokio::sync::Semaphore;

use super::{ct_eq, lookup_tenant, AppState};

mod gateway;
use crate::action::Action;
use crate::opts::ManageOpts;

/// The name a box's nginx serves the edge on (`modules/proxy.nix`,
/// `manageHost`). `.invalid` never resolves, so nothing but this relay asks
/// for it.
pub(crate) const BOX_HOST: &str = "losos-manage.invalid";

/// A settings change or an app install is a small JSON document; a store
/// search answer is the biggest thing that comes back, and that is the box's
/// to bound.
const MAX_RELAY_BODY: usize = 256 * 1024;

/// How long one relayed call may take: an app install waits for Helm to
/// fetch a chart, a store search for the store.
const RELAY_TIMEOUT: Duration = Duration::from_secs(120);

/// Relayed calls in flight at once, apart from the API's own cap.
const MAX_RELAYS: usize = 16;

/// The key, the page's files and which boxes said they are managed.
pub(crate) struct Manage {
    key: String,
    ui: Option<PathBuf>,
    /// Box id to when it last said on a heartbeat that its owner lets this
    /// edge manage it. Live state only, like the relay passes: every box
    /// repeats it on each heartbeat, and a box that stops saying it drops
    /// off within one heartbeat TTL.
    managed: std::sync::Mutex<HashMap<String, Instant>>,
    relays: Semaphore,
    http: reqwest::Client,
}

impl Manage {
    /// Read the key, or make it when the file is missing.
    ///
    /// # Errors
    /// The key file cannot be read or written, or holds something that is
    /// not a key.
    pub(crate) fn load(opts: &ManageOpts) -> Result<Self> {
        let key = load_or_make_key(Path::new(&opts.key_file))?;
        let http = reqwest::Client::builder()
            .timeout(RELAY_TIMEOUT)
            // The box is on this host's loopback, through its tunnel; a proxy
            // in the environment must never see the box's token.
            .no_proxy()
            .user_agent(concat!("losos-registrar/", env!("CARGO_PKG_VERSION")))
            .build()
            .into_diagnostic()
            .wrap_err("build the relay client")?;
        Ok(Manage {
            key,
            ui: opts.ui_dir.as_ref().map(PathBuf::from),
            managed: std::sync::Mutex::new(HashMap::new()),
            relays: Semaphore::new(MAX_RELAYS),
            http,
        })
    }

    /// What a heartbeat said about management.
    pub(crate) fn record(&self, id: &str, managed: bool) {
        let mut map = self
            .managed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if managed {
            map.insert(id.to_string(), Instant::now());
        } else {
            map.remove(id);
        }
    }

    fn is_managed(&self, id: &str, ttl: Duration) -> bool {
        self.managed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(id)
            .is_some_and(|at| at.elapsed() <= ttl)
    }

    fn signed_in(&self, headers: &HeaderMap) -> bool {
        headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split_once(' '))
            .is_some_and(|(scheme, key)| {
                scheme.eq_ignore_ascii_case("bearer") && ct_eq(key.trim(), &self.key)
            })
    }
}

fn load_or_make_key(path: &Path) -> Result<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let key = text.trim().to_string();
            if key.len() == 64 && key.bytes().all(|b| b.is_ascii_hexdigit()) {
                Ok(key)
            } else {
                Err(miette!(
                    "{} does not hold a key (64 hex characters)",
                    path.display()
                ))
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            use ring::rand::SecureRandom;
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut bytes = [0u8; 32];
            ring::rand::SystemRandom::new()
                .fill(&mut bytes)
                .map_err(|_| miette!("the system random source failed"))?;
            let key: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)
                    .into_diagnostic()
                    .wrap_err_with(|| format!("create {}", dir.display()))?;
            }
            let tmp = path.with_extension("new");
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp)
                .into_diagnostic()
                .wrap_err_with(|| format!("write {}", tmp.display()))?;
            writeln!(f, "{key}").into_diagnostic()?;
            f.sync_all().into_diagnostic()?;
            std::fs::rename(&tmp, path)
                .into_diagnostic()
                .wrap_err_with(|| format!("write {}", path.display()))?;
            tracing::info!(target: Action::Serve.target(), "made the manage key at {}", path.display());
            Ok(key)
        }
        Err(e) => Err(e)
            .into_diagnostic()
            .wrap_err_with(|| format!("read {}", path.display())),
    }
}

fn plain(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

fn off() -> Response {
    plain(StatusCode::NOT_FOUND, "this edge does not manage boxes")
}

fn unauthorized() -> Response {
    plain(
        StatusCode::UNAUTHORIZED,
        "sign in with this edge's manage key",
    )
}

/// The routes, with their own body cap and concurrency, merged outside the
/// API's guard (see the module docs).
pub(super) fn router(state: AppState) -> Router {
    Router::new()
        .route("/manage", get(|| async { Redirect::permanent("/manage/") }))
        .route("/manage/api/boxes", get(boxes))
        .route(
            "/manage/api/boxes/{id}/api/{*rest}",
            get(relay).post(relay).put(relay).delete(relay),
        )
        .route("/manage/", get(page_index))
        .route("/manage/{*file}", get(page_file))
        // The page's scripts, styles and pictures are the admin UI's shared
        // ones, at the root as the box serves them.
        .route("/assets/{*file}", get(asset))
        .route("/theme-boot.js", get(theme_boot))
        .merge(gateway::routes())
        .layer(DefaultBodyLimit::max(MAX_RELAY_BODY))
        .with_state(state)
}

/// Every box this edge routes, newest first by contact, and whether its
/// owner lets the edge manage it.
async fn boxes(State(st): State<AppState>, headers: HeaderMap) -> Response {
    let Some(manage) = st.manage.clone() else {
        return off();
    };
    if !manage.signed_in(&headers) {
        return unauthorized();
    }
    let ttl = st.opts.heartbeat_ttl;
    let snapshot = st.reg.export().await;
    let mut rows: Vec<_> = snapshot
        .tenants
        .iter()
        .filter(|(_, t)| t.via.is_none())
        .map(|(id, t)| {
            serde_json::json!({
                "id": id,
                "hostname": t.hostname,
                "seenAgoSecs": t.seen_ago_secs,
                "managed": manage.is_managed(id, ttl),
            })
        })
        .collect();
    rows.sort_by_key(|r| r["seenAgoSecs"].as_u64().unwrap_or(u64::MAX));
    Json(serde_json::json!({ "boxes": rows })).into_response()
}

/// A path under the box's `/api/` this relay will ask for: plain segments
/// only, so the box's nginx and lososd see exactly the route the page named.
fn plain_route(rest: &str) -> bool {
    !rest.is_empty()
        && rest.len() <= 200
        && rest.split('/').all(|seg| {
            !seg.is_empty()
                && seg != "."
                && seg != ".."
                && seg
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        })
}

/// A query string this relay passes on: the URL-safe characters a page's
/// search or filter needs, and nothing that could end the URL early.
fn plain_query(query: &str) -> bool {
    query.len() <= 512
        && query.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(
                    b,
                    b'-' | b'_'
                        | b'.'
                        | b'~'
                        | b'%'
                        | b'='
                        | b'&'
                        | b'+'
                        | b'!'
                        | b'*'
                        | b'('
                        | b')'
                        | b'\''
                )
        })
}

/// One call to a managed box's lososd, through the box's tunnel.
async fn relay(
    State(st): State<AppState>,
    UrlPath((id, rest)): UrlPath<(String, String)>,
    RawQuery(query): RawQuery,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(manage) = st.manage.clone() else {
        return off();
    };
    if !manage.signed_in(&headers) {
        return unauthorized();
    }
    if !plain_route(&rest) || !query.as_deref().is_none_or(plain_query) {
        return plain(StatusCode::BAD_REQUEST, "not a route on the box");
    }
    if !manage.is_managed(&id, st.opts.heartbeat_ttl) {
        return plain(
            StatusCode::FORBIDDEN,
            "this box's owner has not let this edge manage it",
        );
    }
    let Some(port) = st
        .reg
        .views()
        .await
        .into_iter()
        .find(|v| v.id == id)
        .map(|v| v.rathole_port)
    else {
        return plain(StatusCode::NOT_FOUND, "no box by that name");
    };
    let token = match box_token(&st, &id).await {
        Some(token) => token,
        None => return plain(StatusCode::NOT_FOUND, "no box by that name"),
    };
    let Ok(_permit) = manage.relays.try_acquire() else {
        return plain(StatusCode::SERVICE_UNAVAILABLE, "busy; retry later");
    };

    let mut url = format!("http://127.0.0.1:{port}/api/{rest}");
    if let Some(q) = query.filter(|q| !q.is_empty()) {
        url.push('?');
        url.push_str(&q);
    }
    let mut req = manage
        .http
        .request(method.clone(), &url)
        .header(header::HOST, BOX_HOST)
        .header("X-Losos-Edge", token);
    if let Some(ct) = headers.get(header::CONTENT_TYPE) {
        req = req.header(header::CONTENT_TYPE, ct);
    }
    if !body.is_empty() {
        req = req.body(body);
    }
    match req.send().await {
        Ok(resp) => {
            // A 401 from the box means it no longer takes this edge (the
            // owner switched it off since the last heartbeat). Said as a
            // 403, so the page does not read it as its own key failing.
            if resp.status().as_u16() == 401 {
                return plain(
                    StatusCode::FORBIDDEN,
                    "this box's owner has not let this edge manage it",
                );
            }
            let status =
                StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let content_type = resp.headers().get(header::CONTENT_TYPE).cloned();
            match resp.bytes().await {
                Ok(bytes) => {
                    tracing::info!(
                        target: Action::Serve.target(),
                        "managed {id}: {method} /api/{rest} -> {}",
                        status.as_u16()
                    );
                    let mut out = (status, Body::from(bytes)).into_response();
                    if let Some(ct) = content_type {
                        out.headers_mut().insert(header::CONTENT_TYPE, ct);
                    }
                    out
                }
                Err(e) => {
                    tracing::warn!(target: Action::Serve.target(), "managed {id}: reading the answer failed: {e}");
                    plain(StatusCode::BAD_GATEWAY, "the box stopped answering")
                }
            }
        }
        Err(e) => {
            tracing::warn!(target: Action::Serve.target(), "managed {id}: {e}");
            plain(StatusCode::BAD_GATEWAY, "the box did not answer")
        }
    }
}

/// The box's own token, which it enrolled with and its lososd checks.
async fn box_token(st: &AppState, id: &str) -> Option<String> {
    let tenants = st.tenants.load(&st.opts.tenants_file).await.ok()?;
    let entry = lookup_tenant(st, &tenants, id).await.ok()??;
    let token = tokio::fs::read_to_string(&entry.token_file).await.ok()?;
    let token = token.trim();
    (!token.is_empty()).then(|| token.to_string())
}

// ── the page ───────────────────────────────────────────────────────────────

/// The admin UI's own policy, with nothing inline and nothing from
/// elsewhere: the page talks to this edge only.
const PAGE_CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; \
img-src 'self' data:; font-src 'self'; connect-src 'self'; manifest-src 'self'; \
base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

async fn page_index(State(st): State<AppState>) -> Response {
    serve_file(&st, "manage/index.html", false).await
}

/// A deep link (`/manage/boxes/<id>`) is the page too, so a reload works.
async fn page_file(State(st): State<AppState>, UrlPath(file): UrlPath<String>) -> Response {
    let named = format!("manage/{file}");
    if file.contains('.') {
        serve_file(&st, &named, false).await
    } else {
        serve_file(&st, "manage/index.html", false).await
    }
}

async fn asset(State(st): State<AppState>, UrlPath(file): UrlPath<String>) -> Response {
    serve_file(&st, &format!("assets/{file}"), true).await
}

async fn theme_boot(State(st): State<AppState>) -> Response {
    serve_file(&st, "theme-boot.js", false).await
}

/// One file of the built admin UI (`--manage-ui`), by its path in it.
/// Hashed assets are cached for a year, like the box's nginx does.
async fn serve_file(st: &AppState, file: &str, immutable: bool) -> Response {
    let Some(root) = st.manage.as_ref().and_then(|m| m.ui.clone()) else {
        return off();
    };
    let rel = Path::new(file);
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return plain(StatusCode::BAD_REQUEST, "bad path");
    }
    let path = root.join(rel);
    let Ok(bytes) = tokio::fs::read(&path).await else {
        return plain(StatusCode::NOT_FOUND, "no such file");
    };
    let mut resp = (StatusCode::OK, Body::from(bytes)).into_response();
    let h = resp.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type(&path)),
    );
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(PAGE_CSP),
    );
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(if immutable {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        }),
    );
    resp
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("json" | "webmanifest") => "application/json",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

/// Shared with [`super::build`], which keeps it in [`AppState`].
pub(super) type Shared = Option<Arc<Manage>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_routes_are_relayed() {
        assert!(plain_route("apps"));
        assert!(plain_route("apps/install"));
        assert!(plain_route("cloud/apps.json"));
        assert!(!plain_route(""));
        assert!(!plain_route("../setup/claim"));
        assert!(!plain_route("apps/../people"));
        assert!(!plain_route("apps//install"));
        assert!(!plain_route("apps/%2e%2e"));
        assert!(!plain_route("apps?x=1"));
    }

    #[test]
    fn only_plain_queries_are_relayed() {
        assert!(plain_query("q=nextcloud%20news&page=2"));
        assert!(plain_query(""));
        // What encodeURIComponent leaves as it is.
        assert!(plain_query("q=it%27s(a)*!"));
        assert!(plain_query("q=it's(a)*!"));
        assert!(!plain_query("q=a#frag"));
        assert!(!plain_query("q=a b"));
        assert!(!plain_query("q=@evil.example/"));
        assert!(!plain_query(&"a".repeat(513)));
    }

    #[test]
    fn a_key_is_made_once_and_kept() {
        let dir = std::env::temp_dir().join(format!("losos-manage-key-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("manage-key");
        let first = load_or_make_key(&file).unwrap();
        assert_eq!(first.len(), 64);
        assert_eq!(load_or_make_key(&file).unwrap(), first);
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        std::fs::write(&file, "not a key").unwrap();
        assert!(load_or_make_key(&file).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
