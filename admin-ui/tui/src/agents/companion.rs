//! The companion's own credential: a key of its own, the owner's word for
//! it, and an AI agents token, so `losos mcp` can call the box with no
//! passphrase typed. docs/terminal-access.md, "The AI agent companion".
//!
//! [`make`] runs once, in `losos agents install`, with the owner's key
//! unlocked: it makes a fresh OpenPGP key, has the owner's key sign a
//! [`Delegation`](losos_access::Delegation) for it that names this box and
//! this token, and keeps all of it in the Secret Service as one
//! `kind=companion` item per box. The box takes a request signed by that
//! key only with the AI agents token the delegation names, and an AI agents
//! token reaches a few read-only routes and nothing else; so what an AI
//! tool can do with the item is what that token can, never what the owner's
//! key can.

use losos_access::{delegate, HostKey, SeededKey, UserKey};
use serde::{Deserialize, Serialize};

use crate::envelope::Session;
use crate::http::{Http, Trust};
use crate::keychain::Keychain;
use crate::problem::{Problem, Result};
use crate::tr;
use crate::vault::{HostKind, HostRecord};

const SERVICE: &str = "losos";
const KIND: &str = "companion";

/// The secret of a `kind=companion` item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Companion {
    pub url: String,
    /// The box's key, as pinned at login.
    pub host_fingerprint: String,
    pub host_public_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_pin: Option<String>,
    /// An AI agents token, `losos_pat_<id>_<secret>`.
    pub token: String,
    /// The companion's own secret key, armoured, under no passphrase.
    pub key: String,
    /// The owner's signed word for that key, armoured.
    pub delegation: String,
}

/// The `<id>` of `losos_pat_<id>_<secret>`.
pub fn token_id(token: &str) -> Option<&str> {
    let rest = token.strip_prefix("losos_pat_")?;
    let (id, secret) = rest.split_once('_')?;
    let hex = |s: &str, n: usize| {
        s.len() == n && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    };
    (hex(id, 8) && hex(secret, 40)).then_some(id)
}

/// A new companion for `host`: a fresh key, vouched for by `owner` for
/// `token` on this box only.
pub fn make(
    owner: &losos_access::Unlocked,
    host: &HostRecord,
    token: &str,
    device: &str,
    now: i64,
) -> Result<Companion> {
    let id =
        token_id(token).ok_or_else(|| Problem::new("losos::token", tr!("tui.agents.badToken")))?;
    let made = HostKey::generate(&format!("LosOS companion on {device}"))
        .map_err(|e| Problem::new("losos::key", e.to_string()))?;
    let delegation = delegate(owner, made.armored_public(), &host.fingerprint, id, now)
        .map_err(|e| Problem::new("losos::key", e.to_string()))?;
    Ok(Companion {
        url: host.url.clone(),
        host_fingerprint: host.fingerprint.clone(),
        host_public_key: host.public_key.clone(),
        tls_pin: host.tls_pin.clone(),
        token: token.to_string(),
        key: made
            .armored_secret()
            .map_err(|e| Problem::new("losos::key", e.to_string()))?,
        delegation,
    })
}

impl Companion {
    /// A session that signs with the companion's key and sends the owner's
    /// word for it with every call.
    pub fn session(&self) -> Result<Session> {
        let key = UserKey::from_armored(&self.key)?;
        let record = HostRecord {
            url: self.url.clone(),
            kind: HostKind::Box,
            fingerprint: self.host_fingerprint.clone(),
            public_key: self.host_public_key.clone(),
            token: self.token.clone(),
            key_fingerprint: key.fingerprint().to_string(),
            tls_pin: self.tls_pin.clone(),
        };
        let trust = self.tls_pin.clone().map_or(Trust::Public, Trust::Pinned);
        let mut session = Session::new(&record, key.unlock("")?, Http::new(trust))?;
        session.delegation = Some(self.delegation.clone());
        Ok(session)
    }

    /// The pinned host key, for checking a companion still matches a login.
    pub fn host_key(&self) -> Result<SeededKey> {
        SeededKey::from_armored(&self.host_public_key)
            .map_err(|_| Problem::new("losos::pin", tr!("tui.host.pinBroken")))
    }
}

pub fn put(keychain: &dyn Keychain, companion: &Companion) -> Result<()> {
    let json = serde_json::to_string(companion).expect("a companion serialises");
    keychain.store(
        &[
            ("service", SERVICE),
            ("kind", KIND),
            ("url", &companion.url),
        ],
        &tr!("tui.agents.keychainLabel", url = &companion.url),
        &json,
    )
}

pub fn get(keychain: &dyn Keychain, url: &str) -> Result<Option<Companion>> {
    Ok(keychain
        .lookup(&[("service", SERVICE), ("kind", KIND), ("url", url)])?
        .and_then(|s| serde_json::from_str(&s).ok()))
}

/// Every companion this computer keeps, by URL.
pub fn list(keychain: &dyn Keychain) -> Result<Vec<Companion>> {
    let mut all: Vec<Companion> = keychain
        .search(&[("service", SERVICE), ("kind", KIND)])?
        .into_iter()
        .filter_map(|i| serde_json::from_str(&i.secret).ok())
        .collect();
    all.sort_by(|a, b| a.url.cmp(&b.url));
    Ok(all)
}

pub fn forget(keychain: &dyn Keychain, url: &str) -> Result<()> {
    keychain.clear(&[("service", SERVICE), ("kind", KIND), ("url", url)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_whole_token_has_an_id() {
        let t = format!("losos_pat_0a1b2c3d_{}", "e".repeat(40));
        assert_eq!(token_id(&t), Some("0a1b2c3d"));
        assert_eq!(token_id(&t.to_uppercase()), None);
        assert_eq!(token_id("losos_pat_0a1b2c3d_short"), None);
        assert_eq!(token_id(&t.replace("losos_pat_", "")), None);
    }
}
