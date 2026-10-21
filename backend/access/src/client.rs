//! The client's half of the envelope, for the `losos` terminal client: the
//! owner's own secret key, which signs every request and reads every
//! answer, and the two calls that seal a request and open its answer.
//!
//! The owner's key comes from gpg, so unlike [`HostKey`](crate::HostKey) it
//! may sit under a passphrase and may sign with a subkey. It is checked
//! against the contract when it is read (`docs/terminal-access.md`): a
//! secret key, able to sign, with an encryption subkey the server can seal
//! its answers to.

use pgp::composed::{
    ArmorOptions, Deserializable, Message, MessageBuilder, SignedPublicKey, SignedSecretKey,
};
use pgp::crypto::hash::HashAlgorithm;
use pgp::crypto::sym::SymmetricKeyAlgorithm;
use pgp::types::{KeyDetails, Password, SigningKey};

use crate::envelope::{read_limited, Answer, Delegation, Request, DELEGATION_KIND};
use crate::keys::{encryption_subkeys, KeyError, SeededKey};

/// Why a key cannot be the owner's key.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UserKeyError {
    #[error("that is not an ASCII-armoured OpenPGP key")]
    NotArmoured,
    #[error("that is a public key; the secret key is needed")]
    NotSecret,
    #[error("this key cannot sign")]
    CannotSign,
    #[error("this key has no encryption subkey")]
    NoEncryption,
    #[error("that passphrase does not unlock the key")]
    WrongPassphrase,
}

const SECRET_BEGIN: &str = "-----BEGIN PGP PRIVATE KEY BLOCK-----";
const PUBLIC_BEGIN: &str = "-----BEGIN PGP PUBLIC KEY BLOCK-----";

/// The owner's secret key, as exported, possibly still under its passphrase.
#[derive(Clone)]
pub struct UserKey {
    secret: SignedSecretKey,
    public: SignedPublicKey,
    fingerprint: String,
    user_ids: Vec<String>,
}

impl std::fmt::Debug for UserKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserKey")
            .field("fingerprint", &self.fingerprint)
            .field("user_ids", &self.user_ids)
            .finish()
    }
}

/// Which of the key's parts makes the signature: a signing subkey when the
/// key has one, as gpg does, else the primary.
enum Signer {
    Primary,
    Subkey(usize),
}

impl UserKey {
    /// Read an armoured secret key and check it against the contract. A key
    /// under a passphrase is checked as it is; nothing here needs it.
    pub fn from_armored(text: &str) -> Result<UserKey, UserKeyError> {
        let trimmed = text.trim_start();
        if trimmed.starts_with(PUBLIC_BEGIN) {
            return Err(UserKeyError::NotSecret);
        }
        if !trimmed.starts_with(SECRET_BEGIN) {
            return Err(UserKeyError::NotArmoured);
        }
        let (secret, _) =
            SignedSecretKey::from_string(trimmed).map_err(|_| UserKeyError::NotArmoured)?;
        let public = secret.to_public_key();
        let fingerprint = crate::fingerprint_hex(&public.fingerprint());
        let user_ids = public
            .details
            .users
            .iter()
            .map(|u| String::from_utf8_lossy(u.id.id()).into_owned())
            .collect();
        let key = UserKey {
            secret,
            public,
            fingerprint,
            user_ids,
        };
        key.signer().ok_or(UserKeyError::CannotSign)?;
        if encryption_subkeys(&key.public).is_empty() {
            return Err(UserKeyError::NoEncryption);
        }
        Ok(key)
    }

