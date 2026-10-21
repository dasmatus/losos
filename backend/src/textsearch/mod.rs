//! Text inside the owner's files, for the admin pages' palette.
//!
//! Two sources, two methods, and neither runs inside lososd:
//!
//! * LosOS cloud: an SQLite FTS5 index of the owner's Nextcloud files
//!   ([`index`]). `losos-search index` brings it up to date every few
//!   minutes and re-reads only the files whose size or modification time
//!   changed since the last pass. [`extract`] turns one file into text. It
//!   runs as the `nextcloud` user with the files mounted read-only.
//! * LosOS Git: no index at all. `losos-search git` runs `git grep` over the
//!   default branch of each repository when a search comes in ([`git`]), as
//!   the `losos-git` user. A box holds a handful of repositories, and git
//!   reads them faster than a second copy of them could be kept current.
//!
//! Each answers one query per connection on a unix socket systemd listens on
//! (`modules/search.nix`, `Accept=yes`), so no search process stays in memory
//! between searches. lososd is the only process allowed on either socket; it
//! relays `GET /api/cloud/text` and `GET /api/git/text` through [`ask`].
//!
//! The wire is one JSON line each way: [`Request`] in, [`Answer`] or
//! `{"error": "..."}` out.

pub mod extract;
pub mod git;
pub mod index;

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// The longest query the routes take, in characters.
pub const MAX_QUERY_CHARS: usize = 128;
/// Words past this many are ignored: each one narrows, and eight is plenty.
const MAX_TERMS: usize = 8;
/// The most hits one answer carries.
pub const MAX_HITS: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub q: String,
    pub limit: usize,
}

/// A file in LosOS cloud whose text matched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudHit {
    /// From the owner's root in LosOS cloud, `/`-separated, no leading `/`.
    pub path: String,
    /// The text around the match, as alternating runs: even indices are
    /// plain text, odd ones are what matched. Always starts with a plain run,
    /// which may be empty.
    pub snippet: Vec<String>,
}

/// A line on a repository's default branch that matched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHit {
    pub owner: String,
    pub repo: String,
    pub branch: String,
    pub path: String,
    pub line: u64,
    /// Same shape as [`CloudHit::snippet`].
    pub snippet: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer<T> {
    pub hits: Vec<T>,
    /// True while the first pass over the files has not finished, so more
    /// may turn up later.
    #[serde(default)]
    pub partial: bool,
}

/// Check a query from the page: what is left after trimming, or why not.
pub fn validate_query(raw: &str) -> Result<String, String> {
    let q = raw.trim();
    if q.chars().any(char::is_control) {
        return Err("q must not contain control characters".into());
    }
    if q.chars().count() > MAX_QUERY_CHARS {
        return Err(format!("q is longer than {MAX_QUERY_CHARS} characters"));
    }
    if terms(q).is_empty() {
        return Err("q needs at least one letter or digit".into());
    }
    Ok(q.to_string())
}

/// The words a query searches for: split on whitespace, and only those with
/// a letter or digit in them, since punctuation alone matches nothing useful.
pub fn terms(q: &str) -> Vec<String> {
    q.split_whitespace()
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .take(MAX_TERMS)
        .map(|w| w.chars().take(64).collect())
        .collect()
}

/// Split a string marked with STX/ETX around each match (what the index's
/// `snippet()` is asked to write) into alternating runs.
pub fn split_marked(marked: &str) -> Vec<String> {
    let mut runs = vec![String::new()];
    for c in marked.chars() {
        match c {
            '\u{2}' if runs.len() % 2 == 1 => runs.push(String::new()),
            '\u{3}' if runs.len() % 2 == 0 => runs.push(String::new()),
            '\u{2}' | '\u{3}' => {}
            c => runs.last_mut().expect("never empty").push(c),
        }
    }
    if runs.len() > 1 && runs.last().is_some_and(String::is_empty) {
        runs.pop();
    }
    runs
}

