//! Shielded Zcash orders, watched through public lightwalletd servers.
//!
//! The seller hands the edge a unified viewing key: the full one (`uview…`)
//! or, better, the incoming one (`uivk…`). The edge keeps only the incoming
//! viewing key of its Orchard part, which sees what arrives and can neither
//! spend nor see what the seller sends. Each order gets its own address, an
//! Orchard-only unified address of that key at a diversifier index of its
//! own, so the coins land in the seller's wallet: the wallet finds a payment
//! to any of its diversified addresses without being told about it.
//!
//! Since NU6.3 a payment to an Orchard receiver lands in the Ironwood pool,
//! which uses Orchard's keys and addresses with notes of its own version. The
//! edge trial-decrypts both kinds of compact action.
//!
//! The edge runs no node. It asks public lightwalletd-protocol servers for
//! compact blocks (`GetBlockRange`) and decrypts them itself, so the viewing
//! key never leaves the edge. One server could lie, so a payment counts only
//! when servers of two different operators report the same block hash at its
//! height and the same decrypted output in it; a payment still in the mempool
//! counts as seen only when two operators show it too.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use orchard::keys::{DiversifierIndex, FullViewingKey, IncomingViewingKey, Scope};
use orchard::note_encryption::{CompactAction, IronwoodDomain, OrchardDomain};
use zcash_address::unified::{self, Container, Encoding};
use zcash_protocol::consensus::NetworkType;

use crate::coins::Received;
use crate::market::MarketError;

/// The servers a live edge asks by default: mainnet only, run by four
/// different operators so that any two that agree are independent. Taken
/// from hosh.zec.rocks/zec, each seen answering `chainName` "main" when
/// chosen. `modules/options.nix` carries the same list as the option's
/// default.
pub const DEFAULT_SERVERS: [&str; 4] = [
    "https://zec-node.cakewallet.com:443",
    "https://na.zec.rocks:443",
    "https://lwd.z0n.jp:443",
    "https://zec.0xrpc.io:443",
];

/// How far back a pass scans at most, about 42 hours of blocks. An order
/// whose address is older keeps the state it last had.
pub const MAX_SCAN_BLOCKS: u64 = 2_000;

/// The network the edge settles Zcash on. Mainnet unless an option names a
/// test network, which only a test edge takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    Main,
    Test,
    Regtest,
}

impl Network {
    pub fn parse(text: &str) -> Option<Network> {
        match text {
            "main" | "mainnet" => Some(Network::Main),
            "test" | "testnet" => Some(Network::Test),
            "regtest" => Some(Network::Regtest),
            _ => None,
        }
    }

    /// The `chainName` a lightwalletd server reports for this network.
    pub fn chain_name(self) -> &'static str {
        match self {
            Network::Main => "main",
            Network::Test => "test",
            Network::Regtest => "regtest",
        }
    }

    pub fn network_type(self) -> NetworkType {
        match self {
            Network::Main => NetworkType::Main,
            Network::Test => NetworkType::Test,
            Network::Regtest => NetworkType::Regtest,
        }
    }
}

/// Whether an edge whose Stripe key is in test mode (`test`) may settle on
/// `network`: a live edge on mainnet only, a test edge never on mainnet, so
/// test orders cannot take real coins.
pub fn network_fits(network: Network, test: bool) -> bool {
    if test {
        network != Network::Main
    } else {
        network == Network::Main
    }
}

/// The servers the edge asks and the network it expects them on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZecOpts {
    /// lightwalletd-protocol gRPC endpoints, `https://host:port`. Plain http
    /// only to 127.0.0.1, which tests use. At least two operators.
    pub servers: Vec<String>,
    pub network: Network,
}

// ── Keys and addresses ────────────────────────────────────────────────────

/// A seller's key as the edge keeps it: the incoming viewing key of the
/// Orchard part, and the network it was made for.
#[derive(Clone, Debug)]
pub struct ViewingKey {
    pub network: NetworkType,
    ivk: IncomingViewingKey,
}

/// What a spending key or a seed is refused with: whoever holds one can take
/// the seller's coins, so it is named as such.
const SPENDING: &str = "that is a spending key; never send it to anyone. Send the unified viewing key (uview… or uivk…)";
const SEED: &str =
    "that looks like a seed phrase; never send it to anyone. Send the unified viewing key (uview… or uivk…)";

/// Prefixes of encoded Zcash spending keys: Sapling's extended and plain
/// spending keys on every network.
const SECRET_PREFIXES: [&str; 2] = ["secret-", "zxsk"];

/// Why `text` is plainly not a viewing key a seller should send, before it
/// is decoded: a spending key, a seed phrase, a transparent private key.
pub fn refuse_secret(text: &str) -> Result<(), &'static str> {
    let lower = text.trim().to_ascii_lowercase();
    if SECRET_PREFIXES.iter().any(|p| lower.starts_with(p)) {
        return Err(SPENDING);
    }
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() >= 12
        && words
            .iter()
            .all(|w| w.chars().all(|c| c.is_ascii_lowercase()))
    {
        return Err(SEED);
    }
    if is_wif(text.trim()) {
        return Err(SPENDING);
    }
    Ok(())
}

