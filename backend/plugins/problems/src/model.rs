//! The plugin's thinking, apart from the box: what a journal entry says went
//! wrong, how a scan's findings join the problems already kept, and what the
//! page is shown. Everything here runs in `cargo test` on fixtures.

use losos_report::known;
use losos_report::problem::{crash_signal, Kind, Problem};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// systemd's message for a unit's process that exited other than cleanly
/// (`SD_MESSAGE_UNIT_PROCESS_EXIT`): fields `EXIT_CODE`, `EXIT_STATUS`.
pub const PROCESS_EXIT: &str = "98e322203f7a4ed290d09fe03c09fe15";
/// systemd's message for a unit that ended in a failed result
/// (`SD_MESSAGE_UNIT_FAILURE_RESULT`): field `UNIT_RESULT`.
pub const FAILURE_RESULT: &str = "d9b373ed55a64feb8242e02dbe79a49c";

/// How many problems the file keeps; the ones seen longest ago go first.
pub const KEEP: usize = 200;

/// One thing a scan found, with when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub problem: Problem,
    /// Seconds since the epoch.
    pub at: u64,
}

/// A journal entry's field as text; the journal writes binary fields as
/// arrays of bytes.
fn field(entry: &Value, name: &str) -> Option<String> {
    match &entry[name] {
        Value::String(s) => Some(s.clone()),
        Value::Array(bytes) => {
            let bytes: Option<Vec<u8>> = bytes
                .iter()
                .map(|b| b.as_u64().and_then(|b| u8::try_from(b).ok()))
                .collect();
            bytes.map(|b| String::from_utf8_lossy(&b).into_owned())
        }
        _ => None,
    }
}

/// `__REALTIME_TIMESTAMP`, microseconds, as seconds.
fn when(entry: &Value) -> u64 {
    field(entry, "__REALTIME_TIMESTAMP")
        .and_then(|t| t.parse::<u64>().ok())
        .map_or(0, |us| us / 1_000_000)
}

/// The unit a systemd message is about: a system unit, or a person's own.
fn unit_of(entry: &Value) -> Option<String> {
    field(entry, "UNIT").or_else(|| field(entry, "USER_UNIT"))
}

/// `name[123]: segfault at …` and `name[123] general protection …` → `name`.
fn bracketed_name(message: &str) -> Option<String> {
    let open = message.find('[')?;
    let name = message[..open].trim().rsplit(' ').next()?.trim();
    let pid = message[open + 1..].split(']').next()?;
    (!name.is_empty() && !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit()))
        .then(|| name.to_owned())
}

/// What one journal entry says went wrong, if anything.
#[must_use]
pub fn event_of(entry: &Value) -> Option<Event> {
    let at = when(entry);
    let message = field(entry, "MESSAGE").unwrap_or_default();
    let id = field(entry, "MESSAGE_ID");
    let problem = match id.as_deref() {
        Some(PROCESS_EXIT) => {
            let code = field(entry, "EXIT_CODE").unwrap_or_default();
            let signal = crash_signal(&field(entry, "EXIT_STATUS").unwrap_or_default())?;
            if code != "dumped" && code != "killed" {
                return None;
            }
            Problem {
                kind: Kind::Crashed,
                unit: unit_of(entry),
                process: None,
                signal: Some(signal.to_owned()),
                result: None,
            }
        }
        Some(FAILURE_RESULT) => {
            let result = field(entry, "UNIT_RESULT")
                .or_else(|| {
                    // "Failed with result 'exit-code'."
                    let rest = message.split("result '").nth(1)?;
                    Some(rest.split('\'').next()?.to_owned())
                })
                .unwrap_or_else(|| "failed".to_owned());
            if result == "success" {
                return None;
            }
            Problem {
                kind: Kind::Failed,
                unit: unit_of(entry),
                process: None,
                signal: None,
                result: Some(result),
            }
        }
        _ if field(entry, "_TRANSPORT").as_deref() == Some("kernel") => kernel_event(&message)?,
        _ => return None,
    };
    Some(Event { problem, at })
}

