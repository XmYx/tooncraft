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

/// A grain texture with its scale, rotation and contrast, ready to sample: used by the brush's
/// dabs and by the paper texture under the drawing.
#[derive(Clone, Debug)]
pub struct Grain {
    pub kind: GrainKind,
    /// Feature size in canvas pixels (for an image: canvas pixels per image pixel).
    pub scale: f32,
    /// Rotation in degrees.
    pub angle: f32,
    /// 1 leaves the heights as they are; above 1 sharpens peaks and valleys, below 1 flattens.
    pub contrast: f32,
    /// The image when `kind` is `Image` (a missing one falls back to paper).
    pub image: Option<std::sync::Arc<crate::Tip>>,
    cos: f32,
    sin: f32,
}

impl Grain {
    pub fn new(kind: GrainKind, scale: f32, angle: f32, contrast: f32, image: Option<std::sync::Arc<crate::Tip>>) -> Self {
        let angle = if angle.is_finite() { angle } else { 0.0 };
        let (sin, cos) = angle.to_radians().sin_cos();
        let contrast = if contrast.is_finite() { contrast.clamp(0.1, crate::MAX_GRAIN_CONTRAST) } else { 1.0 };
        let scale = if scale.is_finite() { scale.clamp(0.25, 64.0) } else { 2.0 };
        let kind = if kind == GrainKind::Image && image.is_none() { GrainKind::Paper } else { kind };
        Grain { kind, scale, angle, contrast, image, cos, sin }
    }

    /// The height, 0 (valley) … 1 (peak), at the canvas pixel `(x, y)`.
    pub fn height(&self, x: f32, y: f32) -> f32 {
        // the texture turns about the canvas origin
        let (u, v) = if self.sin == 0.0 && self.cos == 1.0 { (x, y) } else { (x * self.cos + y * self.sin, -x * self.sin + y * self.cos) };
        let h = match (&self.kind, &self.image) {
            // image pixels are sampled at their centres, like the dab's own pixels
            (GrainKind::Image, Some(img)) => img.tile_height(u + 0.5, v + 0.5, self.scale),
            (kind, _) => builtin_grain(*kind, u, v, self.scale),
        };
        if self.contrast == 1.0 { h } else { ((h - 0.5) * self.contrast + 0.5).clamp(0.0, 1.0) }
    }
}

impl Grain {
    /// The height at `(x, y)` of a version of the grain that repeats every `period` pixels in
    /// both directions without a seam (four samples, cross-faded), so a small tile can cover a
    /// canvas of any size and a picture rendered in pieces matches the tiled display.
    pub fn seamless(&self, x: u32, y: u32, period: u32) -> f32 {
        let p = period.max(1);
        let (x, y) = (x % p, y % p);
        let (fx, fy, pf) = (x as f32 + 0.5, y as f32 + 0.5, p as f32);
        let (wx, wy) = (fx / pf, fy / pf);
        let a = self.height(fx, fy);
        let b = self.height(fx - pf, fy);
        let c = self.height(fx, fy - pf);
        let d = self.height(fx - pf, fy - pf);
        let mixed = a * (1.0 - wx) * (1.0 - wy) + b * wx * (1.0 - wy) + c * (1.0 - wx) * wy + d * wx * wy;
        // cross-fading two unrelated textures flattens them in the middle: restore the spread
        let w2 = (1.0 - wx) * (1.0 - wx) + wx * wx;
        let k = 1.0 / (w2 * ((1.0 - wy) * (1.0 - wy) + wy * wy)).sqrt();
        ((mixed - 0.5) * k + 0.5).clamp(0.0, 1.0)
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

    fn spread(g: &Grain) -> f32 {
        let vals: Vec<f32> = (0..4000).map(|i| g.height((i % 80) as f32, (i / 80) as f32)).collect();
        let mean = vals.iter().sum::<f32>() / vals.len() as f32;
        (vals.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / vals.len() as f32).sqrt()
    }

    #[test]
    fn contrast_sharpens_and_flattens_the_grain() {
        let base = spread(&Grain::new(GrainKind::Paper, 2.0, 0.0, 1.0, None));
        let hard = spread(&Grain::new(GrainKind::Paper, 2.0, 0.0, 3.0, None));
        let flat = spread(&Grain::new(GrainKind::Paper, 2.0, 0.0, 0.2, None));
        assert!(hard > base * 1.8 && flat < base * 0.3, "{flat} < {base} < {hard}");
        // hostile values fall back to something sane
        let g = Grain::new(GrainKind::Image, f32::NAN, f32::INFINITY, f32::NAN, None);
        assert!((0.0..=1.0).contains(&g.height(3.0, 4.0)) && g.kind == GrainKind::Paper);
    }

    #[test]
    fn the_seamless_tile_wraps_without_a_seam() {
        let g = Grain::new(GrainKind::Paper, 3.0, 20.0, 1.5, None);
        let p = 64;
        // across the wrap the step is no bigger than between neighbours inside the tile
        let inner: f32 = (0..p).map(|y| (g.seamless(30, y, p) - g.seamless(31, y, p)).abs()).sum::<f32>() / p as f32;
        let edge: f32 = (0..p).map(|y| (g.seamless(p - 1, y, p) - g.seamless(0, y, p)).abs()).sum::<f32>() / p as f32;
        assert!(edge < inner * 2.5 + 0.01, "seam {edge} vs {inner}");
        assert_eq!(g.seamless(5, 9, p), g.seamless(5 + 3 * p, 9 + p, p), "repeats");
        assert!(spread_seamless(&g, p) > 0.03, "flat");
    }

    fn spread_seamless(g: &Grain, p: u32) -> f32 {
        let vals: Vec<f32> = (0..p * p).map(|i| g.seamless(i % p, i / p, p)).collect();
        let mean = vals.iter().sum::<f32>() / vals.len() as f32;
        (vals.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / vals.len() as f32).sqrt()
    }

    #[test]
    fn rotation_turns_the_texture() {
        // the canvas weave has threads along x and y; a quarter turn maps it onto itself,
        // an eighth of a turn does not
        let g0 = Grain::new(GrainKind::Canvas, 3.0, 0.0, 1.0, None);
        let g90 = Grain::new(GrainKind::Canvas, 3.0, 90.0, 1.0, None);
        let g45 = Grain::new(GrainKind::Canvas, 3.0, 45.0, 1.0, None);
        let at = |g: &Grain, x: f32, y: f32| g.height(x, y);
        // turned by 90°: the value at (x, y) is the unturned value at (y, -x)
        for (x, y) in [(3.0, 7.0), (11.5, 2.25), (-4.0, 9.0)] {
            assert!((at(&g90, x, y) - at(&g0, y, -x)).abs() < 1e-3);
        }
        let differs = (0..200).filter(|i| (at(&g45, *i as f32, 5.0) - at(&g0, *i as f32, 5.0)).abs() > 0.05).count();
        assert!(differs > 50, "{differs}");
    }

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
