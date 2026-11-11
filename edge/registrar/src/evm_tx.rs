//! The one key the edge holds for USDC: the escrow's releaser.
//!
//! [`LososEscrow`](https://github.com/dasmatus/losos/blob/main/edge/contracts/src/LososEscrow.sol)
//! holds each payment until the edge releases it to the seller recorded at
//! payment time or refunds it to the payer. The edge sends those two calls
//! and nothing else, so this module builds exactly one shape of transaction:
//! an EIP-1559 call with no value to the escrow. The key cannot redirect a
//! deposit; the contract checks that, not this code.
//!
//! The key is a systemd credential (`usdc-releaser-key`, 64 hex characters).
//! Its address needs a little ETH on every chain the edge settles on, for gas.

use std::path::Path;

use k256::ecdsa::SigningKey;
use k256::elliptic_curve::zeroize::Zeroizing;
use serde_json::{json, Value};

use crate::market::MarketError;
use crate::stablecoin::{self, Relay};

/// The escrow's releaser: a secp256k1 key and its address.
#[derive(Clone)]
pub struct Releaser {
    key: SigningKey,
    address: [u8; 20],
}

impl std::fmt::Debug for Releaser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the key.
        f.debug_struct("Releaser")
            .field("address", &self.address_hex())
            .finish()
    }
}

impl Releaser {
    /// A key from its 32 bytes as hex, with or without `0x`.
    pub fn from_hex(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        let hex = raw.strip_prefix("0x").unwrap_or(raw);
        if hex.len() != 64 {
            return None;
        }
        let bytes = stablecoin::decode_hex(&format!("0x{hex}"))?;
        let secret: [u8; 32] = bytes.try_into().ok()?;
        let key = SigningKey::from_bytes((&secret).into()).ok()?;
        let address = stablecoin::address_bytes(&stablecoin::address_of(key.verifying_key()))?;
        Some(Self { key, address })
    }

    /// Read the decrypted credential. Missing or malformed is "not
    /// configured", and the error never carries the file's content.
    pub async fn read(path: &Path) -> Result<Self, MarketError> {
        let raw = tokio::fs::read_to_string(path)
            .await
            .map_err(|_| MarketError::Unconfigured)?;
        Self::from_hex(&raw).ok_or(MarketError::Unconfigured)
    }

    /// A new key from the system's CSPRNG, and its 64 hex characters for
    /// the seal script's stdin. The hex is wiped when it is dropped; the
    /// edge makes the key so it never exists anywhere else.
    pub fn generate() -> Result<(Self, Zeroizing<String>), MarketError> {
        let rng = ring::rand::SystemRandom::new();
        // A random 32 bytes is a valid scalar unless it is zero or above
        // the group order, about once in 2^128 draws.
        for _ in 0..4 {
            let mut secret = Zeroizing::new([0u8; 32]);
            ring::rand::SecureRandom::fill(&rng, secret.as_mut())
                .map_err(|_| MarketError::Store("the system has no randomness".into()))?;
            let Ok(key) = SigningKey::from_bytes((&*secret).into()) else {
                continue;
            };
            let Some(address) =
                stablecoin::address_bytes(&stablecoin::address_of(key.verifying_key()))
            else {
                continue;
            };
            // Written straight into the wiped buffer: no unwiped copy of the
            // key on the way to the seal script.
            let mut hex = Zeroizing::new(String::with_capacity(64));
            for byte in secret.iter() {
                hex.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
                hex.push(char::from(b"0123456789abcdef"[usize::from(byte & 0xf)]));
            }
            return Ok((Self { key, address }, hex));
        }
        Err(MarketError::Store(
            "no valid key came out of four draws".into(),
        ))
    }

