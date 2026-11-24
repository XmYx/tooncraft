//! Side panels and bars: tool options, colour, brush, layers, onion skin and light table (the
//! tool rail is `tool_rail.rs`). Every value set by dragging (sliders, the colour field) also has a typed or keyboard
//! alternative: sliders and drag values take arrow keys and typing, colours take hex and RGB.

use egui::{Color32, Sense, Stroke, vec2};
use tooncraft_brush::{BrushKind, Smoothing};
use tooncraft_doc::{MAX_PIN_SHIFT, OnionMode, PinShift};
use tooncraft_engine::Command;

use crate::app::{Tool, ToonApp};

fn hex(c: [u8; 4]) -> String {
    format!("{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// The Colour panel.
/// The current and previous colour, side by side; a click on the previous one swaps back.
fn colour_chips(app: &mut ToonApp, ui: &mut egui::Ui) {
    let seen_id = egui::Id::unique("tooncraft-colour-seen");
    let prev_id = egui::Id::unique("tooncraft-colour-previous");
    let cur = [app.color[0], app.color[1], app.color[2]];
    let (seen, mut prev) = ui.data(|d| (d.get_temp::<[u8; 3]>(seen_id), d.get_temp::<[u8; 3]>(prev_id)));
    if let Some(s) = seen
        && s != cur
    {
        prev = Some(s);
    }
    ui.horizontal(|ui| {
        let (rect, resp) = ui.allocate_exact_size(vec2(40.0, 26.0), Sense::hover());
        ui.painter().rect(rect, 3.0, Color32::from_rgb(cur[0], cur[1], cur[2]), Stroke::new(1.0, Color32::from_gray(120)), egui::StrokeKind::Inside);
        let label = tlf!("Current colour #{hex}", hex = hex(app.color));
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, label.clone()));
        if let Some(p) = prev {
            let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
            ui.painter().rect(rect, 3.0, Color32::from_rgb(p[0], p[1], p[2]), Stroke::new(1.0, Color32::from_gray(120)), egui::StrokeKind::Inside);
            if resp.has_focus() {
                ui.painter().rect_stroke(rect, 3.0, ui.visuals().selection.stroke, egui::StrokeKind::Outside);
            }
            let label = tlf!("Previous colour #{hex}", hex = hex([p[0], p[1], p[2], 255]));
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label.clone()));
            if resp.on_hover_text(tl!("Back to the previous colour")).clicked() {
                crate::colour_tools::set_rgb(app, p);
            }
        }
        // the current brush's opacity, next to its colour
        if let Some(kind) = app.tool.brush() {
            let b = app.brush_mut(kind);
            let l = ui.label(tl!("Brush opacity"));
            ui.add(egui::Slider::new(&mut b.opacity, 0.0..=1.0).max_decimals(2)).labelled_by(l.id);
        }
    });
    let now = [app.color[0], app.color[1], app.color[2]];
    ui.data_mut(|d| {
        d.insert_temp(seen_id, now);
        if let Some(p) = prev.filter(|p| *p != now).or(prev) {
            d.insert_temp(prev_id, p);
        }
    });
}

pub fn colour(app: &mut ToonApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().id_salt("colour-scroll").show(ui, |ui| {
        colour_chips(app, ui);
        ui.horizontal(|ui| {
            ui.checkbox(&mut app.colour.wheel, tl!("Hue wheel"));
            ui.toggle_value(&mut app.colour.window, tl!("Picker Window")).on_hover_text(tl!("The picker in a window of its own"));
        });
        let mut rgb = [app.color[0], app.color[1], app.color[2]];
        if app.colour.wheel && crate::colour_tools::wheel(ui, &mut rgb) {
            crate::colour_tools::set_rgb(app, rgb);
        }
        if crate::color::picker(ui, "drawing-colour", &mut rgb) {
            crate::colour_tools::set_rgb(app, rgb);
        }
        crate::colour_tools::opacity(app, ui);
        ui.horizontal(|ui| {
            for (i, name) in [tl!("Red"), tl!("Green"), tl!("Blue")].iter().enumerate() {
                let l = ui.label(*name);
                ui.add(egui::DragValue::new(&mut app.color[i]).range(0..=255)).labelled_by(l.id);
            }
        });
        ui.horizontal(|ui| {
            let l = ui.label(tl!("Hex"));
            let mut h = hex(app.color);
            if ui.add(egui::TextEdit::singleline(&mut h).desired_width(70.0)).labelled_by(l.id).changed()
                && let Some(c) = parse_hex(&h)
            {
                crate::colour_tools::set_rgb(app, [c[0], c[1], c[2]]);
            }
        });
        ui.separator();
        crate::colour_tools::harmonies(app, ui);
        ui.separator();
        crate::colour_tools::palettes(app, ui);
        ui.separator();
        crate::history_panel::recent(app, ui);
        ui.separator();
        crate::colour_tools::recolour(app, ui);
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(tl!("Paper"));
            let mut pc = app.engine.project.paper.color;
            if crate::color::swatch_edit(ui, "paper", tl!("Paper colour"), &mut pc) {
                app.run(Command::SetPaper { color: pc });
            }
            ui.checkbox(&mut app.engine.project.paper.export, tl!("Export paper"));
        });
    });
}

