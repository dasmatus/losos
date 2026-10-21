//! `losos-ci serve`'s three sockets.
//!
//! * The events socket: LosOS Git's `POST /events` (`crate::forge::events`).
//!   Its directory is open to the `losos-ci-events` group, which LosOS Git
//!   is in.
//! * The runner socket: `POST /runner/claim` and `POST /runner/report`, for
//!   `losos-ci runner` as `shared`. Its directory is open to `shared`.
//! * The page socket: the admin pages' `/ci/*`, which lososd's `ci` plugin
//!   relays from `/api/ci/*` after LosOS Login (or the spare key) let the
//!   caller in. Root only. The plugin names the person in
//!   `X-Losos-Person` and says `X-Losos-Admin: 1` for an administrator.

use super::dispatch::{Ci, Lend};
use super::net::Report;
use actix_web::http::StatusCode;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

type Shared = web::Data<Arc<Ci>>;

fn say(status: StatusCode, message: &str) -> HttpResponse {
    HttpResponse::build(status).json(json!({ "error": message }))
}

fn failed(e: anyhow::Error) -> HttpResponse {
    tracing::warn!(target: "ci", "{e:#}");
    say(StatusCode::INTERNAL_SERVER_ERROR, &format!("{e:#}"))
}

/// Run blocking work off the server's threads.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
) -> anyhow::Result<T> {
    web::block(f).await.map_err(|e| anyhow::anyhow!("{e}"))?
}

// ── events ────────────────────────────────────────────────────────────────

async fn events(ci: Shared, body: web::Json<Value>) -> HttpResponse {
    let Some(inc) = super::dispatch::parse_event(&body) else {
        return HttpResponse::NoContent().finish();
    };
    // Answer LosOS Git at once; reading the commit and planning is ours.
    let ci = ci.get_ref().clone();
    std::thread::spawn(move || {
        if let Err(e) = ci.on_event(&inc) {
            tracing::warn!(target: "ci", "{}/{}: {e:#}", inc.owner, inc.repo);
        }
    });
    HttpResponse::Accepted().finish()
}

// ── the runner ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ClaimReq {
    #[serde(default)]
    wait: u32,
}

async fn claim(ci: Shared, body: web::Json<ClaimReq>) -> HttpResponse {
    use base64::Engine as _;
    let ci = ci.get_ref().clone();
    let wait = body.wait.min(30);
    match blocking(move || ci.claim(wait)).await {
        Ok(Some(w)) => HttpResponse::Ok().json(json!({
            "kind": w.kind,
            "id": w.id,
            "origin": w.origin,
            "name": w.name,
            "payload": w.payload,
            "timeoutSecs": w.timeout_secs,
            "archive": base64::engine::general_purpose::STANDARD.encode(&w.archive),
        })),
        Ok(None) => HttpResponse::NoContent().finish(),
        Err(e) => failed(e),
    }
}

#[derive(Deserialize)]
struct ReportReq {
    kind: String,
    id: String,
    #[serde(flatten)]
    report: Report,
}

async fn report(ci: Shared, body: web::Json<ReportReq>) -> HttpResponse {
    let ci = ci.get_ref().clone();
    let body = body.into_inner();
    match blocking(move || ci.report(&body.kind, &body.id, &body.report)).await {
        Ok(stop) => HttpResponse::Ok().json(json!({ "stop": stop })),
        Err(e) => failed(e),
    }
}

// ── the admin pages ───────────────────────────────────────────────────────

/// Who the plugin says is asking.
struct Person {
    name: String,
    admin: bool,
}

fn person(req: &HttpRequest) -> Person {
    let h = |n: &str| {
        req.headers()
            .get(n)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .trim()
            .to_string()
    };
    Person {
        name: h("x-losos-person"),
        admin: h("x-losos-admin") == "1",
    }
}

/// What `who` may do in a repository: read, and start or stop runs.
fn access(ci: &Ci, who: &Person, owner: &str, repo: &str) -> (bool, bool) {
    if who.admin {
        return (true, true);
    }
    match ci.forge.info(owner, repo, &who.name, "") {
        Ok(info) => (info.can_read(), info.can_write()),
        Err(_) => (false, false),
    }
}