    /// The seed of this edge's Solana releaser
    /// ([`crate::solana_escrow::SolanaReleaser`]): HMAC-SHA256 of a fixed
    /// label under this key's secret. One sealed credential holds both keys,
    /// and neither can be worked out from the other's signatures.
    #[must_use]
    pub fn solana_seed(&self) -> Zeroizing<[u8; 32]> {
        let secret = Zeroizing::new(self.key.to_bytes());
        let mac = ring::hmac::sign(
            &ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret.as_slice()),
            b"LosOS Solana releaser v1",
        );
        let mut seed = Zeroizing::new([0u8; 32]);
        seed.copy_from_slice(mac.as_ref());
        seed
    }

    #[must_use]
    pub fn address(&self) -> [u8; 20] {
        self.address
    }

    #[must_use]
    pub fn address_hex(&self) -> String {
        format!("0x{}", crate::identity::to_hex(&self.address))
    }
}

/// What one escrow call costs and where it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub chain_id: u64,
    pub nonce: u64,
    pub max_priority_fee: u128,
    pub max_fee: u128,
    pub gas: u64,
    pub to: [u8; 20],
    pub data: Vec<u8>,
}

/// `release(uint256,bytes[])` on the escrow, with the box signatures that
/// agree (`crate::escrow_boxes`). The edge's own agreement is that it sends
/// the call.
#[must_use]
pub fn release_data(deposit: u64, sigs: &[Vec<u8>]) -> Vec<u8> {
    with_sigs(call_data("release(uint256,bytes[])", deposit), sigs)
}

/// `refund(uint256,bytes[])` on the escrow.
#[must_use]
pub fn refund_data(deposit: u64, sigs: &[Vec<u8>]) -> Vec<u8> {
    with_sigs(call_data("refund(uint256,bytes[])", deposit), sigs)
}

/// `settle(uint256,uint256,bytes[])` on the escrow: `refund` base units
/// back to the payer, the rest to the seller and the treasury.
#[must_use]
pub fn settle_data(deposit: u64, refund: u128, sigs: &[Vec<u8>]) -> Vec<u8> {
    let mut data = call_data("settle(uint256,uint256,bytes[])", deposit);
    let mut word = [0u8; 32];
    word[16..].copy_from_slice(&refund.to_be_bytes());
    data.extend_from_slice(&word);
    with_sigs(data, sigs)
}

/// `head` (a selector and its static words) followed by a `bytes[]`
/// argument as its last: the offset word, then the array.
fn with_sigs(mut head: Vec<u8>, sigs: &[Vec<u8>]) -> Vec<u8> {
    let word = |n: usize| {
        let mut w = [0u8; 32];
        w[24..].copy_from_slice(&(n as u64).to_be_bytes());
        w
    };
    let padded = |n: usize| n.div_ceil(32) * 32;
    // The offset counts from the first argument, after the selector, and
    // this word is itself one of the head's words.
    head.extend_from_slice(&word(head.len() - 4 + 32));
    head.extend_from_slice(&word(sigs.len()));
    let mut at = sigs.len() * 32;
    for sig in sigs {
        head.extend_from_slice(&word(at));
        at += 32 + padded(sig.len());
    }
    for sig in sigs {
        head.extend_from_slice(&word(sig.len()));
        head.extend_from_slice(sig);
        head.resize(head.len() + padded(sig.len()) - sig.len(), 0);
    }
    head
}

/// `deposits(uint256)`: the escrow's record of one deposit. The last word
/// of the answer is its state.
#[must_use]
pub fn deposits_data(deposit: u64) -> Vec<u8> {
    call_data("deposits(uint256)", deposit)
}

/// `settleTap(bytes32,uint256)`: bill `amount` base units on a tap.
#[must_use]
pub fn settle_tap_data(tap: [u8; 32], amount: u64) -> Vec<u8> {
    let mut data = word_call("settleTap(bytes32,uint256)", tap);
    let mut word = [0u8; 32];
    word[24..].copy_from_slice(&amount.to_be_bytes());
    data.extend_from_slice(&word);
    data
}

/// `closeTap(bytes32)`.
#[must_use]
pub fn close_tap_data(tap: [u8; 32]) -> Vec<u8> {
    word_call("closeTap(bytes32)", tap)
}

