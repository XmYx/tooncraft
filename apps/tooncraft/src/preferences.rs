//! Preferences: one window with tabs (General, Drawing, Onion skin, Timeline, Export), each with
//! Restore Defaults. Saved as JSON next to the shortcuts; the file is not trusted, so every value
//! is clamped on load (a bad file gives the defaults, never a crash).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::app::ToonApp;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    General,
    Drawing,
    Onion,
    Timeline,
    Export,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    // general
    pub autosave_seconds: u32,
    pub undo_megabytes: u32,
    /// Show the start window when ToonCraft opens with no shot.
    pub start_screen: bool,
    // drawing
    pub smoothing: f32,
    pub gap: u8,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub frames: u32,
    // onion skin and light table, for new shots
    pub onion_before: u8,
    pub onion_after: u8,
    pub onion_opacity: f32,
    pub light_table_opacity: f32,
    // timeline
    pub vertical_sheet: bool,
    pub thumbnails: bool,
    // export
    pub export_scale: u32,
    pub export_paper: bool,
}

pub const AUTOSAVE_RANGE: (u32, u32) = (10, 900);
pub const UNDO_RANGE: (u32, u32) = (64, 16_384);

impl Default for Prefs {
    fn default() -> Self {
        Self {
            autosave_seconds: 45,
            start_screen: true,
            undo_megabytes: (tooncraft_engine::DEFAULT_UNDO_BUDGET >> 20) as u32,
            smoothing: 0.3,
            gap: 3,
            width: 1920,
            height: 1080,
            fps: 24,
            frames: 48,
            onion_before: 2,
            onion_after: 1,
            onion_opacity: 0.45,
            light_table_opacity: 0.35,
            vertical_sheet: false,
            thumbnails: false,
            export_scale: 1,
            export_paper: true,
        }
    }
}

impl Prefs {
    /// Every value inside its range (a hand-edited or damaged file stays usable).
    pub fn sanitized(self) -> Self {
        let d = Prefs::default();
        let unit = |v: f32, def: f32| if v.is_finite() { v.clamp(0.0, 1.0) } else { def };
        Self {
            autosave_seconds: self.autosave_seconds.clamp(AUTOSAVE_RANGE.0, AUTOSAVE_RANGE.1),
            undo_megabytes: self.undo_megabytes.clamp(UNDO_RANGE.0, UNDO_RANGE.1),
            smoothing: unit(self.smoothing, d.smoothing),
            gap: self.gap.min(tooncraft_raster::fill::MAX_GAP),
            width: self.width.clamp(1, tooncraft_raster::MAX_SIDE),
            height: self.height.clamp(1, tooncraft_raster::MAX_SIDE),
            fps: self.fps.clamp(tooncraft_doc::MIN_FPS, tooncraft_doc::MAX_FPS),
            frames: self.frames.clamp(1, tooncraft_doc::MAX_FRAMES),
            onion_before: self.onion_before.min(tooncraft_doc::ONION_MAX_STEPS as u8),
            onion_after: self.onion_after.min(tooncraft_doc::ONION_MAX_STEPS as u8),
            onion_opacity: unit(self.onion_opacity, d.onion_opacity),
            light_table_opacity: unit(self.light_table_opacity, d.light_table_opacity),
            export_scale: if [1, 2, 4].contains(&self.export_scale) { self.export_scale } else { 1 },
            ..self
        }
    }

    /// Read the saved preferences (defaults when there are none or they cannot be read).
    pub fn load(path: Option<&PathBuf>) -> Self {
        path.and_then(|p| std::fs::read(p).ok())
            .filter(|b| b.len() < 1 << 20)
            .and_then(|b| serde_json::from_slice::<Prefs>(&b).ok())
            .unwrap_or_default()
            .sanitized()
    }

    fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        craft_atomic::atomic_write(path, &bytes).map_err(|e| e.to_string())
    }

    /// Put one tab's values back to their defaults.
    pub fn restore(&mut self, tab: Tab) {
        let d = Prefs::default();
        match tab {
            Tab::General => {
                self.autosave_seconds = d.autosave_seconds;
                self.undo_megabytes = d.undo_megabytes;
                self.start_screen = d.start_screen;
            }
            Tab::Drawing => {
                self.smoothing = d.smoothing;
                self.gap = d.gap;
                self.width = d.width;
                self.height = d.height;
                self.fps = d.fps;
                self.frames = d.frames;
            }
            Tab::Onion => {
                self.onion_before = d.onion_before;
                self.onion_after = d.onion_after;
                self.onion_opacity = d.onion_opacity;
                self.light_table_opacity = d.light_table_opacity;
            }
            Tab::Timeline => {
                self.vertical_sheet = d.vertical_sheet;
                self.thumbnails = d.thumbnails;
            }
            Tab::Export => {
                self.export_scale = d.export_scale;
                self.export_paper = d.export_paper;
            }
        }
    }
}