/// Mark every case-insensitive occurrence of `terms` in `text`, as
/// alternating runs, with the text cut to about `width` characters around
/// the first match.
pub fn mark(text: &str, terms: &[String], width: usize) -> Vec<String> {
    let text = text.trim();
    let lower: Vec<(usize, char)> = text
        .char_indices()
        .map(|(i, c)| (i, c.to_lowercase().next().unwrap_or(c)))
        .collect();
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for term in terms {
        let needle: Vec<char> = term
            .chars()
            .map(|c| c.to_lowercase().next().unwrap_or(c))
            .collect();
        if needle.is_empty() || needle.len() > lower.len() {
            continue;
        }
        for start in 0..=lower.len() - needle.len() {
            if lower[start..start + needle.len()]
                .iter()
                .map(|(_, c)| *c)
                .eq(needle.iter().copied())
            {
                ranges.push((start, start + needle.len()));
            }
        }
    }
    ranges.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (s, e) in ranges {
        match merged.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => merged.push((s, e)),
        }
    }
    // The window, in characters.
    let total = lower.len();
    let first = merged.first().map_or(0, |r| r.0);
    let from = if total <= width {
        0
    } else {
        first.saturating_sub(width / 4).min(total - width)
    };
    let to = (from + width).min(total);
    let byte = |ci: usize| lower.get(ci).map_or(text.len(), |(b, _)| *b);

    let mut runs = vec![String::new()];
    if from > 0 {
        runs[0].push('…');
    }
    let mut at = from;
    for (s, e) in merged {
        let (s, e) = (s.max(from), e.min(to));
        if s >= e {
            continue;
        }
        runs.last_mut()
            .expect("never empty")
            .push_str(&text[byte(at)..byte(s)]);
        runs.push(text[byte(s)..byte(e)].to_string());
        runs.push(String::new());
        at = e;
    }
    runs.last_mut()
        .expect("never empty")
        .push_str(&text[byte(at)..byte(to)]);
    if to < total {
        runs.last_mut().expect("never empty").push('…');
    }
    if runs.len() > 1 && runs.last().is_some_and(String::is_empty) {
        runs.pop();
    }
    runs
}

/// Run a command with nothing on stdin, at most `max_bytes` of its stdout
/// kept, and killed at `deadline`. `None` when it ran out of time.
pub fn run_capped(
    cmd: &mut Command,
    max_bytes: usize,
    deadline: Instant,
) -> std::io::Result<Option<(ExitStatus, Vec<u8>)>> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let out = child.stdout.take().expect("stdout is piped");
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out.take(max_bytes as u64).read_to_end(&mut buf);
        buf
    });
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some((status, reader.join().unwrap_or_default())));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Serve one query: read a [`Request`] line from `input`, answer on `output`.
/// What the two `*-query` subcommands run on the socket systemd hands them.
pub fn serve_one<T: Serialize>(
    input: impl Read,
    mut output: impl Write,
    answer: impl FnOnce(&[String], usize) -> anyhow::Result<Answer<T>>,
) -> anyhow::Result<()> {
    let mut line = String::new();
    BufReader::new(input.take(4096)).read_line(&mut line)?;
    let reply = match serde_json::from_str::<Request>(&line)
        .map_err(|e| e.to_string())
        .and_then(|r| validate_query(&r.q).map(|q| (q, r.limit)))
    {
        Ok((q, limit)) => match answer(&terms(&q), limit.clamp(1, MAX_HITS)) {
            Ok(a) => serde_json::to_value(a)?,
            Err(e) => serde_json::json!({ "error": format!("{e:#}") }),
        },
        Err(e) => serde_json::json!({ "error": e }),
    };
    serde_json::to_writer(&mut output, &reply)?;
    output.write_all(b"\n")?;
    output.flush()?;
    Ok(())
}

/// Why lososd could not get an answer from a search socket.
#[derive(Debug, thiserror::Error)]
pub enum AskError {
    /// Nothing listens there: the units are not on this box, or not up yet.
    #[error("nothing answers on {0}")]
    NotUp(String),
    #[error("{0}")]
    Failed(String),
}