    /// Upper-case hex, no spaces.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn user_ids(&self) -> &[String] {
        &self.user_ids
    }

    /// True when the key is under a passphrase.
    pub fn needs_passphrase(&self) -> bool {
        self.secret.primary_key.secret_params().is_encrypted()
            || self
                .secret
                .secret_subkeys
                .iter()
                .any(|s| s.key.secret_params().is_encrypted())
    }

    /// The key as it was read, armoured again (still under its passphrase).
    pub fn armored_secret(&self) -> Result<String, KeyError> {
        self.secret
            .to_armored_string(ArmorOptions::default())
            .map_err(|e| KeyError::Pgp(format!("armouring the key: {e}")))
    }

    pub fn armored_public(&self) -> Result<String, KeyError> {
        self.public
            .to_armored_string(ArmorOptions::default())
            .map_err(|e| KeyError::Pgp(format!("armouring the key: {e}")))
    }

    fn signer(&self) -> Option<Signer> {
        let can_sign = |sigs: &[pgp::packet::Signature]| sigs.iter().any(|s| s.key_flags().sign());
        let newest = self
            .secret
            .secret_subkeys
            .iter()
            .enumerate()
            .filter(|(_, s)| s.key.algorithm().can_sign() && can_sign(&s.signatures))
            .max_by_key(|(_, s)| s.key.created_at().as_secs())
            .map(|(i, _)| Signer::Subkey(i));
        if newest.is_some() {
            return newest;
        }
        let primary_flags: Vec<pgp::packet::Signature> = self
            .secret
            .details
            .users
            .iter()
            .flat_map(|u| u.signatures.iter().cloned())
            .collect();
        // A self-signature with no key flags at all says nothing; the
        // algorithm decides then.
        let flagged = primary_flags.iter().any(|s| s.key_flags().sign())
            || primary_flags
                .iter()
                .all(|s| s.key_flags() == Default::default());
        (self.secret.primary_key.algorithm().can_sign() && flagged).then_some(Signer::Primary)
    }

    fn signing_key(&self) -> &dyn SigningKey {
        match self.signer() {
            Some(Signer::Subkey(i)) => &self.secret.secret_subkeys[i].key,
            _ => &self.secret.primary_key,
        }
    }

    /// The key, unlocked with `passphrase` (empty for a key under none). The
    /// passphrase is tried on every part the client uses, so a wrong one is
    /// reported here and not halfway through a call.
    pub fn unlock(&self, passphrase: &str) -> Result<Unlocked, UserKeyError> {
        let password = Password::from(passphrase);
        let check = |r: pgp::errors::Result<pgp::errors::Result<()>>| match r {
            Ok(Ok(())) => Ok(()),
            _ => Err(UserKeyError::WrongPassphrase),
        };
        check(self.secret.primary_key.unlock(&password, |_, _| Ok(())))?;
        for sub in &self.secret.secret_subkeys {
            check(sub.key.unlock(&password, |_, _| Ok(())))?;
        }
        Ok(Unlocked {
            key: self.clone(),
            passphrase: passphrase.to_string(),
        })
    }
}

/// A [`UserKey`] with the passphrase that unlocks it, for one run. The key
/// stays encrypted; the passphrase is applied to each signature and each
/// answer, so nothing decrypted is kept between calls.
#[derive(Clone)]
pub struct Unlocked {
    key: UserKey,
    passphrase: String,
}

impl std::fmt::Debug for Unlocked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Unlocked")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

impl Unlocked {
    pub fn key(&self) -> &UserKey {
        &self.key
    }

    fn password(&self) -> Password {
        Password::from(self.passphrase.as_str())
    }
}

impl SeededKey {
    /// A server's public key, as `/api/access/host-key` serves it.
    pub fn from_armored(text: &str) -> Result<SeededKey, KeyError> {
        let (key, _) = SignedPublicKey::from_string(text.trim_start())
            .map_err(|e| KeyError::Pgp(format!("the host key: {e}")))?;
        SeededKey::from_key(key)
    }
}

