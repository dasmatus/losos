//! The two boxes' keys on an escrowed deposit.
//!
//! `LososEscrow` and `losos-escrow` move a deposit only when two of three
//! keys agree: the edge's releaser, the buyer's box and the seller's box.
//! The wallet names the two box keys when it pays, from the invoice, and
//! the edge counts a payment only when they are the keys the two boxes
//! registered here. Each decision the edge makes then waits for one box to
//! sign it: the seller's box a release, the buyer's box a refund or a
//! split. The edge's own signature is the transaction it sends, so a stolen
//! edge key alone moves nothing.
//!
//! A box keeps one secret for this (`market-escrow-key` on the box) and
//! signs with a secp256k1 key on EVM networks and an ed25519 key on
//! Solana. On EVM a decision is EIP-712 typed data,
//! `Decision(uint256 depositId,uint256 refund)` in the domain
//! `LososEscrow` version `2`; on Solana it is [`solana_message`].

use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::stablecoin::{address_of, decode_hex, keccak256};

/// A box's escrow keys as it registered them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoxKeys {
    /// `0x` + 40 lowercase hex.
    pub evm: String,
    /// Base58, 32 bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solana: Option<String>,
}

impl BoxKeys {
    /// Both keys in their one written form, or why not.
    pub fn parse(evm: &str, solana: Option<&str>) -> Result<Self, &'static str> {
        let evm = crate::stablecoin::address_bytes(evm)
            .filter(|a| a.iter().any(|b| *b != 0))
            .ok_or("evm is not an address")?;
        let solana = match solana {
            Some(raw) => Some(
                crate::solana_escrow::key(raw)
                    .filter(crate::solana_escrow::on_curve)
                    .map(|k| crate::solana::base58_encode(&k))
                    .ok_or("solana is not a key")?,
            ),
            None => None,
        };
        Ok(Self {
            evm: format!("0x{}", crate::identity::to_hex(&evm)),
            solana,
        })
    }

    /// The key on a network of `family`.
    #[must_use]
    pub fn of(&self, family: crate::stablecoin::Family) -> Option<&str> {
        match family {
            crate::stablecoin::Family::Evm => Some(&self.evm),
            crate::stablecoin::Family::Solana => self.solana.as_deref(),
        }
    }
}

/// The two box keys an order was invoiced with, one pair per family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderBoxes {
    pub buyer: BoxKeys,
    pub seller: BoxKeys,
}

/// What a held deposit names, as the chain wrote it: the keys that may
/// agree with the edge, and the gross a full refund gives back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepositKeys {
    pub buyer_box: String,
    pub seller_box: String,
    /// USDC base units.
    pub gross: u64,
}

/// Whose box signs a decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Buyer,
    Seller,
}

/// One box's signature over one decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Approval {
    /// The refund the signature agrees to, USDC base units: 0 a release,
    /// the gross a refund, anything between a split.
    pub refund: u64,
    pub by: Side,
    /// EVM: `0x` + 65 bytes `r ‖ s ‖ v`. Solana: base58, 64 bytes.
    pub signature: String,
}

/// The EIP-712 digest `LososEscrow.decisionDigest` computes.
#[must_use]
pub fn evm_digest(chain_id: u64, escrow: [u8; 20], deposit: u64, refund: u128) -> [u8; 32] {
    let word = |v: u128| {
        let mut w = [0u8; 32];
        w[16..].copy_from_slice(&v.to_be_bytes());
        w
    };
    let mut domain = Vec::with_capacity(5 * 32);
    domain.extend_from_slice(&keccak256(
        b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
    ));
    domain.extend_from_slice(&keccak256(b"LososEscrow"));
    domain.extend_from_slice(&keccak256(b"2"));
    domain.extend_from_slice(&word(u128::from(chain_id)));
    let mut contract = [0u8; 32];
    contract[12..].copy_from_slice(&escrow);
    domain.extend_from_slice(&contract);
    let mut decision = Vec::with_capacity(3 * 32);
    decision.extend_from_slice(&keccak256(b"Decision(uint256 depositId,uint256 refund)"));
    decision.extend_from_slice(&word(u128::from(deposit)));
    decision.extend_from_slice(&word(refund));
    let mut out = vec![0x19, 0x01];
    out.extend_from_slice(&keccak256(&domain));
    out.extend_from_slice(&keccak256(&decision));
    keccak256(&out)
}

