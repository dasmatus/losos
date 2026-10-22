//! Where an edge runs: from a side of one card to a side of another, as a
//! curve (Obsidian's way), a straight line (`pathfindingMethod: direct`) or
//! right angles (`square`, and `a-star`, which this editor draws the same).
//! A side the file does not name is the one facing the other card.

use super::{Edge, Node};

pub struct Head {
    pub x: f64,
    pub y: f64,
    /// Degrees, the direction the arrow points in (0 is +x, 90 is +y).
    pub angle: f64,
    pub end: &'static str,
}

pub struct Route {
    /// SVG path data.
    pub path: String,
    pub label: (f64, f64),
    pub heads: Vec<Head>,
    pub from_side: &'static str,
    pub to_side: &'static str,
}

type P = (f64, f64);

fn side_name(s: Option<&str>) -> Option<&'static str> {
    match s? {
        "top" => Some("top"),
        "right" => Some("right"),
        "bottom" => Some("bottom"),
        "left" => Some("left"),
        _ => None,
    }
}

/// The side of `a` that faces `b`.
pub fn facing(a: &Node, b: &Node) -> &'static str {
    let (ax, ay) = a.center();
    let (bx, by) = b.center();
    let (dx, dy) = (bx - ax, by - ay);
    // Weighed by the cards' sizes, so a wide card next to a tall one still
    // picks the side a person would.
    let sx = dx.abs() / ((a.width + b.width) / 2.0).max(1.0);
    let sy = dy.abs() / ((a.height + b.height) / 2.0).max(1.0);
    if sx >= sy {
        if dx >= 0.0 {
            "right"
        } else {
            "left"
        }
    } else if dy >= 0.0 {
        "bottom"
    } else {
        "top"
    }
}

pub fn side_point(n: &Node, side: &str) -> P {
    match side {
        "top" => (n.x + n.width / 2.0, n.y),
        "bottom" => (n.x + n.width / 2.0, n.bottom()),
        "left" => (n.x, n.y + n.height / 2.0),
        _ => (n.right(), n.y + n.height / 2.0),
    }
}

fn normal(side: &str) -> P {
    match side {
        "top" => (0.0, -1.0),
        "bottom" => (0.0, 1.0),
        "left" => (-1.0, 0.0),
        _ => (1.0, 0.0),
    }
}

fn r1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn fmt(p: P) -> String {
    format!("{} {}", r1(p.0), r1(p.1))
}

fn angle(from: P, to: P) -> f64 {
    r1((to.1 - from.1).atan2(to.0 - from.0).to_degrees())
}

fn cubic(p0: P, c1: P, c2: P, p3: P, t: f64) -> P {
    let u = 1.0 - t;
    let a = u * u * u;
    let b = 3.0 * u * u * t;
    let c = 3.0 * u * t * t;
    let d = t * t * t;
    (
        a * p0.0 + b * c1.0 + c * c2.0 + d * p3.0,
        a * p0.1 + b * c1.1 + c * c2.1 + d * p3.1,
    )
}

/// The point halfway along a polyline.
fn halfway(points: &[P]) -> P {
    let len: f64 = points
        .windows(2)
        .map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt())
        .sum();
    let mut left = len / 2.0;
    for w in points.windows(2) {
        let seg = ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt();
        if seg >= left && seg > 0.0 {
            let t = left / seg;
            return (
                w[0].0 + (w[1].0 - w[0].0) * t,
                w[0].1 + (w[1].1 - w[0].1) * t,
            );
        }
        left -= seg;
    }
    points[points.len() / 2]
}

/// Right angles from `a` out along `na` to `b` in along `nb`.
fn elbows(a: P, na: P, b: P, nb: P) -> Vec<P> {
    const OUT: f64 = 24.0;
    let a1 = (a.0 + na.0 * OUT, a.1 + na.1 * OUT);
    let b1 = (b.0 + nb.0 * OUT, b.1 + nb.1 * OUT);
    let horiz = |n: P| n.1 == 0.0;
    let mut pts = vec![a, a1];
    match (horiz(na), horiz(nb)) {
        (true, true) => {
            let mx = (a1.0 + b1.0) / 2.0;
            pts.push((mx, a1.1));
            pts.push((mx, b1.1));
        }
        (false, false) => {
            let my = (a1.1 + b1.1) / 2.0;
            pts.push((a1.0, my));
            pts.push((b1.0, my));
        }
        (true, false) => pts.push((b1.0, a1.1)),
        (false, true) => pts.push((a1.0, b1.1)),
    }
    pts.push(b1);
    pts.push(b);
    pts.dedup();
    pts
}

