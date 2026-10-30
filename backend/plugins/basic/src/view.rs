//! What every page needs before it draws: the catalogue in the reader's
//! language, the page's cookie, the built stylesheet, who is signed in and
//! the sidebar they may see (losos_page::nav, which the log reader draws
//! too).

use crate::host::Host;
use losos_page::i18n::{Locale, Messages};
use losos_page::icons;
use losos_page::shell::{self, Assets, Frame, NavEntry, Person, Prefs};
use serde_json::Value;

/// The panes in the script page's order (screens/settings/panes.ts), with
/// their icon and the section of backend/sections.json each works in.
pub const PANES: &[(&str, &str, &str)] = &[
    ("storage", icons::HARD_DRIVE, "storage"),
    ("mesh", icons::SHARE_08, "admin"),
    ("market", icons::SHOPPING_CART_01, "market"),
    ("machines", icons::COMPUTER_CLOUD, "market"),
    ("apps", icons::LAYOUT_GRID, "apps"),
    ("statistics", icons::CHART_LINE_DATA_01, "apps"),
    ("network", icons::GLOBE_02, "network"),
    ("look", icons::PAINT_BOARD, "look"),
    ("hardware", icons::CPU, "admin"),
    ("security", icons::SHIELD_01, "admin"),
    ("account", icons::USER_CIRCLE, "account"),
    ("users", icons::USER_MULTIPLE, "admin"),
    ("moderation", icons::FLAG_02, "moderation"),
    ("access", icons::KEY_01, "access"),
    ("credentials", icons::LOCK_PASSWORD, "admin"),
    ("problems", icons::ALERT_02, "admin"),
    ("advanced", icons::SLIDERS_HORIZONTAL, "admin"),
    ("history", icons::GIT_COMMIT, "admin"),
    ("backup", icons::CLOUD_UPLOAD, "backup"),
    ("about", icons::INFORMATION_CIRCLE, "admin"),
    ("reset", icons::ARROW_RELOAD_HORIZONTAL, "admin"),
];

/// Panes that sit at the top level of the address (lib/routes.ts).
const TOP_LEVEL: &[&str] = &["storage", "mesh", "machines", "apps"];

/// The script page's address of a pane.
#[must_use]
pub fn pane_path(pane: &str) -> String {
    if TOP_LEVEL.contains(&pane) {
        format!("/{pane}")
    } else {
        format!("/settings/{pane}")
    }
}

/// The pane a script page's address shows, if it is one.
#[must_use]
pub fn pane_at(path: &str) -> Option<&'static str> {
    let path = path.trim_end_matches('/');
    if path == "/settings" {
        return Some("network");
    }
    PANES
        .iter()
        .map(|(id, _, _)| *id)
        .find(|id| pane_path(id) == path)
}

#[must_use]
pub fn pane_glyph(pane: &str) -> &'static str {
    PANES
        .iter()
        .find(|(id, _, _)| *id == pane)
        .map_or(icons::SETTINGS_01, |p| p.1)
}

/// GET /api/me, read the way lib/api.ts reads it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Me {
    pub name: Option<String>,
    pub display_name: Option<String>,
    pub admin: bool,
    pub spare_key: bool,
    pub sections: Vec<String>,
    pub shared: Vec<String>,
    pub locked: bool,
    pub standalone: bool,
    pub picture: Option<String>,
    pub accent: Option<String>,
}