/// A transparent private key in wallet import format: base58check of 0x80
/// (mainnet) or 0xef (testnet) and 32 key bytes, with or without the
/// compression flag.
fn is_wif(text: &str) -> bool {
    let Some(raw) = crate::solana::base58_decode(text) else {
        return false;
    };
    if !(raw.len() == 37 || raw.len() == 38) {
        return false;
    }
    let (body, check) = raw.split_at(raw.len() - 4);
    let once = ring::digest::digest(&ring::digest::SHA256, body);
    let twice = ring::digest::digest(&ring::digest::SHA256, once.as_ref());
    twice.as_ref()[..4] == *check && matches!(body[0], 0x80 | 0xef)
}

/// The seller's key in `text`, or why it is not one the edge can watch.
pub fn parse_key(text: &str) -> Result<ViewingKey, &'static str> {
    let text = text.trim();
    if text.is_empty() || text.len() > 2_000 {
        return Err("that is not a Zcash unified viewing key");
    }
    refuse_secret(text)?;
    if text.starts_with("zxview") {
        return Err("that is a Sapling viewing key; send the unified viewing key (uview…), whose Orchard part orders are paid to");
    }
    if let Ok((network, ufvk)) = unified::Ufvk::decode(text) {
        let bytes = ufvk
            .items()
            .into_iter()
            .find_map(|i| match i {
                unified::Fvk::Orchard(b) => Some(b),
                _ => None,
            })
            .ok_or(
                "that viewing key has no Orchard part; export it from a wallet made after 2022",
            )?;
        let fvk = FullViewingKey::from_bytes(&bytes).ok_or("that viewing key is damaged")?;
        return Ok(ViewingKey {
            network,
            ivk: fvk.to_ivk(Scope::External),
        });
    }
    if let Ok((network, uivk)) = unified::Uivk::decode(text) {
        let bytes = uivk
            .items()
            .into_iter()
            .find_map(|i| match i {
                unified::Ivk::Orchard(b) => Some(b),
                _ => None,
            })
            .ok_or(
                "that viewing key has no Orchard part; export it from a wallet made after 2022",
            )?;
        let ivk = Option::<IncomingViewingKey>::from(IncomingViewingKey::from_bytes(&bytes))
            .ok_or("that viewing key is damaged")?;
        return Ok(ViewingKey { network, ivk });
    }
    if unified::Address::decode(text).is_ok() {
        return Err("that is an address; send the wallet's unified viewing key (uview… or uivk…)");
    }
    Err("that is not a Zcash unified viewing key")
}

/// The diversifier index of an order's address: `n` in the low bytes and a
/// 1 in the top byte, so 2^80 + n. Wallets hand out their own addresses from
/// index 0 upwards, one at a time; this range is far above anything they
/// reach, so an address a wallet showed someone is never an order's.
fn order_index(n: u32) -> DiversifierIndex {
    let mut bytes = [0u8; 11];
    bytes[..4].copy_from_slice(&n.to_le_bytes());
    bytes[10] = 1;
    DiversifierIndex::from(bytes)
}

fn encode_address(address: &orchard::Address, network: NetworkType) -> String {
    let receiver = unified::Receiver::Orchard(address.to_raw_address_bytes());
    unified::Address::try_from_items(vec![receiver])
        .expect("one Orchard receiver is a valid unified address")
        .encode(&network)
}

impl ViewingKey {
    /// The key as the edge stores it: an incoming viewing key with the
    /// Orchard part alone (`uivk…`), whatever the seller pasted.
    pub fn encode(&self) -> String {
        unified::Uivk::try_from_items(vec![unified::Ivk::Orchard(self.ivk.to_bytes())])
            .expect("one Orchard item is a valid unified viewing key")
            .encode(&self.network)
    }

    /// The Orchard-only unified address of order number `n`.
    pub fn order_address(&self, n: u32) -> String {
        encode_address(&self.ivk.address_at(order_index(n)), self.network)
    }

    /// The wallet's own first address, diversifier index 0, shown back to
    /// the seller so they can tell which wallet the edge pays to.
    pub fn wallet_address(&self) -> String {
        encode_address(&self.ivk.address_at(0u32), self.network)
    }

