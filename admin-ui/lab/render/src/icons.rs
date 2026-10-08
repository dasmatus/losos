//! The device drawings, shape for shape and token for token the React Lab's
//! (admin-ui/app/src/lab/icons.tsx): `icon` is the logical view's glyph
//! (viewBox 0 0 64 56), `hardware` the physical view's front panel. Every
//! colour is a house token or a lab.css alias of one (`Theme`). Both are
//! drawn in their own coordinates; the caller places them with
//! `Painter::at`.

use crate::paint::{Painter, Rgba};
use crate::theme::Theme;

/// Rotates points about (cx, cy) by `deg`, SVG's `rotate(deg cx cy)`.
fn rot(pts: Vec<(f32, f32)>, deg: f32, cx: f32, cy: f32) -> Vec<(f32, f32)> {
    let (s, c) = deg.to_radians().sin_cos();
    pts.into_iter()
        .map(|(x, y)| {
            let (dx, dy) = (x - cx, y - cy);
            (cx + dx * c - dy * s, cy + dx * s + dy * c)
        })
        .collect()
}

/// A cloud as the union of its lobes and body: stroke colour dilated by
/// half the line width first, fill on top, which is the outline of the
/// union without computing it.
fn cloud(
    p: &mut Painter,
    lobes: &[(f32, f32, f32)],
    body: (f32, f32, f32, f32),
    fill: Rgba,
    sc: Rgba,
    w: f32,
) {
    let h = w / 2.0;
    for &(x, y, r) in lobes {
        p.circle(x, y, r + h, sc);
    }
    p.rect(body.0 - h, body.1 - h, body.2 + w, body.3 + w, 0.0, sc);
    for &(x, y, r) in lobes {
        p.circle(x, y, r - h, fill);
    }
    p.rect(body.0 + h, body.1 + h, body.2 - w, body.3 - w, 0.0, fill);
}

