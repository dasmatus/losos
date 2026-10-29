//! What the log reader hides before a line leaves the box's journal.
//!
//! A service that logs a secret by mistake must not hand it to whoever
//! reads Settings › Logs, downloads the lines for a bug report, or finds
//! them attached to a problem report. Every line goes through [`Redactor`]
//! before it is filtered, drawn or written out, so a search cannot find a
//! secret by its letters either. It errs towards hiding: a long run of hex
//! that was only a checksum is hidden too.

use regex::{Captures, Regex};
use std::sync::LazyLock;

/// What a hidden value is shown as.
pub const HIDDEN: &str = "[hidden]";

/// A PEM block: a private key, a certificate request, a sealed blob.
static PEM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)-----BEGIN [A-Z0-9 ]+-----.*?(?:-----END [A-Z0-9 ]+-----|\z)").unwrap()
});

/// A header full of cookies.
static COOKIE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(set-cookie|cookie)(\s*:\s*)[^\r\n]+").unwrap());

/// An Authorization header's credentials, or `token <value>`.
static SCHEME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(bearer|basic|token)(\s+)([A-Za-z0-9._~+/=-]{8,})").unwrap()
});

/// Tokens with a prefix that says what they are: the box's own access and
/// directory tokens, LosOS Login's, and the services' an owner may paste.
static PREFIXED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"\b(authelia_(?:at|rt|ac)_|losos_(?:pat|dir)_|github_pat_|gh[pousr]_|glpat-|",
        r"sk_live_|sk_test_|rk_live_|rk_test_|pk_live_|whsec_|xox[abposr]-|AKIA|ASIA)",
        r"[A-Za-z0-9_\-]{8,}"
    ))
    .unwrap()
});

/// The password in `scheme://user:password@host`.
static USERINFO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([A-Za-z][A-Za-z0-9+.-]*://[^/\s:@]*:)[^/\s@]+@").unwrap());

/// `password=…`, `client_secret: …`, `"token":"…"`.
static NAMED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)\b((?:[a-z0-9]+[_.-])*(?:password|passwd|passphrase|secret|token|",
        r"api[_-]?key|access[_-]?key|private[_-]?key|credentials?|signature|",
        r"session[_-]?(?:id|token|key)|sessionid))",
        r#"("?\s*[=:]\s*"?)([^\s"',;&}\]]+)"#
    ))
    .unwrap()
});

/// A JSON Web Token.
static JWT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\beyJ[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]*").unwrap()
});

/// A long run of hex: a key, a token, the spare key itself.
static HEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b[0-9A-Fa-f]{48,}\b").unwrap());

/// A long run of base64 or base64url.
static BASE64: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z0-9+_-]{40,}={0,2}").unwrap());

