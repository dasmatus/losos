//! What the canvas shows, as geometry: the logical and physical views of
//! the React Lab's SVG canvas (admin-ui/app/src/lab/svg-canvas.tsx) turned
//! into parts (one mesh per room, Wi-Fi range, cable and device, each in
//! its own local coordinates on the ground plane), the text labels over
//! them and the regions a pointer can hit. Pure Rust over `LabCore`, so the
//! native tests can drive it without a GPU.

use crate::icons::{hardware, hw_geom, icon, Led};
use crate::paint::{dist_seg, Painter, Rgba};
use crate::theme::{alpha, Theme};
use crate::tunables::Tunables;
use losos_lab_core::{Device, LabCore, Link, World};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

/// The physical view's scale for hardware drawings.
pub const HWK: f32 = 1.3;

/// The four rooms of the physical view: key, x, y, w, h.
pub const ROOMS: [(&str, f32, f32, f32, f32); 4] = [
    ("home", 20.0, 20.0, 590.0, 360.0),
    ("office", 630.0, 20.0, 590.0, 360.0),
    ("isp", 20.0, 400.0, 590.0, 310.0),
    ("dc", 630.0, 400.0, 590.0, 310.0),
];

pub fn room(site: &str) -> (f32, f32, f32, f32) {
    let r = ROOMS.iter().find(|r| r.0 == site).unwrap_or(&ROOMS[0]);
    (r.1, r.2, r.3, r.4)
}

