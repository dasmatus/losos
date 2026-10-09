//! Integration tests for the widget builder.
//!
//! The real routes run on a real socket, with the market's real Stripe gate,
//! against two stand-ins: one for Stripe (credit checkouts) and one for the
//! Anthropic API (Managed Agents sessions and the Files API). What is asserted
//! is what the edge asks Anthropic for (a session under a hard cap the
//! balance covers), what it charges the box (the tokens at Opus list price
//! plus the markup), and what it refuses (a build with no balance, a box the
//! operator never cleared for the market, an unsigned top-up).

mod common;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use common::{Edge, TempDir, TenantSpec, GOOD_TOKEN, OTHER_TOKEN, WEBHOOK_SECRET};
use losos_registrar::builder::BuilderOpts;
use losos_registrar::market::sign_webhook;
use losos_registrar::opts::ServeOpts;
use serde_json::{json, Value};
use tokio::sync::oneshot;

const CLAUDE_KEY: &str = "sk-ant-api03-test-0123456789";
const WIDGET: &str = "<div class=\"clock\">12:00</div>";

/// One request a stub saw.
#[derive(Debug, Clone)]
struct Seen {
    method: String,
    path: String,
    query: String,
    headers: HashMap<String, String>,
    body: String,
}

#[derive(Clone, Default)]
struct StubState {
    seen: Arc<Mutex<Vec<Seen>>>,
    /// How many times a session has been read.
    polls: Arc<AtomicU64>,
    checkouts: Arc<AtomicU64>,
}

/// A stub HTTP server answering with `handler`.
struct Stub {
    base: String,
    state: StubState,
    stop: Option<oneshot::Sender<()>>,
}

