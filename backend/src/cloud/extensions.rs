//! Where the later parts of the cloud hang off the server: their tables at
//! start, their own actix scopes, and calls under `/cloud/api/` the files
//! API does not take.
//!
//! A scope added in [`scopes`] takes the person as a [`Who`] argument (an
//! extractor, `server.rs`), which answers 401 for nobody, the same as every
//! other route; `web::Data<Arc<Cloud>>` is in the app data for it too.

use crate::cloud::auth::Who;
use crate::cloud::store::{Store, StoreError};
use crate::cloud::Cloud;
use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse};

pub fn init(store: &Store) -> Result<(), StoreError> {
    crate::cloud::share::init(store)?;
    Ok(())
}

/// The scopes of the later parts, mounted ahead of the catch-all handler,
/// so a path one of them claims is theirs.
pub fn scopes(cfg: &mut web::ServiceConfig, cloud: &std::sync::Arc<Cloud>) {
    let _ = (cfg, cloud);
}

/// A call under `/cloud/api/` the files API did not take.
pub async fn api(
    req: &HttpRequest,
    cloud: &Cloud,
    who: &Who,
    segs: &[&str],
    body: web::Payload,
) -> HttpResponse {
    let _ = body;
    let store = &cloud.store;
    match (req.method().as_str(), segs) {
        ("GET", ["boards"]) => match crate::cloud::boards::list(store, &who.login) {
            Ok(list) => HttpResponse::Ok().json(serde_json::json!({ "boards": list })),
            Err(e) => crate::cloud::api::store_error(&e),
        },
        ("GET", ["boards", id]) => match id.parse::<i64>() {
            Ok(id) => match crate::cloud::boards::get(store, &who.login, id) {
                Ok(b) => HttpResponse::Ok().json(b),
                Err(e) => crate::cloud::api::store_error(&e),
            },
            Err(_) => crate::cloud::api::error(StatusCode::NOT_FOUND, "notFound", "no such board"),
        },
        // Calendars and address books with how much is in each, for the
        // web app's overview; the entries themselves go over CalDAV.
        ("GET", ["collections"]) => {
            use crate::cloud::pim::{self, Kind};
            if let Err(e) = pim::ensure_defaults(store, &who.login) {
                return crate::cloud::api::store_error(&e);
            }
            let mut out = Vec::new();
            for kind in [Kind::Calendar, Kind::AddressBook] {
                let list = match pim::collections(store, &who.login, kind) {
                    Ok(l) => l,
                    Err(e) => return crate::cloud::api::store_error(&e),
                };
                for c in list {
                    out.push(serde_json::json!({
                        "kind": kind.as_str(),
                        "uri": c.uri,
                        "name": c.name,
                        "color": c.color,
                        "components": c.components,
                        "count": pim::count(store, &c).unwrap_or(0),
                    }));
                }
            }
            HttpResponse::Ok().json(serde_json::json!({ "collections": out }))
        }
        _ => crate::cloud::api::error(StatusCode::NOT_FOUND, "notFound", "no such call"),
    }
}
