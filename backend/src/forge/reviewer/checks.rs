//! The box's own checks: a short list of bug shapes that show in one line
//! (or one React effect) and are almost never right.
//!
//! They exist because of what the box's default model can do. Measured on
//! planted bugs, Qwen2.5-Coder 1.5B (the model a 4 GiB box runs) found none
//! of them when asked to review freely, and answered "yes, a bug" for most
//! lines when asked line by line; 7B found most of them with no false
//! alarms. So the box points out what it can tell for certain, in its own
//! words, and the model adds what it finds beside that. Every check here
//! should stay quiet on code that is fine: one that cries wolf teaches
//! people to skip the reviewer.

use super::patch::{Kind, Line, Piece};

/// One line a check flagged, with what to tell the author.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lead {
    pub line: u32,
    /// Which check, for the logs and the tests.
    pub check: &'static str,
    /// The comment, written for the author.
    pub message: String,
    /// The line as it should read, when the fix is that one line.
    pub suggestion: Option<String>,
}

/// Every check that applies to the file's language, on the piece's added
/// lines. At most one lead per line: the first check that matches. `head`
/// is the whole file as the pull request leaves it, when the caller has
/// it: a React effect's opening line is often outside the diff's context.
pub fn leads(path: &str, piece: &Piece, head: Option<&str>) -> Vec<Lead> {
    let lang = super::patch::language(path);
    let mut out: Vec<Lead> = Vec::new();
    let mut push = |lead: Lead| {
        if !out.iter().any(|l| l.line == lead.line) {
            out.push(lead);
        }
    };
    let added: Vec<&Line> = piece
        .lines
        .iter()
        .filter(|l| l.kind == Kind::Added && l.new.is_some())
        .collect();
    for line in &added {
        let n = line.new.unwrap_or(0);
        let text = line.text.as_str();
        if let Some(lead) = secret(n, text) {
            push(lead);
            continue;
        }
        if let Some(lead) = sql_built_from_values(n, text, lang) {
            push(lead);
            continue;
        }
        match lang {
            "Shell" | "Dockerfile" | "Makefile" => {
                if let Some(lead) = unquoted_in_shell(n, text) {
                    push(lead);
                }
            }
            "Python" => {
                if let Some(lead) = mutable_default(n, text) {
                    push(lead);
                } else if let Some(lead) = shell_from_python(n, text) {
                    push(lead);
                }
            }
            "Rust" => {
                if let Some(lead) = inclusive_to_len(n, text) {
                    push(lead);
                }
            }
            "TypeScript" | "TypeScript (React)" | "JavaScript" | "JavaScript (React)" => {
                if let Some(lead) = loose_equality(n, text) {
                    push(lead);
                }
            }
            _ => {}
        }
    }
    if lang.contains("React") || lang.starts_with("TypeScript") || lang.starts_with("JavaScript") {
        for lead in effects_without_dependencies(piece, head) {
            push(lead);
        }
    }
    for lead in safety_step_dropped(piece) {
        push(lead);
    }
    out.sort_by_key(|l| l.line);
    out
}

fn lead(line: u32, check: &'static str, message: impl Into<String>) -> Lead {
    Lead {
        line,
        check,
        message: message.into(),
        suggestion: None,
    }
}

/// The code of a line without what a comment says.
fn code_part<'a>(text: &'a str, lang: &str) -> &'a str {
    let t = text.trim_start();
    let marker = match lang {
        "Python" | "Shell" | "Dockerfile" | "Makefile" | "YAML" | "TOML" | "Nix" | "Ruby" => "#",
        _ => "//",
    };
    if t.starts_with(marker) || t.starts_with("/*") || t.starts_with('*') {
        ""
    } else {
        text
    }
}

// ── Secrets ─────────────────────────────────────────────────────────────────

