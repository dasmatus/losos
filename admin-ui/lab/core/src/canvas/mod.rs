//! The canvas: the diagram as a JSON Canvas document (jsoncanvas.org, 1.0),
//! the format Obsidian's Canvas reads and writes. Four kinds of node (text,
//! file, link, group) and edges between them, each kept whole: keys this
//! crate does not know, on a node, an edge or the document, go back out on
//! save, so a file Obsidian or one of its plugins wrote survives a round
//! trip.
//!
//! The network is part of the same document. Every device of the world is
//! a text node that carries `losos.device`, and every cable an edge that
//! carries `losos.link`; in Obsidian they read as cards and arrows, here
//! they are the simulator's devices and links. The world stays the truth
//! for both: a device node's place is its device's logical position times
//! `SCALE`, and `sync` brings the nodes and cable edges in line after any
//! change, so the logical view and the canvas never disagree.
//!
//! Shapes and edge styles use the Advanced Canvas plugin's
//! `styleAttributes` names (`shape`, `border`, `textAlign`, `path`,
//! `arrow`, `pathfindingMethod`), so the same file looks the same in
//! Obsidian with that plugin.

pub mod file;
pub mod geom;
pub mod templates;

use crate::lab::LabCore;
use crate::world::World;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

/// Canvas units per unit of the logical view.
pub const SCALE: f64 = 1.5;
/// A device card's size when it first lands on the canvas.
pub const DEVICE_W: f64 = 128.0;
pub const DEVICE_H: f64 = 96.0;
/// Undo steps kept.
const HISTORY: usize = 100;
/// Nodes and edges a document may hold.
pub const MAX_NODES: usize = 5000;
pub const MAX_EDGES: usize = 10000;

pub const NODE_KINDS: [&str; 4] = ["text", "file", "link", "group"];
pub const SIDES: [&str; 4] = ["top", "right", "bottom", "left"];

/// Advanced Canvas's node shapes, and the two this editor adds (an
/// Obsidian without the plugin draws every one as a card).
pub const SHAPES: [&str; 10] = [
    "pill",
    "diamond",
    "parallelogram",
    "circle",
    "predefined-process",
    "document",
    "database",
    "hexagon",
    "cloud",
    "note",
];
pub const BORDERS: [&str; 3] = ["dashed", "dotted", "invisible"];
pub const ALIGNS: [&str; 2] = ["center", "right"];
pub const PATHS: [&str; 3] = ["dotted", "short-dashed", "long-dashed"];
pub const ROUTES: [&str; 3] = ["direct", "square", "a-star"];
pub const ARROWS: [&str; 9] = [
    "triangle",
    "triangle-outline",
    "thin-triangle",
    "halved-triangle",
    "diamond",
    "diamond-outline",
    "circle",
    "circle-outline",
    "blunt",
];

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: String,
    pub kind: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub color: Option<String>,
    pub text: Option<String>,
    pub file: Option<String>,
    pub subpath: Option<String>,
    pub url: Option<String>,
    pub label: Option<String>,
    pub background: Option<String>,
    pub background_style: Option<String>,
    pub style: Map<String, Value>,
    /// The `losos` key without `device` (the world writes that one).
    pub losos: Map<String, Value>,
    /// Keys this crate does not know, kept for the way out.
    pub extra: Map<String, Value>,
    /// The world device this node draws, if it is one.
    pub dev: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub id: String,
    pub from_node: String,
    pub from_side: Option<String>,
    pub from_end: Option<String>,
    pub to_node: String,
    pub to_side: Option<String>,
    pub to_end: Option<String>,
    pub color: Option<String>,
    pub label: Option<String>,
    pub style: Map<String, Value>,
    /// The `losos` key without `link`.
    pub losos: Map<String, Value>,
    pub extra: Map<String, Value>,
    /// The world link this edge draws, if it is a cable.
    pub link: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Canvas {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    /// Top-level keys besides `nodes` and `edges`.
    pub extra: Map<String, Value>,
}

impl Node {
    pub fn blank(id: String, kind: &str, x: f64, y: f64, width: f64, height: f64) -> Node {
        Node {
            id,
            kind: kind.into(),
            x,
            y,
            width,
            height,
            color: None,
            text: None,
            file: None,
            subpath: None,
            url: None,
            label: None,
            background: None,
            background_style: None,
            style: Map::new(),
            losos: Map::new(),
            extra: Map::new(),
            dev: None,
        }
    }
    pub fn right(&self) -> f64 {
        self.x + self.width
    }
    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
    /// Lies wholly inside `g`, the way Obsidian decides what a group carries.
    pub fn inside(&self, g: &Node) -> bool {
        self.id != g.id
            && self.x >= g.x
            && self.y >= g.y
            && self.right() <= g.right()
            && self.bottom() <= g.bottom()
    }
}

impl Edge {
    pub fn between(id: String, from: &str, to: &str) -> Edge {
        Edge {
            id,
            from_node: from.into(),
            from_side: None,
            from_end: None,
            to_node: to.into(),
            to_side: None,
            to_end: None,
            color: None,
            label: None,
            style: Map::new(),
            losos: Map::new(),
            extra: Map::new(),
            link: None,
        }
    }
}