impl Me {
    #[must_use]
    pub fn from_json(v: &Value) -> Me {
        let strings = |key: &str| -> Vec<String> {
            v[key]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|s| s.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        let text = |key: &str| {
            v[key]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };
        Me {
            name: text("name"),
            display_name: text("displayName"),
            admin: v["admin"].as_bool().unwrap_or(false),
            spare_key: v["spareKey"].as_bool().unwrap_or(false),
            sections: strings("sections"),
            shared: strings("shared"),
            locked: v["locked"].as_array().is_some_and(|a| !a.is_empty()),
            standalone: v["standalone"].as_bool().unwrap_or(false),
            // Only an address on this box: the page's policy loads nothing else.
            picture: text("picture").filter(|p| p.starts_with('/') && !p.starts_with("//")),
            accent: text("accent").filter(|a| a.bytes().all(|b| b.is_ascii_lowercase())),
        }
    }

    #[must_use]
    pub fn can_use(&self, section: &str) -> bool {
        self.admin || self.sections.iter().any(|s| s == section)
    }

    #[must_use]
    pub fn shares(&self, app: &str) -> bool {
        self.shared.iter().any(|s| s == app)
    }

    /// screens/settings/panes.ts's `paneOpen`.
    #[must_use]
    pub fn pane_open(&self, pane: &str) -> bool {
        if matches!(pane, "users" | "account" | "moderation") && !self.standalone {
            return false;
        }
        if pane == "account" {
            return !self.spare_key && self.can_use("account");
        }
        if self.locked {
            return false;
        }
        let section = PANES
            .iter()
            .find(|(id, _, _)| *id == pane)
            .map_or("admin", |p| p.2);
        self.can_use(section)
    }

    /// The top bar's person (App.tsx's TopBar).
    #[must_use]
    pub fn person(&self, m: &Messages) -> Person {
        if self.spare_key {
            return Person {
                label: m.t("shell.me.spareKey"),
                tip: m.t("shell.me.spareKeyTip"),
                ..Person::default()
            };
        }
        let name = self.name.clone().unwrap_or_default();
        let label = self.display_name.clone().unwrap_or_else(|| name.clone());
        let tip = format!(
            "{} {}",
            m.tv("shell.me.signedIn", &[("name", name.as_str().into())]),
            m.t(if self.admin {
                "shell.me.admin"
            } else {
                "shell.me.limited"
            })
        );
        Person {
            initial: label.chars().next().map(|c| c.to_uppercase().collect()),
            label,
            tip,
            accent: self.accent.clone(),
            picture: self.picture.clone(),
            account: self.name.is_some(),
        }
    }
}

/// The sidebar this person sees, with `path` (a script page's address) as
/// the page on screen.
#[must_use]
pub fn nav(me: &Me, m: &Messages, path: &str, office: bool) -> Vec<NavEntry> {
    let pane = pane_at(path);
    let leaf = |id: &str| NavEntry {
        key: id.to_string(),
        href: Some(shell::href(&pane_path(id))),
        label: m.t(&format!("settings.panes.{id}.label")),
        glyph: pane_glyph(id),
        current: pane == Some(id),
        ..NavEntry::default()
    };
    if me.locked {
        return vec![leaf("account")];
    }
    let link = |key: &str, to: &str, label: &str, glyph: &'static str| NavEntry {
        key: key.to_string(),
        href: Some(shell::href(to)),
        label: m.t(label),
        glyph,
        current: path.trim_end_matches('/') == to.trim_end_matches('/'),
        ..NavEntry::default()
    };
    let mut out = vec![link("overview", "/", "shell.nav.overview", icons::HOME_01)];
    let push_pane =
        |out: &mut Vec<NavEntry>, mut entry: NavEntry, pane_id: &str, children: Vec<NavEntry>| {
            if me.pane_open(pane_id) {
                entry.current = pane == Some(pane_id);
                entry.children = children;
                out.push(entry);
            } else {
                out.extend(children);
            }
        };
    let apps = link("apps", "/apps", "shell.nav.apps", icons::LAYOUT_GRID);
    push_pane(&mut out, apps, "apps", vec![]);
    if office {
        out.push(link(
            "documents",
            "/office",
            "office.files.title",
            icons::DOC_01,
        ));
    }
    let storage = link(
        "storage",
        "/storage",
        "shell.nav.storage",
        icons::HARD_DRIVE,
    );
    push_pane(&mut out, storage, "storage", vec![]);
    let mesh_children: Vec<NavEntry> = ["market", "machines"]
        .into_iter()
        .filter(|id| me.pane_open(id))
        .map(leaf)
        .collect();
    let mesh = link("mesh", "/mesh", "shell.nav.mesh", icons::SHARE_08);
    push_pane(&mut out, mesh, "mesh", mesh_children);
    if me.can_use("lab") {
        out.push(link("lab", "/diagrams", "shell.nav.lab", icons::FLOWCHART));
    }
    for (key, to, label, glyph) in [
        ("chat", "/chat", "shell.nav.chat", icons::BUBBLE_CHAT),
        ("mail", "/mail", "shell.nav.mail", icons::MAIL_01),
        (
            "contacts",
            "/contacts",
            "shell.nav.contacts",
            icons::CONTACT_BOOK,
        ),
        (
            "calendar",
            "/calendar",
            "shell.nav.calendar",
            icons::CALENDAR_03,
        ),
        ("ci", "/ci", "shell.nav.ci", icons::WORKFLOW_SQUARE_01),
    ] {
        if me.shares(key) {
            out.push(link(key, to, label, glyph));
        }
    }
    let settings: Vec<NavEntry> = [
        "network",
        "look",
        "hardware",
        "security",
        "account",
        "users",
        "moderation",
        "access",
        "credentials",
        "problems",
        "advanced",
        "history",
        "backup",
        "about",
        "reset",
    ]
    .into_iter()
    .filter(|id| me.pane_open(id))
    .map(leaf)
    .collect();
    if !settings.is_empty() {
        out.push(NavEntry {
            key: "settings".to_string(),
            href: None,
            rail_href: Some(shell::href("/settings")),
            label: m.t("shell.nav.settings"),
            glyph: icons::SETTINGS_01,
            current: false,
            children: settings,
        });
    }
    out
}

