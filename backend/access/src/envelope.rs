//! One call, sealed both ways: the request signed by the owner's key and
//! encrypted to the server's, the answer signed by the server and encrypted
//! back to the owner. `docs/terminal-access.md` is the contract.

use std::collections::HashMap;
use std::io::Read;
use std::sync::Mutex;

use pgp::composed::{ArmorOptions, Message, MessageBuilder};
use pgp::crypto::hash::HashAlgorithm;
use pgp::crypto::sym::SymmetricKeyAlgorithm;
use pgp::types::Password;
use serde::{Deserialize, Serialize};

use crate::keys::{HostKey, KeyError, Keyring, SeededKey};

/// How far a request's `ts` may be from the server's clock.
pub const MAX_SKEW_SECS: i64 = 300;
/// How long a nonce is remembered: twice the skew, so a request cannot be
/// replayed while its `ts` still passes.
pub const REPLAY_SECS: i64 = 2 * MAX_SKEW_SECS;
/// The largest plaintext either side accepts. Settings, an apply body and
/// the option document all fit with room to spare.
const MAX_PLAINTEXT: u64 = 1 << 20;
/// Nonces kept at most. Only holders of a seeded key get as far as the
/// replay check, so this is a bound, not a defence.
const MAX_NONCES: usize = 100_000;

/// What the client asked for, inside the envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub v: u32,
    /// The fingerprint of the server's key the client pinned.
    pub host: String,
    pub token: String,
    pub method: String,
    pub path: String,
    pub ts: i64,
    pub nonce: String,
    #[serde(default)]
    pub body: serde_json::Value,
    /// A request signed by a companion key carries the seeded key's word for
    /// it: an armoured [`Delegation`], signed by that seeded key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delegation: Option<String>,
}

/// What the `kind` of a [`Delegation`] says.
pub const DELEGATION_KIND: &str = "losos-companion";

/// A seeded key's word that another key may sign for it, made once by the
/// client when the owner sets up the AI agent companion (`losos agents
/// install`) and sent with every request the companion signs. It names the
/// one server and the one token it is good for, and the server lets the
/// companion use no token but an [`Scope::Agent`](crate::Scope::Agent) one.
/// So an AI tool can call the box with no passphrase typed, and the key it
/// holds reaches what that token reaches and nothing more. Nothing is added
/// to the server: only a holder of the seeded secret key can make one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Delegation {
    pub v: u32,
    pub kind: String,
    /// The companion's armoured public key.
    pub key: String,
    /// The server's key fingerprint, as in [`Request::host`].
    pub host: String,
    /// The id of the token the companion holds (`losos_pat_<id>_…`).
    pub token: String,
    pub created: i64,
}

/// The seeded key a companion signs for, when it does.
#[derive(Debug, Clone)]
pub struct Delegated {
    /// The seeded key that made the delegation, which the token is bound to.
    pub owner: SeededKey,
    /// The token id the delegation names.
    pub token_id: String,
}

/// A request that decrypted, verified and is fresh.
#[derive(Debug)]
pub struct Opened {
    /// The key that signed it, and that the answer is sealed to: a seeded
    /// key, or the companion key of a [`Delegation`].
    pub signer: SeededKey,
    pub request: Request,
    /// Set when the signer is a companion key; the caller checks the token
    /// against [`Delegated::owner`] and refuses anything but an agent token.
    pub delegated: Option<Delegated>,
}

/// Why a request was turned away before anyone it could be sealed to was
/// known. Each answers as plain JSON, `{"error": message}`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("this is not a message for this server's key")]
    NotForThisHost,
    #[error("the request was not signed by a GPG key seeded on this server")]
    Unsigned,
    #[error("the request is not shaped like a terminal call: {0}")]
    Malformed(String),
    #[error("the request was made for another server's key")]
    WrongHost,
    #[error("the request's clock is more than five minutes from this server's")]
    Stale,
    #[error("this request was already answered")]
    Replayed,
    #[error("the access token is not valid for this key")]
    Token,
    #[error("this token cannot change anything")]
    Scope,
    #[error("a terminal token cannot reach this path")]
    Path,
    #[error("a companion key works only with the AI agents token it was made for")]
    NotForAgents,
}

