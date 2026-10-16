//! Prices for BTC, XMR and ZEC orders and for the network fee of a USDC
//! payment, against two stand-in price sources.

use std::sync::Arc;

use axum::{extract::Query, routing::get, Json, Router};
use losos_registrar::coins::{price, Coin, PriceBook, PriceError, PriceSource, PRICE_MAX_AGE};
use serde_json::{json, Value};

/// Serve Kraken's and CoinGecko's price routes, answering `kraken` and
/// `gecko` dollars for every coin.
async fn sources(kraken: &'static str, gecko: f64) -> [PriceSource; 2] {
    let app = Router::new()
        .route(
            "/0/public/Ticker",
            get(
                move |Query(q): Query<std::collections::HashMap<String, String>>| async move {
                    let pair = q.get("pair").cloned().unwrap_or_default();
                    Json(json!({"error": [], "result": {format!("X{pair}"): {"c": [kraken, "1"]}}}))
                },
            ),
        )
        .route(
            "/api/v3/simple/price",
            get(
                move |Query(q): Query<std::collections::HashMap<String, String>>| async move {
                    let id = q.get("ids").cloned().unwrap_or_default();
                    let mut reply = serde_json::Map::new();
                    reply.insert(id, json!({"usd": gecko}));
                    Json(Value::Object(reply))
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

#[tokio::test]
async fn agreeing_sources_give_their_mean() {
    let s = sources("60000.00", 60300.0).await;
    assert_eq!(price(&s, Coin::Btc).await, Ok(60_150_000_000));
    assert_eq!(price(&s, Coin::Sol).await, Ok(60_150_000_000));
}

#[tokio::test]
async fn sources_more_than_one_percent_apart_give_no_price() {
    let s = sources("60000.00", 61000.0).await;
    assert_eq!(price(&s, Coin::Xmr).await, Err(PriceError::Disagree));
}

#[tokio::test]
async fn a_silent_source_gives_no_price() {
    let s = [
        PriceSource::Kraken {
            base: "http://127.0.0.1:9".to_string(),
        },
        PriceSource::coingecko(),
    ];
    assert!(matches!(
        price(&s, Coin::Zec).await,
        Err(PriceError::Unreachable("Kraken"))
    ));
}

#[tokio::test]
async fn the_book_serves_fresh_prices_and_forgets_stale_ones() {
    let book = Arc::new(PriceBook::new(sources("3000.5", 3001.0).await));
    assert!(book.snapshot(&Coin::FEES, 1_000).is_empty());
    book.refresh(&Coin::FEES, 1_000).await;
    let snap = book.snapshot(&Coin::FEES, 1_000 + PRICE_MAX_AGE - 1);
    assert_eq!(snap.get("eth").map(String::as_str), Some("3000750000"));
    assert_eq!(snap.get("sol").map(String::as_str), Some("3000750000"));
    assert!(book.snapshot(&Coin::FEES, 1_000 + PRICE_MAX_AGE).is_empty());
}
