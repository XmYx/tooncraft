//! Stroke smoothing: a pulled string followed by a moving average.

use crate::{Sample, Smoothing};
use std::collections::VecDeque;

/// Turns raw pointer samples into steadier ones.
#[derive(Clone, Debug)]
pub struct Stabilizer {
    s: Smoothing,
    anchor: Option<Sample>,
    window: VecDeque<Sample>,
    last_raw: Option<Sample>,
}

impl Stabilizer {
    pub fn new(smoothing: Smoothing) -> Self {
        Self { s: smoothing, anchor: None, window: VecDeque::new(), last_raw: None }
    }

    fn window_len(&self) -> usize {
        1 + (self.s.average.clamp(0.0, 1.0) * 15.0).round() as usize
    }

    /// Feed one raw sample; returns the smoothed samples to draw now (zero or one).
    pub fn feed(&mut self, raw: Sample) -> Option<Sample> {
        self.last_raw = Some(raw);
        // pulled string
        let p = match self.anchor {
            None => raw,
            Some(a) if self.s.string > 0.0 => {
                let (dx, dy) = (raw.x - a.x, raw.y - a.y);
                let d = (dx * dx + dy * dy).sqrt();
                if d <= self.s.string {
                    // The brush stays put, but pressure, tilt and barrel rotation follow the pen.
                    self.anchor = Some(Sample { pressure: raw.pressure, tilt_x: raw.tilt_x, tilt_y: raw.tilt_y, rotation: raw.rotation, time: raw.time, ..a });
                    return None;
                }
                let k = (d - self.s.string) / d;
                Sample { x: a.x + dx * k, y: a.y + dy * k, ..raw }
            }
            Some(_) => raw,
        };
        self.anchor = Some(p);
        // moving average
        self.window.push_back(p);
        while self.window.len() > self.window_len() {
            self.window.pop_front();
        }
        Some(self.average())
    }

    fn average(&self) -> Sample {
        let n = self.window.len().max(1) as f32;
        let mut out = self.window.back().copied().unwrap_or_default();
        out.x = self.window.iter().map(|s| s.x).sum::<f32>() / n;
        out.y = self.window.iter().map(|s| s.y).sum::<f32>() / n;
        out
    }

    /// End of stroke: with catch-up, the remaining lag is drawn to the last raw point.
    pub fn finish(&mut self) -> Vec<Sample> {
        let mut out = Vec::new();
        if self.s.catch_up
            && let Some(raw) = self.last_raw
        {
            // Drain the averaging window towards the final position.
            let steps = self.window.len().saturating_sub(1);
            for _ in 0..steps {
                self.window.push_back(raw);
                self.window.pop_front();
                out.push(self.average());
            }
            if self.s.string > 0.0 || out.last().is_none_or(|l| l.x != raw.x || l.y != raw.y) {
                out.push(raw);
            }
        }
        self.window.clear();
        self.anchor = None;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn average_reduces_jitter() {
        let mut st = Stabilizer::new(Smoothing { average: 1.0, string: 0.0, catch_up: true });
        let mut ys = Vec::new();
        for i in 0..100 {
            let y = if i % 2 == 0 { 10.0 } else { -10.0 };
            if let Some(s) = st.feed(Sample::at(i as f32, y, 0.5, i as f64)) {
                ys.push(s.y);
            }
        }
        let jitter = ys[20..].iter().map(|y| y.abs()).fold(0.0f32, f32::max);
        assert!(jitter < 2.0, "jitter {jitter}");
        let end = st.finish();
        assert_eq!(end.last().map(|s| (s.x, s.y)), Some((99.0, -10.0)));
    }

    #[test]
    fn string_holds_until_pulled() {
        let mut st = Stabilizer::new(Smoothing { average: 0.0, string: 10.0, catch_up: false });
        assert!(st.feed(Sample::at(0.0, 0.0, 0.5, 0.0)).is_some());
        assert!(st.feed(Sample::at(5.0, 0.0, 0.5, 0.1)).is_none());
        let s = st.feed(Sample::at(25.0, 0.0, 0.5, 0.2)).unwrap();
        assert!((s.x - 15.0).abs() < 1e-4);
        assert!(st.finish().is_empty());
    }
}
