//! The clone stamp: paint pixels copied from an offset along a path with a soft round tip. The
//! source is the same layer or another layer's drawing at the same frame, read as it was before
//! the stroke (the stamp never copies its own paint). One undo step per stroke.

use serde::{Deserialize, Serialize};

use crate::undo::{Entry, Part};
use crate::{Engine, EngineError, Result};

/// Most path points one stamp stroke keeps.
pub const MAX_CLONE_POINTS: usize = 20_000;
/// Largest tip diameter, in pixels.
pub const MAX_CLONE_SIZE: f32 = 1000.0;
/// Most pixels one stroke may cover (its bounding box).
pub const MAX_CLONE_PIXELS: u64 = 40_000_000;
/// Most pixel steps one stroke may take, summed over its dabs (each dab's area): a long
/// 1000 px stroke fits (about 45 000 px of path), a hostile zig-zag is refused up front.
pub const MAX_CLONE_WORK: u64 = 300_000_000;
/// Most dabs along one stroke's path, painted or not.
pub const MAX_CLONE_DABS: u64 = 4_000_000;

/// The stamp's tip: diameter in pixels, hardness (0 soft … 1 hard edge) and opacity (0…1).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CloneTip {
    pub size: f32,
    #[serde(default = "half")]
    pub hardness: f32,
    #[serde(default = "one")]
    pub opacity: f32,
}

fn half() -> f32 {
    0.5
}

fn one() -> f32 {
    1.0
}

impl Default for CloneTip {
    fn default() -> Self {
        Self { size: 40.0, hardness: 0.5, opacity: 1.0 }
    }
}

impl CloneTip {
    /// Within the limits; NaN becomes the default.
    pub fn sanitized(self) -> Self {
        let d = Self::default();
        let clamp = |v: f32, lo: f32, hi: f32, def: f32| if v.is_finite() { v.clamp(lo, hi) } else { def };
        Self {
            size: clamp(self.size, 1.0, MAX_CLONE_SIZE, d.size),
            hardness: clamp(self.hardness, 0.0, 1.0, d.hardness),
            opacity: clamp(self.opacity, 0.0, 1.0, d.opacity),
        }
    }

    /// How much of the copy a pixel `dist` from the dab centre takes (0…1).
    fn falloff(&self, dist: f32) -> f32 {
        let r = self.size / 2.0;
        // the outer half pixel is antialiased even at full hardness
        let inner = (r * self.hardness).min(r - 0.5).max(0.0);
        if dist <= inner {
            1.0
        } else if dist >= r + 0.5 {
            0.0
        } else {
            let span = (r + 0.5 - inner).max(1e-3);
            let f = 1.0 - (dist - inner) / span;
            // smooth the soft part
            f * f * (3.0 - 2.0 * f)
        }
    }
}

