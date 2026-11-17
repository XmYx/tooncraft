//! Response curves: a smooth, monotone-preserving curve through up to six control points that
//! maps a normalised input (pressure, speed, …) to a normalised output.
//!
//! The curve is a piecewise cubic Hermite spline whose tangents follow the weighted harmonic
//! mean of the neighbouring slopes (Fritsch and Butland, 1984), so it never overshoots its
//! control points: the output stays between the points' heights and a rising set of points gives
//! a rising curve. The end points sit at `x = 0` and `x = 1` and can only move vertically.

use serde::{Deserialize, Serialize};

/// Most control points a curve can have.
pub const MAX_POINTS: usize = 6;
/// Smallest horizontal gap between two control points.
pub const MIN_GAP: f32 = 0.02;

/// A response curve, `[0, 1] → [0, 1]`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "Vec<[f32; 2]>", into = "Vec<[f32; 2]>")]
pub struct Curve {
    len: u8,
    pts: [[f32; 2]; MAX_POINTS],
}

impl Default for Curve {
    fn default() -> Self {
        Curve::linear()
    }
}

impl From<Curve> for Vec<[f32; 2]> {
    fn from(c: Curve) -> Self {
        c.points().to_vec()
    }
}

impl From<Vec<[f32; 2]>> for Curve {
    fn from(v: Vec<[f32; 2]>) -> Self {
        Curve::from_points(&v)
    }
}

impl Curve {
    /// The identity: output equals input.
    pub fn linear() -> Self {
        Curve::raw(&[[0.0, 0.0], [1.0, 1.0]])
    }

    /// The reverse identity: full output at zero input.
    pub fn falling() -> Self {
        Curve::raw(&[[0.0, 1.0], [1.0, 0.0]])
    }

    /// Slow start, quick finish.
    pub fn ease_in() -> Self {
        Curve::raw(&[[0.0, 0.0], [0.6, 0.3], [1.0, 1.0]])
    }

    /// Quick start, slow finish.
    pub fn ease_out() -> Self {
        Curve::raw(&[[0.0, 0.0], [0.4, 0.7], [1.0, 1.0]])
    }

    /// Slow at both ends.
    pub fn s_curve() -> Self {
        Curve::raw(&[[0.0, 0.0], [0.3, 0.15], [0.7, 0.85], [1.0, 1.0]])
    }

    /// A flat curve at height `y`.
    pub fn constant(y: f32) -> Self {
        let y = if y.is_finite() { y.clamp(0.0, 1.0) } else { 0.0 };
        Curve::raw(&[[0.0, y], [1.0, y]])
    }

    /// Build from points already known to be valid.
    fn raw(p: &[[f32; 2]]) -> Self {
        let mut pts = [[0.0; 2]; MAX_POINTS];
        let n = p.len().min(MAX_POINTS);
        pts[..n].copy_from_slice(&p[..n]);
        Curve { len: n as u8, pts }
    }

    /// Build from any list of points: non-finite points are dropped, coordinates are clamped to
    /// `0 … 1`, the points are sorted, crowded points are thinned, the first and last points are
    /// moved to `x = 0` and `x = 1`, and at most six are kept. Fewer than two usable points give
    /// the identity.
    pub fn from_points(points: &[[f32; 2]]) -> Self {
        let mut v: Vec<[f32; 2]> =
            points.iter().take(64).filter(|p| p[0].is_finite() && p[1].is_finite()).map(|p| [p[0].clamp(0.0, 1.0), p[1].clamp(0.0, 1.0)]).collect();
        v.sort_by(|a, b| a[0].total_cmp(&b[0]));
        // keep the points that are far enough from the previous kept one
        let mut kept: Vec<[f32; 2]> = Vec::new();
        for p in v {
            match kept.last() {
                Some(l) if p[0] - l[0] < MIN_GAP => {}
                _ => kept.push(p),
            }
        }
        if kept.len() < 2 {
            return Curve::linear();
        }
        while kept.len() > MAX_POINTS {
            // drop the interior point that bends the curve least (the one closest to the chord
            // of its neighbours)
            let mut best = 1;
            let mut least = f32::MAX;
            for i in 1..kept.len() - 1 {
                let (a, b, c) = (kept[i - 1], kept[i], kept[i + 1]);
                let t = (b[0] - a[0]) / (c[0] - a[0]).max(1e-6);
                let d = (b[1] - (a[1] + (c[1] - a[1]) * t)).abs();
                if d < least {
                    least = d;
                    best = i;
                }
            }
            kept.remove(best);
        }
        let n = kept.len();
        kept[0][0] = 0.0;
        kept[n - 1][0] = 1.0;
        Curve::raw(&kept)
    }

