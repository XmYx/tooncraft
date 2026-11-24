//! The right-click menus of design §5, built from registry commands ([`crate::commands`]): each
//! item has its icon, label and key, and a disabled item stays visible with the reason in its
//! tooltip. Menus are at most two levels deep. Every menu also opens from the keyboard (the Menu
//! key or Shift+F10 on the focused widget, see [`attach`]) and at launch with `--context-menu`.
//! Every item is also in the menu bar or a panel, so the menus are a fast path, not the only one.

use egui::{Response, Ui};
use tooncraft_doc::{Cell, DrawingId};
use tooncraft_engine::Command;

use crate::app::{Tool, ToonApp};
use crate::dock::Pane;
use crate::menu_commands::Target;
use crate::menus::{SEP, context};

/// Drawings shown in the swap grid, at most.
pub const MAX_SWAP: usize = 64;

/// Open `add` as the right-click menu of `resp`, and from the keyboard (Menu key or Shift+F10
/// while `resp` has the focus), or at launch when `--context-menu` named `launch`.
pub fn attach(ui: &Ui, resp: &Response, launch: &str, mut add: impl FnMut(&mut Ui)) {
    let now = crate::context_menus::launch_now(ui.ctx(), launch) || resp.has_focus() && crate::timeline::menu_key(ui);
    crate::timeline::keyboard_menu(ui, resp, now, &mut add);
    resp.context_menu(&mut add);
}

/// Whether a widget whose menu is drawn only on demand needs it this frame: right-clicked, its
/// menu open (from the pointer or the keyboard), focused (the Menu key may come), or named by a
/// pending `--context-menu`.
pub fn wants_menu(ui: &Ui, resp: &Response, launch: &str) -> bool {
    resp.secondary_clicked()
        || resp.context_menu_opened()
        || resp.has_focus()
        || egui::Popup::is_id_open(ui.ctx(), resp.id.with("keyboard-menu"))
        || !launch.is_empty() && ui.ctx().data(|d| d.get_temp::<String>(launch_id())).is_some_and(|t| t == launch)
}

/// A submenu of registry commands.
fn sub(ui: &mut Ui, app: &mut ToonApp, label: &str, ids: &[&str]) {
    ui.menu_button(label, |ui| {
        context(ui, app, ids);
    });
}

fn set_target(app: &mut ToonApp, t: Target) {
    if app.menus.target != t {
        app.menus.target = t;
    }
}

/// The canvas menu. `at` is the picture point it was opened on (for Pick Colour Here). With a
/// selection under the pointer (or any selection, from the keyboard) it is the selection menu.
pub fn canvas(app: &mut ToonApp, ui: &mut Ui, at: Option<(f32, f32)>) {
    let on_selection = match (at, crate::selection_tools::mask(app)) {
        (_, None) => false,
        (None, Some(_)) => true,
        (Some((x, y)), Some(m)) => x >= 0.0 && y >= 0.0 && m.get(x as u32, y as u32),
    };
    set_target(app, at.map_or(Target::None, |(x, y)| Target::Point(x, y)));
    if on_selection {
        return selection(app, ui);
    }
    context(
        ui,
        app,
        &["edit.undo", "edit.redo", SEP, "selection.cut", "selection.copy", "selection.paste", "edit.select_all", "edit.deselect", SEP, "canvas.pick_colour"],
    );
    let tools: Vec<String> = Tool::ALL.iter().map(|t| format!("tool.{}", t.id())).collect();
    let tools: Vec<&str> = tools.iter().map(String::as_str).collect();
    sub(ui, app, tl!("Tool"), &tools);
    context(ui, app, &[SEP, "frame.new_drawing", "edit.clear_drawing", SEP]);
    sub(ui, app, tl!("Flip"), &["canvas.flip_h", "canvas.flip_v"]);
    sub(ui, app, tl!("Rotate 90°"), &["canvas.rotate_cw", "canvas.rotate_ccw"]);
    ui.separator();
    sub(ui, app, tl!("View"), &["view.fit", "view.actual_size", "view.reset_rotation", "view.mirror", SEP, "view.onion", "view.light_table"]);
}

