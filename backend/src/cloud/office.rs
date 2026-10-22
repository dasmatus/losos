//! Office documents in LosOS Cloud: the cloud's files behind the WOPI host
//! of [`crate::office`], and the two calls the web app makes.
//!
//! `POST /cloud/api/office/open {fileId}` starts an editor session for one
//! of the signed-in person's files; `GET /cloud/api/office/extensions` lists
//! what Collabora opens. Both take the person from the cloud's own sign-in
//! ([`Who`]), like every other `/cloud/api/` call. coolwsd then reads and
//! saves the file through `/wopi/files/<id>` on a loopback listener of the
//! cloud's own (`losos-cloud serve --office-wopi`), where the access token
//! is the whole authentication.
//!
//! The file id is the store's integer id and the version its ETag. Every
//! call looks the file up among that person's own files afresh, so a file
//! deleted or moved to the trash mid-session stops the next save. A save is
//! a write through the store like any other: it keeps the earlier contents
//! as a version and tells the listeners (sync clients, search) it changed.

use crate::cloud::auth::Who;
use crate::cloud::store::{self, Entry, StoreError as CloudError};
use crate::cloud::Cloud;
use crate::office::actix::{extensions_answer, open_answer, page_origin, Sessions};
use crate::office::store::{valid_name, CreateMode, FileInfo, OfficeStore, StoreError};
use crate::office::Person;
use actix_web::{web, HttpRequest, HttpResponse, Scope};
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

/// The cloud's files as the WOPI host sees them.
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
    /// One of `user`'s own files (not a folder) by its id.
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

/// `name` with " (n)" before its extension.
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
    // A session's messages go to the page that asked; another site's page
    // must not get one for this person's file.
    if crate::cloud::server::cross_site(&req) {
        return crate::cloud::api::error(
            actix_web::http::StatusCode::FORBIDDEN,
            "crossSite",
            "a request from another site",
        );
    }
    open_answer(
        person(&who),
        page_origin(&req),
        body.into_inner().file_id,
        sessions.into_inner(),
    )
    .await
}

async fn extensions(_who: Who, sessions: web::Data<Sessions>) -> HttpResponse {
    extensions_answer(sessions.into_inner()).await
}

/// `/cloud/api/office`. Needs `web::Data<Sessions>`.
pub fn scope() -> Scope {
    web::scope("/cloud/api/office")
        .route("/open", web::post().to(open))
        .route("/extensions", web::get().to(extensions))
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
        assert_eq!(info.version, e.etag);
        assert_eq!(files.read("matus", &id).unwrap(), b"one");
        assert_eq!(files.stat("eva", &id), Err(StoreError::NotFound));
        assert_eq!(files.stat("matus", "Trips"), Err(StoreError::NotFound));
        assert_eq!(files.stat("matus", "x"), Err(StoreError::NotFound));
        let folder = files.cloud.store.stat("matus", "Trips").unwrap();
        assert_eq!(
            files.stat("matus", &folder.id.to_string()),
            Err(StoreError::NotFound)
        );
    }

    #[test]
    fn a_save_is_a_new_version_and_is_heard() {
        let (_dir, files, e) = files();
        let mut heard = files.cloud.subscribe();
        let after = files.write("matus", &e.id.to_string(), b"two").unwrap();
        assert_ne!(after.version, e.etag);
        assert_eq!(files.read("matus", &e.id.to_string()).unwrap(), b"two");
        assert_eq!(heard.try_recv().unwrap().file_id, e.id);
        assert_eq!(
            files
                .cloud
                .store
                .versions("matus", "Trips/Plan.docx")
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn save_as_lands_beside_the_document() {
        let (_dir, files, e) = files();
        let id = e.id.to_string();
        let (pdf, info) = files
            .create("matus", &id, "Plan.pdf", b"%PDF", CreateMode::Unique)
            .unwrap();
        assert_eq!(info.name, "Plan.pdf");
        assert_eq!(
            files
                .cloud
                .store
                .stat("matus", "Trips/Plan.pdf")
                .unwrap()
                .id
                .to_string(),
            pdf
        );
        let (_, again) = files
            .create("matus", &id, "Plan.pdf", b"%PDF", CreateMode::Unique)
            .unwrap();
        assert_eq!(again.name, "Plan (2).pdf");
        assert_eq!(
            files.create("matus", &id, "Plan.pdf", b"", CreateMode::Exact),
            Err(StoreError::Exists)
        );
        assert_eq!(
            files.create("matus", &id, "../x.pdf", b"", CreateMode::Unique),
            Err(StoreError::InvalidName)
        );
    }

    #[test]
    fn a_rename_keeps_the_id() {
        let (_dir, files, e) = files();
        let (id, info) = files
            .rename("matus", &e.id.to_string(), "Trip.docx")
            .unwrap();
        assert_eq!(id, e.id.to_string());
        assert_eq!(info.name, "Trip.docx");
        assert!(files.cloud.store.stat("matus", "Trips/Trip.docx").is_ok());
    }
}
