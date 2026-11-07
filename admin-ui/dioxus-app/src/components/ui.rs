use dioxus::prelude::*;

/// Basic button component preserving the design system.
#[component]
pub fn Button(
    #[props(default)] class: String,
    #[props(default)] disabled: bool,
    #[props(default)] kind: String, // primary, secondary, danger, ghost
    onclick: Option<EventHandler<MouseEvent>>,
    children: Element,
) -> Element {
    let cls = if class.is_empty() {
        format!("btn btn-{kind}")
    } else {
        format!("btn btn-{kind} {class}")
    };
    rsx! {
        button { class: "{cls}", disabled: disabled, onclick: move |e| if let Some(h) = &onclick { h.call(e) }, {children} }
    }
}

#[component]
pub fn Card(children: Element, #[props(default)] class: String) -> Element {
    rsx! { div { class: "card {class}", {children} } }
}

#[component]
pub fn Spinner() -> Element {
    rsx! { div { class: "spinner", role: "status", "Loading…" } }
}

#[component]
pub fn Empty(title: String, #[props(default)] description: String) -> Element {
    rsx! {
        div { class: "empty",
            h3 { "{title}" }
            if !description.is_empty() { p { class: "muted", "{description}" } }
        }
    }
}