/// The preferences window's state.
pub struct State {
    pub prefs: Prefs,
    pub open: bool,
    pub tab: Tab,
    /// Where they are saved (none in tests and screenshots).
    store: Option<PathBuf>,
}

impl State {
    pub fn new(persist: bool) -> Self {
        let store = if persist { crate::keys::overrides_path().and_then(|p| p.parent().map(|d| d.join("preferences.json"))) } else { None };
        Self { prefs: Prefs::load(store.as_ref()), open: false, tab: Tab::General, store }
    }
}

/// Settings that take effect at once (autosave interval, undo budget, timeline view).
pub fn apply_live(app: &mut ToonApp) {
    let p = &app.preferences.prefs;
    app.engine.undo_budget = (p.undo_megabytes as u64) << 20;
    app.tl.vertical = p.vertical_sheet;
    app.thumbnails = p.thumbnails;
}

/// Defaults for a shot just made or opened with no settings of its own yet.
pub fn apply_to_new_shot(app: &mut ToonApp) {
    let p = app.preferences.prefs.clone();
    let o = &mut app.engine.project.onion;
    o.before = p.onion_before;
    o.after = p.onion_after;
    o.opacity = p.onion_opacity;
    app.engine.project.light_table.opacity = p.light_table_opacity;
    app.engine.project.paper.export = p.export_paper;
}

/// Defaults for tools at start-up (the drawing tab).
pub fn apply_tool_defaults(app: &mut ToonApp) {
    let p = app.preferences.prefs.clone();
    for b in app.brushes.iter_mut() {
        b.smoothing.average = p.smoothing;
    }
    app.fill.gap = p.gap;
}

fn tab_name(t: Tab) -> String {
    match t {
        Tab::General => tl!("General"),
        Tab::Drawing => tl!("Drawing"),
        Tab::Onion => tl!("Onion skin"),
        Tab::Timeline => tl!("Sheet and timeline"),
        Tab::Export => tl!("Export"),
    }
    .to_string()
}

