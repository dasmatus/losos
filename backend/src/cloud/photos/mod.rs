//! Photos: what each picture's camera wrote about it, and who is in it.
//!
//! A background pass (`losos-cloud photos`, [`index`], a timer in
//! `modules/cloud.nix` at the lowest CPU and disk priority) opens every
//! picture that changed once: it keeps the EXIF tags ([`exif`]) and, when the
//! box looks for faces, where each face is and its embedding ([`faces`]),
//! then groups the faces into people. The server only reads the tables, so
//! an upload never waits on the pass, and the pass's writes are short
//! transactions on the same WAL database.
//!
//! All of it is the owner's alone: the calls below answer only for the
//! signed-in person's own files, and nothing here is shared, published or
//! sent to another box. Turning faces off (`losos.cloud.photos.faces`)
//! deletes every face and person on the next pass.

pub mod exif;
pub mod faces;
pub mod index;

use crate::cloud::auth::Who;
use crate::cloud::store::{Store, StoreError};
use crate::cloud::Cloud;
use actix_web::http::{header, StatusCode};
use actix_web::{web, HttpRequest, HttpResponse};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::collections::HashMap;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS photo_meta (
  file_id INTEGER PRIMARY KEY,
  owner   TEXT    NOT NULL,
  etag    TEXT    NOT NULL,
  taken   TEXT,
  camera  TEXT,
  summary TEXT    NOT NULL,
  fields  TEXT    NOT NULL,
  -- Whether the faces of this content have been looked for.
  faces   INTEGER NOT NULL DEFAULT 0,
  -- The file would not open as a picture; tried again when it changes.
  broken  INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS photo_meta_owner ON photo_meta (owner);
CREATE TABLE IF NOT EXISTS photo_people (
  id       INTEGER PRIMARY KEY AUTOINCREMENT,
  owner    TEXT    NOT NULL,
  name     TEXT    NOT NULL DEFAULT '',
  centroid BLOB    NOT NULL
);
CREATE INDEX IF NOT EXISTS photo_people_owner ON photo_people (owner);
CREATE TABLE IF NOT EXISTS photo_faces (
  id        INTEGER PRIMARY KEY AUTOINCREMENT,
  owner     TEXT    NOT NULL,
  file_id   INTEGER NOT NULL,
  -- Left, top, width and height as fractions of the upright picture.
  x REAL NOT NULL, y REAL NOT NULL, w REAL NOT NULL, h REAL NOT NULL,
  score     REAL    NOT NULL,
  embedding BLOB    NOT NULL,
  person    INTEGER
);
CREATE INDEX IF NOT EXISTS photo_faces_owner ON photo_faces (owner);
CREATE INDEX IF NOT EXISTS photo_faces_file ON photo_faces (file_id);
CREATE INDEX IF NOT EXISTS photo_faces_person ON photo_faces (person);
"#;

/// The longest name a person can be given.
const MAX_NAME: usize = 80;
/// The side of a face's picture in the people strip.
const FACE_SIDE: u32 = 192;

pub fn init(store: &Store) -> Result<(), StoreError> {
    store.with_db(|db| Ok(db.execute_batch(SCHEMA)?))
}

/// Calls under `/cloud/api/photos`.
pub async fn api(
    req: &HttpRequest,
    cloud: &Cloud,
    who: &Who,
    segs: &[&str],
    body: web::Payload,
) -> HttpResponse {
    let store = &cloud.store;
    let login = who.login.as_str();
    let answer = |r: Result<Value, StoreError>| match r {
        Ok(v) => HttpResponse::Ok().json(v),
        Err(e) => crate::cloud::api::store_error(&e),
    };
    let not_found = || crate::cloud::api::error(StatusCode::NOT_FOUND, "notFound", "no such photo");
    match (req.method().as_str(), segs) {
        ("GET", []) => answer(overview(store, login)),
        ("GET", ["faces", id]) => match id.parse() {
            Ok(id) => face_picture(req, cloud, login, id),
            Err(_) => not_found(),
        },
        ("POST", ["people", id]) => {
            let Ok(id) = id.parse::<i64>() else {
                return not_found();
            };
            let body: Value = match crate::cloud::api::json_body(body).await {
                Ok(v) => v,
                Err(r) => return r,
            };
            let name = body.get("name").and_then(Value::as_str).unwrap_or("");
            answer(name_person(store, login, id, name).map(|id| json!({ "id": id })))
        }
        ("GET", [id]) => match id.parse() {
            Ok(id) => answer(details(store, login, id)),
            Err(_) => not_found(),
        },
        _ => crate::cloud::api::error(StatusCode::NOT_FOUND, "notFound", "no such call"),
    }
}

/// Every picture with what it was grouped by, the people, and how far the
/// pass has got.
pub fn overview(store: &Store, owner: &str) -> Result<Value, StoreError> {
    store.with_db(|db| {
        let faces_on = db
            .query_row(
                "SELECT value FROM meta WHERE key = 'photos_faces'",
                [],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .is_some_and(|v| v == "on");
        let last = db
            .query_row("SELECT value FROM meta WHERE key = 'photos_pass'", [], |r| {
                r.get::<_, String>(0)
            })
            .optional()?
            .and_then(|v| v.parse::<i64>().ok());

        let mut in_photo: HashMap<i64, Vec<i64>> = HashMap::new();
        let mut stmt = db.prepare(
            "SELECT file_id, person FROM photo_faces WHERE owner = ?1 AND person IS NOT NULL ORDER BY x",
        )?;
        for row in stmt.query_map(params![owner], |r| Ok((r.get(0)?, r.get(1)?)))? {
            let (file, person): (i64, i64) = row?;
            let list = in_photo.entry(file).or_default();
            if !list.contains(&person) {
                list.push(person);
            }
        }

        let mut photos = Vec::new();
        let (mut total, mut done) = (0u64, 0u64);
        let mut stmt = db.prepare(
            "SELECT f.id, f.path, f.name, f.mime, f.mtime, f.size, f.etag, m.etag, m.summary, m.faces \
             FROM files f LEFT JOIN photo_meta m ON m.file_id = f.id \
             WHERE f.owner = ?1 AND f.dir = 0 AND substr(f.mime, 1, 6) = 'image/' \
             ORDER BY f.mtime DESC",
        )?;
        let rows = stmt.query_map(params![owner], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<bool>>(9)?,
            ))
        })?;
        for row in rows {
            let (id, path, name, mime, mtime, size, etag, seen, summary, looked) = row?;
            if !crate::cloud::preview::can_preview(&mime) {
                continue;
            }
            total += 1;
            let current = seen.as_deref() == Some(etag.as_str());
            if current && (!faces_on || looked == Some(true)) {
                done += 1;
            }
            let mut photo = json!({
                "id": id, "path": path, "name": name, "mime": mime,
                "mtime": mtime, "size": size, "etag": etag,
                "people": in_photo.remove(&id).unwrap_or_default(),
            });
            if let Some(Ok(Value::Object(s))) =
                summary.filter(|_| current).map(|s| serde_json::from_str::<Value>(&s))
            {
                for key in ["taken", "camera", "lens", "lat", "lon", "width", "height"] {
                    if let Some(v) = s.get(key) {
                        photo[key] = v.clone();
                    }
                }
            }
            photos.push(photo);
        }

        let people = if faces_on { people(db, owner)? } else { Vec::new() };
        Ok(json!({
            "photos": photos,
            "people": people,
            "index": { "total": total, "done": done, "faces": faces_on, "last": last },
        }))
    })
}

