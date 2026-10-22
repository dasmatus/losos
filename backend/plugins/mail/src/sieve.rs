//! A person's mail filters, and the Sieve script Stalwart runs for them.
//!
//! The filters are kept in one place only: the person's Sieve script on the
//! mail server, named [`SCRIPT_NAME`] and active. Its first line is a
//! comment carrying the filters as base64 JSON, so the page reads back
//! exactly what it saved; the rest is the same filters written as Sieve,
//! which Stalwart runs at delivery, with the page closed and nobody signed
//! in. A filter that is off is kept in the comment and left out of the
//! Sieve.
//!
//! Filters run top to bottom and the first one that matches decides: each
//! ends in `stop`, so a message is never filed into two folders. Its
//! actions are flags first, then where the message goes: Sieve files a
//! message with the flags set before `fileinto` (Stalwart applies none set
//! after it), and `keep` when it stays in the inbox.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The script's name on the server.
pub const SCRIPT_NAME: &str = "losos";
/// The comment the filters ride in, up to the base64.
pub const HEADER: &str = "# LosOS Mail filters: ";
/// How many filters a person keeps.
pub const MAX_FILTERS: usize = 100;
/// How many conditions one filter has.
pub const MAX_CONDITIONS: usize = 10;
/// The longest name or condition text, in characters.
pub const MAX_TEXT: usize = 200;

/// What a condition looks at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Field {
    /// The sender's address or name.
    From,
    /// A recipient, in To or Cc.
    To,
    Subject,
}

/// `field` contains `value`, letter case aside.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    pub field: Field,
    pub value: String,
}

/// Whether every condition must hold, or one is enough.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Match {
    #[default]
    All,
    Any,
}

/// One filter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Filter {
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default, rename = "match")]
    pub match_: Match,
    pub conditions: Vec<Condition>,
    /// The folder the message goes to, by id; `None` leaves it in the
    /// inbox. The bin is a folder like any other, which is how a filter
    /// deletes.
    #[serde(default)]
    pub move_to: Option<String>,
    #[serde(default)]
    pub mark_read: bool,
    #[serde(default)]
    pub flag: bool,
}

fn yes() -> bool {
    true
}

/// A request the page cannot have, in words it shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalid(pub String);

impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn invalid<T>(message: impl Into<String>) -> Result<T, Invalid> {
    Err(Invalid(message.into()))
}

/// Text a person typed: trimmed, not empty, short, no control characters.
fn text(what: &str, s: &str) -> Result<String, Invalid> {
    let s = s.trim();
    if s.is_empty() {
        return invalid(format!("the {what} is empty"));
    }
    if s.chars().count() > MAX_TEXT {
        return invalid(format!("the {what} is longer than {MAX_TEXT} characters"));
    }
    if s.chars().any(char::is_control) {
        return invalid(format!("the {what} has a character that cannot be typed"));
    }
    Ok(s.to_string())
}

/// A JMAP id as Stalwart makes them.
pub fn is_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 255
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// `PUT /api/mail/filters`'s body, `{"filters": [...]}`, checked. `folders`
/// is every folder id the person has: a filter may only file into one.
pub fn parse_filters(doc: &Value, folders: &[String]) -> Result<Vec<Filter>, Invalid> {
    let Some(list) = doc.get("filters") else {
        return invalid("no filters in the request");
    };
    let filters: Vec<Filter> = match serde_json::from_value(list.clone()) {
        Ok(f) => f,
        Err(e) => return invalid(format!("the filters are not in the right shape: {e}")),
    };
    if filters.len() > MAX_FILTERS {
        return invalid(format!("a person keeps at most {MAX_FILTERS} filters"));
    }
    filters
        .into_iter()
        .map(|f| {
            let name = text("filter's name", &f.name)?;
            if f.conditions.is_empty() {
                return invalid(format!("“{name}” has no condition"));
            }
            if f.conditions.len() > MAX_CONDITIONS {
                return invalid(format!(
                    "“{name}” has more than {MAX_CONDITIONS} conditions"
                ));
            }
            let conditions = f
                .conditions
                .into_iter()
                .map(|c| {
                    Ok(Condition {
                        field: c.field,
                        value: text("text to look for", &c.value)?,
                    })
                })
                .collect::<Result<Vec<_>, Invalid>>()?;
            if let Some(to) = &f.move_to {
                if !is_id(to) || !folders.iter().any(|id| id == to) {
                    return invalid(format!("“{name}” files into a folder that is not there"));
                }
            }
            if f.enabled && f.move_to.is_none() && !f.mark_read && !f.flag {
                return invalid(format!("“{name}” does nothing to the mail it finds"));
            }
            Ok(Filter {
                name,
                conditions,
                ..f
            })
        })
        .collect()
}

