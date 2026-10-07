//! The owner's look: a background picture and the widgets they wrote by hand.
//!
//! Both are **appliance** state, not browser state, and that is the one
//! decision this module exists to carry. The widget board itself stays in the
//! browser (`admin-ui/app/src/lib/widgets.ts` says why: nothing on a
//! dashboard may be able to start a rebuild), but a picture the owner chose
//! and a widget they typed by hand are the box's, so a phone and a laptop see
//! the same page. Neither touches `overrides.nix` or starts a rebuild: the
//! document lives beside `state.json` under `$LOSOS_STATE_DIR` and is written
//! the same way, atomically, by the one daemon that owns that directory.
//!
//! What a hand-written widget is, from here: a name, a size, and a source
//! text the admin page renders inside a **sandboxed iframe** served under its
//! own, permissive-for-itself CSP (`/widget-frame/`, see
//! `modules/containers.nix`). lososd stores the text and never interprets it;
//! the only rules enforced here are the sizes, so one widget cannot be a
//! gigabyte and a thousand of them cannot be a thousand. The security argument
//! for running owner-written script at all is in the frame's own header
//! (`admin-ui/app/public/widget-frame/index.html`), not here.
//!
//! The picture is read back *without* a token (`GET /api/look/background`):
//! a CSS `background-image` cannot carry a Bearer header, and the alternative
//! — fetching bytes with the token and painting a `blob:` URL — needs an
//! `img-src blob:` grant the admin CSP deliberately does not make. The route
//! is behind the same LAN-only guard as the page that shows it, and the
//! picture is the owner's wallpaper, no more secret than the logo beside it.
//! Everything that *changes* the look needs the token and is audited.

use crate::losos::Losos;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// How many widgets the box keeps. Each one is an iframe on the board; a
/// mini-PC serving photos has no business hosting a hundred of them.
pub const MAX_WIDGETS: usize = 24;
/// Longest widget name, in characters.
pub const MAX_NAME_CHARS: usize = 60;
/// Longest widget source, in bytes. Generous for a hand-written tile, far
/// short of anything that could be mistaken for an application.
pub const MAX_SOURCE_BYTES: usize = 64 * 1024;
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

/// Where the page gets the uploaded picture; `?v=` makes a replacement a new
/// URL so no cache serves the old one.
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
pub struct HandWidget {
    pub id: String,
    pub name: String,
    pub span: Span,
    /// HTML, with its own `<style>` and `<script>`, rendered in the frame.
    pub source: String,
}

/// The persisted document at `$LOSOS_STATE_DIR/look.json`.
///
/// Decoding is lenient the way `state.json` is: a document that does not
/// parse reads as [`Look::default`] (see `io_backend`), and every field has a
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

/// A request the owner can act on: the field that was wrong, in a sentence.
/// The HTTP layer answers it as a 400 carrying the sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalid(pub String);

impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Invalid {}

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

/// Where the page fetches an uploaded picture from, versioned.
pub fn background_url(version: u64) -> String {
    format!("{BACKGROUND_ROUTE}?v={version}")
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
    pub source: String,
}

