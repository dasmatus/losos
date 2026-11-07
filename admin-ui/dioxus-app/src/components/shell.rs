use crate::auth::Me;
use crate::components::{app_bar::AppBar, sidebar::Sidebar};
use crate::routes::{is_wide_route, Route};
use dioxus::prelude::*;

/// Shell column/body parity with React `ShellBody`.
/// `wide` gives main the full width (diagrams/calendar/wikis), `fill` for editors.
#[component]
pub fn ShellFrame(
    me: Option<Me>,
    children: Element,
    #[props(default = false)] wide: bool,
    #[props(default = false)] fill: bool,
) -> Element {
    let main_class = if wide || fill {
        "app-main ls-main ls-main-wide"
    } else {
        "app-main ls-main"
    };
    let body_class = if fill {
        "shell-body ls-shell-body ls-shell-body-fill"
    } else {
        "shell-body ls-shell-body"
    };
    rsx! {
        div { class: "shell-root ls-shell",
            AppBar { me: me.clone(), width: "wide".to_string() }
            div { class: "{body_class}",
                Sidebar { me }
                main { class: "{main_class}", {children} }
            }
        }
    }
}

/// Convenience: compute wide from current route and render the standard frame.
#[component]
pub fn RoutedShell(me: Option<Me>) -> Element {
    let route = use_route::<Route>();
    let wide = is_wide_route(&route);
    rsx! {
        ShellFrame { me, wide,
            Outlet::<Route> {}
        }
    }
}
