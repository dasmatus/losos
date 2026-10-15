//! Reads the web UI's own sources at build time, so the terminal client
//! cannot drift from them. Writes four files into `OUT_DIR`:
//!
//!   web_messages.rs   the web catalogue (admin-ui/app/src/i18n/*.ts), every
//!                     key in en, sk and de, as `WEB`
//!   palette.rs        the colour tokens of admin-ui/app/src/styles/tokens.css,
//!                     light and dark, as RGB and the nearest xterm-256 index
//!   logo.rs           the salmon (admin-ui/themes/brand/salmon-128.png) cut to
//!                     its outline and scaled for half-block cells
//!   pane_keywords.rs  the English search words of each settings pane, out of
//!                     admin-ui/app/src/screens/settings/panes.ts
//!
//! Each step fails the build with the file and what it missed, rather than
//! shipping a client with a gap in it.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let here = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    // The web UI's sources sit beside this crate; LOSOS_UI_DIR points
    // elsewhere when the crate is built out of the tree.
    let ui = std::env::var("LOSOS_UI_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| here.join(".."));
    let app = ui.join("app");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    println!("cargo:rerun-if-env-changed=LOSOS_UI_DIR");

    web_messages(&app.join("src/i18n"), &out.join("web_messages.rs"));
    palette(&app.join("src/styles/tokens.css"), &out.join("palette.rs"));
    logo(
        &ui.join("themes/brand/salmon-128.png"),
        &out.join("logo.rs"),
    );
    pane_keywords(
        &app.join("src/screens/settings/panes.ts"),
        &out.join("pane_keywords.rs"),
    );
}

fn read(path: &Path) -> String {
    println!("cargo:rerun-if-changed={}", path.display());
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

// ── The web catalogue ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Str(String),
    Ident(String),
    Punct(char),
}

/// The few tokens a catalogue module is made of: double-quoted strings with
/// JavaScript's escapes, identifiers, punctuation. Comments are dropped.
fn tokenize(src: &str, file: &Path) -> Vec<Token> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else if c == '"' {
            i += 1;
            let mut s = String::new();
            loop {
                let Some(&c) = chars.get(i) else {
                    panic!("{}: a string runs off the end", file.display())
                };
                i += 1;
                match c {
                    '"' => break,
                    '\\' => {
                        let e = chars[i];
                        i += 1;
                        match e {
                            'n' => s.push('\n'),
                            't' => s.push('\t'),
                            'u' => {
                                let hex: String = chars[i..i + 4].iter().collect();
                                i += 4;
                                let n = u32::from_str_radix(&hex, 16).unwrap();
                                s.push(char::from_u32(n).unwrap());
                            }
                            other => s.push(other),
                        }
                    }
                    c => s.push(c),
                }
            }
            out.push(Token::Str(s));
        } else if c.is_alphanumeric() || c == '_' || c == '$' {
            let start = i;
            while i < chars.len()
                && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '$')
            {
                i += 1;
            }
            out.push(Token::Ident(chars[start..i].iter().collect()));
        } else {
            out.push(Token::Punct(c));
            i += 1;
        }
    }
    out
}

#[derive(Debug)]
enum Text {
    One(String),
    Plural(Vec<(String, String)>),
}

struct Parser<'a> {
    tokens: &'a [Token],
    at: usize,
    file: &'a Path,
}