pub fn icon(p: &mut Painter, kind: &str, t: &Theme) {
    let (ink, surface, accent) = (t.ink, t.surface, t.accent);
    match kind {
        "box" | "edge-local" => {
            p.rect_fs(9.0, 16.0, 46.0, 28.0, 5.0, surface, ink, 2.0);
            p.rect(
                9.0,
                16.0,
                46.0,
                6.0,
                3.0,
                if kind == "box" { t.mark } else { accent },
            );
            p.circle(17.0, 35.0, 2.6, t.ok);
            if kind == "box" {
                p.rect(24.0, 33.0, 24.0, 3.0, 1.5, t.faint);
                p.stroke(&[(14.0, 44.0), (14.0, 48.0)], 2.0, ink, true);
                p.stroke(&[(50.0, 44.0), (50.0, 48.0)], 2.0, ink, true);
            } else {
                p.stroke(&[(27.0, 36.0), (45.0, 36.0)], 2.4, accent, true);
                p.stroke(
                    &[(40.0, 31.0), (45.0, 36.0), (40.0, 41.0)],
                    2.4,
                    accent,
                    true,
                );
                // Two signal arcs, drawn bulging right and turned to point up.
                let a = rot(
                    Painter::arc_pts(20.69, 12.0, 12.0, -0.34, 0.34),
                    -90.0,
                    32.0,
                    12.0,
                );
                let b = rot(
                    Painter::arc_pts(13.17, 12.0, 16.0, -0.384, 0.384),
                    -90.0,
                    32.0,
                    12.0,
                );
                p.stroke(&a, 1.8, accent, true);
                p.stroke(&b, 1.8, accent, true);
            }
        }
        "edge-official" => {
            p.rect_fs(6.0, 18.0, 52.0, 11.0, 2.5, surface, ink, 2.0);
            p.rect_fs(6.0, 31.0, 52.0, 11.0, 2.5, surface, ink, 2.0);
            p.circle(12.0, 23.5, 1.8, t.ok);
            p.circle(12.0, 36.5, 1.8, t.ok);
            p.stroke(&[(18.0, 23.5), (38.0, 23.5)], 2.0, t.faint, true);
            p.stroke(&[(18.0, 36.5), (38.0, 36.5)], 2.0, t.faint, true);
            let mut shield = vec![(47.0, 9.0), (55.0, 12.0), (55.0, 18.0)];
            shield.extend(Painter::cubic_pts(
                (55.0, 18.0),
                (55.0, 23.0),
                (51.5, 26.0),
                (47.0, 27.5),
                8,
            ));
            shield.extend(Painter::cubic_pts(
                (47.0, 27.5),
                (42.5, 26.0),
                (39.0, 23.0),
                (39.0, 18.0),
                8,
            ));
            shield.push((39.0, 12.0));
            p.poly_fs(&shield, accent, surface, 1.5);
            p.stroke(
                &[(43.5, 17.5), (46.0, 20.0), (50.5, 15.0)],
                2.0,
                surface,
                true,
            );
        }
        "laptop" => {
            p.rect_fs(13.0, 11.0, 38.0, 26.0, 3.0, t.screen, ink, 2.0);
            p.rect_stroke(17.0, 15.0, 12.0, 8.0, 1.5, 1.4, accent);
            p.rect(31.0, 15.0, 16.0, 18.0, 1.5, t.screen_tile);
            p.rect(17.0, 25.0, 12.0, 8.0, 1.5, t.screen_tile);
            p.poly_fs(
                &[(6.0, 41.0), (58.0, 41.0), (54.0, 46.0), (10.0, 46.0)],
                surface,
                ink,
                2.0,
            );
        }
        "router" => {
            p.ellipse_fs(32.0, 35.0, 23.0, 8.0, surface, ink, 2.0);
            p.rect(9.0, 27.0, 46.0, 8.0, 0.0, surface);
            p.segment((9.0, 27.0), (9.0, 35.0), 2.0, ink);
            p.segment((55.0, 27.0), (55.0, 35.0), 2.0, ink);
            p.ellipse_fs(32.0, 27.0, 23.0, 8.0, surface, ink, 2.0);
            for arrow in [
                [(24.0, 24.0), (18.0, 27.0), (24.0, 30.0)],
                [(40.0, 24.0), (46.0, 27.0), (40.0, 30.0)],
                [(29.0, 22.0), (32.0, 19.0), (35.0, 22.0)],
                [(29.0, 32.0), (32.0, 35.0), (35.0, 32.0)],
            ] {
                p.stroke(&arrow, 2.2, accent, true);
            }
        }
        "switch" => {
            p.poly_fs(
                &[(8.0, 24.0), (16.0, 16.0), (56.0, 16.0), (48.0, 24.0)],
                t.sunk,
                ink,
                2.0,
            );
            p.poly_fs(
                &[(56.0, 16.0), (56.0, 30.0), (48.0, 38.0), (48.0, 24.0)],
                t.sunk,
                ink,
                2.0,
            );
            p.rect_fs(8.0, 24.0, 40.0, 14.0, 0.0, surface, ink, 2.0);
            for arrow in [
                [(14.0, 28.0), (26.0, 28.0), (23.0, 25.5)],
                [(26.0, 34.0), (14.0, 34.0), (17.0, 36.5)],
                [(30.0, 28.0), (42.0, 28.0), (39.0, 25.5)],
                [(42.0, 34.0), (30.0, 34.0), (33.0, 36.5)],
            ] {
                p.stroke(&arrow, 2.0, accent, true);
            }
        }
        "ap" => {
            p.ellipse_fs(32.0, 38.0, 20.0, 7.0, surface, ink, 2.0);
            p.circle(32.0, 38.0, 2.2, accent);
            for (cy, r, a0, a1) in [
                (31.55, 11.0, -2.385, -0.756),
                (30.45, 18.0, -2.379, -0.763),
                (32.57, 5.0, -2.346, -0.796),
            ] {
                p.stroke(&Painter::arc_pts(32.0, cy, r, a0, a1), 2.2, accent, true);
            }
        }
        "bus" => {
            p.stroke(&[(6.0, 30.0), (58.0, 30.0)], 4.0, ink, true);
            p.dashed(&[(6.0, 30.0), (58.0, 30.0)], 1.6, surface, 3.0, 3.0, false);
            p.rect(2.0, 24.0, 6.0, 12.0, 1.5, ink);
            p.rect(56.0, 24.0, 6.0, 12.0, 1.5, ink);
            p.stroke(&[(18.0, 30.0), (18.0, 18.0)], 2.4, accent, true);
            p.stroke(&[(32.0, 30.0), (32.0, 42.0)], 2.4, accent, true);
            p.stroke(&[(46.0, 30.0), (46.0, 18.0)], 2.4, accent, true);
            p.circle(18.0, 16.0, 3.0, accent);
            p.circle(32.0, 44.0, 3.0, accent);
            p.circle(46.0, 16.0, 3.0, accent);
        }
        "internet" => {
            cloud(
                p,
                &[
                    (48.5, 34.0, 10.0),
                    (35.13, 25.87, 14.0),
                    (19.5, 32.5, 11.77),
                ],
                (19.5, 28.0, 29.0, 16.0),
                surface,
                ink,
                2.0,
            );
            p.ellipse_stroke(33.0, 33.0, 9.0, 9.0, 1.6, t.wan);
            p.segment((24.0, 33.0), (42.0, 33.0), 1.6, t.wan);
            p.stroke(
                &Painter::cubic_pts((33.0, 24.0), (29.0, 29.0), (29.0, 37.0), (33.0, 42.0), 10),
                1.6,
                t.wan,
                false,
            );
            p.stroke(
                &Painter::cubic_pts((33.0, 24.0), (37.0, 29.0), (37.0, 37.0), (33.0, 42.0), 10),
                1.6,
                t.wan,
                false,
            );
        }
        _ => {}
    }
}

