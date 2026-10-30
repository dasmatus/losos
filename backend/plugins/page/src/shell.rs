//! The admin page's frame for a browser without JavaScript: the document,
//! the top bar (components/app-bar.tsx), the sidebar
//! (components/app-sidebar.tsx on components/ui/sidebar.tsx), the phone's
//! bottom bar and its More sheet (components/ui/bottom-nav.tsx,
//! components/ui/drawer.tsx), in the same markup and classes.
//!
//! What moves by script in those components moves here by CSS on a form
//! control: a menu is a `<details>`, a fold and the phone's sheet are a
//! checkbox with its `<label>`, and a choice that should outlast the page
//! (the language, the theme, the rail, a notice put away) is a small form
//! that sets the page's cookie and comes back. The motion is the
//! components' own, written as transitions and keyframes in basic.css.

use crate::html::{el, El, Html};
use crate::i18n::{Locale, Messages, Var};
use crate::icons;
use crate::ui::{self, icon};

/// Where every page of this kind lives.
pub const BASE: &str = "/basic";

/// The address of the script page's `path` here: `/` is `/basic/`,
/// `/settings/network` is `/basic/settings/network`.
#[must_use]
pub fn href(path: &str) -> String {
    let path = path.trim_end_matches('/');
    if path.is_empty() {
        format!("{BASE}/")
    } else {
        format!("{BASE}{path}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Auto,
    Light,
    Dark,
}

impl Theme {
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Theme::Auto => "auto",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }

    #[must_use]
    pub fn from_code(code: &str) -> Option<Theme> {
        match code {
            "auto" => Some(Theme::Auto),
            "light" => Some(Theme::Light),
            "dark" => Some(Theme::Dark),
            _ => None,
        }
    }
}

/// What the page's cookie remembers. It is written by `POST /basic/prefs`
/// and handed over by nginx as `X-Losos-Basic-Prefs`, in the shape
/// `theme=dark&lang=sk&rail=1&off=mesh.notpm`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Prefs {
    pub theme: Theme,
    /// `None` follows the browser, as the script page's "Browser language".
    pub lang: Option<Locale>,
    /// The sidebar is the narrow rail of icons.
    pub rail: bool,
    /// Notices put away: `mesh`.
    pub off: Vec<String>,
}

impl Prefs {
    #[must_use]
    pub fn parse(cookie: &str) -> Prefs {
        let mut prefs = Prefs::default();
        for pair in cookie.split('&') {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            match k {
                "theme" => prefs.theme = Theme::from_code(v).unwrap_or_default(),
                "lang" => prefs.lang = Locale::from_code(v),
                "rail" => prefs.rail = v == "1",
                "off" => {
                    prefs.off = v
                        .split('.')
                        .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_lowercase()))
                        .map(str::to_string)
                        .collect();
                }
                _ => {}
            }
        }
        prefs
    }

    /// The cookie's value. Only letters, digits and `=&.`, so it needs no
    /// quoting.
    #[must_use]
    pub fn encode(&self) -> String {
        let mut parts = vec![format!("theme={}", self.theme.code())];
        if let Some(lang) = self.lang {
            parts.push(format!("lang={}", lang.code()));
        }
        if self.rail {
            parts.push("rail=1".to_string());
        }
        if !self.off.is_empty() {
            parts.push(format!("off={}", self.off.join(".")));
        }
        parts.join("&")
    }

    #[must_use]
    pub fn is_off(&self, notice: &str) -> bool {
        self.off.iter().any(|n| n == notice)
    }
}

/// The built admin page's own files, read from its index.html: one
/// stylesheet and the logo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Assets {
    pub styles: Vec<String>,
    pub logo: String,
}

impl Assets {
    /// The `<link rel="stylesheet">` and `<link rel="icon">` addresses in
    /// the script page's index.html.
    #[must_use]
    pub fn from_index(index: &str) -> Assets {
        let mut assets = Assets::default();
        for tag in index.split('<').filter(|t| t.starts_with("link ")) {
            let Some(href) = attr_value(tag, "href") else {
                continue;
            };
            if !href.starts_with("/assets/") {
                continue;
            }
            match attr_value(tag, "rel").as_deref() {
                Some("stylesheet") => assets.styles.push(href),
                Some("icon") => assets.logo = href,
                _ => {}
            }
        }
        assets
    }
}

fn attr_value(tag: &str, name: &str) -> Option<String> {
    let at = tag.find(&format!(" {name}=\""))? + name.len() + 3;
    let rest = &tag[at..];
    Some(rest[..rest.find('"')?].to_string())
}

/// Who is signed in, as the top bar shows them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Person {
    pub label: String,
    pub tip: String,
    /// The avatar's letter; `None` draws the person outline (the spare key).
    pub initial: Option<String>,
    /// components/person.tsx's colour name.
    pub accent: Option<String>,
    /// The profile picture's address, when they have one.
    pub picture: Option<String>,
    /// Whether "Your account" is in their menu.
    pub account: bool,
}

/// One place in the sidebar.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NavEntry {
    pub key: String,
    /// `None` for a fold that is only a heading (Settings).
    pub href: Option<String>,
    /// Where the rail sends a fold.
    pub rail_href: Option<String>,
    pub label: String,
    pub glyph: &'static str,
    /// This is the page on screen.
    pub current: bool,
    pub children: Vec<NavEntry>,
}

impl NavEntry {
    #[must_use]
    pub fn holds_current(&self) -> bool {
        self.current || self.children.iter().any(NavEntry::holds_current)
    }
}

