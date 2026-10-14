//! The two kinds of key a server holds: its own ([`HostKey`], made on first
//! start, secret half never leaves the file) and the owner's seeded public
//! keys ([`Keyring`], read from configuration, never written).

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use pgp::composed::{
    ArmorOptions, Deserializable, EncryptionCaps, KeyType, SecretKeyParamsBuilder, SignedPublicKey,
    SignedPublicSubKey, SignedSecretKey, SubkeyParamsBuilder,
};
use pgp::crypto::ecc_curve::ECCCurve;
use pgp::types::{KeyDetails, VerifyingKey};

use crate::fingerprint_hex;

#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    #[error("{0}")]
    Pgp(String),
    #[error("{what}: {source}")]
    Io {
        what: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{0}")]
    Unusable(String),
}

fn pgp_err(what: &str) -> impl Fn(pgp::errors::Error) -> KeyError + '_ {
    move |e| KeyError::Pgp(format!("{what}: {e}"))
}

/// This server's own OpenPGP key: an Ed25519 primary that signs every
/// answer, and a Curve25519 subkey every request is encrypted to.
pub struct HostKey {
    pub(crate) secret: SignedSecretKey,
    pub(crate) public: SignedPublicKey,
    fingerprint: String,
    armored_public: String,
}

impl HostKey {
    /// A new key with `user_id` on it (`LosOS box <hostname>`).
    pub fn generate(user_id: &str) -> Result<HostKey, KeyError> {
        let mut rng = rand::thread_rng();
        let subkey = SubkeyParamsBuilder::default()
            .key_type(KeyType::ECDH(ECCCurve::Curve25519Legacy))
            .can_sign(false)
            .can_encrypt(EncryptionCaps::All)
            .build()
            .map_err(|e| KeyError::Pgp(format!("the encryption subkey: {e}")))?;
        let params = SecretKeyParamsBuilder::default()
            .key_type(KeyType::Ed25519Legacy)
            .can_certify(true)
            .can_sign(true)
            .can_encrypt(EncryptionCaps::None)
            .primary_user_id(user_id.into())
            .subkeys(vec![subkey])
            .build()
            .map_err(|e| KeyError::Pgp(format!("the key parameters: {e}")))?;
        let secret = params
            .generate(&mut rng)
            .map_err(pgp_err("generating the host key"))?;
        HostKey::from_secret(secret)
    }

    /// Parse an armoured secret key, as [`HostKey::armored_secret`] writes it.
    pub fn from_armored(text: &str) -> Result<HostKey, KeyError> {
        let (secret, _) = SignedSecretKey::from_string(text).map_err(pgp_err("the host key"))?;
        secret
            .verify_bindings()
            .map_err(pgp_err("the host key's bindings"))?;
        HostKey::from_secret(secret)
    }

    fn from_secret(secret: SignedSecretKey) -> Result<HostKey, KeyError> {
        let public = secret.to_public_key();
        let armored_public = public
            .to_armored_string(ArmorOptions::default())
            .map_err(pgp_err("armouring the host key"))?;
        let fingerprint = fingerprint_hex(&public.fingerprint());
        if encryption_subkeys(&public).is_empty() {
            return Err(KeyError::Unusable(
                "the host key has no encryption subkey".into(),
            ));
        }
        Ok(HostKey {
            secret,
            public,
            fingerprint,
            armored_public,
        })
    }

    /// The key at `path`, made and written (mode 0600) when the file does
    /// not exist yet. A file that exists but does not parse is an error,
    /// never a reason to make a new key: the owner pinned the old one.
    pub fn load_or_create(path: &Path, user_id: &str) -> Result<HostKey, KeyError> {
        match std::fs::read_to_string(path) {
            Ok(text) => HostKey::from_armored(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let key = HostKey::generate(user_id)?;
                write_private(path, key.armored_secret()?.as_bytes()).map_err(|source| {
                    KeyError::Io {
                        what: format!("writing {}", path.display()),
                        source,
                    }
                })?;
                Ok(key)
            }
            Err(source) => Err(KeyError::Io {
                what: format!("reading {}", path.display()),
                source,
            }),
        }
    }

    pub fn armored_secret(&self) -> Result<String, KeyError> {
        self.secret
            .to_armored_string(ArmorOptions::default())
            .map_err(pgp_err("armouring the host key"))
    }

