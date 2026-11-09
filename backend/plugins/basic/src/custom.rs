//! The pages an administrator changed (the pages plugin,
//! `backend/plugins/pages/`), drawn without JavaScript.
//!
//! `GET /api/pages` hands back each changed page as the HTML losos-mdx made
//! of it, already through the crate's allowlist, with every block the page
//! keeps as a `<losos-block data-block data-props>` placeholder. Here each
//! placeholder becomes what this plugin draws for that block, as the script
//! page puts its own component there, and a `<Button>` gets the page's own
//! button classes. A page nobody changed is not in the answer, and draws
//! as it always has.

use losos_page::html::Html;
use losos_page::ui::{self, Size, Variant};
use serde_json::{Map, Value};

const OPEN: &str = "<losos-block";
const CLOSE: &str = "</losos-block>";

/// The HTML of page `id` when an administrator changed it.
#[must_use]
pub fn changed(pages: Option<&Value>, id: &str) -> Option<String> {
    pages?["pages"][id]["html"].as_str().map(str::to_string)
}

/// The address of the administrators' stylesheet, when there is one.
#[must_use]
pub fn style_href(pages: Option<&Value>) -> Option<String> {
    let version = pages?["style"].as_str().filter(|v| !v.is_empty())?;
    Some(format!("/api/pages/style.css?v={}", encode(version)))
}

/// `html` with each placeholder replaced by `block(name, props)`, which
/// draws nothing for a block it does not know.
pub fn splice(html: &str, mut block: impl FnMut(&str, &Map<String, Value>) -> Html) -> Html {
    let mut out = Html::new();
    let mut rest = html;
    while let Some(at) = rest.find(OPEN) {
        out.push(text(&rest[..at]));
        let tail = &rest[at..];
        let Some(open_end) = tail.find('>') else {
            break;
        };
        let attrs = &tail[OPEN.len()..open_end];
        let name = attr(attrs, "data-block").unwrap_or_default();
        let props = attr(attrs, "data-props")
            .and_then(|p| serde_json::from_str::<Value>(&p).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();
        out.push(block(&name, &props));
        // Past the matching close, nested placeholders included: what an
        // administrator wrote inside a block is the script page's to draw.
        let mut depth = 1usize;
        let mut at = open_end + 1;
        while depth > 0 {
            let next_open = tail[at..].find(OPEN).map(|i| i + at);
            let next_close = tail[at..].find(CLOSE).map(|i| i + at);
            match (next_open, next_close) {
                (Some(o), Some(c)) if o < c => {
                    depth += 1;
                    at = o + OPEN.len();
                }
                (_, Some(c)) => {
                    depth -= 1;
                    at = c + CLOSE.len();
                }
                (_, None) => {
                    at = tail.len();
                    break;
                }
            }
        }
        rest = &tail[at..];
    }
    out.push(text(rest));
    out
}

/// A run of the crate's HTML, with its buttons drawn as the page's own.
fn text(run: &str) -> Html {
    let mut run = run.to_string();
    for (name, variant) in [
        ("primary", Variant::Primary),
        ("secondary", Variant::Secondary),
        ("outline", Variant::Outline),
        ("ghost", Variant::Ghost),
        ("link", Variant::Link),
    ] {
        for (s, size) in [("sm", Size::Sm), ("md", Size::Md), ("lg", Size::Lg)] {
            let crate_class = format!("class=\"mdx-button mdx-button-{name} mdx-button-{s}\"");
            if run.contains(&crate_class) {
                let ours = format!(
                    "class=\"mdx-button no-underline {}\"",
                    ui::button_classes(variant, size)
                );
                run = run.replace(&crate_class, &ours);
            }
        }
    }
    // Through losos-mdx's allowlist on the box already (sanitize.rs): the
    // only markup in it is what that list lets through.
    Html::trusted(run)
}

/// One attribute's value from the crate's own markup, which always quotes
/// with `"` and escapes `"`, `&`, `<` and `>` inside.
fn attr(attrs: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let start = attrs.find(&key)? + key.len();
    let len = attrs[start..].find('"')?;
    Some(unescape(&attrs[start..start + len]))
}

fn unescape(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw(html: &str) -> String {
        splice(html, |name, props| {
            Html::trusted(format!(
                "[{name}{}]",
                if props.is_empty() {
                    String::new()
                } else {
                    format!(" {}", Value::Object(props.clone()))
                }
            ))
        })
        .into_string()
    }

    #[test]
    fn placeholders_become_blocks_and_the_text_stays() {
        let html = concat!(
            "<div class=\"mdx-document mdx-page-text\"><h1>Hi</h1></div>\n",
            "<losos-block data-block=\"BoxName\" data-props=\"{}\"></losos-block>\n",
            "<losos-block data-block=\"Apps\" data-props=\"{&quot;hideStorage&quot;:true}\"></losos-block>",
        );
        assert_eq!(
            draw(html),
            "<div class=\"mdx-document mdx-page-text\"><h1>Hi</h1></div>\n[BoxName]\n[Apps {\"hideStorage\":true}]"
        );
    }

    #[test]
    fn a_block_with_children_and_nested_blocks_is_one_block() {
        let html = "<losos-block data-block=\"A\" data-props=\"{}\"><p>x</p><losos-block data-block=\"B\" data-props=\"{}\"></losos-block></losos-block>after";
        assert_eq!(draw(html), "[A]after");
    }

    #[test]
    fn buttons_get_the_pages_classes() {
        let html = "<p><a href=\"/apps\" class=\"mdx-button mdx-button-secondary mdx-button-sm\">Apps</a></p>";
        let out = draw(html);
        assert!(
            out.contains(&ui::button_classes(Variant::Secondary, Size::Sm)),
            "{out}"
        );
        assert!(out.contains("href=\"/apps\""));
        assert!(!out.contains("mdx-button-secondary"));
    }

    #[test]
    fn a_page_nobody_changed_is_not_there() {
        let answer = serde_json::json!({ "pages": { "overview": { "html": "x" } }, "style": "3" });
        assert_eq!(changed(Some(&answer), "overview").as_deref(), Some("x"));
        assert_eq!(changed(Some(&answer), "settings.storage"), None);
        assert_eq!(changed(None, "overview"), None);
        assert_eq!(
            style_href(Some(&answer)).as_deref(),
            Some("/api/pages/style.css?v=3")
        );
        assert_eq!(
            style_href(Some(&serde_json::json!({ "pages": {}, "style": null }))),
            None
        );
    }
}
