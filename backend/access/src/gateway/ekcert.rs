//! The endorsement key's certificate: the TPM maker's word that the key
//! belongs to a real TPM, not to software pretending to be one.
//!
//! A box ships the makers' and the cloud vTPM providers' root certificates
//! and walks the gateway's EK certificate up to one of them through whatever
//! intermediates the gateway sends along. Only the signatures and the CA
//! flag are checked: EK certificates are issued at the factory with
//! validity periods that firmware and makers treat loosely (some end in
//! 9999, some before the chip's service life does), so a box does not judge
//! a TPM by its certificate's dates, as other attestation verifiers do not.

use x509_cert::der::{Decode, Encode};
use x509_cert::ext::pkix::BasicConstraints;
use x509_cert::Certificate;

use super::tpm::{Key, Public};

const RSA_SHA256: &str = "1.2.840.113549.1.1.11";
const RSA_SHA384: &str = "1.2.840.113549.1.1.12";
const RSA_SHA512: &str = "1.2.840.113549.1.1.13";
const ECDSA_SHA256: &str = "1.2.840.10045.4.3.2";
const ECDSA_SHA384: &str = "1.2.840.10045.4.3.3";
const BASIC_CONSTRAINTS: &str = "2.5.29.19";
/// Leaf, intermediates, root: more than any maker uses.
const MAX_DEPTH: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CertError {
    #[error("not an X.509 certificate")]
    Malformed,
    #[error("the certificate is for another key than the TPM's endorsement key")]
    WrongKey,
    #[error("the certificate does not lead to a TPM maker this box knows")]
    UnknownIssuer,
    #[error("a signature algorithm this box does not check: {0}")]
    Unsupported(String),
}

/// Parse DER.
pub fn parse(der: &[u8]) -> Result<Certificate, CertError> {
    Certificate::from_der(der).map_err(|_| CertError::Malformed)
}

/// Does `cert` certify the RSA endorsement key `ek`?
pub fn certifies(cert: &Certificate, ek: &Public) -> Result<(), CertError> {
    use rsa::pkcs8::DecodePublicKey;
    use rsa::traits::PublicKeyParts;
    let Key::Rsa {
        modulus, exponent, ..
    } = &ek.key
    else {
        return Err(CertError::WrongKey);
    };
    let spki = cert
        .tbs_certificate
        .subject_public_key_info
        .to_der()
        .map_err(|_| CertError::Malformed)?;
    let key = rsa::RsaPublicKey::from_public_key_der(&spki).map_err(|_| CertError::WrongKey)?;
    let theirs = key.n().to_bytes_be();
    let ours = rsa::BigUint::from_bytes_be(modulus).to_bytes_be();
    if theirs == ours && key.e() == &rsa::BigUint::from(*exponent) {
        Ok(())
    } else {
        Err(CertError::WrongKey)
    }
}

fn is_ca(cert: &Certificate) -> bool {
    cert.tbs_certificate
        .extensions
        .iter()
        .flatten()
        .find(|e| e.extn_id.to_string() == BASIC_CONSTRAINTS)
        .and_then(|e| BasicConstraints::from_der(e.extn_value.as_bytes()).ok())
        .is_some_and(|b| b.ca)
}

/// Did `issuer` sign `cert`?
fn signed_by(cert: &Certificate, issuer: &Certificate) -> Result<bool, CertError> {
    if cert.tbs_certificate.issuer != issuer.tbs_certificate.subject {
        return Ok(false);
    }
    let tbs = cert
        .tbs_certificate
        .to_der()
        .map_err(|_| CertError::Malformed)?;
    let sig = cert.signature.raw_bytes();
    let spki = issuer
        .tbs_certificate
        .subject_public_key_info
        .to_der()
        .map_err(|_| CertError::Malformed)?;
    let alg = cert.signature_algorithm.oid.to_string();
    Ok(match alg.as_str() {
        RSA_SHA256 => rsa_verify::<sha2::Sha256>(&spki, &tbs, sig),
        RSA_SHA384 => rsa_verify::<sha2::Sha384>(&spki, &tbs, sig),
        RSA_SHA512 => rsa_verify::<sha2::Sha512>(&spki, &tbs, sig),
        ECDSA_SHA256 | ECDSA_SHA384 => ecdsa_verify(&alg, &spki, &tbs, sig),
        other => return Err(CertError::Unsupported(other.to_string())),
    })
}

