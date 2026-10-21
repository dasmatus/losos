//! Small helpers the cloud's modules share.

use ring::rand::SecureRandom;

/// `n` random characters from `[A-Za-z0-9]`.
pub fn random_token(n: usize) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let rng = ring::rand::SystemRandom::new();
    let mut out = String::with_capacity(n);
    let mut buf = [0u8; 64];
    while out.len() < n {
        rng.fill(&mut buf).expect("the system's random source");
        for b in buf {
            // 248 is the largest multiple of 62 below 256: no bias.
            if b < 248 && out.len() < n {
                out.push(ALPHABET[(b % 62) as usize] as char);
            }
        }
    }
    out
}

pub fn sha1_hex(bytes: &[u8]) -> String {
    hex(ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, bytes).as_ref())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(ring::digest::digest(&ring::digest::SHA256, bytes).as_ref())
}

pub fn sha512_hex(bytes: &[u8]) -> String {
    hex(ring::digest::digest(&ring::digest::SHA512, bytes).as_ref())
}

pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Compare two secrets without leaking where they differ.
pub fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// `Sun, 06 Nov 1994 08:49:37 GMT`, the date format of `getlastmodified`
/// and `Last-Modified`.
pub fn http_date(secs: i64) -> String {
    httpdate::fmt_http_date(
        std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs.max(0) as u64),
    )
}

/// `1994-11-06T08:49:37Z`, for `creationdate`.
pub fn iso_date(secs: i64) -> String {
    chrono::DateTime::from_timestamp(secs, 0)
        .unwrap_or_default()
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string()
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Escape text for an XML element or attribute.
pub fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => out.push(c),
        }
    }
    out
}

/// Percent-encode a path for an `href`, segment by segment, `/` kept.
pub fn encode_path(path: &str) -> String {
    use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
    const KEEP: &AsciiSet = &NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'~')
        .remove(b'/');
    utf8_percent_encode(path, KEEP).to_string()
}

pub fn decode_path(path: &str) -> Option<String> {
    percent_encoding::percent_decode_str(path)
        .decode_utf8()
        .ok()
        .map(|s| s.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_have_the_asked_length_and_alphabet() {
        let t = random_token(72);
        assert_eq!(t.len(), 72);
        assert!(t.chars().all(|c| c.is_ascii_alphanumeric()));
        assert_ne!(random_token(32), random_token(32));
    }

    #[test]
    fn digests_and_dates() {
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(http_date(784111777), "Sun, 06 Nov 1994 08:49:37 GMT");
        assert_eq!(iso_date(784111777), "1994-11-06T08:49:37Z");
    }

    #[test]
    fn paths_round_trip_through_hrefs() {
        let p = "Photos/Holiday 2026/čaj & co.jpg";
        let e = encode_path(p);
        assert!(!e.contains(' '));
        assert!(e.contains('/'));
        assert_eq!(decode_path(&e).unwrap(), p);
        assert_eq!(xml_escape("a<b&\"c\u{1}"), "a&lt;b&amp;&quot;c");
    }
}
