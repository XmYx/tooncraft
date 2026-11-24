//! Vector strokes (UI-free): drawings kept as editable centre lines with a width at every point,
//! so a line can be reshaped, thickened, smoothed or cut back to where it crosses another line
//! after it is drawn.
//!
//! A stroke is a list of points; the line passes through every point on a Catmull-Rom spline (a
//! public, textbook curve: each piece is a cubic through four neighbouring points). Drawn input
//! is thinned with the Ramer–Douglas–Peucker simplification (iterative, with an explicit stack).
//! Every count is capped and every number is sanitized: files and commands are hostile.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use serde::{Deserialize, Serialize};

/// Most strokes in one drawing.
pub const MAX_STROKES: usize = 4096;
/// Most points in one stroke.
pub const MAX_POINTS: usize = 4096;
/// Most segment pairs one crossing search looks at.
pub const MAX_PAIR_CHECKS: u64 = 4_000_000;
/// Widest line (pixels).
pub const MAX_WIDTH: f32 = 500.0;
/// Most points [`VStroke::samples`] returns.
pub const MAX_SAMPLES: usize = 200_000;
/// Coordinates are kept within ± this.
const MAX_COORD: f32 = 1.0e6;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum VectorError {
    #[error("there is no vector stroke {0}")]
    NoStroke(usize),
    #[error("vector stroke {0} has no point {1}")]
    NoPoint(usize, usize),
    #[error("a drawing can have at most {MAX_STROKES} vector strokes")]
    TooManyStrokes,
    #[error("a vector stroke needs at least one point")]
    Empty,
    #[error("too many crossings to search; simplify the drawing first")]
    TooMuchWork,
}

/// One point of a centre line: position and line width there (pixels).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct VPoint {
    pub x: f32,
    pub y: f32,
    pub width: f32,
}

impl VPoint {
    pub fn new(x: f32, y: f32, width: f32) -> Self {
        Self { x, y, width }.sanitized()
    }

    pub fn sanitized(self) -> Self {
        let c = |v: f32| if v.is_finite() { v.clamp(-MAX_COORD, MAX_COORD) } else { 0.0 };
        let w = if self.width.is_finite() { self.width.clamp(0.0, MAX_WIDTH) } else { 1.0 };
        Self { x: c(self.x), y: c(self.y), width: w }
    }
}

/// One line: its points and colour (straight RGBA).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VStroke {
    pub points: Vec<VPoint>,
    pub color: [u8; 4],
}

/// The vector strokes of one drawing, painted in order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VDrawing {
    pub strokes: Vec<VStroke>,
}

/// Distance from `p` to the segment `a`–`b`.
fn seg_dist(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (p.0 - (a.0 + t * dx)).hypot(p.1 - (a.1 + t * dy))
}

