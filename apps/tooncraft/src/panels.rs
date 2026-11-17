//! Side panels and bars: tool bar, tool options, colour, brush, layers, onion skin and light
//! table. Every value set by dragging (sliders, the colour field) also has a typed or keyboard
//! alternative: sliders and drag values take arrow keys and typing, colours take hex and RGB.

use egui::{Color32, Rect, Sense, Stroke, vec2};
use tooncraft_brush::{BrushKind, Smoothing};
use tooncraft_doc::{OnionMode, Pin};
use tooncraft_engine::{Command, FillReference};

use crate::app::{Tool, ToonApp};

/// The vertical tool bar.
pub fn toolbar(app: &mut ToonApp, ui: &mut egui::Ui) {
    ui.add_space(4.0);
    ui.vertical_centered(|ui| {
        for t in Tool::ALL {
            let selected = app.tool == t;
            let (rect, resp) = ui.allocate_exact_size(vec2(34.0, 32.0), Sense::click());
            let v = ui.style().interact_selectable(&resp, selected);
            let pal = crate::theme::palette(app.light_theme);
            let bg = if selected {
                pal.accent.gamma_multiply(0.35)
            } else if resp.hovered() {
                v.bg_fill
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect(rect, 4.0, bg, if selected { Stroke::new(1.0, pal.accent) } else { Stroke::NONE }, egui::StrokeKind::Inside);
            crate::icons::tool(ui.painter(), rect, t, if selected { pal.text } else { pal.weak });
            if resp.has_focus() {
                ui.painter().rect_stroke(rect, 4.0, ui.visuals().selection.stroke, egui::StrokeKind::Outside);
            }
            let label = format!("{} ({})", t.label(), app.keys.text(&format!("tool.{}", t.id())));
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, label.clone()));
            if resp.on_hover_text(&label).clicked() {
                app.tool = t;
            }
            if t == Tool::Eraser || t == Tool::Select {
                ui.add_space(6.0);
            }
        }
        ui.add_space(10.0);
        // current colour
        let c = app.color;
        let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
        ui.painter().rect(rect, 4.0, Color32::from_rgb(c[0], c[1], c[2]), Stroke::new(1.0, Color32::from_gray(140)), egui::StrokeKind::Inside);
        let label = tlf!("Colour #{hex}", hex = hex(c));
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label.clone()));
        if resp.on_hover_text(tl!("Current colour: open the Colour panel")).clicked() {
            app.workspace.raise(crate::dock::Pane::Colour);
        }
    });
}

