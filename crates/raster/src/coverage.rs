//! Antialiased coverage of filled outlines (text glyphs, filled curves): exact area coverage per
//! pixel with the non-zero rule, by accumulating each edge's signed area into the cells it
//! crosses and summing along each row.
//!
//! Work and memory are bounded: [`MAX_AREA`] cells per rasterised box and [`MAX_EDGES`] edges per
//! call; larger input is refused, never truncated silently.

use crate::{Surface, TILE};

/// Most pixels a single coverage box may hold (a 2048 × 2048 square).
pub const MAX_AREA: u64 = 2048 * 2048;
/// Most edges per rasterisation.
pub const MAX_EDGES: usize = 1 << 20;

/// A line segment of an outline, in pixels.
pub type Edge = ((f32, f32), (f32, f32));

/// Coverage (0 … 1) of a `width`×`height` box whose top-left pixel is `(x0, y0)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Coverage {
    pub x0: i64,
    pub y0: i64,
    pub width: u32,
    pub height: u32,
    pub cells: Vec<f32>,
}

impl Coverage {
    pub fn get(&self, x: u32, y: u32) -> f32 {
        if x >= self.width || y >= self.height {
            return 0.0;
        }
        self.cells.get((y * self.width + x) as usize).copied().unwrap_or(0.0)
    }
}

/// Why an outline was not rasterised.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CoverageError {
    #[error("the shape is too large to fill in one step")]
    TooLarge,
    #[error("the shape has too many edges")]
    TooManyEdges,
}

/// Rasterise closed outlines given as edges, clipped to the box `(0, 0)…clip`. `None` when
/// nothing is inside it.
pub fn rasterize(edges: &[Edge], clip: (u32, u32)) -> Result<Option<Coverage>, CoverageError> {
    if edges.len() > MAX_EDGES {
        return Err(CoverageError::TooManyEdges);
    }
    let good = |p: (f32, f32)| p.0.is_finite() && p.1.is_finite() && p.0.abs() < 1.0e7 && p.1.abs() < 1.0e7;
    let edges: Vec<Edge> = edges.iter().copied().filter(|(a, b)| good(*a) && good(*b)).collect();
    if edges.is_empty() {
        return Ok(None);
    }
    let (mut lx, mut ly, mut hx, mut hy) = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for (a, b) in &edges {
        lx = lx.min(a.0).min(b.0);
        ly = ly.min(a.1).min(b.1);
        hx = hx.max(a.0).max(b.0);
        hy = hy.max(a.1).max(b.1);
    }
    let x0 = (lx.floor() as i64).max(0);
    let y0 = (ly.floor() as i64).max(0);
    let x1 = (hx.ceil() as i64).min(clip.0 as i64);
    let y1 = (hy.ceil() as i64).min(clip.1 as i64);
    if x1 <= x0 || y1 <= y0 {
        return Ok(None);
    }
    let (w, h) = ((x1 - x0) as u32, (y1 - y0) as u32);
    if w as u64 * h as u64 > MAX_AREA {
        return Err(CoverageError::TooLarge);
    }
    // two spare cells per row: area right of the box lands there and is ignored
    let stride = w as usize + 2;
    let mut acc = vec![0.0f32; stride * h as usize];
    let (ox, oy) = (x0 as f32, y0 as f32);
    for &((ax, ay), (bx, by)) in &edges {
        add_edge(&mut acc, stride, w, h, (ax - ox, ay - oy), (bx - ox, by - oy));
    }
    let mut cells = Vec::with_capacity(w as usize * h as usize);
    for row in acc.chunks(stride) {
        let mut sum = 0.0f32;
        for v in row.iter().take(w as usize) {
            sum += v;
            cells.push(sum.abs().min(1.0));
        }
    }
    Ok(Some(Coverage { x0, y0, width: w, height: h, cells }))
}

