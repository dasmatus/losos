//! Knowledge bases: a folder of notes published as a site others read, at
//! `/kb/<name>/`, the way Obsidian Publish publishes a vault.
//!
//! The site reads the folder as it is now, so an edit shows on the next
//! page load and nothing is copied anywhere. Its owner picks who reads it:
//! anyone with the address, or people signed in to the box. A note whose
//! properties say `publish: false` or `private: true` is left out, along
//! with dot folders; every other file in the folder is the site's.
//!
//! Sites run only on a box that shares and has joined a mesh (the same
//! state in which chat and mail run): `losos-cloud serve
//! --knowledge-bases`, which modules/cloud.nix passes then and only then.
//! Off, the owner's list says so and every site answers 404; the list is
//! kept, so the sites come back with the mesh.

use super::{inside, vault, Vault};
use crate::cloud::api::{error, store_error};
use crate::cloud::auth::Who;
use crate::cloud::dav::{read_body, serve_file};
use crate::cloud::store::{join, Store, StoreError};
use crate::cloud::Cloud;
use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS knowledge_bases (
  id        INTEGER PRIMARY KEY AUTOINCREMENT,
  owner     TEXT    NOT NULL,
  folder_id INTEGER NOT NULL UNIQUE,
  slug      TEXT    NOT NULL UNIQUE,
  title     TEXT    NOT NULL,
  audience  TEXT    NOT NULL,
  created   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS knowledge_bases_owner ON knowledge_bases (owner);
"#;

pub fn init(store: &Store) -> Result<(), StoreError> {
    store.with_db(|db| Ok(db.execute_batch(SCHEMA)?))
}

/// Who may read a site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Audience {
    /// Anyone with the address, signed in or not.
    Anyone,
    /// People signed in to this box with LosOS Login.
    Box,
}

