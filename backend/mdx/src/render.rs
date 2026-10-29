//! mdast to HTML. Every tag and attribute the output can hold is written
//! here, by name; the document supplies only text, which is escaped, and
//! the strings the component table lets through. sanitize.rs then checks
//! the result once more.

use crate::components::{self, Component, Prop};
use crate::{Flavour, Heading, Link, Parsed, Place, Problem, Resolver};
use markdown::mdast::{self, AlignKind, AttributeContent, AttributeValue, Node};
use std::collections::{BTreeMap, HashMap};

/// The namespace Notes' own syntax is written in, by a pass over the tree.
/// An author may not use it.
pub const RESERVED: &str = "losos:";

/// Deeper than this, elements are drawn as text. `parse` already refuses a
/// tree past `MAX_NESTING`; this holds for what a pass adds after it.
const MAX_DEPTH: usize = crate::MAX_NESTING + 28;

/// Draws the reserved `losos:` elements a pass put in the tree.
pub trait Hook {
    /// Writes `node`, a reserved element named `name`, and says whether it
    /// did. One it does not know is drawn as its children.
    fn element<'a>(&self, w: &mut Writer<'a>, name: &str, node: &'a Node) -> bool;
}

pub(crate) struct Output {
    pub html: String,
    pub headings: Vec<Heading>,
    pub links: Vec<Link>,
    pub problems: Vec<Problem>,
}

/// The HTML being written, handed to a [`Hook`].
pub struct Writer<'a> {
    out: String,
    flavour: Flavour,
    resolver: &'a dyn Resolver,
    hook: Option<&'a dyn Hook>,
    attributes: &'a BTreeMap<(usize, String), Vec<(String, String)>>,
    definitions: HashMap<String, (String, Option<String>)>,
    footnotes: Vec<(String, &'a mdast::FootnoteDefinition)>,
    footnote_numbers: HashMap<String, usize>,
    slugs: HashMap<String, usize>,
    headings: Vec<Heading>,
    links: Vec<Link>,
    problems: Vec<Problem>,
    depth: usize,
    /// In a tight list, a paragraph is written without its `<p>`.
    tight: bool,
    /// Inside `<Row header>`, cells are `<th>`.
    header_row: bool,
}

/// Where `<TableOfContents>` puts the headings, once they are all known.
const TOC_MARK: &str = "<!--losos-toc-->";

pub(crate) fn render(parsed: &Parsed, resolver: &dyn Resolver, hook: Option<&dyn Hook>) -> Output {
    let mut w = Writer {
        out: String::new(),
        flavour: parsed.flavour,
        resolver,
        hook,
        attributes: &parsed.attributes,
        definitions: HashMap::new(),
        footnotes: Vec::new(),
        footnote_numbers: HashMap::new(),
        slugs: HashMap::new(),
        headings: Vec::new(),
        links: Vec::new(),
        problems: Vec::new(),
        depth: 0,
        tight: false,
        header_row: false,
    };
    w.collect(&parsed.tree);
    w.node(&parsed.tree);
    w.footnote_section();
    if w.out.contains(TOC_MARK) {
        let toc = toc(&w.headings);
        w.out = w.out.replace(TOC_MARK, &toc);
    }
    Output {
        html: w.out,
        headings: w.headings,
        links: w.links,
        problems: w.problems,
    }
}

/// Refuses the reserved namespace in what an author wrote: such an element
/// becomes a fragment (its children stay) and a problem.
pub(crate) fn refuse_reserved(node: &mut Node, problems: &mut Vec<Problem>) {
    let (name, position) = match node {
        Node::MdxJsxFlowElement(e) => (&mut e.name, e.position.as_ref()),
        Node::MdxJsxTextElement(e) => (&mut e.name, e.position.as_ref()),
        _ => (&mut None, None),
    };
    if name.as_deref().is_some_and(|n| n.starts_with(RESERVED)) {
        problems.push(Problem {
            place: place(position),
            message: format!(
                "<{}> is reserved for LosOS",
                name.as_deref().unwrap_or_default()
            ),
        });
        *name = None;
    }
    if let Some(children) = node.children_mut() {
        for child in children {
            refuse_reserved(child, problems);
        }
    }
}

fn place(position: Option<&markdown::unist::Position>) -> Place {
    position
        .map(|p| Place {
            line: p.start.line,
            column: p.start.column,
        })
        .unwrap_or_default()
}