fn parse_hex(s: &str) -> Option<[u8; 4]> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8, 255])
}

/// The Brush panel: presets and every brush setting.
pub fn brush(app: &mut ToonApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().id_salt("brush-scroll").show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for (t, k) in [
                (Tool::Pencil, BrushKind::Pencil),
                (Tool::Ink, BrushKind::Ink),
                (Tool::Airbrush, BrushKind::Airbrush),
                (Tool::Chalk, BrushKind::Chalk),
                (Tool::Eraser, BrushKind::Eraser),
                (Tool::Watercolour, BrushKind::Watercolour),
                (Tool::Smudge, BrushKind::Smudge),
            ] {
                if ui.selectable_label(app.tool == t, t.label()).clicked() {
                    app.tool = t;
                }
                let _ = k;
            }
        });
        let Some(kind) = app.tool.brush() else {
            ui.label(tl!("Choose a brush tool to change its settings."));
            return;
        };
        ui.separator();
        let b = app.brush_mut(kind);
        egui::Grid::new("brush-grid").num_columns(2).spacing(vec2(8.0, 5.0)).show(ui, |ui| {
            let row = |ui: &mut egui::Ui, label: String, v: &mut f32, r: std::ops::RangeInclusive<f32>| {
                let l = ui.label(label);
                ui.add(egui::Slider::new(v, r).max_decimals(2)).labelled_by(l.id);
                ui.end_row();
            };
            row(ui, tl!("Size").into(), &mut b.size, 0.5..=200.0);
            row(ui, tl!("Opacity").into(), &mut b.opacity, 0.0..=1.0);
            row(ui, tl!("Flow").into(), &mut b.flow, 0.01..=1.0);
            row(ui, tl!("Hardness").into(), &mut b.hardness, 0.0..=1.0);
            row(ui, tl!("Spacing").into(), &mut b.spacing, 0.02..=1.0);
            row(ui, tl!("Paper grain").into(), &mut b.grain, 0.0..=1.0);
            row(ui, tl!("Grain size").into(), &mut b.grain_scale, 0.5..=16.0);
            row(ui, tl!("Smoothing").into(), &mut b.smoothing.average, 0.0..=1.0);
            row(ui, tl!("Steady line (px)").into(), &mut b.smoothing.string, 0.0..=120.0);
        });
        let Smoothing { catch_up, .. } = &mut b.smoothing;
        ui.checkbox(catch_up, tl!("Finish the line where the pen lifts"));
        if ui.button(tl!("Reset Brush")).clicked() {
            *app.brush_mut(kind) = tooncraft_brush::Brush::preset(kind);
        }
        ui.horizontal(|ui| {
            let l = ui.label(tl!("Mouse pressure"));
            ui.add(egui::Slider::new(&mut app.mouse_pressure, 0.05..=1.0).max_decimals(2))
                .labelled_by(l.id)
                .on_hover_text(tl!("Pressure used when drawing with a mouse"));
        });
        ui.separator();
        crate::brush_editor::sections(app, ui, kind);
        ui.separator();
        tips_and_presets(app, ui, kind);
    });
}

