//! What the parity fixtures do not cover: refusals, hostile files, the
//! console on each kind of device (including the cases where the JS threw),
//! the clock in another zone, and the frame loop.

use losos_lab_core::LabCore;
use serde_json::{json, Value};

const T0: f64 = 1_791_453_600_000.0; // Thu 2026-10-08 10:00 UTC

fn lab(key: &str) -> LabCore {
    let mut l = LabCore::new(1.0, T0, 0.0);
    l.load_scenario(key);
    l
}

fn id_of(l: &LabCore, name: &str) -> String {
    l.snapshot()["world"]["devices"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|d| d["name"] == name)
        .and_then(|d| d["id"].as_str().map(str::to_string))
        .unwrap_or_else(|| panic!("no device {name}"))
}

fn error(v: &Value) -> &str {
    v["error"].as_str().unwrap_or("")
}

fn lines(v: &Value) -> Vec<String> {
    v["lines"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|l| l.as_str().map(str::to_string))
        .collect()
}

#[test]
fn connect_refusals() {
    let mut l = lab("empty");
    let add = |l: &mut LabCore, t: &str| {
        l.add_device(t, 100.0, 100.0, "")["device"]["id"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_default()
    };
    let sw = add(&mut l, "switch");
    let bx = add(&mut l, "box");
    let ap = add(&mut l, "ap");
    let lap = add(&mut l, "laptop");
    let lap2 = add(&mut l, "laptop");

    assert_eq!(
        error(&l.connect(&sw, &sw, "auto", None, None)),
        "Pick two different devices."
    );
    assert_eq!(
        error(&l.connect(&sw, "d999", "auto", None, None)),
        "Pick two different devices."
    );
    assert_eq!(
        error(&l.connect(&sw, &bx, "laser", None, None)),
        "There is no such kind of link."
    );
    assert_eq!(
        error(&l.connect(&sw, &lap, "wifi", None, None)),
        "A wireless link needs an access point at one end."
    );
    assert_eq!(
        error(&l.connect(&ap, &bx, "wifi", None, None)),
        "mattbox has no free wireless card."
    );
    assert_eq!(
        error(&l.connect(&sw, &bx, "auto", Some("p99"), None)),
        "switch has no Ethernet port p99."
    );
    assert_eq!(
        error(&l.connect(&sw, &bx, "auto", Some("p1"), Some("wlan0"))),
        "mattbox has no Ethernet port wlan0."
    );

    // the automatic kind puts a laptop on Wi-Fi when one end is an AP
    let r = l.connect(&ap, &lap, "auto", None, None);
    assert_eq!(r["link"]["kind"], json!("wifi"), "{r}");
    // as in the JS, the busy card is found before the "already on Wi-Fi" check
    assert_eq!(
        error(&l.connect(&ap, &lap, "wifi", None, None)),
        "matus-laptop has no free wireless card."
    );
    assert!(l
        .connect(&ap, &lap2, "wifi", None, None)
        .get("link")
        .is_some());

    let r = l.connect(&sw, &bx, "copper", Some("p1"), None);
    assert_eq!(r["link"]["a"], json!({ "dev": sw, "port": "p1" }));
    assert_eq!(
        error(&l.connect(&sw, &lap, "copper", Some("p1"), None)),
        "switch p1 is already in use."
    );
    assert_eq!(
        error(&l.connect(&bx, &sw, "copper", None, None)),
        "mattbox has no free Ethernet port."
    );
    for _ in 0..7 {
        let b = add(&mut l, "box");
        assert!(l.connect(&sw, &b, "auto", None, None).get("link").is_some());
    }
    let b = add(&mut l, "box");
    assert_eq!(
        error(&l.connect(&sw, &b, "auto", None, None)),
        "switch has no free Ethernet port."
    );

    let inet = add(&mut l, "internet");
    let r = l.connect(&inet, &b, "copper", None, None);
    assert_eq!(r["link"]["kind"], json!("wan"));
}

#[test]
fn hostile_files() {
    let mut l = lab("home");
    let before = l.snapshot()["world"].clone();
    assert_eq!(
        error(&l.import_setup("{", "x.llf")),
        "This is not a LosOS Lab setup file."
    );
    let huge = format!("\"{}\"", "x".repeat((1 << 20) + 1));
    assert_eq!(
        error(&l.import_setup(&huge, "x.llf")),
        "That file is too big to be a setup."
    );
    assert_eq!(
        error(&l.import_setup(
            r#"{"format":"losos-lab-setup","version":"<b>9</b>","devices":[],"links":[]}"#,
            "x"
        )),
        "This setup file is version <b>9</b>; this Lab reads version 1."
    );
    // a refused file leaves the canvas be
    assert_eq!(l.snapshot()["world"], before);

    let doc = json!({
        "format": "losos-lab-setup", "version": 1, "name": "<img src=x onerror=alert(1)>",
        "devices": [
            { "id": "a", "type": "__proto__" },
            { "id": "b", "type": ["box"] },
            { "id": "c", "type": "box", "name": "<script>alert(1)</script>", "cfg": { "proxy": "yes", "__proto__": { "x": 1 }, "joinMesh": true } },
            { "id": "d", "type": "switch", "x": "1e9", "px": 1e300 },
        ],
        "links": [
            { "a": { "dev": "c", "port": "eth0" }, "b": { "dev": "d", "port": "p1" }, "kind": ["auto"] },
            { "a": { "dev": "c", "port": "eth0" }, "b": { "dev": "zz", "port": "p2" } },
            { "a": { "dev": "c", "port": "eth1" }, "b": { "dev": "d", "port": "p2" } },
        ],
    });
    let r = l.import_setup(&doc.to_string(), "Evil.LLF");
    assert_eq!(r["skipped"], json!(4), "{r}");
    let s = l.snapshot();
    let devs = s["world"]["devices"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(devs.len(), 2);
    assert_eq!(devs[0]["name"], json!("script-alert-1-script"));
    assert_eq!(devs[0]["cfg"]["proxy"], json!(true));
    assert_eq!(devs[0]["cfg"]["joinMesh"], json!(true));
    assert!(devs[0]["cfg"].get("__proto__").is_none());
    assert_eq!(devs[1]["x"], json!(400));
    assert_eq!(devs[1]["px"], json!(2000));
    assert_eq!(s["world"]["links"][0]["kind"], json!("copper"));
    assert_eq!(s["fileName"], json!("<img src=x onerror=alert(1)>"));
    assert_eq!(s["scenario"], json!("file"));
    // the file name is only ever a label; the saved name goes through deviceName
    assert_eq!(
        l.export_setup()["filename"],
        json!("img-src-x-onerror-alert-1.llf")
    );

    // ten thousand devices: the caps hold
    let devices: Vec<Value> = (0..10000)
        .map(|i| json!({ "id": i, "type": "box" }))
        .collect();
    let links: Vec<Value> = (0..5000)
        .map(
            |i| json!({ "a": { "dev": i, "port": "eth0" }, "b": { "dev": i + 1, "port": "eth0" } }),
        )
        .collect();
    let big =
        json!({ "format": "losos-lab-setup", "version": 1, "devices": devices, "links": links })
            .to_string();
    let r = l.import_setup(&big, "big.json");
    let s = l.snapshot();
    assert_eq!(s["world"]["devices"].as_array().map(Vec::len), Some(200));
    assert!(s["world"]["links"].as_array().map_or(0, Vec::len) <= 400);
    // beyond the caps parts are dropped without being counted, as in the JS
    assert_eq!(
        r["skipped"],
        json!(400 - s["world"]["links"].as_array().map_or(0, Vec::len) as u64)
    );
    assert_eq!(s["fileName"], json!("big"));
}

#[test]
fn last_setup_naming() {
    let mut l = lab("home");
    assert_eq!(l.last_setup(), None);
    let bx = id_of(&l, "mattbox");
    l.set_cfg(&bx, "joinMesh", "true");
    let doc: Value =
        serde_json::from_str(&l.last_setup().unwrap_or_default()).unwrap_or(Value::Null);
    assert_eq!(doc["name"], json!("Edited Home: one box"));
    assert_eq!(l.last_setup(), None);
    let text = l.export_setup()["text"].as_str().unwrap_or("").to_string();
    assert_eq!(
        l.peek_setup(&doc.to_string()),
        json!({ "name": "Edited Home: one box" })
    );
    let r = l.open_last(&doc.to_string());
    assert_eq!(r["skipped"], json!(0));
    assert_eq!(l.snapshot()["scenario"], json!("last"));
    let r = l.open_last("not json");
    assert_eq!(r["fallback"], json!("two-sites"));
    assert_eq!(l.snapshot()["scenario"], json!("two-sites"));
    let r = l.import_setup(&text, "My Home.llf");
    assert_eq!(r["name"], json!("Home: one box"));
    let doc: Value =
        serde_json::from_str(&l.last_setup().unwrap_or_default()).unwrap_or(Value::Null);
    assert_eq!(doc["name"], json!("Home: one box"));
}

#[test]
fn editing_rules() {
    let mut l = lab("home");
    let bx = id_of(&l, "mattbox");
    let hr = id_of(&l, "home-router");
    assert_eq!(
        l.set_name(&bx, " Matt Box! "),
        json!({ "name": "matt-box-" })
    );
    assert_eq!(
        error(&l.set_name(&bx, "home-router")),
        "Pick a host name no other device uses."
    );
    assert_eq!(
        error(&l.set_name(&bx, "   ")),
        "Pick a host name no other device uses."
    );
    assert_eq!(
        error(&l.set_cfg(&bx, "proxy", "\"yes\"")),
        "proxy takes a different kind of value."
    );
    assert_eq!(
        error(&l.set_cfg(&bx, "zone", "\"x\"")),
        "A box has no setting zone."
    );
    assert_eq!(
        l.set_cfg(&hr, "subnet", "\" 10.9.9 \"")["cfg"]["subnet"],
        json!("10.9.9")
    );
    assert_eq!(l.snapshot()["net"]["addr"][&bx]["ip"], json!("10.9.9.100"));
    assert_eq!(
        error(&l.set_site(&bx, "moon")),
        "There is no such location."
    );
    assert_eq!(l.set_site(&bx, "office"), json!({ "ok": true }));
    assert_eq!(l.snapshot()["world"]["devices"][4]["px"], Value::Null);
    assert_eq!(l.phys_pos(&bx), json!({ "px": 60, "py": 250 }));
    assert_eq!(
        l.move_device(&bx, r#"{"x":1.5,"y":2,"px":null}"#),
        json!({ "ok": true })
    );
    assert_eq!(l.snapshot()["world"]["devices"][4]["x"], json!(1.5));
    assert_eq!(
        error(&l.move_device(&bx, r#"{"x":"left"}"#)),
        "A device needs a place on the canvas."
    );
    let r = l.set_power(&hr, false);
    assert_eq!(r, json!({ "power": false, "banner": [] }));
    assert_eq!(l.snapshot()["net"]["addr"].get(&bx), None);
    let r = l.set_power(&hr, true);
    assert!(r["banner"].as_array().is_some_and(|b| b.iter().any(|x| x
        .as_str()
        .is_some_and(|s| s.contains("Router home-router is ready")))));
    assert_eq!(
        error(&l.add_device("toaster", 0.0, 0.0, "")),
        "There is no such device type."
    );
    assert_eq!(
        error(&l.add_device("box", f64::NAN, 0.0, "")),
        "A device needs a place on the canvas."
    );
    let d = l.add_device(
        "edge-official",
        5.0,
        6.0,
        r#"{"site":"dc","px":10,"py":20}"#,
    );
    assert_eq!(d["device"]["name"], json!("edge2"));
    assert_eq!(d["device"]["site"], json!("dc"));
    assert_eq!(error(&l.remove_link("l999")), "There is no such link.");
}

#[test]
fn consoles_on_each_kind() {
    let mut l = lab("two-sites");
    let hr = id_of(&l, "home-router");
    let sw = id_of(&l, "office-switch");
    let bx = id_of(&l, "mattbox");
    let lap = id_of(&l, "matus-laptop");

    let r = l.exec(&hr, "show dhcp");
    assert_eq!(r["busy"], json!(false));
    let out = lines(&r);
    assert_eq!(
        out[0],
        "Mac Address       IP Address      Host Name           Expires in"
    );
    assert!(out
        .iter()
        .any(|x| x.contains("192.168.1.100") && x.contains("mattbox")));
    assert_eq!(
        lines(&l.exec(&sw, "show dhcp")),
        ["No DHCP server on a switch."]
    );
    assert!(lines(&l.exec(&sw, "show ip route"))[0].starts_with("default via 10.10.0.1 dev eth0"));
    assert!(lines(&l.exec(&sw, "show int"))[1].starts_with("eth0       up"));
    assert_eq!(lines(&l.exec(&bx, "hostname")), ["mattbox"]);
    let st: Value = serde_json::from_str(&lines(&l.exec(&bx, "losos-ctl status")).join("\n"))
        .unwrap_or(Value::Null);
    assert_eq!(st["tunnel"], json!("registered"));
    assert_eq!(st["publicName"], json!("mattbox.losos.cfd"));
    assert_eq!(
        lines(&l.exec(&lap, "losos-ctl")),
        ["losos-ctl: command not found (simulated shell; try help)"]
    );
    assert_eq!(l.exec(&bx, "clear")["clear"], json!(true));

    // where the JS threw, the Rust answers
    let route = lines(&l.exec(&sw, "ip route"));
    assert_eq!(
        route,
        [
            "default via 10.10.0.1 dev eth0 proto dhcp",
            "10.10.0.0/24 dev eth0 proto kernel scope link src 10.10.0.3"
        ]
    );
    assert_eq!(lines(&l.exec(&lap, "journalctl")), ["-- No entries --"]);
    assert_eq!(lines(&l.exec(&hr, "journalctl -u x")), ["-- No entries --"]);
    let r = l.exec(&sw, "avahi-browse -rt _losos-edge._tcp");
    assert_eq!(r["busy"], json!(true));
    let mut got = Vec::new();
    for _ in 0..50 {
        let t = l.step();
        for c in t["console"].as_array().into_iter().flatten() {
            if c["job"] == r["job"] {
                got.extend(lines(c));
            }
        }
    }
    assert_eq!(got[0], "= eth0 IPv4 acme-gw  _losos-edge._tcp  local");

    // ping prints its header at once, the replies when the packets are back
    let r = l.exec(&lap, "ping mattbox.local");
    assert_eq!(r["busy"], json!(true));
    assert_eq!(
        lines(&r),
        ["PING mattbox.local (192.168.1.100) 56(84) bytes of data."]
    );
    let mut done = false;
    let mut replies = Vec::new();
    for _ in 0..200 {
        let t = l.tick(250.0);
        for c in t["console"].as_array().into_iter().flatten() {
            if c["job"] == r["job"] {
                replies.extend(lines(c));
                done |= c["done"] == json!(true);
            }
        }
        if done {
            break;
        }
    }
    assert!(done);
    assert_eq!(replies.len(), 5);
    assert!(replies[0].starts_with("64 bytes from 192.168.1.100: icmp_seq=1 ttl=64 time=0."));

    let mut off = lab("home");
    let bx = id_of(&off, "mattbox");
    off.set_power(&bx, false);
    assert_eq!(
        lines(&off.exec(&bx, "hostname")),
        ["(device is powered off)"]
    );
    let info = off.console_info(&bx);
    assert_eq!(info["prompt"], json!("mattbox:~# "));
    assert_eq!(info["boot"][1], json!("\u{1b}[90m(powered off)\u{1b}[0m"));
}

#[test]
fn pages_and_errors() {
    let mut l = lab("two-sites");
    let lap = id_of(&l, "matus-laptop");
    let r = l.http(&lap, "not a url");
    assert_eq!(r["busy"], json!(false));
    assert_eq!(r["result"], json!({ "error": "badurl" }));
    assert!(l.error_page("nxdomain", "<x>").contains("&lt;x&gt;"));
    assert!(l.error_page("offline", "").contains("You are offline"));
    let urls = l.suggest_urls(&lap);
    assert_eq!(urls[0], json!("http://mattbox.local/"));
}

#[test]
fn clock_in_another_zone() {
    // UTC+2: the lab starts at 12:00 local
    let mut l = LabCore::new(1.0, T0, 120.0);
    l.load_scenario("office");
    let tb = id_of(&l, "teambox");
    l.set_cfg(&tb, "shareCompute", "true");
    l.clear();
    let text = |l: &LabCore| {
        l.snapshot()["clock"]["text"]
            .as_str()
            .unwrap_or("")
            .to_string()
    };
    assert_eq!(text(&l), "Thu 8 Oct 12:00:00");
    let want = [
        (
            "windowStart",
            "Thu 8 Oct 22:59:45",
            "23:00 compute window opens",
            Some(true),
        ),
        (
            "reboot",
            "Fri 9 Oct 00:06:45",
            "00:07 midnight-reboot.timer",
            Some(true),
        ),
        (
            "upgrade",
            "Fri 9 Oct 02:59:45",
            "03:00 nixos-upgrade.timer",
            Some(true),
        ),
        ("gc", "Fri 9 Oct 04:29:45", "04:30 nix-gc.timer", Some(true)),
        (
            "windowEnd",
            "Fri 9 Oct 06:59:45",
            "07:00 compute window closes",
            Some(false),
        ),
    ];
    for (key, at, toast, open) in want {
        let r = l.skip_to_next_timer();
        assert_eq!(r["next"]["key"], json!(key));
        assert_eq!(text(&l), at);
        l.advance_clock(20e3);
        let out = l.tick(0.0);
        assert!(
            out["toasts"]
                .as_array()
                .is_some_and(|t| t.contains(&json!(toast))),
            "{key}: {}",
            out["toasts"]
        );
        let dev = l.snapshot()["world"]["devices"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|d| d["id"] == json!(tb))
            .cloned()
            .unwrap_or(Value::Null);
        assert_eq!(dev["computeOpen"].as_bool(), open, "{key}");
        if key == "upgrade" {
            assert_eq!(dev["gen"], json!(42));
        }
        l.clear();
    }
    assert_eq!(
        error(&l.set_clock_speed(7.0)),
        "The clock runs at 1×, 10×, 60×, 10 min/s or 1 h/s."
    );
}

#[test]
fn frame_loop() {
    // realtime: packets move on their own and the loop goes idle
    let mut l = lab("home");
    let mut saw_packet = false;
    let mut events = 0;
    for _ in 0..2000 {
        let t = l.tick(16.0);
        if let Some(p) = t["packets"].as_array().and_then(|p| p.first()) {
            saw_packet = true;
            assert!(p["x"].is_number() && p["y"].is_number() && p["color"].is_string());
        }
        events += t["events"].as_array().map_or(0, Vec::len);
        if t["sim"]["inFlight"] == json!(0) {
            break;
        }
    }
    assert!(saw_packet && events > 20);
    assert_eq!(l.tick(16.0)["sim"]["inFlight"], json!(0));

    // simulation: nothing moves until Play or Step
    let mut l = lab("home");
    l.set_mode("simulation");
    let t = l.tick(5000.0);
    assert_eq!(t["sim"]["tick"], json!(0));
    let t = l.step();
    assert_eq!(t["sim"]["tick"], json!(1));
    assert!(t["packets"]
        .as_array()
        .is_some_and(|p| p.iter().all(|x| x["t"] == json!(0.0))));
    let t = l.tick(225.0);
    assert!(t["packets"]
        .as_array()
        .is_some_and(|p| p.iter().all(|x| x["t"] == json!(0.5))));
    let t = l.tick(1000.0);
    assert!(t["packets"]
        .as_array()
        .is_some_and(|p| p.iter().all(|x| x["t"] == json!(1.0))));
    l.set_sim_speed(4.0);
    l.set_playing(true);
    let mut n = 0;
    while l.snapshot()["sim"]["playing"] == json!(true) && n < 1000 {
        l.tick(100.0);
        n += 1;
    }
    assert_eq!(
        l.snapshot()["sim"]["tick"].as_u64().map(|t| t > 10),
        Some(true)
    );
    assert!(l.step()["message"]
        .as_str()
        .is_some_and(|m| m.starts_with("Nothing queued")));

    // the protocol filter hides packets, never events
    l.set_filter("DHCP", false);
    assert_eq!(
        error(&l.set_filter("SMTP", false)),
        "There is no such protocol."
    );
}

#[test]
fn guest_frames_go_in_as_flows() {
    let mut l = lab("home");
    l.set_mode("simulation");
    l.clear();
    let bx = id_of(&l, "mattbox");
    let hr = id_of(&l, "home-router");
    let stages =
        json!([[{ "proto": "ARP", "from": bx, "to": hr, "info": "who-has", "real": true }]])
            .to_string();
    assert_eq!(l.start_flow_json(&stages, "guest"), json!({ "ok": true }));
    let t = l.step();
    assert_eq!(t["events"][0]["real"], json!(true));
    assert_eq!(t["packets"][0]["real"], json!(true));
    assert_eq!(
        error(&l.start_flow_json(
            r#"[[{"proto":"SMTP","from":"d1","to":"d2","info":"x"}]]"#,
            ""
        )),
        "There is no protocol SMTP."
    );
    assert_eq!(
        error(&l.start_flow_json(
            r#"[[{"proto":"ARP","from":"d1","to":"d99","info":"x"}]]"#,
            ""
        )),
        "A packet goes between two devices of this setup."
    );
    assert_eq!(
        error(&l.start_flow_json("{}", "")),
        "The stages are not readable."
    );
}
