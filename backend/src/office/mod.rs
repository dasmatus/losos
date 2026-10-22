//! Office documents in LosOS cloud: the WOPI host for Collabora Online.
//!
//! Collabora Online (`coolwsd`, `modules/office.nix`) renders and edits Word,
//! Excel, PowerPoint and OpenDocument files in the browser. It keeps no files
//! of its own: it fetches a document from a WOPI host, the server that owns
//! the file, and writes it back there. This module is that host, written
//! against a store trait so LosOS cloud's file store and a plain folder both
//! plug in.
//!
//! ```text
//!  browser ─ /cloud/office/<id> ──► cloud ─ open() ──► {url, token}
//!     │
//!     └─ iframe /browser/…/cool.html?WOPISrc=…&access_token=… ─► coolwsd
//!                                                                │
//!          127.0.0.1 only:  GET/POST /wopi/files/<id>?access_token=…
//!                                                                ▼
//!                                         Wopi::handle ─► dyn OfficeStore
//! ```
//!
//! * [`token`]: the access token. HMAC-signed, bound to one file, one person
//!   and a write flag, and short-lived. The token is the whole
//!   authentication of the WOPI routes, which is why they listen on loopback
//!   only and nginx never routes to them: the browser hands the token to
//!   coolwsd and coolwsd alone calls the host.
//! * [`wopi`]: the protocol, as a plain request in and response out, so the
//!   cloud service and the standalone `losos-office` binary mount it the same
//!   way ([`actix`] does that for actix-web).
//! * [`lock`]: WOPI locks, which is what lets two people edit one document
//!   together without one save undoing the other's.
//! * [`discovery`]: which file types Collabora opens, and the address of its
//!   editor for each.
//! * [`store`]: the trait a file store implements, and [`store::DirStore`],
//!   a folder on disk.
//!
//! Proof keys (`X-WOPI-Proof`) are not checked. They let a WOPI host on the
//! internet tell the office server's requests from anyone else's; this host
//! is on 127.0.0.1, where the token already decides, and the nixpkgs build of
//! coolwsd reads its proof key from a path inside the read-only store.

pub mod actix;
pub mod discovery;
pub mod lock;
pub mod store;
pub mod token;
pub mod utf7;
pub mod wopi;

pub use discovery::Discovery;
pub use store::{FileInfo, OfficeStore, StoreError};
pub use token::{AccessToken, TokenKey};
pub use wopi::{Wopi, WopiRequest, WopiResponse};

use serde::{Deserialize, Serialize};

/// How long an editor session's token is good for. Collabora asks for a new
/// session when it runs out, which the editor page answers by reloading; a
/// working day is long enough that nobody sees that.
pub const TOKEN_TTL_MS: u64 = 10 * 60 * 60 * 1000;

/// What the cloud's `POST /cloud/api/office/open` answers: everything the
/// editor page needs to load the document in its frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenSession {
    /// The editor's address on this box, same origin as the page:
    /// `/browser/<hash>/cool.html?WOPISrc=…`. The page appends the token.
    pub url: String,
    pub access_token: String,
    /// When the token runs out, in milliseconds since 1970, as WOPI spells it.
    pub access_token_ttl: u64,
    /// Whether the person may change the file. A read-only file still opens.
    pub writable: bool,
    /// The file's name, for the page's title.
    pub name: String,
    /// Whether the editor works only on the box's https address: coolwsd
    /// sits behind nginx's TLS and points the page at https and wss. The
    /// page moves itself to https first when it was opened over http.
    pub https: bool,
}

/// Why a document could not be opened.
#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error(transparent)]
    Store(#[from] StoreError),
    /// Collabora does not open this type of file.
    #[error("Office does not open .{0} files")]
    Unsupported(String),
}

/// Who asks to open a document, as the cloud knows them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    /// The LosOS Login name: the `user` every store call gets.
    pub login: String,
    /// What the other people editing the document see.
    pub display_name: String,
}

/// Start an editor session for `file_id`: check that `person` may see the
/// file, find Collabora's editor for its type and sign a token for it.
///
/// `origin` is the page's own origin (`https://box.local`), which Collabora
/// needs to post messages back to it. `now_ms` is the clock, passed in so the
/// tests can hold it still.
pub fn open(
    wopi: &Wopi,
    discovery: &Discovery,
    person: &Person,
    file_id: &str,
    origin: &str,
    now_ms: u64,
) -> Result<OpenSession, OpenError> {
    let info = wopi.store().stat(&person.login, file_id)?;
    let ext = extension(&info.name).unwrap_or_default();
    let action = discovery
        .action(&ext)
        .ok_or_else(|| OpenError::Unsupported(ext.clone()))?;
    let writable = info.writable && action.edit;
    let expires = now_ms + TOKEN_TTL_MS;
    let token = AccessToken {
        file_id: file_id.to_string(),
        login: person.login.clone(),
        display_name: person.display_name.clone(),
        writable,
        origin: origin.to_string(),
        expires_ms: expires,
    };
    let src = wopi.file_url(file_id);
    let url = format!("{}WOPISrc={}", action.url_path, utf7::percent_encode(&src));
    Ok(OpenSession {
        url,
        access_token: wopi.key().sign(&token),
        access_token_ttl: expires,
        writable,
        name: info.name,
        https: action.https,
    })
}

