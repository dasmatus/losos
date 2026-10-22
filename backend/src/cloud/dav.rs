//! WebDAV, the way the Nextcloud desktop and phone apps, file managers and
//! `curl` speak it.
//!
//! Five spaces under `/remote.php/`:
//!
//! | path                                   | what                         |
//! |----------------------------------------|------------------------------|
//! | `webdav/…`, `dav/files/<login>/…`      | the person's files           |
//! | `dav/uploads/<login>/<transfer>/…`     | chunks of a large upload     |
//! | `dav/trashbin/<login>/{trash,restore}` | the trash bin                |
//! | `dav/versions/<login>/{versions,restore}` | earlier contents of files |
//! | `dav/`                                 | the root, for discovery      |
//!
//! The properties are Nextcloud's (`oc:fileid`, `oc:permissions`,
//! `oc:checksums` and the rest): the apps parse them, and a missing one is a
//! feature the app turns off, never an error. A write answers with the new
//! `OC-ETag` and `OC-FileId` so the desktop app need not ask again.

use crate::cloud::auth::Who;
use crate::cloud::store::{clean_path, is_within, mime_for, parent_of, Entry, StoreError, Written};
use crate::cloud::util::{
    decode_path, encode_path, http_date, iso_date, now, random_token, xml_escape,
};
use crate::cloud::xml::{self, Name, PropChange, PropRequest, Response, DAV, NC, OC, SABRE};
use crate::cloud::Cloud;
use actix_web::http::{header, Method, StatusCode};
use actix_web::{web, HttpRequest, HttpResponse};
use futures_util::StreamExt;
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;
use std::io::Write;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Largest XML body read into memory.
const MAX_XML: usize = 1 << 20;
/// Upload transfers not finished in this long are cleared away.
pub const UPLOAD_TTL: Duration = Duration::from_secs(24 * 3600);
/// Longest lock a client gets.
const MAX_LOCK: u64 = 3600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Space {
    Files,
    Uploads,
    Trash,
    Versions,
    /// `/remote.php/dav/` and the collections right below it.
    Root,
}

/// Where a request path points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub space: Space,
    pub owner: String,
    /// Decoded and cleaned, relative to the space's root for the owner.
    pub rest: String,
    /// The href of the space's root for the owner, encoded, no trailing slash.
    pub base: String,
}

impl Target {
    pub fn href(&self, path: &str, dir: bool) -> String {
        let mut h = self.base.clone();
        if !path.is_empty() {
            h.push('/');
            h.push_str(&encode_path(path));
        }
        if dir {
            h.push('/');
        }
        h
    }
}

#[derive(Debug)]
pub enum Resolve {
    Ok(Target),
    NotFound,
    Forbidden,
    BadPath,
}

/// Map a raw (encoded) request path to a target, for the person `login`.
pub fn resolve(raw: &str, login: &str) -> Resolve {
    let Some(path) = decode_path(raw) else {
        return Resolve::BadPath;
    };
    let path = path.trim_end_matches('/');
    let (space, owner, rest, base) = if let Some(rest) = strip(path, "/remote.php/webdav") {
        (
            Space::Files,
            login.to_owned(),
            rest,
            "/remote.php/webdav".to_owned(),
        )
    } else if let Some(r) = strip(path, "/remote.php/dav") {
        if r.is_empty() {
            return Resolve::Ok(Target {
                space: Space::Root,
                owner: login.to_owned(),
                rest: String::new(),
                base: "/remote.php/dav".into(),
            });
        }
        let mut parts = r.splitn(3, '/');
        let kind = parts.next().unwrap_or_default();
        let owner = parts.next().unwrap_or_default();
        let rest = parts.next().unwrap_or_default();
        let space = match kind {
            "files" => Space::Files,
            "uploads" => Space::Uploads,
            "trashbin" => Space::Trash,
            "versions" => Space::Versions,
            _ => return Resolve::NotFound,
        };
        if owner.is_empty() {
            return Resolve::Ok(Target {
                space: Space::Root,
                owner: login.to_owned(),
                rest: kind.to_owned(),
                base: "/remote.php/dav".into(),
            });
        }
        if owner != login {
            return Resolve::Forbidden;
        }
        (
            space,
            owner.to_owned(),
            rest,
            format!("/remote.php/dav/{kind}/{}", encode_path(owner)),
        )
    } else {
        return Resolve::NotFound;
    };
    match clean_path(rest) {
        Ok(rest) => Resolve::Ok(Target {
            space,
            owner,
            rest,
            base,
        }),
        Err(_) => Resolve::BadPath,
    }
}

fn strip<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = path.strip_prefix(prefix)?;
    if rest.is_empty() {
        Some("")
    } else {
        rest.strip_prefix('/')
    }
}

/// The path part of a `Destination` header, which may be a full URL.
pub fn destination_path(dest: &str) -> Option<&str> {
    let path = if let Some(i) = dest.find("://") {
        let after = &dest[i + 3..];
        after.find('/').map(|j| &after[j..]).unwrap_or("/")
    } else if dest.starts_with('/') {
        dest
    } else {
        return None;
    };
    // An app set up against Nextcloud's old address keeps sending it.
    Some(match path.strip_prefix(crate::cloud::server::OLD_PREFIX) {
        Some(rest) if rest.starts_with("/remote.php/") => rest,
        _ => path,
    })
}

/// A lock a client holds.
#[derive(Debug, Clone)]
struct Lock {
    token: String,
    owner_xml: String,
    exclusive: bool,
    infinite: bool,
    until: Instant,
    timeout: u64,
}

#[derive(Default)]
pub struct Locks {
    held: Mutex<HashMap<(String, String), Lock>>,
}

impl Locks {
    fn active(&self, owner: &str, path: &str) -> Vec<(String, Lock)> {
        let mut held = self.held.lock().unwrap_or_else(|p| p.into_inner());
        held.retain(|_, l| l.until > Instant::now());
        held.iter()
            .filter(|((o, p), l)| o == owner && (p == path || (l.infinite && is_within(path, p))))
            .map(|((_, p), l)| (p.clone(), l.clone()))
            .collect()
    }

    /// A lock held on `path` (or a folder above it) whose token the request
    /// does not carry.
    fn blocks(&self, owner: &str, path: &str, if_header: &str) -> bool {
        self.active(owner, path)
            .iter()
            .any(|(_, l)| !if_header.contains(&l.token))
    }

    /// Whether a WebDAV client holds a lock that a change to `path` would
    /// cross: on it, on a folder above it that covers it, or on anything
    /// inside it. For the web app's API, which carries no lock token, so
    /// any such lock is someone else's.
    pub fn touches(&self, owner: &str, path: &str) -> bool {
        let mut held = self.held.lock().unwrap_or_else(|p| p.into_inner());
        held.retain(|_, l| l.until > Instant::now());
        held.iter().any(|((o, p), l)| {
            o == owner && (p == path || is_within(p, path) || (l.infinite && is_within(path, p)))
        })
    }

    fn take(&self, owner: &str, path: &str, token: &str) -> bool {
        let mut held = self.held.lock().unwrap_or_else(|p| p.into_inner());
        let key = (owner.to_owned(), path.to_owned());
        match held.get(&key) {
            Some(l) if l.token == token => {
                held.remove(&key);
                true
            }
            _ => false,
        }
    }

    fn put(&self, owner: &str, path: &str, lock: Lock) {
        let mut held = self.held.lock().unwrap_or_else(|p| p.into_inner());
        held.insert((owner.to_owned(), path.to_owned()), lock);
    }

    fn refresh(&self, owner: &str, if_header: &str, timeout: u64) -> Option<(String, Lock)> {
        let mut held = self.held.lock().unwrap_or_else(|p| p.into_inner());
        for ((o, p), l) in held.iter_mut() {
            if o == owner && if_header.contains(&l.token) {
                l.until = Instant::now() + Duration::from_secs(timeout);
                l.timeout = timeout;
                return Some((p.clone(), l.clone()));
            }
        }
        None
    }

    /// A move takes the locks along; a delete drops them.
    fn forget(&self, owner: &str, path: &str) {
        let mut held = self.held.lock().unwrap_or_else(|p| p.into_inner());
        held.retain(|(o, p), _| !(o == owner && is_within(p, path)));
    }
}

/// Dead properties: whatever a client sets that the server does not know,
/// kept as written and given back (Windows Explorer and the macOS Finder
/// both set some).
pub const PROPS_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS props (
  file  INTEGER NOT NULL,
  ns    TEXT    NOT NULL,
  name  TEXT    NOT NULL,
  value TEXT    NOT NULL,
  PRIMARY KEY (file, ns, name)
);
"#;

