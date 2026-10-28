//! The TPM 2.0 structures a box reads from a gateway, and the one it writes.
//!
//! Everything here is the TPM's own wire format (big-endian, sized
//! buffers), parsed by hand: four structures and one command's worth of
//! cryptography do not justify a TSS. The box never talks to a TPM. It reads
//! what the gateway's TPM produced ([`Public`], [`Attest`], [`Signature`]),
//! and makes the one input the gateway's TPM has to decrypt
//! ([`make_credential`]), which is TPM2_MakeCredential done in software, as
//! `tpm2_makecredential --tcti none` does it.

use cipher::{AsyncStreamCipher, KeyIvInit};
use hmac::{Hmac, Mac};
use rand::{CryptoRng, RngCore};
use sha2::{Digest, Sha256};

pub const ALG_RSA: u16 = 0x0001;
pub const ALG_AES: u16 = 0x0006;
pub const ALG_SHA256: u16 = 0x000b;
pub const ALG_NULL: u16 = 0x0010;
pub const ALG_ECDSA: u16 = 0x0018;
pub const ALG_ECC: u16 = 0x0023;
pub const ALG_CFB: u16 = 0x0043;
pub const ECC_NIST_P256: u16 = 0x0003;

/// `TPM_GENERATED_VALUE`: the first word of anything the TPM itself made.
/// A restricted signing key refuses to sign data that starts with it, which
/// is why a quote signed by one can only have come from TPM2_Quote.
pub const GENERATED: u32 = 0xff54_4347;
pub const ST_ATTEST_QUOTE: u16 = 0x8018;

pub const FIXED_TPM: u32 = 1 << 1;
pub const FIXED_PARENT: u32 = 1 << 4;
pub const SENSITIVE_DATA_ORIGIN: u32 = 1 << 5;
pub const RESTRICTED: u32 = 1 << 16;
pub const DECRYPT: u32 = 1 << 17;
pub const SIGN: u32 = 1 << 18;

/// What an attestation key must be: born in this TPM, never leaving it, and
/// restricted to signing what the TPM itself produced.
pub const AK_ATTRIBUTES: u32 = FIXED_TPM | FIXED_PARENT | SENSITIVE_DATA_ORIGIN | RESTRICTED | SIGN;

/// The magic and version `tpm2_activatecredential -i` expects in front of
/// the two blobs.
const CREDENTIAL_FILE_MAGIC: u32 = 0xbadc_c0de;
const CREDENTIAL_FILE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TpmError {
    #[error("not a TPM structure: {0}")]
    Malformed(&'static str),
    #[error("unsupported: {0}")]
    Unsupported(&'static str),
    #[error("{0}")]
    Crypto(&'static str),
}

type R<T> = Result<T, TpmError>;

/// A cursor over big-endian TPM bytes.
pub(crate) struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(b: &'a [u8]) -> Self {
        Reader { b, at: 0 }
    }

    pub(crate) fn take(&mut self, n: usize) -> R<&'a [u8]> {
        let end = self
            .at
            .checked_add(n)
            .filter(|e| *e <= self.b.len())
            .ok_or(TpmError::Malformed("truncated"))?;
        let out = &self.b[self.at..end];
        self.at = end;
        Ok(out)
    }

    pub(crate) fn u8(&mut self) -> R<u8> {
        Ok(self.take(1)?[0])
    }

    pub(crate) fn u16(&mut self) -> R<u16> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    pub(crate) fn u32(&mut self) -> R<u32> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub(crate) fn u64(&mut self) -> R<u64> {
        let b = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Ok(u64::from_be_bytes(a))
    }

    /// A `TPM2B_*`: a u16 size, then that many bytes.
    pub(crate) fn sized(&mut self) -> R<&'a [u8]> {
        let n = self.u16()?;
        self.take(usize::from(n))
    }

    pub(crate) fn done(&self) -> R<()> {
        if self.at == self.b.len() {
            Ok(())
        } else {
            Err(TpmError::Malformed("trailing bytes"))
        }
    }
}

