//! Onion skin and light table settings, and which drawings they show.

use serde::{Deserialize, Serialize};

use tooncraft_raster::select::Affine;

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
    /// Show exactly the frames at the offsets in `picked` instead of stepping.
    pub pick: bool,
    /// Frame offsets from the current frame (negative: before), used when `pick` is on; 0 is an
    /// empty slot.
    pub picked: [i8; MAX_PICKED],
}

/// Most picked offsets.
pub const MAX_PICKED: usize = 16;
/// Furthest picked offset on each side, in frames.
pub const MAX_PICK_OFFSET: i8 = 99;

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
            pick: false,
            picked: picked_default(),
        }
    }
}

fn picked_default() -> [i8; MAX_PICKED] {
    let mut a = [0; MAX_PICKED];
    a[0] = -2;
    a[1] = -1;
    a[2] = 1;
    a
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
            picked: self.picked.map(|v| v.clamp(-MAX_PICK_OFFSET, MAX_PICK_OFFSET)),
            ..self
        }
    }

    /// The picked offsets in use: no empty slots or repeats, sorted.
    pub fn picked_offsets(&self) -> Vec<i32> {
        let mut v: Vec<i32> = self.picked.iter().map(|o| *o as i32).filter(|o| *o != 0 && o.abs() <= MAX_PICK_OFFSET as i32).collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// The picked offsets as text, such as `-4 -2 +2`.
    pub fn picked_text(&self) -> String {
        self.picked_offsets().iter().map(|o| if *o > 0 { format!("+{o}") } else { o.to_string() }).collect::<Vec<_>>().join(" ")
    }

    /// Read offsets typed as numbers separated by spaces or commas, such as `-4 -2 +2`. Refuses
    /// 0, numbers past ±99 and more than 16 offsets; repeats count once.
    pub fn parse_picked(text: &str) -> Result<[i8; MAX_PICKED], PickError> {
        let mut out: Vec<i8> = Vec::new();
        for word in text.split(|c: char| c == ',' || c == ';' || c.is_whitespace()).filter(|w| !w.is_empty()) {
            let n: i32 = word.trim_start_matches('+').parse().map_err(|_| PickError::NotANumber(word.chars().take(16).collect()))?;
            if n == 0 || n.abs() > MAX_PICK_OFFSET as i32 {
                return Err(PickError::OutOfRange(n));
            }
            let n = n as i8;
            if !out.contains(&n) {
                if out.len() == MAX_PICKED {
                    return Err(PickError::TooMany);
                }
                out.push(n);
            }
        }
        let mut a = [0; MAX_PICKED];
        for (slot, v) in a.iter_mut().zip(out) {
            *slot = v;
        }
        Ok(a)
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

/// Why typed onion offsets were refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickError {
    NotANumber(String),
    OutOfRange(i32),
    TooMany,
}

/// A frame of a layer pinned under the current drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pin {
    pub layer: LayerId,
    pub frame: u32,
    /// Shift and trace: where the pinned frame is shown, moved and turned, without changing the
    /// drawing.
    #[serde(default, skip_serializing_if = "PinShift::is_none")]
    pub shift: PinShift,
}

/// How far a pinned frame is moved and turned on the light table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PinShift {
    /// Canvas pixels.
    pub x: i32,
    pub y: i32,
    /// Tenths of a degree, clockwise on the screen.
    pub turn: i32,
}

/// Furthest a pinned frame moves, in canvas pixels.
pub const MAX_PIN_SHIFT: i32 = 20_000;

impl PinShift {
    pub fn is_none(&self) -> bool {
        *self == Self::default()
    }

    pub fn sanitized(self) -> Self {
        Self { x: self.x.clamp(-MAX_PIN_SHIFT, MAX_PIN_SHIFT), y: self.y.clamp(-MAX_PIN_SHIFT, MAX_PIN_SHIFT), turn: self.turn.rem_euclid(3600) }
    }

    /// The map from where the drawing is to where the pin shows it: turned about `pivot`, then
    /// moved.
    pub fn affine(&self, pivot: (f32, f32)) -> Affine {
        Affine::about(pivot, self.x as f32, self.y as f32, 1.0, 1.0, (self.turn as f32 / 10.0).to_radians())
    }
}

impl Pin {
    pub fn new(layer: LayerId, frame: u32) -> Self {
        Self { layer, frame, shift: PinShift::default() }
    }