/// A port's link state, for the LEDs beside it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Led {
    Up,
    Wait,
    Down,
    None,
}

fn led_col(t: &Theme, s: Led) -> Rgba {
    match s {
        Led::Up => t.ok,
        Led::Wait => t.warn,
        _ => t.led_off,
    }
}

/// A hardware drawing's size and its ports (id → point), in its own units
/// before the physical view's `HWK` scale.
pub struct Hw {
    pub w: f32,
    pub h: f32,
    pub ports: Vec<(String, (f32, f32))>,
    /// Where the power LED is, for the click that toggles it.
    pub power: Option<(f32, f32, f32)>,
}

impl Hw {
    pub fn port(&self, id: &str) -> (f32, f32) {
        self.ports
            .iter()
            .find(|p| p.0 == id)
            .map(|p| p.1)
            .unwrap_or((self.w / 2.0, self.h))
    }
}

pub fn hw_geom(kind: &str) -> Hw {
    let ports = |v: Vec<(String, (f32, f32))>| v;
    match kind {
        "box" | "edge-local" => Hw {
            w: 92.0,
            h: 50.0,
            ports: ports(vec![("eth0".into(), (80.0, 40.0))]),
            power: Some((14.0, 29.0, 4.2)),
        },
        "edge-official" => Hw {
            w: 132.0,
            h: 30.0,
            ports: vec![("eth0".into(), (118.0, 15.0))],
            power: Some((88.0, 15.0, 3.6)),
        },
        "laptop" => Hw {
            w: 96.0,
            h: 66.0,
            ports: vec![("eth0".into(), (6.0, 52.0)), ("wlan0".into(), (90.0, 52.0))],
            power: Some((48.0, 55.0, 3.0)),
        },
        "router" => {
            let mut v = vec![("wan".to_string(), (28.0, 36.0))];
            for (i, x) in [44.0, 56.0, 68.0, 80.0].into_iter().enumerate() {
                v.push((format!("lan{}", i + 1), (x, 36.0)));
            }
            Hw {
                w: 100.0,
                h: 46.0,
                ports: v,
                power: Some((10.0, 22.0, 3.4)),
            }
        }
        "switch" => Hw {
            w: 140.0,
            h: 30.0,
            ports: (0..8)
                .map(|i| (format!("p{}", i + 1), (34.0 + i as f32 * 12.0, 17.0)))
                .collect(),
            power: Some((24.0, 15.0, 2.6)),
        },
        "bus" => Hw {
            w: 228.0,
            h: 26.0,
            ports: (0..8)
                .map(|i| (format!("t{}", i + 1), (22.0 + i as f32 * 26.0, 10.0)))
                .collect(),
            power: Some((5.0, 17.0, 5.0)),
        },
        "ap" => Hw {
            w: 60.0,
            h: 40.0,
            ports: vec![("eth0".into(), (30.0, 36.0))],
            power: Some((30.0, 22.0, 3.4)),
        },
        "internet" => Hw {
            w: 120.0,
            h: 64.0,
            ports: (0..8)
                .map(|i| (format!("wan{}", i + 1), (14.0 + i as f32 * 12.0, 58.0)))
                .collect(),
            power: None,
        },
        _ => Hw {
            w: 60.0,
            h: 40.0,
            ports: vec![],
            power: None,
        },
    }
}

