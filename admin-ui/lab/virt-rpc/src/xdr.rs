//! The slice of XDR (RFC 4506) libvirt's protocol uses: big-endian 32-bit
//! words, 64-bit hypers, strings and variable opaques padded to four bytes,
//! fixed opaques, and `*T` optionals as a boolean word before the value.

use std::fmt;

/// A decode that ran past the end of the buffer or met a value XDR forbids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XdrError {
    /// The buffer ended `needed` bytes early.
    Short { needed: usize },
    /// A length word above the limit the `.x` file declares.
    TooLong { len: u32, max: u32 },
    /// A string that is not UTF-8 (libvirt's are).
    NotUtf8,
    /// A boolean or optional discriminant other than 0 or 1.
    BadBool(u32),
}

impl fmt::Display for XdrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XdrError::Short { needed } => write!(f, "XDR: {needed} more bytes expected"),
            XdrError::TooLong { len, max } => write!(f, "XDR: length {len} over limit {max}"),
            XdrError::NotUtf8 => f.write_str("XDR: string is not UTF-8"),
            XdrError::BadBool(v) => write!(f, "XDR: {v} is not a boolean"),
        }
    }
}

impl std::error::Error for XdrError {}

fn pad(len: usize) -> usize {
    (4 - len % 4) % 4
}

/// Appends XDR values to a byte vector.
#[derive(Debug, Default)]
pub struct Encoder {
    buf: Vec<u8>,
}

impl Encoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }

    pub fn i32(&mut self, v: i32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }

    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }

    /// `opaque x[N]`: the bytes, padded, no length word.
    pub fn fixed_opaque(&mut self, v: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(v);
        self.buf.extend(std::iter::repeat_n(0, pad(v.len())));
        self
    }

    /// `opaque x<>` and `string x<>`: a length word, the bytes, padding.
    pub fn opaque(&mut self, v: &[u8]) -> &mut Self {
        // Callers stay far below 4 GiB: libvirt caps a whole message at 32 MiB.
        let len = u32::try_from(v.len()).unwrap_or(u32::MAX);
        self.u32(len).fixed_opaque(v)
    }

    pub fn string(&mut self, v: &str) -> &mut Self {
        self.opaque(v.as_bytes())
    }

    /// `string *x`: libvirt's `remote_string`.
    pub fn opt_string(&mut self, v: Option<&str>) -> &mut Self {
        match v {
            None => self.u32(0),
            Some(s) => self.u32(1).string(s),
        }
    }
}

/// Reads XDR values from a byte slice.
#[derive(Debug)]
pub struct Decoder<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Decoder<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub fn remaining(&self) -> &'a [u8] {
        &self.buf[self.pos..]
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], XdrError> {
        let have = self.buf.len() - self.pos;
        if have < n {
            return Err(XdrError::Short { needed: n - have });
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    pub fn u32(&mut self) -> Result<u32, XdrError> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn i32(&mut self) -> Result<i32, XdrError> {
        let b = self.take(4)?;
        Ok(i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn u64(&mut self) -> Result<u64, XdrError> {
        let b = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Ok(u64::from_be_bytes(a))
    }

    pub fn bool(&mut self) -> Result<bool, XdrError> {
        match self.u32()? {
            0 => Ok(false),
            1 => Ok(true),
            v => Err(XdrError::BadBool(v)),
        }
    }

    pub fn fixed_opaque(&mut self, n: usize) -> Result<&'a [u8], XdrError> {
        let s = self.take(n)?;
        self.take(pad(n))?;
        Ok(s)
    }

    pub fn opaque(&mut self, max: u32) -> Result<&'a [u8], XdrError> {
        let len = self.u32()?;
        if len > max {
            return Err(XdrError::TooLong { len, max });
        }
        self.fixed_opaque(len as usize)
    }

    pub fn string(&mut self, max: u32) -> Result<String, XdrError> {
        let b = self.opaque(max)?;
        String::from_utf8(b.to_vec()).map_err(|_| XdrError::NotUtf8)
    }

    pub fn opt_string(&mut self, max: u32) -> Result<Option<String>, XdrError> {
        if self.bool()? {
            self.string(max).map(Some)
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_are_padded_to_words() {
        let mut e = Encoder::new();
        e.string("abcde").opt_string(None).opt_string(Some("x"));
        let b = e.into_bytes();
        assert_eq!(
            b,
            [
                0, 0, 0, 5, b'a', b'b', b'c', b'd', b'e', 0, 0, 0, // "abcde"
                0, 0, 0, 0, // NULL
                0, 0, 0, 1, 0, 0, 0, 1, b'x', 0, 0, 0, // &"x"
            ]
        );
        let mut d = Decoder::new(&b);
        assert_eq!(d.string(16).unwrap(), "abcde");
        assert_eq!(d.opt_string(16).unwrap(), None);
        assert_eq!(d.opt_string(16).unwrap().as_deref(), Some("x"));
        assert!(d.remaining().is_empty());
    }

    #[test]
    fn decode_errors_name_the_problem() {
        assert_eq!(
            Decoder::new(&[0, 0]).u32(),
            Err(XdrError::Short { needed: 2 })
        );
        assert_eq!(
            Decoder::new(&[0, 0, 0, 9]).string(4),
            Err(XdrError::TooLong { len: 9, max: 4 })
        );
        assert_eq!(
            Decoder::new(&[0, 0, 0, 2]).bool(),
            Err(XdrError::BadBool(2))
        );
    }

    #[test]
    fn hypers_are_big_endian() {
        let mut e = Encoder::new();
        e.u64(11_001_000);
        let b = e.into_bytes();
        assert_eq!(b, [0, 0, 0, 0, 0, 0xa7, 0xdc, 0xa8]);
        assert_eq!(Decoder::new(&b).u64().unwrap(), 11_001_000);
    }
}
