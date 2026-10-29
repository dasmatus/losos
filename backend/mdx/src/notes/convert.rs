//! A Markdown note as MDX, saying the same thing.
//!
//! MDX reads three things Markdown has differently: `{` opens an
//! expression and `<` an element, and it has no raw HTML, no autolinks in
//! angle brackets and no indented code. So the text keeps its words and
//! only those change: braces and angle brackets outside code and maths are
//! escaped, an HTML comment becomes a %%comment%%, the few elements MDX
//! writes as themselves (`<br>`, `<kbd>`) stay, other HTML is kept as text,
//! `<https://…>` becomes a link, and indented code is fenced. The result is
//! read as a note before it is handed back; one that does not read without
//! a problem is refused rather than half converted.

use crate::mdast::Node;
use crate::Flavour;
use markdown::{to_mdast, ParseOptions};

/// What could not be converted, with the line it is on, from 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Refused {
    pub line: usize,
    pub reason: String,
}

/// An edit of the source: replace `start..end` with `with`.
struct Edit {
    start: usize,
    end: usize,
    with: String,
}

pub(crate) fn md_to_mdx(source: &str) -> Result<String, Refused> {
    let md = crate::options(Flavour::Md);
    // Backslashes read best, but a GFM autolink takes a backslash into its
    // address (`http://h:\<port>`), so a note where they do not hold gets
    // character references instead.
    convert(source, &md, false).or_else(|_| convert(source, &md, true))
}

fn convert(source: &str, md: &ParseOptions, entities: bool) -> Result<String, Refused> {
    let tree = to_mdast(source, md).map_err(|m| Refused {
        line: m.place.as_ref().map(|p| line_of(p)).unwrap_or(1),
        reason: m.reason.clone(),
    })?;
    let mut edits = Vec::new();
    let mut kept = Vec::new();
    walk(&tree, source, entities, &mut edits, &mut kept);
    // Escapes everywhere the edits and the protected spans do not reach.
    edits.sort_by_key(|e| e.start);
    kept.sort_unstable();
    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len() + 16);
    let (mut at, mut next) = (0, 0);
    while at < source.len() {
        if let Some(e) = edits.get(next).filter(|e| e.start == at) {
            out.push_str(&e.with);
            at = e.end;
            next += 1;
            continue;
        }
        if let Some(&(_, end)) = kept.iter().find(|&&(s, e)| s <= at && at < e) {
            out.push_str(&source[at..end]);
            at = end;
            continue;
        }
        let ch = source[at..].chars().next().unwrap_or(' ');
        push_escaped(&mut out, ch, escaped(bytes, at), entities);
        at += ch.len_utf8();
    }
    // Read as a note is read, so a conversion that only parses is not
    // enough: nothing in it may be refused either.
    let parsed =
        crate::parse(&super::pass::strip_comments(&out), Flavour::Mdx).map_err(|e| Refused {
            line: match &e {
                crate::Error::Syntax { place, .. } | crate::Error::TooDeep { place } => place.line,
                _ => 1,
            },
            reason: e.to_string(),
        })?;
    if let Some(p) = parsed.problems.first() {
        return Err(Refused {
            line: p.place.line,
            reason: p.message.clone(),
        });
    }
    Ok(out)
}

fn line_of(place: &markdown::message::Place) -> usize {
    match place {
        markdown::message::Place::Position(p) => p.start.line,
        markdown::message::Place::Point(p) => p.line,
    }
}

/// Preceded by an odd number of backslashes.
fn escaped(bytes: &[u8], at: usize) -> bool {
    let n = bytes[..at]
        .iter()
        .rev()
        .take_while(|&&b| b == b'\\')
        .count();
    n % 2 == 1
}

fn span(node: &Node) -> Option<(usize, usize)> {
    node.position().map(|p| (p.start.offset, p.end.offset))
}

fn walk(
    node: &Node,
    source: &str,
    entities: bool,
    edits: &mut Vec<Edit>,
    kept: &mut Vec<(usize, usize)>,
) {
    match node {
        // Code, maths and front matter say what they say.
        Node::InlineCode(_)
        | Node::InlineMath(_)
        | Node::Math(_)
        | Node::Yaml(_)
        | Node::Toml(_) => {
            if let Some(s) = span(node) {
                kept.push(s);
            }
            return;
        }
        Node::Code(code) => {
            let Some((start, end)) = span(node) else {
                return;
            };
            let raw = &source[start..end];
            let fenced = raw.starts_with("```") || raw.starts_with("~~~");
            if fenced {
                kept.push((start, end));
            } else {
                // The container's prefix on the first line (`> `, a list's
                // indent) goes before every line of the fenced copy.
                let line_start = source[..start].rfind('\n').map_or(0, |i| i + 1);
                let prefix = &source[line_start..start];
                let prefix: String = prefix
                    .chars()
                    .map(|c| {
                        if c == '>' || c == ' ' || c == '\t' {
                            c
                        } else {
                            ' '
                        }
                    })
                    .collect();
                let longest = longest_run(&code.value, '`');
                let fence = "`".repeat(longest.max(2) + 1);
                let mut with = format!("{fence}\n");
                for l in code.value.split('\n') {
                    with.push_str(&prefix);
                    with.push_str(l);
                    with.push('\n');
                }
                with.push_str(&prefix);
                with.push_str(&fence);
                edits.push(Edit { start, end, with });
            }
            return;
        }
        Node::Html(html) => {
            let Some((start, end)) = span(node) else {
                return;
            };
            let v = html.value.trim();
            if let Some(inner) = v.strip_prefix("<!--").and_then(|r| r.strip_suffix("-->")) {
                let with = format!("%%{}%%", inner.replace("%%", "% %"));
                edits.push(Edit { start, end, with });
            } else if let Some(tag) = (!entities).then(|| plain_element(v)).flatten() {
                // <br>, <kbd>…</kbd>: elements MDX keeps as themselves.
                edits.push(Edit {
                    start,
                    end,
                    with: tag,
                });
            } else {
                edits.push(Edit {
                    start,
                    end,
                    with: escape_all(&source[start..end], entities),
                });
            }
            return;
        }
        Node::Link(link) => {
            if let Some((start, end)) = span(node) {
                if source[start..].starts_with('<') {
                    // <https://…> and <someone@example.com>
                    // The text escaped so it is not an autolink of its own
                    // inside the link.
                    let shown = source[start + 1..end - 1]
                        .replacen("://", "\\://", 1)
                        .replace('@', "\\@");
                    let with = format!(
                        "[{}]({})",
                        escape_all(&shown, entities),
                        link.url.replace(' ', "%20").replace(')', "%29")
                    );
                    edits.push(Edit { start, end, with });
                    return;
                }
            }
        }
        _ => {}
    }
    if let Some(children) = node.children() {
        for c in children {
            walk(c, source, entities, edits, kept);
        }
    }
}

