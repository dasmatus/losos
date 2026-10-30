//! The look: each person's background picture, and the widgets the owner
//! wrote by hand.
//!
//! Both are **appliance** state, not browser state, and that is the one
//! decision this plugin exists to carry. The widget board itself stays in the
//! browser (`admin-ui/app/src/lib/widgets.ts` says why: nothing on a
//! dashboard may be able to start a rebuild), but a picture someone chose
//! and a widget the owner typed by hand are kept on the box, so a phone and a
//! laptop see the same page. Neither touches `overrides.nix` or starts a
//! rebuild: the documents live beside `state.json` and are written the same
//! way, atomically. What a widget may hold is `losos-widget`'s business; the
//! security argument for running owner-written script at all is in the
//! frame's own header (`admin-ui/app/public/widget-frame/index.html`).
//!
//! **A background belongs to a person** ([`Whose`]). Each person LosOS Login
//! signs in has their own picture and veil, in `looks/<name>.json` and
//! `looks/<name>.img`, and nobody else's choice changes their page. The
//! spare key is nobody, so it keeps the box's own background in `look.json`
//! and `background.img`, which is where every background lived before it
//! became a person's. The owner (the person in `losos-owner`) is handed a
//! copy of that one the first time they ask, so a box that had a
//! background keeps it for the person who chose it.
//!
//! A person's picture is read back by the browser with their LosOS Login
//! cookie (`GET /api/look/background/{name}`), and only by them. The box's
//! own is read back *without* a token (`GET /api/look/background`,
//! `unguarded` in the manifest): a spare-key tab holds its key in the page,
//! a CSS `background-image` cannot carry a Bearer header, and the
//! alternative, fetching bytes with the token and painting a `blob:` URL,
//! needs an `img-src blob:` grant the admin CSP deliberately does not make.
//! That route is behind the same LAN-only guard and LosOS Login check in
//! nginx as the page that shows it. Everything that *changes* a look is
//! checked by lososd and audited.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub use losos_widget::{
    is_file_name, parse_files, validate_files, Invalid, WidgetFile, ENTRY_FILE, FILE_KINDS,
    MAX_FILES, MAX_FILE_NAME_CHARS, MAX_WIDGET_BYTES,
};

/// How many widgets the box keeps. Each one is an iframe on the board; a
/// mini-PC serving photos has no business hosting a hundred of them.
pub const MAX_WIDGETS: usize = 24;
/// Longest widget name, in characters.
pub const MAX_NAME_CHARS: usize = 60;
/// Where the look is kept: [`crate::FileStore`] on the box, a map in the
/// tests.
///
/// A picture is the box's own (`None`) or a person's (`Some(name)`); the
/// name has passed [`is_person_name`] before it gets here.
pub trait Store {
    /// The box's document, or [`Look::default`] when there is none or it
    /// does not parse: a damaged file costs a wallpaper, not the page.
    fn load_look(&mut self) -> Look;
    fn save_look(&mut self, look: &Look) -> anyhow::Result<()>;
    /// A person's background, or `None` while they have never had one. A
    /// file that does not parse reads as [`Mine::default`].
    fn load_person(&mut self, name: &str) -> Option<Mine>;
    fn save_person(&mut self, name: &str, mine: &Mine) -> anyhow::Result<()>;
    fn write_picture(&mut self, name: Option<&str>, bytes: &[u8]) -> anyhow::Result<()>;
    fn read_picture(&mut self, name: Option<&str>) -> anyhow::Result<Option<Vec<u8>>>;
    fn remove_picture(&mut self, name: Option<&str>) -> anyhow::Result<()>;
}

/// Whose background a call reads and changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Whose {
    /// The box's own: the spare key's, and anyone's who is not a person.
    Box,
    /// A person's. `owner` is membership of `losos-owner`, which is who
    /// inherits the box's own background.
    Person { name: String, owner: bool },
}

/// The group the claim puts the first person in (`backend/src/people.rs`).
pub const OWNER_GROUP: &str = "losos-owner";

impl Whose {
    /// From the caller the host names. A person whose name is not one the
    /// people file could hold is refused rather than written as a path.
    pub fn of(principal: &losos_plugin::Principal) -> Result<Whose, Invalid> {
        match principal {
            losos_plugin::Principal::Person(p) => {
                if !is_person_name(&p.name) {
                    return Err(Invalid("that is not a sign-in name".to_string()));
                }
                Ok(Whose::Person {
                    name: p.name.clone(),
                    owner: p.groups.iter().any(|g| g == OWNER_GROUP),
                })
            }
            _ => Ok(Whose::Box),
        }
    }

    fn name(&self) -> Option<&str> {
        match self {
            Whose::Box => None,
            Whose::Person { name, .. } => Some(name),
        }
    }

    /// Whether this is the person called `name`.
    pub fn name_is(&self, name: &str) -> bool {
        self.name() == Some(name)
    }
}

/// A sign-in name as `backend/src/people.rs` keeps them: lower-case letters,
/// digits, `.`, `-` and `_`, starting with a letter. Checked again here
/// because it becomes a file name.
pub fn is_person_name(name: &str) -> bool {
    name.len() <= 64
        && name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_'))
}

