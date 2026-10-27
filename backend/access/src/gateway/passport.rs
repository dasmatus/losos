//! A gateway's passport: who runs it, signed by them with OpenPGP.
//!
//! The owner signs a short statement naming the gateway's URL, its Ed25519
//! key (made on the gateway, private half never leaving it), where their 2%
//! of the market fee goes on each network, and an expiry. The box verifies
//! the signature with the key that came with it, checks the key itself
//! (self-signatures, not revoked, not expired), and shows the key's user id
//! and fingerprint as the gateway's owner. The fingerprint is pinned to the
//! URL the first time a box sees it, so a later passport for the same URL
//! from another key is refused.
//!
//! The statement is plain text so an owner can read what they sign:
//!
//! ```text
//! losos-gateway-passport-v1
//! url: https://gw.example.net
//! edge-key: 3b6a27bc…(64 hex)
//! owner-payout base: 0x52908400098527886E0F7030069857D2E4169EE7
//! owner-payout solana: 7EcDhSYGxXyscszYEp35KHN8vvw3svAuLKTzXwCFLtV
//! not-after: 1798761600
//! ```

use std::collections::BTreeMap;

use pgp::composed::{Deserializable, DetachedSignature, SignedPublicKey};
use pgp::types::KeyDetails;

use crate::fingerprint_hex;

pub const PREFIX: &str = "losos-gateway-passport-v1";
const MAX_STATEMENT: usize = 4096;
const MAX_NETWORKS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PassportError {
    #[error("the passport statement is malformed: {0}")]
    Statement(String),
    #[error("the owner's key is unusable: {0}")]
    Key(String),
    #[error("the owner's signature does not verify")]
    Signature,
    #[error("the passport expired")]
    Expired,
}

/// What the owner signed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    pub url: String,
    /// The gateway's Ed25519 public key, 64 lower-case hex characters.
    pub edge_key: String,
    /// Network name to the owner's payout address there.
    pub owner_payouts: BTreeMap<String, String>,
    pub not_after: i64,
}

fn bad(why: impl Into<String>) -> PassportError {
    PassportError::Statement(why.into())
}

/// A network name: lower-case letters, digits and dashes.
#[must_use]
pub fn network_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A payout address in the shape its network uses: `0x` and 40 hex digits
/// on an EVM network, 32 to 44 base58 characters on Solana.
#[must_use]
pub fn address_ok(network: &str, address: &str) -> bool {
    if network.starts_with("solana") {
        (32..=44).contains(&address.len())
            && address
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() && !matches!(b, b'0' | b'O' | b'I' | b'l'))
    } else {
        address.len() == 42
            && address.starts_with("0x")
            && address[2..].bytes().all(|b| b.is_ascii_hexdigit())
    }
}

/// The address as compared: EVM hex in lower case, Solana as written.
#[must_use]
pub fn normalize_address(network: &str, address: &str) -> String {
    if network.starts_with("solana") {
        address.to_string()
    } else {
        address.to_ascii_lowercase()
    }
}

