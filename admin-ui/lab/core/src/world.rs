//! The world: devices, links and the editing rules (model.js).

use crate::catalog::{self, default_cfg, default_name, default_site, port_kind, ports_of};
use crate::js::{ser_num, ser_opt_num};
use crate::rng::Rng;
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Clone, Debug, Serialize)]
pub struct Device {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    #[serde(serialize_with = "ser_num")]
    pub x: f64,
    #[serde(serialize_with = "ser_num")]
    pub y: f64,
    pub site: String,
    #[serde(serialize_with = "ser_opt_num")]
    pub px: Option<f64>,
    #[serde(serialize_with = "ser_opt_num")]
    pub py: Option<f64>,
    pub power: bool,
    pub cfg: Map<String, Value>,
    pub mac: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rebooting: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gen: Option<u64>,
    #[serde(rename = "computeOpen", skip_serializing_if = "Option::is_none")]
    pub compute_open: Option<bool>,
}

impl Device {
    /// `d.cfg[key]` as JavaScript would test it.
    pub fn flag(&self, key: &str) -> bool {
        crate::js::truthy(self.cfg.get(key))
    }
    /// `d.cfg[key]` as text (`${d.cfg[key]}` for the string keys).
    pub fn text(&self, key: &str) -> String {
        match self.cfg.get(key) {
            Some(Value::String(s)) => s.clone(),
            other => crate::js::string(other),
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct End {
    pub dev: String,
    pub port: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Link {
    pub id: String,
    pub a: End,
    pub b: End,
    pub kind: String,
}

impl Link {
    pub fn touches(&self, id: &str) -> bool {
        self.a.dev == id || self.b.dev == id
    }
    pub fn at(&self, id: &str, port: &str) -> bool {
        (self.a.dev == id && self.a.port == port) || (self.b.dev == id && self.b.port == port)
    }
    pub fn other_end(&self, id: &str) -> &End {
        if self.a.dev == id {
            &self.b
        } else {
            &self.a
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct World {
    pub devices: Vec<Device>,
    pub links: Vec<Link>,
    pub seq: u64,
}

impl Default for World {
    fn default() -> Self {
        World {
            devices: Vec::new(),
            links: Vec::new(),
            seq: 1,
        }
    }
}

#[derive(Default, Clone)]
pub struct NewOpts {
    pub name: Option<String>,
    pub site: Option<String>,
    pub px: Option<f64>,
    pub py: Option<f64>,
    pub power: Option<bool>,
    pub cfg: Vec<(String, Value)>,
}

pub enum Connected {
    Link(Link),
    Error(String),
}

impl World {
    pub fn dev(&self, id: &str) -> Option<&Device> {
        self.devices.iter().find(|d| d.id == id)
    }
    pub fn dev_mut(&mut self, id: &str) -> Option<&mut Device> {
        self.devices.iter_mut().find(|d| d.id == id)
    }
    pub fn name_of(&self, id: &str) -> String {
        self.dev(id)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| id.to_string())
    }
    pub fn is_powered(&self, id: &str) -> bool {
        self.dev(id).is_some_and(|d| d.power)
    }

    /// `newDevice`: the caller has checked `kind` against the catalog.
    pub fn new_device(
        &mut self,
        kind: &str,
        x: f64,
        y: f64,
        opts: NewOpts,
        rng: &mut Rng,
    ) -> String {
        let id = format!("d{}", self.seq);
        self.seq += 1;
        let base = match opts.name {
            Some(n) if !n.is_empty() => n,
            _ => default_name(kind).to_string(),
        };
        let mut name = base.clone();
        let mut n = 2;
        while self.devices.iter().any(|d| d.name == name) {
            name = format!("{base}{n}");
            n += 1;
        }
        let mut cfg = default_cfg(kind);
        for (k, v) in opts.cfg {
            cfg.insert(k, v);
        }
        let mac = format!(
            "52:54:00:4c:{:02x}:{:x}",
            self.seq,
            (rng.random() * 200.0).floor() as u64 + 16
        );
        self.devices.push(Device {
            id: id.clone(),
            kind: kind.to_string(),
            name,
            x,
            y,
            site: opts
                .site
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| default_site(kind).to_string()),
            px: opts.px,
            py: opts.py,
            power: opts.power.unwrap_or(true),
            cfg,
            mac,
            rebooting: None,
            gen: None,
            compute_open: None,
        });
        id
    }

    pub fn links_of(&self, id: &str) -> impl Iterator<Item = &Link> {
        let id = id.to_string();
        self.links.iter().filter(move |l| l.touches(&id))
    }
    pub fn port_used(&self, id: &str, port: &str) -> bool {
        self.links.iter().any(|l| l.at(id, port))
    }

    pub fn free_port(&self, d: &Device, want: &str) -> Option<&'static str> {
        for (p, k) in ports_of(&d.kind) {
            if want == "wifi" {
                if *k == "wifi-ap" {
                    return Some(p);
                }
                if *k == "wifi" && !self.port_used(&d.id, p) {
                    return Some(p);
                }
                continue;
            }
            if *k == "eth" && !self.port_used(&d.id, p) {
                return Some(p);
            }
        }
        None
    }

    /// `connect`, word for word. Explicit ports are trusted here; the
    /// public entry point checks them first.
    pub fn connect(
        &mut self,
        a_id: &str,
        b_id: &str,
        kind: &str,
        a_port: Option<&str>,
        b_port: Option<&str>,
    ) -> Connected {
        let (a, b) = match (self.dev(a_id), self.dev(b_id)) {
            (Some(a), Some(b)) if a.id != b.id => (a.clone(), b.clone()),
            _ => return Connected::Error("Pick two different devices.".into()),
        };
        let has = |d: &Device, k: &str| ports_of(&d.kind).iter().any(|p| p.1 == k);
        let mut wifi = kind == "wifi";
        if kind == "auto" {
            let (aw, bw) = (has(&a, "wifi-ap"), has(&b, "wifi-ap"));
            let (a_cli, b_cli) = (has(&a, "wifi"), has(&b, "wifi"));
            if (aw && b_cli && self.free_port(&b, "eth").is_none())
                || (bw && a_cli && self.free_port(&a, "eth").is_none())
            {
                wifi = true;
            }
            if (aw && b_cli) || (bw && a_cli) {
                wifi = wifi || a.kind == "laptop" || b.kind == "laptop";
            }
        }
        if wifi {
            let ap = if has(&a, "wifi-ap") {
                Some(&a)
            } else if has(&b, "wifi-ap") {
                Some(&b)
            } else {
                None
            };
            let Some(ap) = ap else {
                return Connected::Error(
                    "A wireless link needs an access point at one end.".into(),
                );
            };
            let cli = if ap.id == a.id { &b } else { &a };
            let cp = self.free_port(cli, "wifi");
            let cp = match cp {
                Some(p) if port_kind(&cli.kind, p) == Some("wifi") => p,
                _ => return Connected::Error(format!("{} has no free wireless card.", cli.name)),
            };
            if self
                .links
                .iter()
                .any(|l| l.touches(&cli.id) && l.kind == "wifi")
            {
                return Connected::Error(format!("{} is already on Wi-Fi.", cli.name));
            }
            let l = Link {
                id: format!("l{}", self.seq),
                a: End {
                    dev: ap.id.clone(),
                    port: "wifi".into(),
                },
                b: End {
                    dev: cli.id.clone(),
                    port: cp.into(),
                },
                kind: "wifi".into(),
            };
            self.seq += 1;
            self.links.push(l.clone());
            return Connected::Link(l);
        }
        let ap_ = a_port
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .or_else(|| self.free_port(&a, "eth").map(str::to_string));
        let bp_ = b_port
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .or_else(|| self.free_port(&b, "eth").map(str::to_string));
        let Some(ap_) = ap_ else {
            return Connected::Error(format!("{} has no free Ethernet port.", a.name));
        };
        let Some(bp_) = bp_ else {
            return Connected::Error(format!("{} has no free Ethernet port.", b.name));
        };
        let mut k = if kind == "auto" || kind == "copper" {
            "copper".to_string()
        } else {
            kind.to_string()
        };
        if a.kind == "internet" || b.kind == "internet" {
            k = if kind == "fiber" { "fiber" } else { "wan" }.into();
        }
        let l = Link {
            id: format!("l{}", self.seq),
            a: End {
                dev: a.id,
                port: ap_,
            },
            b: End {
                dev: b.id,
                port: bp_,
            },
            kind: k,
        };
        self.seq += 1;
        self.links.push(l.clone());
        Connected::Link(l)
    }

    pub fn remove_device(&mut self, id: &str) {
        self.links.retain(|l| !l.touches(id));
        self.devices.retain(|d| d.id != id);
    }
    pub fn remove_link(&mut self, id: &str) {
        self.links.retain(|l| l.id != id);
    }

    pub fn link_up(&self, l: &Link) -> bool {
        self.is_powered(&l.a.dev) && self.is_powered(&l.b.dev)
    }

    /// Device-level hop path through network gear only (`hops`).
    pub fn hops(&self, a: &str, b: &str) -> Option<Vec<String>> {
        if a == b {
            return Some(vec![a.to_string()]);
        }
        let mut prev: Vec<(String, Option<String>)> = vec![(a.to_string(), None)];
        let has = |prev: &Vec<(String, Option<String>)>, n: &str| prev.iter().any(|p| p.0 == n);
        let mut q = std::collections::VecDeque::from([a.to_string()]);
        while let Some(cur) = q.pop_front() {
            let links: Vec<&Link> = self.links_of(&cur).collect();
            for l in links {
                if !self.link_up(l) {
                    continue;
                }
                let n = l.other_end(&cur).dev.clone();
                if has(&prev, &n) {
                    continue;
                }
                prev.push((n.clone(), Some(cur.clone())));
                if n == b {
                    let mut path = vec![b.to_string()];
                    let mut c = Some(cur.clone());
                    while let Some(x) = c {
                        c = prev.iter().find(|p| p.0 == x).and_then(|p| p.1.clone());
                        path.insert(0, x);
                    }
                    return Some(path);
                }
                if self.dev(&n).is_some_and(|d| catalog::is_gear(&d.kind)) {
                    q.push_back(n);
                }
            }
        }
        None
    }
}
