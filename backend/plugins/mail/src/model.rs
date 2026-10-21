//! A person's own folders and their filters, written against [`Server`], so
//! the tests run them against a mail server in memory.
//!
//! LosOS Mail's own folders (the inbox, drafts, sent, archive, junk and the
//! bin, the ones with a JMAP role) stay as they are: a person adds folders
//! of their own beside and inside them, renames and moves those, and
//! deletes them. Deleting a folder moves its mail to the bin first, so
//! nothing is lost by a slip, and a folder with folders inside it is
//! refused rather than taking them along.

use crate::sieve::{self, Filter, Invalid};
use serde::Serialize;
use serde_json::{json, Value};

/// The longest folder name, in characters.
pub const MAX_NAME: usize = 100;
/// Messages moved per request while a folder is emptied into the bin.
const BATCH: usize = 256;
/// Rounds of [`BATCH`] before a delete gives up: 256 000 messages.
const MAX_ROUNDS: usize = 1000;

/// A request the page cannot have, in words it shows as they are, and what
/// the journal gets when the words are not the whole story.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub status: u16,
    pub message: String,
    /// A word the page says the refusal by in its own language.
    pub code: Option<&'static str>,
    pub detail: Option<String>,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

pub fn refuse<T>(status: u16, message: impl Into<String>) -> Result<T, Refused> {
    Err(Refused {
        status,
        message: message.into(),
        code: None,
        detail: None,
    })
}

/// [`refuse`] with the word the page knows it by.
pub fn refuse_as<T>(
    status: u16,
    code: &'static str,
    message: impl Into<String>,
) -> Result<T, Refused> {
    Err(Refused {
        status,
        message: message.into(),
        code: Some(code),
        detail: None,
    })
}

impl From<Invalid> for Refused {
    fn from(e: Invalid) -> Self {
        Refused {
            status: 400,
            message: e.0,
            code: None,
            detail: None,
        }
    }
}

/// The server answered something this plugin did not expect.
pub fn odd(what: &str) -> Refused {
    Refused {
        status: 502,
        message: "the mail server gave an answer LosOS Mail does not understand".into(),
        code: None,
        detail: Some(format!(
            "the mail server answered something unexpected: {what}"
        )),
    }
}

/// The mail server, as one person.
pub trait Server {
    /// The person's JMAP account id.
    fn account(&self) -> &str;
    /// One JMAP request: the method calls in, the whole answer out.
    fn jmap(&mut self, calls: Vec<Value>) -> Result<Value, Refused>;
    /// Store `bytes` as a blob of `kind`; its id.
    fn upload(&mut self, bytes: &[u8], kind: &str) -> Result<String, Refused>;
    /// A blob's bytes.
    fn download(&mut self, blob: &str) -> Result<Vec<u8>, Refused>;
}

/// One folder, as the folder routes see it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub role: Option<String>,
}

/// The answer to call `tag`, or the reason it failed.
fn answer<'a>(resp: &'a Value, name: &str, tag: &str) -> Result<&'a Value, Refused> {
    let calls = resp
        .get("methodResponses")
        .and_then(Value::as_array)
        .ok_or_else(|| odd("no methodResponses"))?;
    for call in calls {
        if call.get(2).and_then(Value::as_str) != Some(tag) {
            continue;
        }
        let args = call.get(1).ok_or_else(|| odd(name))?;
        return match call.get(0).and_then(Value::as_str) {
            Some(n) if n == name => Ok(args),
            Some("error") => {
                let kind = args.get("type").and_then(Value::as_str).unwrap_or("");
                Err(odd(&format!("{name}: {kind}")))
            }
            _ => Err(odd(name)),
        };
    }
    Err(odd(name))
}

/// The first reason a `/set` call gave for not doing what was asked: its
/// JMAP type and description.
fn set_failure(args: &Value) -> Option<(String, String)> {
    ["notCreated", "notUpdated", "notDestroyed"]
        .iter()
        .filter_map(|k| args.get(*k).and_then(Value::as_object))
        .flat_map(|m| m.values())
        .next()
        .map(|e| {
            let s = |k: &str| e.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            (s("type"), s("description"))
        })
}

