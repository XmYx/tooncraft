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
        self.at_from(frame, w, h, 0).0
    }

    /// The camera at frames `0..count`, in one pass over the keys: what [`Camera::at`] gives for
    /// each frame, without searching the keys again per frame (the camera path draws every
    /// frame of the shot). `count` is the caller's to cap.
    pub fn sweep(&self, count: u32, w: u32, h: u32) -> Vec<CameraKey> {
        let mut out = Vec::with_capacity(count as usize);
        let mut from = 0;
        for f in 0..count {
            let (k, i) = self.at_from(f, w, h, from);
            // the keys are sorted, so a later frame never finds an earlier pair
            from = i;
            out.push(k);
        }
        out
    }

    /// [`Camera::at`], searching the key pairs from `start` on; also returns the pair it used.
    fn at_from(&self, frame: u32, w: u32, h: u32, start: usize) -> (CameraKey, usize) {
        let neutral = CameraKey { frame, x: w as f32 / 2.0, y: h as f32 / 2.0, zoom: 1.0, angle: 0.0 };
        let Some(first) = self.keys.first() else { return (neutral, 0) };
        if frame <= first.frame {
            return (CameraKey { frame, ..*first }, 0);
        }
        for (i, pair) in self.keys.windows(2).enumerate().skip(start) {
            let (a, b) = (pair[0], pair[1]);
            if frame >= a.frame && frame <= b.frame {
                let t = ease((frame - a.frame) as f32 / (b.frame - a.frame).max(1) as f32);
                let l = |x: f32, y: f32| x + (y - x) * t;
                // zoom eases in log space so a zoom-in feels even
                let z = (a.zoom.max(1e-3).ln() + (b.zoom.max(1e-3).ln() - a.zoom.max(1e-3).ln()) * t).exp();
                return (CameraKey { frame, x: l(a.x, b.x), y: l(a.y, b.y), zoom: z, angle: l(a.angle, b.angle) }, i);
            }
        }
        let last = self.keys.last().copied().unwrap_or(neutral);
        (CameraKey { frame, ..last }, self.keys.len().saturating_sub(2).max(start))
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

/// Most cameras a shot has besides its main camera.
pub const MAX_CAMERAS: usize = 32;
/// Most camera cuts in a shot.
pub const MAX_CUTS: usize = 2_000;
/// Longest camera name (characters).
pub const MAX_CAMERA_NAME: usize = 100;

/// A camera besides the shot's main one, with its own name.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShotCamera {
    pub name: String,
    pub camera: Camera,
}

/// From `frame` on, the shot is filmed through `camera` (0 is the main camera, 1 the first of
/// [`crate::Project::cameras`]), until the next cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraCut {
    pub frame: u32,
    pub camera: usize,
}

/// Hostile-input repair for the extra cameras and the cuts: at most [`MAX_CAMERAS`] cameras with
/// bounded names; cuts sorted, one per frame, at most [`MAX_CUTS`], each to a camera that exists.
pub fn sanitize_cameras(cameras: Vec<ShotCamera>, cuts: Vec<CameraCut>, w: u32, h: u32) -> (Vec<ShotCamera>, Vec<CameraCut>) {
    let cameras: Vec<ShotCamera> = cameras
        .into_iter()
        .take(MAX_CAMERAS)
        .map(|c| ShotCamera { name: c.name.trim().chars().take(MAX_CAMERA_NAME).collect(), camera: c.camera.sanitized(w, h) })
        .collect();
    let mut cuts: Vec<CameraCut> = cuts.into_iter().take(MAX_CUTS).filter(|c| c.camera <= cameras.len()).collect();
    cuts.sort_by_key(|c| c.frame);
    // the last cut given for a frame wins
    cuts.reverse();
    cuts.dedup_by_key(|c| c.frame);
    cuts.reverse();
    (cameras, cuts)
}

impl crate::Project {
    /// How many cameras the shot has, the main one included.
    pub fn camera_count(&self) -> usize {
        1 + self.cameras.len()
    }

