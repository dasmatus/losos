//! The terminal's routes (`crate::operator`, `docs/edge-terminal.md`).
//!
//! `GET /operator/host-key` and `POST /operator/call` are what `losos`
//! talks to; the envelope is their authentication, so neither takes a
//! header. The three `/operator/tokens*` routes are what `provision token`
//! talks to, behind the same GitHub gate as the certificate push: the edge
//! has no web UI to make a token from.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use losos_access::{Refusal, Scope, TokenError};
use serde::Deserialize;

use super::{github_operator, AppState};
use crate::action::Action;
use crate::operator::{Call, NotServed, Operator};

fn off() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(
            serde_json::json!({ "error": "this edge has no terminal access (losos.edge.access)" }),
        ),
    )
        .into_response()
}

fn plain(status: u16, message: impl Into<String>) -> Response {
    (
        StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_REQUEST),
        Json(serde_json::json!({ "error": message.into() })),
    )
        .into_response()
}

/// `GET /operator/host-key`: `{fingerprint, publicKey}`, which `losos login`
/// pins on first contact.
pub(super) async fn host_key(State(st): State<AppState>) -> Response {
    let Some(op) = &st.operator else {
        return off();
    };
    Json(serde_json::json!({
        "fingerprint": op.host.fingerprint(),
        "publicKey": op.host.armored_public(),
    }))
    .into_response()
}

