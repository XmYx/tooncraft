//! Application state, menus, shortcuts, files, autosave and crash recovery.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use tooncraft_brush::{Brush, BrushKind};
use tooncraft_doc::Project;
use tooncraft_engine::playback::Playback;
use tooncraft_engine::{Command, Engine, FillReference};
use tooncraft_raster::fill::FillOptions;

use crate::args::Cli;
use crate::dock::{Pane, Workspace};
use crate::textures::TileTextures;
use crate::view::View;

/// Canvas tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Pencil,
    Ink,
    Airbrush,
    Chalk,
    Eraser,
    Fill,
    Picker,
    Select,
    Shape,
    Hand,
}

/// How the select tool draws a new selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectMode {
    Lasso,
    Rectangle,
}

/// A transform being dragged: the selection before it and what kind of drag it is.
#[derive(Clone, Debug, PartialEq)]
pub struct Xform {
    pub base: Vec<(f32, f32)>,
    pub kind: XformKind,
    pub start: (f32, f32),
    pub applied: bool,
    /// The current numbers: (dx, dy, scale, angle).
    pub now: (f32, f32, f32, f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XformKind {
    Move,
    Scale,
    Rotate,
}

impl Tool {
    pub const ALL: [Tool; 10] =
        [Tool::Pencil, Tool::Ink, Tool::Airbrush, Tool::Chalk, Tool::Eraser, Tool::Fill, Tool::Picker, Tool::Select, Tool::Shape, Tool::Hand];

    pub fn brush(self) -> Option<BrushKind> {
        Some(match self {
            Tool::Pencil => BrushKind::Pencil,
            Tool::Ink => BrushKind::Ink,
            Tool::Airbrush => BrushKind::Airbrush,
            Tool::Chalk => BrushKind::Chalk,
            Tool::Eraser => BrushKind::Eraser,
            _ => return None,
        })
    }

    pub fn label(self) -> String {
        match self {
            Tool::Pencil => tl!("Pencil"),
            Tool::Ink => tl!("Ink pen"),
            Tool::Airbrush => tl!("Airbrush"),
            Tool::Chalk => tl!("Chalk"),
            Tool::Eraser => tl!("Eraser"),
            Tool::Fill => tl!("Paint bucket"),
            Tool::Picker => tl!("Colour picker"),
            Tool::Select => tl!("Select"),
            Tool::Shape => tl!("Shapes"),
            Tool::Hand => tl!("Hand"),
        }
        .to_string()
    }

    /// Command id suffix (`tool.<id>`) in the shortcut catalog.
    pub fn id(self) -> &'static str {
        match self {
            Tool::Pencil => "pencil",
            Tool::Ink => "ink",
            Tool::Airbrush => "airbrush",
            Tool::Chalk => "chalk",
            Tool::Eraser => "eraser",
            Tool::Fill => "fill",
            Tool::Picker => "picker",
            Tool::Select => "select",
            Tool::Shape => "shape",
            Tool::Hand => "hand",
        }
    }
}

fn brush_index(k: BrushKind) -> usize {
    BrushKind::ALL.iter().position(|b| *b == k).unwrap_or(0)
}

/// Timing statistics shown in the status bar and printed by `--bench-playback`.
#[derive(Default)]
pub struct Stats {
    /// CPU time of the last UI frame, ms.
    pub frame_ms: f64,
    /// Last time from the first pen sample of a frame to its tiles being uploaded, ms.
    pub stroke_ms: f64,
    /// Animation frames shown in the last second of playback.
    pub playback_fps: f64,
    shown: Vec<f64>,
    last_shown: Option<u32>,
    pub frame_times: Vec<f64>,
    pub dropped: u64,
    /// Animation frames shown since playback started.
    pub shown_total: u64,
    /// Wall-clock time between UI frames during playback, ms.
    pub intervals: Vec<f64>,
    last_frame_at: Option<f64>,
}

/// A recovery copy found at start-up, offered to the user.
pub struct RecoveryOffer {
    pub path: PathBuf,
    pub when: String,
}

/// The "New shot" dialog's fields.
#[derive(Clone)]
pub struct NewShot {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub frames: u32,
}

impl Default for NewShot {
    fn default() -> Self {
        Self { name: tl!("Shot").to_string(), width: 1920, height: 1080, fps: 24, frames: 48 }
    }
}

/// A pen moved by the keyboard (canvas focused): arrows move, Enter puts it down / lifts it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyboardPen {
    pub x: f32,
    pub y: f32,
    pub down: bool,
}

pub struct ToonApp {
    pub engine: Engine,
    pub path: Option<PathBuf>,
    pub frame: u32,
    pub layer: usize,
    pub tool: Tool,
    pub brushes: [Brush; 5],
    /// Straight RGBA drawing colour.
    pub color: [u8; 4],
    pub swatches: Vec<[u8; 3]>,
    pub fill: FillOptions,
    pub fill_reference: FillReference,
    /// Pressure used for a mouse (no pen).
    pub mouse_pressure: f32,
    pub view: View,
    pub view_fitted: bool,
    pub playback: Playback,
    pub clock: Instant,
    pub textures: TileTextures,
    pub workspace: Workspace,
    pub status: String,
    pub stats: Stats,
    pub keyboard_pen: Option<KeyboardPen>,
    pub stroke_start: Option<Instant>,
    pub light_theme: bool,
    pub renaming: Option<(usize, String)>,
    pub new_shot: Option<NewShot>,
    pub show_shortcuts: bool,
    pub show_about: bool,
    pub confirm_close: bool,
    close_confirmed: bool,
    pub recovery: Option<RecoveryOffer>,
    autosave_dir: Option<PathBuf>,
    autosave_busy: Arc<AtomicBool>,
    autosave_at: Instant,
    autosaved_changes: u64,
    screenshot: Option<PathBuf>,
    frames_until_shot: u32,
    shot_done: Arc<AtomicBool>,
    bench: Option<(f64, f64)>,
    bench_done: bool,
    pub canvas_focus_request: bool,
    /// Timeline scroll offset (frames, layers).
    pub timeline_scroll: egui::Vec2,
    /// The other corner of the exposure-sheet selection (layer, frame); the current cell is the
    /// first corner. `None`: just the current cell.
    pub sel_anchor: Option<(usize, u32)>,
    /// Copied cells, one row per layer (bottom first).
    pub cell_clipboard: Option<Vec<Vec<tooncraft_doc::Cell>>>,
    pub tablet: crate::tablet::Tablet,
    /// The canvas selection (canvas pixels), on the current drawing.
    pub selection: Option<Vec<(f32, f32)>>,
    pub select_mode: SelectMode,
    /// Points of a selection being drawn.
    pub selecting: Vec<(f32, f32)>,
    pub xform: Option<Xform>,
    /// Copied pixels: straight RGBA, size, and where they came from.
    pub pixel_clipboard: Option<(Vec<u8>, u32, u32, (f32, f32))>,
    pub sound: crate::sound::Sound,
    pub keys: crate::keys::Keys,
    /// Show the camera frame on the canvas.
    pub show_camera: bool,
    pub peg_ui: crate::pegs::PegUi,
    pub shape: tooncraft_engine::shapes::ShapeKind,
    /// A shape being dragged: (from, to) in canvas pixels.
    pub shape_drag: Option<((f32, f32), (f32, f32))>,
    /// Paint-bucket clicks fill the point in every drawing of the layer.
    pub fill_every_frame: bool,
    /// Bigger timeline cells with drawing thumbnails.
    pub thumbnails: bool,
    pub thumbs: std::collections::HashMap<tooncraft_doc::DrawingId, (u64, egui::TextureHandle)>,
    /// CPU composite used while a layer with a blend mode is visible: (key, surface).
    pub blend_cache: Option<(u64, tooncraft_raster::Surface)>,
    last_frame: u32,
    /// Where the last stroke ended (canvas pixels), for Shift+click straight lines.
    pub last_stroke_end: Option<(f32, f32)>,
}

/// Where recovery copies go: `TOONCRAFT_AUTOSAVE_DIR`, else the platform's per-user state folder.
pub fn autosave_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("TOONCRAFT_AUTOSAVE_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    let home = |v: &str| std::env::var_os(v).filter(|d| !d.is_empty()).map(PathBuf::from);
    if cfg!(target_os = "windows") {
        home("LOCALAPPDATA").map(|d| d.join("ToonCraft").join("Autosave"))
    } else if cfg!(target_os = "macos") {
        home("HOME").map(|d| d.join("Library/Application Support/ToonCraft/Autosave"))
    } else {
        home("XDG_STATE_HOME").or_else(|| home("HOME").map(|h| h.join(".local/state"))).map(|d| d.join("tooncraft/autosave"))
    }
}

