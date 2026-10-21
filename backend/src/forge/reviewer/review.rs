//! One review, start to finish: the box's checks and the model's questions
//! over each file, the model's answers held to what the diff shows, and
//! the review that comes out of it.
//!
//! The model is asked three kinds of question, each short, each with a
//! fixed answer shape a 1.5B model keeps to:
//!
//!   * per piece of a file: which added lines have a bug, as
//!     ``L<n>: `<code>` <why and the fix>``, or `LGTM`;
//!   * per file: one sentence on what the change does;
//!   * once: two sentences on what the whole pull request does.
//!
//! What it says about a line is kept only when the line is one the pull
//! request adds, in the piece it was shown, and the code it quotes is on
//! that line. That is what turns most of a small model's invented findings
//! into nothing rather than into comments.

use super::checks::{self, Lead};
use super::patch::{self, Piece};

/// The longest piece of a file the model is shown, in lines. A small model
/// reads short pieces better, and at about 45 tokens a second on two cores
/// each one costs some 10 to 20 seconds.
pub const PIECE_LINES: usize = 80;
/// The most pieces one review asks about. A larger pull request is reviewed
/// in its first files, and the review says which ones it left.
pub const MAX_PIECES: usize = 24;
/// The most comments one file gets, and one review.
pub const MAX_PER_FILE: usize = 4;
pub const MAX_COMMENTS: usize = 12;

/// Asks the model one question. The real one is [`super::model`]; tests
/// answer from recordings.
pub trait Ask {
    fn ask(&self, system: &str, user: &str, max_tokens: u32) -> anyhow::Result<String>;
}

impl Ask for super::model::Config {
    fn ask(&self, system: &str, user: &str, max_tokens: u32) -> anyhow::Result<String> {
        super::model::ask(self, system, user, max_tokens, false).map(|r| r.text)
    }
}

pub const FIND: &str = "\
You are a careful code reviewer. You see one file of a pull request. Every line starts with its line number. \"+\" marks a line the pull request adds; \"-\" marks a removed line; other lines are unchanged context.

Report only real bugs in the \"+\" lines: wrong logic, off-by-one, crashes, injection, path traversal, unquoted shell variables, values used before they exist. Do not report style, naming, comments, tests or missing documentation. Do not describe what the code does. Most changes have no bug.

Write each bug on its own line, at most three, exactly like this:
L<line number>: `<the exact code on that line that is wrong>` <why it is wrong and the fix, one sentence>

Example:
L42: `for i in 0..=len {` reads one item past the end; use `0..len`.

If there is no bug, write only:
LGTM";

pub const DESCRIBE: &str = "\
You describe one file's changes in a pull request for a reviewer. Every line starts with its line number. \"+\" marks a line the pull request adds, \"-\" a removed line, the rest is unchanged context.
Answer with one sentence of at most 20 words saying what the change does, starting with a verb (for example \"Adds\", \"Fixes\", \"Moves\"). Do not name the file. No code, no line numbers, no judgement.";

pub const OVERVIEW: &str = "\
You write the overview of a pull request for its reviewers, from its title and a one-line description of each changed file.
Answer with at most two plain sentences on what the pull request does as a whole. No lists, no headings, no judgement, no file names.";

/// One file as the review sees it.
#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: String,
    pub patch: String,
}

/// What became of one changed file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRow {
    pub path: String,
    /// The model's sentence, or why the file was left.
    pub description: String,
    pub reviewed: bool,
}

/// A comment on one line of the head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    pub path: String,
    pub line: u32,
    pub body: String,
    /// The box's own check, rather than the model.
    pub from_check: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub overview: String,
    pub files: Vec<FileRow>,
    pub comments: Vec<Comment>,
    /// The review stopped before the end: a newer push, or the setting
    /// was turned off.
    pub stopped: bool,
}

