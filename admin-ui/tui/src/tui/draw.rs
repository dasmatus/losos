//! Drawing the shell into a ratatui buffer: every line is laid out by the
//! model first, so drawing is painting spans onto their backgrounds.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::app::{App, Focus, Modal};
use super::model::{clip, width, wrap, Line, Span, Tone};
use crate::i18n::t;
use crate::theme::{logo_cells, LogoSize, Theme, C};
use crate::tr;

pub const SIDEBAR: u16 = 26;
pub const HEADER: u16 = 5;

/// Paints lines with the theme.
pub struct Painter<'a> {
    pub buf: &'a mut Buffer,
    pub theme: Theme,
}

impl Painter<'_> {
    fn style(&self, span: &Span, bg: C) -> Style {
        let mut style = Style::default().bg(self.theme.c(span.bg.unwrap_or(bg)));
        if let Some(c) = span.color {
            style = style.fg(self.theme.c(c));
        }
        if span.bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        style
    }

    /// One laid-out line at (x, y), padded to `w` with its background.
    pub fn line(&mut self, x: u16, y: u16, w: u16, line: &Line) {
        let area = *self.buf.area();
        if y >= area.bottom() || x >= area.right() {
            return;
        }
        let w = w.min(area.right() - x);
        let ground = Style::default().bg(self.theme.c(line.bg));
        self.buf.set_style(Rect::new(x, y, w, 1), ground);
        for c in x..x + w {
            self.buf[(c, y)].set_symbol(" ");
        }
        let mut at = x;
        for span in &line.spans {
            let room = (x + w).saturating_sub(at) as usize;
            if room == 0 {
                break;
            }
            let text = clip(&span.text, room);
            let style = self.style(span, line.bg);
            let (next, _) = self.buf.set_stringn(at, y, text, room, style);
            at = next;
        }
    }

    pub fn fill(&mut self, area: Rect, bg: C) {
        let style = Style::default().bg(self.theme.c(bg));
        self.buf.set_style(area, style);
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                self.buf[(x, y)].set_symbol(" ");
            }
        }
    }

    /// The salmon at (x, y), on the ground colour.
    pub fn logo(&mut self, x: u16, y: u16, size: LogoSize) -> (u16, u16) {
        let cells = logo_cells(size, self.theme.ground_rgb());
        let ground = self.theme.c(C::Ground);
        let area = *self.buf.area();
        for (dy, row) in cells.iter().enumerate() {
            let yy = y + dy as u16;
            if yy >= area.bottom() {
                break;
            }
            for (dx, cell) in row.iter().enumerate() {
                let xx = x + dx as u16;
                if xx >= area.right() {
                    break;
                }
                let mut style = Style::default().bg(cell.bg.map_or(ground, |c| self.theme.rgb(c)));
                if let Some(fg) = cell.fg {
                    style = style.fg(self.theme.rgb(fg));
                }
                let target = &mut self.buf[(xx, yy)];
                target.set_char(cell.ch);
                target.set_style(style);
            }
        }
        (
            cells.first().map_or(0, |r| r.len()) as u16,
            cells.len() as u16,
        )
    }
}

fn tone_colour(tone: Tone) -> C {
    tone.colour()
}