/// `POST /operator/call`: open, authenticate, run, seal. A refusal before
/// the signer is known is plain JSON; everything after is sealed to it.
pub(super) async fn call(State(st): State<AppState>, body: Bytes) -> Response {
    let Some(op) = st.operator.clone() else {
        return off();
    };
    let opened = {
        let op = Arc::clone(&op);
        tokio::task::spawn_blocking(move || {
            let now = losos_access::now_secs();
            let opened = losos_access::open(&op.host, &op.ring, &body, &op.replay, now)?;
            let token = op.tokens.authenticate(
                &opened.request.token,
                &opened.signer.fingerprint,
                &opened.request.method,
                now,
            )?;
            Ok::<_, Refusal>((opened, token))
        })
        .await
    };
    let (opened, token) = match opened {
        Ok(Ok(found)) => found,
        Ok(Err(refusal)) => {
            tracing::warn!(target: Action::Authenticate.target(), "terminal call refused: {refusal}");
            return plain(refusal.status(), refusal.to_string());
        }
        Err(e) => {
            tracing::error!(target: Action::Authenticate.target(), "terminal call: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let r = &opened.request;
    let (status, answer) = match crate::operator::route(&opened) {
        Err(NotServed { status, message }) => (status, serde_json::json!({ "error": message })),
        Ok(call) => run(&st, &op, call).await,
    };
    tracing::info!(
        target: Action::Authenticate.target(),
        "terminal call {} {} {} by token {} ({}): {status}",
        r.method, r.path, opened.signer.fingerprint, token.id, token.name
    );

    let nonce = r.nonce.clone();
    let sealed = tokio::task::spawn_blocking(move || {
        losos_access::seal(&op.host, &opened.signer, &nonce, status, answer)
    })
    .await;
    match sealed {
        // The inner status on the outside too, as on a box; the client
        // reads the sealed one.
        Ok(Ok(armored)) => (
            StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            [(header::CONTENT_TYPE, "application/pgp-encrypted")],
            armored,
        )
            .into_response(),
        Ok(Err(e)) => {
            tracing::error!(target: Action::Authenticate.target(), "sealing an answer: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
        Err(e) => {
            tracing::error!(target: Action::Authenticate.target(), "sealing an answer: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn run(st: &AppState, op: &Operator, call: Call) -> (u16, serde_json::Value) {
    let err = |e: NotServed| (e.status, serde_json::json!({ "error": e.message }));
    match call {
        Call::Overview => (200, overview(st).await),
        Call::Reconcile => {
            st.notify.notify_one();
            (200, serde_json::json!({ "reconciling": true }))
        }
        Call::ListSecrets => match op.list_secrets().await {
            Ok(rows) => (200, rows),
            Err(e) => err(e),
        },
        Call::SetSecret { name, value } => match op.set_secret(&name, &value).await {
            Ok(out) if out.ok => {
                tracing::info!(target: Action::Authenticate.target(), "credential {name} sealed from the terminal");
                (
                    200,
                    serde_json::json!({ "name": name, "sealed": true, "report": out.output }),
                )
            }
            Ok(out) => (400, serde_json::json!({ "error": out.output })),
            Err(e) => err(e),
        },
    }
}

/// What the Overview pane shows. Counts, never tokens or addresses beyond
/// what the edge already publishes.
async fn overview(st: &AppState) -> serde_json::Value {
    let snap = st.reg.export().await;
    let tenants: Vec<serde_json::Value> = snap
        .tenants
        .iter()
        .map(|(id, t)| {
            serde_json::json!({
                "id": id,
                "hostname": t.hostname,
                "seenAgoSecs": t.seen_ago_secs,
                "via": t.via,
            })
        })
        .collect();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let identity = match &st.identity {
        None => serde_json::Value::Null,
        Some(id) => match id.cert() {
            None => serde_json::json!({ "official": false }),
            Some(c) => serde_json::json!({
                "official": c.not_after > now,
                "name": c.name,
                "url": c.url,
                "notAfter": c.not_after,
            }),
        },
    };
    let enrolled = match &st.enrolment {
        None => serde_json::Value::Null,
        Some(e) => e
            .list()
            .await
            .map(|l| serde_json::json!(l.len()))
            .unwrap_or(serde_json::Value::Null),
    };
    let routes = st
        .routes
        .as_ref()
        .and_then(|r| r.last.lock().ok().map(|l| l.len()));
    let domains = match &st.domains {
        None => None,
        Some(d) => Some(d.snapshot().await.claims.len()),
    };
    serde_json::json!({
        "health": true,
        "version": env!("CARGO_PKG_VERSION"),
        "identity": identity,
        "tenants": tenants,
        "enrolled": enrolled,
        "routes": routes,
        "domains": domains,
        "optIns": {
            "mesh": st.opts.mesh_agent_token_file.is_some(),
            "market": st.market.is_some(),
            "builder": st.builder.is_some(),
            "domains": st.domains.is_some(),
            "enrolment": st.enrolment.is_some(),
            "uplink": st.opts.uplink.is_some(),
        },
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NewToken {
    name: String,
    fingerprint: String,
    scope: Scope,
    expires_in_days: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RevokeToken {
    id: String,
}

fn token_error(e: TokenError) -> Response {
    match e {
        TokenError::Invalid(why) => plain(400, why),
        TokenError::Full => plain(409, e.to_string()),
        TokenError::NotFound => plain(404, e.to_string()),
        TokenError::Io { .. } => {
            tracing::error!(target: Action::Authenticate.target(), "terminal tokens: {e}");
            plain(507, "the token file could not be written")
        }
    }
}

/// The GitHub gate, then `f` on the operator state. One at a time, under
/// the certificate push's lock.
async fn gated(
    st: &AppState,
    headers: &HeaderMap,
    what: &str,
    f: impl FnOnce(&Operator, &str) -> Response,
) -> Response {
    let Some(op) = &st.operator else {
        return off();
    };
    let _one_at_a_time = st.push_lock.lock().await;
    match github_operator(st, headers, what).await {
        Ok((user, _)) => f(op, &user.login),
        Err(refused) => *refused,
    }
}

/// `POST /operator/tokens`: `{name, fingerprint, scope, expiresInDays}` to
/// `{token, record}`, as the box's `/api/access/tokens`.
pub(super) async fn create_token(
    State(st): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let req: NewToken = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return plain(
                400,
                format!("expected {{name, fingerprint, scope, expiresInDays}}: {e}"),
            )
        }
    };
    gated(&st, &headers, "token create", |op, login| {
        match op.tokens.create(
            &op.ring,
            &req.name,
            &req.fingerprint,
            req.scope,
            req.expires_in_days,
            losos_access::now_secs(),
        ) {
            Ok((token, record)) => {
                tracing::info!(
                    target: Action::Authenticate.target(),
                    "terminal token {} ({}) made by {login}",
                    record.id, record.name
                );
                Json(serde_json::json!({ "token": token, "record": record })).into_response()
            }
            Err(e) => token_error(e),
        }
    })
    .await
}

/// `POST /operator/tokens/list`: `{tokens, keys, hostKey}`, no hashes. A
/// POST so the GitHub token never sits in a GET a proxy might log.
pub(super) async fn list_tokens(State(st): State<AppState>, headers: HeaderMap) -> Response {
    gated(&st, &headers, "token list", |op, _| {
        match op.tokens.list() {
            Ok(tokens) => Json(serde_json::json!({
                "hostKey": { "fingerprint": op.host.fingerprint() },
                "keys": op.ring.keys().iter().map(|k| serde_json::json!({
                    "fingerprint": k.fingerprint,
                    "userIds": k.user_ids,
                    "created": k.created,
                })).collect::<Vec<_>>(),
                "tokens": tokens,
            }))
            .into_response(),
            Err(e) => token_error(e),
        }
    })
    .await
}

/// `POST /operator/tokens/revoke`: `{id}`.
pub(super) async fn revoke_token(
    State(st): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let req: RevokeToken = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return plain(400, format!("expected {{id}}: {e}")),
    };
    gated(&st, &headers, "token revoke", |op, login| match op.tokens.revoke(&req.id) {
        Ok(()) => {
            tracing::info!(target: Action::Authenticate.target(), "terminal token {} revoked by {login}", req.id);
            Json(serde_json::json!({ "revoked": true })).into_response()
        }
        Err(e) => token_error(e),
    })
    .await
}
