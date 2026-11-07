//! MDX/page system for Dioxus (Slice 4 foundation).
//!
//! - `defaults`: build-time embedded HTML from `admin-ui/app/src/pages/*.mdx`
//!   (generated via losos-mdx; no runtime MDX parser for defaults).
//! - `store`: `/api/pages` custom pages, drafts, style (best-effort, web storage-aware).
//! - `renderer`: parses sanitized HTML, renders `<losos-block data-block data-props>`
//!   with flag normalization and children, falling back to defaults.
//! - `overview`: overview/home parity scaffolding (context/facts + wired blocks).

pub mod blocks;
pub mod defaults;
pub mod overview;
pub mod page;
pub mod renderer;
pub mod store;

pub use overview::{AboutPage, OverviewPage, StoragePage};
pub use page::{Page, PageId};
pub use renderer::{BlockProps, BlockValue, RenderedPage};

use dioxus::prelude::*;

use crate::app::ConfigSignal;
use crate::store::BoxConfig;

use self::store::PagesState;

pub type PagesSignal = Signal<PagesState>;

/// Provide pages store context (seed from web cache; load from `/api/pages` best-effort).
pub fn provide_pages_store() -> PagesSignal {
    let mut state = PagesState::default();
    store::seed_from_cache(&mut state);
    use_context_provider(|| Signal::new(state))
}

/// Get pages store from context, or create a local fallback.
pub fn use_pages_store() -> PagesSignal {
    // Try context first; if unavailable, provide a local one.
    // `try_use_context` avoids panics in isolated renders.
    if let Some(sig) = try_use_context::<PagesSignal>() {
        sig
    } else {
        let mut state = PagesState::default();
        store::seed_from_cache(&mut state);
        use_signal(|| state)
    }
}

/// Ensure `/api/pages` is loaded (best-effort). Reloads when host/token changes.
pub fn use_load_pages(mut pages: PagesSignal) {
    let config: ConfigSignal = use_context();
    let cfg = config();
    use_effect(use_reactive((&cfg.host, &cfg.token), move |(host, token)| {
        let cfg = config();
        // Avoid stale loads when host/token just changed.
        let _ = (host, token);
        spawn(async move {
            let mut st = pages.write();
            store::load_pages(&cfg, &mut st).await;
        });
    }));
}
