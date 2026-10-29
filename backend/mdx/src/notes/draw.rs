//! The HTML for what the pass found. The classes and data attributes are
//! the ones Notes' stylesheet and page read (admin-ui/app/src/notes/); the
//! page turns a `.task-box` into a checkbox and a `.embed-media` into a
//! player, since no document may write a form field or a media element.

use super::pass::{
    attr_of, callout_title, CALLOUT, CALLOUT_TITLE, EMBED, MARK, TAG, TASK, WIKILINK,
};
use crate::mdast::{AttributeContent, Node};
use crate::render::{escape, slug};
use crate::{Hook, Writer};

pub(crate) struct Draw;

/// `Note#Heading|shown` into its parts. A pipe escaped for a table (`\|`)
/// counts as the pipe.
pub(crate) struct LinkParts {
    pub target: String,
    pub sub: String,
    pub alias: Option<String>,
}

pub(crate) fn parse_link(inner: &str) -> LinkParts {
    let text = inner.replace("\\|", "|");
    let (head, alias) = match text.split_once('|') {
        Some((h, a)) => (h, Some(a.trim().to_owned()).filter(|a| !a.is_empty())),
        None => (text.as_str(), None),
    };
    let (target, sub) = match head.split_once('#') {
        Some((t, s)) => (t, s.replace('#', " ")),
        None => (head, String::new()),
    };
    LinkParts {
        target: target.trim().to_owned(),
        sub: sub.split_whitespace().collect::<Vec<_>>().join(" "),
        alias,
    }
}

/// The text a link shows.
pub(crate) fn shown(link: &LinkParts) -> String {
    if let Some(a) = &link.alias {
        return a.clone();
    }
    let sub = link.sub.trim_start_matches('^');
    if link.target.is_empty() {
        sub.to_owned()
    } else if link.sub.is_empty() {
        link.target.clone()
    } else {
        format!("{} > {sub}", link.target)
    }
}

/// The id a `#part` of a note lands on: a block's `^id` as it is, a
/// heading by the renderer's own slug.
fn anchor(sub: &str) -> String {
    if sub.starts_with('^') {
        sub.to_owned()
    } else {
        slug(sub)
    }
}

fn attrs(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!(" {k}=\"{}\"", escape(v)))
        .collect()
}

fn ext(target: &str) -> String {
    target
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default()
}

const IMAGE: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "avif", "bmp", "svg"];
const AUDIO: &[&str] = &["mp3", "wav", "m4a", "ogg", "flac", "3gp", "webm"];
const VIDEO: &[&str] = &["mp4", "mov", "mkv", "ogv"];

/// `300` or `300x200`, an embed's size after the pipe.
fn size(alias: &str) -> Option<(String, Option<String>)> {
    let (w, h) = match alias.split_once('x') {
        Some((w, h)) => (w, Some(h)),
        None => (alias, None),
    };
    let digits = |s: &str| !s.is_empty() && s.len() <= 5 && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(w) || h.is_some_and(|h| !digits(h)) {
        return None;
    }
    Some((w.to_owned(), h.map(str::to_owned)))
}

fn children_of(node: &Node) -> &[Node] {
    node.children().map(Vec::as_slice).unwrap_or_default()
}

fn attributes_of(node: &Node) -> &[AttributeContent] {
    match node {
        Node::MdxJsxFlowElement(e) => &e.attributes,
        Node::MdxJsxTextElement(e) => &e.attributes,
        _ => &[],
    }
}

impl Hook for Draw {
    fn element<'a>(&self, w: &mut Writer<'a>, name: &str, node: &'a Node) -> bool {
        let a = attributes_of(node);
        let get = |k: &str| attr_of(a, k).unwrap_or_default().to_owned();
        match name {
            WIKILINK => wikilink(w, &parse_link(&get("link"))),
            EMBED => embed(w, &parse_link(&get("link"))),
            TAG => {
                let tag = get("tag");
                let href = format!("#tag/{tag}");
                w.raw(&format!(
                    "<a class=\"tag\"{}>#",
                    attrs(&[("href", &href), ("data-tag", &tag)])
                ));
                w.text(&tag);
                w.raw("</a>");
            }
            MARK => {
                w.raw("<mark>");
                w.children(node);
                w.raw("</mark>");
            }
            TASK => {
                let mark = get("mark");
                let done = mark != " ";
                w.raw(&format!(
                    "<span class=\"task-box\"{}>{}</span> ",
                    attrs(&[("data-task", &mark), ("data-line", &get("line"))]),
                    if done { "\u{2611}" } else { "\u{2610}" }
                ));
            }
            CALLOUT => callout(w, node, &get("kind"), &get("fold"), attr_of(a, "id")),
            // Drawn by its callout.
            CALLOUT_TITLE => w.children(node),
            _ => return false,
        }
        true
    }
}

