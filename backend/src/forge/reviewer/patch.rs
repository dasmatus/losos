//! A file's unified diff, read into lines that know their place in the new
//! file, and cut into pieces small enough for the model.
//!
//! The model is shown each piece with the new file's line numbers in front
//! of the lines that are there, so what it answers names a line the pull
//! request's head has. Removed lines are shown without a number: the model
//! sees what went, and cannot point at it.

/// What one line of a diff is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Context,
    Added,
    Removed,
}

/// One line of a hunk. `new` is its number in the head's file, for context
/// and added lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub kind: Kind,
    pub new: Option<u32>,
    pub text: String,
}

/// The lines of one file's diff, hunk by hunk.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Hunk {
    pub lines: Vec<Line>,
}

impl Hunk {
    pub fn added(&self) -> usize {
        self.lines.iter().filter(|l| l.kind == Kind::Added).count()
    }
}

/// Read `git diff`'s text for one file (everything from the first `@@`
/// on; the header lines before it are skipped) into its hunks.
pub fn parse(patch: &str) -> Vec<Hunk> {
    let mut hunks: Vec<Hunk> = Vec::new();
    let mut next_new = 0u32;
    for raw in patch.lines() {
        if let Some(start) = hunk_start(raw) {
            hunks.push(Hunk::default());
            next_new = start;
            continue;
        }
        let Some(hunk) = hunks.last_mut() else {
            continue;
        };
        let (kind, text) = match raw.as_bytes().first() {
            Some(b'+') => (Kind::Added, &raw[1..]),
            Some(b'-') => (Kind::Removed, &raw[1..]),
            Some(b' ') => (Kind::Context, &raw[1..]),
            // An empty context line, as some tools write it.
            None => (Kind::Context, ""),
            // "\ No newline at end of file", and anything else.
            _ => continue,
        };
        let new = if kind == Kind::Removed {
            None
        } else {
            let n = next_new;
            next_new += 1;
            Some(n)
        };
        hunk.lines.push(Line {
            kind,
            new,
            text: text.to_string(),
        });
    }
    hunks.retain(|h| h.added() > 0);
    hunks
}

/// One file of a whole `git diff`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    /// The path in the head; `None` for a deleted file.
    pub path: Option<String>,
    /// Git said "Binary files ... differ".
    pub binary: bool,
    /// Its text from the first `@@` on.
    pub patch: String,
    /// The whole file in the head, when the caller read it.
    pub head: Option<String>,
}

/// Cut `git diff --no-color` output into its files.
pub fn files(diff: &str) -> Vec<File> {
    let mut out: Vec<File> = Vec::new();
    let mut in_hunks = false;
    for raw in diff.lines() {
        if raw.starts_with("diff --git ") {
            out.push(File {
                path: None,
                binary: false,
                patch: String::new(),
                head: None,
            });
            in_hunks = false;
            continue;
        }
        let Some(file) = out.last_mut() else {
            continue;
        };
        if !in_hunks {
            if let Some(p) = raw.strip_prefix("+++ ") {
                file.path = p.strip_prefix("b/").map(str::to_string);
            } else if raw.starts_with("Binary files ") {
                file.binary = true;
            } else if raw.starts_with("@@ ") {
                in_hunks = true;
            }
        }
        if in_hunks {
            file.patch.push_str(raw);
            file.patch.push('\n');
        }
    }
    out
}

/// The new side's first line from `@@ -a,b +c,d @@`.
fn hunk_start(line: &str) -> Option<u32> {
    let rest = line.strip_prefix("@@ -")?;
    let plus = rest.find(" +")?;
    let after = &rest[plus + 2..];
    let end = after.find([',', ' '])?;
    after[..end].parse().ok()
}

/// One request's worth of a file: whole hunks while they fit, a long hunk
/// cut at a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub lines: Vec<Line>,
}

impl Piece {
    /// The added line numbered `n`, if this piece shows one.
    pub fn added_line(&self, n: u32) -> Option<&Line> {
        self.lines
            .iter()
            .find(|l| l.kind == Kind::Added && l.new == Some(n))
    }

    pub fn added(&self) -> usize {
        self.lines.iter().filter(|l| l.kind == Kind::Added).count()
    }

