//! LosOS Mail's server settings, written into Stalwart over JMAP.
//!
//! Stalwart 1.0 keeps one file on disk, `config.json`, which names its data
//! store; everything else (listeners, caches, the domain, logging) is an
//! object in its own registry, read and written at `/jmap` with the
//! `urn:stalwart:jmap` capability. `modules/mail.nix` writes what a LosOS
//! box needs as a plan, a JSON file in the Nix store, and `losos-mail
//! setup` brings the server in line with it before the first normal start
//! and after every change: it starts Stalwart in recovery mode (a single
//! HTTP listener on loopback, nothing else running), applies the plan and
//! stops it again.
//!
//! **Why before the first start.** A Stalwart that starts normally with no
//! listener objects makes its defaults: SMTP on 25, IMAP on 993 and five
//! more, on every address. It also makes a log file under /var/log, an
//! ASN source on GitHub and a web interface it downloads from GitHub. The
//! plan makes each of these first, so none of the defaults is ever made.
//!
//! **The plan.** A list of entries, each for one object type:
//!
//! - `singleton`: the one object of the type; `value` holds the properties
//!   to set.
//! - `items` with `key`: objects found by the value of their `key`
//!   property, made when missing and changed where they differ. With
//!   `only`, every other object of the type is destroyed.
//! - `all`: properties every existing object of the type gets.
//!
//! A value `{"$ref": ["x:Domain", "name", "box.home.arpa"]}` stands for the
//! id of that object, found or made by an earlier entry. Applying a plan
//! twice changes nothing the second time.
//!
//! Everything that turns a plan and the server's answers into JMAP is a
//! pure function, tested below; [`apply`] is the loop that sends it.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::mail::{Refused, Request, Transport};

/// The capabilities a registry call uses.
pub const USING: [&str; 2] = ["urn:ietf:params:jmap:core", "urn:stalwart:jmap"];

/// One entry of a plan.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// The object type, `x:NetworkListener` and so on.
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub singleton: bool,
    /// The singleton's properties.
    #[serde(default)]
    pub value: Option<Value>,
    /// The property that names an item.
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub items: Vec<Value>,
    /// Destroy every object of the type the items do not name.
    #[serde(default)]
    pub only: bool,
    /// Properties every existing object gets.
    #[serde(default)]
    pub all: Option<Value>,
}

/// A plan as `modules/mail.nix` writes it.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub entries: Vec<Entry>,
}

impl Plan {
    pub fn parse(text: &str) -> Result<Plan, String> {
        let plan: Plan = serde_json::from_str(text).map_err(|e| format!("the plan: {e}"))?;
        for e in &plan.entries {
            let shapes = [e.singleton, !e.items.is_empty(), e.all.is_some()];
            if shapes.iter().filter(|s| **s).count() != 1 {
                return Err(format!(
                    "{}: an entry is a singleton, a list of items or `all`",
                    e.kind
                ));
            }
            if e.singleton && e.value.is_none() {
                return Err(format!("{}: a singleton needs a value", e.kind));
            }
            if !e.items.is_empty() {
                let key = e
                    .key
                    .as_deref()
                    .ok_or_else(|| format!("{}: items need a key", e.kind))?;
                if e.items.iter().any(|i| i.get(key).is_none()) {
                    return Err(format!("{}: every item needs its {key}", e.kind));
                }
            }
        }
        Ok(plan)
    }
}

/// The id Stalwart gives a singleton.
pub const SINGLETON: &str = "singleton";

/// Ids found or made so far, by type, key property and key value.
pub type Found = HashMap<(String, String, String), String>;