/// Largest background picture accepted, in bytes.
pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
/// The veil over the picture — how much of the theme's ground colour sits on
/// top of it, in percent — may not be so thin that text becomes unreadable
/// nor so thick that the picture is pointless.
pub const MIN_VEIL: u8 = 20;
pub const MAX_VEIL: u8 = 90;
pub const DEFAULT_VEIL: u8 = 60;

/// Pictures that ship with the admin page, under `/backgrounds/<name>.svg`.
/// The names are the contract with `admin-ui/app/src/lib/look.ts`.
pub const SHIPPED: &[&str] = &["tide", "grid", "dusk"];

/// Where the page gets the box's own uploaded picture, and with `/<name>`
/// after it a person's; `?v=` makes a replacement a new URL so no cache
/// serves the old one, and the name keeps two people on one browser apart.
pub const BACKGROUND_ROUTE: &str = "/api/look/background";

/// What sits behind the admin page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Background {
    /// The theme's plain ground colour.
    None,
    /// One of [`SHIPPED`].
    Shipped { name: String },
    /// A picture the owner uploaded, kept beside the document.
    Upload {
        /// Bumped on every upload; the page puts it in the URL.
        version: u64,
        #[serde(rename = "contentType")]
        content_type: String,
    },
}

/// How wide a widget sits on the board: half a row, or the whole of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Span {
    Half,
    Full,
}

/// One widget the owner wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "StoredWidget")]
pub struct HandWidget {
    pub id: String,
    pub name: String,
    pub span: Span,
    /// `index.html`, rendered in the frame, and the files it links.
    pub files: Vec<WidgetFile>,
}

/// A widget as `look.json` may hold it: with files, or, written before a
/// widget could have more than one, a single `source` that is its
/// `index.html`.
#[derive(Deserialize)]
struct StoredWidget {
    id: String,
    name: String,
    span: Span,
    #[serde(default)]
    files: Vec<WidgetFile>,
    #[serde(default)]
    source: Option<String>,
}

impl From<StoredWidget> for HandWidget {
    fn from(w: StoredWidget) -> Self {
        let files = match (w.files.is_empty(), w.source) {
            (true, Some(source)) => vec![WidgetFile::entry(&source)],
            _ => w.files,
        };
        HandWidget {
            id: w.id,
            name: w.name,
            span: w.span,
            files,
        }
    }
}

/// The persisted document at `$LOSOS_STATE_DIR/look.json`: the widgets,
/// and the box's own background ([`Whose::Box`]).
///
/// Decoding is lenient the way `state.json` is: a document that does not
/// parse reads as [`Look::default`] ([`Store::load_look`]), and every field has a
/// default so a file from an older build still opens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Look {
    #[serde(default = "one")]
    pub version: u32,
    #[serde(default = "no_background")]
    pub background: Background,
    #[serde(default = "default_veil")]
    pub veil: u8,
    #[serde(default)]
    pub widgets: Vec<HandWidget>,
    /// Ids are never reused, so a tile on some browser's board that names a
    /// deleted widget can never silently pick up a different one.
    #[serde(default = "one_u64", rename = "nextId")]
    pub next_id: u64,
}

fn one() -> u32 {
    1
}
fn one_u64() -> u64 {
    1
}
fn no_background() -> Background {
    Background::None
}
fn default_veil() -> u8 {
    DEFAULT_VEIL
}

/// One person's background, at `$LOSOS_STATE_DIR/looks/<name>.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mine {
    #[serde(default = "no_background")]
    pub background: Background,
    #[serde(default = "default_veil")]
    pub veil: u8,
}

impl Default for Mine {
    fn default() -> Self {
        Mine {
            background: Background::None,
            veil: DEFAULT_VEIL,
        }
    }
}

impl Default for Look {
    fn default() -> Self {
        Look {
            version: 1,
            background: Background::None,
            veil: DEFAULT_VEIL,
            widgets: Vec::new(),
            next_id: 1,
        }
    }
}

fn invalid<T>(message: impl Into<String>) -> anyhow::Result<T> {
    Err(anyhow::Error::new(Invalid(message.into())))
}

// ── The picture ───────────────────────────────────────────────────────────

/// The image type by its first bytes, or `None` for anything else.
///
/// The client's `Content-Type` is not consulted: a browser labels whatever it
/// was handed, and the bytes are what the page will try to paint. SVG is
/// deliberately absent — an SVG is a document that may carry script, and a
/// picture should not be the one way to get a document onto this origin.
pub fn sniff_image(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// Where the page fetches an uploaded picture from, versioned: the box's
/// own, or a person's under their name.
pub fn background_url(name: Option<&str>, version: u64) -> String {
    match name {
        None => format!("{BACKGROUND_ROUTE}?v={version}"),
        Some(name) => format!("{BACKGROUND_ROUTE}/{name}?v={version}"),
    }
}

/// What the page asks for when it is not uploading: nothing, or a shipped
/// picture by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackgroundChoice {
    None,
    Shipped(String),
}

/// A change to the look that is not a picture upload. Absent fields are left
/// as they are, so the page can send one knob at a time.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LookPatch {
    pub background: Option<BackgroundChoice>,
    pub veil: Option<u8>,
}

