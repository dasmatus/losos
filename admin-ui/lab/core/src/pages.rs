//! What each device answers over HTTP (pages.js).

use crate::js::esc;
use crate::lab::LabCore;
use crate::net::Resolved;
use crate::url::Url;
use crate::world::Device;
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Page {
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<&'static str>,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    pub html: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(untagged)]
pub enum HttpResult {
    Page(Page),
    Error {
        error: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        host: Option<String>,
    },
}

fn page(status: u16, title: &str, text: Option<String>, html: String) -> Page {
    Page {
        status,
        error: None,
        title: title.into(),
        text,
        html,
    }
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
}

fn json_page(obj: Value) -> Page {
    let t = pretty(&obj);
    page(
        200,
        "application/json",
        Some(t.clone()),
        format!(
            "<div class=\"page\"><div class=\"pbody\"><pre>{}</pre></div></div>",
            esc(&t)
        ),
    )
}

pub fn err_page(h: &str, p: &str) -> String {
    format!("<div class=\"page err\"><h1>{h}</h1><p>{p}</p></div>")
}

fn refused(p: &str) -> Page {
    Page {
        status: 0,
        error: Some("refused"),
        title: "Connection refused".into(),
        text: None,
        html: err_page("Unable to connect", p),
    }
}

/// errorPage(err, host)
pub fn error_page(err: &str, host: &str) -> String {
    let h = esc(host);
    let (a, b) = match err {
        "mdns" => (
            "Server not found".to_string(),
            format!("<code>{h}</code> is an mDNS name. Only computers on the same network as the device can resolve it. From elsewhere, use the box's public name."),
        ),
        "nodns" => (
            "No internet connection".into(),
            "This computer has no route to a DNS server. Connect it to a router with internet, or to an access point on one.".into(),
        ),
        "nxdomain" => (
            "This site can’t be reached".into(),
            format!("DNS_PROBE_FINISHED_NXDOMAIN: no DNS record for <code>{h}</code>. A box gets a public name only once its tunnel is registered with an edge that is on the internet."),
        ),
        "timeout" => (
            "The connection timed out".into(),
            format!("<code>{h}</code> is a private address on another network. Routers do not forward inbound connections; that is what the tunnel through an edge is for."),
        ),
        "badurl" => ("Invalid address".into(), "Type a host name or an address.".into()),
        "offline" => (
            "You are offline".into(),
            "The laptop has no network link. Plug in a cable or join Wi-Fi.".into(),
        ),
        other => ("Error".into(), other.to_string()),
    };
    err_page(&a, &b)
}

impl LabCore {
    pub(crate) fn page_for(&self, r: &Resolved, u: &Url) -> Page {
        let Resolved::To { dev, via, .. } = r else {
            return refused("Nothing listens there.");
        };
        let Some(d) = self.world.dev(dev) else {
            return refused("Nothing listens there.");
        };
        let p = u.pathname.as_str();
        let via_tunnel = *via == "tunnel";
        let plate = &self.plate;
        match d.kind.as_str() {
            "box" => {
                if p.starts_with("/nextcloud") {
                    return self.cloud_page(d, via_tunnel);
                }
                if p.starts_with("/forgejo") {
                    return self.git_page(d);
                }
                let st = self.net.losos.boxes.get(&d.id);
                if p.starts_with("/api/edge") {
                    if via_tunnel {
                        return forbidden(d);
                    }
                    let path = st.and_then(|s| s.path.as_ref()).map(
                        |p| json!({ "name": self.nm(&p.id), "url": p.url, "source": p.source }),
                    );
                    let edges: Vec<Value> = st
                        .map(|s| {
                            s.edges
                                .iter()
                                .map(|e| json!({ "name": self.nm(&e.id), "url": e.url, "source": e.source, "official": e.official }))
                                .collect()
                        })
                        .unwrap_or_default();
                    return json_page(json!({ "path": path, "edges": edges }));
                }
                if via_tunnel {
                    return forbidden(d);
                }
                self.admin_page(d)
            }
            "edge-local" => {
                if p.starts_with("/health") {
                    return json_page(json!({ "status": "ok" }));
                }
                let Some(sp) = self.net.losos.spoke.get(&d.id) else {
                    return refused("Nothing listens there.");
                };
                let enrolled: Vec<String> = sp.enrolled.iter().map(|b| self.nm(b)).collect();
                let text = format!(
                    "losos-edge boxes\n{}",
                    if enrolled.is_empty() {
                        "(none)".to_string()
                    } else {
                        enrolled.join("\n")
                    }
                );
                let zone = d.text("zone");
                let uplink_extra = if sp.uplink == "up" {
                    format!(" · boxes are public under <code>{}</code>", esc(&zone))
                } else {
                    String::new()
                };
                let enrolled_html = if enrolled.is_empty() {
                    "<span class=\"muted\">none yet</span>".to_string()
                } else {
                    enrolled
                        .iter()
                        .map(|n| format!("<code>{}</code>", esc(n)))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let cluster: Vec<String> = sp.cluster.iter().map(|b| self.nm(b)).collect();
                let mesh = if d.flag("cluster") {
                    format!(
                        "rke2 server, {} node(s): {}",
                        sp.cluster.len(),
                        if cluster.is_empty() {
                            "—".to_string()
                        } else {
                            cluster.join(", ")
                        }
                    )
                } else {
                    "off".into()
                };
                let html = format!(
                    "<div class=\"page\"><div class=\"pbar\"><img src=\"{plate}\" alt=\"\"><b>{}</b><span class=\"muted\">LosOS edge gateway</span></div><div class=\"pbody\">\n      <h1>Gateway for this network</h1>\n      <div class=\"card\"><b>Uplink to the official edge:</b> {}{uplink_extra}</div>\n      <div class=\"card\"><b>Enrolled boxes</b> ({})<br>{enrolled_html}</div>\n      <div class=\"card\"><b>Mesh control plane</b>: {mesh}</div>\n      <p class=\"muted\">The gateway is not official: boxes behind it can share storage and compute, but trade on the market only through an official edge.</p></div></div>",
                    esc(&d.name),
                    esc(sp.uplink),
                    if d.flag("openEnrolment") { "open enrolment, trust on first use" } else { "closed enrolment" },
                );
                page(200, "LosOS edge gateway", Some(text), html)
            }
            "edge-official" => {
                if p.starts_with("/health") {
                    return json_page(json!({ "status": "ok" }));
                }
                let domain = d.text("domain");
                if p.starts_with("/identity") {
                    return json_page(json!({
                        "cert": {
                            "name": format!("register.{domain}"),
                            "signedBy": if d.flag("certified") { "LosOS root (keys/official-edge-root.pub)" } else { "unknown" },
                            "expires": "2027-04-01",
                        },
                        "nonceSignature": "ed25519:…",
                    }));
                }
                let h = self.net.losos.hub.get(&d.id);
                let names = |v: Option<&Vec<String>>| -> Vec<Value> {
                    v.map(|v| v.iter().map(|b| Value::String(self.nm(b))).collect())
                        .unwrap_or_default()
                };
                let relayed: Vec<Value> = h
                    .map(|h| {
                        h.relays
                            .iter()
                            .map(|b| {
                                match self
                                    .net
                                    .losos
                                    .boxes
                                    .get(b)
                                    .and_then(|s| s.public_name.clone())
                                {
                                    Some(n) => Value::String(n),
                                    None => Value::Null,
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                json_page(json!({
                    "registrar": format!("register.{domain}"),
                    "tenants": names(h.map(|h| &h.tenants)),
                    "relayed": relayed,
                    "mesh": names(h.map(|h| &h.cluster)),
                }))
            }
            "laptop" => refused(&format!("{} runs no web server.", esc(&d.name))),
            "router" => {
                let sn = self
                    .net
                    .subnet
                    .get(&d.id)
                    .cloned()
                    .unwrap_or_else(|| "undefined".into());
                page(
                    200,
                    "Router",
                    Some("router admin".into()),
                    format!(
                        "<div class=\"page err\"><h1>{} · router</h1><p>LAN {}.0/24, DHCP pool .100–.199. Sign-in page of the router's own firmware.</p></div>",
                        esc(&d.name),
                        esc(&sn)
                    ),
                )
            }
            _ => refused("Nothing listens there."),
        }
    }

    fn admin_page(&self, d: &Device) -> Page {
        let plate = &self.plate;
        let st = self.net.losos.boxes.get(&d.id);
        let edge_row = |e: &crate::net::Edge| {
            format!(
                "<div class=\"card\" style=\"display:flex;gap:8px;align-items:center\"><span style=\"color:{};font-weight:700\">{}</span><div><b>{}</b> <span class=\"muted\">{}</span><br><span class=\"muted\">{}{}</span></div></div>",
                if e.official { "#2e7357" } else { "#8c6512" },
                if e.official { "✓" } else { "⚠" },
                esc(&self.nm(&e.id)),
                esc(&e.url),
                if e.source == "lan" { "Found on this network" } else { "Configured address" },
                if e.official { " · official edge" } else { " · can share storage and compute, cannot trade on the market" },
            )
        };
        let edges = match st {
            Some(s) if !s.edges.is_empty() => s.edges.iter().map(edge_row).collect::<String>(),
            _ => "<div class=\"card\"><b>No edge proxy found.</b><br><span class=\"muted\">Tried mDNS <code>_losos-edge._tcp</code> on this network and the configured address. Sharing stays off.</span></div>".into(),
        };
        let path = match st.and_then(|s| s.path.as_ref()) {
            Some(p) => format!(
                "{}{}",
                esc(&self.nm(&p.id)),
                if p.source == "lan" {
                    " (local edge first)"
                } else {
                    " (official edge)"
                }
            ),
            None => "none".into(),
        };
        let public = match st.and_then(|s| s.public_name.as_ref()) {
            Some(n) => format!("<code>https://{}</code>", esc(n)),
            None => "<span class=\"muted\">none</span>".into(),
        };
        let mesh = st.map(|s| s.mesh).unwrap_or("off");
        let market = st.map(|s| s.market).unwrap_or("unavailable");
        let html = format!(
            "<div class=\"page\"><div class=\"pbar\"><img src=\"{plate}\" alt=\"LosOS\"><b>{}</b><span class=\"muted\">Home</span><span style=\"margin-left:auto\" class=\"muted\">Signed in on the LAN</span></div><div class=\"pbody\">\n    <h1>Apps</h1>\n    <div style=\"display:flex;gap:8px;flex-wrap:wrap\"><div class=\"card\" style=\"flex:1;min-width:120px\"><b>LosOS cloud</b><br><span class=\"muted\">/nextcloud</span></div><div class=\"card\" style=\"flex:1;min-width:120px\"><b>LosOS Git</b><br><span class=\"muted\">/forgejo</span></div></div>\n    <h1 style=\"margin-top:12px\">Mesh</h1>\n    {edges}\n    <div class=\"card\"><b>Path</b>: {path}<br><b>Public address</b>: {public}<br><b>Mesh</b>: {} · <b>Market</b>: {}</div>\n  </div></div>",
            esc(&d.name),
            esc(mesh),
            esc(market),
        );
        page(
            200,
            "LosOS",
            Some(format!("LosOS admin · {}", d.name)),
            html,
        )
    }

    fn cloud_page(&self, d: &Device, via_tunnel: bool) -> Page {
        let plate = &self.plate;
        page(
            200,
            "LosOS cloud",
            Some("<title>LosOS cloud</title>".into()),
            format!(
                "<div class=\"page\" style=\"background:linear-gradient(160deg,#0e6e7d,#0a4f5a);min-height:100%;color:#fff;padding:10px\"><div class=\"login\"><img src=\"{plate}\" alt=\"\" width=\"56\" height=\"56\"><h1 style=\"color:#fff\">LosOS cloud</h1><p style=\"opacity:.85;font-size:12px\">{}{}</p><input aria-label=\"Account name\" value=\"notshared\"><input aria-label=\"Password\" type=\"password\" value=\"••••••••••••\"><button class=\"go\" type=\"button\">Log in</button></div></div>",
                esc(&d.name),
                if via_tunnel { " · through the edge tunnel" } else { " · on this network" }
            ),
        )
    }

    fn git_page(&self, d: &Device) -> Page {
        let plate = &self.plate;
        page(
            200,
            "LosOS Git",
            Some("<title>LosOS Git</title>".into()),
            format!(
                "<div class=\"page\"><div class=\"pbar\"><img src=\"{plate}\" alt=\"\"><b>LosOS Git</b><span class=\"muted\">{}</span></div><div class=\"pbody\"><h1>Explore</h1><div class=\"card\"><b>notshared/losos-config</b> <span class=\"muted\">private</span><br><span class=\"muted\">Every apply, mode change and reset of this box is a commit here.</span></div></div></div>",
                esc(&d.name)
            ),
        )
    }
}

fn forbidden(d: &Device) -> Page {
    let n = esc(&d.name);
    page(
        403,
        "Forbidden",
        Some("403 Forbidden".into()),
        format!("<div class=\"page err\"><h1>403 Forbidden</h1><p>nginx</p><hr><p class=\"muted\">The admin pages of <b>{n}</b> answer on its own network only. Tunnel traffic reaches nginx from 127.0.0.1, and the <code>lanOnly</code> guard denies loopback on purpose, so nobody on the internet reaches the admin UI. Use <code>/nextcloud</code> from outside, or open <code>http://{n}.local/</code> from home.</p></div>"),
    )
}
