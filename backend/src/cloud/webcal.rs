//! Reading a subscribed calendar's feed (`webcal://`, `https://`).
//!
//! LosOS Cloud has no network on a box (`PrivateNetwork=` in
//! `modules/cloud.nix`), so it never dials a feed itself. A fetcher does:
//! `losos-cloud webcal-fetch`, one process per feed read, started by a
//! socket systemd holds (`losos-webcal.socket`, `Accept=yes`). The fetcher
//! runs under its own sandbox with the network but no files, and systemd's
//! address filter keeps it off loopback and link-local, so a feed address
//! cannot reach the box's own services whatever its name resolves to. It
//! reads one request line from the socket and answers with one header line
//! and the body ([`Request`], [`Answer`]); [`fetch`] is the cloud's side.
//!
//! The fetch itself is curl ([`curl`]): http and https only, redirects
//! too, at most [`MAX_BYTES`] and [`MAX_SECS`]. `webcal://` is https first
//! and plain http when nothing answers on https, which is what the scheme
//! has always meant.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, Read, Write};
use std::path::Path;

/// The largest feed read, in bytes.
pub const MAX_BYTES: usize = 10 * 1024 * 1024;
/// How long one read may take, in seconds.
pub const MAX_SECS: u64 = 30;
/// The longest feed address kept.
pub const MAX_URL: usize = 2048;

/// What the cloud asks the fetcher.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Request {
    pub url: String,
    /// From the last read, so an unchanged feed answers 304.
    pub etag: String,
    pub last_modified: String,
}

/// What came back.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Answer {
    /// The HTTP status of the last response; 0 when there was none.
    pub status: u16,
    pub etag: String,
    pub last_modified: String,
    /// Why there was no response, in words.
    pub error: String,
    #[serde(skip)]
    pub body: Vec<u8>,
}

/// A feed address as it is kept: `webcal://` and `webcals://` as given,
/// http and https otherwise, no user name or password in it, no spaces.
pub fn normalize(raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if raw.len() > MAX_URL {
        return Err("the address is too long".into());
    }
    let lower = raw.to_ascii_lowercase();
    let rest = ["webcal://", "webcals://", "https://", "http://"]
        .iter()
        .find_map(|p| lower.starts_with(p).then(|| &raw[p.len()..]))
        .ok_or("a calendar address starts with webcal://, https:// or http://")?;
    let scheme = &lower[..lower.len() - rest.len()];
    let parsed = url::Url::parse(&format!("https://{rest}"))
        .map_err(|_| "that is not a web address".to_string())?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("put no user name or password in the address".into());
    }
    if parsed.host_str().is_none_or(str::is_empty) {
        return Err("the address names no server".into());
    }
    let tail = &parsed.as_str()["https://".len()..];
    Ok(format!("{scheme}{tail}"))
}

/// The addresses a feed address is tried at, in order.
fn attempts(url: &str) -> Vec<String> {
    let lower = url.to_ascii_lowercase();
    if let Some(rest) = lower
        .strip_prefix("webcal://")
        .map(|_| &url["webcal://".len()..])
    {
        vec![format!("https://{rest}"), format!("http://{rest}")]
    } else if lower.starts_with("webcals://") {
        vec![format!("https://{}", &url["webcals://".len()..])]
    } else {
        vec![url.to_owned()]
    }
}

const MARK: &str = "\n<<losos-webcal>>";

