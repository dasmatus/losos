//! A page of the admin UI or an app, as MDX that places the page's blocks.
//!
//! Every page LosOS draws is a short MDX file (`admin-ui/app/src/pages/`)
//! that places blocks: the page's own sections, each drawn by a LosOS
//! component (`<BoxName />`, `<Apps />`). An administrator may replace that
//! file to reorder, drop or repeat blocks, set the props a block offers,
//! and write their own text, links and document components around them.
//! What a block does inside stays LosOS's.
//!
//! [`Registry`] is `blocks.json`: each page, and the blocks it has with
//! their props. [`render_page`] reads a page against its entry. A block
//! becomes an empty placeholder, `<losos-block data-block="Apps"
//! data-props="{...}">`, which the page swaps for the component (the
//! browser) or for the same section drawn on the box (the no-JS pages).
//! Nothing in the file runs: the rules of [`crate::Flavour::Mdx`] hold, a
//! block's props are checked strings, and the placeholder can only come
//! from here, since an author writing `<losos-block>` writes an element no
//! table knows.
//!
//! Runs of the author's own text and document components at the top of
//! the page are wrapped in `div.mdx-document.mdx-page-text`, so the
//! document styles reach them and never the blocks beside them.

use crate::components::{self, COMPONENTS};
use crate::render::{self, Hook, Writer, RESERVED};
use crate::{Error, Parsed, Place, Problem, Rendered, Resolver};
use markdown::mdast::{AttributeContent, AttributeValue, MdxJsxAttribute, MdxJsxFlowElement, Node};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The element a block stands in for, in the HTML.
pub const PLACEHOLDER: &str = "losos-block";

/// The reserved name a block carries between the pass and the hook.
const BLOCK: &str = "losos:block";
/// A run of the author's own text at the top of a page.
const TEXT: &str = "losos:page-text";

/// The longest name a page or block may have.
const MAX_NAME: usize = 64;

/// What a block's prop may hold.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum BlockProp {
    /// Any text, up to 500 characters.
    Text,
    /// One of these words.
    Choice { values: Vec<String> },
    /// A whole number from 1 to 1000.
    Number,
    /// Present or not.
    Flag,
    /// A link: http, https, mailto or a path.
    Url,
}

impl BlockProp {
    fn accepts(&self, value: &str) -> Result<(), String> {
        match self {
            BlockProp::Text => {
                if value.chars().count() <= 500 {
                    Ok(())
                } else {
                    Err("at most 500 characters".to_string())
                }
            }
            BlockProp::Choice { values } => {
                if values.iter().any(|v| v == value) {
                    Ok(())
                } else {
                    Err(format!("one of {}", values.join(", ")))
                }
            }
            BlockProp::Number => {
                if components::Prop::Number.accepts(value) {
                    Ok(())
                } else {
                    Err("a whole number from 1 to 1000".to_string())
                }
            }
            BlockProp::Flag => Err("written without a value".to_string()),
            BlockProp::Url => {
                let v = value.trim();
                let scheme = v
                    .split_once(':')
                    .filter(|(s, _)| {
                        !s.is_empty()
                            && s.bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b))
                    })
                    .map(|(s, _)| s.to_ascii_lowercase());
                match scheme.as_deref() {
                    None | Some("http" | "https" | "mailto") if v.len() <= 2048 => Ok(()),
                    None | Some("http" | "https" | "mailto") => {
                        Err("a link of at most 2048 characters".to_string())
                    }
                    Some(_) => Err("a link: http, https, mailto or a path".to_string()),
                }
            }
        }
    }
}

/// A prop of a block, as `blocks.json` gives it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropSpec {
    #[serde(flatten)]
    pub kind: BlockProp,
    /// Without it the block is not drawn.
    #[serde(default)]
    pub required: bool,
    /// What it does, for the editor.
    #[serde(default)]
    pub about: String,
}

/// One block a page has.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockSpec {
    /// What it shows, for the editor.
    #[serde(default)]
    pub about: String,
    #[serde(default)]
    pub props: BTreeMap<String, PropSpec>,
    /// Whether it takes MDX inside it (`<Section>…</Section>`).
    #[serde(default)]
    pub children: bool,
}

/// One page, as `blocks.json` gives it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageSpec {
    /// Its name in the editor's list.
    pub title: String,
    /// Where it opens, for the editor's link.
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub blocks: BTreeMap<String, BlockSpec>,
}

/// `blocks.json`: every page and its blocks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registry {
    pub pages: BTreeMap<String, PageSpec>,
}

