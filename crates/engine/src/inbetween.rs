//! The inbetween helper: new, empty drawings between two drawings, placed along a spacing chart,
//! with optional guides (each a blend of the two drawings, weighted by where it falls) on a
//! layer above to draw over.

use tooncraft_doc::{Cell, DocError, DrawingId, Ease, timing::spacing};
use tooncraft_raster::{RgbaImage, Surface};

use crate::undo::{Entry, Part};
use crate::{Engine, EngineError, Result};

/// Most inbetweens added at once.
pub const MAX_INBETWEENS: u32 = 32;

/// Frames for `count` drawings strictly between `a` and `b`, following `ease`.
pub fn frames(a: u32, b: u32, count: u32, ease: Ease) -> Option<Vec<u32>> {
    if b <= a || (b - a) <= count {
        return None;
    }
    let gap = (b - a) as f32;
    let mut out: Vec<u32> = Vec::new();
    for frac in spacing(count, ease) {
        let prev = out.last().copied().unwrap_or(a);
        out.push((a + (frac.clamp(0.0, 1.0) * gap).round() as u32).max(prev + 1));
    }
    let mut limit = b;
    for f in out.iter_mut().rev() {
        if *f >= limit {
            *f = limit - 1;
        }
        limit = *f;
    }
    if out.first().is_some_and(|f| *f <= a) {
        return None;
    }
    Some(out)
}

fn blend(a: &RgbaImage, b: &RgbaImage, t: f32) -> RgbaImage {
    let mut out = a.clone();
    let t = t.clamp(0.0, 1.0);
    for (o, (pa, pb)) in out.data.iter_mut().zip(a.data.iter().zip(b.data.iter())) {
        *o = (*pa as f32 * (1.0 - t) + *pb as f32 * t).round().clamp(0.0, 255.0) as u8;
    }
    out
}

pub(crate) fn add(e: &mut Engine, layer: usize, frame: u32, count: u32, ease: Ease, guides: bool) -> Result<()> {
    e.check_unlocked(layer)?;
    if count == 0 || count > MAX_INBETWEENS {
        return Err(EngineError::Limit("inbetweens (1 to 32 at a time)"));
    }
    let l = e.project.layer(layer)?;
    let a = l.exposure_start(frame).ok_or(EngineError::NoDrawing(frame))?;
    let da = l.drawing_at(a).ok_or(EngineError::NoDrawing(frame))?;
    let len = l.cells.len() as u32;
    let b = (a + 1..len).find(|f| matches!(l.cells.get(*f as usize), Some(Cell::Key(_)))).ok_or(EngineError::NoDrawing(frame + 1))?;
    let db = l.drawing_at(b).ok_or(EngineError::NoDrawing(b))?;
    let at = frames(a, b, count, ease).ok_or(DocError::from(tooncraft_doc::ExposureError::BadRange(a, b)))?;
    let name = l.name.clone();
    let mut parts = vec![Part::Layers(e.project.layers.clone())];
    // the new drawings, held until the next one
    let mut new: Vec<DrawingId> = Vec::new();
    for _ in &at {
        let id = e.project.add_drawing(None)?;
        parts.insert(0, Part::Drawing { id, surface: None });
        new.push(id);
    }
    let l = e.project.layer_mut(layer)?;
    for f in a + 1..b {
        l.cells[f as usize] = Cell::Hold;
    }
    for (f, id) in at.iter().zip(&new) {
        l.set_key(*f, *id);
    }
    if guides {
        let (w, h) = (e.project.width(), e.project.height());
        let img = |d: DrawingId| e.project.drawing(d).map(Surface::to_image).unwrap_or_else(|| RgbaImage::transparent(w, h));
        let (ia, ib) = (img(da), img(db));
        let gi = e.project.add_layer(&format!("{name} guides"), layer + 1)?;
        let mut keys = Vec::new();
        for (f, frac) in at.iter().zip(spacing(count, ease)) {
            let mut s = Surface::new(w, h)?;
            s.load_image(&blend(&ia, &ib, frac))?;
            s.prune();
            let id = e.project.add_drawing(Some(s))?;
            parts.insert(0, Part::Drawing { id, surface: None });
            keys.push((*f, id));
        }
        let g = e.project.layer_mut(gi)?;
        g.opacity = 0.4;
        g.locked = true;
        for (f, id) in keys {
            g.set_key(f, id);
        }
        if let Some(last) = at.last() {
            for f in last + 1..b {
                g.cells[f as usize] = Cell::Hold;
            }
        }
    }
    e.push(Entry { label: "Add inbetweens".into(), parts });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_follow_the_chart_and_stay_between() {
        assert_eq!(frames(0, 8, 3, Ease::Linear), Some(vec![2, 4, 6]));
        assert_eq!(frames(0, 4, 3, Ease::In), Some(vec![1, 2, 3]));
        assert_eq!(frames(0, 3, 3, Ease::Linear), None);
        let slow = frames(0, 12, 3, Ease::Out).unwrap();
        assert!(slow[2] >= 10 && slow.windows(2).all(|w| w[1] > w[0]), "{slow:?}");
    }
}
