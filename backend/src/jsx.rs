//! An app the owner keeps on the box, as code: one React component in JSX,
//! checked and compiled here.
//!
//! The builder's model writes an app as JSX (`builder-app-prompt.md`), and an
//! owner may edit that by hand. Either way the text has to become a script
//! the app frame can run (`admin-ui/app/public/app-frame/`), and the frame
//! has no compiler: its policy allows the app's own script and nothing that
//! evaluates more. So the box compiles it, once, when the app is written,
//! with oxc, and keeps both the source and the script.
//!
//! The same pass is the builder's check. A small model gets the shape of an
//! app right far more often than the names in it, so [`check`] reports, as
//! sentences the model can act on, what would stop the app from running: a
//! syntax error with its line, a component or a name that does not exist, an
//! import of a package the frame does not have, storage that does not work
//! in the frame, no `App` to mount.
//!
//! # What the app sees
//!
//! The compiled script runs inside a block in the frame, after these names
//! are bound: React's hooks ([`HOOKS`]), the LosOS components
//! ([`COMPONENTS`]), the data hooks and the `losos` object ([`DATA`]). The
//! frame sets `window.React` and `window.LosOS`, and the wrapper reads them
//! from there. Code that declares one of those names itself (a model that
//! writes `const { useState } = React;`) shadows the outer binding, which is
//! the same thing.

use oxc::allocator::Allocator;
use oxc::ast::ast::{ExportDefaultDeclarationKind, ModuleDeclaration, Statement};
use oxc::codegen::Codegen;
use oxc::parser::Parser;
use oxc::semantic::{ScopeFlags, SemanticBuilder, SymbolFlags};
use oxc::span::{GetSpan, SourceType, Span};
use oxc::transformer::{JsxRuntime, TransformOptions, Transformer};
use serde::Serialize;

/// React's hooks and helpers, bound before the app's code.
pub const HOOKS: &[&str] = &[
    "useState",
    "useEffect",
    "useMemo",
    "useRef",
    "useCallback",
    "useReducer",
    "useLayoutEffect",
    "Fragment",
];

/// The LosOS components (`admin-ui/app/src/app-frame/ui.tsx`).
pub const COMPONENTS: &[&str] = &[
    "Page", "Card", "Stack", "Row", "Text", "Heading", "Button", "Input", "TextArea", "Select",
    "Checkbox", "Badge", "Progress", "Stat", "Table", "Empty",
];

/// The data hooks and the box (`admin-ui/app/src/app-frame/data.ts`).
pub const DATA: &[&str] = &["useCollection", "useStored", "losos"];

/// What a browser gives every script, as far as an app has any business
/// with it. A name outside this list, the three above and the app's own is
/// a mistake.
const BROWSER: &[&str] = &[
    "React",
    "window",
    "document",
    "console",
    "globalThis",
    "Math",
    "Date",
    "JSON",
    "Intl",
    "Number",
    "String",
    "Boolean",
    "Array",
    "Object",
    "Symbol",
    "BigInt",
    "Map",
    "Set",
    "WeakMap",
    "WeakSet",
    "Promise",
    "RegExp",
    "Error",
    "TypeError",
    "RangeError",
    "Infinity",
    "NaN",
    "undefined",
    "isNaN",
    "isFinite",
    "parseInt",
    "parseFloat",
    "encodeURIComponent",
    "decodeURIComponent",
    "encodeURI",
    "decodeURI",
    "fetch",
    "Response",
    "Headers",
    "URL",
    "URLSearchParams",
    "AbortController",
    "setTimeout",
    "clearTimeout",
    "setInterval",
    "clearInterval",
    "requestAnimationFrame",
    "cancelAnimationFrame",
    "queueMicrotask",
    "structuredClone",
    "crypto",
    "navigator",
    "performance",
    "Blob",
    "File",
    "FileReader",
    "TextEncoder",
    "TextDecoder",
    "Audio",
    "Image",
    "Event",
    "KeyboardEvent",
    "Notification",
    "atob",
    "btoa",
];

/// What does not work inside the frame, and what to use instead.
const UNUSABLE: &[(&str, &str)] = &[
    (
        "localStorage",
        "keep data with useCollection or useStored instead",
    ),
    (
        "sessionStorage",
        "keep data with useCollection or useStored instead",
    ),
    (
        "indexedDB",
        "keep data with useCollection or useStored instead",
    ),
    (
        "alert",
        "show the message in the page, for example in a <Text>",
    ),
    ("confirm", "show a second button that confirms instead"),
    ("prompt", "use an <Input> in the page instead"),
];

