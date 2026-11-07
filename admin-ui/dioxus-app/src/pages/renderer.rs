//! Renders sanitized page HTML (from losos-mdx) as Dioxus elements.
//!
//! Parses `<losos-block data-block="Name" data-props='...'>children</losos-block>`
//! placeholders, normalizes flag/string props, and dispatches to block components.
//! Non-block HTML is emitted with preserved markup/classes (sanitized upstream).
//! Falls back to embedded defaults when a page has no custom HTML.

use dioxus::prelude::*;
use serde_json::Value;
use std::collections::BTreeMap;

use super::defaults;
use super::store::PagesState;

/// A prop value a block receives: a flag (`true`) or a string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockValue {
    Flag,
    Text(String),
}

impl BlockValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            BlockValue::Text(s) => Some(s.as_str()),
            BlockValue::Flag => None,
        }
    }
    pub fn is_flag(&self) -> bool {
        matches!(self, BlockValue::Flag)
    }
}

/// Normalized props passed to a block (flag normalization + children).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BlockProps {
    pub props: BTreeMap<String, BlockValue>,
    /// Raw sanitized inner HTML children, if the block had any.
    pub children_html: Option<String>,
}

impl BlockProps {
    pub fn get(&self, key: &str) -> Option<&BlockValue> {
        self.props.get(key)
    }
    pub fn flag(&self, key: &str) -> bool {
        matches!(self.props.get(key), Some(BlockValue::Flag))
    }
    pub fn text(&self, key: &str) -> Option<&str> {
        self.props.get(key).and_then(|v| v.as_str())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Segment {
    Html(String),
    Block {
        name: String,
        props: BlockProps,
    },
}

/// Parse sanitized page HTML into segments, preserving order.
pub fn parse_segments(html: &str) -> Vec<Segment> {
    let mut out = Vec::new();
    let mut rest = html;
    let needle = "<losos-block";
    loop {
        let Some(start) = rest.find(needle) else {
            if !rest.trim().is_empty() || !out.is_empty() {
                // Keep meaningful trailing content; avoid emitting pure whitespace-only
                // top-level glue when it's the only thing, but preserve structure.
                if !rest.is_empty() {
                    out.push(Segment::Html(rest.to_string()));
                }
            }
            break;
        };
        if start > 0 {
            let before = &rest[..start];
            if !before.is_empty() {
                out.push(Segment::Html(before.to_string()));
            }
        }
        rest = &rest[start..];
        let Some(open_end) = find_tag_end(rest) else {
            out.push(Segment::Html(rest.to_string()));
            break;
        };
        let open_tag = &rest[..=open_end];
        let self_close = open_tag.trim_end().ends_with("/>") || is_void_close(open_tag);
        let name = attr(open_tag, "data-block").unwrap_or_default();
        let props = normalize_props(attr(open_tag, "data-props").as_deref());
        if self_close || open_tag.ends_with("/>") {
            out.push(Segment::Block {
                name,
                props: BlockProps {
                    props,
                    children_html: None,
                },
            });
            rest = &rest[open_end + 1..];
            continue;
        }
        rest = &rest[open_end + 1..];
        let close = "</losos-block>";
        if let Some(cidx) = rest.find(close) {
            let inner = &rest[..cidx];
            let children_html = if inner.trim().is_empty() {
                None
            } else {
                Some(inner.to_string())
            };
            out.push(Segment::Block {
                name,
                props: BlockProps {
                    props,
                    children_html,
                },
            });
            rest = &rest[cidx + close.len()..];
        } else {
            out.push(Segment::Block {
                name,
                props: BlockProps {
                    props,
                    children_html: None,
                },
            });
            break;
        }
    }
    out
}

fn find_tag_end(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = quote {
            if b == q {
                quote = None;
            }
        } else if b == b'"' || b == b'\'' {
            quote = Some(b);
        } else if b == b'>' {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn is_void_close(_tag: &str) -> bool {
    false
}

fn attr(tag: &str, name: &str) -> Option<String> {
    for q in ['"', '\''] {
        let needle = format!("{name}={q}");
        if let Some(i) = tag.find(&needle) {
            let s = &tag[i + needle.len()..];
            if let Some(j) = s.find(q) {
                let raw = &s[..j];
                return Some(unescape_attr(raw));
            }
        }
    }
    None
}

fn unescape_attr(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn normalize_props(json: Option<&str>) -> BTreeMap<String, BlockValue> {
    let mut out = BTreeMap::new();
    let Some(json) = json else { return out };
    let Ok(Value::Object(map)) = serde_json::from_str::<Value>(json) else {
        return out;
    };
    for (k, v) in map {
        match v {
            Value::Bool(true) => {
                out.insert(k, BlockValue::Flag);
            }
            Value::String(s) => {
                out.insert(k, BlockValue::Text(s));
            }
            Value::Bool(false) => { /* absent flag */ }
            Value::Number(n) => {
                out.insert(k, BlockValue::Text(n.to_string()));
            }
            _ => {}
        }
    }
    out
}

/// Resolve page HTML: draft/custom from store, else default embed.
pub fn resolve_html<'a>(state: &'a PagesState, id: &str) -> Option<&'a str> {
    state.html_for(id).or_else(|| defaults::default_html(id))
}

pub type BlockRenderer = fn(BlockProps) -> Element;

/// Render parsed segments using a block-name resolver.
#[component]
pub fn RenderSegments(
    segments: Vec<Segment>,
    render_block: Callback<(String, BlockProps), Element>,
) -> Element {
    rsx! {
        for (i, seg) in segments.into_iter().enumerate() {
            match seg {
                Segment::Html(html) => rsx! {
                    div { key: "h{i}", class: "losos-page-html", dangerous_inner_html: "{html}" }
                },
                Segment::Block { name, props } => rsx! {
                    Fragment { key: "b{i}", {render_block.call((name, props))} }
                },
            }
        }
    }
}

/// Render a page by id from store+defaults with the given block renderer.
#[component]
pub fn RenderedPage(
    id: String,
    state: PagesState,
    render_block: Callback<(String, BlockProps), Element>,
) -> Element {
    let Some(html) = resolve_html(&state, &id) else {
        // Not ready and no default: wait.
        return rsx! { div { class: "losos-page-loading" } };
    };
    let segments = parse_segments(html);
    rsx! {
        div { class: "losos-page", "data-page": "{id}",
            RenderSegments { segments, render_block }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_flag_and_text() {
        let p = normalize_props(Some(r#"{"hideStorage":true,"label":"hi"}"#));
        assert!(p.get("hideStorage").unwrap().is_flag());
        assert_eq!(p.get("label").unwrap().as_str(), Some("hi"));
    }

    #[test]
    fn parse_block_placeholders() {
        let html = r#"<losos-block data-block="BoxName" data-props="{}"></losos-block><losos-block data-block="Apps" data-props="{&quot;hideStorage&quot;:true}"></losos-block>"#;
        let segs = parse_segments(html);
        let blocks: Vec<_> = segs
            .iter()
            .filter_map(|s| match s {
                Segment::Block { name, props } => Some((name.as_str(), props.flag("hideStorage"))),
                _ => None,
            })
            .collect();
        assert_eq!(blocks, vec![("BoxName", false), ("Apps", true)]);
    }
}
