//! The admin UI's primitives (admin-ui/app/src/components/ui/ and
//! screens/settings/rows.tsx) as markup with the same classes, so the
//! script page's stylesheet draws them the same. Where a primitive moves on
//! its own in the script page (a switch, a fold, a menu), the version here
//! does it with a form control and CSS (`basic.css` beside index.css).
//!
//! Keep the class strings in step with the components they copy: they are
//! what makes the two pages look alike.

use crate::html::{el, El, Html};

/// A Hugeicon the way @hugeicons/react draws it: `inner` is one of
/// [`crate::icons`], `size` its width and height in pixels.
#[must_use]
pub fn icon(inner: &str, size: u32, class: &str) -> Html {
    let class = if class.is_empty() {
        String::new()
    } else {
        format!(" class=\"{}\"", crate::html::escape(class))
    };
    Html::trusted(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{size}\" height=\"{size}\" viewBox=\"0 0 24 24\" fill=\"none\" color=\"currentColor\"{class} stroke-width=\"1.5\" stroke=\"currentColor\" aria-hidden=\"true\">{inner}</svg>"
    ))
}

// ── Button ──────────────────────────────────────────────────────────────

const BUTTON_BASE: &str = "inline-flex shrink-0 items-center justify-center gap-2 rounded-control font-medium whitespace-nowrap select-none transition-[background-color,border-color,color,box-shadow,opacity,scale] duration-150 ease-snap active:scale-[0.97] disabled:pointer-events-none disabled:opacity-45 [&_svg]:pointer-events-none [&_svg]:shrink-0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Primary,
    Secondary,
    Ghost,
    Outline,
    Destructive,
    Link,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Xs,
    Sm,
    Md,
    Lg,
    Icon,
    IconXs,
    IconSm,
}

/// The classes of components/ui/button.tsx's `buttonVariants`.
#[must_use]
pub fn button_classes(variant: Variant, size: Size) -> String {
    let v = match variant {
        Variant::Primary => "bg-accent text-surface hover:opacity-90",
        Variant::Secondary => "border border-line bg-surface text-ink hover:bg-sunk",
        Variant::Ghost => "text-muted hover:bg-sunk hover:text-ink",
        Variant::Outline => "border border-accent bg-transparent text-accent hover:bg-accent-wash",
        Variant::Destructive => "bg-crit text-surface hover:opacity-90",
        Variant::Link => "text-accent underline-offset-4 hover:underline active:scale-100",
    };
    let s = match size {
        Size::Xs => "h-6 gap-1 px-2 text-[12px]",
        Size::Sm => "h-8 px-3 text-[13px]",
        Size::Md => "text-sm h-9 px-4",
        Size::Lg => "h-11 px-5 text-base",
        Size::Icon => "text-sm size-9 p-0",
        Size::IconXs => "text-sm size-6 p-0",
        Size::IconSm => "text-sm size-8 p-0",
    };
    format!("{BUTTON_BASE} {v} {s}")
}

/// A `<button>` with the button look. `type` is `submit` unless changed.
#[must_use]
pub fn button(variant: Variant, size: Size) -> El {
    el("button")
        .attr("type", "submit")
        .attr("data-slot", "button")
        .class(&button_classes(variant, size))
}

/// An `<a>` with the button look.
#[must_use]
pub fn link_button(href: &str, variant: Variant, size: Size) -> El {
    el("a")
        .attr("href", href)
        .attr("data-slot", "button")
        .class(&button_classes(variant, size))
}

// ── Badge and dot ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Default,
    Outline,
    Local,
    Mesh,
    Ok,
    Warn,
    Crit,
}

#[must_use]
pub fn badge(tone: Tone, text: &str) -> Html {
    let t = match tone {
        Tone::Neutral => "bg-sunk text-muted",
        Tone::Default => "bg-accent text-surface",
        Tone::Outline => "border border-line text-muted",
        Tone::Local => "bg-accent-wash text-accent",
        Tone::Mesh => "hatched text-accent",
        Tone::Ok => "bg-ok/12 text-ok",
        Tone::Warn => "bg-warn/12 text-warn",
        Tone::Crit => "bg-crit/12 text-crit",
    };
    el("span")
        .attr("data-slot", "badge")
        .class("inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-[12px] leading-5 font-medium whitespace-nowrap [&_svg]:pointer-events-none [&_svg]:shrink-0")
        .class(t)
        .text(text)
        .into()
}

