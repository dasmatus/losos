//! `LabCore`: the whole Lab state and its public operations. Every method
//! answers a JSON value; a refusal is `{"error": "..."}` and changes nothing.

use crate::catalog::{
    self, default_cfg, is_endpoint, is_gear, is_link_kind, is_proto, is_site, port_kind, SPEEDS,
};
use crate::files::{file_label, keep_name, last_label, parse_doc};
use crate::net::{evaluate, Net};
use crate::rng::Rng;
use crate::scenarios::{this_box, ThisBox, BUILTIN};
use crate::sim::{Clock, Done, Fingerprint, Outbox, Pdu, Sim};
use crate::world::{Connected, NewOpts, World};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

pub struct LabCore {
    pub(crate) world: World,
    pub(crate) net: Net,
    pub(crate) sim: Sim,
    pub(crate) clock: Clock,
    pub(crate) prev: Option<HashMap<String, Fingerprint>>,
    pub(crate) rng: Rng,
    pub(crate) scenario: String,
    pub(crate) file_name: String,
    pub(crate) dirty: bool,
    pub(crate) this_box: Option<ThisBox>,
    pub(crate) plate: String,
    pub(crate) outbox: Outbox,
    next_job: u64,
}

fn err(msg: impl Into<String>) -> Value {
    json!({ "error": msg.into() })
}

fn ok() -> Value {
    json!({ "ok": true })
}

fn parse_obj(text: &str) -> Option<Map<String, Value>> {
    if text.trim().is_empty() {
        return Some(Map::new());
    }
    match serde_json::from_str(text) {
        Ok(Value::Object(m)) => Some(m),
        _ => None,
    }
}

impl LabCore {
    /// `seed` feeds the PRNG; `now_ms` starts the lab clock; the zone is a
    /// fixed offset in minutes east of UTC.
    pub fn new(seed: f64, now_ms: f64, tz_offset_min: f64) -> LabCore {
        let seed = if seed.is_finite() { seed.to_bits() } else { 0 };
        let mut lab = LabCore {
            world: World::default(),
            net: Net::default(),
            sim: Sim::default(),
            clock: Clock {
                t: if now_ms.is_finite() { now_ms } else { 0.0 },
                speed: 1.0,
                next_scan: 0.0,
                next_beat: 0.0,
                offset: if tz_offset_min.is_finite() {
                    tz_offset_min.clamp(-1440.0, 1440.0) * 60_000.0
                } else {
                    0.0
                },
            },
            prev: None,
            rng: Rng::new(seed),
            scenario: "two-sites".into(),
            file_name: String::new(),
            dirty: false,
            this_box: None,
            plate: "plate.png".into(),
            outbox: Outbox::default(),
            next_job: 1,
        };
        lab.net = evaluate(&lab.world);
        lab
    }

    pub fn set_plate(&mut self, url: &str) {
        self.plate = url.to_string();
    }

    pub(crate) fn scenario_name(&self, key: &str) -> Option<String> {
        if key == "this-box" {
            return self.this_box.as_ref().map(|t| t.name.clone());
        }
        BUILTIN
            .iter()
            .find(|b| b.key == key)
            .map(|b| b.name.to_string())
    }

    fn scenario_list(&self) -> Vec<Value> {
        let mut v = Vec::new();
        if let Some(t) = &self.this_box {
            v.push(json!({ "key": "this-box", "name": t.name, "blurb": t.blurb }));
        }
        for b in BUILTIN {
            v.push(json!({ "key": b.key, "name": b.name, "blurb": b.blurb }));
        }
        v
    }

    pub fn catalog(&self) -> Value {
        let mut c = catalog::catalog_json();
        c["scenarios"] = Value::Array(self.scenario_list());
        c
    }

    fn changed(&mut self) {
        self.dirty = true;
        self.net = evaluate(&self.world);
        self.traffic_for_change();
    }