impl Registry {
    /// Reads `blocks.json`, refusing a page or block name that is not
    /// plain, and a block named like one of the document components.
    pub fn from_json(json: &str) -> Result<Registry, String> {
        #[derive(Deserialize)]
        struct File {
            pages: BTreeMap<String, PageSpec>,
        }
        let file: File = serde_json::from_str(json).map_err(|e| e.to_string())?;
        for (id, page) in &file.pages {
            if !page_id_ok(id) {
                return Err(format!("{id:?} is not a page name"));
            }
            for name in page.blocks.keys() {
                if !block_name_ok(name) {
                    return Err(format!("{id}: {name:?} is not a block name"));
                }
                if components::find(name).is_some() {
                    return Err(format!("{id}: {name} is the name of a document component"));
                }
            }
        }
        Ok(Registry { pages: file.pages })
    }

    pub fn page(&self, id: &str) -> Option<&PageSpec> {
        self.pages.get(id)
    }
}

/// A page's name: lowercase words, digits and dashes, with `.` between
/// parts (`overview`, `settings.storage`), so it is one segment of a path
/// and one file name.
pub fn page_id_ok(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_NAME
        && id.split('.').all(|part| {
            !part.is_empty()
                && part.starts_with(|c: char| c.is_ascii_lowercase())
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

/// A block's name: a capital, then letters and digits.
fn block_name_ok(name: &str) -> bool {
    name.len() <= MAX_NAME
        && name.starts_with(|c: char| c.is_ascii_uppercase())
        && name.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// Reads a page against its entry in `blocks.json` and writes it out.
pub fn render_page(source: &str, page: &PageSpec) -> Result<Rendered, Error> {
    let mut parsed = crate::parse(source, crate::Flavour::Mdx)?;
    let mut problems = std::mem::take(&mut parsed.problems);
    refuse_esm(&parsed.tree, &mut problems);
    place_blocks(&mut parsed.tree, page, &mut problems);
    wrap_text(&mut parsed.tree);
    parsed.problems = problems;
    Ok(parsed.render_page(&crate::Plain, &PageHook))
}

/// `import` and `export` lines at the top of the page, before they are
/// wrapped where the writer's own check would not see them.
fn refuse_esm(tree: &Node, problems: &mut Vec<Problem>) {
    for c in tree.children().into_iter().flatten() {
        if let Node::Paragraph(_) = c {
            let text = render::plain_text(c);
            if text.starts_with("import ") || text.starts_with("export ") {
                problems.push(problem(c, "import and export are not allowed".into()));
            }
        }
    }
}

fn problem(node: &Node, message: String) -> Problem {
    Problem {
        place: node
            .position()
            .map(|p| Place {
                line: p.start.line,
                column: p.start.column,
            })
            .unwrap_or_default(),
        message,
    }
}

/// Turns each of the page's blocks into the reserved element the hook
/// draws, with its props checked; anything else with a name the page and
/// the component table do not know becomes its children and a problem.
fn place_blocks(node: &mut Node, page: &PageSpec, problems: &mut Vec<Problem>) {
    let flow = matches!(node, Node::MdxJsxFlowElement(_));
    let name = match node {
        Node::MdxJsxFlowElement(e) => e.name.clone(),
        Node::MdxJsxTextElement(e) => e.name.clone(),
        _ => None,
    };
    if let Some(name) = name.filter(|n| !n.starts_with(RESERVED)) {
        if let Some(spec) = page.blocks.get(&name) {
            if !flow {
                problems.push(problem(
                    node,
                    format!("<{name} /> goes on a line of its own"),
                ));
                unname(node);
            } else {
                block(node, &name, spec, problems);
            }
        } else if components::find(&name).is_none() {
            problems.push(problem(
                node,
                format!("<{name}> is not a block of this page or a LosOS component"),
            ));
            unname(node);
        }
    }
    if let Some(children) = node.children_mut() {
        for child in children {
            place_blocks(child, page, problems);
        }
    }
}

/// An element drawn as its children alone.
fn unname(node: &mut Node) {
    match node {
        Node::MdxJsxFlowElement(e) => e.name = None,
        Node::MdxJsxTextElement(e) => e.name = None,
        _ => {}
    }
}

fn block(node: &mut Node, name: &str, spec: &BlockSpec, problems: &mut Vec<Problem>) {
    let Node::MdxJsxFlowElement(e) = node else {
        return;
    };
    let mut props = serde_json::Map::new();
    let mut fine = true;
    let at = Node::MdxJsxFlowElement(MdxJsxFlowElement {
        children: Vec::new(),
        position: e.position.clone(),
        name: None,
        attributes: Vec::new(),
    });
    for a in &e.attributes {
        match a {
            AttributeContent::Expression(_) => {
                problems.push(problem(
                    &at,
                    format!("<{name}>: spread props are not allowed"),
                ));
            }
            AttributeContent::Property(p) => {
                let Some(prop) = spec.props.get(&p.name) else {
                    problems.push(problem(&at, format!("<{name}> has no prop {}", p.name)));
                    continue;
                };
                match (&p.value, &prop.kind) {
                    (None, BlockProp::Flag) => {
                        props.insert(p.name.clone(), serde_json::Value::Bool(true));
                    }
                    (None, _) => {
                        problems.push(problem(&at, format!("<{name}>: {} needs a value", p.name)));
                    }
                    (Some(AttributeValue::Expression(_)), _) => problems.push(problem(
                        &at,
                        format!(
                            "<{name}>: {} must be a string in quotes, not an expression",
                            p.name
                        ),
                    )),
                    (Some(AttributeValue::Literal(v)), kind) => match kind.accepts(v) {
                        Ok(()) => {
                            props.insert(p.name.clone(), serde_json::Value::String(v.clone()));
                        }
                        Err(wants) => {
                            problems.push(problem(&at, format!("<{name}>: {} is {wants}", p.name)))
                        }
                    },
                }
            }
        }
    }
    for (prop, p) in &spec.props {
        if p.required && !props.contains_key(prop) {
            problems.push(problem(&at, format!("<{name}> needs {prop}")));
            fine = false;
        }
    }
    if !spec.children && !e.children.is_empty() {
        problems.push(problem(&at, format!("<{name} /> takes nothing inside it")));
        e.children.clear();
    }
    if !fine {
        e.name = None;
        e.attributes.clear();
        return;
    }
    e.name = Some(BLOCK.to_string());
    e.attributes = vec![
        literal("block", name),
        literal("props", &serde_json::Value::Object(props).to_string()),
    ];
}

fn literal(name: &str, value: &str) -> AttributeContent {
    AttributeContent::Property(MdxJsxAttribute {
        name: name.to_string(),
        value: Some(AttributeValue::Literal(value.to_string())),
    })
}

/// Wraps each run of the page's top-level nodes that are not blocks.
fn wrap_text(tree: &mut Node) {
    let Node::Root(root) = tree else {
        return;
    };
    let mut out = Vec::new();
    let mut run: Vec<Node> = Vec::new();
    let flush = |run: &mut Vec<Node>, out: &mut Vec<Node>| {
        if run.is_empty() {
            return;
        }
        let shown = run.iter().any(|n| {
            !matches!(
                n,
                Node::Yaml(_) | Node::Toml(_) | Node::Definition(_) | Node::FootnoteDefinition(_)
            )
        });
        if shown {
            let position = match (
                run.first().and_then(Node::position),
                run.last().and_then(Node::position),
            ) {
                (Some(a), Some(b)) => Some(markdown::unist::Position {
                    start: a.start.clone(),
                    end: b.end.clone(),
                }),
                _ => None,
            };
            out.push(Node::MdxJsxFlowElement(MdxJsxFlowElement {
                children: std::mem::take(run),
                position,
                name: Some(TEXT.to_string()),
                attributes: Vec::new(),
            }));
        } else {
            out.append(run);
        }
    };
    for child in std::mem::take(&mut root.children) {
        let is_block =
            matches!(&child, Node::MdxJsxFlowElement(e) if e.name.as_deref() == Some(BLOCK));
        if is_block {
            flush(&mut run, &mut out);
            out.push(child);
        } else {
            run.push(child);
        }
    }
    flush(&mut run, &mut out);
    root.children = out;
}

/// Draws the two reserved elements the passes above put in the tree.
struct PageHook;

impl Hook for PageHook {
    fn element<'a>(&self, w: &mut Writer<'a>, name: &str, node: &'a Node) -> bool {
        match name {
            BLOCK => {
                let block = Writer::attr(node, "block").unwrap_or_default();
                let props = Writer::attr(node, "props").unwrap_or_else(|| "{}".into());
                w.raw(&format!(
                    "<{PLACEHOLDER} data-block=\"{}\" data-props=\"{}\">",
                    render::escape(&block),
                    render::escape(&props)
                ));
                w.children(node);
                w.raw(&format!("</{PLACEHOLDER}>"));
                true
            }
            TEXT => {
                w.raw("<div class=\"mdx-document mdx-page-text\">");
                w.children(node);
                w.raw("</div>");
                true
            }
            _ => false,
        }
    }
}

/// The block names a page's text places, in order, for the editor's
/// outline and the default-file check.
pub fn blocks_used(rendered: &Rendered) -> Vec<String> {
    let mut out = Vec::new();
    let needle = format!("<{PLACEHOLDER} data-block=\"");
    let mut rest = rendered.html.as_str();
    while let Some(at) = rest.find(&needle) {
        rest = &rest[at + needle.len()..];
        if let Some(end) = rest.find('"') {
            out.push(rest[..end].to_string());
        }
    }
    out
}

impl Parsed {
    /// Writes a page out, its placeholders kept by the page allowlist.
    pub(crate) fn render_page(&self, resolver: &dyn Resolver, hook: &dyn Hook) -> Rendered {
        self.finish(resolver, Some(hook), crate::sanitize::clean_page)
    }
}

/// The component names a page may use besides its blocks, for the
/// editor's list.
pub fn component_names() -> Vec<&'static str> {
    COMPONENTS.iter().map(|c| c.name).collect()
}
