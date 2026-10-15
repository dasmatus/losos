//! The full-screen view's state and what each key does: the web UI's shell
//! in a terminal.
//!
//! ```text
//! header   the salmon, LosOS, the host's name and how it is doing
//! sidebar  the panes, in the web UI's order; `/` searches them the way the
//!          web UI's sidebar search does
//! pane     the open pane's groups and rows; the cursor moves over the rows
//! bar      the web UI's Apply bar: pending changes, or the rebuild running
//! footer   the keys
//! ```
//!
//! Settings changes stay a draft until Apply, as in the browser, and Apply
//! asks before it starts a rebuild. Every request runs on a thread of its
//! own and reports back over a channel, so the screen never waits on the
//! network.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::form::{Form, Rebuild, SettingKey, SettingValue, Settings};
use super::model::{
    layout, Confirm, ConfirmSpec, Control, EditTarget, Job, Line, PaneModel, Prepare, Tone,
};
use super::panes::{
    self, chart_path, install_confirm, parse_hits, search_path, Ctx, PaneId, SearchState,
};
use super::resources::{Resource, Resources};
use crate::envelope::Transport;
use crate::i18n::{format_bytes, t};
use crate::tr;
use crate::vault::{host_name, HostKind, HostRecord};

/// A key, as the shell handles it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Esc,
    Tab,
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Backspace,
    /// Ctrl-U: clear a field.
    Clear,
    /// Ctrl-C.
    Interrupt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Pane,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Then {
    Job(Job),
    Apply,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modal {
    Confirm {
        spec: ConfirmSpec,
        then: Then,
    },
    Edit {
        prompt: String,
        masked: bool,
        value: String,
        target: EditTarget,
    },
    Help,
    Busy(String),
}

/// A line at the foot of the screen that goes away after a while.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub tone: Tone,
    pub title: String,
    pub text: String,
}

const NOTICE_FOR: Duration = Duration::from_secs(8);
const POLL_EVERY: Duration = Duration::from_secs(2);

type Fetch = Result<Value, (String, Option<u16>)>;

pub enum Msg {
    Fetched(String, Fetch),
    Settings {
        result: Result<(Settings, Value), String>,
        /// The first load also asks whether a rebuild is running.
        first_status: Option<Option<Value>>,
    },
    Status(Option<Value>),
    Applied(Result<String, String>),
    Done {
        said: Result<Option<String>, String>,
        reload: Vec<&'static str>,
    },
    Prepared(Result<(ConfirmSpec, Job), String>),
    Searched(SearchState),
}

pub struct App {
    pub host: HostRecord,
    transport: Arc<dyn Transport>,
    pub panes: Vec<PaneId>,
    pub res: Resources,
    pub form: Form,
    pub search: SearchState,
    pub pane: PaneId,
    pub focus: Focus,
    pub side_cursor: usize,
    pub cursor: usize,
    pub scroll: usize,
    /// The sidebar's search, while it is open.
    pub filter: Option<String>,
    pub modal: Option<Modal>,
    pub notice: Option<(Notice, Instant)>,
    pub quit: bool,
    /// Unix seconds to draw "ago" from; the clock when absent.
    pub now: Option<f64>,
    /// What the last frame laid out: the pane's lines and rows, and how many
    /// lines the pane had room for.
    pub(crate) laid: (Vec<Line>, usize),
    pub(crate) body_rows: usize,
    next_poll: Option<Instant>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    pending: usize,
}

