//! Obsidian's Markdown on top of CommonMark, GFM and MDX: [[wikilinks]]
//! and ![[embeds]], #tags, ==highlights==, %%comments%%, callouts (and
//! GitHub's alerts, the same syntax), Obsidian's task marks, ^block ids
//! and single line breaks kept as breaks.
//!
//! markdown-rs has no syntax extensions, so this is a pass over the tree
//! it parsed. What the pass finds becomes elements in the reserved
//! `losos:` namespace, which an author cannot write (`parse` refuses
//! them), and `draw.rs` draws them. Attributes for elements the renderer
//! opens itself (a block's id, a task item's mark) go in the parse's
//! attribute table, keyed by the node's start offset and its tag.

use std::collections::BTreeMap;

use markdown::mdast::{
    AttributeContent, AttributeValue, ListItem, MdxJsxAttribute, MdxJsxFlowElement,
    MdxJsxTextElement, Node, Paragraph, Text,
};
use markdown::unist::Position;

/// Extra attributes for the elements the renderer opens, by start offset
/// and tag.
pub type Attributes = BTreeMap<(usize, String), Vec<(String, String)>>;

fn add(attributes: &mut Attributes, at: usize, tag: &str, name: &str, value: String) {
    attributes
        .entry((at, tag.to_owned()))
        .or_default()
        .push((name.to_owned(), value));
}

pub(crate) const WIKILINK: &str = "losos:wikilink";
pub(crate) const EMBED: &str = "losos:embed";
pub(crate) const TAG: &str = "losos:tag";
pub(crate) const MARK: &str = "losos:mark";
pub(crate) const CALLOUT: &str = "losos:callout";
pub(crate) const CALLOUT_TITLE: &str = "losos:callout-title";
pub(crate) const TASK: &str = "losos:task";

/// Take %%comments%% out, inline or over several lines, outside fenced
/// code. A removed line stays as an empty one, so positions keep their
/// lines; inside a line the comment is replaced by spaces of the same
/// length, so they keep their columns too.
pub(crate) fn strip_comments(text: &str) -> String {
    if !text.contains("%%") {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut fence: Option<(u8, usize)> = None;
    let mut open = false;
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if !open {
            if let Some(f) = fence_of(line) {
                match fence {
                    None => fence = Some(f),
                    Some((c, n)) if f.0 == c && is_close(line, c, n) => fence = None,
                    Some(_) => {}
                }
                out.push_str(line);
                continue;
            }
            if fence.is_some() {
                out.push_str(line);
                continue;
            }
        }
        let mut rest = line;
        loop {
            match rest.find("%%") {
                None => {
                    if open {
                        blank(&mut out, rest);
                    } else {
                        out.push_str(rest);
                    }
                    break;
                }
                Some(at) => {
                    if open {
                        blank(&mut out, &rest[..at + 2]);
                    } else {
                        out.push_str(&rest[..at]);
                        blank(&mut out, "%%");
                    }
                    open = !open;
                    rest = &rest[at + 2..];
                }
            }
        }
    }
    out
}

/// Spaces for each character, keeping a carriage return.
fn blank(out: &mut String, text: &str) {
    for c in text.chars() {
        out.push(if c == '\r' { '\r' } else { ' ' });
    }
}

/// A fence's character and length when the line opens or closes one.
fn fence_of(line: &str) -> Option<(u8, usize)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let c = *rest.as_bytes().first()?;
    if c != b'`' && c != b'~' {
        return None;
    }
    let n = rest.bytes().take_while(|&b| b == c).count();
    (n >= 3).then_some((c, n))
}

fn is_close(line: &str, c: u8, n: usize) -> bool {
    let t = line.trim();
    t.len() >= n && t.bytes().all(|b| b == c)
}

/// Run the pass over a parsed document.
pub(crate) fn apply(root: &mut Node, attributes: &mut Attributes) {
    block(root, attributes);
}

fn attr(name: &str, value: &str) -> AttributeContent {
    AttributeContent::Property(MdxJsxAttribute {
        name: name.to_owned(),
        value: Some(AttributeValue::Literal(value.to_owned())),
    })
}

fn text_el(
    name: &str,
    attrs: Vec<AttributeContent>,
    children: Vec<Node>,
    position: Option<Position>,
) -> Node {
    Node::MdxJsxTextElement(MdxJsxTextElement {
        children,
        position,
        name: Some(name.to_owned()),
        attributes: attrs,
    })
}

