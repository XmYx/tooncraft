//! One Export dialog for every format: Format, Range (whole shot, selected frames, current frame,
//! or from–to), Size (full, half, a quarter), the paper, and the format's own options (GIF loop,
//! frame-number digits for sequences). The choices are remembered per shot for the session. The
//! File ▸ Export entries open it with their format chosen.

use std::collections::HashMap;

use tooncraft_engine::export::ExportOptions;

use crate::app::ToonApp;

/// Every format: (id used by `ToonApp::export`, name).
pub fn formats() -> Vec<(&'static str, String)> {
    vec![
        ("mp4", tl!("Video (MP4, H.264)").to_string()),
        ("mp4-hevc", tl!("Video (MP4, H.265)").to_string()),
        ("mp4-av1", tl!("Video (MP4, AV1)").to_string()),
        ("mov", tl!("Video (QuickTime MOV, H.264)").to_string()),
        ("gif", tl!("Animated GIF").to_string()),
        ("apng", tl!("Animated PNG").to_string()),
        ("webp", tl!("Animated WebP").to_string()),
        ("webp-lossy", tl!("Animated WebP, Lossy").to_string()),
        ("png", tl!("PNG Sequence").to_string()),
        ("tga", tl!("TGA Sequence").to_string()),
        ("tiff", tl!("TIFF Sequence").to_string()),
        ("jpeg", tl!("JPEG Sequence").to_string()),
        ("exr", tl!("OpenEXR Sequence").to_string()),
        ("exr-layers", tl!("Layered OpenEXR Sequence").to_string()),
        ("prores4444", tl!("ProRes 4444 with Alpha (MOV)").to_string()),
        ("prores422", tl!("ProRes 422 HQ (MOV)").to_string()),
        ("layers", tl!("PNG Sequence per Layer").to_string()),
        ("psd-frames", tl!("Layered PSD, All Frames").to_string()),
        ("psb-frames", tl!("Layered PSB, All Frames").to_string()),
        ("levels", tl!("XDTS Exposure Sheet with Drawings").to_string()),
        ("psd", tl!("Layered PSD, Current Frame").to_string()),
        ("webp-frame", tl!("WebP Image, Current Frame").to_string()),
        ("ora", tl!("OpenRaster, Current Frame").to_string()),
    ]
}

/// What a format can take.
pub struct Caps {
    /// A range of frames (otherwise the current frame or the whole sheet).
    pub range: bool,
    /// A smaller output size.
    pub scale: bool,
    pub gif_loop: bool,
    pub digits: bool,
}

