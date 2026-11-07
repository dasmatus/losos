use dioxus::prelude::*;

/// Basic button component preserving the design system.
#[component]
pub fn Button(
    #[props(default)] class: String,
    #[props(default)] disabled: bool,
    #[props(default)] kind: String, // primary, secondary, danger, ghost
    #[props(default = "button".to_string())] button_type: String,
    onclick: Option<EventHandler<MouseEvent>>,
    children: Element,
) -> Element {
    let kind_cls = if kind.is_empty() {
        "btn-secondary".to_string()
    } else {
        format!("btn-{kind}")
    };
    let cls = if class.is_empty() {
        format!("btn {kind_cls}")
    } else {
        format!("btn {kind_cls} {class}")
    };
    rsx! {
        button { r#type: "{button_type}", class: "{cls}", disabled: disabled, onclick: move |e| if let Some(h) = &onclick { h.call(e) }, {children} }
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