/// Text for HTML, inside an element or a quoted attribute.
pub fn escape(text: &str) -> String {
    let mut s = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => s.push_str("&amp;"),
            '<' => s.push_str("&lt;"),
            '>' => s.push_str("&gt;"),
            '"' => s.push_str("&quot;"),
            '\'' => s.push_str("&#39;"),
            _ => s.push(c),
        }
    }
    s
}

/// A heading's id, the way GitHub and Obsidian make one: lower case, words
/// joined by `-`, punctuation dropped.
pub fn slug(text: &str) -> String {
    let mut s = String::new();
    for c in text.trim().chars() {
        if c.is_alphanumeric() || c == '_' || c == '-' {
            s.extend(c.to_lowercase());
        } else if c.is_whitespace() {
            s.push('-');
        }
    }
    s
}

/// The text of a node and everything under it.
pub fn plain_text(node: &Node) -> String {
    match node {
        Node::Text(t) => t.value.clone(),
        Node::InlineCode(c) => c.value.clone(),
        Node::InlineMath(m) => m.value.clone(),
        Node::Image(i) => i.alt.clone(),
        _ => node
            .children()
            .map(|c| c.iter().map(plain_text).collect())
            .unwrap_or_default(),
    }
}

fn external(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")
}

/// Whether a URL names a scheme (`x:` before any `/`, `?` or `#`).
fn has_scheme(url: &str) -> bool {
    match url.find(':') {
        Some(i) => !url[..i].contains(['/', '?', '#']),
        None => false,
    }
}

fn is_note(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    [".md", ".mdx", ".note", ".paper", ".slides"]
        .iter()
        .any(|e| lower.ends_with(e))
}

