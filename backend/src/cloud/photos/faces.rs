//! Faces found in a picture and told apart, on the box.
//!
//! Two small ONNX models from OpenCV's model zoo, run by tract in plain Rust:
//! YuNet (MIT, 230 kB) finds faces and five points on each (the eyes, the
//! nose, the corners of the mouth); SFace (Apache 2.0, 37 MB) turns a face,
//! turned and scaled so those points sit where it expects them, into 128
//! numbers. Two faces of one person point the same way: their cosine is
//! above [`SAME_PERSON`]. Both run on the CPU in well under a second per
//! picture on a two-core box, so the index needs no GPU and nothing leaves
//! the machine. `nix/face-models.nix` fetches the two files.

use image::RgbImage;
use std::path::Path;
use tract_onnx::prelude::*;

/// Faces whose embeddings' cosine is at least this are one person. Tested
/// on NASA's portraits of three astronauts: one person's pictures came out
/// between 0.54 and 0.78, two people's never above 0.44.
pub const SAME_PERSON: f32 = 0.5;
/// YuNet's confidence below which a find is not a face.
const MIN_SCORE: f32 = 0.8;
/// The side the detector looks at; a picture is scaled to fit it.
const SIDE: usize = 640;
/// Faces smaller than this, in the detector's 640 pixels, are too few
/// pixels to tell anyone apart by.
const MIN_FACE: f32 = 20.0;

/// Where SFace expects the five points on its 112 by 112 input.
const TEMPLATE: [[f32; 2]; 5] = [
    [38.2946, 51.6963],
    [73.5318, 51.5014],
    [56.0252, 71.7366],
    [41.5493, 92.3655],
    [70.7299, 92.2041],
];

type Plan = std::sync::Arc<TypedRunnableModel>;

pub struct Models {
    detect: Plan,
    /// The detector's outputs by name (`cls_8`, `bbox_16`, ...), in order.
    outputs: Vec<String>,
    embed: Plan,
}

/// A face, in the picture's own pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub score: f32,
    /// Left, top, width, height.
    pub rect: [f32; 4],
    pub points: [[f32; 2]; 5],
}

impl Models {
    /// `yunet.onnx` and `sface.onnx` from `dir`.
    pub fn load(dir: &Path) -> TractResult<Models> {
        let raw = tract_onnx::onnx().model_for_path(dir.join("yunet.onnx"))?;
        // The names are lost when the graph is optimised, so read them first.
        let outputs = raw
            .output_outlets()?
            .iter()
            .map(|o| {
                raw.outlet_label(*o)
                    .map(str::to_string)
                    .unwrap_or_else(|| raw.node(o.node).name.clone())
            })
            .collect();
        let detect = raw
            .with_input_fact(0, f32::fact([1, 3, SIDE, SIDE]).into())?
            .into_optimized()?
            .into_runnable()?;
        let embed = tract_onnx::onnx()
            .model_for_path(dir.join("sface.onnx"))?
            .with_input_fact(0, f32::fact([1, 3, 112, 112]).into())?
            .into_optimized()?
            .into_runnable()?;
        Ok(Models {
            detect,
            outputs,
            embed,
        })
    }

    /// The faces in a picture, best first. The whole picture is looked at
    /// once at the detector's 640 pixels; a large one is also looked at in
    /// overlapping squares of a 1280-pixel copy, so the faces of a group
    /// photo are not too few pixels to find.
    pub fn detect(&self, img: &RgbImage) -> TractResult<Vec<Found>> {
        let (w, h) = img.dimensions();
        if w == 0 || h == 0 {
            return Ok(Vec::new());
        }
        let longest = w.max(h) as f32;
        let mut found = Vec::new();
        let scale = (SIDE as f32 / longest).min(1.0);
        let small = shrink(img, scale);
        found.extend(self.look(&small, scale, (0, 0))?);
        if longest > 1.5 * SIDE as f32 {
            let scale = (2.0 * SIDE as f32 / longest).min(1.0);
            let big = shrink(img, scale);
            for y in tiles(big.height()) {
                for x in tiles(big.width()) {
                    let tile = image::imageops::crop_imm(
                        &big,
                        x,
                        y,
                        (SIDE as u32).min(big.width() - x),
                        (SIDE as u32).min(big.height() - y),
                    )
                    .to_image();
                    found.extend(self.look(&tile, scale, (x, y))?);
                }
            }
        }
        Ok(suppress(found))
    }

