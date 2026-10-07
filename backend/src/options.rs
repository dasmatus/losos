//! The `losos.*` option tree as a document, and the gate it puts on `apply`.
//!
//! `flake/options-doc.nix` walks the option declarations and
//! `modules/config-repo.nix` writes the result to `/etc/losos/options.json`
//! (`$LOSOS_OPTIONS_FILE`) at build time. This module reads that document,
//! joins the current `overrides.nix` assignments into it for
//! `GET /api/options`, and — the part that matters for safety — checks every
//! `losos.<key> = <value>;` line an `apply` carries against the editor kind
//! the declaration has.
//!
//! The admin UI draws the Advanced pane from the same document, so a key the
//! pane could not draw is a key this gate refuses, and the two cannot drift:
//! both read `editor.kind`, nothing else.
//!
//! [`check_body`] is deliberately a *second* gate behind
//! [`crate::overrides::validate_apply`]: that one knows the handful of values
//! whose wrong shape bricks the box (hostName, the compute window) and keeps
//! its sentences; this one knows every option's type. A body that passes
//! both still meets `nixos-rebuild`, which is the real parser.

use crate::overrides::assignments;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fmt;

/// Where `modules/config-repo.nix` writes the document.
pub const DEFAULT_OPTIONS_FILE: &str = "/etc/losos/options.json";
/// The document version this build reads; `flake/options-doc.nix` stamps it.
pub const VERSION: u64 = 1;

/// The editor a declaration gets, by its Nix type (`classify` in
/// `flake/options-doc.nix`). The JSON carries `kind` plus the fields below.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Editor {
    Bool,
    Str {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pattern: Option<String>,
        /// `"path"` for the secret-path options: a file on the box, typed as
        /// a string.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        form: Option<String>,
    },
    Int {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<i64>,
    },
    Float,
    Enum {
        values: Vec<String>,
    },
    /// A list of strings, one Nix list literal on one line.
    List,
    Nullable {
        inner: Box<Editor>,
    },
    /// Shown, never edited: a package, a path, an attribute set.
    Opaque {
        reason: String,
    },
}

/// One declared option.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionDoc {
    /// Dotted, without the `losos.` prefix: `cluster.enable`.
    pub name: String,
    /// The first path segment, or `general` for a top-level option.
    pub group: String,
    pub editor: Editor,
    pub nix_type: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub default: Value,
    #[serde(default)]
    pub default_text: Option<String>,
    /// What the last rebuild merged, from the configuration the document was
    /// built against.
    #[serde(default)]
    pub current: Value,
    /// The pane asks before changing one of these.
    #[serde(default)]
    pub danger: bool,
    /// Who sets the value at normal priority, when someone does: a line in
    /// `overrides.nix` would collide with it.
    #[serde(default)]
    pub fixed: Option<String>,
    #[serde(default)]
    pub read_only: bool,
}

/// The whole document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptionsDoc {
    pub version: u64,
    pub options: Vec<OptionDoc>,
    /// Prefixes left out, with the reason each was.
    #[serde(default)]
    pub excluded: Map<String, Value>,
}

impl OptionsDoc {
    pub fn find(&self, name: &str) -> Option<&OptionDoc> {
        self.options.iter().find(|o| o.name == name)
    }
}

/// Read the document as `modules/config-repo.nix` wrote it.
///
/// A version this build does not know is an error rather than a guess: a
/// document from a newer flake could carry an editor kind the gate below
/// would misread as "anything goes".
pub fn parse(text: &str) -> anyhow::Result<OptionsDoc> {
    let doc: OptionsDoc = serde_json::from_str(text)?;
    if doc.version != VERSION {
        anyhow::bail!(
            "options document is version {}, this lososd reads version {VERSION}",
            doc.version
        );
    }
    Ok(doc)
}

/// Keys an older `overrides.nix` may still spell the old way, and the option
/// each one is today. [`crate::overrides::parse_settings`] honours the same
/// two, so a file that reads back fine there must pass the gate here too.
const ALIASES: &[(&str, &str)] = &[
    ("aio.apachePort", "nextcloud.apachePort"),
    ("cfd.enable", "proxy.enable"),
];

fn canonical(key: &str) -> &str {
    ALIASES
        .iter()
        .find(|(old, _)| *old == key)
        .map_or(key, |(_, new)| new)
}

