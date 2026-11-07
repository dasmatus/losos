use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PageId {
    #[serde(rename = "overview")]
    Overview,
    #[serde(rename = "settings.storage")]
    SettingsStorage,
    #[serde(rename = "settings.about")]
    SettingsAbout,
}

impl PageId {
    pub fn as_str(&self) -> &'static str {
        match self {
            PageId::Overview => "overview",
            PageId::SettingsStorage => "settings.storage",
            PageId::SettingsAbout => "settings.about",
        }
    }
}

/// Props passed to a block component (matches the React/MDX block contract).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BlockProps {
    pub props: HashMap<String, String>,
    pub children: Option<String>,
}

/// Render a page by ID using the provided block renderer.
/// This handles the default MDX layouts (block placeholders in order) and can
/// be extended to parse admin-edited `<losos-block>` HTML.
#[component]
pub fn Page(id: PageId, children: Element) -> Element {
    // Default layouts mirror the .mdx files:
    // overview: BoxName, Applying, Apps, Glance
    // settings.storage: Disk
    // settings.about: About
    // The `children` slot is not used directly; blocks are rendered by
    // page-specific components. This component provides the page wrapper and
    // data-page attribute semantics.
    rsx! {
        div { class: "losos-page", "data-page": id.as_str(),
            {children}
        }
    }
}

/// Parse `<losos-block data-block="Name" data-props='...'>` placeholders from
/// sanitized HTML (admin-edited pages). Returns blocks in order.
pub fn parse_losos_blocks(html: &str) -> Vec<(String, HashMap<String, String>)> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find("<losos-block") {
        rest = &rest[start..];
        let Some(end_tag) = rest.find("</losos-block>").or_else(|| rest.find("/>")) else { break };
        let tag_end = if rest[end_tag..].starts_with("/>") { end_tag + 2 } else { end_tag + "</losos-block>".len() };
        let tag = &rest[..tag_end.min(rest.len())];
        if let Some(name) = attr(tag, "data-block") {
            let props = attr(tag, "data-props")
                .and_then(|s| serde_json::from_str::<HashMap<String, String>>(&s).ok())
                .unwrap_or_default();
            out.push((name, props));
        }
        rest = &rest[tag_end.min(rest.len())..];
    }
    out
}

fn attr(tag: &str, name: &str) -> Option<String> {
    // Handles data-block="X" and data-props='{"k":"v"}' / &quot; entities minimally.
    for q in ['"', '\''] {
        let needle = format!("{name}={q}");
        if let Some(i) = tag.find(&needle) {
            let s = &tag[i + needle.len()..];
            if let Some(j) = s.find(q) {
                let raw = &s[..j];
                return Some(raw.replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&"));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_blocks_finds_placeholders() {
        let html = r#"<losos-block data-block="BoxName"></losos-block><losos-block data-block="Disk" data-props="{&quot;hide&quot;:&quot;1&quot;}"></losos-block>"#;
        let b = parse_losos_blocks(html);
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].0, "BoxName");
        assert_eq!(b[1].0, "Disk");
    }
}
