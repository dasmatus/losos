//! The web UI's palette (admin-ui/app/src/styles/tokens.css, read at build
//! time) in the terminal: truecolor where the terminal says it has it, the
//! nearest of the 256 xterm colours otherwise. And the salmon, drawn with
//! half blocks.
//!
//! Dark or light: `--theme`, else `COLORFGBG` (the background's index, which
//! rxvt, Konsole and others set), else dark.

use ratatui::style::Color;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Colour {
    pub(crate) rgb: (u8, u8, u8),
    pub(crate) ansi256: u8,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Palette {
    pub(crate) ground: Colour,
    pub(crate) surface: Colour,
    pub(crate) sunk: Colour,
    pub(crate) ink: Colour,
    pub(crate) muted: Colour,
    pub(crate) faint: Colour,
    pub(crate) line: Colour,
    pub(crate) hair: Colour,
    pub(crate) accent: Colour,
    pub(crate) accent_wash: Colour,
    pub(crate) ok: Colour,
    pub(crate) warn: Colour,
    pub(crate) crit: Colour,
}

mod generated {
    use super::{Colour, Palette};
    include!(concat!(env!("OUT_DIR"), "/palette.rs"));
}

mod logo {
    /// Red, green, blue, alpha (0-255), or nothing where the fish is not.
    pub(crate) type Pixel = Option<[u8; 4]>;
    include!(concat!(env!("OUT_DIR"), "/logo.rs"));
}

/// The colour tokens, by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum C {
    Ground,
    Surface,
    Sunk,
    Ink,
    Muted,
    Faint,
    Line,
    Hair,
    Accent,
    AccentWash,
    Ok,
    Warn,
    Crit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub mode: Mode,
    pub truecolor: bool,
}

pub fn detect_mode(flag: Option<&str>, env: &dyn Fn(&str) -> Option<String>) -> Mode {
    match flag {
        Some("dark") => return Mode::Dark,
        Some("light") => return Mode::Light,
        _ => {}
    }
    if let Some(fgbg) = env("COLORFGBG").filter(|v| !v.is_empty()) {
        if let Some(Ok(bg)) = fgbg.rsplit(';').next().map(str::parse::<u32>) {
            return if bg == 7 || bg >= 9 {
                Mode::Light
            } else {
                Mode::Dark
            };
        }
    }
    Mode::Dark
}

pub fn detect_truecolor(env: &dyn Fn(&str) -> Option<String>) -> bool {
    let colorterm = env("COLORTERM").unwrap_or_default().to_lowercase();
    if colorterm == "truecolor" || colorterm == "24bit" {
        return true;
    }
    let term = env("TERM").unwrap_or_default();
    term.ends_with("-direct")
        || term.contains("truecolor")
        || term.contains("24bit")
        || env("TERM_PROGRAM").as_deref() == Some("iTerm.app")
        || env("WT_SESSION").is_some()
}

impl Theme {
    pub fn new(mode: Mode, truecolor: bool) -> Theme {
        Theme { mode, truecolor }
    }

    fn palette(&self) -> &'static Palette {
        match self.mode {
            Mode::Dark => &generated::DARK,
            Mode::Light => &generated::LIGHT,
        }
    }

    fn token(&self, c: C) -> Colour {
        let p = self.palette();
        match c {
            C::Ground => p.ground,
            C::Surface => p.surface,
            C::Sunk => p.sunk,
            C::Ink => p.ink,
            C::Muted => p.muted,
            C::Faint => p.faint,
            C::Line => p.line,
            C::Hair => p.hair,
            C::Accent => p.accent,
            C::AccentWash => p.accent_wash,
            C::Ok => p.ok,
            C::Warn => p.warn,
            C::Crit => p.crit,
        }
    }

    /// A token as the terminal draws it.
    pub fn c(&self, c: C) -> Color {
        let colour = self.token(c);
        if self.truecolor {
            let (r, g, b) = colour.rgb;
            Color::Rgb(r, g, b)
        } else {
            Color::Indexed(colour.ansi256)
        }
    }

    /// Any colour as the terminal draws it.
    pub fn rgb(&self, (r, g, b): (u8, u8, u8)) -> Color {
        if self.truecolor {
            Color::Rgb(r, g, b)
        } else {
            Color::Indexed(nearest256(r, g, b))
        }
    }

    pub fn ground_rgb(&self) -> (u8, u8, u8) {
        self.token(C::Ground).rgb
    }
}

