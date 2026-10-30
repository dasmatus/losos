//! Sending a report: GitHub's device flow, then the issue or a comment.
//!
//! The requests go through a [`Transport`], so the same steps run in the
//! box's plugin (the host's `http` import) and in the installer (curl), and
//! in the tests against a script of answers.

use crate::{Report, Target};
use serde_json::{json, Value};

/// One request to GitHub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Call {
    /// A form posted to the sign-in endpoints, which answer JSON when asked.
    Form {
        url: String,
        fields: Vec<(String, String)>,
    },
    /// A `GET` of the API with these query fields, with the token.
    Get {
        url: String,
        query: Vec<(String, String)>,
        token: String,
    },
    /// A JSON body posted to the API, with the token.
    Post {
        url: String,
        body: Value,
        token: String,
    },
}

/// How [`send`] reaches GitHub and waits between polls.
pub trait Transport {
    /// The status and the JSON that came back (`Value::Null` when it was
    /// not JSON), or why GitHub could not be reached.
    fn call(&mut self, call: &Call) -> Result<(u16, Value), String>;

    /// Wait `secs` seconds before the next poll. `false` when the person
    /// cancelled meanwhile.
    fn wait(&mut self, secs: u64) -> bool;
}

/// What sending tells the page or the screen as it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// Open `uri` and enter `code` there, within `expires` seconds.
    SignIn {
        uri: String,
        code: String,
        expires: u64,
    },
    /// Filed at `url`; `existing` when it went onto an issue already open.
    Sent { url: String, existing: bool },
}