fn sized(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 2);
    out.extend_from_slice(&u16::try_from(bytes.len()).unwrap_or(u16::MAX).to_be_bytes());
    out.extend_from_slice(bytes);
    out
}

/// The key half of a public area.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Rsa {
        bits: u16,
        exponent: u32,
        modulus: Vec<u8>,
        /// The symmetric algorithm of a storage key (an EK): `(AES, bits)`.
        symmetric: Option<(u16, u16)>,
    },
    Ecc {
        curve: u16,
        /// The signing scheme and its hash, `ALG_NULL` when the key may use any.
        scheme: (u16, u16),
        x: Vec<u8>,
        y: Vec<u8>,
    },
}

/// A `TPMT_PUBLIC`: what a key is, and the bytes its name is made from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Public {
    pub name_alg: u16,
    pub attributes: u32,
    pub key: Key,
    area: Vec<u8>,
}

impl Public {
    /// Parse a `TPM2B_PUBLIC`, as `tpm2_createek -u` and `tpm2_createak -u`
    /// write it.
    pub fn from_tpm2b(bytes: &[u8]) -> R<Public> {
        let mut r = Reader::new(bytes);
        let area = r.sized()?;
        r.done()?;
        Public::from_area(area)
    }

    fn from_area(area: &[u8]) -> R<Public> {
        let mut r = Reader::new(area);
        let typ = r.u16()?;
        let name_alg = r.u16()?;
        let attributes = r.u32()?;
        let _auth_policy = r.sized()?;
        let symmetric = {
            let alg = r.u16()?;
            if alg == ALG_NULL {
                None
            } else {
                let bits = r.u16()?;
                let _mode = r.u16()?;
                Some((alg, bits))
            }
        };
        let key = match typ {
            ALG_RSA => {
                let scheme = r.u16()?;
                if scheme != ALG_NULL {
                    let _hash = r.u16()?;
                }
                let bits = r.u16()?;
                let exponent = match r.u32()? {
                    0 => 65_537,
                    e => e,
                };
                let modulus = r.sized()?.to_vec();
                Key::Rsa {
                    bits,
                    exponent,
                    modulus,
                    symmetric,
                }
            }
            ALG_ECC => {
                let scheme = r.u16()?;
                let hash = if scheme == ALG_NULL {
                    ALG_NULL
                } else if scheme == ALG_ECDSA {
                    r.u16()?
                } else {
                    return Err(TpmError::Unsupported("an ECC scheme other than ECDSA"));
                };
                let curve = r.u16()?;
                let kdf = r.u16()?;
                if kdf != ALG_NULL {
                    let _kdf_hash = r.u16()?;
                }
                let x = r.sized()?.to_vec();
                let y = r.sized()?.to_vec();
                Key::Ecc {
                    curve,
                    scheme: (scheme, hash),
                    x,
                    y,
                }
            }
            _ => return Err(TpmError::Unsupported("a key that is neither RSA nor ECC")),
        };
        r.done()?;
        Ok(Public {
            name_alg,
            attributes,
            key,
            area: area.to_vec(),
        })
    }

    /// The object's name: its name algorithm, then that hash of the public
    /// area. What credential activation binds and a quote's signer names.
    pub fn name(&self) -> R<Vec<u8>> {
        if self.name_alg != ALG_SHA256 {
            return Err(TpmError::Unsupported("a name algorithm other than SHA-256"));
        }
        let mut out = ALG_SHA256.to_be_bytes().to_vec();
        out.extend_from_slice(&Sha256::digest(&self.area));
        Ok(out)
    }

    /// The `TPM2B_PUBLIC` bytes again.
    pub fn to_tpm2b(&self) -> Vec<u8> {
        sized(&self.area)
    }
}

