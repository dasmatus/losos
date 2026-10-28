//! Keys and hex: an edge's Ed25519 key pair (born on the edge, the private
//! half never leaving it) and the crate's one pair of hex helpers.
//!
//! The key is what an open gateway's passport names (`crate::gateway`): the
//! owner signs a statement carrying its public half, and the edge proves it
//! holds the private half by signing each box's nonce.

use std::path::Path;

use miette::{miette, Context, IntoDiagnostic, Result};
use ring::rand::{SecureRandom, SystemRandom};
use ring::signature::{Ed25519KeyPair, KeyPair};

// ── hex, by hand: two short functions instead of a crate ───────────────────
//
// The crate's one pair: every other module encodes and decodes through these.

#[must_use]
pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `None` for odd length or a non-hex character.
#[must_use]
pub fn from_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

/// `bytes` fresh random bytes as hex; `None` when the system source fails.
#[must_use]
pub fn random_hex(bytes: usize) -> Option<String> {
    let mut raw = vec![0u8; bytes];
    SystemRandom::new().fill(&mut raw).ok()?;
    Some(to_hex(&raw))
}

// ── keys ───────────────────────────────────────────────────────────────────

/// A fresh Ed25519 key pair, as the PKCS#8 document `ring` emits (hex) and
/// the public key (hex). The document is what a key file holds.
pub fn keygen() -> Result<(String, String)> {
    let rng = SystemRandom::new();
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng)
        .map_err(|_| miette!("the system random source failed"))?;
    let pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref())
        .map_err(|_| miette!("ring rejected the key it just made"))?;
    Ok((to_hex(pkcs8.as_ref()), to_hex(pair.public_key().as_ref())))
}

/// Load a key pair from the hex PKCS#8 a key file holds.
pub fn load_key(hex: &str) -> Result<Ed25519KeyPair> {
    let bytes = from_hex(hex.trim()).ok_or_else(|| miette!("the key file is not hex"))?;
    Ed25519KeyPair::from_pkcs8(&bytes)
        .map_err(|_| miette!("the key file does not hold an Ed25519 PKCS#8 key"))
}

/// Sign `msg` with `key`, hex.
#[must_use]
pub fn sign(key: &Ed25519KeyPair, msg: &[u8]) -> String {
    to_hex(key.sign(msg).as_ref())
}

/// Write a key file: created 0600, never over an existing file.
pub(crate) fn write_private(path: &Path, hex: &str) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("creating {}", path.display()))?;
    f.write_all(hex.as_bytes()).into_diagnostic()?;
    f.write_all(b"\n").into_diagnostic()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_and_rejects_garbage() {
        assert_eq!(to_hex(&[0, 15, 255]), "000fff");
        assert_eq!(from_hex("000fff"), Some(vec![0, 15, 255]));
        assert_eq!(from_hex("0"), None);
        assert_eq!(from_hex("zz"), None);
        // Non-ASCII of even length is refused, not a slice off a char boundary.
        assert_eq!(from_hex("a\u{e9}a"), None);
    }

    #[test]
    fn a_made_key_loads_back_and_signs() {
        let (pkcs8, public) = keygen().unwrap();
        let pair = load_key(&pkcs8).unwrap();
        assert_eq!(public.len(), 64);
        assert_eq!(sign(&pair, b"x").len(), 128);
        assert!(load_key("zz").is_err());
    }
}
