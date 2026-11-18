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
pub mod guides;
pub mod markers;
mod onion;
pub mod paging;
pub mod peg;
pub mod render;
pub mod sound;
pub mod timing;

pub use camera::{Camera, CameraKey, Shake};
pub use exposure::{Cell, ExposureError};
pub use markers::{MAX_MARKER_TEXT, MAX_MARKERS, MAX_NAME, parse_timecode, timecode};
pub use onion::{LightTable, OnionMode, OnionSkin, Pin};
pub use peg::{Ease, Peg, PegId, PegKey};
pub use render::{RenderOptions, render_camera, render_frame, render_region};
pub use sound::{AudioTrack, Soundtrack, VolumeKey};
pub use timing::{Mark, TimingPreset};
pub use tooncraft_raster::select::Affine;

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
    #[error("a shot can have at most {} markers", markers::MAX_MARKERS)]
    TooManyMarkers,
    #[error("there is no drawing {0}")]
    NoDrawingId(u64),
    #[error("another drawing is already called “{0}”")]
    NameInUse(String),
    #[error("no drawing is called “{0}”")]
    NoDrawingNamed(String),
    #[error("frame rate {0} is outside {MIN_FPS}…{MAX_FPS}")]
    BadFps(u32),
    #[error("layer {0} is locked")]
    Locked(String),
    #[error("the project must keep at least one layer")]
    LastLayer,
    #[error("a project can have at most {} pegs", peg::MAX_PEGS)]
    TooManyPegs,
    #[error("there is no peg {0}")]
    NoPeg(u64),
    #[error("a drawing could not be read back from the page file")]
    PageRead,
    #[error("a drawing could not be written to the page file (is the disk full?)")]
    PageWrite,
    #[error("a peg cannot hang from itself or from a peg below it")]
    PegLoop,
    #[error("there is no folder “{0}”")]
    NoFolder(String),
    #[error("a folder cannot go inside itself or a folder inside it, or nest deeper than {MAX_FOLDER_DEPTH}")]
    FolderLoop,
    #[error("“{0}” cannot be used as a folder name (empty or taken)")]
    FolderName(String),
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
    /// The peg the layer hangs from (its drawings move with the peg).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peg: Option<PegId>,
    /// Distance from the drawing plane for the multiplane camera (negative: nearer the camera).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub depth: f32,
    /// The layer's effect stack, run top to bottom over its drawing before it is composited.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<tooncraft_fx::Effect>,
    /// Opacity keys, sorted by frame: with keys the opacity follows them (linear between keys,
    /// held outside) and `opacity` is unused.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub opacity_keys: Vec<OpacityKey>,
    /// A matte layer is not drawn: it clips the layers right below it (same folder, down to the
    /// next matte) to its own shape, or to everything outside it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matte: Option<Matte>,
    /// The layer's mouth chart for lip sync: which drawing shows each mouth.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub mouth_chart: BTreeMap<tooncraft_lipsync::Mouth, DrawingId>,
    /// Clipped to the layer below: shows only where that layer (the base of its clipping group)
    /// has paint.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clip: bool,
    /// Transparency is locked: strokes only recolour paint that is already there.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lock_alpha: bool,
    /// One cell per frame of the shot.
    pub cells: Vec<Cell>,
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

/// A layer's opacity from a frame on.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpacityKey {
    pub frame: u32,
    pub opacity: f32,
}

/// What a matte layer keeps of the layers it clips.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Matte {
    /// Where the matte is drawn.
    Inside,
    /// Where the matte is not drawn.
    Outside,
}

/// Most opacity keys on one layer.
pub const MAX_OPACITY_KEYS: usize = 4096;

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

    /// The layer's own opacity at `frame` (its keys, or `opacity` without keys).
    pub fn opacity_at(&self, frame: u32) -> f32 {
        let k = &self.opacity_keys;
        let Some(first) = k.first() else { return self.opacity };
        if frame <= first.frame {
            return first.opacity;
        }
        let i = k.partition_point(|key| key.frame <= frame);
        match (k.get(i.wrapping_sub(1)), k.get(i)) {
            (Some(a), Some(b)) if b.frame > a.frame => a.opacity + (b.opacity - a.opacity) * (frame - a.frame) as f32 / (b.frame - a.frame) as f32,
            (Some(a), _) => a.opacity,
            _ => self.opacity,
        }
    }

    /// Set (or replace) an opacity key. False when the layer has too many keys.
    pub fn set_opacity_key(&mut self, frame: u32, opacity: f32) -> bool {
        let opacity = if opacity.is_finite() { opacity.clamp(0.0, 1.0) } else { 1.0 };
        match self.opacity_keys.binary_search_by_key(&frame, |k| k.frame) {
            Ok(i) => {
                if let Some(k) = self.opacity_keys.get_mut(i) {
                    k.opacity = opacity;
                }
            }
            Err(i) => {
                if self.opacity_keys.len() >= MAX_OPACITY_KEYS {
                    return false;
                }
                self.opacity_keys.insert(i, OpacityKey { frame, opacity });
            }
        }
        true
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
    /// Its layers (and the folders in it) cannot be drawn on or re-timed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub locked: bool,
    /// The folder it is in (folders nest).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Label colour (straight RGB).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
}

