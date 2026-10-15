//! The box's check of a Zcash key before it goes to the edge: a unified
//! viewing key, full (`uview…`) or incoming (`uivk…`), and never a spending
//! key or a seed phrase. Zcash's secrets are encoded with prefixes of their
//! own, so the shape is enough to tell them apart here; the edge decodes the
//! key, checks its checksum and its network, and keeps only its Orchard
//! incoming part. This half exists so a secret pasted by mistake never
//! leaves the box.

/// The human-readable parts of unified viewing keys on mainnet, testnet and
/// regtest, with bech32m's separator.
const VIEWING_PREFIXES: [&str; 6] = [
    "uview1",
    "uviewtest1",
    "uviewregtest1",
    "uivk1",
    "uivktest1",
    "uivkregtest1",
];

/// Prefixes of encoded Zcash spending keys: Orchard's and Sapling's, on
/// every network.
const SECRET_PREFIXES: [&str; 2] = ["secret-", "zxsk"];

const BECH32: &str = "qpzry9x8gf2tvdw0s3jn54khce6mua7l";

const SPENDING: &str =
    "that is a spending key; never send it to anyone. Send the unified viewing key (uview… or uivk…)";
const SEED: &str =
    "that looks like a seed phrase; never send it to anyone. Send the unified viewing key (uview… or uivk…)";

/// Why `key` is not a unified viewing key the box should send, if it is not.
pub fn check(key: &str) -> Result<(), &'static str> {
    let key = key.trim();
    let lower = key.to_ascii_lowercase();
    if SECRET_PREFIXES.iter().any(|p| lower.starts_with(p)) {
        return Err(SPENDING);
    }
    let words: Vec<&str> = key.split_whitespace().collect();
    if words.len() >= 12
        && words
            .iter()
            .all(|w| w.chars().all(|c| c.is_ascii_lowercase()))
    {
        return Err(SEED);
    }
    if lower.starts_with("zxview") {
        return Err("that is a Sapling viewing key; send the unified viewing key (uview…)");
    }
    let Some(prefix) = VIEWING_PREFIXES.iter().find(|p| key.starts_with(*p)) else {
        return Err("key is not a Zcash unified viewing key (uview… or uivk…)");
    };
    let data = &key[prefix.len()..];
    if key.len() > 2_000 || data.len() < 6 || !data.chars().all(|c| BECH32.contains(c)) {
        return Err("key is not a Zcash unified viewing key (uview… or uivk…)");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewing_keys_pass_and_secrets_are_refused_by_name() {
        // Shapes only: none of these is anybody's key.
        assert!(check("uview1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq").is_ok());
        assert!(check("  uivk1qpzry9x8gf2tvdw0s3jn54khce6mua7l ").is_ok());
        assert!(check("uviewregtest1qqqqqqqqqqqqqqqq").is_ok());
        for secret in [
            "secret-orchard-sk-main1qqqqqqqqqqqqqqqqqqqqqqqq",
            "secret-extended-key-main1qqqqqqqqqqqqqqqqqqqqqq",
            "zxsk1notarealkeyonlytheshapeofone",
        ] {
            assert_eq!(check(secret), Err(SPENDING), "{secret}");
        }
        assert_eq!(check(&["zoo"; 24].join(" ")), Err(SEED));
        assert!(check("zxviews1qqqq").unwrap_err().contains("Sapling"));
        // An address, a WIF-shaped string, mixed case, a stray character.
        for not_a_key in [
            "u1qqqqqqqqqqqqqqqqqqqqqqqqqqqq",
            "KzNotARealKeyOnlyTheShapeOfOneNotARealKeyOnlyTheSh",
            "uview1QQQQQQQQQQQQ",
            "uview1qqqqqqqqqbqqq",
            "",
        ] {
            assert!(check(not_a_key).is_err(), "{not_a_key}");
        }
    }
}