/// `GET /api/options`: the document with the current `overrides.nix`
/// assignments joined in.
///
/// Each option gains `set`: the raw Nix literal `overrides.nix` assigns it,
/// or `null` when the file leaves it at its default. Lines the document has
/// no option for come back under `stray`, so the pane can say they exist
/// rather than silently dropping them on the next Apply.
pub fn join(doc: &OptionsDoc, overrides: &str) -> Value {
    let set = assignments(overrides);
    let options: Vec<Value> = doc
        .options
        .iter()
        .map(|o| {
            let mut v = serde_json::to_value(o).unwrap_or_else(|_| json!({}));
            let raw = set
                .iter()
                .find(|(k, _)| canonical(k) == o.name)
                .map(|(_, v)| Value::String(v.clone()))
                .unwrap_or(Value::Null);
            if let Some(obj) = v.as_object_mut() {
                obj.insert("set".to_string(), raw);
            }
            v
        })
        .collect();
    let stray: Vec<Value> = set
        .iter()
        .filter(|(k, _)| doc.find(canonical(k)).is_none())
        .map(|(k, v)| json!({ "key": k, "value": v }))
        .collect();
    json!({
        "available": true,
        "version": doc.version,
        "options": options,
        "stray": stray,
        "excluded": doc.excluded,
    })
}

/// An `apply` body the gate turned down, with the sentence for the owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejected(pub String);

impl fmt::Display for Rejected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Rejected {}

/// Check every assignment in an `apply` body against the document.
///
/// Three things are refused, each with the key in the sentence: a key the box
/// declares no option for (a typo, or an option from another build), a key
/// the document marks read-only (set by the installer at normal priority, or
/// a package chosen by the flake), and a literal that does not fit the
/// option's editor kind. `${` anywhere in a value is refused outright: the
/// body is evaluated by `nixos-rebuild` as root, and
/// `"${builtins.readFile "/var/secrets/…"}"` is a string by this parser's
/// lights.
///
/// Lines the body carries that are not assignments — the header, braces,
/// comments — are not this function's business; [`crate::overrides::
/// validate_apply`] handles the shape of the file.
pub fn check_body(doc: &OptionsDoc, body: &str) -> Result<(), Rejected> {
    for (key, raw) in assignments(body) {
        let name = canonical(&key);
        let Some(opt) = doc.find(name) else {
            return Err(Rejected(format!(
                "losos.{key} is not an option this box declares"
            )));
        };
        if let Some(owner) = &opt.fixed {
            return Err(Rejected(format!(
                "losos.{key} is set by the {owner} and cannot be changed from here"
            )));
        }
        if let Editor::Opaque { reason } = &opt.editor {
            return Err(Rejected(format!(
                "losos.{key} is a {reason}; it is chosen by the flake, not by a setting"
            )));
        }
        if raw.contains("${") {
            return Err(Rejected(format!(
                "losos.{key} must not contain Nix interpolation"
            )));
        }
        if let Err(why) = fits(&opt.editor, &raw) {
            return Err(Rejected(format!("losos.{key} {why}")));
        }
    }
    Ok(())
}

/// Whether `raw`, one Nix literal, is a value of the editor's kind.
///
/// The sentences read after `losos.<key> `, so each starts with a verb.
pub fn fits(editor: &Editor, raw: &str) -> Result<(), String> {
    match editor {
        Editor::Bool => match raw {
            "true" | "false" => Ok(()),
            _ => Err("must be true or false".to_string()),
        },
        Editor::Int { min, max } => {
            let n: i64 = raw
                .parse()
                .map_err(|_| "must be a whole number".to_string())?;
            if min.is_some_and(|m| n < m) || max.is_some_and(|m| n > m) {
                return Err(match (min, max) {
                    (Some(a), Some(b)) => format!("must be between {a} and {b}"),
                    (Some(a), None) => format!("must be at least {a}"),
                    (None, Some(b)) => format!("must be at most {b}"),
                    (None, None) => unreachable!("a bound was violated"),
                });
            }
            Ok(())
        }
        Editor::Float => {
            // Nix's `isFloat` is false for `1`, so the literal must carry a
            // point: `1.0`, `0.25`, `-2.5`.
            let digits = raw.strip_prefix('-').unwrap_or(raw);
            let ok = digits.split_once('.').is_some_and(|(a, b)| {
                !a.is_empty()
                    && !b.is_empty()
                    && a.bytes().all(|c| c.is_ascii_digit())
                    && b.bytes().all(|c| c.is_ascii_digit())
            });
            if ok {
                Ok(())
            } else {
                Err("must be a decimal number such as 0.25".to_string())
            }
        }
        Editor::Str { .. } => nix_string(raw)
            .map(|_| ())
            .ok_or_else(|| "must be a double-quoted string".to_string()),
        Editor::Enum { values } => {
            let s = nix_string(raw).ok_or_else(|| "must be a double-quoted string".to_string())?;
            if values.contains(&s) {
                Ok(())
            } else {
                Err(format!("must be one of {}", values.join(", ")))
            }
        }
        Editor::List => nix_string_list(raw).map(|_| ()).ok_or_else(|| {
            "must be a list of double-quoted strings, such as [ \"a\" \"b\" ]".to_string()
        }),
        Editor::Nullable { inner } => {
            if raw == "null" {
                Ok(())
            } else {
                fits(inner, raw).map_err(|why| format!("{why}, or null"))
            }
        }
        Editor::Opaque { reason } => Err(format!("is a {reason} and has no editor")),
    }
}

