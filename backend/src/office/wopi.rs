//! The WOPI protocol, host side, as coolwsd 25.04 speaks it.
//!
//! | Request                                   | Operation       |
//! |-------------------------------------------|-----------------|
//! | `GET  /wopi/files/<id>`                   | CheckFileInfo   |
//! | `GET  /wopi/files/<id>/contents`          | GetFile         |
//! | `POST /wopi/files/<id>/contents` `PUT`    | PutFile         |
//! | `POST /wopi/files/<id>` `LOCK`            | Lock, or UnlockAndRelock with `X-WOPI-OldLock` |
//! | `POST /wopi/files/<id>` `GET_LOCK`        | GetLock         |
//! | `POST /wopi/files/<id>` `REFRESH_LOCK`    | RefreshLock     |
//! | `POST /wopi/files/<id>` `UNLOCK`          | Unlock          |
//! | `POST /wopi/files/<id>` `PUT_RELATIVE`    | PutRelativeFile ("Save as") |
//! | `POST /wopi/files/<id>` `RENAME_FILE`     | RenameFile      |
//!
//! (the second column of a POST is its `X-WOPI-Override` header). Every
//! request carries `?access_token=`, which must be this box's signature over
//! a grant for exactly this file. Anything else is a 401, before the store
//! is asked anything.
//!
//! [`Wopi::handle`] takes a [`WopiRequest`] and answers a [`WopiResponse`]
//! and does no I/O of its own besides the store's, so it runs the same under
//! any HTTP server and in the tests below without one.

use super::lock::{Locks, Outcome};
use super::store::{valid_name, CreateMode, OfficeStore, StoreError};
use super::token::{AccessToken, TokenKey};
use super::utf7;
use serde_json::json;
use std::sync::Arc;
use std::time::SystemTime;

