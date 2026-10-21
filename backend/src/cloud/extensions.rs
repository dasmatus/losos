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
    // Editing together once `mesh::start` ran, and sharing across boxes
    // when the box talks to other boxes.
    if let Some(mesh) = cloud.mesh.get() {
        cfg.app_data(web::Data::new(mesh.clone()))
            .service(crate::cloud::collab::routes::scope());
        if mesh.networked {
            cfg.service(crate::cloud::mesh::routes::scope())
                .service(crate::cloud::mesh::routes::link_scope());
        }
    }
}

/// A call under `/cloud/api/` the files API did not take.
pub async fn api(
    req: &HttpRequest,
    cloud: &Cloud,
    who: &Who,
    segs: &[&str],
    body: web::Payload,
) -> HttpResponse {
    let _ = (req, cloud, who, segs, body);
    crate::cloud::api::error(StatusCode::NOT_FOUND, "notFound", "no such call")
}
