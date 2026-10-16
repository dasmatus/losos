//! Monero orders, through the edge's own `monerod` and `monero-wallet-rpc`.
//!
//! The seller hands the edge their primary address and private view key.
//! The view key lets a wallet see what arrives, never spend it. The edge
//! keeps one view-only wallet per seller in its wallet service and gives
//! each order the next subaddress of the seller's first account, so the
//! coins land in the seller's own wallet and the buyer never has to report
//! a transaction.
//!
//! The wallet service holds one wallet open at a time, so every call that
//! opens one runs under [`WALLET`]. It signs in with HTTP digest, from a
//! `user:password` file the unit writes on its first start.

use std::time::Duration;

use serde_json::{json, Value};

use crate::coins::Received;
use crate::market::MarketError;

/// Where the edge's wallet service and node listen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmrOpts {
    /// `monero-wallet-rpc`, `http://127.0.0.1:18083`. Same rule as a USDC
    /// endpoint.
    pub wallet_rpc: String,
    /// The wallet service's `user:password`.
    pub login_file: String,
    /// `monerod`'s restricted RPC, `http://127.0.0.1:18081`, asked which
    /// network it follows and how tall the chain is.
    pub daemon_rpc: String,
}

/// The networks a test edge may watch, and the one a live edge must, as
/// `monerod` names them. `fakechain` is a regtest node.
pub fn nettype_fits(nettype: &str, test: bool) -> bool {
    if test {
        matches!(nettype, "testnet" | "stagenet" | "fakechain")
    } else {
        nettype == "mainnet"
    }
}

// ── Addresses and keys ────────────────────────────────────────────────────

/// The network an address is for, from its prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Net {
    Main,
    Test,
    Stage,
}

impl Net {
    /// Whether a node of `nettype` takes this network's addresses. A regtest
    /// node uses mainnet's.
    pub fn fits(self, nettype: &str) -> bool {
        matches!(
            (self, nettype),
            (Net::Main, "mainnet" | "fakechain")
                | (Net::Test, "testnet")
                | (Net::Stage, "stagenet")
        )
    }
}

/// Prefixes of a primary address. Subaddresses and integrated addresses are
/// refused: neither carries the public keys a view-only wallet is made from.
const PRIMARY: [(u8, Net); 3] = [(18, Net::Main), (53, Net::Test), (24, Net::Stage)];
const SUBADDRESS: [u8; 3] = [42, 63, 36];
const INTEGRATED: [u8; 3] = [19, 54, 25];

/// A primary address, decoded and checksummed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    pub net: Net,
    pub spend: [u8; 32],
    pub view: [u8; 32],
}

/// Characters a full block of 8 bytes takes in Monero's base58, and what a
/// shorter tail block of each length takes.
const BLOCK_CHARS: [usize; 9] = [0, 2, 3, 5, 6, 7, 9, 10, 11];

/// Monero's base58: the bitcoin alphabet over 8-byte blocks, each written in
/// 11 characters (the tail in fewer), so leading zero bytes keep their place.
fn base58_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 8 / 11 + 8);
    for chunk in text.as_bytes().chunks(11) {
        let size = BLOCK_CHARS.iter().position(|n| *n == chunk.len())?;
        let digits = std::str::from_utf8(chunk).ok()?;
        let mut value: u128 = 0;
        for c in digits.bytes() {
            let d = crate::solana::ALPHABET.iter().position(|a| *a == c)? as u128;
            value = value.checked_mul(58)?.checked_add(d)?;
        }
        if size < 8 && value >> (8 * size) != 0 {
            return None;
        }
        if value > u128::from(u64::MAX) {
            return None;
        }
        out.extend_from_slice(&(value as u64).to_be_bytes()[8 - size..]);
    }
    Some(out)
}