/// Seal `request` from `user` to the server whose key is `server`: signed,
/// then encrypted, in one OpenPGP message.
pub fn seal_call(
    user: &Unlocked,
    server: &SeededKey,
    request: &Request,
) -> Result<String, KeyError> {
    let bytes = serde_json::to_vec(request).map_err(|e| KeyError::Pgp(e.to_string()))?;
    let mut rng = rand::thread_rng();
    let mut builder =
        MessageBuilder::from_bytes("", bytes).seipd_v1(&mut rng, SymmetricKeyAlgorithm::AES256);
    for sub in server.encryption_subkeys() {
        builder
            .encrypt_to_key(&mut rng, sub)
            .map_err(|e| KeyError::Pgp(format!("encrypting to {}: {e}", server.fingerprint)))?;
    }
    builder.sign(
        user.key.signing_key(),
        user.password(),
        HashAlgorithm::Sha256,
    );
    builder
        .to_armored_string(&mut rng, ArmorOptions::default())
        .map_err(|e| KeyError::Pgp(format!("sealing: {e}")))
}

/// The owner's word for a companion key (`companion_public`, armoured):
/// a [`Delegation`] for the server whose key fingerprint is `host` and the
/// token whose id is `token_id`, signed by `owner` and not encrypted. The
/// companion sends it with every request it signs.
pub fn delegate(
    owner: &Unlocked,
    companion_public: &str,
    host: &str,
    token_id: &str,
    now: i64,
) -> Result<String, KeyError> {
    let delegation = Delegation {
        v: 1,
        kind: DELEGATION_KIND.to_string(),
        key: companion_public.to_string(),
        host: host.to_ascii_uppercase(),
        token: token_id.to_ascii_lowercase(),
        created: now,
    };
    let bytes = serde_json::to_vec(&delegation).map_err(|e| KeyError::Pgp(e.to_string()))?;
    let mut builder = MessageBuilder::from_bytes("", bytes);
    builder.sign(
        owner.key.signing_key(),
        owner.password(),
        HashAlgorithm::Sha256,
    );
    builder
        .to_armored_string(rand::thread_rng(), ArmorOptions::default())
        .map_err(|e| KeyError::Pgp(format!("signing the delegation: {e}")))
}

/// Why an answer was not believed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AnswerError {
    #[error("the answer could not be decrypted with this key")]
    NotForUs,
    #[error("the answer was not signed by the pinned host key")]
    BadSignature,
    #[error("the answer is not a terminal answer")]
    NotAnswer,
}