/// One page: the frame's inputs.
pub struct Frame<'a> {
    pub m: &'a Messages,
    pub assets: &'a Assets,
    pub prefs: &'a Prefs,
    /// This page's own address under [`BASE`], for the forms that come back.
    pub here: &'a str,
    pub title: &'a str,
    pub person: Option<&'a Person>,
    pub nav: &'a [NavEntry],
    /// The phone's bottom bar: the keys of `nav` it shows, in order.
    pub tabs: &'a [&'a str],
    /// The sidebar went narrow or wide on the way here, so it moves.
    pub rail_moved: bool,
    /// Seconds until the page asks again by itself, while something runs.
    pub refresh: Option<u32>,
    /// The sidebar's Pinned section, when the box sent the list.
    pub pins: Option<&'a [NavEntry]>,
    /// The page's name on `<html data-page>`, as the script page sets it,
    /// so an administrator's stylesheet can pick one page out.
    pub page: Option<&'a str>,
}

/// A whole page: the frame around `main`. With no `nav`, only the top bar
/// is drawn (sign-in, and the pages a person a rule holds may see).
#[must_use]
pub fn page(f: &Frame<'_>, main: Html) -> Html {
    let body = el("div")
        .class("losos-shell min-h-dvh text-ink")
        .child(app_bar(f))
        .child(if f.nav.is_empty() {
            Html::from(
                el("div")
                    .class("mx-auto w-full max-w-6xl px-4 pt-5 pb-12 sm:px-6")
                    .child(el("main").class("min-w-0 flex-1").child(main)),
            )
        } else {
            sidebar_and_main(f, main)
        })
        .when(!f.nav.is_empty(), |b| b.child(bottom_nav(f)));
    document(
        f.m.locale,
        f.prefs.theme,
        f.title,
        f.assets,
        f.refresh,
        f.page,
        body.into(),
    )
}

/// The `<html>` around a body: the script page's head, without a script.
#[must_use]
pub fn document(
    locale: Locale,
    theme: Theme,
    title: &str,
    assets: &Assets,
    refresh: Option<u32>,
    page: Option<&str>,
    body: Html,
) -> Html {
    let mut head = Html::trusted(
        "<meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1, viewport-fit=cover\"><meta name=\"color-scheme\" content=\"light dark\"><meta name=\"referrer\" content=\"no-referrer\">",
    );
    if let Some(seconds) = refresh {
        head.push(
            el("meta")
                .attr("http-equiv", "refresh")
                .attr("content", &seconds.to_string()),
        );
    }
    head.push(el("title").text(title));
    if !assets.logo.is_empty() {
        head.push(
            el("link")
                .attr("rel", "icon")
                .attr("type", "image/png")
                .attr("href", &assets.logo),
        );
    }
    for sheet in &assets.styles {
        head.push(el("link").attr("rel", "stylesheet").attr("href", sheet));
    }
    let theme_attr = match theme {
        Theme::Auto => String::new(),
        other => format!(" data-theme=\"{}\"", other.code()),
    };
    // Page names are lowercase words joined by dots (blocks.json).
    let page_attr = page
        .filter(|p| {
            p.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.')
        })
        .map(|p| format!(" data-page=\"{p}\""))
        .unwrap_or_default();
    Html::trusted(format!(
        "<!doctype html><html lang=\"{}\"{}{} data-basic=\"\"><head>{}</head><body>{}</body></html>",
        locale.code(),
        theme_attr,
        page_attr,
        head.as_str(),
        body.as_str()
    ))
}

// ── The top bar ─────────────────────────────────────────────────────────

fn app_bar(f: &Frame<'_>) -> Html {
    let m = f.m;
    el("header")
        .attr("data-slot", "app-bar")
        .class("sticky top-0 z-40 shrink-0 border-b border-line bg-surface/85 backdrop-blur-sm")
        .child(
            el("div")
                .class("flex w-full items-center gap-3 py-3 mx-auto max-w-6xl px-4 sm:px-6")
                .child(
                    el("a")
                        .attr("href", &href("/"))
                        .attr("title", &m.t("shell.logoTip"))
                        .class("inline-flex shrink-0 rounded-full focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent")
                        .child(
                            el("img")
                                .attr("alt", "LosOS")
                                .attr("width", "32")
                                .attr("height", "32")
                                .class("size-8")
                                .attr("src", &f.assets.logo),
                        ),
                )
                .child(
                    el("span")
                        .class("hidden shrink-0 sm:inline-flex")
                        .child(ui::badge(ui::Tone::Outline, &m.t("shell.thisBox"))),
                )
                .child(el("div").class("flex-1"))
                .child(match f.person {
                    Some(p) => person_menu(f, p),
                    None => display_menu(f),
                }),
        )
        .into()
}

/// A form that changes the page's cookie and comes back here.
fn prefs_form(f: &Frame<'_>) -> El {
    el("form")
        .attr("method", "post")
        .attr("action", &format!("{BASE}/prefs"))
        .child(ui::hidden("back", f.here))
}

/// The opening motion and the click-away of a `<details>` menu are
/// basic.css's (`[data-basic-menu]`).
fn menu(name: &str) -> El {
    el("details")
        .attr("data-basic-menu", name)
        .class("relative")
}

const SUMMARY: &str = "list-none [&::-webkit-details-marker]:hidden";

