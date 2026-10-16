//! Bitcoin orders, through the edge's own `bitcoind`.
//!
//! The seller hands the edge a watch-only key: an extended public key or an
//! output descriptor. For each order the edge derives the next receive
//! address from it, imports that one address into a watch-only wallet on its
//! node and reads back what arrived there. The edge never holds a private
//! key, and the coins go straight to the seller's wallet; the buyer never
//! has to report a transaction.
//!
//! The node is reached over JSON-RPC with its cookie, read on every call
//! because `bitcoind` writes a new one each time it starts.

use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};

use crate::coins::Received;
use crate::market::MarketError;

/// Where the edge's node listens and how it signs in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtcOpts {
    /// `http://127.0.0.1:8332`, or https. Same rule as a USDC endpoint.
    pub rpc: String,
    /// The node's `.cookie`, `__cookie__:<password>`.
    pub cookie_file: String,
    /// The watch-only wallet the order addresses are imported into. Made on
    /// first use, with private keys disabled.
    pub wallet: String,
}

/// The wallet name when none is given.
pub const DEFAULT_WALLET: &str = "losos-watch";

/// The chains a test edge may watch, and the one a live edge must.
pub fn chain_fits(chain: &str, test: bool) -> bool {
    if test {
        matches!(chain, "test" | "testnet4" | "signet" | "regtest")
    } else {
        chain == "main"
    }
}

// ── Keys ──────────────────────────────────────────────────────────────────

const XPUB: [u8; 4] = [0x04, 0x88, 0xb2, 0x1e];
const TPUB: [u8; 4] = [0x04, 0x35, 0x87, 0xcf];

/// The extended-key prefixes wallets export, the version `bitcoind` reads in
/// their place, and the script they stand for. An xpub or tpub is read as
/// native SegWit, which is what wallets make by default now.
const PREFIXES: [([u8; 4], [u8; 4], &str); 6] = [
    (XPUB, XPUB, "wpkh"),
    (TPUB, TPUB, "wpkh"),
    ([0x04, 0xb2, 0x47, 0x46], XPUB, "wpkh"),    // zpub
    ([0x04, 0x5f, 0x1c, 0xf6], TPUB, "wpkh"),    // vpub
    ([0x04, 0x9d, 0x7c, 0xb2], XPUB, "sh-wpkh"), // ypub
    ([0x04, 0x4a, 0x52, 0x62], TPUB, "sh-wpkh"), // upub
];

/// Descriptor functions a seller may paste whole.
const FUNCTIONS: [&str; 5] = ["wpkh(", "sh(wpkh(", "tr(", "pkh(", "wsh("];

fn sha256d(bytes: &[u8]) -> [u8; 32] {
    let once = ring::digest::digest(&ring::digest::SHA256, bytes);
    let twice = ring::digest::digest(&ring::digest::SHA256, once.as_ref());
    let mut out = [0u8; 32];
    out.copy_from_slice(twice.as_ref());
    out
}

fn base58check_decode(text: &str) -> Option<Vec<u8>> {
    let raw = crate::solana::base58_decode(text)?;
    if raw.len() < 5 {
        return None;
    }
    let (body, check) = raw.split_at(raw.len() - 4);
    (sha256d(body)[..4] == *check).then(|| body.to_vec())
}

fn base58check_encode(body: &[u8]) -> String {
    let mut raw = body.to_vec();
    raw.extend_from_slice(&sha256d(body)[..4]);
    crate::solana::base58_encode(&raw)
}

