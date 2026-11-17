//! ToonCraft's brush engine (layer 1, UI-free).
//!
//! Input [`Sample`]s (position, pressure, tilt, time) go through the [`Stabilizer`], are turned
//! into evenly spaced [`Dab`]s by [`StrokeEngine`], and the dabs are rasterised by [`Stroke`]
//! into a per-stroke coverage buffer. The surface pixels are always recomputed from the tile
//! as it was before the stroke plus that coverage, so:
//!
//! - overlapping dabs never darken a stroke beyond its opacity (pencil, ink, chalk, eraser use
//!   the maximum coverage; the airbrush builds up with flow),
//! - a stroke is one undo step (the surface's tile capture holds the "before" tiles),
//! - replaying the same samples gives the same pixels (the grain is a hash, not a random state).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

mod dab;
mod grain;
mod stabilizer;
mod stroke;

pub use dab::Dab;
pub use grain::paper_grain;
pub use stabilizer::Stabilizer;
pub use stroke::{Stroke, StrokeEngine, paint};

use serde::{Deserialize, Serialize};

/// Largest brush diameter in pixels.
pub const MAX_SIZE: f32 = 1000.0;

/// The built-in brush families.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BrushKind {
    /// Hard graphite with paper grain; pressure reveals more grain.
    #[default]
    Pencil,
    /// Smooth antialiased ink; pressure sets the width.
    Ink,
    /// Soft round with low flow that builds up.
    Airbrush,
    /// Textured chalk: strong grain and a rough edge.
    Chalk,
    /// Removes pixels; soft or hard by hardness.
    Eraser,
}

impl BrushKind {
    pub const ALL: [BrushKind; 5] = [BrushKind::Pencil, BrushKind::Ink, BrushKind::Airbrush, BrushKind::Chalk, BrushKind::Eraser];

    /// Coverage accumulates (true) or takes the maximum (false).
    pub fn builds_up(self) -> bool {
        matches!(self, BrushKind::Airbrush)
    }
}

/// How samples are smoothed before they become dabs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Smoothing {
    /// Moving-average strength, 0 (off) … 1 (strong).
    pub average: f32,
    /// Pulled-string length in canvas pixels; 0 is off. The brush follows the pointer only when
    /// the pointer is further away than this, which gives very steady lines.
    pub string: f32,
    /// At the end of the stroke, draw on to where the pointer was lifted.
    pub catch_up: bool,
}

impl Default for Smoothing {
    fn default() -> Self {
        Self { average: 0.3, string: 0.0, catch_up: true }
    }
}

/// How input maps to dab size, opacity and shape.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dynamics {
    /// 0 … 1: how much pressure scales the size.
    pub pressure_size: f32,
    /// 0 … 1: how much pressure scales the opacity.
    pub pressure_opacity: f32,
    /// 0 … 1: how much pen tilt flattens and turns the dab.
    pub tilt: f32,
    /// -1 … 1: positive thins the line when drawing fast, negative thickens it.
    pub velocity_size: f32,
    /// Smallest size as a fraction of the brush size (at zero pressure).
    pub min_size: f32,
}

impl Default for Dynamics {
    fn default() -> Self {
        Self { pressure_size: 0.8, pressure_opacity: 0.2, tilt: 0.0, velocity_size: 0.0, min_size: 0.15 }
    }
}

/// A brush: everything a stroke needs besides the input.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Brush {
    pub kind: BrushKind,
    /// Diameter in canvas pixels.
    pub size: f32,
    /// Stroke opacity ceiling, 0 … 1.
    pub opacity: f32,
    /// Coverage each dab adds, 0 … 1.
    pub flow: f32,
    /// 0 (soft edge) … 1 (hard edge).
    pub hardness: f32,
    /// Distance between dabs as a fraction of the diameter.
    pub spacing: f32,
    /// 0 … 1: how much paper grain shapes the dabs.
    pub grain: f32,
    /// Grain feature size in pixels.
    pub grain_scale: f32,
    pub dynamics: Dynamics,
    pub smoothing: Smoothing,
    /// Straight (not premultiplied) RGBA.
    pub color: [u8; 4],
}

impl Default for Brush {
    fn default() -> Self {
        Brush::preset(BrushKind::Pencil)
    }
}

