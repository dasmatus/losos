//! `/api/lab/*`: LosOS Lab's guests, relayed to `losos-registrar lab`.
//!
//! With `losos.lab.libvirt.enable` the box runs the Lab's libvirt helper
//! (`backend-registrar/src/lab/`) on a loopback port, and the Lab page asks
//! for its guests here, under the admin token like every other page. lososd
//! holds no libvirt code: it checks the token, forwards the four JSON
//! routes with the same token, and passes the helper's answer back. The
//! WebSockets (a guest's console and network cards, and the raw libvirt
//! relay the page's WebAssembly client uses) do not come through here;
//! nginx proxies `/api/lab/ws/` and `/api/lab/virt` straight to the
//! helper, which admits a socket only with a ticket: the per-guest one a
//! create answered with, or a single-use one from `POST
//! /api/lab/virt-ticket`. Both need the admin token.
//!
//! Off or not running is a 200 `{"available": false, "reason": ...}` on
//! `hello`, never a 404, for the same reason as the market relay: a page
//! latches a 404 as "this box does not serve the route".
//!
//! The hop is loopback and HTTP/1.1 with `Connection: close`, so std's
//! `TcpStream` carries it, as it carries the sign-in probe (`io_backend`).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

/// Largest answer read from the helper. Its answers are a few hundred bytes.
const MAX_ANSWER: usize = 64 * 1024;
/// Largest request body forwarded; the helper refuses more than 8 KiB.
pub const MAX_BODY: usize = 8 * 1024;

/// Where the helper listens: `$LOSOS_LAB_URL`, an `http://` URL on a
/// loopback address and nothing else. `None` when unset or not loopback.
#[must_use]
pub fn helper_addr(url: &str) -> Option<SocketAddr> {
    let rest = url.strip_prefix("http://")?.trim_end_matches('/');
    let addr: SocketAddr = rest.parse().ok()?;
    addr.ip().is_loopback().then_some(addr)
}

/// A guest key as the helper makes them: `<device id>-<6 hex>`.
#[must_use]
pub fn valid_key(key: &str) -> bool {
    (1..=40).contains(&key.len())
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// The request bytes. `path` is one of the fixed routes plus a checked key.
#[must_use]
pub fn request(method: &str, addr: SocketAddr, path: &str, token: &str, body: &[u8]) -> Vec<u8> {
    let mut out = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {token}\r\nAccept: application/json\r\nConnection: close\r\n"
    );
    if !body.is_empty() || method == "POST" {
        out.push_str("Content-Type: application/json\r\n");
        out.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    out.push_str("\r\n");
    let mut bytes = out.into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

/// Status and body of an HTTP/1.1 answer, de-chunked if it was chunked.
#[must_use]
pub fn parse_answer(raw: &[u8]) -> Option<(u16, Vec<u8>)> {
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = std::str::from_utf8(&raw[..split]).ok()?;
    let body = &raw[split + 4..];
    let mut lines = head.split("\r\n");
    let status: u16 = lines.next()?.split(' ').nth(1)?.parse().ok()?;
    let chunked = lines.any(|l| {
        l.split_once(':').is_some_and(|(k, v)| {
            k.trim().eq_ignore_ascii_case("transfer-encoding")
                && v.trim().eq_ignore_ascii_case("chunked")
        })
    });
    if !chunked {
        return Some((status, body.to_vec()));
    }
    let mut out = Vec::new();
    let mut rest = body;
    loop {
        let eol = rest.windows(2).position(|w| w == b"\r\n")?;
        let size_text = std::str::from_utf8(&rest[..eol]).ok()?;
        let size = usize::from_str_radix(size_text.split(';').next()?.trim(), 16).ok()?;
        rest = &rest[eol + 2..];
        if size == 0 {
            return Some((status, out));
        }
        out.extend_from_slice(rest.get(..size)?);
        rest = rest.get(size + 2..)?;
    }
}

/// One request to the helper. `Err` when nothing answers.
pub fn relay(
    addr: SocketAddr,
    method: &str,
    path: &str,
    token: &str,
    body: &[u8],
) -> std::io::Result<(u16, Vec<u8>)> {
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2))?;
    // A create waits for `virsh create`; the helper gives virsh 30 s.
    stream.set_read_timeout(Some(Duration::from_secs(40)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    stream.write_all(&request(method, addr, path, token, body))?;
    let mut raw = Vec::new();
    stream.take(MAX_ANSWER as u64).read_to_end(&mut raw)?;
    parse_answer(&raw).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "the Lab helper's answer was not HTTP",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_loopback_http_urls_name_the_helper() {
        assert_eq!(
            helper_addr("http://127.0.0.1:8095"),
            Some("127.0.0.1:8095".parse().expect("addr"))
        );
        assert!(helper_addr("http://127.0.0.1:8095/").is_some());
        for bad in [
            "",
            "https://127.0.0.1:8095",
            "http://192.168.1.5:8095",
            "http://localhost:8095",
            "http://127.0.0.1:8095/lab",
        ] {
            assert!(helper_addr(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn keys_are_plain() {
        assert!(valid_key("d3-a1b2c3"));
        for bad in ["", "../x", "d3 x", "d3%2f", &"a".repeat(41)] {
            assert!(!valid_key(bad), "{bad}");
        }
    }

    #[test]
    fn a_post_carries_the_token_and_the_body() {
        let addr: SocketAddr = "127.0.0.1:8095".parse().expect("addr");
        let r =
            String::from_utf8(request("POST", addr, "/lab/v1/guests", "abc", b"{}")).expect("utf8");
        assert!(r.starts_with("POST /lab/v1/guests HTTP/1.1\r\nHost: 127.0.0.1:8095\r\n"));
        assert!(r.contains("Authorization: Bearer abc\r\n"));
        assert!(r.contains("Content-Length: 2\r\n"));
        assert!(r.ends_with("\r\n\r\n{}"));
        let g = String::from_utf8(request("GET", addr, "/lab/v1/hello", "abc", b"")).expect("utf8");
        assert!(!g.contains("Content-Length"));
    }

    #[test]
    fn answers_are_read_plain_or_chunked() {
        assert_eq!(
            parse_answer(b"HTTP/1.1 201 Created\r\ncontent-length: 2\r\n\r\n{}"),
            Some((201, b"{}".to_vec()))
        );
        assert_eq!(
            parse_answer(
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\n{\"a\"\r\n3\r\n:1}\r\n0\r\n\r\n"
            ),
            Some((200, b"{\"a\":1}".to_vec()))
        );
        assert_eq!(parse_answer(b"garbage"), None);
        assert_eq!(
            parse_answer(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\n"),
            None
        );
    }

    #[test]
    fn a_real_socket_round_trip() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 1024];
            let n = s.read(&mut buf).expect("read");
            let req = String::from_utf8_lossy(&buf[..n]).into_owned();
            s.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 18\r\n\r\n{\"available\":true}")
                .expect("write");
            req
        });
        let (status, body) = relay(addr, "GET", "/lab/v1/hello", "tok", b"").expect("relay");
        assert_eq!(status, 200);
        assert_eq!(body, b"{\"available\":true}");
        assert!(server
            .join()
            .expect("join")
            .contains("Authorization: Bearer tok"));
    }
}