impl Canvas {
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }
    pub fn node_mut(&mut self, id: &str) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }
    pub fn edge(&self, id: &str) -> Option<&Edge> {
        self.edges.iter().find(|e| e.id == id)
    }
    pub fn edge_mut(&mut self, id: &str) -> Option<&mut Edge> {
        self.edges.iter_mut().find(|e| e.id == id)
    }
    pub fn node_of_dev(&self, dev: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.dev.as_deref() == Some(dev))
    }
    pub fn has_id(&self, id: &str) -> bool {
        self.nodes.iter().any(|n| n.id == id) || self.edges.iter().any(|e| e.id == id)
    }
}

/// Ids: sixteen hex digits, as Obsidian writes them, from a counter of the
/// canvas's own (the simulator's PRNG is not touched, so its MACs and ping
/// times stay what they were before the canvas existed).
#[derive(Clone, Debug)]
pub struct Ids {
    salt: u64,
    n: u64,
}

impl Ids {
    pub fn new(salt: u64) -> Ids {
        Ids { salt, n: 0 }
    }
    pub fn next(&mut self, canvas: &Canvas) -> String {
        loop {
            self.n += 1;
            let mut r =
                crate::rng::Rng::new(self.salt ^ self.n.wrapping_mul(0x2545_f491_4f6c_dd1d));
            let id = format!("{:016x}", r.next_u64());
            if !canvas.has_id(&id) {
                return id;
            }
        }
    }
}

/// What undo and redo move between: the canvas and the world together,
/// because a device or a cable is in both.
#[derive(Clone, Debug, Default)]
pub struct History {
    undo: Vec<(Canvas, World)>,
    redo: Vec<(Canvas, World)>,
    /// The gesture the last step was taken for: a drag is one step.
    gesture: Option<f64>,
}