fn hex(c: [u8; 4]) -> String {
    format!("{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

fn slider(ui: &mut egui::Ui, label: &str, v: &mut f32, range: std::ops::RangeInclusive<f32>) -> bool {
    let l = ui.label(label);
    ui.add(egui::Slider::new(v, range).max_decimals(2)).labelled_by(l.id).changed()
}

/// The tool options bar under the menu.
pub fn tool_options(app: &mut ToonApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.strong(app.tool.label());
        ui.separator();
        if let Some(kind) = app.tool.brush() {
            let b = app.brush_mut(kind);
            let l = ui.label(tl!("Size"));
            ui.add(egui::Slider::new(&mut b.size, 0.5..=tooncraft_brush::MAX_SIZE).logarithmic(true).max_decimals(1).suffix(" px")).labelled_by(l.id);
            slider(ui, tl!("Opacity"), &mut b.opacity, 0.0..=1.0);
            slider(ui, tl!("Smoothing"), &mut b.smoothing.average, 0.0..=1.0);
            let l = ui.label(tl!("Steady line"));
            ui.add(egui::Slider::new(&mut b.smoothing.string, 0.0..=120.0).max_decimals(0).suffix(" px"))
                .labelled_by(l.id)
                .on_hover_text(tl!("The brush follows the pen only when it is further than this: very steady lines"));
        }
        match app.tool {
            Tool::Fill => {
                let l = ui.label(tl!("Tolerance"));
                ui.add(egui::Slider::new(&mut app.fill.tolerance, 0..=255)).labelled_by(l.id);
                let l = ui.label(tl!("Close gaps"));
                ui.add(egui::Slider::new(&mut app.fill.gap, 0..=tooncraft_raster::fill::MAX_GAP).suffix(" px")).labelled_by(l.id);
                let l = ui.label(tl!("Spread under lines"));
                ui.add(egui::Slider::new(&mut app.fill.spread, 0..=tooncraft_raster::fill::MAX_SPREAD).suffix(" px")).labelled_by(l.id);
                let l = ui.label(tl!("Look at"));
                let text = |r: FillReference| match r {
                    FillReference::Layer => tl!("This layer"),
                    FillReference::AllLayers => tl!("All visible layers"),
                };
                egui::ComboBox::from_id_salt("fill-ref")
                    .selected_text(text(app.fill_reference))
                    .show_ui(ui, |ui| {
                        for r in [FillReference::AllLayers, FillReference::Layer] {
                            ui.selectable_value(&mut app.fill_reference, r, text(r));
                        }
                    })
                    .response
                    .labelled_by(l.id);
                ui.checkbox(&mut app.fill_every_frame, tl!("Every drawing of the layer"));
            }
            Tool::Picker => {
                ui.label(tl!("Click the canvas to pick a colour (Alt+click with any brush)"));
            }
            Tool::Shape => {
                use tooncraft_engine::shapes::ShapeKind;
                ui.selectable_value(&mut app.shape, ShapeKind::Line, tl!("Line"));
                ui.selectable_value(&mut app.shape, ShapeKind::Rectangle, tl!("Rectangle"));
                ui.selectable_value(&mut app.shape, ShapeKind::Ellipse, tl!("Ellipse"));
                ui.separator();
                let b = app.brush_mut(BrushKind::Ink);
                let l = ui.label(tl!("Size"));
                ui.add(egui::Slider::new(&mut b.size, 0.5..=200.0).logarithmic(true).suffix(" px")).labelled_by(l.id);
                ui.label(tl!("Shift: 15° lines, squares, circles"));
            }
            Tool::Select => {
                ui.selectable_value(&mut app.select_mode, crate::app::SelectMode::Lasso, tl!("Lasso"));
                ui.selectable_value(&mut app.select_mode, crate::app::SelectMode::Rectangle, tl!("Rectangle"));
                ui.separator();
                let has = app.selection.is_some();
                if ui.add_enabled(has, egui::Button::new(tl!("Flip H"))).on_hover_text(tl!("Flip horizontally")).clicked() {
                    app.transform_selection(0.0, 0.0, (-1.0, 1.0), 0.0);
                }
                if ui.add_enabled(has, egui::Button::new(tl!("Flip V"))).on_hover_text(tl!("Flip vertically")).clicked() {
                    app.transform_selection(0.0, 0.0, (1.0, -1.0), 0.0);
                }
                if ui.add_enabled(has, egui::Button::new(tl!("Rotate 90°"))).clicked() {
                    app.transform_selection(0.0, 0.0, (1.0, 1.0), std::f32::consts::FRAC_PI_2);
                }
                if ui.add_enabled(has, egui::Button::new(tl!("Deselect"))).clicked() {
                    app.selection = None;
                }
                ui.label(tl!("Drag inside to move, corners to scale, the top knob to rotate"));
            }
            Tool::Hand => {
                ui.label(tl!("Drag to pan; wheel zooms, Alt+wheel rotates"));
            }
            _ => {
                ui.separator();
                let l = ui.label(tl!("Mouse pressure"));
                ui.add(egui::Slider::new(&mut app.mouse_pressure, 0.05..=1.0).max_decimals(2))
                    .labelled_by(l.id)
                    .on_hover_text(tl!("Pressure used when drawing with a mouse"));
            }
        }
    });
}

