//! Who a Matrix OpenID token belongs to.
//!
//! The call page asks its homeserver for an OpenID token
//! (`POST /_matrix/client/v3/user/{userId}/openid/request_token`) and hands
//! it to the SFU, which asks the homeserver that issued it who it is for
//! (`GET /_matrix/federation/v1/openid/userinfo`). That is how a Matrix
//! person proves who they are to a service that is not their homeserver,
//! without the service ever seeing their access token.
//!
//! Today only this box's own homeserver is asked, on loopback: chat does
//! not federate, so a token from anywhere else is refused.

use super::admit::{Refusal, UserId};
use serde::Deserialize;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

/// What the browser sends, as the homeserver gave it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OpenIdToken {
    pub access_token: String,
    pub matrix_server_name: String,
}

pub trait Resolve: Send + Sync {
    /// The Matrix id `token` was issued for.
    fn resolve(&self, token: &OpenIdToken) -> Result<UserId, Refusal>;
}

/// Asks the box's own homeserver over plain HTTP on loopback.
pub struct Loopback {
    pub addr: SocketAddr,
    pub server_name: String,
}

const TIMEOUT: Duration = Duration::from_secs(5);
/// The answer is one small JSON object.
const MAX_ANSWER: u64 = 16 * 1024;

impl Resolve for Loopback {
    fn resolve(&self, token: &OpenIdToken) -> Result<UserId, Refusal> {
        if token.matrix_server_name != self.server_name {
            return Err(Refusal::NotLocal);
        }
        if !token_ok(&token.access_token) {
            return Err(Refusal::Unauthenticated);
        }
        let answer = get(
            self.addr,
            &format!(
                "/_matrix/federation/v1/openid/userinfo?access_token={}",
                token.access_token
            ),
        )
        .map_err(|e| {
            tracing::warn!("homeserver at {} did not answer: {e}", self.addr);
            Refusal::Unauthenticated
        })?;
        user_from_answer(&answer, &self.server_name)
    }
}

/// Tokens Dendrite and Synapse hand out are short and URL-safe; anything
/// else would need escaping in the query, so it is refused instead.
fn token_ok(t: &str) -> bool {
    !t.is_empty()
        && t.len() <= 512
        && t.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.~".contains(&b))
}

/// The `sub` of a `200` answer, which must be on the server that answered:
/// a homeserver vouches only for its own people.
fn user_from_answer(answer: &Answer, server_name: &str) -> Result<UserId, Refusal> {
    #[derive(Deserialize)]
    struct UserInfo {
        sub: String,
    }
    if answer.status != 200 {
        return Err(Refusal::Unauthenticated);
    }
    let info: UserInfo =
        serde_json::from_slice(&answer.body).map_err(|_| Refusal::Unauthenticated)?;
    let user = UserId::parse(&info.sub).map_err(|_| Refusal::Unauthenticated)?;
    if user.server() != server_name {
        return Err(Refusal::Unauthenticated);
    }
    Ok(user)
}

#[derive(Debug)]
struct Answer {
    status: u16,
    body: Vec<u8>,
}

/// One HTTP/1.0 GET, so the answer ends when the connection does and needs
/// no chunked decoding.
fn get(addr: SocketAddr, path: &str) -> std::io::Result<Answer> {
    let mut stream = TcpStream::connect_timeout(&addr, TIMEOUT)?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    write!(
        stream,
        "GET {path} HTTP/1.0\r\nHost: {addr}\r\nAccept: application/json\r\n\r\n"
    )?;
    let mut raw = Vec::new();
    stream.take(MAX_ANSWER).read_to_end(&mut raw)?;
    parse_answer(&raw)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "not an HTTP answer"))
}

fn parse_answer(raw: &[u8]) -> Option<Answer> {
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = std::str::from_utf8(&raw[..split]).ok()?;
    let status = head
        .lines()
        .next()?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    Some(Answer {
        status,
        body: raw[split + 4..].to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    fn token(t: &str, server: &str) -> OpenIdToken {
        OpenIdToken {
            access_token: t.into(),
            matrix_server_name: server.into(),
        }
    }

    /// A homeserver that answers one request with `answer` and hands back
    /// the request line it got.
    fn homeserver(answer: &'static str) -> (SocketAddr, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let t = thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).unwrap();
            s.write_all(answer.as_bytes()).unwrap();
            String::from_utf8_lossy(&buf[..n])
                .lines()
                .next()
                .unwrap()
                .to_owned()
        });
        (addr, t)
    }

    #[test]
    fn a_good_token_names_its_person() {
        let (addr, req) = homeserver(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"sub\":\"@eva:b\"}",
        );
        let hs = Loopback {
            addr,
            server_name: "b".into(),
        };
        assert_eq!(
            hs.resolve(&token("tok-1_x", "b")).unwrap().as_str(),
            "@eva:b"
        );
        assert_eq!(
            req.join().unwrap(),
            "GET /_matrix/federation/v1/openid/userinfo?access_token=tok-1_x HTTP/1.0"
        );
    }

    #[test]
    fn a_refused_token_is_unauthenticated() {
        let (addr, _req) =
            homeserver("HTTP/1.1 401 Unauthorized\r\n\r\n{\"errcode\":\"M_UNKNOWN_TOKEN\"}");
        let hs = Loopback {
            addr,
            server_name: "b".into(),
        };
        assert_eq!(hs.resolve(&token("t", "b")), Err(Refusal::Unauthenticated));
    }

    #[test]
    fn the_homeserver_vouches_only_for_its_own_people() {
        let (addr, _req) =
            homeserver("HTTP/1.1 200 OK\r\n\r\n{\"sub\":\"@eva:elsewhere.example\"}");
        let hs = Loopback {
            addr,
            server_name: "b".into(),
        };
        assert_eq!(hs.resolve(&token("t", "b")), Err(Refusal::Unauthenticated));
    }

    #[test]
    fn tokens_from_other_servers_and_odd_tokens_never_leave_the_box() {
        // Nothing listens here: a request would fail differently.
        let hs = Loopback {
            addr: "127.0.0.1:9".parse().unwrap(),
            server_name: "b".into(),
        };
        assert_eq!(hs.resolve(&token("t", "elsewhere")), Err(Refusal::NotLocal));
        for bad in ["", "a&b=c", "a b", "x\r\nHost: evil", &"a".repeat(513)] {
            assert_eq!(
                hs.resolve(&token(bad, "b")),
                Err(Refusal::Unauthenticated),
                "{bad}"
            );
        }
    }

    #[test]
    fn a_silent_homeserver_is_unauthenticated() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let hs = Loopback {
            addr,
            server_name: "b".into(),
        };
        assert_eq!(hs.resolve(&token("t", "b")), Err(Refusal::Unauthenticated));
    }

    #[test]
    fn answers_are_parsed() {
        let a = parse_answer(b"HTTP/1.1 200 OK\r\nA: b\r\n\r\n{}").unwrap();
        assert_eq!((a.status, a.body.as_slice()), (200, &b"{}"[..]));
        assert!(parse_answer(b"garbage").is_none());
        assert!(user_from_answer(
            &Answer {
                status: 200,
                body: b"[]".to_vec()
            },
            "b"
        )
        .is_err());
    }
}