    /// The lines as the model reads them: the new line number, `+`, `-` or
    /// a space, and the code. Hunks are apart by an empty line.
    pub fn render(&self) -> String {
        let mut out = String::new();
        let mut last: Option<u32> = None;
        for line in &self.lines {
            if let (Some(prev), Some(n)) = (last, line.new) {
                if n > prev + 1 {
                    out.push('\n');
                }
            }
            let mark = match line.kind {
                Kind::Added => '+',
                Kind::Removed => '-',
                Kind::Context => ' ',
            };
            let shown = match line.new {
                Some(n) => format!("{n:>5} {mark} {}", line.text),
                None => format!("      {mark} {}", line.text),
            };
            out.push_str(shown.trim_end());
            out.push('\n');
            if line.new.is_some() {
                last = line.new;
            }
        }
        out.trim_end().to_string()
    }
}

/// Cut a file's hunks into pieces of at most `max_lines` shown lines. Hunks
/// stay whole when they fit; a hunk longer than that is cut, each part
/// keeping at most three lines before it as context.
pub fn pieces(hunks: &[Hunk], max_lines: usize) -> Vec<Piece> {
    let max_lines = max_lines.max(8);
    let mut out: Vec<Piece> = Vec::new();
    let mut current: Vec<Line> = Vec::new();
    for hunk in hunks {
        if !current.is_empty() && current.len() + hunk.lines.len() > max_lines {
            out.push(Piece {
                lines: std::mem::take(&mut current),
            });
        }
        if hunk.lines.len() <= max_lines {
            current.extend(hunk.lines.iter().cloned());
            continue;
        }
        let mut start = 0;
        while start < hunk.lines.len() {
            let lead = start.min(3);
            let end = (start + max_lines - lead).min(hunk.lines.len());
            let part = Piece {
                lines: hunk.lines[start - lead..end].to_vec(),
            };
            if part.added() > 0 {
                out.push(part);
            }
            start = end;
        }
    }
    if !current.is_empty() {
        out.push(Piece { lines: current });
    }
    out.retain(|p| p.added() > 0);
    out
}

/// What the model is told the file is written in.
pub fn language(path: &str) -> &'static str {
    let name = path.rsplit('/').next().unwrap_or(path);
    let ext = name.rsplit_once('.').map_or("", |(_, e)| e);
    match (name, ext.to_ascii_lowercase().as_str()) {
        ("Dockerfile" | "Containerfile", _) => "Dockerfile",
        ("Makefile", _) => "Makefile",
        (_, "rs") => "Rust",
        (_, "py") => "Python",
        (_, "sh" | "bash") => "Shell",
        (_, "ts") => "TypeScript",
        (_, "tsx") => "TypeScript (React)",
        (_, "js" | "mjs" | "cjs") => "JavaScript",
        (_, "jsx") => "JavaScript (React)",
        (_, "nix") => "Nix",
        (_, "go") => "Go",
        (_, "c" | "h") => "C",
        (_, "cc" | "cpp" | "cxx" | "hpp") => "C++",
        (_, "java") => "Java",
        (_, "kt") => "Kotlin",
        (_, "rb") => "Ruby",
        (_, "php") => "PHP",
        (_, "cs") => "C#",
        (_, "swift") => "Swift",
        (_, "sql") => "SQL",
        (_, "html") => "HTML",
        (_, "css") => "CSS",
        (_, "yml" | "yaml") => "YAML",
        (_, "toml") => "TOML",
        (_, "json") => "JSON",
        (_, "md") => "Markdown",
        _ => "code",
    }
}

