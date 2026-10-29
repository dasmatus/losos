//! This box's escrow keys, and which of the edge's decisions it signs.
//!
//! A USDC payment held in escrow moves only when two of three keys agree:
//! the edge's, the buyer's box's and the seller's box's
//! (`edge/contracts/src/LososEscrow.sol`, `edge/contracts/solana`). The two
//! box keys come from one secret, the `market-escrow-key` credential: a
//! secp256k1 key for EVM networks and an ed25519 key for Solana, each
//! derived from it by SHA-256 with a label of its own, so the secret itself
//! never signs anything.
//!
//! The edge decides; the box agrees only to what pays its own owner. The
//! seller's box signs a release (nothing back to the buyer) of one of its
//! own sales. The buyer's box signs a refund or a split of one of its own
//! purchases, or the full refund of a payment no order took. Everything
//! else is refused here, whatever the edge asks, so a stolen edge key and a
//! box's signature never add up to a move against that box's owner.

use k256::ecdsa::SigningKey;
use ring::signature::{Ed25519KeyPair, KeyPair};
use serde_json::{json, Value};
use sha3::{Digest, Keccak256};

const ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// The box's two signing keys.
pub struct Keys {
    evm: SigningKey,
    solana: Ed25519KeyPair,
}

impl std::fmt::Debug for Keys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keys")
            .field("evm", &self.evm_address())
            .field("solana", &self.solana_address())
            .finish()
    }
}

/// What the box knows of its own trades, from `/market/account`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Mine {
    pub purchases: Vec<String>,
    pub sales: Vec<String>,
}