/// components/ui/badge.tsx's StatusDot.
#[must_use]
pub fn dot(state: &str) -> Html {
    let c = match state {
        "ok" => "bg-ok",
        "warn" => "bg-warn",
        "crit" => "bg-crit",
        "pending" => "bg-faint animate-breathe",
        _ => "bg-faint",
    };
    el("span")
        .attr("aria-hidden", "true")
        .class("inline-block size-2 shrink-0 rounded-full")
        .class(c)
        .into()
}

// ── Alert ───────────────────────────────────────────────────────────────

/// components/ui/alert.tsx: `tone` is default, ok, warn or crit.
#[must_use]
pub fn alert(tone: &str, glyph: Option<&str>, title: Option<&str>, body: impl Into<Html>) -> El {
    let t = match tone {
        "ok" => "border-ok/25 bg-ok/10 text-ink [&>svg]:text-ok",
        "warn" => "border-warn/25 bg-warn/10 text-ink [&>svg]:text-warn",
        "crit" => "border-crit/25 bg-crit/10 text-ink [&>svg]:text-crit",
        _ => "border-line bg-sunk text-ink [&>svg]:text-accent",
    };
    el("div")
        .attr("data-slot", "alert")
        .attr("data-variant", if tone.is_empty() { "default" } else { tone })
        .attr("role", if tone == "crit" { "alert" } else { "status" })
        .class("relative grid w-full grid-cols-[0_1fr] items-start gap-y-0.5 rounded-control border px-3.5 py-3 text-[13px] leading-snug has-[>svg]:grid-cols-[19px_1fr] has-[>svg]:gap-x-3 [&>svg]:mt-px [&>svg]:size-[19px] [&>svg]:shrink-0 animate-fade-in")
        .class(t)
        .child(glyph.map(|g| icon(g, 19, "")))
        .child(title.map(|t| {
            Html::from(
                el("p")
                    .attr("data-slot", "alert-title")
                    .class("col-start-2 min-h-4 font-medium tracking-tight text-ink")
                    .text(t),
            )
        }))
        .child(
            el("div")
                .attr("data-slot", "alert-description")
                .class("col-start-2 grid min-w-0 gap-1 text-[13px] text-muted [&_p]:leading-snug [&>*]:min-w-0")
                .child(body),
        )
}

// ── Card ────────────────────────────────────────────────────────────────

#[must_use]
pub fn card() -> El {
    el("div")
        .attr("data-slot", "card")
        .class("rounded-card border border-line bg-surface text-ink shadow-card animate-rise")
}

#[must_use]
pub fn card_content() -> El {
    el("div")
        .attr("data-slot", "card-content")
        .class("px-5 pb-5")
}

#[must_use]
pub fn card_footer() -> El {
    el("div")
        .attr("data-slot", "card-footer")
        .class("flex items-center gap-2 border-t border-hair px-5 py-3.5")
}

// ── Settings rows (screens/settings/rows.tsx) ───────────────────────────

#[must_use]
pub fn pane_header(title: &str, summary: &str) -> Html {
    el("header")
        .class("mb-4")
        .child(
            el("h1")
                .class("text-[19px] leading-tight font-semibold tracking-tight")
                .text(title),
        )
        .child(
            el("p")
                .class("mt-1 max-w-prose text-[13px] leading-snug text-muted")
                .text(summary),
        )
        .into()
}

#[must_use]
pub fn pane_section() -> El {
    el("section").class("mb-6 last:mb-0")
}

#[must_use]
pub fn group_title(text: &str) -> Html {
    el("h2")
        .class("px-1.5 pb-1.5 text-[12.5px] font-medium text-muted")
        .text(text)
        .into()
}

#[must_use]
pub fn group() -> El {
    el("div")
        .class("overflow-hidden rounded-card border border-line bg-surface text-ink shadow-card")
}