/// The address behind a 65-byte `r ‖ s ‖ v` signature of `digest`, under
/// the contract's own rules: low `s`, `v` 27 or 28. `None` otherwise.
#[must_use]
pub fn evm_signer(digest: &[u8; 32], signature_hex: &str) -> Option<String> {
    let sig = decode_hex(signature_hex)?;
    if sig.len() != 65 {
        return None;
    }
    let signature = Signature::from_slice(&sig[..64]).ok()?;
    if signature.normalize_s() != signature {
        return None;
    }
    let recid = match sig[64] {
        27 => RecoveryId::try_from(0u8).ok()?,
        28 => RecoveryId::try_from(1u8).ok()?,
        _ => return None,
    };
    let key = VerifyingKey::recover_from_prehash(digest, &signature, recid).ok()?;
    Some(address_of(&key))
}

/// What a box signs on Solana: `losos_escrow::decision_message`.
#[must_use]
pub fn solana_message(deposit: &[u8; 32], refund: u64) -> [u8; 64] {
    let mut m = [0u8; 64];
    m[..24].copy_from_slice(b"losos-escrow decision v1");
    m[24..56].copy_from_slice(deposit);
    m[56..].copy_from_slice(&refund.to_le_bytes());
    m
}

/// Whether `signature` (base58) is `key`'s (base58) over `message`.
#[must_use]
pub fn solana_verifies(key: &str, message: &[u8], signature: &str) -> bool {
    crate::solana::verify_message(message, key, signature)
}

/// The refund a step asks the keys to agree on, or `None` when the step
/// sends nothing.
#[must_use]
pub fn refund_of(step: crate::market::EscrowStep, gross: u64, split: Option<u64>) -> Option<u64> {
    use crate::market::EscrowStep;
    match step {
        EscrowStep::Release => Some(0),
        EscrowStep::Refund => Some(gross),
        EscrowStep::Split => split,
        _ => None,
    }
}

