//! ToonCraft's document model (layer 1, UI-free).
//!
//! A [`Project`] is a canvas size, a frame rate, a stack of [`Layer`]s and a library of
//! drawings. Each layer has an exposure sheet: one [`Cell`] per frame, which is the start of a
//! drawing ([`Cell::Key`]), a continuation of the drawing above it ([`Cell::Hold`]) or nothing
//! ([`Cell::Blank`]). Drawings live in the library and are referenced by id, so re-timing only
//! edits cells and never loses a drawing: a drawing no cell shows any more stays in the library
//! until it is purged (an undoable command).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

pub mod camera;
mod exposure;
mod onion;
pub mod render;

pub use camera::{Camera, CameraKey};
pub use exposure::{Cell, ExposureError};
pub use onion::{LightTable, OnionMode, OnionSkin, Pin};
pub use render::{RenderOptions, render_camera, render_frame, render_region};

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
pub use tooncraft_raster::BlendMode;
use tooncraft_raster::{MAX_SIDE, RasterError, Surface};

/// Most frames in one project (about 46 minutes at 24 fps).
pub const MAX_FRAMES: u32 = 65_536;
/// Most layers in one project.
pub const MAX_LAYERS: usize = 512;
/// Most drawings in one project's library.
pub const MAX_DRAWINGS: usize = 100_000;
/// Supported frame rates.
pub const MIN_FPS: u32 = 1;
pub const MAX_FPS: u32 = 120;

/// A drawing's id in the project library.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DrawingId(pub u64);

/// A layer's stable id (survives reordering).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LayerId(pub u64);

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum DocError {
    #[error(transparent)]
    Raster(#[from] RasterError),
    #[error(transparent)]
    Exposure(#[from] ExposureError),
    #[error("there is no layer {0}")]
    NoLayer(usize),
    #[error("frame {0} is past the end of the shot")]
    NoFrame(u32),
    #[error("the shot can have at most {MAX_FRAMES} frames")]
    TooLong,
    #[error("a project can have at most {MAX_LAYERS} layers")]
    TooManyLayers,
    #[error("a project can have at most {MAX_DRAWINGS} drawings")]
    TooManyDrawings,
    #[error("frame rate {0} is outside {MIN_FPS}…{MAX_FPS}")]
    BadFps(u32),
    #[error("layer {0} is locked")]
    Locked(String),
    #[error("the project must keep at least one layer")]
    LastLayer,
}

pub type Result<T> = std::result::Result<T, DocError>;

/// One animation layer and its exposure sheet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default = "one")]
    pub opacity: f32,
    #[serde(default, skip_serializing_if = "is_normal")]
    pub blend: BlendMode,
    /// Label colour (straight RGB) for the timeline and layer list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
    /// Name of the folder the layer is in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// One cell per frame of the shot.
    pub cells: Vec<Cell>,
}

fn is_normal(b: &BlendMode) -> bool {
    *b == BlendMode::Normal
}

fn yes() -> bool {
    true
}
fn one() -> f32 {
    1.0
}

impl Layer {
    /// The drawing exposed at `frame`, following holds back to their key.
    pub fn drawing_at(&self, frame: u32) -> Option<DrawingId> {
        exposure::resolve(&self.cells, frame)
    }

    /// First frame of the exposure that contains `frame` (its key), if any.
    pub fn exposure_start(&self, frame: u32) -> Option<u32> {
        exposure::key_frame(&self.cells, frame)
    }

    /// Frames `[start, end)` of the exposure containing `frame`.
    pub fn exposure_range(&self, frame: u32) -> Option<(u32, u32)> {
        let start = self.exposure_start(frame)?;
        let mut end = start + 1;
        while matches!(self.cells.get(end as usize), Some(Cell::Hold)) {
            end += 1;
        }
        Some((start, end))
    }
}

/// A folder of layers: its members are the layers whose `folder` names it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Folder {
    pub name: String,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default = "one")]
    pub opacity: f32,
    /// Its layers are hidden in the timeline and layer list (not in the picture).
    #[serde(default)]
    pub collapsed: bool,
}

/// Paper (background) settings.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Paper {
    /// Straight RGB.
    pub color: [u8; 3],
    /// Show the paper in exports (off: transparent background).
    pub export: bool,
}

impl Default for Paper {
    fn default() -> Self {
        Self { color: [246, 243, 236], export: true }
    }
}

/// The shot's sound: the original file's bytes (any format craft-audio decodes) and where it
/// starts. Kept as the user's file so nothing is re-encoded until export.
#[derive(Clone, Debug, PartialEq)]
pub struct Soundtrack {
    /// File name it was imported from (shown in the timeline; its extension hints the decoder).
    pub name: String,
    /// Frame at which the sound starts (negative: the sound starts before the shot).
    pub offset: i32,
    pub bytes: std::sync::Arc<Vec<u8>>,
}

