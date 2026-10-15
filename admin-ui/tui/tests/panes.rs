//! Every pane, drawn into a buffer over the fixtures, plus the keyboard paths
//! that change something: a switch, Apply and its confirmation, the pane
//! search, an edge secret, and the login screen.

mod common;

use std::sync::Arc;

use common::fixtures::{box_route, edge_route, Fixture};
use losos::computer::Computer;
use losos::i18n::{self, has_key, t, tv, with_locale, Arg, Locale};
use losos::keychain::MemoryKeychain;
use losos::theme::{Mode, Theme};
use losos::tui::app::{App, Key};
use losos::tui::draw::draw;
use losos::tui::login::{KeyChoice, Login, LoginAnswer};
use losos::tui::panes::PaneId;
use losos::vault::{HostKind, HostRecord};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use serde_json::{json, Value};

const COLS: u16 = 120;
const ROWS: u16 = 36;

fn theme() -> Theme {
    Theme::new(Mode::Dark, true)
}

fn host(kind: HostKind) -> HostRecord {
    HostRecord {
        url: match kind {
            HostKind::Box => "https://mattbox.local".into(),
            HostKind::Edge => "https://register.losos.dasmat.us".into(),
        },
        kind,
        fingerprint: "3F2A9C1E0B7D44A1C2D3E4F5061728394A5B6C7D".into(),
        public_key: String::new(),
        token: String::new(),
        key_fingerprint: "B".repeat(40),
        tls_pin: None,
    }
}

fn text_of(buf: &Buffer) -> String {
    let area = buf.area;
    let mut lines = Vec::new();
    for y in area.top()..area.bottom() {
        let mut line = String::new();
        for x in area.left()..area.right() {
            line.push_str(buf[(x, y)].symbol());
        }
        lines.push(line.trim_end().to_string());
    }
    lines.join("\n")
}

fn frame_of(app: &mut App, rows: u16) -> String {
    let area = Rect::new(0, 0, COLS, rows);
    let mut buf = Buffer::empty(area);
    draw(app, area, &mut buf, theme());
    text_of(&buf)
}

fn frame(app: &mut App) -> String {
    frame_of(app, ROWS)
}

struct Open {
    app: App,
    fixture: Arc<Fixture>,
}

fn open(kind: HostKind, pane: Option<PaneId>) -> Open {
    let route = match kind {
        HostKind::Box => box_route,
        HostKind::Edge => edge_route,
    };
    let fixture = Arc::new(Fixture::new(route));
    let mut app = App::new(host(kind), fixture.clone()).with_computer(computer());
    app.now = Some(1_791_960_021.0);
    if let Some(pane) = pane {
        app.open_at(pane);
    }
    app.settle();
    frame(&mut app);
    Open { app, fixture }
}

/// This computer, with a keychain in memory and its files in a directory of
/// its own.
fn computer() -> Computer {
    let dir = tempfile::tempdir().unwrap().keep();
    let mut env = std::collections::HashMap::new();
    env.insert("XDG_CONFIG_HOME".to_string(), dir.display().to_string());
    env.insert("LOSOS_DEVICE".to_string(), "testpc".to_string());
    Computer {
        keychain: Arc::new(MemoryKeychain::new()),
        env,
    }
}

/// Press keys, letting the background work finish after each.
fn press(app: &mut App, keys: &[Key]) {
    for key in keys {
        app.key(*key);
        app.settle();
        frame(app);
    }
}

fn typed(text: &str) -> Vec<Key> {
    text.chars().map(Key::Char).collect()
}

fn clean(text: &str, where_: &str) {
    for bad in [
        "undefined",
        "NaN",
        "[object",
        "null",
        "—",
        "{count}",
        "{name}",
        "tui.",
        "settings.panes.",
    ] {
        assert!(!text.contains(bad), "{where_} shows {bad:?}:\n{text}");
    }
}

