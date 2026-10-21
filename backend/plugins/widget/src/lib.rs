//! What a hand-written widget is, from the box's side: a name, a size, and a
//! few text files (`index.html` and whatever it links: stylesheets,
//! scripts, SVG pictures, JSON) that the admin page renders inside a
//! sandboxed iframe served under its own CSP (`/widget-frame/`, see
//! `modules/containers.nix`). The box stores the text and never interprets
//! it; the only rules are the file names and the sizes, so one widget cannot
//! be a gigabyte and a thousand of them cannot be a thousand.
//!
//! Shared by the look plugin, which stores widgets, and the builder, which
//! writes them, so the two refuse the same things in the same words.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Most bytes in one widget, all its files together. Generous for a tile
/// with a stylesheet, a script and a picture or two, far short of anything
/// that could be mistaken for an application.
pub const MAX_WIDGET_BYTES: usize = 128 * 1024;
/// Most files in one widget.
pub const MAX_FILES: usize = 12;
/// Longest file name, in characters.
pub const MAX_FILE_NAME_CHARS: usize = 40;
/// The file the frame renders; the others are only what it links.
pub const ENTRY_FILE: &str = "index.html";
/// The kinds of file a widget may carry, by extension. Text only: the frame
/// inlines a linked file, and a picture is SVG.
pub const FILE_KINDS: &[&str] = &["html", "css", "js", "mjs", "json", "svg", "txt", "md"];

/// A request the owner can act on: the field that was wrong, in a sentence.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct Invalid(pub String);

/// One file of a widget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WidgetFile {
    pub name: String,
    pub content: String,
}

impl WidgetFile {
    #[must_use]
    pub fn entry(content: &str) -> Self {
        WidgetFile {
            name: ENTRY_FILE.to_string(),
            content: content.to_string(),
        }
    }
}

/// Read a list of files, `[{ "name": …, "content": … }]`, or `None` when it
/// is not that shape.
#[must_use]
pub fn parse_files(value: &Value) -> Option<Vec<WidgetFile>> {
    value
        .as_array()?
        .iter()
        .map(|f| {
            Some(WidgetFile {
                name: f.get("name")?.as_str()?.to_string(),
                content: f.get("content")?.as_str()?.to_string(),
            })
        })
        .collect()
}

/// A file name a widget may use: lowercase letters, digits, `-` and `_`,
/// then a dot and one of [`FILE_KINDS`]. No directories, so a name can
/// never be read as a path.
#[must_use]
pub fn is_file_name(name: &str) -> bool {
    let Some((stem, kind)) = name.rsplit_once('.') else {
        return false;
    };
    name.chars().count() <= MAX_FILE_NAME_CHARS
        && FILE_KINDS.contains(&kind)
        && stem
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && stem
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_.".contains(&b))
        && !stem.contains("..")
}

/// The rules a widget's files have to meet: `index.html` among them and not
/// empty, names [`is_file_name`] and each used once, at most [`MAX_FILES`]
/// and [`MAX_WIDGET_BYTES`] together.
pub fn validate_files(files: &[WidgetFile]) -> Result<(), Invalid> {
    if files.len() > MAX_FILES {
        return Err(Invalid(format!("a widget has at most {MAX_FILES} files")));
    }
    let mut seen = std::collections::HashSet::new();
    for file in files {
        if !is_file_name(&file.name) {
            return Err(Invalid(format!(
                "{:?} is not a file name a widget can use",
                file.name
                    .chars()
                    .take(MAX_FILE_NAME_CHARS + 1)
                    .collect::<String>()
            )));
        }
        if !seen.insert(file.name.as_str()) {
            return Err(Invalid(format!("there are two files called {}", file.name)));
        }
    }
    match files.iter().find(|f| f.name == ENTRY_FILE) {
        None => return Err(Invalid(format!("the widget has no {ENTRY_FILE}"))),
        Some(entry) if entry.content.trim().is_empty() => {
            return Err(Invalid(format!("{ENTRY_FILE} is empty")))
        }
        Some(_) => {}
    }
    let bytes: usize = files.iter().map(|f| f.content.len()).sum();
    if bytes > MAX_WIDGET_BYTES {
        return Err(Invalid(format!(
            "the widget's files are larger than {} KiB together",
            MAX_WIDGET_BYTES / 1024
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, content: &str) -> WidgetFile {
        WidgetFile {
            name: name.to_string(),
            content: content.to_string(),
        }
    }

    #[test]
    fn a_widget_is_a_few_named_files() {
        let ok = |files: Vec<WidgetFile>| validate_files(&files).is_ok();
        assert!(ok(vec![file("index.html", "<p>")]));
        assert!(ok(vec![
            file("style.css", "p{}"),
            file("index.html", "<p>"),
            file("app.js", ""),
            file("logo-2_dark.svg", "<svg/>"),
            file("data.json", "{}"),
        ]));
        // No entry, an empty entry, a name used twice.
        assert!(!ok(vec![file("app.js", "1")]));
        assert!(!ok(vec![file("index.html", "  \n")]));
        assert!(!ok(vec![
            file("index.html", "<p>"),
            file("index.html", "<b>")
        ]));
        // Too many, too large together though each file is small.
        let many: Vec<WidgetFile> = std::iter::once(file("index.html", "<p>"))
            .chain((1..MAX_FILES).map(|i| file(&format!("f{i}.js"), "1")))
            .collect();
        assert!(ok(many.clone()));
        let mut one_more = many;
        one_more.push(file("last.js", "1"));
        assert!(!ok(one_more));
        let half = "x".repeat(MAX_WIDGET_BYTES / 2);
        assert!(!ok(vec![
            file("index.html", &half),
            file("a.css", &half),
            file("b.css", "x"),
        ]));

        for good in [
            "index.html",
            "a.js",
            "0.css",
            "my-tile_v2.min.js",
            "x.mjs",
            "notes.md",
        ] {
            assert!(is_file_name(good), "{good}");
        }
        for bad in [
            "", "index", ".css", "../a.js", "a/b.js", "a\\b.js", "App.js", "a.exe", "a.png",
            "-a.js", "a..b.js", "a b.js", "a.JS",
        ] {
            assert!(!is_file_name(bad), "{bad}");
        }
        assert!(is_file_name(&format!(
            "{}.js",
            "a".repeat(MAX_FILE_NAME_CHARS - 3)
        )));
        assert!(!is_file_name(&format!(
            "{}.js",
            "a".repeat(MAX_FILE_NAME_CHARS - 2)
        )));
    }
}
