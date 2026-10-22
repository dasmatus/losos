//! The background pass: every picture that is new or changed since the last
//! one is opened once for its tags and, with the models, its faces; then the
//! faces nobody has been put with yet are grouped into people.
//!
//! Grouping keeps what it has decided. A face joins the person whose average
//! face it is closest to, when that is close enough ([`faces::SAME_PERSON`]),
//! or starts a person of its own; then two unnamed people whose averages have
//! come close ([`MERGE`]) become one. Two people with different names are
//! never joined: the owner told them apart.

use super::exif;
use super::faces::{self, Models};
use crate::cloud::preview;
use crate::cloud::store::{Store, StoreError};
use rusqlite::{params, Connection, OptionalExtension, Transaction};

/// Two people's average faces at least this close are one person.
pub const MERGE: f32 = 0.55;
/// Pictures larger than this are shrunk before looking for faces: the
/// detector sees 640 pixels anyway, and it bounds the pass's memory.
const MAX_SIDE: u32 = 2048;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub photos: usize,
    pub faces: usize,
    pub broken: usize,
    pub people: usize,
}

/// Where the face models are, loaded the first time a picture needs them:
/// a pass with nothing to read costs a query, not 40 MB of model.
pub enum Faces<'a> {
    Off,
    In(&'a std::path::Path),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("the face models in {0}: {1}")]
    Models(String, String),
}

/// One pass over `owners`' pictures. With faces off, faces are not looked
/// for and every face and person is deleted.
pub fn run(store: &Store, faces: Faces, owners: &[String]) -> Result<Report, Error> {
    let mut report = Report::default();
    let on = !matches!(faces, Faces::Off);
    let mut loaded: Option<Models> = None;
    store.with_db(|db| {
        db.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('photos_faces', ?1)",
            params![if on { "on" } else { "off" }],
        )?;
        if !on {
            db.execute_batch(
                "DELETE FROM photo_faces; DELETE FROM photo_people; UPDATE photo_meta SET faces = 0;",
            )?;
        }
        Ok(())
    })?;
    for owner in owners {
        prune(store, owner)?;
        for (id, path, etag) in todo(store, owner, on)? {
            let models = match &faces {
                Faces::Off => None,
                Faces::In(dir) => {
                    if loaded.is_none() {
                        loaded = Some(Models::load(dir).map_err(|e| {
                            Error::Models(dir.display().to_string(), e.to_string())
                        })?);
                    }
                    loaded.as_ref()
                }
            };
            let src = store.disk_path(owner, &path);
            match look(&src, models) {
                Ok(seen) => {
                    report.photos += 1;
                    report.faces += seen.faces.len();
                    save(store, owner, id, &etag, Some(&seen), on)?;
                }
                Err(e) => {
                    tracing::info!("photos: {owner}/{path}: {e}");
                    report.broken += 1;
                    save(store, owner, id, &etag, None, on)?;
                }
            }
        }
        if on {
            report.people += store.with_db(|db| {
                let tx = db.transaction()?;
                let n = group(&tx, owner)?;
                tx.commit()?;
                Ok(n)
            })?;
        }
    }
    store.with_db(|db| {
        db.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('photos_pass', ?1)",
            params![chrono_now().to_string()],
        )?;
        Ok(())
    })?;
    Ok(report)
}