impl History {
    pub fn clear(&mut self) {
        *self = History::default();
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    /// Forgets the last step taken, for a change that was refused after it.
    pub fn drop_last(&mut self) {
        self.undo.pop();
        self.gesture = None;
    }
}

fn err(msg: impl Into<String>) -> Value {
    json!({ "error": msg.into() })
}

fn parse<T: serde::de::DeserializeOwned>(text: &str) -> Option<T> {
    serde_json::from_str(text).ok()
}

fn finite(v: Option<&Value>) -> Option<f64> {
    v?.as_f64().filter(|f| f.is_finite())
}

/// A colour as JSON Canvas has it: a preset "1" to "6", or "#rgb"/"#rrggbb".
pub fn valid_color(c: &str) -> bool {
    if matches!(c, "1" | "2" | "3" | "4" | "5" | "6") {
        return true;
    }
    let Some(hex) = c.strip_prefix('#') else {
        return false;
    };
    matches!(hex.len(), 3 | 6) && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The text fields a node may carry, with the longest each may be.
const NODE_TEXT: [(&str, usize); 6] = [
    ("text", 100_000),
    ("file", 2_000),
    ("subpath", 500),
    ("url", 4_000),
    ("label", 500),
    ("background", 2_000),
];

/// Checks one `styleAttributes` key and value; `None` for a value this
/// editor would not write (it is kept anyway when it came from a file).
fn style_ok(edge: bool, key: &str, value: &Value) -> Option<()> {
    let s = value.as_str()?;
    let allowed: &[&str] = match (edge, key) {
        (false, "shape") => &SHAPES,
        (false, "border") => &BORDERS,
        (false, "textAlign") => &ALIGNS,
        (true, "path") => &PATHS,
        (true, "arrow") => &ARROWS,
        (true, "pathfindingMethod") => &ROUTES,
        _ => return None,
    };
    allowed.contains(&s).then_some(())
}

impl LabCore {
    // ── keeping the canvas in line with the world ──────────────────────────
    /// After any change to the world: device nodes and cable edges follow
    /// the devices and links, and every device node sits where its device is.
    pub(crate) fn sync_canvas(&mut self) {
        let devs: HashSet<String> = self.world.devices.iter().map(|d| d.id.clone()).collect();
        let links: HashSet<String> = self.world.links.iter().map(|l| l.id.clone()).collect();
        self.canvas
            .nodes
            .retain(|n| n.dev.as_ref().is_none_or(|d| devs.contains(d)));
        let nodes: HashSet<String> = self.canvas.nodes.iter().map(|n| n.id.clone()).collect();
        self.canvas.edges.retain(|e| {
            nodes.contains(&e.from_node)
                && nodes.contains(&e.to_node)
                && e.link.as_ref().is_none_or(|l| links.contains(l))
        });
        for i in 0..self.world.devices.len() {
            let d = &self.world.devices[i];
            if self.canvas.node_of_dev(&d.id).is_some() {
                continue;
            }
            let id = self.ids.next(&self.canvas);
            let mut n = Node::blank(id, "text", 0.0, 0.0, DEVICE_W, DEVICE_H);
            n.dev = Some(d.id.clone());
            self.canvas.nodes.push(n);
        }
        let world = &self.world;
        for n in &mut self.canvas.nodes {
            let Some(d) = n.dev.as_ref().and_then(|id| world.dev(id)) else {
                continue;
            };
            n.x = (d.x * SCALE - n.width / 2.0).round();
            n.y = (d.y * SCALE - n.height / 2.0).round();
        }
        let have: HashSet<String> = self
            .canvas
            .edges
            .iter()
            .filter_map(|e| e.link.clone())
            .collect();
        let by_dev: HashMap<String, String> = self
            .canvas
            .nodes
            .iter()
            .filter_map(|n| n.dev.clone().map(|d| (d, n.id.clone())))
            .collect();
        for i in 0..self.world.links.len() {
            let l = &self.world.links[i];
            if have.contains(&l.id) {
                continue;
            }
            let (Some(a), Some(b)) = (by_dev.get(&l.a.dev), by_dev.get(&l.b.dev)) else {
                continue;
            };
            let (a, b, lid) = (a.clone(), b.clone(), l.id.clone());
            let id = self.ids.next(&self.canvas);
            let mut e = Edge::between(id, &a, &b);
            e.to_end = Some("none".into());
            e.link = Some(lid);
            self.canvas.edges.push(e);
        }
    }

    // ── undo ───────────────────────────────────────────────────────────────
    /// Keeps the state before a change. `gesture`: the steps of one drag
    /// share a number and make one undo step.
    pub(crate) fn checkpoint(&mut self, gesture: Option<f64>) {
        if gesture.is_some() && gesture == self.history.gesture {
            return;
        }
        self.history.gesture = gesture;
        self.history
            .undo
            .push((self.canvas.clone(), self.world.clone()));
        if self.history.undo.len() > HISTORY {
            self.history.undo.remove(0);
        }
        self.history.redo.clear();
    }

    fn restore(&mut self, (canvas, world): (Canvas, World)) {
        self.canvas = canvas;
        self.world = world;
        self.history.gesture = None;
        self.changed();
    }

    pub fn canvas_undo(&mut self) -> Value {
        let Some(prev) = self.history.undo.pop() else {
            return err("There is nothing to undo.");
        };
        self.history
            .redo
            .push((self.canvas.clone(), self.world.clone()));
        self.restore(prev);
        json!({ "ok": true })
    }

    pub fn canvas_redo(&mut self) -> Value {
        let Some(next) = self.history.redo.pop() else {
            return err("There is nothing to redo.");
        };
        self.history
            .undo
            .push((self.canvas.clone(), self.world.clone()));
        let redo = std::mem::take(&mut self.history.redo);
        self.restore(next);
        self.history.redo = redo;
        json!({ "ok": true })
    }

    // ── reading ────────────────────────────────────────────────────────────
    /// The canvas as the page draws it: every node and edge with what the
    /// file would say, plus `dev` / `link` for the network's, and each
    /// edge's path, label spot and arrowheads.
    pub fn canvas_snapshot(&self) -> Value {
        let nodes: Vec<Value> = self
            .canvas
            .nodes
            .iter()
            .map(|n| {
                let mut v = self.node_json(n, false);
                if let Some(d) = &n.dev {
                    v["dev"] = d.clone().into();
                }
                v
            })
            .collect();
        let edges: Vec<Value> = self
            .canvas
            .edges
            .iter()
            .filter_map(|e| {
                let from = self.canvas.node(&e.from_node)?;
                let to = self.canvas.node(&e.to_node)?;
                let mut v = self.edge_json(e, false);
                let route = geom::route(from, to, e);
                v["path"] = route.path.into();
                v["labelAt"] = json!([route.label.0, route.label.1]);
                v["heads"] = Value::Array(
                    route
                        .heads
                        .iter()
                        .map(|h| json!({ "x": h.x, "y": h.y, "angle": h.angle, "end": h.end }))
                        .collect(),
                );
                v["sides"] = json!([route.from_side, route.to_side]);
                if let Some(l) = e
                    .link
                    .as_ref()
                    .and_then(|l| self.world.links.iter().find(|x| &x.id == l))
                {
                    v["link"] = l.id.clone().into();
                    v["cable"] = l.kind.clone().into();
                }
                Some(v)
            })
            .collect();
        json!({
            "nodes": nodes,
            "edges": edges,
            "canUndo": self.history.can_undo(),
            "canRedo": self.history.can_redo(),
            "scale": SCALE,
        })
    }

    // ── editing ────────────────────────────────────────────────────────────
    /// Applies the node fields in `o` to `n`; refuses a bad one by name.
    fn apply_node(n: &mut Node, o: &Map<String, Value>) -> Result<(), String> {
        for (k, max) in NODE_TEXT {
            match o.get(k) {
                None => {}
                Some(Value::Null) => *Self::text_slot(n, k) = None,
                Some(Value::String(s)) if s.chars().count() <= max => {
                    *Self::text_slot(n, k) = Some(s.clone())
                }
                Some(_) => return Err(format!("The {k} of a card is too long or not text.")),
            }
        }
        match o.get("backgroundStyle") {
            None => {}
            Some(Value::Null) => n.background_style = None,
            Some(Value::String(s)) if matches!(s.as_str(), "cover" | "ratio" | "repeat") => {
                n.background_style = Some(s.clone())
            }
            Some(_) => return Err("A group's background is cover, ratio or repeat.".into()),
        }
        match o.get("color") {
            None => {}
            Some(Value::Null) => n.color = None,
            Some(Value::String(c)) if c.is_empty() => n.color = None,
            Some(Value::String(c)) if valid_color(c) => n.color = Some(c.clone()),
            Some(_) => return Err("That is not a colour a canvas can hold.".into()),
        }
        for (k, slot) in [("width", 0), ("height", 1)] {
            if let Some(v) = o.get(k) {
                let Some(f) = finite(Some(v)) else {
                    return Err("A card needs a size.".into());
                };
                let f = f.round().clamp(20.0, 20_000.0);
                if slot == 0 {
                    n.width = f;
                } else {
                    n.height = f;
                }
            }
        }
        if let Some(st) = o.get("styleAttributes") {
            let Value::Object(st) = st else {
                return Err("The card's style is not readable.".into());
            };
            for (k, v) in st {
                if v.is_null() {
                    n.style.remove(k);
                } else if style_ok(false, k, v).is_some() {
                    n.style.insert(k.clone(), v.clone());
                } else {
                    return Err(format!("A card has no style {k} of that kind."));
                }
            }
        }
        if let Some(l) = o.get("losos") {
            match l {
                Value::Object(m) => {
                    for (k, v) in m {
                        if k == "device" {
                            continue;
                        }
                        if v.is_null() {
                            n.losos.remove(k);
                        } else {
                            n.losos.insert(k.clone(), v.clone());
                        }
                    }
                }
                _ => return Err("The card's LosOS data is not readable.".into()),
            }
        }
        Ok(())
    }

    fn text_slot<'a>(n: &'a mut Node, k: &str) -> &'a mut Option<String> {
        match k {
            "text" => &mut n.text,
            "file" => &mut n.file,
            "subpath" => &mut n.subpath,
            "url" => &mut n.url,
            "label" => &mut n.label,
            _ => &mut n.background,
        }
    }

