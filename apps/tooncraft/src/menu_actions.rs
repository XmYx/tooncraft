//! The bodies of the right-click menu commands that had no function of their own yet (design §5):
//! flip and rotate the whole drawing, paste frames reversed, fill and stroke the selection, the
//! ruler's loop and onion-range items, a layer's onion include, opacity and blend, swatch and
//! library-item commands, resetting a tool and a panel. Each has a check that says why it cannot
//! run now (shown in the disabled item's tooltip) and runs as one undo step when it changes the
//! shot. The command registry points at these; nothing here draws.

use tooncraft_brush::Brush;
use tooncraft_doc::{BlendMode, Cell, OnionSkin};
use tooncraft_engine::Command;

use crate::app::{Tool, ToonApp};
use crate::dock::Pane;

/// `Ok` when a command can run now, or the reason it cannot (translated).
pub type Enabled = Result<(), String>;

/// A drawing is shown on the current cell.
pub fn has_drawing(app: &ToonApp) -> Enabled {
    if app.engine.project.drawing_at(app.layer, app.frame).is_none() { Err(tl!("No drawing on this frame").to_string()) } else { Ok(()) }
}

pub fn unlocked(app: &ToonApp, layer: usize) -> Enabled {
    match app.engine.project.layers.get(layer) {
        None => Err(tl!("No such layer").to_string()),
        Some(l) if l.locked => Err(tl!("The layer is locked").to_string()),
        Some(_) => Ok(()),
    }
}

pub fn has_selection(app: &ToonApp) -> Enabled {
    if app.selection.is_none() { Err(tl!("Nothing is selected").to_string()) } else { Ok(()) }
}

// ----- canvas: flip and rotate -----

/// Flip and rotate act on the selection, or on the whole drawing when nothing is selected.
pub fn can_transform(app: &ToonApp) -> Enabled {
    if app.selection.is_none() {
        has_drawing(app)?;
    }
    unlocked(app, app.layer)
}

fn transform(app: &mut ToonApp, scale: (f32, f32), angle: f32, label: &str) {
    if let Err(why) = can_transform(app) {
        app.set_status(why);
        return;
    }
    if app.selection.is_some() {
        app.transform_selection(0.0, 0.0, scale, angle);
        return;
    }
    let (w, h) = (app.engine.project.width() as f32, app.engine.project.height() as f32);
    let (layer, frame, resample) = (app.layer, app.frame, app.sel.resample);
    let cmd = Command::TransformSelection {
        layer,
        frame,
        polygon: vec![(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)],
        rings: Vec::new(),
        dx: 0.0,
        dy: 0.0,
        scale_x: scale.0,
        scale_y: scale.1,
        angle,
        skew_x: 0.0,
        skew_y: 0.0,
        resample,
    };
    if app.run(Command::Batch { label: label.into(), commands: vec![cmd] }) {
        app.set_status(label.to_string());
    }
}

/// Flip the selection, or the whole drawing, horizontally (or vertically).
pub fn flip(app: &mut ToonApp, horizontal: bool) {
    let label = if horizontal { tl!("Flip horizontally") } else { tl!("Flip vertically") };
    transform(app, if horizontal { (-1.0, 1.0) } else { (1.0, -1.0) }, 0.0, label);
}

/// Turn the selection, or the whole drawing, a quarter turn.
pub fn rotate_90(app: &mut ToonApp, clockwise: bool) {
    let label = if clockwise { tl!("Rotate 90° clockwise") } else { tl!("Rotate 90° counter-clockwise") };
    let angle = if clockwise { std::f32::consts::FRAC_PI_2 } else { -std::f32::consts::FRAC_PI_2 };
    transform(app, (1.0, 1.0), angle, label);
}

/// Take the colour of the picture at a canvas point (as the picker tool does).
pub fn pick_colour_at(app: &mut ToonApp, x: f32, y: f32) {
    let (w, h) = (app.engine.project.width() as f32, app.engine.project.height() as f32);
    if !(x >= 0.0 && y >= 0.0 && x < w && y < h) {
        return;
    }
    let col = app.engine.pick_color(app.frame, x as u32, y as u32);
    app.color = [col[0], col[1], col[2], 255];
    app.set_status(tlf!("Picked #{hex}", hex = format!("{:02X}{:02X}{:02X}", col[0], col[1], col[2])));
}

// ----- selection -----

pub fn can_paint_selection(app: &ToonApp) -> Enabled {
    has_selection(app)?;
    unlocked(app, app.layer)
}