pub(crate) fn next_tip_id(app: &ToonApp) -> u64 {
    app.engine.project.tips.keys().next_back().map(|k| k + 1).unwrap_or(1)
}

/// Folder of the user's brush presets.
pub fn preset_dir() -> Option<std::path::PathBuf> {
    crate::keys::overrides_path().and_then(|p| p.parent().map(|d| d.join("brushes")))
}

/// Put a preset to use: its images join the shot's tip table and its brush replaces the current
/// brush of its kind.
pub(crate) fn use_preset(app: &mut ToonApp, path: &std::path::Path) {
    match tooncraft_engine::preset::Preset::load(path) {
        Ok(preset) => apply_preset(app, preset),
        Err(e) => app.set_status(e),
    }
}

pub(crate) fn apply_preset(app: &mut ToonApp, preset: tooncraft_engine::preset::Preset) {
    let tooncraft_engine::preset::Preset { name, mut brush, tip, grain } = preset;
    if let Some(t) = tip {
        let id = next_tip_id(app);
        if app.run(tooncraft_engine::Command::AddTip { id, name: t.name, width: t.width, height: t.height, alpha: t.alpha }) {
            brush.tip = Some(id);
        }
    }
    if let Some(g) = grain {
        let id = next_tip_id(app);
        if app.run(tooncraft_engine::Command::AddTip { id, name: g.name, width: g.width, height: g.height, alpha: g.alpha }) {
            brush.grain_image = Some(id);
        } else {
            brush.grain_kind = tooncraft_brush::GrainKind::Paper;
        }
    }
    let kind = brush.kind;
    app.color = brush.color;
    *app.brush_mut(kind) = brush;
    if let Some(t) = Tool::ALL.into_iter().find(|t| t.brush() == Some(kind)) {
        app.tool = t;
    }
    app.set_status(tlf!("Brush {name} loaded", name = name));
}

/// Image tips and brush preset files.
fn tips_and_presets(app: &mut ToonApp, ui: &mut egui::Ui, kind: BrushKind) {
    let current = app.brushes[BrushKind::ALL.iter().position(|k| *k == kind).unwrap_or(0)].tip;
    let name = |id: Option<u64>| match id.and_then(|i| app.engine.project.tips.get(&i)) {
        Some(t) => t.name.clone(),
        None => tl!("Round").to_string(),
    };
    let mut choice = current;
    let l = ui.label(tl!("Tip"));
    let tips: Vec<u64> = app.engine.project.tips.keys().copied().collect();
    egui::ComboBox::from_id_salt("brush-tip")
        .selected_text(name(current))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut choice, None, tl!("Round"));
            for id in &tips {
                ui.selectable_value(&mut choice, Some(*id), name(Some(*id)));
            }
        })
        .response
        .labelled_by(l.id);
    if choice != current {
        app.brush_mut(kind).tip = choice;
    }
    ui.horizontal_wrapped(|ui| {
        if ui.button(tl!("Tip from Image…")).clicked()
            && let Some(f) = rfd::FileDialog::new().add_filter(tl!("Images"), &["png", "jpg", "jpeg", "webp", "tif", "tiff", "bmp"]).pick_file()
        {
            let id = next_tip_id(app);
            if app.run(tooncraft_engine::Command::AddTipFromImage { id, path: f.to_string_lossy().into_owned() }) {
                app.brush_mut(kind).tip = Some(id);
            }
        }
        if ui.button(tl!("Save Preset…")).clicked() {
            let brush = tooncraft_brush::Brush { color: app.color, ..app.brushes[BrushKind::ALL.iter().position(|k| *k == kind).unwrap_or(0)] };
            let tip = brush.tip.and_then(|i| app.engine.project.tips.get(&i)).map(|t| (**t).clone());
            let grain = brush.grain_image.and_then(|i| app.engine.project.tips.get(&i)).map(|t| (**t).clone());
            let dir = preset_dir();
            if let Some(d) = &dir {
                let _ = std::fs::create_dir_all(d);
            }
            let mut dialog = rfd::FileDialog::new()
                .add_filter(tl!("Brush preset"), &[tooncraft_engine::preset::EXTENSION])
                .set_file_name(format!("{:?}.{}", kind, tooncraft_engine::preset::EXTENSION).to_lowercase());
            if let Some(d) = &dir {
                dialog = dialog.set_directory(d);
            }
            if let Some(f) = dialog.save_file() {
                let n = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                match tooncraft_engine::preset::Preset::new(&n, &brush, tip.as_ref(), grain.as_ref()).save(&f) {
                    Ok(()) => app.set_status(tlf!("Saved brush {name}", name = n)),
                    Err(e) => app.set_status(e),
                }
            }
        }
        if ui.button(tl!("Load Preset…")).clicked()
            && let Some(f) = rfd::FileDialog::new().add_filter(tl!("Brush preset"), &[tooncraft_engine::preset::EXTENSION]).pick_file()
        {
            use_preset(app, &f);
        }
    });
    // the preset library
    if let Some(dir) = preset_dir()
        && let Ok(entries) = std::fs::read_dir(&dir)
    {
        let mut files: Vec<std::path::PathBuf> =
            entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == tooncraft_engine::preset::EXTENSION)).take(200).collect();
        files.sort();
        if !files.is_empty() {
            ui.label(tl!("My brushes"));
            ui.horizontal_wrapped(|ui| {
                for f in files {
                    let n = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    if ui.button(&n).on_hover_text(tlf!("Use brush {name}", name = n)).clicked() {
                        use_preset(app, &f);
                    }
                }
            });
        }
    }
}

