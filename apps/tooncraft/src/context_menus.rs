//! Right-click menus on the canvas, the exposure grid's cells and the layer names (timeline and
//! Layers panel). Each one repeats the commands of the menu bar where the pointer already is; none
//! holds anything the menu bar cannot reach, so keyboard users lose nothing: Swap Drawing is also
//! in the Frame menu, the layer commands in the Layer menu and the Layers panel.

use egui::{Color32, Ui};
use tooncraft_doc::{Cell, DrawingId, Mark};
use tooncraft_engine::Command;

use crate::app::{Tool, ToonApp};

/// Drawings shown in the swap grid, at most.
pub const MAX_SWAP: usize = 64;

/// Named label colours offered in the menus (the Layers panel has the full picker).
pub fn label_colours() -> [(String, [u8; 3]); 6] {
    [
        (tl!("Red").to_string(), [214, 80, 72]),
        (tl!("Orange").to_string(), [226, 142, 58]),
        (tl!("Yellow").to_string(), [214, 190, 64]),
        (tl!("Green").to_string(), [92, 170, 92]),
        (tl!("Blue").to_string(), [74, 132, 214]),
        (tl!("Violet").to_string(), [152, 98, 200]),
    ]
}

/// The canvas menu. `pick` picks the colour where the menu was opened.
pub fn canvas(app: &mut ToonApp, ui: &mut Ui, pick: &mut dyn FnMut(&mut ToonApp)) {
    if ui.add_enabled(app.engine.can_undo(), egui::Button::new(tl!("Undo"))).clicked() {
        app.undo();
        ui.close();
    }
    if ui.add_enabled(app.engine.can_redo(), egui::Button::new(tl!("Redo"))).clicked() {
        app.redo();
        ui.close();
    }
    ui.separator();
    let has_sel = app.selection.is_some();
    if ui.add_enabled(has_sel, egui::Button::new(tl!("Cut"))).clicked() {
        app.copy_pixels(true);
        ui.close();
    }
    if ui.add_enabled(has_sel, egui::Button::new(tl!("Copy"))).clicked() {
        app.copy_pixels(false);
        ui.close();
    }
    if ui.button(tl!("Paste")).clicked() {
        app.paste_pixels();
        ui.close();
    }
    if ui.button(tl!("Select All")).clicked() {
        app.select_all();
        ui.close();
    }
    if ui.add_enabled(has_sel, egui::Button::new(tl!("Deselect"))).clicked() {
        app.selection = None;
        ui.close();
    }
    if ui.add_enabled(has_sel, egui::Button::new(tl!("Erase Selection"))).clicked() {
        app.delete_selected_pixels();
        ui.close();
    }
    ui.separator();
    if ui.button(tl!("Pick Colour Here")).clicked() {
        pick(app);
        ui.close();
    }
    ui.menu_button(tl!("Tool"), |ui| {
        for t in Tool::ALL {
            if ui.selectable_label(app.tool == t, t.label()).clicked() {
                app.tool = t;
                app.set_status(tlf!("{tool} selected", tool = t.label()));
                ui.close();
            }
        }
    });
    let (layer, frame) = (app.layer, app.frame);
    if ui.button(tl!("New Drawing")).clicked() {
        app.run(Command::NewDrawing { layer, frame });
        ui.close();
    }
    if ui.button(tl!("Clear Frame")).clicked() {
        app.run(Command::ClearCell { layer, frame });
        ui.close();
    }
    ui.separator();
    ui.menu_button(tl!("View"), |ui| {
        let vp = app.workspace.canvas_rect;
        let size = egui::vec2(app.engine.project.width() as f32, app.engine.project.height() as f32);
        if ui.button(tl!("Fit to Window")).clicked() {
            app.view = crate::view::View::fit(vp, size);
            ui.close();
        }
        if ui.button(tl!("Actual Size")).clicked() {
            app.view.zoom = 1.0;
            ui.close();
        }
        if ui.button(tl!("Reset Rotation")).clicked() {
            app.view.rotation = 0.0;
            ui.close();
        }
        ui.checkbox(&mut app.view.flip, tl!("Mirror View"));
    });
    ui.checkbox(&mut app.engine.project.onion.enabled, tl!("Onion Skin"));
    ui.checkbox(&mut app.engine.project.light_table.enabled, tl!("Light Table"));
}

/// The drawings a layer can swap between, at most [`MAX_SWAP`]: those it shows, in the order
/// they first appear, then the shot's drawings no layer shows (kept in the library after a swap,
/// a clear or a re-time), so a swap can always be undone by swapping back.
pub fn layer_drawings(app: &ToonApp, layer: usize) -> Vec<DrawingId> {
    let mut out = exposed_drawings(app, layer);
    for d in app.engine.project.unused_drawings() {
        if out.len() >= MAX_SWAP {
            break;
        }
        if !out.contains(&d) {
            out.push(d);
        }
    }
    out
}