    /// Whether this pins `frame` of `layer`, however it is shifted.
    pub fn is(&self, layer: LayerId, frame: u32) -> bool {
        self.layer == layer && self.frame == frame
    }
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
        for p in &mut self.pins {
            p.shift = p.shift.sanitized();
        }
        self
    }

    /// Pin or unpin a frame of a layer.
    pub fn toggle(&mut self, pin: Pin) {
        if let Some(i) = self.pins.iter().position(|p| p.is(pin.layer, pin.frame)) {
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
    if o.pick {
        // exactly the frames picked; each side's steps are numbered by nearness (±1 nearest), so
        // tints and opacities work as when stepping
        let offs = o.picked_offsets();
        let mut shown: Vec<DrawingId> = Vec::new();
        for side in [-1i32, 1] {
            let mut side_offs: Vec<i32> = offs.iter().copied().filter(|v| v.signum() == side).collect();
            side_offs.sort_by_key(|v| v.unsigned_abs());
            let mut step = 0;
            for off in side_offs {
                let f = frame as i64 + off as i64;
                if f < 0 || f >= length as i64 {
                    continue;
                }
                if let Some(d) = layer.drawing_at(f as u32)
                    && Some(d) != current
                    && !shown.contains(&d)
                {
                    step += 1;
                    shown.push(d);
                    out.push((d, side * step));
                }
            }
        }
        return out;
    }
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
            vector: false,
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
    fn picked_offsets_show_exactly_those_frames() {
        // frames:     0     1     2     3     4     5     6     7
        let l = layer(vec![k(1), k(2), k(3), k(4), k(5), k(6), H, k(8)]);
        let picked = OnionSkin::parse_picked("-4, -1 +2 +7").unwrap();
        let o = OnionSkin { pick: true, picked, before: 0, after: 0, ..Default::default() };
        // -1 → drawing 4, -4 → drawing 1, +2 → frame 6 holds drawing 6, +7 is past the end
        assert_eq!(neighbours(&l, 4, 8, &o), vec![(DrawingId(4), -1), (DrawingId(1), -2), (DrawingId(6), 1)]);
        // a held drawing shows once, and never the current one
        let o = OnionSkin { picked: OnionSkin::parse_picked("+1 +2").unwrap(), ..o };
        assert_eq!(neighbours(&l, 5, 8, &o).len(), 1, "frame 6 holds the current drawing; frame 7 is drawing 8");
    }

    #[test]
    fn typed_offsets_are_checked() {
        assert_eq!(OnionSkin::parse_picked("x"), Err(PickError::NotANumber("x".into())));
        assert_eq!(OnionSkin::parse_picked("0"), Err(PickError::OutOfRange(0)));
        assert_eq!(OnionSkin::parse_picked("-100"), Err(PickError::OutOfRange(-100)));
        let many: String = (1..=17).map(|n| format!("{n} ")).collect();
        assert_eq!(OnionSkin::parse_picked(&many), Err(PickError::TooMany));
        let a = OnionSkin::parse_picked("+3 3 -2").unwrap();
        let o = OnionSkin { picked: a, ..Default::default() };
        assert_eq!(o.picked_text(), "-2 +3");
        let hostile = OnionSkin { picked: [i8::MIN; MAX_PICKED], ..Default::default() }.sanitized();
        assert_eq!(hostile.picked_offsets(), vec![-MAX_PICK_OFFSET as i32]);
    }

    #[test]
    fn light_table_toggles() {
        let mut t = LightTable::default();
        let p = Pin::new(LayerId(1), 3);
        t.toggle(p);
        assert_eq!(t.pins, vec![p]);
        t.pins[0].shift.x = 12;
        t.toggle(p);
        assert!(t.pins.is_empty(), "a shifted pin still unpins");
    }

    #[test]
    fn pin_shift_moves_and_turns_about_the_pivot_and_is_sanitized() {
        let s = PinShift { x: 10, y: -5, turn: 900 };
        let (x, y) = s.affine((50.0, 50.0)).apply(60.0, 50.0);
        // a quarter turn about (50, 50) takes (60, 50) to (50, 60), then the move
        assert!((x - 60.0).abs() < 1e-3 && (y - 55.0).abs() < 1e-3, "{x} {y}");
        let h = PinShift { x: i32::MAX, y: i32::MIN, turn: -10 }.sanitized();
        assert_eq!(h, PinShift { x: MAX_PIN_SHIFT, y: -MAX_PIN_SHIFT, turn: 3590 });
        let lt = LightTable { pins: vec![Pin { shift: PinShift { x: i32::MAX, y: 0, turn: 0 }, ..Pin::new(LayerId(1), 0) }], ..Default::default() }.sanitized();
        assert_eq!(lt.pins[0].shift.x, MAX_PIN_SHIFT);
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
