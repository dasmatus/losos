//! What a unified kernel image leaves in PCR 11.
//!
//! systemd-stub measures the image's own sections before it starts the
//! kernel: for each section it knows, in a fixed order, one extend with the
//! SHA-256 of the section's name (with its terminating NUL) and one with the
//! SHA-256 of its contents. `.pcrsig` is left out, since it carries the
//! signature over the result. The gateway image never enables the boot
//! phase measurements (`systemd-pcrphase`), so this is the value a running
//! gateway's PCR 11 holds, and the one a box expects.

use sha2::{Digest, Sha256};

use super::pe::{self, PeError};
use super::tpm::extend;

/// The sections systemd-stub measures, in the order it measures them
/// (systemd's `unified_sections`, minus `.pcrsig`).
pub const MEASURED: [&str; 14] = [
    ".linux", ".osrel", ".cmdline", ".initrd", ".ucode", ".splash", ".dtb", ".uname", ".sbat",
    ".pcrpkey", ".profile", ".dtbauto", ".hwids", ".efifw",
];

/// PCR 11 after systemd-stub measured `uki`, from zero. An image with
/// profiles (`.profile` sections) is refused: the stub would measure the
/// base and one chosen profile, and the gateway image has none.
pub fn pcr11(uki: &[u8]) -> Result<[u8; 32], PeError> {
    let sections = pe::sections(uki)?;
    if sections.iter().filter(|s| s.name == ".profile").count() > 0 {
        return Err(PeError::Malformed("a unified kernel image with profiles"));
    }
    let mut pcr = [0u8; 32];
    for name in MEASURED {
        if let Some(data) = pe::section(uki, name)? {
            let mut label = name.as_bytes().to_vec();
            label.push(0);
            pcr = extend(&pcr, &Sha256::digest(&label));
            pcr = extend(&pcr, &Sha256::digest(data));
        }
    }
    Ok(pcr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pe_without_known_sections_leaves_pcr_11_at_zero() {
        let mut pe = vec![0u8; 0x200];
        pe[..2].copy_from_slice(b"MZ");
        assert!(pcr11(&pe).is_err());
    }

    /// Compared with `systemd-measure calculate` on a ukify-built image by
    /// the swtpm vector script; set `GW_UKI` and `GW_PCR11` to rerun.
    #[test]
    #[ignore = "needs a unified kernel image and systemd-measure's answer"]
    fn matches_systemd_measure() {
        let uki = std::fs::read(std::env::var("GW_UKI").unwrap()).unwrap();
        let expected = std::env::var("GW_PCR11").unwrap();
        assert_eq!(hex::encode(pcr11(&uki).unwrap()), expected.to_lowercase());
    }
}