/// The kernel's words for a process it killed or that crashed outside a
/// unit's notice: the OOM killer, a segfault, a general protection fault.
fn kernel_event(message: &str) -> Option<Problem> {
    if let Some(rest) = message.split("Out of memory: Killed process ").nth(1) {
        // "Out of memory: Killed process 1234 (name) total-vm:…"
        let name = rest.split('(').nth(1)?.split(')').next()?.trim();
        return Some(Problem {
            kind: Kind::OutOfMemory,
            unit: None,
            process: Some(name.to_owned()),
            signal: None,
            result: None,
        });
    }
    let signal =
        if message.contains(": segfault at ") || message.contains("general protection fault") {
            "SEGV"
        } else if message.contains("trap invalid opcode") {
            "ILL"
        } else if message.contains("trap divide error") {
            "FPE"
        } else {
            return None;
        };
    // "traps: name[123] general protection fault ip:…" or
    // "name[123]: segfault at 0 ip …"
    let text = message.strip_prefix("traps: ").unwrap_or(message);
    Some(Problem {
        kind: Kind::Crashed,
        unit: None,
        process: Some(bracketed_name(text)?),
        signal: Some(signal.to_owned()),
        result: None,
    })
}

/// A scan's events with the echoes taken out: a unit whose process crashed
/// also fails with result `core-dump` or `signal`, and the kernel's line
/// for the same segfault comes a moment before systemd's.
#[must_use]
pub fn distinct(events: Vec<Event>) -> Vec<Event> {
    let crashed: Vec<(String, u64)> = events
        .iter()
        .filter(|e| e.problem.kind == Kind::Crashed)
        .filter_map(|e| Some((e.problem.unit.clone()?, e.at)))
        .collect();
    events
        .iter()
        .filter(|e| match (&e.problem.kind, &e.problem.unit) {
            (Kind::Failed, Some(unit)) => {
                let echo = matches!(e.problem.result.as_deref(), Some("core-dump" | "signal"));
                !(echo && crashed.iter().any(|(u, _)| u == unit))
            }
            (Kind::Crashed, None) => !crashed.iter().any(|(_, at)| at.abs_diff(e.at) <= 5),
            _ => true,
        })
        .cloned()
        .collect()
}

/// A problem as the file keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Kept {
    /// The fingerprint.
    pub id: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    pub first: u64,
    pub last: u64,
    pub count: u64,
    /// The count when the owner dismissed it: it comes back once it
    /// happens again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dismissed_at_count: Option<u64>,
    /// The known.json entry that explains it, decided when it was first
    /// seen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documented: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sent: Option<Sent>,
}

/// Where a report of it went.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sent {
    pub url: String,
    pub existing: bool,
    pub at: u64,
}

impl Kept {
    #[must_use]
    pub fn problem(&self) -> Problem {
        Problem {
            kind: Kind::parse(&self.kind).unwrap_or(Kind::Failed),
            unit: self.unit.clone(),
            process: self.process.clone(),
            signal: self.signal.clone(),
            result: self.result.clone(),
        }
    }

    #[must_use]
    pub fn dismissed(&self) -> bool {
        self.dismissed_at_count == Some(self.count)
    }
}

/// The file: the problems, and where in the journal the last scan stopped.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default)]
    pub problems: Vec<Kept>,
}

