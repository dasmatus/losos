//! LosOS Contacts: each person's address book, kept in Matrix on the box's
//! homeserver (`crate::chat`, `modules/chat.nix`).
//!
//! Where it lives. Every person gets one private room of their own, made
//! the first time they open Contacts and found again through their account
//! data (`org.losos.contacts` → `{"room_id"}`):
//!
//!   * the room's create event has `"type": "org.losos.contacts"`, so chat
//!     clients that know it (LosOS Chat's room list) leave it out, and
//!     `"m.federate": false`, so it never leaves this homeserver even on a
//!     box whose chat federates;
//!   * each contact is one state event, `org.losos.contact`, its state key
//!     the contact's ID. Changing a contact replaces that one event, two
//!     pages editing two contacts never overwrite each other, and the
//!     room's timeline keeps every earlier version. Removing a contact
//!     writes an empty event in its place, Matrix's way to clear state.
//!
//! The room rather than account data because account data is one document
//! per type: a book of a few hundred contacts would be rewritten whole on
//! every change, and every chat page would receive it again in its sync.
//! The photo sits inside the contact's event, scaled to [`PHOTO_SIDE`]
//! pixels and re-encoded (which drops whatever metadata the file carried),
//! so a contact is one event and removing it leaves nothing behind in the
//! media store.
//!
//! lososd writes the room as the person, through chat's application
//! service ([`crate::chat::Chat::as_person`]); there is no device and no
//! token of theirs involved, and the room is only ever theirs. Contacts
//! therefore exists exactly where chat does: on the shared side
//! (`lososInternal.sharedSide`), and [`ContactsError::Off`] otherwise.
//!
//! Files come and go as vCards (`crate::vcard`).

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use serde::Serialize;
use serde_json::{json, Value};

use crate::chat::{encode, Chat, ChatError, Reply};
use crate::people::Person;
use crate::vcard::{self, Contact};

/// The create event type of a person's contacts room.
pub const ROOM_TYPE: &str = "org.losos.contacts";
/// The state event type of one contact.
pub const EVENT: &str = "org.losos.contact";
/// The account data type that names the room.
pub const POINTER: &str = "org.losos.contacts";
/// The side of a contact's photo, in pixels.
pub const PHOTO_SIDE: u32 = 160;
/// The most contacts one import takes.
pub const MAX_IMPORT: usize = 5000;
/// The largest vCard file an import reads.
pub const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;
/// The most contacts one book holds.
pub const MAX_CONTACTS: usize = 10_000;
/// The largest picture a photo upload takes.
pub const MAX_PHOTO_BYTES: usize = 8 * 1024 * 1024;

/// Why Contacts could not do what was asked.
#[derive(Debug, thiserror::Error)]
pub enum ContactsError {
    #[error(transparent)]
    Chat(#[from] ChatError),
    #[error("there is no such contact")]
    NotFound,
    #[error("{0}")]
    Bad(String),
}

impl ContactsError {
    pub fn status(&self) -> u16 {
        match self {
            ContactsError::Chat(e) => e.status(),
            ContactsError::NotFound => 404,
            ContactsError::Bad(_) => 400,
        }
    }

    /// Chat is off on this box, so Contacts is too.
    pub fn is_off(&self) -> bool {
        matches!(self, ContactsError::Chat(ChatError::Off))
    }
}

type Result<T> = std::result::Result<T, ContactsError>;

/// One contact as the page sees it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stored {
    pub id: String,
    #[serde(flatten)]
    pub contact: Contact,
    /// `/api/contacts/<id>/photo?v=<updated>` when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub photo: Option<String>,
    /// When it last changed, in milliseconds since the epoch.
    pub updated: i64,
    #[serde(skip)]
    jpeg: Option<Vec<u8>>,
}

/// What the page sends to save a contact: the fields, and what to do with
/// the photo.
pub enum Photo {
    Keep,
    Remove,
    Set(Vec<u8>),
}

/// What an import did.
#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub added: usize,
    pub updated: usize,
    /// Cards with nothing to call the contact by.
    pub skipped: usize,
    /// Photos that could not be read; their contacts came in without one.
    pub photos_dropped: usize,
    /// Cards past [`MAX_IMPORT`] or the book's [`MAX_CONTACTS`].
    pub left_out: usize,
}

/// Someone on this box, for [`Contacts::suggest`]: built by the HTTP layer
/// from LosOS Login's people, chat's server name, mail's domain and the
/// profiles.
#[derive(Debug, Clone, Default)]
pub struct Neighbour {
    pub name: String,
    pub display_name: String,
    pub email: Option<String>,
    pub matrix_id: String,
    pub picture: Option<String>,
}

/// Which people a picker wants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Want {
    Any,
    Email,
    Matrix,
    Mesh,
}

impl Want {
    pub fn parse(s: &str) -> Want {
        match s {
            "email" => Want::Email,
            "matrix" => Want::Matrix,
            "mesh" => Want::Mesh,
            _ => Want::Any,
        }
    }
}