/// The Preferences window (Edit ▸ Preferences…).
pub fn window(app: &mut ToonApp, ctx: &egui::Context) {
    if !app.preferences.open {
        return;
    }
    let mut open = true;
    let before = app.preferences.prefs.clone();
    egui::Window::new(tl!("Preferences")).open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
        ui.horizontal(|ui| {
            for t in [Tab::General, Tab::Drawing, Tab::Onion, Tab::Timeline, Tab::Export] {
                ui.selectable_value(&mut app.preferences.tab, t, tab_name(t));
            }
        });
        ui.separator();
        let tab = app.preferences.tab;
        let p = &mut app.preferences.prefs;
        egui::Grid::new("prefs-grid").num_columns(2).spacing(egui::vec2(10.0, 6.0)).show(ui, |ui| {
            let num = |ui: &mut egui::Ui, label: &str, v: &mut u32, lo: u32, hi: u32, suffix: &str| {
                let l = ui.label(label);
                ui.add(egui::DragValue::new(v).range(lo..=hi).suffix(suffix)).labelled_by(l.id);
                ui.end_row();
            };
            match tab {
                Tab::General => {
                    num(ui, tl!("Autosave every"), &mut p.autosave_seconds, AUTOSAVE_RANGE.0, AUTOSAVE_RANGE.1, tl!(" s"));
                    num(ui, tl!("Undo memory"), &mut p.undo_megabytes, UNDO_RANGE.0, UNDO_RANGE.1, tl!(" MB"));
                    ui.checkbox(&mut p.start_screen, tl!("Show the start window when opening with no shot"));
                    ui.end_row();
                }
                Tab::Drawing => {
                    let l = ui.label(tl!("Smoothing for new brushes"));
                    ui.add(egui::Slider::new(&mut p.smoothing, 0.0..=1.0)).labelled_by(l.id);
                    ui.end_row();
                    let l = ui.label(tl!("Close gaps by default"));
                    ui.add(egui::DragValue::new(&mut p.gap).range(0..=tooncraft_raster::fill::MAX_GAP).suffix(tl!(" px"))).labelled_by(l.id);
                    ui.end_row();
                    num(ui, tl!("New shot width"), &mut p.width, 1, tooncraft_raster::MAX_SIDE, tl!(" px"));
                    num(ui, tl!("New shot height"), &mut p.height, 1, tooncraft_raster::MAX_SIDE, tl!(" px"));
                    num(ui, tl!("New shot frame rate"), &mut p.fps, tooncraft_doc::MIN_FPS, tooncraft_doc::MAX_FPS, tl!(" fps"));
                    num(ui, tl!("New shot length"), &mut p.frames, 1, tooncraft_doc::MAX_FRAMES, tl!(" frames"));
                }
                Tab::Onion => {
                    let max = tooncraft_doc::ONION_MAX_STEPS as u8;
                    let l = ui.label(tl!("Steps before"));
                    ui.add(egui::DragValue::new(&mut p.onion_before).range(0..=max)).labelled_by(l.id);
                    ui.end_row();
                    let l = ui.label(tl!("Steps after"));
                    ui.add(egui::DragValue::new(&mut p.onion_after).range(0..=max)).labelled_by(l.id);
                    ui.end_row();
                    let l = ui.label(tl!("Onion opacity"));
                    ui.add(egui::Slider::new(&mut p.onion_opacity, 0.0..=1.0)).labelled_by(l.id);
                    ui.end_row();
                    let l = ui.label(tl!("Light table opacity"));
                    ui.add(egui::Slider::new(&mut p.light_table_opacity, 0.0..=1.0)).labelled_by(l.id);
                    ui.end_row();
                }
                Tab::Timeline => {
                    ui.checkbox(&mut p.vertical_sheet, tl!("Sheet down the page (layers as columns)"));
                    ui.end_row();
                    ui.checkbox(&mut p.thumbnails, tl!("Drawing thumbnails in the cells"));
                    ui.end_row();
                }
                Tab::Export => {
                    let l = ui.label(tl!("Default size"));
                    egui::ComboBox::from_id_salt("prefs-export-scale")
                        .selected_text(crate::export_dialog::scale_name(p.export_scale))
                        .show_ui(ui, |ui| {
                            for s in [1, 2, 4] {
                                ui.selectable_value(&mut p.export_scale, s, crate::export_dialog::scale_name(s));
                            }
                        })
                        .response
                        .labelled_by(l.id);
                    ui.end_row();
                    ui.checkbox(&mut p.export_paper, tl!("Include the paper"));
                    ui.end_row();
                }
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(tl!("Restore Defaults")).on_hover_text(tl!("Put this tab's settings back")).clicked() {
                app.preferences.prefs.restore(tab);
            }
            if matches!(tab, Tab::Onion) && ui.button(tl!("Use for This Shot")).clicked() {
                apply_to_new_shot(app);
            }
        });
        ui.label(egui::RichText::new(tl!("New-shot and onion settings apply to shots made from now on.")).weak());
    });
    let p = app.preferences.prefs.clone().sanitized();
    if p != before {
        app.preferences.prefs = p;
        apply_live(app);
        save_now(app);
    }
    if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.preferences.open = false;
    }
}

/// Write the preferences now (nothing in tests and screenshots); a failure is a status message.
pub fn save_now(app: &mut ToonApp) {
    if let Some(path) = app.preferences.store.clone()
        && let Err(e) = app.preferences.prefs.save(&path)
    {
        app.set_status(tlf!("Preferences not saved: {error}", error = e));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostile_preferences_are_clamped_and_bad_files_give_defaults() {
        let bad = Prefs {
            autosave_seconds: 0,
            undo_megabytes: u32::MAX,
            smoothing: f32::NAN,
            gap: 255,
            width: 0,
            fps: 0,
            onion_before: 200,
            export_scale: 3,
            ..Prefs::default()
        }
        .sanitized();
        assert_eq!(bad.autosave_seconds, AUTOSAVE_RANGE.0);
        assert_eq!(bad.undo_megabytes, UNDO_RANGE.1);
        assert_eq!(bad.smoothing, Prefs::default().smoothing);
        assert_eq!(bad.gap, tooncraft_raster::fill::MAX_GAP);
        assert_eq!(bad.width, 1);
        assert_eq!(bad.export_scale, 1);
        assert_eq!(bad.onion_before as usize, tooncraft_doc::ONION_MAX_STEPS);
        let dir = std::env::temp_dir().join(format!("tooncraft-prefs-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("preferences.json");
        std::fs::write(&path, b"{ not json").unwrap();
        assert_eq!(Prefs::load(Some(&path)), Prefs::default());
        std::fs::write(&path, br#"{"autosave_seconds": 99999, "fps": -3}"#).unwrap();
        assert_eq!(Prefs::load(Some(&path)), Prefs::default(), "a wrong type is a bad file");
        std::fs::write(&path, br#"{"autosave_seconds": 99999}"#).unwrap();
        assert_eq!(Prefs::load(Some(&path)).autosave_seconds, AUTOSAVE_RANGE.1);
        let mut p = Prefs { gap: 9, ..Prefs::default() };
        p.restore(Tab::Drawing);
        assert_eq!(p.gap, Prefs::default().gap);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