/// Read a draft out of the request body, or `None` for a body that is not
/// the shape. Sizes are checked by the command, with the sentence.
pub fn parse_draft(doc: &Value) -> Option<WidgetDraft> {
    let record = doc.as_object()?;
    let id = match record.get("id") {
        None | Some(Value::Null) => None,
        Some(Value::String(id)) => Some(id.clone()),
        Some(_) => return None,
    };
    let name = record.get("name")?.as_str()?.to_string();
    let source = record.get("source")?.as_str()?.to_string();
    let span = match record.get("span").and_then(Value::as_str) {
        None | Some("half") => Span::Half,
        Some("full") => Span::Full,
        Some(_) => return None,
    };
    Some(WidgetDraft {
        id,
        name,
        span,
        source,
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
    if draft.source.trim().is_empty() {
        return Err(Invalid("the widget has no source".to_string()));
    }
    if draft.source.len() > MAX_SOURCE_BYTES {
        return Err(Invalid(format!(
            "the source is longer than {} KiB",
            MAX_SOURCE_BYTES / 1024
        )));
    }
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

/// The look as the page should see it: the document, plus the URL of an
/// uploaded picture, which is derived rather than stored.
pub fn render(look: &Look) -> Value {
    let background = match &look.background {
        Background::None => json!({ "kind": "none" }),
        Background::Shipped { name } => json!({ "kind": "shipped", "name": name }),
        Background::Upload {
            version,
            content_type,
        } => json!({
            "kind": "upload",
            "version": version,
            "contentType": content_type,
            "url": background_url(*version),
        }),
    };
    json!({
        "version": look.version,
        "background": background,
        "veil": look.veil,
        "widgets": look.widgets,
        "shipped": SHIPPED,
        "limits": {
            "widgets": MAX_WIDGETS,
            "nameChars": MAX_NAME_CHARS,
            "sourceBytes": MAX_SOURCE_BYTES,
            "imageBytes": MAX_IMAGE_BYTES,
            "veil": { "min": MIN_VEIL, "max": MAX_VEIL },
        },
    })
}

/// `GET /api/look`.
pub fn cmd_look<L: Losos>(l: &mut L) -> anyhow::Result<Value> {
    Ok(render(&l.load_look()?))
}

/// `POST /api/look`: the background by choice, the veil, or both.
///
/// Choosing anything but the upload deletes an uploaded picture: the owner
/// has moved on from it, and a megabyte nobody can see has no business
/// staying on `/persist`.
pub fn cmd_patch_look<L: Losos>(l: &mut L, patch: &LookPatch) -> anyhow::Result<Value> {
    let mut look = l.load_look()?;
    if let Some(veil) = patch.veil {
        if !(MIN_VEIL..=MAX_VEIL).contains(&veil) {
            return invalid(format!(
                "the veil must be between {MIN_VEIL} and {MAX_VEIL} percent"
            ));
        }
        look.veil = veil;
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
        if matches!(look.background, Background::Upload { .. }) {
            l.remove_background()?;
        }
        look.background = next;
    }
    l.save_look(&look)?;
    Ok(render(&look))
}

/// `PUT /api/look/background`: the picture, raw.
pub fn cmd_upload_background<L: Losos>(l: &mut L, bytes: &[u8]) -> anyhow::Result<Value> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return invalid(format!(
            "the picture is larger than {} MiB",
            MAX_IMAGE_BYTES / (1024 * 1024)
        ));
    }
    let Some(content_type) = sniff_image(bytes) else {
        return invalid("the picture must be a PNG, JPEG, GIF or WebP");
    };
    let mut look = l.load_look()?;
    let version = match look.background {
        Background::Upload { version, .. } => version + 1,
        _ => 1,
    };
    // The bytes first, then the document that points at them: a crash in
    // between leaves a picture nothing names, never a name with no picture.
    l.write_background(bytes)?;
    look.background = Background::Upload {
        version,
        content_type: content_type.to_string(),
    };
    l.save_look(&look)?;
    Ok(render(&look))
}

/// What `GET /api/look/background` serves: the bytes and their type, or
/// `None` when the look has no uploaded picture.
pub fn cmd_read_background<L: Losos>(l: &mut L) -> anyhow::Result<Option<(String, Vec<u8>)>> {
    let look = l.load_look()?;
    let Background::Upload { content_type, .. } = look.background else {
        return Ok(None);
    };
    Ok(l.read_background()?.map(|bytes| (content_type, bytes)))
}

/// `POST /api/look/widgets`: add a widget, or replace the one its id names.
pub fn cmd_put_widget<L: Losos>(l: &mut L, draft: &WidgetDraft) -> anyhow::Result<Value> {
    validate_draft(draft).map_err(anyhow::Error::new)?;
    let mut look = l.load_look()?;
    let name = draft.name.trim().to_string();
    let widget = match &draft.id {
        Some(id) => {
            let Some(existing) = look.widgets.iter_mut().find(|w| &w.id == id) else {
                return invalid("there is no widget with that id on this box");
            };
            existing.name = name;
            existing.span = draft.span;
            existing.source = draft.source.clone();
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
                source: draft.source.clone(),
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
pub fn cmd_delete_widget<L: Losos>(l: &mut L, id: &str) -> anyhow::Result<Value> {
    if !is_widget_id(id) {
        return invalid("that is not a widget id");
    }
    let mut look = l.load_look()?;
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
    use crate::fake::FakeLosos;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n0000";
    const JPEG: &[u8] = b"\xff\xd8\xff\xe0JFIF";
    const WEBP: &[u8] = b"RIFF\x00\x00\x00\x00WEBPVP8 ";

    fn draft(name: &str, source: &str) -> WidgetDraft {
        WidgetDraft {
            id: None,
            name: name.to_string(),
            span: Span::Half,
            source: source.to_string(),
        }
    }

    #[test]
    fn a_fresh_box_has_a_plain_look() {
        let mut l = FakeLosos::new();
        let doc = cmd_look(&mut l).unwrap();
        assert_eq!(doc["background"]["kind"], "none");
        assert_eq!(doc["veil"], DEFAULT_VEIL);
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
        let mut l = FakeLosos::new();
        let doc = cmd_upload_background(&mut l, PNG).unwrap();
        assert_eq!(doc["background"]["kind"], "upload");
        assert_eq!(doc["background"]["version"], 1);
        assert_eq!(doc["background"]["contentType"], "image/png");
        assert_eq!(doc["background"]["url"], "/api/look/background?v=1");
        assert_eq!(l.background.as_deref(), Some(PNG));

        let (content_type, bytes) = cmd_read_background(&mut l).unwrap().unwrap();
        assert_eq!(content_type, "image/png");
        assert_eq!(bytes, PNG);

        // A replacement is a new URL, so no cache serves the old picture.
        let doc = cmd_upload_background(&mut l, JPEG).unwrap();
        assert_eq!(doc["background"]["version"], 2);
        assert_eq!(doc["background"]["url"], "/api/look/background?v=2");
        assert_eq!(l.background.as_deref(), Some(JPEG));
    }

    #[test]
    fn an_upload_that_is_not_a_picture_is_refused_and_nothing_is_written() {
        let mut l = FakeLosos::new();
        let err = cmd_upload_background(&mut l, b"<svg/>").unwrap_err();
        assert!(err.downcast_ref::<Invalid>().is_some(), "{err}");
        assert!(l.background.is_none());
        assert_eq!(l.look.background, Background::None);

        let big = vec![0u8; MAX_IMAGE_BYTES + 1];
        let err = cmd_upload_background(&mut l, &big).unwrap_err();
        assert!(err.downcast_ref::<Invalid>().is_some(), "{err}");
        assert!(l.background.is_none());
    }

    #[test]
    fn picking_a_shipped_picture_drops_the_upload() {
        let mut l = FakeLosos::new();
        cmd_upload_background(&mut l, PNG).unwrap();
        let patch = LookPatch {
            background: Some(BackgroundChoice::Shipped("dusk".to_string())),
            veil: None,
        };
        let doc = cmd_patch_look(&mut l, &patch).unwrap();
        assert_eq!(doc["background"]["kind"], "shipped");
        assert_eq!(doc["background"]["name"], "dusk");
        assert!(l.background.is_none(), "the upload should be removed");
        assert!(cmd_read_background(&mut l).unwrap().is_none());
    }

    #[test]
    fn a_picture_the_box_does_not_ship_is_refused() {
        let mut l = FakeLosos::new();
        let patch = LookPatch {
            background: Some(BackgroundChoice::Shipped("../etc/passwd".to_string())),
            veil: None,
        };
        let err = cmd_patch_look(&mut l, &patch).unwrap_err();
        assert!(err.downcast_ref::<Invalid>().is_some(), "{err}");
        assert_eq!(l.look.background, Background::None);
    }

    #[test]
    fn the_veil_is_clamped_to_the_readable_range() {
        let mut l = FakeLosos::new();
        let ok = LookPatch {
            background: None,
            veil: Some(35),
        };
        assert_eq!(cmd_patch_look(&mut l, &ok).unwrap()["veil"], 35);
        for bad in [0u8, MIN_VEIL - 1, MAX_VEIL + 1, 200] {
            let patch = LookPatch {
                background: None,
                veil: Some(bad),
            };
            let err = cmd_patch_look(&mut l, &patch).unwrap_err();
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
        let mut l = FakeLosos::new();
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

        let doc = cmd_look(&mut l).unwrap();
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
        let mut l = FakeLosos::new();
        cmd_put_widget(&mut l, &draft("Clock", "<b>12:00</b>")).unwrap();
        let edit = WidgetDraft {
            id: Some("w1".to_string()),
            name: "  Big clock  ".to_string(),
            span: Span::Full,
            source: "<b>13:00</b>".to_string(),
        };
        let doc = cmd_put_widget(&mut l, &edit).unwrap();
        assert_eq!(doc["widget"]["id"], "w1");
        assert_eq!(doc["widget"]["name"], "Big clock", "names are trimmed");
        assert_eq!(doc["widget"]["span"], "full");
        assert_eq!(l.look.widgets.len(), 1);
        assert_eq!(l.look.widgets[0].source, "<b>13:00</b>");

        let missing = WidgetDraft {
            id: Some("w9".to_string()),
            ..edit
        };
        let err = cmd_put_widget(&mut l, &missing).unwrap_err();
        assert!(err.downcast_ref::<Invalid>().is_some(), "{err}");
    }

    #[test]
    fn the_sizes_are_enforced() {
        let mut l = FakeLosos::new();
        let cases = [
            draft("", "<b/>"),
            draft("   ", "<b/>"),
            draft(&"n".repeat(MAX_NAME_CHARS + 1), "<b/>"),
            draft("tab\there", "<b/>"),
            draft("Empty", "   "),
            draft("Huge", &"x".repeat(MAX_SOURCE_BYTES + 1)),
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
        let edge = draft(&"n".repeat(MAX_NAME_CHARS), &"x".repeat(MAX_SOURCE_BYTES));
        cmd_put_widget(&mut l, &edge).unwrap();

        for i in 1..MAX_WIDGETS {
            cmd_put_widget(&mut l, &draft(&format!("w{i}"), "<b/>")).unwrap();
        }
        let err = cmd_put_widget(&mut l, &draft("one too many", "<b/>")).unwrap_err();
        assert!(err.downcast_ref::<Invalid>().is_some(), "{err}");
        assert_eq!(l.look.widgets.len(), MAX_WIDGETS);
    }

    #[test]
    fn drafts_are_read_strictly() {
        let doc = json!({ "name": "Clock", "source": "<b/>", "span": "full" });
        let parsed = parse_draft(&doc).unwrap();
        assert_eq!(parsed.span, Span::Full);
        assert_eq!(parsed.id, None);
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