/// Packages an import may name without it being a mistake: React itself
/// and LosOS. Their names are in scope anyway, so the import is dropped.
fn is_known_package(source: &str) -> bool {
    matches!(source, "react" | "react-dom" | "react-dom/client" | "losos")
        || source.starts_with("@losos/")
        || source.starts_with("losos/")
}

/// The name the frame mounts.
pub const ENTRY: &str = "App";

/// A checked app: the script the frame runs, or the problems that stop it
/// from being made, and the problems that would stop it from running.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Checked {
    /// The compiled script, when the source parses.
    pub code: Option<String>,
    /// Each one a sentence, for the owner and for the model's second turn.
    pub problems: Vec<String>,
}

impl Checked {
    #[must_use]
    pub fn ok(&self) -> bool {
        self.code.is_some() && self.problems.is_empty()
    }
}

/// The line, from 1, that a byte offset is on.
fn line_of(source: &str, offset: u32) -> usize {
    let end = (offset as usize).min(source.len());
    source[..end].matches('\n').count() + 1
}

/// The text of a line, trimmed and cut short, for a sentence about it.
fn line_text(source: &str, line: usize) -> String {
    let text = source.lines().nth(line - 1).unwrap_or("").trim();
    if text.chars().count() > 80 {
        let cut: String = text.chars().take(77).collect();
        format!("{cut}...")
    } else {
        text.to_string()
    }
}

/// The first place a name stands as a word in the source, as a line.
fn first_use(source: &str, name: &str) -> Option<usize> {
    let bytes = source.as_bytes();
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$';
    let mut from = 0;
    while let Some(at) = source[from..].find(name) {
        let start = from + at;
        let end = start + name.len();
        let before = start == 0 || !word(bytes[start - 1]);
        let after = end >= bytes.len() || !word(bytes[end]);
        if before && after {
            return Some(line_of(source, u32::try_from(start).unwrap_or(u32::MAX)));
        }
        from = end;
    }
    None
}

/// Replace a span with spaces, newlines kept, so every line keeps its number.
fn blank(text: &mut [u8], span: Span) {
    let (start, end) = (span.start as usize, (span.end as usize).min(text.len()));
    for b in &mut text[start..end] {
        if *b != b'\n' {
            *b = b' ';
        }
    }
}

/// Drop what a frame without modules cannot have: imports, and the
/// `export` words in front of declarations. An import of anything but React
/// or LosOS is a problem as well, since the names it brings do not exist.
fn strip_module(source: &str, problems: &mut Vec<String>) -> Option<String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::jsx()).parse();
    if !parsed.diagnostics.is_empty() {
        return None;
    }
    let mut text = source.as_bytes().to_vec();
    for statement in &parsed.program.body {
        let Some(module) = statement.as_module_declaration() else {
            continue;
        };
        match module {
            ModuleDeclaration::ImportDeclaration(import) => {
                let package = import.source.value.as_str().unwrap_or("");
                if !is_known_package(package) {
                    problems.push(format!(
                        "Line {}: the app imports \"{package}\", but apps cannot use packages. Use only React's hooks and the LosOS components, without any import.",
                        line_of(source, import.span.start)
                    ));
                }
                blank(&mut text, import.span);
            }
            ModuleDeclaration::ExportDefaultDeclaration(export) => match &export.declaration {
                ExportDefaultDeclarationKind::FunctionDeclaration(f) => {
                    blank(&mut text, Span::new(export.span.start, f.span.start));
                }
                ExportDefaultDeclarationKind::ClassDeclaration(c) => {
                    blank(&mut text, Span::new(export.span.start, c.span.start));
                }
                _ => blank(&mut text, export.span),
            },
            ModuleDeclaration::ExportDeclaration(export) => {
                blank(
                    &mut text,
                    Span::new(export.span.start, export.declaration.span().start),
                );
            }
            ModuleDeclaration::ExportNamedDeclaration(export) => blank(&mut text, export.span),
            other => blank(&mut text, other.span()),
        }
    }
    String::from_utf8(text).ok()
}

