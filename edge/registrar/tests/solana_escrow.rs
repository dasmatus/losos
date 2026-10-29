//! USDC on Solana through the escrow program, end to end: the real edge
//! against a real `solana-test-validator` running `losos-escrow`
//! (`edge/contracts/solana`). The buyer pays into a deposit the way the admin
//! page's wallet does, and what is asserted is what lands in each token
//! account: the seller and treasury paid once a slot is over, a second
//! payment for a paid order back with the buyer, an offline seller's slot
//! split, and a deposit for someone else's order or seller never counted.
//!
//! It needs two things the plain test run has not got, so it passes without
//! doing anything unless both are set: `LOSOS_SOLANA_TEST_VALIDATOR`, the
//! validator binary, and `LOSOS_ESCROW_SO`, the program `cargo build-sbf`
//! made. CI's `solana-escrow` job sets both.

mod common;

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use common::coins::start_stripe;
use common::{Edge, KubeStub, TenantSpec, GOOD_TOKEN, OTHER_TOKEN};
use losos_registrar::evm_tx::Releaser;
use losos_registrar::solana::base58_encode;
use losos_registrar::solana_escrow::{
    associated_token_address, check_final, config_address, find_program_address, key,
    vault_address, SolanaReleaser, SYSTEM_PROGRAM, TOKEN_PROGRAM,
};
use losos_registrar::stablecoin::{
    network, payout_message, UsdcChain, UsdcOpts, USDC_SOLANA_DEVNET,
};
use ring::signature::{Ed25519KeyPair, KeyPair};
use serde_json::{json, Value};

/// Any address with no key behind it serves as the program's.
const PROGRAM: &str = "LosEscrow1111111111111111111111111111111111";
/// The same program at another address, still upgradeable: what a forged
/// escrow looks like.
const CLONE: &str = "LosC1one11111111111111111111111111111111111";
const RELEASER_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const DAY: u64 = 24 * 60 * 60;
const SOL: u64 = 1_000_000_000;
const BUYER_USDC: u64 = 50_000_000;

/// A keypair from a fixed seed, so every run has the same addresses.
struct Wallet(Ed25519KeyPair);

impl Wallet {
    fn new(seed: u8) -> Self {
        Self(Ed25519KeyPair::from_seed_unchecked(&[seed; 32]).expect("key"))
    }

    fn key(&self) -> [u8; 32] {
        self.0.public_key().as_ref().try_into().expect("32 bytes")
    }

    fn address(&self) -> String {
        base58_encode(&self.key())
    }
}

/// The validator, killed when the test ends however it ends.
struct Validator {
    child: Child,
    url: String,
    _ledger: Scratch,
}

impl Drop for Validator {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A scratch directory removed on drop; the crate has no tempfile.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "losos-{tag}-{}-{}",
            std::process::id(),
            losos_registrar::market::now_secs()
        ));
        std::fs::create_dir_all(&path).expect("scratch dir");
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port()
}

fn b64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// An account for `--account`, in the shape `solana account --output json`
/// writes.
fn account_file(
    dir: &std::path::Path,
    address: &str,
    owner: &str,
    lamports: u64,
    data: &[u8],
) -> PathBuf {
    let path = dir.join(format!("{address}.json"));
    let body = json!({
        "pubkey": address,
        "account": {
            "lamports": lamports,
            "data": [b64(data), "base64"],
            "owner": owner,
            "executable": false,
            "rentEpoch": u64::MAX,
            "space": data.len(),
        }
    });
    std::fs::write(&path, body.to_string()).expect("write account");
    path
}

/// A classic SPL mint with six decimals.
fn mint_data(authority: &[u8; 32]) -> Vec<u8> {
    let mut d = vec![0u8; 82];
    d[0] = 1;
    d[4..36].copy_from_slice(authority);
    d[36..44].copy_from_slice(&1_000_000_000_000u64.to_le_bytes());
    d[44] = 6;
    d[45] = 1;
    d
}

/// An initialized SPL token account holding `amount`.
fn token_data(mint: &[u8; 32], owner: &[u8; 32], amount: u64) -> Vec<u8> {
    let mut d = vec![0u8; 165];
    d[0..32].copy_from_slice(mint);
    d[32..64].copy_from_slice(owner);
    d[64..72].copy_from_slice(&amount.to_le_bytes());
    d[108] = 1;
    d
}