fn rsa_verify<D>(spki: &[u8], tbs: &[u8], sig: &[u8]) -> bool
where
    D: sha2::Digest + rsa::pkcs8::AssociatedOid,
{
    use rsa::pkcs8::DecodePublicKey;
    use rsa::signature::Verifier;
    let Ok(key) = rsa::RsaPublicKey::from_public_key_der(spki) else {
        return false;
    };
    let Ok(sig) = rsa::pkcs1v15::Signature::try_from(sig) else {
        return false;
    };
    rsa::pkcs1v15::VerifyingKey::<D>::new(key)
        .verify(tbs, &sig)
        .is_ok()
}

/// ECDSA with the hash the algorithm names, on the curve the issuer's key
/// is on (P-256 or P-384).
fn ecdsa_verify(alg: &str, spki: &[u8], tbs: &[u8], sig: &[u8]) -> bool {
    use p256::ecdsa::signature::hazmat::PrehashVerifier;
    use p256::pkcs8::DecodePublicKey;
    use sha2::Digest;
    let digest = if alg == ECDSA_SHA256 {
        sha2::Sha256::digest(tbs).to_vec()
    } else {
        sha2::Sha384::digest(tbs).to_vec()
    };
    if let Ok(key) = p256::ecdsa::VerifyingKey::from_public_key_der(spki) {
        return p256::ecdsa::Signature::from_der(sig)
            .is_ok_and(|s| key.verify_prehash(&digest, &s).is_ok());
    }
    if let Ok(key) = p384::ecdsa::VerifyingKey::from_public_key_der(spki) {
        return p384::ecdsa::Signature::from_der(sig)
            .is_ok_and(|s| key.verify_prehash(&digest, &s).is_ok());
    }
    false
}

/// Walk `leaf` up through `intermediates` to one of `roots`. Every issuer
/// on the way that is not a root must say it is a CA.
pub fn chains_to(
    leaf: &Certificate,
    intermediates: &[Certificate],
    roots: &[Certificate],
) -> Result<(), CertError> {
    let mut at = leaf.clone();
    for _ in 0..MAX_DEPTH {
        for root in roots {
            if signed_by(&at, root)? {
                return Ok(());
            }
        }
        let mut next = None;
        for c in intermediates {
            if is_ca(c) && signed_by(&at, c)? {
                next = Some(c.clone());
                break;
            }
        }
        match next {
            Some(c) => at = c,
            None => return Err(CertError::UnknownIssuer),
        }
    }
    Err(CertError::UnknownIssuer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::tpm::tests::vectors;

    fn hex(s: &str) -> Vec<u8> {
        hex::decode(s).unwrap()
    }

    #[test]
    fn the_swtpm_certificate_names_its_ek_and_chains_to_its_ca() {
        let ek = Public::from_tpm2b(&hex(vectors::EK_PUB)).unwrap();
        let ak = Public::from_tpm2b(&hex(vectors::AK_PUB)).unwrap();
        let cert = parse(&hex(vectors::EK_CERT)).unwrap();
        let ca = parse(&hex(vectors::CA_CERT)).unwrap();
        certifies(&cert, &ek).unwrap();
        assert_eq!(certifies(&cert, &ak), Err(CertError::WrongKey));
        chains_to(&cert, &[], std::slice::from_ref(&ca)).unwrap();
        // The CA as an intermediate still needs a root above it.
        assert_eq!(
            chains_to(&cert, std::slice::from_ref(&ca), &[]),
            Err(CertError::UnknownIssuer)
        );
        // A leaf is not its own root.
        assert_eq!(
            chains_to(&cert, &[], std::slice::from_ref(&cert)),
            Err(CertError::UnknownIssuer)
        );
    }

    #[test]
    fn a_tampered_certificate_does_not_chain() {
        let ca = parse(&hex(vectors::CA_CERT)).unwrap();
        let mut der = hex(vectors::EK_CERT);
        // A byte inside the subject public key, well inside the TBS.
        der[300] ^= 1;
        let cert = parse(&der).unwrap();
        assert_eq!(
            chains_to(&cert, &[], std::slice::from_ref(&ca)),
            Err(CertError::UnknownIssuer)
        );
        assert_eq!(parse(b"junk").unwrap_err(), CertError::Malformed);
    }
}
