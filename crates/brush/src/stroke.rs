//! Samples → dabs ([`StrokeEngine`]) and dabs → pixels ([`Stroke`]).

use std::collections::HashMap;
use std::f32::consts::TAU;
use std::sync::Arc;

use tooncraft_raster::{Surface, TILE, TILE_PIXELS, Tile, TileSnapshot, over, premultiply, scale_alpha};

use crate::dab::Shape;
use crate::{Brush, BrushKind, Compose, Dab, Grain, Inputs, Sample, Source, Stabilizer, Target, hash01};

/// Turns input samples into evenly spaced dabs with the brush's dynamics applied.
///
/// Everything random (the Random source, scatter directions) comes from a hash of the stroke's
/// first position and the dab's number, so replaying the same samples gives the same dabs.
#[derive(Clone, Debug)]
pub struct StrokeEngine {
    brush: Brush,
    stabilizer: Stabilizer,
    last: Option<Sample>,
    /// Distance travelled since the last dab.
    carry: f32,
    /// Filtered pointer speed in pixels per second.
    speed: f32,
    /// Direction of the last movement in turns (0 rightwards, 0.25 downwards).
    heading: f32,
    /// Number of dabs made so far, and the stroke's random seed.
    index: u64,
    seed: u64,
}

impl StrokeEngine {
    pub fn new(brush: Brush) -> Self {
        let brush = brush.sanitized();
        Self { stabilizer: Stabilizer::new(brush.smoothing), brush, last: None, carry: 0.0, speed: 0.0, heading: 0.0, index: 0, seed: 0 }
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
            self.seed = ((s.x.to_bits() as u64) << 32) | s.y.to_bits() as u64;
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
        self.heading = (dy.atan2(dx) / TAU).rem_euclid(1.0);
        // Cap the number of dabs one segment may emit: hostile input cannot stall the app.
        let mut budget = 20_000usize;
        let mut travelled = 0.0f32;
        loop {
            let t = (travelled / len).clamp(0.0, 1.0);
            let here = lerp_sample(&prev, &s, t);
            let step = (self.brush.spacing * self.diameter(&here, 1.0)).max(0.5);
            let need = step - self.carry;
            if travelled + need > len || budget == 0 {
                self.carry += len - travelled;
                break;
            }
            travelled += need;
            self.carry = 0.0;
            let at = lerp_sample(&prev, &s, (travelled / len).clamp(0.0, 1.0));
            // a spray lays several dabs per step (each with its own scatter and chances)
            for _ in 0..self.brush.count.max(1) {
                out.push(self.dab(&at));
            }
            budget = budget.saturating_sub(self.brush.count.max(1) as usize);
        }
        self.last = Some(s);
    }

    /// The inputs of a dab: pressure, tilt, speed, heading and (filled in per target) chance.
    fn inputs(&self, s: &Sample, random: f32) -> Inputs {
        let tilt = (s.tilt_x * s.tilt_x + s.tilt_y * s.tilt_y).sqrt().min(1.0);
        let mut i = [0.0; 6];
        i[Source::Pressure.index()] = s.pressure;
        i[Source::Tilt.index()] = tilt;
        i[Source::Speed.index()] = (self.speed / 2500.0).clamp(0.0, 1.0);
        i[Source::Rotation.index()] = self.heading;
        i[Source::Random.index()] = random;
        i[Source::Barrel.index()] = s.rotation;
        i
    }

    /// The chance input for one target of the current dab.
    fn random(&self, t: Target) -> f32 {
        hash01(self.seed ^ self.index.wrapping_mul(0xA24B_AED4_963E_E407), t.index() as u64 + 1)
    }

    /// Dab diameter for the inputs at `s`; `random` is the chance input (1 for spacing, which
    /// follows the nominal size).
    fn diameter(&self, s: &Sample, random: f32) -> f32 {
        let d = &self.brush.dynamics;
        let f = d.response.factor(Target::Size, &self.inputs(s, random)).max(d.min_size);
        (self.brush.size * f).clamp(0.5, crate::MAX_SIZE)
    }

    /// The dab's colour under colour jitter (`None` without jitter).
    fn jittered(&self) -> Option<[u8; 3]> {
        let j = self.brush.jitter;
        if j.is_off() {
            return None;
        }
        let key = if j.per_stroke { self.seed } else { self.seed ^ self.index.wrapping_mul(0x9E37_79B9_7F4A_7C15) };
        let r = [hash01(key, 101), hash01(key, 102), hash01(key, 103)];
        let c = self.brush.color;
        Some(j.apply([c[0], c[1], c[2]], r))
    }