/// Words each pane shows once its data is in.
fn expect(pane: PaneId) -> &'static [&'static str] {
    match pane {
        PaneId::Overview => &["mattbox", "jellyfin", "131 GB"],
        PaneId::Storage => &["466 GB", "Use reserve"],
        PaneId::Mesh => &["register.losos.dasmat.us", "23:00", "07:00"],
        PaneId::Apps => &["jellyfin", "vaultwarden", "Search for apps"],
        PaneId::Network => &["mattbox.local", "11000"],
        PaneId::Security => &["Stricter memory handling", "Ignore USB devices"],
        PaneId::Advanced => &["losos.hostName", "losos.tpm.enable", "Set by the installer"],
        PaneId::History => &["matus/losos-config", "4f2a9c1"],
        PaneId::Backup => &["matus-losos", "Back up now"],
        PaneId::About => &["mattbox", "Handbook", "23:00-07:00"],
        PaneId::Computer => &["LosOS cloud", "LosOS Git", "Not set up", "testpc"],
        PaneId::EdgeOverview => &["Holds a LosOS certificate", "Relay routes", "Reconcile"],
        PaneId::EdgeSecrets => &["stripe-secret-key", "claude-api-key", "Not set"],
    }
}

#[test]
fn every_box_pane_draws_its_data() {
    i18n::set_locale(Locale::En);
    for pane in PaneId::BOX {
        let mut o = open(HostKind::Box, Some(pane));
        let text = frame(&mut o.app);
        for word in expect(pane) {
            assert!(text.contains(word), "{pane:?} lacks {word:?}:\n{text}");
        }
        assert!(text.contains("LosOS"), "the header names LosOS");
        assert!(text.contains(&pane.label()), "{pane:?}'s title");
        clean(&text, pane.id());
    }
}

#[test]
fn every_edge_pane_draws_its_data() {
    for pane in PaneId::EDGE {
        let mut o = open(HostKind::Edge, Some(pane));
        let text = frame(&mut o.app);
        for word in expect(pane) {
            assert!(text.contains(word), "{pane:?} lacks {word:?}:\n{text}");
        }
        clean(&text, pane.id());
    }
}

#[test]
fn lists_the_tenants_with_their_last_heartbeat() {
    let mut o = open(HostKind::Edge, Some(PaneId::EdgeOverview));
    let text = frame_of(&mut o.app, 90);
    assert!(
        text.contains("zuzka-nas") && text.contains("office-01"),
        "{text}"
    );
    let line = text.lines().find(|l| l.contains("mattbox")).unwrap();
    assert!(line.contains("21"), "{line}");
}

#[test]
fn every_pane_draws_in_slovak_and_german() {
    for locale in [Locale::Sk, Locale::De] {
        with_locale(locale, || {
            for pane in PaneId::BOX {
                let mut o = open(HostKind::Box, Some(pane));
                let text = frame(&mut o.app);
                assert!(text.contains(&pane.label()), "{locale:?} {pane:?}:\n{text}");
                assert!(
                    !text.contains(&t("tui.loading")),
                    "{locale:?} {pane:?} still loading"
                );
                clean(&text, pane.id());
            }
        });
    }
}

#[test]
fn has_a_word_for_every_key_made_up_at_run_time() {
    let mut keys: Vec<String> = Vec::new();
    for s in [
        "off",
        "pending",
        "ok",
        "waiting",
        "refused",
        "diverged",
        "unavailable",
        "error",
    ] {
        keys.push(format!("history.sync.state.{s}"));
    }
    for s in [
        "off",
        "pending",
        "ok",
        "kept",
        "waiting",
        "unavailable",
        "error",
    ] {
        keys.push(format!("history.update.state.{s}"));
    }
    for n in ["usbguard", "malloc", "apparmor", "nosmt"] {
        keys.push(format!("panes.security.{n}.title"));
        keys.push(format!("panes.security.{n}.detail"));
    }
    for pane in PaneId::BOX {
        if let Some(id) = pane.settings_id() {
            for k in ["label", "summary", "keywords"] {
                keys.push(format!("settings.panes.{id}.{k}"));
            }
        }
    }
    let missing: Vec<&String> = keys.iter().filter(|k| !has_key(k)).collect();
    assert!(missing.is_empty(), "{missing:?}");
}

/// Every key the client's sources name exists in the catalogue, so no
/// screen shows a key instead of a sentence.
#[test]
fn every_key_the_sources_name_exists() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut missing = Vec::new();
    let mut seen = 0;
    let mut stack = vec![dir];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") || path.ends_with("messages.rs") {
                continue;
            }
            let src = std::fs::read_to_string(&path).unwrap();
            for (i, _) in src.match_indices('"') {
                let rest = &src[i + 1..];
                let Some(end) = rest.find('"') else { continue };
                let word = &rest[..end];
                let looks_like_key = word.split('.').count() >= 2
                    && word.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
                    && word.split('.').all(|p| !p.is_empty())
                    && (word.starts_with("tui.")
                        || word.starts_with("settings.")
                        || word.starts_with("panes.")
                        || word.starts_with("history.")
                        || word.starts_with("apps.")
                        || word.starts_with("shell.")
                        || word.starts_with("ui."));
                // An option's name and two prefixes the panes add a state to.
                let not_keys = ["apps.ports", "history.sync.state", "history.update.state"];
                if looks_like_key && !not_keys.contains(&word) {
                    seen += 1;
                    if !has_key(word) {
                        missing.push(format!("{}: {word}", path.display()));
                    }
                }
            }
        }
    }
    assert!(seen > 200, "only {seen} keys found");
    missing.sort();
    missing.dedup();
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}

