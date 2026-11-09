//! The terminal: the alternate screen in raw mode for as long as a screen
//! runs, restored on the way out whatever happens, a panic included.
//!
//! Under wasm32 the run loop has no TTY to take (the browser build of this
//! crate is the API/drive surface, README's wasm paragraph, not the TTY),
//! so `run` keeps its name and answers with the needsTerminal problem the
//! commands' own non-TTY arms use. Everything else here — the `Screen`
//! trait, the screens that satisfy it, the key translation — is buffer and
//! event maths that compiles under wasm32 untouched: the pages' pane port
//! (nix/packages.nix, losos-tui-wasm) hangs off them.

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::app::{App, Key};
use super::login::{Login, Picker};
use crate::problem::{Problem, Result};
use crate::theme::Theme;

/// A screen the loop below can run.
pub trait Screen {
    fn draw(&mut self, area: Rect, buf: &mut Buffer, theme: Theme);
    fn key(&mut self, key: Key);
    fn paste(&mut self, text: &str);
    fn done(&self) -> bool;
    /// Background work and the clock, between frames.
    fn tick(&mut self) {}
}

impl Screen for App {
    fn draw(&mut self, area: Rect, buf: &mut Buffer, theme: Theme) {
        super::draw::draw(self, area, buf, theme);
    }
    fn key(&mut self, key: Key) {
        App::key(self, key);
    }
    fn paste(&mut self, text: &str) {
        App::paste(self, text);
    }
    fn done(&self) -> bool {
        self.quit
    }
    fn tick(&mut self) {
        self.drain();
        App::tick(self, Instant::now());
    }
}

impl Screen for Login {
    fn draw(&mut self, area: Rect, buf: &mut Buffer, theme: Theme) {
        Login::draw(self, area, buf, theme);
    }
    fn key(&mut self, key: Key) {
        Login::key(self, key);
    }
    fn paste(&mut self, text: &str) {
        Login::paste(self, text);
    }
    fn done(&self) -> bool {
        self.outcome.is_some()
    }
}

impl Screen for Picker {
    fn draw(&mut self, area: Rect, buf: &mut Buffer, theme: Theme) {
        Picker::draw(self, area, buf, theme);
    }
    fn key(&mut self, key: Key) {
        Picker::key(self, key);
    }
    fn paste(&mut self, _text: &str) {}
    fn done(&self) -> bool {
        self.outcome.is_some()
    }
}

/// crossterm's key, as the screens take it.
pub fn key_of(event: KeyEvent) -> Option<Key> {
    if event.kind == KeyEventKind::Release {
        return None;
    }
    let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
    Some(match event.code {
        KeyCode::Char('c') if ctrl => Key::Interrupt,
        KeyCode::Char('u') if ctrl => Key::Clear,
        KeyCode::Char(_) if ctrl || event.modifiers.contains(KeyModifiers::ALT) => return None,
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Esc,
        KeyCode::Tab | KeyCode::BackTab => Key::Tab,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Backspace | KeyCode::Delete => Key::Backspace,
        _ => return None,
    })
}

/// The alternated screen, in raw mode, restored on the way out whatever
/// happens. A wasm build neither starts nor calls into it.
#[cfg(not(target_arch = "wasm32"))]
struct Guard;

#[cfg(not(target_arch = "wasm32"))]
impl Guard {
    fn enter() -> Result<Guard> {
        use std::io::stdout;

        use ratatui::crossterm::event::EnableBracketedPaste;
        use ratatui::crossterm::execute;
        use ratatui::crossterm::terminal::{Clear, ClearType, EnterAlternateScreen, enable_raw_mode};

        let fail = |e: std::io::Error| Problem::new("losos::terminal", e.to_string());
        enable_raw_mode().map_err(fail)?;
        execute!(
            stdout(),
            EnterAlternateScreen,
            EnableBracketedPaste,
            Clear(ClearType::All)
        )
        .map_err(fail)?;
        crate::logging::hold(true);
        Ok(Guard)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for Guard {
    fn drop(&mut self) {
        use std::io::stdout;

        use ratatui::crossterm::event::DisableBracketedPaste;
        use ratatui::crossterm::execute;
        use ratatui::crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};

        let _ = execute!(stdout(), DisableBracketedPaste, LeaveAlternateScreen);
        let _ = disable_raw_mode();
        crate::logging::hold(false);
    }
}

/// Run `screen` until it is done.
#[cfg(not(target_arch = "wasm32"))]
pub fn run(screen: &mut dyn Screen, theme: Theme) -> Result<()> {
    use std::io::{stdout, Stdout};

    use ratatui::backend::CrosstermBackend;
    use ratatui::crossterm::event::{self, DisableBracketedPaste, Event};
    use ratatui::crossterm::execute;
    use ratatui::crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};
    use ratatui::Terminal;

    let fail = |e: std::io::Error| Problem::new("losos::terminal", e.to_string());
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(stdout(), DisableBracketedPaste, LeaveAlternateScreen);
        let _ = disable_raw_mode();
        crate::logging::hold(false);
        previous(info);
    }));
    let _guard = Guard::enter()?;
    let mut terminal: Terminal<CrosstermBackend<Stdout>> =
        Terminal::new(CrosstermBackend::new(stdout())).map_err(fail)?;
    // Not `terminal.clear()`: it asks the terminal where the cursor is,
    // and a terminal that does not answer stops the screen from opening.
    // The alternate screen was cleared on the way in.
    while !screen.done() {
        terminal
            .draw(|frame| {
                let area = frame.area();
                screen.draw(area, frame.buffer_mut(), theme);
            })
            .map_err(fail)?;
        // Take every event that is already waiting before drawing again:
        // keys typed or pasted faster than a frame are each handled in turn.
        let mut wait = Duration::from_millis(100);
        while event::poll(wait).map_err(fail)? {
            wait = Duration::ZERO;
            match event::read().map_err(fail)? {
                Event::Key(k) => {
                    if let Some(key) = key_of(k) {
                        screen.key(key);
                    }
                }
                Event::Paste(text) => screen.paste(&text),
                _ => {}
            }
            if screen.done() {
                break;
            }
        }
        screen.tick();
    }
    Ok(())
}

/// Run `screen` until it is done — under wasm32, where the browser build
/// carries the panes' reads and no terminal exists, this keeps the callers
/// compiling and answers with the message the non-TTY command arms get
/// (`io.terminal()` has already refused them there; the browser page does
/// not crash into this path). The substitute is not a lie: a full-screen
/// view genuinely needs a TTY, and README's wasm paragraph documents that
/// the wasm build is the API/drive surface, not the TTY.
#[cfg(target_arch = "wasm32")]
pub fn run(_screen: &mut dyn Screen, _theme: Theme) -> Result<()> {
    Err(Problem::new(
        "losos::terminal",
        crate::tr!("tui.open.needsTerminal"),
    ))
}
