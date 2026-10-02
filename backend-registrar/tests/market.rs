//! Integration tests for the Stripe Connect market.
//!
//! The real routes run on a real socket against a stand-in for Stripe that
//! records every request. What is asserted is what the edge sends Stripe (a
//! destination charge carrying the 4% application fee, to the seller's own
//! connected account), what it refuses to do (sell before onboarding, oversell,
//! trust an unsigned or mismatched webhook), and what each party can see.
//!
//! The stub cannot say whether Stripe accepts the shapes; the first test-mode
//! run on a real platform account is what settles that.

mod common;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use common::{
    Edge, KubeStub, MeshFixture, TenantSpec, GOOD_TOKEN, OTHER_TOKEN, RETURN_URL, STRIPE_KEY,
    WEBHOOK_SECRET,
};
use losos_registrar::market::sign_webhook;
use serde_json::{json, Value};
use tokio::sync::oneshot;

const THIRD_TOKEN: &str = "1111222233334444555566667777888811112222333344445555666677778888";

/// One request the stub saw.
#[derive(Debug, Clone)]
struct Seen {
    method: String,
    path: String,
    bearer: String,
    idempotency_key: Option<String>,
    form: HashMap<String, String>,
}

#[derive(Clone)]
struct StripeState {
    seen: Arc<Mutex<Vec<Seen>>>,
    /// What `GET /v1/accounts?ids[]=...` reports.
    account_ready: Arc<AtomicBool>,
    /// Make checkout creation fail with a 500.
    fail_checkout: Arc<AtomicBool>,
    sessions: Arc<AtomicU64>,
}

struct StripeStub {
    base: String,
    state: StripeState,
    stop: Option<oneshot::Sender<()>>,
    join: Option<tokio::task::JoinHandle<()>>,
}

impl StripeStub {
    async fn start() -> Self {
        let state = StripeState {
            seen: Arc::default(),
            account_ready: Arc::default(),
            fail_checkout: Arc::default(),
            sessions: Arc::default(),
        };
        let app = Router::new()
            .fallback(stripe_handler)
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind the stripe stub");
        let port = listener.local_addr().expect("local_addr").port();
        let (stop, rx) = oneshot::channel::<()>();
        let join = tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = rx.await;
                })
                .await;
        });
        Self {
            base: format!("http://127.0.0.1:{port}"),
            state,
            stop: Some(stop),
            join: Some(join),
        }
    }

    fn seen(&self) -> Vec<Seen> {
        self.state.seen.lock().expect("log not poisoned").clone()
    }

    fn calls(&self, method: &str, path: &str) -> Vec<Seen> {
        self.seen()
            .into_iter()
            .filter(|s| s.method == method && s.path == path)
            .collect()
    }

    async fn shutdown(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(join) = self.join.take() {
            let _ = tokio::time::timeout(Duration::from_secs(5), join).await;
        }
    }
}

async fn stripe_handler(
    State(state): State<StripeState>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    body: String,
) -> Response {
    let form: HashMap<String, String> = body
        .split('&')
        .filter(|p| !p.is_empty())
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            Some((decode(k), decode(v)))
        })
        .collect();
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    state.seen.lock().expect("log not poisoned").push(Seen {
        method: method.to_string(),
        path: uri.path().to_string(),
        bearer: header("authorization").unwrap_or_default(),
        idempotency_key: header("idempotency-key"),
        form,
    });
    match (method.as_str(), uri.path()) {
        ("POST", "/v1/accounts") => Json(json!({ "id": "acct_test_1" })).into_response(),
        ("GET", "/v1/accounts") => {
            let ready = state.account_ready.load(Ordering::SeqCst);
            Json(json!({ "data": [account_object(ready)] })).into_response()
        }
        ("POST", "/v1/accounts/acct_test_1") => Json(account_object(false)).into_response(),
        ("POST", "/v1/account_links") => {
            Json(json!({ "url": "https://connect.stripe.test/onboard/abc" })).into_response()
        }
        ("POST", "/v1/checkout/sessions") => {
            if state.fail_checkout.load(Ordering::SeqCst) {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": { "message": "boom" } })),
                )
                    .into_response();
            }
            let n = state.sessions.fetch_add(1, Ordering::SeqCst) + 1;
            Json(json!({
                "id": format!("cs_test_{n}"),
                "url": format!("https://checkout.stripe.test/c/pay/cs_test_{n}"),
            }))
            .into_response()
        }
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("00");
                out.push(u8::from_str_radix(hex, 16).unwrap_or(b'?'));
                i += 2;
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn account_object(ready: bool) -> Value {
    json!({
        "id": "acct_test_1",
        "details_submitted": ready,
        "payouts_enabled": ready,
        "capabilities": { "transfers": if ready { "active" } else { "inactive" } },
    })
}

fn tenants() -> Vec<TenantSpec> {
    vec![
        TenantSpec::new("seller-box", "seller.example", GOOD_TOKEN).with_market(),
        TenantSpec::new("buyer-box", "buyer.example", OTHER_TOKEN).with_market(),
        // Authenticates, but the operator never cleared it for the market.
        TenantSpec::new("plain-box", "plain.example", THIRD_TOKEN),
    ]
}

