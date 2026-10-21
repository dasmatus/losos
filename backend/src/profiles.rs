//! Each person's profile: a picture, a banner, a colour, their pronouns and
//! a line about themselves.
//!
//! A profile belongs to the person on LosOS Login, not to one app. This
//! daemon keeps it once and every page that shows a person reads it from
//! here: the admin pages, and the box's own apps through `GET /api/profiles`
//! (`docs/profiles.md` is the contract). The name stays in the people file
//! (`crate::people`), which Authelia reads; nothing here is part of
//! signing in.
//!
//! Pictures are never stored as they arrived. Each one is decoded, turned
//! upright by its own orientation tag, cropped to its shape, scaled down and
//! written again as a fresh PNG or JPEG. That drops every tag the camera
//! wrote, the place the photo was taken among them, and anything else a
//! file carried after its image. It also means a page can show any of them
//! at a fixed size without reading the bytes first.
//!
//! The files live under `$LOSOS_PROFILES_DIR` (`/var/lib/losos/profiles`,
//! inside lososd's own state directory, persisted with `/var`): one
//! `profiles.json` and one file per picture, named by the sign-in name,
//! which `crate::people::validate_name` keeps to safe characters.

use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::people::BadDetails;

const DEFAULT_DIR: &str = "/var/lib/losos/profiles";
/// Most bytes in one upload, before it is decoded.
pub const MAX_UPLOAD_BYTES: usize = crate::look::MAX_IMAGE_BYTES;
/// The longest side a picture may have before it is decoded. A phone's
/// photo is 4000 to 8000 pixels on its long side; the admin page scales
/// before it sends, so only a client of its own sends more.
const MAX_SOURCE_SIDE: u32 = 8192;
/// The decoder may allocate this much at most, so a small file that claims a
/// huge picture cannot take the box's memory.
const MAX_DECODE_BYTES: u64 = 192 * 1024 * 1024;
/// Longest pronouns, in characters.
pub const MAX_PRONOUNS_CHARS: usize = 40;
/// Longest line about a person, in characters.
pub const MAX_ABOUT_CHARS: usize = 160;
/// JPEG quality for photos; transparent pictures stay PNG.
const JPEG_QUALITY: u8 = 86;

/// The two pictures a profile has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Square, shown round.
    Picture,
    /// Three times as wide as it is tall, behind the picture.
    Banner,
}

impl Kind {
    pub fn parse(s: &str) -> Option<Kind> {
        match s {
            "picture" => Some(Kind::Picture),
            "banner" => Some(Kind::Banner),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Picture => "picture",
            Kind::Banner => "banner",
        }
    }

    /// The size every stored picture of this kind has, at most: a smaller
    /// upload is cropped to the shape and kept at its own size.
    pub fn size(self) -> (u32, u32) {
        match self {
            Kind::Picture => (512, 512),
            Kind::Banner => (1500, 500),
        }
    }
}

/// The colours a person can pick, each a pair of tokens in
/// `admin-ui/app/src/styles/tokens.css` (`--person-<name>` and
/// `--person-<name>-wash`) with a light and a dark value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Accent {
    Teal,
    Blue,
    Violet,
    Rose,
    Amber,
    Green,
}

impl Accent {
    pub const ALL: [Accent; 6] = [
        Accent::Teal,
        Accent::Blue,
        Accent::Violet,
        Accent::Rose,
        Accent::Amber,
        Accent::Green,
    ];

    pub fn parse(s: &str) -> Option<Accent> {
        Accent::ALL.into_iter().find(|a| a.as_str() == s)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Accent::Teal => "teal",
            Accent::Blue => "blue",
            Accent::Violet => "violet",
            Accent::Rose => "rose",
            Accent::Amber => "amber",
            Accent::Green => "green",
        }
    }

    /// The colour of someone who has not picked one: the same for the same
    /// name on every page, so two people side by side usually differ.
    pub fn of_name(name: &str) -> Accent {
        // FNV-1a: stable across builds, unlike the standard library's hasher.
        let hash = name.bytes().fold(0x811c_9dc5_u32, |h, b| {
            (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
        });
        Accent::ALL[hash as usize % Accent::ALL.len()]
    }
}

