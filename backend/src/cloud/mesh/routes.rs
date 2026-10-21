//! The mesh's HTTP side.
//!
//! `/cloud/api/mesh` is for the people signed in here: share with someone
//! on another box, see and answer what others shared, read shared folders.
//! `/mesh/v1/link` is for other boxes, at the vhost root and reachable from
//! the internet through an edge: it opens a link, and nothing is answered
//! until the link's handshake has checked the other box's key.

use std::sync::Arc;

use actix_web::{web, FromRequest, HttpRequest, HttpResponse, Scope};
use actix_ws::AggregatedMessage;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;

use super::dial::MAX_FRAME;
use super::link::Frame;
use super::{Mesh, MeshError, Person};
use crate::cloud::auth::{Via, Who};
use crate::cloud::collab::routes::error;
use crate::cloud::collab::store::{FileId, Perm};
use crate::cloud::store::valid_owner;

/// The person's LosOS Login name, which nginx sends beside the cloud login
/// (`X-Losos-Cloud-User`). The two differ for the box's owner, whose files
/// are `notshared`'s, and an address on the mesh carries the first.
pub const LOGIN_HEADER: &str = "x-losos-cloud-login";

/// A page on another site asking with the person's session: any request,
/// a WebSocket's opening GET included, whose `Origin` is not this host.
fn foreign_origin(req: &HttpRequest) -> bool {
    let Some(origin) = req.headers().get("origin").and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let host = req
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let origin_host = origin.split_once("://").map_or(origin, |(_, h)| h);
    origin == "null" || !origin_host.trim_end_matches('/').eq_ignore_ascii_case(host)
}

