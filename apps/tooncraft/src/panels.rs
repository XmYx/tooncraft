//! Side panels and bars: tool bar, tool options, colour, brush, layers, onion skin and light
//! table. Every value set by dragging (sliders, the colour field) also has a typed or keyboard
//! alternative: sliders and drag values take arrow keys and typing, colours take hex and RGB.

use egui::{Color32, Sense, Stroke, vec2};
use tooncraft_brush::{BrushKind, Smoothing};
use tooncraft_doc::{OnionMode, Pin};
use tooncraft_engine::{Command, FillReference};

use crate::app::{Tool, ToonApp};

/// The tool bar's groups: variants of one kind of tool share a button, which shows the variant
/// used last and a corner mark; right-click (or press and hold) opens the group to pick another.
pub const TOOL_GROUPS: [&[Tool]; 8] = [
    &[Tool::Pencil, Tool::Ink, Tool::Airbrush, Tool::Chalk, Tool::Watercolour],
    &[Tool::Eraser],
    &[Tool::Smudge],
    &[Tool::Fill, Tool::Gradient],
    &[Tool::Picker],
    &[Tool::Select],
    &[Tool::Shape],
    &[Tool::Hand],
];

/// The variant a group's button shows: the current tool when it is in the group, or the one used
/// last there.
pub fn group_tool(app: &ToonApp, ctx: &egui::Context, group: usize) -> Tool {
    let tools = TOOL_GROUPS[group];
    if tools.contains(&app.tool) {
        return app.tool;
    }
    let id = egui::Id::unique("tooncraft-tool-group-last").with(group);
    ctx.data(|d| d.get_temp::<usize>(id)).and_then(|i| tools.get(i).copied()).unwrap_or(tools[0])
}

fn tool_button(app: &mut ToonApp, ui: &mut egui::Ui, t: Tool, grouped: bool) -> egui::Response {
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
    let fg = if selected { pal.text } else { pal.weak };
    crate::icons::tool(ui.painter(), rect, t, fg);
    if grouped {
        // the corner mark: this button holds more tools
        let c = rect.right_bottom() - vec2(3.0, 3.0);
        ui.painter().add(egui::Shape::convex_polygon(vec![c, c - vec2(6.0, 0.0), c - vec2(0.0, 6.0)], fg, Stroke::NONE));
    }
    if resp.has_focus() {
        ui.painter().rect_stroke(rect, 4.0, ui.visuals().selection.stroke, egui::StrokeKind::Outside);
    }
    let label = format!("{} ({})", t.label(), app.keys.text(&format!("tool.{}", t.id())));
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, label.clone()));
    let tip = if grouped { tlf!("{tool}; right-click or hold for the other tools of the group", tool = label) } else { label };
    resp.on_hover_text(tip)
}

