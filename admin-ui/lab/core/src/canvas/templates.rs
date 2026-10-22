//! Templates: small JSON Canvas fragments the page drops onto the canvas,
//! in English, Slovak or German. A template is an ordinary fragment, so it
//! goes in through the same door as a paste; the network one carries
//! devices and cables and so lands in the simulator too.

use super::file::{to_doc, Doc};
use serde_json::{json, Value};

pub struct Template {
    pub key: &'static str,
}

pub const TEMPLATES: [Template; 5] = [
    Template { key: "flowchart" },
    Template {
        key: "architecture",
    },
    Template { key: "mindmap" },
    Template { key: "board" },
    Template { key: "network" },
];

/// Picks the words for `lang` from an (en, sk, de) triple.
fn w(lang: &str, t: (&'static str, &'static str, &'static str)) -> &'static str {
    match lang {
        "sk" => t.1,
        "de" => t.2,
        _ => t.0,
    }
}

fn card(id: &str, x: i64, y: i64, wd: i64, ht: i64, text: &str) -> Value {
    json!({ "id": id, "type": "text", "x": x, "y": y, "width": wd, "height": ht, "text": text })
}

fn shaped(mut v: Value, shape: &str, color: Option<&str>) -> Value {
    v["styleAttributes"] = json!({ "shape": shape });
    if let Some(c) = color {
        v["color"] = c.into();
    }
    v
}

fn colored(mut v: Value, color: &str) -> Value {
    v["color"] = color.into();
    v
}

fn group(id: &str, x: i64, y: i64, wd: i64, ht: i64, label: &str) -> Value {
    json!({ "id": id, "type": "group", "x": x, "y": y, "width": wd, "height": ht, "label": label })
}

fn edge(id: &str, from: &str, fs: &str, to: &str, ts: &str) -> Value {
    json!({ "id": id, "fromNode": from, "fromSide": fs, "toNode": to, "toSide": ts })
}

fn square(mut v: Value) -> Value {
    v["styleAttributes"] = json!({ "pathfindingMethod": "square" });
    v
}

fn labelled(mut v: Value, label: &str) -> Value {
    v["label"] = label.into();
    v
}

fn device(id: &str, cx: i64, cy: i64, kind: &str, name: &str) -> Value {
    json!({
        "id": id, "type": "text", "x": cx - 88, "y": cy - 36, "width": 176, "height": 72,
        "text": format!("**{name}**"),
        "losos": { "device": { "type": kind, "name": name } },
    })
}

fn cable(id: &str, from: &str, to: &str, kind: &str) -> Value {
    json!({ "id": id, "fromNode": from, "toNode": to, "toEnd": "none", "losos": { "link": { "kind": kind } } })
}

pub fn template(key: &str, lang: &str) -> Option<Doc> {
    let l = |t| w(lang, t);
    let v = match key {
        "flowchart" => json!({
            "nodes": [
                shaped(card("start", 0, 0, 220, 60, l(("Start", "Začiatok", "Start"))), "pill", Some("4")),
                card("ask", 0, 120, 220, 70, l(("Gather the request", "Zozbierajte požiadavku", "Anfrage aufnehmen"))),
                shaped(card("clear", 0, 260, 220, 120, l(("Is it clear?", "Je to jasné?", "Ist es klar?"))), "diamond", Some("3")),
                card("work", 0, 450, 220, 70, l(("Do the work", "Urobte prácu", "Arbeit erledigen"))),
                shaped(card("done", 0, 580, 220, 60, l(("Done", "Hotovo", "Fertig"))), "pill", Some("4")),
                card("question", 320, 285, 220, 70, l(("Ask a question", "Opýtajte sa", "Nachfragen"))),
            ],
            "edges": [
                edge("e1", "start", "bottom", "ask", "top"),
                edge("e2", "ask", "bottom", "clear", "top"),
                labelled(edge("e3", "clear", "bottom", "work", "top"), l(("Yes", "Áno", "Ja"))),
                edge("e4", "work", "bottom", "done", "top"),
                labelled(edge("e5", "clear", "right", "question", "left"), l(("No", "Nie", "Nein"))),
                square(edge("e6", "question", "top", "ask", "right")),
            ],
        }),
        "architecture" => json!({
            "nodes": [
                group("services", 300, -40, 620, 380, l(("Services", "Služby", "Dienste"))),
                shaped(card("people", 0, 120, 200, 70, l(("People", "Ľudia", "Menschen"))), "circle", Some("5")),
                card("web", 340, 20, 240, 80, l(("**Web app**\nWhat people open", "**Webová aplikácia**\nTo, čo ľudia otvárajú", "**Web-App**\nDas, was man öffnet"))),
                card("api", 340, 200, 240, 80, l(("**API**\nAnswers the web app", "**API**\nOdpovedá webovej aplikácii", "**API**\nAntwortet der Web-App"))),
                card("worker", 640, 200, 240, 80, l(("**Worker**\nLong jobs, one at a time", "**Worker**\nDlhé úlohy, jedna po druhej", "**Worker**\nLange Aufgaben, eine nach der anderen"))),
                shaped(card("db", 340, 420, 240, 110, l(("Database", "Databáza", "Datenbank"))), "database", Some("6")),
                shaped(card("queue", 640, 420, 240, 110, l(("Queue", "Fronta", "Warteschlange"))), "database", Some("2")),
            ],
            "edges": [
                edge("a1", "people", "right", "web", "left"),
                edge("a2", "web", "bottom", "api", "top"),
                edge("a3", "api", "bottom", "db", "top"),
                edge("a4", "api", "right", "worker", "left"),
                edge("a5", "worker", "bottom", "queue", "top"),
            ],
        }),
        "mindmap" => {
            let branches = [
                (("Goals", "Ciele", "Ziele"), "1", -380, -220),
                (("People", "Ľudia", "Menschen"), "2", 160, -260),
                (("Timeline", "Časový plán", "Zeitplan"), "3", 420, -40),
                (("Risks", "Riziká", "Risiken"), "4", 220, 200),
                (("Budget", "Rozpočet", "Budget"), "5", -300, 220),
                (
                    ("Open questions", "Otvorené otázky", "Offene Fragen"),
                    "6",
                    -520,
                    0,
                ),
            ];
            let mut nodes = vec![shaped(
                card(
                    "idea",
                    -120,
                    -40,
                    240,
                    80,
                    l(("# Main idea", "# Hlavná myšlienka", "# Hauptidee")),
                ),
                "pill",
                None,
            )];
            let mut edges = Vec::new();
            for (i, (t, c, x, y)) in branches.iter().enumerate() {
                let id = format!("b{i}");
                nodes.push(colored(card(&id, *x, *y, 200, 60, l(*t)), c));
                let mut e = json!({ "id": format!("m{i}"), "fromNode": "idea", "toNode": id, "toEnd": "none" });
                e["color"] = (*c).into();
                edges.push(e);
            }
            json!({ "nodes": nodes, "edges": edges })
        }
        "board" => {
            let cols = [
                (
                    ("To do", "Na urobenie", "Zu tun"),
                    [
                        ("Write the plan", "Napísať plán", "Plan schreiben"),
                        (
                            "Ask Anna about the room",
                            "Opýtať sa Anny na miestnosť",
                            "Anna nach dem Raum fragen",
                        ),
                    ],
                ),
                (
                    ("Doing", "Robí sa", "In Arbeit"),
                    [
                        ("Draft the slides", "Pripraviť snímky", "Folien entwerfen"),
                        ("Book the train", "Kúpiť lístok na vlak", "Zug buchen"),
                    ],
                ),
                (
                    ("Done", "Hotovo", "Erledigt"),
                    [
                        ("Pick a date", "Vybrať dátum", "Datum festlegen"),
                        ("Send the invite", "Poslať pozvánku", "Einladung senden"),
                    ],
                ),
            ];
            let mut nodes = Vec::new();
            for (i, (title, cards)) in cols.iter().enumerate() {
                let x = i as i64 * 300;
                nodes.push(group(&format!("col{i}"), x, 0, 260, 360, l(*title)));
                for (j, c) in cards.iter().enumerate() {
                    let text = format!("- [{}] {}", if i == 2 { "x" } else { " " }, l(*c));
                    nodes.push(card(
                        &format!("c{i}{j}"),
                        x + 20,
                        40 + j as i64 * 110,
                        220,
                        90,
                        &text,
                    ));
                }
            }
            json!({ "nodes": nodes, "edges": [] })
        }
        "network" => json!({
            "nodes": [
                device("inet", 360, 0, "internet", "internet"),
                device("router", 360, 180, "router", "home-router"),
                device("box", 160, 360, "box", "mattbox"),
                device("laptop", 560, 360, "laptop", "laptop"),
                colored(card("note", -200, 120, 260, 120, l((
                    "## Home network\nDouble-click a device to open its console.",
                    "## Domáca sieť\nDvojitým kliknutím na zariadenie otvoríte jeho konzolu.",
                    "## Heimnetz\nDoppelklicke ein Gerät, um seine Konsole zu öffnen.",
                ))), "5"),
            ],
            "edges": [
                cable("n1", "router", "inet", "auto"),
                cable("n2", "router", "box", "auto"),
                cable("n3", "router", "laptop", "auto"),
            ],
        }),
        _ => return None,
    };
    to_doc(&v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_reads_in_every_language() {
        for t in TEMPLATES {
            for lang in ["en", "sk", "de"] {
                let doc = template(t.key, lang).unwrap_or_else(|| panic!("{} {lang}", t.key));
                let nodes: Vec<_> = doc
                    .nodes
                    .iter()
                    .filter_map(super::super::file::read_node)
                    .collect();
                assert_eq!(
                    nodes.len(),
                    doc.nodes.len(),
                    "{} {lang}: a node does not read",
                    t.key
                );
                for e in &doc.edges {
                    let e = super::super::file::read_edge(e).expect("an edge");
                    assert!(nodes.iter().any(|n| n.id == e.from_node), "{}", t.key);
                    assert!(nodes.iter().any(|n| n.id == e.to_node), "{}", t.key);
                }
            }
        }
        assert!(template("nope", "en").is_none());
    }
}
