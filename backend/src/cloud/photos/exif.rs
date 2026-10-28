//! What a camera wrote about a picture: every tag of its EXIF block, and the
//! few the photo views sort and group by (when it was taken, the camera, the
//! lens, where).
//!
//! The block comes from the image decoder ([`crate::cloud::preview::open_upright`]),
//! so a picture is opened once for its tags, its size and its faces. The
//! maker's private notes and the embedded thumbnail are left out: they are
//! long, binary and say nothing a person reads.

use exif::{Context, Exif, In, Tag, Value};
use serde::{Deserialize, Serialize};

/// The longest value kept for one tag.
const MAX_VALUE: usize = 200;

/// The parts of the tags the views use, worded for people.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// As the camera's clock read it, `2019-06-08T10:12:31`, with no zone:
    /// cameras rarely know theirs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taken: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lens: Option<String>,
    /// `1/250 s`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exposure: Option<String>,
    /// `f/2.8`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aperture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iso: Option<u32>,
    /// `50 mm`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lat: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lon: Option<f64>,
    /// Metres above sea level; below it is negative.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub altitude: Option<f64>,
    /// The picture upright, in pixels.
    pub width: u32,
    pub height: u32,
}

/// One tag as a person reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    /// `picture`, `camera`, `place` or `other`.
    pub group: String,
    pub name: String,
    pub value: String,
}

/// A picture's summary and every tag, from its raw EXIF block (none for a
/// picture without one) and its size upright.
pub fn read(raw: Option<&[u8]>, width: u32, height: u32) -> (Summary, Vec<Field>) {
    let mut summary = Summary {
        width,
        height,
        ..Summary::default()
    };
    let Some(exif) = raw.and_then(parse) else {
        return (summary, Vec::new());
    };
    let text = |tag| field_text(&exif, tag);
    summary.taken = [Tag::DateTimeOriginal, Tag::DateTimeDigitized, Tag::DateTime]
        .into_iter()
        .find_map(|t| text(t).as_deref().and_then(iso_time));
    summary.camera = camera(text(Tag::Make), text(Tag::Model));
    summary.lens = text(Tag::LensModel).filter(|l| !l.is_empty());
    summary.exposure = rational(&exif, Tag::ExposureTime).map(|t| {
        if t > 0.0 && t < 1.0 {
            format!("1/{} s", (1.0 / t).round())
        } else {
            format!("{} s", trim(t))
        }
    });
    summary.aperture = rational(&exif, Tag::FNumber).map(|f| format!("f/{}", trim(f)));
    summary.iso = exif
        .get_field(Tag::PhotographicSensitivity, In::PRIMARY)
        .and_then(|f| f.value.get_uint(0));
    summary.focal = rational(&exif, Tag::FocalLength).map(|f| format!("{} mm", trim(f)));
    summary.lat = coordinate(&exif, Tag::GPSLatitude, Tag::GPSLatitudeRef, b'S');
    summary.lon = coordinate(&exif, Tag::GPSLongitude, Tag::GPSLongitudeRef, b'W');
    summary.altitude = rational(&exif, Tag::GPSAltitude).map(|a| {
        let below = exif
            .get_field(Tag::GPSAltitudeRef, In::PRIMARY)
            .and_then(|f| f.value.get_uint(0))
            == Some(1);
        if below {
            -a
        } else {
            a
        }
    });
    if summary.lat.is_none() || summary.lon.is_none() {
        summary.lat = None;
        summary.lon = None;
    }

    let mut fields = Vec::new();
    for f in exif.fields() {
        if f.ifd_num != In::PRIMARY || skipped(f.tag) {
            continue;
        }
        if matches!(&f.value, Value::Undefined(b, _) if b.len() > 64) {
            continue;
        }
        let value = f.display_value().with_unit(&exif).to_string();
        let value = value.trim().trim_matches('"').trim();
        if value.is_empty() {
            continue;
        }
        let group = match f.tag.context() {
            Context::Gps => "place",
            Context::Tiff if matches!(f.tag, Tag::Make | Tag::Model) => "camera",
            Context::Tiff => "picture",
            Context::Exif => "camera",
            _ => "other",
        };
        fields.push(Field {
            group: group.into(),
            name: f.tag.to_string(),
            value: shorten(value),
        });
    }
    (summary, fields)
}

