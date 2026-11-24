//! Stroke rulers: guides that freehand strokes follow while they are on. A straight ruler, a
//! curve ruler through points, an ellipse ruler, concentric ellipses around a centre, perspective
//! lines towards one to three vanishing points, and the field-guide grid. Stored with the shot,
//! like symmetry, so a replayed stroke follows the same ruler.
//!
//! How a stroke picks its ruler: at the first sample, the nearest straight, curve or ellipse
//! ruler within [`Rulers::reach`] wins. Otherwise a concentric ruler takes the stroke on the
//! ellipse through that sample. Otherwise, once the pen has moved a few pixels, the perspective
//! line (or grid line) closest to the direction of travel takes it. A stroke started away from
//! every ruler stays freehand.

use serde::{Deserialize, Serialize};

/// Most rulers in a shot.
pub const MAX_RULERS: usize = 32;
/// Most points of a curve ruler.
pub const MAX_CURVE_POINTS: usize = 64;
/// Most vanishing points of a perspective ruler.
pub const MAX_VANISHING: usize = 3;
/// Largest coordinate a ruler keeps (canvas pixels).
const MAX_COORD: f32 = 1.0e6;
/// How far the pen moves before a direction-dependent ruler (perspective, grid) decides.
const DECIDE_AFTER: f32 = 4.0;
/// Most samples held back while a direction-dependent ruler decides.
pub const MAX_PENDING: usize = 256;

pub type Point = (f32, f32);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Ruler {
    /// The line through `a` and `b` (strokes run along it, past its ends too).
    Line { a: Point, b: Point },
    /// A smooth curve through `points` (2 … [`MAX_CURVE_POINTS`]).
    Curve { points: Vec<Point> },
    /// An ellipse: centre, radii and the turn of its first axis in radians.
    Ellipse { center: Point, rx: f32, ry: f32, angle: f32 },
    /// Ellipses of every size around `center`, `ratio` = minor / major radius.
    Concentric { center: Point, ratio: f32, angle: f32 },
    /// Lines through each vanishing point (1 … [`MAX_VANISHING`]).
    Perspective { points: Vec<Point> },
}

/// The shot's rulers and whether strokes follow them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rulers {
    /// Strokes follow the rulers.
    pub on: bool,
    pub list: Vec<Ruler>,
    /// Freehand strokes started near a field-guide grid line run along it.
    pub grid: bool,
    /// How close (canvas pixels) a stroke must start to a straight, curve or ellipse ruler or a
    /// grid line to follow it.
    pub reach: f32,
}

impl Default for Rulers {
    fn default() -> Self {
        Self { on: false, list: Vec::new(), grid: false, reach: 24.0 }
    }
}

fn ok(v: f32) -> bool {
    v.is_finite() && v.abs() <= MAX_COORD
}

fn ok_point(p: Point) -> bool {
    ok(p.0) && ok(p.1)
}

impl Ruler {
    /// `None` when the ruler cannot be used (non-finite or huge numbers, too few points).
    pub fn sanitized(self) -> Option<Ruler> {
        let tau = std::f32::consts::TAU;
        match self {
            Ruler::Line { a, b } => (ok_point(a) && ok_point(b) && dist(a, b) > 0.5).then_some(Ruler::Line { a, b }),
            Ruler::Curve { points } => {
                let points: Vec<Point> = points.into_iter().filter(|p| ok_point(*p)).take(MAX_CURVE_POINTS).collect();
                (points.len() >= 2).then_some(Ruler::Curve { points })
            }
            Ruler::Ellipse { center, rx, ry, angle } => (ok_point(center) && ok(rx) && ok(ry) && angle.is_finite()).then(|| Ruler::Ellipse {
                center,
                rx: rx.abs().clamp(1.0, 1.0e5),
                ry: ry.abs().clamp(1.0, 1.0e5),
                angle: angle % tau,
            }),
            Ruler::Concentric { center, ratio, angle } => (ok_point(center) && ratio.is_finite() && angle.is_finite()).then(|| Ruler::Concentric {
                center,
                ratio: ratio.abs().clamp(0.01, 100.0),
                angle: angle % tau,
            }),
            Ruler::Perspective { points } => {
                let points: Vec<Point> = points.into_iter().filter(|p| ok_point(*p)).take(MAX_VANISHING).collect();
                (!points.is_empty()).then_some(Ruler::Perspective { points })
            }
        }
    }

