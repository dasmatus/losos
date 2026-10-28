//! Shielded Zcash orders against stand-in lightwalletd servers.
//!
//! No Zcash node runs here. Each stand-in is a gRPC server in this process
//! that answers the four `CompactTxStreamer` calls the edge makes, from a
//! chain of compact blocks the test builds. The payments in those blocks are
//! real notes: Ironwood (V3) and Orchard (V2) note plaintexts encrypted to
//! the order's address with the orchard crate, as a wallet would, so the
//! edge's trial decryption is the same code a mainnet payment meets. What is
//! not real is the chain around them: no proofs, no signatures, block hashes
//! the test makes up. Two honest servers of different operators share one
//! chain; a third lies, with a forged payment of its own, and a fourth is on
//! the wrong network. The seller's key is made up for this test.

mod common;

use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use common::coins::{auth, order, parse, pay, prices, start_coin_edge};
use common::{Edge, GOOD_TOKEN, OTHER_TOKEN};
use losos_registrar::coins::PriceSource;
use losos_registrar::zcash::proto::{
    BlockId, BlockRange, ChainSpec, CompactBlock, CompactOrchardAction, CompactTx, Empty,
    GetMempoolTxRequest, LightdInfo,
};
use losos_registrar::zcash::{Network, ZecOpts};
use orchard::keys::{FullViewingKey, Scope, SpendingKey};
use orchard::note::{ExtractedNoteCommitment, NoteVersion, Nullifier, RandomSeed, Rho};
use orchard::note_encryption::{IronwoodNoteEncryption, OrchardDomain, OrchardNoteEncryption};
use orchard::value::NoteValue;
use serde_json::json;
use tonic::codegen::{http, BoxFuture, BoxStream, Context, Poll, Service};
use zcash_address::unified::{self, Encoding};
use zcash_note_encryption::Domain;
use zcash_protocol::consensus::NetworkType;

// ── A chain of compact blocks ─────────────────────────────────────────────

#[derive(Default)]
struct Chain {
    blocks: Vec<CompactBlock>,
    mempool: Vec<CompactTx>,
}

impl Chain {
    fn new(height: u64) -> Chain {
        let mut chain = Chain::default();
        while chain.tip() < height || chain.blocks.is_empty() {
            chain.mine(Vec::new());
        }
        chain
    }

    fn tip(&self) -> u64 {
        self.blocks.last().map_or(0, |b| b.height)
    }

    /// A block on top holding `vtx`, its hash made up from its height and
    /// transactions; what was in the mempool and is now mined leaves it.
    fn mine(&mut self, vtx: Vec<CompactTx>) {
        let height = if self.blocks.is_empty() {
            0
        } else {
            self.tip() + 1
        };
        let mut hash = [0u8; 32];
        hash[..8].copy_from_slice(&height.to_le_bytes());
        for tx in &vtx {
            for (i, b) in tx.txid.iter().enumerate() {
                hash[8 + i % 24] ^= b;
            }
        }
        let prev = self
            .blocks
            .last()
            .map(|b| b.hash.clone())
            .unwrap_or_default();
        self.mempool
            .retain(|p| !vtx.iter().any(|t| t.txid == p.txid));
        self.blocks.push(CompactBlock {
            height,
            hash: hash.to_vec(),
            prev_hash: prev,
            time: 1_700_000_000 + u32::try_from(height).unwrap() * 75,
            vtx,
        });
    }

    /// Mine what is in the mempool now.
    fn mine_pool(&mut self) {
        let pool = self.mempool.clone();
        self.mine(pool);
    }
}

