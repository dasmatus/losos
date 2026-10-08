//! Packets, flows, the event list, realtime and simulation mode, and the lab
//! clock with LosOS's own timers (sim.js).

use crate::catalog::{is_endpoint, DAILY, PROTO};
use crate::js::{is_dotted_quad, to_fixed};
use crate::lab::LabCore;
use crate::net::Resolved;
use crate::pages::{HttpResult, Page};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;

const DAY: f64 = 864e5;

#[derive(Clone, Debug, Serialize)]
pub struct Pdu {
    pub proto: String,
    pub from: String,
    pub to: String,
    pub info: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub local: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hops: Option<Vec<String>>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub real: bool,
}

pub fn pdu(proto: &str, from: &str, to: &str, info: impl Into<String>) -> Pdu {
    Pdu {
        proto: proto.into(),
        from: from.into(),
        to: to.into(),
        info: info.into(),
        local: false,
        hops: None,
        real: false,
    }
}

pub fn local(proto: &str, at: &str, info: impl Into<String>) -> Pdu {
    Pdu {
        local: true,
        ..pdu(proto, at, at, info)
    }
}

pub type Stage = Vec<Pdu>;

/// What runs when a flow's last stage has been delivered (the JS `done`).
#[derive(Clone, Debug)]
pub enum Done {
    None,
    Reboot {
        id: String,
        name: String,
        kind: String,
    },
    Ping {
        job: u64,
        dev: String,
        host: String,
        count: u32,
        ip: String,
        lan: bool,
    },
    Say {
        job: u64,
        dev: String,
        line: String,
    },
    Http {
        job: u64,
        dev: String,
        url: String,
        curl: bool,
        result: HttpResult,
    },
    Avahi {
        job: u64,
        dev: String,
        edges: Vec<String>,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct Flow {
    pub id: u64,
    pub stages: Vec<Stage>,
    pub stage: i64,
    pub title: String,
    #[serde(skip)]
    pub done: Done,
}

#[derive(Clone, Debug, Serialize)]
pub struct Active {
    pub flow: u64,
    pub proto: String,
    pub info: String,
    pub hops: Vec<String>,
    pub i: usize,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub real: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Event {
    pub proto: String,
    pub last: String,
    pub at: String,
    pub info: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub real: Option<bool>,
    #[serde(rename = "final", skip_serializing_if = "Option::is_none")]
    pub final_: Option<bool>,
    pub t: String,
}

#[derive(Clone, Debug)]
pub struct Sim {
    pub mode: &'static str,
    pub playing: bool,
    pub tick: u64,
    pub speed: f64,
    pub log: Vec<Event>,
    pub flows: Vec<Flow>,
    pub active: Vec<Active>,
    pub filters: Vec<(&'static str, bool)>,
    pub frame_frac: f64,
    pub seq: u64,
    // the rAF loop
    pub running: bool,
    pub since_step: f64,
    pub anim: Option<f64>,
}

impl Default for Sim {
    fn default() -> Self {
        Sim {
            mode: "realtime",
            playing: false,
            tick: 0,
            speed: 1.0,
            log: Vec::new(),
            flows: Vec::new(),
            active: Vec::new(),
            filters: PROTO.iter().map(|p| (p.0, true)).collect(),
            frame_frac: 0.0,
            seq: 0,
            running: false,
            since_step: 0.0,
            anim: None,
        }
    }
}

impl Sim {
    pub fn filter(&self, proto: &str) -> bool {
        self.filters
            .iter()
            .find(|f| f.0 == proto)
            .is_some_and(|f| f.1)
    }
    pub fn to_json(&self) -> Value {
        let filters: serde_json::Map<String, Value> = self
            .filters
            .iter()
            .map(|(k, v)| ((*k).to_string(), Value::Bool(*v)))
            .collect();
        json!({
            "mode": self.mode,
            "playing": self.playing,
            "tick": self.tick,
            "speed": crate::js::num_value(self.speed),
            "log": self.log,
            "flows": self.flows,
            "active": self.active,
            "filters": filters,
            "frameFrac": self.frame_frac,
            "seq": self.seq,
            "cursor": -1,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Clock {
    pub t: f64,
    pub speed: f64,
    pub next_scan: f64,
    pub next_beat: f64,
    /// The zone's offset from UTC in ms, fixed for the session.
    pub offset: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConsoleOut {
    pub job: u64,
    pub dev: String,
    pub lines: Vec<String>,
    pub done: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct HttpOut {
    pub job: u64,
    pub dev: String,
    pub url: String,
    pub result: HttpResult,
}

/// Everything that happened since the last tick, handed out by `tick`.
#[derive(Clone, Debug, Default)]
pub struct Outbox {
    pub events: Vec<Event>,
    pub reset: bool,
    pub console: Vec<ConsoleOut>,
    pub http: Vec<HttpOut>,
    pub toasts: Vec<String>,
    pub world_changed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Fingerprint {
    ip: String,
    boxes: String,
    spoke: String,
}

fn split_lines(s: &str) -> Vec<String> {
    s.split('\n').map(str::to_string).collect()
}

/// Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

impl LabCore {
    // ── flows ────────────────────────────────────────────────────────────
    pub(crate) fn start_flow(&mut self, stages: Vec<Stage>, title: &str, done: Done) {
        let stages: Vec<Stage> = stages.into_iter().filter(|s| !s.is_empty()).collect();
        self.sim.seq += 1;
        let id = self.sim.seq;
        if stages.is_empty() {
            self.run_done(done);
            return;
        }
        self.sim.flows.push(Flow {
            id,
            stages,
            stage: -1,
            title: title.to_string(),
            done,
        });
        if self.sim.mode == "realtime" {
            self.ensure_running();
        }
    }

    pub(crate) fn ensure_running(&mut self) {
        if !self.sim.running {
            self.sim.running = true;
            self.sim.since_step = 0.0;
        }
    }

    fn spawn_stage(&mut self, flow_id: u64) {
        loop {
            let Some(fi) = self.sim.flows.iter().position(|f| f.id == flow_id) else {
                return;
            };
            self.sim.flows[fi].stage += 1;
            let stage = self.sim.flows[fi].stage as usize;
            if stage >= self.sim.flows[fi].stages.len() {
                let f = self.sim.flows.remove(fi);
                self.run_done(f.done);
                return;
            }
            let st = self.sim.flows[fi].stages[stage].clone();
            for p in st {
                if p.local {
                    self.log_event(Event {
                        proto: p.proto,
                        last: p.from.clone(),
                        at: p.from,
                        info: p.info,
                        local: Some(true),
                        fail: None,
                        real: None,
                        final_: None,
                        t: String::new(),
                    });
                    continue;
                }
                let h = p.hops.clone().or_else(|| self.world.hops(&p.from, &p.to));
                match h {
                    Some(h) if h.len() >= 2 => self.sim.active.push(Active {
                        flow: flow_id,
                        proto: p.proto,
                        info: p.info,
                        hops: h,
                        i: 0,
                        real: p.real,
                    }),
                    _ => self.log_event(Event {
                        proto: p.proto,
                        last: p.from.clone(),
                        at: p.from,
                        info: format!("{} · no route", p.info),
                        local: None,
                        fail: Some(true),
                        real: None,
                        final_: None,
                        t: String::new(),
                    }),
                }
            }
            if self.sim.active.iter().any(|a| a.flow == flow_id) {
                return;
            }
        }
    }

    pub(crate) fn step_tick(&mut self) {
        self.sim.tick += 1;
        if self.sim.mode == "simulation" {
            self.clock_advance(250.0);
        }
        self.sim.active.retain(|a| a.i < a.hops.len() - 1);
        let ids: Vec<u64> = self.sim.flows.iter().map(|f| f.id).collect();
        for id in ids {
            if !self.sim.active.iter().any(|a| a.flow == id) {
                self.spawn_stage(id);
            }
        }
        let mut events = Vec::new();
        for a in self.sim.active.iter_mut() {
            if a.i < a.hops.len() - 1 {
                a.i += 1;
                events.push(Event {
                    proto: a.proto.clone(),
                    last: a.hops[a.i - 1].clone(),
                    at: a.hops[a.i].clone(),
                    info: a.info.clone(),
                    local: None,
                    fail: None,
                    real: a.real.then_some(true),
                    final_: Some(a.i == a.hops.len() - 1),
                    t: String::new(),
                });
            }
        }
        for e in events {
            self.log_event(e);
        }
    }

    fn log_event(&mut self, mut e: Event) {
        e.t = to_fixed(self.sim.tick as f64 * 0.25, 2);
        self.sim.log.push(e.clone());
        if self.sim.log.len() > 600 {
            let extra = self.sim.log.len() - 600;
            self.sim.log.drain(0..extra);
        }
        self.outbox.events.push(e);
        if self.outbox.events.len() > 600 {
            let extra = self.outbox.events.len() - 600;
            self.outbox.events.drain(0..extra);
        }
    }

    pub(crate) fn sim_reset(&mut self) {
        self.sim.flows.clear();
        self.sim.active.clear();
        self.sim.log.clear();
        self.sim.tick = 0;
        self.sim.playing = false;
        self.outbox.events.clear();
        self.outbox.reset = true;
    }

    fn console_out(&mut self, job: u64, dev: &str, lines: Vec<String>) {
        self.outbox.console.push(ConsoleOut {
            job,
            dev: dev.to_string(),
            lines,
            done: true,
        });
    }

    fn run_done(&mut self, done: Done) {
        match done {
            Done::None => {}
            Done::Reboot { id, name, kind } => {
                let name = match self.world.dev_mut(&id) {
                    Some(d) => {
                        d.rebooting = Some(false);
                        d.name.clone()
                    }
                    None => name,
                };
                self.outbox.world_changed = true;
                let st = self.flow_address(&id);
                self.start_flow(st, &format!("address {name}"), Done::None);
                if kind == "box" {
                    let st = self.flow_scan(&id);
                    self.start_flow(st, &format!("edge scan {name}"), Done::None);
                }
                if kind == "edge-local" {
                    let st = self.flow_uplink(&id);
                    self.start_flow(st, &format!("uplink {name}"), Done::None);
                }
            }
            Done::Ping {
                job,
                dev,
                host,
                count,
                ip,
                lan,
            } => {
                let mut lines = Vec::new();
                for i in 1..=count {
                    let r = self.rng.random();
                    let ms = if lan { 0.4 + r } else { 14.0 + r * 6.0 };
                    lines.push(format!(
                        "64 bytes from {ip}: icmp_seq={i} ttl={} time={} ms",
                        if lan { 64 } else { 52 },
                        to_fixed(ms, 1)
                    ));
                }
                lines.push(format!("--- {host} ping statistics ---"));
                lines.push(format!(
                    "{count} packets transmitted, {count} received, 0% packet loss"
                ));
                self.console_out(job, &dev, lines);
            }
            Done::Say { job, dev, line } => self.console_out(job, &dev, split_lines(&line)),
            Done::Http {
                job,
                dev,
                url,
                curl,
                result,
            } => {
                if curl {
                    let line = match &result {
                        HttpResult::Error { error, host } => {
                            let host = host.clone().unwrap_or_else(|| "undefined".into());
                            match *error {
                                "mdns" | "nodns" | "nxdomain" => {
                                    format!("curl: (6) Could not resolve host: {host}")
                                }
                                "timeout" => "curl: (28) Connection timed out".into(),
                                "refused" => "curl: (7) Connection refused".into(),
                                _ => "curl: error".into(),
                            }
                        }
                        HttpResult::Page(p) => match &p.text {
                            Some(t) => t.clone(),
                            None => format!("{} {}", p.status, p.title),
                        },
                    };
                    self.console_out(job, &dev, split_lines(&line));
                } else {
                    self.outbox.http.push(HttpOut {
                        job,
                        dev,
                        url,
                        result,
                    });
                }
            }
            Done::Avahi { job, dev, edges } => {
                let mut lines = Vec::new();
                let live: Vec<_> = edges.iter().filter_map(|e| self.world.dev(e)).collect();
                if edges.is_empty() {
                    lines.push("(no _losos-edge._tcp services on this network)".to_string());
                }
                let port = self
                    .net
                    .iface
                    .get(&dev)
                    .map(|f| f.port.clone())
                    .unwrap_or_else(|| "eth0".into());
                for e in live {
                    let ip = self.net.ip(&e.id).unwrap_or("undefined");
                    let s = format!(
                        "= {port} IPv4 {n}  _losos-edge._tcp  local\n   hostname = [{n}.local]\n   address = [{ip}]\n   port = [8443]\n   txt = [\"url=http://{n}.local:8443\" \"rathole={n}.local:2333\" \"enrol={}\"]",
                        if e.flag("openEnrolment") { "open" } else { "closed" },
                        n = e.name
                    );
                    lines.extend(split_lines(&s));
                }
                self.console_out(job, &dev, lines);
            }
        }
    }

    // ── flows LosOS behaviour produces ───────────────────────────────────
    pub(crate) fn nm(&self, id: &str) -> String {
        self.world.name_of(id)
    }

    pub(crate) fn bcast_to(&self, from: &str, info: &str, proto: &str) -> Stage {
        let Some(seg) = self.net.seg_of(from) else {
            return vec![];
        };
        let Some(s) = self.net.segs.get(seg) else {
            return vec![];
        };
        let mut targets: Vec<&String> = Vec::new();
        for m in s
            .members
            .iter()
            .filter(|m| *m != from)
            .chain(s.router.iter())
        {
            if !targets.contains(&m) {
                targets.push(m);
            }
        }
        targets
            .into_iter()
            .map(|t| pdu(proto, from, t, info))
            .collect()
    }

    pub(crate) fn flow_address(&self, id: &str) -> Vec<Stage> {
        let (Some(a), Some(d)) = (self.net.addr.get(id), self.world.dev(id)) else {
            return vec![];
        };
        let mut st = Vec::new();
        if a.src == "dhcp" {
            let r = a.router.clone().unwrap_or_default();
            let gw = a.gw.clone().unwrap_or_default();
            st.push(self.bcast_to(id, &format!("DHCPDISCOVER from {}", d.mac), "DHCP"));
            st.push(vec![pdu(
                "DHCP",
                &r,
                id,
                format!("DHCPOFFER {}/24, router {gw}, dns {gw}", a.ip),
            )]);
            st.push(vec![pdu("DHCP", id, &r, format!("DHCPREQUEST {}", a.ip))]);
            st.push(vec![pdu(
                "DHCP",
                &r,
                id,
                format!("DHCPACK {} lease 86400 s", a.ip),
            )]);
        } else if a.src == "link-local" {
            st.push(self.bcast_to(
                id,
                &format!("ARP probe for {} (no DHCP server: IPv4 link-local)", a.ip),
                "ARP",
            ));
        }
        if is_endpoint(&d.kind) && a.src != "public" {
            st.push(self.bcast_to(id, &format!("announce {}.local A {}", d.name, a.ip), "mDNS"));
        }
        st
    }

    pub(crate) fn dns_leg(&self, from: &str, name: &str) -> Vec<Stage> {
        match self.net.addr.get(from) {
            Some(a) if a.gw.as_deref().is_some_and(|g| !g.is_empty()) => {
                let r = a.router.clone().unwrap_or_else(|| "undefined".into());
                vec![
                    vec![pdu("DNS", from, &r, format!("query A {name}"))],
                    vec![pdu("DNS", &r, from, format!("answer {name}"))],
                ]
            }
            _ => vec![vec![local(
                "DNS",
                from,
                format!("resolve {name} (static resolver)"),
            )]],
        }
    }

    pub(crate) fn flow_scan(&mut self, bid: &str) -> Vec<Stage> {
        let (Some(b), Some(st)) = (
            self.world.dev(bid).cloned(),
            self.net.losos.boxes.get(bid).cloned(),
        ) else {
            return vec![];
        };
        if !st.up {
            return vec![];
        }
        let mut stages = Vec::new();
        let lan: Vec<_> = self
            .world
            .devices
            .iter()
            .filter(|e| {
                e.kind == "edge-local"
                    && e.power
                    && e.flag("advertise")
                    && self.net.same_seg(bid, &e.id)
            })
            .cloned()
            .collect();
        stages.push(self.bcast_to(bid, "query PTR _losos-edge._tcp.local", "mDNS"));
        if !lan.is_empty() {
            stages.push(
                lan.iter()
                    .map(|e| {
                        pdu(
                            "mDNS",
                            &e.id,
                            bid,
                            format!(
                                "answer {n}._losos-edge._tcp url=http://{n}.local:8443 rathole={n}.local:2333 enrol={}",
                                if e.flag("openEnrolment") { "open" } else { "closed" },
                                n = e.name
                            ),
                        )
                    })
                    .collect(),
            );
            stages.push(
                lan.iter()
                    .map(|e| pdu("HTTP", bid, &e.id, "GET /health"))
                    .collect(),
            );
            stages.push(
                lan.iter()
                    .map(|e| pdu("HTTP", &e.id, bid, "200 {\"status\":\"ok\"}"))
                    .collect(),
            );
        }
        if b.flag("proxy") && self.net.has_internet(bid) {
            if let Some(hub) = self.net.hub(&self.world).cloned() {
                let domain = hub.text("domain");
                stages.extend(self.dns_leg(bid, &format!("register.{domain}")));
                stages.push(vec![pdu(
                    "HTTP",
                    bid,
                    &hub.id,
                    format!("GET https://register.{domain}/health"),
                )]);
                stages.push(vec![pdu("HTTP", &hub.id, bid, "200 {\"status\":\"ok\"}")]);
                let nonce = format!("{:08x}", self.rng.next_u64() as u32);
                stages.push(vec![pdu(
                    "HTTP",
                    bid,
                    &hub.id,
                    format!("GET /identity?nonce={nonce}"),
                )]);
                stages.push(vec![pdu(
                    "HTTP",
                    &hub.id,
                    bid,
                    if hub.flag("certified") {
                        "200 cert signed by LosOS root, sig(nonce) ✓ → official"
                    } else {
                        "200 cert not signed by the LosOS root → not official"
                    },
                )]);
            }
        }
        let info = match &st.path {
            Some(p) => format!(
                "path = {} ({}), market {}",
                self.nm(&p.id),
                if p.source == "lan" {
                    "local edge first"
                } else {
                    "official edge second"
                },
                st.market
            ),
            None => {
                "path = none → /run/losos/edge-none, tunnel units stopped, sharing refused".into()
            }
        };
        stages.push(vec![local("lososd", bid, info)]);
        if let Some(p) = st.path.as_ref().filter(|_| b.flag("proxy")) {
            let pn = self.nm(&p.id);
            stages.push(vec![pdu(
                "rathole",
                bid,
                &p.id,
                format!("Noise_NK handshake to {pn}:2333 (pinned key)"),
            )]);
            stages.push(vec![pdu(
                "rathole",
                &p.id,
                bid,
                "handshake ok, control channel open",
            )]);
            stages.push(vec![pdu(
                "HTTP",
                bid,
                &p.id,
                format!("POST /register {{appliance_id:\"{}\"}}", b.name),
            )]);
            let answer = match st.tunnel {
                "refused" => format!("403 {}", st.reason),
                "enrolled" => "200 enrolled (trust on first use)".into(),
                _ => format!("200 route {}", st.public_name.as_deref().unwrap_or("null")),
            };
            stages.push(vec![pdu("HTTP", &p.id, bid, answer)]);
        }
        if b.flag("joinMesh") {
            stages.extend(self.flow_mesh_stages(bid));
        }
        stages
    }

    fn flow_mesh_stages(&self, bid: &str) -> Vec<Stage> {
        let Some(st) = self.net.losos.boxes.get(bid) else {
            return vec![];
        };
        let Some(path) = &st.path else {
            return vec![vec![local(
                "lososd",
                bid,
                "join mesh refused: 409 edgeRequired (no edge in reach)",
            )]];
        };
        let Some(e) = self.world.dev(&path.id) else {
            return vec![];
        };
        if !e.flag("cluster") {
            return vec![vec![local(
                "lososd",
                bid,
                format!("{} runs no rke2 server; mesh stays off", e.name),
            )]];
        }
        vec![
            vec![pdu(
                "rke2",
                bid,
                &e.id,
                format!(
                    "rke2 agent: join https://{}:9345, node password from /etc/rancher",
                    e.name
                ),
            )],
            vec![pdu(
                "rke2",
                &e.id,
                bid,
                "node registered; Longhorn replica scheduled",
            )],
        ]
    }

    pub(crate) fn flow_uplink(&self, sid: &str) -> Vec<Stage> {
        let (Some(s), Some(sp)) = (self.world.dev(sid), self.net.losos.spoke.get(sid)) else {
            return vec![];
        };
        if sp.uplink != "up" {
            return vec![];
        }
        let Some(hub) = sp.hub.as_deref().and_then(|h| self.world.dev(h)) else {
            return vec![];
        };
        let zone = s.text("zone");
        let first = zone.split('.').next().unwrap_or("").to_string();
        let tenants: Vec<String> = sp.relayed.iter().map(|b| self.nm(b)).collect();
        let mut st = self.dns_leg(sid, &format!("register.{}", hub.text("domain")));
        st.push(vec![pdu(
            "HTTP",
            sid,
            &hub.id,
            format!(
                "POST /relay {{appliance_id:\"{first}\", tenants:[{}]}}",
                tenants.join(", ")
            ),
        )]);
        st.push(vec![pdu(
            "HTTP",
            &hub.id,
            sid,
            format!("200 relayed under {zone}"),
        )]);
        st.push(vec![pdu(
            "rathole",
            sid,
            &hub.id,
            "uplink: Noise_NK handshake to edge:2333",
        )]);
        st.push(vec![pdu(
            "rathole",
            &hub.id,
            sid,
            "uplink up: one service per relayed box",
        )]);
        st
    }

    // ── what a change implies ────────────────────────────────────────────
    fn fingerprint(&self) -> HashMap<String, Fingerprint> {
        let mut s = HashMap::new();
        for d in &self.world.devices {
            let ip = self.net.ip(&d.id).unwrap_or("").to_string();
            let boxes = match self.net.losos.boxes.get(&d.id) {
                Some(b) => [
                    b.path.as_ref().map(|p| p.id.clone()).unwrap_or_default(),
                    b.tunnel.to_string(),
                    b.public_name.clone().unwrap_or_default(),
                    b.mesh.to_string(),
                ]
                .join("|"),
                None => String::new(),
            };
            let spoke = match self.net.losos.spoke.get(&d.id) {
                Some(sp) => format!("{}|{}", sp.uplink, sp.relayed.join(",")),
                None => String::new(),
            };
            s.insert(d.id.clone(), Fingerprint { ip, boxes, spoke });
        }
        s
    }

    pub(crate) fn reset_prev(&mut self, now: bool) {
        self.prev = if now { Some(self.fingerprint()) } else { None };
    }

    pub(crate) fn traffic_for_change(&mut self) {
        let now = self.fingerprint();
        let Some(prev) = self.prev.take() else {
            self.prev = Some(now);
            return;
        };
        let devs: Vec<(String, String, String)> = self
            .world
            .devices
            .iter()
            .map(|d| (d.id.clone(), d.kind.clone(), d.name.clone()))
            .collect();
        let mut addr_flows = Vec::new();
        for (id, kind, _) in &devs {
            let (Some(n), o) = (now.get(id), prev.get(id)) else {
                continue;
            };
            if !n.ip.is_empty() && Some(&n.ip) != o.map(|o| &o.ip) && is_endpoint(kind) {
                addr_flows.push(id.clone());
            }
        }
        for id in addr_flows {
            let st = self.flow_address(&id);
            let title = format!("address {}", self.nm(&id));
            self.start_flow(st, &title, Done::None);
        }
        for (id, kind, name) in &devs {
            let (Some(n), o) = (now.get(id), prev.get(id)) else {
                continue;
            };
            if kind == "box" && Some(&n.boxes) != o.map(|o| &o.boxes) && !n.ip.is_empty() {
                let st = self.flow_scan(id);
                self.start_flow(st, &format!("edge scan {name}"), Done::None);
            }
            if kind == "edge-local" && Some(&n.spoke) != o.map(|o| &o.spoke) {
                let st = self.flow_uplink(id);
                self.start_flow(st, &format!("uplink {name}"), Done::None);
            }
        }
        self.prev = Some(now);
    }

    // ── HTTP and ping from a client ──────────────────────────────────────
    pub(crate) fn http_flow(&mut self, from: &str, url: &str, job: u64, curl: bool) {
        let has_scheme = {
            let w = url
                .bytes()
                .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                .count();
            w > 0 && url[w..].starts_with("://")
        };
        let full = if has_scheme {
            url.to_string()
        } else {
            format!("http://{url}")
        };
        let done = |result| Done::Http {
            job,
            dev: from.to_string(),
            url: url.to_string(),
            curl,
            result,
        };
        let Some(u) = crate::url::parse(&full) else {
            let d = done(HttpResult::Error {
                error: "badurl",
                host: None,
            });
            self.run_done(d);
            return;
        };
        let host = u.hostname.clone();
        let path = format!("{}{}", u.pathname, u.search);
        let r = crate::net::resolve_name(&self.world, &self.net, from, &host);
        let mut stages = Vec::new();
        if host.ends_with(".local") {
            stages.push(self.bcast_to(from, &format!("query A {host}"), "mDNS"));
        } else if !is_dotted_quad(&host) && self.net.has_internet(from) {
            stages.extend(self.dns_leg(from, &host));
        }
        let (dev, via, hub, spoke) = match &r {
            Resolved::Error(e) => {
                if host.ends_with(".local") && *e == "mdns" {
                    stages.push(vec![local(
                        "mDNS",
                        from,
                        format!("no answer for {host} on this network"),
                    )]);
                }
                let d = done(HttpResult::Error {
                    error: e,
                    host: Some(host),
                });
                self.start_flow(stages, "", d);
                return;
            }
            Resolved::To {
                dev,
                via,
                hub,
                spoke,
                ..
            } => (dev.clone(), *via, hub.clone(), spoke.clone()),
        };
        if host.ends_with(".local") {
            let ip = self.net.ip(&dev).unwrap_or("undefined").to_string();
            stages.push(vec![pdu(
                "mDNS",
                &dev,
                from,
                format!("answer {host} A {ip}"),
            )]);
        }
        let resp: Page = self.page_for(&r, &u);
        if via == "tunnel" {
            let hub = hub.unwrap_or_else(|| "undefined".into());
            let bx = dev;
            let rev = |h: Option<Vec<String>>| {
                h.map(|mut v| {
                    v.reverse();
                    v
                })
            };
            stages.push(vec![pdu(
                "HTTP",
                from,
                &hub,
                format!(
                    "GET {}//{host}{path} (TLS ends on {})",
                    u.protocol,
                    self.nm(&hub)
                ),
            )]);
            if let Some(spoke) = spoke {
                let zone = self
                    .world
                    .dev(&spoke)
                    .map(|s| s.text("zone"))
                    .unwrap_or_default();
                let first = zone.split('.').next().unwrap_or("").to_string();
                stages.push(vec![Pdu {
                    hops: rev(self.world.hops(&spoke, &hub)),
                    ..pdu(
                        "rathole",
                        &hub,
                        &spoke,
                        format!("uplink service {first}.{}", self.nm(&bx)),
                    )
                }]);
                stages.push(vec![Pdu {
                    hops: rev(self.world.hops(&bx, &spoke)),
                    ..pdu(
                        "rathole",
                        &spoke,
                        &bx,
                        format!("tunnel to {} → nginx :80", self.nm(&bx)),
                    )
                }]);
                stages.push(vec![pdu(
                    "rathole",
                    &bx,
                    &spoke,
                    format!("{} from nginx (client seen as 127.0.0.1)", resp.status),
                )]);
                stages.push(vec![pdu("rathole", &spoke, &hub, resp.status.to_string())]);
            } else {
                stages.push(vec![Pdu {
                    hops: rev(self.world.hops(&bx, &hub)),
                    ..pdu(
                        "rathole",
                        &hub,
                        &bx,
                        format!("tunnel to {} → nginx :80", self.nm(&bx)),
                    )
                }]);
                stages.push(vec![pdu(
                    "rathole",
                    &bx,
                    &hub,
                    format!("{} from nginx (client seen as 127.0.0.1)", resp.status),
                )]);
            }
            stages.push(vec![pdu(
                "HTTP",
                &hub,
                from,
                format!("{} {}", resp.status, resp.title),
            )]);
        } else {
            stages.push(vec![pdu(
                "HTTP",
                from,
                &dev,
                format!("GET {path}  Host: {host}"),
            )]);
            stages.push(vec![pdu(
                "HTTP",
                &dev,
                from,
                format!("{} {}", resp.status, resp.title),
            )]);
        }
        let d = done(HttpResult::Page(resp));
        self.start_flow(stages, &format!("GET {url}"), d);
    }

    /// `pingFlow`: the header goes out at once, the replies when the flow ends.
    pub(crate) fn ping_flow(&mut self, from: &str, host: &str, count: u32, job: u64) {
        let r = crate::net::resolve_name(&self.world, &self.net, from, host);
        let mut stages = Vec::new();
        if host.ends_with(".local") {
            stages.push(self.bcast_to(from, &format!("query A {host}"), "mDNS"));
        } else if !is_dotted_quad(host) && self.net.has_internet(from) {
            stages.extend(self.dns_leg(from, host));
        }
        let (dev, via, hub) = match r {
            Resolved::Error(e) => {
                let line = match e {
                    "mdns" => format!(
                        "ping: {host}: Name or service not known (mDNS: not on this network)"
                    ),
                    "timeout" => {
                        format!("PING {host}: 100% packet loss (no route from this network)")
                    }
                    _ => format!("ping: {host}: Name or service not known"),
                };
                self.start_flow(
                    stages,
                    "",
                    Done::Say {
                        job,
                        dev: from.into(),
                        line,
                    },
                );
                return;
            }
            Resolved::To { dev, via, hub, .. } => (dev, via, hub),
        };
        let tunnel = via == "tunnel";
        let target = if tunnel {
            hub.clone().unwrap_or_else(|| "undefined".into())
        } else {
            dev
        };
        let ip = if tunnel {
            self.net.ip(&target).unwrap_or("undefined").to_string()
        } else {
            self.net
                .ip(&target)
                .or_else(|| self.net.ip(&format!("{target}#lan")))
                .unwrap_or(host)
                .to_string()
        };
        for i in 1..=count {
            stages.push(vec![pdu(
                "ICMP",
                from,
                &target,
                format!("echo request seq={i} to {ip}"),
            )]);
            stages.push(vec![pdu(
                "ICMP",
                &target,
                from,
                format!("echo reply seq={i} from {ip}"),
            )]);
        }
        let first = format!("PING {host} ({ip}) 56(84) bytes of data.");
        // the JS prints the header before it starts the flow
        self.console_out_pending(job, from, vec![first]);
        self.start_flow(
            stages,
            "",
            Done::Ping {
                job,
                dev: from.into(),
                host: host.into(),
                count,
                ip,
                lan: via == "lan",
            },
        );
    }

    fn console_out_pending(&mut self, job: u64, dev: &str, lines: Vec<String>) {
        self.outbox.console.push(ConsoleOut {
            job,
            dev: dev.to_string(),
            lines,
            done: false,
        });
    }

    pub(crate) fn avahi_browse(&mut self, from: &str, job: u64) {
        let es: Vec<(String, String)> = self
            .world
            .devices
            .iter()
            .filter(|e| {
                e.kind == "edge-local"
                    && e.power
                    && e.flag("advertise")
                    && self.net.same_seg(from, &e.id)
            })
            .map(|e| (e.id.clone(), e.name.clone()))
            .collect();
        let stages = vec![
            self.bcast_to(from, "query PTR _losos-edge._tcp.local", "mDNS"),
            es.iter()
                .map(|(id, name)| pdu("mDNS", id, from, format!("answer {name}._losos-edge._tcp")))
                .collect(),
        ];
        self.start_flow(
            stages,
            "",
            Done::Avahi {
                job,
                dev: from.into(),
                edges: es.into_iter().map(|e| e.0).collect(),
            },
        );
    }

    // ── the lab clock ────────────────────────────────────────────────────
    fn local_ms(&self, t: f64) -> f64 {
        t + self.clock.offset
    }

    pub(crate) fn minute_of(&self, t: f64) -> i64 {
        (self.local_ms(t).rem_euclid(DAY) / 60_000.0).floor() as i64
    }

    pub(crate) fn clock_text(&self) -> String {
        let l = self.local_ms(self.clock.t);
        let days = (l / DAY).floor() as i64;
        let (_, m, d) = civil(days);
        let wd =
            ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][(days + 4).rem_euclid(7) as usize];
        let mon = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ][(m as usize).saturating_sub(1).min(11)];
        let secs = (l.rem_euclid(DAY) / 1000.0).floor() as i64;
        format!(
            "{wd} {d} {mon} {:02}:{:02}:{:02}",
            secs / 3600,
            (secs / 60) % 60,
            secs % 60
        )
    }

    /// `HH:MM:SS` of the lab clock, for journal lines.
    pub(crate) fn clock_hms(&self) -> String {
        let secs = (self.local_ms(self.clock.t).rem_euclid(DAY) / 1000.0).floor() as i64;
        format!(
            "{:02}:{:02}:{:02}",
            secs / 3600,
            (secs / 60) % 60,
            secs % 60
        )
    }

    pub(crate) fn next_timer(&self) -> usize {
        let m = self.minute_of(self.clock.t);
        let mut later: Vec<usize> = (0..DAILY.len()).filter(|&i| DAILY[i].minute > m).collect();
        later.sort_by_key(|&i| DAILY[i].minute);
        match later.first() {
            Some(&i) => i,
            None => (0..DAILY.len())
                .min_by_key(|&i| DAILY[i].minute)
                .unwrap_or(0),
        }
    }

    pub(crate) fn clock_advance(&mut self, ms: f64) {
        let from = self.clock.t;
        let to = from + ms;
        self.clock.t = to;
        let off = self.clock.offset;
        let first = ((from + off) / DAY).floor() as i64 - 1;
        let last = ((to + off) / DAY).floor() as i64 + 1;
        for day in first..=last {
            for (i, e) in DAILY.iter().enumerate() {
                let at = day as f64 * DAY + e.minute as f64 * 60_000.0 - off;
                if at > from && at <= to {
                    self.fire_timer(i);
                }
            }
        }
        if to >= self.clock.next_scan {
            if self.clock.next_scan != 0.0 && self.clock.speed <= 60.0 {
                let boxes: Vec<(String, String)> = self
                    .world
                    .devices
                    .iter()
                    .filter(|d| d.kind == "box" && d.power)
                    .map(|d| (d.id.clone(), d.name.clone()))
                    .collect();
                for (id, name) in boxes {
                    let st = self.flow_scan(&id);
                    self.start_flow(st, &format!("rescan {name}"), Done::None);
                }
            }
            self.clock.next_scan = to + 300e3;
        }
        if to >= self.clock.next_beat {
            if self.clock.next_beat != 0.0 && self.clock.speed <= 10.0 {
                let beats: Vec<(String, String)> = self
                    .net
                    .losos
                    .boxes
                    .iter()
                    .filter_map(|(bid, st)| match &st.path {
                        Some(p) if st.tunnel == "registered" || st.tunnel == "enrolled" => {
                            Some((bid.clone(), p.id.clone()))
                        }
                        _ => None,
                    })
                    .collect();
                for (bid, edge) in beats {
                    let title = format!("heartbeat {}", self.nm(&bid));
                    self.start_flow(
                        vec![
                            vec![pdu("HTTP", &bid, &edge, "POST /heartbeat (registrar TTL)")],
                            vec![pdu("HTTP", &edge, &bid, "204")],
                        ],
                        &title,
                        Done::None,
                    );
                }
            }
            self.clock.next_beat = to + 60e3;
        }
    }

    fn fire_timer(&mut self, i: usize) {
        let e = &DAILY[i];
        let los: Vec<(String, String, String)> = self
            .world
            .devices
            .iter()
            .filter(|d| {
                matches!(d.kind.as_str(), "box" | "edge-local" | "edge-official") && d.power
            })
            .map(|d| (d.id.clone(), d.name.clone(), d.kind.clone()))
            .collect();
        match e.key {
            "reboot" => {
                for (id, name, kind) in los {
                    if let Some(d) = self.world.dev_mut(&id) {
                        d.rebooting = Some(true);
                    }
                    self.start_flow(
                        vec![vec![local(
                            "lososd",
                            &id,
                            "00:07 midnight-reboot.timer: systemctl reboot (tmpfs root, /persist kept)",
                        )]],
                        &format!("reboot {name}"),
                        Done::Reboot {
                            id: id.clone(),
                            name: name.clone(),
                            kind,
                        },
                    );
                }
                self.outbox.world_changed = true;
            }
            "upgrade" => {
                let cloud = self
                    .world
                    .devices
                    .iter()
                    .find(|x| x.kind == "internet" && x.power)
                    .map(|x| x.id.clone());
                for (id, name, kind) in los {
                    let gen = match self.world.dev_mut(&id) {
                        Some(d) => {
                            let g = d.gen.filter(|g| *g != 0).unwrap_or(41) + 1;
                            d.gen = Some(g);
                            g
                        }
                        None => 42,
                    };
                    let flake = if kind == "box" {
                        "git+file:///etc/nixos#install"
                    } else {
                        "/etc/nixos#edge"
                    };
                    let mut st = vec![vec![local(
                        "lososd",
                        &id,
                        format!(
                            "03:00 nixos-upgrade: nixos-rebuild switch --impure --flake {flake}"
                        ),
                    )]];
                    if let Some(c) = cloud.as_ref().filter(|_| self.net.has_internet(&id)) {
                        st.push(vec![pdu(
                            "HTTP",
                            &id,
                            c,
                            "GET https://cache.nixos.org/*.narinfo (substitute)",
                        )]);
                        st.push(vec![pdu("HTTP", c, &id, "200 nar.xz")]);
                    }
                    st.push(vec![local(
                        "lososd",
                        &id,
                        format!("switched to generation {gen}; lososd restarted and re-attached to the rebuild"),
                    )]);
                    self.start_flow(st, &format!("upgrade {name}"), Done::None);
                }
                self.outbox.world_changed = true;
            }
            "gc" => {
                for (id, name, _) in los {
                    self.start_flow(
                        vec![vec![local(
                            "lososd",
                            &id,
                            "04:30 nix-collect-garbage --delete-older-than 14d; five boot entries kept",
                        )]],
                        &format!("gc {name}"),
                        Done::None,
                    );
                }
            }
            _ => {
                let open = e.key == "windowStart";
                let boxes: Vec<(String, Option<String>)> = self
                    .net
                    .losos
                    .boxes
                    .iter()
                    .map(|(bid, st)| {
                        (
                            bid.clone(),
                            if st.mesh == "joined" {
                                st.mesh_edge.clone()
                            } else {
                                None
                            },
                        )
                    })
                    .collect();
                for (bid, edge) in boxes {
                    let Some(edge) = edge else { continue };
                    let Some(b) = self.world.dev_mut(&bid) else {
                        continue;
                    };
                    if !b.flag("shareCompute") {
                        continue;
                    }
                    b.compute_open = Some(open);
                    let info = if open {
                        format!(
                            "{} in {}'s zone: remove taint losos/window:NoSchedule, mesh pods may schedule",
                            e.at, b.name
                        )
                    } else {
                        format!(
                            "{}: taint losos/window=closed:NoSchedule, owner's day starts",
                            e.at
                        )
                    };
                    self.outbox.world_changed = true;
                    self.start_flow(
                        vec![vec![pdu("rke2", &edge, &bid, info)]],
                        e.title,
                        Done::None,
                    );
                }
            }
        }
        self.outbox.toasts.push(format!("{} {}", e.at, e.title));
    }

    pub(crate) fn skip_to_next(&mut self) -> usize {
        let n = self.next_timer();
        let l = self.local_ms(self.clock.t);
        let target =
            (l / DAY).floor() * DAY + DAILY[n].minute as f64 * 60_000.0 - self.clock.offset;
        let at = if target > self.clock.t {
            target
        } else {
            target + DAY
        };
        self.clock_advance((at - self.clock.t - 15e3).max(0.0));
        n
    }

    // ── the frame loop ───────────────────────────────────────────────────
    pub(crate) fn frame(&mut self, elapsed: f64) {
        let elapsed = if elapsed.is_finite() && elapsed > 0.0 {
            elapsed
        } else {
            0.0
        };
        if self.sim.running {
            self.sim.since_step += elapsed;
            let period = if self.sim.mode == "realtime" {
                (230.0 / self.clock.speed.sqrt()).max(40.0)
            } else {
                1100.0 / self.sim.speed
            };
            let busy = !self.sim.flows.is_empty() || !self.sim.active.is_empty();
            if self.sim.mode == "simulation" && !self.sim.playing {
                self.sim.frame_frac = 0.0;
                self.sim.running = false;
            } else if !busy {
                self.sim.frame_frac = 0.0;
                self.sim.running = false;
                if self.sim.mode == "simulation" {
                    self.sim.playing = false;
                }
            } else {
                if self.sim.since_step >= period {
                    self.sim.since_step = 0.0;
                    self.step_tick();
                }
                self.sim.frame_frac = (self.sim.since_step / period).min(1.0);
            }
        }
        if let Some(a) = self.sim.anim {
            let a = a + elapsed;
            self.sim.anim = if a >= 450.0 { None } else { Some(a) };
        }
        if self.sim.mode == "realtime" {
            self.clock_advance(elapsed * self.clock.speed);
        }
    }

    /// `drawPdus`: what is on the wire, where, for the protocols shown.
    pub(crate) fn packets(&self) -> Vec<Value> {
        let f = if self.sim.mode == "simulation" && !self.sim.playing {
            match self.sim.anim {
                Some(ms) => (ms / 450.0).min(1.0),
                None => 1.0,
            }
        } else {
            self.sim.frame_frac
        };
        let mut out = Vec::new();
        for a in &self.sim.active {
            if !self.sim.filter(&a.proto) {
                continue;
            }
            let from_id = &a.hops[if a.i == 0 { 0 } else { a.i - 1 }];
            let (Some(fd), Some(td)) = (self.world.dev(from_id), self.world.dev(&a.hops[a.i]))
            else {
                continue;
            };
            let t = if a.i == 0 { 0.0 } else { f };
            let (x, y);
            if td.kind == "bus" || fd.kind == "bus" {
                let (bus, other) = if td.kind == "bus" { (td, fd) } else { (fd, td) };
                let p = self.anchor(bus, Some(other));
                let prev_hop = if a.i >= 2 {
                    self.world.dev(&a.hops[a.i - 2])
                } else {
                    None
                };
                let start = if fd.kind == "bus" {
                    match prev_hop {
                        Some(ph) => self.anchor(bus, Some(ph)),
                        None => p,
                    }
                } else {
                    (fd.x, fd.y)
                };
                let end = if td.kind == "bus" { p } else { (td.x, td.y) };
                if fd.kind == "bus" {
                    if t < 0.5 {
                        x = start.0 + (p.0 - start.0) * t * 2.0;
                        y = bus.y;
                    } else {
                        x = p.0 + (end.0 - p.0) * (t - 0.5) * 2.0;
                        y = p.1 + (end.1 - p.1) * (t - 0.5) * 2.0;
                    }
                } else {
                    x = start.0 + (end.0 - start.0) * t;
                    y = start.1 + (end.1 - start.1) * t;
                }
            } else {
                x = fd.x + (td.x - fd.x) * t;
                y = fd.y + (td.y - fd.y) * t;
            }
            let color = PROTO
                .iter()
                .find(|p| p.0 == a.proto)
                .map(|p| p.1)
                .unwrap_or("#888888");
            let mut o = json!({
                "flow": a.flow, "proto": a.proto, "color": color, "info": a.info,
                "from": fd.id, "to": td.id, "i": a.i, "t": t, "x": x, "y": y,
            });
            if a.real {
                o["real"] = Value::Bool(true);
            }
            out.push(o);
        }
        out
    }

    /// `busSpan` and `anchor` from app.js: where a drop meets the backbone.
    fn anchor(
        &self,
        bus: &crate::world::Device,
        other: Option<&crate::world::Device>,
    ) -> (f64, f64) {
        let Some(other) = other.filter(|_| bus.kind == "bus") else {
            return (bus.x, bus.y);
        };
        let xs: Vec<f64> = self
            .world
            .links_of(&bus.id)
            .filter_map(|l| self.world.dev(&l.other_end(&bus.id).dev))
            .map(|d| d.x)
            .collect();
        let x1 = xs.iter().fold(bus.x - 120.0, |m, &v| m.min(v)) - 30.0;
        let x2 = xs.iter().fold(bus.x + 120.0, |m, &v| m.max(v)) + 30.0;
        ((x1 + 20.0).max((x2 - 20.0).min(other.x)), bus.y)
    }

    pub(crate) fn status_line(&self) -> String {
        format!(
            "t={} s · {} in flight",
            to_fixed(self.sim.tick as f64 * 0.25, 2),
            self.sim.flows.len() + self.sim.active.len()
        )
    }
}
