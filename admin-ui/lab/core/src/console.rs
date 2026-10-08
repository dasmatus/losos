//! The simulated console: boot text, banners, completions and the command
//! interpreter (term.js without the terminal plumbing).

use crate::catalog::{is_gear, ports_of};
use crate::js::pad_end;
use crate::lab::LabCore;
use crate::world::Device;
use serde_json::json;

/// What `exec` hands back straight away.
pub struct ExecOut {
    pub lines: Vec<String>,
    pub busy: bool,
    pub clear: bool,
}

fn lines(s: &str) -> Vec<String> {
    s.split('\n').map(str::to_string).collect()
}

fn row(s: &str) -> String {
    format!("\x1b[44;97m {} \x1b[0m", pad_end(s, 58))
}

impl LabCore {
    pub(crate) fn completions(&self, d: &Device) -> Vec<&'static str> {
        let base = [
            "help", "ip addr", "ip route", "ping ", "curl ", "clear", "hostname",
        ];
        if is_gear(&d.kind) {
            return vec![
                "help",
                "show interfaces",
                "show ip route",
                "show arp",
                "show dhcp",
                "show version",
                "ping ",
                "clear",
            ];
        }
        let extra: &[&str] = match d.kind.as_str() {
            "laptop" => &[
                "avahi-resolve -n ",
                "avahi-browse -rt _losos-edge._tcp",
                "resolvectl query ",
            ],
            "box" => &[
                "losos-ctl status",
                "losos-ctl edge",
                "journalctl -u losos-rathole-client",
                "avahi-browse -rt _losos-edge._tcp",
            ],
            "edge-local" => &[
                "losos-edge boxes",
                "journalctl -u losos-registrar",
                "journalctl -u losos-rathole-uplink",
            ],
            _ => &["journalctl -u losos-registrar", "losos-registrar tenants"],
        };
        base.iter().chain(extra.iter()).copied().collect()
    }

    pub(crate) fn prompt(&self, d: &Device) -> String {
        if is_gear(&d.kind) {
            format!("{}# ", d.name)
        } else {
            format!("{}:~# ", d.name)
        }
    }

    pub(crate) fn banner(&self, d: &Device) -> Vec<String> {
        let ip = self.net.ip(&d.id);
        if is_gear(&d.kind) {
            let m = self.net.mgmt.get(&d.id);
            let what = match d.kind.as_str() {
                "router" => "Router",
                "switch" => "Switch",
                "ap" => "Wi-Fi access point",
                _ => "undefined",
            };
            let mut v = vec![
                String::new(),
                row(""),
                row("Netzgeräte Betriebssystem 1.0"),
                row(""),
                row(&format!("{what} {} is ready", d.name)),
            ];
            match m {
                Some(m) => {
                    v.push(row(&format!("  Address:  {}/{}", m.ip, m.mask)));
                    v.push(row(&format!("  Status:   http://{}", m.ip)));
                }
                None => v.push(row("  No address configured (losos.ip).")),
            }
            v.push(row(""));
            v.push(String::new());
            v.push("Maintenance console. Try: show interfaces | show ip route | show arp".into());
            v.push("                          show dhcp | show version".into());
            return v;
        }
        let title = match d.kind.as_str() {
            "box" => "LosOS is ready".to_string(),
            "edge-local" => format!("LosOS edge gateway ({}) is ready", d.name),
            _ => format!("LosOS official edge ({}) is ready", d.name),
        };
        let mut v = vec![String::new(), row(""), row(&title), row("")];
        match ip {
            Some(ip) => {
                v.push(row(
                    "On any computer on this network, open a web browser at:",
                ));
                v.push(row(&format!("  http://{ip}")));
                v.push(row("or, on computers that find the box by name:"));
                v.push(row(&format!("  http://{}.local", d.name)));
            }
            None => v.push(row("No network address yet. Plug in an Ethernet cable.")),
        }
        v.push(row(""));
        v
    }

    pub(crate) fn boot_text(&self, d: &Device) -> Vec<String> {
        if d.kind == "laptop" {
            return vec![
                "\x1b[90mLosOS Desktop · derisk · Terminal\x1b[0m".into(),
                "Type \x1b[1mhelp\x1b[0m for the commands this simulated shell knows.".into(),
            ];
        }
        let mut v = vec![
            "\x1b[90m[sim] Simulated console. Commands answer from the simulator's model, not from a running guest.\x1b[0m"
                .to_string(),
        ];
        if !d.power {
            v.push("\x1b[90m(powered off)\x1b[0m".into());
            return v;
        }
        v.extend(self.banner(d));
        v.push(String::new());
        v
    }

    fn ip_addr(&self, d: &Device) -> String {
        let a = self.net.addr.get(&d.id);
        let f = self.net.iface.get(&d.id);
        let mut out = vec![
            "1: lo: <LOOPBACK,UP,LOWER_UP> mtu 65536".to_string(),
            "    inet 127.0.0.1/8 scope host lo".to_string(),
        ];
        for (i, (p, _)) in ports_of(&d.kind).iter().enumerate() {
            let up = f.is_some_and(|f| f.port == *p);
            out.push(format!(
                "{}: {p}: <BROADCAST,MULTICAST{}> mtu 1500 state {}",
                i + 2,
                if up { ",UP,LOWER_UP" } else { ",NO-CARRIER" },
                if up { "UP" } else { "DOWN" }
            ));
            out.push(format!("    link/ether {}", d.mac));
            if let (true, Some(a)) = (up, a) {
                out.push(format!(
                    "    inet {}/{} {}scope global {p}",
                    a.ip,
                    a.mask,
                    if a.src == "dhcp" { "dynamic " } else { "" }
                ));
            }
        }
        out.join("\n")
    }

    fn journal(&self, d: &Device) -> String {
        let ts = self.clock_hms();
        let n = &d.name;
        match d.kind.as_str() {
            "box" => {
                let Some(st) = self.net.losos.boxes.get(&d.id) else {
                    return "-- No entries --".into();
                };
                let Some(path) = &st.path else {
                    return format!("{ts} {n} systemd[1]: losos-rathole-client.service: skipped, ConditionPathExists=!/run/losos/edge-none was not met");
                };
                let e = self.nm(&path.id);
                let last = if st.tunnel == "refused" {
                    format!(
                        "{ts} {n} losos-registrar-announce[815]: register refused: {}",
                        st.reason
                    )
                } else {
                    format!(
                        "{ts} {n} losos-registrar-announce[815]: registered ({}){}",
                        st.tunnel,
                        st.public_name
                            .as_ref()
                            .map(|p| format!(" as {p}"))
                            .unwrap_or_default()
                    )
                };
                [
                    format!("{ts} {n} losos-rathole-client[812]: pinned Noise key for {e}"),
                    format!("{ts} {n} losos-rathole-client[812]: control channel established to {e}:2333"),
                    last,
                ]
                .join("\n")
            }
            "edge-local" => {
                let Some(sp) = self.net.losos.spoke.get(&d.id) else {
                    return "-- No entries --".into();
                };
                let mut v: Vec<String> = sp
                    .enrolled
                    .iter()
                    .map(|b| {
                        format!(
                            "{ts} {n} losos-registrar[402]: enrolled {} (trust on first use)",
                            self.nm(b)
                        )
                    })
                    .collect();
                v.push(format!(
                    "{ts} {n} losos-registrar[402]: uplink {}{}",
                    sp.uplink,
                    if sp.uplink == "up" {
                        format!(
                            ", relayed {} box(es) under {}",
                            sp.relayed.len(),
                            d.text("zone")
                        )
                    } else {
                        String::new()
                    }
                ));
                v.join("\n")
            }
            "edge-official" => {
                let Some(h) = self.net.losos.hub.get(&d.id) else {
                    return "-- No entries --".into();
                };
                let mut v = vec![format!(
                    "{ts} {n} losos-registrar[388]: {} tenant(s), {} relayed box(es)",
                    h.tenants.len(),
                    h.relays.len()
                )];
                for b in &h.relays {
                    let pn = self
                        .net
                        .losos
                        .boxes
                        .get(b)
                        .and_then(|s| s.public_name.clone())
                        .unwrap_or_else(|| "null".into());
                    v.push(format!(
                        "{ts} {n} losos-registrar[388]: route {pn} via spoke"
                    ));
                }
                v.join("\n")
            }
            // the JS throws here (no hub entry for a laptop or gear)
            _ => "-- No entries --".into(),
        }
    }

    fn gear_show(&self, d: &Device, args: &[&str]) -> String {
        let m = self.net.mgmt.get(&d.id);
        let what = args.first().copied().unwrap_or("");
        if what.starts_with("int") {
            let mut rows: Vec<[String; 4]> = vec![[
                "Interface".into(),
                "State".into(),
                "MAC".into(),
                "Address".into(),
            ]];
            rows.push([
                "eth0".into(),
                if m.is_some() { "up" } else { "down" }.into(),
                d.mac.clone(),
                m.map(|m| format!("{}/{}", m.ip, m.mask))
                    .unwrap_or_else(|| "-".into()),
            ]);
            for (p, _) in ports_of(&d.kind) {
                let l = self.world.links.iter().find(|l| l.at(&d.id, p));
                rows.push([
                    format!("  {p}"),
                    if l.is_some_and(|l| self.world.link_up(l)) {
                        "up"
                    } else {
                        "down"
                    }
                    .into(),
                    String::new(),
                    l.map(|l| format!("→ {}", self.nm(&l.other_end(&d.id).dev)))
                        .unwrap_or_default(),
                ]);
            }
            return rows
                .iter()
                .map(|r| {
                    format!(
                        "{} {} {} {}",
                        pad_end(&r[0], 10),
                        pad_end(&r[1], 8),
                        pad_end(&r[2], 18),
                        r[3]
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
        }
        if what == "ip" {
            let Some(m) = m else { return String::new() };
            let prefix: Vec<&str> = m.ip.split('.').take(3).collect();
            return format!(
                "{}{}.0/{} dev eth0 scope link  src {}",
                m.gw.as_ref()
                    .map(|g| format!("default via {g} dev eth0\n"))
                    .unwrap_or_default(),
                prefix.join("."),
                m.mask,
                m.ip
            );
        }
        if what == "arp" {
            let Some(s) = self
                .net
                .gear_seg
                .get(&d.id)
                .and_then(|k| self.net.segs.get(k))
            else {
                return String::new();
            };
            return s
                .members
                .iter()
                .filter(|id| self.net.addr.contains(id))
                .map(|id| {
                    format!(
                        "? ({}) at {} [ether]  on eth0",
                        self.net.ip(id).unwrap_or(""),
                        self.world.dev(id).map(|x| x.mac.as_str()).unwrap_or("")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
        }
        if what == "dhcp" {
            if d.kind != "router" {
                return format!("No DHCP server on a {}.", d.kind);
            }
            let sn = self
                .net
                .subnet
                .get(&d.id)
                .cloned()
                .unwrap_or_else(|| "undefined".into());
            let leases: Vec<(&String, &crate::net::Addr)> = self
                .net
                .addr
                .iter()
                .filter(|(_, v)| v.router.as_deref() == Some(d.id.as_str()) && v.src == "dhcp")
                .collect();
            if leases.is_empty() {
                return format!("No leases yet (pool {sn}.100-{sn}.199).");
            }
            let mut v = vec![
                "Mac Address       IP Address      Host Name           Expires in".to_string(),
            ];
            for (id, a) in leases {
                let mac = self
                    .world
                    .dev(id)
                    .map(|x| x.mac.clone())
                    .unwrap_or_default();
                v.push(format!(
                    "{} {} {} 23:59:12",
                    pad_end(&mac, 17),
                    pad_end(&a.ip, 15),
                    pad_end(&self.nm(id), 19)
                ));
            }
            return v.join("\n");
        }
        if what.starts_with("ver") {
            return format!("NAME=\"Netzgeräte Betriebssystem\"\nPRETTY_NAME=\"Netzgeräte Betriebssystem 1.0\"\nID=netzgeraete\nVERSION_ID=1.0\nLinux {} 6.1.0 #1 SMP PREEMPT_DYNAMIC x86_64 GNU/Linux", d.name);
        }
        "usage: show interfaces | ip route | arp | dhcp | version".into()
    }

    /// `runCommand`: what a line typed on a device prints. Commands that
    /// send packets (`ping`, `curl`, `avahi-browse`) finish over later ticks.
    pub(crate) fn run_command(&mut self, id: &str, line: &str, job: u64) -> ExecOut {
        let mut out = ExecOut {
            lines: vec![],
            busy: false,
            clear: false,
        };
        let say = |out: &mut ExecOut, s: &str| out.lines.extend(lines(s));
        let Some(d) = self.world.dev(id).cloned().filter(|d| d.power) else {
            say(&mut out, "(device is powered off)");
            return out;
        };
        let line = line.trim();
        let mut words = line.split_whitespace();
        let cmd = words.next().unwrap_or("");
        let args: Vec<&str> = words.collect();
        let gear = is_gear(&d.kind);
        let a = if gear {
            self.net
                .mgmt
                .get(id)
                .map(|m| (m.ip.clone(), m.mask, m.gw.clone()))
        } else {
            self.net
                .addr
                .get(id)
                .map(|a| (a.ip.clone(), a.mask, a.gw.clone()))
        };
        if gear && cmd == "show" {
            say(&mut out, &self.gear_show(&d, &args));
            return out;
        }
        match cmd {
            "" => return out,
            "help" => {
                let c: Vec<String> = self
                    .completions(&d)
                    .iter()
                    .map(|c| format!("  {}", c.trim()))
                    .collect();
                out.lines.extend(c);
                return out;
            }
            "clear" => {
                out.clear = true;
                return out;
            }
            "hostname" => {
                say(&mut out, &d.name);
                return out;
            }
            "uname" => {
                say(
                    &mut out,
                    &format!(
                        "Linux {} 6.12.51 #1-NixOS SMP PREEMPT_DYNAMIC x86_64 GNU/Linux",
                        d.name
                    ),
                );
                return out;
            }
            "ip" => {
                if matches!(args.first(), Some(&"route") | Some(&"r")) {
                    let s = match &a {
                        Some((ip, mask, gw)) => {
                            // the JS reads the endpoint's interface; gear without one threw
                            let port = self
                                .net
                                .iface
                                .get(id)
                                .map(|f| f.port.clone())
                                .unwrap_or_else(|| "eth0".into());
                            let prefix: Vec<&str> = ip.split('.').take(3).collect();
                            format!(
                                "{}{}.0/{mask} dev {port} proto kernel scope link src {ip}",
                                match gw.as_deref() {
                                    Some(g) if !g.is_empty() =>
                                        format!("default via {g} dev {port} proto dhcp\n"),
                                    _ => String::new(),
                                },
                                prefix.join(".")
                            )
                        }
                        None => "no routes".into(),
                    };
                    say(&mut out, &s);
                    return out;
                }
                say(&mut out, &self.ip_addr(&d));
                return out;
            }
            "ping" => {
                let host = args
                    .iter()
                    .find(|x| !x.starts_with('-') && !x.bytes().all(|b| b.is_ascii_digit()))
                    .copied();
                let Some(host) = host else {
                    say(&mut out, "usage: ping <host>");
                    return out;
                };
                if a.is_none() {
                    say(&mut out, "ping: connect: Network is unreachable");
                    return out;
                }
                self.ping_flow(id, host, 3, job);
                out.busy = true;
                return out;
            }
            "curl" => {
                let Some(url) = args.iter().find(|x| !x.starts_with('-')).copied() else {
                    say(&mut out, "usage: curl <url>");
                    return out;
                };
                if a.is_none() {
                    say(&mut out, "curl: (7) Network is unreachable");
                    return out;
                }
                self.http_flow(id, url, job, true);
                out.busy = true;
                return out;
            }
            "avahi-resolve" | "resolvectl" => {
                let host = args.last().copied();
                let r = crate::net::resolve_name(&self.world, &self.net, id, host.unwrap_or(""));
                let host = host.unwrap_or("undefined");
                let s = match r {
                    crate::net::Resolved::Error(e) => format!(
                        "Failed to resolve host name '{host}': {}",
                        if e == "mdns" {
                            "Timeout reached"
                        } else {
                            "Name not found"
                        }
                    ),
                    crate::net::Resolved::To { dev, via, hub, .. } => {
                        let t = if via == "tunnel" {
                            hub.unwrap_or_default()
                        } else {
                            dev
                        };
                        format!("{host}\t{}", self.net.ip(&t).unwrap_or("undefined"))
                    }
                };
                say(&mut out, &s);
                return out;
            }
            "avahi-browse" => {
                self.avahi_browse(id, job);
                out.busy = true;
                return out;
            }
            "losos-ctl" if d.kind == "box" => {
                let st = self.net.losos.boxes.get(id).cloned();
                let path = st.as_ref().and_then(|s| s.path.as_ref());
                let v = if args.first() == Some(&"edge") {
                    json!({
                        "path": path.map(|p| json!({ "name": self.nm(&p.id), "url": p.url, "source": p.source })),
                        "edges": st.as_ref().map(|s| s.edges.iter().map(|e| json!({ "name": self.nm(&e.id), "url": e.url, "official": e.official })).collect::<Vec<_>>()).unwrap_or_default(),
                    })
                } else {
                    json!({
                        "hostName": d.name,
                        "mode": if d.flag("joinMesh") { "mesh" } else { "local" },
                        "address": a.as_ref().map(|a| a.0.clone()),
                        "internet": self.net.has_internet(id),
                        "edge": path.map(|p| self.nm(&p.id)),
                        "tunnel": st.as_ref().map(|s| s.tunnel),
                        "publicName": st.as_ref().and_then(|s| s.public_name.clone()),
                        "mesh": st.as_ref().map(|s| s.mesh),
                        "market": st.as_ref().map(|s| s.market),
                    })
                };
                say(
                    &mut out,
                    &serde_json::to_string_pretty(&v).unwrap_or_default(),
                );
                return out;
            }
            "losos-edge" if d.kind == "edge-local" => {
                let s = match self.net.losos.spoke.get(id) {
                    Some(sp) if !sp.enrolled.is_empty() => sp
                        .enrolled
                        .iter()
                        .map(|b| {
                            format!(
                                "{}\t{}\t{}",
                                self.nm(b),
                                self.net.ip(b).unwrap_or("undefined"),
                                self.net
                                    .losos
                                    .boxes
                                    .get(b)
                                    .and_then(|s| s.public_name.clone())
                                    .unwrap_or_else(|| "LAN only".into())
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                    _ => "(no enrolled boxes)".into(),
                };
                say(&mut out, &s);
                return out;
            }
            "losos-registrar" if d.kind == "edge-official" => {
                let Some(h) = self.net.losos.hub.get(id) else {
                    return out;
                };
                let tenants: Vec<String> = h.tenants.iter().map(|b| self.nm(b)).collect();
                let relayed: Vec<String> = h
                    .relays
                    .iter()
                    .map(|b| {
                        self.net
                            .losos
                            .boxes
                            .get(b)
                            .and_then(|s| s.public_name.clone())
                            .unwrap_or_default()
                    })
                    .collect();
                let dash = |v: Vec<String>| {
                    if v.is_empty() {
                        "—".to_string()
                    } else {
                        v.join(", ")
                    }
                };
                say(
                    &mut out,
                    &format!("tenants: {}\nrelayed: {}", dash(tenants), dash(relayed)),
                );
                return out;
            }
            "journalctl" => {
                say(&mut out, &self.journal(&d));
                return out;
            }
            _ => {}
        }
        say(
            &mut out,
            &format!("{cmd}: command not found (simulated shell; try help)"),
        );
        out
    }
}