impl Refusal {
    /// The HTTP status the contract gives it.
    pub fn status(&self) -> u16 {
        match self {
            Refusal::NotForThisHost | Refusal::Malformed(_) | Refusal::WrongHost => 400,
            Refusal::Unsigned | Refusal::Stale | Refusal::Token => 401,
            Refusal::Scope | Refusal::Path | Refusal::NotForAgents => 403,
            Refusal::Replayed => 409,
        }
    }
}

/// Nonces seen in the last [`REPLAY_SECS`].
#[derive(Default)]
pub struct Replay {
    seen: Mutex<HashMap<String, i64>>,
}

impl Replay {
    /// Record `nonce`; refuse it if it was already seen.
    fn admit(&self, nonce: &str, now: i64) -> Result<(), Refusal> {
        let mut seen = self.seen.lock().unwrap_or_else(|e| e.into_inner());
        seen.retain(|_, at| now - *at < REPLAY_SECS);
        if seen.contains_key(nonce) {
            return Err(Refusal::Replayed);
        }
        if seen.len() >= MAX_NONCES {
            // Full of live nonces: refusing is safe, forgetting is not.
            return Err(Refusal::Replayed);
        }
        seen.insert(nonce.to_string(), now);
        Ok(())
    }
}

pub(crate) fn read_limited(msg: &mut Message<'_>) -> Result<Vec<u8>, Refusal> {
    let mut out = Vec::new();
    msg.take(MAX_PLAINTEXT + 1)
        .read_to_end(&mut out)
        .map_err(|_| Refusal::NotForThisHost)?;
    if out.len() as u64 > MAX_PLAINTEXT {
        return Err(Refusal::Malformed("larger than 1 MiB".into()));
    }
    Ok(out)
}

/// Open a request: decrypt it with `host`, find the seeded key in `ring`
/// that signed it, and check it is fresh and for this server. The token is
/// the caller's to check, against the returned signer.
pub fn open(
    host: &HostKey,
    ring: &Keyring,
    armored: &[u8],
    replay: &Replay,
    now: i64,
) -> Result<Opened, Refusal> {
    let (msg, _) = Message::from_armor(armored).map_err(|_| Refusal::NotForThisHost)?;
    if !msg.is_encrypted() {
        return Err(Refusal::NotForThisHost);
    }
    let mut msg = msg
        .decrypt(&Password::empty(), &host.secret)
        .map_err(|_| Refusal::NotForThisHost)?;
    if msg.is_compressed() {
        msg = msg.decompress().map_err(|_| Refusal::NotForThisHost)?;
    }
    if !msg.is_signed() {
        // Read nothing from an unsigned message: it has no author to answer.
        return Err(Refusal::Unsigned);
    }
    let plaintext = read_limited(&mut msg)?;
    let seeded = ring
        .keys()
        .iter()
        .find(|k| k.verifiers().into_iter().any(|v| msg.verify(v).is_ok()))
        .cloned();

    let (signer, delegated, request) = match seeded {
        Some(signer) => {
            let request: Request = serde_json::from_slice(&plaintext)
                .map_err(|e| Refusal::Malformed(e.to_string()))?;
            if request.delegation.is_some() {
                return Err(Refusal::Malformed(
                    "a seeded key signs for itself, with no delegation".into(),
                ));
            }
            (signer, None, request)
        }
        None => {
            // Anyone else gets this far only with a seeded key's word for
            // them; until it checks out the plaintext is nobody's.
            let request: Request =
                serde_json::from_slice(&plaintext).map_err(|_| Refusal::Unsigned)?;
            let armored = request.delegation.as_deref().ok_or(Refusal::Unsigned)?;
            let (owner, delegation) = open_delegation(ring, armored)?;
            if !delegation.host.eq_ignore_ascii_case(host.fingerprint()) {
                return Err(Refusal::WrongHost);
            }
            let companion =
                SeededKey::from_armored(&delegation.key).map_err(|_| Refusal::Unsigned)?;
            if !companion
                .verifiers()
                .into_iter()
                .any(|v| msg.verify(v).is_ok())
            {
                return Err(Refusal::Unsigned);
            }
            let delegated = Delegated {
                owner,
                token_id: delegation.token,
            };
            (companion, Some(delegated), request)
        }
    };
    if request.v != 1 {
        return Err(Refusal::Malformed(format!("version {}", request.v)));
    }
    if !request.host.eq_ignore_ascii_case(host.fingerprint()) {
        return Err(Refusal::WrongHost);
    }
    if (request.ts - now).abs() > MAX_SKEW_SECS {
        return Err(Refusal::Stale);
    }
    if request.nonce.len() != 32 || !request.nonce.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Refusal::Malformed(
            "the nonce is not 32 hex characters".into(),
        ));
    }
    replay.admit(&request.nonce.to_ascii_lowercase(), now)?;
    Ok(Opened {
        signer,
        request,
        delegated,
    })
}