/// One stored picture: which upload it is and its type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stored {
    /// Never reused for the same person, so the URL of a replaced or
    /// removed picture never serves the new one from a cache.
    pub version: u64,
    #[serde(rename = "contentType")]
    pub content_type: String,
}

/// What a person set on their profile.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub pronouns: String,
    #[serde(default)]
    pub about: String,
    /// `None`: the colour [`Accent::of_name`] gives them.
    #[serde(default)]
    pub accent: Option<Accent>,
    #[serde(default)]
    pub picture: Option<Stored>,
    #[serde(default)]
    pub banner: Option<Stored>,
    /// The next version a picture of theirs gets.
    #[serde(default = "first_version", rename = "nextVersion")]
    pub next_version: u64,
}

fn first_version() -> u64 {
    1
}

impl Profile {
    fn slot(&mut self, kind: Kind) -> &mut Option<Stored> {
        match kind {
            Kind::Picture => &mut self.picture,
            Kind::Banner => &mut self.banner,
        }
    }

    fn stored(&self, kind: Kind) -> Option<&Stored> {
        match kind {
            Kind::Picture => self.picture.as_ref(),
            Kind::Banner => self.banner.as_ref(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Document {
    #[serde(default)]
    people: BTreeMap<String, Profile>,
}

/// A change to the words and colour of a profile. Absent fields stay as
/// they are; `accent: Some(None)` goes back to the colour of the name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Details {
    pub pronouns: Option<String>,
    pub about: Option<String>,
    pub accent: Option<Option<Accent>>,
}

const DETAILS_SHAPE: &str = r#"body must be JSON: {"pronouns": "...", "about": "...", "accent": "teal|blue|violet|rose|amber|green" or null}, every field optional"#;

/// Read a [`Details`] out of a request body, or the sentence saying why not.
pub fn parse_details(doc: &Value) -> Result<Details, String> {
    let Some(record) = doc.as_object() else {
        return Err(DETAILS_SHAPE.into());
    };
    let text = |key: &str| -> Result<Option<String>, String> {
        match record.get(key) {
            None => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(DETAILS_SHAPE.into()),
        }
    };
    let accent = match record.get("accent") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(Value::String(s)) => match Accent::parse(s) {
            Some(a) => Some(Some(a)),
            None => return Err(format!("{s} is not one of the colours a profile can have")),
        },
        Some(_) => return Err(DETAILS_SHAPE.into()),
    };
    Ok(Details {
        pronouns: text("pronouns")?,
        about: text("about")?,
        accent,
    })
}

/// One line of text, trimmed, at most `max` characters.
fn one_line(value: &str, max: usize, what: &str) -> anyhow::Result<String> {
    let value = value.trim();
    if value.chars().count() > max {
        anyhow::bail!(BadDetails(format!("{what} are at most {max} characters")));
    }
    if value.chars().any(char::is_control) {
        anyhow::bail!(BadDetails(format!("{what} are one line of text")));
    }
    Ok(value.to_string())
}

/// Where the page reads one of `name`'s pictures.
pub fn image_url(name: &str, kind: Kind, stored: &Stored) -> String {
    format!(
        "/api/profiles/{name}/{}?v={}",
        kind.as_str(),
        stored.version
    )
}

/// A profile as every page shows it: never the email address, which the
/// person gave LosOS Login for themselves.
pub fn render(person: &crate::people::Listed, profile: &Profile) -> Value {
    let display = match person.display_name.trim() {
        "" => person.name.as_str(),
        shown => shown,
    };
    let url = |kind: Kind| {
        profile
            .stored(kind)
            .map(|s| image_url(&person.name, kind, s))
    };
    json!({
        "name": person.name,
        "displayName": display,
        "pronouns": profile.pronouns,
        "about": profile.about,
        "accent": profile.accent.unwrap_or_else(|| Accent::of_name(&person.name)).as_str(),
        "accentChosen": profile.accent.is_some(),
        "picture": url(Kind::Picture),
        "banner": url(Kind::Banner),
    })
}

// ── Pictures ──────────────────────────────────────────────────────────────

/// Decode `bytes`, turn the picture upright, crop it to `kind`'s shape from
/// the middle, scale it down to `kind`'s size if it is larger, and encode it
/// again: JPEG when every pixel is opaque, PNG when any is not. Returns the
/// type and the new bytes; nothing of the upload but its pixels survives.
pub fn normalize(kind: Kind, bytes: &[u8]) -> anyhow::Result<(&'static str, Vec<u8>)> {
    use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};

    if bytes.len() > MAX_UPLOAD_BYTES {
        anyhow::bail!(BadDetails(format!(
            "the picture is larger than {} MiB",
            MAX_UPLOAD_BYTES / (1024 * 1024)
        )));
    }
    let format = match crate::look::sniff_image(bytes) {
        Some("image/png") => ImageFormat::Png,
        Some("image/jpeg") => ImageFormat::Jpeg,
        Some("image/gif") => ImageFormat::Gif,
        Some("image/webp") => ImageFormat::WebP,
        _ => anyhow::bail!(BadDetails(
            "the picture must be a PNG, JPEG, GIF or WebP".into()
        )),
    };
    let refuse = |e: image::ImageError| match e {
        image::ImageError::Limits(_) => BadDetails(format!(
            "the picture is larger than {MAX_SOURCE_SIDE} pixels on a side"
        )),
        _ => BadDetails("the picture could not be read".into()),
    };
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_SIDE);
    limits.max_image_height = Some(MAX_SOURCE_SIDE);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(refuse)?;
    let orientation = decoder.orientation().map_err(refuse)?;
    let mut picture = DynamicImage::from_decoder(decoder).map_err(refuse)?;
    picture.apply_orientation(orientation);