    /// `startWorld`: a fresh world from `build`, then its power-on traffic.
    pub(crate) fn start_world(
        &mut self,
        key: &str,
        build: impl FnOnce(&mut Self) -> Option<String>,
    ) -> Option<String> {
        self.world = World::default();
        self.sim_reset();
        let focus = build(self);
        self.scenario = key.to_string();
        self.net = evaluate(&self.world);
        self.reset_prev(false);
        self.traffic_for_change();
        self.reset_prev(true);
        let order = |k: &str| match k {
            "edge-official" => 0,
            "edge-local" => 1,
            "box" => 2,
            "laptop" => 3,
            _ => -1,
        };
        let mut ends: Vec<(String, String, String)> = self
            .world
            .devices
            .iter()
            .filter(|d| is_endpoint(&d.kind))
            .map(|d| (d.id.clone(), d.kind.clone(), d.name.clone()))
            .collect();
        ends.sort_by_key(|e| order(&e.1));
        for (id, _, name) in &ends {
            let st = self.flow_address(id);
            self.start_flow(st, &format!("address {name}"), Done::None);
        }
        for (id, kind, _) in &ends {
            if kind == "edge-local" {
                let st = self.flow_uplink(id);
                self.start_flow(st, "", Done::None);
            }
        }
        for (id, kind, _) in &ends {
            if kind == "box" {
                let st = self.flow_scan(id);
                self.start_flow(st, "", Done::None);
            }
        }
        focus
    }

    pub fn load_scenario(&mut self, key: &str) -> Value {
        let focus = if key == "this-box" {
            let Some(tb) = self.this_box.clone() else {
                return err("This box has not been read yet.");
            };
            self.start_world(key, |lab| lab.build_this_box(&tb))
        } else if BUILTIN.iter().any(|b| b.key == key) {
            self.start_world(key, |lab| lab.build_builtin(key))
        } else {
            return err("There is no such setup.");
        };
        json!({ "focus": focus })
    }

    pub fn load_this_box(&mut self, settings: &str, edge: &str) -> Value {
        let settings: Value = match serde_json::from_str(settings) {
            Ok(v @ Value::Object(_)) => v,
            _ => return err("The box's settings are not readable."),
        };
        let edge: Value = if edge.trim().is_empty() {
            Value::Null
        } else {
            match serde_json::from_str(edge) {
                Ok(v) => v,
                Err(_) => return err("The box's edge report is not readable."),
            }
        };
        let tb = this_box(settings, edge);
        let (name, blurb) = (tb.name.clone(), tb.blurb.clone());
        self.this_box = Some(tb);
        let mut r = self.load_scenario("this-box");
        r["name"] = name.into();
        r["blurb"] = blurb.into();
        r
    }

    // ── setup files ──────────────────────────────────────────────────────
    pub fn export_setup(&self) -> Value {
        let doc = self.setup_doc();
        let text = serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n";
        json!({ "name": doc["name"], "filename": self.setup_filename(&doc), "text": text })
    }

    pub fn import_setup(&mut self, text: &str, file_name: &str) -> Value {
        let doc = match parse_doc(text) {
            Ok(d) => d,
            Err(e) => return err(e),
        };
        match self.open_doc(&doc, "file", &file_label(file_name)) {
            Ok(skipped) => {
                self.dirty = true; // keep it as the last setup too
                json!({ "skipped": skipped, "name": self.file_name })
            }
            Err(e) => err(e),
        }
    }

    pub fn open_last(&mut self, text: &str) -> Value {
        let r = parse_doc(text).and_then(|doc| self.open_doc(&doc, "last", "My setup"));
        match r {
            Ok(skipped) => json!({ "skipped": skipped, "name": self.file_name }),
            Err(e) => {
                self.load_scenario("two-sites");
                json!({ "error": e, "fallback": "two-sites" })
            }
        }
    }

    /// `keepLast`: the document to keep in the browser, when anything changed.
    pub fn last_setup(&mut self) -> Option<String> {
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        let builtin = self.scenario != "file" && self.scenario != "last";
        Some(keep_name(self.setup_doc(), builtin).to_string())
    }

    pub fn peek_setup(&self, text: &str) -> Value {
        match parse_doc(text).ok().as_ref().and_then(last_label) {
            Some(name) => json!({ "name": name }),
            None => err("This is not a LosOS Lab setup file."),
        }
    }

