//! What a note says about itself, read from its Markdown: its properties
//! (the front matter), its tags and the links it makes.
//!
//! The web app renders the note and decides which note a link means
//! (`admin-ui/app/src/notes/links.ts`); the box only lists, so the graph,
//! backlinks and tag counts can be drawn without every note's text going
//! to the browser. Code, inline or fenced, and %%comments%% are skipped,
//! as Obsidian skips them.

use serde::Serialize;

/// A link as written: the note or file named, before `#` and `|`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Link {
    pub target: String,
    /// `![[…]]`: shown inside the note rather than linked.
    pub embed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Scanned {
    /// The `title` property, when there is one.
    pub title: Option<String>,
    pub aliases: Vec<String>,
    /// Without the `#`, as written, properties' and the text's together.
    pub tags: Vec<String>,
    pub links: Vec<Link>,
    /// `publish: false` or `private: true`: never on a knowledge base.
    pub private: bool,
}

/// The front matter's lines and where the text starts, in bytes.
fn front_matter(text: &str) -> (Option<&str>, usize) {
    let Some(rest) = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
    else {
        return (None, 0);
    };
    let start = text.len() - rest.len();
    let mut at = 0;
    for line in rest.split_inclusive('\n') {
        let bare = line.trim_end();
        if bare == "---" || bare == "..." {
            return (Some(&rest[..at]), start + at + line.len());
        }
        at += line.len();
    }
    (None, 0)
}

/// A scalar as YAML writes it: quotes off.
fn unquote(v: &str) -> String {
    let v = v.trim();
    if v.len() >= 2
        && ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')))
    {
        return v[1..v.len() - 1].to_owned();
    }
    v.to_owned()
}

/// `[a, "b"]`, `a, b` or `a b` (tags only), as a list.
fn inline_list(v: &str, spaces: bool) -> Vec<String> {
    let v = v.trim();
    let v = v
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .unwrap_or(v);
    v.split(|c: char| c == ',' || (spaces && c.is_whitespace()))
        .map(unquote)
        .filter(|s| !s.is_empty())
        .collect()
}

/// The few properties the box reads. A full YAML reader is the web app's
/// (it shows every property); these are the keys Obsidian itself gives a
/// meaning, in the shapes it writes them.
fn properties(yaml: &str, out: &mut Scanned) {
    let mut key: Option<String> = None;
    for line in yaml.lines() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let indented = line.starts_with(' ') || line.starts_with('\t');
        if let (true, Some(k), Some(item)) = (
            indented || line.starts_with('-'),
            key.as_deref(),
            line.trim_start().strip_prefix('-'),
        ) {
            let item = unquote(item);
            if item.is_empty() {
                continue;
            }
            match k {
                "tags" | "tag" => out.tags.push(item.trim_start_matches('#').to_owned()),
                "aliases" | "alias" => out.aliases.push(item),
                _ => {}
            }
            continue;
        }
        if indented {
            continue;
        }
        let Some((k, v)) = line.split_once(':') else {
            key = None;
            continue;
        };
        let k = k.trim().to_ascii_lowercase();
        let v = v.trim();
        match k.as_str() {
            "title" if !v.is_empty() => out.title = Some(unquote(v)),
            "tags" | "tag" => out.tags.extend(
                inline_list(v, true)
                    .into_iter()
                    .map(|t| t.trim_start_matches('#').to_owned()),
            ),
            "aliases" | "alias" => out.aliases.extend(inline_list(v, false)),
            "publish" => out.private |= unquote(v).eq_ignore_ascii_case("false"),
            "private" => out.private |= unquote(v).eq_ignore_ascii_case("true"),
            _ => {}
        }
        key = Some(k);
    }
}

