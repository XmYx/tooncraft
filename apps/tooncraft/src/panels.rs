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
            let label = format!("{} ({:?})", t.label(), t.key());
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, label.clone()));
            if resp.on_hover_text(&label).clicked() {
                app.tool = t;
            }
            if t == Tool::Eraser || t == Tool::Picker {
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
            }
            Tool::Picker => {
                ui.label(tl!("Click the canvas to pick a colour (Alt+click with any brush)"));
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
            row(ui, tl!("Pressure → size").into(), &mut b.dynamics.pressure_size, 0.0..=1.0);
            row(ui, tl!("Pressure → opacity").into(), &mut b.dynamics.pressure_opacity, 0.0..=1.0);
            row(ui, tl!("Smallest size").into(), &mut b.dynamics.min_size, 0.01..=1.0);
            row(ui, tl!("Tilt → shape").into(), &mut b.dynamics.tilt, 0.0..=1.0);
            row(ui, tl!("Speed → size").into(), &mut b.dynamics.velocity_size, -1.0..=1.0);
            row(ui, tl!("Smoothing").into(), &mut b.smoothing.average, 0.0..=1.0);
            row(ui, tl!("Steady line (px)").into(), &mut b.smoothing.string, 0.0..=120.0);
        });
        let Smoothing { catch_up, .. } = &mut b.smoothing;
        ui.checkbox(catch_up, tl!("Finish the line where the pen lifts"));
        if ui.button(tl!("Reset Brush")).clicked() {
            *app.brush_mut(kind) = tooncraft_brush::Brush::preset(kind);
        }
        // preview stroke
        ui.separator();
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width().min(260.0), 54.0), Sense::hover());
        preview(app, ui, rect, kind);
    });
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
                } else if ui.selectable_label(li == app.layer, &layer.name).clicked() {
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
    let unused = app.engine.project.unused_drawings().len();
    ui.label(tlf!("{drawings} drawings ({unused} not exposed)", drawings = app.engine.project.drawing_count(), unused = unused));
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
