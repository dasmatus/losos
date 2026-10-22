//! The canvas as a file: JSON Canvas in and out, and the older setup files
//! (`losos-lab-setup`, version 1) read into a canvas.
//!
//! Reading is lenient where Obsidian is (a node with no colour, an edge
//! with no sides) and strict where a value would be meaningless (a node
//! with no place or size is left out and counted). Whatever a node or an
//! edge carries that this editor does not know stays on it.

use super::{valid_color, Edge, Node, MAX_EDGES, MAX_NODES, NODE_KINDS, SCALE, SIDES};
use crate::catalog::{is_link_kind, port_kind, type_info};
use crate::files::{MAX_BYTES, NOT_A_FILE};
use crate::lab::LabCore;
use crate::world::Connected;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

/// A parsed canvas document: its nodes and edges as JSON, before any of
/// them is checked.
pub struct Doc {
    pub nodes: Vec<Value>,
    pub edges: Vec<Value>,
    pub extra: Map<String, Value>,
}

const NODE_KEYS: [&str; 15] = [
    "id",
    "type",
    "x",
    "y",
    "width",
    "height",
    "color",
    "text",
    "file",
    "subpath",
    "url",
    "label",
    "background",
    "backgroundStyle",
    "styleAttributes",
];
const EDGE_KEYS: [&str; 10] = [
    "id",
    "fromNode",
    "fromSide",
    "fromEnd",
    "toNode",
    "toSide",
    "toEnd",
    "color",
    "label",
    "styleAttributes",
];

/// Whether `doc` is a JSON Canvas document: an object whose `nodes` and
/// `edges`, where present, are arrays (an empty `{}` is a valid canvas).
pub fn is_canvas(doc: &Value) -> bool {
    let Value::Object(m) = doc else {
        return false;
    };
    if m.contains_key("format") {
        return false;
    }
    let arr_or_missing = |k: &str| matches!(m.get(k), None | Some(Value::Array(_)));
    arr_or_missing("nodes") && arr_or_missing("edges")
}