    pub fn points(&self) -> &[[f32; 2]] {
        &self.pts[..self.len as usize]
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether this is exactly the identity.
    pub fn is_linear(&self) -> bool {
        *self == Curve::linear()
    }

    fn delta(&self, i: usize) -> f32 {
        let (a, b) = (self.pts[i], self.pts[i + 1]);
        (b[1] - a[1]) / (b[0] - a[0]).max(1e-6)
    }

    fn slope(&self, i: usize) -> f32 {
        let n = self.len as usize;
        if n < 2 {
            return 0.0;
        }
        if i == 0 {
            return self.delta(0);
        }
        if i == n - 1 {
            return self.delta(n - 2);
        }
        let (d0, d1) = (self.delta(i - 1), self.delta(i));
        if d0 * d1 <= 0.0 {
            return 0.0;
        }
        let (h0, h1) = (self.pts[i][0] - self.pts[i - 1][0], self.pts[i + 1][0] - self.pts[i][0]);
        let (w1, w2) = (2.0 * h1 + h0, h1 + 2.0 * h0);
        (w1 + w2) / (w1 / d0 + w2 / d1)
    }

    /// The curve's output for `x` (clamped to `0 … 1`; NaN counts as 0), always in `0 … 1`.
    pub fn eval(&self, x: f32) -> f32 {
        let n = self.len as usize;
        if n < 2 {
            return if n == 1 { self.pts[0][1] } else { x.clamp(0.0, 1.0) };
        }
        let x = if x.is_finite() { x.clamp(0.0, 1.0) } else { 0.0 };
        if x <= self.pts[0][0] {
            return self.pts[0][1];
        }
        if x >= self.pts[n - 1][0] {
            return self.pts[n - 1][1];
        }
        let mut i = 0;
        while i + 2 < n && x > self.pts[i + 1][0] {
            i += 1;
        }
        let (a, b) = (self.pts[i], self.pts[i + 1]);
        let h = (b[0] - a[0]).max(1e-6);
        let t = ((x - a[0]) / h).clamp(0.0, 1.0);
        let (m0, m1) = (self.slope(i) * h, self.slope(i + 1) * h);
        let (t2, t3) = (t * t, t * t * t);
        let y = (2.0 * t3 - 3.0 * t2 + 1.0) * a[1] + (t3 - 2.0 * t2 + t) * m0 + (-2.0 * t3 + 3.0 * t2) * b[1] + (t3 - t2) * m1;
        y.clamp(0.0, 1.0)
    }

    /// Move control point `i`. The first and last points keep their `x`; an interior point stays
    /// between its neighbours. Returns false when `i` does not exist.
    pub fn set_point(&mut self, i: usize, x: f32, y: f32) -> bool {
        let n = self.len as usize;
        if i >= n {
            return false;
        }
        let y = if y.is_finite() { y.clamp(0.0, 1.0) } else { self.pts[i][1] };
        if i != 0 && i != n - 1 {
            let lo = self.pts[i - 1][0] + MIN_GAP;
            let hi = self.pts[i + 1][0] - MIN_GAP;
            let x = if x.is_finite() { x } else { self.pts[i][0] };
            self.pts[i][0] = x.clamp(lo.min(hi), hi.max(lo.min(hi)));
        }
        self.pts[i][1] = y;
        true
    }

    /// Add a control point on the curve at `x`; returns its index, or `None` when the curve is
    /// full or `x` is too close to an existing point.
    pub fn insert(&mut self, x: f32) -> Option<usize> {
        let n = self.len as usize;
        if n >= MAX_POINTS || !x.is_finite() {
            return None;
        }
        let x = x.clamp(0.0, 1.0);
        if self.points().iter().any(|p| (p[0] - x).abs() < MIN_GAP) {
            return None;
        }
        let y = self.eval(x);
        let at = self.points().iter().position(|p| p[0] > x)?;
        let mut v = self.points().to_vec();
        v.insert(at, [x, y]);
        *self = Curve::raw(&v);
        Some(at)
    }

    /// Remove interior point `i`; the end points and a two-point curve cannot lose points.
    pub fn remove(&mut self, i: usize) -> bool {
        let n = self.len as usize;
        if n <= 2 || i == 0 || i >= n - 1 {
            return false;
        }
        let mut v = self.points().to_vec();
        v.remove(i);
        *self = Curve::raw(&v);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_is_identity_and_falling_reverses() {
        for i in 0..=20 {
            let x = i as f32 / 20.0;
            assert!((Curve::linear().eval(x) - x).abs() < 1e-5);
            assert!((Curve::falling().eval(x) - (1.0 - x)).abs() < 1e-5);
        }
    }

    #[test]
    fn passes_through_control_points() {
        let c = Curve::from_points(&[[0.0, 0.1], [0.3, 0.8], [0.7, 0.4], [1.0, 0.9]]);
        for p in c.points() {
            assert!((c.eval(p[0]) - p[1]).abs() < 1e-5, "{p:?}");
        }
    }

    #[test]
    fn rising_points_give_a_rising_curve_without_overshoot() {
        let c = Curve::from_points(&[[0.0, 0.0], [0.2, 0.05], [0.5, 0.9], [0.8, 0.95], [1.0, 1.0]]);
        let mut last = 0.0;
        for i in 0..=200 {
            let y = c.eval(i as f32 / 200.0);
            assert!(y >= last - 1e-5, "not monotone at {i}: {y} < {last}");
            assert!((0.0..=1.0).contains(&y));
            last = y;
        }
    }

    #[test]
    fn hostile_points_are_repaired() {
        assert!(Curve::from_points(&[]).is_linear());
        assert!(Curve::from_points(&[[f32::NAN, 0.0], [0.5, f32::INFINITY]]).is_linear());
        let c = Curve::from_points(&[[0.4, 9.0], [-3.0, -2.0], [0.4001, 0.5], [7.0, 0.2]]);
        assert_eq!(c.points().first().map(|p| p[0]), Some(0.0));
        assert_eq!(c.points().last().map(|p| p[0]), Some(1.0));
        assert!(c.points().iter().all(|p| (0.0..=1.0).contains(&p[1])));
        let many: Vec<[f32; 2]> = (0..40).map(|i| [i as f32 / 40.0, (i % 3) as f32 / 2.0]).collect();
        assert!(Curve::from_points(&many).len() <= MAX_POINTS);
        assert_eq!(Curve::linear().eval(f32::NAN), 0.0);
    }

    #[test]
    fn editing_keeps_the_curve_valid() {
        let mut c = Curve::linear();
        assert_eq!(c.insert(0.5), Some(1));
        assert!(c.set_point(1, 5.0, 0.9)); // pushed back between its neighbours
        assert!(c.points()[1][0] < 1.0 - MIN_GAP + 1e-6);
        assert!(c.set_point(0, 0.5, 0.3)); // the end point only moves vertically
        assert_eq!(c.points()[0], [0.0, 0.3]);
        assert_eq!(c.insert(c.points()[1][0] + 0.001), None);
        assert!(c.remove(1));
        assert!(!c.remove(0) && !c.remove(1));
        for _ in 0..10 {
            let x = c.points().iter().map(|p| p[0]).fold(0.0, f32::max) * 0.5 + 0.01;
            c.insert(x);
        }
        assert!(c.len() <= MAX_POINTS);
    }

    #[test]
    fn serde_round_trip() {
        let c = Curve::s_curve();
        let json = serde_json::to_string(&c).unwrap_or_default();
        let back: Curve = serde_json::from_str(&json).unwrap_or_default();
        assert_eq!(back, c);
        let junk: Curve = serde_json::from_str("[[2,2],[3,3],[null,1]]").unwrap_or_default();
        assert!(junk.points().iter().all(|p| (0.0..=1.0).contains(&p[0]) && (0.0..=1.0).contains(&p[1])));
    }
}
