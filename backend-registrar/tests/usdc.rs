//! Integration tests for USDC settlement.
//!
//! The real routes run on a real socket. The chain is a stand-in JSON-RPC
//! endpoint on loopback that answers the calls the registrar makes:
//! `eth_chainId`, the pay contract's `usdc()` and `treasury()`, receipts and
//! the block height. What is asserted is when an order counts as paid: only
//! for a receipt of the configured contract, on the configured chain, with
//! the token and treasury the config names, a block on top, the amounts of
//! the order and the payout address the invoice named.

mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::response::IntoResponse;
use axum::{Json, Router};
use common::{Edge, TenantSpec, GOOD_TOKEN, OTHER_TOKEN};
use k256::ecdsa::SigningKey;
use losos_registrar::stablecoin::{
    order_id_hash_hex, payout_message, UsdcOpts, BASE_SEPOLIA, USDC_BASE_SEPOLIA,
};
use serde_json::{json, Value};
use sha3::{Digest, Keccak256};

const PAY: &str = "0x1111111111111111111111111111111111111111";
const TREASURY: &str = "0x4444444444444444444444444444444444444444";
const PAID_TOPIC: &str = "Paid(bytes32,address,address,uint256,uint256)";

#[derive(Clone)]
struct Chain {
    inner: Arc<Mutex<ChainState>>,
}

struct ChainState {
    chain_id: u64,
    usdc: String,
    treasury: String,
    head: u64,
    receipts: HashMap<String, Value>,
}

impl Chain {
    fn set(&self, f: impl FnOnce(&mut ChainState)) {
        f(&mut self.inner.lock().expect("chain not poisoned"));
    }
}

async fn rpc(State(chain): State<Chain>, Json(req): Json<Value>) -> impl IntoResponse {
    let st = chain.inner.lock().expect("chain not poisoned");
    let word = |address: &str| format!("0x000000000000000000000000{}", &address[2..]);
    let result = match req["method"].as_str().unwrap_or("") {
        "eth_chainId" => json!(format!("0x{:x}", st.chain_id)),
        "eth_blockNumber" => json!(format!("0x{:x}", st.head)),
        "eth_call" => match req["params"][0]["data"].as_str().unwrap_or("") {
            "0x3e413bee" => json!(word(&st.usdc)),
            "0x61d027b3" => json!(word(&st.treasury)),
            _ => Value::Null,
        },
        "eth_getTransactionReceipt" => st
            .receipts
            .get(req["params"][0].as_str().unwrap_or(""))
            .cloned()
            .unwrap_or(Value::Null),
        _ => Value::Null,
    };
    Json(json!({ "jsonrpc": "2.0", "id": req["id"], "result": result }))
}