impl State {
    #[must_use]
    pub fn parse(bytes: Option<&[u8]>) -> State {
        bytes
            .and_then(|b| serde_json::from_slice(b).ok())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Kept> {
        self.problems.iter().find(|p| p.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Kept> {
        self.problems.iter_mut().find(|p| p.id == id)
    }

    /// Joins `events` in. Answers the ids that are new, which the caller
    /// checks against the handbook ([`Kept::documented`]).
    pub fn record(&mut self, events: &[Event]) -> Vec<String> {
        let mut new = Vec::new();
        for e in events {
            let id = e.problem.fingerprint();
            match self.get_mut(&id) {
                Some(kept) => {
                    kept.count += 1;
                    kept.first = kept.first.min(e.at);
                    kept.last = kept.last.max(e.at);
                    // The latest instance names it: the page links the unit
                    // that is there now.
                    if e.problem.unit.is_some() {
                        kept.unit.clone_from(&e.problem.unit);
                    }
                }
                None => {
                    self.problems.push(Kept {
                        id: id.clone(),
                        kind: e.problem.kind.as_str().to_owned(),
                        unit: e.problem.unit.clone(),
                        process: e.problem.process.clone(),
                        signal: e.problem.signal.clone(),
                        result: e.problem.result.clone(),
                        first: e.at,
                        last: e.at,
                        count: 1,
                        dismissed_at_count: None,
                        documented: None,
                        sent: None,
                    });
                    new.push(id);
                }
            }
        }
        if self.problems.len() > KEEP {
            self.problems.sort_by_key(|p| std::cmp::Reverse(p.last));
            self.problems.truncate(KEEP);
        }
        new
    }
}

/// One problem as the page reads it.
#[must_use]
pub fn view(kept: &Kept) -> Value {
    let problem = kept.problem();
    let documented = kept.documented.as_deref().and_then(|id| {
        known::all()
            .into_iter()
            .find(|(k, _)| k.id == id)
            .map(|(k, _)| {
                json!({
                    "id": k.id,
                    "page": k.page,
                    "title": { "en": k.title[0], "sk": k.title[1], "de": k.title[2] },
                })
            })
    });
    json!({
        "id": kept.id,
        "kind": kept.kind,
        "unit": kept.unit,
        "process": kept.process,
        "signal": kept.signal,
        "result": kept.result,
        "title": problem.title(),
        "first": kept.first,
        "last": kept.last,
        "count": kept.count,
        "dismissed": kept.dismissed(),
        "documented": documented,
        "sent": kept.sent,
    })
}

/// `GET /api/problems`: the newest first, the dismissed ones last.
#[must_use]
pub fn list(state: &State, repository: Option<&str>) -> Value {
    let mut problems: Vec<&Kept> = state.problems.iter().collect();
    problems.sort_by_key(|p| (p.dismissed(), std::cmp::Reverse(p.last)));
    json!({
        "reporting": repository.map(|r| json!({ "repository": r })),
        "problems": problems.into_iter().map(view).collect::<Vec<_>>(),
    })
}

/// The names in `GET /api/people`'s answer that a report must not carry:
/// sign-in names, display names and e-mail addresses.
#[must_use]
pub fn people_names(people: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for p in people["people"].as_array().into_iter().flatten() {
        for key in ["name", "displayName", "displayname", "email"] {
            if let Some(s) = p[key].as_str() {
                out.push(s.to_owned());
                // A display name's words each name the person too.
                if key != "email" {
                    out.extend(s.split_whitespace().map(str::to_owned));
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(fields: &[(&str, &str)]) -> Value {
        let mut v = json!({});
        for (k, val) in fields {
            v[*k] = json!(val);
        }
        v
    }

    #[test]
    fn systemd_says_a_crash_and_a_failure() {
        let crash = event_of(&entry(&[
            ("MESSAGE_ID", PROCESS_EXIT),
            ("UNIT", "losos-cloud.service"),
            ("EXIT_CODE", "dumped"),
            ("EXIT_STATUS", "11"),
            ("__REALTIME_TIMESTAMP", "1793246400000000"),
        ]))
        .unwrap();
        assert_eq!(crash.problem.kind, Kind::Crashed);
        assert_eq!(crash.problem.signal.as_deref(), Some("SEGV"));
        assert_eq!(crash.at, 1_793_246_400);

        // A stop is not a crash, and an exit code is the failure's to say.
        for (code, status) in [("killed", "15"), ("killed", "9"), ("exited", "1")] {
            assert_eq!(
                event_of(&entry(&[
                    ("MESSAGE_ID", PROCESS_EXIT),
                    ("UNIT", "x.service"),
                    ("EXIT_CODE", code),
                    ("EXIT_STATUS", status),
                ])),
                None
            );
        }

        let failed = event_of(&entry(&[
            ("MESSAGE_ID", FAILURE_RESULT),
            ("USER_UNIT", "app.service"),
            ("MESSAGE", "app.service: Failed with result 'timeout'."),
        ]))
        .unwrap();
        assert_eq!(failed.problem.result.as_deref(), Some("timeout"));
        assert_eq!(failed.problem.unit.as_deref(), Some("app.service"));
    }

    #[test]
    fn the_kernel_says_out_of_memory_and_segfault() {
        let oom = event_of(&entry(&[
            ("_TRANSPORT", "kernel"),
            (
                "MESSAGE",
                "Out of memory: Killed process 4242 (k3s-server) total-vm:1234kB, anon-rss:99kB",
            ),
        ]))
        .unwrap();
        assert_eq!(oom.problem.kind, Kind::OutOfMemory);
        assert_eq!(oom.problem.process.as_deref(), Some("k3s-server"));
        let segv = event_of(&entry(&[
            ("_TRANSPORT", "kernel"),
            (
                "MESSAGE",
                "ollama[812]: segfault at 0 ip 00007f sp 00007f error 4 in x",
            ),
        ]))
        .unwrap();
        assert_eq!(segv.problem.process.as_deref(), Some("ollama"));
        let gp = event_of(&entry(&[
            ("_TRANSPORT", "kernel"),
            (
                "MESSAGE",
                "traps: dendrite[77] general protection fault ip:1 sp:2 error:0",
            ),
        ]))
        .unwrap();
        assert_eq!(gp.problem.process.as_deref(), Some("dendrite"));
        assert_eq!(
            event_of(&entry(&[
                ("_TRANSPORT", "kernel"),
                ("MESSAGE", "usb 1-1: new device")
            ])),
            None
        );
        // Binary fields come as bytes.
        let bytes = json!({"_TRANSPORT": "kernel", "MESSAGE": "a[1]: segfault at 0".bytes().collect::<Vec<u8>>()});
        assert!(event_of(&bytes).is_some());
    }

    #[test]
    fn echoes_of_one_crash_are_one_problem() {
        let unit = |kind, result: Option<&str>, at| Event {
            problem: Problem {
                kind,
                unit: Some("x.service".into()),
                process: None,
                signal: (kind == Kind::Crashed).then(|| "SEGV".into()),
                result: result.map(Into::into),
            },
            at,
        };
        let kernel = Event {
            problem: Problem {
                kind: Kind::Crashed,
                unit: None,
                process: Some("x".into()),
                signal: Some("SEGV".into()),
                result: None,
            },
            at: 99,
        };
        let out = distinct(vec![
            kernel.clone(),
            unit(Kind::Crashed, None, 100),
            unit(Kind::Failed, Some("core-dump"), 100),
        ]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].problem.kind, Kind::Crashed);
        assert!(out[0].problem.unit.is_some());
        let alone = Event { at: 500, ..kernel };
        assert_eq!(distinct(vec![alone.clone()]), vec![alone]);
    }

    #[test]
    fn a_problem_seen_again_counts_and_a_dismissed_one_comes_back() {
        let e = |at| Event {
            problem: Problem {
                kind: Kind::Failed,
                unit: Some(format!("losos-rebuild-{at}.service")),
                process: None,
                signal: None,
                result: Some("exit-code".into()),
            },
            at,
        };
        let mut state = State::default();
        let new = state.record(&[e(10), e(20)]);
        assert_eq!(new.len(), 1);
        let id = new[0].clone();
        let kept = state.get(&id).unwrap();
        assert_eq!((kept.count, kept.first, kept.last), (2, 10, 20));
        assert_eq!(kept.unit.as_deref(), Some("losos-rebuild-20.service"));
        state.get_mut(&id).unwrap().dismissed_at_count = Some(2);
        assert!(state.get(&id).unwrap().dismissed());
        assert!(state.record(&[e(30)]).is_empty());
        assert!(!state.get(&id).unwrap().dismissed());

        let back = State::parse(Some(serde_json::to_vec(&state).unwrap().as_slice()));
        assert_eq!(back, state);
        let listed = list(&state, Some("dasmatus/losos"));
        assert_eq!(listed["reporting"]["repository"], "dasmatus/losos");
        assert_eq!(listed["problems"][0]["count"], 3);
        assert!(listed["problems"][0]["title"]
            .as_str()
            .unwrap()
            .contains("failed (exit-code)"));
    }

    #[test]
    fn a_documented_problem_names_its_page() {
        let kept = Kept {
            id: "x".into(),
            kind: "failed".into(),
            unit: None,
            process: None,
            signal: None,
            result: None,
            first: 0,
            last: 0,
            count: 1,
            dismissed_at_count: None,
            documented: Some("out-of-room".into()),
            sent: None,
        };
        assert_eq!(
            view(&kept)["documented"]["page"],
            "troubleshooting/out-of-room"
        );
        assert_eq!(
            view(&kept)["documented"]["title"]["sk"],
            "Nedostatok miesta"
        );
    }

    #[test]
    fn people_are_names_to_take_out() {
        let names = people_names(&json!({"people": [
            {"name": "eva", "displayName": "Eva Nová", "email": "eva@example.org"}
        ]}));
        assert!(names.contains(&"eva".to_owned()));
        assert!(names.contains(&"Nová".to_owned()));
        assert!(names.contains(&"eva@example.org".to_owned()));
    }
}
