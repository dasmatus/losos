//! Editing an AI tool's JSON settings without rewriting them.
//!
//! Most of the tools keep their MCP servers in a JSON file that is also the
//! owner's own: Zed's `settings.json`, VS Code's `mcp.json`, opencode's
//! `opencode.json` take comments and trailing commas, and people keep notes
//! in them. So the companion's entry goes in and comes out as a change to
//! those bytes alone: the file is parsed for where each member sits, one
//! member is replaced, added or cut, and every other byte, comment and
//! blank line stays as it was.

use serde_json::{Map, Value};

/// Why a file could not be edited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unreadable(pub String);

/// Where a parsed value sits.
#[derive(Debug, Clone)]
struct Node {
    value: Value,
    start: usize,
    end: usize,
    /// For an object: its members, in order.
    members: Vec<Member>,
}

#[derive(Debug, Clone)]
struct Member {
    key: String,
    key_start: usize,
    value: Node,
}

struct Parser<'a> {
    text: &'a str,
    bytes: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn fail<T>(&self, what: &str) -> Result<T, Unreadable> {
        let line = self.text[..self.at.min(self.text.len())]
            .matches('\n')
            .count()
            + 1;
        Err(Unreadable(format!("line {line}: {what}")))
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    /// Whitespace and both kinds of comment.
    fn skip(&mut self) -> Result<(), Unreadable> {
        loop {
            match self.peek() {
                Some(b' ' | b'\t' | b'\n' | b'\r') => self.at += 1,
                Some(b'/') if self.bytes.get(self.at + 1) == Some(&b'/') => {
                    while !matches!(self.peek(), None | Some(b'\n')) {
                        self.at += 1;
                    }
                }
                Some(b'/') if self.bytes.get(self.at + 1) == Some(&b'*') => {
                    match self.text[self.at + 2..].find("*/") {
                        Some(i) => self.at += 2 + i + 2,
                        None => return self.fail("a comment that never ends"),
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn value(&mut self) -> Result<Node, Unreadable> {
        self.skip()?;
        let start = self.at;
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => {
                let s = self.string()?;
                Ok(self.leaf(Value::String(s), start))
            }
            Some(_) => self.scalar(),
            None => self.fail("the file ends where a value should be"),
        }
    }

    fn leaf(&self, value: Value, start: usize) -> Node {
        Node {
            value,
            start,
            end: self.at,
            members: Vec::new(),
        }
    }

    fn object(&mut self) -> Result<Node, Unreadable> {
        let start = self.at;
        self.at += 1;
        let mut members = Vec::new();
        let mut map = Map::new();
        loop {
            self.skip()?;
            match self.peek() {
                Some(b'}') => {
                    self.at += 1;
                    break;
                }
                Some(b'"') => {}
                _ => return self.fail("expected a quoted key or }"),
            }
            let key_start = self.at;
            let key = self.string()?;
            self.skip()?;
            if self.peek() != Some(b':') {
                return self.fail("expected : after a key");
            }
            self.at += 1;
            let value = self.value()?;
            map.entry(key.clone())
                .or_insert_with(|| value.value.clone());
            members.push(Member {
                key,
                key_start,
                value,
            });
            self.skip()?;
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b'}') => {}
                _ => return self.fail("expected , or }"),
            }
        }
        Ok(Node {
            value: Value::Object(map),
            start,
            end: self.at,
            members,
        })
    }

    fn array(&mut self) -> Result<Node, Unreadable> {
        let start = self.at;
        self.at += 1;
        let mut items = Vec::new();
        loop {
            self.skip()?;
            if self.peek() == Some(b']') {
                self.at += 1;
                break;
            }
            items.push(self.value()?.value);
            self.skip()?;
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b']') => {}
                _ => return self.fail("expected , or ]"),
            }
        }
        Ok(self.leaf(Value::Array(items), start))
    }

    fn string(&mut self) -> Result<String, Unreadable> {
        let start = self.at;
        self.at += 1;
        loop {
            match self.peek() {
                None | Some(b'\n') => return self.fail("a string that never ends"),
                Some(b'\\') => self.at += 2,
                Some(b'"') => {
                    self.at += 1;
                    break;
                }
                Some(_) => self.at += 1,
            }
        }
        serde_json::from_str(&self.text[start..self.at]).or_else(|_| self.fail("a bad string"))
    }

    fn scalar(&mut self) -> Result<Node, Unreadable> {
        let start = self.at;
        while matches!(self.peek(), Some(b) if b.is_ascii_alphanumeric() || b"+-.".contains(&b)) {
            self.at += 1;
        }
        match serde_json::from_str::<Value>(&self.text[start..self.at]) {
            Ok(v) if start < self.at => Ok(self.leaf(v, start)),
            _ => self.fail("expected a value"),
        }
    }
}

