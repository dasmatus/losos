//! The firmware's TPM event log, as Linux exposes it in
//! `/sys/kernel/security/tpm0/binary_bios_measurements`.
//!
//! A quote says what PCR 4 holds, not how it got there. The log says how:
//! one entry per extend, each with the digest that went in. Replaying the
//! log's SHA-256 digests for PCR 4 and arriving at the quoted value proves
//! the log is the true history of that PCR, and then its entries can be
//! judged one by one: every program the firmware started is there as an
//! `EV_EFI_BOOT_SERVICES_APPLICATION` with its Authenticode hash. That is
//! how a box tells the published unified kernel image, started straight by
//! the firmware, from a loader of someone's own that measures the expected
//! values into PCR 11 and then boots something else.
//!
//! The format is the TCG PC Client "crypto agile" log (TCG PC Client
//! Platform Firmware Profile, 10.2): little-endian, unlike the TPM's own
//! structures.

use super::tpm::ALG_SHA256;

pub const EV_NO_ACTION: u32 = 0x0000_0003;
pub const EV_SEPARATOR: u32 = 0x0000_0004;
pub const EV_EFI_BOOT_SERVICES_APPLICATION: u32 = 0x8000_0003;
pub const EV_EFI_ACTION: u32 = 0x8000_0007;

const SPEC_ID: &[u8] = b"Spec ID Event03\0";
/// More entries than any real boot makes; a bound, not a guess.
const MAX_EVENTS: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LogError {
    #[error("the event log is not a crypto-agile TCG log: {0}")]
    Malformed(&'static str),
}

/// One extend: which PCR, what kind of event, the SHA-256 digest that went
/// in, and the event's own data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub pcr: u32,
    pub kind: u32,
    pub sha256: [u8; 32],
    pub data: Vec<u8>,
}

struct Le<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Le<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], LogError> {
        let end = self
            .at
            .checked_add(n)
            .filter(|e| *e <= self.b.len())
            .ok_or(LogError::Malformed("truncated"))?;
        let out = &self.b[self.at..end];
        self.at = end;
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8, LogError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, LogError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32, LogError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn rest(&self) -> usize {
        self.b.len() - self.at
    }
}

/// Every event in the log that carries a SHA-256 digest. The first entry,
/// the old-format header, names the banks and their digest sizes; an entry
/// without a SHA-256 digest is an error, since the SHA-256 bank is the one
/// quoted.
pub fn parse(bytes: &[u8]) -> Result<Vec<Event>, LogError> {
    let mut r = Le { b: bytes, at: 0 };
    // The header: a TCG_PCR_EVENT with a SHA-1 sized digest field.
    let _pcr = r.u32()?;
    if r.u32()? != EV_NO_ACTION {
        return Err(LogError::Malformed("the first entry is not the header"));
    }
    r.take(20)?;
    let size = r.u32()? as usize;
    let header = r.take(size)?;
    let mut h = Le { b: header, at: 0 };
    if h.take(16)? != SPEC_ID {
        return Err(LogError::Malformed("no Spec ID Event03 header"));
    }
    h.take(4 + 4)?; // platform class, version, errata, uintn size
    let banks = h.u32()?;
    if banks == 0 || banks > 8 {
        return Err(LogError::Malformed("an implausible number of banks"));
    }
    let mut sizes = Vec::new();
    for _ in 0..banks {
        let alg = h.u16()?;
        let size = h.u16()?;
        sizes.push((alg, usize::from(size)));
    }
    let vendor = h.u8()?;
    h.take(usize::from(vendor))?;

    let mut events = Vec::new();
    while r.rest() > 0 {
        if events.len() >= MAX_EVENTS {
            return Err(LogError::Malformed("too many entries"));
        }
        let pcr = r.u32()?;
        let kind = r.u32()?;
        let count = r.u32()?;
        if count == 0 && pcr == 0 && kind == 0 {
            // Some firmware pads the log with zeroes.
            break;
        }
        let mut sha256 = None;
        for _ in 0..count {
            let alg = r.u16()?;
            let size = sizes
                .iter()
                .find(|(a, _)| *a == alg)
                .map(|(_, s)| *s)
                .ok_or(LogError::Malformed(
                    "a digest of a bank the header did not name",
                ))?;
            let digest = r.take(size)?;
            if alg == ALG_SHA256 {
                sha256 = Some(
                    <[u8; 32]>::try_from(digest)
                        .map_err(|_| LogError::Malformed("a SHA-256 digest of the wrong size"))?,
                );
            }
        }
        let size = r.u32()? as usize;
        let data = r.take(size)?.to_vec();
        let sha256 = sha256.ok_or(LogError::Malformed("an entry without a SHA-256 digest"))?;
        events.push(Event {
            pcr,
            kind,
            sha256,
            data,
        });
    }
    Ok(events)
}