/// Ask the search behind `socket`, waiting at most `timeout`.
pub fn ask<T: serde::de::DeserializeOwned>(
    socket: &Path,
    q: &str,
    limit: usize,
    timeout: Duration,
) -> Result<Answer<T>, AskError> {
    use std::os::unix::net::UnixStream;
    let shown = socket.display().to_string();
    let mut stream = UnixStream::connect(socket).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused => {
            AskError::NotUp(shown.clone())
        }
        _ => AskError::Failed(format!("connecting to {shown}: {e}")),
    })?;
    let fail =
        |what: &str, e: &dyn std::fmt::Display| AskError::Failed(format!("{what} {shown}: {e}"));
    stream
        .set_read_timeout(Some(timeout))
        .and_then(|()| stream.set_write_timeout(Some(timeout)))
        .map_err(|e| fail("setting timeouts on", &e))?;
    let mut request = serde_json::to_vec(&Request {
        q: q.to_string(),
        limit,
    })
    .map_err(|e| fail("encoding a query for", &e))?;
    request.push(b'\n');
    stream
        .write_all(&request)
        .map_err(|e| fail("writing to", &e))?;
    let mut line = String::new();
    BufReader::new(stream.take(4 << 20))
        .read_line(&mut line)
        .map_err(|e| fail("reading from", &e))?;
    let value: serde_json::Value =
        serde_json::from_str(&line).map_err(|e| fail("decoding the answer from", &e))?;
    if let Some(error) = value.get("error").and_then(|e| e.as_str()) {
        return Err(AskError::Failed(format!("{shown}: {error}")));
    }
    serde_json::from_value(value).map_err(|e| fail("decoding the answer from", &e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_string()).collect()
    }

    #[test]
    fn terms_keep_words_with_letters_or_digits() {
        assert_eq!(terms("  tax  -- 2025 "), s(&["tax", "2025"]));
        assert!(terms("-- ..").is_empty());
    }

    #[test]
    fn queries_are_checked() {
        assert_eq!(validate_query(" tax ").as_deref(), Ok("tax"));
        assert!(validate_query("--").is_err());
        assert!(validate_query("a\u{7}b").is_err());
        assert!(validate_query(&"x".repeat(129)).is_err());
    }

    #[test]
    fn marked_snippets_split_into_runs() {
        assert_eq!(
            split_marked("pay the \u{2}tax\u{3} by May"),
            s(&["pay the ", "tax", " by May"])
        );
        assert_eq!(split_marked("\u{2}Tax\u{3}"), s(&["", "Tax"]));
        assert_eq!(split_marked("no match"), s(&["no match"]));
    }

    #[test]
    fn mark_finds_every_term_without_case() {
        assert_eq!(
            mark("let TAX = tax_rate * 2;", &s(&["tax"]), 80),
            s(&["let ", "TAX", " = ", "tax", "_rate * 2;"])
        );
        assert_eq!(
            mark("Daň z príjmu", &s(&["daň", "príj"]), 80),
            s(&["", "Daň", " z ", "príj", "mu"])
        );
    }

    #[test]
    fn mark_cuts_long_lines_around_the_match() {
        let line = format!("{}needle{}", "a".repeat(200), "b".repeat(200));
        let runs = mark(&line, &s(&["needle"]), 40);
        assert_eq!(runs.len(), 3);
        assert!(runs[0].starts_with('…'));
        assert_eq!(runs[1], "needle");
        assert!(runs[2].ends_with('…'));
        assert!(runs.concat().chars().count() <= 42);
    }

    #[test]
    fn capped_commands_stop_at_the_deadline() {
        let deadline = Instant::now() + Duration::from_millis(100);
        let ran = run_capped(Command::new("sleep").arg("5"), 10, deadline).unwrap();
        assert!(ran.is_none());
        let (status, out) = run_capped(
            Command::new("printf").arg("abcdef"),
            3,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap()
        .unwrap();
        assert!(status.success());
        assert_eq!(out, b"abc");
    }

    #[test]
    fn one_query_is_served_and_asked() {
        let mut out = Vec::new();
        serve_one(
            &b"{\"q\":\" tax  rate \",\"limit\":99}\n"[..],
            &mut out,
            |t, limit| {
                assert_eq!(t, ["tax", "rate"]);
                assert_eq!(limit, MAX_HITS);
                Ok(Answer {
                    hits: vec![CloudHit {
                        path: "a.txt".into(),
                        snippet: s(&["", "tax"]),
                    }],
                    partial: true,
                })
            },
        )
        .unwrap();
        let back: Answer<CloudHit> = serde_json::from_slice(&out).unwrap();
        assert!(back.partial);
        assert_eq!(back.hits[0].path, "a.txt");

        let mut out = Vec::new();
        serve_one(
            &b"{\"q\":\"--\",\"limit\":1}\n"[..],
            &mut out,
            |_, _| -> anyhow::Result<Answer<CloudHit>> {
                unreachable!("an invalid query is not searched")
            },
        )
        .unwrap();
        assert!(String::from_utf8(out).unwrap().contains("\"error\""));
    }
}
