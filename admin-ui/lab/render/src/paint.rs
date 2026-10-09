//! A small immediate-mode tessellator: filled and stroked shapes in the
//! canvas's own coordinates (x right, y down, the SVG Lab's units) turned
//! into a triangle list with a colour per vertex. The triangles lie flat on
//! the 3D scene's ground plane: canvas (x, y) is world (x, 0, y), Y up, so a
//! camera looking straight down with -Z as its up shows them the way the SVG
//! Lab did, and a tilted camera shows the same meshes in perspective.
//! Within one mesh, painter's order is index order.

// Drawing calls mirror SVG attribute lists.
#![allow(clippy::too_many_arguments)]
use std::f32::consts::{PI, TAU};

/// A colour already in linear RGBA, which is what a vertex colour is.
pub type Rgba = [f32; 4];

#[derive(Default)]
pub struct Painter {
    pub pos: Vec<[f32; 3]>,
    pub col: Vec<[f32; 4]>,
    pub idx: Vec<u32>,
    /// Applied to every point: (x, y) * k + (dx, dy). The icon and hardware
    /// drawings are written in their own viewBox and placed with this.
    pub tf: (f32, f32, f32),
    /// Mixes every colour toward `.0` by `.1`: the SVG Lab's `opacity` on a
    /// powered-off device, done opaque so overlapping parts do not show
    /// through each other.
    pub fade: Option<(Rgba, f32)>,
}

impl Painter {
    pub fn new() -> Painter {
        Painter {
            tf: (0.0, 0.0, 1.0),
            ..Painter::default()
        }
    }

    pub fn clear(&mut self) {
        self.pos.clear();
        self.col.clear();
        self.idx.clear();
        self.tf = (0.0, 0.0, 1.0);
        self.fade = None;
    }

    /// Draws `f` translated by (dx, dy) and scaled by `k`, on top of the
    /// current transform.
    pub fn at(&mut self, dx: f32, dy: f32, k: f32, f: impl FnOnce(&mut Painter)) {
        let old = self.tf;
        self.tf = (old.0 + dx * old.2, old.1 + dy * old.2, old.2 * k);
        f(self);
        self.tf = old;
    }

    fn v(&mut self, x: f32, y: f32, c: Rgba) -> u32 {
        let c = match self.fade {
            Some((to, f)) => [
                c[0] + (to[0] - c[0]) * f,
                c[1] + (to[1] - c[1]) * f,
                c[2] + (to[2] - c[2]) * f,
                c[3],
            ],
            None => c,
        };
        let (dx, dy, k) = self.tf;
        let i = self.pos.len() as u32;
        // The ground plane: canvas y runs along world +Z.
        self.pos.push([x * k + dx, 0.0, y * k + dy]);
        self.col.push(c);
        i
    }

    fn tri(&mut self, a: u32, b: u32, c: u32) {
        self.idx.extend_from_slice(&[a, b, c]);
    }

    /// A filled polygon, fanned from its centroid: right for convex and
    /// star-shaped outlines, which is every shape these drawings use.
    pub fn fill_poly(&mut self, pts: &[(f32, f32)], c: Rgba) {
        if pts.len() < 3 || c[3] <= 0.0 {
            return;
        }
        let n = pts.len() as f32;
        let (cx, cy) = pts
            .iter()
            .fold((0.0, 0.0), |a, p| (a.0 + p.0 / n, a.1 + p.1 / n));
        let centre = self.v(cx, cy, c);
        let first = self.v(pts[0].0, pts[0].1, c);
        let mut prev = first;
        for p in &pts[1..] {
            let cur = self.v(p.0, p.1, c);
            self.tri(centre, prev, cur);
            prev = cur;
        }
        self.tri(centre, prev, first);
    }

    /// One straight stroke segment as a quad.
    pub fn segment(&mut self, a: (f32, f32), b: (f32, f32), w: f32, c: Rgba) {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-4 || c[3] <= 0.0 {
            return;
        }
        let (nx, ny) = (-dy / len * w / 2.0, dx / len * w / 2.0);
        let i0 = self.v(a.0 + nx, a.1 + ny, c);
        let i1 = self.v(b.0 + nx, b.1 + ny, c);
        let i2 = self.v(b.0 - nx, b.1 - ny, c);
        let i3 = self.v(a.0 - nx, a.1 - ny, c);
        self.tri(i0, i1, i2);
        self.tri(i0, i2, i3);
    }

