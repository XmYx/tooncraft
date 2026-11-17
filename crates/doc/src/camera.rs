//! The camera: keys that frame part of the canvas over time (pan, zoom, rotate), eased between
//! keys. Exports render through it when it is on; the canvas shows its frame.
//!
//! Layers can sit at a depth in front of or behind the drawing plane (multiplane): when the camera
//! pans, far layers slide less and near layers more, and when it trucks in (zoom), near layers grow
//! faster than far ones. Shakes add a smooth, repeatable jitter over a range of frames.

use tooncraft_raster::select::Affine;

use serde::{Deserialize, Serialize};

/// The camera at one frame. `x`, `y` is the centre in canvas pixels; `zoom` 1 shows the whole
/// canvas, 2 shows half of it; `angle` in degrees, clockwise.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CameraKey {
    pub frame: u32,
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
    pub angle: f32,
}

impl CameraKey {
    pub fn sanitized(self, w: u32, h: u32) -> Self {
        let f = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        Self {
            frame: self.frame,
            x: f(self.x, -(w as f32) * 4.0, w as f32 * 5.0, w as f32 / 2.0),
            y: f(self.y, -(h as f32) * 4.0, h as f32 * 5.0, h as f32 / 2.0),
            zoom: f(self.zoom, 0.05, 50.0, 1.0),
            angle: f(self.angle, -3600.0, 3600.0, 0.0),
        }
    }
}

/// Most shakes on one camera.
pub const MAX_SHAKES: usize = 256;
/// Nearest a layer can sit to the camera (depth units: 0 is the drawing plane, 1 is as far behind
/// it again as the camera is in front of it).
pub const MIN_DEPTH: f32 = -0.9;
pub const MAX_DEPTH: f32 = 100.0;

/// A camera shake over frames `from..=to`: up to `amplitude` pixels and `rotation` degrees of
/// smooth jitter, `frequency` wobbles per frame (0.02 … 1), easing in and out over `fade` frames.
/// The same `seed` always gives the same shake.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shake {
    pub from: u32,
    pub to: u32,
    pub amplitude: f32,
    #[serde(default)]
    pub rotation: f32,
    pub frequency: f32,
    #[serde(default)]
    pub fade: u32,
    #[serde(default)]
    pub seed: u32,
}

impl Shake {
    pub fn sanitized(self) -> Self {
        let f = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        Self {
            from: self.from.min(self.to),
            to: self.to.max(self.from),
            amplitude: f(self.amplitude, 0.0, 2000.0, 0.0),
            rotation: f(self.rotation, 0.0, 90.0, 0.0),
            frequency: f(self.frequency, 0.02, 1.0, 0.3),
            fade: self.fade.min(10_000),
            seed: self.seed,
        }
    }

    /// `(dx, dy, dangle)` at `frame` (zero outside the range).
    pub fn offset(&self, frame: u32) -> (f32, f32, f32) {
        if frame < self.from || frame > self.to {
            return (0.0, 0.0, 0.0);
        }
        let mut env = 1.0f32;
        if self.fade > 0 {
            let edge = (frame - self.from).min(self.to - frame) as f32 / self.fade as f32;
            let e = edge.clamp(0.0, 1.0);
            env = e * e * (3.0 - 2.0 * e);
        }
        let t = (frame - self.from) as f32 * self.frequency;
        let n = |channel: u32| smooth_noise(self.seed.wrapping_mul(3).wrapping_add(channel), t);
        (n(0) * self.amplitude * env, n(1) * self.amplitude * env, n(2) * self.rotation * env)
    }
}