/// One person a picker offers: someone on this box, or a contact.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub key: String,
    pub source: &'static str,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    pub emails: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matrix_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_id: Option<String>,
}

/// Contacts as lososd runs it: which room is whose, remembered.
#[derive(Default)]
pub struct Contacts {
    rooms: Mutex<HashMap<String, String>>,
    /// Held while a room is looked up or made, so two first requests make
    /// one room.
    making: Mutex<()>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn refused(r: &Reply) -> ContactsError {
    ContactsError::Chat(ChatError::Homeserver {
        status: r.status,
        errcode: r.errcode(),
    })
}

/// A new contact ID: 32 hex characters.
fn new_id() -> Result<String> {
    use ring::rand::SecureRandom;
    let mut b = [0u8; 16];
    ring::rand::SystemRandom::new()
        .fill(&mut b)
        .map_err(|_| ContactsError::Bad("no randomness for a contact ID".into()))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

/// A contact ID as the page may name one.
pub fn valid_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// The `UID` a contact made here gets: a URN from its ID.
fn uid_for(id: &str) -> String {
    format!(
        "urn:uuid:{}-{}-{}-{}-{}",
        &id[..8],
        &id[8..12],
        &id[12..16],
        &id[16..20],
        &id[20..]
    )
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// The photo as the book keeps it: square, [`PHOTO_SIDE`] on a side, JPEG,
/// turned upright, with nothing of the original file left.
pub fn scale_photo(bytes: &[u8]) -> Result<Vec<u8>> {
    use image::{DynamicImage, ImageDecoder, ImageReader};
    use std::io::Cursor;
    if bytes.len() > MAX_PHOTO_BYTES {
        return Err(ContactsError::Bad("the photo is too large".into()));
    }
    let bad = |_| ContactsError::Bad("the photo could not be read".into());
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(256 * 1024 * 1024);
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| ContactsError::Bad("the photo could not be read".into()))?;
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(bad)?;
    let orientation = decoder.orientation().map_err(bad)?;
    let mut picture = DynamicImage::from_decoder(decoder).map_err(bad)?;
    picture.apply_orientation(orientation);
    let side = picture.width().min(picture.height());
    if side == 0 {
        return Err(ContactsError::Bad("the photo is empty".into()));
    }
    let square = picture.crop_imm(
        (picture.width() - side) / 2,
        (picture.height() - side) / 2,
        side,
        side,
    );
    let small = square.resize_exact(
        PHOTO_SIDE,
        PHOTO_SIDE,
        image::imageops::FilterType::Lanczos3,
    );
    let mut out = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 82);
    DynamicImage::ImageRgb8(small.to_rgb8())
        .write_with_encoder(encoder)
        .map_err(|_| ContactsError::Bad("the photo could not be written".into()))?;
    Ok(out)
}

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

/// A contact from its state event, or `None` for a cleared one or one some
/// other client wrote in a shape this does not read.
fn from_event(id: &str, content: &Value) -> Option<Stored> {
    use base64::Engine;
    if !valid_id(id) {
        return None;
    }
    let mut contact: Contact = serde_json::from_value(content.get("contact")?.clone()).ok()?;
    if !contact.tidy() {
        return None;
    }
    let updated = content.get("updated").and_then(Value::as_i64).unwrap_or(0);
    let jpeg = content
        .get("photo")
        .and_then(Value::as_str)
        .and_then(|p| b64().decode(p).ok());
    Some(Stored {
        id: id.to_string(),
        photo: jpeg
            .as_ref()
            .map(|_| format!("/api/contacts/{id}/photo?v={updated}")),
        contact,
        updated,
        jpeg,
    })
}

fn to_event(contact: &Contact, jpeg: Option<&[u8]>, updated: i64) -> Value {
    use base64::Engine;
    let mut v = json!({ "v": 1, "contact": contact, "updated": updated });
    if let Some(j) = jpeg {
        v["photo"] = json!(b64().encode(j));
    }
    v
}

/// Lower case, with the accents of Latin letters taken off, so "stefan"
/// finds "Štefan".
fn folded(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
            'č' | 'ć' | 'ç' => 'c',
            'ď' => 'd',
            'é' | 'è' | 'ê' | 'ë' | 'ě' | 'ē' | 'ę' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ľ' | 'ĺ' | 'ł' => 'l',
            'ň' | 'ñ' | 'ń' => 'n',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ő' | 'ø' => 'o',
            'ř' | 'ŕ' => 'r',
            'š' | 'ś' | 'ş' => 's',
            'ť' | 'ţ' => 't',
            'ú' | 'ù' | 'û' | 'ü' | 'ů' | 'ű' => 'u',
            'ý' | 'ÿ' => 'y',
            'ž' | 'ź' | 'ż' => 'z',
            'ß' => 's',
            c => c,
        })
        .collect()
}