#[test]
fn turns_a_switch_on_applies_it_and_confirms_before_the_rebuild() {
    let mut o = open(HostKind::Box, Some(PaneId::Security));
    press(&mut o.app, &[Key::Enter]);
    let dirty = frame(&mut o.app);
    assert!(dirty.contains(&t("settings.applyBar.discard")), "{dirty}");
    assert!(
        dirty.contains(&tv(
            "settings.applyBar.pending",
            &[("count", Arg::Num(1.0))]
        )),
        "{dirty}"
    );
    let applied = |f: &Fixture| {
        f.calls()
            .into_iter()
            .filter(|c| c.1 == "/api/apply")
            .collect::<Vec<_>>()
    };
    assert!(applied(&o.fixture).is_empty(), "nothing applied yet");
    press(&mut o.app, &[Key::Char('a')]);
    let asking = frame(&mut o.app);
    assert!(asking.contains(&t("tui.apply.title")), "{asking}");
    assert!(
        applied(&o.fixture).is_empty(),
        "the confirmation comes first"
    );
    press(&mut o.app, &[Key::Char('y')]);
    let calls = applied(&o.fixture);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "POST");
    let body = calls[0].2.as_str().expect("overrides.nix as a string");
    assert!(body.contains("losos.hardening.usbguard = true;"), "{body}");
    assert!(
        body.contains(r#"losos.updates.extraSubstituters = [ "https://cache.losos.dasmat.us" ];"#),
        "{body}"
    );
}

#[test]
fn discards_a_draft_with_d() {
    let mut o = open(HostKind::Box, Some(PaneId::Security));
    press(&mut o.app, &[Key::Enter]);
    assert!(frame(&mut o.app).contains(&t("settings.applyBar.discard")));
    press(&mut o.app, &[Key::Char('d')]);
    assert!(!frame(&mut o.app).contains(&t("settings.applyBar.discard")));
}

#[test]
fn filters_the_panes_with_slash_and_opens_the_one_picked() {
    let mut o = open(HostKind::Box, None);
    press(&mut o.app, &[Key::Char('/')]);
    press(&mut o.app, &typed("bucket"));
    let filtered = frame(&mut o.app);
    assert!(filtered.contains("/ bucket"), "{filtered}");
    let sidebar: String = filtered
        .lines()
        .map(|l| l.chars().take(26).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        sidebar.contains("Backup") && !sidebar.contains("Mesh"),
        "{sidebar}"
    );
    press(&mut o.app, &[Key::Enter]);
    assert!(frame(&mut o.app).contains("matus-losos"));
}

#[test]
fn matches_a_pane_by_its_english_words_in_any_language() {
    with_locale(Locale::Sk, || {
        assert!(PaneId::Backup.matches("backup"));
        assert!(PaneId::Backup.matches(&PaneId::Backup.label()));
        assert!(!PaneId::Backup.matches("zzz-nothing"));
    });
}

#[test]
fn moves_between_panes_from_the_sidebar_and_shows_the_help() {
    let mut o = open(HostKind::Box, None);
    o.app.paste("j\r");
    o.app.settle();
    assert!(frame(&mut o.app).contains("Use reserve"));
    press(&mut o.app, &[Key::Char('?')]);
    assert!(frame(&mut o.app).contains(&t("tui.help.title")));
}