/// `value` with every `{"$ref": [type, key, value]}` replaced by its id.
pub fn resolve(value: &Value, found: &Found) -> Result<Value, String> {
    match value {
        Value::Object(map) => {
            if let Some(r) = map.get("$ref") {
                let parts: Vec<&str> = r
                    .as_array()
                    .map(|a| a.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                if parts.len() != 3 || map.len() != 1 {
                    return Err(format!("not a reference: {r}"));
                }
                let id = found
                    .get(&(parts[0].into(), parts[1].into(), parts[2].into()))
                    .ok_or_else(|| format!("{} {} is not made yet", parts[0], parts[2]))?;
                return Ok(Value::String(id.clone()));
            }
            let mut out = Map::new();
            for (k, v) in map {
                out.insert(k.clone(), resolve(v, found)?);
            }
            Ok(Value::Object(out))
        }
        Value::Array(a) => Ok(Value::Array(
            a.iter()
                .map(|v| resolve(v, found))
                .collect::<Result<_, _>>()?,
        )),
        other => Ok(other.clone()),
    }
}

/// The properties of `wanted` that `current` does not already have, as a
/// patch. `None` when there is nothing to change.
pub fn differs(current: &Value, wanted: &Value) -> Option<Map<String, Value>> {
    let wanted = wanted.as_object()?;
    let patch: Map<String, Value> = wanted
        .iter()
        .filter(|(k, v)| current.get(k.as_str()) != Some(v))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    (!patch.is_empty()).then_some(patch)
}

/// What one entry asks of the server, given the objects it has now: the
/// arguments of a `<type>/set`, or `None` when nothing differs.
pub fn set_args(entry: &Entry, existing: &[Value], found: &Found) -> Result<Option<Value>, String> {
    let mut create = Map::new();
    let mut update = Map::new();
    let mut destroy = Vec::new();
    if entry.singleton {
        let wanted = resolve(entry.value.as_ref().unwrap_or(&Value::Null), found)?;
        match existing.first() {
            Some(cur) => {
                if let Some(patch) = differs(cur, &wanted) {
                    update.insert(SINGLETON.into(), Value::Object(patch));
                }
            }
            None => {
                create.insert("s".into(), wanted);
            }
        }
    } else if let Some(all) = &entry.all {
        let wanted = resolve(all, found)?;
        for cur in existing {
            let id = id_of(cur)?;
            if let Some(patch) = differs(cur, &wanted) {
                update.insert(id, Value::Object(patch));
            }
        }
    } else {
        let key = entry.key.as_deref().unwrap_or_default();
        let mut named = Vec::new();
        for (n, item) in entry.items.iter().enumerate() {
            let wanted = resolve(item, found)?;
            let name = &wanted[key];
            named.push(name.clone());
            match existing.iter().find(|cur| cur.get(key) == Some(name)) {
                Some(cur) => {
                    if let Some(patch) = differs(cur, &wanted) {
                        update.insert(id_of(cur)?, Value::Object(patch));
                    }
                }
                None => {
                    create.insert(format!("i{n}"), wanted);
                }
            }
        }
        if entry.only {
            for cur in existing {
                let name = cur.get(key).cloned().unwrap_or(Value::Null);
                if !named.contains(&name) {
                    destroy.push(Value::String(id_of(cur)?));
                }
            }
        }
    }
    if create.is_empty() && update.is_empty() && destroy.is_empty() {
        return Ok(None);
    }
    let mut args = Map::new();
    if !create.is_empty() {
        args.insert("create".into(), Value::Object(create));
    }
    if !update.is_empty() {
        args.insert("update".into(), Value::Object(update));
    }
    if !destroy.is_empty() {
        args.insert("destroy".into(), Value::Array(destroy));
    }
    Ok(Some(Value::Object(args)))
}

fn id_of(v: &Value) -> Result<String, String> {
    v.get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "an object without an id".to_string())
}

/// The `get` call that reads every object of an entry's type.
pub fn get_call(entry: &Entry) -> Value {
    let ids = if entry.singleton {
        json!([SINGLETON])
    } else {
        Value::Null
    };
    json!([format!("{}/get", entry.kind), { "ids": ids }, "g"])
}

/// A JMAP request of `calls` with the registry's capabilities.
pub fn envelope(calls: Vec<Value>) -> Value {
    json!({ "using": USING, "methodCalls": calls })
}