    /// Order number `n`'s, if `address` is one of this key's order addresses.
    fn order_of(&self, address: &orchard::Address) -> Option<u32> {
        let j = self.ivk.diversifier_index(address)?;
        let bytes = j.as_bytes();
        if bytes[10] != 1 || bytes[4..10].iter().any(|b| *b != 0) {
            return None;
        }
        Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

// ── The lightwalletd protocol, as far as the edge reads it ────────────────

/// The messages of lightwalletd's `service.proto` and
/// `compact_formats.proto` the edge sends or reads; fields it does not use
/// are skipped on the wire.
pub mod proto {
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct Empty {}

    #[derive(Clone, PartialEq, prost::Message)]
    pub struct ChainSpec {}

    #[derive(Clone, PartialEq, prost::Message)]
    pub struct BlockId {
        #[prost(uint64, tag = "1")]
        pub height: u64,
        #[prost(bytes = "vec", tag = "2")]
        pub hash: Vec<u8>,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub struct BlockRange {
        #[prost(message, optional, tag = "1")]
        pub start: Option<BlockId>,
        #[prost(message, optional, tag = "2")]
        pub end: Option<BlockId>,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub struct LightdInfo {
        #[prost(string, tag = "1")]
        pub version: String,
        #[prost(string, tag = "2")]
        pub vendor: String,
        #[prost(string, tag = "4")]
        pub chain_name: String,
        #[prost(uint64, tag = "7")]
        pub block_height: u64,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub struct GetMempoolTxRequest {
        #[prost(bytes = "vec", repeated, tag = "1")]
        pub exclude_txid_suffixes: Vec<Vec<u8>>,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub struct CompactBlock {
        #[prost(uint64, tag = "2")]
        pub height: u64,
        #[prost(bytes = "vec", tag = "3")]
        pub hash: Vec<u8>,
        #[prost(bytes = "vec", tag = "4")]
        pub prev_hash: Vec<u8>,
        #[prost(uint32, tag = "5")]
        pub time: u32,
        #[prost(message, repeated, tag = "7")]
        pub vtx: Vec<CompactTx>,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub struct CompactTx {
        #[prost(uint64, tag = "1")]
        pub index: u64,
        /// The txid in protocol order (the reverse of how it is shown).
        #[prost(bytes = "vec", tag = "2")]
        pub txid: Vec<u8>,
        #[prost(message, repeated, tag = "6")]
        pub actions: Vec<CompactOrchardAction>,
        /// NU6.3 onward: the Ironwood pool's actions, Orchard-shaped.
        #[prost(message, repeated, tag = "9")]
        pub ironwood_actions: Vec<CompactOrchardAction>,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub struct CompactOrchardAction {
        #[prost(bytes = "vec", tag = "1")]
        pub nullifier: Vec<u8>,
        #[prost(bytes = "vec", tag = "2")]
        pub cmx: Vec<u8>,
        #[prost(bytes = "vec", tag = "3")]
        pub ephemeral_key: Vec<u8>,
        #[prost(bytes = "vec", tag = "4")]
        pub ciphertext: Vec<u8>,
    }

    /// The service every lightwalletd-protocol server answers.
    pub const SERVICE: &str = "cash.z.wallet.sdk.rpc.CompactTxStreamer";
}

/// The registrable part of a server's host, which stands for its operator:
/// `na.zec.rocks` and `eu.zec.rocks` are one operator. An IP address is its
/// own operator with its port, which is what a test's local servers are.
pub fn operator(url: &str) -> String {
    let rest = url
        .split_once("://")
        .map_or(url, |(_, r)| r)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("");
    if rest.starts_with('[') {
        return rest.to_ascii_lowercase();
    }
    let host = rest.rsplit_once(':').map_or(rest, |(h, _)| h);
    if host.parse::<std::net::Ipv4Addr>().is_ok() {
        return rest.to_ascii_lowercase();
    }
    let labels: Vec<&str> = host.trim_end_matches('.').split('.').collect();
    let n = labels.len();
    labels[n.saturating_sub(2)..].join(".").to_ascii_lowercase()
}

/// Whether `servers` name at least two operators.
pub fn operators_enough(servers: &[String]) -> bool {
    servers
        .iter()
        .map(|s| operator(s))
        .collect::<BTreeSet<_>>()
        .len()
        >= 2
}

/// One lightwalletd server.
#[derive(Clone)]
pub struct Server {
    url: String,
    channel: tonic::transport::Channel,
}

/// A server's answers, kept apart from the others' until they are compared.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct View {
    pub operator: String,
    /// The best block the server knows.
    pub tip: u64,
    /// The block hash it gave at each height it served.
    pub hashes: BTreeMap<u64, Vec<u8>>,
    /// What it showed paying the watched orders.
    pub found: Vec<Found>,
}

/// One payment to one order's address, as one server showed it: every
/// output of one transaction to that address, added up.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Found {
    pub order: String,
    /// The txid as wallets and explorers show it.
    pub tx: String,
    pub amount: u64,
    /// The block it is in, `None` while in the mempool.
    pub height: Option<u64>,
    /// The block's time; 0 in the mempool.
    pub time: u64,
}

const GRPC_TIMEOUT: Duration = Duration::from_secs(60);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// A compact block of a full block of actions runs to a few megabytes.
const MAX_MESSAGE: usize = 64 * 1024 * 1024;

fn unreachable_err() -> MarketError {
    MarketError::Conflict("a Zcash server is not answering")
}

impl Server {
    /// A server at `url`, connected on first use. https is checked against
    /// the webpki roots, as everything else the registrar reaches.
    pub fn new(url: &str) -> Result<Server, MarketError> {
        if !crate::stablecoin::rpc_url_ok(url) {
            return Err(MarketError::Unconfigured);
        }
        let mut endpoint = tonic::transport::Endpoint::from_shared(url.to_string())
            .map_err(|_| MarketError::Unconfigured)?
            .timeout(GRPC_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT);
        if url.starts_with("https://") {
            endpoint = endpoint
                .tls_config(tonic::transport::ClientTlsConfig::new().with_webpki_roots())
                .map_err(|_| MarketError::Unconfigured)?;
        }
        Ok(Server::with_channel(url, endpoint.connect_lazy()))
    }

    /// A server reached over a channel the caller built, for a connection
    /// the registrar does not make itself (a proxy, in a probe).
    pub fn with_channel(url: &str, channel: tonic::transport::Channel) -> Server {
        Server {
            url: url.to_string(),
            channel,
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    async fn grpc(&self) -> Result<tonic::client::Grpc<tonic::transport::Channel>, MarketError> {
        let mut grpc =
            tonic::client::Grpc::new(self.channel.clone()).max_decoding_message_size(MAX_MESSAGE);
        grpc.ready().await.map_err(|e| {
            tracing::warn!("zcash server {}: {e}", self.url);
            unreachable_err()
        })?;
        Ok(grpc)
    }

    fn path(method: &str) -> Result<tonic::codegen::http::uri::PathAndQuery, MarketError> {
        format!("/{}/{method}", proto::SERVICE)
            .parse()
            .map_err(|_| MarketError::Unconfigured)
    }

    fn status(&self, method: &str, e: &tonic::Status) -> MarketError {
        tracing::warn!("zcash server {} {method}: {e}", self.url);
        unreachable_err()
    }

    /// The network the server follows (`main`, `test`, `regtest`) and its
    /// best block.
    pub async fn info(&self) -> Result<(String, u64), MarketError> {
        let mut grpc = self.grpc().await?;
        let reply: tonic::Response<proto::LightdInfo> = grpc
            .unary(
                tonic::Request::new(proto::Empty {}),
                Self::path("GetLightdInfo")?,
                tonic_prost::ProstCodec::default(),
            )
            .await
            .map_err(|e| self.status("GetLightdInfo", &e))?;
        let info = reply.into_inner();
        // The latest block answers more exactly than the info's height,
        // which some servers cache.
        let mut grpc = self.grpc().await?;
        let latest: tonic::Response<proto::BlockId> = grpc
            .unary(
                tonic::Request::new(proto::ChainSpec {}),
                Self::path("GetLatestBlock")?,
                tonic_prost::ProstCodec::default(),
            )
            .await
            .map_err(|e| self.status("GetLatestBlock", &e))?;
        Ok((info.chain_name, latest.into_inner().height))
    }

    /// The compact blocks from `start` to `end`, both included.
    pub async fn blocks(
        &self,
        start: u64,
        end: u64,
    ) -> Result<Vec<proto::CompactBlock>, MarketError> {
        let mut grpc = self.grpc().await?;
        let range = proto::BlockRange {
            start: Some(proto::BlockId {
                height: start,
                hash: Vec::new(),
            }),
            end: Some(proto::BlockId {
                height: end,
                hash: Vec::new(),
            }),
        };
        let mut stream = grpc
            .server_streaming(
                tonic::Request::new(range),
                Self::path("GetBlockRange")?,
                tonic_prost::ProstCodec::<proto::BlockRange, proto::CompactBlock>::default(),
            )
            .await
            .map_err(|e| self.status("GetBlockRange", &e))?
            .into_inner();
        let mut out = Vec::new();
        while let Some(block) = stream
            .message()
            .await
            .map_err(|e| self.status("GetBlockRange", &e))?
        {
            out.push(block);
        }
        Ok(out)
    }

    /// The shielded parts of the transactions in the server's mempool.
    pub async fn mempool(&self) -> Result<Vec<proto::CompactTx>, MarketError> {
        let mut grpc = self.grpc().await?;
        let mut stream = grpc
            .server_streaming(
                tonic::Request::new(proto::GetMempoolTxRequest {
                    exclude_txid_suffixes: Vec::new(),
                }),
                Self::path("GetMempoolTx")?,
                tonic_prost::ProstCodec::<proto::GetMempoolTxRequest, proto::CompactTx>::default(),
            )
            .await
            .map_err(|e| self.status("GetMempoolTx", &e))?
            .into_inner();
        let mut out = Vec::new();
        while let Some(tx) = stream
            .message()
            .await
            .map_err(|e| self.status("GetMempoolTx", &e))?
        {
            out.push(tx);
        }
        Ok(out)
    }
}

// ── Trial decryption ──────────────────────────────────────────────────────

/// An order the edge is watching: its address is the order address number
/// `index` of `key`.
#[derive(Debug, Clone)]
pub struct Watch {
    pub order: String,
    pub key: ViewingKey,
    pub index: u32,
    /// The best block when the address was issued: payments count from here.
    pub since: u64,
}

/// The txid of a compact transaction as it is shown: protocol order reversed,
/// in hex.
pub fn txid_hex(txid: &[u8]) -> String {
    txid.iter().rev().map(|b| format!("{b:02x}")).collect()
}

fn compact_action(a: &proto::CompactOrchardAction) -> Option<CompactAction> {
    let nf: [u8; 32] = a.nullifier.as_slice().try_into().ok()?;
    let cmx: [u8; 32] = a.cmx.as_slice().try_into().ok()?;
    let epk: [u8; 32] = a.ephemeral_key.as_slice().try_into().ok()?;
    let ct: [u8; 52] = a.ciphertext.as_slice().try_into().ok()?;
    Some(CompactAction::from_parts(
        Option::from(orchard::note::Nullifier::from_bytes(&nf))?,
        Option::from(orchard::note::ExtractedNoteCommitment::from_bytes(&cmx))?,
        zcash_note_encryption::EphemeralKeyBytes(epk),
        ct,
    ))
}

/// Every payment in `txs` to a watched order's address, one per order and
/// transaction. Orchard actions are tried as Orchard notes, Ironwood actions
/// as Ironwood notes; a note of the wrong version for its pool never
/// decrypts.
pub fn decrypt(
    watches: &[Watch],
    txs: &[proto::CompactTx],
    height: Option<u64>,
    time: u64,
) -> Vec<Found> {
    // One prepared key per distinct key, and which orders it watches.
    let mut keys: Vec<(orchard::keys::PreparedIncomingViewingKey, &ViewingKey)> = Vec::new();
    for w in watches {
        if !keys.iter().any(|(_, k)| k.ivk == w.key.ivk) {
            keys.push((
                orchard::keys::PreparedIncomingViewingKey::new(&w.key.ivk),
                &w.key,
            ));
        }
    }
    let mut out: BTreeMap<(String, String), u64> = BTreeMap::new();
    for tx in txs {
        let pools = [(false, &tx.actions), (true, &tx.ironwood_actions)];
        for (ironwood, actions) in pools {
            for action in actions.iter().filter_map(compact_action) {
                for (prepared, key) in &keys {
                    let found = if ironwood {
                        zcash_note_encryption::try_compact_note_decryption(
                            &IronwoodDomain::for_compact_action(&action),
                            prepared,
                            &action,
                        )
                    } else {
                        zcash_note_encryption::try_compact_note_decryption(
                            &OrchardDomain::for_compact_action(&action),
                            prepared,
                            &action,
                        )
                    };
                    let Some((note, address)) = found else {
                        continue;
                    };
                    let Some(n) = key.order_of(&address) else {
                        continue;
                    };
                    for w in watches
                        .iter()
                        .filter(|w| w.index == n && w.key.ivk == key.ivk)
                    {
                        if height.is_some_and(|h| h < w.since) {
                            continue;
                        }
                        let slot = out
                            .entry((w.order.clone(), txid_hex(&tx.txid)))
                            .or_default();
                        *slot = slot.saturating_add(note.value().inner());
                    }
                }
            }
        }
    }
    out.into_iter()
        .filter(|(_, amount)| *amount > 0)
        .map(|((order, tx), amount)| Found {
            order,
            tx,
            amount,
            height,
            time,
        })
        .collect()
}

// ── Asking several servers, and agreeing ──────────────────────────────────

/// What servers of two operators agree on: the best block both have, and
/// the payments both showed, per order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agreed {
    pub tip: u64,
    pub received: BTreeMap<String, Vec<Received>>,
}

/// The best block at least two operators have reached: each operator's best
/// tip, and the second highest of those. `None` with fewer than two.
fn agreed_tip(views: &[View]) -> Option<u64> {
    let mut best: BTreeMap<&str, u64> = BTreeMap::new();
    for v in views {
        let t = best.entry(&v.operator).or_default();
        *t = (*t).max(v.tip);
    }
    let mut tips: Vec<u64> = best.into_values().collect();
    tips.sort_unstable_by(|a, b| b.cmp(a));
    tips.get(1).copied()
}

/// Compare the servers' answers. A mined payment counts when two operators
/// showed it in a block of the same hash at the same height, at or below
/// the agreed tip; a payment in the mempool counts as seen when two
/// operators showed it there and it is not counted from a block. A server
/// that showed something no other operator confirms is simply outvoted.
pub fn agree(views: &[View], now: u64) -> Option<Agreed> {
    let tip = agreed_tip(views)?;
    // Each operator's vote for a payment, keyed with the block hash it gave.
    let mut votes: BTreeMap<(Found, Vec<u8>), BTreeSet<&str>> = BTreeMap::new();
    for v in views {
        for f in &v.found {
            let hash = match f.height {
                Some(h) if h > tip => continue,
                Some(h) => match v.hashes.get(&h) {
                    Some(hash) => hash.clone(),
                    None => continue,
                },
                None => Vec::new(),
            };
            votes
                .entry((f.clone(), hash))
                .or_default()
                .insert(&v.operator);
        }
    }
    let agreed: Vec<&Found> = votes
        .iter()
        .filter(|(_, ops)| ops.len() >= 2)
        .map(|((f, _), _)| f)
        .collect();
    let mut received: BTreeMap<String, Vec<Received>> = BTreeMap::new();
    let mined: BTreeSet<(&str, &str)> = agreed
        .iter()
        .filter(|f| f.height.is_some())
        .map(|f| (f.order.as_str(), f.tx.as_str()))
        .collect();
    for f in &agreed {
        let r = match f.height {
            Some(h) => Received {
                tx: f.tx.clone(),
                amount: u128::from(f.amount),
                confirmations: tip.saturating_sub(h) + 1,
                first_seen: f.time,
            },
            None if mined.contains(&(f.order.as_str(), f.tx.as_str())) => continue,
            None => Received {
                tx: f.tx.clone(),
                amount: u128::from(f.amount),
                confirmations: 0,
                first_seen: now,
            },
        };
        let list = received.entry(f.order.clone()).or_default();
        // Two blocks of different hashes can both win a vote only if the
        // chain split between the servers; keep the deeper one.
        match list.iter_mut().find(|x| x.tx == r.tx) {
            Some(x) if x.confirmations < r.confirmations => *x = r,
            Some(_) => {}
            None => list.push(r),
        }
    }
    Some(Agreed { tip, received })
}

/// How many of the newest blocks every pass fetches again, though it read
/// them before: more than the most confirmations an order asks for, so a
/// block a reorg replaced is read again before a payment in it can count.
pub const RESCAN_DEPTH: u64 = 30;

/// What the edge remembers of one server between passes, so a pass fetches
/// only the blocks it has not read: the orders it was asked about, the best
/// block it had, the hashes it gave and the mined payments it showed.
#[derive(Default)]
struct Cached {
    server: Option<Server>,
    orders: BTreeSet<String>,
    top: Option<u64>,
    hashes: BTreeMap<u64, Vec<u8>>,
    found: Vec<Found>,
}

struct Remote {
    url: String,
    cache: tokio::sync::Mutex<Cached>,
}

/// The edge's Zcash watcher: the configured servers, each with what it read
/// of them before.
pub struct Scanner {
    opts: ZecOpts,
    remotes: Vec<std::sync::Arc<Remote>>,
}

impl Scanner {
    pub fn new(opts: ZecOpts) -> Scanner {
        let remotes = opts
            .servers
            .iter()
            .map(|url| {
                std::sync::Arc::new(Remote {
                    url: url.clone(),
                    cache: tokio::sync::Mutex::new(Cached::default()),
                })
            })
            .collect();
        Scanner { opts, remotes }
    }

    pub fn opts(&self) -> &ZecOpts {
        &self.opts
    }

    /// The best block servers of two operators have reached, after checking
    /// they follow the edge's network.
    pub async fn tip(&self) -> Result<u64, MarketError> {
        let mut set = tokio::task::JoinSet::new();
        for remote in &self.remotes {
            let (remote, network) = (std::sync::Arc::clone(remote), self.opts.network);
            set.spawn(async move {
                let server = remote.server().await?;
                let tip = checked_tip(&server, network).await?;
                Ok::<View, MarketError>(View {
                    operator: operator(&remote.url),
                    tip,
                    ..View::default()
                })
            });
        }
        let mut views = Vec::new();
        while let Some(joined) = set.join_next().await {
            if let Ok(Ok(view)) = joined {
                views.push(view);
            }
        }
        agreed_tip(&views).ok_or(TOO_FEW)
    }

    /// What reached each watched order's address, as two operators agree.
    /// `watches` should be every order the edge is waiting on, each pass:
    /// a server's memory is kept to them.
    pub async fn scan(&self, watches: Vec<Watch>, now: u64) -> Result<Agreed, MarketError> {
        let watches = std::sync::Arc::new(watches);
        let mut set = tokio::task::JoinSet::new();
        for remote in &self.remotes {
            set.spawn(view_of(
                std::sync::Arc::clone(remote),
                self.opts.network,
                std::sync::Arc::clone(&watches),
            ));
        }
        let mut views = Vec::new();
        while let Some(joined) = set.join_next().await {
            if let Ok(Ok(view)) = joined {
                views.push(view);
            }
        }
        agree(&views, now).ok_or(TOO_FEW)
    }
}

impl Remote {
    async fn server(&self) -> Result<Server, MarketError> {
        let mut cache = self.cache.lock().await;
        if cache.server.is_none() {
            cache.server = Some(Server::new(&self.url)?);
        }
        Ok(cache.server.clone().expect("just set"))
    }
}

/// The server's best block, once it has said it follows `network`. A server
/// on another chain is never asked anything else.
async fn checked_tip(server: &Server, network: Network) -> Result<u64, MarketError> {
    let (chain, tip) = server.info().await?;
    if chain != network.chain_name() {
        tracing::error!(
            "zcash server {} follows {chain:?}, not the {} this edge settles on; it is not asked",
            server.url(),
            network.chain_name()
        );
        return Err(MarketError::Conflict(
            "a Zcash server is on the wrong network",
        ));
    }
    Ok(tip)
}

/// One server's answers about `watches`: the blocks from the oldest order's
/// issue height (at most [`MAX_SCAN_BLOCKS`] back) to its best block, read
/// from what it served before where that is old enough to trust, and its
/// mempool.
async fn view_of(
    remote: std::sync::Arc<Remote>,
    network: Network,
    watches: std::sync::Arc<Vec<Watch>>,
) -> Result<View, MarketError> {
    let server = remote.server().await?;
    let tip = checked_tip(&server, network).await?;
    let mut cache = remote.cache.lock().await;
    let orders: BTreeSet<String> = watches.iter().map(|w| w.order.clone()).collect();
    let mut view = View {
        operator: operator(&remote.url),
        tip,
        ..View::default()
    };
    let Some(oldest) = watches.iter().map(|w| w.since).min() else {
        *cache = Cached {
            server: cache.server.take(),
            ..Cached::default()
        };
        return Ok(view);
    };
    let oldest = oldest.max(tip.saturating_sub(MAX_SCAN_BLOCKS));
    // Orders it was not asked about before are read from their own issue
    // height; the rest from a little below the best block it had.
    let newcomers = watches
        .iter()
        .filter(|w| !cache.orders.contains(&w.order))
        .map(|w| w.since)
        .min();
    let from = match cache.top {
        Some(top) => top
            .saturating_add(1)
            .saturating_sub(RESCAN_DEPTH)
            .min(newcomers.unwrap_or(u64::MAX)),
        None => oldest,
    }
    .max(oldest);
    if from <= tip {
        let blocks = server.blocks(from, tip).await?;
        let scan = std::sync::Arc::clone(&watches);
        let (hashes, found) = tokio::task::spawn_blocking(move || {
            let mut found = Vec::new();
            let mut hashes = BTreeMap::new();
            for b in blocks.iter().filter(|b| (from..=tip).contains(&b.height)) {
                hashes.insert(b.height, b.hash.clone());
                found.extend(decrypt(&scan, &b.vtx, Some(b.height), u64::from(b.time)));
            }
            (hashes, found)
        })
        .await
        .map_err(|_| MarketError::Store("zcash decryption task failed".to_string()))?;
        cache.hashes.retain(|h, _| *h < from);
        cache.found.retain(|f| f.height.is_some_and(|h| h < from));
        cache.hashes.extend(hashes);
        cache.found.extend(found);
    }
    // A server that went back (a reorg, or one behind its own earlier
    // answer) is believed from where it is now.
    cache.hashes.retain(|h, _| (oldest..=tip).contains(h));
    cache.found.retain(|f| {
        orders.contains(&f.order) && f.height.is_some_and(|h| (oldest..=tip).contains(&h))
    });
    cache.orders = orders;
    cache.top = Some(tip);
    view.hashes = cache.hashes.clone();
    view.found = cache.found.clone();
    drop(cache);
    match server.mempool().await {
        Ok(txs) => {
            let pooled = tokio::task::spawn_blocking(move || decrypt(&watches, &txs, None, 0))
                .await
                .map_err(|_| MarketError::Store("zcash decryption task failed".to_string()))?;
            view.found.extend(pooled);
        }
        Err(e) => tracing::debug!("zcash server {}: no mempool: {e}", remote.url),
    }
    Ok(view)
}

const TOO_FEW: MarketError =
    MarketError::Conflict("fewer than two Zcash servers of different operators are answering");

#[cfg(test)]
mod tests {
    use super::*;

    /// A key made for these tests from a made-up spending key; nobody's
    /// wallet holds it.
    fn key(seed: u8, network: NetworkType) -> (FullViewingKey, ViewingKey) {
        let sk = Option::<orchard::keys::SpendingKey>::from(
            orchard::keys::SpendingKey::from_bytes([seed; 32]),
        )
        .expect("a spending key");
        let fvk = FullViewingKey::from(&sk);
        let vk = ViewingKey {
            network,
            ivk: fvk.to_ivk(Scope::External),
        };
        (fvk, vk)
    }

    fn ufvk(fvk: &FullViewingKey, network: NetworkType) -> String {
        unified::Ufvk::try_from_items(vec![unified::Fvk::Orchard(fvk.to_bytes())])
            .unwrap()
            .encode(&network)
    }

    #[test]
    fn a_full_viewing_key_is_kept_as_its_incoming_part() {
        let (fvk, vk) = key(7, NetworkType::Main);
        let text = ufvk(&fvk, NetworkType::Main);
        assert!(text.starts_with("uview1"), "{text}");
        let parsed = parse_key(&text).expect("parses");
        assert_eq!(parsed.network, NetworkType::Main);
        let kept = parsed.encode();
        assert!(kept.starts_with("uivk1"), "{kept}");
        // The kept key reads back as the same key, and gives the same
        // addresses as the one the full key makes.
        let again = parse_key(&kept).expect("the kept key parses");
        assert_eq!(again.order_address(5), vk.order_address(5));
        assert_eq!(again.wallet_address(), vk.wallet_address());
    }

    #[test]
    fn order_addresses_are_the_sellers_and_their_own() {
        let (_, vk) = key(7, NetworkType::Main);
        let a = vk.order_address(0);
        let b = vk.order_address(1);
        assert!(a.starts_with("u1"), "{a}");
        assert_ne!(a, b);
        assert_ne!(a, vk.wallet_address());
        assert_eq!(vk.order_address(1), b);
        // Only order addresses map back to an order number.
        let addr = vk.ivk.address_at(order_index(41));
        assert_eq!(vk.order_of(&addr), Some(41));
        assert_eq!(vk.order_of(&vk.ivk.address_at(41u32)), None);
        let (_, other) = key(8, NetworkType::Main);
        assert_eq!(other.order_of(&addr), None);
        let (_, test) = key(7, NetworkType::Test);
        assert!(test.order_address(0).starts_with("utest1"));
    }

    #[test]
    fn spending_keys_seeds_and_addresses_are_refused_by_name() {
        // Made-up strings in the shape of secrets; none is a real key.
        for secret in [
            "secret-extended-key-main1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
            "secret-orchard-sk-main1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
            "zxsk1notarealkeyonlytheshapeofone",
        ] {
            assert_eq!(parse_key(secret).err(), Some(SPENDING), "{secret}");
        }
        let seed = ["abandon"; 23].join(" ") + " art";
        assert_eq!(parse_key(&seed).err(), Some(SEED));
        // A WIF private key: base58check of 0x80 and 32 bytes of 0x01.
        let mut raw = vec![0x80u8];
        raw.extend([1u8; 32]);
        raw.push(1);
        let once = ring::digest::digest(&ring::digest::SHA256, &raw);
        let twice = ring::digest::digest(&ring::digest::SHA256, once.as_ref());
        raw.extend_from_slice(&twice.as_ref()[..4]);
        let wif = crate::solana::base58_encode(&raw);
        assert_eq!(parse_key(&wif).err(), Some(SPENDING));
        let (_, vk) = key(7, NetworkType::Main);
        assert!(parse_key(&vk.order_address(0))
            .err()
            .unwrap()
            .contains("that is an address"));
        assert!(parse_key("zxviews1qqqq").err().unwrap().contains("Sapling"));
        assert!(parse_key("").is_err());
        assert!(parse_key("uview1garbage").is_err());
    }

    #[test]
    fn live_edges_settle_on_mainnet_and_test_edges_never_do() {
        assert!(network_fits(Network::Main, false));
        assert!(!network_fits(Network::Test, false));
        assert!(!network_fits(Network::Regtest, false));
        assert!(!network_fits(Network::Main, true));
        assert!(network_fits(Network::Test, true));
        assert!(network_fits(Network::Regtest, true));
        assert_eq!(Network::parse("mainnet"), Some(Network::Main));
        assert_eq!(Network::parse("main"), Some(Network::Main));
        assert_eq!(Network::parse("devnet"), None);
    }

    #[test]
    fn operators_are_registrable_domains() {
        assert_eq!(operator("https://na.zec.rocks:443"), "zec.rocks");
        assert_eq!(operator("https://eu.zec.rocks:443/"), "zec.rocks");
        assert_eq!(
            operator("https://zec-node.cakewallet.com:443"),
            "cakewallet.com"
        );
        assert_eq!(operator("http://127.0.0.1:9067"), "127.0.0.1:9067");
        let urls = |u: &[&str]| u.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(!operators_enough(&urls(&[
            "https://na.zec.rocks:443",
            "https://eu.zec.rocks:443"
        ])));
        assert!(operators_enough(&urls(&DEFAULT_SERVERS)));
        assert_eq!(
            DEFAULT_SERVERS
                .iter()
                .map(|s| operator(s))
                .collect::<BTreeSet<_>>()
                .len(),
            DEFAULT_SERVERS.len()
        );
    }

    fn found(order: &str, tx: &str, amount: u64, height: Option<u64>) -> Found {
        Found {
            order: order.to_string(),
            tx: tx.to_string(),
            amount,
            height,
            time: 1_000,
        }
    }

    fn view(op: &str, tip: u64, hashes: &[(u64, u8)], found: Vec<Found>) -> View {
        View {
            operator: op.to_string(),
            tip,
            hashes: hashes.iter().map(|(h, b)| (*h, vec![*b; 32])).collect(),
            found,
        }
    }

    #[test]
    fn a_payment_counts_only_when_two_operators_agree() {
        let paid = found("o1", "aa", 500, Some(100));
        let honest_a = view("a", 105, &[(100, 1)], vec![paid.clone()]);
        let honest_b = view("b", 104, &[(100, 1)], vec![paid.clone()]);
        let got = agree(&[honest_a.clone(), honest_b.clone()], 2_000).expect("two operators");
        assert_eq!(got.tip, 104);
        assert_eq!(got.received["o1"][0].confirmations, 5);
        assert_eq!(got.received["o1"][0].amount, 500);

        // A liar alone is outvoted, whatever it shows.
        let liar = view(
            "c",
            200,
            &[(100, 1), (150, 9)],
            vec![
                paid.clone(),
                found("o1", "forged", 9_999, Some(150)),
                found("o2", "forged", 1, None),
            ],
        );
        let got = agree(&[honest_a.clone(), honest_b.clone(), liar.clone()], 2_000).unwrap();
        assert_eq!(got.tip, 105);
        assert_eq!(got.received["o1"].len(), 1);
        assert!(!got.received.contains_key("o2"));

        // Two servers of one operator are one vote.
        let same_op = view("a", 105, &[(100, 1)], vec![paid.clone()]);
        assert!(agree(&[honest_a.clone(), same_op], 2_000).is_none());

        // The same payment in blocks of different hashes is not agreed on.
        let fork = view("b", 104, &[(100, 2)], vec![paid.clone()]);
        let got = agree(&[honest_a.clone(), fork], 2_000).unwrap();
        assert!(got.received.is_empty());

        // A different amount in the same block is not the same payment.
        let other_amount = view(
            "b",
            104,
            &[(100, 1)],
            vec![found("o1", "aa", 400, Some(100))],
        );
        assert!(agree(&[honest_a, other_amount], 2_000)
            .unwrap()
            .received
            .is_empty());
    }

    #[test]
    fn a_mempool_payment_is_seen_once_two_operators_show_it() {
        let pending = found("o1", "bb", 700, None);
        let a = view("a", 10, &[], vec![pending.clone()]);
        let b = view("b", 10, &[], vec![pending.clone()]);
        let got = agree(&[a.clone(), b], 5_000).unwrap();
        let r = &got.received["o1"][0];
        assert_eq!((r.confirmations, r.first_seen), (0, 5_000));
        // Mined on one, still pooled on the other: the block does not count
        // alone, the pool still shows it.
        let mined = view("b", 11, &[(11, 3)], vec![found("o1", "bb", 700, Some(11))]);
        let got = agree(&[a, mined], 5_000).unwrap();
        assert!(got.received.is_empty());
    }

    #[test]
    fn txids_show_reversed() {
        assert_eq!(txid_hex(&[0x01, 0x02, 0xab]), "ab0201");
    }
}
