//! The PNGs in .github/screenshots/tui/: each screen drawn at 120x36 over
//! the test fixtures, in truecolour, written as HTML and shot in headless
//! Chromium with a monospace font. Dark and light for each.
//!
//! ```sh
//! cargo run --example screenshots                   # into .github/screenshots/tui/after
//! LOSOS_SHOTS=<dir> cargo run --example screenshots # somewhere else
//! LOSOS_CHROMIUM=<path> cargo run --example screenshots
//! ```
//!
//! Without a Chromium that starts, the HTML pages are left in the folder.

#[path = "../tests/common/fixtures.rs"]
mod fixtures;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use fixtures::{box_route, edge_route, Fixture};
use losos::i18n::{set_locale, Locale};
use losos::theme::{Mode, Theme, C};
use losos::tui::app::{App, Key};
use losos::tui::draw::draw;
use losos::tui::login::{KeyChoice, Login};
use losos::tui::panes::PaneId;
use losos::vault::{HostKind, HostRecord};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

const COLS: u16 = 120;
const ROWS: u16 = 36;
/// DejaVu Sans Mono at 15px is 9.03px wide; 18px rows; 14px and 16px of
/// padding. The window is the terminal and nothing else.
const WIDTH: u32 = 1116;
const HEIGHT: u32 = 676;

fn host(kind: HostKind) -> HostRecord {
    let (url, fingerprint) = match kind {
        HostKind::Box => (
            "https://mattbox.local",
            "3F2A9C1E0B7D44A1C2D3E4F5061728394A5B6C7D",
        ),
        HostKind::Edge => (
            "https://register.losos.dasmat.us",
            "C07A55E2918B4DF06A3C1B7E92D4F8A0613E5B29",
        ),
    };
    HostRecord {
        url: url.into(),
        kind,
        fingerprint: fingerprint.into(),
        public_key: String::new(),
        token: String::new(),
        key_fingerprint: "8E41D07CB2A95F3361C0E7AD4B19F2C805D6A3E7".into(),
        tls_pin: None,
    }
}

fn app_frame(kind: HostKind, pane: PaneId, keys: &[Key], theme: Theme) -> Buffer {
    let route = match kind {
        HostKind::Box => box_route,
        HostKind::Edge => edge_route,
    };
    let mut app = App::new(host(kind), Arc::new(Fixture::new(route)));
    app.now = Some(1_791_960_021.0);
    app.open_at(pane);
    app.settle();
    let area = Rect::new(0, 0, COLS, ROWS);
    let mut buf = Buffer::empty(area);
    draw(&mut app, area, &mut buf, theme);
    for key in keys {
        app.key(*key);
        app.settle();
        buf = Buffer::empty(area);
        draw(&mut app, area, &mut buf, theme);
    }
    buf
}

fn login_frame(theme: Theme) -> Buffer {
    let h = host(HostKind::Box);
    let login = Login::new(
        h.url,
        HostKind::Box,
        h.fingerprint,
        vec![KeyChoice {
            fingerprint: h.key_fingerprint,
            user_ids: vec!["Matus <matus@example.org>".into()],
        }],
        false,
        None,
    );
    let area = Rect::new(0, 0, COLS, ROWS);
    let mut buf = Buffer::empty(area);
    login.draw(area, &mut buf, theme);
    buf
}