#[must_use]
pub fn group_caption(text: impl Into<Html>) -> Html {
    el("p")
        .class("px-1.5 pt-2 text-[12.5px] leading-snug text-muted")
        .child(text)
        .into()
}

const ROW_RULE: &str = "after:pointer-events-none after:absolute after:right-0 after:bottom-0 after:left-3.5 after:h-px after:bg-hair after:content-[''] last:after:hidden";

#[must_use]
pub fn row() -> El {
    el("div")
        .class("relative flex min-h-[42px] items-center justify-between gap-4 px-3.5 py-2")
        .class(ROW_RULE)
}

#[must_use]
pub fn stack_row() -> El {
    el("div")
        .class("relative flex flex-col gap-2.5 px-3.5 py-3")
        .class(ROW_RULE)
}

/// The left side of a row: its title, a `<label>` when `for_id` is given,
/// and a detail line under it.
#[must_use]
pub fn row_text(title: &str, detail: Option<&str>, for_id: Option<&str>) -> Html {
    let title_el = match for_id {
        Some(id) => el("label")
            .attr("for", id)
            .class("text-sm leading-snug text-ink select-none")
            .text(title),
        None => el("p").class("text-sm leading-snug text-ink").text(title),
    };
    el("div")
        .class("min-w-0 flex-1")
        .child(title_el)
        .child(detail.map(|d| {
            Html::from(
                el("p")
                    .class("mt-0.5 text-[12.5px] leading-snug text-muted")
                    .text(d),
            )
        }))
        .into()
}

#[must_use]
pub fn row_value(text: &str) -> Html {
    el("span")
        .class("numeric shrink-0 text-[13px] text-muted tabular-nums")
        .text(text)
        .into()
}

/// A checkbox drawn as the script page's switch (components/ui/switch.tsx).
/// It moves when clicked, in CSS (`has-checked:`), and is sent with its
/// form like any checkbox.
#[must_use]
pub fn switch(
    id: &str,
    name: &str,
    checked: bool,
    disabled: bool,
    described_by: Option<&str>,
) -> Html {
    el("label")
        .attr("data-slot", "switch")
        .attr("data-size", "default")
        .class("group/switch relative inline-flex shrink-0 cursor-pointer items-center rounded-full border border-line bg-sunk p-0.5 outline-none transition-colors duration-200 ease-out data-[size=default]:h-6 data-[size=default]:w-11 has-checked:border-transparent has-checked:bg-accent has-focus-visible:ring-2 has-focus-visible:ring-accent/40 has-disabled:cursor-not-allowed has-disabled:opacity-45 after:absolute after:-inset-x-2 after:-inset-y-2 after:content-['']")
        .child(
            el("input")
                .attr("type", "checkbox")
                .attr("role", "switch")
                .attr("id", id)
                .attr("name", name)
                .attr("value", "on")
                // What the box has: basic.css counts a switch that differs
                // as a change waiting for Apply.
                .attr("data-was", if checked { "on" } else { "off" })
                .attr_opt("aria-describedby", described_by)
                .class("sr-only")
                .flag("checked", checked)
                .flag("disabled", disabled),
        )
        .child(
            el("span")
                .attr("data-slot", "switch-thumb")
                .attr("aria-hidden", "true")
                .class("pointer-events-none block rounded-full bg-surface shadow-sm transition-[translate,width] duration-200 ease-snap group-data-[size=default]/switch:size-5 group-has-checked/switch:translate-x-5 group-active/switch:w-6 group-has-checked/switch:group-active/switch:translate-x-4"),
        )
        .into()
}

