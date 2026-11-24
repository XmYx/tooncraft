//! The tool options bar (28 px), under the top bar. It always starts with the tool's glyph and
//! name, a menu of the tool's group, then (for brushes) the preset dropdown that shows each preset
//! as a stroke. Then come the most-used numbers as scrub fields (drag the name or the bar, or type
//! in the box), then the options of that tool. The full brush settings are in the Brush panel.

use egui::{Color32, Sense, Stroke, vec2};

use crate::app::{Tool, ToonApp};

/// The bar's height.
pub const HEIGHT: f32 = 28.0;
/// A scrub field's bar.
const BAR: egui::Vec2 = vec2(72.0, 4.0);

pub fn show(app: &mut ToonApp, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| body(app, ui));
}

/// The Tool Properties panel: the same settings as the bar, one per line, for a
/// side-panel layout. Its widgets have their own ids, so the bar and the panel can both show.
pub fn panel(app: &mut ToonApp, ui: &mut egui::Ui) {
    ui.push_id("tool-properties", |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // one setting per line: a side panel is narrow and tall
            ui.vertical(|ui| body(app, ui));
        });
    });
}

fn body(app: &mut ToonApp, ui: &mut egui::Ui) {
    {
        ui.spacing_mut().item_spacing.x = 6.0;
        tool_menu(app, ui);
        ui.separator();
        if let Some(kind) = app.tool.brush() {
            crate::brush_presets::picker(app, ui, kind);
            ui.separator();
            let b = app.brush_mut(kind);
            scrub(ui, tl!("Size"), &mut b.size, 0.5..=tooncraft_brush::MAX_SIZE, Scale::Log, " px");
            scrub(ui, tl!("Opacity"), &mut b.opacity, 0.0..=1.0, Scale::Percent, " %");
            scrub(ui, tl!("Smoothing"), &mut b.smoothing.average, 0.0..=1.0, Scale::Percent, " %");
            ui.separator();
            crate::draw_aids::brush_options(app, ui);
        }
        match app.tool {
            Tool::Fill => fill_options(app, ui),
            Tool::Gradient => crate::draw_aids::gradient_options(app, ui),
            Tool::Clone => crate::clone_tool::options(app, ui),
            Tool::Text => crate::text_tool::options(app, ui),
            Tool::Picker => {
                ui.label(tl!("Click the canvas to pick a colour (Alt+click with any brush)"));
            }
            Tool::Shape => {
                crate::draw_aids::shape_options(app, ui);
                ui.separator();
                let b = app.brush_mut(tooncraft_brush::BrushKind::Ink);
                scrub(ui, tl!("Size"), &mut b.size, 0.5..=200.0, Scale::Log, " px");
                ui.label(tl!("Shift: 15° lines, squares, circles"));
            }
            Tool::Select => select_options(app, ui),
            Tool::Hand => {
                ui.label(tl!("Drag to pan; wheel zooms, Alt+wheel rotates"));
            }
            Tool::Zoom => {
                ui.label(tl!("Click to zoom in, Alt+click to zoom out, drag left or right to zoom smoothly"));
            }
            Tool::RotateView => {
                ui.label(tl!("Drag around the centre to turn the view; double-click puts it upright"));
            }
            Tool::Transform => crate::peg_transform::options(app, ui),
            _ => {}
        }
    }
}

/// The tool's glyph and name; a click lists the other tools of its group.
fn tool_menu(app: &mut ToonApp, ui: &mut egui::Ui) {
    let pal = crate::theme::palette(app.light_theme);
    let group = crate::tool_rail::active_group(app);
    let variant = group.map(|g| crate::tool_rail::shown(app, g)).unwrap_or(crate::tool_rail::Variant::Tool(app.tool));
    let font = egui::TextStyle::Button.resolve(ui.style());
    let name = ui.painter().layout_no_wrap(variant.label(), font, pal.accent);
    let (r, resp) = ui.allocate_exact_size(vec2(name.size().x + 42.0, 22.0), Sense::click());
    if resp.hovered() || resp.has_focus() {
        ui.painter().rect_filled(r, 4.0, pal.raised);
    }
    variant.paint(ui.painter(), egui::Rect::from_min_size(r.min + vec2(2.0, 2.0), vec2(18.0, 18.0)), pal.accent, true);
    ui.painter().galley(egui::pos2(r.left() + 22.0, r.center().y - name.size().y * 0.5), name, pal.accent);
    caret(ui.painter(), egui::pos2(r.right() - 8.0, r.center().y), pal.weak);
    if resp.has_focus() {
        ui.painter().rect_stroke(r, 4.0, Stroke::new(2.0, pal.playhead), egui::StrokeKind::Outside);
    }
    let label = tlf!("Tool: {tool}", tool = variant.label());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, label.clone()));
    let resp = resp.on_hover_text(tl!("The other tools of this group"));
    let Some(g) = group else { return };
    egui::Popup::menu(&resp).show(|ui| {
        for v in crate::tool_rail::GROUPS[g].variants {
            let key = app.keys.text(&v.key_id());
            if ui.selectable_label(v.is_active(app), format!("{}   {key}", v.label())).clicked() {
                crate::tool_rail::pick(app, g, *v);
                ui.close();
            }
        }
    });
}