/// Open an answer from `server`: it must decrypt with `user`'s key and carry
/// a signature by `server`. The nonce is the caller's to check.
pub fn open_call(
    user: &Unlocked,
    server: &SeededKey,
    armored: &[u8],
) -> Result<Answer, AnswerError> {
    let (msg, _) = Message::from_armor(armored).map_err(|_| AnswerError::NotAnswer)?;
    let mut msg = msg
        .decrypt(&user.password(), &user.key.secret)
        .map_err(|_| AnswerError::NotForUs)?;
    if msg.is_compressed() {
        msg = msg.decompress().map_err(|_| AnswerError::NotAnswer)?;
    }
    if !msg.is_signed() {
        return Err(AnswerError::BadSignature);
    }
    let plaintext = read_limited(&mut msg).map_err(|_| AnswerError::NotAnswer)?;
    if !server
        .verifiers()
        .into_iter()
        .any(|v| msg.verify(v).is_ok())
    {
        return Err(AnswerError::BadSignature);
    }
    let answer: Answer = serde_json::from_slice(&plaintext).map_err(|_| AnswerError::NotAnswer)?;
    if answer.v != 1 {
        return Err(AnswerError::NotAnswer);
    }
    Ok(answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{open, seal, HostKey, Keyring, Refusal, Replay};

    fn request(host: &str, now: i64) -> Request {
        Request {
            v: 1,
            host: host.to_string(),
            token: "losos_pat_00000000_0000000000000000000000000000000000000000".into(),
            method: "GET".into(),
            path: "/api/state".into(),
            ts: now,
            nonce: "0123456789abcdef0123456789abcdef".into(),
            body: serde_json::Value::Null,
            delegation: None,
        }
    }

    #[test]
    fn a_call_goes_both_ways() {
        let host = HostKey::generate("LosOS box test").unwrap();
        let owner = HostKey::generate("Owner <owner@example.com>").unwrap();
        let user = UserKey::from_armored(&owner.armored_secret().unwrap()).unwrap();
        assert_eq!(user.fingerprint(), owner.fingerprint());
        assert_eq!(user.user_ids(), ["Owner <owner@example.com>"]);
        assert!(!user.needs_passphrase());
        let unlocked = user.unlock("").unwrap();
        let server = SeededKey::from_armored(host.armored_public()).unwrap();
        let now = crate::now_secs();

        let sealed = seal_call(&unlocked, &server, &request(host.fingerprint(), now)).unwrap();
        let ring = Keyring::parse(owner.armored_public()).unwrap();
        let opened = open(&host, &ring, sealed.as_bytes(), &Replay::default(), now).unwrap();
        assert_eq!(opened.signer.fingerprint, owner.fingerprint());

        let answer = seal(
            &host,
            &opened.signer,
            &opened.request.nonce,
            200,
            serde_json::json!({"ok": true}),
        )
        .unwrap();
        let back = open_call(&unlocked, &server, answer.as_bytes()).unwrap();
        assert_eq!(back.status, 200);
        assert_eq!(back.nonce, opened.request.nonce);
        assert_eq!(back.body, serde_json::json!({"ok": true}));
    }

    #[test]
    fn an_answer_signed_by_another_key_is_refused() {
        let host = HostKey::generate("LosOS box test").unwrap();
        let impostor = HostKey::generate("Impostor").unwrap();
        let owner = HostKey::generate("Owner").unwrap();
        let user = UserKey::from_armored(&owner.armored_secret().unwrap()).unwrap();
        let unlocked = user.unlock("").unwrap();
        let server = SeededKey::from_armored(host.armored_public()).unwrap();
        let recipient = owner.as_seeded().unwrap();
        let forged = seal(&impostor, &recipient, "n", 200, serde_json::Value::Null).unwrap();
        assert_eq!(
            open_call(&unlocked, &server, forged.as_bytes()).unwrap_err(),
            AnswerError::BadSignature
        );
    }

    /// A gpg-shaped key: Ed25519 primary, Curve25519 subkey, both under
    /// `passphrase`.
    fn protected(passphrase: &str) -> String {
        use pgp::composed::{EncryptionCaps, KeyType, SecretKeyParamsBuilder, SubkeyParamsBuilder};
        use pgp::crypto::ecc_curve::ECCCurve;
        let subkey = SubkeyParamsBuilder::default()
            .key_type(KeyType::ECDH(ECCCurve::Curve25519Legacy))
            .can_encrypt(EncryptionCaps::All)
            .passphrase(Some(passphrase.into()))
            .build()
            .unwrap();
        let params = SecretKeyParamsBuilder::default()
            .key_type(KeyType::Ed25519Legacy)
            .can_certify(true)
            .can_sign(true)
            .primary_user_id("Locked <locked@example.com>".into())
            .passphrase(Some(passphrase.into()))
            .subkeys(vec![subkey])
            .build()
            .unwrap();
        let key = params.generate(rand::thread_rng()).unwrap();
        key.to_armored_string(ArmorOptions::default()).unwrap()
    }

    #[test]
    fn a_key_under_a_passphrase_opens_only_with_it() {
        let host = HostKey::generate("LosOS box test").unwrap();
        let user = UserKey::from_armored(&protected("correct horse")).unwrap();
        assert!(user.needs_passphrase());
        assert_eq!(
            user.unlock("wrong").err(),
            Some(UserKeyError::WrongPassphrase)
        );
        let unlocked = user.unlock("correct horse").unwrap();
        let server = SeededKey::from_armored(host.armored_public()).unwrap();
        let now = crate::now_secs();
        let sealed = seal_call(&unlocked, &server, &request(host.fingerprint(), now)).unwrap();
        let ring = Keyring::parse(&user.armored_public().unwrap()).unwrap();
        let opened = open(&host, &ring, sealed.as_bytes(), &Replay::default(), now).unwrap();
        let answer = seal(&host, &opened.signer, "n", 204, serde_json::Value::Null).unwrap();
        assert_eq!(
            open_call(&unlocked, &server, answer.as_bytes())
                .unwrap()
                .status,
            204
        );
    }

    /// The box, the owner's seeded key and a companion key the owner
    /// vouches for, as `losos agents install` leaves them.
    struct Companion {
        host: HostKey,
        ring: Keyring,
        owner: Unlocked,
        companion: Unlocked,
        companion_public: String,
        server: SeededKey,
    }

    fn companion() -> Companion {
        let host = HostKey::generate("LosOS box test").unwrap();
        let owner_key = HostKey::generate("Owner <owner@example.com>").unwrap();
        let ring = Keyring::parse(owner_key.armored_public()).unwrap();
        let owner = UserKey::from_armored(&owner_key.armored_secret().unwrap())
            .unwrap()
            .unlock("")
            .unwrap();
        let made = HostKey::generate("LosOS companion on laptop").unwrap();
        let companion = UserKey::from_armored(&made.armored_secret().unwrap())
            .unwrap()
            .unlock("")
            .unwrap();
        Companion {
            server: SeededKey::from_armored(host.armored_public()).unwrap(),
            host,
            ring,
            owner,
            companion,
            companion_public: made.armored_public().to_string(),
        }
    }

    fn delegated_request(c: &Companion, delegation: String, now: i64) -> Request {
        let mut r = request(c.host.fingerprint(), now);
        r.nonce = hex::encode(rand::random::<[u8; 16]>());
        r.delegation = Some(delegation);
        r
    }

    #[test]
    fn a_companion_signs_for_its_owner_and_reads_its_answers() {
        let c = companion();
        let now = crate::now_secs();
        let word = delegate(
            &c.owner,
            &c.companion_public,
            c.host.fingerprint(),
            "0A1B2C3D",
            now,
        )
        .unwrap();
        let sealed = seal_call(&c.companion, &c.server, &delegated_request(&c, word, now)).unwrap();
        let opened = open(&c.host, &c.ring, sealed.as_bytes(), &Replay::default(), now).unwrap();
        assert_eq!(opened.signer.fingerprint, c.companion.key().fingerprint());
        let d = opened
            .delegated
            .expect("a companion's request is delegated");
        assert_eq!(d.owner.fingerprint, c.owner.key().fingerprint());
        assert_eq!(d.token_id, "0a1b2c3d");

        let answer = seal(
            &c.host,
            &opened.signer,
            &opened.request.nonce,
            200,
            serde_json::json!(1),
        )
        .unwrap();
        assert_eq!(
            open_call(&c.companion, &c.server, answer.as_bytes())
                .unwrap()
                .body,
            serde_json::json!(1)
        );
        // Sealed to the companion, so the owner's own key cannot read it.
        assert!(open_call(&c.owner, &c.server, answer.as_bytes()).is_err());
    }

    #[test]
    fn a_delegation_from_a_key_that_was_not_seeded_is_refused() {
        let c = companion();
        let now = crate::now_secs();
        let stranger = UserKey::from_armored(
            &HostKey::generate("Stranger")
                .unwrap()
                .armored_secret()
                .unwrap(),
        )
        .unwrap()
        .unlock("")
        .unwrap();
        let word = delegate(
            &stranger,
            &c.companion_public,
            c.host.fingerprint(),
            "0a1b2c3d",
            now,
        )
        .unwrap();
        let sealed = seal_call(&c.companion, &c.server, &delegated_request(&c, word, now)).unwrap();
        assert_eq!(
            open(&c.host, &c.ring, sealed.as_bytes(), &Replay::default(), now).unwrap_err(),
            Refusal::Unsigned
        );
    }

    #[test]
    fn a_delegation_for_another_box_or_another_key_is_refused() {
        let c = companion();
        let now = crate::now_secs();
        let elsewhere = HostKey::generate("Another box").unwrap();
        let word = delegate(
            &c.owner,
            &c.companion_public,
            elsewhere.fingerprint(),
            "0a1b2c3d",
            now,
        )
        .unwrap();
        let sealed = seal_call(&c.companion, &c.server, &delegated_request(&c, word, now)).unwrap();
        assert_eq!(
            open(&c.host, &c.ring, sealed.as_bytes(), &Replay::default(), now).unwrap_err(),
            Refusal::WrongHost
        );

        // A good delegation, but the request is signed by some other key.
        let word = delegate(
            &c.owner,
            &c.companion_public,
            c.host.fingerprint(),
            "0a1b2c3d",
            now,
        )
        .unwrap();
        let thief = UserKey::from_armored(
            &HostKey::generate("Thief")
                .unwrap()
                .armored_secret()
                .unwrap(),
        )
        .unwrap()
        .unlock("")
        .unwrap();
        let sealed = seal_call(&thief, &c.server, &delegated_request(&c, word, now)).unwrap();
        assert_eq!(
            open(&c.host, &c.ring, sealed.as_bytes(), &Replay::default(), now).unwrap_err(),
            Refusal::Unsigned
        );
    }

    #[test]
    fn a_seeded_key_does_not_carry_a_delegation_and_noise_is_not_one() {
        let c = companion();
        let now = crate::now_secs();
        let word = delegate(
            &c.owner,
            &c.companion_public,
            c.host.fingerprint(),
            "0a1b2c3d",
            now,
        )
        .unwrap();
        let sealed = seal_call(&c.owner, &c.server, &delegated_request(&c, word, now)).unwrap();
        assert!(matches!(
            open(&c.host, &c.ring, sealed.as_bytes(), &Replay::default(), now).unwrap_err(),
            Refusal::Malformed(_)
        ));
        let sealed = seal_call(
            &c.companion,
            &c.server,
            &delegated_request(&c, "not a delegation".into(), now),
        )
        .unwrap();
        assert_eq!(
            open(&c.host, &c.ring, sealed.as_bytes(), &Replay::default(), now).unwrap_err(),
            Refusal::Unsigned
        );
    }

    #[test]
    fn a_key_under_a_passphrase_delegates_once_and_the_companion_needs_none() {
        let host = HostKey::generate("LosOS box test").unwrap();
        let owner = UserKey::from_armored(&protected("correct horse")).unwrap();
        let ring = Keyring::parse(&owner.armored_public().unwrap()).unwrap();
        let owner = owner.unlock("correct horse").unwrap();
        let made = HostKey::generate("LosOS companion").unwrap();
        let companion = UserKey::from_armored(&made.armored_secret().unwrap()).unwrap();
        assert!(!companion.needs_passphrase());
        let companion = companion.unlock("").unwrap();
        let server = SeededKey::from_armored(host.armored_public()).unwrap();
        let now = crate::now_secs();
        let word = delegate(
            &owner,
            made.armored_public(),
            host.fingerprint(),
            "0a1b2c3d",
            now,
        )
        .unwrap();
        let mut r = request(host.fingerprint(), now);
        r.delegation = Some(word);
        let sealed = seal_call(&companion, &server, &r).unwrap();
        let opened = open(&host, &ring, sealed.as_bytes(), &Replay::default(), now).unwrap();
        assert_eq!(
            opened.delegated.unwrap().owner.fingerprint,
            owner.key().fingerprint()
        );
    }

    #[test]
    fn a_public_key_or_noise_is_not_a_user_key() {
        let owner = HostKey::generate("Owner").unwrap();
        assert_eq!(
            UserKey::from_armored(owner.armored_public()).unwrap_err(),
            UserKeyError::NotSecret
        );
        assert_eq!(
            UserKey::from_armored("hello").unwrap_err(),
            UserKeyError::NotArmoured
        );
    }
}
