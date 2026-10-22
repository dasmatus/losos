//! OpenPGP in LosOS Mail: the public half, on the box.
//!
//! **Where the private key lives.** A person's key is made, or imported, in
//! their browser (OpenPGP.js, `admin-ui/app/src/mail/pgp.ts`), and it is
//! locked there with a passphrase before anything is sent here. What the box
//! keeps is that locked key, sealed again with `systemd-creds` to the TPM or
//! to the host key (`crate::credentials`), so a copy of the disk does not
//! carry it either, and the public key. Nothing on the box ever holds the
//! passphrase: decrypting and signing happen in the browser, after the
//! person types it. [`check_own`] refuses a private key that is not locked.
//!
//! **What the box does with keys.** Everything that needs only public keys:
//!
//!   * it checks a person's public key before it takes it (self-signatures,
//!     an encryption subkey, a user ID with their own address);
//!   * it publishes that key in the box's Web Key Directory, files under
//!     `/var/lib/losos-wkd` that nginx serves at
//!     `/.well-known/openpgpkey/` (`modules/mail.nix`);
//!   * it puts an `Autocrypt:` header on every message the person sends
//!     ([`autocrypt_header`]) and learns the key in the header of every
//!     message the person opens ([`parse_autocrypt`]);
//!   * it finds someone else's key for the composer and for checking a
//!     signature ([`Keys::find`]): a person on this box, a card in LosOS
//!     Contacts with a `KEY`, a key learned from Autocrypt, the recipient's
//!     own Web Key Directory, then a keyserver (keys.openpgp.org unless
//!     `losos.mail.keyserver` says otherwise).
//!
//! The browser checks every key it is handed again before it encrypts to
//! it; this module is where keys are found and kept, not the last word on
//! them.

use std::collections::BTreeMap;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use base64::Engine as _;
use pgp::composed::{ArmorOptions, Deserializable, SignedPublicKey, SignedSecretKey};
use pgp::ser::Serialize as _;
use pgp::types::KeyDetails;
use serde::{Deserialize, Serialize};
use sha1::Digest as _;

use crate::mail::Refused;

/// The largest key the box takes or fetches: a key with a photo and many
/// signatures is a few hundred kilobytes; more than this is not a key
/// anybody needs to send mail.
pub const MAX_KEY: usize = 512 * 1024;
/// Keys learned from Autocrypt one person keeps.
const MAX_PEERS: usize = 2000;

fn refuse<T>(status: u16, message: impl Into<String>) -> Result<T, Refused> {
    Err(Refused {
        status,
        message: message.into(),
    })
}

fn broken(what: &str, e: impl std::fmt::Display) -> Refused {
    tracing::warn!(what, error = %e, "the mail key store failed");
    Refused {
        status: 500,
        message: format!("the box could not {what}"),
    }
}

// ── Reading keys ───────────────────────────────────────────────────────────

/// A public key the box has checked.
#[derive(Debug, Clone)]
pub struct PublicKey {
    pub fingerprint: String,
    pub user_ids: Vec<String>,
    /// Unix seconds.
    pub created: i64,
    key: SignedPublicKey,
}

impl PublicKey {
    /// The key, armoured, for the browser.
    pub fn armored(&self) -> String {
        self.key
            .to_armored_string(ArmorOptions::default())
            .unwrap_or_default()
    }

    /// The key as binary packets, for WKD and Autocrypt. Serialised from
    /// the parsed public key, so nothing secret can ride along.
    pub fn binary(&self) -> Vec<u8> {
        self.key.to_bytes().unwrap_or_default()
    }

    /// The addresses its user IDs name, lower case.
    pub fn emails(&self) -> Vec<String> {
        self.user_ids.iter().filter_map(|u| uid_email(u)).collect()
    }

    pub fn names(&self, email: &str) -> bool {
        let email = email.to_ascii_lowercase();
        self.emails().contains(&email)
    }
}

/// The address in a user ID: `Name <a@b>`, or the whole ID when it is an
/// address.
pub fn uid_email(uid: &str) -> Option<String> {
    let inner = match (uid.rfind('<'), uid.rfind('>')) {
        (Some(a), Some(b)) if a < b => &uid[a + 1..b],
        _ => uid.trim(),
    };
    crate::mail::valid_address(inner).then(|| inner.to_ascii_lowercase())
}

fn has_encryption_key(key: &SignedPublicKey) -> bool {
    let primary = key.details.users.iter().any(|u| {
        u.signatures.iter().any(|s| {
            let f = s.key_flags();
            f.encrypt_comms() || f.encrypt_storage()
        })
    }) && key.primary_key.algorithm().can_encrypt();
    primary
        || key.public_subkeys.iter().any(|sub| {
            sub.key.algorithm().can_encrypt()
                && sub.signatures.iter().any(|s| {
                    let f = s.key_flags();
                    f.encrypt_comms() || f.encrypt_storage()
                })
        })
}

fn checked(key: SignedPublicKey) -> Option<PublicKey> {
    if key.verify_bindings().is_err() || !has_encryption_key(&key) {
        return None;
    }
    let user_ids = key
        .details
        .users
        .iter()
        .map(|u| String::from_utf8_lossy(u.id.id()).into_owned())
        .collect();
    Some(PublicKey {
        fingerprint: format!("{:X}", key.fingerprint()),
        user_ids,
        created: i64::from(key.created_at().as_secs()),
        key,
    })
}