impl Outcome {
    /// The review's own text: the overview, then a table of the files.
    /// `since_last`: only the commits since the last review were looked at.
    pub fn body(&self, model: &str, since_last: bool) -> String {
        let reviewed = self.files.iter().filter(|f| f.reviewed).count();
        let mut out = String::from("## Pull request overview\n\n");
        if !self.overview.is_empty() {
            out.push_str(&self.overview);
            out.push_str("\n\n");
        }
        let comments = match self.comments.len() {
            0 => "found nothing to comment on".to_string(),
            1 => "left 1 comment".to_string(),
            n => format!("left {n} comments"),
        };
        if since_last {
            out.push_str("These are the commits pushed since the last review.\n\n");
        }
        out.push_str(&format!(
            "LosOS Reviewer looked at {reviewed} of {} changed {} and {comments}.\n\n",
            self.files.len(),
            if self.files.len() == 1 {
                "file"
            } else {
                "files"
            }
        ));
        if !self.files.is_empty() {
            out.push_str("| File | Description |\n| --- | --- |\n");
            for f in &self.files {
                out.push_str(&format!(
                    "| `{}` | {} |\n",
                    f.path.replace('|', "\\|"),
                    f.description.replace('|', "\\|").replace('\n', " ")
                ));
            }
            out.push('\n');
        }
        out.push_str(&format!(
            "Reviewed on this box by {model}. The code did not leave it."
        ));
        out
    }
}

/// A finding the model wrote, before it is checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub line: u32,
    pub quote: Option<String>,
    pub text: String,
}

/// Read the model's answer to [`FIND`] into findings, as written.
pub fn parse_findings(answer: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    for raw in answer.lines() {
        let line = raw.trim().trim_start_matches(['-', '*', ' ']);
        let rest = line
            .strip_prefix('L')
            .or_else(|| line.strip_prefix("Line "))
            .or_else(|| line.strip_prefix("line "))
            .unwrap_or(line);
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        let Ok(n) = digits.parse::<u32>() else {
            continue;
        };
        let after = rest[digits.len()..].trim_start();
        let Some(after) = after
            .strip_prefix(':')
            .or_else(|| after.strip_prefix('.'))
            .or_else(|| after.strip_prefix(')'))
            .or_else(|| after.strip_prefix(" -"))
        else {
            continue;
        };
        let after = after.trim();
        let (quote, text) = match after.strip_prefix('`') {
            Some(q) => match q.find('`') {
                Some(end) => (Some(q[..end].to_string()), q[end + 1..].trim()),
                None => (None, after),
            },
            None => (None, after),
        };
        let text = text
            .trim_start_matches(['-', ':', '–', ' '])
            .trim_start_matches('<')
            .trim_end_matches('>')
            .trim();
        out.push(Finding {
            line: n,
            quote,
            text: text.to_string(),
        });
    }
    out
}

fn squeeze(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Words a model uses when it says a line is fine, which it then files as
/// a finding anyway.
const NOT_A_BUG: [&str; 7] = [
    "not a bug",
    "not a real bug",
    "is correct",
    "is fine",
    "no issue",
    "no bug",
    "lgtm",
];

/// Keep the findings that hold up against the piece: an added line, the
/// quoted code on it, something said that is not the code again or a
/// change to nothing.
pub fn checked(findings: Vec<Finding>, piece: &Piece) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    for f in findings {
        let Some(line) = piece.added_line(f.line) else {
            continue;
        };
        let Some(quote) = f.quote.as_deref().map(squeeze) else {
            continue;
        };
        let code = squeeze(&line.text);
        if quote.chars().count() < 4.min(code.chars().count()) || !code.contains(&quote) {
            continue;
        }
        let text = f.text.trim();
        let lower = text.to_lowercase();
        if text.chars().count() < 12 || NOT_A_BUG.iter().any(|w| lower.contains(w)) {
            continue;
        }
        // "should be `x`" where x is the line as it is: no change at all.
        if let Some(start) = text.find('`') {
            if let Some(len) = text[start + 1..].find('`') {
                let proposed = squeeze(&text[start + 1..start + 1 + len]);
                if proposed == quote || proposed == code {
                    continue;
                }
            }
        }
        if out.iter().any(|o| o.line == f.line) {
            continue;
        }
        out.push(Finding {
            text: text.to_string(),
            ..f
        });
    }
    out.truncate(3);
    out
}