const AUTOSAVE_SECONDS: u64 = 45;

impl ToonApp {
    pub fn new(cc: &eframe::CreationContext<'_>, cli: Cli) -> Self {
        Self::with_context(&cc.egui_ctx, cli)
    }

    /// Build the app on a context (also used by the UI tests).
    pub fn with_context(ctx: &egui::Context, cli: Cli) -> Self {
        crate::i18n::set_language(None);
        crate::i18n::install_fonts(ctx);
        crate::theme::apply(ctx, cli.light);
        let mut status = String::new();
        let (engine, path) = if cli.demo {
            match tooncraft_engine::demo::demo(1280, 720) {
                Ok(e) => (e, None),
                Err(e) => {
                    status = e.to_string();
                    (Engine::new(blank_project()), None)
                }
            }
        } else if let Some(p) = &cli.file {
            match open_checked(p) {
                Ok(e) => (e, Some(p.clone())),
                Err(e) => {
                    status = e;
                    (Engine::new(blank_project()), None)
                }
            }
        } else {
            (Engine::new(blank_project()), None)
        };
        let brushes = BrushKind::ALL.map(Brush::preset);
        let mut app = Self {
            frame: cli.frame.unwrap_or(0).min(engine.project.length().saturating_sub(1)),
            layer: engine.project.layers.len().saturating_sub(1),
            engine,
            path,
            tool: Tool::Pencil,
            brushes,
            color: [28, 26, 40, 255],
            swatches: vec![
                [28, 26, 40],
                [90, 86, 110],
                [246, 243, 236],
                [232, 92, 70],
                [242, 168, 60],
                [246, 214, 92],
                [96, 176, 104],
                [64, 148, 200],
                [52, 84, 168],
                [142, 92, 176],
                [214, 128, 160],
                [140, 104, 76],
            ],
            fill: FillOptions::default(),
            fill_reference: FillReference::AllLayers,
            mouse_pressure: 0.75,
            view: View::default(),
            view_fitted: false,
            playback: Playback::default(),
            clock: Instant::now(),
            textures: TileTextures::default(),
            workspace: Workspace::default(),
            status,
            stats: Stats::default(),
            keyboard_pen: None,
            stroke_start: None,
            light_theme: cli.light,
            renaming: None,
            new_shot: None,
            show_shortcuts: false,
            show_about: false,
            confirm_close: false,
            close_confirmed: false,
            recovery: None,
            autosave_dir: if cli.screenshot.is_some() || cli.bench_playback.is_some() { None } else { autosave_dir() },
            autosave_busy: Arc::new(AtomicBool::new(false)),
            autosave_at: Instant::now(),
            autosaved_changes: 0,
            screenshot: cli.screenshot.clone(),
            frames_until_shot: 8,
            shot_done: Arc::new(AtomicBool::new(false)),
            bench: cli.bench_playback.map(|s| (s, 0.0)),
            bench_done: false,
            canvas_focus_request: false,
            timeline_scroll: egui::Vec2::ZERO,
            sel_anchor: None,
            cell_clipboard: None,
            last_stroke_end: None,
            sound: crate::sound::Sound::default(),
            keys: crate::keys::Keys::new(!cfg!(test) && cli.screenshot.is_none()),
            show_camera: true,
            peg_ui: Default::default(),
            shape: tooncraft_engine::shapes::ShapeKind::Line,
            shape_drag: None,
            fill_every_frame: false,
            thumbnails: false,
            thumbs: std::collections::HashMap::new(),
            blend_cache: None,
            selection: None,
            select_mode: SelectMode::Lasso,
            selecting: Vec::new(),
            xform: None,
            pixel_clipboard: None,
            last_frame: 0,
            tablet: if cfg!(test) { crate::tablet::Tablet::default() } else { crate::tablet::Tablet::start(ctx) },
        };
        if let Some(name) = &cli.panel {
            app.workspace.raise(Pane::from_name(name));
        }
        if cli.demo && cli.screenshot.is_some() {
            app.engine.project.onion.before = 2;
            app.engine.project.onion.after = 2;
        }
        if let Some((a, b)) = cli.select {
            let last = app.engine.project.length().saturating_sub(1);
            app.sel_anchor = Some((app.layer, a.min(last)));
            app.frame = b.min(last);
        }
        app.thumbnails = cli.thumbnails;
        if cli.light_table {
            let layer = app.layer;
            app.run(Command::TogglePin { layer, frame: 0 });
            app.run(Command::SetLightTable { enabled: Some(true), opacity: Some(0.3) });
        }
        app.offer_recovery();
        app
    }

    /// No autosave and no recovery offer (UI tests).
    #[cfg(test)]
    pub fn without_autosave(mut self) -> Self {
        self.autosave_dir = None;
        self.recovery = None;
        self
    }

    pub fn now(&self) -> f64 {
        self.clock.elapsed().as_secs_f64()
    }

    pub fn brush(&self) -> Brush {
        let kind = self.tool.brush().unwrap_or(BrushKind::Pencil);
        Brush { color: self.color, ..self.brushes[brush_index(kind)] }
    }

    pub fn brush_mut(&mut self, kind: BrushKind) -> &mut Brush {
        &mut self.brushes[brush_index(kind)]
    }

    pub fn set_status(&mut self, s: impl Into<String>) {
        self.status = s.into();
    }

    fn autosave_key(&self) -> String {
        tooncraft_format::autosave::key_for(self.path.as_deref(), &self.engine.project.name)
    }

    fn offer_recovery(&mut self) {
        let Some(dir) = &self.autosave_dir else { return };
        let key = self.autosave_key();
        if let Some((path, when)) = tooncraft_format::autosave::find(dir, &key)
            && tooncraft_format::autosave::newer_than_document(when, self.path.as_deref())
        {
            let when: chrono::DateTime<chrono::Local> = when.into();
            self.recovery = Some(RecoveryOffer { path, when: when.format("%Y-%m-%d %H:%M").to_string() });
        }
    }

    /// Apply a command, reporting errors in the status bar. Returns success.
    pub fn run(&mut self, cmd: Command) -> bool {
        match self.engine.apply(cmd) {
            Ok(()) => true,
            Err(e) => {
                self.set_status(e.to_string());
                false
            }
        }
    }

    pub fn clamp_selection(&mut self) {
        self.frame = self.frame.min(self.engine.project.length().saturating_sub(1));
        self.layer = self.layer.min(self.engine.project.layers.len().saturating_sub(1));
    }

    pub fn undo(&mut self) {
        let label = self.engine.undo_label().map(str::to_string);
        match self.engine.undo() {
            Ok(()) => self.set_status(tlf!("Undid {action}", action = label.unwrap_or_default())),
            Err(e) => self.set_status(e.to_string()),
        }
        self.clamp_selection();
    }

    pub fn redo(&mut self) {
        let label = self.engine.redo_label().map(str::to_string);
        match self.engine.redo() {
            Ok(()) => self.set_status(tlf!("Redid {action}", action = label.unwrap_or_default())),
            Err(e) => self.set_status(e.to_string()),
        }
        self.clamp_selection();
    }

    /// The selected block: (first layer, last layer, first frame, last frame).
    pub fn selection(&self) -> (usize, usize, u32, u32) {
        let (al, af) = self.sel_anchor.unwrap_or((self.layer, self.frame));
        (al.min(self.layer), al.max(self.layer), af.min(self.frame), af.max(self.frame))
    }

    pub fn has_range(&self) -> bool {
        self.sel_anchor.is_some_and(|a| a != (self.layer, self.frame))
    }

    pub fn copy_cells(&mut self) {
        let (l0, l1, f0, f1) = self.selection();
        let block: Vec<Vec<tooncraft_doc::Cell>> = (l0..=l1).filter_map(|l| self.engine.project.layers.get(l)).map(|l| l.copy_cells(f0, f1)).collect();
        let n = f1 - f0 + 1;
        self.cell_clipboard = Some(block);
        self.set_status(tlf!("Copied {frames} frames from {layers} layers", frames = n, layers = l1 - l0 + 1));
    }

    pub fn cut_cells(&mut self) {
        self.copy_cells();
        let (l0, l1, f0, f1) = self.selection();
        self.run(Command::ClearRange { layers: (l0..=l1).collect(), from: f0, to: f1, shift: false });
    }