async fn start_chain() -> (Chain, String) {
    let chain = Chain {
        inner: Arc::new(Mutex::new(ChainState {
            chain_id: BASE_SEPOLIA,
            usdc: USDC_BASE_SEPOLIA.to_string(),
            treasury: TREASURY.to_string(),
            head: 100,
            receipts: HashMap::new(),
        })),
    };
    let app = Router::new()
        .route("/", axum::routing::post(rpc))
        .with_state(chain.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind the chain stub");
    let port = listener.local_addr().expect("local_addr").port();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (chain, format!("http://127.0.0.1:{port}/"))
}

/// Answers nothing: USDC orders never reach Stripe, and the gate only needs
/// a test-mode key to tell the registrar which ledger it is on.
async fn start_stripe() -> String {
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

fn tenants() -> Vec<TenantSpec> {
    vec![
        TenantSpec::new("seller-box", "seller.example", GOOD_TOKEN).with_market(),
        TenantSpec::new("buyer-box", "buyer.example", OTHER_TOKEN).with_market(),
    ]
}

async fn start(tag: &str, rpc_url: &str, state: Option<&Value>) -> Edge {
    let stripe = start_stripe().await;
    let rpc_url = rpc_url.to_string();
    Edge::start_market_full(
        tag,
        &tenants(),
        &stripe,
        &[("seller-box", false)],
        state,
        move |opts, dir| {
            std::fs::write(dir.join("usdc-rpc-url"), &rpc_url).expect("write rpc url");
            let market = opts.market.as_mut().expect("market on");
            market.currency = "usd".to_string();
            market.usdc = Some(UsdcOpts {
                chain_id: BASE_SEPOLIA,
                rpc_file: dir.path_str("usdc-rpc-url"),
                usdc: USDC_BASE_SEPOLIA.to_string(),
                pay: PAY.to_string(),
                treasury: TREASURY.to_string(),
            });
        },
    )
    .await
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

fn keccak(data: &[u8]) -> [u8; 32] {
    Keccak256::digest(data).into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A wallet: its lowercase address and a `personal_sign` of `message`.
fn wallet(seed: u8) -> (SigningKey, String) {
    let mut secret = [0u8; 32];
    secret[31] = seed;
    let key = SigningKey::from_bytes((&secret).into()).expect("key");
    let point = key.verifying_key().to_sec1_point(false);
    let address = format!("0x{}", hex(&keccak(&point.as_bytes()[1..])[12..]));
    (key, address)
}

fn personal_sign(key: &SigningKey, message: &str) -> String {
    let mut data = format!("\x19Ethereum Signed Message:\n{}", message.len()).into_bytes();
    data.extend_from_slice(message.as_bytes());
    let (signature, recid) = key.sign_prehash_recoverable(&keccak(&data));
    format!(
        "0x{}{:02x}",
        hex(&signature.to_bytes()),
        u8::from(recid) + 27
    )
}

async fn set_payout(edge: &Edge, seed: u8) -> String {
    let (key, address) = wallet(seed);
    let signature = personal_sign(&key, &payout_message(BASE_SEPOLIA, &address));
    let (status, body) = edge
        .post(
            "/market/payout",
            with(
                auth("seller-box", GOOD_TOKEN),
                json!({ "address": address, "signature": signature }),
            ),
        )
        .await;
    assert_eq!(status, 204, "{body}");
    address
}

/// List 100 GiB at 5 cents and order `quantity` of it. The invoice.
async fn order(edge: &Edge, quantity: u64) -> Value {
    let (status, body) = edge
        .post(
            "/market/listings",
            with(
                auth("seller-box", GOOD_TOKEN),
                json!({ "kind": "storage", "unit_price": 5, "capacity": 100 }),
            ),
        )
        .await;
    assert_eq!(status, 201, "{body}");
    let listing = parse(&body)["listing_id"].as_str().expect("id").to_string();
    let (status, body) = edge
        .post(
            "/market/orders",
            with(
                auth("buyer-box", OTHER_TOKEN),
                json!({ "listing_id": listing, "quantity": quantity }),
            ),
        )
        .await;
    assert_eq!(status, 201, "{body}");
    let checkout = parse(&body);
    assert!(checkout.get("checkout_url").is_none(), "{checkout}");
    checkout["stablecoin"].clone()
}

fn word_u(n: u128) -> String {
    format!("{n:064x}")
}

/// A mined `LososPay.pay` paying `invoice` to `seller`, in block `block`.
fn receipt(invoice: &Value, seller: &str, block: u64) -> Value {
    let gross: u128 = invoice["gross"].as_str().unwrap().parse().unwrap();
    let fee: u128 = invoice["fee"].as_str().unwrap().parse().unwrap();
    json!({
        "status": "0x1",
        "blockNumber": format!("0x{block:x}"),
        "logs": [{
            "address": PAY,
            "topics": [
                format!("0x{}", hex(&keccak(PAID_TOPIC.as_bytes()))),
                invoice["order_id_hash"],
                "0x0000000000000000000000003333333333333333333333333333333333333333",
                format!("0x000000000000000000000000{}", &seller[2..]),
            ],
            "data": format!("0x{}{}", word_u(gross), word_u(fee)),
        }],
    })
}

async fn confirm(edge: &Edge, order_id: &str, tx: &str) -> (u16, String) {
    edge.post(
        "/market/confirm",
        with(
            auth("buyer-box", OTHER_TOKEN),
            json!({ "order_id": order_id, "tx": tx }),
        ),
    )
    .await
}

async fn status_of(edge: &Edge, order_id: &str) -> String {
    let (status, body) = edge
        .post("/market/account", auth("buyer-box", OTHER_TOKEN))
        .await;
    assert_eq!(status, 200, "{body}");
    parse(&body)["purchases"]
        .as_array()
        .expect("purchases")
        .iter()
        .find(|o| o["id"] == order_id)
        .map_or_else(String::new, |o| {
            o["status"].as_str().unwrap_or("").to_string()
        })
}

fn tx(n: u8) -> String {
    format!("0x{}", hex(&[n; 32]))
}

#[tokio::test]
async fn a_payment_counts_once_it_has_a_block_on_top() {
    let (chain, url) = start_chain().await;
    let edge = start("usdc-paid", &url, None).await;
    let seller = set_payout(&edge, 7).await;
    let invoice = order(&edge, 40).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    // 40 GiB at 5 cents is 2 USD, 4% of it the fee.
    assert_eq!(invoice["gross"], "2000000");
    assert_eq!(invoice["fee"], "80000");
    assert_eq!(invoice["seller"], seller.as_str());
    assert_eq!(
        invoice["order_id_hash"],
        order_id_hash_hex(&order_id).as_str()
    );
    assert!(invoice["pay_before"].as_u64().unwrap() > losos_registrar::market::now_secs());

    chain.set(|c| {
        c.receipts.insert(tx(1), receipt(&invoice, &seller, 100));
    });
    let (status, body) = confirm(&edge, &order_id, &tx(1)).await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("another block"), "{body}");
    assert_eq!(status_of(&edge, &order_id).await, "pending");

    chain.set(|c| c.head = 101);
    let (status, body) = confirm(&edge, &order_id, &tx(1)).await;
    assert_eq!(status, 204, "{body}");
    assert_eq!(status_of(&edge, &order_id).await, "paid");
    // Reporting the same transaction again is not an error.
    assert_eq!(confirm(&edge, &order_id, &tx(1)).await.0, 204);
    edge.shutdown().await;
}

#[tokio::test]
async fn an_endpoint_on_another_chain_settles_nothing() {
    let (chain, url) = start_chain().await;
    let edge = start("usdc-wrong-chain", &url, None).await;
    let seller = set_payout(&edge, 7).await;
    let invoice = order(&edge, 10).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    chain.set(|c| {
        // Base mainnet behind a Sepolia config.
        c.chain_id = 8453;
        c.head = 200;
        c.receipts.insert(tx(2), receipt(&invoice, &seller, 100));
    });
    let (status, body) = confirm(&edge, &order_id, &tx(2)).await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("different chain"), "{body}");
    assert_eq!(status_of(&edge, &order_id).await, "pending");
    edge.shutdown().await;
}

#[tokio::test]
async fn a_pay_contract_over_another_token_settles_nothing() {
    let (chain, url) = start_chain().await;
    let edge = start("usdc-wrong-token", &url, None).await;
    let seller = set_payout(&edge, 7).await;
    let invoice = order(&edge, 10).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    chain.set(|c| {
        // A contract deployed over a token anyone can mint.
        c.usdc = "0x5555555555555555555555555555555555555555".to_string();
        c.head = 200;
        c.receipts.insert(tx(3), receipt(&invoice, &seller, 100));
    });
    let (status, body) = confirm(&edge, &order_id, &tx(3)).await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("Circle's USDC"), "{body}");

    chain.set(|c| {
        c.usdc = USDC_BASE_SEPOLIA.to_string();
        c.treasury = "0x6666666666666666666666666666666666666666".to_string();
    });
    let (status, body) = confirm(&edge, &order_id, &tx(3)).await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("different treasury"), "{body}");
    assert_eq!(status_of(&edge, &order_id).await, "pending");
    edge.shutdown().await;
}