    /// The points a user drags to edit the ruler (handles), in order.
    pub fn handles(&self) -> Vec<Point> {
        match self {
            Ruler::Line { a, b } => vec![*a, *b],
            Ruler::Curve { points } | Ruler::Perspective { points } => points.clone(),
            Ruler::Ellipse { center, rx, ry, angle } => {
                let (s, c) = angle.sin_cos();
                vec![*center, (center.0 + rx * c, center.1 + rx * s), (center.0 - ry * s, center.1 + ry * c)]
            }
            Ruler::Concentric { center, .. } => vec![*center],
        }
    }

    /// Move handle `i` (as in [`Ruler::handles`]) to `p`.
    pub fn move_handle(&mut self, i: usize, p: Point) {
        if !ok_point(p) {
            return;
        }
        match self {
            Ruler::Line { a, b } => match i {
                0 => *a = p,
                1 => *b = p,
                _ => {}
            },
            Ruler::Curve { points } | Ruler::Perspective { points } => {
                if let Some(q) = points.get_mut(i) {
                    *q = p;
                }
            }
            Ruler::Ellipse { center, rx, ry, angle } => match i {
                0 => *center = p,
                1 => {
                    *rx = dist(*center, p).clamp(1.0, 1.0e5);
                    *angle = (p.1 - center.1).atan2(p.0 - center.0);
                }
                2 => *ry = dist(*center, p).clamp(1.0, 1.0e5),
                _ => {}
            },
            Ruler::Concentric { center, .. } => {
                if i == 0 {
                    *center = p;
                }
            }
        }
    }

    /// The ruler drawn as lines, for the canvas overlay (`w`×`h` is the canvas, for the
    /// perspective and concentric guides that have no size of their own).
    pub fn outline(&self, w: u32, h: u32) -> Vec<Vec<Point>> {
        let span = (w.max(h) as f32).max(1.0) * 2.0;
        match self {
            Ruler::Line { a, b } => {
                let d = unit(*a, *b);
                vec![vec![(a.0 - d.0 * span, a.1 - d.1 * span), (b.0 + d.0 * span, b.1 + d.1 * span)]]
            }
            Ruler::Curve { points } => vec![curve(points)],
            Ruler::Ellipse { center, rx, ry, angle } => vec![ellipse(*center, *rx, *ry, *angle)],
            Ruler::Concentric { center, ratio, angle } => {
                let step = (w.max(h) as f32 / 8.0).max(4.0);
                (1..=4).map(|k| ellipse(*center, step * k as f32, step * k as f32 * ratio, *angle)).collect()
            }
            Ruler::Perspective { points } => {
                let mut v = Vec::new();
                for &vp in points {
                    for k in 0..12 {
                        let t = std::f32::consts::TAU * k as f32 / 12.0;
                        v.push(vec![vp, (vp.0 + t.cos() * span, vp.1 + t.sin() * span)]);
                    }
                }
                v
            }
        }
    }
}

impl Rulers {
    pub fn sanitized(self) -> Self {
        let reach = if self.reach.is_finite() { self.reach.clamp(1.0, 500.0) } else { 24.0 };
        let list = self.list.into_iter().take(MAX_RULERS).filter_map(Ruler::sanitized).collect();
        Self { on: self.on, list, grid: self.grid, reach }
    }

    /// Whether any stroke would follow a ruler.
    pub fn is_on(&self) -> bool {
        self.on && (!self.list.is_empty() || self.grid)
    }

    /// A snapper for one stroke on a `w`×`h` canvas, or `None` when the rulers are off.
    pub fn snapper(&self, w: u32, h: u32) -> Option<Snapper> {
        self.is_on().then(|| Snapper { rulers: self.clone(), size: (w, h), state: State::Start })
    }
}

/// The path a stroke has settled on.
#[derive(Clone, Debug, PartialEq)]
enum Track {
    /// Freehand: no ruler near.
    Free,
    /// The infinite line through `p` along unit `d`.
    Line { p: Point, d: Point },
    /// A polyline (a curve or an ellipse as segments).
    Path(Vec<Point>),
}

