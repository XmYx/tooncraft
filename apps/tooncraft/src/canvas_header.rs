//! The Canvas panel's header: icon toggles for what the canvas shows (onion skin, light table,
//! field guide, safe areas, mirror view, symmetry, rulers), then the zoom and the view rotation.
//! Right-click a toggle (or Shift+F10 / the Menu key on it) for its settings: for onion skin that
//! is the popover with an opacity slider per step. Each toggle publishes its on/off state.

use egui::{Align, Layout, Ui};
use tooncraft_doc::symmetry::SymmetryMode;
use tooncraft_engine::Command;

use crate::app::ToonApp;
use crate::icons::IconButton;

/// The registry toggle `id` as an icon button named `label`, with its settings `menu` on
/// right-click and from the keyboard (Shift+F10 or the Menu key).
fn toggle(app: &mut ToonApp, ui: &mut Ui, id: &str, label: &str, menu: fn(&mut ToonApp, &mut Ui)) {
    let Some(c) = crate::commands::get(id) else { return };
    let Some(icon) = c.icon else { return };
    let on = c.checked.is_some_and(|f| f(app));
    let resp = ui.add(IconButton::new(icon, label).active(on).shortcut(c.key_text(app)));
    let now = resp.has_focus() && crate::timeline::menu_key(ui) || id == "view.onion" && crate::context_menus::launch_now(ui.ctx(), "onion");
    crate::timeline::keyboard_menu(ui, &resp, now, |ui| menu(app, ui));
    resp.context_menu(|ui| menu(app, ui));
    if resp.clicked() {
        let ctx = ui.ctx().clone();
        crate::commands::run(app, &ctx, c.id);
    }
}

pub fn strip(app: &mut ToonApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        toggle(app, ui, "view.onion", tl!("Onion Skin"), crate::onion_popover::ui);
        toggle(app, ui, "view.light_table", tl!("Light Table"), light_table_menu);
        toggle(app, ui, "view.field_guide", tl!("Field Guide"), guides_menu);
        toggle(app, ui, "view.safe_areas", tl!("Safe Areas"), guides_menu);
        toggle(app, ui, "view.mirror", tl!("Mirror View"), view_menu);
        toggle(app, ui, "view.symmetry", tl!("Symmetry"), symmetry_menu);
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
