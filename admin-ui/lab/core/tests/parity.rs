//! The Rust Lab against what the JavaScript Lab computed for the same steps.
//! The fixtures come from running the JavaScript Lab (admin-ui/lab/src, removed
//! once this port replaced it; it is in the git history) under node (TZ=UTC, clock
//! at 2026-10-08 10:00 UTC); random parts (MAC suffixes, nonces, ping times)
//! and journal timestamps are masked on both sides.

use losos_lab_core::LabCore;
use serde_json::{json, Value};
use std::path::PathBuf;

const T0: f64 = 1_791_453_600_000.0; // 2026-10-08T10:00:00Z

fn fixture(name: &str) -> Value {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn is_hex(c: char) -> bool {
    c.is_ascii_hexdigit() && !c.is_ascii_uppercase()
}

/// The harness's `norm`, on one string.
fn norm_str(s: &str) -> String {
    let mut out = String::new();
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    let starts = |i: usize, p: &str| {
        cs[i..]
            .iter()
            .take(p.chars().count())
            .copied()
            .eq(p.chars())
    };
    let mut line_start = true;
    while i < cs.len() {
        if line_start
            && i + 9 <= cs.len()
            && cs[i].is_ascii_digit()
            && cs[i + 1].is_ascii_digit()
            && cs[i + 2] == ':'
            && cs[i + 3].is_ascii_digit()
            && cs[i + 4].is_ascii_digit()
            && cs[i + 5] == ':'
            && cs[i + 6].is_ascii_digit()
            && cs[i + 7].is_ascii_digit()
            && cs[i + 8] == ' '
        {
            out.push_str("HH:MM:SS ");
            i += 9;
            line_start = false;
            continue;
        }
        line_start = false;
        if starts(i, "52:54:00:4c:") {
            let mut j = i + 12;
            let k = j;
            while j < cs.len() && is_hex(cs[j]) {
                j += 1;
            }
            if j - k >= 2 && j < cs.len() && cs[j] == ':' && j + 1 < cs.len() && is_hex(cs[j + 1]) {
                out.extend(&cs[i..=j]);
                out.push_str("xx");
                j += 1;
                while j < cs.len() && is_hex(cs[j]) {
                    j += 1;
                }
                i = j;
                continue;
            }
        }
        if starts(i, "nonce=") && i + 6 < cs.len() && is_hex(cs[i + 6]) {
            out.push_str("nonce=N");
            let mut j = i + 6;
            while j < cs.len() && is_hex(cs[j]) {
                j += 1;
            }
            i = j;
            continue;
        }
        if starts(i, "time=") {
            let mut j = i + 5;
            while j < cs.len() && (cs[j].is_ascii_digit() || cs[j] == '.') {
                j += 1;
            }
            if j > i + 5 && starts(j, " ms") {
                out.push_str("time=T ms");
                i = j + 3;
                continue;
            }
        }
        if cs[i] == '\n' {
            line_start = true;
        }
        out.push(cs[i]);
        i += 1;
    }
    out
}

fn norm(v: &Value) -> Value {
    match v {
        Value::String(s) => Value::String(norm_str(s)),
        Value::Array(a) => Value::Array(a.iter().map(norm).collect()),
        Value::Object(m) => Value::Object(m.iter().map(|(k, v)| (k.clone(), norm(v))).collect()),
        other => other.clone(),
    }
}

/// Compares two JSON values and names the first path where they differ.
fn diff(path: &str, a: &Value, b: &Value) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for k in x.keys().chain(y.keys()) {
                if let Some(d) = diff(
                    &format!("{path}.{k}"),
                    x.get(k).unwrap_or(&Value::Null),
                    y.get(k).unwrap_or(&Value::Null),
                ) {
                    return Some(d);
                }
                if x.contains_key(k) != y.contains_key(k) {
                    return Some(format!("{path}.{k}: present on one side only"));
                }
            }
            None
        }
        (Value::Array(x), Value::Array(y)) => {
            for (i, (p, q)) in x.iter().zip(y.iter()).enumerate() {
                if let Some(d) = diff(&format!("{path}[{i}]"), p, q) {
                    return Some(d);
                }
            }
            (x.len() != y.len()).then(|| format!("{path}: {} items vs {}", x.len(), y.len()))
        }
        (Value::Number(x), Value::Number(y)) if x.as_f64() == y.as_f64() => None,
        _ => (a != b).then(|| format!("{path}: rust {a} vs js {b}")),
    }
}