/// Keep the points needed to stay within `tolerance` pixels of the input line (Ramer–Douglas–
/// Peucker); widths stay with their points. The input is capped first.
pub fn simplify(points: &[VPoint], tolerance: f32) -> Vec<VPoint> {
    let pts: Vec<VPoint> = points.iter().take(MAX_POINTS * 8).map(|p| p.sanitized()).collect();
    if pts.len() <= 2 {
        return pts;
    }
    let tol = if tolerance.is_finite() { tolerance.max(0.0) } else { 0.5 };
    let last = pts.len() - 1;
    let mut keep = vec![false; pts.len()];
    if let Some(k) = keep.first_mut() {
        *k = true;
    }
    if let Some(k) = keep.last_mut() {
        *k = true;
    }
    let mut stack = vec![(0usize, last)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let (pa, pb) = (pts[a], pts[b]);
        let mut far = (0.0f32, a);
        let (dx, dy) = (pb.x - pa.x, pb.y - pa.y);
        let len2 = dx * dx + dy * dy;
        for (i, p) in pts.iter().enumerate().take(b).skip(a + 1) {
            // a change of width counts too, against the width the line would have there
            let t = if len2 > 0.0 { (((p.x - pa.x) * dx + (p.y - pa.y) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
            let w = pa.width + (pb.width - pa.width) * t;
            let d = seg_dist((p.x, p.y), (pa.x, pa.y), (pb.x, pb.y)) + (p.width - w).abs() * 0.5;
            if d > far.0 {
                far = (d, i);
            }
        }
        if far.0 > tol {
            keep[far.1] = true;
            stack.push((a, far.1));
            stack.push((far.1, b));
        }
    }
    let out: Vec<VPoint> = pts.iter().zip(&keep).filter(|(_, k)| **k).map(|(p, _)| *p).collect();
    if out.len() <= MAX_POINTS {
        return out;
    }
    // still too many: an even spread, ends included
    let n = out.len();
    (0..MAX_POINTS).filter_map(|i| out.get(i * (n - 1) / (MAX_POINTS - 1)).copied()).collect()
}

fn catmull_rom(p0: VPoint, p1: VPoint, p2: VPoint, p3: VPoint, t: f32) -> VPoint {
    let (t2, t3) = (t * t, t * t * t);
    let f = |a: f32, b: f32, c: f32, d: f32| 0.5 * (2.0 * b + (-a + c) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (-a + 3.0 * b - 3.0 * c + d) * t3);
    // widths blend straight between the two points, so they never overshoot
    VPoint::new(f(p0.x, p1.x, p2.x, p3.x), f(p0.y, p1.y, p2.y, p3.y), p1.width + (p2.width - p1.width) * t)
}

/// Where segment `a`–`b` crosses `c`–`d`, as the fraction along `a`–`b`.
fn cross(a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32)) -> Option<f32> {
    let r = (b.0 - a.0, b.1 - a.1);
    let s = (d.0 - c.0, d.1 - c.1);
    let den = r.0 * s.1 - r.1 * s.0;
    if den.abs() < 1e-9 {
        return None;
    }
    let q = (c.0 - a.0, c.1 - a.1);
    let t = (q.0 * s.1 - q.1 * s.0) / den;
    let u = (q.0 * r.1 - q.1 * r.0) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some(t)
}

impl VStroke {
    /// A stroke from drawn points, thinned to within `tolerance` pixels.
    pub fn from_points(points: &[VPoint], color: [u8; 4], tolerance: f32) -> Result<Self, VectorError> {
        let points = simplify(points, tolerance);
        if points.is_empty() {
            return Err(VectorError::Empty);
        }
        Ok(Self { points, color })
    }

    pub fn sanitized(mut self) -> Option<Self> {
        self.points.truncate(MAX_POINTS);
        self.points = self.points.into_iter().map(VPoint::sanitized).collect();
        (!self.points.is_empty()).then_some(self)
    }

    /// Points along the curve about every `step` pixels (at most [`MAX_SAMPLES`]), passing
    /// through every stored point.
    pub fn samples(&self, step: f32) -> Vec<VPoint> {
        let p = &self.points;
        let step = if step.is_finite() { step.max(0.25) } else { 1.0 };
        if p.len() < 2 {
            return p.clone();
        }
        let n = p.len() as isize;
        let at = |i: isize| -> VPoint { p[i.clamp(0, n - 1) as usize] };
        let mut out = Vec::new();
        for i in 0..n - 1 {
            let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
            let len = (p2.x - p1.x).hypot(p2.y - p1.y);
            let parts = ((len / step).ceil() as usize).clamp(1, 4096);
            for k in 0..parts {
                if out.len() >= MAX_SAMPLES - 1 {
                    break;
                }
                out.push(catmull_rom(p0, p1, p2, p3, k as f32 / parts as f32));
            }
        }
        if let Some(l) = p.last() {
            out.push(*l);
        }
        out
    }

    /// Move each inner point part of the way towards the middle of its neighbours (`strength`
    /// 0…1, at most 16 `passes`); the ends stay put.
    pub fn smooth(&mut self, strength: f32, passes: u32) {
        let s = if strength.is_finite() { strength.clamp(0.0, 1.0) } else { 0.0 };
        for _ in 0..passes.min(16) {
            let old = self.points.clone();
            for (i, w) in old.windows(3).enumerate() {
                let mid = |u: f32, v: f32, x: f32| v + ((u + x) * 0.5 - v) * s;
                if let Some(p) = self.points.get_mut(i + 1) {
                    *p = VPoint::new(mid(w[0].x, w[1].x, w[2].x), mid(w[0].y, w[1].y, w[2].y), mid(w[0].width, w[1].width, w[2].width));
                }
            }
        }
    }

    /// The polyline crossings are searched on (curve samples every 2 px).
    fn polyline(&self) -> Vec<(f32, f32)> {
        self.samples(2.0).iter().map(|p| (p.x, p.y)).collect()
    }
}

impl VDrawing {
    pub fn sanitized(mut self) -> Self {
        self.strokes.truncate(MAX_STROKES);
        self.strokes = self.strokes.into_iter().filter_map(VStroke::sanitized).collect();
        self
    }

    pub fn add(&mut self, stroke: VStroke) -> Result<usize, VectorError> {
        if self.strokes.len() >= MAX_STROKES {
            return Err(VectorError::TooManyStrokes);
        }
        self.strokes.push(stroke.sanitized().ok_or(VectorError::Empty)?);
        Ok(self.strokes.len() - 1)
    }

    fn stroke_mut(&mut self, i: usize) -> Result<&mut VStroke, VectorError> {
        self.strokes.get_mut(i).ok_or(VectorError::NoStroke(i))
    }

    /// Move one point.
    pub fn move_point(&mut self, stroke: usize, point: usize, x: f32, y: f32) -> Result<(), VectorError> {
        let s = self.stroke_mut(stroke)?;
        let p = s.points.get_mut(point).ok_or(VectorError::NoPoint(stroke, point))?;
        *p = VPoint::new(x, y, p.width);
        Ok(())
    }

    /// Set the width at one point, or (`point` = `None`) scale the stroke so its widest point
    /// is `width`.
    pub fn set_width(&mut self, stroke: usize, point: Option<usize>, width: f32) -> Result<(), VectorError> {
        let s = self.stroke_mut(stroke)?;
        match point {
            Some(i) => {
                let p = s.points.get_mut(i).ok_or(VectorError::NoPoint(stroke, i))?;
                *p = VPoint::new(p.x, p.y, width);
            }
            None => {
                let widest = s.points.iter().map(|p| p.width).fold(0.0f32, f32::max);
                for p in &mut s.points {
                    let w = if widest > 0.0 { p.width * width / widest } else { width };
                    *p = VPoint::new(p.x, p.y, w);
                }
            }
        }
        Ok(())
    }

    /// Smooth one stroke.
    pub fn smooth(&mut self, stroke: usize, strength: f32, passes: u32) -> Result<(), VectorError> {
        self.stroke_mut(stroke)?.smooth(strength, passes);
        Ok(())
    }

    /// The stroke and point nearest `(x, y)` within `within` pixels.
    pub fn nearest_point(&self, x: f32, y: f32, within: f32) -> Option<(usize, usize)> {
        let mut best: Option<(f32, usize, usize)> = None;
        for (si, s) in self.strokes.iter().enumerate() {
            for (pi, p) in s.points.iter().enumerate() {
                let d = (p.x - x).hypot(p.y - y);
                if d <= within && best.is_none_or(|b| d < b.0) {
                    best = Some((d, si, pi));
                }
            }
        }
        best.map(|(_, s, p)| (s, p))
    }

    /// The stroke whose line passes nearest `(x, y)` within `within` pixels plus half its width.
    pub fn stroke_at(&self, x: f32, y: f32, within: f32) -> Option<usize> {
        let mut best: Option<(f32, usize)> = None;
        for (si, s) in self.strokes.iter().enumerate() {
            let line = s.polyline();
            let d = match line.len() {
                0 => continue,
                1 => (line[0].0 - x).hypot(line[0].1 - y),
                _ => line.windows(2).map(|w| seg_dist((x, y), w[0], w[1])).fold(f32::INFINITY, f32::min),
            };
            let reach = within + s.points.iter().map(|p| p.width).fold(0.0f32, f32::max) * 0.5;
            if d <= reach && best.is_none_or(|b| d <= b.0) {
                best = Some((d, si));
            }
        }
        best.map(|(_, s)| s)
    }

    /// Remove the piece of `stroke` around the point nearest `(x, y)` that runs between the
    /// neighbouring crossings with any stroke (itself included) or to the stroke's ends. What is
    /// left stays as up to two strokes in its place. Refused (nothing changes) when the search
    /// would look at more than [`MAX_PAIR_CHECKS`] segment pairs.
    pub fn erase_to_crossings(&mut self, stroke: usize, x: f32, y: f32) -> Result<(), VectorError> {
        let s = self.strokes.get(stroke).ok_or(VectorError::NoStroke(stroke))?;
        let samples = s.samples(2.0);
        let line: Vec<(f32, f32)> = samples.iter().map(|p| (p.x, p.y)).collect();
        if line.len() < 2 {
            self.strokes.remove(stroke);
            return Ok(());
        }
        let others: Vec<Vec<(f32, f32)>> = self.strokes.iter().map(VStroke::polyline).collect();
        let pairs: u64 = others.iter().map(|o| o.len().saturating_sub(1) as u64).sum::<u64>() * (line.len() as u64 - 1);
        if pairs > MAX_PAIR_CHECKS {
            return Err(VectorError::TooMuchWork);
        }
        // crossings as positions along the line: segment index plus fraction
        let mut at: Vec<f32> = Vec::new();
        for (oi, other) in others.iter().enumerate() {
            for (i, w) in line.windows(2).enumerate() {
                for (j, v) in other.windows(2).enumerate() {
                    // a line touches itself at neighbouring segments; those are not crossings
                    if oi == stroke && i.abs_diff(j) <= 1 {
                        continue;
                    }
                    if let Some(t) = cross(w[0], w[1], v[0], v[1]) {
                        at.push(i as f32 + t);
                    }
                }
            }
        }
        let mut near = (f32::INFINITY, 0.0f32);
        for (i, w) in line.windows(2).enumerate() {
            let (dx, dy) = (w[1].0 - w[0].0, w[1].1 - w[0].1);
            let len2 = dx * dx + dy * dy;
            let t = if len2 > 0.0 { (((x - w[0].0) * dx + (y - w[0].1) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
            let d = (w[0].0 + t * dx - x).hypot(w[0].1 + t * dy - y);
            if d < near.0 {
                near = (d, i as f32 + t);
            }
        }
        let pos = near.1;
        let lo = at.iter().copied().filter(|a| *a < pos - 1e-4).fold(f32::NEG_INFINITY, f32::max);
        let hi = at.iter().copied().filter(|a| *a > pos + 1e-4).fold(f32::INFINITY, f32::min);
        let point = |a: f32| -> VPoint {
            let i = (a.floor().max(0.0) as usize).min(samples.len() - 2);
            let t = (a - i as f32).clamp(0.0, 1.0);
            let (p, q) = (samples[i], samples[i + 1]);
            VPoint::new(p.x + (q.x - p.x) * t, p.y + (q.y - p.y) * t, p.width + (q.width - p.width) * t)
        };
        let color = s.color;
        let mut pieces: Vec<Vec<VPoint>> = Vec::new();
        if lo.is_finite() {
            let mut a: Vec<VPoint> = samples.iter().take(lo.floor() as usize + 1).copied().collect();
            a.push(point(lo));
            pieces.push(a);
        }
        if hi.is_finite() {
            let mut b = vec![point(hi)];
            b.extend(samples.iter().skip(hi.floor() as usize + 1).copied());
            pieces.push(b);
        }
        self.strokes.remove(stroke);
        let mut index = stroke;
        for p in pieces {
            if let Ok(v) = VStroke::from_points(&p, color, 0.3)
                && v.points.len() >= 2
            {
                self.strokes.insert(index.min(self.strokes.len()), v);
                index += 1;
            }
        }
        Ok(())
    }

    /// Points in every stroke, for budgets.
    pub fn point_count(&self) -> usize {
        self.strokes.iter().map(|s| s.points.len()).sum()
    }
}

#[cfg(test)]
mod tests;
