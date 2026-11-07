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
        NavItem { label: "Mesh", route: Route::Mesh {}, section: "Main" },
        NavItem { label: "Market", route: Route::Market {}, section: "Mesh" },
        NavItem { label: "Machines", route: Route::Machines {}, section: "Machines" },
        NavItem { label: "Diagrams", route: Route::Diagrams {}, section: "Tools" },
        NavItem { label: "Wikis", route: Route::Wikis {}, section: "Tools" },
        NavItem { label: "Settings", route: Route::SettingsRoot {}, section: "Settings" },
        NavItem { label: "About", route: Route::About {}, section: "Settings" },
    ]
}

/// Top bar aligned with the React AppBar's visual structure.
#[component]
pub fn AppBar() -> Element {
    rsx! {
        header { class: "app-bar",
            div { class: "app-bar-inner",
                a { href: "/", class: "app-bar-home", "LosOS" }
                span { class: "app-bar-badge", "this box" }
                div { class: "app-bar-spacer" }
                span { class: "app-bar-person muted", "Admin UI (Dioxus)" }
            }
        }
    }
}

#[component]
pub fn Sidebar() -> Element {
    let current = use_route::<Route>();
    let items = nav_items();
    let sections = ["Main", "Mesh", "Machines", "Tools", "Settings"];

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
                                        let active = &current == &item.route
                                            || (matches!(current, Route::SettingsRoot {} | Route::SettingsPane { .. }) && item.label == "Settings")
                                            || (matches!(current, Route::Mesh {} | Route::Market {}) && item.label == "Mesh");
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
