//! LosOS Contacts' book: a person's address books in LosOS Cloud, read and
//! written as contacts for the Contacts page (`/contacts` in the admin UI,
//! `admin-ui/app/src/contacts/`), under `/cloud/api/contacts/…`.
//!
//! There is one store. The cards are the ones [`crate::cloud::pim`] keeps
//! and [`crate::cloud::caldav`] serves over CardDAV, so a phone or desktop
//! address book synced with the box and the Contacts page see the same
//! contacts, and the contacts LosOS Cloud took over from Nextcloud
//! ([`crate::cloud::import`]) are there from the start. Every address book
//! the person has is read; a contact added here goes into their first one
//! (`contacts`, which [`pim::ensure_defaults`] makes).
//!
//! A contact's ID is its card's row in `dav_object`, which stays the same
//! while the card is rewritten in place. An edit made here rewrites only
//! the lines the page has fields for ([`vcard::rewrite`]) and is refused
//! with 412 when the card changed since it was read, so it never undoes
//! what a phone synced in between. A photo is served scaled to
//! [`PHOTO_SIDE`] pixels; one set here is stored at that size.
//!
//! [`suggest`] is the book's half of the people pickers in Mail and Chat:
//! lososd asks it (`crate::contacts`) and adds the people on the box.

use crate::cloud::auth::Who;
use crate::cloud::pim::{self, Collection, Expect, Kind, Object, PutError};
use crate::cloud::store::{Store, StoreError};
use crate::cloud::Cloud;
use crate::vcard::{self, Contact, Version};
use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::json;
use std::collections::{HashMap, HashSet};

/// The side of a contact's photo, in pixels.
pub const PHOTO_SIDE: u32 = 160;
/// The most contacts one import takes.
pub const MAX_IMPORT: usize = 5000;
/// The largest vCard file an import reads.
pub const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;
/// The most contacts one person's books hold, together.
pub const MAX_CONTACTS: usize = 10_000;
/// The largest picture a photo upload takes.
pub const MAX_PHOTO_BYTES: usize = 8 * 1024 * 1024;
/// The largest JSON body a save sends.
const MAX_JSON: usize = 1 << 20;

/// Why Contacts could not do what was asked.
#[derive(Debug, thiserror::Error)]
pub enum ContactsError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("there is no such contact")]
    NotFound,
    #[error("the contact changed since it was opened; open it again")]
    Changed,
    #[error("{0}")]
    Bad(String),
}

impl ContactsError {
    pub fn status(&self) -> StatusCode {
        match self {
            ContactsError::Store(StoreError::NotFound) | ContactsError::NotFound => {
                StatusCode::NOT_FOUND
            }
            ContactsError::Store(StoreError::Precondition) | ContactsError::Changed => {
                StatusCode::PRECONDITION_FAILED
            }
            ContactsError::Store(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ContactsError::Bad(_) => StatusCode::BAD_REQUEST,
        }
    }
}

impl From<PutError> for ContactsError {
    fn from(e: PutError) -> Self {
        match e {
            PutError::Store(StoreError::Precondition) => ContactsError::Changed,
            PutError::Store(e) => ContactsError::Store(e),
            PutError::Invalid | PutError::Unsupported => {
                ContactsError::Bad("the card could not be written".into())
            }
            PutError::UidConflict(_) => {
                ContactsError::Bad("another card in this address book has the same UID".into())
            }
        }
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
    /// The address book it is in, by its name.
    pub book: String,
    /// `/cloud/api/contacts/<id>/photo?v=<etag>` when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub photo: Option<String>,
    /// When it last changed, in milliseconds since the epoch.
    pub updated: i64,
    #[serde(skip)]
    pub etag: String,
    #[serde(skip)]
    card: String,
    #[serde(skip)]
    collection: i64,
    #[serde(skip)]
    uri: String,
}

/// What a save does with the photo.
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
    /// Cards past [`MAX_IMPORT`] or the books' [`MAX_CONTACTS`].
    pub left_out: usize,
}

/// Which people a picker wants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Want {
    Any,
    Email,
    Matrix,
    Mesh,
    /// The people on this box alone, every one of them, contact or not:
    /// the book's "On this box" view.
    Box,
}