const POPUP: &str = "absolute top-full z-50 mt-1.5 rounded-card border border-line bg-surface p-1 text-ink shadow-pop outline-none";
const MENU_ITEM: &str = "flex w-full cursor-pointer items-center gap-2 rounded-control px-2 py-1.5 text-[13px] text-ink outline-none select-none transition-colors duration-100 hover:bg-accent-wash focus-visible:bg-accent-wash";

/// components/preferences-menu.tsx's two rows, the appearance and the
/// language. There each opens a submenu; here each is a disclosure that
/// opens its choices under it, buttons of one form that changes the page's
/// cookie and comes back.
fn preference_rows(f: &Frame<'_>) -> [Html; 2] {
    let m = f.m;
    let theme_choices = [
        (Theme::Auto, icons::COMPUTER, m.t("ui.theme.auto")),
        (Theme::Light, icons::SUN_03, m.t("ui.theme.light")),
        (Theme::Dark, icons::MOON_02, m.t("ui.theme.dark")),
    ];
    let (theme_glyph, theme_now) = theme_choices
        .iter()
        .find(|(t, _, _)| *t == f.prefs.theme)
        .map_or((icons::COMPUTER, m.t("ui.theme.auto")), |(_, g, l)| {
            (*g, l.clone())
        });
    let theme = choice_row(
        f,
        "theme",
        theme_glyph,
        &m.t("ui.theme.label"),
        &theme_now,
        theme_choices
            .iter()
            .map(|(t, glyph, label)| {
                choice(
                    "theme",
                    t.code(),
                    label,
                    Some(glyph),
                    None,
                    f.prefs.theme == *t,
                )
            })
            .collect(),
    );
    let lang_now = f
        .prefs
        .lang
        .map_or_else(|| m.t("ui.language.auto"), |l| l.own_name().to_string());
    let mut langs = vec![choice(
        "lang",
        "auto",
        &m.t("ui.language.auto"),
        None,
        None,
        f.prefs.lang.is_none(),
    )];
    langs.extend([Locale::En, Locale::Sk, Locale::De].into_iter().map(|l| {
        choice(
            "lang",
            l.code(),
            l.own_name(),
            None,
            Some(l.code()),
            f.prefs.lang == Some(l),
        )
    }));
    let language = choice_row(
        f,
        "language",
        icons::LANGUAGE_CIRCLE,
        &m.t("ui.language.label"),
        &lang_now,
        langs,
    );
    [theme, language]
}

/// One row: its name and the current choice, opening the choices below.
fn choice_row(
    f: &Frame<'_>,
    slot: &str,
    glyph: &str,
    label: &str,
    current: &str,
    choices: Vec<Html>,
) -> Html {
    el("details")
        .attr("data-slot", &format!("{slot}-menu"))
        .class("group/choice")
        .child(
            el("summary")
                .class(SUMMARY)
                .class(MENU_ITEM)
                .child(icon(glyph, 14, ""))
                .child(el("span").class("flex-1").text(label))
                .child(
                    el("span")
                        .class("max-w-[8rem] truncate text-[12px] text-muted")
                        .text(current),
                )
                .child(icon(
                    icons::ARROW_RIGHT_01,
                    14,
                    "text-faint transition-[rotate] duration-150 ease-snap group-open/choice:rotate-90",
                )),
        )
        .child(
            prefs_form(f)
                .attr("role", "group")
                .attr("aria-label", label)
                .attr("data-slot", &format!("{slot}-choices"))
                .class("pb-1 pl-4")
                .children(choices),
        )
        .into()
}

fn choice(
    name: &str,
    value: &str,
    text: &str,
    glyph: Option<&str>,
    lang: Option<&str>,
    on: bool,
) -> Html {
    el("button")
        .attr("type", "submit")
        .attr("name", name)
        .attr("value", value)
        .attr_opt("lang", lang)
        .attr("role", "menuitemradio")
        .attr("aria-checked", if on { "true" } else { "false" })
        .class(MENU_ITEM)
        .child(match glyph {
            Some(g) => icon(g, 14, ""),
            None => el("span")
                .attr("aria-hidden", "true")
                .class("size-[14px] shrink-0")
                .into(),
        })
        .child(el("span").class("flex-1 text-left").text(text))
        .child(on.then(|| icon(icons::TICK_02, 14, "text-accent")))
        .into()
}

/// components/preferences-menu.tsx's DisplayMenu: the two rows behind one
/// button, for a page nobody is signed in to.
fn display_menu(f: &Frame<'_>) -> Html {
    let m = f.m;
    let [theme, language] = preference_rows(f);
    menu("display")
        .child(
            el("summary")
                .attr("data-testid", "display-menu")
                .attr("aria-label", &m.t("ui.display.label"))
                .attr("title", &m.t("ui.display.label"))
                .class(SUMMARY)
                .class("inline-flex size-[34px] cursor-pointer items-center justify-center rounded-control text-faint hover:bg-sunk hover:text-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent")
                .child(icon(icons::PAINT_BOARD, 18, "")),
        )
        .child(
            el("div")
                .attr("role", "menu")
                .attr("data-slot", "basic-popup")
                .class(POPUP)
                .class("right-0 w-64 max-w-[calc(100vw-2rem)]")
                .child(theme)
                .child(language),
        )
        .into()
}