/// Take %%comments%% and `inline code` out of a line, keeping the rest.
/// `open` carries a comment over lines.
fn visible(line: &str, open: &mut bool) -> String {
    let mut out = String::new();
    let mut rest = line;
    loop {
        if *open {
            match rest.find("%%") {
                Some(at) => {
                    *open = false;
                    rest = &rest[at + 2..];
                }
                None => return out,
            }
        }
        let c = rest.find("%%");
        let b = rest.find('`');
        match (c, b) {
            (Some(c), b) if b.is_none_or(|b| c < b) => {
                out.push_str(&rest[..c]);
                *open = true;
                rest = &rest[c + 2..];
            }
            (_, Some(b)) => {
                out.push_str(&rest[..b]);
                let ticks = rest[b..].chars().take_while(|&c| c == '`').count();
                let fence = &rest[b..b + ticks];
                match rest[b + ticks..].find(fence) {
                    Some(end) => rest = &rest[b + ticks + end + ticks..],
                    None => {
                        out.push_str(&rest[b..]);
                        return out;
                    }
                }
            }
            _ => {
                out.push_str(rest);
                return out;
            }
        }
    }
}

fn tag_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '/')
}

/// `#tag` after a space or at the start: letters, digits, `_`, `-`, `/`,
/// and not digits alone (`#1` is no tag).
fn tags_in(line: &str, out: &mut Vec<String>) {
    let mut prev = ' ';
    let mut chars = line.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '#' && (prev.is_whitespace() || matches!(prev, '(' | '[' | '{' | ',' | ';')) {
            let tag: String = line[i + 1..].chars().take_while(|&c| tag_char(c)).collect();
            let tag = tag.trim_end_matches('/');
            if !tag.is_empty() && tag.chars().any(|c| !c.is_ascii_digit()) {
                out.push(tag.to_owned());
                for _ in 0..tag.chars().count() {
                    chars.next();
                }
                prev = 'x';
                continue;
            }
        }
        prev = c;
    }
}

/// `[[…]]` and `![[…]]`.
fn wikilinks_in(line: &str, out: &mut Vec<Link>) {
    let mut rest = line;
    let mut offset = 0;
    while let Some(at) = rest.find("[[") {
        let embed = at > 0 && rest.as_bytes()[at - 1] == b'!';
        let after = &rest[at + 2..];
        let Some(end) = after.find("]]") else { break };
        let inner = &after[..end];
        let target = inner.replace("\\|", "|");
        let target = target.split('|').next().unwrap_or_default();
        let target = target.split('#').next().unwrap_or_default().trim();
        if !target.is_empty() && !inner.contains("[[") {
            out.push(Link {
                target: target.to_owned(),
                embed,
            });
        }
        offset += at + 2 + end + 2;
        rest = &line[offset..];
    }
}

/// `[text](path.md)` and `![alt](picture.png)` to files in the vault, not
/// to other sites.
fn markdown_links_in(line: &str, out: &mut Vec<Link>) {
    let mut rest = line;
    while let Some(at) = rest.find("](") {
        let embed = rest[..at]
            .rfind('[')
            .is_some_and(|open| open > 0 && rest.as_bytes()[open - 1] == b'!');
        let after = &rest[at + 2..];
        let Some(end) = after.find(')') else { break };
        let href = after[..end].trim();
        let href = href
            .strip_prefix('<')
            .and_then(|h| h.strip_suffix('>'))
            .unwrap_or_else(|| href.split_whitespace().next().unwrap_or_default());
        let path = href.split('#').next().unwrap_or_default();
        let outside = path.contains(':') || path.starts_with("//");
        if !path.is_empty() && !outside {
            let decoded = crate::cloud::util::decode_path(path).unwrap_or_else(|| path.to_owned());
            out.push(Link {
                target: decoded,
                embed,
            });
        }
        rest = &after[end + 1..];
    }
}