impl Parser<'_> {
    fn next(&mut self) -> Token {
        let t = self
            .tokens
            .get(self.at)
            .cloned()
            .unwrap_or_else(|| panic!("{}: ends inside defineMessages", self.file.display()));
        self.at += 1;
        t
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn expect(&mut self, c: char) {
        let t = self.next();
        assert!(
            t == Token::Punct(c),
            "{}: expected {c:?}, found {t:?}",
            self.file.display()
        );
    }

    fn comma(&mut self) {
        if self.peek() == Some(&Token::Punct(',')) {
            self.at += 1;
        }
    }

    fn name(&mut self) -> String {
        match self.next() {
            Token::Ident(s) | Token::Str(s) => s,
            t => panic!("{}: expected a name, found {t:?}", self.file.display()),
        }
    }

    fn text(&mut self) -> Text {
        match self.next() {
            Token::Str(s) => Text::One(s),
            Token::Punct('{') => {
                let mut forms = Vec::new();
                while self.peek() != Some(&Token::Punct('}')) {
                    let form = self.name();
                    self.expect(':');
                    let Token::Str(s) = self.next() else {
                        panic!(
                            "{}: a plural form that is not a string",
                            self.file.display()
                        )
                    };
                    forms.push((form, s));
                    self.comma();
                }
                self.expect('}');
                assert!(
                    forms.iter().any(|(f, _)| f == "other"),
                    "{}: a plural without other",
                    self.file.display()
                );
                Text::Plural(forms)
            }
            t => panic!("{}: expected a text, found {t:?}", self.file.display()),
        }
    }
}

fn web_messages(dir: &Path, out: &Path) {
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "ts"))
        .collect();
    files.sort();
    let mut all: BTreeMap<String, [Text; 3]> = BTreeMap::new();
    for file in &files {
        let src = read(file);
        let tokens = tokenize(&src, file);
        // The call, not the import: `defineMessages({`. define.ts declares
        // the function and has no call.
        let Some(start) = (0..tokens.len()).find(|&i| {
            tokens[i] == Token::Ident("defineMessages".into())
                && tokens.get(i + 1) == Some(&Token::Punct('('))
                && tokens.get(i + 2) == Some(&Token::Punct('{'))
        }) else {
            continue;
        };
        let mut p = Parser {
            tokens: &tokens,
            at: start + 3,
            file,
        };
        while p.peek() != Some(&Token::Punct('}')) {
            let key = match p.next() {
                Token::Str(s) => s,
                t => panic!("{}: expected a quoted key, found {t:?}", file.display()),
            };
            p.expect(':');
            p.expect('{');
            let mut langs: BTreeMap<String, Text> = BTreeMap::new();
            while p.peek() != Some(&Token::Punct('}')) {
                let lang = p.name();
                p.expect(':');
                langs.insert(lang, p.text());
                p.comma();
            }
            p.expect('}');
            p.comma();
            let mut take = |l: &str| {
                langs
                    .remove(l)
                    .unwrap_or_else(|| panic!("{}: {key} has no {l}", file.display()))
            };
            let entry = [take("en"), take("sk"), take("de")];
            assert!(
                all.insert(key.clone(), entry).is_none(),
                "{}: {key} is defined twice",
                file.display()
            );
        }
    }
    assert!(
        all.len() > 500,
        "{}: only {} keys",
        dir.display(),
        all.len()
    );

    let text = |t: &Text| match t {
        Text::One(s) => format!("Text::One({s:?})"),
        Text::Plural(forms) => {
            let forms: Vec<String> = forms
                .iter()
                .map(|(f, s)| format!("({f:?}, {s:?})"))
                .collect();
            format!("Text::Plural(&[{}])", forms.join(", "))
        }
    };
    let mut code = String::from("pub(crate) static WEB: &[(&str, Entry)] = &[\n");
    for (key, [en, sk, de]) in &all {
        writeln!(
            code,
            "    ({key:?}, Entry {{ en: {}, sk: {}, de: {} }}),",
            text(en),
            text(sk),
            text(de)
        )
        .unwrap();
    }
    code.push_str("];\n");
    fs::write(out, code).unwrap();
}

// ── The palette ──────────────────────────────────────────────────────────

const NAMES: [&str; 13] = [
    "ground",
    "surface",
    "sunk",
    "ink",
    "muted",
    "faint",
    "line",
    "hair",
    "accent",
    "accent-wash",
    "ok",
    "warn",
    "crit",
];

fn css_block<'a>(css: &'a str, selector: &str, file: &Path) -> &'a str {
    let at = css
        .find(selector)
        .unwrap_or_else(|| panic!("{}: no block starting {selector:?}", file.display()));
    let open = at + css[at..].find('{').unwrap();
    let close = open + css[open..].find('}').unwrap();
    &css[open + 1..close]
}