#[test]
fn sets_an_edge_secret_without_showing_it() {
    let mut o = open(HostKind::Edge, Some(PaneId::EdgeSecrets));
    press(&mut o.app, &[Key::Enter]);
    let prompt = tv(
        "tui.edge.newValue",
        &[("name", Arg::from("stripe-secret-key"))],
    );
    assert!(frame(&mut o.app).contains(&prompt), "{}", frame(&mut o.app));
    press(&mut o.app, &typed("sk_live_42"));
    assert!(
        !frame(&mut o.app).contains("sk_live_42"),
        "the value is masked while typed"
    );
    press(&mut o.app, &[Key::Enter]);
    let said = tv(
        "tui.edge.secretSet",
        &[("name", Arg::from("stripe-secret-key"))],
    );
    assert!(frame(&mut o.app).contains(&said), "{}", frame(&mut o.app));
    let post = o
        .fixture
        .calls()
        .into_iter()
        .find(|c| c.0 == "POST")
        .unwrap();
    assert_eq!(post.1, "/secrets");
    assert_eq!(
        post.2,
        json!({ "name": "stripe-secret-key", "value": "sk_live_42" })
    );
}

#[test]
fn asks_before_reconciling_the_edge() {
    let mut o = open(HostKind::Edge, Some(PaneId::EdgeOverview));
    assert!(frame(&mut o.app).contains("Reconcile now"));
    let model = o.app.model(80);
    let at = model
        .rows()
        .iter()
        .position(|r| r.key == "reconcile")
        .unwrap();
    for _ in 0..at {
        press(&mut o.app, &[Key::Char('j')]);
    }
    let reconciled = |f: &Fixture| {
        f.calls()
            .iter()
            .any(|c| c.0 == "POST" && c.1 == "/reconcile")
    };
    press(&mut o.app, &[Key::Enter]);
    assert!(!reconciled(&o.fixture), "the confirmation comes first");
    let asking = frame(&mut o.app);
    assert!(o.app.modal.is_some(), "{asking}");
    assert!(asking.contains(&t("tui.edge.reconcile")), "{asking}");
    press(&mut o.app, &[Key::Char('y')]);
    assert!(reconciled(&o.fixture), "{:?}", o.fixture.calls());
}

fn login_text(login: &Login) -> String {
    let area = Rect::new(0, 0, COLS, ROWS);
    let mut buf = Buffer::empty(area);
    login.draw(area, &mut buf, theme());
    text_of(&buf)
}

#[test]
fn login_shows_the_fingerprint_in_groups_and_takes_a_token_and_a_key() {
    let mut login = Login::new(
        "https://mattbox.local".into(),
        HostKind::Box,
        "3F2A9C1E0B7D44A1C2D3E4F5061728394A5B6C7D".into(),
        vec![
            KeyChoice {
                fingerprint: "A".repeat(40),
                user_ids: vec!["Matus <matus@example.org>".into()],
            },
            KeyChoice {
                fingerprint: "C".repeat(40),
                user_ids: vec!["LosOS CI <dasmatus/losos>".into()],
            },
        ],
        false,
        None,
    );
    let first = login_text(&login);
    assert!(first.contains("3F2A 9C1E 0B7D 44A1 C2D3"), "{first}");
    assert!(first.contains("E4F5 0617 2839 4A5B 6C7D"), "{first}");
    clean(&first, "login");
    login.key(Key::Char('y'));
    let token = format!("losos_pat_0a1b2c3d_{}", "a".repeat(40));
    login.paste(&format!("{token}\r"));
    let picking = login_text(&login);
    assert!(!picking.contains(&token), "the token is not echoed");
    assert!(picking.contains("LosOS CI <dasmatus/losos>"), "{picking}");
    login.key(Key::Char('j'));
    login.key(Key::Enter);
    assert_eq!(
        login.outcome,
        Some(Some(LoginAnswer {
            token,
            key_fingerprint: "C".repeat(40),
        }))
    );
}

#[test]
fn login_answers_none_when_the_fingerprint_is_refused() {
    let mut login = Login::new(
        "https://edge.example".into(),
        HostKind::Edge,
        "3F2A9C1E0B7D44A1C2D3E4F5061728394A5B6C7D".into(),
        vec![],
        false,
        None,
    );
    assert!(login_text(&login).contains("3F2A"));
    login.key(Key::Char('n'));
    assert_eq!(login.outcome, Some(None));
}

#[test]
fn a_failed_request_shows_the_hosts_sentence() {
    fn broken(_: &str, path: &str, _: &Value) -> (u16, Value) {
        if path == "/api/backup" {
            (500, json!({ "error": "restic is not installed" }))
        } else {
            box_route("GET", path, &Value::Null)
        }
    }
    let fixture = Arc::new(Fixture::new(broken));
    let mut app = App::new(host(HostKind::Box), fixture);
    app.open_at(PaneId::Backup);
    app.settle();
    let text = frame(&mut app);
    assert!(text.contains("restic is not installed"), "{text}");
}