/// The Layers panel (top layer first).
pub fn layers(app: &mut ToonApp, ui: &mut egui::Ui) {
    crate::layers_list::toolbar(app, ui);
    ui.separator();
    crate::layers_list::rows(app, ui);
    ui.separator();
    layer_properties(app, ui);
    ui.separator();
    let unused = app.engine.project.unused_drawings().len();
    ui.label(tlf!("{drawings} drawings ({unused} not exposed)", drawings = app.engine.project.drawing_count(), unused = unused));
}

pub fn blend_name(b: tooncraft_doc::BlendMode) -> String {
    use tooncraft_doc::BlendMode::*;
    match b {
        Normal => tl!("Normal"),
        Multiply => tl!("Multiply"),
        Screen => tl!("Screen"),
        Overlay => tl!("Overlay"),
        Darken => tl!("Darken"),
        Lighten => tl!("Lighten"),
        Add => tl!("Add"),
        ColorDodge => tl!("Colour dodge"),
        ColorBurn => tl!("Colour burn"),
        HardLight => tl!("Hard light"),
        SoftLight => tl!("Soft light"),
        Difference => tl!("Difference"),
        Exclusion => tl!("Exclusion"),
        LinearBurn => tl!("Linear burn"),
        LinearLight => tl!("Linear light"),
        VividLight => tl!("Vivid light"),
        PinLight => tl!("Pin light"),
        HardMix => tl!("Hard mix"),
        Subtract => tl!("Subtract"),
        Divide => tl!("Divide"),
        Hue => tl!("Hue"),
        Saturation => tl!("Saturation"),
        Colour => tl!("Colour"),
        Luminosity => tl!("Luminosity"),
    }
    .to_string()
}