fn dead_props(cloud: &Cloud, file: i64) -> Vec<(Name, String)> {
    cloud
        .store
        .with_db(|db| {
            let mut stmt = db.prepare("SELECT ns, name, value FROM props WHERE file = ?1")?;
            let rows = stmt.query_map(params![file], |r| {
                Ok((
                    Name::new(&r.get::<_, String>(0)?, &r.get::<_, String>(1)?),
                    r.get(2)?,
                ))
            })?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
}

fn dead_prop(cloud: &Cloud, file: i64, n: &Name) -> Option<String> {
    cloud
        .store
        .with_db(|db| {
            Ok(db
                .query_row(
                    "SELECT value FROM props WHERE file = ?1 AND ns = ?2 AND name = ?3",
                    params![file, n.ns, n.local],
                    |r| r.get(0),
                )
                .optional()?)
        })
        .ok()
        .flatten()
}

/// The status and body for a store error.
pub fn store_error(e: &StoreError) -> HttpResponse {
    let (status, exception) = match e {
        StoreError::NotFound => (StatusCode::NOT_FOUND, "Sabre\\DAV\\Exception\\NotFound"),
        StoreError::Exists => (
            StatusCode::METHOD_NOT_ALLOWED,
            "Sabre\\DAV\\Exception\\MethodNotAllowed",
        ),
        StoreError::Conflict => (StatusCode::CONFLICT, "Sabre\\DAV\\Exception\\Conflict"),
        StoreError::Precondition => (
            StatusCode::PRECONDITION_FAILED,
            "Sabre\\DAV\\Exception\\PreconditionFailed",
        ),
        StoreError::BadName => (StatusCode::BAD_REQUEST, "Sabre\\DAV\\Exception\\BadRequest"),
        StoreError::Loop => (StatusCode::FORBIDDEN, "Sabre\\DAV\\Exception\\Forbidden"),
        StoreError::Full => (
            StatusCode::INSUFFICIENT_STORAGE,
            "Sabre\\DAV\\Exception\\InsufficientStorage",
        ),
        StoreError::Io(err) => {
            tracing::warn!("cloud: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Sabre\\DAV\\Exception")
        }
        StoreError::Db(err) => {
            tracing::warn!("cloud: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Sabre\\DAV\\Exception")
        }
    };
    dav_error(status, exception, &e.to_string())
}

pub fn dav_error(status: StatusCode, exception: &str, message: &str) -> HttpResponse {
    HttpResponse::build(status)
        .content_type("application/xml; charset=utf-8")
        .body(xml::error(exception, message))
}

fn multistatus(responses: &[Response]) -> HttpResponse {
    HttpResponse::build(StatusCode::MULTI_STATUS)
        .content_type("application/xml; charset=utf-8")
        .body(xml::multistatus(responses))
}

pub async fn read_body(mut body: web::Payload, limit: usize) -> Result<Vec<u8>, HttpResponse> {
    let mut out = Vec::new();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|_| HttpResponse::BadRequest().finish())?;
        if out.len() + chunk.len() > limit {
            return Err(HttpResponse::PayloadTooLarge().finish());
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

fn header_str<'a>(req: &'a HttpRequest, name: &str) -> Option<&'a str> {
    req.headers().get(name).and_then(|v| v.to_str().ok())
}

/// An ETag from a header, quotes and weakness taken off.
fn bare_etag(v: &str) -> &str {
    v.trim().trim_start_matches("W/").trim_matches('"')
}

fn quoted(etag: &str) -> String {
    format!("\"{etag}\"")
}

/// The headers every successful write carries.
fn written(status: StatusCode, cloud: &Cloud, e: &Entry, mtime_accepted: bool) -> HttpResponse {
    let mut r = HttpResponse::build(status);
    r.insert_header(("OC-ETag", quoted(&e.etag)))
        .insert_header((header::ETAG, quoted(&e.etag)))
        .insert_header(("OC-FileId", cloud.store.oc_id(e.id)));
    if mtime_accepted {
        r.insert_header(("X-OC-MTime", "accepted"));
    }
    r.finish()
}

fn client_mtime(req: &HttpRequest) -> Option<i64> {
    header_str(req, "x-oc-mtime")
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
        .map(|v| v as i64)
}

/// `SHA1:<hex>` from `OC-Checksum`, the only kind checked; other kinds are
/// kept as sent.
fn client_checksum(req: &HttpRequest) -> Option<String> {
    let v = header_str(req, "oc-checksum")?.trim();
    let (kind, value) = v.split_once(':')?;
    if value.is_empty() {
        return None;
    }
    Some(format!(
        "{}:{}",
        kind.to_ascii_uppercase(),
        value.to_ascii_lowercase()
    ))
}

/// Everything about one request the handlers need.
pub struct Ctx<'a> {
    pub cloud: &'a Cloud,
    pub who: &'a Who,
    pub req: &'a HttpRequest,
}

impl Ctx<'_> {
    fn if_header(&self) -> &str {
        header_str(self.req, "if").unwrap_or_default()
    }

    fn locked(&self, owner: &str, path: &str) -> bool {
        self.cloud.locks.blocks(owner, path, self.if_header())
    }

    fn locked_response() -> HttpResponse {
        dav_error(
            StatusCode::LOCKED,
            "Sabre\\DAV\\Exception\\Locked",
            "the resource is locked",
        )
    }

    fn permissions(&self, e: &Entry) -> String {
        let mut p = String::from("RGDNV");
        if e.path.is_empty() {
            // The home itself cannot be deleted, renamed or moved.
            p = "RG".into();
        }
        if e.dir {
            p.push_str("CK");
        } else {
            p.push('W');
        }
        p
    }

    /// One property of a file or folder, as inner XML, or `None` if the
    /// entry has no such property.
    fn prop(&self, e: &Entry, n: &Name) -> Option<String> {
        let store = &self.cloud.store;
        match (n.ns.as_str(), n.local.as_str()) {
            (DAV, "resourcetype") => Some(if e.dir {
                "<d:collection/>".into()
            } else {
                String::new()
            }),
            (DAV, "getcontentlength") if !e.dir => Some(e.size.to_string()),
            (DAV, "getcontenttype") if !e.dir => Some(xml_escape(&e.mime)),
            (DAV, "getetag") => Some(xml_escape(&quoted(&e.etag))),
            (DAV, "getlastmodified") => Some(http_date(e.mtime)),
            (DAV, "creationdate") => Some(iso_date(e.mtime)),
            (DAV, "displayname") => Some(xml_escape(if e.path.is_empty() {
                &e.owner
            } else {
                &e.name
            })),
            (DAV, "quota-used-bytes") if e.dir => Some(e.size.to_string()),
            (DAV, "quota-available-bytes") if e.dir => Some(store.free().to_string()),
            (DAV, "supportedlock") => Some(
                "<d:lockentry><d:lockscope><d:exclusive/></d:lockscope><d:locktype><d:write/></d:locktype></d:lockentry>\
                 <d:lockentry><d:lockscope><d:shared/></d:lockscope><d:locktype><d:write/></d:locktype></d:lockentry>"
                    .into(),
            ),
            (DAV, "lockdiscovery") => Some(
                self.cloud
                    .locks
                    .active(&e.owner, &e.path)
                    .iter()
                    .map(|(p, l)| active_lock(l, &self.files_href(p)))
                    .collect(),
            ),
            (OC, "id") => Some(store.oc_id(e.id)),
            (OC, "fileid") => Some(e.id.to_string()),
            (OC, "permissions") => Some(self.permissions(e)),
            (OC, "size") => Some(e.size.to_string()),
            (OC, "favorite") => Some(if e.favorite { "1" } else { "0" }.into()),
            (OC, "owner-id") => Some(xml_escape(&e.owner)),
            (OC, "owner-display-name") => Some(xml_escape(&self.who.name)),
            (OC, "checksums") => e
                .checksum
                .as_ref()
                .map(|c| format!("<oc:checksum>{}</oc:checksum>", xml_escape(c))),
            (OC, "share-types") => Some(String::new()),
            (OC, "tags") => Some(String::new()),
            (OC, "comments-count") | (OC, "comments-unread") => Some("0".into()),
            (OC, "data-fingerprint") => Some(String::new()),
            (NC, "has-preview") => Some(
                crate::cloud::preview::can_preview(&e.mime)
                    .to_string(),
            ),
            (NC, "mount-type") => Some(String::new()),
            (NC, "is-encrypted") => Some("0".into()),
            (NC, "hidden") => Some("false".into()),
            (NC, "rich-workspace") | (NC, "rich-workspace-file") | (NC, "note") => {
                Some(String::new())
            }
            (NC, "sharees") => Some(String::new()),
            (NC, "creation_time") | (NC, "upload_time") => Some("0".into()),
            (NC, "contained-folder-count") if e.dir => {
                self.cloud.store.list(&e.owner, &e.path).ok().map(|c| {
                    c.iter().filter(|c| c.dir).count().to_string()
                })
            }
            (NC, "contained-file-count") if e.dir => {
                self.cloud.store.list(&e.owner, &e.path).ok().map(|c| {
                    c.iter().filter(|c| !c.dir).count().to_string()
                })
            }
            _ => dead_prop(self.cloud, e.id, n),
        }
    }

    fn files_href(&self, path: &str) -> String {
        let mut h = format!("/remote.php/dav/files/{}", encode_path(&self.who.login));
        if !path.is_empty() {
            h.push('/');
            h.push_str(&encode_path(path));
        }
        h
    }

    fn allprop_names(&self, e: &Entry) -> Vec<Name> {
        let mut names = vec![
            Name::new(DAV, "resourcetype"),
            Name::new(DAV, "getetag"),
            Name::new(DAV, "getlastmodified"),
            Name::new(DAV, "creationdate"),
            Name::new(DAV, "displayname"),
            Name::new(DAV, "supportedlock"),
            Name::new(DAV, "lockdiscovery"),
            Name::new(OC, "id"),
            Name::new(OC, "fileid"),
            Name::new(OC, "permissions"),
            Name::new(OC, "size"),
            Name::new(OC, "favorite"),
        ];
        if e.dir {
            names.push(Name::new(DAV, "quota-used-bytes"));
            names.push(Name::new(DAV, "quota-available-bytes"));
        } else {
            names.push(Name::new(DAV, "getcontentlength"));
            names.push(Name::new(DAV, "getcontenttype"));
        }
        if e.checksum.is_some() {
            names.push(Name::new(OC, "checksums"));
        }
        names.extend(dead_props(self.cloud, e.id).into_iter().map(|(n, _)| n));
        names
    }

    pub fn entry_response(&self, t: &Target, e: &Entry, req: &PropRequest) -> Response {
        let mut r = Response::new(t.href(&e.path, e.dir));
        match req {
            PropRequest::PropName => {
                for n in self.allprop_names(e) {
                    r.found.push((n, String::new()));
                }
            }
            PropRequest::AllProp(include) => {
                let mut names = self.allprop_names(e);
                for n in include {
                    if !names.contains(n) {
                        names.push(n.clone());
                    }
                }
                for n in names {
                    match self.prop(e, &n) {
                        Some(v) => r.found.push((n, v)),
                        None => r.missing.push(n),
                    }
                }
            }
            PropRequest::Props(names) => {
                for n in names {
                    match self.prop(e, n) {
                        Some(v) => r.found.push((n.clone(), v)),
                        None => r.missing.push(n.clone()),
                    }
                }
            }
        }
        r
    }
}