    /// `{type, x, y, width?, height?, text?, file?, url?, label?, color?,
    /// styleAttributes?}` → `{node}`.
    pub fn canvas_add_node(&mut self, spec: &str) -> Value {
        let Some(o) = parse::<Map<String, Value>>(spec) else {
            return err("The card is not readable.");
        };
        let kind = o.get("type").and_then(Value::as_str).unwrap_or("text");
        if !NODE_KINDS.contains(&kind) {
            return err("A card is text, a file, a link or a group.");
        }
        if self.canvas.nodes.len() >= MAX_NODES {
            return err("This canvas holds as many cards as it can.");
        }
        let (Some(x), Some(y)) = (finite(o.get("x")), finite(o.get("y"))) else {
            return err("A card needs a place on the canvas.");
        };
        let (w, h) = match kind {
            "group" => (400.0, 300.0),
            "link" => (400.0, 240.0),
            "file" => (400.0, 300.0),
            _ => (250.0, 60.0),
        };
        let id = self.ids.next(&self.canvas);
        let mut n = Node::blank(id, kind, x.round(), y.round(), w, h);
        if kind == "text" {
            n.text = Some(String::new());
        }
        if let Err(e) = Self::apply_node(&mut n, &o) {
            return err(e);
        }
        if kind == "file" && n.file.as_deref().is_none_or(str::is_empty) {
            return err("A file card needs a file.");
        }
        if kind == "link" && n.url.as_deref().is_none_or(str::is_empty) {
            return err("A link card needs an address.");
        }
        self.checkpoint(None);
        let v = self.node_json(&n, false);
        self.canvas.nodes.push(n);
        self.dirty = true;
        json!({ "node": v })
    }

    /// A device from the parts tray, placed on the canvas with its card's
    /// centre at (`cx`, `cy`).
    pub fn canvas_add_device(&mut self, kind: &str, cx: f64, cy: f64) -> Value {
        if crate::catalog::type_info(kind).is_none() {
            return err("There is no such device type.");
        }
        if !cx.is_finite() || !cy.is_finite() {
            return err("A device needs a place on the canvas.");
        }
        let site = if kind == "edge-official" {
            "dc"
        } else {
            "home"
        };
        let r = self.add_device(
            kind,
            (cx / SCALE).round(),
            (cy / SCALE).round(),
            &json!({ "site": site }).to_string(),
        );
        if r.get("error").is_some() {
            return r;
        }
        let dev = r["device"]["id"].as_str().unwrap_or_default().to_string();
        let node = self
            .canvas
            .node_of_dev(&dev)
            .map(|n| self.node_json(n, false));
        json!({ "device": r["device"], "node": node })
    }

    /// `patch`: the node fields to change (`null` clears one). A device
    /// card keeps its text; its name is the device's.
    pub fn canvas_update_node(&mut self, id: &str, patch: &str) -> Value {
        let Some(o) = parse::<Map<String, Value>>(patch) else {
            return err("The change is not readable.");
        };
        let Some(n) = self.canvas.node(id) else {
            return err("There is no such card.");
        };
        let mut next = n.clone();
        let dev = n.dev.clone();
        let mut o = o;
        if dev.is_some() {
            o.remove("text");
        }
        if let Err(e) = Self::apply_node(&mut next, &o) {
            return err(e);
        }
        if next == *n {
            return json!({ "node": self.node_json(n, false) });
        }
        self.checkpoint(None);
        let v = self.node_json(&next, false);
        if let Some(slot) = self.canvas.node_mut(id) {
            *slot = next;
        }
        if dev.is_some() {
            // A device card's size moves its centre; the device follows.
            self.follow_device(id);
        }
        self.dirty = true;
        json!({ "node": v })
    }