/// `taps(bytes32)`: buyer, seller, rate, feeBps, settledAt, closedAt.
#[must_use]
pub fn taps_data(tap: [u8; 32]) -> Vec<u8> {
    word_call("taps(bytes32)", tap)
}

/// `balances(address)`.
#[must_use]
pub fn balances_data(wallet: [u8; 20]) -> Vec<u8> {
    word_call("balances(address)", address_word(wallet))
}

/// `withdrawals(address)`: amount, notBefore.
#[must_use]
pub fn withdrawals_data(wallet: [u8; 20]) -> Vec<u8> {
    word_call("withdrawals(address)", address_word(wallet))
}

/// An address as an ABI word, and as the 32-byte key [`crate::meter`] keeps.
#[must_use]
pub fn address_word(address: [u8; 20]) -> [u8; 32] {
    let mut word = [0u8; 32];
    word[12..].copy_from_slice(&address);
    word
}

fn word_call(signature: &str, word: [u8; 32]) -> Vec<u8> {
    let mut data = stablecoin::keccak256(signature.as_bytes())[..4].to_vec();
    data.extend_from_slice(&word);
    data
}

fn call_data(signature: &str, deposit: u64) -> Vec<u8> {
    let mut data = stablecoin::keccak256(signature.as_bytes())[..4].to_vec();
    let mut word = [0u8; 32];
    word[24..].copy_from_slice(&deposit.to_be_bytes());
    data.extend_from_slice(&word);
    data
}

/// Sign `call` as an EIP-1559 transaction. Returns the raw transaction and
/// its hash, both `0x` hex.
#[must_use]
pub fn sign(releaser: &Releaser, call: &Call) -> (String, String) {
    let fields = unsigned_fields(call);
    let mut payload = vec![0x02];
    payload.extend(rlp_list(&fields));
    let digest = stablecoin::keccak256(&payload);
    let (signature, recid) = releaser.key.sign_prehash_recoverable(&digest);
    let bytes = signature.to_bytes();
    let mut signed = fields;
    signed.push(rlp_uint(u128::from(u8::from(recid) & 1)));
    signed.push(rlp_bytes(trim(&bytes[..32])));
    signed.push(rlp_bytes(trim(&bytes[32..])));
    let mut raw = vec![0x02];
    raw.extend(rlp_list(&signed));
    let hash = stablecoin::keccak256(&raw);
    (
        format!("0x{}", crate::identity::to_hex(&raw)),
        format!("0x{}", crate::identity::to_hex(&hash)),
    )
}

fn unsigned_fields(call: &Call) -> Vec<Vec<u8>> {
    vec![
        rlp_uint(u128::from(call.chain_id)),
        rlp_uint(u128::from(call.nonce)),
        rlp_uint(call.max_priority_fee),
        rlp_uint(call.max_fee),
        rlp_uint(u128::from(call.gas)),
        rlp_bytes(&call.to),
        rlp_uint(0),
        rlp_bytes(&call.data),
        // An empty access list.
        rlp_list(&[]),
    ]
}