/// The menu on the canvas selection.
pub fn selection(app: &mut ToonApp, ui: &mut Ui) {
    context(
        ui,
        app,
        &[
            "selection.transform",
            "select.flip_h",
            "select.flip_v",
            "select.rotate_cw",
            SEP,
            "selection.copy",
            "selection.cut",
            "select.erase",
            SEP,
            "selection.fill",
            "selection.stroke",
            SEP,
            "selection.save_library",
            "edit.deselect",
        ],
    );
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

/// The menu on an exposure-grid cell (the cell is the current one).
pub fn cell(app: &mut ToonApp, ui: &mut Ui, layer: usize, frame: u32) {
    set_target(app, Target::None);
    ui.menu_button(tl!("Swap Drawing"), |ui| swap_grid(app, ui, layer, frame));
    sub(
        ui,
        app,
        tl!("Hold For"),
        &[
            "cell.hold_1",
            "cell.hold_2",
            "cell.hold_3",
            "cell.hold_4",
            "cell.hold_5",
            "cell.hold_6",
            "cell.hold_7",
            "cell.hold_8",
            SEP,
            "frame.hold_more",
            "frame.hold_less",
        ],
    );
    sub(ui, app, tl!("Mark As"), &["cell.mark_key", "cell.mark_breakdown", "cell.mark_inbetween", "cell.mark_none"]);
    context(
        ui,
        app,
        &[
            SEP,
            "frame.new_drawing",
            "frame.duplicate",
            SEP,
            "frame.insert",
            "frame.delete",
            "cell.fill_empty",
            "frame.clear",
            SEP,
            "cell.cut",
            "cell.copy",
            "cell.paste",
            "cell.paste_reversed",
            SEP,
        ],
    );
    sub(ui, app, tl!("Go To"), &["frame.next_drawing", "frame.prev_drawing", "frame.next_key", "frame.prev_key"]);
    context(ui, app, &["view.pin"]);
}

/// The menu on the timeline ruler, opened on `frame`.
pub fn ruler(app: &mut ToonApp, ui: &mut Ui, frame: u32) {
    set_target(app, Target::Ruler(frame));
    context(ui, app, &["ruler.loop_in", "ruler.loop_out", "ruler.clear_loop", SEP]);
    sub(ui, app, tl!("Onion Range"), &["onion.range_1", "onion.range_2", "onion.range_3", "onion.range_5", SEP, "onion.keys_only"]);
    ui.separator();
    // Go To Frame: typed in place, no dialog
    let len = i64::from(app.engine.project.length().max(1));
    if app.menus.go_to < 1 {
        app.menus.go_to = i64::from(frame) + 1;
    }
    ui.horizontal(|ui| {
        let l = ui.label(tl!("Go To Frame"));
        let r = ui.add(egui::DragValue::new(&mut app.menus.go_to).range(1..=len)).labelled_by(l.id);
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.button(tl!("Go")).clicked() || enter {
            let n = app.menus.go_to;
            crate::menu_actions::go_to_frame(app, n);
            app.menus.go_to = 0;
            ui.close();
        }
    });
}

/// The menu on a layer's row (timeline, sheet and Layers panel). The layer becomes the current one.
pub fn layer(app: &mut ToonApp, ui: &mut Ui, li: usize) {
    if li >= app.engine.project.layers.len() {
        return;
    }
    app.layer = li;
    set_target(app, Target::None);
    context(ui, app, &["layer.visible", "layer.lock", "layer.lock_alpha", "layer.onion", SEP, "layer.rename"]);
    sub(
        ui,
        app,
        tl!("Label Colour"),
        &[
            "layer.label_red",
            "layer.label_orange",
            "layer.label_yellow",
            "layer.label_green",
            "layer.label_blue",
            "layer.label_violet",
            SEP,
            "layer.label_none",
        ],
    );
    context(ui, app, &[SEP, "layer.add_above", "layer.duplicate", "layer.delete", SEP, "layer.merge_down", "layer.add_peg", SEP]);
    sub(ui, app, tl!("Opacity"), &["layer.opacity_100", "layer.opacity_75", "layer.opacity_50", "layer.opacity_25", "layer.opacity_10"]);
    let blends: Vec<&str> = crate::menu_commands::BLENDS.iter().map(|(id, _)| *id).collect();
    sub(ui, app, tl!("Blend"), &blends);
    sub(ui, app, tl!("Arrange"), &["layer.move_up", "layer.move_down", SEP, "layer.clip"]);
    crate::colour_layers_ui::menu_items(app, ui, li);
}

/// The menu on a swatch of the palette shown.
pub fn swatch(app: &mut ToonApp, ui: &mut Ui, index: usize, rgb: [u8; 3]) {
    set_target(app, Target::Swatch(index, rgb));
    context(ui, app, &["swatch.use", "swatch.edit", "swatch.rename", "swatch.duplicate", "swatch.delete", SEP, "swatch.select_pixels"]);
}

/// The menu on a brush of the library.
pub fn brush_preset(app: &mut ToonApp, ui: &mut Ui, id: &str) {
    set_target(app, Target::Preset(id.to_string()));
    context(ui, app, &["preset.use", "preset.rename", "preset.duplicate", "preset.delete", "preset.save_into", "preset.export"]);
}

/// The menu on an item of the shot's drawing library.
pub fn library_item(app: &mut ToonApp, ui: &mut Ui, item: usize) {
    set_target(app, Target::Item(item));
    context(ui, app, &["item.place", "item.place_layer", SEP, "item.rename", "item.delete", SEP, "item.show_folder"]);
}

/// The tool button's menu after its variants (the rail's flyout draws those): reset the tool's
/// brush, and the shortcut editor at the tool.
pub fn tool_button_extras(app: &mut ToonApp, ui: &mut Ui, tool: Tool) {
    set_target(app, Target::Tool(tool));
    context(ui, app, &["tool.reset_settings", "tool.shortcut"]);
}

/// The panel tab's own items (the docking library adds Float, Move To and Close).
pub fn panel_tab(app: &mut ToonApp, ui: &mut Ui, pane: Pane) {
    set_target(app, Target::Pane(pane));
    context(ui, app, &["panel.reset"]);
}

/// Once a frame: hand a pending `--context-menu` to the widgets, and show the rename window.
pub fn frame(app: &mut ToonApp, ctx: &egui::Context) {
    if let Some(target) = app.menus.launch.take() {
        ctx.data_mut(|d| d.insert_temp(launch_id(), target));
    }
    rename_window(app, ctx);
}

/// The small window for a name typed from a menu (Rename Brush…, Rename Item…).
fn rename_window(app: &mut ToonApp, ctx: &egui::Context) {
    let Some((what, mut text)) = app.menus.rename.take() else { return };
    let mut keep = true;
    let mut open = true;
    egui::Window::new(tl!("Rename")).open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_TOP, [0.0, 80.0]).show(ctx, |ui| {
        ui.horizontal(|ui| {
            let l = ui.label(tl!("New name"));
            let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(200.0)).labelled_by(l.id);
            if !r.has_focus() && !r.lost_focus() {
                r.request_focus();
            }
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (enter || ui.button(tl!("OK")).clicked()) && !text.trim().is_empty() {
                crate::menu_commands::apply_rename(app, &what, &text);
                keep = false;
            }
        });
    });
    if open && keep && !ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.menus.rename = Some((what, text));
    }
}

fn launch_id() -> egui::Id {
    egui::Id::unique("tooncraft-launch-context-menu")
}

/// `--context-menu <target>`: remember the menu to open on the first frame, and set the shot up
/// so the menu has something to act on (a selection for the selection menu).
pub fn prepare_launch(app: &mut ToonApp, target: &str) {
    if target == "selection" && app.selection.is_none() {
        let (w, h) = (app.engine.project.width() as f32, app.engine.project.height() as f32);
        crate::selection_tools::set_rings(app, vec![vec![(w * 0.3, h * 0.3), (w * 0.6, h * 0.3), (w * 0.6, h * 0.65), (w * 0.3, h * 0.65)]]);
        app.tool = Tool::Select;
    }
    app.menus.launch = Some(target.to_string());
}

/// Whether the launch asked for `target`'s menu; true once, then it is forgotten.
pub fn launch_now(ctx: &egui::Context, target: &str) -> bool {
    let wanted = ctx.data(|d| d.get_temp::<String>(launch_id())).is_some_and(|t| t == target);
    if wanted {
        ctx.data_mut(|d| d.remove::<String>(launch_id()));
    }
    wanted
}