impl Brush {
    /// The built-in preset for a brush family.
    pub fn preset(kind: BrushKind) -> Self {
        let base = Brush {
            kind,
            size: 4.0,
            opacity: 1.0,
            flow: 1.0,
            hardness: 0.9,
            spacing: 0.12,
            grain: 0.0,
            grain_scale: 2.0,
            dynamics: Dynamics::default(),
            smoothing: Smoothing::default(),
            color: [24, 22, 30, 255],
        };
        match kind {
            BrushKind::Pencil => Brush { size: 3.5, hardness: 0.7, grain: 0.55, grain_scale: 1.6, opacity: 0.9, ..base },
            BrushKind::Ink => Brush {
                size: 5.0,
                hardness: 1.0,
                spacing: 0.08,
                dynamics: Dynamics { pressure_size: 1.0, pressure_opacity: 0.0, min_size: 0.08, velocity_size: 0.25, ..Dynamics::default() },
                smoothing: Smoothing { average: 0.5, ..Smoothing::default() },
                ..base
            },
            BrushKind::Airbrush => Brush {
                size: 60.0,
                hardness: 0.0,
                flow: 0.08,
                spacing: 0.08,
                opacity: 0.8,
                dynamics: Dynamics { pressure_size: 0.2, pressure_opacity: 0.8, ..Dynamics::default() },
                ..base
            },
            BrushKind::Chalk => Brush {
                size: 18.0,
                hardness: 0.6,
                grain: 0.85,
                grain_scale: 3.0,
                spacing: 0.1,
                dynamics: Dynamics { pressure_size: 0.4, pressure_opacity: 0.4, tilt: 0.6, ..Dynamics::default() },
                ..base
            },
            BrushKind::Eraser => Brush { size: 24.0, hardness: 0.8, dynamics: Dynamics { pressure_size: 0.5, ..Dynamics::default() }, ..base },
        }
    }

    /// Clamp every field to its range; NaN becomes the preset's value. Brushes come from files
    /// and command input, so they are hostile until sanitized.
    pub fn sanitized(self) -> Self {
        let p = Brush::preset(self.kind);
        let f = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        Brush {
            kind: self.kind,
            size: f(self.size, 0.5, MAX_SIZE, p.size),
            opacity: f(self.opacity, 0.0, 1.0, p.opacity),
            flow: f(self.flow, 0.01, 1.0, p.flow),
            hardness: f(self.hardness, 0.0, 1.0, p.hardness),
            spacing: f(self.spacing, 0.02, 2.0, p.spacing),
            grain: f(self.grain, 0.0, 1.0, p.grain),
            grain_scale: f(self.grain_scale, 0.5, 64.0, p.grain_scale),
            dynamics: Dynamics {
                pressure_size: f(self.dynamics.pressure_size, 0.0, 1.0, p.dynamics.pressure_size),
                pressure_opacity: f(self.dynamics.pressure_opacity, 0.0, 1.0, p.dynamics.pressure_opacity),
                tilt: f(self.dynamics.tilt, 0.0, 1.0, p.dynamics.tilt),
                velocity_size: f(self.dynamics.velocity_size, -1.0, 1.0, p.dynamics.velocity_size),
                min_size: f(self.dynamics.min_size, 0.01, 1.0, p.dynamics.min_size),
            },
            smoothing: Smoothing {
                average: f(self.smoothing.average, 0.0, 1.0, p.smoothing.average),
                string: f(self.smoothing.string, 0.0, 500.0, p.smoothing.string),
                catch_up: self.smoothing.catch_up,
            },
            color: self.color,
        }
    }
}

/// One input event in canvas coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sample {
    pub x: f32,
    pub y: f32,
    /// 0 … 1; mice report 0.5.
    pub pressure: f32,
    /// Pen tilt towards +x and +y, each -1 … 1 (0 upright).
    pub tilt_x: f32,
    pub tilt_y: f32,
    /// Seconds since the stroke began.
    pub time: f64,
}

impl Default for Sample {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0, pressure: 0.5, tilt_x: 0.0, tilt_y: 0.0, time: 0.0 }
    }
}

impl Sample {
    pub fn at(x: f32, y: f32, pressure: f32, time: f64) -> Self {
        Self { x, y, pressure, time, ..Default::default() }
    }

    /// `None` when the position is not finite; other fields are clamped.
    pub fn sanitized(self) -> Option<Self> {
        if !self.x.is_finite() || !self.y.is_finite() || self.x.abs() > 1e7 || self.y.abs() > 1e7 {
            return None;
        }
        let c = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        Some(Sample {
            x: self.x,
            y: self.y,
            pressure: c(self.pressure, 0.0, 1.0, 0.5),
            tilt_x: c(self.tilt_x, -1.0, 1.0, 0.0),
            tilt_y: c(self.tilt_y, -1.0, 1.0, 0.0),
            time: if self.time.is_finite() { self.time.max(0.0) } else { 0.0 },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_brush_and_samples() {
        let b = Brush { size: f32::NAN, opacity: 7.0, spacing: -1.0, ..Brush::preset(BrushKind::Ink) }.sanitized();
        assert_eq!(b.size, Brush::preset(BrushKind::Ink).size);
        assert_eq!(b.opacity, 1.0);
        assert_eq!(b.spacing, 0.02);
        assert!(Sample { x: f32::INFINITY, ..Default::default() }.sanitized().is_none());
        assert_eq!(Sample { pressure: 9.0, ..Default::default() }.sanitized().map(|s| s.pressure), Some(1.0));
    }
}