impl<'a> Writer<'a> {
    /// The resolver the caller passed.
    pub fn resolver(&self) -> &'a dyn Resolver {
        self.resolver
    }

    /// Writes HTML as it is. The allowlist still runs over all of it.
    pub fn raw(&mut self, html: &str) {
        self.out.push_str(html);
    }

    /// Writes text, escaped.
    pub fn text(&mut self, text: &str) {
        self.out.push_str(&escape(text));
    }

    /// Writes a node's children.
    pub fn children(&mut self, node: &'a Node) {
        if let Some(children) = node.children() {
            for c in children {
                self.node(c);
            }
        }
    }

    /// Records a link or embed for the result's `links`.
    pub fn link_found(&mut self, target: &str, embed: bool) {
        self.links.push(Link {
            target: target.to_string(),
            embed,
            external: external(target),
        });
    }

    /// Records a problem at a node.
    pub fn problem(&mut self, node: &Node, message: impl Into<String>) {
        self.problems.push(Problem {
            place: place(node.position()),
            message: message.into(),
        });
    }

    /// A string attribute of a JSX element, as written.
    pub fn attr(node: &Node, name: &str) -> Option<String> {
        jsx_attributes(node)?.iter().find_map(|a| match a {
            AttributeContent::Property(p) if p.name == name => match &p.value {
                Some(AttributeValue::Literal(v)) => Some(v.clone()),
                None => Some(String::new()),
                _ => None,
            },
            _ => None,
        })
    }

    /// Opens a tag with the attributes a pass added for `node`.
    fn open(&mut self, tag: &str, node: &Node, attrs: &[(&str, String)]) {
        self.out.push('<');
        self.out.push_str(tag);
        let mut class: Vec<String> = Vec::new();
        for (k, v) in attrs {
            if *k == "class" {
                class.push(v.clone());
            } else {
                self.out.push_str(&format!(" {k}=\"{}\"", escape(v)));
            }
        }
        let start = node.position().map(|p| p.start.offset);
        if let Some(extra) = start.and_then(|s| self.attributes.get(&(s, tag.to_string()))) {
            for (k, v) in extra {
                if k == "class" {
                    class.push(v.clone());
                } else if k == "id"
                    || k.starts_with("data-")
                        && k.len() > 5
                        && k[5..].bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
                {
                    self.out.push_str(&format!(" {k}=\"{}\"", escape(v)));
                }
            }
        }
        if !class.is_empty() {
            self.out
                .push_str(&format!(" class=\"{}\"", escape(&class.join(" "))));
        }
        self.out.push('>');
    }

    fn close(&mut self, tag: &str) {
        self.out.push_str(&format!("</{tag}>"));
    }

    /// Definitions and footnotes, gathered before writing, since a
    /// reference may come before what it names.
    fn collect(&mut self, node: &'a Node) {
        match node {
            Node::Definition(d) => {
                self.definitions
                    .entry(d.identifier.to_lowercase())
                    .or_insert((d.url.clone(), d.title.clone()));
            }
            Node::FootnoteDefinition(f)
                if !self.footnotes.iter().any(|(id, _)| *id == f.identifier) =>
            {
                self.footnotes.push((f.identifier.clone(), f));
            }
            _ => {}
        }
        if let Some(children) = node.children() {
            for c in children {
                self.collect(c);
            }
        }
    }

    /// Where a link goes: a scheme or `#` as written, a note through the
    /// resolver, anything else as a file. A scheme other than http, https
    /// or mailto is a problem; the allowlist drops the URL either way.
    fn href(&mut self, node: &Node, url: &str, embed: bool) -> String {
        self.link_found(url, embed);
        if has_scheme(url.trim()) && !external(url.trim()) {
            self.problem(node, format!("{url} is not a link a page may follow"));
            // Not left to the allowlist: a data-* attribute is not a URL to
            // it, and a page reads <Chart>'s source from one.
            return "#".to_string();
        }
        if url.starts_with('#') || has_scheme(url) {
            return url.to_string();
        }
        let (path, sub) = url.split_once('#').unwrap_or((url, ""));
        if !embed && is_note(path) {
            if let Some(p) = self.resolver.note(path) {
                return self.resolver.href(&p, sub);
            }
            return url.to_string();
        }
        self.resolver.file(path).unwrap_or_else(|| url.to_string())
    }

    pub(crate) fn node(&mut self, node: &'a Node) {
        if self.depth >= MAX_DEPTH {
            if self.depth == MAX_DEPTH {
                self.problem(node, "nested too deep; the rest is shown as text");
            }
            self.text(&plain_text(node));
            return;
        }
        self.depth += 1;
        self.node_inner(node);
        self.depth -= 1;
    }

    fn node_inner(&mut self, node: &'a Node) {
        match node {
            Node::Root(_) => {
                self.refuse_esm(node);
                self.blocks(node)
            }
            Node::Paragraph(_) => {
                if self.tight || only_blocks(node) {
                    self.children(node);
                } else {
                    self.open("p", node, &[]);
                    self.children(node);
                    self.close("p");
                }
            }
            Node::Heading(h) => {
                let text = plain_text(node);
                let base = slug(&text);
                let n = self.slugs.entry(base.clone()).or_insert(0);
                let id = if *n == 0 {
                    base.clone()
                } else {
                    format!("{base}-{n}")
                };
                *n += 1;
                self.headings.push(Heading {
                    level: h.depth,
                    text,
                    id: id.clone(),
                    line: node.position().map(|p| p.start.line - 1).unwrap_or(0),
                });
                let tag = format!("h{}", h.depth.clamp(1, 6));
                self.open(&tag, node, &[("id", id)]);
                self.children(node);
                self.close(&tag);
            }
            Node::Text(t) => self.text(&t.value),
            Node::Emphasis(_) => self.wrap("em", node),
            Node::Strong(_) => self.wrap("strong", node),
            Node::Delete(_) => self.wrap("del", node),
            Node::Break(_) => self.raw("<br>"),
            Node::ThematicBreak(_) => self.raw("<hr>"),
            Node::InlineCode(c) => {
                self.raw("<code>");
                self.text(&c.value);
                self.raw("</code>");
            }
            Node::Code(c) => {
                self.open("pre", node, &[]);
                let lang = c
                    .lang
                    .as_deref()
                    .map(|l| {
                        l.chars()
                            .filter(|ch| ch.is_ascii_alphanumeric() || "+#_.-".contains(*ch))
                            .collect::<String>()
                    })
                    .filter(|l| !l.is_empty());
                match lang {
                    Some(l) => self.raw(&format!(
                        "<code class=\"language-{}\">",
                        l.to_ascii_lowercase()
                    )),
                    None => self.raw("<code>"),
                }
                self.text(&c.value);
                self.raw("</code></pre>");
            }
            Node::InlineMath(m) => {
                self.raw("<code class=\"language-math math-inline\">");
                self.text(&m.value);
                self.raw("</code>");
            }
            Node::Math(m) => {
                self.raw("<pre><code class=\"language-math math-display\">");
                self.text(&m.value);
                self.raw("</code></pre>");
            }
            Node::Blockquote(_) => {
                self.open("blockquote", node, &[]);
                self.blocks(node);
                self.close("blockquote");
            }
            Node::List(l) => {
                let tag = if l.ordered { "ol" } else { "ul" };
                let mut attrs = Vec::new();
                if let Some(start) = l.start.filter(|s| l.ordered && *s != 1) {
                    attrs.push(("start", start.to_string()));
                }
                let tasks = l
                    .children
                    .iter()
                    .any(|c| matches!(c, Node::ListItem(i) if i.checked.is_some()));
                if tasks {
                    attrs.push(("class", "contains-task-list".to_string()));
                }
                self.open(tag, node, &attrs);
                let was = self.tight;
                for item in &l.children {
                    let spread = l.spread || matches!(item, Node::ListItem(i) if i.spread);
                    self.tight = !spread;
                    self.node(item);
                }
                self.tight = was;
                self.close(tag);
            }
            Node::ListItem(i) => {
                match i.checked {
                    Some(checked) => {
                        self.open("li", node, &[("class", "task-list-item".to_string())]);
                        // A glyph rather than an <input>: no document may put a
                        // form field on a page, and the glyph survives Pandoc.
                        self.raw(if checked {
                            "<span class=\"task-box\" data-checked=\"true\">\u{2611}</span> "
                        } else {
                            "<span class=\"task-box\" data-checked=\"false\">\u{2610}</span> "
                        });
                    }
                    None => self.open("li", node, &[]),
                }
                let tight = self.tight;
                for (n, c) in i.children.iter().enumerate() {
                    if tight && n > 0 {
                        self.raw("\n");
                    }
                    // Only the item's own paragraphs lose their <p>.
                    self.tight = tight && matches!(c, Node::Paragraph(_));
                    self.node(c);
                }
                self.tight = tight;
                self.close("li");
            }
            Node::Link(l) => {
                let href = self.href(node, &l.url, false);
                let mut attrs = vec![("href", href)];
                if let Some(t) = &l.title {
                    attrs.push(("title", t.clone()));
                }
                self.open("a", node, &attrs);
                self.children(node);
                self.close("a");
            }
            Node::LinkReference(r) => {
                match self.definitions.get(&r.identifier.to_lowercase()).cloned() {
                    Some((url, title)) => {
                        let href = self.href(node, &url, false);
                        let mut attrs = vec![("href", href)];
                        if let Some(t) = title {
                            attrs.push(("title", t));
                        }
                        self.open("a", node, &attrs);
                        self.children(node);
                        self.close("a");
                    }
                    None => self.children(node),
                }
            }
            Node::Image(i) => {
                let src = self.href(node, &i.url, true);
                self.image(&src, &i.alt, i.title.as_deref());
            }
            Node::ImageReference(r) => {
                match self.definitions.get(&r.identifier.to_lowercase()).cloned() {
                    Some((url, title)) => {
                        let src = self.href(node, &url, true);
                        self.image(&src, &r.alt, title.as_deref());
                    }
                    None => self.text(&r.alt),
                }
            }
            Node::Definition(_) | Node::FootnoteDefinition(_) | Node::Yaml(_) | Node::Toml(_) => {}
            Node::FootnoteReference(f) => match self
                .footnotes
                .iter()
                .position(|(id, _)| *id == f.identifier)
            {
                Some(_) => {
                    let next = self.footnote_numbers.len() + 1;
                    let n = *self
                        .footnote_numbers
                        .entry(f.identifier.clone())
                        .or_insert(next);
                    let id = slug(&f.identifier);
                    self.raw(&format!(
                        "<sup><a href=\"#fn-{id}\" id=\"fnref-{id}\" class=\"footnote-ref\">{n}</a></sup>"
                    ));
                }
                None => self.text(&format!(
                    "[^{}]",
                    f.label.as_deref().unwrap_or(&f.identifier)
                )),
            },
            Node::Table(t) => {
                self.open("table", node, &[]);
                for (r, row) in t.children.iter().enumerate() {
                    if r == 0 {
                        self.raw("<thead>");
                    } else if r == 1 {
                        self.raw("<tbody>");
                    }
                    self.raw("<tr>");
                    let cell = if r == 0 { "th" } else { "td" };
                    if let Some(cells) = row.children() {
                        for (c, content) in cells.iter().enumerate() {
                            let align = match t.align.get(c) {
                                Some(AlignKind::Left) => Some("left"),
                                Some(AlignKind::Right) => Some("right"),
                                Some(AlignKind::Center) => Some("center"),
                                _ => None,
                            };
                            match align {
                                Some(a) => self.raw(&format!("<{cell} align=\"{a}\">")),
                                None => self.raw(&format!("<{cell}>")),
                            }
                            self.children(content);
                            self.close(cell);
                        }
                    }
                    self.raw("</tr>");
                    if r == 0 {
                        self.raw("</thead>");
                    }
                }
                if t.children.len() > 1 {
                    self.raw("</tbody>");
                }
                self.close("table");
            }
            Node::TableRow(_) | Node::TableCell(_) => self.children(node),
            Node::Html(h) => self.raw(&h.value),
            Node::MdxFlowExpression(e) => {
                self.problem(
                    node,
                    "expressions in braces are not run; write \\{ for a brace",
                );
                self.raw("<p>");
                self.text(&format!("{{{}}}", e.value));
                self.raw("</p>");
            }
            Node::MdxTextExpression(e) => {
                self.problem(
                    node,
                    "expressions in braces are not run; write \\{ for a brace",
                );
                self.text(&format!("{{{}}}", e.value));
            }
            Node::MdxjsEsm(_) => self.problem(node, "import and export are not allowed"),
            Node::MdxJsxFlowElement(_) | Node::MdxJsxTextElement(_) => self.jsx(node),
        }
    }

    fn wrap(&mut self, tag: &str, node: &'a Node) {
        self.raw(&format!("<{tag}>"));
        self.children(node);
        self.close(tag);
    }

    /// A container's block children, never tight.
    fn blocks(&mut self, node: &'a Node) {
        let was = self.tight;
        self.tight = false;
        self.children(node);
        self.tight = was;
    }

    fn image(&mut self, src: &str, alt: &str, title: Option<&str>) {
        let mut s = format!("<img src=\"{}\" alt=\"{}\"", escape(src), escape(alt));
        if let Some(t) = title {
            s.push_str(&format!(" title=\"{}\"", escape(t)));
        }
        s.push('>');
        self.raw(&s);
    }

    /// Without an ESM parser, markdown-rs reads `import x from "y"` as a
    /// paragraph. In MDX that line would run code, so it is refused here.
    fn refuse_esm(&mut self, root: &'a Node) {
        if self.flavour != Flavour::Mdx {
            return;
        }
        for c in root.children().into_iter().flatten() {
            if let Node::Paragraph(_) = c {
                let text = plain_text(c);
                if text.starts_with("import ") || text.starts_with("export ") {
                    self.problem(c, "import and export are not allowed");
                }
            }
        }
    }

    fn jsx(&mut self, node: &'a Node) {
        let Some(name) = jsx_name(node) else {
            return self.children(node);
        };
        if name.starts_with(RESERVED) {
            if let Some(hook) = self.hook {
                if hook.element(self, name, node) {
                    return;
                }
            }
            return self.children(node);
        }
        let Some(component) = components::find(name) else {
            self.problem(node, format!("<{name}> is not a LosOS component"));
            return self.children(node);
        };
        let Some(props) = self.props(node, component) else {
            return self.children(node);
        };
        let get = |k: &str| props.get(k).cloned().unwrap_or_default();
        let flow = matches!(node, Node::MdxJsxFlowElement(_));
        match name {
            "Callout" => {
                let kind = props.get("type").cloned().unwrap_or_else(|| "note".into());
                self.open(
                    "aside",
                    node,
                    &[
                        ("class", format!("callout callout-{kind}")),
                        ("data-callout", kind.clone()),
                    ],
                );
                if let Some(title) = props.get("title") {
                    self.raw("<p class=\"callout-title\">");
                    self.text(title);
                    self.raw("</p>");
                }
                self.blocks(node);
                self.close("aside");
            }
            "Details" => {
                let mut attrs = vec![("class", "mdx-details".to_string())];
                if props.contains_key("open") {
                    attrs.push(("open", String::new()));
                }
                self.open("details", node, &attrs);
                self.raw("<summary>");
                self.text(
                    props
                        .get("summary")
                        .map(String::as_str)
                        .unwrap_or("Details"),
                );
                self.raw("</summary>");
                self.blocks(node);
                self.close("details");
            }
            "Columns" | "Column" | "Tabs" => {
                let class = match name {
                    "Columns" => "mdx-columns",
                    "Column" => "mdx-column",
                    _ => "mdx-tabs",
                };
                self.open("div", node, &[("class", class.to_string())]);
                self.blocks(node);
                self.close("div");
            }
            "Tab" => {
                let label = get("label");
                self.open(
                    "section",
                    node,
                    &[("class", "mdx-tab".into()), ("data-label", label.clone())],
                );
                self.raw("<p class=\"mdx-tab-label\"><strong>");
                self.text(&label);
                self.raw("</strong></p>");
                self.blocks(node);
                self.close("section");
            }
            "Figure" => {
                let src = get("src");
                let src = self.href(node, &src, true);
                self.open("figure", node, &[("class", "mdx-figure".into())]);
                self.image(&src, &get("alt"), None);
                let has_children = node.children().is_some_and(|c| !c.is_empty());
                if props.contains_key("caption") || has_children {
                    self.raw("<figcaption>");
                    if let Some(c) = props.get("caption") {
                        self.text(c);
                    }
                    self.blocks(node);
                    self.raw("</figcaption>");
                }
                self.close("figure");
            }
            "Embed" => {
                let target = get("href");
                let href = self.href(node, &target, false);
                if flow {
                    self.raw("<p>");
                }
                self.open("a", node, &[("class", "mdx-embed".into()), ("href", href)]);
                match props.get("title") {
                    Some(t) => self.text(t),
                    None if node.children().is_some_and(|c| !c.is_empty()) => self.children(node),
                    None => self.text(&target),
                }
                self.close("a");
                if flow {
                    self.raw("</p>");
                }
            }
            "Button" => {
                // A link drawn as LosOS's button: plain HTML, no script
                // behind it. The page swaps the classes for its own button's.
                let target = get("href");
                let href = self.href(node, &target, false);
                let variant = props
                    .get("variant")
                    .cloned()
                    .unwrap_or_else(|| "primary".into());
                let size = props.get("size").cloned().unwrap_or_else(|| "md".into());
                if flow {
                    self.raw("<p>");
                }
                self.open(
                    "a",
                    node,
                    &[
                        (
                            "class",
                            format!("mdx-button mdx-button-{variant} mdx-button-{size}"),
                        ),
                        ("href", href),
                    ],
                );
                let label = plain_text(node);
                let label = label.trim();
                self.text(if label.is_empty() { &target } else { label });
                self.close("a");
                if flow {
                    self.raw("</p>");
                }
            }
            "Slide" => {
                let mut attrs = vec![("class", "mdx-slide".to_string())];
                if let Some(layout) = props.get("layout") {
                    attrs.push(("data-layout", layout.clone()));
                }
                self.open("section", node, &attrs);
                self.blocks(node);
                self.close("section");
            }
            "PageBreak" => self.raw("<div class=\"page-break\"></div>"),
            "Notes" => {
                self.open("aside", node, &[("class", "mdx-speaker-notes".into())]);
                self.blocks(node);
                self.close("aside");
            }
            "TableOfContents" => {
                self.open("nav", node, &[("class", "mdx-toc".into())]);
                if let Some(t) = props.get("title") {
                    self.raw("<p class=\"mdx-toc-title\">");
                    self.text(t);
                    self.raw("</p>");
                }
                // Filled in once every heading is known.
                self.raw(TOC_MARK);
                self.close("nav");
            }
            "Header" | "Footer" => {
                let (tag, class) = if name == "Header" {
                    ("header", "mdx-page-header")
                } else {
                    ("footer", "mdx-page-footer")
                };
                self.open(tag, node, &[("class", class.into())]);
                self.blocks(node);
                self.close(tag);
            }
            "PageNumber" => self.raw("<span class=\"mdx-page-number\"></span>"),
            "Table" => {
                self.open("table", node, &[("class", "mdx-table".into())]);
                if let Some(c) = props.get("caption") {
                    self.raw("<caption>");
                    self.text(c);
                    self.raw("</caption>");
                }
                self.raw("<tbody>");
                self.children(node);
                self.raw("</tbody>");
                self.close("table");
            }
            "Row" => {
                let was = self.header_row;
                self.header_row = props.contains_key("header");
                self.open("tr", node, &[]);
                self.children(node);
                self.close("tr");
                self.header_row = was;
            }
            "Cell" => {
                let tag = if self.header_row { "th" } else { "td" };
                let mut attrs = Vec::new();
                for (k, a) in [
                    ("colSpan", "colspan"),
                    ("rowSpan", "rowspan"),
                    ("align", "align"),
                ] {
                    if let Some(v) = props.get(k) {
                        attrs.push((a, v.clone()));
                    }
                }
                self.open(tag, node, &attrs);
                self.blocks(node);
                self.close(tag);
            }
            "Comment" => {
                let mut attrs = vec![
                    ("class", "mdx-comment".to_string()),
                    ("data-comment", get("text")),
                ];
                if let Some(a) = props.get("author") {
                    attrs.push(("data-author", a.clone()));
                }
                if let Some(d) = props.get("date") {
                    attrs.push(("data-date", d.clone()));
                }
                let tag = if flow { "div" } else { "span" };
                self.open(tag, node, &attrs);
                self.children(node);
                self.close(tag);
            }
            "Equation" => {
                let inline = props.contains_key("inline") || !flow;
                if inline {
                    self.raw("<code class=\"language-math math-inline\">");
                } else {
                    self.raw("<pre><code class=\"language-math math-display\">");
                }
                self.text(&get("tex"));
                self.raw(if inline { "</code>" } else { "</code></pre>" });
            }
            "Chart" => {
                let src = get("src");
                let href = self.href(node, &src, true);
                let mut attrs = vec![
                    ("class", "mdx-chart".to_string()),
                    ("data-src", href),
                    (
                        "data-type",
                        props.get("type").cloned().unwrap_or_else(|| "bar".into()),
                    ),
                ];
                if let Some(r) = props.get("range") {
                    attrs.push(("data-range", r.clone()));
                }
                self.open("figure", node, &attrs);
                if let Some(t) = props.get("title") {
                    self.raw("<figcaption>");
                    self.text(t);
                    self.raw("</figcaption>");
                }
                self.close("figure");
            }
            "Mention" => {
                let login = get("login");
                self.raw(&format!(
                    "<span class=\"mdx-mention\" data-login=\"{}\">@",
                    escape(&login)
                ));
                self.text(&login);
                self.raw("</span>");
            }
            "Date" => {
                let value = get("value");
                let mut attrs = vec![
                    ("class", "mdx-date".to_string()),
                    ("datetime", value.clone()),
                ];
                if let Some(f) = props.get("format") {
                    attrs.push(("data-format", f.clone()));
                }
                self.open("time", node, &attrs);
                self.text(&value);
                self.close("time");
            }
            "br" => self.raw("<br>"),
            "abbr" => {
                match props.get("title") {
                    Some(t) => self.raw(&format!("<abbr title=\"{}\">", escape(t))),
                    None => self.raw("<abbr>"),
                }
                self.children(node);
                self.close("abbr");
            }
            // The other plain elements are themselves, with no attributes.
            _ => {
                if flow {
                    self.raw("<p>");
                }
                self.wrap(component.name, node);
                if flow {
                    self.raw("</p>");
                }
            }
        }
    }

    /// The element's props, checked against its table entry: None, with a
    /// problem, when a required one is missing.
    fn props(&mut self, node: &Node, component: &Component) -> Option<BTreeMap<String, String>> {
        let mut props = BTreeMap::new();
        for a in jsx_attributes(node).into_iter().flatten() {
            match a {
                AttributeContent::Expression(_) => {
                    self.problem(
                        node,
                        format!("<{}>: spread props are not allowed", component.name),
                    );
                }
                AttributeContent::Property(p) => {
                    let Some(kind) = component.prop(&p.name) else {
                        self.problem(node, format!("<{}> has no prop {}", component.name, p.name));
                        continue;
                    };
                    let value = match (&p.value, kind) {
                        (None, Prop::Flag) => String::new(),
                        (None, _) => {
                            self.problem(
                                node,
                                format!("<{}>: {} needs a value", component.name, p.name),
                            );
                            continue;
                        }
                        (Some(AttributeValue::Expression(_)), _) => {
                            self.problem(
                                node,
                                format!(
                                    "<{}>: {} must be a string in quotes, not an expression",
                                    component.name, p.name
                                ),
                            );
                            continue;
                        }
                        (Some(AttributeValue::Literal(_)), Prop::Flag) => {
                            self.problem(
                                node,
                                format!("<{}>: {} takes no value", component.name, p.name),
                            );
                            continue;
                        }
                        (Some(AttributeValue::Literal(v)), kind) => {
                            if !kind.accepts(v) {
                                let wants = match kind {
                                    Prop::Choice(words) => format!("one of {}", words.join(", ")),
                                    Prop::Number => "a whole number from 1 to 1000".to_string(),
                                    _ => "a date written YYYY-MM-DD".to_string(),
                                };
                                self.problem(
                                    node,
                                    format!("<{}>: {} is {wants}", component.name, p.name),
                                );
                                continue;
                            }
                            v.clone()
                        }
                    };
                    props.insert(p.name.clone(), value);
                }
            }
        }
        for r in component.required {
            if !props.contains_key(*r) {
                self.problem(node, format!("<{}> needs {}", component.name, r));
                return None;
            }
        }
        Some(props)
    }

    fn footnote_section(&mut self) {
        if self.footnote_numbers.is_empty() {
            return;
        }
        let mut used: Vec<(usize, &'a mdast::FootnoteDefinition)> = self
            .footnotes
            .iter()
            .filter_map(|(id, f)| self.footnote_numbers.get(id).map(|n| (*n, *f)))
            .collect();
        used.sort_by_key(|(n, _)| *n);
        self.raw("<section class=\"footnotes\"><ol>");
        for (_, f) in used {
            let id = slug(&f.identifier);
            self.raw(&format!("<li id=\"fn-{id}\">"));
            let was = self.tight;
            self.tight = false;
            for c in &f.children {
                self.node(c);
            }
            self.tight = was;
            self.raw(&format!(
                " <a href=\"#fnref-{id}\" class=\"footnote-backref\">\u{21a9}</a></li>"
            ));
        }
        self.raw("</ol></section>");
    }
}

