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
    Hand,
}

impl Tool {
    pub const ALL: [Tool; 8] = [Tool::Pencil, Tool::Ink, Tool::Airbrush, Tool::Chalk, Tool::Eraser, Tool::Fill, Tool::Picker, Tool::Hand];

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
            Tool::Hand => tl!("Hand"),
        }
        .to_string()
    }

    pub fn key(self) -> egui::Key {
        match self {
            Tool::Pencil => egui::Key::P,
            Tool::Ink => egui::Key::N,
            Tool::Airbrush => egui::Key::A,
            Tool::Chalk => egui::Key::C,
            Tool::Eraser => egui::Key::E,
            Tool::Fill => egui::Key::F,
            Tool::Picker => egui::Key::I,
            Tool::Hand => egui::Key::H,
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
        };
        if let Some(name) = &cli.panel {
            app.workspace.raise(Pane::from_name(name));
        }
        if cli.demo && cli.screenshot.is_some() {
            app.engine.project.onion.before = 2;
            app.engine.project.onion.after = 2;
        }
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

    pub fn go_to(&mut self, frame: i64) {
        let len = self.engine.project.length() as i64;
        self.frame = frame.rem_euclid(len.max(1)) as u32;
    }

    pub fn toggle_play(&mut self) {
        if self.playback.playing {
            self.playback.stop();
        } else {
            let now = self.now();
            self.playback.play(now, self.frame, 0, self.engine.project.length());
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
                ui.separator();
                ui.menu_button(tl!("Export"), |ui| {
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
        if (ctx.egui_wants_keyboard_input() && !own_focus) || (focus.is_some() && !own_focus) || self.new_shot.is_some() || self.renaming.is_some() {
            return;
        }
        let plain = |i: &egui::InputState, k| i.modifiers.is_none() && i.key_pressed(k);
        let shift = |i: &egui::InputState, k| i.modifiers.shift_only() && i.key_pressed(k);
        let mut picked = None;
        for t in Tool::ALL {
            if ctx.input(|i| plain(i, t.key())) {
                picked = Some(t);
            }
        }
        if let Some(t) = picked
            && !self.engine.is_stroking()
        {
            self.tool = t;
            self.set_status(tlf!("{tool} selected", tool = t.label()));
        }
        let (layer, frame) = (self.layer, self.frame);
        let keys = ctx.input(|i| {
            [
                plain(i, Key::Enter),
                plain(i, Key::Comma) || (plain(i, Key::ArrowLeft) && !own_focus),
                plain(i, Key::Period) || (plain(i, Key::ArrowRight) && !own_focus),
                plain(i, Key::Home),
                plain(i, Key::End),
                plain(i, Key::O),
                shift(i, Key::N),
                shift(i, Key::D),
                plain(i, Key::Insert),
                shift(i, Key::Delete),
                plain(i, Key::Plus) || plain(i, Key::Equals),
                plain(i, Key::Minus),
                plain(i, Key::Delete) || plain(i, Key::Backspace),
                plain(i, Key::OpenBracket),
                plain(i, Key::CloseBracket),
                plain(i, Key::M),
                shift(i, Key::L),
                plain(i, Key::ArrowUp) && !own_focus,
                plain(i, Key::ArrowDown) && !own_focus,
            ]
        });
        let [play, prev, next, first, last, onion, new_d, dup, ins, del, plus, minus, clear, smaller, bigger, mirror, pin, up, down] = keys;
        if play && focus != Some(crate::canvas::canvas_id()) {
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
            self.run(Command::DeleteFrame { layer: Some(layer), frame });
        }
        if plus {
            self.run(Command::ExtendHold { layer, frame });
        }
        if minus {
            self.run(Command::ShortenHold { layer, frame });
        }
        if clear && !own_focus_is_canvas_pen(self) {
            self.run(Command::ClearCell { layer, frame });
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
            egui::Window::new(tl!("Keyboard shortcuts")).open(&mut open).collapsible(false).show(ctx, |ui| {
                egui::Grid::new("keys").num_columns(2).striped(true).show(ui, |ui| {
                    for (k, what) in shortcut_table() {
                        ui.label(k);
                        ui.label(what);
                        ui.end_row();
                    }
                });
            });
            if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.show_shortcuts = false;
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
    let mut v: Vec<(String, String)> = Tool::ALL.iter().map(|t| (format!("{:?}", t.key()), t.label())).collect();
    for (k, what) in [
        ("Enter", tl!("Play / stop")),
        (", / .", tl!("Previous / next frame")),
        ("← / →", tl!("Previous / next frame (timeline: move)")),
        ("↑ / ↓", tl!("Layer above / below")),
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
        ("Space + drag", tl!("Pan")),
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
