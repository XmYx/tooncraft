//! Samples → dabs ([`StrokeEngine`]) and dabs → pixels ([`Stroke`]).

use std::collections::HashMap;
use std::sync::Arc;

use tooncraft_raster::{Surface, TILE, TILE_PIXELS, Tile, TileSnapshot, over, premultiply, scale_alpha};

use crate::dab::Shape;
use crate::{Brush, BrushKind, Dab, Sample, Stabilizer};

/// Turns input samples into evenly spaced dabs with the brush's dynamics applied.
#[derive(Clone, Debug)]
pub struct StrokeEngine {
    brush: Brush,
    stabilizer: Stabilizer,
    last: Option<Sample>,
    /// Distance travelled since the last dab.
    carry: f32,
    /// Filtered pointer speed in pixels per second.
    speed: f32,
}

impl StrokeEngine {
    pub fn new(brush: Brush) -> Self {
        let brush = brush.sanitized();
        Self { stabilizer: Stabilizer::new(brush.smoothing), brush, last: None, carry: 0.0, speed: 0.0 }
    }

    pub fn brush(&self) -> &Brush {
        &self.brush
    }

    /// Feed one raw input sample; returns the dabs it produces.
    pub fn feed(&mut self, raw: Sample) -> Vec<Dab> {
        let mut out = Vec::new();
        if let Some(s) = raw.sanitized().and_then(|s| self.stabilizer.feed(s)) {
            self.walk(s, &mut out);
        }
        out
    }

    /// End the stroke: the stabilizer's lag is drawn out (with catch-up).
    pub fn finish(&mut self) -> Vec<Dab> {
        let mut out = Vec::new();
        for s in self.stabilizer.finish() {
            self.walk(s, &mut out);
        }
        out
    }

    fn walk(&mut self, s: Sample, out: &mut Vec<Dab>) {
        let Some(prev) = self.last else {
            out.push(self.dab(&s));
            self.last = Some(s);
            self.carry = 0.0;
            return;
        };
        let (dx, dy) = (s.x - prev.x, s.y - prev.y);
        let len = (dx * dx + dy * dy).sqrt();
        let dt = (s.time - prev.time).max(1e-4) as f32;
        let inst = len / dt;
        self.speed = if self.speed == 0.0 { inst } else { self.speed * 0.7 + inst * 0.3 };
        if len <= f32::EPSILON {
            self.last = Some(s);
            return;
        }
        // Cap the number of dabs one segment may emit: hostile input cannot stall the app.
        let mut budget = 20_000usize;
        let mut travelled = 0.0f32;
        loop {
            let t = (travelled / len).clamp(0.0, 1.0);
            let here = lerp_sample(&prev, &s, t);
            let step = (self.brush.spacing * self.diameter(&here)).max(0.5);
            let need = step - self.carry;
            if travelled + need > len || budget == 0 {
                self.carry += len - travelled;
                break;
            }
            travelled += need;
            self.carry = 0.0;
            let at = lerp_sample(&prev, &s, (travelled / len).clamp(0.0, 1.0));
            out.push(self.dab(&at));
            budget -= 1;
        }
        self.last = Some(s);
    }

    fn diameter(&self, s: &Sample) -> f32 {
        let d = self.brush.dynamics;
        let p = 1.0 - d.pressure_size * (1.0 - s.pressure);
        let v = 1.0 - d.velocity_size * (self.speed / 2500.0).clamp(0.0, 1.0);
        (self.brush.size * p.max(d.min_size) * v.max(0.05)).max(0.5)
    }

    fn dab(&self, s: &Sample) -> Dab {
        let d = self.brush.dynamics;
        let m = (s.tilt_x * s.tilt_x + s.tilt_y * s.tilt_y).sqrt().min(1.0);
        Dab {
            x: s.x,
            y: s.y,
            radius: self.diameter(s) * 0.5,
            alpha: (self.brush.flow * (1.0 - d.pressure_opacity * (1.0 - s.pressure))).clamp(0.0, 1.0),
            angle: if m > 0.0 { s.tilt_y.atan2(s.tilt_x) } else { 0.0 },
            roundness: (1.0 - d.tilt * m * 0.75).clamp(0.1, 1.0),
            pressure: s.pressure,
        }
    }
}

fn lerp_sample(a: &Sample, b: &Sample, t: f32) -> Sample {
    let l = |x: f32, y: f32| x + (y - x) * t;
    Sample {
        x: l(a.x, b.x),
        y: l(a.y, b.y),
        pressure: l(a.pressure, b.pressure),
        tilt_x: l(a.tilt_x, b.tilt_x),
        tilt_y: l(a.tilt_y, b.tilt_y),
        time: a.time + (b.time - a.time) * t as f64,
    }
}

/// A stroke being painted onto one surface.
///
/// Holds 16-bit coverage per touched tile. Each touched pixel is recomputed from the tile as it
/// was before the stroke (the surface's capture) plus the coverage, so the result never depends
/// on how the dabs overlapped.
pub struct Stroke {
    brush: Brush,
    shape: Shape,
    color: [u8; 4],
    coverage: HashMap<u32, Vec<u16>>,
    dabs: u64,
}

