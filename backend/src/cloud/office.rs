//! Office documents in LosOS Cloud: native LosOS Office capabilities and
//! open/import routing.
//!
//! Collabora/WOPI integration has been removed. `POST /cloud/api/office/open`
//! resolves how a file should be handled by native LosOS Office (native open,
//! import to native, or view/unsupported). The web app opens `.md/.paper/.slides`
//! directly; legacy formats are imported into native formats.

use crate::cloud::auth::Who;
use crate::cloud::store::{self, Entry, StoreError as CloudError};
use crate::cloud::Cloud;
use crate::office::actix::{extensions_answer, open_answer, Sessions};
use crate::office::store::{valid_name, CreateMode, FileInfo, OfficeStore, StoreError};
use crate::office::Person;
use actix_web::{web, HttpRequest, HttpResponse, Scope};
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

pub struct CloudFiles {
    pub cloud: Arc<Cloud>,
}

fn office_error(e: CloudError) -> StoreError {
    match e {
        CloudError::NotFound | CloudError::Conflict => StoreError::NotFound,
        CloudError::Exists => StoreError::Exists,
        CloudError::BadName | CloudError::Loop => StoreError::InvalidName,
        CloudError::Precondition => StoreError::Forbidden,
        e => StoreError::Io(e.to_string()),
    }
}

fn info(e: &Entry) -> FileInfo {
    FileInfo {
        name: e.name.clone(),
        size: e.size,
        version: e.etag.clone(),
        modified: UNIX_EPOCH + Duration::from_secs(e.mtime.max(0) as u64),
        owner: e.owner.clone(),
        writable: true,
    }
}

impl CloudFiles {
    fn file(&self, user: &str, file_id: &str) -> Result<Entry, StoreError> {
        let id: i64 = file_id.parse().map_err(|_| StoreError::NotFound)?;
        let e = self.cloud.store.by_id(user, id).map_err(office_error)?;
        if e.dir {
            return Err(StoreError::NotFound);
        }
        Ok(e)
    }

    fn written(&self, user: &str, e: &Entry) -> (String, FileInfo) {
        self.cloud.changed(user, e);
        (e.id.to_string(), info(e))
    }
}

fn numbered(name: &str, n: u32) -> String {
    match name.rfind('.') {
        Some(dot) if dot > 0 => format!("{} ({n}){}", &name[..dot], &name[dot..]),
        _ => format!("{name} ({n})"),
    }
}

impl OfficeStore for CloudFiles {
    fn stat(&self, user: &str, file_id: &str) -> Result<FileInfo, StoreError> {
        Ok(info(&self.file(user, file_id)?))
    }

    fn read(&self, user: &str, file_id: &str) -> Result<Vec<u8>, StoreError> {
        let e = self.file(user, file_id)?;
        self.cloud
            .store
            .read_bytes(user, &e.path)
            .map_err(office_error)
    }

    fn write(&self, user: &str, file_id: &str, bytes: &[u8]) -> Result<FileInfo, StoreError> {
        let e = self.file(user, file_id)?;
        let w = self
            .cloud
            .store
            .write_bytes(user, &e.path, bytes)
            .map_err(office_error)?;
        Ok(self.written(user, &w.entry).1)
    }

    fn create(
        &self,
        user: &str,
        beside: &str,
        name: &str,
        bytes: &[u8],
        mode: CreateMode,
    ) -> Result<(String, FileInfo), StoreError> {
        if !valid_name(name) || !store::valid_name(name) {
            return Err(StoreError::InvalidName);
        }
        let dir = store::parent_of(&self.file(user, beside)?.path)
            .unwrap_or_default()
            .to_owned();
        let taken = |path: &str| self.cloud.store.stat(user, path).is_ok();
        let mut path = store::join(&dir, name);
        match mode {
            CreateMode::Overwrite => {}
            CreateMode::Exact if taken(&path) => return Err(StoreError::Exists),
            CreateMode::Exact => {}
            CreateMode::Unique => {
                let mut n = 2;
                while taken(&path) {
                    path = store::join(&dir, &numbered(name, n));
                    n += 1;
                }
            }
        }
        let w = self
            .cloud
            .store
            .write_bytes(user, &path, bytes)
            .map_err(office_error)?;
        Ok(self.written(user, &w.entry))
    }