/// The value `pcr` holds after `events`, from zero. `EV_NO_ACTION` entries
/// are not extended.
#[must_use]
pub fn replay(events: &[Event], pcr: u32) -> [u8; 32] {
    events
        .iter()
        .filter(|e| e.pcr == pcr && e.kind != EV_NO_ACTION)
        .fold([0u8; 32], |acc, e| super::tpm::extend(&acc, &e.sha256))
}

/// The Authenticode hashes of every program the firmware started, in order,
/// or the first PCR 4 entry that is neither a program, a separator nor a
/// plain action (whose kind is returned).
pub fn boot_applications(events: &[Event]) -> Result<Vec<[u8; 32]>, u32> {
    let mut apps = Vec::new();
    for e in events.iter().filter(|e| e.pcr == 4) {
        match e.kind {
            EV_EFI_BOOT_SERVICES_APPLICATION => apps.push(e.sha256),
            EV_EFI_ACTION | EV_SEPARATOR | EV_NO_ACTION => {}
            other => return Err(other),
        }
    }
    Ok(apps)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A log with the header, then PCR 4: an action, a separator and two
    /// programs.
    pub(crate) fn log(apps: &[[u8; 32]]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&EV_NO_ACTION.to_le_bytes());
        out.extend_from_slice(&[0; 20]);
        let mut header = SPEC_ID.to_vec();
        header.extend_from_slice(&[0, 0, 0, 0, 0, 2, 0, 2]);
        header.extend_from_slice(&2u32.to_le_bytes());
        header.extend_from_slice(&0x0004u16.to_le_bytes());
        header.extend_from_slice(&20u16.to_le_bytes());
        header.extend_from_slice(&ALG_SHA256.to_le_bytes());
        header.extend_from_slice(&32u16.to_le_bytes());
        header.push(0);
        out.extend_from_slice(&u32::try_from(header.len()).unwrap().to_le_bytes());
        out.extend_from_slice(&header);
        let mut push = |pcr: u32, kind: u32, digest: [u8; 32], data: &[u8]| {
            out.extend_from_slice(&pcr.to_le_bytes());
            out.extend_from_slice(&kind.to_le_bytes());
            out.extend_from_slice(&2u32.to_le_bytes());
            out.extend_from_slice(&0x0004u16.to_le_bytes());
            out.extend_from_slice(&[0xaa; 20]);
            out.extend_from_slice(&ALG_SHA256.to_le_bytes());
            out.extend_from_slice(&digest);
            out.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
            out.extend_from_slice(data);
        };
        push(
            4,
            EV_EFI_ACTION,
            [1; 32],
            b"Calling EFI Application from Boot Option",
        );
        push(4, EV_SEPARATOR, [2; 32], &[0, 0, 0, 0]);
        push(7, 0x8000_0001, [9; 32], b"SecureBoot");
        for a in apps {
            push(4, EV_EFI_BOOT_SERVICES_APPLICATION, *a, b"");
        }
        out
    }

    #[test]
    fn a_log_parses_replays_and_names_its_programs() {
        let events = parse(&log(&[[3; 32], [4; 32]])).unwrap();
        assert_eq!(events.len(), 5);
        assert_eq!(boot_applications(&events), Ok(vec![[3; 32], [4; 32]]));
        let mut pcr = [0u8; 32];
        for d in [[1u8; 32], [2; 32], [3; 32], [4; 32]] {
            pcr = super::super::tpm::extend(&pcr, &d);
        }
        assert_eq!(replay(&events, 4), pcr);
    }

    #[test]
    fn anything_else_in_pcr_4_is_refused_and_garbage_is_an_error() {
        let mut events = parse(&log(&[[3; 32]])).unwrap();
        events.push(Event {
            pcr: 4,
            kind: 0x0000_000d,
            sha256: [5; 32],
            data: vec![],
        });
        assert_eq!(boot_applications(&events), Err(0x0000_000d));
        assert!(parse(&[]).is_err());
        assert!(parse(&[0; 64]).is_err());
        let full = log(&[[3; 32]]);
        assert!(parse(&full[..full.len() - 3]).is_err());
    }
}