fn active_lock(l: &Lock, href: &str) -> String {
    format!(
        "<d:activelock><d:lockscope>{}</d:lockscope><d:locktype><d:write/></d:locktype>\
         <d:depth>{}</d:depth>{}<d:timeout>Second-{}</d:timeout>\
         <d:locktoken><d:href>{}</d:href></d:locktoken><d:lockroot><d:href>{}</d:href></d:lockroot></d:activelock>",
        if l.exclusive { "<d:exclusive/>" } else { "<d:shared/>" },
        if l.infinite { "infinity" } else { "0" },
        if l.owner_xml.is_empty() {
            String::new()
        } else {
            format!("<d:owner>{}</d:owner>", l.owner_xml)
        },
        l.timeout,
        xml_escape(&l.token),
        xml_escape(href),
    )
}

/// The entry point: one request to `/remote.php/…`, already signed in.
pub async fn handle(ctx: Ctx<'_>, body: web::Payload) -> HttpResponse {
    if crate::cloud::caldav::route(ctx.req.path()).is_some() {
        if let Some(r) = crate::cloud::caldav::handle(ctx.cloud, ctx.who, ctx.req, body).await {
            return r;
        }
        return HttpResponse::NotFound().finish();
    }
    let t = match resolve(ctx.req.path(), &ctx.who.login) {
        Resolve::Ok(t) => t,
        Resolve::NotFound => {
            return dav_error(
                StatusCode::NOT_FOUND,
                "Sabre\\DAV\\Exception\\NotFound",
                "no such path",
            )
        }
        Resolve::Forbidden => {
            return dav_error(
                StatusCode::FORBIDDEN,
                "Sabre\\DAV\\Exception\\Forbidden",
                "not your files",
            )
        }
        Resolve::BadPath => {
            return dav_error(
                StatusCode::BAD_REQUEST,
                "Sabre\\DAV\\Exception\\BadRequest",
                "a name the cloud cannot hold",
            )
        }
    };
    if let Err(e) = ctx.cloud.store.ensure_home(&t.owner) {
        return store_error(&e);
    }
    let method = ctx.req.method().as_str().to_owned();
    match (t.space, method.as_str()) {
        (_, "OPTIONS") => options(),
        (Space::Root, "PROPFIND") => root_propfind(&ctx, &t, body).await,
        (Space::Root, "SEARCH") => crate::cloud::search::handle(&ctx, body).await,
        (Space::Root, _) => method_not_allowed(),
        (Space::Files, "PROPFIND") => propfind(&ctx, &t, body).await,
        (Space::Files, "PROPPATCH") => proppatch(&ctx, &t, body).await,
        (Space::Files, "GET" | "HEAD") => get(&ctx, &t),
        (Space::Files, "PUT") => put(&ctx, &t, body).await,
        (Space::Files, "MKCOL") => mkcol(&ctx, &t, body).await,
        (Space::Files, "DELETE") => delete(&ctx, &t),
        (Space::Files, "MOVE") => move_or_copy(&ctx, &t, true),
        (Space::Files, "COPY") => move_or_copy(&ctx, &t, false),
        (Space::Files, "LOCK") => lock(&ctx, &t, body).await,
        (Space::Files, "UNLOCK") => unlock(&ctx, &t),
        (Space::Files, "REPORT") => report(&ctx, &t, body).await,
        (Space::Uploads, _) => uploads(&ctx, &t, &method, body).await,
        (Space::Trash, _) => trash(&ctx, &t, &method, body).await,
        (Space::Versions, _) => versions(&ctx, &t, &method, body).await,
        _ => method_not_allowed(),
    }
}

fn method_not_allowed() -> HttpResponse {
    dav_error(
        StatusCode::METHOD_NOT_ALLOWED,
        "Sabre\\DAV\\Exception\\MethodNotAllowed",
        "not here",
    )
}

pub fn options() -> HttpResponse {
    HttpResponse::Ok()
        .insert_header((
            "DAV",
            "1, 2, 3, access-control, calendar-access, addressbook, extended-mkcol, nextcloud-checksum-update",
        ))
        .insert_header(("MS-Author-Via", "DAV"))
        .insert_header((
            header::ALLOW,
            "OPTIONS, GET, HEAD, DELETE, PROPFIND, PUT, PROPPATCH, COPY, MOVE, REPORT, SEARCH, LOCK, UNLOCK, MKCOL, MKCALENDAR",
        ))
        .insert_header(("Accept-Ranges", "bytes"))
        .finish()
}

fn depth(req: &HttpRequest) -> Option<u32> {
    match header_str(req, "depth").map(str::trim) {
        Some("0") => Some(0),
        Some("1") | None => Some(1),
        Some(d) if d.eq_ignore_ascii_case("infinity") => None,
        Some(_) => Some(1),
    }
}

async fn parse_request(body: web::Payload) -> Result<PropRequest, HttpResponse> {
    let bytes = read_body(body, MAX_XML).await?;
    let text = String::from_utf8_lossy(&bytes);
    xml::parse_propfind(&text).map_err(|e| {
        dav_error(
            StatusCode::BAD_REQUEST,
            "Sabre\\DAV\\Exception\\BadRequest",
            &e.to_string(),
        )
    })
}

async fn propfind(ctx: &Ctx<'_>, t: &Target, body: web::Payload) -> HttpResponse {
    let req = match parse_request(body).await {
        Ok(r) => r,
        Err(r) => return r,
    };
    let entry = match ctx.cloud.store.stat(&t.owner, &t.rest) {
        Ok(e) => e,
        Err(e) => return store_error(&e),
    };
    let mut responses = vec![ctx.entry_response(t, &entry, &req)];
    if entry.dir {
        let children = match depth(ctx.req) {
            Some(0) => Ok(Vec::new()),
            Some(_) => ctx.cloud.store.list(&t.owner, &t.rest),
            None => ctx.cloud.store.walk(&t.owner, &t.rest),
        };
        match children {
            Ok(children) => {
                for c in &children {
                    responses.push(ctx.entry_response(t, c, &req));
                }
            }
            Err(e) => return store_error(&e),
        }
    }
    multistatus(&responses)
}