/// The ranged descriptor for what the seller pasted, before the node checks
/// it: a descriptor as given (without its checksum), or the receive chain of
/// an extended public key. `None` for anything else, a private key included.
pub fn descriptor_for(key: &str) -> Option<String> {
    let key = key.trim();
    if key.is_empty() || key.len() > 1024 || key.chars().any(|c| c.is_whitespace()) {
        return None;
    }
    if key.contains("prv") {
        return None;
    }
    if FUNCTIONS.iter().any(|f| key.starts_with(f)) {
        let body = key.split('#').next()?;
        return body.contains("/*").then(|| body.to_string());
    }
    let raw = base58check_decode(key)?;
    // version 4, depth 1, fingerprint 4, child 4, chain code 32, key 33.
    if raw.len() != 78 {
        return None;
    }
    let (_, as_read, script) = PREFIXES.iter().find(|(v, _, _)| raw[..4] == *v)?;
    let mut body = raw;
    body[..4].copy_from_slice(as_read);
    let xpub = base58check_encode(&body);
    Some(match *script {
        "sh-wpkh" => format!("sh(wpkh({xpub}/0/*))"),
        _ => format!("wpkh({xpub}/0/*)"),
    })
}

// ── The node ──────────────────────────────────────────────────────────────

/// A JSON-RPC error from the node, with its code.
#[derive(Debug)]
struct RpcError {
    code: i64,
    message: String,
}

pub struct Node<'a> {
    opts: &'a BtcOpts,
}

/// Money is counted in satoshis; the node answers in whole BTC with eight
/// decimals, well inside an f64's exact range for any real amount.
fn sats(value: &Value) -> Option<u128> {
    let btc = value.as_f64()?;
    if !(0.0..=21e6).contains(&btc) {
        return None;
    }
    Some((btc * 1e8).round() as u128)
}

impl<'a> Node<'a> {
    pub fn new(opts: &'a BtcOpts) -> Self {
        Node { opts }
    }

    async fn cookie(&self) -> Result<(String, String), MarketError> {
        let raw = tokio::fs::read_to_string(Path::new(&self.opts.cookie_file))
            .await
            .map_err(|_| MarketError::Unconfigured)?;
        let (user, pass) = raw
            .trim()
            .split_once(':')
            .ok_or(MarketError::Unconfigured)?;
        Ok((user.to_string(), pass.to_string()))
    }

    async fn call_at(
        &self,
        wallet: bool,
        method: &str,
        params: Value,
    ) -> Result<Result<Value, RpcError>, MarketError> {
        if !crate::stablecoin::rpc_url_ok(&self.opts.rpc) {
            return Err(MarketError::Unconfigured);
        }
        let (user, pass) = self.cookie().await?;
        let base = self.opts.rpc.trim_end_matches('/');
        let url = if wallet {
            format!("{base}/wallet/{}", self.opts.wallet)
        } else {
            base.to_string()
        };
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|_| MarketError::Unconfigured)?;
        let reply = client
            .post(url)
            .basic_auth(user, Some(pass))
            .json(&json!({"jsonrpc": "1.0", "id": "losos", "method": method, "params": params}))
            .send()
            .await
            .map_err(|_| MarketError::Conflict("the edge's Bitcoin node is not answering"))?;
        // bitcoind answers an RPC error with a 4xx or 5xx and the error in
        // the body, so the body is read whatever the status.
        let body: Value = reply
            .json()
            .await
            .map_err(|_| MarketError::Conflict("the edge's Bitcoin node is not answering"))?;
        if let Some(err) = body.get("error").filter(|e| !e.is_null()) {
            return Ok(Err(RpcError {
                code: err.get("code").and_then(Value::as_i64).unwrap_or(0),
                message: err
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            }));
        }
        Ok(Ok(body.get("result").cloned().unwrap_or(Value::Null)))
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value, MarketError> {
        self.call_at(false, method, params).await?.map_err(|e| {
            tracing::warn!("bitcoind {method}: {} ({})", e.message, e.code);
            MarketError::Conflict("the edge's Bitcoin node refused the request")
        })
    }

    async fn wallet_call(&self, method: &str, params: Value) -> Result<Value, MarketError> {
        match self.call_at(true, method, params.clone()).await? {
            Ok(v) => Ok(v),
            // -18: the wallet is not loaded (after a restart) or not made yet.
            Err(e) if e.code == -18 => {
                self.ensure_wallet().await?;
                self.call_at(true, method, params).await?.map_err(|e| {
                    tracing::warn!("bitcoind {method}: {} ({})", e.message, e.code);
                    MarketError::Conflict("the edge's Bitcoin node refused the request")
                })
            }
            Err(e) => {
                tracing::warn!("bitcoind {method}: {} ({})", e.message, e.code);
                Err(MarketError::Conflict(
                    "the edge's Bitcoin node refused the request",
                ))
            }
        }
    }