pub fn folders(server: &mut dyn Server) -> Result<Vec<Folder>, Refused> {
    let account = server.account().to_string();
    let resp = server.jmap(vec![json!(["Mailbox/get", {
        "accountId": account,
        "properties": ["name", "parentId", "role"],
    }, "m"])])?;
    let list = answer(&resp, "Mailbox/get", "m")?
        .get("list")
        .and_then(Value::as_array)
        .ok_or_else(|| odd("Mailbox/get list"))?;
    Ok(list
        .iter()
        .filter_map(|m| {
            let s = |k: &str| m.get(k).and_then(Value::as_str).map(str::to_string);
            Some(Folder {
                id: s("id")?,
                name: s("name")?,
                parent_id: s("parentId"),
                role: s("role"),
            })
        })
        .collect())
}

/// A folder name a person typed.
pub fn check_name(name: &str) -> Result<String, Refused> {
    let name = name.trim();
    if name.is_empty() {
        return refuse_as(400, "badName", "a folder needs a name");
    }
    if name.chars().count() > MAX_NAME {
        return refuse_as(
            400,
            "badName",
            format!("a folder's name is at most {MAX_NAME} characters"),
        );
    }
    // A slash is the folder separator of every IMAP client.
    if name.contains('/') || name.chars().any(char::is_control) {
        return refuse_as(
            400,
            "badName",
            "a folder's name cannot have a slash or a line break in it",
        );
    }
    Ok(name.to_string())
}

fn find<'a>(boxes: &'a [Folder], id: &str) -> Result<&'a Folder, Refused> {
    boxes.iter().find(|f| f.id == id).ok_or_else(|| Refused {
        status: 404,
        message: "there is no such folder".into(),
        code: Some("notFound"),
        detail: None,
    })
}

/// One of the person's own, not one LosOS Mail keeps.
fn own<'a>(boxes: &'a [Folder], id: &str) -> Result<&'a Folder, Refused> {
    let f = find(boxes, id)?;
    if f.role.is_some() {
        return refuse_as(
            403,
            "own",
            "LosOS Mail's own folders keep their names and places",
        );
    }
    Ok(f)
}

/// Whether `id` is `ancestor` or sits somewhere inside it.
fn within(boxes: &[Folder], id: &str, ancestor: &str) -> bool {
    let mut at = Some(id.to_string());
    for _ in 0..=boxes.len() {
        match at {
            Some(ref a) if a == ancestor => return true,
            Some(a) => {
                at = boxes
                    .iter()
                    .find(|f| f.id == a)
                    .and_then(|f| f.parent_id.clone())
            }
            None => return false,
        }
    }
    false
}

/// What a `Mailbox/set` refusal means to the person.
fn mailbox_refused(args: &Value, name: &str) -> Refused {
    match set_failure(args) {
        Some((_, d)) if d.contains("already exists") => Refused {
            status: 409,
            message: format!("there is already a folder called “{name}” there"),
            code: Some("exists"),
            detail: None,
        },
        Some((kind, d)) => Refused {
            status: 409,
            message: if d.is_empty() {
                kind.clone()
            } else {
                d.clone()
            },
            code: None,
            detail: Some(format!("the mail server refused a folder: {kind} {d}")),
        },
        None => odd("Mailbox/set"),
    }
}

/// The `parentId` a body asks for: absent, `null` (the top) or a folder.
fn parent_of(doc: &Value, boxes: &[Folder]) -> Result<Option<Option<String>>, Refused> {
    match doc.get("parentId") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(p)) => Ok(Some(Some(find(boxes, p)?.id.clone()))),
        Some(_) => refuse(400, "parentId is a folder's id or null"),
    }
}