/// How well `q` matches `fields`: 0 for the start of the first field (the
/// name), 1 for the start of a word or of another field, 2 for anywhere,
/// `None` for no match.
fn rank(q: &str, fields: &[&str]) -> Option<u8> {
    if q.is_empty() {
        return Some(2);
    }
    let mut best = None;
    for (i, f) in fields.iter().enumerate() {
        let f = folded(f);
        let Some(at) = f.find(q) else { continue };
        let r = if at == 0 && i == 0 {
            0
        } else if at == 0 || f[..at].ends_with([' ', '.', '-', '@', ':']) {
            1
        } else {
            2
        };
        best = Some(best.map_or(r, |b: u8| b.min(r)));
    }
    best
}

impl Contacts {
    /// The person's contacts room, made the first time.
    fn room(&self, chat: &Chat, person: &Person) -> Result<String> {
        if let Some(r) = lock(&self.rooms).get(&person.name) {
            return Ok(r.clone());
        }
        let _one = lock(&self.making);
        if let Some(r) = lock(&self.rooms).get(&person.name) {
            return Ok(r.clone());
        }
        let user = chat.user_id(&person.name)?;
        let pointer = format!(
            "/_matrix/client/v3/user/{}/account_data/{POINTER}",
            encode(&user)
        );
        let found = chat.as_person(person, "GET", &pointer, None)?;
        let mut room = match found.status {
            200 => found
                .body
                .get("room_id")
                .and_then(Value::as_str)
                .map(str::to_string),
            404 => None,
            _ => return Err(refused(&found)),
        };
        // A pointer to a room the person is no longer in (left it from some
        // other client) is a book that is gone; start a new one.
        if let Some(r) = &room {
            let path = format!(
                "/_matrix/client/v3/rooms/{}/state/m.room.create/",
                encode(r)
            );
            let here = chat.as_person(person, "GET", &path, None)?;
            if here.status != 200 {
                tracing::warn!(status = here.status, person = %person.name, "contacts room unreachable; making a new one");
                room = None;
            }
        }
        let room = match room {
            Some(r) => r,
            None => {
                let made = chat.as_person(
                    person,
                    "POST",
                    "/_matrix/client/v3/createRoom",
                    Some(&json!({
                        "preset": "private_chat",
                        "visibility": "private",
                        "name": "Contacts",
                        "creation_content": { "m.federate": false, "type": ROOM_TYPE },
                        "initial_state": [{
                            "type": "m.room.history_visibility",
                            "state_key": "",
                            "content": { "history_visibility": "joined" },
                        }],
                    })),
                )?;
                let id = made
                    .body
                    .get("room_id")
                    .and_then(Value::as_str)
                    .filter(|_| made.status == 200)
                    .ok_or_else(|| refused(&made))?
                    .to_string();
                let set =
                    chat.as_person(person, "PUT", &pointer, Some(&json!({ "room_id": id })))?;
                if set.status != 200 {
                    return Err(refused(&set));
                }
                id
            }
        };
        lock(&self.rooms).insert(person.name.clone(), room.clone());
        Ok(room)
    }

    fn forget(&self, person: &Person) {
        lock(&self.rooms).remove(&person.name);
    }

    /// A call into the person's room, retried once with the room looked up
    /// again when the remembered one refuses (the person left it).
    fn in_room(
        &self,
        chat: &Chat,
        person: &Person,
        method: &str,
        tail: &str,
        body: Option<&Value>,
    ) -> Result<Reply> {
        for attempt in 0..2 {
            let room = self.room(chat, person)?;
            let path = format!("/_matrix/client/v3/rooms/{}{tail}", encode(&room));
            let r = chat.as_person(person, method, &path, body)?;
            if r.status == 403 && attempt == 0 {
                self.forget(person);
                continue;
            }
            return Ok(r);
        }
        unreachable!("the loop returns on its second pass")
    }

