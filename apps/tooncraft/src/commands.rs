//! The command registry: one static table of every user command, with its id, label, icon, group,
//! default key, when it is available (and why not) and what it does. The menu bar, the right-click
//! menus ([`crate::menus::context`]), Find Command and the shortcut editor all read it, so a command
//! added here shows up everywhere with the same label, icon and key.
//!
//! Ids are stable: the remappable ones are the ids the user's saved shortcuts refer to.

use crate::app::{Tool, ToonApp};
use crate::dock::Pane;
use crate::icons::Icon;
use crate::menu_commands as mc;
use crate::view::View;
use tooncraft_engine::Command;

/// Marks a source text for the string catalog without translating it yet (labels in the static
/// table are translated when they are shown).
macro_rules! tlm {
    ($s:literal) => {
        $s
    };
}

/// Where a command belongs (the menu it lives in, and the group Find Command shows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    File,
    Edit,
    Select,
    Tool,
    Frame,
    Layer,
    View,
    Brush,
    Timeline,
    Help,
}

/// A command's key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    None,
    /// A default the user can change in the shortcut editor (craft-keymap).
    Remap(&'static str),
    /// A fixed key the app handles itself (shown, not remappable).
    Fixed(&'static str),
}

/// Why a command cannot run now (shown in the tooltip of its disabled menu item).
pub type Reason = String;

pub struct Cmd {
    pub id: &'static str,
    /// English source text; [`Cmd::label`] translates it.
    label: &'static str,
    pub icon: Option<Icon>,
    pub group: Group,
    pub key: Key,
    /// Available now, or why not.
    pub enabled: fn(&ToonApp) -> Result<(), Reason>,
    /// For on/off commands: whether it is on.
    pub checked: Option<fn(&ToonApp) -> bool>,
    /// A label that says more than the static one ("Undo Brush stroke").
    pub dynamic_label: Option<fn(&ToonApp) -> Option<String>>,
    pub run: fn(&mut ToonApp, &egui::Context, &'static str),
}

impl Group {
    /// The group's name, as Find Command shows it.
    pub fn label(self) -> &'static str {
        match self {
            Group::File => tl!("File"),
            Group::Edit => tl!("Edit"),
            Group::Select => tl!("Selection"),
            Group::Tool => tl!("Tool"),
            Group::Frame => tl!("Frame"),
            Group::Layer => tl!("Layer"),
            Group::View => tl!("View"),
            Group::Brush => tl!("Brush"),
            Group::Timeline => tl!("Timeline"),
            Group::Help => tl!("Help"),
        }
    }
}

impl Cmd {
    pub fn label(&self) -> String {
        tl!(self.label).to_string()
    }

    /// The label shown in menus (the dynamic one when the command has one).
    pub fn menu_label(&self, app: &ToonApp) -> String {
        self.dynamic_label.and_then(|f| f(app)).unwrap_or_else(|| self.label())
    }

    /// The key as the user sees it now ("" for none).
    pub fn key_text(&self, app: &ToonApp) -> String {
        match self.key {
            Key::None => String::new(),
            Key::Remap(_) => {
                let t = app.keys.text(self.id);
                if t == tl!("none") { String::new() } else { t }
            }
            Key::Fixed(k) => crate::keys::chord_text(k),
        }
    }
}

const fn cmd(id: &'static str, label: &'static str, icon: Option<Icon>, group: Group, key: Key, run: fn(&mut ToonApp, &egui::Context, &'static str)) -> Cmd {
    Cmd { id, label, icon, group, key, enabled: always, checked: None, dynamic_label: None, run }
}

const fn when(mut c: Cmd, enabled: fn(&ToonApp) -> Result<(), Reason>) -> Cmd {
    c.enabled = enabled;
    c
}

const fn toggle(mut c: Cmd, checked: fn(&ToonApp) -> bool) -> Cmd {
    c.checked = Some(checked);
    c
}

const fn named(mut c: Cmd, f: fn(&ToonApp) -> Option<String>) -> Cmd {
    c.dynamic_label = Some(f);
    c
}

fn always(_: &ToonApp) -> Result<(), Reason> {
    Ok(())
}

fn not_stroking(a: &ToonApp) -> Result<(), Reason> {
    if a.engine.is_stroking() { Err(tl!("Finish the stroke first").to_string()) } else { Ok(()) }
}

fn has_selection(a: &ToonApp) -> Result<(), Reason> {
    if a.selection.is_some() { Ok(()) } else { Err(tl!("Nothing is selected").to_string()) }
}

fn can_undo(a: &ToonApp) -> Result<(), Reason> {
    not_stroking(a)?;
    if a.engine.can_undo() { Ok(()) } else { Err(tl!("Nothing to undo").to_string()) }
}

fn can_redo(a: &ToonApp) -> Result<(), Reason> {
    not_stroking(a)?;
    if a.engine.can_redo() { Ok(()) } else { Err(tl!("Nothing to redo").to_string()) }
}

fn has_sound(a: &ToonApp) -> Result<(), Reason> {
    if a.engine.project.sounds.is_empty() { Err(tl!("The shot has no sound").to_string()) } else { Ok(()) }
}

fn has_frames_selected(a: &ToonApp) -> Result<(), Reason> {
    if a.has_range() { Ok(()) } else { Err(tl!("Select a range of frames in the exposure sheet first").to_string()) }
}

fn has_in_out(a: &ToonApp) -> Result<(), Reason> {
    if a.tl.range.is_some() { Ok(()) } else { Err(tl!("No in and out range is set").to_string()) }
}

fn has_brush(a: &ToonApp) -> Result<(), Reason> {
    if a.tool.brush().is_some() { Ok(()) } else { Err(tl!("The tool has no brush").to_string()) }
}

fn pick_tool(a: &mut ToonApp, _: &egui::Context, id: &'static str) {
    if a.engine.is_stroking() {
        return;
    }
    if let Some(t) = Tool::ALL.into_iter().find(|t| id.strip_prefix("tool.") == Some(t.id())) {
        a.tool = t;
        // a tool key gives the plain tool (the rail's selection-only transform is a variant)
        a.rail.transform_only = false;
        a.set_status(tlf!("{tool} selected", tool = t.label()));
    }
}

fn layer_frame(a: &ToonApp) -> (usize, u32) {
    (a.layer, a.frame)
}

fn view_rects(a: &ToonApp) -> (egui::Rect, egui::Vec2) {
    (a.workspace.canvas_rect, egui::vec2(a.engine.project.width() as f32, a.engine.project.height() as f32))
}

use Group as G;
use Key::{Fixed, None as NoKey, Remap};

macro_rules! tool {
    ($id:literal, $label:literal, $icon:ident, $key:literal) => {
        when(cmd($id, tlm!($label), Some(Icon::$icon), G::Tool, Remap($key), pick_tool), not_stroking)
    };
}

/// Every command. Order is the order Find Command lists them in.
pub static ALL: &[Cmd] = &[
    // ── tools ──
    tool!("tool.pencil", "Pencil", Pencil, "P"),
    tool!("tool.ink", "Ink pen", Ink, "N"),
    tool!("tool.airbrush", "Airbrush", Airbrush, "A"),
    tool!("tool.chalk", "Chalk", Chalk, "C"),
    tool!("tool.eraser", "Eraser", Eraser, "E"),
    tool!("tool.watercolour", "Watercolour", Watercolour, "W"),
    tool!("tool.smudge", "Smudge", Smudge, "K"),
    tool!("tool.fill", "Paint bucket", Fill, "F"),
    tool!("tool.picker", "Colour picker", Picker, "I"),
    tool!("tool.select", "Select", Select, "S"),
    tool!("tool.shape", "Shapes", Shape, "U"),
    tool!("tool.gradient", "Gradient", Gradient, "G"),
    tool!("tool.hand", "Hand", Hand, "H"),
    tool!("tool.transform", "Transform layer", Transform, "Q"),
    tool!("tool.text", "Text", Text, "T"),
    tool!("tool.zoom", "Zoom view", Zoom, "Z"),
    tool!("tool.rotate_view", "Rotate view", RotateView, "R"),
    // ── playback and frames ──
    cmd("frame.play", tlm!("Play / stop"), Some(Icon::Play), G::Frame, Remap("Enter"), |a, _, _| a.toggle_play()),
    cmd("frame.prev", tlm!("Previous frame"), Some(Icon::PrevFrame), G::Frame, Remap(","), |a, _, _| a.go_to(a.frame as i64 - 1)),
    cmd("frame.next", tlm!("Next frame"), Some(Icon::NextFrame), G::Frame, Remap("."), |a, _, _| a.go_to(a.frame as i64 + 1)),
    cmd("frame.prev_drawing", tlm!("Previous drawing"), Some(Icon::PrevDrawing), G::Frame, Remap("Ctrl+Left"), |a, _, _| a.step_drawing(false)),
    cmd("frame.next_drawing", tlm!("Next drawing"), Some(Icon::NextDrawing), G::Frame, Remap("Ctrl+Right"), |a, _, _| a.step_drawing(true)),
    cmd("view.radial_menu", tlm!("Radial menu"), Some(Icon::Radial), G::View, Remap("V"), |a, ctx, _| crate::tool_palettes::open_radial(a, ctx)),
    cmd("app.find_command", tlm!("Find a command"), Some(Icon::Search), G::Help, Remap("Ctrl+Shift+P"), |a, _, _| a.find.open = true),
    cmd("select.invert", tlm!("Invert selection"), None, G::Select, Remap("Ctrl+Shift+I"), |a, _, _| crate::selection_tools::invert(a)),
    cmd("frame.swap_next", tlm!("Show the next drawing of the layer here"), None, G::Frame, Remap("Ctrl+Alt+Down"), |a, _, _| a.swap_step(true)),
    cmd("frame.swap_prev", tlm!("Show the previous drawing of the layer here"), None, G::Frame, Remap("Ctrl+Alt+Up"), |a, _, _| a.swap_step(false)),
    cmd("flip.toggle", tlm!("Add or remove this frame from the flip set"), None, G::Frame, Remap("Ctrl+Alt+Enter"), |a, _, _| a.toggle_flip()),
    cmd("flip.next", tlm!("Flip to the next chosen frame"), None, G::Frame, Remap("Ctrl+Alt+Right"), |a, _, _| a.flip_step(true)),
    cmd("flip.prev", tlm!("Flip to the previous chosen frame"), None, G::Frame, Remap("Ctrl+Alt+Left"), |a, _, _| a.flip_step(false)),
    cmd("frame.prev_key", tlm!("Previous key drawing"), Some(Icon::MarkKey), G::Frame, Remap("Ctrl+Shift+Left"), |a, _, _| a.step_key(false)),
    cmd("frame.next_key", tlm!("Next key drawing"), Some(Icon::MarkKey), G::Frame, Remap("Ctrl+Shift+Right"), |a, _, _| a.step_key(true)),
    cmd("layer.prev", tlm!("Layer below"), None, G::Layer, Remap("Ctrl+Down"), |a, _, _| a.step_layer(false)),
    cmd("layer.next", tlm!("Layer above"), None, G::Layer, Remap("Ctrl+Up"), |a, _, _| a.step_layer(true)),
    cmd("frame.first", tlm!("First frame"), Some(Icon::First), G::Frame, Remap("Home"), |a, _, _| a.frame = 0),
    cmd("frame.last", tlm!("Last frame"), Some(Icon::Last), G::Frame, Remap("End"), |a, _, _| a.frame = a.engine.project.length().saturating_sub(1)),
    cmd("frame.new_drawing", tlm!("New drawing"), Some(Icon::Add), G::Frame, Remap("Shift+N"), |a, _, _| {
        let (layer, frame) = layer_frame(a);
        if a.run(Command::NewDrawing { layer, frame }) {
            a.set_status(tl!("New drawing"));
        }
    }),
    cmd("frame.duplicate", tlm!("Duplicate drawing"), Some(Icon::Duplicate), G::Frame, Remap("Shift+D"), |a, _, _| {
        let (layer, frame) = layer_frame(a);
        if a.run(Command::DuplicateDrawing { layer, frame }) {
            if let Some((_, end)) = a.engine.project.layers.get(layer).and_then(|l| l.exposure_range(frame)) {
                a.frame = end.saturating_sub(1).max(frame + 1).min(a.engine.project.length().saturating_sub(1));
            }
            a.set_status(tl!("Drawing duplicated"));
        }
    }),
    cmd("frame.insert", tlm!("Insert frame"), None, G::Frame, Remap("Insert"), |a, _, _| {
        let (layer, frame) = layer_frame(a);
        a.run(Command::InsertFrame { layer: Some(layer), frame });
    }),
    cmd("frame.delete", tlm!("Delete frame"), Some(Icon::Delete), G::Frame, Remap("Shift+Delete"), |a, _, _| {
        let (layer, frame) = layer_frame(a);
        if a.has_range() {
            a.delete_selection(true);
        } else {
            a.run(Command::DeleteFrame { layer: Some(layer), frame });
        }
    }),
    cmd("frame.hold_more", tlm!("Hold one frame longer"), None, G::Frame, Remap("="), |a, _, _| {
        let (layer, frame) = layer_frame(a);
        a.run(Command::ExtendHold { layer, frame });
    }),
    cmd("frame.hold_less", tlm!("Hold one frame shorter"), None, G::Frame, Remap("-"), |a, _, _| {
        let (layer, frame) = layer_frame(a);
        a.run(Command::ShortenHold { layer, frame });
    }),
    cmd("frame.clear", tlm!("Clear frame / erase selection"), Some(Icon::Close), G::Frame, Remap("Delete"), |a, _, _| {
        let (layer, frame) = layer_frame(a);
        if a.selection.is_some() {
            a.delete_selected_pixels();
        } else if a.has_range() {
            a.delete_selection(false);
        } else {
            a.run(Command::ClearCell { layer, frame });
        }
    }),
    toggle(
        cmd("view.onion", tlm!("Onion skin on / off"), Some(Icon::Onion), G::View, Remap("O"), |a, _, _| {
            a.engine.project.onion.enabled = !a.engine.project.onion.enabled;
            let s = if a.engine.project.onion.enabled { tl!("Onion skin on") } else { tl!("Onion skin off") };
            a.set_status(s);
        }),
        |a| a.engine.project.onion.enabled,
    ),
    toggle(cmd("view.mirror", tlm!("Mirror view"), Some(Icon::Mirror), G::View, Remap("M"), |a, _, _| a.view.flip = !a.view.flip), |a| a.view.flip),
    cmd("view.pin", tlm!("Pin frame to the light table"), Some(Icon::LightTable), G::View, Remap("Shift+L"), |a, _, _| {
        let (layer, frame) = layer_frame(a);
        a.run(Command::TogglePin { layer, frame });
        a.set_status(tl!("Light table pins changed"));
    }),
    when(
        cmd("brush.smaller", tlm!("Smaller brush"), None, G::Brush, Remap("["), |a, _, _| {
            if let Some(kind) = a.tool.brush() {
                let b = a.brush_mut(kind);
                b.size = (b.size / 1.2).max(0.5);
            }
        }),
        has_brush,
    ),
    when(
        cmd("brush.bigger", tlm!("Bigger brush"), None, G::Brush, Remap("]"), |a, _, _| {
            if let Some(kind) = a.tool.brush() {
                let b = a.brush_mut(kind);
                b.size = (b.size * 1.2).min(tooncraft_brush::MAX_SIZE);
            }
        }),
        has_brush,
    ),
    cmd("library.find", tlm!("Find brush"), Some(Icon::Brush), G::Brush, Remap("Ctrl+B"), |a, _, _| {
        a.workspace.raise(Pane::Library);
        a.library.focus_search = true;
    }),
    cmd("marker.toggle", tlm!("Add or remove a marker on this frame"), Some(Icon::Label), G::Timeline, Remap("Shift+M"), |a, _, _| {
        crate::timeline_extras::toggle_here(a)
    }),
    cmd("marker.next", tlm!("Next marker"), None, G::Timeline, Remap("Alt+."), |a, _, _| crate::timeline_extras::jump(a, true)),
    cmd("marker.prev", tlm!("Previous marker"), None, G::Timeline, Remap("Alt+,"), |a, _, _| crate::timeline_extras::jump(a, false)),
    cmd("timeline.zoom_in", tlm!("Zoom the timeline in"), None, G::Timeline, Remap("Alt+="), |a, _, _| a.tl.zoom_by(1.25)),
    cmd("timeline.zoom_out", tlm!("Zoom the timeline out"), None, G::Timeline, Remap("Alt+-"), |a, _, _| a.tl.zoom_by(0.8)),
    // ── file ──
    cmd("file.new", tlm!("New Shot…"), Some(Icon::Add), G::File, Fixed("Ctrl+N"), |a, _, _| crate::unsaved::request(a, crate::unsaved::Replace::NewShot)),
    cmd("file.open", tlm!("Open…"), Some(Icon::Folder), G::File, Fixed("Ctrl+O"), |a, _, _| crate::unsaved::request(a, crate::unsaved::Replace::Browse)),
    cmd("file.save", tlm!("Save"), None, G::File, Fixed("Ctrl+S"), |a, _, _| {
        a.save();
    }),
    cmd("file.save_as", tlm!("Save As…"), None, G::File, Fixed("Ctrl+Shift+S"), |a, _, _| {
        a.save_as();
    }),
    cmd("file.save_version", tlm!("Save New Version"), None, G::File, Fixed("Ctrl+Alt+S"), |a, _, _| a.save_new_version()),
    cmd("file.import_images", tlm!("Import Image Sequence…"), None, G::File, NoKey, |a, _, _| a.import_images()),
    cmd("file.import_video", tlm!("Import Video…"), None, G::File, NoKey, |a, _, _| crate::video_dialog::open(a)),
    cmd("file.import_sound", tlm!("Import Sound…"), Some(Icon::Sound), G::File, NoKey, |a, _, _| a.import_sound()),
    when(
        cmd("file.remove_sound", tlm!("Remove Sound"), None, G::File, NoKey, |a, _, _| {
            a.run(Command::SetSound { path: None, offset: 0 });
        }),
        has_sound,
    ),
    cmd("file.export", tlm!("Export…"), None, G::File, NoKey, |a, _, _| {
        let kind = a.export_memory.get(&a.engine.project.name).map(|d| d.kind).unwrap_or("mp4");
        crate::export_dialog::open(a, kind);
    }),
    cmd("file.quit", tlm!("Quit"), Some(Icon::Close), G::File, NoKey, |_, ctx, _| ctx.send_viewport_cmd(egui::ViewportCommand::Close)),
    // ── edit ──
    cmd("edit.preferences", tlm!("Preferences…"), Some(Icon::Settings), G::Edit, NoKey, |a, _, _| a.preferences.open = true),
    named(when(cmd("edit.undo", tlm!("Undo"), Some(Icon::Undo), G::Edit, Fixed("Ctrl+Z"), |a, _, _| a.undo()), can_undo), |a| {
        a.engine.undo_label().map(|l| tlf!("Undo {action}", action = l))
    }),
    named(when(cmd("edit.redo", tlm!("Redo"), Some(Icon::Redo), G::Edit, Fixed("Ctrl+Shift+Z"), |a, _, _| a.redo()), can_redo), |a| {
        a.engine.redo_label().map(|l| tlf!("Redo {action}", action = l))
    }),
    cmd("edit.history", tlm!("Undo History"), Some(Icon::History), G::Edit, NoKey, |a, _, _| a.workspace.raise(Pane::History)),
    cmd("edit.cut", tlm!("Cut"), Some(Icon::Cut), G::Edit, Fixed("Ctrl+X"), |a, _, _| {
        if a.selection.is_some() {
            a.copy_pixels(true);
        } else {
            a.cut_cells();
        }
    }),
    cmd("edit.copy", tlm!("Copy"), Some(Icon::Copy), G::Edit, Fixed("Ctrl+C"), |a, _, _| {
        if a.selection.is_some() {
            a.copy_pixels(false);
        } else {
            a.copy_cells();
        }
    }),
    cmd("edit.paste", tlm!("Paste"), Some(Icon::Paste), G::Edit, Fixed("Ctrl+V"), |a, ctx, _| {
        // pixels go onto the canvas when it has the keyboard, cells into the sheet otherwise
        if ctx.memory(|m| m.focused()) == Some(crate::canvas::canvas_id()) || a.selection.is_some() {
            a.paste_pixels();
        } else {
            a.paste_cells(false, false);
        }
    }),
    cmd("edit.select_all", tlm!("Select All"), None, G::Select, Fixed("Ctrl+A"), |a, _, _| a.select_all()),
    when(cmd("edit.deselect", tlm!("Deselect"), None, G::Select, Fixed("Escape"), |a, _, _| a.selection = None), has_selection),
    when(cmd("select.grow", tlm!("Grow Selection"), None, G::Select, NoKey, |a, _, _| crate::selection_tools::grow(a, a.sel.grow_by)), has_selection),
    when(cmd("select.shrink", tlm!("Shrink Selection"), None, G::Select, NoKey, |a, _, _| crate::selection_tools::grow(a, -a.sel.grow_by)), has_selection),
    when(
        cmd("select.flip_h", tlm!("Flip Horizontally"), Some(Icon::FlipH), G::Select, NoKey, |a, _, _| a.transform_selection(0.0, 0.0, (-1.0, 1.0), 0.0)),
        has_selection,
    ),
    when(
        cmd("select.flip_v", tlm!("Flip Vertically"), Some(Icon::FlipV), G::Select, NoKey, |a, _, _| a.transform_selection(0.0, 0.0, (1.0, -1.0), 0.0)),
        has_selection,
    ),
    when(
        cmd("select.rotate_cw", tlm!("Rotate 90° Clockwise"), Some(Icon::Rotate90), G::Select, NoKey, |a, _, _| {
            a.transform_selection(0.0, 0.0, (1.0, 1.0), std::f32::consts::FRAC_PI_2)
        }),
        has_selection,
    ),
    when(
        cmd("select.rotate_ccw", tlm!("Rotate 90° Counter-clockwise"), None, G::Select, NoKey, |a, _, _| {
            a.transform_selection(0.0, 0.0, (1.0, 1.0), -std::f32::consts::FRAC_PI_2)
        }),
        has_selection,
    ),
    when(cmd("select.scale_half", tlm!("Scale 50 %"), None, G::Select, NoKey, |a, _, _| a.transform_selection(0.0, 0.0, (0.5, 0.5), 0.0)), has_selection),
    when(cmd("select.scale_double", tlm!("Scale 200 %"), None, G::Select, NoKey, |a, _, _| a.transform_selection(0.0, 0.0, (2.0, 2.0), 0.0)), has_selection),
    when(cmd("select.erase", tlm!("Erase Selection"), Some(Icon::Delete), G::Select, NoKey, |a, _, _| a.delete_selected_pixels()), has_selection),
    cmd("edit.clear_drawing", tlm!("Clear Drawing"), None, G::Edit, NoKey, |a, _, _| {
        let (layer, frame) = layer_frame(a);
        a.run(Command::ClearDrawing { layer, frame });
    }),
    cmd("edit.purge_unused", tlm!("Purge Unused Drawings"), None, G::Edit, NoKey, |a, _, _| {
        let n = a.engine.project.unused_drawings().len();
        if a.run(Command::PurgeUnused) {
            a.set_status(tlf!("Removed {count} unused drawings", count = n));
        }
    }),
    // ── view ──
    cmd("view.zoom_in", tlm!("Zoom In"), Some(Icon::Search), G::View, Fixed("Ctrl+="), |a, _, _| {
        let (vp, canvas) = view_rects(a);
        a.view.zoom_at(vp, canvas, vp.center(), 1.25);
    }),
    cmd("view.zoom_out", tlm!("Zoom Out"), None, G::View, Fixed("Ctrl+-"), |a, _, _| {
        let (vp, canvas) = view_rects(a);
        a.view.zoom_at(vp, canvas, vp.center(), 0.8);
    }),
    cmd("view.fit", tlm!("Fit to Window"), None, G::View, Fixed("Ctrl+0"), |a, _, _| {
        let (vp, canvas) = view_rects(a);
        a.view = View::fit(vp, canvas);
    }),
    cmd("view.actual_size", tlm!("Actual Size"), None, G::View, Fixed("Ctrl+1"), |a, _, _| a.view.zoom = 1.0),
    cmd("view.rotate_left", tlm!("Rotate Left"), None, G::View, Fixed("Ctrl+["), |a, _, _| a.view.rotate(-15f32.to_radians())),
    cmd("view.rotate_right", tlm!("Rotate Right"), None, G::View, Fixed("Ctrl+]"), |a, _, _| a.view.rotate(15f32.to_radians())),
    cmd("view.reset_rotation", tlm!("Reset Rotation"), None, G::View, NoKey, |a, _, _| a.view.rotation = 0.0),
    toggle(
        cmd("view.light_table", tlm!("Light Table"), Some(Icon::LightTable), G::View, NoKey, |a, _, _| {
            a.engine.project.light_table.enabled = !a.engine.project.light_table.enabled
        }),
        |a| a.engine.project.light_table.enabled,
    ),
    toggle(
        cmd("view.light_theme", tlm!("Light Theme"), Some(Icon::Theme), G::View, NoKey, |a, ctx, _| {
            a.light_theme = !a.light_theme;
            crate::theme::apply(ctx, a.light_theme);
        }),
        |a| a.light_theme,
    ),
    cmd("view.reset_panels", tlm!("Reset Panels"), None, G::View, NoKey, |a, _, _| a.workspace = crate::dock::Workspace::default()),
    // ── help ──
    cmd("help.shortcuts", tlm!("Keyboard Shortcuts"), None, G::Help, NoKey, |a, _, _| a.show_shortcuts = true),
    cmd("help.icons", tlm!("Icon Reference"), Some(Icon::Palettes), G::Help, NoKey, |a, _, _| a.show_icons = true),
    cmd("help.about", tlm!("About ToonCraft"), None, G::Help, NoKey, |a, _, _| a.show_about = true),
    // ── right-click menus (design §5); bodies in menu_commands / menu_actions ──
    when(cmd("canvas.pick_colour", tlm!("Pick Colour Here"), Some(Icon::Picker), G::Edit, NoKey, mc::run_pick), mc::can_pick),
    when(cmd("canvas.flip_h", tlm!("Flip Drawing Horizontally"), Some(Icon::FlipH), G::Edit, NoKey, mc::run_canvas_transform), mc::can_transform),
    when(cmd("canvas.flip_v", tlm!("Flip Drawing Vertically"), Some(Icon::FlipV), G::Edit, NoKey, mc::run_canvas_transform), mc::can_transform),
    when(cmd("canvas.rotate_cw", tlm!("Rotate Drawing 90° Clockwise"), Some(Icon::Rotate90), G::Edit, NoKey, mc::run_canvas_transform), mc::can_transform),
    when(cmd("canvas.rotate_ccw", tlm!("Rotate Drawing 90° Counter-clockwise"), None, G::Edit, NoKey, mc::run_canvas_transform), mc::can_transform),
    when(cmd("selection.transform", tlm!("Transform"), Some(Icon::SelectTransform), G::Select, NoKey, mc::run_selection), mc::has_selection),
    when(cmd("selection.copy", tlm!("Copy Pixels"), Some(Icon::Copy), G::Select, NoKey, mc::run_selection), mc::has_selection),
    when(cmd("selection.cut", tlm!("Cut Pixels"), Some(Icon::Cut), G::Select, NoKey, mc::run_selection), mc::can_paint_selection),
    when(cmd("selection.paste", tlm!("Paste Pixels"), Some(Icon::Paste), G::Select, NoKey, mc::run_selection), mc::can_paste_pixels),
    when(cmd("selection.fill", tlm!("Fill Selection"), Some(Icon::Fill), G::Select, NoKey, mc::run_selection), mc::can_paint_selection),
    when(cmd("selection.stroke", tlm!("Stroke Selection"), Some(Icon::Ink), G::Select, NoKey, mc::run_selection), mc::can_paint_selection),
    when(cmd("selection.save_library", tlm!("Save to Library"), Some(Icon::Drawings), G::Select, NoKey, mc::run_selection), mc::can_save_to_library),
    when(cmd("cell.hold_1", tlm!("1 frame"), None, G::Frame, NoKey, mc::run_hold), mc::has_drawing),
    when(cmd("cell.hold_2", tlm!("2 frames"), None, G::Frame, NoKey, mc::run_hold), mc::has_drawing),
    when(cmd("cell.hold_3", tlm!("3 frames"), None, G::Frame, NoKey, mc::run_hold), mc::has_drawing),
    when(cmd("cell.hold_4", tlm!("4 frames"), None, G::Frame, NoKey, mc::run_hold), mc::has_drawing),
    when(cmd("cell.hold_5", tlm!("5 frames"), None, G::Frame, NoKey, mc::run_hold), mc::has_drawing),
    when(cmd("cell.hold_6", tlm!("6 frames"), None, G::Frame, NoKey, mc::run_hold), mc::has_drawing),
    when(cmd("cell.hold_7", tlm!("7 frames"), None, G::Frame, NoKey, mc::run_hold), mc::has_drawing),
    when(cmd("cell.hold_8", tlm!("8 frames"), None, G::Frame, NoKey, mc::run_hold), mc::has_drawing),
    toggle(when(cmd("cell.mark_key", tlm!("Key"), Some(Icon::MarkKey), G::Frame, NoKey, mc::run_mark), mc::has_drawing), mc::is_key),
    toggle(when(cmd("cell.mark_breakdown", tlm!("Breakdown"), Some(Icon::MarkBreakdown), G::Frame, NoKey, mc::run_mark), mc::has_drawing), mc::is_breakdown),
    toggle(when(cmd("cell.mark_inbetween", tlm!("In-between"), Some(Icon::MarkInbetween), G::Frame, NoKey, mc::run_mark), mc::has_drawing), mc::is_inbetween),
    toggle(when(cmd("cell.mark_none", tlm!("No Mark"), None, G::Frame, NoKey, mc::run_mark), mc::has_drawing), mc::is_unmarked),
    cmd("cell.copy", tlm!("Copy Frames"), Some(Icon::Copy), G::Frame, NoKey, mc::run_cells),
    cmd("cell.cut", tlm!("Cut Frames"), Some(Icon::Cut), G::Frame, NoKey, mc::run_cells),
    when(cmd("cell.paste", tlm!("Paste Frames"), Some(Icon::Paste), G::Frame, NoKey, mc::run_cells), mc::can_paste_cells),
    when(cmd("cell.paste_reversed", tlm!("Paste Reversed"), None, G::Frame, NoKey, mc::run_cells), mc::can_paste_cells),
    cmd("cell.fill_empty", tlm!("Fill Empty Frames"), None, G::Frame, NoKey, mc::run_cells),
    cmd("ruler.loop_in", tlm!("Set Loop In Here"), Some(Icon::Loop), G::Timeline, NoKey, mc::run_ruler),
    cmd("ruler.loop_out", tlm!("Set Loop Out Here"), Some(Icon::Loop), G::Timeline, NoKey, mc::run_ruler),
    toggle(cmd("onion.range_1", tlm!("1 Each Side"), Some(Icon::Onion), G::View, NoKey, mc::run_onion_range), mc::range_1),
    toggle(cmd("onion.range_2", tlm!("2 Each Side"), Some(Icon::Onion), G::View, NoKey, mc::run_onion_range), mc::range_2),
    toggle(cmd("onion.range_3", tlm!("3 Each Side"), Some(Icon::Onion), G::View, NoKey, mc::run_onion_range), mc::range_3),
    toggle(cmd("onion.range_5", tlm!("5 Each Side"), Some(Icon::Onion), G::View, NoKey, mc::run_onion_range), mc::range_5),
    toggle(cmd("onion.keys_only", tlm!("Keys Only"), Some(Icon::MarkKey), G::View, NoKey, mc::run_keys_only), mc::keys_only),
    toggle(cmd("layer.visible", tlm!("Show Layer"), Some(Icon::Eye), G::Layer, NoKey, mc::run_layer_flag), mc::is_visible),
    toggle(cmd("layer.lock", tlm!("Lock Layer"), Some(Icon::Lock), G::Layer, NoKey, mc::run_layer_flag), mc::is_locked),
    toggle(cmd("layer.lock_alpha", tlm!("Lock Transparency"), Some(Icon::LockAlpha), G::Layer, NoKey, mc::run_layer_flag), mc::is_alpha_locked),
    toggle(cmd("layer.onion", tlm!("Include in Onion Skin"), Some(Icon::OnionInclude), G::Layer, NoKey, mc::run_layer_flag), mc::in_onion),
    toggle(when(cmd("layer.clip", tlm!("Clip to Layer Below"), None, G::Layer, NoKey, mc::run_layer_flag), mc::not_bottom), mc::is_clipped),
    cmd("layer.rename", tlm!("Rename Layer…"), Some(Icon::Rename), G::Layer, NoKey, mc::run_layer),
    cmd("layer.add_above", tlm!("Add Layer Above"), Some(Icon::Add), G::Layer, NoKey, mc::run_layer),
    cmd("layer.duplicate", tlm!("Duplicate Layer"), Some(Icon::Duplicate), G::Layer, NoKey, mc::run_layer),
    when(cmd("layer.delete", tlm!("Delete Layer"), Some(Icon::Delete), G::Layer, NoKey, mc::run_layer), mc::not_last),
    when(cmd("layer.merge_down", tlm!("Merge Down"), Some(Icon::MergeDown), G::Layer, NoKey, mc::run_layer), mc::not_bottom),
    when(cmd("layer.add_peg", tlm!("Add Peg"), Some(Icon::Peg), G::Layer, NoKey, mc::run_layer), mc::can_add_peg),
    when(cmd("layer.move_up", tlm!("Move Layer Up"), None, G::Layer, NoKey, mc::run_layer), mc::not_top),
    when(cmd("layer.move_down", tlm!("Move Layer Down"), None, G::Layer, NoKey, mc::run_layer), mc::not_bottom),
    toggle(cmd("layer.adjustment", tlm!("Adjustment Layer"), Some(Icon::Effects), G::Layer, NoKey, crate::layer_fx::run), crate::layer_fx::is_adjustment),
    when(cmd("layer.mask_add", tlm!("Add Layer Mask"), None, G::Layer, NoKey, crate::layer_fx::run), crate::layer_fx::no_mask),
    toggle(
        when(cmd("layer.mask_edit", tlm!("Paint on Layer Mask"), None, G::Layer, NoKey, crate::layer_fx::run), crate::layer_fx::has_mask),
        crate::layer_fx::is_editing,
    ),
    toggle(
        when(cmd("layer.mask_enabled", tlm!("Use Layer Mask"), None, G::Layer, NoKey, crate::layer_fx::run), crate::layer_fx::has_mask),
        crate::layer_fx::is_enabled,
    ),
    toggle(
        when(cmd("layer.mask_invert", tlm!("Invert Layer Mask"), None, G::Layer, NoKey, crate::layer_fx::run), crate::layer_fx::has_mask),
        crate::layer_fx::is_inverted,
    ),
    when(cmd("layer.mask_remove", tlm!("Remove Layer Mask"), None, G::Layer, NoKey, crate::layer_fx::run), crate::layer_fx::has_mask),
    toggle(cmd("layer.label_red", tlm!("Red"), Some(Icon::Label), G::Layer, NoKey, mc::run_label), mc::label_red),
    toggle(cmd("layer.label_orange", tlm!("Orange"), Some(Icon::Label), G::Layer, NoKey, mc::run_label), mc::label_orange),
    toggle(cmd("layer.label_yellow", tlm!("Yellow"), Some(Icon::Label), G::Layer, NoKey, mc::run_label), mc::label_yellow),
    toggle(cmd("layer.label_green", tlm!("Green"), Some(Icon::Label), G::Layer, NoKey, mc::run_label), mc::label_green),
    toggle(cmd("layer.label_blue", tlm!("Blue"), Some(Icon::Label), G::Layer, NoKey, mc::run_label), mc::label_blue),
    toggle(cmd("layer.label_violet", tlm!("Violet"), Some(Icon::Label), G::Layer, NoKey, mc::run_label), mc::label_violet),
    toggle(cmd("layer.label_none", tlm!("No Colour"), None, G::Layer, NoKey, mc::run_label), mc::label_none),
    toggle(cmd("layer.opacity_100", tlm!("100 %"), None, G::Layer, NoKey, mc::run_opacity), mc::opacity_100),
    toggle(cmd("layer.opacity_75", tlm!("75 %"), None, G::Layer, NoKey, mc::run_opacity), mc::opacity_75),
    toggle(cmd("layer.opacity_50", tlm!("50 %"), None, G::Layer, NoKey, mc::run_opacity), mc::opacity_50),
    toggle(cmd("layer.opacity_25", tlm!("25 %"), None, G::Layer, NoKey, mc::run_opacity), mc::opacity_25),
    toggle(cmd("layer.opacity_10", tlm!("10 %"), None, G::Layer, NoKey, mc::run_opacity), mc::opacity_10),
    toggle(cmd("layer.blend_normal", tlm!("Normal"), None, G::Layer, NoKey, mc::run_blend), mc::blend_normal),
    toggle(cmd("layer.blend_multiply", tlm!("Multiply"), None, G::Layer, NoKey, mc::run_blend), mc::blend_multiply),
    toggle(cmd("layer.blend_screen", tlm!("Screen"), None, G::Layer, NoKey, mc::run_blend), mc::blend_screen),
    toggle(cmd("layer.blend_overlay", tlm!("Overlay"), None, G::Layer, NoKey, mc::run_blend), mc::blend_overlay),
    toggle(cmd("layer.blend_darken", tlm!("Darken"), None, G::Layer, NoKey, mc::run_blend), mc::blend_darken),
    toggle(cmd("layer.blend_lighten", tlm!("Lighten"), None, G::Layer, NoKey, mc::run_blend), mc::blend_lighten),
    toggle(cmd("layer.blend_add", tlm!("Add"), None, G::Layer, NoKey, mc::run_blend), mc::blend_add),
    toggle(cmd("layer.blend_difference", tlm!("Difference"), None, G::Layer, NoKey, mc::run_blend), mc::blend_difference),
    when(cmd("tool.reset_settings", tlm!("Reset Tool Settings"), None, G::Tool, NoKey, mc::run_tool), mc::can_reset_tool),
    cmd("tool.shortcut", tlm!("Shortcut…"), Some(Icon::Settings), G::Tool, NoKey, mc::run_tool),
    when(cmd("panel.reset", tlm!("Reset Panel"), None, G::View, NoKey, mc::run_reset_panel), mc::can_reset_panel),
    when(cmd("swatch.use", tlm!("Use Colour"), Some(Icon::Colour), G::Edit, NoKey, mc::run_swatch), mc::can_use_swatch),
    when(cmd("swatch.edit", tlm!("Edit Swatch…"), Some(Icon::Settings), G::Edit, NoKey, mc::run_swatch), mc::can_edit_swatch),
    when(cmd("swatch.rename", tlm!("Rename Swatch…"), Some(Icon::Rename), G::Edit, NoKey, mc::run_swatch), mc::can_edit_swatch),
    when(cmd("swatch.duplicate", tlm!("Duplicate Swatch"), Some(Icon::Duplicate), G::Edit, NoKey, mc::run_swatch), mc::can_edit_swatch),
    when(cmd("swatch.delete", tlm!("Delete Swatch"), Some(Icon::Delete), G::Edit, NoKey, mc::run_swatch), mc::can_edit_swatch),
    when(cmd("swatch.select_pixels", tlm!("Select Pixels of This Colour"), Some(Icon::SelectWand), G::Select, NoKey, mc::run_swatch), mc::can_select_colour),
    when(cmd("preset.use", tlm!("Use Brush"), Some(Icon::Brush), G::Brush, NoKey, mc::run_preset), mc::can_use_preset),
    when(cmd("preset.rename", tlm!("Rename Brush…"), Some(Icon::Rename), G::Brush, NoKey, mc::run_preset), mc::can_change_preset),
    when(cmd("preset.duplicate", tlm!("Duplicate Brush"), Some(Icon::Duplicate), G::Brush, NoKey, mc::run_preset), mc::can_use_preset),
    when(cmd("preset.delete", tlm!("Delete Brush"), Some(Icon::Delete), G::Brush, NoKey, mc::run_preset), mc::can_change_preset),
    when(cmd("preset.save_into", tlm!("Save Current Brush Into"), None, G::Brush, NoKey, mc::run_preset), mc::can_save_into_preset),
    when(cmd("preset.export", tlm!("Export Brush…"), None, G::Brush, NoKey, mc::run_preset), mc::can_use_preset),
    when(cmd("item.place", tlm!("Place in Cell"), Some(Icon::Drawings), G::Frame, NoKey, mc::run_item), mc::can_use_item),
    when(cmd("item.place_layer", tlm!("Place as New Layer"), Some(Icon::Layers), G::Frame, NoKey, mc::run_item), mc::can_use_item),
    when(cmd("item.rename", tlm!("Rename Item…"), Some(Icon::Rename), G::Frame, NoKey, mc::run_item), mc::can_use_item),
    when(cmd("item.delete", tlm!("Delete Item"), Some(Icon::Delete), G::Frame, NoKey, mc::run_item), mc::can_use_item),
    when(cmd("item.show_folder", tlm!("Show in Folder"), Some(Icon::Folder), G::Frame, NoKey, mc::run_item), mc::can_show_item_in_folder),
    // ── timeline header and canvas header (UI pass S3) ──
    toggle(cmd("timeline.loop", tlm!("Loop playback"), Some(Icon::Loop), G::Timeline, NoKey, |a, _, _| a.playback.looping = !a.playback.looping), |a| {
        a.playback.looping
    }),
    cmd("timeline.play_around", tlm!("Play Around"), Some(Icon::PlayAround), G::Timeline, NoKey, |a, _, _| crate::timeline_extras::play_around(a)),
    toggle(
        cmd("timeline.record_motion", tlm!("Record Motion"), None, G::Timeline, NoKey, |a, _, _| {
            let on = !a.recorder.armed;
            crate::motion_record::set_armed(a, on);
        }),
        |a| a.recorder.armed,
    ),
    toggle(cmd("timeline.ping_pong", tlm!("Ping-pong"), None, G::Timeline, NoKey, |a, _, _| a.playback.ping_pong = !a.playback.ping_pong), |a| {
        a.playback.ping_pong
    }),
    when(
        cmd("timeline.play_selection", tlm!("Play Selection"), Some(Icon::Play), G::Timeline, NoKey, |a, _, _| crate::timeline_extras::play_selection(a)),
        has_frames_selected,
    ),
    cmd("timeline.set_in", tlm!("Set In"), None, G::Timeline, NoKey, |a, _, _| crate::timeline_extras::set_in_out(a, true)),
    cmd("timeline.set_out", tlm!("Set Out"), None, G::Timeline, NoKey, |a, _, _| crate::timeline_extras::set_in_out(a, false)),
    when(cmd("timeline.clear_in_out", tlm!("Clear In and Out"), None, G::Timeline, NoKey, |a, _, _| a.tl.range = None), has_in_out),
    toggle(cmd("timeline.vertical", tlm!("Vertical"), None, G::Timeline, NoKey, |a, _, _| a.tl.vertical = !a.tl.vertical), |a| a.tl.vertical),
    toggle(cmd("timeline.timecode", tlm!("Timecode"), None, G::Timeline, NoKey, |a, _, _| a.tl.timecode = !a.tl.timecode), |a| a.tl.timecode),
    toggle(cmd("timeline.thumbnails", tlm!("Thumbnails"), Some(Icon::Drawings), G::Timeline, NoKey, |a, _, _| a.thumbnails = !a.thumbnails), |a| a.thumbnails),
    when(
        toggle(cmd("timeline.sound", tlm!("Play Sound"), Some(Icon::Sound), G::Timeline, NoKey, |a, _, _| a.sound.muted = !a.sound.muted), |a| !a.sound.muted),
        has_sound,
    ),
    toggle(cmd("view.field_guide", tlm!("Field Guide"), Some(Icon::Grid), G::View, NoKey, |a, _, _| a.canvas_ui.field_guide = !a.canvas_ui.field_guide), |a| {
        a.canvas_ui.field_guide
    }),
    toggle(cmd("view.safe_areas", tlm!("Safe Areas"), Some(Icon::SafeArea), G::View, NoKey, |a, _, _| a.canvas_ui.safe_areas = !a.canvas_ui.safe_areas), |a| {
        a.canvas_ui.safe_areas
    }),
    toggle(
        cmd("view.symmetry", tlm!("Symmetry"), Some(Icon::FlipH), G::View, NoKey, |a, _, _| {
            let mut s = a.engine.project.symmetry;
            s.mode = if s.is_on() { tooncraft_doc::symmetry::SymmetryMode::Off } else { tooncraft_doc::symmetry::SymmetryMode::Vertical };
            a.run(Command::SetSymmetry { symmetry: s });
        }),
        |a| a.engine.project.symmetry.is_on(),
    ),
];

/// The command with this id.
pub fn get(id: &str) -> Option<&'static Cmd> {
    ALL.iter().find(|c| c.id == id)
}

/// The commands the user can rebind, as (id, English label, default key).
pub fn remappable() -> impl Iterator<Item = (&'static str, &'static str, &'static str)> {
    ALL.iter().filter_map(|c| match c.key {
        Key::Remap(k) => Some((c.id, c.label, k)),
        _ => None,
    })
}

/// Run a command if it is available now. Returns whether it ran; an unavailable command says why
/// in the status bar.
pub fn run(app: &mut ToonApp, ctx: &egui::Context, id: &str) -> bool {
    let Some(c) = get(id) else {
        log::warn!("unknown command {id}");
        return false;
    };
    match (c.enabled)(app) {
        Ok(()) => {
            (c.run)(app, ctx, c.id);
            true
        }
        Err(why) => {
            app.set_status(tlf!("{command}: {reason}", command = c.label(), reason = why));
            false
        }
    }
}

/// Run a command if it is available now, quietly otherwise (for keys: a key that does nothing
/// here says nothing).
pub fn try_run(app: &mut ToonApp, ctx: &egui::Context, id: &str) -> bool {
    match get(id) {
        Some(c) if (c.enabled)(app).is_ok() => {
            (c.run)(app, ctx, c.id);
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn app() -> ToonApp {
        ToonApp::with_context(&egui::Context::default(), crate::args::Cli::default()).without_autosave()
    }

    #[test]
    fn ids_are_unique_and_every_command_has_a_label() {
        let mut ids = HashSet::new();
        for c in ALL {
            assert!(ids.insert(c.id), "{} is defined twice", c.id);
            assert!(!c.label().trim().is_empty(), "{} has no label", c.id);
            assert!(c.id.contains('.'), "{}: ids are group.name", c.id);
        }
    }

    #[test]
    fn default_keys_parse_and_do_not_collide() {
        let mut seen = std::collections::HashMap::new();
        for c in ALL {
            let k = match c.key {
                Key::Remap(k) | Key::Fixed(k) => k,
                Key::None => continue,
            };
            assert!(craft_keymap::Chord::parse(k).is_ok(), "{}: the key {k} does not parse", c.id);
            if let Some(other) = seen.insert(k, c.id) {
                panic!("{k} is the default of both {other} and {}", c.id);
            }
        }
    }

    #[test]
    fn the_saved_shortcut_ids_are_kept() {
        // ids users' shortcuts.json files refer to; renaming one loses their keys
        for id in [
            "tool.pencil",
            "tool.text",
            "frame.play",
            "frame.prev",
            "frame.next",
            "app.find_command",
            "view.onion",
            "brush.smaller",
            "marker.toggle",
            "timeline.zoom_out",
        ] {
            assert!(get(id).is_some_and(|c| matches!(c.key, Key::Remap(_))), "{id}");
        }
        // 51, then the zoom and rotate-view tools, then the radial menu
        assert_eq!(remappable().count(), 54);
    }

    #[test]
    fn disabled_commands_say_why_and_do_not_run() {
        let mut a = app();
        let ctx = egui::Context::default();
        assert!(a.selection.is_none());
        let c = get("select.flip_h").expect("registered");
        let why = (c.enabled)(&a).expect_err("no selection");
        assert!(!why.is_empty());
        assert!(!run(&mut a, &ctx, "select.flip_h"));
        assert!(a.status.contains(&why), "the status says why: {}", a.status);
        assert!(!run(&mut a, &ctx, "edit.undo"), "nothing to undo in a new shot");
        assert!(!run(&mut a, &ctx, "no.such_command"));
    }

    #[test]
    fn commands_run_and_toggles_report_their_state() {
        let mut a = app();
        let ctx = egui::Context::default();
        let onion = get("view.onion").and_then(|c| c.checked).expect("a toggle");
        let before = onion(&a);
        assert!(run(&mut a, &ctx, "view.onion"));
        assert_ne!(onion(&a), before);
        assert!(run(&mut a, &ctx, "tool.eraser"));
        assert_eq!(a.tool, Tool::Eraser);
        let len = a.engine.project.length();
        assert!(run(&mut a, &ctx, "frame.last"));
        assert_eq!(a.frame, len.saturating_sub(1));
        assert!(run(&mut a, &ctx, "frame.first"));
        assert_eq!(a.frame, 0);
    }

    #[test]
    fn every_tool_has_a_command() {
        for t in Tool::ALL {
            let id = format!("tool.{}", t.id());
            let c = get(&id).unwrap_or_else(|| panic!("{id}"));
            assert_eq!(c.icon, Some(Icon::from(t)));
        }
    }
}
