//! Authenticode hashes of PE images: the digest UEFI firmware measures into
//! PCR 4 for every program it starts. The gateway image's build computes
//! the hashes of its unified kernel image and of the kernel inside it with
//! this, so the box knows in advance which programs may appear in PCR 4.
//!
//! The algorithm is Microsoft's "Windows Authenticode Portable Executable
//! Signature Format", as EDK2's `HashPeImage` implements it: the headers
//! without the checksum and the certificate-table entry, then every section
//! in file order, then whatever follows the sections apart from the
//! certificate table.

use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PeError {
    #[error("not a PE image: {0}")]
    Malformed(&'static str),
}

fn u16_at(b: &[u8], at: usize) -> Result<u16, PeError> {
    b.get(at..at + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or(PeError::Malformed("truncated"))
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, PeError> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or(PeError::Malformed("truncated"))
}

/// One section header, as much of it as hashing and extraction need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub virtual_size: usize,
    pub raw_offset: usize,
    pub raw_size: usize,
}

struct Layout {
    checksum: usize,
    cert_entry: usize,
    cert: (usize, usize),
    headers: usize,
    sections: Vec<Section>,
}

fn layout(pe: &[u8]) -> Result<Layout, PeError> {
    if pe.get(..2) != Some(b"MZ") {
        return Err(PeError::Malformed("no MZ header"));
    }
    let nt = u32_at(pe, 0x3c)? as usize;
    if pe.get(nt..nt + 4) != Some(b"PE\0\0") {
        return Err(PeError::Malformed("no PE signature"));
    }
    let coff = nt + 4;
    let count = usize::from(u16_at(pe, coff + 2)?);
    let optional_size = usize::from(u16_at(pe, coff + 16)?);
    let opt = coff + 20;
    let (dirs, rva_count_at) = match u16_at(pe, opt)? {
        0x10b => (opt + 96, opt + 92),
        0x20b => (opt + 112, opt + 108),
        _ => {
            return Err(PeError::Malformed(
                "an optional header that is neither PE32 nor PE32+",
            ))
        }
    };
    let headers = u32_at(pe, opt + 60)? as usize;
    let cert_entry = dirs + 4 * 8;
    let cert = if u32_at(pe, rva_count_at)? > 4 {
        (
            u32_at(pe, cert_entry)? as usize,
            u32_at(pe, cert_entry + 4)? as usize,
        )
    } else {
        (0, 0)
    };
    let table = opt + optional_size;
    let mut sections = Vec::with_capacity(count);
    for i in 0..count {
        let at = table + i * 40;
        let raw_name = pe.get(at..at + 8).ok_or(PeError::Malformed("truncated"))?;
        let end = raw_name.iter().position(|b| *b == 0).unwrap_or(8);
        sections.push(Section {
            name: String::from_utf8_lossy(&raw_name[..end]).into_owned(),
            virtual_size: u32_at(pe, at + 8)? as usize,
            raw_size: u32_at(pe, at + 16)? as usize,
            raw_offset: u32_at(pe, at + 20)? as usize,
        });
    }
    if headers > pe.len() || cert_entry + 8 > headers {
        return Err(PeError::Malformed("headers past the end of the file"));
    }
    Ok(Layout {
        checksum: opt + 64,
        cert_entry,
        cert,
        headers,
        sections,
    })
}

/// The sections of `pe`, in header order.
pub fn sections(pe: &[u8]) -> Result<Vec<Section>, PeError> {
    Ok(layout(pe)?.sections)
}

/// The contents of the section called `name`, cut to its virtual size: the
/// bytes a loader sees, which is what systemd-stub hands the firmware when
/// it starts the kernel inside a unified kernel image.
pub fn section<'a>(pe: &'a [u8], name: &str) -> Result<Option<&'a [u8]>, PeError> {
    let Some(s) = layout(pe)?.sections.into_iter().find(|s| s.name == name) else {
        return Ok(None);
    };
    let len = s.virtual_size.min(s.raw_size);
    pe.get(s.raw_offset..s.raw_offset + len)
        .map(Some)
        .ok_or(PeError::Malformed("a section past the end of the file"))
}

