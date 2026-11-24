//! Pegs: keyed transforms (move, rotate, scale, skew about a pivot) that layers hang from.
//!
//! A peg can hang from another peg, so a character's arm can swing on its own peg while the body
//! peg carries it across the shot. A layer on a peg is drawn through the chain of transforms from
//! its peg up to the top; its pixels are never changed, so taking a layer off a peg gives the
//! drawing back as it was drawn.
//!
//! Between two keys the motion follows the first key's [`Ease`]: linear, ease in, ease out, both,
//! a held value, or a cubic timing curve given by two handles (the same curve shape as the cubic
//! Bézier timing functions of CSS, a public specification).

use serde::{Deserialize, Serialize};
use tooncraft_raster::select::Affine;

/// Most pegs in a project.
pub const MAX_PEGS: usize = 256;
/// Most keys on one peg.
pub const MAX_PEG_KEYS: usize = 10_000;
/// Deepest chain of pegs hanging from pegs.
pub const MAX_PEG_DEPTH: usize = 32;
/// Furthest a key moves in one step (the longest shot).
const MAX_FRAMES_I64: i64 = crate::MAX_FRAMES as i64;

/// A peg's stable id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PegId(pub u64);

/// How a value moves from one key to the next.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Ease {
    /// Constant speed.
    Linear,
    /// Starts slow.
    In,
    /// Ends slow.
    Out,
    /// Starts and ends slow.
    #[default]
    InOut,
    /// Keeps the key's value until the next key (a pop).
    Hold,
    /// A timing curve through (0, 0), two handles and (1, 1); handle x values are kept in 0…1.
    Curve { x1: f32, y1: f32, x2: f32, y2: f32 },
}

impl Ease {
    /// Every preset, for menus.
    pub const PRESETS: [Ease; 5] = [Ease::Linear, Ease::In, Ease::Out, Ease::InOut, Ease::Hold];

    /// The handles of the curve this ease follows (`Hold` has none).
    pub fn handles(self) -> Option<(f32, f32, f32, f32)> {
        match self {
            Ease::Linear => Some((0.0, 0.0, 1.0, 1.0)),
            Ease::In => Some((0.42, 0.0, 1.0, 1.0)),
            Ease::Out => Some((0.0, 0.0, 0.58, 1.0)),
            Ease::InOut => Some((0.42, 0.0, 0.58, 1.0)),
            Ease::Hold => None,
            Ease::Curve { x1, y1, x2, y2 } => {
                let f = |v: f32, lo: f32, hi: f32| if v.is_finite() { v.clamp(lo, hi) } else { 0.5 };
                Some((f(x1, 0.0, 1.0), f(y1, -2.0, 3.0), f(x2, 0.0, 1.0), f(y2, -2.0, 3.0)))
            }
        }
    }

    /// Progress (0 at the first key, 1 at the next; curves may overshoot) at time `t` in 0…1.
    pub fn apply(self, t: f32) -> f32 {
        let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
        let Some((x1, y1, x2, y2)) = self.handles() else { return if t >= 1.0 { 1.0 } else { 0.0 } };
        if self == Ease::Linear {
            return t;
        }
        // x(s) is monotonic for handle x in 0…1: bisect for s, then read y(s)
        let bez = |a: f32, b: f32, s: f32| {
            let u = 1.0 - s;
            3.0 * u * u * s * a + 3.0 * u * s * s * b + s * s * s
        };
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for _ in 0..32 {
            let mid = (lo + hi) * 0.5;
            if bez(x1, x2, mid) < t {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        bez(y1, y2, (lo + hi) * 0.5)
    }

    pub fn label(self) -> &'static str {
        match self {
            Ease::Linear => "Linear",
            Ease::In => "Ease in",
            Ease::Out => "Ease out",
            Ease::InOut => "Ease in and out",
            Ease::Hold => "Hold",
            Ease::Curve { .. } => "Custom curve",
        }
    }
}

/// A peg's transform at one frame: move by `x`, `y`; rotate by `angle` degrees clockwise; scale;
/// skew (degrees, horizontal); all about the peg's pivot. `ease` shapes the way to the next key.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PegKey {
    pub frame: u32,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub angle: f32,
    #[serde(default = "one")]
    pub scale_x: f32,
    #[serde(default = "one")]
    pub scale_y: f32,
    #[serde(default)]
    pub skew: f32,
    #[serde(default)]
    pub ease: Ease,
}