/// Blend mode, label colour and folder of the current layer, and the folders' switches.
fn layer_properties(app: &mut ToonApp, ui: &mut egui::Ui) {
    let li = app.layer;
    let Some(layer) = app.engine.project.layers.get(li).cloned() else { return };
    egui::Grid::new("layer-props").num_columns(2).spacing(vec2(8.0, 5.0)).show(ui, |ui| {
        let l = ui.label(tl!("Blend"));
        let mut blend = layer.blend;
        egui::ComboBox::from_id_salt("layer-blend")
            .selected_text(blend_name(blend))
            .show_ui(ui, |ui| {
                for b in tooncraft_doc::BlendMode::ALL {
                    ui.selectable_value(&mut blend, b, blend_name(b));
                }
            })
            .response
            .labelled_by(l.id);
        if blend != layer.blend {
            app.run(Command::SetLayerBlend { layer: li, blend });
        }
        ui.end_row();
        ui.label(tl!("Colour"));
        ui.horizontal(|ui| {
            let mut c = layer.color.unwrap_or([150, 150, 150]);
            if crate::color::swatch_edit(ui, "layer-colour", tl!("Layer colour"), &mut c) {
                app.run(Command::SetLayerColor { layer: li, color: Some(c) });
            }
            if layer.color.is_some() && ui.small_button(tl!("None")).clicked() {
                app.run(Command::SetLayerColor { layer: li, color: None });
            }
        });
        ui.end_row();
        let l = ui.label(tl!("Folder"));
        let key = egui::Id::unique("layer-folder-edit");
        let mut text = ui
            .data(|d| d.get_temp::<(usize, String)>(key))
            .filter(|(i, _)| *i == li)
            .map(|(_, t)| t)
            .unwrap_or_else(|| layer.folder.clone().unwrap_or_default());
        let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(110.0)).labelled_by(l.id);
        ui.data_mut(|d| d.insert_temp(key, (li, text.clone())));
        if r.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) && r.has_focus() {
            let folder = Some(text.trim().to_string()).filter(|t| !t.is_empty());
            if folder != layer.folder {
                app.run(Command::SetLayerFolder { layer: li, folder });
            }
        }
        ui.end_row();
    });
    crate::layer_ops::properties(app, ui);
    crate::layer_tree::folders(app, ui);
}

/// The Layer menu (menu bar).
pub fn layer_menu(app: &mut ToonApp, ui: &mut egui::Ui) {
    let n = app.engine.project.layers.len();
    let layer = app.layer;
    if ui.button(tl!("Select Layer Above")).clicked() {
        app.step_layer(true);
    }
    if ui.button(tl!("Select Layer Below")).clicked() {
        app.step_layer(false);
    }
    ui.separator();
    if ui.button(tl!("New Layer")).clicked() {
        let name = tlf!("Layer {n}", n = n + 1);
        if app.run(Command::AddLayer { name, index: Some(layer + 1) }) {
            app.layer += 1;
        }
    }
    if ui.button(tl!("Delete Layer")).clicked() {
        app.run(Command::DeleteLayer { layer });
        app.clamp_selection();
    }
    if ui.button(tl!("Rename Layer…")).clicked() {
        let name = app.engine.project.layers.get(layer).map(|l| l.name.clone()).unwrap_or_default();
        app.renaming = Some((layer, name));
        app.workspace.raise(crate::dock::Pane::Layers);
    }
    if ui.add_enabled(layer + 1 < n, egui::Button::new(tl!("Move Layer Up"))).clicked() && app.run(Command::MoveLayer { from: layer, to: layer + 1 }) {
        app.layer += 1;
    }
    if ui.add_enabled(layer > 0, egui::Button::new(tl!("Move Layer Down"))).clicked() && app.run(Command::MoveLayer { from: layer, to: layer - 1 }) {
        app.layer -= 1;
    }
    ui.separator();
    let (vis, lock) = app.engine.project.layers.get(layer).map(|l| (l.visible, l.locked)).unwrap_or((true, false));
    if ui.button(if vis { tl!("Hide Layer") } else { tl!("Show Layer") }).clicked() {
        app.run(Command::SetLayer { layer, visible: Some(!vis), locked: None, opacity: None });
    }
    if ui.button(if lock { tl!("Unlock Layer") } else { tl!("Lock Layer") }).clicked() {
        app.run(Command::SetLayer { layer, visible: None, locked: Some(!lock), opacity: None });
    }
    crate::layer_ops::menu(app, ui);
}

/// The Onion Skin panel, with the light table.
/// A small vertical opacity slider for one onion-skin step, in the side's tint.
fn step_slider(ui: &mut egui::Ui, v: &mut f32, tint: [u8; 3], name: String) {
    ui.scope(|ui| {
        let c = Color32::from_rgb(tint[0], tint[1], tint[2]);
        ui.visuals_mut().selection.bg_fill = c;
        ui.spacing_mut().slider_width = 70.0;
        let resp = ui.add(egui::Slider::new(v, 0.0..=1.0).vertical().show_value(false));
        let value = *v;
        resp.widget_info(|| egui::WidgetInfo::slider(true, value as f64, name.clone()));
    });
}

