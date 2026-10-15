//! The two kinds of item this client keeps, over the [`Keychain`]
//! interface. docs/terminal-access.md, "What the client keeps, and where".

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::keychain::Keychain;
use crate::problem::Result;

const SERVICE: &str = "losos";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HostKind {
    Box,
    Edge,
}

impl HostKind {
    /// The word on screen: "box" or "edge".
    pub fn word(self) -> String {
        crate::i18n::t(match self {
            HostKind::Box => "tui.kind.box",
            HostKind::Edge => "tui.kind.edge",
        })
    }
}

/// The secret of a `kind=host` item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostRecord {
    pub url: String,
    pub kind: HostKind,
    /// The host key's fingerprint, 40 hex, upper case. Pinned on first login.
    pub fingerprint: String,
    /// The host key's armoured public half, as served at login.
    pub public_key: String,
    pub token: String,
    /// Which of the user's keys (`kind=gpg-key`) signs for this host.
    pub key_fingerprint: String,
    /// The SHA-256 of the TLS certificate the host showed at login, hex,
    /// when no public authority vouched for it: a box's own self-signed
    /// certificate. Absent for a host with a certificate the web trusts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_pin: Option<String>,
}

/// One spelling per host: scheme and host lower case, no trailing slash, no
/// default port. A bare name gets `https://`.
pub fn normalize_url(raw: &str) -> std::result::Result<String, String> {
    let trimmed = raw.trim();
    let has_scheme = trimmed.split_once("://").is_some_and(|(s, _)| {
        !s.is_empty()
            && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
    });
    let with_scheme = if has_scheme {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let url = url::Url::parse(&with_scheme).map_err(|e| e.to_string())?;
    if url.scheme() != "https" && url.scheme() != "http" {
        return Err(format!("{}: is not http or https", url.scheme()));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("a URL with a user name or password".into());
    }
    let host = url.host_str().ok_or("a URL without a host")?;
    let port = url.port().map(|p| format!(":{p}")).unwrap_or_default();
    let path = url.path().trim_end_matches('/');
    Ok(format!("{}://{host}{port}{path}", url.scheme()))
}

/// The host part of a stored URL, for titles: `https://box.local` -> `box.local`.
pub fn host_name(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| url.to_string())
}

#[derive(Clone)]
pub struct Vault {
    keychain: Arc<dyn Keychain + Send + Sync>,
}

impl Vault {
    pub fn new(keychain: Arc<dyn Keychain + Send + Sync>) -> Vault {
        Vault { keychain }
    }

    pub fn keychain(&self) -> &dyn Keychain {
        &*self.keychain
    }

    pub fn put_key(&self, fingerprint: &str, armored: &str, label: &str) -> Result<()> {
        self.keychain.store(
            &[
                ("service", SERVICE),
                ("kind", "gpg-key"),
                ("fingerprint", fingerprint),
            ],
            label,
            armored,
        )
    }

    pub fn get_key(&self, fingerprint: &str) -> Result<Option<String>> {
        self.keychain.lookup(&[
            ("service", SERVICE),
            ("kind", "gpg-key"),
            ("fingerprint", fingerprint),
        ])
    }

    /// Every stored key, armoured.
    pub fn list_keys(&self) -> Result<Vec<String>> {
        Ok(self
            .keychain
            .search(&[("service", SERVICE), ("kind", "gpg-key")])?
            .into_iter()
            .map(|i| i.secret)
            .filter(|s| s.contains("BEGIN PGP PRIVATE KEY BLOCK"))
            .collect())
    }

    pub fn remove_key(&self, fingerprint: &str) -> Result<()> {
        self.keychain.clear(&[
            ("service", SERVICE),
            ("kind", "gpg-key"),
            ("fingerprint", fingerprint),
        ])
    }

    pub fn put_host(&self, record: &HostRecord) -> Result<()> {
        let kind = match record.kind {
            HostKind::Box => "box",
            HostKind::Edge => "edge",
        };
        let json = serde_json::to_string(record).expect("a host record serialises");
        self.keychain.store(
            &[("service", SERVICE), ("kind", "host"), ("url", &record.url)],
            &format!("losos {kind} {}", record.url),
            &json,
        )
    }

    pub fn get_host(&self, url: &str) -> Result<Option<HostRecord>> {
        Ok(self
            .keychain
            .lookup(&[("service", SERVICE), ("kind", "host"), ("url", url)])?
            .and_then(|s| serde_json::from_str(&s).ok()))
    }

    /// Every pinned host, by URL. An item this client cannot read is skipped.
    pub fn list_hosts(&self) -> Result<Vec<HostRecord>> {
        let mut hosts: Vec<HostRecord> = self
            .keychain
            .search(&[("service", SERVICE), ("kind", "host")])?
            .into_iter()
            .filter_map(|i| serde_json::from_str(&i.secret).ok())
            .collect();
        hosts.sort_by(|a, b| a.url.cmp(&b.url));
        Ok(hosts)
    }

    pub fn remove_host(&self, url: &str) -> Result<()> {
        self.keychain
            .clear(&[("service", SERVICE), ("kind", "host"), ("url", url)])
    }
}

/// "ABCD EF01 2345 ..." for reading aloud and comparing by eye.
pub fn group_fingerprint(fingerprint: &str) -> String {
    let clean: Vec<char> = fingerprint
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    clean
        .chunks(4)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn same_fingerprint(a: &str, b: &str) -> bool {
    let clean = |s: &str| -> String {
        s.chars()
            .filter(|c| !c.is_whitespace())
            .map(|c| c.to_ascii_uppercase())
            .collect()
    };
    clean(a) == clean(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_get_one_spelling() {
        assert_eq!(normalize_url("Box.Local").unwrap(), "https://box.local");
        assert_eq!(
            normalize_url("https://BOX.local:443/").unwrap(),
            "https://box.local"
        );
        assert_eq!(
            normalize_url("http://192.168.1.4:8080/x/").unwrap(),
            "http://192.168.1.4:8080/x"
        );
        assert!(normalize_url("ftp://box").is_err());
        assert!(normalize_url("https://me:pw@box").is_err());
    }

    #[test]
    fn fingerprints_group_and_compare() {
        assert_eq!(group_fingerprint("3f2a9c1e0b7d"), "3F2A 9C1E 0B7D");
        assert!(same_fingerprint("3F2A 9C1E", "3f2a9c1e"));
    }

    #[test]
    fn a_record_from_before_the_tls_pin_still_reads() {
        let old = r#"{"url":"https://box.local","kind":"box","fingerprint":"AB","publicKey":"k","token":"t","keyFingerprint":"CD"}"#;
        let record: HostRecord = serde_json::from_str(old).unwrap();
        assert_eq!(record.tls_pin, None);
        assert!(!serde_json::to_string(&record).unwrap().contains("tlsPin"));
    }
}