/// The content of one double-quoted Nix string literal, or `None` when `raw`
/// is not exactly one.
///
/// Escapes are the four Nix knows in `"…"` strings (`\"`, `\\`, `\n`, `\t`,
/// `\r`, `\$`); anything else after a backslash is kept as-is, which is what
/// Nix does too.
pub fn nix_string(raw: &str) -> Option<String> {
    let inner = raw.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                other => out.push(other),
            },
            // An unescaped quote means `raw` was two strings, not one.
            '"' => return None,
            other => out.push(other),
        }
    }
    Some(out)
}

/// The elements of one `[ "a" "b" ]` literal, or `None` when `raw` is not
/// exactly that shape: one list, strings only, on one line.
pub fn nix_string_list(raw: &str) -> Option<Vec<String>> {
    let inner = raw.strip_prefix('[')?.strip_suffix(']')?;
    let mut items = Vec::new();
    let mut rest = inner.trim_start();
    while !rest.is_empty() {
        if !rest.starts_with('"') {
            return None;
        }
        // Find the closing quote, skipping escaped ones.
        let bytes = rest.as_bytes();
        let mut i = 1;
        loop {
            match bytes.get(i)? {
                b'\\' => i += 2,
                b'"' => break,
                _ => i += 1,
            }
        }
        let (lit, after) = rest.split_at(i + 1);
        items.push(nix_string(lit)?);
        if !after.is_empty() && !after.starts_with(char::is_whitespace) {
            return None;
        }
        rest = after.trim_start();
    }
    Some(items)
}

/// Format a JSON value as the Nix literal an editor of this kind expects —
/// the inverse of [`fits`], used by the fake and by tests, and what the
/// admin UI's `lib/option-value.ts` does for the browser.
pub fn to_literal(editor: &Editor, value: &Value) -> Option<String> {
    match (editor, value) {
        (Editor::Bool, Value::Bool(b)) => Some(b.to_string()),
        (Editor::Int { .. }, Value::Number(n)) => n.as_i64().map(|i| i.to_string()),
        (Editor::Float, Value::Number(n)) => n.as_f64().map(|f| {
            if f.fract() == 0.0 {
                format!("{f:.1}")
            } else {
                f.to_string()
            }
        }),
        (Editor::Str { .. } | Editor::Enum { .. }, Value::String(s)) => Some(quote(s)),
        (Editor::List, Value::Array(items)) => {
            let parts: Option<Vec<String>> = items.iter().map(|i| i.as_str().map(quote)).collect();
            parts.map(|p| {
                if p.is_empty() {
                    "[ ]".to_string()
                } else {
                    format!("[ {} ]", p.join(" "))
                }
            })
        }
        (Editor::Nullable { .. }, Value::Null) => Some("null".to_string()),
        (Editor::Nullable { inner }, v) => to_literal(inner, v),
        _ => None,
    }
}

