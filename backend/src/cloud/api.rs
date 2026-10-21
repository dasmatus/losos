//! The JSON API under `/cloud/api/`, for the web app at `/cloud/`.
//!
//! Plain JSON in and out, one call per thing the app does; uploads and
//! downloads stream. Every call is the signed-in person's own: paths are
//! relative to their home, and nothing here can name another person's file.

use crate::cloud::auth::Who;
use crate::cloud::dav::{read_body, receive, serve_file};
use crate::cloud::store::{clean_path, Entry, StoreError};
use crate::cloud::Cloud;
use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::json;

const MAX_JSON: usize = 1 << 20;

pub fn error(status: StatusCode, code: &str, message: &str) -> HttpResponse {
    HttpResponse::build(status).json(json!({ "error": message, "code": code }))
}

/// 423 when a WebDAV client's lock covers one of `paths`: the web app's
/// changes respect the desktop client's locks as WebDAV's own do.
fn locked(cloud: &Cloud, login: &str, paths: &[&str]) -> Option<HttpResponse> {
    paths
        .iter()
        .any(|p| cloud.locks.touches(login, p))
        .then(|| {
            error(
                StatusCode::LOCKED,
                "locked",
                "another app has this file locked",
            )
        })
}
pub fn store_error(e: &StoreError) -> HttpResponse {
    let (status, code) = match e {
        StoreError::NotFound => (StatusCode::NOT_FOUND, "notFound"),
        StoreError::Exists => (StatusCode::CONFLICT, "exists"),
        StoreError::Conflict => (StatusCode::CONFLICT, "conflict"),
        StoreError::Precondition => (StatusCode::PRECONDITION_FAILED, "exists"),
        StoreError::BadName => (StatusCode::BAD_REQUEST, "badName"),
        StoreError::Loop => (StatusCode::BAD_REQUEST, "loop"),
        StoreError::Full => (StatusCode::INSUFFICIENT_STORAGE, "full"),
        StoreError::Io(err) => {
            tracing::warn!("cloud: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal")
        }
        StoreError::Db(err) => {
            tracing::warn!("cloud: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal")
        }
    };
    error(status, code, &e.to_string())
}

fn query(req: &HttpRequest) -> Vec<(String, String)> {
    url::form_urlencoded::parse(req.query_string().as_bytes())
        .into_owned()
        .collect()
}

fn param(req: &HttpRequest, name: &str) -> Option<String> {
    query(req)
        .into_iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v)
}

fn path_param(req: &HttpRequest) -> Result<String, HttpResponse> {
    clean_path(&param(req, "path").unwrap_or_default()).map_err(|e| store_error(&e))
}

async fn json_body<T: for<'de> Deserialize<'de>>(body: web::Payload) -> Result<T, HttpResponse> {
    let bytes = read_body(body, MAX_JSON).await?;
    serde_json::from_slice(&bytes)
        .map_err(|e| error(StatusCode::BAD_REQUEST, "badRequest", &e.to_string()))
}

fn ok<T: serde::Serialize>(v: T) -> HttpResponse {
    HttpResponse::Ok().json(v)
}

#[derive(Deserialize)]
struct PathBody {
    path: String,
}

#[derive(Deserialize)]
struct FromTo {
    from: String,
    to: String,
    #[serde(default)]
    overwrite: bool,
}

#[derive(Deserialize)]
struct Paths {
    paths: Vec<String>,
}

#[derive(Deserialize)]
struct Favorite {
    path: String,
    favorite: bool,
}

#[derive(Deserialize)]
struct Id {
    id: i64,
}

#[derive(Deserialize)]
struct Client {
    #[serde(default)]
    client: String,
}

#[derive(Deserialize)]
struct VersionRef {
    id: i64,
    mtime: i64,
}

