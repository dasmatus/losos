//! The terminal: the alternate screen in raw mode for as long as a screen
//! runs, restored on the way out whatever happens, a panic included.

use std::io::{stdout, Stdout};
use std::time::{Duration, Instant};

use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::layout::Rect;
use ratatui::Terminal;

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

struct Guard;

impl Guard {
    fn enter() -> Result<Guard> {
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

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = execute!(stdout(), DisableBracketedPaste, LeaveAlternateScreen);
        let _ = disable_raw_mode();
        crate::logging::hold(false);
    }
}

/// Run `screen` until it is done.
pub fn run(screen: &mut dyn Screen, theme: Theme) -> Result<()> {
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