pub fn onion(app: &mut ToonApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().id_salt("onion-scroll").show(ui, |ui| {
        // the current layer may have its own settings
        let li = app.layer;
        let own_before = app.engine.project.layers.get(li).and_then(|l| l.onion);
        let mut own = own_before.is_some();
        let lname = app.engine.project.layers.get(li).map(|l| l.name.clone()).unwrap_or_default();
        if ui
            .checkbox(&mut own, tlf!("Own settings for {layer}", layer = lname))
            .on_hover_text(tl!("This layer gets its own steps, opacity and colours; the others keep the shot's"))
            .changed()
        {
            let onion = own.then_some(app.engine.project.onion);
            app.run(Command::SetLayerOnion { layer: li, onion });
        }
        let own_now = app.engine.project.layers.get(li).and_then(|l| l.onion);
        let start = own_now.unwrap_or(app.engine.project.onion);
        let mut o = start;
        ui.checkbox(&mut o.enabled, tl!("Show onion skin"));
        egui::Grid::new("onion-grid").num_columns(2).spacing(vec2(8.0, 5.0)).show(ui, |ui| {
            let l = ui.label(tl!("Before"));
            ui.add(egui::DragValue::new(&mut o.before).range(0..=10)).labelled_by(l.id);
            ui.end_row();
            let l = ui.label(tl!("After"));
            ui.add(egui::DragValue::new(&mut o.after).range(0..=10)).labelled_by(l.id);
            ui.end_row();
            let l = ui.label(tl!("Opacity"));
            ui.add(egui::Slider::new(&mut o.opacity, 0.0..=1.0)).labelled_by(l.id);
            ui.end_row();
            let l = ui.label(tl!("Fade per step"));
            ui.add(egui::Slider::new(&mut o.falloff, 0.0..=1.0)).labelled_by(l.id);
            ui.end_row();
            ui.label(tl!("Step by"));
            ui.horizontal(|ui| {
                ui.radio_value(&mut o.mode, OnionMode::Drawings, tl!("Drawings"));
                ui.radio_value(&mut o.mode, OnionMode::Frames, tl!("Frames"));
            });
            ui.end_row();
            let l = ui.label(tl!("Colours"));
            let name = |tinted: bool| if tinted { tl!("Tinted before and after") } else { tl!("The drawings' own colours") };
            egui::ComboBox::from_id_salt("onion-render")
                .selected_text(name(o.tinted))
                .show_ui(ui, |ui| {
                    for t in [true, false] {
                        ui.selectable_value(&mut o.tinted, t, name(t));
                    }
                })
                .response
                .labelled_by(l.id);
            ui.end_row();
            ui.label(tl!("Tint"));
            ui.horizontal(|ui| {
                crate::color::swatch_edit(ui, "onion-before", tl!("Tint before"), &mut o.before_tint);
                crate::color::swatch_edit(ui, "onion-after", tl!("Tint after"), &mut o.after_tint);
            });
            ui.end_row();
        });
        ui.checkbox(&mut o.keys_only, tl!("Key drawings only")).on_hover_text(tl!("Stepping by drawings, show only drawings marked as keys"));
        picked_frames(app, ui, &mut o);
        ui.checkbox(&mut o.all_layers, tl!("Onion skin on every visible layer"));
        ui.checkbox(&mut o.per_step, tl!("Opacity per step"));
        if o.per_step {
            // one small vertical slider per step: before (nearest on the right) | after
            ui.horizontal(|ui| {
                for i in (0..o.before as usize).rev() {
                    step_slider(ui, &mut o.before_steps[i], o.before_tint, tlf!("Before, step {n}", n = i + 1));
                }
                ui.separator();
                for i in 0..o.after as usize {
                    step_slider(ui, &mut o.after_steps[i], o.after_tint, tlf!("After, step {n}", n = i + 1));
                }
            });
        }
        ui.checkbox(&mut o.outline, tl!("Outlines only"));
        ui.checkbox(&mut o.hide_previous, tl!("Hide the previous step"));
        if o != start {
            if own_now.is_some() {
                app.run(Command::SetLayerOnion { layer: li, onion: Some(o) });
            } else {
                app.engine.project.onion = o.sanitized();
            }
        }
        ui.separator();
        ui.strong(tl!("Light table"));
        let mut lt = app.engine.project.light_table.clone();
        ui.checkbox(&mut lt.enabled, tl!("Show pinned frames"));
        let l = ui.label(tl!("Opacity"));
        ui.add(egui::Slider::new(&mut lt.opacity, 0.0..=1.0)).labelled_by(l.id);
        let (layer, frame) = (app.layer, app.frame);
        let pinned = app.engine.project.layers.get(layer).is_some_and(|l| lt.pins.iter().any(|p| p.is(l.id, frame)));
        if ui.button(if pinned { tl!("Unpin This Frame") } else { tl!("Pin This Frame") }).clicked() {
            app.run(Command::TogglePin { layer, frame });
            lt.pins = app.engine.project.light_table.pins.clone();
        }
        let mut remove = None;
        let mut shift = None;
        for (i, pin) in lt.pins.iter().enumerate() {
            let name = app.engine.project.layer_index(pin.layer).and_then(|li| app.engine.project.layers.get(li)).map(|l| l.name.clone()).unwrap_or_default();
            let label = tlf!("{layer}, frame {frame}", layer = name, frame = pin.frame + 1);
            ui.horizontal(|ui| {
                ui.label(&label);
                if ui.small_button(tl!("Remove")).on_hover_text(tlf!("Unpin {pin}", pin = label)).clicked() {
                    remove = Some(i);
                }
            });
            // shift and trace: move or turn the pinned frame to trace over it; the drawing is
            // not changed
            if let Some(s) = shift_row(ui, i, pin.shift, &label) {
                shift = Some((i, s));
            }
        }
        if let Some(i) = remove {
            lt.pins.remove(i);
        }
        if let Some((index, s)) = shift {
            app.run(Command::ShiftPin { index, x: s.x, y: s.y, turn: s.turn });
            lt.pins = app.engine.project.light_table.pins.clone();
        }
        if lt != app.engine.project.light_table {
            app.engine.project.light_table = lt.sanitized();
        }
    });
}