/// The primary address in `text`, or why it is not one.
pub fn parse_address(text: &str) -> Result<Address, &'static str> {
    let text = text.trim();
    let raw = (text.len() == 95)
        .then(|| base58_decode(text))
        .flatten()
        .filter(|raw| raw.len() == 69)
        .ok_or("that is not a Monero address")?;
    if crate::stablecoin::keccak256(&raw[..65])[..4] != raw[65..] {
        return Err("that Monero address has a typo in it");
    }
    let prefix = raw[0];
    if SUBADDRESS.contains(&prefix) {
        return Err("send the wallet's primary address, the one starting with 4, not a subaddress");
    }
    if INTEGRATED.contains(&prefix) {
        return Err("send the wallet's primary address, not an integrated one");
    }
    let (_, net) = PRIMARY
        .iter()
        .find(|(p, _)| *p == prefix)
        .ok_or("that is not a Monero address")?;
    let mut spend = [0u8; 32];
    let mut view = [0u8; 32];
    spend.copy_from_slice(&raw[1..33]);
    view.copy_from_slice(&raw[33..65]);
    Ok(Address {
        net: *net,
        spend,
        view,
    })
}

fn hex32(text: &str) -> Option<[u8; 32]> {
    let text = text.trim();
    if text.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(out)
}

/// Check that `view_key` is the private view key of `address`: a reduced
/// scalar whose public point is the address's view key. The spend key is
/// told apart and refused by name, since a seller who pastes it has handed
/// over their funds.
pub fn check_view_key(address: &Address, view_key: &str) -> Result<(), &'static str> {
    use curve25519_dalek::{EdwardsPoint, Scalar};
    let bytes = hex32(view_key).ok_or("the view key is 64 hexadecimal characters")?;
    let scalar = Option::<Scalar>::from(Scalar::from_canonical_bytes(bytes))
        .ok_or("that is not a Monero private key")?;
    let public = EdwardsPoint::mul_base(&scalar).compress().to_bytes();
    if public == address.spend {
        return Err("that is the spend key; never send it to anyone. Send the view key");
    }
    if public != address.view {
        return Err("that view key does not belong to this address");
    }
    Ok(())
}

/// The name of a seller's wallet file in the wallet service, from their
/// address: the service holds nothing else to tell sellers apart by.
pub fn wallet_name(address: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, address.as_bytes());
    format!("losos-{}", crate::identity::to_hex(&digest.as_ref()[..12]))
}

// ── The wallet service ────────────────────────────────────────────────────

/// One wallet open at a time: open, ask, and nothing else in between.
static WALLET: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A seller's view-only wallet, as the market keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchKey<'a> {
    pub address: &'a str,
    pub view_key: &'a str,
    /// The chain height the wallet scans from when it has to be made again.
    pub from_height: u64,
}

/// A JSON-RPC error from the wallet service.
#[derive(Debug)]
struct RpcError {
    code: i64,
    message: String,
}

pub struct Node<'a> {
    opts: &'a XmrOpts,
}

const UNREACHABLE: MarketError = MarketError::Conflict("the edge's Monero wallet is not answering");
const REFUSED: MarketError = MarketError::Conflict("the edge's Monero wallet refused the request");

/// The `Digest` challenge of a 401, the MD5 one with `qop=auth` (the wallet
/// service offers MD5-sess beside it).
fn challenge(headers: &reqwest::header::HeaderMap) -> Option<(String, String)> {
    for value in headers.get_all(reqwest::header::WWW_AUTHENTICATE) {
        let text = value.to_str().ok()?;
        let Some(params) = text.strip_prefix("Digest ") else {
            continue;
        };
        let field = |name: &str| {
            params.split(',').find_map(|p| {
                let (k, v) = p.trim().split_once('=')?;
                (k.trim() == name).then(|| v.trim().trim_matches('"').to_string())
            })
        };
        let algorithm = field("algorithm").unwrap_or_else(|| "MD5".to_string());
        let qop_auth = field("qop").is_some_and(|q| q.split(',').any(|q| q.trim() == "auth"));
        if algorithm.eq_ignore_ascii_case("MD5") && qop_auth {
            return Some((field("realm")?, field("nonce")?));
        }
    }
    None
}

fn md5_hex(text: &str) -> String {
    use md5::Digest;
    crate::identity::to_hex(&md5::Md5::digest(text.as_bytes()))
}

