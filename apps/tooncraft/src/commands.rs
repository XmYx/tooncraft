//! The command registry: one static table of every user command, with its id, label, icon, group,
//! default key, when it is available (and why not) and what it does. The menu bar, the right-click
//! menus ([`crate::menus::context`]), Find Command and the shortcut editor all read it, so a command
//! added here shows up everywhere with the same label, icon and key.
//!
//! Ids are stable: the remappable ones are the ids the user's saved shortcuts refer to.

use crate::app::{Tool, ToonApp};
use crate::dock::Pane;
use crate::icons::Icon;
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

fn has_brush(a: &ToonApp) -> Result<(), Reason> {
    if a.tool.brush().is_some() { Ok(()) } else { Err(tl!("The tool has no brush").to_string()) }
}

fn pick_tool(a: &mut ToonApp, _: &egui::Context, id: &'static str) {
    if a.engine.is_stroking() {
        return;
    }
    if let Some(t) = Tool::ALL.into_iter().find(|t| id.strip_prefix("tool.") == Some(t.id())) {
        a.tool = t;
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
    // ── playback and frames ──
    cmd("frame.play", tlm!("Play / stop"), Some(Icon::Play), G::Frame, Remap("Enter"), |a, _, _| a.toggle_play()),
    cmd("frame.prev", tlm!("Previous frame"), Some(Icon::PrevFrame), G::Frame, Remap(","), |a, _, _| a.go_to(a.frame as i64 - 1)),
    cmd("frame.next", tlm!("Next frame"), Some(Icon::NextFrame), G::Frame, Remap("."), |a, _, _| a.go_to(a.frame as i64 + 1)),
    cmd("frame.prev_drawing", tlm!("Previous drawing"), Some(Icon::PrevDrawing), G::Frame, Remap("Ctrl+Left"), |a, _, _| a.step_drawing(false)),
    cmd("frame.next_drawing", tlm!("Next drawing"), Some(Icon::NextDrawing), G::Frame, Remap("Ctrl+Right"), |a, _, _| a.step_drawing(true)),
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
        assert_eq!(remappable().count(), 51);
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