fn colours(body: &str) -> BTreeMap<String, [u8; 3]> {
    let mut found = BTreeMap::new();
    for line in body.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("--") else {
            continue;
        };
        let Some((name, value)) = rest.split_once(':') else {
            continue;
        };
        let value = value.trim().trim_end_matches(';').trim();
        if value.len() == 7 && value.starts_with('#') {
            let hex = |i: usize| u8::from_str_radix(&value[i..i + 2], 16).ok();
            if let (Some(r), Some(g), Some(b)) = (hex(1), hex(3), hex(5)) {
                found.insert(name.trim().to_string(), [r, g, b]);
            }
        }
    }
    found
}

/// The xterm-256 colour nearest to `rgb`, weighted for the eye (green counts
/// most, blue least). The sixteen system colours are skipped: terminals
/// theme them.
fn nearest256([r, g, b]: [u8; 3]) -> u8 {
    const LEVELS: [i32; 6] = [0, 95, 135, 175, 215, 255];
    let mut best = (16u8, i32::MAX);
    for index in 16u16..256 {
        let (cr, cg, cb) = if index < 232 {
            let n = (index - 16) as usize;
            (LEVELS[n / 36], LEVELS[(n / 6) % 6], LEVELS[n % 6])
        } else {
            let v = 8 + (index as i32 - 232) * 10;
            (v, v, v)
        };
        let (r, g, b) = (r as i32, g as i32, b as i32);
        let d = 2 * (r - cr).pow(2) + 4 * (g - cg).pow(2) + 3 * (b - cb).pow(2);
        if d < best.1 {
            best = (index as u8, d);
        }
    }
    best.0
}

fn palette(file: &Path, out: &Path) {
    let css = read(file);
    let light = colours(css_block(&css, ":root,\n[data-theme-light]", file));
    let dark = colours(css_block(&css, ":root[data-theme=\"dark\"]", file));
    let mut code = String::new();
    for (mode, set) in [("LIGHT", &light), ("DARK", &dark)] {
        writeln!(code, "pub(crate) const {mode}: Palette = Palette {{").unwrap();
        for name in NAMES {
            let rgb = set
                .get(name)
                .unwrap_or_else(|| panic!("{}: {mode} has no --{name}", file.display()));
            let field = name.replace('-', "_");
            writeln!(
                code,
                "    {field}: Colour {{ rgb: ({}, {}, {}), ansi256: {} }},",
                rgb[0],
                rgb[1],
                rgb[2],
                nearest256(*rgb)
            )
            .unwrap();
        }
        code.push_str("};\n");
    }
    fs::write(out, code).unwrap();
}

// ── The logo ─────────────────────────────────────────────────────────────