/// The people, most photographed first, each with the face that shows them
/// best.
fn people(db: &rusqlite::Connection, owner: &str) -> Result<Vec<Value>, StoreError> {
    let mut stmt = db.prepare(
        "SELECT p.id, p.name, COUNT(DISTINCT f.file_id), \
                (SELECT b.id FROM photo_faces b WHERE b.person = p.id ORDER BY b.score * b.w * b.h DESC LIMIT 1) \
         FROM photo_people p JOIN photo_faces f ON f.person = p.id \
         WHERE p.owner = ?1 GROUP BY p.id ORDER BY (p.name = '') ASC, COUNT(DISTINCT f.file_id) DESC, p.id",
    )?;
    let rows = stmt.query_map(params![owner], |r| {
        Ok(json!({
            "id": r.get::<_, i64>(0)?,
            "name": r.get::<_, String>(1)?,
            "photos": r.get::<_, i64>(2)?,
            "face": r.get::<_, i64>(3)?,
        }))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// One picture: its summary, every tag, and its faces.
pub fn details(store: &Store, owner: &str, file_id: i64) -> Result<Value, StoreError> {
    let entry = store.by_id(owner, file_id)?;
    store.with_db(|db| {
        let meta = db
            .query_row(
                "SELECT etag, summary, fields FROM photo_meta WHERE owner = ?1 AND file_id = ?2",
                params![owner, file_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
            )
            .optional()?
            .filter(|(etag, _, _)| *etag == entry.etag);
        let Some((_, summary, fields)) = meta else {
            return Ok(json!({ "id": file_id, "indexed": false, "faces": [] }));
        };
        let mut stmt = db.prepare(
            "SELECT f.id, f.x, f.y, f.w, f.h, f.person, p.name FROM photo_faces f \
             LEFT JOIN photo_people p ON p.id = f.person WHERE f.owner = ?1 AND f.file_id = ?2 ORDER BY f.x",
        )?;
        let faces = stmt
            .query_map(params![owner, file_id], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "x": r.get::<_, f64>(1)?, "y": r.get::<_, f64>(2)?,
                    "w": r.get::<_, f64>(3)?, "h": r.get::<_, f64>(4)?,
                    "person": r.get::<_, Option<i64>>(5)?,
                    "name": r.get::<_, Option<String>>(6)?.unwrap_or_default(),
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(json!({
            "id": file_id,
            "indexed": true,
            "summary": serde_json::from_str::<Value>(&summary).unwrap_or(Value::Null),
            "fields": serde_json::from_str::<Value>(&fields).unwrap_or(Value::Null),
            "faces": faces,
        }))
    })
}

/// Give a person a name. A name another person already has makes the two
/// one person; an empty name takes it away. Returns the person's id after.
pub fn name_person(store: &Store, owner: &str, id: i64, name: &str) -> Result<i64, StoreError> {
    let name: String = name.trim().chars().filter(|c| !c.is_control()).collect();
    if name.chars().count() > MAX_NAME {
        return Err(StoreError::BadName);
    }
    store.with_db(|db| {
        let tx = db.transaction()?;
        let mine = tx
            .query_row(
                "SELECT 1 FROM photo_people WHERE id = ?1 AND owner = ?2",
                params![id, owner],
                |_| Ok(()),
            )
            .optional()?;
        if mine.is_none() {
            return Err(StoreError::NotFound);
        }
        let same = if name.is_empty() {
            None
        } else {
            tx.query_row(
                "SELECT id FROM photo_people WHERE owner = ?1 AND id != ?2 AND lower(name) = lower(?3)",
                params![owner, id, name],
                |r| r.get::<_, i64>(0),
            )
            .optional()?
        };
        let kept = match same {
            Some(other) => {
                tx.execute(
                    "UPDATE photo_faces SET person = ?1 WHERE person = ?2 AND owner = ?3",
                    params![other, id, owner],
                )?;
                tx.execute("DELETE FROM photo_people WHERE id = ?1", params![id])?;
                index::recentre(&tx, other)?;
                other
            }
            None => id,
        };
        tx.execute(
            "UPDATE photo_people SET name = ?1 WHERE id = ?2",
            params![name, kept],
        )?;
        tx.commit()?;
        Ok(kept)
    })
}

/// A face cut out of its picture, square, for the people strip. Kept beside
/// the picture's thumbnails, under the same content tag.
fn face_picture(req: &HttpRequest, cloud: &Cloud, owner: &str, id: i64) -> HttpResponse {
    let store = &cloud.store;
    let face = store.with_db(|db| {
        Ok(db
            .query_row(
                "SELECT file_id, x, y, w, h FROM photo_faces WHERE id = ?1 AND owner = ?2",
                params![id, owner],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        [r.get::<_, f64>(1)?, r.get(2)?, r.get(3)?, r.get(4)?],
                    ))
                },
            )
            .optional()?)
    });
    let (file_id, rect) = match face {
        Ok(Some(f)) => f,
        Ok(None) => return crate::cloud::api::store_error(&StoreError::NotFound),
        Err(e) => return crate::cloud::api::store_error(&e),
    };
    let entry = match store.by_id(owner, file_id) {
        Ok(e) => e,
        Err(e) => return crate::cloud::api::store_error(&e),
    };
    let tag = format!("\"f{id}-{}\"", entry.etag);
    if req
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains(&tag))
    {
        return HttpResponse::NotModified().finish();
    }
    // `<size>-<etag>.jpg` is how the thumbnails name theirs, so a new
    // content clears this one with them (`preview::forget_older`).
    let dir = store.root().join(".previews").join(file_id.to_string());
    let cached = dir.join(format!("f{id}-{}.jpg", entry.etag));
    let bytes = match std::fs::read(&cached) {
        Ok(b) => b,
        Err(_) => match cut_face(&store.disk_path(owner, &entry.path), rect) {
            Ok(b) => {
                if std::fs::create_dir_all(&dir).is_ok() {
                    let _ = std::fs::write(&cached, &b);
                }
                b
            }
            Err(e) => {
                tracing::debug!("cloud: no face {id}: {e}");
                return crate::cloud::api::store_error(&StoreError::NotFound);
            }
        },
    };
    HttpResponse::Ok()
        .content_type("image/jpeg")
        .insert_header((header::ETAG, tag))
        .insert_header((header::CACHE_CONTROL, "private, max-age=86400"))
        .body(bytes)
}

/// The square around a face with some of the head and shoulders, as JPEG.
fn cut_face(src: &std::path::Path, rect: [f64; 4]) -> Result<Vec<u8>, String> {
    let (img, _) = crate::cloud::preview::open_upright(src)?;
    let (w, h) = (img.width() as f64, img.height() as f64);
    let side = (rect[2] * w).max(rect[3] * h) * 1.6;
    let side = side.min(w).min(h).max(1.0);
    let cx = (rect[0] + rect[2] / 2.0) * w;
    let cy = (rect[1] + rect[3] / 2.0) * h;
    let x = (cx - side / 2.0).clamp(0.0, w - side);
    let y = (cy - side / 2.0).clamp(0.0, h - side);
    let face = img
        .crop_imm(x as u32, y as u32, side as u32, side as u32)
        .resize_exact(FACE_SIDE, FACE_SIDE, image::imageops::FilterType::Triangle);
    let mut out = std::io::Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85)
        .encode_image(&image::DynamicImage::ImageRgb8(face.to_rgb8()))
        .map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, Store) {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(d.path()).unwrap();
        init(&s).unwrap();
        s.ensure_home("ana").unwrap();
        (d, s)
    }

    fn person(s: &Store, owner: &str, name: &str, v: &[f32], file: i64) -> i64 {
        s.with_db(|db| {
            db.execute(
                "INSERT INTO photo_people (owner, name, centroid) VALUES (?1, ?2, ?3)",
                params![owner, name, faces::to_blob(v)],
            )?;
            let id = db.last_insert_rowid();
            db.execute(
                "INSERT INTO photo_faces (owner, file_id, x, y, w, h, score, embedding, person) \
                 VALUES (?1, ?2, 0.1, 0.1, 0.2, 0.2, 0.9, ?3, ?4)",
                params![owner, file, faces::to_blob(v), id],
            )?;
            Ok(id)
        })
        .unwrap()
    }

    #[test]
    fn the_same_name_makes_two_people_one() {
        let (_d, s) = store();
        let a = person(&s, "ana", "", &[1.0, 0.0], 1);
        let b = person(&s, "ana", "Bea", &[0.0, 1.0], 2);
        assert_eq!(name_person(&s, "ana", a, "  bea ").unwrap(), b);
        let (people, on_b): (i64, i64) = s
            .with_db(|db| {
                Ok((
                    db.query_row("SELECT COUNT(*) FROM photo_people", [], |r| r.get(0))?,
                    db.query_row(
                        "SELECT COUNT(*) FROM photo_faces WHERE person = ?1",
                        [b],
                        |r| r.get(0),
                    )?,
                ))
            })
            .unwrap();
        assert_eq!((people, on_b), (1, 2));
    }

    #[test]
    fn nobody_names_another_persons_people() {
        let (_d, s) = store();
        let a = person(&s, "ana", "", &[1.0, 0.0], 1);
        assert!(matches!(
            name_person(&s, "eve", a, "Mallory"),
            Err(StoreError::NotFound)
        ));
        assert!(matches!(
            name_person(&s, "ana", a, &"x".repeat(81)),
            Err(StoreError::BadName)
        ));
        assert_eq!(name_person(&s, "ana", a, "Ana\u{7}").unwrap(), a);
    }

    #[test]
    fn a_face_is_cut_square_inside_the_picture() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("a.png");
        image::RgbImage::from_pixel(400, 300, image::Rgb([200, 10, 10]))
            .save(&src)
            .unwrap();
        // A face at the very edge still gives a whole square.
        let jpg = cut_face(&src, [0.9, 0.0, 0.1, 0.2]).unwrap();
        let back = image::load_from_memory(&jpg).unwrap();
        assert_eq!((back.width(), back.height()), (FACE_SIDE, FACE_SIDE));
    }
}