    /// Puts a device card's device under the card's centre.
    fn follow_device(&mut self, node: &str) {
        let Some(n) = self.canvas.node(node) else {
            return;
        };
        let Some(dev) = n.dev.clone() else {
            return;
        };
        let (cx, cy) = n.center();
        if let Some(d) = self.world.dev_mut(&dev) {
            d.x = cx / SCALE;
            d.y = cy / SCALE;
        }
    }

    /// Moves cards by (`dx`, `dy`), and with each group what lies inside
    /// it. `gesture`: the drag's number; its steps make one undo step.
    pub fn canvas_move(&mut self, ids: &str, dx: f64, dy: f64, gesture: f64) -> Value {
        let Some(ids) = parse::<Vec<String>>(ids) else {
            return err("The cards to move are not readable.");
        };
        if !dx.is_finite() || !dy.is_finite() {
            return err("A card needs a place on the canvas.");
        }
        let mut moving: Vec<String> = Vec::new();
        for id in &ids {
            if self.canvas.node(id).is_some() && !moving.contains(id) {
                moving.push(id.clone());
            }
        }
        let mut i = 0;
        while i < moving.len() {
            let n = self.canvas.node(&moving[i]).cloned();
            if let Some(g) = n.filter(|n| n.kind == "group") {
                for c in &self.canvas.nodes {
                    if c.inside(&g) && !moving.contains(&c.id) {
                        moving.push(c.id.clone());
                    }
                }
            }
            i += 1;
        }
        if moving.is_empty() {
            return err("There is no such card.");
        }
        self.checkpoint(Some(gesture));
        for id in &moving {
            if let Some(n) = self.canvas.node_mut(id) {
                n.x += dx;
                n.y += dy;
            }
            self.follow_device(id);
        }
        self.dirty = true;
        json!({ "moved": moving })
    }

    /// One card's new box (`{x, y, width, height}`), from a resize handle.
    pub fn canvas_resize(&mut self, id: &str, rect: &str, gesture: f64) -> Value {
        let Some(o) = parse::<Map<String, Value>>(rect) else {
            return err("The size is not readable.");
        };
        let vals: Vec<Option<f64>> = ["x", "y", "width", "height"]
            .iter()
            .map(|k| finite(o.get(*k)))
            .collect();
        let [Some(x), Some(y), Some(w), Some(h)] = vals[..] else {
            return err("A card needs a place and a size.");
        };
        if self.canvas.node(id).is_none() {
            return err("There is no such card.");
        }
        self.checkpoint(Some(gesture));
        if let Some(n) = self.canvas.node_mut(id) {
            n.width = w.round().clamp(20.0, 20_000.0);
            n.height = h.round().clamp(20.0, 20_000.0);
            n.x = x.round();
            n.y = y.round();
        }
        self.follow_device(id);
        self.dirty = true;
        json!({ "ok": true })
    }

    /// `{fromNode, fromSide?, toNode, toSide?, fromEnd?, toEnd?, label?,
    /// color?, styleAttributes?, cable?}`. With `cable` (a link kind) and
    /// two devices at the ends, the edge is a real cable in the world.
    pub fn canvas_connect(&mut self, spec: &str) -> Value {
        let Some(o) = parse::<Map<String, Value>>(spec) else {
            return err("The arrow is not readable.");
        };
        let from = o.get("fromNode").and_then(Value::as_str).unwrap_or("");
        let to = o.get("toNode").and_then(Value::as_str).unwrap_or("");
        let (Some(a), Some(b)) = (
            self.canvas.node(from).cloned(),
            self.canvas.node(to).cloned(),
        ) else {
            return err("An arrow goes from one card to another.");
        };
        if from == to {
            return err("An arrow goes from one card to another.");
        }
        if self.canvas.edges.len() >= MAX_EDGES {
            return err("This canvas holds as many arrows as it can.");
        }
        let id = self.ids.next(&self.canvas);
        let mut e = Edge::between(id, from, to);
        if let Err(m) = Self::apply_edge(&mut e, &o) {
            return err(m);
        }
        let cable = o
            .get("cable")
            .and_then(Value::as_str)
            .filter(|c| !c.is_empty());
        if let Some(kind) = cable {
            let (Some(da), Some(db)) = (a.dev.clone(), b.dev.clone()) else {
                return err("A cable goes between two devices.");
            };
            let r = self.connect(&da, &db, kind, None, None);
            if r.get("error").is_some() {
                return r;
            }
            let lid = r["link"]["id"].as_str().unwrap_or_default().to_string();
            // sync made the cable's edge; it takes the sides and style drawn.
            if let Some(edge) = self
                .canvas
                .edges
                .iter_mut()
                .find(|x| x.link.as_deref() == Some(&lid))
            {
                let keep = (
                    edge.id.clone(),
                    edge.from_node.clone(),
                    edge.to_node.clone(),
                );
                let flip = keep.1 != from;
                *edge = Edge {
                    id: keep.0,
                    from_node: keep.1,
                    to_node: keep.2,
                    link: Some(lid.clone()),
                    to_end: e.to_end.clone().or(Some("none".into())),
                    from_side: if flip {
                        e.to_side.clone()
                    } else {
                        e.from_side.clone()
                    },
                    to_side: if flip {
                        e.from_side.clone()
                    } else {
                        e.to_side.clone()
                    },
                    ..e
                };
            }
            let v = self
                .canvas
                .edges
                .iter()
                .find(|x| x.link.as_deref() == Some(&lid))
                .map(|x| self.edge_json(x, false));
            return json!({ "edge": v, "link": r["link"] });
        }
        self.checkpoint(None);
        let v = self.edge_json(&e, false);
        self.canvas.edges.push(e);
        self.dirty = true;
        json!({ "edge": v })
    }

