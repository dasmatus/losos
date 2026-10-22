//! The canvas edited by several people at once (src/lab/canvas/shared.ts).
//! The file's text is one shared text that every editor changes line by
//! line, and whatever anyone changes arrives here as the whole text again.
//!
//! Two kinds of change come in. Most move, write on or restyle cards, and
//! leave the network alone: then only the canvas is read again, and the
//! simulator keeps its clock, its traffic and its consoles. A change to a
//! device or a cable rebuilds the world from the file, as opening it does.

use super::file::{parse_canvas_text, read_edge, read_node, to_doc, Doc};
use super::{Canvas, MAX_EDGES, MAX_NODES, SCALE};
use crate::lab::LabCore;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

/// Puts back the commas that two people's edits, merged as text, can lose
/// or double: one too many before a `]` or `}` or after another comma, or
/// none between two values. Strings are left as they are.
pub fn repair(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    // The last character outside a string and outside white space, and
    // where it is in `out`; whether white space came after it.
    let mut last: Option<(char, usize)> = None;
    let mut gap = false;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            gap = true;
            out.push(c);
            continue;
        }
        let ends_value = |l: char| matches!(l, '}' | ']' | '"') || l.is_ascii_alphanumeric();
        let starts_value = matches!(c, '{' | '[' | '"' | '-') || c.is_ascii_alphanumeric();
        match (c, last) {
            (',', Some((',' | '[' | '{', _))) | (',', None) => {
                gap = false;
                continue;
            }
            (']' | '}', Some((',', at))) => {
                out.remove(at);
            }
            _ if starts_value && gap => {
                if let Some((_, at)) = last.filter(|(l, _)| ends_value(*l)) {
                    out.insert(at + 1, ',');
                }
            }
            _ => {}
        }
        gap = false;
        out.push(c);
        last = Some((c, out.len() - 1));
        if c == '"' {
            // The whole string, escapes and all, as it is.
            let mut escaped = false;
            for s in chars.by_ref() {
                out.push(s);
                if escaped {
                    escaped = false;
                } else if s == '\\' {
                    escaped = true;
                } else if s == '"' {
                    break;
                }
            }
            last = Some(('"', out.len() - 1));
        }
    }
    out
}

