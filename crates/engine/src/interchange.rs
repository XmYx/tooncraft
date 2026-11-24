//! Imports from other animation and paint apps through `tooncraft-interop`, as undoable commands:
//! layered PSD / PSB and OpenRaster files, animated GIFs, and XDTS exposure sheets with their
//! drawing folders. Every import decodes fully first, so a bad file changes nothing.

use tooncraft_doc::{Cell, DocError};
use tooncraft_raster::{RgbaImage, Surface};

use crate::undo::{Entry, Part};
use crate::{Engine, EngineError, Result};

fn err(e: impl std::fmt::Display) -> EngineError {
    EngineError::Import(e.to_string())
}

/// Centre `img` on the canvas, scaled down to fit (same rule as image import).
fn fit(img: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    if img.width == w && img.height == h {
        return img.clone();
    }
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

fn surface(e: &Engine, img: &RgbaImage) -> Result<Surface> {
    let (w, h) = (e.project.width(), e.project.height());
    let mut s = Surface::new(w, h)?;
    s.load_image(&fit(img, w, h))?;
    Ok(s)
}

fn read(path: &str) -> Result<Vec<u8>> {
    let meta = std::fs::metadata(path).map_err(|e| err(format!("{path}: {e}")))?;
    if meta.len() > tooncraft_interop::MAX_IMPORT_FILE {
        return Err(err(format!("{path}: the file is too large")));
    }
    std::fs::read(path).map_err(|e| err(format!("{path}: {e}")))
}

/// Layers of a PSD / PSB or OpenRaster file, each a new layer with a drawing at `frame`.
pub(crate) fn layers(e: &mut Engine, path: &str, frame: u32) -> Result<()> {
    if frame >= e.project.length() {
        return Err(DocError::NoFrame(frame).into());
    }
    let bytes = read(path)?;
    let lower = path.to_ascii_lowercase();
    let imported = if lower.ends_with(".ora") {
        tooncraft_interop::ora::import(&bytes).map_err(err)?
    } else {
        tooncraft_interop::psd::import(&bytes, tooncraft_raster::MAX_SIDE).map_err(err)?
    };
    let mut surfaces = Vec::with_capacity(imported.len());
    for l in &imported {
        surfaces.push(surface(e, &l.image)?);
    }
    let mut parts = vec![Part::Layers(e.project.layers.clone())];
    for (l, s) in imported.iter().zip(surfaces) {
        let index = e.project.add_layer(&l.name, e.project.layers.len())?;
        let id = e.project.add_drawing(Some(s))?;
        parts.insert(0, Part::Drawing { id, surface: None });
        let layer = e.project.layer_mut(index)?;
        layer.visible = l.visible;
        layer.opacity = l.opacity.clamp(0.0, 1.0);
        layer.cells[frame as usize] = Cell::Key(id);
    }
    e.push(Entry { label: "Import layers".into(), parts });
    Ok(())
}

/// An animated GIF as a new layer from `frame`, each GIF frame held for its delay at the shot's
/// frame rate (at least one frame).
pub(crate) fn gif(e: &mut Engine, path: &str, frame: u32) -> Result<()> {
    if frame >= e.project.length() {
        return Err(DocError::NoFrame(frame).into());
    }
    let (frames, delays) = tooncraft_interop::gif::import(&read(path)?).map_err(err)?;
    if frames.len() > crate::import::MAX_IMPORT {
        return Err(DocError::TooManyDrawings.into());
    }
    let mut surfaces = Vec::with_capacity(frames.len());
    for f in &frames {
        surfaces.push(surface(e, f)?);
    }
    let fps = e.project.fps() as f64;
    let name = std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "GIF".into());
    let mut parts = vec![Part::Layers(e.project.layers.clone())];
    let index = e.project.add_layer(&name, e.project.layers.len())?;
    let len = e.project.length();
    let mut f = frame;
    let mut clock = 0.0f64;
    for (i, s) in surfaces.into_iter().enumerate() {
        if f >= len {
            break;
        }
        let id = e.project.add_drawing(Some(s))?;
        parts.insert(0, Part::Drawing { id, surface: None });
        clock += delays.get(i).copied().unwrap_or(10).max(1) as f64 / 100.0;
        let end = (frame as f64 + (clock * fps).round()).max(f as f64 + 1.0) as u32;
        let layer = e.project.layer_mut(index)?;
        layer.cells[f as usize] = Cell::Key(id);
        for k in f + 1..end.min(len) {
            layer.cells[k as usize] = Cell::Hold;
        }
        f = end;
    }
    e.push(Entry { label: "Import GIF".into(), parts });
    Ok(())
}

