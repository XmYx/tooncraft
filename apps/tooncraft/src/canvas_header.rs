//! The strip above the canvas: on/off toggles for what the canvas shows (onion skin, light table,
//! field guide, mirror view, symmetry), so the tool options bar holds only the tool's settings.
//! Each toggle is a selectable button with its state published to assistive technology.

use tooncraft_doc::symmetry::SymmetryMode;
use tooncraft_engine::Command;

use crate::app::ToonApp;

pub fn strip(app: &mut ToonApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let onion = ui
            .toggle_value(&mut app.engine.project.onion.enabled, tl!("Onion Skin"))
            .on_hover_text(tl!("Show the drawings before and after this one (O); right-click for its settings"));
        // right-click: the most used onion settings without opening the panel
        onion.context_menu(|ui| {
            let mut o = app.engine.project.onion;
            egui::Grid::new("onion-quick").num_columns(2).show(ui, |ui| {
                let l = ui.label(tl!("Before"));
                ui.add(egui::DragValue::new(&mut o.before).range(0..=10)).labelled_by(l.id);
                ui.end_row();
                let l = ui.label(tl!("After"));
                ui.add(egui::DragValue::new(&mut o.after).range(0..=10)).labelled_by(l.id);
                ui.end_row();
                let l = ui.label(tl!("Opacity"));
                ui.add(egui::Slider::new(&mut o.opacity, 0.0..=1.0)).labelled_by(l.id);
                ui.end_row();
            });
            ui.checkbox(&mut o.outline, tl!("Outlines only"));
            ui.checkbox(&mut o.all_layers, tl!("Onion skin on every visible layer"));
            if o != app.engine.project.onion {
                app.engine.project.onion = o.sanitized();
            }
            if ui.button(tl!("More Onion Settings…")).clicked() {
                app.workspace.raise(crate::dock::Pane::Onion);
                ui.close();
            }
        });
        ui.toggle_value(&mut app.engine.project.light_table.enabled, tl!("Light Table")).on_hover_text(tl!("Show the pinned drawings under this one"));
        ui.toggle_value(&mut app.canvas_ui.field_guide, tl!("Field Guide"));
        ui.toggle_value(&mut app.view.flip, tl!("Mirror View")).on_hover_text(tl!("Flip the view to check the drawing (M); the drawing does not change"));
        let mut sym = app.engine.project.symmetry;
        let mut on = sym.is_on();
        if ui.toggle_value(&mut on, tl!("Symmetry")).on_hover_text(tl!("Mirror strokes as you draw; choose the kind in a brush's options")).changed() {
            sym.mode = if on { SymmetryMode::Vertical } else { SymmetryMode::Off };
            app.run(Command::SetSymmetry { symmetry: sym });
        }
    });
    ui.separator();
}
