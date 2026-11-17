//! The canvas view: pan, zoom, rotation and mirror, and the maps between canvas pixels and
//! screen points.

use egui::{Pos2, Rect, Vec2, pos2, vec2};

pub const MIN_ZOOM: f32 = 0.02;
pub const MAX_ZOOM: f32 = 64.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// Screen offset of the canvas centre from the viewport centre, in points.
    pub pan: Vec2,
    /// Screen points per canvas pixel.
    pub zoom: f32,
    /// Rotation in radians, clockwise on screen.
    pub rotation: f32,
    /// Mirror left-right (a check for drawing errors).
    pub flip: bool,
}

impl Default for View {
    fn default() -> Self {
        Self { pan: Vec2::ZERO, zoom: 1.0, rotation: 0.0, flip: false }
    }
}

impl View {
    /// Canvas pixel → screen point inside `viewport`.
    pub fn to_screen(self, viewport: Rect, canvas: Vec2, p: Pos2) -> Pos2 {
        let mut d = (p.to_vec2() - canvas * 0.5) * self.zoom;
        if self.flip {
            d.x = -d.x;
        }
        let (s, c) = self.rotation.sin_cos();
        viewport.center() + self.pan + vec2(d.x * c - d.y * s, d.x * s + d.y * c)
    }

    /// Screen point → canvas pixel.
    pub fn to_canvas(self, viewport: Rect, canvas: Vec2, p: Pos2) -> Pos2 {
        let d = p - viewport.center() - self.pan;
        let (s, c) = self.rotation.sin_cos();
        let mut r = vec2(d.x * c + d.y * s, -d.x * s + d.y * c) / self.zoom.max(MIN_ZOOM);
        if self.flip {
            r.x = -r.x;
        }
        pos2(r.x, r.y) + canvas * 0.5
    }

    /// Zoom by `factor`, keeping the canvas point under `anchor` (screen) in place.
    pub fn zoom_at(&mut self, viewport: Rect, canvas: Vec2, anchor: Pos2, factor: f32) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let before = self.to_canvas(viewport, canvas, anchor);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let after = self.to_screen(viewport, canvas, before);
        self.pan += anchor - after;
    }

    /// Rotate by `radians` about the viewport centre.
    pub fn rotate(&mut self, radians: f32) {
        if !radians.is_finite() {
            return;
        }
        let (s, c) = radians.sin_cos();
        self.pan = vec2(self.pan.x * c - self.pan.y * s, self.pan.x * s + self.pan.y * c);
        self.rotation = (self.rotation + radians).rem_euclid(std::f32::consts::TAU);
    }

    /// Fit the canvas into the viewport, upright, with a margin.
    pub fn fit(viewport: Rect, canvas: Vec2) -> Self {
        let z = ((viewport.width() - 40.0) / canvas.x.max(1.0)).min((viewport.height() - 40.0) / canvas.y.max(1.0));
        Self { zoom: z.clamp(MIN_ZOOM, MAX_ZOOM), ..Default::default() }
    }

    pub fn rotation_degrees(self) -> f32 {
        let d = self.rotation.to_degrees();
        if d > 180.0 { d - 360.0 } else { d }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_with_rotation_and_flip() {
        let vp = Rect::from_min_size(pos2(10.0, 20.0), vec2(800.0, 600.0));
        let canvas = vec2(1920.0, 1080.0);
        let mut v = View { pan: vec2(30.0, -12.0), zoom: 0.37, rotation: 0.7, flip: true };
        for p in [pos2(0.0, 0.0), pos2(1919.0, 5.0), pos2(400.5, 900.25)] {
            let back = v.to_canvas(vp, canvas, v.to_screen(vp, canvas, p));
            assert!((back - p).length() < 1e-2, "{p:?} → {back:?}");
        }
        let anchor = pos2(200.0, 300.0);
        let under = v.to_canvas(vp, canvas, anchor);
        v.zoom_at(vp, canvas, anchor, 2.0);
        assert!((v.to_canvas(vp, canvas, anchor) - under).length() < 1e-2);
        v.rotate(std::f32::consts::TAU + 0.1);
        assert!(v.rotation < std::f32::consts::TAU);
    }
}