fn logo(file: &Path, out: &Path) {
    println!("cargo:rerun-if-changed={}", file.display());
    let decoder = png::Decoder::new(fs::File::open(file).unwrap());
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!(
        (info.color_type, info.bit_depth),
        (png::ColorType::Rgba, png::BitDepth::Eight),
        "{}: expected 8-bit RGBA",
        file.display()
    );
    let (w, h) = (info.width as usize, info.height as usize);
    let at = |x: usize, y: usize| -> [f64; 4] {
        let i = (y * w + x) * 4;
        [
            buf[i] as f64,
            buf[i + 1] as f64,
            buf[i + 2] as f64,
            buf[i + 3] as f64,
        ]
    };
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if at(x, y)[3] > 24.0 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    let box_w = (max_x - min_x + 1) as f64;
    let box_h = (max_y - min_y + 1) as f64;

    // Area-averaged downscale with premultiplied alpha, so the fish's edge
    // does not pick up the black of its transparent surroundings.
    let scale = |cols: usize| -> Vec<Vec<Option<[u8; 4]>>> {
        let mut rows = ((cols as f64 * box_h) / box_w).round() as usize;
        if rows % 2 == 1 {
            rows += 1;
        }
        let mut pixels = Vec::new();
        for oy in 0..rows {
            let mut row = Vec::new();
            for ox in 0..cols {
                let x0 = min_x as f64 + (ox as f64 * box_w) / cols as f64;
                let x1 = min_x as f64 + ((ox + 1) as f64 * box_w) / cols as f64;
                let y0 = min_y as f64 + (oy as f64 * box_h) / rows as f64;
                let y1 = min_y as f64 + ((oy + 1) as f64 * box_h) / rows as f64;
                let (mut r, mut g, mut b, mut a, mut weight) = (0.0, 0.0, 0.0, 0.0, 0.0);
                for y in (y0.floor() as usize)..(y1.ceil() as usize) {
                    for x in (x0.floor() as usize)..(x1.ceil() as usize) {
                        let wx = (x as f64 + 1.0).min(x1) - (x as f64).max(x0);
                        let wy = (y as f64 + 1.0).min(y1) - (y as f64).max(y0);
                        let wt = wx.max(0.0) * wy.max(0.0);
                        if wt == 0.0 {
                            continue;
                        }
                        let p = at(x.min(w - 1), y.min(h - 1));
                        let alpha = p[3] / 255.0;
                        r += p[0] * alpha * wt;
                        g += p[1] * alpha * wt;
                        b += p[2] * alpha * wt;
                        a += alpha * wt;
                        weight += wt;
                    }
                }
                row.push((a > 0.0).then(|| {
                    [
                        (r / a).round() as u8,
                        (g / a).round() as u8,
                        (b / a).round() as u8,
                        ((255.0 * a) / weight).round() as u8,
                    ]
                }));
            }
            pixels.push(row);
        }
        pixels
    };
    let mut code = String::new();
    for (name, cols) in [("LOGO_SMALL", 16), ("LOGO_LARGE", 34)] {
        let pixels = scale(cols);
        writeln!(code, "pub(crate) const {name}: &[&[Pixel]] = &[").unwrap();
        for row in pixels {
            code.push_str("    &[");
            for p in row {
                match p {
                    Some([r, g, b, a]) => write!(code, "Some([{r}, {g}, {b}, {a}]), ").unwrap(),
                    None => code.push_str("None, "),
                }
            }
            code.push_str("],\n");
        }
        code.push_str("];\n");
    }
    fs::write(out, code).unwrap();
}

// ── Pane keywords ────────────────────────────────────────────────────────

fn pane_keywords(file: &Path, out: &Path) {
    let src = read(file);
    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut rest = src.as_str();
    // pane("storage", Icon, ["disk", "space", ...], ...)
    while let Some(at) = rest.find("pane(") {
        rest = &rest[at + 5..];
        let call = rest.trim_start();
        let Some(call) = call.strip_prefix('"') else {
            continue;
        };
        let Some(end) = call.find('"') else { continue };
        let id = &call[..end];
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_lowercase()) {
            continue;
        }
        let after = &call[end + 1..];
        let Some(open) = after.find('[') else {
            continue;
        };
        // The icon sits between the id and the list; nothing else may.
        if after[..open].contains('(') || after[..open].contains('"') {
            continue;
        }
        let Some(close) = after[open..].find(']') else {
            continue;
        };
        let list = &after[open + 1..open + close];
        let words: Vec<String> = list
            .lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .flat_map(|l| l.split('"').skip(1).step_by(2).map(str::to_string))
            .collect();
        found.insert(id.to_string(), words);
    }
    for id in [
        "storage", "mesh", "apps", "network", "security", "advanced", "history", "backup", "about",
    ] {
        assert!(
            found.get(id).is_some_and(|w| !w.is_empty()),
            "{}: found no search words for the {id} pane",
            file.display()
        );
    }
    let mut code = String::from("pub(crate) const PANE_KEYWORDS: &[(&str, &[&str])] = &[\n");
    for (id, words) in &found {
        writeln!(code, "    ({id:?}, &{words:?}),").unwrap();
    }
    code.push_str("];\n");
    fs::write(out, code).unwrap();
}