/// The vertical tool bar.
pub fn toolbar(app: &mut ToonApp, ui: &mut egui::Ui) {
    ui.add_space(4.0);
    ui.vertical_centered(|ui| {
        let ctx = ui.ctx().clone();
        for (g, tools) in TOOL_GROUPS.iter().enumerate() {
            let t = group_tool(app, &ctx, g);
            let grouped = tools.len() > 1;
            let resp = tool_button(app, ui, t, grouped);
            if resp.clicked() {
                app.tool = t;
            }
            if grouped {
                let held = resp.is_pointer_button_down_on() && ui.input(|i| i.pointer.press_start_time().is_some_and(|s| i.time - s > 0.45));
                let open = (resp.secondary_clicked() || held).then_some(egui::SetOpenCommand::Bool(true));
                egui::Popup::from_response(&resp)
                    .kind(egui::PopupKind::Menu)
                    .layout(egui::Layout::top_down_justified(egui::Align::Min))
                    .open_memory(open)
                    .align(egui::RectAlign::RIGHT_START)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
                    .show(|ui| {
                        for (i, v) in tools.iter().enumerate() {
                            let key = app.keys.text(&format!("tool.{}", v.id()));
                            if ui.selectable_label(app.tool == *v, format!("{}   {key}", v.label())).clicked() {
                                app.tool = *v;
                                ctx.data_mut(|d| d.insert_temp(egui::Id::unique("tooncraft-tool-group-last").with(g), i));
                            }
                        }
                    });
            }
            if let Some(i) = tools.iter().position(|v| *v == app.tool) {
                ctx.data_mut(|d| d.insert_temp(egui::Id::unique("tooncraft-tool-group-last").with(g), i));
            }
            if matches!(g, 2 | 5) {
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
    // wraps onto a second row when a tool has more options than fit
    ui.horizontal_wrapped(|ui| {
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
            ui.separator();
            crate::brush_presets::picker(app, ui, kind);
            ui.separator();
            crate::draw_aids::brush_options(app, ui);
        }
        match app.tool {
            Tool::Fill => {
                use tooncraft_raster::fill::{FillMode, GAP_PRESETS};
                for (m, name, tip) in [
                    (FillMode::Any, tl!("Fill"), tl!("Fill the area under the click")),
                    (FillMode::EmptyOnly, tl!("Empty Only"), tl!("Fill only empty areas; a click on paint does nothing")),
                    (FillMode::Repaint, tl!("Repaint"), tl!("Give an area already painted on this layer a new colour")),
                ] {
                    ui.selectable_value(&mut app.fill.mode, m, name).on_hover_text(tip);
                }
                ui.separator();
                ui.label(tl!("Gaps"));
                for (g, name) in GAP_PRESETS.iter().zip([tl!("Off"), tl!("Small"), tl!("Medium"), tl!("Large")]) {
                    let resp = ui.selectable_label(app.fill.gap == *g, name).on_hover_text(tlf!("Close gaps up to about {n} px", n = *g as u32 * 2));
                    if resp.clicked() {
                        app.fill.gap = *g;
                    }
                }
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
                ui.checkbox(&mut app.aids.lasso_fill, tl!("Lasso fill")).on_hover_text(tl!("Drag around an area to fill it"));
            }
            Tool::Gradient => crate::draw_aids::gradient_options(app, ui),
            Tool::Picker => {
                ui.label(tl!("Click the canvas to pick a colour (Alt+click with any brush)"));
            }
            Tool::Shape => {
                crate::draw_aids::shape_options(app, ui);
                ui.separator();
                let b = app.brush_mut(BrushKind::Ink);
                let l = ui.label(tl!("Size"));
                ui.add(egui::Slider::new(&mut b.size, 0.5..=200.0).logarithmic(true).suffix(" px")).labelled_by(l.id);
                ui.label(tl!("Shift: 15° lines, squares, circles"));
            }
            Tool::Select => {
                ui.selectable_value(&mut app.select_mode, crate::app::SelectMode::Lasso, tl!("Lasso"));
                ui.selectable_value(&mut app.select_mode, crate::app::SelectMode::Rectangle, tl!("Rectangle"));
                ui.selectable_value(&mut app.select_mode, crate::app::SelectMode::Ellipse, tl!("Ellipse"));
                ui.selectable_value(&mut app.select_mode, crate::app::SelectMode::Wand, tl!("Magic Wand"))
                    .on_hover_text(tl!("Click an area to select the pixels like it"));
                ui.separator();
                crate::selection_tools::options(app, ui);
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
    let n = app.engine.project.layers.len();
    ui.horizontal(|ui| {
        if ui.button(tl!("Add")).on_hover_text(tl!("New layer above the current one")).clicked() {
            let name = tlf!("Layer {n}", n = n + 1);
            if app.run(Command::AddLayer { name, index: Some(app.layer + 1) }) {
                app.layer += 1;
            }
        }
        // the kinds of layer: a drawing layer (Add), or a reference image or a sound
        ui.menu_button(tl!("Add Other"), |ui| {
            if ui.button(tl!("Drawing Layer")).clicked() {
                let name = tlf!("Layer {n}", n = n + 1);
                if app.run(Command::AddLayer { name, index: Some(app.layer + 1) }) {
                    app.layer += 1;
                }
                ui.close();
            }
            if ui.button(tl!("Reference Image…")).on_hover_text(tl!("An image under the drawing, locked and faded, shown for the whole shot")).clicked() {
                app.import_reference();
                ui.close();
            }
            if ui.button(tl!("Sound…")).clicked() {
                app.import_sound();
                ui.close();
            }
        });
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
    crate::layer_ops::buttons(app, ui);
    ui.separator();
    let mut cmds = Vec::new();
    let mut select = None;
    let mut menus: Vec<(usize, egui::Response)> = Vec::new();
    egui::ScrollArea::vertical().id_salt("layers-scroll").show(ui, |ui| {
        for li in (0..n).rev() {
            let Some(layer) = app.engine.project.layers.get(li) else { continue };
            if !crate::layer_tree::listed(app, li) {
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
                } else {
                    let resp = ui.selectable_label(li == app.layer, crate::layer_tree::path_name(&app.engine.project, layer));
                    if resp.clicked() {
                        select = Some(li);
                    }
                    if resp.secondary_clicked() || resp.context_menu_opened() {
                        menus.push((li, resp));
                    }
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
    for (li, resp) in menus {
        resp.context_menu(|ui| crate::context_menus::layer(app, ui, li));
    }
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
