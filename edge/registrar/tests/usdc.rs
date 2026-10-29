//! Integration tests for USDC settlement.
//!
//! The real routes run on a real socket. The chain is a stand-in JSON-RPC
//! endpoint on loopback that answers the calls the registrar makes:
//! `eth_chainId`, the escrow's `usdc()`, `treasury()`, `releaser()` and
//! `deposits()`, code at an address, receipts, the block height, and what
//! sending a transaction takes. What is asserted is when an order counts as
//! paid: only for a receipt of the configured escrow, on the configured
//! chain, with the token, treasury and releaser the config names, a block
//! on top, the amounts and deadline of the order and the payout address the
//! invoice named; and which deposits the edge then releases or refunds. A
//! second stand-in answers as a Solana cluster: its genesis hash and parsed
//! transactions, for the same rules on the second family.

mod common;

use common::coins::start_stripe;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::response::IntoResponse;
use axum::{Json, Router};
use common::{Edge, KubeStub, TenantSpec, GOOD_TOKEN, OTHER_TOKEN};
use k256::ecdsa::SigningKey;
use losos_registrar::solana::base58_encode;
use losos_registrar::stablecoin::{
    network, order_id_hash_hex, payout_message, refund_deadline, UsdcChain, UsdcOpts, BASE_SEPOLIA,
    USDC_BASE_SEPOLIA, USDC_SOLANA_DEVNET,
};
use ring::signature::{Ed25519KeyPair, KeyPair};
use serde_json::{json, Value};
use sha3::{Digest, Keccak256};

const PAY: &str = "0x1111111111111111111111111111111111111111";
const TREASURY: &str = "0x4444444444444444444444444444444444444444";
const HELD_TOPIC: &str =
    "Held(uint256,bytes32,address,address,uint256,uint256,uint64,address,address)";
/// Anvil's first key, as the edge's releaser.
const RELEASER_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const RELEASER: &str = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";
/// `release(uint256,bytes[])`, `refund(uint256,bytes[])`,
/// `settle(uint256,uint256,bytes[])`.
const RELEASE: &str = "1aa4c26a";
const REFUND: &str = "1df60d97";
const SETTLE: &str = "0e0f4b0c";
/// The two boxes, each with its escrow key's wallet seed.
const BOXES: [(&str, &str, u8); 2] = [
    ("seller-box", GOOD_TOKEN, 201),
    ("buyer-box", OTHER_TOKEN, 202),
];
const DAY: u64 = 24 * 60 * 60;
const SOL_TREASURY: &str = "5ZWj7a1f8tWkjBESHKgrLmXshuXxqeY9SYcfbshpAqPG";
const SOL_BUYER: &str = "HN7cABqLq46Es1jh92dQQisAq662SmxELLLsHHe4YWrH";
/// A local validator's genesis, which the test network is configured with.
const LOCAL_GENESIS: &str = "DhAHV5FNDCT3mnxKYD4jGR9Luk9eA9TTjxHG557JT2Bm";

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
    releaser: String,
    /// `eth_getCode` by lowercase address; an account without code if absent.
    code: HashMap<String, String>,
    /// Deposit states the escrow reports; held if absent.
    deposits: HashMap<u64, u8>,
    /// Raw transactions the edge sent, in order.
    sent: Vec<String>,
    /// Solana: what `getGenesisHash` answers, and `getTransaction` results.
    genesis: String,
    transactions: HashMap<String, Value>,
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
            "0x02691bcb" => json!(word(&st.releaser)),
            data if data.starts_with("0xb02c43d0") => {
                let id = u64::from_str_radix(&data[data.len() - 16..], 16).unwrap();
                let state = st.deposits.get(&id).copied().unwrap_or(1);
                json!(format!(
                    "0x{}{}",
                    "0".repeat(64 * 6),
                    word_u(u128::from(state))
                ))
            }
            _ => Value::Null,
        },
        "eth_getCode" => json!(st
            .code
            .get(req["params"][0].as_str().unwrap_or(""))
            .cloned()
            .unwrap_or_else(|| "0x".to_string())),
        "eth_getTransactionCount" => json!(format!("0x{:x}", st.sent.len())),
        "eth_estimateGas" => json!("0x15f90"),
        "eth_getBlockByNumber" => json!({ "baseFeePerGas": "0x3b9aca00" }),
        "eth_maxPriorityFeePerGas" => json!("0xf4240"),
        "eth_sendRawTransaction" => {
            drop(st);
            let raw = req["params"][0].as_str().unwrap_or("").to_string();
            let bytes: Vec<u8> = (2..raw.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&raw[i..i + 2], 16).unwrap())
                .collect();
            chain.set(|c| c.sent.push(raw));
            return Json(json!({
                "jsonrpc": "2.0",
                "id": req["id"],
                "result": format!("0x{}", hex(&keccak(&bytes))),
            }));
        }
        "eth_getTransactionReceipt" => st
            .receipts
            .get(req["params"][0].as_str().unwrap_or(""))
            .cloned()
            .unwrap_or(Value::Null),
        "getGenesisHash" => json!(st.genesis),
        "getTransaction" => {
            // The edge must ask for what is confirmed, parsed.
            assert_eq!(req["params"][1]["commitment"], "confirmed");
            assert_eq!(req["params"][1]["encoding"], "jsonParsed");
            st.transactions
                .get(req["params"][0].as_str().unwrap_or(""))
                .cloned()
                .unwrap_or(Value::Null)
        }
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
            releaser: RELEASER.to_string(),
            code: HashMap::new(),
            deposits: HashMap::new(),
            sent: Vec::new(),
            genesis: LOCAL_GENESIS.to_string(),
            transactions: HashMap::new(),
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
fn tenants() -> Vec<TenantSpec> {
    vec![
        TenantSpec::new("seller-box", "seller.example", GOOD_TOKEN).with_market(),
        TenantSpec::new("buyer-box", "buyer.example", OTHER_TOKEN).with_market(),
    ]
}

async fn start(tag: &str, rpc_url: &str, state: Option<&Value>) -> Edge {
    start_with_cards(tag, rpc_url, state, true).await
}

/// As [`start`]; `cards: false` is an edge with no Stripe account, whose gate
/// is stopped before the first request.
async fn start_with_cards(tag: &str, rpc_url: &str, state: Option<&Value>, cards: bool) -> Edge {
    start_full(tag, rpc_url, state, cards, false, mesh("Bound").await).await
}

/// The mesh apiserver stand-in, its claims in `phase`. A storage order's
/// escrow is released only once its claim is bound, so every edge here has
/// one. Leaked: it serves until the test process ends, like the edge.
async fn mesh(phase: &str) -> &'static KubeStub {
    Box::leak(Box::new(KubeStub::start_with_claim_phase(201, phase).await))
}

/// As [`start`], also taking Bitcoin through a node that never answers:
/// enough for the edge to offer the coin and refuse it before asking.
async fn start_with_btc(tag: &str, rpc_url: &str, state: Option<&Value>) -> Edge {
    start_full(tag, rpc_url, state, true, true, mesh("Bound").await).await
}

async fn start_full(
    tag: &str,
    rpc_url: &str,
    state: Option<&Value>,
    cards: bool,
    btc: bool,
    kube: &KubeStub,
) -> Edge {
    start_opts(tag, rpc_url, state, cards, btc, kube, true, true).await
}

/// As [`start_full`]; `direct_pay: false` is the default edge, which takes
/// only payments it can hold for a slot. `boxes: false` leaves the two
/// boxes' signing to the test ([`box_pass`]).
#[allow(clippy::too_many_arguments)]
async fn start_opts(
    tag: &str,
    rpc_url: &str,
    state: Option<&Value>,
    cards: bool,
    btc: bool,
    kube: &KubeStub,
    direct_pay: bool,
    boxes: bool,
) -> Edge {
    let stripe = start_stripe().await;
    let rpc_url = rpc_url.to_string();
    let kube_api = kube.base.clone();
    let mut edge = Edge::start_market_full(
        tag,
        &tenants(),
        &stripe,
        &[("seller-box", false)],
        state,
        move |opts, dir| {
            std::fs::write(dir.join("usdc-rpc-url"), &rpc_url).expect("write rpc url");
            std::fs::write(dir.join("usdc-releaser-key"), RELEASER_KEY).expect("write key");
            std::fs::write(dir.join("kube.token"), common::KUBE_TOKEN).expect("kube token");
            opts.kube_api = kube_api;
            opts.kube_token_file = Some(dir.path_str("kube.token"));
            let market = opts.market.as_mut().expect("market on");
            market.currency = "usd".to_string();
            market.direct_pay = direct_pay;
            if !cards {
                market.cards = false;
                market.gate_socket = dir.path_str("no-gate.sock");
            }
            market.usdc = Some(UsdcOpts {
                chains: vec![
                    UsdcChain {
                        network: network("base-sepolia").unwrap(),
                        rpc_file: dir.path_str("usdc-rpc-url"),
                        pay: Some(PAY.to_string()),
                        treasury: TREASURY.to_string(),
                        genesis: None,
                    },
                    // The same stand-in, so it answers Base Sepolia's chain
                    // id: a payment reported as Arbitrum is on another chain.
                    UsdcChain {
                        network: network("arbitrum-sepolia").unwrap(),
                        rpc_file: dir.path_str("usdc-rpc-url"),
                        pay: Some(PAY.to_string()),
                        treasury: TREASURY.to_string(),
                        genesis: None,
                    },
                    UsdcChain {
                        network: network("solana-devnet").unwrap(),
                        rpc_file: dir.path_str("usdc-rpc-url"),
                        pay: None,
                        treasury: SOL_TREASURY.to_string(),
                        genesis: Some(LOCAL_GENESIS.to_string()),
                    },
                ],
                ens_rpc_file: None,
                price_sources: silent_prices(),
                releaser_file: Some(dir.path_str("usdc-releaser-key")),
            });
            if btc {
                market.btc = Some(losos_registrar::bitcoin::BtcOpts {
                    rpc: "http://127.0.0.1:9".to_string(),
                    cookie_file: dir.path_str("no-cookie"),
                    wallet: "losos-watch".to_string(),
                });
            }
        },
    )
    .await;
    if !cards {
        edge.stop_gate().await;
    }
    register_boxes(&edge.client, &edge.base).await;
    if boxes {
        let (client, base) = (edge.client.clone(), edge.base.clone());
        tokio::spawn(async move {
            loop {
                box_pass(&client, &base).await;
                tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            }
        });
    }
    edge
}

