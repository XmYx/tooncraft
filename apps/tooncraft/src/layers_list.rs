//! The Layers panel's list: a toolbar with a split add button ("+" adds a drawing layer above
//! the current one, the arrow beside it offers the other kinds), duplicate, merge down, add a peg
//! and delete, with the rest in its ⋯ menu; then a row per layer with the label colour, the show
//! and lock toggles, a thumbnail of the layer's drawing on the current frame, the name and the
//! opacity. Every change is an engine command.

use egui::{Color32, Rect, Sense, Stroke, Ui, pos2, vec2};
use tooncraft_engine::Command;

use crate::app::ToonApp;
use crate::icons::{Icon, IconButton};

/// Thumbnail height in the rows; the width follows the shot's aspect, within limits.
const THUMB_H: f32 = 20.0;

/// The thumbnail's size for a shot `w` × `h`: [`THUMB_H`] high, between 1:1 and 2.4:1 wide.
pub fn thumb_size(w: u32, h: u32) -> egui::Vec2 {
    let aspect = if h == 0 { 1.0 } else { (w as f32 / h as f32).clamp(1.0, 2.4) };
    vec2((THUMB_H * aspect).round(), THUMB_H)
}

fn add_layer(app: &mut ToonApp) {
    let name = tlf!("Layer {n}", n = app.engine.project.layers.len() + 1);
    if app.run(Command::AddLayer { name, index: Some(app.layer + 1) }) {
        app.layer += 1;
    }
}

/// The small arrow half of the split add button: opens the other kinds of layer.
fn add_other(app: &mut ToonApp, ui: &mut Ui) {
    let (rect, resp) = ui.allocate_exact_size(vec2(12.0, crate::theme::metrics::CONTROL_H), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tl!("Add Other")));
    let pal = crate::theme::palette(!ui.visuals().dark_mode);
    let hot = resp.hovered() || resp.has_focus();
    if hot {
        ui.painter().rect_filled(rect, crate::theme::metrics::RADIUS_CONTROL as f32, pal.raised);
    }
    let c = rect.center();
    let col = if hot { pal.text } else { pal.weak };
    ui.painter().add(egui::Shape::convex_polygon(vec![c + vec2(-3.5, -1.5), c + vec2(3.5, -1.5), c + vec2(0.0, 2.5)], col, Stroke::NONE));
    if resp.has_focus() {
        ui.painter().rect_stroke(rect, 2.0, Stroke::new(2.0, pal.playhead), egui::StrokeKind::Inside);
    }
    let resp = resp.on_hover_text(tl!("Add another kind of layer: a reference image or a sound"));
    egui::Popup::menu(&resp).show(|ui| {
        if ui.button(tl!("Drawing Layer")).clicked() {
            add_layer(app);
            ui.close();
        }
        if ui.button(tl!("Reference Image…")).on_hover_text(tl!("An image under the drawing, locked and faded, shown for the whole shot")).clicked() {
            app.import_reference();
            ui.close();
        }
        if ui.button(tl!("Sound…")).clicked() {
            app.import_sound();
            ui.close();
        }
    });
}

/// The toolbar above the list. Its buttons act on the current layer and say so in their names, so
/// they never share a name with the same command in a layer row's right-click menu.
pub fn toolbar(app: &mut ToonApp, ui: &mut Ui) {
    let n = app.engine.project.layers.len();
    let layer = app.layer;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        if ui.add(IconButton::new(Icon::Add, tl!("New Layer"))).on_hover_text(tl!("New layer above the current one")).clicked() {
            add_layer(app);
        }
        add_other(app, ui);
        ui.add_space(6.0);
        if ui
            .add(IconButton::new(Icon::Duplicate, tl!("Duplicate Current Layer")))
            .on_hover_text(tl!("Copy the layer with its drawings, just above it"))
            .clicked()
            && app.run(Command::DuplicateLayer { layer })
        {
            app.layer += 1;
        }
        let below = (layer == 0).then(|| tl!("There is no layer below").to_string());
        if ui
            .add(IconButton::new(Icon::MergeDown, tl!("Merge Current Layer Down")).disabled(below))
            .on_hover_text(tl!("Merge the layer into the one below it"))
            .clicked()
        {
            crate::layer_ops::merge(app, Command::MergeDown { layer }, Some(layer.saturating_sub(1)));
        }
        let has_peg = app.engine.project.layers.get(layer).is_some_and(|l| l.peg.is_some());
        let why = has_peg.then(|| tl!("The layer already hangs from a peg").to_string());
        if ui.add(IconButton::new(Icon::Peg, tl!("Peg Current Layer")).disabled(why)).clicked() {
            let name = tlf!("Peg {n}", n = app.engine.project.pegs.len() + 1);
            app.run(Command::AddPegForLayer { layer, name });
        }
        let last = (n <= 1).then(|| tl!("A shot keeps at least one layer").to_string());
        if ui.add(IconButton::new(Icon::Delete, tl!("Delete Current Layer")).disabled(last)).on_hover_text(tl!("Delete the current layer")).clicked() {
            app.run(Command::DeleteLayer { layer });
            app.clamp_selection();
        }
        let more = ui.add(IconButton::new(Icon::More, tl!("Layer options")));
        egui::Popup::menu(&more).show(|ui| options(app, ui));
    });
}