/// Build, sign and send one escrow call from `releaser`. Returns the
/// transaction hash. The node picks the nonce (pending count) and the fee
/// (twice the latest base fee plus its tip), and the gas is its estimate
/// with a fifth on top. A call the contract would refuse fails the estimate
/// and is never sent. With `replace`, the call takes the oldest unmined
/// nonce and twice the fees, so it replaces a call that never got mined.
pub async fn send(
    rpc_url: &str,
    chain_id: u64,
    releaser: &Releaser,
    to: [u8; 20],
    data: Vec<u8>,
    replace: bool,
) -> Result<String, MarketError> {
    let from = releaser.address_hex();
    let to_hex = format!("0x{}", crate::identity::to_hex(&to));
    let data_hex = format!("0x{}", crate::identity::to_hex(&data));
    let nonce = quantity(
        &stablecoin::call_result(
            rpc_url,
            Relay::Evm,
            "eth_getTransactionCount",
            json!([from, if replace { "latest" } else { "pending" }]),
        )
        .await?,
    )?;
    let gas = quantity(
        &stablecoin::call_result(
            rpc_url,
            Relay::Evm,
            "eth_estimateGas",
            json!([{ "from": from, "to": to_hex, "data": data_hex }]),
        )
        .await
        .map_err(|_| MarketError::Conflict("the escrow refused the call"))?,
    )?;
    let block = stablecoin::call_result(
        rpc_url,
        Relay::Evm,
        "eth_getBlockByNumber",
        json!(["latest", false]),
    )
    .await?;
    let base = quantity(&block["baseFeePerGas"])?;
    let tip =
        match stablecoin::call_result(rpc_url, Relay::Evm, "eth_maxPriorityFeePerGas", json!([]))
            .await
        {
            Ok(v) => quantity(&v)?,
            // A node without the method: a thousandth of a gwei is enough on an
            // L2, where the sequencer orders by arrival.
            Err(_) => 1_000_000,
        };
    let bump = if replace { 2 } else { 1 };
    let call = Call {
        chain_id,
        nonce: u64::try_from(nonce).map_err(|_| bad_answer())?,
        max_priority_fee: tip.saturating_mul(bump),
        max_fee: base
            .saturating_mul(2)
            .saturating_add(tip)
            .saturating_mul(bump),
        gas: u64::try_from(gas.saturating_mul(6) / 5).map_err(|_| bad_answer())?,
        to,
        data,
    };
    let (raw, hash) = sign(releaser, &call);
    let sent = stablecoin::call_result(rpc_url, Relay::Evm, "eth_sendRawTransaction", json!([raw]))
        .await
        .map_err(|_| MarketError::Conflict("the chain refused the escrow call"))?;
    // The node's hash is the same as ours; keep ours, which cannot be spoofed.
    if sent
        .as_str()
        .is_some_and(|s| !s.eq_ignore_ascii_case(&hash))
    {
        tracing::warn!("the chain endpoint named escrow call {hash} differently: {sent}");
    }
    Ok(hash)
}

fn bad_answer() -> MarketError {
    MarketError::Conflict("the chain endpoint did not answer")
}

fn quantity(v: &Value) -> Result<u128, MarketError> {
    let bytes = stablecoin::decode_hex(v.as_str().unwrap_or("")).ok_or_else(bad_answer)?;
    if bytes.len() > 16 {
        return Err(bad_answer());
    }
    Ok(bytes.iter().fold(0u128, |acc, b| acc << 8 | u128::from(*b)))
}

fn trim(bytes: &[u8]) -> &[u8] {
    let start = bytes.iter().position(|b| *b != 0).unwrap_or(bytes.len());
    &bytes[start..]
}

fn rlp_uint(n: u128) -> Vec<u8> {
    let be = n.to_be_bytes();
    rlp_bytes(trim(&be))
}

fn rlp_bytes(bytes: &[u8]) -> Vec<u8> {
    if bytes.len() == 1 && bytes[0] < 0x80 {
        return bytes.to_vec();
    }
    let mut out = rlp_length(bytes.len(), 0x80);
    out.extend_from_slice(bytes);
    out
}

fn rlp_list(items: &[Vec<u8>]) -> Vec<u8> {
    let payload: Vec<u8> = items.concat();
    let mut out = rlp_length(payload.len(), 0xc0);
    out.extend(payload);
    out
}