#[track_caller]
fn same(what: &str, rust: &Value, js: &Value) {
    if let Some(d) = diff(what, &norm(rust), &norm(js)) {
        panic!("{d}");
    }
}

fn parse(s: impl AsRef<str>) -> Value {
    serde_json::from_str(s.as_ref()).unwrap_or_else(|e| panic!("not JSON ({e}): {}", s.as_ref()))
}

fn snap(lab: &LabCore) -> Value {
    lab.snapshot()
}

fn idle(lab: &LabCore) -> bool {
    let s = snap(lab);
    s["sim"]["flows"].as_array().is_some_and(Vec::is_empty)
        && s["sim"]["active"].as_array().is_some_and(Vec::is_empty)
}

fn fresh(key: &str) -> LabCore {
    let mut lab = LabCore::new(7.0, T0, 0.0);
    assert_eq!(lab.set_mode("simulation"), json!({ "ok": true }));
    let r = lab.load_scenario(key);
    assert!(r.get("error").is_none(), "{r}");
    lab
}

/// Steps until a console job is done; answers its lines and the step count.
fn finish_exec(lab: &mut LabCore, dev: &str, line: &str) -> (Vec<Value>, u64, bool) {
    let r = lab.exec(dev, line);
    let job = r["job"].clone();
    let mut lines: Vec<Value> = r["lines"].as_array().cloned().unwrap_or_default();
    let mut busy = r["busy"] == json!(true);
    let mut steps = 0;
    while busy && steps < 2000 {
        let t = lab.step();
        steps += 1;
        for c in t["console"].as_array().into_iter().flatten() {
            if c["job"] == job {
                lines.extend(c["lines"].as_array().cloned().unwrap_or_default());
                if c["done"] == json!(true) {
                    busy = false;
                }
            }
        }
    }
    (lines, steps, r["clear"] == json!(true))
}

fn finish_http(lab: &mut LabCore, dev: &str, url: &str) -> (Value, u64) {
    let r = lab.http(dev, url);
    if r["busy"] == json!(false) {
        return (r["result"].clone(), 0);
    }
    let job = r["job"].clone();
    let mut steps = 0;
    while steps < 2000 {
        let t = lab.step();
        steps += 1;
        for h in t["http"].as_array().into_iter().flatten() {
            if h["job"] == job {
                return (h["result"].clone(), steps);
            }
        }
    }
    (Value::Null, steps)
}

fn flow_titles(lab: &LabCore) -> Value {
    Value::Array(
        snap(lab)["sim"]["flows"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|f| f["title"].clone())
            .collect(),
    )
}

fn check_scenario(key: &str) {
    let fx = fixture(&format!("scenario-{key}.json"));
    let mut lab = fresh(key);
    let s = snap(&lab);
    same(&format!("{key} focus"), &lab_focus(key), &fx["focus"]);
    same(&format!("{key} world"), &s["world"], &fx["world"]);
    same(&format!("{key} net"), &s["net"], &fx["net"]);
    let boot: Vec<Value> = s["sim"]["flows"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|f| json!({ "id": f["id"], "title": f["title"], "stages": f["stages"].as_array().map_or(0, Vec::len) }))
        .collect();
    same(
        &format!("{key} boot flows"),
        &Value::Array(boot),
        &fx["bootFlows"],
    );

    let mut steps = 0;
    while !idle(&lab) && steps < 600 {
        lab.step();
        steps += 1;
    }
    let s = snap(&lab);
    same(
        &format!("{key} boot steps"),
        &json!(steps),
        &fx["bootSteps"],
    );
    same(&format!("{key} log"), &s["sim"]["log"], &fx["log"]);
    same(&format!("{key} tick"), &s["sim"]["tick"], &fx["tick"]);
    same(&format!("{key} clock"), &s["clock"]["t"], &fx["clock"]);
    let doc = parse(lab.export_setup()["text"].as_str().unwrap_or(""));
    same(&format!("{key} setup"), &doc, &fx["setup"]);

    lab.clear();
    for c in fx["console"].as_array().into_iter().flatten() {
        let dev = c["dev"].as_str().unwrap_or("");
        let line = c["line"].as_str().unwrap_or("");
        let (lines, steps, _) = finish_exec(&mut lab, dev, line);
        same(
            &format!("{key} `{line}` on {dev}"),
            &Value::Array(lines),
            &c["lines"],
        );
        same(
            &format!("{key} `{line}` on {dev} steps"),
            &json!(steps),
            &c["steps"],
        );
    }
    lab.clear();
    for p in fx["pages"].as_array().into_iter().flatten() {
        let dev = p["dev"].as_str().unwrap_or("");
        let url = p["url"].as_str().unwrap_or("");
        let (res, steps) = finish_http(&mut lab, dev, url);
        same(&format!("{key} GET {url} from {dev}"), &res, &p["res"]);
        same(
            &format!("{key} GET {url} from {dev} steps"),
            &json!(steps),
            &p["steps"],
        );
    }
    for v in fx["variants"].as_array().into_iter().flatten() {
        let a = &v["action"];
        let mut lab = fresh(key);
        lab.clear();
        let dev = a["dev"].as_str().unwrap_or("");
        let r = match a["op"].as_str() {
            Some("power") => {
                let on = snap(&lab)["world"]["devices"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|d| d["id"] == a["dev"])
                    .map(|d| d["power"] == json!(true))
                    .unwrap_or(false);
                lab.set_power(dev, !on)
            }
            Some("cfg") => lab.set_cfg(
                dev,
                a["key"].as_str().unwrap_or(""),
                &a["value"].to_string(),
            ),
            Some("unlink") => lab.remove_link(a["link"].as_str().unwrap_or("")),
            other => panic!("unknown variant {other:?}"),
        };
        assert!(r.get("error").is_none(), "{key} {a}: {r}");
        same(
            &format!("{key} variant {a} net"),
            &snap(&lab)["net"],
            &v["net"],
        );
        same(
            &format!("{key} variant {a} flows"),
            &flow_titles(&lab),
            &v["flows"],
        );
    }
}