async fn overview(ci: Shared, req: HttpRequest) -> HttpResponse {
    let who = person(&req);
    let ci = ci.get_ref().clone();
    let r = blocking(move || {
        Ok(json!({
            "box": ci.box_id,
            "lend": ci.lend().as_str(),
            "lendingNow": ci.lending_now(),
            "window": ci.window.as_ref().map(|(s, e)| json!({ "start": s, "end": e })),
            "mesh": ci.mesh.reachable(),
            "privateOnMesh": ci.private_on_mesh,
            "canChange": who.admin,
        }))
    })
    .await;
    match r {
        Ok(v) => HttpResponse::Ok().json(v),
        Err(e) => failed(e),
    }
}

#[derive(Deserialize)]
struct LendReq {
    lend: String,
}

async fn set_lend(ci: Shared, req: HttpRequest, body: web::Json<LendReq>) -> HttpResponse {
    if !person(&req).admin {
        return say(
            StatusCode::FORBIDDEN,
            "Only an administrator lends this box's runner.",
        );
    }
    let Some(lend) = Lend::parse(&body.lend) else {
        return say(StatusCode::BAD_REQUEST, "lend is always, window or never.");
    };
    let ci = ci.get_ref().clone();
    let name = lend.as_str();
    match blocking(move || ci.set_lend(lend)).await {
        Ok(()) => HttpResponse::Ok().json(json!({ "lend": name })),
        Err(e) => failed(e),
    }
}

#[derive(Deserialize)]
struct RunsQuery {
    #[serde(default)]
    owner: String,
    #[serde(default)]
    repo: String,
    #[serde(default)]
    before: Option<i64>,
    #[serde(default)]
    limit: Option<u32>,
}

async fn runs(ci: Shared, req: HttpRequest, q: web::Query<RunsQuery>) -> HttpResponse {
    let who = person(&req);
    let ci = ci.get_ref().clone();
    let q = q.into_inner();
    let r = blocking(move || {
        let limit = q.limit.unwrap_or(30).clamp(1, 100);
        let filter = (!q.owner.is_empty() && !q.repo.is_empty())
            .then_some((q.owner.as_str(), q.repo.as_str()));
        let mut seen: HashMap<(String, String), bool> = HashMap::new();
        let mut out = Vec::new();
        let mut before = q.before;
        // Pages of runs, dropping what the person may not read, until the
        // page is full or the runs run out.
        for _ in 0..10 {
            let page = ci.store.runs(filter, before, limit)?;
            let last = page.last().map(|r| r.id);
            for run in page {
                let key = (run.owner.clone(), run.repo.clone());
                let readable = *seen
                    .entry(key)
                    .or_insert_with(|| access(&ci, &who, &run.owner, &run.repo).0);
                if readable {
                    let jobs = ci.store.jobs(run.id)?;
                    let mut v = serde_json::to_value(&run)?;
                    v["jobs"] = json!(jobs
                        .iter()
                        .map(|j| json!({ "name": j.name, "state": j.state, "runner": j.runner }))
                        .collect::<Vec<_>>());
                    out.push(v);
                }
            }
            match last {
                Some(id) if out.len() < limit as usize => before = Some(id),
                _ => break,
            }
        }
        out.truncate(limit as usize);
        Ok(json!({ "runs": out }))
    })
    .await;
    match r {
        Ok(v) => HttpResponse::Ok().json(v),
        Err(e) => failed(e),
    }
}

async fn run(ci: Shared, req: HttpRequest, path: web::Path<i64>) -> HttpResponse {
    let who = person(&req);
    let id = path.into_inner();
    let ci = ci.get_ref().clone();
    let r = blocking(move || {
        let Some(run) = ci.store.run(id)? else {
            return Ok(None);
        };
        let (read, write) = access(&ci, &who, &run.owner, &run.repo);
        if !read {
            return Ok(None);
        }
        let jobs = ci.store.jobs(id)?;
        let mut v = serde_json::to_value(&run)?;
        v["jobs"] = serde_json::to_value(&jobs)?;
        v["canChange"] = json!(write);
        Ok(Some(v))
    })
    .await;
    match r {
        Ok(Some(v)) => HttpResponse::Ok().json(v),
        Ok(None) => say(StatusCode::NOT_FOUND, "There is no such run."),
        Err(e) => failed(e),
    }
}