/// Each box registers its escrow key, as lososd does on its first pass.
async fn register_boxes(client: &reqwest::Client, base: &str) {
    for (id, token, seed) in BOXES {
        let (_, address) = wallet(seed);
        let r = client
            .post(format!("{base}/market/escrow/decisions"))
            .json(&with(auth(id, token), json!({ "evm": address })))
            .send()
            .await
            .expect("the edge answers");
        assert_eq!(r.status(), 200, "{}", r.text().await.unwrap_or_default());
    }
}

/// One pass of both boxes: each signs every decision the edge has waiting
/// for it, the way lososd's market loop does. Returns how many it signed.
async fn box_pass(client: &reqwest::Client, base: &str) -> usize {
    let mut signed = 0;
    for (id, token, seed) in BOXES {
        let (key, address) = wallet(seed);
        let Ok(r) = client
            .post(format!("{base}/market/escrow/decisions"))
            .json(&with(auth(id, token), json!({ "evm": address })))
            .send()
            .await
        else {
            continue;
        };
        let Ok(decisions) = r.json::<Vec<Value>>().await else {
            continue;
        };
        for d in decisions {
            let escrow =
                losos_registrar::stablecoin::address_bytes(d["escrow"].as_str().unwrap()).unwrap();
            let refund: u64 = d["refund"].as_str().unwrap().parse().unwrap();
            let deposit = d["deposit"].as_u64().unwrap();
            let digest = losos_registrar::escrow_boxes::evm_digest(
                d["chainId"].as_u64().unwrap(),
                escrow,
                deposit,
                u128::from(refund),
            );
            let (sig, recid) = key.sign_prehash_recoverable(&digest);
            let signature = format!("0x{}{:02x}", hex(&sig.to_bytes()), u8::from(recid) + 27);
            let r = client
                .post(format!("{base}/market/escrow/sign"))
                .json(&with(
                    auth(id, token),
                    json!({
                        "network": d["network"],
                        "deposit": deposit,
                        "refund": refund,
                        "signature": signature,
                    }),
                ))
                .send()
                .await;
            if r.is_ok_and(|r| r.status() == 204) {
                signed += 1;
            }
        }
    }
    signed
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

/// A mined `LososEscrow.pay` holding `invoice` for `seller` as deposit
/// `deposit`, in block `block`.
fn receipt(invoice: &Value, seller: &str, block: u64, deposit: u8) -> Value {
    let gross: u128 = invoice["gross"].as_str().unwrap().parse().unwrap();
    let fee: u128 = invoice["fee"].as_str().unwrap().parse().unwrap();
    let deadline = u128::from(invoice["deadline"].as_u64().unwrap());
    let boxes = invoice["chains"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["buyer_box"].is_string())
        .expect("an escrowed network");
    let buyer_box = boxes["buyer_box"].as_str().unwrap();
    let seller_box = boxes["seller_box"].as_str().unwrap();
    json!({
        "status": "0x1",
        "blockNumber": format!("0x{block:x}"),
        "logs": [{
            "address": PAY,
            "topics": [
                format!("0x{}", hex(&keccak(HELD_TOPIC.as_bytes()))),
                format!("0x{}", word_u(u128::from(deposit))),
                invoice["order_id_hash"],
                "0x0000000000000000000000003333333333333333333333333333333333333333",
            ],
            "data": format!(
                "0x000000000000000000000000{}{}{}{}000000000000000000000000{}000000000000000000000000{}",
                &seller[2..],
                word_u(gross),
                word_u(fee),
                word_u(deadline),
                &buyer_box[2..],
                &seller_box[2..],
            ),
        }],
    })
}

/// The escrow calls the edge has sent, as `(selector, deposit)`.
fn sent_calls(chain: &Chain) -> Vec<(String, u64)> {
    let raws = chain.inner.lock().expect("chain not poisoned").sent.clone();
    raws.iter()
        .map(|raw| {
            // The call data sits between the address and the signature;
            // nothing before it spells either selector.
            let at = raw
                .find(RELEASE)
                .or_else(|| raw.find(REFUND))
                .or_else(|| raw.find(SETTLE))
                .expect("an escrow call");
            let deposit = u64::from_str_radix(&raw[at + 8 + 48..at + 8 + 64], 16).unwrap();
            (raw[at..at + 8].to_string(), deposit)
        })
        .collect()
}

/// The edge's test-mode ledger as it stands on disk.
fn ledger(edge: &Edge) -> Value {
    let raw = std::fs::read(edge.dir.join("market-test.json")).expect("read the ledger");
    serde_json::from_slice(&raw).expect("the ledger is JSON")
}

/// Every order's clock moved back by `secs`, as if that long had passed.
fn aged(mut ledger: Value, secs: u64) -> Value {
    if let Some(orders) = ledger["orders"].as_object_mut() {
        for o in orders.values_mut() {
            for key in ["created_at", "paid_at", "expires_at"] {
                if let Some(t) = o[key].as_u64() {
                    o[key] = json!(t - secs);
                }
            }
        }
    }
    ledger
}

/// Stop `edge` and start it again on its ledger aged by `secs`: how these
/// tests let a slot run its course. The chain stand-in carries on.
async fn later(edge: Edge, tag: &str, url: &str, secs: u64, kube: &KubeStub) -> Edge {
    let ledger = aged(ledger(&edge), secs);
    edge.shutdown().await;
    start_full(tag, url, Some(&ledger), true, false, kube).await
}

/// Wait until the order's volume is bound: what was bought exists.
async fn wait_volume(edge: &Edge, order_id: &str) {
    for _ in 0..100 {
        if !purchase(edge, order_id).await["volume"].is_null() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("{order_id} never got its volume");
}

/// Give the reconciler a few passes, then say the edge sent nothing.
async fn sends_nothing(chain: &Chain) {
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    assert!(sent_calls(chain).is_empty(), "{:?}", sent_calls(chain));
}

/// Wait for the reconciler to have sent `n` escrow calls.
async fn wait_sent(chain: &Chain, n: usize) -> Vec<(String, u64)> {
    for _ in 0..100 {
        let calls = sent_calls(chain);
        if calls.len() >= n {
            return calls;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("the edge sent {:?}, not {n} calls", sent_calls(chain));
}

/// Mine every sent call one block under the head, with `status`.
fn mine_sent(chain: &Chain, status: &str) {
    chain.set(|c| {
        for raw in c.sent.clone() {
            let bytes: Vec<u8> = (2..raw.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&raw[i..i + 2], 16).unwrap())
                .collect();
            let hash = format!("0x{}", hex(&keccak(&bytes)));
            let block = c.head - 1;
            c.receipts.insert(
                hash,
                json!({ "status": status, "blockNumber": format!("0x{block:x}"), "logs": [] }),
            );
        }
    });
}

/// The buyer's view of an order.
async fn purchase(edge: &Edge, order_id: &str) -> Value {
    let (status, body) = edge
        .post("/market/account", auth("buyer-box", OTHER_TOKEN))
        .await;
    assert_eq!(status, 200, "{body}");
    parse(&body)["purchases"]
        .as_array()
        .expect("purchases")
        .iter()
        .find(|o| o["id"] == order_id)
        .cloned()
        .unwrap_or(Value::Null)
}

/// Wait until the order's escrow reaches `step`.
async fn wait_step(edge: &Edge, order_id: &str, step: &str) -> Value {
    for _ in 0..100 {
        let order = purchase(edge, order_id).await;
        if order["escrow"]["step"] == step {
            return order;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!(
        "{order_id} never reached {step}: {}",
        purchase(edge, order_id).await
    );
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

async fn confirm_on(edge: &Edge, order_id: &str, tx: &str, network: &str) -> (u16, String) {
    edge.post(
        "/market/confirm",
        with(
            auth("buyer-box", OTHER_TOKEN),
            json!({ "order_id": order_id, "tx": tx, "network": network }),
        ),
    )
    .await
}

/// A Solana wallet: its base58 address, and the payout set with it.
async fn set_solana_payout(edge: &Edge, seed: u8) -> String {
    let pair = Ed25519KeyPair::from_seed_unchecked(&[seed; 32]).expect("key");
    let address = base58_encode(pair.public_key().as_ref());
    let message = payout_message("solana-devnet", &address);
    let signature = base58_encode(pair.sign(message.as_bytes()).as_ref());
    let (status, body) = edge
        .post(
            "/market/payout",
            with(
                auth("seller-box", GOOD_TOKEN),
                json!({ "address": address, "signature": signature, "family": "solana" }),
            ),
        )
        .await;
    assert_eq!(status, 204, "{body}");
    address
}

fn token_balance(index: u64, owner: &str, amount: u128) -> Value {
    json!({
        "accountIndex": index,
        "mint": USDC_SOLANA_DEVNET,
        "owner": owner,
        "uiTokenAmount": { "amount": amount.to_string(), "decimals": 6 }
    })
}

/// A confirmed Solana transaction paying `invoice` to `seller`, the way the
/// admin page's payer builds it.
fn solana_tx(invoice: &Value, seller: &str) -> Value {
    let gross: u128 = invoice["gross"].as_str().unwrap().parse().unwrap();
    let fee: u128 = invoice["fee"].as_str().unwrap().parse().unwrap();
    json!({
        "slot": 140,
        "meta": {
            "err": null,
            "preTokenBalances": [token_balance(1, SOL_BUYER, 50_000_000)],
            "postTokenBalances": [
                token_balance(1, SOL_BUYER, 50_000_000 - gross),
                token_balance(2, seller, gross - fee),
                token_balance(3, SOL_TREASURY, fee),
            ],
        },
        "transaction": { "message": { "instructions": [
            { "program": "spl-token", "parsed": { "type": "transferChecked" } },
            { "program": "spl-token", "parsed": { "type": "transferChecked" } },
            { "program": "spl-memo", "parsed": invoice["order_id_hash"] },
        ] } },
    })
}

fn sig(n: u8) -> String {
    base58_encode(&[n; 64])
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
    // Only the EVM payout is set, so both EVM networks and not Solana.
    let chains: Vec<&str> = invoice["chains"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["network"].as_str().unwrap())
        .collect();
    assert_eq!(chains, ["base-sepolia", "arbitrum-sepolia"]);
    assert_eq!(invoice["chains"][0]["seller"], seller.as_str());
    assert_eq!(invoice["chains"][0]["usdc"], USDC_BASE_SEPOLIA);
    assert_eq!(
        invoice["order_id_hash"],
        order_id_hash_hex(&order_id).as_str()
    );
    assert!(invoice["pay_before"].as_u64().unwrap() > losos_registrar::market::now_secs());

    chain.set(|c| {
        c.receipts.insert(tx(1), receipt(&invoice, &seller, 100, 1));
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

    // The deposit is held while the slot runs: the seller sees it, cannot
    // be paid yet, and cannot stop selling what was bought.
    sends_nothing(&chain).await;
    assert_eq!(purchase(&edge, &order_id).await["escrow"]["step"], "held");
    let account = seller_account(&edge).await;
    assert_eq!(account["payouts"]["held"], 192, "{account}");
    assert_eq!(account["payouts"]["ready"], 0, "{account}");
    let listing = account["listings"][0]["id"].as_str().unwrap().to_string();
    assert!(account["listings"][0]["locked_until"].as_u64().is_some());
    let (status, body) = release(&edge).await;
    assert_eq!(status, 409, "{body}");
    let (status, body) = close(&edge, &listing).await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("slot ends"), "{body}");

    // A month on, the slot is over: the seller asks, and the edge releases
    // the deposit once and follows the call until it has a block on top.
    let kube = mesh("Bound").await;
    wait_volume(&edge, &order_id).await;
    let edge = later(edge, "usdc-paid-later", &url, 30 * DAY + 60, kube).await;
    let account = seller_account(&edge).await;
    assert_eq!(account["payouts"]["ready"], 192, "{account}");
    assert!(
        account["listings"][0].get("locked_until").is_none(),
        "{account}"
    );
    let (status, body) = release(&edge).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(wait_sent(&chain, 1).await, [(RELEASE.to_string(), 1)]);
    assert_eq!(
        purchase(&edge, &order_id).await["escrow"]["step"],
        "release"
    );
    // Asking twice pays nothing twice.
    assert_eq!(release(&edge).await.0, 409);
    mine_sent(&chain, "0x1");
    let order = wait_step(&edge, &order_id, "released").await;
    assert!(order["escrow"]["tx"].is_null(), "{order}");
    assert_eq!(sent_calls(&chain).len(), 1);
    assert_eq!(close(&edge, &listing).await.0, 204);
    edge.shutdown().await;
}

/// The edge's key is one of three: it decides, but sends nothing until the
/// box the decision pays has signed it, and that box sees only its own.
#[tokio::test]
async fn the_edge_alone_moves_no_deposit() {
    let (chain, url) = start_chain().await;
    let kube = mesh("Bound").await;
    let edge = start_opts(
        "usdc-two-of-three",
        &url,
        None,
        true,
        false,
        kube,
        true,
        false,
    )
    .await;
    let seller = set_payout(&edge, 7).await;
    let invoice = order(&edge, 40).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    let (_, seller_box) = wallet(201);
    let (_, buyer_box) = wallet(202);
    for c in invoice["chains"].as_array().unwrap() {
        assert_eq!(c["buyer_box"], buyer_box.as_str(), "{c}");
        assert_eq!(c["seller_box"], seller_box.as_str(), "{c}");
    }
    chain.set(|c| {
        c.receipts.insert(tx(1), receipt(&invoice, &seller, 100, 1));
        c.head = 101;
    });
    assert_eq!(confirm(&edge, &order_id, &tx(1)).await.0, 204);
    wait_volume(&edge, &order_id).await;
    // A month on, as `later` does it, but with boxes that sign only when
    // this test says so.
    let ledger = aged(ledger(&edge), 30 * DAY + 60);
    edge.shutdown().await;
    let edge = start_opts(
        "usdc-two-of-three-later",
        &url,
        Some(&ledger),
        true,
        false,
        kube,
        true,
        false,
    )
    .await;
    assert_eq!(release(&edge).await.0, 200);
    assert_eq!(
        purchase(&edge, &order_id).await["escrow"]["step"],
        "release"
    );
    sends_nothing(&chain).await;
    // The buyer's box is not asked to agree to the seller being paid.
    let decisions = |id: &'static str, token: &'static str, seed: u8| {
        let edge = &edge;
        async move {
            let (_, address) = wallet(seed);
            let (status, body) = edge
                .post(
                    "/market/escrow/decisions",
                    with(auth(id, token), json!({ "evm": address })),
                )
                .await;
            assert_eq!(status, 200, "{body}");
            parse(&body)
        }
    };
    assert_eq!(decisions("buyer-box", OTHER_TOKEN, 202).await, json!([]));
    let mine = decisions("seller-box", GOOD_TOKEN, 201).await;
    assert_eq!(mine.as_array().unwrap().len(), 1, "{mine}");
    assert_eq!(mine[0]["side"], "seller");
    assert_eq!(mine[0]["refund"], "0");
    assert_eq!(mine[0]["deposit"], 1);
    assert_eq!(mine[0]["orderId"], order_id.as_str());
    // A signature over another decision, or by another key, is refused.
    let (key, _) = wallet(201);
    let (stranger, _) = wallet(203);
    let sign = |key: &SigningKey, refund: u128| {
        let digest = losos_registrar::escrow_boxes::evm_digest(
            BASE_SEPOLIA,
            losos_registrar::stablecoin::address_bytes(PAY).unwrap(),
            1,
            refund,
        );
        let (sig, recid) = key.sign_prehash_recoverable(&digest);
        format!("0x{}{:02x}", hex(&sig.to_bytes()), u8::from(recid) + 27)
    };
    let post_sign = |signature: String, refund: u64| {
        let edge = &edge;
        async move {
            edge.post(
                "/market/escrow/sign",
                with(
                    auth("seller-box", GOOD_TOKEN),
                    json!({ "network": "base-sepolia", "deposit": 1, "refund": refund, "signature": signature }),
                ),
            )
            .await
        }
    };
    let (status, body) = post_sign(sign(&key, 5), 5).await;
    assert_eq!(status, 409, "{body}");
    let (status, body) = post_sign(sign(&key, 5), 0).await;
    assert_eq!(status, 400, "{body}");
    let (status, body) = post_sign(sign(&stranger, 0), 0).await;
    assert_eq!(status, 400, "{body}");
    sends_nothing(&chain).await;
    // The seller's box agrees: now it is two of three, and the call goes.
    let (status, body) = post_sign(sign(&key, 0), 0).await;
    assert_eq!(status, 204, "{body}");
    assert_eq!(wait_sent(&chain, 1).await, [(RELEASE.to_string(), 1)]);
    let raw = chain.inner.lock().unwrap().sent[0].clone();
    assert!(
        raw.contains(&sign(&key, 0)[2..]),
        "the call carries the box's signature"
    );
    edge.shutdown().await;
}

async fn release(edge: &Edge) -> (u16, String) {
    edge.post("/market/sales/release", auth("seller-box", GOOD_TOKEN))
        .await
}

async fn close(edge: &Edge, listing: &str) -> (u16, String) {
    edge.post(
        "/market/listings/close",
        with(
            auth("seller-box", GOOD_TOKEN),
            json!({ "listing_id": listing }),
        ),
    )
    .await
}

#[tokio::test]
async fn the_seller_is_paid_only_once_the_storage_exists() {
    let (chain, url) = start_chain().await;
    // The mesh takes the claim but cannot bind it yet: no volume to give.
    let kube = mesh("Pending").await;
    let edge = start_full("usdc-undelivered", &url, None, true, false, kube).await;
    let seller = set_payout(&edge, 7).await;
    let invoice = order(&edge, 40).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    chain.set(|c| {
        c.receipts.insert(tx(1), receipt(&invoice, &seller, 100, 1));
        c.head = 101;
    });
    assert_eq!(confirm(&edge, &order_id, &tx(1)).await.0, 204);
    assert_eq!(status_of(&edge, &order_id).await, "paid");

    // The claim was asked for, and the deposit stays in the escrow.
    for _ in 0..100 {
        if kube
            .seen()
            .iter()
            .any(|r| r.contains("persistentvolumeclaims/"))
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    sends_nothing(&chain).await;
    assert_eq!(purchase(&edge, &order_id).await["escrow"]["step"], "held");

    // Not a minute short of a day, and still nothing: the volume may bind.
    let edge = later(edge, "usdc-undelivered-23h", &url, 23 * 3600 - 60, kube).await;
    sends_nothing(&chain).await;
    // A day without a volume: the buyer gets it all back, and the seller
    // is never paid for it.
    let edge = later(edge, "usdc-undelivered-day", &url, 120, kube).await;
    assert_eq!(wait_sent(&chain, 1).await, [(REFUND.to_string(), 1)]);
    let order = purchase(&edge, &order_id).await;
    assert_eq!(order["escrow"]["reason"], "notDelivered", "{order}");
    assert_eq!(order["expired"], true, "{order}");
    edge.shutdown().await;
}

#[tokio::test]
async fn the_seller_is_paid_once_the_slot_with_its_storage_is_over() {
    let (chain, url) = start_chain().await;
    let kube = mesh("Pending").await;
    let edge = start_full("usdc-bound-late", &url, None, true, false, kube).await;
    let seller = set_payout(&edge, 7).await;
    let invoice = order(&edge, 40).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    chain.set(|c| {
        c.receipts.insert(tx(1), receipt(&invoice, &seller, 100, 1));
        c.head = 101;
    });
    assert_eq!(confirm(&edge, &order_id, &tx(1)).await.0, 204);
    // The volume binds within the hour; a month on, the seller is paid by
    // the edge itself a day after the slot, without asking.
    kube.set_claim_phase("Bound");
    wait_volume(&edge, &order_id).await;
    sends_nothing(&chain).await;
    let edge = later(edge, "usdc-bound-later", &url, 31 * DAY + 60, kube).await;
    assert_eq!(wait_sent(&chain, 1).await, [(RELEASE.to_string(), 1)]);
    mine_sent(&chain, "0x1");
    wait_step(&edge, &order_id, "released").await;
    edge.shutdown().await;
}

#[tokio::test]
async fn an_edge_without_a_stripe_account_takes_usdc() {
    let (chain, url) = start_chain().await;
    let edge = start_with_cards("usdc-no-cards", &url, None, false).await;
    // The mode comes from the networks, not from a Stripe key.
    let (status, body) = edge
        .post("/market/account", auth("seller-box", GOOD_TOKEN))
        .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(parse(&body)["mode"], "test", "{body}");
    // Card payouts are not offered.
    let (status, body) = edge
        .post("/market/seller/onboard", auth("seller-box", GOOD_TOKEN))
        .await;
    assert_eq!(status, 503, "{body}");

    let seller = set_payout(&edge, 7).await;
    let invoice = order(&edge, 40).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    chain.set(|c| {
        c.receipts.insert(tx(1), receipt(&invoice, &seller, 100, 1));
        c.head = 101;
    });
    let (status, body) = confirm(&edge, &order_id, &tx(1)).await;
    assert_eq!(status, 204, "{body}");
    assert_eq!(status_of(&edge, &order_id).await, "paid");
    sends_nothing(&chain).await;
    let ledger = aged(ledger(&edge), 31 * DAY + 60);
    edge.shutdown().await;
    let edge = start_with_cards("usdc-no-cards-later", &url, Some(&ledger), false).await;
    assert_eq!(wait_sent(&chain, 1).await, [(RELEASE.to_string(), 1)]);
    mine_sent(&chain, "0x1");
    wait_step(&edge, &order_id, "released").await;
    edge.shutdown().await;
}

#[tokio::test]
async fn a_release_that_reverts_is_sent_again_while_the_deposit_is_held() {
    let (chain, url) = start_chain().await;
    let edge = start("usdc-release-again", &url, None).await;
    let seller = set_payout(&edge, 7).await;
    let invoice = order(&edge, 10).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    chain.set(|c| {
        c.head = 200;
        c.receipts.insert(tx(1), receipt(&invoice, &seller, 100, 1));
    });
    assert_eq!(confirm(&edge, &order_id, &tx(1)).await.0, 204);
    wait_volume(&edge, &order_id).await;
    let edge = later(
        edge,
        "usdc-release-again-later",
        &url,
        31 * DAY + 60,
        mesh("Bound").await,
    )
    .await;
    wait_sent(&chain, 1).await;
    mine_sent(&chain, "0x0");
    let calls = wait_sent(&chain, 2).await;
    assert!(
        calls.iter().all(|c| *c == (RELEASE.to_string(), 1)),
        "{calls:?}"
    );
    // Someone mined it after all: the escrow says released, so that is that.
    chain.set(|c| {
        c.deposits.insert(1, 2);
        c.receipts.retain(|_, r| r["status"] == "0x1");
    });
    mine_sent(&chain, "0x1");
    wait_step(&edge, &order_id, "released").await;
    edge.shutdown().await;
}

#[tokio::test]
async fn a_seller_address_that_is_a_contract_gets_nothing() {
    let (chain, url) = start_chain().await;
    let edge = start("usdc-seller-contract", &url, None).await;
    let seller = set_payout(&edge, 7).await;
    let invoice = order(&edge, 10).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    chain.set(|c| {
        c.head = 200;
        c.code.insert(seller.clone(), "0x6080604052".to_string());
        c.receipts.insert(tx(2), receipt(&invoice, &seller, 100, 2));
    });
    let (status, body) = confirm(&edge, &order_id, &tx(2)).await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("cannot receive"), "{body}");
    assert!(body.contains("refunded"), "{body}");
    // The same answer again, and the deposit goes back once.
    let (status, again) = confirm(&edge, &order_id, &tx(2)).await;
    assert_eq!((status, again.as_str()), (409, body.as_str()));
    assert_eq!(wait_sent(&chain, 1).await, [(REFUND.to_string(), 2)]);
    let order = purchase(&edge, &order_id).await;
    assert_eq!(order["status"], "expired", "{order}");
    assert_eq!(order["escrow"]["reason"], "sellerCannotReceive", "{order}");
    mine_sent(&chain, "0x1");
    wait_step(&edge, &order_id, "refunded").await;
    edge.shutdown().await;
}

#[tokio::test]
async fn a_seller_account_delegated_under_eip_7702_is_paid() {
    let (chain, url) = start_chain().await;
    let edge = start("usdc-seller-7702", &url, None).await;
    let seller = set_payout(&edge, 7).await;
    let invoice = order(&edge, 10).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    chain.set(|c| {
        c.head = 200;
        c.code.insert(
            seller.clone(),
            "0xef010063c0c19a282a1b52b07dd5a65b58948a07dae32b".to_string(),
        );
        c.receipts.insert(tx(3), receipt(&invoice, &seller, 100, 3));
    });
    let (status, body) = confirm(&edge, &order_id, &tx(3)).await;
    assert_eq!(status, 204, "{body}");
    wait_volume(&edge, &order_id).await;
    let edge = later(
        edge,
        "usdc-seller-7702-later",
        &url,
        31 * DAY + 60,
        mesh("Bound").await,
    )
    .await;
    assert_eq!(wait_sent(&chain, 1).await, [(RELEASE.to_string(), 3)]);
    edge.shutdown().await;
}

#[tokio::test]
async fn a_second_payment_for_a_paid_order_goes_back() {
    let (chain, url) = start_chain().await;
    let edge = start("usdc-paid-twice", &url, None).await;
    let seller = set_payout(&edge, 7).await;
    let invoice = order(&edge, 10).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    chain.set(|c| {
        c.head = 200;
        c.receipts.insert(tx(4), receipt(&invoice, &seller, 100, 4));
        c.receipts.insert(tx(5), receipt(&invoice, &seller, 100, 5));
    });
    assert_eq!(confirm(&edge, &order_id, &tx(4)).await.0, 204);
    let (status, body) = confirm(&edge, &order_id, &tx(5)).await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("already paid"), "{body}");
    // The second goes back at once; the first is held for the slot.
    assert_eq!(wait_sent(&chain, 1).await, [(REFUND.to_string(), 5)]);
    let order = purchase(&edge, &order_id).await;
    assert_eq!(order["status"], "paid", "{order}");
    assert_eq!(order["tx"], tx(4).as_str(), "{order}");
    assert_eq!(order["escrow"]["step"], "held", "{order}");
    edge.shutdown().await;
}

#[tokio::test]
async fn a_deposit_the_buyer_took_back_ends_the_order() {
    let (chain, url) = start_chain().await;
    let edge = start("usdc-reclaimed", &url, None).await;
    let seller = set_payout(&edge, 7).await;
    let invoice = order(&edge, 10).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    chain.set(|c| {
        c.head = 200;
        c.receipts.insert(tx(6), receipt(&invoice, &seller, 100, 6));
        // The edge was down past the deadline and the buyer refunded.
        c.deposits.insert(6, 3);
    });
    assert_eq!(confirm(&edge, &order_id, &tx(6)).await.0, 204);
    // The edge finds out when it comes to pay the seller.
    wait_volume(&edge, &order_id).await;
    let edge = later(
        edge,
        "usdc-reclaimed-later",
        &url,
        31 * DAY + 60,
        mesh("Bound").await,
    )
    .await;
    let order = wait_step(&edge, &order_id, "refunded").await;
    assert_eq!(order["escrow"]["reason"], "buyerReclaimed", "{order}");
    assert_eq!(order["expired"], true, "{order}");
    assert!(sent_calls(&chain).is_empty());
    edge.shutdown().await;
}

#[tokio::test]
async fn an_escrow_another_key_releases_settles_nothing() {
    let (chain, url) = start_chain().await;
    let edge = start("usdc-wrong-releaser", &url, None).await;
    let seller = set_payout(&edge, 7).await;
    let invoice = order(&edge, 10).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    chain.set(|c| {
        c.head = 200;
        c.releaser = "0x7777777777777777777777777777777777777777".to_string();
        c.receipts.insert(tx(7), receipt(&invoice, &seller, 100, 7));
    });
    let (status, body) = confirm(&edge, &order_id, &tx(7)).await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("different releaser"), "{body}");
    assert_eq!(status_of(&edge, &order_id).await, "pending");
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
        c.receipts.insert(tx(2), receipt(&invoice, &seller, 100, 2));
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
        c.receipts.insert(tx(3), receipt(&invoice, &seller, 100, 3));
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
        c.receipts.insert(tx(4), receipt(&invoice, &first, 100, 4));
        c.receipts.insert(tx(5), receipt(&invoice, &second, 100, 5));
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
/// Both boxes' keys as an order records them.
fn order_boxes() -> Value {
    let (_, seller_box) = wallet(201);
    let (_, buyer_box) = wallet(202);
    json!({ "buyer": { "evm": buyer_box }, "seller": { "evm": seller_box } })
}

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
            "boxes": order_boxes(),
        }},
    })
}

fn late_invoice(quantity: u64, ledger: &Value) -> Value {
    let created = ledger["orders"]["ord_late"]["created_at"].as_u64().unwrap();
    json!({
        "order_id_hash": order_id_hash_hex("ord_late"),
        "gross": (u128::from(quantity) * 5 * 10_000).to_string(),
        "fee": "0",
        "deadline": refund_deadline(created + 15 * 60),
        "chains": [{
            "buyer_box": order_boxes()["buyer"]["evm"],
            "seller_box": order_boxes()["seller"]["evm"],
        }],
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
            .insert(tx(6), receipt(&late_invoice(10, &ledger), &seller, 100, 6));
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
            .insert(tx(7), receipt(&late_invoice(10, &ledger), &seller, 100, 7));
    });
    let (status, body) = confirm(&edge, "ord_late", &tx(7)).await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("units are sold"), "{body}");
    assert!(body.contains("refunded"), "{body}");
    // The money is in the escrow, so it goes back to the buyer.
    assert_eq!(wait_sent(&chain, 1).await, [(REFUND.to_string(), 7)]);
    assert_eq!(status_of(&edge, "ord_late").await, "expired");
    edge.shutdown().await;
}