/// Read a patch out of the request body. `None` for a body that is not the
/// shape; a well-shaped body with a bad value is an [`Invalid`] from the
/// command, with the sentence.
pub fn parse_patch(doc: &Value) -> Option<LookPatch> {
    let record = doc.as_object()?;
    let mut patch = LookPatch::default();
    if let Some(background) = record.get("background") {
        let kind = background.get("kind")?.as_str()?;
        patch.background = Some(match kind {
            "none" => BackgroundChoice::None,
            "shipped" => BackgroundChoice::Shipped(background.get("name")?.as_str()?.to_string()),
            _ => return None,
        });
    }
    if let Some(veil) = record.get("veil") {
        patch.veil = Some(u8::try_from(veil.as_u64()?).ok()?);
    }
    if patch.background.is_none() && patch.veil.is_none() {
        return None;
    }
    Some(patch)
}

// ── Widgets ───────────────────────────────────────────────────────────────

/// What the editor sends: a widget with or without an id. Without one it is
/// new; with one it replaces the widget of that id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetDraft {
    pub id: Option<String>,
    pub name: String,
    pub span: Span,
    pub files: Vec<WidgetFile>,
}

/// Read a draft out of the request body, or `None` for a body that is not
/// the shape. Sizes are checked by the command, with the sentence. A body
/// with a single `source` instead of `files` is a one-file widget.
pub fn parse_draft(doc: &Value) -> Option<WidgetDraft> {
    let record = doc.as_object()?;
    let id = match record.get("id") {
        None | Some(Value::Null) => None,
        Some(Value::String(id)) => Some(id.clone()),
        Some(_) => return None,
    };
    let name = record.get("name")?.as_str()?.to_string();
    let files = match record.get("files") {
        Some(files) => parse_files(files)?,
        None => vec![WidgetFile::entry(record.get("source")?.as_str()?)],
    };
    let span = match record.get("span").and_then(Value::as_str) {
        None | Some("half") => Span::Half,
        Some("full") => Span::Full,
        Some(_) => return None,
    };
    Some(WidgetDraft {
        id,
        name,
        span,
        files,
    })
}

/// The rules a draft has to meet, pure, so the HTTP layer and the command
/// refuse the same things with the same words.
pub fn validate_draft(draft: &WidgetDraft) -> Result<(), Invalid> {
    let name = draft.name.trim();
    if name.is_empty() {
        return Err(Invalid("the widget needs a name".to_string()));
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(Invalid(format!(
            "the name is longer than {MAX_NAME_CHARS} characters"
        )));
    }
    if name.chars().any(char::is_control) {
        return Err(Invalid(
            "the name has a control character in it".to_string(),
        ));
    }
    validate_files(&draft.files)?;
    if let Some(id) = &draft.id {
        if !is_widget_id(id) {
            return Err(Invalid("that is not a widget id".to_string()));
        }
    }
    Ok(())
}

/// Ids are `w` and digits, minted here, never by the page.
pub fn is_widget_id(id: &str) -> bool {
    id.len() > 1
        && id.len() <= 24
        && id.starts_with('w')
        && id[1..].bytes().all(|b| b.is_ascii_digit())
}

// ── Commands ──────────────────────────────────────────────────────────────

/// The background a call reads: the box's own, a person's, or, for the
/// owner who has none yet, a copy of the box's own, kept as theirs. The copy
/// is made once: afterwards the person has a file, whatever they pick.
pub fn mine<S: Store>(l: &mut S, whose: &Whose) -> anyhow::Result<Mine> {
    match whose {
        Whose::Box => {
            let look = l.load_look();
            Ok(Mine {
                background: look.background,
                veil: look.veil,
            })
        }
        Whose::Person { name, owner } => {
            if let Some(mine) = l.load_person(name) {
                return Ok(mine);
            }
            if !owner {
                return Ok(Mine::default());
            }
            let look = l.load_look();
            let mut copy = Mine {
                background: look.background,
                veil: look.veil,
            };
            if matches!(copy.background, Background::Upload { .. }) {
                // The bytes first, as on an upload.
                match l.read_picture(None)? {
                    Some(bytes) => l.write_picture(Some(name), &bytes)?,
                    None => copy.background = Background::None,
                }
            }
            l.save_person(name, &copy)?;
            Ok(copy)
        }
    }
}

fn save_mine<S: Store>(l: &mut S, whose: &Whose, mine: &Mine) -> anyhow::Result<()> {
    match whose {
        Whose::Box => {
            let mut look = l.load_look();
            look.background = mine.background.clone();
            look.veil = mine.veil;
            l.save_look(&look)
        }
        Whose::Person { name, .. } => l.save_person(name, mine),
    }
}

