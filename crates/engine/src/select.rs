//! Selection commands: transform, clear and paste pixels on one drawing.

use tooncraft_raster::select::{self, Affine};
use tooncraft_raster::{RgbaImage, premultiply};

use crate::undo::{Entry, Part};
use crate::{Engine, EngineError, Result};

/// The centre of a polygon's bounding box (the transform pivot).
pub fn pivot(poly: &[(f32, f32)]) -> (f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in poly {
        x0 = x0.min(p.0);
        y0 = y0.min(p.1);
        x1 = x1.max(p.0);
        y1 = y1.max(p.1);
    }
    if x0 > x1 { (0.0, 0.0) } else { ((x0 + x1) * 0.5, (y0 + y1) * 0.5) }
}

/// The affine of a transform command's numbers.
pub fn affine(poly: &[(f32, f32)], dx: f32, dy: f32, sx: f32, sy: f32, angle: f32) -> Affine {
    Affine::about(pivot(poly), dx, dy, sx, sy, angle)
}

pub(crate) fn transform(e: &mut Engine, layer: usize, frame: u32, poly: &[(f32, f32)], (dx, dy, sx, sy, angle): (f32, f32, f32, f32, f32)) -> Result<()> {
    e.check_unlocked(layer)?;
    let d = e.project.drawing_at(layer, frame).ok_or(EngineError::NoDrawing(frame))?;
    let (w, h) = (e.project.width(), e.project.height());
    let Some(mask) = select::polygon_mask(w, h, poly) else { return Ok(()) };
    let t = affine(poly, dx, dy, sx, sy, angle);
    let s = e.project.drawing_mut(d).ok_or(EngineError::NoDrawing(frame))?;
    s.begin_capture();
    let r = select::transform(s, &mask, t);
    let tiles = s.take_capture();
    if let Err(err) = r {
        s.swap_snapshot(tiles);
        return Err(err.into());
    }
    e.push(Entry { label: "Transform".into(), parts: vec![Part::Tiles { drawing: d, tiles }] });
    Ok(())
}

pub(crate) fn clear(e: &mut Engine, layer: usize, frame: u32, poly: &[(f32, f32)]) -> Result<()> {
    e.check_unlocked(layer)?;
    let d = e.project.drawing_at(layer, frame).ok_or(EngineError::NoDrawing(frame))?;
    let (w, h) = (e.project.width(), e.project.height());
    let Some(mask) = select::polygon_mask(w, h, poly) else { return Ok(()) };
    let s = e.project.drawing_mut(d).ok_or(EngineError::NoDrawing(frame))?;
    s.begin_capture();
    select::clear(s, &mask);
    let tiles = s.take_capture();
    e.push(Entry { label: "Erase selection".into(), parts: vec![Part::Tiles { drawing: d, tiles }] });
    Ok(())
}

pub(crate) fn paste(e: &mut Engine, layer: usize, frame: u32, at: (f32, f32), width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    if width == 0
        || height == 0
        || width > tooncraft_raster::MAX_SIDE
        || height > tooncraft_raster::MAX_SIDE
        || rgba.len() != width as usize * height as usize * 4
    {
        return Err(tooncraft_raster::RasterError::BadImage { got: rgba.len(), want: width as usize * height as usize * 4 }.into());
    }
    let mut img = RgbaImage::transparent(width, height);
    for (i, p) in rgba.as_chunks::<4>().0.iter().enumerate() {
        img.set_pixel(i as u32 % width, i as u32 / width, premultiply(*p));
    }
    let (d, mut parts) = e.ensure_drawing(layer, frame)?;
    let s = e.project.drawing_mut(d).ok_or(EngineError::NoDrawing(frame))?;
    s.begin_capture();
    let r = select::draw_transformed(s, &img, at, Affine::IDENTITY);
    let tiles = s.take_capture();
    if let Err(err) = r {
        s.swap_snapshot(tiles);
        return Err(err.into());
    }
    parts.push(Part::Tiles { drawing: d, tiles });
    e.push(Entry { label: "Paste".into(), parts });
    Ok(())
}