/// The front panel. `led(port)` answers the link state on that port.
pub fn hardware(p: &mut Painter, kind: &str, power: bool, t: &Theme, led: &dyn Fn(&str) -> Led) {
    let pwr = if power { t.ok } else { t.led_off };
    match kind {
        "box" | "edge-local" => {
            let stripe = if kind == "box" { t.mark } else { t.accent };
            p.rect_fs(0.0, 6.0, 92.0, 40.0, 7.0, t.kit, t.kit_edge, 1.0);
            p.rect(0.0, 6.0, 92.0, 7.0, 3.5, stripe);
            p.rect(6.0, 18.0, 40.0, 22.0, 3.0, t.kit_face);
            p.circle_fs(14.0, 29.0, 4.2, pwr, t.kit_edge, 1.0);
            p.rect(52.0, 24.0, 14.0, 9.0, 1.5, t.hole);
            p.rect(74.0, 34.0, 12.0, 9.0, 1.5, t.hole);
            p.circle(77.0, 32.0, 1.6, led_col(t, led("eth0")));
            p.circle(83.0, 32.0, 1.6, led_col(t, led("eth0")));
            p.rect(8.0, 46.0, 10.0, 3.0, 0.0, t.kit_edge);
            p.rect(74.0, 46.0, 10.0, 3.0, 0.0, t.kit_edge);
        }
        "edge-official" => {
            p.rect_fs(0.0, 2.0, 132.0, 26.0, 2.0, t.rack, t.rack_edge, 1.0);
            p.rect(6.0, 7.0, 70.0, 16.0, 1.5, t.rack_face);
            for i in 0..6 {
                p.rect(9.0 + i as f32 * 11.0, 10.0, 8.0, 10.0, 1.0, t.rack_bay);
            }
            p.circle(88.0, 15.0, 3.6, pwr);
            p.rect(111.0, 10.0, 14.0, 10.0, 1.5, t.hole);
            p.circle(114.0, 8.0, 1.4, led_col(t, led("eth0")));
            let mut shield = vec![(98.0, 9.0), (103.0, 10.8), (103.0, 14.4)];
            shield.extend(Painter::cubic_pts(
                (103.0, 14.4),
                (103.0, 17.4),
                (100.8, 19.4),
                (98.0, 20.4),
                6,
            ));
            shield.extend(Painter::cubic_pts(
                (98.0, 20.4),
                (95.2, 19.4),
                (93.0, 17.4),
                (93.0, 14.4),
                6,
            ));
            shield.push((93.0, 10.8));
            p.fill_poly(&shield, t.accent);
        }
        "laptop" => {
            p.rect_fs(10.0, 0.0, 76.0, 48.0, 4.0, t.rack, t.rack_edge, 1.0);
            p.rect(
                14.0,
                4.0,
                68.0,
                40.0,
                2.0,
                if power { t.screen } else { t.screen_off },
            );
            if power {
                p.rect_stroke(18.0, 8.0, 20.0, 12.0, 2.0, 1.0, t.accent);
                p.rect(41.0, 8.0, 37.0, 32.0, 2.0, t.screen_tile);
                p.rect(18.0, 23.0, 20.0, 17.0, 2.0, t.screen_tile);
            }
            p.poly_fs(
                &[(0.0, 50.0), (96.0, 50.0), (90.0, 60.0), (6.0, 60.0)],
                t.kit,
                t.kit_edge,
                1.0,
            );
            p.rect(2.0, 50.0, 9.0, 6.0, 1.0, t.hole);
            p.circle(48.0, 55.0, 3.0, pwr);
            p.circle(90.0, 55.0, 1.8, led_col(t, led("wlan0")));
        }
        "router" => {
            p.stroke(&[(14.0, 0.0), (14.0, 10.0)], 3.0, t.kit_edge, true);
            p.stroke(&[(86.0, 0.0), (86.0, 10.0)], 3.0, t.kit_edge, true);
            p.rect_fs(0.0, 10.0, 100.0, 34.0, 6.0, t.kit, t.kit_edge, 1.0);
            p.circle(10.0, 22.0, 3.4, pwr);
            p.rect(22.0, 31.0, 12.0, 9.0, 1.5, t.wan);
            p.circle(28.0, 20.0, 1.8, led_col(t, led("wan")));
            for (i, x) in [44.0, 56.0, 68.0, 80.0].into_iter().enumerate() {
                p.rect(x - 5.0, 31.0, 10.0, 9.0, 1.5, t.hole);
                p.circle(x, 20.0, 1.8, led_col(t, led(&format!("lan{}", i + 1))));
            }
        }
        "switch" => {
            p.rect_fs(0.0, 2.0, 140.0, 26.0, 2.5, t.rack, t.rack_edge, 1.0);
            p.circle(24.0, 15.0, 2.6, pwr);
            for i in 0..8 {
                let x = 34.0 + i as f32 * 12.0;
                p.rect(x - 4.5, 12.0, 9.0, 9.0, 1.0, t.hole);
                p.circle(x, 8.0, 1.4, led_col(t, led(&format!("p{}", i + 1))));
            }
        }
        "bus" => {
            p.rect(0.0, 12.0, 10.0, 10.0, 2.0, t.rack);
            p.rect(218.0, 12.0, 10.0, 10.0, 2.0, t.rack);
            p.segment((10.0, 17.0), (218.0, 17.0), 4.0, t.rack_edge);
            p.segment(
                (10.0, 17.0),
                (218.0, 17.0),
                1.4,
                if power { t.warn } else { t.kit_edge },
            );
            for i in 0..8 {
                let x = 22.0 + i as f32 * 26.0;
                p.segment((x, 17.0), (x, 11.0), 4.0, t.rack);
                p.circle(x, 10.0, 3.0, t.rack);
                p.circle(x, 22.5, 1.4, led_col(t, led(&format!("t{}", i + 1))));
            }
        }
        "ap" => {
            p.ellipse_fs(30.0, 22.0, 28.0, 13.0, t.kit, t.kit_edge, 1.0);
            p.circle(30.0, 22.0, 3.4, if power { t.accent } else { t.led_off });
            p.rect(25.0, 33.0, 10.0, 6.0, 1.0, t.hole);
        }
        "internet" => {
            cloud(
                p,
                &[(97.0, 38.0, 16.0), (74.2, 25.1, 24.0), (39.0, 35.0, 23.02)],
                (39.0, 30.0, 58.0, 24.0),
                t.surface,
                t.wan,
                2.0,
            );
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_type_draws_something() {
        let t = Theme::light();
        for k in [
            "box",
            "edge-local",
            "edge-official",
            "laptop",
            "router",
            "switch",
            "ap",
            "bus",
            "internet",
        ] {
            let mut p = Painter::new();
            icon(&mut p, k, &t);
            assert!(!p.idx.is_empty(), "{k} icon");
            let mut p = Painter::new();
            hardware(&mut p, k, true, &t, &|_| Led::Up);
            assert!(!p.idx.is_empty(), "{k} hardware");
        }
    }

    #[test]
    fn ports_match_the_catalogue_names() {
        assert_eq!(hw_geom("router").port("lan4"), (80.0, 36.0));
        assert_eq!(hw_geom("switch").port("p8"), (118.0, 17.0));
        assert_eq!(hw_geom("box").port("nope"), (46.0, 50.0));
    }
}
