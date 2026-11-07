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
        NavItem { label: "Machines", route: Route::Machines {}, section: "Mesh" },
        NavItem { label: "Diagrams", route: Route::Diagrams {}, section: "Tools" },
        NavItem { label: "Wikis", route: Route::Wikis {}, section: "Tools" },
        NavItem { label: "Settings", route: Route::SettingsRoot {}, section: "Settings" },
        NavItem { label: "About", route: Route::About {}, section: "Settings" },
    ]
}

#[component]
pub fn Sidebar() -> Element {
    let current = use_route::<Route>();
    let items = nav_items();
    let sections = ["Main", "Mesh", "Tools", "Settings"];

    rsx! {
        aside { class: "sidebar",
            div { class: "sidebar-header",
                div { class: "brand", "LosOS" }
                div { class: "brand-sub muted", "Admin" }
            }
            nav { class: "sidebar-nav",
                for section in sections {
                    div { class: "nav-section",
                        div { class: "nav-section-label", "{section}" }
                        for item in items.iter().filter(|i| i.section == section) {
                            {
                                let active = &current == &item.route
                                    || (section == "Settings" && matches!(current, Route::SettingsRoot {} | Route::SettingsPane { .. }) && item.label == "Settings");
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