    let picture = fit(&picture, kind.size());
    let opaque = !picture.color().has_alpha() || picture.to_rgba8().pixels().all(|p| p.0[3] == 255);
    let mut out = Vec::new();
    if opaque {
        let rgb = DynamicImage::ImageRgb8(picture.to_rgb8());
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY);
        rgb.write_with_encoder(encoder)?;
        Ok(("image/jpeg", out))
    } else {
        DynamicImage::ImageRgba8(picture.to_rgba8())
            .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)?;
        Ok(("image/png", out))
    }
}

/// The largest middle part of `picture` with the shape of `target`, scaled
/// down to `target` when it is larger. Never scaled up: a small picture
/// stays sharp and the page scales it.
fn fit(picture: &image::DynamicImage, target: (u32, u32)) -> image::DynamicImage {
    let (w, h) = (picture.width(), picture.height());
    let (tw, th) = (u64::from(target.0), u64::from(target.1));
    // The widest crop that has the target's shape and fits.
    let (cw, ch) = if u64::from(w) * th > u64::from(h) * tw {
        (((u64::from(h) * tw) / th).max(1) as u32, h)
    } else {
        (w, ((u64::from(w) * th) / tw).max(1) as u32)
    };
    let cropped = picture.crop_imm((w - cw) / 2, (h - ch) / 2, cw, ch);
    if cw > target.0 {
        cropped.resize_exact(target.0, target.1, image::imageops::FilterType::Lanczos3)
    } else {
        cropped
    }
}

// ── The store ─────────────────────────────────────────────────────────────

/// Every profile on the box. One lock around every read-modify-write.
pub struct Profiles {
    dir: PathBuf,
    lock: Mutex<()>,
}

impl Profiles {
    pub fn new(dir: impl Into<PathBuf>) -> Profiles {
        Profiles {
            dir: dir.into(),
            lock: Mutex::new(()),
        }
    }

