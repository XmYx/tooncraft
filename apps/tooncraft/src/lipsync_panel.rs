//! The Lip Sync panel: the current layer's mouth chart (which drawing shows each mouth), mouth
//! detection from the shot's sound, and mouth timing sheets in and out. Detection writes ordinary
//! keys and holds, so the result is edited in the exposure sheet like any timing.

use egui::vec2;
use tooncraft_engine::Command;
use tooncraft_lipsync::{Mouth, Settings};

use crate::app::ToonApp;

pub fn mouth_name(m: Mouth) -> String {
    match m {
        Mouth::Rest => tl!("Rest"),
        Mouth::Closed => tl!("Closed (M B P)"),
        Mouth::Open => tl!("Open (A I)"),
        Mouth::Wide => tl!("Wide (E)"),
        Mouth::Round => tl!("Round (O)"),
        Mouth::Pucker => tl!("Pucker (U W Q)"),
        Mouth::Teeth => tl!("Teeth on lip (F V)"),
        Mouth::Tongue => tl!("Tongue (L TH)"),
        Mouth::Consonant => tl!("Other consonants"),
    }
    .to_string()
}

#[derive(Clone, Copy)]
struct State {
    settings: Settings,
}

fn state(ui: &egui::Ui) -> State {
    ui.data(|d| d.get_temp::<(f32, u32)>(egui::Id::unique("lipsync-settings")))
        .map(|(silence, min_hold)| State { settings: Settings { silence, min_hold } })
        .unwrap_or(State { settings: Settings::default() })
}

pub fn show(app: &mut ToonApp, ui: &mut egui::Ui) {
    let li = app.layer;
    let frame = app.frame;
    let Some(layer) = app.engine.project.layers.get(li).cloned() else { return };
    let here = app.engine.project.drawing_at(li, frame);
    ui.label(tlf!("Mouth chart of {layer}", layer = layer.name));
    ui.weak(tl!("Show a mouth drawing at the current frame, then press its “Use” button."));
    egui::Grid::new("mouth-chart").num_columns(3).spacing(vec2(8.0, 4.0)).show(ui, |ui| {
        for m in Mouth::ALL {
            let name = mouth_name(m);
            ui.label(&name);
            ui.label(match layer.mouth_chart.get(&m) {
                Some(d) => tlf!("drawing {n}", n = d.0),
                None => tl!("not set").to_string(),
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(here.is_some(), egui::Button::new(tlf!("Use for {mouth}", mouth = name)).small()).clicked() {
                    app.run(Command::SetMouth { layer: li, mouth: m, drawing: here });
                }
                if ui.add_enabled(layer.mouth_chart.contains_key(&m), egui::Button::new(tlf!("Clear {mouth}", mouth = name)).small()).clicked() {
                    app.run(Command::SetMouth { layer: li, mouth: m, drawing: None });
                }
            });
            ui.end_row();
        }
    });
    ui.separator();
    let mut st = state(ui);
    egui::Grid::new("lipsync-settings-grid").num_columns(2).spacing(vec2(8.0, 4.0)).show(ui, |ui| {
        let l = ui.label(tl!("Silence below"));
        ui.add(egui::DragValue::new(&mut st.settings.silence).range(0.0..=1.0).speed(0.005).max_decimals(2)).labelled_by(l.id);
        ui.end_row();
        let l = ui.label(tl!("Hold each mouth at least"));
        ui.add(egui::DragValue::new(&mut st.settings.min_hold).range(1..=12).suffix(tl!(" frames"))).labelled_by(l.id);
        ui.end_row();
    });
    ui.data_mut(|d| d.insert_temp(egui::Id::unique("lipsync-settings"), (st.settings.silence, st.settings.min_hold)));
    ui.horizontal_wrapped(|ui| {
        let ready = !layer.mouth_chart.is_empty() && !app.engine.project.sounds.is_empty();
        if ui.add_enabled(ready, egui::Button::new(tl!("Detect Mouths from Sound"))).clicked()
            && app.run(Command::LipSync { layer: li, from: 0, settings: st.settings })
        {
            app.set_status(tl!("Mouths keyed from the sound; edit them in the exposure sheet"));
        }
        if ui.add_enabled(!layer.mouth_chart.is_empty(), egui::Button::new(tl!("Import Mouth Sheet…"))).clicked()
            && let Some(f) = rfd::FileDialog::new().add_filter(tl!("Mouth sheet"), &["txt", "dat"]).pick_file()
        {
            app.run(Command::ImportMouthSheet { layer: li, path: f.to_string_lossy().into_owned(), from: 0 });
        }
        if ui.add_enabled(!layer.mouth_chart.is_empty(), egui::Button::new(tl!("Export Mouth Sheet…"))).clicked()
            && let Some(f) = rfd::FileDialog::new().add_filter(tl!("Mouth sheet"), &["txt", "dat"]).set_file_name("mouths.txt").save_file()
        {
            let switch = f.extension().is_some_and(|e| e.eq_ignore_ascii_case("dat"));
            match tooncraft_engine::lipsync::sheet_text(&app.engine.project, li, switch)
                .and_then(|t| craft_atomic::atomic_write(&f, t.as_bytes()).map_err(|e| e.to_string()))
            {
                Ok(()) => app.set_status(tlf!("Wrote {file}", file = f.display())),
                Err(e) => app.set_status(e),
            }
        }
    });
}