/// `s` as a Nix string literal.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A small document in the shape `flake/options-doc.nix` produces.
    pub(crate) fn sample() -> OptionsDoc {
        parse(
            r#"{
              "version": 1,
              "options": [
                {"name":"sharingMyStorage","group":"general","editor":{"kind":"bool"},"nixType":"boolean",
                 "default":true,"current":true,"danger":false,"fixed":null,"readOnly":false},
                {"name":"hostName","group":"general","editor":{"kind":"str"},"nixType":"string",
                 "description":"The box's name.","default":"mattbox","current":"mattbox","danger":true,"fixed":null,"readOnly":false},
                {"name":"forgejo.enable","group":"forgejo","editor":{"kind":"bool"},"nixType":"boolean",
                 "default":true,"current":true,"danger":false,"fixed":null,"readOnly":false},
                {"name":"forgejo.mode","group":"forgejo","editor":{"kind":"enum","values":["native","container"]},"nixType":"one of",
                 "default":"container","current":"container","danger":true,"fixed":null,"readOnly":false},
                {"name":"storage.fillPercent","group":"storage","editor":{"kind":"int","min":50,"max":100},"nixType":"integer",
                 "default":90,"current":90,"danger":true,"fixed":null,"readOnly":false},
                {"name":"cluster.idleLoadThreshold","group":"cluster","editor":{"kind":"float"},"nixType":"float",
                 "default":0.25,"current":0.25,"danger":true,"fixed":null,"readOnly":false},
                {"name":"cache.substituters","group":"cache","editor":{"kind":"list"},"nixType":"list of string",
                 "default":["https://proxy.losos.dasmat.us"],"current":["https://proxy.losos.dasmat.us"],"danger":true,"fixed":null,"readOnly":false},
                {"name":"proxy.noisePublicKeyFile","group":"proxy","editor":{"kind":"nullable","inner":{"kind":"str","form":"path"}},"nixType":"null or string",
                 "default":null,"current":"/var/secrets/x","danger":false,"fixed":null,"readOnly":false},
                {"name":"tpm.enable","group":"tpm","editor":{"kind":"bool"},"nixType":"boolean",
                 "default":true,"current":true,"danger":false,"fixed":"installer","readOnly":true},
                {"name":"backend.package","group":"backend","editor":{"kind":"opaque","reason":"null or package"},"nixType":"null or package",
                 "default":{"package":"losos-ctl"},"current":{"package":"losos-ctl"},"danger":true,"fixed":null,"readOnly":true},
                {"name":"nextcloud.apachePort","group":"nextcloud","editor":{"kind":"int","min":0,"max":65535},"nixType":"16 bit unsigned integer",
                 "default":11000,"current":11000,"danger":false,"fixed":null,"readOnly":false}
              ],
              "excluded": {"edge": "the VPS side"}
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn the_editor_kinds_round_trip_through_serde() {
        let doc = sample();
        assert_eq!(
            doc.find("hostName").unwrap().editor,
            Editor::Str {
                pattern: None,
                form: None
            }
        );
        assert_eq!(
            doc.find("storage.fillPercent").unwrap().editor,
            Editor::Int {
                min: Some(50),
                max: Some(100)
            }
        );
        assert_eq!(
            doc.find("proxy.noisePublicKeyFile").unwrap().editor,
            Editor::Nullable {
                inner: Box::new(Editor::Str {
                    pattern: None,
                    form: Some("path".into())
                })
            }
        );
        let back = serde_json::to_value(&doc.find("forgejo.mode").unwrap().editor).unwrap();
        assert_eq!(back, json!({"kind":"enum","values":["native","container"]}));
    }

    #[test]
    fn an_unknown_version_is_refused() {
        assert!(parse(r#"{"version": 2, "options": []}"#).is_err());
    }

    #[test]
    fn join_adds_the_set_literal_and_lists_strays() {
        let doc = sample();
        let out = join(
            &doc,
            "{ ... }:\n{\n  losos.hostName = \"box2\";\n  losos.aio.apachePort = 12000;\n  losos.ghost = 1;\n}\n",
        );
        let opts = out["options"].as_array().unwrap();
        let by = |n: &str| opts.iter().find(|o| o["name"] == n).unwrap().clone();
        assert_eq!(by("hostName")["set"], "\"box2\"");
        assert_eq!(by("forgejo.enable")["set"], Value::Null);
        // The legacy spelling lands on the option it became.
        assert_eq!(by("nextcloud.apachePort")["set"], "12000");
        assert_eq!(out["stray"], json!([{"key": "ghost", "value": "1"}]));
        assert_eq!(out["available"], true);
    }

    #[test]
    fn a_body_of_fitting_literals_passes() {
        let doc = sample();
        let body = "{ ... }:\n{\n  losos.hostName = \"box2\";\n  losos.forgejo.enable = false;\n  \
                    losos.forgejo.mode = \"native\";\n  losos.storage.fillPercent = 75;\n  \
                    losos.cluster.idleLoadThreshold = 0.5;\n  \
                    losos.cache.substituters = [ \"https://a\" \"https://b\" ];\n  \
                    losos.proxy.noisePublicKeyFile = null;\n  losos.cfd.enable = true;\n}\n";
        // `cfd.enable` is an alias of proxy.enable, which the sample does not
        // declare — so that line alone must be the one refused.
        let err = check_body(&doc, body).unwrap_err();
        assert!(err.0.contains("losos.cfd.enable"), "{err}");
        let body = body.replace("  losos.cfd.enable = true;\n", "");
        assert_eq!(check_body(&doc, &body), Ok(()));
    }

    #[test]
    fn an_undeclared_key_is_refused_by_name() {
        let err = check_body(&sample(), "{ losos.ghost = true; }").unwrap_err();
        assert_eq!(err.0, "losos.ghost is not an option this box declares");
    }

    #[test]
    fn the_installer_owned_and_package_options_are_refused() {
        let doc = sample();
        let err = check_body(&doc, "{ losos.tpm.enable = false; }").unwrap_err();
        assert!(err.0.contains("set by the installer"), "{err}");
        let err = check_body(&doc, "{ losos.backend.package = null; }").unwrap_err();
        assert!(err.0.contains("chosen by the flake"), "{err}");
    }

    #[test]
    fn literals_of_the_wrong_kind_are_refused_with_the_rule() {
        let doc = sample();
        let cases = [
            ("losos.forgejo.enable = \"yes\";", "must be true or false"),
            (
                "losos.storage.fillPercent = 10;",
                "must be between 50 and 100",
            ),
            (
                "losos.storage.fillPercent = \"90\";",
                "must be a whole number",
            ),
            ("losos.cluster.idleLoadThreshold = 1;", "must be a decimal"),
            (
                "losos.forgejo.mode = \"podman\";",
                "must be one of native, container",
            ),
            ("losos.hostName = box;", "must be a double-quoted string"),
            (
                "losos.cache.substituters = \"https://a\";",
                "must be a list",
            ),
            (
                "losos.cache.substituters = [ https://a ];",
                "must be a list",
            ),
            ("losos.proxy.noisePublicKeyFile = 3;", ", or null"),
        ];
        for (line, expect) in cases {
            let err = check_body(&doc, &format!("{{ {line} }}")).unwrap_err();
            assert!(err.0.contains(expect), "{line}: {err}");
        }
    }

    #[test]
    fn interpolation_is_refused_whatever_the_kind() {
        let err = check_body(
            &sample(),
            "{ losos.hostName = \"${builtins.readFile \"/var/secrets/losos-admin-token\"}\"; }",
        )
        .unwrap_err();
        assert!(err.0.contains("Nix interpolation"), "{err}");
    }

    #[test]
    fn nix_strings_unescape_and_refuse_two_strings() {
        assert_eq!(nix_string("\"a b\""), Some("a b".into()));
        assert_eq!(nix_string("\"say \\\"hi\\\"\""), Some("say \"hi\"".into()));
        assert_eq!(nix_string("\"a\" \"b\""), None);
        assert_eq!(nix_string("a"), None);
        assert_eq!(nix_string("\"unterminated"), None);
    }

    #[test]
    fn nix_string_lists_parse_and_refuse_bare_words() {
        assert_eq!(nix_string_list("[ ]"), Some(vec![]));
        assert_eq!(nix_string_list("[]"), Some(vec![]));
        assert_eq!(
            nix_string_list("[ \"a\" \"b c\" ]"),
            Some(vec!["a".into(), "b c".into()])
        );
        assert_eq!(nix_string_list("[\"a\"\"b\"]"), None);
        assert_eq!(nix_string_list("[ a ]"), None);
        assert_eq!(nix_string_list("[ \"a\" 1 ]"), None);
    }

    #[test]
    fn to_literal_is_the_inverse_of_fits() {
        let doc = sample();
        let cases: Vec<(&str, Value)> = vec![
            ("hostName", json!("box-2")),
            ("forgejo.enable", json!(false)),
            ("forgejo.mode", json!("native")),
            ("storage.fillPercent", json!(80)),
            ("cluster.idleLoadThreshold", json!(1.0)),
            ("cluster.idleLoadThreshold", json!(0.25)),
            ("cache.substituters", json!(["https://a", "https://b"])),
            ("cache.substituters", json!([])),
            ("proxy.noisePublicKeyFile", json!(null)),
            ("proxy.noisePublicKeyFile", json!("/var/secrets/x")),
        ];
        for (name, value) in cases {
            let editor = &doc.find(name).unwrap().editor;
            let lit = to_literal(editor, &value).unwrap_or_else(|| panic!("{name}: no literal"));
            assert_eq!(fits(editor, &lit), Ok(()), "{name}: {lit}");
        }
        assert_eq!(quote("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }
}