/// `POST /api/mail/folders` `{"name", "parentId"?}`: a new folder of the
/// person's own, at the top or inside another one.
pub fn create_folder(server: &mut dyn Server, doc: &Value) -> Result<Value, Refused> {
    let name = check_name(doc.get("name").and_then(Value::as_str).unwrap_or(""))?;
    let boxes = folders(server)?;
    let parent = parent_of(doc, &boxes)?.flatten();
    let account = server.account().to_string();
    let resp = server.jmap(vec![json!(["Mailbox/set", {
        "accountId": account,
        "create": { "f": { "name": name, "parentId": parent } },
    }, "s"])])?;
    let args = answer(&resp, "Mailbox/set", "s")?;
    match args.pointer("/created/f/id").and_then(Value::as_str) {
        Some(id) => Ok(json!({ "id": id })),
        None => Err(mailbox_refused(args, &name)),
    }
}

/// `PUT /api/mail/folders/{id}` `{"name"?, "parentId"?}`: rename one of the
/// person's own folders, or move it to the top or into another folder.
pub fn update_folder(server: &mut dyn Server, id: &str, doc: &Value) -> Result<Value, Refused> {
    let boxes = folders(server)?;
    let folder = own(&boxes, id)?.clone();
    let mut patch = serde_json::Map::new();
    let name = match doc.get("name") {
        Some(Value::String(n)) => {
            let n = check_name(n)?;
            patch.insert("name".into(), json!(n));
            n
        }
        Some(_) => return refuse(400, "name is text"),
        None => folder.name.clone(),
    };
    if let Some(parent) = parent_of(doc, &boxes)? {
        if let Some(p) = &parent {
            if within(&boxes, p, id) {
                return refuse_as(409, "loop", "a folder cannot go inside itself");
            }
        }
        patch.insert("parentId".into(), json!(parent));
    }
    if patch.is_empty() {
        return refuse(400, "nothing to change");
    }
    let account = server.account().to_string();
    let resp = server.jmap(vec![json!(["Mailbox/set", {
        "accountId": account,
        "update": { id: patch },
    }, "s"])])?;
    let args = answer(&resp, "Mailbox/set", "s")?;
    if args.get("updated").and_then(|u| u.get(id)).is_some() {
        Ok(json!({ "ok": true }))
    } else {
        Err(mailbox_refused(args, &name))
    }
}

/// `DELETE /api/mail/folders/{id}`: its mail to the bin, then the folder.
/// Filters that filed into it stop doing so; one left with nothing to do
/// is turned off. How many messages went to the bin is the answer.
pub fn delete_folder(server: &mut dyn Server, id: &str) -> Result<Value, Refused> {
    let boxes = folders(server)?;
    let folder = own(&boxes, id)?;
    if boxes.iter().any(|f| f.parent_id.as_deref() == Some(id)) {
        return refuse_as(
            409,
            "hasFolders",
            format!(
                "“{}” has folders inside it; move or delete those first",
                folder.name
            ),
        );
    }
    let trash = boxes
        .iter()
        .find(|f| f.role.as_deref() == Some("trash"))
        .ok_or_else(|| odd("no Deleted Items"))?
        .id
        .clone();
    let account = server.account().to_string();
    let mut moved = 0usize;
    for _ in 0..MAX_ROUNDS {
        let resp = server.jmap(vec![json!(["Email/query", {
            "accountId": account,
            "filter": { "inMailbox": id },
            "limit": BATCH,
        }, "q"])])?;
        let ids: Vec<String> = answer(&resp, "Email/query", "q")?
            .get("ids")
            .and_then(Value::as_array)
            .ok_or_else(|| odd("Email/query ids"))?
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        if ids.is_empty() {
            break;
        }
        let update: serde_json::Map<String, Value> = ids
            .iter()
            .map(|m| {
                (
                    m.clone(),
                    json!({ format!("mailboxIds/{id}"): null, format!("mailboxIds/{trash}"): true }),
                )
            })
            .collect();
        let resp = server.jmap(vec![json!(["Email/set", {
            "accountId": account,
            "update": update,
        }, "u"])])?;
        let args = answer(&resp, "Email/set", "u")?;
        if let Some((kind, d)) = set_failure(args) {
            return Err(odd(&format!(
                "Email/set while emptying a folder: {kind} {d}"
            )));
        }
        moved += ids.len();
    }
    let resp = server.jmap(vec![json!(["Mailbox/set", {
        "accountId": account,
        "destroy": [id],
        "onDestroyRemoveEmails": false,
    }, "d"])])?;
    let args = answer(&resp, "Mailbox/set", "d")?;
    let gone = args
        .get("destroyed")
        .and_then(Value::as_array)
        .is_some_and(|d| d.iter().any(|v| v.as_str() == Some(id)));
    if !gone {
        let name = folder.name.clone();
        return Err(mailbox_refused(args, &name));
    }

    let (mut filters, _) = load_filters(server)?;
    let mut changed = false;
    for f in &mut filters {
        if f.move_to.as_deref() == Some(id) {
            f.move_to = None;
            if !f.mark_read && !f.flag {
                f.enabled = false;
            }
            changed = true;
        }
    }
    if changed {
        let boxes: Vec<Folder> = boxes.into_iter().filter(|f| f.id != id).collect();
        save_filters(server, &filters, &boxes)?;
    }
    Ok(json!({ "moved": moved }))
}

