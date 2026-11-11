//! Reading from the terminal outside the full-screen view: a value typed
//! without echo, or everything piped on stdin.

use std::io::{Read, Write};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::crossterm::terminal;

use crate::problem::{Problem, Result};

/// One line typed with no echo. Ctrl-C and Ctrl-D cancel.
pub fn read_hidden(prompt: &str, err: &mut dyn Write) -> Result<String> {
    let _ = write!(err, "{prompt}");
    let _ = err.flush();
    terminal::enable_raw_mode().map_err(|e| Problem::new("losos::terminal", e.to_string()))?;
    let result = read_line_raw();
    let _ = terminal::disable_raw_mode();
    let _ = writeln!(err);
    result
}

fn read_line_raw() -> Result<String> {
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
