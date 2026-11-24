//! Shape strokes: lines, rectangles and ellipses drawn with the current brush. With `constrain`,
//! lines snap to 15° steps, rectangles become squares and ellipses circles.

use serde::{Deserialize, Serialize};
use tooncraft_brush::Sample;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    #[default]
    Line,
    Rectangle,
    Ellipse,
}

/// The second corner after constraint (snapped line angle, square, circle).
pub fn constrain(kind: ShapeKind, a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    match kind {
        ShapeKind::Line => {
            let len = dx.hypot(dy);
            let step = 15f32.to_radians();
            let ang = (dy.atan2(dx) / step).round() * step;
            (a.0 + ang.cos() * len, a.1 + ang.sin() * len)
        }
        _ => {
            let s = dx.abs().max(dy.abs());
            (a.0 + s * dx.signum(), a.1 + s * dy.signum())
        }
    }
}

/// Pen samples tracing the shape from corner `a` to corner `b`, at most every 2 px.
pub fn samples(kind: ShapeKind, a: (f32, f32), b: (f32, f32), constrained: bool, pressure: f32) -> Vec<Sample> {
    let b = if constrained { constrain(kind, a, b) } else { b };
    let pts: Vec<(f32, f32)> = match kind {
        ShapeKind::Line => vec![a, b],
        ShapeKind::Rectangle => vec![a, (b.0, a.1), b, (a.0, b.1), a],
        ShapeKind::Ellipse => {
            let (cx, cy) = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
            let (rx, ry) = ((b.0 - a.0).abs() / 2.0, (b.1 - a.1).abs() / 2.0);
            let n = ((rx.max(ry) * std::f32::consts::TAU / 2.0) as usize).clamp(16, 4000);
            (0..=n)
                .map(|i| {
                    let t = i as f32 / n as f32 * std::f32::consts::TAU;
                    (cx + t.cos() * rx, cy + t.sin() * ry)
                })
                .collect()
        }
    };
    let mut out = Vec::new();
    let mut time = 0.0;
    for w in pts.windows(2) {
        let n = ((w[1].0 - w[0].0).hypot(w[1].1 - w[0].1) / 2.0).ceil().clamp(1.0, 20_000.0) as usize;
        for k in 0..n {
            let t = k as f32 / n as f32;
            out.push(Sample { x: w[0].0 + (w[1].0 - w[0].0) * t, y: w[0].1 + (w[1].1 - w[0].1) * t, pressure, tilt_x: 0.0, tilt_y: 0.0, time, rotation: 0.0 });
            time += 0.002;
        }
    }
    if let Some(l) = pts.last() {
        out.push(Sample { x: l.0, y: l.1, pressure, tilt_x: 0.0, tilt_y: 0.0, time, rotation: 0.0 });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constraints() {
        let b = constrain(ShapeKind::Line, (0.0, 0.0), (100.0, 9.0));
        assert!(b.1.abs() < 1e-3, "snaps to horizontal");
        assert_eq!(constrain(ShapeKind::Rectangle, (0.0, 0.0), (10.0, -4.0)), (10.0, -10.0));
        let s = samples(ShapeKind::Ellipse, (0.0, 0.0), (100.0, 50.0), false, 1.0);
        assert!(s.iter().all(|p| p.x >= -0.01 && p.x <= 100.01 && p.y >= -0.01 && p.y <= 50.01));
        assert_eq!(samples(ShapeKind::Rectangle, (0.0, 0.0), (10.0, 10.0), false, 1.0).last().map(|p| (p.x, p.y)), Some((0.0, 0.0)));
    }
}
