//! The command set: every change to a project, as data (JSON for the CLI and agents).

use serde::{Deserialize, Serialize};
use tooncraft_brush::{Brush, Sample};
use tooncraft_doc::{Cell, DocError, OnionSkin, Pin};
use tooncraft_raster::fill::FillOptions;

use crate::undo::{Entry, Part};
use crate::{Engine, EngineError, Result};

/// What the fill bucket looks at to find the area.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FillReference {
    /// The drawing being filled only.
    Layer,
    /// Every visible layer (paint under ink on its own layer).
    #[default]
    AllLayers,
}

/// One change to a project. `layer` is a layer index (0 = bottom), `frame` is 0-based.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    /// Paint a stroke (a new drawing is made on a blank cell).
    Stroke {
        layer: usize,
        frame: u32,
        brush: Brush,
        samples: Vec<Sample>,
    },
    /// Paint-bucket fill from a point.
    Fill {
        layer: usize,
        frame: u32,
        x: u32,
        y: u32,
        /// Straight RGBA.
        color: [u8; 4],
        #[serde(default)]
        options: FillOptions,
        #[serde(default)]
        reference: FillReference,
    },
    /// Start a new, empty drawing at the frame.
    NewDrawing {
        layer: usize,
        frame: u32,
    },
    /// Copy the drawing at the frame into a new drawing on the next frame (later frames move).
    DuplicateDrawing {
        layer: usize,
        frame: u32,
    },
    /// Insert a frame (one layer, or all when `layer` is absent).
    InsertFrame {
        #[serde(default)]
        layer: Option<usize>,
        frame: u32,
    },
    /// Delete a frame (one layer, or all when `layer` is absent).
    DeleteFrame {
        #[serde(default)]
        layer: Option<usize>,
        frame: u32,
    },
    /// Show the drawing at the frame one frame longer.
    ExtendHold {
        layer: usize,
        frame: u32,
    },
    /// Show the drawing at the frame for exactly `length` frames (later frames move).
    SetExposure {
        layer: usize,
        frame: u32,
        length: u32,
    },
    /// Show the drawing at the frame one frame shorter.
    ShortenHold {
        layer: usize,
        frame: u32,
    },
    /// Make the frame blank (the drawing stays in the library).
    ClearCell {
        layer: usize,
        frame: u32,
    },
    /// Expose every drawing in `from..=to` for `step` frames.
    Retime {
        layer: usize,
        from: u32,
        to: u32,
        step: u32,
    },
    /// Paste a block of cells: `block[0]` goes to `layer`, `block[1]` to the layer above, and so
    /// on, starting at `frame`. `insert` moves later cells right; `duplicate` pastes independent
    /// copies of the drawings instead of more exposures of the same ones.
    PasteCells {
        layer: usize,
        frame: u32,
        block: Vec<Vec<Cell>>,
        #[serde(default)]
        insert: bool,
        #[serde(default)]
        duplicate: bool,
    },
    /// Blank `from..=to` on the layers (or remove those frames when `shift`).
    ClearRange {
        layers: Vec<usize>,
        from: u32,
        to: u32,
        #[serde(default)]
        shift: bool,
    },
    /// Play `from..=to` backwards.
    Reverse {
        layer: usize,
        from: u32,
        to: u32,
    },
    /// Repeat `from..=to` `times` more times after itself.
    Cycle {
        layer: usize,
        from: u32,
        to: u32,
        times: u32,
    },
    /// Images (PNG, JPEG, WebP, TIFF …) as drawings on a new layer, one per `hold` frames from
    /// `frame`, centred and scaled down to fit the canvas.
    ImportImages {
        paths: Vec<String>,
        frame: u32,
        #[serde(default = "one_frame")]
        hold: u32,
        name: String,
    },
    /// Erase all pixels of the drawing at the frame.
    ClearDrawing {
        layer: usize,
        frame: u32,
    },
    AddLayer {
        name: String,
        #[serde(default)]
        index: Option<usize>,
    },
    DeleteLayer {
        layer: usize,
    },
    RenameLayer {
        layer: usize,
        name: String,
    },
    MoveLayer {
        from: usize,
        to: usize,
    },
    SetLayer {
        layer: usize,
        #[serde(default)]
        visible: Option<bool>,
        #[serde(default)]
        locked: Option<bool>,
        #[serde(default)]
        opacity: Option<f32>,
    },
    SetLength {
        frames: u32,
    },
    SetFps {
        fps: u32,
    },
    /// Remove drawings no cell shows.
    PurgeUnused,
    /// Settings (not undo steps).
    SetOnion {
        onion: OnionSkin,
    },
    TogglePin {
        layer: usize,
        frame: u32,
    },
    SetLightTable {
        #[serde(default)]
        enabled: Option<bool>,
        #[serde(default)]
        opacity: Option<f32>,
    },
    SetPaper {
        color: [u8; 3],
    },
}

