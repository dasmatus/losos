//! The `http` import: requests to the manifest's endpoints, made with curl.
//!
//! curl, not an HTTP crate, because lososd already reaches every service
//! this way (it is on the unit's `path`), it speaks Unix sockets and the
//! box's CA bundle, and a body read off its stdout streams for free. The
//! URL is checked against the manifest before curl is started; redirects
//! are off, so an endpoint cannot hand the plugin on to another host.

use super::bindings::losos::plugin::http::Outgoing;
use super::bindings::losos::plugin::types::Error;
use std::io::Read;
use std::process::{Child, ChildStdout, Stdio};
use std::time::{Duration, Instant};

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
/// The most header bytes read before the body.
const MAX_HEAD: usize = 64 * 1024;

/// Where a URL goes, once allowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// What curl is given as the URL.
    pub url: String,
    /// `--unix-socket` for a socket endpoint.
    pub socket: Option<String>,
}

/// Match `url` against the endpoints, or say why not.
pub fn allow(endpoints: &[String], url: &str) -> Result<Target, Error> {
    if url.contains(|c: char| c.is_whitespace() || c.is_control()) {
        return Err(Error::Denied(
            "the URL has a space or a control character".to_string(),
        ));
    }
    for e in endpoints {
        if let Some(sock) = e.strip_prefix("unix:") {
            if let Some(rest) = url.strip_prefix(e.as_str()) {
                if rest.is_empty() || rest.starts_with('/') || rest.starts_with('?') {
                    let path = if rest.is_empty() || rest.starts_with('?') {
                        format!("/{rest}")
                    } else {
                        rest.to_string()
                    };
                    return Ok(Target {
                        url: format!("http://localhost{path}"),
                        socket: Some(sock.to_string()),
                    });
                }
            }
            continue;
        }
        if e == "https://*" {
            if let Some(rest) = url.strip_prefix("https://") {
                let host = rest.split(['/', '?', '#']).next().unwrap_or("");
                if !host.is_empty() && !host.contains('@') {
                    return Ok(Target {
                        url: url.to_string(),
                        socket: None,
                    });
                }
            }
            continue;
        }
        if let Some(rest) = url.strip_prefix(e.as_str()) {
            if rest.is_empty() || rest.starts_with('/') || rest.starts_with('?') {
                return Ok(Target {
                    url: url.to_string(),
                    socket: None,
                });
            }
        }
    }
    Err(Error::Denied(format!(
        "{} is not one of the manifest's http endpoints",
        url.split('?').next().unwrap_or(url)
    )))
}

/// A request on its way: curl started, its head read.
pub struct Exchange {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    child: Child,
    stdout: ChildStdout,
    /// Body bytes read past the head.
    pending: Vec<u8>,
    until: Instant,
}

impl Exchange {
    /// Up to `max` more bytes of body; `None` at the end.
    pub fn read(&mut self, max: usize) -> Result<Option<Vec<u8>>, Error> {
        let max = max.clamp(1, 1024 * 1024);
        if !self.pending.is_empty() {
            let n = max.min(self.pending.len());
            return Ok(Some(self.pending.drain(..n).collect()));
        }
        if Instant::now() >= self.until {
            let _ = self.child.kill();
            return Err(Error::TimedOut);
        }
        let mut buf = vec![0u8; max];
        match self.stdout.read(&mut buf) {
            Ok(0) => {
                let status = self
                    .child
                    .wait()
                    .map_err(|e| Error::Failed(format!("curl: {e}")))?;
                match status.code() {
                    Some(0) => Ok(None),
                    Some(28) => Err(Error::TimedOut),
                    _ => Err(Error::Failed(format!("curl exited {status} mid-body"))),
                }
            }
            Ok(n) => {
                buf.truncate(n);
                Ok(Some(buf))
            }
            Err(e) => Err(Error::Failed(format!("reading the answer: {e}"))),
        }
    }

    /// The rest of the body, at most `max` bytes or an error.
    pub fn read_all(&mut self, max: u64) -> Result<Vec<u8>, Error> {
        let mut out = Vec::new();
        while let Some(chunk) = self.read(256 * 1024)? {
            out.extend_from_slice(&chunk);
            if out.len() as u64 > max {
                let _ = self.child.kill();
                return Err(Error::Failed(format!(
                    "the answer is larger than {max} bytes"
                )));
            }
        }
        Ok(out)
    }
}

