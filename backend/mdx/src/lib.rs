//! MDX and Markdown to HTML, for LosOS Office and Notes.
//!
//! MDX is Markdown with JSX in it, and the usual MDX tools compile a
//! document to JavaScript and run it. This crate never does: it parses the
//! document with markdown-rs and writes the HTML itself. A JSX element is
//! one of LosOS's components or a few plain HTML elements
//! (`components.rs`), its props are strings checked against that table,
//! and expressions, `import` and `export` are refused. Whatever comes out
//! then passes an allowlist of tags, attributes and URL schemes
//! (`sanitize.rs`), so neither a document nor a bug in the writer can put a
//! script, an event handler or a `style` attribute on a page.
//!
//! A refusal is a [`Problem`], not a failure: the page still draws the
//! document, with the refused part as plain text, and says what was
//! refused. An export asks for [`Rendered::strict`] instead.
//!
//! The two flavours share everything but the parser's rules: [`Flavour::Mdx`]
//! for `.paper`, `.slides`, `.note` and `.mdx`, [`Flavour::Md`] for `.md`, where `<`
//! and `{` are plain text and raw HTML is allowed through the same
//! allowlist.

mod components;
mod date;
mod math;
pub mod notes;
pub mod page;
mod render;
mod sanitize;
#[cfg(feature = "wasm")]
mod wasm;

pub use components::{Component, Prop, COMPONENTS, INTRINSIC};
pub use date::{format as format_date, LANGS};
pub use markdown::mdast;
pub use math::{mathml, MAX_TEX, MAX_TEX_NESTING};
pub use render::{Hook, Writer, RESERVED};

use markdown::mdast::Node;
use markdown::{Constructs, ParseOptions};
use serde::Serialize;
use std::collections::BTreeMap;

/// The largest document read, in bytes: about 500 pages of text. markdown-rs
/// takes time that grows faster than the document (a 1 MiB file of short
/// paragraphs takes around ten seconds natively, more in WebAssembly), so a
/// bigger one is an error the page can show as plain text instead.
pub const MAX_SOURCE: usize = 1024 * 1024;

/// How deep quotes, lists and elements may nest. markdown-rs recurses, and
/// past a few thousand levels it overflows a 1 MiB stack; nobody writes a
/// hundred.
pub const MAX_NESTING: usize = 100;

/// Which rules a document is read by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flavour {
    /// MDX: JSX elements from the component table; `<` and `{` are syntax.
    Mdx,
    /// Markdown as Obsidian and GitHub read it; raw HTML is allowed and
    /// sanitized like everything else.
    Md,
}

impl Flavour {
    /// The flavour a file name says, or None for a file that is neither.
    pub fn for_name(name: &str) -> Option<Flavour> {
        let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
        match ext.as_str() {
            "paper" | "slides" | "note" | "mdx" => Some(Flavour::Mdx),
            "md" | "markdown" => Some(Flavour::Md),
            _ => None,
        }
    }
}

/// Where the links in a document go. Notes, the knowledge-base pages and
/// Office each answer differently; the crate never decides.
pub trait Resolver {
    /// A note by the name a link gives it (`plan`, `trips/plan.md`), as its
    /// path, or None when there is none.
    fn note(&self, _target: &str) -> Option<String> {
        None
    }
    /// The address of a picture or other file the document names.
    fn file(&self, _target: &str) -> Option<String> {
        None
    }
    /// Where a note opens, with `heading` or `^block` when one is asked for.
    fn href(&self, path: &str, sub: &str) -> String {
        if sub.is_empty() {
            path.to_string()
        } else {
            format!("{path}#{sub}")
        }
    }
}

/// Links left as the document wrote them.
pub struct Plain;
impl Resolver for Plain {}

/// A place in the source, from 1, like an editor's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Place {
    pub line: usize,
    pub column: usize,
}

/// Something the document asked for and did not get.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Problem {
    pub place: Place,
    pub message: String,
}

/// A document that could not be read at all.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Error {
    /// Not valid MDX (an unclosed `<`, an unbalanced `{`). The editor shows
    /// it where it is.
    Syntax { place: Place, message: String },
    /// Over [`MAX_SOURCE`].
    TooLarge { bytes: usize },
    /// Nested deeper than [`MAX_NESTING`]: quotes in quotes, elements in
    /// elements. The parser recurses, and a stack (1 MiB in WebAssembly)
    /// is finite.
    TooDeep { place: Place },
    /// An export was asked for, and the document has these problems.
    Refused(Vec<Problem>),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Syntax { place, message } => {
                write!(f, "line {}, column {}: {message}", place.line, place.column)
            }
            Error::TooLarge { bytes } => {
                write!(f, "the document is {bytes} bytes, over {MAX_SOURCE}")
            }
            Error::TooDeep { place } => write!(
                f,
                "line {}: nested more than {MAX_NESTING} levels deep",
                place.line
            ),
            Error::Refused(problems) => {
                let first = problems.first().map(|p| p.message.as_str()).unwrap_or("");
                write!(f, "{} problem(s), first: {first}", problems.len())
            }
        }
    }
}

