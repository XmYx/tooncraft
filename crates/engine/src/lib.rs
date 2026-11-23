//! ToonCraft's engine (layer 3, UI-free): everything that changes a project is a [`Command`].
//!
//! The desktop app, `tooncraft-cli run` and tests all go through [`Engine::apply`] (or the live
//! stroke calls, which end in the same undo record a `stroke` command makes). Each change is one
//! undo [`Entry`] made of self-inverse swaps, so undo and redo are the same operation. Undo is
//! unlimited in steps and bounded in memory ([`Engine::undo_budget`]): when the history holds
//! more pixel data than the budget, the oldest steps are dropped.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

pub mod audio;
mod boards;
pub mod burnin;
#[cfg(test)]
mod cells_tests;
mod command;
pub mod demo;
pub mod drawing;
#[cfg(test)]
mod drawing_tests;
mod effects;
pub mod export;
pub mod import;
pub mod inbetween;
mod interchange;
mod layerops;
pub mod library;
pub mod lipsync;
pub mod paging;
pub mod playback;
pub mod preset;
pub mod preview;
pub mod select;
pub mod shapes;
pub mod touch;
mod undo;

pub use command::{Command, FillReference, sheet};
pub use undo::Entry;

use tooncraft_brush::{Brush, Sample, Stroke, StrokeEngine};
use tooncraft_doc::{DocError, DrawingId, Project, RenderOptions};
use tooncraft_raster::fill::{self, FillOptions, MAX_FILL_PIXELS};
use tooncraft_raster::{RasterError, premultiply, unpremultiply};

use undo::Part;