/// Files the reviewer leaves alone: what tools write (lock files, minified
/// and generated code), what is not code, and what nobody reads line by
/// line.
pub fn skipped(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    const NAMES: [&str; 10] = [
        "cargo.lock",
        "package-lock.json",
        "yarn.lock",
        "pnpm-lock.yaml",
        "flake.lock",
        "go.sum",
        "poetry.lock",
        "composer.lock",
        "gemfile.lock",
        "devenv.lock",
    ];
    const ENDINGS: [&str; 16] = [
        ".min.js", ".min.css", ".map", ".svg", ".png", ".jpg", ".jpeg", ".gif", ".webp", ".ico",
        ".pdf", ".woff", ".woff2", ".lock", ".snap", ".pb.go",
    ];
    NAMES.contains(&name.as_str())
        || ENDINGS.iter().any(|e| name.ends_with(e))
        || path.split('/').any(|seg| {
            matches!(
                seg,
                "node_modules" | "vendor" | "dist" | "target" | "__snapshots__"
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATCH: &str = "\
diff --git a/src/a.rs b/src/a.rs
index 1111111..2222222 100644
--- a/src/a.rs
+++ b/src/a.rs
@@ -12,7 +12,6 @@ impl Store {
     /// Every user.
     pub fn find(&self) {
-        let a = 1;
-        let b = 2;
+        let a = 3;
         go(a);
     }

@@ -40,3 +39,5 @@ fn tail() {
     x();
+    y();
+    z();
 }
\\ No newline at end of file
";

    #[test]
    fn lines_carry_the_head_numbers_and_removed_lines_none() {
        let hunks = parse(PATCH);
        assert_eq!(hunks.len(), 2);
        let first = &hunks[0].lines;
        assert_eq!(first[0].new, Some(12));
        assert_eq!(first[2].kind, Kind::Removed);
        assert_eq!(first[2].new, None);
        assert_eq!(first[4].kind, Kind::Added);
        assert_eq!(first[4].new, Some(14));
        assert_eq!(first[4].text, "        let a = 3;");
        assert_eq!(first[5].new, Some(15));
        let second = &hunks[1].lines;
        assert_eq!(second[1].new, Some(40));
        assert_eq!(second[2].new, Some(41));
        assert_eq!(second.len(), 4, "the no-newline marker is not a line");
    }

    #[test]
    fn a_hunk_that_only_removes_is_nothing_to_review() {
        let hunks = parse("@@ -1,2 +1,1 @@\n a\n-b\n");
        assert!(hunks.is_empty());
    }

    #[test]
    fn the_model_sees_numbers_marks_and_a_gap_between_hunks() {
        let pieces = pieces(&parse(PATCH), 100);
        assert_eq!(pieces.len(), 1);
        let shown = pieces[0].render();
        assert!(shown.contains("   14 +         let a = 3;"), "{shown}");
        assert!(shown.contains("      -         let a = 1;"), "{shown}");
        assert!(shown.contains("   17\n\n   39       x();"), "{shown}");
    }

    #[test]
    fn long_hunks_are_cut_with_some_context_and_pieces_stay_small() {
        let mut patch = String::from("@@ -1,0 +1,50 @@\n");
        for i in 0..50 {
            patch.push_str(&format!("+line {i}\n"));
        }
        let all = pieces(&parse(&patch), 20);
        assert!(all.iter().all(|p| p.lines.len() <= 20));
        assert_eq!(all[0].lines[0].new, Some(1));
        assert_eq!(all[1].lines[0].new, Some(18), "three lines before it");
        let added: std::collections::BTreeSet<u32> = all
            .iter()
            .flat_map(|p| p.lines.iter().filter_map(|l| l.new))
            .collect();
        assert_eq!(added.len(), 50, "every line is shown");
    }

    #[test]
    fn small_hunks_share_a_piece_until_it_is_full() {
        let patch = "@@ -1,3 +1,4 @@\n a\n+b\n c\n d\n@@ -10,3 +11,4 @@\n e\n+f\n g\n h\n@@ -20,3 +22,4 @@\n i\n+j\n k\n l\n";
        assert_eq!(pieces(&parse(patch), 12).len(), 1);
        assert_eq!(pieces(&parse(patch), 8).len(), 2);
    }

    #[test]
    fn a_whole_diff_is_cut_into_files() {
        let diff = format!(
            "{PATCH}diff --git a/old.txt b/old.txt\ndeleted file mode 100644\n--- a/old.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-x\ndiff --git a/i.png b/i.png\nBinary files a/i.png and b/i.png differ\n"
        );
        let all = files(&diff);
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].path.as_deref(), Some("src/a.rs"));
        assert!(all[0].patch.starts_with("@@ -12,7"));
        assert_eq!(parse(&all[0].patch).len(), 2);
        assert_eq!(all[1].path, None);
        assert!(all[2].binary);
    }

    #[test]
    fn languages_and_files_left_alone() {
        assert_eq!(language("backend/src/main.rs"), "Rust");
        assert_eq!(language("x/Dockerfile"), "Dockerfile");
        assert_eq!(language("a/b.tsx"), "TypeScript (React)");
        assert_eq!(language("README"), "code");
        assert!(skipped("backend/Cargo.lock"));
        assert!(skipped("web/node_modules/x/index.js"));
        assert!(skipped("assets/app.min.js"));
        assert!(!skipped("src/lock.rs"));
    }
}