impl App {
    pub fn new(host: HostRecord, transport: Arc<dyn Transport>) -> App {
        let (tx, rx) = channel();
        let panes = match host.kind {
            HostKind::Box => PaneId::BOX.to_vec(),
            HostKind::Edge => PaneId::EDGE.to_vec(),
        };
        let mut app = App {
            pane: panes[0],
            host,
            transport,
            panes,
            res: Resources::default(),
            form: Form::default(),
            search: SearchState::Idle,
            focus: Focus::Sidebar,
            side_cursor: 0,
            cursor: 0,
            scroll: 0,
            filter: None,
            modal: None,
            notice: None,
            quit: false,
            now: None,
            laid: (Vec::new(), 0),
            body_rows: 20,
            next_poll: None,
            tx,
            rx,
            pending: 0,
        };
        if app.is_box() {
            app.load_settings(true);
            app.want(&["/api/health", "/api/status"]);
        } else {
            // The header's status comes from /overview, whichever pane opens.
            app.want(&["/overview"]);
        }
        app.want(app.pane.needs());
        app
    }

    /// Open on `pane`, with the cursor in it, as `--pane` and the
    /// screenshots do.
    pub fn open_at(&mut self, pane: PaneId) {
        if let Some(i) = self.panes.iter().position(|p| *p == pane) {
            self.side_cursor = i;
            self.open_pane(pane);
            self.focus = Focus::Pane;
        }
    }

    pub fn is_box(&self) -> bool {
        self.host.kind == HostKind::Box
    }

    // ── Background work ──

    fn spawn(&mut self, work: impl FnOnce(&dyn Transport) -> Msg + Send + 'static) {
        let transport = self.transport.clone();
        let tx = self.tx.clone();
        self.pending += 1;
        std::thread::spawn(move || {
            let msg = work(&*transport);
            let _ = tx.send(msg);
        });
    }

    fn fetch(&mut self, path: String) {
        self.res.mark_asked(&path);
        self.spawn(move |t| {
            let result = t
                .request("GET", &path, Value::Null)
                .map_err(|p| (p.message, None))
                .and_then(|a| {
                    if a.ok() {
                        Ok(a.body)
                    } else {
                        Err((a.error_text(), Some(a.status)))
                    }
                });
            Msg::Fetched(path, result)
        });
    }

    /// Fetch every path not yet fetched.
    pub fn want(&mut self, paths: &[&str]) {
        for path in self.res.take_unasked(paths) {
            self.fetch(path);
        }
    }

    /// Fetch these again.
    pub fn reload(&mut self, paths: &[&str]) {
        for path in paths {
            self.fetch((*path).to_string());
        }
    }

    fn load_settings(&mut self, first: bool) {
        self.spawn(move |t| {
            let result = t
                .get("/api/settings")
                .map_err(|p| p.message)
                .and_then(|v| serde_json::from_value::<Settings>(v).map_err(|e| e.to_string()))
                .map(|settings| {
                    let options = t
                        .get("/api/options")
                        .ok()
                        .filter(|o| o["options"].is_array() && o["stray"].is_array())
                        .unwrap_or_else(|| {
                            json!({"available": false, "version": 0, "options": [], "stray": []})
                        });
                    (settings, options)
                });
            let first_status = first.then(|| t.get("/api/status").ok());
            Msg::Settings {
                result,
                first_status,
            }
        });
    }

    fn poll_later(&mut self) {
        self.next_poll = Some(Instant::now() + POLL_EVERY);
    }