/// The "Exact frames" switch and the typed list of offsets it shows (`-4 -2 +2`). A list that
/// does not read stays in the field with the reason in the status bar.
fn picked_frames(app: &mut ToonApp, ui: &mut egui::Ui, o: &mut tooncraft_doc::OnionSkin) {
    ui.checkbox(&mut o.pick, tl!("Exact frames"))
        .on_hover_text(tl!("Show the frames at these offsets from the current one instead of stepping, such as -4 -2 +2"));
    if !o.pick {
        return;
    }
    let id = ui.make_persistent_id("onion-picked-text");
    let mut text: String = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| o.picked_text());
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        let l = ui.label(tl!("Offsets"));
        let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(140.0).hint_text("-4 -2 +2")).labelled_by(l.id);
        if r.changed() {
            match tooncraft_doc::OnionSkin::parse_picked(&text) {
                Ok(a) => o.picked = a,
                Err(e) => app.set_status(pick_error(&e)),
            }
        }
        if r.lost_focus() {
            text = o.picked_text();
        }
    });
    ui.data_mut(|d| d.insert_temp(id, text));
}

/// A refused offset list, in words.
pub fn pick_error(e: &tooncraft_doc::PickError) -> String {
    match e {
        tooncraft_doc::PickError::NotANumber(w) => tlf!("“{word}” is not a frame offset", word = w),
        tooncraft_doc::PickError::OutOfRange(n) => {
            tlf!("Offset {n} is out of range: use −{max} to +{max}, not 0", n = n, max = tooncraft_doc::ONION_MAX_PICK_OFFSET)
        }
        tooncraft_doc::PickError::TooMany => tlf!("At most {n} offsets", n = tooncraft_doc::ONION_MAX_PICKED),
    }
}