fn flow_el(
    name: &str,
    attrs: Vec<AttributeContent>,
    children: Vec<Node>,
    position: Option<Position>,
) -> Node {
    Node::MdxJsxFlowElement(MdxJsxFlowElement {
        children,
        position,
        name: Some(name.to_owned()),
        attributes: attrs,
    })
}

/// A node's start offset, when it has a position.
fn start(node: &Node) -> Option<usize> {
    node.position().map(|p| p.start.offset)
}

/// Flow: callouts, task items and block ids, then the text inside.
fn block(node: &mut Node, attributes: &mut Attributes) {
    match node {
        Node::Code(_) | Node::Math(_) | Node::Html(_) | Node::Yaml(_) | Node::Toml(_) => return,
        Node::Link(_) | Node::LinkReference(_) | Node::InlineCode(_) | Node::InlineMath(_) => {
            return
        }
        // What the pass made: their text is already read.
        Node::MdxJsxTextElement(e)
            if matches!(e.name.as_deref(), Some(WIKILINK | EMBED | TAG | TASK)) =>
        {
            return
        }
        _ => {}
    }
    if let Node::Blockquote(_) = node {
        if let Some(callout) = callout(node) {
            *node = callout;
        }
    }
    if let Node::List(list) = node {
        let tasks = list
            .children
            .iter_mut()
            .fold(false, |any, item| match item {
                Node::ListItem(i) => task(i, attributes) || any,
                _ => any,
            });
        if let (true, Some(at)) = (tasks, list.position.as_ref().map(|p| p.start.offset)) {
            let tag = if list.ordered { "ol" } else { "ul" };
            add(attributes, at, tag, "class", "contains-task-list".into());
        }
    }
    let Some(children) = node.children_mut() else {
        return;
    };
    // Phrasing holders get the inline pass.
    let phrasing = children.iter().any(is_phrasing);
    if phrasing {
        let taken = std::mem::take(children);
        *children = inline(taken);
    }
    for child in children.iter_mut() {
        block(child, attributes);
    }
    block_ids(node, attributes);
}

fn is_phrasing(n: &Node) -> bool {
    matches!(
        n,
        Node::Text(_)
            | Node::Emphasis(_)
            | Node::Strong(_)
            | Node::Delete(_)
            | Node::InlineCode(_)
            | Node::InlineMath(_)
            | Node::Break(_)
            | Node::Link(_)
            | Node::LinkReference(_)
            | Node::Image(_)
            | Node::ImageReference(_)
            | Node::FootnoteReference(_)
            | Node::MdxJsxTextElement(_)
            | Node::MdxTextExpression(_)
            | Node::Html(_)
    )
}

// ── Callouts ────────────────────────────────────────────────────────────

/// `> [!type]± Title`: the blockquote as a callout, or None.
fn callout(quote: &Node) -> Option<Node> {
    let Node::Blockquote(q) = quote else {
        return None;
    };
    let Some(Node::Paragraph(p)) = q.children.first() else {
        return None;
    };
    let Some(Node::Text(first)) = p.children.first() else {
        return None;
    };
    let line_end = first.value.find('\n').unwrap_or(first.value.len());
    let head = &first.value[..line_end];
    let (kind, fold, title_text) = callout_marker(head)?;
    // The title: the rest of the marker line, with whatever inline nodes
    // follow on that line when the text node ends before the line does.
    let mut title: Vec<Node> = Vec::new();
    let mut rest: Vec<Node> = Vec::new();
    let mut in_title = true;
    if !title_text.is_empty() {
        title.push(Node::Text(Text {
            value: title_text.to_owned(),
            position: None,
        }));
    }
    if line_end < first.value.len() {
        in_title = false;
        let after = &first.value[line_end + 1..];
        if !after.is_empty() {
            rest.push(Node::Text(Text {
                value: after.to_owned(),
                position: None,
            }));
        }
    }
    for n in p.children.iter().skip(1) {
        if in_title {
            if let Node::Text(t) = n {
                if let Some(at) = t.value.find('\n') {
                    if at > 0 {
                        title.push(Node::Text(Text {
                            value: t.value[..at].to_owned(),
                            position: None,
                        }));
                    }
                    let after = &t.value[at + 1..];
                    if !after.is_empty() {
                        rest.push(Node::Text(Text {
                            value: after.to_owned(),
                            position: None,
                        }));
                    }
                    in_title = false;
                    continue;
                }
            }
            if let Node::Break(_) = n {
                in_title = false;
                continue;
            }
            title.push(n.clone());
        } else {
            rest.push(n.clone());
        }
    }
    if let Some(Node::Text(t)) = title.last_mut() {
        t.value = t.value.trim_end().to_owned();
        if t.value.is_empty() {
            title.pop();
        }
    }
    let mut children = vec![flow_el(CALLOUT_TITLE, vec![], inline(title), None)];
    if !rest.is_empty() {
        children.push(Node::Paragraph(Paragraph {
            children: rest,
            position: p.position.clone(),
        }));
    }
    children.extend(q.children.iter().skip(1).cloned());
    let mut attrs = vec![attr("kind", &kind)];
    if !fold.is_empty() {
        attrs.push(attr("fold", fold));
    }
    Some(flow_el(CALLOUT, attrs, children, q.position.clone()))
}