async fn root_propfind(ctx: &Ctx<'_>, t: &Target, body: web::Payload) -> HttpResponse {
    let preq = match parse_request(body).await {
        Ok(p) => p,
        Err(r) => return r,
    };
    let login = encode_path(&ctx.who.login);
    // Calendar and contacts apps start here and ask who they are.
    let principal = format!(
        "<d:href>{}</d:href>",
        crate::cloud::caldav::principal_href(&ctx.who.login)
    );
    let collection = |href: String| {
        let mut r = Response::new(href);
        r.found
            .push((Name::new(DAV, "resourcetype"), "<d:collection/>".into()));
        if let PropRequest::Props(names) = &preq {
            for name in names {
                if name.is(DAV, "current-user-principal") || name.is(DAV, "principal-URL") {
                    r.found.push((name.clone(), principal.clone()));
                } else if !name.is(DAV, "resourcetype") {
                    r.missing.push(name.clone());
                }
            }
        }
        r
    };
    let mut out = Vec::new();
    match t.rest.as_str() {
        "" => {
            out.push(collection("/remote.php/dav/".into()));
            if depth(ctx.req) != Some(0) {
                for kind in [
                    "files",
                    "uploads",
                    "trashbin",
                    "versions",
                    "principals",
                    "calendars",
                    "addressbooks",
                ] {
                    out.push(collection(format!("/remote.php/dav/{kind}/")));
                }
            }
        }
        kind => {
            out.push(collection(format!("/remote.php/dav/{kind}/")));
            if depth(ctx.req) != Some(0) {
                out.push(collection(format!("/remote.php/dav/{kind}/{login}/")));
            }
        }
    }
    multistatus(&out)
}

async fn proppatch(ctx: &Ctx<'_>, t: &Target, body: web::Payload) -> HttpResponse {
    let bytes = match read_body(body, MAX_XML).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    let changes = match xml::parse_proppatch(&String::from_utf8_lossy(&bytes)) {
        Ok(c) => c,
        Err(e) => {
            return dav_error(
                StatusCode::BAD_REQUEST,
                "Sabre\\DAV\\Exception\\BadRequest",
                &e.to_string(),
            )
        }
    };
    let store = &ctx.cloud.store;
    let mut entry = match store.stat(&t.owner, &t.rest) {
        Ok(e) => e,
        Err(e) => return store_error(&e),
    };
    if ctx.locked(&t.owner, &t.rest) {
        return Ctx::locked_response();
    }
    let mut r = Response::new(t.href(&entry.path, entry.dir));
    for change in changes {
        let (name, value) = match &change {
            PropChange::Set(el) => (el.name.clone(), Some(el)),
            PropChange::Remove(n) => (n.clone(), None),
        };
        let result: Result<(), &'static str> = match (name.ns.as_str(), name.local.as_str()) {
            (OC, "favorite") => {
                let on = value.is_some_and(|v| v.deep_text() == "1" || v.deep_text() == "true");
                match store.set_favorite(&t.owner, &t.rest, on) {
                    Ok(e) => {
                        entry = e;
                        Ok(())
                    }
                    Err(_) => Err("500 Internal Server Error"),
                }
            }
            (DAV, "lastmodified") | (DAV, "getlastmodified") => {
                let parsed = value.map(|v| v.deep_text()).and_then(|s| {
                    s.parse::<i64>().ok().or_else(|| {
                        httpdate::parse_http_date(&s).ok().map(|t| {
                            t.duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs() as i64)
                                .unwrap_or(0)
                        })
                    })
                });
                match parsed {
                    Some(m) => match crate::cloud::set_mtime(ctx.cloud, &t.owner, &t.rest, m) {
                        Ok(e) => {
                            entry = e;
                            Ok(())
                        }
                        Err(_) => Err("500 Internal Server Error"),
                    },
                    None => Err("400 Bad Request"),
                }
            }
            (DAV, _) | (OC, _) | (NC, _) | (SABRE, _) => Err("403 Forbidden"),
            _ => {
                let id = entry.id;
                let res = store.with_db(|db| {
                    match value {
                        Some(v) => db.execute(
                            "INSERT INTO props (file, ns, name, value) VALUES (?1, ?2, ?3, ?4)
                             ON CONFLICT (file, ns, name) DO UPDATE SET value = excluded.value",
                            params![
                                id,
                                name.ns,
                                name.local,
                                v.inner_xml.chars().take(65536).collect::<String>()
                            ],
                        )?,
                        None => db.execute(
                            "DELETE FROM props WHERE file = ?1 AND ns = ?2 AND name = ?3",
                            params![id, name.ns, name.local],
                        )?,
                    };
                    Ok(())
                });
                res.map_err(|_| "500 Internal Server Error")
            }
        };
        match result {
            Ok(()) => r.found.push((name, String::new())),
            Err(status) => r.other.push((name, status)),
        }
    }
    // One failure fails the lot (RFC 4918 9.2): the rest are reported as
    // not done. The writes above are each harmless to keep, so they are.
    if !r.other.is_empty() && !r.found.is_empty() {
        for (n, _) in std::mem::take(&mut r.found) {
            r.other.push((n, "424 Failed Dependency"));
        }
    }
    multistatus(&[r])
}

fn get(ctx: &Ctx<'_>, t: &Target) -> HttpResponse {
    let e = match ctx.cloud.store.stat(&t.owner, &t.rest) {
        Ok(e) => e,
        Err(e) => return store_error(&e),
    };
    if e.dir {
        return HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .insert_header((header::ETAG, quoted(&e.etag)))
            .body("This is a folder of LosOS Cloud. Open it with a WebDAV client.\n");
    }
    serve_file(
        ctx.req,
        &ctx.cloud.store.disk_path(&t.owner, &t.rest),
        &e.name,
        &e.mime,
        Some(&e.etag),
        e.mtime,
        Some(ctx.cloud.store.oc_id(e.id)),
        false,
    )
}

/// The bytes of a file, with ranges and conditional requests. Never shown
/// as a page of the box's own origin: a file is the owner's, its script
/// is not to run here, so it is sandboxed and active types are downloads.
#[allow(clippy::too_many_arguments)]
pub fn serve_file(
    req: &HttpRequest,
    disk: &std::path::Path,
    name: &str,
    mime: &str,
    etag: Option<&str>,
    mtime: i64,
    file_id: Option<String>,
    download: bool,
) -> HttpResponse {
    if let (Some(tag), Some(inm)) = (etag, header_str(req, "if-none-match")) {
        if inm
            .split(',')
            .any(|v| bare_etag(v) == tag || v.trim() == "*")
        {
            return HttpResponse::NotModified()
                .insert_header((header::ETAG, quoted(tag)))
                .finish();
        }
    }
    let file = match actix_files::NamedFile::open(disk) {
        Ok(f) => f,
        Err(e) => return store_error(&StoreError::from(e)),
    };
    let active = matches!(
        mime,
        "text/html" | "image/svg+xml" | "application/xml" | "text/javascript" | "text/xml"
    ) || mime.contains("xhtml");
    let disposition = actix_web::http::header::ContentDisposition {
        disposition: if download || active {
            actix_web::http::header::DispositionType::Attachment
        } else {
            actix_web::http::header::DispositionType::Inline
        },
        parameters: vec![actix_web::http::header::DispositionParam::FilenameExt(
            actix_web::http::header::ExtendedValue {
                charset: actix_web::http::header::Charset::Ext("UTF-8".into()),
                language_tag: None,
                value: name.as_bytes().to_vec(),
            },
        )],
    };
    let content_type: mime::Mime = if active {
        mime::APPLICATION_OCTET_STREAM
    } else {
        mime.parse().unwrap_or(mime::APPLICATION_OCTET_STREAM)
    };
    let mut resp = file
        .use_etag(false)
        .use_last_modified(false)
        .set_content_type(content_type)
        .set_content_disposition(disposition)
        .into_response(req);
    let h = resp.headers_mut();
    if let Some(tag) = etag {
        if let Ok(v) = quoted(tag).parse() {
            h.insert(header::ETAG, v);
        }
    }
    if let Ok(v) = http_date(mtime).parse() {
        h.insert(header::LAST_MODIFIED, v);
    }
    if let Some(id) = file_id {
        if let Ok(v) = id.parse() {
            h.insert(header::HeaderName::from_static("oc-fileid"), v);
        }
    }
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        header::HeaderValue::from_static("default-src 'none'; sandbox"),
    );
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        header::HeaderValue::from_static("nosniff"),
    );
    resp
}

/// How much of a body is gathered before it goes to disk in one write.
/// actix hands a body over in pieces of a few KiB; writing each one on its
/// own costs a system call per piece on the worker that also reads the
/// network. Two batches at most are held per upload: one filling, one being
/// written.
pub const BATCH: usize = 1 << 20;

