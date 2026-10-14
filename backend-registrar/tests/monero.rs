//! Monero orders against a real `monerod` on regtest.
//!
//! Two wallet services run beside the node: the edge's, which only ever gets
//! the seller's address and view key, and the test's own, which holds the
//! seller's and the buyer's full wallets. The buyer pays the subaddress the
//! edge handed out. Runs when `LOSOS_TEST_MONEROD` names a `monerod` binary
//! with `monero-wallet-rpc` beside it, and passes without doing anything
//! otherwise:
//!
//! ```text
//! LOSOS_TEST_MONEROD=$(command -v monerod) cargo test --test monero
//! ```

mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use axum::{extract::Query, routing::get, Json, Router};
use common::{Edge, TenantSpec, GOOD_TOKEN, OTHER_TOKEN};
use losos_registrar::coins::PriceSource;
use losos_registrar::monero::XmrOpts;
use losos_registrar::stablecoin::{network, UsdcChain, UsdcOpts};
use serde_json::{json, Value};

const LOGIN: &str = "losos:regtest-only";

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port()
}

/// A process stopped when dropped.
struct Child(tokio::process::Child);

impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.start_kill();
    }
}

/// A regtest node and two wallet services in a temporary directory.
struct Rig {
    _node: Child,
    _edge_wallets: Child,
    _test_wallets: Child,
    daemon: String,
    edge_wallet_rpc: String,
    test_wallet_rpc: String,
    login_file: PathBuf,
    _dir: common::TempDir,
}

