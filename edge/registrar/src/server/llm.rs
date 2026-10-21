//! The pay-as-you-go routes for AI models (`crate::inference`,
//! `crate::meter`). 503 unless the edge runs models (`--inference-models`)
//! and a USDC market. Every route takes the appliance token, like the rest
//! of the market.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{DefaultBodyLimit, State};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::Semaphore;

use super::{market_tenant, ApiError, AppState, MarketAuth};
use crate::inference::Inference;
use crate::market::{MarketError, TapQuote};

/// Chat requests answered at once across the edge. A head answers one at a
/// time anyway; this bounds the waiting.
const MAX_CHATS: usize = 32;
/// A chat body. The prompt limit counts decoded text, and JSON writes a
/// control character as six bytes (`\u0000`), so a prompt at the limit can
/// take six times as much on the wire; the rest is room for the messages'
/// envelopes. `modules/edge-inference.nix` gives the route the same cap in
/// Traefik, and the box's nginx (`modules/containers.nix`) too.
const MAX_CHAT_BYTES: usize = 8 * crate::inference::MAX_PROMPT_BYTES;

pub(super) fn chats() -> Arc<Semaphore> {
    Arc::new(Semaphore::new(MAX_CHATS))
}

fn inference_of(st: &AppState) -> Result<&Arc<Inference>, ApiError> {
    st.inference
        .as_ref()
        .ok_or(ApiError::Market(MarketError::Unconfigured))
}

/// The boxes lending memory now whose operator lets them sell.
pub(super) async fn lenders_of(st: &AppState) -> Vec<(String, u16, u64)> {
    let Ok(tenants) = st.tenants.load(&st.opts.tenants_file).await else {
        return Vec::new();
    };
    st.reg
        .lenders(st.opts.heartbeat_ttl)
        .await
        .into_iter()
        .filter(|(id, _, _)| tenants.get(id).is_some_and(|t| t.market))
        .collect()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OverviewReq {
    #[serde(flatten)]
    auth: MarketAuth,
    #[serde(default)]
    network: Option<String>,
}

pub(super) async fn overview(
    State(st): State<AppState>,
    Json(req): Json<OverviewReq>,
) -> Result<Json<Value>, ApiError> {
    let market = market_tenant(&st, &req.auth).await?;
    let inference = inference_of(&st)?;
    let lenders = lenders_of(&st).await;
    Ok(Json(
        market
            .llm_overview(
                inference,
                &lenders,
                &req.auth.appliance_id,
                req.network.as_deref(),
            )
            .await?,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct QuoteReq {
    #[serde(flatten)]
    auth: MarketAuth,
    model: String,
    #[serde(default)]
    network: Option<String>,
    /// Base units per hour, decimal text or a number.
    spend_per_hour: Value,
}

fn units(v: &Value) -> Result<u64, ApiError> {
    v.as_u64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .ok_or(ApiError::Market(MarketError::Invalid(
            "spendPerHour is a whole number of USDC base units",
        )))
}

pub(super) async fn quote(
    State(st): State<AppState>,
    Json(req): Json<QuoteReq>,
) -> Result<Json<TapQuote>, ApiError> {
    let market = market_tenant(&st, &req.auth).await?;
    let inference = inference_of(&st)?;
    let lenders = lenders_of(&st).await;
    let spend = units(&req.spend_per_hour)?;
    Ok(Json(
        market
            .tap_quote(
                inference,
                &lenders,
                &req.auth.appliance_id,
                &req.model,
                req.network.as_deref(),
                spend,
            )
            .await?,
    ))
}

#[derive(Debug, Deserialize)]
pub(super) struct MeterReq {
    #[serde(flatten)]
    auth: MarketAuth,
    meter: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct ConfirmReq {
    #[serde(flatten)]
    auth: MarketAuth,
    meter: String,
    /// The wallet that opened the taps: the escrow keeps them under it.
    wallet: String,
}

pub(super) async fn confirm(
    State(st): State<AppState>,
    Json(req): Json<ConfirmReq>,
) -> Result<Json<Value>, ApiError> {
    let market = market_tenant(&st, &req.auth).await?;
    inference_of(&st)?;
    let state = market
        .tap_confirm(&req.auth.appliance_id, &req.meter, &req.wallet)
        .await?;
    Ok(Json(
        serde_json::json!({ "meter": req.meter, "state": state }),
    ))
}

pub(super) async fn stop(
    State(st): State<AppState>,
    Json(req): Json<MeterReq>,
) -> Result<Json<Value>, ApiError> {
    let market = market_tenant(&st, &req.auth).await?;
    inference_of(&st)?;
    let state = market.tap_stop(&req.auth.appliance_id, &req.meter).await?;
    st.notify.notify_one();
    Ok(Json(
        serde_json::json!({ "meter": req.meter, "state": state }),
    ))
}

async fn chat(State(st): State<AppState>, Json(req): Json<Value>) -> Result<Json<Value>, ApiError> {
    let Ok(_permit) = Arc::clone(&st.chats).try_acquire_owned() else {
        return Err(ApiError::Market(MarketError::Busy(
            "the edge is answering too many requests; try again soon",
        )));
    };
    let auth: MarketAuth = serde_json::from_value(req.clone())
        .map_err(|_| ApiError::Market(MarketError::Forbidden))?;
    let meter = req
        .get("meter")
        .and_then(Value::as_str)
        .ok_or(ApiError::Market(MarketError::Invalid("name the meter")))?
        .to_string();
    let market = market_tenant(&st, &auth).await?;
    let inference = inference_of(&st)?;
    let answer = tokio::time::timeout(
        Duration::from_secs(crate::inference::ANSWER_SECS + 60),
        market.tap_chat(inference, &auth.appliance_id, &meter, &req),
    )
    .await
    .map_err(|_| {
        ApiError::Market(MarketError::Unavailable(
            "the model took too long to answer; nothing was billed",
        ))
    })??;
    Ok(Json(answer))
}

/// The chat route, merged outside the API's guard: an answer from a large
/// model takes minutes, not the API's five seconds, and its body is larger.
/// [`MAX_CHATS`] stands in for the guard's in-flight cap.
pub(super) fn chat_router(state: AppState) -> Router {
    Router::new()
        .route(
            "/market/llm/chat",
            post(chat).layer(DefaultBodyLimit::max(MAX_CHAT_BYTES)),
        )
        .with_state(state)
}