/// A transaction with one action paying `zatoshi` to `to`: an Ironwood note
/// in the Ironwood pool, or an Orchard note in the Orchard pool. `n` makes
/// its txid and its nullifier its own.
fn payment(n: u8, to: orchard::Address, zatoshi: u64, ironwood: bool) -> CompactTx {
    let mut nf = [0u8; 32];
    nf[0] = n;
    nf[1] = 0x5a;
    // The new note's rho is the nullifier of the note the action spends.
    let rho = Rho::from_bytes(&nf).unwrap();
    let nf = Nullifier::from_bytes(&nf).unwrap();
    let rseed = (0u8..=255)
        .find_map(|k| Option::from(RandomSeed::from_bytes([k ^ n; 32], &rho)))
        .expect("a seed");
    let version = if ironwood {
        NoteVersion::V3
    } else {
        NoteVersion::V2
    };
    let note =
        orchard::Note::from_parts(to, NoteValue::from_raw(zatoshi), rho, rseed, version).unwrap();
    let cmx = ExtractedNoteCommitment::from(note.commitment());
    let (epk, ct) = if ironwood {
        let enc = IronwoodNoteEncryption::new(None, note, [0u8; 512]);
        (
            OrchardDomain::epk_bytes(enc.epk()),
            enc.encrypt_note_plaintext(),
        )
    } else {
        let enc = OrchardNoteEncryption::new(None, note, [0u8; 512]);
        (
            OrchardDomain::epk_bytes(enc.epk()),
            enc.encrypt_note_plaintext(),
        )
    };
    let action = CompactOrchardAction {
        nullifier: nf.to_bytes().to_vec(),
        cmx: cmx.to_bytes().to_vec(),
        ephemeral_key: epk.0.to_vec(),
        ciphertext: ct.as_ref()[..52].to_vec(),
    };
    let mut txid = vec![n; 32];
    txid[31] = 0x7c;
    CompactTx {
        index: 1,
        txid,
        actions: if ironwood {
            Vec::new()
        } else {
            vec![action.clone()]
        },
        ironwood_actions: if ironwood { vec![action] } else { Vec::new() },
    }
}

/// How the txid of `payment(n, ..)` is shown.
fn txid_shown(n: u8) -> String {
    let mut txid = vec![n; 32];
    txid[31] = 0x7c;
    losos_registrar::zcash::txid_hex(&txid)
}

// ── A stand-in lightwalletd ───────────────────────────────────────────────

#[derive(Clone)]
struct Lightwalletd {
    chain_name: &'static str,
    chain: Arc<Mutex<Chain>>,
}

impl tonic::server::NamedService for Lightwalletd {
    const NAME: &'static str = "cash.z.wallet.sdk.rpc.CompactTxStreamer";
}

struct Unary<F>(F);

impl<Req, Res, F> tonic::server::UnaryService<Req> for Unary<F>
where
    F: FnMut(Req) -> Result<Res, tonic::Status>,
    Res: Send + 'static,
{
    type Response = Res;
    type Future = BoxFuture<tonic::Response<Res>, tonic::Status>;
    fn call(&mut self, request: tonic::Request<Req>) -> Self::Future {
        let out = (self.0)(request.into_inner()).map(tonic::Response::new);
        Box::pin(async move { out })
    }
}

struct Streaming<F>(F);

impl<Req, Res, F> tonic::server::ServerStreamingService<Req> for Streaming<F>
where
    F: FnMut(Req) -> Result<Vec<Res>, tonic::Status>,
    Res: Send + 'static,
{
    type Response = Res;
    type ResponseStream = BoxStream<Res>;
    type Future = BoxFuture<tonic::Response<BoxStream<Res>>, tonic::Status>;
    fn call(&mut self, request: tonic::Request<Req>) -> Self::Future {
        let out = (self.0)(request.into_inner()).map(|items| {
            let stream: BoxStream<Res> =
                Box::pin(futures_util::stream::iter(items.into_iter().map(Ok)));
            tonic::Response::new(stream)
        });
        Box::pin(async move { out })
    }
}