/// Stream a request body into a temporary file beside the owner's files,
/// with its SHA-1. `expected` is the Content-Length, which a body cut short
/// must not pass for a whole one.
///
/// The body is written in batches of [`BATCH`] on the blocking pool, so the
/// next batch arrives while the last one is written. A body smaller than one
/// batch is written once, here.
pub async fn receive(
    cloud: &Cloud,
    owner: &str,
    mut body: web::Payload,
    expected: Option<u64>,
) -> Result<(tempfile::NamedTempFile, String), HttpResponse> {
    let fail = |e: std::io::Error| store_error(&StoreError::from(e));
    let mut file = Some(cloud.store.temp_file(owner).map_err(|e| store_error(&e))?);
    let mut writing: Option<WriteBatch> = None;
    let mut batch = web::BytesMut::new();
    let mut sha = ring::digest::Context::new(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY);
    let mut n: u64 = 0;
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|_| {
            dav_error(
                StatusCode::BAD_REQUEST,
                "Sabre\\DAV\\Exception\\BadRequest",
                "the upload was cut short",
            )
        })?;
        sha.update(&chunk);
        n += chunk.len() as u64;
        if batch.is_empty() {
            batch.reserve(BATCH);
        }
        batch.extend_from_slice(&chunk);
        if batch.len() >= BATCH {
            let full = batch.split().freeze();
            let f = match writing.take() {
                Some(w) => w.done().await.map_err(fail)?,
                None => file.take().expect("the temporary file"),
            };
            writing = Some(WriteBatch::start(f, full));
        }
    }
    let mut temp = match writing.take() {
        Some(w) => {
            let mut f = w.done().await.map_err(fail)?;
            if !batch.is_empty() {
                let rest = batch.freeze();
                f = WriteBatch::start(f, rest).done().await.map_err(fail)?;
            }
            f
        }
        None => {
            let mut f = file.take().expect("the temporary file");
            f.write_all(&batch).map_err(fail)?;
            f
        }
    };
    if let Some(len) = expected {
        if n != len {
            return Err(dav_error(
                StatusCode::BAD_REQUEST,
                "Sabre\\DAV\\Exception\\BadRequest",
                &format!("expected {len} bytes, received {n}"),
            ));
        }
    }
    temp.flush().map_err(fail)?;
    Ok((temp, crate::cloud::util::hex(sha.finish().as_ref())))
}

/// A batch on its way to disk, handing the file back when it is written.
struct WriteBatch(actix_web::rt::task::JoinHandle<std::io::Result<tempfile::NamedTempFile>>);

impl WriteBatch {
    fn start(mut f: tempfile::NamedTempFile, bytes: web::Bytes) -> WriteBatch {
        WriteBatch(actix_web::rt::task::spawn_blocking(move || {
            f.write_all(&bytes)?;
            Ok(f)
        }))
    }

    async fn done(self) -> std::io::Result<tempfile::NamedTempFile> {
        self.0.await.map_err(std::io::Error::other)?
    }
}

fn content_length(req: &HttpRequest) -> Option<u64> {
    header_str(req, "content-length").and_then(|v| v.trim().parse().ok())
}

async fn put(ctx: &Ctx<'_>, t: &Target, body: web::Payload) -> HttpResponse {
    if t.rest.is_empty() {
        return method_not_allowed();
    }
    if ctx.req.headers().contains_key(header::CONTENT_RANGE) {
        return dav_error(
            StatusCode::BAD_REQUEST,
            "Sabre\\DAV\\Exception\\BadRequest",
            "partial PUT is not supported",
        );
    }
    if ctx.locked(&t.owner, &t.rest) {
        return Ctx::locked_response();
    }
    // Refuse early what would be refused after the whole upload.
    match ctx
        .cloud
        .store
        .stat(&t.owner, parent_of(&t.rest).unwrap_or(""))
    {
        Ok(p) if p.dir => {}
        _ => return store_error(&StoreError::Conflict),
    }
    let expected = header_str(ctx.req, "oc-total-length")
        .and_then(|v| v.parse().ok())
        .or_else(|| content_length(ctx.req));
    let (temp, sha) = match receive(ctx.cloud, &t.owner, body, expected).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    commit_upload(ctx, t, &t.rest, temp, sha)
}

/// Check the client's checksum, then commit. Shared by PUT and the final
/// MOVE of a chunked upload.
fn commit_upload(
    ctx: &Ctx<'_>,
    t: &Target,
    path: &str,
    temp: tempfile::NamedTempFile,
    sha: String,
) -> HttpResponse {
    let ours = format!("SHA1:{sha}");
    let checksum = match client_checksum(ctx.req) {
        Some(c) if c.starts_with("SHA1:") && c != ours => {
            return dav_error(
                StatusCode::BAD_REQUEST,
                "Sabre\\DAV\\Exception\\BadRequest",
                "the checksum does not match what arrived",
            )
        }
        _ => ours,
    };
    let if_match = header_str(ctx.req, "if-match").map(bare_etag);
    let none_match = header_str(ctx.req, "if-none-match").is_some_and(|v| v.trim() == "*");
    let mtime = client_mtime(ctx.req);
    match ctx.cloud.store.commit_file(
        &t.owner,
        path,
        temp,
        mtime,
        if_match,
        none_match,
        Some(checksum),
    ) {
        Ok(Written { entry, created }) => {
            ctx.cloud.changed(&t.owner, &entry);
            written(
                if created {
                    StatusCode::CREATED
                } else {
                    StatusCode::NO_CONTENT
                },
                ctx.cloud,
                &entry,
                mtime.is_some(),
            )
        }
        Err(e) => store_error(&e),
    }
}

async fn mkcol(ctx: &Ctx<'_>, t: &Target, body: web::Payload) -> HttpResponse {
    match read_body(body, MAX_XML).await {
        Ok(b) if b.is_empty() => {}
        Ok(_) => {
            return dav_error(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "Sabre\\DAV\\Exception\\UnsupportedMediaType",
                "extended MKCOL is not supported",
            )
        }
        Err(r) => return r,
    }
    if ctx.locked(&t.owner, &t.rest) {
        return Ctx::locked_response();
    }
    match ctx.cloud.store.mkdir(&t.owner, &t.rest) {
        Ok(e) => written(StatusCode::CREATED, ctx.cloud, &e, false),
        Err(e) => store_error(&e),
    }
}

fn delete(ctx: &Ctx<'_>, t: &Target) -> HttpResponse {
    if t.rest.is_empty() {
        return dav_error(
            StatusCode::FORBIDDEN,
            "Sabre\\DAV\\Exception\\Forbidden",
            "the home folder cannot be deleted",
        );
    }
    if ctx.locked(&t.owner, &t.rest) {
        return Ctx::locked_response();
    }
    let if_match = header_str(ctx.req, "if-match").map(bare_etag);
    match ctx.cloud.store.delete(&t.owner, &t.rest, if_match) {
        Ok(_) => {
            ctx.cloud.locks.forget(&t.owner, &t.rest);
            HttpResponse::NoContent().finish()
        }
        Err(e) => store_error(&e),
    }
}

/// The target of a `Destination` header, in the same space and for the same
/// person, or the response that refuses it.
fn destination(ctx: &Ctx<'_>, space: Space) -> Result<Target, HttpResponse> {
    let bad = |m: &str| {
        dav_error(
            StatusCode::BAD_REQUEST,
            "Sabre\\DAV\\Exception\\BadRequest",
            m,
        )
    };
    let dest = header_str(ctx.req, "destination").ok_or_else(|| bad("no Destination"))?;
    let path = destination_path(dest).ok_or_else(|| bad("a Destination that is not a URL"))?;
    match resolve(path, &ctx.who.login) {
        Resolve::Ok(d) if d.space == space => Ok(d),
        Resolve::Ok(_) | Resolve::NotFound => Err(dav_error(
            StatusCode::BAD_GATEWAY,
            "Sabre\\DAV\\Exception\\BadGateway",
            "the Destination is not in this space",
        )),
        Resolve::Forbidden => Err(dav_error(
            StatusCode::FORBIDDEN,
            "Sabre\\DAV\\Exception\\Forbidden",
            "not your files",
        )),
        Resolve::BadPath => Err(bad("a name the cloud cannot hold")),
    }
}

fn overwrite(req: &HttpRequest) -> bool {
    !header_str(req, "overwrite").is_some_and(|v| v.trim().eq_ignore_ascii_case("f"))
}

