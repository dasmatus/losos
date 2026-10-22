//! The WOPI routes and the editor-session routes on actix-web.
//!
//! [`wopi_routes`] is what coolwsd calls, on a loopback listener of its own;
//! [`session_routes`] is what the editor page calls through nginx, which
//! names the signed-in person in `X-Losos-Cloud-User`. LosOS Cloud mounts
//! the same two pieces in its own service; the standalone `losos-office`
//! binary mounts them over a [`DirStore`](super::store::DirStore).

use super::discovery::Coolwsd;
use super::store::DirStore;
use super::{extension, open, OpenError, Person, StoreError, Wopi, WopiRequest};
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use std::sync::Arc;

/// The largest document a save may carry. Office files are zip archives
/// of XML and pictures; a deck full of photos is a few hundred megabytes.
pub const MAX_DOCUMENT: usize = 1024 * 1024 * 1024;

/// `/wopi/files/{id}[/contents]`, for coolwsd.
pub fn wopi_routes(wopi: Arc<Wopi>) -> impl FnOnce(&mut web::ServiceConfig) {
    move |cfg| {
        cfg.app_data(web::Data::from(wopi))
            .app_data(web::PayloadConfig::new(MAX_DOCUMENT))
            .route("/wopi/files/{tail:.*}", web::route().to(wopi_handler));
    }
}

/// The same routes as one scope, for a service that mounts them beside its
/// own (LosOS Cloud, on the socket nginx bridges coolwsd to). The large
/// body limit stays inside it.
pub fn wopi_scope(wopi: Arc<Wopi>) -> actix_web::Scope {
    web::scope("/wopi/files")
        .app_data(web::Data::from(wopi))
        .app_data(web::PayloadConfig::new(MAX_DOCUMENT))
        .route("/{tail:.*}", web::route().to(wopi_handler))
}

async fn wopi_handler(req: HttpRequest, body: web::Bytes, wopi: web::Data<Wopi>) -> HttpResponse {
    let request = WopiRequest {
        method: req.method().as_str().to_string(),
        path: req.path().to_string(),
        query: req.query_string().to_string(),
        headers: req
            .headers()
            .iter()
            .filter_map(|(n, v)| Some((n.as_str().to_string(), v.to_str().ok()?.to_string())))
            .collect(),
        body: body.to_vec(),
    };
    let wopi = wopi.into_inner();
    let answer = match web::block(move || wopi.handle(&request)).await {
        Ok(answer) => answer,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };
    let status = actix_web::http::StatusCode::from_u16(answer.status)
        .unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR);
    let mut out = HttpResponse::build(status);
    for (name, value) in &answer.headers {
        out.insert_header((name.as_str(), value.as_str()));
    }
    if let Some(ct) = answer.content_type {
        out.content_type(ct);
    }
    out.body(answer.body)
}

/// What the session routes need besides the WOPI host.
pub struct Sessions {
    pub wopi: Arc<Wopi>,
    pub coolwsd: Coolwsd,
}

