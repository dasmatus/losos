//! The reviewer's routes under `/git/api/v1/`: the repository setting, and
//! how far the review of a pull request is.
//!
//!   GET /repos/{owner}/{repo}/ai-review                 the setting
//!   PUT /repos/{owner}/{repo}/ai-review {enabled}       change it (admins)
//!   GET /repos/{owner}/{repo}/pulls/{number}/ai-review  the latest review
//!
//! The reviews themselves are ordinary reviews by `losos-reviewer`, read
//! and answered through the pull request routes like anyone's.

use super::super::api::{find_repo, need_person, need_scope, seen_by_nobody, Answer};
use super::super::db::{Permission, Repo};
use super::super::server::{self, Fail, State};
use super::jobs::JobState;
use super::worker::{BOT_LOGIN, BOT_NAME};
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn routes(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .route("/repos/{owner}/{repo}/ai-review", web::get().to(setting))
        .route("/repos/{owner}/{repo}/ai-review", web::put().to(change))
        .route(
            "/repos/{owner}/{repo}/pulls/{number}/ai-review",
            web::get().to(pull),
        )
}

/// The setting as the pages show it.
fn setting_json(state: &State, repo: &Repo, permission: Permission) -> Result<Value, Fail> {
    Ok(json!({
        "enabled": state.store.ai_review(repo.id)?,
        // The box has a code model (Build with AI is on).
        "available": state.reviewer.available(),
        "model": state.reviewer.config.as_ref().map(|c| c.model.clone()),
        "reviewer": { "login": BOT_LOGIN, "full_name": BOT_NAME },
        "canChange": permission >= Permission::Admin,
    }))
}

async fn setting(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    seen_by_nobody(&state, &c)?;
    need_scope(&c, "repository", false)?;
    let (repo, p) = find_repo(&state, &c, &path.0, &path.1)?;
    Ok(HttpResponse::Ok().json(setting_json(&state, &repo, p)?))
}

#[derive(Deserialize)]
struct Change {
    enabled: bool,
}

async fn change(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    body: web::Json<Change>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    need_person(&c)?;
    need_scope(&c, "repository", true)?;
    let (repo, p) = find_repo(&state, &c, &path.0, &path.1)?;
    if p < Permission::Admin {
        return Err(Fail::forbidden(
            "Only the repository's administrators may change it.",
        ));
    }
    if body.enabled && !state.reviewer.available() {
        return Err(Fail::bad(
            "LosOS Reviewer needs the box's code model: turn on Build with AI.",
        ));
    }
    state.store.set_ai_review(repo.id, body.enabled)?;
    if body.enabled {
        state.store.ensure_bot(BOT_LOGIN, BOT_NAME)?;
    }
    Ok(HttpResponse::Ok().json(setting_json(&state, &repo, p)?))
}

async fn pull(
    req: HttpRequest,
    path: web::Path<(String, String, i64)>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    seen_by_nobody(&state, &c)?;
    need_scope(&c, "issue", false)?;
    let (repo, _) = find_repo(&state, &c, &path.0, &path.1)?;
    let pr = state
        .store
        .issue(repo.id, path.2)?
        .filter(|i| i.is_pull)
        .ok_or_else(|| Fail::not_found("pull request"))?;
    let enabled = state.store.ai_review(repo.id)?;
    let job = state.store.last_review_job(pr.id)?;
    let ahead = match &job {
        Some(j) if j.state == JobState::Queued => state.store.reviews_ahead(j.id)?,
        _ => 0,
    };
    let state_word = match (&job, enabled, state.reviewer.available()) {
        (_, _, false) => "unavailable",
        (None, false, _) => "off",
        (None, true, _) => "idle",
        (Some(j), _, _) => match j.state {
            JobState::Queued => "queued",
            JobState::Reviewing => "reviewing",
            JobState::Done => "done",
            JobState::Failed => "failed",
            JobState::Superseded if enabled => "idle",
            JobState::Superseded => "off",
        },
    };
    Ok(HttpResponse::Ok().json(json!({
        "enabled": enabled,
        "available": state.reviewer.available(),
        "state": state_word,
        "reason": job.as_ref().map(|j| j.reason),
        "head": job.as_ref().map(|j| j.head.clone()),
        "filesDone": job.as_ref().map_or(0, |j| j.files_done),
        "filesTotal": job.as_ref().map_or(0, |j| j.files_total),
        "comments": job.as_ref().map_or(0, |j| j.comments),
        "ahead": ahead,
        "reviewId": job.as_ref().and_then(|j| j.review_id),
        "error": job.as_ref().and_then(|j| j.error.clone()),
        "started": job.as_ref().and_then(|j| j.started.clone()),
        "finished": job.as_ref().and_then(|j| j.finished.clone()),
    })))
}
