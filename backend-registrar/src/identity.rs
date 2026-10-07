//! Edge identity: who an edge is, and whether LosOS vouches for it.
//!
//! Any company may run an edge on its own network (`losos.edge.lan.advertise`),
//! and a box beside it finds it and shares storage through it. Trading —
//! the market, with its cut — is a different matter: only edges LosOS itself
//! runs may process it. The box therefore has to tell an *official* edge from
//! any other one, and this module is the whole mechanism:
//!
//! * **One root key**, Ed25519. The private half is held by the LosOS owner,
//!   offline, and used for nothing but signing edge identities. The public
//!   half ships inside every box (`losos.proxy.officialRootKeyFile`, read by
//!   lososd) — the trust anchor.
//! * **An identity per edge**: its own Ed25519 key pair plus a *certificate*
//!   — `{name, url, public_key, not_after}` signed by the root key. The
//!   certificate is public; the edge's private key stays on the edge.
//! * **A challenge on every scan**: the box asks `GET /identity?nonce=<hex>`
//!   and the edge answers with its certificate and a signature over the
//!   nonce. The box checks the root's signature on the certificate, the
//!   edge's signature on the nonce, that the certificate names the URL it
//!   is talking to, and that it has not expired. All four, or the edge is a
//!   company edge: sharing yes, trading no.
//!
//! That is a two-link chain and one freshness check: short enough to draw on
//! a whiteboard, with no CA, no TLS dependency (a LAN edge is plain http) and
//! no online service the box has to reach. An edge without an identity
//! answers 404 here and is simply not official.
//!
//! The same primitives, byte for byte, are used by the box
//! (`backend/src/edge.rs`): the two crates do not share code, so the message
//! formats below are the contract and each side's tests pin them.

use std::path::Path;

use miette::{miette, Context, IntoDiagnostic, Result};
use ring::rand::SystemRandom;
use ring::signature::{self, Ed25519KeyPair, KeyPair, UnparsedPublicKey};
use serde::{Deserialize, Serialize};

/// Domain separator of the certificate message. Versioned so a later format
/// cannot be confused with this one.
pub const CERT_PREFIX: &str = "losos-edge-identity-v1";
/// Domain separator of the nonce challenge.
pub const NONCE_PREFIX: &str = "losos-edge-nonce-v1";
/// Nonce length the server accepts, in hex characters: 16 to 64 bytes.
pub const NONCE_HEX_MIN: usize = 32;
pub const NONCE_HEX_MAX: usize = 128;

/// What the root signs. Public, so a box can show who vouched for an edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cert {
    /// A human name ("losos edge eu-1"), shown in the Mesh pane.
    pub name: String,
    /// The registrar URL this identity is valid for, no trailing slash.
    pub url: String,
    /// The edge's Ed25519 public key, 64 hex characters.
    pub public_key: String,
    /// Unix seconds after which the certificate is dead.
    pub not_after: u64,
    /// The root's Ed25519 signature over [`cert_message`], 128 hex characters.
    pub signature: String,
}

/// The answer to `GET /identity?nonce=…`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer {
    pub cert: Cert,
    /// The edge key's signature over [`nonce_message`], 128 hex characters.
    pub nonce_signature: String,
}

/// The bytes the root signs for a certificate. Line-separated fields with a
/// prefix: unambiguous because none of the fields may contain a newline
/// (`sign` refuses them), and readable in a hex dump.
#[must_use]
pub fn cert_message(name: &str, url: &str, public_key: &str, not_after: u64) -> Vec<u8> {
    format!("{CERT_PREFIX}\n{name}\n{url}\n{public_key}\n{not_after}\n").into_bytes()
}

/// The bytes an edge signs to prove it holds the key its certificate names.
#[must_use]
pub fn nonce_message(nonce: &str) -> Vec<u8> {
    format!("{NONCE_PREFIX}\n{nonce}\n").into_bytes()
}

/// A nonce the server will sign: hex, within the length bounds. Anything
/// else is refused, so the edge never signs attacker-shaped bytes.
#[must_use]
pub fn nonce_ok(nonce: &str) -> bool {
    (NONCE_HEX_MIN..=NONCE_HEX_MAX).contains(&nonce.len())
        && nonce.bytes().all(|b| b.is_ascii_hexdigit())
}

