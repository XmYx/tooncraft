//! Onion skin and light table settings, and which drawings they show.

use serde::{Deserialize, Serialize};

use crate::{DrawingId, Layer, LayerId};

/// What one onion-skin step means.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OnionMode {
    /// One step is the previous / next different drawing (holds are skipped).
    #[default]
    Drawings,
    /// One step is one frame (a held drawing is not shown twice).
    Frames,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OnionSkin {
    pub enabled: bool,
    /// Steps shown before the current frame (0 … 10).
    pub before: u8,
    /// Steps shown after (0 … 10).
    pub after: u8,
    /// Opacity of the nearest step.
    pub opacity: f32,
    /// Each further step is this fraction of the previous one's opacity.
    pub falloff: f32,
    /// Straight RGB tints.
    pub before_tint: [u8; 3],
    pub after_tint: [u8; 3],
    /// Off: show the drawings in their own colours.
    pub tinted: bool,
    pub mode: OnionMode,
    /// Show onion skin of every visible layer, not just the current one.
    pub all_layers: bool,
}

impl Default for OnionSkin {
    fn default() -> Self {
        Self {
            enabled: true,
            before: 2,
            after: 1,
            opacity: 0.45,
            falloff: 0.6,
            before_tint: [214, 64, 64],
            after_tint: [46, 150, 92],
            tinted: true,
            mode: OnionMode::Drawings,
            all_layers: false,
        }
    }
}

impl OnionSkin {
    pub fn sanitized(self) -> Self {
        let f = |v: f32, d: f32| if v.is_finite() { v.clamp(0.0, 1.0) } else { d };
        Self { before: self.before.min(10), after: self.after.min(10), opacity: f(self.opacity, 0.45), falloff: f(self.falloff, 0.6), ..self }
    }

    /// Opacity of the step `n` away (1 = nearest).
    pub fn step_opacity(&self, n: u32) -> f32 {
        self.opacity * self.falloff.powi(n.saturating_sub(1).min(16) as i32)
    }
}

/// A frame of a layer pinned under the current drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pin {
    pub layer: LayerId,
    pub frame: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LightTable {
    pub enabled: bool,
    pub opacity: f32,
    /// Straight RGB tint for pinned drawings.
    pub tint: [u8; 3],
    pub pins: Vec<Pin>,
}

impl Default for LightTable {
    fn default() -> Self {
        Self { enabled: true, opacity: 0.35, tint: [70, 110, 200], pins: Vec::new() }
    }
}

impl LightTable {
    pub fn sanitized(mut self) -> Self {
        self.opacity = if self.opacity.is_finite() { self.opacity.clamp(0.0, 1.0) } else { 0.35 };
        self.pins.truncate(64);
        self
    }

    /// Pin or unpin a frame of a layer.
    pub fn toggle(&mut self, pin: Pin) {
        if let Some(i) = self.pins.iter().position(|p| *p == pin) {
            self.pins.remove(i);
        } else if self.pins.len() < 64 {
            self.pins.push(pin);
        }
    }
}

pub(crate) fn neighbours(layer: &Layer, frame: u32, length: u32, o: &OnionSkin) -> Vec<(DrawingId, i32)> {
    let current = layer.drawing_at(frame);
    let mut out = Vec::new();
    for (dir, count) in [(-1i64, o.before), (1i64, o.after)] {
        let mut last = current;
        let mut steps = 0u8;
        let mut f = frame as i64;
        while steps < count {
            f += dir;
            if f < 0 || f >= length as i64 {
                break;
            }
            let d = layer.drawing_at(f as u32);
            match o.mode {
                OnionMode::Drawings => {
                    if let Some(d) = d
                        && Some(d) != last
                    {
                        steps += 1;
                        out.push((d, dir as i32 * steps as i32));
                        last = Some(d);
                    }
                }
                OnionMode::Frames => {
                    steps += 1;
                    if let Some(d) = d
                        && Some(d) != last
                        && Some(d) != current
                    {
                        out.push((d, dir as i32 * steps as i32));
                    }
                    last = d;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cell::{self, Blank as X, Hold as H};

    fn k(n: u64) -> Cell {
        Cell::Key(DrawingId(n))
    }

    fn layer(cells: Vec<Cell>) -> Layer {
        Layer { id: LayerId(1), name: "L".into(), visible: true, locked: false, opacity: 1.0, cells }
    }

    #[test]
    fn by_drawings_skips_holds() {
        let l = layer(vec![k(1), H, k(2), H, k(3), X, k(4), H]);
        let o = OnionSkin { before: 2, after: 2, ..Default::default() };
        let n = neighbours(&l, 4, 8, &o);
        assert_eq!(n, vec![(DrawingId(2), -1), (DrawingId(1), -2), (DrawingId(4), 1)]);
    }

    #[test]
    fn by_frames_counts_frames() {
        let l = layer(vec![k(1), H, k(2), H, k(3)]);
        let o = OnionSkin { before: 2, after: 0, mode: OnionMode::Frames, ..Default::default() };
        assert_eq!(neighbours(&l, 4, 5, &o), vec![(DrawingId(2), -1)]);
        let o = OnionSkin { before: 3, ..o };
        assert_eq!(neighbours(&l, 4, 5, &o), vec![(DrawingId(2), -1), (DrawingId(1), -3)]);
    }

    #[test]
    fn falloff_and_sanitize() {
        let o = OnionSkin { opacity: 0.5, falloff: 0.5, ..Default::default() };
        assert_eq!(o.step_opacity(1), 0.5);
        assert_eq!(o.step_opacity(3), 0.125);
        let s = OnionSkin { before: 99, opacity: f32::NAN, ..Default::default() }.sanitized();
        assert_eq!((s.before, s.opacity), (10, 0.45));
    }

    #[test]
    fn light_table_toggles() {
        let mut t = LightTable::default();
        let p = Pin { layer: LayerId(1), frame: 3 };
        t.toggle(p);
        assert_eq!(t.pins, vec![p]);
        t.toggle(p);
        assert!(t.pins.is_empty());
    }
}