/// Is `ak` an attestation key: a restricted P-256 ECDSA signing key with
/// SHA-256, fixed to its TPM.
pub fn check_ak(ak: &Public) -> R<()> {
    if ak.attributes & AK_ATTRIBUTES != AK_ATTRIBUTES || ak.attributes & DECRYPT != 0 {
        return Err(TpmError::Crypto(
            "the attestation key is not a restricted signing key fixed to its TPM",
        ));
    }
    match &ak.key {
        Key::Ecc {
            curve: ECC_NIST_P256,
            scheme: (ALG_ECDSA, ALG_SHA256),
            ..
        } => Ok(()),
        _ => Err(TpmError::Unsupported(
            "an attestation key other than P-256 ECDSA with SHA-256",
        )),
    }
}

// ── credential activation ─────────────────────────────────────────────────

/// KDFa (TPM 2.0 part 1, 11.4.10.2): SP 800-108 counter mode over
/// HMAC-SHA-256. `label` is given without its terminating NUL, which this
/// adds, as the specification counts it part of the label.
fn kdfa(key: &[u8], label: &str, context_u: &[u8], context_v: &[u8], bits: u32) -> Vec<u8> {
    let bytes = usize::try_from(bits.div_ceil(8)).unwrap_or(0);
    let mut out = Vec::with_capacity(bytes + 32);
    let mut counter: u32 = 1;
    while out.len() < bytes {
        let mut mac =
            <Hmac<Sha256> as Mac>::new_from_slice(key).expect("HMAC takes a key of any length");
        mac.update(&counter.to_be_bytes());
        mac.update(label.as_bytes());
        mac.update(&[0]);
        mac.update(context_u);
        mac.update(context_v);
        mac.update(&bits.to_be_bytes());
        out.extend_from_slice(&mac.finalize().into_bytes());
        counter += 1;
    }
    out.truncate(bytes);
    out
}

/// A credential for the object named `name`, readable only by the TPM that
/// holds the endorsement key `ek` and has that object loaded:
/// `(TPM2B_ID_OBJECT, TPM2B_ENCRYPTED_SECRET)`.
///
/// This is how a box learns that the attestation key a gateway shows it
/// lives in the same TPM as the endorsement key the maker certified: only
/// that TPM can turn the credential back into `secret`, and it does so only
/// for an object whose name matches.
pub fn make_credential(
    ek: &Public,
    name: &[u8],
    secret: &[u8],
    rng: &mut (impl CryptoRng + RngCore),
) -> R<(Vec<u8>, Vec<u8>)> {
    let Key::Rsa {
        modulus,
        exponent,
        symmetric,
        ..
    } = &ek.key
    else {
        return Err(TpmError::Unsupported("an endorsement key that is not RSA"));
    };
    if ek.name_alg != ALG_SHA256 {
        return Err(TpmError::Unsupported(
            "an endorsement key not named by SHA-256",
        ));
    }
    if *symmetric != Some((ALG_AES, 128)) {
        return Err(TpmError::Unsupported("an endorsement key without AES-128"));
    }
    if secret.len() > 32 {
        return Err(TpmError::Crypto("a credential secret is at most 32 bytes"));
    }

    let mut seed = [0u8; 32];
    rng.fill_bytes(&mut seed);
    let public = rsa::RsaPublicKey::new(
        rsa::BigUint::from_bytes_be(modulus),
        rsa::BigUint::from(*exponent),
    )
    .map_err(|_| TpmError::Crypto("the endorsement key is not a usable RSA key"))?;
    let encrypted_seed = public
        .encrypt(
            rng,
            rsa::Oaep::new_with_label::<Sha256, _>("IDENTITY\0"),
            &seed,
        )
        .map_err(|_| TpmError::Crypto("RSA-OAEP failed"))?;

    let sym_key = kdfa(&seed, "STORAGE", name, &[], 128);
    let mut enc_identity = sized(secret);
    cfb_mode::Encryptor::<aes::Aes128>::new(sym_key.as_slice().into(), &[0u8; 16].into())
        .encrypt(&mut enc_identity);

    let hmac_key = kdfa(&seed, "INTEGRITY", &[], &[], 256);
    let mut mac =
        <Hmac<Sha256> as Mac>::new_from_slice(&hmac_key).expect("HMAC takes a key of any length");
    mac.update(&enc_identity);
    mac.update(name);
    let outer = mac.finalize().into_bytes();

    let mut id_object = sized(&outer);
    id_object.extend_from_slice(&enc_identity);
    Ok((sized(&id_object), sized(&encrypted_seed)))
}