/// Deepest nesting of folders.
pub const MAX_FOLDER_DEPTH: usize = 16;
/// Most folders in a project.
pub const MAX_FOLDERS: usize = 512;

impl Folder {
    pub fn new(name: &str) -> Self {
        Self { name: name.chars().take(200).collect(), visible: true, opacity: 1.0, collapsed: false, locked: false, parent: None, color: None }
    }
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
    drawings: BTreeMap<DrawingId, paging::Slot>,
    pub onion: OnionSkin,
    pub light_table: LightTable,
    /// Sound clips, in the order they were added.
    pub sounds: Vec<Soundtrack>,
    /// Audio track switches by track index (tracks past the end have defaults).
    pub audio_tracks: Vec<AudioTrack>,
    pub camera: Camera,
    pub folders: Vec<Folder>,
    /// Image brush tips used by this shot's brushes, by id.
    pub tips: BTreeMap<u64, std::sync::Arc<tooncraft_brush::Tip>>,
    /// Pegs layers can hang from (see [`peg`]).
    pub pegs: Vec<Peg>,
    /// Key, breakdown and inbetween marks of drawings (see [`timing`]).
    pub marks: BTreeMap<DrawingId, Mark>,
    /// Notes on frames (0-based), see [`markers`].
    pub markers: BTreeMap<u32, String>,
    /// Names of drawings, to expose them again by name (see [`markers`]).
    pub drawing_names: BTreeMap<DrawingId, String>,
    next_drawing: u64,
    next_layer: u64,
}

