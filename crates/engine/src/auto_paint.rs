//! Paint the same regions across drawings: a painted drawing is the reference; in each other
//! drawing of the layer, every enclosed empty region takes the colour the reference has most
//! under it (when that colour covers enough of the region), filled with the usual fill, so the
//! character's colours carry from one drawing to the next. One undo step for all of it.

use std::collections::{BTreeSet, HashMap};

use tooncraft_raster::fill::FillOptions;

use crate::undo::Entry;
use crate::{Engine, EngineError, FillReference, Result};

/// Most drawings one auto paint goes through.
pub const MAX_AUTO_DRAWINGS: usize = 256;
/// Most canvas pixels auto paint works on.
pub const MAX_AUTO_PIXELS: u64 = 16_000_000;
/// Most regions filled in one drawing.
pub const MAX_AUTO_REGIONS: usize = 2048;
/// Smallest region (pixels) worth a fill.
pub const MIN_REGION: usize = 6;
/// Share of a region the reference colour must cover.
pub const MIN_SHARE: f32 = 0.3;
/// Most pixel visits one auto paint may make (drawings × canvas pixels), checked before any work.
pub const MAX_AUTO_WORK: u64 = 256_000_000;

/// A pixel that bounds a region: drawn paint (enough opacity).
fn solid(p: [u8; 4]) -> bool {
    p[3] >= 128
}

/// The regions of empty pixels in `drawing` (4-connected): each one's seed, size and the most
/// common opaque colour of `reference` under it with its count. Nothing is kept per pixel.
fn regions(e: &Engine, drawing: tooncraft_doc::DrawingId, reference: &tooncraft_raster::Surface) -> Vec<((u32, u32), usize, Option<([u8; 4], usize)>)> {
    let (w, h) = (e.project.width(), e.project.height());
    let Some(s) = e.project.drawing(drawing) else { return Vec::new() };
    let mut seen = vec![false; w as usize * h as usize];
    let mut out = Vec::new();
    let mut stack = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if seen.get(i).copied().unwrap_or(true) || solid(s.pixel(x as i64, y as i64)) {
                continue;
            }
            // iterative flood over empty pixels, counting the reference's colours as it goes
            let mut counts: HashMap<[u8; 4], usize> = HashMap::new();
            let mut size = 0usize;
            stack.clear();
            stack.push((x, y));
            if let Some(v) = seen.get_mut(i) {
                *v = true;
            }
            while let Some((cx, cy)) = stack.pop() {
                size += 1;
                let p = reference.pixel(cx as i64, cy as i64);
                if p[3] >= 200 && counts.len() < 4096 {
                    *counts.entry(tooncraft_raster::unpremultiply(p)).or_default() += 1;
                }
                let n = [(cx.wrapping_sub(1), cy), (cx + 1, cy), (cx, cy.wrapping_sub(1)), (cx, cy + 1)];
                for (nx, ny) in n {
                    if nx >= w || ny >= h {
                        continue;
                    }
                    let j = (ny * w + nx) as usize;
                    if seen.get(j).copied().unwrap_or(true) || solid(s.pixel(nx as i64, ny as i64)) {
                        continue;
                    }
                    if let Some(v) = seen.get_mut(j) {
                        *v = true;
                    }
                    stack.push((nx, ny));
                }
            }
            let best = counts.into_iter().max_by_key(|(c, n)| (*n, *c));
            out.push(((x, y), size, best));
        }
    }
    out
}

/// Paint the regions of the drawings at `frames` on `layer` like the drawing at `from`.
pub fn auto_paint(e: &mut Engine, layer: usize, from: u32, frames: &[u32], options: FillOptions, reference: FillReference) -> Result<usize> {
    e.check_unlocked(layer)?;
    let (w, h) = (e.project.width(), e.project.height());
    if w as u64 * h as u64 > MAX_AUTO_PIXELS {
        return Err(EngineError::Limit("canvas pixels for painting drawings alike"));
    }
    let l = e.project.layer(layer)?;
    let r = l.drawing_at(from).ok_or(EngineError::NoDrawing(from))?;
    // each other drawing once, in frame order
    let mut seen = BTreeSet::from([r]);
    let mut targets = Vec::new();
    for &f in frames {
        if let Some(d) = l.drawing_at(f)
            && seen.insert(d)
        {
            targets.push(f);
        }
    }
    if targets.len() > MAX_AUTO_DRAWINGS {
        return Err(EngineError::Limit("drawings to paint at once"));
    }
    if (targets.len() as u64).saturating_mul(w as u64 * h as u64) > MAX_AUTO_WORK {
        return Err(EngineError::Limit("pixels to look at for painting drawings alike"));
    }
    let reference_surface = e.project.drawing(r).cloned().ok_or(EngineError::NoDrawing(from))?;
    let mut parts = Vec::new();
    let mut filled = 0;
    for f in targets {
        let Some(d) = e.project.layer(layer)?.drawing_at(f) else { continue };
        // what each region gets: the most common opaque colour of the reference under it
        let mut plan = Vec::new();
        for (seed, size, best) in regions(e, d, &reference_surface) {
            if size < MIN_REGION {
                continue;
            }
            if let Some((c, n)) = best
                && n as f32 >= size as f32 * MIN_SHARE
            {
                plan.push((seed, c));
            }
            if plan.len() >= MAX_AUTO_REGIONS {
                break;
            }
        }
        for (seed, c) in plan {
            match e.fill_parts_for(layer, f, seed, c, options, reference) {
                Ok(Some(p)) => {
                    parts.extend(p);
                    filled += 1;
                }
                Ok(None) => {}
                Err(err) => {
                    // undo the fills made so far: nothing is left changed outside undo
                    let _ = Entry { label: String::new(), parts }.swap(&mut e.project);
                    return Err(err);
                }
            }
        }
    }
    if !parts.is_empty() {
        e.push(Entry { label: "Paint drawings alike".into(), parts });
    }
    Ok(filled)
}