/// The nearest xterm-256 index of any colour, for the logo's pixels.
pub fn nearest256(r: u8, g: u8, b: u8) -> u8 {
    const LEVELS: [i32; 6] = [0, 95, 135, 175, 215, 255];
    let step = |v: i32| -> usize {
        (0..LEVELS.len())
            .min_by_key(|&i| (LEVELS[i] - v).abs())
            .unwrap_or(0)
    };
    let (r, g, b) = (r as i32, g as i32, b as i32);
    let (ri, gi, bi) = (step(r), step(g), step(b));
    let cube = 16 + 36 * ri + 6 * gi + bi;
    let cube_rgb = [LEVELS[ri], LEVELS[gi], LEVELS[bi]];
    let grey = (((r + g + b) as f64 / 3.0 - 8.0) / 10.0)
        .round()
        .clamp(0.0, 23.0) as i32;
    let grey_v = 8 + grey * 10;
    let d = |x: [i32; 3]| (r - x[0]).pow(2) + (g - x[1]).pow(2) + (b - x[2]).pow(2);
    if d([grey_v, grey_v, grey_v]) < d(cube_rgb) {
        (232 + grey) as u8
    } else {
        cube as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoSize {
    Small,
    Large,
}

/// One cell of the logo: a half block with the upper pixel as its colour
/// and the lower one as its background, or a blank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogoCell {
    pub ch: char,
    pub fg: Option<(u8, u8, u8)>,
    pub bg: Option<(u8, u8, u8)>,
}

/// The salmon, two pixels per cell. Edge pixels are blended onto the page's
/// ground colour, so the outline is soft on either theme.
pub fn logo_cells(size: LogoSize, ground: (u8, u8, u8)) -> Vec<Vec<LogoCell>> {
    let pixels = match size {
        LogoSize::Small => logo::LOGO_SMALL,
        LogoSize::Large => logo::LOGO_LARGE,
    };
    let blend = |p: logo::Pixel| -> Option<(u8, u8, u8)> {
        let [r, g, b, a] = p?;
        if a < 40 {
            return None;
        }
        let a = a as f64 / 255.0;
        let mix = |c: u8, gr: u8| (c as f64 * a + gr as f64 * (1.0 - a)).round() as u8;
        Some((mix(r, ground.0), mix(g, ground.1), mix(b, ground.2)))
    };
    let mut rows = Vec::new();
    for pair in pixels.chunks(2) {
        let top = pair[0];
        let bottom = pair.get(1).copied().unwrap_or(&[]);
        let row = (0..top.len())
            .map(|x| {
                let up = blend(top[x]);
                let down = bottom.get(x).copied().and_then(blend);
                match (up, down) {
                    (None, None) => LogoCell {
                        ch: ' ',
                        fg: None,
                        bg: None,
                    },
                    (Some(u), None) => LogoCell {
                        ch: '▀',
                        fg: Some(u),
                        bg: None,
                    },
                    (None, Some(d)) => LogoCell {
                        ch: '▄',
                        fg: Some(d),
                        bg: None,
                    },
                    (Some(u), Some(d)) => LogoCell {
                        ch: '▀',
                        fg: Some(u),
                        bg: Some(d),
                    },
                }
            })
            .collect();
        rows.push(row);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mode_comes_from_the_flag_then_colorfgbg() {
        let env = |v: &'static str| move |n: &str| (n == "COLORFGBG").then(|| v.to_string());
        assert_eq!(detect_mode(Some("light"), &env("15;0")), Mode::Light);
        assert_eq!(detect_mode(None, &env("0;15")), Mode::Light);
        assert_eq!(detect_mode(None, &env("15;0")), Mode::Dark);
        assert_eq!(detect_mode(None, &|_| None), Mode::Dark);
    }

    #[test]
    fn the_logo_is_a_fish_and_not_a_blank() {
        let small = logo_cells(LogoSize::Small, (0, 0, 0));
        assert_eq!(small[0].len(), 16);
        let inked = small.iter().flatten().filter(|c| c.ch != ' ').count();
        assert!(inked > 20, "{inked} cells");
        assert_eq!(logo_cells(LogoSize::Large, (0, 0, 0))[0].len(), 34);
    }

    #[test]
    fn nearest_256() {
        assert_eq!(nearest256(0, 0, 0), 16);
        assert_eq!(nearest256(255, 255, 255), 231);
        assert_eq!(nearest256(128, 128, 128), 244);
    }
}