/// Shift-and-trace fields for one light-table pin: across, down and a turn, plus a reset.
/// Returns the new shift when it changed.
fn shift_row(ui: &mut egui::Ui, i: usize, cur: PinShift, pin: &str) -> Option<PinShift> {
    let mut s = cur;
    let mut deg = s.turn as f32 / 10.0;
    ui.push_id(("pin-shift", i), |ui| {
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            let l = ui.label(tl!("Across"));
            ui.add(egui::DragValue::new(&mut s.x).range(-MAX_PIN_SHIFT..=MAX_PIN_SHIFT).suffix(" px"))
                .labelled_by(l.id)
                .on_hover_text(tlf!("Shift and trace: move {pin} across; the drawing does not change", pin = pin));
            let l = ui.label(tl!("Down"));
            ui.add(egui::DragValue::new(&mut s.y).range(-MAX_PIN_SHIFT..=MAX_PIN_SHIFT).suffix(" px"))
                .labelled_by(l.id)
                .on_hover_text(tlf!("Shift and trace: move {pin} down; the drawing does not change", pin = pin));
            let l = ui.label(tl!("Turn"));
            ui.add(egui::DragValue::new(&mut deg).range(-360.0..=360.0).speed(0.5).suffix("°"))
                .labelled_by(l.id)
                .on_hover_text(tlf!("Shift and trace: turn {pin}; the drawing does not change", pin = pin));
            if !cur.is_none() && ui.small_button(tl!("Reset Shift")).on_hover_text(tlf!("Put {pin} back where it was drawn", pin = pin)).clicked() {
                s = PinShift::default();
                deg = 0.0;
            }
        });
    });
    s.turn = (deg * 10.0).round() as i32;
    (s.sanitized() != cur).then_some(s)
}

/// The Camera panel: keys for pan, zoom and rotation, typed (keyboard) rather than dragged.
pub fn camera(app: &mut ToonApp, ui: &mut egui::Ui) {
    let (w, h) = (app.engine.project.width(), app.engine.project.height());
    let frame = app.frame;
    let mut enabled = app.engine.project.camera.enabled;
    if ui.checkbox(&mut enabled, tl!("Export through the camera")).changed() {
        app.run(Command::SetCamera { enabled });
    }
    ui.checkbox(&mut app.show_camera, tl!("Show the camera frame"));
    ui.separator();
    let key_here = app.engine.project.camera.keys.iter().any(|k| k.frame == frame);
    ui.label(if key_here { tlf!("Key at frame {n}", n = frame + 1) } else { tlf!("Frame {n} (between keys)", n = frame + 1) });
    let mut k = app.engine.project.camera.at(frame, w, h);
    let mut changed = false;
    egui::Grid::new("camera-grid").num_columns(2).spacing(vec2(8.0, 5.0)).show(ui, |ui| {
        let l = ui.label(tl!("Centre X"));
        changed |= ui.add(egui::DragValue::new(&mut k.x).speed(1.0).max_decimals(1)).labelled_by(l.id).changed();
        ui.end_row();
        let l = ui.label(tl!("Centre Y"));
        changed |= ui.add(egui::DragValue::new(&mut k.y).speed(1.0).max_decimals(1)).labelled_by(l.id).changed();
        ui.end_row();
        let l = ui.label(tl!("Zoom"));
        changed |= ui.add(egui::DragValue::new(&mut k.zoom).speed(0.01).range(0.05..=50.0).max_decimals(2)).labelled_by(l.id).changed();
        ui.end_row();
        let l = ui.label(tl!("Rotation"));
        changed |= ui.add(egui::DragValue::new(&mut k.angle).speed(0.5).range(-3600.0..=3600.0).suffix("°").max_decimals(1)).labelled_by(l.id).changed();
        ui.end_row();
    });
    if changed {
        app.run(Command::SetCameraKey { frame, x: k.x, y: k.y, zoom: k.zoom, angle: k.angle });
    }
    ui.horizontal(|ui| {
        if ui.button(tl!("Set Key")).clicked() {
            app.run(Command::SetCameraKey { frame, x: k.x, y: k.y, zoom: k.zoom, angle: k.angle });
        }
        if ui.add_enabled(key_here, egui::Button::new(tl!("Remove Key"))).clicked() {
            app.run(Command::RemoveCameraKey { frame });
        }
    });
    crate::pegs::shakes(app, ui);
    ui.separator();
    let keys: Vec<u32> = app.engine.project.camera.keys.iter().map(|k| k.frame).collect();
    egui::ScrollArea::vertical().id_salt("camera-keys").show(ui, |ui| {
        for f in keys {
            if ui.selectable_label(f == frame, tlf!("Key at frame {n}", n = f + 1)).clicked() {
                app.frame = f;
            }
        }
    });
}
