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

/// The edge's key, and the certificate for it if one has been installed.
///
/// The key is born on the edge: [`Identity::open`] makes it on the first
/// start when the key file is missing, so the private half never travels.
/// The certificate arrives later, pushed by an operator through
/// `POST /identity/cert` ([`Identity::install`]) once the root has signed
/// the edge's public key; until then the edge answers `/identity` with 404
/// and is not official. Both halves are read once at startup and the
/// certificate swapped in place on a push, so a push needs no restart.
pub struct Identity {
    key: Ed25519KeyPair,
    public_hex: String,
    cert_path: std::path::PathBuf,
    cert: std::sync::RwLock<Option<Cert>>,
}

impl Identity {
    /// Open the two files `--identity-key-file` / `--identity-cert-file`
    /// name. A missing key file is made (0600) and logged; a missing
    /// certificate is simply "not official yet". A certificate that is not
    /// for this key is logged and ignored rather than refused at startup:
    /// the edge has to be up to accept the push that replaces it, and a 404
    /// on `/identity` is as visible as a refusal to start and costs no box
    /// its tunnel.
    pub fn open(key_file: &Path, cert_file: &Path) -> Result<Self> {
        let key = match std::fs::read_to_string(key_file) {
            Ok(hex) => load_key(&hex)
                .wrap_err_with(|| format!("the identity key {}", key_file.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let (pkcs8_hex, public_hex) = keygen()?;
                if let Some(dir) = key_file.parent() {
                    std::fs::create_dir_all(dir)
                        .into_diagnostic()
                        .wrap_err_with(|| format!("creating {}", dir.display()))?;
                }
                write_private(key_file, &pkcs8_hex)?;
                tracing::info!(
                    target: crate::action::Action::Identity.target(),
                    "made this edge's identity key at {} (public key {public_hex}); \
                     not official until a certificate for it is pushed",
                    key_file.display()
                );
                load_key(&pkcs8_hex)?
            }
            Err(e) => {
                return Err(e)
                    .into_diagnostic()
                    .wrap_err_with(|| format!("reading the identity key {}", key_file.display()))
            }
        };
        let public_hex = to_hex(key.public_key().as_ref());
        let cert = match std::fs::read_to_string(cert_file) {
            Ok(json) => match serde_json::from_str::<Cert>(&json) {
                Ok(c) if c.public_key == public_hex => Some(c),
                Ok(_) => {
                    tracing::warn!(
                        target: crate::action::Action::Identity.target(),
                        "ignoring {}: the certificate is for another key (not this edge's); \
                         push a new one with `losos-registrar provision edge`",
                        cert_file.display()
                    );
                    None
                }
                Err(e) => {
                    tracing::warn!(
                        target: crate::action::Action::Identity.target(),
                        "ignoring {}: not the JSON a certificate is ({e})",
                        cert_file.display()
                    );
                    None
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                return Err(e).into_diagnostic().wrap_err_with(|| {
                    format!("reading the identity certificate {}", cert_file.display())
                })
            }
        };
        match &cert {
            Some(c) => tracing::info!(
                target: crate::action::Action::Identity.target(),
                "identity: {} at {} (certificate until {})", c.name, c.url, c.not_after
            ),
            None => tracing::info!(
                target: crate::action::Action::Identity.target(),
                "identity key {public_hex}, no certificate yet: /identity answers 404"
            ),
        }
        Ok(Identity {
            key,
            public_hex,
            cert_path: cert_file.to_path_buf(),
            cert: std::sync::RwLock::new(cert),
        })
    }

    /// This edge's public key, 64 hex characters: what the root signs.
    #[must_use]
    pub fn public_key(&self) -> &str {
        &self.public_hex
    }

    /// The installed certificate, if any.
    #[must_use]
    pub fn cert(&self) -> Option<Cert> {
        self.cert.read().map(|c| c.clone()).unwrap_or(None)
    }

    /// Answer a challenge, or `None` while no certificate is installed. The
    /// caller has checked [`nonce_ok`].
    #[must_use]
    pub fn answer(&self, nonce: &str) -> Option<Answer> {
        let cert = self.cert()?;
        Some(Answer {
            cert,
            nonce_signature: sign(&self.key, &nonce_message(nonce)),
        })
    }

    /// Install a certificate an operator pushed: it must be for this key,
    /// shaped like one the root issues, and not already dead. Written to
    /// the certificate file (temp + rename, world-readable: it is public)
    /// and swapped in, so the next `/identity` answers with it. Who may
    /// call this is the server's business ([`crate::server`]); the root's
    /// signature is the boxes' to check, and `provision edge` probes it
    /// right after.
    pub fn install(&self, cert: Cert, now: u64) -> Result<(), Pushed> {
        if cert.public_key != self.public_hex {
            return Err(Pushed::OtherKey);
        }
        if cert.name.trim().is_empty() || cert.name.contains('\n') || cert.url.contains('\n') {
            return Err(Pushed::Malformed("name and url must be one non-empty line"));
        }
        if !(cert.url.starts_with("http://") || cert.url.starts_with("https://")) {
            return Err(Pushed::Malformed("url must start with http:// or https://"));
        }
        if from_hex(&cert.signature).map(|s| s.len()) != Some(64) {
            return Err(Pushed::Malformed("signature must be 128 hex characters"));
        }
        if now >= cert.not_after {
            return Err(Pushed::Malformed("the certificate has already expired"));
        }
        let json = serde_json::to_string_pretty(&cert).map_err(|e| Pushed::Write(e.to_string()))?;
        let tmp = self.cert_path.with_extension("json.tmp");
        if let Some(dir) = self.cert_path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| Pushed::Write(format!("{}: {e}", dir.display())))?;
        }
        std::fs::write(&tmp, format!("{json}\n"))
            .and_then(|()| std::fs::rename(&tmp, &self.cert_path))
            .map_err(|e| Pushed::Write(format!("{}: {e}", self.cert_path.display())))?;
        if let Ok(mut slot) = self.cert.write() {
            *slot = Some(cert);
        }
        Ok(())
    }
}

