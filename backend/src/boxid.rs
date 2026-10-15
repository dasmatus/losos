//! The box's public UUID.
//!
//! The appliance already has one UUID — the recovery code — but that is a
//! credential (see `recovery.rs`), so it must not be handed to Stripe, which
//! stores account metadata in plain view of the dashboard, nor to the edge.
//! What is shared instead is a *derived* identifier: a one-way function of the
//! code, formatted as a UUID. It is stable for the life of the installation,
//! identifies this box on the Stripe account, and reveals nothing from which
//! the code could be recovered.

use crate::recovery::format_uuid_v4;

/// Domain separation: the same code hashed for any other purpose gives a
/// different digest.
const DOMAIN: &str = "losos-box-id-v1:";

/// SHA-256 of `data`.
#[must_use]
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(ring::digest::digest(&ring::digest::SHA256, data).as_ref());
    out
}

/// The UUID this box is known by outside it, derived from its recovery code.
#[must_use]
pub fn box_uuid(recovery_code: &str) -> String {
    let digest = sha256(format!("{DOMAIN}{recovery_code}").as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    format_uuid_v4(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn sha256_matches_the_fips_vectors() {
        assert_eq!(
            hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(&sha256(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn the_box_uuid_never_changes() {
        // Stripe and the edge store this value; a different hash would
        // orphan every account.
        assert_eq!(
            box_uuid("3f2b8c1e-7a4d-4e9b-9c15-0d6a2b7e4f31"),
            "62ade026-4e00-4559-94de-8e82cc8abb99"
        );
    }

    #[test]
    fn the_box_uuid_is_a_stable_canonical_uuid() {
        let code = "3f2b8c1e-7a4d-4e9b-9c15-0d6a2b7e4f31";
        let id = box_uuid(code);
        assert_eq!(id, box_uuid(code));
        assert!(
            crate::recovery::is_well_formed(&id),
            "generated box UUID is not well formed"
        );
        assert_ne!(id, box_uuid("0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d"));
    }

    #[test]
    fn the_box_uuid_does_not_contain_the_recovery_code() {
        let code = "3f2b8c1e-7a4d-4e9b-9c15-0d6a2b7e4f31";
        let id = box_uuid(code);
        assert_ne!(id, code);
        assert!(!id.contains(&code[..8]));
    }
}