    /// Every contact in the person's book, by name.
    pub fn list(&self, chat: &Chat, person: &Person) -> Result<Vec<Stored>> {
        let r = self.in_room(chat, person, "GET", "/state", None)?;
        if r.status != 200 {
            return Err(refused(&r));
        }
        let mut out: Vec<Stored> = r
            .body
            .as_array()
            .map(|events| {
                events
                    .iter()
                    .filter(|e| e.get("type").and_then(Value::as_str) == Some(EVENT))
                    .filter_map(|e| {
                        from_event(
                            e.get("state_key").and_then(Value::as_str)?,
                            e.get("content")?,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.sort_by(|a, b| {
            folded(&a.contact.name)
                .cmp(&folded(&b.contact.name))
                .then(a.id.cmp(&b.id))
        });
        Ok(out)
    }

    /// One contact.
    pub fn get(&self, chat: &Chat, person: &Person, id: &str) -> Result<Stored> {
        if !valid_id(id) {
            return Err(ContactsError::NotFound);
        }
        let r = self.in_room(chat, person, "GET", &format!("/state/{EVENT}/{id}"), None)?;
        match r.status {
            200 => from_event(id, &r.body).ok_or(ContactsError::NotFound),
            404 => Err(ContactsError::NotFound),
            _ => Err(refused(&r)),
        }
    }

    /// A contact's photo, JPEG.
    pub fn photo(&self, chat: &Chat, person: &Person, id: &str) -> Result<Vec<u8>> {
        self.get(chat, person, id)?
            .jpeg
            .ok_or(ContactsError::NotFound)
    }

    fn write(&self, chat: &Chat, person: &Person, id: &str, content: &Value) -> Result<()> {
        let r = self.in_room(
            chat,
            person,
            "PUT",
            &format!("/state/{EVENT}/{id}"),
            Some(content),
        )?;
        if r.status != 200 {
            return Err(refused(&r));
        }
        Ok(())
    }

    /// Save a contact: a new one when `id` is `None`. The fields are tidied
    /// the way an import tidies them.
    pub fn save(
        &self,
        chat: &Chat,
        person: &Person,
        id: Option<&str>,
        mut contact: Contact,
        photo: Photo,
    ) -> Result<Stored> {
        if !contact.tidy() {
            return Err(ContactsError::Bad("give the contact a name".into()));
        }
        let (id, old) = match id {
            Some(id) => (id.to_string(), Some(self.get(chat, person, id)?)),
            None => {
                if self.list(chat, person)?.len() >= MAX_CONTACTS {
                    return Err(ContactsError::Bad(format!(
                        "the book holds at most {MAX_CONTACTS} contacts"
                    )));
                }
                (new_id()?, None)
            }
        };
        if contact.uid.is_empty() {
            contact.uid = old
                .as_ref()
                .map(|o| o.contact.uid.clone())
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| uid_for(&id));
        }
        let jpeg = match photo {
            Photo::Keep => old.and_then(|o| o.jpeg),
            Photo::Remove => None,
            Photo::Set(bytes) => Some(scale_photo(&bytes)?),
        };
        let updated = now_ms();
        let content = to_event(&contact, jpeg.as_deref(), updated);
        self.write(chat, person, &id, &content)?;
        from_event(&id, &content).ok_or(ContactsError::NotFound)
    }

    /// Remove a contact.
    pub fn remove(&self, chat: &Chat, person: &Person, id: &str) -> Result<()> {
        self.get(chat, person, id)?;
        self.write(chat, person, id, &json!({}))
    }

    /// Take the cards of a vCard file into the book. A card whose `UID` is
    /// already in the book, or, without one, whose name and first email or
    /// phone match a contact's, updates that contact; the rest are added.
    pub fn import(&self, chat: &Chat, person: &Person, bytes: &[u8]) -> Result<Report> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err(ContactsError::Bad("the file is too large".into()));
        }
        let parsed = vcard::parse(bytes);
        let mut report = Report {
            skipped: parsed.skipped,
            ..Report::default()
        };
        if parsed.cards.is_empty() && parsed.skipped == 0 {
            return Err(ContactsError::Bad(
                "the file has no contacts in it (no BEGIN:VCARD)".into(),
            ));
        }
        let book = self.list(chat, person)?;
        let mut count = book.len();
        let mut by_uid: HashMap<String, Stored> = HashMap::new();
        let mut by_key: HashMap<String, Stored> = HashMap::new();
        for s in book {
            if let Some(k) = loose_key(&s.contact) {
                by_key.insert(k, s.clone());
            }
            if !s.contact.uid.is_empty() {
                by_uid.insert(s.contact.uid.clone(), s);
            }
        }
        let mut done: HashSet<String> = HashSet::new();
        for (i, card) in parsed.cards.into_iter().enumerate() {
            if i >= MAX_IMPORT {
                report.left_out += 1;
                continue;
            }
            let mut contact = card.contact;
            let old = if contact.uid.is_empty() {
                loose_key(&contact).and_then(|k| by_key.get(&k)).cloned()
            } else {
                by_uid.get(&contact.uid).cloned()
            };
            let id = match &old {
                Some(o) if !done.contains(&o.id) => o.id.clone(),
                _ => {
                    if count >= MAX_CONTACTS {
                        report.left_out += 1;
                        continue;
                    }
                    count += 1;
                    new_id()?
                }
            };
            let replacing = old.as_ref().is_some_and(|o| o.id == id);
            if contact.uid.is_empty() {
                contact.uid = old
                    .as_ref()
                    .filter(|_| replacing)
                    .map(|o| o.contact.uid.clone())
                    .filter(|u| !u.is_empty())
                    .unwrap_or_else(|| uid_for(&id));
            }
            let jpeg = match card.photo {
                Some(bytes) => match scale_photo(&bytes) {
                    Ok(j) => Some(j),
                    Err(_) => {
                        report.photos_dropped += 1;
                        None
                    }
                },
                None if replacing => old.and_then(|o| o.jpeg),
                None => None,
            };
            self.write(
                chat,
                person,
                &id,
                &to_event(&contact, jpeg.as_deref(), now_ms()),
            )?;
            done.insert(id);
            if replacing {
                report.updated += 1;
            } else {
                report.added += 1;
            }
        }
        Ok(report)
    }

    /// The person's contacts as one vCard file: `ids` only, or all.
    pub fn export(
        &self,
        chat: &Chat,
        person: &Person,
        ids: Option<&[String]>,
        version: vcard::Version,
    ) -> Result<String> {
        let all = self.list(chat, person)?;
        let picked: Vec<&Stored> = match ids {
            Some(ids) => {
                let want: HashSet<&str> = ids.iter().map(String::as_str).collect();
                all.iter()
                    .filter(|s| want.contains(s.id.as_str()))
                    .collect()
            }
            None => all.iter().collect(),
        };
        if ids.is_some() && picked.is_empty() {
            return Err(ContactsError::NotFound);
        }
        let uids: Vec<String> = picked
            .iter()
            .map(|s| {
                if s.contact.uid.is_empty() {
                    uid_for(&s.id)
                } else {
                    s.contact.uid.clone()
                }
            })
            .collect();
        let out: Vec<vcard::Out<'_>> = picked
            .iter()
            .zip(&uids)
            .map(|(s, uid)| vcard::Out {
                contact: &s.contact,
                uid,
                updated_ms: s.updated,
                photo: s.jpeg.as_deref(),
            })
            .collect();
        Ok(vcard::write(&out, version))
    }

    /// The people a picker offers for `q`: the person's contacts and the
    /// people on this box, best match first. A box person who is also a
    /// contact (by Matrix ID) shows once, as the contact.
    pub fn suggest(
        &self,
        chat: &Chat,
        person: &Person,
        neighbours: &[Neighbour],
        q: &str,
        want: Want,
        limit: usize,
    ) -> Result<Vec<Suggestion>> {
        let q = folded(q.trim());
        let book = self.list(chat, person)?;
        let mut ranked: Vec<(u8, Suggestion)> = Vec::new();
        let mut known: HashSet<String> = HashSet::new();
        for s in &book {
            let c = &s.contact;
            if !c.matrix.is_empty() {
                known.insert(c.matrix.clone());
            }
            let fits = match want {
                Want::Any => true,
                Want::Email => !c.emails.is_empty(),
                Want::Matrix => !c.matrix.is_empty(),
                Want::Mesh => !c.mesh.is_empty(),
            };
            if !fits {
                continue;
            }
            let mut fields = vec![c.name.as_str(), c.nickname.as_str(), c.org.as_str()];
            fields.extend(c.emails.iter().map(|e| e.value.as_str()));
            fields.push(c.matrix.as_str());
            fields.push(c.mesh.as_str());
            let Some(r) = rank(&q, &fields) else { continue };
            ranked.push((
                r,
                Suggestion {
                    key: format!("contact:{}", s.id),
                    source: "contact",
                    name: c.name.clone(),
                    picture: s.photo.clone(),
                    emails: c.emails.iter().map(|e| e.value.clone()).collect(),
                    matrix_id: (!c.matrix.is_empty()).then(|| c.matrix.clone()),
                    mesh_address: (!c.mesh.is_empty()).then(|| c.mesh.clone()),
                    contact_id: Some(s.id.clone()),
                },
            ));
        }
        for n in neighbours {
            if n.name == person.name || known.contains(&n.matrix_id) {
                continue;
            }
            let fits = match want {
                Want::Any | Want::Matrix => true,
                Want::Email => n.email.is_some(),
                // Sharing with someone on the same box is LosOS Cloud's own
                // share, not a mesh address.
                Want::Mesh => false,
            };
            if !fits {
                continue;
            }
            let shown = if n.display_name.trim().is_empty() {
                n.name.clone()
            } else {
                n.display_name.trim().to_string()
            };
            let email = n.email.clone().unwrap_or_default();
            let Some(r) = rank(&q, &[&shown, &n.name, &email, &n.matrix_id]) else {
                continue;
            };
            ranked.push((
                r,
                Suggestion {
                    key: format!("box:{}", n.name),
                    source: "box",
                    name: shown,
                    picture: n.picture.clone(),
                    emails: n.email.iter().cloned().collect(),
                    matrix_id: Some(n.matrix_id.clone()),
                    mesh_address: None,
                    contact_id: None,
                },
            ));
        }
        ranked.sort_by(|(ra, a), (rb, b)| {
            ra.cmp(rb)
                .then_with(|| folded(&a.name).cmp(&folded(&b.name)))
                .then_with(|| a.key.cmp(&b.key))
        });
        Ok(ranked
            .into_iter()
            .take(limit.clamp(1, 50))
            .map(|(_, s)| s)
            .collect())
    }
}

/// Name and first email or phone, for matching a card without a `UID`.
fn loose_key(c: &Contact) -> Option<String> {
    let reach: String = match (c.emails.first(), c.phones.first()) {
        (Some(e), _) => e.value.to_lowercase(),
        (None, Some(p)) => p.value.chars().filter(char::is_ascii_digit).collect(),
        (None, None) => String::new(),
    };
    let name = folded(&c.name);
    (!name.is_empty()).then(|| format!("{name}\n{reach}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::{
        Asked, Config, Homeserver, LoginGate, ThreadRow, ThreadSource, ThreadsQuery,
    };
    use std::sync::Arc;

    /// Enough of a homeserver for Contacts: accounts, profiles, account
    /// data, rooms and their state, each call checked to come as the
    /// application service on someone's behalf.
    #[derive(Default)]
    struct Fake {
        s: Mutex<FakeState>,
    }

    #[derive(Default)]
    struct FakeState {
        account_data: HashMap<(String, String), Value>,
        /// room -> (creator, create content, state key -> content)
        rooms: HashMap<String, (String, Value, HashMap<String, Value>)>,
        left: HashSet<String>,
        next: u32,
        calls: Vec<String>,
    }

    fn decode(s: &str) -> String {
        let mut out = Vec::new();
        let b = s.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'%' && i + 3 <= b.len() {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
                i += 3;
            } else {
                out.push(b[i]);
                i += 1;
            }
        }
        String::from_utf8(out).unwrap()
    }

    impl Homeserver for Arc<Fake> {
        fn call(
            &self,
            method: &str,
            path: &str,
            token: &str,
            body: Option<&Value>,
        ) -> std::result::Result<Reply, ChatError> {
            assert_eq!(token, "as-token");
            let mut s = self.s.lock().unwrap();
            s.calls.push(format!("{method} {path}"));
            let ok = |body: Value| {
                Ok(Reply {
                    status: 200,
                    headers: vec![],
                    body,
                })
            };
            let no = |status: u16, code: &str| {
                Ok(Reply {
                    status,
                    headers: vec![],
                    body: json!({ "errcode": code }),
                })
            };
            let (route, query) = path.split_once('?').unwrap_or((path, ""));
            let who = query
                .split('&')
                .find_map(|kv| kv.strip_prefix("user_id="))
                .map(decode)
                .unwrap_or_default();
            let parts: Vec<String> = route.split('/').map(decode).collect();
            let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
            match (method, &parts[..]) {
                ("POST", ["", "_matrix", "client", "v3", "register"]) => ok(json!({})),
                (_, ["", "_matrix", "client", "v3", "profile", _, "displayname"]) => {
                    ok(json!({ "displayname": "x" }))
                }
                (m, ["", "_matrix", "client", "v3", "user", user, "account_data", kind]) => {
                    assert_eq!(*user, who, "account data of someone else");
                    let key = (user.to_string(), kind.to_string());
                    if m == "PUT" {
                        s.account_data.insert(key, body.cloned().unwrap());
                        ok(json!({}))
                    } else {
                        match s.account_data.get(&key) {
                            Some(v) => ok(v.clone()),
                            None => no(404, "M_NOT_FOUND"),
                        }
                    }
                }
                ("POST", ["", "_matrix", "client", "v3", "createRoom"]) => {
                    s.next += 1;
                    let id = format!("!r{}:box", s.next);
                    let create = body.unwrap()["creation_content"].clone();
                    s.rooms
                        .insert(id.clone(), (who.clone(), create, HashMap::new()));
                    ok(json!({ "room_id": id }))
                }
                (m, ["", "_matrix", "client", "v3", "rooms", room, rest @ ..]) => {
                    let room = room.to_string();
                    let member =
                        s.rooms.get(&room).is_some_and(|r| r.0 == who) && !s.left.contains(&room);
                    if !member {
                        return no(403, "M_FORBIDDEN");
                    }
                    let state = &mut s.rooms.get_mut(&room).unwrap().2;
                    match (m, rest) {
                        ("GET", ["state"]) => ok(Value::Array(
                            state
                                .iter()
                                .map(
                                    |(k, v)| json!({ "type": EVENT, "state_key": k, "content": v }),
                                )
                                .collect(),
                        )),
                        ("GET", ["state", "m.room.create", ""]) => ok(json!({})),
                        ("GET", ["state", EVENT, key]) => match state.get(*key) {
                            Some(v) => ok(v.clone()),
                            None => no(404, "M_NOT_FOUND"),
                        },
                        ("PUT", ["state", EVENT, key]) => {
                            state.insert(key.to_string(), body.cloned().unwrap());
                            ok(json!({ "event_id": "$e" }))
                        }
                        _ => panic!("unexpected {method} {path}"),
                    }
                }
                _ => panic!("unexpected {method} {path}"),
            }
        }
    }

    struct NoGate;
    impl LoginGate for NoGate {
        fn ask(&self, _: &Asked) -> std::result::Result<Option<(String, Vec<String>)>, ChatError> {
            Ok(None)
        }
    }
    struct NoThreads;
    impl ThreadSource for NoThreads {
        fn rows(&self, _: &str, _: &str, _: &ThreadsQuery) -> anyhow::Result<Vec<ThreadRow>> {
            Ok(vec![])
        }
    }

    fn chat(fake: &Arc<Fake>, dir: &std::path::Path) -> Chat {
        let token = dir.join("as");
        std::fs::write(&token, "as-token").unwrap();
        Chat::new(
            Config {
                homeserver: "127.0.0.1:1".into(),
                server_name: "box".into(),
                as_token_file: token,
                threads_socket: dir.join("t"),
                login: "127.0.0.1:2".into(),
                federation: false,
            },
            Box::new(fake.clone()),
            Box::new(NoGate),
            Box::new(NoThreads),
        )
    }

    fn person(name: &str) -> Person {
        Person {
            name: name.into(),
            display_name: String::new(),
            groups: vec![],
        }
    }

    fn named(name: &str) -> Contact {
        Contact {
            name: name.into(),
            ..Contact::default()
        }
    }

    fn picture(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(w, h, image::Rgb([200, 10, 10]));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn a_book_is_one_private_room_per_person_found_again_through_account_data() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Arc::new(Fake::default());
        let chat = chat(&fake, dir.path());
        let book = Contacts::default();
        let eva = person("eva");
        let saved = book
            .save(&chat, &eva, None, named("Ján"), Photo::Keep)
            .unwrap();
        assert!(valid_id(&saved.id));
        assert!(saved.contact.uid.starts_with("urn:uuid:"));
        {
            let s = fake.s.lock().unwrap();
            assert_eq!(s.rooms.len(), 1);
            let (owner, create, _) = s.rooms.values().next().unwrap();
            assert_eq!(owner, "@eva:box");
            assert_eq!(create, &json!({ "m.federate": false, "type": ROOM_TYPE }));
        }
        // A fresh lososd (nothing remembered) finds the same room.
        let again = Contacts::default();
        assert_eq!(again.list(&chat, &eva).unwrap().len(), 1);
        assert_eq!(fake.s.lock().unwrap().rooms.len(), 1);
        // Someone else gets a book of their own and cannot see Eva's.
        assert!(book.list(&chat, &person("jan")).unwrap().is_empty());
        assert!(matches!(
            book.get(&chat, &person("jan"), &saved.id),
            Err(ContactsError::NotFound)
        ));
        assert_eq!(fake.s.lock().unwrap().rooms.len(), 2);
    }

    #[test]
    fn save_edit_photo_and_remove() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Arc::new(Fake::default());
        let chat = chat(&fake, dir.path());
        let book = Contacts::default();
        let eva = person("eva");
        assert!(matches!(
            book.save(&chat, &eva, None, named("  "), Photo::Keep),
            Err(ContactsError::Bad(_))
        ));
        let a = book
            .save(
                &chat,
                &eva,
                None,
                named("Ján"),
                Photo::Set(picture(400, 300)),
            )
            .unwrap();
        let picture = book.photo(&chat, &eva, &a.id).unwrap();
        let back = image::load_from_memory(&picture).unwrap();
        assert_eq!((back.width(), back.height()), (PHOTO_SIDE, PHOTO_SIDE));
        assert!(a
            .photo
            .as_deref()
            .unwrap()
            .starts_with(&format!("/api/contacts/{}/photo?v=", a.id)));

        let mut edit = a.contact.clone();
        edit.matrix = "@jan:box".into();
        let b = book
            .save(&chat, &eva, Some(&a.id), edit, Photo::Keep)
            .unwrap();
        assert_eq!(b.contact.uid, a.contact.uid);
        assert!(b.photo.is_some(), "the photo stays when the page keeps it");
        let c = book
            .save(&chat, &eva, Some(&a.id), b.contact.clone(), Photo::Remove)
            .unwrap();
        assert!(c.photo.is_none());
        assert!(matches!(
            book.photo(&chat, &eva, &a.id),
            Err(ContactsError::NotFound)
        ));

        book.remove(&chat, &eva, &a.id).unwrap();
        assert!(book.list(&chat, &eva).unwrap().is_empty());
        assert!(matches!(
            book.remove(&chat, &eva, &a.id),
            Err(ContactsError::NotFound)
        ));
        assert!(matches!(
            book.get(&chat, &eva, "../x"),
            Err(ContactsError::NotFound)
        ));
    }

    #[test]
    fn importing_twice_updates_instead_of_doubling_and_export_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Arc::new(Fake::default());
        let chat = chat(&fake, dir.path());
        let book = Contacts::default();
        let eva = person("eva");
        let file = concat!(
            "BEGIN:VCARD\nVERSION:3.0\nFN:Ján Štefan\nEMAIL:jan@example.sk\nEND:VCARD\n",
            "BEGIN:VCARD\nVERSION:4.0\nFN:Mária\nUID:urn:uuid:m\nTEL:+421900\nEND:VCARD\n",
            "BEGIN:VCARD\nVERSION:4.0\nN:;;;;\nEND:VCARD\n",
            "BEGIN:VCARD\nVERSION:3.0\nFN:Broken photo\nPHOTO;ENCODING=b;TYPE=JPEG:AAAA\nEND:VCARD\n",
        );
        let first = book.import(&chat, &eva, file.as_bytes()).unwrap();
        assert_eq!(
            first,
            Report {
                added: 3,
                updated: 0,
                skipped: 1,
                photos_dropped: 1,
                left_out: 0
            }
        );
        let second = book.import(&chat, &eva, file.as_bytes()).unwrap();
        assert_eq!(second.added, 0);
        assert_eq!(second.updated, 3);
        assert_eq!(book.list(&chat, &eva).unwrap().len(), 3);

        let all = book.export(&chat, &eva, None, vcard::Version::V4).unwrap();
        let third = book.import(&chat, &eva, all.as_bytes()).unwrap();
        assert_eq!((third.added, third.updated), (0, 3));
        let names: Vec<String> = book
            .list(&chat, &eva)
            .unwrap()
            .into_iter()
            .map(|s| s.contact.name)
            .collect();
        assert_eq!(names, ["Broken photo", "Ján Štefan", "Mária"]);

        assert!(matches!(
            book.import(&chat, &eva, b"hello"),
            Err(ContactsError::Bad(_))
        ));
        let one = book.list(&chat, &eva).unwrap()[2].id.clone();
        let only = book
            .export(&chat, &eva, Some(&[one]), vcard::Version::V3)
            .unwrap();
        assert_eq!(only.matches("BEGIN:VCARD").count(), 1);
        assert!(only.contains("UID:urn:uuid:m"));
    }