/// Why a pushed certificate was not installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pushed {
    /// The certificate names another public key: it is not for this edge.
    OtherKey,
    /// Not shaped like a certificate the root issues.
    Malformed(&'static str),
    /// The certificate file could not be written (a read-only path, for one).
    Write(String),
}

impl std::fmt::Display for Pushed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Pushed::OtherKey => f.write_str(
                "the certificate is for another key; sign this edge's public key (GET /identity/public-key)",
            ),
            Pushed::Malformed(why) => write!(f, "not a certificate: {why}"),
            Pushed::Write(why) => write!(f, "could not write the certificate file: {why}"),
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
    fn an_identity_makes_its_key_once_and_installs_only_certificates_for_it() {
        let dir = std::env::temp_dir().join(format!(
            "losos-identity-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let key_file = dir.join("sub").join("edge.key");
        let cert_file = dir.join("sub").join("edge.cert.json");

        // First start: the key is made, nothing is official.
        let id = Identity::open(&key_file, &cert_file).unwrap();
        let public = id.public_key().to_string();
        assert_eq!(from_hex(&public).map(|k| k.len()), Some(32));
        assert!(id.cert().is_none());
        assert!(id.answer("ab").is_none());
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&key_file).unwrap().permissions().mode() & 0o777,
            0o600
        );

        // A certificate for another key is refused and nothing is written.
        let (root, root_pub) = pair();
        let (_, other_pub) = pair();
        let other = issue(&root, "e", "https://e", &other_pub, 2_000_000_000).unwrap();
        assert_eq!(id.install(other, 1_900_000_000), Err(Pushed::OtherKey));
        assert!(!cert_file.exists());

        // A dead one, and a malformed one, are refused too.
        let dead = issue(&root, "e", "https://e", &public, 1_000).unwrap();
        assert!(matches!(
            id.install(dead, 1_900_000_000),
            Err(Pushed::Malformed(_))
        ));
        let mut bad_sig = issue(&root, "e", "https://e", &public, 2_000_000_000).unwrap();
        bad_sig.signature = "zz".into();
        assert!(matches!(
            id.install(bad_sig, 1_900_000_000),
            Err(Pushed::Malformed(_))
        ));

        // The right one: written, served, and the challenge is answered.
        let good = issue(&root, "e", "https://e", &public, 2_000_000_000).unwrap();
        assert_eq!(id.install(good.clone(), 1_900_000_000), Ok(()));
        let on_disk: Cert =
            serde_json::from_str(&std::fs::read_to_string(&cert_file).unwrap()).unwrap();
        assert_eq!(on_disk, good);
        let nonce = "00112233445566778899aabbccddeeff";
        let answer = id.answer(nonce).unwrap();
        assert_eq!(
            check_answer(&root_pub, &answer, "https://e", nonce, 1_900_000_000),
            Ok(())
        );

        // Second start: the same key is loaded, the certificate with it.
        let again = Identity::open(&key_file, &cert_file).unwrap();
        assert_eq!(again.public_key(), public);
        assert_eq!(again.cert(), Some(good));

        // A certificate on disk for another key is ignored, not fatal: the
        // edge stays up to take the push that replaces it.
        let (_, stray_pub) = pair();
        let stray = issue(&root, "e", "https://e", &stray_pub, 2_000_000_000).unwrap();
        std::fs::write(&cert_file, serde_json::to_string(&stray).unwrap()).unwrap();
        let third = Identity::open(&key_file, &cert_file).unwrap();
        assert_eq!(third.public_key(), public);
        assert!(third.cert().is_none());

        // A key file that is not a key is fatal: the edge would otherwise
        // silently make a new identity and orphan its certificate.
        std::fs::write(&key_file, "not hex\n").unwrap();
        assert!(Identity::open(&key_file, &cert_file).is_err());
        let _ = std::fs::remove_dir_all(&dir);
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