pub fn caps(kind: &str) -> Caps {
    let ours = matches!(kind, "mp4" | "mp4-hevc" | "mp4-av1" | "mov" | "gif" | "apng" | "webp" | "webp-lossy" | "png" | "tga" | "tiff" | "jpeg" | "exr" | "exr-layers");
    Caps {
        range: ours || matches!(kind, "prores4444" | "prores422" | "layers" | "psd-frames" | "psb-frames"),
        scale: ours,
        gif_loop: kind == "gif",
        digits: matches!(kind, "png" | "tga" | "tiff" | "jpeg" | "exr" | "exr-layers"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Range {
    Whole,
    Selection,
    Current,
    Custom,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Dialog {
    pub kind: &'static str,
    pub range: Range,
    /// 1-based, inclusive, for Custom.
    pub from: u32,
    pub to: u32,
    pub scale: u32,
    pub paper: bool,
    pub loop_forever: bool,
    pub digits: u8,
    pub burn_in: bool,
}

pub fn scale_name(s: u32) -> String {
    match s {
        2 => tl!("Half size").to_string(),
        4 => tl!("Quarter size").to_string(),
        _ => tl!("Full size").to_string(),
    }
}

impl Dialog {
    pub fn new(app: &ToonApp, kind: &'static str) -> Self {
        let p = &app.preferences.prefs;
        let len = app.engine.project.length();
        Self {
            kind,
            range: Range::Whole,
            from: 1,
            to: len.max(1),
            scale: p.export_scale,
            paper: app.engine.project.paper.export,
            loop_forever: true,
            digits: 0,
            burn_in: false,
        }
    }

    /// The export options these choices make, checked against the shot.
    pub fn options(&self, app: &ToonApp) -> ExportOptions {
        let len = app.engine.project.length().max(1);
        let (start, end) = match self.range {
            Range::Whole => (0, len),
            Range::Current => (app.frame.min(len - 1), app.frame.min(len - 1) + 1),
            Range::Selection => {
                let (_, _, a, b) = app.selection();
                (a.min(len - 1), (b + 1).min(len))
            }
            Range::Custom => {
                let a = self.from.clamp(1, len) - 1;
                let b = self.to.clamp(self.from.clamp(1, len), len);
                (a, b)
            }
        };
        let c = caps(self.kind);
        ExportOptions {
            start,
            end: end.max(start + 1),
            paper: self.paper,
            scale: if c.scale { self.scale.clamp(1, tooncraft_engine::export::MAX_SCALE) } else { 1 },
            loop_forever: self.loop_forever,
            digits: if c.digits { self.digits.min(9) } else { 0 },
            burn_in: c.scale && self.burn_in,
        }
    }
}

/// Open the dialog for `kind`, with the last choices for this shot when there are some.
pub fn open(app: &mut ToonApp, kind: &'static str) {
    let key = app.engine.project.name.clone();
    let mut d = app.export_memory.get(&key).cloned().unwrap_or_else(|| Dialog::new(app, kind));
    d.kind = kind;
    app.export_dialog = Some(d);
}

/// The dialog window.
pub fn window(app: &mut ToonApp, ctx: &egui::Context) {
    let Some(mut d) = app.export_dialog.take() else { return };
    let mut keep = true;
    let mut go = false;
    let len = app.engine.project.length().max(1);
    egui::Window::new(tl!("Export")).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
        egui::Grid::new("export-grid").num_columns(2).spacing(egui::vec2(10.0, 6.0)).show(ui, |ui| {
            let l = ui.label(tl!("Format"));
            let list = formats();
            let current = list.iter().find(|(k, _)| *k == d.kind).map(|(_, n)| n.clone()).unwrap_or_default();
            egui::ComboBox::from_id_salt("export-format")
                .selected_text(current)
                .width(260.0)
                .show_ui(ui, |ui| {
                    for (k, n) in &list {
                        ui.selectable_value(&mut d.kind, *k, n);
                    }
                })
                .response
                .labelled_by(l.id);
            ui.end_row();
            let c = caps(d.kind);
            ui.label(tl!("Frames"));
            ui.add_enabled_ui(c.range, |ui| {
                ui.vertical(|ui| {
                    ui.radio_value(&mut d.range, Range::Whole, tlf!("Whole shot (1 to {n})", n = len));
                    ui.add_enabled_ui(app.has_range(), |ui| ui.radio_value(&mut d.range, Range::Selection, tl!("Selected frames")));
                    ui.radio_value(&mut d.range, Range::Current, tl!("This frame"));
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut d.range, Range::Custom, tl!("From"));
                        let l = ui.label(tl!("first frame"));
                        ui.add(egui::DragValue::new(&mut d.from).range(1..=len)).labelled_by(l.id);
                        let l = ui.label(tl!("to"));
                        ui.add(egui::DragValue::new(&mut d.to).range(1..=len)).labelled_by(l.id);
                    });
                });
            });
            ui.end_row();
            let l = ui.label(tl!("Size"));
            ui.add_enabled_ui(c.scale, |ui| {
                egui::ComboBox::from_id_salt("export-scale")
                    .selected_text(scale_name(d.scale))
                    .show_ui(ui, |ui| {
                        for s in [1, 2, 4] {
                            ui.selectable_value(&mut d.scale, s, scale_name(s));
                        }
                    })
                    .response
                    .labelled_by(l.id);
            });
            ui.end_row();
            ui.label("");
            ui.checkbox(&mut d.paper, tl!("Include the paper"));
            ui.end_row();
            if c.scale {
                ui.label("");
                ui.checkbox(&mut d.burn_in, tl!("Burn in the shot, frame and timecode"))
                    .on_hover_text(tl!("For review copies: a bar along the bottom of every frame"));
                ui.end_row();
            }
            if c.gif_loop {
                ui.label("");
                ui.checkbox(&mut d.loop_forever, tl!("Loop"));
                ui.end_row();
            }
            if c.digits {
                let l = ui.label(tl!("Digits in frame numbers"));
                ui.add(egui::DragValue::new(&mut d.digits).range(0..=9)).labelled_by(l.id).on_hover_text(tl!("0: four, or more for a long shot"));
                ui.end_row();
            }
        });
        let o = d.options(app);
        let (w, h) = o.size(&app.engine.project);
        ui.weak(tlf!("{n} frames at {w}×{h}", n = o.end - o.start, w = w, h = h));
        ui.horizontal(|ui| {
            if ui.button(tl!("Export…")).clicked() {
                go = true;
            }
            if ui.button(tl!("Cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                keep = false;
            }
        });
    });
    d.from = d.from.clamp(1, len);
    d.to = d.to.clamp(1, len);
    app.export_memory.insert(app.engine.project.name.clone(), d.clone());
    if go {
        let o = d.options(app);
        app.export_with(d.kind, o);
        keep = false;
    }
    if keep {
        app.export_dialog = Some(d);
    }
}

/// The remembered choices, per shot name.
pub type Memory = HashMap<String, Dialog>;