    fn apply_edge(e: &mut Edge, o: &Map<String, Value>) -> Result<(), String> {
        for (k, slot) in [("fromSide", 0), ("toSide", 1)] {
            match o.get(k) {
                None => {}
                Some(Value::Null) => {
                    *if slot == 0 {
                        &mut e.from_side
                    } else {
                        &mut e.to_side
                    } = None
                }
                Some(Value::String(s)) if SIDES.contains(&s.as_str()) => {
                    *if slot == 0 {
                        &mut e.from_side
                    } else {
                        &mut e.to_side
                    } = Some(s.clone())
                }
                Some(_) => return Err("A side is top, right, bottom or left.".into()),
            }
        }
        for (k, slot) in [("fromEnd", 0), ("toEnd", 1)] {
            match o.get(k) {
                None => {}
                Some(Value::Null) => {
                    *if slot == 0 {
                        &mut e.from_end
                    } else {
                        &mut e.to_end
                    } = None
                }
                Some(Value::String(s)) if matches!(s.as_str(), "none" | "arrow") => {
                    *if slot == 0 {
                        &mut e.from_end
                    } else {
                        &mut e.to_end
                    } = Some(s.clone())
                }
                Some(_) => return Err("An arrow's end is none or arrow.".into()),
            }
        }
        match o.get("label") {
            None => {}
            Some(Value::Null) => e.label = None,
            Some(Value::String(s)) if s.is_empty() => e.label = None,
            Some(Value::String(s)) if s.chars().count() <= 500 => e.label = Some(s.clone()),
            Some(_) => return Err("An arrow's label is short text.".into()),
        }
        match o.get("color") {
            None => {}
            Some(Value::Null) => e.color = None,
            Some(Value::String(c)) if c.is_empty() => e.color = None,
            Some(Value::String(c)) if valid_color(c) => e.color = Some(c.clone()),
            Some(_) => return Err("That is not a colour a canvas can hold.".into()),
        }
        if let Some(st) = o.get("styleAttributes") {
            let Value::Object(st) = st else {
                return Err("The arrow's style is not readable.".into());
            };
            for (k, v) in st {
                if v.is_null() {
                    e.style.remove(k);
                } else if style_ok(true, k, v).is_some() {
                    e.style.insert(k.clone(), v.clone());
                } else {
                    return Err(format!("An arrow has no style {k} of that kind."));
                }
            }
        }
        Ok(())
    }

    /// `patch`: the edge fields to change, as in `canvas_connect`, plus
    /// `fromNode` / `toNode` to move an end to another card (not a cable's).
    pub fn canvas_update_edge(&mut self, id: &str, patch: &str) -> Value {
        let Some(o) = parse::<Map<String, Value>>(patch) else {
            return err("The change is not readable.");
        };
        let Some(e) = self.canvas.edge(id) else {
            return err("There is no such arrow.");
        };
        let mut next = e.clone();
        if let Err(m) = Self::apply_edge(&mut next, &o) {
            return err(m);
        }
        for (k, slot) in [("fromNode", 0), ("toNode", 1)] {
            let Some(v) = o.get(k).and_then(Value::as_str) else {
                continue;
            };
            if next.link.is_some() {
                return err("A cable's ends are its devices; draw a new cable instead.");
            }
            if self.canvas.node(v).is_none() {
                return err("There is no such card.");
            }
            if slot == 0 {
                next.from_node = v.into();
            } else {
                next.to_node = v.into();
            }
        }
        if next.from_node == next.to_node {
            return err("An arrow goes from one card to another.");
        }
        if next == *e {
            return json!({ "edge": self.edge_json(e, false) });
        }
        self.checkpoint(None);
        let v = self.edge_json(&next, false);
        if let Some(slot) = self.canvas.edge_mut(id) {
            *slot = next;
        }
        self.dirty = true;
        json!({ "edge": v })
    }