pub fn to_doc(doc: &Value) -> Option<Doc> {
    if !is_canvas(doc) {
        return None;
    }
    let m = doc.as_object()?;
    let arr = |k: &str| {
        m.get(k)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let mut extra = m.clone();
    extra.remove("nodes");
    extra.remove("edges");
    Some(Doc {
        nodes: arr("nodes"),
        edges: arr("edges"),
        extra,
    })
}

pub fn parse_canvas_text(text: &str) -> Result<Doc, String> {
    if text.len() > MAX_BYTES {
        return Err("That canvas is too big to open.".into());
    }
    let v: Value = serde_json::from_str(text).map_err(|_| NOT_A_FILE.to_string())?;
    to_doc(&v).ok_or_else(|| NOT_A_FILE.to_string())
}

fn opt_str(m: &Map<String, Value>, k: &str) -> Option<String> {
    m.get(k).and_then(Value::as_str).map(str::to_string)
}

fn finite(v: Option<&Value>) -> Option<f64> {
    v?.as_f64().filter(|f| f.is_finite())
}

/// One node from a file; `None` (left out) when it has no id, kind, place
/// or size.
pub fn read_node(v: &Value) -> Option<Node> {
    let m = v.as_object()?;
    let id = opt_str(m, "id").filter(|s| !s.is_empty() && s.len() <= 200)?;
    let kind = opt_str(m, "type")?;
    if !NODE_KINDS.contains(&kind.as_str()) {
        return None;
    }
    let (x, y) = (finite(m.get("x"))?, finite(m.get("y"))?);
    let (w, h) = (finite(m.get("width"))?, finite(m.get("height"))?);
    let mut n = Node::blank(
        id,
        &kind,
        x.clamp(-1e7, 1e7),
        y.clamp(-1e7, 1e7),
        w.clamp(1.0, 1e6),
        h.clamp(1.0, 1e6),
    );
    n.color = opt_str(m, "color").filter(|c| valid_color(c));
    n.text = opt_str(m, "text");
    n.file = opt_str(m, "file");
    n.subpath = opt_str(m, "subpath");
    n.url = opt_str(m, "url");
    n.label = opt_str(m, "label");
    n.background = opt_str(m, "background");
    n.background_style = opt_str(m, "backgroundStyle");
    if let Some(Value::Object(st)) = m.get("styleAttributes") {
        n.style = st.clone();
    }
    if let Some(Value::Object(l)) = m.get("losos") {
        n.losos = l.clone();
    }
    for (k, v) in m {
        if !NODE_KEYS.contains(&k.as_str()) && k != "losos" {
            n.extra.insert(k.clone(), v.clone());
        }
    }
    if kind == "text" && n.text.is_none() {
        n.text = Some(String::new());
    }
    Some(n)
}

pub fn read_edge(v: &Value) -> Option<Edge> {
    let m = v.as_object()?;
    let id = opt_str(m, "id").filter(|s| !s.is_empty() && s.len() <= 200)?;
    let from = opt_str(m, "fromNode")?;
    let to = opt_str(m, "toNode")?;
    let mut e = Edge::between(id, &from, &to);
    let side = |k: &str| opt_str(m, k).filter(|s| SIDES.contains(&s.as_str()));
    let end = |k: &str| opt_str(m, k).filter(|s| matches!(s.as_str(), "none" | "arrow"));
    e.from_side = side("fromSide");
    e.to_side = side("toSide");
    e.from_end = end("fromEnd");
    e.to_end = end("toEnd");
    e.color = opt_str(m, "color").filter(|c| valid_color(c));
    e.label = opt_str(m, "label");
    if let Some(Value::Object(st)) = m.get("styleAttributes") {
        e.style = st.clone();
    }
    if let Some(Value::Object(l)) = m.get("losos") {
        e.losos = l.clone();
    }
    for (k, v) in m {
        if !EDGE_KEYS.contains(&k.as_str()) && k != "losos" {
            e.extra.insert(k.clone(), v.clone());
        }
    }
    Some(e)
}

/// The width and height a fragment's nodes span.
pub fn fragment_size(doc: &Doc) -> (f64, f64) {
    let nodes: Vec<Node> = doc.nodes.iter().filter_map(read_node).collect();
    if nodes.is_empty() {
        return (0.0, 0.0);
    }
    let l = nodes.iter().map(|n| n.x).fold(f64::INFINITY, f64::min);
    let t = nodes.iter().map(|n| n.y).fold(f64::INFINITY, f64::min);
    let r = nodes
        .iter()
        .map(Node::right)
        .fold(f64::NEG_INFINITY, f64::max);
    let b = nodes
        .iter()
        .map(Node::bottom)
        .fold(f64::NEG_INFINITY, f64::max);
    (r - l, b - t)
}

fn num_out(x: f64) -> Value {
    // JSON Canvas positions and sizes are integers.
    Value::from(x.round() as i64)
}

impl LabCore {
    /// A node as JSON. `for_file`: a device card's `text` and `losos.device`
    /// written from its device, so the file can rebuild it.
    pub(crate) fn node_json(&self, n: &Node, for_file: bool) -> Value {
        let mut m = Map::new();
        m.insert("id".into(), n.id.clone().into());
        m.insert("type".into(), n.kind.clone().into());
        m.insert("x".into(), num_out(n.x));
        m.insert("y".into(), num_out(n.y));
        m.insert("width".into(), num_out(n.width));
        m.insert("height".into(), num_out(n.height));
        let mut put = |k: &str, v: &Option<String>| {
            if let Some(v) = v {
                m.insert(k.into(), v.clone().into());
            }
        };
        put("color", &n.color);
        let dev = n.dev.as_ref().and_then(|d| self.world.dev(d));
        let text = match dev {
            Some(d) => Some(format!(
                "**{}**\n{}",
                d.name,
                type_info(&d.kind)
                    .map(|t| t.label)
                    .unwrap_or(d.kind.as_str())
            )),
            None => n.text.clone(),
        };
        put("text", &text);
        put("file", &n.file);
        put("subpath", &n.subpath);
        put("url", &n.url);
        put("label", &n.label);
        put("background", &n.background);
        put("backgroundStyle", &n.background_style);
        if !n.style.is_empty() {
            m.insert("styleAttributes".into(), Value::Object(n.style.clone()));
        }
        let mut losos = n.losos.clone();
        if let (Some(d), true) = (dev, for_file) {
            losos.insert(
                "device".into(),
                json!({
                    "type": d.kind, "name": d.name, "site": d.site,
                    "x": crate::js::num_value(d.x), "y": crate::js::num_value(d.y),
                    "px": d.px.map(crate::js::num_value), "py": d.py.map(crate::js::num_value),
                    "power": d.power, "cfg": d.cfg,
                }),
            );
        }
        if !losos.is_empty() {
            m.insert("losos".into(), Value::Object(losos));
        }
        for (k, v) in &n.extra {
            m.entry(k.clone()).or_insert_with(|| v.clone());
        }
        Value::Object(m)
    }

    pub(crate) fn edge_json(&self, e: &Edge, for_file: bool) -> Value {
        let mut m = Map::new();
        m.insert("id".into(), e.id.clone().into());
        m.insert("fromNode".into(), e.from_node.clone().into());
        let put = |m: &mut Map<String, Value>, k: &str, v: &Option<String>| {
            if let Some(v) = v {
                m.insert(k.into(), v.clone().into());
            }
        };
        put(&mut m, "fromSide", &e.from_side);
        put(&mut m, "fromEnd", &e.from_end);
        m.insert("toNode".into(), e.to_node.clone().into());
        put(&mut m, "toSide", &e.to_side);
        put(&mut m, "toEnd", &e.to_end);
        put(&mut m, "color", &e.color);
        put(&mut m, "label", &e.label);
        if !e.style.is_empty() {
            m.insert("styleAttributes".into(), Value::Object(e.style.clone()));
        }
        let mut losos = e.losos.clone();
        let link = e
            .link
            .as_ref()
            .and_then(|l| self.world.links.iter().find(|x| &x.id == l));
        if let (Some(l), true) = (link, for_file) {
            // The ends as the file has them: fromNode is one of the two.
            let from_dev = self.canvas.node(&e.from_node).and_then(|n| n.dev.clone());
            let (fp, tp) = if from_dev.as_deref() == Some(l.a.dev.as_str()) {
                (&l.a.port, &l.b.port)
            } else {
                (&l.b.port, &l.a.port)
            };
            losos.insert(
                "link".into(),
                json!({ "kind": l.kind, "fromPort": fp, "toPort": tp }),
            );
        }
        if !losos.is_empty() {
            m.insert("losos".into(), Value::Object(losos));
        }
        for (k, v) in &e.extra {
            m.entry(k.clone()).or_insert_with(|| v.clone());
        }
        Value::Object(m)
    }

    /// The whole canvas as a JSON Canvas document, devices and cables in it.
    pub(crate) fn canvas_doc(&self, name: &str) -> Value {
        let mut m = self.canvas.extra.clone();
        let nodes: Vec<Value> = self
            .canvas
            .nodes
            .iter()
            .map(|n| self.node_json(n, true))
            .collect();
        let edges: Vec<Value> = self
            .canvas
            .edges
            .iter()
            .map(|e| self.edge_json(e, true))
            .collect();
        m.insert("nodes".into(), Value::Array(nodes));
        m.insert("edges".into(), Value::Array(edges));
        // The name the picker shows for "Last diagram"; Obsidian ignores it.
        let mut losos = m
            .get("losos")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        losos.insert("name".into(), name.into());
        m.insert("losos".into(), Value::Object(losos));
        Value::Object(m)
    }

    /// A device from a card's `losos.device`, its centre at the card's.
    /// The device keeps the exact logical place the file wrote while the
    /// card is still there (a card's corner is rounded to whole units); a
    /// card moved elsewhere, in Obsidian say, takes its device along.
    fn device_from_node(&mut self, n: &Node, spec: &Value) -> Option<String> {
        let mut s = spec.clone();
        let (cx, cy) = n.center();
        let (x, y) = (cx / SCALE, cy / SCALE);
        let near = |k: &str, v: f64| {
            spec.get(k)
                .and_then(Value::as_f64)
                .is_some_and(|f| (f - v).abs() <= 1.0)
        };
        if !(near("x", x) && near("y", y)) {
            s["x"] = json!(x);
            s["y"] = json!(y);
        }
        self.device_from_doc(&s)
    }

    /// Reads `doc` into the canvas and the world, every id kept as written
    /// (`fresh: false`, opening a file) or made new and the fragment moved
    /// so its top-left corner is at `at` (`fresh: true`, a paste or a
    /// template). Returns the ids it placed and how many it left out.
    fn read_doc(
        &mut self,
        doc: &Doc,
        fresh: bool,
        at: (f64, f64),
    ) -> (Vec<String>, Vec<String>, usize) {
        let mut skipped = 0;
        let mut ids: HashMap<String, String> = HashMap::new();
        let mut nodes: Vec<Node> = Vec::new();
        for v in &doc.nodes {
            match read_node(v) {
                Some(n) if !ids.contains_key(&n.id) => {
                    ids.insert(n.id.clone(), String::new());
                    nodes.push(n);
                }
                _ => skipped += 1,
            }
        }
        let (ox, oy) = if fresh && !nodes.is_empty() {
            let l = nodes.iter().map(|n| n.x).fold(f64::INFINITY, f64::min);
            let t = nodes.iter().map(|n| n.y).fold(f64::INFINITY, f64::min);
            (at.0 - l, at.1 - t)
        } else {
            (0.0, 0.0)
        };
        let room = MAX_NODES.saturating_sub(self.canvas.nodes.len());
        if nodes.len() > room {
            skipped += nodes.len() - room;
            nodes.truncate(room);
        }
        let mut placed = Vec::new();
        for mut n in nodes {
            let old = n.id.clone();
            if fresh || self.canvas.has_id(&n.id) {
                n.id = self.ids.next(&self.canvas);
            }
            n.x = (n.x + ox).round();
            n.y = (n.y + oy).round();
            if let Some(spec) = n.losos.remove("device") {
                if n.kind == "text" {
                    match self.device_from_node(&n, &spec) {
                        Some(dev) => n.dev = Some(dev),
                        None => {
                            // Not a device this editor knows: a plain card,
                            // and the device data stays on it.
                            n.losos.insert("device".into(), spec);
                        }
                    }
                } else {
                    n.losos.insert("device".into(), spec);
                }
            }
            ids.insert(old, n.id.clone());
            placed.push(n.id.clone());
            self.canvas.nodes.push(n);
        }
        let mut edges_placed = Vec::new();
        for v in &doc.edges {
            if self.canvas.edges.len() >= MAX_EDGES {
                skipped += 1;
                continue;
            }
            let Some(mut e) = read_edge(v) else {
                skipped += 1;
                continue;
            };
            let (Some(from), Some(to)) = (
                ids.get(&e.from_node).filter(|s| !s.is_empty()).cloned(),
                ids.get(&e.to_node).filter(|s| !s.is_empty()).cloned(),
            ) else {
                skipped += 1;
                continue;
            };
            e.from_node = from;
            e.to_node = to;
            if fresh || self.canvas.has_id(&e.id) {
                e.id = self.ids.next(&self.canvas);
            }
            if let Some(spec) = e.losos.remove("link") {
                match self.cable_from_edge(&e, &spec) {
                    Some(l) => e.link = Some(l),
                    None => {
                        e.losos.insert("link".into(), spec);
                    }
                }
            }
            edges_placed.push(e.id.clone());
            self.canvas.edges.push(e);
        }
        (placed, edges_placed, skipped)
    }

    /// A cable from an edge's `losos.link` between two device cards.
    fn cable_from_edge(&mut self, e: &Edge, spec: &Value) -> Option<String> {
        let a = self.canvas.node(&e.from_node)?.dev.clone()?;
        let b = self.canvas.node(&e.to_node)?.dev.clone()?;
        let kind = spec.get("kind").and_then(Value::as_str).unwrap_or("auto");
        let port = |k: &str| spec.get(k).and_then(Value::as_str).map(str::to_string);
        let (ap, bp) = (port("fromPort"), port("toPort"));
        let r = if kind == "wifi" {
            self.world.connect(&a, &b, "wifi", None, None)
        } else {
            let ok = |lab: &LabCore, d: &str, p: &Option<String>| match p {
                None => true,
                Some(p) => {
                    let kind = lab.world.dev(d).map(|x| x.kind.clone()).unwrap_or_default();
                    port_kind(&kind, p) == Some("eth") && !lab.world.port_used(d, p)
                }
            };
            if !ok(self, &a, &ap) || !ok(self, &b, &bp) {
                return None;
            }
            let k = if is_link_kind(kind) { kind } else { "auto" };
            self.world.connect(&a, &b, k, ap.as_deref(), bp.as_deref())
        };
        match r {
            Connected::Link(l) => Some(l.id),
            Connected::Error(_) => None,
        }
    }

    /// `openDoc` for a canvas: checked, then read into a fresh world.
    pub(crate) fn open_canvas(&mut self, doc: &Doc, key: &str, label: &str) -> usize {
        let name = doc
            .extra
            .get("losos")
            .and_then(|l| l.get("name"))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| label.to_string());
        self.file_name = crate::js::slice16(&name, 60);
        let mut skipped = 0;
        let extra = doc.extra.clone();
        self.start_world(key, |lab| {
            let (_, _, s) = lab.read_doc(doc, false, (0.0, 0.0));
            skipped = s;
            None
        });
        let mut extra = extra;
        if let Some(Value::Object(l)) = extra.get_mut("losos") {
            l.remove("name");
            if l.is_empty() {
                extra.remove("losos");
            }
        }
        self.canvas.extra = extra;
        self.dirty = false;
        skipped
    }

    /// A paste or a template: fresh ids, its corner at (`x`, `y`).
    pub(crate) fn insert_fragment(&mut self, doc: &Doc, x: f64, y: f64) -> Result<Value, String> {
        if doc.nodes.is_empty() {
            return Err("There is nothing to paste.".into());
        }
        let (nodes, edges, skipped) = self.read_doc(doc, true, (x, y));
        if nodes.is_empty() {
            return Err("There is nothing to paste.".into());
        }
        self.changed();
        Ok(json!({ "nodes": nodes, "edges": edges, "skipped": skipped }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_obsidian_nodes_and_keeps_what_it_does_not_know() {
        let v = json!({
            "id": "a1", "type": "text", "x": 10, "y": -20.5, "width": 250, "height": 60,
            "text": "# Hello", "color": "4", "styleAttributes": {"shape": "diamond"},
            "plugin": {"kept": true}
        });
        let n = read_node(&v).expect("a node");
        assert_eq!(n.color.as_deref(), Some("4"));
        assert_eq!(n.style.get("shape"), Some(&json!("diamond")));
        assert_eq!(n.extra.get("plugin"), Some(&json!({"kept": true})));
        assert!(read_node(&json!({"id": "x", "type": "text", "x": 1})).is_none());
        assert!(read_node(
            &json!({"id": "x", "type": "frame", "x": 0, "y": 0, "width": 1, "height": 1})
        )
        .is_none());
        assert!(is_canvas(&json!({})));
        assert!(!is_canvas(&json!({"format": "losos-lab-setup"})));
    }
}