/// `application/x-www-form-urlencoded` and query-string escaping.
#[must_use]
pub fn urlencode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// `fields` as `a=1&b=2`.
#[must_use]
pub fn form(fields: &[(String, String)]) -> String {
    fields
        .iter()
        .map(|(k, v)| format!("{}={}", urlencode(k), urlencode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// GitHub's own words for a refused request, or its status.
fn refusal(status: u16, answer: &Value) -> String {
    match answer["message"].as_str() {
        Some(message) => format!("GitHub said: {message} ({status})"),
        None => format!("GitHub answered {status}"),
    }
}

const EXPIRED: &str =
    "The code expired before it was entered. Send the report again for a new one.";

/// Signs in with the device flow, then files `report` under `target` with
/// `body` (its own, or [`crate::with_note`]'s), telling `progress` as it
/// goes. An error is a sentence for the person.
pub fn send(
    target: &Target,
    report: &Report,
    body: &str,
    transport: &mut dyn Transport,
    progress: &mut dyn FnMut(Progress),
) -> Result<(), String> {
    let (status, code) = transport.call(&Call::Form {
        url: format!("{}/login/device/code", target.login),
        fields: vec![("client_id".into(), target.client_id.clone())],
    })?;
    let (Some(device), Some(user_code), Some(uri)) = (
        code["device_code"].as_str(),
        code["user_code"].as_str(),
        code["verification_uri"].as_str(),
    ) else {
        return Err(refusal(status, &code));
    };
    let expires = code["expires_in"].as_u64().unwrap_or(900);
    progress(Progress::SignIn {
        uri: uri.to_owned(),
        code: user_code.to_owned(),
        expires,
    });
    let mut interval = code["interval"].as_u64().unwrap_or(5).max(1);
    let mut waited = 0;
    let token = loop {
        if !transport.wait(interval) {
            return Err("Cancelled.".to_owned());
        }
        waited += interval;
        if waited > expires {
            return Err(EXPIRED.to_owned());
        }
        let (status, answer) = transport.call(&Call::Form {
            url: format!("{}/login/oauth/access_token", target.login),
            fields: vec![
                ("client_id".into(), target.client_id.clone()),
                ("device_code".into(), device.to_owned()),
                (
                    "grant_type".into(),
                    "urn:ietf:params:oauth:grant-type:device_code".into(),
                ),
            ],
        })?;
        if let Some(token) = answer["access_token"].as_str() {
            break token.to_owned();
        }
        match answer["error"].as_str() {
            Some("authorization_pending") => {}
            // GitHub asks for five seconds more each time.
            Some("slow_down") => interval += 5,
            Some("access_denied") => return Err("The sign-in was declined on GitHub.".to_owned()),
            Some("expired_token") => return Err(EXPIRED.to_owned()),
            _ => {
                return Err(match answer["error_description"].as_str() {
                    Some(why) => format!("GitHub said: {why}"),
                    None => refusal(status, &answer),
                });
            }
        }
    };

    let api = &target.api;
    let repo = &target.repo;
    // An open issue with the same fingerprint is the same problem. A search
    // that fails only means a new issue, never a lost report.
    let query = format!(
        "repo:{repo} is:issue is:open in:title \"[{}]\"",
        report.fingerprint
    );
    let found = transport
        .call(&Call::Get {
            url: format!("{api}/search/issues"),
            query: vec![("q".into(), query)],
            token: token.clone(),
        })
        .map(|(_, answer)| answer)
        .unwrap_or(Value::Null);
    let suffix = format!("[{}]", report.fingerprint);
    let existing = found["items"]
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .find(|i| i["title"].as_str().is_some_and(|t| t.ends_with(&suffix)))
        })
        .and_then(|i| i["number"].as_u64());
    let (call, existing) = match existing {
        Some(number) => (
            Call::Post {
                url: format!("{api}/repos/{repo}/issues/{number}/comments"),
                body: json!({ "body": body }),
                token,
            },
            true,
        ),
        None => (
            Call::Post {
                url: format!("{api}/repos/{repo}/issues"),
                body: json!({ "title": report.title, "body": body }),
                token,
            },
            false,
        ),
    };
    let (status, answer) = transport.call(&call)?;
    match answer["html_url"].as_str() {
        Some(url) if (200..300).contains(&status) => {
            progress(Progress::Sent {
                url: url.to_owned(),
                existing,
            });
            Ok(())
        }
        _ => Err(refusal(status, &answer)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// Answers in order, and remembers what was asked.
    struct Script {
        answers: VecDeque<Result<(u16, Value), String>>,
        calls: Vec<Call>,
        waits: Vec<u64>,
        cancel_after: Option<usize>,
    }

    impl Script {
        fn new(answers: Vec<(u16, Value)>) -> Script {
            Script {
                answers: answers.into_iter().map(Ok).collect(),
                calls: Vec::new(),
                waits: Vec::new(),
                cancel_after: None,
            }
        }
    }

    impl Transport for Script {
        fn call(&mut self, call: &Call) -> Result<(u16, Value), String> {
            self.calls.push(call.clone());
            self.answers.pop_front().expect("an answer for every call")
        }
        fn wait(&mut self, secs: u64) -> bool {
            self.waits.push(secs);
            self.cancel_after.is_none_or(|n| self.waits.len() <= n)
        }
    }

    fn target() -> Target {
        Target::from_settings(Some("dasmatus/losos"), Some("Iv23test"), None, None).unwrap()
    }

    fn report() -> Report {
        Report {
            title: "lososd crashed (SIGSEGV) [0badc0de]".into(),
            body: "body".into(),
            fingerprint: "0badc0de".into(),
        }
    }

    fn code() -> (u16, Value) {
        (
            200,
            json!({"device_code": "dev", "user_code": "ABCD-1234",
                   "verification_uri": "https://github.com/login/device",
                   "expires_in": 900, "interval": 5}),
        )
    }

    #[test]
    fn a_new_problem_opens_an_issue_after_the_sign_in() {
        let mut script = Script::new(vec![
            code(),
            (200, json!({"error": "authorization_pending"})),
            (200, json!({"error": "slow_down"})),
            (200, json!({"access_token": "ghu_x"})),
            (200, json!({"items": []})),
            (
                201,
                json!({"html_url": "https://github.com/dasmatus/losos/issues/7"}),
            ),
        ]);
        let mut seen = Vec::new();
        send(&target(), &report(), "body", &mut script, &mut |p| {
            seen.push(p)
        })
        .unwrap();
        assert_eq!(
            seen,
            vec![
                Progress::SignIn {
                    uri: "https://github.com/login/device".into(),
                    code: "ABCD-1234".into(),
                    expires: 900
                },
                Progress::Sent {
                    url: "https://github.com/dasmatus/losos/issues/7".into(),
                    existing: false
                },
            ]
        );
        assert_eq!(script.waits, vec![5, 5, 10]);
        match &script.calls[4] {
            Call::Get { query, token, .. } => {
                assert_eq!(token, "ghu_x");
                assert!(query[0].1.contains("in:title \"[0badc0de]\""));
            }
            other => panic!("{other:?}"),
        }
        match &script.calls[5] {
            Call::Post { url, body, .. } => {
                assert_eq!(url, "https://api.github.com/repos/dasmatus/losos/issues");
                assert_eq!(body["title"], "lososd crashed (SIGSEGV) [0badc0de]");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_same_problem_again_is_a_comment() {
        let mut script = Script::new(vec![
            code(),
            (200, json!({"access_token": "ghu_x"})),
            (
                200,
                json!({"items": [{"number": 3, "title": "other [0badc0de0]"},
                                 {"number": 9, "title": "lososd crashed [0badc0de]"}]}),
            ),
            (
                201,
                json!({"html_url": "https://github.com/dasmatus/losos/issues/9#c"}),
            ),
        ]);
        let mut seen = Vec::new();
        send(&target(), &report(), "b", &mut script, &mut |p| {
            seen.push(p)
        })
        .unwrap();
        assert!(matches!(&seen[1], Progress::Sent { existing: true, .. }));
        match &script.calls[3] {
            Call::Post { url, .. } => assert!(url.ends_with("/issues/9/comments")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_decline_a_cancel_and_a_refusal_are_said_plainly() {
        let mut script = Script::new(vec![code(), (200, json!({"error": "access_denied"}))]);
        let err = send(&target(), &report(), "b", &mut script, &mut |_| {}).unwrap_err();
        assert_eq!(err, "The sign-in was declined on GitHub.");

        let mut script = Script::new(vec![code()]);
        script.cancel_after = Some(0);
        let err = send(&target(), &report(), "b", &mut script, &mut |_| {}).unwrap_err();
        assert_eq!(err, "Cancelled.");

        let mut script = Script::new(vec![(400, json!({"message": "Bad client"}))]);
        let err = send(&target(), &report(), "b", &mut script, &mut |_| {}).unwrap_err();
        assert_eq!(err, "GitHub said: Bad client (400)");
    }

    #[test]
    fn forms_are_escaped() {
        assert_eq!(
            form(&[("q".into(), "repo:a/b \"[x]\"".into())]),
            "q=repo%3Aa%2Fb%20%22%5Bx%5D%22"
        );
    }
}