impl Service<http::Request<tonic::body::Body>> for Lightwalletd {
    type Response = http::Response<tonic::body::Body>;
    type Error = Infallible;
    type Future = BoxFuture<Self::Response, Infallible>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: http::Request<tonic::body::Body>) -> Self::Future {
        let me = self.clone();
        let method = req
            .uri()
            .path()
            .rsplit('/')
            .next()
            .unwrap_or("")
            .to_string();
        Box::pin(async move {
            let reply = match method.as_str() {
                "GetLightdInfo" => {
                    let svc = Unary(|_: Empty| {
                        Ok(LightdInfo {
                            version: "stand-in".to_string(),
                            vendor: "losos tests".to_string(),
                            chain_name: me.chain_name.to_string(),
                            block_height: me.chain.lock().unwrap().tip(),
                        })
                    });
                    tonic::server::Grpc::new(tonic_prost::ProstCodec::default())
                        .unary(svc, req)
                        .await
                }
                "GetLatestBlock" => {
                    let svc = Unary(|_: ChainSpec| {
                        let chain = me.chain.lock().unwrap();
                        let last = chain.blocks.last().expect("a block");
                        Ok(BlockId {
                            height: last.height,
                            hash: last.hash.clone(),
                        })
                    });
                    tonic::server::Grpc::new(tonic_prost::ProstCodec::default())
                        .unary(svc, req)
                        .await
                }
                "GetBlockRange" => {
                    let svc = Streaming(|range: BlockRange| {
                        let start = range.start.map_or(0, |b| b.height);
                        let end = range.end.map_or(0, |b| b.height);
                        let chain = me.chain.lock().unwrap();
                        if end > chain.tip() {
                            return Err(tonic::Status::out_of_range("past the tip"));
                        }
                        Ok(chain
                            .blocks
                            .iter()
                            .filter(|b| (start..=end).contains(&b.height))
                            .cloned()
                            .collect())
                    });
                    tonic::server::Grpc::new(tonic_prost::ProstCodec::default())
                        .server_streaming(svc, req)
                        .await
                }
                "GetMempoolTx" => {
                    let svc = Streaming(|_: GetMempoolTxRequest| {
                        Ok(me.chain.lock().unwrap().mempool.clone())
                    });
                    tonic::server::Grpc::new(tonic_prost::ProstCodec::default())
                        .server_streaming(svc, req)
                        .await
                }
                _ => tonic::Status::unimplemented("not served here").into_http(),
            };
            Ok(reply)
        })
    }
}

/// Serve `chain` as a lightwalletd on `chain_name`; its URL.
async fn serve(chain_name: &'static str, chain: Arc<Mutex<Chain>>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let url = format!("http://{}", listener.local_addr().expect("addr"));
    let svc = Lightwalletd { chain_name, chain };
    tokio::spawn(async move {
        let _ = tonic::transport::Server::builder()
            .add_service(svc)
            .serve_with_incoming(tonic::transport::server::TcpIncoming::from(listener))
            .await;
    });
    url
}

// ── The edge ──────────────────────────────────────────────────────────────

async fn start_edge(servers: Vec<String>, prices: [PriceSource; 2]) -> Edge {
    // The test edge's Stripe key is a test key, so it settles on regtest; a
    // live edge takes mainnet only.
    start_coin_edge("zcash", prices, move |market| {
        market.zec = Some(ZecOpts {
            servers,
            network: Network::Regtest,
        });
    })
    .await
}

async fn set_key(edge: &Edge, key: &str) -> (u16, String) {
    edge.post(
        "/market/coins/key",
        auth("seller-box", GOOD_TOKEN, json!({"coin": "zec", "key": key})),
    )
    .await
}

fn ufvk(fvk: &FullViewingKey, network: NetworkType) -> String {
    unified::Ufvk::try_from_items(
        unified::Revision::R0,
        vec![unified::Uitem::Data(unified::Fvk::Orchard(fvk.to_bytes()))],
    )
    .unwrap()
    .encode(&network)
}

/// The receiver inside an Orchard-only unified address.
fn receiver(address: &str) -> orchard::Address {
    let (_, _, ua) = unified::Address::decode(address).expect("a unified address");
    let raw = zcash_address::unified::Container::items(&ua)
        .into_iter()
        .find_map(|r| match r {
            unified::Receiver::Orchard(b) => Some(b),
            _ => None,
        })
        .expect("an Orchard receiver");
    Option::from(orchard::Address::from_raw_address_bytes(&raw)).expect("a valid receiver")
}