fn lab_focus(key: &str) -> Value {
    let mut lab = LabCore::new(7.0, T0, 0.0);
    lab.load_scenario(key)["focus"].clone()
}

#[test]
fn scenario_two_sites() {
    check_scenario("two-sites");
}
#[test]
fn scenario_home() {
    check_scenario("home");
}
#[test]
fn scenario_office() {
    check_scenario("office");
}
#[test]
fn scenario_star() {
    check_scenario("star");
}
#[test]
fn scenario_bus() {
    check_scenario("bus");
}
#[test]
fn scenario_web() {
    check_scenario("web");
}
#[test]
fn scenario_lan() {
    check_scenario("lan");
}
#[test]
fn scenario_empty() {
    check_scenario("empty");
}

#[test]
fn this_box_matches() {
    for (i, c) in fixture("this-box.json")
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let mut lab = LabCore::new(7.0, T0, 0.0);
        let r = lab.load_this_box(
            &c["input"]["settings"].to_string(),
            &c["input"]["edge"].to_string(),
        );
        assert!(r.get("error").is_none(), "{r}");
        same(&format!("this-box {i} name"), &r["name"], &c["name"]);
        same(&format!("this-box {i} blurb"), &r["blurb"], &c["blurb"]);
        same(&format!("this-box {i} focus"), &r["focus"], &c["focus"]);
        let s = snap(&lab);
        same(&format!("this-box {i} world"), &s["world"], &c["world"]);
        same(&format!("this-box {i} net"), &s["net"], &c["net"]);
        same(
            &format!("this-box {i} setup"),
            &parse(lab.export_setup()["text"].as_str().unwrap_or("")),
            &c["setup"],
        );
        let cat = lab.catalog();
        assert_eq!(cat["scenarios"][0]["key"], json!("this-box"));
    }
}

fn big_doc() -> String {
    let devices: Vec<Value> = (0..10000)
        .map(|i| json!({ "id": format!("n{i}"), "type": if i % 2 == 1 { "box" } else { "switch" }, "name": format!("n{i}"), "x": i, "y": i }))
        .collect();
    let links: Vec<Value> = (0..1000)
        .map(|i| json!({ "a": { "dev": format!("n{}", i * 2), "port": "p1" }, "b": { "dev": format!("n{}", i * 2 + 1), "port": "eth0" }, "kind": "copper" }))
        .collect();
    json!({ "format": "losos-lab-setup", "version": 1, "name": "big", "devices": devices, "links": links }).to_string()
}