    /// A polyline of width `w`; `round` puts a disc on every vertex, which
    /// gives round joins and round caps.
    pub fn stroke(&mut self, pts: &[(f32, f32)], w: f32, c: Rgba, round: bool) {
        for p in pts.windows(2) {
            self.segment(p[0], p[1], w, c);
        }
        if round {
            for p in pts {
                self.circle(p.0, p.1, w / 2.0, c);
            }
        } else {
            // Mitre-free joins: a disc on the inner vertices only.
            if pts.len() > 2 {
                for p in &pts[1..pts.len() - 1] {
                    self.circle(p.0, p.1, w / 2.0, c);
                }
            }
        }
    }

    /// A dashed polyline: `dash` on, `gap` off, along the whole length.
    pub fn dashed(
        &mut self,
        pts: &[(f32, f32)],
        w: f32,
        c: Rgba,
        dash: f32,
        gap: f32,
        round: bool,
    ) {
        let mut on = true;
        let mut left = dash;
        for p in pts.windows(2) {
            let (a, b) = (p[0], p[1]);
            let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            if len < 1e-4 {
                continue;
            }
            let mut s = 0.0;
            while s < len {
                let e = (s + left).min(len);
                if on {
                    let pa = (a.0 + (b.0 - a.0) * s / len, a.1 + (b.1 - a.1) * s / len);
                    let pb = (a.0 + (b.0 - a.0) * e / len, a.1 + (b.1 - a.1) * e / len);
                    if round {
                        self.stroke(&[pa, pb], w, c, true);
                    } else {
                        self.segment(pa, pb, w, c);
                    }
                }
                left -= e - s;
                s = e;
                if left <= 1e-4 {
                    on = !on;
                    left = if on { dash } else { gap };
                }
            }
        }
    }

    fn segs(r: f32, k: f32) -> usize {
        // Enough segments to look round at the largest zoom.
        ((r * k).sqrt() * 4.0).clamp(10.0, 64.0) as usize
    }

