//! Bitcoin orders against a real `bitcoind` on regtest.
//!
//! The seller's wallet lives on the node too, but the edge only ever sees its
//! public descriptor; the buyer's wallet pays the address the edge derived.
//! Runs when `LOSOS_TEST_BITCOIND` names a `bitcoind` binary, and passes
//! without doing anything otherwise:
//!
//! ```text
//! LOSOS_TEST_BITCOIND=$(command -v bitcoind) cargo test --test bitcoin
//! ```

mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use axum::{extract::Query, routing::get, Json, Router};
use common::{Edge, TenantSpec, GOOD_TOKEN, OTHER_TOKEN};
use losos_registrar::bitcoin::BtcOpts;
use losos_registrar::coins::PriceSource;
use losos_registrar::stablecoin::{network, UsdcChain, UsdcOpts};
use serde_json::{json, Value};

/// A regtest node in a temporary directory, stopped when dropped.
struct Bitcoind {
    child: tokio::process::Child,
    rpc: String,
    cookie: PathBuf,
    _dir: common::TempDir,
}

impl Drop for Bitcoind {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port()
}

impl Bitcoind {
    async fn start(binary: &Path) -> Bitcoind {
        let dir = common::TempDir::new("bitcoind");
        let rpc_port = free_port();
        let child = tokio::process::Command::new(binary)
            .arg("-regtest")
            .arg(format!("-datadir={}", dir.path_str("")))
            .arg(format!("-rpcport={rpc_port}"))
            .arg(format!("-port={}", free_port()))
            .arg("-listen=0")
            .arg("-fallbackfee=0.0002")
            .arg("-server")
            .kill_on_drop(true)
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("start bitcoind");
        let node = Bitcoind {
            child,
            rpc: format!("http://127.0.0.1:{rpc_port}"),
            cookie: dir.join("regtest/.cookie"),
            _dir: dir,
        };
        for _ in 0..100 {
            if node.cookie.exists() && node.call("getblockchaininfo", json!([])).await.is_ok() {
                return node;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        panic!("bitcoind did not come up");
    }

    async fn call_at(&self, path: &str, method: &str, params: Value) -> Result<Value, Value> {
        let cookie = std::fs::read_to_string(&self.cookie).map_err(|_| Value::Null)?;
        let (user, pass) = cookie.trim().split_once(':').expect("cookie");
        let reply: Value = reqwest::Client::new()
            .post(format!("{}{path}", self.rpc))
            .basic_auth(user, Some(pass))
            .json(&json!({"jsonrpc": "1.0", "id": "t", "method": method, "params": params}))
            .send()
            .await
            .map_err(|_| Value::Null)?
            .json()
            .await
            .map_err(|_| Value::Null)?;
        match reply.get("error") {
            Some(e) if !e.is_null() => Err(e.clone()),
            _ => Ok(reply["result"].clone()),
        }
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value, Value> {
        self.call_at("", method, params).await
    }

    async fn wallet(&self, wallet: &str, method: &str, params: Value) -> Value {
        self.call_at(&format!("/wallet/{wallet}"), method, params)
            .await
            .unwrap_or_else(|e| panic!("{wallet} {method}: {e}"))
    }

    async fn mine(&self, blocks: u64) {
        let to = self.wallet("buyer", "getnewaddress", json!([])).await;
        self.wallet("buyer", "generatetoaddress", json!([blocks, to]))
            .await;
        self.settle().await;
    }

    /// Pay from the buyer's wallet.
    async fn send(&self, address: &str, btc: f64) {
        self.wallet("buyer", "sendtoaddress", json!([address, btc]))
            .await;
        self.settle().await;
    }

    /// Wallets hear of a transaction or a block a moment after the call that
    /// made it returns; wait until they have, as Core's own tests do.
    async fn settle(&self) {
        self.call("syncwithvalidationinterfacequeue", json!([]))
            .await
            .expect("sync");
    }
}

/// Kraken and CoinGecko both saying `usd` dollars for every coin.
async fn prices(usd: &'static str) -> [PriceSource; 2] {
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

async fn start_edge(node: &Bitcoind, prices: [PriceSource; 2]) -> Edge {
    let stripe = start_stripe().await;
    let btc = BtcOpts {
        rpc: node.rpc.clone(),
        cookie_file: node.cookie.to_string_lossy().into_owned(),
        wallet: "losos-watch".to_string(),
    };
    Edge::start_market_full(
        "bitcoin",
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
            market.btc = Some(btc);
        },
    )
    .await
}

fn auth(id: &str, token: &str, extra: Value) -> Value {
    let mut doc = json!({ "appliance_id": id, "token": token });
    for (k, v) in extra.as_object().expect("object") {
        doc[k] = v.clone();
    }
    doc
}

fn parse(body: &str) -> Value {
    serde_json::from_str(body).unwrap_or_else(|_| panic!("not JSON: {body}"))
}

async fn pay(edge: &Edge, order_id: &str) -> Value {
    let (status, body) = edge
        .post(
            "/market/coins/pay",
            auth(
                "buyer-box",
                OTHER_TOKEN,
                json!({"order_id": order_id, "coin": "btc"}),
            ),
        )
        .await;
    assert_eq!(status, 200, "{body}");
    parse(&body)
}

async fn order(edge: &Edge, listing: &str) -> Value {
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

#[tokio::test]
async fn a_bitcoin_order_is_paid_to_the_sellers_own_key() {
    let Some(binary) = std::env::var_os("LOSOS_TEST_BITCOIND") else {
        eprintln!("LOSOS_TEST_BITCOIND is not set; skipping");
        return;
    };
    let node = Bitcoind::start(Path::new(&binary)).await;
    for wallet in ["seller", "buyer"] {
        node.call("createwallet", json!([wallet]))
            .await
            .expect("wallet");
    }
    node.mine(101).await;
    // The seller's receive descriptor, public half only.
    let listed = node.wallet("seller", "listdescriptors", json!([])).await;
    let descriptor = listed["descriptors"]
        .as_array()
        .expect("descriptors")
        .iter()
        .find(|d| d["internal"] == false && d["desc"].as_str().unwrap().starts_with("wpkh("))
        .expect("a wpkh receive descriptor")["desc"]
        .as_str()
        .unwrap()
        .to_string();

    let edge = start_edge(&node, prices("60000.00").await).await;

    // A mainnet key is refused on a regtest node; a private one never sent.
    let xpub = "xpub6CatWdiZiodmUeTDp8LT5or8nmbKNcuyvz7WyksVFkKB4RHwCD3XyuvPEbvqAQY3rAPshWcMLoP2fMFMKHPJ4ZeZXYVUhLv1VMrjPC7PW6V";
    for bad in [xpub, "tprvNotARealKeyOnlyTheShapeOfOneNotARealKeyOnlyTheShapeOfOneNotARealKeyOnlyTheShapeOfOneNotARealKeyOnlyTheShapeOfOne"] {
        let (status, body) = edge
            .post(
                "/market/coins/key",
                auth("seller-box", GOOD_TOKEN, json!({"coin": "btc", "key": bad})),
            )
            .await;
        assert_eq!(status, 400, "{body}");
    }
    let (status, body) = edge
        .post(
            "/market/coins/key",
            auth(
                "seller-box",
                GOOD_TOKEN,
                json!({"coin": "btc", "key": descriptor}),
            ),
        )
        .await;
    assert_eq!(status, 204, "{body}");

    // A seller with only a Bitcoin key can list; the checkout offers BTC.
    let (status, body) = edge
        .post(
            "/market/listings",
            auth(
                "seller-box",
                GOOD_TOKEN,
                json!({"kind": "storage", "unit_price": 5, "capacity": 300}),
            ),
        )
        .await;
    assert_eq!(status, 201, "{body}");
    let listing = parse(&body)["listing_id"].as_str().unwrap().to_string();
    let checkout = order(&edge, &listing).await;
    assert_eq!(
        checkout["stablecoin"]["coins"],
        json!(["btc"]),
        "{checkout}"
    );
    let order_id = checkout["order_id"].as_str().unwrap().to_string();

    // $5.00 at $60,000 is 8,333.3 satoshis, rounded up for the seller.
    let invoice = pay(&edge, &order_id).await;
    assert_eq!(invoice["amount"], "0.00008334", "{invoice}");
    assert_eq!(invoice["state"], "waiting");
    assert_eq!(invoice["required"], 1);
    let address = invoice["address"].as_str().unwrap().to_string();
    assert!(address.starts_with("bcrt1q"), "{address}");
    assert_eq!(
        invoice["uri"],
        format!("bitcoin:{address}?amount=0.00008334&label={order_id}")
    );
    // The address is the seller's own: their wallet knows it.
    let mine = node
        .wallet("seller", "getaddressinfo", json!([address]))
        .await;
    assert_eq!(mine["ismine"], true, "{mine}");
    // Asked again, the same address.
    assert_eq!(pay(&edge, &order_id).await["address"], address);

    // Two payments add up; the first alone is short.
    node.send(&address, 0.00004).await;
    let short = pay(&edge, &order_id).await;
    assert_eq!(short["state"], "underpaid", "{short}");
    assert_eq!(short["due"], "0.00008334");
    node.send(&address, 0.00004334).await;
    let seen = pay(&edge, &order_id).await;
    assert_eq!(seen["state"], "seen", "{seen}");
    assert_eq!(seen["paid"], "0.00008334");
    assert_eq!(seen["txs"].as_array().unwrap().len(), 2);

    // One block is enough for $5.
    node.mine(1).await;
    let paid = pay(&edge, &order_id).await;
    assert_eq!(paid["state"], "paid", "{paid}");

    let (_, body) = edge
        .post("/market/account", auth("buyer-box", OTHER_TOKEN, json!({})))
        .await;
    let account = parse(&body);
    let purchase = &account["purchases"][0];
    assert_eq!(purchase["status"], "paid", "{account}");
    assert_eq!(purchase["network"], "bitcoin-regtest");
    let (_, body) = edge
        .post("/market/account", auth("seller-box", GOOD_TOKEN, json!({})))
        .await;
    let seller = parse(&body);
    // The platform's 4% of $5.00 is billed to the seller apart.
    assert_eq!(seller["coin_fees_owed"], 20, "{seller}");
    assert_eq!(seller["coins"], json!(["btc"]));
    assert_eq!(seller["coin_keys"]["btc"].as_str().unwrap(), descriptor);

    // The next order gets the next address.
    let second = order(&edge, &listing).await;
    let next = pay(&edge, second["order_id"].as_str().unwrap()).await;
    assert_ne!(next["address"], address);

    edge.shutdown().await;
}