/// The Authenticode SHA-256 of `pe`.
pub fn authenticode_sha256(pe: &[u8]) -> Result<[u8; 32], PeError> {
    let l = layout(pe)?;
    let mut h = Sha256::new();
    h.update(&pe[..l.checksum]);
    h.update(&pe[l.checksum + 4..l.cert_entry]);
    h.update(&pe[l.cert_entry + 8..l.headers]);
    let mut sections: Vec<&Section> = l.sections.iter().filter(|s| s.raw_size > 0).collect();
    sections.sort_by_key(|s| s.raw_offset);
    let mut hashed = l.headers;
    for s in sections {
        let data = pe
            .get(s.raw_offset..s.raw_offset + s.raw_size)
            .ok_or(PeError::Malformed("a section past the end of the file"))?;
        h.update(data);
        hashed += s.raw_size;
    }
    let (cert_at, cert_len) = l.cert;
    let end = if cert_len > 0 && cert_at + cert_len <= pe.len() {
        pe.len() - cert_len
    } else {
        pe.len()
    };
    if end > hashed {
        h.update(&pe[hashed..end]);
    }
    Ok(h.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The smallest PE32+ these functions accept: headers, one section.
    pub(crate) fn tiny_pe(body: &[u8]) -> Vec<u8> {
        let mut pe = vec![0u8; 0x200];
        pe[..2].copy_from_slice(b"MZ");
        pe[0x3c..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        pe[0x40..0x44].copy_from_slice(b"PE\0\0");
        let coff = 0x44;
        pe[coff + 2..coff + 4].copy_from_slice(&1u16.to_le_bytes());
        pe[coff + 16..coff + 18].copy_from_slice(&240u16.to_le_bytes());
        let opt = coff + 20;
        pe[opt..opt + 2].copy_from_slice(&0x20bu16.to_le_bytes());
        pe[opt + 60..opt + 64].copy_from_slice(&0x200u32.to_le_bytes());
        pe[opt + 64..opt + 68].copy_from_slice(&0xdead_beefu32.to_le_bytes());
        pe[opt + 108..opt + 112].copy_from_slice(&16u32.to_le_bytes());
        let table = opt + 240;
        pe[table..table + 6].copy_from_slice(b".linux");
        let size = u32::try_from(body.len()).unwrap();
        pe[table + 8..table + 12].copy_from_slice(&size.to_le_bytes());
        pe[table + 16..table + 20].copy_from_slice(&size.to_le_bytes());
        pe[table + 20..table + 24].copy_from_slice(&0x200u32.to_le_bytes());
        pe.extend_from_slice(body);
        pe
    }

    #[test]
    fn the_checksum_is_not_hashed_and_the_sections_are() {
        let a = tiny_pe(b"kernel");
        let mut b = a.clone();
        let opt = 0x44 + 20;
        b[opt + 64] ^= 0xff;
        assert_eq!(authenticode_sha256(&a), authenticode_sha256(&b));
        let c = tiny_pe(b"kernal");
        assert_ne!(authenticode_sha256(&a), authenticode_sha256(&c));
        assert_eq!(section(&a, ".linux").unwrap(), Some(&b"kernel"[..]));
        assert_eq!(section(&a, ".initrd").unwrap(), None);
        assert_eq!(sections(&a).unwrap()[0].name, ".linux");
    }

    #[test]
    fn not_a_pe_is_an_error() {
        assert!(authenticode_sha256(b"hello").is_err());
        assert!(authenticode_sha256(&[]).is_err());
        let mut bad = tiny_pe(b"x");
        bad[0x40] = b'X';
        assert!(authenticode_sha256(&bad).is_err());
    }
}