fn one() -> f32 {
    1.0
}

impl PegKey {
    /// No change, at `frame`.
    pub fn rest(frame: u32) -> Self {
        Self { frame, x: 0.0, y: 0.0, angle: 0.0, scale_x: 1.0, scale_y: 1.0, skew: 0.0, ease: Ease::default() }
    }

    pub fn sanitized(self) -> Self {
        let f = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        let scale = |v: f32| {
            let v = f(v, -100.0, 100.0, 1.0);
            if v.abs() < 0.001 { 0.001f32.copysign(v) } else { v }
        };
        Self {
            frame: self.frame,
            x: f(self.x, -1.0e6, 1.0e6, 0.0),
            y: f(self.y, -1.0e6, 1.0e6, 0.0),
            angle: f(self.angle, -36_000.0, 36_000.0, 0.0),
            scale_x: scale(self.scale_x),
            scale_y: scale(self.scale_y),
            skew: f(self.skew, -80.0, 80.0, 0.0),
            ease: match self.ease.handles() {
                Some((x1, y1, x2, y2)) if matches!(self.ease, Ease::Curve { .. }) => Ease::Curve { x1, y1, x2, y2 },
                _ => self.ease,
            },
        }
    }

    /// The transform about `pivot`: scale, skew, rotate, then move.
    pub fn affine(&self, pivot: (f32, f32)) -> Affine {
        let (s, c) = self.angle.to_radians().sin_cos();
        let k = self.skew.to_radians().tan();
        // R · K · S with K = [[1, k], [0, 1]]
        let (m00, m01, m10, m11) = (self.scale_x, k * self.scale_y, 0.0, self.scale_y);
        let (a, b, cc, d) = (c * m00 - s * m10, c * m01 - s * m11, s * m00 + c * m10, s * m01 + c * m11);
        let (px, py) = pivot;
        Affine { a, b, c: cc, d, tx: px + self.x - (a * px + b * py), ty: py + self.y - (cc * px + d * py) }
    }
}

/// One keyed value of a peg, as the curve editor plots it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    X,
    Y,
    Angle,
    ScaleX,
    ScaleY,
    Skew,
}

impl Channel {
    pub const ALL: [Channel; 6] = [Channel::X, Channel::Y, Channel::Angle, Channel::ScaleX, Channel::ScaleY, Channel::Skew];

    pub fn get(self, k: &PegKey) -> f32 {
        match self {
            Channel::X => k.x,
            Channel::Y => k.y,
            Channel::Angle => k.angle,
            Channel::ScaleX => k.scale_x,
            Channel::ScaleY => k.scale_y,
            Channel::Skew => k.skew,
        }
    }

    /// Set the value; the caller sanitizes the key afterwards.
    pub fn set(self, k: &mut PegKey, v: f32) {
        match self {
            Channel::X => k.x = v,
            Channel::Y => k.y = v,
            Channel::Angle => k.angle = v,
            Channel::ScaleX => k.scale_x = v,
            Channel::ScaleY => k.scale_y = v,
            Channel::Skew => k.skew = v,
        }
    }

    /// One keyboard nudge of this value (Shift nudges ten times as far).
    pub fn step(self) -> f32 {
        match self {
            Channel::X | Channel::Y | Channel::Angle | Channel::Skew => 1.0,
            Channel::ScaleX | Channel::ScaleY => 0.01,
        }
    }
}

/// Most points [`Peg::sample`] returns, whatever is asked.
pub const MAX_SAMPLES: usize = 4096;

/// A peg: a named transform with keys, a pivot and an optional parent peg.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Peg {
    pub id: PegId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<PegId>,
    /// Canvas point it rotates and scales about.
    #[serde(default)]
    pub pivot: (f32, f32),
    /// Sorted by frame, at most one per frame.
    #[serde(default)]
    pub keys: Vec<PegKey>,
}