    pub fn from_env() -> Profiles {
        Profiles::new(
            std::env::var_os("LOSOS_PROFILES_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(DEFAULT_DIR)),
        )
    }

    fn document(&self) -> PathBuf {
        self.dir.join("profiles.json")
    }

    fn image_path(&self, name: &str, kind: Kind) -> anyhow::Result<PathBuf> {
        // A name from the people file is always one of these; anything else
        // is refused before it can be a path.
        crate::people::validate_name(name).map_err(|e| anyhow::anyhow!(e))?;
        Ok(self.dir.join(format!("{name}.{}", kind.as_str())))
    }

    fn read(&self) -> anyhow::Result<Document> {
        match std::fs::read(self.document()) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| anyhow::anyhow!("reading {}: {e}", self.document().display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Document::default()),
            Err(e) => Err(anyhow::anyhow!(
                "reading {}: {e}",
                self.document().display()
            )),
        }
    }

    fn write_file(&self, path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::create_dir_all(&self.dir)?;
        let tmp = path.with_extension("tmp");
        {
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    fn write(&self, doc: &Document) -> anyhow::Result<()> {
        self.write_file(&self.document(), &serde_json::to_vec_pretty(doc)?)
    }

    /// `name`'s profile, or an empty one.
    pub fn get(&self, name: &str) -> anyhow::Result<Profile> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        Ok(self.read()?.people.remove(name).unwrap_or_default())
    }

    /// Every profile anyone has set, by name.
    pub fn all(&self) -> anyhow::Result<BTreeMap<String, Profile>> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        Ok(self.read()?.people)
    }

    /// Change the words and colour of `name`'s profile.
    pub fn set_details(&self, name: &str, details: &Details) -> anyhow::Result<Profile> {
        let pronouns = details
            .pronouns
            .as_deref()
            .map(|p| one_line(p, MAX_PRONOUNS_CHARS, "pronouns"))
            .transpose()?;
        let about = details
            .about
            .as_deref()
            .map(|a| one_line(a, MAX_ABOUT_CHARS, "a few words about you"))
            .transpose()?;
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let profile = doc.people.entry(name.to_string()).or_default();
        if let Some(p) = pronouns {
            profile.pronouns = p;
        }
        if let Some(a) = about {
            profile.about = a;
        }
        if let Some(accent) = details.accent {
            profile.accent = accent;
        }
        let out = profile.clone();
        self.write(&doc)?;
        Ok(out)
    }

    /// Store a new picture of `kind` for `name`, made from `bytes` by
    /// [`normalize`].
    pub fn put_image(&self, name: &str, kind: Kind, bytes: &[u8]) -> anyhow::Result<Profile> {
        let path = self.image_path(name, kind)?;
        let (content_type, encoded) = normalize(kind, bytes)?;
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let profile = doc.people.entry(name.to_string()).or_default();
        let version = profile.next_version.max(1);
        profile.next_version = version + 1;
        // The bytes first, then the document that names them: a crash in
        // between leaves a file nothing names, never a name with no file.
        self.write_file(&path, &encoded)?;
        *profile.slot(kind) = Some(Stored {
            version,
            content_type: content_type.to_string(),
        });
        let out = profile.clone();
        self.write(&doc)?;
        Ok(out)
    }

