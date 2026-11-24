//! Selection commands: transform, clear and paste pixels on one drawing.

use tooncraft_raster::fill::Mask;
use tooncraft_raster::select::{self, Affine, Resample};
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

/// Most rings a selection command takes (the traced outline of a busy magic-wand area can have
/// many; past this the rest are ignored).
pub const MAX_RINGS: usize = 4096;

/// The mask of a selection's main ring and its other rings (even-odd).
pub fn selection_mask(w: u32, h: u32, poly: &[(f32, f32)], rings: &[Vec<(f32, f32)>]) -> Option<Mask> {
    let mut all: Vec<&[(f32, f32)]> = vec![poly];
    all.extend(rings.iter().take(MAX_RINGS).map(|r| r.as_slice()));
    select::rings_mask(w, h, &all)
}

/// A selection as the commands take it: the main ring, the other rings, the feather (pixels).
pub type Area<'a> = (&'a [(f32, f32)], &'a [Vec<(f32, f32)>], f32);

/// [`selection_mask`] with its edge softened by `feather` pixels (0: hard).
pub fn soft_mask(w: u32, h: u32, poly: &[(f32, f32)], rings: &[Vec<(f32, f32)>], feather: f32) -> Option<Mask> {
    let m = selection_mask(w, h, poly, rings)?;
    select::feather(&m, feather, w, h)
}

/// The affine of a transform command's numbers.
pub fn affine(poly: &[(f32, f32)], dx: f32, dy: f32, sx: f32, sy: f32, angle: f32) -> Affine {
    Affine::about(pivot(poly), dx, dy, sx, sy, angle)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn transform(
    e: &mut Engine,
    layer: usize,
    frame: u32,
    (poly, rings, feather): Area,
    (dx, dy, sx, sy, angle): (f32, f32, f32, f32, f32),
    skew: (f32, f32),
    resample: Resample,
) -> Result<()> {
    e.check_unlocked(layer)?;
    let d = e.project.drawing_at(layer, frame).ok_or(EngineError::NoDrawing(frame))?;
    let (w, h) = (e.project.width(), e.project.height());
    let Some(mask) = soft_mask(w, h, poly, rings, feather) else { return Ok(()) };
    let t = Affine::about_skewed(pivot(poly), dx, dy, sx, sy, angle, skew);
    let s = e.project.drawing_mut(d).ok_or(EngineError::NoDrawing(frame))?;
    s.begin_capture();
    let r = select::transform_with(s, &mask, t, resample);
    let tiles = s.take_capture();
    if let Err(err) = r {
        s.swap_snapshot(tiles);
        return Err(err.into());
    }
    e.push(Entry { label: "Transform".into(), parts: vec![Part::Tiles { drawing: d, tiles }] });
    Ok(())
}

pub(crate) fn distort(e: &mut Engine, layer: usize, frame: u32, (poly, rings, feather): Area, corners: [(f32, f32); 4], resample: Resample) -> Result<()> {
    e.check_unlocked(layer)?;
    let d = e.project.drawing_at(layer, frame).ok_or(EngineError::NoDrawing(frame))?;
    let (w, h) = (e.project.width(), e.project.height());
    let Some(mask) = soft_mask(w, h, poly, rings, feather) else { return Ok(()) };
    let s = e.project.drawing_mut(d).ok_or(EngineError::NoDrawing(frame))?;
    s.begin_capture();
    let r = select::distort(s, &mask, corners, resample);
    let tiles = s.take_capture();
    if let Err(err) = r {
        s.swap_snapshot(tiles);
        return Err(err.into());
    }
    e.push(Entry { label: "Distort".into(), parts: vec![Part::Tiles { drawing: d, tiles }] });
    Ok(())
}

/// The area a magic-wand click at `(x, y)` selects: pixels like the one clicked, on the drawing
/// alone or on everything visible, within `tolerance`. Read-only (a selection is not part of the
/// shot).
pub fn wand(e: &Engine, layer: usize, frame: u32, x: u32, y: u32, tolerance: u8, all_layers: bool) -> Option<Mask> {
    wand_with(e, layer, frame, x, y, tolerance, all_layers, true)
}

/// [`wand`], or with `contiguous` off: every pixel of the picture like the one clicked, wherever
/// it is.
#[allow(clippy::too_many_arguments)]
pub fn wand_with(e: &Engine, layer: usize, frame: u32, x: u32, y: u32, tolerance: u8, all_layers: bool, contiguous: bool) -> Option<Mask> {
    let (w, h) = (e.project.width(), e.project.height());
    if x >= w || y >= h || w as u64 * h as u64 > select::MAX_SELECTION_PIXELS {
        return None;
    }
    let opts = tooncraft_doc::RenderOptions { paper: false, only_layer: (!all_layers).then_some(layer), effects: false, ..Default::default() };
    let img = tooncraft_doc::render_region(&e.project, frame, (0, 0, w, h), opts);
    if !contiguous {
        let seed = img.pixel(x, y);
        let near = |p: [u8; 4]| (0..4).all(|k| p[k].abs_diff(seed[k]) <= tolerance);
        let data: Vec<u8> = img.data.as_chunks::<4>().0.iter().map(|p| if near(*p) { 255 } else { 0 }).collect();
        return Some(Mask { x: 0, y: 0, width: w, height: h, data });
    }
    let options = tooncraft_raster::fill::FillOptions { tolerance, gap: 0, spread: 0, ..Default::default() };
    tooncraft_raster::fill::flood(&img, (0, 0), (x, y), options).ok().flatten()
}

/// Every pixel of the layer's drawing at `frame` whose colour is within `tolerance` (per channel)
/// of the straight colour `rgb`, ignoring opacity; fully clear pixels are left out. `None` when
/// the canvas is too big to select or nothing matches.
pub fn colour_mask(e: &Engine, layer: usize, frame: u32, rgb: [u8; 3], tolerance: u8) -> Option<Mask> {
    let (w, h) = (e.project.width(), e.project.height());
    if w as u64 * h as u64 > select::MAX_SELECTION_PIXELS {
        return None;
    }
    let opts = tooncraft_doc::RenderOptions { paper: false, only_layer: Some(layer), effects: false, ..Default::default() };
    let img = tooncraft_doc::render_region(&e.project, frame, (0, 0, w, h), opts);
    let near = |p: &[u8; 4]| p[3] > 0 && (0..3).all(|k| tooncraft_raster::unpremultiply(*p)[k].abs_diff(rgb[k]) <= tolerance);
    let data: Vec<u8> = img.data.as_chunks::<4>().0.iter().map(|p| if near(p) { 255 } else { 0 }).collect();
    data.contains(&255).then_some(Mask { x: 0, y: 0, width: w, height: h, data })
}

pub(crate) fn clear(e: &mut Engine, layer: usize, frame: u32, (poly, rings, feather): Area) -> Result<()> {
    e.check_unlocked(layer)?;
    let d = e.project.drawing_at(layer, frame).ok_or(EngineError::NoDrawing(frame))?;
    let (w, h) = (e.project.width(), e.project.height());
    let Some(mask) = soft_mask(w, h, poly, rings, feather) else { return Ok(()) };
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