impl Peg {
    /// The peg's own value at `frame` (rest without keys; the nearest key's value outside the
    /// keyed range).
    pub fn at(&self, frame: u32) -> PegKey {
        let Some(first) = self.keys.first() else { return PegKey::rest(frame) };
        if frame <= first.frame {
            return PegKey { frame, ..*first };
        }
        for pair in self.keys.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if frame >= a.frame && frame <= b.frame {
                let t = a.ease.apply((frame - a.frame) as f32 / (b.frame - a.frame).max(1) as f32);
                let l = |x: f32, y: f32| x + (y - x) * t;
                return PegKey {
                    frame,
                    x: l(a.x, b.x),
                    y: l(a.y, b.y),
                    angle: l(a.angle, b.angle),
                    scale_x: l(a.scale_x, b.scale_x),
                    scale_y: l(a.scale_y, b.scale_y),
                    skew: l(a.skew, b.skew),
                    ease: a.ease,
                };
            }
        }
        let last = self.keys.last().copied().unwrap_or(PegKey::rest(frame));
        PegKey { frame, ..last }
    }

    /// The value at a fractional time `t` (frames), for drawing smooth curves; whole frames give
    /// the same values as [`Peg::at`]. Finds the segment by binary search.
    pub fn at_time(&self, t: f32) -> PegKey {
        let t = if t.is_finite() { t.max(0.0) } else { 0.0 };
        let (Some(first), Some(last)) = (self.keys.first(), self.keys.last()) else { return PegKey::rest(0) };
        if t <= first.frame as f32 {
            return *first;
        }
        if t >= last.frame as f32 {
            return *last;
        }
        // the last key at or before t
        let i = self.keys.partition_point(|k| k.frame as f32 <= t).saturating_sub(1);
        let (Some(a), Some(b)) = (self.keys.get(i), self.keys.get(i + 1)) else { return *last };
        let u = a.ease.apply((t - a.frame as f32) / (b.frame - a.frame).max(1) as f32);
        let l = |x: f32, y: f32| x + (y - x) * u;
        PegKey {
            frame: a.frame,
            x: l(a.x, b.x),
            y: l(a.y, b.y),
            angle: l(a.angle, b.angle),
            scale_x: l(a.scale_x, b.scale_x),
            scale_y: l(a.scale_y, b.scale_y),
            skew: l(a.skew, b.skew),
            ease: a.ease,
        }
    }

    /// `count` evenly spaced (time, value) points of one channel from frame `from` to `to`
    /// (at most [`MAX_SAMPLES`]; work is `count · log keys`).
    pub fn sample(&self, channel: Channel, from: f32, to: f32, count: usize) -> Vec<(f32, f32)> {
        let count = count.clamp(2, MAX_SAMPLES);
        let (from, to) = (if from.is_finite() { from } else { 0.0 }, if to.is_finite() { to } else { 0.0 });
        (0..count)
            .map(|i| {
                let t = from + (to - from) * i as f32 / (count - 1) as f32;
                (t, channel.get(&self.at_time(t)))
            })
            .collect()
    }

    /// Move the keys at `frames` by `by_frames` and, with a `channel`, add `by_value` to that
    /// value, as one change: the moved keys keep their order, and nothing changes when a key is
    /// missing, would land before frame 0 or past `last`, or on a key that is not moving. Returns
    /// the moved keys' new frames.
    pub fn move_keys(&mut self, frames: &[u32], by_frames: i64, channel: Option<Channel>, by_value: f32, last: u32) -> Result<Vec<u32>, crate::DocError> {
        let moving: std::collections::BTreeSet<u32> = frames.iter().copied().take(MAX_PEG_KEYS).collect();
        for f in &moving {
            if !self.has_key(*f) {
                return Err(crate::DocError::NoKey(*f));
            }
        }
        let by_value = if by_value.is_finite() { by_value } else { 0.0 };
        let mut keys = Vec::with_capacity(self.keys.len());
        let mut moved = Vec::with_capacity(moving.len());
        for k in &self.keys {
            if !moving.contains(&k.frame) {
                keys.push(*k);
                continue;
            }
            let to = k.frame as i64 + by_frames.clamp(-(MAX_FRAMES_I64), MAX_FRAMES_I64);
            if to < 0 {
                return Err(crate::DocError::KeyBeforeStart);
            }
            let to = u32::try_from(to).map_err(|_| crate::DocError::NoFrame(u32::MAX))?;
            if to > last {
                return Err(crate::DocError::NoFrame(to));
            }
            if self.has_key(to) && !moving.contains(&to) {
                return Err(crate::DocError::KeyInTheWay(to));
            }
            let mut nk = PegKey { frame: to, ..*k };
            if let Some(c) = channel {
                c.set(&mut nk, c.get(k) + by_value);
            }
            keys.push(nk.sanitized());
            moved.push(to);
        }
        keys.sort_by_key(|k| k.frame);
        self.keys = keys;
        Ok(moved)
    }

    /// Give every key at `frames` the same ease (handle values are clamped); returns how many
    /// keys changed.
    pub fn set_eases(&mut self, frames: &[u32], ease: Ease) -> usize {
        let ease = PegKey { ease, ..PegKey::rest(0) }.sanitized().ease;
        let set: std::collections::BTreeSet<u32> = frames.iter().copied().take(MAX_PEG_KEYS).collect();
        let mut n = 0;
        for k in &mut self.keys {
            if set.contains(&k.frame) {
                k.ease = ease;
                n += 1;
            }
        }
        n
    }

    /// Add or replace the key at its frame.
    pub fn set_key(&mut self, key: PegKey) {
        let key = key.sanitized();
        match self.keys.binary_search_by_key(&key.frame, |k| k.frame) {
            Ok(i) => self.keys[i] = key,
            Err(i) => {
                if self.keys.len() < MAX_PEG_KEYS {
                    self.keys.insert(i, key);
                }
            }
        }
    }

    pub fn remove_key(&mut self, frame: u32) -> bool {
        match self.keys.binary_search_by_key(&frame, |k| k.frame) {
            Ok(i) => {
                self.keys.remove(i);
                true
            }
            Err(_) => false,
        }
    }

    /// Whether a key sits at `frame`.
    pub fn has_key(&self, frame: u32) -> bool {
        self.keys.binary_search_by_key(&frame, |k| k.frame).is_ok()
    }
}

