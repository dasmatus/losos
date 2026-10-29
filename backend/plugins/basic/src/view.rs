//! What every page needs before it draws: the catalogue in the reader's
//! language, the page's cookie, the built stylesheet, who is signed in and
//! the sidebar they may see (losos_page::nav, which the log reader draws
//! too).

use crate::host::Host;
use losos_page::i18n::{Locale, Messages};
use losos_page::shell::{Assets, Frame, NavEntry, Person, Prefs};

pub use losos_page::nav::*;

/// The catalogue in `locale`, from the built admin page; English when the
/// language's file is missing, and nothing when the page is not there.
pub fn messages(host: &mut dyn Host, locale: Locale) -> Messages {
    for l in [locale, Locale::En] {
        if let Some(bytes) = host.ui_file(&format!("basic/messages.{}.json", l.code())) {
            return Messages::parse(l, &bytes);
        }
    }
    Messages::from_pairs(locale, &[])
}

/// The reader's language: the page's cookie, else the browser's.
#[must_use]
pub fn locale(prefs: &Prefs, accept_language: &str) -> Locale {
    prefs
        .lang
        .unwrap_or_else(|| Locale::from_accept_language(accept_language))
}

pub fn assets(host: &mut dyn Host) -> Assets {
    host.ui_file("index.html")
        .map(|b| Assets::from_index(&String::from_utf8_lossy(&b)))
        .unwrap_or_default()
}

/// Everything a signed-in page draws around itself.
pub struct Ctx {
    pub m: Messages,
    pub prefs: Prefs,
    pub assets: Assets,
    pub me: Me,
    pub person: Person,
    pub nav: Vec<NavEntry>,
    /// The script page's address of this page (`/storage`).
    pub path: String,
    /// This page's own address (`/basic/storage`).
    pub here: String,
    pub rail_moved: bool,
    pub refresh: Option<u32>,
    pub pins: Option<Vec<NavEntry>>,
}

impl Ctx {
    #[must_use]
    pub fn frame<'a>(&'a self, title: &'a str) -> Frame<'a> {
        Frame {
            m: &self.m,
            assets: &self.assets,
            prefs: &self.prefs,
            here: &self.here,
            title,
            person: Some(&self.person),
            nav: &self.nav,
            tabs: TABS,
            rail_moved: self.rail_moved,
            refresh: self.refresh,
            pins: self.pins.as_deref(),
        }
    }
}
