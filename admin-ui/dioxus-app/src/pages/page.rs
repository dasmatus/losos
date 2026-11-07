//! Page primitives: IDs, legacy wrapper, and block parsing helpers.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Known page identifiers (extend as parity grows).
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

/// Legacy/simple page wrapper. Prefer `renderer::RenderedPage` for defaults/custom
/// HTML rendering with `<losos-block>` placeholders and flag normalization.
#[component]
pub fn Page(id: PageId, children: Element) -> Element {
    rsx! {
        div { class: "losos-page", "data-page": id.as_str(),
            {children}
        }
    }
}

/// Parse `<losos-block data-block="Name" data-props='...'>` placeholders from
/// sanitized HTML. Returns (name, raw props map) in order. For full rendering
/// with flag normalization and children, use `renderer::parse_segments`.
pub fn parse_losos_blocks(html: &str) -> Vec<(String, HashMap<String, String>)> {
    let segs = super::renderer::parse_segments(html);
    let mut out = Vec::new();
    for seg in segs {
        if let super::renderer::Segment::Block { name, props } = seg {
            let mut map = HashMap::new();
            for (k, v) in props.props {
                match v {
                    super::renderer::BlockValue::Flag => {
                        map.insert(k, "true".to_string());
                    }
                    super::renderer::BlockValue::Text(s) => {
                        map.insert(k, s);
                    }
                }
            }
            out.push((name, map));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_blocks_finds_placeholders() {
        let html = r#"<losos-block data-block="BoxName"></losos-block><losos-block data-block="Disk" data-props="{&quot;hide&quot;:true}"></losos-block>"#;
        let b = parse_losos_blocks(html);
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].0, "BoxName");
        assert_eq!(b[1].0, "Disk");
    }
}
