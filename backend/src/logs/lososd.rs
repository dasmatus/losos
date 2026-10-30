//! The log reader's question to lososd: who is this, and what do they see
//! in the sidebar (`GET /api/me`, `GET /api/pins`)? One HTTP/1.0 request
//! over loopback with short timeouts, because the answer that matters most
//! is "lososd does not answer", and the page must not wait long for it.

use serde_json::Value;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

/// The most of an answer read.
const MAX_ANSWER: u64 = 1 << 20;

#[derive(Debug, Clone)]
pub struct Lososd {
    pub addr: SocketAddr,
    pub timeout: Duration,
}

/// What lososd said.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// 200 with a JSON body.
    Json(Value),
    /// Any other answer.
    Status(u16),
    /// No answer: not listening, too slow, or not HTTP.
    Down,
}

impl Lososd {
    #[must_use]
    pub fn new(addr: SocketAddr) -> Lososd {
        Lososd {
            addr,
            timeout: Duration::from_secs(3),
        }
    }

    /// `GET path` with `headers`.
    #[must_use]
    pub fn get(&self, path: &str, headers: &[(&str, String)]) -> Answer {
        let Ok(mut stream) = TcpStream::connect_timeout(&self.addr, self.timeout) else {
            return Answer::Down;
        };
        let _ = stream.set_read_timeout(Some(self.timeout));
        let _ = stream.set_write_timeout(Some(self.timeout));
        let mut request =
            format!("GET {path} HTTP/1.0\r\nHost: 127.0.0.1\r\nAccept: application/json\r\n");
        for (name, value) in headers {
            // A value nginx passed on, never a way to add a header.
            let value: String = value.chars().filter(|c| !c.is_control()).collect();
            if !value.is_empty() {
                request.push_str(&format!("{name}: {value}\r\n"));
            }
        }
        request.push_str("\r\n");
        if stream.write_all(request.as_bytes()).is_err() {
            return Answer::Down;
        }
        let mut raw = Vec::new();
        if stream.take(MAX_ANSWER).read_to_end(&mut raw).is_err() {
            return Answer::Down;
        }
        parse(&raw)
    }
}

/// An HTTP/1.x answer.
#[must_use]
pub fn parse(raw: &[u8]) -> Answer {
    let Some(split) = raw.windows(4).position(|w| w == b"\r\n\r\n") else {
        return Answer::Down;
    };
    let head = String::from_utf8_lossy(&raw[..split]);
    let Some(status) = head
        .lines()
        .next()
        .filter(|l| l.starts_with("HTTP/1."))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
    else {
        return Answer::Down;
    };
    if status != 200 {
        return Answer::Status(status);
    }
    serde_json::from_slice(&raw[split + 4..]).map_or(Answer::Status(status), Answer::Json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn answers_are_read() {
        assert_eq!(
            parse(b"HTTP/1.0 200 OK\r\ncontent-type: application/json\r\n\r\n{\"a\":1}"),
            Answer::Json(serde_json::json!({"a": 1}))
        );
        assert_eq!(
            parse(b"HTTP/1.1 401 Unauthorized\r\n\r\n"),
            Answer::Status(401)
        );
        assert_eq!(parse(b"garbage"), Answer::Down);
    }

    #[test]
    fn nobody_listening_is_down() {
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        // The listener is gone by now.
        assert_eq!(Lososd::new(port).get("/api/me", &[]), Answer::Down);
    }

    #[test]
    fn headers_go_along_without_line_breaks() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).unwrap();
            s.write_all(b"HTTP/1.0 200 OK\r\n\r\n{\"name\":\"matus\"}")
                .unwrap();
            String::from_utf8_lossy(&buf[..n]).into_owned()
        });
        let answer = Lososd::new(addr).get(
            "/api/me",
            &[("Remote-User", "matus\r\nX-Losos-As: root".to_string())],
        );
        assert_eq!(answer, Answer::Json(serde_json::json!({"name": "matus"})));
        let request = server.join().unwrap();
        assert!(
            request.contains("Remote-User: matusX-Losos-As: root\r\n"),
            "{request}"
        );
        assert!(!request.contains("\nX-Losos-As"));
    }
}
