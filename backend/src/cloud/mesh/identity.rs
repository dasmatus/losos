//! This box's key for the mesh, and the keys of the boxes it has met.
//!
//! Every call between boxes is signed, so who sent it does not rest on the
//! TLS certificate: boxes on a LAN answer with certificates they made
//! themselves, which nothing can check. A box's Ed25519 key is made on first
//! start and never leaves it. Another box's key is learned the first time
//! this box reaches it by its name, and kept (`pins`), the way SSH keeps
//! `known_hosts`. A box that later answers to that name with another key is
//! refused, and the refusal is shown, rather than the new key being taken.

use std::io;
use std::path::Path;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use ring::rand::{SecureRandom, SystemRandom};
use ring::signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519};

/// Every signature covers this first, so a signature made for the mesh
/// cannot be replayed as anything else.
const CONTEXT: &[u8] = b"losos-mesh-v1";

/// This box's signing key.
pub struct Identity {
    key: Ed25519KeyPair,
}

impl Identity {
    /// Load the key from `path`, making it on first start.
    pub fn load_or_create(path: &Path) -> io::Result<Self> {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let doc = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
                    .map_err(|_| io::Error::other("could not make a key"))?;
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                let tmp = path.with_extension("tmp");
                write_private(&tmp, doc.as_ref())?;
                std::fs::rename(&tmp, path)?;
                doc.as_ref().to_vec()
            }
            Err(e) => return Err(e),
        };
        let key = Ed25519KeyPair::from_pkcs8(&bytes)
            .map_err(|_| io::Error::other("the mesh key does not parse"))?;
        Ok(Identity { key })
    }

    /// A key that lives only in memory, for tests.
    pub fn ephemeral() -> Self {
        let doc = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).expect("rng");
        Identity {
            key: Ed25519KeyPair::from_pkcs8(doc.as_ref()).expect("fresh key parses"),
        }
    }

    /// The public key, base64.
    pub fn public(&self) -> String {
        B64.encode(self.key.public_key().as_ref())
    }

    /// Sign `parts` (see [`message`]), base64.
    pub fn sign(&self, parts: &[&[u8]]) -> String {
        B64.encode(self.key.sign(&message(parts)).as_ref())
    }
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    std::fs::write(path, bytes)
}

/// What is signed: the context, then each part with its length in front,
/// so no two different lists of parts sign the same bytes.
pub fn message(parts: &[&[u8]]) -> Vec<u8> {
    let mut out = CONTEXT.to_vec();
    for p in parts {
        out.extend_from_slice(&(p.len() as u32).to_be_bytes());
        out.extend_from_slice(p);
    }
    out
}

/// Whether `sig` (base64) is `key`'s (base64) signature over `parts`.
pub fn verify(key: &str, parts: &[&[u8]], sig: &str) -> bool {
    let (Ok(key), Ok(sig)) = (B64.decode(key), B64.decode(sig)) else {
        return false;
    };
    if key.len() != 32 {
        return false;
    }
    UnparsedPublicKey::new(&ED25519, key)
        .verify(&message(parts), &sig)
        .is_ok()
}

/// Random bytes, base64, for nonces.
pub fn nonce() -> String {
    let mut b = [0u8; 32];
    SystemRandom::new().fill(&mut b).expect("rng");
    B64.encode(b)
}

/// A random id of letters and digits: shares are named by these, and they
/// go into file names (`collab::hub`) and URLs.
pub fn random_id() -> String {
    const ALPHABET: &[u8] = b"abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut b = [0u8; 22];
    SystemRandom::new().fill(&mut b).expect("rng");
    b.iter()
        .map(|x| ALPHABET[*x as usize % ALPHABET.len()] as char)
        .collect()
}

/// A box name as other boxes reach it: a DNS name, lower case, no port, no
/// path. `bob@box.example` addresses use the part after the `@`.
pub fn valid_box_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 253
        && !name.starts_with('.')
        && !name.ends_with('.')
        && !name.contains("..")
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
}

/// Split `bob@box.example` into its login and box name.
pub fn split_address(address: &str) -> Option<(&str, &str)> {
    let (login, host) = address.rsplit_once('@')?;
    let ok_login = !login.is_empty()
        && login.len() <= 64
        && login
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b));
    (ok_login && valid_box_name(host)).then_some((login, host))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signature_checks_out_only_for_its_own_parts() {
        let id = Identity::ephemeral();
        let sig = id.sign(&[b"a", b"bc"]);
        assert!(verify(&id.public(), &[b"a", b"bc"], &sig));
        // Same bytes, cut differently.
        assert!(!verify(&id.public(), &[b"ab", b"c"], &sig));
        assert!(!verify(
            &Identity::ephemeral().public(),
            &[b"a", b"bc"],
            &sig
        ));
        assert!(!verify("not base64!", &[b"a"], &sig));
    }

    #[test]
    fn the_key_survives_a_restart() {
        let dir = std::env::temp_dir().join(format!("losos-mesh-key-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("identity.pk8");
        let first = Identity::load_or_create(&path).unwrap().public();
        let second = Identity::load_or_create(&path).unwrap().public();
        assert_eq!(first, second);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn addresses() {
        assert_eq!(
            split_address("bob@boxb.example"),
            Some(("bob", "boxb.example"))
        );
        assert_eq!(split_address("a.b@x@boxb"), None);
        assert_eq!(split_address("bob@BoxB"), None);
        assert_eq!(split_address("bob@boxb:443"), None);
        assert_eq!(split_address("bob@boxb/path"), None);
        assert_eq!(split_address("@boxb"), None);
        assert_eq!(split_address("bob@"), None);
        assert!(random_id().bytes().all(|b| b.is_ascii_alphanumeric()));
        assert_ne!(random_id(), random_id());
    }
}