/// A Sieve quoted string: only `"` and `\` are escaped (RFC 5228 2.4.2).
fn quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

fn test(c: &Condition) -> String {
    let value = quoted(&c.value);
    match c.field {
        // The whole header, decoded: the name and the address both.
        Field::From => format!("header :contains \"from\" {value}"),
        Field::To => format!("header :contains [\"to\", \"cc\"] {value}"),
        Field::Subject => format!("header :contains \"subject\" {value}"),
    }
}

/// The script for `filters`, with `names` the current name of each folder
/// a filter files into (by id), which Sieve wants beside the id in case the
/// folder has gone by the time a message arrives.
pub fn script(filters: &[Filter], names: &dyn Fn(&str) -> Option<String>) -> String {
    let json = serde_json::to_vec(filters).unwrap_or_default();
    let mut out = format!("{HEADER}{}\n", crate::base64::encode(&json));
    let live: Vec<&Filter> = filters.iter().filter(|f| f.enabled).collect();
    if live.is_empty() {
        return out;
    }
    out.push_str("require [\"fileinto\", \"imap4flags\", \"mailbox\", \"mailboxid\"];\n");
    for f in live {
        let tests: Vec<String> = f.conditions.iter().map(test).collect();
        let joiner = match f.match_ {
            Match::All => "allof",
            Match::Any => "anyof",
        };
        // The name, as a comment, cut at a line break it cannot have.
        out.push_str(&format!("\n# {}\n", f.name.replace(['\r', '\n'], " ")));
        out.push_str(&format!("if {joiner} ({}) {{\n", tests.join(", ")));
        let mut flags = Vec::new();
        if f.mark_read {
            flags.push("\"\\\\Seen\"");
        }
        if f.flag {
            flags.push("\"\\\\Flagged\"");
        }
        if !flags.is_empty() {
            out.push_str(&format!("  addflag [{}];\n", flags.join(", ")));
        }
        match &f.move_to {
            Some(id) => {
                let fallback = names(id).unwrap_or_else(|| "INBOX".to_string());
                out.push_str(&format!(
                    "  fileinto :mailboxid {} {};\n",
                    quoted(id),
                    quoted(&fallback)
                ));
            }
            None => out.push_str("  keep;\n"),
        }
        out.push_str("  stop;\n}\n");
    }
    out
}