/// The `list` of the one answer in `resp` named `name`, or the error the
/// server gave instead.
pub fn answer<'a>(resp: &'a Value, name: &str) -> Result<&'a Value, String> {
    let calls = resp
        .get("methodResponses")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("no answer to {name}: {resp}"))?;
    let call = calls
        .first()
        .and_then(Value::as_array)
        .ok_or_else(|| format!("no answer to {name}"))?;
    if call.first().and_then(Value::as_str) == Some(name) {
        Ok(call.get(1).unwrap_or(&Value::Null))
    } else {
        Err(format!("{name}: {}", call.get(1).unwrap_or(&Value::Null)))
    }
}

/// What a `set` answer refused, in words; `None` when it took everything.
pub fn refused(args: &Value) -> Option<String> {
    let mut out = Vec::new();
    for field in ["notCreated", "notUpdated", "notDestroyed"] {
        if let Some(map) = args.get(field).and_then(Value::as_object) {
            for (id, why) in map {
                out.push(format!("{field} {id}: {why}"));
            }
        }
    }
    (!out.is_empty()).then(|| out.join("; "))
}

/// Remember the ids of an entry's items, from the objects the server had
/// and the ones the `set` made.
pub fn remember(entry: &Entry, existing: &[Value], set: Option<&Value>, found: &mut Found) {
    let Some(key) = entry.key.as_deref() else {
        return;
    };
    for cur in existing {
        if let (Some(id), Some(name)) = (cur.get("id").and_then(Value::as_str), cur.get(key)) {
            if let Some(name) = name.as_str() {
                found.insert((entry.kind.clone(), key.into(), name.into()), id.into());
            }
        }
    }
    let Some(created) = set
        .and_then(|s| s.get("created"))
        .and_then(Value::as_object)
    else {
        return;
    };
    for (n, item) in entry.items.iter().enumerate() {
        let made = created.get(&format!("i{n}"));
        if let (Some(id), Some(name)) = (
            made.and_then(|m| m.get("id")).and_then(Value::as_str),
            item.get(key).and_then(Value::as_str),
        ) {
            found.insert((entry.kind.clone(), key.into(), name.into()), id.into());
        }
    }
}

/// Bring the server in line with `plan`, as `user` with `secret`. Returns
/// how many entries changed something.
pub fn apply(
    transport: &dyn Transport,
    user: &str,
    secret: &str,
    plan: &Plan,
) -> Result<usize, String> {
    let call = |calls: Vec<Value>| -> Result<Value, String> {
        let resp = transport
            .send(&Request {
                method: "POST",
                path: "/jmap/".into(),
                user: user.into(),
                secret: secret.into(),
                body: Some(serde_json::to_vec(&envelope(calls)).unwrap_or_default()),
                content_type: None,
            })
            .map_err(|e: Refused| e.message)?;
        if resp.status != 200 {
            return Err(format!(
                "the mail server answered {}: {}",
                resp.status,
                String::from_utf8_lossy(&resp.body)
            ));
        }
        serde_json::from_slice(&resp.body).map_err(|e| format!("not JSON: {e}"))
    };
    let mut found = Found::new();
    let mut changed = 0;
    for entry in &plan.entries {
        let got = call(vec![get_call(entry)])?;
        let got = answer(&got, &format!("{}/get", entry.kind))?;
        let existing: Vec<Value> = got
            .get("list")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let args = set_args(entry, &existing, &found)?;
        let mut set = None;
        if let Some(args) = args {
            let resp = call(vec![json!([format!("{}/set", entry.kind), args, "s"])])?;
            let done = answer(&resp, &format!("{}/set", entry.kind))?.clone();
            if let Some(why) = refused(&done) {
                return Err(format!("{}: {why}", entry.kind));
            }
            set = Some(done);
            changed += 1;
        }
        remember(entry, &existing, set.as_ref(), &mut found);
    }
    Ok(changed)
}

// ── Accounts ───────────────────────────────────────────────────────────────

/// The login that acts as `address` with the recovery administrator's
/// secret: Stalwart 1.0's impersonation, `<account>%<admin>`. The account
/// is a whole address.
pub fn as_person(address: &str) -> String {
    format!("{address}%{}", crate::mail::MASTER_USER)
}