#[derive(Deserialize)]
struct LogQuery {
    #[serde(default)]
    from: u64,
}

async fn log(
    ci: Shared,
    req: HttpRequest,
    path: web::Path<i64>,
    q: web::Query<LogQuery>,
) -> HttpResponse {
    let who = person(&req);
    let id = path.into_inner();
    let from = q.from;
    let ci = ci.get_ref().clone();
    let r = blocking(move || {
        let Some(job) = ci.store.job(id)? else {
            return Ok(None);
        };
        let Some(run) = ci.store.run(job.run_id)? else {
            return Ok(None);
        };
        if !access(&ci, &who, &run.owner, &run.repo).0 {
            return Ok(None);
        }
        let text = ci.read_log(id, from);
        Ok(Some(json!({
            "state": job.state,
            "from": from,
            "next": from + text.len() as u64,
            "text": text,
        })))
    })
    .await;
    match r {
        Ok(Some(v)) => HttpResponse::Ok().json(v),
        Ok(None) => say(StatusCode::NOT_FOUND, "There is no such job."),
        Err(e) => failed(e),
    }
}

async fn cancel(ci: Shared, req: HttpRequest, path: web::Path<i64>) -> HttpResponse {
    change(ci, req, path.into_inner(), false).await
}

async fn rerun(ci: Shared, req: HttpRequest, path: web::Path<i64>) -> HttpResponse {
    change(ci, req, path.into_inner(), true).await
}

async fn change(ci: Shared, req: HttpRequest, id: i64, again: bool) -> HttpResponse {
    let who = person(&req);
    let ci = ci.get_ref().clone();
    let r = blocking(move || {
        let Some(run) = ci.store.run(id)? else {
            return Ok(Err(StatusCode::NOT_FOUND));
        };
        let (read, write) = access(&ci, &who, &run.owner, &run.repo);
        if !read {
            return Ok(Err(StatusCode::NOT_FOUND));
        }
        if !write {
            return Ok(Err(StatusCode::FORBIDDEN));
        }
        let by = if who.name.is_empty() {
            "the administrator"
        } else {
            &who.name
        };
        if again {
            Ok(Ok(json!({ "id": ci.rerun(id, by)? })))
        } else {
            ci.cancel_run(id, by)?;
            Ok(Ok(json!({ "id": id })))
        }
    })
    .await;
    match r {
        Ok(Ok(v)) => HttpResponse::Ok().json(v),
        Ok(Err(StatusCode::FORBIDDEN)) => say(
            StatusCode::FORBIDDEN,
            "Starting and stopping runs takes write access to the repository.",
        ),
        Ok(Err(s)) => say(s, "There is no such run."),
        Err(e) => failed(e),
    }
}

#[derive(Deserialize)]
struct RepoQuery {
    owner: String,
    repo: String,
    #[serde(default, rename = "ref")]
    git_ref: String,
}

async fn workflows(ci: Shared, req: HttpRequest, q: web::Query<RepoQuery>) -> HttpResponse {
    let who = person(&req);
    let ci = ci.get_ref().clone();
    let q = q.into_inner();
    let r = blocking(move || {
        let user = if who.admin { "" } else { who.name.as_str() };
        let info = ci.forge.info(&q.owner, &q.repo, user, &q.git_ref)?;
        if !who.admin && !info.can_read() {
            return Ok(None);
        }
        let list = match &info.sha {
            Some(sha) => {
                let archive = ci.forge.archive(&q.owner, &q.repo, sha)?;
                Ci::dispatchable(&archive)
            }
            None => Vec::new(),
        };
        Ok(Some(json!({
            "ref": info.r#ref,
            "sha": info.sha,
            "canRun": who.admin || info.can_write(),
            "workflows": list
                .into_iter()
                .map(|(file, name)| json!({ "file": file, "name": name }))
                .collect::<Vec<_>>(),
        })))
    })
    .await;
    match r {
        Ok(Some(v)) => HttpResponse::Ok().json(v),
        Ok(None) => say(StatusCode::NOT_FOUND, "There is no such repository."),
        Err(e) => failed(e),
    }
}