/// Default undo memory budget: 1.5 GiB of tile data.
pub const DEFAULT_UNDO_BUDGET: u64 = 1536 << 20;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum EngineError {
    #[error(transparent)]
    Doc(#[from] DocError),
    #[error(transparent)]
    Raster(#[from] RasterError),
    #[error("layer “{0}” is locked")]
    Locked(String),
    #[error("there is no drawing at frame {0}")]
    NoDrawing(u32),
    #[error("a stroke is already in progress")]
    Busy,
    #[error("no stroke is in progress")]
    NotStroking,
    #[error("nothing to undo")]
    NothingToUndo,
    #[error("nothing to redo")]
    NothingToRedo,
    #[error("could not import {0}")]
    Import(String),
    #[error("too many {0}")]
    Limit(&'static str),
    #[error("there is no sound clip {0}")]
    NoSound(usize),
    #[error("there is no effect {0} on this layer")]
    NoEffect(usize),
    #[error("a layer can have at most {max} effects", max = tooncraft_fx::MAX_EFFECTS)]
    TooManyEffects,
    #[error("that change does not fit this effect")]
    EffectMismatch,
    #[error("cannot merge: {0}")]
    Merge(String),
    #[error("layer “{0}” has its transparency locked, so it cannot take a fill")]
    TransparencyLocked(String),
    #[error("there is no swatch {1} in palette {0}")]
    NoSwatch(usize, usize),
}

pub type Result<T> = std::result::Result<T, EngineError>;

struct LiveStroke {
    last: Option<Sample>,
    drawing: DrawingId,
    created: Vec<Part>,
    engine: StrokeEngine,
    stroke: Stroke,
    /// Symmetry copies: each has its own stroke engine (smoothing and spacing per copy), all
    /// painting into the same stroke so overlaps never darken.
    mirrors: Vec<(tooncraft_doc::symmetry::Mirror, StrokeEngine)>,
}

fn mirror_sample(m: &tooncraft_doc::symmetry::Mirror, s: Sample) -> Sample {
    let (x, y) = m.apply(s.x, s.y);
    let (tx, ty) = m.apply_vector(s.tilt_x, s.tilt_y);
    Sample { x, y, tilt_x: tx, tilt_y: ty, ..s }
}

/// A project with its history.
pub struct Engine {
    pub project: Project,
    pub(crate) undo: Vec<Entry>,
    redo: Vec<Entry>,
    /// Most bytes of tile data the undo history may hold.
    pub undo_budget: u64,
    live: Option<LiveStroke>,
    changes: u64,
    saved_at: u64,
    /// Long shots: the page file drawings go to (made on first use) and how many bytes of
    /// drawings may stay in memory (see [`paging`]).
    pub(crate) pages: Option<std::sync::Arc<tooncraft_doc::paging::PageFile>>,
    pub page_budget: u64,
}

impl Engine {
    pub fn new(project: Project) -> Self {
        Self {
            project,
            undo: Vec::new(),
            redo: Vec::new(),
            undo_budget: DEFAULT_UNDO_BUDGET,
            live: None,
            changes: 0,
            saved_at: 0,
            pages: None,
            page_budget: paging::DEFAULT_PAGE_BUDGET,
        }
    }

    /// Changes since the project was created or opened (undo and redo count as changes).
    pub fn changes(&self) -> u64 {
        self.changes
    }

    /// Unsaved changes?
    pub fn is_dirty(&self) -> bool {
        self.changes != self.saved_at
    }

    pub fn mark_saved(&mut self) {
        self.saved_at = self.changes;
    }

    /// Something changed outside the undo history (settings such as onion skin).
    pub fn touch(&mut self) {
        self.changes += 1;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() && self.live.is_none()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty() && self.live.is_none()
    }

    /// Label of the step undo would revert.
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|e| e.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|e| e.label.as_str())
    }

    /// Steps that can be redone.
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    /// Bytes of tile data held by the undo and redo history.
    pub fn history_bytes(&self) -> u64 {
        self.undo.iter().chain(&self.redo).map(Entry::bytes).sum()
    }

    pub fn undo(&mut self) -> Result<()> {
        if self.live.is_some() {
            return Err(EngineError::Busy);
        }
        let e = self.undo.pop().ok_or(EngineError::NothingToUndo)?;
        let inv = e.swap(&mut self.project);
        self.redo.push(inv);
        self.changes += 1;
        Ok(())
    }

    pub fn redo(&mut self) -> Result<()> {
        if self.live.is_some() {
            return Err(EngineError::Busy);
        }
        let e = self.redo.pop().ok_or(EngineError::NothingToRedo)?;
        let inv = e.swap(&mut self.project);
        self.undo.push(inv);
        self.changes += 1;
        Ok(())
    }

    /// The history as labels, oldest first: the steps that can be undone, then the steps that
    /// can be redone (nearest first). `undo_labels().len()` is the current position.
    pub fn history(&self) -> (Vec<&str>, Vec<&str>) {
        (self.undo.iter().map(|e| e.label.as_str()).collect(), self.redo.iter().rev().map(|e| e.label.as_str()).collect())
    }

    /// Move through the history until `position` steps are applied (0 is the shot as it was
    /// opened, or before the oldest step still remembered). Undoes or redoes as needed, one step
    /// at a time; stops at the first error. Returns the position reached.
    pub fn go_to_step(&mut self, position: usize) -> Result<usize> {
        let total = self.undo.len() + self.redo.len();
        let position = position.min(total);
        while self.undo.len() > position {
            self.undo()?;
        }
        while self.undo.len() < position {
            self.redo()?;
        }
        Ok(self.undo.len())
    }

    pub(crate) fn push(&mut self, entry: Entry) {
        if entry.parts.is_empty() {
            return;
        }
        self.undo.push(entry);
        self.redo.clear();
        self.changes += 1;
        let mut total: u64 = self.undo.iter().map(Entry::bytes).sum();
        while total > self.undo_budget && self.undo.len() > 1 {
            let dropped = self.undo.remove(0);
            total = total.saturating_sub(dropped.bytes());
        }
    }

    pub(crate) fn check_unlocked(&self, layer: usize) -> Result<()> {
        let l = self.project.layer(layer)?;
        if self.project.is_locked(l) {
            return Err(EngineError::Locked(l.name.clone()));
        }
        Ok(())
    }

    /// The drawing to paint on at a frame; on a blank cell a new drawing is made (the parts
    /// that undo that are returned).
    pub(crate) fn ensure_drawing(&mut self, layer: usize, frame: u32) -> Result<(DrawingId, Vec<Part>)> {
        self.check_unlocked(layer)?;
        if frame >= self.project.length() {
            return Err(DocError::NoFrame(frame).into());
        }
        if let Some(d) = self.project.drawing_at(layer, frame) {
            return Ok((d, Vec::new()));
        }
        let before = Part::Layers(self.project.layers.clone());
        let id = self.project.add_drawing(None)?;
        self.project.layer_mut(layer)?.set_key(frame, id);
        Ok((id, vec![Part::Drawing { id, surface: None }, before]))
    }

    // ----- live strokes (the app's pen input) -----

    /// Start painting on a layer at a frame.
    pub fn begin_stroke(&mut self, layer: usize, frame: u32, brush: Brush) -> Result<()> {
        if self.live.is_some() {
            return Err(EngineError::Busy);
        }
        let (drawing, created) = self.ensure_drawing(layer, frame)?;
        // a layer with its transparency locked only takes paint where there is paint
        let brush = if self.project.layer(layer)?.lock_alpha { Brush { compose: tooncraft_brush::Compose::KeepAlpha, ..brush } } else { brush };
        let images = tooncraft_brush::Images {
            tip: brush.tip.and_then(|id| self.project.tips.get(&id).cloned()),
            grain: brush.grain_image.and_then(|id| self.project.tips.get(&id).cloned()),
        };
        let surface = self.project.drawing_mut(drawing).ok_or(EngineError::NoDrawing(frame))?;
        let stroke = Stroke::begin_with(surface, brush, images);
        let (w, h) = (self.project.width(), self.project.height());
        let mirrors = self.project.symmetry.copies(w, h).into_iter().map(|m| (m, StrokeEngine::new(brush))).collect();
        self.live = Some(LiveStroke { last: None, drawing, created, engine: StrokeEngine::new(brush), stroke, mirrors });
        Ok(())
    }

    pub fn is_stroking(&self) -> bool {
        self.live.is_some()
    }

    /// Add one input sample to the live stroke; returns the number of dabs painted.
    pub fn stroke_to(&mut self, sample: Sample) -> Result<usize> {
        let live = self.live.as_mut().ok_or(EngineError::NotStroking)?;
        if let Some(s) = sample.sanitized() {
            live.last = Some(s);
        }
        let mut dabs = live.engine.feed(sample);
        for (m, eng) in &mut live.mirrors {
            dabs.extend(eng.feed(mirror_sample(m, sample)));
        }
        let surface = self.project.drawing_mut(live.drawing).ok_or(EngineError::NotStroking)?;
        live.stroke.add(surface, &dabs);
        Ok(dabs.len())
    }

    /// Finish the live stroke as one undo step.
    pub fn end_stroke(&mut self) -> Result<()> {
        let mut live = self.live.take().ok_or(EngineError::NotStroking)?;
        let mut dabs = live.engine.finish();
        for (_, eng) in &mut live.mirrors {
            dabs.extend(eng.finish());
        }
        let surface = self.project.drawing_mut(live.drawing).ok_or(EngineError::NotStroking)?;
        live.stroke.add(surface, &dabs);
        let tiles = live.stroke.end(surface);
        let mut parts = live.created;
        parts.push(Part::Tiles { drawing: live.drawing, tiles });
        self.push(Entry { label: "Stroke".into(), parts });
        Ok(())
    }

    /// Abandon the live stroke, restoring the drawing.
    pub fn cancel_stroke(&mut self) {
        if let Some(live) = self.live.take() {
            if let Some(surface) = self.project.drawing_mut(live.drawing) {
                let tiles = live.stroke.end(surface);
                surface.swap_snapshot(tiles);
            }
            let entry = Entry { label: String::new(), parts: live.created };
            entry.swap(&mut self.project);
        }
    }

    /// The last input sample of the live stroke.
    pub fn live_last_sample(&self) -> Option<Sample> {
        self.live.as_ref().and_then(|l| l.last)
    }

    /// The drawing the live stroke paints on.
    pub fn live_drawing(&self) -> Option<DrawingId> {
        self.live.as_ref().map(|l| l.drawing)
    }

    // ----- queries -----

    /// Straight RGBA colour of the composited frame (with paper) at a canvas pixel.
    pub fn pick_color(&self, frame: u32, x: u32, y: u32) -> [u8; 4] {
        let img = tooncraft_doc::render_region(&self.project, frame, (x, y, 1, 1), RenderOptions::default());
        unpremultiply(img.pixel(0, 0))
    }

    fn fill(&mut self, layer: usize, frame: u32, x: u32, y: u32, color: [u8; 4], options: FillOptions, reference: FillReference) -> Result<bool> {
        match self.fill_parts(layer, frame, x, y, color, options, reference)? {
            Some(parts) => {
                self.push(Entry { label: "Fill".into(), parts });
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Fill the same point in every drawing exposed on the layer, as one undo step. Returns how
    /// many drawings were filled.
    pub(crate) fn fill_all(&mut self, layer: usize, x: u32, y: u32, color: [u8; 4], options: FillOptions, reference: FillReference) -> Result<usize> {
        let l = self.project.layer(layer)?;
        let mut frames = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for f in 0..self.project.length() {
            if let Some(d) = l.drawing_at(f)
                && seen.insert(d)
            {
                frames.push(f);
            }
        }
        let mut all = Vec::new();
        for f in frames {
            if let Some(parts) = self.fill_parts(layer, f, x, y, color, options, reference)? {
                all.extend(parts);
            }
        }
        let n = all.len();
        self.push(Entry { label: "Fill all frames".into(), parts: all });
        Ok(n)
    }

    fn fill_parts(
        &mut self,
        layer: usize,
        frame: u32,
        x: u32,
        y: u32,
        color: [u8; 4],
        options: FillOptions,
        reference: FillReference,
    ) -> Result<Option<Vec<Part>>> {
        self.check_unlocked(layer)?;
        if let Ok(l) = self.project.layer(layer)
            && l.lock_alpha
        {
            return Err(EngineError::TransparencyLocked(l.name.clone()));
        }
        let (w, h) = (self.project.width(), self.project.height());
        if x >= w || y >= h {
            return Ok(None);
        }
        // Huge canvases fill a window around the seed so memory stays bounded.
        let side = (MAX_FILL_PIXELS as f64).sqrt() as u32;
        let (wx, wy) = (x.saturating_sub(side / 2).min(w.saturating_sub(side)), y.saturating_sub(side / 2).min(h.saturating_sub(side)));
        let rect = (wx, wy, side.min(w), side.min(h));
        // `x`, `y` are in the drawing's own pixels: a pegged layer looks at the picture through
        // the inverse of its peg chain, so the area found lines up with what it fills.
        let space = self.project.layer_transform(layer, frame).inverse().filter(|t| !t.is_identity(1e-6));
        // repainting looks at this layer's own paint only
        let reference = if options.mode == fill::FillMode::Repaint { FillReference::Layer } else { reference };
        let opts = match reference {
            FillReference::Layer => RenderOptions { paper: false, only_layer: Some(layer), space, effects: false, ..Default::default() },
            FillReference::AllLayers => RenderOptions { paper: false, space, effects: false, ..Default::default() },
        };
        let reference_img = tooncraft_doc::render_region(&self.project, frame, rect, opts);
        let seed_alpha = reference_img.pixel(x - rect.0, y - rect.1)[3];
        match options.mode {
            fill::FillMode::EmptyOnly if seed_alpha > 0 => return Ok(None),
            fill::FillMode::Repaint if seed_alpha == 0 => return Ok(None),
            _ => {}
        }
        let Some(mask) = fill::flood(&reference_img, (rect.0, rect.1), (x - rect.0, y - rect.1), options)? else { return Ok(None) };
        let (drawing, mut parts) = self.ensure_drawing(layer, frame)?;
        let surface = self.project.drawing_mut(drawing).ok_or(EngineError::NoDrawing(frame))?;
        surface.begin_capture();
        if options.mode == fill::FillMode::Repaint {
            // over the old paint (a fill goes under it, so lines stay on top)
            fill::paint_over(surface, &mask, premultiply(color));
        } else {
            fill::apply(surface, &mask, premultiply(color));
        }
        let tiles = surface.take_capture();
        parts.push(Part::Tiles { drawing, tiles });
        Ok(Some(parts))
    }

    /// Apply a command as one undo step.
    pub fn apply(&mut self, cmd: Command) -> Result<()> {
        if self.live.is_some() {
            return Err(EngineError::Busy);
        }
        command::apply(self, cmd)
    }
}

#[cfg(test)]
mod history_tests;
#[cfg(test)]
mod layerops_tests;
#[cfg(test)]
mod swatch_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod timeline_tests;
