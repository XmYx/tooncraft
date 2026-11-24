//! The right-click menus' registry commands (design §5), as the command table calls them: each
//! `run_*` has the registry's `fn(&mut ToonApp, &egui::Context, &'static str)` shape and each
//! `can_*` its `enabled` shape. Commands that act on something other than the current layer and
//! frame (a swatch, a brush preset, a library item, the ruler frame, a tool or a panel) read it
//! from [`State::target`], which the menu sets when it opens. The logic lives in `menu_actions`.

use tooncraft_doc::{BlendMode, Mark};
use tooncraft_engine::Command;

use crate::app::{Tool, ToonApp};
use crate::dock::Pane;
use crate::menu_actions as act;

type Enabled = Result<(), String>;

/// What a right-click menu was opened on, beyond the current layer and frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Target {
    #[default]
    None,
    /// A point of the picture (canvas pixels).
    Point(f32, f32),
    /// A frame on the timeline ruler (0-based).
    Ruler(u32),
    /// A swatch of the palette shown: index and colour.
    Swatch(usize, [u8; 3]),
    /// A brush of the library, by id.
    Preset(String),
    /// An item of the shot's drawing library.
    Item(usize),
    Tool(Tool),
    Pane(Pane),
}

/// The menus' state: what the open menu acts on, and a name being typed for a rename.
#[derive(Clone, Debug, Default)]
pub struct State {
    pub target: Target,
    pub rename: Option<(Target, String)>,
    /// The frame typed in the ruler's Go To Frame field (1-based).
    pub go_to: i64,
}

fn target(a: &ToonApp) -> &Target {
    &a.menus.target
}

// ----- canvas -----

pub fn can_pick(a: &ToonApp) -> Enabled {
    match target(a) {
        Target::Point(x, y) if *x >= 0.0 && *y >= 0.0 => Ok(()),
        _ => Err(tl!("Open the menu on the canvas to pick there").to_string()),
    }
}

pub fn run_pick(a: &mut ToonApp, _: &egui::Context, _: &'static str) {
    if let Target::Point(x, y) = *target(a) {
        act::pick_colour_at(a, x, y);
    }
}

pub fn can_transform(a: &ToonApp) -> Enabled {
    act::can_transform(a)
}

pub fn run_canvas_transform(a: &mut ToonApp, _: &egui::Context, id: &'static str) {
    match id {
        "canvas.flip_h" => act::flip(a, true),
        "canvas.flip_v" => act::flip(a, false),
        "canvas.rotate_cw" => act::rotate_90(a, true),
        _ => act::rotate_90(a, false),
    }
}

pub fn can_paste_pixels(a: &ToonApp) -> Enabled {
    if a.pixel_clipboard.is_none() { Err(tl!("Nothing copied yet").to_string()) } else { Ok(()) }
}

// ----- selection -----

pub fn can_paint_selection(a: &ToonApp) -> Enabled {
    act::can_paint_selection(a)
}

pub fn has_selection(a: &ToonApp) -> Enabled {
    act::has_selection(a)
}

pub fn run_selection(a: &mut ToonApp, _: &egui::Context, id: &'static str) {
    match id {
        "selection.transform" => {
            a.tool = Tool::Select;
            a.set_status(tl!("Drag the handles to move, scale or turn the selection"));
        }
        "selection.copy" => a.copy_pixels(false),
        "selection.cut" => a.copy_pixels(true),
        "selection.paste" => a.paste_pixels(),
        "selection.fill" => act::fill_selection(a),
        "selection.stroke" => act::stroke_selection(a),
        _ => act::save_cell_to_library(a),
    }
}

pub fn can_save_to_library(a: &ToonApp) -> Enabled {
    act::has_drawing(a)
}

// ----- timeline cell -----

pub fn has_drawing(a: &ToonApp) -> Enabled {
    act::has_drawing(a)
}

/// `cell.hold_N`: show the drawing for N frames.
pub fn run_hold(a: &mut ToonApp, _: &egui::Context, id: &'static str) {
    let Some(length) = id.strip_prefix("cell.hold_").and_then(|n| n.parse::<u32>().ok()) else { return };
    let (layer, frame) = (a.layer, a.frame);
    a.run(Command::SetExposure { layer, frame, length });
}

fn mark_of(id: &str) -> Option<Mark> {
    match id {
        "cell.mark_key" => Some(Mark::Key),
        "cell.mark_breakdown" => Some(Mark::Breakdown),
        "cell.mark_inbetween" => Some(Mark::Inbetween),
        _ => None,
    }
}