/// The look as the page should see it: the caller's background, the box's
/// widgets, and the URL of an uploaded picture, which is derived rather than
/// stored. `personal` says whether the background is the caller's own or
/// the box's.
pub fn render(look: &Look, mine: &Mine, whose: &Whose) -> Value {
    let background = match &mine.background {
        Background::None => json!({ "kind": "none" }),
        Background::Shipped { name } => json!({ "kind": "shipped", "name": name }),
        Background::Upload {
            version,
            content_type,
        } => json!({
            "kind": "upload",
            "version": version,
            "contentType": content_type,
            "url": background_url(whose.name(), *version),
        }),
    };
    json!({
        "version": look.version,
        "background": background,
        "veil": mine.veil,
        "personal": matches!(whose, Whose::Person { .. }),
        "widgets": look.widgets,
        "shipped": SHIPPED,
        "limits": {
            "widgets": MAX_WIDGETS,
            "nameChars": MAX_NAME_CHARS,
            "widgetBytes": MAX_WIDGET_BYTES,
            "files": MAX_FILES,
            "fileNameChars": MAX_FILE_NAME_CHARS,
            "fileKinds": FILE_KINDS,
            "imageBytes": MAX_IMAGE_BYTES,
            "veil": { "min": MIN_VEIL, "max": MAX_VEIL },
        },
    })
}

/// `GET /api/look`.
pub fn cmd_look<S: Store>(l: &mut S, whose: &Whose) -> anyhow::Result<Value> {
    let mine = mine(l, whose)?;
    Ok(render(&l.load_look(), &mine, whose))
}

/// `POST /api/look`: the caller's background by choice, their veil, or
/// both.
///
/// Choosing anything but the upload deletes an uploaded picture: its owner
/// has moved on from it, and a megabyte nobody can see has no business
/// staying on `/persist`.
pub fn cmd_patch_look<S: Store>(
    l: &mut S,
    whose: &Whose,
    patch: &LookPatch,
) -> anyhow::Result<Value> {
    let mut mine = mine(l, whose)?;
    if let Some(veil) = patch.veil {
        if !(MIN_VEIL..=MAX_VEIL).contains(&veil) {
            return invalid(format!(
                "the veil must be between {MIN_VEIL} and {MAX_VEIL} percent"
            ));
        }
        mine.veil = veil;
    }
    if let Some(choice) = &patch.background {
        let next = match choice {
            BackgroundChoice::None => Background::None,
            BackgroundChoice::Shipped(name) => {
                if !SHIPPED.contains(&name.as_str()) {
                    return invalid("that is not one of the pictures this box ships");
                }
                Background::Shipped { name: name.clone() }
            }
        };
        if matches!(mine.background, Background::Upload { .. }) {
            l.remove_picture(whose.name())?;
        }
        mine.background = next;
    }
    save_mine(l, whose, &mine)?;
    Ok(render(&l.load_look(), &mine, whose))
}

/// `PUT /api/look/background`: the caller's picture, raw.
pub fn cmd_upload_background<S: Store>(
    l: &mut S,
    whose: &Whose,
    bytes: &[u8],
) -> anyhow::Result<Value> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return invalid(format!(
            "the picture is larger than {} MiB",
            MAX_IMAGE_BYTES / (1024 * 1024)
        ));
    }
    let Some(content_type) = sniff_image(bytes) else {
        return invalid("the picture must be a PNG, JPEG, GIF or WebP");
    };
    let mut mine = mine(l, whose)?;
    let version = match mine.background {
        Background::Upload { version, .. } => version + 1,
        _ => 1,
    };
    // The bytes first, then the document that points at them: a crash in
    // between leaves a picture nothing names, never a name with no picture.
    l.write_picture(whose.name(), bytes)?;
    mine.background = Background::Upload {
        version,
        content_type: content_type.to_string(),
    };
    save_mine(l, whose, &mine)?;
    Ok(render(&l.load_look(), &mine, whose))
}

/// What `GET /api/look/background[/{name}]` serves: the bytes and their
/// type, or `None` when that background is not an uploaded picture. Nothing
/// is copied here: a picture is asked for after the look that names it.
pub fn cmd_read_background<S: Store>(
    l: &mut S,
    name: Option<&str>,
) -> anyhow::Result<Option<(String, Vec<u8>)>> {
    let background = match name {
        None => l.load_look().background,
        Some(name) => match l.load_person(name) {
            Some(mine) => mine.background,
            None => return Ok(None),
        },
    };
    let Background::Upload { content_type, .. } = background else {
        return Ok(None);
    };
    Ok(l.read_picture(name)?.map(|bytes| (content_type, bytes)))
}

/// `POST /api/look/widgets`: add a widget, or replace the one its id names.
pub fn cmd_put_widget<S: Store>(l: &mut S, draft: &WidgetDraft) -> anyhow::Result<Value> {
    validate_draft(draft).map_err(anyhow::Error::new)?;
    let mut look = l.load_look();
    let name = draft.name.trim().to_string();
    let widget = match &draft.id {
        Some(id) => {
            let Some(existing) = look.widgets.iter_mut().find(|w| &w.id == id) else {
                return invalid("there is no widget with that id on this box");
            };
            existing.name = name;
            existing.span = draft.span;
            existing.files = draft.files.clone();
            existing.clone()
        }
        None => {
            if look.widgets.len() >= MAX_WIDGETS {
                return invalid(format!("this box keeps at most {MAX_WIDGETS} widgets"));
            }
            let widget = HandWidget {
                id: format!("w{}", look.next_id),
                name,
                span: draft.span,
                files: draft.files.clone(),
            };
            look.next_id += 1;
            look.widgets.push(widget.clone());
            widget
        }
    };
    l.save_look(&look)?;
    Ok(json!({ "widget": widget }))
}

