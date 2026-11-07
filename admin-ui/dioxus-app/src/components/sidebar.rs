use crate::auth::Me;
use crate::routes::{pane_label, Route};
use dioxus::prelude::*;
use std::collections::BTreeSet;

#[derive(Clone, PartialEq)]
pub(crate) enum EntryKind {
    Link,
    Fold,
}

#[derive(Clone, PartialEq)]
pub(crate) struct Entry {
    kind: EntryKind,
    key: &'static str,
    label: &'static str,
    route: Option<Route>,
    href_hint: &'static str,
    pane: Option<&'static str>,
    shared: Option<&'static str>,
    also_prefixes: &'static [&'static str],
    children: Vec<Entry>,
}

fn link(
    key: &'static str,
    label: &'static str,
    route: Route,
    href_hint: &'static str,
) -> Entry {
    Entry {
        kind: EntryKind::Link,
        key,
        label,
        route: Some(route),
        href_hint,
        pane: None,
        shared: None,
        also_prefixes: &[],
        children: vec![],
    }
}

fn pane_link(pane: &'static str) -> Entry {
    Entry {
        kind: EntryKind::Link,
        key: pane,
        label: pane_label(pane),
        route: Some(Route::SettingsPane {
            pane: pane.to_string(),
        }),
        href_hint: "",
        pane: Some(pane),
        shared: None,
        also_prefixes: &[],
        children: vec![],
    }
}

fn tree() -> Vec<Entry> {
    vec![
        link("overview", "Overview", Route::Home {}, "/"),
        Entry {
            kind: EntryKind::Link,
            key: "apps",
            label: "Apps",
            route: Some(Route::Apps {}),
            href_hint: "/apps",
            pane: Some("apps"),
            shared: None,
            also_prefixes: &["/apps/"],
            children: vec![],
        },
        Entry {
            kind: EntryKind::Link,
            key: "office",
            label: "Documents",
            route: Some(Route::Office {}),
            href_hint: "/office",
            pane: None,
            shared: None,
            also_prefixes: &["/office/"],
            children: vec![],
        },
        Entry {
            kind: EntryKind::Link,
            key: "cloud",
            label: "Cloud",
            route: Some(Route::Cloud {}),
            href_hint: "/cloud",
            pane: None,
            shared: Some("cloud"),
            also_prefixes: &["/cloud/"],
            children: vec![
                placeholder_child("cloud-notes", "Notes", "/cloud/notes", Some("cloud")),
                placeholder_child("cloud-recent", "Recent", "/cloud/recent", Some("cloud")),
                placeholder_child("cloud-favorites", "Favorites", "/cloud/favorites", Some("cloud")),
                placeholder_child("cloud-shared", "Shared with you", "/cloud/shared", Some("cloud")),
                placeholder_child("cloud-photos", "Photos", "/cloud/photos", Some("cloud")),
                placeholder_child("cloud-trash", "Trash", "/cloud/trash", Some("cloud")),
                placeholder_child("cloud-devices", "Devices", "/cloud/devices", Some("cloud")),
            ],
        },
        Entry {
            kind: EntryKind::Link,
            key: "storage",
            label: "Storage",
            route: Some(Route::Storage {}),
            href_hint: "/storage",
            pane: Some("storage"),
            shared: None,
            also_prefixes: &[],
            children: vec![],
        },
        Entry {
            kind: EntryKind::Link,
            key: "mesh",
            label: "Mesh",
            route: Some(Route::Mesh {}),
            href_hint: "/mesh",
            pane: Some("mesh"),
            shared: None,
            also_prefixes: &["/mesh/"],
            children: vec![pane_link("market"), pane_link("machines")],
        },
        Entry {
            kind: EntryKind::Link,
            key: "diagrams",
            label: "Diagrams",
            route: Some(Route::Diagrams {}),
            href_hint: "/diagrams",
            pane: None,
            shared: None,
            also_prefixes: &["/diagrams/"],
            children: vec![],
        },
        Entry {
            kind: EntryKind::Link,
            key: "wikis",
            label: "Wikis",
            route: Some(Route::Wikis {}),
            href_hint: "/wikis",
            pane: None,
            shared: None,
            also_prefixes: &["/wikis/"],
            children: vec![],
        },
        Entry {
            kind: EntryKind::Link,
            key: "chat",
            label: "Chat",
            route: Some(Route::Chat {}),
            href_hint: "/chat",
            pane: None,
            shared: Some("chat"),
            also_prefixes: &["/chat/"],
            children: vec![],
        },
        Entry {
            kind: EntryKind::Link,
            key: "mail",
            label: "Mail",
            route: Some(Route::Mail {}),
            href_hint: "/mail",
            pane: None,
            shared: Some("mail"),
            also_prefixes: &[],
            children: vec![],
        },
        Entry {
            kind: EntryKind::Link,
            key: "contacts",
            label: "Contacts",
            route: Some(Route::Contacts {}),
            href_hint: "/contacts",
            pane: None,
            shared: Some("contacts"),
            also_prefixes: &[],
            children: vec![],
        },
        Entry {
            kind: EntryKind::Link,
            key: "calendar",
            label: "Calendar",
            route: Some(Route::Calendar {}),
            href_hint: "/calendar",
            pane: None,
            shared: Some("calendar"),
            also_prefixes: &[],
            children: vec![],
        },
        Entry {
            kind: EntryKind::Link,
            key: "ci",
            label: "Mesh CI",
            route: Some(Route::Ci {}),
            href_hint: "/ci",
            pane: None,
            shared: Some("ci"),
            also_prefixes: &["/ci/"],
            children: vec![],
        },
        Entry {
            kind: EntryKind::Fold,
            key: "settings",
            label: "Settings",
            route: Some(Route::SettingsRoot {}),
            href_hint: "/settings",
            pane: None,
            shared: None,
            also_prefixes: &[],
            children: vec![
                pane_link("network"),
                pane_link("look"),
                pane_link("hardware"),
                pane_link("statistics"),
                pane_link("security"),
                pane_link("account"),
                pane_link("users"),
                pane_link("moderation"),
                pane_link("access"),
                pane_link("credentials"),
                pane_link("problems"),
                pane_link("advanced"),
                pane_link("history"),
                pane_link("logs"),
                pane_link("backup"),
                pane_link("about"),
                pane_link("reset"),
            ],
        },
    ]
}

