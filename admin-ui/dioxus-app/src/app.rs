use crate::routes::Route;
use crate::store::{self, BoxConfig};
use dioxus::prelude::*;

const MAIN_CSS: Asset = asset!("/assets/main.css");

/// Shared application config signal type.
pub type ConfigSignal = Signal<BoxConfig>;

#[component]
pub fn App() -> Element {
    let config = use_context_provider(|| Signal::new(store::load()));

    // Persist whenever config changes.
    use_effect(move || {
        store::save(&config.read());
    });

    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        Router::<Route> {}
    }
}

#[component]
pub fn AppLayout() -> Element {
    let mut config: ConfigSignal = use_context();
    let needs = config.read().needs_connect();

    if needs {
        let current = config();
        return rsx! {
            div { class: "app-shell",
                crate::screens::connect::ConnectScreen {
                    config: current,
                    on_change: move |next: BoxConfig| config.set(next),
                }
            }
        };
    }

    rsx! {
        div { class: "shell-root",
            crate::layout::AppBar {}
            div { class: "shell-body",
                crate::layout::Sidebar {}
                main { class: "app-main",
                    Outlet::<Route> {}
                }
            }
        }
    }
}

/// Common result state for pane data fetches.
#[derive(Clone, PartialEq)]
pub enum Load<T: Clone + PartialEq + 'static> {
    Idle,
    Loading,
    Ready(T),
    Err(String),
    Unauthorized,
}
