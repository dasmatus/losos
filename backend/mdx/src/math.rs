//! Equations as MathML, written here so a page draws them with no script:
//! browsers lay MathML Core out themselves. math-core turns the TeX into
//! MathML and keeps the TeX beside it as an annotation, which Pandoc reads
//! back on export. Its output still passes the allowlist (`sanitize.rs`).

use math_core::{LatexToMathML, MathCoreConfig, MathDisplay};
use std::sync::OnceLock;

/// The longest equation converted. A longer one stays TeX, as text.
pub const MAX_TEX: usize = 4096;

/// How deep braces and environments may nest in one equation. math-core
/// recurses as it parses, so this keeps a hostile equation off the stack's
/// end; nobody writes twenty.
pub const MAX_TEX_NESTING: usize = 32;

fn converter() -> &'static LatexToMathML {
    static C: OnceLock<LatexToMathML> = OnceLock::new();
    C.get_or_init(|| {
        let config = MathCoreConfig {
            annotation: true,
            // An unknown command is drawn as itself, marked, rather than
            // losing the whole equation.
            ignore_unknown_commands: true,
            id_prefix: "mdx-eq-".into(),
            ..Default::default()
        };
        LatexToMathML::new(config, Vec::new()).expect("no macros to parse")
    })
}

/// How deep `tex` nests: braces, `\left`, `\begin`, and commands that take
/// the next token as an argument without braces (`\sqrt\sqrt\sqrt x`).
fn nesting(tex: &str) -> usize {
    let (mut depth, mut deepest, mut run) = (0usize, 0usize, 0usize);
    let mut chars = tex.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            '\\' => {
                let mut name = String::new();
                while let Some(&n) = chars.peek() {
                    if !n.is_ascii_alphabetic() {
                        break;
                    }
                    name.push(n);
                    chars.next();
                }
                // A run of commands with nothing between them nests.
                run = match name.as_str() {
                    "left" | "begin" => {
                        depth += 1;
                        0
                    }
                    "right" | "end" => {
                        depth = depth.saturating_sub(1);
                        0
                    }
                    "" => {
                        chars.next();
                        0
                    }
                    _ => run + 1,
                };
                deepest = deepest.max(depth + run);
                continue;
            }
            _ => {}
        }
        if !c.is_whitespace() {
            run = 0;
        }
        deepest = deepest.max(depth);
    }
    deepest
}

/// `tex` as a `<math>` element, or why it cannot be one.
pub fn mathml(tex: &str, block: bool) -> Result<String, String> {
    if tex.len() > MAX_TEX {
        return Err(format!("longer than {MAX_TEX} bytes"));
    }
    if nesting(tex) > MAX_TEX_NESTING {
        return Err(format!("nested deeper than {MAX_TEX_NESTING} levels"));
    }
    let display = if block {
        MathDisplay::Block
    } else {
        MathDisplay::Inline
    };
    converter()
        .convert_with_local_state(tex, display)
        .map(|r| r.mathml)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nesting_counts_braces_environments_and_runs() {
        assert_eq!(nesting("x^2"), 0);
        assert_eq!(nesting("\\frac{a}{b}"), 1);
        assert_eq!(nesting("{{{x}}}"), 3);
        assert_eq!(nesting("\\left( \\left( x \\right) \\right)"), 2);
        assert_eq!(nesting("\\sqrt\\sqrt\\sqrt x"), 3);
        assert_eq!(nesting("\\alpha + \\beta"), 1);
    }
}
