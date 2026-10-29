//! The problems plugin: what went wrong on the box, written up as reports
//! the owner reads and, if they choose, sends to LosOS's issue tracker.
//!
//! Every minute (`scan`) it reads the journal entries since the last scan
//! that say something went wrong: systemd's messages about a unit whose
//! process crashed or that ended in a failed result, and the kernel's about
//! a process it killed for memory or that crashed outside any unit. Each
//! becomes a problem kept by its fingerprint in `problems.json`, so a
//! service that fails every night is one problem seen many times. A failed
//! unit whose log says what the handbook already explains (`known.json` of
//! `losos-report`) is marked documented, and the page links the handbook
//! instead of offering a report.
//!
//! Nothing leaves the box by itself. `GET /api/problems/{id}` writes the
//! report from the box's own records, with addresses, the box's name and
//! the people on it taken out, and the page shows the whole text. Only
//! `POST …/send` starts the sending, as a job: GitHub's device flow, whose
//! code the page shows and the owner enters at github.com/login/device. The
//! token lives in the job's memory for that one report.
//!
//! ```text
//! GET  /api/problems              the list, and where reports go
//! GET  /api/problems/{id}         the report as it would be sent
//! POST /api/problems/{id}/send    { "note": "…" } → 202, the job starts
//! GET  /api/problems/{id}/send    where the sending stands
//! POST /api/problems/{id}/cancel  stop waiting for the code
//! POST /api/problems/{id}/dismiss hide it until it happens again
//! ```

pub mod model;

use losos_plugin::{cache, clock, config, dispatch, exec, fs, http, jobs, lock, log};
use losos_plugin::{Plugin, Request, Response};
use losos_report::github::{self, Call, Progress, Transport};
use losos_report::problem::{self, Evidence, Kind};
use losos_report::{Report, System, SystemFiles, Target};
use model::{Sent, State};
use serde_json::{json, Value};

/// What the owner adds is quoted under this, before the report.
const NOTE_HEADING: &str = "From the box's owner:";

/// How long a written report and a sending's progress stay in the cache.
const KEEP_SECS: u32 = 3600;

fn state_file() -> String {
    config::get_or("stateFile", "/var/lib/losos/problems.json")
}

fn load() -> State {
    let path = state_file();
    match fs::read_optional(&path) {
        Ok(bytes) => State::parse(bytes.as_deref()),
        Err(e) => {
            log::warn(&format!("cannot read {path}: {e}"));
            State::default()
        }
    }
}

fn save(state: &State) -> losos_plugin::Result<()> {
    let bytes = serde_json::to_vec_pretty(state).unwrap_or_default();
    fs::write(&state_file(), &bytes, fs::WriteOptions::secret())
}

/// Where reports go, when this box has a GitHub App to sign in with: its
/// client ID is lososd's systemd credential `report-client-id`, read from
/// the unit's credential directory (modules/problems.nix).
fn target() -> Option<Target> {
    let get = |k: &str| config::get(k);
    Target::from_settings(
        get("repository").as_deref(),
        client_id().as_deref(),
        get("login").as_deref(),
        get("api").as_deref(),
    )
}

fn client_id() -> Option<String> {
    let path = config::get("clientIdFile").filter(|p| !p.is_empty())?;
    match fs::read_string_optional(&path) {
        Ok(text) => text.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty()),
        Err(e) => {
            log::warn(&format!("cannot read {path}: {e}"));
            None
        }
    }
}

fn run(program: &str, args: &[String]) -> Option<String> {
    match exec::Command::new(program)
        .args(args.iter().map(String::as_str))
        .env("SYSTEMD_COLORS", "0")
        .timeout_ms(20_000)
        .output()
    {
        Ok(out) if out.success() => Some(out.stdout_string()),
        Ok(out) => {
            log::warn(&format!("{program} failed: {}", out.stderr_last_line()));
            None
        }
        Err(e) => {
            log::warn(&format!("{program}: {e}"));
            None
        }
    }
}

// ── The scan ────────────────────────────────────────────────────────────