impl Want {
    pub fn parse(s: &str) -> Want {
        match s {
            "email" => Want::Email,
            "matrix" => Want::Matrix,
            "mesh" => Want::Mesh,
            "box" => Want::Box,
            _ => Want::Any,
        }
    }
}

/// One person a picker offers: a contact, or someone on this box.
#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub key: String,
    pub source: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    #[serde(default)]
    pub emails: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matrix_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh_address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_id: Option<String>,
}

/// The book's matches for a picker, ranked ([`rank`]), with every Matrix ID
/// in the book so the caller can leave out box people already there.
#[derive(Debug, Clone, Default, Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Matches {
    pub matches: Vec<Ranked>,
    pub known_matrix_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct Ranked {
    pub rank: u8,
    #[serde(flatten)]
    pub suggestion: Suggestion,
}

/// A contact ID as the page may name one: a row number.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 18 && id.bytes().all(|b| b.is_ascii_digit())
}

/// A new card's name and `UID`, from 16 random bytes.
fn new_name() -> Result<(String, String)> {
    use ring::rand::SecureRandom;
    let mut b = [0u8; 16];
    ring::rand::SystemRandom::new()
        .fill(&mut b)
        .map_err(|_| ContactsError::Bad("no randomness for a new card".into()))?;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    let uuid = format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    );
    Ok((format!("{uuid}.vcf"), format!("urn:uuid:{uuid}")))
}

/// The photo as the book serves and stores it: square, [`PHOTO_SIDE`] on a
/// side, JPEG, turned upright, with nothing of the original file left.
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

/// The contact in a stored card, with its photo's bytes as the card has
/// them. `None` for a card with nothing to call the contact by.
fn read_card(text: &str) -> Option<vcard::Card> {
    vcard::parse(text.as_bytes()).cards.into_iter().next()
}

fn stored(c: &Collection, o: &Object) -> Option<Stored> {
    let card = read_card(&o.data)?;
    let id = o.id.to_string();
    let v = o.etag.trim_matches('"');
    Some(Stored {
        photo: card
            .photo
            .as_ref()
            .map(|_| format!("/cloud/api/contacts/{id}/photo?v={v}")),
        id,
        contact: card.contact,
        book: c.name.clone(),
        updated: o.modified.saturating_mul(1000),
        etag: o.etag.clone(),
        card: o.data.clone(),
        collection: c.id,
        uri: o.uri.clone(),
    })
}