#[derive(Clone, Debug, PartialEq)]
enum State {
    Start,
    /// Waiting for the pen to move: the first sample, the held samples and the lines to choose
    /// from by direction.
    Deciding {
        first: Point,
        held: Vec<Point>,
        lines: Vec<(Point, Point)>,
    },
    On(Track),
}

/// Maps one stroke's sample positions onto the ruler it picked.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapper {
    rulers: Rulers,
    size: (u32, u32),
    state: State,
}

impl Snapper {
    /// Feed one sample position; returns the positions to paint (none while deciding, several
    /// when a decision releases the held ones). Non-finite input passes through unchanged (the
    /// stroke engine drops it).
    pub fn feed(&mut self, p: Point) -> Vec<Point> {
        if !ok_point(p) {
            return vec![p];
        }
        match std::mem::replace(&mut self.state, State::Start) {
            State::Start => {
                let (track, lines) = self.pick(p);
                match track {
                    Some(t) => {
                        let out = vec![project(&t, p)];
                        self.state = State::On(t);
                        out
                    }
                    None if !lines.is_empty() => {
                        self.state = State::Deciding { first: p, held: vec![p], lines };
                        Vec::new()
                    }
                    None => {
                        self.state = State::On(Track::Free);
                        vec![p]
                    }
                }
            }
            State::Deciding { first, mut held, lines } => {
                held.push(p);
                if dist(first, p) < DECIDE_AFTER && held.len() < MAX_PENDING {
                    self.state = State::Deciding { first, held, lines };
                    return Vec::new();
                }
                let t = choose(first, p, &lines);
                let out = held.iter().map(|q| project(&t, *q)).collect();
                self.state = State::On(t);
                out
            }
            State::On(t) => {
                let out = vec![project(&t, p)];
                self.state = State::On(t);
                out
            }
        }
    }

    /// The end of the stroke: positions still held back (a short tap near a perspective ruler),
    /// projected onto the first line.
    pub fn finish(&mut self) -> Vec<Point> {
        match std::mem::replace(&mut self.state, State::On(Track::Free)) {
            State::Deciding { first, held, lines } => {
                let t = choose(first, first, &lines);
                held.iter().map(|q| project(&t, *q)).collect()
            }
            _ => Vec::new(),
        }
    }

    /// The ruler track for a stroke starting at `p`, or the lines to choose from by direction.
    fn pick(&self, p: Point) -> (Option<Track>, Vec<(Point, Point)>) {
        let reach = self.rulers.reach;
        let mut best: Option<(f32, Track)> = None;
        let mut concentric = None;
        let mut lines = Vec::new();
        for r in &self.rulers.list {
            let cand = match r {
                Ruler::Line { a, b } => Some(Track::Line { p: *a, d: unit(*a, *b) }),
                Ruler::Curve { points } => Some(Track::Path(curve(points))),
                Ruler::Ellipse { center, rx, ry, angle } => Some(Track::Path(ellipse(*center, *rx, *ry, *angle))),
                Ruler::Concentric { center, ratio, angle } => {
                    if concentric.is_none() {
                        let (s, c) = (-angle).sin_cos();
                        let (dx, dy) = (p.0 - center.0, p.1 - center.1);
                        let (u, v) = (dx * c - dy * s, dx * s + dy * c);
                        let rx = (u * u + (v / ratio) * (v / ratio)).sqrt().clamp(1.0, 1.0e5);
                        concentric = Some(Track::Path(ellipse(*center, rx, rx * ratio, *angle)));
                    }
                    None
                }
                Ruler::Perspective { points } => {
                    lines.extend(points.iter().filter(|vp| dist(**vp, p) > 0.5).map(|vp| (*vp, unit(*vp, p))));
                    None
                }
            };
            if let Some(t) = cand {
                let d = dist(p, project(&t, p));
                if d <= reach && best.as_ref().is_none_or(|(bd, _)| d < *bd) {
                    best = Some((d, t));
                }
            }
        }
        if let Some((_, t)) = best {
            return (Some(t), Vec::new());
        }
        if let Some(t) = concentric {
            return (Some(t), Vec::new());
        }
        if self.rulers.grid {
            let (xs, ys) = crate::guides::grid(self.size.0, self.size.1);
            if let Some(x) = nearest(&xs, p.0).filter(|x| (x - p.0).abs() <= reach) {
                lines.push(((x, p.1), (0.0, 1.0)));
            }
            if let Some(y) = nearest(&ys, p.1).filter(|y| (y - p.1).abs() <= reach) {
                lines.push(((p.0, y), (1.0, 0.0)));
            }
        }
        (None, lines)
    }
}