impl Stroke {
    /// Start a stroke; the surface starts capturing tiles for undo.
    pub fn begin(surface: &mut Surface, brush: Brush) -> Self {
        let brush = brush.sanitized();
        surface.begin_capture();
        Self {
            shape: Shape { kind: brush.kind, hardness: brush.hardness, grain: brush.grain, grain_scale: brush.grain_scale },
            color: premultiply(brush.color),
            brush,
            coverage: HashMap::new(),
            dabs: 0,
        }
    }

    pub fn dab_count(&self) -> u64 {
        self.dabs
    }

    /// Rasterise dabs into the surface.
    pub fn add(&mut self, surface: &mut Surface, dabs: &[Dab]) {
        for dab in dabs {
            self.add_one(surface, dab);
        }
    }

    fn add_one(&mut self, surface: &mut Surface, dab: &Dab) {
        if !(dab.x.is_finite() && dab.y.is_finite() && dab.radius.is_finite()) {
            return;
        }
        let (x0, y0, x1, y1) = dab.bounds();
        let (w, h) = (surface.width() as i64, surface.height() as i64);
        let (x0, y0, x1, y1) = (x0.max(0), y0.max(0), x1.min(w - 1), y1.min(h - 1));
        if x0 > x1 || y0 > y1 {
            return;
        }
        self.dabs += 1;
        let (sin, cos) = dab.angle.sin_cos();
        let t = TILE as i64;
        let opacity = (self.brush.opacity * 255.0).round().clamp(0.0, 255.0) as u32;
        let eraser = self.brush.kind == BrushKind::Eraser;
        let build = self.brush.kind.builds_up();
        for ty in y0 / t..=y1 / t {
            for tx in x0 / t..=x1 / t {
                let Some(index) = surface.tile_index(tx as u32, ty as u32) else { continue };
                let original: Option<Arc<Tile>> = surface.original(index);
                if eraser && original.is_none() {
                    continue; // nothing to erase in a missing tile
                }
                let cov = self.coverage.entry(index).or_insert_with(|| vec![0u16; TILE_PIXELS]);
                let Some(tile) = surface.tile_mut(index) else { continue };
                let px = tile.bytes_mut();
                let (bx, by) = (tx * t, ty * t);
                for y in y0.max(by)..=y1.min(by + t - 1) {
                    for x in x0.max(bx)..=x1.min(bx + t - 1) {
                        let a = dab.coverage(x, y, &self.shape, cos, sin);
                        if a <= 0.0 {
                            continue;
                        }
                        let li = ((y - by) * t + (x - bx)) as usize;
                        let old = cov[li] as f32 / 65535.0;
                        let new = if build { old + a * (1.0 - old) } else { old.max(a) };
                        if new <= old {
                            continue;
                        }
                        cov[li] = (new * 65535.0).round() as u16;
                        let k = ((new * opacity as f32).round() as u32).min(255) as u8;
                        let i = li * 4;
                        let base = match &original {
                            Some(o) => {
                                let b = o.bytes();
                                [b[i], b[i + 1], b[i + 2], b[i + 3]]
                            }
                            None => [0; 4],
                        };
                        let out = if eraser { scale_alpha(base, 255 - k) } else { over(base, scale_alpha(self.color, k)) };
                        px[i..i + 4].copy_from_slice(&out);
                    }
                }
            }
        }
    }

    /// Finish the stroke; returns the tiles as they were before it (the undo record).
    pub fn end(self, surface: &mut Surface) -> TileSnapshot {
        if self.brush.kind == BrushKind::Eraser {
            surface.prune();
        }
        surface.take_capture()
    }
}