/// The whole shell.
pub fn draw(app: &mut App, area: Rect, buf: &mut Buffer, theme: Theme) {
    let mut p = Painter { buf, theme };
    p.fill(area, C::Ground);
    let cols = area.width;
    let rows = area.height;
    let bar_rows: u16 = if app.form.rebuild.is_some() || app.form.dirty() {
        2
    } else {
        0
    };
    let notice_rows: u16 = u16::from(app.notice.is_some());
    let body_rows = rows
        .saturating_sub(HEADER + 1 + 1 + bar_rows + notice_rows)
        .max(6);
    let pane_cols = cols.saturating_sub(SIDEBAR + 1).max(30);
    let inner_cols = pane_cols.saturating_sub(2);
    app.lay_out(inner_cols as usize, body_rows as usize);

    header(app, &mut p, area);
    let sep = Line::new(
        vec![Span::new("─".repeat(cols as usize), C::Line)],
        C::Ground,
    );
    p.line(area.x, area.y + HEADER, cols, &sep);

    let top = area.y + HEADER + 1;
    sidebar(app, &mut p, area.x, top, body_rows);
    for i in 0..body_rows {
        p.line(
            area.x + SIDEBAR,
            top + i,
            1,
            &Line::new(vec![Span::new("│", C::Line)], C::Ground),
        );
    }
    let px = area.x + SIDEBAR + 1;
    match app.modal.clone() {
        None => {
            let (lines, _) = &app.laid;
            let more = lines.len() > app.scroll + body_rows as usize;
            for i in 0..body_rows {
                let mut line = lines
                    .get(app.scroll + i as usize)
                    .cloned()
                    .unwrap_or_else(Line::blank);
                line.spans.insert(0, Span::raw(" ").on(C::Ground));
                p.line(px, top + i, pane_cols.saturating_sub(1), &line);
                let mark = if i + 1 == body_rows && more {
                    "↓"
                } else {
                    " "
                };
                p.line(
                    px + pane_cols.saturating_sub(1),
                    top + i,
                    1,
                    &Line::new(vec![Span::new(mark, C::Faint)], C::Ground),
                );
            }
        }
        Some(modal) => modal_view(&mut p, &modal, px, top, pane_cols, body_rows),
    }

    let mut y = top + body_rows;
    if let Some((notice, _)) = &app.notice {
        let text = if notice.text.is_empty() {
            notice.title.clone()
        } else {
            format!("{}: {}", notice.title, notice.text)
        };
        let tone = match notice.tone {
            Tone::Ok => C::Ok,
            Tone::Crit => C::Crit,
            _ => C::Accent,
        };
        p.line(
            area.x,
            y,
            cols,
            &Line::new(
                vec![Span::new(" ● ", tone), Span::new(text, C::Ink)],
                C::Sunk,
            ),
        );
        y += 1;
    }
    if bar_rows > 0 {
        let (first, second) = match &app.form.rebuild {
            Some(r) => (
                vec![
                    Span::new(" ◐ ", C::Accent).bold(),
                    Span::new(r.title.clone(), C::Ink).bold(),
                ],
                vec![Span::new(format!("   {}", r.message), C::Muted)],
            ),
            None => {
                let valid = app.form.valid();
                (
                    vec![
                        Span::new(" ● ", C::Accent),
                        Span::new(
                            tr!("settings.applyBar.pending", count = app.form.change_count()),
                            C::Ink,
                        )
                        .bold(),
                    ],
                    vec![Span::new(
                        format!(
                            "   {}",
                            t(if valid {
                                "settings.applyBar.valid"
                            } else {
                                "settings.applyBar.invalid"
                            })
                        ),
                        if valid { C::Muted } else { C::Crit },
                    )],
                )
            }
        };
        p.line(area.x, y, cols, &Line::new(first, C::Surface));
        p.line(area.x, y + 1, cols, &Line::new(second, C::Surface));
        y += 2;
    }
    footer(app, &mut p, area.x, y, cols);
}

fn header(app: &App, p: &mut Painter<'_>, area: Rect) {
    let (status, tone) = app.status();
    let (logo_w, _) = p.logo(area.x + 1, area.y, LogoSize::Small);
    let lines: Vec<Vec<Span>> = vec![
        vec![
            Span::new("LosOS", C::Ink).bold(),
            Span::new(format!("  {}", app.host.kind.word()), C::Faint),
        ],
        vec![Span::new(app.title(), C::Ink).bold()],
        vec![Span::new(app.host.url.clone(), C::Muted)],
        vec![
            Span::new("● ", tone_colour(tone)),
            Span::new(
                status,
                if tone == Tone::Muted {
                    C::Muted
                } else {
                    C::Ink
                },
            ),
        ],
        vec![],
    ];
    let x = area.x + 1 + logo_w;
    for (i, spans) in lines.into_iter().enumerate() {
        let mut all = vec![Span::raw("  ")];
        all.extend(spans);
        p.line(
            x,
            area.y + i as u16,
            area.width.saturating_sub(x - area.x),
            &Line::new(all, C::Ground),
        );
    }
}

fn sidebar(app: &App, p: &mut Painter<'_>, x: u16, top: u16, rows: u16) {
    let mut lines = Vec::new();
    match &app.filter {
        Some(f) => lines.push(Line::new(
            vec![
                Span::new(" / ", C::Accent).bold(),
                Span::new(f.clone(), C::Ink),
                Span::new("▏", C::Accent),
            ],
            C::Sunk,
        )),
        None => lines.push(Line::new(
            vec![Span::new(format!(" / {}", t("tui.searchPanes")), C::Faint)],
            C::Ground,
        )),
    }
    lines.push(Line::blank());
    let shown = app.shown_panes();
    for (i, pane) in shown.iter().enumerate() {
        let current = *pane == app.pane;
        let pointed = (app.focus == Focus::Sidebar || app.filter.is_some()) && i == app.side_cursor;
        let mut label = Span::new(
            pane.label(),
            if current {
                C::Accent
            } else if pointed {
                C::Ink
            } else {
                C::Muted
            },
        );
        label.bold = current || pointed;
        lines.push(Line::new(
            vec![
                Span::new(if current { " ▌" } else { "  " }, C::Accent),
                label,
            ],
            if pointed {
                C::AccentWash
            } else if current {
                C::Sunk
            } else {
                C::Ground
            },
        ));
    }
    if app.filter.is_some() && shown.is_empty() {
        lines.push(Line::new(
            vec![Span::new(format!("  {}", t("tui.noPaneMatches")), C::Faint)],
            C::Ground,
        ));
    }
    for i in 0..rows {
        let line = lines.get(i as usize).cloned().unwrap_or_else(Line::blank);
        p.line(x, top + i, SIDEBAR, &line);
    }
}