/// The ⋯ menu: order, rename, merge visible, flatten.
fn options(app: &mut ToonApp, ui: &mut Ui) {
    let n = app.engine.project.layers.len();
    if ui.add_enabled(app.layer + 1 < n, egui::Button::new(tl!("Move Layer Up"))).on_hover_text(tl!("Move the layer up")).clicked() {
        let from = app.layer;
        if app.run(Command::MoveLayer { from, to: from + 1 }) {
            app.layer += 1;
        }
    }
    if ui.add_enabled(app.layer > 0, egui::Button::new(tl!("Move Layer Down"))).on_hover_text(tl!("Move the layer down")).clicked() {
        let from = app.layer;
        if app.run(Command::MoveLayer { from, to: from - 1 }) {
            app.layer -= 1;
        }
    }
    if ui.button(tl!("Rename Layer")).clicked() {
        let name = app.engine.project.layers.get(app.layer).map(|l| l.name.clone()).unwrap_or_default();
        app.renaming = Some((app.layer, name));
        ui.close();
    }
    ui.separator();
    if ui.button(tl!("Merge Visible")).on_hover_text(tl!("Merge every visible layer into one")).clicked() {
        crate::layer_ops::merge(app, Command::MergeVisible, Some(0));
    }
    if ui.button(tl!("Flatten")).on_hover_text(tl!("Merge every visible layer into one and remove the hidden layers")).clicked() {
        crate::layer_ops::merge(app, Command::Flatten, Some(0));
    }
}

/// The rows, top layer first.
pub fn rows(app: &mut ToonApp, ui: &mut Ui) {
    let n = app.engine.project.layers.len();
    // thumbnails of the drawings on this frame (cached, a few made a frame)
    let ids: Vec<_> = (0..n).filter_map(|li| app.engine.project.drawing_at(li, app.frame)).collect();
    crate::timeline::refresh_drawings(app, ui.ctx(), ids);
    let pal = crate::theme::palette(app.light_theme);
    let tsize = thumb_size(app.engine.project.width(), app.engine.project.height());
    let mut cmds = Vec::new();
    let mut select = None;
    let mut menus: Vec<(usize, egui::Response)> = Vec::new();
    egui::ScrollArea::vertical().id_salt("layers-scroll").show(ui, |ui| {
        for li in (0..n).rev() {
            let Some(layer) = app.engine.project.layers.get(li) else { continue };
            if !crate::layer_tree::listed(app, li) {
                continue;
            }
            let (visible, locked, mut opacity) = (layer.visible, layer.locked, layer.opacity);
            let current = li == app.layer;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                // the label colour
                let (bar, _) = ui.allocate_exact_size(vec2(3.0, THUMB_H), Sense::hover());
                if let Some(c) = layer.color {
                    ui.painter().rect_filled(bar, 1.0, Color32::from_rgb(c[0], c[1], c[2]));
                }
                if ui.add(IconButton::new(Icon::Eye, tl!("Show")).active(visible)).clicked() {
                    cmds.push(Command::SetLayer { layer: li, visible: Some(!visible), locked: None, opacity: None });
                }
                if ui.add(IconButton::new(Icon::Lock, tl!("Lock")).active(locked)).clicked() {
                    cmds.push(Command::SetLayer { layer: li, visible: None, locked: Some(!locked), opacity: None });
                }
                // the drawing on this frame, on paper
                let (thumb, _) = ui.allocate_exact_size(tsize, Sense::hover());
                ui.painter().rect_filled(thumb, 2.0, Color32::from_rgb(246, 243, 236));
                if let Some((_, t)) = app.engine.project.drawing_at(li, app.frame).and_then(|d| app.thumbs.get(&d)) {
                    ui.painter().image(t.id(), thumb, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
                ui.painter().rect_stroke(thumb, 2.0, Stroke::new(1.0, if current { pal.accent } else { pal.line }), egui::StrokeKind::Inside);
                if let Some((r, name)) = &mut app.renaming
                    && *r == li
                {
                    let resp = ui.add(egui::TextEdit::singleline(name).desired_width(110.0));
                    resp.request_focus();
                    if resp.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        cmds.push(Command::RenameLayer { layer: li, name: name.clone() });
                        app.renaming = None;
                    }
                } else {
                    let resp = ui.selectable_label(current, crate::layer_tree::path_name(&app.engine.project, layer));
                    if resp.clicked() {
                        select = Some(li);
                    }
                    if crate::context_menus::wants_menu(ui, &resp, if current { "layer" } else { "" }) {
                        menus.push((li, resp));
                    }
                }
                let l = ui.label(tl!("Opacity"));
                if ui.add(egui::DragValue::new(&mut opacity).range(0.0..=1.0).speed(0.01).max_decimals(2)).labelled_by(l.id).changed() {
                    cmds.push(Command::SetLayer { layer: li, visible: None, locked: None, opacity: Some(opacity) });
                }
            });
        }
    });
    if let Some(li) = select {
        app.layer = li;
    }
    for c in cmds {
        app.run(c);
    }
    for (li, resp) in menus {
        let launch = if li == app.layer { "layer" } else { "" };
        crate::context_menus::attach(ui, &resp, launch, |ui| crate::context_menus::layer(app, ui, li));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnails_follow_the_shot_aspect_within_limits() {
        assert_eq!(thumb_size(1920, 1080), vec2(36.0, THUMB_H));
        assert_eq!(thumb_size(1000, 1000), vec2(THUMB_H, THUMB_H));
        assert_eq!(thumb_size(100, 2000), vec2(THUMB_H, THUMB_H), "portrait shots get a square");
        assert_eq!(thumb_size(u32::MAX, 1), vec2(48.0, THUMB_H), "very wide shots are capped");
        assert_eq!(thumb_size(10, 0), vec2(THUMB_H, THUMB_H));
    }
}