/// The transform of a peg and every peg above it at `frame` (drawing pixels → canvas pixels).
/// Chains deeper than [`MAX_PEG_DEPTH`] or looping back on themselves stop there.
pub fn chain(pegs: &[Peg], id: PegId, frame: u32) -> Affine {
    let mut t = Affine::IDENTITY;
    let mut next = Some(id);
    let mut seen: Vec<PegId> = Vec::new();
    while let Some(i) = next {
        if seen.contains(&i) || seen.len() >= MAX_PEG_DEPTH {
            break;
        }
        seen.push(i);
        let Some(p) = pegs.iter().find(|p| p.id == i) else { break };
        t = t.then(&p.at(frame).affine(p.pivot));
        next = p.parent;
    }
    t
}

/// Whether hanging `id` from `parent` would make a loop (or too deep a chain).
pub fn would_loop(pegs: &[Peg], id: PegId, parent: PegId) -> bool {
    let mut next = Some(parent);
    let mut n = 0;
    while let Some(i) = next {
        if i == id || n >= MAX_PEG_DEPTH {
            return true;
        }
        n += 1;
        next = pegs.iter().find(|p| p.id == i).and_then(|p| p.parent);
    }
    false
}

/// What a drag on the canvas does to a peg.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PegDrag {
    /// Move by the drag.
    Move,
    /// Scale both ways about the pivot by how much further the pointer is from it.
    Scale,
    /// Turn about the pivot by the angle the pointer swept.
    Rotate,
}

/// The transform of everything above `id` (its parent chain) at `frame`.
fn parent_chain(pegs: &[Peg], id: PegId, frame: u32) -> Affine {
    match pegs.iter().find(|p| p.id == id).and_then(|p| p.parent) {
        Some(parent) => chain(pegs, parent, frame),
        None => Affine::IDENTITY,
    }
}

/// Where the peg's pivot is on the canvas at `frame`.
pub fn pivot_on_canvas(pegs: &[Peg], id: PegId, frame: u32) -> Option<(f32, f32)> {
    let p = pegs.iter().find(|p| p.id == id)?;
    Some(chain(pegs, id, frame).apply(p.pivot.0, p.pivot.1))
}