impl Statement {
    /// The text an owner signs.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = format!("{PREFIX}\nurl: {}\nedge-key: {}\n", self.url, self.edge_key);
        for (network, address) in &self.owner_payouts {
            out.push_str(&format!("owner-payout {network}: {address}\n"));
        }
        out.push_str(&format!("not-after: {}\n", self.not_after));
        out
    }

    /// Parse the exact text that was signed. Every line is required in this
    /// order, nothing else is allowed, and the text must be what
    /// [`Statement::render`] would write back, so two readers can never see
    /// two different statements in the same bytes.
    pub fn parse(text: &str) -> Result<Statement, PassportError> {
        if text.len() > MAX_STATEMENT {
            return Err(bad("longer than a passport can be"));
        }
        let mut lines = text.lines();
        if lines.next() != Some(PREFIX) {
            return Err(bad(format!("the first line must be {PREFIX}")));
        }
        let field = |line: Option<&str>, key: &str| -> Result<String, PassportError> {
            line.and_then(|l| l.strip_prefix(key))
                .and_then(|l| l.strip_prefix(": "))
                .map(str::to_string)
                .ok_or_else(|| bad(format!("expected the {key} line")))
        };
        let url = field(lines.next(), "url")?;
        if !(url.starts_with("http://") || url.starts_with("https://"))
            || url.ends_with('/')
            || url.contains(char::is_whitespace)
        {
            return Err(bad("the url must be http(s) with no trailing slash"));
        }
        let edge_key = field(lines.next(), "edge-key")?;
        if edge_key.len() != 64
            || !edge_key
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(bad("the edge key must be 64 lower-case hex characters"));
        }
        let mut owner_payouts = BTreeMap::new();
        let mut not_after = None;
        for line in lines {
            if let Some(rest) = line.strip_prefix("owner-payout ") {
                let (network, address) = rest
                    .split_once(": ")
                    .ok_or_else(|| bad("an owner-payout line needs a network and an address"))?;
                if !network_ok(network) || !address_ok(network, address) {
                    return Err(bad(format!("not a payout address on {network}: {address}")));
                }
                if owner_payouts
                    .insert(network.to_string(), address.to_string())
                    .is_some()
                {
                    return Err(bad(format!("{network} is named twice")));
                }
            } else if let Some(v) = line.strip_prefix("not-after: ") {
                if not_after.is_some() {
                    return Err(bad("not-after is named twice"));
                }
                not_after = Some(
                    v.parse::<i64>()
                        .map_err(|_| bad("not-after is not a number"))?,
                );
            } else {
                return Err(bad(format!("an unknown line: {line}")));
            }
        }
        if owner_payouts.is_empty() || owner_payouts.len() > MAX_NETWORKS {
            return Err(bad("name between one and sixteen owner payout addresses"));
        }
        let statement = Statement {
            url,
            edge_key,
            owner_payouts,
            not_after: not_after.ok_or_else(|| bad("the not-after line is missing"))?,
        };
        if statement.render() != text {
            return Err(bad(
                "not in the canonical form (field order, one newline each)",
            ));
        }
        Ok(statement)
    }
}

/// The person behind a passport, as their key names them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Owner {
    /// Upper-case hex, no spaces.
    pub fingerprint: String,
    /// The primary user id, `Name <email>` as the owner wrote it.
    pub user_id: String,
}

fn key_err(e: impl std::fmt::Display) -> PassportError {
    PassportError::Key(e.to_string())
}

/// Check the owner's key on its own: it parses, its self-signatures verify,
/// it carries no revocation, it has a user id, and it has not expired.
pub fn check_owner_key(armored: &str, now: i64) -> Result<(SignedPublicKey, Owner), PassportError> {
    let (key, _) = SignedPublicKey::from_string(armored.trim_start()).map_err(key_err)?;
    key.verify_bindings().map_err(key_err)?;
    if !key.details.revocation_signatures.is_empty() {
        return Err(PassportError::Key("the key is revoked".into()));
    }
    let user = key
        .details
        .users
        .iter()
        .find(|u| u.is_primary())
        .or_else(|| key.details.users.first())
        .ok_or_else(|| PassportError::Key("the key has no user id".into()))?;
    let created = i64::from(key.created_at().as_secs());
    let expires = user
        .signatures
        .iter()
        .filter_map(|s| s.key_expiration_time())
        .map(|d| created + i64::from(d.as_secs()))
        .max();
    if expires.is_some_and(|at| now >= at) {
        return Err(PassportError::Key("the key has expired".into()));
    }
    let owner = Owner {
        fingerprint: fingerprint_hex(&key.fingerprint()),
        user_id: String::from_utf8_lossy(user.id.id()).into_owned(),
    };
    Ok((key, owner))
}

/// Did `key` (its primary key, or a bound signing subkey) make the armored
/// detached `signature` over exactly `data`?
#[must_use]
pub fn signed_by(key: &SignedPublicKey, signature: &str, data: &[u8]) -> bool {
    let Ok((sig, _)) = DetachedSignature::from_string(signature.trim_start()) else {
        return false;
    };
    sig.verify(&key.primary_key, data).is_ok()
        || key.public_subkeys.iter().any(|sub| {
            sub.verify_bindings(&key.primary_key).is_ok()
                && sub.signatures.iter().any(|s| s.key_flags().sign())
                && sig.verify(&sub.key, data).is_ok()
        })
}