    #[test]
    fn a_book_left_from_another_client_is_started_again() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Arc::new(Fake::default());
        let chat = chat(&fake, dir.path());
        let book = Contacts::default();
        let eva = person("eva");
        book.save(&chat, &eva, None, named("A"), Photo::Keep)
            .unwrap();
        let old = fake.s.lock().unwrap().rooms.keys().next().unwrap().clone();
        fake.s.lock().unwrap().left.insert(old);
        // Remembered room refuses: looked up again, found gone, made anew.
        assert!(book.list(&chat, &eva).unwrap().is_empty());
        assert_eq!(fake.s.lock().unwrap().rooms.len(), 2);
    }

    #[test]
    fn suggestions_rank_contacts_and_people_on_the_box() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Arc::new(Fake::default());
        let chat = chat(&fake, dir.path());
        let book = Contacts::default();
        let eva = person("eva");
        let mut jan = named("Ján Štefan");
        jan.matrix = "@jan:box".into();
        jan.emails = vec![vcard::Labelled {
            value: "jan@example.sk".into(),
            label: String::new(),
        }];
        book.save(&chat, &eva, None, jan, Photo::Keep).unwrap();
        let mut stefania = named("Stefánia");
        stefania.mesh = "stefi@kitchen.local".into();
        book.save(&chat, &eva, None, stefania, Photo::Keep).unwrap();
        let neighbours = vec![
            Neighbour {
                name: "eva".into(),
                matrix_id: "@eva:box".into(),
                ..Neighbour::default()
            },
            Neighbour {
                name: "jan".into(),
                display_name: "Jan".into(),
                matrix_id: "@jan:box".into(),
                email: Some("jan@box.home.arpa".into()),
                ..Neighbour::default()
            },
            Neighbour {
                name: "stano".into(),
                display_name: "Stano".into(),
                matrix_id: "@stano:box".into(),
                email: Some("stano@box.home.arpa".into()),
                picture: Some("/p".into()),
            },
        ];
        let names = |q: &str, want| -> Vec<String> {
            book.suggest(&chat, &eva, &neighbours, q, want, 10)
                .unwrap()
                .into_iter()
                .map(|s| s.name)
                .collect()
        };
        // "st": Stefánia and Stano start with it, Ján Štefan has a word that does.
        assert_eq!(names("st", Want::Any), ["Stano", "Stefánia", "Ján Štefan"]);
        assert_eq!(names("", Want::Mesh), ["Stefánia"]);
        assert_eq!(names("", Want::Email), ["Ján Štefan", "Stano"]);
        // Jan on the box is a contact already: shown once, as the contact.
        assert_eq!(names("jan", Want::Matrix), ["Ján Štefan"]);
        let s = &book
            .suggest(&chat, &eva, &neighbours, "stano", Want::Any, 10)
            .unwrap()[0];
        assert_eq!(s.source, "box");
        assert_eq!(s.key, "box:stano");
        assert_eq!(s.emails, ["stano@box.home.arpa"]);
    }

    #[test]
    fn photos_are_squared_and_scaled() {
        let out = scale_photo(&picture(30, 90)).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!((img.width(), img.height()), (PHOTO_SIDE, PHOTO_SIDE));
        assert!(scale_photo(b"not a picture").is_err());
    }
}
