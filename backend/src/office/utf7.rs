//! The two encodings the WOPI routes meet: UTF-7 in file-name headers, and
//! percent-encoding for the WOPISrc parameter.
//!
//! MS-WOPI puts file names in HTTP headers (`X-WOPI-SuggestedTarget`,
//! `X-WOPI-RequestedName`) as UTF-7, RFC 2152, because a header is ASCII.
//! coolwsd follows it, so "Zpráva" arrives as `Zpr+AOE-va`.

/// Decode a UTF-7 header value. A value that is not ASCII is taken to be
/// UTF-8 already and returned as it is; one that is not valid UTF-7 is
/// `None`.
pub fn decode(text: &str) -> Option<String> {
    if !text.is_ascii() {
        return Some(text.to_string());
    }
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c != b'+' {
            out.push(c as char);
            i += 1;
            continue;
        }
        i += 1;
        if bytes.get(i) == Some(&b'-') {
            out.push('+');
            i += 1;
            continue;
        }
        // A run of modified base64 (no padding) holding UTF-16BE units.
        let mut bits: u32 = 0;
        let mut nbits = 0;
        let mut units = Vec::new();
        while let Some(v) = bytes.get(i).and_then(|&b| sextet(b)) {
            bits = (bits << 6) | v as u32;
            nbits += 6;
            if nbits >= 16 {
                nbits -= 16;
                units.push((bits >> nbits) as u16);
                bits &= (1 << nbits) - 1;
            }
            i += 1;
        }
        // Leftover bits are padding and must be zero.
        if bits != 0 || units.is_empty() {
            return None;
        }
        out.push_str(&String::from_utf16(&units).ok()?);
        if bytes.get(i) == Some(&b'-') {
            i += 1;
        }
    }
    Some(out)
}

fn sextet(b: u8) -> Option<u8> {
    match b {
        b'A'..=b'Z' => Some(b - b'A'),
        b'a'..=b'z' => Some(b - b'a' + 26),
        b'0'..=b'9' => Some(b - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// JavaScript's `encodeURIComponent`: everything but `A-Z a-z 0-9 - _ . ! ~
/// * ' ( )` as `%XX` of its UTF-8 bytes.
pub fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Undo [`percent_encode`] (and any other `%XX`), for query parameters.
/// `+` stays a `+`: WOPI's tokens and ids never carry spaces.
pub fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc_2152_examples() {
        assert_eq!(
            decode("Hi Mom -+Jjo--!").as_deref(),
            Some("Hi Mom -\u{263A}-!")
        );
        assert_eq!(decode("+ZeVnLIqe-").as_deref(), Some("日本語"));
        assert_eq!(decode("A+ImIDkQ.").as_deref(), Some("A\u{2262}\u{0391}."));
        assert_eq!(decode("1 +- 1").as_deref(), Some("1 + 1"));
    }

    #[test]
    fn names_as_coolwsd_sends_them() {
        assert_eq!(decode("Report.docx").as_deref(), Some("Report.docx"));
        assert_eq!(decode("Zpr+AOE-va.odt").as_deref(), Some("Zpráva.odt"));
        assert_eq!(decode("+2D3eAA-.xlsx").as_deref(), Some("\u{1F600}.xlsx"));
        assert_eq!(decode("Zpráva.odt").as_deref(), Some("Zpráva.odt"));
    }

    #[test]
    fn broken_utf7_is_none() {
        assert_eq!(decode("+"), None);
        assert_eq!(decode("+A-"), None);
        assert_eq!(decode("+2D0-"), None, "a lone surrogate");
    }

    #[test]
    fn percent_round_trip() {
        let src = "http://127.0.0.1:8093/wopi/files/42";
        let enc = percent_encode(src);
        assert_eq!(enc, "http%3A%2F%2F127.0.0.1%3A8093%2Fwopi%2Ffiles%2F42");
        assert_eq!(percent_decode(&enc).as_deref(), Some(src));
        assert_eq!(percent_encode("Zpráva"), "Zpr%C3%A1va");
        assert_eq!(percent_decode("%zz"), None);
        assert_eq!(percent_decode("%4"), None);
    }
}
