//! Office session and capability routes on actix-web.
//!
//! Native LosOS Office no longer uses Collabora/WOPI. These routes answer
//! the web app's capability and open requests.

use super::capabilities::Capabilities;
use super::store::DirStore;
use super::{extension, open, OpenError, Person, StoreError};
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use std::sync::Arc;

/// What the session routes need.
pub struct Sessions {
    pub store: Arc<dyn super::OfficeStore>,
    pub capabilities: Capabilities,
}

impl Sessions {
    pub fn new(store: Arc<dyn super::OfficeStore>) -> Self {
        Self {
            store,
            capabilities: Capabilities::native(),
        }
    }
}

/// `POST {prefix}/open` and `GET {prefix}/extensions`.
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
            )
            .route(&format!("{prefix}/import"), web::post().to(import_handler));
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenBody {
    file_id: String,
}

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
        body.into_inner().file_id,
        sessions.into_inner(),
    )
    .await
}

pub async fn open_answer(
    person: Person,
    file_id: String,
    sessions: Arc<Sessions>,
) -> HttpResponse {
    let answer = web::block(move || open(sessions.store.as_ref(), &sessions.capabilities, &person, &file_id)).await;
    match answer {
        Ok(Ok(session)) => HttpResponse::Ok().json(session),
        Ok(Err(OpenError::Unsupported(ext))) => HttpResponse::UnprocessableEntity()
            .json(serde_json::json!({ "error": "unsupported", "extension": ext })),
        Ok(Err(OpenError::Store(StoreError::NotFound | StoreError::Forbidden))) => {
            HttpResponse::NotFound().json(serde_json::json!({ "error": "notFound" }))
        }
        Ok(Err(OpenError::Store(e))) => {
            tracing::warn!(error = %e, "office open failed");
            HttpResponse::InternalServerError().json(serde_json::json!({ "error": "store" }))
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

pub async fn extensions_answer(sessions: Arc<Sessions>) -> HttpResponse {
    let list: Vec<_> = sessions.capabilities.list();
    HttpResponse::Ok().json(serde_json::json!({ "extensions": list }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportBody {
    file_id: String,
}

async fn import_handler(
    req: HttpRequest,
    body: web::Json<ImportBody>,
    sessions: web::Data<Sessions>,
) -> HttpResponse {
    let Some(person) = person(&req) else {
        return HttpResponse::Unauthorized().json(serde_json::json!({ "error": "signedOut" }));
    };
    let sessions = sessions.into_inner();
    let file_id = body.into_inner().file_id;
    let result = web::block(move || -> Result<serde_json::Value, StoreError> {
        let info = sessions.store.stat(&person.login, &file_id)?;
        let bytes = sessions.store.read(&person.login, &file_id)?;
        let text = super::import::extract_text(&info.name, &bytes);
        let ext = extension(&info.name).unwrap_or_default();
        let target = sessions.capabilities.action(&ext).and_then(|a| a.import_target).unwrap_or("paper");
        let (new_name, body) = super::import::native_body(&info.name, &text, target);
        let (id, info) = sessions.store.create(&person.login, &file_id, &new_name, body.as_bytes(), super::store::CreateMode::Unique)?;
        Ok(serde_json::json!({ "id": id, "name": info.name, "kind": target }))
    }).await;
    match result {
        Ok(Ok(v)) => HttpResponse::Ok().json(v),
        Ok(Err(StoreError::NotFound | StoreError::Forbidden)) => HttpResponse::NotFound().json(serde_json::json!({"error":"notFound"})),
        Ok(Err(e)) => { tracing::warn!(error=%e, "office import failed"); HttpResponse::InternalServerError().json(serde_json::json!({"error":"store"})) }
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

/// `GET {prefix}/files`: documents in a plain folder.
pub fn folder_routes(
    prefix: &str,
    folder: Arc<DirStore>,
) -> impl FnOnce(&mut web::ServiceConfig) + '_ {
    move |cfg| {
        cfg.app_data(web::Data::from(folder.clone()))
            .app_data(web::Data::from(Arc::new(Sessions::new(folder))))
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
        let files = folder
            .list()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let rows: Vec<_> = files
            .into_iter()
            .filter_map(|f| {
                let action = sessions.capabilities.action(&extension(&f.info.name)?)?;
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
                    "mode": action.mode,
                    "importTarget": action.import_target,
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
            HttpResponse::InternalServerError().json(serde_json::json!({ "error": "store" }))
        }
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}