/// `POST {prefix}/open` and `GET {prefix}/extensions`, for the editor page
/// and the file view, behind nginx.
pub fn session_routes(
    prefix: &str,
    sessions: Arc<Sessions>,
) -> impl FnOnce(&mut web::ServiceConfig) + '_ {
    move |cfg| {
        cfg.app_data(web::Data::from(sessions))
            .route(&format!("{prefix}/open"), web::post().to(open_handler))
            .route(
                &format!("{prefix}/extensions"),
                web::get().to(extensions_handler),
            );
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenBody {
    file_id: String,
}

/// The signed-in person, as nginx names them. Only nginx can reach the
/// listener these routes are on, so the header is trusted here.
pub fn person(req: &HttpRequest) -> Option<Person> {
    let header = |name| {
        req.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };
    let login = header("X-Losos-Cloud-User")?;
    Some(Person {
        display_name: header("X-Losos-Cloud-Name").unwrap_or_else(|| login.clone()),
        login,
    })
}

async fn open_handler(
    req: HttpRequest,
    body: web::Json<OpenBody>,
    sessions: web::Data<Sessions>,
) -> HttpResponse {
    let Some(person) = person(&req) else {
        return HttpResponse::Unauthorized().json(serde_json::json!({ "error": "signedOut" }));
    };
    open_answer(
        person,
        page_origin(&req),
        body.into_inner().file_id,
        sessions.into_inner(),
    )
    .await
}

/// The page's origin, for Collabora's messages back to it. A same-origin
/// POST from a browser always names it.
pub fn page_origin(req: &HttpRequest) -> Option<String> {
    req.headers()
        .get("Origin")
        .and_then(|v| v.to_str().ok())
        .filter(|o| o.starts_with("https://") || o.starts_with("http://"))
        .map(str::to_string)
}

/// `POST …/open`'s answer for `person`, whoever named them: nginx here, the
/// cloud's own sign-in in LosOS Cloud (`cloud::office`).
pub async fn open_answer(
    person: Person,
    origin: Option<String>,
    file_id: String,
    sessions: Arc<Sessions>,
) -> HttpResponse {
    let Some(origin) = origin else {
        return HttpResponse::BadRequest().json(serde_json::json!({ "error": "noOrigin" }));
    };
    let answer = web::block(move || {
        let discovery = sessions.coolwsd.discovery()?;
        Ok::<_, std::io::Error>(open(
            &sessions.wopi,
            &discovery,
            &person,
            &file_id,
            &origin,
            super::now_ms(),
        ))
    })
    .await;
    match answer {
        Ok(Ok(Ok(session))) => HttpResponse::Ok().json(session),
        Ok(Ok(Err(OpenError::Unsupported(ext)))) => HttpResponse::UnprocessableEntity()
            .json(serde_json::json!({ "error": "unsupported", "extension": ext })),
        Ok(Ok(Err(OpenError::Store(StoreError::NotFound | StoreError::Forbidden)))) => {
            HttpResponse::NotFound().json(serde_json::json!({ "error": "notFound" }))
        }
        Ok(Ok(Err(OpenError::Store(e)))) => {
            tracing::warn!(error = %e, "office open failed");
            HttpResponse::InternalServerError().json(serde_json::json!({ "error": "store" }))
        }
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "Collabora is not answering");
            HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({ "error": "officeStarting" }))
        }
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

async fn extensions_handler(req: HttpRequest, sessions: web::Data<Sessions>) -> HttpResponse {
    if person(&req).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({ "error": "signedOut" }));
    }
    extensions_answer(sessions.into_inner()).await
}

/// `GET …/extensions`: the types Collabora opens, and whether it edits each.
pub async fn extensions_answer(sessions: Arc<Sessions>) -> HttpResponse {
    match web::block(move || sessions.coolwsd.discovery()).await {
        Ok(Ok(discovery)) => {
            let list: Vec<_> = discovery
                .extensions()
                .map(|(ext, edit)| serde_json::json!({ "extension": ext, "edit": edit }))
                .collect();
            HttpResponse::Ok().json(serde_json::json!({ "extensions": list }))
        }
        _ => HttpResponse::ServiceUnavailable()
            .json(serde_json::json!({ "error": "officeStarting" })),
    }
}

/// `GET {prefix}/files`: the documents in a plain folder (losos-office's
/// `--root`), for the Office page's list. LosOS Cloud lists its own files.
pub fn folder_routes(
    prefix: &str,
    folder: Arc<DirStore>,
) -> impl FnOnce(&mut web::ServiceConfig) + '_ {
    move |cfg| {
        cfg.app_data(web::Data::from(folder))
            .route(&format!("{prefix}/files"), web::get().to(files_handler));
    }
}

async fn files_handler(
    req: HttpRequest,
    folder: web::Data<DirStore>,
    sessions: web::Data<Sessions>,
) -> HttpResponse {
    if person(&req).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({ "error": "signedOut" }));
    }
    let folder = folder.into_inner();
    let sessions = sessions.into_inner();
    let answer = web::block(move || {
        let discovery = sessions.coolwsd.discovery()?;
        let files = folder
            .list()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let rows: Vec<_> = files
            .into_iter()
            .filter_map(|f| {
                let action = discovery.action(&extension(&f.info.name)?)?;
                let modified = f
                    .info
                    .modified
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                Some(serde_json::json!({
                    "id": f.id,
                    "name": f.info.name,
                    "folder": f.folder,
                    "size": f.info.size,
                    "modified": modified,
                    "writable": f.info.writable && action.edit,
                }))
            })
            .collect();
        Ok::<_, std::io::Error>(rows)
    })
    .await;
    match answer {
        Ok(Ok(files)) => HttpResponse::Ok().json(serde_json::json!({ "files": files })),
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "office list failed");
            HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({ "error": "officeStarting" }))
        }
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}