/// A shot: canvas, timing, layers and the drawing library.
///
/// Equality compares what the project *is* (content and settings), not the id counters: ids are
/// never reused, so a project after "draw, undo" equals the one before it.
#[derive(Clone, Debug)]
pub struct Project {
    pub name: String,
    width: u32,
    height: u32,
    fps: u32,
    length: u32,
    pub paper: Paper,
    pub layers: Vec<Layer>,
    drawings: BTreeMap<DrawingId, Surface>,
    pub onion: OnionSkin,
    pub light_table: LightTable,
    pub soundtrack: Option<Soundtrack>,
    pub camera: Camera,
    pub folders: Vec<Folder>,
    /// Image brush tips used by this shot's brushes, by id.
    pub tips: BTreeMap<u64, std::sync::Arc<tooncraft_brush::Tip>>,
    next_drawing: u64,
    next_layer: u64,
}

impl PartialEq for Project {
    fn eq(&self, o: &Self) -> bool {
        self.name == o.name
            && (self.width, self.height, self.fps, self.length) == (o.width, o.height, o.fps, o.length)
            && self.paper == o.paper
            && self.layers == o.layers
            && self.drawings == o.drawings
            && self.onion == o.onion
            && self.light_table == o.light_table
            && self.soundtrack == o.soundtrack
            && self.camera == o.camera
            && self.folders == o.folders
            && self.tips == o.tips
    }
}