    pub fn paste_cells(&mut self, insert: bool, duplicate: bool) {
        let Some(block) = self.cell_clipboard.clone() else {
            self.set_status(tl!("Nothing copied yet"));
            return;
        };
        let (l0, _, f0, _) = self.selection();
        let n = block.first().map(|r| r.len()).unwrap_or(0) as u32;
        if self.run(Command::PasteCells { layer: l0, frame: f0, block, insert, duplicate }) {
            self.sel_anchor = Some((l0, f0));
            self.layer = l0;
            self.frame = (f0 + n.saturating_sub(1)).min(self.engine.project.length().saturating_sub(1));
            self.set_status(tl!("Pasted"));
        }
    }

    pub fn delete_selection(&mut self, shift: bool) {
        let (l0, l1, f0, f1) = self.selection();
        self.run(Command::ClearRange { layers: (l0..=l1).collect(), from: f0, to: f1, shift });
        if shift {
            self.sel_anchor = None;
            self.frame = f0;
        }
    }

    /// Apply a transform to the selection (one undo step) and move the outline with it.
    pub fn transform_selection(&mut self, dx: f32, dy: f32, scale: (f32, f32), angle: f32) {
        let Some(poly) = self.selection.clone() else { return };
        let (layer, frame) = (self.layer, self.frame);
        if self.run(Command::TransformSelection { layer, frame, polygon: poly.clone(), dx, dy, scale_x: scale.0, scale_y: scale.1, angle }) {
            let t = tooncraft_engine::select::affine(&poly, dx, dy, scale.0, scale.1, angle);
            self.selection = Some(poly.iter().map(|p| t.apply(p.0, p.1)).collect());
        }
    }

    pub fn delete_selected_pixels(&mut self) {
        if let Some(poly) = self.selection.clone() {
            let (layer, frame) = (self.layer, self.frame);
            self.run(Command::ClearSelection { layer, frame, polygon: poly });
        }
    }

    pub fn select_all(&mut self) {
        let (w, h) = (self.engine.project.width() as f32, self.engine.project.height() as f32);
        self.selection = Some(vec![(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)]);
        self.tool = Tool::Select;
    }

    pub fn copy_pixels(&mut self, cut: bool) {
        let Some(poly) = self.selection.clone() else { return };
        let (w, h) = (self.engine.project.width(), self.engine.project.height());
        let Some(mask) = tooncraft_raster::select::polygon_mask(w, h, &poly) else { return };
        let Some(s) = self.engine.project.drawing_at(self.layer, self.frame).and_then(|d| self.engine.project.drawing(d)) else { return };
        let img = tooncraft_raster::select::copy(s, &mask);
        self.pixel_clipboard = Some((img.to_straight(), img.width, img.height, (mask.x as f32, mask.y as f32)));
        if cut {
            self.delete_selected_pixels();
        }
        self.set_status(if cut { tl!("Cut") } else { tl!("Copied") });
    }