/// rows.tsx's SwitchRow: a title, a line under it, a switch on the right.
#[must_use]
pub fn switch_row(
    id: &str,
    name: &str,
    title: &str,
    description: Option<&str>,
    checked: bool,
    disabled: bool,
) -> Html {
    let described = description.map(|_| format!("{id}-description"));
    row()
        .when(disabled, |r| r.attr("data-disabled", "true"))
        .child(
            el("div")
                .attr("role", "group")
                .attr("data-slot", "field")
                .attr("data-orientation", "horizontal")
                .attr_opt("data-disabled", disabled.then_some("true"))
                .class("group/field flex w-full data-[invalid=true]:text-crit flex-row items-center [&>[data-slot=field-label]]:flex-auto gap-4 has-[>[data-slot=field-content]]:items-center has-[>[data-slot=field-content]]:[&>[data-slot=switch]]:mt-0")
                .child(
                    el("div")
                        .attr("data-slot", "field-content")
                        .class("group/field-content flex flex-1 flex-col gap-1 leading-snug min-w-0")
                        .child(
                            el("label")
                                .attr("data-slot", "field-label")
                                .attr("for", id)
                                .class("text-sm text-ink select-none group/field-label peer/field-label flex gap-2 leading-snug group-data-[disabled=true]/field:text-faint w-auto font-normal")
                                .text(title),
                        )
                        .child(description.map(|d| {
                            Html::from(
                                el("p")
                                    .attr("data-slot", "field-description")
                                    .attr_opt("id", described.as_deref())
                                    .class("text-[12.5px] leading-snug font-normal text-muted group-has-[[data-orientation=horizontal]]/field:text-balance")
                                    .text(d),
                            )
                        })),
                )
                .child(switch(id, name, checked, disabled, described.as_deref())),
        )
        .into()
}

/// components/ui/input.tsx's Input.
#[must_use]
pub fn input(id: &str, name: &str, value: &str) -> El {
    el("input")
        .attr("data-slot", "input")
        .attr("id", id)
        .attr("name", name)
        .attr("value", value)
        .class("h-9 min-w-0 rounded-control border border-line bg-surface px-3 text-sm text-ink placeholder:text-faint transition-[border-color,box-shadow] duration-150 hover:border-muted/60 focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/35 disabled:cursor-not-allowed disabled:bg-sunk disabled:text-faint aria-invalid:border-crit aria-invalid:ring-2 aria-invalid:ring-crit/25")
}

/// components/ui/native-select.tsx's NativeSelect, as wide as its cell.
/// `options` are (value, label) pairs; the one equal to `selected` is chosen.
#[must_use]
pub fn native_select(id: &str, name: &str, options: &[(String, String)], selected: &str) -> Html {
    el("div")
        .attr("data-slot", "native-select-wrapper")
        .class("group/native-select relative w-full has-[select:disabled]:opacity-50")
        .child(
            el("select")
                .attr("data-slot", "native-select")
                .attr("id", id)
                .attr("name", name)
                .class("h-9 w-full appearance-none rounded-control border border-line bg-surface py-1 pr-9 pl-3 text-sm text-ink transition-[border-color,box-shadow] duration-150 hover:border-muted/60 focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/35 disabled:cursor-not-allowed disabled:bg-sunk disabled:text-faint aria-invalid:border-crit aria-invalid:ring-2 aria-invalid:ring-crit/25")
                .children(options.iter().map(|(value, label)| {
                    el("option")
                        .attr("data-slot", "native-select-option")
                        .attr("value", value)
                        .flag("selected", value == selected)
                        .text(label)
                })),
        )
        .child(icon(
            crate::icons::ARROW_DOWN_01,
            16,
            "pointer-events-none absolute top-1/2 right-3 -translate-y-1/2 text-muted select-none",
        ))
        .into()
}

/// field.tsx's FieldLabel, the vertical kind a form field has.
#[must_use]
pub fn field_label(for_id: &str, text: &str) -> Html {
    el("label")
        .attr("data-slot", "field-label")
        .attr("for", for_id)
        .class("text-sm text-ink select-none group/field-label peer/field-label flex w-fit gap-2 leading-snug group-data-[disabled=true]/field:text-faint")
        .text(text)
        .into()
}

/// field.tsx's Field, vertical.
#[must_use]
pub fn field() -> El {
    el("div")
        .attr("role", "group")
        .attr("data-slot", "field")
        .attr("data-orientation", "vertical")
        .class("group/field flex w-full gap-2 data-[invalid=true]:text-crit flex-col [&>*]:w-full [&>.sr-only]:w-auto")
}

#[must_use]
pub fn field_description(text: &str) -> Html {
    el("p")
        .attr("data-slot", "field-description")
        .class("text-[12.5px] leading-snug font-normal text-muted nth-last-2:-mt-1 [&>a]:text-accent [&>a]:underline-offset-4 [&>a:hover]:underline")
        .text(text)
        .into()
}