fn rlp_length(len: usize, offset: u8) -> Vec<u8> {
    if len < 56 {
        return vec![offset + len as u8];
    }
    let be = (len as u64).to_be_bytes();
    let be = trim(&be);
    let mut out = vec![offset + 55 + be.len() as u8];
    out.extend_from_slice(be);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // anvil's first default key. Never holds anything real.
    const KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    /// A held deposit on a local chain, released by the edge's own signer:
    /// `edge/contracts/test-payment.sh` writes the file and runs this. A node
    /// that takes the transaction and an escrow that then reads `Released`
    /// is the proof the encoding and the signature are right.
    #[tokio::test]
    async fn an_anvil_deposit_is_released_by_the_edge_key() {
        let Ok(path) = std::env::var("LOSOS_ANVIL_ESCROW") else {
            return;
        };
        let meta: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("escrow file")).unwrap();
        let rpc = meta["rpc"].as_str().unwrap();
        let escrow = meta["escrow"].as_str().unwrap();
        let deposit = meta["deposit"].as_u64().unwrap();
        let releaser = Releaser::from_hex(meta["releaser_key"].as_str().unwrap()).unwrap();
        assert_eq!(
            stablecoin::deposit_state(rpc, escrow, deposit)
                .await
                .unwrap(),
            stablecoin::DepositState::Held
        );
        let to = stablecoin::address_bytes(escrow).unwrap();
        // The edge's key alone is one of the three: refused at the estimate,
        // so nothing is sent.
        assert!(
            send(rpc, 31337, &releaser, to, release_data(deposit, &[]), false)
                .await
                .is_err()
        );
        // The seller's box agrees to the release; with the edge that is two.
        let seller_box = k256::ecdsa::SigningKey::from_slice(
            &stablecoin::decode_hex(meta["seller_box_key"].as_str().unwrap()).unwrap(),
        )
        .unwrap();
        let digest = crate::escrow_boxes::evm_digest(31337, to, deposit, 0);
        let (sig, recid) = seller_box.sign_prehash_recoverable(&digest);
        let mut sig = sig.to_bytes().to_vec();
        sig.push(27 + u8::from(recid));
        let sigs = [sig];
        let hash = send(
            rpc,
            31337,
            &releaser,
            to,
            release_data(deposit, &sigs),
            false,
        )
        .await
        .unwrap();
        // anvil answers eth_sendRawTransaction before it has mined the
        // block, so the receipt can be a moment behind, as on a real chain.
        let mut receipt = stablecoin::fetch_receipt(rpc, &hash).await;
        for _ in 0..50 {
            if receipt.is_ok() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            receipt = stablecoin::fetch_receipt(rpc, &hash).await;
        }
        let receipt = receipt.unwrap();
        assert_eq!(receipt["status"], "0x1", "{receipt}");
        assert_eq!(
            stablecoin::deposit_state(rpc, escrow, deposit)
                .await
                .unwrap(),
            stablecoin::DepositState::Released
        );
        // Released once: the escrow refuses a second call at the estimate,
        // so nothing is sent.
        assert!(send(
            rpc,
            31337,
            &releaser,
            to,
            release_data(deposit, &sigs),
            false
        )
        .await
        .is_err());
    }

    async fn mined(rpc: &str, hash: &str) -> serde_json::Value {
        // anvil answers eth_sendRawTransaction before it has mined the
        // block, so the receipt can be a moment behind, as on a real chain.
        let mut receipt = stablecoin::fetch_receipt(rpc, hash).await;
        for _ in 0..50 {
            if receipt.is_ok() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            receipt = stablecoin::fetch_receipt(rpc, hash).await;
        }
        receipt.unwrap()
    }

    /// `LOSOS_ANVIL_TAPS`, written by `edge/contracts/test-payment.sh`: a
    /// wallet topped up and opened one tap; the edge key settles it within
    /// the head start, is refused past the cap, and closes it.
    #[tokio::test]
    async fn an_anvil_tap_is_settled_within_its_cap_and_closed() {
        let Ok(path) = std::env::var("LOSOS_ANVIL_TAPS") else {
            return;
        };
        let meta: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("taps file")).unwrap();
        let rpc = meta["rpc"].as_str().unwrap();
        let escrow = meta["escrow"].as_str().unwrap();
        let rate = meta["rate"].as_u64().unwrap();
        let releaser = Releaser::from_hex(meta["releaser_key"].as_str().unwrap()).unwrap();
        let buyer = stablecoin::address_bytes(meta["buyer"].as_str().unwrap()).unwrap();
        // The escrow's own `tapKey`, read back by the script, is what the
        // edge works out.
        let tap = stablecoin::tap_key(buyer, meta["tap"].as_str().unwrap());
        assert_eq!(
            format!("0x{}", crate::identity::to_hex(&tap)),
            meta["tap_key"].as_str().unwrap()
        );
        let seller = stablecoin::address_bytes(meta["seller"].as_str().unwrap()).unwrap();
        let word = |a: [u8; 20]| {
            let mut w = [0u8; 32];
            w[12..].copy_from_slice(&a);
            w
        };
        let to = stablecoin::address_bytes(escrow).unwrap();

        let open = stablecoin::tap_state(rpc, escrow, tap)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(open.buyer, word(buyer));
        assert_eq!(open.seller, word(seller));
        assert_eq!(open.rate, rate);
        assert_eq!(open.fee_bps, 400);
        assert_eq!(open.closed_at, None);
        let before = stablecoin::wallet_view(rpc, escrow, buyer).await.unwrap();
        assert_eq!(before.balance, meta["topped_up"].as_u64().unwrap());

        // The head start: an hour at the rate is billable the moment the
        // tap opens.
        let hash = send(rpc, 31337, &releaser, to, settle_tap_data(tap, rate), false)
            .await
            .unwrap();
        assert_eq!(mined(rpc, &hash).await["status"], "0x1");
        let settled = stablecoin::tap_state(rpc, escrow, tap)
            .await
            .unwrap()
            .unwrap();
        assert!(settled.settled_at > open.settled_at);
        let after = stablecoin::wallet_view(rpc, escrow, buyer).await.unwrap();
        assert_eq!(after.balance, before.balance - rate);

        // The cap moved with it: another hour's worth a moment later is
        // refused at the estimate, so nothing is sent.
        assert!(
            send(rpc, 31337, &releaser, to, settle_tap_data(tap, rate), false)
                .await
                .is_err()
        );
        // So is a settle of nothing.
        assert!(
            send(rpc, 31337, &releaser, to, settle_tap_data(tap, 0), false)
                .await
                .is_err()
        );

        let hash = send(rpc, 31337, &releaser, to, close_tap_data(tap), false)
            .await
            .unwrap();
        assert_eq!(mined(rpc, &hash).await["status"], "0x1");
        let closed = stablecoin::tap_state(rpc, escrow, tap)
            .await
            .unwrap()
            .unwrap();
        assert!(closed.closed_at.is_some());
        // Closed once.
        assert!(send(rpc, 31337, &releaser, to, close_tap_data(tap), false)
            .await
            .is_err());
    }

    #[test]
    fn the_key_names_its_address_and_never_prints_itself() {
        let r = Releaser::from_hex(KEY).unwrap();
        assert_eq!(
            r.address_hex(),
            "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
        );
        assert!(!format!("{r:?}").contains(KEY));
        assert!(Releaser::from_hex("0x").is_none());
        assert!(Releaser::from_hex(&KEY[2..]).is_none());
        assert!(Releaser::from_hex(&format!("0x{KEY}\n")).is_some());
    }

    #[test]
    fn a_made_key_is_the_shape_the_seal_script_takes() {
        let (a, hex) = Releaser::generate().unwrap();
        let (b, _) = Releaser::generate().unwrap();
        // modules/edge-credentials.nix: `(0x)?[0-9a-fA-F]{64}`.
        assert_eq!(hex.len(), 64);
        assert!(hex.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(Releaser::from_hex(&hex).unwrap().address(), a.address());
        assert_ne!(a.address(), b.address());
    }

    /// Each against `cast calldata`, with a signature of 65 `ab` bytes
    /// ending in `1b`.
    #[test]
    fn call_data_is_what_cast_encodes() {
        let mut sig = vec![0xab; 64];
        sig.push(0x1b);
        let hex = |d: Vec<u8>| crate::identity::to_hex(&d);
        let tail = "0000000000000000000000000000000000000000000000000000000000000041\
            abababababababababababababababababababababababababababababababab\
            abababababababababababababababababababababababababababababababab\
            1b00000000000000000000000000000000000000000000000000000000000000";
        assert_eq!(
            hex(release_data(7, std::slice::from_ref(&sig))),
            [
                "1aa4c26a",
                "0000000000000000000000000000000000000000000000000000000000000007",
                "0000000000000000000000000000000000000000000000000000000000000040",
                "0000000000000000000000000000000000000000000000000000000000000001",
                "0000000000000000000000000000000000000000000000000000000000000020",
                tail,
            ]
            .concat()
        );
        assert_eq!(
            hex(refund_data(1, &[])),
            [
                "1df60d97",
                "0000000000000000000000000000000000000000000000000000000000000001",
                "0000000000000000000000000000000000000000000000000000000000000040",
                "0000000000000000000000000000000000000000000000000000000000000000",
            ]
            .concat()
        );
        assert_eq!(
            hex(settle_data(7, 2_500_000, &[sig.clone(), sig])),
            [
                "0e0f4b0c",
                "0000000000000000000000000000000000000000000000000000000000000007",
                "00000000000000000000000000000000000000000000000000000000002625a0",
                "0000000000000000000000000000000000000000000000000000000000000060",
                "0000000000000000000000000000000000000000000000000000000000000002",
                "0000000000000000000000000000000000000000000000000000000000000040",
                "00000000000000000000000000000000000000000000000000000000000000c0",
                tail,
                tail,
            ]
            .concat()
        );
    }

    #[test]
    fn rlp_encodes_the_edge_cases() {
        assert_eq!(rlp_uint(0), vec![0x80]);
        assert_eq!(rlp_uint(0x7f), vec![0x7f]);
        assert_eq!(rlp_uint(0x80), vec![0x81, 0x80]);
        assert_eq!(rlp_uint(1024), vec![0x82, 0x04, 0x00]);
        assert_eq!(rlp_list(&[]), vec![0xc0]);
        let long = vec![0xaa; 60];
        assert_eq!(&rlp_bytes(&long)[..2], &[0xb8, 60]);
    }

    /// The same call signed by `cast mktx` (Foundry 1.5.1):
    /// `cast mktx --private-key $KEY --chain 84532 --nonce 3 --gas-limit 90000
    ///  --priority-gas-price 1000000 --gas-price 2000000000
    ///  0x1111111111111111111111111111111111111111 'release(uint256)' 7`.
    /// Both sign deterministically (RFC 6979), so the bytes must match.
    #[test]
    fn a_release_signs_byte_for_byte_like_foundry() {
        let r = Releaser::from_hex(KEY).unwrap();
        let call = Call {
            chain_id: 84532,
            nonce: 3,
            max_priority_fee: 1_000_000,
            max_fee: 2_000_000_000,
            gas: 90_000,
            to: [0x11; 20],
            // What foundry signed: the escrow's earlier one-argument call.
            data: call_data("release(uint256)", 7),
        };
        let (raw, hash) = sign(&r, &call);
        assert_eq!(raw, FOUNDRY_RAW);
        assert_eq!(hash, stablecoin_hash(FOUNDRY_RAW));
    }

    fn stablecoin_hash(raw: &str) -> String {
        let bytes = stablecoin::decode_hex(raw).unwrap();
        format!(
            "0x{}",
            crate::identity::to_hex(&stablecoin::keccak256(&bytes))
        )
    }

    const FOUNDRY_RAW: &str = "0x02f89183014a3403830f4240847735940083015f9094111111111111111111111111111111111111111180a437bdc99b0000000000000000000000000000000000000000000000000000000000000007c080a07d9e1ba59223873c910ec009739efdc98ff52ebda49575f5b489cc05757479f6a07383db6f1cc7ac43b6a9844756599242865ae7297142ea5f61462a39a5e967e2";
}
