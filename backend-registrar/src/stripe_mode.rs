//! Whether a Stripe key works on test data or on real money.
//!
//! Stripe writes the mode into every API key: `sk_test_`, `rk_test_` and
//! `pk_test_` keys only ever see test data, `sk_live_`, `rk_live_` and
//! `pk_live_` keys move real money. The edge reads the mode off the key it is
//! given rather than from a setting of its own, so the one thing an operator
//! changes to go live is the key, and a setting can never disagree with it.
//!
//! Webhook signing secrets (`whsec_`) carry no mode. Every event Stripe sends
//! does (`livemode`), so the market compares that against the key's mode when
//! an event arrives: a secret from the other mode's endpoint shows up there.

use serde::{Deserialize, Serialize};

/// A Stripe key's mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StripeMode {
    Test,
    Live,
}

impl StripeMode {
    /// The mode of a secret (`sk_`), restricted (`rk_`) or publishable
    /// (`pk_`) key, or `None` for anything else, including a key whose mode
    /// cannot be read off it.
    #[must_use]
    pub fn of_key(key: &str) -> Option<Self> {
        let rest = ["sk_", "rk_", "pk_"]
            .iter()
            .find_map(|p| key.strip_prefix(p))?;
        if rest.strip_prefix("test_").is_some_and(|r| !r.is_empty()) {
            Some(Self::Test)
        } else if rest.strip_prefix("live_").is_some_and(|r| !r.is_empty()) {
            Some(Self::Live)
        } else {
            None
        }
    }

    /// The mode a Stripe event was sent in, from its `livemode` field.
    #[must_use]
    pub fn of_event(event: &serde_json::Value) -> Option<Self> {
        event["livemode"]
            .as_bool()
            .map(|live| if live { Self::Live } else { Self::Test })
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Test => "test",
            Self::Live => "live",
        }
    }
}

impl std::fmt::Display for StripeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The one mode a set of keys shares. Every key must name a mode, and all of
/// them the same one: a test publishable key next to a live secret key would
/// take a buyer through a test checkout for a live charge, or the reverse.
///
/// # Errors
/// Names the first key, by its position, that has no mode or disagrees.
pub fn shared_mode<'a>(keys: impl IntoIterator<Item = &'a str>) -> Result<StripeMode, String> {
    let mut mode = None;
    for (i, key) in keys.into_iter().enumerate() {
        let Some(this) = StripeMode::of_key(key) else {
            return Err(format!("key {} is neither a test nor a live key", i + 1));
        };
        match mode {
            None => mode = Some(this),
            Some(first) if first != this => {
                return Err(format!(
                    "key {} is a {this} key, but the keys before it are {first} keys",
                    i + 1
                ));
            }
            Some(_) => {}
        }
    }
    mode.ok_or_else(|| "no key".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mode_is_read_off_the_key() {
        assert_eq!(StripeMode::of_key("sk_test_abc"), Some(StripeMode::Test));
        assert_eq!(StripeMode::of_key("rk_test_abc"), Some(StripeMode::Test));
        assert_eq!(StripeMode::of_key("pk_test_abc"), Some(StripeMode::Test));
        assert_eq!(StripeMode::of_key("sk_live_abc"), Some(StripeMode::Live));
        assert_eq!(StripeMode::of_key("rk_live_abc"), Some(StripeMode::Live));
        assert_eq!(StripeMode::of_key("pk_live_abc"), Some(StripeMode::Live));
    }

    #[test]
    fn a_key_without_a_readable_mode_has_none() {
        for key in [
            "",
            "sk_",
            "sk_test_",
            "sk_live_",
            "sk_abc",
            "whsec_abc",
            "sk-ant-api03-abc",
            "xk_test_abc",
            "SK_TEST_abc",
        ] {
            assert_eq!(StripeMode::of_key(key), None, "{key:?}");
        }
    }

    #[test]
    fn the_event_says_its_own_mode() {
        let live = serde_json::json!({ "livemode": true });
        let test = serde_json::json!({ "livemode": false });
        assert_eq!(StripeMode::of_event(&live), Some(StripeMode::Live));
        assert_eq!(StripeMode::of_event(&test), Some(StripeMode::Test));
        assert_eq!(StripeMode::of_event(&serde_json::json!({})), None);
    }

    #[test]
    fn keys_of_one_mode_pair_and_mixed_ones_do_not() {
        assert_eq!(
            shared_mode(["sk_test_a", "pk_test_b"]),
            Ok(StripeMode::Test)
        );
        assert_eq!(shared_mode(["rk_live_a"]), Ok(StripeMode::Live));
        let mixed = shared_mode(["sk_live_a", "pk_test_b"]).unwrap_err();
        assert!(mixed.contains("key 2 is a test key"), "{mixed}");
        assert!(shared_mode(["sk_test_a", "whsec_b"]).is_err());
        assert!(shared_mode(std::iter::empty()).is_err());
    }

    #[test]
    fn the_wire_names_are_lowercase() {
        assert_eq!(
            serde_json::to_string(&StripeMode::Live).unwrap(),
            "\"live\""
        );
        assert_eq!(StripeMode::Test.to_string(), "test");
    }
}
