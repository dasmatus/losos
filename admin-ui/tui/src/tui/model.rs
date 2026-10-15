//! A pane is data: sections of rows, the web UI's settings vocabulary
//! (admin-ui/app/src/screens/settings/rows.tsx). A group title above, a card
//! of rows (title on the left, the value or control on the right, a detail
//! line under the title), a caption under the card. [`layout`] turns that
//! into terminal lines, and the shell moves a cursor over the rows.
//!
//! What a row does is data too ([`Control`]), so the panes are testable
//! without a terminal and the shell, not the pane, runs anything.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::form::SettingKey;
use crate::theme::C;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Span {
    pub text: String,
    pub color: Option<C>,
    pub bg: Option<C>,
    pub bold: bool,
}

impl Span {
    pub fn raw(text: impl Into<String>) -> Span {
        Span {
            text: text.into(),
            ..Span::default()
        }
    }

    pub fn new(text: impl Into<String>, color: C) -> Span {
        Span {
            text: text.into(),
            color: Some(color),
            ..Span::default()
        }
    }

    pub fn bold(mut self) -> Span {
        self.bold = true;
        self
    }

    pub fn on(mut self, bg: C) -> Span {
        self.bg = Some(bg);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Ink,
    Muted,
    Faint,
    Accent,
    Ok,
    Warn,
    Crit,
}

impl Tone {
    pub fn colour(self) -> C {
        match self {
            Tone::Ink => C::Ink,
            Tone::Muted => C::Muted,
            Tone::Faint => C::Faint,
            Tone::Accent => C::Accent,
            Tone::Ok => C::Ok,
            Tone::Warn => C::Warn,
            Tone::Crit => C::Crit,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmSpec {
    pub title: String,
    pub body: Vec<String>,
    pub confirm: String,
    pub cancel: String,
}

/// A chart the Apps pane can install: where it comes from.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Chart {
    pub repo: String,
    pub name: String,
    pub version: String,
}

/// What an install will do, once the box has named the release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPlan {
    pub release: String,
    pub chart: Chart,
    pub run_as: String,
}

/// Work a row starts. Each runs off the screen's thread, against the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Job {
    Grow,
    RemoveApp(String),
    InstallApp(InstallPlan),
    SyncConfig,
    RunBackup,
    Reconcile,
    SetSecret { name: String, value: String },
}

/// A question that needs the host's answer before it can be asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prepare {
    /// Ask the box what the release would be called, then confirm.
    Install(Chart),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Confirm {
    Ready(ConfirmSpec),
    Prepare(Prepare),
}

/// Where an edited value goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditTarget {
    Setting(SettingKey),
    AppSearch,
    Secret(String),
    /// The owner's password, to set up this computer for one of the two.
    Computer(crate::computer::Service),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Control {
    Toggle {
        on: bool,
        disabled: bool,
        key: SettingKey,
    },
    Choice {
        value: String,
        options: Vec<(String, String)>,
        disabled: bool,
        key: SettingKey,
    },
    Edit {
        value: String,
        prompt: String,
        masked: bool,
        disabled: bool,
        target: EditTarget,
    },
    Run {
        label: String,
        disabled: bool,
        confirm: Option<Confirm>,
        job: Option<Job>,
    },
    /// A button that asks for a secret, typed without echo, and hands it
    /// to `target`.
    Ask {
        label: String,
        prompt: String,
        disabled: bool,
        target: EditTarget,
    },
}

impl Control {
    pub fn disabled(&self) -> bool {
        match self {
            Control::Toggle { disabled, .. }
            | Control::Choice { disabled, .. }
            | Control::Edit { disabled, .. }
            | Control::Run { disabled, .. }
            | Control::Ask { disabled, .. } => *disabled,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Row {
    pub key: String,
    pub title: String,
    pub detail: Option<String>,
    /// Shown on the right.
    pub value: Option<String>,
    pub tone: Option<Tone>,
    pub control: Option<Control>,
    /// Lines drawn under the row's text, full width (a meter, an hour strip).
    pub extra: Vec<Vec<Span>>,
}

impl Row {
    pub fn new(key: impl Into<String>, title: impl Into<String>) -> Row {
        Row {
            key: key.into(),
            title: title.into(),
            ..Row::default()
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Row {
        let detail = detail.into();
        if !detail.is_empty() {
            self.detail = Some(detail);
        }
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> Row {
        self.value = Some(value.into());
        self
    }

    pub fn tone(mut self, tone: Tone) -> Row {
        self.tone = Some(tone);
        self
    }

    pub fn control(mut self, control: Control) -> Row {
        self.control = Some(control);
        self
    }

    pub fn extra(mut self, extra: Vec<Vec<Span>>) -> Row {
        self.extra = extra;
        self
    }
}

/// A notice above a card, with a tone: the web UI's Alert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub title: Option<String>,
    pub text: String,
    pub tone: Tone,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Section {
    pub key: String,
    pub title: Option<String>,
    pub notice: Option<Notice>,
    pub rows: Vec<Row>,
    pub caption: Option<String>,
    /// Lines drawn in place of a card (a table, the app grid).
    pub lines: Vec<Vec<Span>>,
}

impl Section {
    pub fn new(key: impl Into<String>) -> Section {
        Section {
            key: key.into(),
            ..Section::default()
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Section {
        self.title = Some(title.into());
        self
    }

    pub fn notice(mut self, tone: Tone, title: Option<String>, text: impl Into<String>) -> Section {
        self.notice = Some(Notice {
            title,
            text: text.into(),
            tone,
        });
        self
    }

    pub fn rows(mut self, rows: Vec<Row>) -> Section {
        self.rows = rows;
        self
    }

    pub fn row(mut self, row: Row) -> Section {
        self.rows.push(row);
        self
    }

    pub fn caption(mut self, caption: impl Into<String>) -> Section {
        let caption = caption.into();
        if !caption.is_empty() {
            self.caption = Some(caption);
        }
        self
    }

    pub fn lines(mut self, lines: Vec<Vec<Span>>) -> Section {
        self.lines = lines;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PaneModel {
    pub title: String,
    pub summary: Option<String>,
    pub sections: Vec<Section>,
}

impl PaneModel {
    pub fn new(title: impl Into<String>, summary: impl Into<String>) -> PaneModel {
        let summary = summary.into();
        PaneModel {
            title: title.into(),
            summary: (!summary.is_empty()).then_some(summary),
            sections: Vec::new(),
        }
    }

    pub fn with(mut self, sections: Vec<Section>) -> PaneModel {
        self.sections = sections;
        self
    }

    /// Every row, flat, in cursor order.
    pub fn rows(&self) -> Vec<&Row> {
        self.sections.iter().flat_map(|s| s.rows.iter()).collect()
    }
}

/// A laid-out line. `row` is the index into the pane's rows when the line
/// belongs to one, so the cursor can be painted and kept in view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub spans: Vec<Span>,
    pub bg: C,
    pub row: Option<usize>,
}

impl Line {
    pub fn new(spans: Vec<Span>, bg: C) -> Line {
        Line {
            spans,
            bg,
            row: None,
        }
    }

    pub fn blank() -> Line {
        Line::new(Vec::new(), C::Ground)
    }
}

/// Columns a string takes on screen.
pub fn width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

/// The longest start of `text` that fits in `max` columns.
pub fn clip(text: &str, max: usize) -> &str {
    let mut used = 0;
    for (i, c) in text.char_indices() {
        let w = c.width().unwrap_or(0);
        if used + w > max {
            return &text[..i];
        }
        used += w;
    }
    text
}

/// `text` cut to `max` columns with an ellipsis, or as it is.
pub fn truncate(text: &str, max: usize) -> String {
    if width(text) <= max {
        return text.to_string();
    }
    if max == 0 {
        return String::new();
    }
    format!("{}…", clip(text, max - 1))
}

/// Pad or cut to exactly `n` columns, an ellipsis and a space where cut.
pub fn pad(text: &str, n: usize) -> String {
    let w = width(text);
    if w >= n {
        format!("{} ", truncate(text, n.saturating_sub(1)))
    } else {
        format!("{text}{}", " ".repeat(n - w))
    }
}

/// Word wrap, hard-splitting a word longer than the width.
pub fn wrap(text: &str, max: usize) -> Vec<String> {
    if max <= 1 {
        return vec![text.to_string()];
    }
    let mut out = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let mut rest = word;
            while width(rest) > max {
                if !line.is_empty() {
                    out.push(std::mem::take(&mut line));
                }
                let head = clip(rest, max);
                out.push(head.to_string());
                rest = &rest[head.len()..];
            }
            if line.is_empty() {
                line = rest.to_string();
            } else if width(&line) + 1 + width(rest) <= max {
                line.push(' ');
                line.push_str(rest);
            } else {
                out.push(std::mem::replace(&mut line, rest.to_string()));
            }
        }
        out.push(line);
    }
    out
}

/// What a row's control looks like on the right.
pub fn control_spans(row: &Row) -> Vec<Span> {
    let on = crate::i18n::t("advanced.on");
    let off = crate::i18n::t("advanced.off");
    match &row.control {
        Some(Control::Toggle {
            on: true, disabled, ..
        }) => {
            let s = Span::new(
                format!(" ● {on} "),
                if *disabled { C::Faint } else { C::Accent },
            );
            vec![if *disabled { s } else { s.bold() }]
        }
        Some(Control::Toggle { .. }) => vec![Span::new(format!(" ○ {off} "), C::Faint)],
        Some(Control::Choice {
            value,
            options,
            disabled,
            ..
        }) => {
            let label = options
                .iter()
                .find(|(v, _)| v == value)
                .map_or(value.as_str(), |(_, l)| l.as_str());
            vec![Span::new(
                format!("‹ {label} ›"),
                if *disabled { C::Faint } else { C::Ink },
            )]
        }
        Some(Control::Edit {
            value,
            masked,
            disabled,
            ..
        }) => {
            let shown = if *masked {
                "••••••"
            } else if value.is_empty() {
                "-"
            } else {
                value.as_str()
            };
            vec![Span::new(
                format!("[ {shown} ]"),
                if *disabled { C::Faint } else { C::Ink },
            )]
        }
        Some(
            Control::Run {
                label, disabled, ..
            }
            | Control::Ask {
                label, disabled, ..
            },
        ) => {
            let s = Span::new(
                format!("[ {label} ]"),
                if *disabled { C::Faint } else { C::Accent },
            );
            vec![if *disabled { s } else { s.bold() }]
        }
        None => match &row.value {
            Some(v) => vec![Span::new(
                v.clone(),
                row.tone.map_or(C::Muted, Tone::colour),
            )],
            None => Vec::new(),
        },
    }
}

/// Lay a pane out at `cols` wide. Rows are numbered in order across
/// sections; that number is what the shell's cursor holds.
pub fn layout(model: &PaneModel, cols: usize, cursor: Option<usize>) -> (Vec<Line>, usize) {
    let mut lines = Vec::new();
    let inner = cols.saturating_sub(4).max(20);

    lines.push(Line::new(
        vec![Span::new(model.title.clone(), C::Ink).bold()],
        C::Ground,
    ));
    if let Some(summary) = &model.summary {
        for text in wrap(summary, cols) {
            lines.push(Line::new(vec![Span::new(text, C::Muted)], C::Ground));
        }
    }

    let mut index = 0;
    for section in &model.sections {
        lines.push(Line::blank());
        if let Some(title) = &section.title {
            lines.push(Line::new(
                vec![Span::new(title.clone(), C::Muted).bold()],
                C::Ground,
            ));
        }
        if let Some(notice) = &section.notice {
            let colour = notice.tone.colour();
            if let Some(title) = &notice.title {
                lines.push(Line::new(
                    vec![
                        Span::new("▌ ", colour),
                        Span::new(title.clone(), C::Ink).bold(),
                    ],
                    C::Sunk,
                ));
            }
            for text in wrap(&notice.text, cols.saturating_sub(2)) {
                lines.push(Line::new(
                    vec![Span::new("▌ ", colour), Span::new(text, C::Muted)],
                    C::Sunk,
                ));
            }
            if !section.rows.is_empty() || !section.lines.is_empty() {
                lines.push(Line::blank());
            }
        }
        for spans in &section.lines {
            lines.push(Line::new(spans.clone(), C::Surface));
        }
        for (i, row) in section.rows.iter().enumerate() {
            let selected = cursor == Some(index);
            let bg = if selected { C::AccentWash } else { C::Surface };
            let control = control_spans(row);
            let control_width: usize = control.iter().map(|s| width(&s.text)).sum();
            let title_room = inner.saturating_sub(control_width + 2).max(8);
            let marker = Span::new(if selected { "› " } else { "  " }, C::Accent).bold();
            let title_colour = if selected { C::Accent } else { C::Ink };
            for (n, text) in wrap(&row.title, title_room).into_iter().enumerate() {
                let gap = inner.saturating_sub(width(&text) + control_width).max(1);
                let mut title = Span::new(text, title_colour);
                title.bold = selected;
                let mut spans = vec![
                    if n == 0 {
                        marker.clone()
                    } else {
                        Span::raw("  ")
                    },
                    title,
                ];
                if n == 0 && control_width > 0 {
                    spans.push(Span::raw(" ".repeat(gap)));
                    spans.extend(control.iter().cloned());
                }
                lines.push(Line {
                    spans,
                    bg,
                    row: Some(index),
                });
            }
            if let Some(detail) = &row.detail {
                for text in wrap(detail, inner) {
                    lines.push(Line {
                        spans: vec![Span::raw("  "), Span::new(text, C::Muted)],
                        bg,
                        row: Some(index),
                    });
                }
            }
            for extra in &row.extra {
                let mut spans = vec![Span::raw("  ")];
                spans.extend(extra.iter().cloned());
                lines.push(Line {
                    spans,
                    bg,
                    row: Some(index),
                });
            }
            if i + 1 < section.rows.len() {
                lines.push(Line::new(
                    vec![Span::raw("  "), Span::new("─".repeat(inner), C::Hair)],
                    C::Surface,
                ));
            }
            index += 1;
        }
        if let Some(caption) = &section.caption {
            for text in wrap(caption, cols) {
                lines.push(Line::new(vec![Span::new(text, C::Faint)], C::Ground));
            }
        }
    }
    (lines, index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_breaks_on_words_and_splits_long_ones() {
        assert_eq!(wrap("one two three", 7), vec!["one two", "three"]);
        assert_eq!(wrap("abcdefghij", 4), vec!["abcd", "efgh", "ij"]);
        assert_eq!(wrap("a\nb", 10), vec!["a", "b"]);
        assert_eq!(wrap("", 10), vec![""]);
    }

    #[test]
    fn truncate_and_pad_count_columns() {
        assert_eq!(truncate("Úložisko", 4), "Úlo…");
        assert_eq!(pad("ab", 4), "ab  ");
        assert_eq!(pad("abcdef", 4), "ab… ");
    }

    #[test]
    fn rows_are_numbered_across_sections() {
        let model = PaneModel::new("T", "").with(vec![
            Section::new("a").rows(vec![Row::new("1", "one"), Row::new("2", "two")]),
            Section::new("b").rows(vec![Row::new("3", "three")]),
        ]);
        let (lines, count) = layout(&model, 60, Some(2));
        assert_eq!(count, 3);
        let selected: Vec<&Line> = lines.iter().filter(|l| l.bg == C::AccentWash).collect();
        assert_eq!(selected.len(), 1);
        assert!(selected[0].spans.iter().any(|s| s.text == "three"));
    }
}