/// Read a feed with curl. `curl` is its path (tests point it elsewhere).
pub fn curl(curl: &str, req: &Request) -> Answer {
    let url = match normalize(&req.url) {
        Ok(u) => u,
        Err(e) => {
            return Answer {
                error: e,
                ..Answer::default()
            }
        }
    };
    let mut last = Answer::default();
    for address in attempts(&url) {
        let mut cmd = std::process::Command::new(curl);
        cmd.args([
            "--silent",
            "--show-error",
            "--proto",
            "=http,https",
            "--proto-redir",
            "=http,https",
            "--location",
            "--max-redirs",
            "5",
            "--compressed",
            "--connect-timeout",
            "10",
            "--max-time",
            &MAX_SECS.to_string(),
            "--max-filesize",
            &MAX_BYTES.to_string(),
            "--user-agent",
            "LosOS Calendar",
            "--header",
            "Accept: text/calendar, text/plain;q=0.5, */*;q=0.1",
            "--write-out",
            &format!("{MARK}%{{http_code}}\t%header{{etag}}\t%header{{last-modified}}"),
        ]);
        if !req.etag.is_empty() && !req.etag.contains(['\r', '\n']) {
            cmd.args(["--header", &format!("If-None-Match: {}", req.etag)]);
        }
        if !req.last_modified.is_empty() && !req.last_modified.contains(['\r', '\n']) {
            cmd.args([
                "--header",
                &format!("If-Modified-Since: {}", req.last_modified),
            ]);
        }
        cmd.arg("--").arg(&address);
        cmd.stdin(std::process::Stdio::null());
        let out = match cmd.output() {
            Ok(o) => o,
            Err(e) => {
                return Answer {
                    error: format!("could not run curl: {e}"),
                    ..Answer::default()
                }
            }
        };
        let text = out.stdout;
        let at = text.windows(MARK.len()).rposition(|w| w == MARK.as_bytes());
        let (body, tail) = match at {
            Some(i) => (
                text[..i].to_vec(),
                String::from_utf8_lossy(&text[i + MARK.len()..]).into_owned(),
            ),
            None => (Vec::new(), String::new()),
        };
        let mut parts = tail.split('\t');
        let status: u16 = parts
            .next()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        let etag = parts
            .next()
            .unwrap_or("")
            .trim()
            .chars()
            .take(200)
            .collect();
        let last_modified = parts
            .next()
            .unwrap_or("")
            .trim()
            .chars()
            .take(100)
            .collect();
        let code = out.status.code().unwrap_or(-1);
        last = Answer {
            status,
            etag,
            last_modified,
            error: if code == 0 {
                String::new()
            } else {
                curl_error(code, &String::from_utf8_lossy(&out.stderr))
            },
            body: if code == 0 { body } else { Vec::new() },
        };
        // Only "nothing answered" moves on to plain http.
        if code != 7 {
            break;
        }
    }
    last
}

fn curl_error(code: i32, stderr: &str) -> String {
    // The box's address filter (loopback, link-local) refuses the
    // connect itself.
    if stderr.contains("Permission denied") || stderr.contains("not permitted") {
        return "the box does not read calendars from that address".into();
    }
    match code {
        6 => "the server's name was not found".into(),
        7 => "nothing answered at that address".into(),
        28 => "the server took too long to answer".into(),
        35 | 60 => "the server's certificate could not be checked".into(),
        47 => "the address redirects too many times".into(),
        63 => "the calendar is larger than 10 MB".into(),
        1 => "that kind of address cannot be read".into(),
        _ => {
            let line = stderr.lines().last().unwrap_or("").trim();
            let line = line.strip_prefix("curl: ").unwrap_or(line);
            let line = line.split_once(") ").map(|(_, r)| r).unwrap_or(line);
            if line.is_empty() {
                format!("the calendar could not be read (curl {code})")
            } else {
                line.chars().take(200).collect()
            }
        }
    }
}

/// The fetcher's side: one request line in, an answer out, then exit.
pub fn serve_one(curl_path: &str, input: impl Read, mut output: impl Write) -> std::io::Result<()> {
    let mut line = String::new();
    std::io::BufReader::new(input.take(MAX_URL as u64 * 2 + 4096)).read_line(&mut line)?;
    let answer = match serde_json::from_str::<Request>(line.trim()) {
        Ok(req) => curl(curl_path, &req),
        Err(_) => Answer {
            error: "the request was not understood".into(),
            ..Answer::default()
        },
    };
    let head = serde_json::to_string(&answer).unwrap_or_else(|_| "{}".into());
    output.write_all(head.as_bytes())?;
    output.write_all(b"\n")?;
    output.write_all(&answer.body)?;
    output.flush()
}

