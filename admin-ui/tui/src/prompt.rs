//! Reading from the terminal outside the full-screen view: a value typed
//! without echo, or everything piped on stdin.
//!
//! The hidden prompt is a raw-mode question (crossterm's terminal half has
//! no wasm32 TTY behind it), so under wasm32 `read_hidden` keeps its name
//! and answers with the needsTerminal problem; the browser build's calls
//! are token-pasted, not typed blind (docs/terminal-access.md). `read_piped`
//! reads a pipe, which is not a TTY question, and compiles untouched.

use std::io::{Read, Write};

use crate::problem::{Problem, Result};

/// One line typed with no echo. Ctrl-C and Ctrl-D cancel.
#[cfg(not(target_arch = "wasm32"))]
pub fn read_hidden(prompt: &str, err: &mut dyn Write) -> Result<String> {
    use ratatui::crossterm::terminal;

    let _ = write!(err, "{prompt}");
    let _ = err.flush();
    terminal::enable_raw_mode().map_err(|e| Problem::new("losos::terminal", e.to_string()))?;
    let result = read_line_raw();
    let _ = terminal::disable_raw_mode();
    let _ = writeln!(err);
    result
}

/// Under wasm32 there is no terminal to read a keystroke from: the browser
/// build is the API/drive surface (README's wasm paragraph), and this keeps
/// the callers compiling with the same refusal a non-TTY run gets.
#[cfg(target_arch = "wasm32")]
pub fn read_hidden(_prompt: &str, _err: &mut dyn Write) -> Result<String> {
    Err(Problem::new(
        "losos::terminal",
        crate::tr!("tui.open.needsTerminal"),
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn read_line_raw() -> Result<String> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

    let mut value = String::new();
    loop {
        let ev = event::read().map_err(|e| Problem::new("losos::terminal", e.to_string()))?;
        match ev {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                match key.code {
                    KeyCode::Enter => return Ok(value),
                    KeyCode::Char('c' | 'd') if ctrl => return Err(Problem::cancelled()),
                    KeyCode::Char('u') if ctrl => value.clear(),
                    KeyCode::Backspace => {
                        value.pop();
                    }
                    KeyCode::Char(c) if !ctrl => value.push(c),
                    KeyCode::Esc => return Err(Problem::cancelled()),
                    _ => {}
                }
            }
            Event::Paste(text) => value.push_str(text.trim_end_matches(['\r', '\n'])),
            _ => {}
        }
    }
}

/// Everything on stdin, for a value piped in by a script. It loses one
/// trailing newline, the one `echo` adds.
pub fn read_piped(stdin: &mut dyn Read) -> Result<String> {
    let mut text = String::new();
    stdin
        .read_to_string(&mut text)
        .map_err(|e| Problem::new("losos::stdin", e.to_string()))?;
    if text.ends_with('\n') {
        text.pop();
        if text.ends_with('\r') {
            text.pop();
        }
    }
    Ok(text)
}
