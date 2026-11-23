//! Symmetry drawing: every stroke is repeated mirrored across one or two axes, or turned around a
//! centre (radial). Stored with the shot, like the onion-skin settings, so commands replay alike.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SymmetryMode {
    #[default]
    Off,
    /// Mirror across the vertical axis through the centre (left ↔ right).
    Vertical,
    /// Mirror across the horizontal axis (top ↔ bottom).
    Horizontal,
    /// Both axes: four copies.
    Both,
    /// `spokes` copies turned evenly around the centre.
    Radial,
    /// `spokes` turned copies, each also mirrored (kaleidoscope).
    RadialMirror,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Symmetry {
    pub mode: SymmetryMode,
    /// Centre in canvas pixels; `None` is the canvas centre.
    pub center: Option<(f32, f32)>,
    /// Copies for the radial modes (2 … 32).
    pub spokes: u8,
}

impl Default for Symmetry {
    fn default() -> Self {
        Self { mode: SymmetryMode::Off, center: None, spokes: 6 }
    }
}

/// A linear map plus translation: `(a·x + b·y + tx, c·x + d·y + ty)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mirror {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Mirror {
    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        (self.a * x + self.b * y + self.tx, self.c * x + self.d * y + self.ty)
    }

    /// The linear part only (for tilt vectors).
    pub fn apply_vector(&self, x: f32, y: f32) -> (f32, f32) {
        (self.a * x + self.b * y, self.c * x + self.d * y)
    }

    /// About centre `(cx, cy)`: rotate by `angle`, after mirroring left–right when `flip`.
    fn about(cx: f32, cy: f32, angle: f32, flip_x: bool, flip_y: bool) -> Mirror {
        let (s, c) = angle.sin_cos();
        let (fx, fy) = (if flip_x { -1.0 } else { 1.0 }, if flip_y { -1.0 } else { 1.0 });
        let (a, b, cc, d) = (c * fx, -s * fy, s * fx, c * fy);
        Mirror { a, b, c: cc, d, tx: cx - (a * cx + b * cy), ty: cy - (cc * cx + d * cy) }
    }
}

impl Symmetry {
    pub fn sanitized(self) -> Self {
        let center = self.center.filter(|c| c.0.is_finite() && c.1.is_finite() && c.0.abs() < 1e6 && c.1.abs() < 1e6);
        Self { mode: self.mode, center, spokes: self.spokes.clamp(2, 32) }
    }

    pub fn is_on(&self) -> bool {
        self.mode != SymmetryMode::Off
    }

    /// The extra copies of a stroke (the original is not included).
    pub fn copies(&self, width: u32, height: u32) -> Vec<Mirror> {
        let (cx, cy) = self.center.unwrap_or((width as f32 / 2.0, height as f32 / 2.0));
        let tau = std::f32::consts::TAU;
        let n = self.spokes.clamp(2, 32) as u32;
        match self.mode {
            SymmetryMode::Off => Vec::new(),
            SymmetryMode::Vertical => vec![Mirror::about(cx, cy, 0.0, true, false)],
            SymmetryMode::Horizontal => vec![Mirror::about(cx, cy, 0.0, false, true)],
            SymmetryMode::Both => {
                vec![Mirror::about(cx, cy, 0.0, true, false), Mirror::about(cx, cy, 0.0, false, true), Mirror::about(cx, cy, 0.0, true, true)]
            }
            SymmetryMode::Radial => (1..n).map(|k| Mirror::about(cx, cy, tau * k as f32 / n as f32, false, false)).collect(),
            SymmetryMode::RadialMirror => {
                let mut v: Vec<Mirror> = (1..n).map(|k| Mirror::about(cx, cy, tau * k as f32 / n as f32, false, false)).collect();
                v.extend((0..n).map(|k| Mirror::about(cx, cy, tau * k as f32 / n as f32, true, false)));
                v
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies() {
        let s = Symmetry { mode: SymmetryMode::Vertical, center: None, spokes: 6 };
        let m = s.copies(100, 50);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].apply(10.0, 5.0), (90.0, 5.0));
        let both = Symmetry { mode: SymmetryMode::Both, ..s }.copies(100, 50);
        assert!(both.iter().any(|m| {
            let p = m.apply(10.0, 5.0);
            (p.0 - 90.0).abs() < 1e-4 && (p.1 - 45.0).abs() < 1e-4
        }));
        let r = Symmetry { mode: SymmetryMode::Radial, center: Some((0.0, 0.0)), spokes: 4 }.copies(10, 10);
        assert_eq!(r.len(), 3);
        let p = r[0].apply(1.0, 0.0);
        assert!(p.0.abs() < 1e-5 && (p.1 - 1.0).abs() < 1e-5, "a quarter turn");
        assert_eq!(Symmetry { mode: SymmetryMode::RadialMirror, spokes: 3, ..s }.copies(10, 10).len(), 5);
        assert!(Symmetry { mode: SymmetryMode::Off, ..s }.copies(1, 1).is_empty());
        let bad = Symmetry { mode: SymmetryMode::Radial, center: Some((f32::NAN, 0.0)), spokes: 200 }.sanitized();
        assert_eq!((bad.center, bad.spokes), (None, 32));
    }
}
