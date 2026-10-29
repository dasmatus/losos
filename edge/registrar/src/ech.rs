//! The edge's Encrypted Client Hello key (RFC 9849), kept as RFC 9934 PEM.
//!
//! With ECH a browser encrypts the name it asks for inside the TLS
//! handshake, so the network between it and the edge sees only the edge's
//! *public name*. The browser learns the key from DNS: the `ech` parameter
//! of the name's HTTPS record (RFC 9848), which [`crate::zone`] writes for
//! every name in the edge's zone from the key here. Whatever terminates TLS
//! on the edge holds the private half and decrypts.
//!
//! The file is the one place the key lives, outside any one program's
//! config: `--ech-key-file`, made by the registrar on its first start when
//! missing, and remade when the public name it was made for changes. It is
//! read by the TLS front and, because the ECHConfigList is public, by
//! anything that publishes it.
//!
//! One key: X25519 for the KEM, HKDF-SHA256, and AES-128-GCM or
//! ChaCha20-Poly1305, the suites every ECH client offers.

use std::path::Path;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use curve25519_dalek::montgomery::MontgomeryPoint;
use miette::{miette, Context, IntoDiagnostic, Result};

/// RFC 9849's ECHConfig version.
const VERSION: u16 = 0xfe0d;
/// DHKEM(X25519, HKDF-SHA256).
const KEM_X25519: u16 = 0x0020;
/// HKDF-SHA256 with AES-128-GCM, then with ChaCha20-Poly1305.
const SUITES: [(u16, u16); 2] = [(0x0001, 0x0001), (0x0001, 0x0003)];
/// PKCS #8 for an X25519 private key, up to the 32 key bytes (RFC 8410).
const PKCS8_X25519: [u8; 16] = [
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x6e, 0x04, 0x22, 0x04, 0x20,
];

/// An ECH key: the private half and the ECHConfigList it belongs to.
#[derive(Clone, PartialEq, Eq)]
pub struct EchKey {
    private: [u8; 32],
    /// The ECHConfigList, its two-byte length included: what goes in DNS.
    config_list: Vec<u8>,
}

impl std::fmt::Debug for EchKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EchKey")
            .field("public_name", &self.public_name())
            .finish_non_exhaustive()
    }
}

impl EchKey {
    /// A new key for `public_name`, the name the outer handshake shows.
    ///
    /// # Errors
    /// The name is not a DNS name, or the system's random source failed.
    pub fn generate(public_name: &str) -> Result<Self> {
        use ring::rand::SecureRandom;
        let mut private = [0u8; 32];
        let mut id = [0u8; 1];
        let rng = ring::rand::SystemRandom::new();
        rng.fill(&mut private)
            .and_then(|()| rng.fill(&mut id))
            .map_err(|_| miette!("the system random source failed"))?;
        Self::from_parts(private, id[0], public_name)
    }

    fn from_parts(private: [u8; 32], config_id: u8, public_name: &str) -> Result<Self> {
        let name = public_name
            .trim()
            .trim_end_matches('.')
            .to_ascii_lowercase();
        if !dns_name(&name) {
            return Err(miette!("{public_name:?} is not a DNS name for ECH"));
        }
        let public = MontgomeryPoint::mul_base_clamped(private).to_bytes();
        let mut contents = vec![config_id];
        contents.extend(KEM_X25519.to_be_bytes());
        contents.extend(32u16.to_be_bytes());
        contents.extend(public);
        contents.extend((SUITES.len() as u16 * 4).to_be_bytes());
        for (kdf, aead) in SUITES {
            contents.extend(kdf.to_be_bytes());
            contents.extend(aead.to_be_bytes());
        }
        // maximum_name_length: 0 leaves the padding to the client.
        contents.push(0);
        contents.push(name.len() as u8);
        contents.extend(name.as_bytes());
        // No extensions.
        contents.extend(0u16.to_be_bytes());

        let mut config = VERSION.to_be_bytes().to_vec();
        config.extend((contents.len() as u16).to_be_bytes());
        config.extend(contents);
        let mut config_list = (config.len() as u16).to_be_bytes().to_vec();
        config_list.extend(config);
        Ok(EchKey {
            private,
            config_list,
        })
    }

    /// The ECHConfigList as base64: the HTTPS record's `ech` value.
    #[must_use]
    pub fn config_list_base64(&self) -> String {
        STANDARD.encode(&self.config_list)
    }

    /// The public name of the first config.
    #[must_use]
    pub fn public_name(&self) -> Option<String> {
        first_public_name(&self.config_list)
    }