impl std::error::Error for Error {}

/// A heading, for the outline and for links to it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    /// The `id` it carries in the HTML.
    pub id: String,
    /// Its line in the source, from 0, front matter counted.
    pub line: usize,
}

/// A link or picture the document points at.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Link {
    /// As written.
    pub target: String,
    /// A picture (or, in Notes, an embed) rather than a link.
    pub embed: bool,
    /// http, https or mailto, rather than something on the box.
    pub external: bool,
}

/// A document read and parsed, before it is written out. Notes' own syntax
/// (wikilinks, tags, callouts) is a pass over `tree` between [`parse`] and
/// [`Parsed::render`].
#[derive(Clone, Debug)]
pub struct Parsed {
    pub flavour: Flavour,
    pub tree: Node,
    /// The front matter as written, without its fences.
    pub front_matter: Option<String>,
    pub problems: Vec<Problem>,
    /// Attributes a pass adds to the element written for a node, keyed by
    /// the node's start offset and the tag written for it (a list and its
    /// first item start at the same offset): `id`, `class` and `data-*`
    /// only, on a paragraph, list item, heading, quote, table or code block.
    pub attributes: BTreeMap<(usize, String), Vec<(String, String)>>,
    /// The reader's language, for what is written out in words (a
    /// `<Date>`) when the document names none in its front matter's
    /// `lang`. English when neither does.
    pub lang: Option<String>,
}

/// What a page needs to show a document.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rendered {
    pub html: String,
    /// The front matter as JSON, `{}` for an empty one, None without any or
    /// when it is not YAML this can read (then `properties_error` holds it).
    pub properties: Option<serde_json::Value>,
    pub properties_error: Option<String>,
    pub headings: Vec<Heading>,
    pub links: Vec<Link>,
    pub problems: Vec<Problem>,
}

impl Rendered {
    /// The document for an export: refused when anything in it was.
    pub fn strict(self) -> Result<Rendered, Error> {
        if self.problems.is_empty() {
            Ok(self)
        } else {
            Err(Error::Refused(self.problems))
        }
    }
}

fn options(flavour: Flavour) -> ParseOptions {
    let mut constructs = Constructs {
        frontmatter: true,
        math_flow: true,
        math_text: true,
        ..Constructs::gfm()
    };
    if flavour == Flavour::Mdx {
        // MDX's own set, but no ESM: without a parser for it markdown-rs
        // reads `import` lines as text, which render.rs then refuses.
        constructs.autolink = false;
        constructs.code_indented = false;
        constructs.html_flow = false;
        constructs.html_text = false;
        constructs.mdx_expression_flow = true;
        constructs.mdx_expression_text = true;
        constructs.mdx_jsx_flow = true;
        constructs.mdx_jsx_text = true;
    }
    ParseOptions {
        constructs,
        // `~one~` is not strikethrough in Obsidian or LosOS; `~~two~~` is.
        gfm_strikethrough_single_tilde: false,
        math_text_single_dollar: true,
        ..ParseOptions::default()
    }
}

/// Reads a document.
pub fn parse(source: &str, flavour: Flavour) -> Result<Parsed, Error> {
    if source.len() > MAX_SOURCE {
        return Err(Error::TooLarge {
            bytes: source.len(),
        });
    }
    if let Some(line) = too_deep_to_parse(source, flavour) {
        return Err(Error::TooDeep {
            place: Place { line, column: 1 },
        });
    }
    let tree = markdown::to_mdast(source, &options(flavour)).map_err(|m| Error::Syntax {
        place: match m.place.as_deref() {
            Some(markdown::message::Place::Point(p)) => Place {
                line: p.line,
                column: p.column,
            },
            Some(markdown::message::Place::Position(p)) => Place {
                line: p.start.line,
                column: p.start.column,
            },
            None => Place::default(),
        },
        message: m.reason,
    })?;
    let front_matter = match &tree {
        Node::Root(root) => root.children.iter().find_map(|n| match n {
            Node::Yaml(y) => Some(y.value.clone()),
            _ => None,
        }),
        _ => None,
    };
    if let Some(place) = deeper_than(&tree, MAX_NESTING) {
        return Err(Error::TooDeep { place });
    }
    let mut tree = tree;
    let mut problems = Vec::new();
    render::refuse_reserved(&mut tree, &mut problems);
    Ok(Parsed {
        flavour,
        tree,
        front_matter,
        problems,
        attributes: BTreeMap::new(),
        lang: None,
    })
}