/// Check a [`Delegation`]: signed, not encrypted, by a key in `ring`.
fn open_delegation(ring: &Keyring, armored: &str) -> Result<(SeededKey, Delegation), Refusal> {
    let (msg, _) = Message::from_armor(armored.as_bytes()).map_err(|_| Refusal::Unsigned)?;
    if msg.is_encrypted() {
        return Err(Refusal::Unsigned);
    }
    let mut msg = msg;
    if msg.is_compressed() {
        msg = msg.decompress().map_err(|_| Refusal::Unsigned)?;
    }
    if !msg.is_signed() {
        return Err(Refusal::Unsigned);
    }
    let plaintext = read_limited(&mut msg)?;
    let owner = ring
        .keys()
        .iter()
        .find(|k| k.verifiers().into_iter().any(|v| msg.verify(v).is_ok()))
        .ok_or(Refusal::Unsigned)?
        .clone();
    let delegation: Delegation = serde_json::from_slice(&plaintext)
        .map_err(|e| Refusal::Malformed(format!("the delegation: {e}")))?;
    if delegation.v != 1 || delegation.kind != DELEGATION_KIND {
        return Err(Refusal::Malformed(
            "the delegation is not a companion's".into(),
        ));
    }
    Ok((owner, delegation))
}

/// The answer's plaintext.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Answer {
    pub v: u32,
    pub nonce: String,
    pub status: u16,
    pub body: serde_json::Value,
}

fn sign_and_encrypt(
    signer: &HostKey,
    recipient: &SeededKey,
    plaintext: Vec<u8>,
) -> Result<String, KeyError> {
    let mut rng = rand::thread_rng();
    let mut builder =
        MessageBuilder::from_bytes("", plaintext).seipd_v1(&mut rng, SymmetricKeyAlgorithm::AES256);
    for sub in recipient.encryption_subkeys() {
        builder
            .encrypt_to_key(&mut rng, sub)
            .map_err(|e| KeyError::Pgp(format!("encrypting to {}: {e}", recipient.fingerprint)))?;
    }
    builder.sign(
        &signer.secret.primary_key,
        Password::empty(),
        HashAlgorithm::Sha256,
    );
    builder
        .to_armored_string(&mut rng, ArmorOptions::default())
        .map_err(|e| KeyError::Pgp(format!("sealing: {e}")))
}

/// Seal an answer to the key that signed the request.
pub fn seal(
    host: &HostKey,
    recipient: &SeededKey,
    nonce: &str,
    status: u16,
    body: serde_json::Value,
) -> Result<String, KeyError> {
    let answer = Answer {
        v: 1,
        nonce: nonce.to_string(),
        status,
        body,
    };
    let bytes = serde_json::to_vec(&answer).map_err(|e| KeyError::Pgp(e.to_string()))?;
    sign_and_encrypt(host, recipient, bytes)
}

/// The client's half, for tests and for Rust callers: seal `request` from
/// `client` to the server whose public key is `server`.
pub fn seal_request(
    client: &HostKey,
    server: &SeededKey,
    request: &Request,
) -> Result<String, KeyError> {
    let bytes = serde_json::to_vec(request).map_err(|e| KeyError::Pgp(e.to_string()))?;
    sign_and_encrypt(client, server, bytes)
}