/// The Colour panel.
pub fn colour(app: &mut ToonApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().id_salt("colour-scroll").show(ui, |ui| {
        let mut rgb = [app.color[0], app.color[1], app.color[2]];
        if crate::color::picker(ui, "drawing-colour", &mut rgb) {
            app.color = [rgb[0], rgb[1], rgb[2], 255];
        }
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
                app.color = c;
            }
        });
        ui.separator();
        ui.label(tl!("Swatches"));
        let mut chosen = None;
        ui.horizontal_wrapped(|ui| {
            for s in &app.swatches {
                let (rect, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                ui.painter().rect(rect, 3.0, Color32::from_rgb(s[0], s[1], s[2]), Stroke::new(1.0, Color32::from_gray(120)), egui::StrokeKind::Inside);
                if resp.has_focus() {
                    ui.painter().rect_stroke(rect, 3.0, ui.visuals().selection.stroke, egui::StrokeKind::Outside);
                }
                let label = tlf!("Swatch #{hex}", hex = hex([s[0], s[1], s[2], 255]));
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label.clone()));
                if resp.clicked() {
                    chosen = Some(*s);
                }
            }
        });
        if let Some(s) = chosen {
            app.color = [s[0], s[1], s[2], 255];
        }
        if ui.button(tl!("Add Current Colour")).clicked() && app.swatches.len() < 64 {
            app.swatches.push([app.color[0], app.color[1], app.color[2]]);
        }
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
            row(ui, tl!("Size from pressure").into(), &mut b.dynamics.pressure_size, 0.0..=1.0);
            row(ui, tl!("Opacity from pressure").into(), &mut b.dynamics.pressure_opacity, 0.0..=1.0);
            row(ui, tl!("Smallest size").into(), &mut b.dynamics.min_size, 0.01..=1.0);
            row(ui, tl!("Shape from tilt").into(), &mut b.dynamics.tilt, 0.0..=1.0);
            row(ui, tl!("Size from speed").into(), &mut b.dynamics.velocity_size, -1.0..=1.0);
            row(ui, tl!("Smoothing").into(), &mut b.smoothing.average, 0.0..=1.0);
            row(ui, tl!("Steady line (px)").into(), &mut b.smoothing.string, 0.0..=120.0);
        });
        let Smoothing { catch_up, .. } = &mut b.smoothing;
        ui.checkbox(catch_up, tl!("Finish the line where the pen lifts"));
        if ui.button(tl!("Reset Brush")).clicked() {
            *app.brush_mut(kind) = tooncraft_brush::Brush::preset(kind);
        }
        ui.separator();
        tips_and_presets(app, ui, kind);
        // preview stroke
        ui.separator();
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width().min(260.0), 54.0), Sense::hover());
        preview(app, ui, rect, kind);
    });
}

fn next_tip_id(app: &ToonApp) -> u64 {
    app.engine.project.tips.keys().next_back().map(|k| k + 1).unwrap_or(1)
}

/// Folder of the user's brush presets.
pub fn preset_dir() -> Option<std::path::PathBuf> {
    crate::keys::overrides_path().and_then(|p| p.parent().map(|d| d.join("brushes")))
}