#[tokio::test]
async fn a_zcash_order_is_paid_to_a_fresh_shielded_address_two_servers_agree_on() {
    // The seller's wallet, made up here; nobody holds it.
    let sk = Option::<SpendingKey>::from(SpendingKey::from_bytes([0x42; 32])).expect("a key");
    let fvk = FullViewingKey::from(&sk);
    let seller_key = ufvk(&fvk, NetworkType::Regtest);
    let wallet_ivk = fvk.to_ivk(Scope::External);

    let honest = Arc::new(Mutex::new(Chain::new(100)));
    let lying = Arc::new(Mutex::new(Chain::new(100)));
    let elsewhere = Arc::new(Mutex::new(Chain::new(100)));
    let servers = vec![
        serve("regtest", Arc::clone(&honest)).await,
        serve("regtest", Arc::clone(&honest)).await,
        serve("regtest", Arc::clone(&lying)).await,
        serve("main", Arc::clone(&elsewhere)).await,
    ];
    let edge = start_edge(servers, prices("40.00").await).await;

    // A spending key and a seed are refused by name; a mainnet key on a
    // regtest edge for its network; an address for not being a key.
    for (secret, says) in [
        (
            "secret-orchard-sk-regtest1notarealkeyonlyitsshape".to_string(),
            "spending key",
        ),
        (["zoo"; 24].join(" "), "seed phrase"),
        (ufvk(&fvk, NetworkType::Main), "another network"),
    ] {
        let (status, body) = set_key(&edge, &secret).await;
        assert_eq!(status, 400, "{body}");
        assert!(body.contains(says), "{body}");
    }
    let (status, body) = set_key(&edge, &seller_key).await;
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
        json!(["zec"]),
        "{checkout}"
    );
    let order_id = checkout["order_id"].as_str().unwrap().to_string();

    // $5.00 at $40 is 0.125 ZEC; three blocks for an order this size.
    let invoice = pay(&edge, &order_id, "zec").await;
    assert_eq!(invoice["amount"], "0.12500000", "{invoice}");
    assert_eq!(invoice["state"], "waiting");
    assert_eq!(invoice["required"], 3);
    let address = invoice["address"].as_str().unwrap().to_string();
    assert!(address.starts_with("uregtest1"), "{address}");
    assert!(invoice["uri"]
        .as_str()
        .unwrap()
        .starts_with(&format!("zcash:{address}?amount=0.125&label=")));
    // The address is the seller's own: their key finds it, at an index of
    // its own far above the ones a wallet hands out.
    let to = receiver(&address);
    let index = wallet_ivk
        .diversifier_index(&to)
        .expect("the seller's address");
    assert_eq!(index.as_bytes()[10], 1);
    assert_ne!(to, wallet_ivk.address_at(0u32));
    assert_eq!(pay(&edge, &order_id, "zec").await["address"], address);

    // One server shows the whole amount paid, in its mempool and in a block
    // of its own. Nobody else does, so it does not count.
    {
        let mut liar = lying.lock().unwrap();
        liar.mempool.push(payment(1, to, 12_500_000, true));
        liar.mine_pool();
        liar.mempool.push(payment(2, to, 12_500_000, true));
    }
    assert_eq!(pay(&edge, &order_id, "zec").await["state"], "waiting");

    // Part of it reaches the address as an Ironwood note: underpaid.
    honest
        .lock()
        .unwrap()
        .mempool
        .push(payment(3, to, 5_000_000, true));
    let short = pay(&edge, &order_id, "zec").await;
    assert_eq!(short["state"], "underpaid", "{short}");
    assert_eq!(short["paid"], "0.05000000");

    // The rest as an Orchard note: enough, in the mempool.
    honest
        .lock()
        .unwrap()
        .mempool
        .push(payment(4, to, 7_500_000, false));
    let seen = pay(&edge, &order_id, "zec").await;
    assert_eq!(seen["state"], "seen", "{seen}");
    assert_eq!(seen["paid"], "0.12500000");
    let mut txs: Vec<String> = seen["txs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap().to_string())
        .collect();
    txs.sort();
    let mut want = vec![txid_shown(3), txid_shown(4)];
    want.sort();
    assert_eq!(txs, want);

    // Mined, then two blocks on top: paid. The liar keeps up, on its own
    // chain, so the agreed tip moves.
    let mine = |n: usize| {
        for _ in 0..n {
            honest.lock().unwrap().mine(Vec::new());
            lying.lock().unwrap().mine(Vec::new());
        }
    };
    {
        let mut chain = honest.lock().unwrap();
        chain.mine_pool();
        assert!(chain.mempool.is_empty());
    }
    lying.lock().unwrap().mine(Vec::new());
    let confirming = pay(&edge, &order_id, "zec").await;
    assert_eq!(confirming["state"], "confirming", "{confirming}");
    assert_eq!(confirming["confirmations"], 1);
    mine(2);
    let paid = pay(&edge, &order_id, "zec").await;
    assert_eq!(paid["state"], "paid", "{paid}");

    let (_, body) = edge
        .post("/market/account", auth("buyer-box", OTHER_TOKEN, json!({})))
        .await;
    let account = parse(&body);
    assert_eq!(account["purchases"][0]["status"], "paid");
    let (_, body) = edge
        .post("/market/account", auth("seller-box", GOOD_TOKEN, json!({})))
        .await;
    let seller_view = parse(&body);
    // The coins went to the seller; the 4% is owed apart.
    assert_eq!(seller_view["coin_fees_owed"], 20, "{seller_view}");
    // The wallet's first address is shown back; the key never is, in either
    // the form the seller sent or the one the edge keeps.
    let shown = seller_view["coin_keys"]["zec"].as_str().unwrap();
    assert_eq!(receiver(shown), wallet_ivk.address_at(0u32));
    assert!(!body.contains(&seller_key));
    assert!(!body.contains("uivk"), "{body}");

    // A payment to the next address before it was anyone's order does not
    // count for the order that gets it.
    let next_to = {
        let parsed = losos_registrar::zcash::parse_key(&seller_key).unwrap();
        receiver(&parsed.order_address(1))
    };
    {
        let mut chain = honest.lock().unwrap();
        chain.mine(vec![payment(5, next_to, 12_500_000, true)]);
    }
    lying.lock().unwrap().mine(Vec::new());
    let second = order(&edge, &listing).await;
    let next = pay(&edge, second["order_id"].as_str().unwrap(), "zec").await;
    assert_ne!(next["address"], address);
    assert_eq!(receiver(next["address"].as_str().unwrap()), next_to);
    mine(3);
    assert_eq!(
        pay(&edge, second["order_id"].as_str().unwrap(), "zec").await["state"],
        "waiting"
    );

    edge.shutdown().await;
}

/// The chain a server is on is checked before anything else is asked: with
/// only one server on the edge's network, nothing is quoted.
#[tokio::test]
async fn servers_on_the_wrong_network_are_not_asked() {
    let main = Arc::new(Mutex::new(Chain::new(10)));
    let servers = vec![
        serve("regtest", Arc::new(Mutex::new(Chain::new(10)))).await,
        serve("main", Arc::clone(&main)).await,
        serve("test", Arc::clone(&main)).await,
    ];
    let edge = start_edge(servers, prices("40.00").await).await;
    let sk = Option::<SpendingKey>::from(SpendingKey::from_bytes([0x43; 32])).expect("a key");
    let key = ufvk(&FullViewingKey::from(&sk), NetworkType::Regtest);
    let (status, body) = set_key(&edge, &key).await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("fewer than two"), "{body}");
    edge.shutdown().await;
}
