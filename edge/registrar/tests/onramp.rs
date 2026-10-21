//! Integration tests for buying USDC at Binance (`src/onramp.rs`).
//!
//! The real routes run on a real socket. Binance is a stand-in on loopback
//! that checks every call the way Binance documents it: the `X-Tesla-*`
//! headers, and `SHA256withRSA(body + timestamp)` against the partner's
//! public key. Its webhooks are signed with a second key, the one the edge
//! is told is Binance's. What is asserted: the pre-order the edge signs and
//! what it asks for, the link it hands back and the ones it refuses, whose
//! orders a box can read, and which webhooks move an order.

mod common;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::post;
use axum::{Json, Router};
use common::{Edge, TenantSpec, GOOD_TOKEN, OTHER_TOKEN};
use losos_registrar::onramp::{OnrampOpts, ORDER, PRE_ORDER};
use ring::rand::SystemRandom;
use ring::signature::{self, KeyPair, RsaKeyPair};
use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey};
use serde_json::{json, Value};

const CLIENT_ID: &str = "losos-test-partner";
const ACCESS_TOKEN: &str = "test-sign-access-token";
const WALLET: &str = "0xF39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
const TX: &str = "0x5c504ed432cb51138bcf09aa5e8a410dd4a1e204ef84bfed1be16dfba1b22060";