/// components/person.tsx's PersonAvatar at size xs.
#[must_use]
pub fn avatar(person: &Person) -> Html {
    let inner: Html = match (&person.picture, &person.initial) {
        (Some(src), _) => el("img")
            .attr("src", src)
            .attr("alt", "")
            .class("size-full object-cover")
            .into(),
        (None, Some(letter)) => el("span").attr("aria-hidden", "true").text(letter).into(),
        (None, None) => return icon(icons::USER_CIRCLE, 18, ""),
    };
    el("span")
        .attr("data-slot", "avatar")
        .attr_opt("data-accent", person.accent.as_deref())
        .class("relative inline-flex shrink-0 items-center justify-center overflow-hidden rounded-full bg-person-wash font-medium text-person select-none size-6 text-[10px]")
        .child(inner)
        .into()
}

fn person_menu(f: &Frame<'_>, person: &Person) -> Html {
    let m = f.m;
    menu("person")
        .class("group/person min-w-0 shrink")
        .child(
            el("summary")
                .attr("data-testid", "signed-in-as")
                .attr("aria-label", &format!("{}: {}", person.label, person.tip))
                .class(SUMMARY)
                .class("inline-flex h-[34px] min-w-0 cursor-pointer items-center gap-1.5 rounded-control px-2 text-sm text-ink hover:bg-sunk group-open/person:bg-sunk focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent")
                .child(avatar(person))
                .child(
                    el("span")
                        .attr("data-testid", "signed-in-name")
                        .class("hidden max-w-[12rem] truncate sm:inline")
                        .text(&person.label),
                )
                .child(icon(
                    icons::ARROW_DOWN_01,
                    14,
                    "text-faint transition-[rotate] duration-150 ease-snap group-open/person:rotate-180 max-sm:hidden",
                )),
        )
        .child(
            el("div")
                .attr("role", "menu")
                .attr("data-slot", "person-menu")
                .class(POPUP)
                .class("right-0 w-64 max-w-[calc(100vw-2rem)]")
                .child(
                    el("div")
                        .class("flex flex-col gap-0.5 px-2 pt-1.5 pb-2")
                        .child(el("span").class("truncate text-[13.5px] font-medium").text(&person.label))
                        .child(el("span").class("text-[12px] leading-snug text-muted").text(&person.tip)),
                )
                .child(el("div").attr("role", "separator").class("my-1 h-px bg-hair"))
                .children(preference_rows(f))
                .child(el("div").attr("role", "separator").class("my-1 h-px bg-hair"))
                .when(person.account, |d| {
                    d.child(
                        el("a")
                            .attr("role", "menuitem")
                            .attr("href", &href("/settings/account"))
                            .class(MENU_ITEM)
                            .child(icon(icons::USER_CIRCLE, 14, ""))
                            .child(el("span").class("flex-1").text(&m.t("settings.panes.account.label"))),
                    )
                })
                .child(
                    el("form")
                        .attr("method", "post")
                        .attr("action", &format!("{BASE}/sign-out"))
                        .child(
                            el("button")
                                .attr("type", "submit")
                                .attr("role", "menuitem")
                                .attr("data-testid", "sign-out")
                                .class(MENU_ITEM)
                                .child(icon(icons::LOGOUT_01, 14, ""))
                                .child(el("span").class("flex-1 text-left").text(&m.t("shell.signOut"))),
                        ),
                ),
        )
        .into()
}

// ── The sidebar ─────────────────────────────────────────────────────────

const SIDEBAR_ICON_BUTTON: &str = "flex size-8 shrink-0 items-center justify-center rounded-control text-muted transition-colors duration-150 hover:bg-surface hover:text-ink focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:outline-none";

fn sidebar_and_main(f: &Frame<'_>, main: Html) -> Html {
    let rail = f.prefs.rail;
    el("div")
        .attr("data-slot", "sidebar-wrapper")
        .class("group/sidebar-wrapper mx-auto flex w-full px-4 pt-5 pb-12 sm:px-6 max-w-6xl max-md:flex-col max-md:gap-3 md:gap-6")
        .child(
            el("nav")
                .attr("data-slot", "sidebar")
                .attr("data-state", if rail { "collapsed" } else { "expanded" })
                .attr("data-collapsible", if rail { "icon" } else { "" })
                .attr_opt("data-moved", f.rail_moved.then_some(if rail { "in" } else { "out" }))
                .attr("aria-label", &f.m.t("shell.nav.label"))
                .class("group/sidebar flex shrink-0 flex-col rounded-card border border-line bg-sunk p-2 sticky top-20 max-h-[calc(100dvh-6rem)] self-start overflow-x-hidden overflow-y-auto w-(--sidebar-width) transition-[width] duration-200 ease-out data-[collapsible=icon]:w-(--sidebar-width-icon) max-md:hidden")
                .child(sidebar_header(f))
                .child(menu_list(f, "d", rail)),
        )
        .child(el("main").class("min-w-0 flex-1").child(main))
        .into()
}

