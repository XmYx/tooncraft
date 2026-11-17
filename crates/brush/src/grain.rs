//! Grain: the surface texture a brush reveals. Built-in grains are procedural (value noise from an
//! integer hash, deterministic and free of tiling); a grain can also be an image (see
//! [`crate::Tip::tile_height`]). A grain value is a height, 0 (valley) … 1 (peak): light pressure
//! only touches the peaks, heavy pressure fills the valleys.

use serde::{Deserialize, Serialize};

/// Which texture a brush's grain uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GrainKind {
    /// Fibres and tooth of drawing paper.
    #[default]
    Paper,
    /// A woven canvas.
    Canvas,
    /// Sparse specks: stippling.
    Speckle,
    /// Soft mottling over a large area.
    Clouds,
    /// A greyscale image from the shot (the brush's `grain_image`), tiled.
    Image,
}

impl GrainKind {
    pub const ALL: [GrainKind; 5] = [GrainKind::Paper, GrainKind::Canvas, GrainKind::Speckle, GrainKind::Clouds, GrainKind::Image];

    pub fn id(self) -> &'static str {
        match self {
            GrainKind::Paper => "paper",
            GrainKind::Canvas => "canvas",
            GrainKind::Speckle => "speckle",
            GrainKind::Clouds => "clouds",
            GrainKind::Image => "image",
        }
    }
}

/// Height of a built-in grain at a canvas position. `Image` has no procedural form and falls back
/// to paper.
pub fn builtin_grain(kind: GrainKind, x: f32, y: f32, scale: f32) -> f32 {
    let s = scale.max(0.5);
    match kind {
        GrainKind::Paper | GrainKind::Image => paper_grain(x, y, scale),
        GrainKind::Canvas => {
            let (u, v) = (x / (s * 2.0) * std::f32::consts::TAU, y / (s * 2.0) * std::f32::consts::TAU);
            // over-under threads: each direction is a raised ridge, crossings sit between
            let weave = 0.5 + 0.22 * (u.cos() + v.cos()) + 0.06 * (u.cos() * v.cos());
            (0.85 * weave + 0.15 * value_noise(x / s * 0.9, y / s * 0.9)).clamp(0.0, 1.0)
        }
        GrainKind::Speckle => {
            let n = 0.7 * value_noise(x / s * 1.3 + 3.1, y / s * 1.3 - 8.7) + 0.3 * value_noise(x / s * 3.1, y / s * 3.1 + 40.0);
            smooth(((n - 0.55) / 0.2).clamp(0.0, 1.0))
        }
        GrainKind::Clouds => {
            let n = 0.55 * value_noise(x / (s * 6.0), y / (s * 6.0))
                + 0.3 * value_noise(x / (s * 2.7) + 9.0, y / (s * 2.7))
                + 0.15 * value_noise(x / s + 1.0, y / s - 3.0);
            n.clamp(0.0, 1.0)
        }
    }
}

fn hash(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ 0x2545_f491;
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn value_noise(x: f32, y: f32) -> f32 {
    let (fx, fy) = (x.floor(), y.floor());
    let (ix, iy) = (fx as i32, fy as i32);
    let (tx, ty) = (smooth(x - fx), smooth(y - fy));
    let a = hash(ix, iy);
    let b = hash(ix + 1, iy);
    let c = hash(ix, iy + 1);
    let d = hash(ix + 1, iy + 1);
    let top = a + (b - a) * tx;
    let bottom = c + (d - c) * tx;
    top + (bottom - top) * ty
}

/// Paper grain height at a canvas position, 0 (valley) … 1 (peak). Two octaves: fibres and tooth.
pub fn paper_grain(x: f32, y: f32, scale: f32) -> f32 {
    let s = scale.max(0.5);
    let n = 0.65 * value_noise(x / s, y / s) + 0.35 * value_noise(x / (s * 0.43) + 17.3, y / (s * 0.43) - 5.1);
    n.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grain_is_deterministic_and_in_range() {
        let mut lo = 1.0f32;
        let mut hi = 0.0f32;
        for i in 0..2000 {
            let v = paper_grain(i as f32 * 0.37, i as f32 * 1.13, 2.0);
            assert_eq!(v, paper_grain(i as f32 * 0.37, i as f32 * 1.13, 2.0));
            lo = lo.min(v);
            hi = hi.max(v);
        }
        assert!(lo >= 0.0 && hi <= 1.0 && hi - lo > 0.4, "{lo}..{hi}");
    }

    #[test]
    fn every_builtin_grain_has_texture_and_is_deterministic() {
        for kind in GrainKind::ALL {
            let (mut lo, mut hi, mut sum) = (1.0f32, 0.0f32, 0.0f32);
            for i in 0..4000 {
                let (x, y) = ((i % 80) as f32, (i / 80) as f32);
                let v = builtin_grain(kind, x, y, 2.0);
                assert_eq!(v, builtin_grain(kind, x, y, 2.0));
                assert!((0.0..=1.0).contains(&v), "{kind:?} {v}");
                lo = lo.min(v);
                hi = hi.max(v);
                sum += v;
            }
            assert!(hi - lo > 0.25, "{kind:?} is flat: {lo}..{hi}");
            assert!(sum / 4000.0 > 0.02, "{kind:?} is empty");
        }
    }
}