/// Read a note.
pub fn scan(text: &str) -> Scanned {
    let mut out = Scanned::default();
    let (yaml, start) = front_matter(text);
    if let Some(y) = yaml {
        properties(y, &mut out);
    }
    let mut fence: Option<String> = None;
    let mut comment = false;
    for line in text[start..].lines() {
        let bare = line.trim_start();
        let lead = line.len() - bare.len();
        let marker: String = bare
            .chars()
            .take_while(|&c| c == '`' || c == '~')
            .collect();
        if let Some(f) = &fence {
            if marker.len() >= f.len()
                && marker.starts_with(&f[..1])
                && bare.trim_end() == marker
                && !comment
            {
                fence = None;
            }
            continue;
        }
        if lead <= 3 && marker.len() >= 3 && !comment {
            let ch = marker.chars().next().unwrap_or('`');
            fence = Some(marker.chars().take_while(|&c| c == ch).collect());
            continue;
        }
        // Four spaces in and outside a list: indented code.
        if lead >= 4 && !bare.starts_with(['-', '*', '+']) && !comment {
            let numbered = bare.split_once(". ").is_some_and(|(n, _)| n.chars().all(|c| c.is_ascii_digit()));
            if !numbered {
                continue;
            }
        }
        let shown = visible(line, &mut comment);
        // A heading's own `#`s are not tags.
        let body = shown.trim_start();
        let body = if body.starts_with('#') {
            let hashes = body.chars().take_while(|&c| c == '#').count();
            if hashes <= 6 && body[hashes..].starts_with([' ', '\t']) {
                &body[hashes..]
            } else {
                body
            }
        } else {
            body
        };
        tags_in(body, &mut out.tags);
        wikilinks_in(&shown, &mut out.links);
        markdown_links_in(&shown, &mut out.links);
    }
    let mut seen = std::collections::HashSet::new();
    out.tags.retain(|t| !t.is_empty() && seen.insert(t.to_lowercase()));
    out
}

/// Read a JSON Canvas file: its file nodes link the files they show, and
/// its text nodes are Markdown with links of their own.
pub fn scan_canvas(text: &str) -> Scanned {
    let mut out = Scanned::default();
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(text) else {
        return out;
    };
    for node in doc["nodes"].as_array().into_iter().flatten() {
        match node["type"].as_str() {
            Some("file") => {
                if let Some(file) = node["file"].as_str().filter(|f| !f.is_empty()) {
                    out.links.push(Link {
                        target: file.to_owned(),
                        embed: false,
                    });
                }
            }
            Some("text") => {
                let inner = scan(node["text"].as_str().unwrap_or_default());
                out.links.extend(inner.links);
                out.tags.extend(inner.tags);
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(target: &str, embed: bool) -> Link {
        Link {
            target: target.into(),
            embed,
        }
    }

    #[test]
    fn front_matter_gives_title_tags_aliases_and_privacy() {
        let s = scan(
            "---\ntitle: \"Plans\"\ntags:\n  - work\n  - '#home/garden'\naliases: [Plan, \"The plan\"]\npublish: false\n---\n# Plans\n",
        );
        assert_eq!(s.title.as_deref(), Some("Plans"));
        assert_eq!(s.tags, ["work", "home/garden"]);
        assert_eq!(s.aliases, ["Plan", "The plan"]);
        assert!(s.private);
        let s = scan("---\ntags: one two #three\nprivate: true\n---\n");
        assert_eq!(s.tags, ["one", "two", "three"]);
        assert!(s.private);
        assert!(!scan("---\npublish: true\n---\n").private);
    }

    #[test]
    fn no_front_matter_without_the_closing_line() {
        let s = scan("---\ntitle: x\nno end here #tag\n");
        assert_eq!(s.title, None);
    }

    #[test]
    fn tags_in_the_text_but_not_in_code_comments_or_headings() {
        let s = scan(
            "# Heading #inhead\nSome #idea and #area/sub, not #1 or a#b.\n`#code` %%#hidden%%\n```\n#fenced\n```\n%%\n#gone\n%%\n#last",
        );
        assert_eq!(s.tags, ["inhead", "idea", "area/sub", "last"]);
    }

    #[test]
    fn links_wiki_embeds_and_markdown() {
        let s = scan(
            "See [[Other note#Part|shown]] and ![[diagram.png|300]].\n[md](sub/Thing.md#x) ![pic](img/a%20b.png) [web](https://x.example) [[a\\|b]]\n`[[not]]`",
        );
        assert_eq!(
            s.links,
            [
                link("Other note", false),
                link("diagram.png", true),
                // A line's wikilinks, then its Markdown links.
                link("a", false),
                link("sub/Thing.md", false),
                link("img/a b.png", true),
            ]
        );
    }

    #[test]
    fn a_heading_link_to_this_note_names_nothing() {
        assert!(scan("[[#Only here]]").links.is_empty());
    }

    #[test]
    fn tags_once_whatever_their_case() {
        assert_eq!(scan("---\ntags: Work\n---\n#work #Other").tags, ["Work", "Other"]);
    }
}
