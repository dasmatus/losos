//! The fixed tables: device types, sites, link kinds, protocols, timers and
//! the defaults a new device starts from (model.js, sim.js).

use serde_json::{json, Map, Value};

pub struct TypeInfo {
    pub key: &'static str,
    pub label: &'static str,
    pub short: &'static str,
    pub cat: &'static str,
    pub ports: &'static [(&'static str, &'static str)],
    pub endpoint: bool,
    pub emulate: Option<&'static str>,
    pub blurb: &'static str,
}

const ETH0: &[(&str, &str)] = &[("eth0", "eth")];
const EIGHT_P: &[(&str, &str)] = &[
    ("p1", "eth"),
    ("p2", "eth"),
    ("p3", "eth"),
    ("p4", "eth"),
    ("p5", "eth"),
    ("p6", "eth"),
    ("p7", "eth"),
    ("p8", "eth"),
];
const EIGHT_T: &[(&str, &str)] = &[
    ("t1", "eth"),
    ("t2", "eth"),
    ("t3", "eth"),
    ("t4", "eth"),
    ("t5", "eth"),
    ("t6", "eth"),
    ("t7", "eth"),
    ("t8", "eth"),
];
const EIGHT_WAN: &[(&str, &str)] = &[
    ("wan1", "eth"),
    ("wan2", "eth"),
    ("wan3", "eth"),
    ("wan4", "eth"),
    ("wan5", "eth"),
    ("wan6", "eth"),
    ("wan7", "eth"),
    ("wan8", "eth"),
];

pub const TYPES: &[TypeInfo] = &[
    TypeInfo {
        key: "box",
        label: "LosOS box",
        short: "Box",
        cat: "losos",
        ports: ETH0,
        endpoint: true,
        emulate: Some("box"),
        blurb: "A mini-PC running LosOS: Nextcloud and Forgejo, the admin UI on :80.",
    },
    TypeInfo {
        key: "edge-local",
        label: "Edge gateway",
        short: "Gateway",
        cat: "losos",
        ports: ETH0,
        endpoint: true,
        emulate: Some("edge-local"),
        blurb: "A local edge (spoke): registrar, rathole server, LAN advert, optional mesh.",
    },
    TypeInfo {
        key: "edge-official",
        label: "Official edge",
        short: "Official edge",
        cat: "losos",
        ports: ETH0,
        endpoint: true,
        emulate: Some("edge-official"),
        blurb: "An official edge (hub) on a VPS, certified by the LosOS root key.",
    },
    TypeInfo {
        key: "laptop",
        label: "Laptop · LosOS Desktop",
        short: "Laptop",
        cat: "end",
        ports: &[("eth0", "eth"), ("wlan0", "wifi")],
        endpoint: true,
        emulate: None,
        blurb: "A laptop running LosOS Desktop (derisk), with the Danube browser.",
    },
    TypeInfo {
        key: "router",
        label: "Router",
        short: "Router",
        cat: "net",
        ports: &[
            ("wan", "eth"),
            ("lan1", "eth"),
            ("lan2", "eth"),
            ("lan3", "eth"),
            ("lan4", "eth"),
        ],
        endpoint: false,
        emulate: Some("router"),
        blurb: "Home or office router: DHCP and NAT for one LAN. Runs Netzgeräte Betriebssystem.",
    },
    TypeInfo {
        key: "switch",
        label: "Switch",
        short: "Switch",
        cat: "net",
        ports: EIGHT_P,
        endpoint: false,
        emulate: Some("switch"),
        blurb: "Eight-port Ethernet switch with a management address. Runs Netzgeräte Betriebssystem.",
    },
    TypeInfo {
        key: "ap",
        label: "Wi-Fi access point",
        short: "Access point",
        cat: "net",
        ports: &[("eth0", "eth"), ("wifi", "wifi-ap")],
        endpoint: false,
        emulate: Some("ap"),
        blurb: "Bridges wireless clients onto its Ethernet segment. Runs Netzgeräte Betriebssystem.",
    },
    TypeInfo {
        key: "bus",
        label: "Coax bus",
        short: "Bus",
        cat: "net",
        ports: EIGHT_T,
        endpoint: false,
        emulate: None,
        blurb: "One shared cable with a tap per device and a terminator at each end: every frame reaches every tap.",
    },
    TypeInfo {
        key: "internet",
        label: "Internet",
        short: "Internet",
        cat: "net",
        ports: EIGHT_WAN,
        endpoint: false,
        emulate: None,
        blurb: "The public internet: routers' WAN ports and VPSes attach here.",
    },
];

pub fn type_info(kind: &str) -> Option<&'static TypeInfo> {
    TYPES.iter().find(|t| t.key == kind)
}

/// The ports of a type, empty for a type the catalog does not know.
pub fn ports_of(kind: &str) -> &'static [(&'static str, &'static str)] {
    type_info(kind).map(|t| t.ports).unwrap_or(&[])
}

pub fn is_endpoint(kind: &str) -> bool {
    type_info(kind).is_some_and(|t| t.endpoint)
}

pub fn port_kind(kind: &str, port: &str) -> Option<&'static str> {
    ports_of(kind).iter().find(|p| p.0 == port).map(|p| p.1)
}

pub fn is_gear(kind: &str) -> bool {
    matches!(kind, "router" | "switch" | "bus" | "ap" | "internet")
}

pub const SITES: &[(&str, &str, &str)] = &[
    ("home", "Home", "Flat in Bratislava"),
    ("office", "Office", "Company floor"),
    ("dc", "Datacenter", "VPS rack, Falkenstein"),
    ("isp", "Internet", "ISP uplinks"),
];