    pub fn paste_pixels(&mut self) {
        let Some((rgba, w, h, (x, y))) = self.pixel_clipboard.clone() else { return };
        let (layer, frame) = (self.layer, self.frame);
        if self.run(Command::PastePixels { layer, frame, x, y, width: w, height: h, rgba }) {
            let (w, h) = (w as f32, h as f32);
            self.selection = Some(vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)]);
            self.tool = Tool::Select;
        }
    }

    pub fn save_new_version(&mut self) {
        match self.path.clone() {
            Some(p) => {
                let next = tooncraft_format::next_version_path(&p);
                self.save_to(&next);
            }
            None => {
                self.save_as();
            }
        }
    }

    pub fn import_images(&mut self) {
        let Some(files) = rfd::FileDialog::new().add_filter(tl!("Images"), &["png", "jpg", "jpeg", "webp", "tif", "tiff", "bmp", "gif"]).pick_files() else {
            return;
        };
        let mut paths: Vec<String> = files.iter().map(|f| f.to_string_lossy().into_owned()).collect();
        paths.sort();
        let n = paths.len();
        let frame = self.frame;
        if self.run(Command::ImportImages { paths, frame, hold: 1, name: tl!("Imported").to_string() }) {
            self.layer = self.engine.project.layers.len().saturating_sub(1);
            self.set_status(tlf!("Imported {count} images", count = n));
        }
    }

    pub fn go_to(&mut self, frame: i64) {
        let len = self.engine.project.length() as i64;
        self.frame = frame.rem_euclid(len.max(1)) as u32;
    }

    pub fn toggle_play(&mut self) {
        if self.playback.playing {
            self.playback.stop();
            self.sound.stop();
        } else {
            let now = self.now();
            self.playback.play(now, self.frame, 0, self.engine.project.length());
            let (offset, fps) = (self.sound_offset(), self.engine.project.fps());
            self.sound.play(self.frame as f64, offset, fps);
        }
    }

    pub fn sound_offset(&self) -> i32 {
        // the mix starts at frame 0 (see `Sound::sync`)
        0
    }

    pub fn import_sound(&mut self) {
        let Some(f) = rfd::FileDialog::new().add_filter(tl!("Sound"), &["wav", "aif", "aiff", "mp3", "m4a", "aac", "flac", "ogg", "opus"]).pick_file() else {
            return;
        };
        // a new clip on the next free track (the first one when the shot has no sound)
        let track = self.engine.project.sounds.iter().map(|s| s.track + 1).max().unwrap_or(0);
        if self.run(Command::AddSound { path: f.to_string_lossy().into_owned(), offset: 0, track }) {
            self.set_status(tlf!("Sound {file} added", file = f.display()));
        }
    }

    // ----- files -----

    pub fn replace_engine(&mut self, engine: Engine, path: Option<PathBuf>) {
        self.engine = engine;
        self.path = path;
        self.frame = 0;
        self.layer = self.engine.project.layers.len().saturating_sub(1);
        self.textures.clear();
        self.view_fitted = false;
        self.playback.stop();
        self.autosaved_changes = 0;
    }

    pub fn save(&mut self) -> bool {
        match self.path.clone() {
            Some(p) => self.save_to(&p),
            None => self.save_as(),
        }
    }

    pub fn save_as(&mut self) -> bool {
        let name = format!("{}.{}", self.engine.project.name, tooncraft_format::EXTENSION);
        let Some(mut p) = rfd::FileDialog::new().add_filter(tl!("ToonCraft shot"), &[tooncraft_format::EXTENSION]).set_file_name(name).save_file() else {
            return false;
        };
        if p.extension().is_none() {
            p.set_extension(tooncraft_format::EXTENSION);
        }
        self.save_to(&p)
    }

    pub fn save_to(&mut self, p: &Path) -> bool {
        let old_key = self.autosave_key();
        match tooncraft_format::save(p, &self.engine.project) {
            Ok(r) => {
                self.engine.mark_saved();
                self.path = Some(p.to_path_buf());
                if let Some(dir) = &self.autosave_dir {
                    tooncraft_format::autosave::discard(dir, &old_key);
                    tooncraft_format::autosave::discard(dir, &self.autosave_key());
                }
                self.set_status(tlf!("Saved {file} ({size} KB)", file = p.display(), size = r.bytes / 1024));
                true
            }
            Err(e) => {
                self.set_status(tlf!("Could not save: {error}", error = e));
                false
            }
        }
    }

    pub fn open_dialog(&mut self) {
        if let Some(p) = rfd::FileDialog::new().add_filter(tl!("ToonCraft shot"), &[tooncraft_format::EXTENSION]).pick_file() {
            self.open(&p);
        }
    }

    pub fn open(&mut self, p: &Path) {
        match open_checked(p) {
            Ok(e) => {
                self.replace_engine(e, Some(p.to_path_buf()));
                self.set_status(tlf!("Opened {file}", file = p.display()));
                self.offer_recovery();
            }
            Err(e) => self.set_status(e),
        }
    }

    pub fn export(&mut self, kind: &str) {
        use tooncraft_engine::export::{self, ExportOptions};
        let o = ExportOptions::whole(&self.engine.project);
        let name = self.engine.project.name.clone();
        let result = match kind {
            "png" => rfd::FileDialog::new().set_title(tl!("Folder for the PNG frames")).pick_folder().map(|d| {
                export::png_sequence(&self.engine.project, &d, &name, o, |_| {})
                    .map(|f| tlf!("Exported {count} frames to {folder}", count = f.len(), folder = d.display()))
                    .map_err(|e| e.to_string())
            }),
            "mp4" => rfd::FileDialog::new()
                .add_filter("MP4", &["mp4"])
                .set_file_name(format!("{name}.mp4"))
                .save_file()
                .map(|f| export::mp4(&self.engine.project, &f, o, |_| {}).map(|_| tlf!("Exported {file}", file = f.display())).map_err(|e| e.to_string())),
            "apng" => rfd::FileDialog::new()
                .add_filter("PNG", &["png"])
                .set_file_name(format!("{name}.png"))
                .save_file()
                .map(|f| export::apng(&self.engine.project, &f, o, |_| {}).map(|_| tlf!("Exported {file}", file = f.display())).map_err(|e| e.to_string())),
            "webp" | "webp-lossy" => rfd::FileDialog::new().add_filter("WebP", &["webp"]).set_file_name(format!("{name}.webp")).save_file().map(|f| {
                let opt = export::WebpOptions { lossless: kind == "webp", quality: 80 };
                export::webp_animated(&self.engine.project, &f, o, opt, |_| {}).map(|_| tlf!("Exported {file}", file = f.display())).map_err(|e| e.to_string())
            }),
            "webp-frame" => {
                rfd::FileDialog::new().add_filter("WebP", &["webp"]).set_file_name(format!("{name}-{}.webp", self.frame + 1)).save_file().map(|f| {
                    export::webp_frame(&self.engine.project, self.frame, &f, export::WebpOptions::default(), o.paper)
                        .map(|_| tlf!("Exported {file}", file = f.display()))
                        .map_err(|e| e.to_string())
                })
            }
            "tga" => rfd::FileDialog::new().set_title(tl!("Folder for the TGA frames")).pick_folder().map(|d| {
                export::tga_sequence(&self.engine.project, &d, &name, o, |_| {})
                    .map(|f| tlf!("Exported {count} frames to {folder}", count = f.len(), folder = d.display()))
                    .map_err(|e| e.to_string())
            }),
            "tiff" => rfd::FileDialog::new().set_title(tl!("Folder for the TIFF frames")).pick_folder().map(|d| {
                export::tiff_sequence(&self.engine.project, &d, &name, o, |_| {})
                    .map(|f| tlf!("Exported {count} frames to {folder}", count = f.len(), folder = d.display()))
                    .map_err(|e| e.to_string())
            }),
            "psd-frames" | "psb-frames" => {
                let ext = if kind == "psb-frames" { "psb" } else { "psd" };
                rfd::FileDialog::new().add_filter("PSD", &[ext]).set_file_name(format!("{name}.{ext}")).save_file().map(|f| {
                    tooncraft_interop::psd::export_frames(&self.engine.project, o.start, o.end, kind == "psb-frames")
                        .map_err(|e| e.to_string())
                        .and_then(|b| std::fs::write(&f, b).map_err(|e| e.to_string()))
                        .map(|_| tlf!("Exported {file}", file = f.display()))
                })
            }
            "prores4444" | "prores422" => rfd::FileDialog::new().add_filter("MOV", &["mov"]).set_file_name(format!("{name}.mov")).save_file().map(|f| {
                tooncraft_interop::prores::export_mov(&self.engine.project, &f, o.start, o.end, kind == "prores4444")
                    .map(|_| tlf!("Exported {file}", file = f.display()))
                    .map_err(|e| e.to_string())
            }),
            "layers" => rfd::FileDialog::new().set_title(tl!("Folder for the layer sequences")).pick_folder().map(|d| {
                tooncraft_interop::layer_sequences::export(&self.engine.project, &d, o.start, o.end, |_| {})
                    .map(|f| tlf!("Exported {count} files to {folder}", count = f.len(), folder = d.display()))
                    .map_err(|e| e.to_string())
            }),
            "levels" => rfd::FileDialog::new().set_title(tl!("Folder for the exposure sheet and drawings")).pick_folder().map(|d| {
                tooncraft_interop::levels::export(&self.engine.project, &d)
                    .map(|f| tlf!("Exported {count} files to {folder}", count = f.len(), folder = d.display()))
                    .map_err(|e| e.to_string())
            }),
            "psd" => rfd::FileDialog::new().add_filter("PSD", &["psd"]).set_file_name(format!("{name}-{}.psd", self.frame + 1)).save_file().map(|f| {
                tooncraft_interop::psd::export_frame(&self.engine.project, self.frame)
                    .map_err(|e| e.to_string())
                    .and_then(|b| std::fs::write(&f, b).map_err(|e| e.to_string()))
                    .map(|_| tlf!("Exported {file}", file = f.display()))
            }),
            "gif" => rfd::FileDialog::new()
                .add_filter("GIF", &["gif"])
                .set_file_name(format!("{name}.gif"))
                .save_file()
                .map(|f| export::gif(&self.engine.project, &f, o, |_| {}).map(|_| tlf!("Exported {file}", file = f.display())).map_err(|e| e.to_string())),
            _ => rfd::FileDialog::new().add_filter("OpenRaster", &["ora"]).set_file_name(format!("{name}-{}.ora", self.frame + 1)).save_file().map(|f| {
                export::ora_frame(&self.engine.project, self.frame, &f).map(|_| tlf!("Exported {file}", file = f.display())).map_err(|e| e.to_string())
            }),
        };
        match result {
            Some(Ok(s)) => self.set_status(s),
            Some(Err(e)) => self.set_status(tlf!("Export failed: {error}", error = e)),
            None => {}
        }
    }

    fn autosave_tick(&mut self) {
        let Some(dir) = self.autosave_dir.clone() else { return };
        if !self.engine.is_dirty() || self.engine.changes() == self.autosaved_changes || self.engine.is_stroking() {
            return;
        }
        if self.autosave_at.elapsed().as_secs() < AUTOSAVE_SECONDS || self.autosave_busy.load(Ordering::SeqCst) {
            return;
        }
        // Copy-on-write snapshot: tiles are shared, so this is a pointer copy.
        let snapshot = self.engine.project.clone();
        let key = self.autosave_key();
        let busy = self.autosave_busy.clone();
        busy.store(true, Ordering::SeqCst);
        self.autosave_at = Instant::now();
        self.autosaved_changes = self.engine.changes();
        let spawned = std::thread::Builder::new().name("autosave".into()).spawn(move || {
            if let Err(e) = tooncraft_format::autosave::write(&dir, &key, &snapshot) {
                log::warn!("autosave failed: {e}");
            }
            busy.store(false, Ordering::SeqCst);
        });
        if spawned.is_err() {
            self.autosave_busy.store(false, Ordering::SeqCst);
        }
    }

    // ----- per-frame -----

    /// Per-frame logic outside drawing: playback clock, autosave.
    pub fn tick(&mut self, ctx: &egui::Context) {
        if self.playback.playing {
            let now = self.now();
            if let Some(l) = self.stats.last_frame_at {
                self.stats.intervals.push((now - l) * 1000.0);
            }
            self.stats.last_frame_at = Some(now);
            let f = self.playback.frame(now, self.engine.project.fps());
            if self.stats.last_shown.is_some_and(|l| l != f) || self.stats.last_shown.is_none() {
                if let Some(l) = self.stats.last_shown {
                    let len = self.engine.project.length().max(1);
                    let step = (f + len - l) % len;
                    self.stats.dropped += step.saturating_sub(1) as u64;
                }
                self.stats.shown.push(now);
                self.stats.shown_total += 1;
                self.stats.last_shown = Some(f);
            }
            self.stats.shown.retain(|t| now - t <= 1.0);
            self.stats.playback_fps = self.stats.shown.len() as f64;
            self.frame = f;
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(self.playback.until_next(now, self.engine.project.fps())));
        } else {
            self.stats.last_shown = None;
        }
        self.autosave_tick();
        // Sound: decode when the soundtrack changed, follow playback, scrub on frame changes.
        self.sound.sync(&self.engine.project);
        let (offset, fps) = (self.sound_offset(), self.engine.project.fps());
        if self.playback.playing {
            self.sound.follow(self.frame as f64, offset, fps);
        } else if self.frame != self.last_frame {
            self.sound.scrub(self.frame, offset, fps);
        }
        self.last_frame = self.frame;
        let now = self.now();
        if let Some((secs, started)) = &mut self.bench {
            if *started == 0.0 {
                *started = now;
                self.playback.play(now, 0, 0, self.engine.project.length());
                self.stats.dropped = 0;
                self.stats.shown_total = 0;
                self.stats.frame_times.clear();
                self.stats.intervals.clear();
            } else if now - *started > *secs && !self.bench_done {
                self.bench_done = true;
                let mut t = self.stats.frame_times.clone();
                t.sort_by(|a, b| a.total_cmp(b));
                let pct = |p: f64| t.get(((t.len().max(1) - 1) as f64 * p) as usize).copied().unwrap_or(0.0);
                let mean = t.iter().sum::<f64>() / t.len().max(1) as f64;
                println!(
                    "playback: {} UI frames in {:.1} s ({:.0} redraws/s); UI frame CPU time mean {:.2} ms, median {:.2} ms, 99th percentile {:.2} ms, max {:.2} ms; shot at {} fps, {} animation frames shown ({:.1} per second), {} dropped",
                    t.len(),
                    secs,
                    t.len() as f64 / *secs,
                    mean,
                    pct(0.5),
                    pct(0.99),
                    t.last().copied().unwrap_or(0.0),
                    self.engine.project.fps(),
                    self.stats.shown_total,
                    self.stats.shown_total as f64 / *secs,
                    self.stats.dropped
                );
                let mut iv = self.stats.intervals.clone();
                iv.sort_by(|a, b| a.total_cmp(b));
                let ip = |p: f64| iv.get(((iv.len().max(1) - 1) as f64 * p) as usize).copied().unwrap_or(0.0);
                println!(
                    "redraw interval: median {:.1} ms, 99th percentile {:.1} ms, max {:.1} ms (gaps longer than one shot frame drop frames)",
                    ip(0.5),
                    ip(0.99),
                    iv.last().copied().unwrap_or(0.0)
                );
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint();
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui) {
        let t0 = Instant::now();
        let ctx = ui.ctx().clone();
        self.textures.begin_frame();
        self.shortcuts(&ctx);
        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::top("tool-options").show(ui, |ui| crate::panels::tool_options(self, ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("tools").exact_size(46.0).resizable(false).show(ui, |ui| crate::panels::toolbar(self, ui));
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| crate::dock::show(self, ui));
        self.dialogs(&ctx);
        self.textures.end_frame();
        self.screenshot_step(&ctx);
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        self.stats.frame_ms = ms;
        if self.playback.playing {
            self.stats.frame_times.push(ms);
            if self.stats.frame_times.len() > 100_000 {
                self.stats.frame_times.drain(..50_000);
            }
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let p = &self.engine.project;
            let layer = p.layers.get(self.layer).map(|l| l.name.clone()).unwrap_or_default();
            ui.label(tlf!("Frame {frame} / {total}", frame = self.frame + 1, total = p.length()));
            ui.separator();
            ui.label(layer);
            ui.separator();
            ui.label(format!("{:.0} %  {:.0}°", self.view.zoom * 100.0, self.view.rotation_degrees()));
            ui.separator();
            ui.label(tlf!("{w}×{h} · {fps} fps", w = p.width(), h = p.height(), fps = p.fps()));
            ui.separator();
            ui.label(tlf!("Memory: {pixels} MB drawings, {history} MB history", pixels = p.memory_bytes() >> 20, history = self.engine.history_bytes() >> 20));
            ui.separator();
            let perf = if self.playback.playing {
                tlf!("{fps} fps playback · {ms} ms/frame", fps = format!("{:.0}", self.stats.playback_fps), ms = format!("{:.1}", self.stats.frame_ms))
            } else {
                tlf!("stroke {ms} ms", ms = format!("{:.2}", self.stats.stroke_ms))
            };
            ui.label(perf);
            ui.separator();
            // The status line is a polite live region: results of actions are announced.
            let status = ui.label(&self.status);
            ui.ctx().accesskit_node_builder(status.id, |n| n.set_live(egui::accesskit::Live::Polite));
        });
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(tl!("File"), |ui| {
                if ui.button(tl!("New Shot…")).clicked() {
                    self.new_shot = Some(NewShot::default());
                }
                if ui.button(tl!("Open…")).clicked() {
                    self.open_dialog();
                }
                ui.separator();
                if ui.button(tl!("Save")).clicked() {
                    self.save();
                }
                if ui.button(tl!("Save As…")).clicked() {
                    self.save_as();
                }
                if ui.button(tl!("Save New Version")).on_hover_text(tl!("Save as shot_v002, shot_v003 … next to the current file (Ctrl+Alt+S)")).clicked() {
                    self.save_new_version();
                }
                ui.separator();
                if ui.button(tl!("Import Image Sequence…")).clicked() {
                    self.import_images();
                }
                ui.menu_button(tl!("Import from Other Apps"), |ui| {
                    if ui.button(tl!("Layers from PSD or OpenRaster…")).clicked()
                        && let Some(f) = rfd::FileDialog::new().add_filter(tl!("Layered images"), &["psd", "psb", "ora"]).pick_file()
                    {
                        let frame = self.frame;
                        if self.run(Command::ImportLayers { path: f.to_string_lossy().into_owned(), frame }) {
                            self.set_status(tlf!("Imported layers from {file}", file = f.display()));
                        }
                    }
                    if ui.button(tl!("Animated GIF…")).clicked()
                        && let Some(f) = rfd::FileDialog::new().add_filter("GIF", &["gif"]).pick_file()
                    {
                        let frame = self.frame;
                        self.run(Command::ImportGif { path: f.to_string_lossy().into_owned(), frame });
                    }
                    if ui.button(tl!("XDTS Exposure Sheet with Drawings…")).clicked()
                        && let Some(f) = rfd::FileDialog::new().add_filter("XDTS", &["xdts"]).pick_file()
                    {
                        self.run(Command::ImportSheet { path: f.to_string_lossy().into_owned() });
                    }
                });
                if ui.button(tl!("Import Sound…")).clicked() {
                    self.import_sound();
                }
                if ui.add_enabled(!self.engine.project.sounds.is_empty(), egui::Button::new(tl!("Remove Sound"))).clicked() {
                    self.run(Command::SetSound { path: None, offset: 0 });
                }
                ui.menu_button(tl!("Export"), |ui| {
                    if ui.button(tl!("Video (MP4, H.264)…")).clicked() {
                        self.export("mp4");
                    }
                    if ui.button(tl!("Animated PNG…")).clicked() {
                        self.export("apng");
                    }
                    if ui.button(tl!("Animated WebP…")).clicked() {
                        self.export("webp");
                    }
                    if ui.button(tl!("Animated WebP, Lossy…")).clicked() {
                        self.export("webp-lossy");
                    }
                    if ui.button(tl!("WebP Image, Current Frame…")).clicked() {
                        self.export("webp-frame");
                    }
                    ui.separator();
                    if ui.button(tl!("ProRes 4444 with Alpha (MOV)…")).clicked() {
                        self.export("prores4444");
                    }
                    if ui.button(tl!("ProRes 422 HQ (MOV)…")).clicked() {
                        self.export("prores422");
                    }
                    if ui.button(tl!("PNG Sequence per Layer…")).clicked() {
                        self.export("layers");
                    }
                    if ui.button(tl!("Layered PSD, Current Frame…")).clicked() {
                        self.export("psd");
                    }
                    if ui.button(tl!("Layered PSD, All Frames…")).clicked() {
                        self.export("psd-frames");
                    }
                    if ui.button(tl!("Layered PSB, All Frames…")).clicked() {
                        self.export("psb-frames");
                    }
                    if ui.button(tl!("TGA Sequence…")).clicked() {
                        self.export("tga");
                    }
                    if ui.button(tl!("TIFF Sequence…")).clicked() {
                        self.export("tiff");
                    }
                    if ui.button(tl!("XDTS Exposure Sheet with Drawings…")).clicked() {
                        self.export("levels");
                    }
                    if ui.button(tl!("PNG Sequence…")).clicked() {
                        self.export("png");
                    }
                    if ui.button(tl!("Animated GIF…")).clicked() {
                        self.export("gif");
                    }
                    if ui.button(tl!("OpenRaster, Current Frame…")).clicked() {
                        self.export("ora");
                    }
                });
                ui.separator();
                if ui.button(tl!("Quit")).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button(tl!("Edit"), |ui| {
                let u = self.engine.undo_label().map(|l| tlf!("Undo {action}", action = l)).unwrap_or_else(|| tl!("Undo").to_string());
                if ui.add_enabled(self.engine.can_undo(), egui::Button::new(u)).clicked() {
                    self.undo();
                }
                let r = self.engine.redo_label().map(|l| tlf!("Redo {action}", action = l)).unwrap_or_else(|| tl!("Redo").to_string());
                if ui.add_enabled(self.engine.can_redo(), egui::Button::new(r)).clicked() {
                    self.redo();
                }
                ui.separator();
                if ui.button(tl!("Select All")).clicked() {
                    self.select_all();
                }
                let has = self.selection.is_some();
                if ui.add_enabled(has, egui::Button::new(tl!("Deselect"))).clicked() {
                    self.selection = None;
                }
                ui.menu_button(tl!("Transform Selection"), |ui| {
                    if ui.add_enabled(has, egui::Button::new(tl!("Flip Horizontally"))).clicked() {
                        self.transform_selection(0.0, 0.0, (-1.0, 1.0), 0.0);
                    }
                    if ui.add_enabled(has, egui::Button::new(tl!("Flip Vertically"))).clicked() {
                        self.transform_selection(0.0, 0.0, (1.0, -1.0), 0.0);
                    }
                    if ui.add_enabled(has, egui::Button::new(tl!("Rotate 90° Clockwise"))).clicked() {
                        self.transform_selection(0.0, 0.0, (1.0, 1.0), std::f32::consts::FRAC_PI_2);
                    }
                    if ui.add_enabled(has, egui::Button::new(tl!("Rotate 90° Counter-clockwise"))).clicked() {
                        self.transform_selection(0.0, 0.0, (1.0, 1.0), -std::f32::consts::FRAC_PI_2);
                    }
                    if ui.add_enabled(has, egui::Button::new(tl!("Scale 50 %"))).clicked() {
                        self.transform_selection(0.0, 0.0, (0.5, 0.5), 0.0);
                    }
                    if ui.add_enabled(has, egui::Button::new(tl!("Scale 200 %"))).clicked() {
                        self.transform_selection(0.0, 0.0, (2.0, 2.0), 0.0);
                    }
                });
                if ui.add_enabled(has, egui::Button::new(tl!("Erase Selection"))).clicked() {
                    self.delete_selected_pixels();
                }
                ui.separator();
                if ui.button(tl!("Clear Drawing")).clicked() {
                    let (layer, frame) = (self.layer, self.frame);
                    self.run(Command::ClearDrawing { layer, frame });
                }
                if ui.button(tl!("Purge Unused Drawings")).clicked() {
                    let n = self.engine.project.unused_drawings().len();
                    if self.run(Command::PurgeUnused) {
                        self.set_status(tlf!("Removed {count} unused drawings", count = n));
                    }
                }
            });
            ui.menu_button(tl!("Frame"), |ui| crate::timeline::frame_menu(self, ui));
            ui.menu_button(tl!("Layer"), |ui| crate::panels::layer_menu(self, ui));
            ui.menu_button(tl!("View"), |ui| {
                let vp = self.workspace.canvas_rect;
                let canvas = egui::vec2(self.engine.project.width() as f32, self.engine.project.height() as f32);
                if ui.button(tl!("Zoom In")).clicked() {
                    self.view.zoom_at(vp, canvas, vp.center(), 1.25);
                }
                if ui.button(tl!("Zoom Out")).clicked() {
                    self.view.zoom_at(vp, canvas, vp.center(), 0.8);
                }
                if ui.button(tl!("Fit to Window")).clicked() {
                    self.view = View::fit(vp, canvas);
                }
                if ui.button(tl!("Actual Size")).clicked() {
                    self.view.zoom = 1.0;
                }
                ui.separator();
                if ui.button(tl!("Rotate Left")).clicked() {
                    self.view.rotate(-15f32.to_radians());
                }
                if ui.button(tl!("Rotate Right")).clicked() {
                    self.view.rotate(15f32.to_radians());
                }
                if ui.button(tl!("Reset Rotation")).clicked() {
                    self.view.rotation = 0.0;
                }
                ui.checkbox(&mut self.view.flip, tl!("Mirror View"));
                ui.separator();
                let mut onion = self.engine.project.onion.enabled;
                if ui.checkbox(&mut onion, tl!("Onion Skin")).changed() {
                    self.engine.project.onion.enabled = onion;
                }
                let mut lt = self.engine.project.light_table.enabled;
                if ui.checkbox(&mut lt, tl!("Light Table")).changed() {
                    self.engine.project.light_table.enabled = lt;
                }
                ui.separator();
                if ui.checkbox(&mut self.light_theme, tl!("Light Theme")).changed() {
                    crate::theme::apply(ui.ctx(), self.light_theme);
                }
                if ui.button(tl!("Reset Panels")).clicked() {
                    self.workspace = Workspace::default();
                }
            });
            ui.menu_button(tl!("Help"), |ui| {
                if ui.button(tl!("Keyboard Shortcuts")).clicked() {
                    self.show_shortcuts = true;
                }
                if ui.button(tl!("About ToonCraft")).clicked() {
                    self.show_about = true;
                }
            });
        });
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        use egui::{Key, KeyboardShortcut, Modifiers};
        let cmd = |k| KeyboardShortcut::new(Modifiers::COMMAND, k);
        let cmd_shift = |k| KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, k);
        // Shift variants first: egui matches Ctrl+S logically, so it would also eat Ctrl+Shift+S.
        let save_version = ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, Key::S)));
        if save_version {
            self.save_new_version();
        }
        let (save_as, save, open, redo, undo, new) = ctx.input_mut(|i| {
            let save_as = i.consume_shortcut(&cmd_shift(Key::S));
            let save = i.consume_shortcut(&cmd(Key::S));
            let open = i.consume_shortcut(&cmd(Key::O));
            let redo = i.consume_shortcut(&cmd_shift(Key::Z)) || i.consume_shortcut(&cmd(Key::Y));
            let undo = i.consume_shortcut(&cmd(Key::Z));
            (save_as, save, open, redo, undo, i.consume_shortcut(&cmd(Key::N)))
        });
        if save_as {
            self.save_as();
        } else if save {
            self.save();
        }
        if open {
            self.open_dialog();
        }
        if new {
            self.new_shot = Some(NewShot::default());
        }
        if undo && !self.engine.is_stroking() {
            self.undo();
        }
        if redo && !self.engine.is_stroking() {
            self.redo();
        }
        let vp = self.workspace.canvas_rect;
        let canvas = egui::vec2(self.engine.project.width() as f32, self.engine.project.height() as f32);
        let (zoom_in, zoom_out, fit, actual, rot_l, rot_r) = ctx.input_mut(|i| {
            (
                i.consume_shortcut(&cmd(Key::Equals)) || i.consume_shortcut(&cmd(Key::Plus)),
                i.consume_shortcut(&cmd(Key::Minus)),
                i.consume_shortcut(&cmd(Key::Num0)),
                i.consume_shortcut(&cmd(Key::Num1)),
                i.consume_shortcut(&cmd(Key::OpenBracket)),
                i.consume_shortcut(&cmd(Key::CloseBracket)),
            )
        });
        if zoom_in {
            self.view.zoom_at(vp, canvas, vp.center(), 1.25);
        }
        if zoom_out {
            self.view.zoom_at(vp, canvas, vp.center(), 0.8);
        }
        if fit {
            self.view = View::fit(vp, canvas);
        }
        if actual {
            self.view.zoom = 1.0;
        }
        if rot_l {
            self.view.rotate(-15f32.to_radians());
        }
        if rot_r {
            self.view.rotate(15f32.to_radians());
        }
        // F6 moves focus between the canvas and the exposure sheet.
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F6)) {
            let to = if ctx.memory(|m| m.focused()) == Some(crate::canvas::canvas_id()) { crate::timeline::grid_id() } else { crate::canvas::canvas_id() };
            ctx.memory_mut(|m| m.request_focus(to));
        }
        // Single keys only when no text field or focused grid/canvas takes the keyboard.
        let focus = ctx.memory(|m| m.focused());
        let own_focus = focus.is_some_and(|f| f == crate::canvas::canvas_id() || f == crate::timeline::grid_id());
        // Copy / cut / paste of exposure-sheet cells (egui turns Ctrl+C/X/V into these events).
        if focus == Some(crate::timeline::grid_id()) || (focus.is_none() && !ctx.egui_wants_keyboard_input()) {
            let (copy, cut, paste, shift) = ctx.input(|i| {
                let mut r = (false, false, false, i.modifiers.shift);
                for e in &i.events {
                    match e {
                        egui::Event::Copy => r.0 = true,
                        egui::Event::Cut => r.1 = true,
                        egui::Event::Paste(_) => r.2 = true,
                        _ => {}
                    }
                }
                r
            });
            if copy {
                self.copy_cells();
            }
            if cut {
                self.cut_cells();
            }
            if paste {
                self.paste_cells(shift, false);
            }
        } else if focus == Some(crate::canvas::canvas_id()) {
            let (copy, cut, paste) = ctx.input(|i| {
                let mut r = (false, false, false);
                for e in &i.events {
                    match e {
                        egui::Event::Copy => r.0 = true,
                        egui::Event::Cut => r.1 = true,
                        egui::Event::Paste(_) => r.2 = true,
                        _ => {}
                    }
                }
                r
            });
            if copy {
                self.copy_pixels(false);
            }
            if cut {
                self.copy_pixels(true);
            }
            if paste {
                self.paste_pixels();
            }
        }
        if self.selection.is_some()
            && !self.engine.is_stroking()
            && !ctx.egui_wants_keyboard_input()
            && ctx.input(|i| i.key_pressed(Key::Escape))
            && focus != Some(crate::timeline::grid_id())
        {
            self.selection = None;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&cmd(Key::A))) && !ctx.egui_wants_keyboard_input() {
            self.select_all();
        }
        // Alt+arrows pan the canvas from the keyboard.
        if focus.is_none() || focus == Some(crate::canvas::canvas_id()) {
            let d = ctx.input_mut(|i| {
                let mut d = egui::Vec2::ZERO;
                for (k, v) in [
                    (Key::ArrowLeft, egui::vec2(60.0, 0.0)),
                    (Key::ArrowRight, egui::vec2(-60.0, 0.0)),
                    (Key::ArrowUp, egui::vec2(0.0, 60.0)),
                    (Key::ArrowDown, egui::vec2(0.0, -60.0)),
                ] {
                    if i.consume_key(Modifiers::ALT, k) {
                        d += v;
                    }
                }
                d
            });
            self.view.pan += d;
        }
        if (ctx.egui_wants_keyboard_input() && !own_focus) || (focus.is_some() && !own_focus) || self.new_shot.is_some() || self.renaming.is_some() {
            return;
        }
        let plain = |i: &egui::InputState, k| i.modifiers.is_none() && i.key_pressed(k);
        // Remappable commands (craft-keymap); Enter stays with the canvas's keyboard pen.
        let canvas_focused = focus == Some(crate::canvas::canvas_id());
        let hits: Vec<String> = if self.keys.recording.is_some() {
            Vec::new()
        } else {
            self.keys
                .dispatch_filtered(ctx, |id| !(canvas_focused && (id == "frame.play" || (id == "frame.clear" && self.keyboard_pen.is_some_and(|p| p.down)))))
        };
        let hit = |id: &str| hits.iter().any(|h| h == id);
        let picked = Tool::ALL.into_iter().find(|t| hit(&format!("tool.{}", t.id())));
        if let Some(t) = picked
            && !self.engine.is_stroking()
        {
            self.tool = t;
            self.set_status(tlf!("{tool} selected", tool = t.label()));
        }
        let (layer, frame) = (self.layer, self.frame);
        let arrows = ctx.input(|i| {
            [
                plain(i, Key::ArrowLeft) && !own_focus,
                plain(i, Key::ArrowRight) && !own_focus,
                plain(i, Key::ArrowUp) && !own_focus,
                plain(i, Key::ArrowDown) && !own_focus,
                plain(i, Key::Backspace),
                plain(i, Key::Plus),
            ]
        });
        let keys = [
            hit("frame.play"),
            hit("frame.prev") || arrows[0],
            hit("frame.next") || arrows[1],
            hit("frame.first"),
            hit("frame.last"),
            hit("view.onion"),
            hit("frame.new_drawing"),
            hit("frame.duplicate"),
            hit("frame.insert"),
            hit("frame.delete"),
            hit("frame.hold_more") || arrows[5],
            hit("frame.hold_less"),
            hit("frame.clear") || arrows[4],
            hit("brush.smaller"),
            hit("brush.bigger"),
            hit("view.mirror"),
            hit("view.pin"),
            arrows[2],
            arrows[3],
        ];
        let [play, prev, next, first, last, onion, new_d, dup, ins, del, plus, minus, clear, smaller, bigger, mirror, pin, up, down] = keys;
        if play {
            self.toggle_play();
        }
        if prev {
            self.go_to(self.frame as i64 - 1);
        }
        if next {
            self.go_to(self.frame as i64 + 1);
        }
        if first {
            self.frame = 0;
        }
        if last {
            self.frame = self.engine.project.length().saturating_sub(1);
        }
        if onion {
            self.engine.project.onion.enabled = !self.engine.project.onion.enabled;
            let s = if self.engine.project.onion.enabled { tl!("Onion skin on") } else { tl!("Onion skin off") };
            self.set_status(s);
        }
        if new_d && self.run(Command::NewDrawing { layer, frame }) {
            self.set_status(tl!("New drawing"));
        }
        if dup && self.run(Command::DuplicateDrawing { layer, frame }) {
            if let Some((_, end)) = self.engine.project.layers.get(layer).and_then(|l| l.exposure_range(frame)) {
                self.frame = end.saturating_sub(1).max(frame + 1).min(self.engine.project.length().saturating_sub(1));
            }
            self.set_status(tl!("Drawing duplicated"));
        }
        if ins {
            self.run(Command::InsertFrame { layer: Some(layer), frame });
        }
        if del {
            if self.has_range() {
                self.delete_selection(true);
            } else {
                self.run(Command::DeleteFrame { layer: Some(layer), frame });
            }
        }
        if plus {
            self.run(Command::ExtendHold { layer, frame });
        }
        if minus {
            self.run(Command::ShortenHold { layer, frame });
        }
        if clear && self.selection.is_some() && focus != Some(crate::timeline::grid_id()) {
            self.delete_selected_pixels();
        } else if clear && !own_focus_is_canvas_pen(self) {
            if self.has_range() {
                self.delete_selection(false);
            } else {
                self.run(Command::ClearCell { layer, frame });
            }
        }
        if let Some(kind) = self.tool.brush() {
            let b = self.brush_mut(kind);
            if smaller {
                b.size = (b.size / 1.2).max(0.5);
            }
            if bigger {
                b.size = (b.size * 1.2).min(tooncraft_brush::MAX_SIZE);
            }
        }
        if mirror {
            self.view.flip = !self.view.flip;
        }
        if pin {
            self.run(Command::TogglePin { layer, frame });
            self.set_status(tl!("Light table pins changed"));
        }
        if up {
            self.layer = (self.layer + 1).min(self.engine.project.layers.len().saturating_sub(1));
        }
        if down {
            self.layer = self.layer.saturating_sub(1);
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(offer) = &self.recovery {
            let (path, when) = (offer.path.clone(), offer.when.clone());
            let mut decided = None;
            egui::Window::new(tl!("Recover unsaved work")).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(
                ctx,
                |ui| {
                    ui.label(tlf!("ToonCraft found a recovery copy from {when} that is newer than the shot. Open it?", when = when));
                    ui.horizontal(|ui| {
                        if ui.button(tl!("Recover")).clicked() {
                            decided = Some(true);
                        }
                        if ui.button(tl!("Discard")).clicked() {
                            decided = Some(false);
                        }
                    });
                },
            );
            match decided {
                Some(true) => {
                    match tooncraft_format::load(&path) {
                        Ok(p) => {
                            let doc = self.path.clone();
                            self.replace_engine(Engine::new(p), doc);
                            self.engine.touch();
                            self.set_status(tl!("Recovered; save to keep it"));
                        }
                        Err(e) => self.set_status(e.to_string()),
                    }
                    self.recovery = None;
                }
                Some(false) => {
                    if let Some(dir) = &self.autosave_dir {
                        let key = self.autosave_key();
                        tooncraft_format::autosave::discard(dir, &key);
                    }
                    self.recovery = None;
                }
                None => {}
            }
        }
        if let Some(mut n) = self.new_shot.take() {
            let mut keep = true;
            egui::Window::new(tl!("New shot")).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                egui::Grid::new("new-shot").num_columns(2).show(ui, |ui| {
                    let l = ui.label(tl!("Name"));
                    ui.text_edit_singleline(&mut n.name).labelled_by(l.id);
                    ui.end_row();
                    ui.label(tl!("Size"));
                    ui.horizontal(|ui| {
                        for (label, w, h) in [("1280×720", 1280, 720), ("1920×1080", 1920, 1080), ("3840×2160", 3840, 2160), ("1080×1080", 1080, 1080)] {
                            if ui.selectable_label(n.width == w && n.height == h, label).clicked() {
                                n.width = w;
                                n.height = h;
                            }
                        }
                    });
                    ui.end_row();
                    let l = ui.label(tl!("Width"));
                    ui.add(egui::DragValue::new(&mut n.width).range(1..=tooncraft_raster::MAX_SIDE)).labelled_by(l.id);
                    ui.end_row();
                    let l = ui.label(tl!("Height"));
                    ui.add(egui::DragValue::new(&mut n.height).range(1..=tooncraft_raster::MAX_SIDE)).labelled_by(l.id);
                    ui.end_row();
                    let l = ui.label(tl!("Frames per second"));
                    ui.add(egui::DragValue::new(&mut n.fps).range(tooncraft_doc::MIN_FPS..=tooncraft_doc::MAX_FPS)).labelled_by(l.id);
                    ui.end_row();
                    let l = ui.label(tl!("Frames"));
                    ui.add(egui::DragValue::new(&mut n.frames).range(1..=tooncraft_doc::MAX_FRAMES)).labelled_by(l.id);
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    if ui.button(tl!("Create")).clicked() {
                        match Project::new(&n.name, n.width, n.height, n.fps, n.frames) {
                            Ok(p) => {
                                self.replace_engine(Engine::new(p), None);
                                keep = false;
                            }
                            Err(e) => self.status = e.to_string(),
                        }
                    }
                    if ui.button(tl!("Cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        keep = false;
                    }
                });
            });
            if keep {
                self.new_shot = Some(n);
            }
        }
        if self.show_shortcuts {
            let mut open = true;
            if self.keys.record(ctx) {
                self.set_status(tl!("Shortcut changed"));
            }
            egui::Window::new(tl!("Keyboard shortcuts")).open(&mut open).collapsible(false).default_height(520.0).show(ctx, |ui| {
                ui.label(tl!("Choose Change, then press the new key. Escape cancels."));
                egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                    egui::Grid::new("keys").num_columns(4).striped(true).show(ui, |ui| {
                        for (id, _, _) in crate::keys::COMMANDS {
                            ui.label(crate::keys::label(id));
                            let recording = self.keys.recording.as_deref() == Some(*id);
                            ui.label(if recording { tl!("Press a key…").to_string() } else { self.keys.text(id) });
                            if ui.button(tlf!("Change {what}", what = crate::keys::label(id))).clicked() {
                                self.keys.recording = Some(id.to_string());
                            }
                            if ui.button(tlf!("Reset {what}", what = crate::keys::label(id))).clicked() {
                                self.keys.reset(id);
                            }
                            ui.end_row();
                        }
                    });
                    ui.separator();
                    egui::Grid::new("fixed-keys").num_columns(2).striped(true).show(ui, |ui| {
                        for (k, what) in shortcut_table() {
                            ui.label(k);
                            ui.label(what);
                            ui.end_row();
                        }
                    });
                });
                if ui.button(tl!("Reset All Shortcuts")).clicked() {
                    self.keys.reset_all();
                }
            });
            if !open || (self.keys.recording.is_none() && ctx.input(|i| i.key_pressed(egui::Key::Escape))) {
                self.show_shortcuts = false;
                self.keys.recording = None;
            }
        }
        if self.show_about {
            let mut open = true;
            egui::Window::new(tl!("About ToonCraft")).open(&mut open).collapsible(false).show(ctx, |ui| {
                ui.label(crate::args::version());
                ui.label(tl!("Hand-drawn, frame-by-frame animation. Part of the ArtCraft Crafting Apps."));
                ui.label(tl!("Licensed under the MIT License or the Apache License 2.0."));
            });
            if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.show_about = false;
            }
        }
        // Closing with unsaved work asks first.
        if ctx.input(|i| i.viewport().close_requested()) && self.engine.is_dirty() && !self.close_confirmed && self.screenshot.is_none() && self.bench.is_none()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm_close = true;
        }
        if self.confirm_close {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.confirm_close = false;
                return;
            }
            egui::Window::new(tl!("Unsaved changes")).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.label(tl!("Save the shot before quitting?"));
                ui.horizontal(|ui| {
                    if ui.button(tl!("Save")).clicked() && self.save() {
                        self.close_confirmed = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button(tl!("Don’t Save")).clicked() {
                        self.close_confirmed = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button(tl!("Cancel")).clicked() {
                        self.confirm_close = false;
                    }
                });
            });
        }
    }

    fn screenshot_step(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else { return };
        if self.shot_done.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        ctx.request_repaint();
        if self.frames_until_shot > 0 {
            self.frames_until_shot -= 1;
            if self.frames_until_shot == 0 {
                let done = self.shot_done.clone();
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img| {
                    let mut rgba = tooncraft_raster::RgbaImage::transparent(img.size[0] as u32, img.size[1] as u32);
                    rgba.data = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                    let result =
                        tooncraft_format::encode_png(&rgba).map_err(|e| e.to_string()).and_then(|png| std::fs::write(&path, png).map_err(|e| e.to_string()));
                    match result {
                        Ok(()) => println!("screenshot written to {}", path.display()),
                        Err(e) => eprintln!("screenshot failed: {e}"),
                    }
                    done.store(true, Ordering::SeqCst);
                })));
            }
        }
    }
}