/// What the simulator is built from: each device card's device, without
/// its place, and each cable, sorted so the order of the cards does not
/// count.
fn world_sig(doc: &Doc) -> Vec<(String, Value)> {
    let id = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let mut out = Vec::new();
    for v in &doc.nodes {
        if let Some(d) = v.get("losos").and_then(|l| l.get("device")) {
            let mut d = d.clone();
            if let Value::Object(m) = &mut d {
                m.remove("x");
                m.remove("y");
            }
            out.push((format!("n {}", id(v, "id")), d));
        }
    }
    for v in &doc.edges {
        if let Some(l) = v.get("losos").and_then(|l| l.get("link")) {
            let key = format!(
                "e {} {} {}",
                id(v, "id"),
                id(v, "fromNode"),
                id(v, "toNode")
            );
            out.push((key, l.clone()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

impl LabCore {
    /// `canvasSync`: the shared text as it is now. Answers whether the
    /// world was rebuilt (`full`) and how many cards or arrows were left out.
    pub fn canvas_sync(&mut self, text: &str) -> Value {
        let doc = match parse_canvas_text(text).or_else(|_| parse_canvas_text(&repair(text))) {
            Ok(d) => d,
            Err(e) => return json!({ "error": e }),
        };
        let name = self.doc_name();
        let mine = to_doc(&self.canvas_doc(&name)).map(|d| world_sig(&d));
        if mine.as_ref() != Some(&world_sig(&doc)) {
            let key = self.scenario.clone();
            let skipped = self.open_canvas(&doc, &key, &name);
            return json!({ "full": true, "skipped": skipped });
        }
        let devs: HashMap<String, String> = self
            .canvas
            .nodes
            .iter()
            .filter_map(|n| n.dev.clone().map(|d| (n.id.clone(), d)))
            .collect();
        let links: HashMap<String, String> = self
            .canvas
            .edges
            .iter()
            .filter_map(|e| e.link.clone().map(|l| (e.id.clone(), l)))
            .collect();
        let mut canvas = Canvas {
            extra: doc.extra.clone(),
            ..Canvas::default()
        };
        if let Some(Value::Object(l)) = canvas.extra.get_mut("losos") {
            if let Some(n) = l.remove("name").as_ref().and_then(Value::as_str) {
                if !n.is_empty() {
                    self.file_name = crate::js::slice16(n, 60);
                }
            }
            if l.is_empty() {
                canvas.extra.remove("losos");
            }
        }
        let mut skipped = 0;
        let mut seen = HashSet::new();
        for v in &doc.nodes {
            let Some(mut n) = read_node(v) else {
                skipped += 1;
                continue;
            };
            if canvas.nodes.len() >= MAX_NODES || !seen.insert(n.id.clone()) {
                skipped += 1;
                continue;
            }
            if let Some(dev) = devs.get(&n.id) {
                if let Some(spec) = n.losos.remove("device") {
                    // As opening the file places it: where the file says,
                    // unless the card has gone somewhere else since.
                    let (cx, cy) = n.center();
                    let at = |k: &str, c: f64| {
                        spec.get(k)
                            .and_then(Value::as_f64)
                            .filter(|f| (f - c / SCALE).abs() <= 1.0)
                            .unwrap_or(c / SCALE)
                    };
                    let (x, y) = (at("x", cx), at("y", cy));
                    if let Some(d) = self.world.dev_mut(dev) {
                        d.x = x;
                        d.y = y;
                    }
                    n.dev = Some(dev.clone());
                }
            }
            canvas.nodes.push(n);
        }
        for v in &doc.edges {
            let Some(mut e) = read_edge(v) else {
                skipped += 1;
                continue;
            };
            if canvas.edges.len() >= MAX_EDGES
                || !seen.contains(&e.from_node)
                || !seen.contains(&e.to_node)
                || !seen.insert(e.id.clone())
            {
                skipped += 1;
                continue;
            }
            if let Some(link) = links.get(&e.id) {
                if e.losos.remove("link").is_some() {
                    e.link = Some(link.clone());
                }
            }
            canvas.edges.push(e);
        }
        self.canvas = canvas;
        self.history.clear();
        self.sync_canvas();
        self.dirty = true;
        json!({ "full": false, "skipped": skipped })
    }
}

#[cfg(test)]
mod tests {
    use super::repair;
    use serde_json::Value;

    fn parses(s: &str) -> Value {
        serde_json::from_str(&repair(s)).unwrap_or_else(|e| panic!("{e}: {}", repair(s)))
    }

    #[test]
    fn repair_puts_commas_right_and_leaves_strings_alone() {
        assert_eq!(parses("[1, 2,\n]"), serde_json::json!([1, 2]));
        assert_eq!(parses("[1,,\n 2]"), serde_json::json!([1, 2]));
        assert_eq!(parses("[,1]"), serde_json::json!([1]));
        assert_eq!(
            parses("[{\"a\": 1}\n{\"a\": 2}]"),
            serde_json::json!([{"a": 1}, {"a": 2}])
        );
        assert_eq!(
            parses("{\"a\": \"x, ]\"\n \"b\": true\n \"c\": [1] }"),
            serde_json::json!({"a": "x, ]", "b": true, "c": [1]})
        );
        assert_eq!(
            parses("{\"q\": \"say \\\"hi\\\"\" }"),
            serde_json::json!({"q": "say \"hi\""})
        );
        // Valid text comes out as it went in.
        let ok = "{\n  \"nodes\": [\n    { \"id\": \"a\", \"x\": -1.5e3 }\n  ]\n}";
        assert_eq!(repair(ok), ok);
    }
}