/// The whole file, which must be one object. An empty file is `{}`.
fn parse(text: &str) -> Result<Node, Unreadable> {
    let mut p = Parser {
        text,
        bytes: text.as_bytes(),
        at: 0,
    };
    p.skip()?;
    if p.peek().is_none() {
        return Err(Unreadable("empty".into()));
    }
    let root = p.value()?;
    p.skip()?;
    if p.peek().is_some() {
        return p.fail("more after the end of the settings");
    }
    if !root.value.is_object() {
        return Err(Unreadable("the settings are not a JSON object".into()));
    }
    Ok(root)
}

/// The file as a value, comments and trailing commas forgiven.
pub fn read(text: &str) -> Result<Value, Unreadable> {
    if text.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    Ok(parse(text)?.value)
}

/// The objects along `path` from the root, as far as they go, and how many
/// keys of it were found.
fn find<'n>(root: &'n Node, path: &[&str]) -> Result<(Vec<&'n Node>, usize), Unreadable> {
    let mut node = root;
    let mut chain = vec![node];
    for (depth, key) in path.iter().enumerate() {
        match node.members.iter().find(|m| m.key == *key) {
            Some(m) if m.value.value.is_object() => {
                node = &m.value;
                chain.push(node);
            }
            Some(_) => {
                return Err(Unreadable(format!(
                    "\"{key}\" is there but is not an object"
                )))
            }
            None => return Ok((chain, depth)),
        }
    }
    Ok((chain, path.len()))
}

/// The indentation one level takes in this file: what its first indented
/// line uses, else two spaces.
fn unit(text: &str) -> String {
    text.lines()
        .skip(1)
        .find_map(|line| {
            let lead: String = line
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .collect();
            (!lead.is_empty() && line.trim_start().starts_with('"')).then_some(lead)
        })
        .unwrap_or_else(|| "  ".into())
}

/// `value` written out at `depth`, its first line unindented.
fn render(value: &Value, unit: &str, depth: usize) -> String {
    use serde::Serialize;
    let mut out = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(unit.as_bytes());
    let mut ser = serde_json::Serializer::with_formatter(&mut out, fmt);
    value
        .serialize(&mut ser)
        .expect("a Value always serialises");
    let text = String::from_utf8(out).expect("serde_json writes UTF-8");
    text.replace('\n', &format!("\n{}", unit.repeat(depth)))
}

fn quoted(key: &str) -> String {
    Value::String(key.to_string()).to_string()
}

/// Set `name` in the object at `path` to `value`, making the objects on the
/// way when they are missing. The rest of `text` is kept byte for byte.
pub fn upsert(text: &str, path: &[&str], name: &str, value: &Value) -> Result<String, Unreadable> {
    if text.trim().is_empty() {
        let mut v = value.clone();
        for key in std::iter::once(&name).chain(path.iter().rev()) {
            let mut m = Map::new();
            m.insert((*key).to_string(), v);
            v = Value::Object(m);
        }
        return Ok(format!("{}\n", render(&v, "  ", 0)));
    }
    let root = parse(text)?;
    let unit = unit(text);
    let (chain, reached) = find(&root, path)?;
    let container = *chain.last().expect("the root is always there");
    let depth = chain.len() - 1;
    // What goes in: the member itself, or the missing objects around it.
    let mut inner = value.clone();
    let mut key = name;
    for k in path[reached..].iter().rev() {
        let mut m = Map::new();
        m.insert(key.to_string(), inner);
        inner = Value::Object(m);
        key = k;
    }
    if reached == path.len() {
        if let Some(m) = container.members.iter().find(|m| m.key == name) {
            let mut out = text.to_string();
            out.replace_range(m.value.start..m.value.end, &render(value, &unit, depth + 1));
            return Ok(out);
        }
    }
    Ok(insert(text, container, depth, &unit, key, &inner))
}