    /// One run of the detector over a square of at most 640 pixels that is
    /// `scale` times the picture and starts at `at` in that scaled copy.
    /// The finds come back in the picture's own pixels.
    fn look(&self, part: &RgbImage, scale: f32, at: (u32, u32)) -> TractResult<Vec<Found>> {
        // Blue, green, red, 0 to 255, the way OpenCV hands it over; the rest
        // of the square stays black.
        let mut input = tract_ndarray::Array4::<f32>::zeros((1, 3, SIDE, SIDE));
        for (x, y, px) in part.enumerate_pixels() {
            let (x, y) = (x as usize, y as usize);
            if x < SIDE && y < SIDE {
                input[[0, 0, y, x]] = px[2] as f32;
                input[[0, 1, y, x]] = px[1] as f32;
                input[[0, 2, y, x]] = px[0] as f32;
            }
        }
        let out = self.detect.run(tvec!(Tensor::from(input).into()))?;
        let get = |name: &str| -> TractResult<Vec<f32>> {
            let i = self
                .outputs
                .iter()
                .position(|n| n == name)
                .ok_or_else(|| anyhow::anyhow!("the detector has no output {name}"))?;
            Ok(out[i]
                .to_plain_array_view::<f32>()?
                .iter()
                .copied()
                .collect())
        };
        let (ox, oy) = (at.0 as f32, at.1 as f32);
        let back = |x: f32, y: f32| [(x + ox) / scale, (y + oy) / scale];
        let mut found = Vec::new();
        for stride in [8usize, 16, 32] {
            let cls = get(&format!("cls_{stride}"))?;
            let obj = get(&format!("obj_{stride}"))?;
            let bbox = get(&format!("bbox_{stride}"))?;
            let kps = get(&format!("kps_{stride}"))?;
            let cols = SIDE / stride;
            let s = stride as f32;
            for i in 0..cls.len().min(obj.len()) {
                let score = (cls[i].clamp(0.0, 1.0) * obj[i].clamp(0.0, 1.0)).sqrt();
                if score < MIN_SCORE || bbox.len() < 4 * i + 4 || kps.len() < 10 * i + 10 {
                    continue;
                }
                let (r, c) = ((i / cols) as f32, (i % cols) as f32);
                let cx = (c + bbox[4 * i]) * s;
                let cy = (r + bbox[4 * i + 1]) * s;
                let bw = bbox[4 * i + 2].exp() * s;
                let bh = bbox[4 * i + 3].exp() * s;
                if !(bw.is_finite() && bh.is_finite()) || bw.min(bh) < MIN_FACE {
                    continue;
                }
                let mut points = [[0f32; 2]; 5];
                for (n, p) in points.iter_mut().enumerate() {
                    *p = back(
                        (kps[10 * i + 2 * n] + c) * s,
                        (kps[10 * i + 2 * n + 1] + r) * s,
                    );
                }
                let [x, y] = back(cx - bw / 2.0, cy - bh / 2.0);
                found.push(Found {
                    score,
                    rect: [x, y, bw / scale, bh / scale],
                    points,
                });
            }
        }
        Ok(found)
    }

    /// The face's 128 numbers, of length one.
    pub fn embed(&self, img: &RgbImage, face: &Found) -> TractResult<Vec<f32>> {
        let input = align(img, &face.points);
        let out = self.embed.run(tvec!(Tensor::from(input).into()))?;
        let v: Vec<f32> = out[0]
            .to_plain_array_view::<f32>()?
            .iter()
            .copied()
            .collect();
        Ok(normalised(v))
    }
}

/// The picture `scale` times its size.
fn shrink(img: &RgbImage, scale: f32) -> RgbImage {
    let (w, h) = img.dimensions();
    let (nw, nh) = (
        ((w as f32 * scale).round() as u32).max(1),
        ((h as f32 * scale).round() as u32).max(1),
    );
    if (nw, nh) == (w, h) {
        return img.clone();
    }
    image::imageops::resize(img, nw, nh, image::imageops::FilterType::Triangle)
}

/// Where the squares start along a side: both ends and the middle, so
/// every face of the detector's size is whole in one of them.
fn tiles(len: u32) -> Vec<u32> {
    let side = SIDE as u32;
    if len <= side {
        return vec![0];
    }
    let mut at = vec![0, (len - side) / 2, len - side];
    at.dedup();
    at
}

/// The best of overlapping finds, the others dropped.
fn suppress(mut found: Vec<Found>) -> Vec<Found> {
    found.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut kept: Vec<Found> = Vec::new();
    for f in found {
        if kept.iter().all(|k| overlap(&k.rect, &f.rect) < 0.3) {
            kept.push(f);
        }
    }
    kept
}

/// Intersection over union.
fn overlap(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let (x1, y1) = (a[0].max(b[0]), a[1].max(b[1]));
    let (x2, y2) = (
        (a[0] + a[2]).min(b[0] + b[2]),
        (a[1] + a[3]).min(b[1] + b[3]),
    );
    let inter = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
    let union = a[2] * a[3] + b[2] * b[3] - inter;
    if union <= 0.0 {
        0.0
    } else {
        inter / union
    }
}