impl Project {
    /// A new shot with one empty layer.
    pub fn new(name: &str, width: u32, height: u32, fps: u32, length: u32) -> Result<Self> {
        if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
            return Err(RasterError::BadSize(width, height).into());
        }
        if !(MIN_FPS..=MAX_FPS).contains(&fps) {
            return Err(DocError::BadFps(fps));
        }
        if length == 0 || length > MAX_FRAMES {
            return Err(DocError::TooLong);
        }
        let mut p = Project {
            name: name.to_string(),
            width,
            height,
            fps,
            length,
            paper: Paper::default(),
            layers: Vec::new(),
            drawings: BTreeMap::new(),
            onion: OnionSkin::default(),
            light_table: LightTable::default(),
            soundtrack: None,
            camera: Camera::default(),
            folders: Vec::new(),
            tips: BTreeMap::new(),
            next_drawing: 1,
            next_layer: 1,
        };
        p.add_layer("Layer 1", 0)?;
        Ok(p)
    }

    /// A new 1920×1080, 24 fps, 48-frame shot with one empty layer (cannot fail).
    pub fn blank(name: &str) -> Self {
        let mut p = Project {
            name: name.to_string(),
            width: 1920,
            height: 1080,
            fps: 24,
            length: 48,
            paper: Paper::default(),
            layers: Vec::new(),
            drawings: BTreeMap::new(),
            onion: OnionSkin::default(),
            light_table: LightTable::default(),
            soundtrack: None,
            camera: Camera::default(),
            folders: Vec::new(),
            tips: BTreeMap::new(),
            next_drawing: 1,
            next_layer: 1,
        };
        let _ = p.add_layer("Layer 1", 0);
        p
    }

    /// Rebuild a project from its parts (file loading). Everything is validated and repaired:
    /// cells are padded or cut to `length`, keys to missing drawings become blanks, ids are
    /// renumbered past the largest one in use.
    pub fn from_parts(
        name: String,
        size: (u32, u32),
        fps: u32,
        length: u32,
        paper: Paper,
        layers: Vec<Layer>,
        drawings: BTreeMap<DrawingId, Surface>,
        onion: OnionSkin,
        light_table: LightTable,
    ) -> Result<Self> {
        let mut p = Project::new(&name, size.0, size.1, fps.clamp(MIN_FPS, MAX_FPS), length.clamp(1, MAX_FRAMES))?;
        if layers.len() > MAX_LAYERS {
            return Err(DocError::TooManyLayers);
        }
        if drawings.len() > MAX_DRAWINGS {
            return Err(DocError::TooManyDrawings);
        }
        for d in drawings.values() {
            if d.width() != p.width || d.height() != p.height {
                return Err(RasterError::BadSize(d.width(), d.height()).into());
            }
        }
        p.paper = paper;
        p.drawings = drawings;
        p.onion = onion.sanitized();
        p.light_table = light_table.sanitized();
        if !layers.is_empty() {
            p.layers = layers;
        }
        let mut seen = std::collections::BTreeSet::new();
        for layer in &mut p.layers {
            layer.cells.resize(p.length as usize, Cell::Blank);
            exposure::repair(&mut layer.cells, |id| p.drawings.contains_key(&id));
            if !layer.opacity.is_finite() {
                layer.opacity = 1.0;
            }
            layer.opacity = layer.opacity.clamp(0.0, 1.0);
            layer.name = layer.name.chars().take(200).collect();
            if !seen.insert(layer.id) {
                layer.id = LayerId(u64::MAX); // fixed below
            }
        }
        let max_layer = p.layers.iter().map(|l| l.id.0).filter(|i| *i != u64::MAX).max().unwrap_or(0);
        let mut next = max_layer.saturating_add(1);
        for layer in &mut p.layers {
            if layer.id.0 == u64::MAX {
                layer.id = LayerId(next);
                next = next.saturating_add(1);
            }
        }
        p.next_layer = next;
        p.next_drawing = p.drawings.keys().next_back().map(|d| d.0.saturating_add(1)).unwrap_or(1);
        let ids: Vec<LayerId> = p.layers.iter().map(|l| l.id).collect();
        p.light_table.pins.retain(|pin| ids.contains(&pin.layer) && pin.frame < p.length);
        Ok(p)
    }

    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn fps(&self) -> u32 {
        self.fps
    }
    /// Frames in the shot.
    pub fn length(&self) -> u32 {
        self.length
    }

    pub fn set_fps(&mut self, fps: u32) -> Result<()> {
        if !(MIN_FPS..=MAX_FPS).contains(&fps) {
            return Err(DocError::BadFps(fps));
        }
        self.fps = fps;
        Ok(())
    }

    /// Change the shot length; cells past the end are cut, new frames are blank (or holds of the
    /// last exposure, which is what animators expect when extending a shot).
    pub fn set_length(&mut self, length: u32) -> Result<()> {
        if length == 0 || length > MAX_FRAMES {
            return Err(DocError::TooLong);
        }
        for layer in &mut self.layers {
            let fill = if layer.cells.last().is_some_and(|c| !matches!(c, Cell::Blank)) { Cell::Hold } else { Cell::Blank };
            layer.cells.resize(length as usize, fill);
        }
        self.length = length;
        self.light_table.pins.retain(|p| p.frame < length);
        Ok(())
    }

    pub fn layer(&self, index: usize) -> Result<&Layer> {
        self.layers.get(index).ok_or(DocError::NoLayer(index))
    }

    pub fn layer_mut(&mut self, index: usize) -> Result<&mut Layer> {
        self.layers.get_mut(index).ok_or(DocError::NoLayer(index))
    }

    pub fn layer_index(&self, id: LayerId) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    /// Add an empty layer at `index` (0 is the bottom of the stack). Returns its index.
    pub fn add_layer(&mut self, name: &str, index: usize) -> Result<usize> {
        if self.layers.len() >= MAX_LAYERS {
            return Err(DocError::TooManyLayers);
        }
        let id = LayerId(self.next_layer);
        self.next_layer += 1;
        let index = index.min(self.layers.len());
        self.layers.insert(
            index,
            Layer {
                id,
                name: name.chars().take(200).collect(),
                visible: true,
                locked: false,
                opacity: 1.0,
                blend: Default::default(),
                color: None,
                folder: None,
                cells: vec![Cell::Blank; self.length as usize],
            },
        );
        Ok(index)
    }

    pub fn delete_layer(&mut self, index: usize) -> Result<Layer> {
        if index >= self.layers.len() {
            return Err(DocError::NoLayer(index));
        }
        if self.layers.len() == 1 {
            return Err(DocError::LastLayer);
        }
        let l = self.layers.remove(index);
        self.light_table.pins.retain(|p| p.layer != l.id);
        Ok(l)
    }

    pub fn move_layer(&mut self, from: usize, to: usize) -> Result<()> {
        if from >= self.layers.len() {
            return Err(DocError::NoLayer(from));
        }
        let l = self.layers.remove(from);
        let to = to.min(self.layers.len());
        self.layers.insert(to, l);
        Ok(())
    }

    pub fn drawing(&self, id: DrawingId) -> Option<&Surface> {
        self.drawings.get(&id)
    }

    pub fn drawing_mut(&mut self, id: DrawingId) -> Option<&mut Surface> {
        self.drawings.get_mut(&id)
    }

    pub fn drawings(&self) -> impl Iterator<Item = (DrawingId, &Surface)> {
        self.drawings.iter().map(|(k, v)| (*k, v))
    }

    pub fn drawing_count(&self) -> usize {
        self.drawings.len()
    }

    /// Add a drawing to the library (empty when `surface` is `None`).
    pub fn add_drawing(&mut self, surface: Option<Surface>) -> Result<DrawingId> {
        if self.drawings.len() >= MAX_DRAWINGS {
            return Err(DocError::TooManyDrawings);
        }
        let s = match surface {
            Some(s) if s.width() == self.width && s.height() == self.height => s,
            Some(s) => return Err(RasterError::BadSize(s.width(), s.height()).into()),
            None => Surface::new(self.width, self.height)?,
        };
        let id = DrawingId(self.next_drawing);
        self.next_drawing += 1;
        self.drawings.insert(id, s);
        Ok(id)
    }

    /// Put a drawing back or remove it (undo/redo). Returns what was there.
    pub fn swap_drawing(&mut self, id: DrawingId, surface: Option<Surface>) -> Option<Surface> {
        match surface {
            Some(s) => {
                self.next_drawing = self.next_drawing.max(id.0.saturating_add(1));
                self.drawings.insert(id, s)
            }
            None => self.drawings.remove(&id),
        }
    }

    /// The drawing shown on a layer at a frame.
    pub fn drawing_at(&self, layer: usize, frame: u32) -> Option<DrawingId> {
        self.layers.get(layer)?.drawing_at(frame)
    }

    /// Drawings no cell refers to.
    pub fn unused_drawings(&self) -> Vec<DrawingId> {
        let used: std::collections::BTreeSet<DrawingId> = self.layers.iter().flat_map(|l| l.cells.iter().filter_map(|c| c.key())).collect();
        self.drawings.keys().filter(|d| !used.contains(d)).copied().collect()
    }

    /// The folder a layer is in.
    pub fn folder_of(&self, layer: &Layer) -> Option<&Folder> {
        layer.folder.as_ref().and_then(|n| self.folders.iter().find(|f| &f.name == n))
    }

    /// Visible after its folder's visibility, and its opacity times the folder's.
    pub fn effective(&self, layer: &Layer) -> (bool, f32) {
        match self.folder_of(layer) {
            Some(f) => (layer.visible && f.visible, layer.opacity * f.opacity.clamp(0.0, 1.0)),
            None => (layer.visible, layer.opacity),
        }
    }

    /// Bytes of pixel data held by the library (shared tiles counted in full).
    pub fn memory_bytes(&self) -> u64 {
        self.drawings.values().map(Surface::memory_bytes).sum()
    }

    /// Onion-skin drawings around `frame` on a layer: `(drawing, offset)` with negative offsets
    /// before and positive after, nearest first, never the current drawing.
    pub fn onion_drawings(&self, layer: usize, frame: u32) -> Vec<(DrawingId, i32)> {
        let Some(l) = self.layers.get(layer) else { return Vec::new() };
        onion::neighbours(l, frame, self.length, &self.onion)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_project_and_limits() {
        let p = Project::new("Shot", 1920, 1080, 24, 48).unwrap();
        assert_eq!(p.layers.len(), 1);
        assert_eq!(p.layers[0].cells.len(), 48);
        assert!(Project::new("x", 0, 10, 24, 1).is_err());
        assert!(Project::new("x", 10, 10, 0, 1).is_err());
        assert!(Project::new("x", 10, 10, 24, MAX_FRAMES + 1).is_err());
    }

    #[test]
    fn layers_add_move_delete() {
        let mut p = Project::new("Shot", 64, 64, 24, 10).unwrap();
        let i = p.add_layer("Ink", 1).unwrap();
        p.add_layer("Paint", 0).unwrap();
        assert_eq!(p.layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["Paint", "Layer 1", "Ink"]);
        assert_eq!(i, 1);
        p.move_layer(0, 2).unwrap();
        assert_eq!(p.layers[2].name, "Paint");
        p.delete_layer(0).unwrap();
        p.delete_layer(0).unwrap();
        assert_eq!(p.delete_layer(0), Err(DocError::LastLayer));
    }

    #[test]
    fn set_length_extends_holds() {
        let mut p = Project::new("Shot", 64, 64, 24, 3).unwrap();
        let d = p.add_drawing(None).unwrap();
        p.layers[0].cells[2] = Cell::Key(d);
        p.set_length(6).unwrap();
        assert_eq!(p.drawing_at(0, 5), Some(d));
        p.set_length(2).unwrap();
        assert_eq!(p.layers[0].cells.len(), 2);
        assert_eq!(p.unused_drawings(), vec![d]);
    }

    #[test]
    fn from_parts_repairs() {
        let p = Project::new("Shot", 64, 64, 24, 4).unwrap();
        let mut layers = p.layers.clone();
        layers[0].cells = vec![Cell::Hold, Cell::Key(DrawingId(99)), Cell::Hold];
        layers[0].opacity = f32::NAN;
        let mut dup = layers[0].clone();
        dup.name = "dup".into();
        layers.push(dup);
        let q =
            Project::from_parts("x".into(), (64, 64), 500, 4, Paper::default(), layers, BTreeMap::new(), OnionSkin::default(), LightTable::default()).unwrap();
        assert_eq!(q.fps(), MAX_FPS);
        assert_eq!(q.layers[0].cells, vec![Cell::Blank; 4]);
        assert_eq!(q.layers[0].opacity, 1.0);
        assert_ne!(q.layers[0].id, q.layers[1].id);
    }
}
