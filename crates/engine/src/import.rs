//! Image-sequence import (roughs, reference, BoardCraft stills): each image becomes a drawing.

use tooncraft_doc::{Cell, DocError};
use tooncraft_raster::{RgbaImage, Surface, premultiply};

use crate::undo::{Entry, Part};
use crate::{Engine, EngineError, Result};

/// Most images one import takes.
pub const MAX_IMPORT: usize = 2000;
/// Most layers one import by file names makes.
pub const MAX_IMPORT_LAYERS: usize = 64;

/// How an image is placed on the canvas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    /// Centred, scaled down to fit inside the canvas (never up).
    #[default]
    Fit,
    /// Centred at its own size; what falls outside the canvas is cut.
    Actual,
    /// Centred, scaled so it covers the whole canvas (up or down).
    Fill,
}

/// The import's choices.
#[derive(Clone, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ImportOptions {
    pub placement: Placement,
    /// Into this existing layer (its cells from `frame` on are replaced) instead of a new one.
    pub into_layer: Option<usize>,
    /// One new layer per file name (`walk_0001.png`, `walk_0002.png` … make a layer "walk").
    pub by_name: bool,
    /// The files' colours are already premultiplied by their alpha (most are not).
    pub premultiplied: bool,
}

/// The name a numbered file belongs to: the file name without extension and trailing digits,
/// separators trimmed (`run_012.png` → `run`).
pub fn sequence_name(path: &str) -> String {
    let file = std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let base = file.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end_matches(['_', '-', '.', ' ']);
    let base = if base.is_empty() { file.as_str() } else { base };
    base.chars().take(tooncraft_doc::MAX_NAME).collect()
}

#[derive(Debug, thiserror::Error)]
#[error("{file}: {reason}")]
struct ImportFailure {
    file: String,
    reason: String,
}

fn decode(path: &str, max_side: u32, premultiplied: bool) -> std::result::Result<RgbaImage, String> {
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
        let px = if premultiplied { [p[0].min(p[3]), p[1].min(p[3]), p[2].min(p[3]), p[3]] } else { premultiply(*p) };
        out.set_pixel(i as u32 % w.max(1), i as u32 / w.max(1), px);
    }
    Ok(out)
}

/// Place `img` centred on a `w`×`h` canvas as `placement` says.
pub(crate) fn place(img: &RgbaImage, w: u32, h: u32, placement: Placement) -> RgbaImage {
    let (iw, ih) = (img.width.max(1) as f64, img.height.max(1) as f64);
    let scale = match placement {
        Placement::Fit => (w as f64 / iw).min(h as f64 / ih).min(1.0),
        Placement::Actual => 1.0,
        Placement::Fill => (w as f64 / iw).max(h as f64 / ih),
    };
    let mut out = RgbaImage::transparent(w, h);
    let (sw, sh) = (iw * scale, ih * scale);
    let (ox, oy) = ((w as f64 - sw) / 2.0, (h as f64 - sh) / 2.0);
    if scale < 1.0 {
        // shrinking: average blocks first so detail is not lost to aliasing
        let small = img.downscaled((iw.max(ih) * scale).floor().max(1.0) as u32);
        let (ox, oy) = (((w as i64 - small.width as i64) / 2), ((h as i64 - small.height as i64) / 2));
        for y in 0..small.height {
            for x in 0..small.width {
                let (cx, cy) = (ox + x as i64, oy + y as i64);
                if cx >= 0 && cy >= 0 && (cx as u32) < w && (cy as u32) < h {
                    out.set_pixel(cx as u32, cy as u32, small.pixel(x, y));
                }
            }
        }
        return out;
    }
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = ((x as f64 + 0.5 - ox) / scale, (y as f64 + 0.5 - oy) / scale);
            if sx < 0.0 || sy < 0.0 || sx >= iw || sy >= ih {
                continue;
            }
            // sample inside the image so its edges do not fade into the transparent outside
            let (cx, cy) = (sx.clamp(0.5, iw - 0.5), sy.clamp(0.5, ih - 0.5));
            out.set_pixel(x, y, if scale == 1.0 { img.pixel(sx as u32, sy as u32) } else { tooncraft_raster::select::sample(img, cx as f32, cy as f32) });
        }
    }
    out
}

pub(crate) fn images(e: &mut Engine, paths: &[String], frame: u32, hold: u32, name: &str, o: &ImportOptions) -> Result<()> {
    if paths.len() > MAX_IMPORT {
        return Err(DocError::TooManyDrawings.into());
    }
    if frame >= e.project.length() {
        return Err(DocError::NoFrame(frame).into());
    }
    if let Some(l) = o.into_layer {
        e.check_unlocked(l)?;
    }
    let (w, h) = (e.project.width(), e.project.height());
    // groups of files: one per name, or all of them
    let mut groups: Vec<(String, Vec<&String>)> = Vec::new();
    for p in paths {
        let key = if o.by_name && o.into_layer.is_none() { sequence_name(p) } else { name.to_string() };
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, v)) => v.push(p),
            None => groups.push((key, vec![p])),
        }
    }
    if groups.len() > MAX_IMPORT_LAYERS || e.project.layers.len() + groups.len() > tooncraft_doc::MAX_LAYERS {
        return Err(DocError::TooManyLayers.into());
    }
    // Decode everything first: a bad file changes nothing.
    let mut decoded: Vec<(String, Vec<Surface>)> = Vec::with_capacity(groups.len());
    for (key, files) in &groups {
        let mut surfaces = Vec::with_capacity(files.len());
        for p in files {
            let img = decode(p, w.max(h), o.premultiplied).map_err(|reason| EngineError::Import(ImportFailure { file: (*p).clone(), reason }.to_string()))?;
            let mut s = Surface::new(w, h)?;
            s.load_image(&place(&img, w, h, o.placement))?;
            surfaces.push(s);
        }
        decoded.push((key.clone(), surfaces));
    }
    let mut parts = vec![Part::Layers(e.project.layers.clone())];
    let len = e.project.length();
    let hold = hold.max(1);
    for (key, surfaces) in decoded {
        let index = match o.into_layer {
            Some(l) => l,
            None => e.project.add_layer(&key, e.project.layers.len())?,
        };
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
    }
    e.push(Entry { label: "Import images".into(), parts });
    Ok(())
}
