//! The Canvas panel's header: icon toggles for what the canvas shows (onion skin, light table,
//! field guide, safe areas, mirror view, symmetry, rulers), then the zoom and the view rotation.
//! Right-click a toggle (or Shift+F10 / the Menu key on it) for its settings: for onion skin that
//! is the popover with an opacity slider per step. Each toggle publishes its on/off state.

use egui::{Align, Layout, Ui};
use tooncraft_doc::symmetry::SymmetryMode;
use tooncraft_engine::Command;

use crate::app::ToonApp;
use crate::icons::{Icon, IconButton};

/// A toggle with a settings menu on right-click and from the keyboard; true when clicked.
fn toggle(app: &mut ToonApp, ui: &mut Ui, icon: Icon, label: &str, on: bool, key: &str, menu: fn(&mut ToonApp, &mut Ui)) -> bool {
    let b = IconButton::new(icon, label).active(on);
    let b = if key.is_empty() { b } else { b.shortcut(app.keys.text(key)) };
    let resp = ui.add(b);
    let now = resp.has_focus() && crate::timeline::menu_key(ui);
    crate::timeline::keyboard_menu(ui, &resp, now, |ui| menu(app, ui));
    resp.context_menu(|ui| menu(app, ui));
    resp.clicked()
}

pub fn strip(app: &mut ToonApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let onion = app.engine.project.onion.enabled;
        if toggle(app, ui, Icon::Onion, tl!("Onion Skin"), onion, "view.onion", crate::onion_popover::ui) {
            app.run(Command::SetOnion { onion: tooncraft_doc::OnionSkin { enabled: !onion, ..app.engine.project.onion } });
        }
        let lt = app.engine.project.light_table.enabled;
        if toggle(app, ui, Icon::LightTable, tl!("Light Table"), lt, "", light_table_menu) {
            app.run(Command::SetLightTable { enabled: Some(!lt), opacity: None });
        }
        let fg = app.canvas_ui.field_guide;
        if toggle(app, ui, Icon::Grid, tl!("Field Guide"), fg, "", guides_menu) {
            app.canvas_ui.field_guide = !fg;
        }
        let sa = app.canvas_ui.safe_areas;
        if toggle(app, ui, Icon::SafeArea, tl!("Safe Areas"), sa, "", guides_menu) {
            app.canvas_ui.safe_areas = !sa;
        }
        let flip = app.view.flip;
        if toggle(app, ui, Icon::Mirror, tl!("Mirror View"), flip, "view.mirror", view_menu) {
            app.view.flip = !flip;
        }
        let sym = app.engine.project.symmetry;
        if toggle(app, ui, Icon::FlipH, tl!("Symmetry"), sym.is_on(), "", symmetry_menu) {
            let mut s = sym;
            s.mode = if sym.is_on() { SymmetryMode::Off } else { SymmetryMode::Vertical };
            app.run(Command::SetSymmetry { symmetry: s });
        }
        crate::rulers_ui::toggle(app, ui);
        // one control high: a right-to-left layout would otherwise take all the height there is
        let h = ui.spacing().interact_size.y;
        ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), h), Layout::right_to_left(Align::Center), |ui| {
            let deg = app.view.rotation_degrees();
            let rot = ui.button(format!("{deg:.0}°")).on_hover_text(tl!("View rotation; click to set it back to 0°"));
            rot.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tl!("Reset Rotation")));
            if rot.clicked() {
                app.view.rotation = 0.0;
            }
            let zoom = ui.menu_button(format!("{:.0} %", app.view.zoom * 100.0), |ui| view_menu(app, ui)).response;
            zoom.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tl!("Canvas zoom")));
        });
    });
    ui.separator();
}

/// Zoom steps offered in the zoom menu.
pub const ZOOMS: [f32; 6] = [0.25, 0.5, 1.0, 1.5, 2.0, 4.0];

fn view_menu(app: &mut ToonApp, ui: &mut Ui) {
    if ui.button(tl!("Fit to Window")).clicked() {
        let size = egui::vec2(app.engine.project.width() as f32, app.engine.project.height() as f32);
        app.view = crate::view::View::fit(app.workspace.canvas_rect, size);
        ui.close();
    }
    for z in ZOOMS {
        if ui.selectable_label((app.view.zoom - z).abs() < 1e-3, format!("{:.0} %", z * 100.0)).clicked() {
            app.view.zoom = z;
            ui.close();
        }
    }
    ui.separator();
    if ui.button(tl!("Reset Rotation")).clicked() {
        app.view.rotation = 0.0;
        ui.close();
    }
}

fn guides_menu(app: &mut ToonApp, ui: &mut Ui) {
    ui.checkbox(&mut app.canvas_ui.field_guide, tl!("Field Guide"));
    ui.checkbox(&mut app.canvas_ui.safe_areas, tl!("Safe Areas"));
}

fn light_table_menu(app: &mut ToonApp, ui: &mut Ui) {
    let mut o = app.engine.project.light_table.opacity;
    let l = ui.label(tl!("Opacity"));
    if ui.add(egui::Slider::new(&mut o, 0.0..=1.0)).labelled_by(l.id).changed() {
        app.run(Command::SetLightTable { enabled: None, opacity: Some(o) });
    }
    let (layer, frame) = (app.layer, app.frame);
    if ui.add(egui::Button::new(tl!("Pin This Frame")).shortcut_text(app.keys.text("view.pin"))).clicked() {
        app.run(Command::TogglePin { layer, frame });
        ui.close();
    }
}

fn symmetry_menu(app: &mut ToonApp, ui: &mut Ui) {
    let sym = app.engine.project.symmetry;
    for (mode, name) in [
        (SymmetryMode::Vertical, tl!("Left and right")),
        (SymmetryMode::Horizontal, tl!("Top and bottom")),
        (SymmetryMode::Both, tl!("Four ways")),
        (SymmetryMode::Radial, tl!("Radial")),
        (SymmetryMode::RadialMirror, tl!("Radial, mirrored")),
    ] {
        if ui.selectable_label(sym.mode == mode, name).clicked() {
            let mut s = sym;
            s.mode = mode;
            app.run(Command::SetSymmetry { symmetry: s });
            ui.close();
        }
    }
}