pub fn is_site(key: &str) -> bool {
    SITES.iter().any(|s| s.0 == key)
}

pub const LINK_KINDS: &[(&str, &str, &str)] = &[
    (
        "auto",
        "Automatic",
        "Picks the cable and the first free ports.",
    ),
    ("copper", "Copper", "Ethernet patch cable."),
    ("fiber", "Fiber", "Fiber run between buildings or racks."),
    (
        "wifi",
        "Wireless",
        "Associate a laptop with an access point.",
    ),
];

pub fn is_link_kind(key: &str) -> bool {
    LINK_KINDS.iter().any(|k| k.0 == key)
}

/// Protocols in the order the filter chips list them.
pub const PROTO: &[(&str, &str, &str)] = &[
    ("DHCP", "#8e6fd1", "DHCP"),
    ("ARP", "#9aa34a", "ARP"),
    ("mDNS", "#2f9e8f", "mDNS"),
    ("DNS", "#4f86e0", "DNS"),
    ("HTTP", "#e8775a", "HTTP(S)"),
    ("ICMP", "#d4a72c", "ICMP"),
    ("rathole", "#2b6cb0", "rathole/Noise"),
    ("rke2", "#c05fa0", "rke2 (mesh)"),
    ("lososd", "#5c6a77", "lososd"),
];

pub fn is_proto(key: &str) -> bool {
    PROTO.iter().any(|p| p.0 == key)
}

pub const SPEEDS: &[f64] = &[1.0, 10.0, 60.0, 600.0, 3600.0];

pub struct Daily {
    pub at: &'static str,
    pub key: &'static str,
    pub title: &'static str,
    pub minute: i64,
}

pub const DAILY: &[Daily] = &[
    Daily {
        at: "00:07",
        key: "reboot",
        title: "midnight-reboot.timer",
        minute: 7,
    },
    Daily {
        at: "03:00",
        key: "upgrade",
        title: "nixos-upgrade.timer",
        minute: 180,
    },
    Daily {
        at: "04:30",
        key: "gc",
        title: "nix-gc.timer",
        minute: 270,
    },
    Daily {
        at: "07:00",
        key: "windowEnd",
        title: "compute window closes",
        minute: 420,
    },
    Daily {
        at: "23:00",
        key: "windowStart",
        title: "compute window opens",
        minute: 1380,
    },
];

pub fn default_cfg(kind: &str) -> Map<String, Value> {
    let v = match kind {
        "box" => {
            json!({ "proxy": true, "tenantOnHub": true, "joinMesh": false, "shareCompute": false })
        }
        "edge-local" => json!({
            "advertise": true, "openEnrolment": true, "cluster": true, "uplink": true, "zone": "acme.losos.cfd"
        }),
        "edge-official" => {
            json!({ "certified": true, "domain": "losos.cfd", "cluster": true, "acceptsRelay": true })
        }
        "router" => json!({ "subnet": "" }),
        "ap" => json!({ "ssid": "losos-lab" }),
        _ => json!({}),
    };
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

pub fn default_name(kind: &str) -> &'static str {
    match kind {
        "box" => "mattbox",
        "edge-local" => "acme-gw",
        "edge-official" => "edge",
        "laptop" => "matus-laptop",
        "router" => "router",
        "switch" => "switch",
        "bus" => "bus",
        "ap" => "ap",
        "internet" => "internet",
        _ => "device",
    }
}

pub fn default_site(kind: &str) -> &'static str {
    match kind {
        "edge-official" => "dc",
        "internet" => "isp",
        _ => "home",
    }
}

/// TYPES, SITES, LINK_KINDS, PROTO and the rest, shaped as the JS objects.
pub fn catalog_json() -> Value {
    let mut types = Map::new();
    for t in TYPES {
        let mut o = Map::new();
        o.insert("label".into(), t.label.into());
        o.insert("short".into(), t.short.into());
        o.insert("cat".into(), t.cat.into());
        o.insert(
            "ports".into(),
            Value::Array(t.ports.iter().map(|(p, k)| json!([p, k])).collect()),
        );
        if t.endpoint {
            o.insert("endpoint".into(), true.into());
        }
        if let Some(e) = t.emulate {
            o.insert("emulate".into(), e.into());
        }
        o.insert("blurb".into(), t.blurb.into());
        types.insert(t.key.into(), Value::Object(o));
    }
    let mut sites = Map::new();
    for (k, name, sub) in SITES {
        sites.insert((*k).into(), json!({ "name": name, "sub": sub }));
    }
    let mut kinds = Map::new();
    for (k, label, hint) in LINK_KINDS {
        kinds.insert((*k).into(), json!({ "label": label, "hint": hint }));
    }
    let mut proto = Map::new();
    for (k, color, label) in PROTO {
        proto.insert((*k).into(), json!({ "color": color, "label": label }));
    }
    let mut cfg = Map::new();
    let mut names = Map::new();
    let mut site = Map::new();
    for t in TYPES {
        cfg.insert(t.key.into(), Value::Object(default_cfg(t.key)));
        names.insert(t.key.into(), default_name(t.key).into());
        site.insert(t.key.into(), default_site(t.key).into());
    }
    json!({
        "types": types,
        "sites": sites,
        "linkKinds": kinds,
        "proto": proto,
        "speeds": SPEEDS,
        "daily": DAILY.iter().map(|d| json!({ "at": d.at, "key": d.key, "title": d.title })).collect::<Vec<_>>(),
        "defaultCfg": cfg,
        "defaultName": names,
        "defaultSite": site,
    })
}