/// The key a canvas drag from `from` to `to` (canvas pixels) makes on peg `id` at `frame`,
/// starting from the peg's value there. `None` when the peg is missing, the parent chain cannot
/// be inverted, or the drag is too close to the pivot to measure.
pub fn dragged_key(pegs: &[Peg], id: PegId, frame: u32, kind: PegDrag, from: (f32, f32), to: (f32, f32)) -> Option<PegKey> {
    let p = pegs.iter().find(|p| p.id == id)?;
    let finite = |v: (f32, f32)| v.0.is_finite() && v.1.is_finite();
    if !finite(from) || !finite(to) {
        return None;
    }
    let mut k = p.at(frame);
    k.frame = frame;
    let parent = parent_chain(pegs, id, frame);
    let inv = parent.inverse()?;
    match kind {
        PegDrag::Move => {
            let (ax, ay) = inv.apply(from.0, from.1);
            let (bx, by) = inv.apply(to.0, to.1);
            k.x += bx - ax;
            k.y += by - ay;
        }
        PegDrag::Scale | PegDrag::Rotate => {
            let c = pivot_on_canvas(pegs, id, frame)?;
            let (a, b) = ((from.0 - c.0, from.1 - c.1), (to.0 - c.0, to.1 - c.1));
            let (la, lb) = (a.0.hypot(a.1), b.0.hypot(b.1));
            if la < 1.0 || lb < 1.0 {
                return None;
            }
            if kind == PegDrag::Scale {
                let r = lb / la;
                k.scale_x *= r;
                k.scale_y *= r;
            } else {
                let mut turn = (b.1.atan2(b.0) - a.1.atan2(a.0)).to_degrees();
                if turn > 180.0 {
                    turn -= 360.0;
                } else if turn < -180.0 {
                    turn += 360.0;
                }
                // a mirrored parent turns the other way
                if parent.a * parent.d - parent.b * parent.c < 0.0 {
                    turn = -turn;
                }
                k.angle += turn;
            }
        }
    }
    Some(k.sanitized())
}

/// The pivot (in the peg's own space) that puts the pivot under the canvas point `at` at
/// `frame`. The drawing keeps its place where the peg is not turned or scaled; where it is, it
/// now turns and scales about the new point.
pub fn pivot_under(pegs: &[Peg], id: PegId, frame: u32, at: (f32, f32)) -> Option<(f32, f32)> {
    let p = pegs.iter().find(|p| p.id == id)?;
    let inv = parent_chain(pegs, id, frame).inverse()?;
    let k = p.at(frame);
    let (x, y) = inv.apply(at.0, at.1);
    let f = |v: f32| if v.is_finite() { v.clamp(-1.0e6, 1.0e6) } else { 0.0 };
    Some((f(x - k.x), f(y - k.y)))
}

