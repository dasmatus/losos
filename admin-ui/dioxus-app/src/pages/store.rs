//! Pages store: `/api/pages` custom pages + drafts + style (best-effort,
//! web storage-aware). Mirrors `admin-ui/app/src/pages/store.ts`.

use crate::actions;
use crate::store::BoxConfig;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const CACHE_KEY: &str = "losos.pages";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorTarget {
    Page(String),
    Style,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PagesState {
    /// False until the box has answered or the wait ran out.
    pub ready: bool,
    /// Page id to its HTML, for pages an administrator changed.
    pub custom: BTreeMap<String, String>,
    /// The stylesheet's version, or null for none.
    pub style: Option<String>,
    /// Page id to draft HTML being edited.
    pub drafts: BTreeMap<String, String>,
    /// Stylesheet draft.
    pub style_draft: Option<String>,
    /// What the editor has open.
    pub editor: Option<EditorTarget>,
}

impl PagesState {
    /// HTML to draw now: draft, custom, or None (caller falls back to default).
    /// Returns `Err(())` when not ready and no override exists (wait).
    pub fn html_for(&self, id: &str) -> Option<&str> {
        if let Some(d) = self.drafts.get(id) {
            return Some(d.as_str());
        }
        if let Some(c) = self.custom.get(id) {
            return Some(c.as_str());
        }
        None
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct PagesResponse {
    #[serde(default)]
    pages: BTreeMap<String, PageEntry>,
    #[serde(default)]
    style: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct PageEntry {
    #[serde(default)]
    html: String,
}

fn revive(doc: &serde_json::Value) -> (BTreeMap<String, String>, Option<String>) {
    let mut custom = BTreeMap::new();
    let pages = doc.get("pages").and_then(|v| v.as_object());
    if let Some(pages) = pages {
        for (id, page) in pages {
            if let Some(html) = page.get("html").and_then(|v| v.as_str()) {
                custom.insert(id.clone(), html.to_string());
            }
        }
    }
    let style = doc
        .get("style")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    (custom, style)
}

#[cfg(target_arch = "wasm32")]
fn read_cache() -> Option<(BTreeMap<String, String>, Option<String>)> {
    let win = web_sys::window()?;
    let storage = win.local_storage().ok()??;
    let raw = storage.get_item(CACHE_KEY).ok()??;
    let doc: serde_json::Value = serde_json::from_str(&raw).ok()?;
    Some(revive(&doc))
}

#[cfg(not(target_arch = "wasm32"))]
fn read_cache() -> Option<(BTreeMap<String, String>, Option<String>)> {
    None
}

#[cfg(target_arch = "wasm32")]
fn write_cache(custom: &BTreeMap<String, String>, style: &Option<String>) {
    let mut pages = serde_json::Map::new();
    for (id, html) in custom {
        pages.insert(
            id.clone(),
            serde_json::json!({ "html": html }),
        );
    }
    let doc = serde_json::json!({ "pages": pages, "style": style });
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item(CACHE_KEY, &doc.to_string());
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn write_cache(_custom: &BTreeMap<String, String>, _style: &Option<String>) {}

/// Apply a `/api/pages` response document to state.
pub fn take_pages(state: &mut PagesState, doc: &serde_json::Value) {
    let (custom, style) = revive(doc);
    write_cache(&custom, &style);
    state.custom = custom;
    state.style = style;
    state.ready = true;
}

/// Seed from web cache (best-effort) so first paint can show custom pages.
pub fn seed_from_cache(state: &mut PagesState) {
    if let Some((custom, style)) = read_cache() {
        state.custom = custom;
        state.style = style;
        state.ready = true;
    }
}

/// Fetch `/api/pages` best-effort. Marks ready on completion or timeout path.
pub async fn load_pages(config: &BoxConfig, state: &mut PagesState) {
    if config.host.trim().is_empty() || config.token.trim().is_empty() {
        state.ready = true;
        state.custom.clear();
        state.style = None;
        state.drafts.clear();
        state.style_draft = None;
        return;
    }
    // Best-effort: fetch; an older box / missing plugin falls back to defaults.
    match actions::dispatch(config, actions::Action::Get("/api/pages".into())).await {
        Ok(doc) => take_pages(state, &doc),
        Err(_) => {
            state.ready = true;
        }
    }
}

pub fn set_draft(state: &mut PagesState, id: &str, html: Option<String>) {
    match html {
        Some(h) => {
            state.drafts.insert(id.to_string(), h);
        }
        None => {
            state.drafts.remove(id);
        }
    }
}

pub fn set_style_draft(state: &mut PagesState, css: Option<String>) {
    state.style_draft = css;
}

pub fn open_editor(state: &mut PagesState, target: EditorTarget) {
    state.editor = Some(target);
    state.drafts.clear();
    state.style_draft = None;
}

pub fn close_editor(state: &mut PagesState) {
    state.editor = None;
    state.drafts.clear();
    state.style_draft = None;
}

/// Effective style URL (saved version), if any and no draft override active.
pub fn style_href(state: &PagesState) -> Option<String> {
    if state.style_draft.is_some() {
        return None;
    }
    state.style.as_ref().map(|v| {
        format!(
            "/api/pages/style.css?v={}",
            urlencoding::encode(v)
        )
    })
}