/// `GET /api/pins` as the sidebar's links (lib/pins.ts's viewPin). An app
/// opens on its own port, which only the script page can read off the
/// address bar, so its pin leads to the Apps pane; an issue shows its key.
#[must_use]
pub fn pins(v: &Value, m: &Messages) -> Vec<NavEntry> {
    let enc = |s: &str| -> String {
        s.bytes()
            .map(|b| {
                if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                    (b as char).to_string()
                } else {
                    format!("%{b:02X}")
                }
            })
            .collect()
    };
    let text = |p: &Value, k: &str| p[k].as_str().unwrap_or_default().to_string();
    v["pins"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let (key, label, glyph, href) = match p["kind"].as_str()? {
                "tile" => {
                    let app = text(p, "app");
                    let (glyph, to) = match app.as_str() {
                        "files" => (icons::FOLDER_01, "/cloud/files".to_string()),
                        "photos" => (icons::IMAGE_02, "/cloud/photos".to_string()),
                        "code" => (icons::GIT_BRANCH, "/git/".to_string()),
                        "issues" => (icons::TASK_DONE_01, "/git/issues".to_string()),
                        "settings" => (icons::SETTINGS_01, shell::href("/settings")),
                        _ => return None,
                    };
                    (
                        format!("tile:{app}"),
                        m.t(&format!("apps.{app}.name")),
                        glyph,
                        to,
                    )
                }
                "app" => {
                    let release = text(p, "release");
                    (
                        format!("app:{release}"),
                        release,
                        icons::PACKAGE,
                        shell::href("/apps"),
                    )
                }
                "file" => {
                    let path = text(p, "path");
                    let trimmed = path.trim_end_matches('/');
                    let name = trimmed
                        .rsplit('/')
                        .next()
                        .filter(|n| !n.is_empty())
                        .unwrap_or(trimmed)
                        .to_string();
                    let id = p["id"].as_u64()?;
                    let glyph = if p["folder"].as_bool() == Some(true) {
                        icons::FOLDER_01
                    } else {
                        icons::FILE_01
                    };
                    (
                        format!("file:{id}"),
                        name,
                        glyph,
                        format!("/cloud/files?fileid={id}"),
                    )
                }
                "repo" => {
                    let (owner, name) = (text(p, "owner"), text(p, "name"));
                    (
                        format!("repo:{owner}/{name}"),
                        name.clone(),
                        icons::GIT_BRANCH,
                        format!("/git/{}/{}", enc(&owner), enc(&name)),
                    )
                }
                kind @ ("issue" | "project") => {
                    let key = text(p, "key");
                    let (glyph, sub) = if kind == "issue" {
                        (icons::TASK_DONE_01, "")
                    } else {
                        (icons::TASK_DAILY_01, "p/")
                    };
                    (
                        format!("{kind}:{key}"),
                        key.clone(),
                        glyph,
                        format!("/git/issues/{sub}{}", enc(&key)),
                    )
                }
                _ => return None,
            };
            Some(NavEntry {
                key,
                href: Some(href),
                label,
                glyph,
                ..NavEntry::default()
            })
        })
        .collect()
}

/// The phone's tabs (app-sidebar.tsx's TABS): the first four that are there.
pub const TABS: &[&str] = &["overview", "chat", "mail", "apps", "storage", "mesh", "lab"];

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