    /// `{nodes: [id], edges: [id]}`: a device card takes its device with it,
    /// and a cable its link.
    pub fn canvas_delete(&mut self, what: &str) -> Value {
        let Some(o) = parse::<Map<String, Value>>(what) else {
            return err("What to delete is not readable.");
        };
        let list = |k: &str| -> Vec<String> {
            o.get(k)
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        let (nodes, edges) = (list("nodes"), list("edges"));
        let real_nodes: Vec<String> = nodes
            .into_iter()
            .filter(|n| self.canvas.node(n).is_some())
            .collect();
        let real_edges: Vec<String> = edges
            .into_iter()
            .filter(|e| self.canvas.edge(e).is_some())
            .collect();
        if real_nodes.is_empty() && real_edges.is_empty() {
            return err("There is nothing to delete.");
        }
        self.checkpoint(None);
        let mut world = false;
        for id in &real_edges {
            if let Some(l) = self.canvas.edge(id).and_then(|e| e.link.clone()) {
                self.world.remove_link(&l);
                world = true;
            }
        }
        for id in &real_nodes {
            if let Some(d) = self.canvas.node(id).and_then(|n| n.dev.clone()) {
                self.world.remove_device(&d);
                world = true;
            }
        }
        self.canvas.edges.retain(|e| !real_edges.contains(&e.id));
        self.canvas.nodes.retain(|n| !real_nodes.contains(&n.id));
        let left: HashSet<String> = self.canvas.nodes.iter().map(|n| n.id.clone()).collect();
        self.canvas
            .edges
            .retain(|e| left.contains(&e.from_node) && left.contains(&e.to_node));
        if world {
            self.changed();
        } else {
            self.dirty = true;
        }
        json!({ "nodes": real_nodes, "edges": real_edges })
    }

    /// `how`: "front" or "back". Groups are drawn under the other cards
    /// whatever their order, as in Obsidian.
    pub fn canvas_order(&mut self, ids: &str, how: &str) -> Value {
        let Some(ids) = parse::<Vec<String>>(ids) else {
            return err("The cards are not readable.");
        };
        if !matches!(how, "front" | "back") {
            return err("Cards go to the front or to the back.");
        }
        let (mut picked, rest): (Vec<Node>, Vec<Node>) = self
            .canvas
            .nodes
            .iter()
            .cloned()
            .partition(|n| ids.contains(&n.id));
        if picked.is_empty() {
            return err("There is no such card.");
        }
        self.checkpoint(None);
        self.canvas.nodes = if how == "front" {
            let mut v = rest;
            v.append(&mut picked);
            v
        } else {
            picked.extend(rest);
            picked
        };
        self.dirty = true;
        json!({ "ok": true })
    }

    /// Lines the cards up: `how` is left, center, right, top, middle, bottom,
    /// or spreads them evenly: horizontal, vertical.
    pub fn canvas_align(&mut self, ids: &str, how: &str) -> Value {
        let Some(ids) = parse::<Vec<String>>(ids) else {
            return err("The cards are not readable.");
        };
        let mut nodes: Vec<Node> = self
            .canvas
            .nodes
            .iter()
            .filter(|n| ids.contains(&n.id))
            .cloned()
            .collect();
        if nodes.len() < 2 {
            return err("Pick two cards or more to line up.");
        }
        let left = nodes.iter().map(|n| n.x).fold(f64::INFINITY, f64::min);
        let right = nodes
            .iter()
            .map(Node::right)
            .fold(f64::NEG_INFINITY, f64::max);
        let top = nodes.iter().map(|n| n.y).fold(f64::INFINITY, f64::min);
        let bottom = nodes
            .iter()
            .map(Node::bottom)
            .fold(f64::NEG_INFINITY, f64::max);
        match how {
            "left" => nodes.iter_mut().for_each(|n| n.x = left),
            "right" => nodes.iter_mut().for_each(|n| n.x = right - n.width),
            "center" => nodes
                .iter_mut()
                .for_each(|n| n.x = ((left + right) / 2.0 - n.width / 2.0).round()),
            "top" => nodes.iter_mut().for_each(|n| n.y = top),
            "bottom" => nodes.iter_mut().for_each(|n| n.y = bottom - n.height),
            "middle" => nodes
                .iter_mut()
                .for_each(|n| n.y = ((top + bottom) / 2.0 - n.height / 2.0).round()),
            "horizontal" | "vertical" => {
                let h = how == "horizontal";
                nodes.sort_by(|a, b| {
                    let (ka, kb) = if h { (a.x, b.x) } else { (a.y, b.y) };
                    ka.total_cmp(&kb)
                });
                let total: f64 = nodes
                    .iter()
                    .map(|n| if h { n.width } else { n.height })
                    .sum();
                let span = if h { right - left } else { bottom - top };
                let gap = ((span - total) / (nodes.len() - 1) as f64).max(0.0);
                let mut at = if h { left } else { top };
                for n in &mut nodes {
                    if h {
                        n.x = at.round();
                        at += n.width + gap;
                    } else {
                        n.y = at.round();
                        at += n.height + gap;
                    }
                }
            }
            _ => return err("There is no such way to line cards up."),
        }
        self.checkpoint(None);
        for n in nodes {
            let id = n.id.clone();
            if let Some(slot) = self.canvas.node_mut(&id) {
                slot.x = n.x;
                slot.y = n.y;
            }
            self.follow_device(&id);
        }
        self.dirty = true;
        json!({ "ok": true })
    }

    /// A group around the cards, `padding` 40 on every side, labelled.
    pub fn canvas_group(&mut self, ids: &str, label: &str) -> Value {
        let Some(ids) = parse::<Vec<String>>(ids) else {
            return err("The cards are not readable.");
        };
        let nodes: Vec<&Node> = self
            .canvas
            .nodes
            .iter()
            .filter(|n| ids.contains(&n.id))
            .collect();
        if nodes.is_empty() {
            return err("Pick the cards to group.");
        }
        let pad = 40.0;
        let left = nodes.iter().map(|n| n.x).fold(f64::INFINITY, f64::min) - pad;
        let top = nodes.iter().map(|n| n.y).fold(f64::INFINITY, f64::min) - pad;
        let right = nodes
            .iter()
            .map(|n| n.right())
            .fold(f64::NEG_INFINITY, f64::max)
            + pad;
        let bottom = nodes
            .iter()
            .map(|n| n.bottom())
            .fold(f64::NEG_INFINITY, f64::max)
            + pad;
        self.checkpoint(None);
        let id = self.ids.next(&self.canvas);
        let mut g = Node::blank(id, "group", left, top, right - left, bottom - top);
        if !label.trim().is_empty() {
            g.label = Some(crate::js::slice16(label.trim(), 500));
        }
        let v = self.node_json(&g, false);
        // Under everything, so it never hides the cards it holds.
        self.canvas.nodes.insert(0, g);
        self.dirty = true;
        json!({ "node": v })
    }

    // ── fragments: templates, copy and paste ───────────────────────────────
    /// The cards `ids` (and the edges between them) as a JSON Canvas
    /// document, for the clipboard. Device cards carry their device.
    pub fn canvas_copy(&self, ids: &str) -> Value {
        let Some(ids) = parse::<Vec<String>>(ids) else {
            return err("The cards are not readable.");
        };
        let nodes: Vec<Value> = self
            .canvas
            .nodes
            .iter()
            .filter(|n| ids.contains(&n.id))
            .map(|n| self.node_json(n, true))
            .collect();
        let edges: Vec<Value> = self
            .canvas
            .edges
            .iter()
            .filter(|e| ids.contains(&e.from_node) && ids.contains(&e.to_node))
            .map(|e| self.edge_json(e, true))
            .collect();
        if nodes.is_empty() {
            return err("There is nothing to copy.");
        }
        json!({ "text": serde_json::to_string(&json!({ "nodes": nodes, "edges": edges })).unwrap_or_default() })
    }

    /// Puts a JSON Canvas fragment on the canvas with its top-left corner at
    /// (`x`, `y`), every id fresh; device cards become new devices and
    /// cables between them new links. → `{nodes: [id], edges: [id]}`.
    pub fn canvas_paste(&mut self, text: &str, x: f64, y: f64) -> Value {
        let doc = match file::parse_canvas_text(text) {
            Ok(d) => d,
            Err(e) => return err(e),
        };
        if !x.is_finite() || !y.is_finite() {
            return err("A card needs a place on the canvas.");
        }
        self.checkpoint(None);
        match self.insert_fragment(&doc, x, y) {
            Ok(v) => v,
            Err(e) => {
                if let Some(prev) = self.history.undo.pop() {
                    self.canvas = prev.0;
                    self.world = prev.1;
                }
                self.history.gesture = None;
                err(e)
            }
        }
    }

    pub fn canvas_templates(&self) -> Value {
        Value::Array(
            templates::TEMPLATES
                .iter()
                .map(|t| json!({ "key": t.key }))
                .collect(),
        )
    }

    /// A template, in the language `lang` (en, sk, de), centred on (`cx`, `cy`).
    pub fn canvas_insert_template(&mut self, key: &str, lang: &str, cx: f64, cy: f64) -> Value {
        let Some(doc) = templates::template(key, lang) else {
            return err("There is no such template.");
        };
        if !cx.is_finite() || !cy.is_finite() {
            return err("A card needs a place on the canvas.");
        }
        let (w, h) = file::fragment_size(&doc);
        self.checkpoint(None);
        match self.insert_fragment(&doc, (cx - w / 2.0).round(), (cy - h / 2.0).round()) {
            Ok(v) => v,
            Err(e) => {
                self.history.drop_last();
                err(e)
            }
        }
    }

    /// A fresh, empty diagram: no cards, no devices.
    pub fn canvas_new(&mut self) -> Value {
        self.file_name = String::new();
        self.start_world("blank", |_| None);
        self.dirty = false;
        json!({ "ok": true })
    }

    /// Names the diagram; the name travels in the file as `losos.name`.
    pub fn rename_doc(&mut self, name: &str) -> Value {
        let name = name.trim();
        if name.is_empty() {
            return err("A diagram needs a name.");
        }
        if name.chars().count() > 60 || name.chars().any(char::is_control) {
            return err("A diagram's name is one line of at most 60 characters.");
        }
        self.file_name = name.to_string();
        if self.scenario != "file" && self.scenario != "last" {
            self.scenario = "file".into();
        }
        self.dirty = true;
        json!({ "name": name })
    }
}
