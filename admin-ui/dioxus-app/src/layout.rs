use crate::app::{AuthSignal, ConfigSignal};
use crate::auth::{self, Me};
use crate::routes::Route;
use dioxus::prelude::*;

#[derive(Clone, PartialEq)]
struct NavItem {
    label: &'static str,
    route: Route,
    section: &'static str,
}

fn nav_items() -> Vec<NavItem> {
    vec![
        NavItem { label: "Overview", route: Route::Home {}, section: "Main" },
        NavItem { label: "Apps", route: Route::Apps {}, section: "Main" },
        NavItem { label: "Storage", route: Route::Storage {}, section: "Main" },
        NavItem { label: "Mesh", route: Route::Mesh {}, section: "Mesh" },
        NavItem { label: "Market", route: Route::Market {}, section: "Mesh" },
        NavItem { label: "Machines", route: Route::Machines {}, section: "Mesh" },
        NavItem { label: "Diagrams", route: Route::Diagrams {}, section: "Tools" },
        NavItem { label: "Wikis", route: Route::Wikis {}, section: "Tools" },
        NavItem { label: "Settings", route: Route::SettingsRoot {}, section: "Settings" },
        NavItem { label: "About", route: Route::About {}, section: "Settings" },
    ]
}

/// Top bar aligned with the React AppBar's visual structure.
#[component]
pub fn AppBar(me: Option<Me>) -> Element {
    let mut config: ConfigSignal = use_context();
    let mut auth: AuthSignal = use_context();
    let person = me
        .as_ref()
        .map(|m| m.label())
        .unwrap_or_else(|| "Signed in".to_string());

    let sign_out = move |_| {
        let mut next = config();
        auth::drop_token(&mut next);
        config.set(next);
        auth.set(Default::default());
    };

    rsx! {
        header { class: "app-bar",
            div { class: "app-bar-inner",
                Link { to: Route::Home {}, class: "app-bar-home", "LosOS" }
                span { class: "app-bar-badge", "this box" }
                div { class: "app-bar-spacer" }
                span { class: "app-bar-person muted", "{person}" }
                button { class: "btn btn-ghost", style: "margin-left: 8px; padding: 4px 8px; font-size: 13px;", onclick: sign_out, "Sign out" }
            }
        }
    }
}

fn is_route_current(current: &Route, item: &Route) -> bool {
    if current == item {
        return true;
    }
    matches!(
        (current, item),
        (Route::SettingsRoot {} | Route::SettingsPane { .. }, Route::SettingsRoot {})
            | (Route::Mesh {} | Route::Market {} | Route::Machines {} | Route::MeshMachines {}, Route::Mesh {})
            | (Route::Machines {} | Route::MeshMachines {}, Route::Machines {})
    )
}

#[component]
pub fn Sidebar() -> Element {
    let current = use_route::<Route>();
    let items = nav_items();
    let sections = ["Main", "Mesh", "Tools", "Settings"];

    rsx! {
        aside { class: "sidebar",
            nav { class: "sidebar-nav",
                for section in sections {
                    {
                        let section_items: Vec<_> = items.iter().filter(|i| i.section == section).cloned().collect();
                        if section_items.is_empty() { return rsx!{}; }
                        rsx! {
                            div { class: "nav-section",
                                div { class: "nav-section-label", "{section}" }
                                for item in section_items {
                                    {
                                        let active = is_route_current(&current, &item.route);
                                        let cls = if active { "nav-item active" } else { "nav-item" };
                                        rsx! {
                                            Link { to: item.route.clone(), class: "{cls}", "{item.label}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