impl PartialEq for Project {
    fn eq(&self, o: &Self) -> bool {
        self.name == o.name
            && (self.width, self.height, self.fps, self.length) == (o.width, o.height, o.fps, o.length)
            && self.paper == o.paper
            && self.layers == o.layers
            && self.drawings.len() == o.drawings.len()
            && self.drawings.keys().all(|k| self.drawing(*k) == o.drawing(*k))
            && self.onion == o.onion
            && self.light_table == o.light_table
            && self.sounds == o.sounds
            && self.audio_tracks == o.audio_tracks
            && self.camera == o.camera
            && self.folders == o.folders
            && self.tips == o.tips
            && self.pegs == o.pegs
            && self.marks == o.marks
            && self.markers == o.markers
            && self.drawing_names == o.drawing_names
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
            sounds: Vec::new(),
            audio_tracks: Vec::new(),
            camera: Camera::default(),
            folders: Vec::new(),
            tips: BTreeMap::new(),
            pegs: Vec::new(),
            marks: BTreeMap::new(),
            markers: BTreeMap::new(),
            drawing_names: BTreeMap::new(),
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
            sounds: Vec::new(),
            audio_tracks: Vec::new(),
            camera: Camera::default(),
            folders: Vec::new(),
            tips: BTreeMap::new(),
            pegs: Vec::new(),
            marks: BTreeMap::new(),
            markers: BTreeMap::new(),
            drawing_names: BTreeMap::new(),
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
        p.drawings = drawings.into_iter().map(|(k, v)| (k, paging::Slot::Resident(v))).collect();
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
            layer.depth = if layer.depth.is_finite() { layer.depth.clamp(camera::MIN_DEPTH, camera::MAX_DEPTH) } else { 0.0 };
            layer.opacity = layer.opacity.clamp(0.0, 1.0);
            layer.name = layer.name.chars().take(200).collect();
            layer.effects = tooncraft_fx::sanitize_stack(std::mem::take(&mut layer.effects));
            let mut keys: Vec<OpacityKey> = std::mem::take(&mut layer.opacity_keys).into_iter().take(MAX_OPACITY_KEYS).collect();
            keys.sort_by_key(|k| k.frame);
            keys.dedup_by_key(|k| k.frame);
            for k in &mut keys {
                k.opacity = if k.opacity.is_finite() { k.opacity.clamp(0.0, 1.0) } else { 1.0 };
            }
            layer.opacity_keys = keys;
            layer.mouth_chart.retain(|_, d| p.drawings.contains_key(d));
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

    /// Put the shot on a `width`×`height` canvas with everything moved by `(dx, dy)` whole
    /// pixels (canvas resize and crop): drawings are moved, never resampled, and cut where they
    /// leave the canvas; peg pivots and camera keys move with them.
    pub fn reframe(&mut self, width: u32, height: u32, dx: i64, dy: i64) -> Result<()> {
        if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
            return Err(RasterError::BadSize(width, height).into());
        }
        let limit = 4 * MAX_SIDE as i64;
        if dx.abs() > limit || dy.abs() > limit {
            return Err(RasterError::BadSize(width, height).into());
        }
        let mut drawings = BTreeMap::new();
        for (id, slot) in &self.drawings {
            let Some(s) = slot.get(self.width, self.height) else { return Err(DocError::PageRead) };
            drawings.insert(*id, paging::Slot::Resident(s.reframed(width, height, dx, dy)?));
        }
        self.drawings = drawings;
        let (fx, fy) = (dx as f32, dy as f32);
        for p in &mut self.pegs {
            p.pivot = (p.pivot.0 + fx, p.pivot.1 + fy);
        }
        // the camera's neutral centre is the canvas centre: keep keyed framings where they were
        for k in &mut self.camera.keys {
            k.x += fx;
            k.y += fy;
        }
        self.width = width;
        self.height = height;
        self.camera = std::mem::take(&mut self.camera).sanitized(width, height);
        Ok(())
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
                peg: None,
                depth: 0.0,
                effects: Vec::new(),
                opacity_keys: Vec::new(),
                matte: None,
                mouth_chart: BTreeMap::new(),
                clip: false,
                lock_alpha: false,
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

    /// A drawing (a paged one is read back and cached until the next [`Project::page_out`]).
    pub fn drawing(&self, id: DrawingId) -> Option<&Surface> {
        self.drawings.get(&id)?.get(self.width, self.height)
    }

    /// A drawing to edit (a paged one becomes resident).
    pub fn drawing_mut(&mut self, id: DrawingId) -> Option<&mut Surface> {
        let (w, h) = (self.width, self.height);
        self.drawings.get_mut(&id)?.make_resident(w, h)
    }

    /// Every drawing (paged ones are read back and cached; prefer [`Project::drawing_ids`] with
    /// [`Project::drawing_owned`] to walk a long shot without loading it all).
    pub fn drawings(&self) -> impl Iterator<Item = (DrawingId, &Surface)> {
        self.drawings.iter().filter_map(|(k, v)| v.get(self.width, self.height).map(|s| (*k, s)))
    }

    pub fn drawing_ids(&self) -> impl Iterator<Item = DrawingId> + '_ {
        self.drawings.keys().copied()
    }

    /// A drawing by value without caching a paged one (tiles are shared, so a resident drawing
    /// costs a pointer copy). `Err` when a paged drawing cannot be read back.
    pub fn drawing_owned(&self, id: DrawingId) -> Result<Option<Surface>> {
        match self.drawings.get(&id) {
            None => Ok(None),
            Some(slot) => slot.clone().take(self.width, self.height).map(Some).ok_or(DocError::PageRead),
        }
    }

    /// Page drawings out to `file` until the drawings in memory take at most `budget` bytes,
    /// leaving those in `keep` (the frames around the playhead) in memory. Returns the bytes
    /// freed.
    pub fn page_out(&mut self, file: &std::sync::Arc<paging::PageFile>, keep: &std::collections::BTreeSet<DrawingId>, budget: u64) -> Result<u64> {
        let mut used = self.memory_bytes();
        let mut freed = 0;
        for (id, slot) in self.drawings.iter_mut() {
            if used <= budget {
                break;
            }
            if keep.contains(id) || slot.memory_bytes() == 0 {
                continue;
            }
            let f = slot.page_out(file).map_err(|_| DocError::PageWrite)?;
            used = used.saturating_sub(f);
            freed += f;
        }
        Ok(freed)
    }

    /// Drawings whose tiles live in a page file.
    pub fn paged_count(&self) -> usize {
        self.drawings.values().filter(|s| s.is_paged()).count()
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
        self.drawings.insert(id, paging::Slot::Resident(s));
        Ok(id)
    }

