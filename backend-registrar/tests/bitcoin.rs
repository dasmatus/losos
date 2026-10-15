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

use common::coins::{auth, free_port, order, parse, pay, prices, start_coin_edge};
use common::{Edge, GOOD_TOKEN, OTHER_TOKEN};
use losos_registrar::bitcoin::BtcOpts;
use losos_registrar::coins::PriceSource;
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

async fn start_edge(node: &Bitcoind, prices: [PriceSource; 2]) -> Edge {
    let btc = BtcOpts {
        rpc: node.rpc.clone(),
        cookie_file: node.cookie.to_string_lossy().into_owned(),
        wallet: "losos-watch".to_string(),
    };
    start_coin_edge("bitcoin", prices, move |market| market.btc = Some(btc)).await
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
    let invoice = pay(&edge, &order_id, "btc").await;
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
    assert_eq!(pay(&edge, &order_id, "btc").await["address"], address);

    // Two payments add up; the first alone is short.
    node.send(&address, 0.00004).await;
    let short = pay(&edge, &order_id, "btc").await;
    assert_eq!(short["state"], "underpaid", "{short}");
    assert_eq!(short["due"], "0.00008334");
    node.send(&address, 0.00004334).await;
    let seen = pay(&edge, &order_id, "btc").await;
    assert_eq!(seen["state"], "seen", "{seen}");
    assert_eq!(seen["paid"], "0.00008334");
    assert_eq!(seen["txs"].as_array().unwrap().len(), 2);

    // One block is enough for $5.
    node.mine(1).await;
    let paid = pay(&edge, &order_id, "btc").await;
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
    let next = pay(&edge, second["order_id"].as_str().unwrap(), "btc").await;
    assert_ne!(next["address"], address);

    edge.shutdown().await;
}