/// A key or password written into the code.
fn secret(n: u32, text: &str) -> Option<Lead> {
    const PREFIXES: [(&str, usize, &str); 7] = [
        ("AKIA", 16, "an AWS access key"),
        ("ghp_", 36, "a GitHub token"),
        ("github_pat_", 40, "a GitHub token"),
        ("glpat-", 20, "a GitLab token"),
        ("sk_live_", 20, "a live Stripe key"),
        ("xoxb-", 20, "a Slack token"),
        ("xoxp-", 20, "a Slack token"),
    ];
    for (prefix, len, what) in PREFIXES {
        if let Some(at) = text.find(prefix) {
            let rest = &text[at + prefix.len()..];
            let body = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .count();
            if body >= len {
                return Some(lead(
                    n,
                    "secret",
                    format!(
                        "This looks like {what} written into the code. Anyone who can read the repository can use it: take it out, revoke it, and read it from the environment or a secret file instead."
                    ),
                ));
            }
        }
    }
    if text.contains("-----BEGIN") && text.contains("PRIVATE KEY-----") {
        return Some(lead(
            n,
            "secret",
            "A private key is written into the code. Anyone who can read the repository has it: take it out, make a new key, and load it from a file that is not committed.",
        ));
    }
    None
}

// ── SQL ─────────────────────────────────────────────────────────────────────

/// SQL text put together from values, rather than values passed as
/// parameters.
fn sql_built_from_values(n: u32, text: &str, lang: &str) -> Option<Lead> {
    let code = code_part(text, lang);
    let upper = code.to_ascii_uppercase();
    let statement = ["SELECT ", "INSERT INTO ", "UPDATE ", "DELETE FROM "]
        .iter()
        .any(|k| upper.contains(k))
        && [" FROM ", " WHERE ", " SET ", " VALUES", " INTO "]
            .iter()
            .any(|k| upper.contains(k));
    if !statement {
        return None;
    }
    let built = match lang {
        "Rust" => code.contains("format!(") && has_placeholder(code, '{', '}'),
        "Python" => {
            code.contains("f\"") && has_placeholder(code, '{', '}')
                || code.contains("f'") && has_placeholder(code, '{', '}')
                || code.contains(".format(")
                || code.contains("\" % ")
                || code.contains("' % ")
                || concatenated(code)
        }
        "TypeScript" | "TypeScript (React)" | "JavaScript" | "JavaScript (React)" => {
            code.contains("${") || concatenated(code)
        }
        "Go" => code.contains("fmt.Sprintf(") || concatenated(code),
        "Java" | "Kotlin" | "C#" | "PHP" => concatenated(code) || code.contains("String.format("),
        _ => false,
    };
    built.then(|| {
        lead(
            n,
            "sql",
            "The SQL text is built from a value, so a quote in that value changes the query (SQL injection). Pass the value as a query parameter instead.",
        )
    })
}

/// `{name}` inside the line: a format placeholder that takes a value.
fn has_placeholder(code: &str, open: char, close: char) -> bool {
    let mut rest = code;
    while let Some(at) = rest.find(open) {
        let after = &rest[at + open.len_utf8()..];
        if let Some(end) = after.find(close) {
            let inside = &after[..end];
            if !inside.is_empty()
                && inside
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == ':')
                && !inside.starts_with(':')
            {
                return true;
            }
        }
        rest = after;
    }
    false
}

/// A string literal joined to something with `+`.
fn concatenated(code: &str) -> bool {
    code.contains("\" +") || code.contains("+ \"") || code.contains("' +") || code.contains("+ '")
}

// ── Shell ───────────────────────────────────────────────────────────────────