#[must_use]
pub fn field_error(id: &str, text: &str) -> Html {
    el("p")
        .attr("data-slot", "field-error")
        .attr("id", id)
        .attr("role", "alert")
        .class("text-[12.5px] leading-snug font-normal text-crit")
        .text(text)
        .into()
}

// ── Empty ───────────────────────────────────────────────────────────────

#[must_use]
pub fn empty(glyph: &str, title: &str, description: &str) -> El {
    el("div")
        .attr("data-slot", "empty")
        .class("flex min-w-0 flex-1 flex-col items-center justify-center gap-5 p-6 text-center text-balance rounded-card border border-dashed border-line md:p-10 animate-fade-in")
        .child(
            el("div")
                .attr("data-slot", "empty-header")
                .class("flex max-w-sm flex-col items-center gap-2 text-center")
                .child(
                    el("div")
                        .attr("data-slot", "empty-media")
                        .attr("data-variant", "icon")
                        .class("flex shrink-0 items-center justify-center mb-1 [&_svg]:pointer-events-none [&_svg]:shrink-0 size-10 rounded-control bg-accent-wash text-accent [&_svg:not([class*='size-'])]:size-[22px]")
                        .child(icon(glyph, 24, "")),
                )
                .child(
                    el("h2")
                        .attr("data-slot", "empty-title")
                        .class("text-[15px] leading-tight font-semibold tracking-tight text-ink")
                        .text(title),
                )
                .child(
                    el("p")
                        .attr("data-slot", "empty-description")
                        .class("text-[13px] leading-relaxed text-muted [&>a]:text-accent [&>a]:underline [&>a]:underline-offset-4")
                        .text(description),
                ),
        )
}

/// A hidden form field.
#[must_use]
pub fn hidden(name: &str, value: &str) -> Html {
    el("input")
        .attr("type", "hidden")
        .attr("name", name)
        .attr("value", value)
        .into()
}

// ── Dialog ──────────────────────────────────────────────────────────────

/// components/ui/dialog.tsx, open: the native `<dialog>` drawn over a
/// backdrop of its own, since only a script can make one modal. Its
/// opening fade and the backdrop's are index.css's `dialog` rules, which
/// `@starting-style` plays without a script; basic.css centres it.
/// `size` is the width class the script page passes (`w-[min(26rem,…)]`).
#[must_use]
pub fn dialog(size: &str, labelled_by: &str, body: impl Into<Html>) -> Html {
    el("div")
        .attr("data-slot", "basic-modal")
        .child(
            el("dialog")
                .flag("open", true)
                .attr("aria-modal", "true")
                .attr("aria-labelledby", labelled_by)
                .class(size)
                .child(
                    el("div")
                        .class("rounded-card border border-line bg-surface text-ink shadow-pop max-h-[calc(100dvh-4rem)] overflow-y-auto")
                        .child(body),
                ),
        )
        .into()
}

#[must_use]
pub fn dialog_header(title_id: &str, title: &str, description: &str) -> Html {
    el("div")
        .class("flex flex-col gap-1.5 px-5 pt-5 pb-2")
        .child(
            el("h2")
                .id(title_id)
                .class("text-base leading-tight font-semibold")
                .text(title),
        )
        .child(
            el("p")
                .class("text-[13px] leading-snug text-muted")
                .text(description),
        )
        .into()
}

#[must_use]
pub fn dialog_body() -> El {
    el("div").class("px-5 py-3")
}

#[must_use]
pub fn dialog_footer() -> El {
    el("div")
        .class("flex flex-wrap items-center justify-end gap-2 border-t border-hair px-5 py-3.5")
}

// ── Motion while something runs ─────────────────────────────────────────

/// components/ui/spinner.tsx.
#[must_use]
pub fn spinner(size: u32, label: &str, class: &str) -> Html {
    el("span")
        .attr("role", "status")
        .attr("data-slot", "spinner")
        .class("inline-flex items-center")
        .class(class)
        .child(icon(crate::icons::LOADING_03, size, "animate-spin-slow"))
        .child(el("span").class("sr-only").text(label))
        .into()
}