    /// The camera that films `frame`: the one the last cut at or before it chose (the main
    /// camera before the first cut).
    pub fn camera_index_at(&self, frame: u32) -> usize {
        let i = self.camera_cuts.partition_point(|c| c.frame <= frame);
        i.checked_sub(1).and_then(|i| self.camera_cuts.get(i)).map(|c| c.camera).filter(|c| *c < self.camera_count()).unwrap_or(0)
    }

    /// Camera `index` (0: the main camera).
    pub fn camera_by_index(&self, index: usize) -> Option<&Camera> {
        match index {
            0 => Some(&self.camera),
            i => self.cameras.get(i - 1).map(|c| &c.camera),
        }
    }

    pub fn camera_by_index_mut(&mut self, index: usize) -> Option<&mut Camera> {
        match index {
            0 => Some(&mut self.camera),
            i => self.cameras.get_mut(i - 1).map(|c| &mut c.camera),
        }
    }

    /// The camera that films `frame`.
    pub fn camera_at(&self, frame: u32) -> &Camera {
        self.camera_by_index(self.camera_index_at(frame)).unwrap_or(&self.camera)
    }

    /// Camera `index`'s name; `main` names the main camera.
    pub fn camera_name<'a>(&'a self, index: usize, main: &'a str) -> &'a str {
        match index {
            0 => main,
            i => self.cameras.get(i - 1).map(|c| c.name.as_str()).unwrap_or(main),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuts_choose_the_camera_that_films_each_frame() {
        let mut p = crate::Project::new("Cuts", 100, 50, 24, 40).unwrap();
        assert_eq!(p.camera_index_at(30), 0, "the main camera without cuts");
        p.cameras.push(ShotCamera { name: "Close".into(), camera: Camera::default() });
        p.camera_cuts = vec![CameraCut { frame: 10, camera: 1 }, CameraCut { frame: 20, camera: 0 }, CameraCut { frame: 30, camera: 7 }];
        let at: Vec<usize> = [0, 9, 10, 19, 20, 29, 35].iter().map(|f| p.camera_index_at(*f)).collect();
        assert_eq!(at, [0, 0, 1, 1, 0, 0, 0], "a cut to a missing camera films with the main one");
        assert_eq!(p.camera_name(1, "Main"), "Close");
        assert_eq!(p.camera_name(0, "Main"), "Main");
        let (cams, cuts) = sanitize_cameras(
            vec![ShotCamera { name: format!("  {}", "n".repeat(500)), camera: Camera::default() }; MAX_CAMERAS + 5],
            vec![CameraCut { frame: 5, camera: 1 }, CameraCut { frame: 2, camera: 99 }, CameraCut { frame: 5, camera: 0 }, CameraCut { frame: 1, camera: 2 }],
            100,
            50,
        );
        assert_eq!(cams.len(), MAX_CAMERAS);
        assert_eq!(cams[0].name.chars().count(), MAX_CAMERA_NAME);
        assert_eq!(cuts, [CameraCut { frame: 1, camera: 2 }, CameraCut { frame: 5, camera: 0 }], "sorted, one per frame, to cameras that exist");
    }

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
    fn a_sweep_gives_what_at_gives_on_every_frame() {
        let mut c = Camera::default();
        assert_eq!(c.sweep(4, 100, 50), (0..4).map(|f| c.at(f, 100, 50)).collect::<Vec<_>>(), "no keys");
        // keys on neighbouring frames, far apart, hostile values, one past the sweep
        for (frame, x) in [(3, 10.0), (4, -50.0), (5, 1.0e6), (40, f32::NAN), (41, 0.0), (90, 7.0), (500, 3.0)] {
            c.set(CameraKey { frame, x, y: x * 0.5, zoom: 1.0 + frame as f32 * 0.01, angle: frame as f32 });
        }
        let c = c.sanitized(100, 50);
        let swept = c.sweep(120, 100, 50);
        assert_eq!(swept.len(), 120);
        for (f, k) in swept.iter().enumerate() {
            assert_eq!(*k, c.at(f as u32, 100, 50), "frame {f}");
        }
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