#[tokio::test]
async fn a_payout_change_after_the_order_does_not_refuse_its_payment() {
    let (chain, url) = start_chain().await;
    let edge = start("usdc-payout-change", &url, None).await;
    let first = set_payout(&edge, 7).await;
    let invoice = order(&edge, 10).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    // The seller moves to another wallet while the buyer is paying.
    let second = set_payout(&edge, 8).await;
    assert_ne!(first, second);
    chain.set(|c| {
        c.head = 200;
        c.receipts.insert(tx(4), receipt(&invoice, &first, 100));
        c.receipts.insert(tx(5), receipt(&invoice, &second, 100));
    });
    // A payment to the new address is not what the invoice asked for.
    let (status, body) = confirm(&edge, &order_id, &tx(5)).await;
    assert_eq!(status, 409, "{body}");
    let (status, body) = confirm(&edge, &order_id, &tx(4)).await;
    assert_eq!(status, 204, "{body}");
    assert_eq!(status_of(&edge, &order_id).await, "paid");
    edge.shutdown().await;
}

/// A ledger with one USDC order made an hour ago, long past its hold.
fn lapsed_ledger(capacity: u64, quantity: u64, seller: &str) -> Value {
    let now = losos_registrar::market::now_secs();
    json!({
        "mode": "test",
        "sellers": { "seller-box": { "account_id": "", "ready": false, "payout": seller } },
        "listings": { "lst_1": {
            "id": "lst_1", "seller": "seller-box", "kind": "storage",
            "unit_price": 5, "capacity": capacity, "active": true, "created_at": now - 7200,
        }},
        "orders": { "ord_late": {
            "id": "ord_late", "listing_id": "lst_1", "buyer": "buyer-box",
            "seller": "seller-box", "kind": "storage", "quantity": quantity,
            "unit_price": 5, "amount": quantity * 5, "fee": 0, "currency": "usd",
            "status": "pending", "session_id": null, "created_at": now - 3600,
            "paid_at": null, "expires_at": null, "rail": "usdc", "pay_to": seller,
        }},
    })
}