fn one_frame() -> u32 {
    1
}

fn layers_entry(e: &Engine, label: &str) -> Entry {
    Entry { label: label.into(), parts: vec![Part::Layers(e.project.layers.clone())] }
}

/// Run `edit` on the layers, recording one undo step; on error the layers are restored.
fn edit_layers(e: &mut Engine, label: &str, edit: impl FnOnce(&mut Engine) -> Result<()>) -> Result<()> {
    let entry = layers_entry(e, label);
    match edit(e) {
        Ok(()) => {
            e.push(entry);
            Ok(())
        }
        Err(err) => {
            entry.swap(&mut e.project);
            Err(err)
        }
    }
}

fn frame_ok(e: &Engine, frame: u32) -> Result<()> {
    if frame >= e.project.length() { Err(DocError::NoFrame(frame).into()) } else { Ok(()) }
}

pub(crate) fn apply(e: &mut Engine, cmd: Command) -> Result<()> {
    match cmd {
        Command::Stroke { layer, frame, brush, samples } => {
            e.begin_stroke(layer, frame, brush)?;
            for s in samples.into_iter().take(1_000_000) {
                e.stroke_to(s)?;
            }
            e.end_stroke()
        }
        Command::Fill { layer, frame, x, y, color, options, reference } => {
            frame_ok(e, frame)?;
            e.fill(layer, frame, x, y, color, options, reference).map(|_| ())
        }
        Command::NewDrawing { layer, frame } => {
            e.check_unlocked(layer)?;
            frame_ok(e, frame)?;
            let before = Part::Layers(e.project.layers.clone());
            let id = e.project.add_drawing(None)?;
            e.project.layer_mut(layer)?.set_key(frame, id);
            e.push(Entry { label: "New drawing".into(), parts: vec![Part::Drawing { id, surface: None }, before] });
            Ok(())
        }
        Command::DuplicateDrawing { layer, frame } => {
            e.check_unlocked(layer)?;
            frame_ok(e, frame)?;
            let src = e.project.drawing_at(layer, frame).ok_or(EngineError::NoDrawing(frame))?;
            let copy = e.project.drawing(src).cloned().ok_or(EngineError::NoDrawing(frame))?;
            let before = Part::Layers(e.project.layers.clone());
            let id = e.project.add_drawing(Some(copy))?;
            let len = e.project.length();
            let l = e.project.layer_mut(layer)?;
            let (_, end) = l.exposure_range(frame).unwrap_or((frame, frame + 1));
            if end < len {
                l.insert_frame(end);
                l.set_key(end, id);
            } else if frame + 1 < len {
                l.set_key(frame + 1, id);
            } else {
                l.set_key(frame, id);
            }
            e.push(Entry { label: "Duplicate drawing".into(), parts: vec![Part::Drawing { id, surface: None }, before] });
            Ok(())
        }
        Command::InsertFrame { layer, frame } => edit_layers(e, "Insert frame", |e| {
            frame_ok(e, frame)?;
            for (i, l) in e.project.layers.iter_mut().enumerate() {
                if layer.is_none_or(|t| t == i) && !l.locked {
                    l.insert_frame(frame);
                }
            }
            Ok(())
        }),
        Command::DeleteFrame { layer, frame } => edit_layers(e, "Delete frame", |e| {
            frame_ok(e, frame)?;
            for (i, l) in e.project.layers.iter_mut().enumerate() {
                if layer.is_none_or(|t| t == i) && !l.locked {
                    l.delete_frame(frame);
                }
            }
            Ok(())
        }),
        Command::ExtendHold { layer, frame } => edit_layers(e, "Extend hold", |e| {
            e.check_unlocked(layer)?;
            e.project.layer_mut(layer)?.extend_hold(frame).map_err(DocError::from)?;
            Ok(())
        }),
        Command::SetExposure { layer, frame, length } => edit_layers(e, "Change hold", |e| {
            e.check_unlocked(layer)?;
            e.project.layer_mut(layer)?.set_exposure_length(frame, length).map_err(DocError::from)?;
            Ok(())
        }),
        Command::ShortenHold { layer, frame } => edit_layers(e, "Shorten hold", |e| {
            e.check_unlocked(layer)?;
            e.project.layer_mut(layer)?.shorten_hold(frame).map_err(DocError::from)?;
            Ok(())
        }),
        Command::ClearCell { layer, frame } => edit_layers(e, "Clear frame", |e| {
            e.check_unlocked(layer)?;
            frame_ok(e, frame)?;
            e.project.layer_mut(layer)?.clear(frame);
            Ok(())
        }),
        Command::Retime { layer, from, to, step } => edit_layers(e, "Re-time", |e| {
            e.check_unlocked(layer)?;
            e.project.layer_mut(layer)?.retime(from, to, step).map_err(DocError::from)?;
            Ok(())
        }),
        Command::PasteCells { layer, frame, block, insert, duplicate } => {
            frame_ok(e, frame)?;
            if block.len() > tooncraft_doc::MAX_LAYERS || block.iter().any(|r| r.len() > tooncraft_doc::MAX_FRAMES as usize) {
                return Err(DocError::TooLong.into());
            }
            for i in 0..block.len() {
                e.check_unlocked(layer + i)?;
            }
            let mut parts = vec![Part::Layers(e.project.layers.clone())];
            let mut block = block;
            if duplicate {
                let mut map = std::collections::BTreeMap::new();
                for row in &mut block {
                    for c in row.iter_mut() {
                        if let Cell::Key(d) = *c {
                            let new = match map.get(&d) {
                                Some(n) => *n,
                                None => {
                                    let Some(src) = e.project.drawing(d).cloned() else { continue };
                                    let n = e.project.add_drawing(Some(src))?;
                                    parts.insert(0, Part::Drawing { id: n, surface: None });
                                    map.insert(d, n);
                                    n
                                }
                            };
                            *c = Cell::Key(new);
                        }
                    }
                }
            }
            for (i, row) in block.iter().enumerate() {
                let ok: Vec<Cell> = row
                    .iter()
                    .map(|c| match c {
                        Cell::Key(d) if e.project.drawing(*d).is_none() => Cell::Blank,
                        c => *c,
                    })
                    .collect();
                e.project.layer_mut(layer + i)?.paste_cells(frame, &ok, insert);
            }
            e.push(Entry { label: "Paste frames".into(), parts });
            Ok(())
        }
        Command::ClearRange { layers, from, to, shift } => edit_layers(e, if shift { "Delete frames" } else { "Clear frames" }, |e| {
            for l in layers {
                e.check_unlocked(l)?;
                e.project.layer_mut(l)?.clear_range(from, to, shift);
            }
            Ok(())
        }),
        Command::Reverse { layer, from, to } => edit_layers(e, "Reverse", |e| {
            e.check_unlocked(layer)?;
            e.project.layer_mut(layer)?.reverse_range(from, to).map_err(DocError::from)?;
            Ok(())
        }),
        Command::Cycle { layer, from, to, times } => edit_layers(e, "Cycle", |e| {
            e.check_unlocked(layer)?;
            e.project.layer_mut(layer)?.cycle(from, to, times).map_err(DocError::from)?;
            Ok(())
        }),
        Command::ImportImages { paths, frame, hold, name } => crate::import::images(e, &paths, frame, hold.clamp(1, 64), &name),
        Command::ClearDrawing { layer, frame } => {
            e.check_unlocked(layer)?;
            let d = e.project.drawing_at(layer, frame).ok_or(EngineError::NoDrawing(frame))?;
            let s = e.project.drawing_mut(d).ok_or(EngineError::NoDrawing(frame))?;
            let indices: Vec<u32> = s.tiles().map(|(i, _)| i).collect();
            let mut tiles = Vec::with_capacity(indices.len());
            for i in indices {
                if let Ok(old) = s.swap_tile(i, None) {
                    tiles.push((i, old));
                }
            }
            e.push(Entry { label: "Clear drawing".into(), parts: vec![Part::Tiles { drawing: d, tiles }] });
            Ok(())
        }
        Command::AddLayer { name, index } => edit_layers(e, "Add layer", |e| {
            let at = index.unwrap_or(e.project.layers.len());
            e.project.add_layer(&name, at)?;
            Ok(())
        }),
        Command::DeleteLayer { layer } => edit_layers(e, "Delete layer", |e| {
            e.project.delete_layer(layer)?;
            Ok(())
        }),
        Command::RenameLayer { layer, name } => edit_layers(e, "Rename layer", |e| {
            e.project.layer_mut(layer)?.name = name.chars().take(200).collect();
            Ok(())
        }),
        Command::MoveLayer { from, to } => edit_layers(e, "Move layer", |e| {
            e.project.move_layer(from, to)?;
            Ok(())
        }),
        Command::SetLayer { layer, visible, locked, opacity } => edit_layers(e, "Layer settings", |e| {
            let l = e.project.layer_mut(layer)?;
            if let Some(v) = visible {
                l.visible = v;
            }
            if let Some(v) = locked {
                l.locked = v;
            }
            if let Some(o) = opacity.filter(|o| o.is_finite()) {
                l.opacity = o.clamp(0.0, 1.0);
            }
            Ok(())
        }),
        Command::SetLength { frames } => {
            let before = Part::Length { length: e.project.length(), layers: e.project.layers.clone() };
            e.project.set_length(frames)?;
            e.push(Entry { label: "Shot length".into(), parts: vec![before] });
            Ok(())
        }
        Command::SetFps { fps } => {
            let before = Part::Fps(e.project.fps());
            e.project.set_fps(fps)?;
            e.push(Entry { label: "Frame rate".into(), parts: vec![before] });
            Ok(())
        }
        Command::PurgeUnused => {
            let mut parts = Vec::new();
            for id in e.project.unused_drawings() {
                let s = e.project.swap_drawing(id, None);
                parts.push(Part::Drawing { id, surface: s });
            }
            e.push(Entry { label: "Purge unused drawings".into(), parts });
            Ok(())
        }
        Command::SetOnion { onion } => {
            e.project.onion = onion.sanitized();
            e.touch();
            Ok(())
        }
        Command::TogglePin { layer, frame } => {
            frame_ok(e, frame)?;
            let id = e.project.layer(layer)?.id;
            e.project.light_table.toggle(Pin { layer: id, frame });
            e.touch();
            Ok(())
        }
        Command::SetLightTable { enabled, opacity } => {
            if let Some(v) = enabled {
                e.project.light_table.enabled = v;
            }
            if let Some(o) = opacity.filter(|o| o.is_finite()) {
                e.project.light_table.opacity = o.clamp(0.0, 1.0);
            }
            e.touch();
            Ok(())
        }
        Command::SetPaper { color } => {
            e.project.paper.color = color;
            e.touch();
            Ok(())
        }
    }
}

/// Cells of a layer as a compact string (`#` key, `-` hold, `.` blank), for tests and the CLI.
pub fn sheet(cells: &[Cell]) -> String {
    cells
        .iter()
        .map(|c| match c {
            Cell::Key(_) => '#',
            Cell::Hold => '-',
            Cell::Blank => '.',
        })
        .collect()
}
