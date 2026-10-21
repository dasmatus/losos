//! The canvas: JSON Canvas files in and out (what Obsidian writes, kept
//! whole), devices and cables as cards and edges that stay in step with the
//! world, editing, undo, groups, paste and templates, and hostile files.

use losos_lab_core::LabCore;
use serde_json::{json, Value};

const T0: f64 = 1_791_453_600_000.0;

fn blank() -> LabCore {
    let mut l = LabCore::new(1.0, T0, 0.0);
    l.canvas_new();
    l
}

fn canvas(l: &LabCore) -> Value {
    l.canvas_snapshot()
}

fn nodes(l: &LabCore) -> Vec<Value> {
    canvas(l)["nodes"].as_array().cloned().unwrap_or_default()
}

fn edges(l: &LabCore) -> Vec<Value> {
    canvas(l)["edges"].as_array().cloned().unwrap_or_default()
}

fn devices(l: &LabCore) -> Vec<Value> {
    l.snapshot()["world"]["devices"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

fn links(l: &LabCore) -> Vec<Value> {
    l.snapshot()["world"]["links"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

fn id(v: &Value) -> String {
    v["id"].as_str().unwrap_or_default().to_string()
}

fn add(l: &mut LabCore, spec: Value) -> String {
    let r = l.canvas_add_node(&spec.to_string());
    assert!(r.get("error").is_none(), "{r}");
    id(&r["node"])
}

fn exported(l: &LabCore) -> Value {
    serde_json::from_str(l.export_setup()["text"].as_str().unwrap_or("")).unwrap_or(Value::Null)
}

/// A canvas as Obsidian writes one, with a plugin's keys on a node, an edge
/// and the document.
const OBSIDIAN: &str = r##"{
  "nodes": [
    {"id":"6f0ad84f44ce9c17","type":"text","text":"# Plan\n\n- [ ] one","x":-220,"y":-120,"width":250,"height":140,"color":"1"},
    {"id":"a2c6b3d1e0f94e7b","type":"file","file":"Notes/Ideas.md","subpath":"#Later","x":120,"y":-120,"width":400,"height":400},
    {"id":"5bd3a0c2e8f71964","type":"link","url":"https://jsoncanvas.org","x":-220,"y":80,"width":250,"height":120},
    {"id":"0c7e5d9a8b6f4321","type":"group","label":"Inbox","x":-260,"y":-160,"width":820,"height":460,"background":"bg.png","backgroundStyle":"cover","color":"#ff8800"},
    {"id":"77aa","type":"text","text":"Shape","x":0,"y":400,"width":200,"height":120,"styleAttributes":{"shape":"diamond","border":"dashed"},"plugin":{"pinned":true}}
  ],
  "edges": [
    {"id":"e1","fromNode":"6f0ad84f44ce9c17","fromSide":"right","toNode":"a2c6b3d1e0f94e7b","toSide":"left","label":"see","color":"4"},
    {"id":"e2","fromNode":"5bd3a0c2e8f71964","toNode":"77aa","fromEnd":"arrow","toEnd":"none","styleAttributes":{"path":"dotted","pathfindingMethod":"square"},"seen":3}
  ],
  "metadata":{"version":"1.0-1.0"}
}"##;

#[test]
fn obsidian_files_round_trip_whole() {
    let mut l = LabCore::new(1.0, T0, 0.0);
    let r = l.import_setup(OBSIDIAN, "Plan.canvas");
    assert_eq!(r, json!({ "skipped": 0, "name": "Plan" }));
    assert!(devices(&l).is_empty());
    let doc = exported(&l);
    let want: Value = serde_json::from_str(OBSIDIAN).unwrap_or(Value::Null);
    assert_eq!(doc["nodes"], want["nodes"]);
    assert_eq!(doc["edges"], want["edges"]);
    assert_eq!(doc["metadata"], want["metadata"]);
    assert_eq!(doc["losos"]["name"], json!("Plan"));
    assert_eq!(l.export_setup()["filename"], json!("plan.canvas"));
    // Opened again, it is the same document.
    let mut again = LabCore::new(2.0, T0, 0.0);
    again.import_setup(
        l.export_setup()["text"].as_str().unwrap_or(""),
        "Plan.canvas",
    );
    assert_eq!(exported(&again)["nodes"], want["nodes"]);
}

#[test]
fn every_edge_has_a_path_and_heads() {
    let mut l = LabCore::new(1.0, T0, 0.0);
    l.import_setup(OBSIDIAN, "Plan.canvas");
    let e = edges(&l);
    assert_eq!(e.len(), 2);
    assert!(
        e[0]["path"].as_str().unwrap_or("").starts_with("M30 -50 C"),
        "{}",
        e[0]["path"]
    );
    assert_eq!(e[0]["heads"].as_array().map(Vec::len), Some(1));
    assert_eq!(e[0]["heads"][0]["end"], json!("to"));
    assert!(
        e[1]["path"].as_str().unwrap_or("").contains(" L"),
        "a square route is straight lines"
    );
    assert_eq!(e[1]["heads"][0]["end"], json!("from"));
}

#[test]
fn devices_are_cards_and_cables_are_edges() {
    let mut l = LabCore::new(1.0, T0, 0.0);
    l.load_scenario("home");
    let d = devices(&l);
    let n = nodes(&l);
    assert_eq!(n.len(), d.len());
    for dev in &d {
        let card = n
            .iter()
            .find(|c| c["dev"] == dev["id"])
            .expect("a card per device");
        let cx = card["x"].as_f64().unwrap_or(0.0) + card["width"].as_f64().unwrap_or(0.0) / 2.0;
        let x = dev["x"].as_f64().unwrap_or(0.0);
        assert!((cx - x * 1.5).abs() <= 1.0, "the card sits on its device");
        assert!(card["text"]
            .as_str()
            .unwrap_or("")
            .contains(dev["name"].as_str().unwrap_or("?")));
    }
    let e = edges(&l);
    assert_eq!(e.len(), links(&l).len());
    assert!(e
        .iter()
        .all(|e| e["link"].is_string() && e["cable"].is_string()));
    // The file carries the device and the cable, ports and all.
    let doc = exported(&l);
    let box_card = doc["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|n| n["losos"]["device"]["name"] == "mattbox")
        .cloned()
        .unwrap_or(Value::Null);
    assert_eq!(box_card["type"], json!("text"));
    assert_eq!(box_card["losos"]["device"]["type"], json!("box"));
    assert!(doc["edges"][0]["losos"]["link"]["fromPort"].is_string());
    // And builds the same world again.
    let mut again = LabCore::new(1.0, T0, 0.0);
    let r = again.import_setup(
        l.export_setup()["text"].as_str().unwrap_or(""),
        "home.canvas",
    );
    assert_eq!(r["skipped"], json!(0));
    assert_eq!(
        again.snapshot()["world"]["devices"],
        l.snapshot()["world"]["devices"]
    );
    assert_eq!(
        again.snapshot()["world"]["links"],
        l.snapshot()["world"]["links"]
    );
}

#[test]
fn the_logical_view_and_the_canvas_move_together() {
    let mut l = LabCore::new(1.0, T0, 0.0);
    l.load_scenario("home");
    let dev = id(&devices(&l)[0]);
    let card = nodes(&l)
        .into_iter()
        .find(|c| c["dev"] == json!(dev))
        .unwrap_or(Value::Null);
    let r = l.canvas_move(&json!([id(&card)]).to_string(), 150.0, -30.0, 1.0);
    assert!(r.get("error").is_none(), "{r}");
    let after = devices(&l)
        .into_iter()
        .find(|d| d["id"] == json!(dev))
        .unwrap_or(Value::Null);
    assert_eq!(
        after["x"].as_f64().unwrap_or(0.0) - card_dev_x(&card),
        100.0
    );
    l.move_device(&dev, &json!({ "x": 100, "y": 100 }).to_string());
    let moved = nodes(&l)
        .into_iter()
        .find(|c| c["dev"] == json!(dev))
        .unwrap_or(Value::Null);
    assert_eq!(moved["x"], json!(150 - 88));
    assert_eq!(moved["y"], json!(150 - 36));
}

fn card_dev_x(card: &Value) -> f64 {
    (card["x"].as_f64().unwrap_or(0.0) + card["width"].as_f64().unwrap_or(0.0) / 2.0) / 1.5
}

#[test]
fn drawing_a_cable_makes_a_link_and_deleting_a_card_its_device() {
    let mut l = blank();
    let r = l.canvas_add_device("router", 0.0, 0.0);
    let router = id(&r["node"]);
    let r = l.canvas_add_device("box", 300.0, 0.0);
    let bx = id(&r["node"]);
    assert_eq!(devices(&l).len(), 2);
    // A plain arrow between two devices is a drawing, not a cable.
    let r = l.canvas_connect(
        &json!({ "fromNode": router, "toNode": bx, "label": "talks to" }).to_string(),
    );
    assert!(r["edge"]["link"].is_null(), "{r}");
    assert!(links(&l).is_empty());
    let r = l.canvas_connect(
        &json!({ "fromNode": bx, "fromSide": "left", "toNode": router, "toSide": "right", "cable": "copper" }).to_string(),
    );
    assert!(r.get("error").is_none(), "{r}");
    assert_eq!(links(&l).len(), 1);
    let cable = edges(&l)
        .into_iter()
        .find(|e| e["link"].is_string())
        .unwrap_or(Value::Null);
    assert_eq!(cable["cable"], json!("copper"));
    assert_eq!(cable["sides"], json!(["left", "right"]));
    // A cable needs two devices.
    let note = add(
        &mut l,
        json!({ "type": "text", "x": 0, "y": 300, "text": "hi" }),
    );
    let r =
        l.canvas_connect(&json!({ "fromNode": note, "toNode": bx, "cable": "copper" }).to_string());
    assert_eq!(r["error"], json!("A cable goes between two devices."));
    // Deleting the box takes its device, its cable and both arrows.
    let r = l.canvas_delete(&json!({ "nodes": [bx] }).to_string());
    assert_eq!(r["nodes"], json!([bx]));
    assert_eq!(devices(&l).len(), 1);
    assert!(links(&l).is_empty());
    assert!(edges(&l).is_empty());
    // Undo brings all of it back, redo takes it away again.
    assert_eq!(l.canvas_undo(), json!({ "ok": true }));
    assert_eq!(devices(&l).len(), 2);
    assert_eq!(links(&l).len(), 1);
    assert_eq!(edges(&l).len(), 2);
    assert_eq!(l.canvas_redo(), json!({ "ok": true }));
    assert_eq!(devices(&l).len(), 1);
    assert!(canvas(&l)["canUndo"].as_bool().unwrap_or(false));
}

#[test]
fn a_drag_is_one_undo_step() {
    let mut l = blank();
    let a = add(
        &mut l,
        json!({ "type": "text", "x": 0, "y": 0, "text": "a" }),
    );
    for _ in 0..10 {
        l.canvas_move(&json!([a]).to_string(), 5.0, 0.0, 7.0);
    }
    assert_eq!(nodes(&l)[0]["x"], json!(50));
    l.canvas_undo();
    assert_eq!(nodes(&l)[0]["x"], json!(0));
    l.canvas_undo();
    assert!(nodes(&l).is_empty());
    assert!(l.canvas_undo().get("error").is_some());
}

#[test]
fn groups_carry_what_lies_inside() {
    let mut l = blank();
    let a = add(
        &mut l,
        json!({ "type": "text", "x": 0, "y": 0, "text": "a" }),
    );
    let b = add(
        &mut l,
        json!({ "type": "text", "x": 0, "y": 100, "text": "b" }),
    );
    let out = add(
        &mut l,
        json!({ "type": "text", "x": 900, "y": 0, "text": "c" }),
    );
    let r = l.canvas_group(&json!([a, b]).to_string(), "Inbox");
    let g = id(&r["node"]);
    assert_eq!(
        nodes(&l)[0]["type"],
        json!("group"),
        "a group goes under its cards"
    );
    assert_eq!(r["node"]["label"], json!("Inbox"));
    let r = l.canvas_move(&json!([g]).to_string(), 10.0, 20.0, 1.0);
    let moved: Vec<String> = r["moved"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    assert!(
        moved.contains(&a) && moved.contains(&b) && !moved.contains(&out),
        "{moved:?}"
    );
}

#[test]
fn editing_cards_and_edges() {
    let mut l = blank();
    let a = add(
        &mut l,
        json!({ "type": "text", "x": 0, "y": 0, "text": "a" }),
    );
    let b = add(
        &mut l,
        json!({ "type": "text", "x": 400, "y": 0, "text": "b" }),
    );
    let r = l.canvas_update_node(
        &a,
        &json!({ "color": "3", "styleAttributes": { "shape": "diamond" }, "text": "**A**" })
            .to_string(),
    );
    assert_eq!(r["node"]["color"], json!("3"));
    assert_eq!(r["node"]["styleAttributes"], json!({ "shape": "diamond" }));
    let r = l.canvas_update_node(
        &a,
        &json!({ "styleAttributes": { "shape": null }, "color": null }).to_string(),
    );
    assert!(
        r["node"].get("styleAttributes").is_none() && r["node"].get("color").is_none(),
        "{r}"
    );
    assert_eq!(
        l.canvas_update_node(&a, &json!({ "color": "red" }).to_string())["error"],
        json!("That is not a colour a canvas can hold.")
    );
    assert_eq!(
        l.canvas_update_node(
            &a,
            &json!({ "styleAttributes": { "shape": "star" } }).to_string()
        )["error"],
        json!("A card has no style shape of that kind.")
    );
    let r = l.canvas_connect(&json!({ "fromNode": a, "toNode": b }).to_string());
    let e = id(&r["edge"]);
    let r = l.canvas_update_edge(&e, &json!({ "label": "next", "toEnd": "none", "fromEnd": "arrow", "styleAttributes": { "arrow": "diamond" } }).to_string());
    assert_eq!(r["edge"]["label"], json!("next"));
    assert_eq!(edges(&l)[0]["heads"][0]["end"], json!("from"));
    assert_eq!(
        l.canvas_connect(&json!({ "fromNode": a, "toNode": a }).to_string())["error"],
        json!("An arrow goes from one card to another.")
    );
    // Lining up and ordering.
    l.canvas_update_node(&b, &json!({ "height": 200 }).to_string());
    l.canvas_align(&json!([a, b]).to_string(), "top");
    assert_eq!(nodes(&l)[1]["y"], json!(0));
    l.canvas_order(&json!([a]).to_string(), "front");
    assert_eq!(id(&nodes(&l)[1]), a);
    // A file card needs its file, a link card its address.
    assert!(l
        .canvas_add_node(&json!({ "type": "file", "x": 0, "y": 0 }).to_string())
        .get("error")
        .is_some());
    assert!(l
        .canvas_add_node(
            &json!({ "type": "link", "x": 0, "y": 0, "url": "https://losos.dasmat.us" })
                .to_string()
        )
        .get("error")
        .is_none());
}

#[test]
fn copy_and_paste_bring_new_devices() {
    let mut l = blank();
    let r = l.canvas_add_device("router", 0.0, 0.0);
    let router = id(&r["node"]);
    let r = l.canvas_add_device("box", 300.0, 0.0);
    let bx = id(&r["node"]);
    l.canvas_connect(&json!({ "fromNode": router, "toNode": bx, "cable": "auto" }).to_string());
    let clip = l.canvas_copy(&json!([router, bx]).to_string())["text"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let r = l.canvas_paste(&clip, 0.0, 400.0);
    assert_eq!(r["nodes"].as_array().map(Vec::len), Some(2), "{r}");
    assert_eq!(devices(&l).len(), 4);
    assert_eq!(links(&l).len(), 2);
    let names: Vec<String> = devices(&l)
        .iter()
        .filter_map(|d| d["name"].as_str().map(str::to_string))
        .collect();
    assert!(names.contains(&"router2".to_string()), "{names:?}");
    assert!(l.canvas_paste("not json", 0.0, 0.0).get("error").is_some());
    // Undo of a paste is one step.
    l.canvas_undo();
    assert_eq!(devices(&l).len(), 2);
}

#[test]
fn templates_land_centred_in_any_language() {
    let mut l = blank();
    for t in l.canvas_templates().as_array().cloned().unwrap_or_default() {
        let key = t["key"].as_str().unwrap_or("");
        let r = l.canvas_insert_template(key, "sk", 1000.0, 1000.0);
        assert!(r.get("error").is_none(), "{key}: {r}");
    }
    // The network template is real gear.
    assert_eq!(devices(&l).len(), 4);
    assert_eq!(links(&l).len(), 3);
    let texts: Vec<String> = nodes(&l)
        .iter()
        .filter_map(|n| n["text"].as_str().map(str::to_string))
        .collect();
    assert!(texts.iter().any(|t| t == "Je to jasné?"), "{texts:?}");
    assert!(l
        .canvas_insert_template("nope", "en", 0.0, 0.0)
        .get("error")
        .is_some());
}

#[test]
fn hostile_canvases() {
    let mut l = LabCore::new(1.0, T0, 0.0);
    // Not a canvas at all.
    for text in ["[]", "\"x\"", "{\"nodes\": 3}", "{\"edges\": {}}"] {
        assert_eq!(
            l.import_setup(text, "x.canvas")["error"],
            json!("This is not a canvas or a setup file."),
            "{text}"
        );
    }
    let doc = json!({
        "nodes": [
            { "id": "a", "type": "text", "x": 0, "y": 0, "width": 10, "height": 10, "color": "javascript:alert(1)" },
            { "id": "a", "type": "text", "x": 0, "y": 0, "width": 10, "height": 10 },
            { "id": "b", "type": "script", "x": 0, "y": 0, "width": 10, "height": 10 },
            { "id": "c", "type": "text", "x": "0", "y": 0, "width": 10, "height": 10 },
            { "id": "d", "type": "text", "x": 1e300, "y": 0, "width": -5, "height": 10,
              "losos": { "device": { "type": "__proto__" } } },
            { "id": "e", "type": "text", "x": 0, "y": 0, "width": 10, "height": 10,
              "losos": { "device": { "type": "box", "name": "<b>x</b>" } } },
        ],
        "edges": [
            { "id": "x1", "fromNode": "a", "toNode": "zz" },
            { "id": "x2", "fromNode": "a", "toNode": "e", "fromSide": "middle", "losos": { "link": { "kind": "copper" } } },
            { "id": "x3", "fromNode": "a", "toNode": "e", "toEnd": "spear" },
        ],
    });
    let r = l.import_setup(&doc.to_string(), "x.canvas");
    // A second "a", a "script" and a "c" with text for a number: 3 nodes;
    // the edge to nowhere: 1.
    assert_eq!(r["skipped"], json!(4), "{r}");
    let n = nodes(&l);
    assert_eq!(n.len(), 3);
    assert!(
        n[0].get("color").is_none(),
        "a colour that is not one is dropped"
    );
    assert_eq!(n[1]["x"], json!(10_000_000), "places are clamped");
    assert_eq!(n[1]["width"], json!(1));
    assert_eq!(
        n[1]["losos"]["device"]["type"],
        json!("__proto__"),
        "an unknown device stays data"
    );
    assert_eq!(devices(&l).len(), 1);
    assert_eq!(devices(&l)[0]["name"], json!("b-x-b"));
    let e = edges(&l);
    assert_eq!(e.len(), 2);
    assert!(e[0].get("fromSide").is_none());
    assert!(
        e[0]["link"].is_null(),
        "a cable from a card that is no device is a drawing"
    );
    assert!(e[1].get("toEnd").is_none());
    // Ten thousand cards: the cap holds.
    let many: Vec<Value> = (0..10_000)
        .map(|i| json!({ "id": format!("n{i}"), "type": "text", "x": i, "y": 0, "width": 10, "height": 10, "text": "" }))
        .collect();
    let r = l.import_setup(&json!({ "nodes": many }).to_string(), "big.canvas");
    assert_eq!(nodes(&l).len(), 5000);
    assert_eq!(r["skipped"], json!(5000));
}

#[test]
fn old_setup_files_open_as_canvases() {
    let mut l = LabCore::new(1.0, T0, 0.0);
    l.load_scenario("office");
    let old = l.export_setup_v1();
    let mut m = LabCore::new(1.0, T0, 0.0);
    let r = m.import_setup(&old.to_string(), "office.llf");
    assert_eq!(r["skipped"], json!(0), "{r}");
    assert_eq!(nodes(&m).len(), devices(&m).len());
    assert_eq!(edges(&m).len(), links(&m).len());
    assert_eq!(
        m.export_setup()["filename"],
        json!("company-with-its-own-gateway.canvas")
    );
}

#[test]
fn ids_are_obsidian_shaped_and_reproducible() {
    let mk = || {
        let mut l = blank();
        add(&mut l, json!({ "type": "text", "x": 0, "y": 0 }))
    };
    let (a, b) = (mk(), mk());
    assert_eq!(a, b);
    assert_eq!(a.len(), 16);
    assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn a_diagram_keeps_the_name_it_is_given() {
    let mut l = blank();
    assert_eq!(l.export_setup()["name"], "My diagram");
    assert_eq!(l.rename_doc("  Office plan  ")["name"], "Office plan");
    let doc = l.export_setup();
    assert_eq!(doc["filename"], "office-plan.canvas");
    let text = doc["text"].as_str().unwrap().to_string();
    let file: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(file["losos"]["name"], "Office plan");
    assert!(l.rename_doc("   ").get("error").is_some());
    assert!(l.rename_doc("two\nlines").get("error").is_some());
    assert!(l.rename_doc(&"x".repeat(61)).get("error").is_some());
    // The name comes back with the file.
    let mut m = blank();
    m.import_setup(&text, "anything.canvas");
    assert_eq!(m.export_setup()["name"], "Office plan");
}

#[test]
fn a_shared_text_reads_back_and_keeps_the_simulator_when_only_cards_change() {
    let mut l = blank();
    let r = l.canvas_add_device("box", 300.0, 300.0);
    assert!(r.get("error").is_none(), "{r}");
    let card = id(&r["node"]);
    let dev = r["device"]["id"].as_str().unwrap_or_default().to_string();
    let note = add(
        &mut l,
        json!({"type": "text", "x": 0, "y": 0, "width": 200, "height": 60, "text": "hello"}),
    );
    let mut doc = exported(&l);
    let x0 = devices(&l)[0]["x"].as_f64().unwrap_or_default();

    // Someone else writes on the note and drags the device's card.
    for n in doc["nodes"].as_array_mut().unwrap() {
        if n["id"] == note.as_str() {
            n["text"] = json!("hello, both of you");
        }
        if n["id"] == card.as_str() {
            n["x"] = json!(n["x"].as_f64().unwrap_or_default() + 150.0);
        }
    }
    let r = l.canvas_sync(&serde_json::to_string_pretty(&doc).unwrap());
    assert_eq!(r["full"], json!(false), "{r}");
    let n = nodes(&l).into_iter().find(|n| id(n) == note).unwrap();
    assert_eq!(n["text"], json!("hello, both of you"));
    let d = &devices(&l)[0];
    assert_eq!(id(d), dev, "the same device, not a new one");
    assert!(
        (d["x"].as_f64().unwrap_or_default() - (x0 + 100.0)).abs() < 1e-9,
        "{d}"
    );

    // A change to the device itself rebuilds the world.
    let mut doc = exported(&l);
    for n in doc["nodes"].as_array_mut().unwrap() {
        if n["id"] == card.as_str() {
            n["losos"]["device"]["name"] = json!("kitchenbox");
        }
    }
    let r = l.canvas_sync(&doc.to_string());
    assert_eq!(r["full"], json!(true), "{r}");
    assert_eq!(devices(&l)[0]["name"], json!("kitchenbox"));
    assert_eq!(nodes(&l).len(), 2);

    // Two edits merged as text can lose a comma; the file still reads.
    let text = l.export_setup()["text"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let broken = text.replacen("},\n", "}\n", 1);
    assert_ne!(broken, text);
    let r = l.canvas_sync(&broken);
    assert!(r.get("error").is_none(), "{r}");
    assert_eq!(nodes(&l).len(), 2);

    assert!(l.canvas_sync("not a canvas").get("error").is_some());
}
