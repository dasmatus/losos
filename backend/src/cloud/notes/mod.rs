//! Notes: a folder of Markdown files read as a vault, the way Obsidian
//! reads one, so a vault synced to the same folder opens in either.
//!
//! The box lists a vault ([`vault`]): every note with what [`scan`] read
//! from it, and every other file a note may embed. The web app resolves the
//! links, draws the graph and renders the notes; their text comes from the
//! files API like any other file's, and is edited together through
//! [`crate::cloud::collab`]. A folder can also be published as a knowledge
//! base ([`kb`]), a site of its notes others can read.
//!
//! Folders and files whose name starts with a dot (`.obsidian`, `.trash`)
//! are not part of the vault, as in Obsidian.

pub mod kb;
pub mod scan;

use crate::cloud::api::{error, store_error};
use crate::cloud::auth::Who;
use crate::cloud::store::{clean_path, Entry, StoreError};
use crate::cloud::Cloud;
use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A note larger than this is listed but not read.
const MAX_NOTE: u64 = 2 << 20;
/// Search answers with at most this many lines.
const MAX_HITS: usize = 60;

/// One note of a vault.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: i64,
    /// Relative to the vault's folder, with `.md`.
    pub path: String,
    pub size: u64,
    pub mtime: i64,
    pub etag: String,
    #[serde(flatten)]
    pub scanned: Arc<scan::Scanned>,
}

/// A file in a vault that is not a note: a picture, a PDF, a recording.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: i64,
    pub path: String,
    pub mime: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Vault {
    /// The folder, relative to the person's home.
    pub root: String,
    pub name: String,
    pub folder_id: i64,
    pub notes: Vec<Note>,
    /// JSON Canvas files (`.canvas`), with the links their nodes make.
    pub canvases: Vec<Note>,
    pub files: Vec<Attachment>,
    /// Folders, for the tree, even the empty ones.
    pub folders: Vec<String>,
}

/// Notes read once per version: file id to its ETag and what was read.
#[derive(Default)]
pub struct Cache {
    read: Mutex<HashMap<i64, (String, Arc<scan::Scanned>)>>,
}

fn hidden(rel: &str) -> bool {
    rel.split('/').any(|part| part.starts_with('.'))
}

fn ends_with(name: &str, ext: &str) -> bool {
    name.len() > ext.len()
        && name
            .get(name.len() - ext.len()..)
            .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

pub fn is_note(name: &str) -> bool {
    ends_with(name, ".md")
}

pub fn is_canvas(name: &str) -> bool {
    ends_with(name, ".canvas")
}

fn relative<'a>(root: &str, path: &'a str) -> &'a str {
    if root.is_empty() {
        path
    } else {
        path.get(root.len() + 1..).unwrap_or_default()
    }
}

impl Cache {
    fn scanned(&self, cloud: &Cloud, e: &Entry) -> Arc<scan::Scanned> {
        if let Some((etag, s)) = self
            .read
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&e.id)
        {
            if *etag == e.etag {
                return s.clone();
            }
        }
        let s = if e.size > MAX_NOTE {
            scan::Scanned::default()
        } else {
            let text = std::fs::read(cloud.store.disk_path(&e.owner, &e.path)).unwrap_or_default();
            let text = String::from_utf8_lossy(&text);
            if is_canvas(&e.name) {
                scan::scan_canvas(&text)
            } else {
                scan::scan(&text)
            }
        };
        let s = Arc::new(s);
        let mut read = self.read.lock().unwrap_or_else(|p| p.into_inner());
        // A bound on what is kept: a box with many vaults reads again.
        if read.len() > 20_000 {
            read.clear();
        }
        read.insert(e.id, (e.etag.clone(), s.clone()));
        s
    }
}