/// Paint a whole stroke in one call (commands, tests, replay). Returns the undo record.
pub fn paint(surface: &mut Surface, brush: Brush, samples: &[Sample]) -> TileSnapshot {
    let mut engine = StrokeEngine::new(brush);
    let mut stroke = Stroke::begin(surface, brush);
    for s in samples {
        let d = engine.feed(*s);
        stroke.add(surface, &d);
    }
    let d = engine.finish();
    stroke.add(surface, &d);
    stroke.end(surface)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(n: usize, pressure: f32) -> Vec<Sample> {
        (0..n).map(|i| Sample::at(20.0 + i as f32 * 3.0, 50.0 + (i as f32 * 0.2).sin() * 10.0, pressure, i as f64 * 0.008)).collect()
    }

    fn max_alpha(s: &Surface) -> u8 {
        s.tiles().flat_map(|(_, t)| t.bytes().as_chunks::<4>().0.iter().map(|p| p[3]).collect::<Vec<_>>()).max().unwrap_or(0)
    }

    #[test]
    fn every_brush_paints_and_is_deterministic() {
        for kind in [BrushKind::Pencil, BrushKind::Ink, BrushKind::Airbrush, BrushKind::Chalk] {
            let mut a = Surface::new(400, 200).unwrap();
            let mut b = Surface::new(400, 200).unwrap();
            let brush = Brush::preset(kind);
            paint(&mut a, brush, &line(80, 0.8));
            paint(&mut b, brush, &line(80, 0.8));
            assert!(!a.is_empty(), "{kind:?} painted nothing");
            assert_eq!(a, b, "{kind:?} is not deterministic");
        }
    }

    #[test]
    fn overlapping_dabs_do_not_exceed_opacity() {
        let mut s = Surface::new(200, 200).unwrap();
        let brush = Brush { opacity: 0.5, ..Brush::preset(BrushKind::Ink) };
        // a stroke that crosses itself many times
        let pts: Vec<Sample> =
            (0..400).map(|i| Sample::at(100.0 + (i as f32 * 0.3).cos() * 40.0, 100.0 + (i as f32 * 0.7).sin() * 40.0, 1.0, i as f64 * 0.01)).collect();
        paint(&mut s, brush, &pts);
        assert!(max_alpha(&s) <= 128, "alpha {}", max_alpha(&s));
        assert!(max_alpha(&s) >= 120);
    }

    #[test]
    fn airbrush_builds_up() {
        let mut once = Surface::new(200, 100).unwrap();
        let brush = Brush { opacity: 1.0, ..Brush::preset(BrushKind::Airbrush) };
        let pts: Vec<Sample> = (0..2).map(|i| Sample::at(100.0, 50.0, 1.0, i as f64)).collect();
        paint(&mut once, brush, &pts);
        let back_and_forth: Vec<Sample> = (0..60).map(|i| Sample::at(90.0 + (i % 2) as f32 * 20.0, 50.0, 1.0, i as f64 * 0.01)).collect();
        let mut many = Surface::new(200, 100).unwrap();
        paint(&mut many, brush, &back_and_forth);
        assert!(many.pixel(100, 50)[3] > once.pixel(100, 50)[3]);
    }

    #[test]
    fn pressure_changes_width() {
        let width = |p: f32| {
            let mut s = Surface::new(400, 120).unwrap();
            let pts: Vec<Sample> = (0..60).map(|i| Sample::at(50.0 + i as f32 * 4.0, 60.0, p, i as f64 * 0.01)).collect();
            paint(&mut s, Brush { size: 20.0, ..Brush::preset(BrushKind::Ink) }, &pts);
            (0..120).filter(|y| s.pixel(150, *y)[3] > 128).count()
        };
        assert!(width(1.0) > width(0.2) + 5, "{} vs {}", width(1.0), width(0.2));
    }

    #[test]
    fn eraser_removes_and_undo_record_restores() {
        let mut s = Surface::new(300, 100).unwrap();
        paint(&mut s, Brush { size: 20.0, ..Brush::preset(BrushKind::Ink) }, &line(60, 1.0));
        let before = s.clone();
        let undo = paint(&mut s, Brush { size: 60.0, hardness: 1.0, ..Brush::preset(BrushKind::Eraser) }, &line(60, 1.0));
        assert!(s.tiles().map(|(_, t)| t.bytes().as_chunks::<4>().0.iter().filter(|p| p[3] > 0).count()).sum::<usize>() < 50);
        s.swap_snapshot(undo);
        assert_eq!(s, before);
    }

    #[test]
    fn tilt_flattens_dabs() {
        let mut e = StrokeEngine::new(Brush { dynamics: crate::Dynamics { tilt: 1.0, ..Default::default() }, ..Brush::preset(BrushKind::Chalk) });
        let d = e.feed(Sample { x: 5.0, y: 5.0, pressure: 1.0, tilt_x: 0.0, tilt_y: 1.0, time: 0.0 });
        assert!(d[0].roundness < 0.5);
        assert!((d[0].angle - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
    }

    #[test]
    fn velocity_thins_ink() {
        let r = |dt: f64| {
            let mut e = StrokeEngine::new(Brush { smoothing: crate::Smoothing { average: 0.0, ..Default::default() }, ..Brush::preset(BrushKind::Ink) });
            let mut last = 0.0;
            for i in 0..20 {
                for d in e.feed(Sample::at(i as f32 * 20.0, 0.0, 1.0, i as f64 * dt)) {
                    last = d.radius;
                }
            }
            last
        };
        assert!(r(0.001) < r(0.5));
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(48))]
        /// Any sample stream (huge, negative, NaN coordinates, odd pressure) paints without panic
        /// and the undo record restores the surface exactly.
        #[test]
        fn hostile_samples_never_panic(
            pts in proptest::collection::vec((-1e4f32..1e4, -1e4f32..1e4, -2f32..2.0, -2f32..2.0, 0f64..10.0), 0..40),
            kind in 0usize..5, size in -10f32..2000.0,
        ) {
            let mut s = Surface::new(256, 256).unwrap();
            let before = s.clone();
            let brush = Brush { size, ..Brush::preset(BrushKind::ALL[kind]) };
            let samples: Vec<Sample> = pts.iter().map(|p| Sample { x: p.0, y: p.1, pressure: p.2, tilt_x: p.3, tilt_y: 0.0, time: p.4 }).collect();
            let undo = paint(&mut s, brush, &samples);
            s.swap_snapshot(undo);
            s.prune();
            proptest::prop_assert_eq!(s, before);
        }
    }
}