pub fn run_mark(a: &mut ToonApp, _: &egui::Context, id: &'static str) {
    let (layer, frame) = (a.layer, a.frame);
    a.run(Command::SetMark { layer, frame, mark: mark_of(id) });
}

fn current_mark(a: &ToonApp) -> Option<Mark> {
    let d = a.engine.project.drawing_at(a.layer, a.frame)?;
    a.engine.project.marks.get(&d).copied()
}

pub fn is_key(a: &ToonApp) -> bool {
    current_mark(a) == Some(Mark::Key)
}
pub fn is_breakdown(a: &ToonApp) -> bool {
    current_mark(a) == Some(Mark::Breakdown)
}
pub fn is_inbetween(a: &ToonApp) -> bool {
    current_mark(a) == Some(Mark::Inbetween)
}
pub fn is_unmarked(a: &ToonApp) -> bool {
    a.engine.project.drawing_at(a.layer, a.frame).is_some() && current_mark(a).is_none()
}

pub fn run_cells(a: &mut ToonApp, _: &egui::Context, id: &'static str) {
    match id {
        "cell.copy" => a.copy_cells(),
        "cell.cut" => a.cut_cells(),
        "cell.paste" => a.paste_cells(false, false),
        "cell.paste_reversed" => act::paste_reversed(a),
        _ => {
            let layer = a.layer;
            a.run(Command::FillEmpty { layer });
        }
    }
}

pub fn can_paste_cells(a: &ToonApp) -> Enabled {
    act::can_paste_cells(a)
}

// ----- ruler -----

fn ruler_frame(a: &ToonApp) -> u32 {
    match target(a) {
        Target::Ruler(f) => *f,
        _ => a.frame,
    }
}

pub fn run_ruler(a: &mut ToonApp, _: &egui::Context, id: &'static str) {
    let f = ruler_frame(a);
    match id {
        "ruler.loop_in" => act::set_loop_at(a, f, true),
        "ruler.loop_out" => act::set_loop_at(a, f, false),
        _ => act::clear_loop(a),
    }
}

pub fn has_loop(a: &ToonApp) -> Enabled {
    act::has_loop(a)
}

fn range_of(id: &str) -> u8 {
    id.strip_prefix("onion.range_").and_then(|n| n.parse().ok()).unwrap_or(1)
}

pub fn run_onion_range(a: &mut ToonApp, _: &egui::Context, id: &'static str) {
    act::onion_range(a, range_of(id));
}

fn range_is(a: &ToonApp, n: u8) -> bool {
    let o = a.engine.project.onion;
    o.enabled && o.before == n && o.after == n
}
pub fn range_1(a: &ToonApp) -> bool {
    range_is(a, 1)
}
pub fn range_2(a: &ToonApp) -> bool {
    range_is(a, 2)
}
pub fn range_3(a: &ToonApp) -> bool {
    range_is(a, 3)
}
pub fn range_5(a: &ToonApp) -> bool {
    range_is(a, 5)
}

pub fn run_keys_only(a: &mut ToonApp, _: &egui::Context, _: &'static str) {
    let on = !a.engine.project.onion.keys_only;
    act::onion_keys_only(a, on);
}

pub fn keys_only(a: &ToonApp) -> bool {
    a.engine.project.onion.keys_only
}

// ----- layer row (the menu makes its layer the current one) -----

fn layer(a: &ToonApp) -> Option<&tooncraft_doc::Layer> {
    a.engine.project.layers.get(a.layer)
}

pub fn run_layer_flag(a: &mut ToonApp, _: &egui::Context, id: &'static str) {
    let li = a.layer;
    let Some(l) = layer(a) else { return };
    let (visible, locked, lock_alpha, clip) = (l.visible, l.locked, l.lock_alpha, l.clip);
    match id {
        "layer.visible" => {
            a.run(Command::SetLayer { layer: li, visible: Some(!visible), locked: None, opacity: None });
        }
        "layer.lock" => {
            a.run(Command::SetLayer { layer: li, visible: None, locked: Some(!locked), opacity: None });
        }
        "layer.lock_alpha" => {
            a.run(Command::SetLayerLockAlpha { layer: li, lock: !lock_alpha });
        }
        "layer.clip" => {
            a.run(Command::SetLayerClip { layer: li, clip: !clip });
        }
        _ => {
            let on = !act::layer_in_onion(a, li);
            act::set_layer_in_onion(a, li, on);
        }
    }
}