// ── hex, by hand: two short functions instead of a crate ───────────────────

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
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
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

/// Verify `sig_hex` over `msg` under `public_hex`. Any malformed input is a
/// plain `false`: a verifier has no one to explain itself to.
#[must_use]
pub fn verify(public_hex: &str, msg: &[u8], sig_hex: &str) -> bool {
    let (Some(public), Some(sig)) = (from_hex(public_hex.trim()), from_hex(sig_hex.trim())) else {
        return false;
    };
    UnparsedPublicKey::new(&signature::ED25519, public)
        .verify(msg, &sig)
        .is_ok()
}

// ── certificates ───────────────────────────────────────────────────────────

/// Issue a certificate for an edge: the root signs its name, URL, public key
/// and expiry. Fields may not contain newlines (they would split the
/// message); the URL is normalised the way the box normalises what it finds.
pub fn issue(
    root: &Ed25519KeyPair,
    name: &str,
    url: &str,
    public_key: &str,
    not_after: u64,
) -> Result<Cert> {
    let url = url.trim().trim_end_matches('/');
    for (what, v) in [("name", name), ("url", url), ("public key", public_key)] {
        if v.is_empty() || v.contains('\n') {
            return Err(miette!("the {what} must be one non-empty line"));
        }
    }
    if from_hex(public_key).map(|k| k.len()) != Some(32) {
        return Err(miette!("the public key must be 64 hex characters"));
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(miette!("the url must start with http:// or https://"));
    }
    let signature = sign(root, &cert_message(name, url, public_key, not_after));
    Ok(Cert {
        name: name.to_string(),
        url: url.to_string(),
        public_key: public_key.to_string(),
        not_after,
        signature,
    })
}

/// What a box checks about a certificate: the root's signature, the URL it
/// is talking to, and the clock. The nonce signature is checked separately
/// ([`Answer`] carries it) because it needs the nonce the box chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejected {
    RootSignature,
    Url,
    Expired,
    EdgeSignature,
}

impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Rejected::RootSignature => "the certificate is not signed by the LosOS root key",
            Rejected::Url => "the certificate names a different URL",
            Rejected::Expired => "the certificate has expired",
            Rejected::EdgeSignature => "the edge did not prove it holds the certified key",
        })
    }
}

/// Check a certificate against the root public key, the URL the box reached
/// the edge at, and `now`.
pub fn check_cert(root_public: &str, cert: &Cert, url: &str, now: u64) -> Result<(), Rejected> {
    let msg = cert_message(&cert.name, &cert.url, &cert.public_key, cert.not_after);
    if !verify(root_public, &msg, &cert.signature) {
        return Err(Rejected::RootSignature);
    }
    if cert.url.trim_end_matches('/') != url.trim().trim_end_matches('/') {
        return Err(Rejected::Url);
    }
    if now >= cert.not_after {
        return Err(Rejected::Expired);
    }
    Ok(())
}

/// Check a whole answer: the certificate, then the nonce signature under the
/// certified key.
pub fn check_answer(
    root_public: &str,
    answer: &Answer,
    url: &str,
    nonce: &str,
    now: u64,
) -> Result<(), Rejected> {
    check_cert(root_public, &answer.cert, url, now)?;
    if !verify(
        &answer.cert.public_key,
        &nonce_message(nonce),
        &answer.nonce_signature,
    ) {
        return Err(Rejected::EdgeSignature);
    }
    Ok(())
}

// ── the edge's own identity, as the server holds it ────────────────────────

/// The edge's key and certificate, loaded once at startup.
pub struct Identity {
    key: Ed25519KeyPair,
    cert: Cert,
}