/// Every usable public key in `bytes`, armoured or binary, one or several.
/// A key whose self-signatures do not hold, or that nothing could be
/// encrypted to, is left out.
pub fn read_public(bytes: &[u8]) -> Vec<PublicKey> {
    if bytes.is_empty() || bytes.len() > MAX_KEY {
        return Vec::new();
    }
    let Ok((keys, _)) = SignedPublicKey::from_reader_many(bytes) else {
        return Vec::new();
    };
    keys.filter_map(Result::ok).filter_map(checked).collect()
}

/// What a person sent to set their key: the public half, and the private
/// half locked with their passphrase.
#[derive(Debug, Clone)]
pub struct Own {
    pub public: PublicKey,
    /// The locked key as the browser armoured it, kept byte for byte.
    pub locked: String,
}

/// Check a person's key before the box keeps it: the public key is whole
/// and has their `address`, and the private key is the same key, every
/// part of it locked with a passphrase.
pub fn check_own(public: &str, locked: &str, address: &str) -> Result<Own, Refused> {
    if public.len() > MAX_KEY || locked.len() > MAX_KEY {
        return refuse(413, "that key is larger than a mail key needs to be");
    }
    let mut keys = read_public(public.as_bytes());
    if keys.len() != 1 {
        return refuse(
            400,
            "that is not one public key that mail can be encrypted to",
        );
    }
    let public = keys.remove(0);
    if !public.names(address) {
        return refuse(
            400,
            format!(
                "the key is for {}, not for {address}",
                public.emails().join(", ")
            ),
        );
    }
    let Ok((secret, _)) = SignedSecretKey::from_string(locked) else {
        return refuse(400, "that is not a private key");
    };
    if format!("{:X}", secret.fingerprint()) != public.fingerprint {
        return refuse(
            400,
            "the private key and the public key are not the same key",
        );
    }
    let unlocked = !secret.primary_key.secret_params().is_encrypted()
        || secret
            .secret_subkeys
            .iter()
            .any(|s| !s.key.secret_params().is_encrypted());
    if unlocked {
        return refuse(
            400,
            "lock the private key with a passphrase before the box keeps it",
        );
    }
    Ok(Own {
        public,
        locked: locked.to_string(),
    })
}

// ── Web Key Directory ──────────────────────────────────────────────────────

/// The z-base-32 alphabet (RFC 6189 section 5.1.6), which WKD names files in.
const ZBASE32: &[u8; 32] = b"ybndrfg8ejkmcpqxot1uwisza345h769";

fn zbase32(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for &b in bytes {
        buffer = (buffer << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(char::from(ZBASE32[((buffer >> bits) & 31) as usize]));
        }
    }
    if bits > 0 {
        out.push(char::from(ZBASE32[((buffer << (5 - bits)) & 31) as usize]));
    }
    out
}

/// The file name WKD gives the local part of an address: SHA-1 of it in
/// lower case, in z-base-32.
pub fn wkd_hash(local: &str) -> String {
    zbase32(&sha1::Sha1::digest(local.to_lowercase().as_bytes()))
}

/// Whether `domain` is one the box may ask over the internet: a dotted name
/// of letters, digits and hyphens, not an address, not a name that only
/// means something on this network.
pub fn public_domain(domain: &str) -> bool {
    let d = domain.trim_end_matches('.').to_ascii_lowercase();
    d.contains('.')
        && d.len() <= 253
        && d.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
        && !d.split('.').all(|l| l.chars().all(|c| c.is_ascii_digit()))
        && ![".local", ".home.arpa", ".internal", ".localhost", ".lan"]
            .iter()
            .any(|s| d.ends_with(s))
        && d != "localhost"
}

/// The two places WKD says to look, advanced first.
pub fn wkd_urls(email: &str) -> Option<[String; 2]> {
    let (local, domain) = email.rsplit_once('@')?;
    let domain = domain.to_ascii_lowercase();
    if !public_domain(&domain) {
        return None;
    }
    let hash = wkd_hash(local);
    let l = percent(local);
    Some([
        format!("https://openpgpkey.{domain}/.well-known/openpgpkey/{domain}/hu/{hash}?l={l}"),
        format!("https://{domain}/.well-known/openpgpkey/hu/{hash}?l={l}"),
    ])
}

