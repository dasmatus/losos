//! Ready-made setups (scenarios.js) and "This box" (thisbox.js).

use crate::js::{prop, string, truthy};
use crate::lab::LabCore;
use crate::world::NewOpts;
use serde_json::Value;

pub struct Builtin {
    pub key: &'static str,
    pub name: &'static str,
    pub blurb: &'static str,
}

pub const BUILTIN: &[Builtin] = &[
    Builtin { key: "two-sites", name: "Two sites, one official edge", blurb: "A home box on the official edge directly, and an office behind its own gateway that relays to the same hub." },
    Builtin { key: "home", name: "Home: one box", blurb: "One box behind a home router, reached from the laptop on Wi-Fi and from outside through the official edge." },
    Builtin { key: "office", name: "Company with its own gateway", blurb: "An office LAN: a gateway with open enrolment and its own mesh, three boxes, one uplink to the official edge." },
    Builtin { key: "star", name: "Star: everything on one switch", blurb: "Every device has its own cable to one central switch. One cable down takes one device off; the switch down takes everyone off." },
    Builtin { key: "bus", name: "Bus: one shared coax cable", blurb: "All devices tap into one cable with a terminator at each end. Every frame reaches every tap, so every device sees every other device’s traffic." },
    Builtin { key: "web", name: "Web: four switches cabled to each other", blurb: "Every switch has a cable to every other switch, so traffic has another way round when a cable is pulled. Real switches need spanning tree for this; the canvas takes the shortest live path." },
    Builtin { key: "lan", name: "LAN only, no internet", blurb: "A switch, a gateway and two boxes with no router: link-local addresses, mDNS discovery and a local mesh. No public names, no market." },
    Builtin { key: "empty", name: "Empty canvas", blurb: "Start from nothing: drag devices up from the tray." },
];

/// The settings and edges read from the box, kept to rebuild "This box".
#[derive(Clone, Debug)]
pub struct ThisBox {
    pub name: String,
    pub blurb: String,
    settings: Value,
    edge: Value,
}

/// A device name on the canvas: lower case, letters, digits and dashes.
pub fn device_name(text: Option<&Value>, fallback: &str) -> String {
    let raw = if truthy(text) {
        string(text)
    } else {
        String::new()
    };
    let lower = raw.to_lowercase();
    let lower = lower.strip_suffix(".local").unwrap_or(&lower);
    let mut out = String::new();
    let mut in_run = false;
    for c in lower.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' {
            out.push(c);
            in_run = false;
        } else if !in_run {
            out.push('-');
            in_run = true;
        }
    }
    let name = out.trim_matches('-').to_string();
    if name.is_empty() {
        fallback.to_string()
    } else {
        name
    }
}

pub fn device_name_str(text: &str, fallback: &str) -> String {
    device_name(Some(&Value::String(text.to_string())), fallback)
}

pub fn edge_host(url: Option<&Value>) -> String {
    let s = match url {
        Some(Value::String(s)) => s.clone(),
        other => string(other),
    };
    crate::url::parse(&s)
        .map(|u| u.hostname)
        .unwrap_or_default()
}

fn objects(v: Option<&Value>) -> Vec<&Value> {
    match v {
        Some(Value::Array(a)) => a.iter().collect(),
        _ => vec![],
    }
}