/// The top-level names the code declares.
fn top_level_names(program: &oxc::ast::ast::Program<'_>) -> Vec<String> {
    let mut names = Vec::new();
    for statement in &program.body {
        match statement {
            Statement::FunctionDeclaration(f) => {
                if let Some(id) = &f.id {
                    names.push(id.name.to_string());
                }
            }
            Statement::ClassDeclaration(c) => {
                if let Some(id) = &c.id {
                    names.push(id.name.to_string());
                }
            }
            Statement::VariableDeclaration(v) => {
                for d in &v.declarations {
                    names.extend(
                        d.id.get_binding_identifiers()
                            .iter()
                            .map(|b| b.name.to_string()),
                    );
                }
            }
            _ => {}
        }
    }
    names
}

fn is_component_name(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
}

/// Check an app's source and compile it.
#[must_use]
pub fn check(source: &str) -> Checked {
    let mut problems = Vec::new();
    if source.trim().is_empty() {
        return Checked {
            code: None,
            problems: vec!["The app is empty. Write it as function App() { ... }.".into()],
        };
    }

    // Syntax first: nothing else can be said about code that does not parse.
    {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, SourceType::jsx()).parse();
        if !parsed.diagnostics.is_empty() {
            for d in parsed.diagnostics.iter().take(3) {
                let start = d.labels.as_ref().first().map_or(0, |l| l.offset());
                let line = line_of(source, start);
                problems.push(format!(
                    "Line {line} has a syntax error ({}): {}",
                    d.message.trim_end_matches('.'),
                    line_text(source, line)
                ));
            }
            if problems.is_empty() {
                problems.push("The code does not parse as JSX.".into());
            }
            return Checked {
                code: None,
                problems,
            };
        }
    }

    let Some(stripped) = strip_module(source, &mut problems) else {
        return Checked {
            code: None,
            problems: vec!["The code does not parse as JSX.".into()],
        };
    };

    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, &stripped, SourceType::jsx()).parse();
    if !parsed.diagnostics.is_empty() {
        return Checked {
            code: None,
            problems: vec!["The code does not parse as JSX.".into()],
        };
    }
    let mut program = parsed.program;

    let declared = top_level_names(&program);
    let entry = if declared.iter().any(|n| n == ENTRY) {
        Some(ENTRY.to_string())
    } else {
        // A model that names the component after the app (`TodoApp`) is
        // understood: the last component declared at the top is the app.
        declared
            .iter()
            .rev()
            .find(|n| is_component_name(n))
            .cloned()
    };
    if entry.is_none() {
        problems.push("There is no App component. Write the app as function App() { ... } that returns a <Page>.".into());
    }

    let semantic = SemanticBuilder::new()
        .with_build_nodes(true)
        .build(&program)
        .semantic;
    for (used, made, name) in used_before_made(&semantic, &stripped) {
        problems.push(format!(
            "Line {used}: {name} is used before line {made}, which makes it, so the app stops as it starts. Move line {made} above line {used}."
        ));
    }
    let scoping = semantic.into_scoping();
    let mut unknown: Vec<(usize, String)> = scoping
        .root_unresolved_references()
        .keys()
        .map(|name| name.to_string())
        .filter(|name| {
            !HOOKS.contains(&name.as_str())
                && !COMPONENTS.contains(&name.as_str())
                && !DATA.contains(&name.as_str())
                && !BROWSER.contains(&name.as_str())
        })
        .map(|name| (first_use(&stripped, &name).unwrap_or(0), name))
        .collect();
    unknown.sort();
    for (line, name) in unknown {
        let at = if line > 0 {
            format!("Line {line}: ")
        } else {
            String::new()
        };
        if let Some((_, instead)) = UNUSABLE.iter().find(|(n, _)| *n == name) {
            problems.push(format!(
                "{at}{name} does not work in an app on the box; {instead}."
            ));
        } else if is_component_name(&name) {
            problems.push(format!(
                "{at}there is no component called {name}. The components are {}. Use one of them or plain HTML elements, or write function {name}() yourself.",
                COMPONENTS.join(", ")
            ));
        } else if name.starts_with("use") {
            problems.push(format!(
                "{at}there is no hook called {name}. Use {} or {}.",
                HOOKS[..5].join(", "),
                DATA[..2].join(" or ")
            ));
        } else {
            problems.push(format!(
                "{at}{name} is used but never defined. Define it, or use a name that exists."
            ));
        }
    }
    for member in losos_members(&stripped) {
        problems.push(format!(
            "Line {}: losos.{member} does not exist. losos has only metric, lang and user.",
            first_use(&stripped, &format!("losos.{member}")).unwrap_or(1)
        ));
    }

    let mut options = TransformOptions::default();
    options.jsx.runtime = JsxRuntime::Classic;
    let transformed = Transformer::new(&allocator, std::path::Path::new("app.jsx"), &options)
        .build_with_scoping(scoping, &mut program);
    if !transformed.diagnostics.is_empty() {
        let line = transformed
            .diagnostics
            .first()
            .and_then(|d| d.labels.as_ref().first())
            .map_or(1, |l| line_of(&stripped, l.offset()));
        problems.push(format!(
            "Line {line}: {}",
            transformed.diagnostics[0].message.trim_end_matches('.')
        ));
        return Checked {
            code: None,
            problems,
        };
    }
    let body = Codegen::new().build(&program).code;
    let code = entry.map(|entry| wrap(&body, &entry));
    Checked { code, problems }
}