/// The cloud's side: ask the fetcher at `socket`, or, with none (a
/// developer's machine), run curl here.
pub fn fetch(socket: Option<&Path>, req: &Request) -> Answer {
    let Some(socket) = socket else {
        return curl("curl", req);
    };
    let attempt = || -> std::io::Result<Answer> {
        let mut s = std::os::unix::net::UnixStream::connect(socket)?;
        s.set_read_timeout(Some(std::time::Duration::from_secs(MAX_SECS * 2 + 10)))?;
        let mut line = serde_json::to_string(req).unwrap_or_default();
        line.push('\n');
        s.write_all(line.as_bytes())?;
        let mut raw = Vec::new();
        s.take((MAX_BYTES + 64 * 1024) as u64)
            .read_to_end(&mut raw)?;
        let nl = raw.iter().position(|b| *b == b'\n').unwrap_or(raw.len());
        let mut answer: Answer = serde_json::from_slice(&raw[..nl]).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "a reply that is not JSON")
        })?;
        answer.body = raw.get(nl + 1..).unwrap_or_default().to_vec();
        Ok(answer)
    };
    attempt().unwrap_or_else(|e| Answer {
        error: format!("the box's calendar reader did not answer: {e}"),
        ..Answer::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_kept_as_given_without_secrets() {
        assert_eq!(
            normalize(" webcal://feeds.example/tax.ics ").unwrap(),
            "webcal://feeds.example/tax.ics"
        );
        assert_eq!(
            normalize("https://x.edupage.org/timetable/ics?id=1&k=a b").unwrap(),
            "https://x.edupage.org/timetable/ics?id=1&k=a%20b"
        );
        assert!(normalize("ftp://feeds.example/a.ics").is_err());
        assert!(normalize("file:///etc/passwd").is_err());
        assert!(normalize("https://me:secret@feeds.example/a.ics").is_err());
        assert!(normalize("https://").is_err());
        assert_eq!(
            attempts("webcal://feeds.example/a.ics"),
            vec!["https://feeds.example/a.ics", "http://feeds.example/a.ics"]
        );
        assert_eq!(
            attempts("webcals://f.example/a"),
            vec!["https://f.example/a"]
        );
        assert_eq!(attempts("http://f.example/a"), vec!["http://f.example/a"]);
    }

    /// A stand-in for curl that prints a body and the write-out line, so
    /// the parsing is tested without a network.
    fn fake_curl(dir: &Path, body: &str, tail: &str, code: i32) -> String {
        let p = dir.join("curl");
        std::fs::write(
            &p,
            format!(
                "#!/bin/sh\nprintf '%s' '{body}'\nprintf '\\n<<losos-webcal>>{tail}'\nexit {code}\n"
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn a_read_comes_back_with_its_status_and_validators() {
        let dir = tempfile::tempdir().unwrap();
        let c = fake_curl(
            dir.path(),
            "BEGIN:VCALENDAR",
            "200\t\"v1\"\tWed, 21 Oct 2026 07:28:00 GMT",
            0,
        );
        let a = curl(
            &c,
            &Request {
                url: "https://f.example/a.ics".into(),
                ..Default::default()
            },
        );
        assert_eq!(a.status, 200);
        assert_eq!(a.etag, "\"v1\"");
        assert_eq!(a.last_modified, "Wed, 21 Oct 2026 07:28:00 GMT");
        assert_eq!(a.body, b"BEGIN:VCALENDAR");
        assert!(a.error.is_empty());
        // Through the fetcher's own protocol.
        let mut out = Vec::new();
        serve_one(
            &c,
            &b"{\"url\":\"webcal://f.example/a.ics\"}\n"[..],
            &mut out,
        )
        .unwrap();
        let nl = out.iter().position(|b| *b == b'\n').unwrap();
        let head: Answer = serde_json::from_slice(&out[..nl]).unwrap();
        assert_eq!(head.status, 200);
        assert_eq!(&out[nl + 1..], b"BEGIN:VCALENDAR");
        let c = fake_curl(dir.path(), "", "000\t\t", 6);
        let a = curl(
            &c,
            &Request {
                url: "https://nowhere.example/".into(),
                ..Default::default()
            },
        );
        assert_eq!(a.error, "the server's name was not found");
        assert!(a.body.is_empty());
    }
}
