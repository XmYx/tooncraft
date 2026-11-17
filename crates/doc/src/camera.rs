//! The camera: keys that frame part of the canvas over time (pan, zoom, rotate), eased between
//! keys. Exports render through it when it is on; the canvas shows its frame.

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

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Camera {
    /// Render exports through the camera.
    pub enabled: bool,
    /// Keys sorted by frame, at most one per frame.
    pub keys: Vec<CameraKey>,
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
        self
    }

    /// Canvas position of an output pixel (both `w`×`h`) for the camera at `frame`.
    pub fn output_to_canvas(&self, frame: u32, w: u32, h: u32) -> impl Fn(f32, f32) -> (f32, f32) + use<> {
        let k = self.at(frame, w, h);
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
        let s = Camera { enabled: true, keys: vec![CameraKey { frame: 1, x: f32::NAN, y: 0.0, zoom: 0.0, angle: 0.0 }; 3] }.sanitized(100, 50);
        assert_eq!(s.keys.len(), 1);
        assert_eq!((s.keys[0].x, s.keys[0].zoom), (50.0, 0.05));
    }
}