/// Accumulate one edge (box coordinates) into the area buffer, row by row.
fn add_edge(acc: &mut [f32], stride: usize, w: u32, h: u32, a: (f32, f32), b: (f32, f32)) {
    if a.1 == b.1 {
        return;
    }
    let (dir, top, bot) = if a.1 < b.1 { (1.0f32, a, b) } else { (-1.0f32, b, a) };
    let y_start = top.1.max(0.0);
    let y_end = bot.1.min(h as f32);
    if y_end <= y_start {
        return;
    }
    let slope = (bot.0 - top.0) / (bot.1 - top.1);
    let x_at = |y: f32| top.0 + (y - top.1) * slope;
    let mut y = y_start;
    while y < y_end {
        let row = y.floor();
        let next = (row + 1.0).min(y_end);
        add_span(acc, stride, w, row as usize, dir * (next - y), x_at(y), x_at(next));
        y = next;
    }
}

/// One edge piece inside a row: signed height `dy`, running from `xa` to `xb`. Split at every pixel
/// column it crosses; each piece puts the part of its height right of the line into its cell and
/// the rest into the next cell, so a running sum along the row is the covered share.
fn add_span(acc: &mut [f32], stride: usize, w: u32, row: usize, dy: f32, xa: f32, xb: f32) {
    let wf = w as f32;
    let (xa, xb) = (xa.clamp(0.0, wf), xb.clamp(0.0, wf));
    let base = row * stride;
    let (lo, hi) = if xa <= xb { (xa, xb) } else { (xb, xa) };
    let span = hi - lo;
    let mut put = |cell: f32, v: f32| {
        if let Some(c) = acc.get_mut(base + cell as usize) {
            *c += v;
        }
    };
    if span < 1e-6 {
        let c = lo.floor();
        let mid = lo - c;
        put(c, dy * (1.0 - mid));
        put(c + 1.0, dy * mid);
        return;
    }
    // walk the columns; each piece's height share is its width share of the span
    let mut x = lo;
    while x < hi {
        let c = x.floor();
        let nx = (c + 1.0).min(hi);
        let piece = dy * (nx - x) / span;
        let mid = (x + nx) * 0.5 - c;
        put(c, piece * (1.0 - mid));
        put(c + 1.0, piece * mid);
        x = nx;
    }
}

/// Paint straight RGBA `color` scaled by `cov` over the surface (premultiplied "over").
pub fn paint(surface: &mut Surface, cov: &Coverage, color: [u8; 4]) {
    if cov.width == 0 || cov.height == 0 || color[3] == 0 {
        return;
    }
    let (sw, sh) = (surface.width() as i64, surface.height() as i64);
    let (x_end, y_end) = ((cov.x0 + cov.width as i64).min(sw), (cov.y0 + cov.height as i64).min(sh));
    let (x_start, y_start) = (cov.x0.max(0), cov.y0.max(0));
    if x_end <= x_start || y_end <= y_start {
        return;
    }
    let t = TILE as i64;
    let at = |x: i64, y: i64| cov.get((x - cov.x0) as u32, (y - cov.y0) as u32);
    for ty in y_start / t..=(y_end - 1) / t {
        for tx in x_start / t..=(x_end - 1) / t {
            let (px0, py0) = (tx * t, ty * t);
            let (cx0, cx1, cy0, cy1) = (px0.max(x_start), (px0 + t).min(x_end), py0.max(y_start), (py0 + t).min(y_end));
            if !(cy0..cy1).any(|y| (cx0..cx1).any(|x| at(x, y) > 0.0)) {
                continue;
            }
            let Some(index) = surface.tile_index(tx as u32, ty as u32) else { continue };
            let Some(tile) = surface.tile_mut(index) else { continue };
            let px = tile.bytes_mut();
            for y in cy0..cy1 {
                for x in cx0..cx1 {
                    let k = at(x, y);
                    if k <= 0.0 {
                        continue;
                    }
                    let a = (color[3] as f32 * k).round().clamp(0.0, 255.0);
                    let pm = |c: u8| (c as f32 * a / 255.0).round().clamp(0.0, 255.0) as u8;
                    let src = [pm(color[0]), pm(color[1]), pm(color[2]), a as u8];
                    let i = (((y - py0) * t + (x - px0)) * 4) as usize;
                    if let Some(dst) = px.get_mut(i..i + 4) {
                        let out = crate::pixel::over([dst[0], dst[1], dst[2], dst[3]], src);
                        dst.copy_from_slice(&out);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "coverage_tests.rs"]
mod tests;