    /// Take `name`'s picture of `kind` away. Doing it twice is not an error.
    pub fn remove_image(&self, name: &str, kind: Kind) -> anyhow::Result<Profile> {
        let path = self.image_path(name, kind)?;
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let Some(profile) = doc.people.get_mut(name) else {
            return Ok(Profile::default());
        };
        if profile.slot(kind).take().is_some() {
            self.write(&doc)?;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        Ok(doc.people.get(name).cloned().unwrap_or_default())
    }

    /// The bytes and type of `name`'s picture of `kind`, if they have one.
    pub fn read_image(&self, name: &str, kind: Kind) -> anyhow::Result<Option<(String, Vec<u8>)>> {
        let path = self.image_path(name, kind)?;
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let Some(stored) = self
            .read()?
            .people
            .remove(name)
            .and_then(|mut p| p.slot(kind).take())
        else {
            return Ok(None);
        };
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some((stored.content_type, bytes))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Forget everything about `name`, for a person taken off the box.
    pub fn forget(&self, name: &str) -> anyhow::Result<()> {
        for kind in [Kind::Picture, Kind::Banner] {
            self.remove_image(name, kind)?;
        }
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        if doc.people.remove(name).is_some() {
            self.write(&doc)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, GenericImageView, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};

    fn store() -> (assert_fs::TempDir, Profiles) {
        let dir = assert_fs::TempDir::new().unwrap();
        let profiles = Profiles::new(dir.path().join("profiles"));
        (dir, profiles)
    }

    fn listed(name: &str, display: &str) -> crate::people::Listed {
        crate::people::Listed {
            name: name.into(),
            display_name: display.into(),
            email: format!("{name}@example.org"),
            groups: vec![],
            disabled: false,
        }
    }

    fn encode(picture: DynamicImage, format: ImageFormat) -> Vec<u8> {
        let mut out = Vec::new();
        picture
            .write_to(&mut Cursor::new(&mut out), format)
            .unwrap();
        out
    }

    /// A photo the way a phone writes one: a JPEG with an APP1 Exif block
    /// carrying a GPS position, stored on its side with an orientation tag
    /// that says to turn it.
    fn phone_photo(width: u32, height: u32, orientation: u16) -> Vec<u8> {
        // Left half red, right half blue, so a turn shows.
        let img = RgbImage::from_fn(width, height, |x, _| {
            if x < width / 2 {
                Rgb([200, 30, 30])
            } else {
                Rgb([30, 30, 200])
            }
        });
        let jpeg = encode(DynamicImage::ImageRgb8(img), ImageFormat::Jpeg);
        // Exif: "Exif\0\0", a little-endian TIFF header, one IFD with the
        // orientation and a GPS IFD pointer, then the GPS IFD with a
        // latitude reference and a marker string where the position goes.
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II*\0");
        tiff.extend_from_slice(&8u32.to_le_bytes());
        tiff.extend_from_slice(&2u16.to_le_bytes());
        // 0x0112 Orientation, SHORT, 1.
        tiff.extend_from_slice(&0x0112u16.to_le_bytes());
        tiff.extend_from_slice(&3u16.to_le_bytes());
        tiff.extend_from_slice(&1u32.to_le_bytes());
        tiff.extend_from_slice(&u32::from(orientation).to_le_bytes());
        // 0x8825 GPS IFD pointer, LONG, 1, at offset 38.
        tiff.extend_from_slice(&0x8825u16.to_le_bytes());
        tiff.extend_from_slice(&4u16.to_le_bytes());
        tiff.extend_from_slice(&1u32.to_le_bytes());
        tiff.extend_from_slice(&38u32.to_le_bytes());
        tiff.extend_from_slice(&0u32.to_le_bytes());
        // GPS IFD: one entry, 0x0001 GPSLatitudeRef "N".
        tiff.extend_from_slice(&1u16.to_le_bytes());
        tiff.extend_from_slice(&0x0001u16.to_le_bytes());
        tiff.extend_from_slice(&2u16.to_le_bytes());
        tiff.extend_from_slice(&2u32.to_le_bytes());
        tiff.extend_from_slice(b"N\0\0\0");
        tiff.extend_from_slice(&0u32.to_le_bytes());
        tiff.extend_from_slice(b"GPS-48.1486N-17.1077E");
        let mut app1 = b"Exif\0\0".to_vec();
        app1.extend_from_slice(&tiff);
        let mut out = jpeg[..2].to_vec();
        out.extend_from_slice(&[0xff, 0xe1]);
        out.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
        out.extend_from_slice(&app1);
        out.extend_from_slice(&jpeg[2..]);
        out
    }

    fn decode(bytes: &[u8]) -> DynamicImage {
        image::load_from_memory(bytes).unwrap()
    }

    #[test]
    fn a_phone_photo_loses_its_location_and_stands_upright() {
        let photo = phone_photo(1200, 800, 6);
        // The fixture carries what the store must drop.
        assert!(photo.windows(4).any(|w| w == b"Exif"));
        assert!(photo.windows(3).any(|w| w == b"GPS"));
        let (kind, bytes) = normalize(Kind::Picture, &photo).unwrap();
        assert_eq!(kind, "image/jpeg");
        assert!(!bytes.windows(4).any(|w| w == b"Exif"), "Exif survived");
        assert!(
            !bytes.windows(3).any(|w| w == b"GPS"),
            "the position survived"
        );
        let out = decode(&bytes);
        assert_eq!(out.dimensions(), (512, 512));
        // Orientation 6 turns the picture a quarter clockwise: the red left
        // half ends on top, the blue right half at the bottom.
        let top = out.get_pixel(256, 40).0;
        let bottom = out.get_pixel(256, 470).0;
        assert!(top[0] > 150 && top[2] < 100, "top is {top:?}");
        assert!(bottom[2] > 150 && bottom[0] < 100, "bottom is {bottom:?}");
    }

    #[test]
    fn each_kind_gets_its_shape_from_the_middle() {
        // A wide picture: a 3:1 banner keeps its width, a square picture
        // its middle.
        let wide = RgbImage::from_fn(3000, 600, |x, _| {
            if (1350..1650).contains(&x) {
                Rgb([0, 200, 0])
            } else {
                Rgb([0, 0, 0])
            }
        });
        let wide = encode(DynamicImage::ImageRgb8(wide), ImageFormat::Png);
        let picture = decode(&normalize(Kind::Picture, &wide).unwrap().1);
        assert_eq!(picture.dimensions(), (512, 512));
        assert!(
            picture.get_pixel(256, 256).0[1] > 150,
            "the middle is green"
        );
        assert!(picture.get_pixel(10, 256).0[1] < 60, "the edges are not");
        let banner = decode(&normalize(Kind::Banner, &wide).unwrap().1);
        assert_eq!(banner.dimensions(), (1500, 500));
    }

    #[test]
    fn a_small_picture_is_cropped_but_never_scaled_up() {
        let small = encode(
            DynamicImage::ImageRgb8(RgbImage::from_pixel(300, 200, Rgb([9, 9, 9]))),
            ImageFormat::Png,
        );
        assert_eq!(
            decode(&normalize(Kind::Picture, &small).unwrap().1).dimensions(),
            (200, 200)
        );
        assert_eq!(
            decode(&normalize(Kind::Banner, &small).unwrap().1).dimensions(),
            (300, 100)
        );
    }

    #[test]
    fn transparency_stays_png_and_opaque_becomes_jpeg() {
        let clear = RgbaImage::from_fn(64, 64, |x, _| {
            Rgba([10, 20, 30, if x < 32 { 0 } else { 255 }])
        });
        let (kind, bytes) = normalize(
            Kind::Picture,
            &encode(DynamicImage::ImageRgba8(clear), ImageFormat::Png),
        )
        .unwrap();
        assert_eq!(kind, "image/png");
        assert_eq!(decode(&bytes).get_pixel(5, 5).0[3], 0);
        // An alpha channel with nothing transparent in it is a photo.
        let solid = RgbaImage::from_pixel(64, 64, Rgba([10, 20, 30, 255]));
        let (kind, _) = normalize(
            Kind::Picture,
            &encode(DynamicImage::ImageRgba8(solid), ImageFormat::Png),
        )
        .unwrap();
        assert_eq!(kind, "image/jpeg");
    }

    #[test]
    fn what_is_not_a_picture_is_refused_in_words() {
        let words = |bytes: &[u8]| normalize(Kind::Picture, bytes).unwrap_err().to_string();
        assert_eq!(
            words(b"<svg xmlns='http://www.w3.org/2000/svg'/>"),
            "the picture must be a PNG, JPEG, GIF or WebP"
        );
        // The right first bytes and nothing after them.
        assert_eq!(
            words(b"\x89PNG\r\n\x1a\n0000"),
            "the picture could not be read"
        );
        assert!(words(&vec![0xff; MAX_UPLOAD_BYTES + 1]).starts_with("the picture is larger than"));
    }

    #[test]
    fn a_picture_that_claims_too_many_pixels_is_refused_before_it_is_decoded() {
        // A valid PNG header for 20000 x 20000 pixels and a few bytes of
        // data: decoding it would ask for 1.6 GB.
        let mut png = encode(
            DynamicImage::ImageRgb8(RgbImage::from_pixel(1, 1, Rgb([0, 0, 0]))),
            ImageFormat::Png,
        );
        // IHDR's width and height sit at bytes 16..24; its CRC at 29..33.
        png[16..20].copy_from_slice(&20_000u32.to_be_bytes());
        png[20..24].copy_from_slice(&20_000u32.to_be_bytes());
        let crc = crc32(&png[12..29]);
        png[29..33].copy_from_slice(&crc.to_be_bytes());
        let said = normalize(Kind::Picture, &png).unwrap_err().to_string();
        assert!(said.contains("8192 pixels"), "{said}");
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &b in bytes {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    #[test]
    fn a_fresh_person_has_a_colour_of_their_own_and_nothing_else() {
        let (_d, profiles) = store();
        let shown = render(&listed("eva", "Eva Novak"), &profiles.get("eva").unwrap());
        assert_eq!(shown["displayName"], "Eva Novak");
        assert_eq!(shown["picture"], Value::Null);
        assert_eq!(shown["banner"], Value::Null);
        assert_eq!(shown["accentChosen"], false);
        assert_eq!(shown["accent"], Accent::of_name("eva").as_str());
        assert!(
            shown.get("email").is_none(),
            "a profile never shows the address"
        );
        // Stable, and spread over the palette.
        assert_eq!(Accent::of_name("eva"), Accent::of_name("eva"));
        let names = [
            "eva", "matus", "oma", "jana", "peter", "lucia", "tomas", "zuzana",
        ];
        let spread: std::collections::BTreeSet<_> =
            names.iter().map(|n| Accent::of_name(n).as_str()).collect();
        assert!(spread.len() >= 4, "{spread:?}");
        // No display name falls back to the sign-in name.
        assert_eq!(
            render(&listed("eva", " "), &Profile::default())["displayName"],
            "eva"
        );
    }

    #[test]
    fn details_change_one_field_at_a_time() {
        let (_d, profiles) = store();
        let set = |doc: Value| profiles.set_details("eva", &parse_details(&doc).unwrap());
        set(json!({"pronouns": " she/her ", "accent": "rose"})).unwrap();
        let p = set(json!({"about": "Keeps the family photos in order."})).unwrap();
        assert_eq!(p.pronouns, "she/her");
        assert_eq!(p.accent, Some(Accent::Rose));
        assert_eq!(p.about, "Keeps the family photos in order.");
        // null goes back to the colour of the name.
        assert_eq!(set(json!({"accent": null})).unwrap().accent, None);
        assert_eq!(profiles.get("eva").unwrap().pronouns, "she/her");
    }

    #[test]
    fn details_are_read_strictly() {
        assert!(parse_details(&json!([])).is_err());
        assert!(parse_details(&json!({"pronouns": 3})).is_err());
        assert_eq!(
            parse_details(&json!({"accent": "mauve"})).unwrap_err(),
            "mauve is not one of the colours a profile can have"
        );
        let (_d, profiles) = store();
        let long = "x".repeat(MAX_ABOUT_CHARS + 1);
        let said = profiles
            .set_details(
                "eva",
                &Details {
                    about: Some(long),
                    ..Details::default()
                },
            )
            .unwrap_err();
        assert!(said.downcast_ref::<BadDetails>().is_some());
        let said = profiles
            .set_details(
                "eva",
                &Details {
                    pronouns: Some("she\nher".into()),
                    ..Details::default()
                },
            )
            .unwrap_err();
        assert_eq!(said.to_string(), "pronouns are one line of text");
        // Nothing was written by either.
        assert_eq!(profiles.get("eva").unwrap(), Profile::default());
    }

    #[test]
    fn a_replaced_or_removed_picture_never_gets_its_url_back() {
        let (_d, profiles) = store();
        let png = encode(
            DynamicImage::ImageRgb8(RgbImage::from_pixel(40, 40, Rgb([1, 2, 3]))),
            ImageFormat::Png,
        );
        let first = profiles.put_image("eva", Kind::Picture, &png).unwrap();
        let v1 = first.picture.unwrap().version;
        let banner = profiles.put_image("eva", Kind::Banner, &png).unwrap();
        assert!(banner.banner.unwrap().version > v1);
        profiles.remove_image("eva", Kind::Picture).unwrap();
        assert!(profiles.read_image("eva", Kind::Picture).unwrap().is_none());
        let again = profiles.put_image("eva", Kind::Picture, &png).unwrap();
        let v3 = again.picture.as_ref().unwrap().version;
        assert!(v3 > v1, "{v3} reuses an old version");
        let (kind, bytes) = profiles.read_image("eva", Kind::Picture).unwrap().unwrap();
        assert_eq!(kind, "image/jpeg");
        assert_eq!(decode(&bytes).dimensions(), (40, 40));
        assert_eq!(
            render(&listed("eva", "Eva"), &again)["picture"],
            format!("/api/profiles/eva/picture?v={v3}")
        );
        // Removing twice is fine, and someone with no profile has nothing.
        profiles.remove_image("eva", Kind::Picture).unwrap();
        profiles.remove_image("eva", Kind::Picture).unwrap();
        assert!(profiles.read_image("oma", Kind::Banner).unwrap().is_none());
    }

    #[test]
    fn a_refused_upload_leaves_the_old_picture() {
        let (_d, profiles) = store();
        let png = encode(
            DynamicImage::ImageRgb8(RgbImage::from_pixel(40, 40, Rgb([1, 2, 3]))),
            ImageFormat::Png,
        );
        let before = profiles.put_image("eva", Kind::Picture, &png).unwrap();
        assert!(profiles
            .put_image("eva", Kind::Picture, b"GIF89a broken")
            .is_err());
        assert_eq!(profiles.get("eva").unwrap(), before);
        assert!(profiles.read_image("eva", Kind::Picture).unwrap().is_some());
    }

    #[test]
    fn a_name_that_is_not_one_never_becomes_a_path() {
        let (_d, profiles) = store();
        assert!(profiles.read_image("../state", Kind::Picture).is_err());
        assert!(profiles
            .put_image("../../etc/x", Kind::Banner, b"")
            .is_err());
    }

    #[test]
    fn forgetting_a_person_takes_their_files_too() {
        let (dir, profiles) = store();
        let png = encode(
            DynamicImage::ImageRgb8(RgbImage::from_pixel(40, 40, Rgb([1, 2, 3]))),
            ImageFormat::Png,
        );
        profiles.put_image("eva", Kind::Picture, &png).unwrap();
        profiles.put_image("eva", Kind::Banner, &png).unwrap();
        profiles.put_image("oma", Kind::Picture, &png).unwrap();
        profiles.forget("eva").unwrap();
        let left: Vec<String> = std::fs::read_dir(dir.path().join("profiles"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert!(!left.iter().any(|f| f.starts_with("eva")), "{left:?}");
        assert!(left.contains(&"oma.picture".to_string()), "{left:?}");
        assert!(!profiles.all().unwrap().contains_key("eva"));
    }
}
