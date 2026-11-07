use crate::app::{AuthSignal, ConfigSignal};
use crate::auth::{self, Me};
use crate::routes::Route;
use dioxus::prelude::*;

/// Top bar aligned with the React AppBar's visual structure.
/// `width` controls the inner column: page/wide/full parity hook.
#[component]
pub fn AppBar(me: Option<Me>, #[props(default = "wide".to_string())] width: String) -> Element {
    let mut config: ConfigSignal = use_context();
    let mut auth: AuthSignal = use_context();
    let person = me
        .as_ref()
        .map(|m| m.label())
        .unwrap_or_else(|| "Signed in".to_string());
    let inner_class = match width.as_str() {
        "page" => "app-bar-inner app-bar-inner-page",
        "full" => "app-bar-inner app-bar-inner-full",
        _ => "app-bar-inner", // wide/default
    };

    let sign_out = move |_| {
        let mut next = config();
        auth::drop_token(&mut next);
        config.set(next);
        auth.set(Default::default());
    };

    rsx! {
        header { class: "app-bar ls-app-bar",
            div { class: "{inner_class}",
                Link { to: Route::Home {}, class: "app-bar-home ls-app-bar-home", "LosOS" }
                span { class: "app-bar-badge ls-app-bar-badge", "this box" }
                div { class: "app-bar-spacer" }
                span { class: "app-bar-person muted ls-app-bar-person", "{person}" }
                button {
                    r#type: "button",
                    class: "btn btn-ghost ls-app-bar-signout",
                    style: "margin-left: 8px; font-size: 13px;",
                    onclick: sign_out,
                    "Sign out"
                }
            }
        }
    }
}