/// `DELETE /api/look/widgets/{id}`. Idempotent: deleting a widget that is
/// already gone is `deleted: false`, not an error — a 404 would be read by
/// the page as "this box does not serve the route".
pub fn cmd_delete_widget<S: Store>(l: &mut S, id: &str) -> anyhow::Result<Value> {
    if !is_widget_id(id) {
        return invalid("that is not a widget id");
    }
    let mut look = l.load_look();
    let before = look.widgets.len();
    look.widgets.retain(|w| w.id != id);
    let deleted = look.widgets.len() != before;
    if deleted {
        l.save_look(&look)?;
    }
    Ok(json!({ "deleted": deleted }))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeMap;

    /// The look in memory: the box's document, each person's, and the
    /// pictures by whose they are (`None` the box's).
    #[derive(Default)]
    struct Mem {
        look: Look,
        people: BTreeMap<String, Mine>,
        pictures: BTreeMap<Option<String>, Vec<u8>>,
    }

    impl Store for Mem {
        fn load_look(&mut self) -> Look {
            self.look.clone()
        }
        fn save_look(&mut self, look: &Look) -> anyhow::Result<()> {
            self.look = look.clone();
            Ok(())
        }
        fn load_person(&mut self, name: &str) -> Option<Mine> {
            self.people.get(name).cloned()
        }
        fn save_person(&mut self, name: &str, mine: &Mine) -> anyhow::Result<()> {
            self.people.insert(name.to_string(), mine.clone());
            Ok(())
        }
        fn write_picture(&mut self, name: Option<&str>, bytes: &[u8]) -> anyhow::Result<()> {
            self.pictures
                .insert(name.map(str::to_string), bytes.to_vec());
            Ok(())
        }
        fn read_picture(&mut self, name: Option<&str>) -> anyhow::Result<Option<Vec<u8>>> {
            Ok(self.pictures.get(&name.map(str::to_string)).cloned())
        }
        fn remove_picture(&mut self, name: Option<&str>) -> anyhow::Result<()> {
            self.pictures.remove(&name.map(str::to_string));
            Ok(())
        }
    }

    impl Mem {
        fn box_picture(&self) -> Option<&[u8]> {
            self.pictures.get(&None).map(Vec::as_slice)
        }
        fn picture_of(&self, name: &str) -> Option<&[u8]> {
            self.pictures
                .get(&Some(name.to_string()))
                .map(Vec::as_slice)
        }
    }

    const BOX: Whose = Whose::Box;

    fn person(name: &str) -> Whose {
        Whose::Person {
            name: name.to_string(),
            owner: false,
        }
    }

    fn owner(name: &str) -> Whose {
        Whose::Person {
            name: name.to_string(),
            owner: true,
        }
    }

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n0000";
    const JPEG: &[u8] = b"\xff\xd8\xff\xe0JFIF";
    const WEBP: &[u8] = b"RIFF\x00\x00\x00\x00WEBPVP8 ";

    fn draft(name: &str, source: &str) -> WidgetDraft {
        WidgetDraft {
            id: None,
            name: name.to_string(),
            span: Span::Half,
            files: vec![WidgetFile::entry(source)],
        }
    }

    fn file(name: &str, content: &str) -> WidgetFile {
        WidgetFile {
            name: name.to_string(),
            content: content.to_string(),
        }
    }

    #[test]
    fn a_fresh_box_has_a_plain_look() {
        let mut l = Mem::default();
        let doc = cmd_look(&mut l, &BOX).unwrap();
        assert_eq!(doc["background"]["kind"], "none");
        assert_eq!(doc["veil"], DEFAULT_VEIL);
        assert_eq!(doc["personal"], false);
        assert_eq!(doc["widgets"].as_array().unwrap().len(), 0);
        assert_eq!(doc["shipped"][0], "tide");
        assert_eq!(doc["limits"]["widgets"], MAX_WIDGETS);
    }

    #[test]
    fn sniffing_reads_the_bytes_not_the_label() {
        assert_eq!(sniff_image(PNG), Some("image/png"));
        assert_eq!(sniff_image(JPEG), Some("image/jpeg"));
        assert_eq!(sniff_image(b"GIF89a..."), Some("image/gif"));
        assert_eq!(sniff_image(WEBP), Some("image/webp"));
        assert_eq!(
            sniff_image(b"<svg xmlns='http://www.w3.org/2000/svg'/>"),
            None
        );
        assert_eq!(sniff_image(b"RIFF\x00\x00\x00\x00WAVE"), None);
        assert_eq!(sniff_image(b""), None);
    }

    #[test]
    fn an_upload_is_stored_versioned_and_served_back() {
        let mut l = Mem::default();
        let doc = cmd_upload_background(&mut l, &BOX, PNG).unwrap();
        assert_eq!(doc["background"]["kind"], "upload");
        assert_eq!(doc["background"]["version"], 1);
        assert_eq!(doc["background"]["contentType"], "image/png");
        assert_eq!(doc["background"]["url"], "/api/look/background?v=1");
        assert_eq!(l.box_picture(), Some(PNG));

        let (content_type, bytes) = cmd_read_background(&mut l, None).unwrap().unwrap();
        assert_eq!(content_type, "image/png");
        assert_eq!(bytes, PNG);

        // A replacement is a new URL, so no cache serves the old picture.
        let doc = cmd_upload_background(&mut l, &BOX, JPEG).unwrap();
        assert_eq!(doc["background"]["version"], 2);
        assert_eq!(doc["background"]["url"], "/api/look/background?v=2");
        assert_eq!(l.box_picture(), Some(JPEG));
    }

    #[test]
    fn each_person_has_their_own_background() {
        let mut l = Mem::default();
        let ada = person("ada");
        let bob = person("bob");
        let doc = cmd_upload_background(&mut l, &ada, PNG).unwrap();
        assert_eq!(doc["personal"], true);
        assert_eq!(doc["background"]["url"], "/api/look/background/ada?v=1");
        assert_eq!(l.picture_of("ada"), Some(PNG));
        let patch = LookPatch {
            background: Some(BackgroundChoice::Shipped("tide".to_string())),
            veil: Some(40),
        };
        cmd_patch_look(&mut l, &bob, &patch).unwrap();

        let a = cmd_look(&mut l, &ada).unwrap();
        assert_eq!(a["background"]["kind"], "upload");
        assert_eq!(a["veil"], DEFAULT_VEIL);
        let b = cmd_look(&mut l, &bob).unwrap();
        assert_eq!(b["background"]["name"], "tide");
        assert_eq!(b["veil"], 40);
        // Neither touched the box's own, nor the widgets they share.
        assert_eq!(
            cmd_look(&mut l, &BOX).unwrap()["background"]["kind"],
            "none"
        );
        assert!(l.box_picture().is_none());

        let (_, bytes) = cmd_read_background(&mut l, Some("ada")).unwrap().unwrap();
        assert_eq!(bytes, PNG);
        assert!(cmd_read_background(&mut l, Some("bob")).unwrap().is_none());
        assert!(cmd_read_background(&mut l, Some("cyd")).unwrap().is_none());

        // Ada going plain drops her picture and nobody else's.
        cmd_upload_background(&mut l, &BOX, JPEG).unwrap();
        let plain = LookPatch {
            background: Some(BackgroundChoice::None),
            veil: None,
        };
        cmd_patch_look(&mut l, &ada, &plain).unwrap();
        assert!(l.picture_of("ada").is_none());
        assert_eq!(l.box_picture(), Some(JPEG));
    }

    #[test]
    fn the_owner_inherits_the_box_background_once() {
        let mut l = Mem::default();
        cmd_upload_background(&mut l, &BOX, PNG).unwrap();
        let veil = LookPatch {
            background: None,
            veil: Some(45),
        };
        cmd_patch_look(&mut l, &BOX, &veil).unwrap();

        // Someone else starts plain and is given nothing.
        let bob = cmd_look(&mut l, &person("bob")).unwrap();
        assert_eq!(bob["background"]["kind"], "none");
        assert!(l.people.is_empty(), "reading a plain look writes nothing");

        // The owner's first look is a copy of the box's, picture and veil.
        let ola = owner("ola");
        let doc = cmd_look(&mut l, &ola).unwrap();
        assert_eq!(doc["background"]["kind"], "upload");
        assert_eq!(doc["background"]["url"], "/api/look/background/ola?v=1");
        assert_eq!(doc["veil"], 45);
        assert_eq!(l.picture_of("ola"), Some(PNG));
        assert_eq!(l.box_picture(), Some(PNG), "the spare key keeps the box's");

        // Once: going plain sticks, and a later box picture is not copied.
        let plain = LookPatch {
            background: Some(BackgroundChoice::None),
            veil: None,
        };
        cmd_patch_look(&mut l, &ola, &plain).unwrap();
        cmd_upload_background(&mut l, &BOX, JPEG).unwrap();
        assert_eq!(
            cmd_look(&mut l, &ola).unwrap()["background"]["kind"],
            "none"
        );
        assert!(l.picture_of("ola").is_none());
    }

    #[test]
    fn an_owner_with_a_box_picture_gone_missing_starts_plain() {
        let mut l = Mem::default();
        cmd_upload_background(&mut l, &BOX, PNG).unwrap();
        l.pictures.clear();
        let doc = cmd_look(&mut l, &owner("ola")).unwrap();
        assert_eq!(doc["background"]["kind"], "none");
        assert_eq!(l.people["ola"].background, Background::None);
    }

    #[test]
    fn whose_comes_from_the_caller() {
        use losos_plugin::{Person, Principal};
        let p = |name: &str, groups: &[&str]| {
            Principal::Person(Person {
                name: name.to_string(),
                display_name: name.to_string(),
                groups: groups.iter().map(|g| (*g).to_string()).collect(),
                admin: false,
            })
        };
        assert_eq!(Whose::of(&Principal::Spare).unwrap(), Whose::Box);
        assert_eq!(Whose::of(&Principal::Anonymous).unwrap(), Whose::Box);
        assert_eq!(
            Whose::of(&p("ada", &["losos-look"])).unwrap(),
            person("ada")
        );
        assert_eq!(
            Whose::of(&p("ola", &["losos-admins", OWNER_GROUP])).unwrap(),
            owner("ola")
        );
        for bad in ["", "../etc", "Ada", "a/b", "1ada", ".hidden"] {
            assert!(Whose::of(&p(bad, &[])).is_err(), "{bad}");
        }
    }

    #[test]
    fn an_upload_that_is_not_a_picture_is_refused_and_nothing_is_written() {
        let mut l = Mem::default();
        let err = cmd_upload_background(&mut l, &BOX, b"<svg/>").unwrap_err();
        assert!(err.downcast_ref::<Invalid>().is_some(), "{err}");
        assert!(l.pictures.is_empty());
        assert_eq!(l.look.background, Background::None);

        let big = vec![0u8; MAX_IMAGE_BYTES + 1];
        let err = cmd_upload_background(&mut l, &person("ada"), &big).unwrap_err();
        assert!(err.downcast_ref::<Invalid>().is_some(), "{err}");
        assert!(l.pictures.is_empty());
        assert!(l.people.is_empty());
    }

    #[test]
    fn picking_a_shipped_picture_drops_the_upload() {
        let mut l = Mem::default();
        cmd_upload_background(&mut l, &BOX, PNG).unwrap();
        let patch = LookPatch {
            background: Some(BackgroundChoice::Shipped("dusk".to_string())),
            veil: None,
        };
        let doc = cmd_patch_look(&mut l, &BOX, &patch).unwrap();
        assert_eq!(doc["background"]["kind"], "shipped");
        assert_eq!(doc["background"]["name"], "dusk");
        assert!(l.box_picture().is_none(), "the upload should be removed");
        assert!(cmd_read_background(&mut l, None).unwrap().is_none());
    }

    #[test]
    fn a_picture_the_box_does_not_ship_is_refused() {
        let mut l = Mem::default();
        let patch = LookPatch {
            background: Some(BackgroundChoice::Shipped("../etc/passwd".to_string())),
            veil: None,
        };
        let err = cmd_patch_look(&mut l, &person("ada"), &patch).unwrap_err();
        assert!(err.downcast_ref::<Invalid>().is_some(), "{err}");
        assert!(l.people.is_empty());
    }

    #[test]
    fn the_veil_is_clamped_to_the_readable_range() {
        let mut l = Mem::default();
        let ok = LookPatch {
            background: None,
            veil: Some(35),
        };
        assert_eq!(cmd_patch_look(&mut l, &BOX, &ok).unwrap()["veil"], 35);
        for bad in [0u8, MIN_VEIL - 1, MAX_VEIL + 1, 200] {
            let patch = LookPatch {
                background: None,
                veil: Some(bad),
            };
            let err = cmd_patch_look(&mut l, &BOX, &patch).unwrap_err();
            assert!(err.downcast_ref::<Invalid>().is_some(), "{bad}: {err}");
        }
        assert_eq!(l.look.veil, 35, "a refused patch changes nothing");
    }

    #[test]
    fn patches_are_read_strictly() {
        assert_eq!(
            parse_patch(&json!({ "background": { "kind": "none" } })),
            Some(LookPatch {
                background: Some(BackgroundChoice::None),
                veil: None
            })
        );
        assert_eq!(
            parse_patch(
                &json!({ "background": { "kind": "shipped", "name": "tide" }, "veil": 40 })
            ),
            Some(LookPatch {
                background: Some(BackgroundChoice::Shipped("tide".to_string())),
                veil: Some(40)
            })
        );
        assert_eq!(parse_patch(&json!({})), None);
        assert_eq!(
            parse_patch(&json!({ "background": { "kind": "upload" } })),
            None
        );
        assert_eq!(
            parse_patch(&json!({ "background": { "kind": "shipped" } })),
            None
        );
        assert_eq!(parse_patch(&json!({ "veil": "sixty" })), None);
        assert_eq!(parse_patch(&json!({ "veil": -1 })), None);
        assert_eq!(parse_patch(&json!([])), None);
    }

    #[test]
    fn widgets_are_minted_with_ids_that_are_never_reused() {
        let mut l = Mem::default();
        let first = cmd_put_widget(&mut l, &draft("Clock", "<b>12:00</b>")).unwrap();
        assert_eq!(first["widget"]["id"], "w1");
        assert_eq!(first["widget"]["name"], "Clock");
        assert_eq!(first["widget"]["span"], "half");
        let second = cmd_put_widget(&mut l, &draft("Weather", "<i>sun</i>")).unwrap();
        assert_eq!(second["widget"]["id"], "w2");

        assert_eq!(cmd_delete_widget(&mut l, "w1").unwrap()["deleted"], true);
        assert_eq!(cmd_delete_widget(&mut l, "w1").unwrap()["deleted"], false);

        let third = cmd_put_widget(&mut l, &draft("Clock again", "<b>1</b>")).unwrap();
        assert_eq!(third["widget"]["id"], "w3", "w1 must not come back");

        let doc = cmd_look(&mut l, &person("ada")).unwrap();
        let ids: Vec<&str> = doc["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["w2", "w3"]);
    }

    #[test]
    fn a_widget_is_edited_in_place() {
        let mut l = Mem::default();
        cmd_put_widget(&mut l, &draft("Clock", "<b>12:00</b>")).unwrap();
        let edit = WidgetDraft {
            id: Some("w1".to_string()),
            name: "  Big clock  ".to_string(),
            span: Span::Full,
            files: vec![
                file(
                    "index.html",
                    "<b>13:00</b><link rel=stylesheet href=clock.css>",
                ),
                file("clock.css", "b { color: red }"),
            ],
        };
        let doc = cmd_put_widget(&mut l, &edit).unwrap();
        assert_eq!(doc["widget"]["id"], "w1");
        assert_eq!(doc["widget"]["name"], "Big clock", "names are trimmed");
        assert_eq!(doc["widget"]["span"], "full");
        assert_eq!(l.look.widgets.len(), 1);
        assert_eq!(l.look.widgets[0].files.len(), 2);
        assert_eq!(l.look.widgets[0].files[1].name, "clock.css");

        let missing = WidgetDraft {
            id: Some("w9".to_string()),
            ..edit
        };
        let err = cmd_put_widget(&mut l, &missing).unwrap_err();
        assert!(err.downcast_ref::<Invalid>().is_some(), "{err}");
    }

    #[test]
    fn the_sizes_are_enforced() {
        let mut l = Mem::default();
        let cases = [
            draft("", "<b/>"),
            draft("   ", "<b/>"),
            draft(&"n".repeat(MAX_NAME_CHARS + 1), "<b/>"),
            draft("tab\there", "<b/>"),
            draft("Empty", "   "),
            draft("Huge", &"x".repeat(MAX_WIDGET_BYTES + 1)),
            WidgetDraft {
                id: Some("../w1".to_string()),
                ..draft("Bad id", "<b/>")
            },
        ];
        for case in &cases {
            let err = cmd_put_widget(&mut l, case).unwrap_err();
            assert!(err.downcast_ref::<Invalid>().is_some(), "{case:?}: {err}");
        }
        assert!(l.look.widgets.is_empty());

        // Exactly at the limits is fine.
        let edge = draft(&"n".repeat(MAX_NAME_CHARS), &"x".repeat(MAX_WIDGET_BYTES));
        cmd_put_widget(&mut l, &edge).unwrap();

        for i in 1..MAX_WIDGETS {
            cmd_put_widget(&mut l, &draft(&format!("w{i}"), "<b/>")).unwrap();
        }
        let err = cmd_put_widget(&mut l, &draft("one too many", "<b/>")).unwrap_err();
        assert!(err.downcast_ref::<Invalid>().is_some(), "{err}");
        assert_eq!(l.look.widgets.len(), MAX_WIDGETS);
    }

    #[test]
    fn a_widget_saved_with_one_source_reads_as_its_index() {
        let look: Look = serde_json::from_str(
            r#"{"widgets":[{"id":"w1","name":"Old","span":"half","source":"<b>1</b>"}]}"#,
        )
        .unwrap();
        assert_eq!(look.widgets[0].files, vec![WidgetFile::entry("<b>1</b>")]);
        // And it is written back with files, not source.
        let written = serde_json::to_value(&look.widgets[0]).unwrap();
        assert!(written.get("source").is_none());
        assert_eq!(written["files"][0]["name"], "index.html");
    }

    #[test]
    fn drafts_are_read_strictly() {
        let doc = json!({ "name": "Clock", "source": "<b/>", "span": "full" });
        let parsed = parse_draft(&doc).unwrap();
        assert_eq!(parsed.span, Span::Full);
        assert_eq!(parsed.id, None);
        assert_eq!(parsed.files, vec![WidgetFile::entry("<b/>")]);
        let doc = json!({ "name": "Clock", "files": [
            { "name": "index.html", "content": "<b/>" },
            { "name": "a.css", "content": "" },
        ] });
        assert_eq!(parse_draft(&doc).unwrap().files.len(), 2);
        assert!(parse_draft(&json!({ "name": "x", "files": "<b/>" })).is_none());
        assert!(
            parse_draft(&json!({ "name": "x", "files": [{ "name": "index.html" }] })).is_none()
        );
        let doc = json!({ "id": "w4", "name": "Clock", "source": "<b/>" });
        assert_eq!(parse_draft(&doc).unwrap().span, Span::Half);
        assert_eq!(parse_draft(&doc).unwrap().id.as_deref(), Some("w4"));
        assert!(parse_draft(&json!({ "name": "Clock" })).is_none());
        assert!(parse_draft(&json!({ "name": 1, "source": "<b/>" })).is_none());
        assert!(parse_draft(&json!({ "name": "x", "source": "<b/>", "span": "wide" })).is_none());
        assert!(parse_draft(&json!({ "id": 7, "name": "x", "source": "<b/>" })).is_none());
    }

    #[test]
    fn an_older_or_damaged_document_still_opens() {
        let look: Look = serde_json::from_str(r#"{"version":1}"#).unwrap();
        assert_eq!(look, Look::default());
        let look: Look =
            serde_json::from_str(r#"{"background":{"kind":"shipped","name":"grid"},"extra":true}"#)
                .unwrap();
        assert_eq!(
            look.background,
            Background::Shipped {
                name: "grid".to_string()
            }
        );
        assert_eq!(look.veil, DEFAULT_VEIL);
    }
}