    /// The RFC 9934 file: the private key, then the ECHConfigList.
    #[must_use]
    pub fn to_pem(&self) -> String {
        let mut pkcs8 = PKCS8_X25519.to_vec();
        pkcs8.extend(self.private);
        format!(
            "{}{}",
            pem_block("PRIVATE KEY", &pkcs8),
            pem_block("ECHCONFIG", &self.config_list)
        )
    }

    /// Read an RFC 9934 file this module wrote.
    ///
    /// # Errors
    /// No private key or ECHConfigList, or ones that do not belong together.
    pub fn from_pem(text: &str) -> Result<Self> {
        let pkcs8 = pem_body(text, "PRIVATE KEY").ok_or_else(|| miette!("no PRIVATE KEY block"))?;
        let config_list =
            pem_body(text, "ECHCONFIG").ok_or_else(|| miette!("no ECHCONFIG block"))?;
        let key = pkcs8
            .strip_prefix(&PKCS8_X25519[..])
            .filter(|k| k.len() == 32)
            .ok_or_else(|| miette!("the PRIVATE KEY is not an X25519 key"))?;
        let mut private = [0u8; 32];
        private.copy_from_slice(key);
        let key = EchKey {
            private,
            config_list,
        };
        let public = MontgomeryPoint::mul_base_clamped(private).to_bytes();
        if first_public_key(&key.config_list) != Some(public) {
            return Err(miette!("the ECHCONFIG does not belong to the PRIVATE KEY"));
        }
        Ok(key)
    }
}

/// The key in `path` for `public_name`, made (and written 0640, for the TLS
/// front's group) when the file is missing or was made for another name.
///
/// # Errors
/// The file cannot be read or written, or holds something that is not a key.
pub fn load_or_make(path: &Path, public_name: &str) -> Result<EchKey> {
    let want = public_name
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let key =
                EchKey::from_pem(&text).wrap_err_with(|| format!("read {}", path.display()))?;
            if key.public_name().as_deref() == Some(want.as_str()) {
                return Ok(key);
            }
            tracing::info!(
                "the ECH key in {} names another public name; making a new one for {want}",
                path.display()
            );
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(e)
                .into_diagnostic()
                .wrap_err_with(|| format!("read {}", path.display()))
        }
    }
    let key = EchKey::generate(&want)?;
    write_key(path, &key)?;
    tracing::info!("made an ECH key for {want} at {}", path.display());
    Ok(key)
}

fn write_key(path: &Path, key: &EchKey) -> Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .into_diagnostic()
            .wrap_err_with(|| format!("create {}", dir.display()))?;
    }
    let tmp = path.with_extension("new");
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o640)
        .open(&tmp)
        .into_diagnostic()
        .wrap_err_with(|| format!("write {}", tmp.display()))?;
    f.write_all(key.to_pem().as_bytes()).into_diagnostic()?;
    f.sync_all().into_diagnostic()?;
    std::fs::rename(&tmp, path)
        .into_diagnostic()
        .wrap_err_with(|| format!("write {}", path.display()))
}

/// A name a public_name may be: LDH labels, at least two of them.
fn dns_name(name: &str) -> bool {
    name.len() <= 253
        && name.contains('.')
        && name.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && !l.starts_with('-')
                && !l.ends_with('-')
                && l.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

/// The first ECHConfig's contents, after the list and config headers.
fn first_contents(list: &[u8]) -> Option<&[u8]> {
    let len = usize::from(u16::from_be_bytes([*list.first()?, *list.get(1)?]));
    let list = list.get(2..2 + len)?;
    if u16::from_be_bytes([*list.first()?, *list.get(1)?]) != VERSION {
        return None;
    }
    let len = usize::from(u16::from_be_bytes([*list.get(2)?, *list.get(3)?]));
    list.get(4..4 + len)
}

fn first_public_key(list: &[u8]) -> Option<[u8; 32]> {
    let c = first_contents(list)?;
    // config_id(1) kem_id(2) public_key length(2)
    if u16::from_be_bytes([*c.get(3)?, *c.get(4)?]) != 32 {
        return None;
    }
    c.get(5..37)?.try_into().ok()
}

fn first_public_name(list: &[u8]) -> Option<String> {
    let c = first_contents(list)?;
    let key_len = usize::from(u16::from_be_bytes([*c.get(3)?, *c.get(4)?]));
    let at = 5 + key_len;
    let suites_len = usize::from(u16::from_be_bytes([*c.get(at)?, *c.get(at + 1)?]));
    let at = at + 2 + suites_len + 1; // maximum_name_length
    let name_len = usize::from(*c.get(at)?);
    String::from_utf8(c.get(at + 1..at + 1 + name_len)?.to_vec()).ok()
}

fn pem_block(label: &str, der: &[u8]) -> String {
    let b64 = STANDARD.encode(der);
    let mut out = format!("-----BEGIN {label}-----\n");
    for chunk in b64.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(chunk).unwrap_or_default());
        out.push('\n');
    }
    out.push_str(&format!("-----END {label}-----\n"));
    out
}