    fn dab(&mut self, s: &Sample) -> Dab {
        let d = self.brush.dynamics;
        let r = d.response;
        let with = |t: Target| self.inputs(s, self.random(t));
        let diameter = self.diameter(s, self.random(Target::Size));
        let alpha = (self.brush.flow * r.factor(Target::Flow, &with(Target::Flow)) * r.factor(Target::Opacity, &with(Target::Opacity))).clamp(0.0, 1.0);
        let m = (s.tilt_x * s.tilt_x + s.tilt_y * s.tilt_y).sqrt().min(1.0);
        let tilt_angle = if m > 0.0 { s.tilt_y.atan2(s.tilt_x) } else { 0.0 };
        let turn = r.offset(Target::Angle, &with(Target::Angle));
        let scatter = r.offset(Target::Scatter, &with(Target::Scatter)).max(0.0) * diameter;
        let (mut x, mut y) = (s.x, s.y);
        if scatter > 0.0 {
            let a = hash01(self.seed ^ self.index.wrapping_mul(0xD6E8_FEB8_6659_FD93), 77) * TAU;
            x += a.cos() * scatter;
            y += a.sin() * scatter;
        }
        let dab = Dab {
            x,
            y,
            radius: diameter * 0.5,
            alpha,
            angle: self.brush.angle.to_radians() + tilt_angle + turn * TAU,
            roundness: (self.brush.roundness * (1.0 - d.tilt * m * 0.75)).clamp(0.1, 1.0),
            pressure: s.pressure,
            color: self.jittered(),
        };
        self.index = self.index.wrapping_add(1);
        dab
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
        // the barrel turns the short way round: 0.95 to 0.05 passes through 0
        rotation: {
            let d = (b.rotation - a.rotation + 0.5).rem_euclid(1.0) - 0.5;
            (a.rotation + d * t).rem_euclid(1.0)
        },
    }
}

/// The images a brush refers to, resolved by the caller from the shot's tip table.
#[derive(Clone, Debug, Default)]
pub struct Images {
    /// The image tip stamped instead of the round dab.
    pub tip: Option<Arc<crate::Tip>>,
    /// The grain image (used when the brush's grain kind is `Image`).
    pub grain: Option<Arc<crate::Tip>>,
}

/// Paint carried by a wet brush: premultiplied RGBA, each 0 … 1.
type Load = [f32; 4];

/// A stroke being painted onto one surface.
///
/// Holds 16-bit coverage per touched tile. Each touched pixel is recomputed from the tile as it
/// was before the stroke (the surface's capture) plus the coverage, so the result never depends
/// on how the dabs overlapped. Wet brushes (see [`Stroke::is_wet`]) instead stamp onto the tile
/// as it is now, because they read the colour under them.
pub struct Stroke {
    brush: Brush,
    shape: Shape,
    color: [u8; 4],
    coverage: HashMap<u32, Vec<u16>>,
    dabs: u64,
    wet: bool,
    load: Option<Load>,
}

impl Stroke {
    /// Start a stroke; the surface starts capturing tiles for undo.
    pub fn begin(surface: &mut Surface, brush: Brush) -> Self {
        Self::begin_with(surface, brush, Images::default())
    }

    /// Start a stroke stamping an image tip (the brush's `tip` resolved by the caller).
    pub fn begin_with_tip(surface: &mut Surface, brush: Brush, tip: Option<Arc<crate::Tip>>) -> Self {
        Self::begin_with(surface, brush, Images { tip, grain: None })
    }

    /// Start a stroke with its tip and grain images.
    pub fn begin_with(surface: &mut Surface, brush: Brush, images: Images) -> Self {
        let brush = brush.sanitized();
        surface.begin_capture();

        let wet = brush.kind != BrushKind::Eraser && (brush.kind == BrushKind::Smudge || brush.wet.pickup > 0.0);
        Self {
            shape: Shape {
                kind: brush.kind,
                hardness: brush.hardness,
                grain: brush.grain,
                // a grain image that is missing falls back to paper
                texture: Grain::new(brush.grain_kind, brush.grain_scale, brush.grain_angle, brush.grain_contrast, images.grain),
                tip: images.tip,
            },
            color: premultiply(brush.color),
            brush,
            coverage: HashMap::new(),
            dabs: 0,
            wet,
            load: None,
        }
    }

    pub fn dab_count(&self) -> u64 {
        self.dabs
    }