/// A small downward caret centred on `at`.
pub fn caret(p: &egui::Painter, at: egui::Pos2, color: Color32) {
    p.add(egui::Shape::convex_polygon(vec![at + vec2(-3.5, -2.0), at + vec2(3.5, -2.0), at + vec2(0.0, 2.5)], color, Stroke::NONE));
}

/// How a scrub field shows its value and moves along its bar.
#[derive(Clone, Copy)]
pub enum Scale {
    /// Even steps (the value as it is).
    Linear,
    /// Steps that grow with the value (sizes).
    Log,
    /// A 0 … 1 value shown as 0 … 100.
    Percent,
}

/// A scrub field: the name (drag it sideways to change the value), a bar (click or drag on it)
/// and a box to type in. The box is the keyboard path (arrow keys and typing); it carries the
/// name for assistive technology. Returns whether the value changed.
pub fn scrub(ui: &mut egui::Ui, name: &str, v: &mut f32, range: std::ops::RangeInclusive<f32>, scale: Scale, suffix: &str) -> bool {
    // one unit, so a wrapping row (the Tool Properties panel) never parts the name from its bar
    ui.horizontal(|ui| scrub_row(ui, name, v, range, scale, suffix)).inner
}

fn scrub_row(ui: &mut egui::Ui, name: &str, v: &mut f32, range: std::ops::RangeInclusive<f32>, scale: Scale, suffix: &str) -> bool {
    let (lo, hi) = (*range.start(), *range.end());
    if !v.is_finite() {
        *v = lo;
    }
    *v = v.clamp(lo, hi);
    let before = *v;
    // the position of a value along the bar, 0 … 1, and back
    let to_t = |x: f32| match scale {
        Scale::Log if lo > 0.0 => ((x.max(lo) / lo).ln() / (hi / lo).ln()).clamp(0.0, 1.0),
        _ => ((x - lo) / (hi - lo).max(f32::EPSILON)).clamp(0.0, 1.0),
    };
    let from_t = |t: f32| match scale {
        Scale::Log if lo > 0.0 => lo * (hi / lo).powf(t.clamp(0.0, 1.0)),
        _ => lo + (hi - lo) * t.clamp(0.0, 1.0),
    };
    let visuals = ui.visuals().clone();
    // the name: a sideways drag moves along the bar
    let label = ui.add(egui::Label::new(name).sense(Sense::drag()));
    if label.hovered() || label.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if label.dragged() {
        let dt = label.drag_delta().x / 200.0;
        *v = from_t(to_t(*v) + dt);
    }
    // the bar
    let (row, bar) = ui.allocate_exact_size(vec2(BAR.x, 18.0), Sense::click_and_drag());
    let track = egui::Rect::from_center_size(row.center(), BAR);
    ui.painter().rect_filled(track, 2.0, visuals.extreme_bg_color);
    let mut filled = track;
    filled.set_right(track.left() + track.width() * to_t(*v));
    ui.painter().rect_filled(filled, 2.0, visuals.selection.stroke.color);
    if (bar.clicked() || bar.dragged())
        && let Some(p) = bar.interact_pointer_pos()
    {
        *v = from_t((p.x - track.left()) / track.width().max(1.0));
    }
    let bar_name = tlf!("{name} bar", name = name);
    bar.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Slider, true, bar_name.clone()));
    // the box
    let (factor, decimals) = match scale {
        Scale::Percent => (100.0, 0),
        Scale::Log | Scale::Linear => (1.0, if hi <= 10.0 { 2 } else { 1 }),
    };
    let mut shown = *v * factor;
    let speed = match scale {
        Scale::Log => f64::from((*v * 0.02).max(0.05)),
        _ => f64::from((hi - lo) * factor / 200.0),
    };
    let field =
        ui.add(egui::DragValue::new(&mut shown).range(lo * factor..=hi * factor).speed(speed).max_decimals(decimals).suffix(suffix)).labelled_by(label.id);
    if field.changed() {
        *v = shown / factor;
    }
    if !v.is_finite() {
        *v = before;
    }
    *v = v.clamp(lo, hi);
    *v != before
}