pub fn is_visible(a: &ToonApp) -> bool {
    layer(a).is_some_and(|l| l.visible)
}
pub fn is_locked(a: &ToonApp) -> bool {
    layer(a).is_some_and(|l| l.locked)
}
pub fn is_alpha_locked(a: &ToonApp) -> bool {
    layer(a).is_some_and(|l| l.lock_alpha)
}
pub fn is_clipped(a: &ToonApp) -> bool {
    layer(a).is_some_and(|l| l.clip)
}
pub fn in_onion(a: &ToonApp) -> bool {
    act::layer_in_onion(a, a.layer)
}

pub fn not_bottom(a: &ToonApp) -> Enabled {
    if a.layer == 0 { Err(tl!("There is no layer below").to_string()) } else { Ok(()) }
}

pub fn not_top(a: &ToonApp) -> Enabled {
    if a.layer + 1 >= a.engine.project.layers.len() { Err(tl!("There is no layer above").to_string()) } else { Ok(()) }
}

pub fn not_last(a: &ToonApp) -> Enabled {
    if a.engine.project.layers.len() <= 1 { Err(tl!("A shot keeps at least one layer").to_string()) } else { Ok(()) }
}

pub fn run_layer(a: &mut ToonApp, _: &egui::Context, id: &'static str) {
    let li = a.layer;
    let n = a.engine.project.layers.len();
    match id {
        "layer.rename" => {
            let name = layer(a).map(|l| l.name.clone()).unwrap_or_default();
            a.renaming = Some((li, name));
            a.workspace.raise(Pane::Layers);
        }
        "layer.add_above" => {
            let name = tlf!("Layer {n}", n = n + 1);
            if a.run(Command::AddLayer { name, index: Some(li + 1) }) {
                a.layer = li + 1;
            }
        }
        "layer.duplicate" => {
            if a.run(Command::DuplicateLayer { layer: li }) {
                a.layer = li + 1;
            }
        }
        "layer.delete" => {
            a.run(Command::DeleteLayer { layer: li });
            a.clamp_selection();
        }
        "layer.merge_down" => crate::layer_ops::merge(a, Command::MergeDown { layer: li }, Some(li.saturating_sub(1))),
        "layer.add_peg" => act::add_peg(a, li),
        "layer.move_up" => {
            if a.run(Command::MoveLayer { from: li, to: li + 1 }) {
                a.layer = li + 1;
            }
        }
        _ => {
            if a.run(Command::MoveLayer { from: li, to: li.saturating_sub(1) }) {
                a.layer = li.saturating_sub(1);
            }
        }
    }
}

pub fn can_add_peg(a: &ToonApp) -> Enabled {
    act::can_add_peg(a, a.layer)
}

/// Label colours offered in the layer menu (the Layers panel has the full picker).
pub const LABELS: [(&str, [u8; 3]); 6] = [
    ("layer.label_red", [214, 80, 72]),
    ("layer.label_orange", [226, 142, 58]),
    ("layer.label_yellow", [214, 190, 64]),
    ("layer.label_green", [92, 170, 92]),
    ("layer.label_blue", [74, 132, 214]),
    ("layer.label_violet", [152, 98, 200]),
];

fn label_of(id: &str) -> Option<[u8; 3]> {
    LABELS.iter().find(|(i, _)| *i == id).map(|(_, c)| *c)
}

pub fn run_label(a: &mut ToonApp, _: &egui::Context, id: &'static str) {
    let layer = a.layer;
    a.run(Command::SetLayerColor { layer, color: label_of(id) });
}

fn label_is(a: &ToonApp, id: &str) -> bool {
    layer(a).is_some_and(|l| l.color == label_of(id))
}
pub fn label_red(a: &ToonApp) -> bool {
    label_is(a, "layer.label_red")
}
pub fn label_orange(a: &ToonApp) -> bool {
    label_is(a, "layer.label_orange")
}
pub fn label_yellow(a: &ToonApp) -> bool {
    label_is(a, "layer.label_yellow")
}
pub fn label_green(a: &ToonApp) -> bool {
    label_is(a, "layer.label_green")
}
pub fn label_blue(a: &ToonApp) -> bool {
    label_is(a, "layer.label_blue")
}
pub fn label_violet(a: &ToonApp) -> bool {
    label_is(a, "layer.label_violet")
}
pub fn label_none(a: &ToonApp) -> bool {
    label_is(a, "layer.label_none")
}

