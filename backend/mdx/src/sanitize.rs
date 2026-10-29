//! The last word on what reaches a page: an allowlist of tags, attributes,
//! classes and URL schemes, applied to everything render.rs wrote, raw HTML
//! from a `.md` file and a Notes pass's output included.
//!
//! What it keeps out: scripts, styles, frames, forms and their fields,
//! event handlers,
//! `style` attributes (the admin pages' CSP refuses them, and they could
//! dress a document up as the page around it), any URL scheme but http,
//! https and mailto, and any class but LosOS's own document classes, so a
//! note cannot borrow the page's layout classes to cover the page.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

const TAGS: &[&str] = &[
    "a",
    "abbr",
    "aside",
    "blockquote",
    "br",
    "caption",
    "code",
    "del",
    "details",
    "div",
    "em",
    "figcaption",
    "figure",
    "footer",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "img",
    "kbd",
    "li",
    "mark",
    "nav",
    "ol",
    "p",
    "pre",
    "s",
    "section",
    "small",
    "span",
    "strong",
    "sub",
    "summary",
    "sup",
    "table",
    "tbody",
    "td",
    "th",
    "thead",
    "time",
    "tr",
    "u",
    "ul",
];

const TAG_ATTRIBUTES: &[(&str, &[&str])] = &[
    ("a", &["href", "title"]),
    ("abbr", &["title"]),
    ("details", &["open"]),
    ("img", &["src", "alt", "title"]),
    ("ol", &["start"]),
    ("td", &["align", "colspan", "rowspan"]),
    ("th", &["align", "colspan", "rowspan"]),
    ("time", &["datetime"]),
];

/// Class names that may stay: exact, or a prefix ending in `-`.
const CLASSES: &[&str] = &[
    "callout",
    "callout-",
    "contains-task-list",
    "footnote-",
    "footnotes",
    "language-",
    "math-",
    "mdx-",
    "page-break",
    "task-box",
    "task-list-item",
    // Notes' own syntax (its pass in this crate).
    "losos-",
    "is-",
    "wikilink",
    "tag",
    "note-embed",
    "embed-",
];

fn class_ok(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= 64
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'+' || b == b'#')
        && CLASSES.iter().any(|c| {
            if c.ends_with('-') {
                token.starts_with(c)
            } else {
                token == *c
            }
        })
}

/// An id a heading, footnote or Notes block may carry: short and plain, so
/// it cannot stand in for a property of the page (`id="location"`, a
/// clobbering trick, would still need a heading named so; that is accepted).
fn id_ok(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && !id.contains(|c: char| c.is_whitespace() || c == '"' || c == '<' || c == '>')
}

fn builder() -> &'static ammonia::Builder<'static> {
    static B: OnceLock<ammonia::Builder<'static>> = OnceLock::new();
    B.get_or_init(|| make(&[]))
}

/// The page allowlist: the document's, and the placeholder a page's
/// blocks become (`crate::page`), which only that module writes.
fn page_builder() -> &'static ammonia::Builder<'static> {
    static B: OnceLock<ammonia::Builder<'static>> = OnceLock::new();
    B.get_or_init(|| make(&[crate::page::PLACEHOLDER]))
}

fn make(extra: &[&'static str]) -> ammonia::Builder<'static> {
    {
        let mut b = ammonia::Builder::empty();
        b.tags(TAGS.iter().chain(extra).copied().collect::<HashSet<_>>())
            .tag_attributes(
                TAG_ATTRIBUTES
                    .iter()
                    .map(|(t, a)| (*t, a.iter().copied().collect::<HashSet<_>>()))
                    .collect::<HashMap<_, _>>(),
            )
            .generic_attributes(["class", "id"].into_iter().collect())
            .generic_attribute_prefixes(["data-"].into_iter().collect())
            .url_schemes(["http", "https", "mailto"].into_iter().collect())
            .url_relative(ammonia::UrlRelative::PassThrough)
            .link_rel(Some("noopener noreferrer"))
            .strip_comments(true)
            .attribute_filter(|_element, attribute, value| match attribute {
                "class" => {
                    let kept: Vec<&str> =
                        value.split_whitespace().filter(|t| class_ok(t)).collect();
                    (!kept.is_empty()).then(|| kept.join(" ").into())
                }
                "id" => id_ok(value).then(|| value.into()),
                "align" => matches!(value, "left" | "center" | "right").then(|| value.into()),
                "colspan" | "rowspan" | "start" => (value.len() <= 6
                    && value.bytes().all(|b| b.is_ascii_digit()))
                .then(|| value.into()),
                a if a.starts_with("data-") => {
                    let name_ok = a.len() <= 40
                        && a[5..].bytes().all(|b| b.is_ascii_lowercase() || b == b'-');
                    (name_ok && value.len() <= 4096).then(|| value.into())
                }
                _ => Some(value.into()),
            });
        b
    }
}

/// HTML with everything outside the allowlist taken out.
pub(crate) fn clean(html: &str) -> String {
    builder().clean(html).to_string()
}

/// A page's HTML, its block placeholders kept.
pub(crate) fn clean_page(html: &str) -> String {
    page_builder().clean(html).to_string()
}