impl Mine {
    /// The order ids of the account view's `purchases` and `sales`.
    #[must_use]
    pub fn from_account(account: &Value) -> Self {
        let ids = |key: &str| {
            account[key]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|o| o["id"].as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        Self {
            purchases: ids("purchases"),
            sales: ids("sales"),
        }
    }
}

fn derive(label: &[u8], seed: &[u8; 32]) -> [u8; 32] {
    let mut ctx = ring::digest::Context::new(&ring::digest::SHA256);
    ctx.update(label);
    ctx.update(seed);
    ctx.finish().as_ref().try_into().expect("32 bytes")
}

fn keccak(data: &[u8]) -> [u8; 32] {
    Keccak256::digest(data).into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(raw: &str) -> Option<Vec<u8>> {
    let raw = raw.strip_prefix("0x").unwrap_or(raw);
    if !raw.len().is_multiple_of(2) || !raw.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    (0..raw.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&raw[i..i + 2], 16).ok())
        .collect()
}

#[must_use]
pub fn base58_encode(bytes: &[u8]) -> String {
    let zeros = bytes.iter().take_while(|b| **b == 0).count();
    let mut digits: Vec<u8> = Vec::new();
    for b in bytes {
        let mut carry = u32::from(*b);
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
    let mut out = "1".repeat(zeros);
    out.extend(digits.iter().rev().map(|d| ALPHABET[*d as usize] as char));
    out
}

/// A 32-byte key or account, from base58.
#[must_use]
pub fn base58_key(raw: &str) -> Option<[u8; 32]> {
    if raw.is_empty() || raw.len() > 44 {
        return None;
    }
    let zeros = raw.bytes().take_while(|b| *b == b'1').count();
    let mut bytes: Vec<u8> = Vec::new();
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
    out.try_into().ok()
}

/// The EIP-712 digest `LososEscrow.decisionDigest(depositId, refund)` signs:
/// `Decision(uint256 depositId,uint256 refund)` in the domain `LososEscrow`,
/// version `2`.
#[must_use]
pub fn evm_digest(chain_id: u64, escrow: [u8; 20], deposit: u64, refund: u64) -> [u8; 32] {
    let word = |v: u64| {
        let mut w = [0u8; 32];
        w[24..].copy_from_slice(&v.to_be_bytes());
        w
    };
    let mut domain = Vec::with_capacity(5 * 32);
    domain.extend_from_slice(&keccak(
        b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
    ));
    domain.extend_from_slice(&keccak(b"LososEscrow"));
    domain.extend_from_slice(&keccak(b"2"));
    domain.extend_from_slice(&word(chain_id));
    let mut contract = [0u8; 32];
    contract[12..].copy_from_slice(&escrow);
    domain.extend_from_slice(&contract);
    let mut decision = Vec::with_capacity(3 * 32);
    decision.extend_from_slice(&keccak(b"Decision(uint256 depositId,uint256 refund)"));
    decision.extend_from_slice(&word(deposit));
    decision.extend_from_slice(&word(refund));
    let mut out = vec![0x19, 0x01];
    out.extend_from_slice(&keccak(&domain));
    out.extend_from_slice(&keccak(&decision));
    keccak(&out)
}

/// What a box signs on Solana: the program's `decision_message`.
#[must_use]
pub fn solana_message(deposit: &[u8; 32], refund: u64) -> [u8; 64] {
    let mut m = [0u8; 64];
    m[..24].copy_from_slice(b"losos-escrow decision v1");
    m[24..56].copy_from_slice(deposit);
    m[56..].copy_from_slice(&refund.to_le_bytes());
    m
}

impl Keys {
    /// Both keys from the credential's 32 bytes.
    ///
    /// # Errors
    /// Only if a derived secp256k1 scalar is zero or past the order, which
    /// SHA-256 output is with probability about 2^-128.
    pub fn from_seed(seed: &[u8; 32]) -> anyhow::Result<Self> {
        let evm = SigningKey::from_slice(&derive(b"losos escrow evm v1", seed))
            .map_err(|_| anyhow::anyhow!("the escrow secret makes no secp256k1 key"))?;
        let solana = Ed25519KeyPair::from_seed_unchecked(&derive(b"losos escrow solana v1", seed))
            .map_err(|_| anyhow::anyhow!("the escrow secret makes no ed25519 key"))?;
        Ok(Self { evm, solana })
    }

    /// The credential file's text: 64 hex characters.
    ///
    /// # Errors
    /// When the text is anything else.
    pub fn from_hex(text: &str) -> anyhow::Result<Self> {
        let raw = unhex(text.trim())
            .filter(|b| b.len() == 32)
            .ok_or_else(|| anyhow::anyhow!("the escrow secret is not 32 bytes of hex"))?;
        Self::from_seed(raw.as_slice().try_into().expect("32 bytes"))
    }

    /// `0x` and 40 lowercase hex digits.
    #[must_use]
    pub fn evm_address(&self) -> String {
        let point = self.evm.verifying_key().to_encoded_point(false);
        format!("0x{}", hex(&keccak(&point.as_bytes()[1..])[12..]))
    }

    /// Base58, 32 bytes.
    #[must_use]
    pub fn solana_address(&self) -> String {
        base58_encode(self.solana.public_key().as_ref())
    }

    /// The key for one family of networks.
    #[must_use]
    pub fn address_of(&self, solana: bool) -> String {
        if solana {
            self.solana_address()
        } else {
            self.evm_address()
        }
    }

    /// `0x` + `r ‖ s ‖ v`, `v` 27 or 28, `s` low: the one form the contract
    /// takes.
    fn sign_evm(&self, digest: &[u8; 32]) -> anyhow::Result<String> {
        let (sig, recid) = self
            .evm
            .sign_prehash_recoverable(digest)
            .map_err(|_| anyhow::anyhow!("could not sign the decision"))?;
        let (sig, recid) = match sig.normalize_s() {
            Some(low) => (
                low,
                k256::ecdsa::RecoveryId::new(!recid.is_y_odd(), recid.is_x_reduced()),
            ),
            None => (sig, recid),
        };
        let mut out = sig.to_bytes().to_vec();
        out.push(27 + recid.to_byte());
        Ok(format!("0x{}", hex(&out)))
    }

    /// The body of `/market/escrow/sign` for one decision the edge listed,
    /// or why this box will not sign it.
    ///
    /// # Errors
    /// A sentence for the journal: a decision that is not well formed, or one
    /// that would move money against this box's owner.
    pub fn sign(&self, d: &Value, mine: &Mine) -> anyhow::Result<Value> {
        let field = |k: &str| d[k].as_str();
        let network = field("network")
            .filter(|n| {
                !n.is_empty()
                    && n.len() <= 32
                    && n.bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            })
            .ok_or_else(|| anyhow::anyhow!("the decision names no network"))?;
        let refund: u64 = field("refund")
            .and_then(|r| r.parse().ok())
            .ok_or_else(|| anyhow::anyhow!("the decision names no refund"))?;
        let gross: u64 = field("gross")
            .and_then(|r| r.parse().ok())
            .filter(|g| *g > 0)
            .ok_or_else(|| anyhow::anyhow!("the decision names no gross"))?;
        if refund > gross {
            anyhow::bail!("the decision gives back more than was paid");
        }
        let order = field("orderId");
        // Who the decision pays, by the money, not by what the edge says.
        let allowed = match order {
            Some(id) if refund == 0 => mine.sales.iter().any(|s| s == id),
            Some(id) => mine.purchases.iter().any(|p| p == id),
            // A payment no order took only ever goes back whole.
            None => refund == gross,
        };
        if !allowed {
            anyhow::bail!(
                "the edge asked this box to agree to a move that does not pay its owner ({network}, order {}, refund {refund})",
                order.unwrap_or("none")
            );
        }
        if d["family"] == "solana" {
            let account = field("account")
                .ok_or_else(|| anyhow::anyhow!("the decision names no deposit account"))?;
            let key = base58_key(account)
                .ok_or_else(|| anyhow::anyhow!("the deposit account is not a Solana key"))?;
            let signature = self.solana.sign(&solana_message(&key, refund));
            Ok(json!({
                "network": network,
                "account": account,
                "refund": refund,
                "signature": base58_encode(signature.as_ref()),
            }))
        } else {
            let chain_id = d["chainId"]
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("the decision names no chain"))?;
            let escrow: [u8; 20] = field("escrow")
                .and_then(unhex)
                .and_then(|b| b.try_into().ok())
                .ok_or_else(|| anyhow::anyhow!("the decision names no escrow contract"))?;
            let deposit = d["deposit"]
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("the decision names no deposit"))?;
            let signature = self.sign_evm(&evm_digest(chain_id, escrow, deposit, refund))?;
            Ok(json!({
                "network": network,
                "deposit": deposit,
                "refund": refund,
                "signature": signature,
            }))
        }
    }