fn parse(raw: &[u8]) -> Option<Exif> {
    // JPEG's APP1 starts with this; the TIFF data follows.
    let raw = raw.strip_prefix(b"Exif\0\0").unwrap_or(raw);
    exif::Reader::new().read_raw(raw.to_vec()).ok()
}

/// Tags left out: binary blobs, pointers into the file, the thumbnail's.
fn skipped(tag: Tag) -> bool {
    matches!(
        tag,
        Tag::MakerNote
            | Tag::JPEGInterchangeFormat
            | Tag::JPEGInterchangeFormatLength
            | Tag::ExifVersion
            | Tag::FlashpixVersion
            | Tag::InteroperabilityIndex
            | Tag::InteroperabilityVersion
            | Tag::ComponentsConfiguration
            | Tag::StripOffsets
            | Tag::StripByteCounts
    )
}

fn field_text(exif: &Exif, tag: Tag) -> Option<String> {
    let f = exif.get_field(tag, In::PRIMARY)?;
    match &f.value {
        Value::Ascii(parts) => {
            let s: Vec<String> = parts
                .iter()
                .map(|p| {
                    String::from_utf8_lossy(p)
                        .trim_matches(char::from(0))
                        .trim()
                        .to_string()
                })
                .filter(|p| !p.is_empty())
                .collect();
            (!s.is_empty()).then(|| s.join(" "))
        }
        _ => Some(f.display_value().to_string()),
    }
}

fn rational(exif: &Exif, tag: Tag) -> Option<f64> {
    match &exif.get_field(tag, In::PRIMARY)?.value {
        Value::Rational(v) => v.first().filter(|r| r.denom != 0).map(|r| r.to_f64()),
        Value::SRational(v) => v.first().filter(|r| r.denom != 0).map(|r| r.to_f64()),
        _ => None,
    }
    .filter(|v| v.is_finite())
}

/// Degrees, minutes and seconds with their hemisphere, as one signed number.
fn coordinate(exif: &Exif, tag: Tag, reference: Tag, negative: u8) -> Option<f64> {
    let Value::Rational(parts) = &exif.get_field(tag, In::PRIMARY)?.value else {
        return None;
    };
    if parts.len() < 3 || parts.iter().any(|r| r.denom == 0) {
        return None;
    }
    let deg = parts[0].to_f64() + parts[1].to_f64() / 60.0 + parts[2].to_f64() / 3600.0;
    let south_or_west = match &exif.get_field(reference, In::PRIMARY)?.value {
        Value::Ascii(v) => v.first().and_then(|s| s.first()) == Some(&negative),
        _ => false,
    };
    let deg = if south_or_west { -deg } else { deg };
    (deg.is_finite() && deg.abs() <= 180.0).then_some(deg)
}

/// `2019:06:08 10:12:31` as `2019-06-08T10:12:31`, or nothing for a clock
/// that was never set.
fn iso_time(s: &str) -> Option<String> {
    let b = s.trim().as_bytes();
    if b.len() < 19
        || !b[..19].iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b':' || *c == b'-',
            10 => *c == b' ' || *c == b'T',
            13 | 16 => *c == b':',
            _ => c.is_ascii_digit(),
        })
    {
        return None;
    }
    let s = &s.trim()[..19];
    if s.starts_with("0000") {
        return None;
    }
    Some(format!(
        "{}-{}-{}T{}",
        &s[0..4],
        &s[5..7],
        &s[8..10],
        &s[11..19]
    ))
}

/// "Canon" and "Canon EOS R6" are one camera, not "Canon Canon EOS R6".
fn camera(make: Option<String>, model: Option<String>) -> Option<String> {
    match (make, model) {
        (Some(make), Some(model)) => {
            let first = make.split_whitespace().next().unwrap_or("").to_lowercase();
            if !first.is_empty() && model.to_lowercase().starts_with(&first) {
                Some(model)
            } else {
                Some(format!("{make} {model}"))
            }
        }
        (Some(one), None) | (None, Some(one)) => Some(one),
        (None, None) => None,
    }
}

fn trim(v: f64) -> String {
    let s = format!("{v:.1}");
    s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
}