/// The comment for a check's lead.
fn lead_body(lead: &Lead) -> String {
    match &lead.suggestion {
        Some(fixed) => format!("{}\n\n```suggestion\n{fixed}\n```", lead.message),
        None => lead.message.clone(),
    }
}

/// The question about one piece. It does not say what the checks found:
/// told that a line already has a comment, 7B answered LGTM for the whole
/// piece, bugs on other lines included. A finding on a line a check took
/// is dropped afterwards instead.
pub fn piece_question(title: &str, path: &str, piece: &Piece) -> String {
    format!(
        "Pull request: {title}\nFile: {path} ({})\n\n{}",
        patch::language(path),
        piece.render()
    )
}

/// The model's one-sentence description, tidied; `None` when it wrote
/// something else.
pub fn clean_sentence(answer: &str, max_chars: usize) -> Option<String> {
    let first = answer.lines().map(str::trim).find(|l| !l.is_empty())?;
    if first.starts_with("```") {
        return None;
    }
    let text = first.trim_matches(['"', '*', '`']).trim();
    let text = text.strip_prefix("Summary:").unwrap_or(text).trim();
    if text.is_empty() || text.contains("```") {
        return None;
    }
    let mut out: String = text.chars().take(max_chars).collect();
    if out.chars().count() < text.chars().count() {
        if let Some(cut) = out.rfind(' ') {
            out.truncate(cut);
        }
        out.push('…');
    }
    Some(out)
}

/// Why a file is not reviewed, if it is not.
fn left_out(path: &str, binary: bool) -> Option<&'static str> {
    if binary {
        Some("Not reviewed: not text.")
    } else if patch::skipped(path) {
        Some("Not reviewed: written by a tool, or not code.")
    } else {
        None
    }
}

/// Review the files. `progress(done, total)` is called after each file and
/// answers whether to go on.
pub fn run(
    ask: &dyn Ask,
    title: &str,
    files: &[patch::File],
    progress: &mut dyn FnMut(usize, usize) -> bool,
) -> anyhow::Result<Outcome> {
    let mut rows: Vec<FileRow> = Vec::new();
    let mut todo: Vec<(usize, String, Vec<Piece>, Option<String>)> = Vec::new();
    let mut budget = MAX_PIECES;
    for f in files {
        let Some(path) = f.path.clone() else { continue };
        if let Some(why) = left_out(&path, f.binary) {
            rows.push(FileRow {
                path,
                description: why.to_string(),
                reviewed: false,
            });
            continue;
        }
        let pieces = patch::pieces(&patch::parse(&f.patch), PIECE_LINES);
        if pieces.is_empty() {
            rows.push(FileRow {
                path,
                description: "Only removes lines.".to_string(),
                reviewed: false,
            });
            continue;
        }
        if pieces.len() > budget {
            rows.push(FileRow {
                path,
                description: "Not reviewed: the pull request is larger than one review covers."
                    .to_string(),
                reviewed: false,
            });
            budget = 0;
            continue;
        }
        budget -= pieces.len();
        rows.push(FileRow {
            path: path.clone(),
            description: String::new(),
            reviewed: true,
        });
        todo.push((rows.len() - 1, path, pieces, f.head.clone()));
    }

    let total = todo.len();
    let mut comments: Vec<Comment> = Vec::new();
    let mut stopped = !progress(0, total);
    for (done, (row, path, pieces, head)) in todo.iter().enumerate() {
        if stopped {
            break;
        }
        let mut here: Vec<Comment> = Vec::new();
        for piece in pieces {
            let leads = checks::leads(path, piece, head.as_deref());
            for lead in &leads {
                here.push(Comment {
                    path: path.clone(),
                    line: lead.line,
                    body: lead_body(lead),
                    from_check: true,
                });
            }
            let answer = ask.ask(FIND, &piece_question(title, path, piece), 300)?;
            for f in checked(parse_findings(&answer), piece) {
                if here.iter().any(|c| c.line == f.line) {
                    continue;
                }
                here.push(Comment {
                    path: path.clone(),
                    line: f.line,
                    body: f.text,
                    from_check: false,
                });
            }
        }
        here.sort_by_key(|c| (!c.from_check, c.line));
        here.truncate(MAX_PER_FILE);
        here.sort_by_key(|c| c.line);
        comments.extend(here);

        let first = &pieces[0];
        let said = ask.ask(DESCRIBE, &piece_question(title, path, first), 60)?;
        rows[*row].description = clean_sentence(&said, 160)
            .unwrap_or_else(|| format!("Changes {} lines.", count_changes(pieces)));
        stopped = !progress(done + 1, total);
    }
    if stopped {
        return Ok(Outcome {
            overview: String::new(),
            files: rows,
            comments: Vec::new(),
            stopped: true,
        });
    }
    comments.sort_by_key(|c| !c.from_check);
    comments.truncate(MAX_COMMENTS);
    comments.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));

    let described: Vec<String> = rows
        .iter()
        .filter(|r| r.reviewed)
        .map(|r| format!("- {}", r.description))
        .collect();
    // One file: its sentence says it all, and the question is saved.
    let overview = if described.len() < 2 {
        rows.iter()
            .find(|r| r.reviewed)
            .map(|r| r.description.clone())
            .unwrap_or_default()
    } else {
        let said = ask.ask(
            OVERVIEW,
            &format!("Title: {title}\n\nFiles:\n{}", described.join("\n")),
            90,
        )?;
        clean_sentence(&said, 400).unwrap_or_default()
    };
    Ok(Outcome {
        overview,
        files: rows,
        comments,
        stopped: false,
    })
}

