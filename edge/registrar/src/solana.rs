//! USDC on Solana, the second family [`crate::stablecoin`] settles on.
//!
//! There is no pay contract. The buyer's wallet sends one transaction with
//! two `TransferChecked` instructions, the seller's share and the fee, and a
//! memo naming the order. The edge reads that transaction back at the
//! `confirmed` commitment and checks the token balances it moved, which is
//! what a Solana explorer shows too. A transaction cannot carry two memos
//! for one order and a ledger keeps every signature it counted, so one
//! payment cannot pay twice.

use ring::signature::{UnparsedPublicKey, ED25519};
use serde_json::{json, Value};

use crate::market::MarketError;
use crate::stablecoin::{call_result, Relay};

/// Methods the registrar forwards for a Solana wallet. `sendTransaction` is
/// the only one that publishes anything, and only a transaction the wallet
/// has already signed.
pub const METHODS: &[&str] = &[
    "getAccountInfo",
    "getBalance",
    "getBlockHeight",
    "getFeeForMessage",
    "getGenesisHash",
    "getLatestBlockhash",
    "getMinimumBalanceForRentExemption",
    "getSignatureStatuses",
    "getSlot",
    "getTokenAccountBalance",
    "getTokenAccountsByOwner",
    "getTransaction",
    "isBlockhashValid",
    "sendTransaction",
    "simulateTransaction",
];

/// A Solana transaction is at most 1232 bytes; base64 of that is 1644.
const MAX_WIRE_TX: usize = 1644;

/// What one paid order must look like on Solana.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedTransfer {
    /// Circle's USDC mint for the network.
    pub mint: String,
    /// The seller's payout owner (base58); the tokens land in its account.
    pub seller: String,
    pub treasury: String,
    /// Base units the seller receives: gross less the fee.
    pub seller_amount: u128,
    pub fee: u128,
    /// The memo the wallet writes: the order id hash, `0x` + 64 hex.
    pub memo: String,
}

/// A base58 public key, re-encoded, or `None` unless it is 32 bytes.
#[must_use]
pub fn normalize_address(raw: &str) -> Option<String> {
    let bytes = base58_decode(raw.trim())?;
    (bytes.len() == 32).then(|| base58_encode(&bytes))
}

/// A base58 transaction signature (64 bytes), re-encoded.
#[must_use]
pub fn normalize_signature(raw: &str) -> Option<String> {
    let bytes = base58_decode(raw.trim())?;
    (bytes.len() == 64).then(|| base58_encode(&bytes))
}

/// Whether `signature` (base58, 64 bytes) is `address`'s ed25519 signature
/// of `message`. Wallets sign the UTF-8 bytes as they are, with no prefix.
#[must_use]
pub fn verify_message(message: &[u8], address: &str, signature: &str) -> bool {
    let (Some(key), Some(sig)) = (base58_decode(address), base58_decode(signature)) else {
        return false;
    };
    key.len() == 32
        && sig.len() == 64
        && UnparsedPublicKey::new(&ED25519, &key)
            .verify(message, &sig)
            .is_ok()
}

/// A `sendTransaction` call: one base64 string of a signed transaction, and
/// at most an options object.
pub(crate) fn wire_tx_ok(params: &Value) -> bool {
    let Some(p) = params.as_array() else {
        return false;
    };
    let Some(wire) = p.first().and_then(Value::as_str) else {
        return false;
    };
    p.len() <= 2
        && p.get(1).is_none_or(Value::is_object)
        && (100..=MAX_WIRE_TX).contains(&wire.len())
        && wire
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
}

/// Check that the sealed endpoint is the cluster this edge settles on. The
/// genesis hash is what tells devnet from mainnet, the way a chain id does
/// on an EVM chain.
pub async fn check_genesis(rpc_url: &str, want: &str) -> Result<(), MarketError> {
    let got = call_result(rpc_url, Relay::Solana, "getGenesisHash", json!([])).await?;
    if got.as_str() != Some(want) {
        return Err(MarketError::Conflict(
            "the chain endpoint is on a different cluster",
        ));
    }
    Ok(())
}