pub fn route(from: &Node, to: &Node, e: &Edge) -> Route {
    let fs = side_name(e.from_side.as_deref()).unwrap_or_else(|| facing(from, to));
    let ts = side_name(e.to_side.as_deref()).unwrap_or_else(|| facing(to, from));
    let a = side_point(from, fs);
    let b = side_point(to, ts);
    let (na, nb) = (normal(fs), normal(ts));
    let method = e
        .style
        .get("pathfindingMethod")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    // Where each end's arrow points from: the last stretch into it.
    let (path, label, into_a, into_b) = match method {
        "direct" => (
            format!("M{} L{}", fmt(a), fmt(b)),
            ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0),
            b,
            a,
        ),
        "square" | "a-star" => {
            let pts = elbows(a, na, b, nb);
            let d = pts
                .iter()
                .enumerate()
                .map(|(i, p)| format!("{}{}", if i == 0 { "M" } else { " L" }, fmt(*p)))
                .collect::<String>();
            let n = pts.len();
            (d, halfway(&pts), pts[1], pts[n - 2])
        }
        _ => {
            let dist = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            let k = (dist * 0.5).clamp(40.0, 260.0);
            let c1 = (a.0 + na.0 * k, a.1 + na.1 * k);
            let c2 = (b.0 + nb.0 * k, b.1 + nb.1 * k);
            (
                format!("M{} C{} {} {}", fmt(a), fmt(c1), fmt(c2), fmt(b)),
                cubic(a, c1, c2, b, 0.5),
                c1,
                c2,
            )
        }
    };
    let mut heads = Vec::new();
    // JSON Canvas: fromEnd defaults to none, toEnd to arrow.
    if e.from_end.as_deref() == Some("arrow") {
        heads.push(Head {
            x: r1(a.0),
            y: r1(a.1),
            angle: angle(into_a, a),
            end: "from",
        });
    }
    if e.to_end.as_deref() != Some("none") {
        heads.push(Head {
            x: r1(b.0),
            y: r1(b.1),
            angle: angle(into_b, b),
            end: "to",
        });
    }
    Route {
        path,
        label: (r1(label.0), r1(label.1)),
        heads,
        from_side: fs,
        to_side: ts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(id: &str, x: f64, y: f64) -> Node {
        Node::blank(id.into(), "text", x, y, 100.0, 50.0)
    }

    #[test]
    fn sides_face_each_other() {
        let a = card("a", 0.0, 0.0);
        let b = card("b", 300.0, 10.0);
        assert_eq!(facing(&a, &b), "right");
        assert_eq!(facing(&b, &a), "left");
        let c = card("c", 20.0, 400.0);
        assert_eq!(facing(&a, &c), "bottom");
        assert_eq!(facing(&c, &a), "top");
    }

    #[test]
    fn curves_and_heads() {
        let a = card("a", 0.0, 0.0);
        let b = card("b", 300.0, 0.0);
        let e = Edge::between("e".into(), "a", "b");
        let r = route(&a, &b, &e);
        assert!(r.path.starts_with("M100 25 C"), "{}", r.path);
        assert!(r.path.ends_with("300 25"), "{}", r.path);
        assert_eq!(r.heads.len(), 1);
        assert_eq!(r.heads[0].end, "to");
        assert_eq!(r.heads[0].angle, 0.0);
        assert_eq!(r.label, (200.0, 25.0));
    }

    #[test]
    fn square_routes_turn_at_right_angles() {
        let a = card("a", 0.0, 0.0);
        let b = card("b", 300.0, 200.0);
        let mut e = Edge::between("e".into(), "a", "b");
        e.style.insert("pathfindingMethod".into(), "square".into());
        e.from_side = Some("right".into());
        e.to_side = Some("top".into());
        e.from_end = Some("arrow".into());
        let r = route(&a, &b, &e);
        assert_eq!(r.path, "M100 25 L124 25 L350 25 L350 176 L350 200");
        assert_eq!(r.heads.len(), 2);
        assert_eq!(r.heads[0].angle, 180.0);
        assert_eq!(r.heads[1].angle, 90.0);
    }
}