fn use_preset(app: &mut ToonApp, path: &std::path::Path) {
    match tooncraft_engine::preset::load(path) {
        Ok((name, mut brush, tip)) => {
            if let Some(t) = tip {
                let id = next_tip_id(app);
                if app.run(tooncraft_engine::Command::AddTip { id, name: t.name, width: t.width, height: t.height, alpha: t.alpha }) {
                    brush.tip = Some(id);
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
        Err(e) => app.set_status(e),
    }
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
                match tooncraft_engine::preset::save(&f, &n, &brush, tip.as_ref()) {
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

fn preview(app: &ToonApp, ui: &egui::Ui, rect: Rect, kind: BrushKind) {
    let (w, h) = (rect.width().max(8.0) as u32, rect.height().max(8.0) as u32);
    let Ok(mut s) = tooncraft_raster::Surface::new(w, h) else { return };
    let brush = tooncraft_brush::Brush {
        color: if kind == BrushKind::Eraser { [28, 26, 40, 255] } else { app.color },
        kind: if kind == BrushKind::Eraser { BrushKind::Ink } else { kind },
        ..app.brushes[BrushKind::ALL.iter().position(|k| *k == kind).unwrap_or(0)]
    };
    let samples: Vec<tooncraft_brush::Sample> = (0..60)
        .map(|i| {
            let t = i as f32 / 59.0;
            tooncraft_brush::Sample::at(
                8.0 + t * (w as f32 - 16.0),
                h as f32 * 0.5 + (t * std::f32::consts::TAU).sin() * h as f32 * 0.25,
                (t * std::f32::consts::PI).sin().max(0.05),
                i as f64 * 0.01,
            )
        })
        .collect();
    tooncraft_brush::paint(&mut s, brush, &samples);
    let img = s.to_image();
    let tex =
        ui.ctx().load_texture("brush-preview", egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], &img.data), egui::TextureOptions::LINEAR);
    ui.painter().rect_filled(rect, 3.0, Color32::from_rgb(246, 243, 236));
    ui.painter().image(tex.id(), rect, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
}

/// The Layers panel (top layer first).
pub fn layers(app: &mut ToonApp, ui: &mut egui::Ui) {
    let n = app.engine.project.layers.len();
    ui.horizontal(|ui| {
        if ui.button(tl!("Add")).on_hover_text(tl!("New layer above the current one")).clicked() {
            let name = tlf!("Layer {n}", n = n + 1);
            if app.run(Command::AddLayer { name, index: Some(app.layer + 1) }) {
                app.layer += 1;
            }
        }
        if ui.button(tl!("Delete")).clicked() {
            let layer = app.layer;
            app.run(Command::DeleteLayer { layer });
            app.clamp_selection();
        }
        if ui.button(tl!("Up")).on_hover_text(tl!("Move the layer up")).clicked() && app.layer + 1 < n {
            let from = app.layer;
            if app.run(Command::MoveLayer { from, to: from + 1 }) {
                app.layer += 1;
            }
        }
        if ui.button(tl!("Down")).on_hover_text(tl!("Move the layer down")).clicked() && app.layer > 0 {
            let from = app.layer;
            if app.run(Command::MoveLayer { from, to: from - 1 }) {
                app.layer -= 1;
            }
        }
        if ui.button(tl!("Rename")).clicked() {
            let name = app.engine.project.layers.get(app.layer).map(|l| l.name.clone()).unwrap_or_default();
            app.renaming = Some((app.layer, name));
        }
    });
    ui.separator();
    let mut cmds = Vec::new();
    let mut select = None;
    egui::ScrollArea::vertical().id_salt("layers-scroll").show(ui, |ui| {
        for li in (0..n).rev() {
            let Some(layer) = app.engine.project.layers.get(li) else { continue };
            if app.engine.project.folder_of(layer).is_some_and(|f| f.collapsed) && li != app.layer {
                continue;
            }
            let (mut visible, mut locked, mut opacity) = (layer.visible, layer.locked, layer.opacity);
            ui.horizontal(|ui| {
                if ui.checkbox(&mut visible, tl!("Show")).changed() {
                    cmds.push(Command::SetLayer { layer: li, visible: Some(visible), locked: None, opacity: None });
                }
                if ui.checkbox(&mut locked, tl!("Lock")).changed() {
                    cmds.push(Command::SetLayer { layer: li, visible: None, locked: Some(locked), opacity: None });
                }
                if let Some((r, name)) = &mut app.renaming
                    && *r == li
                {
                    let resp = ui.add(egui::TextEdit::singleline(name).desired_width(110.0));
                    resp.request_focus();
                    if resp.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        cmds.push(Command::RenameLayer { layer: li, name: name.clone() });
                        app.renaming = None;
                    }
                } else if ui
                    .selectable_label(
                        li == app.layer,
                        match &layer.folder {
                            Some(f) => format!("{f} › {}", layer.name),
                            None => layer.name.clone(),
                        },
                    )
                    .clicked()
                {
                    select = Some(li);
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
    ui.separator();
    layer_properties(app, ui);
    ui.separator();
    let unused = app.engine.project.unused_drawings().len();
    ui.label(tlf!("{drawings} drawings ({unused} not exposed)", drawings = app.engine.project.drawing_count(), unused = unused));
}

fn blend_name(b: tooncraft_doc::BlendMode) -> String {
    use tooncraft_doc::BlendMode::*;
    match b {
        Normal => tl!("Normal"),
        Multiply => tl!("Multiply"),
        Screen => tl!("Screen"),
        Overlay => tl!("Overlay"),
        Darken => tl!("Darken"),
        Lighten => tl!("Lighten"),
        Add => tl!("Add"),
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
    let folders = app.engine.project.folders.clone();
    for f in folders {
        ui.horizontal(|ui| {
            let mut vis = f.visible;
            if ui.checkbox(&mut vis, tlf!("Folder {name}", name = f.name)).changed() {
                app.run(Command::SetFolder { name: f.name.clone(), visible: Some(vis), opacity: None, collapsed: None });
            }
            let mut col = f.collapsed;
            if ui.checkbox(&mut col, tl!("Collapsed")).changed() {
                app.run(Command::SetFolder { name: f.name.clone(), visible: None, opacity: None, collapsed: Some(col) });
            }
            let mut o = f.opacity;
            let l = ui.label(tl!("Opacity"));
            if ui.add(egui::DragValue::new(&mut o).range(0.0..=1.0).speed(0.01).max_decimals(2)).labelled_by(l.id).changed() {
                app.run(Command::SetFolder { name: f.name.clone(), visible: None, opacity: Some(o), collapsed: None });
            }
        });
    }
}

/// The Layer menu (menu bar).
pub fn layer_menu(app: &mut ToonApp, ui: &mut egui::Ui) {
    let n = app.engine.project.layers.len();
    let layer = app.layer;
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
}

/// The Onion Skin panel, with the light table.
pub fn onion(app: &mut ToonApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().id_salt("onion-scroll").show(ui, |ui| {
        let mut o = app.engine.project.onion;
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
            ui.label(tl!("Tint"));
            ui.horizontal(|ui| {
                ui.checkbox(&mut o.tinted, tl!("On"));
                crate::color::swatch_edit(ui, "onion-before", tl!("Tint before"), &mut o.before_tint);
                crate::color::swatch_edit(ui, "onion-after", tl!("Tint after"), &mut o.after_tint);
            });
            ui.end_row();
        });
        ui.checkbox(&mut o.all_layers, tl!("Onion skin on every visible layer"));
        if o != app.engine.project.onion {
            app.engine.project.onion = o.sanitized();
        }
        ui.separator();
        ui.strong(tl!("Light table"));
        let mut lt = app.engine.project.light_table.clone();
        ui.checkbox(&mut lt.enabled, tl!("Show pinned frames"));
        let l = ui.label(tl!("Opacity"));
        ui.add(egui::Slider::new(&mut lt.opacity, 0.0..=1.0)).labelled_by(l.id);
        let (layer, frame) = (app.layer, app.frame);
        let pinned = app.engine.project.layers.get(layer).is_some_and(|l| lt.pins.contains(&Pin { layer: l.id, frame }));
        if ui.button(if pinned { tl!("Unpin This Frame") } else { tl!("Pin This Frame") }).clicked() {
            app.run(Command::TogglePin { layer, frame });
            lt.pins = app.engine.project.light_table.pins.clone();
        }
        let mut remove = None;
        for (i, pin) in lt.pins.iter().enumerate() {
            let name = app.engine.project.layer_index(pin.layer).and_then(|li| app.engine.project.layers.get(li)).map(|l| l.name.clone()).unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label(tlf!("{layer}, frame {frame}", layer = name, frame = pin.frame + 1));
                if ui.small_button(tl!("Remove")).clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            lt.pins.remove(i);
        }
        if lt != app.engine.project.light_table {
            app.engine.project.light_table = lt.sanitized();
        }
    });
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