fn css(c: Color, fallback: (u8, u8, u8)) -> String {
    let (r, g, b) = match c {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => fallback,
    };
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn rgb_of(theme: &Theme, c: C) -> (u8, u8, u8) {
    match theme.c(c) {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (0, 0, 0),
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The buffer as HTML. Half blocks are drawn as two-colour boxes: a font's
/// block glyphs rarely fill the line box exactly, and the salmon would show
/// seams between rows.
fn page(buf: &Buffer, theme: &Theme) -> String {
    let ground = rgb_of(theme, C::Ground);
    let ink = rgb_of(theme, C::Ink);
    let mut body = String::new();
    for y in 0..ROWS {
        body.push_str("<div class=\"l\">");
        let mut run = String::new();
        let mut style = String::new();
        let flush = |body: &mut String, run: &mut String, style: &str| {
            if !run.is_empty() {
                let _ = write!(body, "<span style=\"{style}\">{}</span>", escape(run));
                run.clear();
            }
        };
        for x in 0..COLS {
            let cell = &buf[(x, y)];
            let fg = css(cell.fg, ink);
            let bg = css(cell.bg, ground);
            let symbol = cell.symbol();
            if matches!(symbol, "▀" | "▄" | "█") {
                flush(&mut body, &mut run, &style);
                let (top, bottom) = match symbol {
                    "▀" => (&fg, &bg),
                    "▄" => (&bg, &fg),
                    _ => (&fg, &fg),
                };
                let _ = write!(
                    body,
                    "<span class=\"b\" style=\"background:linear-gradient({top} 50%,{bottom} 50%)\"></span>"
                );
                continue;
            }
            let bold = if cell.modifier.contains(Modifier::BOLD) {
                ";font-weight:700"
            } else {
                ""
            };
            let this = format!("color:{fg};background:{bg}{bold}");
            if this != style {
                flush(&mut body, &mut run, &style);
                style = this;
            }
            run.push_str(if symbol.is_empty() { " " } else { symbol });
        }
        flush(&mut body, &mut run, &style);
        body.push_str("</div>");
    }
    let ground = css(Color::Rgb(ground.0, ground.1, ground.2), ground);
    let ink = css(Color::Rgb(ink.0, ink.1, ink.2), ink);
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>\
         html,body{{margin:0;background:{ground};}}\
         #t{{display:inline-block;padding:14px 16px;background:{ground};color:{ink};\
         font-family:\"DejaVu Sans Mono\",monospace;font-size:15px;font-variant-ligatures:none;}}\
         .l{{white-space:pre;height:18px;line-height:18px;overflow:hidden;}}\
         .b{{display:inline-block;width:1ch;height:18px;vertical-align:top;}}\
         </style></head><body><div id=\"t\">{body}</div></body></html>"
    )
}

/// Headless Chromium's page is shorter than the window it is given, so the
/// window is taller than the terminal and the shot is cut to it.
fn shoot(chromium: &str, html: &Path, png: &Path) -> bool {
    let ran = Command::new(chromium)
        .args([
            "--headless",
            "--no-sandbox",
            "--disable-gpu",
            "--hide-scrollbars",
            "--force-device-scale-factor=1",
            &format!("--window-size={WIDTH},{}", HEIGHT + 200),
            &format!("--screenshot={}", png.display()),
        ])
        .arg(format!("file://{}", html.display()))
        .output()
        .is_ok_and(|o| o.status.success() && png.exists());
    ran && crop(png)
}

fn crop(path: &Path) -> bool {
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let mut reader = png::Decoder::new(file).read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels).unwrap();
    let row = info.line_size;
    let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), WIDTH, HEIGHT);
    encoder.set_color(info.color_type);
    encoder.set_depth(info.bit_depth);
    let mut writer = encoder.write_header().unwrap();
    writer
        .write_image_data(&pixels[..row * HEIGHT as usize])
        .unwrap();
    info.width == WIDTH && info.height >= HEIGHT
}

fn main() {
    set_locale(Locale::En);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = std::env::var("LOSOS_SHOTS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join(".github/screenshots/tui/after"));
    let chromium = std::env::var("LOSOS_CHROMIUM")
        .unwrap_or_else(|_| "/opt/pw-browsers/chromium-1194/chrome-linux/chrome".into());
    std::fs::create_dir_all(&out).unwrap();

    type Shot = (&'static str, Box<dyn Fn(Theme) -> Buffer>);
    let j = |n: usize| vec![Key::Char('j'); n];
    let shots: Vec<Shot> = vec![
        ("login", Box::new(login_frame)),
        (
            "overview",
            Box::new(|t| app_frame(HostKind::Box, PaneId::Overview, &[], t)),
        ),
        (
            "storage",
            Box::new(|t| app_frame(HostKind::Box, PaneId::Storage, &[], t)),
        ),
        (
            "apps",
            Box::new(|t| app_frame(HostKind::Box, PaneId::Apps, &[], t)),
        ),
        // A change waiting in the Apply bar.
        (
            "security",
            Box::new(|t| {
                app_frame(
                    HostKind::Box,
                    PaneId::Security,
                    &[Key::Char('j'), Key::Enter],
                    t,
                )
            }),
        ),
        (
            "advanced",
            Box::new(|t| app_frame(HostKind::Box, PaneId::Advanced, &[], t)),
        ),
        // Down to the opt-ins, so the tenants table is in view.
        (
            "edge-overview",
            Box::new(move |t| app_frame(HostKind::Edge, PaneId::EdgeOverview, &j(8), t)),
        ),
        (
            "edge-secrets",
            Box::new(|t| app_frame(HostKind::Edge, PaneId::EdgeSecrets, &[], t)),
        ),
    ];
    let mut shot_any = false;
    for (name, frame) in &shots {
        for (mode, word) in [(Mode::Dark, "dark"), (Mode::Light, "light")] {
            let theme = Theme::new(mode, true);
            let html = out.join(format!("{name}-{word}.html"));
            std::fs::write(&html, page(&frame(theme), &theme)).unwrap();
            let png = out.join(format!("{name}-{word}.png"));
            if shoot(&chromium, &html, &png) {
                std::fs::remove_file(&html).unwrap();
                shot_any = true;
                println!("screenshots: {}", png.display());
            } else {
                println!("screenshots: {} (no Chromium to shoot it)", html.display());
            }
        }
    }
    if !shot_any {
        eprintln!(
            "screenshots: {chromium} did not start; the HTML pages are in {}",
            out.display()
        );
    }
}