    /// Put a drawing back or remove it (undo/redo). Returns what was there.
    pub fn swap_drawing(&mut self, id: DrawingId, surface: Option<Surface>) -> Option<Surface> {
        match surface {
            Some(s) => {
                self.next_drawing = self.next_drawing.max(id.0.saturating_add(1));
                self.drawings.insert(id, paging::Slot::Resident(s)).and_then(|old| old.take(self.width, self.height))
            }
            None => self.drawings.remove(&id).and_then(|old| old.take(self.width, self.height)),
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
        layer.folder.as_ref().and_then(|n| self.folder(n))
    }

    pub fn folder(&self, name: &str) -> Option<&Folder> {
        self.folders.iter().find(|f| f.name == name)
    }

    /// A folder and the folders it is in, innermost first (stops at loops and
    /// [`MAX_FOLDER_DEPTH`]).
    pub fn folder_chain(&self, name: Option<&str>) -> Vec<&Folder> {
        let mut out: Vec<&Folder> = Vec::new();
        let mut next = name;
        while let Some(n) = next {
            if out.len() >= MAX_FOLDER_DEPTH || out.iter().any(|f| f.name == n) {
                break;
            }
            let Some(f) = self.folder(n) else { break };
            out.push(f);
            next = f.parent.as_deref();
        }
        out
    }

    /// Whether putting folder `name` in `parent` would make a loop or nest too deep.
    pub fn folder_would_loop(&self, name: &str, parent: &str) -> bool {
        let chain = self.folder_chain(Some(parent));
        chain.iter().any(|f| f.name == name) || chain.len() >= MAX_FOLDER_DEPTH
    }

    /// How deep a folder is nested (0: at the top).
    pub fn folder_depth(&self, name: &str) -> usize {
        self.folder_chain(Some(name)).len().saturating_sub(1)
    }

    /// Visible after its folders' visibility, and its opacity times theirs.
    pub fn effective(&self, layer: &Layer) -> (bool, f32) {
        let mut visible = layer.visible;
        let mut opacity = layer.opacity;
        for f in self.folder_chain(layer.folder.as_deref()) {
            visible &= f.visible;
            opacity *= f.opacity.clamp(0.0, 1.0);
        }
        (visible, opacity)
    }

    /// Locked itself or through one of its folders.
    pub fn is_locked(&self, layer: &Layer) -> bool {
        layer.locked || self.folder_chain(layer.folder.as_deref()).iter().any(|f| f.locked)
    }

    /// Hidden in the timeline and layer list because a folder it is in is collapsed.
    pub fn is_collapsed(&self, layer: &Layer) -> bool {
        self.folder_chain(layer.folder.as_deref()).iter().any(|f| f.collapsed)
    }

    /// Repair folders read from a file: unique names, bounded count, no loops, no missing
    /// parents; layers in missing folders come out.
    pub fn set_folders(&mut self, folders: Vec<Folder>) {
        let mut seen = std::collections::BTreeSet::new();
        let mut list: Vec<Folder> = folders.into_iter().filter(|f| seen.insert(f.name.clone())).take(MAX_FOLDERS).collect();
        for f in &mut list {
            f.opacity = if f.opacity.is_finite() { f.opacity.clamp(0.0, 1.0) } else { 1.0 };
            if f.parent.as_ref().is_some_and(|p| !seen.contains(p) || *p == f.name) {
                f.parent = None;
            }
        }
        self.folders = list;
        // break loops: each link is kept only if the chain above it does not lead back
        for i in 0..self.folders.len() {
            if let Some(p) = self.folders[i].parent.clone() {
                let name = self.folders[i].name.clone();
                self.folders[i].parent = None;
                if !self.folder_would_loop(&name, &p) {
                    self.folders[i].parent = Some(p);
                }
            }
        }
        let names: Vec<String> = self.folders.iter().map(|f| f.name.clone()).collect();
        for l in &mut self.layers {
            if l.folder.as_ref().is_some_and(|n| !names.contains(n)) {
                l.folder = None;
            }
        }
    }

    /// Drop folders that hold no layer, directly or through folders in them.
    pub fn prune_folders(&mut self) {
        let mut used = std::collections::BTreeSet::new();
        for l in &self.layers {
            for f in self.folder_chain(l.folder.as_deref()) {
                used.insert(f.name.clone());
            }
        }
        self.folders.retain(|f| used.contains(&f.name));
    }

    /// As [`Project::effective`], with the layer's opacity keys at `frame`.
    pub fn effective_at(&self, layer: &Layer, frame: u32) -> (bool, f32) {
        let o = layer.opacity_at(frame);
        match self.folder_of(layer) {
            Some(f) => (layer.visible && f.visible, o * f.opacity.clamp(0.0, 1.0)),
            None => (layer.visible, o),
        }
    }

    /// For every layer, the matte layer that clips it, if any: a matte clips the layers right
    /// below it in the same folder, down to the next matte layer.
    pub fn matte_owners(&self) -> Vec<Option<usize>> {
        let mut out = vec![None; self.layers.len()];
        for (m, ml) in self.layers.iter().enumerate() {
            if ml.matte.is_none() {
                continue;
            }
            for j in (0..m).rev() {
                let Some(l) = self.layers.get(j) else { break };
                if l.matte.is_some() || l.folder != ml.folder {
                    break;
                }
                if let Some(o) = out.get_mut(j) {
                    *o = Some(m);
                }
            }
        }
        out
    }

    /// Bytes of pixel data held by the library (shared tiles counted in full).
    pub fn memory_bytes(&self) -> u64 {
        self.drawings.values().map(paging::Slot::memory_bytes).sum()
    }

    /// Replace the pegs with repaired ones (file loading); layers on pegs that do not exist are
    /// taken off.
    pub fn set_pegs(&mut self, pegs: Vec<Peg>) {
        self.pegs = peg::sanitize(pegs);
        let ids: Vec<PegId> = self.pegs.iter().map(|p| p.id).collect();
        for l in &mut self.layers {
            if l.peg.is_some_and(|p| !ids.contains(&p)) {
                l.peg = None;
            }
        }
    }

    pub fn peg(&self, id: PegId) -> Option<&Peg> {
        self.pegs.iter().find(|p| p.id == id)
    }

    pub fn peg_mut(&mut self, id: PegId) -> Option<&mut Peg> {
        self.pegs.iter_mut().find(|p| p.id == id)
    }

    /// Add a peg pivoting about the canvas centre. Returns its id.
    pub fn add_peg(&mut self, name: &str, parent: Option<PegId>) -> Result<PegId> {
        if self.pegs.len() >= peg::MAX_PEGS {
            return Err(DocError::TooManyPegs);
        }
        if let Some(p) = parent
            && self.peg(p).is_none()
        {
            return Err(DocError::NoPeg(p.0));
        }
        let id = PegId(self.pegs.iter().map(|p| p.id.0).max().unwrap_or(0).saturating_add(1));
        let pivot = (self.width as f32 / 2.0, self.height as f32 / 2.0);
        self.pegs.push(Peg { id, name: name.chars().take(200).collect(), parent, pivot, keys: Vec::new() });
        Ok(id)
    }

    /// Remove a peg: its layers are taken off and its child pegs hang from its parent.
    pub fn delete_peg(&mut self, id: PegId) -> Result<Peg> {
        let i = self.pegs.iter().position(|p| p.id == id).ok_or(DocError::NoPeg(id.0))?;
        let gone = self.pegs.remove(i);
        for p in &mut self.pegs {
            if p.parent == Some(id) {
                p.parent = gone.parent;
            }
        }
        for l in &mut self.layers {
            if l.peg == Some(id) {
                l.peg = None;
            }
        }
        Ok(gone)
    }

    /// Hang a peg from another (or from nothing). Refuses loops.
    pub fn set_peg_parent(&mut self, id: PegId, parent: Option<PegId>) -> Result<()> {
        if let Some(q) = parent {
            if self.peg(q).is_none() {
                return Err(DocError::NoPeg(q.0));
            }
            if peg::would_loop(&self.pegs, id, q) {
                return Err(DocError::PegLoop);
            }
        }
        self.peg_mut(id).ok_or(DocError::NoPeg(id.0))?.parent = parent;
        Ok(())
    }

    /// The transform of a layer's drawings at `frame` (drawing pixels → canvas pixels): its peg
    /// chain, or the identity.
    pub fn layer_transform(&self, layer: usize, frame: u32) -> Affine {
        match self.layers.get(layer).and_then(|l| l.peg) {
            Some(id) => peg::chain(&self.pegs, id, frame),
            None => Affine::IDENTITY,
        }
    }

    /// Canvas positions of a peg's pivot over `from..=to` (its motion path), at most 10 000
    /// points.
    pub fn motion_path(&self, id: PegId, from: u32, to: u32) -> Vec<(f32, f32)> {
        let Some(p) = self.peg(id) else { return Vec::new() };
        let to = to.min(self.length.saturating_sub(1)).min(from.saturating_add(10_000));
        (from..=to).map(|f| peg::chain(&self.pegs, id, f).apply(p.pivot.0, p.pivot.1)).collect()
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