fn b64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            out.push(if i <= c.len() {
                T[((n >> (18 - 6 * i)) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

fn b64_decode(text: &str) -> Vec<u8> {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let (mut acc, mut bits, mut out) = (0u32, 0, Vec::new());
    for c in text.trim_end_matches('=').bytes() {
        acc = (acc << 6) | T.iter().position(|t| *t == c).expect("base64") as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

/// An RSA key as an operator makes one: PKCS#8 DER, and the public half as
/// `SubjectPublicKeyInfo` DER.
fn keypair() -> (Vec<u8>, Vec<u8>) {
    let key = rsa::RsaPrivateKey::new(&mut rand_core::OsRng, 2048).expect("rsa key");
    let pkcs8 = key.to_pkcs8_der().expect("pkcs8").as_bytes().to_vec();
    let spki = key
        .to_public_key()
        .to_public_key_der()
        .expect("spki")
        .as_bytes()
        .to_vec();
    (pkcs8, spki)
}

fn sign(pkcs8: &[u8], message: &[u8]) -> String {
    let pair = RsaKeyPair::from_pkcs8(pkcs8).expect("ring reads the key");
    let mut sig = vec![0; pair.public().modulus_len()];
    pair.sign(
        &signature::RSA_PKCS1_SHA256,
        &SystemRandom::new(),
        message,
        &mut sig,
    )
    .expect("sign");
    b64(&sig)
}

/// What the stand-in says when asked.
#[derive(Default)]
struct Binance {
    /// The partner's public key (PKCS#1), which every call must verify with.
    partner: Vec<u8>,
    /// Every verified call: path and body.
    calls: Vec<(String, Value)>,
    /// Status per order, as `ORDER` answers it.
    orders: BTreeMap<String, Value>,
    /// The next pre-order's link, when not Binance's own.
    link: Option<String>,
    /// The next pre-order refused with this message.
    refuse: Option<String>,
}

type Shared = Arc<Mutex<Binance>>;

async fn answer(
    State(b): State<Shared>,
    path: String,
    headers: HeaderMap,
    body: Bytes,
) -> Json<Value> {
    let h = |n: &str| {
        headers
            .get(n)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string()
    };
    let mut b = b.lock().unwrap();
    let mut signed = body.to_vec();
    signed.extend_from_slice(h("X-Tesla-Timestamp").as_bytes());
    let verified = h("X-Tesla-ClientId") == CLIENT_ID
        && h("X-Tesla-SignAccessToken") == ACCESS_TOKEN
        && h("Content-Type") == "application/json"
        && signature::UnparsedPublicKey::new(&signature::RSA_PKCS1_2048_8192_SHA256, &b.partner)
            .verify(&signed, &b64_decode(&h("X-Tesla-Signature")))
            .is_ok();
    if !verified {
        return Json(json!({"success": false, "code": "000002", "message": "Invalid signature"}));
    }
    let doc: Value = serde_json::from_slice(&body).expect("json body");
    b.calls.push((path.clone(), doc.clone()));
    let id = doc["externalOrderId"].as_str().unwrap_or("").to_string();
    if path == PRE_ORDER {
        if let Some(message) = b.refuse.take() {
            return Json(json!({"success": false, "code": "300001", "message": message}));
        }
        b.orders
            .insert(id.clone(), json!({"externalOrderId": id, "status": 0}));
        let link = b.link.take().unwrap_or_else(|| {
            format!("https://app.binance.com/uni-qr/qccnt?merchantCode=losos&externalOrderId={id}")
        });
        return Json(json!({
            "success": true, "code": "000000", "message": "success",
            "data": {"link": link, "linkExpireTime": 1_900_000_000_000_u64},
        }));
    }
    match b.orders.get(&id) {
        Some(order) => Json(json!({"success": true, "code": "000000", "data": order})),
        None => Json(json!({"success": false, "code": "300404", "message": "Order not found"})),
    }
}

struct Fixture {
    edge: Edge,
    binance: Shared,
    /// Binance's own key, which signs webhooks.
    binance_key: Vec<u8>,
}

async fn start(tag: &str) -> Fixture {
    let (partner_pkcs8, _) = keypair();
    let (binance_key, binance_spki) = keypair();
    let partner = RsaKeyPair::from_pkcs8(&partner_pkcs8)
        .expect("ring reads the key")
        .public_key()
        .as_ref()
        .to_vec();
    let binance = Arc::new(Mutex::new(Binance {
        partner,
        ..Binance::default()
    }));
    let app = Router::new()
        .route(
            PRE_ORDER,
            post(|s, h, b| answer(s, PRE_ORDER.to_string(), h, b)),
        )
        .route(ORDER, post(|s, h, b| answer(s, ORDER.to_string(), h, b)))
        .with_state(Arc::clone(&binance));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let api_url = format!("http://{}", listener.local_addr().expect("addr"));
    tokio::spawn(async move { axum::serve(listener, app).await });

    let tenants = [
        TenantSpec::new("mattbox", "mattbox.example", GOOD_TOKEN),
        TenantSpec::new("otherbox", "otherbox.example", OTHER_TOKEN),
    ];
    // The key the way a sealed credential holds it: PEM, with line breaks.
    let pem = format!(
        "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
        b64(&partner_pkcs8)
            .as_bytes()
            .chunks(64)
            .map(|c| std::str::from_utf8(c).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
    );
    let edge = Edge::start_custom(tag, &tenants, move |opts, dir| {
        std::fs::write(dir.join("binance-connect-key"), pem).unwrap();
        std::fs::write(
            dir.join("binance-connect-token"),
            format!("{ACCESS_TOKEN}\n"),
        )
        .unwrap();
        opts.onramp = Some(OnrampOpts {
            api_url,
            client_id: CLIENT_ID.to_string(),
            key_file: dir.path_str("binance-connect-key"),
            token_file: dir.path_str("binance-connect-token"),
            webhook_key: losos_registrar::onramp::parse_public_key(&b64(&binance_spki))
                .expect("spki"),
            state_file: dir.path_str("onramp.json"),
        });
    })
    .await;
    Fixture {
        edge,
        binance,
        binance_key,
    }
}

fn auth(id: &str, token: &str) -> Value {
    json!({"appliance_id": id, "token": token})
}

async fn buy(f: &Fixture, euros: &str) -> (u16, Value) {
    let mut body = auth("mattbox", GOOD_TOKEN);
    body["address"] = json!(WALLET);
    body["euros"] = json!(euros);
    let (status, text) = f.edge.post("/market/onramp", body).await;
    (status, serde_json::from_str(&text).unwrap_or(json!(text)))
}

async fn status_of(f: &Fixture, id: &str, token: &str, order: &str) -> (u16, Value) {
    let mut body = auth(id, token);
    body["order_id"] = json!(order);
    let (status, text) = f.edge.post("/market/onramp/status", body).await;
    (status, serde_json::from_str(&text).unwrap_or(json!(text)))
}

async fn webhook(f: &Fixture, client: &str, key: &[u8], doc: &Value) -> u16 {
    let body = doc.to_string();
    let ts = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis())
    .to_string();
    let sig = sign(key, format!("{body}{ts}").as_bytes());
    f.edge
        .client
        .post(format!("{}/market/onramp/webhook", f.edge.base))
        .header("Content-Type", "application/json")
        .header("X-BN-Connect-For", client)
        .header("X-BN-Connect-Timestamp", ts)
        .header("X-BN-Connect-Signature", sig)
        .body(body)
        .send()
        .await
        .expect("webhook reaches the edge")
        .status()
        .as_u16()
}

#[tokio::test]
async fn a_signed_pre_order_hands_the_box_binances_checkout() {
    let f = start("onramp-order").await;
    let (status, order) = buy(&f, "50.00").await;
    assert_eq!(status, 201, "{order}");
    let id = order["order_id"].as_str().unwrap().to_string();
    assert_eq!(id.len(), 32);
    assert!(order["link"]
        .as_str()
        .unwrap()
        .starts_with("https://app.binance.com/uni-qr/qccnt?"));
    assert_eq!(order["stage"], "open");
    assert_eq!(order["euros"], "50");
    assert_eq!(order["network"], "base");

    // What was asked of Binance: USDC on Base for euros, to the box's wallet.
    let calls = f.binance.lock().unwrap().calls.clone();
    assert_eq!(calls.len(), 1);
    let (path, body) = &calls[0];
    assert_eq!(path, PRE_ORDER);
    assert_eq!(body["externalOrderId"], id);
    assert_eq!(body["fiatCurrency"], "EUR");
    assert_eq!(body["cryptoCurrency"], "USDC");
    assert_eq!(body["network"], "BASE");
    assert_eq!(body["amountType"], 1);
    assert_eq!(body["requestedAmount"], "50");
    assert_eq!(body["address"], WALLET.to_ascii_lowercase());
    // Nothing about the buyer goes to Binance from LosOS.
    assert!(body.get("clientIp").is_none() && body.get("redirectUrl").is_none());

    // The box asks where it is: the edge asks Binance once, then reuses it.
    f.binance.lock().unwrap().orders.insert(
        id.clone(),
        json!({"externalOrderId": id, "status": 2, "cryptoAmount": "49.62"}),
    );
    let (status, view) = status_of(&f, "mattbox", GOOD_TOKEN, &id).await;
    assert_eq!(status, 200, "{view}");
    assert_eq!(view["stage"], "sending");
    assert_eq!(view["usdc"], "49.62");
    let (_, again) = status_of(&f, "mattbox", GOOD_TOKEN, &id).await;
    assert_eq!(again["stage"], "sending");
    assert_eq!(
        f.binance.lock().unwrap().calls.len(),
        2,
        "one order query, then the stored answer"
    );

    // Another box cannot read it, and a wrong token reads nothing.
    let (status, _) = status_of(&f, "otherbox", OTHER_TOKEN, &id).await;
    assert_eq!(status, 404);
    let (status, _) = status_of(&f, "mattbox", OTHER_TOKEN, &id).await;
    assert_eq!(status, 401);

    // Binance's webhook finishes it: withdrawn to the wallet, with the hash.
    let done = json!({
        "webhookEventType": "connect_order_event",
        "externalOrderId": id,
        "type": 1, "status": 20,
        "cryptoCurrency": "USDC", "cryptoAmount": "49.62",
        "withdrawWalletAddress": WALLET, "withdrawNetwork": "BASE",
        "withdrawTxHash": TX,
    });
    // Signed by anyone but Binance, or for another partner, it moves nothing.
    let (partner_like, _) = keypair();
    assert_eq!(webhook(&f, CLIENT_ID, &partner_like, &done).await, 400);
    assert_eq!(
        webhook(&f, "someone-else", &f.binance_key, &done).await,
        400
    );
    let (_, still) = status_of(&f, "mattbox", GOOD_TOKEN, &id).await;
    assert_eq!(still["stage"], "sending");

    assert_eq!(webhook(&f, CLIENT_ID, &f.binance_key, &done).await, 200);
    let (_, view) = status_of(&f, "mattbox", GOOD_TOKEN, &id).await;
    assert_eq!(view["stage"], "done");
    assert_eq!(view["tx"], TX);

    // A late event never moves a finished order back.
    let late =
        json!({"webhookEventType": "connect_order_event", "externalOrderId": id, "status": 11});
    assert_eq!(webhook(&f, CLIENT_ID, &f.binance_key, &late).await, 200);
    let (_, view) = status_of(&f, "mattbox", GOOD_TOKEN, &id).await;
    assert_eq!(view["stage"], "done");

    // The order survives a restart of the store.
    let saved: Value =
        serde_json::from_slice(&std::fs::read(f.edge.dir.join("onramp.json")).unwrap()).unwrap();
    assert_eq!(saved["orders"][&id]["status"], 20);
    f.edge.shutdown().await;
}

#[tokio::test]
async fn a_withdrawal_to_another_wallet_is_not_this_order() {
    let f = start("onramp-other-wallet").await;
    let (_, order) = buy(&f, "20").await;
    let id = order["order_id"].as_str().unwrap();
    let elsewhere = json!({
        "webhookEventType": "connect_order_event", "externalOrderId": id, "status": 20,
        "withdrawWalletAddress": "0x0000000000000000000000000000000000000001",
        "withdrawTxHash": TX,
    });
    assert_eq!(
        webhook(&f, CLIENT_ID, &f.binance_key, &elsewhere).await,
        502
    );
    let (_, view) = status_of(&f, "mattbox", GOOD_TOKEN, id).await;
    assert_ne!(view["stage"], "done");
    f.edge.shutdown().await;
}

#[tokio::test]
async fn only_binance_links_and_sane_orders_leave_the_edge() {
    let f = start("onramp-refusals").await;
    for (euros, why) in [
        ("0.50", "below one euro"),
        ("100000.01", "above the cap"),
        ("1,5", "a comma"),
    ] {
        let (status, _) = buy(&f, euros).await;
        assert_eq!(status, 400, "{why}");
    }
    let mut body = auth("mattbox", GOOD_TOKEN);
    body["address"] = json!("0x1234");
    body["euros"] = json!("50");
    assert_eq!(f.edge.post("/market/onramp", body).await.0, 400);
    assert!(
        f.binance.lock().unwrap().calls.is_empty(),
        "nothing invalid reaches Binance"
    );

    // A link anywhere but binance.com is never handed to a box.
    f.binance.lock().unwrap().link = Some("https://binance.com.evil.example/pay".into());
    assert_eq!(buy(&f, "50").await.0, 502);

    // Binance's own refusal reaches the owner in its words.
    f.binance.lock().unwrap().refuse = Some("Amount is below the minimum of 15 EUR".into());
    let (status, body) = buy(&f, "5").await;
    assert_eq!(status, 409);
    assert!(
        body.as_str()
            .unwrap()
            .contains("below the minimum of 15 EUR"),
        "{body}"
    );
    f.edge.shutdown().await;
}

#[tokio::test]
async fn an_edge_without_a_binance_partner_offers_nothing() {
    let edge = Edge::start(
        "onramp-off",
        &[TenantSpec::new("mattbox", "mattbox.example", GOOD_TOKEN)],
    )
    .await;
    let mut body = auth("mattbox", GOOD_TOKEN);
    body["address"] = json!(WALLET);
    body["euros"] = json!("50");
    assert_eq!(edge.post("/market/onramp", body).await.0, 503);
    assert_eq!(edge.post("/market/onramp/webhook", json!({})).await.0, 503);
    edge.shutdown().await;
}