    /// Whether this stroke mixes with the colours under it (the smudge brush, or any brush with
    /// colour pickup).
    pub fn is_wet(&self) -> bool {
        self.wet
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
        if self.wet {
            self.add_wet(surface, dab, (x0, y0, x1, y1));
            return;
        }
        let (sin, cos) = dab.angle.sin_cos();
        let t = TILE as i64;
        let opacity = (self.brush.opacity * 255.0).round().clamp(0.0, 255.0) as u32;
        let eraser = self.brush.kind == BrushKind::Eraser;
        let compose = self.brush.compose;
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
                        let colour = match dab.color {
                            Some(c) => premultiply([c[0], c[1], c[2], self.brush.color[3]]),
                            None => self.color,
                        };
                        let paint = scale_alpha(colour, k);
                        let out = match (eraser, compose) {
                            (true, Compose::Normal) => scale_alpha(base, 255 - k),
                            // erasing "behind", inside a locked layer or in a blend mode changes
                            // nothing sensible
                            (true, _) => base,
                            (false, Compose::Normal) => over(base, paint),
                            (false, Compose::Behind) => over(paint, base),
                            (false, Compose::KeepAlpha) => keep_alpha(base, paint),
                            (false, mode) => blend_over(base, paint, mode),
                        };
                        px[i..i + 4].copy_from_slice(&out);
                    }
                }
            }
        }
    }

    /// One dab of a wet brush. The brush carries a load of paint. A painting brush starts with
    /// its colour and mixes the colour of the layer under it into the load; the smudge brush
    /// starts with the layer's colour and lets go of it gradually, so the colour is dragged along.
    /// Each dab then lays the load onto the pixels under it. Flow, opacity and pickup are per
    /// brush width of travel, not per dab, so the look does not change with the spacing.
    fn add_wet(&mut self, surface: &mut Surface, dab: &Dab, (x0, y0, x1, y1): (i64, i64, i64, i64)) {
        let smudge = self.brush.kind == BrushKind::Smudge;
        if smudge && self.brush.wet.pickup >= 1.0 {
            self.add_blur(surface, dab, (x0, y0, x1, y1));
            return;
        }
        let step = self.brush.spacing.clamp(0.02, 1.0);
        let rate = |p: f32| 1.0 - (1.0 - p.clamp(0.0, 0.999)).powf(step);
        let avg = average_under(surface, dab);
        let first = self.load.is_none();
        let paint = {
            let c = self.color;
            [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, c[3] as f32 / 255.0]
        };
        let load = self.load.get_or_insert(if smudge { avg } else { paint });
        let k = rate(self.brush.wet.pickup);
        if smudge {
            if !first {
                for c in 0..4 {
                    load[c] += (avg[c] - load[c]) * k;
                }
            }
        } else if avg[3] > 0.0 && load[3] > 0.0 {
            // mix the colours, not the transparency: paint does not thin out on blank canvas
            let w = (k * avg[3]).clamp(0.0, 1.0);
            for c in 0..3 {
                let (own, theirs) = (load[c] / load[3], avg[c] / avg[3]);
                load[c] = (own + (theirs - own) * w).clamp(0.0, 1.0) * load[3];
            }
        }
        let load = *load;
        let (sin, cos) = dab.angle.sin_cos();
        let t = TILE as i64;
        let opacity = self.brush.opacity;
        let salt = self.dabs;
        for ty in y0 / t..=y1 / t {
            for tx in x0 / t..=x1 / t {
                let Some(index) = surface.tile_index(tx as u32, ty as u32) else { continue };
                let Some(tile) = surface.tile_mut(index) else { continue };
                let px = tile.bytes_mut();
                let (bx, by) = (tx * t, ty * t);
                for y in y0.max(by)..=y1.min(by + t - 1) {
                    for x in x0.max(bx)..=x1.min(bx + t - 1) {
                        let a = dab.coverage(x, y, &self.shape, cos, sin) * opacity;
                        if a <= 0.0 {
                            continue;
                        }
                        let d = 1.0 - (1.0 - a.min(0.999)).powf(step);
                        let i = (((y - by) * t + (x - bx)) * 4) as usize;
                        let locked_alpha = (self.brush.compose == Compose::KeepAlpha).then_some(px[i + 3]);
                        // an unbiased rounding keeps slow blends moving
                        let dither = hash01(salt, (x as u64) << 20 ^ y as u64);
                        for c in 0..4 {
                            let cur = px[i + c] as f32;
                            let v = if smudge { cur + (load[c] * 255.0 - cur) * d } else { cur * (1.0 - load[3] * d) + load[c] * 255.0 * d };
                            px[i + c] = (v + dither).floor().clamp(0.0, 255.0) as u8;
                        }
                        if let Some(a0) = locked_alpha {
                            // the layer's transparency is locked: keep the old alpha, rescale the colour
                            let a1 = px[i + 3].max(1) as f32;
                            for c in 0..3 {
                                px[i + c] = (px[i + c] as f32 * a0 as f32 / a1).round().min(a0 as f32) as u8;
                            }
                            px[i + 3] = a0;
                        }
                        // keep the pixel a valid premultiplied colour
                        let alpha = px[i + 3];
                        for c in 0..3 {
                            px[i + c] = px[i + c].min(alpha);
                        }
                    }
                }
            }
        }
    }

    /// One dab of the blur: the smudge brush at full pickup. Every pixel under the dab moves
    /// towards the mean of its own neighbourhood (a box 0.6 of the dab's radius wide), so
    /// edges soften in place and nothing is carried along the stroke. The strength is per brush
    /// width of travel, like the other wet brushes.
    fn add_blur(&mut self, surface: &mut Surface, dab: &Dab, (x0, y0, x1, y1): (i64, i64, i64, i64)) {
        let r = ((dab.radius * 0.3).round() as i64).clamp(1, 8);
        let (w, h) = ((x1 - x0 + 1) as usize, (y1 - y0 + 1) as usize);
        let (pw, ph) = (w + 2 * r as usize, h + 2 * r as usize);
        // the region with a margin, premultiplied, as floats
        let mut padded = vec![[0.0f32; 4]; pw * ph];
        for py in 0..ph {
            for px in 0..pw {
                let p = surface.pixel(x0 - r + px as i64, y0 - r + py as i64);
                padded[py * pw + px] = [p[0] as f32, p[1] as f32, p[2] as f32, p[3] as f32];
            }
        }
        // box blur: along the rows, then down the columns, with running sums
        let win = (2 * r + 1) as f32;
        let mut rows = vec![[0.0f32; 4]; w * ph];
        for py in 0..ph {
            let mut sum = [0.0f32; 4];
            for px in 0..(2 * r as usize) {
                for c in 0..4 {
                    sum[c] += padded[py * pw + px][c];
                }
            }
            for x in 0..w {
                let add = padded[py * pw + x + 2 * r as usize];
                for c in 0..4 {
                    sum[c] += add[c];
                    rows[py * w + x][c] = sum[c] / win;
                    sum[c] -= padded[py * pw + x][c];
                }
            }
        }
        let mut blurred = vec![[0.0f32; 4]; w * h];
        for x in 0..w {
            let mut sum = [0.0f32; 4];
            for py in 0..(2 * r as usize) {
                for c in 0..4 {
                    sum[c] += rows[py * w + x][c];
                }
            }
            for y in 0..h {
                let add = rows[(y + 2 * r as usize) * w + x];
                for c in 0..4 {
                    sum[c] += add[c];
                    blurred[y * w + x][c] = sum[c] / win;
                    sum[c] -= rows[y * w + x][c];
                }
            }
        }
        let (sin, cos) = dab.angle.sin_cos();
        let t = TILE as i64;
        // softening is much slower than smearing for the same travel: three times the strength
        let step = (self.brush.spacing.clamp(0.02, 1.0) * 3.0).min(1.0);
        let opacity = self.brush.opacity;
        let salt = self.dabs;
        for ty in y0 / t..=y1 / t {
            for tx in x0 / t..=x1 / t {
                let Some(index) = surface.tile_index(tx as u32, ty as u32) else { continue };
                let Some(tile) = surface.tile_mut(index) else { continue };
                let px = tile.bytes_mut();
                let (bx, by) = (tx * t, ty * t);
                for y in y0.max(by)..=y1.min(by + t - 1) {
                    for x in x0.max(bx)..=x1.min(bx + t - 1) {
                        let a = dab.coverage(x, y, &self.shape, cos, sin) * opacity;
                        if a <= 0.0 {
                            continue;
                        }
                        let d = 1.0 - (1.0 - a.min(0.999)).powf(step);
                        let i = (((y - by) * t + (x - bx)) * 4) as usize;
                        let target = blurred[(y - y0) as usize * w + (x - x0) as usize];
                        let dither = hash01(salt, (x as u64) << 20 ^ y as u64);
                        for c in 0..4 {
                            let cur = px[i + c] as f32;
                            let v = cur + (target[c] - cur) * d;
                            // a pixel that is already its neighbourhood's mean stays as it is
                            px[i + c] = if (v - cur).abs() < 0.02 { px[i + c] } else { (v + dither).floor().clamp(0.0, 255.0) as u8 };
                        }
                        let alpha = px[i + 3];
                        for c in 0..3 {
                            px[i + c] = px[i + c].min(alpha);
                        }
                    }
                }
            }
        }
    }

    /// Finish the stroke; returns the tiles as they were before it (the undo record).
    pub fn end(self, surface: &mut Surface) -> TileSnapshot {
        if self.brush.kind == BrushKind::Eraser || self.wet || self.brush.compose == Compose::KeepAlpha {
            surface.prune();
        }
        surface.take_capture()
    }
}