    /// Take in whatever the background work has finished.
    pub fn drain(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            self.pending = self.pending.saturating_sub(1);
            self.handle(msg);
        }
    }

    /// Wait for every request in flight, for tests and the screenshots.
    pub fn settle(&mut self) {
        while self.pending > 0 {
            match self.rx.recv_timeout(Duration::from_secs(30)) {
                Ok(msg) => {
                    self.pending -= 1;
                    self.handle(msg);
                }
                Err(_) => break,
            }
        }
    }

    /// The clock: notices expire, the rebuild is polled.
    pub fn tick(&mut self, now: Instant) {
        if self
            .notice
            .as_ref()
            .is_some_and(|(_, at)| now.duration_since(*at) >= NOTICE_FOR)
        {
            self.notice = None;
        }
        if self.next_poll.is_some_and(|at| now >= at) {
            self.next_poll = None;
            self.spawn(|t| Msg::Status(t.get("/api/status").ok()));
        }
    }

    pub fn say(&mut self, tone: Tone, title: impl Into<String>, text: impl Into<String>) {
        self.notice = Some((
            Notice {
                tone,
                title: title.into(),
                text: text.into(),
            },
            Instant::now(),
        ));
    }

    fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::Fetched(path, result) => {
                let resource = match result {
                    Ok(v) => Resource::Ok(v),
                    Err((message, status)) => Resource::Error { message, status },
                };
                self.res.put(path, resource);
            }
            Msg::Settings {
                result,
                first_status,
            } => {
                match result {
                    Ok((settings, options)) => self.form.loaded(settings, options),
                    Err(e) => self.form.load_error = Some(e),
                }
                if let Some(Some(status)) = first_status {
                    if status["state"] == "building" {
                        self.form.job = status["job"].as_str().map(str::to_string);
                        self.form.rebuild = Some(Rebuild {
                            title: t("settings.form.joinedTitle"),
                            message: message_or(&status, "settings.form.joinedMessage"),
                        });
                        self.poll_later();
                    }
                }
            }
            Msg::Status(None) => self.poll_later(),
            Msg::Status(Some(status)) => self.on_status(status),
            Msg::Applied(Ok(job)) => {
                self.form.job = Some(job).filter(|j| !j.is_empty());
                self.poll_later();
            }
            Msg::Applied(Err(e)) => {
                self.form.rebuild = None;
                self.say(Tone::Crit, t("settings.form.didNotStart"), e);
            }
            Msg::Done { said, reload } => {
                if matches!(self.modal, Some(Modal::Busy(_))) {
                    self.modal = None;
                }
                self.reload(&reload);
                match said {
                    Ok(Some(said)) => self.say(Tone::Ok, said, ""),
                    Ok(None) => {}
                    Err(e) => self.say(Tone::Crit, t("tui.failed"), e),
                }
            }
            Msg::Prepared(Ok((spec, job))) => {
                self.modal = Some(Modal::Confirm {
                    spec,
                    then: Then::Job(job),
                });
            }
            Msg::Prepared(Err(e)) => {
                self.modal = None;
                self.say(Tone::Crit, t("tui.failed"), e);
            }
            Msg::Searched(state) => self.search = state,
        }
    }

    fn on_status(&mut self, status: Value) {
        let job = status["job"].as_str();
        if let (Some(job), Some(ours)) = (job, &self.form.job) {
            if job != ours {
                self.poll_later();
                return;
            }
        }
        match status["state"].as_str() {
            Some("building") | Some("idle") => {
                if status["state"] == "building" {
                    self.form.rebuild = Some(Rebuild {
                        title: t("settings.form.applyingYours"),
                        message: message_or(&status, "settings.form.buildingMessage"),
                    });
                }
                self.poll_later();
            }
            state => {
                self.form.job = None;
                self.form.rebuild = None;
                if state == Some("done") {
                    self.say(
                        Tone::Ok,
                        t("settings.form.doneTitle"),
                        message_or(&status, "settings.form.doneMessage"),
                    );
                    self.load_settings(false);
                } else {
                    self.say(
                        Tone::Crit,
                        t("settings.form.failedTitle"),
                        message_or(&status, "settings.form.failedMessage"),
                    );
                }
            }
        }
    }

    // ── What the screen shows ──

    pub fn ctx(&self) -> Ctx<'_> {
        Ctx {
            host: &self.host,
            res: &self.res,
            form: &self.form,
            search: &self.search,
            now: self.now.unwrap_or_else(|| losos_access::now_secs() as f64),
        }
    }

    pub fn model(&self, cols: usize) -> PaneModel {
        panes::build(self.pane, &self.ctx(), cols)
    }

    /// The panes the sidebar lists: all, or the search's matches.
    pub fn shown_panes(&self) -> Vec<PaneId> {
        match &self.filter {
            None => self.panes.clone(),
            Some(f) => self
                .panes
                .iter()
                .copied()
                .filter(|p| p.matches(f))
                .collect(),
        }
    }

    pub fn title(&self) -> String {
        match (&self.form.saved, self.host.kind) {
            (Some(s), HostKind::Box) => s.host_name.clone(),
            _ => host_name(&self.host.url),
        }
    }

    /// The header's status line.
    pub fn status(&self) -> (String, Tone) {
        if let Some(r) = &self.form.rebuild {
            return (r.title.clone(), Tone::Accent);
        }
        let path = if self.is_box() {
            "/api/health"
        } else {
            "/overview"
        };
        match self.res.get(path) {
            Resource::Loading => (t("home.header.checking"), Tone::Muted),
            Resource::Ok(v) if !self.is_box() || v["ok"] == Value::Bool(true) => {
                (t("home.header.answering"), Tone::Ok)
            }
            _ => (t("home.header.notAnswering"), Tone::Crit),
        }
    }

    /// Lay the open pane out for a frame `cols` wide with `rows` lines of
    /// room, and keep the cursor's row in view.
    pub fn lay_out(&mut self, cols: usize, rows: usize) {
        let model = self.model(cols);
        let cursor = (self.focus == Focus::Pane).then_some(self.cursor);
        let (lines, count) = layout(&model, cols, cursor);
        self.body_rows = rows;
        if count > 0 && self.cursor >= count {
            self.cursor = count - 1;
        }
        let first = lines.iter().position(|l| l.row == Some(self.cursor));
        let last = lines.iter().rposition(|l| l.row == Some(self.cursor));
        if let (Some(first), Some(last)) = (first, last) {
            if first < self.scroll {
                let first_row_line = lines.iter().position(|l| l.row.is_some());
                self.scroll = if Some(first) == first_row_line {
                    0
                } else {
                    first
                };
            } else if last >= self.scroll + rows {
                self.scroll = first.min(last + 1 - rows);
            }
        }
        self.scroll = self.scroll.min(lines.len().saturating_sub(1));
        self.laid = (lines, count);
    }

    // ── Keys ──

    fn open_pane(&mut self, pane: PaneId) {
        self.pane = pane;
        self.cursor = 0;
        self.scroll = 0;
        self.want(pane.needs());
    }

    fn ask_apply(&mut self) {
        if !self.is_box() || !self.form.dirty() || self.form.rebuild.is_some() {
            return;
        }
        if !self.form.valid() {
            self.say(Tone::Crit, t("settings.applyBar.invalid"), "");
            return;
        }
        self.modal = Some(Modal::Confirm {
            spec: ConfirmSpec {
                title: t("tui.apply.title"),
                body: vec![
                    tr!("tui.apply.body", count = self.form.change_count()),
                    t("settings.applyBar.valid"),
                ],
                confirm: t("settings.applyBar.apply"),
                cancel: t("tui.cancel"),
            },
            then: Then::Apply,
        });
    }

    fn apply(&mut self) {
        let Some(body) = self.form.apply_body() else {
            return;
        };
        self.form.rebuild = Some(Rebuild {
            title: t("settings.form.applyingYours"),
            message: t("settings.form.starting"),
        });
        self.spawn(move |t| {
            Msg::Applied(
                t.post("/api/apply", Value::String(body))
                    .map(|ack| ack["job"].as_str().unwrap_or_default().to_string())
                    .map_err(|p| p.message),
            )
        });
    }

    fn run(&mut self, then: Then) {
        match then {
            Then::Apply => self.apply(),
            Then::Job(job) => {
                self.modal = Some(Modal::Busy(t("ui.working")));
                self.start(job);
            }
        }
    }

    fn start(&mut self, job: Job) {
        self.spawn(move |t| {
            let (said, reload) = match run_job(&job, t) {
                Ok((said, reload)) => (Ok(said), reload),
                Err(e) => (Err(e), Vec::new()),
            };
            Msg::Done { said, reload }
        });
    }

    fn act(&mut self) {
        let model = self.model(80);
        let rows = model.rows();
        let Some(control) = rows.get(self.cursor).and_then(|r| r.control.clone()) else {
            return;
        };
        if control.disabled() {
            return;
        }
        match control {
            Control::Toggle { on, key, .. } => self.form.set(key, SettingValue::Bool(!on)),
            Control::Choice {
                value,
                options,
                key,
                ..
            } => {
                let i = options.iter().position(|(v, _)| *v == value);
                let next = i.map_or(0, |i| (i + 1) % options.len());
                if let Some((v, _)) = options.get(next) {
                    self.form.set(key, SettingValue::Text(v.clone()));
                }
            }
            Control::Edit {
                value,
                prompt,
                masked,
                target,
                ..
            } => {
                self.modal = Some(Modal::Edit {
                    prompt,
                    masked,
                    value: if masked { String::new() } else { value },
                    target,
                });
            }
            Control::Run { confirm, job, .. } => match (confirm, job) {
                (None, Some(job)) => self.run(Then::Job(job)),
                (Some(Confirm::Ready(spec)), Some(job)) => {
                    self.modal = Some(Modal::Confirm {
                        spec,
                        then: Then::Job(job),
                    })
                }
                (Some(Confirm::Prepare(Prepare::Install(chart))), _) => {
                    let shared = self
                        .res
                        .get("/api/apps")
                        .data()
                        .is_some_and(|a| a["sharedAvailable"] == Value::Bool(true));
                    self.modal = Some(Modal::Busy(t("ui.working")));
                    self.spawn(move |t| {
                        Msg::Prepared(t.get(&chart_path(&chart)).map_err(|p| p.message).and_then(
                            |info| {
                                let release = info["release"]
                                    .as_str()
                                    .ok_or_else(|| "no release".to_string())?
                                    .to_string();
                                let spec = install_confirm(&release, shared);
                                Ok((
                                    spec,
                                    Job::InstallApp(super::model::InstallPlan {
                                        release,
                                        chart,
                                        run_as: "notshared".into(),
                                    }),
                                ))
                            },
                        ))
                    });
                }
                _ => {}
            },
        }
    }

    fn submit(&mut self, target: EditTarget, value: String) {
        match target {
            EditTarget::Setting(SettingKey::ApachePort) => {
                let port = value.trim().parse::<i64>().ok();
                self.form
                    .set(SettingKey::ApachePort, SettingValue::Port(port));
            }
            EditTarget::Setting(key) => self
                .form
                .set(key, SettingValue::Text(value.trim().to_string())),
            EditTarget::AppSearch => {
                let query = value.trim().to_string();
                if query.is_empty() {
                    self.search = SearchState::Idle;
                    return;
                }
                self.search = SearchState::Searching(query.clone());
                self.spawn(move |t| Msg::Searched(search(t, &query)));
            }
            EditTarget::Secret(name) => self.start(Job::SetSecret { name, value }),
        }
    }

    /// Text pasted into the terminal.
    pub fn paste(&mut self, text: &str) {
        if let Some(Modal::Edit { value, .. }) = &mut self.modal {
            value.push_str(&text.replace(['\r', '\n'], ""));
            return;
        }
        for c in text.chars() {
            self.key(match c {
                '\r' | '\n' => Key::Enter,
                '\t' => Key::Tab,
                c => Key::Char(c),
            });
        }
    }

    pub fn key(&mut self, key: Key) {
        if key == Key::Interrupt {
            self.quit = true;
            return;
        }
        if self.notice.is_some() && key != Key::Enter {
            self.notice = None;
        }
        if let Some(modal) = self.modal.take() {
            self.modal_key(modal, key);
            return;
        }
        if let Some(filter) = self.filter.clone() {
            self.filter_key(filter, key);
            return;
        }
        match key {
            Key::Char('q') => {
                self.quit = true;
                return;
            }
            Key::Char('?') => {
                self.modal = Some(Modal::Help);
                return;
            }
            Key::Char('/') => {
                self.filter = Some(String::new());
                self.side_cursor = 0;
                self.focus = Focus::Sidebar;
                return;
            }
            Key::Char('r') => {
                let mut paths: Vec<&str> = if self.is_box() {
                    vec!["/api/health", "/api/status"]
                } else {
                    vec![]
                };
                paths.extend(self.pane.needs());
                paths.dedup();
                self.reload(&paths);
                if self.is_box() {
                    self.load_settings(false);
                }
                self.say(Tone::Accent, t("tui.refreshed"), "");
                return;
            }
            Key::Char('a') => {
                self.ask_apply();
                return;
            }
            Key::Char('d') if self.form.dirty() && self.form.rebuild.is_none() => {
                self.form.discard();
                return;
            }
            Key::Tab => {
                self.focus = match self.focus {
                    Focus::Sidebar => Focus::Pane,
                    Focus::Pane => Focus::Sidebar,
                };
                return;
            }
            _ => {}
        }
        let up = matches!(key, Key::Up | Key::Char('k'));
        let down = matches!(key, Key::Down | Key::Char('j'));
        if self.focus == Focus::Sidebar {
            if up {
                self.side_cursor = self.side_cursor.saturating_sub(1);
            } else if down {
                self.side_cursor = (self.side_cursor + 1).min(self.panes.len() - 1);
            } else if matches!(
                key,
                Key::Enter | Key::Right | Key::Char('l') | Key::Char(' ')
            ) {
                if let Some(pane) = self.panes.get(self.side_cursor).copied() {
                    self.open_pane(pane);
                    self.focus = Focus::Pane;
                }
            }
            return;
        }
        let (lines, count) = (&self.laid.0, self.laid.1);
        let last_row = count.saturating_sub(1);
        if up {
            self.cursor = self.cursor.saturating_sub(1);
        } else if down && self.cursor >= last_row {
            // Past the last row: scroll on to the captions and tables below.
            self.scroll = (self.scroll + 1).min(lines.len().saturating_sub(self.body_rows));
        } else if down {
            self.cursor = (self.cursor + 1).min(last_row);
        } else if key == Key::PageUp {
            self.cursor = self.cursor.saturating_sub(8);
        } else if key == Key::PageDown {
            self.cursor = (self.cursor + 8).min(last_row);
        } else if matches!(key, Key::Left | Key::Char('h') | Key::Esc) {
            self.focus = Focus::Sidebar;
        } else if matches!(key, Key::Enter | Key::Char(' ')) {
            self.act();
        }
    }

    fn modal_key(&mut self, modal: Modal, key: Key) {
        match modal {
            Modal::Busy(_) => self.modal = Some(modal),
            Modal::Help => {}
            Modal::Confirm { spec, then } => match key {
                Key::Esc | Key::Char('n') => {}
                Key::Enter | Key::Char('y') => self.run(then),
                _ => self.modal = Some(Modal::Confirm { spec, then }),
            },
            Modal::Edit {
                prompt,
                masked,
                mut value,
                target,
            } => match key {
                Key::Enter => self.submit(target, value),
                Key::Esc => {}
                other => {
                    match other {
                        Key::Backspace => {
                            value.pop();
                        }
                        Key::Clear => value.clear(),
                        Key::Char(c) => value.push(c),
                        _ => {}
                    }
                    self.modal = Some(Modal::Edit {
                        prompt,
                        masked,
                        value,
                        target,
                    });
                }
            },
        }
    }

    fn filter_key(&mut self, mut filter: String, key: Key) {
        let shown = self.shown_panes();
        match key {
            Key::Esc => self.filter = None,
            Key::Enter => {
                self.filter = None;
                if let Some(pane) = shown.get(self.side_cursor.min(shown.len().saturating_sub(1))) {
                    self.side_cursor = self.panes.iter().position(|p| p == pane).unwrap_or(0);
                    self.open_pane(*pane);
                    self.focus = Focus::Pane;
                }
            }
            Key::Up => self.side_cursor = self.side_cursor.saturating_sub(1),
            Key::Down => {
                self.side_cursor = (self.side_cursor + 1).min(shown.len().saturating_sub(1))
            }
            Key::Backspace => {
                filter.pop();
                self.filter = Some(filter);
            }
            Key::Char(c) => {
                filter.push(c);
                self.filter = Some(filter);
                self.side_cursor = 0;
            }
            _ => {}
        }
    }
}