#[tokio::test]
async fn a_solana_payment_counts_with_the_order_in_its_memo() {
    let (chain, url) = start_chain().await;
    let edge = start("usdc-solana", &url, None).await;
    set_payout(&edge, 7).await;
    let seller = set_solana_payout(&edge, 9).await;
    let invoice = order(&edge, 40).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    let solana = invoice["chains"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["network"] == "solana-devnet")
        .expect("Solana is offered once the seller has a Solana payout")
        .clone();
    assert_eq!(solana["family"], "solana");
    assert_eq!(solana["seller"], seller.as_str());
    assert_eq!(solana["usdc"], USDC_SOLANA_DEVNET);
    assert_eq!(solana["treasury"], SOL_TREASURY);
    assert!(solana["pay"].is_null(), "{solana}");

    // Not yet confirmed: the cluster answers null.
    let (status, body) = confirm_on(&edge, &order_id, &sig(1), "solana-devnet").await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("not on chain yet"), "{body}");

    // A payment for another order, then one that skimps on the fee.
    let mut other = invoice.clone();
    other["order_id_hash"] = json!(order_id_hash_hex("ord_other"));
    let mut short = invoice.clone();
    short["fee"] = json!("0");
    chain.set(|c| {
        c.transactions.insert(sig(2), solana_tx(&other, &seller));
        c.transactions.insert(sig(3), solana_tx(&short, &seller));
        c.transactions.insert(sig(4), solana_tx(&invoice, &seller));
    });
    let (status, body) = confirm_on(&edge, &order_id, &sig(2), "solana-devnet").await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("does not pay this order"), "{body}");
    let (status, body) = confirm_on(&edge, &order_id, &sig(3), "solana-devnet").await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("amount"), "{body}");
    assert_eq!(status_of(&edge, &order_id).await, "pending");

    // A cluster that is not the configured one counts nothing.
    chain.set(|c| c.genesis = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG".to_string());
    let (status, body) = confirm_on(&edge, &order_id, &sig(4), "solana-devnet").await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("different cluster"), "{body}");

    chain.set(|c| c.genesis = LOCAL_GENESIS.to_string());
    let (status, body) = confirm_on(&edge, &order_id, &sig(4), "solana-devnet").await;
    assert_eq!(status, 204, "{body}");
    assert_eq!(status_of(&edge, &order_id).await, "paid");
    edge.shutdown().await;
}

