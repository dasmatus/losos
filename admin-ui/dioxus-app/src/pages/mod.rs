//! MDX page handling for Dioxus.
//!
//! Rewrites the React MDX page system (`admin-ui/app/src/pages/*`) for RSX:
//! pages are defined by `blocks.json` and rendered as slotted block components.
//! Default page content (from *.mdx) is mapped to block layouts; admin-edited
//! HTML pages with `<losos-block>` placeholders can be rendered via the parser.

pub mod page;
pub mod blocks;

pub use page::{Page, PageId};