fn footer(app: &App, p: &mut Painter<'_>, x: u16, y: u16, cols: u16) {
    let dirty = app.form.dirty() && app.form.rebuild.is_none();
    let keys: Vec<(&str, String)> = if app.filter.is_some() {
        vec![
            ("↑↓", t("tui.keys.move")),
            ("Enter", t("tui.keys.open")),
            ("Esc", t("tui.keys.clear")),
        ]
    } else {
        let mut keys = vec![
            ("↑↓ j k", t("tui.keys.move")),
            (
                "Enter",
                t(if app.focus == Focus::Sidebar {
                    "tui.keys.open"
                } else {
                    "tui.keys.change"
                }),
            ),
            ("Tab", t("tui.keys.switch")),
            ("/", t("tui.keys.search")),
        ];
        if dirty {
            keys.push(("a", t("settings.applyBar.apply")));
            keys.push(("d", t("settings.applyBar.discard")));
        }
        keys.push(("r", t("tui.keys.refresh")));
        keys.push(("?", t("tui.keys.help")));
        keys.push(("q", t("tui.keys.quit")));
        keys
    };
    let mut spans = vec![Span::raw(" ")];
    for (k, what) in keys {
        spans.push(Span::new(format!(" {k} "), C::Ink).on(C::Sunk).bold());
        spans.push(Span::new(format!(" {what}  "), C::Muted));
    }
    p.line(x, y, cols, &Line::new(spans, C::Ground));
}

fn modal_view(p: &mut Painter<'_>, modal: &Modal, x: u16, top: u16, cols: u16, rows: u16) {
    let w = cols.saturating_sub(4).min(70);
    let pad = (cols - w) / 2;
    let card = |spans: Vec<Span>| {
        let mut all = vec![Span::raw("  ")];
        all.extend(spans);
        Line::new(all, C::Surface)
    };
    let mut lines: Vec<Line> = Vec::new();
    let mut field: Option<(String, bool)> = None;
    let mut after: Vec<Line> = Vec::new();
    match modal {
        Modal::Help => {
            lines.push(card(vec![Span::new(t("tui.help.title"), C::Ink).bold()]));
            lines.push(card(vec![]));
            for (k, what) in [
                ("↑ ↓  j k", "tui.help.move"),
                ("Enter  Space", "tui.help.enter"),
                ("Tab", "tui.help.tab"),
                ("← h  Esc", "tui.help.back"),
                ("/", "tui.help.search"),
                ("a  d", "tui.help.apply"),
                ("r", "tui.help.refresh"),
                ("PgUp PgDn", "tui.help.page"),
                ("q  Ctrl-C", "tui.help.quit"),
            ] {
                lines.push(card(vec![
                    Span::new(format!("{k:<14}"), C::Accent).bold(),
                    Span::new(t(what), C::Muted),
                ]));
            }
            lines.push(card(vec![]));
            lines.push(card(vec![Span::new(t("tui.help.close"), C::Faint)]));
        }
        Modal::Busy(text) => lines.push(card(vec![Span::new(format!("◐ {text}"), C::Accent)])),
        Modal::Confirm { spec, .. } => {
            lines.push(card(vec![Span::new(spec.title.clone(), C::Ink).bold()]));
            lines.push(card(vec![]));
            for paragraph in &spec.body {
                for text in wrap(paragraph, w.saturating_sub(4) as usize) {
                    lines.push(card(vec![Span::new(text, C::Muted)]));
                }
                lines.push(card(vec![]));
            }
            lines.push(card(vec![
                Span::new(" Enter ", C::Ground).on(C::Accent).bold(),
                Span::new(format!(" {}    ", spec.confirm), C::Ink),
                Span::new(" Esc ", C::Ink).on(C::Sunk).bold(),
                Span::new(format!(" {}", spec.cancel), C::Muted),
            ]));
        }
        Modal::Edit {
            prompt,
            masked,
            value,
            ..
        } => {
            for text in wrap(prompt, w.saturating_sub(4) as usize) {
                lines.push(card(vec![Span::new(text, C::Ink).bold()]));
            }
            lines.push(card(vec![]));
            field = Some((value.clone(), *masked));
            after.push(card(vec![]));
            after.push(card(vec![Span::new(
                t(if *masked {
                    "tui.edit.hintMasked"
                } else {
                    "tui.edit.hint"
                }),
                C::Faint,
            )]));
        }
    }
    let mut y = top + 1;
    for line in &lines {
        p.line(x + pad, y, w, line);
        y += 1;
    }
    if let Some((value, masked)) = field {
        let shown = if masked {
            "•".repeat(value.chars().count())
        } else {
            value
        };
        let fill = 40usize.saturating_sub(width(&shown)).max(1);
        p.line(x + pad, y, 2, &card(vec![]));
        let field_w = (width(&shown) + 2 + fill) as u16;
        p.line(
            x + pad + 2,
            y,
            field_w.min(w.saturating_sub(2)),
            &Line::new(
                vec![
                    Span::new(format!(" {shown}"), C::Ink),
                    Span::new("▏", C::Accent),
                    Span::raw(" ".repeat(fill)),
                ],
                C::Sunk,
            ),
        );
        y += 1;
    }
    for line in &after {
        p.line(x + pad, y, w, line);
        y += 1;
    }
    let _ = rows;
}