#[tokio::test]
async fn a_payment_reported_on_the_wrong_network_counts_nothing() {
    let (chain, url) = start_chain().await;
    let edge = start("usdc-wrong-network", &url, None).await;
    let seller = set_payout(&edge, 7).await;
    let invoice = order(&edge, 10).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    chain.set(|c| {
        c.head = 200;
        c.receipts.insert(tx(8), receipt(&invoice, &seller, 100, 8));
    });
    // Paid on Base Sepolia, reported as Arbitrum Sepolia.
    let (status, body) = confirm_on(&edge, &order_id, &tx(8), "arbitrum-sepolia").await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("different chain"), "{body}");
    // An EVM hash is not a Solana signature, and Solana was not invoiced.
    let (status, _) = confirm_on(&edge, &order_id, &tx(8), "solana-devnet").await;
    assert_eq!(status, 400);
    let (status, body) = confirm_on(&edge, &order_id, &sig(5), "solana-devnet").await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("no payout address on that network"), "{body}");
    let (status, _) = confirm_on(&edge, &order_id, &tx(8), "polygon").await;
    assert_eq!(status, 400);
    let (status, body) = confirm_on(&edge, &order_id, &tx(8), "base-sepolia").await;
    assert_eq!(status, 204, "{body}");
    edge.shutdown().await;
}

