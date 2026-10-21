//! The editor's HTTP side, under `/cloud/api/collab`.
//!
//! `GET /doc` tells the page what it is editing, and `GET /ws` is the
//! y-sync WebSocket the editor's Yjs provider connects to. Either names a
//! file on this box (`?file=<id>`) or a file in a share from another box
//! (`?share=<id>&file=<id on that box>`).

use std::sync::Arc;

use actix_web::{web, HttpRequest, HttpResponse, Scope};
use actix_ws::AggregatedMessage;
use serde::Deserialize;
use serde_json::json;

use super::hub::{Doc, DocKey};
use super::room::{Access, MAX_MESSAGE};
use super::store::{FileId, Perm};
use crate::cloud::mesh::{Mesh, MeshError, Person};

#[derive(Deserialize)]
pub struct Which {
    file: FileId,
    share: Option<String>,
}

/// The JSON error the web app shows a sentence for.
pub fn error(e: &MeshError) -> HttpResponse {
    // The cloud API's shape: a code in its camelCase, and words.
    let mut code = String::new();
    let mut upper = false;
    for c in e.code().chars() {
        if c == '-' {
            upper = true;
        } else if upper {
            code.push(c.to_ascii_uppercase());
            upper = false;
        } else {
            code.push(c);
        }
    }
    let body = json!({ "code": code, "error": e.to_string() });
    match e {
        MeshError::NotFound | MeshError::Open(super::hub::OpenError::NotFound) => {
            HttpResponse::NotFound().json(body)
        }
        MeshError::Forbidden => HttpResponse::Forbidden().json(body),
        MeshError::BadAddress | MeshError::Open(_) | MeshError::TooLarge => {
            HttpResponse::UnprocessableEntity().json(body)
        }
        MeshError::Unreachable(_) | MeshError::Timeout | MeshError::Closed => {
            HttpResponse::BadGateway().json(body)
        }
        _ => HttpResponse::InternalServerError().json(body),
    }
}

/// Open the document `who` asked for, with what they may do in it.
async fn open(
    mesh: &Arc<Mesh>,
    who: &Person,
    w: &Which,
) -> Result<(Arc<Doc>, Access, serde_json::Value), MeshError> {
    match &w.share {
        None => {
            let store = mesh.hub.store().clone();
            let (file, login) = (w.file, who.login.clone());
            let (info, perm) = web::block(move || (store.stat(file), store.access(&login, file)))
                .await
                .map_err(|_| MeshError::Store)?;
            let info = info
                .map_err(|_| MeshError::Store)?
                .ok_or(MeshError::NotFound)?;
            if perm == Perm::None {
                return Err(MeshError::NotFound);
            }
            let doc = mesh.hub.open(DocKey::File(w.file)).await?;
            let access = if perm == Perm::Edit {
                Access::Write
            } else {
                Access::Read
            };
            let meta = json!({
                "name": info.name,
                "owner": info.owner,
                "box": mesh.name,
                "perm": perm,
                "remote": false,
                "me": me(mesh, who),
            });
            Ok((doc, access, meta))
        }
        Some(share) => {
            let s = mesh
                .incoming(&who.public)
                .into_iter()
                .find(|s| &s.id == share)
                .ok_or(MeshError::NotFound)?;
            let (doc, access) = mesh.open_copy(&who.public, share, w.file).await?;
            // In a shared folder the name is the file's own, read through
            // the link; a single shared file is named by the share.
            let name = if s.folder {
                mesh.list_name(&who.public, share, w.file)
                    .await
                    .unwrap_or_else(|| s.name.clone())
            } else {
                s.name.clone()
            };
            let meta = json!({
                "name": name,
                "owner": s.owner_name,
                "ownerAddress": s.owner_remote,
                "box": s.remote,
                "perm": s.perm,
                "remote": true,
                // Edits made here while the owner's box is out of reach
                // stay here and merge when it is back.
                "linked": mesh.reaches(&s.remote),
                "me": me(mesh, who),
            });
            Ok((doc, access, meta))
        }
    }
}

/// Who the page is, for the others in the document: the name beside their
/// cursor, and the address that says which box they are on.
fn me(mesh: &Mesh, who: &Person) -> serde_json::Value {
    json!({ "name": who.name, "address": format!("{}@{}", who.public, mesh.name) })
}

async fn doc(mesh: web::Data<Arc<Mesh>>, who: Person, w: web::Query<Which>) -> HttpResponse {
    match open(&mesh, &who, &w).await {
        Ok((_, _, meta)) => HttpResponse::Ok().json(meta),
        Err(e) => error(&e),
    }
}

async fn ws(
    req: HttpRequest,
    body: web::Payload,
    mesh: web::Data<Arc<Mesh>>,
    who: Person,
    w: web::Query<Which>,
) -> HttpResponse {
    let (doc, access, _) = match open(&mesh, &who, &w).await {
        Ok(x) => x,
        Err(e) => return error(&e),
    };
    let (res, mut session, stream) = match actix_ws::handle(&req, body) {
        Ok(x) => x,
        Err(e) => return e.error_response(),
    };
    let mut stream = stream.max_frame_size(MAX_MESSAGE).aggregate_continuations();
    let (peer, mut rx) = doc.room.join(access);
    actix_web::rt::spawn(async move {
        loop {
            tokio::select! {
                out = rx.recv() => match out {
                    Some(m) => if session.binary(m).await.is_err() { break },
                    None => break,
                },
                msg = stream.recv() => match msg {
                    Some(Ok(AggregatedMessage::Binary(b))) => {
                        if doc.room.receive(peer, &b).is_err() {
                            break;
                        }
                    }
                    Some(Ok(AggregatedMessage::Ping(p))) => {
                        if session.pong(&p).await.is_err() { break }
                    }
                    Some(Ok(AggregatedMessage::Text(_) | AggregatedMessage::Pong(_))) => {}
                    Some(Ok(AggregatedMessage::Close(_))) | Some(Err(_)) | None => break,
                },
            }
        }
        doc.room.leave(peer);
        let _ = session.close(None).await;
    });
    res
}

/// `/cloud/api/collab`. Needs `web::Data<Arc<Mesh>>`.
pub fn scope() -> Scope {
    web::scope("/cloud/api/collab")
        .route("/doc", web::get().to(doc))
        .route("/ws", web::get().to(ws))
}