fn nearest(v: &[f32], x: f32) -> Option<f32> {
    v.iter().copied().min_by(|a, b| (a - x).abs().total_cmp(&(b - x).abs()))
}

/// The line whose direction is closest to the travel `first → now` (the first line when the
/// pen has not moved).
fn choose(first: Point, now: Point, lines: &[(Point, Point)]) -> Track {
    if dist(first, now) <= 0.0 {
        return lines.first().map(|(p, d)| Track::Line { p: *p, d: *d }).unwrap_or(Track::Free);
    }
    let m = unit(first, now);
    let mut best: Option<(f32, Track)> = None;
    for (p, d) in lines {
        let f = (d.0 * m.0 + d.1 * m.1).abs();
        if best.as_ref().is_none_or(|(bf, _)| f > *bf) {
            best = Some((f, Track::Line { p: *p, d: *d }));
        }
    }
    best.map(|(_, t)| t).unwrap_or(Track::Free)
}

fn dist(a: Point, b: Point) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// The unit vector from `a` to `b` (along x when they meet).
fn unit(a: Point, b: Point) -> Point {
    let l = dist(a, b);
    if l > 1e-6 { ((b.0 - a.0) / l, (b.1 - a.1) / l) } else { (1.0, 0.0) }
}

fn project(t: &Track, p: Point) -> Point {
    match t {
        Track::Free => p,
        Track::Line { p: o, d } => {
            let k = (p.0 - o.0) * d.0 + (p.1 - o.1) * d.1;
            (o.0 + d.0 * k, o.1 + d.1 * k)
        }
        Track::Path(pts) => {
            let mut best = (f32::INFINITY, p);
            for w in pts.windows(2) {
                let q = on_segment(w[0], w[1], p);
                let d = dist(q, p);
                if d < best.0 {
                    best = (d, q);
                }
            }
            best.1
        }
    }
}

fn on_segment(a: Point, b: Point, p: Point) -> Point {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    if l2 <= 1e-12 {
        return a;
    }
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0);
    (a.0 + dx * t, a.1 + dy * t)
}

/// An ellipse as a closed polyline, finer for bigger ellipses (32 … 2048 segments).
fn ellipse(c: Point, rx: f32, ry: f32, angle: f32) -> Vec<Point> {
    let n = ((rx.max(ry) * std::f32::consts::TAU / 3.0) as usize).clamp(32, 2048);
    let (s, co) = angle.sin_cos();
    (0..=n)
        .map(|k| {
            let t = std::f32::consts::TAU * k as f32 / n as f32;
            let (u, v) = (rx * t.cos(), ry * t.sin());
            (c.0 + u * co - v * s, c.1 + u * s + v * co)
        })
        .collect()
}

/// A Catmull-Rom curve through every point, 16 segments per span.
fn curve(pts: &[Point]) -> Vec<Point> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let n = pts.len() as isize;
    let at = |i: isize| pts.get(i.clamp(0, n - 1) as usize).copied().unwrap_or((0.0, 0.0));
    let mut out = Vec::with_capacity(pts.len() * 16);
    for i in 0..n - 1 {
        let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        for k in 0..16 {
            let t = k as f32 / 16.0;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f32, b: f32, c: f32, d: f32| 0.5 * (2.0 * b + (c - a) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (3.0 * b - a - 3.0 * c + d) * t3);
            out.push((f(p0.0, p1.0, p2.0, p3.0), f(p0.1, p1.1, p2.1, p3.1)));
        }
    }
    out.push(at(n - 1));
    out
}

#[cfg(test)]
#[path = "rulers_tests.rs"]
mod tests;