/// The box whose owner a decision pays: the seller's for a release, the
/// buyer's for a refund or a split.
#[must_use]
pub fn favoured(refund: u64) -> Side {
    if refund == 0 {
        Side::Seller
    } else {
        Side::Buyer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k256::ecdsa::SigningKey;

    fn sign(key: &SigningKey, digest: &[u8; 32]) -> String {
        let (sig, recid) = key.sign_prehash_recoverable(digest);
        let mut out = sig.to_bytes().to_vec();
        out.push(27 + u8::from(recid));
        format!("0x{}", crate::identity::to_hex(&out))
    }

    #[test]
    fn the_digest_is_the_contracts() {
        // Built by hand with `cast keccak` and `cast abi-encode` from the
        // contract's domain and type strings.
        let escrow =
            crate::stablecoin::address_bytes("0x5615dEB798BB3E4dFa0139dFa1b3D433Cc23b72f").unwrap();
        let d = evm_digest(31337, escrow, 1, 0);
        assert_eq!(
            crate::identity::to_hex(&d),
            "133d6c0eebdd8695575a8241a3a36bfa091c7648e7e52111382e2188233b7e21"
        );
        assert_ne!(d, evm_digest(31337, escrow, 1, 1));
        assert_ne!(d, evm_digest(31337, escrow, 2, 0));
        assert_ne!(d, evm_digest(1, escrow, 1, 0));
    }

    #[test]
    fn a_signature_recovers_only_in_its_one_form() {
        let key = SigningKey::from_slice(&[7u8; 32]).unwrap();
        let me = address_of(key.verifying_key());
        let digest = evm_digest(84532, [9u8; 20], 3, 5);
        let sig = sign(&key, &digest);
        assert_eq!(evm_signer(&digest, &sig).as_deref(), Some(me.as_str()));
        // Another decision recovers someone else.
        let other = evm_digest(84532, [9u8; 20], 3, 6);
        assert_ne!(evm_signer(&other, &sig).as_deref(), Some(me.as_str()));
        // v as 0 or 1, a high s, a short signature: refused.
        let mut raw = decode_hex(&sig).unwrap();
        let v = raw[64];
        raw[64] = v - 27;
        assert!(evm_signer(&digest, &format!("0x{}", crate::identity::to_hex(&raw))).is_none());
        raw[64] = v;
        let s = Signature::from_slice(&raw[..64]).unwrap();
        let (r, s) = s.split_scalars();
        let high = Signature::from_scalars(r, -*s).unwrap();
        let mut flipped = high.to_bytes().to_vec();
        flipped.push(if v == 27 { 28 } else { 27 });
        assert!(evm_signer(&digest, &format!("0x{}", crate::identity::to_hex(&flipped))).is_none());
        assert!(evm_signer(&digest, "0x1234").is_none());
    }

    #[test]
    fn keys_have_one_written_form() {
        let k = BoxKeys::parse("0xAbCDEF0123456789abcdef0123456789ABCDEF01", None).unwrap();
        assert_eq!(k.evm, "0xabcdef0123456789abcdef0123456789abcdef01");
        assert!(BoxKeys::parse("0x0000000000000000000000000000000000000000", None).is_err());
        assert!(BoxKeys::parse("0x12", None).is_err());
        // A program address is off the curve, so no one holds its key.
        let pda = crate::solana_escrow::find_program_address(&[b"box"], &[1u8; 32]).0;
        let pda = crate::solana::base58_encode(&pda);
        assert!(BoxKeys::parse("0xabcdef0123456789abcdef0123456789abcdef01", Some(&pda)).is_err());
        let pair = ring::signature::Ed25519KeyPair::from_seed_unchecked(&[3u8; 32]).unwrap();
        let sol =
            crate::solana::base58_encode(ring::signature::KeyPair::public_key(&pair).as_ref());
        let k = BoxKeys::parse("0xabcdef0123456789abcdef0123456789abcdef01", Some(&sol)).unwrap();
        assert_eq!(k.of(crate::stablecoin::Family::Solana), Some(sol.as_str()));
    }

    #[test]
    fn a_solana_decision_is_the_programs_message() {
        let pair = ring::signature::Ed25519KeyPair::from_seed_unchecked(&[5u8; 32]).unwrap();
        let key =
            crate::solana::base58_encode(ring::signature::KeyPair::public_key(&pair).as_ref());
        let m = solana_message(&[1u8; 32], 7);
        assert_eq!(&m[..24], b"losos-escrow decision v1");
        let sig = crate::solana::base58_encode(pair.sign(&m).as_ref());
        assert!(solana_verifies(&key, &m, &sig));
        assert!(!solana_verifies(&key, &solana_message(&[1u8; 32], 8), &sig));
    }

    #[test]
    fn each_step_asks_its_own_box() {
        use crate::market::EscrowStep;
        assert_eq!(refund_of(EscrowStep::Release, 20, None), Some(0));
        assert_eq!(refund_of(EscrowStep::Refund, 20, None), Some(20));
        assert_eq!(refund_of(EscrowStep::Split, 20, Some(5)), Some(5));
        assert_eq!(refund_of(EscrowStep::Held, 20, None), None);
        assert_eq!(favoured(0), Side::Seller);
        assert_eq!(favoured(5), Side::Buyer);
    }
}