/// Paint the selection (holes kept) with the current colour.
pub fn fill_selection(app: &mut ToonApp) {
    if let Err(why) = can_paint_selection(app) {
        app.set_status(why);
        return;
    }
    let Some(area) = app.selection.clone() else { return };
    let (layer, frame, c) = (app.layer, app.frame, app.color);
    let area_rings = crate::selection_tools::extra_rings(app);
    let cmd = Command::Gradient {
        layer,
        frame,
        kind: Default::default(),
        from: (0.0, 0.0),
        to: (1.0, 0.0),
        start: c,
        end: c,
        area: Some(area),
        area_rings,
        stops: Vec::new(),
    };
    app.run(Command::Batch { label: tl!("Fill selection").to_string(), commands: vec![cmd] });
}

/// Draw the selection's outlines with the current brush (one undo step for every ring).
pub fn stroke_selection(app: &mut ToonApp) {
    if let Err(why) = can_paint_selection(app) {
        app.set_status(why);
        return;
    }
    let (layer, frame, brush) = (app.layer, app.frame, app.brush());
    let commands: Vec<Command> = crate::selection_tools::all_rings(app)
        .into_iter()
        .filter(|r| r.len() >= 3)
        .take(tooncraft_engine::MAX_BATCH)
        .map(|points| Command::PathShape { layer, frame, brush, points, closed: true, curve: false, outline: true, fill: None })
        .collect();
    app.run(Command::Batch { label: tl!("Stroke selection").to_string(), commands });
}

/// Keep the drawing at the cursor, with its timing, as a drawing-library item.
pub fn save_cell_to_library(app: &mut ToonApp) {
    if let Err(why) = has_drawing(app) {
        app.set_status(why);
        return;
    }
    let (layer, frame) = (app.layer, app.frame);
    let (from, to) =
        app.engine.project.layers.get(layer).and_then(|l| l.exposure_range(frame)).map(|(a, b)| (a, b.saturating_sub(1).max(a))).unwrap_or((frame, frame));
    let name = tlf!("Drawing {n}", n = app.engine.project.library.len() + 1);
    if app.run(Command::SaveToLibrary { layer, from, to, name: name.clone() }) {
        app.workspace.raise(Pane::Drawings);
        app.set_status(tlf!("Saved to the library as {name}", name = name));
    }
}

// ----- timeline cell -----

pub fn can_paste_cells(app: &ToonApp) -> Enabled {
    if app.cell_clipboard.is_none() { Err(tl!("Nothing copied yet").to_string()) } else { Ok(()) }
}

/// Paste the copied frames played backwards, each exposure keeping its length.
pub fn paste_reversed(app: &mut ToonApp) {
    let Some(block) = app.cell_clipboard.clone() else {
        app.set_status(tl!("Nothing copied yet"));
        return;
    };
    let reversed: Vec<Vec<Cell>> = block.iter().map(|row| tooncraft_doc::reverse_cells(row)).collect();
    let saved = app.cell_clipboard.replace(reversed);
    app.paste_cells(false, false);
    app.cell_clipboard = saved;
}

// ----- ruler -----

/// Put the loop's in (`start`) or out point on `frame`.
pub fn set_loop_at(app: &mut ToonApp, frame: u32, start: bool) {
    let keep = app.frame;
    app.frame = frame.min(app.engine.project.length().saturating_sub(1));
    crate::timeline_extras::set_in_out(app, start);
    app.frame = keep;
}

/// Show `steps` drawings either side in the onion skin, and turn it on.
pub fn onion_range(app: &mut ToonApp, steps: u8) {
    let o = OnionSkin { enabled: true, before: steps, after: steps, ..app.engine.project.onion };
    app.run(Command::SetOnion { onion: o.sanitized() });
}

/// Step the onion skin by key drawings only (on), or by every drawing (off).
pub fn onion_keys_only(app: &mut ToonApp, on: bool) {
    let o = OnionSkin { keys_only: on, ..app.engine.project.onion };
    app.run(Command::SetOnion { onion: o.sanitized() });
}

/// Go to a frame typed as the user counts them (from 1), clamped to the shot.
pub fn go_to_frame(app: &mut ToonApp, one_based: i64) {
    let len = i64::from(app.engine.project.length().max(1));
    app.frame = u32::try_from(one_based.clamp(1, len) - 1).unwrap_or(0);
    app.playback.stop();
}

// ----- layer row -----