fn auth(id: &str, token: &str) -> Value {
    json!({ "appliance_id": id, "token": token })
}

fn with(mut base: Value, extra: Value) -> Value {
    if let (Some(b), Some(e)) = (base.as_object_mut(), extra.as_object()) {
        for (k, v) in e {
            b.insert(k.clone(), v.clone());
        }
    }
    base
}

fn parse(body: &str) -> Value {
    serde_json::from_str(body).unwrap_or_else(|_| panic!("not JSON body"))
}

fn now() -> u64 {
    losos_registrar::market::now_secs()
}

async fn webhook(edge: &Edge, event: &Value) -> (u16, String) {
    let body = serde_json::to_vec(event).expect("serialize event");
    webhook_raw(edge, &body, &sign_webhook(WEBHOOK_SECRET, &body, now())).await
}

async fn webhook_raw(edge: &Edge, body: &[u8], signature: &str) -> (u16, String) {
    let response = edge
        .client
        .post(format!("{}/market/webhook", edge.base))
        .header("stripe-signature", signature)
        .body(body.to_vec())
        .send()
        .await
        .expect("webhook reaches the registrar");
    let status = response.status().as_u16();
    (status, response.text().await.unwrap_or_default())
}

async fn browse(edge: &Edge) -> (u16, Value) {
    let response = edge
        .client
        .get(format!("{}/market/listings", edge.base))
        .send()
        .await
        .expect("browse reaches the registrar");
    let status = response.status().as_u16();
    let text = response.text().await.unwrap_or_default();
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

fn account_updated(ready: bool) -> Value {
    json!({ "type": "account.updated", "data": { "object": account_object(ready) } })
}

/// Onboard the seller and have Stripe report the account ready.
async fn ready_seller(edge: &Edge) {
    let (status, body) = edge
        .post("/market/seller/onboard", auth("seller-box", GOOD_TOKEN))
        .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(webhook(edge, &account_updated(true)).await.0, 200);
}

async fn list(edge: &Edge, kind: &str, unit_price: u64, capacity: u64) -> (u16, String) {
    edge.post(
        "/market/listings",
        with(
            auth("seller-box", GOOD_TOKEN),
            json!({ "kind": kind, "unit_price": unit_price, "capacity": capacity }),
        ),
    )
    .await
}

async fn order(edge: &Edge, listing_id: &str, quantity: u64) -> (u16, String) {
    edge.post(
        "/market/orders",
        with(
            auth("buyer-box", OTHER_TOKEN),
            json!({ "listing_id": listing_id, "quantity": quantity }),
        ),
    )
    .await
}

fn completed(order_id: &str, session_id: &str, amount: u64) -> Value {
    json!({
        "type": "checkout.session.completed",
        "data": { "object": {
            "id": session_id,
            "client_reference_id": order_id,
            "payment_status": "paid",
            "amount_total": amount,
            "currency": "eur",
        }}
    })
}

#[tokio::test]
async fn every_market_route_is_503_when_the_edge_has_no_market() {
    let edge = Edge::start("market-off", &tenants()).await;

    assert_eq!(browse(&edge).await.0, 503);
    for path in [
        "/market/account",
        "/market/seller/onboard",
        "/market/orders",
        "/market/listings",
    ] {
        let body = with(
            auth("seller-box", GOOD_TOKEN),
            json!({ "kind": "storage", "unit_price": 100, "capacity": 5,
                    "listing_id": "lst_x", "quantity": 1 }),
        );
        let (status, _) = edge.post(path, body).await;
        assert_eq!(status, 503, "{path}");
    }
    assert_eq!(webhook_raw(&edge, b"{}", "t=1,v1=00").await.0, 503);
    edge.shutdown().await;
}

#[tokio::test]
async fn market_routes_are_disabled_when_either_stripe_secret_is_missing() {
    let stripe = StripeStub::start().await;
    let edge = Edge::start_with_market("market-missing-secret", &tenants(), &stripe.base).await;

    std::fs::remove_file(edge.dir.join("webhook.secret")).expect("remove webhook secret");
    assert_eq!(browse(&edge).await.0, 503);
    assert_eq!(order(&edge, "lst_test", 1).await.0, 503);
    assert!(stripe.calls("POST", "/v1/checkout/sessions").is_empty());

    std::fs::write(edge.dir.join("webhook.secret"), WEBHOOK_SECRET)
        .expect("restore webhook secret");
    std::fs::remove_file(edge.dir.join("stripe.key")).expect("remove Stripe key");
    let (status, _) = edge
        .post("/market/account", auth("seller-box", GOOD_TOKEN))
        .await;
    assert_eq!(status, 503);
    assert_eq!(
        edge.client
            .get(format!("{}/health", edge.base))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );

    edge.shutdown().await;
    stripe.shutdown().await;
}

#[tokio::test]
async fn market_routes_need_a_token_and_the_market_bit() {
    let stripe = StripeStub::start().await;
    let edge = Edge::start_with_market("market-auth", &tenants(), &stripe.base).await;

    let (status, _) = edge
        .post("/market/account", auth("seller-box", OTHER_TOKEN))
        .await;
    assert_eq!(status, 401, "wrong token");
    let (status, _) = edge
        .post("/market/account", auth("nobody", GOOD_TOKEN))
        .await;
    assert_eq!(status, 401, "unknown id answers like a wrong token");
    let (status, body) = edge
        .post("/market/account", auth("plain-box", THIRD_TOKEN))
        .await;
    assert_eq!(status, 403, "{body}");

    let (status, body) = edge
        .post("/market/account", auth("seller-box", GOOD_TOKEN))
        .await;
    assert_eq!(status, 200, "{body}");
    let account = parse(&body);
    assert_eq!(account["fee_bps"], 400);
    assert_eq!(account["currency"], "eur");
    assert_eq!(account["seller_onboarded"], false);

    assert!(stripe.seen().is_empty(), "auth failures never reach Stripe");
    edge.shutdown().await;
    stripe.shutdown().await;
}

#[tokio::test]
async fn onboarding_creates_one_account_and_hands_back_a_link() {
    let stripe = StripeStub::start().await;
    let edge = Edge::start_with_market("market-onboard", &tenants(), &stripe.base).await;

    let (status, body) = edge
        .post("/market/seller/onboard", auth("seller-box", GOOD_TOKEN))
        .await;
    assert_eq!(status, 200, "{body}");
    let view = parse(&body);
    assert_eq!(view["ready"], false);
    assert_eq!(view["url"], "https://connect.stripe.test/onboard/abc");

    let created = stripe.calls("POST", "/v1/accounts");
    assert_eq!(created.len(), 1);
    let expected_bearer = ["Bearer", STRIPE_KEY].join(" ");
    assert_eq!(created[0].bearer, expected_bearer);
    assert_eq!(created[0].form["type"], "express");
    assert_eq!(
        created[0].form["capabilities[transfers][requested]"],
        "true"
    );
    assert_eq!(
        created[0].idempotency_key.as_deref(),
        Some("losos-account-seller-box")
    );
    let link = &stripe.calls("POST", "/v1/account_links")[0];
    assert_eq!(link.form["account"], "acct_test_1");
    assert_eq!(link.form["type"], "account_onboarding");
    assert_eq!(link.form["return_url"], RETURN_URL);

    // A second call resumes the same account instead of making another.
    let (status, _) = edge
        .post("/market/seller/onboard", auth("seller-box", GOOD_TOKEN))
        .await;
    assert_eq!(status, 200);
    assert_eq!(stripe.calls("POST", "/v1/accounts").len(), 1);

    // Once Stripe says the account is ready there is nothing left to do.
    stripe.state.account_ready.store(true, Ordering::SeqCst);
    let (_, body) = edge
        .post("/market/seller/onboard", auth("seller-box", GOOD_TOKEN))
        .await;
    let view = parse(&body);
    assert_eq!(view["ready"], true);
    assert!(view["url"].is_null());

    edge.shutdown().await;
    stripe.shutdown().await;
}

#[tokio::test]
async fn nothing_can_be_listed_until_the_seller_is_ready() {
    let stripe = StripeStub::start().await;
    let edge = Edge::start_with_market("market-unready", &tenants(), &stripe.base).await;

    assert_eq!(list(&edge, "storage", 100, 10).await.0, 409, "no account");
    edge.post("/market/seller/onboard", auth("seller-box", GOOD_TOKEN))
        .await;
    assert_eq!(list(&edge, "storage", 100, 10).await.0, 409, "not ready");

    ready_seller(&edge).await;
    let (status, body) = list(&edge, "storage", 100, 10).await;
    assert_eq!(status, 201, "{body}");

    // And a seller Stripe later disables disappears from the shelf.
    assert_eq!(browse(&edge).await.1.as_array().map(Vec::len), Some(1));
    assert_eq!(webhook(&edge, &account_updated(false)).await.0, 200);
    assert_eq!(browse(&edge).await.1.as_array().map(Vec::len), Some(0));

    edge.shutdown().await;
    stripe.shutdown().await;
}

#[tokio::test]
async fn listings_are_validated() {
    let stripe = StripeStub::start().await;
    let edge = Edge::start_with_market("market-validate", &tenants(), &stripe.base).await;
    ready_seller(&edge).await;

    assert_eq!(list(&edge, "storage", 0, 10).await.0, 400, "free");
    assert_eq!(list(&edge, "storage", 100, 0).await.0, 400, "empty");
    assert_eq!(
        list(&edge, "storage", 5, 3).await.0,
        400,
        "can never reach 50"
    );
    assert_eq!(
        list(&edge, "storage", u64::MAX, 3).await.0,
        400,
        "absurd price"
    );
    let (status, _) = list(&edge, "teleportation", 100, 10).await;
    assert!(status == 400 || status == 422, "unknown kind: {status}");

    edge.shutdown().await;
    stripe.shutdown().await;
}

#[tokio::test]
async fn a_purchase_is_a_destination_charge_with_the_four_percent_cut() {
    let stripe = StripeStub::start().await;
    let edge = Edge::start_with_market("market-purchase", &tenants(), &stripe.base).await;
    ready_seller(&edge).await;

    let (status, body) = list(&edge, "compute", 250, 20).await;
    assert_eq!(status, 201, "{body}");
    let listing_id = parse(&body)["listing_id"]
        .as_str()
        .expect("listing id")
        .to_string();

    // The public shelf names no seller.
    let (status, shelf) = browse(&edge).await;
    assert_eq!(status, 200);
    assert_eq!(shelf[0]["id"], listing_id.as_str());
    assert_eq!(shelf[0]["kind"], "compute");
    assert_eq!(shelf[0]["unit"], "vCPU-hour");
    assert_eq!(shelf[0]["unit_price"], 250);
    assert_eq!(shelf[0]["available"], 20);
    assert!(!shelf.to_string().contains("seller-box"));
    assert!(!shelf.to_string().contains("acct_"));

    let (status, body) = order(&edge, &listing_id, 8).await;
    assert_eq!(status, 201, "{body}");
    let checkout = parse(&body);
    assert_eq!(checkout["amount"], 2000);
    assert_eq!(checkout["fee"], 80);
    assert_eq!(checkout["currency"], "eur");
    assert_eq!(
        checkout["checkout_url"],
        "https://checkout.stripe.test/c/pay/cs_test_1"
    );
    let order_id = checkout["order_id"].as_str().expect("order id").to_string();

    let sent = &stripe.calls("POST", "/v1/checkout/sessions")[0];
    assert_eq!(sent.form["mode"], "payment");
    assert_eq!(sent.form["client_reference_id"], order_id.as_str());
    assert_eq!(sent.form["line_items[0][quantity]"], "8");
    assert_eq!(sent.form["line_items[0][price_data][unit_amount]"], "250");
    assert_eq!(sent.form["line_items[0][price_data][currency]"], "eur");
    assert_eq!(sent.form["payment_method_types[]"], "card");
    assert_eq!(
        sent.form["payment_intent_data[application_fee_amount]"],
        "80"
    );
    assert_eq!(
        sent.form["payment_intent_data[transfer_data][destination]"],
        "acct_test_1"
    );
    assert!(sent.form["success_url"].starts_with(RETURN_URL));
    let expires: u64 = sent.form["expires_at"].parse().expect("expires_at");
    assert!(expires > now() + 25 * 60 && expires <= now() + 31 * 60);
    assert_eq!(
        sent.idempotency_key.as_deref(),
        Some(format!("losos-checkout-{order_id}").as_str())
    );

    // Pending reserves the units but is not an entitlement.
    let (_, body) = edge
        .post("/market/account", auth("buyer-box", OTHER_TOKEN))
        .await;
    let buyer = parse(&body);
    assert_eq!(buyer["purchases"][0]["status"], "pending");
    assert_eq!(browse(&edge).await.1[0]["available"], 12);

    // A webhook whose amount disagrees with the order is not fulfilled.
    assert_eq!(
        webhook(&edge, &completed(&order_id, "cs_test_1", 1))
            .await
            .0,
        200
    );
    let (_, body) = edge
        .post("/market/account", auth("buyer-box", OTHER_TOKEN))
        .await;
    assert_eq!(parse(&body)["purchases"][0]["status"], "pending");

    // The real one is.
    assert_eq!(
        webhook(&edge, &completed(&order_id, "cs_test_1", 2000))
            .await
            .0,
        200
    );
    let (_, body) = edge
        .post("/market/account", auth("buyer-box", OTHER_TOKEN))
        .await;
    let buyer = parse(&body);
    assert_eq!(buyer["purchases"][0]["status"], "paid");
    assert_eq!(buyer["purchases"][0]["quantity"], 8);
    assert!(buyer["sales"].as_array().expect("sales").is_empty());
    assert!(!buyer.to_string().contains("seller-box"));

    let (_, body) = edge
        .post("/market/account", auth("seller-box", GOOD_TOKEN))
        .await;
    let seller = parse(&body);
    assert_eq!(seller["seller_ready"], true);
    assert_eq!(seller["sales"][0]["status"], "paid");
    assert_eq!(seller["sales"][0]["amount"], 2000);
    assert_eq!(seller["sales"][0]["fee"], 80);
    assert_eq!(seller["sales"][0]["seller_net"], 1920);
    assert_eq!(seller["listings"][0]["available"], 12);
    assert!(!seller.to_string().contains("buyer-box"));

    // Stripe redelivers events; a repeat changes nothing.
    assert_eq!(
        webhook(&edge, &completed(&order_id, "cs_test_1", 2000))
            .await
            .0,
        200
    );

    edge.shutdown().await;
    stripe.shutdown().await;
}

#[tokio::test]
async fn orders_cannot_oversell_or_buy_from_oneself() {
    let stripe = StripeStub::start().await;
    let edge = Edge::start_with_market("market-oversell", &tenants(), &stripe.base).await;
    ready_seller(&edge).await;
    let (_, body) = list(&edge, "storage", 100, 10).await;
    let listing_id = parse(&body)["listing_id"].as_str().expect("id").to_string();

    let (status, _) = order(&edge, &listing_id, 11).await;
    assert_eq!(status, 409, "more than exists");
    let (status, body) = order(&edge, &listing_id, 6).await;
    assert_eq!(status, 201, "{body}");
    let (status, _) = order(&edge, &listing_id, 5).await;
    assert_eq!(
        status, 409,
        "the rest is held by the first buyer's checkout"
    );
    assert_eq!(order(&edge, &listing_id, 4).await.0, 201);
    assert_eq!(order(&edge, &listing_id, 0).await.0, 400);
    assert_eq!(order(&edge, "lst_missing", 1).await.0, 404);

    let (status, _) = edge
        .post(
            "/market/orders",
            with(
                auth("seller-box", GOOD_TOKEN),
                json!({ "listing_id": listing_id, "quantity": 1 }),
            ),
        )
        .await;
    assert_eq!(status, 400, "self-dealing");

    edge.shutdown().await;
    stripe.shutdown().await;
}

#[tokio::test]
async fn a_failed_checkout_or_an_expired_session_frees_the_units() {
    let stripe = StripeStub::start().await;
    let edge = Edge::start_with_market("market-release", &tenants(), &stripe.base).await;
    ready_seller(&edge).await;
    let (_, body) = list(&edge, "storage", 100, 10).await;
    let listing_id = parse(&body)["listing_id"].as_str().expect("id").to_string();

    stripe.state.fail_checkout.store(true, Ordering::SeqCst);
    let (status, body) = order(&edge, &listing_id, 10).await;
    assert_eq!(status, 502);
    assert!(
        !body.contains("boom") && !body.contains("stripe"),
        "provider detail stays in the log: {body}"
    );
    assert_eq!(browse(&edge).await.1[0]["available"], 10);

    stripe.state.fail_checkout.store(false, Ordering::SeqCst);
    let (status, body) = order(&edge, &listing_id, 10).await;
    assert_eq!(status, 201, "{body}");
    let order_id = parse(&body)["order_id"].as_str().expect("id").to_string();
    assert_eq!(browse(&edge).await.1.as_array().map(Vec::len), Some(0));

    let expired = json!({
        "type": "checkout.session.expired",
        "data": { "object": { "client_reference_id": order_id } }
    });
    assert_eq!(webhook(&edge, &expired).await.0, 200);
    assert_eq!(browse(&edge).await.1[0]["available"], 10);

    edge.shutdown().await;
    stripe.shutdown().await;
}

#[tokio::test]
async fn closing_a_listing_stops_new_orders_but_keeps_paid_ones() {
    let stripe = StripeStub::start().await;
    let edge = Edge::start_with_market("market-close", &tenants(), &stripe.base).await;
    ready_seller(&edge).await;
    let (_, body) = list(&edge, "storage", 100, 10).await;
    let listing_id = parse(&body)["listing_id"].as_str().expect("id").to_string();
    let (_, body) = order(&edge, &listing_id, 2).await;
    let order_id = parse(&body)["order_id"].as_str().expect("id").to_string();
    webhook(&edge, &completed(&order_id, "cs_test_1", 200)).await;

    // Only the seller may close it.
    let (status, _) = edge
        .post(
            "/market/listings/close",
            with(
                auth("buyer-box", OTHER_TOKEN),
                json!({ "listing_id": listing_id }),
            ),
        )
        .await;
    assert_eq!(status, 404);
    let (status, _) = edge
        .post(
            "/market/listings/close",
            with(
                auth("seller-box", GOOD_TOKEN),
                json!({ "listing_id": listing_id }),
            ),
        )
        .await;
    assert_eq!(status, 204);

    assert_eq!(order(&edge, &listing_id, 1).await.0, 404);
    assert_eq!(browse(&edge).await.1.as_array().map(Vec::len), Some(0));
    let (_, body) = edge
        .post("/market/account", auth("buyer-box", OTHER_TOKEN))
        .await;
    assert_eq!(parse(&body)["purchases"][0]["status"], "paid");

    edge.shutdown().await;
    stripe.shutdown().await;
}

#[tokio::test]
async fn webhooks_without_a_valid_signature_change_nothing() {
    let stripe = StripeStub::start().await;
    let edge = Edge::start_with_market("market-webhook", &tenants(), &stripe.base).await;
    ready_seller(&edge).await;
    let (_, body) = list(&edge, "storage", 100, 10).await;
    let listing_id = parse(&body)["listing_id"].as_str().expect("id").to_string();
    let (_, body) = order(&edge, &listing_id, 2).await;
    let order_id = parse(&body)["order_id"].as_str().expect("id").to_string();

    let body = serde_json::to_vec(&completed(&order_id, "cs_test_1", 200)).expect("event");

    let (status, _) = edge
        .client
        .post(format!("{}/market/webhook", edge.base))
        .body(body.clone())
        .send()
        .await
        .map(|r| (r.status().as_u16(), ()))
        .expect("sent");
    assert_eq!(status, 400, "no signature header");

    let wrong = sign_webhook("whsec_someone_else", &body, now());
    assert_eq!(
        webhook_raw(&edge, &body, &wrong).await.0,
        400,
        "wrong secret"
    );

    let stale = sign_webhook(WEBHOOK_SECRET, &body, now() - 3600);
    assert_eq!(webhook_raw(&edge, &body, &stale).await.0, 400, "replayed");

    let signed_for_other_body = sign_webhook(WEBHOOK_SECRET, b"{}", now());
    assert_eq!(
        webhook_raw(&edge, &body, &signed_for_other_body).await.0,
        400,
        "signature of a different body"
    );

    let (_, account) = edge
        .post("/market/account", auth("buyer-box", OTHER_TOKEN))
        .await;
    assert_eq!(parse(&account)["purchases"][0]["status"], "pending");

    edge.shutdown().await;
    stripe.shutdown().await;
}

#[tokio::test]
async fn the_webhook_takes_bodies_the_other_routes_would_refuse() {
    let stripe = StripeStub::start().await;
    let edge = Edge::start_with_market("market-big-webhook", &tenants(), &stripe.base).await;

    let big = json!({ "type": "ping", "data": { "object": { "pad": "x".repeat(100 * 1024) } } });
    assert_eq!(webhook(&edge, &big).await.0, 200);

    let huge = json!({ "type": "ping", "pad": "x".repeat(300 * 1024) });
    assert_eq!(webhook(&edge, &huge).await.0, 413);

    // Everything else keeps the 16 KiB cap.
    let (status, _) = edge
        .post(
            "/market/account",
            with(
                auth("seller-box", GOOD_TOKEN),
                json!({ "pad": "x".repeat(32 * 1024) }),
            ),
        )
        .await;
    assert_eq!(status, 413);

    edge.shutdown().await;
    stripe.shutdown().await;
}

#[tokio::test]
async fn a_malformed_secret_file_refuses_rather_than_calling_stripe() {
    let stripe = StripeStub::start().await;
    let edge = Edge::start_with_market("market-badkey", &tenants(), &stripe.base).await;
    std::fs::write(edge.dir.join("stripe.key"), "pk_test_publishable").expect("overwrite key");

    let (status, _) = edge
        .post("/market/seller/onboard", auth("seller-box", GOOD_TOKEN))
        .await;
    assert_eq!(status, 503);
    assert!(stripe.seen().is_empty(), "the wrong key is never sent");

    edge.shutdown().await;
    stripe.shutdown().await;
}

#[tokio::test]
async fn either_of_two_webhook_secrets_verifies() {
    let stripe = StripeStub::start().await;
    let edge = Edge::start_with_market("market-two-secrets", &tenants(), &stripe.base).await;
    let connect_secret = "whsec_connect_endpoint";
    std::fs::write(
        edge.dir.join("webhook.secret"),
        format!("{WEBHOOK_SECRET}\n{connect_secret}\n"),
    )
    .expect("overwrite webhook secrets");

    let body = serde_json::to_vec(&json!({ "type": "ping" })).expect("event");
    for secret in [WEBHOOK_SECRET, connect_secret] {
        let header = sign_webhook(secret, &body, now());
        assert_eq!(webhook_raw(&edge, &body, &header).await.0, 200, "{secret}");
    }
    let header = sign_webhook("whsec_third", &body, now());
    assert_eq!(webhook_raw(&edge, &body, &header).await.0, 400);

    edge.shutdown().await;
    stripe.shutdown().await;
}

/// List, order and pay for `quantity` units; returns the order id.
async fn buy_paid(edge: &Edge, kind: &str, quantity: u64) -> String {
    ready_seller(edge).await;
    let (status, body) = list(edge, kind, 100, 100).await;
    assert_eq!(status, 201, "{body}");
    let listing_id = parse(&body)["listing_id"].as_str().expect("id").to_string();
    let (status, body) = order(edge, &listing_id, quantity).await;
    assert_eq!(status, 201, "{body}");
    let checkout = parse(&body);
    let order_id = checkout["order_id"].as_str().expect("order id").to_string();
    let amount = checkout["amount"].as_u64().expect("amount");
    assert_eq!(
        webhook(edge, &completed(&order_id, "cs_test_1", amount))
            .await
            .0,
        200
    );
    order_id
}

async fn buyer_account(edge: &Edge) -> Value {
    let (status, body) = edge
        .post("/market/account", auth("buyer-box", OTHER_TOKEN))
        .await;
    assert_eq!(status, 200, "{body}");
    parse(&body)
}

/// Poll the buyer's account until `purchases[0].volume` is set.
async fn wait_for_volume(edge: &Edge) -> Value {
    for _ in 0..100 {
        let account = buyer_account(edge).await;
        if !account["purchases"][0]["volume"].is_null() {
            return account;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the volume was never provisioned");
}

#[tokio::test]
async fn a_paid_storage_order_becomes_a_claim_in_the_buyers_namespace() {
    let stripe = StripeStub::start().await;
    let kube = KubeStub::expecting_bearer(201, common::KUBE_TOKEN).await;
    let edge = Edge::start_with_market_and_mesh(
        "market-fulfil-storage",
        &tenants(),
        &stripe.base,
        MeshFixture::enabled(&kube.base),
    )
    .await;
    let order_id = buy_paid(&edge, "storage", 5).await;

    let account = wait_for_volume(&edge).await;
    let claim = order_id.replace('_', "-");
    assert_eq!(
        account["purchases"][0]["volume"],
        format!("market-buyer-box/{claim}").as_str()
    );
    assert_eq!(account["entitlements"]["storage_gib"], 5);
    assert_eq!(account["entitlements"]["compute_vcpu_hours"], 0);
    let expires = account["purchases"][0]["expires_at"]
        .as_u64()
        .expect("expiry");
    let thirty_days = 30 * 24 * 3600;
    assert!(expires > now() + thirty_days - 60 && expires <= now() + thirty_days);
    assert_eq!(account["purchases"][0]["expired"], false);

    let bodies = kube.bodies();
    let namespace = bodies
        .iter()
        .find(|(k, _)| k == "POST /api/v1/namespaces")
        .expect("a namespace was created");
    assert_eq!(namespace.1["metadata"]["name"], "market-buyer-box");
    let pvc = bodies
        .iter()
        .find(|(k, _)| k == "POST /api/v1/namespaces/market-buyer-box/persistentvolumeclaims")
        .expect("a claim was created");
    assert_eq!(pvc.1["metadata"]["name"], claim.as_str());
    assert_eq!(pvc.1["spec"]["storageClassName"], "longhorn");
    assert_eq!(pvc.1["spec"]["resources"]["requests"]["storage"], "5Gi");
    assert_eq!(pvc.1["spec"]["accessModes"][0], "ReadWriteOnce");

    // Fulfilment is once-only: later reconcile passes create nothing more.
    let seen = kube.seen().len();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(kube.seen().len(), seen);

    edge.shutdown().await;
    kube.shutdown().await;
    stripe.shutdown().await;
}

#[tokio::test]
async fn a_conflict_from_the_apiserver_counts_as_already_provisioned() {
    let stripe = StripeStub::start().await;
    let kube = KubeStub::start(409).await;
    let edge = Edge::start_with_market_and_mesh(
        "market-fulfil-conflict",
        &tenants(),
        &stripe.base,
        MeshFixture::enabled(&kube.base),
    )
    .await;
    buy_paid(&edge, "storage", 2).await;
    let account = wait_for_volume(&edge).await;
    assert_eq!(account["entitlements"]["storage_gib"], 2);

    edge.shutdown().await;
    kube.shutdown().await;
    stripe.shutdown().await;
}

#[tokio::test]
async fn a_refused_claim_stays_pending_and_never_loses_the_payment() {
    let stripe = StripeStub::start().await;
    let kube = KubeStub::start(403).await;
    let edge = Edge::start_with_market_and_mesh(
        "market-fulfil-refused",
        &tenants(),
        &stripe.base,
        MeshFixture::enabled(&kube.base),
    )
    .await;
    buy_paid(&edge, "storage", 3).await;

    // The reconciler keeps retrying …
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(kube.seen().len() >= 2, "{:?}", kube.seen());
    // … and the buyer still holds the entitlement, with no volume yet.
    let account = buyer_account(&edge).await;
    assert_eq!(account["purchases"][0]["status"], "paid");
    assert!(account["purchases"][0]["volume"].is_null());
    assert_eq!(account["entitlements"]["storage_gib"], 3);

    edge.shutdown().await;
    kube.shutdown().await;
    stripe.shutdown().await;
}

#[tokio::test]
async fn compute_orders_are_a_credit_and_touch_no_cluster() {
    let stripe = StripeStub::start().await;
    let kube = KubeStub::start(201).await;
    let edge = Edge::start_with_market_and_mesh(
        "market-fulfil-compute",
        &tenants(),
        &stripe.base,
        MeshFixture::enabled(&kube.base),
    )
    .await;
    buy_paid(&edge, "compute", 12).await;

    tokio::time::sleep(Duration::from_millis(300)).await;
    let account = buyer_account(&edge).await;
    assert_eq!(account["entitlements"]["compute_vcpu_hours"], 12);
    assert_eq!(account["entitlements"]["storage_gib"], 0);
    assert!(account["purchases"][0]["volume"].is_null());
    assert!(kube.seen().is_empty(), "{:?}", kube.seen());

    edge.shutdown().await;
    kube.shutdown().await;
    stripe.shutdown().await;
}

#[tokio::test]
async fn only_what_an_appliance_shares_on_the_mesh_can_be_sold() {
    let stripe = StripeStub::start().await;
    // seller-box is enrolled but not sharing compute; buyer-box is not enrolled.
    let edge = Edge::start_market(
        "market-sharing",
        &tenants(),
        &stripe.base,
        MeshFixture::default(),
        &[("seller-box", false)],
    )
    .await;
    ready_seller(&edge).await;

    let (status, body) = list(&edge, "compute", 100, 10).await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("sharing its compute"), "{body}");
    let (status, body) = list(&edge, "storage", 100, 10).await;
    assert_eq!(status, 201, "{body}");
    let listing_id = parse(&body)["listing_id"].as_str().expect("id").to_string();

    let (_, body) = edge
        .post("/market/account", auth("seller-box", GOOD_TOKEN))
        .await;
    let seller = parse(&body);
    assert_eq!(seller["can_sell_storage"], true);
    assert_eq!(seller["can_sell_compute"], false);
    let buyer = buyer_account(&edge).await;
    assert_eq!(buyer["can_sell_storage"], false);

    // A box that is not on the mesh cannot list at all.
    let (status, _) = edge
        .post("/market/seller/onboard", auth("buyer-box", OTHER_TOKEN))
        .await;
    assert_eq!(status, 200);
    let (status, _) = edge
        .post(
            "/market/listings",
            with(
                auth("buyer-box", OTHER_TOKEN),
                json!({ "kind": "storage", "unit_price": 100, "capacity": 10 }),
            ),
        )
        .await;
    assert_eq!(status, 409);

    // The storage listing is on the shelf while sharing, and orderable.
    assert_eq!(browse(&edge).await.1[0]["id"], listing_id.as_str());

    edge.shutdown().await;
    stripe.shutdown().await;
}

const BOX_UUID: &str = "3f2b8c1e-7a4d-4e9b-9c15-0d6a2b7e4f31";

#[tokio::test]
async fn the_box_uuid_is_written_onto_the_stripe_account() {
    let stripe = StripeStub::start().await;
    let edge = Edge::start_with_market("market-box-uuid", &tenants(), &stripe.base).await;

    let mut body = auth("seller-box", GOOD_TOKEN);
    body["box_uuid"] = json!(BOX_UUID);
    let (status, text) = edge.post("/market/seller/onboard", body.clone()).await;
    assert_eq!(status, 200, "{text}");
    let created = &stripe.calls("POST", "/v1/accounts")[0];
    assert_eq!(created.form["metadata[losos_box_uuid]"], BOX_UUID);
    assert_eq!(created.form["metadata[losos_appliance_id]"], "seller-box");

    // The same UUID again changes nothing at Stripe.
    let (status, _) = edge.post("/market/seller/onboard", body).await;
    assert_eq!(status, 200);
    assert!(stripe.calls("POST", "/v1/accounts/acct_test_1").is_empty());

    // A different one re-tags the existing account rather than making another.
    let mut other = auth("seller-box", GOOD_TOKEN);
    other["box_uuid"] = json!("0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d");
    let (status, _) = edge.post("/market/seller/onboard", other).await;
    assert_eq!(status, 200);
    let tags = stripe.calls("POST", "/v1/accounts/acct_test_1");
    assert_eq!(tags.len(), 1);
    assert_eq!(
        tags[0].form["metadata[losos_box_uuid]"],
        "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d"
    );
    assert_eq!(stripe.calls("POST", "/v1/accounts").len(), 1);

    edge.shutdown().await;
    stripe.shutdown().await;
}

#[tokio::test]
async fn a_box_uuid_that_is_not_a_uuid_is_refused_before_stripe() {
    let stripe = StripeStub::start().await;
    let edge = Edge::start_with_market("market-bad-uuid", &tenants(), &stripe.base).await;
    for bad in [
        "",
        "not-a-uuid",
        "3F2B8C1E-7A4D-4E9B-9C15-0D6A2B7E4F31",
        "x'; drop",
    ] {
        let mut body = auth("seller-box", GOOD_TOKEN);
        body["box_uuid"] = json!(bad);
        let (status, text) = edge.post("/market/seller/onboard", body).await;
        assert_eq!(status, 400, "{bad:?}: {text}");
    }
    assert!(stripe.calls("POST", "/v1/accounts").is_empty());
    edge.shutdown().await;
    stripe.shutdown().await;
}

#[tokio::test]
async fn without_the_gate_the_market_answers_503_and_the_proxy_lives() {
    let stripe = StripeStub::start().await;
    let mut edge = Edge::start_with_market("market-no-gate", &tenants(), &stripe.base).await;
    edge.stop_gate().await;

    let (status, _) = edge
        .post("/market/seller/onboard", auth("seller-box", GOOD_TOKEN))
        .await;
    assert_eq!(status, 503);
    assert!(stripe.calls("POST", "/v1/accounts").is_empty());
    let health = edge
        .client
        .get(format!("{}/health", edge.base))
        .send()
        .await
        .expect("health");
    assert!(health.status().is_success());

    edge.shutdown().await;
    stripe.shutdown().await;
}
