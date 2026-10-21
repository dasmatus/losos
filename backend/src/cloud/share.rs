//! Sharing. The table is here from the start so the files and their ids
//! never need moving; the calls and the link pages come with it.

use crate::cloud::store::{Store, StoreError};
use crate::cloud::Cloud;
use serde_json::{json, Value};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS shares (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  owner         TEXT    NOT NULL,
  file_id       INTEGER NOT NULL,
  kind          TEXT    NOT NULL,
  recipient     TEXT,
  perm          TEXT    NOT NULL,
  token         TEXT    UNIQUE,
  password_hash TEXT,
  expires       INTEGER,
  note          TEXT    NOT NULL DEFAULT '',
  created       INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS shares_owner ON shares (owner);
CREATE INDEX IF NOT EXISTS shares_recipient ON shares (recipient);
CREATE INDEX IF NOT EXISTS shares_file ON shares (file_id);
"#;

pub fn init(store: &Store) -> Result<(), StoreError> {
    store.with_db(|db| Ok(db.execute_batch(SCHEMA)?))
}

/// The `files_sharing` capability the apps read.
pub fn capabilities(_cloud: &Cloud) -> Value {
    json!({ "api_enabled": false })
}

/// The pages and WebDAV of a link share, which need no sign-in.
pub fn public(
    _req: &actix_web::HttpRequest,
    _cloud: &Cloud,
    _method: &str,
    _path: &str,
) -> Option<actix_web::HttpResponse> {
    None
}

/// The OCS share API the apps' share buttons call.
pub async fn ocs(
    _req: &actix_web::HttpRequest,
    _cloud: &Cloud,
    _who: &crate::cloud::auth::Who,
    _body: actix_web::web::Payload,
) -> Option<actix_web::HttpResponse> {
    None
}