/// Add `key: value` as the last member of `object`, which sits at `depth`.
fn insert(text: &str, object: &Node, depth: usize, unit: &str, key: &str, value: &Value) -> String {
    let member = format!("{}: {}", quoted(key), render(value, unit, depth + 1));
    let indent = unit.repeat(depth + 1);
    let mut out = text.to_string();
    match object.members.last() {
        Some(last) => {
            out.insert_str(last.value.end, &format!(",\n{indent}{member}"));
        }
        None => {
            let inside = &text[object.start + 1..object.end - 1];
            if inside.trim().is_empty() {
                out.replace_range(
                    object.start + 1..object.end - 1,
                    &format!("\n{indent}{member}\n{}", unit.repeat(depth)),
                );
            } else {
                // Only comments inside: the member goes first, they stay.
                out.insert_str(object.start + 1, &format!("\n{indent}{member}"));
            }
        }
    }
    out
}

/// The value of `name` in the object at `path`, if there is one.
pub fn get(text: &str, path: &[&str], name: &str) -> Result<Option<Value>, Unreadable> {
    let mut v = &read(text)?;
    for key in path {
        match v.get(key) {
            Some(next) => v = next,
            None => return Ok(None),
        }
    }
    Ok(v.get(name).cloned())
}

/// Cut `name` out of the object at `path`. `None` when it is not there.
pub fn remove(text: &str, path: &[&str], name: &str) -> Result<Option<String>, Unreadable> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let root = parse(text)?;
    let Ok((chain, reached)) = find(&root, path) else {
        return Ok(None);
    };
    if reached != path.len() {
        return Ok(None);
    }
    let object = *chain.last().expect("the root is always there");
    let Some(i) = object.members.iter().position(|m| m.key == name) else {
        return Ok(None);
    };
    let members = &object.members;
    let mut out = text.to_string();
    if members.len() == 1 {
        out.replace_range(object.start + 1..object.end - 1, "");
    } else if i + 1 < members.len() {
        // Up to the next member's key: this member and the comma after it.
        let from = line_start(text, members[i].key_start);
        let to = line_start(text, members[i + 1].key_start);
        out.replace_range(from..to, "");
    } else {
        // The last one: from the end of the one before, taking its comma.
        out.replace_range(members[i - 1].value.end..members[i].value.end, "");
    }
    Ok(Some(out))
}