fn move_or_copy(ctx: &Ctx<'_>, t: &Target, is_move: bool) -> HttpResponse {
    let d = match destination(ctx, Space::Files) {
        Ok(d) => d,
        Err(r) => return r,
    };
    if t.rest.is_empty() || d.rest.is_empty() {
        return dav_error(
            StatusCode::FORBIDDEN,
            "Sabre\\DAV\\Exception\\Forbidden",
            "the home folder cannot be moved",
        );
    }
    if (is_move && ctx.locked(&t.owner, &t.rest)) || ctx.locked(&d.owner, &d.rest) {
        return Ctx::locked_response();
    }
    let store = &ctx.cloud.store;
    let r = if is_move {
        store.rename(&t.owner, &t.rest, &d.rest, overwrite(ctx.req))
    } else {
        store.copy(&t.owner, &t.rest, &d.rest, overwrite(ctx.req))
    };
    match r {
        Ok(Written { entry, created }) => {
            if is_move {
                ctx.cloud.locks.forget(&t.owner, &t.rest);
            }
            let mtime = client_mtime(ctx.req);
            let entry = match mtime {
                Some(m) if !entry.dir => {
                    crate::cloud::set_mtime(ctx.cloud, &d.owner, &d.rest, m).unwrap_or(entry)
                }
                _ => entry,
            };
            written(
                if created {
                    StatusCode::CREATED
                } else {
                    StatusCode::NO_CONTENT
                },
                ctx.cloud,
                &entry,
                mtime.is_some(),
            )
        }
        Err(e) => store_error(&e),
    }
}

fn lock_timeout(req: &HttpRequest) -> u64 {
    header_str(req, "timeout")
        .and_then(|v| {
            v.split(',')
                .filter_map(|p| p.trim().strip_prefix("Second-"))
                .find_map(|s| s.parse::<u64>().ok())
        })
        .unwrap_or(MAX_LOCK)
        .clamp(1, MAX_LOCK)
}

fn lock_answer(status: StatusCode, l: &Lock, href: &str) -> HttpResponse {
    let body = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<d:prop xmlns:d="DAV:"><d:lockdiscovery>{}</d:lockdiscovery></d:prop>"#,
        active_lock(l, href)
    );
    HttpResponse::build(status)
        .insert_header(("Lock-Token", format!("<{}>", l.token)))
        .content_type("application/xml; charset=utf-8")
        .body(body)
}

async fn lock(ctx: &Ctx<'_>, t: &Target, body: web::Payload) -> HttpResponse {
    let bytes = match read_body(body, MAX_XML).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    let timeout = lock_timeout(ctx.req);
    let href = t.href(&t.rest, false);
    if bytes.iter().all(u8::is_ascii_whitespace) {
        // A refresh names its lock in the If header.
        return match ctx.cloud.locks.refresh(&t.owner, ctx.if_header(), timeout) {
            Some((p, l)) => lock_answer(StatusCode::OK, &l, &t.href(&p, false)),
            None => dav_error(
                StatusCode::PRECONDITION_FAILED,
                "Sabre\\DAV\\Exception\\PreconditionFailed",
                "no lock to refresh",
            ),
        };
    }
    let info = match xml::parse(&String::from_utf8_lossy(&bytes)) {
        Ok(i) if i.is(DAV, "lockinfo") => i,
        _ => {
            return dav_error(
                StatusCode::BAD_REQUEST,
                "Sabre\\DAV\\Exception\\BadRequest",
                "not a lockinfo",
            )
        }
    };
    let exclusive = info
        .find(&[(DAV, "lockscope"), (DAV, "exclusive")])
        .is_some();
    let owner_xml = info
        .child(DAV, "owner")
        .map(|o| o.inner_xml.chars().take(4096).collect())
        .unwrap_or_default();
    let existing = ctx.cloud.locks.active(&t.owner, &t.rest);
    if existing.iter().any(|(_, l)| l.exclusive || exclusive) {
        return Ctx::locked_response();
    }
    // Locking a name with nothing behind it makes an empty file there, the
    // way the Finder and Windows start a new file.
    let mut status = StatusCode::OK;
    match ctx.cloud.store.stat(&t.owner, &t.rest) {
        Ok(_) => {}
        Err(StoreError::NotFound) if !t.rest.is_empty() => {
            match ctx.cloud.store.write_bytes(&t.owner, &t.rest, b"") {
                Ok(w) => {
                    ctx.cloud.changed(&t.owner, &w.entry);
                    status = StatusCode::CREATED;
                }
                Err(e) => return store_error(&e),
            }
        }
        Err(e) => return store_error(&e),
    }
    let l = Lock {
        token: format!("opaquelocktoken:{}", uuid_like()),
        owner_xml,
        exclusive,
        infinite: !header_str(ctx.req, "depth").is_some_and(|d| d.trim() == "0"),
        until: Instant::now() + Duration::from_secs(timeout),
        timeout,
    };
    ctx.cloud.locks.put(&t.owner, &t.rest, l.clone());
    lock_answer(status, &l, &href)
}

fn uuid_like() -> String {
    let t = random_token(32).to_lowercase();
    format!(
        "{}-{}-{}-{}-{}",
        &t[0..8],
        &t[8..12],
        &t[12..16],
        &t[16..20],
        &t[20..32]
    )
}

fn unlock(ctx: &Ctx<'_>, t: &Target) -> HttpResponse {
    let token = header_str(ctx.req, "lock-token")
        .unwrap_or_default()
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .to_owned();
    if ctx.cloud.locks.take(&t.owner, &t.rest, &token) {
        HttpResponse::NoContent().finish()
    } else {
        dav_error(
            StatusCode::CONFLICT,
            "Sabre\\DAV\\Exception\\Conflict",
            "no such lock here",
        )
    }
}

/// `oc:filter-files`: the person's favourites, as the phone apps ask.
async fn report(ctx: &Ctx<'_>, t: &Target, body: web::Payload) -> HttpResponse {
    let bytes = match read_body(body, MAX_XML).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    let root = match xml::parse(&String::from_utf8_lossy(&bytes)) {
        Ok(r) => r,
        Err(e) => {
            return dav_error(
                StatusCode::BAD_REQUEST,
                "Sabre\\DAV\\Exception\\BadRequest",
                &e.to_string(),
            )
        }
    };
    if !root.is(OC, "filter-files") {
        return dav_error(
            StatusCode::NOT_IMPLEMENTED,
            "Sabre\\DAV\\Exception\\ReportNotSupported",
            "no such report",
        );
    }
    let props = root
        .child(DAV, "prop")
        .map(|p| PropRequest::Props(p.children.iter().map(|c| c.name.clone()).collect()))
        .unwrap_or(PropRequest::AllProp(Vec::new()));
    let favorites_only = root
        .find(&[(OC, "filter-rules"), (OC, "favorite")])
        .is_some_and(|f| f.deep_text() == "1");
    let entries = if favorites_only {
        ctx.cloud.store.favorites(&t.owner)
    } else {
        ctx.cloud.store.walk(&t.owner, &t.rest)
    };
    match entries {
        Ok(es) => {
            let responses: Vec<Response> = es
                .iter()
                .filter(|e| is_within(&e.path, &t.rest))
                .map(|e| ctx.entry_response(t, e, &props))
                .collect();
            multistatus(&responses)
        }
        Err(e) => store_error(&e),
    }
}

// ── Chunked uploads ─────────────────────────────────────────────────────────

fn transfer_dir(ctx: &Ctx<'_>, t: &Target) -> Result<(std::path::PathBuf, String), HttpResponse> {
    let transfer = t.rest.split('/').next().unwrap_or_default().to_owned();
    if transfer.is_empty() {
        return Err(method_not_allowed());
    }
    let dir = ctx
        .cloud
        .store
        .uploads_dir(&t.owner)
        .map_err(|e| store_error(&e))?
        .join(format!("t-{transfer}"));
    Ok((dir, transfer))
}

/// Chunk names in the order they are joined: by number where they are
/// numbers (`1`…`10000`, or Nextcloud's older zero-padded offsets), else by
/// name.
pub fn chunk_order(names: &mut [String]) {
    names.sort_by(|a, b| {
        let key = |s: &str| {
            s.split('-')
                .next()
                .and_then(|n| n.parse::<u64>().ok())
                .unwrap_or(u64::MAX)
        };
        key(a).cmp(&key(b)).then_with(|| a.cmp(b))
    });
}