/// The extension of a file name, lower case, without the dot.
pub fn extension(name: &str) -> Option<String> {
    let (stem, ext) = name.rsplit_once('.')?;
    if stem.is_empty() || ext.is_empty() {
        return None;
    }
    Some(ext.to_ascii_lowercase())
}

/// Milliseconds since 1970, the clock every caller outside the tests passes.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::office::store::DirStore;
    use std::sync::Arc;

    const DISCOVERY: &str = r#"<wopi-discovery><net-zone name="external-http">
<app name="writer"><action default="true" ext="docx" name="edit" urlsrc="http://127.0.0.1:9980/browser/abc/cool.html?"/>
<action ext="pdf" name="view" urlsrc="http://127.0.0.1:9980/browser/abc/cool.html?"/></app>
</net-zone></wopi-discovery>"#;

    fn setup() -> (assert_fs::TempDir, Wopi, Discovery) {
        let dir = assert_fs::TempDir::new().unwrap();
        std::fs::write(dir.path().join("Report.docx"), b"doc").unwrap();
        std::fs::write(dir.path().join("Scan.pdf"), b"pdf").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"txt").unwrap();
        let store = Arc::new(DirStore::new(dir.path()));
        let wopi = Wopi::new(
            store,
            TokenKey::from_bytes(&[7; 32]),
            "http://127.0.0.1:8093",
        );
        (dir, wopi, Discovery::parse(DISCOVERY))
    }

    fn person() -> Person {
        Person {
            login: "matus".into(),
            display_name: "Matus".into(),
        }
    }

    #[test]
    fn open_signs_a_token_for_the_file_and_points_at_the_editor() {
        let (_dir, wopi, discovery) = setup();
        let id = DirStore::id_for("Report.docx");
        let s = open(
            &wopi,
            &discovery,
            &person(),
            &id,
            "https://box.local",
            1_000,
        )
        .unwrap();
        assert_eq!(
            s.url,
            format!(
                "/browser/abc/cool.html?WOPISrc=http%3A%2F%2F127.0.0.1%3A8093%2Fwopi%2Ffiles%2F{id}"
            )
        );
        assert!(s.writable);
        assert_eq!(s.name, "Report.docx");
        assert_eq!(s.access_token_ttl, 1_000 + TOKEN_TTL_MS);
        let t = wopi.key().verify(&s.access_token, 2_000).unwrap();
        assert_eq!(t.file_id, id);
        assert_eq!(t.login, "matus");
        assert_eq!(t.origin, "https://box.local");
    }

    #[test]
    fn a_type_collabora_only_views_opens_read_only() {
        let (_dir, wopi, discovery) = setup();
        let s = open(
            &wopi,
            &discovery,
            &person(),
            &DirStore::id_for("Scan.pdf"),
            "o",
            0,
        )
        .unwrap();
        assert!(!s.writable);
        let t = wopi.key().verify(&s.access_token, 0).unwrap();
        assert!(!t.writable);
    }

    #[test]
    fn a_type_collabora_does_not_list_is_refused() {
        let (_dir, wopi, discovery) = setup();
        let err = open(
            &wopi,
            &discovery,
            &person(),
            &DirStore::id_for("notes.txt"),
            "o",
            0,
        )
        .unwrap_err();
        assert!(matches!(err, OpenError::Unsupported(ext) if ext == "txt"));
    }

    #[test]
    fn a_missing_file_is_the_store_error() {
        let (_dir, wopi, discovery) = setup();
        let err = open(
            &wopi,
            &discovery,
            &person(),
            &DirStore::id_for("gone.docx"),
            "o",
            0,
        )
        .unwrap_err();
        assert!(matches!(err, OpenError::Store(StoreError::NotFound)));
    }

    #[test]
    fn extensions_are_lower_case_and_need_a_stem() {
        assert_eq!(extension("A.DOCX").as_deref(), Some("docx"));
        assert_eq!(extension("archive.tar.gz").as_deref(), Some("gz"));
        assert_eq!(extension(".bashrc"), None);
        assert_eq!(extension("README"), None);
        assert_eq!(extension("trailing."), None);
    }
}