impl Stub {
    async fn start(handler: fn(&StubState, &Seen) -> Response) -> Self {
        let state = StubState::default();
        let shared = state.clone();
        let app = Router::new().fallback(
            move |headers: HeaderMap, method: Method, uri: Uri, body: String| {
                let state = shared.clone();
                async move {
                    let seen = Seen {
                        method: method.to_string(),
                        path: uri.path().to_string(),
                        query: uri.query().unwrap_or("").to_string(),
                        headers: headers
                            .iter()
                            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                            .collect(),
                        body,
                    };
                    state.seen.lock().expect("log").push(seen.clone());
                    handler(&state, &seen)
                }
            },
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind stub");
        let port = listener.local_addr().expect("local_addr").port();
        let (stop, rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
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
        }
    }

    fn calls(&self, method: &str, path: &str) -> Vec<Seen> {
        self.state
            .seen
            .lock()
            .expect("log")
            .iter()
            .filter(|s| s.method == method && s.path == path)
            .cloned()
            .collect()
    }

    fn shutdown(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

fn stripe(state: &StubState, seen: &Seen) -> Response {
    if (seen.method.as_str(), seen.path.as_str()) == ("POST", "/v1/checkout/sessions") {
        let n = state.checkouts.fetch_add(1, Ordering::SeqCst) + 1;
        return Json(json!({
            "id": format!("cs_test_{n}"),
            "url": format!("https://checkout.stripe.test/c/pay/cs_test_{n}"),
        }))
        .into_response();
    }
    StatusCode::NOT_FOUND.into_response()
}

/// The Anthropic API: one session that runs for two reads, then sits idle
/// having written a widget and a summary.
fn anthropic(state: &StubState, seen: &Seen) -> Response {
    match (seen.method.as_str(), seen.path.as_str()) {
        ("POST", "/v1/sessions") => {
            Json(json!({ "id": "sesn_1", "status": "running" })).into_response()
        }
        ("GET", "/v1/sessions/sesn_1") => {
            let n = state.polls.fetch_add(1, Ordering::SeqCst);
            let status = if n < 2 { "running" } else { "idle" };
            Json(json!({
                "id": "sesn_1",
                "status": status,
                "usage": {
                    "input_tokens": 200_000,
                    "output_tokens": 10_000,
                    "cache_creation_input_tokens": 0,
                    "cache_read_input_tokens": 0,
                    "list_cost": { "amount": "100", "currency": "USD" },
                },
            }))
            .into_response()
        }
        ("GET", "/v1/files") => Json(json!({ "data": [
            { "id": "file_w", "filename": "widget.html", "size_bytes": WIDGET.len(),
              "created_at": "2026-10-09T10:00:00Z" },
            { "id": "file_s", "filename": "summary.txt", "size_bytes": 12,
              "created_at": "2026-10-09T10:00:01Z" },
        ]}))
        .into_response(),
        ("GET", "/v1/files/file_w/content") => WIDGET.into_response(),
        ("GET", "/v1/files/file_s/content") => "A big clock.".into_response(),
        ("DELETE", _) => StatusCode::OK.into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

fn tenants() -> Vec<TenantSpec> {
    vec![
        TenantSpec::new("owner-box", "owner.example", GOOD_TOKEN).with_market(),
        // Authenticates, but the operator never cleared it for the market.
        TenantSpec::new("plain-box", "plain.example", OTHER_TOKEN),
    ]
}

fn builder_on(api: String) -> impl FnOnce(&mut ServeOpts, &TempDir) + Send + 'static {
    move |opts, dir| {
        std::fs::write(dir.join("claude.key"), CLAUDE_KEY).expect("write key");
        opts.builder = Some(Box::new(BuilderOpts {
            state_file: dir.path_str("builder.json"),
            key_file: dir.path_str("claude.key"),
            api,
            agent_id: "agent_011test".to_string(),
            environment_id: "env_011test".to_string(),
            markup_bps: losos_registrar::builder::DEFAULT_MARKUP_BPS,
            usd_rate_ppm: 1_000_000,
            packs: vec![500, 1_000, 2_000],
            max_build_cents: 300,
            poll: Duration::from_millis(20),
        }));
    }
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
    serde_json::from_str(body).unwrap_or_else(|_| panic!("not JSON: {body}"))
}

async fn webhook(edge: &Edge, event: &Value) -> u16 {
    let body = serde_json::to_vec(event).expect("serialize event");
    let signature = sign_webhook(WEBHOOK_SECRET, &body, losos_registrar::market::now_secs());
    edge.client
        .post(format!("{}/market/webhook", edge.base))
        .header("stripe-signature", signature)
        .body(body)
        .send()
        .await
        .expect("webhook reaches the registrar")
        .status()
        .as_u16()
}

fn paid(order_id: &str, session_id: &str, amount: u64) -> Value {
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

async fn account(edge: &Edge) -> Value {
    let (status, body) = edge
        .post("/builder/account", auth("owner-box", GOOD_TOKEN))
        .await;
    assert_eq!(status, 200, "{body}");
    parse(&body)
}

/// Top the owner's balance up by `amount` through a real checkout and a
/// signed webhook.
async fn top_up(edge: &Edge, amount: u64) {
    let (status, body) = edge
        .post(
            "/builder/credits",
            with(auth("owner-box", GOOD_TOKEN), json!({ "amount": amount })),
        )
        .await;
    assert_eq!(status, 201, "{body}");
    let credit = parse(&body);
    let order_id = credit["order_id"].as_str().expect("order id");
    assert!(order_id.starts_with("cr_"), "{credit}");
    let url = credit["checkout_url"].as_str().expect("url");
    let session = url.rsplit('/').next().expect("session id");
    assert_eq!(webhook(edge, &paid(order_id, session, amount)).await, 200);
}

#[tokio::test]
async fn every_builder_route_is_503_when_the_edge_has_no_builder() {
    let stripe = Stub::start(stripe).await;
    let edge = Edge::start_with_market("builder-off", &tenants(), &stripe.base).await;
    for (path, extra) in [
        ("/builder/account", json!({})),
        ("/builder/credits", json!({ "amount": 500 })),
        ("/builder/builds", json!({ "prompt": "a clock" })),
        ("/builder/build", json!({ "build_id": "bld_x" })),
    ] {
        let (status, body) = edge
            .post(path, with(auth("owner-box", GOOD_TOKEN), extra))
            .await;
        assert_eq!(status, 503, "{path}: {body}");
    }
    edge.shutdown().await;
    stripe.shutdown();
}

#[tokio::test]
async fn only_an_authenticated_market_tenant_reaches_the_builder() {
    let stripe = Stub::start(stripe).await;
    let claude = Stub::start(anthropic).await;
    let edge = Edge::start_market_custom(
        "builder-auth",
        &tenants(),
        &stripe.base,
        builder_on(claude.base.clone()),
    )
    .await;

    let (status, _) = edge
        .post("/builder/account", auth("owner-box", OTHER_TOKEN))
        .await;
    assert_eq!(status, 401, "wrong token");
    let (status, _) = edge
        .post("/builder/account", auth("plain-box", OTHER_TOKEN))
        .await;
    assert_eq!(status, 403, "not cleared for the market");

    let view = account(&edge).await;
    assert_eq!(view["currency"], "eur");
    assert_eq!(view["balance"], 0);
    assert_eq!(view["can_build"], false);
    assert_eq!(view["packs"], json!([500, 1000, 2000]));
    // Opus at $4 / $20 per million, plus 20%.
    assert_eq!(view["price"]["input_per_million"], 480);
    assert_eq!(view["price"]["output_per_million"], 2400);
    assert_eq!(view["price"]["markup_percent"], 20.0);

    edge.shutdown().await;
    stripe.shutdown();
    claude.shutdown();
}

#[tokio::test]
async fn a_build_with_no_balance_is_refused_before_anthropic_is_asked() {
    let stripe = Stub::start(stripe).await;
    let claude = Stub::start(anthropic).await;
    let edge = Edge::start_market_custom(
        "builder-broke",
        &tenants(),
        &stripe.base,
        builder_on(claude.base.clone()),
    )
    .await;

    let (status, body) = edge
        .post(
            "/builder/builds",
            with(
                auth("owner-box", GOOD_TOKEN),
                json!({ "prompt": "a clock" }),
            ),
        )
        .await;
    assert_eq!(status, 409, "{body}");
    assert!(claude.calls("POST", "/v1/sessions").is_empty());

    edge.shutdown().await;
    stripe.shutdown();
    claude.shutdown();
}

#[tokio::test]
async fn a_top_up_is_a_pack_and_counts_only_once_it_is_paid() {
    let stripe = Stub::start(stripe).await;
    let claude = Stub::start(anthropic).await;
    let edge = Edge::start_market_custom(
        "builder-topup",
        &tenants(),
        &stripe.base,
        builder_on(claude.base.clone()),
    )
    .await;

    let (status, _) = edge
        .post(
            "/builder/credits",
            with(auth("owner-box", GOOD_TOKEN), json!({ "amount": 777 })),
        )
        .await;
    assert_eq!(status, 400, "not a pack");

    let (status, body) = edge
        .post(
            "/builder/credits",
            with(auth("owner-box", GOOD_TOKEN), json!({ "amount": 1000 })),
        )
        .await;
    assert_eq!(status, 201, "{body}");
    let credit = parse(&body);
    assert_eq!(account(&edge).await["balance"], 0, "unpaid");

    // A plain platform charge for the pack, in the edge's currency.
    let forms = stripe.calls("POST", "/v1/checkout/sessions");
    let form = &forms.last().expect("a checkout").body;
    assert!(form.contains("mode=payment"), "{form}");
    assert!(form.contains("1000"), "{form}");
    assert!(!form.contains("application_fee"), "{form}");
    assert!(!form.contains("transfer_data"), "{form}");

    let order_id = credit["order_id"].as_str().expect("id");
    // The wrong amount does not credit anything.
    assert_eq!(webhook(&edge, &paid(order_id, "cs_test_1", 500)).await, 200);
    assert_eq!(account(&edge).await["balance"], 0);
    // The right one does, once.
    assert_eq!(
        webhook(&edge, &paid(order_id, "cs_test_1", 1000)).await,
        200
    );
    assert_eq!(
        webhook(&edge, &paid(order_id, "cs_test_1", 1000)).await,
        200
    );
    assert_eq!(account(&edge).await["balance"], 1000);

    edge.shutdown().await;
    stripe.shutdown();
    claude.shutdown();
}

#[tokio::test]
async fn a_build_runs_under_a_cap_and_charges_the_tokens_plus_the_markup() {
    let stripe = Stub::start(stripe).await;
    let claude = Stub::start(anthropic).await;
    let edge = Edge::start_market_custom(
        "builder-run",
        &tenants(),
        &stripe.base,
        builder_on(claude.base.clone()),
    )
    .await;
    top_up(&edge, 500).await;
    assert_eq!(account(&edge).await["can_build"], true);

    let (status, body) = edge
        .post(
            "/builder/builds",
            with(
                auth("owner-box", GOOD_TOKEN),
                json!({ "prompt": "a big clock", "lang": "sk" }),
            ),
        )
        .await;
    assert_eq!(status, 201, "{body}");
    let started = parse(&body);
    assert_eq!(started["status"], "running");
    let build_id = started["id"].as_str().expect("id").to_string();

    // One running build per box.
    let (status, _) = edge
        .post(
            "/builder/builds",
            with(
                auth("owner-box", GOOD_TOKEN),
                json!({ "prompt": "another" }),
            ),
        )
        .await;
    assert_eq!(status, 409);

    // The session: the configured agent, a hard cap, the key in a header.
    let create = claude.calls("POST", "/v1/sessions");
    let create = create.first().expect("a session");
    assert_eq!(
        create.headers.get("x-api-key").map(String::as_str),
        Some(CLAUDE_KEY)
    );
    assert!(create
        .headers
        .get("anthropic-beta")
        .is_some_and(|b| b.contains("managed-agents-")));
    let body: Value = serde_json::from_str(&create.body).expect("json");
    assert_eq!(body["agent"], "agent_011test");
    assert_eq!(body["environment_id"], "env_011test");
    assert_eq!(body["budget"]["type"], "limit");
    assert_eq!(body["budget"]["max_list_cost"]["currency"], "USD");
    let cap: u64 = body["budget"]["max_list_cost"]["amount"]
        .as_str()
        .and_then(|a| a.parse().ok())
        .expect("cap");
    // 500 at list price plus 20% covers 416 at list; the build cap is 300.
    assert_eq!(cap, 300);
    let text = body["initial_events"][0]["content"][0]["text"]
        .as_str()
        .expect("text");
    assert!(text.contains("a big clock"), "{text}");
    assert!(text.contains("Slovak"), "{text}");

    let mut done = Value::Null;
    for _ in 0..200 {
        let (_, body) = edge
            .post(
                "/builder/build",
                with(
                    auth("owner-box", GOOD_TOKEN),
                    json!({ "build_id": build_id }),
                ),
            )
            .await;
        done = parse(&body);
        if done["status"] != "running" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(done["status"], "done", "{done}");
    assert_eq!(done["source"], WIDGET);
    assert_eq!(done["notes"], "A big clock.");
    // 200k in at 4.80 and 10k out at 24.00 per million: 96 + 24.
    assert_eq!(done["charged"], 120);
    assert_eq!(account(&edge).await["balance"], 380);

    // The other box sees nothing of it.
    let (status, _) = edge
        .post(
            "/builder/build",
            with(
                auth("plain-box", OTHER_TOKEN),
                json!({ "build_id": build_id }),
            ),
        )
        .await;
    assert_eq!(status, 403);

    // The session and its files are deleted afterwards.
    assert!(
        edge.wait_for(|| !claude.calls("DELETE", "/v1/sessions/sesn_1").is_empty()
            && !claude.calls("DELETE", "/v1/files/file_w").is_empty())
            .await
    );
    let list = claude.calls("GET", "/v1/files");
    assert!(list.iter().all(|s| s.query.contains("scope_id=sesn_1")));

    edge.shutdown().await;
    stripe.shutdown();
    claude.shutdown();
}