/// The vault at `root` in `owner`'s files.
pub fn vault(cloud: &Cloud, cache: &Cache, owner: &str, root: &str) -> Result<Vault, StoreError> {
    let folder = cloud.store.stat(owner, root)?;
    if !folder.dir {
        return Err(StoreError::Conflict);
    }
    let mut notes = Vec::new();
    let mut canvases = Vec::new();
    let mut files = Vec::new();
    let mut folders = Vec::new();
    for e in cloud.store.walk(owner, root)? {
        let rel = relative(root, &e.path).to_owned();
        if rel.is_empty() || hidden(&rel) {
            continue;
        }
        if e.dir {
            folders.push(rel);
        } else if is_note(&e.name) || is_canvas(&e.name) {
            let list = if is_note(&e.name) {
                &mut notes
            } else {
                &mut canvases
            };
            list.push(Note {
                id: e.id,
                scanned: cache.scanned(cloud, &e),
                path: rel,
                size: e.size,
                mtime: e.mtime,
                etag: e.etag.clone(),
            });
        } else {
            files.push(Attachment {
                id: e.id,
                path: rel,
                mime: e.mime.clone(),
                size: e.size,
            });
        }
    }
    let name = if root.is_empty() {
        String::new()
    } else {
        folder.name.clone()
    };
    Ok(Vault {
        root: root.to_owned(),
        name,
        folder_id: folder.id,
        notes,
        canvases,
        files,
        folders,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub path: String,
    /// From 0, in the file.
    pub line: usize,
    pub text: String,
}

/// Lines of the vault's notes that hold every word of `q`, best first: a
/// note whose name matches, then by how many lines matched.
pub fn search(cloud: &Cloud, vault: &Vault, owner: &str, q: &str) -> Vec<Hit> {
    let words: Vec<String> = q
        .split_whitespace()
        .map(str::to_lowercase)
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<(bool, Vec<Hit>)> = Vec::new();
    for n in &vault.notes {
        if n.size > MAX_NOTE {
            continue;
        }
        let path = if vault.root.is_empty() {
            n.path.clone()
        } else {
            format!("{}/{}", vault.root, n.path)
        };
        let Ok(bytes) = std::fs::read(cloud.store.disk_path(owner, &path)) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        let name = n.path.to_lowercase();
        let by_name = words.iter().all(|w| name.contains(w.as_str()));
        let mut hits = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let lower = line.to_lowercase();
            if words.iter().all(|w| lower.contains(w.as_str())) {
                let text: String = line.trim().chars().take(240).collect();
                hits.push(Hit {
                    path: n.path.clone(),
                    line: i,
                    text,
                });
            }
        }
        if by_name && hits.is_empty() {
            hits.push(Hit {
                path: n.path.clone(),
                line: 0,
                text: String::new(),
            });
        }
        if !hits.is_empty() {
            found.push((by_name, hits));
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.len().cmp(&a.1.len())));
    found
        .into_iter()
        .flat_map(|(_, h)| h)
        .take(MAX_HITS)
        .collect()
}

fn param(req: &HttpRequest, name: &str) -> Option<String> {
    url::form_urlencoded::parse(req.query_string().as_bytes())
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.into_owned())
}

/// The vault's folder from `?path=`, cleaned.
fn root_param(req: &HttpRequest) -> Result<String, HttpResponse> {
    clean_path(&param(req, "path").unwrap_or_default()).map_err(|e| store_error(&e))
}

/// Calls under `/cloud/api/notes/`.
pub async fn api(
    req: &HttpRequest,
    cloud: &Cloud,
    who: &Who,
    segs: &[&str],
    _body: web::Payload,
) -> HttpResponse {
    let login = who.login.as_str();
    let cache = &cloud.notes;
    match (req.method().as_str(), segs) {
        ("GET", ["vault"]) => {
            let root = match root_param(req) {
                Ok(r) => r,
                Err(r) => return r,
            };
            match vault(cloud, cache, login, &root) {
                Ok(v) => HttpResponse::Ok().json(v),
                Err(e) => store_error(&e),
            }
        }
        ("GET", ["search"]) => {
            let root = match root_param(req) {
                Ok(r) => r,
                Err(r) => return r,
            };
            let q = param(req, "q").unwrap_or_default();
            match vault(cloud, cache, login, &root) {
                Ok(v) => HttpResponse::Ok().json(json!({ "hits": search(cloud, &v, login, &q) })),
                Err(e) => store_error(&e),
            }
        }
        // Which vault a note or folder belongs to, for a note opened from
        // the file list.
        ("GET", ["root"]) => {
            let path = match root_param(req) {
                Ok(r) => r,
                Err(r) => return r,
            };
            HttpResponse::Ok().json(json!({ "root": vault_root(cloud, login, &path) }))
        }
        _ => error(StatusCode::NOT_FOUND, "notFound", "no such call"),
    }
}

/// A path a client named inside a vault, cleaned, or None when it is the
/// vault itself, climbs out or is hidden.
pub fn inside(rel: &str) -> Option<String> {
    let p = clean_path(rel).ok()?;
    (!p.is_empty() && !hidden(&p)).then_some(p)
}