impl Parsed {
    /// Writes the document out as sanitized HTML.
    pub fn render(&self, resolver: &dyn Resolver) -> Rendered {
        self.render_with(resolver, None)
    }

    /// Writes the document out, handing the reserved `losos:` elements a
    /// pass put in the tree to `hook`.
    pub fn render_with(&self, resolver: &dyn Resolver, hook: Option<&dyn Hook>) -> Rendered {
        self.finish(resolver, hook, sanitize::clean)
    }

    /// Writes the tree out and runs `clean`, an allowlist, over the HTML.
    fn finish(
        &self,
        resolver: &dyn Resolver,
        hook: Option<&dyn Hook>,
        clean: fn(&str) -> String,
    ) -> Rendered {
        let (properties, properties_error) = properties(self.front_matter.as_deref());
        let lang = properties
            .as_ref()
            .and_then(|p| p.get("lang"))
            .and_then(|l| l.as_str())
            .or(self.lang.as_deref())
            .map_or("en", date::lang);
        let out = render::render(self, resolver, hook, lang);
        let mut problems = self.problems.clone();
        problems.extend(out.problems);
        problems.sort_by_key(|p| (p.place.line, p.place.column));
        Rendered {
            html: clean(&out.html),
            properties,
            properties_error,
            headings: out.headings,
            links: out.links,
            problems,
        }
    }
}

/// Reads and writes out a document in one go.
pub fn render(source: &str, flavour: Flavour, resolver: &dyn Resolver) -> Result<Rendered, Error> {
    Ok(parse(source, flavour)?.render(resolver))
}

/// A cheap look at the source for what would make the parser recurse
/// past its stack: a line's run of quote markers, a run of emphasis
/// markers, a line's indentation, and in MDX how deep tags open. Gives the
/// line (from 1) of the first. Generous: the tree is measured exactly once
/// it exists.
fn too_deep_to_parse(source: &str, flavour: Flavour) -> Option<usize> {
    let mut open = 0usize;
    for (n, line) in source.lines().enumerate() {
        let quotes = line
            .chars()
            .take_while(|c| matches!(c, '>' | ' ' | '\t'))
            .filter(|c| *c == '>')
            .count();
        let indent = line.len() - line.trim_start().len();
        if quotes > 4 * MAX_NESTING || indent > 8 * MAX_NESTING {
            return Some(n + 1);
        }
        let mut run = (' ', 0usize);
        for c in line.chars() {
            run = if c == run.0 { (c, run.1 + 1) } else { (c, 1) };
            if matches!(c, '*' | '_' | '~') && run.1 > 10 * MAX_NESTING {
                return Some(n + 1);
            }
        }
        if flavour == Flavour::Mdx {
            let b = line.as_bytes();
            for i in 0..b.len() {
                if b[i] == b'<' {
                    match b.get(i + 1) {
                        Some(b'/') => open = open.saturating_sub(1),
                        Some(c) if c.is_ascii_alphabetic() => {
                            let end = b[i..].iter().position(|c| *c == b'>').map(|e| i + e);
                            if !end.is_some_and(|e| e > 0 && b[e - 1] == b'/') {
                                open += 1;
                            }
                        }
                        _ => {}
                    }
                    if open > 4 * MAX_NESTING {
                        return Some(n + 1);
                    }
                }
            }
        }
    }
    None
}

/// The first node deeper than `max` containers, by its place. Walked with
/// a stack of its own, not recursion.
fn deeper_than(tree: &Node, max: usize) -> Option<Place> {
    let mut stack = vec![(tree, 0usize)];
    while let Some((node, depth)) = stack.pop() {
        if depth > max {
            let p = node.position().map(|p| Place {
                line: p.start.line,
                column: p.start.column,
            });
            return Some(p.unwrap_or_default());
        }
        if let Some(children) = node.children() {
            stack.extend(children.iter().map(|c| (c, depth + 1)));
        }
    }
    None
}

/// Front matter as JSON. YAML that is not a mapping (a bare list, a
/// string) is an error the page shows as written.
fn properties(yaml: Option<&str>) -> (Option<serde_json::Value>, Option<String>) {
    let Some(yaml) = yaml else {
        return (None, None);
    };
    match serde_yaml::from_str::<serde_yaml::Value>(yaml) {
        Ok(serde_yaml::Value::Null) => (Some(serde_json::json!({})), None),
        Ok(v @ serde_yaml::Value::Mapping(_)) => match serde_json::to_value(v) {
            Ok(json) => (Some(json), None),
            Err(_) => (None, Some(yaml.to_string())),
        },
        _ => (None, Some(yaml.to_string())),
    }
}

/// The mdast as JSON (unist's shape, `type` on every node, positions
/// kept), for the browser's editor.
pub fn tree_json(node: &Node) -> serde_json::Value {
    serde_json::to_value(node).unwrap_or(serde_json::Value::Null)
}