fn late_invoice(quantity: u64) -> Value {
    json!({
        "order_id_hash": order_id_hash_hex("ord_late"),
        "gross": (u128::from(quantity) * 5 * 10_000).to_string(),
        "fee": "0",
    })
}

#[tokio::test]
async fn a_late_payment_is_honoured_while_its_units_are_free() {
    let (chain, url) = start_chain().await;
    let (_, seller) = wallet(7);
    let ledger = lapsed_ledger(100, 10, &seller);
    let edge = start("usdc-late", &url, Some(&ledger)).await;
    chain.set(|c| {
        c.head = 200;
        c.receipts
            .insert(tx(6), receipt(&late_invoice(10), &seller, 100));
    });
    let (status, body) = confirm(&edge, "ord_late", &tx(6)).await;
    assert_eq!(status, 204, "{body}");
    assert_eq!(status_of(&edge, "ord_late").await, "paid");
    edge.shutdown().await;
}

#[tokio::test]
async fn a_late_payment_for_units_sold_since_is_refused() {
    let (chain, url) = start_chain().await;
    let (_, seller) = wallet(7);
    let mut ledger = lapsed_ledger(10, 10, &seller);
    // Someone else bought the same ten units after the hold ran out.
    let now = losos_registrar::market::now_secs();
    ledger["orders"]["ord_other"] = json!({
        "id": "ord_other", "listing_id": "lst_1", "buyer": "third-box",
        "seller": "seller-box", "kind": "storage", "quantity": 10,
        "unit_price": 5, "amount": 50, "fee": 0, "currency": "usd",
        "status": "paid", "session_id": null, "created_at": now - 600,
        "paid_at": now - 590, "expires_at": now + 86_400, "rail": "usdc",
        "tx_hash": tx(9),
    });
    let edge = start("usdc-late-sold", &url, Some(&ledger)).await;
    chain.set(|c| {
        c.head = 200;
        c.receipts
            .insert(tx(7), receipt(&late_invoice(10), &seller, 100));
    });
    let (status, body) = confirm(&edge, "ord_late", &tx(7)).await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("units are sold"), "{body}");
    edge.shutdown().await;
}
