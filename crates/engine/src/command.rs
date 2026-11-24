//! The command set: every change to a project, as data (JSON for the CLI and agents).

use serde::{Deserialize, Serialize};
use tooncraft_brush::{Brush, Sample};
use tooncraft_doc::{Cell, Channel, DocError, Ease, Mark, OnionSkin, PegId, PegKey, Pin, Shake, TimingPreset};
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
    /// A polyline, polygon or smooth curve through `points`: outlined with the brush, filled
    /// with straight RGBA `fill`, or both (lasso fill is `outline: false` with a fill).
    PathShape {
        layer: usize,
        frame: u32,
        brush: Brush,
        points: Vec<(f32, f32)>,
        #[serde(default)]
        closed: bool,
        #[serde(default)]
        curve: bool,
        #[serde(default = "yes")]
        outline: bool,
        #[serde(default)]
        fill: Option<[u8; 4]>,
    },
    /// A linear or radial gradient from `from` to `to`, inside `area` when given.
    Gradient {
        layer: usize,
        frame: u32,
        #[serde(default)]
        kind: crate::drawing::GradientKind,
        from: (f32, f32),
        to: (f32, f32),
        start: [u8; 4],
        end: [u8; 4],
        #[serde(default)]
        area: Option<Vec<(f32, f32)>>,
        /// More rings of the selection (holes, other parts), with `area`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        area_rings: Vec<Vec<(f32, f32)>>,
        /// A gradient swatch's stops; two or more replace `start` and `end`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        stops: Vec<tooncraft_doc::palette::Stop>,
    },
    /// Give a layer its own onion-skin settings (`None`: back to the shot's). A setting, not an
    /// undo step.
    SetLayerOnion {
        layer: usize,
        #[serde(default)]
        onion: Option<OnionSkin>,
    },
    /// Replace the shot's named colour palettes (a setting, not an undo step).
    SetPalettes {
        palettes: Vec<tooncraft_doc::palette::Palette>,
    },
    /// Change a swatch of the shot's palette `palette` (0-based): its colour, and its name when
    /// given. `update_paint` (default true): paint in the old colour on every unlocked layer takes
    /// the new one, opacity kept. One undo step.
    EditSwatch {
        palette: usize,
        swatch: usize,
        rgb: [u8; 3],
        #[serde(default)]
        name: Option<String>,
        #[serde(default = "yes")]
        update_paint: bool,
    },
    /// Save the layer's cells `from..=to` (0-based frames, either order) as a drawing-library item
    /// called `name`, with its own copies of the drawings. One undo step.
    SaveToLibrary {
        layer: usize,
        from: u32,
        to: u32,
        name: String,
    },
    /// Put library item `item` on the layer from `frame` on, over the cells there (cut at the end of
    /// the shot). `linked`: the layer shows the item's own drawings, so drawing on one changes the
    /// item and every linked use; otherwise the layer gets new copies. One undo step.
    InsertFromLibrary {
        item: usize,
        layer: usize,
        frame: u32,
        #[serde(default)]
        linked: bool,
    },
    RenameLibraryItem {
        item: usize,
        name: String,
    },
    /// Remove a library item (its drawings go with Purge Unused unless a linked layer shows them).
    RemoveLibraryItem {
        item: usize,
    },
    MoveLibraryItem {
        item: usize,
        to: usize,
    },
    /// Write library item `item` to `path` as a drawing file (`.toondrawings`) to use in other
    /// shots. Not an undo step.
    SaveLibraryFile {
        item: usize,
        path: String,
    },
    /// Add the drawing file at `path` to the library; drawings made for another canvas size are
    /// centred as `placement` says (default: fit). One undo step.
    OpenLibraryFile {
        path: String,
        #[serde(default)]
        placement: crate::import::Placement,
    },
    /// In every drawing of the layer, give the pixels close to `from` (within `tolerance` per
    /// channel) the colour `to`, keeping their opacity (antialiased edges stay soft). One undo
    /// step.
    RecolourLayer {
        layer: usize,
        from: [u8; 3],
        to: [u8; 3],
        #[serde(default = "recolour_tolerance")]
        tolerance: u8,
    },
    /// Symmetry drawing for the following strokes (a setting, not an undo step).
    SetSymmetry {
        symmetry: tooncraft_doc::symmetry::Symmetry,
    },
    /// Stroke rulers and grid snapping for the following strokes (a setting, not an undo step).
    SetRulers {
        rulers: tooncraft_doc::rulers::Rulers,
    },
    /// Paint text into the drawing (one undo step): `x`, `y` is the anchor at the top of the
    /// first line (its left edge, centre or right edge by the alignment).
    Text {
        layer: usize,
        frame: u32,
        text: String,
        x: f32,
        y: f32,
        #[serde(default)]
        style: crate::text::TextStyle,
    },
    /// Add a colour layer just below `layer` that follows its exposures (line and colour art in
    /// one drawing). One undo step.
    AddColourLayer {
        layer: usize,
    },
    /// Make `layer` follow the line layer `line`'s exposures, or stop following (`None`).
    SetColourOf {
        layer: usize,
        #[serde(default)]
        line: Option<usize>,
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
    /// Show another drawing (or nothing) for the whole exposure at the frame.
    SwapDrawing {
        layer: usize,
        frame: u32,
        #[serde(default)]
        drawing: Option<tooncraft_doc::DrawingId>,
    },
    /// Hold each drawing of the layer over the empty frames after it.
    FillEmpty {
        layer: usize,
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
    /// Images (PNG, JPEG, WebP, TIFF …) as drawings, one per `hold` frames from `frame`: on a new
    /// layer, one layer per file name, or an existing layer, placed as `options` says (by default
    /// centred and scaled down to fit the canvas).
    ImportImages {
        paths: Vec<String>,
        frame: u32,
        #[serde(default = "one_frame")]
        hold: u32,
        name: String,
        #[serde(default)]
        options: crate::import::ImportOptions,
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
    /// Add a peg and hang `layer` from it, as one undo step.
    AddPegForLayer {
        layer: usize,
        name: String,
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
    /// Move several keys of one peg together (the curve editor's drag): by `by_frames` in time
    /// and, with a `channel`, by `by_value` in that value. Refused whole when a key would leave
    /// the shot or land on a key that is not moving.
    MovePegKeys {
        peg: PegId,
        frames: Vec<u32>,
        #[serde(default)]
        by_frames: i64,
        #[serde(default)]
        channel: Option<Channel>,
        #[serde(default)]
        by_value: f32,
    },
    /// Give several keys of one peg the same ease towards their next key.
    SetPegEases {
        peg: PegId,
        frames: Vec<u32>,
        ease: Ease,
    },
    /// Remove several keys of one peg.
    RemovePegKeys {
        peg: PegId,
        frames: Vec<u32>,
    },
    /// Hang a layer from a peg, or take it off (`None`). Its pixels do not change.
    SetLayerPeg {
        layer: usize,
        #[serde(default)]
        peg: Option<PegId>,
    },
    /// Keep the strokes drawn on a layer as editable vector lines (or not, for new drawings).
    SetLayerVector {
        layer: usize,
        vector: bool,
    },
    /// Add a vector stroke through `points` (x, y, width) to the drawing at a vector layer's
    /// frame (a new drawing on a blank cell); points closer than `tolerance` px to the line are
    /// dropped.
    VectorStroke {
        layer: usize,
        frame: u32,
        color: [u8; 4],
        points: Vec<tooncraft_vector::VPoint>,
        #[serde(default = "vector_tolerance")]
        tolerance: f32,
    },
    /// Change one stroke of a vector drawing: move a point, its width, smooth, erase to the
    /// crossings, delete, recolour.
    EditVector {
        layer: usize,
        frame: u32,
        edit: crate::vector::VectorEdit,
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
    /// Add a grain image from an image file (light is a peak, dark a valley; scaled down to
    /// 1024 px at most) under `id`. Brushes use it with `grain_kind: "image"` and `grain_image: id`.
    AddGrainFromImage {
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
    /// A BoardCraft board or animatic (`.boardcraft`) as the layout, from `frame`: every panel a
    /// drawing on a locked reference layer at the bottom, held for its duration and marked as a
    /// key, scaled to fit the canvas; with `camera`, the panels' camera keys become camera keys.
    /// The shot grows to fit the board.
    ImportBoards {
        path: String,
        #[serde(default)]
        frame: u32,
        #[serde(default = "yes")]
        camera: bool,
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
    /// Cut a sound clip in two at a shot frame.
    SplitSound {
        index: usize,
        frame: i32,
    },
    /// A copy of a sound clip starting at a shot frame.
    DuplicateSound {
        index: usize,
        offset: i32,
    },
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
        /// More rings of the selection (holes, other parts); even-odd with `polygon`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        rings: Vec<Vec<(f32, f32)>>,
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
        /// Skew along x and y (radians), applied before the scale and turn.
        #[serde(default)]
        skew_x: f32,
        #[serde(default)]
        skew_y: f32,
        #[serde(default)]
        resample: tooncraft_raster::select::Resample,
    },
    /// Distort the pixels inside `polygon` so the corners of its box land on `corners`
    /// (top-left, top-right, bottom-right, bottom-left): free distort and perspective.
    DistortSelection {
        layer: usize,
        frame: u32,
        polygon: Vec<(f32, f32)>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        rings: Vec<Vec<(f32, f32)>>,
        corners: [(f32, f32); 4],
        #[serde(default)]
        resample: tooncraft_raster::select::Resample,
    },
    /// Erase the pixels inside `polygon`.
    ClearSelection {
        layer: usize,
        frame: u32,
        polygon: Vec<(f32, f32)>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        rings: Vec<Vec<(f32, f32)>>,
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
    /// Lock a layer's transparency: strokes only recolour paint that is already there.
    SetLayerLockAlpha {
        layer: usize,
        lock: bool,
    },
    /// A copy of the layer (with its own drawings) just above it.
    DuplicateLayer {
        layer: usize,
    },
    /// Merge the layer into the one below it (blend mode, opacity and effects baked in).
    MergeDown {
        layer: usize,
    },
    /// Merge every visible layer into one.
    MergeVisible,
    /// Merge every visible layer into one and remove the hidden ones.
    Flatten,
    /// Copy opacity (and keys), blend mode, label colour, folder, peg, depth and effects from
    /// one layer to another.
    CopyLayerProperties {
        from: usize,
        to: usize,
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
    /// Change the canvas size; `anchor` (0…1 each way, default the centre) says which part of
    /// the picture stays put. Drawings are moved, never resampled.
    ResizeCanvas {
        width: u32,
        height: u32,
        #[serde(default = "centre")]
        anchor: (f32, f32),
    },
    /// Crop the canvas to the rectangle at (`x`, `y`) of `width`×`height` pixels.
    CropCanvas {
        x: i64,
        y: i64,
        width: u32,
        height: u32,
    },
    /// Mark the drawing at the frame as a key, breakdown or inbetween (`None`: no mark).
    SetMark {
        layer: usize,
        frame: u32,
        #[serde(default)]
        mark: Option<Mark>,
    },
    /// Name the drawing shown at the frame, so it can be exposed again by name (an empty name
    /// removes it).
    SetDrawingName {
        layer: usize,
        frame: u32,
        name: String,
    },
    /// Show the drawing called `name` at the frame (a substitution: only this frame changes),
    /// on any layer.
    ExposeNamed {
        layer: usize,
        frame: u32,
        name: String,
    },
    /// Put a note on a frame (an empty `text` removes the marker).
    SetMarker {
        frame: u32,
        text: String,
    },
    /// Remove the marker on a frame.
    RemoveMarker {
        frame: u32,
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
    /// Append several effects (a preset or a saved stack) as one undo step.
    AddEffects {
        layer: usize,
        effects: Vec<tooncraft_fx::Effect>,
    },
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
    /// Several commands as one undo step called `label` (at most [`MAX_BATCH`], none of them a
    /// batch). When one fails, the ones before it are undone and the error is returned, so the
    /// project is as it was.
    Batch {
        label: String,
        commands: Vec<Command>,
    },
}

/// Most commands in one [`Command::Batch`].
pub const MAX_BATCH: usize = 256;

fn centre() -> (f32, f32) {
    (0.5, 0.5)
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

fn reframe(e: &mut Engine, label: &str, width: u32, height: u32, dx: i64, dy: i64) -> Result<()> {
    if e.is_stroking() {
        return Err(EngineError::Busy);
    }
    let before = Box::new(e.project.clone());
    e.project.reframe(width, height, dx, dy)?;
    e.push(Entry { label: label.into(), parts: vec![Part::Whole(before)] });
    Ok(())
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

fn vector_tolerance() -> f32 {
    0.6
}

fn recolour_tolerance() -> u8 {
    24
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
        Command::PathShape { layer, frame, brush, points, closed, curve, outline, fill } => {
            crate::drawing::path_shape(e, layer, frame, brush, &points, closed, curve, outline, fill)
        }
        Command::Gradient { layer, frame, kind, from, to, start, end, area, area_rings, stops } => {
            frame_ok(e, frame)?;
            let stops = tooncraft_doc::palette::sanitize_stops(stops);
            let stops = if stops.is_empty() {
                vec![tooncraft_doc::palette::Stop { at: 0.0, rgba: start }, tooncraft_doc::palette::Stop { at: 1.0, rgba: end }]
            } else {
                stops
            };
            crate::drawing::gradient(e, layer, frame, kind, from, to, &stops, area.as_deref(), &area_rings)
        }
        Command::SetLayerOnion { layer, onion } => {
            e.project.layer_mut(layer)?.onion = onion.map(OnionSkin::sanitized);
            e.touch();
            Ok(())
        }
        Command::SetPalettes { palettes } => {
            e.project.palettes = tooncraft_doc::palette::sanitize_all(palettes);
            e.touch();
            Ok(())
        }
        Command::EditSwatch { palette, swatch, rgb, name, update_paint } => crate::drawing::edit_swatch(e, palette, swatch, rgb, name, update_paint),
        Command::SaveToLibrary { layer, from, to, name } => crate::drawing_library::save(e, layer, from, to, &name),
        Command::InsertFromLibrary { item, layer, frame, linked } => crate::drawing_library::insert(e, item, layer, frame, linked),
        Command::RenameLibraryItem { item, name } => crate::drawing_library::rename(e, item, &name),
        Command::RemoveLibraryItem { item } => crate::drawing_library::remove(e, item),
        Command::MoveLibraryItem { item, to } => crate::drawing_library::move_item(e, item, to),
        Command::SaveLibraryFile { item, path } => crate::drawing_library::save_file(e, item, &path),
        Command::OpenLibraryFile { path, placement } => crate::drawing_library::open_file(e, &path, placement),
        Command::RecolourLayer { layer, from, to, tolerance } => crate::drawing::recolour(e, layer, from, to, tolerance),
        Command::SetSymmetry { symmetry } => {
            e.project.symmetry = symmetry.sanitized();
            e.touch();
            Ok(())
        }
        Command::SetRulers { rulers } => {
            e.project.rulers = rulers.sanitized();
            e.touch();
            Ok(())
        }
        Command::Text { layer, frame, text, x, y, style } => {
            frame_ok(e, frame)?;
            crate::text::paint_text(e, layer, frame, &text, (x, y), &style)
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
            let mut parts = vec![Part::Drawing { id, surface: None }, before];
            // a vector drawing's copy keeps its lines
            if let Some(v) = e.project.vectors.get(&src).cloned() {
                e.project.vectors.insert(id, v);
                parts.push(Part::Vector { id, strokes: None });
            }
            e.push(Entry { label: "Duplicate drawing".into(), parts });
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
        Command::SwapDrawing { layer, frame, drawing } => edit_layers(e, "Swap drawing", |e| {
            e.check_unlocked(layer)?;
            frame_ok(e, frame)?;
            if let Some(d) = drawing
                && e.project.drawing(d).is_none()
            {
                return Err(DocError::NoDrawingId(d.0).into());
            }
            e.project.layer_mut(layer)?.swap(frame, drawing);
            Ok(())
        }),
        Command::FillEmpty { layer } => edit_layers(e, "Fill empty frames", |e| {
            e.check_unlocked(layer)?;
            e.project.layer_mut(layer)?.fill_empty();
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
        Command::ImportImages { paths, frame, hold, name, options } => crate::import::images(e, &paths, frame, hold.clamp(1, 64), &name, &options),
        Command::TransformSelection { layer, frame, polygon, rings, dx, dy, scale_x, scale_y, angle, skew_x, skew_y, resample } => {
            crate::select::transform(e, layer, frame, &polygon, &rings, (dx, dy, scale_x, scale_y, angle), (skew_x, skew_y), resample)
        }
        Command::DistortSelection { layer, frame, polygon, rings, corners, resample } => {
            crate::select::distort(e, layer, frame, &polygon, &rings, corners, resample)
        }
        Command::ClearSelection { layer, frame, polygon, rings } => crate::select::clear(e, layer, frame, &polygon, &rings),
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
        Command::AddPegForLayer { layer, name } => {
            e.project.layer(layer)?;
            let parts = vec![Part::Layers(e.project.layers.clone()), Part::Pegs(e.project.pegs.clone())];
            let id = e.project.add_peg(&name, None)?;
            e.project.layer_mut(layer)?.peg = Some(id);
            e.push(Entry { label: "Add peg for layer".into(), parts });
            Ok(())
        }
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
        Command::MovePegKeys { peg, frames, by_frames, channel, by_value } => {
            let last = e.project.length().saturating_sub(1);
            let label = if by_frames == 0 && channel.is_some() { "Change key values" } else { "Move keys" };
            pegs(e, label, |p| {
                p.peg_mut(peg).ok_or(DocError::NoPeg(peg.0))?.move_keys(&frames, by_frames, channel, by_value, last)?;
                Ok(())
            })
        }
        Command::SetPegEases { peg, frames, ease } => pegs(e, "Ease keys", |p| {
            p.peg_mut(peg).ok_or(DocError::NoPeg(peg.0))?.set_eases(&frames, ease);
            Ok(())
        }),
        Command::RemovePegKeys { peg, frames } => pegs(e, "Remove peg keys", |p| {
            let pg = p.peg_mut(peg).ok_or(DocError::NoPeg(peg.0))?;
            for f in frames.iter().take(tooncraft_doc::peg::MAX_PEG_KEYS) {
                pg.remove_key(*f);
            }
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
        Command::SetLayerVector { layer, vector } => edit_layers(e, "Vector layer", |e| {
            e.project.layer_mut(layer)?.vector = vector;
            Ok(())
        }),
        Command::VectorStroke { layer, frame, color, points, tolerance } => crate::vector::add_stroke(e, layer, frame, color, &points, tolerance),
        Command::EditVector { layer, frame, edit } => crate::vector::edit(e, layer, frame, edit),
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
        Command::AddGrainFromImage { id, path } => {
            let tip = crate::preset::grain_from_image(&path).map_err(EngineError::Import)?;
            add_tip(e, id, tip)
        }
        Command::AddTipFromImage { id, path } => {
            let tip = crate::preset::tip_from_image(&path).map_err(EngineError::Import)?;
            add_tip(e, id, tip)
        }
        Command::ImportLayers { path, frame } => crate::interchange::layers(e, &path, frame),
        Command::ImportGif { path, frame } => crate::interchange::gif(e, &path, frame),
        Command::ImportSheet { path } => crate::interchange::sheet(e, &path),
        Command::ImportBoards { path, frame, camera } => crate::boards::import(e, &path, frame, camera),
        Command::MoveSound { offset, index } => crate::audio::edit(e, index, "Move sound", |c| {
            c.offset = offset;
            true
        }),
        Command::AddSound { path, offset, track } => crate::audio::add(e, &path, offset, track),
        Command::RemoveSound { index } => crate::audio::remove(e, index),
        Command::SplitSound { index, frame } => crate::audio::split(e, index, frame),
        Command::DuplicateSound { index, offset } => crate::audio::duplicate(e, index, offset),
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
        Command::AddColourLayer { layer } => edit_layers(e, "Add colour layer", |e| {
            let l = e.project.layer(layer)?;
            if l.colour_of.is_some() {
                return Err(EngineError::Import("a colour layer cannot have a colour layer of its own".into()));
            }
            let (id, name) = (l.id, format!("{} colour", l.name));
            let at = e.project.add_layer(&name, layer)?;
            e.project.layer_mut(at)?.colour_of = Some(id);
            Ok(())
        }),
        Command::SetColourOf { layer, line } => edit_layers(e, "Link colour layer", |e| {
            let id = match line {
                Some(li) if li == layer => return Err(EngineError::Import("a layer cannot colour itself".into())),
                Some(li) => {
                    let l = e.project.layer(li)?;
                    if l.colour_of.is_some() {
                        return Err(EngineError::Import("the line layer is itself a colour layer".into()));
                    }
                    Some(l.id)
                }
                None => None,
            };
            let c = e.project.layer_mut(layer)?;
            c.colour_of = id;
            if id.is_some() {
                // drawings already on the layer are not the colour of any line drawing
                c.colour_pairs.clear();
            }
            Ok(())
        }),
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
        Command::SetLayerLockAlpha { layer, lock } => edit_layers(e, "Lock transparency", |e| {
            e.project.layer_mut(layer)?.lock_alpha = lock;
            Ok(())
        }),
        Command::DuplicateLayer { layer } => crate::layerops::duplicate(e, layer),
        Command::MergeDown { layer } => crate::layerops::merge(e, crate::layerops::Merge::Down(layer)),
        Command::MergeVisible => crate::layerops::merge(e, crate::layerops::Merge::Visible),
        Command::Flatten => crate::layerops::merge(e, crate::layerops::Merge::Flatten),
        Command::CopyLayerProperties { from, to } => edit_layers(e, "Copy layer properties", |e| crate::layerops::copy_properties(e, from, to)),
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
        Command::ResizeCanvas { width, height, anchor } => {
            let a = |v: f32| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.5 };
            let dx = ((width as f64 - e.project.width() as f64) * a(anchor.0) as f64).round() as i64;
            let dy = ((height as f64 - e.project.height() as f64) * a(anchor.1) as f64).round() as i64;
            reframe(e, "Canvas size", width, height, dx, dy)
        }
        Command::CropCanvas { x, y, width, height } => reframe(e, "Crop canvas", width, height, -x, -y),
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
        Command::SetDrawingName { layer, frame, name } => {
            frame_ok(e, frame)?;
            let d = e.project.drawing_at(layer, frame).ok_or(EngineError::NoDrawing(frame))?;
            let before = Part::Names(e.project.drawing_names.clone());
            e.project.set_drawing_name(d, &name)?;
            e.push(Entry { label: "Name drawing".into(), parts: vec![before] });
            Ok(())
        }
        Command::ExposeNamed { layer, frame, name } => edit_layers(e, "Expose drawing", |e| {
            e.check_unlocked(layer)?;
            frame_ok(e, frame)?;
            let id = e.project.drawing_by_name(&name).ok_or_else(|| DocError::NoDrawingNamed(name.clone()))?;
            let len = e.project.length();
            let l = e.project.layer_mut(layer)?;
            let next_is_hold = frame + 1 < len && matches!(l.cells.get(frame as usize + 1), Some(Cell::Hold));
            let old = l.drawing_at(frame);
            l.set_key(frame, id);
            // only this frame changes: the rest of the hold it cut keeps showing the old drawing
            if next_is_hold && let Some(old) = old.filter(|o| *o != id) {
                l.set_key(frame + 1, old);
            }
            Ok(())
        }),
        Command::SetMarker { frame, text } => {
            let before = Part::Markers(e.project.markers.clone());
            e.project.set_marker(frame, &text)?; // refuses before changing anything
            e.push(Entry { label: "Marker".into(), parts: vec![before] });
            Ok(())
        }
        Command::RemoveMarker { frame } => {
            if !e.project.markers.contains_key(&frame) {
                return Ok(());
            }
            let before = Part::Markers(e.project.markers.clone());
            e.project.markers.remove(&frame);
            e.push(Entry { label: "Remove marker".into(), parts: vec![before] });
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
                // its vector lines go with it
                if let Some(v) = e.project.vectors.remove(&id) {
                    parts.push(Part::Vector { id, strokes: Some(v) });
                }
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
        Command::AddEffects { layer, effects } => crate::effects::add_all(e, layer, effects),
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
        Command::Batch { label, commands } => batch(e, &label, commands),
    }
}

/// Apply `commands` as one undo step; on an error, undo the ones applied and return it.
fn batch(e: &mut Engine, label: &str, commands: Vec<Command>) -> Result<()> {
    if commands.len() > MAX_BATCH {
        return Err(EngineError::Import(format!("a batch holds at most {MAX_BATCH} commands")));
    }
    if commands.iter().any(|c| matches!(c, Command::Batch { .. })) {
        return Err(EngineError::Import("a batch cannot hold another batch".into()));
    }
    let before = e.undo.len();
    // kept aside: a batch that changes nothing, or fails, leaves the redo steps as they were
    let redo = std::mem::take(&mut e.redo);
    for c in commands {
        if let Err(err) = apply(e, c) {
            let n = e.undo.len().saturating_sub(before);
            if n > 0 {
                crate::drawing::merge_last(e, n, label);
                e.undo()?;
            }
            e.redo = redo;
            return Err(err);
        }
    }
    let n = e.undo.len().saturating_sub(before);
    if n > 0 {
        crate::drawing::merge_last(e, n, label);
    } else {
        e.redo = redo;
    }
    Ok(())
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