/// The `Authorization` header answering a challenge (RFC 7616, MD5,
/// `qop=auth`, the first use of the nonce).
fn digest_header(
    user: &str,
    pass: &str,
    realm: &str,
    nonce: &str,
    uri: &str,
    cnonce: &str,
) -> String {
    let ha1 = md5_hex(&format!("{user}:{realm}:{pass}"));
    let ha2 = md5_hex(&format!("POST:{uri}"));
    let response = md5_hex(&format!("{ha1}:{nonce}:00000001:{cnonce}:auth:{ha2}"));
    format!(
        "Digest username=\"{user}\", realm=\"{realm}\", nonce=\"{nonce}\", uri=\"{uri}\", \
         algorithm=MD5, response=\"{response}\", qop=auth, nc=00000001, cnonce=\"{cnonce}\""
    )
}

/// A fresh client nonce for one digest answer.
fn cnonce() -> Result<String, MarketError> {
    crate::identity::random_hex(8).ok_or_else(|| MarketError::Store("no randomness".to_string()))
}

/// Piconero; the wallet service answers in whole units already.
fn atomic(value: &Value) -> Option<u128> {
    value.as_u64().map(u128::from)
}

impl<'a> Node<'a> {
    pub fn new(opts: &'a XmrOpts) -> Self {
        Node { opts }
    }