fn wikilink(w: &mut Writer<'_>, link: &LinkParts) {
    let text = shown(link);
    if link.target.is_empty() {
        // [[#Heading]]: a place in this note.
        let href = format!("#{}", anchor(&link.sub));
        w.raw(&format!(
            "<a class=\"wikilink is-internal\"{}>",
            attrs(&[("href", &href), ("data-sub", &link.sub)])
        ));
        w.text(&text);
        w.raw("</a>");
        return;
    }
    w.link_found(&link.target, false);
    let resolver = w.resolver();
    let open = match resolver.note(&link.target) {
        Some(path) => {
            let href = resolver.href(&path, &link.sub);
            format!(
                "<a class=\"wikilink\"{}>",
                attrs(&[
                    ("href", &href),
                    ("data-note-path", &path),
                    ("data-sub", &link.sub)
                ])
            )
        }
        None => match resolver.file(&link.target) {
            Some(file) => format!(
                "<a class=\"wikilink is-file\"{}>",
                attrs(&[("href", &file)])
            ),
            None => format!(
                "<a class=\"wikilink is-unresolved\"{}>",
                attrs(&[("href", "#"), ("data-note-target", &link.target)])
            ),
        },
    };
    w.raw(&open);
    w.text(&text);
    w.raw("</a>");
}

fn embed(w: &mut Writer<'_>, link: &LinkParts) {
    w.link_found(&link.target, true);
    let e = ext(&link.target);
    let media = IMAGE.contains(&e.as_str())
        || AUDIO.contains(&e.as_str())
        || VIDEO.contains(&e.as_str())
        || e == "pdf";
    let resolver = w.resolver();
    if media {
        if let Some(file) = resolver.file(&link.target) {
            let sized = link.alias.as_deref().and_then(size);
            if IMAGE.contains(&e.as_str()) {
                let alt = match (&sized, &link.alias) {
                    (None, Some(a)) => a.clone(),
                    _ => link.target.clone(),
                };
                let (width, height) = sized.unwrap_or_default();
                let mut pairs = vec![("src", file.as_str()), ("alt", alt.as_str())];
                if !width.is_empty() {
                    pairs.push(("data-width", width.as_str()));
                }
                if let Some(h) = &height {
                    pairs.push(("data-height", h.as_str()));
                }
                w.raw(&format!("<img class=\"embed-image\"{}>", attrs(&pairs)));
            } else if e == "pdf" {
                w.raw(&format!(
                    "<a class=\"wikilink is-file\"{}>",
                    attrs(&[("href", &file)])
                ));
                w.text(link.alias.as_deref().unwrap_or(&link.target));
                w.raw("</a>");
            } else {
                let kind = if AUDIO.contains(&e.as_str()) {
                    "audio"
                } else {
                    "video"
                };
                w.raw(&format!(
                    "<span class=\"embed-media\"{}>",
                    attrs(&[("data-kind", kind), ("data-src", &file)])
                ));
                w.text(&link.target);
                w.raw("</span>");
            }
            return;
        }
    }
    match resolver.note(&link.target) {
        // Filled in by the page, which has the other note's text.
        Some(path) => w.raw(&format!(
            "<span class=\"note-embed\"{}></span>",
            attrs(&[("data-embed-path", &path), ("data-sub", &link.sub)])
        )),
        None => {
            w.raw(&format!(
                "<span class=\"embed-missing\"{}>",
                attrs(&[("data-note-target", &link.target)])
            ));
            w.text(&link.target);
            w.raw("</span>");
        }
    }
}

fn callout<'a>(w: &mut Writer<'a>, node: &'a Node, kind: &str, fold: &str, id: Option<&str>) {
    let kids = children_of(node);
    let mut pairs = vec![("data-callout", kind)];
    if let Some(id) = id {
        pairs.push(("id", id));
    }
    let class = format!("callout callout-{kind}");
    let (outer, head) = if fold.is_empty() {
        ("aside", "p")
    } else {
        ("details", "summary")
    };
    w.raw(&format!(
        "<{outer} class=\"{}\"{}{}><{head} class=\"callout-title\"><span class=\"callout-icon\"></span><span class=\"callout-title-inner\">",
        escape(&class),
        attrs(&pairs),
        if fold == "+" { " open" } else { "" }
    ));
    match kids.first() {
        Some(title) if !children_of(title).is_empty() => w.children(title),
        _ => w.text(&callout_title(kind)),
    }
    w.raw(&format!("</span></{head}><div class=\"callout-content\">"));
    for child in kids.iter().skip(1) {
        w.node(child);
    }
    w.raw(&format!("</div></{outer}>"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_parts() {
        let l = parse_link("Note#Part one#Two|shown");
        assert_eq!(
            (l.target.as_str(), l.sub.as_str(), l.alias.as_deref()),
            ("Note", "Part one Two", Some("shown"))
        );
        let l = parse_link("a\\|b");
        assert_eq!((l.target.as_str(), l.alias.as_deref()), ("a", Some("b")));
        assert_eq!(shown(&parse_link("#^blk")), "blk");
        assert_eq!(shown(&parse_link("N#^blk")), "N > blk");
    }
}