async fn rpc(url: &str, method: &str, params: Value) -> Result<Value, Value> {
    let reply: Value = reqwest::Client::new()
        .post(format!("{url}/json_rpc"))
        .json(&json!({"jsonrpc": "2.0", "id": "t", "method": method, "params": params}))
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

impl Rig {
    async fn start(monerod: &Path) -> Rig {
        let bin = monerod.parent().expect("a directory");
        let dir = common::TempDir::new("monero");
        let rpc_port = free_port();
        let node = tokio::process::Command::new(monerod)
            .args(["--regtest", "--offline", "--fixed-difficulty", "1"])
            .args(["--no-igd", "--no-zmq", "--non-interactive"])
            .arg(format!("--data-dir={}", dir.path_str("node")))
            .arg(format!("--log-file={}", dir.path_str("monerod.log")))
            .arg(format!("--p2p-bind-port={}", free_port()))
            .arg("--rpc-bind-ip=127.0.0.1")
            .arg(format!("--rpc-bind-port={rpc_port}"))
            .kill_on_drop(true)
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("start monerod");
        let daemon = format!("http://127.0.0.1:{rpc_port}");
        let login_file = dir.join("rpc-login");
        std::fs::write(&login_file, LOGIN).expect("login");
        std::fs::write(dir.join("wallet-rpc.conf"), format!("rpc-login={LOGIN}\n")).expect("conf");
        let wallets = |name: &str, port: u16| {
            std::fs::create_dir_all(dir.join(name)).expect("wallet dir");
            let mut c = tokio::process::Command::new(bin.join("monero-wallet-rpc"));
            c.arg(format!("--daemon-address=127.0.0.1:{rpc_port}"))
                .arg("--trusted-daemon")
                // A regtest chain starts at the newest fork from block 1,
                // which the wallet otherwise takes for an outdated node.
                .arg("--allow-mismatched-daemon-version")
                .arg(format!("--wallet-dir={}", dir.path_str(name)))
                .arg(format!(
                    "--log-file={}",
                    dir.path_str(&format!("{name}.log"))
                ))
                .arg("--rpc-bind-ip=127.0.0.1")
                .arg(format!("--rpc-bind-port={port}"))
                .arg("--non-interactive")
                .kill_on_drop(true)
                .stdout(std::process::Stdio::null());
            c
        };
        let edge_port = free_port();
        let test_port = free_port();
        let edge_wallets = wallets("edge", edge_port)
            .arg(format!("--config-file={}", dir.path_str("wallet-rpc.conf")))
            .spawn()
            .expect("start the edge's wallet service");
        let test_wallets = wallets("test", test_port)
            .arg("--disable-rpc-login")
            .spawn()
            .expect("start the test's wallet service");
        let rig = Rig {
            _node: Child(node),
            _edge_wallets: Child(edge_wallets),
            _test_wallets: Child(test_wallets),
            daemon,
            edge_wallet_rpc: format!("http://127.0.0.1:{edge_port}"),
            test_wallet_rpc: format!("http://127.0.0.1:{test_port}"),
            login_file,
            _dir: dir,
        };
        for _ in 0..150 {
            let up = rpc(&rig.daemon, "get_info", json!({})).await.is_ok()
                && rpc(&rig.test_wallet_rpc, "get_version", json!({}))
                    .await
                    .is_ok();
            if up {
                return rig;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        panic!("monerod or monero-wallet-rpc did not come up");
    }

    /// Open one of the test's own wallets and call it.
    async fn wallet(&self, name: &str, method: &str, params: Value) -> Value {
        rpc(
            &self.test_wallet_rpc,
            "open_wallet",
            json!({"filename": name, "password": ""}),
        )
        .await
        .unwrap_or_else(|e| panic!("open {name}: {e}"));
        rpc(&self.test_wallet_rpc, method, params)
            .await
            .unwrap_or_else(|e| panic!("{name} {method}: {e}"))
    }

    async fn create(&self, name: &str) -> String {
        rpc(
            &self.test_wallet_rpc,
            "create_wallet",
            json!({"filename": name, "password": "", "language": "English"}),
        )
        .await
        .unwrap_or_else(|e| panic!("create {name}: {e}"));
        self.wallet(name, "get_address", json!({"account_index": 0}))
            .await["address"]
            .as_str()
            .unwrap()
            .to_string()
    }

    async fn mine(&self, blocks: u64, to: &str) {
        rpc(
            &self.daemon,
            "generateblocks",
            json!({"amount_of_blocks": blocks, "wallet_address": to}),
        )
        .await
        .expect("mine");
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

async fn start_edge(rig: &Rig, prices: [PriceSource; 2]) -> Edge {
    let stripe = start_stripe().await;
    let xmr = XmrOpts {
        wallet_rpc: rig.edge_wallet_rpc.clone(),
        login_file: rig.login_file.to_string_lossy().into_owned(),
        daemon_rpc: rig.daemon.clone(),
    };
    Edge::start_market_full(
        "monero",
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
            market.xmr = Some(xmr);
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
                json!({"order_id": order_id, "coin": "xmr"}),
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

async fn set_key(edge: &Edge, address: &str, view_key: &str) -> (u16, String) {
    edge.post(
        "/market/coins/key",
        auth(
            "seller-box",
            GOOD_TOKEN,
            json!({"coin": "xmr", "key": address, "view_key": view_key}),
        ),
    )
    .await
}

#[tokio::test]
async fn a_monero_order_is_paid_to_the_sellers_own_subaddress() {
    let Some(binary) = std::env::var_os("LOSOS_TEST_MONEROD") else {
        eprintln!("LOSOS_TEST_MONEROD is not set; skipping");
        return;
    };
    let rig = Rig::start(Path::new(&binary)).await;
    let buyer = rig.create("buyer").await;
    let seller = rig.create("seller").await;
    let key = |kind: &'static str| {
        let rig = &rig;
        async move {
            rig.wallet("seller", "query_key", json!({"key_type": kind}))
                .await["key"]
                .as_str()
                .unwrap()
                .to_string()
        }
    };
    let view_key = key("view_key").await;
    let spend_key = key("spend_key").await;
    // Coinbase outputs unlock after 60 blocks; the rest are decoys.
    rig.mine(80, &buyer).await;

    let edge = start_edge(&rig, prices("150.00").await).await;

    // The spend key is refused by name, a subaddress for not being the
    // primary one, a stranger's view key for not matching.
    let (status, body) = set_key(&edge, &seller, &spend_key).await;
    assert_eq!(status, 400, "{body}");
    assert!(body.contains("spend key"), "{body}");
    let sub = rig
        .wallet("seller", "create_address", json!({"account_index": 0}))
        .await["address"]
        .as_str()
        .unwrap()
        .to_string();
    let (status, body) = set_key(&edge, &sub, &view_key).await;
    assert_eq!(status, 400, "{body}");
    let (status, body) = set_key(&edge, &buyer, &view_key).await;
    assert_eq!(status, 400, "{body}");
    let (status, body) = set_key(&edge, &seller, &view_key).await;
    assert_eq!(status, 204, "{body}");

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
        json!(["xmr"]),
        "{checkout}"
    );
    let order_id = checkout["order_id"].as_str().unwrap().to_string();

    // $5.00 at $150 is 0.0333... XMR, rounded up for the seller.
    let invoice = pay(&edge, &order_id).await;
    assert_eq!(invoice["amount"], "0.033333333334", "{invoice}");
    assert_eq!(invoice["state"], "waiting");
    assert_eq!(invoice["required"], 10);
    let address = invoice["address"].as_str().unwrap().to_string();
    assert!(address.starts_with('8'), "{address}");
    assert!(invoice["uri"]
        .as_str()
        .unwrap()
        .starts_with(&format!("monero:{address}?tx_amount=0.033333333334")));
    // The subaddress is the seller's own: their wallet knows its index. The
    // seller made subaddress 1 above, but the edge never saw it handed out,
    // so the order may get it; any index of the first account is theirs.
    let index = rig
        .wallet("seller", "get_address_index", json!({"address": address}))
        .await;
    assert_eq!(index["index"]["major"], 0, "{index}");
    assert!(index["index"]["minor"].as_u64().unwrap() >= 1, "{index}");
    assert_eq!(pay(&edge, &order_id).await["address"], address);

    // Two payments add up; the first alone is short.
    rig.wallet("buyer", "refresh", json!({})).await;
    rig.wallet(
        "buyer",
        "transfer",
        json!({"destinations": [{"amount": 20_000_000_000u64, "address": address}]}),
    )
    .await;
    let short = pay(&edge, &order_id).await;
    assert_eq!(short["state"], "underpaid", "{short}");
    rig.wallet(
        "buyer",
        "transfer",
        json!({"destinations": [{"amount": 13_333_333_334u64, "address": address}]}),
    )
    .await;
    let seen = pay(&edge, &order_id).await;
    assert_eq!(seen["state"], "seen", "{seen}");
    assert_eq!(seen["paid"], "0.033333333334");
    assert_eq!(seen["txs"].as_array().unwrap().len(), 2);

    // Ten blocks for Monero.
    rig.mine(9, &buyer).await;
    assert_eq!(pay(&edge, &order_id).await["state"], "confirming");
    rig.mine(1, &buyer).await;
    let paid = pay(&edge, &order_id).await;
    assert_eq!(paid["state"], "paid", "{paid}");

    let (_, body) = edge
        .post("/market/account", auth("buyer-box", OTHER_TOKEN, json!({})))
        .await;
    let account = parse(&body);
    assert_eq!(account["purchases"][0]["status"], "paid", "{account}");
    let (_, body) = edge
        .post("/market/account", auth("seller-box", GOOD_TOKEN, json!({})))
        .await;
    let seller_view = parse(&body);
    assert_eq!(seller_view["coin_fees_owed"], 20, "{seller_view}");
    // The address is shown back; the view key never is.
    assert_eq!(seller_view["coin_keys"]["xmr"].as_str().unwrap(), seller);
    assert!(!body.contains(&view_key));

    // The next order gets the next subaddress.
    let second = order(&edge, &listing).await;
    let next = pay(&edge, second["order_id"].as_str().unwrap()).await;
    assert_ne!(next["address"], address);
    assert!(next["address"].as_str().unwrap().starts_with('8'));

    edge.shutdown().await;
}