fn count_changes(pieces: &[Piece]) -> usize {
    pieces.iter().map(Piece::added).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn findings_are_read_in_the_shapes_a_small_model_writes() {
        let answer = "L15: `let sql = format!(\"x\")` SQL injection; use a parameter.\n\nL17: no quote here at all\n- L23: `for i in 0..=n {` <off by one; use `0..n`.>\nLGTM\nline 9) `x` y";
        let all = parse_findings(answer);
        assert_eq!(all.len(), 4);
        assert_eq!(all[0].line, 15);
        assert_eq!(all[0].quote.as_deref(), Some("let sql = format!(\"x\")"));
        assert_eq!(all[0].text, "SQL injection; use a parameter.");
        assert_eq!(all[1].quote, None);
        assert_eq!(all[2].text, "off by one; use `0..n`.");
        assert_eq!(all[3].line, 9);
    }

    fn piece() -> Piece {
        let patch = "@@ -10,3 +10,5 @@\n fn a() {\n-    let x = 1;\n+    let sql = format!(\"SELECT * FROM t WHERE a = '{a}'\");\n+    let rows = stmt.query_map([], |r| r.get(0))?;\n     done();\n+    Ok(())\n";
        patch::pieces(&patch::parse(patch), 80).remove(0)
    }

    #[test]
    fn findings_that_do_not_hold_up_against_the_diff_are_dropped() {
        let p = piece();
        let kept = checked(
            parse_findings(
                "L12: `stmt.query_map([], |r| r.get(0))` passes no parameters; pass `params![a]`.\n\
                 L10: `fn a() {` context is not added.\n\
                 L13: `done();` this line is not added.\n\
                 L12: `stmt.query_map` said twice about the same line.\n\
                 L14: `Ok(())` should be `Ok(())` for safety.\n\
                 L14: `return Ok(())` the quote is not on the line.\n\
                 L14: `Ok(())` short\n\
                 L12: `stmt.query_map([], |r| r.get(0))` This is not a real bug, it is fine.\n\
                 L40: `x` not in this piece at all.",
            ),
            &p,
        );
        assert_eq!(kept.len(), 1, "{kept:?}");
        assert_eq!(kept[0].line, 12);
    }

    struct Recorded {
        answers: RefCell<Vec<&'static str>>,
        asked: RefCell<Vec<(String, String)>>,
    }

    impl Ask for Recorded {
        fn ask(&self, system: &str, user: &str, _max: u32) -> anyhow::Result<String> {
            self.asked
                .borrow_mut()
                .push((system.to_string(), user.to_string()));
            let mut a = self.answers.borrow_mut();
            Ok(if a.is_empty() {
                "LGTM".to_string()
            } else {
                a.remove(0).to_string()
            })
        }
    }

    #[test]
    fn a_review_puts_the_checks_and_the_models_findings_together() {
        let diff = "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -10,3 +10,5 @@\n fn a() {\n-    let x = 1;\n+    let sql = format!(\"SELECT * FROM t WHERE a = '{a}'\");\n+    let rows = stmt.query_map([], |r| r.get(0))?;\n     done();\n+    Ok(())\ndiff --git a/Cargo.lock b/Cargo.lock\n--- a/Cargo.lock\n+++ b/Cargo.lock\n@@ -1 +1 @@\n-a\n+b\n";
        let model = Recorded {
            answers: RefCell::new(vec![
                "L12: `stmt.query_map([], |r| r.get(0))` runs the query with no parameters; pass `params![a]`.",
                "Builds a query for rows of t that match a.",
            ]),
            asked: RefCell::new(Vec::new()),
        };
        let mut seen = Vec::new();
        let out = run(&model, "Look rows up", &patch::files(diff), &mut |d, t| {
            seen.push((d, t));
            true
        })
        .unwrap();
        assert_eq!(seen, vec![(0, 1), (1, 1)]);
        assert_eq!(out.comments.len(), 2);
        assert_eq!(out.comments[0].line, 11);
        assert!(out.comments[0].from_check);
        assert!(out.comments[0].body.contains("SQL injection"));
        assert_eq!(out.comments[1].line, 12);
        assert!(!out.comments[1].from_check);
        assert_eq!(out.overview, "Builds a query for rows of t that match a.");
        assert_eq!(out.files[1].path, "Cargo.lock");
        assert!(!out.files[1].reviewed);
        let asked = model.asked.borrow();
        assert_eq!(asked.len(), 2, "one file: no overview question");
        assert!(asked[0]
            .1
            .contains("already commented on line 11; leave it out"));
        assert!(asked[0].1.contains("   12 +     let rows"));
        let body = out.body("qwen2.5-coder:1.5b", false);
        assert!(body.contains("looked at 1 of 2 changed files and left 2 comments"));
        assert!(body.contains("| `src/a.rs` | Builds a query for rows of t that match a. |"));
        assert!(body.contains("The code did not leave it."));
    }

    #[test]
    fn a_review_stops_when_told_and_says_so() {
        let diff = "diff --git a/a.py b/a.py\n--- a/a.py\n+++ b/a.py\n@@ -1 +1,2 @@\n x\n+y = 1\ndiff --git a/b.py b/b.py\n--- a/b.py\n+++ b/b.py\n@@ -1 +1,2 @@\n x\n+z = 2\n";
        let model = Recorded {
            answers: RefCell::new(Vec::new()),
            asked: RefCell::new(Vec::new()),
        };
        let out = run(&model, "t", &patch::files(diff), &mut |d, _| d < 1).unwrap();
        assert!(out.stopped);
        assert_eq!(model.asked.borrow().len(), 2, "one file, then it stopped");
    }

    #[test]
    fn a_sentence_is_one_line_and_not_too_long() {
        assert_eq!(
            clean_sentence("\n\"Adds a thing.\"\nMore.", 100).as_deref(),
            Some("Adds a thing.")
        );
        assert_eq!(
            clean_sentence("one two three four", 9).as_deref(),
            Some("one two…")
        );
        assert_eq!(clean_sentence("```rust\nx\n```", 100), None);
    }
}