fn placeholder_child(
    key: &'static str,
    label: &'static str,
    href_hint: &'static str,
    shared: Option<&'static str>,
) -> Entry {
    Entry {
        kind: EntryKind::Link,
        key,
        label,
        route: Some(Route::PlaceholderPath {
            segments: href_hint
                .trim_start_matches('/')
                .split('/')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect(),
        }),
        href_hint,
        pane: None,
        shared,
        also_prefixes: &[],
        children: vec![],
    }
}

pub(crate) fn shared_ok(me: &Option<Me>, shared: Option<&str>) -> bool {
    let Some(name) = shared else {
        return true;
    };
    match me {
        Some(m) => match &m.shared {
            Some(list) => list.iter().any(|s| s == name),
            // Unknown shared state in this client: show for parity.
            None => true,
        },
        // Signed-in shell should already have me in normal flow; keep permissive.
        None => true,
    }
}

pub(crate) fn visible(entries: Vec<Entry>, me: &Option<Me>) -> Vec<Entry> {
    let mut out = Vec::new();
    for entry in entries {
        let children = visible(entry.children, me);
        if entry.kind == EntryKind::Fold {
            if !children.is_empty() {
                out.push(Entry { children, ..entry });
            }
            continue;
        }
        if shared_ok(me, entry.shared) {
            out.push(Entry { children, ..entry });
        } else {
            out.extend(children);
        }
    }
    out
}

/// Navigation entries for the current user (held users see account only).
pub(crate) fn visible_nav(me: &Option<Me>) -> Vec<Entry> {
    let is_held = me.as_ref().map(|m| m.is_held()).unwrap_or(false);
    if is_held {
        vec![pane_link("account")]
    } else {
        visible(tree(), me)
    }
}

fn route_path(route: &Route) -> String {
    // Route implements Routable/Display-like formatting via router in practice;
    // use debug fallback patterns for active matching helpers.
    format!("{route:?}")
}

pub(crate) fn is_current(entry: &Entry, current: &Route) -> bool {
    if let Some(r) = &entry.route {
        if r == current {
            return true;
        }
    }
    // Prefix/pane parity cases.
    let hint = entry.href_hint;
    if !hint.is_empty() {
        let cp = route_path(current);
        // Rough but effective for placeholder/catch-all matching.
        if cp.contains(&format!("segments: [\"{}\"", hint.trim_start_matches('/').split('/').next().unwrap_or("")))
            && hint.starts_with("/cloud")
            && entry.key.starts_with("cloud")
        {
            // handled by contains_current/prefix below
        }
    }
    match (entry.key, current) {
        ("office", Route::Office {} | Route::OfficePath { .. }) => true,
        ("ci", Route::Ci {} | Route::CiRun { .. }) => true,
        ("chat", Route::Chat {} | Route::ChatPath { .. }) => true,
        ("diagrams", Route::Diagrams {} | Route::DiagramsPath { .. }) => true,
        ("wikis", Route::Wikis {} | Route::WikisPath { .. }) => true,
        ("apps", Route::Apps {} | Route::AppNew {} | Route::AppScreen { .. } | Route::AppEdit { .. }) => true,
        ("settings", Route::SettingsRoot {} | Route::SettingsPane { .. } | Route::About {} | Route::Storage {} | Route::Mesh {} | Route::Market {} | Route::Machines {} | Route::MeshMachines {}) => false,
        _ => false,
    }
}

pub(crate) fn contains_current(entry: &Entry, current: &Route) -> bool {
    entry.children.iter().any(|c| is_current(c, current) || contains_current(c, current))
}

pub(crate) fn route_for_entry(entry: &Entry) -> Route {
    if let Some(pane) = entry.pane {
        // Top-level pane routes where React uses them.
        return match pane {
            "storage" => Route::Storage {},
            "mesh" => Route::Mesh {},
            "machines" => Route::Machines {},
            "apps" => Route::Apps {},
            "market" => Route::Market {},
            _ => Route::SettingsPane {
                pane: pane.to_string(),
            },
        };
    }
    entry.route.clone().unwrap_or(Route::Home {})
}

