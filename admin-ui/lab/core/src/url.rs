//! The part of the WHATWG URL parser the Lab leans on (`new URL(x)` with no
//! base): scheme, host name, path and query. Hosts that are not ASCII are
//! refused rather than turned into punycode.

#[derive(Clone, Debug, PartialEq)]
pub struct Url {
    /// "http:", with the colon, like `URL.protocol`.
    pub protocol: String,
    pub hostname: String,
    pub pathname: String,
    /// "?a=1", or "" when there is no query.
    pub search: String,
}

const SPECIAL: &[&str] = &["http", "https", "ws", "wss", "ftp", "file"];

fn encode(out: &mut String, c: char, set: impl Fn(char) -> bool) {
    if set(c) {
        let mut buf = [0u8; 4];
        for b in c.encode_utf8(&mut buf).bytes() {
            out.push_str(&format!("%{b:02X}"));
        }
    } else {
        out.push(c);
    }
}

fn path_set(c: char) -> bool {
    (c as u32) < 0x20
        || (c as u32) > 0x7e
        || matches!(c, ' ' | '"' | '#' | '<' | '>' | '?' | '`' | '{' | '}')
}

fn query_set(special: bool) -> impl Fn(char) -> bool {
    move |c: char| {
        (c as u32) < 0x20
            || (c as u32) > 0x7e
            || matches!(c, ' ' | '"' | '#' | '<' | '>')
            || (special && c == '\'')
    }
}

fn parse_ipv4_number(s: &str) -> Option<u64> {
    if s.is_empty() {
        return None;
    }
    let (digits, radix) = if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        (h, 16)
    } else if s.len() > 1 && s.starts_with('0') {
        (&s[1..], 8)
    } else {
        (s, 10)
    };
    if digits.is_empty() {
        return Some(0);
    }
    if !digits.chars().all(|c| c.is_digit(radix)) {
        return None;
    }
    // anything this long is out of range anyway
    if digits.len() > 20 {
        return Some(u64::MAX);
    }
    u64::from_str_radix(digits, radix).ok().or(Some(u64::MAX))
}

fn ends_in_number(host: &str) -> bool {
    let mut parts: Vec<&str> = host.split('.').collect();
    if parts.last() == Some(&"") {
        if parts.len() == 1 {
            return false;
        }
        parts.pop();
    }
    let last = parts.last().copied().unwrap_or("");
    if !last.is_empty() && last.bytes().all(|b| b.is_ascii_digit()) {
        return true;
    }
    parse_ipv4_number(last).is_some()
}

fn parse_ipv4(host: &str) -> Option<String> {
    let mut parts: Vec<&str> = host.split('.').collect();
    if parts.last() == Some(&"") && parts.len() > 1 {
        parts.pop();
    }
    if parts.len() > 4 {
        return None;
    }
    let nums: Vec<u64> = parts
        .iter()
        .map(|p| parse_ipv4_number(p))
        .collect::<Option<_>>()?;
    let (last, rest) = nums.split_last()?;
    if rest.iter().any(|n| *n > 255) {
        return None;
    }
    if *last >= 256u64.pow(5 - nums.len() as u32) {
        return None;
    }
    let mut v = *last;
    for (i, n) in rest.iter().enumerate() {
        v += n * 256u64.pow(3 - i as u32);
    }
    Some(format!(
        "{}.{}.{}.{}",
        (v >> 24) & 255,
        (v >> 16) & 255,
        (v >> 8) & 255,
        v & 255
    ))
}

fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && b[i + 1].is_ascii_hexdigit()
            && b[i + 2].is_ascii_hexdigit()
        {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).ok()
}

fn parse_host(raw: &str, special: bool) -> Option<String> {
    if raw.starts_with('[') {
        if !raw.ends_with(']') || raw.len() < 3 {
            return None;
        }
        let inner = &raw[1..raw.len() - 1];
        if !inner
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.')
        {
            return None;
        }
        return Some(raw.to_ascii_lowercase());
    }
    if !special {
        let bad = |c: char| {
            matches!(
                c,
                '\0' | '\t'
                    | '\n'
                    | '\r'
                    | ' '
                    | '#'
                    | '/'
                    | ':'
                    | '<'
                    | '>'
                    | '?'
                    | '@'
                    | '['
                    | '\\'
                    | ']'
                    | '^'
                    | '|'
            )
        };
        if raw.chars().any(bad) {
            return None;
        }
        let mut out = String::new();
        for c in raw.chars() {
            encode(&mut out, c, |c| (c as u32) < 0x20 || (c as u32) > 0x7e);
        }
        return Some(out);
    }
    let host = percent_decode(raw)?;
    if host.is_empty() || !host.is_ascii() {
        return None;
    }
    let host = host.to_ascii_lowercase();
    let forbidden = |c: char| {
        (c as u32) <= 0x20
            || c as u32 == 0x7f
            || matches!(
                c,
                '#' | '%' | '/' | ':' | '<' | '>' | '?' | '@' | '[' | '\\' | ']' | '^' | '|'
            )
    };
    if host.chars().any(forbidden) {
        return None;
    }
    if ends_in_number(&host) {
        return parse_ipv4(&host);
    }
    Some(host)
}

