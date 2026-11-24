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
    /// Draw the onion skin as outlines of the paint, not filled silhouettes.
    pub outline: bool,
    /// Leave out the nearest step before (keep only the earlier ones).
    pub hide_previous: bool,
    /// Use `before_steps` / `after_steps` for each step's opacity instead of `opacity` and
    /// `falloff`.
    pub per_step: bool,
    /// Opacity of each step before (nearest first) and after, when `per_step` is on.
    pub before_steps: [f32; MAX_STEPS],
    pub after_steps: [f32; MAX_STEPS],
    /// Stepping by drawings: show only drawings marked as keys.
    pub keys_only: bool,
}

/// Most steps on each side.
pub const MAX_STEPS: usize = 10;

/// Falling default opacities for per-step mode.
fn falling() -> [f32; MAX_STEPS] {
    let mut a = [0.0; MAX_STEPS];
    for (i, v) in a.iter_mut().enumerate() {
        *v = 0.5 * 0.7f32.powi(i as i32);
    }
    a
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
            outline: false,
            hide_previous: false,
            per_step: false,
            before_steps: falling(),
            after_steps: falling(),
            keys_only: false,
        }
    }
}

impl OnionSkin {
    pub fn sanitized(self) -> Self {
        let f = |v: f32, d: f32| if v.is_finite() { v.clamp(0.0, 1.0) } else { d };
        let steps = |a: [f32; MAX_STEPS]| a.map(|v| f(v, 0.3));
        Self {
            before: self.before.min(MAX_STEPS as u8),
            after: self.after.min(MAX_STEPS as u8),
            opacity: f(self.opacity, 0.45),
            falloff: f(self.falloff, 0.6),
            before_steps: steps(self.before_steps),
            after_steps: steps(self.after_steps),
            ..self
        }
    }

    /// Opacity of the step at offset `off` (negative: before, positive: after).
    pub fn offset_opacity(&self, off: i32) -> f32 {
        let n = off.unsigned_abs();
        if !self.per_step || n == 0 {
            return self.step_opacity(n);
        }
        let i = (n as usize - 1).min(MAX_STEPS - 1);
        if off < 0 { self.before_steps[i] } else { self.after_steps[i] }
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

#[cfg(test)]
pub(crate) fn neighbours(layer: &Layer, frame: u32, length: u32, o: &OnionSkin) -> Vec<(DrawingId, i32)> {
    neighbours_where(layer, frame, length, o, &|_| true)
}

/// The drawings the onion skin shows around `frame`, with their step offsets. Stepping by
/// drawings with `keys_only`, only drawings for which `is_key` holds count.
pub(crate) fn neighbours_where(layer: &Layer, frame: u32, length: u32, o: &OnionSkin, is_key: &dyn Fn(DrawingId) -> bool) -> Vec<(DrawingId, i32)> {
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
                        && (!o.keys_only || is_key(d))
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
    if o.hide_previous {
        out.retain(|(_, off)| *off != -1);
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
        Layer {
            id: LayerId(1),
            name: "L".into(),
            visible: true,
            locked: false,
            opacity: 1.0,
            blend: Default::default(),
            color: None,
            folder: None,
            peg: None,
            depth: 0.0,
            effects: Vec::new(),
            opacity_keys: Vec::new(),
            matte: None,
            mouth_chart: Default::default(),
            clip: false,
            lock_alpha: false,
            onion: None,
            colour_of: None,
            colour_pairs: Vec::new(),
            cells,
        }
    }

    #[test]
    fn by_drawings_skips_holds() {
        let l = layer(vec![k(1), H, k(2), H, k(3), X, k(4), H]);
        let o = OnionSkin { before: 2, after: 2, ..Default::default() };
        let n = neighbours(&l, 4, 8, &o);
        assert_eq!(n, vec![(DrawingId(2), -1), (DrawingId(1), -2), (DrawingId(4), 1)]);
    }

    #[test]
    fn hiding_the_previous_step_keeps_the_earlier_ones() {
        let l = layer(vec![k(1), H, k(2), H, k(3), X, k(4), H]);
        let o = OnionSkin { before: 2, after: 1, hide_previous: true, ..Default::default() };
        assert_eq!(neighbours(&l, 4, 8, &o), vec![(DrawingId(1), -2), (DrawingId(4), 1)]);
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

    #[test]
    fn per_step_opacity_and_keys_only() {
        let mut o = OnionSkin { per_step: true, ..Default::default() };
        o.before_steps[0] = 0.9;
        o.after_steps[1] = 0.2;
        assert_eq!(o.offset_opacity(-1), 0.9);
        assert_eq!(o.offset_opacity(2), 0.2);
        assert_eq!(o.offset_opacity(40), o.after_steps[MAX_STEPS - 1], "far steps use the last slider");
        let hostile = OnionSkin { before_steps: [f32::NAN; MAX_STEPS], after_steps: [9.0; MAX_STEPS], ..o }.sanitized();
        assert!(hostile.before_steps.iter().all(|v| v.is_finite()) && hostile.after_steps.iter().all(|v| *v <= 1.0));
        // keys only: drawings 1 and 3 are keys; 2 is skipped
        let l = layer(vec![k(1), k(2), k(3), k(4)]);
        let o = OnionSkin { before: 2, after: 0, keys_only: true, ..Default::default() };
        assert_eq!(neighbours_where(&l, 3, 4, &o, &|d| d.0 % 2 == 1), vec![(DrawingId(3), -1), (DrawingId(1), -2)]);
    }
}