#[tokio::test]
async fn the_account_names_every_network_and_both_payouts() {
    let (_chain, url) = start_chain().await;
    let edge = start("usdc-account", &url, None).await;
    let evm = set_payout(&edge, 7).await;
    let sol = set_solana_payout(&edge, 9).await;
    let (status, body) = edge
        .post("/market/account", auth("seller-box", GOOD_TOKEN))
        .await;
    assert_eq!(status, 200, "{body}");
    let account = parse(&body);
    assert_eq!(account["payout"], evm.as_str());
    assert_eq!(account["payout_solana"], sol.as_str());
    let usdc = &account["usdc"];
    assert_eq!(usdc["test"], true);
    assert_eq!(usdc["chain_id"], BASE_SEPOLIA);
    assert_eq!(usdc["solana"], "solana-devnet");
    assert!(usdc["ens_chain_id"].is_null());
    let names: Vec<&str> = usdc["chains"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Base Sepolia", "Arbitrum Sepolia", "Solana Devnet"]);
    assert!(
        !body.contains("usdc-rpc-url"),
        "no credential path on the wire"
    );

    // A Solana signature over the live network's message sets nothing.
    let pair = Ed25519KeyPair::from_seed_unchecked(&[10; 32]).expect("key");
    let address = base58_encode(pair.public_key().as_ref());
    let signature = base58_encode(
        pair.sign(payout_message("solana", &address).as_bytes())
            .as_ref(),
    );
    let (status, _) = edge
        .post(
            "/market/payout",
            with(
                auth("seller-box", GOOD_TOKEN),
                json!({ "address": address, "signature": signature, "family": "solana" }),
            ),
        )
        .await;
    assert_eq!(status, 400);
    edge.shutdown().await;
}

#[tokio::test]
async fn the_relay_picks_the_network_and_names_need_an_endpoint() {
    let (_chain, url) = start_chain().await;
    let edge = start("usdc-relay", &url, None).await;
    let call = |network: Value, method: &str| {
        with(
            auth("buyer-box", OTHER_TOKEN),
            json!({ "network": network, "method": method, "params": [] }),
        )
    };
    let (status, body) = edge
        .post(
            "/market/rpc",
            call(json!("solana-devnet"), "getGenesisHash"),
        )
        .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(parse(&body)["result"], LOCAL_GENESIS);
    // A Solana method is not forwarded to an EVM chain, nor the reverse.
    let (status, _) = edge
        .post("/market/rpc", call(json!("base-sepolia"), "getGenesisHash"))
        .await;
    assert_eq!(status, 400);
    let (status, _) = edge
        .post("/market/rpc", call(json!("solana-devnet"), "eth_chainId"))
        .await;
    assert_eq!(status, 400);
    let (status, body) = edge
        .post("/market/rpc", call(Value::Null, "eth_chainId"))
        .await;
    assert_eq!(status, 200, "{body}");
    // No ENS endpoint was configured.
    let (status, _) = edge
        .post("/market/rpc", call(json!("ens"), "eth_chainId"))
        .await;
    assert_eq!(status, 503);
    // Nor does the offchain step of a name lookup go anywhere without one.
    let (status, _) = edge
        .post("/market/rpc", call(json!("ens"), "ens_ccipRead"))
        .await;
    assert_eq!(status, 503);
    // And it is not a payment network's method.
    let (status, _) = edge
        .post("/market/rpc", call(json!("base-sepolia"), "ens_ccipRead"))
        .await;
    assert_eq!(status, 400);
    edge.shutdown().await;
}

#[tokio::test]
async fn live_networks_open_no_market_behind_a_test_key() {
    // The real boxes pay on mainnet; an edge that lists Base while its
    // Stripe key is a test key would mix the ledgers, so it trades nothing.
    let (_chain, url) = start_chain().await;
    let stripe = start_stripe().await;
    let edge = Edge::start_market_full(
        "usdc-live-test-key",
        &tenants(),
        &stripe,
        &[("seller-box", false)],
        None,
        move |opts, dir| {
            std::fs::write(dir.join("usdc-rpc-url"), &url).expect("write rpc url");
            let market = opts.market.as_mut().expect("market on");
            market.currency = "usd".to_string();
            market.usdc = Some(UsdcOpts {
                chains: vec![UsdcChain {
                    network: network("base").unwrap(),
                    rpc_file: dir.path_str("usdc-rpc-url"),
                    pay: Some(PAY.to_string()),
                    treasury: TREASURY.to_string(),
                    genesis: None,
                }],
                ens_rpc_file: None,
                price_sources: silent_prices(),
                releaser_file: Some(dir.path_str("usdc-releaser-key")),
            });
        },
    )
    .await;
    let (status, body) = edge
        .post("/market/account", auth("buyer-box", OTHER_TOKEN))
        .await;
    assert_eq!(status, 503, "{body}");
    edge.shutdown().await;
}

// ── platform fees owed on coin sales ─────────────────────────────────────

const USDC_TOKEN_TRANSFER: &str = "Transfer(address,address,uint256)";

/// A ledger in which the seller owes `cents` on coin sales since `since`,
/// with a Bitcoin key on file.
fn owing_ledger(cents: u64, since: u64) -> Value {
    json!({
        "mode": "test",
        "coin_sellers": { "seller-box": {
            "keys": { "btc": { "descriptor": "wpkh(tpubD6NzVbkrYhZ4X/0/*)", "next": 0 } },
            "fees_owed": cents,
            "owed_since": since,
        }},
    })
}

/// The seller's own account view.
async fn seller_account(edge: &Edge) -> Value {
    let (status, body) = edge
        .post("/market/account", auth("seller-box", GOOD_TOKEN))
        .await;
    assert_eq!(status, 200, "{body}");
    parse(&body)
}

async fn owed(edge: &Edge) -> u64 {
    seller_account(edge).await["coin_fees_owed"]
        .as_u64()
        .unwrap_or(0)
}

/// Wait until the seller owes `cents`.
async fn wait_owed(edge: &Edge, cents: u64) {
    for _ in 0..100 {
        if owed(edge).await == cents {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("the seller owes {}, not {cents}", owed(edge).await);
}

/// A mined transaction moving `value` USDC base units of `token` from `from`
/// to `to`, in block `block`.
fn transfer_receipt(token: &str, from: &str, to: &str, value: u128, block: u64) -> Value {
    let word = |a: &str| format!("0x000000000000000000000000{}", &a[2..]);
    json!({
        "status": "0x1",
        "blockNumber": format!("0x{block:x}"),
        "logs": [{
            "address": token,
            "topics": [
                format!("0x{}", hex(&keccak(USDC_TOKEN_TRANSFER.as_bytes()))),
                word(from),
                word(to),
            ],
            "data": format!("0x{}", word_u(value)),
        }],
    })
}

/// Ask for an address to pay `order_id` in Bitcoin, as the buyer.
async fn pay_btc(edge: &Edge, order_id: &str) -> (u16, String) {
    edge.post(
        "/market/coins/pay",
        with(
            auth("buyer-box", OTHER_TOKEN),
            json!({ "order_id": order_id, "coin": "btc" }),
        ),
    )
    .await
}

async fn pay_fees(edge: &Edge, tx: &str, network: &str) -> (u16, String) {
    edge.post(
        "/market/fees/pay",
        with(
            auth("seller-box", GOOD_TOKEN),
            json!({ "tx": tx, "network": network }),
        ),
    )
    .await
}

#[tokio::test]
async fn a_usdc_sale_collects_coin_fees_owed_once_the_escrow_releases_it() {
    let (chain, url) = start_chain().await;
    let now = losos_registrar::market::now_secs();
    let edge = start("usdc-offset", &url, Some(&owing_ledger(50, now))).await;
    let seller = set_payout(&edge, 7).await;
    // 2 USD: the platform's 8 cents and the 50 owed go to the treasury. The
    // buyer pays the listed price all the same.
    let invoice = order(&edge, 40).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    assert_eq!(invoice["gross"], "2000000");
    assert_eq!(invoice["fee"], "580000");
    assert_eq!(invoice["fee_offset"], "500000");
    // While that sale is in flight, the next one does not take the same 50.
    let second = order(&edge, 40).await;
    assert_eq!(second["fee"], "80000");
    assert!(second.get("fee_offset").is_none(), "{second}");

    chain.set(|c| {
        c.head = 200;
        c.receipts.insert(tx(1), receipt(&invoice, &seller, 100, 1));
    });
    let (status, body) = confirm(&edge, &order_id, &tx(1)).await;
    assert_eq!(status, 204, "{body}");
    // Paid, but the treasury has nothing until the release is mined.
    assert_eq!(owed(&edge).await, 50);
    let account = seller_account(&edge).await;
    let sale = account["sales"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["id"] == order_id.as_str())
        .cloned()
        .expect("the sale");
    assert_eq!(
        (
            &sale["amount"],
            &sale["fee"],
            &sale["fee_offset"],
            &sale["seller_net"]
        ),
        (&json!(200), &json!(8), &json!(50), &json!(142)),
        "{sale}"
    );
    sends_nothing(&chain).await;
    assert_eq!(owed(&edge).await, 50);
    wait_volume(&edge, &order_id).await;
    let edge = later(
        edge,
        "usdc-offset-later",
        &url,
        31 * DAY + 60,
        mesh("Bound").await,
    )
    .await;
    assert_eq!(wait_sent(&chain, 1).await, [(RELEASE.to_string(), 1)]);
    assert_eq!(owed(&edge).await, 50);
    mine_sent(&chain, "0x1");
    wait_step(&edge, &order_id, "released").await;
    wait_owed(&edge, 0).await;
    let account = seller_account(&edge).await;
    assert!(account.get("coin_fees_since").is_none(), "{account}");
    assert!(account.get("coin_fee_credit").is_none(), "{account}");
    edge.shutdown().await;
}

#[tokio::test]
async fn a_usdc_sale_that_goes_back_to_the_buyer_collects_nothing() {
    let (chain, url) = start_chain().await;
    let now = losos_registrar::market::now_secs();
    let edge = start("usdc-offset-refund", &url, Some(&owing_ledger(50, now))).await;
    let seller = set_payout(&edge, 7).await;
    let invoice = order(&edge, 40).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    assert_eq!(invoice["fee_offset"], "500000");
    chain.set(|c| {
        c.head = 200;
        c.receipts.insert(tx(6), receipt(&invoice, &seller, 100, 6));
        // The edge was down past the deadline and the buyer took it back.
        c.deposits.insert(6, 3);
    });
    assert_eq!(confirm(&edge, &order_id, &tx(6)).await.0, 204);
    wait_volume(&edge, &order_id).await;
    let edge = later(
        edge,
        "usdc-offset-refund-later",
        &url,
        31 * DAY + 60,
        mesh("Bound").await,
    )
    .await;
    let refunded = wait_step(&edge, &order_id, "refunded").await;
    assert_eq!(refunded["escrow"]["reason"], "buyerReclaimed", "{refunded}");
    assert_eq!(owed(&edge).await, 50);
    // The 50 is free to ride on the next sale again.
    let next = order_fee_offset(&edge).await;
    assert_eq!(next, Some("500000".to_string()));

    // A seller whose address cannot receive is refunded too, and owes as before.
    chain.set(|c| {
        c.code.insert(seller.clone(), "0x6080604052".to_string());
    });
    let invoice = order(&edge, 40).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    chain.set(|c| {
        c.receipts.insert(tx(7), receipt(&invoice, &seller, 100, 7));
    });
    assert_eq!(confirm(&edge, &order_id, &tx(7)).await.0, 409);
    wait_sent(&chain, 1).await;
    mine_sent(&chain, "0x1");
    wait_step(&edge, &order_id, "refunded").await;
    assert_eq!(owed(&edge).await, 50);
    edge.shutdown().await;
}

/// The fee offset of a new order's invoice.
async fn order_fee_offset(edge: &Edge) -> Option<String> {
    order(edge, 40).await["fee_offset"]
        .as_str()
        .map(str::to_string)
}

#[tokio::test]
async fn a_solana_sale_collects_coin_fees_owed_as_it_is_confirmed() {
    let (chain, url) = start_chain().await;
    let now = losos_registrar::market::now_secs();
    let edge = start("usdc-offset-solana", &url, Some(&owing_ledger(500, now))).await;
    set_payout(&edge, 7).await;
    let seller = set_solana_payout(&edge, 9).await;
    let invoice = order(&edge, 40).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    // Half the gross at most, on Solana as in the escrow: 100 - 8.
    assert_eq!(invoice["fee"], "1000000");
    assert_eq!(invoice["fee_offset"], "920000");
    // A transfer that splits off only the platform's 4% does not pay it.
    let mut short = invoice.clone();
    short["fee"] = json!("80000");
    chain.set(|c| {
        c.transactions.insert(sig(3), solana_tx(&short, &seller));
        c.transactions.insert(sig(4), solana_tx(&invoice, &seller));
    });
    let (status, body) = confirm_on(&edge, &order_id, &sig(3), "solana-devnet").await;
    assert_eq!(status, 409, "{body}");
    assert_eq!(owed(&edge).await, 500);
    let (status, body) = confirm_on(&edge, &order_id, &sig(4), "solana-devnet").await;
    assert_eq!(status, 204, "{body}");
    assert_eq!(owed(&edge).await, 408);
    edge.shutdown().await;
}

#[tokio::test]
async fn coin_fees_paid_to_the_treasury_count_once_and_only_from_the_seller() {
    let (chain, url) = start_chain().await;
    let now = losos_registrar::market::now_secs();
    let edge = start("usdc-fees-paid", &url, Some(&owing_ledger(300, now))).await;
    let seller = set_payout(&edge, 7).await;
    let other = "0x6666666666666666666666666666666666666666";
    chain.set(|c| {
        c.head = 100;
        c.receipts.insert(
            tx(20),
            transfer_receipt(USDC_BASE_SEPOLIA, &seller, TREASURY, 2_000_000, 100),
        );
        // Another token, another recipient, another sender.
        c.receipts.insert(
            tx(21),
            transfer_receipt(other, &seller, TREASURY, 3_000_000, 90),
        );
        c.receipts.insert(
            tx(22),
            transfer_receipt(USDC_BASE_SEPOLIA, &seller, other, 3_000_000, 90),
        );
        c.receipts.insert(
            tx(23),
            transfer_receipt(USDC_BASE_SEPOLIA, other, TREASURY, 3_000_000, 90),
        );
        c.receipts.insert(
            tx(24),
            transfer_receipt(USDC_BASE_SEPOLIA, &seller, TREASURY, 5_000_000, 90),
        );
    });
    // One block is not enough.
    let (status, body) = pay_fees(&edge, &tx(20), "base-sepolia").await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("another block"), "{body}");
    assert_eq!(owed(&edge).await, 300);

    chain.set(|c| c.head = 101);
    let (status, body) = pay_fees(&edge, &tx(20), "base-sepolia").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        parse(&body),
        json!({ "paid": 200, "credited": 200, "owed": 100 })
    );
    // Counted once.
    let (status, body) = pay_fees(&edge, &tx(20), "base-sepolia").await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("already counted"), "{body}");
    assert_eq!(owed(&edge).await, 100);

    for (n, why) in [
        (21, "moves no USDC"),
        (22, "does not pay the edge's treasury"),
        (23, "did not come from the seller's payout address"),
    ] {
        let (status, body) = pay_fees(&edge, &tx(n), "base-sepolia").await;
        assert_eq!(status, 409, "{body}");
        assert!(body.contains(why), "{n}: {body}");
    }
    // On another chain than the endpoint's, nothing counts; Solana is not
    // where fees are paid.
    let (status, body) = pay_fees(&edge, &tx(24), "arbitrum-sepolia").await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("different chain"), "{body}");
    let (status, _) = pay_fees(&edge, &tx(24), "solana-devnet").await;
    assert_eq!(status, 400);
    assert_eq!(owed(&edge).await, 100);

    // More than owed: only what was owed counts, and nothing is left over.
    let (status, body) = pay_fees(&edge, &tx(24), "base-sepolia").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        parse(&body),
        json!({ "paid": 500, "credited": 100, "owed": 0 })
    );
    let account = seller_account(&edge).await;
    assert!(account.get("coin_fees_owed").is_none(), "{account}");
    assert!(account.get("coin_fee_credit").is_none(), "{account}");
    // With nothing owed there is nothing to pay.
    let (status, body) = pay_fees(&edge, &tx(25), "base-sepolia").await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("no platform fees are owed"), "{body}");
    edge.shutdown().await;
}