/// Lower case, with the accents of Latin letters taken off, so "stefan"
/// finds "Štefan".
pub fn folded(s: &str) -> String {
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

/// How well `q` (already [`folded`]) matches `fields`: 0 for the start of
/// the first field (the name), 1 for the start of a word or of another
/// field, 2 for anywhere, `None` for no match.
pub fn rank(q: &str, fields: &[&str]) -> Option<u8> {
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

/// Every contact in the person's address books, by name.
pub fn list(store: &Store, login: &str) -> Result<Vec<Stored>> {
    pim::ensure_defaults(store, login)?;
    let mut out = Vec::new();
    for c in pim::collections(store, login, Kind::AddressBook)? {
        for o in pim::objects(store, &c)? {
            if let Some(s) = stored(&c, &o) {
                out.push(s);
            }
        }
    }
    out.sort_by(|a, b| {
        folded(&a.contact.name)
            .cmp(&folded(&b.contact.name))
            .then(a.id.len().cmp(&b.id.len()))
            .then(a.id.cmp(&b.id))
    });
    Ok(out)
}

/// The address book a new contact goes into: `contacts` when the person
/// has it, else their first.
fn home_book(store: &Store, login: &str) -> Result<Collection> {
    pim::ensure_defaults(store, login)?;
    let books = pim::collections(store, login, Kind::AddressBook)?;
    books
        .iter()
        .find(|c| c.uri == "contacts")
        .or(books.first())
        .cloned()
        .ok_or(ContactsError::NotFound)
}

/// One contact, by ID, from the person's own address books only.
pub fn get(store: &Store, login: &str, id: &str) -> Result<Stored> {
    if !valid_id(id) {
        return Err(ContactsError::NotFound);
    }
    let row: i64 = id.parse().map_err(|_| ContactsError::NotFound)?;
    let found: Option<(String, String)> = store.with_db(|db| {
        Ok(db
            .query_row(
                "SELECT c.uri, o.uri FROM dav_object o JOIN dav_collection c ON o.collection = c.id
                 WHERE o.id = ?1 AND c.owner = ?2 AND c.kind = 'addressbook'",
                params![row, login],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    })?;
    let (book, uri) = found.ok_or(ContactsError::NotFound)?;
    let c = pim::collection(store, login, Kind::AddressBook, &book)?;
    let o = pim::object(store, &c, &uri)?;
    stored(&c, &o).ok_or(ContactsError::NotFound)
}

/// A contact's photo, JPEG, [`PHOTO_SIDE`] on a side.
pub fn photo(store: &Store, login: &str, id: &str) -> Result<Vec<u8>> {
    let s = get(store, login, id)?;
    let bytes = read_card(&s.card)
        .and_then(|c| c.photo)
        .ok_or(ContactsError::NotFound)?;
    scale_photo(&bytes)
}

fn collection_of(store: &Store, login: &str, s: &Stored) -> Result<Collection> {
    pim::collections(store, login, Kind::AddressBook)?
        .into_iter()
        .find(|c| c.id == s.collection)
        .ok_or(ContactsError::NotFound)
}

/// Save a contact: a new one when `id` is `None`. The fields are tidied
/// the way an import tidies them. An existing card keeps its other lines
/// and its address book, and is written only if it is still the version
/// that was read.
pub fn save(
    store: &Store,
    login: &str,
    id: Option<&str>,
    mut contact: Contact,
    photo: Photo,
) -> Result<Stored> {
    if !contact.tidy() {
        return Err(ContactsError::Bad("give the contact a name".into()));
    }
    let now_ms = crate::cloud::util::now().saturating_mul(1000);
    let jpeg = match &photo {
        Photo::Set(bytes) => Some(scale_photo(bytes)?),
        _ => None,
    };
    match id {
        Some(id) => {
            let old = get(store, login, id)?;
            if contact.uid.is_empty() {
                contact.uid = old.contact.uid.clone();
            }
            if contact.uid.is_empty() {
                contact.uid = new_name()?.1;
            }
            let out = vcard::Out {
                contact: &contact,
                uid: &contact.uid,
                updated_ms: now_ms,
                photo: jpeg.as_deref(),
            };
            let text = vcard::rewrite(
                &old.card,
                &out,
                vcard::version_of(&old.card),
                matches!(photo, Photo::Keep),
            );
            let c = collection_of(store, login, &old)?;
            pim::put(store, &c, &old.uri, &text, Expect::Etag(&old.etag))?;
            get(store, login, id)
        }
        None => {
            if list(store, login)?.len() >= MAX_CONTACTS {
                return Err(ContactsError::Bad(format!(
                    "the address books hold at most {MAX_CONTACTS} contacts"
                )));
            }
            let (uri, uid) = new_name()?;
            if contact.uid.is_empty() {
                contact.uid = uid;
            }
            let c = home_book(store, login)?;
            let text = vcard::write(
                &[vcard::Out {
                    contact: &contact,
                    uid: &contact.uid,
                    updated_ms: now_ms,
                    photo: jpeg.as_deref(),
                }],
                Version::V3,
            );
            pim::put(store, &c, &uri, &text, Expect::New)?;
            let o = pim::object(store, &c, &uri)?;
            stored(&c, &o).ok_or(ContactsError::NotFound)
        }
    }
}

/// Remove a contact.
pub fn remove(store: &Store, login: &str, id: &str) -> Result<()> {
    let old = get(store, login, id)?;
    let c = collection_of(store, login, &old)?;
    pim::delete_object(store, &c, &old.uri, None)?;
    Ok(())
}

/// Take the cards of a vCard file into the books. A card whose `UID` is
/// already in a book, or, without one, whose name and first email or phone
/// match a contact's, replaces that contact where it is; the rest go into
/// the first address book. Cards are written as vCard 3.0 (or 4.0 over a
/// 4.0 card), whatever version the file was.
pub fn import(store: &Store, login: &str, bytes: &[u8]) -> Result<Report> {
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
    let book = list(store, login)?;
    let home = home_book(store, login)?;
    let books: HashMap<i64, Collection> = pim::collections(store, login, Kind::AddressBook)?
        .into_iter()
        .map(|c| (c.id, c))
        .collect();
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
    let now_ms = crate::cloud::util::now().saturating_mul(1000);
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
        let old = old.filter(|o| !done.contains(&o.id));
        if old.is_none() && count >= MAX_CONTACTS {
            report.left_out += 1;
            continue;
        }
        let (scaled, dropped) = match card.photo {
            Some(bytes) => match scale_photo(&bytes) {
                Ok(j) => (Some(j), false),
                Err(_) => (None, true),
            },
            None => (None, false),
        };
        if dropped {
            report.photos_dropped += 1;
        }
        match old {
            Some(o) => {
                if contact.uid.is_empty() {
                    contact.uid = o.contact.uid.clone();
                }
                if contact.uid.is_empty() {
                    contact.uid = new_name()?.1;
                }
                let out = vcard::Out {
                    contact: &contact,
                    uid: &contact.uid,
                    updated_ms: now_ms,
                    photo: scaled.as_deref(),
                };
                // A card without a photo keeps the one the contact had.
                let text =
                    vcard::rewrite(&o.card, &out, vcard::version_of(&o.card), scaled.is_none());
                let c = books.get(&o.collection).ok_or(ContactsError::NotFound)?;
                pim::put(store, c, &o.uri, &text, Expect::Etag(&o.etag))?;
                done.insert(o.id);
                report.updated += 1;
            }
            None => {
                let (uri, uid) = new_name()?;
                if contact.uid.is_empty() {
                    contact.uid = uid;
                }
                let text = vcard::write(
                    &[vcard::Out {
                        contact: &contact,
                        uid: &contact.uid,
                        updated_ms: now_ms,
                        photo: scaled.as_deref(),
                    }],
                    Version::V3,
                );
                pim::put(store, &home, &uri, &text, Expect::New)?;
                count += 1;
                report.added += 1;
            }
        }
    }
    Ok(report)
}

/// The person's contacts as one vCard file: `ids` only, or all.
pub fn export(
    store: &Store,
    login: &str,
    ids: Option<&[String]>,
    version: Version,
) -> Result<String> {
    let all = list(store, login)?;
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
    let photos: Vec<Option<Vec<u8>>> = picked
        .iter()
        .map(|s| {
            read_card(&s.card)
                .and_then(|c| c.photo)
                .and_then(|p| scale_photo(&p).ok())
        })
        .collect();
    let out: Vec<vcard::Out<'_>> = picked
        .iter()
        .zip(&photos)
        .map(|(s, p)| vcard::Out {
            contact: &s.contact,
            uid: &s.contact.uid,
            updated_ms: s.updated,
            photo: p.as_deref(),
        })
        .collect();
    Ok(vcard::write(&out, version))
}

/// The book's matches for `q`, for a picker that wants `want`.
pub fn suggest(store: &Store, login: &str, q: &str, want: Want) -> Result<Matches> {
    let q = folded(q.trim());
    let mut out = Matches::default();
    if want == Want::Box {
        return Ok(out);
    }
    for s in list(store, login)? {
        let c = &s.contact;
        if !c.matrix.is_empty() {
            out.known_matrix_ids.push(c.matrix.clone());
        }
        let fits = match want {
            Want::Any => true,
            Want::Box => false,
            Want::Email => !c.emails.is_empty(),
            Want::Matrix => !c.matrix.is_empty(),
            Want::Mesh => !c.mesh.is_empty(),
        };
        if !fits {
            continue;
        }
        let mut fields = vec![c.name.as_str(), c.nickname.as_str(), c.org.as_str()];
        fields.extend(c.emails.iter().map(|e| e.value.as_str()));
        fields.extend(c.phones.iter().map(|p| p.value.as_str()));
        fields.push(c.matrix.as_str());
        fields.push(c.mesh.as_str());
        let Some(rank) = rank(&q, &fields) else {
            continue;
        };
        out.matches.push(Ranked {
            rank,
            suggestion: Suggestion {
                key: format!("contact:{}", s.id),
                source: "contact".into(),
                name: c.name.clone(),
                picture: s.photo.clone(),
                emails: c.emails.iter().map(|e| e.value.clone()).collect(),
                matrix_id: (!c.matrix.is_empty()).then(|| c.matrix.clone()),
                mesh_address: (!c.mesh.is_empty()).then(|| c.mesh.clone()),
                contact_id: Some(s.id.clone()),
            },
        });
    }
    Ok(out)
}

// ── /cloud/api/contacts/… ──────────────────────────────────────────────────

fn refused(e: &ContactsError) -> HttpResponse {
    let status = e.status();
    if status.is_server_error() {
        tracing::warn!(error = %e, "contacts");
    }
    HttpResponse::build(status).json(json!({ "error": e.to_string() }))
}

fn answer<T: Serialize>(r: Result<T>) -> HttpResponse {
    match r {
        Ok(v) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(v),
        Err(e) => refused(&e),
    }
}

fn query(req: &HttpRequest) -> HashMap<String, String> {
    url::form_urlencoded::parse(req.query_string().as_bytes())
        .into_owned()
        .collect()
}

async fn bytes(body: web::Payload, max: usize) -> std::result::Result<Vec<u8>, HttpResponse> {
    crate::cloud::dav::read_body(body, max).await
}

fn contact_body(body: &[u8]) -> Result<Contact> {
    serde_json::from_slice(body).map_err(|_| {
        ContactsError::Bad("the body must be a contact as JSON: {\"name\": \"...\", ...}".into())
    })
}

/// One call under `/cloud/api/contacts`. `rest` is what follows it.
///
/// ```text
/// GET    /contacts                      {"contacts": [...]}
/// POST   /contacts                      a new contact, JSON
/// GET    /contacts/<id>                 one
/// PUT    /contacts/<id>                 its fields, replaced; the photo stays
/// DELETE /contacts/<id>
/// GET    /contacts/<id>/photo?v=        JPEG
/// PUT    /contacts/<id>/photo           a picture as the body
/// DELETE /contacts/<id>/photo
/// POST   /contacts/import               a .vcf as the body: a Report
/// GET    /contacts/export?version=&ids= a .vcf
/// GET    /contacts/suggest?q=&want=     Matches, for lososd's pickers
/// ```
pub async fn api(
    req: &HttpRequest,
    cloud: &Cloud,
    who: &Who,
    rest: &[&str],
    body: web::Payload,
) -> HttpResponse {
    let store = &cloud.store;
    let login = who.login.as_str();
    match (req.method().as_str(), rest) {
        ("GET", []) => answer(list(store, login).map(|l| json!({ "contacts": l }))),
        ("POST", []) => match bytes(body, MAX_JSON).await {
            Ok(raw) => {
                answer(contact_body(&raw).and_then(|c| save(store, login, None, c, Photo::Keep)))
            }
            Err(r) => r,
        },
        ("POST", ["import"]) => match bytes(body, MAX_FILE_BYTES).await {
            Ok(raw) => answer(import(store, login, &raw)),
            Err(r) => r,
        },
        ("GET", ["export"]) => {
            let q = query(req);
            let version = match q.get("version").map(String::as_str) {
                Some("4.0" | "4") => Version::V4,
                _ => Version::V3,
            };
            let ids: Option<Vec<String>> = q
                .get("ids")
                .filter(|s| !s.is_empty())
                .map(|s| s.split(',').map(str::to_string).collect());
            match export(store, login, ids.as_deref(), version) {
                Ok(file) => HttpResponse::Ok()
                    .content_type("text/vcard; charset=utf-8")
                    .insert_header((
                        "Content-Disposition",
                        "attachment; filename=\"contacts.vcf\"",
                    ))
                    .insert_header(("Cache-Control", "no-store"))
                    .body(file),
                Err(e) => refused(&e),
            }
        }
        ("GET", ["suggest"]) => {
            let q = query(req);
            let text = q.get("q").map(String::as_str).unwrap_or("");
            let want = Want::parse(q.get("want").map(String::as_str).unwrap_or(""));
            answer(suggest(store, login, text, want))
        }
        ("GET", [id]) => answer(get(store, login, id)),
        ("PUT", [id]) => match bytes(body, MAX_JSON).await {
            Ok(raw) => answer(
                contact_body(&raw).and_then(|c| save(store, login, Some(id), c, Photo::Keep)),
            ),
            Err(r) => r,
        },
        ("DELETE", [id]) => answer(remove(store, login, id).map(|()| json!({ "removed": id }))),
        ("GET", [id, "photo"]) => match photo(store, login, id) {
            Ok(jpeg) => HttpResponse::Ok()
                .content_type("image/jpeg")
                .insert_header(("Cache-Control", "private, max-age=31536000, immutable"))
                .insert_header(("X-Content-Type-Options", "nosniff"))
                .body(jpeg),
            Err(e) => refused(&e),
        },
        ("PUT", [id, "photo"]) => match bytes(body, MAX_PHOTO_BYTES).await {
            Ok(raw) => answer(
                get(store, login, id)
                    .and_then(|old| save(store, login, Some(id), old.contact, Photo::Set(raw))),
            ),
            Err(r) => r,
        },
        ("DELETE", [id, "photo"]) => answer(
            get(store, login, id)
                .and_then(|old| save(store, login, Some(id), old.contact, Photo::Remove)),
        ),
        _ => crate::cloud::api::error(StatusCode::NOT_FOUND, "notFound", "no such call"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        pim::init(&store).unwrap();
        (dir, store)
    }

    fn contact(name: &str, email: &str) -> Contact {
        Contact {
            name: name.into(),
            emails: vec![vcard::Labelled {
                value: email.into(),
                label: String::new(),
            }],
            ..Contact::default()
        }
    }

    fn png() -> Vec<u8> {
        let img = image::RgbImage::from_pixel(300, 200, image::Rgb([200, 80, 40]));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn a_contact_made_here_is_a_card_in_the_first_address_book() {
        let (_d, s) = store();
        let saved = save(
            &s,
            "eva",
            None,
            contact("Jana Kováčová", "jana@example.org"),
            Photo::Keep,
        )
        .unwrap();
        let book = pim::collection(&s, "eva", Kind::AddressBook, "contacts").unwrap();
        let objects = pim::objects(&s, &book).unwrap();
        assert_eq!(objects.len(), 1);
        assert!(objects[0].uri.ends_with(".vcf"));
        assert!(objects[0].data.contains("FN:Jana Kováčová\r\n"));
        assert!(objects[0].data.contains("VERSION:3.0\r\n"));
        assert_eq!(saved.id, objects[0].id.to_string());
        assert_eq!(saved.book, "Contacts");
        assert!(saved.contact.uid.starts_with("urn:uuid:"));
        // Someone else asking for it by ID finds nothing.
        assert!(matches!(
            get(&s, "matus", &saved.id),
            Err(ContactsError::NotFound)
        ));
    }

    #[test]
    fn an_edit_keeps_what_a_phone_wrote_and_its_book() {
        let (_d, s) = store();
        let other = pim::create(
            &s,
            "eva",
            Kind::AddressBook,
            "family",
            &pim::CollectionFields {
                name: Some("Family".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let card = concat!(
            "BEGIN:VCARD\r\nVERSION:4.0\r\nUID:phone-1\r\nFN:Tomáš Bielik\r\n",
            "item1.EMAIL:tomas@example.org\r\nitem1.X-ABLabel:Club\r\n",
            "CATEGORIES:Cycling\r\nX-PHONETIC-FIRST-NAME:Tomasch\r\n",
            "IMPP:xmpp:tomas@example.org\r\nEND:VCARD\r\n"
        );
        pim::put(&s, &other, "phone-1.vcf", card, Expect::New).unwrap();
        let listed = list(&s, "eva").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].book, "Family");
        let mut c = listed[0].contact.clone();
        c.title = "Captain".into();
        save(&s, "eva", Some(&listed[0].id), c, Photo::Keep).unwrap();
        let after = pim::object(&s, &other, "phone-1.vcf").unwrap().data;
        assert!(after.contains("VERSION:4.0\r\n"), "{after}");
        assert!(after.contains("TITLE:Captain\r\n"), "{after}");
        assert!(after.contains("CATEGORIES:Cycling\r\n"), "{after}");
        assert!(
            after.contains("X-PHONETIC-FIRST-NAME:Tomasch\r\n"),
            "{after}"
        );
        assert!(after.contains("IMPP:xmpp:tomas@example.org\r\n"), "{after}");
        assert!(after.contains("UID:phone-1\r\n"), "{after}");
        assert!(!after.contains("X-ABLabel"), "{after}");
        assert_eq!(after.matches("EMAIL").count(), 1, "{after}");
        assert_eq!(after.matches("BEGIN:VCARD").count(), 1, "{after}");
    }

    #[test]
    fn an_edit_over_a_card_that_changed_meanwhile_is_refused() {
        let (_d, s) = store();
        let saved = save(
            &s,
            "eva",
            None,
            contact("Ondrej", "o@example.org"),
            Photo::Keep,
        )
        .unwrap();
        let first = get(&s, "eva", &saved.id).unwrap();
        // The phone syncs a change in between.
        let book = pim::collection(&s, "eva", Kind::AddressBook, "contacts").unwrap();
        let changed = first.card.replace("FN:Ondrej", "FN:Ondrej Rak");
        pim::put(&s, &book, &first.uri, &changed, Expect::Any).unwrap();
        let c = first.contact.clone();
        let r = (|| -> Result<()> {
            let text = vcard::rewrite(
                &first.card,
                &vcard::Out {
                    contact: &c,
                    uid: &c.uid,
                    updated_ms: 0,
                    photo: None,
                },
                Version::V3,
                true,
            );
            pim::put(&s, &book, &first.uri, &text, Expect::Etag(&first.etag))?;
            Ok(())
        })();
        assert!(matches!(r, Err(ContactsError::Changed)));
    }

    #[test]
    fn photos_are_served_small_and_kept_through_an_edit() {
        let (_d, s) = store();
        let saved = save(
            &s,
            "eva",
            None,
            contact("Eva", "eva@example.org"),
            Photo::Keep,
        )
        .unwrap();
        assert!(saved.photo.is_none());
        let with = save(
            &s,
            "eva",
            Some(&saved.id),
            saved.contact.clone(),
            Photo::Set(png()),
        )
        .unwrap();
        assert!(with
            .photo
            .as_deref()
            .unwrap()
            .starts_with("/cloud/api/contacts/"));
        let jpeg = photo(&s, "eva", &saved.id).unwrap();
        assert_eq!(&jpeg[..3], b"\xff\xd8\xff");
        let img = image::load_from_memory(&jpeg).unwrap();
        assert_eq!((img.width(), img.height()), (PHOTO_SIDE, PHOTO_SIDE));
        let mut c = with.contact.clone();
        c.note = "Bread on Fridays".into();
        let kept = save(&s, "eva", Some(&saved.id), c, Photo::Keep).unwrap();
        assert!(kept.photo.is_some());
        let gone = save(&s, "eva", Some(&saved.id), kept.contact, Photo::Remove).unwrap();
        assert!(gone.photo.is_none());
        assert!(matches!(
            photo(&s, "eva", &saved.id),
            Err(ContactsError::NotFound)
        ));
    }

    #[test]
    fn an_import_updates_by_uid_and_by_name_and_address_across_books() {
        let (_d, s) = store();
        let family = pim::create(
            &s,
            "eva",
            Kind::AddressBook,
            "family",
            &pim::CollectionFields::default(),
        )
        .unwrap();
        pim::put(
            &s,
            &family,
            "a.vcf",
            "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:a-1\r\nFN:Jana\r\nCATEGORIES:Kin\r\nEND:VCARD\r\n",
            Expect::New,
        )
        .unwrap();
        save(
            &s,
            "eva",
            None,
            contact("Peter Dlhý", "peter@example.org"),
            Photo::Keep,
        )
        .unwrap();
        let file = concat!(
            "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:a-1\r\nFN:Jana Kováčová\r\nEND:VCARD\r\n",
            "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Peter Dlhý\r\nEMAIL:peter@example.org\r\nTITLE:Baker\r\nEND:VCARD\r\n",
            "BEGIN:VCARD\r\nVERSION:2.1\r\nN;CHARSET=UTF-8;ENCODING=QUOTED-PRINTABLE:Rak;Ondrej\r\nTEL;CELL:+421 900 1\r\nEND:VCARD\r\n",
            "BEGIN:VCARD\r\nVERSION:3.0\r\nEND:VCARD\r\n",
        );
        let r = import(&s, "eva", file.as_bytes()).unwrap();
        assert_eq!(
            r,
            Report {
                added: 1,
                updated: 2,
                skipped: 1,
                ..Report::default()
            }
        );
        let again = import(&s, "eva", file.as_bytes()).unwrap();
        assert_eq!((again.added, again.updated), (0, 3));
        let all = list(&s, "eva").unwrap();
        let names: Vec<&str> = all.iter().map(|x| x.contact.name.as_str()).collect();
        assert_eq!(names, ["Jana Kováčová", "Ondrej Rak", "Peter Dlhý"]);
        // Jana stayed in her book with her category; Peter got his title.
        let jana = pim::object(&s, &family, "a.vcf").unwrap().data;
        assert!(jana.contains("FN:Jana Kováčová\r\n") && jana.contains("CATEGORIES:Kin\r\n"));
        assert_eq!(all[2].contact.title, "Baker");
        // The 2.1 card is stored as 3.0.
        let ondrej = get(&s, "eva", &all[1].id).unwrap();
        assert!(ondrej.card.contains("VERSION:3.0\r\n"));
        assert!(!ondrej.card.contains("QUOTED-PRINTABLE"));
    }

    #[test]
    fn export_writes_the_picked_cards_in_the_version_asked() {
        let (_d, s) = store();
        let a = save(
            &s,
            "eva",
            None,
            contact("Alena", "a@example.org"),
            Photo::Keep,
        )
        .unwrap();
        save(
            &s,
            "eva",
            None,
            contact("Boris", "b@example.org"),
            Photo::Keep,
        )
        .unwrap();
        let all = export(&s, "eva", None, Version::V4).unwrap();
        assert_eq!(all.matches("BEGIN:VCARD").count(), 2);
        assert!(all.contains("VERSION:4.0"));
        let one = export(&s, "eva", Some(std::slice::from_ref(&a.id)), Version::V3).unwrap();
        assert_eq!(one.matches("BEGIN:VCARD").count(), 1);
        assert!(one.contains("FN:Alena"));
        assert!(matches!(
            export(&s, "eva", Some(&["999".into()]), Version::V3),
            Err(ContactsError::NotFound)
        ));
    }

    #[test]
    fn suggest_ranks_folds_accents_and_names_the_matrix_ids_it_knows() {
        let (_d, s) = store();
        let mut eva = contact("Eva Nováková", "eva@example.org");
        eva.matrix = "@eva:box.local".into();
        save(&s, "eva", None, eva, Photo::Keep).unwrap();
        save(
            &s,
            "eva",
            None,
            contact("Jana Kováčová", "jana@example.org"),
            Photo::Keep,
        )
        .unwrap();
        let names = |q: &str, w: Want| -> Vec<String> {
            let mut m = suggest(&s, "eva", q, w).unwrap().matches;
            m.sort_by_key(|r| r.rank);
            m.into_iter().map(|r| r.suggestion.name).collect()
        };
        assert_eq!(names("kov", Want::Any), ["Jana Kováčová", "Eva Nováková"]);
        assert_eq!(names("", Want::Matrix), ["Eva Nováková"]);
        assert!(names("", Want::Box).is_empty());
        let m = suggest(&s, "eva", "", Want::Any).unwrap();
        assert_eq!(m.known_matrix_ids, ["@eva:box.local"]);
    }
}