/// A lone tag of an element MDX writes as itself, as MDX spells it, or
/// None. An unmatched one fails the MDX parse, and the second try then
/// writes all HTML as text.
fn plain_element(html: &str) -> Option<String> {
    let inner = html.strip_prefix('<')?.strip_suffix('>')?;
    let (close, inner) = match inner.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (false, inner),
    };
    let name = inner.trim_end_matches('/').trim();
    let known = ["br", "kbd", "mark", "sub", "sup", "u", "s", "small"];
    if !known.contains(&name.to_ascii_lowercase().as_str()) {
        return None;
    }
    let name = name.to_ascii_lowercase();
    Some(match (close, name.as_str()) {
        (false, "br") => "<br />".to_owned(),
        (true, "br") => return None,
        (false, _) => format!("<{name}>"),
        (true, _) => format!("</{name}>"),
    })
}

/// Every brace and angle bracket escaped, for text that was HTML.
fn escape_all(text: &str, entities: bool) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let bytes = text.as_bytes();
    for (i, c) in text.char_indices() {
        push_escaped(&mut out, c, escaped(bytes, i), entities);
    }
    out
}

/// A character, escaped when MDX would read it as syntax.
fn push_escaped(out: &mut String, c: char, already: bool, entities: bool) {
    if already || !matches!(c, '{' | '}' | '<') {
        out.push(c);
    } else if entities {
        out.push_str(match c {
            '{' => "&#123;",
            '}' => "&#125;",
            _ => "&lt;",
        });
    } else {
        out.push('\\');
        out.push(c);
    }
}

fn longest_run(text: &str, c: char) -> usize {
    let mut best = 0;
    let mut run = 0;
    for ch in text.chars() {
        if ch == c {
            run += 1;
            best = best.max(run);
        } else {
            run = 0;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(s: &str) -> String {
        md_to_mdx(s).unwrap()
    }

    #[test]
    fn braces_and_brackets_are_escaped_outside_code() {
        assert_eq!(
            conv("a {b} c < d `{x}` $\\{y\\}$ \\{z"),
            "a \\{b\\} c \\< d `{x}` $\\{y\\}$ \\{z"
        );
    }

    #[test]
    fn html_comments_become_comments_and_html_text() {
        assert_eq!(
            conv("<!-- hidden -->\n\nx <br> y <kbd>K</kbd>"),
            "%% hidden %%\n\nx <br /> y <kbd>K</kbd>"
        );
        assert_eq!(
            conv("x <div>y</div> <u>open"),
            "x &lt;div>y&lt;/div> &lt;u>open"
        );
    }

    #[test]
    fn autolinks_become_links() {
        assert_eq!(
            conv("see <https://x.y/a> and <a@b.cz>"),
            "see [https\\://x.y/a](https://x.y/a) and [a\\@b.cz](mailto:a@b.cz)"
        );
    }

    #[test]
    fn indented_code_is_fenced() {
        assert_eq!(
            conv("text\n\n    let x = {1};\n    y"),
            "text\n\n```\nlet x = {1};\ny\n```"
        );
        assert_eq!(conv("> q\n>\n>     code"), "> q\n>\n> ```\n> code\n> ```");
    }

    #[test]
    fn obsidian_syntax_is_kept() {
        let s = "---\ntags: [a]\n---\n# T\n[[x|y]] ![[p.png]] #tag ==h== %%c%% ^id\n\n> [!note] t\n> b\n\n- [ ] task\n\n```js\nconst a = {b: 1} < 2;\n```\n";
        assert_eq!(conv(s), s);
    }

    #[test]
    fn autolink_literals_get_references() {
        assert_eq!(
            conv("see http://1.2.3.4:<p> x"),
            "see http://1.2.3.4:&lt;p> x"
        );
    }

    #[test]
    fn front_matter_and_fences_stay() {
        let s = "---\na: \"{x}\"\n---\n```\n<div>{y}</div>\n```\n";
        assert_eq!(conv(s), s);
    }
}