/// A command that changes or removes files given a variable outside double
/// quotes: a space in it makes two arguments, an empty one a different
/// command.
fn unquoted_in_shell(n: u32, text: &str) -> Option<Lead> {
    let code = code_part(text, "Shell");
    let trimmed = code.trim_start();
    let body = trimmed.strip_prefix("RUN ").unwrap_or(trimmed);
    let mut words = body.split_whitespace();
    let mut command = words.next()?;
    if command == "sudo" {
        command = words.next()?;
    }
    if !["rm", "mv", "cp", "chmod", "chown", "rmdir", "ln", "rsync"].contains(&command) {
        return None;
    }
    let names = unquoted_variables(body);
    if names.is_empty() {
        return None;
    }
    let list = names
        .iter()
        .map(|v| format!("`${v}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut found = lead(
        n,
        "shell-quotes",
        format!(
            "`{command}` gets {list} without double quotes. A space in the value splits it into two paths, and an empty value leaves the command working on the wrong place. Put each variable in double quotes."
        ),
    );
    found.suggestion = Some(quote_variables(code));
    Some(found)
}

/// The names of `$x` and `${x}` that stand outside double and single
/// quotes.
fn unquoted_variables(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = code.chars().collect();
    let (mut single, mut double) = (false, false);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' => i += 1,
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            '#' if !single && !double && (i == 0 || chars[i - 1].is_whitespace()) => break,
            '$' if !single && !double => {
                if let Some((name, len)) = variable_at(&chars[i + 1..]) {
                    if !out.contains(&name) {
                        out.push(name);
                    }
                    i += len;
                }
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// The variable after a `$`: its name and how many characters it takes.
/// `$(` and `$((` are commands, not variables.
fn variable_at(rest: &[char]) -> Option<(String, usize)> {
    match rest.first()? {
        '{' => {
            let end = rest.iter().position(|c| *c == '}')?;
            let name: String = rest[1..end].iter().collect();
            let plain: String = name
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            (!plain.is_empty()).then_some((plain, end + 1))
        }
        c if c.is_alphabetic() || *c == '_' => {
            let name: String = rest
                .iter()
                .take_while(|c| c.is_alphanumeric() || **c == '_')
                .collect();
            let len = name.chars().count();
            Some((name, len))
        }
        c if c.is_ascii_digit() || *c == '@' || *c == '*' => Some((c.to_string(), 1)),
        _ => None,
    }
}

/// The line with every word that holds an unquoted variable in double
/// quotes.
fn quote_variables(code: &str) -> String {
    let lead_ws: String = code.chars().take_while(|c| c.is_whitespace()).collect();
    let words = split_words(code.trim_start());
    let quoted: Vec<String> = words
        .into_iter()
        .map(|w| {
            if !unquoted_variables(&w).is_empty() && !w.contains('"') && !w.contains('\'') {
                format!("\"{w}\"")
            } else {
                w
            }
        })
        .collect();
    format!("{lead_ws}{}", quoted.join(" "))
}

/// Words split at blanks outside quotes, kept as written.
fn split_words(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let (mut single, mut double) = (false, false);
    for c in code.chars() {
        match c {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            c if c.is_whitespace() && !single && !double => {
                if !word.is_empty() {
                    out.push(std::mem::take(&mut word));
                }
                continue;
            }
            _ => {}
        }
        word.push(c);
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

// ── Python ──────────────────────────────────────────────────────────────────

/// `def f(x=[])`: one list for every call.
fn mutable_default(n: u32, text: &str) -> Option<Lead> {
    let code = code_part(text, "Python").trim_start();
    if !(code.starts_with("def ") || code.starts_with("async def ")) {
        return None;
    }
    let squeezed: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    let hit = ["=[]", "={}", "=set()", "=list()", "=dict()"]
        .iter()
        .find(|k| squeezed.contains(*k))?;
    Some(lead(
        n,
        "mutable-default",
        format!(
            "The default `{}` is made once, when the function is defined, so every call that leaves it out shares the same object and sees what earlier calls put in it. Default to `None` and make a new one inside the function.",
            hit.trim_start_matches('=')
        ),
    ))
}

/// A shell command line put together from values in Python.
fn shell_from_python(n: u32, text: &str) -> Option<Lead> {
    let code = code_part(text, "Python");
    let formatted = (code.contains("f\"") || code.contains("f'"))
        && has_placeholder(code, '{', '}')
        || code.contains(".format(")
        || code.contains("\" % ")
        || concatenated(code);
    let shell = code.contains("os.system(")
        || code.contains("os.popen(")
        || (code.contains("subprocess.") && code.contains("shell=True"));
    (shell && formatted).then(|| {
        lead(
            n,
            "shell-injection",
            "The command line is built from a value and run by a shell, so a `;` or a space in the value runs something else. Pass the program and its arguments as a list without `shell=True`.",
        )
    })
}

// ── Rust ────────────────────────────────────────────────────────────────────

/// `0..=v.len()`: one step past the last item.
fn inclusive_to_len(n: u32, text: &str) -> Option<Lead> {
    let code = code_part(text, "Rust");
    let at = code.find("..=")?;
    let bound: String = code[at + 3..]
        .chars()
        .take_while(|c| !matches!(c, '{' | ';' | ']'))
        .collect();
    if !bound.contains("len()") {
        return None;
    }
    let mut found = lead(
        n,
        "range-end",
        "The range `..=` includes its end, and the end here is a length, so the last step is one past the last item and indexing with it panics. Use `..` instead.",
    );
    found.suggestion = Some(code.replacen("..=", "..", 1));
    Some(found)
}

// ── JavaScript and TypeScript ───────────────────────────────────────────────

/// `==` and `!=`, which convert before they compare. `== null` is the one
/// common use on purpose and is left alone.
fn loose_equality(n: u32, text: &str) -> Option<Lead> {
    let code = code_part(text, "TypeScript");
    let bytes = code.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        let two = &bytes[i..i + 2];
        if (two == b"==" || two == b"!=")
            && bytes.get(i + 2) != Some(&b'=')
            && (i == 0 || !matches!(bytes[i - 1], b'=' | b'!' | b'<' | b'>'))
        {
            let after = code[i + 2..].trim_start();
            if after.starts_with("null") {
                i += 2;
                continue;
            }
            let op = if two == b"==" { "==" } else { "!=" };
            let strict = if op == "==" { "===" } else { "!==" };
            let mut found = lead(
                n,
                "loose-equality",
                format!(
                    "`{op}` converts both sides before it compares, so `\"1\" {op} 1` is {}. Use `{strict}`.",
                    if op == "==" { "true" } else { "false" }
                ),
            );
            let mut fixed = code.to_string();
            fixed.replace_range(i..i + 2, strict);
            found.suggestion = Some(fixed);
            return Some(found);
        }
        i += 1;
    }
    None
}

/// A React effect with no dependency list runs after every render; if it
/// sets state, it renders again, forever.
fn effects_without_dependencies(piece: &Piece, head: Option<&str>) -> Vec<Lead> {
    let whole: Vec<Line>;
    let lines: Vec<&Line> = match head {
        Some(text) => {
            whole = text
                .lines()
                .zip(1u32..)
                .map(|(t, n)| Line {
                    kind: if piece.added_line(n).is_some() {
                        Kind::Added
                    } else {
                        Kind::Context
                    },
                    new: Some(n),
                    text: t.to_string(),
                })
                .collect();
            whole.iter().collect()
        }
        None => piece
            .lines
            .iter()
            .filter(|l| l.kind != Kind::Removed)
            .collect(),
    };
    let mut out = Vec::new();
    for (start, line) in lines.iter().enumerate() {
        let Some(at) = ["useEffect(", "useLayoutEffect("]
            .iter()
            .find_map(|k| line.text.find(k).map(|p| p + k.len()))
        else {
            continue;
        };
        // Follow the brackets from the call's "(" to the one closing it.
        let mut depth = 1i32;
        let mut seen = String::new();
        let mut end = None;
        'lines: for (i, l) in lines.iter().enumerate().skip(start) {
            let text = if i == start {
                &l.text[at..]
            } else {
                &l.text[..]
            };
            for c in text.chars() {
                match c {
                    '(' | '[' | '{' => depth += 1,
                    ')' | ']' | '}' => depth -= 1,
                    _ => {}
                }
                if depth == 0 {
                    end = Some(i);
                    break 'lines;
                }
                if depth == 1 {
                    seen.push(c);
                }
            }
        }
        let Some(end) = end else { continue };
        // At the call's own level the function has no comma of its own
        // (its parameters and body are deeper), so a comma there is the
        // second argument: the dependency list.
        let flat: String = seen.chars().filter(|c| !c.is_whitespace()).collect();
        let has_list = flat.contains(',');
        let changed = lines[start..=end].iter().any(|l| l.kind == Kind::Added);
        if !has_list && changed {
            if let Some(n) = lines[end].new {
                out.push(lead(
                    n,
                    "effect-dependencies",
                    "This effect has no dependency list, so it runs after every render. If it sets state, that render runs it again, without end. List what it reads, for example `[url]`, or `[]` to run it once.",
                ));
            }
        }
    }
    out
}

// ── A safety step taken out ─────────────────────────────────────────────────

/// A hunk removes `basename(` (or `file_name()`) and adds a path join
/// without it: a name that was cut to its last part now goes in whole, so
/// `../` in it leaves the folder.
fn safety_step_dropped(piece: &Piece) -> Vec<Lead> {
    const SAFE: [&str; 3] = ["basename(", "file_name()", "path.basename("];
    let removed = piece
        .lines
        .iter()
        .any(|l| l.kind == Kind::Removed && SAFE.iter().any(|s| l.text.contains(s)));
    let kept = piece
        .lines
        .iter()
        .any(|l| l.kind == Kind::Added && SAFE.iter().any(|s| l.text.contains(s)));
    if !removed || kept {
        return Vec::new();
    }
    piece
        .lines
        .iter()
        .filter(|l| {
            l.kind == Kind::Added
                && (l.text.contains("path.join(")
                    || l.text.contains(".join(")
                    || l.text.contains("Path::new("))
        })
        .filter_map(|l| l.new)
        .take(1)
        .map(|n| {
            lead(
                n,
                "path-traversal",
                "The name goes into the path whole now; the old code kept only its last part. A name like `../../etc/passwd` then points outside the folder. Keep only the last part (for example with `basename`) or refuse names with a `/`.",
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::patch::{parse, pieces};
    use super::*;

    fn piece(patch: &str) -> Piece {
        pieces(&parse(patch), 200).remove(0)
    }

    fn found(path: &str, patch: &str) -> Vec<(u32, &'static str)> {
        leads(path, &piece(patch), None)
            .into_iter()
            .map(|l| (l.line, l.check))
            .collect()
    }

    #[test]
    fn sql_built_with_values_is_flagged_and_parameters_are_not() {
        let p = "@@ -1,1 +1,3 @@\n fn a() {\n+    let sql = format!(\"SELECT login FROM users WHERE login LIKE '{prefix}%'\");\n+    conn.execute(\"DELETE FROM users WHERE login = ?1\", params![login])?;\n";
        assert_eq!(found("src/a.rs", p), vec![(2, "sql")]);
        let py = "@@ -1,1 +1,3 @@\n x\n+cur.execute(f\"SELECT * FROM t WHERE id = {id}\")\n+cur.execute(\"SELECT * FROM t WHERE id = %s\", (id,))\n";
        assert_eq!(found("a.py", py), vec![(2, "sql")]);
        let ts = "@@ -1,1 +1,2 @@\n x\n+db.query(`SELECT * FROM t WHERE id = ${id}`);\n";
        assert_eq!(found("a.ts", ts), vec![(2, "sql")]);
        let fine = "@@ -1,1 +1,2 @@\n x\n+println!(\"{count} rows SELECTed from the table\");\n";
        assert!(found("a.rs", fine).is_empty());
    }

    #[test]
    fn unquoted_variables_given_to_rm_are_flagged_with_the_quoted_line() {
        let p = "@@ -1,1 +1,4 @@\n set -eu\n+for d in $OLD; do\n+  rm -rf $TARGET/$d\n+  rm -rf \"$TARGET/$d\"\n";
        let all = leads("backup.sh", &piece(p), None);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].line, 3);
        assert!(all[0].message.contains("`$TARGET`, `$d`"));
        assert_eq!(
            all[0].suggestion.as_deref(),
            Some("  rm -rf \"$TARGET/$d\"")
        );
    }

    #[test]
    fn a_shared_default_list_in_python_is_flagged() {
        let p = "@@ -1,1 +1,3 @@\n x\n+def add_tag(name, tags=[]):\n+def other(name, tags=None):\n";
        assert_eq!(found("a.py", p), vec![(2, "mutable-default")]);
    }

    #[test]
    fn a_range_up_to_and_including_a_length_is_flagged() {
        let p = "@@ -1,1 +1,3 @@\n x\n+        for i in 0..=n.min(logins.len()) {\n+        for i in 0..=9 {\n";
        let all = leads("a.rs", &piece(p), None);
        assert_eq!(all.len(), 1);
        assert_eq!(
            all[0].suggestion.as_deref(),
            Some("        for i in 0..n.min(logins.len()) {")
        );
    }

    #[test]
    fn loose_comparisons_are_flagged_but_null_checks_are_not() {
        let p = "@@ -1,1 +1,4 @@\n x\n+  items.filter((x) => x.id == i.id)\n+  if (a == null) return;\n+  if (a === b) go();\n";
        let all = leads("a.tsx", &piece(p), None);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].line, 2);
        assert_eq!(
            all[0].suggestion.as_deref(),
            Some("  items.filter((x) => x.id === i.id)")
        );
    }

    #[test]
    fn an_effect_without_dependencies_is_flagged_where_it_closes() {
        let without = "@@ -8,6 +8,6 @@\n   useEffect(() => {\n     let live = true;\n     fetch(url).then(setItems);\n     return () => {\n       live = false;\n     };\n-  }, [url]);\n+  });\n";
        assert_eq!(
            found("list.tsx", without),
            vec![(14, "effect-dependencies")]
        );
        let with = "@@ -8,3 +8,4 @@\n   useEffect(() => {\n+    fetch(url).then(setItems);\n   }, [url]);\n";
        assert!(found("list.tsx", with).is_empty());
        // The effect opens above the diff's context: only the whole file
        // shows it.
        let far = "@@ -18,3 +18,3 @@\n       live = false;\n     };\n-  }, [url]);\n+  });\n";
        assert!(found("list.tsx", far).is_empty());
        let head = "x\n".repeat(13)
            + "  useEffect(() => {\n    let live = true;\n    fetch(url).then(setItems);\n    return () => {\n      live = false;\n    };\n  });\n";
        let all = leads("list.tsx", &piece(far), Some(&head));
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].line, 20);
        let multi = "@@ -8,3 +8,6 @@\n   useEffect(() => {\n+    go(a, b);\n   }, [\n+    a,\n+    b,\n+  ]);\n";
        assert!(found("list.tsx", multi).is_empty());
    }

    #[test]
    fn a_dropped_basename_is_flagged_on_the_join() {
        let p = "@@ -9,4 +9,3 @@\n def save_upload(folder, name, data):\n-    safe = os.path.basename(name)\n-    target = os.path.join(folder, safe)\n+    target = os.path.join(folder, name)\n     with open(target, \"wb\") as f:\n";
        assert_eq!(found("upload.py", p), vec![(10, "path-traversal")]);
    }

    #[test]
    fn keys_written_into_code_are_flagged() {
        let p = "@@ -1,1 +1,2 @@\n x\n+TOKEN = \"ghp_abcdefghijklmnopqrstuvwxyz0123456789\"\n";
        assert_eq!(found("a.py", p), vec![(2, "secret")]);
    }

    #[test]
    fn comments_are_not_code() {
        let p = "@@ -1,1 +1,3 @@\n x\n+# rm -rf $TARGET\n+// x == y\n";
        assert!(found("a.sh", p).is_empty());
        assert!(found("a.ts", "@@ -1,1 +1,2 @@\n x\n+// x == y\n").is_empty());
    }
}