fn percent(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

// ── Autocrypt (Level 1) ────────────────────────────────────────────────────

/// The `Autocrypt:` header's value for a message from `addr`, folded so
/// no line passes 78 characters: `keydata` is base64, which ignores the
/// folding whitespace.
pub fn autocrypt_header(addr: &str, key: &[u8]) -> String {
    let data = base64::engine::general_purpose::STANDARD.encode(key);
    let mut out = format!(" addr={addr}; prefer-encrypt=mutual; keydata=");
    for chunk in data.as_bytes().chunks(72) {
        out.push_str("\r\n ");
        out.push_str(std::str::from_utf8(chunk).unwrap_or(""));
    }
    out
}

/// The address and key in an `Autocrypt:` header, or `None` when the
/// header is not one Level 1 may use: no `addr` or `keydata`, or an
/// attribute it does not know that does not start with `_`.
pub fn parse_autocrypt(value: &str) -> Option<(String, Vec<u8>)> {
    let mut addr = None;
    let mut keydata = None;
    for part in value.split(';') {
        let (k, v) = part.split_once('=')?;
        let (k, v) = (k.trim(), v.trim());
        match k {
            "addr" => addr = Some(v.to_ascii_lowercase()),
            "keydata" => keydata = Some(v),
            "prefer-encrypt" => {}
            other if other.starts_with('_') => {}
            _ => return None,
        }
    }
    let clean: String = keydata?.chars().filter(|c| !c.is_whitespace()).collect();
    let key = base64::engine::general_purpose::STANDARD
        .decode(clean.as_bytes())
        .ok()?;
    let addr = addr?;
    crate::mail::valid_address(&addr).then_some((addr, key))
}

// ── vCard KEY (LosOS Contacts) ─────────────────────────────────────────────

/// The keys a vCard holds for `email`, when one of its `EMAIL` lines is
/// that address: `KEY` as a `data:` URI (vCard 4) or `ENCODING=b` (vCard
/// 3), or an armoured block in the value. A `KEY` that is a web address is
/// not fetched.
pub fn vcard_keys(card: &str, email: &str) -> Vec<Vec<u8>> {
    // Unfold: a line that starts with a space or a tab continues the last.
    let mut lines: Vec<String> = Vec::new();
    for raw in card.split('\n') {
        let raw = raw.trim_end_matches('\r');
        match raw.strip_prefix([' ', '\t']) {
            Some(rest) if !lines.is_empty() => lines.last_mut().unwrap().push_str(rest),
            _ => lines.push(raw.to_string()),
        }
    }
    let prop = |line: &str| -> Option<(String, String, String)> {
        let (head, value) = line.split_once(':')?;
        let mut parts = head.split(';');
        let name = parts.next()?;
        // `item1.EMAIL` groups, as Apple writes them.
        let name = name.rsplit('.').next()?.to_ascii_uppercase();
        let params = parts.collect::<Vec<_>>().join(";").to_ascii_uppercase();
        Some((name, params, value.to_string()))
    };
    let wanted = email.to_ascii_lowercase();
    let mine = lines.iter().filter_map(|l| prop(l)).any(|(n, _, v)| {
        n == "EMAIL" && v.trim().trim_start_matches("mailto:").to_ascii_lowercase() == wanted
    });
    if !mine {
        return Vec::new();
    }
    let b64 = |s: &str| {
        let clean: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        base64::engine::general_purpose::STANDARD
            .decode(clean.as_bytes())
            .ok()
    };
    lines
        .iter()
        .filter_map(|l| prop(l))
        .filter(|(n, _, _)| n == "KEY")
        .filter_map(|(_, params, value)| {
            let value = value.trim();
            if let Some(rest) = value.strip_prefix("data:") {
                let (meta, data) = rest.split_once(',')?;
                return if meta.contains(";base64") {
                    b64(data)
                } else {
                    Some(
                        percent_decode(data)
                            .replace("\\n", "\n")
                            .replace("\\N", "\n")
                            .into_bytes(),
                    )
                };
            }
            if value.contains("-----BEGIN PGP PUBLIC KEY BLOCK-----") {
                return Some(value.replace("\\n", "\n").replace("\\N", "\n").into_bytes());
            }
            if params.contains("ENCODING=B") || params.contains("ENCODING=BASE64") {
                return b64(value);
            }
            None
        })
        .collect()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ── The store ──────────────────────────────────────────────────────────────

/// Where a key was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// A person on this box.
    Box,
    /// A card in the person's LosOS Contacts.
    Contacts,
    /// The `Autocrypt:` header of mail the person read.
    Autocrypt,
    /// The recipient's own Web Key Directory.
    Wkd,
    /// The keyserver.
    Keyserver,
}

/// A key the composer or the reader may use, and where it came from.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub source: Source,
    pub fingerprint: String,
    pub user_ids: Vec<String>,
    pub created: i64,
    pub public_key: String,
}

impl Found {
    fn new(source: Source, key: &PublicKey) -> Found {
        Found {
            source,
            fingerprint: key.fingerprint.clone(),
            user_ids: key.user_ids.clone(),
            created: key.created,
            public_key: key.armored(),
        }
    }
}

/// A person's own key, as their Mail page sees it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mine {
    pub fingerprint: String,
    pub user_ids: Vec<String>,
    pub created: i64,
    pub public_key: String,
    /// The private key, locked with the person's passphrase.
    pub locked_key: String,
    /// What the locked key is sealed to on the box: `tpm2`, `host`, or
    /// `null` when the box has no credential store.
    pub sealed: Option<crate::credentials::Key>,
    /// The address the box's Web Key Directory publishes it under.
    pub wkd: String,
}

/// One key learned from Autocrypt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Peer {
    key: String,
    fingerprint: String,
    /// When the message that carried it was sent, RFC 3339.
    seen: String,
}

/// How the box fetches a key from the internet: `curl`, or a fake in tests.
pub trait Fetch: Send + Sync {
    /// The body of `url` when it answered 200, at most [`MAX_KEY`] bytes.
    fn get(&self, url: &str) -> Option<Vec<u8>>;
}

/// `curl`, which is on lososd's path (`modules/daemon.nix`): https only,
/// redirects included, a short timeout, a size cap.
pub struct Curl;