/// Where the line that holds `at` starts, when only blanks come before
/// `at` on it; else `at` itself.
fn line_start(text: &str, at: usize) -> usize {
    let before = &text[..at];
    let start = before.rfind('\n').map_or(0, |i| i + 1);
    if before[start..].chars().all(|c| c == ' ' || c == '\t') {
        start
    } else {
        at
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry() -> Value {
        json!({ "command": "/usr/bin/losos", "args": ["mcp"] })
    }

    #[test]
    fn a_new_file_gets_the_objects_around_the_entry() {
        let out = upsert("", &["mcpServers"], "losos", &entry()).unwrap();
        assert_eq!(
            read(&out).unwrap(),
            json!({ "mcpServers": { "losos": entry() } })
        );
        assert!(out.ends_with("}\n"));
    }

    #[test]
    fn comments_and_the_owners_own_entries_stay_byte_for_byte() {
        let text = r#"// Zed settings
{
    "theme": "One Dark", // the theme
    /* servers */
    "context_servers": {
        "mine": { "command": "mine" },
    },
}
"#;
        let out = upsert(text, &["context_servers"], "losos", &entry()).unwrap();
        assert!(out.starts_with(
            "// Zed settings\n{\n    \"theme\": \"One Dark\", // the theme\n    /* servers */"
        ));
        assert!(out.contains(
            "\"mine\": { \"command\": \"mine\" },\n        \"losos\": {\n            \"command\""
        ));
        let v = read(&out).unwrap();
        assert_eq!(v["context_servers"]["losos"], entry());
        assert_eq!(v["context_servers"]["mine"], json!({"command": "mine"}));

        let back = remove(&out, &["context_servers"], "losos")
            .unwrap()
            .unwrap();
        assert_eq!(back, text);
    }

    #[test]
    fn a_missing_container_is_made_in_the_file_style() {
        let text = "{\n\t\"editor.fontSize\": 14\n}\n";
        let out = upsert(text, &["mcp", "servers"], "losos", &entry()).unwrap();
        assert_eq!(
            out,
            "{\n\t\"editor.fontSize\": 14,\n\t\"mcp\": {\n\t\t\"servers\": {\n\t\t\t\"losos\": {\n\t\t\t\t\"command\": \"/usr/bin/losos\",\n\t\t\t\t\"args\": [\n\t\t\t\t\t\"mcp\"\n\t\t\t\t]\n\t\t\t}\n\t\t}\n\t}\n}\n"
        );
        assert_eq!(read(&out).unwrap()["mcp"]["servers"]["losos"], entry());
    }

    #[test]
    fn an_entry_already_there_is_replaced_in_place() {
        let text = "{\"mcpServers\": {\"losos\": {\"command\": \"old\"}, \"b\": 1}}";
        let out = upsert(text, &["mcpServers"], "losos", &entry()).unwrap();
        let v = read(&out).unwrap();
        assert_eq!(v["mcpServers"]["losos"], entry());
        assert_eq!(v["mcpServers"]["b"], json!(1));
        assert!(out.ends_with(", \"b\": 1}}"));
    }

    #[test]
    fn an_empty_container_and_one_holding_only_comments() {
        let out = upsert("{ \"servers\": {} }", &["servers"], "losos", &json!(1)).unwrap();
        assert_eq!(out, "{ \"servers\": {\n    \"losos\": 1\n  } }");
        let out = upsert(
            "{\"servers\": { // none yet\n}}",
            &["servers"],
            "losos",
            &json!(1),
        )
        .unwrap();
        assert_eq!(read(&out).unwrap(), json!({"servers": {"losos": 1}}));
        assert!(out.contains("// none yet"));
    }

    #[test]
    fn removing_the_first_middle_last_and_only_member() {
        let text = "{\n  \"s\": {\n    \"a\": 1,\n    \"losos\": 2,\n    \"b\": 3\n  }\n}\n";
        assert_eq!(
            remove(text, &["s"], "losos").unwrap().unwrap(),
            "{\n  \"s\": {\n    \"a\": 1,\n    \"b\": 3\n  }\n}\n"
        );
        let text = "{\n  \"s\": {\n    \"a\": 1,\n    \"losos\": 2\n  }\n}\n";
        assert_eq!(
            remove(text, &["s"], "losos").unwrap().unwrap(),
            "{\n  \"s\": {\n    \"a\": 1\n  }\n}\n"
        );
        let text = "{\n  \"s\": {\n    \"losos\": 2,\n    \"a\": 1\n  }\n}\n";
        assert_eq!(
            remove(text, &["s"], "losos").unwrap().unwrap(),
            "{\n  \"s\": {\n    \"a\": 1\n  }\n}\n"
        );
        let text = "{\"s\": {\"losos\": 2}}";
        assert_eq!(
            remove(text, &["s"], "losos").unwrap().unwrap(),
            "{\"s\": {}}"
        );
        assert_eq!(remove(text, &["s"], "other").unwrap(), None);
        assert_eq!(remove(text, &["t"], "losos").unwrap(), None);
    }

    #[test]
    fn what_is_not_a_settings_object_is_refused() {
        assert!(upsert("[1, 2]", &["s"], "losos", &json!(1)).is_err());
        assert!(upsert("{\"s\": 1}", &["s"], "losos", &json!(1)).is_err());
        assert!(upsert("{\"a\": }", &["s"], "losos", &json!(1)).is_err());
        assert!(upsert("{} {}", &["s"], "losos", &json!(1)).is_err());
        assert!(read("{\"a\": /* never closed").is_err());
        assert_eq!(
            get("{\"s\": {\"losos\": [1]}}", &["s"], "losos").unwrap(),
            Some(json!([1]))
        );
        assert_eq!(get("{}", &["s"], "losos").unwrap(), None);
    }

    #[test]
    fn strings_with_escapes_and_slashes_are_not_comments() {
        let text = r#"{"url": "https://x//y", "q": "a\"b /* c */", "s": {}}"#;
        let out = upsert(text, &["s"], "losos", &json!(1)).unwrap();
        let v = read(&out).unwrap();
        assert_eq!(v["url"], json!("https://x//y"));
        assert_eq!(v["q"], json!("a\"b /* c */"));
        assert_eq!(v["s"]["losos"], json!(1));
    }
}