#[tokio::test]
async fn coin_payments_stop_while_fees_are_overdue_and_resume_once_paid() {
    let (chain, url) = start_chain().await;
    let now = losos_registrar::market::now_secs();
    // US$15 owed, over the US$10 limit.
    let edge = start_with_btc("usdc-fees-enforced", &url, Some(&owing_ledger(1_500, now))).await;
    let seller = set_payout(&edge, 7).await;
    let account = seller_account(&edge).await;
    assert_eq!(account["coin_sales_paused"], true, "{account}");
    assert_eq!(account["coin_fee_limit"], 1_000, "{account}");
    assert_eq!(account["coin_fees_since"], now, "{account}");

    // The invoice offers no coin, and asking for one says why.
    let invoice = order(&edge, 40).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    assert!(invoice.get("coins").is_none(), "{invoice}");
    let (status, body) = pay_btc(&edge, &order_id).await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("owes platform fees"), "{body}");

    // Paying the fees lifts it: the edge goes on to quote, and only the
    // silent price sources stop it.
    chain.set(|c| {
        c.head = 200;
        c.receipts.insert(
            tx(30),
            transfer_receipt(USDC_BASE_SEPOLIA, &seller, TREASURY, 15_000_000, 100),
        );
    });
    let (status, body) = pay_fees(&edge, &tx(30), "base-sepolia").await;
    assert_eq!(status, 200, "{body}");
    let account = seller_account(&edge).await;
    assert!(account.get("coin_sales_paused").is_none(), "{account}");
    let invoice = order(&edge, 40).await;
    assert_eq!(invoice["coins"], json!(["btc"]), "{invoice}");
    let (status, body) = pay_btc(&edge, invoice["order_id"].as_str().unwrap()).await;
    assert_eq!(status, 409, "{body}");
    assert!(!body.contains("owes platform fees"), "{body}");
    assert!(body.contains("price sources"), "{body}");
    edge.shutdown().await;
}