/// The call that lists the domains, to find one's id.
pub fn domains_call() -> Value {
    json!(["x:Domain/get", { "ids": null, "properties": ["id", "name"] }, "d"])
}

/// The id of `domain` in a `domains_call` answer.
pub fn domain_id(resp: &Value, domain: &str) -> Result<String, String> {
    answer(resp, "x:Domain/get")?
        .get("list")
        .and_then(Value::as_array)
        .and_then(|l| {
            l.iter()
                .find(|d| d.get("name").and_then(Value::as_str) == Some(domain))
        })
        .and_then(|d| d.get("id").and_then(Value::as_str))
        .map(str::to_string)
        .ok_or_else(|| format!("the mail server has no domain {domain}"))
}

/// The call that makes `name`'s account under the domain `domain_id`.
/// Nobody signs in to it with a password: lososd acts as them.
pub fn account_call(domain_id: &str, name: &str, description: &str) -> Value {
    json!(["x:Account/set", { "create": { "a": {
        "@type": "User",
        "name": name,
        "domainId": domain_id,
        "description": description,
    } } }, "a"])
}

/// Whether an `account_call` answer made the account or found it made.
pub fn account_made(resp: &Value) -> Result<(), String> {
    let args = answer(resp, "x:Account/set")?;
    if args.pointer("/created/a").is_some() {
        return Ok(());
    }
    let why = args
        .pointer("/notCreated/a")
        .cloned()
        .unwrap_or(Value::Null);
    let kind = why.get("type").and_then(Value::as_str).unwrap_or_default();
    // Making it twice is harmless: the name is the account's key.
    if kind == "alreadyExists" || kind == "primaryKeyViolation" {
        return Ok(());
    }
    Err(format!("the mail server would not make the account: {why}"))
}

// ── The copy from Stalwart 0.15 ────────────────────────────────────────────