/// `paint` laid over `base` with the alpha of `base` unchanged (premultiplied in and out).
/// Paint `paint` over `base` (both premultiplied) in a blend mode: where there is paint under it,
/// the paint's colour becomes the blend of the two (weighted by the paint under's opacity), then
/// goes over as usual.
fn blend_over(base: [u8; 4], paint: [u8; 4], mode: Compose) -> [u8; 4] {
    if paint[3] == 0 {
        return base;
    }
    let (ab, as_) = (base[3] as f32 / 255.0, paint[3] as f32 / 255.0);
    let straight = |p: [u8; 4], a: f32, k: usize| if a > 0.0 { (p[k] as f32 / 255.0 / a).min(1.0) } else { 0.0 };
    let mut c = [0u8; 4];
    for k in 0..3 {
        let (cb, cs) = (straight(base, ab, k), straight(paint, as_, k));
        let mixed = mode.blend(cb, cs).unwrap_or(cs);
        let v = (1.0 - ab) * cs + ab * mixed;
        c[k] = (v * as_ * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    c[3] = paint[3];
    over(base, c)
}

fn keep_alpha(base: [u8; 4], paint: [u8; 4]) -> [u8; 4] {
    if base[3] == 0 || paint[3] == 0 {
        return base;
    }
    let mixed = over(base, paint);
    // the colour the pixel would have, at the pixel's own alpha
    let scale = base[3] as f32 / mixed[3].max(1) as f32;
    let c = |v: u8| ((v as f32 * scale).round().min(base[3] as f32)) as u8;
    [c(mixed[0]), c(mixed[1]), c(mixed[2]), base[3]]
}

/// The mean of the premultiplied colours under a dab (13 sample points inside its radius).
fn average_under(surface: &Surface, dab: &Dab) -> Load {
    let r = dab.radius.max(0.5);
    let mut sum = [0.0f32; 4];
    let mut n = 0.0;
    let mut add = |dx: f32, dy: f32| {
        let p = surface.pixel((dab.x + dx).floor() as i64, (dab.y + dy).floor() as i64);
        for c in 0..4 {
            sum[c] += p[c] as f32 / 255.0;
        }
        n += 1.0;
    };
    add(0.0, 0.0);
    for ring in [0.45f32, 0.85] {
        for k in 0..6 {
            let a = k as f32 * TAU / 6.0 + ring * 2.0;
            add(a.cos() * r * ring, a.sin() * r * ring);
        }
    }
    [sum[0] / n, sum[1] / n, sum[2] / n, sum[3] / n]
}

/// Paint a whole stroke in one call (commands, tests, replay). Returns the undo record.
pub fn paint(surface: &mut Surface, brush: Brush, samples: &[Sample]) -> TileSnapshot {
    paint_with(surface, brush, samples, Images::default())
}

/// [`paint`] with the brush's tip and grain images.
pub fn paint_with(surface: &mut Surface, brush: Brush, samples: &[Sample], images: Images) -> TileSnapshot {
    let mut engine = StrokeEngine::new(brush);
    let mut stroke = Stroke::begin_with(surface, brush, images);
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
    fn image_tip_stamps_its_shape() {
        // a tip that is only its left half: a horizontal stroke of it leaves a thinner trail than
        // a vertical one would not; simpler: a single dab covers only the left half
        let mut alpha = vec![0u8; 16 * 16];
        for y in 0..16 {
            for x in 0..8 {
                alpha[y * 16 + x] = 255;
            }
        }
        let tip = Arc::new(crate::Tip::new("half", 16, 16, alpha).unwrap());
        let mut s = Surface::new(128, 128).unwrap();
        let brush = Brush { size: 40.0, smoothing: crate::Smoothing { average: 0.0, ..Default::default() }, ..Brush::preset(BrushKind::Ink) };
        let mut st = Stroke::begin_with_tip(&mut s, brush, Some(tip));
        let dab = Dab { x: 64.0, y: 64.0, radius: 20.0, alpha: 1.0, angle: 0.0, roundness: 1.0, pressure: 1.0, color: None };
        st.add(&mut s, &[dab]);
        st.end(&mut s);
        assert!(s.pixel(54, 64)[3] > 200 && s.pixel(74, 64)[3] == 0);
    }

    #[test]
    fn tilt_flattens_dabs() {
        let mut e = StrokeEngine::new(Brush { dynamics: crate::Dynamics { tilt: 1.0, ..Default::default() }, ..Brush::preset(BrushKind::Chalk) });
        let d = e.feed(Sample { x: 5.0, y: 5.0, pressure: 1.0, tilt_x: 0.0, tilt_y: 1.0, time: 0.0, rotation: 0.0 });
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
            kind in 0usize..BrushKind::ALL.len(), size in -10f32..2000.0,
        ) {
            let mut s = Surface::new(256, 256).unwrap();
            let before = s.clone();
            let brush = Brush { size, ..Brush::preset(BrushKind::ALL[kind]) };
            let samples: Vec<Sample> = pts.iter().map(|p| Sample { x: p.0, y: p.1, pressure: p.2, tilt_x: p.3, tilt_y: 0.0, time: p.4, rotation: 0.0 }).collect();
            let undo = paint(&mut s, brush, &samples);
            s.swap_snapshot(undo);
            s.prune();
            proptest::prop_assert_eq!(s, before);
        }
    }
}

#[cfg(test)]
mod dynamics_tests {
    use super::*;
    use crate::{Curve, Dynamics, GrainKind, Response, Smoothing, Wet};

    fn plain(kind: BrushKind) -> Brush {
        Brush { smoothing: Smoothing { average: 0.0, catch_up: false, ..Default::default() }, ..Brush::preset(kind) }
    }

    fn dabs(brush: Brush, samples: &[Sample]) -> Vec<Dab> {
        let mut e = StrokeEngine::new(brush);
        let mut out: Vec<Dab> = samples.iter().flat_map(|s| e.feed(*s)).collect();
        out.extend(e.finish());
        out
    }

    fn row(n: usize, pressure: f32) -> Vec<Sample> {
        (0..n).map(|i| Sample::at(20.0 + i as f32 * 4.0, 50.0, pressure, i as f64 * 0.01)).collect()
    }

    fn with_response(f: impl FnOnce(&mut Response)) -> Brush {
        let mut dynamics = Dynamics { min_size: 0.01, ..Dynamics::linear(0.0, 0.0, 0.0, 0.01) };
        f(&mut dynamics.response);
        Brush { dynamics, size: 20.0, ..plain(BrushKind::Ink) }
    }

    #[test]
    fn the_curve_changes_how_pressure_maps_to_size() {
        let linear = with_response(|r| r.set(Target::Size, Source::Pressure, 1.0));
        let eased = with_response(|r| {
            r.set(Target::Size, Source::Pressure, 1.0);
            r.get_mut(Target::Size, Source::Pressure).curve = Curve::ease_in();
        });
        let radius = |b: Brush| dabs(b, &row(6, 0.5)).first().map(|d| d.radius).unwrap_or(0.0);
        assert!((radius(linear) - 5.0).abs() < 0.2, "{}", radius(linear));
        assert!(radius(eased) < radius(linear) * 0.8, "{} vs {}", radius(eased), radius(linear));
    }

    #[test]
    fn every_source_can_drive_every_scaled_target() {
        for target in [Target::Size, Target::Opacity, Target::Flow] {
            for source in [Source::Pressure, Source::Tilt, Source::Speed, Source::Rotation, Source::Random] {
                let brush = with_response(|r| r.set(target, source, 1.0));
                // two strokes with different inputs: pressure, tilt, speed, heading
                let a: Vec<Sample> = (0..30)
                    .map(|i| Sample { x: 10.0 + i as f32 * 2.0, y: 20.0, pressure: 1.0, tilt_x: 0.0, tilt_y: 0.0, time: i as f64 * 0.5, rotation: 0.0 })
                    .collect();
                let b: Vec<Sample> = (0..30)
                    .map(|i| Sample { x: 200.0, y: 10.0 + i as f32 * 12.0, pressure: 0.1, tilt_x: 0.9, tilt_y: 0.0, time: i as f64 * 0.0005, rotation: 0.0 })
                    .collect();
                // the mean size and strength of the dabs after the first (the speed is unknown there)
                let key = |ds: Vec<Dab>| {
                    let n = ds.len().saturating_sub(1).max(1) as f32;
                    (ds.iter().skip(1).map(|d| d.radius).sum::<f32>() / n, ds.iter().skip(1).map(|d| d.alpha).sum::<f32>() / n)
                };
                let (ka, kb) = (key(dabs(brush, &a)), key(dabs(brush, &b)));
                assert!(ka != kb || source == Source::Random, "{target:?} ignores {source:?}: {ka:?} {kb:?}");
            }
        }
    }

    #[test]
    fn random_jitter_varies_the_dabs_but_replays_exactly() {
        let brush = with_response(|r| r.set(Target::Size, Source::Random, 0.8));
        let (a, b) = (dabs(brush, &row(40, 1.0)), dabs(brush, &row(40, 1.0)));
        assert_eq!(a, b, "the same samples give the same dabs");
        let (lo, hi) = a.iter().fold((f32::MAX, 0.0f32), |m, d| (m.0.min(d.radius), m.1.max(d.radius)));
        assert!(hi > lo * 1.5, "no variation: {lo}..{hi}");
        let moved: Vec<Sample> = row(40, 1.0).into_iter().map(|s| Sample { y: s.y + 3.0, ..s }).collect();
        assert_ne!(
            a.iter().map(|d| d.radius).collect::<Vec<_>>(),
            dabs(brush, &moved).iter().map(|d| d.radius).collect::<Vec<_>>(),
            "another stroke jitters differently"
        );
    }

    #[test]
    fn scatter_moves_dabs_off_the_path_and_angle_jitter_turns_them() {
        let scatter = with_response(|r| r.set(Target::Scatter, Source::Random, 1.0));
        let off = dabs(scatter, &row(40, 1.0)).iter().filter(|d| (d.y - 50.0).abs() > 1.0).count();
        assert!(off > 10, "{off} dabs left the path");
        let turn = with_response(|r| r.set(Target::Angle, Source::Random, 0.5));
        let angles: Vec<f32> = dabs(turn, &row(40, 1.0)).iter().map(|d| d.angle).collect();
        assert!(angles.iter().any(|a| a.abs() > 0.5) && angles.iter().all(|a| a.abs() <= std::f32::consts::PI + 1e-3));
    }

    #[test]
    fn barrel_rotation_turns_the_dab() {
        let brush = with_response(|r| r.set(Target::Angle, Source::Barrel, 1.0));
        let turned = |rotation: f32| -> Vec<f32> {
            let pts: Vec<Sample> = row(6, 1.0).into_iter().map(|s| Sample { rotation, ..s }).collect();
            dabs(brush, &pts).iter().map(|d| d.angle).collect()
        };
        let (flat, quarter) = (turned(0.0), turned(0.25));
        assert!(!flat.is_empty() && flat.iter().all(|a| a.abs() < 1e-4), "{flat:?}");
        assert!(quarter.iter().all(|a| (a - std::f32::consts::FRAC_PI_2).abs() < 1e-3), "{quarter:?}");
        // without a mapping from the barrel, the rotation changes nothing
        let plain = with_response(|_| {});
        let still: Vec<Sample> = row(6, 1.0).into_iter().map(|s| Sample { rotation: 0.4, ..s }).collect();
        assert_eq!(dabs(plain, &still), dabs(plain, &row(6, 1.0)));
    }

    #[test]
    fn barrel_rotation_is_interpolated_the_short_way_round() {
        let a = Sample { rotation: 0.95, ..Sample::at(0.0, 0.0, 1.0, 0.0) };
        let b = Sample { rotation: 0.05, ..Sample::at(10.0, 0.0, 1.0, 0.1) };
        let mid = lerp_sample(&a, &b, 0.5).rotation;
        assert!(!(0.05..0.95).contains(&mid), "went the long way: {mid}");
    }

    #[test]
    fn tip_shape_sets_roundness_and_angle() {
        let blade = Brush { roundness: 0.2, angle: 90.0, ..with_response(|_| {}) };
        let d = dabs(blade, &row(4, 1.0));
        assert!(d.iter().all(|d| (d.roundness - 0.2).abs() < 1e-5 && (d.angle - std::f32::consts::FRAC_PI_2).abs() < 1e-4));
        // a thin blade turned upright covers a tall, narrow patch
        let mut s = Surface::new(80, 80).unwrap();
        let mut st = Stroke::begin(&mut s, Brush { size: 40.0, hardness: 1.0, ..blade });
        st.add(&mut s, &[Dab { x: 40.0, y: 40.0, radius: 20.0, alpha: 1.0, angle: std::f32::consts::FRAC_PI_2, roundness: 0.2, pressure: 1.0, color: None }]);
        st.end(&mut s);
        let (tall, wide) = ((0..80).filter(|y| s.pixel(40, *y)[3] > 128).count(), (0..80).filter(|x| s.pixel(*x, 40)[3] > 128).count());
        assert!(tall > wide * 3, "{tall} tall, {wide} wide");
    }

    #[test]
    fn a_grain_image_decides_where_paint_lands() {
        // columns alternate between peak and valley, one image pixel per canvas pixel
        let mut alpha = vec![0u8; 8 * 8];
        for y in 0..8 {
            for x in 0..8 {
                alpha[y * 8 + x] = if x % 2 == 0 { 255 } else { 0 };
            }
        }
        let grain = Arc::new(crate::Tip::new("columns", 8, 8, alpha).unwrap());
        let brush = Brush { grain: 1.0, grain_kind: GrainKind::Image, grain_scale: 1.0, size: 30.0, hardness: 1.0, opacity: 1.0, ..plain(BrushKind::Chalk) };
        // light pressure only touches the peaks of the grain
        let pts: Vec<Sample> = (0..40).map(|i| Sample::at(30.0 + i as f32 * 2.0, 30.0, 0.3, i as f64 * 0.01)).collect();
        let mut s = Surface::new(120, 60).unwrap();
        paint_with(&mut s, brush, &pts, Images { tip: None, grain: Some(grain) });
        let (even, odd): (u32, u32) =
            (0..100).fold((0, 0), |(e, o), x| if x % 2 == 0 { (e + s.pixel(x, 30)[3] as u32, o) } else { (e, o + s.pixel(x, 30)[3] as u32) });
        assert!(even > odd * 3, "peaks {even} vs valleys {odd}");
        // without the image the brush falls back to paper instead of failing
        let mut t = Surface::new(120, 60).unwrap();
        paint(&mut t, brush, &pts);
        assert!(!t.is_empty());
    }

    #[test]
    fn grain_rotation_and_contrast_change_the_stroke() {
        let stroke = |angle: f32, contrast: f32| {
            let mut s = Surface::new(200, 80).unwrap();
            let brush =
                Brush { grain: 1.0, grain_kind: GrainKind::Canvas, grain_angle: angle, grain_contrast: contrast, size: 24.0, ..plain(BrushKind::Chalk) };
            let pts: Vec<Sample> = (0..60).map(|i| Sample::at(20.0 + i as f32 * 2.5, 40.0, 0.5, i as f64 * 0.01)).collect();
            paint(&mut s, brush, &pts);
            s
        };
        let base = stroke(0.0, 1.0);
        assert_ne!(stroke(30.0, 1.0), base, "rotation does nothing");
        assert_ne!(stroke(0.0, 3.0), base, "contrast does nothing");
        assert_eq!(stroke(30.0, 3.0), stroke(30.0, 3.0), "replays exactly");
    }

    #[test]
    fn built_in_grains_change_the_stroke() {
        let paint_with_kind = |kind: GrainKind| {
            let mut s = Surface::new(200, 80).unwrap();
            let brush = Brush { grain: 1.0, grain_kind: kind, size: 24.0, ..plain(BrushKind::Chalk) };
            let pts: Vec<Sample> = (0..60).map(|i| Sample::at(20.0 + i as f32 * 2.5, 40.0, 0.5, i as f64 * 0.01)).collect();
            paint(&mut s, brush, &pts);
            s
        };
        let paper = paint_with_kind(GrainKind::Paper);
        for kind in [GrainKind::Canvas, GrainKind::Speckle, GrainKind::Clouds] {
            assert_ne!(paint_with_kind(kind), paper, "{kind:?} looks like paper");
        }
    }

    fn block(s: &mut Surface, x0: u32, x1: u32, color: [u8; 4]) {
        for y in 20..60 {
            for x in x0..x1 {
                if let Some(i) = s.tile_index(x / TILE, y / TILE)
                    && let Some(t) = s.tile_mut(i)
                {
                    let (lx, ly) = (x % TILE, y % TILE);
                    let o = ((ly * TILE + lx) * 4) as usize;
                    t.bytes_mut()[o..o + 4].copy_from_slice(&premultiply(color));
                }
            }
        }
    }

    #[test]
    fn smudge_drags_colour_into_empty_space_and_is_one_undo_step() {
        let mut s = Surface::new(200, 80).unwrap();
        block(&mut s, 20, 70, [220, 30, 30, 255]);
        let before = s.clone();
        let brush = Brush { size: 24.0, flow: 0.9, wet: Wet { pickup: 0.2 }, ..plain(BrushKind::Smudge) };
        let pts: Vec<Sample> = (0..60).map(|i| Sample::at(50.0 + i as f32 * 2.0, 40.0, 1.0, i as f64 * 0.01)).collect();
        let undo = paint(&mut s, brush, &pts);
        let smear = s.pixel(90, 40);
        assert!(smear[3] > 40 && smear[0] > smear[2] + 30, "no red smear past the block: {smear:?}");
        assert!(s.pixel(160, 40)[3] < smear[3], "the smear thins out along the stroke");
        // reloading fully from the empty layer at every step leaves no smear behind
        let mut t = before.clone();
        paint(&mut t, Brush { wet: Wet { pickup: 1.0 }, ..brush }, &pts);
        assert!(t.pixel(110, 40)[3] < s.pixel(110, 40)[3] / 2, "{} vs {}", t.pixel(110, 40)[3], s.pixel(110, 40)[3]);
        s.swap_snapshot(undo);
        s.prune();
        let mut b = before;
        b.prune();
        assert_eq!(s, b);
    }

    #[test]
    fn smudging_empty_canvas_paints_nothing() {
        let mut s = Surface::new(100, 80).unwrap();
        paint(&mut s, plain(BrushKind::Smudge), &row(20, 1.0));
        assert!(s.is_empty());
    }

    #[test]
    fn wet_paint_picks_up_the_colour_under_it() {
        let ground = [20, 40, 230, 255];
        let stroke = |pickup: f32| {
            let mut s = Surface::new(240, 80).unwrap();
            block(&mut s, 0, 240, ground);
            let brush =
                Brush { color: [240, 220, 20, 255], size: 20.0, flow: 1.0, opacity: 1.0, hardness: 1.0, wet: Wet { pickup }, ..plain(BrushKind::Watercolour) };
            let pts: Vec<Sample> = (0..80).map(|i| Sample::at(20.0 + i as f32 * 2.5, 40.0, 1.0, i as f64 * 0.01)).collect();
            paint(&mut s, brush, &pts);
            unpremultiply_blue(s.pixel(190, 40))
        };
        let (dry, wet) = (stroke(0.0), stroke(0.8));
        assert!(wet > dry + 40, "blue from the ground should mix in: dry {dry}, wet {wet}");
    }

    fn unpremultiply_blue(p: [u8; 4]) -> i32 {
        tooncraft_raster::unpremultiply(p)[2] as i32
    }

    #[test]
    fn wet_strokes_replay_exactly() {
        let make = || {
            let mut s = Surface::new(160, 80).unwrap();
            block(&mut s, 10, 80, [200, 60, 60, 255]);
            let pts: Vec<Sample> = (0..50).map(|i| Sample::at(30.0 + i as f32 * 2.0, 38.0 + (i as f32 * 0.3).sin() * 6.0, 0.7, i as f64 * 0.01)).collect();
            paint(&mut s, Brush { size: 22.0, ..plain(BrushKind::Smudge) }, &pts);
            paint(&mut s, Brush { color: [30, 200, 60, 255], ..plain(BrushKind::Watercolour) }, &pts);
            s
        };
        assert_eq!(make(), make());
    }

    #[test]
    fn wet_strokes_keep_valid_premultiplied_pixels() {
        let mut s = Surface::new(120, 80).unwrap();
        block(&mut s, 10, 60, [255, 255, 255, 255]);
        let pts: Vec<Sample> = (0..40).map(|i| Sample::at(20.0 + i as f32 * 2.5, 40.0, 1.0, i as f64 * 0.01)).collect();
        paint(&mut s, Brush { size: 30.0, flow: 1.0, wet: Wet { pickup: 0.9 }, ..plain(BrushKind::Smudge) }, &pts);
        for (_, t) in s.tiles() {
            assert!(t.bytes().as_chunks::<4>().0.iter().all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]));
        }
    }
}
