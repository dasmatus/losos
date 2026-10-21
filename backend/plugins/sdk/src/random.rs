//! The kernel's random numbers.

/// `N` random bytes.
#[must_use]
pub fn array<const N: usize>() -> [u8; N] {
    let mut out = [0u8; N];
    fill(&mut out);
    out
}

/// Fill `buf` with random bytes.
pub fn fill(buf: &mut [u8]) {
    for chunk in buf.chunks_mut(64 * 1024) {
        let bytes = crate::wit::random::bytes(chunk.len() as u32);
        chunk.copy_from_slice(&bytes[..chunk.len()]);
    }
}

/// `n` random bytes as lower-case hex, `2n` characters.
#[must_use]
pub fn hex(n: usize) -> String {
    let mut bytes = vec![0u8; n];
    fill(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
