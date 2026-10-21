//! Thumbnails of pictures, for the web app's grids and the phone apps.
//!
//! Drawn on first request at one of a few sizes, turned the way the camera
//! held the picture (EXIF orientation), and kept under
//! `<root>/.previews/<file id>/<size>-<etag>.jpg` until the file changes:
//! the ETag in the name means a new content never meets an old picture.
//! Pictures with transparency are kept as PNG.

use crate::cloud::store::StoreError;
use crate::cloud::Cloud;
use actix_web::http::header;
use actix_web::{HttpRequest, HttpResponse};
use image::{DynamicImage, ImageDecoder, ImageReader};
use std::path::{Path, PathBuf};

/// The sizes drawn; a request is rounded up to the next one.
const SIZES: [u32; 4] = [64, 256, 512, 1024];
/// Larger pictures than this are not decoded: a crafted file must not take
/// the box's memory.
const MAX_PIXELS: u64 = 80_000_000;
const MAX_BYTES: u64 = 100 << 20;

/// Whether the server can draw a thumbnail of this kind of file.
pub fn can_preview(mime: &str) -> bool {
    matches!(
        mime,
        "image/jpeg" | "image/png" | "image/gif" | "image/webp" | "image/bmp" | "image/tiff"
    )
}

pub fn size_for(asked: u32) -> u32 {
    SIZES.iter().copied().find(|s| *s >= asked).unwrap_or(1024)
}

fn cache_dir(cloud: &Cloud, id: i64) -> PathBuf {
    cloud.store.root().join(".previews").join(id.to_string())
}

/// Decode, turn and shrink. Returns the bytes and whether they are PNG.
pub fn render(src: &Path, size: u32) -> Result<(Vec<u8>, bool), String> {
    let meta = std::fs::metadata(src).map_err(|e| e.to_string())?;
    if meta.len() > MAX_BYTES {
        return Err("too large".into());
    }
    let reader = ImageReader::open(src)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let (w, h) = decoder.dimensions();
    if (w as u64) * (h as u64) > MAX_PIXELS {
        return Err("too many pixels".into());
    }
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    let thumb = img.thumbnail(size, size);
    let alpha = thumb.color().has_alpha();
    let mut out = std::io::Cursor::new(Vec::new());
    if alpha {
        thumb
            .write_to(&mut out, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
    } else {
        let rgb = DynamicImage::ImageRgb8(thumb.to_rgb8());
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 82);
        enc.encode_image(&rgb).map_err(|e| e.to_string())?;
    }
    Ok((out.into_inner(), alpha))
}

/// The thumbnail of `id` at about `size` pixels, for `login`.
pub fn serve(req: &HttpRequest, cloud: &Cloud, login: &str, id: i64, size: u32) -> HttpResponse {
    let e = match cloud.store.by_id(login, id) {
        Ok(e) => e,
        Err(err) => return crate::cloud::api::store_error(&err),
    };
    if e.dir || !can_preview(&e.mime) {
        return crate::cloud::api::store_error(&StoreError::NotFound);
    }
    let size = size_for(size);
    let tag = format!("\"p{}-{size}\"", e.etag);
    if req
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains(&tag))
    {
        return HttpResponse::NotModified().finish();
    }
    let dir = cache_dir(cloud, id);
    let jpg = dir.join(format!("{size}-{}.jpg", e.etag));
    let png = dir.join(format!("{size}-{}.png", e.etag));
    let cached = if jpg.is_file() {
        std::fs::read(&jpg).ok().map(|b| (b, false))
    } else if png.is_file() {
        std::fs::read(&png).ok().map(|b| (b, true))
    } else {
        None
    };
    let (bytes, is_png) = match cached {
        Some(c) => c,
        None => match render(&cloud.store.disk_path(login, &e.path), size) {
            Ok((bytes, is_png)) => {
                // Older pictures of this file go; the new one is kept.
                let _ = std::fs::remove_dir_all(&dir);
                if std::fs::create_dir_all(&dir).is_ok() {
                    let _ = std::fs::write(if is_png { &png } else { &jpg }, &bytes);
                }
                (bytes, is_png)
            }
            Err(err) => {
                tracing::debug!("cloud: no preview of {id}: {err}");
                return crate::cloud::api::store_error(&StoreError::NotFound);
            }
        },
    };
    HttpResponse::Ok()
        .content_type(if is_png { "image/png" } else { "image/jpeg" })
        .insert_header((header::ETAG, tag))
        .insert_header((header::CACHE_CONTROL, "private, max-age=86400"))
        .body(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_shrinks_and_keeps_its_aspect() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("a.png");
        let img = image::RgbImage::from_fn(400, 200, |x, _| image::Rgb([(x % 255) as u8, 0, 0]));
        img.save(&src).unwrap();
        let (bytes, png) = render(&src, 64).unwrap();
        assert!(!png);
        let back = image::load_from_memory(&bytes).unwrap();
        assert_eq!((back.width(), back.height()), (64, 32));
        assert_eq!(size_for(100), 256);
        assert_eq!(size_for(5000), 1024);
    }

    #[test]
    fn transparency_stays_png() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("a.png");
        image::RgbaImage::new(10, 10).save(&src).unwrap();
        assert!(render(&src, 64).unwrap().1);
    }

    #[test]
    fn a_broken_file_is_an_error_not_a_panic() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("a.jpg");
        std::fs::write(&src, b"not a jpeg at all").unwrap();
        assert!(render(&src, 64).is_err());
    }
}