    /// Upper-case hex, no spaces.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn armored_public(&self) -> &str {
        &self.armored_public
    }

    /// The public half as a [`SeededKey`], so tests can play the server's
    /// part from the other side.
    pub fn as_seeded(&self) -> Result<SeededKey, KeyError> {
        SeededKey::from_key(self.public.clone())
    }
}

/// Write `bytes` to `path` with mode 0600, by a temporary file and a rename
/// so a crash never leaves half a key.
pub(crate) fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

/// One of the owner's seeded public keys.
#[derive(Clone)]
pub struct SeededKey {
    pub fingerprint: String,
    pub user_ids: Vec<String>,
    /// When the key was made, Unix seconds.
    pub created: i64,
    pub(crate) key: SignedPublicKey,
}

impl std::fmt::Debug for SeededKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SeededKey")
            .field("fingerprint", &self.fingerprint)
            .field("user_ids", &self.user_ids)
            .finish()
    }
}

impl SeededKey {
    fn from_key(key: SignedPublicKey) -> Result<SeededKey, KeyError> {
        let fingerprint = fingerprint_hex(&key.fingerprint());
        key.verify_bindings()
            .map_err(|e| KeyError::Unusable(format!("{fingerprint}: {e}")))?;
        if encryption_subkeys(&key).is_empty() {
            return Err(KeyError::Unusable(format!(
                "{fingerprint} has no encryption subkey, so no answer could be sealed to it"
            )));
        }
        let user_ids = key
            .details
            .users
            .iter()
            .map(|u| String::from_utf8_lossy(u.id.id()).into_owned())
            .collect();
        let created = i64::from(key.created_at().as_secs());
        Ok(SeededKey {
            fingerprint,
            user_ids,
            created,
            key,
        })
    }

    /// The primary key and every subkey bound for signing: what may have made
    /// a request's signature. gpg signs with the primary unless the key has a
    /// signing subkey.
    pub(crate) fn verifiers(&self) -> Vec<&dyn VerifyingKey> {
        let mut out: Vec<&dyn VerifyingKey> = vec![&self.key.primary_key];
        for sub in &self.key.public_subkeys {
            if sub.signatures.iter().any(|s| s.key_flags().sign()) {
                out.push(&sub.key);
            }
        }
        out
    }

    pub(crate) fn encryption_subkeys(&self) -> Vec<&SignedPublicSubKey> {
        encryption_subkeys(&self.key)
    }
}

fn encryption_subkeys(key: &SignedPublicKey) -> Vec<&SignedPublicSubKey> {
    key.public_subkeys
        .iter()
        .filter(|sub| {
            sub.key.algorithm().can_encrypt()
                && sub.signatures.iter().any(|s| {
                    let flags = s.key_flags();
                    flags.encrypt_comms() || flags.encrypt_storage()
                })
        })
        .collect()
}

/// The owner's seeded keys, in the order they were listed.
#[derive(Clone, Debug, Default)]
pub struct Keyring {
    keys: Vec<SeededKey>,
}

const BEGIN: &str = "-----BEGIN PGP PUBLIC KEY BLOCK-----";

impl Keyring {
    /// Every public key block in `text`, which may hold several one after
    /// another (`losos.access.gpgKeys` is a list, written out joined). A
    /// block that does not parse, or a key that could never be answered,
    /// fails the whole ring: a typo in the seed should be loud at start,
    /// not a key that silently never works.
    pub fn parse(text: &str) -> Result<Keyring, KeyError> {
        let mut keys: Vec<SeededKey> = Vec::new();
        for block in text.split(BEGIN).skip(1) {
            let armored = format!("{BEGIN}{block}");
            let (key, _) =
                SignedPublicKey::from_string(&armored).map_err(pgp_err("a seeded GPG key"))?;
            let seeded = SeededKey::from_key(key)?;
            if !keys.iter().any(|k| k.fingerprint == seeded.fingerprint) {
                keys.push(seeded);
            }
        }
        Ok(Keyring { keys })
    }

    /// The ring at `path`; no file is an empty ring (no keys were seeded).
    pub fn load(path: &Path) -> Result<Keyring, KeyError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Keyring::parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Keyring::default()),
            Err(source) => Err(KeyError::Io {
                what: format!("reading {}", path.display()),
                source,
            }),
        }
    }

    pub fn keys(&self) -> &[SeededKey] {
        &self.keys
    }

    pub fn get(&self, fingerprint: &str) -> Option<&SeededKey> {
        self.keys
            .iter()
            .find(|k| k.fingerprint.eq_ignore_ascii_case(fingerprint))
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}
