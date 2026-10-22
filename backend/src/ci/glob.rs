//! GitHub Actions' filter patterns: the branch and tag lists of `on:`, and
//! the file patterns of `hashFiles()`.
//!
//! `*` matches anything but `/`, `**` anything, `?` zero or one of the
//! character before it, `+` one or more of it, `[...]` one character of a
//! set (`a-z` ranges), and a leading `!` in a list turns a pattern into an
//! exclusion. Later patterns win over earlier ones, as on GitHub.

#[derive(Debug, Clone, PartialEq)]
enum Atom {
    Char(char),
    Set(Vec<(char, char)>),
    Star,
    DoubleStar,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Count {
    One,
    ZeroOrOne,
    OneOrMore,
}

fn compile(pattern: &str) -> Vec<(Atom, Count)> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out: Vec<(Atom, Count)> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '*' if chars.get(i + 1) == Some(&'*') => {
                out.push((Atom::DoubleStar, Count::One));
                i += 2;
                // `**/` also matches nothing at all: `**/x` finds `x`.
                if chars.get(i) == Some(&'/') {
                    out.push((Atom::Char('/'), Count::ZeroOrOne));
                    i += 1;
                }
            }
            '*' => {
                out.push((Atom::Star, Count::One));
                i += 1;
            }
            '?' | '+' if !out.is_empty() => {
                if let Some(last) = out.last_mut() {
                    last.1 = if chars[i] == '?' {
                        Count::ZeroOrOne
                    } else {
                        Count::OneOrMore
                    };
                }
                i += 1;
            }
            '[' => match chars[i..].iter().position(|&c| c == ']') {
                Some(end) if end > 1 => {
                    let inner = &chars[i + 1..i + end];
                    let mut set = Vec::new();
                    let mut j = 0;
                    while j < inner.len() {
                        if inner.get(j + 1) == Some(&'-') && j + 2 < inner.len() {
                            set.push((inner[j], inner[j + 2]));
                            j += 3;
                        } else {
                            set.push((inner[j], inner[j]));
                            j += 1;
                        }
                    }
                    out.push((Atom::Set(set), Count::One));
                    i += end + 1;
                }
                _ => {
                    out.push((Atom::Char('['), Count::One));
                    i += 1;
                }
            },
            '\\' if i + 1 < chars.len() => {
                out.push((Atom::Char(chars[i + 1]), Count::One));
                i += 2;
            }
            c => {
                out.push((Atom::Char(c), Count::One));
                i += 1;
            }
        }
    }
    out
}

fn one(atom: &Atom, c: char) -> bool {
    match atom {
        Atom::Char(x) => *x == c,
        Atom::Set(ranges) => ranges.iter().any(|(a, b)| *a <= c && c <= *b),
        Atom::Star => c != '/',
        Atom::DoubleStar => true,
    }
}

fn walk(p: &[(Atom, Count)], s: &[char]) -> bool {
    let Some(((atom, count), rest)) = p.split_first() else {
        return s.is_empty();
    };
    match (atom, count) {
        (Atom::Star | Atom::DoubleStar, _) => {
            for k in 0..=s.len() {
                if walk(rest, &s[k..]) {
                    return true;
                }
                if k < s.len() && !one(atom, s[k]) {
                    return false;
                }
            }
            false
        }
        (_, Count::One) => !s.is_empty() && one(atom, s[0]) && walk(rest, &s[1..]),
        (_, Count::ZeroOrOne) => {
            walk(rest, s) || (!s.is_empty() && one(atom, s[0]) && walk(rest, &s[1..]))
        }
        (_, Count::OneOrMore) => {
            let mut k = 0;
            while k < s.len() && one(atom, s[k]) {
                k += 1;
                if walk(rest, &s[k..]) {
                    return true;
                }
            }
            false
        }
    }
}

/// Whether `text` matches one pattern.
pub fn matches(pattern: &str, text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    walk(&compile(pattern), &chars)
}

/// Whether `text` passes a list: included by the last pattern that
/// matches it, where a `!` pattern excludes.
pub fn filter(patterns: &[String], text: &str) -> bool {
    let mut included = false;
    for p in patterns {
        match p.strip_prefix('!') {
            Some(neg) if matches(neg, text) => included = false,
            None if matches(p, text) => included = true,
            _ => {}
        }
    }
    included
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_examples() {
        assert!(matches("feature/*", "feature/my-branch"));
        assert!(!matches("feature/*", "feature/your/branch"));
        assert!(matches("feature/**", "feature/your/branch"));
        assert!(matches("main", "main"));
        assert!(!matches("main", "mainline"));
        assert!(matches("v2*", "v2.0.1"));
        assert!(matches("v[12].[0-9]+.[0-9]+", "v1.10.1"));
        assert!(!matches("v[12].[0-9]+.[0-9]+", "v3.1.0"));
        assert!(matches("*.jsx?", "page.js"));
        assert!(matches("*.jsx?", "page.jsx"));
        assert!(matches("**/README.md", "README.md"));
        assert!(matches("**/README.md", "docs/README.md"));
        assert!(matches("**.js", "a/b/c.js"));
    }

    #[test]
    fn lists_with_exclusions() {
        let l = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let pats = l(&["releases/**", "!releases/**-alpha"]);
        assert!(filter(&pats, "releases/10"));
        assert!(!filter(&pats, "releases/10-alpha"));
        let pats = l(&["releases/**", "!releases/**-alpha", "releases/beta-alpha"]);
        assert!(filter(&pats, "releases/beta-alpha"));
        assert!(!filter(&l(&[]), "main"));
    }
}
