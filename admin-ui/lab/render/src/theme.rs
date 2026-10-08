//! The house palette. The page reads the CSS custom properties (tokens.css
//! plus the Lab's own cable and room colours) and hands them over with
//! `set_theme`; until it does, the canvas draws with the same values
//! compiled in, light or dark.

use crate::paint::Rgba;
use serde_json::Value;

/// The house tokens (admin-ui/app/src/styles/tokens.css) and the Lab's
/// aliases of them (admin-ui/app/src/lab/lab.css), resolved to linear RGBA.
/// The field names are the tokens' without `--` and `lab-`.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub dark: bool,
    // house tokens
    pub ground: Rgba,
    pub surface: Rgba,
    pub sunk: Rgba,
    pub ink: Rgba,
    pub muted: Rgba,
    pub faint: Rgba,
    pub line: Rgba,
    pub hair: Rgba,
    pub accent: Rgba,
    pub accent_wash: Rgba,
    pub ok: Rgba,
    pub warn: Rgba,
    pub crit: Rgba,
    // the Lab's aliases: cables, canvas, hardware
    pub copper: Rgba,
    pub fiber: Rgba,
    pub wan: Rgba,
    pub wifi: Rgba,
    pub grid: Rgba,
    pub room: Rgba,
    pub room_edge: Rgba,
    pub mark: Rgba,
    pub led_off: Rgba,
    pub kit: Rgba,
    pub kit_face: Rgba,
    pub kit_edge: Rgba,
    pub hole: Rgba,
    pub rack: Rgba,
    pub rack_edge: Rgba,
    pub rack_face: Rgba,
    pub rack_bay: Rgba,
    pub rack_ink: Rgba,
    pub screen: Rgba,
    pub screen_off: Rgba,
    pub screen_tile: Rgba,
    /// `--lab-p-<proto>` by lower-case protocol key: a packet's colour.
    /// Empty until the page passes them; packets then keep the core's.
    pub protos: Vec<(String, Rgba)>,
}