/// The client's half: open an answer, which must be signed by `server`.
pub fn open_answer(
    client: &HostKey,
    server: &SeededKey,
    armored: &[u8],
) -> Result<Answer, KeyError> {
    let bad = |what: &str| KeyError::Unusable(format!("the answer {what}"));
    let (msg, _) = Message::from_armor(armored).map_err(|_| bad("is not an OpenPGP message"))?;
    let mut msg = msg
        .decrypt(&Password::empty(), &client.secret)
        .map_err(|_| bad("is not for this key"))?;
    if msg.is_compressed() {
        msg = msg.decompress().map_err(|_| bad("does not decompress"))?;
    }
    if !msg.is_signed() {
        return Err(bad("is not signed"));
    }
    let plaintext = read_limited(&mut msg).map_err(|_| bad("is too large"))?;
    if !server
        .verifiers()
        .into_iter()
        .any(|v| msg.verify(v).is_ok())
    {
        return Err(bad("was not signed by the server's key"));
    }
    serde_json::from_slice(&plaintext).map_err(|e| KeyError::Unusable(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct World {
        host: HostKey,
        owner: HostKey,
        stranger: HostKey,
        ring: Keyring,
    }

    fn world() -> World {
        let host = HostKey::generate("LosOS box test").unwrap();
        let owner = HostKey::generate("Owner <owner@example.com>").unwrap();
        let stranger = HostKey::generate("Stranger <s@example.com>").unwrap();
        let ring = Keyring::parse(owner.armored_public()).unwrap();
        World {
            host,
            owner,
            stranger,
            ring,
        }
    }

    fn request(w: &World, now: i64, nonce: &str) -> Request {
        Request {
            v: 1,
            host: w.host.fingerprint().to_string(),
            token: "losos_pat_00000000_0000000000000000000000000000000000000000".into(),
            method: "GET".into(),
            path: "/api/state".into(),
            ts: now,
            nonce: nonce.into(),
            body: serde_json::Value::Null,
            delegation: None,
        }
    }

    /// A fresh nonce, as a client makes one.
    fn fresh_nonce() -> String {
        hex::encode(rand::random::<[u8; 16]>())
    }

    static N1: std::sync::LazyLock<String> = std::sync::LazyLock::new(fresh_nonce);
    static N2: std::sync::LazyLock<String> = std::sync::LazyLock::new(fresh_nonce);

    #[test]
    fn a_call_round_trips_both_ways() {
        let w = world();
        let server = w.host.as_seeded().unwrap();
        let sealed = seal_request(&w.owner, &server, &request(&w, 1000, &N1)).unwrap();
        let opened = open(
            &w.host,
            &w.ring,
            sealed.as_bytes(),
            &Replay::default(),
            1000,
        )
        .unwrap();
        assert_eq!(opened.signer.fingerprint, w.owner.fingerprint());
        assert_eq!(opened.request.path, "/api/state");

        let answer = seal(
            &w.host,
            &opened.signer,
            &N1,
            200,
            serde_json::json!({"ok": true}),
        )
        .unwrap();
        let back = open_answer(&w.owner, &server, answer.as_bytes()).unwrap();
        assert_eq!(back.nonce, *N1);
        assert_eq!(back.status, 200);
        assert_eq!(back.body, serde_json::json!({"ok": true}));
    }

    #[test]
    fn a_key_that_was_not_seeded_is_refused() {
        let w = world();
        let server = w.host.as_seeded().unwrap();
        let sealed = seal_request(&w.stranger, &server, &request(&w, 1000, &N1)).unwrap();
        let got = open(
            &w.host,
            &w.ring,
            sealed.as_bytes(),
            &Replay::default(),
            1000,
        );
        assert_eq!(got.unwrap_err(), Refusal::Unsigned);
    }

    #[test]
    fn a_message_for_another_server_is_refused() {
        let w = world();
        let other = HostKey::generate("other box").unwrap().as_seeded().unwrap();
        let sealed = seal_request(&w.owner, &other, &request(&w, 1000, &N1)).unwrap();
        let got = open(
            &w.host,
            &w.ring,
            sealed.as_bytes(),
            &Replay::default(),
            1000,
        );
        assert_eq!(got.unwrap_err(), Refusal::NotForThisHost);
        assert_eq!(
            open(&w.host, &w.ring, b"not armour", &Replay::default(), 1000).unwrap_err(),
            Refusal::NotForThisHost
        );
    }

    #[test]
    fn a_request_naming_another_host_fingerprint_is_refused() {
        let w = world();
        let server = w.host.as_seeded().unwrap();
        let mut req = request(&w, 1000, &N1);
        req.host = w.stranger.fingerprint().to_string();
        let sealed = seal_request(&w.owner, &server, &req).unwrap();
        let got = open(
            &w.host,
            &w.ring,
            sealed.as_bytes(),
            &Replay::default(),
            1000,
        );
        assert_eq!(got.unwrap_err(), Refusal::WrongHost);
    }

    #[test]
    fn stale_and_replayed_requests_are_refused() {
        let w = world();
        let server = w.host.as_seeded().unwrap();
        let replay = Replay::default();
        let sealed = seal_request(&w.owner, &server, &request(&w, 1000, &N1)).unwrap();
        assert_eq!(
            open(
                &w.host,
                &w.ring,
                sealed.as_bytes(),
                &replay,
                1000 + MAX_SKEW_SECS + 1
            )
            .unwrap_err(),
            Refusal::Stale
        );
        assert!(open(&w.host, &w.ring, sealed.as_bytes(), &replay, 1000).is_ok());
        assert_eq!(
            open(&w.host, &w.ring, sealed.as_bytes(), &replay, 1001).unwrap_err(),
            Refusal::Replayed
        );
        // Another nonce is another request.
        let fresh = seal_request(&w.owner, &server, &request(&w, 1000, &N2)).unwrap();
        assert!(open(&w.host, &w.ring, fresh.as_bytes(), &replay, 1001).is_ok());
    }

    #[test]
    fn an_answer_signed_by_another_key_is_refused() {
        let w = world();
        let owner = w.ring.get(w.owner.fingerprint()).unwrap().clone();
        let forged = seal(&w.stranger, &owner, &N1, 200, serde_json::Value::Null).unwrap();
        let server = w.host.as_seeded().unwrap();
        assert!(open_answer(&w.owner, &server, forged.as_bytes()).is_err());
    }

    #[test]
    fn a_bad_nonce_is_malformed() {
        let w = world();
        let server = w.host.as_seeded().unwrap();
        let sealed =
            seal_request(&w.owner, &server, &request(&w, 1000, &fresh_nonce()[..5])).unwrap();
        let got = open(
            &w.host,
            &w.ring,
            sealed.as_bytes(),
            &Replay::default(),
            1000,
        );
        assert!(matches!(got.unwrap_err(), Refusal::Malformed(_)));
    }

    #[test]
    fn the_host_key_survives_a_restart() {
        let dir = std::env::temp_dir().join(format!("losos-access-{}", std::process::id()));
        let path = dir.join("host-key.asc");
        let _ = std::fs::remove_dir_all(&dir);
        let first = HostKey::load_or_create(&path, "LosOS box test").unwrap();
        let second = HostKey::load_or_create(&path, "LosOS box test").unwrap();
        assert_eq!(first.fingerprint(), second.fingerprint());
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        std::fs::write(&path, "garbage").unwrap();
        assert!(HostKey::load_or_create(&path, "x").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_ring_takes_several_blocks_and_drops_duplicates() {
        let a = HostKey::generate("a").unwrap();
        let b = HostKey::generate("b").unwrap();
        let text = format!(
            "{}\n{}\n{}",
            a.armored_public(),
            b.armored_public(),
            a.armored_public()
        );
        let ring = Keyring::parse(&text).unwrap();
        assert_eq!(ring.keys().len(), 2);
        assert!(ring.get(&a.fingerprint().to_lowercase()).is_some());
        assert!(Keyring::parse("").unwrap().is_empty());
        assert!(Keyring::parse("-----BEGIN PGP PUBLIC KEY BLOCK-----\nnope").is_err());
    }
}