fn shorten(v: &str) -> String {
    if v.chars().count() <= MAX_VALUE {
        v.to_string()
    } else {
        let mut s: String = v.chars().take(MAX_VALUE).collect();
        s.push('…');
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exif::experimental::Writer;
    use exif::{Field as ExifField, Rational};

    fn ascii(tag: Tag, s: &str) -> ExifField {
        ExifField {
            tag,
            ifd_num: In::PRIMARY,
            value: Value::Ascii(vec![s.as_bytes().to_vec()]),
        }
    }

    fn rationals(tag: Tag, v: &[(u32, u32)]) -> ExifField {
        ExifField {
            tag,
            ifd_num: In::PRIMARY,
            value: Value::Rational(
                v.iter()
                    .map(|&(num, denom)| Rational { num, denom })
                    .collect(),
            ),
        }
    }

    fn block(fields: &[ExifField]) -> Vec<u8> {
        let mut w = Writer::new();
        for f in fields {
            w.push_field(f);
        }
        let mut out = std::io::Cursor::new(Vec::new());
        w.write(&mut out, false).unwrap();
        out.into_inner()
    }

    #[test]
    fn a_cameras_tags_become_a_summary_and_a_list() {
        let raw = block(&[
            ascii(Tag::Make, "NIKON CORPORATION"),
            ascii(Tag::Model, "NIKON D5"),
            ascii(Tag::DateTimeOriginal, "2019:06:08 10:12:31"),
            rationals(Tag::ExposureTime, &[(1, 250)]),
            rationals(Tag::FNumber, &[(28, 10)]),
            rationals(Tag::FocalLength, &[(50, 1)]),
            ascii(Tag::GPSLatitudeRef, "N"),
            rationals(Tag::GPSLatitude, &[(48, 1), (8, 1), (36, 1)]),
            ascii(Tag::GPSLongitudeRef, "E"),
            rationals(Tag::GPSLongitude, &[(17, 1), (6, 1), (0, 1)]),
        ]);
        let (s, fields) = read(Some(&raw), 4000, 3000);
        assert_eq!(s.taken.as_deref(), Some("2019-06-08T10:12:31"));
        assert_eq!(s.camera.as_deref(), Some("NIKON D5"));
        assert_eq!(s.exposure.as_deref(), Some("1/250 s"));
        assert_eq!(s.aperture.as_deref(), Some("f/2.8"));
        assert_eq!(s.focal.as_deref(), Some("50 mm"));
        assert!((s.lat.unwrap() - 48.143_333).abs() < 1e-4);
        assert!((s.lon.unwrap() - 17.1).abs() < 1e-4);
        assert_eq!((s.width, s.height), (4000, 3000));
        let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
        assert!(
            names.contains(&"Make") && names.contains(&"GPSLatitude"),
            "{names:?}"
        );
        let gps = fields.iter().find(|f| f.name == "GPSLatitude").unwrap();
        assert_eq!(gps.group, "place");
    }

    #[test]
    fn a_picture_without_tags_still_has_its_size() {
        let (s, fields) = read(None, 640, 480);
        assert_eq!((s.width, s.height, s.taken), (640, 480, None));
        assert!(fields.is_empty());
        let (s, _) = read(Some(b"not exif"), 1, 1);
        assert_eq!(s.camera, None);
    }

    #[test]
    fn half_a_position_or_an_unset_clock_is_nothing() {
        let raw = block(&[
            ascii(Tag::DateTimeOriginal, "0000:00:00 00:00:00"),
            ascii(Tag::GPSLatitudeRef, "S"),
            rationals(Tag::GPSLatitude, &[(33, 1), (52, 1), (0, 1)]),
        ]);
        let (s, _) = read(Some(&raw), 1, 1);
        assert_eq!((s.taken, s.lat, s.lon), (None, None, None));
    }

    #[test]
    fn the_make_is_said_once() {
        assert_eq!(
            camera(Some("Canon".into()), Some("Canon EOS R6".into())).as_deref(),
            Some("Canon EOS R6")
        );
        assert_eq!(
            camera(Some("Apple".into()), Some("iPhone 15".into())).as_deref(),
            Some("Apple iPhone 15")
        );
    }
}
