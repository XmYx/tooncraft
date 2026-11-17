//! One brush imprint and its coverage function.

use crate::{BrushKind, paper_grain};

/// One imprint of the brush.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dab {
    pub x: f32,
    pub y: f32,
    /// Radius in pixels.
    pub radius: f32,
    /// Peak coverage, 0 … 1 (flow × pressure opacity).
    pub alpha: f32,
    /// Rotation in radians (tilt direction).
    pub angle: f32,
    /// Minor / major axis, 0.1 … 1.
    pub roundness: f32,
    /// Pressure at this dab (drives grain reveal).
    pub pressure: f32,
}

/// Shape parameters precomputed for a stroke.
#[derive(Clone, Debug)]
pub(crate) struct Shape {
    pub kind: BrushKind,
    pub hardness: f32,
    pub grain: f32,
    pub grain_scale: f32,
    pub tip: Option<std::sync::Arc<crate::Tip>>,
}

impl Dab {
    /// Inclusive pixel bounds `(x0, y0, x1, y1)`, possibly negative.
    pub fn bounds(&self) -> (i64, i64, i64, i64) {
        let r = self.radius + 1.5;
        ((self.x - r).floor() as i64, (self.y - r).floor() as i64, (self.x + r).ceil() as i64, (self.y + r).ceil() as i64)
    }

    /// Coverage of the pixel whose centre is `(px + 0.5, py + 0.5)`.
    pub(crate) fn coverage(&self, px: i64, py: i64, shape: &Shape, cos: f32, sin: f32) -> f32 {
        let (cx, cy) = (px as f32 + 0.5 - self.x, py as f32 + 0.5 - self.y);
        // rotate into the dab frame and stretch the minor axis
        let u = cx * cos + cy * sin;
        let v = (-cx * sin + cy * cos) / self.roundness.max(0.1);
        let d = (u * u + v * v).sqrt();
        let r = self.radius.max(0.25);
        let mut a = if let Some(tip) = &shape.tip {
            tip.sample(u / r, v / r)
        } else if shape.kind == BrushKind::Airbrush || shape.hardness <= 0.0 {
            // gaussian-like falloff, zero at the radius
            let t = (d / r).min(1.0);
            let g = (1.0 - t * t).max(0.0);
            g * g
        } else {
            // flat core up to the hardness radius, then a smooth shoulder; at least a 1 px
            // antialiased edge so small brushes stay smooth
            let edge = (r * (1.0 - shape.hardness)).max(0.7);
            let inner = r - edge;
            if d <= inner {
                1.0
            } else {
                let t = ((d - inner) / (r + 0.5 - inner)).clamp(0.0, 1.0);
                1.0 - t * t * (3.0 - 2.0 * t)
            }
        };
        if a <= 0.0 {
            return 0.0;
        }
        if shape.grain > 0.0 {
            let g = paper_grain(px as f32, py as f32, shape.grain_scale);
            // Light pressure only touches the peaks of the paper; heavy pressure fills the tooth.
            let reveal = (g + (self.pressure - 0.5) * 0.7 + 0.15).clamp(0.0, 1.0);
            let t = if shape.kind == BrushKind::Chalk { smooth_step(0.35, 0.75, reveal) } else { reveal };
            a *= 1.0 - shape.grain * (1.0 - t);
        }
        (a * self.alpha).clamp(0.0, 1.0)
    }
}

fn smooth_step(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