pub fn run_opacity(a: &mut ToonApp, _: &egui::Context, id: &'static str) {
    let percent = id.strip_prefix("layer.opacity_").and_then(|n| n.parse().ok()).unwrap_or(100);
    act::set_layer_opacity(a, a.layer, percent);
}

fn opacity_is(a: &ToonApp, percent: u8) -> bool {
    layer(a).is_some_and(|l| (l.opacity * 100.0).round() as i32 == i32::from(percent))
}
pub fn opacity_100(a: &ToonApp) -> bool {
    opacity_is(a, 100)
}
pub fn opacity_75(a: &ToonApp) -> bool {
    opacity_is(a, 75)
}
pub fn opacity_50(a: &ToonApp) -> bool {
    opacity_is(a, 50)
}
pub fn opacity_25(a: &ToonApp) -> bool {
    opacity_is(a, 25)
}
pub fn opacity_10(a: &ToonApp) -> bool {
    opacity_is(a, 10)
}

/// The blend modes offered in the layer menu (the Layers panel lists them all).
pub const BLENDS: [(&str, BlendMode); 8] = [
    ("layer.blend_normal", BlendMode::Normal),
    ("layer.blend_multiply", BlendMode::Multiply),
    ("layer.blend_screen", BlendMode::Screen),
    ("layer.blend_overlay", BlendMode::Overlay),
    ("layer.blend_darken", BlendMode::Darken),
    ("layer.blend_lighten", BlendMode::Lighten),
    ("layer.blend_add", BlendMode::Add),
    ("layer.blend_difference", BlendMode::Difference),
];

fn blend_of(id: &str) -> BlendMode {
    BLENDS.iter().find(|(i, _)| *i == id).map(|(_, b)| *b).unwrap_or(BlendMode::Normal)
}

pub fn run_blend(a: &mut ToonApp, _: &egui::Context, id: &'static str) {
    act::set_layer_blend(a, a.layer, blend_of(id));
}

fn blend_is(a: &ToonApp, id: &str) -> bool {
    layer(a).is_some_and(|l| l.blend == blend_of(id))
}
pub fn blend_normal(a: &ToonApp) -> bool {
    blend_is(a, "layer.blend_normal")
}
pub fn blend_multiply(a: &ToonApp) -> bool {
    blend_is(a, "layer.blend_multiply")
}
pub fn blend_screen(a: &ToonApp) -> bool {
    blend_is(a, "layer.blend_screen")
}
pub fn blend_overlay(a: &ToonApp) -> bool {
    blend_is(a, "layer.blend_overlay")
}
pub fn blend_darken(a: &ToonApp) -> bool {
    blend_is(a, "layer.blend_darken")
}
pub fn blend_lighten(a: &ToonApp) -> bool {
    blend_is(a, "layer.blend_lighten")
}
pub fn blend_add(a: &ToonApp) -> bool {
    blend_is(a, "layer.blend_add")
}
pub fn blend_difference(a: &ToonApp) -> bool {
    blend_is(a, "layer.blend_difference")
}

// ----- tool button -----

fn tool_target(a: &ToonApp) -> Tool {
    match target(a) {
        Target::Tool(t) => *t,
        _ => a.tool,
    }
}

pub fn can_reset_tool(a: &ToonApp) -> Enabled {
    act::can_reset_tool(tool_target(a))
}

pub fn run_tool(a: &mut ToonApp, _: &egui::Context, id: &'static str) {
    let t = tool_target(a);
    if id == "tool.reset_settings" { act::reset_tool(a, t) } else { act::edit_tool_shortcut(a, t) }
}

// ----- panel tab -----

pub fn run_reset_panel(a: &mut ToonApp, _: &egui::Context, _: &'static str) {
    if let Target::Pane(p) = *target(a) {
        act::reset_panel(a, p);
    }
}

pub fn can_reset_panel(a: &ToonApp) -> Enabled {
    match target(a) {
        Target::Pane(Pane::Canvas) => Err(tl!("The canvas stays in its place").to_string()),
        Target::Pane(_) => Ok(()),
        _ => Err(tl!("Open the menu on a panel tab").to_string()),
    }
}

// ----- swatch -----

fn swatch(a: &ToonApp) -> Option<(usize, [u8; 3])> {
    match target(a) {
        Target::Swatch(i, rgb) => Some((*i, *rgb)),
        _ => None,
    }
}

pub fn can_use_swatch(a: &ToonApp) -> Enabled {
    swatch(a).map(|_| ()).ok_or_else(|| tl!("Open the menu on a swatch").to_string())
}

