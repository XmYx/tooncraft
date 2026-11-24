//! Liquify: push, twirl, pinch or bloat a drawing's pixels with a soft round brush. A stroke's
//! dabs add up into a displacement field over the area they touch; the drawing is then
//! resampled (bilinear, premultiplied) from how it was before the stroke. One undo step.

use serde::{Deserialize, Serialize};

use crate::undo::{Entry, Part};
use crate::{Engine, EngineError, Result};

/// Most path points one stroke keeps.
pub const MAX_LIQUIFY_POINTS: usize = 20_000;
/// Largest brush radius, pixels.
pub const MAX_LIQUIFY_RADIUS: f32 = 500.0;
/// Most pixels the stroke's area may hold (its displacement field is 8 bytes a pixel).
pub const MAX_LIQUIFY_PIXELS: u64 = 8_000_000;
/// Most pixel steps the dabs may take together.
pub const MAX_LIQUIFY_WORK: u64 = 300_000_000;
/// Most dabs along one stroke.
pub const MAX_LIQUIFY_DABS: u64 = 1_000_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiquifyMode {
    /// Drag the pixels along with the brush.
    #[default]
    Push,
    /// Turn them counter-clockwise round the brush.
    TwirlLeft,
    /// Turn them clockwise.
    TwirlRight,
    /// Draw them in towards the brush centre.
    Pinch,
    /// Swell them out from it.
    Bloat,
}

/// The brush: radius in pixels and strength (0…1).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LiquifyBrush {
    pub radius: f32,
    #[serde(default = "half")]
    pub strength: f32,
}

fn half() -> f32 {
    0.5
}

impl Default for LiquifyBrush {
    fn default() -> Self {
        Self { radius: 40.0, strength: 0.5 }
    }
}

impl LiquifyBrush {
    pub fn sanitized(self) -> Self {
        let d = Self::default();
        let clamp = |v: f32, lo: f32, hi: f32, def: f32| if v.is_finite() { v.clamp(lo, hi) } else { def };
        Self { radius: clamp(self.radius, 2.0, MAX_LIQUIFY_RADIUS, d.radius), strength: clamp(self.strength, 0.0, 1.0, d.strength) }
    }
}

