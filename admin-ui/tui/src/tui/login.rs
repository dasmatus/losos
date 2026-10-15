//! `losos login`: pin the host key the way SSH pins a host key, then take
//! the token and the key that signs for it.
//!
//! 1. the host key's fingerprint, in groups of four, and the question
//!    whether it matches the one the web UI's Access tokens pane shows;
//! 2. the token, typed without echo;
//! 3. which of the stored keys signs (skipped when there is one).
//!
//! And the host picker, when more than one host is pinned and none named.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::app::Key;
use super::draw::Painter;
use super::model::{wrap, Line, Span};
use crate::i18n::t;
use crate::theme::{LogoSize, Theme, C};
use crate::vault::{group_fingerprint, HostKind, HostRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyChoice {
    pub fingerprint: String,
    pub user_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginAnswer {
    pub token: String,
    pub key_fingerprint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Fingerprint,
    Token,
    Key,
}

pub struct Login {
    pub url: String,
    pub kind: HostKind,
    pub fingerprint: String,
    pub keys: Vec<KeyChoice>,
    /// The key named with `--key`.
    pub preset_key: Option<String>,
    pub step: Step,
    pub token: String,
    pub key_index: usize,
    /// Set when the screen is done: the answer, or None when refused.
    pub outcome: Option<Option<LoginAnswer>>,
}

impl Login {
    pub fn new(
        url: String,
        kind: HostKind,
        fingerprint: String,
        keys: Vec<KeyChoice>,
        confirmed: bool,
        preset_key: Option<String>,
    ) -> Login {
        Login {
            url,
            kind,
            fingerprint,
            keys,
            preset_key,
            step: if confirmed {
                Step::Token
            } else {
                Step::Fingerprint
            },
            token: String::new(),
            key_index: 0,
            outcome: None,
        }
    }

    fn finish(&mut self, key_fingerprint: Option<String>) {
        self.outcome = Some(key_fingerprint.map(|k| LoginAnswer {
            token: self.token.clone(),
            key_fingerprint: k,
        }));
    }

    fn after_token(&mut self) {
        if let Some(k) = self.preset_key.clone() {
            self.finish(Some(k));
        } else if self.keys.len() == 1 {
            self.finish(Some(self.keys[0].fingerprint.clone()));
        } else {
            self.step = Step::Key;
        }
    }

    pub fn paste(&mut self, text: &str) {
        if self.step == Step::Token {
            // A token pasted with its newline submits, as typed would.
            let ends = text.ends_with(['\r', '\n']);
            self.token.push_str(text.trim_end_matches(['\r', '\n']));
            if ends {
                self.after_token();
            }
        } else {
            for c in text.chars() {
                if self.outcome.is_some() || self.step == Step::Token {
                    break;
                }
                self.key(Key::Char(c));
            }
        }
    }

    pub fn key(&mut self, key: Key) {
        if self.outcome.is_some() {
            return;
        }
        if key == Key::Interrupt {
            self.finish(None);
            return;
        }
        match self.step {
            Step::Fingerprint => match key {
                Key::Char('y' | 'Y') => self.step = Step::Token,
                Key::Char('n' | 'N') | Key::Esc => self.finish(None),
                _ => {}
            },
            Step::Token => match key {
                Key::Enter => self.after_token(),
                Key::Esc => self.finish(None),
                Key::Backspace => {
                    self.token.pop();
                }
                Key::Clear => self.token.clear(),
                Key::Char(c) => self.token.push(c),
                _ => {}
            },
            Step::Key => match key {
                Key::Up | Key::Char('k') => self.key_index = self.key_index.saturating_sub(1),
                Key::Down | Key::Char('j') => {
                    self.key_index = (self.key_index + 1).min(self.keys.len().saturating_sub(1))
                }
                Key::Enter => {
                    let k = self.keys.get(self.key_index).map(|k| k.fingerprint.clone());
                    self.finish(k);
                }
                Key::Esc => self.finish(None),
                _ => {}
            },
        }
    }

    pub fn draw(&self, area: Rect, buf: &mut Buffer, theme: Theme) {
        let mut p = Painter { buf, theme };
        p.fill(area, C::Ground);
        let (logo_w, _) = p.logo(area.x, area.y + 1, LogoSize::Large);
        let text_cols = area.width.saturating_sub(40).max(30) as usize;
        let groups: Vec<String> = group_fingerprint(&self.fingerprint)
            .split(' ')
            .map(str::to_string)
            .collect();
        let mut lines: Vec<Line> = Vec::new();
        let pad = |spans: Vec<Span>| {
            let mut all = vec![Span::raw("   ")];
            all.extend(spans);
            Line::new(all, C::Ground)
        };
        lines.push(pad(vec![
            Span::new("LosOS", C::Ink).bold(),
            Span::new(format!("  {}", self.kind.word()), C::Faint),
        ]));
        lines.push(pad(vec![Span::new(self.url.clone(), C::Muted)]));
        lines.push(pad(vec![]));
        lines.push(pad(
            vec![Span::new(t("tui.login.hostKey"), C::Muted).bold()],
        ));
        lines.push(pad(vec![Span::new(
            groups[..groups.len().min(5)].join(" "),
            C::Accent,
        )
        .bold()]));
        lines.push(pad(vec![Span::new(
            groups.get(5..).map(|g| g.join(" ")).unwrap_or_default(),
            C::Accent,
        )
        .bold()]));
        lines.push(pad(vec![]));
        let question = t(match self.kind {
            HostKind::Edge => "tui.login.compareEdge",
            HostKind::Box => "tui.login.compareBox",
        });
        for text in wrap(&question, text_cols) {
            lines.push(pad(vec![Span::new(text, C::Ink)]));
        }
        lines.push(pad(vec![]));
        match self.step {
            Step::Fingerprint => lines.push(pad(vec![
                Span::new(" y ", C::Ground).on(C::Accent).bold(),
                Span::new(format!(" {}   ", t("tui.login.matches")), C::Ink),
                Span::new(" n ", C::Ink).on(C::Sunk).bold(),
                Span::new(format!(" {}", t("tui.login.differs")), C::Muted),
            ])),
            Step::Token => {
                lines.push(pad(vec![Span::new(t("tui.login.token"), C::Ink).bold()]));
                let dots = "•".repeat(self.token.chars().count());
                let fill = 40usize.saturating_sub(dots.chars().count()).max(1);
                lines.push(pad(vec![
                    Span::new(format!(" {dots}"), C::Ink).on(C::Sunk),
                    Span::new("▏", C::Accent).on(C::Sunk),
                    Span::raw(" ".repeat(fill)).on(C::Sunk),
                ]));
                lines.push(pad(vec![Span::new(t("tui.login.tokenHint"), C::Faint)]));
            }
            Step::Key => {
                lines.push(pad(vec![Span::new(t("tui.login.whichKey"), C::Ink).bold()]));
                for (i, k) in self.keys.iter().enumerate() {
                    let here = i == self.key_index;
                    let grouped = group_fingerprint(&k.fingerprint);
                    let tail: String = grouped
                        .chars()
                        .skip(grouped.chars().count().saturating_sub(24))
                        .collect();
                    lines.push(Line::new(
                        vec![
                            Span::new(if here { " › " } else { "   " }, C::Accent).bold(),
                            Span::new(tail, if here { C::Accent } else { C::Ink }),
                            Span::new(
                                format!("  {}", k.user_ids.first().cloned().unwrap_or_default()),
                                C::Muted,
                            ),
                        ],
                        if here { C::AccentWash } else { C::Ground },
                    ));
                }
            }
        }
        let x = area.x + logo_w;
        for (i, line) in lines.iter().enumerate() {
            p.line(x, area.y + 1 + i as u16, (text_cols + 4) as u16, line);
        }
    }
}

/// Which pinned host to open.
pub struct Picker {
    pub hosts: Vec<HostRecord>,
    pub index: usize,
    pub outcome: Option<Option<usize>>,
}

impl Picker {
    pub fn new(hosts: Vec<HostRecord>) -> Picker {
        Picker {
            hosts,
            index: 0,
            outcome: None,
        }
    }

    pub fn key(&mut self, key: Key) {
        match key {
            Key::Up | Key::Char('k') => self.index = self.index.saturating_sub(1),
            Key::Down | Key::Char('j') => {
                self.index = (self.index + 1).min(self.hosts.len().saturating_sub(1))
            }
            Key::Enter => self.outcome = Some(Some(self.index)),
            Key::Char('q') | Key::Esc | Key::Interrupt => self.outcome = Some(None),
            _ => {}
        }
    }

    pub fn draw(&self, area: Rect, buf: &mut Buffer, theme: Theme) {
        let mut p = Painter { buf, theme };
        p.fill(area, C::Ground);
        let (logo_w, _) = p.logo(area.x, area.y + 1, LogoSize::Small);
        let w = area.width.saturating_sub(22).max(40);
        let mut lines = vec![
            Line::blank(),
            Line::new(vec![Span::new("  LosOS", C::Ink).bold()], C::Ground),
            Line::new(
                vec![Span::new(format!("  {}", t("tui.pick.title")), C::Muted)],
                C::Ground,
            ),
            Line::blank(),
        ];
        for (i, h) in self.hosts.iter().enumerate() {
            let here = i == self.index;
            let mut url = Span::new(h.url.clone(), if here { C::Accent } else { C::Ink });
            url.bold = here;
            lines.push(Line::new(
                vec![
                    Span::new(if here { " › " } else { "   " }, C::Accent).bold(),
                    url,
                    Span::new(format!("  {}", h.kind.word()), C::Faint),
                ],
                if here { C::AccentWash } else { C::Ground },
            ));
        }
        let x = area.x + logo_w;
        for (i, line) in lines.iter().enumerate() {
            p.line(x, area.y + i as u16, w, line);
        }
    }
}
