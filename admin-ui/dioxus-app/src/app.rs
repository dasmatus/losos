use crate::screens::{connect::ConnectScreen, dashboard::DashboardScreen};
use crate::store::{self, BoxConfig};
use dioxus::prelude::*;

const MAIN_CSS: Asset = asset!("/assets/main.css");

#[component]
pub fn App() -> Element {
    let mut config = use_signal(store::load);

    // Persist whenever config changes.
    use_effect(move || {
        store::save(&config.read());
    });

    let needs = config.read().needs_connect();

    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        div { class: "app-shell",
            if needs {
                ConnectScreen {
                    config: config(),
                    on_change: move |next: BoxConfig| config.set(next),
                }
            } else {
                DashboardScreen {
                    config: config(),
                    on_change: move |next: BoxConfig| config.set(next),
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