/// Liquify `layer` at `frame` along `points` (canvas pixels).
pub fn liquify(e: &mut Engine, layer: usize, frame: u32, mode: LiquifyMode, brush: LiquifyBrush, points: &[(f32, f32)]) -> Result<()> {
    e.check_unlocked(layer)?;
    let brush = brush.sanitized();
    let points: Vec<(f32, f32)> = points.iter().copied().filter(|p| p.0.is_finite() && p.1.is_finite()).take(MAX_LIQUIFY_POINTS).collect();
    if points.is_empty() || brush.strength <= 0.0 {
        return Ok(());
    }
    let (w, h) = (e.project.width(), e.project.height());
    let r = brush.radius;
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in &points {
        x0 = x0.min(p.0 - r);
        y0 = y0.min(p.1 - r);
        x1 = x1.max(p.0 + r);
        y1 = y1.max(p.1 + r);
    }
    let bx0 = x0.floor().clamp(0.0, w as f32) as u32;
    let by0 = y0.floor().clamp(0.0, h as f32) as u32;
    let bx1 = x1.ceil().clamp(0.0, w as f32) as u32;
    let by1 = y1.ceil().clamp(0.0, h as f32) as u32;
    if bx1 <= bx0 || by1 <= by0 {
        return Ok(());
    }
    let (bw, bh) = (bx1 - bx0, by1 - by0);
    if bw as u64 * bh as u64 > MAX_LIQUIFY_PIXELS {
        return Err(EngineError::Limit("pixels in one liquify stroke"));
    }
    // the dabs (centre and the move since the last one), costed before any work
    let step = (r * 0.25).max(1.0);
    let clipped = |cx: f32, cy: f32| -> (i64, i64, i64, i64) {
        (
            ((cx - r).floor() as i64).max(bx0 as i64),
            ((cy - r).floor() as i64).max(by0 as i64),
            ((cx + r).ceil() as i64).min(bx1 as i64),
            ((cy + r).ceil() as i64).min(by1 as i64),
        )
    };
    let mut dabs: Vec<((f32, f32), (f32, f32))> = Vec::new();
    let (mut work, mut steps) = (0u64, 0u64);
    let mut last = points[0];
    let mut add = |c: (f32, f32), mv: (f32, f32), dabs: &mut Vec<((f32, f32), (f32, f32))>| -> Result<()> {
        steps += 1;
        if steps > MAX_LIQUIFY_DABS {
            return Err(EngineError::Limit("dabs in one liquify stroke"));
        }
        let (lx, ly, hx, hy) = clipped(c.0, c.1);
        if hx > lx && hy > ly {
            work = work.saturating_add(((hx - lx) * (hy - ly)) as u64);
            if work > MAX_LIQUIFY_WORK {
                return Err(EngineError::Limit("pixels moved by one liquify stroke"));
            }
            dabs.push((c, mv));
        }
        Ok(())
    };
    add(last, (0.0, 0.0), &mut dabs)?;
    for &p in &points[1..] {
        let len = (p.0 - last.0).hypot(p.1 - last.1);
        let n = ((len / step).ceil() as u64).clamp(1, MAX_LIQUIFY_DABS);
        let mv = ((p.0 - last.0) / n as f32, (p.1 - last.1) / n as f32);
        for k in 1..=n {
            let t = k as f32 / n as f32;
            add((last.0 + (p.0 - last.0) * t, last.1 + (p.1 - last.1) * t), mv, &mut dabs)?;
        }
        last = p;
    }
    // the displacement: each output pixel samples the old drawing at (x, y) − field
    let mut field = vec![(0.0f32, 0.0f32); bw as usize * bh as usize];
    let k = brush.strength;
    for (c, mv) in dabs {
        let (lx, ly, hx, hy) = clipped(c.0, c.1);
        for y in ly..hy {
            for x in lx..hx {
                let (px, py) = (x as f32 + 0.5 - c.0, y as f32 + 0.5 - c.1);
                let d2 = (px * px + py * py) / (r * r);
                if d2 >= 1.0 {
                    continue;
                }
                let f = (1.0 - d2) * (1.0 - d2) * k;
                let (dx, dy) = match mode {
                    LiquifyMode::Push => (mv.0 * f, mv.1 * f),
                    LiquifyMode::TwirlLeft | LiquifyMode::TwirlRight => {
                        let a = if mode == LiquifyMode::TwirlLeft { -0.15 } else { 0.15 } * f;
                        let (s, co) = a.sin_cos();
                        // the point this pixel came from, turned back by the angle
                        let (sx, sy) = (px * co + py * s, -px * s + py * co);
                        (px - sx, py - sy)
                    }
                    LiquifyMode::Bloat => (px * f * 0.08, py * f * 0.08),
                    LiquifyMode::Pinch => (-px * f * 0.08, -py * f * 0.08),
                };
                if let Some(v) = field.get_mut((y - by0 as i64) as usize * bw as usize + (x - bx0 as i64) as usize) {
                    v.0 += dx;
                    v.1 += dy;
                }
            }
        }
    }
    let (drawing, mut parts) = e.ensure_drawing(layer, frame)?;
    let s = e.project.drawing_mut(drawing).ok_or(EngineError::NoDrawing(frame))?;
    let before = s.clone();
    let sample = |x: f32, y: f32| -> [u8; 4] {
        let (fx, fy) = (x - 0.5, y - 0.5);
        let (ix, iy) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - ix, fy - iy);
        let (ix, iy) = (ix as i64, iy as i64);
        let p = |dx: i64, dy: i64| before.pixel(ix + dx, iy + dy);
        let (a, b, c, d) = (p(0, 0), p(1, 0), p(0, 1), p(1, 1));
        let mut out = [0u8; 4];
        for i in 0..4 {
            let top = a[i] as f32 + (b[i] as f32 - a[i] as f32) * tx;
            let bot = c[i] as f32 + (d[i] as f32 - c[i] as f32) * tx;
            out[i] = (top + (bot - top) * ty).round().clamp(0.0, 255.0) as u8;
        }
        out
    };
    s.begin_capture();
    let t = tooncraft_raster::TILE;
    for ty in by0 / t..=(by1 - 1) / t {
        for tx in bx0 / t..=(bx1 - 1) / t {
            let (cx0, cy0, cx1, cy1) = ((tx * t).max(bx0), (ty * t).max(by0), (tx * t + t).min(bx1), (ty * t + t).min(by1));
            let at = |x: u32, y: u32| field.get(((y - by0) * bw + (x - bx0)) as usize).copied().unwrap_or((0.0, 0.0));
            let moved = |v: (f32, f32)| v.0.abs() > 0.01 || v.1.abs() > 0.01;
            if !(cy0..cy1).any(|y| (cx0..cx1).any(|x| moved(at(x, y)))) {
                continue;
            }
            let Some(index) = s.tile_index(tx, ty) else { continue };
            let Some(tile) = s.tile_mut(index) else { continue };
            let px = tile.bytes_mut();
            for y in cy0..cy1 {
                for x in cx0..cx1 {
                    let v = at(x, y);
                    if !moved(v) {
                        continue;
                    }
                    let out = sample(x as f32 + 0.5 - v.0, y as f32 + 0.5 - v.1);
                    let i = (((y - ty * t) * t + (x - tx * t)) * 4) as usize;
                    if let Some(dst) = px.get_mut(i..i + 4) {
                        dst.copy_from_slice(&out);
                    }
                }
            }
        }
    }
    parts.push(Part::Tiles { drawing, tiles: s.take_capture() });
    e.push(Entry { label: "Liquify".into(), parts });
    Ok(())
}