    fn client(timeout: Duration) -> Result<reqwest::Client, MarketError> {
        reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|_| MarketError::Unconfigured)
    }

    /// Ask the node, which needs no sign-in on its restricted port.
    async fn daemon(&self, method: &str) -> Result<Value, MarketError> {
        if !crate::stablecoin::rpc_url_ok(&self.opts.daemon_rpc) {
            return Err(MarketError::Unconfigured);
        }
        let url = format!("{}/json_rpc", self.opts.daemon_rpc.trim_end_matches('/'));
        let body: Value = Self::client(Duration::from_secs(5))?
            .post(url)
            .json(&json!({"jsonrpc": "2.0", "id": "losos", "method": method}))
            .send()
            .await
            .map_err(|_| MarketError::Conflict("the edge's Monero node is not answering"))?
            .json()
            .await
            .map_err(|_| MarketError::Conflict("the edge's Monero node is not answering"))?;
        body.get("result").cloned().ok_or(MarketError::Conflict(
            "the edge's Monero node refused the request",
        ))
    }

    /// The network the node follows (`mainnet`, `testnet`, `stagenet`,
    /// `fakechain`) and the chain's height.
    pub async fn info(&self) -> Result<(String, u64), MarketError> {
        let info = self.daemon("get_info").await?;
        let nettype = info
            .get("nettype")
            .and_then(Value::as_str)
            .ok_or(MarketError::Conflict(
                "the edge's Monero node gave no network",
            ))?;
        let height = info.get("height").and_then(Value::as_u64).unwrap_or(0);
        Ok((nettype.to_string(), height))
    }

    async fn login(&self) -> Result<(String, String), MarketError> {
        let raw = tokio::fs::read_to_string(&self.opts.login_file)
            .await
            .map_err(|_| MarketError::Unconfigured)?;
        let (user, pass) = raw
            .trim()
            .split_once(':')
            .ok_or(MarketError::Unconfigured)?;
        Ok((user.to_string(), pass.to_string()))
    }

    async fn call_raw(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Result<Value, RpcError>, MarketError> {
        if !crate::stablecoin::rpc_url_ok(&self.opts.wallet_rpc) {
            return Err(MarketError::Unconfigured);
        }
        let (user, pass) = self.login().await?;
        let base = self.opts.wallet_rpc.trim_end_matches('/');
        let path = "/json_rpc";
        let client = Self::client(timeout)?;
        let body = json!({"jsonrpc": "2.0", "id": "losos", "method": method, "params": params});
        let first = client
            .post(format!("{base}{path}"))
            .json(&body)
            .send()
            .await
            .map_err(|_| UNREACHABLE)?;
        let reply = if first.status() == reqwest::StatusCode::UNAUTHORIZED {
            let (realm, nonce) = challenge(first.headers()).ok_or(MarketError::Unconfigured)?;
            let cnonce = cnonce()?;
            client
                .post(format!("{base}{path}"))
                .header(
                    reqwest::header::AUTHORIZATION,
                    digest_header(&user, &pass, &realm, &nonce, path, &cnonce),
                )
                .json(&body)
                .send()
                .await
                .map_err(|_| UNREACHABLE)?
        } else {
            first
        };
        if reply.status() == reqwest::StatusCode::UNAUTHORIZED {
            tracing::warn!("monero-wallet-rpc refused the login in its login file");
            return Err(MarketError::Unconfigured);
        }
        let body: Value = reply.json().await.map_err(|_| UNREACHABLE)?;
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
        self.call_raw(method, params, Duration::from_secs(10))
            .await?
            .map_err(|e| {
                tracing::warn!("monero-wallet-rpc {method}: {} ({})", e.message, e.code);
                REFUSED
            })
    }

    /// Open the seller's wallet, or make it from their keys when the service
    /// has none by that name. Called under [`WALLET`].
    async fn open(&self, key: &WatchKey<'_>) -> Result<(), MarketError> {
        let name = wallet_name(key.address);
        match self
            .call_raw(
                "open_wallet",
                json!({"filename": name, "password": ""}),
                Duration::from_secs(10),
            )
            .await?
        {
            Ok(_) => return Ok(()),
            Err(e) => tracing::debug!("monero-wallet-rpc open_wallet {name}: {}", e.message),
        }
        self.call(
            "generate_from_keys",
            json!({
                "filename": name,
                "address": key.address,
                "viewkey": key.view_key,
                "password": "",
                "restore_height": key.from_height,
                "autosave_current": true,
            }),
        )
        .await
        .map(|_| ())
    }

    /// Make sure the wallet service can open a wallet for `key`, made now if
    /// it has none.
    pub async fn prepare(&self, key: &WatchKey<'_>) -> Result<(), MarketError> {
        let _held = WALLET.lock().await;
        self.open(key).await
    }

    /// The subaddress at `index` of the seller's first account, made in the
    /// wallet when it has fewer, so the wallet scans for it.
    pub async fn subaddress(&self, key: &WatchKey<'_>, index: u32) -> Result<String, MarketError> {
        if index == 0 {
            return Err(MarketError::Invalid("subaddress 0 is the primary address"));
        }
        let _held = WALLET.lock().await;
        self.open(key).await?;
        let have = self
            .call("get_address", json!({"account_index": 0}))
            .await?
            .get("addresses")
            .and_then(Value::as_array)
            .map_or(1, Vec::len) as u64;
        let wanted = u64::from(index) + 1;
        if have < wanted {
            // The service makes at most 64 at a time.
            let mut left = wanted - have;
            while left > 0 {
                let count = left.min(64);
                self.call(
                    "create_address",
                    json!({"account_index": 0, "count": count}),
                )
                .await?;
                left -= count;
            }
            self.call("store", json!({})).await?;
        }
        let found = self
            .call(
                "get_address",
                json!({"account_index": 0, "address_index": [index]}),
            )
            .await?;
        found
            .get("addresses")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .filter(|a| a.get("address_index").and_then(Value::as_u64) == Some(u64::from(index)))
            .and_then(|a| a.get("address"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or(MarketError::Conflict(
                "the edge's Monero wallet made no subaddress",
            ))
    }

    /// Every payment to subaddress `index` in a block at or above `since`,
    /// or still in the pool. A payment locked past its block, or one the
    /// pool saw double spent, is left out.
    pub async fn received(
        &self,
        key: &WatchKey<'_>,
        index: u32,
        since: u64,
    ) -> Result<Vec<Received>, MarketError> {
        let _held = WALLET.lock().await;
        self.open(key).await?;
        // Catch the wallet up first; it only follows the chain while open.
        self.call_raw("refresh", json!({}), Duration::from_secs(15))
            .await?
            .map_err(|e| {
                tracing::warn!("monero-wallet-rpc refresh: {}", e.message);
                REFUSED
            })?;
        let reply = self
            .call(
                "get_transfers",
                json!({"in": true, "pool": true, "account_index": 0, "subaddr_indices": [index]}),
            )
            .await?;
        Ok(transfers_to(&reply, index, since))
    }
}

/// The payments in a `get_transfers` reply that count for subaddress
/// `index`, one per transaction.
fn transfers_to(reply: &Value, index: u32, since: u64) -> Vec<Received> {
    let mut out: Vec<Received> = Vec::new();
    for (list, pooled) in [("in", false), ("pool", true)] {
        for t in reply
            .get(list)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let minor = t
                .get("subaddr_index")
                .and_then(|s| s.get("minor"))
                .and_then(Value::as_u64);
            let major = t
                .get("subaddr_index")
                .and_then(|s| s.get("major"))
                .and_then(Value::as_u64);
            if minor != Some(u64::from(index)) || major != Some(0) {
                continue;
            }
            if t.get("unlock_time").and_then(Value::as_u64) != Some(0) {
                continue;
            }
            if t.get("double_spend_seen").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            let height = t.get("height").and_then(Value::as_u64).unwrap_or(0);
            if !pooled && height < since {
                continue;
            }
            let (Some(tx), Some(amount)) = (
                t.get("txid").and_then(Value::as_str),
                t.get("amount").and_then(atomic),
            ) else {
                continue;
            };
            if amount == 0 {
                continue;
            }
            let confirmations = if pooled {
                0
            } else {
                t.get("confirmations").and_then(Value::as_u64).unwrap_or(0)
            };
            let first_seen = t.get("timestamp").and_then(Value::as_u64).unwrap_or(0);
            match out.iter_mut().find(|r| r.tx == tx) {
                // A transaction already counted from the chain is not
                // counted again from the pool.
                Some(_) if pooled => {}
                Some(r) => r.amount += amount,
                None => out.push(Received {
                    tx: tx.to_string(),
                    amount,
                    confirmations,
                    first_seen,
                }),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // A regtest wallet made for these tests; its funds never existed
    // anywhere else. The keys are public test data.
    const ADDRESS: &str = "43zQiVx36vm3hJRF36yL5REQD8xbw1k4SXAPvHRnFwkYYpneprfrQTwekqufJnmjB8bpRWayeRPAE1jiLX3BNBkLFoGA5wD";
    const SUBADDRESS: &str = "85y2xadA6utEvsULSVVKBNRXuEeGnTwvZXspACFSmHm7UvW9XvqHKW2GWX7pEadnt1R7LeGCeiZqxTSHzzCSEYXC9yXbWL8";

    #[test]
    fn a_primary_address_decodes_with_its_network() {
        let a = parse_address(ADDRESS).expect("decodes");
        assert_eq!(a.net, Net::Main);
        assert!(a.net.fits("mainnet"));
        assert!(a.net.fits("fakechain"));
        assert!(!a.net.fits("stagenet"));
    }

    #[test]
    fn subaddresses_typos_and_noise_are_refused() {
        assert_eq!(
            parse_address(SUBADDRESS),
            Err("send the wallet's primary address, the one starting with 4, not a subaddress")
        );
        let mut typo = ADDRESS.to_string();
        typo.replace_range(40..41, if &ADDRESS[40..41] == "a" { "b" } else { "a" });
        assert!(parse_address(&typo).is_err());
        assert!(parse_address("").is_err());
        assert!(parse_address(&ADDRESS[..94]).is_err());
        assert!(parse_address("bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu").is_err());
    }

    #[test]
    fn base58_blocks_keep_leading_zeros() {
        // "11" is a two-character block of one zero byte.
        assert_eq!(base58_decode("11"), Some(vec![0]));
        assert_eq!(base58_decode("1111111111111"), Some(vec![0; 9]));
        // A tail block too big for its length is refused.
        assert_eq!(base58_decode("zz"), None);
        assert_eq!(base58_decode("0"), None);
    }

    #[test]
    fn view_keys_are_checked_against_the_address() {
        use curve25519_dalek::{EdwardsPoint, Scalar};
        // Build an address from two known scalars, so the test carries no
        // key anyone uses.
        let view = Scalar::from_bytes_mod_order([7u8; 32]);
        let spend = Scalar::from_bytes_mod_order([9u8; 32]);
        let a = Address {
            net: Net::Main,
            spend: EdwardsPoint::mul_base(&spend).compress().to_bytes(),
            view: EdwardsPoint::mul_base(&view).compress().to_bytes(),
        };
        let hex = |s: &Scalar| {
            s.to_bytes()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        };
        assert_eq!(check_view_key(&a, &hex(&view)), Ok(()));
        assert_eq!(
            check_view_key(&a, &hex(&spend)),
            Err("that is the spend key; never send it to anyone. Send the view key")
        );
        let other = Scalar::from_bytes_mod_order([3u8; 32]);
        assert_eq!(
            check_view_key(&a, &hex(&other)),
            Err("that view key does not belong to this address")
        );
        assert!(check_view_key(&a, "ff".repeat(32).as_str()).is_err());
        assert!(check_view_key(&a, "xyz").is_err());
    }

    #[test]
    fn test_edges_watch_test_networks_and_live_edges_mainnet() {
        for n in ["testnet", "stagenet", "fakechain"] {
            assert!(nettype_fits(n, true));
            assert!(!nettype_fits(n, false));
        }
        assert!(nettype_fits("mainnet", false));
        assert!(!nettype_fits("mainnet", true));
    }

    #[test]
    fn digest_matches_rfc_2617s_example() {
        // RFC 2617 section 3.5, with POST in place of GET.
        let header = digest_header(
            "Mufasa",
            "Circle Of Life",
            "testrealm@host.com",
            "dcd98b7102dd2f0e8b11d0f600bfb0c093",
            "/dir/index.html",
            "0a4f113b",
        );
        let ha1 = md5_hex("Mufasa:testrealm@host.com:Circle Of Life");
        assert_eq!(ha1, "939e7578ed9e3c518a452acee763bce9");
        let ha2 = md5_hex("POST:/dir/index.html");
        let want = md5_hex(&format!(
            "{ha1}:dcd98b7102dd2f0e8b11d0f600bfb0c093:00000001:0a4f113b:auth:{ha2}"
        ));
        assert!(header.contains(&format!("response=\"{want}\"")));
    }

    #[test]
    fn the_md5_challenge_is_picked() {
        let mut h = reqwest::header::HeaderMap::new();
        h.append(
            reqwest::header::WWW_AUTHENTICATE,
            "Digest qop=\"auth\",algorithm=MD5-sess,realm=\"monero-rpc\",nonce=\"a\",stale=false"
                .parse()
                .unwrap(),
        );
        h.append(
            reqwest::header::WWW_AUTHENTICATE,
            "Digest qop=\"auth\",algorithm=MD5,realm=\"monero-rpc\",nonce=\"b\",stale=false"
                .parse()
                .unwrap(),
        );
        assert_eq!(
            challenge(&h),
            Some(("monero-rpc".to_string(), "b".to_string()))
        );
    }

    #[test]
    fn transfers_count_once_and_skip_locked_or_old_ones() {
        let t = |tx: &str, minor: u64, height: u64, unlock: u64| {
            json!({"txid": tx, "amount": 1_500_000_000_000u64, "confirmations": 3,
                   "height": height, "timestamp": 1_791_999_090u64, "unlock_time": unlock,
                   "double_spend_seen": false, "subaddr_index": {"major": 0, "minor": minor}})
        };
        let reply = json!({
            "in": [t("a", 1, 81, 0), t("old", 1, 10, 0), t("locked", 1, 81, 500), t("other", 2, 81, 0)],
            "pool": [t("a", 1, 0, 0), t("b", 1, 0, 0)],
        });
        let got = transfers_to(&reply, 1, 50);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].tx, "a");
        assert_eq!(got[0].confirmations, 3);
        assert_eq!(got[0].amount, 1_500_000_000_000);
        assert_eq!(got[1].tx, "b");
        assert_eq!(got[1].confirmations, 0);
    }

    #[test]
    fn wallet_names_are_stable_and_safe() {
        let n = wallet_name(ADDRESS);
        assert_eq!(n, wallet_name(ADDRESS));
        assert_eq!(n.len(), "losos-".len() + 24);
        assert!(n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    }
}