impl Identity {
    /// Load from the two files `--identity-key-file` / `--identity-cert-file`
    /// name. The certificate must be for this key, or the edge would answer
    /// challenges it cannot pass; that is caught here, at startup, with a
    /// message, rather than on every box's scan with a silent `false`.
    pub fn load(key_file: &Path, cert_file: &Path) -> Result<Self> {
        let key_hex = std::fs::read_to_string(key_file)
            .into_diagnostic()
            .wrap_err_with(|| format!("reading the identity key {}", key_file.display()))?;
        let key = load_key(&key_hex)?;
        let cert_json = std::fs::read_to_string(cert_file)
            .into_diagnostic()
            .wrap_err_with(|| {
                format!("reading the identity certificate {}", cert_file.display())
            })?;
        let cert: Cert = serde_json::from_str(&cert_json)
            .into_diagnostic()
            .wrap_err("the identity certificate is not the JSON `identity sign` writes")?;
        if cert.public_key != to_hex(key.public_key().as_ref()) {
            return Err(miette!(
                "the identity certificate is for another key (its public key is not this key's)"
            ));
        }
        Ok(Identity { key, cert })
    }

    #[must_use]
    pub fn cert(&self) -> &Cert {
        &self.cert
    }

    /// Answer a challenge. The caller has checked [`nonce_ok`].
    #[must_use]
    pub fn answer(&self, nonce: &str) -> Answer {
        Answer {
            cert: self.cert.clone(),
            nonce_signature: sign(&self.key, &nonce_message(nonce)),
        }
    }
}

// ── the CLI: the key ceremony ──────────────────────────────────────────────

/// `losos-registrar identity …`. Printing goes to stdout so the owner can
/// redirect it; the key file itself is written 0600 and never printed.
pub fn run(opts: crate::opts::IdentityOpts) -> Result<()> {
    use crate::opts::IdentityOpts;
    match opts {
        IdentityOpts::Keygen { out } => {
            let (pkcs8_hex, public_hex) = keygen()?;
            let path = Path::new(&out);
            if path.exists() {
                return Err(miette!("{out} exists; not overwriting a key"));
            }
            write_private(path, &pkcs8_hex)?;
            println!("{public_hex}");
            Ok(())
        }
        IdentityOpts::Show { key } => {
            let hex = std::fs::read_to_string(&key)
                .into_diagnostic()
                .wrap_err_with(|| format!("reading {key}"))?;
            let pair = load_key(&hex)?;
            println!("{}", to_hex(pair.public_key().as_ref()));
            Ok(())
        }
        IdentityOpts::Verify {
            root_public,
            url,
            nonce,
            answer,
        } => {
            let text = if answer == "-" {
                std::io::read_to_string(std::io::stdin()).into_diagnostic()?
            } else {
                std::fs::read_to_string(&answer)
                    .into_diagnostic()
                    .wrap_err_with(|| format!("reading {answer}"))?
            };
            let answer: Answer = serde_json::from_str(&text)
                .into_diagnostic()
                .wrap_err("the answer is not a /identity document")?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            match check_answer(root_public.trim(), &answer, &url, &nonce, now) {
                Ok(()) => {
                    println!("official: {} at {}", answer.cert.name, answer.cert.url);
                    Ok(())
                }
                Err(why) => Err(miette!("not an official edge: {why:?}")),
            }
        }
        IdentityOpts::Sign {
            root_key,
            public_key,
            name,
            url,
            days,
        } => {
            let hex = std::fs::read_to_string(&root_key)
                .into_diagnostic()
                .wrap_err_with(|| format!("reading the root key {root_key}"))?;
            let root = load_key(&hex)?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| miette!("the clock is before 1970"))?
                .as_secs();
            let cert = issue(&root, &name, &url, public_key.trim(), now + days * 86_400)?;
            println!("{}", serde_json::to_string_pretty(&cert).into_diagnostic()?);
            Ok(())
        }
    }
}

