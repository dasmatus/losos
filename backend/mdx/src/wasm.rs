//! The browser's way in: `render(source, flavour, resolver)` gives the
//! [`crate::Rendered`] result as JSON, and `tree(source, flavour)` the mdast
//! the WYSIWYG editor builds its document from. The resolver is a plain JS
//! object with `note`, `file` and `href` methods, each optional.

use crate::{Flavour, Resolver};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    /// `{ note?(target), file?(target), href?(path, sub) }` from the page.
    pub type JsResolver;

    #[wasm_bindgen(method, structural, catch, js_name = note)]
    fn js_note(this: &JsResolver, target: &str) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(method, structural, catch, js_name = file)]
    fn js_file(this: &JsResolver, target: &str) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(method, structural, catch, js_name = href)]
    fn js_href(this: &JsResolver, path: &str, sub: &str) -> Result<JsValue, JsValue>;
}

impl Resolver for JsResolver {
    fn note(&self, target: &str) -> Option<String> {
        self.js_note(target).ok().and_then(|v| v.as_string())
    }
    fn file(&self, target: &str) -> Option<String> {
        self.js_file(target).ok().and_then(|v| v.as_string())
    }
    fn href(&self, path: &str, sub: &str) -> String {
        self.js_href(path, sub)
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_else(|| crate::Plain.href(path, sub))
    }
}

fn flavour(name: &str) -> Flavour {
    if name == "md" {
        Flavour::Md
    } else {
        Flavour::Mdx
    }
}

fn to_js(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// A document as `Rendered` JSON. `flavour` is "mdx" or "md". A document
/// that does not parse throws, with the place in the message.
#[wasm_bindgen]
pub fn render(
    source: &str,
    flavour_name: &str,
    resolver: Option<JsResolver>,
) -> Result<String, JsError> {
    let parsed = crate::parse(source, flavour(flavour_name)).map_err(to_js)?;
    let rendered = match &resolver {
        Some(r) => parsed.render(r),
        None => parsed.render(&crate::Plain),
    };
    serde_json::to_string(&rendered).map_err(to_js)
}

/// The parse of a document, as `{ tree, problems }` JSON: mdast with
/// positions, and the reserved names already refused.
#[wasm_bindgen]
pub fn tree(source: &str, flavour_name: &str) -> Result<String, JsError> {
    let parsed = crate::parse(source, flavour(flavour_name)).map_err(to_js)?;
    serde_json::to_string(&serde_json::json!({
        "tree": crate::tree_json(&parsed.tree),
        "problems": parsed.problems,
    }))
    .map_err(to_js)
}