/// One call to `/cloud/api/…`.
pub async fn handle(
    req: &HttpRequest,
    cloud: &Cloud,
    who: &Who,
    body: web::Payload,
) -> HttpResponse {
    let store = &cloud.store;
    let login = who.login.as_str();
    if let Err(e) = store.ensure_home(login) {
        return store_error(&e);
    }
    let rest = req
        .path()
        .trim_start_matches("/cloud/api")
        .trim_matches('/');
    let segs: Vec<&str> = if rest.is_empty() {
        Vec::new()
    } else {
        rest.split('/').collect()
    };
    let method = req.method().as_str();
    match (method, segs.as_slice()) {
        ("GET", ["me"]) => {
            let used = store.used(login).unwrap_or(0);
            ok(json!({
                "login": login,
                "name": who.name,
                "email": who.email,
                "used": used,
                "free": store.free(),
                "product": cloud.config.product,
            }))
        }
        ("GET", ["files"]) => {
            let path = match path_param(req) {
                Ok(p) => p,
                Err(r) => return r,
            };
            let folder = match store.stat(login, &path) {
                Ok(f) => f,
                Err(e) => return store_error(&e),
            };
            if !folder.dir {
                return ok(json!({ "folder": folder, "entries": [] }));
            }
            match store.list(login, &path) {
                Ok(entries) => ok(json!({ "folder": folder, "entries": entries })),
                Err(e) => store_error(&e),
            }
        }
        ("GET", ["files", "by-id", id]) => match id.parse::<i64>() {
            Ok(id) => match store.by_id(login, id) {
                Ok(e) => ok(e),
                Err(e) => store_error(&e),
            },
            Err(_) => store_error(&StoreError::NotFound),
        },
        ("POST", ["files", "folder"]) => {
            let b: PathBody = match json_body(body).await {
                Ok(b) => b,
                Err(r) => return r,
            };
            match clean_path(&b.path).and_then(|p| store.mkdir(login, &p)) {
                Ok(e) => ok(e),
                Err(e) => store_error(&e),
            }
        }
        ("POST", ["files", op @ ("move" | "copy")]) => {
            let b: FromTo = match json_body(body).await {
                Ok(b) => b,
                Err(r) => return r,
            };
            let (from, to) = match (clean_path(&b.from), clean_path(&b.to)) {
                (Ok(f), Ok(t)) => (f, t),
                _ => return store_error(&StoreError::BadName),
            };
            let touched: &[&str] = if *op == "move" { &[&from, &to] } else { &[&to] };
            if let Some(r) = locked(cloud, login, touched) {
                return r;
            }
            let r = if *op == "move" {
                store.rename(login, &from, &to, b.overwrite)
            } else {
                store.copy(login, &from, &to, b.overwrite)
            };
            match r {
                Ok(w) => ok(w.entry),
                Err(e) => store_error(&e),
            }
        }
        ("POST", ["files", "delete"]) => {
            let b: Paths = match json_body(body).await {
                Ok(b) => b,
                Err(r) => return r,
            };
            let cleaned: Vec<String> = b.paths.iter().filter_map(|p| clean_path(p).ok()).collect();
            let refs: Vec<&str> = cleaned.iter().map(String::as_str).collect();
            if let Some(r) = locked(cloud, login, &refs) {
                return r;
            }
            let mut trashed = Vec::new();
            for p in &b.paths {
                match clean_path(p).and_then(|p| store.delete(login, &p, None)) {
                    Ok(t) => trashed.push(t),
                    Err(e) => return store_error(&e),
                }
            }
            ok(json!({ "trashed": trashed }))
        }
        ("POST", ["files", "favorite"]) => {
            let b: Favorite = match json_body(body).await {
                Ok(b) => b,
                Err(r) => return r,
            };
            match clean_path(&b.path).and_then(|p| store.set_favorite(login, &p, b.favorite)) {
                Ok(e) => ok(e),
                Err(e) => store_error(&e),
            }
        }
        ("PUT", ["files", "content"]) => {
            let path = match path_param(req) {
                Ok(p) => p,
                Err(r) => return r,
            };
            match store.stat(login, crate::cloud::store::parent_of(&path).unwrap_or("")) {
                Ok(p) if p.dir => {}
                _ => return store_error(&StoreError::Conflict),
            }
            if let Some(r) = locked(cloud, login, &[&path]) {
                return r;
            }
            let expected = req
                .headers()
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse().ok());
            let (temp, sha) = match receive(cloud, login, body, expected).await {
                Ok(v) => v,
                Err(r) => return r,
            };
            let mtime = param(req, "mtime").and_then(|m| m.parse::<i64>().ok());
            let if_match = param(req, "ifMatch");
            let none = param(req, "ifNew").is_some();
            match store.commit_file(
                login,
                &path,
                temp,
                mtime,
                if_match.as_deref(),
                none,
                Some(format!("SHA1:{sha}")),
            ) {
                Ok(w) => {
                    cloud.changed(login, &w.entry);
                    ok(w.entry)
                }
                Err(e) => store_error(&e),
            }
        }
        ("GET" | "HEAD", ["files", "content"]) => {
            let path = match path_param(req) {
                Ok(p) => p,
                Err(r) => return r,
            };
            let e = match store.stat(login, &path) {
                Ok(e) if !e.dir => e,
                Ok(_) => return store_error(&StoreError::Conflict),
                Err(e) => return store_error(&e),
            };
            serve_file(
                req,
                &store.disk_path(login, &path),
                &e.name,
                &e.mime,
                Some(&e.etag),
                e.mtime,
                Some(store.oc_id(e.id)),
                param(req, "download").is_some(),
            )
        }
        ("GET", ["files", "recent"]) => list(store.recent(login, 100)),
        ("GET", ["files", "favorites"]) => list(store.favorites(login)),
        ("GET", ["files", "search"]) => {
            let q = param(req, "q").unwrap_or_default();
            if q.trim().is_empty() {
                return ok(json!({ "entries": [] }));
            }
            let limit = param(req, "limit")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(200)
                .clamp(1, 200);
            list(store.search(login, q.trim(), limit))
        }
        ("GET", ["files", "media"]) => {
            let kinds: &[&str] = match param(req, "kind").as_deref() {
                Some("video") => &["video/"],
                Some("audio") => &["audio/"],
                Some("image") => &["image/"],
                _ => &["image/", "video/"],
            };
            let limit = param(req, "limit")
                .and_then(|l| l.parse().ok())
                .unwrap_or(500usize)
                .min(5000);
            list(store.by_mime_prefix(login, kinds, limit))
        }
        ("GET", ["files", "versions"]) => {
            let path = match path_param(req) {
                Ok(p) => p,
                Err(r) => return r,
            };
            match store.versions(login, &path) {
                Ok(v) => ok(json!({ "versions": v })),
                Err(e) => store_error(&e),
            }
        }
        ("GET", ["files", "versions", id, mtime]) => {
            let (Ok(id), Ok(mtime)) = (id.parse::<i64>(), mtime.parse::<i64>()) else {
                return store_error(&StoreError::NotFound);
            };
            let e = match store.by_id(login, id) {
                Ok(e) => e,
                Err(e) => return store_error(&e),
            };
            match store.version_path(login, id, mtime) {
                Ok(p) => serve_file(req, &p, &e.name, &e.mime, None, mtime, None, true),
                Err(e) => store_error(&e),
            }
        }
        ("POST", ["files", "versions", "restore"]) => {
            let b: VersionRef = match json_body(body).await {
                Ok(b) => b,
                Err(r) => return r,
            };
            if let Ok(e) = store.by_id(login, b.id) {
                if let Some(r) = locked(cloud, login, &[&e.path]) {
                    return r;
                }
            }
            match store.restore_version(login, b.id, b.mtime) {
                Ok(e) => {
                    cloud.changed(login, &e);
                    ok(e)
                }
                Err(e) => store_error(&e),
            }
        }
        ("GET", ["trash"]) => match store.trash(login) {
            Ok(t) => ok(json!({ "entries": t, "days": cloud.config.trash_days })),
            Err(e) => store_error(&e),
        },
        ("POST", ["trash", "restore"]) => {
            let b: Id = match json_body(body).await {
                Ok(b) => b,
                Err(r) => return r,
            };
            match store.restore(login, b.id, None) {
                Ok(e) => ok(e),
                Err(e) => store_error(&e),
            }
        }
        ("POST", ["trash", "delete"]) => {
            let b: Id = match json_body(body).await {
                Ok(b) => b,
                Err(r) => return r,
            };
            match store.purge(login, b.id) {
                Ok(()) => ok(json!({})),
                Err(e) => store_error(&e),
            }
        }
        ("POST", ["trash", "empty"]) => match store.empty_trash(login) {
            Ok(n) => ok(json!({ "deleted": n })),
            Err(e) => store_error(&e),
        },
        ("GET", ["devices"]) => match cloud.auth.app_passwords(store, login) {
            Ok(d) => ok(json!({ "devices": d })),
            Err(e) => store_error(&e),
        },
        ("DELETE", ["devices", id]) => match id.parse::<i64>() {
            Ok(id) => match cloud.auth.revoke(store, login, id) {
                Ok(true) => ok(json!({})),
                Ok(false) => store_error(&StoreError::NotFound),
                Err(e) => store_error(&e),
            },
            Err(_) => store_error(&StoreError::NotFound),
        },
        ("GET", ["connect", token]) => match cloud.auth.flow(store, token) {
            Ok(Some(f)) => ok(json!({
                "client": f.client,
                "created": f.created,
                "granted": f.granted.is_some(),
            })),
            Ok(None) => store_error(&StoreError::NotFound),
            Err(e) => store_error(&e),
        },
        // Granted by a person signed in through LosOS Login, never by an
        // app password: one device must not mint another behind the owner's
        // back, which revoking the first would then leave working.
        ("POST", ["connect", _]) if matches!(who.via, crate::cloud::auth::Via::AppPassword(_)) => {
            error(
                StatusCode::FORBIDDEN,
                "appPassword",
                "already an app password",
            )
        }
        ("POST", ["connect", token]) => match cloud.auth.grant_flow(store, token, who) {
            Ok(true) => ok(json!({ "granted": true })),
            Ok(false) => store_error(&StoreError::NotFound),
            Err(e) => store_error(&e),
        },
        // Login Flow v1: the phone app's web view is on the connect page,
        // signed in, and takes the password from the nc:// address the page
        // goes to next. Never for a request that is itself an app.
        ("POST", ["connect-v1"]) => {
            if matches!(who.via, crate::cloud::auth::Via::AppPassword(_)) {
                return error(
                    StatusCode::FORBIDDEN,
                    "appPassword",
                    "already an app password",
                );
            }
            let b: Client = match json_body(body).await {
                Ok(b) => b,
                Err(r) => return r,
            };
            let client = crate::cloud::nc::device_name(&b.client);
            match cloud.auth.new_app_password(store, login, &client) {
                Ok((_, pw)) => ok(json!({
                    "server": crate::cloud::nc::origin(req),
                    "login": login,
                    "appPassword": pw,
                })),
                Err(e) => store_error(&e),
            }
        }
        ("GET", ["preview", id]) => {
            let Ok(id) = id.parse::<i64>() else {
                return store_error(&StoreError::NotFound);
            };
            let size = param(req, "size")
                .and_then(|s| s.parse().ok())
                .unwrap_or(256u32);
            crate::cloud::preview::serve(req, cloud, login, id, size)
        }
        _ => crate::cloud::extensions::api(req, cloud, who, &segs, body).await,
    }
}

fn list(r: Result<Vec<Entry>, StoreError>) -> HttpResponse {
    match r {
        Ok(entries) => ok(json!({ "entries": entries })),
        Err(e) => store_error(&e),
    }
}
