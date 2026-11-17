//! The command set: every change to a project, as data (JSON for the CLI and agents).

use serde::{Deserialize, Serialize};
use tooncraft_brush::{Brush, Sample};
use tooncraft_doc::{Cell, DocError, Ease, Mark, OnionSkin, PegId, PegKey, Pin, Shake, TimingPreset};
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
    /// Fill the same point in every drawing exposed on the layer (one undo step).
    FillAllFrames {
        layer: usize,
        x: u32,
        y: u32,
        color: [u8; 4],
        #[serde(default)]
        options: FillOptions,
        #[serde(default)]
        reference: FillReference,
    },
    /// A line, rectangle or ellipse drawn with the brush between two corners (`constrain`:
    /// 15° lines, squares, circles).
    Shape {
        layer: usize,
        frame: u32,
        brush: Brush,
        shape: crate::shapes::ShapeKind,
        from: (f32, f32),
        to: (f32, f32),
        #[serde(default)]
        constrain: bool,
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
    /// Use a sound file (WAV, AIFF, MP3, AAC/M4A, FLAC, Ogg …) as the shot's soundtrack, starting at
    /// `offset` frames; no `path` removes it.
    SetSound {
        #[serde(default)]
        path: Option<String>,
        #[serde(default)]
        offset: i32,
    },
    /// Set (or replace) the camera key at `frame`: centre `x`, `y` in canvas pixels, `zoom` (1 =
    /// whole canvas), `angle` in degrees. The camera is switched on.
    SetCameraKey {
        frame: u32,
        x: f32,
        y: f32,
        zoom: f32,
        angle: f32,
    },
    /// Remove the camera key at `frame`.
    RemoveCameraKey {
        frame: u32,
    },
    /// Render exports through the camera, or not.
    SetCamera {
        enabled: bool,
    },
    /// Add a peg (pivot at the canvas centre), optionally hanging from `parent`.
    AddPeg {
        name: String,
        #[serde(default)]
        parent: Option<PegId>,
    },
    /// Remove a peg: its layers come off it and its child pegs hang from its parent.
    DeletePeg {
        peg: PegId,
    },
    RenamePeg {
        peg: PegId,
        name: String,
    },
    /// Hang a peg from another peg, or from nothing (`None`). Loops are refused.
    SetPegParent {
        peg: PegId,
        #[serde(default)]
        parent: Option<PegId>,
    },
    /// The canvas point the peg rotates and scales about.
    SetPegPivot {
        peg: PegId,
        x: f32,
        y: f32,
    },
    /// Set (or replace) a peg key: move by `x`, `y`, rotate `angle` degrees clockwise, scale,
    /// skew (degrees), and the ease towards the next key.
    SetPegKey {
        peg: PegId,
        frame: u32,
        #[serde(default)]
        x: f32,
        #[serde(default)]
        y: f32,
        #[serde(default)]
        angle: f32,
        #[serde(default = "unit")]
        scale_x: f32,
        #[serde(default = "unit")]
        scale_y: f32,
        #[serde(default)]
        skew: f32,
        #[serde(default)]
        ease: Ease,
    },
    RemovePegKey {
        peg: PegId,
        frame: u32,
    },
    /// Hang a layer from a peg, or take it off (`None`). Its pixels do not change.
    SetLayerPeg {
        layer: usize,
        #[serde(default)]
        peg: Option<PegId>,
    },
    /// The layer's distance from the drawing plane for the multiplane camera (negative: nearer).
    SetLayerDepth {
        layer: usize,
        depth: f32,
    },
    /// Add a camera shake.
    AddCameraShake {
        shake: Shake,
    },
    /// Remove the camera shake at `index`.
    RemoveCameraShake {
        index: usize,
    },
    /// Add an image brush tip (grey mask; dark is ink) under `id`, replacing a tip with that id.
    AddTip {
        id: u64,
        name: String,
        width: u32,
        height: u32,
        alpha: Vec<u8>,
    },
    /// Add a brush tip from an image file (scaled down to 1024 px at most) under `id`.
    AddTipFromImage {
        id: u64,
        path: String,
    },
    /// The layers of a PSD / PSB or OpenRaster file, as new layers with a drawing at `frame`.
    ImportLayers {
        path: String,
        frame: u32,
    },
    /// An animated GIF as a new layer from `frame`, timed by its delays.
    ImportGif {
        path: String,
        frame: u32,
    },
    /// An XDTS exposure sheet and its drawing folders, one new layer per track.
    ImportSheet {
        path: String,
    },
    /// Move a sound clip (the first by default) to start at `offset` frames.
    MoveSound {
        offset: i32,
        #[serde(default)]
        index: usize,
    },
    /// Add a sound file as a new clip on audio track `track` (0 is the first), starting at
    /// frame `offset`.
    AddSound {
        path: String,
        #[serde(default)]
        offset: i32,
        #[serde(default)]
        track: u32,
    },
    RemoveSound {
        index: usize,
    },
    /// Change a sound clip: its track, start frame, the seconds skipped at the start of the file
    /// (`trim_start`), the seconds played (`length`; `to_end` plays to the end of the file),
    /// fades in seconds, volume (gain 0…4) and mute.
    SetSoundClip {
        index: usize,
        #[serde(default)]
        track: Option<u32>,
        #[serde(default)]
        offset: Option<i32>,
        #[serde(default)]
        trim_start: Option<f64>,
        #[serde(default)]
        length: Option<f64>,
        #[serde(default)]
        to_end: bool,
        #[serde(default)]
        fade_in: Option<f64>,
        #[serde(default)]
        fade_out: Option<f64>,
        #[serde(default)]
        volume: Option<f32>,
        #[serde(default)]
        muted: Option<bool>,
    },
    /// Key a clip's volume (gain 0…4) at a shot frame.
    SetVolumeKey {
        index: usize,
        frame: u32,
        gain: f32,
    },
    RemoveVolumeKey {
        index: usize,
        frame: u32,
    },
    /// Use `drawing` for `mouth` in the layer's mouth chart (`None`: forget that mouth).
    SetMouth {
        layer: usize,
        mouth: tooncraft_lipsync::Mouth,
        #[serde(default)]
        drawing: Option<tooncraft_doc::DrawingId>,
    },
    /// Detect mouths from the shot's sound and key the chart's drawings on the layer from
    /// `from` (one undo step; edit the keys afterwards as usual).
    LipSync {
        layer: usize,
        #[serde(default)]
        from: u32,
        #[serde(default)]
        settings: tooncraft_lipsync::Settings,
    },
    /// Key a mouth timing sheet (ToonCraft mouth sheet or switch data) on the layer from `from`.
    ImportMouthSheet {
        layer: usize,
        path: String,
        #[serde(default)]
        from: u32,
    },
    /// An audio track's name, mute, solo and volume.
    SetAudioTrack {
        track: u32,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        muted: Option<bool>,
        #[serde(default)]
        solo: Option<bool>,
        #[serde(default)]
        volume: Option<f32>,
    },
    /// Move / scale / rotate (radians) / flip (negative scale) the pixels inside `polygon`, about
    /// the centre of its bounding box.
    TransformSelection {
        layer: usize,
        frame: u32,
        polygon: Vec<(f32, f32)>,
        #[serde(default)]
        dx: f32,
        #[serde(default)]
        dy: f32,
        #[serde(default = "unit")]
        scale_x: f32,
        #[serde(default = "unit")]
        scale_y: f32,
        #[serde(default)]
        angle: f32,
    },
    /// Erase the pixels inside `polygon`.
    ClearSelection {
        layer: usize,
        frame: u32,
        polygon: Vec<(f32, f32)>,
    },
    /// Paste straight-RGBA pixels with their top-left at (x, y) (a new drawing on a blank cell).
    PastePixels {
        layer: usize,
        frame: u32,
        x: f32,
        y: f32,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
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
    /// How the layer combines with the layers below.
    SetLayerBlend {
        layer: usize,
        blend: tooncraft_doc::BlendMode,
    },
    /// The layer's label colour (`None`: no colour).
    SetLayerColor {
        layer: usize,
        #[serde(default)]
        color: Option<[u8; 3]>,
    },
    /// Put the layer in a folder (made if missing), or take it out (`None`).
    SetLayerFolder {
        layer: usize,
        #[serde(default)]
        folder: Option<String>,
    },
    /// A folder's visibility, opacity or collapsed state.
    SetFolder {
        name: String,
        #[serde(default)]
        visible: Option<bool>,
        #[serde(default)]
        opacity: Option<f32>,
        #[serde(default)]
        collapsed: Option<bool>,
    },
    /// Mark the drawing at the frame as a key, breakdown or inbetween (`None`: no mark).
    SetMark {
        layer: usize,
        frame: u32,
        #[serde(default)]
        mark: Option<Mark>,
    },
    /// Re-time `from..=to` with a timing preset (each drawing held as the preset says).
    ApplyTiming {
        layer: usize,
        from: u32,
        to: u32,
        preset: TimingPreset,
    },
    /// Re-space the drawings between keys and breakdowns in `from..=to` along a spacing chart;
    /// the keys and breakdowns keep their frames.
    RetimeBetweenKeys {
        layer: usize,
        from: u32,
        to: u32,
        #[serde(default)]
        ease: Ease,
    },
    /// Add `count` new, empty inbetween drawings between the drawing at `frame` and the next one,
    /// placed along the spacing chart for `ease`; with `guides`, a guide layer above shows each
    /// one as a blend of the two drawings, weighted by where it falls.
    AddInbetweens {
        layer: usize,
        frame: u32,
        count: u32,
        #[serde(default)]
        ease: Ease,
        #[serde(default = "yes")]
        guides: bool,
    },
    /// Clip the layer to the layer below (it shows only where that layer has paint), or not.
    SetLayerClip {
        layer: usize,
        clip: bool,
    },
    /// Lock a folder (and everything in it) against drawing and re-timing.
    SetFolderLock {
        name: String,
        locked: bool,
    },
    /// Put a folder in another folder, or at the top (`None`). Loops are refused.
    SetFolderParent {
        name: String,
        #[serde(default)]
        parent: Option<String>,
    },
    /// A folder's label colour (`None`: no colour).
    SetFolderColor {
        name: String,
        #[serde(default)]
        color: Option<[u8; 3]>,
    },
    /// Rename a folder (its layers and folders follow).
    RenameFolder {
        name: String,
        to: String,
    },
    /// A new folder (empty folders are kept until a layer leaves the last one in it), optionally
    /// inside `parent`, holding `layers`.
    AddFolder {
        name: String,
        #[serde(default)]
        parent: Option<String>,
        #[serde(default)]
        layers: Vec<usize>,
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
    /// Add an effect to the layer's stack at `index` (default: last, so it runs after the others).
    AddEffect {
        layer: usize,
        effect: tooncraft_fx::Effect,
        #[serde(default)]
        index: Option<usize>,
    },
    /// Replace the effect at `index` (parameters, keys and switch).
    SetEffect {
        layer: usize,
        index: usize,
        effect: tooncraft_fx::Effect,
    },
    /// Change an effect's parameters as seen at `frame`: a key there when it has keys, otherwise
    /// the effect itself.
    EditEffect {
        layer: usize,
        index: usize,
        frame: u32,
        value: tooncraft_fx::EffectKind,
    },
    /// Key an effect at `frame` with `value`, or with the parameters it shows there.
    SetEffectKey {
        layer: usize,
        index: usize,
        frame: u32,
        #[serde(default)]
        value: Option<tooncraft_fx::EffectKind>,
    },
    RemoveEffectKey {
        layer: usize,
        index: usize,
        frame: u32,
    },
    /// Switch an effect on or off.
    EnableEffect {
        layer: usize,
        index: usize,
        enabled: bool,
    },
    RemoveEffect {
        layer: usize,
        index: usize,
    },
    /// Move an effect within the stack.
    MoveEffect {
        layer: usize,
        from: usize,
        to: usize,
    },
    /// Key the layer's opacity (0…1) at `frame`.
    SetOpacityKey {
        layer: usize,
        frame: u32,
        opacity: f32,
    },
    RemoveOpacityKey {
        layer: usize,
        frame: u32,
    },
    /// Make the layer a matte for the layers right below it (`inside` / `outside`), or a normal
    /// layer again (`None`).
    SetLayerMatte {
        layer: usize,
        #[serde(default)]
        matte: Option<tooncraft_doc::Matte>,
    },
}

fn yes() -> bool {
    true
}

fn unit() -> f32 {
    1.0
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

fn folder_mut<'a>(p: &'a mut tooncraft_doc::Project, name: &str) -> std::result::Result<&'a mut tooncraft_doc::Folder, DocError> {
    p.folders.iter_mut().find(|f| f.name == name).ok_or_else(|| DocError::NoFolder(name.to_string()))
}

/// Run `edit` on the project's folders as one undo step; on error they are restored.
fn folders(e: &mut Engine, label: &str, edit: impl FnOnce(&mut tooncraft_doc::Project) -> std::result::Result<(), DocError>) -> Result<()> {
    let before = e.project.folders.clone();
    match edit(&mut e.project) {
        Ok(()) => {
            e.push(Entry { label: label.into(), parts: vec![Part::Folders(before)] });
            Ok(())
        }
        Err(err) => {
            e.project.folders = before;
            Err(err.into())
        }
    }
}

/// Run `edit` on the project's pegs as one undo step; on error the pegs are restored.
fn pegs(e: &mut Engine, label: &str, edit: impl FnOnce(&mut tooncraft_doc::Project) -> std::result::Result<(), DocError>) -> Result<()> {
    let before = e.project.pegs.clone();
    match edit(&mut e.project) {
        Ok(()) => {
            e.push(Entry { label: label.into(), parts: vec![Part::Pegs(before)] });
            Ok(())
        }
        Err(err) => {
            e.project.pegs = before;
            Err(err.into())
        }
    }
}

fn frame_ok(e: &Engine, frame: u32) -> Result<()> {
    if frame >= e.project.length() { Err(DocError::NoFrame(frame).into()) } else { Ok(()) }
}

fn add_tip(e: &mut Engine, id: u64, tip: tooncraft_brush::Tip) -> Result<()> {
    if e.project.tips.len() >= 256 && !e.project.tips.contains_key(&id) {
        return Err(DocError::TooManyDrawings.into());
    }
    let old = e.project.tips.insert(id, std::sync::Arc::new(tip));
    e.push(Entry { label: "Brush tip".into(), parts: vec![Part::Tip { id, tip: old }] });
    Ok(())
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
        Command::FillAllFrames { layer, x, y, color, options, reference } => e.fill_all(layer, x, y, color, options, reference).map(|_| ()),
        Command::Shape { layer, frame, brush, shape, from, to, constrain } => {
            let samples = crate::shapes::samples(shape, from, to, constrain, 1.0);
            let brush = tooncraft_brush::Brush { smoothing: tooncraft_brush::Smoothing { average: 0.0, string: 0.0, catch_up: true }, ..brush };
            apply(e, Command::Stroke { layer, frame, brush, samples })?;
            if let Some(top) = e.undo.last_mut() {
                top.label = "Shape".into();
            }
            Ok(())
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
            let locked: Vec<bool> = e.project.layers.iter().map(|l| e.project.is_locked(l)).collect();
            for (i, l) in e.project.layers.iter_mut().enumerate() {
                if layer.is_none_or(|t| t == i) && !locked.get(i).copied().unwrap_or(true) {
                    l.insert_frame(frame);
                }
            }
            Ok(())
        }),
        Command::DeleteFrame { layer, frame } => edit_layers(e, "Delete frame", |e| {
            frame_ok(e, frame)?;
            let locked: Vec<bool> = e.project.layers.iter().map(|l| e.project.is_locked(l)).collect();
            for (i, l) in e.project.layers.iter_mut().enumerate() {
                if layer.is_none_or(|t| t == i) && !locked.get(i).copied().unwrap_or(true) {
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
        Command::TransformSelection { layer, frame, polygon, dx, dy, scale_x, scale_y, angle } => {
            crate::select::transform(e, layer, frame, &polygon, (dx, dy, scale_x, scale_y, angle))
        }
        Command::ClearSelection { layer, frame, polygon } => crate::select::clear(e, layer, frame, &polygon),
        Command::PastePixels { layer, frame, x, y, width, height, rgba } => crate::select::paste(e, layer, frame, (x, y), width, height, &rgba),
        Command::SetSound { path, offset } => crate::audio::set(e, path.as_deref(), offset),
        Command::SetCameraKey { frame, x, y, zoom, angle } => {
            frame_ok(e, frame)?;
            let before = Part::Camera(e.project.camera.clone());
            let (w, h) = (e.project.width(), e.project.height());
            e.project.camera.set(tooncraft_doc::CameraKey { frame, x, y, zoom, angle }.sanitized(w, h));
            e.project.camera.enabled = true;
            e.push(Entry { label: "Camera key".into(), parts: vec![before] });
            Ok(())
        }
        Command::RemoveCameraKey { frame } => {
            let before = Part::Camera(e.project.camera.clone());
            if e.project.camera.remove(frame) {
                e.push(Entry { label: "Remove camera key".into(), parts: vec![before] });
            }
            Ok(())
        }
        Command::SetCamera { enabled } => {
            let before = Part::Camera(e.project.camera.clone());
            e.project.camera.enabled = enabled;
            e.push(Entry { label: "Camera".into(), parts: vec![before] });
            Ok(())
        }
        Command::AddPeg { name, parent } => pegs(e, "Add peg", |p| p.add_peg(&name, parent).map(|_| ())),
        Command::DeletePeg { peg } => {
            let parts = vec![Part::Layers(e.project.layers.clone()), Part::Pegs(e.project.pegs.clone())];
            e.project.delete_peg(peg)?;
            e.push(Entry { label: "Delete peg".into(), parts });
            Ok(())
        }
        Command::RenamePeg { peg, name } => pegs(e, "Rename peg", |p| {
            p.peg_mut(peg).ok_or(DocError::NoPeg(peg.0))?.name = name.chars().take(200).collect();
            Ok(())
        }),
        Command::SetPegParent { peg, parent } => pegs(e, "Peg parent", |p| p.set_peg_parent(peg, parent)),
        Command::SetPegPivot { peg, x, y } => pegs(e, "Peg pivot", |p| {
            let f = |v: f32| if v.is_finite() { v.clamp(-1.0e6, 1.0e6) } else { 0.0 };
            p.peg_mut(peg).ok_or(DocError::NoPeg(peg.0))?.pivot = (f(x), f(y));
            Ok(())
        }),
        Command::SetPegKey { peg, frame, x, y, angle, scale_x, scale_y, skew, ease } => {
            frame_ok(e, frame)?;
            pegs(e, "Peg key", |p| {
                p.peg_mut(peg).ok_or(DocError::NoPeg(peg.0))?.set_key(PegKey { frame, x, y, angle, scale_x, scale_y, skew, ease });
                Ok(())
            })
        }
        Command::RemovePegKey { peg, frame } => pegs(e, "Remove peg key", |p| {
            p.peg_mut(peg).ok_or(DocError::NoPeg(peg.0))?.remove_key(frame);
            Ok(())
        }),
        Command::SetLayerPeg { layer, peg } => edit_layers(e, "Layer peg", |e| {
            if let Some(id) = peg
                && e.project.peg(id).is_none()
            {
                return Err(DocError::NoPeg(id.0).into());
            }
            e.project.layer_mut(layer)?.peg = peg;
            Ok(())
        }),
        Command::SetLayerDepth { layer, depth } => edit_layers(e, "Layer depth", |e| {
            let d = if depth.is_finite() { depth.clamp(tooncraft_doc::camera::MIN_DEPTH, tooncraft_doc::camera::MAX_DEPTH) } else { 0.0 };
            e.project.layer_mut(layer)?.depth = d;
            Ok(())
        }),
        Command::AddCameraShake { shake } => {
            if e.project.camera.shakes.len() >= tooncraft_doc::camera::MAX_SHAKES {
                return Err(EngineError::Limit("camera shakes"));
            }
            let before = Part::Camera(e.project.camera.clone());
            e.project.camera.shakes.push(shake.sanitized());
            e.project.camera.enabled = true;
            e.push(Entry { label: "Camera shake".into(), parts: vec![before] });
            Ok(())
        }
        Command::RemoveCameraShake { index } => {
            if index >= e.project.camera.shakes.len() {
                return Err(EngineError::Limit("camera shake index"));
            }
            let before = Part::Camera(e.project.camera.clone());
            e.project.camera.shakes.remove(index);
            e.push(Entry { label: "Remove camera shake".into(), parts: vec![before] });
            Ok(())
        }
        Command::AddTip { id, name, width, height, alpha } => {
            let tip = tooncraft_brush::Tip::new(&name, width, height, alpha).ok_or(tooncraft_raster::RasterError::BadImage { got: 0, want: 1 })?;
            add_tip(e, id, tip)
        }
        Command::AddTipFromImage { id, path } => {
            let tip = crate::preset::tip_from_image(&path).map_err(EngineError::Import)?;
            add_tip(e, id, tip)
        }
        Command::ImportLayers { path, frame } => crate::interchange::layers(e, &path, frame),
        Command::ImportGif { path, frame } => crate::interchange::gif(e, &path, frame),
        Command::ImportSheet { path } => crate::interchange::sheet(e, &path),
        Command::MoveSound { offset, index } => crate::audio::edit(e, index, "Move sound", |c| {
            c.offset = offset;
            true
        }),
        Command::AddSound { path, offset, track } => crate::audio::add(e, &path, offset, track),
        Command::RemoveSound { index } => crate::audio::remove(e, index),
        Command::SetSoundClip { index, track, offset, trim_start, length, to_end, fade_in, fade_out, volume, muted } => {
            crate::audio::edit(e, index, "Sound clip", |c| {
                if let Some(v) = track {
                    c.track = v;
                }
                if let Some(v) = offset {
                    c.offset = v;
                }
                if let Some(v) = trim_start {
                    c.trim_start = v;
                }
                if let Some(v) = length {
                    c.length = Some(v);
                }
                if to_end {
                    c.length = None;
                }
                if let Some(v) = fade_in {
                    c.fade_in = v;
                }
                if let Some(v) = fade_out {
                    c.fade_out = v;
                }
                if let Some(v) = volume {
                    c.volume = v;
                }
                if let Some(v) = muted {
                    c.muted = v;
                }
                true
            })
        }
        Command::SetVolumeKey { index, frame, gain } => crate::audio::edit(e, index, "Volume key", |c| c.set_volume_key(frame, gain)),
        Command::RemoveVolumeKey { index, frame } => crate::audio::edit(e, index, "Remove volume key", |c| {
            let n = c.volume_keys.len();
            c.volume_keys.retain(|k| k.frame != frame);
            c.volume_keys.len() != n
        }),
        Command::SetMouth { layer, mouth, drawing } => edit_layers(e, "Mouth chart", |e| {
            if let Some(d) = drawing
                && e.project.drawing(d).is_none()
            {
                return Err(EngineError::Import(format!("there is no drawing {}", d.0)));
            }
            let l = e.project.layer_mut(layer)?;
            match drawing {
                Some(d) => {
                    l.mouth_chart.insert(mouth, d);
                }
                None => {
                    l.mouth_chart.remove(&mouth);
                }
            }
            Ok(())
        }),
        Command::LipSync { layer, from, settings } => crate::lipsync::detect(e, layer, from, settings).map(|_| ()),
        Command::ImportMouthSheet { layer, path, from } => crate::lipsync::import(e, layer, &path, from).map(|_| ()),
        Command::SetAudioTrack { track, name, muted, solo, volume } => crate::audio::set_track(e, track, |t| {
            if let Some(v) = name {
                t.name = v;
            }
            if let Some(v) = muted {
                t.muted = v;
            }
            if let Some(v) = solo {
                t.solo = v;
            }
            if let Some(v) = volume {
                t.volume = v;
            }
        }),
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
        Command::SetLayerBlend { layer, blend } => edit_layers(e, "Blend mode", |e| {
            e.project.layer_mut(layer)?.blend = blend;
            Ok(())
        }),
        Command::SetLayerColor { layer, color } => edit_layers(e, "Layer colour", |e| {
            e.project.layer_mut(layer)?.color = color;
            Ok(())
        }),
        Command::SetLayerFolder { layer, folder } => {
            let parts = vec![Part::Folders(e.project.folders.clone()), Part::Layers(e.project.layers.clone())];
            let folder = folder.map(|f| f.chars().take(200).collect::<String>()).filter(|f| !f.trim().is_empty());
            e.project.layer_mut(layer)?.folder = folder.clone();
            if let Some(name) = folder
                && !e.project.folders.iter().any(|f| f.name == name)
            {
                if e.project.folders.len() >= 512 {
                    return Err(DocError::TooManyLayers.into());
                }
                e.project.folders.push(tooncraft_doc::Folder::new(&name));
            }
            // drop folders nobody is in (directly or through folders in them)
            e.project.prune_folders();
            e.push(Entry { label: "Layer folder".into(), parts });
            Ok(())
        }
        Command::SetFolder { name, visible, opacity, collapsed } => {
            let before = Part::Folders(e.project.folders.clone());
            let f = folder_mut(&mut e.project, &name)?;
            if let Some(v) = visible {
                f.visible = v;
            }
            if let Some(o) = opacity.filter(|o| o.is_finite()) {
                f.opacity = o.clamp(0.0, 1.0);
            }
            if let Some(c) = collapsed {
                f.collapsed = c;
            }
            e.push(Entry { label: "Folder".into(), parts: vec![before] });
            Ok(())
        }
        Command::SetMark { layer, frame, mark } => {
            frame_ok(e, frame)?;
            let d = e.project.drawing_at(layer, frame).ok_or(EngineError::NoDrawing(frame))?;
            let before = Part::Marks(e.project.marks.clone());
            match mark {
                Some(m) => e.project.marks.insert(d, m),
                None => e.project.marks.remove(&d),
            };
            e.push(Entry { label: "Mark drawing".into(), parts: vec![before] });
            Ok(())
        }
        Command::ApplyTiming { layer, from, to, preset } => edit_layers(e, "Timing preset", |e| {
            e.check_unlocked(layer)?;
            e.project.layer_mut(layer)?.apply_timing(from, to, &preset).map_err(DocError::from)?;
            Ok(())
        }),
        Command::RetimeBetweenKeys { layer, from, to, ease } => edit_layers(e, "Re-time between keys", |e| {
            e.check_unlocked(layer)?;
            let marks = e.project.marks.clone();
            let anchor = |d| marks.get(&d).is_some_and(|m: &Mark| m.is_anchor());
            e.project.layer_mut(layer)?.retime_between_anchors(from, to, anchor, ease).map_err(DocError::from)?;
            Ok(())
        }),
        Command::AddInbetweens { layer, frame, count, ease, guides } => crate::inbetween::add(e, layer, frame, count, ease, guides),
        Command::SetLayerClip { layer, clip } => edit_layers(e, "Clipping", |e| {
            e.project.layer_mut(layer)?.clip = clip;
            Ok(())
        }),
        Command::SetFolderLock { name, locked } => folders(e, "Lock folder", |p| {
            folder_mut(p, &name)?.locked = locked;
            Ok(())
        }),
        Command::SetFolderColor { name, color } => folders(e, "Folder colour", |p| {
            folder_mut(p, &name)?.color = color;
            Ok(())
        }),
        Command::SetFolderParent { name, parent } => folders(e, "Folder parent", |p| {
            folder_mut(p, &name)?;
            if let Some(q) = &parent {
                if p.folder(q).is_none() {
                    return Err(DocError::NoFolder(q.clone()));
                }
                if p.folder_would_loop(&name, q) {
                    return Err(DocError::FolderLoop);
                }
            }
            folder_mut(p, &name)?.parent = parent;
            Ok(())
        }),
        Command::RenameFolder { name, to } => {
            let to: String = to.chars().take(200).collect();
            if to.trim().is_empty() || e.project.folder(&to).is_some() {
                return Err(DocError::FolderName(to).into());
            }
            let parts = vec![Part::Folders(e.project.folders.clone()), Part::Layers(e.project.layers.clone())];
            folder_mut(&mut e.project, &name)?.name = to.clone();
            for f in &mut e.project.folders {
                if f.parent.as_deref() == Some(name.as_str()) {
                    f.parent = Some(to.clone());
                }
            }
            for l in &mut e.project.layers {
                if l.folder.as_deref() == Some(name.as_str()) {
                    l.folder = Some(to.clone());
                }
            }
            e.push(Entry { label: "Rename folder".into(), parts });
            Ok(())
        }
        Command::AddFolder { name, parent, layers } => {
            let name: String = name.chars().take(200).collect();
            if name.trim().is_empty() || e.project.folder(&name).is_some() {
                return Err(DocError::FolderName(name).into());
            }
            if e.project.folders.len() >= tooncraft_doc::MAX_FOLDERS {
                return Err(EngineError::Limit("folders"));
            }
            if let Some(q) = &parent {
                if e.project.folder(q).is_none() {
                    return Err(DocError::NoFolder(q.clone()).into());
                }
                if e.project.folder_depth(q) + 1 >= tooncraft_doc::MAX_FOLDER_DEPTH {
                    return Err(DocError::FolderLoop.into());
                }
            }
            for &l in &layers {
                e.project.layer(l)?;
            }
            let parts = vec![Part::Folders(e.project.folders.clone()), Part::Layers(e.project.layers.clone())];
            e.project.folders.push(tooncraft_doc::Folder { parent, ..tooncraft_doc::Folder::new(&name) });
            for l in layers {
                e.project.layer_mut(l)?.folder = Some(name.clone());
            }
            e.push(Entry { label: "New folder".into(), parts });
            Ok(())
        }
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
            let mut parts = vec![Part::Marks(e.project.marks.clone())];
            let unused = e.project.unused_drawings();
            e.project.marks.retain(|d, _| !unused.contains(d));
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
        Command::AddEffect { layer, effect, index } => crate::effects::add(e, layer, effect, index),
        Command::SetEffect { layer, index, effect } => crate::effects::edit(e, layer, index, "Effect", |fx| {
            *fx = effect.sanitized();
            true
        }),
        Command::EditEffect { layer, index, frame, value } => crate::effects::edit(e, layer, index, "Effect settings", |fx| fx.edit(frame, value)),
        Command::SetEffectKey { layer, index, frame, value } => crate::effects::edit(e, layer, index, "Effect key", |fx| {
            let v = value.unwrap_or_else(|| fx.at(frame));
            fx.set_key(frame, v)
        }),
        Command::RemoveEffectKey { layer, index, frame } => crate::effects::edit(e, layer, index, "Remove effect key", |fx| fx.remove_key(frame)),
        Command::EnableEffect { layer, index, enabled } => crate::effects::edit(e, layer, index, "Effect on / off", |fx| {
            fx.enabled = enabled;
            true
        }),
        Command::RemoveEffect { layer, index } => crate::effects::remove(e, layer, index),
        Command::MoveEffect { layer, from, to } => crate::effects::move_to(e, layer, from, to),
        Command::SetOpacityKey { layer, frame, opacity } => edit_layers(e, "Opacity key", |e| {
            frame_ok(e, frame)?;
            if e.project.layer_mut(layer)?.set_opacity_key(frame, opacity) { Ok(()) } else { Err(EngineError::EffectMismatch) }
        }),
        Command::RemoveOpacityKey { layer, frame } => edit_layers(e, "Remove opacity key", |e| {
            let l = e.project.layer_mut(layer)?;
            let n = l.opacity_keys.len();
            l.opacity_keys.retain(|k| k.frame != frame);
            if l.opacity_keys.len() == n { Err(EngineError::NoEffect(frame as usize)) } else { Ok(()) }
        }),
        Command::SetLayerMatte { layer, matte } => edit_layers(e, "Matte", |e| {
            e.project.layer_mut(layer)?.matte = matte;
            Ok(())
        }),
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