fn sidebar_header(f: &Frame<'_>) -> Html {
    let m = f.m;
    let rail = f.prefs.rail;
    let label = m.t(if rail {
        "shell.nav.expand"
    } else {
        "shell.nav.collapse"
    });
    let search = m.t("palette.open").replace(" ({shortcut})", "");
    el("div")
        .attr("data-slot", "sidebar-header")
        .class("flex items-center gap-1.5 group-data-[collapsible=icon]/sidebar:flex-col")
        .child(
            prefs_form(f).class("contents").child(
                el("button")
                    .attr("type", "submit")
                    .attr("name", "rail")
                    .attr("value", if rail { "0" } else { "1" })
                    .attr("data-slot", "sidebar-trigger")
                    .attr("aria-label", &label)
                    .attr("title", &label)
                    .attr("aria-expanded", if rail { "false" } else { "true" })
                    .class(SIDEBAR_ICON_BUTTON)
                    .child(icon(icons::SIDEBAR_LEFT, 17, "")),
            ),
        )
        .child(
            el("a")
                .attr("href", &format!("{BASE}/search"))
                .attr("aria-label", &search)
                .attr("title", &search)
                .class(SIDEBAR_ICON_BUTTON)
                .class("ml-auto group-data-[collapsible=icon]/sidebar:ml-0")
                .child(icon(icons::SEARCH_01, 16, "")),
        )
        .into()
}

const MENU_BUTTON: &str = "peer/menu-button flex w-full min-w-0 items-center gap-2 overflow-hidden rounded-control px-2 py-1.5 text-left text-[13px] whitespace-nowrap outline-none transition-[background-color,color,width,padding] duration-150 focus-visible:ring-2 focus-visible:ring-accent/40 [&>span:last-child]:truncate group-data-[collapsible=icon]/sidebar:size-9 group-data-[collapsible=icon]/sidebar:justify-center group-data-[collapsible=icon]/sidebar:px-0 group-has-data-[slot=sidebar-menu-action]/menu-item:pr-8";
const ON: &str = "bg-accent font-medium text-surface";
const OFF: &str = "text-ink hover:bg-surface active:bg-surface";
const GLYPH: &str = "flex size-[21px] shrink-0 items-center justify-center rounded-[6px] transition-colors duration-150";
const GLYPH_ON: &str = "bg-accent-wash text-accent";
const GLYPH_OFF: &str = "bg-surface text-muted";

fn glyph(name: &str, on: bool) -> Html {
    el("span")
        .attr("aria-hidden", "true")
        .class(GLYPH)
        .class(if on { GLYPH_ON } else { GLYPH_OFF })
        .child(icon(name, 13, ""))
        .into()
}

/// The menu, drawn twice on a page: `d` beside the page and `p` in the
/// phone's sheet, so the checkboxes of their folds need ids of their own.
fn menu_list(f: &Frame<'_>, side: &str, rail: bool) -> Html {
    el("div")
        .attr("data-slot", "sidebar-content")
        .class("mt-2 flex min-h-0 flex-col gap-2")
        .child(
            el("ul")
                .attr("data-slot", "sidebar-menu")
                .class("flex min-w-0 flex-col gap-0.5")
                .children(f.nav.iter().map(|entry| menu_item(f, entry, side, rail))),
        )
        .child(f.pins.map(|pins| pinned(f, pins, rail)))
        .into()
}

/// app-sidebar.tsx's PinnedSection: the owner's pins under the sections,
/// with the way to change them.
fn pinned(f: &Frame<'_>, pins: &[NavEntry], rail: bool) -> Html {
    let m = f.m;
    if pins.is_empty() && rail {
        return Html::new();
    }
    let edit = href("/pins");
    let list = if pins.is_empty() {
        Html::from(
            el("a")
                .attr("href", &edit)
                .class("mx-1 rounded-control border border-dashed border-line px-2.5 py-2 text-left text-[12.5px] leading-snug text-muted no-underline transition-colors duration-150 hover:border-accent/45 hover:text-ink")
                .text(&m.t("pins.empty")),
        )
    } else {
        Html::from(
            el("ul")
                .attr("data-slot", "sidebar-menu")
                .class("flex min-w-0 flex-col gap-0.5")
                .children(pins.iter().map(|p| {
                    el("li")
                        .attr("data-slot", "sidebar-menu-item")
                        .class("group/menu-item relative")
                        .child(
                            el("a")
                                .attr("href", p.href.as_deref().unwrap_or("#"))
                                .attr("title", &p.label)
                                .attr("data-slot", "sidebar-menu-button")
                                .class(MENU_BUTTON)
                                .class(OFF)
                                .child(glyph(p.glyph, false))
                                .child(
                                    el("span")
                                        .class("group-data-[collapsible=icon]/sidebar:sr-only")
                                        .text(&p.label),
                                ),
                        )
                })),
        )
    };
    el("div")
        .attr("data-slot", "sidebar-group")
        .attr("role", "group")
        .attr("aria-label", &m.t("pins.title"))
        .attr("data-pins", "true")
        .class("flex min-w-0 flex-col mt-2")
        .child(
            el("div")
                .attr("aria-hidden", "true")
                .class("mx-2 mb-1.5 hidden h-px bg-line group-data-[collapsible=icon]/sidebar:block"),
        )
        .child(
            el("div")
                .class("flex items-center group-data-[collapsible=icon]/sidebar:hidden")
                .child(
                    el("div")
                        .attr("data-slot", "sidebar-group-label")
                        .class("px-2 pt-1 text-[11.5px] font-medium tracking-wide text-faint group-data-[collapsible=icon]/sidebar:sr-only flex-1 pb-1")
                        .text(&m.t("pins.title")),
                )
                .child(
                    el("a")
                        .attr("href", &edit)
                        .attr("aria-label", &m.t("pins.edit"))
                        .attr("title", &m.t("pins.edit"))
                        .class("mr-1 flex size-6 items-center justify-center rounded-[6px] text-faint transition-[background-color,color] duration-150 hover:bg-surface hover:text-ink focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:outline-none")
                        .child(icon(icons::PENCIL_EDIT_02, 13, "")),
                ),
        )
        .child(list)
        .into()
}