/// The bucket: mode, gaps, tolerance and what to look at, then the rest of its settings.
fn fill_options(app: &mut ToonApp, ui: &mut egui::Ui) {
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
    let mut tolerance = f32::from(app.fill.tolerance);
    if scrub(ui, tl!("Tolerance"), &mut tolerance, 0.0..=255.0, Scale::Linear, "") {
        app.fill.tolerance = tolerance.round().clamp(0.0, 255.0) as u8;
    }
    crate::layer_fx::reference_picker(ui, "fill-ref", &mut app.fill_reference);
    ui.separator();
    let l = ui.label(tl!("Close gaps"));
    ui.add(egui::DragValue::new(&mut app.fill.gap).range(0..=tooncraft_raster::fill::MAX_GAP).suffix(" px")).labelled_by(l.id);
    let l = ui.label(tl!("Spread under lines"));
    ui.add(egui::DragValue::new(&mut app.fill.spread).range(0..=tooncraft_raster::fill::MAX_SPREAD).suffix(" px")).labelled_by(l.id);
    ui.checkbox(&mut app.fill_every_frame, tl!("Every drawing of the layer"));
    ui.checkbox(&mut app.aids.lasso_fill, tl!("Lasso fill")).on_hover_text(tl!("Drag around an area to fill it"));
}

/// Select: how to select, flip and turn, then how a new area combines and the typed box.
fn select_options(app: &mut ToonApp, ui: &mut egui::Ui) {
    use crate::app::SelectMode;
    if app.rail.transform_only {
        ui.label(tl!("Drag inside to move, a corner to scale, the top knob to rotate"));
    } else {
        ui.selectable_value(&mut app.select_mode, SelectMode::Lasso, tl!("Lasso"));
        ui.selectable_value(&mut app.select_mode, SelectMode::Rectangle, tl!("Rectangle"));
        ui.selectable_value(&mut app.select_mode, SelectMode::Ellipse, tl!("Ellipse"));
        ui.selectable_value(&mut app.select_mode, SelectMode::Wand, tl!("Magic Wand")).on_hover_text(tl!("Click an area to select the pixels like it"));
    }
    ui.separator();
    let has = app.selection.is_some();
    let why = tl!("Select something first");
    if ui.add_enabled(has, egui::Button::new(tl!("Flip H"))).on_hover_text(tl!("Flip horizontally")).on_disabled_hover_text(why).clicked() {
        app.transform_selection(0.0, 0.0, (-1.0, 1.0), 0.0);
    }
    if ui.add_enabled(has, egui::Button::new(tl!("Flip V"))).on_hover_text(tl!("Flip vertically")).on_disabled_hover_text(why).clicked() {
        app.transform_selection(0.0, 0.0, (1.0, -1.0), 0.0);
    }
    if ui.add_enabled(has, egui::Button::new(tl!("Rotate 90°"))).on_disabled_hover_text(why).clicked() {
        app.transform_selection(0.0, 0.0, (1.0, 1.0), std::f32::consts::FRAC_PI_2);
    }
    if ui.add_enabled(has, egui::Button::new(tl!("Deselect"))).on_disabled_hover_text(why).clicked() {
        app.selection = None;
    }
    ui.separator();
    crate::selection_tools::options(app, ui);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrub_fields_keep_hostile_values_in_range() {
        let ctx = egui::Context::default();
        let mut ran = false;
        let mut out = ctx.run_ui(Default::default(), |ui| {
            for start in [f32::NAN, f32::INFINITY, -5.0, 1e9] {
                let mut v = start;
                scrub(ui, "Size", &mut v, 0.5..=500.0, Scale::Log, " px");
                assert!((0.5..=500.0).contains(&v), "{start} → {v}");
                let mut o = start;
                scrub(ui, "Opacity", &mut o, 0.0..=1.0, Scale::Percent, " %");
                assert!((0.0..=1.0).contains(&o), "{start} → {o}");
            }
            ran = true;
        });
        out.textures_delta.clear();
        assert!(ran);
    }
}