impl FromRequest for Person {
    type Error = actix_web::Error;
    type Future = std::future::Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, payload: &mut actix_web::dev::Payload) -> Self::Future {
        let who = match Who::from_request(req, payload).into_inner() {
            Ok(w) => w,
            Err(e) => return std::future::ready(Err(e)),
        };
        if foreign_origin(req) {
            return std::future::ready(Err(actix_web::error::ErrorForbidden(
                "a request from another site",
            )));
        }
        // Only nginx names the LosOS Login name, as it names the person.
        let public = match who.via {
            Via::Header => req
                .headers()
                .get(LOGIN_HEADER)
                .and_then(|v| v.to_str().ok())
                .filter(|l| valid_owner(l) && !l.contains('@'))
                .map(str::to_string),
            Via::AppPassword(_) => None,
        }
        .unwrap_or_else(|| who.login.clone());
        let name = if who.name.trim().is_empty() {
            public.clone()
        } else {
            who.name.trim().chars().take(120).collect()
        };
        std::future::ready(Ok(Person {
            login: who.login,
            public,
            name,
        }))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewShare {
    file_id: FileId,
    /// `eva@kitchenbox.example`.
    to: String,
    perm: Perm,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ShareQuery {
    file_id: Option<FileId>,
}

#[derive(Deserialize)]
struct PermBody {
    perm: Perm,
}

#[derive(Deserialize)]
struct FolderQuery {
    folder: Option<FileId>,
}

async fn this_box(mesh: web::Data<Arc<Mesh>>, who: Person) -> HttpResponse {
    HttpResponse::Ok().json(json!({
        "name": mesh.name,
        "key": mesh.identity.public(),
        "me": format!("{}@{}", who.public, mesh.name),
    }))
}

async fn create(
    mesh: web::Data<Arc<Mesh>>,
    who: Person,
    body: web::Json<NewShare>,
) -> HttpResponse {
    let to = body.to.trim().to_ascii_lowercase();
    match mesh.share(&who, body.file_id, &to, body.perm) {
        Ok(s) => HttpResponse::Created().json(s),
        Err(e) => error(&e),
    }
}

async fn list_out(
    mesh: web::Data<Arc<Mesh>>,
    who: Person,
    q: web::Query<ShareQuery>,
) -> HttpResponse {
    let all = mesh.outgoing(&who.login);
    let shown: Vec<_> = all
        .into_iter()
        .filter(|s| q.file_id.is_none_or(|f| s.file_id == f))
        .collect();
    HttpResponse::Ok().json(shown)
}

async fn change(
    mesh: web::Data<Arc<Mesh>>,
    who: Person,
    id: web::Path<String>,
    body: web::Json<PermBody>,
) -> HttpResponse {
    match mesh.set_perm(&who.login, &id, body.perm) {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(e) => error(&e),
    }
}

async fn remove(mesh: web::Data<Arc<Mesh>>, who: Person, id: web::Path<String>) -> HttpResponse {
    match mesh.unshare(&who.login, &id) {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(e) => error(&e),
    }
}

async fn incoming(mesh: web::Data<Arc<Mesh>>, who: Person) -> HttpResponse {
    HttpResponse::Ok().json(mesh.incoming(&who.public))
}

async fn accept(mesh: web::Data<Arc<Mesh>>, who: Person, id: web::Path<String>) -> HttpResponse {
    match mesh.answer(&who.public, &id, true).await {
        Ok(s) => HttpResponse::Ok().json(s),
        Err(e) => error(&e),
    }
}

async fn decline(mesh: web::Data<Arc<Mesh>>, who: Person, id: web::Path<String>) -> HttpResponse {
    match mesh.answer(&who.public, &id, false).await {
        Ok(s) => HttpResponse::Ok().json(s),
        Err(e) => error(&e),
    }
}

async fn folder(
    mesh: web::Data<Arc<Mesh>>,
    who: Person,
    id: web::Path<String>,
    q: web::Query<FolderQuery>,
) -> HttpResponse {
    match mesh.list(&who.public, &id, q.folder).await {
        Ok(items) => HttpResponse::Ok().json(items),
        Err(e) => error(&e),
    }
}

async fn download(
    mesh: web::Data<Arc<Mesh>>,
    who: Person,
    path: web::Path<(String, FileId)>,
) -> HttpResponse {
    let (id, file) = path.into_inner();
    match mesh.read(&who.public, &id, file).await {
        Ok((name, bytes)) => {
            let safe: String = name
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || " ._-()".contains(c) {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            HttpResponse::Ok()
                .content_type("application/octet-stream")
                .insert_header((
                    "Content-Disposition",
                    format!("attachment; filename=\"{safe}\""),
                ))
                .insert_header(("X-Content-Type-Options", "nosniff"))
                .body(bytes)
        }
        Err(e) => error(&e),
    }
}

/// `/cloud/api/mesh`. Needs `web::Data<Arc<Mesh>>`.
pub fn scope() -> Scope {
    web::scope("/cloud/api/mesh")
        .route("/box", web::get().to(this_box))
        .route("/shares", web::get().to(list_out))
        .route("/shares", web::post().to(create))
        .route("/shares/{id}", web::patch().to(change))
        .route("/shares/{id}", web::delete().to(remove))
        .route("/incoming", web::get().to(incoming))
        .route("/incoming/{id}/accept", web::post().to(accept))
        .route("/incoming/{id}/decline", web::post().to(decline))
        .route("/incoming/{id}/list", web::get().to(folder))
        .route("/incoming/{id}/files/{file}", web::get().to(download))
}

async fn link(req: HttpRequest, body: web::Payload, mesh: web::Data<Arc<Mesh>>) -> HttpResponse {
    let (res, mut session, stream) = match actix_ws::handle(&req, body) {
        Ok(x) => x,
        Err(e) => return e.error_response(),
    };
    let mut stream = stream.max_frame_size(MAX_FRAME).aggregate_continuations();
    let (out_tx, mut out_rx) = mpsc::channel::<Frame>(512);
    let (in_tx, in_rx) = mpsc::channel::<Frame>(512);
    actix_web::rt::spawn(async move {
        loop {
            tokio::select! {
                f = out_rx.recv() => {
                    let sent = match f {
                        Some(Frame::Text(t)) => session.text(t).await,
                        Some(Frame::Binary(b)) => session.binary(b).await,
                        None => break,
                    };
                    if sent.is_err() { break }
                }
                m = stream.recv() => {
                    let f = match m {
                        Some(Ok(AggregatedMessage::Text(t))) => Frame::Text(t.to_string()),
                        Some(Ok(AggregatedMessage::Binary(b))) => Frame::Binary(b.to_vec()),
                        Some(Ok(AggregatedMessage::Ping(p))) => {
                            if session.pong(&p).await.is_err() { break }
                            continue;
                        }
                        Some(Ok(AggregatedMessage::Pong(_))) => continue,
                        _ => break,
                    };
                    if in_tx.send(f).await.is_err() { break }
                }
            }
        }
        let _ = session.close(None).await;
    });
    let mesh = mesh.get_ref().clone();
    tokio::spawn(async move {
        if let Err(e) = mesh.accept((out_tx, in_rx)).await {
            tracing::info!(error = %e, "mesh: refused a link");
        }
    });
    res
}

/// `/mesh/v1`, at the vhost root. Needs `web::Data<Arc<Mesh>>`.
pub fn link_scope() -> Scope {
    web::scope("/mesh/v1").route("/link", web::get().to(link))
}

impl MeshError {
    /// For handlers outside this module.
    pub fn response(&self) -> HttpResponse {
        error(self)
    }
}
