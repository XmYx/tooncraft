//! Image-sequence import (roughs, reference, BoardCraft stills): each image becomes a drawing.

use tooncraft_doc::{Cell, DocError};
use tooncraft_raster::{RgbaImage, Surface, premultiply};

use crate::undo::{Entry, Part};
use crate::{Engine, EngineError, Result};

/// Most images one import takes.
pub const MAX_IMPORT: usize = 2000;

#[derive(Debug, thiserror::Error)]
#[error("{file}: {reason}")]
struct ImportFailure {
    file: String,
    reason: String,
}

fn decode(path: &str, max_side: u32) -> std::result::Result<RgbaImage, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > 512 << 20 {
        return Err("the file is larger than 512 MB".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let (w, h) = craft_image_codecs::probe_size(&bytes).map_err(|e| e.to_string())?;
    if w == 0 || h == 0 || w > max_side * 4 || h > max_side * 4 {
        return Err(format!("the image is {w}×{h}, too large"));
    }
    let img = craft_image_codecs::decode(&bytes).map_err(|e| e.to_string())?;
    let (w, h) = (img.width(), img.height());
    let rgba = img.to_rgba8();
    let mut out = RgbaImage::transparent(w, h);
    for (i, p) in rgba.as_chunks::<4>().0.iter().enumerate() {
        out.set_pixel(i as u32 % w.max(1), i as u32 / w.max(1), premultiply(*p));
    }
    Ok(out)
}

/// Place `img` centred on a `w`×`h` canvas, scaled down (box filter) to fit.
fn fit(img: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    let img = if img.width > w || img.height > h {
        let scale = (img.width as f64 / w as f64).max(img.height as f64 / h as f64);
        img.downscaled(((img.width.max(img.height) as f64) / scale).floor().max(1.0) as u32)
    } else {
        img.clone()
    };
    let mut out = RgbaImage::transparent(w, h);
    let (ox, oy) = ((w.saturating_sub(img.width)) / 2, (h.saturating_sub(img.height)) / 2);
    for y in 0..img.height.min(h) {
        for x in 0..img.width.min(w) {
            out.set_pixel(ox + x, oy + y, img.pixel(x, y));
        }
    }
    out
}

pub(crate) fn images(e: &mut Engine, paths: &[String], frame: u32, hold: u32, name: &str) -> Result<()> {
    if paths.len() > MAX_IMPORT {
        return Err(DocError::TooManyDrawings.into());
    }
    if frame >= e.project.length() {
        return Err(DocError::NoFrame(frame).into());
    }
    let (w, h) = (e.project.width(), e.project.height());
    // Decode everything first: a bad file changes nothing.
    let mut surfaces = Vec::with_capacity(paths.len());
    for p in paths {
        let img = decode(p, w.max(h)).map_err(|reason| EngineError::Import(ImportFailure { file: p.clone(), reason }.to_string()))?;
        let mut s = Surface::new(w, h)?;
        s.load_image(&fit(&img, w, h))?;
        surfaces.push(s);
    }
    let mut parts = vec![Part::Layers(e.project.layers.clone())];
    let index = e.project.add_layer(name, e.project.layers.len())?;
    let len = e.project.length();
    let mut f = frame;
    for s in surfaces {
        if f >= len {
            break;
        }
        let id = e.project.add_drawing(Some(s))?;
        parts.insert(0, Part::Drawing { id, surface: None });
        let layer = e.project.layer_mut(index)?;
        layer.cells[f as usize] = Cell::Key(id);
        for k in 1..hold {
            if let Some(c) = layer.cells.get_mut((f + k) as usize) {
                *c = Cell::Hold;
            }
        }
        f += hold;
    }
    e.push(Entry { label: "Import images".into(), parts });
    Ok(())
}