/// The headings as nested lists of links.
fn toc(headings: &[Heading]) -> String {
    let mut s = String::new();
    let mut open: Vec<u8> = Vec::new();
    for h in headings {
        while open.last().is_some_and(|l| *l > h.level) {
            s.push_str("</li></ol>");
            open.pop();
        }
        if open.last() == Some(&h.level) {
            s.push_str("</li>");
        } else {
            s.push_str("<ol>");
            open.push(h.level);
        }
        s.push_str(&format!(
            "<li><a href=\"#{}\">{}</a>",
            escape(&h.id),
            escape(&h.text)
        ));
    }
    for _ in open {
        s.push_str("</li></ol>");
    }
    s
}

/// Components that sit inside a line of text; the rest are blocks.
const INLINE: &[&str] = &[
    "Button",
    "Comment",
    "Date",
    "Embed",
    "Equation",
    "Mention",
    "PageNumber",
];

/// A paragraph that holds nothing but block elements (`<Row>` lines in a
/// `<Table>`, a lone `<PageBreak />`): written without its `<p>`, which
/// would otherwise land around a table row or a section.
fn only_blocks(node: &Node) -> bool {
    let children = node.children().map(Vec::as_slice).unwrap_or_default();
    let mut any = false;
    for c in children {
        match c {
            Node::Text(t) if t.value.trim().is_empty() => {}
            Node::MdxJsxTextElement(e) => match e.name.as_deref() {
                Some(n) if INLINE.contains(&n) || n.starts_with(RESERVED) => return false,
                Some(n) if components::INTRINSIC.iter().any(|c| c.name == n) => return false,
                _ => any = true,
            },
            _ => return false,
        }
    }
    any
}

fn jsx_name(node: &Node) -> Option<&str> {
    match node {
        Node::MdxJsxFlowElement(e) => e.name.as_deref(),
        Node::MdxJsxTextElement(e) => e.name.as_deref(),
        _ => None,
    }
}

fn jsx_attributes(node: &Node) -> Option<&Vec<AttributeContent>> {
    match node {
        Node::MdxJsxFlowElement(e) => Some(&e.attributes),
        Node::MdxJsxTextElement(e) => Some(&e.attributes),
        _ => None,
    }
}