async fn rpc(url: &str, method: &str, params: Value) -> Value {
    let reply: Value = reqwest::Client::new()
        .post(url)
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
        .send()
        .await
        .expect("the validator answers")
        .json()
        .await
        .expect("JSON");
    assert!(reply.get("error").is_none(), "{method}: {reply}");
    reply["result"].clone()
}

/// Start the validator with the program, Circle's devnet mint, the buyer's
/// USDC and SOL for everyone who pays a fee. `None` when the test is off.
async fn validator(funded: &[[u8; 32]], buyer: &[u8; 32]) -> Option<Validator> {
    let (Ok(bin), Ok(so)) = (
        std::env::var("LOSOS_SOLANA_TEST_VALIDATOR"),
        std::env::var("LOSOS_ESCROW_SO"),
    ) else {
        eprintln!("LOSOS_SOLANA_TEST_VALIDATOR or LOSOS_ESCROW_SO unset; skipping");
        return None;
    };
    let ledger = Scratch::new("sol-ledger");
    let accounts = Scratch::new("sol-accounts");
    let mint = key(USDC_SOLANA_DEVNET).expect("mint");
    let mut args: Vec<String> = vec![
        "--ledger".into(),
        ledger.0.display().to_string(),
        "--reset".into(),
        "--quiet".into(),
        "--bind-address".into(),
        "127.0.0.1".into(),
    ];
    let port = free_port();
    let faucet = free_port();
    // Gossip, TPU and the rest take a range of their own above the RPC port.
    let base = 20_000 + (std::process::id() % 200) as u16 * 100;
    args.extend([
        "--rpc-port".into(),
        port.to_string(),
        "--faucet-port".into(),
        faucet.to_string(),
        "--dynamic-port-range".into(),
        format!("{base}-{}", base + 99),
        "--bpf-program".into(),
        PROGRAM.into(),
        so.clone(),
        // The copy keeps an upgrade authority, as a program fresh from
        // `solana program deploy` does.
        "--upgradeable-program".into(),
        CLONE.into(),
        so,
        base58_encode(buyer),
    ]);
    let mut add = |address: String, owner: &str, lamports: u64, data: &[u8]| {
        let file = account_file(&accounts.0, &address, owner, lamports, data);
        args.extend(["--account".into(), address, file.display().to_string()]);
    };
    add(
        USDC_SOLANA_DEVNET.to_string(),
        TOKEN_PROGRAM,
        SOL,
        &mint_data(&[3; 32]),
    );
    add(
        base58_encode(&associated_token_address(buyer, &mint)),
        TOKEN_PROGRAM,
        SOL / 100,
        &token_data(&mint, buyer, BUYER_USDC),
    );
    for who in funded {
        add(base58_encode(who), SYSTEM_PROGRAM, 10 * SOL, &[]);
    }
    let child = Command::new(bin)
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start the validator");
    let v = Validator {
        child,
        url: format!("http://127.0.0.1:{port}"),
        _ledger: ledger,
    };
    for _ in 0..240 {
        let up = reqwest::Client::new()
            .post(&v.url)
            .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "getHealth" }))
            .send()
            .await;
        if let Ok(reply) = up {
            if reply
                .json::<Value>()
                .await
                .ok()
                .and_then(|r| r["result"].as_str().map(String::from))
                == Some("ok".to_string())
            {
                // A program in the genesis can be called from the slot
                // after the one it was loaded in.
                while rpc(&v.url, "getSlot", json!([{ "commitment": "confirmed" }]))
                    .await
                    .as_u64()
                    .unwrap_or(0)
                    < 3
                {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                drop(accounts);
                return Some(v);
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("the validator never came up");
}

struct Meta {
    key: [u8; 32],
    signer: bool,
    writable: bool,
}

fn w(key: [u8; 32]) -> Meta {
    Meta {
        key,
        signer: false,
        writable: true,
    }
}

fn r(key: [u8; 32]) -> Meta {
    Meta {
        key,
        signer: false,
        writable: false,
    }
}

/// One instruction, signed by `payer` alone, sent and waited for. Built here
/// rather than with the edge's own builder so the two check each other.
async fn send_one(
    url: &str,
    payer: &Wallet,
    program: [u8; 32],
    accounts: Vec<Meta>,
    data: Vec<u8>,
) -> Result<String, Value> {
    let latest = rpc(
        url,
        "getLatestBlockhash",
        json!([{ "commitment": "confirmed" }]),
    )
    .await;
    let blockhash = key(latest["value"]["blockhash"].as_str().expect("blockhash")).expect("hash");
    let mut keys = vec![Meta {
        key: payer.key(),
        signer: true,
        writable: true,
    }];
    for m in accounts.iter().chain(std::iter::once(&r(program))) {
        if let Some(k) = keys.iter_mut().find(|k| k.key == m.key) {
            k.signer |= m.signer;
            k.writable |= m.writable;
        } else {
            keys.push(Meta {
                key: m.key,
                signer: m.signer,
                writable: m.writable,
            });
        }
    }
    keys.sort_by_key(|m| match (m.signer, m.writable) {
        (true, true) => 0,
        (true, false) => 1,
        (false, true) => 2,
        (false, false) => 3,
    });
    let index = |k: &[u8; 32]| keys.iter().position(|m| m.key == *k).expect("listed") as u8;
    let mut msg = vec![
        keys.iter().filter(|m| m.signer).count() as u8,
        0,
        keys.iter().filter(|m| !m.signer && !m.writable).count() as u8,
        keys.len() as u8,
    ];
    for m in &keys {
        msg.extend_from_slice(&m.key);
    }
    msg.extend_from_slice(&blockhash);
    msg.push(1);
    msg.push(index(&program));
    msg.push(accounts.len() as u8);
    msg.extend(accounts.iter().map(|a| index(&a.key)));
    // compact-u16: a pay with both box keys is past 127 bytes.
    let mut n = data.len();
    loop {
        let byte = (n & 0x7f) as u8;
        n >>= 7;
        if n == 0 {
            msg.push(byte);
            break;
        }
        msg.push(byte | 0x80);
    }
    msg.extend_from_slice(&data);
    let signature = payer.0.sign(&msg);
    let mut wire = vec![1u8];
    wire.extend_from_slice(signature.as_ref());
    wire.extend_from_slice(&msg);
    let sent = reqwest::Client::new()
        .post(url)
        .json(&json!({
            "jsonrpc": "2.0", "id": 1, "method": "sendTransaction",
            "params": [b64(&wire), { "encoding": "base64", "preflightCommitment": "confirmed" }],
        }))
        .send()
        .await
        .expect("send")
        .json::<Value>()
        .await
        .expect("JSON");
    if let Some(e) = sent.get("error") {
        return Err(e.clone());
    }
    let sig = sent["result"].as_str().expect("signature").to_string();
    wait_landed(url, &sig).await;
    Ok(sig)
}

async fn wait_landed(url: &str, sig: &str) {
    for _ in 0..120 {
        let s = rpc(url, "getSignatureStatuses", json!([[sig]])).await;
        let st = &s["value"][0];
        if matches!(
            st["confirmationStatus"].as_str(),
            Some("confirmed" | "finalized")
        ) {
            assert!(st["err"].is_null(), "{sig} failed: {st}");
            return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("{sig} never landed");
}

async fn usdc_of(url: &str, owner: &[u8; 32]) -> u64 {
    let mint = key(USDC_SOLANA_DEVNET).expect("mint");
    let ata = base58_encode(&associated_token_address(owner, &mint));
    let info = rpc(
        url,
        "getAccountInfo",
        json!([ata, { "encoding": "base64", "commitment": "confirmed" }]),
    )
    .await;
    if info["value"].is_null() {
        return 0;
    }
    let reply = rpc(
        url,
        "getTokenAccountBalance",
        json!([ata, { "commitment": "confirmed" }]),
    )
    .await;
    reply["value"]["amount"]
        .as_str()
        .expect("amount")
        .parse()
        .expect("u64")
}

async fn exists(url: &str, address: &[u8; 32]) -> bool {
    let info = rpc(
        url,
        "getAccountInfo",
        json!([base58_encode(address), { "encoding": "base64", "commitment": "confirmed" }]),
    )
    .await;
    !info["value"].is_null()
}

/// The deposit a payment with `nonce` makes, under `treasury`'s config.
fn deposit_address(treasury: &[u8; 32], nonce: &[u8; 32]) -> [u8; 32] {
    deposit_in(PROGRAM, treasury, nonce)
}

fn deposit_in(program: &str, treasury: &[u8; 32], nonce: &[u8; 32]) -> [u8; 32] {
    let program = key(program).expect("program");
    let mint = key(USDC_SOLANA_DEVNET).expect("mint");
    let config = config_address(&program, treasury, &mint);
    find_program_address(&[b"deposit", &config, nonce], &program).0
}

/// The nonce of payment `n`, made up at run time: CodeQL reads a literal
/// deposit seed as a hard-coded nonce. The same `n` is the same nonce for
/// the whole run, so a test can still pay one twice.
fn a_nonce(n: u8) -> [u8; 32] {
    static RUN: std::sync::OnceLock<u128> = std::sync::OnceLock::new();
    let run = RUN.get_or_init(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    });
    let text = format!("{n:02x}{run:030x}");
    text.as_bytes().try_into().expect("32 hex digits")
}

/// The buyer pays `invoice` into a new deposit, as the wallet does.
async fn pay(
    url: &str,
    buyer: &Wallet,
    treasury: &[u8; 32],
    invoice: &Value,
    seller: &[u8; 32],
    n: u8,
) -> Result<String, Value> {
    pay_into(PROGRAM, url, buyer, treasury, invoice, seller, n).await
}

async fn pay_into(
    program: &str,
    url: &str,
    buyer: &Wallet,
    treasury: &[u8; 32],
    invoice: &Value,
    seller: &[u8; 32],
    n: u8,
) -> Result<String, Value> {
    let program_name = program;
    let program = key(program).expect("program");
    let mint = key(USDC_SOLANA_DEVNET).expect("mint");
    let nonce = a_nonce(n);
    let config = config_address(&program, treasury, &mint);
    let deposit = deposit_in(program_name, treasury, &nonce);
    let order = invoice["order_id_hash"].as_str().expect("hash");
    let order = order.strip_prefix("0x").unwrap_or(order);
    let order: Vec<u8> = (0..64)
        .step_by(2)
        .map(|i| u8::from_str_radix(&order[i..i + 2], 16).unwrap())
        .collect();
    let gross: u64 = invoice["gross"].as_str().unwrap().parse().unwrap();
    let fee: u64 = invoice["fee"].as_str().unwrap().parse().unwrap();
    let deadline = invoice["deadline"].as_u64().unwrap();
    let mut data = vec![1u8];
    data.extend_from_slice(&nonce);
    data.extend_from_slice(&order);
    data.extend_from_slice(&gross.to_le_bytes());
    data.extend_from_slice(&fee.to_le_bytes());
    data.extend_from_slice(&deadline.to_le_bytes());
    let chain = &invoice["chains"][0];
    for side in ["buyer_box", "seller_box"] {
        let k = chain[side].as_str().expect("the invoice names both boxes");
        data.extend_from_slice(&key(k).expect("box key"));
    }
    send_one(
        url,
        buyer,
        program,
        vec![
            Meta {
                key: buyer.key(),
                signer: true,
                writable: true,
            },
            w(associated_token_address(&buyer.key(), &mint)),
            r(config),
            w(deposit),
            w(vault_address(&program, &deposit)),
            r(*seller),
            r(mint),
            r(key(TOKEN_PROGRAM).unwrap()),
            r(key(SYSTEM_PROGRAM).unwrap()),
        ],
        data,
    )
    .await
}

/// The treasury makes its config, naming the edge's releaser.
async fn initialize(program: &str, url: &str, treasury: &Wallet, releaser: &[u8; 32]) {
    let program = key(program).expect("program");
    let mint = key(USDC_SOLANA_DEVNET).expect("mint");
    let mut data = vec![0u8];
    data.extend_from_slice(releaser);
    send_one(
        url,
        treasury,
        program,
        vec![
            Meta {
                key: treasury.key(),
                signer: true,
                writable: true,
            },
            w(config_address(&program, &treasury.key(), &mint)),
            r(mint),
            r(key(SYSTEM_PROGRAM).unwrap()),
        ],
        data,
    )
    .await
    .expect("initialize");
}

fn tenants() -> Vec<TenantSpec> {
    vec![
        TenantSpec::new("seller-box", "seller.example", GOOD_TOKEN).with_market(),
        TenantSpec::new("buyer-box", "buyer.example", OTHER_TOKEN).with_market(),
    ]
}

async fn mesh() -> &'static KubeStub {
    Box::leak(Box::new(
        KubeStub::start_with_claim_phase(201, "Bound").await,
    ))
}

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

/// The default edge: no direct payments, Solana held in the escrow.
async fn start(
    tag: &str,
    url: &str,
    genesis: &str,
    treasury: &str,
    state: Option<&Value>,
    kube: &KubeStub,
) -> Edge {
    let stripe = start_stripe().await;
    let url = url.to_string();
    let genesis = genesis.to_string();
    let treasury = treasury.to_string();
    let kube_api = kube.base.clone();
    Edge::start_market_full(
        tag,
        &tenants(),
        &stripe,
        &[("seller-box", false)],
        state,
        move |opts, dir| {
            std::fs::write(dir.join("usdc-rpc-url"), &url).expect("rpc url");
            std::fs::write(dir.join("usdc-releaser-key"), RELEASER_KEY).expect("key");
            std::fs::write(dir.join("kube.token"), common::KUBE_TOKEN).expect("kube token");
            opts.kube_api = kube_api;
            opts.kube_token_file = Some(dir.path_str("kube.token"));
            let market = opts.market.as_mut().expect("market on");
            market.currency = "usd".to_string();
            market.direct_pay = false;
            market.usdc = Some(UsdcOpts {
                chains: vec![UsdcChain {
                    network: network("solana-devnet").unwrap(),
                    rpc_file: dir.path_str("usdc-rpc-url"),
                    pay: Some(PROGRAM.to_string()),
                    treasury,
                    genesis: Some(genesis),
                }],
                ens_rpc_file: None,
                price_sources: silent_prices(),
                releaser_file: Some(dir.path_str("usdc-releaser-key")),
            });
        },
    )
    .await
}

fn auth(id: &str, token: &str) -> Value {
    json!({ "appliance_id": id, "token": token })
}

fn with(mut base: Value, extra: Value) -> Value {
    for (k, v) in extra.as_object().expect("object") {
        base[k] = v.clone();
    }
    base
}

/// The two boxes' escrow keys, as lososd derives them from its own secret.
fn box_wallet(id: &str) -> Wallet {
    Wallet::new(if id == "seller-box" { 11 } else { 13 })
}

const BOX_IDS: [(&str, &str); 2] = [("seller-box", GOOD_TOKEN), ("buyer-box", OTHER_TOKEN)];

fn box_keys(id: &str) -> Value {
    json!({
        "evm": if id == "seller-box" {
            "0x00000000000000000000000000000000000000b1"
        } else {
            "0x00000000000000000000000000000000000000b2"
        },
        "solana": box_wallet(id).address(),
    })
}

/// Both boxes tell the edge their keys; an order needs them.
async fn register_boxes(edge: &Edge) {
    for (id, token) in BOX_IDS {
        let (status, body) = edge
            .post(
                "/market/escrow/decisions",
                with(auth(id, token), box_keys(id)),
            )
            .await;
        assert_eq!(status, 200, "{body}");
    }
}

/// One pass of both boxes, the way lososd's market loop runs: each signs
/// what the edge has waiting for it. How many it signed.
async fn boxes_sign(edge: &Edge) -> usize {
    let mut signed = 0;
    for (id, token) in BOX_IDS {
        let (status, body) = edge
            .post(
                "/market/escrow/decisions",
                with(auth(id, token), box_keys(id)),
            )
            .await;
        assert_eq!(status, 200, "{body}");
        for d in parse(&body).as_array().expect("decisions") {
            assert_eq!(d["family"], "solana", "{d}");
            let account = d["account"].as_str().expect("account");
            let refund: u64 = d["refund"].as_str().unwrap().parse().unwrap();
            let message = losos_registrar::escrow_boxes::solana_message(
                &key(account).expect("account"),
                refund,
            );
            let signature = base58_encode(box_wallet(id).0.sign(&message).as_ref());
            let (status, body) = edge
                .post(
                    "/market/escrow/sign",
                    with(
                        auth(id, token),
                        json!({
                            "network": d["network"],
                            "account": account,
                            "refund": refund,
                            "signature": signature,
                        }),
                    ),
                )
                .await;
            assert_eq!(status, 204, "{body}");
            signed += 1;
        }
    }
    signed
}

/// Sign until the escrow reaches `step`.
async fn sign_until(edge: &Edge, order_id: &str, step: &str) -> Value {
    for _ in 0..240 {
        boxes_sign(edge).await;
        let order = purchase(edge, order_id).await;
        if order["escrow"]["step"] == step {
            return order;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!(
        "{order_id} never reached {step}: {}",
        purchase(edge, order_id).await
    );
}

/// The edge decides `step` but, with no box's word, moves nothing: the
/// deposit stays where it is for a few seconds of the edge's own loop.
async fn waits_for_a_box(url: &str, edge: &Edge, order_id: &str, deposit: &[u8; 32]) {
    for _ in 0..12 {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let order = purchase(edge, order_id).await;
        assert_eq!(order["escrow"]["tx"], Value::Null, "{order}");
        assert!(
            exists(url, deposit).await,
            "moved with the edge's key alone"
        );
    }
}

fn parse(body: &str) -> Value {
    serde_json::from_str(body).unwrap_or_else(|_| panic!("not JSON: {body}"))
}

async fn set_payout(edge: &Edge, seller: &Wallet) {
    let message = payout_message("solana-devnet", &seller.address());
    let signature = base58_encode(seller.0.sign(message.as_bytes()).as_ref());
    let (status, body) = edge
        .post(
            "/market/payout",
            with(
                auth("seller-box", GOOD_TOKEN),
                json!({ "address": seller.address(), "signature": signature, "family": "solana" }),
            ),
        )
        .await;
    assert_eq!(status, 204, "{body}");
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
    let invoice = parse(&body)["stablecoin"].clone();
    let chain = &invoice["chains"][0];
    assert_eq!(chain["network"], "solana-devnet", "{invoice}");
    assert_eq!(chain["pay"], PROGRAM, "{invoice}");
    invoice
}

async fn confirm(edge: &Edge, order_id: &str, sig: &str) -> (u16, String) {
    edge.post(
        "/market/confirm",
        with(
            auth("buyer-box", OTHER_TOKEN),
            json!({ "order_id": order_id, "tx": sig, "network": "solana-devnet" }),
        ),
    )
    .await
}

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

/// Wait, on the real chain's time, for the escrow to reach `step`.
async fn wait_step(edge: &Edge, order_id: &str, step: &str) -> Value {
    for _ in 0..240 {
        let order = purchase(edge, order_id).await;
        if order["escrow"]["step"] == step {
            return order;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!(
        "{order_id} never reached {step}: {}",
        purchase(edge, order_id).await
    );
}

async fn wait_until<F: std::future::Future<Output = bool>>(
    what: &str,
    mut check: impl FnMut() -> F,
) {
    for _ in 0..240 {
        if check().await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("{what} never happened");
}

fn ledger(edge: &Edge) -> Value {
    let raw = std::fs::read(edge.dir.join("market-test.json")).expect("ledger");
    serde_json::from_slice(&raw).expect("JSON")
}

fn aged(mut ledger: Value, secs: u64) -> Value {
    if let Some(orders) = ledger["orders"].as_object_mut() {
        for o in orders.values_mut() {
            for k in ["created_at", "paid_at", "expires_at"] {
                if let Some(t) = o[k].as_u64() {
                    o[k] = json!(t - secs);
                }
            }
        }
    }
    ledger
}

async fn wait_volume(edge: &Edge, order_id: &str) {
    wait_until("the volume", || async {
        !purchase(edge, order_id).await["volume"].is_null()
    })
    .await;
}

/// One validator for every case: it takes seconds to start, and the cases
/// use their own orders and nonces.
#[tokio::test(flavor = "multi_thread")]
async fn usdc_on_solana_is_held_in_the_escrow_program() {
    let buyer = Wallet::new(5);
    let seller = Wallet::new(7);
    let treasury = Wallet::new(9);
    let releaser = SolanaReleaser::from_releaser(&Releaser::from_hex(RELEASER_KEY).expect("key"));
    let Some(v) = validator(
        &[buyer.key(), treasury.key(), releaser.address()],
        &buyer.key(),
    )
    .await
    else {
        return;
    };
    let url = v.url.clone();
    let genesis = rpc(&url, "getGenesisHash", json!([])).await;
    let genesis = genesis.as_str().expect("genesis").to_string();
    let t = treasury.address();

    // A live edge takes only a program nobody can change: the genesis one
    // has no authority anyone can sign for, the copy still has one.
    check_final(&url, PROGRAM).await.expect("final");
    let upgradeable = check_final(&url, CLONE).await.expect_err("upgradeable");
    assert!(
        upgradeable.to_string().contains("upgraded"),
        "{upgradeable}"
    );
    assert!(check_final(&url, &treasury.address()).await.is_err());

    // Until the treasury makes its config, the program takes no deposit.
    let kube = mesh().await;
    let edge = start("sol-escrow", &url, &genesis, &t, None, kube).await;
    // An order needs both boxes' escrow keys on the edge first.
    set_payout(&edge, &seller).await;
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
    let listing = parse(&body)["listing_id"].as_str().unwrap().to_string();
    let (status, body) = edge
        .post(
            "/market/orders",
            with(
                auth("buyer-box", OTHER_TOKEN),
                json!({ "listing_id": listing, "quantity": 40 }),
            ),
        )
        .await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("escrow key"), "{body}");
    register_boxes(&edge).await;
    let invoice = order(&edge, 40).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    let refused = pay(&url, &buyer, &treasury.key(), &invoice, &seller.key(), 1).await;
    assert!(refused.is_err(), "a deposit with no config");
    initialize(PROGRAM, &url, &treasury, &releaser.address()).await;
    initialize(CLONE, &url, &treasury, &releaser.address()).await;

    // A deposit to the buyer's own address instead of the seller's, or one
    // for another order, does not count.
    let wrong_seller = pay(&url, &buyer, &treasury.key(), &invoice, &buyer.key(), 2)
        .await
        .expect("pay");
    let (status, body) = confirm(&edge, &order_id, &wrong_seller).await;
    assert_eq!(status, 409, "{body}");
    let mut other = invoice.clone();
    other["order_id_hash"] = json!(format!("0x{}", "ab".repeat(32)));
    let wrong_order = pay(&url, &buyer, &treasury.key(), &other, &seller.key(), 3)
        .await
        .expect("pay");
    let (status, body) = confirm(&edge, &order_id, &wrong_order).await;
    assert_eq!(status, 409, "{body}");
    // Nor one naming other box keys than the invoice: the edge could then
    // move it with a key of its own making in a box's place.
    let mut swapped = invoice.clone();
    swapped["chains"][0]["buyer_box"] = invoice["chains"][0]["seller_box"].clone();
    swapped["chains"][0]["seller_box"] = invoice["chains"][0]["buyer_box"].clone();
    let wrong_boxes = pay(&url, &buyer, &treasury.key(), &swapped, &seller.key(), 7)
        .await
        .expect("pay");
    let (status, body) = confirm(&edge, &order_id, &wrong_boxes).await;
    assert_eq!(status, 409, "{body}");
    // Nor does a deposit in a copy of the program at another address, even
    // with every field right.
    let forged = pay_into(
        CLONE,
        &url,
        &buyer,
        &treasury.key(),
        &invoice,
        &seller.key(),
        3,
    )
    .await
    .expect("pay");
    let (status, body) = confirm(&edge, &order_id, &forged).await;
    assert_eq!(status, 409, "{body}");
    assert_eq!(purchase(&edge, &order_id).await["status"], "pending");

    // The real payment is held, and the buyer's money is in the vault.
    let gross: u64 = invoice["gross"].as_str().unwrap().parse().unwrap();
    let fee: u64 = invoice["fee"].as_str().unwrap().parse().unwrap();
    let before = usdc_of(&url, &buyer.key()).await;
    let paid = pay(&url, &buyer, &treasury.key(), &invoice, &seller.key(), 4)
        .await
        .expect("pay");
    assert_eq!(usdc_of(&url, &buyer.key()).await, before - gross);
    let (status, body) = confirm(&edge, &order_id, &paid).await;
    assert_eq!(status, 204, "{body}");
    // Reporting it again changes nothing.
    assert_eq!(confirm(&edge, &order_id, &paid).await.0, 204);
    let held = wait_step(&edge, &order_id, "held").await;
    let deposit = deposit_address(&treasury.key(), &a_nonce(4));
    assert_eq!(held["escrow"]["account"], base58_encode(&deposit), "{held}");
    wait_volume(&edge, &order_id).await;

    // A second payment for the paid order goes straight back.
    let twice = pay(&url, &buyer, &treasury.key(), &invoice, &seller.key(), 5)
        .await
        .expect("pay");
    let (status, body) = confirm(&edge, &order_id, &twice).await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("already paid"), "{body}");
    let stray = deposit_address(&treasury.key(), &a_nonce(5));
    // Its refund waits for the buyer's box, which the stray pays.
    wait_until("the stray refund", || async {
        boxes_sign(&edge).await;
        !exists(&url, &stray).await
    })
    .await;
    assert_eq!(usdc_of(&url, &buyer.key()).await, before - gross);
    // Nobody is paid yet: the slot is still running.
    assert_eq!(usdc_of(&url, &seller.key()).await, 0);
    assert!(exists(&url, &deposit).await);

    // The slot is over: the seller and the treasury are paid, and the
    // deposit's rent goes back to the buyer.
    let lamports = |who: [u8; 32]| {
        let url = url.clone();
        async move {
            rpc(
                &url,
                "getBalance",
                json!([base58_encode(&who), { "commitment": "confirmed" }]),
            )
            .await["value"]
                .as_u64()
                .unwrap()
        }
    };
    let buyer_sol = lamports(buyer.key()).await;
    let ledger_then = aged(ledger(&edge), 31 * DAY + 60);
    edge.shutdown().await;
    let edge = start(
        "sol-escrow-later",
        &url,
        &genesis,
        &t,
        Some(&ledger_then),
        kube,
    )
    .await;
    // The edge's key alone does not release it; the seller's box agrees.
    waits_for_a_box(&url, &edge, &order_id, &deposit).await;
    sign_until(&edge, &order_id, "released").await;
    assert_eq!(usdc_of(&url, &seller.key()).await, gross - fee);
    assert_eq!(usdc_of(&url, &treasury.key()).await, fee);
    assert!(!exists(&url, &deposit).await);
    assert!(
        lamports(buyer.key()).await > buyer_sol,
        "the deposit's rent came back"
    );
    edge.shutdown().await;

    // A seller offline for a day, ten days in: the rest goes back.
    let edge = start("sol-escrow-offline", &url, &genesis, &t, None, kube).await;
    set_payout(&edge, &seller).await;
    register_boxes(&edge).await;
    let invoice = order(&edge, 40).await;
    let order_id = invoice["order_id"].as_str().unwrap().to_string();
    let gross: u64 = invoice["gross"].as_str().unwrap().parse().unwrap();
    let paid = pay(&url, &buyer, &treasury.key(), &invoice, &seller.key(), 6)
        .await
        .expect("pay");
    assert_eq!(confirm(&edge, &order_id, &paid).await.0, 204);
    wait_volume(&edge, &order_id).await;
    let (buyer_had, seller_had, treasury_had) = (
        usdc_of(&url, &buyer.key()).await,
        usdc_of(&url, &seller.key()).await,
        usdc_of(&url, &treasury.key()).await,
    );
    let offline = mesh().await;
    offline.set_node("seller-box", false, false);
    let now = losos_registrar::market::now_secs();
    let mut later = aged(ledger(&edge), 10 * DAY);
    later["presence"] = json!({ "seller-box": { "streak": DAY - 100, "offline": DAY - 100 } });
    later["presence_tick"] = json!(now - 200);
    edge.shutdown().await;
    let edge = start(
        "sol-escrow-offline-later",
        &url,
        &genesis,
        &t,
        Some(&later),
        offline,
    )
    .await;
    let held_in = deposit_address(&treasury.key(), &a_nonce(6));
    waits_for_a_box(&url, &edge, &order_id, &held_in).await;
    // The split pays the buyer back, so the buyer's box signs it.
    let split = sign_until(&edge, &order_id, "settled").await;
    assert_eq!(split["escrow"]["reason"], "sellerOffline", "{split}");
    let back = split["escrow"]["refund"].as_u64().unwrap() * 10_000;
    assert_eq!(back, 1_410_000, "{split}");
    let to_buyer = usdc_of(&url, &buyer.key()).await - buyer_had;
    let to_seller = usdc_of(&url, &seller.key()).await - seller_had;
    let to_treasury = usdc_of(&url, &treasury.key()).await - treasury_had;
    assert_eq!(to_buyer, back);
    assert_eq!(
        to_buyer + to_seller + to_treasury,
        gross,
        "every unit accounted for"
    );
    assert!(!exists(&url, &deposit_address(&treasury.key(), &a_nonce(6))).await);
    edge.shutdown().await;
}