/// Repeatable value in -1…1 for an integer lattice point.
fn lattice(seed: u32, i: i64) -> f32 {
    let mut x = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (seed as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 29;
    ((x >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
}

/// Smoothly interpolated lattice noise in -1…1.
fn smooth_noise(seed: u32, t: f32) -> f32 {
    let i = t.floor();
    let f = t - i;
    let u = f * f * (3.0 - 2.0 * f);
    let (a, b) = (lattice(seed, i as i64), lattice(seed, i as i64 + 1));
    a + (b - a) * u
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Camera {
    /// Render exports through the camera.
    pub enabled: bool,
    /// Keys sorted by frame, at most one per frame.
    pub keys: Vec<CameraKey>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub shakes: Vec<Shake>,
}

fn ease(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

impl Camera {
    /// The camera at `frame` for a `w`×`h` canvas: the neutral camera without keys, the key's value
    /// outside the keyed range, eased in and out between keys.
    pub fn at(&self, frame: u32, w: u32, h: u32) -> CameraKey {
        let neutral = CameraKey { frame, x: w as f32 / 2.0, y: h as f32 / 2.0, zoom: 1.0, angle: 0.0 };
        let Some(first) = self.keys.first() else { return neutral };
        if frame <= first.frame {
            return CameraKey { frame, ..*first };
        }
        for pair in self.keys.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if frame >= a.frame && frame <= b.frame {
                let t = ease((frame - a.frame) as f32 / (b.frame - a.frame).max(1) as f32);
                let l = |x: f32, y: f32| x + (y - x) * t;
                // zoom eases in log space so a zoom-in feels even
                let z = (a.zoom.max(1e-3).ln() + (b.zoom.max(1e-3).ln() - a.zoom.max(1e-3).ln()) * t).exp();
                return CameraKey { frame, x: l(a.x, b.x), y: l(a.y, b.y), zoom: z, angle: l(a.angle, b.angle) };
            }
        }
        let last = self.keys.last().copied().unwrap_or(neutral);
        CameraKey { frame, ..last }
    }

    /// Add or replace the key at its frame.
    pub fn set(&mut self, key: CameraKey) {
        match self.keys.binary_search_by_key(&key.frame, |k| k.frame) {
            Ok(i) => self.keys[i] = key,
            Err(i) => {
                if self.keys.len() < 10_000 {
                    self.keys.insert(i, key);
                }
            }
        }
    }

    pub fn remove(&mut self, frame: u32) -> bool {
        match self.keys.binary_search_by_key(&frame, |k| k.frame) {
            Ok(i) => {
                self.keys.remove(i);
                true
            }
            Err(_) => false,
        }
    }

    pub fn sanitized(mut self, w: u32, h: u32) -> Self {
        self.keys = self.keys.into_iter().map(|k| k.sanitized(w, h)).collect();
        self.keys.sort_by_key(|k| k.frame);
        self.keys.dedup_by_key(|k| k.frame);
        self.keys.truncate(10_000);
        self.shakes.truncate(MAX_SHAKES);
        self.shakes = self.shakes.into_iter().map(Shake::sanitized).collect();
        self
    }

    /// Whether exports and the camera view frame the canvas through this camera.
    pub fn active(&self) -> bool {
        self.enabled && (!self.keys.is_empty() || !self.shakes.is_empty())
    }

    /// The camera at `frame` with its shakes added (what is filmed).
    pub fn shot(&self, frame: u32, w: u32, h: u32) -> CameraKey {
        let mut k = self.at(frame, w, h);
        for s in &self.shakes {
            let (dx, dy, da) = s.offset(frame);
            k.x += dx;
            k.y += dy;
            k.angle += da;
        }
        k
    }

    /// The map from canvas pixels of a layer at `depth` to output pixels at `frame` (`None`: the
    /// layer is behind the camera). At depth 0 this is the plain camera; a layer drawn at its
    /// depth keeps its drawn size while the camera rests at the centre with zoom 1.
    pub fn layer_map(&self, frame: u32, w: u32, h: u32, depth: f32) -> Option<Affine> {
        let k = self.shot(frame, w, h);
        let d = if depth.is_finite() { depth.clamp(MIN_DEPTH, MAX_DEPTH) } else { 0.0 };
        // the camera has moved t towards the drawing plane (1 = onto it): zoom = 1 / (1 - t)
        let t = 1.0 - 1.0 / k.zoom.max(1e-3);
        let dist = 1.0 + d - t;
        if dist < 0.01 {
            return None;
        }
        let scale = (1.0 + d) / dist;
        let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
        // screen = scale·(p − c) + (c − cam) / dist, then rotate by −angle about the centre
        let (ox, oy) = ((cx - k.x) / dist, (cy - k.y) / dist);
        let (s, c) = (-k.angle).to_radians().sin_cos();
        let (a, b, cc, dd) = (c * scale, -s * scale, s * scale, c * scale);
        let (rx, ry) = (c * ox - s * oy, s * ox + c * oy);
        Some(Affine { a, b, c: cc, d: dd, tx: cx + rx - (a * cx + b * cy), ty: cy + ry - (cc * cx + dd * cy) })
    }

    /// Canvas position of an output pixel (both `w`×`h`) for the camera at `frame`.
    pub fn output_to_canvas(&self, frame: u32, w: u32, h: u32) -> impl Fn(f32, f32) -> (f32, f32) + use<> {
        let k = self.shot(frame, w, h);
        let (s, c) = k.angle.to_radians().sin_cos();
        let (cw, ch) = (w as f32 / 2.0, h as f32 / 2.0);
        let z = k.zoom.max(1e-3);
        move |ox: f32, oy: f32| {
            let (dx, dy) = ((ox - cw) / z, (oy - ch) / z);
            (k.x + dx * c - dy * s, k.y + dx * s + dy * c)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolation() {
        let mut c = Camera::default();
        assert_eq!(c.at(5, 100, 50), CameraKey { frame: 5, x: 50.0, y: 25.0, zoom: 1.0, angle: 0.0 });
        c.set(CameraKey { frame: 10, x: 0.0, y: 0.0, zoom: 1.0, angle: 0.0 });
        c.set(CameraKey { frame: 20, x: 100.0, y: 0.0, zoom: 4.0, angle: 90.0 });
        assert_eq!(c.at(0, 100, 50).x, 0.0);
        let mid = c.at(15, 100, 50);
        assert!((mid.x - 50.0).abs() < 1e-3 && (mid.zoom - 2.0).abs() < 1e-3 && (mid.angle - 45.0).abs() < 1e-3);
        assert!(c.at(11, 100, 50).x < 10.0, "eased");
        assert_eq!(c.at(99, 100, 50).x, 100.0);
        c.set(CameraKey { frame: 20, x: 7.0, y: 0.0, zoom: 1.0, angle: 0.0 });
        assert_eq!(c.keys.len(), 2);
        assert!(c.remove(10) && !c.remove(10));
        let map = Camera::default().output_to_canvas(0, 100, 50);
        assert_eq!(map(50.0, 25.0), (50.0, 25.0));
        let s =
            Camera { enabled: true, keys: vec![CameraKey { frame: 1, x: f32::NAN, y: 0.0, zoom: 0.0, angle: 0.0 }; 3], shakes: Vec::new() }.sanitized(100, 50);
        assert_eq!(s.keys.len(), 1);
        assert_eq!((s.keys[0].x, s.keys[0].zoom), (50.0, 0.05));
    }

    #[test]
    fn depth_zero_matches_the_plain_camera() {
        let mut c = Camera::default();
        c.set(CameraKey { frame: 0, x: 30.0, y: 20.0, zoom: 2.0, angle: 30.0 });
        let m = c.layer_map(0, 100, 50, 0.0).unwrap();
        let back = c.output_to_canvas(0, 100, 50);
        for (x, y) in [(0.0, 0.0), (50.0, 25.0), (13.0, 41.0)] {
            let (ox, oy) = m.apply(x, y);
            let (bx, by) = back(ox, oy);
            assert!((bx - x).abs() < 1e-3 && (by - y).abs() < 1e-3, "{x},{y} → {ox},{oy} → {bx},{by}");
        }
    }

    #[test]
    fn far_layers_pan_less_and_near_layers_grow_more() {
        let mut c = Camera::default();
        c.set(CameraKey { frame: 0, x: 50.0, y: 25.0, zoom: 1.0, angle: 0.0 });
        c.set(CameraKey { frame: 10, x: 70.0, y: 25.0, zoom: 1.0, angle: 0.0 });
        let at = |depth: f32, f: u32| c.layer_map(f, 100, 50, depth).unwrap().apply(50.0, 25.0).0;
        assert_eq!(at(1.0, 0), 50.0, "at rest every plane shows as drawn");
        let (near, plane, far) = (at(-0.5, 10) - 50.0, at(0.0, 10) - 50.0, at(1.0, 10) - 50.0);
        assert!((plane + 20.0).abs() < 1e-3);
        assert!(far.abs() < plane.abs() && near.abs() > plane.abs(), "{near} {plane} {far}");
        let mut z = Camera::default();
        z.set(CameraKey { frame: 0, x: 50.0, y: 25.0, zoom: 2.0, angle: 0.0 });
        let size = |depth: f32| z.layer_map(0, 100, 50, depth).unwrap().a;
        assert!((size(0.0) - 2.0).abs() < 1e-4);
        assert!(size(-0.3) > 2.0 && size(2.0) < 2.0);
        assert!(z.layer_map(0, 100, 50, -0.6).is_none(), "the camera has passed a near layer");
    }

    #[test]
    fn shakes_are_bounded_repeatable_and_fade() {
        let mut c = Camera::default();
        c.shakes.push(Shake { from: 10, to: 30, amplitude: 8.0, rotation: 2.0, frequency: 0.5, fade: 4, seed: 7 }.sanitized());
        assert_eq!(c.shot(5, 100, 50), c.at(5, 100, 50));
        assert_eq!(c.shot(10, 100, 50).x, 50.0, "faded in from zero");
        let mut moved = false;
        for f in 10..=30 {
            let k = c.shot(f, 100, 50);
            assert!((k.x - 50.0).abs() <= 8.0 && (k.y - 25.0).abs() <= 8.0 && k.angle.abs() <= 2.0);
            moved |= (k.x - 50.0).abs() > 0.5;
            assert_eq!(k, c.shot(f, 100, 50));
        }
        assert!(moved);
        let s = Shake { from: 9, to: 2, amplitude: f32::NAN, rotation: 1e9, frequency: 0.0, fade: 0, seed: 0 }.sanitized();
        assert_eq!((s.from, s.to, s.amplitude, s.rotation, s.frequency), (2, 9, 0.0, 90.0, 0.02));
    }
}