/// The LosOS script, if the person has one: its id and blob.
fn script(server: &mut dyn Server) -> Result<Option<(String, String)>, Refused> {
    let account = server.account().to_string();
    let resp = server.jmap(vec![
        json!(["SieveScript/get", { "accountId": account }, "g"]),
    ])?;
    Ok(answer(&resp, "SieveScript/get", "g")?
        .get("list")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|s| s.get("name").and_then(Value::as_str) == Some(sieve::SCRIPT_NAME))
        .and_then(|s| {
            Some((
                s.get("id")?.as_str()?.to_string(),
                s.get("blobId")?.as_str()?.to_string(),
            ))
        }))
}

/// The filters, and the script's id when there is one.
fn load_filters(server: &mut dyn Server) -> Result<(Vec<Filter>, Option<String>), Refused> {
    match script(server)? {
        Some((id, blob)) => {
            let bytes = server.download(&blob)?;
            Ok((
                sieve::from_script(&String::from_utf8_lossy(&bytes)),
                Some(id),
            ))
        }
        None => Ok((Vec::new(), None)),
    }
}

/// Write `filters` as the person's active script.
fn save_filters(
    server: &mut dyn Server,
    filters: &[Filter],
    boxes: &[Folder],
) -> Result<(), Refused> {
    let text = sieve::script(filters, &|id| {
        boxes.iter().find(|f| f.id == id).map(|f| f.name.clone())
    });
    let blob = server.upload(text.as_bytes(), "application/sieve")?;
    let existing = script(server)?.map(|(id, _)| id);
    let account = server.account().to_string();
    let (set, tag_id) = match &existing {
        Some(id) => (
            json!({ "accountId": account, "update": { id: { "blobId": blob } }, "onSuccessActivateScript": id }),
            id.clone(),
        ),
        None => (
            json!({
                "accountId": account,
                "create": { "s": { "name": sieve::SCRIPT_NAME, "blobId": blob } },
                "onSuccessActivateScript": "#s",
            }),
            "s".to_string(),
        ),
    };
    let resp = server.jmap(vec![json!(["SieveScript/set", set, "s"])])?;
    let args = answer(&resp, "SieveScript/set", "s")?;
    let done = args.get("updated").and_then(|u| u.get(&tag_id)).is_some()
        || args.get("created").and_then(|c| c.get(&tag_id)).is_some();
    if done {
        Ok(())
    } else {
        let (kind, d) = set_failure(args).unwrap_or_default();
        Err(odd(&format!(
            "SieveScript/set refused the filters: {kind} {d}"
        )))
    }
}

/// `GET /api/mail/filters`.
pub fn get_filters(server: &mut dyn Server) -> Result<Value, Refused> {
    let (filters, _) = load_filters(server)?;
    Ok(json!({ "filters": filters }))
}

