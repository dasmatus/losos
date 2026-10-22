//! The words that tie text to issues: `LOS-12`, `#12` and `owner/repo#12`
//! name an issue, and a closing word before them (`Fixes LOS-12`,
//! `closes #3, #4`) says the commit or pull request finishes it. `@eva`
//! mentions a person. What is inside a fenced code block names nothing.

/// Which issue a reference points at.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Target {
    /// `LOS-12`.
    Key(String, i64),
    /// `#12`, in the repository the text belongs to.
    Local(i64),
    /// `matus/site#12`.
    Repo(String, String, i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub target: Target,
    /// A closing word came before it.
    pub closes: bool,
}

const CLOSING: &[&str] = &[
    "close",
    "closes",
    "closed",
    "closing",
    "fix",
    "fixes",
    "fixed",
    "fixing",
    "resolve",
    "resolves",
    "resolved",
    "resolving",
];

/// What keeps a list going after a closing word: `Fixes #1, #2 and #3`.
const JOINERS: &[&str] = &["and", "&", "+"];

fn trim_punct(w: &str) -> &str {
    w.trim_matches(|c: char| {
        matches!(
            c,
            '(' | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '.'
                | ','
                | ';'
                | ':'
                | '!'
                | '?'
                | '"'
                | '\''
                | '`'
                | '*'
                | '_'
        )
    })
}

fn number(s: &str) -> Option<i64> {
    if s.is_empty() || s.len() > 9 || !s.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.parse().ok().filter(|n| *n > 0)
}

fn target(word: &str) -> Option<Target> {
    if let Some(n) = word.strip_prefix('#') {
        return number(n).map(Target::Local);
    }
    if let Some((repo, n)) = word.split_once('#') {
        let (owner, name) = repo.split_once('/')?;
        let n = number(n)?;
        if super::super::names::owner(owner).is_err() || super::super::names::repo(name).is_err() {
            return None;
        }
        return Some(Target::Repo(owner.to_string(), name.to_string(), n));
    }
    // A key is written upper case; `fix-3` in a sentence is not one.
    let (k, n) = word.rsplit_once('-')?;
    if k.chars().any(|c| c.is_ascii_lowercase()) {
        return None;
    }
    let (k, n) = super::parse_key(&format!("{k}-{n}"))?;
    Some(Target::Key(k, n))
}

/// The lines of `text` outside fenced code blocks.
fn prose(text: &str) -> impl Iterator<Item = &str> {
    let mut fenced = false;
    text.lines().filter(move |l| {
        if l.trim_start().starts_with("```") || l.trim_start().starts_with("~~~") {
            fenced = !fenced;
            return false;
        }
        !fenced
    })
}

/// Every issue `text` names, once each, in the order first named. One
/// named both ways closes.
pub fn references(text: &str) -> Vec<Reference> {
    let mut out: Vec<Reference> = Vec::new();
    for line in prose(text) {
        let mut closing = false;
        for raw in line.split_whitespace() {
            // `Fixes:` and `fixes,` are the word too.
            let word = trim_punct(raw);
            if word.is_empty() {
                continue;
            }
            if CLOSING.contains(&word.to_ascii_lowercase().as_str()) {
                closing = true;
                continue;
            }
            if let Some(t) = target(word) {
                match out.iter_mut().find(|r| r.target == t) {
                    Some(r) => r.closes |= closing,
                    None => out.push(Reference {
                        target: t,
                        closes: closing,
                    }),
                }
                // A list goes on while the reference ends in a comma.
                if !raw.ends_with(',') {
                    closing = closing && raw == word;
                }
                continue;
            }
            if JOINERS.contains(&word.to_ascii_lowercase().as_str()) {
                continue;
            }
            closing = false;
        }
    }
    out
}

/// The people `text` mentions: `@eva`, lower case, once each.
pub fn mentions(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in prose(text) {
        for (i, _) in line.match_indices('@') {
            // `me@example.org` is an address, not a mention.
            if line[..i]
                .chars()
                .last()
                .is_some_and(|c| c.is_alphanumeric() || c == '.' || c == '_')
            {
                continue;
            }
            let rest = &line[i + 1..];
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.'))
                .unwrap_or(rest.len());
            let login = rest[..end].trim_end_matches('.').to_ascii_lowercase();
            if !login.is_empty()
                && super::super::names::owner(&login).is_ok()
                && !out.contains(&login)
            {
                out.push(login);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(t: Target, closes: bool) -> Reference {
        Reference { target: t, closes }
    }

    #[test]
    fn closing_words_close_what_follows() {
        assert_eq!(
            references("Fixes LOS-12"),
            vec![r(Target::Key("LOS".into(), 12), true)]
        );
        assert_eq!(
            references("closes #3, #4 and matus/site#5\n\nSee LOS-1."),
            vec![
                r(Target::Local(3), true),
                r(Target::Local(4), true),
                r(Target::Repo("matus".into(), "site".into(), 5), true),
                r(Target::Key("LOS".into(), 1), false),
            ]
        );
        assert_eq!(
            references("Resolved: LOS-2"),
            vec![r(Target::Key("LOS".into(), 2), true)]
        );
    }

    #[test]
    fn a_mention_is_a_link_only() {
        assert_eq!(
            references("Start on LOS-7 (needs #2)"),
            vec![
                r(Target::Key("LOS".into(), 7), false),
                r(Target::Local(2), false)
            ]
        );
        assert_eq!(
            references("fix the thing in LOS-7"),
            vec![r(Target::Key("LOS".into(), 7), false)]
        );
    }

    #[test]
    fn named_twice_closes_once() {
        assert_eq!(
            references("LOS-3: tidy\n\nFixes LOS-3"),
            vec![r(Target::Key("LOS".into(), 3), true)]
        );
    }

    #[test]
    fn code_and_ordinary_words_name_nothing() {
        assert!(references("```\nfixes #1\n```\nfix-3 is a branch, not-an-issue").is_empty());
        assert!(references("utf-8 and x-1 and #0").is_empty());
    }

    #[test]
    fn mentions_are_people() {
        assert_eq!(
            mentions("@eva and @Matus, write to eva@box.local. @eva"),
            vec!["eva".to_string(), "matus".to_string()]
        );
        assert!(mentions("```\n@eva\n```").is_empty());
    }
}