/// The vault a note or folder belongs to: the nearest folder at or above
/// it holding `.obsidian`, as Obsidian finds its vault, or None.
pub fn vault_root(cloud: &Cloud, owner: &str, path: &str) -> Option<String> {
    let mut at = match cloud.store.stat(owner, path) {
        Ok(e) if e.dir => Some(path.to_owned()),
        Ok(_) => crate::cloud::store::parent_of(path).map(str::to_owned),
        Err(_) => None,
    };
    while let Some(dir) = at {
        let marker = crate::cloud::store::join(&dir, ".obsidian");
        if cloud.store.stat(owner, &marker).is_ok_and(|e| e.dir) {
            return Some(dir);
        }
        at = crate::cloud::store::parent_of(&dir).map(str::to_owned);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::Config;

    fn cloud() -> (tempfile::TempDir, Cloud) {
        let dir = tempfile::tempdir().unwrap();
        let cloud = Cloud::open(dir.path(), Config::default()).unwrap();
        cloud.store.ensure_home("ann").unwrap();
        (dir, cloud)
    }

    fn write(cloud: &Cloud, path: &str, text: &str) {
        if let Some(parent) = crate::cloud::store::parent_of(path) {
            if !parent.is_empty() {
                cloud.store.mkdir_all("ann", parent).unwrap();
            }
        }
        cloud
            .store
            .write_bytes("ann", path, text.as_bytes())
            .unwrap();
    }

    #[test]
    fn a_vault_lists_notes_and_files_but_not_dot_folders() {
        let (_d, cloud) = cloud();
        write(&cloud, "Vault/Home.md", "# Home\n[[Ideas]] #start\n");
        write(
            &cloud,
            "Vault/Sub/Ideas.md",
            "---\naliases: [Thoughts]\n---\n![[pic.png]]",
        );
        write(&cloud, "Vault/pic.png", "png");
        write(&cloud, "Vault/.obsidian/app.json", "{}");
        write(&cloud, "Vault/.trash/Old.md", "old");
        write(&cloud, "Elsewhere.md", "not in the vault");
        let cache = Cache::default();
        let v = vault(&cloud, &cache, "ann", "Vault").unwrap();
        let notes: Vec<&str> = v.notes.iter().map(|n| n.path.as_str()).collect();
        assert_eq!(notes, ["Home.md", "Sub/Ideas.md"]);
        assert_eq!(v.files.len(), 1);
        assert_eq!(v.files[0].path, "pic.png");
        assert_eq!(v.folders, ["Sub"]);
        assert_eq!(v.notes[0].scanned.tags, ["start"]);
        assert_eq!(v.notes[1].scanned.aliases, ["Thoughts"]);
        // Read again only when the note changes.
        let first = v.notes[0].scanned.clone();
        let again = vault(&cloud, &cache, "ann", "Vault").unwrap();
        assert!(Arc::ptr_eq(&first, &again.notes[0].scanned));
        write(&cloud, "Vault/Home.md", "#changed");
        let changed = vault(&cloud, &cache, "ann", "Vault").unwrap();
        assert_eq!(changed.notes[0].scanned.tags, ["changed"]);
    }

    #[test]
    fn canvases_are_listed_with_their_links() {
        let (_d, cloud) = cloud();
        write(
            &cloud,
            "V/Map.canvas",
            r#"{"nodes":[{"id":"a","type":"file","file":"Notes/Home.md","x":0,"y":0,"width":1,"height":1},{"id":"b","type":"text","text":"See [[Ideas]]","x":0,"y":0,"width":1,"height":1}],"edges":[]}"#,
        );
        let v = vault(&cloud, &Cache::default(), "ann", "V").unwrap();
        assert!(v.notes.is_empty());
        let links: Vec<&str> = v.canvases[0]
            .scanned
            .links
            .iter()
            .map(|l| l.target.as_str())
            .collect();
        assert_eq!(links, ["Notes/Home.md", "Ideas"]);
    }

    #[test]
    fn the_vault_is_the_folder_holding_dot_obsidian() {
        let (_d, cloud) = cloud();
        write(&cloud, "Work/Vault/.obsidian/app.json", "{}");
        write(&cloud, "Work/Vault/Sub/Note.md", "x");
        write(&cloud, "Loose/Note.md", "x");
        assert_eq!(
            vault_root(&cloud, "ann", "Work/Vault/Sub/Note.md").as_deref(),
            Some("Work/Vault")
        );
        assert_eq!(
            vault_root(&cloud, "ann", "Work/Vault").as_deref(),
            Some("Work/Vault")
        );
        assert_eq!(vault_root(&cloud, "ann", "Loose/Note.md"), None);
    }

    #[test]
    fn paths_inside_a_vault() {
        assert_eq!(inside("a/b.md").as_deref(), Some("a/b.md"));
        assert_eq!(inside("/a//b.md/").as_deref(), Some("a/b.md"));
        assert_eq!(inside("../x.md"), None);
        assert_eq!(inside(".obsidian/app.json"), None);
        assert_eq!(inside(""), None);
    }

    #[test]
    fn the_whole_home_is_a_vault_too() {
        let (_d, cloud) = cloud();
        write(&cloud, "a.md", "x");
        let v = vault(&cloud, &Cache::default(), "ann", "").unwrap();
        assert_eq!(v.notes[0].path, "a.md");
        assert_eq!(v.name, "");
    }

    #[test]
    fn a_file_is_no_vault() {
        let (_d, cloud) = cloud();
        write(&cloud, "a.md", "x");
        assert!(matches!(
            vault(&cloud, &Cache::default(), "ann", "a.md"),
            Err(StoreError::Conflict)
        ));
    }

    #[test]
    fn search_finds_lines_with_every_word_names_first() {
        let (_d, cloud) = cloud();
        write(&cloud, "V/Garden plan.md", "nothing here");
        write(
            &cloud,
            "V/Log.md",
            "The garden plan is late\nplan\nGarden PLAN again",
        );
        let v = vault(&cloud, &Cache::default(), "ann", "V").unwrap();
        let hits = search(&cloud, &v, "ann", "garden plan");
        let got: Vec<(&str, usize)> = hits.iter().map(|h| (h.path.as_str(), h.line)).collect();
        assert_eq!(got, [("Garden plan.md", 0), ("Log.md", 0), ("Log.md", 2)]);
        assert!(search(&cloud, &v, "ann", "  ").is_empty());
    }
}