/// Write a key file: created 0600, never over an existing file.
fn write_private(path: &Path, hex: &str) -> Result<()> {
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

    fn pair() -> (Ed25519KeyPair, String) {
        let (k, p) = keygen().unwrap();
        (load_key(&k).unwrap(), p)
    }

    #[test]
    fn hex_round_trips_and_rejects_garbage() {
        assert_eq!(to_hex(&[0, 15, 255]), "000fff");
        assert_eq!(from_hex("000fff"), Some(vec![0, 15, 255]));
        assert_eq!(from_hex("0"), None);
        assert_eq!(from_hex("zz"), None);
    }

    #[test]
    fn a_certificate_the_root_signed_passes_and_a_forged_one_does_not() {
        let (root, root_pub) = pair();
        let (edge, edge_pub) = pair();
        let cert = issue(
            &root,
            "edge one",
            "https://edge.example/",
            &edge_pub,
            2_000_000_000,
        )
        .unwrap();
        assert_eq!(cert.url, "https://edge.example", "the url is normalised");
        assert_eq!(
            check_cert(&root_pub, &cert, "https://edge.example", 1_900_000_000),
            Ok(())
        );
        // Another root: not ours.
        let (_, other_pub) = pair();
        assert_eq!(
            check_cert(&other_pub, &cert, "https://edge.example", 1_900_000_000),
            Err(Rejected::RootSignature)
        );
        // Tampered name: the signature no longer matches.
        let mut t = cert.clone();
        t.name = "edge two".into();
        assert_eq!(
            check_cert(&root_pub, &t, "https://edge.example", 1_900_000_000),
            Err(Rejected::RootSignature)
        );
        // Right certificate, wrong place.
        assert_eq!(
            check_cert(&root_pub, &cert, "https://other.example", 1_900_000_000),
            Err(Rejected::Url)
        );
        // Dead.
        assert_eq!(
            check_cert(&root_pub, &cert, "https://edge.example", 2_000_000_000),
            Err(Rejected::Expired)
        );
        let _ = edge;
    }

    #[test]
    fn the_challenge_needs_the_certified_key() {
        let (root, root_pub) = pair();
        let (edge, edge_pub) = pair();
        let cert = issue(
            &root,
            "edge",
            "http://edge.local:8443",
            &edge_pub,
            2_000_000_000,
        )
        .unwrap();
        let nonce = "00112233445566778899aabbccddeeff";
        let good = Answer {
            cert: cert.clone(),
            nonce_signature: sign(&edge, &nonce_message(nonce)),
        };
        assert_eq!(
            check_answer(
                &root_pub,
                &good,
                "http://edge.local:8443/",
                nonce,
                1_900_000_000
            ),
            Ok(())
        );
        // Replayed under another nonce.
        assert_eq!(
            check_answer(
                &root_pub,
                &good,
                "http://edge.local:8443",
                "ffeeddccbbaa99887766554433221100",
                1_900_000_000
            ),
            Err(Rejected::EdgeSignature)
        );
        // A stranger holding the certificate but not the key.
        let (stranger, _) = pair();
        let bad = Answer {
            cert,
            nonce_signature: sign(&stranger, &nonce_message(nonce)),
        };
        assert_eq!(
            check_answer(
                &root_pub,
                &bad,
                "http://edge.local:8443",
                nonce,
                1_900_000_000
            ),
            Err(Rejected::EdgeSignature)
        );
    }

    #[test]
    fn issue_refuses_fields_that_would_split_the_message() {
        let (root, _) = pair();
        let (_, edge_pub) = pair();
        assert!(issue(&root, "a\nb", "https://e", &edge_pub, 1).is_err());
        assert!(issue(&root, "a", "ftp://e", &edge_pub, 1).is_err());
        assert!(issue(&root, "a", "https://e", "abcd", 1).is_err());
    }

    #[test]
    fn nonces_are_hex_within_bounds() {
        assert!(nonce_ok(&"ab".repeat(16)));
        assert!(nonce_ok(&"ab".repeat(64)));
        assert!(!nonce_ok(&"ab".repeat(15)));
        assert!(!nonce_ok(&"ab".repeat(65)));
        assert!(!nonce_ok(&"zz".repeat(16)));
    }

    #[test]
    fn the_message_formats_are_the_contract_with_the_box() {
        assert_eq!(
            cert_message("n", "u", "k", 7),
            b"losos-edge-identity-v1\nn\nu\nk\n7\n".to_vec()
        );
        assert_eq!(nonce_message("ab"), b"losos-edge-nonce-v1\nab\n".to_vec());
    }
}
