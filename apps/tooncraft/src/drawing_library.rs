//! The Drawing Library panel: the shot's reusable drawings (a blink, a cycle, a prop) with a
//! thumbnail, a name and a length. Add the selected frames of the current layer (or the drawing at
//! the cursor), put an item on the current layer at the cursor as new drawings or linked, rename,
//! reorder and remove items. Every change is an engine command (`SaveToLibrary`, …) and one undo
//! step. The panel's own state (chosen item, typed names) lives in egui's memory.

use tooncraft_doc::{Cell, DrawingId};
use tooncraft_engine::Command;

use std::path::PathBuf;

use crate::app::ToonApp;

#[derive(Clone, Default)]
struct State {
    chosen: usize,
    name: String,
    rename: Option<String>,
    /// Give the rename field the focus once (set when Rename Item is pressed).
    focus_rename: bool,
    mine: Mine,
}

/// The user's own drawing files, kept between shots in a folder of their own.
#[derive(Clone, Default)]
struct Mine {
    /// The folder (`None` until first looked up; `Some(None)`: no place for it on this system).
    dir: Option<Option<PathBuf>>,
    files: Vec<PathBuf>,
    chosen: usize,
    scanned: bool,
}

/// Most entries of the folder looked at, and most files listed.
const MAX_SCAN: usize = 5000;
const MAX_FILES: usize = 500;

/// The folder of the user's drawing files, beside their other settings.
fn default_dir() -> Option<PathBuf> {
    crate::keys::overrides_path().and_then(|p| p.parent().map(|d| d.join("drawings")))
}

/// Use `dir` as the user's drawing folder (tests point it at a temporary folder).
#[cfg(test)]
pub(crate) fn set_folder(ctx: &egui::Context, dir: &std::path::Path) {
    ctx.data_mut(|d| {
        let mut st: State = d.get_temp(state_id()).unwrap_or_default();
        st.mine = Mine { dir: Some(Some(dir.to_path_buf())), ..Mine::default() };
        d.insert_temp(state_id(), st);
    });
}

/// The drawing files in `dir`, sorted by name: at most [`MAX_SCAN`] entries are read and
/// [`MAX_FILES`] listed, so a huge folder cannot stall the panel.
fn scan(dir: &std::path::Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut files: Vec<PathBuf> = rd
        .take(MAX_SCAN)
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == tooncraft_interop::drawing_file::EXTENSION))
        .collect();
    files.sort();
    files.truncate(MAX_FILES);
    files
}

/// A free file name for `name` in `dir`: `<name>.toondrawings`, else `<name> 2…`.
fn free_path(dir: &std::path::Path, name: &str) -> PathBuf {
    let stem = tooncraft_engine::library::clean_category(name);
    let ext = tooncraft_interop::drawing_file::EXTENSION;
    (1..1000)
        .map(|n| if n == 1 { dir.join(format!("{stem}.{ext}")) } else { dir.join(format!("{stem} {n}.{ext}")) })
        .find(|p| !p.exists())
        .unwrap_or_else(|| dir.join(format!("{stem} {}.{ext}", std::process::id())))
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
    if let Some(chosen) = shot_items(app, ui, st) {
        mine(app, ui, st, chosen);
    }
}

/// The shot's items; returns the chosen one (`Some(None)` when there are none), or `None` when
/// the section already showed My Drawings.
fn shot_items(app: &mut ToonApp, ui: &mut egui::Ui, st: &mut State) -> Option<Option<usize>> {
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
        mine(app, ui, st, None);
        return None;
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
            let r = ui.selectable_label(st.chosen == i, text);
            if r.clicked() || r.secondary_clicked() {
                st.chosen = i;
            }
            // right-click, or the Menu key / Shift+F10 on the focused item
            let launch = if st.chosen == i { "library" } else { "" };
            crate::context_menus::attach(ui, &r, launch, |ui| crate::context_menus::library_item(app, ui, i));
        });
    }
    ui.separator();
    let i = st.chosen;
    let Some((name, _, _)) = items.get(i) else { return Some(None) };
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
    Some(Some(i))
}

/// My Drawings: the user's drawing files, shared by every shot. Save the chosen shot item there,
/// add a file to the shot's library, or open and save files anywhere. The folder is read only
/// when this section is opened, and again after a save or Reload.
fn mine(app: &mut ToonApp, ui: &mut egui::Ui, st: &mut State, chosen: Option<usize>) {
    ui.separator();
    let open = egui::CollapsingHeader::new(tl!("My Drawings")).id_salt("my-drawings").show(ui, |ui| {
        let m = &mut st.mine;
        let dir = m.dir.get_or_insert_with(default_dir).clone();
        if !m.scanned {
            m.files = dir.as_deref().map(scan).unwrap_or_default();
            m.chosen = m.chosen.min(m.files.len().saturating_sub(1));
            m.scanned = true;
        }
        let ext = tooncraft_interop::drawing_file::EXTENSION;
        ui.horizontal_wrapped(|ui| {
            let can_save = chosen.is_some() && dir.is_some();
            if ui.add_enabled(can_save, egui::Button::new(tl!("Save Item to My Drawings"))).on_hover_text(tl!("Keep the chosen item for other shots")).clicked()
                && let (Some(item), Some(d)) = (chosen, dir.as_deref())
            {
                let name = app.engine.project.library.get(item).map(|i| i.name.clone()).unwrap_or_default();
                match std::fs::create_dir_all(d) {
                    Ok(()) => {
                        let path = free_path(d, &name);
                        if app.run(Command::SaveLibraryFile { item, path: path.to_string_lossy().into_owned() }) {
                            app.set_status(tlf!("Saved {file}", file = path.display()));
                        }
                    }
                    Err(e) => app.set_status(tlf!("Could not make the folder {dir}: {error}", dir = d.display(), error = e)),
                }
                m.scanned = false;
            }
            if ui.add_enabled(chosen.is_some(), egui::Button::new(tl!("Save Drawing File…"))).clicked()
                && let Some(item) = chosen
                && let Some(f) = rfd::FileDialog::new().add_filter(tl!("Drawing files"), &[ext]).save_file()
            {
                app.run(Command::SaveLibraryFile { item, path: f.to_string_lossy().into_owned() });
            }
            if ui.button(tl!("Open Drawing File…")).clicked()
                && let Some(f) = rfd::FileDialog::new().add_filter(tl!("Drawing files"), &[ext]).pick_file()
            {
                app.run(Command::OpenLibraryFile { path: f.to_string_lossy().into_owned(), placement: Default::default() });
            }
            if ui.button(tl!("Reload")).clicked() {
                m.scanned = false;
            }
        });
        if dir.is_none() {
            ui.weak(tl!("No settings folder on this system, so My Drawings is not available."));
        } else if m.files.is_empty() {
            ui.weak(tl!("No drawing files yet: save an item here to use it in other shots."));
        }
        for (i, f) in m.files.iter().enumerate() {
            let name = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            if ui.selectable_label(m.chosen == i, tlf!("File {name}", name = name)).clicked() {
                m.chosen = i;
            }
        }
        if let Some(f) = m.files.get(m.chosen).cloned()
            && ui.button(tl!("Add File to Shot Library")).clicked()
        {
            app.run(Command::OpenLibraryFile { path: f.to_string_lossy().into_owned(), placement: Default::default() });
        }
    });
    if open.fully_closed() {
        // looked at again when it opens next time
        st.mine.scanned = false;
    }
}
