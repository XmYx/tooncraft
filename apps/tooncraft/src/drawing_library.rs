//! The Drawing Library panel: the shot's reusable drawings (a blink, a cycle, a prop) with a
//! thumbnail, a name and a length. Add the selected frames of the current layer (or the drawing at
//! the cursor), put an item on the current layer at the cursor as new drawings or linked, rename,
//! reorder and remove items. Every change is an engine command (`SaveToLibrary`, …) and one undo
//! step. The panel's own state (chosen item, typed names) lives in egui's memory.

use tooncraft_doc::{Cell, DrawingId};
use tooncraft_engine::Command;

use crate::app::ToonApp;

#[derive(Clone, Default)]
struct State {
    chosen: usize,
    name: String,
    rename: Option<String>,
    /// Give the rename field the focus once (set when Rename Item is pressed).
    focus_rename: bool,
}

fn state_id() -> egui::Id {
    egui::Id::unique("drawing-library-state")
}

/// Thumbnails of the items' first drawings, a few new ones per UI frame, cached in `app.thumbs`
/// until the drawing changes.
fn refresh_thumbnails(app: &mut ToonApp, ctx: &egui::Context) {
    let mut budget = 4;
    let firsts: Vec<DrawingId> = app.engine.project.library.iter().filter_map(|i| i.first_drawing()).collect();
    for d in firsts {
        let Some(s) = app.engine.project.drawing(d) else { continue };
        let rev = s.tiles().map(|(_, t)| t.revision()).max().unwrap_or(0) ^ (s.tile_count() as u64).rotate_left(40);
        if app.thumbs.get(&d).is_some_and(|(r, _)| *r == rev) {
            continue;
        }
        if budget == 0 {
            ctx.request_repaint();
            break;
        }
        budget -= 1;
        let img = s.to_image().downscaled(80);
        let tex = ctx.load_texture(
            format!("thumb-{}", d.0),
            egui::ColorImage::from_rgba_premultiplied([img.width as usize, img.height as usize], &img.data),
            egui::TextureOptions::LINEAR,
        );
        app.thumbs.insert(d, (rev, tex));
    }
}

/// The frames "Add" takes: the selected frames of the current layer, or the exposure at the cursor.
fn add_range(app: &ToonApp) -> (u32, u32) {
    if app.has_range() {
        let (_, _, a, b) = app.selection();
        return (a, b);
    }
    app.engine
        .project
        .layers
        .get(app.layer)
        .and_then(|l| l.exposure_range(app.frame))
        .map(|(a, b)| (a, b.saturating_sub(1).max(a)))
        .unwrap_or((app.frame, app.frame))
}

pub fn panel(app: &mut ToonApp, ui: &mut egui::Ui) {
    let mut st: State = ui.data(|d| d.get_temp(state_id())).unwrap_or_default();
    egui::ScrollArea::vertical().id_salt("drawing-library-scroll").auto_shrink([false, false]).show(ui, |ui| body(app, ui, &mut st));
    ui.data_mut(|d| d.insert_temp(state_id(), st));
}

fn body(app: &mut ToonApp, ui: &mut egui::Ui, st: &mut State) {
    refresh_thumbnails(app, ui.ctx());
    let (layer, frame) = (app.layer, app.frame);
    let (from, to) = add_range(app);
    ui.horizontal_wrapped(|ui| {
        let l = ui.label(tl!("New item"));
        ui.add(egui::TextEdit::singleline(&mut st.name).hint_text(tl!("Name")).desired_width(140.0)).labelled_by(l.id);
        let shows =
            app.engine.project.layers.get(layer).is_some_and(|l| (from..=to).any(|f| matches!(l.cells.get(f as usize), Some(Cell::Key(_) | Cell::Hold))));
        let add = ui.add_enabled(shows, egui::Button::new(tl!("Add Frames to Library"))).on_hover_text(tlf!(
            "Frames {a} to {b} of the current layer, with their timing",
            a = from + 1,
            b = to + 1
        ));
        if add.clicked() {
            let name = if st.name.trim().is_empty() { tlf!("Drawing {n}", n = app.engine.project.library.len() + 1) } else { st.name.clone() };
            if app.run(Command::SaveToLibrary { layer, from, to, name }) {
                st.chosen = app.engine.project.library.len().saturating_sub(1);
                st.name.clear();
            }
        }
    });
    // only what is drawn: name, length and thumbnail drawing (not the cells)
    let items: Vec<(String, usize, Option<DrawingId>)> = app.engine.project.library.iter().map(|i| (i.name.clone(), i.len(), i.first_drawing())).collect();
    if items.is_empty() {
        ui.weak(tl!("No drawings yet: select frames on the timeline and add them."));
        return;
    }
    st.chosen = st.chosen.min(items.len() - 1);
    ui.separator();
    for (i, (name, len, first)) in items.iter().enumerate() {
        ui.horizontal(|ui| {
            let thumb = first.and_then(|d| app.thumbs.get(&d)).map(|(_, t)| t.clone());
            match thumb {
                Some(t) => {
                    ui.add(egui::Image::new(&t).fit_to_exact_size(egui::vec2(48.0, 27.0)));
                }
                None => {
                    ui.allocate_space(egui::vec2(48.0, 27.0));
                }
            }
            let text = tlf!("{name} ({n} frames)", name = name, n = len);
            if ui.selectable_label(st.chosen == i, text).clicked() {
                st.chosen = i;
            }
        });
    }
    ui.separator();
    let i = st.chosen;
    let Some((name, _, _)) = items.get(i) else { return };
    ui.label(tlf!("Chosen: {name}", name = name));
    ui.horizontal_wrapped(|ui| {
        if ui.button(tl!("Insert Item")).on_hover_text(tl!("On the current layer at the current frame, as new drawings")).clicked() {
            app.run(Command::InsertFromLibrary { item: i, layer, frame, linked: false });
        }
        if ui.button(tl!("Insert Item Linked")).on_hover_text(tl!("Shows the item's own drawings: drawing on one changes every linked use")).clicked() {
            app.run(Command::InsertFromLibrary { item: i, layer, frame, linked: true });
        }
        if ui.button(tl!("Rename Item")).clicked() {
            st.rename = Some(name.clone());
            st.focus_rename = true;
        }
        if ui.add_enabled(i > 0, egui::Button::new(tl!("Move Item Up"))).clicked() && app.run(Command::MoveLibraryItem { item: i, to: i - 1 }) {
            st.chosen = i - 1;
        }
        if ui.add_enabled(i + 1 < items.len(), egui::Button::new(tl!("Move Item Down"))).clicked() && app.run(Command::MoveLibraryItem { item: i, to: i + 1 }) {
            st.chosen = i + 1;
        }
        if ui.button(tl!("Remove from Library")).clicked() {
            app.run(Command::RemoveLibraryItem { item: i });
        }
    });
    if let Some(mut n) = st.rename.take() {
        let mut keep = true;
        ui.horizontal(|ui| {
            let l = ui.label(tl!("New name"));
            let r = ui.add(egui::TextEdit::singleline(&mut n).desired_width(160.0)).labelled_by(l.id);
            if std::mem::take(&mut st.focus_rename) {
                r.request_focus();
            }
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if enter || ui.button(tl!("OK")).clicked() {
                app.run(Command::RenameLibraryItem { item: i, name: n.clone() });
                keep = false;
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                keep = false;
            }
        });
        if keep {
            st.rename = Some(n);
        }
    }
}