pub fn parse(input: &str) -> Option<Url> {
    let trimmed = input.trim_matches(|c: char| (c as u32) <= 0x20);
    let s: String = trimmed
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect();
    let colon = s.find(':')?;
    let scheme = &s[..colon];
    let mut sc = scheme.chars();
    if !sc.next().is_some_and(|c| c.is_ascii_alphabetic())
        || !sc.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    {
        return None;
    }
    let scheme = scheme.to_ascii_lowercase();
    let special = SPECIAL.contains(&scheme.as_str());
    let mut rest = &s[colon + 1..];
    if special {
        rest = rest.trim_start_matches(['/', '\\']);
    } else if let Some(r) = rest.strip_prefix("//") {
        rest = r;
    } else {
        // an opaque path: no host at all
        return Some(Url {
            protocol: format!("{scheme}:"),
            hostname: String::new(),
            pathname: rest.split(['?', '#']).next().unwrap_or("").to_string(),
            search: String::new(),
        });
    }
    let is_end = |c: char| c == '/' || c == '?' || c == '#' || (special && c == '\\');
    let auth_end = rest.find(is_end).unwrap_or(rest.len());
    let authority = &rest[..auth_end];
    let after = &rest[auth_end..];
    let hostport = match authority.rfind('@') {
        Some(i) => &authority[i + 1..],
        None => authority,
    };
    let (host_raw, port) = if hostport.starts_with('[') {
        let i = hostport.find(']')?;
        let (h, p) = hostport.split_at(i + 1);
        (h, p.strip_prefix(':'))
    } else {
        match hostport.rfind(':') {
            Some(i) => (&hostport[..i], Some(&hostport[i + 1..])),
            None => (hostport, None),
        }
    };
    if let Some(p) = port {
        if !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        if !p.is_empty() && p.trim_start_matches('0').len() > 5 {
            return None;
        }
        if !p.is_empty() && p.parse::<u32>().map_or(true, |n| n > 65535) {
            return None;
        }
    }
    if host_raw.is_empty() {
        if special && scheme != "file" {
            return None;
        }
        if port.is_some_and(|p| !p.is_empty()) {
            return None;
        }
    }
    let hostname = if host_raw.is_empty() {
        String::new()
    } else {
        parse_host(host_raw, special)?
    };
    let (path_part, query) = {
        let no_frag = after.split('#').next().unwrap_or("");
        match no_frag.find('?') {
            Some(i) => (&no_frag[..i], Some(&no_frag[i + 1..])),
            None => (no_frag, None),
        }
    };
    let mut segments: Vec<String> = Vec::new();
    let mut last_was_dir = false;
    if !path_part.is_empty() {
        let body = &path_part[1..];
        let pieces: Vec<&str> = if special {
            body.split(['/', '\\']).collect()
        } else {
            body.split('/').collect()
        };
        let n = pieces.len();
        for (i, seg) in pieces.into_iter().enumerate() {
            let lower = seg.to_ascii_lowercase();
            let is_last = i + 1 == n;
            if matches!(lower.as_str(), ".." | ".%2e" | "%2e." | "%2e%2e") {
                segments.pop();
                last_was_dir = is_last;
            } else if matches!(lower.as_str(), "." | "%2e") {
                last_was_dir = is_last;
            } else {
                let mut enc = String::new();
                for c in seg.chars() {
                    encode(&mut enc, c, path_set);
                }
                segments.push(enc);
                last_was_dir = false;
            }
        }
        if last_was_dir {
            segments.push(String::new());
        }
    }
    let pathname = if segments.is_empty() {
        if special || !path_part.is_empty() {
            "/".to_string()
        } else {
            String::new()
        }
    } else {
        format!("/{}", segments.join("/"))
    };
    let search = match query {
        Some(q) if !q.is_empty() => {
            let mut enc = String::from("?");
            for c in q.chars() {
                encode(&mut enc, c, query_set(special));
            }
            enc
        }
        _ => String::new(),
    };
    Some(Url {
        protocol: format!("{scheme}:"),
        hostname,
        pathname,
        search,
    })
}

#[cfg(test)]
mod tests {
    use super::parse;

    fn host(s: &str) -> Option<String> {
        parse(s).map(|u| u.hostname)
    }

    #[test]
    fn hosts_like_browsers() {
        assert_eq!(
            host("http://MattBox.LOCAL:8443/x").as_deref(),
            Some("mattbox.local")
        );
        assert_eq!(
            host("http://192.168.001.1/").as_deref(),
            Some("192.168.1.1")
        );
        assert_eq!(host("http://0x7f.1/").as_deref(), Some("127.0.0.1"));
        assert_eq!(host("http://a b/"), None);
        assert_eq!(host("http://"), None);
        assert_eq!(host("http://x:99999/"), None);
        assert_eq!(host("not a url"), None);
        assert_eq!(host("1http://x"), None);
        assert_eq!(host("http://user:pw@host/").as_deref(), Some("host"));
    }

    #[test]
    fn paths_and_queries() {
        let u = parse("HTTP://h/a/../b c?x=1 2#f").unwrap_or_else(|| panic!("parses"));
        assert_eq!(u.protocol, "http:");
        assert_eq!(u.pathname, "/b%20c");
        assert_eq!(u.search, "?x=1%202");
        assert_eq!(parse("http://h").map(|u| u.pathname).as_deref(), Some("/"));
        assert_eq!(parse("http://h?").map(|u| u.search).as_deref(), Some(""));
    }
}