/// `[!type]±` and the title after it.
fn callout_marker(line: &str) -> Option<(String, &str, &str)> {
    let rest = line.strip_prefix("[!")?;
    let close = rest.find(']')?;
    let kind = &rest[..close];
    if kind.is_empty()
        || !kind
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return None;
    }
    let mut after = &rest[close + 1..];
    let fold = if after.starts_with('+') || after.starts_with('-') {
        let f = &after[..1];
        after = &after[1..];
        f
    } else {
        ""
    };
    Some((kind.to_lowercase(), fold, after.trim_start()))
}

/// Obsidian's names for the kinds, for a callout without a title.
pub(crate) fn callout_title(kind: &str) -> String {
    let known = match kind {
        "note" => "Note",
        "abstract" => "Abstract",
        "summary" => "Summary",
        "tldr" => "TL;DR",
        "info" => "Info",
        "todo" => "To do",
        "tip" => "Tip",
        "hint" => "Hint",
        "important" => "Important",
        "success" => "Success",
        "check" => "Check",
        "done" => "Done",
        "question" => "Question",
        "help" => "Help",
        "faq" => "FAQ",
        "warning" => "Warning",
        "caution" => "Caution",
        "attention" => "Attention",
        "failure" => "Failure",
        "fail" => "Fail",
        "missing" => "Missing",
        "danger" => "Danger",
        "error" => "Error",
        "bug" => "Bug",
        "example" => "Example",
        "quote" => "Quote",
        "cite" => "Cite",
        _ => "",
    };
    if !known.is_empty() {
        return known.to_owned();
    }
    let mut c = kind.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

// ── Tasks ───────────────────────────────────────────────────────────────

const TASK_MARKS: &str = " xX/-><!?*\"lbiSIpcfkwud";

/// `- [ ]`, `- [x]` and Obsidian's other marks: a box at the start of the
/// item, and the item marked for the stylesheet.
/// Says whether the item is a task. Items are seen once, from their list.
fn task(item: &mut ListItem, attributes: &mut Attributes) -> bool {
    let line = item
        .position
        .as_ref()
        .map(|p| p.start.line.saturating_sub(1));
    let mark = match item.checked {
        Some(true) => Some('x'),
        Some(false) => Some(' '),
        None => strip_mark(item),
    };
    let Some(mark) = mark else { return false };
    // Drawn here, with its line, rather than as the renderer's glyph.
    item.checked = None;
    if let Some(at) = item.position.as_ref().map(|p| p.start.offset) {
        add(attributes, at, "li", "class", "task-list-item".into());
        add(attributes, at, "li", "data-task", mark.to_string());
    }
    let mut attrs = vec![attr("mark", &mark.to_string())];
    if let Some(line) = line {
        attrs.push(attr("line", &line.to_string()));
    }
    let boxed = text_el(TASK, attrs, vec![], None);
    match item.children.first_mut() {
        Some(Node::Paragraph(p)) => p.children.insert(0, boxed),
        _ => item.children.insert(
            0,
            Node::Paragraph(Paragraph {
                children: vec![boxed],
                position: None,
            }),
        ),
    }
    true
}

/// An Obsidian mark GFM does not know (`[/]`, `[-]`, …), taken off the text.
fn strip_mark(item: &mut ListItem) -> Option<char> {
    let Some(Node::Paragraph(p)) = item.children.first_mut() else {
        return None;
    };
    let Some(Node::Text(t)) = p.children.first_mut() else {
        return None;
    };
    let b = t.value.as_bytes();
    if b.len() < 4 || b[0] != b'[' || b[2] != b']' || !(b[3] == b' ' || b[3] == b'\t') {
        return None;
    }
    let mark = b[1] as char;
    if !TASK_MARKS.contains(mark) {
        return None;
    }
    t.value = t.value[4..].to_owned();
    if t.value.is_empty() {
        p.children.remove(0);
    }
    Some(mark)
}

// ── Block ids ───────────────────────────────────────────────────────────

/// `^id` at the end of a block's last line names the block, so
/// [[note#^id]] lands on it; on a paragraph of its own it names the block
/// before.
fn block_ids(parent: &mut Node, attributes: &mut Attributes) {
    let is_item = matches!(parent, Node::ListItem(_));
    let item_at = start(parent);
    let Some(children) = parent.children_mut() else {
        return;
    };
    let mut i = 0;
    while i < children.len() {
        let alone = match &children[i] {
            Node::Paragraph(p) => alone_id(p),
            _ => None,
        };
        if let (Some(id), true) = (alone, i > 0) {
            name_block(&mut children[i - 1], &id, attributes);
            children.remove(i);
            continue;
        }
        if let Node::Paragraph(p) = &mut children[i] {
            if let Some(id) = trailing_id(p) {
                // A list item's own paragraph may be written without its
                // <p>, so the id goes on the item.
                let (at, tag) = if is_item && i == 0 {
                    (item_at, "li")
                } else {
                    (p.position.as_ref().map(|x| x.start.offset), "p")
                };
                if let Some(at) = at {
                    add(attributes, at, tag, "id", format!("^{id}"));
                }
            }
        }
        i += 1;
    }
}

/// Gives the block before a lone `^id` its id: by the tag it is written
/// as, or on a callout, which draw.rs writes, as its own attribute.
fn name_block(block: &mut Node, id: &str, attributes: &mut Attributes) {
    let id = format!("^{id}");
    let tag = match block {
        Node::Paragraph(_) => "p",
        Node::List(l) if l.ordered => "ol",
        Node::List(_) => "ul",
        Node::Table(_) => "table",
        Node::Blockquote(_) => "blockquote",
        Node::Code(_) => "pre",
        Node::Heading(h) => match h.depth {
            1 => "h1",
            2 => "h2",
            3 => "h3",
            4 => "h4",
            5 => "h5",
            _ => "h6",
        },
        Node::MdxJsxFlowElement(e) if e.name.as_deref() == Some(CALLOUT) => {
            e.attributes.push(attr("id", &id));
            return;
        }
        _ => return,
    };
    if let Some(at) = start(block) {
        add(attributes, at, tag, "id", id);
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// A paragraph that is only `^id`.
fn alone_id(p: &Paragraph) -> Option<String> {
    let [Node::Text(t)] = p.children.as_slice() else {
        return None;
    };
    let v = t.value.trim_end();
    let id = v.strip_prefix('^')?;
    valid_id(id).then(|| id.to_owned())
}

/// ` ^id` at the end of the paragraph's last text, taken off it.
fn trailing_id(p: &mut Paragraph) -> Option<String> {
    let Some(Node::Text(t)) = p.children.last_mut() else {
        return None;
    };
    let v = t.value.trim_end_matches([' ', '\t']);
    let caret = v.rfind('^')?;
    let id = &v[caret + 1..];
    if !valid_id(id) {
        return None;
    }
    let before = &v[..caret];
    if !(before.ends_with(' ') || before.ends_with('\t')) {
        return None;
    }
    let id = id.to_owned();
    t.value = before.trim_end_matches([' ', '\t']).to_owned();
    if t.value.is_empty() {
        p.children.pop();
    }
    Some(id)
}

// ── Inline ──────────────────────────────────────────────────────────────

/// Wikilinks, embeds, tags, line breaks and highlights in a run of
/// phrasing.
fn inline(children: Vec<Node>) -> Vec<Node> {
    let merged = merge_text(children);
    let mut out = Vec::with_capacity(merged.len());
    // The character before a text node, for a tag's left edge: a
    // non-text node counts as a letter.
    let mut before: char = ' ';
    for n in merged {
        match n {
            Node::Text(t) => {
                let last = t.value.chars().last();
                split_text(&t.value, before, &mut out);
                before = last.unwrap_or(before);
            }
            Node::Break(_) => {
                before = ' ';
                out.push(n);
            }
            other => {
                before = 'a';
                out.push(other);
            }
        }
    }
    highlights(merge_text(out))
}

/// Neighbouring text nodes as one.
fn merge_text(children: Vec<Node>) -> Vec<Node> {
    let mut out: Vec<Node> = Vec::with_capacity(children.len());
    for n in children {
        if let (Node::Text(t), Some(Node::Text(prev))) = (&n, out.last_mut()) {
            prev.value.push_str(&t.value);
            prev.position = None;
            continue;
        }
        out.push(n);
    }
    out
}

fn push_text(out: &mut Vec<Node>, text: &str) {
    if !text.is_empty() {
        out.push(Node::Text(Text {
            value: text.to_owned(),
            position: None,
        }));
    }
}

/// One text node into text, links, embeds, tags and breaks.
fn split_text(text: &str, mut before: char, out: &mut Vec<Node>) {
    let mut plain_from = 0;
    let mut i = 0;
    let bytes = text.as_bytes();
    while i < bytes.len() {
        let c = bytes[i];
        // [[link]] and ![[embed]]
        if c == b'[' || (c == b'!' && bytes.get(i + 1) == Some(&b'[')) {
            let embed = c == b'!';
            let open = if embed { i + 1 } else { i };
            if bytes.get(open + 1) == Some(&b'[') {
                if let Some(len) = text[open + 2..].find("]]") {
                    let inner = &text[open + 2..open + 2 + len];
                    if !inner.contains('\n') && !inner.trim().is_empty() && !inner.contains("[[") {
                        push_text(out, &text[plain_from..i]);
                        // A link's children are the text it shows, for the
                        // outline and heading ids; draw.rs writes its own.
                        let (name, kids) = if embed {
                            (EMBED, vec![])
                        } else {
                            let shown = super::draw::shown(&super::draw::parse_link(inner));
                            (
                                WIKILINK,
                                vec![Node::Text(Text {
                                    value: shown,
                                    position: None,
                                })],
                            )
                        };
                        out.push(text_el(name, vec![attr("link", inner)], kids, None));
                        i = open + 2 + len + 2;
                        plain_from = i;
                        before = ']';
                        continue;
                    }
                }
            }
        }
        // A line break: kept, as Obsidian keeps it.
        if c == b'\n' {
            push_text(out, &text[plain_from..i]);
            out.push(Node::Break(markdown::mdast::Break { position: None }));
            i += 1;
            plain_from = i;
            before = ' ';
            continue;
        }
        // #tag
        if c == b'#' && (before.is_whitespace() || "([{,;".contains(before)) {
            if let Some(len) = tag_len(&text[i + 1..]) {
                push_text(out, &text[plain_from..i]);
                let tag = &text[i + 1..i + 1 + len];
                let shown = Node::Text(Text {
                    value: format!("#{tag}"),
                    position: None,
                });
                out.push(text_el(TAG, vec![attr("tag", tag)], vec![shown], None));
                i += 1 + len;
                plain_from = i;
                before = 'a';
                continue;
            }
        }
        let ch = text[i..].chars().next().unwrap_or(' ');
        before = ch;
        i += ch.len_utf8();
    }
    push_text(out, &text[plain_from..]);
}

/// A tag's length after the `#`: letters, digits, `_`, `-` and `/`, not
/// only digits.
pub(crate) fn tag_len(rest: &str) -> Option<usize> {
    let mut len = 0;
    let mut word = false;
    for c in rest.chars() {
        if c.is_alphabetic() || c == '_' || c == '-' || c == '/' {
            word = true;
        } else if !c.is_numeric() {
            break;
        }
        len += c.len_utf8();
    }
    (len > 0 && word).then_some(len)
}

/// ==highlight==, which may hold other inline nodes.
fn highlights(children: Vec<Node>) -> Vec<Node> {
    if !children
        .iter()
        .any(|n| matches!(n, Node::Text(t) if t.value.contains("==")))
    {
        return children;
    }
    // Text split at every `==`, the markers kept apart.
    enum Item {
        Node(Node),
        Marker,
    }
    let mut items: Vec<Item> = Vec::new();
    for n in children {
        match n {
            Node::Text(t) if t.value.contains("==") => {
                let mut rest = t.value.as_str();
                while let Some(at) = rest.find("==") {
                    if at > 0 {
                        items.push(Item::Node(Node::Text(Text {
                            value: rest[..at].to_owned(),
                            position: None,
                        })));
                    }
                    items.push(Item::Marker);
                    rest = &rest[at + 2..];
                }
                if !rest.is_empty() {
                    items.push(Item::Node(Node::Text(Text {
                        value: rest.to_owned(),
                        position: None,
                    })));
                }
            }
            other => items.push(Item::Node(other)),
        }
    }
    let starts_solid = |i: Option<&Item>| match i {
        Some(Item::Node(Node::Text(t))) => {
            t.value.chars().next().is_some_and(|c| !c.is_whitespace())
        }
        Some(Item::Node(Node::Break(_))) | None | Some(Item::Marker) => false,
        Some(Item::Node(_)) => true,
    };
    let ends_solid = |i: Option<&Item>| match i {
        Some(Item::Node(Node::Text(t))) => {
            t.value.chars().last().is_some_and(|c| !c.is_whitespace())
        }
        Some(Item::Node(Node::Break(_))) | None | Some(Item::Marker) => false,
        Some(Item::Node(_)) => true,
    };
    let mut out: Vec<Node> = Vec::new();
    let mut i = 0;
    while i < items.len() {
        if let Item::Marker = items[i] {
            if starts_solid(items.get(i + 1)) {
                // The nearest closing marker on the same line.
                let mut j = i + 1;
                let mut close = None;
                while j < items.len() {
                    match &items[j] {
                        Item::Marker if ends_solid(items.get(j - 1)) => {
                            close = Some(j);
                            break;
                        }
                        Item::Node(Node::Break(_)) => break,
                        _ => {}
                    }
                    j += 1;
                }
                if let Some(j) = close {
                    let inner: Vec<Node> = items
                        .drain(i + 1..j)
                        .map(|it| match it {
                            Item::Node(n) => n,
                            Item::Marker => Node::Text(Text {
                                value: "==".into(),
                                position: None,
                            }),
                        })
                        .collect();
                    // items[i] is the opener, items[i+1] now the closer.
                    items.remove(i + 1);
                    out.push(text_el(MARK, vec![], merge_text(inner), None));
                    i += 1;
                    continue;
                }
            }
            push_text(&mut out, "==");
            i += 1;
            continue;
        }
        if let Item::Node(n) = std::mem::replace(&mut items[i], Item::Marker) {
            out.push(n);
        }
        i += 1;
    }
    merge_text(out)
}

/// The value of a `losos:` element's attribute.
pub(crate) fn attr_of<'a>(attrs: &'a [AttributeContent], name: &str) -> Option<&'a str> {
    attrs.iter().find_map(|a| match a {
        AttributeContent::Property(p) if p.name == name => match &p.value {
            Some(AttributeValue::Literal(v)) => Some(v.as_str()),
            _ => None,
        },
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Flavour;

    fn parse(src: &str) -> (Node, Attributes) {
        let mut parsed = crate::parse(&strip_comments(src), Flavour::Md).unwrap();
        apply(&mut parsed.tree, &mut parsed.attributes);
        (parsed.tree, parsed.attributes)
    }

    /// The tree as a compact string: elements as `<name attrs>…</>`.
    fn show(n: &Node) -> String {
        let kids = |n: &Node| {
            n.children()
                .map(|c| c.iter().map(show).collect::<String>())
                .unwrap_or_default()
        };
        match n {
            Node::Text(t) => t.value.clone(),
            Node::Break(_) => "⏎".into(),
            Node::MdxJsxTextElement(e) => {
                let a: Vec<String> = e
                    .attributes
                    .iter()
                    .filter_map(|a| match a {
                        AttributeContent::Property(p) => Some(format!(
                            "{}={}",
                            p.name,
                            match &p.value {
                                Some(AttributeValue::Literal(v)) => v.clone(),
                                _ => String::new(),
                            }
                        )),
                        _ => None,
                    })
                    .collect();
                format!(
                    "<{} {}>{}</>",
                    e.name.as_deref().unwrap_or(""),
                    a.join(" "),
                    kids(n)
                )
            }
            Node::MdxJsxFlowElement(e) => {
                let a: Vec<String> = e
                    .attributes
                    .iter()
                    .filter_map(|a| match a {
                        AttributeContent::Property(p) => Some(format!(
                            "{}={}",
                            p.name,
                            match &p.value {
                                Some(AttributeValue::Literal(v)) => v.clone(),
                                _ => String::new(),
                            }
                        )),
                        _ => None,
                    })
                    .collect();
                format!(
                    "[{} {}]{}[/]",
                    e.name.as_deref().unwrap_or(""),
                    a.join(" "),
                    kids(n)
                )
            }
            Node::Emphasis(_) => format!("*{}*", kids(n)),
            Node::Paragraph(_) => format!("¶{}¶", kids(n)),
            Node::InlineCode(c) => format!("`{}`", c.value),
            Node::Code(c) => c.value.clone(),
            _ => kids(n),
        }
    }

    #[test]
    fn links_embeds_tags() {
        let (n, _) =
            parse("See [[Note#H|alias]] and ![[p.png|300]] #tag, #123 not, a#b not, (#in/side)");
        assert_eq!(
            show(&n),
            "¶See <losos:wikilink link=Note#H|alias>alias</> and <losos:embed link=p.png|300></> <losos:tag tag=tag>#tag</>, #123 not, a#b not, (<losos:tag tag=in/side>#in/side</>)¶"
        );
    }

    #[test]
    fn code_is_left_alone() {
        let (n, _) = parse("`[[x]] #y` ==z==\n\n```\n[[a]] %% b %%\n```");
        assert_eq!(show(&n), "¶`[[x]] #y` <losos:mark >z</>¶[[a]] %% b %%");
    }

    #[test]
    fn highlights_hold_other_nodes() {
        let (n, _) = parse("==hi *x*== and == not== and ==a\nb==");
        assert_eq!(
            show(&n),
            "¶<losos:mark >hi *x*</> and == not== and ==a⏎b==¶"
        );
    }

    #[test]
    fn comments_go_lines_stay() {
        assert_eq!(
            strip_comments("a %%x%% b\n%%\ngone\n%%\nc"),
            "a       b\n  \n    \n  \nc"
        );
        assert_eq!(strip_comments("```\n%%kept%%\n```"), "```\n%%kept%%\n```");
    }

    #[test]
    fn callouts_and_alerts() {
        let (n, _) = parse("> [!tip]- Grow *fast*\n> Body [[x]]\n\n> [!NOTE]\n> GitHub\n\n> plain");
        assert_eq!(
            show(&n),
            "[losos:callout kind=tip fold=-][losos:callout-title ]Grow *fast*[/]¶Body <losos:wikilink link=x>x</>¶[/][losos:callout kind=note][losos:callout-title ][/]¶GitHub¶[/]¶plain¶"
        );
    }

    #[test]
    fn tasks_with_obsidian_marks() {
        let (n, a) = parse("- [ ] one\n- [x] two\n- [/] half\n- plain");
        assert_eq!(
            show(&n),
            "¶<losos:task mark=  line=0></>one¶¶<losos:task mark=x line=1></>two¶¶<losos:task mark=/ line=2></>half¶¶plain¶"
        );
        let marks: Vec<String> = a
            .values()
            .flatten()
            .filter(|(k, _)| k == "data-task")
            .map(|(_, v)| v.clone())
            .collect();
        assert_eq!(marks, [" ", "x", "/"]);
    }

    #[test]
    fn block_ids() {
        let (n, a) = parse("Para one ^p1\n\n- item ^i1\n\n| a |\n|---|\n| b |\n\n^t1");
        assert_eq!(show(&n), "¶Para one¶¶item¶ab");
        let ids: Vec<&str> = a
            .values()
            .flatten()
            .filter(|(k, _)| k == "id")
            .map(|(_, v)| v.as_str())
            .collect();
        assert_eq!(ids, ["^p1", "^i1", "^t1"]);
        let (n, _) = parse("price ^ not, x^2 ^no_good");
        assert_eq!(show(&n), "¶price ^ not, x^2 ^no_good¶");
    }
}