/// The transaction at the `confirmed` commitment, or a conflict while it is
/// not there yet.
pub async fn fetch_transaction(rpc_url: &str, signature: &str) -> Result<Value, MarketError> {
    let reply = crate::stablecoin::forward_rpc(
        rpc_url,
        Relay::Solana,
        "getTransaction",
        &json!([
            signature,
            {
                "encoding": "jsonParsed",
                "commitment": "confirmed",
                "maxSupportedTransactionVersion": 0
            }
        ]),
        &json!(1),
    )
    .await?;
    match reply.get("result") {
        Some(Value::Object(_)) => Ok(reply["result"].clone()),
        Some(Value::Null) | None => {
            Err(MarketError::Conflict("the transaction is not on chain yet"))
        }
        _ => Err(MarketError::Conflict(
            "the chain endpoint did not answer JSON",
        )),
    }
}

/// Whether `tx` (a `getTransaction` result, `jsonParsed`) succeeded, names
/// the order in a memo and moved exactly the order's USDC to the seller and
/// the treasury. A static sentence on failure, safe to show the owner.
pub fn transfer_matches(tx: &Value, expected: &ExpectedTransfer) -> Result<(), &'static str> {
    let meta = tx
        .get("meta")
        .filter(|m| m.is_object())
        .ok_or("the transaction has no result")?;
    if !meta["err"].is_null() {
        return Err("the transaction failed");
    }
    let instructions = tx["transaction"]["message"]["instructions"]
        .as_array()
        .ok_or("the transaction has no instructions")?;
    let names_order = instructions.iter().any(|i| {
        (i["program"] == "spl-memo" || i["programId"] == MEMO_PROGRAM)
            && i["parsed"].as_str() == Some(expected.memo.as_str())
    });
    if !names_order {
        return Err("the transaction does not pay this order");
    }
    let seller = received(meta, &expected.mint, &expected.seller)?;
    let treasury = received(meta, &expected.mint, &expected.treasury)?;
    if seller != expected.seller_amount as i128 || treasury != expected.fee as i128 {
        return Err("the payment amount is not the order's");
    }
    Ok(())
}

/// The memo program the wallet writes the order into.
pub const MEMO_PROGRAM: &str = "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr";

/// How much of `mint` the token accounts `owner` holds gained in the
/// transaction, from its pre and post token balances. An account the
/// transaction created has no pre balance, which counts as zero.
fn received(meta: &Value, mint: &str, owner: &str) -> Result<i128, &'static str> {
    let balances = |key: &str| -> Result<Vec<(u64, i128)>, &'static str> {
        let Some(list) = meta[key].as_array() else {
            return Ok(Vec::new());
        };
        list.iter()
            .filter(|b| b["mint"] == mint && b["owner"] == owner)
            .map(|b| {
                let index = b["accountIndex"]
                    .as_u64()
                    .ok_or("the receipt is malformed")?;
                let amount = b["uiTokenAmount"]["amount"]
                    .as_str()
                    .and_then(|a| a.parse::<i128>().ok())
                    .ok_or("the receipt is malformed")?;
                Ok((index, amount))
            })
            .collect()
    };
    let pre = balances("preTokenBalances")?;
    let post = balances("postTokenBalances")?;
    let mut total = 0i128;
    for (index, after) in &post {
        let before = pre
            .iter()
            .find(|(i, _)| i == index)
            .map_or(0, |(_, amount)| *amount);
        total += after - before;
    }
    // An account that was emptied and closed has a pre balance and no post.
    for (index, before) in &pre {
        if !post.iter().any(|(i, _)| i == index) {
            total -= before;
        }
    }
    Ok(total)
}

pub(crate) const ALPHABET: &[u8; 58] =
    b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