#[tokio::test]
async fn a_small_debt_stops_coin_payments_after_thirty_days() {
    let (_chain, url) = start_chain().await;
    let now = losos_registrar::market::now_secs();
    let day = 24 * 3600;
    let edge = start_with_btc(
        "usdc-fees-old",
        &url,
        Some(&owing_ledger(100, now - 31 * day)),
    )
    .await;
    set_payout(&edge, 7).await;
    assert_eq!(seller_account(&edge).await["coin_sales_paused"], true);
    edge.shutdown().await;

    let (_chain, url) = start_chain().await;
    let edge = start_with_btc(
        "usdc-fees-recent",
        &url,
        Some(&owing_ledger(100, now - 29 * day)),
    )
    .await;
    set_payout(&edge, 7).await;
    let account = seller_account(&edge).await;
    assert!(account.get("coin_sales_paused").is_none(), "{account}");
    assert_eq!(account["coin_fees_owed"], 100, "{account}");
    edge.shutdown().await;
}

/// Price sources that never answer: these tests need no ETH or SOL price.
fn silent_prices() -> [losos_registrar::coins::PriceSource; 2] {
    use losos_registrar::coins::PriceSource;
    [
        PriceSource::Kraken {
            base: "http://127.0.0.1:9".to_string(),
        },
        PriceSource::CoinGecko {
            base: "http://127.0.0.1:9".to_string(),
        },
    ]
}