/// One request to the WOPI routes.
#[derive(Debug, Clone, Default)]
pub struct WopiRequest {
    pub method: String,
    /// The path without the query: `/wopi/files/<id>[/contents]`.
    pub path: String,
    /// The query string without the `?`.
    pub query: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl WopiRequest {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// A header that is there and not empty.
    fn value(&self, name: &str) -> Option<&str> {
        self.header(name).filter(|v| !v.is_empty())
    }

    fn param(&self, name: &str) -> Option<String> {
        self.query.split('&').find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            (k == name).then(|| utf7::percent_decode(v)).flatten()
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WopiResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub content_type: Option<&'static str>,
    pub body: Vec<u8>,
}

impl WopiResponse {
    fn status(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            content_type: None,
            body: Vec::new(),
        }
    }

    fn json(status: u16, value: serde_json::Value) -> Self {
        Self {
            status,
            headers: Vec::new(),
            content_type: Some("application/json"),
            body: value.to_string().into_bytes(),
        }
    }

    fn with(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers.push((name.to_string(), value.into()));
        self
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    fn from_store(e: StoreError) -> Self {
        match e {
            StoreError::NotFound => Self::status(404),
            StoreError::Forbidden => Self::status(401),
            StoreError::Exists => Self::status(409),
            StoreError::InvalidName => {
                Self::status(400).with("X-WOPI-InvalidFileNameError", "That name is not allowed")
            }
            StoreError::Io(msg) => {
                tracing::warn!(%msg, "office store failed");
                Self::status(500)
            }
        }
    }

    fn conflict(current: String) -> Self {
        Self::status(409).with("X-WOPI-Lock", current).with(
            "X-WOPI-LockFailureReason",
            "The file is locked by another session",
        )
    }
}

/// The WOPI host: a store, the key tokens are signed with, and the locks.
pub struct Wopi {
    store: Arc<dyn OfficeStore>,
    key: TokenKey,
    locks: Locks,
    base: String,
    clock: fn() -> u64,
}

impl Wopi {
    /// `base` is the address coolwsd reaches this host on, e.g.
    /// `http://127.0.0.1:8093`; it goes into every WOPISrc.
    pub fn new(store: Arc<dyn OfficeStore>, key: TokenKey, base: &str) -> Self {
        Self {
            store,
            key,
            locks: Locks::default(),
            base: base.trim_end_matches('/').to_string(),
            clock: super::now_ms,
        }
    }

    /// The same host with another clock, for tests.
    pub fn with_clock(mut self, clock: fn() -> u64) -> Self {
        self.clock = clock;
        self
    }

    pub fn store(&self) -> &dyn OfficeStore {
        self.store.as_ref()
    }

    pub fn key(&self) -> &TokenKey {
        &self.key
    }

    /// The WOPISrc of a file.
    pub fn file_url(&self, file_id: &str) -> String {
        format!("{}/wopi/files/{}", self.base, utf7::percent_encode(file_id))
    }

    pub fn handle(&self, req: &WopiRequest) -> WopiResponse {
        let Some(rest) = req.path.strip_prefix("/wopi/files/") else {
            return WopiResponse::status(404);
        };
        let (id, contents) = match rest.strip_suffix("/contents") {
            Some(id) => (id, true),
            None => (rest, false),
        };
        let Some(id) = utf7::percent_decode(id).filter(|i| !i.is_empty() && !i.contains('/'))
        else {
            return WopiResponse::status(404);
        };
        let now = (self.clock)();
        let token = match req.param("access_token").map(|t| self.key.verify(&t, now)) {
            Some(Ok(t)) if t.file_id == id => t,
            _ => return WopiResponse::status(401),
        };
        let op = req.header("X-WOPI-Override").unwrap_or_default();
        match (req.method.as_str(), contents, op) {
            ("GET", false, _) => self.check_file_info(&token),
            ("GET", true, _) => self.get_file(&token, req),
            ("POST", true, "PUT") => self.put_file(&token, req, now),
            ("POST", false, "LOCK") => self.lock(&token, req, now),
            ("POST", false, "GET_LOCK") => {
                let current = self.locks.current(&token.file_id, now).unwrap_or_default();
                WopiResponse::status(200).with("X-WOPI-Lock", current)
            }
            ("POST", false, "REFRESH_LOCK") => {
                self.with_lock(&token, req, |l| self.locks.refresh(&token.file_id, l, now))
            }
            ("POST", false, "UNLOCK") => {
                self.with_lock(&token, req, |l| self.locks.unlock(&token.file_id, l, now))
            }
            ("POST", false, "PUT_RELATIVE") => self.put_relative(&token, req),
            ("POST", false, "RENAME_FILE") => self.rename(&token, req, now),
            ("POST", _, _) => WopiResponse::status(501),
            _ => WopiResponse::status(405),
        }
    }

    fn check_file_info(&self, token: &AccessToken) -> WopiResponse {
        let info = match self.store.stat(&token.login, &token.file_id) {
            Ok(info) => info,
            Err(e) => return WopiResponse::from_store(e),
        };
        let write = token.writable && info.writable;
        let rename = write && !self.store.ids_follow_names();
        WopiResponse::json(
            200,
            json!({
                "BaseFileName": info.name,
                "Size": info.size,
                "Version": info.version,
                "LastModifiedTime": timestamp(info.modified),
                "OwnerId": info.owner,
                "UserId": token.login,
                "UserFriendlyName": token.display_name,
                "UserCanWrite": write,
                "ReadOnly": !write,
                "UserCanNotWriteRelative": !write,
                "UserCanRename": rename,
                "SupportsRename": rename,
                "SupportsLocks": true,
                "SupportsGetLock": true,
                "SupportsExtendedLockLength": true,
                "SupportsUpdate": true,
                "PostMessageOrigin": token.origin,
                // The box has no admin pages in the editor and no links out.
                "IsAdminUser": false,
                "EnableInsertRemoteImage": false,
                "EnableShare": false,
                "HideUserList": false,
            }),
        )
    }

    fn get_file(&self, token: &AccessToken, req: &WopiRequest) -> WopiResponse {
        let info = match self.store.stat(&token.login, &token.file_id) {
            Ok(info) => info,
            Err(e) => return WopiResponse::from_store(e),
        };
        if let Some(max) = req
            .value("X-WOPI-MaxExpectedSize")
            .and_then(|v| v.parse::<u64>().ok())
        {
            if info.size > max {
                return WopiResponse::status(412);
            }
        }
        match self.store.read(&token.login, &token.file_id) {
            Ok(body) => WopiResponse {
                status: 200,
                headers: vec![("X-WOPI-ItemVersion".into(), info.version)],
                content_type: Some("application/octet-stream"),
                body,
            },
            Err(e) => WopiResponse::from_store(e),
        }
    }

    fn put_file(&self, token: &AccessToken, req: &WopiRequest, now: u64) -> WopiResponse {
        if !token.writable {
            return WopiResponse::status(401);
        }
        let info = match self.store.stat(&token.login, &token.file_id) {
            Ok(info) => info,
            Err(e) => return WopiResponse::from_store(e),
        };
        // coolwsd sends back the LastModifiedTime it loaded. Another time
        // means the file changed under the open document (a sync client, a
        // second editor on another box), and coolwsd asks the person which
        // copy to keep when it reads code 1010.
        if let Some(seen) = req
            .value("X-COOL-WOPI-Timestamp")
            .or_else(|| req.value("X-LOOL-WOPI-Timestamp"))
        {
            if seen != timestamp(info.modified) {
                tracing::info!(file = %token.file_id, "office save refused: the file changed");
                return WopiResponse::json(
                    409,
                    json!({ "COOLStatusCode": 1010, "LOOLStatusCode": 1010 }),
                );
            }
        }
        let lock = req.value("X-WOPI-Lock");
        if let Outcome::Conflict(current) =
            self.locks.may_write(&token.file_id, lock, info.size, now)
        {
            return WopiResponse::conflict(current);
        }
        match self.store.write(&token.login, &token.file_id, &req.body) {
            Ok(after) => {
                tracing::info!(file = %token.file_id, user = %token.login, size = after.size, "office save");
                WopiResponse::json(
                    200,
                    json!({ "LastModifiedTime": timestamp(after.modified) }),
                )
                .with("X-WOPI-ItemVersion", after.version)
            }
            Err(e) => WopiResponse::from_store(e),
        }
    }

    fn lock(&self, token: &AccessToken, req: &WopiRequest, now: u64) -> WopiResponse {
        if !token.writable {
            return WopiResponse::status(401);
        }
        if let Err(e) = self.store.stat(&token.login, &token.file_id) {
            return WopiResponse::from_store(e);
        }
        let Some(lock) = req.value("X-WOPI-Lock") else {
            return WopiResponse::status(400);
        };
        let outcome = match req.value("X-WOPI-OldLock") {
            Some(old) => self.locks.unlock_and_relock(&token.file_id, old, lock, now),
            None => self.locks.lock(&token.file_id, lock, now),
        };
        match outcome {
            Outcome::Ok => WopiResponse::status(200),
            Outcome::Conflict(current) => WopiResponse::conflict(current),
        }
    }

    fn with_lock(
        &self,
        token: &AccessToken,
        req: &WopiRequest,
        op: impl FnOnce(&str) -> Outcome,
    ) -> WopiResponse {
        if !token.writable {
            return WopiResponse::status(401);
        }
        let Some(lock) = req.value("X-WOPI-Lock") else {
            return WopiResponse::status(400);
        };
        match op(lock) {
            Outcome::Ok => WopiResponse::status(200),
            Outcome::Conflict(current) => WopiResponse::conflict(current),
        }
    }

    fn put_relative(&self, token: &AccessToken, req: &WopiRequest) -> WopiResponse {
        if !token.writable {
            return WopiResponse::status(401);
        }
        let current = match self.store.stat(&token.login, &token.file_id) {
            Ok(info) => info,
            Err(e) => return WopiResponse::from_store(e),
        };
        let decode = |name| req.value(name).map(utf7::decode);
        let (name, mode) = match (
            decode("X-WOPI-SuggestedTarget"),
            decode("X-WOPI-RelativeTarget"),
        ) {
            (Some(Some(s)), None) => {
                // Just an extension: the current name with that extension.
                let name = if s.starts_with('.') {
                    let stem = current
                        .name
                        .rsplit_once('.')
                        .map_or(current.name.as_str(), |(stem, _)| stem);
                    format!("{stem}{s}")
                } else {
                    s
                };
                (name, CreateMode::Unique)
            }
            // An exact name is never an overwrite here: a document someone
            // has open could be replaced under them. coolwsd sends only
            // suggestions anyway.
            (None, Some(Some(r))) => (r, CreateMode::Exact),
            _ => return WopiResponse::status(400),
        };
        if !valid_name(&name) {
            return WopiResponse::from_store(StoreError::InvalidName);
        }
        let created = self
            .store
            .create(&token.login, &token.file_id, &name, &req.body, mode);
        let (new_id, info) = match created {
            Ok(done) => done,
            Err(StoreError::Exists) => {
                return WopiResponse::status(409)
                    .with("X-WOPI-ValidRelativeTarget", utf7_safe(&name));
            }
            Err(e) => return WopiResponse::from_store(e),
        };
        let grant = AccessToken {
            file_id: new_id.clone(),
            ..token.clone()
        };
        let url = format!(
            "{}?access_token={}",
            self.file_url(&new_id),
            self.key.sign(&grant)
        );
        tracing::info!(file = %new_id, user = %token.login, "office save as");
        WopiResponse::json(200, json!({ "Name": info.name, "Url": url }))
    }

    fn rename(&self, token: &AccessToken, req: &WopiRequest, now: u64) -> WopiResponse {
        if !token.writable || self.store.ids_follow_names() {
            return WopiResponse::status(if token.writable { 501 } else { 401 });
        }
        let Some(Some(requested)) = req.value("X-WOPI-RequestedName").map(utf7::decode) else {
            return WopiResponse::status(400);
        };
        let info = match self.store.stat(&token.login, &token.file_id) {
            Ok(info) => info,
            Err(e) => return WopiResponse::from_store(e),
        };
        if let Some(held) = self.locks.current(&token.file_id, now) {
            if req.value("X-WOPI-Lock") != Some(held.as_str()) {
                return WopiResponse::conflict(held);
            }
        }
        let new_name = match super::extension(&info.name) {
            Some(ext) => format!("{requested}.{ext}"),
            None => requested.clone(),
        };
        match self.store.rename(&token.login, &token.file_id, &new_name) {
            Ok((new_id, _)) => {
                if new_id != token.file_id {
                    self.locks.moved(&token.file_id, &new_id);
                }
                WopiResponse::json(200, json!({ "Name": requested }))
            }
            Err(e) => WopiResponse::from_store(e),
        }
    }
}

/// A name for `X-WOPI-ValidRelativeTarget`, which must be ASCII: the taken
/// name with a number, UTF-7 where it needs it.
fn utf7_safe(name: &str) -> String {
    let free = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => format!("{stem} (2).{ext}"),
        _ => format!("{name} (2)"),
    };
    if free.is_ascii() {
        free.replace('+', "+-")
    } else {
        // Rare enough, and only a hint: drop what is not ASCII.
        free.chars().filter(char::is_ascii).collect()
    }
}

/// ISO 8601 in UTC with microseconds, the form coolwsd reads and echoes back.
pub fn timestamp(t: SystemTime) -> String {
    chrono::DateTime::<chrono::Utc>::from(t)
        .format("%Y-%m-%dT%H:%M:%S%.6fZ")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::office::store::DirStore;
    use std::cell::Cell;

    thread_local! {
        static NOW: Cell<u64> = const { Cell::new(1_000_000) };
    }

    fn now() -> u64 {
        NOW.with(Cell::get)
    }

    fn advance(ms: u64) {
        NOW.with(|n| n.set(n.get() + ms));
    }

    struct Rig {
        dir: assert_fs::TempDir,
        wopi: Wopi,
    }

    fn rig() -> Rig {
        let dir = assert_fs::TempDir::new().unwrap();
        std::fs::write(dir.path().join("Plan.docx"), b"version one").unwrap();
        std::fs::write(dir.path().join("Empty.xlsx"), b"").unwrap();
        let wopi = Wopi::new(
            Arc::new(DirStore::new(dir.path())),
            TokenKey::from_bytes(&[9; 32]),
            "http://127.0.0.1:8093/",
        )
        .with_clock(now);
        Rig { dir, wopi }
    }

    fn grant(file: &str, login: &str, writable: bool) -> AccessToken {
        AccessToken {
            file_id: DirStore::id_for(file),
            login: login.into(),
            display_name: login.to_uppercase(),
            writable,
            origin: "https://box.local".into(),
            expires_ms: now() + 60_000,
        }
    }

    impl Rig {
        fn token(&self, file: &str, login: &str, writable: bool) -> String {
            self.wopi.key().sign(&grant(file, login, writable))
        }

        fn call(
            &self,
            method: &str,
            file: &str,
            suffix: &str,
            token: &str,
            headers: &[(&str, &str)],
            body: &[u8],
        ) -> WopiResponse {
            self.wopi.handle(&WopiRequest {
                method: method.into(),
                path: format!("/wopi/files/{}{suffix}", DirStore::id_for(file)),
                query: format!("access_token={token}&access_token_ttl=0"),
                headers: headers
                    .iter()
                    .map(|(n, v)| (n.to_string(), v.to_string()))
                    .collect(),
                body: body.to_vec(),
            })
        }

        fn info(&self, file: &str, token: &str) -> serde_json::Value {
            let r = self.call("GET", file, "", token, &[], b"");
            assert_eq!(r.status, 200);
            serde_json::from_slice(&r.body).unwrap()
        }

        fn lock(&self, file: &str, token: &str, lock: &str) -> WopiResponse {
            self.call(
                "POST",
                file,
                "",
                token,
                &[("X-WOPI-Override", "LOCK"), ("X-WOPI-Lock", lock)],
                b"",
            )
        }

        fn save(
            &self,
            file: &str,
            token: &str,
            extra: &[(&str, &str)],
            body: &[u8],
        ) -> WopiResponse {
            let mut headers = vec![("X-WOPI-Override", "PUT")];
            headers.extend_from_slice(extra);
            self.call("POST", file, "/contents", token, &headers, body)
        }

        fn read(&self, file: &str) -> Vec<u8> {
            std::fs::read(self.dir.path().join(file)).unwrap()
        }
    }

    #[test]
    fn check_file_info_describes_the_file_and_the_person() {
        let r = rig();
        let t = r.token("Plan.docx", "matus", true);
        let info = r.info("Plan.docx", &t);
        assert_eq!(info["BaseFileName"], "Plan.docx");
        assert_eq!(info["Size"], 11);
        assert_eq!(info["UserId"], "matus");
        assert_eq!(info["UserFriendlyName"], "MATUS");
        assert_eq!(info["UserCanWrite"], true);
        assert_eq!(info["SupportsLocks"], true);
        assert_eq!(info["SupportsUpdate"], true);
        assert_eq!(info["PostMessageOrigin"], "https://box.local");
        assert_eq!(info["IsAdminUser"], false);
        // A folder store's ids follow names: no rename from the editor.
        assert_eq!(info["SupportsRename"], false);
        let modified = info["LastModifiedTime"].as_str().unwrap();
        assert!(
            modified.ends_with('Z') && modified.len() == 27,
            "{modified}"
        );

        let ro = r.token("Plan.docx", "guest", false);
        let info = r.info("Plan.docx", &ro);
        assert_eq!(info["UserCanWrite"], false);
        assert_eq!(info["ReadOnly"], true);
        assert_eq!(info["UserCanNotWriteRelative"], true);
    }

    #[test]
    fn get_file_returns_the_bytes_and_the_version() {
        let r = rig();
        let t = r.token("Plan.docx", "matus", false);
        let got = r.call("GET", "Plan.docx", "/contents", &t, &[], b"");
        assert_eq!(got.status, 200);
        assert_eq!(got.body, b"version one");
        let version = got.header("X-WOPI-ItemVersion").unwrap();
        assert_eq!(r.info("Plan.docx", &t)["Version"], version);
        let small = r.call(
            "GET",
            "Plan.docx",
            "/contents",
            &t,
            &[("X-WOPI-MaxExpectedSize", "5")],
            b"",
        );
        assert_eq!(small.status, 412);
    }

    #[test]
    fn a_token_opens_its_own_file_only_and_only_while_it_lasts() {
        let r = rig();
        let t = r.token("Plan.docx", "matus", true);
        assert_eq!(r.call("GET", "Empty.xlsx", "", &t, &[], b"").status, 401);
        assert_eq!(r.call("GET", "Plan.docx", "", "nope", &[], b"").status, 401);
        let other_box = TokenKey::from_bytes(&[1; 32]).sign(&grant("Plan.docx", "matus", true));
        assert_eq!(
            r.call("GET", "Plan.docx", "", &other_box, &[], b"").status,
            401
        );
        let no_token = r.wopi.handle(&WopiRequest {
            method: "GET".into(),
            path: format!("/wopi/files/{}", DirStore::id_for("Plan.docx")),
            ..Default::default()
        });
        assert_eq!(no_token.status, 401);
        advance(60_001);
        assert_eq!(r.call("GET", "Plan.docx", "", &t, &[], b"").status, 401);
    }

    #[test]
    fn paths_outside_the_routes_are_not_found() {
        let r = rig();
        for path in [
            "/wopi/files/",
            "/wopi/files/a/b",
            "/other",
            "/wopi/files//contents",
        ] {
            let got = r.wopi.handle(&WopiRequest {
                method: "GET".into(),
                path: path.into(),
                ..Default::default()
            });
            assert_eq!(got.status, 404, "{path}");
        }
    }

    #[test]
    fn a_save_needs_the_lock_and_writes_the_file() {
        let r = rig();
        let t = r.token("Plan.docx", "matus", true);
        assert_eq!(r.lock("Plan.docx", &t, "L1").status, 200);
        let saved = r.save("Plan.docx", &t, &[("X-WOPI-Lock", "L1")], b"version two");
        assert_eq!(saved.status, 200);
        assert_eq!(r.read("Plan.docx"), b"version two");
        let body: serde_json::Value = serde_json::from_slice(&saved.body).unwrap();
        assert_eq!(
            body["LastModifiedTime"],
            r.info("Plan.docx", &t)["LastModifiedTime"]
        );
        assert_eq!(
            saved.header("X-WOPI-ItemVersion").unwrap(),
            r.info("Plan.docx", &t)["Version"]
        );

        // Another session's lock, or none, on a file that is not empty.
        let stale = r.save("Plan.docx", &t, &[("X-WOPI-Lock", "L0")], b"lost");
        assert_eq!(stale.status, 409);
        assert_eq!(stale.header("X-WOPI-Lock"), Some("L1"));
        assert_eq!(r.save("Plan.docx", &t, &[], b"lost").status, 409);
        assert_eq!(r.read("Plan.docx"), b"version two");
    }

    #[test]
    fn an_empty_file_takes_its_first_save_without_a_lock() {
        let r = rig();
        let t = r.token("Empty.xlsx", "matus", true);
        assert_eq!(r.save("Empty.xlsx", &t, &[], b"cells").status, 200);
        assert_eq!(r.read("Empty.xlsx"), b"cells");
    }

    #[test]
    fn a_read_only_token_cannot_lock_or_save() {
        let r = rig();
        let t = r.token("Plan.docx", "guest", false);
        assert_eq!(r.lock("Plan.docx", &t, "L1").status, 401);
        assert_eq!(
            r.save("Plan.docx", &t, &[("X-WOPI-Lock", "L1")], b"x")
                .status,
            401
        );
        let rel = r.call(
            "POST",
            "Plan.docx",
            "",
            &t,
            &[
                ("X-WOPI-Override", "PUT_RELATIVE"),
                ("X-WOPI-SuggestedTarget", ".pdf"),
            ],
            b"pdf",
        );
        assert_eq!(rel.status, 401);
        assert_eq!(r.read("Plan.docx"), b"version one");
    }

    #[test]
    fn a_save_over_a_file_that_changed_meanwhile_is_coolwsds_conflict() {
        let r = rig();
        let t = r.token("Plan.docx", "matus", true);
        let loaded = r.info("Plan.docx", &t)["LastModifiedTime"]
            .as_str()
            .unwrap()
            .to_string();
        r.lock("Plan.docx", &t, "L1");
        // Someone else's copy lands (a sync client, say) a moment later.
        std::thread::sleep(std::time::Duration::from_millis(5));
        std::fs::write(r.dir.path().join("Plan.docx"), b"from elsewhere").unwrap();
        let stale = r.save(
            "Plan.docx",
            &t,
            &[("X-WOPI-Lock", "L1"), ("X-COOL-WOPI-Timestamp", &loaded)],
            b"mine",
        );
        assert_eq!(stale.status, 409);
        let body: serde_json::Value = serde_json::from_slice(&stale.body).unwrap();
        assert_eq!(body["COOLStatusCode"], 1010);
        assert_eq!(r.read("Plan.docx"), b"from elsewhere");

        // With the time it has now, the save goes through.
        let current = r.info("Plan.docx", &t)["LastModifiedTime"]
            .as_str()
            .unwrap()
            .to_string();
        let ok = r.save(
            "Plan.docx",
            &t,
            &[("X-WOPI-Lock", "L1"), ("X-COOL-WOPI-Timestamp", &current)],
            b"mine",
        );
        assert_eq!(ok.status, 200);
    }

    #[test]
    fn two_people_share_one_coolwsd_lock_and_the_second_lock_conflicts() {
        let r = rig();
        let a = r.token("Plan.docx", "alice", true);
        let b = r.token("Plan.docx", "bob", true);
        assert_eq!(r.lock("Plan.docx", &a, "doc-session").status, 200);
        // coolwsd edits one document for everyone in it with one lock.
        assert_eq!(r.lock("Plan.docx", &b, "doc-session").status, 200);
        let other = r.lock("Plan.docx", &b, "another-server");
        assert_eq!(other.status, 409);
        assert_eq!(other.header("X-WOPI-Lock"), Some("doc-session"));

        let get = r.call(
            "POST",
            "Plan.docx",
            "",
            &b,
            &[("X-WOPI-Override", "GET_LOCK")],
            b"",
        );
        assert_eq!(get.header("X-WOPI-Lock"), Some("doc-session"));

        let refresh = r.call(
            "POST",
            "Plan.docx",
            "",
            &a,
            &[
                ("X-WOPI-Override", "REFRESH_LOCK"),
                ("X-WOPI-Lock", "doc-session"),
            ],
            b"",
        );
        assert_eq!(refresh.status, 200);
        let relock = r.call(
            "POST",
            "Plan.docx",
            "",
            &a,
            &[
                ("X-WOPI-Override", "LOCK"),
                ("X-WOPI-OldLock", "doc-session"),
                ("X-WOPI-Lock", "doc-session-2"),
            ],
            b"",
        );
        assert_eq!(relock.status, 200);
        let unlock = r.call(
            "POST",
            "Plan.docx",
            "",
            &a,
            &[
                ("X-WOPI-Override", "UNLOCK"),
                ("X-WOPI-Lock", "doc-session-2"),
            ],
            b"",
        );
        assert_eq!(unlock.status, 200);
        let get = r.call(
            "POST",
            "Plan.docx",
            "",
            &b,
            &[("X-WOPI-Override", "GET_LOCK")],
            b"",
        );
        assert_eq!(get.header("X-WOPI-Lock"), Some(""));
        assert_eq!(r.lock("Plan.docx", &b, "").status, 400);
    }

    #[test]
    fn save_as_lands_beside_the_file_with_a_token_for_the_copy() {
        let r = rig();
        let t = r.token("Plan.docx", "matus", true);
        let got = r.call(
            "POST",
            "Plan.docx",
            "",
            &t,
            &[
                ("X-WOPI-Override", "PUT_RELATIVE"),
                ("X-WOPI-SuggestedTarget", ".pdf"),
            ],
            b"%PDF",
        );
        assert_eq!(got.status, 200);
        let body: serde_json::Value = serde_json::from_slice(&got.body).unwrap();
        assert_eq!(body["Name"], "Plan.pdf");
        assert_eq!(r.read("Plan.pdf"), b"%PDF");
        let url = body["Url"].as_str().unwrap();
        let prefix = format!(
            "http://127.0.0.1:8093/wopi/files/{}?access_token=",
            DirStore::id_for("Plan.pdf")
        );
        assert!(url.starts_with(&prefix), "{url}");
        let copy = r.wopi.key().verify(&url[prefix.len()..], now()).unwrap();
        assert_eq!(copy.file_id, DirStore::id_for("Plan.pdf"));
        assert_eq!(copy.login, "matus");

        // A UTF-7 name, and a taken one gets a number.
        let again = r.call(
            "POST",
            "Plan.docx",
            "",
            &t,
            &[
                ("X-WOPI-Override", "PUT_RELATIVE"),
                ("X-WOPI-SuggestedTarget", "Zpr+AOE-va.odt"),
            ],
            b"odt",
        );
        let body: serde_json::Value = serde_json::from_slice(&again.body).unwrap();
        assert_eq!(body["Name"], "Zpráva.odt");
        let taken = r.call(
            "POST",
            "Plan.docx",
            "",
            &t,
            &[
                ("X-WOPI-Override", "PUT_RELATIVE"),
                ("X-WOPI-SuggestedTarget", "Plan.pdf"),
            ],
            b"x",
        );
        let body: serde_json::Value = serde_json::from_slice(&taken.body).unwrap();
        assert_eq!(body["Name"], "Plan (2).pdf");

        // An exact name that is taken is a conflict with a free one offered.
        let exact = r.call(
            "POST",
            "Plan.docx",
            "",
            &t,
            &[
                ("X-WOPI-Override", "PUT_RELATIVE"),
                ("X-WOPI-RelativeTarget", "Plan.pdf"),
            ],
            b"x",
        );
        assert_eq!(exact.status, 409);
        assert_eq!(
            exact.header("X-WOPI-ValidRelativeTarget"),
            Some("Plan (2).pdf")
        );
        assert_eq!(r.read("Plan.pdf"), b"%PDF");

        for bad in ["../evil.docx", "a/b.docx", "+"] {
            let got = r.call(
                "POST",
                "Plan.docx",
                "",
                &t,
                &[
                    ("X-WOPI-Override", "PUT_RELATIVE"),
                    ("X-WOPI-SuggestedTarget", bad),
                ],
                b"x",
            );
            assert_eq!(got.status, 400, "{bad}");
        }
    }

    #[test]
    fn rename_is_not_offered_on_a_folder_store_and_unknown_operations_are_501() {
        let r = rig();
        let t = r.token("Plan.docx", "matus", true);
        let rename = r.call(
            "POST",
            "Plan.docx",
            "",
            &t,
            &[
                ("X-WOPI-Override", "RENAME_FILE"),
                ("X-WOPI-RequestedName", "Budget"),
            ],
            b"",
        );
        assert_eq!(rename.status, 501);
        let delete = r.call(
            "POST",
            "Plan.docx",
            "",
            &t,
            &[("X-WOPI-Override", "DELETE")],
            b"",
        );
        assert_eq!(delete.status, 501);
        assert_eq!(r.call("PUT", "Plan.docx", "", &t, &[], b"").status, 405);
    }

    /// A store whose ids survive a rename, like LosOS cloud's.
    struct Fixed(DirStore);

    impl OfficeStore for Fixed {
        fn stat(&self, u: &str, id: &str) -> Result<crate::office::FileInfo, StoreError> {
            self.0.stat(u, id)
        }
        fn read(&self, u: &str, id: &str) -> Result<Vec<u8>, StoreError> {
            self.0.read(u, id)
        }
        fn write(
            &self,
            u: &str,
            id: &str,
            b: &[u8],
        ) -> Result<crate::office::FileInfo, StoreError> {
            self.0.write(u, id, b)
        }
        fn create(
            &self,
            u: &str,
            beside: &str,
            name: &str,
            b: &[u8],
            mode: CreateMode,
        ) -> Result<(String, crate::office::FileInfo), StoreError> {
            self.0.create(u, beside, name, b, mode)
        }
        fn rename(
            &self,
            u: &str,
            id: &str,
            name: &str,
        ) -> Result<(String, crate::office::FileInfo), StoreError> {
            self.0.rename(u, id, name)
        }
    }

    #[test]
    fn rename_keeps_the_extension_and_respects_the_lock() {
        let dir = assert_fs::TempDir::new().unwrap();
        std::fs::write(dir.path().join("Plan.docx"), b"x").unwrap();
        let wopi = Wopi::new(
            Arc::new(Fixed(DirStore::new(dir.path()))),
            TokenKey::from_bytes(&[9; 32]),
            "http://127.0.0.1:8093",
        )
        .with_clock(now);
        let r = Rig { dir, wopi };
        let t = r.token("Plan.docx", "matus", true);
        assert_eq!(r.info("Plan.docx", &t)["SupportsRename"], true);
        r.lock("Plan.docx", &t, "L1");
        let rename = |lock: &str| {
            r.call(
                "POST",
                "Plan.docx",
                "",
                &t,
                &[
                    ("X-WOPI-Override", "RENAME_FILE"),
                    ("X-WOPI-RequestedName", "Rozpo+AQ0-et"),
                    ("X-WOPI-Lock", lock),
                ],
                b"",
            )
        };
        assert_eq!(rename("L0").status, 409);
        let done = rename("L1");
        assert_eq!(done.status, 200);
        let body: serde_json::Value = serde_json::from_slice(&done.body).unwrap();
        assert_eq!(body["Name"], "Rozpočet");
        assert_eq!(r.read("Rozpočet.docx"), b"x");
    }
}
