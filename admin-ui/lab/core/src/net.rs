//! Evaluation: segments, addresses, reachability, and what lososd and the
//! registrars would conclude (model.js `evaluate`, `computeLosos`,
//! `resolveName`). A pure function of the world.

use crate::catalog::{is_endpoint, ports_of};
use crate::js::is_dotted_quad;
use crate::omap::OMap;
use crate::world::World;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize)]
pub struct Seg {
    pub id: String,
    pub members: Vec<String>,
    pub routers: Vec<String>,
    pub internet: bool,
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloud: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub router: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Iface {
    pub port: String,
    pub seg: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Addr {
    pub ip: String,
    pub mask: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gw: Option<String>,
    pub src: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub router: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Mgmt {
    pub ip: String,
    pub mask: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gw: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Edge {
    pub id: String,
    pub source: &'static str,
    pub official: bool,
    pub url: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct BoxSt {
    pub up: bool,
    pub edges: Vec<Edge>,
    pub path: Option<Edge>,
    pub tunnel: &'static str,
    #[serde(rename = "publicName")]
    pub public_name: Option<String>,
    pub mesh: &'static str,
    pub market: &'static str,
    pub reason: &'static str,
    #[serde(rename = "meshEdge", skip_serializing_if = "Option::is_none")]
    pub mesh_edge: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SpokeSt {
    pub up: bool,
    pub uplink: &'static str,
    pub relayed: Vec<String>,
    pub enrolled: Vec<String>,
    pub cluster: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hub: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct HubSt {
    pub up: bool,
    pub tenants: Vec<String>,
    pub relays: Vec<String>,
    pub cluster: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Losos {
    #[serde(rename = "box")]
    pub boxes: OMap<BoxSt>,
    pub spoke: OMap<SpokeSt>,
    pub hub: OMap<HubSt>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Net {
    pub segs: OMap<Seg>,
    pub iface: OMap<Iface>,
    pub addr: OMap<Addr>,
    pub internet: OMap<bool>,
    pub subnet: OMap<String>,
    #[serde(rename = "gearSeg")]
    pub gear_seg: OMap<String>,
    pub mgmt: OMap<Mgmt>,
    pub losos: Losos,
}

impl Net {
    pub fn seg_of(&self, id: &str) -> Option<&str> {
        self.iface.get(id).map(|f| f.seg.as_str())
    }
    pub fn segs_of(&self, id: &str) -> Vec<&str> {
        let mut v = Vec::new();
        if let Some(s) = self.seg_of(id) {
            v.push(s);
        }
        if let Some(s) = self.gear_seg.get(id) {
            v.push(s.as_str());
        }
        v
    }
    pub fn same_seg(&self, a: &str, b: &str) -> bool {
        let bs = self.segs_of(b);
        self.segs_of(a).iter().any(|x| bs.contains(x))
    }
    pub fn ip(&self, id: &str) -> Option<&str> {
        self.addr.get(id).map(|a| a.ip.as_str())
    }
    pub fn has_internet(&self, id: &str) -> bool {
        self.internet.get(id).copied().unwrap_or(false)
    }
    /// The first powered official edge with internet (`NET.hub()`).
    pub fn hub<'w>(&self, world: &'w World) -> Option<&'w crate::world::Device> {
        world
            .devices
            .iter()
            .find(|d| d.kind == "edge-official" && d.power && self.has_internet(&d.id))
    }
}

struct Uf {
    parent: HashMap<String, String>,
}

impl Uf {
    fn find(&mut self, k: &str) -> String {
        let mut k = k.to_string();
        loop {
            let p = match self.parent.get(&k) {
                Some(p) => p.clone(),
                None => {
                    self.parent.insert(k.clone(), k.clone());
                    return k;
                }
            };
            if p == k {
                return k;
            }
            let gp = self.parent.get(&p).cloned().unwrap_or_else(|| p.clone());
            self.parent.insert(k.clone(), gp.clone());
            k = gp;
        }
    }
    fn union(&mut self, a: &str, b: &str) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra != rb {
            self.parent.insert(ra, rb);
        }
    }
}

fn key(d: &str, p: &str) -> String {
    format!("{d}:{p}")
}

fn new_seg(id: &str) -> Seg {
    Seg {
        id: id.to_string(),
        members: Vec::new(),
        routers: Vec::new(),
        internet: false,
        kind: "isolated",
        cloud: None,
        router: None,
    }
}

fn get_seg<'a>(segs: &'a mut OMap<Seg>, k: &str) -> &'a mut Seg {
    segs.get_or_insert_with(k, || new_seg(k))
}

const SUBNETS: [&str; 4] = ["192.168.1", "10.10.0", "192.168.50", "172.16.4"];

pub fn evaluate(world: &World) -> Net {
    let mut uf = Uf {
        parent: HashMap::new(),
    };
    for d in &world.devices {
        for (p, _) in ports_of(&d.kind) {
            let k = key(&d.id, p);
            uf.parent.insert(k.clone(), k);
        }
    }
    for l in &world.links {
        if world.link_up(l) {
            uf.union(&key(&l.a.dev, &l.a.port), &key(&l.b.dev, &l.b.port));
        }
    }
    for d in &world.devices {
        if !d.power {
            continue;
        }
        let ps: Vec<&str> = ports_of(&d.kind).iter().map(|p| p.0).collect();
        if matches!(d.kind.as_str(), "switch" | "bus" | "ap" | "internet") {
            for p in ps.iter().skip(1) {
                uf.union(&key(&d.id, ps[0]), &key(&d.id, p));
            }
        }
        if d.kind == "router" {
            for p in ps.iter().skip(2) {
                uf.union(&key(&d.id, "lan1"), &key(&d.id, p));
            }
        }
    }
    let mut net = Net::default();
    let segs = &mut net.segs;
    for d in &world.devices {
        if !d.power {
            continue;
        }
        if d.kind == "internet" {
            let k = uf.find(&key(&d.id, "wan1"));
            let s = get_seg(segs, &k);
            s.kind = "internet";
            s.internet = true;
            s.cloud = Some(d.id.clone());
        }
        if d.kind == "router" {
            let k = uf.find(&key(&d.id, "lan1"));
            get_seg(segs, &k).routers.push(d.id.clone());
        }
    }
    for s in segs.values_mut() {
        if s.kind != "internet" && !s.routers.is_empty() {
            s.kind = "lan";
            s.router = Some(s.routers[0].clone());
        }
    }
    // active interface per endpoint (Ethernet beats Wi-Fi), and the router's WAN
    for d in &world.devices {
        if !d.power {
            continue;
        }
        if is_endpoint(&d.kind) {
            for (p, _) in ports_of(&d.kind) {
                if world
                    .links
                    .iter()
                    .any(|l| world.link_up(l) && l.at(&d.id, p))
                {
                    let seg = uf.find(&key(&d.id, p));
                    net.iface.insert(
                        d.id.clone(),
                        Iface {
                            port: p.to_string(),
                            seg,
                        },
                    );
                    break;
                }
            }
        }
        if d.kind == "router"
            && world
                .links
                .iter()
                .any(|l| world.link_up(l) && l.at(&d.id, "wan"))
        {
            let seg = uf.find(&key(&d.id, "wan"));
            net.iface.insert(
                d.id.clone(),
                Iface {
                    port: "wan".into(),
                    seg,
                },
            );
        }
    }
    for (id, f) in net.iface.iter() {
        let s = get_seg(&mut net.segs, &f.seg);
        if !s.members.contains(id) {
            s.members.push(id.clone());
        }
    }
    // subnets per router, in a stable order
    let routers: Vec<&str> = world
        .devices
        .iter()
        .filter(|d| d.kind == "router")
        .map(|d| d.id.as_str())
        .collect();
    for (i, r) in routers.iter().enumerate() {
        let sn = match world.dev(r) {
            Some(d) if d.flag("subnet") => d.text("subnet"),
            _ => SUBNETS[i % 4].to_string(),
        };
        net.subnet.insert(r.to_string(), sn);
    }
    // addresses: internet segment first (public), then LANs outward
    let mut public = 0u32;
    let internet_segs: Vec<Vec<String>> = net
        .segs
        .values()
        .filter(|s| s.kind == "internet")
        .map(|s| s.members.clone())
        .collect();
    for members in internet_segs {
        for id in members {
            let Some(d) = world.dev(&id) else { continue };
            let ip = if d.kind == "edge-official" {
                public += 1;
                format!("49.12.34.{}", 56 + public - 1)
            } else if d.kind == "router" {
                let i = routers.iter().position(|r| *r == id).unwrap_or(0);
                format!("85.216.{}.{}", 120 + i, 17 + i * 3)
            } else {
                public += 1;
                format!("49.12.40.{}", 10 + public - 1)
            };
            net.addr.insert(
                id,
                Addr {
                    ip,
                    mask: 24,
                    gw: Some(String::new()),
                    src: "public",
                    router: None,
                },
            );
        }
    }
    let lans: Vec<(String, Vec<String>)> = net
        .segs
        .values()
        .filter(|s| s.kind == "lan")
        .filter_map(|s| s.router.clone().map(|r| (r, s.members.clone())))
        .collect();
    for _pass in 0..4 {
        for (r, members) in &lans {
            let sn = net.subnet.get(r).cloned().unwrap_or_default();
            let mut n = 100;
            net.addr.insert(
                format!("{r}#lan"),
                Addr {
                    ip: format!("{sn}.1"),
                    mask: 24,
                    gw: None,
                    src: "router",
                    router: None,
                },
            );
            for id in members {
                if id == r {
                    continue;
                }
                net.addr.insert(
                    id.clone(),
                    Addr {
                        ip: format!("{sn}.{n}"),
                        mask: 24,
                        gw: Some(format!("{sn}.1")),
                        src: "dhcp",
                        router: Some(r.clone()),
                    },
                );
                n += 1;
            }
        }
    }
    let mut ll: u32 = 20;
    let isolated: Vec<Vec<String>> = net
        .segs
        .values()
        .filter(|s| s.kind == "isolated")
        .map(|s| s.members.clone())
        .collect();
    for members in isolated {
        for id in members {
            if net.addr.contains(&id) {
                continue;
            }
            let ip = format!("169.254.{}.{}", 10 + (ll >> 8), ll & 255);
            ll += 1;
            net.addr.insert(
                id,
                Addr {
                    ip,
                    mask: 16,
                    gw: Some(String::new()),
                    src: "link-local",
                    router: None,
                },
            );
        }
    }
    let reach: Vec<(String, bool)> = net
        .iface
        .iter()
        .map(|(id, f)| (id.clone(), has_internet(&net, &f.seg, 0)))
        .collect();
    for (id, v) in reach {
        net.internet.insert(id, v);
    }
    // network gear: the segment its own OS sits on, and its management address
    let (mut mg, mut gl) = (2u32, 200u32);
    for d in &world.devices {
        if !d.power || !matches!(d.kind.as_str(), "router" | "switch" | "ap") {
            continue;
        }
        let first = if d.kind == "router" {
            "lan1"
        } else {
            ports_of(&d.kind).first().map(|p| p.0).unwrap_or("")
        };
        let k = uf.find(&key(&d.id, first));
        net.gear_seg.insert(d.id.clone(), k.clone());
        if d.kind == "router" {
            if let Some(a) = net.addr.get(&format!("{}#lan", d.id)) {
                let m = Mgmt {
                    ip: a.ip.clone(),
                    mask: 24,
                    gw: None,
                };
                net.mgmt.insert(d.id.clone(), m);
            }
            continue;
        }
        let lan_router = net
            .segs
            .get(&k)
            .filter(|s| s.kind == "lan")
            .and_then(|s| s.router.clone());
        let m = match lan_router {
            Some(r) => {
                let sn = net.subnet.get(&r).cloned().unwrap_or_default();
                mg += 1;
                Mgmt {
                    ip: format!("{sn}.{}", mg - 1),
                    mask: 24,
                    gw: Some(format!("{sn}.1")),
                }
            }
            None => {
                gl += 1;
                Mgmt {
                    ip: format!("169.254.{}.1", gl - 1),
                    mask: 16,
                    gw: None,
                }
            }
        };
        net.mgmt.insert(d.id.clone(), m);
    }
    net.losos = compute_losos(world, &net);
    net
}

fn has_internet(net: &Net, seg: &str, depth: u32) -> bool {
    let Some(s) = net.segs.get(seg) else {
        return false;
    };
    if depth > 6 {
        return false;
    }
    if s.kind == "internet" {
        return true;
    }
    if s.kind != "lan" {
        return false;
    }
    let Some(r) = &s.router else { return false };
    match net.iface.get(r) {
        Some(w) => has_internet(net, &w.seg, depth + 1),
        None => false,
    }
}

fn compute_losos(world: &World, net: &Net) -> Losos {
    let mut l = Losos::default();
    let hubs: Vec<&crate::world::Device> = world
        .devices
        .iter()
        .filter(|d| d.kind == "edge-official")
        .collect();
    let live_hub = hubs
        .iter()
        .find(|h| h.power && net.has_internet(&h.id) && net.ip(&h.id).is_some())
        .copied();
    for s in world.devices.iter().filter(|d| d.kind == "edge-local") {
        let mut st = SpokeSt {
            up: s.power && net.ip(&s.id).is_some(),
            uplink: "off",
            relayed: vec![],
            enrolled: vec![],
            cluster: vec![],
            hub: None,
        };
        if st.up && s.flag("uplink") {
            if !net.has_internet(&s.id) {
                st.uplink = "no internet";
            } else if let Some(h) = live_hub {
                if !h.flag("acceptsRelay") {
                    st.uplink = "refused by hub";
                } else {
                    st.uplink = "up";
                    st.hub = Some(h.id.clone());
                }
            } else {
                st.uplink = "hub unreachable";
            }
        }
        l.spoke.insert(s.id.clone(), st);
    }
    for h in &hubs {
        l.hub.insert(
            h.id.clone(),
            HubSt {
                up: h.power && net.ip(&h.id).is_some() && net.has_internet(&h.id),
                tenants: vec![],
                relays: vec![],
                cluster: vec![],
            },
        );
    }
    for b in world.devices.iter().filter(|d| d.kind == "box") {
        let mut st = BoxSt {
            up: b.power && net.ip(&b.id).is_some(),
            edges: vec![],
            path: None,
            tunnel: "off",
            public_name: None,
            mesh: "off",
            market: "unavailable",
            reason: "",
            mesh_edge: None,
        };
        if st.up {
            for e in world.devices.iter().filter(|d| d.kind == "edge-local") {
                if e.power
                    && e.flag("advertise")
                    && net.same_seg(&b.id, &e.id)
                    && net.ip(&e.id).is_some()
                {
                    st.edges.push(Edge {
                        id: e.id.clone(),
                        source: "lan",
                        official: false,
                        url: format!("http://{}.local:8443", e.name),
                    });
                }
            }
            if let Some(h) = live_hub.filter(|_| b.flag("proxy") && net.has_internet(&b.id)) {
                st.edges.push(Edge {
                    id: h.id.clone(),
                    source: "configured",
                    official: h.flag("certified"),
                    url: format!("https://register.{}", h.text("domain")),
                });
            }
            st.path = st.edges.first().cloned();
            st.market = if st.edges.iter().any(|e| e.official) {
                "available"
            } else {
                "unavailable (noOfficialEdge)"
            };
            match (&st.path, b.flag("proxy")) {
                (Some(path), true) => {
                    if let Some(e) = world.dev(&path.id) {
                        if path.source == "lan" {
                            if e.flag("openEnrolment") {
                                st.tunnel = "enrolled";
                                let mut hub_of = None;
                                if let Some(sp) = l.spoke.get_mut(&e.id) {
                                    sp.enrolled.push(b.id.clone());
                                    if sp.uplink == "up" {
                                        st.public_name =
                                            Some(format!("{}.{}", b.name, e.text("zone")));
                                        sp.relayed.push(b.id.clone());
                                        hub_of = sp.hub.clone();
                                    }
                                }
                                if let Some(h) = hub_of.and_then(|h| l.hub.get_mut(&h)) {
                                    h.relays.push(b.id.clone());
                                }
                            } else {
                                st.tunnel = "refused";
                                st.reason = "gateway has closed enrolment";
                            }
                        } else if b.flag("tenantOnHub") {
                            st.tunnel = "registered";
                            st.public_name = Some(format!("{}.{}", b.name, e.text("domain")));
                            if let Some(h) = l.hub.get_mut(&e.id) {
                                h.tenants.push(b.id.clone());
                            }
                        } else {
                            st.tunnel = "refused";
                            st.reason = "not a tenant of the official edge";
                        }
                    }
                }
                (None, true) => {
                    st.tunnel = "stopped";
                    st.reason = "no edge in reach (/run/losos/edge-none)";
                }
                _ => {}
            }
            if b.flag("joinMesh") {
                match &st.path {
                    None => st.mesh = "refused: 409 edgeRequired",
                    Some(path) => {
                        if let Some(e) = world.dev(&path.id) {
                            if e.flag("cluster") {
                                st.mesh = "joined";
                                st.mesh_edge = Some(e.id.clone());
                                if e.kind == "edge-local" {
                                    if let Some(sp) = l.spoke.get_mut(&e.id) {
                                        sp.cluster.push(b.id.clone());
                                    }
                                } else if let Some(h) = l.hub.get_mut(&e.id) {
                                    h.cluster.push(b.id.clone());
                                }
                            } else {
                                st.mesh = "edge runs no mesh control plane";
                            }
                        }
                    }
                }
            }
        }
        l.boxes.insert(b.id.clone(), st);
    }
    l
}

/// What a name resolves to from one device (`resolveName`).
#[derive(Clone, Debug, PartialEq)]
pub enum Resolved {
    To {
        dev: String,
        via: &'static str,
        mdns: bool,
        hub: Option<String>,
        spoke: Option<String>,
    },
    Error(&'static str),
}

fn to(dev: &str, via: &'static str) -> Resolved {
    Resolved::To {
        dev: dev.to_string(),
        via,
        mdns: false,
        hub: None,
        spoke: None,
    }
}

pub fn resolve_name(world: &World, net: &Net, from: &str, host: &str) -> Resolved {
    let host = host.to_lowercase();
    if is_dotted_quad(&host) {
        for (id, a) in net.addr.iter() {
            if a.ip != host {
                continue;
            }
            let real = id.split('#').next().unwrap_or(id);
            if id.ends_with("#lan") {
                let mine = net
                    .seg_of(from)
                    .and_then(|s| net.segs.get(s))
                    .and_then(|s| s.router.as_deref());
                if net.seg_of(from).is_some() && mine == Some(real) {
                    return to(real, "lan");
                }
                continue;
            }
            if net.same_seg(from, real) {
                return to(real, "lan");
            }
            if a.src == "public" && net.has_internet(from) {
                return to(real, "internet");
            }
        }
        return Resolved::Error("timeout");
    }
    if let Some(label) = host.strip_suffix(".local") {
        let t = world
            .devices
            .iter()
            .find(|d| d.name.to_lowercase() == label && is_endpoint(&d.kind));
        if let Some(t) = t {
            if net.same_seg(from, &t.id) && net.ip(&t.id).is_some() {
                return Resolved::To {
                    dev: t.id.clone(),
                    via: "lan",
                    mdns: true,
                    hub: None,
                    spoke: None,
                };
            }
        }
        return Resolved::Error("mdns");
    }
    if !net.has_internet(from) {
        return Resolved::Error("nodns");
    }
    let hub = world.devices.iter().find(|d| {
        let domain = d.text("domain");
        d.kind == "edge-official"
            && net.losos.hub.get(&d.id).is_some_and(|h| h.up)
            && (host == format!("register.{domain}")
                || host == format!("edge.{domain}")
                || host == domain)
    });
    if let Some(h) = hub {
        return to(&h.id, "internet");
    }
    for (bid, st) in net.losos.boxes.iter() {
        if st.public_name.as_deref() == Some(host.as_str()) {
            let Some(path) = &st.path else { continue };
            let via_spoke = (path.source == "lan").then(|| path.id.clone());
            let hub_id = match &via_spoke {
                Some(s) => net.losos.spoke.get(s).and_then(|sp| sp.hub.clone()),
                None => Some(path.id.clone()),
            };
            return Resolved::To {
                dev: bid.clone(),
                via: "tunnel",
                mdns: false,
                hub: hub_id,
                spoke: via_spoke,
            };
        }
    }
    Resolved::Error("nxdomain")
}