/// Whether the layer's drawings show in the onion skin.
pub fn layer_in_onion(app: &ToonApp, layer: usize) -> bool {
    app.engine.project.layers.get(layer).is_none_or(|l| l.onion.is_none_or(|o| o.enabled))
}

/// Include the layer in the onion skin, or leave it out. A layer included again whose settings
/// are otherwise the shot's goes back to the shot's settings.
pub fn set_layer_in_onion(app: &mut ToonApp, layer: usize, on: bool) {
    let Some(l) = app.engine.project.layers.get(layer) else { return };
    let shot = app.engine.project.onion;
    let own = l.onion.unwrap_or(shot);
    let back_to_shot = on && (OnionSkin { enabled: shot.enabled, ..own }) == shot;
    let onion = if back_to_shot { None } else { Some(OnionSkin { enabled: on, ..own }) };
    app.run(Command::SetLayerOnion { layer, onion });
}

pub fn set_layer_opacity(app: &mut ToonApp, layer: usize, percent: u8) {
    let opacity = Some(f32::from(percent.min(100)) / 100.0);
    app.run(Command::SetLayer { layer, visible: None, locked: None, opacity });
}

pub fn set_layer_blend(app: &mut ToonApp, layer: usize, blend: BlendMode) {
    app.run(Command::SetLayerBlend { layer, blend });
}

pub fn can_add_peg(app: &ToonApp, layer: usize) -> Enabled {
    match app.engine.project.layers.get(layer) {
        None => Err(tl!("No such layer").to_string()),
        Some(l) if l.peg.is_some() => Err(tl!("The layer already hangs from a peg").to_string()),
        Some(_) => Ok(()),
    }
}

/// A new peg that the layer hangs from (one undo step).
pub fn add_peg(app: &mut ToonApp, layer: usize) {
    if let Err(why) = can_add_peg(app, layer) {
        app.set_status(why);
        return;
    }
    let n = app.engine.project.pegs.len() + 1;
    if app.run(Command::AddPegForLayer { layer, name: tlf!("Peg {n}", n = n) }) {
        app.workspace.raise(Pane::Pegs);
    }
}

// ----- swatch -----

/// Swatches of the built-in palette (index 0) are not the shot's, so they cannot be changed.
pub fn can_edit_swatch(app: &ToonApp) -> Enabled {
    if app.colour.current == 0 { Err(tl!("The basic palette cannot be changed; add the colour to a palette of the shot").to_string()) } else { Ok(()) }
}

/// A copy of swatch `swatch` right after it, in the palette shown.
pub fn duplicate_swatch(app: &mut ToonApp, swatch: usize) {
    if let Err(why) = can_edit_swatch(app) {
        app.set_status(why);
        return;
    }
    let palette = app.colour.current - 1;
    let mut v = app.engine.project.palettes.clone();
    let Some(p) = v.get_mut(palette) else { return };
    let Some(s) = p.swatches.get(swatch).cloned() else { return };
    p.swatches.insert(swatch + 1, s);
    app.run(Command::SetPalettes { palettes: v });
}

/// Remove swatch `swatch` from the palette shown.
pub fn delete_swatch(app: &mut ToonApp, swatch: usize) {
    if let Err(why) = can_edit_swatch(app) {
        app.set_status(why);
        return;
    }
    let palette = app.colour.current - 1;
    let mut v = app.engine.project.palettes.clone();
    if let Some(p) = v.get_mut(palette)
        && swatch < p.swatches.len()
    {
        p.swatches.remove(swatch);
        app.run(Command::SetPalettes { palettes: v });
    }
}

/// Select the pixels of the current drawing in `rgb` (within the wand's tolerance).
pub fn select_colour(app: &mut ToonApp, rgb: [u8; 3]) {
    if let Err(why) = has_drawing(app) {
        app.set_status(why);
        return;
    }
    let tol = app.sel.wand_tolerance;
    match tooncraft_engine::select::colour_mask(&app.engine, app.layer, app.frame, rgb, tol) {
        Some(m) => {
            crate::selection_tools::apply_mask(app, &m, tooncraft_raster::select::Combine::Replace);
            app.tool = Tool::Select;
            app.set_status(tlf!("Selected {n} pixels", n = m.count()));
        }
        None => app.set_status(tl!("No pixels of this colour in the drawing")),
    }
}

// ----- brush preset -----

/// Built-in brushes cannot be renamed, changed or deleted.
pub fn can_change_preset(app: &ToonApp, id: &str) -> Enabled {
    match app.library.library.get(id).map(|e| &e.source) {
        None => Err(tl!("That brush is not in the library").to_string()),
        Some(tooncraft_engine::library::Source::Builtin(_)) => Err(tl!("Built-in brushes cannot be changed; duplicate one to make your own").to_string()),
        Some(_) => Ok(()),
    }
}