/// The drawings a layer shows, in the order they first appear (at most [`MAX_SWAP`]).
fn exposed_drawings(app: &ToonApp, layer: usize) -> Vec<DrawingId> {
    let mut out: Vec<DrawingId> = Vec::new();
    if let Some(l) = app.engine.project.layers.get(layer) {
        for c in &l.cells {
            if let Cell::Key(d) = c
                && !out.contains(d)
            {
                out.push(*d);
                if out.len() >= MAX_SWAP {
                    break;
                }
            }
        }
    }
    out
}

fn drawing_label(app: &ToonApp, d: DrawingId) -> String {
    match app.engine.project.drawing_names.get(&d) {
        Some(name) => tlf!("Drawing {id}, {name}", id = d.0, name = name),
        None => tlf!("Drawing {id}", id = d.0),
    }
}

/// The grid of a layer's drawings, plus Empty: a click shows it for the whole exposure.
pub fn swap_grid(app: &mut ToonApp, ui: &mut Ui, layer: usize, frame: u32) {
    let current = app.engine.project.layers.get(layer).and_then(|l| l.drawing_at(frame));
    let drawings = layer_drawings(app, layer);
    let mut chosen: Option<Option<DrawingId>> = None;
    egui::Grid::new(("swap-grid", layer, frame)).spacing(egui::vec2(4.0, 4.0)).show(ui, |ui| {
        for (i, d) in drawings.iter().enumerate() {
            let label = drawing_label(app, *d);
            let resp = match app.thumbs.get(d).filter(|_| app.thumbnails) {
                Some((_, t)) => ui.add(egui::Button::image(egui::Image::new(t).fit_to_exact_size(egui::vec2(56.0, 40.0))).selected(current == Some(*d))),
                None => ui.add(egui::Button::new(format!("{}", d.0)).min_size(egui::vec2(56.0, 40.0)).selected(current == Some(*d))),
            };
            let resp = resp.on_hover_text(&label);
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, current == Some(*d), label.clone()));
            if resp.clicked() {
                chosen = Some(Some(*d));
            }
            if i % 4 == 3 {
                ui.end_row();
            }
        }
        if ui.add(egui::Button::new(tl!("Empty")).min_size(egui::vec2(56.0, 40.0)).selected(current.is_none())).clicked() {
            chosen = Some(None);
        }
    });
    if let Some(drawing) = chosen {
        if app.run(Command::SwapDrawing { layer, frame, drawing }) {
            app.set_status(match drawing {
                Some(d) => tlf!("Now showing drawing {id}", id = d.0),
                None => tl!("Exposure emptied").to_string(),
            });
        }
        ui.close();
    }
}

/// The menu on an exposure-grid cell.
pub fn cell(app: &mut ToonApp, ui: &mut Ui, layer: usize, frame: u32) {
    let key = app.engine.project.layers.get(layer).and_then(|l| l.drawing_at(frame));
    ui.menu_button(tl!("Swap Drawing"), |ui| swap_grid(app, ui, layer, frame));
    ui.add_enabled_ui(key.is_some(), |ui| {
        ui.menu_button(tl!("Mark Drawing"), |ui| {
            let current = key.and_then(|d| app.engine.project.marks.get(&d).copied());
            let options: [(Option<Mark>, String); 4] = [
                (Some(Mark::Key), tl!("Key").to_string()),
                (Some(Mark::Breakdown), tl!("Breakdown").to_string()),
                (Some(Mark::Inbetween), tl!("Inbetween").to_string()),
                (None, tl!("No Mark").to_string()),
            ];
            for (mark, name) in options {
                if ui.selectable_label(current == mark, name).clicked() {
                    app.run(Command::SetMark { layer, frame, mark });
                    ui.close();
                }
            }
        });
    });
    ui.separator();
    let items: [(String, Command); 7] = [
        (tl!("New Drawing").to_string(), Command::NewDrawing { layer, frame }),
        (tl!("Duplicate Drawing").to_string(), Command::DuplicateDrawing { layer, frame }),
        (tl!("Insert Frame").to_string(), Command::InsertFrame { layer: Some(layer), frame }),
        (tl!("Delete Frame").to_string(), Command::DeleteFrame { layer: Some(layer), frame }),
        (tl!("Hold One Frame Longer").to_string(), Command::ExtendHold { layer, frame }),
        (tl!("Hold One Frame Shorter").to_string(), Command::ShortenHold { layer, frame }),
        (tl!("Clear Frame").to_string(), Command::ClearCell { layer, frame }),
    ];
    for (label, cmd) in items {
        if ui.button(label).clicked() {
            app.run(cmd);
            ui.close();
        }
    }
    ui.add_enabled_ui(key.is_some(), |ui| {
        ui.menu_button(tl!("Hold For"), |ui| {
            for length in 1..=8u32 {
                if ui.button(tlf!("{n} frames", n = length)).clicked() {
                    app.run(Command::SetExposure { layer, frame, length });
                    ui.close();
                }
            }
        });
    });
    if ui.button(tl!("Fill Empty Frames")).on_hover_text(tl!("Hold each drawing of the layer until the next one")).clicked() {
        app.run(Command::FillEmpty { layer });
        ui.close();
    }
    ui.separator();
    if ui.button(tl!("Copy Frames")).clicked() {
        app.copy_cells();
        ui.close();
    }
    if ui.button(tl!("Cut Frames")).clicked() {
        app.cut_cells();
        ui.close();
    }
    if ui.add_enabled(app.cell_clipboard.is_some(), egui::Button::new(tl!("Paste Frames"))).clicked() {
        app.paste_cells(false, false);
        ui.close();
    }
    if ui.add_enabled(key.is_some(), egui::Button::new(tl!("Pin to Light Table"))).clicked() {
        app.run(Command::TogglePin { layer, frame });
        ui.close();
    }
}