/// The two blobs in the file `tpm2_activatecredential -i` reads.
#[must_use]
pub fn credential_file(id_object: &[u8], encrypted_secret: &[u8]) -> Vec<u8> {
    let mut out = CREDENTIAL_FILE_MAGIC.to_be_bytes().to_vec();
    out.extend_from_slice(&CREDENTIAL_FILE_VERSION.to_be_bytes());
    out.extend_from_slice(id_object);
    out.extend_from_slice(encrypted_secret);
    out
}

/// Split a `tpm2_makecredential -o` file into its two blobs.
pub fn parse_credential_file(bytes: &[u8]) -> R<(Vec<u8>, Vec<u8>)> {
    let mut r = Reader::new(bytes);
    if r.u32()? != CREDENTIAL_FILE_MAGIC || r.u32()? != CREDENTIAL_FILE_VERSION {
        return Err(TpmError::Malformed("not a credential file"));
    }
    let id = sized(r.sized()?);
    let secret = sized(r.sized()?);
    r.done()?;
    Ok((id, secret))
}

// ── quotes ─────────────────────────────────────────────────────────────────

/// A `TPMS_ATTEST` of a quote: what the TPM vouched for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attest {
    /// The caller's qualifying data, echoed: a box puts its nonce here.
    pub extra_data: Vec<u8>,
    /// Boots since the TPM was made, and restarts since this boot.
    pub reset_count: u32,
    pub restart_count: u32,
    /// The PCRs quoted, per bank: `(hash, indices)`.
    pub selection: Vec<(u16, Vec<u8>)>,
    /// The hash of the quoted PCR values, concatenated in index order.
    pub pcr_digest: Vec<u8>,
}

impl Attest {
    pub fn parse(bytes: &[u8]) -> R<Attest> {
        let mut r = Reader::new(bytes);
        if r.u32()? != GENERATED {
            return Err(TpmError::Malformed("not made by a TPM"));
        }
        if r.u16()? != ST_ATTEST_QUOTE {
            return Err(TpmError::Malformed("not a quote"));
        }
        let _signer = r.sized()?;
        let extra_data = r.sized()?.to_vec();
        let _clock = r.u64()?;
        let reset_count = r.u32()?;
        let restart_count = r.u32()?;
        let _safe = r.u8()?;
        let _firmware = r.u64()?;
        let count = r.u32()?;
        if count > 4 {
            return Err(TpmError::Malformed("too many PCR banks"));
        }
        let mut selection = Vec::new();
        for _ in 0..count {
            let hash = r.u16()?;
            let size = r.u8()?;
            let bitmap = r.take(usize::from(size))?;
            let mut indices = Vec::new();
            for (byte, bits) in bitmap.iter().enumerate() {
                for bit in 0..8u8 {
                    if bits & (1 << bit) != 0 {
                        indices.push(u8::try_from(byte * 8).unwrap_or(u8::MAX) + bit);
                    }
                }
            }
            selection.push((hash, indices));
        }
        let pcr_digest = r.sized()?.to_vec();
        r.done()?;
        Ok(Attest {
            extra_data,
            reset_count,
            restart_count,
            selection,
            pcr_digest,
        })
    }
}

/// A `TPMT_SIGNATURE` made with ECDSA over SHA-256: `r` and `s`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub r: Vec<u8>,
    pub s: Vec<u8>,
}