/// `ledger` with the seller's box seen offline for `streak` seconds in a
/// row and `offline` in all, the last check `ago` seconds back.
fn with_presence(mut ledger: Value, streak: u64, offline: u64, ago: u64) -> Value {
    let now = losos_registrar::market::now_secs();
    ledger["presence"] = json!({ "seller-box": { "streak": streak, "offline": offline } });
    ledger["presence_tick"] = json!(now - ago);
    ledger
}

/// A paid and delivered 40 GiB slot, ten days into its month.
async fn ten_days_in(tag: &str) -> (Chain, String, Edge, String) {
    let (chain, url) = start_chain().await;
    let edge = start(tag, &url, None).await;
    let seller = set_payout(&edge, 7).await;
    let invoice = order(&edge, 40).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    chain.set(|c| {
        c.receipts.insert(tx(1), receipt(&invoice, &seller, 100, 1));
        c.head = 101;
    });
    assert_eq!(confirm(&edge, &order_id, &tx(1)).await.0, 204);
    wait_volume(&edge, &order_id).await;
    (chain, url, edge, order_id)
}

/// The refund a sent `settle` carries, in USDC base units.
fn settled_refund(chain: &Chain) -> u128 {
    let raw = chain.inner.lock().expect("chain not poisoned").sent[0].clone();
    let at = raw.find(SETTLE).expect("a settle call");
    u128::from_str_radix(&raw[at + 8 + 64..at + 8 + 128], 16).unwrap()
}

#[tokio::test]
async fn a_box_offline_for_a_day_refunds_what_it_did_not_deliver() {
    let (chain, url, edge, order_id) = ten_days_in("usdc-offline").await;
    // The seller's node drops out of Ready a few minutes short of a day
    // offline in a row; the next pass the edge watches makes it a day.
    let kube = mesh("Bound").await;
    kube.set_node("seller-box", false, false);
    let ledger = with_presence(aged(ledger(&edge), 10 * DAY), DAY - 100, DAY - 100, 200);
    edge.shutdown().await;
    let edge = start_full("usdc-offline-later", &url, Some(&ledger), true, false, kube).await;
    // Twenty days still to come and the day it was down (a little over, by
    // the last pass): just over 21 of 30 days of the 2 USD go back to the
    // buyer, rounded up to the cent; the seller keeps the rest.
    let calls = wait_sent(&chain, 1).await;
    assert_eq!(calls, [(SETTLE.to_string(), 1)]);
    assert_eq!(settled_refund(&chain), 1_410_000);
    let order = purchase(&edge, &order_id).await;
    assert_eq!(order["escrow"]["step"], "split", "{order}");
    assert_eq!(order["escrow"]["reason"], "sellerOffline", "{order}");
    assert_eq!(order["escrow"]["refund"], 141, "{order}");
    assert_eq!(order["expired"], true, "{order}");
    // The slot is over, so the seller may stop selling it now.
    let account = seller_account(&edge).await;
    assert!(
        account["listings"][0].get("locked_until").is_none(),
        "{account}"
    );
    mine_sent(&chain, "0x1");
    chain.set(|c| {
        c.deposits.insert(1, 4);
    });
    wait_step(&edge, &order_id, "settled").await;
    assert_eq!(sent_calls(&chain).len(), 1);
    edge.shutdown().await;
}

#[tokio::test]
async fn a_box_that_comes_back_in_time_keeps_its_buyer() {
    let (chain, url, edge, order_id) = ten_days_in("usdc-back").await;
    // Almost a day down, but the node is Ready again at the next pass.
    let kube = mesh("Bound").await;
    let ledger = with_presence(aged(ledger(&edge), 10 * DAY), DAY - 100, DAY - 100, 200);
    edge.shutdown().await;
    let edge = start_full("usdc-back-later", &url, Some(&ledger), true, false, kube).await;
    sends_nothing(&chain).await;
    assert_eq!(purchase(&edge, &order_id).await["escrow"]["step"], "held");
    // The count in a row starts over; what it was down in all stays.
    let raw = std::fs::read(edge.dir.join("market-test.json")).unwrap();
    let saved: Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(saved["presence"]["seller-box"]["streak"], 0, "{saved}");
    assert_eq!(
        saved["presence"]["seller-box"]["offline"],
        DAY - 100,
        "{saved}"
    );
    edge.shutdown().await;
}

#[tokio::test]
async fn a_box_down_a_minute_a_day_is_refunded_after_two_days_in_all() {
    let (chain, url, edge, _) = ten_days_in("usdc-blink").await;
    // Never a day in a row, but two days in all since the purchase.
    let kube = mesh("Bound").await;
    kube.set_node("seller-box", false, true);
    let ledger = with_presence(aged(ledger(&edge), 10 * DAY), 3_600, 2 * DAY - 100, 200);
    edge.shutdown().await;
    let edge = start_full("usdc-blink-later", &url, Some(&ledger), true, false, kube).await;
    assert_eq!(wait_sent(&chain, 1).await, [(SETTLE.to_string(), 1)]);
    // Twenty days to come and two down: 22 of 30, rounded up to the cent.
    assert_eq!(settled_refund(&chain), 1_470_000);
    edge.shutdown().await;
}

#[tokio::test]
async fn an_edge_that_was_down_counts_none_of_it_against_the_seller() {
    let (chain, url, edge, order_id) = ten_days_in("usdc-edge-down").await;
    // The node is not Ready, but the edge's last look was two hours ago:
    // it was down itself, and cannot say the box was.
    let kube = mesh("Bound").await;
    kube.set_node("seller-box", false, false);
    let ledger = with_presence(aged(ledger(&edge), 10 * DAY), DAY - 100, DAY - 100, 7_200);
    edge.shutdown().await;
    let edge = start_full(
        "usdc-edge-down-later",
        &url,
        Some(&ledger),
        true,
        false,
        kube,
    )
    .await;
    sends_nothing(&chain).await;
    assert_eq!(purchase(&edge, &order_id).await["escrow"]["step"], "held");
    edge.shutdown().await;
}

#[tokio::test]
async fn an_edge_takes_only_payments_it_can_hold_unless_told_otherwise() {
    let (_chain, url) = start_chain().await;
    let edge = start_opts(
        "usdc-held-only",
        &url,
        None,
        true,
        true,
        mesh("Bound").await,
        false,
        true,
    )
    .await;
    // A seller with only a Solana payout cannot be paid in a way the edge
    // could refund: not on the shelf, and no order can be made.
    set_solana_payout(&edge, 9).await;
    let account = seller_account(&edge).await;
    assert_eq!(account["seller_ready"], false, "{account}");
    assert_eq!(account["direct_pay"], false, "{account}");
    assert!(account.get("coins").is_none(), "{account}");
    // Nor is Solana offered as a payout to set up.
    let networks: Vec<&str> = account["usdc"]["chains"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["network"].as_str().unwrap())
        .collect();
    assert_eq!(networks, ["base-sepolia", "arbitrum-sepolia"], "{account}");
    assert!(account["usdc"]["solana"].is_null(), "{account}");
    // With an EVM payout too, the checkout names only the EVM networks and
    // no coin, though the edge runs a Bitcoin node.
    set_payout(&edge, 7).await;
    let invoice = order(&edge, 10).await;
    let chains: Vec<&str> = invoice["chains"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["network"].as_str().unwrap())
        .collect();
    assert_eq!(chains, ["base-sepolia", "arbitrum-sepolia"], "{invoice}");
    assert!(
        invoice
            .get("coins")
            .is_none_or(|c| c.as_array().is_some_and(Vec::is_empty)),
        "{invoice}"
    );
    let (status, body) = edge
        .post(
            "/market/coins/pay",
            with(
                auth("buyer-box", OTHER_TOKEN),
                json!({ "order_id": invoice["order_id"], "coin": "btc" }),
            ),
        )
        .await;
    assert_eq!(status, 400, "{body}");
    edge.shutdown().await;
}