/// `PUT /api/mail/filters` `{"filters": [...]}`: the whole list, in order.
pub fn put_filters(server: &mut dyn Server, doc: &Value) -> Result<Value, Refused> {
    let boxes = folders(server)?;
    let ids: Vec<String> = boxes.iter().map(|f| f.id.clone()).collect();
    let filters = sieve::parse_filters(doc, &ids)?;
    save_filters(server, &filters, &boxes)?;
    Ok(json!({ "filters": filters }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// A mail server in memory: folders, which folder each message is in,
    /// one script, and every call made.
    struct Fake {
        boxes: Vec<Folder>,
        mail: BTreeMap<String, Vec<String>>,
        script: Option<String>,
        blobs: BTreeMap<String, Vec<u8>>,
        next: u32,
        calls: Vec<String>,
    }

    impl Fake {
        fn new() -> Fake {
            let f = |id: &str, name: &str, parent: Option<&str>, role: Option<&str>| Folder {
                id: id.into(),
                name: name.into(),
                parent_id: parent.map(str::to_string),
                role: role.map(str::to_string),
            };
            Fake {
                boxes: vec![
                    f("a", "Inbox", None, Some("inbox")),
                    f("b", "Deleted Items", None, Some("trash")),
                    f("h", "Projects", None, None),
                    f("i", "LosOS box", Some("h"), None),
                ],
                mail: BTreeMap::new(),
                script: None,
                blobs: BTreeMap::new(),
                next: 0,
                calls: Vec::new(),
            }
        }

        fn id(&mut self) -> String {
            self.next += 1;
            format!("n{}", self.next)
        }

        fn one(&mut self, call: &Value) -> Value {
            let name = call[0].as_str().unwrap().to_string();
            let a = &call[1];
            let tag = call[2].clone();
            self.calls.push(name.clone());
            let out = match name.as_str() {
                "Mailbox/get" => json!({ "list": self.boxes }),
                "Mailbox/set" => {
                    let mut out = json!({});
                    if let Some(c) = a["create"]["f"].as_object() {
                        let name = c["name"].as_str().unwrap().to_string();
                        if self.boxes.iter().any(|b| b.name == name) {
                            out["notCreated"] = json!({ "f": { "type": "invalidProperties",
                                "description": format!("A mailbox with name '{name}' already exists.") } });
                        } else {
                            let id = self.id();
                            self.boxes.push(Folder {
                                id: id.clone(),
                                name,
                                parent_id: c["parentId"].as_str().map(str::to_string),
                                role: None,
                            });
                            out["created"] = json!({ "f": { "id": id } });
                        }
                    }
                    if let Some(u) = a["update"].as_object() {
                        for (id, patch) in u {
                            let b = self.boxes.iter_mut().find(|b| &b.id == id).unwrap();
                            if let Some(n) = patch.get("name") {
                                b.name = n.as_str().unwrap().into();
                            }
                            if let Some(p) = patch.get("parentId") {
                                b.parent_id = p.as_str().map(str::to_string);
                            }
                            out["updated"] = json!({ id.clone(): null });
                        }
                    }
                    if let Some(d) = a["destroy"].as_array() {
                        let id = d[0].as_str().unwrap();
                        if self.mail.values().any(|m| m.iter().any(|x| x == id)) {
                            out["notDestroyed"] = json!({ id: { "type": "mailboxHasEmail" } });
                        } else {
                            self.boxes.retain(|b| b.id != id);
                            out["destroyed"] = json!([id]);
                        }
                    }
                    out
                }
                "Email/query" => {
                    let inside = a["filter"]["inMailbox"].as_str().unwrap();
                    let limit = a["limit"].as_u64().unwrap() as usize;
                    let ids: Vec<&String> = self
                        .mail
                        .iter()
                        .filter(|(_, m)| m.iter().any(|x| x == inside))
                        .map(|(id, _)| id)
                        .take(limit)
                        .collect();
                    json!({ "ids": ids })
                }
                "Email/set" => {
                    for (id, patch) in a["update"].as_object().unwrap() {
                        let m = self.mail.get_mut(id).unwrap();
                        for (k, v) in patch.as_object().unwrap() {
                            let b = k.strip_prefix("mailboxIds/").unwrap().to_string();
                            if v.is_null() {
                                m.retain(|x| x != &b);
                            } else {
                                m.push(b);
                            }
                        }
                    }
                    json!({})
                }
                "SieveScript/get" => json!({ "list": self.script.as_ref().map(|b| vec![
                    json!({ "id": "sc", "name": "losos", "blobId": b, "isActive": true })
                ]).unwrap_or_default() }),
                "SieveScript/set" => {
                    let blob = a["update"]["sc"]["blobId"]
                        .as_str()
                        .or_else(|| a["create"]["s"]["blobId"].as_str())
                        .unwrap()
                        .to_string();
                    let created = self.script.is_none();
                    assert_eq!(
                        a["onSuccessActivateScript"],
                        if created { "#s" } else { "sc" }
                    );
                    self.script = Some(blob);
                    if created {
                        json!({ "created": { "s": { "id": "sc" } } })
                    } else {
                        json!({ "updated": { "sc": null } })
                    }
                }
                other => panic!("unexpected {other}"),
            };
            json!([name, out, tag])
        }

        fn sieve(&self) -> String {
            String::from_utf8(self.blobs[self.script.as_ref().unwrap()].clone()).unwrap()
        }
    }

    impl Server for Fake {
        fn account(&self) -> &str {
            "c"
        }
        fn jmap(&mut self, calls: Vec<Value>) -> Result<Value, Refused> {
            for c in &calls {
                assert_eq!(c[1]["accountId"], "c");
            }
            let resps: Vec<Value> = calls.iter().map(|c| self.one(c)).collect();
            Ok(json!({ "methodResponses": resps }))
        }
        fn upload(&mut self, bytes: &[u8], kind: &str) -> Result<String, Refused> {
            assert_eq!(kind, "application/sieve");
            let id = self.id();
            self.blobs.insert(id.clone(), bytes.to_vec());
            Ok(id)
        }
        fn download(&mut self, blob: &str) -> Result<Vec<u8>, Refused> {
            self.blobs.get(blob).cloned().ok_or_else(|| odd("no blob"))
        }
    }

    fn named<'a>(s: &'a Fake, name: &str) -> &'a Folder {
        s.boxes.iter().find(|b| b.name == name).unwrap()
    }

    #[test]
    fn a_folder_is_made_at_the_top_or_inside_another() {
        let mut s = Fake::new();
        let top = create_folder(&mut s, &json!({ "name": " Receipts " })).unwrap();
        assert_eq!(named(&s, "Receipts").parent_id, None);
        assert_eq!(top["id"], json!(named(&s, "Receipts").id));
        create_folder(&mut s, &json!({ "name": "2026", "parentId": "h" })).unwrap();
        assert_eq!(named(&s, "2026").parent_id.as_deref(), Some("h"));
    }

    #[test]
    fn a_folder_name_is_checked_and_a_twin_refused() {
        let mut s = Fake::new();
        for (doc, status) in [
            (json!({ "name": "" }), 400),
            (json!({ "name": "a/b" }), 400),
            (json!({ "name": "a\nb" }), 400),
            (json!({ "name": "x".repeat(MAX_NAME + 1) }), 400),
            (json!({ "name": "Ok", "parentId": "zz" }), 404),
            (json!({ "name": "Ok", "parentId": 3 }), 400),
            (json!({ "name": "Projects" }), 409),
        ] {
            assert_eq!(
                create_folder(&mut s, &doc).unwrap_err().status,
                status,
                "{doc}"
            );
        }
        let twin = create_folder(&mut s, &json!({ "name": "Projects" })).unwrap_err();
        assert!(
            twin.message.contains("already a folder called “Projects”"),
            "{twin}"
        );
        assert_eq!(twin.code, Some("exists"));
    }

    #[test]
    fn a_folder_is_renamed_and_moved_but_never_into_itself() {
        let mut s = Fake::new();
        update_folder(&mut s, "i", &json!({ "name": "Box" })).unwrap();
        assert_eq!(named(&s, "Box").id, "i");
        update_folder(&mut s, "i", &json!({ "parentId": null })).unwrap();
        assert_eq!(named(&s, "Box").parent_id, None);
        update_folder(&mut s, "i", &json!({ "parentId": "h" })).unwrap();
        let into_child = update_folder(&mut s, "h", &json!({ "parentId": "i" })).unwrap_err();
        assert_eq!((into_child.status, into_child.code), (409, Some("loop")));
        assert_eq!(
            update_folder(&mut s, "h", &json!({ "parentId": "h" }))
                .unwrap_err()
                .status,
            409
        );
        assert_eq!(
            update_folder(&mut s, "h", &json!({})).unwrap_err().status,
            400
        );
    }

    #[test]
    fn mails_own_folders_stay_put() {
        let mut s = Fake::new();
        assert_eq!(
            update_folder(&mut s, "a", &json!({ "name": "Mine" }))
                .unwrap_err()
                .status,
            403
        );
        assert_eq!(delete_folder(&mut s, "b").unwrap_err().status, 403);
        assert_eq!(delete_folder(&mut s, "zz").unwrap_err().status, 404);
        assert!(s.calls.iter().all(|c| c == "Mailbox/get"), "{:?}", s.calls);
    }

    #[test]
    fn a_folder_with_folders_inside_is_refused() {
        let mut s = Fake::new();
        let r = delete_folder(&mut s, "h").unwrap_err();
        assert_eq!(r.status, 409);
        assert!(
            r.message.contains("“Projects” has folders inside it"),
            "{r}"
        );
    }

    #[test]
    fn deleting_a_folder_bins_its_mail_first() {
        let mut s = Fake::new();
        for n in 0..(BATCH + 3) {
            s.mail.insert(format!("m{n}"), vec!["i".into()]);
        }
        // In two folders at once: it leaves this one and keeps the other.
        s.mail.insert("both".into(), vec!["i".into(), "a".into()]);
        let r = delete_folder(&mut s, "i").unwrap();
        assert_eq!(r["moved"], BATCH + 4);
        assert!(s.boxes.iter().all(|b| b.id != "i"));
        assert_eq!(s.mail["m0"], vec!["b".to_string()]);
        assert_eq!(s.mail["both"], vec!["a".to_string(), "b".to_string()]);
        assert_eq!(s.calls.iter().filter(|c| *c == "Email/set").count(), 2);
    }

    fn eva(to: Option<&str>) -> Value {
        json!({ "name": "Eva", "conditions": [{ "field": "from", "value": "eva@" }],
                "moveTo": to, "flag": false })
    }

    #[test]
    fn filters_are_saved_as_the_active_script_and_read_back() {
        let mut s = Fake::new();
        assert_eq!(get_filters(&mut s).unwrap(), json!({ "filters": [] }));
        let saved = put_filters(&mut s, &json!({ "filters": [eva(Some("i"))] })).unwrap();
        assert!(
            s.sieve()
                .contains(r#"fileinto :mailboxid "i" "LosOS box";"#),
            "{}",
            s.sieve()
        );
        assert_eq!(get_filters(&mut s).unwrap(), saved);
        // A second save replaces the same script.
        put_filters(&mut s, &json!({ "filters": [] })).unwrap();
        assert_eq!(get_filters(&mut s).unwrap(), json!({ "filters": [] }));
        assert_eq!(
            put_filters(&mut s, &json!({ "filters": [eva(Some("zz"))] }))
                .unwrap_err()
                .status,
            400
        );
    }

    #[test]
    fn deleting_a_folder_turns_off_the_filters_that_filed_into_it() {
        let mut s = Fake::new();
        let mut flagging = eva(Some("i"));
        flagging["flag"] = json!(true);
        flagging["name"] = json!("Flag Eva");
        let other = eva(Some("h"));
        put_filters(
            &mut s,
            &json!({ "filters": [eva(Some("i")), flagging, other] }),
        )
        .unwrap();
        delete_folder(&mut s, "i").unwrap();
        let f = get_filters(&mut s).unwrap();
        assert_eq!(f["filters"][0]["enabled"], false);
        assert_eq!(f["filters"][0]["moveTo"], Value::Null);
        assert_eq!(f["filters"][1]["enabled"], true);
        assert_eq!(f["filters"][1]["moveTo"], Value::Null);
        assert_eq!(f["filters"][2]["moveTo"], "h");
        assert!(!s.sieve().contains("\"i\""), "{}", s.sieve());
        // The list the page holds now saves as it is.
        put_filters(&mut s, &f).unwrap();
    }
}