fn pem_body(text: &str, label: &str) -> Option<Vec<u8>> {
    let begin = format!("-----BEGIN {label}-----");
    let end = format!("-----END {label}-----");
    let start = text.find(&begin)? + begin.len();
    let stop = start + text[start..].find(&end)?;
    let b64: String = text[start..stop]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    STANDARD.decode(b64).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 9934's Figure 1, verbatim.
    const RFC_EXAMPLE: &str = "-----BEGIN PRIVATE KEY-----\n\
MC4CAQAwBQYDK2VuBCIEICjd4yGRdsoP9gU7YT7My8DHx1Tjme8GYDXrOMCi8v1V\n\
-----END PRIVATE KEY-----\n\
-----BEGIN ECHCONFIG-----\n\
AD7+DQA65wAgACA8wVN2BtscOl3vQheUzHeIkVmKIiydUhDCliA4iyQRCwAEAAEA\n\
AQALZXhhbXBsZS5jb20AAA==\n\
-----END ECHCONFIG-----\n";

    #[test]
    fn the_rfc_example_reads_and_its_key_matches_its_config() {
        let key = EchKey::from_pem(RFC_EXAMPLE).unwrap();
        assert_eq!(key.public_name().as_deref(), Some("example.com"));
        // Rebuilt from its private key, id and name with one suite, the
        // example's own ECHConfigList comes out byte for byte.
        let private = key.private;
        let config_id = first_contents(&key.config_list).unwrap()[0];
        let mut rebuilt = EchKey::from_parts(private, config_id, "example.com").unwrap();
        // The example offers only AES-128-GCM.
        let ours = rebuilt.config_list_base64();
        assert_ne!(ours, key.config_list_base64());
        assert_eq!(
            first_public_key(&rebuilt.config_list),
            first_public_key(&key.config_list)
        );
        rebuilt.config_list = key.config_list.clone();
        assert_eq!(EchKey::from_pem(&rebuilt.to_pem()).unwrap(), key);
    }

    #[test]
    fn a_new_key_round_trips_through_its_pem() {
        let key = EchKey::generate("Edge.Example.ORG.").unwrap();
        assert_eq!(key.public_name().as_deref(), Some("edge.example.org"));
        let pem = key.to_pem();
        assert!(pem.starts_with("-----BEGIN PRIVATE KEY-----\n"));
        assert!(pem.contains("-----BEGIN ECHCONFIG-----\n"));
        assert_eq!(EchKey::from_pem(&pem).unwrap(), key);
        // Version, X25519, two suites.
        let list = &key.config_list;
        assert_eq!(&list[2..4], &[0xfe, 0x0d]);
        let c = first_contents(list).unwrap();
        assert_eq!(&c[1..3], &[0x00, 0x20]);
        assert_eq!(&c[37..39], &[0x00, 0x08]);
    }

    #[test]
    fn a_config_from_another_key_is_refused() {
        let a = EchKey::generate("a.example.org").unwrap();
        let b = EchKey::generate("a.example.org").unwrap();
        let mixed = format!(
            "{}{}",
            pem_block("PRIVATE KEY", &[&PKCS8_X25519[..], &a.private[..]].concat()),
            pem_block("ECHCONFIG", &b.config_list)
        );
        assert!(EchKey::from_pem(&mixed).is_err());
        assert!(EchKey::from_pem("nothing here").is_err());
    }

    #[test]
    fn a_public_name_must_be_a_dns_name() {
        for bad in [
            "",
            "localhost",
            "a..b",
            "-a.example",
            "a b.example",
            "ä.example",
        ] {
            assert!(EchKey::generate(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_file_is_made_once_and_remade_for_another_name() {
        let dir = std::env::temp_dir().join(format!("losos-ech-{}", std::process::id()));
        let path = dir.join("ech.pem");
        let _ = std::fs::remove_dir_all(&dir);
        let first = load_or_make(&path, "edge.example.org").unwrap();
        let again = load_or_make(&path, "edge.example.org").unwrap();
        assert_eq!(first, again);
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        let other = load_or_make(&path, "other.example.org").unwrap();
        assert_ne!(other, first);
        assert_eq!(other.public_name().as_deref(), Some("other.example.org"));
        std::fs::write(&path, "garbage").unwrap();
        assert!(load_or_make(&path, "edge.example.org").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