fn chrono_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Forget pictures that are gone, and people left with no face.
fn prune(store: &Store, owner: &str) -> Result<(), StoreError> {
    store.with_db(|db| {
        let tx = db.transaction()?;
        tx.execute(
            "DELETE FROM photo_meta WHERE owner = ?1 AND file_id NOT IN (SELECT id FROM files WHERE owner = ?1)",
            params![owner],
        )?;
        tx.execute(
            "DELETE FROM photo_faces WHERE owner = ?1 AND file_id NOT IN (SELECT file_id FROM photo_meta WHERE owner = ?1)",
            params![owner],
        )?;
        let changed: Vec<i64> = {
            let mut stmt = tx.prepare(
                "SELECT id FROM photo_people WHERE owner = ?1 AND id NOT IN \
                 (SELECT DISTINCT person FROM photo_faces WHERE owner = ?1 AND person IS NOT NULL)",
            )?;
            let rows = stmt.query_map(params![owner], |r| r.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        for id in changed {
            tx.execute("DELETE FROM photo_people WHERE id = ?1", params![id])?;
        }
        tx.commit()?;
        Ok(())
    })
}

/// The pictures to open: new, changed, or not yet looked at for faces.
fn todo(store: &Store, owner: &str, faces: bool) -> Result<Vec<(i64, String, String)>, StoreError> {
    store.with_db(|db| {
        let mut stmt = db.prepare(
            "SELECT f.id, f.path, f.etag, f.mime FROM files f LEFT JOIN photo_meta m ON m.file_id = f.id \
             WHERE f.owner = ?1 AND f.dir = 0 AND substr(f.mime, 1, 6) = 'image/' \
               AND (m.etag IS NULL OR m.etag != f.etag OR (?2 AND m.faces = 0 AND m.broken = 0)) \
             ORDER BY f.mtime DESC",
        )?;
        let rows = stmt.query_map(params![owner, faces], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, String>(3)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, path, etag, mime) = row?;
            if preview::can_preview(&mime) {
                out.push((id, path, etag));
            }
        }
        Ok(out)
    })
}

/// What one picture holds.
pub struct Seen {
    pub summary: exif::Summary,
    pub fields: Vec<exif::Field>,
    /// Where each face is, as fractions, its score and its embedding.
    pub faces: Vec<([f32; 4], f32, Vec<f32>)>,
}

/// Open a picture and read it. The slow part, done with no lock held.
pub fn look(src: &std::path::Path, models: Option<&Models>) -> Result<Seen, String> {
    let (img, raw) = preview::open_upright(src)?;
    let (summary, fields) = exif::read(raw.as_deref(), img.width(), img.height());
    let mut found = Vec::new();
    if let Some(models) = models {
        let img = if img.width().max(img.height()) > MAX_SIDE {
            img.thumbnail(MAX_SIDE, MAX_SIDE)
        } else {
            img
        };
        let rgb = img.to_rgb8();
        let (w, h) = (rgb.width() as f32, rgb.height() as f32);
        for face in models.detect(&rgb).map_err(|e| e.to_string())? {
            let embedding = models.embed(&rgb, &face).map_err(|e| e.to_string())?;
            let [x, y, fw, fh] = face.rect;
            let (x0, y0) = (x.max(0.0), y.max(0.0));
            let rect = [
                x0 / w,
                y0 / h,
                ((x + fw).min(w) - x0).max(0.0) / w,
                ((y + fh).min(h) - y0).max(0.0) / h,
            ];
            found.push((rect, face.score, embedding));
        }
    }
    Ok(Seen {
        summary,
        fields,
        faces: found,
    })
}

/// Write what a picture holds over what an older content held. `None` marks
/// it broken, so it waits for a new content before it is tried again.
fn save(
    store: &Store,
    owner: &str,
    id: i64,
    etag: &str,
    seen: Option<&Seen>,
    faces: bool,
) -> Result<(), StoreError> {
    store.with_db(|db| {
        let tx = db.transaction()?;
        // The file may have changed or gone while it was being read; the
        // next pass sees the new content.
        let current = tx
            .query_row(
                "SELECT etag FROM files WHERE id = ?1 AND owner = ?2",
                params![id, owner],
                |r| r.get::<_, String>(0),
            )
            .optional()?;
        if current.as_deref() != Some(etag) {
            return Ok(());
        }
        tx.execute("DELETE FROM photo_faces WHERE file_id = ?1", params![id])?;
        let (summary, fields) = match seen {
            Some(s) => (
                serde_json::to_string(&s.summary).unwrap_or_default(),
                serde_json::to_string(&s.fields).unwrap_or_default(),
            ),
            None => ("{}".to_string(), "[]".to_string()),
        };
        tx.execute(
            "INSERT OR REPLACE INTO photo_meta (file_id, owner, etag, taken, camera, summary, fields, faces, broken) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                owner,
                etag,
                seen.and_then(|s| s.summary.taken.clone()),
                seen.and_then(|s| s.summary.camera.clone()),
                summary,
                fields,
                faces,
                seen.is_none(),
            ],
        )?;
        for (rect, score, embedding) in seen.map(|s| s.faces.as_slice()).unwrap_or_default() {
            tx.execute(
                "INSERT INTO photo_faces (owner, file_id, x, y, w, h, score, embedding) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    owner,
                    id,
                    rect[0],
                    rect[1],
                    rect[2],
                    rect[3],
                    score,
                    faces::to_blob(embedding)
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    })
}

struct Person {
    id: i64,
    named: bool,
    /// The sum of its faces' embeddings; its direction is the average face.
    sum: Vec<f32>,
}

impl Person {
    fn centre(&self) -> Vec<f32> {
        faces::normalised(self.sum.clone())
    }
}

fn add(sum: &mut [f32], v: &[f32]) {
    sum.iter_mut().zip(v).for_each(|(s, x)| *s += x);
}

fn sum_of(tx: &Connection, person: i64) -> Result<Vec<f32>, StoreError> {
    let mut stmt = tx.prepare("SELECT embedding FROM photo_faces WHERE person = ?1")?;
    let mut sum: Vec<f32> = Vec::new();
    for blob in stmt.query_map(params![person], |r| r.get::<_, Vec<u8>>(0))? {
        let v = faces::from_blob(&blob?);
        if sum.is_empty() {
            sum = vec![0.0; v.len()];
        }
        add(&mut sum, &v);
    }
    Ok(sum)
}

/// Store a person's average face again from the faces it has now.
pub fn recentre(tx: &Connection, person: i64) -> Result<(), StoreError> {
    let sum = sum_of(tx, person)?;
    tx.execute(
        "UPDATE photo_people SET centroid = ?1 WHERE id = ?2",
        params![faces::to_blob(&faces::normalised(sum)), person],
    )?;
    Ok(())
}

/// Put every face that has no person with one, then join unnamed people who
/// turn out to be one. Returns how many people there are.
pub fn group(tx: &Transaction, owner: &str) -> Result<usize, StoreError> {
    let mut people: Vec<Person> = {
        let mut stmt =
            tx.prepare("SELECT id, name FROM photo_people WHERE owner = ?1 ORDER BY id")?;
        let rows = stmt.query_map(params![owner], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    }
    .into_iter()
    .map(|(id, name)| {
        Ok(Person {
            id,
            named: !name.is_empty(),
            sum: sum_of(tx, id)?,
        })
    })
    .collect::<Result<_, StoreError>>()?;

    let loose: Vec<(i64, Vec<f32>)> = {
        let mut stmt = tx.prepare(
            "SELECT id, embedding FROM photo_faces WHERE owner = ?1 AND person IS NULL ORDER BY id",
        )?;
        let rows = stmt.query_map(params![owner], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                faces::from_blob(&r.get::<_, Vec<u8>>(1)?),
            ))
        })?;
        rows.collect::<Result<_, _>>()?
    };
    for (face, v) in loose {
        let best = people
            .iter_mut()
            .filter(|p| p.sum.len() == v.len())
            .map(|p| (faces::cosine(&p.centre(), &v), p))
            .max_by(|a, b| a.0.total_cmp(&b.0));
        let person = match best {
            Some((c, p)) if c >= faces::SAME_PERSON => {
                add(&mut p.sum, &v);
                p.id
            }
            _ => {
                tx.execute(
                    "INSERT INTO photo_people (owner, centroid) VALUES (?1, ?2)",
                    params![owner, faces::to_blob(&v)],
                )?;
                let id = tx.last_insert_rowid();
                people.push(Person {
                    id,
                    named: false,
                    sum: v,
                });
                id
            }
        };
        tx.execute(
            "UPDATE photo_faces SET person = ?1 WHERE id = ?2",
            params![person, face],
        )?;
    }

    // Join the closest pair that may be joined, again and again.
    loop {
        let centres: Vec<Vec<f32>> = people.iter().map(Person::centre).collect();
        let mut best: Option<(f32, usize, usize)> = None;
        for i in 0..people.len() {
            for j in i + 1..people.len() {
                if people[i].named && people[j].named {
                    continue;
                }
                let c = faces::cosine(&centres[i], &centres[j]);
                if c >= MERGE && best.is_none_or(|(b, _, _)| c > b) {
                    best = Some((c, i, j));
                }
            }
        }
        let Some((_, i, j)) = best else { break };
        // The named one stays; else the older.
        let (keep, gone) = if people[j].named { (j, i) } else { (i, j) };
        let keep_id = people[keep].id;
        let gone = people.remove(gone);
        let Some(keep) = people.iter().position(|p| p.id == keep_id) else {
            break;
        };
        add(&mut people[keep].sum, &gone.sum);
        tx.execute(
            "UPDATE photo_faces SET person = ?1 WHERE person = ?2",
            params![people[keep].id, gone.id],
        )?;
        tx.execute("DELETE FROM photo_people WHERE id = ?1", params![gone.id])?;
    }

    for p in &people {
        tx.execute(
            "UPDATE photo_people SET centroid = ?1 WHERE id = ?2",
            params![faces::to_blob(&p.centre()), p.id],
        )?;
    }
    Ok(people.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, Store) {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(d.path()).unwrap();
        super::super::init(&s).unwrap();
        s.ensure_home("ana").unwrap();
        (d, s)
    }

    fn face(s: &Store, file: i64, v: &[f32]) {
        s.with_db(|db| {
            db.execute(
                "INSERT INTO photo_faces (owner, file_id, x, y, w, h, score, embedding) \
                 VALUES ('ana', ?1, 0, 0, 0.1, 0.1, 0.9, ?2)",
                params![file, faces::to_blob(&faces::normalised(v.to_vec()))],
            )?;
            Ok(())
        })
        .unwrap();
    }

    fn groups(s: &Store) -> Vec<Vec<i64>> {
        s.with_db(|db| {
            let mut stmt = db.prepare(
                "SELECT group_concat(file_id) FROM (SELECT file_id, person FROM photo_faces ORDER BY file_id) GROUP BY person ORDER BY min(file_id)",
            )?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            Ok(rows
                .map(|r| r.unwrap().split(',').map(|n| n.parse().unwrap()).collect())
                .collect())
        })
        .unwrap()
    }

    fn regroup(s: &Store) -> usize {
        s.with_db(|db| {
            let tx = db.transaction()?;
            let n = group(&tx, "ana")?;
            tx.commit()?;
            Ok(n)
        })
        .unwrap()
    }

    #[test]
    fn faces_close_together_are_one_person() {
        let (_d, s) = store();
        face(&s, 1, &[1.0, 0.1, 0.0]);
        face(&s, 2, &[0.0, 1.0, 0.0]);
        face(&s, 3, &[0.9, 0.2, 0.1]);
        face(&s, 4, &[0.1, 0.9, 0.2]);
        face(&s, 5, &[0.0, 0.0, 1.0]);
        assert_eq!(regroup(&s), 3);
        assert_eq!(groups(&s), vec![vec![1, 3], vec![2, 4], vec![5]]);
        // A later face joins whoever it looks like; the rest stay put.
        face(&s, 6, &[0.95, 0.0, 0.05]);
        assert_eq!(regroup(&s), 3);
        assert_eq!(groups(&s), vec![vec![1, 3, 6], vec![2, 4], vec![5]]);
    }

    #[test]
    fn unnamed_people_who_drift_together_become_one_but_named_ones_never_do() {
        let (_d, s) = store();
        // Far enough apart to start as two, close enough as averages once a
        // face between them arrives.
        face(&s, 1, &[1.0, 0.0, 0.0]);
        face(&s, 2, &[0.45, 0.9, 0.0]);
        assert_eq!(regroup(&s), 2);
        face(&s, 3, &[0.8, 0.6, 0.0]);
        face(&s, 4, &[0.7, 0.7, 0.0]);
        assert_eq!(regroup(&s), 1);

        let (_d, s) = store();
        face(&s, 1, &[1.0, 0.0, 0.0]);
        face(&s, 2, &[0.45, 0.9, 0.0]);
        regroup(&s);
        s.with_db(|db| Ok(db.execute("UPDATE photo_people SET name = 'P' || id", [])?))
            .unwrap();
        face(&s, 3, &[0.8, 0.6, 0.0]);
        face(&s, 4, &[0.7, 0.7, 0.0]);
        assert_eq!(regroup(&s), 2);
    }

    #[test]
    fn a_pass_reads_tags_and_forgets_deleted_pictures() {
        let (_d, s) = store();
        let png = {
            let mut out = std::io::Cursor::new(Vec::new());
            image::RgbImage::from_pixel(30, 20, image::Rgb([1, 2, 3]))
                .write_to(&mut out, image::ImageFormat::Png)
                .unwrap();
            out.into_inner()
        };
        s.write_bytes("ana", "a.png", &png).unwrap();
        s.write_bytes("ana", "b.jpg", b"not a picture").unwrap();
        s.write_bytes("ana", "c.txt", b"text").unwrap();
        let r = run(&s, Faces::Off, &["ana".into()]).unwrap();
        assert_eq!((r.photos, r.broken), (1, 1));
        // Nothing changed, nothing to do.
        assert_eq!(
            run(&s, Faces::Off, &["ana".into()]).unwrap(),
            Report::default()
        );

        let o = super::super::overview(&s, "ana").unwrap();
        assert_eq!(o["index"]["total"], 2);
        assert_eq!(o["index"]["done"], 2);
        assert_eq!(o["index"]["faces"], false);
        let a = o["photos"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == "a.png")
            .unwrap();
        assert_eq!(
            (a["width"].as_u64(), a["height"].as_u64()),
            (Some(30), Some(20))
        );

        s.delete("ana", "a.png", None).unwrap();
        run(&s, Faces::Off, &["ana".into()]).unwrap();
        let left: i64 = s
            .with_db(|db| Ok(db.query_row("SELECT COUNT(*) FROM photo_meta", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(left, 1);
    }
}