impl Signature {
    pub fn parse(bytes: &[u8]) -> R<Signature> {
        let mut r = Reader::new(bytes);
        if r.u16()? != ALG_ECDSA || r.u16()? != ALG_SHA256 {
            return Err(TpmError::Unsupported(
                "a signature other than ECDSA with SHA-256",
            ));
        }
        let sig_r = r.sized()?.to_vec();
        let sig_s = r.sized()?.to_vec();
        r.done()?;
        Ok(Signature { r: sig_r, s: sig_s })
    }
}

fn left_pad_32(v: &[u8]) -> R<[u8; 32]> {
    let v = match v.iter().position(|b| *b != 0) {
        Some(i) => &v[i..],
        None => &[][..],
    };
    if v.len() > 32 {
        return Err(TpmError::Malformed("a P-256 value longer than 32 bytes"));
    }
    let mut out = [0u8; 32];
    out[32 - v.len()..].copy_from_slice(v);
    Ok(out)
}

/// Check that `ak` signed `attest`. The attestation key's shape is checked
/// separately ([`check_ak`]).
pub fn verify_quote(ak: &Public, attest: &[u8], sig: &Signature) -> R<()> {
    use p256::ecdsa::signature::Verifier;
    let Key::Ecc { x, y, .. } = &ak.key else {
        return Err(TpmError::Unsupported("an attestation key that is not ECC"));
    };
    let point = p256::EncodedPoint::from_affine_coordinates(
        &left_pad_32(x)?.into(),
        &left_pad_32(y)?.into(),
        false,
    );
    let key = p256::ecdsa::VerifyingKey::from_encoded_point(&point)
        .map_err(|_| TpmError::Crypto("the attestation key is not a point on P-256"))?;
    let signature =
        p256::ecdsa::Signature::from_scalars(left_pad_32(&sig.r)?, left_pad_32(&sig.s)?)
            .map_err(|_| TpmError::Crypto("the quote's signature is not a P-256 signature"))?;
    key.verify(attest, &signature)
        .map_err(|_| TpmError::Crypto("the quote is not signed by the attestation key"))
}

/// One PCR extend in the SHA-256 bank.
#[must_use]
pub fn extend(pcr: &[u8; 32], digest: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(pcr);
    h.update(digest);
    h.finalize().into()
}