/// The people of a 0.15 server, from its `GET /api/principal`: name and
/// display name of every individual.
pub fn legacy_people(resp: &Value) -> Vec<(String, String)> {
    resp.pointer("/data/items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|p| p.get("type").and_then(Value::as_str) == Some("individual"))
                .filter_map(|p| {
                    let name = p.get("name").and_then(Value::as_str)?;
                    // The names lososd made; nothing else was ever there.
                    crate::people::validate_name(name).is_ok().then(|| {
                        let shown = p.get("description").and_then(Value::as_str).unwrap_or(name);
                        (name.to_string(), shown.to_string())
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// What Vandelay copies: the mail, its folders, the identities a person
/// sends as and their filters. Calendars, contacts and files live in LosOS
/// Cloud, never here.
pub const COPIED: &str = "mailbox,email,identity,sievescript";

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(v: Value) -> Plan {
        Plan::parse(&v.to_string()).unwrap()
    }

    fn listeners() -> Entry {
        plan(json!({ "entries": [{
            "type": "x:NetworkListener", "key": "name", "only": true,
            "items": [{ "name": "http", "bind": { "127.0.0.1:8096": true }, "protocol": "http" }]
        }]}))
        .entries
        .remove(0)
    }

    #[test]
    fn an_entry_has_exactly_one_shape() {
        for bad in [
            json!({ "entries": [{ "type": "x:Cache" }] }),
            json!({ "entries": [{ "type": "x:Cache", "singleton": true }] }),
            json!({ "entries": [{ "type": "x:Domain", "items": [{ "name": "a" }] }] }),
            json!({ "entries": [{ "type": "x:Domain", "key": "name", "items": [{}] }] }),
            json!({ "entries": [{ "type": "x:Cache", "singleton": true, "value": {}, "all": {} }] }),
            json!({ "entries": [{ "type": "x:Cache", "singletons": true }] }),
        ] {
            assert!(Plan::parse(&bad.to_string()).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_first_start_makes_the_listener_and_nothing_else() {
        let args = set_args(&listeners(), &[], &Found::new()).unwrap().unwrap();
        assert_eq!(
            args,
            json!({ "create": { "i0": { "name": "http", "bind": { "127.0.0.1:8096": true }, "protocol": "http" } } })
        );
    }

    #[test]
    fn defaults_made_by_an_older_start_are_destroyed() {
        let existing = [
            json!({ "id": "a", "name": "smtp", "bind": { "[::]:25": true }, "protocol": "smtp" }),
            json!({ "id": "b", "name": "http", "bind": { "[::]:8080": true }, "protocol": "http" }),
        ];
        let args = set_args(&listeners(), &existing, &Found::new())
            .unwrap()
            .unwrap();
        assert_eq!(
            args,
            json!({
                "update": { "b": { "bind": { "127.0.0.1:8096": true } } },
                "destroy": ["a"]
            })
        );
    }

    #[test]
    fn a_second_run_changes_nothing() {
        let existing = [
            json!({ "id": "b", "name": "http", "bind": { "127.0.0.1:8096": true }, "protocol": "http", "useTls": true }),
        ];
        assert_eq!(
            set_args(&listeners(), &existing, &Found::new()).unwrap(),
            None
        );
    }

    #[test]
    fn a_singleton_is_patched_where_it_differs() {
        let e = plan(json!({ "entries": [{ "type": "x:Cache", "singleton": true,
            "value": { "messagesLarge": 0, "messages": 33554432 } }] }))
        .entries
        .remove(0);
        let cur = [json!({ "id": "singleton", "messagesLarge": 536870912, "messages": 33554432 })];
        assert_eq!(
            set_args(&e, &cur, &Found::new()).unwrap().unwrap(),
            json!({ "update": { "singleton": { "messagesLarge": 0 } } })
        );
        assert_eq!(
            get_call(&e),
            json!(["x:Cache/get", { "ids": ["singleton"] }, "g"])
        );
    }

    #[test]
    fn all_reaches_every_object() {
        let e =
            plan(json!({ "entries": [{ "type": "x:Application", "all": { "enable": false } }] }))
                .entries
                .remove(0);
        let cur = [
            json!({ "id": "w", "enable": true }),
            json!({ "id": "x", "enable": false }),
        ];
        assert_eq!(
            set_args(&e, &cur, &Found::new()).unwrap().unwrap(),
            json!({ "update": { "w": { "enable": false } } })
        );
    }

    #[test]
    fn a_reference_is_the_id_an_earlier_entry_found_or_made() {
        let p = plan(json!({ "entries": [
            { "type": "x:Domain", "key": "name", "items": [{ "name": "box.home.arpa" }] },
            { "type": "x:SystemSettings", "singleton": true,
              "value": { "defaultDomainId": { "$ref": ["x:Domain", "name", "box.home.arpa"] } } }
        ]}));
        let mut found = Found::new();
        assert!(set_args(&p.entries[1], &[], &found).is_err());
        let made = json!({ "created": { "i0": { "id": "d1" } } });
        remember(&p.entries[0], &[], Some(&made), &mut found);
        assert_eq!(
            set_args(&p.entries[1], &[json!({ "id": "singleton" })], &found)
                .unwrap()
                .unwrap(),
            json!({ "update": { "singleton": { "defaultDomainId": "d1" } } })
        );
        let mut found = Found::new();
        remember(
            &p.entries[0],
            &[json!({ "id": "d0", "name": "box.home.arpa" })],
            None,
            &mut found,
        );
        assert_eq!(
            found.get(&("x:Domain".into(), "name".into(), "box.home.arpa".into())),
            Some(&"d0".to_string())
        );
    }

    #[test]
    fn a_refusal_is_named() {
        assert_eq!(refused(&json!({ "created": {} })), None);
        assert_eq!(
            refused(&json!({ "notCreated": { "i0": { "type": "invalidProperties" } } })).unwrap(),
            r#"notCreated i0: {"type":"invalidProperties"}"#
        );
        assert!(answer(
            &json!({ "methodResponses": [["error", { "type": "forbidden" }, "g"]] }),
            "x:Cache/get"
        )
        .is_err());
    }
}