pub fn this_box(settings: Value, edge: Value) -> ThisBox {
    let s = Some(&settings);
    let edges = objects(prop(Some(&edge), "edges"));
    let host_name = string(prop(s, "hostName"));
    let found = if edges.is_empty() {
        "no edge in reach".to_string()
    } else {
        edges
            .iter()
            .map(|e| {
                let n = prop(Some(e), "name");
                if truthy(n) {
                    string(n)
                } else {
                    edge_host(prop(Some(e), "url"))
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let on = |k: &str, a: &'static str, b: &'static str| if truthy(prop(s, k)) { a } else { b };
    ThisBox {
        name: format!("This box: {host_name}"),
        blurb: format!(
            "Drawn from {host_name}'s own settings: tunnel {}, compute mesh {}, storage {}. Edges it found: {found}. The router, the cabling and the laptop are assumed.",
            on("proxyEnable", "on", "off"),
            on("clusterEnable", "joined", "off"),
            on("sharingMyStorage", "shared", "local"),
        ),
        settings,
        edge,
    }
}

fn o(name: &str, site: Option<&str>, px: f64, py: f64) -> NewOpts {
    NewOpts {
        name: Some(name.into()),
        site: site.map(str::to_string),
        px: Some(px),
        py: Some(py),
        ..NewOpts::default()
    }
}

fn mesh(mut n: NewOpts) -> NewOpts {
    n.cfg.push(("joinMesh".into(), Value::Bool(true)));
    n
}

impl LabCore {
    fn nd(&mut self, kind: &str, x: f64, y: f64, opts: NewOpts) -> String {
        self.world.new_device(kind, x, y, opts, &mut self.rng)
    }
    fn inet(&mut self, x: f64, y: f64) -> String {
        let n = NewOpts {
            px: Some(200.0),
            py: Some(110.0),
            ..NewOpts::default()
        };
        self.nd("internet", x, y, n)
    }
    fn link(&mut self, a: &str, b: &str, kind: &str, ap: Option<&str>, bp: Option<&str>) {
        let _ = self.world.connect(a, b, kind, ap, bp);
    }
    fn c(&mut self, a: &str, b: &str) {
        self.link(a, b, "auto", None, None);
    }

    /// Builds a built-in setup into the (empty) world; returns the device to select.
    pub(crate) fn build_builtin(&mut self, key: &str) -> Option<String> {
        const O: Option<&str> = Some("office");
        match key {
            "two-sites" => {
                let inet = self.inet(640.0, 90.0);
                let hub = self.nd("edge-official", 900.0, 90.0, o("edge", None, 40.0, 70.0));
                let hr = self.nd("router", 330.0, 250.0, o("home-router", None, 40.0, 50.0));
                let ap = self.nd("ap", 180.0, 360.0, o("home-wifi", None, 190.0, 40.0));
                let bx = self.nd("box", 340.0, 430.0, o("mattbox", None, 60.0, 250.0));
                let lap = self.nd(
                    "laptop",
                    140.0,
                    500.0,
                    o("matus-laptop", None, 260.0, 230.0),
                );
                let or = self.nd("router", 950.0, 250.0, o("office-router", O, 40.0, 60.0));
                let sw = self.nd("switch", 950.0, 370.0, o("office-switch", O, 40.0, 120.0));
                let gw = self.nd("edge-local", 760.0, 470.0, o("acme-gw", O, 40.0, 180.0));
                let ob = self.nd("box", 950.0, 500.0, o("teambox", O, 260.0, 250.0));
                let ob2 = self.nd("box", 1130.0, 470.0, o("annabox", O, 400.0, 250.0));
                self.link(&hr, &inet, "auto", Some("wan"), None);
                self.link(&or, &inet, "auto", Some("wan"), None);
                self.link(&hub, &inet, "fiber", None, None);
                self.link(&hr, &bx, "auto", Some("lan1"), None);
                self.link(&hr, &ap, "auto", Some("lan2"), None);
                self.link(&ap, &lap, "wifi", None, None);
                self.link(&or, &sw, "auto", Some("lan1"), None);
                self.c(&sw, &gw);
                self.c(&sw, &ob);
                self.c(&sw, &ob2);
                Some(lap)
            }
            "home" => {
                let inet = self.inet(640.0, 100.0);
                let hub = self.nd("edge-official", 960.0, 100.0, o("edge", None, 40.0, 70.0));
                let hr = self.nd("router", 520.0, 270.0, o("home-router", None, 40.0, 50.0));
                let ap = self.nd("ap", 330.0, 380.0, o("home-wifi", None, 190.0, 40.0));
                let bx = self.nd("box", 640.0, 430.0, o("mattbox", None, 60.0, 250.0));
                let lap = self.nd(
                    "laptop",
                    300.0,
                    520.0,
                    o("matus-laptop", None, 260.0, 230.0),
                );
                self.link(&hr, &inet, "auto", Some("wan"), None);
                self.link(&hub, &inet, "fiber", None, None);
                self.link(&hr, &bx, "auto", Some("lan1"), None);
                self.link(&hr, &ap, "auto", Some("lan2"), None);
                self.link(&ap, &lap, "wifi", None, None);
                Some(bx)
            }
            "office" => {
                let inet = self.inet(640.0, 90.0);
                let hub = self.nd("edge-official", 960.0, 90.0, o("edge", None, 40.0, 70.0));
                let or = self.nd("router", 640.0, 230.0, o("office-router", O, 40.0, 60.0));
                let sw = self.nd("switch", 640.0, 350.0, o("office-switch", O, 40.0, 120.0));
                let gw = self.nd("edge-local", 360.0, 470.0, o("acme-gw", O, 40.0, 180.0));
                let b1 = self.nd("box", 560.0, 520.0, mesh(o("teambox", O, 230.0, 250.0)));
                let b2 = self.nd("box", 740.0, 520.0, mesh(o("annabox", O, 360.0, 250.0)));
                let b3 = self.nd("box", 920.0, 470.0, o("peterbox", O, 490.0, 250.0));
                let lap = self.nd("laptop", 360.0, 300.0, o("matus-laptop", O, 230.0, 140.0));
                self.link(&or, &inet, "auto", Some("wan"), None);
                self.link(&hub, &inet, "fiber", None, None);
                self.link(&or, &sw, "auto", Some("lan1"), None);
                for d in [&gw, &b1, &b2, &b3, &lap] {
                    self.c(&sw, d);
                }
                Some(gw)
            }
            "star" => {
                let inet = self.inet(640.0, 50.0);
                let hub = self.nd("edge-official", 960.0, 50.0, o("edge", None, 40.0, 70.0));
                let or = self.nd("router", 640.0, 170.0, o("office-router", O, 40.0, 45.0));
                let sw = self.nd("switch", 640.0, 360.0, o("core-switch", O, 40.0, 105.0));
                let gw = self.nd("edge-local", 380.0, 300.0, o("acme-gw", O, 40.0, 160.0));
                let b1 = self.nd("box", 400.0, 470.0, mesh(o("teambox", O, 250.0, 250.0)));
                let b2 = self.nd("box", 560.0, 540.0, mesh(o("annabox", O, 370.0, 250.0)));
                let b3 = self.nd("box", 740.0, 540.0, o("peterbox", O, 490.0, 250.0));
                let b4 = self.nd("box", 900.0, 470.0, o("evabox", O, 470.0, 120.0));
                let lap = self.nd("laptop", 900.0, 300.0, o("matus-laptop", O, 300.0, 110.0));
                self.link(&or, &inet, "auto", Some("wan"), None);
                self.link(&hub, &inet, "fiber", None, None);
                self.link(&or, &sw, "auto", Some("lan1"), None);
                for d in [&gw, &b1, &b2, &b3, &b4, &lap] {
                    self.c(&sw, d);
                }
                Some(sw)
            }
            "bus" => {
                let inet = self.inet(640.0, 50.0);
                let hub = self.nd("edge-official", 960.0, 50.0, o("edge", None, 40.0, 70.0));
                let or = self.nd("router", 300.0, 190.0, o("office-router", O, 40.0, 45.0));
                let bus = self.nd("bus", 640.0, 360.0, o("coax-bus", O, 250.0, 52.0));
                let gw = self.nd("edge-local", 480.0, 200.0, o("acme-gw", O, 40.0, 120.0));
                let b1 = self.nd("box", 420.0, 500.0, mesh(o("teambox", O, 250.0, 250.0)));
                let b2 = self.nd("box", 640.0, 520.0, mesh(o("annabox", O, 370.0, 250.0)));
                let b3 = self.nd("box", 860.0, 500.0, o("peterbox", O, 490.0, 250.0));
                let lap = self.nd("laptop", 860.0, 200.0, o("matus-laptop", O, 330.0, 140.0));
                self.link(&or, &inet, "auto", Some("wan"), None);
                self.link(&hub, &inet, "fiber", None, None);
                self.link(&or, &bus, "auto", Some("lan1"), Some("t1"));
                self.link(&gw, &bus, "auto", Some("eth0"), Some("t2"));
                self.link(&b1, &bus, "auto", Some("eth0"), Some("t4"));
                self.link(&b2, &bus, "auto", Some("eth0"), Some("t5"));
                self.link(&b3, &bus, "auto", Some("eth0"), Some("t6"));
                self.link(&lap, &bus, "auto", Some("eth0"), Some("t7"));
                Some(bus)
            }
            "web" => {
                let inet = self.inet(640.0, 40.0);
                let hub = self.nd("edge-official", 960.0, 40.0, o("edge", None, 40.0, 70.0));
                let or = self.nd("router", 380.0, 120.0, o("office-router", O, 40.0, 45.0));
                let s1 = self.nd("switch", 460.0, 250.0, o("sw-north", O, 40.0, 105.0));
                let s2 = self.nd("switch", 820.0, 250.0, o("sw-east", O, 262.0, 50.0));
                let s3 = self.nd("switch", 460.0, 460.0, o("sw-west", O, 40.0, 150.0));
                let s4 = self.nd("switch", 820.0, 460.0, o("sw-south", O, 262.0, 110.0));
                let gw = self.nd("edge-local", 230.0, 330.0, o("acme-gw", O, 218.0, 250.0));
                let b1 = self.nd("box", 1050.0, 250.0, mesh(o("teambox", O, 468.0, 250.0)));
                let b2 = self.nd("box", 640.0, 580.0, mesh(o("annabox", O, 343.0, 250.0)));
                let lap = self.nd("laptop", 1050.0, 470.0, o("matus-laptop", O, 455.0, 150.0));
                self.link(&or, &inet, "auto", Some("wan"), None);
                self.link(&hub, &inet, "fiber", None, None);
                self.link(&or, &s1, "auto", Some("lan1"), None);
                let s = [&s1, &s2, &s3, &s4];
                for i in 0..4 {
                    for j in i + 1..4 {
                        self.c(s[i], s[j]);
                    }
                }
                self.c(&s1, &gw);
                self.c(&s2, &b1);
                self.c(&s3, &b2);
                self.c(&s4, &lap);
                Some(s1)
            }
            "lan" => {
                let sw = self.nd("switch", 640.0, 260.0, o("lab-switch", O, 40.0, 100.0));
                let mut g = o("lab-gw", O, 40.0, 180.0);
                g.cfg.push(("uplink".into(), Value::Bool(false)));
                let gw = self.nd("edge-local", 420.0, 420.0, g);
                let b1 = self.nd("box", 640.0, 460.0, mesh(o("box-a", O, 230.0, 250.0)));
                let b2 = self.nd("box", 860.0, 420.0, mesh(o("box-b", O, 360.0, 250.0)));
                let lap = self.nd("laptop", 860.0, 230.0, o("matus-laptop", O, 230.0, 140.0));
                for d in [&gw, &b1, &b2, &lap] {
                    self.c(&sw, d);
                }
                Some(b1)
            }
            _ => None,
        }
    }

    pub(crate) fn build_this_box(&mut self, tb: &ThisBox) -> Option<String> {
        let s = Some(&tb.settings);
        let edge = Some(&tb.edge);
        let edges = objects(prop(edge, "edges"));
        let lan: Vec<&Value> = edges
            .iter()
            .filter(|e| prop(Some(e), "source").and_then(Value::as_str) == Some("lan"))
            .copied()
            .collect();
        let configured: Option<(Option<&Value>, bool)> = edges
            .iter()
            .find(|e| prop(Some(e), "source").and_then(Value::as_str) == Some("configured"))
            .map(|e| (prop(Some(e), "url"), truthy(prop(Some(e), "official"))))
            .or_else(|| {
                let u = prop(edge, "configuredUrl");
                truthy(u).then_some((u, false))
            });
        let inet = self.inet(640.0, 100.0);
        let hr = self.nd("router", 520.0, 270.0, o("router", None, 40.0, 50.0));
        let mut bo = o(
            &device_name(prop(s, "hostName"), "losos"),
            None,
            60.0,
            250.0,
        );
        let proxy = truthy(prop(s, "proxyEnable"));
        bo.cfg = vec![
            ("proxy".into(), Value::Bool(proxy)),
            ("tenantOnHub".into(), Value::Bool(proxy)),
            (
                "joinMesh".into(),
                Value::Bool(truthy(prop(s, "clusterEnable"))),
            ),
            (
                "shareCompute".into(),
                Value::Bool(truthy(prop(s, "shareCompute"))),
            ),
        ];
        let bx = self.nd("box", 640.0, 430.0, bo);
        let lap = self.nd("laptop", 330.0, 470.0, o("this-laptop", None, 260.0, 230.0));
        self.link(&hr, &inet, "auto", Some("wan"), None);
        self.link(&hr, &bx, "auto", Some("lan1"), None);
        self.link(&hr, &lap, "auto", Some("lan2"), None);
        for (i, e) in lan.iter().take(2).enumerate() {
            let n = prop(Some(e), "name");
            let text = if truthy(n) {
                n.cloned()
            } else {
                Some(Value::String(
                    edge_host(prop(Some(e), "url"))
                        .split('.')
                        .next()
                        .unwrap_or("")
                        .to_string(),
                ))
            };
            let name = device_name(text.as_ref(), "lan-edge");
            let gw = self.nd(
                "edge-local",
                860.0 + i as f64 * 160.0,
                430.0,
                o(&name, None, 200.0, 120.0 + i as f64 * 70.0),
            );
            let port = format!("lan{}", 3 + i);
            self.link(&hr, &gw, "auto", Some(&port), None);
        }
        if let Some((url, official)) = configured {
            let host = edge_host(url);
            let stripped = host
                .strip_prefix("register.")
                .or_else(|| host.strip_prefix("edge."))
                .unwrap_or(&host);
            let domain = if stripped.is_empty() {
                "losos.cfd"
            } else {
                stripped
            }
            .to_string();
            let mut h = o(
                &device_name_str(host.split('.').next().unwrap_or(""), "edge"),
                None,
                40.0,
                70.0,
            );
            h.cfg = vec![
                ("certified".into(), Value::Bool(official)),
                ("domain".into(), Value::String(domain)),
            ];
            let hub = self.nd("edge-official", 960.0, 100.0, h);
            self.link(&hub, &inet, "fiber", None, None);
        }
        Some(bx)
    }
}

#[cfg(test)]
mod tests {
    use super::device_name_str;

    #[test]
    fn names_are_sanitised() {
        assert_eq!(device_name_str("MattBox.local", "x"), "mattbox");
        assert_eq!(
            device_name_str("<script>alert(1)</script>", "x"),
            "script-alert-1-script"
        );
        assert_eq!(device_name_str("---", "x"), "x");
        assert_eq!(device_name_str("a..b", "x"), "a-b");
    }
}