pub fn can_edit_swatch(a: &ToonApp) -> Enabled {
    can_use_swatch(a)?;
    act::can_edit_swatch(a)
}

pub fn can_select_colour(a: &ToonApp) -> Enabled {
    can_use_swatch(a)?;
    act::has_drawing(a)
}

pub fn run_swatch(a: &mut ToonApp, _: &egui::Context, id: &'static str) {
    let Some((i, rgb)) = swatch(a) else { return };
    match id {
        "swatch.use" => crate::colour_tools::set_rgb(a, rgb),
        "swatch.edit" | "swatch.rename" => {
            let palette = a.colour.current.saturating_sub(1);
            let name = a.engine.project.palettes.get(palette).and_then(|p| p.swatches.get(i)).map(|s| s.name.clone()).unwrap_or_default();
            a.colour.edit = Some(crate::colour_tools::SwatchEdit { palette, swatch: i, name, rgb, update_paint: true });
        }
        "swatch.duplicate" => act::duplicate_swatch(a, i),
        "swatch.delete" => act::delete_swatch(a, i),
        _ => act::select_colour(a, rgb),
    }
}

// ----- brush preset -----

fn preset(a: &ToonApp) -> Option<String> {
    match target(a) {
        Target::Preset(id) => Some(id.clone()),
        _ => None,
    }
}

pub fn can_use_preset(a: &ToonApp) -> Enabled {
    match preset(a) {
        Some(id) if a.library.library.get(&id).is_some() => Ok(()),
        _ => Err(tl!("Open the menu on a brush").to_string()),
    }
}

pub fn can_change_preset(a: &ToonApp) -> Enabled {
    can_use_preset(a)?;
    preset(a).map_or(Ok(()), |id| act::can_change_preset(a, &id))
}

pub fn can_save_into_preset(a: &ToonApp) -> Enabled {
    can_use_preset(a)?;
    preset(a).map_or(Ok(()), |id| act::can_save_into_preset(a, &id))
}

pub fn run_preset(a: &mut ToonApp, _: &egui::Context, id: &'static str) {
    let Some(p) = preset(a) else { return };
    match id {
        "preset.use" => crate::brush_library::use_brush(a, &p),
        "preset.rename" => {
            let name = a.library.library.get(&p).map(|e| e.name.clone()).unwrap_or_default();
            a.menus.rename = Some((Target::Preset(p), name));
        }
        "preset.duplicate" => act::duplicate_preset(a, &p),
        "preset.delete" => act::delete_preset(a, &p),
        "preset.save_into" => act::save_current_into_preset(a, &p),
        _ => {
            let ext = tooncraft_engine::preset::EXTENSION;
            let name = a.library.library.get(&p).map(|e| e.name.clone()).unwrap_or_default();
            if let Some(path) = rfd::FileDialog::new().add_filter(tl!("Brush files"), &[ext]).set_file_name(format!("{name}.{ext}")).save_file() {
                act::export_preset(a, &p, &path);
            }
        }
    }
}

// ----- drawing library item -----

fn item(a: &ToonApp) -> Option<usize> {
    match target(a) {
        Target::Item(i) if *i < a.engine.project.library.len() => Some(*i),
        _ => None,
    }
}

pub fn can_use_item(a: &ToonApp) -> Enabled {
    item(a).map(|_| ()).ok_or_else(|| tl!("Open the menu on a library item").to_string())
}

pub fn can_show_item_in_folder(_: &ToonApp) -> Enabled {
    act::can_show_item_in_folder()
}

pub fn run_item(a: &mut ToonApp, _: &egui::Context, id: &'static str) {
    let Some(i) = item(a) else { return };
    let (layer, frame) = (a.layer, a.frame);
    match id {
        "item.place" => {
            a.run(Command::InsertFromLibrary { item: i, layer, frame, linked: false });
        }
        "item.place_layer" => act::place_as_new_layer(a, i),
        "item.rename" => {
            let name = a.engine.project.library.get(i).map(|it| it.name.clone()).unwrap_or_default();
            a.menus.rename = Some((Target::Item(i), name));
        }
        "item.delete" => {
            a.run(Command::RemoveLibraryItem { item: i });
        }
        _ => {}
    }
}

/// Apply a typed name to what is being renamed.
pub fn apply_rename(a: &mut ToonApp, what: &Target, name: &str) {
    match what {
        Target::Preset(id) => act::rename_preset(a, id, name),
        Target::Item(i) => {
            a.run(Command::RenameLibraryItem { item: *i, name: name.trim().to_string() });
        }
        _ => {}
    }
}