/// The menu on a layer's name (timeline and Layers panel). It acts on that layer.
pub fn layer(app: &mut ToonApp, ui: &mut Ui, li: usize) {
    let Some(l) = app.engine.project.layers.get(li) else { return };
    let (visible, locked, clip, lock_alpha, color) = (l.visible, l.locked, l.clip, l.lock_alpha, l.color);
    let n = app.engine.project.layers.len();
    app.layer = li;
    if ui.button(if visible { tl!("Hide Layer") } else { tl!("Show Layer") }).clicked() {
        app.run(Command::SetLayer { layer: li, visible: Some(!visible), locked: None, opacity: None });
        ui.close();
    }
    if ui.button(if locked { tl!("Unlock Layer") } else { tl!("Lock Layer") }).clicked() {
        app.run(Command::SetLayer { layer: li, visible: None, locked: Some(!locked), opacity: None });
        ui.close();
    }
    if ui.button(if lock_alpha { tl!("Unlock Transparency") } else { tl!("Lock Transparency") }).clicked() {
        app.run(Command::SetLayerLockAlpha { layer: li, lock: !lock_alpha });
        ui.close();
    }
    if ui.add_enabled(li > 0, egui::Button::new(if clip { tl!("Stop Clipping") } else { tl!("Clip to Layer Below") })).clicked() {
        app.run(Command::SetLayerClip { layer: li, clip: !clip });
        ui.close();
    }
    ui.separator();
    if ui.button(tl!("Rename Layer…")).clicked() {
        let name = app.engine.project.layers.get(li).map(|l| l.name.clone()).unwrap_or_default();
        app.renaming = Some((li, name));
        app.workspace.raise(crate::dock::Pane::Layers);
        ui.close();
    }
    if ui.button(tl!("Add Layer Above")).clicked() {
        let name = tlf!("Layer {n}", n = n + 1);
        if app.run(Command::AddLayer { name, index: Some(li + 1) }) {
            app.layer = li + 1;
        }
        ui.close();
    }
    if ui.button(tl!("Duplicate Layer")).clicked() {
        if app.run(Command::DuplicateLayer { layer: li }) {
            app.layer = li + 1;
        }
        ui.close();
    }
    crate::colour_layers_ui::menu_items(app, ui, li);
    if ui.add_enabled(li > 0, egui::Button::new(tl!("Merge Layer Down"))).clicked() {
        crate::layer_ops::merge(app, Command::MergeDown { layer: li }, Some(li.saturating_sub(1)));
        ui.close();
    }
    if ui.add_enabled(li + 1 < n, egui::Button::new(tl!("Move Layer Up"))).clicked() {
        if app.run(Command::MoveLayer { from: li, to: li + 1 }) {
            app.layer = li + 1;
        }
        ui.close();
    }
    if ui.add_enabled(li > 0, egui::Button::new(tl!("Move Layer Down"))).clicked() {
        if app.run(Command::MoveLayer { from: li, to: li - 1 }) {
            app.layer = li - 1;
        }
        ui.close();
    }
    ui.menu_button(tl!("Label Colour"), |ui| {
        for (name, c) in label_colours() {
            let text = egui::RichText::new(name).color(Color32::from_rgb(c[0], c[1], c[2])).strong();
            let resp = ui.selectable_label(color == Some(c), text);
            if resp.clicked() {
                app.run(Command::SetLayerColor { layer: li, color: Some(c) });
                ui.close();
            }
        }
        if ui.selectable_label(color.is_none(), tl!("No Colour")).clicked() {
            app.run(Command::SetLayerColor { layer: li, color: None });
            ui.close();
        }
    });
    ui.separator();
    if ui.add_enabled(n > 1, egui::Button::new(tl!("Delete Layer"))).clicked() {
        app.run(Command::DeleteLayer { layer: li });
        app.clamp_selection();
        ui.close();
    }
}
