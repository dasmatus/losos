//! Pointer handling: down, move, up and wheel as the React SVG canvas
//! handles them, plus pinch zoom and the context menu. Pure state over `LabCore` and `Ui`; the
//! canvas feeds it DOM pointer events in CSS pixels and passes the events it
//! returns to the page's callbacks. What lies under the pointer comes from a
//! `Pick` function: in the browser a ray cast from the camera through the
//! pointer against the scene's pick meshes, in the tests the same hit
//! shapes tested in canvas units. Dragging is the one edit made here
//! (`move_device`), because it happens on every pointer move; placing,
//! connecting, deleting and power go to the page as requests, so the page
//! makes those calls itself and keeps its own toasts and dirty flag.

use crate::scene::{phys_abs, port_at, room, room_at, HitTarget, Target, Tool, Ui, View};
use losos_lab_core::LabCore;
use serde_json::{json, Value};
use std::collections::HashMap;

/// An event for the page: a name and its payload.
pub type Event = (&'static str, Value);

/// What is under a canvas point (CSS pixels), and its tooltip.
pub type Pick<'a> = dyn FnMut(f32, f32) -> Option<(HitTarget, String)> + 'a;

#[derive(Clone, Debug)]
enum Drag {
    Pan {
        sx: f32,
        sy: f32,
        ox: f32,
        oy: f32,
    },
    Node {
        id: String,
        sx: f32,
        sy: f32,
        ox: f32,
        oy: f32,
    },
    Pinch {
        d0: f32,
        k0: f32,
    },
}

#[derive(Default)]
pub struct Pointer {
    drag: Option<(Drag, bool)>,
    /// Active pointers by id, for pinch zoom.
    down: HashMap<i32, (f32, f32)>,
}

pub struct Mods {
    pub shift: bool,
}

fn target_json(t: &Option<Target>) -> Value {
    match t {
        Some(Target::Dev(id)) => json!({ "kind": "device", "id": id }),
        Some(Target::Link(id)) => json!({ "kind": "link", "id": id }),
        None => Value::Null,
    }
}

fn hit_target(h: Option<&HitTarget>) -> Option<Target> {
    match h {
        Some(HitTarget::Dev(id)) | Some(HitTarget::Power(id)) => Some(Target::Dev(id.clone())),
        Some(HitTarget::Link(id)) => Some(Target::Link(id.clone())),
        None => None,
    }
}

/// Where a dragged device is now, in the React boundary's `MoveTo` shape:
/// `{id, to: {view: "logical", x, y} | {view: "physical", site, px, py},
/// done}`. The core has the position already; this is for a host that
/// keeps its own copy.
fn move_to(core: &LabCore, ui: &Ui, id: &str, done: bool) -> Value {
    let to = match (core.world().dev(id), ui.view) {
        (Some(d), View::Logical) => json!({ "view": "logical", "x": d.x, "y": d.y }),
        (Some(d), View::Physical) => {
            json!({ "view": "physical", "site": d.site, "px": d.px, "py": d.py })
        }
        (None, _) => Value::Null,
    };
    json!({ "id": id, "to": to, "done": done })
}

/// Ports of `dev` with no cable on them, in the catalogue's order.
pub fn free_ports(core: &LabCore, ports: &[String], dev: &str) -> Vec<String> {
    ports
        .iter()
        .filter(|p| !core.world().port_used(dev, p))
        .cloned()
        .collect()
}

impl Pointer {
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Pointer down at canvas coordinates (sx, sy). `pick` says what is
    /// there; `ports` maps a device type to its port ids.
    #[allow(clippy::too_many_arguments)]
    pub fn down(
        &mut self,
        core: &mut LabCore,
        ui: &mut Ui,
        pick: &mut Pick,
        ports: &HashMap<String, Vec<String>>,
        pid: i32,
        sx: f32,
        sy: f32,
        m: Mods,
    ) -> Vec<Event> {
        self.down.insert(pid, (sx, sy));
        if self.down.len() == 2 {
            let v: Vec<_> = self.down.values().copied().collect();
            let d0 = ((v[0].0 - v[1].0).powi(2) + (v[0].1 - v[1].1).powi(2))
                .sqrt()
                .max(1.0);
            self.drag = Some((Drag::Pinch { d0, k0: ui.cam().k }, true));
            return vec![];
        }
        if self.down.len() > 2 {
            return vec![];
        }
        let w = ui.to_world(sx, sy);
        let hit = pick(sx, sy).map(|h| h.0);
        let mut ev = Vec::new();
        if let Tool::Place(kind) = &ui.tool {
            let mut site = "home".to_string();
            let mut opts = serde_json::Map::new();
            let (mut lx, mut ly) = (w.0.round(), w.1.round());
            if ui.view == View::Physical {
                if let Some(r) = room_at(w.0, w.1) {
                    site = r.to_string();
                }
                let (rx, ry, _, _) = room(&site);
                opts.insert("px".into(), json!((w.0 - rx - 40.0).round()));
                opts.insert("py".into(), json!((w.1 - ry - 20.0).round()));
                lx = 200.0 + core.world().devices.len() as f32 * 60.0;
                ly = 300.0;
            } else if kind == "edge-official" {
                site = "dc".into();
            }
            opts.insert("site".into(), json!(site));
            ev.push((
                "place",
                json!({ "type": kind, "x": lx, "y": ly, "opts": Value::Object(opts), "shift": m.shift }),
            ));
            return ev;
        }
        match hit {
            Some(HitTarget::Power(id)) if ui.tool == Tool::Select => {
                ev.push(("power", json!({ "id": id })));
            }
            Some(HitTarget::Dev(id)) | Some(HitTarget::Power(id)) => match &ui.tool {
                Tool::Delete => ev.push(("delete", json!({ "kind": "device", "id": id }))),
                Tool::Connect(_) if ui.host_connect => {
                    let port = match (ui.view, core.world().dev(&id)) {
                        (View::Physical, Some(d)) => port_at(d, w.0, w.1),
                        _ => None,
                    };
                    ev.push((
                        "connect",
                        json!({ "id": id, "port": port, "shift": m.shift }),
                    ));
                }
                Tool::Connect(kind) => match ui.connect_from.take() {
                    None => ui.connect_from = Some(id),
                    Some(from) if from == id => ui.connect_from = Some(from),
                    Some(from) => {
                        let w = core.world();
                        let pa = w.dev(&from).map(|d| d.kind.clone()).unwrap_or_default();
                        let pb = w.dev(&id).map(|d| d.kind.clone()).unwrap_or_default();
                        let none = Vec::new();
                        ev.push((
                            "request-port-pick",
                            json!({
                                "a": from, "b": id, "kind": kind, "shift": m.shift,
                                "aPorts": free_ports(core, ports.get(&pa).unwrap_or(&none), &from),
                                "bPorts": free_ports(core, ports.get(&pb).unwrap_or(&none), &id),
                            }),
                        ));
                    }
                },
                _ => {
                    let Some(d) = core.world().dev(&id) else {
                        return ev;
                    };
                    let (ox, oy) = match ui.view {
                        View::Logical => (d.x as f32, d.y as f32),
                        View::Physical => (d.px.unwrap_or(0.0) as f32, d.py.unwrap_or(0.0) as f32),
                    };
                    let t = Some(Target::Dev(id.clone()));
                    if ui.sel != t {
                        ui.sel = t;
                        ev.push(("select", target_json(&ui.sel)));
                    }
                    self.drag = Some((Drag::Node { id, sx, sy, ox, oy }, false));
                }
            },
            Some(HitTarget::Link(id)) => {
                if ui.tool == Tool::Delete {
                    ev.push(("delete", json!({ "kind": "link", "id": id })));
                } else {
                    let t = Some(Target::Link(id));
                    if ui.sel != t {
                        ui.sel = t;
                        ev.push(("select", target_json(&ui.sel)));
                    }
                    ev.push(("tap", target_json(&ui.sel)));
                }
            }
            None => {
                let c = ui.cam();
                self.drag = Some((
                    Drag::Pan {
                        sx,
                        sy,
                        ox: c.x,
                        oy: c.y,
                    },
                    false,
                ));
            }
        }
        ev
    }

    pub fn moved(
        &mut self,
        core: &mut LabCore,
        ui: &mut Ui,
        pick: &mut Pick,
        pid: i32,
        sx: f32,
        sy: f32,
    ) -> Vec<Event> {
        let mut ev = Vec::new();
        if let Some(p) = self.down.get_mut(&pid) {
            *p = (sx, sy);
        }
        ui.pointer = Some(ui.to_world(sx, sy));
        match &mut self.drag {
            Some((Drag::Pinch { d0, k0 }, _)) if self.down.len() >= 2 => {
                let v: Vec<_> = self.down.values().copied().collect();
                let d = ((v[0].0 - v[1].0).powi(2) + (v[0].1 - v[1].1).powi(2))
                    .sqrt()
                    .max(1.0);
                let (mx, my) = ((v[0].0 + v[1].0) / 2.0, (v[0].1 + v[1].1) / 2.0);
                let k2 = ui.tune.clamp_zoom(*k0 * d / *d0);
                zoom_to(ui, mx, my, k2);
            }
            Some((
                Drag::Pan {
                    sx: x0,
                    sy: y0,
                    ox,
                    oy,
                },
                moved,
            )) => {
                let (dx, dy) = (sx - *x0, sy - *y0);
                if dx.abs() + dy.abs() > 3.0 {
                    *moved = true;
                }
                let c = ui.cam_mut();
                c.x = *ox + dx;
                c.y = *oy + dy;
            }
            Some((
                Drag::Node {
                    id,
                    sx: x0,
                    sy: y0,
                    ox,
                    oy,
                },
                moved,
            )) => {
                let (dx, dy) = (sx - *x0, sy - *y0);
                if dx.abs() + dy.abs() > 3.0 {
                    *moved = true;
                }
                if *moved {
                    let k = ui.cam().k;
                    let (nx, ny) = ((*ox + dx / k).round(), (*oy + dy / k).round());
                    let pos = match ui.view {
                        View::Logical => json!({ "x": nx, "y": ny }),
                        View::Physical => json!({ "px": nx, "py": ny }),
                    };
                    core.move_device(id, &pos.to_string());
                    ev.push(("move", move_to(core, ui, id, false)));
                }
            }
            _ => {
                let picked = pick(sx, sy);
                let h = hit_target(picked.as_ref().map(|p| &p.0));
                if h != ui.hover {
                    ui.hover = h;
                    let title = picked.map(|p| p.1);
                    let mut v = target_json(&ui.hover);
                    if let (Value::Object(o), Some(t)) = (&mut v, title) {
                        o.insert("title".into(), json!(t));
                    }
                    ev.push(("hover", v));
                }
            }
        }
        ev
    }

    pub fn up(&mut self, core: &mut LabCore, ui: &mut Ui, pid: i32) -> Vec<Event> {
        self.down.remove(&pid);
        let mut ev = Vec::new();
        let Some((drag, moved)) = self.drag.take() else {
            return ev;
        };
        match drag {
            Drag::Pinch { .. } => {
                // The finger left on the glass does not start a pan.
                self.down.clear();
            }
            Drag::Node { id, .. } if moved => {
                let mut site_changed = None;
                if ui.view == View::Physical {
                    if let Some(d) = core.world().dev(&id).cloned() {
                        let (x, y) = phys_abs(&d);
                        let mut site = d.site.clone();
                        let (mut px, mut py) =
                            (d.px.unwrap_or(0.0) as f32, d.py.unwrap_or(0.0) as f32);
                        if let Some(k) = room_at(x + 40.0, y + 20.0) {
                            if k != d.site {
                                let (rx, ry, _, _) = room(k);
                                px = (x - rx).round();
                                py = (y - ry).round();
                                site = k.to_string();
                                site_changed = Some(k);
                            }
                        }
                        let (_, _, rw, rh) = room(&site);
                        px = px.clamp(0.0, rw - 60.0);
                        py = py.clamp(30.0, rh - 40.0);
                        core.move_device(
                            &id,
                            &json!({ "px": px, "py": py, "site": site }).to_string(),
                        );
                    }
                }
                ev.push(("move", move_to(core, ui, &id, true)));
                ev.push((
                    "moved",
                    json!({ "id": id, "view": if ui.view == View::Logical { "logical" } else { "physical" }, "site": site_changed }),
                ));
            }
            Drag::Node { id, .. } => {
                // A click without a drag: the SVG canvas's `tap`, which opens
                // the inspector sheet on a phone.
                ev.push(("tap", target_json(&Some(Target::Dev(id)))));
            }
            Drag::Pan { .. } if !moved && ui.tool == Tool::Select && ui.sel.is_some() => {
                ui.sel = None;
                ev.push(("select", Value::Null));
            }
            _ => {}
        }
        ev
    }

    pub fn leave(&mut self, ui: &mut Ui) -> Vec<Event> {
        ui.pointer = None;
        if ui.hover.take().is_some() {
            return vec![("hover", Value::Null)];
        }
        vec![]
    }

    pub fn context(
        &mut self,
        ui: &Ui,
        pick: &mut Pick,
        sx: f32,
        sy: f32,
        cx: f32,
        cy: f32,
    ) -> Vec<Event> {
        let w = ui.to_world(sx, sy);
        let t = hit_target(pick(sx, sy).map(|p| p.0).as_ref());
        vec![(
            "context",
            json!({ "target": target_json(&t), "clientX": cx, "clientY": cy, "x": w.0, "y": w.1 }),
        )]
    }
}

/// Zooms to scale `k2` keeping canvas point (mx, my) still.
pub fn zoom_to(ui: &mut Ui, mx: f32, my: f32, k2: f32) {
    let c = ui.cam_mut();
    c.x = mx - (mx - c.x) * (k2 / c.k);
    c.y = my - (my - c.y) * (k2 / c.k);
    c.k = k2;
}

/// `onWheel`.
pub fn wheel(ui: &mut Ui, mx: f32, my: f32, delta_y: f32) {
    let k2 = ui
        .tune
        .clamp_zoom(ui.cam().k * (-delta_y * ui.tune.wheel_rate).exp());
    zoom_to(ui, mx, my, k2);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{build, fit, hit_at, Cam, Catalog, Hit};
    use crate::theme::Theme;

    fn setup() -> (LabCore, Ui, Catalog) {
        let mut core = LabCore::new(3.0, 1_759_910_400_000.0, 0.0);
        core.load_scenario("two-sites");
        let size = (1000.0, 700.0);
        let mut ui = Ui {
            size,
            ..Ui::default()
        };
        ui.cams[0] = fit(&core, View::Logical, size);
        let cat = Catalog::from_core(&core);
        (core, ui, cat)
    }

    /// The tests' `Pick`: the hit shapes under the point, with the camera
    /// the scene was built for.
    fn picker(hits: &[Hit], c: Cam) -> impl FnMut(f32, f32) -> Option<(HitTarget, String)> + '_ {
        move |sx, sy| {
            hit_at(hits, ((sx - c.x) / c.k, (sy - c.y) / c.k))
                .map(|h| (h.target.clone(), h.title.clone()))
        }
    }

    fn screen(ui: &Ui, x: f64, y: f64) -> (f32, f32) {
        let c = ui.cam();
        (x as f32 * c.k + c.x, y as f32 * c.k + c.y)
    }

    #[test]
    fn click_selects_and_drag_moves() {
        let (mut core, mut ui, cat) = setup();
        let d = core.world().devices[0].clone();
        let hits = build(&core, &cat, &ui, &Theme::light(), 0.0).hits;
        let mut pick = picker(&hits, ui.cam());
        let (sx, sy) = screen(&ui, d.x, d.y);
        let mut p = Pointer::default();
        let ev = p.down(
            &mut core,
            &mut ui,
            &mut pick,
            &cat.ports,
            1,
            sx,
            sy,
            Mods { shift: false },
        );
        assert_eq!(ev[0].0, "select");
        assert_eq!(ev[0].1["id"], d.id.as_str());
        let ev = p.moved(&mut core, &mut ui, &mut pick, 1, sx + 50.0, sy + 20.0);
        assert_eq!(ev[0].0, "move");
        assert_eq!(ev[0].1["done"], false);
        let ev = p.up(&mut core, &mut ui, 1);
        assert_eq!(ev[0].0, "move");
        assert_eq!(ev[0].1["to"]["view"], "logical");
        assert_eq!(ev[0].1["done"], true);
        assert_eq!(ev[1].0, "moved");
        let moved = core.world().dev(&d.id).unwrap();
        assert!((moved.x - d.x - 50.0 / ui.cam().k as f64).abs() <= 1.0);
    }

    #[test]
    fn connect_asks_for_ports_and_place_reports_where() {
        let (mut core, mut ui, cat) = setup();
        let hits = build(&core, &cat, &ui, &Theme::light(), 0.0).hits;
        let mut pick = picker(&hits, ui.cam());
        let ds = core.world().devices.clone();
        ui.tool = Tool::Connect("copper".into());
        let mut p = Pointer::default();
        let a = screen(&ui, ds[0].x, ds[0].y);
        let b = screen(&ui, ds[1].x, ds[1].y);
        assert!(p
            .down(
                &mut core,
                &mut ui,
                &mut pick,
                &cat.ports,
                1,
                a.0,
                a.1,
                Mods { shift: false }
            )
            .is_empty());
        p.up(&mut core, &mut ui, 1);
        let ev = p.down(
            &mut core,
            &mut ui,
            &mut pick,
            &cat.ports,
            1,
            b.0,
            b.1,
            Mods { shift: true },
        );
        assert_eq!(ev[0].0, "request-port-pick");
        assert_eq!(ev[0].1["a"], ds[0].id.as_str());
        assert!(ev[0].1["aPorts"].is_array());
        assert!(ui.connect_from.is_none());
        p.up(&mut core, &mut ui, 1);

        ui.tool = Tool::Place("laptop".into());
        let ev = p.down(
            &mut core,
            &mut ui,
            &mut pick,
            &cat.ports,
            1,
            5.0,
            5.0,
            Mods { shift: false },
        );
        assert_eq!(ev[0].0, "place");
        assert_eq!(ev[0].1["type"], "laptop");
        assert_eq!(ev[0].1["opts"]["site"], "home");
    }

    #[test]
    fn empty_click_clears_selection_and_wheel_zooms_about_the_pointer() {
        let (mut core, mut ui, cat) = setup();
        let hits = build(&core, &cat, &ui, &Theme::light(), 0.0).hits;
        let mut pick = picker(&hits, ui.cam());
        ui.sel = Some(Target::Dev("d1".into()));
        let mut p = Pointer::default();
        // Far outside every device.
        let s = screen(&ui, -3000.0, -3000.0);
        p.down(
            &mut core,
            &mut ui,
            &mut pick,
            &cat.ports,
            1,
            s.0,
            s.1,
            Mods { shift: false },
        );
        let ev = p.up(&mut core, &mut ui, 1);
        assert_eq!(ev[0].0, "select");
        assert!(ui.sel.is_none());
        let before = ui.to_world(300.0, 200.0);
        wheel(&mut ui, 300.0, 200.0, -200.0);
        let after = ui.to_world(300.0, 200.0);
        assert!((before.0 - after.0).abs() < 1e-3 && (before.1 - after.1).abs() < 1e-3);
    }

    #[test]
    fn a_page_that_owns_the_connection_hears_each_click_and_taps() {
        let (mut core, mut ui, cat) = setup();
        let hits = build(&core, &cat, &ui, &Theme::light(), 0.0).hits;
        let mut pick = picker(&hits, ui.cam());
        let d = core.world().devices[0].clone();
        let (sx, sy) = screen(&ui, d.x, d.y);
        let mut p = Pointer::default();
        // A click without a drag is a tap after the select.
        p.down(
            &mut core,
            &mut ui,
            &mut pick,
            &cat.ports,
            1,
            sx,
            sy,
            Mods { shift: false },
        );
        let ev = p.up(&mut core, &mut ui, 1);
        assert_eq!(ev[0].0, "tap");
        assert_eq!(ev[0].1["id"], d.id.as_str());

        ui.tool = Tool::Connect("copper".into());
        ui.host_connect = true;
        ui.connect_from = Some("elsewhere".into());
        let ev = p.down(
            &mut core,
            &mut ui,
            &mut pick,
            &cat.ports,
            1,
            sx,
            sy,
            Mods { shift: true },
        );
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].0, "connect");
        assert_eq!(ev[0].1["id"], d.id.as_str());
        assert_eq!(ev[0].1["shift"], true);
        assert!(
            ev[0].1["port"].is_null(),
            "the logical view cannot tell a port"
        );
        // The page keeps the first end; the canvas leaves it alone.
        assert_eq!(ui.connect_from.as_deref(), Some("elsewhere"));
    }

    #[test]
    fn a_click_on_a_port_in_the_physical_view_names_it() {
        let (mut core, _, _) = setup();
        let ids: Vec<String> = core.world().devices.iter().map(|d| d.id.clone()).collect();
        for id in &ids {
            core.phys_pos(id);
        }
        let d = core
            .world()
            .devices
            .iter()
            .find(|d| d.kind == "box")
            .unwrap()
            .clone();
        let (x, y) = phys_abs(&d);
        let g = crate::icons::hw_geom("box");
        let (px, py) = g.port("eth0");
        let s = crate::scene::HWK;
        assert_eq!(
            port_at(&d, x + px * s + 2.0, y + py * s - 2.0).as_deref(),
            Some("eth0")
        );
        assert_eq!(port_at(&d, x, y), None);
    }
}
