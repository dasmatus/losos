//! The box's check of a Monero key pair before it goes to the edge: the
//! private key the owner pasted must be the view key of the address, never
//! its spend key. The edge checks the same and more (the checksum, the
//! network); this half exists so a spend key pasted by mistake never leaves
//! the box.

use curve25519_dalek::{EdwardsPoint, Scalar};

const ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
/// Characters each tail length of Monero's 8-byte base58 blocks takes.
const BLOCK_CHARS: [usize; 9] = [0, 2, 3, 5, 6, 7, 9, 10, 11];

/// Monero's base58: 8-byte blocks of 11 characters, the tail in fewer.
fn base58_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 8 / 11 + 8);
    for chunk in text.as_bytes().chunks(11) {
        let size = BLOCK_CHARS.iter().position(|n| *n == chunk.len())?;
        let mut value: u128 = 0;
        for c in chunk {
            let d = ALPHABET.iter().position(|a| a == c)? as u128;
            value = value.checked_mul(58)?.checked_add(d)?;
        }
        if value >> (8 * size) != 0 {
            return None;
        }
        out.extend_from_slice(&(value as u64).to_be_bytes()[8 - size..]);
    }
    Some(out)
}

/// The public spend and view keys of a 95-character address.
fn public_keys(address: &str) -> Option<([u8; 32], [u8; 32])> {
    if address.len() != 95 {
        return None;
    }
    let raw = base58_decode(address)?;
    if raw.len() != 69 {
        return None;
    }
    let mut spend = [0u8; 32];
    let mut view = [0u8; 32];
    spend.copy_from_slice(&raw[1..33]);
    view.copy_from_slice(&raw[33..65]);
    Some((spend, view))
}

fn hex32(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(out)
}

/// Why `view_key` is not the private view key of `address`, if it is not.
pub fn check(address: &str, view_key: &str) -> Result<(), &'static str> {
    let (spend, view) = public_keys(address).ok_or("key is not a Monero address")?;
    let scalar = hex32(view_key)
        .and_then(|b| Option::<Scalar>::from(Scalar::from_canonical_bytes(b)))
        .ok_or("view_key is not a Monero private key")?;
    let public = EdwardsPoint::mul_base(&scalar).compress().to_bytes();
    if public == spend {
        return Err("view_key is the spend key; it stays in your wallet");
    }
    if public != view {
        return Err("view_key does not belong to the address");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Monero's base58 of a 69-byte address payload, for a test address
    /// made from known scalars.
    fn encode(raw: &[u8]) -> String {
        let mut out = String::new();
        for block in raw.chunks(8) {
            let mut value: u128 = 0;
            for b in block {
                value = (value << 8) | u128::from(*b);
            }
            let mut digits = vec![b'1'; BLOCK_CHARS[block.len()]];
            for d in digits.iter_mut().rev() {
                *d = ALPHABET[(value % 58) as usize];
                value /= 58;
            }
            out.push_str(std::str::from_utf8(&digits).unwrap());
        }
        out
    }

    #[test]
    fn the_view_key_passes_and_the_spend_key_never_leaves() {
        let view = Scalar::from_bytes_mod_order([7u8; 32]);
        let spend = Scalar::from_bytes_mod_order([9u8; 32]);
        let mut raw = vec![18u8];
        raw.extend_from_slice(&EdwardsPoint::mul_base(&spend).compress().to_bytes());
        raw.extend_from_slice(&EdwardsPoint::mul_base(&view).compress().to_bytes());
        raw.extend_from_slice(&[0, 0, 0, 0]);
        let address = encode(&raw);
        assert_eq!(address.len(), 95);
        assert_eq!(check(&address, &hex(&view.to_bytes())), Ok(()));
        assert_eq!(
            check(&address, &hex(&spend.to_bytes())),
            Err("view_key is the spend key; it stays in your wallet")
        );
        let other = Scalar::from_bytes_mod_order([3u8; 32]);
        assert!(check(&address, &hex(&other.to_bytes())).is_err());
        assert!(check(&address, "zz").is_err());
        assert!(check("4abc", &hex(&view.to_bytes())).is_err());
    }
}