    /// Load the watch wallet, or make it: blank, descriptor-based, with
    /// private keys disabled, loaded again whenever the node starts.
    async fn ensure_wallet(&self) -> Result<(), MarketError> {
        let name = json!(self.opts.wallet);
        match self.call_at(false, "loadwallet", json!([name])).await? {
            Ok(_) => return Ok(()),
            // -35: already loaded.
            Err(e) if e.code == -35 => return Ok(()),
            Err(_) => {}
        }
        self.call(
            "createwallet",
            json!([name, true, true, "", false, true, true]),
        )
        .await
        .map(|_| ())
    }

    /// Which chain the node follows: `main`, `test`, `testnet4`, `signet` or
    /// `regtest`.
    pub async fn chain(&self) -> Result<String, MarketError> {
        let info = self.call("getblockchaininfo", json!([])).await?;
        info.get("chain")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or(MarketError::Conflict(
                "the edge's Bitcoin node gave no chain",
            ))
    }

    /// The node's checked form of a seller's key: the descriptor with its
    /// checksum. Refused when it is not ranged, carries a private key, or
    /// names another network's keys (the node will not parse those).
    pub async fn check_key(&self, key: &str) -> Result<String, MarketError> {
        let desc = descriptor_for(key).ok_or(MarketError::Invalid(
            "that is not an extended public key or a ranged descriptor",
        ))?;
        let info = match self
            .call_at(false, "getdescriptorinfo", json!([desc]))
            .await?
        {
            Ok(info) => info,
            Err(_) => {
                return Err(MarketError::Invalid(
                    "that key is not for the network this edge uses",
                ))
            }
        };
        if info.get("hasprivatekeys").and_then(Value::as_bool) != Some(false) {
            return Err(MarketError::Invalid(
                "send the public key, never a private one",
            ));
        }
        if info.get("isrange").and_then(Value::as_bool) != Some(true) {
            return Err(MarketError::Invalid(
                "the descriptor must be ranged, ending in /*",
            ));
        }
        info.get("descriptor")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or(MarketError::Conflict(
                "the edge's Bitcoin node gave no descriptor",
            ))
    }

    /// The address at `index` of a checked descriptor.
    pub async fn derive(&self, descriptor: &str, index: u32) -> Result<String, MarketError> {
        let found = self
            .call("deriveaddresses", json!([descriptor, [index, index]]))
            .await?;
        found
            .get(0)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or(MarketError::Conflict(
                "the edge's Bitcoin node derived no address",
            ))
    }

    /// Watch `address` from now on, labelled with the order.
    pub async fn watch(&self, address: &str, label: &str) -> Result<(), MarketError> {
        let info = self
            .call("getdescriptorinfo", json!([format!("addr({address})")]))
            .await?;
        let desc = info
            .get("descriptor")
            .and_then(Value::as_str)
            .ok_or(MarketError::Conflict(
                "the edge's Bitcoin node gave no descriptor",
            ))?;
        let reply = self
            .wallet_call(
                "importdescriptors",
                json!([[{"desc": desc, "timestamp": "now", "label": label}]]),
            )
            .await?;
        let ok = reply
            .get(0)
            .and_then(|r| r.get("success"))
            .and_then(Value::as_bool)
            == Some(true);
        if ok {
            Ok(())
        } else {
            Err(MarketError::Conflict(
                "the edge's Bitcoin node would not watch the address",
            ))
        }
    }