/// An escape sequence a terminal would act on.
static ESCAPE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1b(?:\[[0-?]*[ -/]*[@-~]|[@-Z\\-_])").unwrap());

/// Hides secrets in log lines.
#[derive(Debug, Default, Clone)]
pub struct Redactor {
    /// Values hidden wherever they appear, whatever they look like: the
    /// spare key.
    literals: Vec<String>,
}

impl Redactor {
    #[must_use]
    pub fn new(literals: impl IntoIterator<Item = String>) -> Redactor {
        Redactor {
            literals: literals
                .into_iter()
                .map(|s| s.trim().to_string())
                .filter(|s| s.len() >= 8)
                .collect(),
        }
    }

    /// `line` with its secrets replaced by [`HIDDEN`], and no escape
    /// sequences or control characters but tabs and newlines.
    #[must_use]
    pub fn line(&self, line: &str) -> String {
        let mut out = ESCAPE.replace_all(line, "").into_owned();
        out.retain(|c| !c.is_control() || c == '\t' || c == '\n');
        for literal in &self.literals {
            if out.contains(literal.as_str()) {
                out = out.replace(literal.as_str(), HIDDEN);
            }
        }
        let out = PEM.replace_all(&out, HIDDEN);
        let out = COOKIE.replace_all(&out, format!("$1$2{HIDDEN}"));
        let hay: &str = &out;
        let out = SCHEME.replace_all(hay, |c: &Captures<'_>| {
            // "token rotation" is a word; a value has a digit or a symbol.
            let value = &c[3];
            let whole = c.get(0).map_or(0..0, |m| m.range());
            // `plugin=basic routes=12` is two fields of a log line, not a
            // header: the scheme is a value, the "credential" a key.
            let a_value = hay[..whole.start].ends_with('=');
            let a_key = hay[whole.end..].starts_with(['=', ':'])
                || value.trim_end_matches('=').contains('=')
                || (value.ends_with('=')
                    && hay[whole.end..].starts_with(|ch: char| ch.is_alphanumeric()));
            if a_value
                || a_key
                || (c[1].eq_ignore_ascii_case("token")
                    && value.chars().all(|ch| ch.is_alphabetic()))
            {
                c[0].to_string()
            } else {
                format!("{}{}{HIDDEN}", &c[1], &c[2])
            }
        });
        let out = PREFIXED.replace_all(&out, format!("$1{HIDDEN}"));
        let out = USERINFO.replace_all(&out, format!("${{1}}{HIDDEN}@"));
        let out = NAMED.replace_all(&out, |c: &Captures<'_>| {
            if &c[3] == HIDDEN || c[3].starts_with(HIDDEN) {
                c[0].to_string()
            } else {
                format!("{}{}{HIDDEN}", &c[1], &c[2])
            }
        });
        let out = JWT.replace_all(&out, HIDDEN);
        let out = HEX.replace_all(&out, HIDDEN);
        let out = BASE64.replace_all(&out, |c: &Captures<'_>| {
            let s = &c[0];
            let upper = s.bytes().any(|b| b.is_ascii_uppercase());
            let lower = s.bytes().any(|b| b.is_ascii_lowercase());
            let digit = s.bytes().any(|b| b.is_ascii_digit());
            if upper && lower && digit {
                HIDDEN.to_string()
            } else {
                s.to_string()
            }
        });
        out.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(line: &str) -> String {
        Redactor::new(["c0ffee-spare-key-0001".to_string()]).line(line)
    }

    #[test]
    fn ordinary_lines_are_left_alone() {
        for line in [
            "Started lososd.service - losos appliance control daemon.",
            "rebuild 20261029120001-42-1 finished: done",
            "GET /api/state 200 3ms",
            "token rotation scheduled for tomorrow",
            "/nix/store/0c7rbz7ahsmm5kgfx1brhbpvh9cmym6p-losos-ctl-0.1.0/bin/lososd",
            "commit 4b825dc642cb6eb9a060e54bf8d69288fbee4904 pushed",
            "password policy: at least 12 characters",
        ] {
            assert_eq!(r(line), line, "{line}");
        }
    }

    #[test]
    fn the_spare_key_is_hidden_wherever_it_is() {
        assert_eq!(r("key is c0ffee-spare-key-0001!"), "key is [hidden]!");
        let hex = "a".repeat(64);
        assert_eq!(r(&format!("admin token {hex}")), "admin token [hidden]");
    }

    #[test]
    fn named_values_are_hidden() {
        assert_eq!(r("password=hunter2 next"), "password=[hidden] next");
        assert_eq!(r("client_secret: abc123"), "client_secret: [hidden]");
        assert_eq!(
            r(r#"{"token":"abc","user":"m"}"#),
            r#"{"token":"[hidden]","user":"m"}"#
        );
        assert_eq!(r("SMTP_PASSWORD=x&y"), "SMTP_PASSWORD=[hidden]&y");
        assert_eq!(r("session_id=42"), "session_id=[hidden]");
    }

    #[test]
    fn headers_and_addresses_are_hidden() {
        assert_eq!(
            r("Authorization: Bearer abcdefgh123"),
            "Authorization: Bearer [hidden]"
        );
        assert_eq!(r("Basic bWF0dXM6aHVudGVyMg=="), "Basic [hidden]");
        assert_eq!(r("Cookie: a=b; c=d"), "Cookie: [hidden]");
        assert_eq!(
            r("cloning https://matus:s3cret@git.example/x.git"),
            "cloning https://matus:[hidden]@git.example/x.git"
        );
    }

    #[test]
    fn fields_named_like_a_scheme_are_left_alone() {
        for line in [
            "plugin loaded plugin=basic routes=12 ms=22453",
            "auth=bearer realm=losos.local",
            "checked token expiry_seconds=3600",
        ] {
            assert_eq!(r(line), line);
        }
        assert_eq!(
            r("Authorization: Basic bWF0dXM6aHVudGVyMg=="),
            "Authorization: Basic [hidden]"
        );
    }

    #[test]
    fn known_tokens_are_hidden() {
        assert_eq!(
            r("used losos_pat_0123456789abcdef"),
            "used losos_pat_[hidden]"
        );
        assert_eq!(
            r("ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"),
            "ghp_[hidden]"
        );
        assert_eq!(r("key sk_live_51Habcdefgh"), "key sk_live_[hidden]");
        assert_eq!(r("authelia_at_abcdefghij.k"), "authelia_at_[hidden].k");
        assert_eq!(
            r("jwt eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJtIn0.c2lnbmF0dXJl"),
            "jwt [hidden]"
        );
        assert_eq!(
            r("blob 3q2+7wABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4f done"),
            "blob [hidden] done"
        );
    }

    #[test]
    fn key_blocks_are_hidden() {
        let pem = "-----BEGIN PRIVATE KEY-----\nMIIabc\n-----END PRIVATE KEY-----";
        assert_eq!(r(&format!("got {pem} ok")), "got [hidden] ok");
        assert_eq!(r("-----BEGIN PGP MESSAGE-----\nhQEMA"), "[hidden]");
    }

    #[test]
    fn escape_sequences_and_controls_go() {
        assert_eq!(r("\x1b[31mred\x1b[0m\x07 bell\tok"), "red bell\tok");
    }
}