async fn uploads(ctx: &Ctx<'_>, t: &Target, method: &str, body: web::Payload) -> HttpResponse {
    if t.rest.is_empty() {
        return match method {
            "PROPFIND" => {
                let mut r = Response::new(format!("{}/", t.base));
                r.found
                    .push((Name::new(DAV, "resourcetype"), "<d:collection/>".into()));
                multistatus(&[r])
            }
            _ => method_not_allowed(),
        };
    }
    let (dir, transfer) = match transfer_dir(ctx, t) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let chunk = t.rest.split_once('/').map(|(_, c)| c.to_owned());
    match (method, chunk.as_deref()) {
        ("MKCOL", None) => {
            if dir.exists() {
                return method_not_allowed();
            }
            if let Err(e) = std::fs::create_dir_all(&dir) {
                return store_error(&e.into());
            }
            if let Some(d) = header_str(ctx.req, "destination") {
                let _ = std::fs::write(dir.join(".destination"), d);
            }
            HttpResponse::Created().finish()
        }
        ("PUT", Some(name)) if !name.contains('/') && !name.starts_with('.') => {
            if !dir.is_dir() {
                return store_error(&StoreError::Conflict);
            }
            let expected = content_length(ctx.req);
            let (temp, _) = match receive(ctx.cloud, &t.owner, body, expected).await {
                Ok(v) => v,
                Err(r) => return r,
            };
            let target = dir.join(name);
            let existed = target.exists();
            if let Err(e) = temp.persist(&target) {
                return store_error(&e.error.into());
            }
            if existed {
                HttpResponse::NoContent().finish()
            } else {
                HttpResponse::Created().finish()
            }
        }
        ("PROPFIND", None) => {
            if !dir.is_dir() {
                return store_error(&StoreError::NotFound);
            }
            let mut out = Vec::new();
            let mut r = Response::new(format!("{}/{}/", t.base, encode_path(&transfer)));
            r.found
                .push((Name::new(DAV, "resourcetype"), "<d:collection/>".into()));
            out.push(r);
            if depth(ctx.req) != Some(0) {
                let mut names = chunk_names(&dir);
                chunk_order(&mut names);
                for n in names {
                    let size = std::fs::metadata(dir.join(&n))
                        .map(|m| m.len())
                        .unwrap_or(0);
                    let mut r = Response::new(format!(
                        "{}/{}/{}",
                        t.base,
                        encode_path(&transfer),
                        encode_path(&n)
                    ));
                    r.found
                        .push((Name::new(DAV, "resourcetype"), String::new()));
                    r.found
                        .push((Name::new(DAV, "getcontentlength"), size.to_string()));
                    out.push(r);
                }
            }
            multistatus(&out)
        }
        ("MOVE", Some(".file")) => {
            if !dir.is_dir() {
                return store_error(&StoreError::NotFound);
            }
            let d = match destination(ctx, Space::Files) {
                Ok(d) => d,
                Err(r) => return r,
            };
            if d.rest.is_empty() {
                return store_error(&StoreError::Conflict);
            }
            if ctx.locked(&d.owner, &d.rest) {
                return Ctx::locked_response();
            }
            let total = header_str(ctx.req, "oc-total-length").and_then(|v| v.parse::<u64>().ok());
            let joined = match assemble(ctx, &t.owner, &dir, total).await {
                Ok(v) => v,
                Err(r) => return r,
            };
            let r = commit_upload(ctx, &d, &d.rest, joined.0, joined.1);
            if r.status().is_success() {
                let _ = std::fs::remove_dir_all(&dir);
            }
            r
        }
        ("DELETE", None) => {
            let _ = std::fs::remove_dir_all(&dir);
            HttpResponse::NoContent().finish()
        }
        ("OPTIONS", _) => options(),
        _ => method_not_allowed(),
    }
}

fn chunk_names(dir: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| e.file_name().to_str().map(str::to_owned))
                .filter(|n| !n.starts_with('.'))
                .collect()
        })
        .unwrap_or_default()
}

async fn assemble(
    ctx: &Ctx<'_>,
    owner: &str,
    dir: &std::path::Path,
    total: Option<u64>,
) -> Result<(tempfile::NamedTempFile, String), HttpResponse> {
    let mut names = chunk_names(dir);
    chunk_order(&mut names);
    let temp = ctx
        .cloud
        .store
        .temp_file(owner)
        .map_err(|e| store_error(&e))?;
    // Gigabytes for a large file: off the worker, in batches.
    let dir = dir.to_path_buf();
    let joined = actix_web::rt::task::spawn_blocking(move || {
        let mut temp = temp;
        let mut sha = ring::digest::Context::new(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY);
        let mut n: u64 = 0;
        let mut buf = vec![0u8; BATCH];
        for name in &names {
            let mut f = std::fs::File::open(dir.join(name))?;
            loop {
                use std::io::Read;
                let k = f.read(&mut buf)?;
                if k == 0 {
                    break;
                }
                sha.update(&buf[..k]);
                temp.write_all(&buf[..k])?;
                n += k as u64;
            }
        }
        temp.flush()?;
        Ok::<_, std::io::Error>((temp, sha.finish(), n))
    })
    .await
    .map_err(std::io::Error::other)
    .and_then(|r| r);
    let (temp, sha, n) = joined.map_err(|e| store_error(&e.into()))?;
    if let Some(total) = total {
        if total != n {
            return Err(dav_error(
                StatusCode::BAD_REQUEST,
                "Sabre\\DAV\\Exception\\BadRequest",
                &format!("the chunks hold {n} bytes, not {total}"),
            ));
        }
    }
    Ok((temp, crate::cloud::util::hex(sha.as_ref())))
}

/// Remove upload transfers nobody finished.
pub fn sweep_uploads(cloud: &Cloud) {
    let Ok(owners) = cloud.store.owners() else {
        return;
    };
    for owner in owners {
        let Ok(dir) = cloud.store.uploads_dir(&owner) else {
            continue;
        };
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let old = e
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|m| m.elapsed().ok())
                .is_some_and(|age| age > UPLOAD_TTL);
            if old {
                let p = e.path();
                let _ = if p.is_dir() {
                    std::fs::remove_dir_all(&p)
                } else {
                    std::fs::remove_file(&p)
                };
            }
        }
    }
}

// ── Trash bin ───────────────────────────────────────────────────────────────

async fn trash(ctx: &Ctx<'_>, t: &Target, method: &str, body: web::Payload) -> HttpResponse {
    let store = &ctx.cloud.store;
    let mut parts = t.rest.splitn(2, '/');
    let area = parts.next().unwrap_or_default();
    let item = parts.next().unwrap_or_default();
    match (method, area, item) {
        ("PROPFIND", "", _) => {
            let mut r = Response::new(format!("{}/", t.base));
            r.found
                .push((Name::new(DAV, "resourcetype"), "<d:collection/>".into()));
            let mut out = vec![r];
            if depth(ctx.req) != Some(0) {
                for a in ["trash", "restore"] {
                    let mut r = Response::new(format!("{}/{a}/", t.base));
                    r.found
                        .push((Name::new(DAV, "resourcetype"), "<d:collection/>".into()));
                    out.push(r);
                }
            }
            multistatus(&out)
        }
        ("PROPFIND", "trash", "") => {
            let req = match parse_request(body).await {
                Ok(r) => r,
                Err(r) => return r,
            };
            let items = match store.trash(&t.owner) {
                Ok(i) => i,
                Err(e) => return store_error(&e),
            };
            let mut r = Response::new(format!("{}/trash/", t.base));
            r.found
                .push((Name::new(DAV, "resourcetype"), "<d:collection/>".into()));
            let mut out = vec![r];
            if depth(ctx.req) != Some(0) {
                for i in &items {
                    out.push(trash_response(t, i, &req));
                }
            }
            multistatus(&out)
        }
        ("PROPFIND", "trash", stored) => {
            let req = match parse_request(body).await {
                Ok(r) => r,
                Err(r) => return r,
            };
            match store.trashed_by_stored(&t.owner, stored) {
                Ok(i) => multistatus(&[trash_response(t, &i, &req)]),
                Err(e) => store_error(&e),
            }
        }
        ("PROPFIND", "restore", _) => {
            let mut r = Response::new(format!("{}/restore/", t.base));
            r.found
                .push((Name::new(DAV, "resourcetype"), "<d:collection/>".into()));
            multistatus(&[r])
        }
        ("GET" | "HEAD", "trash", stored) if !stored.is_empty() && !stored.contains('/') => {
            match store.trashed_by_stored(&t.owner, stored) {
                Ok(i) if !i.dir => serve_file(
                    ctx.req,
                    &store.trash_disk_path(&t.owner, &i),
                    &i.name,
                    &mime_for(&i.name, false),
                    None,
                    i.deleted,
                    None,
                    true,
                ),
                Ok(_) => method_not_allowed(),
                Err(e) => store_error(&e),
            }
        }
        ("MOVE", "trash", stored) if !stored.is_empty() && !stored.contains('/') => {
            let d = match destination(ctx, Space::Trash) {
                Ok(d) => d,
                Err(r) => return r,
            };
            if !d.rest.starts_with("restore") {
                return method_not_allowed();
            }
            let i = match store.trashed_by_stored(&t.owner, stored) {
                Ok(i) => i,
                Err(e) => return store_error(&e),
            };
            match store.restore(&t.owner, i.id, None) {
                Ok(e) => {
                    ctx.cloud.changed(&t.owner, &e);
                    HttpResponse::Created().finish()
                }
                Err(e) => store_error(&e),
            }
        }
        ("DELETE", "trash", "") => match store.empty_trash(&t.owner) {
            Ok(_) => HttpResponse::NoContent().finish(),
            Err(e) => store_error(&e),
        },
        ("DELETE", "trash", stored) if !stored.contains('/') => {
            match store.trashed_by_stored(&t.owner, stored) {
                Ok(i) => match store.purge(&t.owner, i.id) {
                    Ok(()) => HttpResponse::NoContent().finish(),
                    Err(e) => store_error(&e),
                },
                Err(e) => store_error(&e),
            }
        }
        ("OPTIONS", _, _) => options(),
        _ => method_not_allowed(),
    }
}