#[derive(Deserialize)]
struct DispatchReq {
    owner: String,
    repo: String,
    workflow: String,
    #[serde(default, rename = "ref")]
    git_ref: String,
}

async fn dispatch(ci: Shared, req: HttpRequest, body: web::Json<DispatchReq>) -> HttpResponse {
    let who = person(&req);
    let ci = ci.get_ref().clone();
    let b = body.into_inner();
    let r = blocking(move || {
        let (read, write) = access(&ci, &who, &b.owner, &b.repo);
        if !read {
            return Ok(Err(StatusCode::NOT_FOUND));
        }
        if !write {
            return Ok(Err(StatusCode::FORBIDDEN));
        }
        let actor = if who.name.is_empty() {
            "admin"
        } else {
            &who.name
        };
        Ok(Ok(ci.dispatch(
            &b.owner,
            &b.repo,
            &b.workflow,
            &b.git_ref,
            actor,
        )?))
    })
    .await;
    match r {
        Ok(Ok(id)) => HttpResponse::Ok().json(json!({ "id": id })),
        Ok(Err(StatusCode::FORBIDDEN)) => say(
            StatusCode::FORBIDDEN,
            "Running a workflow takes write access to the repository.",
        ),
        Ok(Err(s)) => say(s, "There is no such repository."),
        Err(e) => say(StatusCode::BAD_REQUEST, &format!("{e:#}")),
    }
}

fn page_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/ci", web::get().to(overview))
        .route("/ci/runner", web::put().to(set_lend))
        .route("/ci/runs", web::get().to(runs))
        .route("/ci/runs/{id}", web::get().to(run))
        .route("/ci/runs/{id}/cancel", web::post().to(cancel))
        .route("/ci/runs/{id}/rerun", web::post().to(rerun))
        .route("/ci/jobs/{id}/log", web::get().to(log))
        .route("/ci/workflows", web::get().to(workflows))
        .route("/ci/dispatch", web::post().to(dispatch));
}

/// Where `serve` listens.
pub struct Sockets {
    pub events: PathBuf,
    pub runner: PathBuf,
    pub page: PathBuf,
}

/// Serve the three sockets, with the dispatcher ticking beside them.
pub fn serve(ci: Arc<Ci>, sockets: Sockets) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    ci.recover()?;
    {
        let ci = ci.clone();
        std::thread::spawn(move || {
            let mut sweeps = 0u32;
            loop {
                if let Err(e) = ci.tick() {
                    tracing::warn!(target: "ci", "tick: {e:#}");
                }
                sweeps += 1;
                if sweeps.is_multiple_of(1800) {
                    let _ = ci.sweep();
                }
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        });
    }
    let data = web::Data::new(ci);
    actix_web::rt::System::new().block_on(async move {
        let mut servers = Vec::new();
        let listen =
            |path: &PathBuf, mode: u32, routes: fn(&mut web::ServiceConfig), limit: usize| {
                let _ = std::fs::remove_file(path);
                let data = data.clone();
                let server = HttpServer::new(move || {
                    App::new()
                        .app_data(data.clone())
                        .app_data(web::JsonConfig::default().limit(limit))
                        .configure(routes)
                })
                .workers(2)
                .bind_uds(path)?;
                // The directories are the gates (modules/ci.nix).
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
                tracing::info!(socket = %path.display(), "listening");
                anyhow::Ok(server.run())
            };
        servers.push(listen(
            &sockets.events,
            0o666,
            |c| {
                c.route("/events", web::post().to(events));
            },
            64 * 1024,
        )?);
        servers.push(listen(
            &sockets.runner,
            0o666,
            |c| {
                c.route("/runner/claim", web::post().to(claim))
                    .route("/runner/report", web::post().to(report));
            },
            2 * 1024 * 1024,
        )?);
        servers.push(listen(&sockets.page, 0o600, page_routes, 64 * 1024)?);
        for result in futures_util::future::join_all(servers).await {
            result?;
        }
        Ok(())
    })
}