impl Drop for Exchange {
    fn drop(&mut self) {
        // An unread body: nobody wants the rest.
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// Split `HTTP/1.1 200 OK\r\nk: v\r\n…` into a status and headers.
pub fn parse_head(head: &str) -> Option<(u16, Vec<(String, String)>)> {
    let mut lines = head.split("\r\n");
    let status = lines.next()?.split_whitespace().nth(1)?.parse().ok()?;
    let headers = lines
        .filter(|l| !l.is_empty())
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    Some((status, headers))
}

/// Start `req` and read its head. Interim `1xx` heads are skipped.
pub fn send(endpoints: &[String], req: &Outgoing, deadline: Instant) -> Result<Exchange, Error> {
    let target = allow(endpoints, &req.url)?;
    let method = req.method.to_ascii_uppercase();
    if !method.bytes().all(|b| b.is_ascii_uppercase()) || method.is_empty() {
        return Err(Error::Denied(format!("{} is not a method", req.method)));
    }
    let timeout = req
        .timeout_ms
        .map_or(DEFAULT_TIMEOUT, |ms| Duration::from_millis(u64::from(ms)));
    let until = (Instant::now() + timeout).min(deadline);
    let secs = until
        .saturating_duration_since(Instant::now())
        .as_secs_f64()
        .max(0.1);

    let mut cmd = std::process::Command::new("curl");
    cmd.args([
        "--silent",
        "--show-error",
        "--dump-header",
        "-",
        "--request",
    ])
    .arg(&method)
    .args(["--max-redirs", "0", "--proto", "=http,https", "--max-time"])
    .arg(format!("{secs:.1}"))
    // curl must not add a body type of its own to a raw body.
    .args(["--header", "Expect:"]);
    if let Some(sock) = &target.socket {
        cmd.arg("--unix-socket").arg(sock);
    }
    for (k, v) in &req.headers {
        if k.is_empty()
            || !k
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || v.contains(['\r', '\n', '\0'])
        {
            return Err(Error::Denied(format!(
                "{k:?} is not a header curl can send"
            )));
        }
        cmd.arg("--header").arg(format!("{k}: {v}"));
    }
    if !req.body.is_empty() {
        cmd.args(["--data-binary", "@-"]);
    }
    cmd.arg("--").arg(&target.url);
    cmd.stdin(if req.body.is_empty() {
        Stdio::null()
    } else {
        Stdio::piped()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| Error::Failed(format!("starting curl: {e}")))?;
    if !req.body.is_empty() {
        let body = req.body.clone();
        let mut stdin = child.stdin.take().expect("piped");
        std::thread::spawn(move || {
            use std::io::Write;
            let _ = stdin.write_all(&body);
        });
    }
    let mut stdout = child.stdout.take().expect("piped");
    let mut stderr = child.stderr.take().expect("piped");

    // Read until a head that is not 1xx is complete.
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..end]).into_owned();
            let rest = buf[end + 4..].to_vec();
            let Some((status, headers)) = parse_head(&head) else {
                let _ = child.kill();
                return Err(Error::Failed("the answer's head is not HTTP".to_string()));
            };
            if (100..200).contains(&status) {
                buf = rest;
                continue;
            }
            return Ok(Exchange {
                status,
                headers,
                child,
                stdout,
                pending: rest,
                until,
            });
        }
        if buf.len() > MAX_HEAD {
            let _ = child.kill();
            return Err(Error::Failed("the answer's head is too long".to_string()));
        }
        match stdout.read(&mut chunk) {
            Ok(0) => {
                let status = child.wait().ok();
                let mut why = String::new();
                let _ = stderr.read_to_string(&mut why);
                return match status.and_then(|s| s.code()) {
                    Some(28) => Err(Error::TimedOut),
                    _ => Err(Error::Failed(format!("{}: {}", target.url, why.trim()))),
                };
            }
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) => return Err(Error::Failed(format!("reading the answer: {e}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eps() -> Vec<String> {
        vec![
            "http://127.0.0.1:11434".to_string(),
            "unix:/run/losos-search/cloud.sock".to_string(),
        ]
    }

    #[test]
    fn urls_are_held_to_the_endpoints() {
        assert_eq!(
            allow(&eps(), "http://127.0.0.1:11434/api/chat").unwrap(),
            Target {
                url: "http://127.0.0.1:11434/api/chat".to_string(),
                socket: None
            }
        );
        assert_eq!(
            allow(&eps(), "unix:/run/losos-search/cloud.sock/search?q=a").unwrap(),
            Target {
                url: "http://localhost/search?q=a".to_string(),
                socket: Some("/run/losos-search/cloud.sock".to_string())
            }
        );
        assert_eq!(
            allow(&eps(), "unix:/run/losos-search/cloud.sock")
                .unwrap()
                .url,
            "http://localhost/"
        );
        for bad in [
            "http://127.0.0.1:114340/x",
            "http://127.0.0.1:11434.evil.example/x",
            "http://127.0.0.1:8082/api/apply",
            "unix:/run/losos-search/cloud.sockX/y",
            "unix:/run/other.sock/",
            "https://example.org/",
            "http://127.0.0.1:11434/a b",
            "http://127.0.0.1:11434/a\r\nX: y",
        ] {
            assert!(matches!(allow(&eps(), bad), Err(Error::Denied(_))), "{bad}");
        }
        let any = vec!["https://*".to_string()];
        assert!(allow(&any, "https://artifacthub.io/api").is_ok());
        assert!(allow(&any, "http://artifacthub.io/api").is_err());
        assert!(allow(&any, "https://user@evil/").is_err());
    }

    #[test]
    fn heads_are_parsed() {
        let (status, headers) =
            parse_head("HTTP/1.1 201 Created\r\nContent-Type: text/plain\r\nX-A:  b ").unwrap();
        assert_eq!(status, 201);
        assert_eq!(
            headers[0],
            ("content-type".to_string(), "text/plain".to_string())
        );
        assert_eq!(headers[1], ("x-a".to_string(), "b".to_string()));
        assert!(parse_head("garbage").is_none());
    }
}
