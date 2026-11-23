//! The options shown after picking images to import: where they go (a new layer, one layer per
//! file name, or the current layer), how long each is held, how it is placed on the canvas (fit,
//! actual size, fill), and whether the files' colours are already premultiplied. The import is
//! one undo step; a file that cannot be read stops it before anything changes.

use tooncraft_engine::Command;
use tooncraft_engine::import::{ImportOptions, Placement};

use crate::app::ToonApp;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Target {
    #[default]
    NewLayer,
    ByName,
    CurrentLayer,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Dialog {
    pub paths: Vec<String>,
    pub hold: u32,
    pub placement: Placement,
    pub target: Target,
    pub premultiplied: bool,
}

impl Dialog {
    pub fn new(mut paths: Vec<String>) -> Self {
        paths.sort();
        Self { paths, hold: 1, placement: Placement::Fit, target: Target::NewLayer, premultiplied: false }
    }

    /// The command these choices make, at the current frame and layer.
    pub fn command(&self, app: &ToonApp) -> Command {
        let options = ImportOptions {
            placement: self.placement,
            into_layer: (self.target == Target::CurrentLayer).then_some(app.layer),
            by_name: self.target == Target::ByName,
            premultiplied: self.premultiplied,
        };
        Command::ImportImages { paths: self.paths.clone(), frame: app.frame, hold: self.hold.clamp(1, 64), name: tl!("Imported").to_string(), options }
    }
}

/// Run the import; true when it went through.
pub fn run(app: &mut ToonApp, d: &Dialog) -> bool {
    let n = d.paths.len();
    let before = app.engine.project.layers.len();
    let cmd = d.command(app);
    if !app.run(cmd) {
        return false;
    }
    if d.target != Target::CurrentLayer {
        // the first new layer
        app.layer = before.min(app.engine.project.layers.len().saturating_sub(1));
    }
    app.set_status(tlf!("Imported {count} images", count = n));
    true
}

fn placement_name(p: Placement) -> String {
    match p {
        Placement::Fit => tl!("Fit inside the canvas"),
        Placement::Actual => tl!("Actual size"),
        Placement::Fill => tl!("Fill the canvas"),
    }
    .to_string()
}

pub fn window(app: &mut ToonApp, ctx: &egui::Context) {
    let Some(mut d) = app.import_dialog.take() else { return };
    let mut keep = true;
    let mut go = false;
    egui::Window::new(tl!("Import Images")).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
        ui.label(tlf!("{n} images, from frame {frame}", n = d.paths.len(), frame = app.frame + 1));
        egui::Grid::new("import-grid").num_columns(2).spacing(egui::vec2(10.0, 6.0)).show(ui, |ui| {
            ui.label(tl!("Into"));
            ui.vertical(|ui| {
                ui.radio_value(&mut d.target, Target::NewLayer, tl!("A new layer"));
                ui.radio_value(&mut d.target, Target::ByName, tl!("One layer per file name"));
                ui.radio_value(&mut d.target, Target::CurrentLayer, tl!("The current layer, from this frame"));
            });
            ui.end_row();
            let l = ui.label(tl!("Hold each for"));
            ui.add(egui::DragValue::new(&mut d.hold).range(1..=64).suffix(tl!(" fr"))).labelled_by(l.id);
            ui.end_row();
            let l = ui.label(tl!("Placement"));
            egui::ComboBox::from_id_salt("import-placement")
                .selected_text(placement_name(d.placement))
                .show_ui(ui, |ui| {
                    for p in [Placement::Fit, Placement::Actual, Placement::Fill] {
                        ui.selectable_value(&mut d.placement, p, placement_name(p));
                    }
                })
                .response
                .labelled_by(l.id);
            ui.end_row();
            ui.label("");
            ui.checkbox(&mut d.premultiplied, tl!("Colours already premultiplied")).on_hover_text(tl!("Only for files made that way; most are not"));
            ui.end_row();
        });
        ui.horizontal(|ui| {
            if ui.button(tl!("Import")).clicked() {
                go = true;
            }
            if ui.button(tl!("Cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                keep = false;
            }
        });
    });
    if go {
        run(app, &d);
        keep = false;
    }
    if keep {
        app.import_dialog = Some(d);
    }
}