    pub fn ellipse_pts(cx: f32, cy: f32, rx: f32, ry: f32, n: usize) -> Vec<(f32, f32)> {
        (0..n)
            .map(|i| {
                let a = i as f32 / n as f32 * TAU;
                (cx + rx * a.cos(), cy + ry * a.sin())
            })
            .collect()
    }

    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, c: Rgba) {
        self.ellipse(cx, cy, r, r, c);
    }

    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: Rgba) {
        let n = Self::segs(rx.max(ry), self.tf.2);
        let pts = Self::ellipse_pts(cx, cy, rx, ry, n);
        self.fill_poly(&pts, c);
    }

    pub fn ellipse_stroke(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, w: f32, c: Rgba) {
        let n = Self::segs(rx.max(ry), self.tf.2);
        let mut pts = Self::ellipse_pts(cx, cy, rx, ry, n);
        pts.push(pts[0]);
        // A ring as quads between the inner and outer outline.
        let inner = Self::ellipse_pts(cx, cy, (rx - w / 2.0).max(0.0), (ry - w / 2.0).max(0.0), n);
        let outer = Self::ellipse_pts(cx, cy, rx + w / 2.0, ry + w / 2.0, n);
        for i in 0..n {
            let j = (i + 1) % n;
            let a = self.v(inner[i].0, inner[i].1, c);
            let b = self.v(outer[i].0, outer[i].1, c);
            let d = self.v(outer[j].0, outer[j].1, c);
            let e = self.v(inner[j].0, inner[j].1, c);
            self.tri(a, b, d);
            self.tri(a, d, e);
        }
    }

    /// An outline with fill and stroke, the SVG way: stroke centred on the
    /// edge, drawn over the fill.
    pub fn ellipse_fs(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, fill: Rgba, sc: Rgba, w: f32) {
        self.ellipse(cx, cy, rx, ry, fill);
        self.ellipse_stroke(cx, cy, rx, ry, w, sc);
    }

    pub fn circle_fs(&mut self, cx: f32, cy: f32, r: f32, fill: Rgba, sc: Rgba, w: f32) {
        self.ellipse_fs(cx, cy, r, r, fill, sc, w);
    }

    pub fn rrect_pts(x: f32, y: f32, w: f32, h: f32, r: f32) -> Vec<(f32, f32)> {
        let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
        if r < 0.01 {
            return vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
        }
        let mut pts = Vec::with_capacity(28);
        let corners = [
            (x + w - r, y + r, -PI / 2.0),
            (x + w - r, y + h - r, 0.0),
            (x + r, y + h - r, PI / 2.0),
            (x + r, y + r, PI),
        ];
        for (cx, cy, a0) in corners {
            for i in 0..=6 {
                let a = a0 + i as f32 / 6.0 * PI / 2.0;
                pts.push((cx + r * a.cos(), cy + r * a.sin()));
            }
        }
        pts
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, c: Rgba) {
        if c[3] <= 0.0 {
            return;
        }
        if r < 0.01 {
            let a = self.v(x, y, c);
            let b = self.v(x + w, y, c);
            let d = self.v(x + w, y + h, c);
            let e = self.v(x, y + h, c);
            self.tri(a, b, d);
            self.tri(a, d, e);
            return;
        }
        let pts = Self::rrect_pts(x, y, w, h, r);
        self.fill_poly(&pts, c);
    }

    pub fn rect_stroke(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, lw: f32, c: Rgba) {
        let mut pts = Self::rrect_pts(x, y, w, h, r);
        pts.push(pts[0]);
        pts.push(pts[1]);
        for p in pts.windows(2) {
            self.segment(p[0], p[1], lw, c);
        }
        for p in &pts {
            self.circle_small(p.0, p.1, lw / 2.0, c);
        }
    }

    fn circle_small(&mut self, cx: f32, cy: f32, r: f32, c: Rgba) {
        let pts = Self::ellipse_pts(cx, cy, r, r, 8);
        self.fill_poly(&pts, c);
    }

    pub fn rect_fs(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        fill: Rgba,
        sc: Rgba,
        lw: f32,
    ) {
        self.rect(x, y, w, h, r, fill);
        self.rect_stroke(x, y, w, h, r, lw, sc);
    }

    pub fn poly_fs(&mut self, pts: &[(f32, f32)], fill: Rgba, sc: Rgba, lw: f32) {
        self.fill_poly(pts, fill);
        let mut closed = pts.to_vec();
        closed.push(pts[0]);
        self.stroke(&closed, lw, sc, true);
    }

    /// Points along a circular arc from angle `a0` to `a1` (radians, SVG's
    /// clockwise-from-x orientation since y points down).
    pub fn arc_pts(cx: f32, cy: f32, r: f32, a0: f32, a1: f32) -> Vec<(f32, f32)> {
        let n = ((a1 - a0).abs() / (PI / 24.0)).ceil().max(2.0) as usize;
        (0..=n)
            .map(|i| {
                let a = a0 + (a1 - a0) * i as f32 / n as f32;
                (cx + r * a.cos(), cy + r * a.sin())
            })
            .collect()
    }

    /// A cubic Bézier sampled into a polyline.
    pub fn cubic_pts(
        p0: (f32, f32),
        p1: (f32, f32),
        p2: (f32, f32),
        p3: (f32, f32),
        n: usize,
    ) -> Vec<(f32, f32)> {
        (0..=n)
            .map(|i| {
                let t = i as f32 / n as f32;
                let u = 1.0 - t;
                let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                (
                    a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0,
                    a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1,
                )
            })
            .collect()
    }
}

/// Distance from `p` to the segment a–b.
pub fn dist_seg(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-6 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0)
    };
    let (x, y) = (a.0 + dx * t, a.1 + dy * t);
    ((p.0 - x).powi(2) + (p.1 - y).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_lands_on_the_ground_plane() {
        let mut p = Painter::new();
        p.at(10.0, 20.0, 2.0, |p| {
            p.rect(1.0, 1.0, 1.0, 1.0, 0.0, [1.0; 4])
        });
        assert_eq!(p.pos[0], [12.0, 0.0, 22.0]);
        assert_eq!(p.idx.len(), 6);
    }

    #[test]
    fn dashes_cover_about_half() {
        let mut p = Painter::new();
        p.dashed(&[(0.0, 0.0), (100.0, 0.0)], 2.0, [1.0; 4], 5.0, 5.0, false);
        assert_eq!(p.idx.len() / 6, 10);
    }

    #[test]
    fn segment_distance() {
        assert!((dist_seg((5.0, 3.0), (0.0, 0.0), (10.0, 0.0)) - 3.0).abs() < 1e-5);
        assert!((dist_seg((-4.0, 3.0), (0.0, 0.0), (10.0, 0.0)) - 5.0).abs() < 1e-5);
    }
}