/// An XDTS exposure sheet and the drawing folders next to it: one new layer per track, timed as
/// the sheet says, from frame 0. The shot grows to the sheet's length if it is shorter.
pub(crate) fn sheet(e: &mut Engine, path: &str) -> Result<()> {
    let imported = tooncraft_interop::levels::import(std::path::Path::new(path)).map_err(err)?;
    // decode every drawing first
    let mut tracks = Vec::new();
    for t in &imported.tracks {
        let mut drawings = std::collections::BTreeMap::new();
        for (name, img) in &t.drawings {
            drawings.insert(name.clone(), surface(e, img)?);
        }
        tracks.push((t, drawings));
    }
    let mut parts = vec![Part::Length { length: e.project.length(), layers: e.project.layers.clone() }];
    if imported.duration > e.project.length() {
        e.project.set_length(imported.duration.min(tooncraft_doc::MAX_FRAMES))?;
    }
    let len = e.project.length();
    for (t, drawings) in tracks {
        let index = e.project.add_layer(&t.name, e.project.layers.len())?;
        let mut ids = std::collections::BTreeMap::new();
        for (name, s) in drawings {
            let id = e.project.add_drawing(Some(s))?;
            parts.insert(0, Part::Drawing { id, surface: None });
            ids.insert(name, id);
        }
        let layer = e.project.layer_mut(index)?;
        let mut prev: Option<&String> = None;
        for (f, v) in t.cells.iter().enumerate().take(len as usize) {
            layer.cells[f] = match (v, ids.get(v.as_deref().unwrap_or_default())) {
                (Some(v), Some(id)) if prev != Some(v) => Cell::Key(*id),
                (Some(_), Some(_)) => Cell::Hold,
                _ => Cell::Blank,
            };
            prev = v.as_ref().filter(|v| ids.contains_key(v.as_str()));
        }
    }
    e.push(Entry { label: "Import exposure sheet".into(), parts });
    Ok(())
}

/// The choices of a video import ([`crate::Command::ImportVideo`]).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct VideoImport {
    /// Seconds into the video of the first picture.
    pub from_seconds: f64,
    /// Every `every`-th picture (1 = every picture; 2 = on twos, and so on).
    pub every: u32,
    /// At most this many pictures (0 = as many as fit).
    pub max_frames: u32,
    /// Put the layer at the bottom, locked and at half opacity, to draw over (rotoscoping).
    pub reference: bool,
    /// Lengthen the shot so the whole video fits (otherwise it stops at the shot's end).
    pub grow: bool,
}

impl Default for VideoImport {
    fn default() -> Self {
        VideoImport { from_seconds: 0.0, every: 1, max_frames: 0, reference: true, grow: true }
    }
}

/// Most bytes of video pictures one import keeps on the canvas.
pub const MAX_VIDEO_BYTES: u64 = tooncraft_interop::MAX_IMPORT_BYTES;

/// How many video pictures one import keeps on a `w`×`h` canvas.
pub(crate) fn video_fit(w: u32, h: u32) -> usize {
    let per = w as u64 * h as u64 * 4;
    (MAX_VIDEO_BYTES / per.max(1)).min(crate::import::MAX_IMPORT as u64) as usize
}