fn menu_item(f: &Frame<'_>, entry: &NavEntry, side: &str, rail: bool) -> Html {
    let on = entry.current;
    let target = if rail {
        entry.rail_href.as_ref().or(entry.href.as_ref())
    } else {
        entry.href.as_ref()
    };
    let label = el("span")
        .class("group-data-[collapsible=icon]/sidebar:sr-only")
        .text(&entry.label);
    let holds = target.is_none() && entry.holds_current();
    let button = match target {
        Some(to) => el("a")
            .attr("href", to)
            .attr_opt("aria-current", on.then_some("page"))
            .attr("title", &entry.label),
        // A fold with no page of its own opens and closes itself.
        None => el("label")
            .attr("for", &format!("fold-{side}-{}", entry.key))
            .class("cursor-pointer"),
    }
    .attr("data-slot", "sidebar-menu-button")
    .attr_opt("data-active", on.then_some("true"))
    .class(MENU_BUTTON)
    .class(if on { ON } else { OFF })
    // A fold that holds the page on screen marks itself, as the script
    // page's does.
    .when(holds, |b| b.class("font-medium"))
    .child(glyph(entry.glyph, on || holds))
    .child(label)
    .when(target.is_none(), |b| {
        b.child(
            el("span")
                .class("ml-auto flex text-muted group-data-[collapsible=icon]/sidebar:hidden")
                .child(icon(
                    icons::ARROW_RIGHT_01,
                    14,
                    "transition-transform duration-200 group-has-[>input:checked]/menu-item:rotate-90",
                )),
        )
    });

    let mut li = el("li")
        .attr("data-slot", "sidebar-menu-item")
        .class("group/menu-item relative");
    if entry.children.is_empty() {
        return li.child(button).into();
    }
    let id = format!("fold-{side}-{}", entry.key);
    let open = entry.holds_current();
    li = li
        .child(
            el("input")
                .attr("type", "checkbox")
                .id(&id)
                .class("sr-only")
                .attr(
                    "aria-label",
                    &f.m.tv(
                        "shell.nav.subEntries",
                        &[("name", Var::from(entry.label.as_str()))],
                    ),
                )
                .flag("checked", open),
        )
        .child(button);
    if entry.href.is_some() {
        li = li.child(
            el("label")
                .attr("for", &id)
                .attr("data-slot", "sidebar-menu-action")
                .attr("aria-hidden", "true")
                .class("absolute top-1 right-1 flex size-6 cursor-pointer items-center justify-center rounded-[6px] text-muted transition-[background-color,color] duration-150 hover:bg-surface hover:text-ink peer-data-[active=true]/menu-button:text-surface peer-data-[active=true]/menu-button:hover:bg-accent-wash peer-data-[active=true]/menu-button:hover:text-accent group-data-[collapsible=icon]/sidebar:hidden")
                .child(icon(
                    icons::ARROW_RIGHT_01,
                    14,
                    "transition-transform duration-200 group-has-[>input:checked]/menu-item:rotate-90",
                )),
        );
    }
    li.child(
        el("div")
            .attr("data-slot", "collapsible-content")
            .class("grid grid-rows-[0fr] transition-[grid-template-rows] duration-200 ease-out group-has-[>input:checked]/menu-item:grid-rows-[1fr]")
            .child(
                el("div").class("min-h-0 overflow-hidden").child(
                    el("ul")
                        .attr("data-slot", "sidebar-menu-sub")
                        .class("mt-0.5 ml-[18px] flex min-w-0 flex-col gap-0.5 border-l border-line py-0.5 pl-2 group-data-[collapsible=icon]/sidebar:hidden")
                        .children(entry.children.iter().map(sub_item)),
                ),
            ),
    )
    .into()
}

fn sub_item(entry: &NavEntry) -> Html {
    let on = entry.current;
    el("li")
        .attr("data-slot", "sidebar-menu-sub-item")
        .class("group/menu-sub-item relative")
        .child(
            el("a")
                .attr("href", entry.href.as_deref().unwrap_or("#"))
                .attr_opt("aria-current", on.then_some("page"))
                .attr("data-slot", "sidebar-menu-sub-button")
                .class("flex w-full min-w-0 items-center gap-2 rounded-control px-2 py-1 text-left text-[13px] whitespace-nowrap outline-none transition-colors duration-150 focus-visible:ring-2 focus-visible:ring-accent/40")
                .class(if on { ON } else { OFF })
                .child(glyph(entry.glyph, on))
                .child(el("span").class("truncate").text(&entry.label)),
        )
        .into()
}

// ── The phone ───────────────────────────────────────────────────────────

const TAB: &str = "group/tab flex h-14 w-full min-w-0 cursor-pointer flex-col items-center justify-center gap-1 rounded-control text-muted transition-[color,scale] duration-150 ease-snap active:scale-[0.97] data-active:text-ink focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-accent";

fn tab_glyph(name: &str) -> Html {
    el("span")
        .class("relative flex h-7 w-14 items-center justify-center")
        .child(
            el("span")
                .attr("aria-hidden", "true")
                .class("absolute inset-0 rounded-full bg-accent-wash opacity-0 scale-x-50 transition-[opacity,scale] duration-200 ease-snap group-data-active/tab:opacity-100 group-data-active/tab:scale-x-100"),
        )
        .child(icon(
            name,
            20,
            "relative transition-colors duration-150 group-data-active/tab:text-accent",
        ))
        .into()
}