    // ── editing ──────────────────────────────────────────────────────────
    pub fn add_device(&mut self, kind: &str, x: f64, y: f64, opts: &str) -> Value {
        if catalog::type_info(kind).is_none() {
            return err("There is no such device type.");
        }
        if !x.is_finite() || !y.is_finite() {
            return err("A device needs a place on the canvas.");
        }
        let Some(o) = parse_obj(opts) else {
            return err("The device options are not readable.");
        };
        let site = match o.get("site") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) if is_site(s) => Some(s.clone()),
            _ => return err("There is no such location."),
        };
        let coord = |k: &str| match o.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v.as_f64().filter(|f| f.is_finite()).map(Some).ok_or(()),
        };
        let (Ok(px), Ok(py)) = (coord("px"), coord("py")) else {
            return err("A device needs a place on the canvas.");
        };
        let id = self.world.new_device(
            kind,
            x,
            y,
            NewOpts {
                site,
                px,
                py,
                ..NewOpts::default()
            },
            &mut self.rng,
        );
        self.changed();
        json!({ "device": self.world.dev(&id) })
    }

    pub fn connect(
        &mut self,
        a: &str,
        b: &str,
        kind: &str,
        a_port: Option<&str>,
        b_port: Option<&str>,
    ) -> Value {
        if !is_link_kind(kind) {
            return err("There is no such kind of link.");
        }
        for (id, port) in [(a, a_port), (b, b_port)] {
            let Some(p) = port.filter(|p| !p.is_empty()) else {
                continue;
            };
            let Some(d) = self.world.dev(id) else {
                continue;
            };
            if port_kind(&d.kind, p) != Some("eth") {
                return err(format!("{} has no Ethernet port {p}.", d.name));
            }
            if self.world.port_used(id, p) {
                return err(format!("{} {p} is already in use.", d.name));
            }
        }
        let r = self.world.connect(a, b, kind, a_port, b_port);
        self.changed();
        match r {
            Connected::Link(l) => json!({ "link": l }),
            Connected::Error(e) => err(e),
        }
    }

    pub fn remove_device(&mut self, id: &str) -> Value {
        if self.world.dev(id).is_none() {
            return err("There is no such device.");
        }
        self.world.remove_device(id);
        self.changed();
        ok()
    }

    pub fn remove_link(&mut self, id: &str) -> Value {
        if !self.world.links.iter().any(|l| l.id == id) {
            return err("There is no such link.");
        }
        self.world.remove_link(id);
        self.changed();
        ok()
    }

    pub fn set_power(&mut self, id: &str, on: bool) -> Value {
        let Some(d) = self.world.dev_mut(id) else {
            return err("There is no such device.");
        };
        if d.power != on {
            d.power = on;
            self.changed();
        }
        let banner = match self.world.dev(id) {
            Some(d) if on && !matches!(d.kind.as_str(), "laptop" | "bus" | "internet") => {
                self.banner(d)
            }
            _ => vec![],
        };
        json!({ "power": on, "banner": banner })
    }

    pub fn set_cfg(&mut self, id: &str, key: &str, value: &str) -> Value {
        let Some(kind) = self.world.dev(id).map(|d| d.kind.clone()) else {
            return err("There is no such device.");
        };
        let Ok(v) = serde_json::from_str::<Value>(value) else {
            return err("That value is not readable.");
        };
        let v = match (default_cfg(&kind).get(key), v) {
            (Some(Value::Bool(_)), Value::Bool(b)) => Value::Bool(b),
            (Some(Value::String(_)), Value::String(s)) => Value::String(s.trim().to_string()),
            (Some(_), _) => return err(format!("{key} takes a different kind of value.")),
            (None, _) => return err(format!("A {kind} has no setting {key}.")),
        };
        let cfg = match self.world.dev_mut(id) {
            Some(d) => {
                d.cfg.insert(key.to_string(), v);
                d.cfg.clone()
            }
            None => return err("There is no such device."),
        };
        self.changed();
        json!({ "cfg": cfg })
    }

    pub fn set_name(&mut self, id: &str, name: &str) -> Value {
        let v: String = name
            .trim()
            .to_lowercase()
            .chars()
            .map(|c| {
                if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let v = crate::js::slice16(&v, 30);
        if self.world.dev(id).is_none() {
            return err("There is no such device.");
        }
        if v.is_empty() || self.world.devices.iter().any(|x| x.id != id && x.name == v) {
            return err("Pick a host name no other device uses.");
        }
        if let Some(d) = self.world.dev_mut(id) {
            d.name = v.clone();
        }
        self.changed();
        json!({ "name": v })
    }

    pub fn set_site(&mut self, id: &str, site: &str) -> Value {
        if !is_site(site) {
            return err("There is no such location.");
        }
        let Some(d) = self.world.dev_mut(id) else {
            return err("There is no such device.");
        };
        d.site = site.to_string();
        d.px = None;
        // the JS did not mark this as a change, so "Last setup" missed it
        self.dirty = true;
        ok()
    }

    pub fn move_device(&mut self, id: &str, pos: &str) -> Value {
        let Some(o) = parse_obj(pos) else {
            return err("The position is not readable.");
        };
        if self.world.dev(id).is_none() {
            return err("There is no such device.");
        }
        let num = |k: &str| -> Result<Option<Option<f64>>, ()> {
            match o.get(k) {
                None => Ok(None),
                Some(Value::Null) => Ok(Some(None)),
                Some(v) => v
                    .as_f64()
                    .filter(|f| f.is_finite())
                    .map(|f| Some(Some(f)))
                    .ok_or(()),
            }
        };
        let (Ok(x), Ok(y), Ok(px), Ok(py)) = (num("x"), num("y"), num("px"), num("py")) else {
            return err("A device needs a place on the canvas.");
        };
        if matches!(x, Some(None)) || matches!(y, Some(None)) {
            return err("A device needs a place on the canvas.");
        }
        let site = match o.get("site") {
            None => None,
            Some(Value::String(s)) if is_site(s) => Some(s.clone()),
            _ => return err("There is no such location."),
        };
        if let Some(d) = self.world.dev_mut(id) {
            if let Some(Some(x)) = x {
                d.x = x;
            }
            if let Some(Some(y)) = y {
                d.y = y;
            }
            if let Some(p) = px {
                d.px = p;
            }
            if let Some(p) = py {
                d.py = p;
            }
            if let Some(s) = site {
                d.site = s;
            }
        }
        self.dirty = true;
        ok()
    }

    /// `physPos`: places a device that has no spot in its room yet.
    pub fn phys_pos(&mut self, id: &str) -> Value {
        let Some(d) = self.world.dev(id).cloned() else {
            return err("There is no such device.");
        };
        if let (Some(px), Some(py)) = (d.px, d.py) {
            return json!({ "px": crate::js::num_value(px), "py": crate::js::num_value(py) });
        }
        let same: Vec<(f64, f64)> = self
            .world
            .devices
            .iter()
            .filter(|x| x.site == d.site)
            .filter_map(|x| x.px.map(|px| (px, x.py.unwrap_or(0.0))))
            .collect();
        let gear = is_gear(&d.kind);
        let (mut px, mut py) = if gear { (40.0, 50.0) } else { (60.0, 250.0) };
        for _ in 0..20 {
            if !same
                .iter()
                .any(|(x, y)| (x - px).abs() < 110.0 && (y - py).abs() < 60.0)
            {
                break;
            }
            px += 130.0;
            if px > 470.0 {
                px = 40.0;
                py += if gear { 70.0 } else { -80.0 };
            }
        }
        if let Some(d) = self.world.dev_mut(id) {
            d.px = Some(px);
            d.py = Some(py);
        }
        json!({ "px": crate::js::num_value(px), "py": crate::js::num_value(py) })
    }

    // ── reading ──────────────────────────────────────────────────────────
    fn clock_json(&self) -> Value {
        let n = &catalog::DAILY[self.next_timer()];
        json!({
            "t": crate::js::num_value(self.clock.t),
            "speed": crate::js::num_value(self.clock.speed),
            "nextScan": crate::js::num_value(self.clock.next_scan),
            "nextBeat": crate::js::num_value(self.clock.next_beat),
            "text": self.clock_text(),
            "next": { "at": n.at, "key": n.key, "title": n.title },
        })
    }

    pub fn snapshot(&self) -> Value {
        json!({
            "world": self.world,
            "net": self.net,
            "sim": self.sim.to_json(),
            "clock": self.clock_json(),
            "scenario": self.scenario,
            "fileName": self.file_name,
            "dirty": self.dirty,
        })
    }

    pub fn suggest_urls(&self, _id: &str) -> Value {
        let mut out: Vec<String> = Vec::new();
        for b in self.world.devices.iter().filter(|x| x.kind == "box") {
            out.push(format!("http://{}.local/", b.name));
            if let Some(p) = self
                .net
                .losos
                .boxes
                .get(&b.id)
                .and_then(|s| s.public_name.as_ref())
            {
                out.push(format!("https://{p}/nextcloud"));
                out.push(format!("https://{p}/"));
            }
        }
        for e in self.world.devices.iter().filter(|x| x.kind == "edge-local") {
            out.push(format!("http://{}.local:8443/", e.name));
        }
        if let Some(h) = self
            .world
            .devices
            .iter()
            .find(|x| x.kind == "edge-official")
        {
            out.push(format!("https://register.{}/health", h.text("domain")));
        }
        let mut seen = Vec::new();
        for u in out {
            if !seen.contains(&u) {
                seen.push(u);
            }
        }
        seen.truncate(8);
        json!(seen)
    }

    // ── simulator and clock ──────────────────────────────────────────────
    fn drain(&mut self) -> Value {
        let ob = std::mem::take(&mut self.outbox);
        json!({
            "packets": self.packets(),
            "events": ob.events,
            "reset": ob.reset,
            "console": ob.console,
            "http": ob.http,
            "toasts": ob.toasts,
            "worldChanged": ob.world_changed,
            "sim": {
                "mode": self.sim.mode,
                "playing": self.sim.playing,
                "tick": self.sim.tick,
                "frameFrac": self.sim.frame_frac,
                "inFlight": self.sim.flows.len() + self.sim.active.len(),
                "status": self.status_line(),
            },
            "clock": self.clock_json(),
        })
    }

    pub fn tick(&mut self, elapsed_ms: f64) -> Value {
        self.frame(elapsed_ms);
        self.drain()
    }

    pub fn step(&mut self) -> Value {
        self.sim.playing = false;
        if self.sim.flows.is_empty() && self.sim.active.is_empty() {
            let mut r = self.drain();
            r["message"] = "Nothing queued. Power something on, browse from the laptop or ping from a console.".into();
            return r;
        }
        self.step_tick();
        self.sim.anim = Some(0.0);
        self.drain()
    }

    pub fn set_mode(&mut self, mode: &str) -> Value {
        self.sim.mode = match mode {
            "realtime" => "realtime",
            "simulation" => "simulation",
            _ => return err("The mode is realtime or simulation."),
        };
        self.sim.playing = false;
        if self.sim.mode == "realtime" {
            self.ensure_running();
        }
        ok()
    }

    pub fn set_playing(&mut self, on: bool) {
        self.sim.playing = on;
        if on {
            self.ensure_running();
        }
    }

    pub fn set_sim_speed(&mut self, speed: f64) {
        if speed.is_finite() {
            self.sim.speed = speed.clamp(0.25, 4.0);
        }
    }

    pub fn set_clock_speed(&mut self, speed: f64) -> Value {
        if !SPEEDS.contains(&speed) {
            return err("The clock runs at 1×, 10×, 60×, 10 min/s or 1 h/s.");
        }
        self.clock.speed = speed;
        ok()
    }

    pub fn skip_to_next_timer(&mut self) -> Value {
        let n = &catalog::DAILY[self.skip_to_next()];
        json!({ "ok": true, "next": { "at": n.at, "key": n.key, "title": n.title } })
    }

    /// Moves the lab clock directly, as the clock interval does in realtime.
    pub fn advance_clock(&mut self, ms: f64) {
        if ms.is_finite() && ms > 0.0 {
            self.clock_advance(ms);
        }
    }

    pub fn set_filter(&mut self, proto: &str, on: bool) -> Value {
        match self.sim.filters.iter_mut().find(|f| f.0 == proto) {
            Some(f) => {
                f.1 = on;
                ok()
            }
            None => err("There is no such protocol."),
        }
    }

    pub fn clear(&mut self) {
        self.sim_reset();
    }

    pub fn scan(&mut self, id: &str) -> Value {
        if !self.world.dev(id).is_some_and(|d| d.kind == "box") {
            return err("Only a box scans for edges.");
        }
        let st = self.flow_scan(id);
        self.start_flow(st, "scan", Done::None);
        ok()
    }

    /// Frames from a guest (the engine), as `startFlow` stages.
    pub fn start_flow_json(&mut self, stages: &str, title: &str) -> Value {
        let Ok(Value::Array(raw)) = serde_json::from_str::<Value>(stages) else {
            return err("The stages are not readable.");
        };
        let mut out = Vec::new();
        for st in raw {
            let Value::Array(ps) = st else {
                return err("A stage is a list of packets.");
            };
            let mut stage = Vec::new();
            for p in ps {
                let s = |k: &str| p.get(k).and_then(Value::as_str).map(str::to_string);
                let (Some(proto), Some(from), Some(to), Some(info)) =
                    (s("proto"), s("from"), s("to"), s("info"))
                else {
                    return err("A packet has proto, from, to and info.");
                };
                if !is_proto(&proto) {
                    return err(format!("There is no protocol {proto}."));
                }
                if self.world.dev(&from).is_none() || self.world.dev(&to).is_none() {
                    return err("A packet goes between two devices of this setup.");
                }
                let hops = match p.get("hops") {
                    Some(Value::Array(h)) => {
                        let h: Option<Vec<String>> =
                            h.iter().map(|x| x.as_str().map(str::to_string)).collect();
                        match h {
                            Some(h) if h.iter().all(|x| self.world.dev(x).is_some()) => Some(h),
                            _ => return err("Hops name devices of this setup."),
                        }
                    }
                    _ => None,
                };
                stage.push(Pdu {
                    proto,
                    from,
                    to,
                    info,
                    local: p.get("local") == Some(&Value::Bool(true)),
                    hops,
                    real: p.get("real") == Some(&Value::Bool(true)),
                });
            }
            out.push(stage);
        }
        self.start_flow(out, title, Done::None);
        ok()
    }

    // ── consoles and pages ───────────────────────────────────────────────
    pub fn console_info(&self, id: &str) -> Value {
        let Some(d) = self.world.dev(id) else {
            return err("There is no such device.");
        };
        json!({
            "prompt": self.prompt(d),
            "boot": self.boot_text(d),
            "completions": self.completions(d),
        })
    }

    pub fn exec(&mut self, id: &str, line: &str) -> Value {
        if self.world.dev(id).is_none() {
            return err("There is no such device.");
        }
        let job = self.next_job;
        self.next_job += 1;
        let r = self.run_command(id, line, job);
        let mut lines = r.lines;
        let mut busy = r.busy;
        let mut rest = Vec::new();
        for c in std::mem::take(&mut self.outbox.console) {
            if c.job == job {
                lines.extend(c.lines);
                if c.done {
                    busy = false;
                }
            } else {
                rest.push(c);
            }
        }
        self.outbox.console = rest;
        json!({ "job": job, "lines": lines, "busy": busy, "clear": r.clear })
    }

    pub fn http(&mut self, id: &str, url: &str) -> Value {
        if self.world.dev(id).is_none() {
            return err("There is no such device.");
        }
        let job = self.next_job;
        self.next_job += 1;
        self.http_flow(id, url, job, false);
        match self.outbox.http.iter().position(|h| h.job == job) {
            Some(i) => {
                let h = self.outbox.http.remove(i);
                json!({ "job": job, "busy": false, "result": h.result })
            }
            None => json!({ "job": job, "busy": true }),
        }
    }

    pub fn error_page(&self, error: &str, host: &str) -> String {
        crate::pages::error_page(error, host)
    }
}