/// A video file as a new layer from `frame` (see [`crate::Command::ImportVideo`]). Decodes every
/// chosen picture first, so a bad file changes nothing.
pub(crate) fn video(e: &mut Engine, path: &str, frame: u32, o: &VideoImport) -> Result<()> {
    if frame >= e.project.length() {
        return Err(DocError::NoFrame(frame).into());
    }
    let file = std::fs::File::open(path).map_err(|x| err(format!("{path}: {x}")))?;
    let (w, h) = (e.project.width(), e.project.height());
    // the work and memory cap, checked before each picture is kept
    let fit = video_fit(w, h);
    let limit = if o.max_frames == 0 { fit } else { (o.max_frames as usize).min(fit) };
    let room = if o.grow { tooncraft_doc::MAX_FRAMES } else { e.project.length() };
    let fps = e.project.fps().max(1) as f64;
    let pick = tooncraft_interop::video::Pick { from: o.from_seconds, every: o.every, max: limit.saturating_add(1) };
    let start = pick.sanitized().from;
    let mut kept: Vec<(u32, Surface)> = Vec::new();
    let mut over = false;
    let mut failed: Option<EngineError> = None;
    let info = tooncraft_interop::video::frames(&file, pick, |img, t| {
        let at = frame as f64 + ((t - start).max(0.0) * fps).round();
        if at >= room as f64 {
            return Ok(false);
        }
        let at = at as u32;
        // two pictures on one shot frame: the first one stays
        if kept.last().is_some_and(|(f, _)| *f >= at) {
            return Ok(true);
        }
        if kept.len() >= limit {
            over = true;
            return Ok(false);
        }
        match surface(e, &img) {
            Ok(s) => kept.push((at, s)),
            Err(x) => {
                failed = Some(x);
                return Ok(false);
            }
        }
        Ok(true)
    })
    .map_err(|x| err(format!("{path}: {x}")))?;
    if let Some(x) = failed {
        return Err(x);
    }
    if over && (o.max_frames == 0 || o.max_frames as usize > limit) {
        return Err(err(format!("{path}: more than {limit} pictures do not fit in one import at this canvas size; import a part (a start, a step or a cap)")));
    }
    let Some(last) = kept.last().map(|(f, _)| *f) else {
        return Err(err(format!("{path}: no pictures from {start:.2} s on (the video is {:.2} s long)", info.seconds)));
    };
    let parts = vec![Part::Whole(Box::new(e.project.clone()))];
    // the last picture is held for its own length
    let tail = if info.fps > 0.0 { (fps * o.every.max(1) as f64 / info.fps).round().max(1.0) as u32 } else { 1 };
    let end = last.saturating_add(tail).min(room);
    if o.grow && e.project.length() < end {
        e.project.set_length(end)?;
    }
    let name: String = std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Video".into());
    let name: String = name.chars().take(tooncraft_doc::MAX_NAME).collect();
    let index = if o.reference { 0 } else { e.project.layers.len() };
    let li = e.project.add_layer(&name, index)?;
    let len = e.project.length();
    let starts: Vec<u32> = kept.iter().map(|(f, _)| *f).collect();
    for (i, (at, s)) in kept.into_iter().enumerate() {
        let id = e.project.add_drawing(Some(s))?;
        let next = starts.get(i + 1).copied().unwrap_or(end).min(len);
        let layer = e.project.layer_mut(li)?;
        if let Some(c) = layer.cells.get_mut(at as usize) {
            *c = Cell::Key(id);
        }
        for k in at + 1..next {
            if let Some(c) = layer.cells.get_mut(k as usize) {
                *c = Cell::Hold;
            }
        }
    }
    let layer = e.project.layer_mut(li)?;
    if o.reference {
        layer.locked = true;
        layer.opacity = 0.5;
    }
    e.push(Entry { label: "Import video".into(), parts });
    Ok(())
}