/// The current brush can go into a preset of the user's of the same kind.
pub fn can_save_into_preset(app: &ToonApp, id: &str) -> Enabled {
    can_change_preset(app, id)?;
    let kind = app.library.library.get(id).map(|e| e.kind);
    match app.tool.brush() {
        Some(k) if Some(k) == kind => Ok(()),
        _ => Err(tl!("The current tool is not a brush of this kind").to_string()),
    }
}

fn report(app: &mut ToonApp, r: Result<(), String>, done: String) {
    match r {
        Ok(()) => app.set_status(done),
        Err(e) => app.set_status(e),
    }
}

pub fn rename_preset(app: &mut ToonApp, id: &str, name: &str) {
    let r = app.library.library.rename(id, name);
    report(app, r, tlf!("Brush renamed to {name}", name = name.trim()));
}

/// A copy of the preset, chosen in the library.
pub fn duplicate_preset(app: &mut ToonApp, id: &str) {
    match app.library.library.duplicate(id, tl!("copy")) {
        Ok(new) => {
            app.library.selected = Some(new);
            app.set_status(tl!("Brush duplicated"));
        }
        Err(e) => app.set_status(e),
    }
}

pub fn delete_preset(app: &mut ToonApp, id: &str) {
    let r = app.library.library.remove(id);
    report(app, r, tl!("Brush deleted").to_string());
}

/// Save the current brush's settings into the preset (its name and tip stay).
pub fn save_current_into_preset(app: &mut ToonApp, id: &str) {
    if let Err(why) = can_save_into_preset(app, id) {
        app.set_status(why);
        return;
    }
    let Some(kind) = app.tool.brush() else { return };
    let brush = *app.brush_mut(kind);
    let r = app.library.library.overwrite(id, &brush);
    report(app, r, tl!("Brush saved").to_string());
}

/// Write the preset to `path` as a brush file to share.
pub fn export_preset(app: &mut ToonApp, id: &str, path: &std::path::Path) {
    let r = app.library.library.export(id, path);
    report(app, r, tlf!("Saved {file}", file = path.display()));
}

// ----- drawing library item -----

/// Put the item on a new layer above the current one, from the current frame (one undo step).
pub fn place_as_new_layer(app: &mut ToonApp, item: usize) {
    let Some(name) = app.engine.project.library.get(item).map(|i| i.name.clone()) else { return };
    let layer = (app.layer + 1).min(app.engine.project.layers.len());
    let frame = app.frame;
    let commands = vec![Command::AddLayer { name, index: Some(layer) }, Command::InsertFromLibrary { item, layer, frame, linked: false }];
    if app.run(Command::Batch { label: tl!("Place as new layer").to_string(), commands }) {
        app.layer = layer;
    }
}

/// Library items live inside the shot file, so there is no folder to show.
pub fn can_show_item_in_folder() -> Enabled {
    Err(tl!("Kept inside the shot; save it to My Drawings to have it as a file").to_string())
}

// ----- tool button -----

pub fn can_reset_tool(tool: Tool) -> Enabled {
    if tool.brush().is_none() { Err(tl!("This tool has no brush settings").to_string()) } else { Ok(()) }
}

/// Put the tool's brush back to its built-in settings (the colour stays: it is the app's).
pub fn reset_tool(app: &mut ToonApp, tool: Tool) {
    let Some(kind) = tool.brush() else {
        app.set_status(tl!("This tool has no brush settings"));
        return;
    };
    // the preset, with the smoothing the user chose in Preferences
    let smoothing = app.preferences.prefs.smoothing;
    let b = app.brush_mut(kind);
    *b = Brush::preset(kind);
    b.smoothing.average = smoothing;
    app.set_status(tlf!("{tool} settings reset", tool = tool.label()));
}

/// Open the shortcut editor at the tool's command.
pub fn edit_tool_shortcut(app: &mut ToonApp, tool: Tool) {
    app.keys.filter = crate::keys::label(&format!("tool.{}", tool.id()));
    app.show_shortcuts = true;
}

// ----- panel tab -----

/// Put a panel back in its first place (a closed panel is opened there).
pub fn reset_panel(app: &mut ToonApp, pane: Pane) {
    if app.workspace.reset_panel(pane) {
        app.set_status(tlf!("{panel} is back in its place", panel = pane.label()));
    }
}