impl Fetch for Curl {
    fn get(&self, url: &str) -> Option<Vec<u8>> {
        let out = Command::new("curl")
            .args([
                "--silent",
                "--fail",
                "--location",
                "--proto",
                "=https",
                "--proto-redir",
                "=https",
                "--max-redirs",
                "3",
                "--connect-timeout",
                "4",
                "--max-time",
                "8",
                "--max-filesize",
                &MAX_KEY.to_string(),
                "--",
                url,
            ])
            .output()
            .ok()?;
        (out.status.success() && out.stdout.len() <= MAX_KEY).then_some(out.stdout)
    }
}

/// People's keys: theirs, the ones they learned, and the box's WKD.
pub struct Keys {
    /// `/var/lib/losos/mail-keys`, one directory per person.
    dir: PathBuf,
    /// `/var/lib/losos-wkd`, which nginx serves.
    wkd: PathBuf,
    domain: String,
    keyserver: Option<String>,
    seal: Box<dyn crate::credentials::Seal>,
    fetch: Box<dyn Fetch>,
}

/// A person's name as it may appear in a path: the people file's names are
/// already this shape (`crate::people`), checked again here.
fn safe(name: &str) -> Result<&str, Refused> {
    if !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        && !name.starts_with('.')
    {
        Ok(name)
    } else {
        refuse(400, "no such person")
    }
}

fn write_file(path: &Path, bytes: &[u8], mode: u32) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(mode)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode))?;
    std::fs::rename(&tmp, path)
}

impl Keys {
    pub fn new(
        dir: PathBuf,
        wkd: PathBuf,
        domain: String,
        keyserver: Option<String>,
        seal: Box<dyn crate::credentials::Seal>,
        fetch: Box<dyn Fetch>,
    ) -> Keys {
        Keys {
            dir,
            wkd,
            domain,
            keyserver: keyserver.filter(|k| k.starts_with("https://")),
            seal,
            fetch,
        }
    }

    /// From `$LOSOS_MAIL_KEYS_DIR`, `$LOSOS_MAIL_WKD_DIR` and
    /// `$LOSOS_MAIL_KEYSERVER`, which `modules/mail.nix` sets.
    pub fn from_env(domain: &str) -> Keys {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        Keys::new(
            var("LOSOS_MAIL_KEYS_DIR")
                .map_or_else(|| "/var/lib/losos/mail-keys".into(), PathBuf::from),
            var("LOSOS_MAIL_WKD_DIR").map_or_else(|| "/var/lib/losos-wkd".into(), PathBuf::from),
            domain.to_string(),
            var("LOSOS_MAIL_KEYSERVER").map(|k| k.trim_end_matches('/').to_string()),
            Box::new(crate::credentials::SystemdCreds),
            Box::new(Curl),
        )
    }

    fn home(&self, person: &str) -> Result<PathBuf, Refused> {
        Ok(self.dir.join(safe(person)?))
    }

    fn credential_name(person: &str) -> String {
        format!("mail-key-{person}")
    }

    /// The URL the box's WKD answers `person`'s key at.
    pub fn wkd_url(&self, person: &str) -> String {
        format!(
            "https://{}/.well-known/openpgpkey/hu/{}?l={}",
            self.domain,
            wkd_hash(person),
            percent(person)
        )
    }

    /// `person`'s public key, if they set one.
    pub fn public(&self, person: &str) -> Option<PublicKey> {
        let bytes = std::fs::read(self.home(person).ok()?.join("public.pgp")).ok()?;
        read_public(&bytes).into_iter().next()
    }

    /// Everything about `person`'s own key, or `None` before they set one.
    pub fn mine(&self, person: &str) -> Result<Option<Mine>, Refused> {
        let Some(public) = self.public(person) else {
            return Ok(None);
        };
        let home = self.home(person)?;
        let (locked, sealed) = match std::fs::read_to_string(home.join("sealed-to")) {
            Ok(to) => {
                let key = match to.trim() {
                    "tpm2" => crate::credentials::Key::Tpm2,
                    _ => crate::credentials::Key::Host,
                };
                let bytes = self
                    .seal
                    .open(&Keys::credential_name(person), &home.join("locked.cred"))
                    .map_err(|e| broken("open the sealed private key", e))?;
                (String::from_utf8_lossy(&bytes).into_owned(), Some(key))
            }
            Err(_) => (
                std::fs::read_to_string(home.join("locked.asc"))
                    .map_err(|e| broken("read the private key", e))?,
                None,
            ),
        };
        Ok(Some(Mine {
            fingerprint: public.fingerprint.clone(),
            user_ids: public.user_ids.clone(),
            created: public.created,
            public_key: public.armored(),
            locked_key: locked,
            sealed,
            wkd: self.wkd_url(person),
        }))
    }