fn tab_label(text: &str) -> Html {
    el("span")
        .class("max-w-full truncate px-1 text-[11px] leading-none font-medium")
        .text(text)
        .into()
}

/// The bottom bar and the sheet its More raises. The sheet's checkbox
/// comes first so the bar and the sheet can both follow it in CSS.
fn bottom_nav(f: &Frame<'_>) -> Html {
    let m = f.m;
    let tabs: Vec<&NavEntry> = f
        .tabs
        .iter()
        .filter_map(|key| f.nav.iter().find(|e| e.key == *key && e.href.is_some()))
        .take(4)
        .collect();
    let any_active = tabs.iter().any(|t| t.holds_current());
    let mut out = Html::new();
    out.push(
        el("input")
            .attr("type", "checkbox")
            .id("basic-more")
            .class("peer/more sr-only md:hidden")
            .attr("aria-label", &m.t("ui.bottomNav.more")),
    );
    out.push(
        el("nav")
            .attr("data-slot", "bottom-nav")
            .attr("aria-label", &m.t("ui.bottomNav.label"))
            .class("fixed inset-x-0 bottom-0 z-40 border-t border-line bg-surface/85 pb-[env(safe-area-inset-bottom,0px)] backdrop-blur-sm md:hidden")
            .child(
                el("ul")
                    .class("mx-auto grid max-w-lg auto-cols-fr grid-flow-col px-1")
                    .children(tabs.iter().map(|tab| {
                        let on = tab.holds_current();
                        el("li").class("min-w-0").child(
                            el("a")
                                .attr("href", tab.href.as_deref().unwrap_or("#"))
                                .attr_opt("aria-current", on.then_some("page"))
                                .attr_opt("data-active", on.then_some(""))
                                .class(TAB)
                                .child(tab_glyph(tab.glyph))
                                .child(tab_label(&tab.label)),
                        )
                    }))
                    .child(
                        el("li").class("min-w-0").child(
                            el("label")
                                .attr("for", "basic-more")
                                .attr("aria-haspopup", "dialog")
                                .attr_opt("data-active", (!any_active).then_some(""))
                                .attr("data-more", "")
                                .class(TAB)
                                .child(tab_glyph(icons::MENU_01))
                                .child(tab_label(&m.t("ui.bottomNav.more"))),
                        ),
                    ),
            ),
    );
    out.push(sheet(f));
    out
}

/// components/ui/drawer.tsx with the sidebar in it.
fn sheet(f: &Frame<'_>) -> Html {
    let m = f.m;
    el("div")
        .attr("data-slot", "basic-sheet")
        .class("pointer-events-none invisible fixed inset-0 z-50 transition-[visibility] duration-200 peer-checked/more:pointer-events-auto peer-checked/more:visible md:hidden")
        .child(
            el("label")
                .attr("for", "basic-more")
                .attr("data-slot", "drawer-overlay")
                .attr("aria-hidden", "true")
                .class("absolute inset-0 bg-black/30 opacity-0 transition-opacity duration-200 ease-snap"),
        )
        .child(
            el("div")
                .attr("data-slot", "drawer-content")
                .attr("role", "dialog")
                .attr("aria-label", &m.t("shell.nav.label"))
                .class("absolute inset-x-0 bottom-0 flex max-h-[85dvh] w-full flex-col gap-3 overflow-y-auto overscroll-contain rounded-t-card border-t border-line bg-sunk px-3 pt-2 pb-[calc(0.75rem+env(safe-area-inset-bottom,0px))] shadow-pop outline-none translate-y-full transition-[translate] duration-220 ease-snap")
                .child(el("div").attr("aria-hidden", "true").class("mx-auto h-1 w-10 shrink-0 rounded-full bg-line"))
                .child(
                    el("div")
                        .class("flex h-8 shrink-0 items-center gap-1.5 pr-10 pl-2")
                        .child(el("p").class("min-w-0 flex-1 truncate text-[14px] font-semibold text-ink").text(&m.t("shell.nav.label")))
                        .child(
                            el("a")
                                .attr("href", &format!("{BASE}/search"))
                                .attr("aria-label", &m.t("palette.open").replace(" ({shortcut})", ""))
                                .class(SIDEBAR_ICON_BUTTON)
                                .child(icon(icons::SEARCH_01, 16, "")),
                        ),
                )
                .child(
                    el("nav")
                        .attr("data-slot", "sidebar")
                        .attr("data-state", "expanded")
                        .attr("data-collapsible", "")
                        .attr("aria-label", &m.t("shell.nav.label"))
                        .class("group/sidebar flex min-h-0 flex-1 flex-col")
                        .child(menu_list(f, "p", false)),
                )
                .child(
                    el("label")
                        .attr("for", "basic-more")
                        .attr("data-slot", "drawer-close")
                        .attr("aria-label", &m.t("ui.dismiss"))
                        .class("absolute top-6 right-3 flex size-8 cursor-pointer items-center justify-center rounded-control text-muted transition-colors duration-150 hover:bg-surface hover:text-ink")
                        .child(icon(icons::CANCEL_01, 16, "")),
                ),
        )
        .into()
}

// ── Pieces every page uses ──────────────────────────────────────────────