/// sRGB channel (0..1) to linear, the space vertex colours are in.
fn lin(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb(r g b / a)` or `rgb(r, g, b)` to
/// linear RGBA. Anything else is `None`.
pub fn parse(s: &str) -> Option<Rgba> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix('#') {
        let n = |i: usize, w: usize| u8::from_str_radix(h.get(i..i + w)?, 16).ok();
        let (r, g, b, a) = match h.len() {
            3 => (n(0, 1)? * 17, n(1, 1)? * 17, n(2, 1)? * 17, 255),
            6 => (n(0, 2)?, n(2, 2)?, n(4, 2)?, 255),
            8 => (n(0, 2)?, n(2, 2)?, n(4, 2)?, n(6, 2)?),
            _ => return None,
        };
        return Some(srgb8(r, g, b, a as f32 / 255.0));
    }
    if let Some(inner) = s.strip_prefix("oklch(").and_then(|x| x.strip_suffix(')')) {
        return oklch(inner);
    }
    let inner = s
        .strip_prefix("rgba(")
        .or_else(|| s.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let parts: Vec<f32> = inner
        .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .map(|p| {
            p.strip_suffix('%')
                .map(|v| v.parse::<f32>().map(|v| v / 100.0))
                .unwrap_or_else(|| p.parse::<f32>())
        })
        .collect::<Result<_, _>>()
        .ok()?;
    match parts[..] {
        [r, g, b] => Some(srgb8f(r, g, b, 1.0)),
        [r, g, b, a] => Some(srgb8f(r, g, b, a)),
        _ => None,
    }
}

/// `oklch(L C H [/ A])`, or the relative form the Lab's protocol colours
/// use, `oklch(from <colour> l c H)`, where `l`, `c`, `h` and `alpha` keep
/// the origin colour's channel. Out-of-gamut results are clipped.
fn oklch(inner: &str) -> Option<Rgba> {
    let inner = inner.trim();
    let (origin, rest) = match inner.strip_prefix("from ") {
        Some(r) => {
            let r = r.trim_start();
            // The origin is one token, or a function with its parentheses.
            let end = match r.find('(') {
                Some(i) if i < r.find(' ').unwrap_or(usize::MAX) => {
                    let mut depth = 0;
                    let mut end = r.len();
                    for (j, ch) in r.char_indices() {
                        match ch {
                            '(' => depth += 1,
                            ')' => {
                                depth -= 1;
                                if depth == 0 {
                                    end = j + 1;
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    end
                }
                _ => r.find(' ').unwrap_or(r.len()),
            };
            (Some(to_oklch(parse(&r[..end])?)), &r[end..])
        }
        None => (None, inner),
    };
    let toks: Vec<&str> = rest
        .split(|c: char| c.is_whitespace() || c == '/')
        .filter(|t| !t.is_empty())
        .collect();
    let chan = |i: usize, key: &str, scale: f32| -> Option<f32> {
        let t = *toks.get(i)?;
        if t == key {
            return origin.map(|o| match key {
                "l" => o[0],
                "c" => o[1],
                "h" => o[2],
                _ => o[3],
            });
        }
        match t.strip_suffix('%') {
            Some(v) => v.parse::<f32>().ok().map(|v| v / 100.0 * scale),
            None => t.strip_suffix("deg").unwrap_or(t).parse().ok(),
        }
    };
    let l = chan(0, "l", 1.0)?;
    let c = chan(1, "c", 0.4)?;
    let h = chan(2, "h", 1.0)?;
    let a = if toks.len() > 3 {
        chan(3, "alpha", 1.0)?
    } else {
        origin.map(|o| o[3]).unwrap_or(1.0)
    };
    let (sh, ch) = h.to_radians().sin_cos();
    let (aa, bb) = (c * ch, c * sh);
    let l_ = (l + 0.396_337_8 * aa + 0.215_803_76 * bb).powi(3);
    let m_ = (l - 0.105_561_35 * aa - 0.063_854_17 * bb).powi(3);
    let s_ = (l - 0.089_484_18 * aa - 1.291_485_5 * bb).powi(3);
    let r = 4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_;
    let g = -1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_;
    let b = -0.004_196_086 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_;
    Some([
        r.clamp(0.0, 1.0),
        g.clamp(0.0, 1.0),
        b.clamp(0.0, 1.0),
        a.clamp(0.0, 1.0),
    ])
}

/// Linear RGBA to [L, C, H (degrees), A].
fn to_oklch(c: Rgba) -> [f32; 4] {
    let l = (0.412_221_46 * c[0] + 0.536_332_55 * c[1] + 0.051_445_99 * c[2]).cbrt();
    let m = (0.211_903_5 * c[0] + 0.680_699_5 * c[1] + 0.107_396_96 * c[2]).cbrt();
    let s = (0.088_302_46 * c[0] + 0.281_718_85 * c[1] + 0.629_978_7 * c[2]).cbrt();
    let ll = 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s;
    let a = 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s;
    let b = 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s;
    [ll, a.hypot(b), b.atan2(a).to_degrees(), c[3]]
}

fn srgb8f(r: f32, g: f32, b: f32, a: f32) -> Rgba {
    [
        lin(r / 255.0),
        lin(g / 255.0),
        lin(b / 255.0),
        a.clamp(0.0, 1.0),
    ]
}

pub fn srgb8(r: u8, g: u8, b: u8, a: f32) -> Rgba {
    srgb8f(r as f32, g as f32, b as f32, a)
}

/// A compiled-in colour; the literals below are all valid.
pub fn hex(s: &str) -> Rgba {
    parse(s).unwrap_or([1.0, 0.0, 1.0, 1.0])
}

pub fn alpha(c: Rgba, a: f32) -> Rgba {
    [c[0], c[1], c[2], c[3] * a]
}

/// `color-mix(in srgb, a p%, b)` closely enough for a tint.
pub fn mix(a: Rgba, b: Rgba, p: f32) -> Rgba {
    [
        a[0] * p + b[0] * (1.0 - p),
        a[1] * p + b[1] * (1.0 - p),
        a[2] * p + b[2] * (1.0 - p),
        a[3] * p + b[3] * (1.0 - p),
    ]
}

impl Theme {
    /// tokens.css, light, with lab.css's aliases.
    pub fn light() -> Theme {
        Theme::house(false).derived()
    }

    /// tokens.css, dark, with lab.css's aliases.
    pub fn dark() -> Theme {
        Theme::house(true).derived()
    }

    fn house(dark: bool) -> Theme {
        let z = [0.0; 4];
        let mut t = Theme {
            dark,
            ground: z,
            surface: z,
            sunk: z,
            ink: z,
            muted: z,
            faint: z,
            line: z,
            hair: z,
            accent: z,
            accent_wash: z,
            ok: z,
            warn: z,
            crit: z,
            copper: z,
            fiber: z,
            wan: z,
            wifi: z,
            grid: z,
            room: z,
            room_edge: z,
            mark: z,
            led_off: z,
            kit: z,
            kit_face: z,
            kit_edge: z,
            hole: z,
            rack: z,
            rack_edge: z,
            rack_face: z,
            rack_bay: z,
            rack_ink: z,
            screen: z,
            screen_off: z,
            screen_tile: z,
            protos: Vec::new(),
        };
        let v = if dark {
            [
                "#0a0e12", "#141b22", "#10171d", "#e2e8ee", "#94a2b0", "#6b7986", "#212c36",
                "#1a242d", "#48b3c0", "#0d2a2f", "#4fa583", "#c79a3e", "#d1756c",
            ]
        } else {
            [
                "#f3f5f7", "#ffffff", "#eaedf0", "#18212a", "#5c6a77", "#8c98a4", "#dce1e6",
                "#eaeef1", "#0e6e7d", "#e2eff1", "#2e7357", "#8c6512", "#a8443c",
            ]
        };
        for (slot, c) in [
            &mut t.ground,
            &mut t.surface,
            &mut t.sunk,
            &mut t.ink,
            &mut t.muted,
            &mut t.faint,
            &mut t.line,
            &mut t.hair,
            &mut t.accent,
            &mut t.accent_wash,
            &mut t.ok,
            &mut t.warn,
            &mut t.crit,
        ]
        .into_iter()
        .zip(v)
        {
            *slot = hex(c);
        }
        t
    }

    /// lab.css: every Lab colour is a house token under another name, so
    /// they follow the house ones; the hardware has a dark arm of its own.
    fn derived(mut self) -> Theme {
        self.copper = self.accent;
        self.fiber = self.warn;
        self.wan = self.muted;
        self.wifi = self.ok;
        self.grid = self.hair;
        self.room = self.surface;
        self.room_edge = self.line;
        self.mark = self.accent;
        self.led_off = self.faint;
        self.kit_edge = self.faint;
        if self.dark {
            self.kit = self.line;
            self.kit_face = self.hair;
            self.hole = self.ground;
            self.rack = self.sunk;
            self.rack_edge = self.line;
            self.rack_face = self.ground;
            self.rack_bay = self.line;
            self.rack_ink = self.muted;
            self.screen = self.ground;
            self.screen_off = self.sunk;
            self.screen_tile = self.line;
        } else {
            self.kit = self.sunk;
            self.kit_face = self.line;
            self.hole = self.ink;
            self.rack = self.ink;
            self.rack_edge = self.ink;
            self.rack_face = self.muted;
            self.rack_bay = self.faint;
            self.rack_ink = self.line;
            self.screen = self.ink;
            self.screen_off = self.muted;
            self.screen_tile = self.muted;
        }
        self
    }

    /// A packet's colour: `--lab-p-<proto>` when the page passed it,
    /// else `--faint` for a protocol it did not name; `None` when the page
    /// passed no protocol colours at all.
    pub fn proto(&self, proto: &str) -> Option<Rgba> {
        if self.protos.is_empty() {
            return None;
        }
        let key = proto.to_ascii_lowercase();
        Some(
            self.protos
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, c)| *c)
                .unwrap_or(self.faint),
        )
    }

    /// `{dark: bool, colors: {"--ground": "#…", "lab-copper": "…", …}}`,
    /// or the React Lab's own shape, `{theme: "light"|"dark", palette:
    /// {…}}`. Keys may carry the leading `--` or not. House tokens are read
    /// first and the Lab's aliases are derived from them, so a page that
    /// passes only the house tokens still gets matching cables and
    /// hardware; a `lab-*` key then overrides its alias. Unknown keys and
    /// unparsable values are ignored.
    pub fn from_json(text: &str) -> Theme {
        let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
        let dark = v
            .get("dark")
            .and_then(Value::as_bool)
            .unwrap_or_else(|| v.get("theme").and_then(Value::as_str) == Some("dark"));
        let mut t = Theme::house(dark);
        let map = v
            .get("colors")
            .or_else(|| v.get("palette"))
            .and_then(Value::as_object);
        let entries: Vec<(&str, Rgba)> = map
            .map(|m| {
                m.iter()
                    .filter_map(|(k, val)| {
                        Some((k.trim_start_matches("--"), val.as_str().and_then(parse)?))
                    })
                    .collect()
            })
            .unwrap_or_default();
        for (k, c) in &entries {
            let slot = match *k {
                "ground" => &mut t.ground,
                "surface" => &mut t.surface,
                "sunk" => &mut t.sunk,
                "ink" => &mut t.ink,
                "muted" => &mut t.muted,
                "faint" => &mut t.faint,
                "line" => &mut t.line,
                "hair" => &mut t.hair,
                "accent" => &mut t.accent,
                "accent-wash" | "accentWash" => &mut t.accent_wash,
                "ok" => &mut t.ok,
                "warn" => &mut t.warn,
                "crit" => &mut t.crit,
                _ => continue,
            };
            *slot = *c;
        }
        let mut t = t.derived();
        for (k, c) in &entries {
            let Some(k) = k.strip_prefix("lab-") else {
                continue;
            };
            if let Some(proto) = k.strip_prefix("p-") {
                t.protos.push((proto.to_ascii_lowercase(), *c));
                continue;
            }
            let slot = match k {
                "copper" => &mut t.copper,
                "fiber" => &mut t.fiber,
                "wan" => &mut t.wan,
                "wifi" => &mut t.wifi,
                "grid" => &mut t.grid,
                "room" => &mut t.room,
                "room-edge" => &mut t.room_edge,
                "mark" => &mut t.mark,
                "led-off" => &mut t.led_off,
                "kit" => &mut t.kit,
                "kit-face" => &mut t.kit_face,
                "kit-edge" => &mut t.kit_edge,
                "hole" => &mut t.hole,
                "rack" => &mut t.rack,
                "rack-edge" => &mut t.rack_edge,
                "rack-face" => &mut t.rack_face,
                "rack-bay" => &mut t.rack_bay,
                "rack-ink" => &mut t.rack_ink,
                "screen" => &mut t.screen,
                "screen-off" => &mut t.screen_off,
                "screen-tile" => &mut t.screen_tile,
                _ => continue,
            };
            *slot = *c;
        }
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_css_colours() {
        assert_eq!(parse("#fff"), Some([1.0, 1.0, 1.0, 1.0]));
        assert_eq!(parse(" #000000 "), Some([0.0, 0.0, 0.0, 1.0]));
        let c = parse("rgb(255 255 255 / 0.5)").unwrap();
        assert_eq!(c, [1.0, 1.0, 1.0, 0.5]);
        let c = parse("rgba(0, 0, 0, 0.25)").unwrap();
        assert_eq!(c[3], 0.25);
        assert_eq!(parse("var(--x)"), None);
        assert_eq!(parse("#12"), None);
    }

    #[test]
    fn theme_takes_the_page_values() {
        let t = Theme::from_json(
            r##"{"dark":true,"colors":{"--accent":"#ff0000","bogus":"#00ff00","--ink":"nope"}}"##,
        );
        assert!(t.dark);
        assert_eq!(t.accent, [1.0, 0.0, 0.0, 1.0]);
        // The aliases follow the house token they name.
        assert_eq!(t.copper, t.accent);
        assert_eq!(t.ink, Theme::dark().ink);
        assert_eq!(Theme::from_json("not json"), Theme::light());
    }

    #[test]
    fn takes_the_react_palette() {
        let t = Theme::from_json(
            r##"{"theme":"dark","palette":{"lab-wan":"#00ff00","lab-p-dhcp":"oklch(from #0e6e7d l c 300)","ground":"#000"}}"##,
        );
        assert!(t.dark);
        assert_eq!(t.wan, [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(t.ground, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(t.rack, Theme::dark().rack);
        let dhcp = t.proto("DHCP").unwrap();
        assert_eq!(t.proto("SMTP"), Some(t.faint));
        // Same lightness as the accent, another hue: bluer than it is green.
        assert!(dhcp[2] > dhcp[1]);
        assert_eq!(Theme::light().proto("DHCP"), None);
    }

    #[test]
    fn oklch_round_trips() {
        let teal = hex("#0e6e7d");
        let back = parse("oklch(from #0e6e7d l c h)").unwrap();
        for i in 0..4 {
            assert!((teal[i] - back[i]).abs() < 1e-3, "{teal:?} {back:?}");
        }
        let w = parse("oklch(100% 0 0)").unwrap();
        assert!(w.iter().all(|v| (v - 1.0).abs() < 1e-3));
    }
}