/// Desktop sidebar navigation. Mobile primary navigation is owned by
/// [`crate::components::bottom_nav::BottomNav`] (rendered by the shell) so the
/// tab bar/sheet remain stable app chrome on narrow widths.
#[component]
pub fn Sidebar(me: Option<Me>) -> Element {
    let current = use_route::<Route>();
    let mut open_folds = use_signal(BTreeSet::<String>::new);
    let base = visible_nav(&me);

    // Auto-open folds/links with children when current is inside.
    let base_for_effect = base.clone();
    let current_for_effect = current.clone();
    use_effect(use_reactive((&current_for_effect.clone(),), move |_| {
        let mut next = open_folds();
        for entry in &base_for_effect {
            let here = is_current(entry, &current_for_effect);
            if here || contains_current(entry, &current_for_effect) {
                next.insert(entry.key.to_string());
            }
        }
        open_folds.set(next);
    }));

    rsx! {
        aside { class: "sidebar ls-sidebar",
            nav { class: "sidebar-nav ls-sidebar-nav", "aria-label": "Navigation",
                SidebarTree { base, current, open_folds, nested: false }
            }
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct BottomTab {
    pub(crate) label: &'static str,
    pub(crate) route: Route,
    pub(crate) active: bool,
}

pub(crate) fn bottom_tabs(base: &[Entry], current: &Route) -> Vec<BottomTab> {
    // React parity priority (app-sidebar TABS): overview, cloud, chat, mail,
    // apps, storage, mesh, lab — first 4 visible. Dioxus maps "lab" to diagrams.
    let order = ["overview", "cloud", "chat", "mail", "apps", "storage", "mesh", "diagrams", "lab"];
    let mut out = Vec::new();
    for key in order {
        if out.len() >= 4 {
            break;
        }
        if let Some(entry) = base.iter().find(|e| e.key == key && e.kind == EntryKind::Link) {
            out.push(BottomTab {
                label: entry.label,
                route: route_for_entry(entry),
                active: is_current(entry, current) || contains_current(entry, current),
            });
        }
    }
    out
}

#[component]
pub(crate) fn SidebarTree(
    base: Vec<Entry>,
    current: Route,
    open_folds: Signal<BTreeSet<String>>,
    nested: bool,
) -> Element {
    if nested {
        return rsx! {
            ul { class: "nav-sub ls-nav-sub",
                for entry in base.iter().cloned() {
                    { render_entry(entry, current.clone(), open_folds, true) }
                }
            }
        };
    }
    rsx! {
        div { class: "nav-sections ls-nav-sections",
            for entry in base.iter().cloned() {
                { render_entry(entry, current.clone(), open_folds, false) }
            }
        }
    }
}

fn render_entry(
    entry: Entry,
    current: Route,
    mut open_folds: Signal<BTreeSet<String>>,
    nested: bool,
) -> Element {
    let active = is_current(&entry, &current);
    let has_children = !entry.children.is_empty();
    let contains = contains_current(&entry, &current);
    let is_open = open_folds().contains(entry.key);
    let cls = if active {
        "nav-item active ls-nav-item"
    } else {
        "nav-item ls-nav-item"
    };

    if entry.kind == EntryKind::Fold || (has_children && !nested) {
        // Fold or link-with-children: button/chevron + optional subentries.
        let key = entry.key.to_string();
        let route = route_for_entry(&entry);
        let row_cls = if contains { "nav-fold open ls-nav-fold" } else { "nav-fold ls-nav-fold" };
        let toggle_label = if is_open {
            format!("Hide {} sections", entry.label)
        } else {
            format!("Show {} sections", entry.label)
        };
        return rsx! {
            div { class: "{row_cls}",
                div { class: "nav-fold-row nav-row",
                    if active {
                        Link { to: route.clone(), class: "{cls}", "aria-current": "page", "{entry.label}" }
                    } else {
                        Link { to: route, class: "{cls}", "{entry.label}" }
                    }
                    if has_children {
                        button {
                            r#type: "button",
                            class: "nav-fold-toggle ls-nav-fold-toggle",
                            "aria-expanded": if is_open { "true" } else { "false" },
                            "aria-label": "{toggle_label}",
                            onclick: move |_| {
                                let mut next = open_folds();
                                if next.contains(&key) { next.remove(&key); } else { next.insert(key.clone()); }
                                open_folds.set(next);
                            },
                            if is_open { "▾" } else { "▸" }
                        }
                    }
                }
                if has_children && (is_open || contains) {
                    SidebarTree { base: entry.children, current: current.clone(), open_folds, nested: true }
                }
            }
        };
    }

    let route = route_for_entry(&entry);
    rsx! {
        div { class: "nav-row ls-nav-row",
            if active {
                Link { to: route, class: "{cls}", "aria-current": "page", "{entry.label}" }
            } else {
                Link { to: route, class: "{cls}", "{entry.label}" }
            }
        }
    }
}