/// Verify a passport: the owner's key, their detached signature over the
/// statement's exact bytes (by the primary key or a signing subkey), the
/// statement's form and its expiry.
pub fn verify(
    statement: &str,
    signature: &str,
    owner_key: &str,
    now: i64,
) -> Result<(Statement, Owner), PassportError> {
    let (key, owner) = check_owner_key(owner_key, now)?;
    if !signed_by(&key, signature, statement.as_bytes()) {
        return Err(PassportError::Signature);
    }
    let parsed = Statement::parse(statement)?;
    if now >= parsed.not_after {
        return Err(PassportError::Expired);
    }
    Ok((parsed, owner))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use pgp::composed::{ArmorOptions, KeyType, SecretKeyParamsBuilder, SignedSecretKey};
    use pgp::crypto::hash::HashAlgorithm;
    use pgp::types::Password;

    pub(crate) fn owner_key(user_id: &str) -> SignedSecretKey {
        SecretKeyParamsBuilder::default()
            .key_type(KeyType::Ed25519Legacy)
            .can_certify(true)
            .can_sign(true)
            .primary_user_id(user_id.into())
            .build()
            .unwrap()
            .generate(rand::thread_rng())
            .unwrap()
    }

    pub(crate) fn sign(key: &SignedSecretKey, text: &str) -> String {
        DetachedSignature::sign_binary_data(
            rand::thread_rng(),
            &key.primary_key,
            &Password::empty(),
            HashAlgorithm::Sha256,
            text.as_bytes(),
        )
        .unwrap()
        .to_armored_string(ArmorOptions::default())
        .unwrap()
    }

    pub(crate) fn armored_public(key: &SignedSecretKey) -> String {
        key.to_public_key()
            .to_armored_string(ArmorOptions::default())
            .unwrap()
    }

    pub(crate) fn statement(url: &str, edge_key: &str) -> Statement {
        let mut owner_payouts = BTreeMap::new();
        owner_payouts.insert(
            "base".to_string(),
            "0x52908400098527886E0F7030069857D2E4169EE7".to_string(),
        );
        Statement {
            url: url.into(),
            edge_key: edge_key.into(),
            owner_payouts,
            not_after: 2_000_000_000,
        }
    }

    #[test]
    fn a_signed_passport_names_its_owner_and_a_forged_one_does_not() {
        let key = owner_key("Ada Gateway <ada@example.net>");
        let text = statement("http://gw.local:8080", &"ab".repeat(32)).render();
        let sig = sign(&key, &text);
        let (parsed, owner) = verify(&text, &sig, &armored_public(&key), 1_800_000_000).unwrap();
        assert_eq!(owner.user_id, "Ada Gateway <ada@example.net>");
        assert_eq!(owner.fingerprint.len(), 40);
        assert_eq!(parsed.url, "http://gw.local:8080");

        // Another URL under the same signature, another signer, an expired one.
        let other = text.replace("gw.local", "gw.evil");
        assert_eq!(
            verify(&other, &sig, &armored_public(&key), 1_800_000_000),
            Err(PassportError::Signature)
        );
        let mallory = owner_key("Mallory <m@example.net>");
        assert_eq!(
            verify(&text, &sig, &armored_public(&mallory), 1_800_000_000),
            Err(PassportError::Signature)
        );
        assert_eq!(
            verify(&text, &sig, &armored_public(&key), 2_000_000_000),
            Err(PassportError::Expired)
        );
    }

    #[test]
    fn the_statement_has_one_spelling() {
        let s = statement("https://gw.example.net", &"0f".repeat(32));
        assert_eq!(Statement::parse(&s.render()).unwrap(), s);
        let text = s.render();
        for broken in [
            text.replace("url: ", "url:"),
            text.replace("\n", "\r\n"),
            format!("{text}\n"),
            text.replace("edge-key: 0f", "edge-key: 0F"),
            text.replace("not-after", "expires"),
            text.replace("0x5290", "0xZZ90"),
            text.replace("https://gw.example.net", "https://gw.example.net/"),
            format!("{text}owner-payout base: 0x52908400098527886E0F7030069857D2E4169EE7\n"),
        ] {
            assert!(Statement::parse(&broken).is_err(), "{broken:?}");
        }
    }

    #[test]
    fn addresses_are_shaped_per_network() {
        assert!(address_ok(
            "base",
            "0x52908400098527886E0F7030069857D2E4169EE7"
        ));
        assert!(!address_ok(
            "base",
            "52908400098527886E0F7030069857D2E4169EE7"
        ));
        assert!(address_ok(
            "solana",
            "7EcDhSYGxXyscszYEp35KHN8vvw3svAuLKTzXwCFLtV"
        ));
        assert!(!address_ok(
            "solana",
            "0x52908400098527886E0F7030069857D2E4169EE7"
        ));
        assert!(network_ok("arbitrum") && !network_ok("Base") && !network_ok(""));
    }
}