/// A `const`, `let` or class read before the line that makes it, in the
/// same function, as (line used, line made, name), in line order. Such a
/// read throws the moment the function runs (`const [child] =
/// useState(children[0])` above `const children = [...]`), and a small
/// model's fix writes it often. A read inside a function written further up
/// is left alone: that function may run only once the name is made.
fn used_before_made(
    semantic: &oxc::semantic::Semantic<'_>,
    source: &str,
) -> Vec<(usize, usize, String)> {
    let scoping = semantic.scoping();
    let nodes = semantic.nodes();
    let body_of = |scope| {
        scoping
            .scope_ancestors(scope)
            .find(|&s| {
                let flags = scoping.scope_flags(s);
                flags.is_function() || flags.contains(ScopeFlags::Top)
            })
            .unwrap_or(scope)
    };
    let mut found = Vec::new();
    for symbol in scoping.symbol_ids() {
        let flags = scoping.symbol_flags(symbol);
        if !flags.intersects(SymbolFlags::BlockScopedVariable | SymbolFlags::Class) {
            continue;
        }
        let made = scoping.symbol_span(symbol).start;
        let body = body_of(scoping.symbol_scope_id(symbol));
        let early = scoping.get_resolved_references(symbol).find_map(|r| {
            let node = nodes.get_node(r.node_id());
            (node.span().start < made && body_of(node.scope_id()) == body)
                .then(|| node.span().start)
        });
        if let Some(used) = early {
            found.push((
                line_of(source, used),
                line_of(source, made),
                scoping.symbol_name(symbol).to_string(),
            ));
        }
    }
    found.sort();
    found
}

/// The compiled app inside the bindings it was promised, mounted.
fn wrap(body: &str, entry: &str) -> String {
    format!(
        "(function () {{\n\"use strict\";\nconst {{ {hooks} }} = window.React;\nconst {{ {components} }} = window.LosOS.ui;\nconst {{ {data} }} = window.LosOS;\n{{\n{body}\nwindow.LosOS.mount({entry});\n}}\n}})();\n",
        hooks = HOOKS.join(", "),
        components = COMPONENTS.join(", "),
        data = DATA.join(", "),
    )
}