#[must_use]
pub fn base58_encode(bytes: &[u8]) -> String {
    let zeros = bytes.iter().take_while(|b| **b == 0).count();
    let mut digits: Vec<u8> = Vec::with_capacity(bytes.len() * 138 / 100 + 1);
    for &byte in &bytes[zeros..] {
        let mut carry = u32::from(byte);
        for d in &mut digits {
            carry += u32::from(*d) << 8;
            *d = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }
    let mut out = String::with_capacity(zeros + digits.len());
    out.extend(std::iter::repeat_n('1', zeros));
    out.extend(digits.iter().rev().map(|d| ALPHABET[*d as usize] as char));
    out
}

/// Base58 to bytes. Long enough for a whole transaction's worth of
/// instruction data (1232 bytes), which a `pay` naming both boxes' keys
/// needs; the work grows with the square of the length, so no more.
#[must_use]
pub fn base58_decode(raw: &str) -> Option<Vec<u8>> {
    if raw.is_empty() || raw.len() > 1700 {
        return None;
    }
    let zeros = raw.bytes().take_while(|b| *b == b'1').count();
    let mut bytes: Vec<u8> = Vec::with_capacity(raw.len());
    for c in raw.bytes().skip(zeros) {
        let mut carry = ALPHABET.iter().position(|a| *a == c)? as u32;
        for b in &mut bytes {
            carry += u32::from(*b) * 58;
            *b = (carry & 0xff) as u8;
            carry >>= 8;
        }
        while carry > 0 {
            bytes.push((carry & 0xff) as u8);
            carry >>= 8;
        }
    }
    let mut out = vec![0u8; zeros];
    out.extend(bytes.iter().rev());
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINT: &str = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";
    const SELLER: &str = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";
    const TREASURY: &str = "5ZWj7a1f8tWkjBESHKgrLmXshuXxqeY9SYcfbshpAqPG";
    const BUYER: &str = "HN7cABqLq46Es1jh92dQQisAq662SmxELLLsHHe4YWrH";
    const MEMO: &str = "0x6b0e8e7b0d3b1b8a1c0ef4a7d0b5a3b38f0f74d7fd2c3b3a6f3b1c0e9d8f7a6b";

    #[test]
    fn base58_round_trips_and_keeps_leading_zeros() {
        for raw in ["11111111111111111111111111111111", MINT, SELLER, "1112"] {
            let bytes = base58_decode(raw).unwrap();
            assert_eq!(base58_encode(&bytes), raw);
        }
        assert_eq!(
            base58_decode("11111111111111111111111111111111").unwrap(),
            vec![0; 32]
        );
        assert_eq!(base58_decode("0OIl"), None);
        assert_eq!(normalize_address(MINT).as_deref(), Some(MINT));
        assert_eq!(normalize_address("1112"), None);
    }

    #[test]
    fn a_payout_signature_is_the_addresses_ed25519_signature() {
        use ring::signature::{Ed25519KeyPair, KeyPair};
        let pair = Ed25519KeyPair::from_seed_unchecked(&[7u8; 32]).unwrap();
        let address = base58_encode(pair.public_key().as_ref());
        let message = crate::stablecoin::payout_message("solana-devnet", &address);
        let signature = base58_encode(pair.sign(message.as_bytes()).as_ref());
        assert!(verify_message(message.as_bytes(), &address, &signature));
        // The live network is a different message.
        let live = crate::stablecoin::payout_message("solana", &address);
        assert!(!verify_message(live.as_bytes(), &address, &signature));
        assert!(!verify_message(message.as_bytes(), SELLER, &signature));
    }

    #[test]
    fn only_a_signed_transaction_is_sent() {
        let wire = "A".repeat(400);
        assert!(wire_tx_ok(&json!([wire, { "encoding": "base64" }])));
        assert!(!wire_tx_ok(&json!([])));
        assert!(!wire_tx_ok(&json!(["AAAA"])));
        assert!(!wire_tx_ok(&json!(["A".repeat(2000)])));
        assert!(!wire_tx_ok(&json!([format!("{wire}\"")])));
        assert!(!wire_tx_ok(&json!([wire, "base64"])));
    }

    fn balance(index: u64, owner: &str, amount: u64) -> Value {
        json!({
            "accountIndex": index,
            "mint": MINT,
            "owner": owner,
            "programId": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
            "uiTokenAmount": { "amount": amount.to_string(), "decimals": 6 }
        })
    }

    fn paid_tx(memo: &str, seller_gets: u64, fee: u64) -> Value {
        json!({
            "slot": 120,
            "meta": {
                "err": null,
                "preTokenBalances": [balance(1, BUYER, 50_000_000)],
                "postTokenBalances": [
                    balance(1, BUYER, 50_000_000 - seller_gets - fee),
                    balance(2, SELLER, seller_gets),
                    balance(3, TREASURY, fee)
                ]
            },
            "transaction": { "message": { "instructions": [
                { "program": "spl-associated-token-account", "parsed": { "type": "createIdempotent" } },
                { "program": "spl-token", "parsed": { "type": "transferChecked" } },
                { "program": "spl-memo", "programId": MEMO_PROGRAM, "parsed": memo }
            ] } }
        })
    }

    fn expected() -> ExpectedTransfer {
        ExpectedTransfer {
            mint: MINT.to_string(),
            seller: SELLER.to_string(),
            treasury: TREASURY.to_string(),
            seller_amount: 5_760_000,
            fee: 240_000,
            memo: MEMO.to_string(),
        }
    }

    #[test]
    fn a_transfer_counts_only_with_the_memo_and_the_exact_split() {
        assert_eq!(
            transfer_matches(&paid_tx(MEMO, 5_760_000, 240_000), &expected()),
            Ok(())
        );
        let other_memo = MEMO.replace("6b0e", "0000");
        assert_eq!(
            transfer_matches(&paid_tx(&other_memo, 5_760_000, 240_000), &expected()),
            Err("the transaction does not pay this order")
        );
        assert_eq!(
            transfer_matches(&paid_tx(MEMO, 6_000_000, 0), &expected()),
            Err("the payment amount is not the order's")
        );
        assert_eq!(
            transfer_matches(&paid_tx(MEMO, 5_760_000, 200_000), &expected()),
            Err("the payment amount is not the order's")
        );
        let mut failed = paid_tx(MEMO, 5_760_000, 240_000);
        failed["meta"]["err"] = json!({ "InstructionError": [1, "Custom"] });
        assert_eq!(
            transfer_matches(&failed, &expected()),
            Err("the transaction failed")
        );
        // Some other token than Circle's USDC does not count.
        let mut fake = paid_tx(MEMO, 5_760_000, 240_000);
        for b in fake["meta"]["postTokenBalances"].as_array_mut().unwrap() {
            b["mint"] = json!(TREASURY);
        }
        assert_eq!(
            transfer_matches(&fake, &expected()),
            Err("the payment amount is not the order's")
        );
    }

    #[test]
    fn a_seller_account_with_money_in_it_counts_only_the_increase() {
        let mut tx = paid_tx(MEMO, 5_760_000, 240_000);
        tx["meta"]["preTokenBalances"]
            .as_array_mut()
            .unwrap()
            .push(balance(2, SELLER, 1_000_000));
        tx["meta"]["postTokenBalances"][1] = balance(2, SELLER, 6_760_000);
        assert_eq!(transfer_matches(&tx, &expected()), Ok(()));
    }

    /// A real devnet-shaped transaction the admin page's payer sent to a
    /// local validator. Unset, the test does nothing, so `cargo test` stays
    /// offline.
    #[test]
    fn a_validator_transaction_pays_the_order_it_names() {
        let Ok(path) = std::env::var("LOSOS_SOLANA_TX") else {
            return;
        };
        let meta: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("tx file")).unwrap();
        let expected = ExpectedTransfer {
            mint: meta["mint"].as_str().unwrap().to_string(),
            seller: meta["seller"].as_str().unwrap().to_string(),
            treasury: meta["treasury"].as_str().unwrap().to_string(),
            seller_amount: meta["seller_amount"].as_str().unwrap().parse().unwrap(),
            fee: meta["fee"].as_str().unwrap().parse().unwrap(),
            memo: meta["memo"].as_str().unwrap().to_string(),
        };
        transfer_matches(&meta["tx"], &expected).unwrap();
    }
}