fn trash_response(t: &Target, i: &crate::cloud::store::Trashed, req: &PropRequest) -> Response {
    let mut r = Response::new(format!(
        "{}/trash/{}{}",
        t.base,
        encode_path(&i.stored),
        if i.dir { "/" } else { "" }
    ));
    let value = |n: &Name| -> Option<String> {
        match (n.ns.as_str(), n.local.as_str()) {
            (DAV, "resourcetype") => Some(if i.dir {
                "<d:collection/>".into()
            } else {
                String::new()
            }),
            (DAV, "getcontentlength") if !i.dir => Some(i.size.to_string()),
            (DAV, "getcontenttype") if !i.dir => Some(xml_escape(&mime_for(&i.name, false))),
            (DAV, "getlastmodified") => Some(http_date(i.deleted)),
            (DAV, "getetag") => Some(xml_escape(&quoted(&format!("t{}", i.id)))),
            (DAV, "displayname") => Some(xml_escape(&i.name)),
            (OC, "size") => Some(i.size.to_string()),
            (OC, "fileid") => Some(i.id.to_string()),
            (OC, "id") => Some(i.id.to_string()),
            (OC, "permissions") => Some("GD".into()),
            (NC, "trashbin-filename") => Some(xml_escape(&i.name)),
            (NC, "trashbin-original-location") => Some(xml_escape(&i.original)),
            (NC, "trashbin-deletion-time") => Some(i.deleted.to_string()),
            (NC, "has-preview") => Some("false".into()),
            _ => None,
        }
    };
    let names: Vec<Name> = match req {
        PropRequest::Props(n) => n.clone(),
        _ => [
            (DAV, "resourcetype"),
            (DAV, "getcontentlength"),
            (DAV, "getlastmodified"),
            (NC, "trashbin-filename"),
            (NC, "trashbin-original-location"),
            (NC, "trashbin-deletion-time"),
        ]
        .iter()
        .map(|(ns, l)| Name::new(ns, l))
        .collect(),
    };
    for n in names {
        match value(&n) {
            Some(v) => r.found.push((n, v)),
            None => r.missing.push(n),
        }
    }
    r
}

// ── Versions ────────────────────────────────────────────────────────────────

async fn versions(ctx: &Ctx<'_>, t: &Target, method: &str, body: web::Payload) -> HttpResponse {
    let store = &ctx.cloud.store;
    let parts: Vec<&str> = t.rest.split('/').filter(|p| !p.is_empty()).collect();
    match (method, parts.as_slice()) {
        ("PROPFIND", ["versions", id]) => {
            let req = match parse_request(body).await {
                Ok(r) => r,
                Err(r) => return r,
            };
            let Ok(id) = id.parse::<i64>() else {
                return store_error(&StoreError::NotFound);
            };
            if let Err(e) = store.by_id(&t.owner, id) {
                return store_error(&e);
            }
            let list = match store.versions_of(&t.owner, id) {
                Ok(l) => l,
                Err(e) => return store_error(&e),
            };
            let mut r = Response::new(format!("{}/versions/{id}/", t.base));
            r.found
                .push((Name::new(DAV, "resourcetype"), "<d:collection/>".into()));
            let mut out = vec![r];
            if depth(ctx.req) != Some(0) {
                for v in list {
                    let mut r = Response::new(format!("{}/versions/{id}/{}", t.base, v.mtime));
                    let names: Vec<Name> = match &req {
                        PropRequest::Props(n) => n.clone(),
                        _ => vec![
                            Name::new(DAV, "getcontentlength"),
                            Name::new(DAV, "getlastmodified"),
                            Name::new(DAV, "getetag"),
                        ],
                    };
                    for n in names {
                        let value = match (n.ns.as_str(), n.local.as_str()) {
                            (DAV, "resourcetype") => Some(String::new()),
                            (DAV, "getcontentlength") => Some(v.size.to_string()),
                            (DAV, "getlastmodified") => Some(http_date(v.mtime)),
                            (DAV, "getetag") => {
                                Some(xml_escape(&quoted(&format!("v{id}-{}", v.mtime))))
                            }
                            (NC, "has-preview") => Some("false".into()),
                            (NC, "version-label") => Some(String::new()),
                            _ => None,
                        };
                        match value {
                            Some(val) => r.found.push((n, val)),
                            None => r.missing.push(n),
                        }
                    }
                    out.push(r);
                }
            }
            multistatus(&out)
        }
        ("GET" | "HEAD", ["versions", id, mtime]) => {
            let (Ok(id), Ok(mtime)) = (id.parse::<i64>(), mtime.parse::<i64>()) else {
                return store_error(&StoreError::NotFound);
            };
            let e = match store.by_id(&t.owner, id) {
                Ok(e) => e,
                Err(e) => return store_error(&e),
            };
            match store.version_path(&t.owner, id, mtime) {
                Ok(p) => serve_file(ctx.req, &p, &e.name, &e.mime, None, mtime, None, true),
                Err(e) => store_error(&e),
            }
        }
        ("MOVE", ["versions", id, mtime]) => {
            let (Ok(id), Ok(mtime)) = (id.parse::<i64>(), mtime.parse::<i64>()) else {
                return store_error(&StoreError::NotFound);
            };
            match store.restore_version(&t.owner, id, mtime) {
                Ok(e) => {
                    ctx.cloud.changed(&t.owner, &e);
                    HttpResponse::NoContent().finish()
                }
                Err(e) => store_error(&e),
            }
        }
        ("PROPFIND", _) => {
            let mut r = Response::new(format!("{}/{}", t.base, encode_path(&t.rest)));
            r.found
                .push((Name::new(DAV, "resourcetype"), "<d:collection/>".into()));
            multistatus(&[r])
        }
        ("OPTIONS", _) => options(),
        _ => method_not_allowed(),
    }
}

/// Kept for the server to tell WebDAV methods from the rest.
pub fn is_dav_method(m: &Method) -> bool {
    matches!(
        m.as_str(),
        "PROPFIND"
            | "PROPPATCH"
            | "MKCOL"
            | "COPY"
            | "MOVE"
            | "LOCK"
            | "UNLOCK"
            | "REPORT"
            | "SEARCH"
    )
}

/// A fresh transfer id, for tests and the web app's own large uploads.
pub fn new_transfer_id() -> String {
    format!("web-{}-{}", now(), random_token(12))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_resolve_to_spaces() {
        match resolve("/remote.php/webdav/a%20b/c.txt", "alice") {
            Resolve::Ok(t) => {
                assert_eq!(t.space, Space::Files);
                assert_eq!(t.rest, "a b/c.txt");
                assert_eq!(t.href("a b/c.txt", false), "/remote.php/webdav/a%20b/c.txt");
            }
            r => panic!("{r:?}"),
        }
        match resolve("/remote.php/dav/files/alice/", "alice") {
            Resolve::Ok(t) => {
                assert_eq!(t.rest, "");
                assert_eq!(t.href("", true), "/remote.php/dav/files/alice/");
            }
            r => panic!("{r:?}"),
        }
        assert!(matches!(
            resolve("/remote.php/dav/files/bob/x", "alice"),
            Resolve::Forbidden
        ));
        assert!(matches!(
            resolve("/remote.php/dav/files/alice/../bob", "alice"),
            Resolve::BadPath
        ));
        assert!(matches!(
            resolve("/remote.php/dav/files/alice/a%2F..%2Fb", "alice"),
            Resolve::BadPath
        ));
        match resolve("/remote.php/dav/uploads/alice/123/00001", "alice") {
            Resolve::Ok(t) => {
                assert_eq!(t.space, Space::Uploads);
                assert_eq!(t.rest, "123/00001");
            }
            r => panic!("{r:?}"),
        }
        match resolve("/remote.php/dav/", "alice") {
            Resolve::Ok(t) => assert_eq!(t.space, Space::Root),
            r => panic!("{r:?}"),
        }
        assert!(matches!(
            resolve("/remote.php/other", "alice"),
            Resolve::NotFound
        ));
    }

    #[test]
    fn destinations_are_paths_of_urls() {
        assert_eq!(
            destination_path("https://box.local/remote.php/dav/files/a/x"),
            Some("/remote.php/dav/files/a/x")
        );
        assert_eq!(
            destination_path("/remote.php/webdav/x"),
            Some("/remote.php/webdav/x")
        );
        assert_eq!(destination_path("x"), None);
    }

    #[test]
    fn chunks_join_in_number_order() {
        let mut names: Vec<String> = ["10", "2", "1", "00000000000000000003-x"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        chunk_order(&mut names);
        assert_eq!(names, ["1", "2", "00000000000000000003-x", "10"]);
    }
}
