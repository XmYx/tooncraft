//! Layer operations in the interface: duplicate, merge down, merge visible, flatten, copying a
//! layer's look, the transparency lock and the brush's paint position (normal, behind, locked).

use egui::Ui;
use tooncraft_brush::Compose;
use tooncraft_engine::Command;

use crate::app::ToonApp;

pub fn compose_name(c: Compose) -> String {
    match c {
        Compose::Normal => tl!("On top"),
        Compose::Behind => tl!("Behind"),
        Compose::KeepAlpha => tl!("Only on paint"),
        Compose::Multiply => tl!("Multiply"),
        Compose::Screen => tl!("Screen"),
        Compose::Overlay => tl!("Overlay"),
        Compose::Darken => tl!("Darken"),
        Compose::Lighten => tl!("Lighten"),
        Compose::Add => tl!("Add light"),
    }
    .to_string()
}

/// The brush's paint position, as a labelled combo box.
pub fn compose_combo(ui: &mut Ui, compose: &mut Compose) {
    let l = ui.label(tl!("Paint"));
    egui::ComboBox::from_id_salt("brush-compose")
        .selected_text(compose_name(*compose))
        .show_ui(ui, |ui| {
            for c in Compose::ALL {
                ui.selectable_value(compose, c, compose_name(c));
            }
        })
        .response
        .labelled_by(l.id)
        .on_hover_text(tl!("Paint on top of the drawing, behind it (only filling gaps), or only where there is already paint"));
}

/// Run a merge and keep the current layer on the merged one.
pub fn merge(app: &mut ToonApp, cmd: Command, lands_on: Option<usize>) {
    if app.run(cmd) {
        if let Some(l) = lands_on {
            app.layer = l;
        }
        app.clamp_selection();
        app.set_status(tl!("Layers merged").to_string());
    }
}

/// Duplicate, merge and flatten buttons for the Layers panel.
pub fn buttons(app: &mut ToonApp, ui: &mut Ui) {
    let layer = app.layer;
    ui.horizontal_wrapped(|ui| {
        if ui.button(tl!("Duplicate Layer")).on_hover_text(tl!("Copy the layer with its drawings, just above it")).clicked()
            && app.run(Command::DuplicateLayer { layer })
        {
            app.layer += 1;
        }
        if ui.add_enabled(layer > 0, egui::Button::new(tl!("Merge Down"))).on_hover_text(tl!("Merge the layer into the one below it")).clicked() {
            merge(app, Command::MergeDown { layer }, Some(layer.saturating_sub(1)));
        }
        if ui.button(tl!("Merge Visible")).on_hover_text(tl!("Merge every visible layer into one")).clicked() {
            merge(app, Command::MergeVisible, Some(0));
        }
        if ui.button(tl!("Flatten")).on_hover_text(tl!("Merge every visible layer into one and remove the hidden layers")).clicked() {
            merge(app, Command::Flatten, Some(0));
        }
    });
}

/// The transparency lock and "copy the look of another layer" for the current layer.
pub fn properties(app: &mut ToonApp, ui: &mut Ui) {
    let li = app.layer;
    let Some(layer) = app.engine.project.layers.get(li) else { return };
    let mut lock = layer.lock_alpha;
    if ui.checkbox(&mut lock, tl!("Lock transparency")).on_hover_text(tl!("Strokes only recolour paint that is already on the layer")).changed() {
        app.run(Command::SetLayerLockAlpha { layer: li, lock });
    }
    let others: Vec<(usize, String)> = app
        .engine
        .project
        .layers
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != li)
        .map(|(i, l)| (i, crate::layer_tree::path_name(&app.engine.project, l)))
        .collect();
    ui.horizontal(|ui| {
        let l = ui.label(tl!("Copy look from"));
        let mut from = None;
        egui::ComboBox::from_id_salt("layer-copy-look")
            .selected_text(tl!("Choose a layer"))
            .show_ui(ui, |ui| {
                for (i, name) in &others {
                    if ui.selectable_label(false, name).clicked() {
                        from = Some(*i);
                    }
                }
            })
            .response
            .labelled_by(l.id)
            .on_hover_text(tl!("Copy opacity, blend mode, label colour, folder, peg, depth and effects to this layer"));
        if let Some(from) = from
            && app.run(Command::CopyLayerProperties { from, to: li })
        {
            app.set_status(tl!("Layer properties copied").to_string());
        }
    });
}

/// The same operations in the Layer menu.
pub fn menu(app: &mut ToonApp, ui: &mut Ui) {
    let layer = app.layer;
    ui.separator();
    if ui.button(tl!("Duplicate Layer")).clicked() && app.run(Command::DuplicateLayer { layer }) {
        app.layer += 1;
    }
    if ui.add_enabled(layer > 0, egui::Button::new(tl!("Merge Layer Down"))).clicked() {
        merge(app, Command::MergeDown { layer }, Some(layer.saturating_sub(1)));
    }
    if ui.button(tl!("Merge Visible Layers")).clicked() {
        merge(app, Command::MergeVisible, Some(0));
    }
    if ui.button(tl!("Flatten Layers")).clicked() {
        merge(app, Command::Flatten, Some(0));
    }
    let lock = app.engine.project.layers.get(layer).is_some_and(|l| l.lock_alpha);
    if ui.button(if lock { tl!("Unlock Transparency") } else { tl!("Lock Transparency") }).clicked() {
        app.run(Command::SetLayerLockAlpha { layer, lock: !lock });
    }
}