/// The journal entries since `cursor` that can name a problem, oldest
/// first, and the cursor after the last one.
fn read_journal(cursor: Option<&str>) -> (Vec<Value>, Option<String>) {
    let mut args: Vec<String> = ["--no-pager", "--output=json", "--quiet"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    match cursor {
        Some(c) => args.push(format!("--after-cursor={c}")),
        // The first scan looks back a day: a box that just got this plugin
        // has its last night's problems still to tell.
        None => args.push("--since=-1d".to_owned()),
    }
    // Two of systemd's messages, or anything the kernel said. The same
    // field twice is an OR, and `+` joins the two groups.
    args.extend([
        format!("MESSAGE_ID={}", model::PROCESS_EXIT),
        format!("MESSAGE_ID={}", model::FAILURE_RESULT),
        "+".to_owned(),
        "_TRANSPORT=kernel".to_owned(),
    ]);
    let Some(text) = run("journalctl", &args) else {
        return (Vec::new(), cursor.map(str::to_owned));
    };
    let entries: Vec<Value> = text
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let last = entries
        .iter()
        .rev()
        .find_map(|e| e["__CURSOR"].as_str().map(str::to_owned))
        .or_else(|| cursor.map(str::to_owned));
    (entries, last)
}

fn scan() {
    let Ok(_guard) = lock::acquire("problems") else {
        return;
    };
    let mut state = load();
    let (entries, cursor) = read_journal(state.cursor.as_deref());
    let events = model::distinct(entries.iter().filter_map(model::event_of).collect());
    let new = state.record(&events);
    for id in &new {
        let Some(kept) = state.get(id) else { continue };
        let problem = kept.problem();
        if problem.kind == Kind::Failed {
            let evidence = gather(&problem, kept.first, kept.last, kept.count);
            let documented = problem::documented(&problem, &evidence).map(|k| k.id);
            if let Some(kept) = state.get_mut(id) {
                kept.documented = documented;
            }
        }
        log::info(&format!("new problem {id}: {}", problem.title()));
    }
    let changed = !events.is_empty() || cursor != state.cursor;
    state.cursor = cursor;
    if changed {
        if let Err(e) = save(&state) {
            log::error(&format!("cannot write {}: {e}", state_file()));
        }
    }
}

// ── The report ──────────────────────────────────────────────────────────

/// The properties of `unit` that say how it is and how it ended.
fn unit_state(unit: &str, user: bool) -> Vec<String> {
    let mut args = Vec::new();
    if user {
        args.push("--user".to_owned());
    }
    args.extend(
        [
            "show",
            "--no-pager",
            "--property=ActiveState,SubState,Result,ExecMainCode,ExecMainStatus,NRestarts,MemoryPeak,ActiveEnterTimestamp,InactiveEnterTimestamp",
            "--",
            unit,
        ]
        .into_iter()
        .map(str::to_owned),
    );
    run("systemctl", &args)
        .unwrap_or_default()
        .lines()
        .filter(|l| l.split_once('=').is_some_and(|(_, v)| !v.is_empty()))
        .map(str::to_owned)
        .collect()
}

/// The journal lines that go with a problem: its unit's from a few minutes
/// before it first happened until just after the last time, or the
/// kernel's around the last time.
fn gather(problem: &problem::Problem, first: u64, last: u64, count: u64) -> Evidence {
    let mut args: Vec<String> = [
        "--no-pager",
        "--quiet",
        "--output=short-iso",
        "--no-hostname",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    args.push(format!("--lines={}", problem::JOURNAL_LINES));
    let (since, until) = match &problem.unit {
        Some(unit) => {
            args.push(format!("--unit={unit}"));
            (first.saturating_sub(300), last + 30)
        }
        None => {
            args.push("--dmesg".to_owned());
            (last.saturating_sub(60), last + 5)
        }
    };
    args.push(format!("--since=@{since}"));
    args.push(format!("--until=@{until}"));
    let journal = run("journalctl", &args)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect();
    Evidence {
        state: problem
            .unit
            .as_deref()
            .map(|u| unit_state(u, false))
            .unwrap_or_default(),
        journal,
        count,
        first,
        last,
    }
}

fn read_text(path: &str) -> String {
    fs::read_string_optional(path)
        .ok()
        .flatten()
        .unwrap_or_default()
}

fn this_system() -> System {
    let dmi = config::get_or("dmiDir", "/sys/devices/virtual/dmi/id");
    System::from_files(
        &SystemFiles {
            os_release: format!("IMAGE_VERSION={}", config::get_or("release", "")),
            cpuinfo: read_text("/proc/cpuinfo"),
            meminfo: read_text("/proc/meminfo"),
            vendor: read_text(&format!("{dmi}/sys_vendor")),
            product: read_text(&format!("{dmi}/product_name")),
            kernel: read_text("/proc/sys/kernel/osrelease"),
            efi: fs::exists("/sys/firmware/efi").unwrap_or(false),
        },
        "x86_64",
    )
}

/// What a report must not carry: the box's name and everyone on it, asked
/// of lososd as the caller.
fn private_names(req: &Request) -> Vec<String> {
    let mut names = vec![read_text("/proc/sys/kernel/hostname").trim().to_owned()];
    let ask = Request {
        method: "GET".into(),
        path: "/api/people".into(),
        route: String::new(),
        params: Vec::new(),
        query: String::new(),
        headers: Vec::new(),
        body: Vec::new(),
        caller: req.caller.clone(),
    };
    match dispatch::call(ask) {
        Ok(r) if r.status == 200 => names.extend(model::people_names(&r.json_value())),
        Ok(r) => log::warn(&format!("GET /api/people answered {}", r.status)),
        Err(e) => log::warn(&format!("GET /api/people: {e}")),
    }
    names
}

fn write_report(req: &Request, kept: &model::Kept) -> Report {
    let problem = kept.problem();
    let evidence = gather(&problem, kept.first, kept.last, kept.count);
    problem::build(&problem, &this_system(), &evidence, &private_names(req))
}

fn report_key(id: &str) -> String {
    format!("report/{id}")
}

fn cached_report(id: &str) -> Option<Report> {
    let v: Value = cache::get_json(&report_key(id))?;
    Some(Report {
        title: v["title"].as_str()?.to_owned(),
        body: v["body"].as_str()?.to_owned(),
        fingerprint: v["fingerprint"].as_str()?.to_owned(),
    })
}

// ── Sending ─────────────────────────────────────────────────────────────

fn progress_key(id: &str) -> String {
    format!("send/{id}")
}

fn cancel_key(id: &str) -> String {
    format!("cancel/{id}")
}

fn set_progress(id: &str, value: &Value) {
    cache::set_json(&progress_key(id), value, KEEP_SECS);
}

fn progress_of(id: &str) -> Value {
    cache::get_json(&progress_key(id)).unwrap_or_else(|| json!({ "state": "idle" }))
}

fn sending(id: &str) -> bool {
    matches!(
        progress_of(id)["state"].as_str(),
        Some("starting" | "sign-in")
    )
}

/// GitHub through the host's `http` import; waits a second at a time so a
/// cancel is seen soon.
struct HostTransport {
    id: String,
}

impl Transport for HostTransport {
    fn call(&mut self, call: &Call) -> Result<(u16, Value), String> {
        let request = match call {
            Call::Form { url, fields } => http::Request::post(url)
                .header("accept", "application/json")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(github::form(fields).into_bytes()),
            Call::Get { url, query, token } => {
                http::Request::get(&format!("{url}?{}", github::form(query)))
                    .header("accept", "application/vnd.github+json")
                    .header("authorization", &format!("Bearer {token}"))
            }
            Call::Post { url, body, token } => http::Request::post(url)
                .header("accept", "application/vnd.github+json")
                .header("authorization", &format!("Bearer {token}"))
                .json(body),
        };
        let answer = request
            .header("x-github-api-version", "2022-11-28")
            .header("user-agent", "losos-problems")
            .timeout_ms(30_000)
            .fetch(4 << 20)
            .map_err(|e| format!("GitHub could not be reached: {e}"))?;
        Ok((answer.status, answer.json().unwrap_or(Value::Null)))
    }

    fn wait(&mut self, secs: u64) -> bool {
        for _ in 0..secs {
            if cache::get(&cancel_key(&self.id)).is_some() {
                return false;
            }
            clock::sleep_ms(1000);
        }
        cache::get(&cancel_key(&self.id)).is_none()
    }
}

/// The job: sign in, file, and keep where it went.
fn send_job(input: &[u8]) {
    let job: Value = serde_json::from_slice(input).unwrap_or(Value::Null);
    let (Some(id), Some(title), Some(body), Some(fingerprint)) = (
        job["id"].as_str(),
        job["title"].as_str(),
        job["body"].as_str(),
        job["fingerprint"].as_str(),
    ) else {
        return;
    };
    let Some(target) = target() else {
        set_progress(
            id,
            &json!({ "state": "failed", "message": "This box does not report problems." }),
        );
        return;
    };
    let report = Report {
        title: title.to_owned(),
        body: body.to_owned(),
        fingerprint: fingerprint.to_owned(),
    };
    let mut transport = HostTransport { id: id.to_owned() };
    let result = github::send(&target, &report, body, &mut transport, &mut |p| match p {
        Progress::SignIn { uri, code, expires } => set_progress(
            id,
            &json!({ "state": "sign-in", "uri": uri, "code": code,
                     "expires": clock::now() + expires }),
        ),
        Progress::Sent { url, existing } => {
            set_progress(
                id,
                &json!({ "state": "sent", "url": url, "existing": existing }),
            );
            if let Ok(_guard) = lock::acquire("problems") {
                let mut state = load();
                if let Some(kept) = state.get_mut(id) {
                    kept.sent = Some(Sent {
                        url,
                        existing,
                        at: clock::now(),
                    });
                    if let Err(e) = save(&state) {
                        log::error(&format!("cannot write {}: {e}", state_file()));
                    }
                }
            }
        }
    });
    cache::remove(&cancel_key(id));
    if let Err(message) = result {
        let state = if message == "Cancelled." {
            "cancelled"
        } else {
            "failed"
        };
        set_progress(id, &json!({ "state": state, "message": message }));
        log::info(&format!("problem {id} not sent: {message}"));
    }
}

fn handle_send(req: &Request, id: &str) -> Response {
    let Some(target) = target() else {
        return Response::error(409, "This box does not report problems.");
    };
    let state = load();
    let Some(kept) = state.get(id) else {
        return Response::error(404, "No such problem.");
    };
    if kept.documented.is_some() {
        return Response::error(409, "The handbook already explains this problem.");
    }
    if sending(id) {
        return Response::error(409, "The report is already being sent.");
    }
    let report = cached_report(id).unwrap_or_else(|| write_report(req, kept));
    let note = req.json_value()["note"].as_str().unwrap_or("").to_owned();
    let body = losos_report::with_note(&report, NOTE_HEADING, &note);
    let input = json!({
        "id": id,
        "title": report.title,
        "body": body,
        "fingerprint": report.fingerprint,
    });
    cache::remove(&cancel_key(id));
    set_progress(id, &json!({ "state": "starting" }));
    match jobs::spawn("send", input.to_string().as_bytes()) {
        Ok(()) => Response::json(
            202,
            &json!({ "state": "starting", "repository": target.repo }),
        ),
        Err(e) => {
            cache::remove(&progress_key(id));
            Response::failed(&e)
        }
    }
}

fn handle_dismiss(id: &str) -> Response {
    let Ok(_guard) = lock::acquire("problems") else {
        return Response::error(503, "Busy, try again.");
    };
    let mut state = load();
    let Some(kept) = state.get_mut(id) else {
        return Response::error(404, "No such problem.");
    };
    kept.dismissed_at_count = Some(kept.count);
    let view = model::view(kept);
    match save(&state) {
        Ok(()) => Response::ok(&view),
        Err(e) => Response::failed(&e),
    }
}

pub struct ProblemsPlugin;

impl Plugin for ProblemsPlugin {
    fn handle(req: Request) -> Response {
        let id = req.param("id").unwrap_or_default().to_owned();
        match (req.method.as_str(), req.route.as_str()) {
            ("GET", "/api/problems") => Response::ok(&model::list(
                &load(),
                target().as_ref().map(|t| t.repo.as_str()),
            )),
            ("GET", "/api/problems/{id}") => {
                let state = load();
                let Some(kept) = state.get(&id) else {
                    return Response::error(404, "No such problem.");
                };
                let report = write_report(&req, kept);
                cache::set_json(
                    &report_key(&id),
                    &json!({ "title": report.title, "body": report.body,
                             "fingerprint": report.fingerprint }),
                    KEEP_SECS,
                );
                Response::ok(&json!({
                    "problem": model::view(kept),
                    "repository": target().map(|t| t.repo),
                    "title": report.title,
                    "body": report.body,
                }))
            }
            ("POST", "/api/problems/{id}/send") => handle_send(&req, &id),
            ("GET", "/api/problems/{id}/send") => Response::ok(&progress_of(&id)),
            ("POST", "/api/problems/{id}/cancel") => {
                cache::set(&cancel_key(&id), b"1", 600);
                Response::ok(&json!({ "state": "cancelling" }))
            }
            ("POST", "/api/problems/{id}/dismiss") => handle_dismiss(&id),
            _ => Response::error(404, "not found"),
        }
    }

    fn tick(timer: &str) {
        if matches!(timer, "start" | "scan") {
            scan();
        }
    }

    fn run(kind: &str, input: &[u8]) {
        if kind == "send" {
            send_job(input);
        }
    }
}

losos_plugin::export!(ProblemsPlugin);