    /// Every payment `address` received, mempool included. A transaction the
    /// chain dropped for a conflicting one is left out.
    pub async fn received(&self, address: &str) -> Result<Vec<Received>, MarketError> {
        let rows = self
            .wallet_call("listreceivedbyaddress", json!([0, true, true, address]))
            .await?;
        let mut txids: Vec<String> = rows
            .as_array()
            .into_iter()
            .flatten()
            .filter(|r| r.get("address").and_then(Value::as_str) == Some(address))
            .flat_map(|r| {
                r.get("txids")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
            })
            .filter_map(|t| t.as_str().map(str::to_string))
            .collect();
        txids.sort();
        txids.dedup();
        let mut out = Vec::new();
        for txid in txids {
            let tx = self
                .wallet_call("gettransaction", json!([txid, true]))
                .await?;
            let confirmations = tx.get("confirmations").and_then(Value::as_i64).unwrap_or(0);
            if confirmations < 0 {
                continue;
            }
            let amount: u128 = tx
                .get("details")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|d| d.get("address").and_then(Value::as_str) == Some(address))
                .filter(|d| d.get("category").and_then(Value::as_str) == Some("receive"))
                .filter_map(|d| d.get("amount").and_then(sats))
                .sum();
            if amount == 0 {
                continue;
            }
            out.push(Received {
                tx: txid,
                amount,
                confirmations: confirmations as u64,
                first_seen: tx.get("timereceived").and_then(Value::as_u64).unwrap_or(0),
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // BIP 84's test vector account: m/84'/0'/0' of "abandon abandon ...
    // about", as a zpub and as the xpub bitcoind reads.
    const ZPUB: &str = "zpub6rFR7y4Q2AijBEqTUquhVz398htDFrtymD9xYYfG1m4wAcvPhXNfE3EfH1r1ADqtfSdVCToUG868RvUUkgDKf31mGDtKsAYz2oz2AGutZYs";
    const XPUB_84: &str = "xpub6CatWdiZiodmUeTDp8LT5or8nmbKNcuyvz7WyksVFkKB4RHwCD3XyuvPEbvqAQY3rAPshWcMLoP2fMFMKHPJ4ZeZXYVUhLv1VMrjPC7PW6V";

    #[test]
    fn a_zpub_reads_as_the_xpub_of_native_segwit() {
        assert_eq!(descriptor_for(ZPUB), Some(format!("wpkh({XPUB_84}/0/*)")));
        assert_eq!(
            descriptor_for(XPUB_84),
            Some(format!("wpkh({XPUB_84}/0/*)"))
        );
    }

    #[test]
    fn a_descriptor_passes_without_its_checksum() {
        let d = format!("wpkh([73c5da0a/84h/0h/0h]{XPUB_84}/0/*)");
        assert_eq!(descriptor_for(&format!("{d}#abcdefgh")), Some(d.clone()));
        // Not ranged: one address, not one per order.
        assert_eq!(descriptor_for(&format!("wpkh({XPUB_84}/0/0)")), None);
    }

    #[test]
    fn private_keys_and_noise_are_refused() {
        assert_eq!(descriptor_for("xprvNotARealKeyOnlyTheShapeOfOneNotARealKeyOnlyTheShapeOfOneNotARealKeyOnlyTheShapeOfOneNotARealKeyOnlyTheShapeOfOne"), None);
        assert_eq!(descriptor_for("wpkh(tprv8ZgxMBicQKsPd/0/*)"), None);
        assert_eq!(
            descriptor_for("bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu"),
            None
        );
        assert_eq!(descriptor_for(""), None);
        assert_eq!(descriptor_for("wpkh(a b/0/*)"), None);
        // A flipped character breaks the checksum.
        let mut broken = ZPUB.to_string();
        broken.replace_range(20..21, "X");
        assert_eq!(descriptor_for(&broken), None);
    }

    #[test]
    fn test_edges_watch_test_chains_and_live_edges_main() {
        for chain in ["test", "testnet4", "signet", "regtest"] {
            assert!(chain_fits(chain, true));
            assert!(!chain_fits(chain, false));
        }
        assert!(chain_fits("main", false));
        assert!(!chain_fits("main", true));
    }

    #[test]
    fn amounts_turn_into_satoshis() {
        assert_eq!(sats(&json!(0.00012345)), Some(12_345));
        assert_eq!(sats(&json!(1e-8)), Some(1));
        assert_eq!(sats(&json!(20999999.9769)), Some(2_099_999_997_690_000));
        assert_eq!(sats(&json!(-0.1)), None);
    }
}