fn own_focus_is_canvas_pen(app: &ToonApp) -> bool {
    app.keyboard_pen.is_some_and(|p| p.down)
}

fn blank_project() -> Project {
    Project::blank(tl!("Shot"))
}

/// Open a file after checking for an interrupted save.
fn open_checked(p: &Path) -> Result<Engine, String> {
    match tooncraft_format::check(p) {
        tooncraft_format::FileState::Interrupted { backup: Some(b) } => {
            return tooncraft_format::load(&b)
                .map(|proj| {
                    let mut e = Engine::new(proj);
                    e.touch();
                    e
                })
                .map_err(|e| tlf!("The last save was interrupted and the backup does not open: {error}", error = e));
        }
        tooncraft_format::FileState::Interrupted { backup: None } => {
            return Err(tl!("The last save of this shot was interrupted and there is no backup.").to_string());
        }
        _ => {}
    }
    tooncraft_format::load(p).map(Engine::new).map_err(|e| tlf!("Could not open {file}: {error}", file = p.display(), error = e))
}

pub fn shortcut_table() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    for (k, what) in [
        ("Enter", tl!("Play / stop")),
        (", / .", tl!("Previous / next frame")),
        ("Left / Right", tl!("Previous / next frame (timeline: move)")),
        ("Up / Down", tl!("Layer above / below")),
        ("Home / End", tl!("First / last frame")),
        ("Shift+N", tl!("New drawing")),
        ("Shift+D", tl!("Duplicate drawing")),
        ("Insert", tl!("Insert frame")),
        ("Shift+Delete", tl!("Delete frame")),
        ("+ / −", tl!("Hold one frame longer / shorter")),
        ("Delete", tl!("Clear frame")),
        ("O", tl!("Onion skin on / off")),
        ("Shift+L", tl!("Pin frame to the light table")),
        ("[ / ]", tl!("Smaller / bigger brush")),
        ("M", tl!("Mirror view")),
        ("Ctrl+[ / Ctrl+]", tl!("Rotate view")),
        ("Ctrl+0 / Ctrl+1", tl!("Fit / actual size")),
        ("Ctrl+= / Ctrl+-", tl!("Zoom in / out")),
        ("Space + drag, Alt+arrows", tl!("Pan")),
        ("Shift+click", tl!("Straight line from the last stroke")),
        ("S, Ctrl+A", tl!("Select tool, select all")),
        ("Select tool: drag inside / corners / top knob", tl!("Move / scale / rotate the selection")),
        ("Canvas focused with a selection: arrows", tl!("Nudge the selection (Shift: 10 px)")),
        ("Shift+arrows, Shift+click (sheet)", tl!("Select frames")),
        ("Ctrl+C / Ctrl+X / Ctrl+V", tl!("Copy / cut / paste frames")),
        ("Ctrl+Shift+V", tl!("Paste frames, moving later frames right")),
        ("Ctrl+Alt+S", tl!("Save new version")),
        ("Alt + click", tl!("Pick colour")),
        ("Canvas focused: arrows, Enter", tl!("Keyboard pen: move, put down / lift")),
        ("Ctrl+Z / Ctrl+Shift+Z", tl!("Undo / redo")),
        ("Ctrl+S / Ctrl+Shift+S", tl!("Save / save as")),
        ("F6", tl!("Next panel")),
    ] {
        v.push((k.to_string(), what.to_string()));
    }
    v
}

impl eframe::App for ToonApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }
}