/// The face cut out, turned and scaled so its five points land on
/// [`TEMPLATE`]: the similarity transform that fits them best (Umeyama),
/// sampled backwards with bilinear filtering. Red, green, blue, 0 to 255.
fn align(img: &RgbImage, points: &[[f32; 2]; 5]) -> tract_ndarray::Array4<f32> {
    let mean = |p: &[[f32; 2]; 5]| {
        let (x, y) = p.iter().fold((0.0, 0.0), |(x, y), q| (x + q[0], y + q[1]));
        (x / 5.0, y / 5.0)
    };
    let (sx, sy) = mean(points);
    let (dx, dy) = mean(&TEMPLATE);
    let (mut a, mut b, mut ss) = (0f32, 0f32, 0f32);
    for (p, q) in points.iter().zip(TEMPLATE.iter()) {
        let (px, py) = (p[0] - sx, p[1] - sy);
        let (qx, qy) = (q[0] - dx, q[1] - dy);
        a += px * qx + py * qy;
        b += px * qy - py * qx;
        ss += px * px + py * py;
    }
    let ss = ss.max(f32::EPSILON);
    // dst = [m00 -m01; m01 m00] (src - s) + d, so src = its inverse (dst - d) + s.
    let (m00, m01) = (a / ss, b / ss);
    let det = (m00 * m00 + m01 * m01).max(f32::EPSILON);
    let (w, h) = (img.width() as f32, img.height() as f32);
    let pixel = |x: f32, y: f32, ch: usize| -> f32 {
        if x < 0.0 || y < 0.0 || x >= w || y >= h {
            0.0
        } else {
            img.get_pixel(x as u32, y as u32)[ch] as f32
        }
    };
    let mut out = tract_ndarray::Array4::<f32>::zeros((1, 3, 112, 112));
    for y in 0..112 {
        for x in 0..112 {
            let (qx, qy) = (x as f32 - dx, y as f32 - dy);
            let ux = (m00 * qx + m01 * qy) / det + sx;
            let uy = (-m01 * qx + m00 * qy) / det + sy;
            let (x0, y0) = (ux.floor(), uy.floor());
            let (fx, fy) = (ux - x0, uy - y0);
            for ch in 0..3 {
                out[[0, ch, y, x]] = pixel(x0, y0, ch) * (1.0 - fx) * (1.0 - fy)
                    + pixel(x0 + 1.0, y0, ch) * fx * (1.0 - fy)
                    + pixel(x0, y0 + 1.0, ch) * (1.0 - fx) * fy
                    + pixel(x0 + 1.0, y0 + 1.0, ch) * fx * fy;
            }
        }
    }
    out
}

pub fn normalised(mut v: Vec<f32>) -> Vec<f32> {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        v.iter_mut().for_each(|x| *x /= norm);
    }
    v
}

/// The cosine of two vectors of length one.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

pub fn to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

pub fn from_blob(b: &[u8]) -> Vec<f32> {
    b.as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(score: f32, rect: [f32; 4]) -> Found {
        Found {
            score,
            rect,
            points: [[0.0; 2]; 5],
        }
    }

    #[test]
    fn squares_cover_a_side_with_overlap() {
        assert_eq!(tiles(500), [0]);
        assert_eq!(tiles(640), [0]);
        assert_eq!(tiles(1280), [0, 320, 640]);
        assert_eq!(tiles(900), [0, 130, 260]);
    }

    #[test]
    fn overlapping_finds_keep_the_best() {
        let kept = suppress(vec![
            found(0.85, [10.0, 10.0, 100.0, 100.0]),
            found(0.95, [12.0, 12.0, 100.0, 100.0]),
            found(0.9, [300.0, 10.0, 80.0, 80.0]),
        ]);
        let scores: Vec<f32> = kept.iter().map(|f| f.score).collect();
        assert_eq!(scores, [0.95, 0.9]);
    }

    #[test]
    fn points_already_on_the_template_copy_the_pixels_through() {
        // A picture that is the template's own frame: the transform is the
        // identity, so the input is the picture itself.
        let img = RgbImage::from_fn(112, 112, |x, y| image::Rgb([x as u8, y as u8, 7]));
        let out = align(&img, &TEMPLATE);
        assert_eq!(out[[0, 0, 40, 30]].round(), 30.0);
        assert_eq!(out[[0, 1, 40, 30]].round(), 40.0);
        assert_eq!(out[[0, 2, 40, 30]].round(), 7.0);
    }

    #[test]
    fn a_face_twice_the_size_is_scaled_down() {
        let img = RgbImage::from_fn(224, 224, |x, y| {
            image::Rgb([(x / 2) as u8, (y / 2) as u8, 0])
        });
        let doubled = TEMPLATE.map(|[x, y]| [x * 2.0, y * 2.0]);
        let out = align(&img, &doubled);
        assert!((out[[0, 0, 60, 50]] - 50.0).abs() <= 1.0);
        assert!((out[[0, 1, 60, 50]] - 60.0).abs() <= 1.0);
    }

    #[test]
    fn embeddings_survive_the_database() {
        let v = normalised(vec![3.0, 4.0]);
        assert_eq!(v, [0.6, 0.8]);
        assert_eq!(from_blob(&to_blob(&v)), v);
        assert!((cosine(&v, &v) - 1.0).abs() < 1e-6);
    }
}