    /// Whether an invoice the edge just made names this box's own keys as the
    /// buyer's box. A wallet pays what the invoice says, and a deposit naming
    /// another key would let whoever holds it agree in this box's place.
    ///
    /// # Errors
    /// The chain whose invoice names another key.
    pub fn check_invoice(&self, invoice: &Value) -> anyhow::Result<()> {
        for chain in invoice["chains"].as_array().into_iter().flatten() {
            let Some(named) = chain["buyer_box"].as_str() else {
                continue;
            };
            let solana = chain["network"]
                .as_str()
                .is_some_and(|n| n.starts_with("solana"));
            let mine = self.address_of(solana);
            let same = if solana {
                named == mine
            } else {
                named.eq_ignore_ascii_case(&mine)
            };
            if !same {
                anyhow::bail!(
                    "the edge's invoice names another key than this box's for its escrow on {}",
                    chain["network"].as_str().unwrap_or("a network")
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};

    fn keys() -> Keys {
        Keys::from_seed(&[7u8; 32]).unwrap()
    }

    fn recover(digest: &[u8; 32], sig: &str) -> String {
        let raw = unhex(sig).unwrap();
        assert_eq!(raw.len(), 65);
        let s = Signature::from_slice(&raw[..64]).unwrap();
        assert!(s.normalize_s().is_none(), "s is low");
        assert!(raw[64] == 27 || raw[64] == 28);
        let id = RecoveryId::from_byte(raw[64] - 27).unwrap();
        let key = VerifyingKey::recover_from_prehash(digest, &s, id).unwrap();
        let point = key.to_encoded_point(false);
        format!("0x{}", hex(&keccak(&point.as_bytes()[1..])[12..]))
    }

    #[test]
    fn the_digest_is_the_contracts() {
        // The same value the registrar's test pins, made with `cast` from the
        // contract's domain and type strings.
        let escrow: [u8; 20] = unhex("0x5615dEB798BB3E4dFa0139dFa1b3D433Cc23b72f")
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(
            hex(&evm_digest(31337, escrow, 1, 0)),
            "133d6c0eebdd8695575a8241a3a36bfa091c7648e7e52111382e2188233b7e21"
        );
    }

    #[test]
    fn the_keys_are_the_secrets_own_and_differ_by_family() {
        let a = keys();
        let b = Keys::from_seed(&[7u8; 32]).unwrap();
        assert_eq!(a.evm_address(), b.evm_address());
        assert_eq!(a.solana_address(), b.solana_address());
        let c = Keys::from_seed(&[8u8; 32]).unwrap();
        assert_ne!(a.evm_address(), c.evm_address());
        assert_ne!(a.solana_address(), c.solana_address());
        assert!(a.evm_address().starts_with("0x") && a.evm_address().len() == 42);
        assert_eq!(base58_key(&a.solana_address()).map(|k| k.len()), Some(32));
        let text = "07".repeat(32);
        assert_eq!(
            Keys::from_hex(&format!("{text}\n")).unwrap().evm_address(),
            a.evm_address()
        );
        assert!(Keys::from_hex("07").is_err());
    }

    fn decision(refund: u64, order: Option<&str>) -> Value {
        let mut d = json!({
            "network": "base-sepolia",
            "family": "evm",
            "chainId": 84532,
            "escrow": "0x5615deb798bb3e4dfa0139dfa1b3d433cc23b72f",
            "deposit": 3,
            "side": if refund == 0 { "seller" } else { "buyer" },
            "step": "release",
            "refund": refund.to_string(),
            "gross": "2000000",
        });
        if let Some(o) = order {
            d["orderId"] = json!(o);
        }
        d
    }

    #[test]
    fn a_box_signs_only_what_pays_its_owner() {
        let k = keys();
        let mine = Mine {
            purchases: vec!["ord_bought".into()],
            sales: vec!["ord_sold".into()],
        };
        // A release of its own sale, and a refund or split of its purchase.
        let body = k.sign(&decision(0, Some("ord_sold")), &mine).unwrap();
        let escrow: [u8; 20] = unhex("0x5615deb798bb3e4dfa0139dfa1b3d433cc23b72f")
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(
            recover(
                &evm_digest(84532, escrow, 3, 0),
                body["signature"].as_str().unwrap()
            ),
            k.evm_address()
        );
        assert_eq!(body["deposit"], 3);
        assert!(k
            .sign(&decision(2_000_000, Some("ord_bought")), &mine)
            .is_ok());
        assert!(k
            .sign(&decision(500_000, Some("ord_bought")), &mine)
            .is_ok());
        // Never a release of what it bought, nor a refund of what it sold.
        assert!(k.sign(&decision(0, Some("ord_bought")), &mine).is_err());
        assert!(k.sign(&decision(500_000, Some("ord_sold")), &mine).is_err());
        assert!(k.sign(&decision(0, Some("ord_someone")), &mine).is_err());
        // A stray goes back whole or not at all.
        assert!(k.sign(&decision(2_000_000, None), &mine).is_ok());
        assert!(k.sign(&decision(1, None), &mine).is_err());
        assert!(k.sign(&decision(0, None), &mine).is_err());
        // More back than was paid is no decision.
        assert!(k
            .sign(&decision(3_000_000, Some("ord_bought")), &mine)
            .is_err());
    }

    #[test]
    fn a_solana_decision_signs_the_programs_message() {
        let k = keys();
        let account = base58_encode(&[4u8; 32]);
        let d = json!({
            "network": "solana-devnet",
            "family": "solana",
            "escrow": "Esc",
            "account": account,
            "side": "buyer",
            "step": "refund",
            "refund": "40",
            "gross": "40",
            "orderId": "ord_bought",
        });
        let mine = Mine {
            purchases: vec!["ord_bought".into()],
            sales: vec![],
        };
        let body = k.sign(&d, &mine).unwrap();
        assert_eq!(body["account"], account.as_str());
        let sig = {
            // base58 to bytes, any length, for the check.
            let raw = body["signature"].as_str().unwrap();
            let mut bytes: Vec<u8> = Vec::new();
            for c in raw.bytes() {
                let mut carry = ALPHABET.iter().position(|a| *a == c).unwrap() as u32;
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
            bytes.reverse();
            bytes
        };
        let public = ring::signature::UnparsedPublicKey::new(
            &ring::signature::ED25519,
            base58_key(&k.solana_address()).unwrap(),
        );
        assert!(public.verify(&solana_message(&[4u8; 32], 40), &sig).is_ok());
    }

    #[test]
    fn an_invoice_must_name_this_box_as_the_buyer() {
        let k = keys();
        let good = json!({ "chains": [
            { "network": "base-sepolia", "buyer_box": k.evm_address().to_uppercase().replace("0X", "0x") },
            { "network": "solana-devnet", "buyer_box": k.solana_address() },
            { "network": "base", "pay": null },
        ]});
        assert!(k.check_invoice(&good).is_ok());
        let bad = json!({ "chains": [
            { "network": "base-sepolia", "buyer_box": "0x00000000000000000000000000000000000000b1" },
        ]});
        assert!(k.check_invoice(&bad).is_err());
        assert!(k.check_invoice(&json!({})).is_ok());
    }

    #[test]
    fn base58_round_trips_a_key() {
        for key in [[0u8; 32], [1u8; 32], [255u8; 32]] {
            assert_eq!(base58_key(&base58_encode(&key)), Some(key));
        }
        assert_eq!(base58_key("0OIl"), None);
    }
}
