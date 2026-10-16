//! What the Bitcoin, Monero and Zcash order tests share: a price feed, a
//! Stripe stub, an edge with USDC on and one coin set by the caller, and the
//! buyer's two calls.

use axum::{extract::Query, routing::get, Json, Router};
use losos_registrar::coins::PriceSource;
use losos_registrar::market::MarketOpts;
use losos_registrar::stablecoin::{network, UsdcChain, UsdcOpts};
use serde_json::{json, Value};

use super::{Edge, TenantSpec, GOOD_TOKEN, OTHER_TOKEN};

pub fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port()
}

/// Kraken and CoinGecko both saying `usd` dollars for every coin.
pub async fn prices(usd: &'static str) -> [PriceSource; 2] {
    let app = Router::new()
        .route(
            "/0/public/Ticker",
            get(
                move |Query(q): Query<std::collections::HashMap<String, String>>| async move {
                    let pair = q.get("pair").cloned().unwrap_or_default();
                    Json(json!({"error": [], "result": {pair: {"c": [usd, "1"]}}}))
                },
            ),
        )
        .route(
            "/api/v3/simple/price",
            get(
                move |Query(q): Query<std::collections::HashMap<String, String>>| async move {
                    let id = q.get("ids").cloned().unwrap_or_default();
                    Json(json!({id: {"usd": usd.parse::<f64>().unwrap()}}))
                },
            ),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let base = format!("http://{}", listener.local_addr().expect("addr"));
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    [
        PriceSource::Kraken { base: base.clone() },
        PriceSource::CoinGecko { base },
    ]
}

/// A Stripe API that knows nothing: every call is a 404.
pub async fn start_stripe() -> String {
    let app = Router::new().fallback(|| async { axum::http::StatusCode::NOT_FOUND });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind the stripe stub");
    let port = listener.local_addr().expect("local_addr").port();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    format!("http://127.0.0.1:{port}")
}

/// An edge selling in dollars with USDC on Base Sepolia, a seller box and a
/// buyer box; `coin` switches the coin under test on.
pub async fn start_coin_edge(
    tag: &str,
    prices: [PriceSource; 2],
    coin: impl FnOnce(&mut MarketOpts) + Send + 'static,
) -> Edge {
    let stripe = start_stripe().await;
    Edge::start_market_full(
        tag,
        &[
            TenantSpec::new("seller-box", "seller.example", GOOD_TOKEN).with_market(),
            TenantSpec::new("buyer-box", "buyer.example", OTHER_TOKEN).with_market(),
        ],
        &stripe,
        &[("seller-box", false)],
        None,
        move |opts, dir| {
            std::fs::write(dir.join("usdc-rpc-url"), "http://127.0.0.1:9").expect("rpc url");
            let market = opts.market.as_mut().expect("market on");
            market.currency = "usd".to_string();
            market.usdc = Some(UsdcOpts {
                chains: vec![UsdcChain {
                    network: network("base-sepolia").unwrap(),
                    rpc_file: dir.path_str("usdc-rpc-url"),
                    pay: Some("0x1111111111111111111111111111111111111111".to_string()),
                    treasury: "0x4444444444444444444444444444444444444444".to_string(),
                    genesis: None,
                }],
                ens_rpc_file: None,
                price_sources: prices,
                releaser_file: None,
            });
            coin(market);
        },
    )
    .await
}

pub fn auth(id: &str, token: &str, extra: Value) -> Value {
    let mut doc = json!({ "appliance_id": id, "token": token });
    for (k, v) in extra.as_object().expect("object") {
        doc[k] = v.clone();
    }
    doc
}

pub fn parse(body: &str) -> Value {
    serde_json::from_str(body).unwrap_or_else(|_| panic!("not JSON: {body}"))
}

/// The buyer asks to pay `order_id` in `coin`.
pub async fn pay(edge: &Edge, order_id: &str, coin: &str) -> Value {
    let (status, body) = edge
        .post(
            "/market/coins/pay",
            auth(
                "buyer-box",
                OTHER_TOKEN,
                json!({"order_id": order_id, "coin": coin}),
            ),
        )
        .await;
    assert_eq!(status, 200, "{body}");
    parse(&body)
}

/// The buyer orders 100 units of `listing`.
pub async fn order(edge: &Edge, listing: &str) -> Value {
    let (status, body) = edge
        .post(
            "/market/orders",
            auth(
                "buyer-box",
                OTHER_TOKEN,
                json!({"listing_id": listing, "quantity": 100}),
            ),
        )
        .await;
    assert_eq!(status, 201, "{body}");
    parse(&body)
}