/// components/mesh-notice.tsx, for a box that is not on a mesh or does
/// not share its storage. `link` is where the owner fixes it, when they
/// may and are not already there.
#[must_use]
pub fn mesh_notice(f: &Frame<'_>, not_joined: bool, link: Option<&str>, can_change: bool) -> Html {
    let m = f.m;
    let mut text = m.t(if not_joined {
        "ui.mesh.notJoined"
    } else {
        "ui.mesh.notSharing"
    });
    if !can_change {
        text.push(' ');
        text.push_str(&m.t("ui.mesh.askAdmin"));
    }
    let mut body = Html::from(el("p").text(&text));
    if let Some(to) = link {
        body.push(
            el("a")
                .attr("href", to)
                .attr("data-mesh-link", "")
                .class("inline-flex w-fit items-center gap-1 text-[12.5px] font-medium text-accent underline-offset-2 hover:underline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent")
                .text(&m.t(if not_joined { "ui.mesh.join" } else { "ui.mesh.share" })),
        );
    }
    ui::alert(
        "warn",
        Some(icons::SHARE_08),
        Some(&m.t("ui.mesh.title")),
        body,
    )
    .attr("data-notice", "mesh")
    .attr(
        "data-mesh",
        if not_joined {
            "notJoined"
        } else {
            "notSharing"
        },
    )
    .class("mb-4 pr-11")
    .child(
        prefs_form(f).class("contents").child(
            ui::button(ui::Variant::Ghost, ui::Size::IconSm)
                .attr("name", "off")
                .attr("value", "mesh")
                .attr("aria-label", &m.t("ui.dismiss"))
                .attr("title", &m.t("ui.dismiss"))
                .class("absolute top-2 right-2")
                .child(icon(icons::CANCEL_01, 16, "")),
        ),
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefs_round_trip_and_refuse_junk() {
        let p = Prefs::parse("theme=dark&lang=sk&rail=1&off=mesh.notpm");
        assert_eq!(p.theme, Theme::Dark);
        assert_eq!(p.lang, Some(Locale::Sk));
        assert!(p.rail);
        assert!(p.is_off("mesh"));
        assert_eq!(Prefs::parse(&p.encode()), p);
        let junk = Prefs::parse("theme=<x>&lang=xx&off=a\"b.c");
        assert_eq!(junk.theme, Theme::Auto);
        assert_eq!(junk.lang, None);
        assert_eq!(junk.off, vec!["c".to_string()]);
    }

    #[test]
    fn the_index_names_the_stylesheet_and_the_logo() {
        let index = r#"<head><link rel="icon" type="image/png" href="/assets/losos-AB.png"><script src="/theme-boot.js"></script><link rel="stylesheet" crossorigin href="/assets/styles-CD.css"><link rel="stylesheet" href="https://evil/x.css"></head>"#;
        let a = Assets::from_index(index);
        assert_eq!(a.logo, "/assets/losos-AB.png");
        assert_eq!(a.styles, vec!["/assets/styles-CD.css".to_string()]);
    }

    #[test]
    fn addresses_map_onto_the_base() {
        assert_eq!(href("/"), "/basic/");
        assert_eq!(href("/storage"), "/basic/storage");
        assert_eq!(href("/office/"), "/basic/office");
    }

    fn frame_with<'a>(
        m: &'a Messages,
        assets: &'a Assets,
        prefs: &'a Prefs,
        nav: &'a [NavEntry],
        person: Option<&'a Person>,
    ) -> Frame<'a> {
        Frame {
            m,
            assets,
            prefs,
            here: "/basic/",
            title: "LosOS",
            person,
            nav,
            tabs: &["overview", "storage"],
            rail_moved: false,
            refresh: None,
            pins: None,
            page: None,
        }
    }

    #[test]
    fn a_page_has_no_script_and_escapes_the_person() {
        let m = Messages::from_pairs(Locale::En, &[("shell.signOut", "Sign out")]);
        let assets = Assets {
            styles: vec!["/assets/s.css".into()],
            logo: "/assets/l.png".into(),
        };
        let prefs = Prefs {
            theme: Theme::Dark,
            ..Prefs::default()
        };
        let nav = vec![
            NavEntry {
                key: "overview".into(),
                href: Some(href("/")),
                label: "Overview".into(),
                glyph: icons::HOME_01,
                current: true,
                ..NavEntry::default()
            },
            NavEntry {
                key: "settings".into(),
                label: "Settings".into(),
                glyph: icons::SETTINGS_01,
                rail_href: Some(href("/settings")),
                children: vec![NavEntry {
                    key: "network".into(),
                    href: Some(href("/settings/network")),
                    label: "Network".into(),
                    glyph: icons::GLOBE_02,
                    ..NavEntry::default()
                }],
                ..NavEntry::default()
            },
        ];
        let person = Person {
            label: "<b>Eve</b>".into(),
            tip: "tip".into(),
            initial: Some("E".into()),
            account: true,
            ..Person::default()
        };
        let f = frame_with(&m, &assets, &prefs, &nav, Some(&person));
        let html = page(&f, Html::text("hello")).into_string();
        assert!(html.starts_with("<!doctype html><html lang=\"en\" data-theme=\"dark\""));
        assert!(!html.contains("<script"));
        assert!(!html.contains(" style="));
        assert!(html.contains("&lt;b&gt;Eve&lt;/b&gt;"));
        assert!(html.contains("href=\"/assets/s.css\""));
        assert!(html.contains("for=\"fold-d-settings\""));
        assert!(html.contains("id=\"fold-p-settings\""));
        assert!(html.contains("action=\"/basic/sign-out\""));
    }
}