/// Repair pegs read from a file: bounded counts, sorted unique keys, unique ids, no loops and no
/// parents that do not exist.
pub fn sanitize(mut pegs: Vec<Peg>) -> Vec<Peg> {
    pegs.truncate(MAX_PEGS);
    let mut ids = std::collections::BTreeSet::new();
    pegs.retain(|p| ids.insert(p.id));
    for p in &mut pegs {
        p.name = p.name.chars().take(200).collect();
        let f = |v: f32| if v.is_finite() { v.clamp(-1.0e6, 1.0e6) } else { 0.0 };
        p.pivot = (f(p.pivot.0), f(p.pivot.1));
        p.keys.truncate(MAX_PEG_KEYS);
        p.keys = p.keys.iter().map(|k| k.sanitized()).collect();
        p.keys.sort_by_key(|k| k.frame);
        p.keys.dedup_by_key(|k| k.frame);
        if p.parent.is_some_and(|q| !ids.contains(&q)) {
            p.parent = None;
        }
    }
    for i in 0..pegs.len() {
        if let Some(parent) = pegs[i].parent {
            let id = pegs[i].id;
            // test the chain above the parent without this link
            pegs[i].parent = None;
            if !would_loop(&pegs, id, parent) {
                pegs[i].parent = Some(parent);
            }
        }
    }
    pegs
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn peg(id: u64, parent: Option<u64>) -> Peg {
        Peg { id: PegId(id), name: format!("P{id}"), parent: parent.map(PegId), pivot: (0.0, 0.0), keys: Vec::new() }
    }

    #[test]
    fn eases_start_at_zero_and_end_at_one() {
        for e in Ease::PRESETS.into_iter().chain([Ease::Curve { x1: 0.2, y1: 1.4, x2: 0.6, y2: -0.3 }]) {
            assert!(e.apply(0.0).abs() < 1e-4, "{e:?}");
            assert!((e.apply(1.0) - 1.0).abs() < 1e-4, "{e:?}");
        }
        assert!(Ease::In.apply(0.25) < 0.25 && Ease::Out.apply(0.25) > 0.25);
        assert!((Ease::InOut.apply(0.5) - 0.5).abs() < 1e-3);
        assert_eq!(Ease::Hold.apply(0.99), 0.0);
        assert!((Ease::Linear.apply(0.3) - 0.3).abs() < 1e-6);
    }

    #[test]
    fn keys_interpolate_with_their_ease() {
        let mut p = peg(1, None);
        assert_eq!(p.at(5), PegKey::rest(5));
        p.set_key(PegKey { x: 0.0, ease: Ease::Linear, ..PegKey::rest(0) });
        p.set_key(PegKey { x: 100.0, angle: 90.0, ..PegKey::rest(10) });
        assert!((p.at(5).x - 50.0).abs() < 1e-4);
        assert!((p.at(5).angle - 45.0).abs() < 1e-4);
        assert_eq!(p.at(50).x, 100.0);
        p.set_key(PegKey { x: 0.0, ease: Ease::Hold, ..PegKey::rest(0) });
        assert_eq!(p.at(9).x, 0.0);
        assert_eq!(p.at(10).x, 100.0);
        assert!(p.has_key(10) && p.remove_key(10) && !p.remove_key(10));
    }

    #[test]
    fn key_transform_is_about_the_pivot() {
        let k = PegKey { angle: 90.0, ..PegKey::rest(0) };
        let t = k.affine((10.0, 10.0));
        let (x, y) = t.apply(10.0, 10.0);
        assert!((x - 10.0).abs() < 1e-4 && (y - 10.0).abs() < 1e-4, "pivot stays");
        let (x, y) = t.apply(20.0, 10.0);
        assert!((x - 10.0).abs() < 1e-4 && (y - 20.0).abs() < 1e-4, "clockwise on screen (y down): {x} {y}");
        let s = PegKey { scale_x: 2.0, skew: 45.0, x: 5.0, ..PegKey::rest(0) }.affine((0.0, 0.0));
        let (x, y) = s.apply(1.0, 1.0);
        assert!((x - 8.0).abs() < 1e-4 && (y - 1.0).abs() < 1e-4, "{x} {y}");
    }

    #[test]
    fn nested_pegs_compose_child_then_parent() {
        let mut parent = peg(1, None);
        parent.set_key(PegKey { x: 100.0, ..PegKey::rest(0) });
        let mut child = peg(2, Some(1));
        child.set_key(PegKey { angle: 90.0, ..PegKey::rest(0) });
        let pegs = vec![parent, child];
        let (x, y) = chain(&pegs, PegId(2), 0).apply(10.0, 0.0);
        assert!((x - 100.0).abs() < 1e-3 && (y - 10.0).abs() < 1e-3, "rotate about the child's pivot, then the parent moves it: {x} {y}");
        assert!(would_loop(&pegs, PegId(1), PegId(2)));
        assert!(!would_loop(&pegs, PegId(2), PegId(1)));
    }

    #[test]
    fn sanitize_breaks_loops_and_drops_bad_values() {
        let mut a = peg(1, Some(2));
        a.keys = vec![PegKey { x: f32::NAN, scale_x: 0.0, ..PegKey::rest(3) }, PegKey::rest(3), PegKey::rest(1)];
        let b = peg(2, Some(1));
        let c = peg(3, Some(99));
        let dup = peg(2, None);
        let s = sanitize(vec![a, b, c, dup]);
        assert_eq!(s.len(), 3);
        assert!(s.iter().all(|p| !p.parent.is_some_and(|q| would_loop(&s, p.id, q))));
        assert_eq!(s[2].parent, None);
        assert_eq!(s[0].keys.iter().map(|k| k.frame).collect::<Vec<_>>(), [1, 3]);
        assert!(s[0].keys.iter().all(|k| k.x.is_finite() && k.scale_x.abs() >= 0.001));
        // a looping chain still terminates
        let looped = vec![peg(1, Some(2)), peg(2, Some(1))];
        assert!(chain(&looped, PegId(1), 0).is_finite());
    }

    #[test]
    fn canvas_drags_become_keys_that_match_typed_values() {
        let mut body = peg(1, None);
        body.pivot = (100.0, 100.0);
        body.set_key(PegKey { x: 10.0, ..PegKey::rest(0) });
        let pegs = vec![body.clone()];
        // move: the drag distance is the key's change
        let k = dragged_key(&pegs, PegId(1), 0, PegDrag::Move, (5.0, 5.0), (35.0, -15.0)).unwrap();
        assert_eq!((k.x, k.y), (40.0, -20.0));
        // turn a quarter about the pivot (now at 110, 100), clockwise on screen
        let k = dragged_key(&pegs, PegId(1), 0, PegDrag::Rotate, (160.0, 100.0), (110.0, 150.0)).unwrap();
        assert!((k.angle - 90.0).abs() < 1e-3, "{}", k.angle);
        // scale: twice as far from the pivot
        let k = dragged_key(&pegs, PegId(1), 0, PegDrag::Scale, (130.0, 100.0), (150.0, 100.0)).unwrap();
        assert!((k.scale_x - 2.0).abs() < 1e-4 && (k.scale_y - 2.0).abs() < 1e-4);
        assert!(dragged_key(&pegs, PegId(1), 0, PegDrag::Scale, (110.0, 100.0), (150.0, 100.0)).is_none(), "on the pivot");
        assert!(dragged_key(&pegs, PegId(9), 0, PegDrag::Move, (0.0, 0.0), (1.0, 1.0)).is_none());
        assert!(dragged_key(&pegs, PegId(1), 0, PegDrag::Move, (f32::NAN, 0.0), (1.0, 1.0)).is_none());
        // the drag and the same values typed give the same picture
        let mut typed = body.clone();
        typed.set_key(PegKey { x: 40.0, y: -20.0, ..PegKey::rest(0) });
        let mut dragged = body;
        dragged.set_key(dragged_key(&pegs, PegId(1), 0, PegDrag::Move, (5.0, 5.0), (35.0, -15.0)).unwrap());
        assert_eq!(typed.at(0), dragged.at(0));
    }

    #[test]
    fn a_child_peg_moves_with_the_pointer_under_a_turned_parent() {
        let mut parent = peg(1, None);
        parent.set_key(PegKey { angle: 90.0, scale_x: 2.0, scale_y: 2.0, ..PegKey::rest(0) });
        let child = peg(2, Some(1));
        let mut pegs = vec![parent, child];
        let before = chain(&pegs, PegId(2), 0).apply(3.0, 4.0);
        let k = dragged_key(&pegs, PegId(2), 0, PegDrag::Move, (0.0, 0.0), (20.0, -6.0)).unwrap();
        pegs[1].set_key(k);
        let after = chain(&pegs, PegId(2), 0).apply(3.0, 4.0);
        assert!((after.0 - before.0 - 20.0).abs() < 1e-3 && (after.1 - before.1 + 6.0).abs() < 1e-3, "{before:?} {after:?}");
        // the pivot lands under the pointer
        let p = pivot_under(&pegs, PegId(2), 0, (50.0, 60.0)).unwrap();
        pegs[1].pivot = p;
        let (x, y) = pivot_on_canvas(&pegs, PegId(2), 0).unwrap();
        assert!((x - 50.0).abs() < 1e-3 && (y - 60.0).abs() < 1e-3, "{x} {y}");
    }

    #[test]
    fn fractional_time_matches_whole_frames_and_samples_are_bounded() {
        let mut p = peg(1, None);
        p.set_key(PegKey { x: 0.0, angle: 10.0, ease: Ease::In, ..PegKey::rest(2) });
        p.set_key(PegKey { x: 100.0, ease: Ease::Out, ..PegKey::rest(12) });
        p.set_key(PegKey { x: 40.0, ..PegKey::rest(20) });
        for f in 0..25 {
            let (a, b) = (p.at(f), p.at_time(f as f32));
            for c in Channel::ALL {
                assert!((c.get(&a) - c.get(&b)).abs() < 1e-3, "{c:?} at {f}");
            }
        }
        let s = p.sample(Channel::X, 0.0, 24.0, 1_000_000);
        assert_eq!(s.len(), MAX_SAMPLES);
        assert_eq!(s[0], (0.0, 0.0));
        assert!((s[s.len() - 1].1 - 40.0).abs() < 1e-4);
        assert_eq!(p.sample(Channel::X, f32::NAN, f32::INFINITY, 0).len(), 2);
    }

    #[test]
    fn keys_move_together_or_not_at_all() {
        let mut p = peg(1, None);
        for f in [0, 5, 10] {
            p.set_key(PegKey { x: f as f32, ..PegKey::rest(f) });
        }
        // 5 and 10 move right by 2, x + 7
        assert_eq!(p.move_keys(&[5, 10], 2, Some(Channel::X), 7.0, 100).unwrap(), [7, 12]);
        assert_eq!(p.keys.iter().map(|k| (k.frame, k.x)).collect::<Vec<_>>(), [(0, 0.0), (7, 12.0), (12, 17.0)]);
        let before = p.clone();
        assert!(matches!(p.move_keys(&[7], -7, None, 0.0, 100), Err(crate::DocError::KeyInTheWay(0))));
        assert!(matches!(p.move_keys(&[0, 7], -1, None, 0.0, 100), Err(crate::DocError::KeyBeforeStart)));
        assert!(matches!(p.move_keys(&[12], 5, None, 0.0, 15), Err(crate::DocError::NoFrame(17))));
        assert!(matches!(p.move_keys(&[3], 1, None, 0.0, 100), Err(crate::DocError::NoKey(3))));
        assert_eq!(p, before, "a refused move changes nothing");
        // keys moving onto each other's old frames is fine
        assert_eq!(p.move_keys(&[0, 7, 12], 7, None, 0.0, 100).unwrap(), [7, 14, 19]);
        // hostile values are clamped
        p.move_keys(&[7], 0, Some(Channel::ScaleX), f32::NAN, 100).unwrap();
        p.move_keys(&[7], 0, Some(Channel::ScaleX), -1.0, 100).unwrap();
        assert!(p.keys[0].scale_x.abs() >= 0.001 && p.keys[0].scale_x.is_finite());
        p.move_keys(&[14], i64::MIN, None, 0.0, u32::MAX).unwrap_err();
        p.move_keys(&[14], i64::MAX, None, 0.0, 100).unwrap_err();
    }

    #[test]
    fn eases_are_set_on_many_keys_with_handles_clamped() {
        let mut p = peg(1, None);
        for f in [0, 5, 10, 15] {
            p.set_key(PegKey::rest(f));
        }
        assert_eq!(p.set_eases(&[0, 10, 99], Ease::Hold), 2);
        assert_eq!(p.keys.iter().map(|k| k.ease).collect::<Vec<_>>(), [Ease::Hold, Ease::InOut, Ease::Hold, Ease::InOut]);
        p.set_eases(&[5], Ease::Curve { x1: 9.0, y1: f32::NAN, x2: -3.0, y2: 1e9 });
        assert_eq!(p.keys[1].ease, Ease::Curve { x1: 1.0, y1: 0.5, x2: 0.0, y2: 3.0 });
    }

    proptest! {
        #[test]
        fn hostile_keys_give_finite_transforms(vals in proptest::collection::vec(proptest::num::f32::ANY, 7), frame in 0u32..100, t in proptest::num::f32::ANY) {
            let k = PegKey { frame: 0, x: vals[0], y: vals[1], angle: vals[2], scale_x: vals[3], scale_y: vals[4], skew: vals[5],
                ease: Ease::Curve { x1: vals[6], y1: vals[0], x2: vals[1], y2: vals[2] } }.sanitized();
            let mut p = peg(1, None);
            p.set_key(k);
            p.set_key(PegKey::rest(50));
            // pivots are kept within ±10⁶ by `sanitize` and the pivot command
            let piv = |v: f32| if v.is_finite() { v.clamp(-1.0e6, 1.0e6) } else { 0.0 };
            prop_assert!(p.at(frame).affine((piv(vals[0]), piv(vals[1]))).is_finite());
            prop_assert!(k.ease.apply(t).is_finite());
        }
    }
}