/// `losos.x` members the frame's object does not have.
fn losos_members(source: &str) -> Vec<String> {
    const MEMBERS: [&str; 3] = ["metric", "lang", "user"];
    let bytes = source.as_bytes();
    let mut found: Vec<String> = Vec::new();
    let mut from = 0;
    while let Some(at) = source[from..].find("losos.") {
        let start = from + at;
        from = start + "losos.".len();
        let word_before = start > 0
            && (bytes[start - 1].is_ascii_alphanumeric()
                || bytes[start - 1] == b'_'
                || bytes[start - 1] == b'.');
        if word_before {
            continue;
        }
        let member: String = source[from..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if !member.is_empty() && !MEMBERS.contains(&member.as_str()) && !found.contains(&member) {
            found.push(member);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"function App() {
  const notes = useCollection("notes");
  const [text, setText] = useState("");
  return (
    <Page title="Notes">
      <Input label="Note" value={text} onChange={e => setText(e.target.value)} />
      <Button onClick={() => notes.add({ text })}>Add</Button>
      {notes.items.map(n => <Text key={n.id}>{n.text}</Text>)}
    </Page>
  );
}"#;

    #[test]
    fn an_app_that_uses_only_what_the_frame_has_compiles_and_is_mounted() {
        let checked = check(GOOD);
        assert!(checked.problems.is_empty(), "{:?}", checked.problems);
        let code = checked.code.expect("compiled");
        assert!(code.contains("React.createElement(Page"), "{code}");
        assert!(code.contains("window.LosOS.mount(App);"));
        assert!(!code.contains("<Page"));
    }

    #[test]
    fn a_syntax_error_is_named_with_its_line() {
        let source = "function App() {\n  return <Page title=\"x\">\n    <Button onClick={() => go(}>Go</Button>\n  </Page>;\n}";
        let checked = check(source);
        assert!(checked.code.is_none());
        assert!(
            checked.problems[0].starts_with("Line 3 has a syntax error"),
            "{:?}",
            checked.problems
        );
    }

    #[test]
    fn a_component_or_name_that_does_not_exist_is_a_problem() {
        let source = "function App() {\n  const x = useFetch('/a');\n  return <Page><DataGrid rows={rowz} /></Page>;\n}";
        let checked = check(source);
        assert!(checked.code.is_some(), "it still compiles");
        let all = checked.problems.join("\n");
        assert!(
            all.contains("Line 2: there is no hook called useFetch"),
            "{all}"
        );
        assert!(
            all.contains("Line 3: there is no component called DataGrid"),
            "{all}"
        );
        assert!(
            all.contains("Line 3: rowz is used but never defined"),
            "{all}"
        );
    }

    #[test]
    fn a_name_read_before_the_line_that_makes_it_is_named_with_both_lines() {
        // A small model's fix, as it wrote it: the first choice is read
        // before the list exists, and the app stops as it starts.
        let source = "function App() {\n  const [child, setChild] = useState(children[0]);\n  const children = [\"Ann\", \"Bob\"];\n  return <Page><Text>{child}</Text></Page>;\n}";
        let checked = check(source);
        assert!(checked.code.is_some(), "it still compiles");
        assert_eq!(
            checked.problems,
            ["Line 2: children is used before line 3, which makes it, so the app stops as it starts. Move line 3 above line 2."]
        );
        // Read inside a handler written above it, the name is made by the
        // time the handler runs; and a function is there from the start.
        let fine = "function App() {\n  function add() { items.add({ n: count }); }\n  const items = useCollection(\"items\");\n  const count = total();\n  function total() { return 1; }\n  return <Page><Button onClick={add}>Add</Button></Page>;\n}";
        assert_eq!(check(fine).problems, Vec::<String>::new());
    }

    #[test]
    fn storage_and_dialogs_that_do_not_work_in_the_frame_are_named() {
        let source = "function App() {\n  localStorage.setItem('a', '1');\n  alert('hi');\n  return <Page />;\n}";
        let all = check(source).problems.join("\n");
        assert!(all.contains("localStorage does not work"), "{all}");
        assert!(all.contains("alert does not work"), "{all}");
    }

    #[test]
    fn imports_of_react_and_exports_are_dropped_and_other_packages_are_problems() {
        let source = "import React, { useState } from 'react';\nimport { Card } from '@mui/material';\n\nexport default function App() {\n  const [n] = useState(0);\n  return <Page><Card>{n}</Card></Page>;\n}";
        let checked = check(source);
        assert_eq!(checked.problems.len(), 1, "{:?}", checked.problems);
        assert!(checked.problems[0].contains("Line 2: the app imports \"@mui/material\""));
        let code = checked.code.expect("compiled");
        assert!(!code.contains("import "), "{code}");
        assert!(!code.contains("export"), "{code}");
    }

    #[test]
    fn a_component_named_after_the_app_is_the_app() {
        let checked = check("function Header() { return <Text>Hi</Text>; }\nconst TodoApp = () => <Page><Header /></Page>;");
        assert!(checked.problems.is_empty(), "{:?}", checked.problems);
        assert!(checked
            .code
            .unwrap()
            .contains("window.LosOS.mount(TodoApp);"));
    }

    #[test]
    fn code_with_no_component_is_told_to_write_one() {
        let checked = check("const x = 1;");
        assert!(checked.code.is_none());
        assert!(checked.problems[0].contains("There is no App component"));
    }

    #[test]
    fn a_losos_member_the_frame_lacks_is_named() {
        let checked =
            check("function App() { losos.data.get('x'); return <Page>{losos.lang}</Page>; }");
        assert_eq!(checked.problems.len(), 1, "{:?}", checked.problems);
        assert!(checked.problems[0].contains("losos.data does not exist"));
    }

    #[test]
    fn the_app_declaring_a_hook_itself_still_compiles() {
        let checked = check("const { useState } = React;\nfunction App() { const [a] = useState(1); return <Page>{a}</Page>; }");
        assert!(checked.ok(), "{:?}", checked.problems);
    }
}