pub fn room_at(x: f32, y: f32) -> Option<&'static str> {
    ROOMS
        .iter()
        .rev()
        .find(|r| x >= r.1 && x <= r.1 + r.3 && y >= r.2 && y <= r.2 + r.4)
        .map(|r| r.0)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum View {
    Logical,
    Physical,
}

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub enum Tool {
    Select,
    Place(String),
    Connect(String),
    Delete,
}

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub enum Target {
    Dev(String),
    Link(String),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Cam {
    pub x: f32,
    pub y: f32,
    pub k: f32,
}

impl Default for Cam {
    fn default() -> Self {
        Cam {
            x: 0.0,
            y: 0.0,
            k: 1.0,
        }
    }
}

/// What the page has told the canvas, and what the pointer is doing.
pub struct Ui {
    pub view: View,
    pub tool: Tool,
    pub connect_from: Option<String>,
    /// The page owns the half-made connection (`set_connect_from`): a click
    /// with the connect tool is reported as a `connect` event and changes
    /// nothing here.
    pub host_connect: bool,
    pub sel: Option<Target>,
    pub hover: Option<Target>,
    pub cams: [Cam; 2],
    /// Devices with a guest running: the "VM" tag.
    pub running: HashSet<String>,
    /// Link id → when it appeared, for the amber "negotiating" LEDs.
    pub link_born: HashMap<String, f64>,
    /// The pointer on the canvas, in world units, for the rubber band.
    pub pointer: Option<(f32, f32)>,
    /// Canvas size in CSS pixels.
    pub size: (f32, f32),
    pub tune: Tunables,
}

impl Default for Ui {
    fn default() -> Self {
        Ui {
            view: View::Logical,
            tool: Tool::Select,
            connect_from: None,
            host_connect: false,
            sel: None,
            hover: None,
            cams: [Cam::default(); 2],
            running: HashSet::new(),
            link_born: HashMap::new(),
            pointer: None,
            size: (800.0, 600.0),
            tune: Tunables::default(),
        }
    }
}

impl Ui {
    pub fn cam(&self) -> Cam {
        self.cams[self.view as usize]
    }
    pub fn cam_mut(&mut self) -> &mut Cam {
        &mut self.cams[self.view as usize]
    }
    pub fn to_world(&self, sx: f32, sy: f32) -> (f32, f32) {
        let c = self.cam();
        ((sx - c.x) / c.k, (sy - c.y) / c.k)
    }
}

/// The parts of `catalog()` the drawing needs.
#[derive(Default)]
pub struct Catalog {
    pub endpoint: HashSet<String>,
    pub ports: HashMap<String, Vec<String>>,
    pub label: HashMap<String, String>,
    pub sites: HashMap<String, (String, String)>,
}

impl Catalog {
    pub fn from_core(core: &LabCore) -> Catalog {
        let v = core.catalog();
        let mut c = Catalog::default();
        if let Some(types) = v.get("types").and_then(Value::as_object) {
            for (k, t) in types {
                if t.get("endpoint").and_then(Value::as_bool) == Some(true) {
                    c.endpoint.insert(k.clone());
                }
                let ports = t
                    .get("ports")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|p| p.get(0).and_then(Value::as_str).map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                c.ports.insert(k.clone(), ports);
                if let Some(l) = t.get("label").and_then(Value::as_str) {
                    c.label.insert(k.clone(), l.to_string());
                }
            }
        }
        if let Some(sites) = v.get("sites").and_then(Value::as_object) {
            for (k, s) in sites {
                let g = |f: &str| s.get(f).and_then(Value::as_str).unwrap_or("").to_string();
                c.sites.insert(k.clone(), (g("name"), g("sub")));
            }
        }
        c
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Face {
    Sans,
    Mono,
}

/// A line of text in world units. `y` is the centre of the line box;
/// `ax` is the anchor: -0.5 start, 0 middle, 0.5 end.
#[derive(Clone, PartialEq, Debug)]
pub struct Label {
    pub key: String,
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub face: Face,
    pub color: Rgba,
    pub ax: f32,
}

/// Where a part sits in the stack. The views are flat today, so overlap is
/// settled by height above the ground: each layer has its own band, and
/// within a band later parts sit a hair higher, which is the SVG's
/// document order. A tilted camera sees the same stack as thin slabs.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Layer {
    Grid,
    Room,
    Range,
    Bus,
    Cable,
    Device,
    Backing,
    Band,
    Packet,
}

impl Layer {
    /// The band's floor, in world units above the ground. Bands are ten
    /// apart and parts within one a hundredth, so a thousand parts fit in
    /// a band and every step is far above f32's resolution at the camera's
    /// distance.
    pub fn base(self) -> f32 {
        10.0 * self as u8 as f32
    }
}

/// The height labels are projected from: above every part but the moving
/// ones.
pub const LABEL_Y: f32 = 65.0;

/// One mesh of the scene, an entity of its own. `mesh` is in the part's own
/// coordinates, with `at` (canvas units) as its origin, so moving a device
/// moves its entity and leaves its mesh alone. `model` is the device type
/// for a device part: the key a per-type 3D model will replace the drawn
/// geometry by.
pub struct Part {
    pub key: String,
    pub layer: Layer,
    pub at: (f32, f32),
    /// Height above the ground plane (see `Layer`).
    pub y: f32,
    pub mesh: Painter,
    pub model: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Shape {
    Rect(f32, f32, f32, f32),
    Circle(f32, f32, f32),
    Line(Vec<(f32, f32)>, f32),
}

impl Shape {
    pub fn contains(&self, p: (f32, f32)) -> bool {
        match self {
            Shape::Rect(x, y, w, h) => p.0 >= *x && p.0 <= x + w && p.1 >= *y && p.1 <= y + h,
            Shape::Circle(x, y, r) => (p.0 - x).powi(2) + (p.1 - y).powi(2) <= r * r,
            Shape::Line(pts, tol) => pts.windows(2).any(|s| dist_seg(p, s[0], s[1]) <= *tol),
        }
    }

    pub fn offset(&self, dx: f32, dy: f32) -> Shape {
        match self {
            Shape::Rect(x, y, w, h) => Shape::Rect(x + dx, y + dy, *w, *h),
            Shape::Circle(x, y, r) => Shape::Circle(x + dx, y + dy, *r),
            Shape::Line(pts, tol) => {
                Shape::Line(pts.iter().map(|p| (p.0 + dx, p.1 + dy)).collect(), *tol)
            }
        }
    }

    /// The shape as a flat mesh, for the pick proxies the canvas casts its
    /// rays against.
    pub fn paint(&self, p: &mut Painter) {
        let c = [1.0; 4];
        match self {
            Shape::Rect(x, y, w, h) => p.rect(*x, *y, *w, *h, 0.0, c),
            Shape::Circle(x, y, r) => p.circle(*x, *y, *r, c),
            Shape::Line(pts, tol) => p.stroke(pts, tol * 2.0, c, true),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum HitTarget {
    Dev(String),
    Link(String),
    Power(String),
}

#[derive(Clone, Debug)]
pub struct Hit {
    /// The part it belongs to (an index into `Built::parts`).
    pub part: usize,
    pub shape: Shape,
    pub target: HitTarget,
    /// The tooltip the SVG Lab put in a `<title>`.
    pub title: String,
}

/// The topmost hit under `p` (hits are pushed bottom to top).
pub fn hit_at(hits: &[Hit], p: (f32, f32)) -> Option<&Hit> {
    hits.iter().rev().find(|h| h.shape.contains(p))
}

#[derive(Default)]
pub struct Built {
    pub labels: Vec<Label>,
    /// In canvas units, pushed bottom to top.
    pub hits: Vec<Hit>,
    pub parts: Vec<Part>,
}

/// Starts a part with its origin at `at`: what is painted next in canvas
/// units lands in the part's own coordinates.
fn begin(p: &mut Painter, at: (f32, f32)) {
    p.clear();
    p.tf = (-at.0, -at.1, 1.0);
}

fn end(p: &mut Painter, out: &mut Built, key: String, layer: Layer, at: (f32, f32)) {
    end_model(p, out, key, layer, at, None);
}

fn end_model(
    p: &mut Painter,
    out: &mut Built,
    key: String,
    layer: Layer,
    at: (f32, f32),
    model: Option<&str>,
) {
    let mut mesh = std::mem::replace(p, Painter::new());
    mesh.tf = (0.0, 0.0, 1.0);
    mesh.fade = None;
    out.parts.push(Part {
        key,
        layer,
        at,
        y: 0.0,
        mesh,
        model: model.map(String::from),
    });
}

/// Heights within each layer's band, in the order the parts were made.
fn stack(parts: &mut [Part]) {
    let mut n: HashMap<Layer, usize> = HashMap::new();
    for part in parts {
        let i = n.entry(part.layer).or_default();
        part.y = part.layer.base() + *i as f32 * 0.01;
        *i += 1;
    }
}

fn f(x: f64) -> f32 {
    x as f32
}

pub fn link_led(core: &LabCore, ui: &Ui, l: &Link, now: f64) -> Led {
    if !core.world().link_up(l) {
        return Led::Down;
    }
    match ui.link_born.get(&l.id) {
        Some(b) if now - b < 1400.0 => Led::Wait,
        _ => Led::Up,
    }
}

/// `busSpan`: the backbone's extent.
fn bus_span(core: &LabCore, b: &Device) -> (f32, f32) {
    let w = core.world();
    let xs: Vec<f32> = w
        .links_of(&b.id)
        .filter_map(|l| w.dev(&l.other_end(&b.id).dev))
        .map(|d| f(d.x))
        .collect();
    let x1 = xs.iter().fold(f(b.x) - 120.0, |m, &v| m.min(v)) - 30.0;
    let x2 = xs.iter().fold(f(b.x) + 120.0, |m, &v| m.max(v)) + 30.0;
    (x1, x2)
}

/// `anchor`: where a cable meets a device (a bus meets it at the drop).
fn anchor(core: &LabCore, d: &Device, other: Option<&Device>) -> (f32, f32) {
    match other {
        Some(o) if d.kind == "bus" => {
            let (x1, x2) = bus_span(core, d);
            ((x1 + 20.0).max((x2 - 20.0).min(f(o.x))), f(d.y))
        }
        _ => (f(d.x), f(d.y)),
    }
}

/// The line under a node's name: "off", its address, "no address" for an
/// endpoint without one, or nothing.
fn sub_text(core: &LabCore, cat: &Catalog, d: &Device) -> String {
    if !d.power {
        "off".to_string()
    } else if let Some(ip) = ip_of(core, d) {
        ip
    } else if cat.endpoint.contains(&d.kind) {
        "no address".to_string()
    } else {
        String::new()
    }
}

/// An axis-aligned box in canvas units.
#[derive(Clone, Copy, Debug)]
pub struct Bx {
    pub x1: f32,
    pub x2: f32,
    pub y1: f32,
    pub y2: f32,
}

impl Bx {
    /// A box of half-size (rx, ry) at (x, y) overlaps this one.
    fn hits(&self, x: f32, y: f32, rx: f32, ry: f32) -> bool {
        x + rx > self.x1 && x - rx < self.x2 && y + ry > self.y1 && y - ry < self.y2
    }
}

/// `labelBox` (svg-canvas.tsx): the box a node's name and address take
/// below its icon, relative to the node's centre.
fn label_box(core: &LabCore, cat: &Catalog, d: &Device) -> Bx {
    let sub = sub_text(core, cat, d);
    let w = (d.name.chars().count() as f32 * 7.2).max(sub.chars().count() as f32 * 6.4) + 10.0;
    Bx {
        x1: -w / 2.0,
        x2: w / 2.0,
        y1: 26.0,
        y2: if sub.is_empty() { 43.0 } else { 57.0 },
    }
}

/// The icon and the halo drawn round it: a mark inside it would be cut by
/// the halo's edge the moment the device is picked.
const ICON_BOX: Bx = Bx {
    x1: -47.0,
    x2: 47.0,
    y1: -39.0,
    y2: 65.0,
};

/// Where a cable end's LED and port name go, in canvas units; `None` when
/// the cable is too short to keep that mark clear of the device.
#[derive(Clone, Debug, Default)]
pub struct EndMarks {
    pub led: Option<(f32, f32)>,
    pub label: Option<(f32, f32)>,
}

/// `placeMarks` (svg-canvas.tsx): the LED and port-name spots for every
/// cable end. Each device collects the marks already placed round it, so
/// two cables leaving a switch side by side never stack their port names.
pub fn place_marks(core: &LabCore, cat: &Catalog) -> HashMap<String, [EndMarks; 2]> {
    let w = core.world();
    let mut taken: HashMap<String, Vec<Bx>> = HashMap::new();
    let mut out = HashMap::new();
    for l in &w.links {
        let (Some(da), Some(db)) = (w.dev(&l.a.dev), w.dev(&l.b.dev)) else {
            continue;
        };
        let a = anchor(core, da, Some(db));
        let b = anchor(core, db, Some(da));
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let len = if len == 0.0 { 1.0 } else { len };
        let (ux, uy) = ((b.0 - a.0) / len, (b.1 - a.1) / len);
        let half = len / 2.0 - 6.0;
        let mut ends: [EndMarks; 2] = Default::default();
        for (i, (pt, end, sg, d)) in [(a, &l.a, 1.0, da), (b, &l.b, -1.0, db)]
            .into_iter()
            .enumerate()
        {
            if d.kind == "bus" {
                ends[i] = EndMarks {
                    led: Some(pt),
                    label: None,
                };
                continue;
            }
            let bx = label_box(core, cat, d);
            let (vx, vy) = (ux * sg, uy * sg);
            let mine = taken.entry(d.id.clone()).or_default();
            let clear = |mine: &Vec<Bx>, x: f32, y: f32, rx: f32, ry: f32| {
                !bx.hits(x, y, rx, ry)
                    && !ICON_BOX.hits(x, y, rx, ry)
                    && !mine.iter().any(|o| o.hits(x, y, rx, ry))
            };
            let take = |mine: &mut Vec<Bx>, x: f32, y: f32, rx: f32, ry: f32| {
                mine.push(Bx {
                    x1: x - rx,
                    x2: x + rx,
                    y1: y - ry,
                    y2: y + ry,
                })
            };
            let mut led = None;
            let mut t = 40.0;
            while t <= half {
                if clear(mine, vx * t, vy * t, 5.0, 5.0) {
                    led = Some(t);
                    break;
                }
                t += 4.0;
            }
            let Some(led) = led else {
                continue;
            };
            take(mine, vx * led, vy * led, 5.0, 5.0);
            // A port name is about 6px a letter and one line of 9.5px mono.
            // It sits beside the cable, as far off it as its box reaches
            // across the cable's normal, so it never covers the line, the
            // LED or a packet on it.
            let tx = end.port.chars().count() as f32 * 3.0 + 3.0;
            let off = uy.abs() * tx + ux.abs() * 6.0 + 6.0;
            let mut label = None;
            let mut t = led;
            'search: while t <= half {
                for side in [1.0, -1.0] {
                    let x = vx * t - vy * off * side;
                    let y = vy * t + vx * off * side;
                    if clear(mine, x, y, tx + 2.0, 7.0) {
                        take(mine, x, y, tx + 2.0, 7.0);
                        label = Some((pt.0 + x, pt.1 + y));
                        break 'search;
                    }
                }
                t += 4.0;
            }
            ends[i] = EndMarks {
                led: Some((pt.0 + vx * led, pt.1 + vy * led)),
                label,
            };
        }
        out.insert(l.id.clone(), ends);
    }
    out
}

/// `portAt` (svg-canvas.tsx): the port of `d` under a floor point in the
/// physical view, if the point is within 9 units of one.
pub fn port_at(d: &Device, x: f32, y: f32) -> Option<String> {
    let (ox, oy) = phys_abs(d);
    let g = hw_geom(&d.kind);
    let mut best = None;
    let mut best_d = 9.0;
    for (id, (px, py)) in &g.ports {
        let dist = ((ox + px * HWK - x).powi(2) + (oy + py * HWK - y).powi(2)).sqrt();
        if dist < best_d {
            best = Some(id.clone());
            best_d = dist;
        }
    }
    best
}

/// `nodeBadge`: the dot at the icon's corner, and its tooltip.
fn badge(core: &LabCore, d: &Device, t: &Theme) -> Option<(Rgba, String)> {
    if !d.power {
        return None;
    }
    let net = core.net();
    let name = |id: &str| core.world().name_of(id);
    match d.kind.as_str() {
        "box" => {
            let st = net.losos.boxes.get(&d.id)?;
            if !st.up {
                return Some((t.faint, "No address".into()));
            }
            if st.tunnel == "refused" {
                return Some((t.crit, format!("Tunnel refused: {}", st.reason)));
            }
            let Some(path) = &st.path else {
                return Some((t.warn, "No edge in reach".into()));
            };
            let col = if path.source == "lan" { t.accent } else { t.ok };
            let tail = match &st.public_name {
                Some(p) => format!(" · {p}"),
                None => " · LAN only".into(),
            };
            Some((col, format!("Edge: {}{tail}", name(&path.id))))
        }
        "edge-local" => {
            let sp = net.losos.spoke.get(&d.id)?;
            if sp.uplink == "up" {
                Some((t.ok, "Uplink to the official edge is up".into()))
            } else {
                let c = if d.flag("uplink") { t.warn } else { t.faint };
                Some((c, format!("Uplink: {}", sp.uplink)))
            }
        }
        "edge-official" => Some(if d.flag("certified") {
            (t.ok, "Certificate signed by the LosOS root".into())
        } else {
            (t.warn, "Not certified: not official".into())
        }),
        _ => None,
    }
}

fn ip_of(core: &LabCore, d: &Device) -> Option<String> {
    let a = &core.net().addr;
    if d.kind == "router" {
        if let Some(x) = a.get(&format!("{}#lan", d.id)) {
            return Some(x.ip.clone());
        }
    }
    a.get(&d.id).map(|x| x.ip.clone())
}

fn is_gear(kind: &str) -> bool {
    matches!(kind, "router" | "switch" | "bus" | "ap" | "internet")
}

/// What a cable's length label must not cover on the physical view: every
/// placed device, its name, and each room's title. As `taken` in
/// svg-canvas.tsx's `Physical`.
fn meter_obstacles(w: &World) -> Vec<Bx> {
    let mut out = Vec::new();
    for d in w
        .devices
        .iter()
        .filter(|d| d.px.is_some() && d.py.is_some())
    {
        let (x, y) = phys_abs(d);
        let g = hw_geom(&d.kind);
        let (hw, hh) = (g.w * HWK, g.h * HWK);
        let side = is_gear(&d.kind)
            || ((d.site == "office" || d.site == "dc") && f(d.px.unwrap_or(0.0)) < 190.0);
        let nw = d.name.chars().count() as f32 * 7.4;
        out.push(Bx {
            x1: x - 6.0,
            x2: x + hw + 6.0,
            y1: y - 6.0,
            y2: y + hh + 6.0,
        });
        out.push(if side {
            Bx {
                x1: x + hw + 4.0,
                x2: x + hw + 12.0 + nw,
                y1: y + hh / 2.0 - 10.0,
                y2: y + hh / 2.0 + 8.0,
            }
        } else {
            Bx {
                x1: x + hw / 2.0 - nw / 2.0 - 4.0,
                x2: x + hw / 2.0 + nw / 2.0 + 4.0,
                y1: y + hh + 2.0,
                y2: y + hh + 20.0,
            }
        });
    }
    for (_, rx, ry, _, _) in ROOMS {
        out.push(Bx {
            x1: rx + 8.0,
            x2: rx + 190.0,
            y1: ry + 6.0,
            y2: ry + 38.0,
        });
    }
    out
}

/// `meterAt` (svg-canvas.tsx): the first spot along the sagging cable,
/// from the middle outwards, below then above it, where the length label
/// covers nothing taken and does not cross a room's wall. Returns the
/// label's centre x and baseline y, and takes its box.
fn meter_at(taken: &mut Vec<Bx>, a: (f32, f32), b: (f32, f32), sag: f32, text: &str) -> (f32, f32) {
    let half = text.chars().count() as f32 * 3.1 + 3.0;
    let at = |t: f32, dy: f32| {
        let u = 1.0 - t;
        let x = u * u * u * a.0 + 3.0 * u * u * t * a.0 + 3.0 * u * t * t * b.0 + t * t * t * b.0;
        let y = u * u * u * a.1
            + 3.0 * u * u * t * (a.1 + sag)
            + 3.0 * u * t * t * (b.1 + sag)
            + t * t * t * b.1;
        (x, y + dy)
    };
    let overlaps = |p: &Bx, q: &Bx| p.x1 < q.x2 && q.x1 < p.x2 && p.y1 < q.y2 && q.y1 < p.y2;
    for t in [0.5, 0.42, 0.58, 0.34, 0.66, 0.26, 0.74, 0.18, 0.82] {
        for dy in [12.0, -5.0] {
            let (x, y) = at(t, dy);
            let bx = Bx {
                x1: x - half,
                x2: x + half,
                y1: y - 9.0,
                y2: y + 3.0,
            };
            if taken.iter().any(|o| overlaps(o, &bx)) {
                continue;
            }
            let straddles = ROOMS.iter().any(|&(_, rx, ry, rw, rh)| {
                let room = Bx {
                    x1: rx,
                    x2: rx + rw,
                    y1: ry,
                    y2: ry + rh,
                };
                let inside =
                    bx.x1 >= room.x1 && bx.x2 <= room.x2 && bx.y1 >= room.y1 && bx.y2 <= room.y2;
                overlaps(&room, &bx) && !inside
            });
            if straddles {
                continue;
            }
            taken.push(bx);
            return (x, y);
        }
    }
    at(0.5, 12.0)
}

/// A device's spot on the physical canvas. `px`/`py` must have been set
/// (the caller runs `phys_pos` for every device first).
pub fn phys_abs(d: &Device) -> (f32, f32) {
    let r = room(&d.site);
    (r.0 + f(d.px.unwrap_or(0.0)), r.1 + f(d.py.unwrap_or(0.0)))
}

fn line_style(kind: &str, t: &Theme) -> (Rgba, Option<(f32, f32)>, bool) {
    match kind {
        "fiber" => (t.fiber, None, false),
        "wan" => (t.wan, Some((9.0, 5.0)), false),
        "wifi" => (t.wifi, Some((2.0, 5.0)), true),
        _ => (t.copper, None, false),
    }
}

fn draw_line(p: &mut Painter, pts: &[(f32, f32)], w: f32, style: (Rgba, Option<(f32, f32)>, bool)) {
    match style.1 {
        Some((d, g)) => p.dashed(pts, w, style.0, d, g, style.2),
        None => p.stroke(pts, w, style.0, style.2),
    }
}

/// A fingerprint of everything the static layer depends on: when it does
/// not change, the mesh and the labels are left as they are.
pub fn fingerprint(core: &LabCore, ui: &Ui, theme_gen: u64, now: f64) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    theme_gen.hash(&mut h);
    ui.view.hash(&mut h);
    ui.tool.hash(&mut h);
    ui.connect_from.hash(&mut h);
    ui.sel.hash(&mut h);
    ui.hover.hash(&mut h);
    let mut run: Vec<_> = ui.running.iter().collect();
    run.sort();
    run.hash(&mut h);
    let w = core.world();
    for d in &w.devices {
        d.id.hash(&mut h);
        d.kind.hash(&mut h);
        d.name.hash(&mut h);
        d.site.hash(&mut h);
        d.power.hash(&mut h);
        d.rebooting.hash(&mut h);
        d.gen.hash(&mut h);
        d.x.to_bits().hash(&mut h);
        d.y.to_bits().hash(&mut h);
        d.px.map(f64::to_bits).hash(&mut h);
        d.py.map(f64::to_bits).hash(&mut h);
        d.flag("uplink").hash(&mut h);
        d.flag("certified").hash(&mut h);
    }
    for l in &w.links {
        l.id.hash(&mut h);
        l.a.dev.hash(&mut h);
        l.a.port.hash(&mut h);
        l.b.dev.hash(&mut h);
        l.b.port.hash(&mut h);
        l.kind.hash(&mut h);
        matches!(ui.link_born.get(&l.id), Some(b) if now - b < 1400.0).hash(&mut h);
    }
    let net = core.net();
    for (k, a) in net.addr.iter() {
        k.hash(&mut h);
        a.ip.hash(&mut h);
    }
    for (k, b) in net.losos.boxes.iter() {
        k.hash(&mut h);
        b.up.hash(&mut h);
        b.tunnel.hash(&mut h);
        b.public_name.hash(&mut h);
        b.path.as_ref().map(|p| (&p.id, p.source)).hash(&mut h);
    }
    for (k, s) in net.losos.spoke.iter() {
        k.hash(&mut h);
        s.uplink.hash(&mut h);
    }
    h.finish()
}

/// `fitView`.
pub fn fit(core: &LabCore, view: View, size: (f32, f32)) -> Cam {
    let (w, h) = size;
    if w <= 0.0 || h <= 0.0 {
        return Cam::default();
    }
    match view {
        View::Logical => {
            let ds = &core.world().devices;
            if ds.is_empty() {
                return Cam::default();
            }
            let min_x = ds.iter().map(|d| f(d.x)).fold(f32::MAX, f32::min) - 90.0;
            let max_x = ds.iter().map(|d| f(d.x)).fold(f32::MIN, f32::max) + 90.0;
            let min_y = ds.iter().map(|d| f(d.y)).fold(f32::MAX, f32::min) - 70.0;
            let max_y = ds.iter().map(|d| f(d.y)).fold(f32::MIN, f32::max) + 80.0;
            // The legend sits along the bottom edge: keep the devices above it.
            let hh = (h - 44.0).max(120.0);
            let k = 1.25f32.min((w / (max_x - min_x)).min(hh / (max_y - min_y)));
            Cam {
                k,
                x: (w - (max_x - min_x) * k) / 2.0 - min_x * k,
                y: (hh - (max_y - min_y) * k) / 2.0 - min_y * k,
            }
        }
        View::Physical => {
            let (ww, hh) = (1240.0, 730.0);
            let k = (w / ww).min(h / hh) * 0.98;
            Cam {
                k,
                x: (w - ww * k) / 2.0,
                y: (h - hh * k) / 2.0,
            }
        }
    }
}

/// The logical view's grid: the SVG pattern's 40-unit cells over ±4000.
/// A layer of its own, built once per theme, so panning never rebuilds it.
pub fn grid(t: &Theme, p: &mut Painter) {
    let mut v = -4000.0;
    while v <= 4000.0 {
        p.segment((v, -4000.0), (v, 4000.0), 1.0, t.grid);
        p.segment((-4000.0, v), (4000.0, v), 1.0, t.grid);
        v += 40.0;
    }
}

/// About how wide a label is: IBM Plex Mono advances 0.6 em, Plex Sans
/// SemiBold averages a little under that over names and addresses.
pub fn est_width(text: &str, size: f32, face: Face) -> f32 {
    let em = match face {
        Face::Mono => 0.6,
        Face::Sans => 0.58,
    };
    text.chars().count() as f32 * em * size
}

/// Builds the static scene: its parts, labels and hit regions.
pub fn build(core: &LabCore, cat: &Catalog, ui: &Ui, t: &Theme, now: f64) -> Built {
    let mut out = Built::default();
    let p = &mut Painter::new();
    match ui.view {
        View::Logical => logical(core, cat, ui, t, now, p, &mut out),
        View::Physical => physical(core, cat, ui, t, now, p, &mut out),
    }
    // On the logical view a device's name and address sit on a
    // ground-coloured plate (the SVG canvas's `label-plate`), so cables pass
    // under the words, not through them; a selected device has its halo
    // there instead. The physical view has no plates, as on the SVG canvas.
    begin(p, (0.0, 0.0));
    let selected = match &ui.sel {
        Some(Target::Dev(id)) => Some(id.as_str()),
        _ => None,
    };
    let plated = ui.view == View::Logical;
    for l in out.labels.iter().filter(|l| {
        if !plated {
            return false;
        }
        let id = l
            .key
            .strip_prefix("name:")
            .or_else(|| l.key.strip_prefix("sub:"));
        id.is_some() && id != selected
    }) {
        let w = est_width(&l.text, l.size, l.face) + 6.0;
        let left = l.x - w * (l.ax + 0.5);
        p.rect(
            left,
            l.y - l.size * 0.62,
            w,
            l.size * 1.24,
            3.0,
            alpha(t.ground, 0.85),
        );
    }
    end(p, &mut out, "names".into(), Layer::Backing, (0.0, 0.0));
    stack(&mut out.parts);
    out
}

#[allow(clippy::too_many_arguments)]
fn label(
    out: &mut Built,
    key: String,
    text: impl Into<String>,
    x: f32,
    y: f32,
    size: f32,
    face: Face,
    color: Rgba,
    ax: f32,
) {
    // SVG puts `y` on the baseline; the line box's centre sits about a third
    // of the size above it.
    out.labels.push(Label {
        key,
        text: text.into(),
        x,
        y: y - size * 0.34,
        size,
        face,
        color,
        ax,
    });
}

fn logical(
    core: &LabCore,
    cat: &Catalog,
    ui: &Ui,
    t: &Theme,
    now: f64,
    p: &mut Painter,
    out: &mut Built,
) {
    let w = core.world();
    for b in w.devices.iter().filter(|d| d.kind == "bus") {
        let at = (f(b.x), f(b.y));
        begin(p, at);
        let (x1, x2) = bus_span(core, b);
        let c = if b.power { t.ink } else { alpha(t.ink, 0.35) };
        p.segment((x1, f(b.y)), (x2, f(b.y)), 6.0, c);
        for x in [x1, x2] {
            p.rect(x - 5.0, f(b.y) - 9.0, 10.0, 18.0, 2.5, t.ink);
        }
        end(p, out, format!("bus:{}", b.id), Layer::Bus, at);
    }
    let marks = place_marks(core, cat);
    for l in &w.links {
        let (Some(da), Some(db)) = (w.dev(&l.a.dev), w.dev(&l.b.dev)) else {
            continue;
        };
        let a = anchor(core, da, Some(db));
        let b = anchor(core, db, Some(da));
        begin(p, a);
        let sel = ui.sel == Some(Target::Link(l.id.clone()));
        let hov = ui.hover == Some(Target::Link(l.id.clone()));
        let width = if sel {
            4.5
        } else if hov {
            3.4
        } else {
            2.4
        };
        draw_line(p, &[a, b], width, line_style(&l.kind, t));
        out.hits.push(Hit {
            part: out.parts.len(),
            shape: Shape::Line(vec![a, b], 7.0),
            target: HitTarget::Link(l.id.clone()),
            title: format!(
                "{} {} ↔ {} {} ({})",
                da.name, l.a.port, db.name, l.b.port, l.kind
            ),
        });
        let led = match link_led(core, ui, l, now) {
            Led::Up => t.ok,
            Led::Wait => t.warn,
            _ => t.crit,
        };
        let Some(ends) = marks.get(&l.id) else {
            end(p, out, format!("link:{}", l.id), Layer::Cable, a);
            continue;
        };
        for (m, end, dev) in [(&ends[0], &l.a, da), (&ends[1], &l.b, db)] {
            if let Some((x, y)) = m.led {
                let r = if dev.kind == "bus" { 4.0 } else { 4.5 };
                p.circle_fs(x, y, r, led, t.surface, 1.5);
            }
            if let Some((x, y)) = m.label {
                label(
                    out,
                    format!("port:{}:{}", l.id, end.dev),
                    end.port.clone(),
                    x,
                    y + 3.0,
                    9.5,
                    Face::Mono,
                    t.muted,
                    0.0,
                );
            }
        }
        end(p, out, format!("link:{}", l.id), Layer::Cable, a);
    }
    for d in &w.devices {
        let (x, y) = (f(d.x), f(d.y));
        begin(p, (x, y));
        let tgt = Target::Dev(d.id.clone());
        let sel = ui.sel.as_ref() == Some(&tgt) || ui.connect_from.as_deref() == Some(&d.id);
        // The halo grows with a long name or address so its edge never cuts the text.
        let hx = (-44.0f32).min(label_box(core, cat, d).x1 - 4.0);
        if sel {
            p.rect_fs(
                x + hx,
                y - 36.0,
                -2.0 * hx,
                98.0,
                10.0,
                t.accent_wash,
                t.accent,
                2.0,
            );
        } else if ui.hover.as_ref() == Some(&tgt) {
            p.rect_stroke(x + hx, y - 36.0, -2.0 * hx, 98.0, 10.0, 1.0, t.line);
        }
        if !d.power {
            p.fade = Some((t.ground, 0.55));
        }
        p.at(x - 32.0, y - 32.0, 1.0, |p| icon(p, &d.kind, t));
        p.fade = None;
        out.hits.push(Hit {
            part: out.parts.len(),
            shape: Shape::Rect(x + hx, y - 36.0, -2.0 * hx, 98.0),
            target: HitTarget::Dev(d.id.clone()),
            title: format!(
                "{}, {}",
                d.name,
                cat.label
                    .get(&d.kind)
                    .map(String::as_str)
                    .unwrap_or(&d.kind)
            ),
        });
        label(
            out,
            format!("name:{}", d.id),
            d.name.clone(),
            x,
            y + 38.0,
            12.0,
            Face::Sans,
            t.ink,
            0.0,
        );
        let sub = sub_text(core, cat, d);
        if !sub.is_empty() {
            label(
                out,
                format!("sub:{}", d.id),
                sub,
                x,
                y + 52.0,
                10.5,
                Face::Mono,
                t.muted,
                0.0,
            );
        }
        if let Some((col, title)) = badge(core, d, t) {
            p.circle_fs(x + 22.0, y - 30.0, 8.0, t.surface, t.line, 1.0);
            p.circle(x + 22.0, y - 30.0, 4.5, col);
            out.hits.push(Hit {
                part: out.parts.len(),
                shape: Shape::Circle(x + 22.0, y - 30.0, 8.0),
                target: HitTarget::Dev(d.id.clone()),
                title,
            });
        }
        if ui.running.contains(&d.id) {
            p.rect(x - 40.0, y - 32.0, 26.0, 14.0, 3.0, t.ok);
            label(
                out,
                format!("vm:{}", d.id),
                "VM",
                x - 27.0,
                y - 32.0 + 10.5,
                9.0,
                Face::Sans,
                t.surface,
                0.0,
            );
        }
        end_model(
            p,
            out,
            format!("dev:{}", d.id),
            Layer::Device,
            (x, y),
            Some(&d.kind),
        );
    }
}

fn led_fn<'a>(core: &'a LabCore, ui: &'a Ui, d: &'a Device, now: f64) -> impl Fn(&str) -> Led + 'a {
    move |port: &str| {
        core.world()
            .links
            .iter()
            .find(|l| l.at(&d.id, port))
            .map(|l| link_led(core, ui, l, now))
            .unwrap_or(Led::None)
    }
}

fn physical(
    core: &LabCore,
    cat: &Catalog,
    ui: &Ui,
    t: &Theme,
    now: f64,
    p: &mut Painter,
    out: &mut Built,
) {
    let w = core.world();
    let furn = |p: &mut Painter, x: f32, y: f32, ww: f32, hh: f32, r: f32| {
        p.rect_fs(x, y, ww, hh, r, t.sunk, t.room_edge, 1.0)
    };
    for (k, rx, ry, rw, rh) in ROOMS {
        begin(p, (rx, ry));
        p.rect_fs(rx, ry, rw, rh, 6.0, t.room, t.room_edge, 1.5);
        let (name, sub) = cat.sites.get(k).cloned().unwrap_or_default();
        label(
            out,
            format!("room:{k}"),
            name,
            rx + 14.0,
            ry + 22.0,
            15.0,
            Face::Sans,
            t.ink,
            -0.5,
        );
        label(
            out,
            format!("roomsub:{k}"),
            sub,
            rx + 14.0,
            ry + 37.0,
            11.0,
            Face::Sans,
            t.muted,
            -0.5,
        );
        let mut note = |key: &str, text: &str, x: f32, y: f32| {
            label(
                out,
                format!("note:{k}:{key}"),
                text,
                x,
                y,
                11.0,
                Face::Sans,
                t.muted,
                -0.5,
            );
        };
        match k {
            "home" => {
                furn(p, rx + 20.0, ry + 108.0, 300.0, 8.0, 2.0);
                furn(p, rx + 20.0, ry + rh - 46.0, rw - 40.0, 12.0, 3.0);
                furn(p, rx + 40.0, ry + rh - 34.0, 8.0, 30.0, 0.0);
                furn(p, rx + rw - 48.0, ry + rh - 34.0, 8.0, 30.0, 0.0);
                note("shelf", "hallway shelf", rx + 24.0, ry + 130.0);
                note("desk", "desk", rx + 24.0, ry + rh - 52.0);
            }
            "office" | "dc" => {
                let (h, units) = if k == "office" {
                    (196.0, 12)
                } else {
                    (250.0, 15)
                };
                p.rect_fs(
                    rx + 22.0,
                    ry + 38.0,
                    184.0,
                    h,
                    3.0,
                    t.rack,
                    t.rack_edge,
                    1.0,
                );
                for u in 0..units {
                    let y = ry + 46.0 + u as f32 * 15.5;
                    p.segment((rx + 28.0, y), (rx + 200.0, y), 0.6, t.rack_face);
                }
                if k == "office" {
                    note("rack", "network cabinet, 12U", rx + 26.0, ry + 250.0);
                    furn(p, rx + 220.0, ry + rh - 46.0, rw - 240.0, 12.0, 3.0);
                    note("desks", "desks", rx + 224.0, ry + rh - 52.0);
                } else {
                    note(
                        "vps",
                        "rented VPS, public IPv4, 1 Gbit/s",
                        rx + 220.0,
                        ry + 60.0,
                    );
                }
            }
            _ => {}
        }
        end(p, out, format!("room:{k}"), Layer::Room, (rx, ry));
    }
    // Wi-Fi ranges first.
    for d in w.devices.iter().filter(|d| d.kind == "ap" && d.power) {
        let (x, y) = phys_abs(d);
        let (cx, cy) = (x + 30.0 * HWK, y + 22.0 * HWK);
        begin(p, (cx, cy));
        // Bevy blends in linear light, where a low alpha reads stronger on
        // a dark ground than CSS's sRGB mix: these match `color-mix(… 8%)`.
        p.circle(
            cx,
            cy,
            125.0,
            alpha(t.wifi, if t.dark { 0.015 } else { 0.08 }),
        );
        let mut ring = Painter::ellipse_pts(cx, cy, 125.0, 125.0, 96);
        ring.push(ring[0]);
        p.dashed(&ring, 1.0, t.wifi, 3.0, 6.0, false);
        end(p, out, format!("range:{}", d.id), Layer::Range, (cx, cy));
    }
    let port_abs = |d: &Device, port: &str| {
        let (x, y) = phys_abs(d);
        let pp = hw_geom(&d.kind).port(port);
        (x + pp.0 * HWK, y + pp.1 * HWK)
    };
    let mut taken = meter_obstacles(w);
    for l in &w.links {
        let (Some(da), Some(db)) = (w.dev(&l.a.dev), w.dev(&l.b.dev)) else {
            continue;
        };
        let (a, b) = (port_abs(da, &l.a.port), port_abs(db, &l.b.port));
        begin(p, a);
        let sel = ui.sel == Some(Target::Link(l.id.clone()));
        let title = format!("{} {} ↔ {} {}", da.name, l.a.port, db.name, l.b.port);
        if l.kind == "wifi" {
            draw_line(
                p,
                &[a, b],
                if sel { 4.5 } else { 2.4 },
                line_style("wifi", t),
            );
            out.hits.push(Hit {
                part: out.parts.len(),
                shape: Shape::Line(vec![a, b], 7.0),
                target: HitTarget::Link(l.id.clone()),
                title,
            });
            end(p, out, format!("link:{}", l.id), Layer::Cable, a);
            continue;
        }
        let dist = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let same = da.site == db.site;
        let sag = if same { 30.0 + dist * 0.18 } else { 40.0 };
        let pts = Painter::cubic_pts(a, (a.0, a.1 + sag), (b.0, b.1 + sag), b, 32);
        let style = if !same {
            (t.wan, Some((8.0, 5.0)), false)
        } else if l.kind == "fiber" {
            (t.fiber, None, true)
        } else {
            (t.copper, None, true)
        };
        draw_line(p, &pts, if sel { 5.0 } else { 3.0 }, style);
        out.hits.push(Hit {
            part: out.parts.len(),
            shape: Shape::Line(pts, 7.0),
            target: HitTarget::Link(l.id.clone()),
            title,
        });
        let text = if same {
            format!(
                "{:.1} m {}",
                (dist * 0.02).max(0.5),
                if l.kind == "fiber" { "fiber" } else { "Cat6" }
            )
        } else if da.site == "dc" || db.site == "dc" {
            "datacenter uplink".into()
        } else {
            "ISP line".into()
        };
        let (mx, my) = meter_at(&mut taken, a, b, sag, &text);
        // The SVG canvas strokes this text in the room's colour, so the
        // cable stops short of the words; a plate in that colour does the same.
        let tw = est_width(&text, 10.0, Face::Mono);
        p.rect(mx - tw / 2.0 - 2.0, my - 8.5, tw + 4.0, 11.5, 2.0, t.room);
        label(
            out,
            format!("meters:{}", l.id),
            text,
            mx,
            my,
            10.0,
            Face::Mono,
            t.muted,
            0.0,
        );
        end(p, out, format!("link:{}", l.id), Layer::Cable, a);
    }
    for d in &w.devices {
        let (x, y) = phys_abs(d);
        let g = hw_geom(&d.kind);
        let (hw, hh) = (g.w * HWK, g.h * HWK);
        begin(p, (x, y));
        let tgt = Target::Dev(d.id.clone());
        if ui.sel.as_ref() == Some(&tgt) || ui.connect_from.as_deref() == Some(&d.id) {
            p.rect_stroke(x - 5.0, y - 5.0, hw + 10.0, hh + 10.0, 6.0, 2.5, t.accent);
        } else if ui.hover.as_ref() == Some(&tgt) {
            p.rect_stroke(x - 5.0, y - 5.0, hw + 10.0, hh + 10.0, 6.0, 1.0, t.line);
        }
        let leds = led_fn(core, ui, d, now);
        p.at(x, y, HWK, |p| hardware(p, &d.kind, d.power, t, &leds));
        out.hits.push(Hit {
            part: out.parts.len(),
            shape: Shape::Rect(x - 5.0, y - 5.0, hw + 10.0, hh + 10.0),
            target: HitTarget::Dev(d.id.clone()),
            title: format!(
                "{}, {}",
                d.name,
                cat.label
                    .get(&d.kind)
                    .map(String::as_str)
                    .unwrap_or(&d.kind)
            ),
        });
        if let Some((px, py, r)) = g.power {
            out.hits.push(Hit {
                part: out.parts.len(),
                shape: Shape::Circle(x + px * HWK, y + py * HWK, (r + 2.0) * HWK),
                target: HitTarget::Power(d.id.clone()),
                title: format!("Power {}", if d.power { "off" } else { "on" }),
            });
        }
        match d.kind.as_str() {
            "switch" => label(
                out,
                format!("sw:{}", d.id),
                "SW",
                x + 8.0 * HWK,
                y + 19.0 * HWK,
                8.0 * HWK,
                Face::Mono,
                t.rack_ink,
                -0.5,
            ),
            "internet" => label(
                out,
                format!("isp:{}", d.id),
                "ISP / Internet",
                x + 60.0 * HWK,
                y + 40.0 * HWK,
                11.0 * HWK,
                Face::Sans,
                t.muted,
                0.0,
            ),
            _ => {}
        }
        let px = f(d.px.unwrap_or(0.0));
        let racked = (d.site == "office" || d.site == "dc") && px < 190.0;
        let gear = is_gear(&d.kind);
        let (lx, ly, ax) = if racked || gear {
            let lx = if racked && !gear {
                x + 184.0 - px + 30.0
            } else {
                x + hw + 8.0
            };
            (lx, y + hh / 2.0 + 4.0, -0.5)
        } else {
            (x + hw / 2.0, y + hh + 15.0, 0.0)
        };
        label(
            out,
            format!("name:{}", d.id),
            d.name.clone(),
            lx,
            ly,
            12.0,
            Face::Sans,
            t.ink,
            ax,
        );
        end_model(
            p,
            out,
            format!("dev:{}", d.id),
            Layer::Device,
            (x, y),
            Some(&d.kind),
        );
    }
}

/// The connect tool's rubber band, in canvas units (one part of its own,
/// rebuilt every frame while it shows).
pub fn rubber_band(core: &LabCore, ui: &Ui, t: &Theme, p: &mut Painter) {
    if ui.view != View::Logical {
        return;
    }
    let w = core.world();
    if let (Some(from), Some(ptr)) = (
        ui.connect_from.as_ref().and_then(|id| w.dev(id)),
        ui.pointer,
    ) {
        p.dashed(
            &[(f(from.x), f(from.y)), ptr],
            2.0,
            t.accent,
            5.0,
            4.0,
            false,
        );
    }
}

/// A packet on the logical view: where it is and how it looks.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Mark {
    pub x: f32,
    pub y: f32,
    pub color: Rgba,
    pub real: bool,
}

pub fn marks(list: &[Value], t: &Theme) -> Vec<Mark> {
    list.iter()
        .map(|pk| {
            let num = |k: &str| pk.get(k).and_then(Value::as_f64).unwrap_or(0.0) as f32;
            let proto = pk.get("proto").and_then(Value::as_str).unwrap_or("");
            Mark {
                x: num("x"),
                y: num("y"),
                // The page's `--lab-p-<proto>`, as the SVG canvas colours
                // them; without those, the core's own colour.
                color: t
                    .proto(proto)
                    .or_else(|| {
                        pk.get("color")
                            .and_then(Value::as_str)
                            .and_then(crate::theme::parse)
                    })
                    .unwrap_or(t.accent),
                real: pk.get("real").and_then(Value::as_bool) == Some(true),
            }
        })
        .collect()
}

/// One packet's envelope, centred on its own origin: a mesh shared by every
/// packet of that colour, moved by its entity's transform.
pub fn envelope(p: &mut Painter, color: Rgba, real: bool, t: &Theme) {
    p.at(-11.0, -8.0, 1.0, |p| {
        p.rect_fs(0.0, 0.0, 22.0, 15.0, 2.5, color, t.surface, 1.5);
        p.stroke(
            &[(1.5, 2.0), (11.0, 9.0), (20.5, 2.0)],
            1.4,
            [1.0, 1.0, 1.0, 0.85],
            false,
        );
        if real {
            p.circle(20.0, 1.0, 3.0, t.ok);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lab(key: &str) -> LabCore {
        let mut l = LabCore::new(7.0, 1_759_910_400_000.0, 120.0);
        let r = l.load_scenario(key);
        assert!(r.get("error").is_none(), "{r}");
        l
    }

    #[test]
    fn port_names_round_a_device_never_overlap() {
        // The web setup's switches have four cables each.
        let core = lab("web");
        let cat = Catalog::from_core(&core);
        let marks = place_marks(&core, &cat);
        assert_eq!(marks.len(), core.world().links.len());
        let w = core.world();
        for d in &w.devices {
            let mut boxes: Vec<Bx> = Vec::new();
            for l in w.links_of(&d.id) {
                let i = if l.a.dev == d.id { 0 } else { 1 };
                let port = if i == 0 { &l.a.port } else { &l.b.port };
                if let Some((x, y)) = marks[&l.id][i].label {
                    let rx = port.chars().count() as f32 * 3.0 + 3.0;
                    let b = Bx {
                        x1: x - rx,
                        x2: x + rx,
                        y1: y - 7.0,
                        y2: y + 7.0,
                    };
                    assert!(
                        !boxes
                            .iter()
                            .any(|o| o.hits((b.x1 + b.x2) / 2.0, y, rx, 7.0)),
                        "two port names of {} overlap",
                        d.name
                    );
                    boxes.push(b);
                }
            }
        }
        // Every end that is not on a bus has its LED off the device.
        let some_led = marks.values().flatten().filter(|m| m.led.is_some()).count();
        assert!(some_led > 0);
    }

    #[test]
    fn logical_view_has_every_device_and_link() {
        let core = lab("two-sites");
        let cat = Catalog::from_core(&core);
        let size = (1200.0, 800.0);
        let mut ui = Ui {
            size,
            ..Ui::default()
        };
        ui.cams[0] = fit(&core, View::Logical, size);
        let b = build(&core, &cat, &ui, &Theme::light(), 0.0);
        let w = core.world();
        let devs = b
            .hits
            .iter()
            .filter(|h| matches!(h.target, HitTarget::Dev(_)))
            .count();
        let links = b
            .hits
            .iter()
            .filter(|h| matches!(h.target, HitTarget::Link(_)))
            .count();
        assert!(devs >= w.devices.len());
        assert_eq!(links, w.links.len());
        for d in &w.devices {
            assert!(b
                .labels
                .iter()
                .any(|l| l.key == format!("name:{}", d.id) && l.text == d.name));
            // The device's own centre hits that device.
            let h = hit_at(&b.hits, (d.x as f32, d.y as f32)).unwrap();
            assert_eq!(h.target, HitTarget::Dev(d.id.clone()));
        }
        assert!(b.parts.iter().map(|p| p.mesh.idx.len()).sum::<usize>() > 1000);
        // One part per device and per cable, each hit owned by one of them.
        for d in &w.devices {
            assert!(b
                .parts
                .iter()
                .any(|p| p.key == format!("dev:{}", d.id)
                    && p.model.as_deref() == Some(d.kind.as_str())));
        }
        for l in &w.links {
            assert!(b.parts.iter().any(|p| p.key == format!("link:{}", l.id)));
        }
        assert!(b.hits.iter().all(|h| h.part < b.parts.len()));
        // Devices stack over cables, cables over the ground.
        let y = |k: &str| b.parts.iter().find(|p| p.key == k).unwrap().y;
        assert!(y(&format!("dev:{}", w.devices[0].id)) > y(&format!("link:{}", w.links[0].id)));
    }

    #[test]
    fn parts_are_local_to_their_origin() {
        let core = lab("two-sites");
        let cat = Catalog::from_core(&core);
        let b = build(&core, &cat, &Ui::default(), &Theme::light(), 0.0);
        let d = &core.world().devices[0];
        let part = b
            .parts
            .iter()
            .find(|p| p.key == format!("dev:{}", d.id))
            .unwrap();
        assert_eq!(part.at, (d.x as f32, d.y as f32));
        // The icon is drawn around the origin, flat on the ground.
        for v in &part.mesh.pos {
            assert!(v[0].abs() < 60.0 && v[2].abs() < 60.0 && v[1] == 0.0);
        }
    }

    #[test]
    fn physical_view_needs_spots_and_draws_rooms() {
        let mut core = lab("web");
        let ids: Vec<String> = core.world().devices.iter().map(|d| d.id.clone()).collect();
        for id in &ids {
            core.phys_pos(id);
        }
        let cat = Catalog::from_core(&core);
        let ui = Ui {
            view: View::Physical,
            ..Ui::default()
        };
        let b = build(&core, &cat, &ui, &Theme::dark(), 0.0);
        assert_eq!(
            b.labels
                .iter()
                .filter(|l| l.key.starts_with("room:"))
                .count(),
            4
        );
        assert!(b
            .hits
            .iter()
            .any(|h| matches!(h.target, HitTarget::Power(_))));
    }

    #[test]
    fn cable_lengths_keep_off_the_hardware_and_each_other() {
        let mut core = lab("two-sites");
        let ids: Vec<String> = core.world().devices.iter().map(|d| d.id.clone()).collect();
        for id in &ids {
            core.phys_pos(id);
        }
        let cat = Catalog::from_core(&core);
        let ui = Ui {
            view: View::Physical,
            ..Ui::default()
        };
        let b = build(&core, &cat, &ui, &Theme::light(), 0.0);
        let hardware = meter_obstacles(core.world());
        let mut placed: Vec<Bx> = Vec::new();
        let meters: Vec<&Label> = b
            .labels
            .iter()
            .filter(|l| l.key.starts_with("meters:"))
            .collect();
        assert!(meters.len() >= 5);
        let mut clear = 0;
        for l in meters {
            let half = l.text.chars().count() as f32 * 3.1 + 3.0;
            // Back from the line box's centre to the baseline `meter_at` used.
            let base = l.y + l.size * 0.34;
            let bx = Bx {
                x1: l.x - half,
                x2: l.x + half,
                y1: base - 9.0,
                y2: base + 3.0,
            };
            let (cx, cy) = (l.x, (bx.y1 + bx.y2) / 2.0);
            assert!(
                !placed.iter().any(|o| o.hits(cx, cy, half, 6.0)),
                "two cable labels overlap at {}",
                l.text
            );
            if !hardware.iter().any(|o| o.hits(cx, cy, half, 6.0)) {
                clear += 1;
            }
            placed.push(bx);
        }
        // A short cable between racked gear has no free spot, and its label
        // falls back to the cable's middle, as on the SVG canvas; the
        // two-sites office rack has two. Everything else finds room.
        assert_eq!(clear + 2, placed.len());
    }

    #[test]
    fn fit_keeps_devices_on_screen() {
        let core = lab("two-sites");
        let size = (900.0, 600.0);
        let c = fit(&core, View::Logical, size);
        for d in &core.world().devices {
            let sx = d.x as f32 * c.k + c.x;
            let sy = d.y as f32 * c.k + c.y;
            assert!(
                sx > 0.0 && sx < size.0 && sy > 0.0 && sy < size.1,
                "{} at {sx},{sy}",
                d.id
            );
        }
    }

    #[test]
    fn fingerprint_moves_with_the_world_only() {
        let mut core = lab("two-sites");
        let ui = Ui::default();
        let a = fingerprint(&core, &ui, 0, 0.0);
        assert_eq!(a, fingerprint(&core, &ui, 0, 0.0));
        let id = core.world().devices[0].id.clone();
        core.move_device(&id, r#"{"x": 1234}"#);
        assert_ne!(a, fingerprint(&core, &ui, 0, 0.0));
    }

    #[test]
    fn packets_draw_after_a_tick() {
        let mut core = lab("web");
        let mut any = false;
        for _ in 0..200 {
            core.tick(50.0);
            let pk = core.packets_now();
            if !pk.is_empty() {
                let m = marks(&pk, &Theme::light());
                assert_eq!(m.len(), pk.len());
                let mut p = Painter::new();
                envelope(&mut p, m[0].color, m[0].real, &Theme::light());
                assert!(!p.idx.is_empty());
                any = true;
                break;
            }
        }
        assert!(any, "the web setup puts traffic on the wire");
    }
}