fn message_or(status: &Value, key: &str) -> String {
    match status["message"].as_str() {
        Some(m) if !m.is_empty() => m.to_string(),
        _ => t(key),
    }
}

/// The Apps pane's search, against GET /api/apps/search.
pub fn search(t: &dyn Transport, query: &str) -> SearchState {
    match t.request("GET", &search_path(query), Value::Null) {
        Err(p) => SearchState::Failed(p.message),
        Ok(a) if a.status == 404 => SearchState::Unsupported,
        Ok(a) if !a.ok() => SearchState::Failed(a.error_text()),
        Ok(a) => {
            let (sources, apps) = parse_hits(&a.body);
            SearchState::Results {
                query: query.to_string(),
                sources,
                apps,
            }
        }
    }
}

/// Run one job against the host: what to say when it is done, and which
/// reads to fetch again.
pub fn run_job(
    job: &Job,
    t: &dyn Transport,
) -> Result<(Option<String>, Vec<&'static str>), String> {
    let post = |path: &str, body: Value| t.post(path, body).map_err(|p| p.message);
    Ok(match job {
        Job::Grow => {
            let grown = post("/api/grow", Value::Null)?;
            let said = if grown["grew"] == Value::Bool(true) {
                tr!(
                    "tui.storage.grew",
                    size = format_bytes(grown["afterBytes"].as_f64())
                )
            } else {
                crate::i18n::t("tui.storage.notGrown")
            };
            (Some(said), vec!["/api/storage"])
        }
        Job::RemoveApp(release) => {
            post("/api/apps/remove", json!({ "release": release }))?;
            (
                Some(tr!("tui.apps.removing", name = release.clone())),
                vec!["/api/apps"],
            )
        }
        Job::InstallApp(plan) => {
            post(
                "/api/apps/install",
                json!({
                    "release": plan.release,
                    "chart": plan.chart,
                    "runAs": plan.run_as,
                    "values": {},
                    "valuesYaml": null,
                }),
            )?;
            (
                Some(tr!("install.startedTitle", name = plan.release.clone())),
                vec!["/api/apps"],
            )
        }
        Job::SyncConfig => {
            post("/api/config/sync", Value::Null)?;
            (
                Some(crate::i18n::t("tui.history.synced")),
                vec!["/api/config"],
            )
        }
        Job::RunBackup => {
            post("/api/backup/run", Value::Null)?;
            (
                Some(crate::i18n::t("panes.backup.runs.running")),
                vec!["/api/backup"],
            )
        }
        Job::Reconcile => {
            let answer = post("/reconcile", Value::Null)?;
            let said = panes::str_of(&answer["message"])
                .map(str::to_string)
                .unwrap_or_else(|| crate::i18n::t("tui.edge.reconciled"));
            (Some(said), vec!["/overview"])
        }
        Job::SetSecret { name, value } => {
            post("/secrets", json!({ "name": name, "value": value }))?;
            (
                Some(tr!("tui.edge.secretSet", name = name.clone())),
                vec!["/secrets"],
            )
        }
    })
}
