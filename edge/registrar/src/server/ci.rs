//! Mesh CI's routes (`crate::ci`). Every one takes the appliance token in
//! the body, like the market, and the tenant's `cluster` bit: CI is for the
//! boxes of this edge's mesh. 503 on an edge started without `--ci-dir`.
//!
//! They are merged outside the API's guard: a claim waits up to 25 s for
//! work and a submit carries a commit's archive, neither of which fits the
//! guard's five seconds and small bodies. `Ci::inflight` stands in for the
//! guard's cap.

use std::time::Duration;

use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use base64::Engine as _;
use serde::Deserialize;

use super::{authenticate, AppState};
use crate::ci::{Ci, CiError, JobSpec, Report};

/// A submit's body: the archive in base64, the job, the envelope.
const MAX_SUBMIT_BYTES: usize =
    crate::ci::MAX_ARCHIVE_BYTES / 3 * 4 + crate::ci::MAX_PAYLOAD_BYTES + 64 * 1024;
/// A report carries up to a log chunk.
const MAX_REPORT_BYTES: usize = 512 * 1024;

impl IntoResponse for CiError {
    fn into_response(self) -> Response {
        let status = match &self {
            CiError::Unconfigured => StatusCode::SERVICE_UNAVAILABLE,
            CiError::Forbidden => StatusCode::FORBIDDEN,
            CiError::Invalid(_) => StatusCode::BAD_REQUEST,
            CiError::NotFound => StatusCode::NOT_FOUND,
            CiError::Full(_) => StatusCode::TOO_MANY_REQUESTS,
            CiError::Io(e) => {
                tracing::error!(target: "ci", "CI request failed: {e}");
                return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
            }
        };
        (status, self.to_string()).into_response()
    }
}

#[derive(Debug, Deserialize)]
struct Auth {
    appliance_id: String,
    token: String,
}

/// The queue, and the caller as a box of the mesh, or the answer.
async fn member<'a>(st: &'a AppState, auth: &Auth) -> Result<&'a Ci, Response> {
    let Some(ci) = st.ci.as_deref() else {
        return Err(CiError::Unconfigured.into_response());
    };
    let tenant = authenticate(st, &auth.appliance_id, &auth.token)
        .await
        .map_err(IntoResponse::into_response)?;
    if !tenant.cluster {
        return Err(CiError::Forbidden.into_response());
    }
    Ok(ci)
}

fn busy() -> Response {
    (StatusCode::TOO_MANY_REQUESTS, "busy; retry later").into_response()
}

#[derive(Debug, Deserialize)]
struct SubmitReq {
    #[serde(flatten)]
    auth: Auth,
    job: JobSpec,
    /// The gzipped tar of the commit, base64.
    archive: String,
}

async fn submit(State(st): State<AppState>, Json(req): Json<SubmitReq>) -> Response {
    let ci = match member(&st, &req.auth).await {
        Ok(ci) => ci,
        Err(r) => return r,
    };
    let Ok(_permit) = ci.inflight.try_acquire() else {
        return busy();
    };
    let Ok(archive) = base64::engine::general_purpose::STANDARD.decode(req.archive.as_bytes())
    else {
        return CiError::Invalid("the archive is base64").into_response();
    };
    match ci.submit(&req.auth.appliance_id, req.job, archive).await {
        Ok(id) => Json(serde_json::json!({ "id": id })).into_response(),
        Err(e) => e.into_response(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaimReq {
    #[serde(flatten)]
    auth: Auth,
    /// The box lends CI time now: it takes other boxes' jobs.
    #[serde(default)]
    foreign: bool,
    /// Seconds to wait for work, at most 25.
    #[serde(default)]
    wait: u64,
}

async fn claim(State(st): State<AppState>, Json(req): Json<ClaimReq>) -> Response {
    let ci = match member(&st, &req.auth).await {
        Ok(ci) => ci,
        Err(r) => return r,
    };
    let Ok(_permit) = ci.inflight.try_acquire() else {
        return busy();
    };
    match ci
        .claim(
            &req.auth.appliance_id,
            req.foreign,
            Duration::from_secs(req.wait),
        )
        .await
    {
        Ok(Some(c)) => {
            let archive = base64::engine::general_purpose::STANDARD.encode(&c.archive);
            Json(serde_json::json!({
                "id": c.id,
                "origin": c.origin,
                "job": c.job,
                "archive": archive,
            }))
            .into_response()
        }
        Ok(None) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct UpdateReq {
    #[serde(flatten)]
    auth: Auth,
    id: String,
    #[serde(flatten)]
    report: Report,
}

async fn update(State(st): State<AppState>, Json(req): Json<UpdateReq>) -> Response {
    let ci = match member(&st, &req.auth).await {
        Ok(ci) => ci,
        Err(r) => return r,
    };
    match ci.update(&req.auth.appliance_id, &req.id, req.report).await {
        Ok(cancel) => Json(serde_json::json!({ "cancel": cancel })).into_response(),
        Err(e) => e.into_response(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Wanted {
    id: String,
    #[serde(default)]
    log_from: usize,
}

#[derive(Debug, Deserialize)]
struct JobsReq {
    #[serde(flatten)]
    auth: Auth,
    jobs: Vec<Wanted>,
}

async fn jobs(State(st): State<AppState>, Json(req): Json<JobsReq>) -> Response {
    let ci = match member(&st, &req.auth).await {
        Ok(ci) => ci,
        Err(r) => return r,
    };
    let wanted: Vec<(String, usize)> = req.jobs.into_iter().map(|w| (w.id, w.log_from)).collect();
    let views = ci.jobs(&req.auth.appliance_id, &wanted).await;
    Json(serde_json::json!({ "jobs": views })).into_response()
}

#[derive(Debug, Deserialize)]
struct CancelReq {
    #[serde(flatten)]
    auth: Auth,
    id: String,
}

async fn cancel(State(st): State<AppState>, Json(req): Json<CancelReq>) -> Response {
    let ci = match member(&st, &req.auth).await {
        Ok(ci) => ci,
        Err(r) => return r,
    };
    match ci.cancel(&req.auth.appliance_id, &req.id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

/// The routes, outside the guard (see the module comment).
pub(super) fn router(state: AppState) -> Router {
    Router::new()
        .route(
            "/ci/submit",
            post(submit).layer(DefaultBodyLimit::max(MAX_SUBMIT_BYTES)),
        )
        .route("/ci/claim", post(claim))
        .route("/ci/update", post(update))
        .route("/ci/jobs", post(jobs))
        .route("/ci/cancel", post(cancel))
        .layer(DefaultBodyLimit::max(MAX_REPORT_BYTES))
        .with_state(state)
}