    fn rename(
        &self,
        user: &str,
        file_id: &str,
        new_name: &str,
    ) -> Result<(String, FileInfo), StoreError> {
        if !valid_name(new_name) || !store::valid_name(new_name) {
            return Err(StoreError::InvalidName);
        }
        let e = self.file(user, file_id)?;
        let to = store::join(store::parent_of(&e.path).unwrap_or_default(), new_name);
        let w = self
            .cloud
            .store
            .rename(user, &e.path, &to, false)
            .map_err(office_error)?;
        Ok(self.written(user, &w.entry))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenBody {
    file_id: String,
}

fn person(who: &Who) -> Person {
    Person {
        login: who.login.clone(),
        display_name: if who.name.is_empty() {
            who.login.clone()
        } else {
            who.name.clone()
        },
    }
}

async fn open(
    req: HttpRequest,
    who: Who,
    body: web::Json<OpenBody>,
    sessions: web::Data<Sessions>,
) -> HttpResponse {
    if crate::cloud::server::cross_site(&req) {
        return crate::cloud::api::error(
            actix_web::http::StatusCode::FORBIDDEN,
            "crossSite",
            "a request from another site",
        );
    }
    open_answer(person(&who), body.into_inner().file_id, sessions.into_inner()).await
}

async fn extensions(_who: Who, sessions: web::Data<Sessions>) -> HttpResponse {
    extensions_answer(sessions.into_inner()).await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportBody {
    file_id: String,
}

fn escape_mdx(text: &str) -> String {
    text.replace('\\', "\\\\").replace('{', "\\{").replace('}', "\\}").replace('<', "&lt;").replace('>', "&gt;")
}

fn extract_text(name: &str, bytes: &[u8]) -> String {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    if matches!(ext.as_str(), "txt" | "csv" | "md" | "markdown") {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    if let Ok(mut zip) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) {
        let mut out = String::new();
        let targets: &[&str] = match ext.as_str() {
            "odt" | "odp" | "ods" => &["content.xml"],
            "docx" => &["word/document.xml"],
            "pptx" => &["ppt/slides/slide1.xml", "ppt/slides/slide2.xml", "ppt/slides/slide3.xml"],
            "xlsx" => &["xl/sharedStrings.xml"],
            _ => &[],
        };
        for target in targets {
            if let Ok(mut f) = zip.by_name(target) {
                let mut xml = String::new();
                if std::io::Read::read_to_string(&mut f, &mut xml).is_ok() {
                    // Very rough text extraction: strip tags, collapse whitespace.
                    let mut text = String::with_capacity(xml.len() / 2);
                    let mut in_tag = false;
                    for c in xml.chars() {
                        match c {
                            '<' => in_tag = true,
                            '>' => { in_tag = false; text.push(' '); }
                            _ if !in_tag => text.push(c),
                            _ => {}
                        }
                    }
                    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
                    if !collapsed.is_empty() {
                        if !out.is_empty() { out.push_str("\n\n"); }
                        out.push_str(&collapsed);
                    }
                }
            }
        }
        // DOCX: also try to get all word/*.xml text if targeted extraction missed.
        if out.is_empty() && ext == "docx" {
            for i in 0..zip.len() {
                if let Ok(mut f) = zip.by_index(i) {
                    let n = f.name().map(|s| s.to_string()).unwrap_or_default();
                    if n.starts_with("word/") && n.ends_with(".xml") && !n.contains("fontTable") && !n.contains("settings") {
                        let mut xml = String::new();
                        if std::io::Read::read_to_string(&mut f, &mut xml).is_ok() {
                            let mut text = String::new();
                            let mut in_tag = false;
                            for c in xml.chars() {
                                match c { '<' => in_tag = true, '>' => { in_tag = false; text.push(' '); } _ if !in_tag => text.push(c), _ => {} }
                            }
                            let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
                            if collapsed.len() > 40 { out = collapsed; break; }
                        }
                    }
                }
            }
        }
        if !out.is_empty() { return out; }
    }
    format!("Imported from {name}. Formatting was not preserved.")
}

async fn import_legacy(
    req: HttpRequest,
    who: Who,
    body: web::Json<ImportBody>,
    sessions: web::Data<Sessions>,
) -> HttpResponse {
    if crate::cloud::server::cross_site(&req) {
        return crate::cloud::api::error(actix_web::http::StatusCode::FORBIDDEN, "crossSite", "a request from another site");
    }
    let sessions = sessions.into_inner();
    let login = who.login.clone();
    let file_id = body.into_inner().file_id;
    let result = web::block(move || -> Result<serde_json::Value, StoreError> {
        let info = sessions.store.stat(&login, &file_id)?;
        let bytes = sessions.store.read(&login, &file_id)?;
        let text = extract_text(&info.name, &bytes);
        let ext = crate::office::extension(&info.name).unwrap_or_default();
        let target = sessions.capabilities.action(&ext).and_then(|a| a.import_target).unwrap_or("paper");
        let stem = info.name.rsplit_once('.').map(|(s, _)| s).unwrap_or(&info.name);
        let (new_name, body) = if target == "slides" {
            (format!("{stem}.slides"), format!("# {title}\n\n{body}\n", title = escape_mdx(stem), body = escape_mdx(&text)))
        } else {
            (format!("{stem}.paper"), format!("# {title}\n\n{body}\n", title = escape_mdx(stem), body = escape_mdx(&text)))
        };
        let (id, info) = sessions.store.create(&login, &file_id, &new_name, body.as_bytes(), crate::office::store::CreateMode::Unique)?;
        Ok(serde_json::json!({ "id": id.parse::<i64>().unwrap_or(0), "name": info.name, "kind": target }))
    }).await;
    match result {
        Ok(Ok(v)) => HttpResponse::Ok().json(v),
        Ok(Err(StoreError::NotFound | StoreError::Forbidden)) => HttpResponse::NotFound().json(serde_json::json!({"error":"notFound"})),
        Ok(Err(e)) => { tracing::warn!(error=%e, "office import failed"); HttpResponse::InternalServerError().json(serde_json::json!({"error":"store"})) }
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

/// `/cloud/api/office`. Needs `web::Data<Sessions>`.
pub fn scope() -> Scope {
    web::scope("/cloud/api/office")
        .route("/open", web::post().to(open))
        .route("/extensions", web::get().to(extensions))
        .route("/import", web::post().to(import_legacy))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::Config;

    fn files() -> (tempfile::TempDir, CloudFiles, Entry) {
        let dir = tempfile::tempdir().unwrap();
        let cloud = Arc::new(Cloud::open(dir.path(), Config::default()).unwrap());
        cloud.store.ensure_home("matus").unwrap();
        cloud.store.mkdir("matus", "Trips").unwrap();
        let e = cloud
            .store
            .write_bytes("matus", "Trips/Plan.docx", b"one")
            .unwrap()
            .entry;
        (dir, CloudFiles { cloud }, e)
    }

    #[test]
    fn a_file_is_its_owners_by_id_and_nobody_elses() {
        let (_dir, files, e) = files();
        let id = e.id.to_string();
        let info = files.stat("matus", &id).unwrap();
        assert_eq!(info.name, "Plan.docx");
        assert_eq!(files.stat("eva", &id), Err(StoreError::NotFound));
    }
}
