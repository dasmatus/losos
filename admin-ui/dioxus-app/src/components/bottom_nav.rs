use crate::auth::Me;
use crate::components::sidebar::{bottom_tabs, visible_nav, SidebarTree};
use crate::routes::Route;
use dioxus::prelude::*;
use std::collections::BTreeSet;

fn tab_icon(key: &str) -> &'static str {
    // Simple parity-oriented glyphs (React uses icon components; Dioxus uses
    // lightweight text glyphs where shared icon assets are unavailable).
    match key {
        "overview" => "⌂",
        "cloud" => "☁",
        "chat" => "💬",
        "mail" => "✉",
        "apps" => "▦",
        "storage" => "▣",
        "mesh" => "⎔",
        "diagrams" | "lab" => "◇",
        "ci" => "◎",
        _ => "•",
    }
}

/// Mobile app-chrome: primary bottom tab bar + "More" sheet.
/// Rendered by the shell (not the desktop sidebar) so it persists as stable
/// navigation chrome on narrow widths. Respects `apple-design` guidance:
/// tab-bar-as-navigation, 44px targets, clear labels, sheet as a modal surface.
#[component]
pub fn BottomNav(me: Option<Me>) -> Element {
    let current = use_route::<Route>();
    let mut mobile_open = use_signal(|| false);
    let open_folds = use_signal(BTreeSet::<String>::new);

    let base = visible_nav(&me);
    let tabs = bottom_tabs(&base, &current);

    // Close sheet on route change.
    let current_for_effect = current.clone();
    use_effect(use_reactive((&current_for_effect.clone(),), move |_| {
        mobile_open.set(false);
    }));

    rsx! {
        nav {
            class: "bottom-nav ls-bottom-nav",
            role: "tablist",
            "aria-label": "Primary navigation",
            for tab in tabs.iter() {
                {
                    let active = tab.active;
                    let cls = if active {
                        "bottom-nav-tab active ls-bottom-nav-tab"
                    } else {
                        "bottom-nav-tab ls-bottom-nav-tab"
                    };
                    {
                        let icon = tab_icon(tab.key);
                        if active {
                            rsx! {
                                Link {
                                    to: tab.route.clone(),
                                    class: "{cls}",
                                    role: "tab",
                                    "aria-selected": "true",
                                    "aria-current": "page",
                                    span { class: "bottom-nav-icon", "aria-hidden": "true", "{icon}" }
                                    span { class: "bottom-nav-label", "{tab.label}" }
                                }
                            }
                        } else {
                            rsx! {
                                Link {
                                    to: tab.route.clone(),
                                    class: "{cls}",
                                    role: "tab",
                                    "aria-selected": "false",
                                    span { class: "bottom-nav-icon", "aria-hidden": "true", "{icon}" }
                                    span { class: "bottom-nav-label", "{tab.label}" }
                                }
                            }
                        }
                    }
                }
            }
            button {
                r#type: "button",
                class: "bottom-nav-tab ls-bottom-nav-more",
                role: "tab",
                "aria-haspopup": "dialog",
                "aria-expanded": if *mobile_open.read() { "true" } else { "false" },
                "aria-label": "More sections",
                onclick: move |_| {
                    let v = *mobile_open.read();
                    mobile_open.set(!v);
                },
                span { class: "bottom-nav-icon", "aria-hidden": "true", "⋯" }
                span { class: "bottom-nav-label", "More" }
            }
        }

        if *mobile_open.read() {
            // Modal sheet: backdrop + surface. Rendered in-flow; fixed positioning
            // makes it a viewport overlay. Clicking backdrop closes.
            div {
                class: "bottom-nav-backdrop ls-bottom-nav-backdrop",
                onclick: move |_| mobile_open.set(false),
            }
            section {
                class: "bottom-nav-sheet ls-bottom-nav-sheet",
                role: "dialog",
                "aria-modal": "true",
                "aria-label": "All sections",
                div { class: "bottom-nav-sheet-grabber", "aria-hidden": "true" }
                div { class: "bottom-nav-sheet-header",
                    h2 { class: "bottom-nav-sheet-title", "Sections" }
                    button {
                        r#type: "button",
                        class: "btn btn-ghost bottom-nav-sheet-close",
                        "aria-label": "Close sections",
                        onclick: move |_| mobile_open.set(false),
                        "Done"
                    }
                }
                div { class: "bottom-nav-sheet-body",
                    SidebarTree { base: base.clone(), current: current.clone(), open_folds, nested: false }
                }
            }
        }
    }
}