/// The digest a quote carries over `values`, in index order.
#[must_use]
pub fn pcr_digest(values: &[[u8; 32]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for v in values {
        h.update(v);
    }
    h.finalize().into()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Made by `tpm2_createek -G rsa` and `tpm2_createak -G ecc -g sha256 -s
    /// ecdsa` on swtpm, then a `tpm2_quote -l sha256:4,11,15 -q
    /// 00112233…eeff` after extending PCR 15 with sha256("hello").
    pub(crate) mod vectors {
        include!("testdata/swtpm.rs");
    }

    fn hex(s: &str) -> Vec<u8> {
        hex::decode(s).unwrap()
    }

    #[test]
    fn the_keys_parse_and_their_names_are_what_the_tpm_said() {
        let ek = Public::from_tpm2b(&hex(vectors::EK_PUB)).unwrap();
        assert!(matches!(
            ek.key,
            Key::Rsa {
                bits: 2048,
                exponent: 65_537,
                symmetric: Some((ALG_AES, 128)),
                ..
            }
        ));
        let ak = Public::from_tpm2b(&hex(vectors::AK_PUB)).unwrap();
        check_ak(&ak).unwrap();
        assert_eq!(hex::encode(ak.name().unwrap()), vectors::AK_NAME);
        assert_eq!(ak.to_tpm2b(), hex(vectors::AK_PUB));
        // An EK is a decryption key: never an attestation key.
        assert!(check_ak(&ek).is_err());
    }

    #[test]
    fn the_quote_verifies_and_carries_the_nonce_and_the_pcr_digest() {
        let ak = Public::from_tpm2b(&hex(vectors::AK_PUB)).unwrap();
        let attest_bytes = hex(vectors::ATTEST);
        let sig = Signature::parse(&hex(vectors::SIG)).unwrap();
        verify_quote(&ak, &attest_bytes, &sig).unwrap();
        let attest = Attest::parse(&attest_bytes).unwrap();
        assert_eq!(attest.extra_data, hex(vectors::QUALIFYING));
        assert_eq!(attest.selection, vec![(ALG_SHA256, vec![4, 11, 15])]);
        let values = hex(vectors::PCR_VALUES);
        let values: Vec<[u8; 32]> = values
            .chunks(32)
            .map(|c| <[u8; 32]>::try_from(c).unwrap())
            .collect();
        assert_eq!(attest.pcr_digest, pcr_digest(&values));
        // PCR 15 was extended once, with sha256("hello"), from zero.
        assert_eq!(
            values[2],
            extend(&[0; 32], &Sha256::digest(b"hello")),
            "the PCR 15 replay"
        );

        // One flipped bit anywhere and it does not.
        let mut forged = attest_bytes.clone();
        let last = forged.len() - 1;
        forged[last] ^= 1;
        assert!(verify_quote(&ak, &forged, &sig).is_err());
        let ek = Public::from_tpm2b(&hex(vectors::EK_PUB)).unwrap();
        assert!(verify_quote(&ek, &attest_bytes, &sig).is_err());
    }

    #[test]
    fn a_credential_file_round_trips_and_ours_has_the_tools_shape() {
        let (id, secret) = parse_credential_file(&hex(vectors::CREDENTIAL_FILE)).unwrap();
        assert_eq!(credential_file(&id, &secret), hex(vectors::CREDENTIAL_FILE));

        let ek = Public::from_tpm2b(&hex(vectors::EK_PUB)).unwrap();
        let mut rng = rand::thread_rng();
        let (ours_id, ours_secret) =
            make_credential(&ek, &hex(vectors::AK_NAME), &[7; 32], &mut rng).unwrap();
        // The same sizes as the tool's: the HMAC, the encrypted 32-byte
        // secret with its size, and a 2048-bit RSA block.
        assert_eq!(ours_id.len(), id.len());
        assert_eq!(ours_secret.len(), secret.len());
        assert_eq!(&ours_id[..4], &id[..4]);
    }

    /// Writes a credential for a live TPM's keys, for the vector script
    /// to activate on swtpm: `GW_EK` (tpm2_createek -u), `GW_NAME`
    /// (tpm2_createak -n), `GW_OUT`. The secret is 32 bytes of 7.
    #[test]
    #[ignore = "run by the swtpm vector script"]
    fn make_credential_for_a_live_tpm() {
        let ek =
            Public::from_tpm2b(&std::fs::read(std::env::var("GW_EK").unwrap()).unwrap()).unwrap();
        let name = std::fs::read(std::env::var("GW_NAME").unwrap()).unwrap();
        let (id, secret) = make_credential(&ek, &name, &[7; 32], &mut rand::thread_rng()).unwrap();
        std::fs::write(
            std::env::var("GW_OUT").unwrap(),
            credential_file(&id, &secret),
        )
        .unwrap();
    }

    #[test]
    fn kdfa_matches_the_specification_example_shape() {
        // Deterministic, sized as asked, and label-sensitive.
        let a = kdfa(&[1; 32], "STORAGE", b"name", &[], 128);
        let b = kdfa(&[1; 32], "INTEGRITY", b"name", &[], 128);
        assert_eq!(a.len(), 16);
        assert_ne!(a, b);
        assert_eq!(kdfa(&[1; 32], "INTEGRITY", &[], &[], 256).len(), 32);
    }

    #[test]
    fn garbage_is_refused_not_panicked_on() {
        for bytes in [&[][..], &[0, 5, 1][..], &[0xff; 3][..]] {
            assert!(Public::from_tpm2b(bytes).is_err());
            assert!(Attest::parse(bytes).is_err());
            assert!(Signature::parse(bytes).is_err());
            assert!(parse_credential_file(bytes).is_err());
        }
    }
}