/// Paint the clone stamp on `layer` at `frame` along `points`: each pixel takes the pixel at
/// `offset` from it (source minus destination) on `source_layer` (this layer when absent), mixed
/// in by the tip. Overlapping dabs never add up: each pixel keeps the strongest dab over it.
pub fn clone_stamp(
    e: &mut Engine,
    layer: usize,
    frame: u32,
    source_layer: Option<usize>,
    offset: (f32, f32),
    tip: CloneTip,
    points: &[(f32, f32)],
) -> Result<()> {
    if !(offset.0.is_finite() && offset.1.is_finite()) {
        return Err(EngineError::Import("the clone offset must be finite".into()));
    }
    e.check_unlocked(layer)?;
    let src_layer = source_layer.unwrap_or(layer);
    e.project.layer(src_layer)?;
    let tip = tip.sanitized();
    let points: Vec<(f32, f32)> = points.iter().copied().filter(|p| p.0.is_finite() && p.1.is_finite()).take(MAX_CLONE_POINTS).collect();
    if points.is_empty() || tip.opacity <= 0.0 {
        return Ok(());
    }
    let (w, h) = (e.project.width(), e.project.height());
    let r = tip.size / 2.0 + 1.0;
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
    if bw as u64 * bh as u64 > MAX_CLONE_PIXELS {
        return Err(EngineError::Limit("pixels in one clone stroke"));
    }
    // the dab centres along the path, and the pixel work they cost (each dab's area clipped
    // to the box), checked before any pixel is touched: a big tip on a long zig-zag path is
    // refused instead of running for minutes
    let step = (tip.size * 0.15).max(1.0);
    let clipped = |cx: f32, cy: f32| -> (i64, i64, i64, i64) {
        let (lx, ly) = (((cx - r).floor() as i64).max(bx0 as i64), ((cy - r).floor() as i64).max(by0 as i64));
        let (hx, hy) = (((cx + r).ceil() as i64).min(bx1 as i64), ((cy + r).ceil() as i64).min(by1 as i64));
        (lx, ly, hx, hy)
    };
    let mut centres: Vec<(f32, f32)> = Vec::new();
    let mut work: u64 = 0;
    let mut add = |cx: f32, cy: f32, centres: &mut Vec<(f32, f32)>| -> Result<()> {
        let (lx, ly, hx, hy) = clipped(cx, cy);
        if hx > lx && hy > ly {
            work = work.saturating_add(((hx - lx) * (hy - ly)) as u64);
            if work > MAX_CLONE_WORK {
                return Err(EngineError::Limit("pixels painted by one clone stroke"));
            }
            centres.push((cx, cy));
        }
        Ok(())
    };
    let Some(&first) = points.first() else { return Ok(()) };
    add(first.0, first.1, &mut centres)?;
    let mut steps: u64 = 0;
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let len = (b.0 - a.0).hypot(b.1 - a.1);
        // dabs off the box paint nothing but still take a step: cap the steps too
        let n = ((len / step).ceil() as u64).clamp(1, MAX_CLONE_DABS);
        steps = steps.saturating_add(n);
        if steps > MAX_CLONE_DABS {
            return Err(EngineError::Limit("dabs in one clone stroke"));
        }
        for k in 1..=n {
            let t = k as f32 / n as f32;
            add(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, &mut centres)?;
        }
    }
    // the strongest dab over each pixel
    let mut cover = vec![0.0f32; bw as usize * bh as usize];
    for (cx, cy) in centres {
        let (lx, ly, hx, hy) = clipped(cx, cy);
        for y in ly..hy {
            for x in lx..hx {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let f = tip.falloff(d);
                if f > 0.0 {
                    let i = (y - by0 as i64) as usize * bw as usize + (x - bx0 as i64) as usize;
                    if let Some(c) = cover.get_mut(i) {
                        *c = c.max(f);
                    }
                }
            }
        }
    }
    // read the source as it is before painting
    let source = match e.project.drawing_at(src_layer, frame) {
        Some(id) => e.project.drawing(id).cloned(),
        None => None,
    };
    let (drawing, mut parts) = e.ensure_drawing(layer, frame)?;
    let s = e.project.drawing_mut(drawing).ok_or(EngineError::NoDrawing(frame))?;
    let source = match source {
        Some(src) => src,
        // no drawing there: the source is empty
        None if src_layer != layer => tooncraft_raster::Surface::new(w, h).map_err(|err| EngineError::Import(err.to_string()))?,
        None => s.clone(),
    };
    let (ox, oy) = (offset.0.round() as i64, offset.1.round() as i64);
    s.begin_capture();
    let t = tooncraft_raster::TILE;
    for ty in by0 / t..=(by1 - 1) / t {
        for tx in bx0 / t..=(bx1 - 1) / t {
            let Some(index) = s.tile_index(tx, ty) else { continue };
            // only touch tiles a dab reaches
            let (cx0, cy0) = ((tx * t).max(bx0), (ty * t).max(by0));
            let (cx1, cy1) = ((tx * t + t).min(bx1), (ty * t + t).min(by1));
            let at = |x: u32, y: u32| cover.get(((y - by0) * bw + (x - bx0)) as usize).copied().unwrap_or(0.0);
            if !(cy0..cy1).any(|y| (cx0..cx1).any(|x| at(x, y) > 0.0)) {
                continue;
            }
            let Some(tile) = s.tile_mut(index) else { continue };
            let px = tile.bytes_mut();
            for y in cy0..cy1 {
                for x in cx0..cx1 {
                    let wgt = at(x, y) * tip.opacity;
                    if wgt <= 0.0 {
                        continue;
                    }
                    let from = source.pixel(x as i64 + ox, y as i64 + oy);
                    let i = (((y - ty * t) * t + (x - tx * t)) * 4) as usize;
                    let Some(dst) = px.get_mut(i..i + 4) else { continue };
                    for c in 0..4 {
                        let d = dst[c] as f32;
                        dst[c] = (d + (from[c] as f32 - d) * wgt).round().clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }
    }
    parts.push(Part::Tiles { drawing, tiles: s.take_capture() });
    e.push(Entry { label: "Clone".into(), parts });
    Ok(())
}