/// The filters a script carries in its first line. A script LosOS did not
/// write, or one whose comment does not read, carries none.
pub fn from_script(script: &str) -> Vec<Filter> {
    script
        .lines()
        .next()
        .and_then(|l| l.strip_prefix(HEADER))
        .and_then(|b| crate::base64::decode(b.trim()))
        .and_then(|j| serde_json::from_slice(&j).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn folders() -> Vec<String> {
        vec!["a".into(), "b".into(), "i".into()]
    }

    fn filter() -> Filter {
        Filter {
            name: "Eva".into(),
            enabled: true,
            match_: Match::All,
            conditions: vec![Condition {
                field: Field::From,
                value: "eva@".into(),
            }],
            move_to: Some("i".into()),
            mark_read: false,
            flag: true,
        }
    }

    #[test]
    fn a_filter_is_read_with_its_defaults() {
        let doc = json!({ "filters": [{
            "name": "  Eva  ",
            "conditions": [{ "field": "from", "value": " eva@ " }],
            "moveTo": "i",
            "flag": true,
        }] });
        assert_eq!(parse_filters(&doc, &folders()).unwrap(), vec![filter()]);
    }

    #[test]
    fn filters_that_cannot_work_are_refused_by_name() {
        let refused = |f: Value| {
            parse_filters(&json!({ "filters": [f] }), &folders())
                .unwrap_err()
                .0
        };
        let base = json!({ "name": "X", "conditions": [{ "field": "subject", "value": "a" }], "flag": true });
        let with = |k: &str, v: Value| {
            let mut f = base.clone();
            f[k] = v;
            f
        };
        assert!(refused(with("conditions", json!([]))).contains("no condition"));
        assert!(refused(with("moveTo", json!("zz"))).contains("not there"));
        assert!(refused(with("flag", json!(false))).contains("does nothing"));
        // One that is off may do nothing yet: a folder it filed into went.
        let mut off = with("flag", json!(false));
        off["enabled"] = json!(false);
        assert!(parse_filters(&json!({ "filters": [off] }), &folders()).is_ok());
        assert!(refused(with("name", json!(" "))).contains("empty"));
        assert!(refused(with("name", json!("a\u{7}b"))).contains("cannot be typed"));
        assert!(refused(with("name", json!("x".repeat(201)))).contains("longer"));
        assert!(refused(with(
            "conditions",
            json!([{ "field": "body", "value": "a" }])
        ))
        .contains("shape"));
        assert!(refused(with("extra", json!(1))).contains("shape"));
        let many = vec![base.clone(); MAX_FILTERS + 1];
        assert!(parse_filters(&json!({ "filters": many }), &folders()).is_err());
        assert!(parse_filters(&json!({}), &folders()).is_err());
    }

    #[test]
    fn the_script_sets_flags_before_it_files_and_stops() {
        let s = script(&[filter()], &|id| {
            (id == "i").then(|| "LosOS box".to_string())
        });
        let body: Vec<&str> = s.lines().skip(1).collect();
        assert_eq!(
            body,
            [
                r#"require ["fileinto", "imap4flags", "mailbox", "mailboxid"];"#,
                "",
                "# Eva",
                r#"if allof (header :contains "from" "eva@") {"#,
                r#"  addflag ["\\Flagged"];"#,
                r#"  fileinto :mailboxid "i" "LosOS box";"#,
                "  stop;",
                "}",
            ]
        );
    }

    #[test]
    fn a_filter_without_a_folder_keeps_the_message() {
        let f = Filter {
            match_: Match::Any,
            conditions: vec![
                Condition {
                    field: Field::To,
                    value: "team".into(),
                },
                Condition {
                    field: Field::Subject,
                    value: "say \"hi\" \\o/".into(),
                },
            ],
            move_to: None,
            mark_read: true,
            flag: true,
            ..filter()
        };
        let s = script(&[f], &|_| None);
        assert!(s.contains(r#"if anyof (header :contains ["to", "cc"] "team", header :contains "subject" "say \"hi\" \\o/") {"#), "{s}");
        assert!(s.contains(r#"  addflag ["\\Seen", "\\Flagged"];"#), "{s}");
        assert!(s.contains("  keep;\n  stop;"), "{s}");
    }

    #[test]
    fn a_folder_gone_falls_back_to_the_inbox() {
        let s = script(&[filter()], &|_| None);
        assert!(s.contains(r#"fileinto :mailboxid "i" "INBOX";"#), "{s}");
    }

    #[test]
    fn filters_that_are_off_ride_in_the_comment_only() {
        let off = Filter {
            enabled: false,
            ..filter()
        };
        let s = script(std::slice::from_ref(&off), &|_| None);
        assert_eq!(s.lines().count(), 1, "{s}");
        assert_eq!(from_script(&s), vec![off]);
    }

    #[test]
    fn the_filters_come_back_from_the_script() {
        let fs = vec![
            filter(),
            Filter {
                name: "Line\nbreak".into(),
                ..filter()
            },
        ];
        let s = script(&fs, &|_| Some("x".into()));
        assert_eq!(from_script(&s), fs);
        // The name's comment stays on one line.
        assert!(s.contains("# Line break\n"));
        assert_eq!(from_script("require \"fileinto\";\n"), vec![]);
        assert_eq!(from_script(&format!("{HEADER}not base64\n")), vec![]);
        assert_eq!(from_script(""), vec![]);
    }
}