    /// Keep `person`'s key: the locked private key sealed with `sealed_to`
    /// (or, on a box with no credential store, as it came, already locked),
    /// the public key beside it and in the WKD.
    pub fn set(
        &self,
        person: &str,
        own: &Own,
        sealed_to: Option<crate::credentials::Key>,
    ) -> Result<(), Refused> {
        let home = self.home(person)?;
        std::fs::create_dir_all(&home).map_err(|e| broken("make the key folder", e))?;
        let _ = std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o700));
        match sealed_to {
            Some(key) => {
                self.seal
                    .seal(
                        &Keys::credential_name(person),
                        key,
                        own.locked.as_bytes(),
                        &home.join("locked.cred"),
                    )
                    .map_err(|e| broken("seal the private key", e))?;
                write_file(&home.join("sealed-to"), key.as_arg().as_bytes(), 0o600)
                    .map_err(|e| broken("keep the private key", e))?;
                let _ = std::fs::remove_file(home.join("locked.asc"));
            }
            None => {
                write_file(&home.join("locked.asc"), own.locked.as_bytes(), 0o600)
                    .map_err(|e| broken("keep the private key", e))?;
                let _ = std::fs::remove_file(home.join("locked.cred"));
                let _ = std::fs::remove_file(home.join("sealed-to"));
            }
        }
        let binary = own.public.binary();
        write_file(&home.join("public.pgp"), &binary, 0o600)
            .map_err(|e| broken("keep the public key", e))?;
        self.publish(person, Some(&binary))
    }

    /// Forget `person`'s key, and take it out of the WKD.
    pub fn remove(&self, person: &str) -> Result<(), Refused> {
        let home = self.home(person)?;
        for f in ["public.pgp", "locked.cred", "locked.asc", "sealed-to"] {
            match std::fs::remove_file(home.join(f)) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(broken("remove the key", e)),
            }
        }
        self.publish(person, None)
    }

    /// Put `key` in the WKD under `person`, or take it out: the direct
    /// method's `hu/<hash>` and the advanced method's
    /// `<domain>/hu/<hash>`, each with the `policy` file the spec asks for.
    fn publish(&self, person: &str, key: Option<&[u8]>) -> Result<(), Refused> {
        let hash = wkd_hash(person);
        let roots = [self.wkd.clone(), self.wkd.join(&self.domain)];
        for root in &roots {
            let file = root.join("hu").join(&hash);
            match key {
                Some(bytes) => {
                    write_file(&file, bytes, 0o644).map_err(|e| broken("publish the key", e))?;
                    for dir in [root.clone(), root.join("hu")] {
                        let _ =
                            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755));
                    }
                    let policy = root.join("policy");
                    if !policy.exists() {
                        write_file(&policy, b"", 0o644)
                            .map_err(|e| broken("publish the key", e))?;
                    }
                }
                None => match std::fs::remove_file(&file) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(broken("unpublish the key", e)),
                },
            }
        }
        let _ = std::fs::set_permissions(&self.wkd, std::fs::Permissions::from_mode(0o755));
        Ok(())
    }

    /// Put back in the WKD every key a person kept, after the directory
    /// was lost (it is not where the keys are kept, only where they are
    /// shown). Called once at start.
    pub fn republish(&self) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if let Ok(bytes) = std::fs::read(entry.path().join("public.pgp")) {
                if let Err(e) = self.publish(&name, Some(&bytes)) {
                    tracing::warn!(person = %name, error = %e, "could not publish a mail key");
                }
            }
        }
    }

    fn peers(&self, person: &str) -> BTreeMap<String, Peer> {
        self.home(person)
            .ok()
            .and_then(|h| std::fs::read(h.join("peers.json")).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// Learn the key in an `Autocrypt:` header of a message `person` opened,
    /// sent by `from` at `sent` (RFC 3339): kept when it is the sender's own
    /// address and the message is newer than the one the key on file came
    /// from. Mail from the person themselves teaches nothing.
    pub fn learn(&self, person: &str, own_address: &str, from: &str, sent: &str, header: &str) {
        let Some((addr, key)) = parse_autocrypt(header) else {
            return;
        };
        let from = from.to_ascii_lowercase();
        if addr != from || addr == own_address.to_ascii_lowercase() {
            return;
        }
        let Some(key) = read_public(&key).into_iter().next() else {
            return;
        };
        let mut peers = self.peers(person);
        if peers.get(&addr).is_some_and(|p| p.seen.as_str() >= sent) {
            return;
        }
        if !peers.contains_key(&addr) && peers.len() >= MAX_PEERS {
            return;
        }
        peers.insert(
            addr,
            Peer {
                key: key.armored(),
                fingerprint: key.fingerprint.clone(),
                seen: sent.to_string(),
            },
        );
        let Ok(home) = self.home(person) else { return };
        let bytes = serde_json::to_vec(&peers).unwrap_or_default();
        if let Err(e) = write_file(&home.join("peers.json"), &bytes, 0o600) {
            tracing::warn!(error = %e, "could not keep a key learned from Autocrypt");
        }
    }

    /// The keys `person` may encrypt to `email` with, or check its
    /// signatures against, best first. On this box's own domain only the
    /// box answers; otherwise Contacts and Autocrypt, and only when neither
    /// knows a key, the recipient's WKD and then the keyserver.
    /// `contacts` asks LosOS Contacts for the cards that hold the address.
    pub fn find(
        &self,
        person: &str,
        email: &str,
        contacts: impl FnOnce() -> Vec<String>,
    ) -> Result<Vec<Found>, Refused> {
        let email = email.trim().to_ascii_lowercase();
        if !crate::mail::valid_address(&email) {
            return refuse(400, format!("{email} is not an email address"));
        }
        let (local, domain) = email.rsplit_once('@').unwrap_or_default();
        if domain == self.domain {
            return Ok(match safe(local).ok().and_then(|l| self.public(l)) {
                Some(key) => vec![Found::new(Source::Box, &key)],
                None => Vec::new(),
            });
        }
        let mut found: Vec<Found> = Vec::new();
        let add = |source: Source, keys: Vec<PublicKey>, found: &mut Vec<Found>| {
            for key in keys.iter().filter(|k| k.names(&email)) {
                if !found.iter().any(|f| f.fingerprint == key.fingerprint) {
                    found.push(Found::new(source, key));
                }
            }
        };
        for card in contacts() {
            for key in vcard_keys(&card, &email) {
                add(Source::Contacts, read_public(&key), &mut found);
            }
        }
        if let Some(peer) = self.peers(person).get(&email) {
            // Autocrypt keys need not name the address in a user ID: the
            // header said whose it is.
            if let Some(key) = read_public(peer.key.as_bytes()).into_iter().next() {
                if !found.iter().any(|f| f.fingerprint == key.fingerprint) {
                    found.push(Found::new(Source::Autocrypt, &key));
                }
            }
        }
        if !found.is_empty() {
            return Ok(found);
        }
        if let Some(urls) = wkd_urls(&email) {
            for url in urls {
                if let Some(bytes) = self.fetch.get(&url) {
                    add(Source::Wkd, read_public(&bytes), &mut found);
                    if !found.is_empty() {
                        return Ok(found);
                    }
                }
            }
        }
        if let (Some(server), true) = (&self.keyserver, public_domain(domain)) {
            let url = format!("{server}/vks/v1/by-email/{}", percent(&email));
            if let Some(bytes) = self.fetch.get(&url) {
                add(Source::Keyserver, read_public(&bytes), &mut found);
            }
        }
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pgp::composed::{EncryptionCaps, KeyType, SecretKeyParamsBuilder, SubkeyParamsBuilder};
    use pgp::crypto::ecc_curve::ECCCurve;
    use std::sync::{Arc, Mutex};

    fn key(uid: &str, password: Option<&str>) -> SignedSecretKey {
        let mut rng = rand::thread_rng();
        let sub = SubkeyParamsBuilder::default()
            .key_type(KeyType::ECDH(ECCCurve::Curve25519Legacy))
            .can_encrypt(EncryptionCaps::All)
            .build()
            .unwrap();
        let mut params = SecretKeyParamsBuilder::default();
        params
            .key_type(KeyType::Ed25519Legacy)
            .can_certify(true)
            .can_sign(true)
            .primary_user_id(uid.into())
            .subkeys(vec![sub]);
        let mut key = params.build().unwrap().generate(&mut rng).unwrap();
        if let Some(p) = password {
            let pw = pgp::types::Password::from(p);
            key.primary_key.set_password(&mut rng, &pw).unwrap();
            for sub in &mut key.secret_subkeys {
                sub.key.set_password(&mut rng, &pw).unwrap();
            }
        }
        key
    }

    fn armored_public(k: &SignedSecretKey) -> String {
        k.to_public_key()
            .to_armored_string(ArmorOptions::default())
            .unwrap()
    }

    #[test]
    fn wkd_hashes_the_local_part_as_the_draft_does() {
        // The example in draft-koch-openpgp-webkey-service, section 3.1.
        assert_eq!(wkd_hash("Joe.Doe"), "iy9q119eutrkn8s1mk4r39qejnbu3n5q");
        assert_eq!(wkd_hash("joe.doe"), "iy9q119eutrkn8s1mk4r39qejnbu3n5q");
    }

    #[test]
    fn only_names_on_the_internet_are_asked() {
        assert!(public_domain("example.org"));
        assert!(public_domain("mail.posteo.de"));
        for bad in [
            "localhost",
            "box.local",
            "mattbox.home.arpa",
            "10.0.0.1",
            "a..b",
            "-x.org",
            "evil.org/x",
            "noDot",
        ] {
            assert!(!public_domain(bad), "{bad}");
        }
        let urls = wkd_urls("Joe.Doe@Example.org").unwrap();
        assert_eq!(
            urls[0],
            "https://openpgpkey.example.org/.well-known/openpgpkey/example.org/hu/iy9q119eutrkn8s1mk4r39qejnbu3n5q?l=Joe.Doe"
        );
        assert_eq!(
            urls[1],
            "https://example.org/.well-known/openpgpkey/hu/iy9q119eutrkn8s1mk4r39qejnbu3n5q?l=Joe.Doe"
        );
        assert!(wkd_urls("eva@mattbox.local").is_none());
    }

    #[test]
    fn a_locked_key_for_ones_own_address_is_taken() {
        let k = key("Eva <eva@mattbox.home.arpa>", Some("long passphrase"));
        let locked = k.to_armored_string(ArmorOptions::default()).unwrap();
        let own = check_own(&armored_public(&k), &locked, "eva@mattbox.home.arpa").unwrap();
        assert_eq!(
            own.public.emails(),
            vec!["eva@mattbox.home.arpa".to_string()]
        );
        // The binary is the public key alone.
        let again = read_public(&own.public.binary());
        assert_eq!(again.len(), 1);
        assert_eq!(again[0].fingerprint, own.public.fingerprint);
        assert!(SignedSecretKey::from_bytes(&own.public.binary()[..]).is_err());
    }

    #[test]
    fn an_unlocked_or_foreign_or_mismatched_key_is_refused() {
        let open = key("Eva <eva@mattbox.home.arpa>", None);
        let r = check_own(
            &armored_public(&open),
            &open.to_armored_string(ArmorOptions::default()).unwrap(),
            "eva@mattbox.home.arpa",
        );
        assert!(r.unwrap_err().message.contains("passphrase"));

        let other = key("Adam <adam@mattbox.home.arpa>", Some("pw pw pw pw"));
        let r = check_own(
            &armored_public(&other),
            &other.to_armored_string(ArmorOptions::default()).unwrap(),
            "eva@mattbox.home.arpa",
        );
        assert!(r.unwrap_err().message.contains("not for eva@"));

        let eva = key("Eva <eva@mattbox.home.arpa>", Some("pw pw pw pw"));
        let r = check_own(
            &armored_public(&eva),
            &other.to_armored_string(ArmorOptions::default()).unwrap(),
            "eva@mattbox.home.arpa",
        );
        assert!(r.unwrap_err().message.contains("not the same key"));

        // A private key handed in as the public half is not a public key.
        let r = check_own(
            &eva.to_armored_string(ArmorOptions::default()).unwrap(),
            &eva.to_armored_string(ArmorOptions::default()).unwrap(),
            "eva@mattbox.home.arpa",
        );
        assert!(r.is_err());
    }

    #[test]
    fn autocrypt_headers_go_out_folded_and_come_back_whole() {
        let k = key("Eva <eva@mattbox.home.arpa>", None);
        let bin = k.to_public_key().to_bytes().unwrap();
        let header = autocrypt_header("eva@mattbox.home.arpa", &bin);
        assert!(header.lines().all(|l| l.len() <= 78), "{header}");
        let (addr, back) = parse_autocrypt(&header).unwrap();
        assert_eq!(addr, "eva@mattbox.home.arpa");
        assert_eq!(back, bin);
        // An attribute Level 1 does not know makes the header unusable,
        // unless it starts with an underscore.
        assert!(parse_autocrypt("addr=a@b.org; keydata=AAAA; future=1").is_none());
        assert!(parse_autocrypt("addr=a@b.org; _x=1; keydata=AAAA").is_some());
        assert!(parse_autocrypt("keydata=AAAA").is_none());
    }

    #[test]
    fn a_vcard_hands_over_its_key_only_for_its_own_address() {
        let k = key("Friend <friend@example.org>", None);
        let b64 =
            base64::engine::general_purpose::STANDARD.encode(k.to_public_key().to_bytes().unwrap());
        let v4 = format!(
            "BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Friend\r\nEMAIL;TYPE=work:friend@example.org\r\nKEY:data:application/pgp-keys;base64,{}\r\n {}\r\nEND:VCARD\r\n",
            &b64[..40],
            &b64[40..]
        );
        let keys = vcard_keys(&v4, "Friend@Example.org");
        assert_eq!(keys.len(), 1);
        assert_eq!(read_public(&keys[0]).len(), 1);
        assert!(vcard_keys(&v4, "someone@example.org").is_empty());
        let v3 = format!(
            "BEGIN:VCARD\nVERSION:3.0\nitem1.EMAIL:friend@example.org\nKEY;TYPE=PGP;ENCODING=b:{b64}\nEND:VCARD\n"
        );
        assert_eq!(vcard_keys(&v3, "friend@example.org").len(), 1);
        let url = "BEGIN:VCARD\nEMAIL:friend@example.org\nKEY;MEDIATYPE=application/pgp-keys:https://example.org/k.asc\nEND:VCARD\n";
        assert!(vcard_keys(url, "friend@example.org").is_empty());
    }

    /// Seals by keeping the bytes, so the store's bookkeeping runs without
    /// systemd-creds.
    #[derive(Default)]
    struct Jar(Mutex<BTreeMap<PathBuf, Vec<u8>>>);

    impl crate::credentials::Seal for Arc<Jar> {
        fn seal(
            &self,
            _name: &str,
            _key: crate::credentials::Key,
            value: &[u8],
            out: &Path,
        ) -> anyhow::Result<()> {
            std::fs::write(out, b"sealed")?;
            self.0
                .lock()
                .unwrap()
                .insert(out.to_path_buf(), value.to_vec());
            Ok(())
        }
        fn open(&self, _name: &str, path: &Path) -> anyhow::Result<Vec<u8>> {
            self.0
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("not sealed"))
        }
        fn restart(&self, _units: &[String]) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct Web(Mutex<BTreeMap<String, Vec<u8>>>, Mutex<Vec<String>>);

    impl Fetch for Arc<Web> {
        fn get(&self, url: &str) -> Option<Vec<u8>> {
            self.1.lock().unwrap().push(url.to_string());
            self.0.lock().unwrap().get(url).cloned()
        }
    }

    fn store(name: &str) -> (Keys, Arc<Jar>, Arc<Web>, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("losos-mail-pgp-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let jar = Arc::new(Jar::default());
        let web = Arc::new(Web::default());
        let keys = Keys::new(
            dir.join("keys"),
            dir.join("wkd"),
            "mattbox.home.arpa".into(),
            Some("https://keys.example.net".into()),
            Box::new(jar.clone()),
            Box::new(web.clone()),
        );
        (keys, jar, web, dir)
    }

    #[test]
    fn a_kept_key_is_sealed_published_and_taken_back() {
        let (keys, jar, _, dir) = store("keep");
        let k = key("Eva <eva@mattbox.home.arpa>", Some("long passphrase"));
        let locked = k.to_armored_string(ArmorOptions::default()).unwrap();
        let own = check_own(&armored_public(&k), &locked, "eva@mattbox.home.arpa").unwrap();
        keys.set("eva", &own, Some(crate::credentials::Key::Tpm2))
            .unwrap();

        // Sealed, not left as it came.
        let on_disk = std::fs::read(dir.join("keys/eva/locked.cred")).unwrap();
        assert_eq!(on_disk, b"sealed");
        assert!(!dir.join("keys/eva/locked.asc").exists());
        assert_eq!(jar.0.lock().unwrap().len(), 1);
        let mine = keys.mine("eva").unwrap().unwrap();
        assert_eq!(mine.locked_key, locked);
        assert_eq!(mine.sealed, Some(crate::credentials::Key::Tpm2));
        assert_eq!(mine.fingerprint, own.public.fingerprint);

        // In both WKD layouts, readable by nginx, with a policy file.
        let hash = wkd_hash("eva");
        for root in ["wkd", "wkd/mattbox.home.arpa"] {
            let file = dir.join(root).join("hu").join(&hash);
            assert_eq!(std::fs::read(&file).unwrap(), own.public.binary());
            let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o644);
            assert!(dir.join(root).join("policy").exists());
        }

        // Another person on the box finds it, and only the box is asked.
        let found = keys
            .find("adam", "EVA@mattbox.home.arpa", Vec::new)
            .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].source, Source::Box);

        keys.remove("eva").unwrap();
        assert!(keys.mine("eva").unwrap().is_none());
        assert!(!dir.join("wkd/hu").join(&hash).exists());
        assert!(keys
            .find("adam", "eva@mattbox.home.arpa", Vec::new)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn outside_keys_come_from_contacts_autocrypt_wkd_then_the_keyserver() {
        let (keys, _, web, _) = store("find");
        let friend = key("Friend <friend@example.org>", None);
        let bin = friend.to_public_key().to_bytes().unwrap();
        let fp = format!("{:X}", friend.fingerprint());

        // Nothing anywhere: WKD (both methods), then the keyserver.
        assert!(keys
            .find("eva", "friend@example.org", Vec::new)
            .unwrap()
            .is_empty());
        let asked = web.1.lock().unwrap().clone();
        assert_eq!(asked.len(), 3, "{asked:?}");
        assert!(asked[0].starts_with("https://openpgpkey.example.org/"));
        assert!(asked[1].starts_with("https://example.org/"));
        assert_eq!(
            asked[2],
            "https://keys.example.net/vks/v1/by-email/friend%40example.org"
        );

        // The direct method answers.
        web.0.lock().unwrap().insert(asked[1].clone(), bin.clone());
        let found = keys.find("eva", "friend@example.org", Vec::new).unwrap();
        assert_eq!((found.len(), found[0].source), (1, Source::Wkd));
        assert_eq!(found[0].fingerprint, fp);

        // A key WKD serves for some other address is not taken.
        let stranger = key("Stranger <x@example.org>", None);
        web.0.lock().unwrap().insert(
            asked[1].clone(),
            stranger.to_public_key().to_bytes().unwrap(),
        );
        assert!(keys
            .find("eva", "friend@example.org", Vec::new)
            .unwrap()
            .is_empty());

        // Autocrypt: learned from the sender's own mail, newer wins, and the
        // web is not asked once it is known.
        let header = autocrypt_header("friend@example.org", &bin);
        keys.learn(
            "eva",
            "eva@mattbox.home.arpa",
            "Friend@example.org",
            "2026-10-22T08:00:00Z",
            &header,
        );
        web.1.lock().unwrap().clear();
        let found = keys.find("eva", "friend@example.org", Vec::new).unwrap();
        assert_eq!((found.len(), found[0].source), (1, Source::Autocrypt));
        assert!(web.1.lock().unwrap().is_empty());
        // An older message does not replace it; a header for someone else
        // than the sender is ignored.
        let stranger_header = autocrypt_header(
            "friend@example.org",
            &stranger.to_public_key().to_bytes().unwrap(),
        );
        keys.learn(
            "eva",
            "eva@mattbox.home.arpa",
            "friend@example.org",
            "2026-10-21T08:00:00Z",
            &stranger_header,
        );
        keys.learn(
            "eva",
            "eva@mattbox.home.arpa",
            "mallory@example.org",
            "2026-10-23T08:00:00Z",
            &stranger_header,
        );
        let found = keys.find("eva", "friend@example.org", Vec::new).unwrap();
        assert_eq!(found[0].fingerprint, fp);
        // Adam learned nothing.
        assert!(keys.peers("adam").is_empty());

        // A contact card's key comes first.
        let card_key = key("Friend <friend@example.org>", None);
        let card = format!(
            "BEGIN:VCARD\nEMAIL:friend@example.org\nKEY;ENCODING=b:{}\nEND:VCARD\n",
            base64::engine::general_purpose::STANDARD
                .encode(card_key.to_public_key().to_bytes().unwrap())
        );
        let found = keys
            .find("eva", "friend@example.org", || vec![card])
            .unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].source, Source::Contacts);
        assert_eq!(found[1].source, Source::Autocrypt);
    }
}