impl Audience {
    fn as_str(self) -> &'static str {
        match self {
            Audience::Anyone => "anyone",
            Audience::Box => "box",
        }
    }

    fn parse(s: &str) -> Audience {
        if s == "anyone" {
            Audience::Anyone
        } else {
            Audience::Box
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Site {
    pub id: i64,
    #[serde(skip)]
    pub owner: String,
    pub folder_id: i64,
    /// The folder now, relative to the owner's home; None once it is gone.
    pub folder: Option<String>,
    pub slug: String,
    pub title: String,
    pub audience: Audience,
    pub created: i64,
}

fn row_site(row: &rusqlite::Row<'_>) -> rusqlite::Result<Site> {
    Ok(Site {
        id: row.get(0)?,
        owner: row.get(1)?,
        folder_id: row.get(2)?,
        folder: None,
        slug: row.get(3)?,
        title: row.get(4)?,
        audience: Audience::parse(&row.get::<_, String>(5)?),
        created: row.get(6)?,
    })
}

const COLUMNS: &str = "id, owner, folder_id, slug, title, audience, created";

/// A site's address name from its title: lower case ASCII letters, digits
/// and dashes, at most 48 of them.
pub fn slug_for(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars().flat_map(fold) {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out: String = out.trim_matches('-').chars().take(48).collect();
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() {
        "notes".into()
    } else {
        out
    }
}

/// Accented letters to their plain ones, for the address.
fn fold(c: char) -> Vec<char> {
    const FROM: &str = "áàâäãåāăąçćčďđéèêëēėęěíìîïīįľĺłñńňóòôöõøōőŕřśšşťţúùûüūůűųýÿžźż";
    const TO: &str = "aaaaaaaaacccddeeeeeeeeiiiiiilllnnnoooooooorrsssttuuuuuuuuyyzzz";
    if c == 'ß' {
        return vec!['s', 's'];
    }
    match FROM
        .chars()
        .position(|f| f == c.to_lowercase().next().unwrap_or(c))
    {
        Some(i) => TO.chars().nth(i).into_iter().collect(),
        None => vec![c],
    }
}

/// A slug is ours to serve: what [`slug_for`] makes, nothing else.
fn valid_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 52
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !s.starts_with('-')
}

impl Site {
    fn with_folder(mut self, store: &Store) -> Site {
        self.folder = store
            .by_id(&self.owner, self.folder_id)
            .ok()
            .filter(|e| e.dir)
            .map(|e| e.path);
        self
    }
}

pub fn list(store: &Store, owner: &str) -> Result<Vec<Site>, StoreError> {
    let sites = store.with_db(|db| {
        let mut stmt = db.prepare(&format!(
            "SELECT {COLUMNS} FROM knowledge_bases WHERE owner = ?1 ORDER BY title COLLATE NOCASE"
        ))?;
        let rows = stmt.query_map(params![owner], row_site)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })?;
    Ok(sites.into_iter().map(|s| s.with_folder(store)).collect())
}

/// Every site on the box, by title, for the directory of wikis.
pub fn all(store: &Store) -> Result<Vec<Site>, StoreError> {
    let sites = store.with_db(|db| {
        let mut stmt = db.prepare(&format!(
            "SELECT {COLUMNS} FROM knowledge_bases ORDER BY title COLLATE NOCASE"
        ))?;
        let rows = stmt.query_map([], row_site)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })?;
    Ok(sites.into_iter().map(|s| s.with_folder(store)).collect())
}

/// What `GET /kb/api/` answers: the sites `who` may read (all of them for
/// someone signed in, the open ones for anyone else), each with its note
/// count and last change, and with `q` the lines of their notes that hold
/// it. Neither the owner nor the folder is named: a reader sees the site,
/// not whose files it is.
pub fn index(cloud: &Cloud, who: Option<&Who>, q: &str) -> serde_json::Value {
    let sites = all(&cloud.store).unwrap_or_default();
    let mut list = Vec::new();
    let mut hits = Vec::new();
    for site in sites {
        if site.folder.is_none() || (site.audience == Audience::Box && who.is_none()) {
            continue;
        }
        let Ok(v) = published(cloud, &site) else {
            continue;
        };
        let updated = v.notes.iter().map(|n| n.mtime).max();
        if q.trim().chars().count() >= 2 && hits.len() < 50 {
            for h in search(cloud, &site, &v, q).into_iter().take(10) {
                hits.push(json!({
                    "slug": site.slug,
                    "site": site.title,
                    "path": h.path,
                    "line": h.line,
                    "text": h.text,
                }));
            }
        }
        list.push(json!({
            "slug": site.slug,
            "title": site.title,
            "audience": site.audience,
            "notes": v.notes.len(),
            "updated": updated,
            "mine": who.is_some_and(|w| w.login == site.owner),
        }));
    }
    hits.truncate(50);
    json!({ "sites": list, "hits": hits })
}

pub fn by_slug(store: &Store, slug: &str) -> Result<Option<Site>, StoreError> {
    let site = store.with_db(|db| {
        Ok(db
            .query_row(
                &format!("SELECT {COLUMNS} FROM knowledge_bases WHERE slug = ?1"),
                params![slug],
                row_site,
            )
            .optional()?)
    })?;
    Ok(site.map(|s| s.with_folder(store)))
}

fn by_id(store: &Store, owner: &str, id: i64) -> Result<Site, StoreError> {
    let site = store.with_db(|db| {
        Ok(db
            .query_row(
                &format!("SELECT {COLUMNS} FROM knowledge_bases WHERE id = ?1 AND owner = ?2"),
                params![id, owner],
                row_site,
            )
            .optional()?)
    })?;
    site.map(|s| s.with_folder(store))
        .ok_or(StoreError::NotFound)
}

/// Publish `folder`. A folder is one site at most; a title already taken
/// by another site gets a number on its address.
pub fn create(
    store: &Store,
    owner: &str,
    folder: &str,
    title: &str,
    audience: Audience,
) -> Result<Site, StoreError> {
    let dir = store.stat(owner, folder)?;
    if !dir.dir {
        return Err(StoreError::Conflict);
    }
    let title = title.trim();
    let title = if title.is_empty() {
        if dir.name.is_empty() {
            "Notes".to_owned()
        } else {
            dir.name.clone()
        }
    } else {
        title.chars().take(120).collect()
    };
    let base = slug_for(&title);
    let id = store.with_db(|db| {
        let tx = db.transaction()?;
        let taken: bool = tx
            .query_row(
                "SELECT 1 FROM knowledge_bases WHERE folder_id = ?1",
                params![dir.id],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if taken {
            return Err(StoreError::Exists);
        }
        let mut slug = base.clone();
        let mut n = 1;
        while tx
            .query_row(
                "SELECT 1 FROM knowledge_bases WHERE slug = ?1",
                params![slug],
                |_| Ok(()),
            )
            .optional()?
            .is_some()
        {
            n += 1;
            slug = format!("{base}-{n}");
        }
        tx.execute(
            "INSERT INTO knowledge_bases (owner, folder_id, slug, title, audience, created) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![owner, dir.id, slug, title, audience.as_str(), crate::cloud::util::now()],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(id)
    })?;
    by_id(store, owner, id)
}

pub fn update(
    store: &Store,
    owner: &str,
    id: i64,
    title: Option<&str>,
    audience: Option<Audience>,
) -> Result<Site, StoreError> {
    let site = by_id(store, owner, id)?;
    let title = title
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| t.chars().take(120).collect::<String>())
        .unwrap_or(site.title);
    let audience = audience.unwrap_or(site.audience);
    store.with_db(|db| {
        db.execute(
            "UPDATE knowledge_bases SET title = ?1, audience = ?2 WHERE id = ?3 AND owner = ?4",
            params![title, audience.as_str(), id, owner],
        )?;
        Ok(())
    })?;
    by_id(store, owner, id)
}

pub fn remove(store: &Store, owner: &str, id: i64) -> Result<(), StoreError> {
    let n = store.with_db(|db| {
        Ok(db.execute(
            "DELETE FROM knowledge_bases WHERE id = ?1 AND owner = ?2",
            params![id, owner],
        )?)
    })?;
    if n == 0 {
        Err(StoreError::NotFound)
    } else {
        Ok(())
    }
}

/// What the site shows: the folder's vault without its private notes and
/// without the owner's folder names above it.
pub fn published(cloud: &Cloud, site: &Site) -> Result<Vault, StoreError> {
    let folder = site.folder.as_deref().ok_or(StoreError::NotFound)?;
    let mut v = vault(cloud, &cloud.notes, &site.owner, folder)?;
    v.notes.retain(|n| !n.scanned.private);
    v.root = String::new();
    v.name = site.title.clone();
    Ok(v)
}

/// The site's notes that say `q`. The vault the site shows has its root
/// cleared, so the search reads the files below the folder itself.
fn search(cloud: &Cloud, site: &Site, v: &Vault, q: &str) -> Vec<super::Hit> {
    let mut v = v.clone();
    v.root = site.folder.clone().unwrap_or_default();
    super::search(cloud, &v, &site.owner, q)
}

fn param(req: &HttpRequest, name: &str) -> Option<String> {
    url::form_urlencoded::parse(req.query_string().as_bytes())
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.into_owned())
}

#[derive(Deserialize)]
struct NewSite {
    path: String,
    #[serde(default)]
    title: String,
    audience: Audience,
}

#[derive(Deserialize)]
struct Change {
    title: Option<String>,
    audience: Option<Audience>,
}

fn off() -> HttpResponse {
    error(
        StatusCode::CONFLICT,
        "meshRequired",
        "knowledge bases need a box that shares and has joined a mesh",
    )
}

/// The owner's calls, under `/cloud/api/kb`.
pub async fn api(
    req: &HttpRequest,
    cloud: &Cloud,
    who: &Who,
    segs: &[&str],
    body: web::Payload,
) -> HttpResponse {
    let store = &cloud.store;
    let login = who.login.as_str();
    let available = cloud.config.knowledge_bases;
    match (req.method().as_str(), segs) {
        ("GET", []) => match list(store, login) {
            Ok(sites) => HttpResponse::Ok().json(json!({ "available": available, "sites": sites })),
            Err(e) => store_error(&e),
        },
        ("POST", []) => {
            if !available {
                return off();
            }
            let bytes = match read_body(body, 1 << 16).await {
                Ok(b) => b,
                Err(r) => return r,
            };
            let Ok(b) = serde_json::from_slice::<NewSite>(&bytes) else {
                return error(
                    StatusCode::BAD_REQUEST,
                    "badRequest",
                    "path, title and audience",
                );
            };
            let path = match crate::cloud::store::clean_path(&b.path) {
                Ok(p) => p,
                Err(e) => return store_error(&e),
            };
            match create(store, login, &path, &b.title, b.audience) {
                Ok(site) => HttpResponse::Ok().json(site),
                Err(e) => store_error(&e),
            }
        }
        ("PATCH", [id]) => {
            if !available {
                return off();
            }
            let Ok(id) = id.parse::<i64>() else {
                return store_error(&StoreError::NotFound);
            };
            let bytes = match read_body(body, 1 << 16).await {
                Ok(b) => b,
                Err(r) => return r,
            };
            let Ok(b) = serde_json::from_slice::<Change>(&bytes) else {
                return error(StatusCode::BAD_REQUEST, "badRequest", "title or audience");
            };
            match update(store, login, id, b.title.as_deref(), b.audience) {
                Ok(site) => HttpResponse::Ok().json(site),
                Err(e) => store_error(&e),
            }
        }
        // Stopping always works, mesh or not.
        ("DELETE", [id]) => match id.parse::<i64>() {
            Ok(id) => match remove(store, login, id) {
                Ok(()) => HttpResponse::Ok().json(json!({})),
                Err(e) => store_error(&e),
            },
            Err(_) => store_error(&StoreError::NotFound),
        },
        _ => error(StatusCode::NOT_FOUND, "notFound", "no such call"),
    }
}

fn not_found() -> HttpResponse {
    error(StatusCode::NOT_FOUND, "notFound", "no such knowledge base")
}

/// The site's own calls, `/kb/api/<slug>/…`, for whoever it is open to.
/// `who` is the person nginx names when they are signed in, else None.
pub async fn public(
    req: &HttpRequest,
    cloud: &Cloud,
    who: Option<&Who>,
    path: &str,
) -> HttpResponse {
    if !cloud.config.knowledge_bases || !matches!(req.method().as_str(), "GET" | "HEAD") {
        return not_found();
    }
    let rest = path.trim_start_matches("/kb/api/");
    // The box's sites, for the directory of wikis (the admin UI's /wikis).
    if rest.is_empty() {
        let q = param(req, "q").unwrap_or_default();
        let mut r = HttpResponse::Ok().json(index(cloud, who, &q));
        r.headers_mut().insert(
            actix_web::http::header::CACHE_CONTROL,
            actix_web::http::header::HeaderValue::from_static("private, no-cache"),
        );
        return r;
    }
    let (slug, call) = rest.split_once('/').unwrap_or((rest, ""));
    if !valid_slug(slug) {
        return not_found();
    }
    let site = match by_slug(&cloud.store, slug) {
        Ok(Some(s)) if s.folder.is_some() => s,
        Ok(_) => return not_found(),
        Err(e) => return store_error(&e),
    };
    if site.audience == Audience::Box && who.is_none() {
        return error(
            StatusCode::UNAUTHORIZED,
            "signIn",
            "this knowledge base is for people signed in to the box",
        );
    }
    let v = match published(cloud, &site) {
        Ok(v) => v,
        Err(_) => return not_found(),
    };
    let folder = site.folder.clone().unwrap_or_default();
    let mut r = match call {
        "site" => HttpResponse::Ok().json(json!({
            "slug": site.slug,
            "title": site.title,
            "audience": site.audience,
            "vault": v,
        })),
        "note" | "file" => {
            let Some(rel) = param(req, "path").as_deref().and_then(inside) else {
                return not_found();
            };
            let listed = if call == "note" {
                v.notes.iter().chain(&v.canvases).any(|n| n.path == rel)
            } else {
                v.files.iter().any(|f| f.path == rel)
            };
            if !listed {
                return not_found();
            }
            let full = join(&folder, &rel);
            let e = match cloud.store.stat(&site.owner, &full) {
                Ok(e) if !e.dir => e,
                _ => return not_found(),
            };
            let mime = if call == "note" {
                "text/markdown; charset=utf-8"
            } else {
                e.mime.as_str()
            };
            serve_file(
                req,
                &cloud.store.disk_path(&site.owner, &full),
                &e.name,
                mime,
                Some(&e.etag),
                e.mtime,
                None,
                false,
            )
        }
        "search" => {
            let q = param(req, "q").unwrap_or_default();
            HttpResponse::Ok().json(json!({ "hits": search(cloud, &site, &v, &q) }))
        }
        _ => return not_found(),
    };
    // A file from the site is never a page that runs anything on the box's
    // origin, and nothing here is kept by a shared cache.
    let headers = r.headers_mut();
    headers.insert(
        actix_web::http::header::HeaderName::from_static("x-content-type-options"),
        actix_web::http::header::HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        actix_web::http::header::CACHE_CONTROL,
        actix_web::http::header::HeaderValue::from_static("private, no-cache"),
    );
    if call == "file" {
        headers.insert(
            actix_web::http::header::CONTENT_SECURITY_POLICY,
            actix_web::http::header::HeaderValue::from_static(
                "sandbox; default-src 'none'; img-src 'self'; media-src 'self'",
            ),
        );
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::Config;

    fn cloud(on: bool) -> (tempfile::TempDir, Cloud) {
        let dir = tempfile::tempdir().unwrap();
        let cloud = Cloud::open(
            dir.path(),
            Config {
                knowledge_bases: on,
                ..Config::default()
            },
        )
        .unwrap();
        for who in ["ann", "bob"] {
            cloud.store.ensure_home(who).unwrap();
        }
        (dir, cloud)
    }

    fn write(cloud: &Cloud, owner: &str, path: &str, text: &str) {
        if let Some(parent) = crate::cloud::store::parent_of(path) {
            if !parent.is_empty() {
                cloud.store.mkdir_all(owner, parent).unwrap();
            }
        }
        cloud
            .store
            .write_bytes(owner, path, text.as_bytes())
            .unwrap();
    }

    #[test]
    fn slugs_are_plain_and_short() {
        assert_eq!(slug_for("Garden Notes"), "garden-notes");
        assert_eq!(slug_for("  Poznámky o záhrade! "), "poznamky-o-zahrade");
        assert_eq!(slug_for("Über Straße"), "uber-strasse");
        assert_eq!(slug_for("***"), "notes");
        assert!(slug_for(&"x".repeat(80)).len() <= 48);
        assert!(valid_slug("garden-notes-2"));
        assert!(!valid_slug("../etc"));
        assert!(!valid_slug("A"));
    }

    #[test]
    fn one_site_per_folder_and_unique_addresses() {
        let (_d, cloud) = cloud(true);
        write(&cloud, "ann", "Garden/a.md", "x");
        write(&cloud, "bob", "Garden/b.md", "x");
        let a = create(&cloud.store, "ann", "Garden", "Garden", Audience::Anyone).unwrap();
        assert_eq!(a.slug, "garden");
        assert_eq!(a.folder.as_deref(), Some("Garden"));
        assert!(matches!(
            create(&cloud.store, "ann", "Garden", "Other", Audience::Box),
            Err(StoreError::Exists)
        ));
        let b = create(&cloud.store, "bob", "Garden", "", Audience::Box).unwrap();
        assert_eq!(b.slug, "garden-2");
        assert_eq!(b.title, "Garden");
        // Nobody changes or stops another person's site.
        assert!(update(&cloud.store, "bob", a.id, Some("Mine"), None).is_err());
        assert!(remove(&cloud.store, "bob", a.id).is_err());
        let a = update(
            &cloud.store,
            "ann",
            a.id,
            Some("Our garden"),
            Some(Audience::Box),
        )
        .unwrap();
        assert_eq!(
            (a.title.as_str(), a.audience, a.slug.as_str()),
            ("Our garden", Audience::Box, "garden")
        );
        assert_eq!(list(&cloud.store, "ann").unwrap().len(), 1);
        remove(&cloud.store, "ann", a.id).unwrap();
        assert!(list(&cloud.store, "ann").unwrap().is_empty());
    }

    #[test]
    fn the_index_lists_what_the_reader_may_read() {
        let (_d, cloud) = cloud(true);
        write(&cloud, "ann", "Garden/Beans.md", "Sow beans in May.");
        write(
            &cloud,
            "ann",
            "Garden/Diary.md",
            "---\nprivate: true\n---\nbeans for me",
        );
        write(&cloud, "bob", "Recipes/Soup.md", "Bean soup.");
        create(&cloud.store, "ann", "Garden", "Garden", Audience::Anyone).unwrap();
        create(&cloud.store, "bob", "Recipes", "Recipes", Audience::Box).unwrap();
        let who = Who {
            login: "bob".into(),
            name: "Bob".into(),
            email: String::new(),
            via: crate::cloud::auth::Via::Header,
        };
        let titles = |v: &serde_json::Value| -> Vec<String> {
            v["sites"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s["title"].as_str().unwrap().to_owned())
                .collect()
        };
        // Nobody signed in reads the open site only.
        let open = index(&cloud, None, "");
        assert_eq!(titles(&open), ["Garden"]);
        assert_eq!(open["sites"][0]["notes"], 1);
        assert!(open["sites"][0].get("folder").is_none());
        let signed = index(&cloud, Some(&who), "bean");
        assert_eq!(titles(&signed), ["Garden", "Recipes"]);
        assert_eq!(signed["sites"][1]["mine"], true);
        let hits: Vec<&str> = signed["hits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| h["path"].as_str().unwrap())
            .collect();
        // The private diary is no hit, though it says beans.
        assert_eq!(hits, ["Beans.md", "Soup.md"]);
    }

    #[test]
    fn a_site_follows_its_folder_and_ends_with_it() {
        let (_d, cloud) = cloud(true);
        write(&cloud, "ann", "Garden/a.md", "x");
        let s = create(&cloud.store, "ann", "Garden", "Garden", Audience::Anyone).unwrap();
        cloud.store.rename("ann", "Garden", "Yard", false).unwrap();
        assert_eq!(
            by_slug(&cloud.store, &s.slug)
                .unwrap()
                .unwrap()
                .folder
                .as_deref(),
            Some("Yard")
        );
        cloud.store.delete("ann", "Yard", None).unwrap();
        assert_eq!(
            by_slug(&cloud.store, &s.slug).unwrap().unwrap().folder,
            None
        );
    }

    #[test]
    fn private_notes_and_dot_folders_stay_off_the_site() {
        let (_d, cloud) = cloud(true);
        write(&cloud, "ann", "Garden/Open.md", "# Open\n[[Secret]]");
        write(
            &cloud,
            "ann",
            "Garden/Secret.md",
            "---\npublish: false\n---\nhidden",
        );
        write(
            &cloud,
            "ann",
            "Garden/Diary.md",
            "---\nprivate: true\n---\nhidden",
        );
        write(&cloud, "ann", "Garden/.obsidian/workspace.json", "{}");
        write(&cloud, "ann", "Garden/pic.png", "png");
        let s = create(&cloud.store, "ann", "Garden", "Garden", Audience::Anyone).unwrap();
        let v = published(&cloud, &s).unwrap();
        let notes: Vec<&str> = v.notes.iter().map(|n| n.path.as_str()).collect();
        assert_eq!(notes, ["Open.md"]);
        assert_eq!(v.root, "");
        assert_eq!(v.files.len(), 1);
    }

    mod http {
        use super::*;
        use actix_web::test::TestRequest;

        async fn get(cloud: &Cloud, who: Option<&Who>, uri: &str) -> (u16, String) {
            let req = TestRequest::get().uri(uri).to_http_request();
            let path = req.path().to_owned();
            let r = public(&req, cloud, who, &path).await;
            let status = r.status().as_u16();
            let body = actix_web::body::to_bytes(r.into_body())
                .await
                .unwrap_or_default();
            (status, String::from_utf8_lossy(&body).into_owned())
        }

        fn eva() -> Who {
            Who {
                login: "eva".into(),
                name: "Eva".into(),
                email: String::new(),
                via: crate::cloud::auth::Via::Header,
            }
        }

        #[actix_web::test]
        async fn anyone_reads_a_public_site_but_never_a_private_note() {
            let (_d, cloud) = cloud(true);
            write(&cloud, "ann", "Garden/Open.md", "# Open");
            write(
                &cloud,
                "ann",
                "Garden/Secret.md",
                "---\nprivate: true\n---\nsecret words",
            );
            write(&cloud, "ann", "Garden/.obsidian/app.json", "{}");
            write(&cloud, "ann", "Elsewhere.md", "not published");
            create(&cloud.store, "ann", "Garden", "Garden", Audience::Anyone).unwrap();
            let (status, body) = get(&cloud, None, "/kb/api/garden/site").await;
            assert_eq!(status, 200, "{body}");
            assert!(
                body.contains("Open.md") && !body.contains("Secret.md"),
                "{body}"
            );
            assert!(
                !body.contains("\"root\":\"Garden\""),
                "the folder's place stays private: {body}"
            );
            assert_eq!(
                get(&cloud, None, "/kb/api/garden/note?path=Open.md").await,
                (200, "# Open".into())
            );
            for path in [
                "Secret.md",
                "../Elsewhere.md",
                ".obsidian/app.json",
                "%2E%2E/Elsewhere.md",
                "Missing.md",
            ] {
                let (status, body) =
                    get(&cloud, None, &format!("/kb/api/garden/note?path={path}")).await;
                assert_eq!(status, 404, "{path}: {body}");
                assert!(!body.contains("secret") && !body.contains("not published"));
            }
            let (_, body) = get(&cloud, None, "/kb/api/garden/search?q=secret").await;
            assert!(body.contains("\"hits\":[]"), "{body}");
            assert_eq!(get(&cloud, None, "/kb/api/nothing/site").await.0, 404);
        }

        #[actix_web::test]
        async fn a_box_site_wants_someone_signed_in() {
            let (_d, cloud) = cloud(true);
            write(&cloud, "ann", "Team/Plan.md", "plan");
            create(&cloud.store, "ann", "Team", "Team", Audience::Box).unwrap();
            assert_eq!(get(&cloud, None, "/kb/api/team/site").await.0, 401);
            assert_eq!(
                get(&cloud, None, "/kb/api/team/note?path=Plan.md").await.0,
                401
            );
            assert_eq!(
                get(&cloud, Some(&eva()), "/kb/api/team/note?path=Plan.md")
                    .await
                    .0,
                200
            );
        }

        #[actix_web::test]
        async fn without_the_mesh_no_site_answers() {
            let (_d, cloud) = cloud(true);
            write(&cloud, "ann", "Garden/Open.md", "x");
            create(&cloud.store, "ann", "Garden", "Garden", Audience::Anyone).unwrap();
            let cloud = Cloud {
                config: Config {
                    knowledge_bases: false,
                    ..Config::default()
                },
                ..cloud
            };
            assert_eq!(get(&cloud, None, "/kb/api/garden/site").await.0, 404);
            assert_eq!(
                get(&cloud, Some(&eva()), "/kb/api/garden/site").await.0,
                404
            );
        }
    }
}