/// components/ui/progress.tsx's Progress with no value: the sweeping band.
#[must_use]
pub fn progress_sweep(label: &str, class: &str) -> Html {
    el("div")
        .attr("role", "progressbar")
        .attr("aria-label", label)
        .attr("data-slot", "progress")
        .class("h-1.5 w-full overflow-hidden rounded-full bg-sunk")
        .class(class)
        .child(el("div").class("h-full w-1/3 animate-sweep rounded-full bg-accent"))
        .into()
}

/// components/ui/scroll-area.tsx's LogView: lososd's last words.
#[must_use]
pub fn log_view(text: &str) -> Html {
    el("div")
        .attr("data-slot", "log-view")
        .attr("aria-live", "polite")
        .class("numeric max-h-40 overflow-auto rounded-control bg-sunk px-3 py-2 text-[12.5px] leading-relaxed break-words whitespace-pre-wrap text-muted")
        .text(text)
        .into()
}

// ── Toasts (components/ui/toaster.tsx) ──────────────────────────────────

/// One confirmation: `tone` is done, success or error.
pub struct Toast {
    pub tone: &'static str,
    pub title: String,
    pub body: String,
}

/// The confirmations' corner. Each comes in and, a few seconds later,
/// goes by CSS alone (basic.css's `[data-basic-toast]`); its close button
/// is a checkbox's label, which hides it at once.
#[must_use]
pub fn toasts(dismiss: &str, label: &str, list: &[Toast]) -> Html {
    if list.is_empty() {
        return Html::new();
    }
    el("ol")
        .attr("aria-label", label)
        .attr("data-basic-toasts", "")
        .class("fixed top-4 right-4 z-[2147483000] m-0 flex w-[356px] max-w-[calc(100vw-2rem)] list-none flex-col gap-2.5 p-0 outline-none max-sm:top-3 max-sm:right-3 max-sm:w-[calc(100vw-1.5rem)] max-sm:max-w-none")
        .children(list.iter().enumerate().map(|(n, t)| {
            let (tone, glyph) = match t.tone {
                "success" => ("[--tone:var(--ok)]", crate::icons::CHECKMARK_CIRCLE_02),
                "error" => ("[--tone:var(--crit)]", crate::icons::ALERT_02),
                _ => ("[--tone:var(--accent)]", crate::icons::INFORMATION_CIRCLE),
            };
            let id = format!("basic-toast-{n}");
            el("li")
                .attr("role", if t.tone == "error" { "alert" } else { "status" })
                .attr("data-basic-toast", "")
                .attr("data-tone", t.tone)
                .class("group pointer-events-auto relative flex w-full items-start gap-2.5 rounded-card border border-line bg-surface py-3 pr-3.5 pl-[18px] text-ink shadow-[inset_3px_0_0_0_var(--tone),var(--elev-pop)]")
                .class(tone)
                .child(el("input").attr("type", "checkbox").id(&id).class("sr-only").attr("tabindex", "-1").attr("aria-hidden", "true"))
                .child(el("span").attr("aria-hidden", "true").class("mt-px shrink-0 text-[var(--tone)]").child(icon(glyph, 20, "")))
                .child(
                    el("div")
                        .class("min-w-0 flex-1 py-px")
                        .child(el("p").class("text-[13.5px] leading-[1.35] font-medium text-ink").text(&t.title))
                        .when(!t.body.is_empty(), |d| {
                            d.child(el("p").class("mt-0.5 text-[12.5px] leading-[1.35] text-muted [overflow-wrap:anywhere]").text(&t.body))
                        }),
                )
                .child(
                    el("label")
                        .attr("for", &id)
                        .attr("aria-label", dismiss)
                        .attr("title", dismiss)
                        .class("absolute top-0 left-0 z-10 flex size-5 -translate-x-[35%] -translate-y-[35%] cursor-pointer items-center justify-center rounded-full border border-line bg-surface p-0 text-muted transition-[background-color,color] duration-150 hover:bg-sunk hover:text-ink")
                        .child(icon(crate::icons::CANCEL_01, 12, "")),
                )
        }))
        .into()
}