#[test]
fn files_match() {
    let fx = fixture("files.json");
    let docs = fx["docs"].as_array().cloned().unwrap_or_default();
    for r in fx["results"].as_array().into_iter().flatten() {
        let label = r["label"].as_str().unwrap_or("");
        let text = if label == "big" {
            big_doc()
        } else {
            docs.iter()
                .find(|d| d["label"] == r["label"])
                .and_then(|d| d["text"].as_str())
                .unwrap_or("")
                .to_string()
        };
        let mut lab = LabCore::new(7.0, T0, 0.0);
        lab.set_mode("simulation");
        let out = lab.import_setup(&text, "x.llf");
        if let Some(p) = r["problem"].as_str() {
            assert_eq!(out["error"], json!(p), "{label}");
            continue;
        }
        assert!(out.get("error").is_none(), "{label}: {out}");
        same(&format!("{label} skipped"), &out["skipped"], &r["skipped"]);
        let s = snap(&lab);
        same(
            &format!("{label} devices"),
            &json!(s["world"]["devices"].as_array().map_or(0, Vec::len)),
            &r["devices"],
        );
        same(
            &format!("{label} links"),
            &json!(s["world"]["links"].as_array().map_or(0, Vec::len)),
            &r["links"],
        );
        if label != "big" {
            same(&format!("{label} world"), &s["world"], &r["world"]);
            same(&format!("{label} net"), &s["net"], &r["net"]);
        }
    }
}

#[test]
fn round_trips_match() {
    for t in fixture("roundtrip.json").as_array().into_iter().flatten() {
        let key = t["key"].as_str().unwrap_or("");
        let mut lab = LabCore::new(7.0, T0, 0.0);
        lab.load_scenario(key);
        let text = lab.export_setup()["text"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let mut lab = LabCore::new(7.0, T0, 0.0);
        let r = lab.import_setup(&text, "setup.llf");
        same(&format!("{key} skipped"), &r["skipped"], &t["skipped"]);
        let s = snap(&lab);
        same(&format!("{key} world"), &s["world"], &t["world"]);
        same(&format!("{key} net"), &s["net"], &t["net"]);
    }
}

#[test]
fn clock_matches() {
    let fx = fixture("clock.json");
    let mut lab = LabCore::new(7.0, T0, 0.0);
    lab.set_mode("simulation");
    lab.load_scenario("office");
    let tb = snap(&lab)["world"]["devices"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|d| d["name"] == "teambox")
        .and_then(|d| d["id"].as_str().map(str::to_string))
        .unwrap_or_default();
    lab.set_cfg(&tb, "shareCompute", "true");
    lab.clear();
    lab.set_mode("realtime");
    let devs = |lab: &LabCore, f: &dyn Fn(&Value) -> Option<Value>| -> Value {
        Value::Array(
            snap(lab)["world"]["devices"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(f)
                .collect(),
        )
    };
    for s in fx["steps"].as_array().into_iter().flatten() {
        if s["next"] == "jump30h" {
            lab.clear();
            lab.set_clock_speed(3600.0);
            lab.advance_clock(30.0 * 3600e3);
            let flows = flow_titles(&lab);
            let out = lab.tick(0.0);
            same("jump t", &snap(&lab)["clock"]["t"], &s["t"]);
            same("jump text", &out["clock"]["text"], &s["text"]);
            same("jump toasts", &out["toasts"], &s["toasts"]);
            same("jump flows", &flows, &s["flows"]);
            continue;
        }
        lab.clear();
        let next = snap(&lab)["clock"]["next"]["key"].clone();
        same("next timer", &next, &s["next"]);
        lab.skip_to_next_timer();
        let c = snap(&lab)["clock"].clone();
        same("after skip t", &c["t"], &s["afterSkip"]["t"]);
        same("after skip text", &c["text"], &s["afterSkip"]["text"]);
        same(
            "after skip flows",
            &flow_titles(&lab),
            &s["afterSkip"]["flows"],
        );
        lab.clear();
        lab.advance_clock(20e3);
        let flows = flow_titles(&lab);
        let out = lab.tick(0.0);
        let at = s["at"].as_str().unwrap_or("");
        same(&format!("{at} t"), &snap(&lab)["clock"]["t"], &s["t"]);
        same(&format!("{at} text"), &out["clock"]["text"], &s["text"]);
        same(&format!("{at} toasts"), &out["toasts"], &s["toasts"]);
        same(&format!("{at} flows"), &flows, &s["flows"]);
        same(
            &format!("{at} rebooting"),
            &devs(&lab, &|d| {
                (d["rebooting"] == json!(true)).then(|| d["id"].clone())
            }),
            &s["rebooting"],
        );
        same(
            &format!("{at} gen"),
            &devs(&lab, &|d| d.get("gen").map(|g| json!([d["id"], g]))),
            &s["gen"],
        );
        same(
            &format!("{at} computeOpen"),
            &devs(&lab, &|d| d.get("computeOpen").map(|c| json!([d["id"], c]))),
            &s["computeOpen"],
        );
    }
}
