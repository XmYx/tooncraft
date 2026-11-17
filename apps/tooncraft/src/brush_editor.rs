//! The brush editor, shown in the Brush panel under the basic settings: tip shape, a response
//! curve for every pairing of an input (pressure, tilt, speed, direction, chance) and a dab
//! property (size, opacity, flow, scatter, angle), jitter, grain textures, wet mixing, and a live
//! preview stroke.
//!
//! Everything that can be dragged also has a keyboard path: the curve editor takes the arrow
//! keys when focused (Left/Right pick a point, Up/Down move it, Alt+Left/Right slide it sideways,
//! Insert adds a point, Delete removes it) and has typed fields for the selected point.

use egui::{Color32, Id, Modifiers, Pos2, Rect, Sense, Stroke, Ui, WidgetInfo, pos2, vec2};
use tooncraft_brush::{Brush, BrushKind, Curve, GrainKind, Images, MAX_POINTS, Source, Target};

use crate::app::ToonApp;

/// What the dynamics section is editing: one source driving one target, and a point of its curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorState {
    pub target: Target,
    pub source: Source,
    pub point: usize,
}

impl Default for EditorState {
    fn default() -> Self {
        Self { target: Target::Size, source: Source::Pressure, point: 0 }
    }
}

fn state_id() -> Id {
    Id::unique("brush-editor-state")
}

/// The curve editor's widget id (tests and keyboard focus use it).
pub fn curve_id() -> Id {
    Id::unique("brush-editor-curve")
}

pub fn state(ctx: &egui::Context) -> EditorState {
    ctx.data(|d| d.get_temp::<EditorState>(state_id())).unwrap_or_default()
}

fn set_state(ctx: &egui::Context, s: EditorState) {
    ctx.data_mut(|d| d.insert_temp(state_id(), s));
}

pub fn target_label(t: Target) -> String {
    match t {
        Target::Size => tl!("Size"),
        Target::Opacity => tl!("Opacity"),
        Target::Flow => tl!("Flow"),
        Target::Scatter => tl!("Scatter"),
        Target::Angle => tl!("Angle"),
    }
    .to_string()
}

pub fn source_label(s: Source) -> String {
    match s {
        Source::Pressure => tl!("Pressure"),
        Source::Tilt => tl!("Tilt"),
        Source::Speed => tl!("Speed"),
        Source::Rotation => tl!("Direction"),
        Source::Random => tl!("Random"),
    }
    .to_string()
}

fn grain_label(k: GrainKind) -> String {
    match k {
        GrainKind::Paper => tl!("Paper"),
        GrainKind::Canvas => tl!("Canvas weave"),
        GrainKind::Speckle => tl!("Speckle"),
        GrainKind::Clouds => tl!("Clouds"),
        GrainKind::Image => tl!("Image"),
    }
    .to_string()
}

fn slider(ui: &mut Ui, label: &str, v: &mut f32, range: std::ops::RangeInclusive<f32>) -> bool {
    let l = ui.label(label);
    ui.add(egui::Slider::new(v, range).max_decimals(2)).labelled_by(l.id).changed()
}

/// All sections of the editor for the brush of `kind`.
pub fn sections(app: &mut ToonApp, ui: &mut Ui, kind: BrushKind) {
    preview(app, ui, kind);
    egui::CollapsingHeader::new(tl!("Tip shape")).default_open(true).show(ui, |ui| shape(app, ui, kind));
    egui::CollapsingHeader::new(tl!("Dynamics")).default_open(true).show(ui, |ui| dynamics(app, ui, kind));
    egui::CollapsingHeader::new(tl!("Jitter")).default_open(false).show(ui, |ui| jitter(app, ui, kind));
    egui::CollapsingHeader::new(tl!("Grain texture")).default_open(false).show(ui, |ui| grain(app, ui, kind));
    if kind != BrushKind::Eraser {
        egui::CollapsingHeader::new(tl!("Wet mixing")).default_open(false).show(ui, |ui| wet(app, ui, kind));
    }
}

fn shape(app: &mut ToonApp, ui: &mut Ui, kind: BrushKind) {
    let b = app.brush_mut(kind);
    egui::Grid::new("brush-shape-grid").num_columns(2).spacing(vec2(8.0, 5.0)).show(ui, |ui| {
        slider(ui, tl!("Roundness"), &mut b.roundness, 0.1..=1.0);
        ui.end_row();
        let l = ui.label(tl!("Tip angle"));
        ui.add(egui::Slider::new(&mut b.angle, -180.0..=180.0).max_decimals(0).suffix("°")).labelled_by(l.id);
        ui.end_row();
    });
}

fn dynamics(app: &mut ToonApp, ui: &mut Ui, kind: BrushKind) {
    let ctx = ui.ctx().clone();
    let mut st = state(&ctx);
    ui.horizontal_wrapped(|ui| {
        for t in Target::ALL {
            let selected = st.target == t;
            let name = target_label(t);
            let r = ui.selectable_label(selected, &name);
            let label = tlf!("Dynamics for {target}", target = name);
            r.widget_info(|| WidgetInfo::selected(egui::Role::Button, true, selected, label.clone()));
            if r.clicked() {
                st.target = t;
                st.point = 0;
            }
        }
    });
    let b = app.brush_mut(kind);
    egui::Grid::new("brush-dynamics-grid").num_columns(3).spacing(vec2(8.0, 5.0)).show(ui, |ui| {
        for s in Source::ALL {
            let l = ui.label(source_label(s));
            let m = b.dynamics.response.get_mut(st.target, s);
            ui.add(egui::Slider::new(&mut m.amount, -1.0..=1.0).max_decimals(2)).labelled_by(l.id);
            let selected = st.source == s;
            let r = ui.selectable_label(selected, tl!("Curve"));
            let label = tlf!("Edit the curve of {source}", source = source_label(s));
            r.widget_info(|| WidgetInfo::selected(egui::Role::Button, true, selected, label.clone()));
            if r.clicked() {
                st.source = s;
                st.point = 0;
            }
            ui.end_row();
        }
    });
    let name = tlf!("Curve: {source} to {target}", source = source_label(st.source), target = target_label(st.target));
    let curve = &mut b.dynamics.response.get_mut(st.target, st.source).curve;
    curve_editor(ui, curve, &mut st.point, &name);
    egui::Grid::new("brush-dynamics-limits").num_columns(2).spacing(vec2(8.0, 5.0)).show(ui, |ui| {
        slider(ui, tl!("Smallest size"), &mut b.dynamics.min_size, 0.01..=1.0);
        ui.end_row();
        slider(ui, tl!("Shape from tilt"), &mut b.dynamics.tilt, 0.0..=1.0);
        ui.end_row();
    });
    set_state(&ctx, st);
}

/// Quick access to the random mappings: how much chance varies each property of every dab.
fn jitter(app: &mut ToonApp, ui: &mut Ui, kind: BrushKind) {
    let b = app.brush_mut(kind);
    egui::Grid::new("brush-jitter-grid").num_columns(2).spacing(vec2(8.0, 5.0)).show(ui, |ui| {
        for (t, label) in [
            (Target::Size, tl!("Size jitter")),
            (Target::Opacity, tl!("Opacity jitter")),
            (Target::Flow, tl!("Flow jitter")),
            (Target::Scatter, tl!("Position jitter")),
            (Target::Angle, tl!("Angle jitter")),
        ] {
            let l = ui.label(label);
            let m = b.dynamics.response.get_mut(t, Source::Random);
            ui.add(egui::Slider::new(&mut m.amount, 0.0..=1.0).max_decimals(2)).labelled_by(l.id);
            ui.end_row();
        }
    });
}

fn grain(app: &mut ToonApp, ui: &mut Ui, kind: BrushKind) {
    let has_images = !app.engine.project.tips.is_empty();
    let b = app.brush_mut(kind);
    let l = ui.label(tl!("Texture"));
    let mut choice = b.grain_kind;
    egui::ComboBox::from_id_salt("brush-grain-kind")
        .selected_text(grain_label(choice))
        .show_ui(ui, |ui| {
            for k in GrainKind::ALL {
                if k == GrainKind::Image && !has_images {
                    continue;
                }
                ui.selectable_value(&mut choice, k, grain_label(k));
            }
        })
        .response
        .labelled_by(l.id);
    b.grain_kind = choice;
    slider(ui, tl!("Grain strength"), &mut b.grain, 0.0..=1.0);
    slider(ui, tl!("Grain size"), &mut b.grain_scale, 0.5..=16.0);
    if b.grain_kind == GrainKind::Image {
        let current = b.grain_image;
        let tips: Vec<(u64, String)> = app.engine.project.tips.iter().map(|(id, t)| (*id, t.name.clone())).collect();
        let name = |id: Option<u64>| id.and_then(|i| tips.iter().find(|t| t.0 == i)).map(|t| t.1.clone()).unwrap_or_else(|| tl!("None").to_string());
        let l = ui.label(tl!("Grain image"));
        let mut pick = current;
        egui::ComboBox::from_id_salt("brush-grain-image")
            .selected_text(name(current))
            .show_ui(ui, |ui| {
                for (id, n) in &tips {
                    ui.selectable_value(&mut pick, Some(*id), n);
                }
            })
            .response
            .labelled_by(l.id);
        app.brush_mut(kind).grain_image = pick;
    }
    if ui.button(tl!("Grain from Image…")).clicked()
        && let Some(f) = rfd::FileDialog::new().add_filter(tl!("Images"), &["png", "jpg", "jpeg", "webp", "tif", "tiff", "bmp"]).pick_file()
    {
        let id = crate::panels::next_tip_id(app);
        if app.run(tooncraft_engine::Command::AddGrainFromImage { id, path: f.to_string_lossy().into_owned() }) {
            let b = app.brush_mut(kind);
            b.grain_kind = GrainKind::Image;
            b.grain_image = Some(id);
            if b.grain < 0.2 {
                b.grain = 0.6;
            }
        }
    }
}

fn wet(app: &mut ToonApp, ui: &mut Ui, kind: BrushKind) {
    let b = app.brush_mut(kind);
    let (label, hint) = if kind == BrushKind::Smudge {
        (tl!("Smear fade"), tl!("How quickly the smear lets go of the colour it carries: low values drag long smears"))
    } else {
        (tl!("Colour pickup"), tl!("How much the colour already on the layer mixes into the paint; 0 keeps the paint pure"))
    };
    let l = ui.label(label);
    ui.add(egui::Slider::new(&mut b.wet.pickup, 0.0..=1.0).max_decimals(2)).labelled_by(l.id).on_hover_text(hint);
}

// ----- the curve editor -----

fn to_screen(r: Rect, p: [f32; 2]) -> Pos2 {
    pos2(r.left() + p[0] * r.width(), r.bottom() - p[1] * r.height())
}

fn from_screen(r: Rect, p: Pos2) -> [f32; 2] {
    [((p.x - r.left()) / r.width().max(1.0)).clamp(0.0, 1.0), ((r.bottom() - p.y) / r.height().max(1.0)).clamp(0.0, 1.0)]
}

fn nearest(curve: &Curve, r: Rect, p: Pos2, within: f32) -> Option<usize> {
    curve
        .points()
        .iter()
        .enumerate()
        .map(|(i, q)| (i, to_screen(r, *q).distance(p)))
        .filter(|(_, d)| *d <= within)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// A response-curve editor: returns whether the curve changed. `point` is the selected control
/// point.
pub fn curve_editor(ui: &mut Ui, curve: &mut Curve, point: &mut usize, name: &str) -> bool {
    let before = *curve;
    *point = (*point).min(curve.len().saturating_sub(1));
    let side = ui.available_width().clamp(120.0, 220.0);
    let (_, rect) = ui.allocate_space(vec2(side, side));
    let resp = ui.interact(rect, curve_id(), Sense::click_and_drag());
    if resp.clicked() || resp.drag_started() || resp.double_clicked() {
        resp.request_focus();
    }
    // pointer
    if let Some(p) = resp.interact_pointer_pos() {
        if (resp.drag_started() || resp.clicked())
            && let Some(i) = nearest(curve, rect, p, 14.0)
        {
            *point = i;
        }
        if resp.dragged() {
            let q = from_screen(rect, p);
            curve.set_point(*point, q[0], q[1]);
        }
        if resp.double_clicked()
            && let Some(i) = curve.insert(from_screen(rect, p)[0])
        {
            *point = i;
        }
        if resp.secondary_clicked()
            && let Some(i) = nearest(curve, rect, p, 14.0)
            && curve.remove(i)
        {
            *point = i.saturating_sub(1);
        }
    }
    // keyboard
    if resp.has_focus() {
        ui.memory_mut(|m| m.set_focus_lock_filter(resp.id, egui::EventFilter { tab: false, horizontal_arrows: true, vertical_arrows: true, escape: false }));
        let keys = ui.input_mut(|i| {
            use egui::Key::*;
            let mut step = |small: Modifiers, big: Modifiers, k| -> f32 {
                if i.consume_key(big, k) {
                    10.0
                } else if i.consume_key(small, k) {
                    1.0
                } else {
                    0.0
                }
            };
            let up = step(Modifiers::NONE, Modifiers::SHIFT, ArrowUp);
            let down = step(Modifiers::NONE, Modifiers::SHIFT, ArrowDown);
            let slide_l = i.consume_key(Modifiers::ALT, ArrowLeft);
            let slide_r = i.consume_key(Modifiers::ALT, ArrowRight);
            let prev = i.consume_key(Modifiers::NONE, ArrowLeft);
            let next = i.consume_key(Modifiers::NONE, ArrowRight);
            let add = i.consume_key(Modifiers::NONE, Insert);
            let del = i.consume_key(Modifiers::NONE, Delete) | i.consume_key(Modifiers::NONE, Backspace);
            (up - down, slide_r as i32 - slide_l as i32, next as i32 - prev as i32, add, del)
        });
        let n = curve.len();
        if keys.2 != 0 && n > 0 {
            *point = (*point as i32 + keys.2).clamp(0, n as i32 - 1) as usize;
        }
        if let Some(p) = curve.points().get(*point).copied() {
            if keys.0 != 0.0 {
                curve.set_point(*point, p[0], p[1] + keys.0 * 0.02);
            }
            if keys.1 != 0 {
                curve.set_point(*point, p[0] + keys.1 as f32 * 0.02, p[1]);
            }
        }
        if keys.3 {
            // a new point in the middle of the widest gap
            let pts = curve.points().to_vec();
            if let Some((i, w)) = pts.windows(2).enumerate().max_by(|a, b| (a.1[1][0] - a.1[0][0]).total_cmp(&(b.1[1][0] - b.1[0][0])))
                && let Some(at) = curve.insert((w[0][0] + w[1][0]) * 0.5)
            {
                *point = at;
                let _ = i;
            }
        }
        if keys.4 && curve.remove(*point) {
            *point = point.saturating_sub(1);
        }
    }
    *point = (*point).min(curve.len().saturating_sub(1));

    // paint
    let sel = curve.points().get(*point).copied().unwrap_or([0.0, 0.0]);
    let label = tlf!(
        "{name}, point {n} of {count}: input {x}, output {y}",
        name = name,
        n = *point + 1,
        count = curve.len(),
        x = format!("{:.2}", sel[0]),
        y = format!("{:.2}", sel[1])
    );
    resp.widget_info(|| WidgetInfo::slider(true, sel[1] as f64, label.clone()));
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    painter.rect_filled(rect, 3.0, visuals.extreme_bg_color);
    let grid = Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color);
    for k in 1..4 {
        let t = k as f32 / 4.0;
        painter.line_segment([pos2(rect.left() + t * rect.width(), rect.top()), pos2(rect.left() + t * rect.width(), rect.bottom())], grid);
        painter.line_segment([pos2(rect.left(), rect.top() + t * rect.height()), pos2(rect.right(), rect.top() + t * rect.height())], grid);
    }
    let line = Stroke::new(2.0, visuals.selection.bg_fill.gamma_multiply(1.6));
    let pts: Vec<Pos2> = (0..=64).map(|i| to_screen(rect, [i as f32 / 64.0, curve.eval(i as f32 / 64.0)])).collect();
    painter.add(egui::Shape::line(pts, line));
    for (i, p) in curve.points().iter().enumerate() {
        let c = to_screen(rect, *p);
        let selected = i == *point;
        painter.circle(
            c,
            if selected { 6.0 } else { 4.5 },
            if selected { Color32::WHITE } else { visuals.widgets.inactive.fg_stroke.color },
            Stroke::new(1.5, visuals.selection.bg_fill),
        );
    }
    painter.rect_stroke(
        rect,
        3.0,
        if resp.has_focus() { visuals.selection.stroke } else { visuals.widgets.noninteractive.bg_stroke },
        egui::StrokeKind::Inside,
    );

    // typed fields and presets
    ui.horizontal_wrapped(|ui| {
        let mut p = curve.points().get(*point).copied().unwrap_or([0.0, 0.0]);
        let (n, last) = (*point, curve.len().saturating_sub(1));
        let lx = ui.label(tl!("Input"));
        let rx = ui.add_enabled(n != 0 && n != last, egui::DragValue::new(&mut p[0]).range(0.0..=1.0).speed(0.005).max_decimals(2));
        let rx = rx.labelled_by(lx.id);
        let ly = ui.label(tl!("Output"));
        let ry = ui.add(egui::DragValue::new(&mut p[1]).range(0.0..=1.0).speed(0.005).max_decimals(2));
        let ry = ry.labelled_by(ly.id);
        if rx.changed() || ry.changed() {
            curve.set_point(*point, p[0], p[1]);
        }
    });
    ui.horizontal_wrapped(|ui| {
        for (label, c) in [
            (tl!("Linear"), Curve::linear()),
            (tl!("Reverse"), Curve::falling()),
            (tl!("Ease in"), Curve::ease_in()),
            (tl!("Ease out"), Curve::ease_out()),
            (tl!("S-curve"), Curve::s_curve()),
        ] {
            if ui.button(label).clicked() {
                *curve = c;
                *point = 0;
            }
        }
        if ui.add_enabled(curve.len() < MAX_POINTS, egui::Button::new(tl!("Add point"))).clicked() {
            let pts = curve.points().to_vec();
            if let Some(w) = pts.windows(2).max_by(|a, b| (a[1][0] - a[0][0]).total_cmp(&(b[1][0] - b[0][0])))
                && let Some(at) = curve.insert((w[0][0] + w[1][0]) * 0.5)
            {
                *point = at;
            }
        }
        if ui.add_enabled(*point != 0 && *point + 1 < curve.len(), egui::Button::new(tl!("Remove point"))).clicked() && curve.remove(*point) {
            *point = point.saturating_sub(1);
        }
    });
    *curve != before
}

// ----- the live preview -----

fn preview(app: &mut ToonApp, ui: &mut Ui, kind: BrushKind) {
    let brush = Brush { color: app.color, ..*app.brush_mut(kind) };
    let tips = &app.engine.project.tips;
    let images = Images { tip: brush.tip.and_then(|i| tips.get(&i).cloned()), grain: brush.grain_image.and_then(|i| tips.get(&i).cloned()) };
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width().clamp(120.0, 300.0), 84.0), Sense::hover());
    resp.widget_info(|| WidgetInfo::labeled(egui::Role::Image, true, tl!("Brush preview: a stroke drawn with the current settings")));
    let id = Id::unique("brush-preview").with(kind.id());
    let cached: Option<(Brush, u64, egui::TextureHandle)> = ui.ctx().data(|d| d.get_temp(id));
    let revision = tips.len() as u64 ^ ((rect.width() as u64) << 32);
    let tex = match cached {
        Some((b, r, t)) if b == brush && r == revision => t,
        _ => {
            let (w, h) = (rect.width().max(8.0) as u32, rect.height().max(8.0) as u32);
            let Some(img) = tooncraft_engine::preview::render(brush, images, w, h) else { return };
            let t = ui.ctx().load_texture(
                format!("brush-preview-{}", kind.id()),
                egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], &img.data),
                egui::TextureOptions::LINEAR,
            );
            ui.ctx().data_mut(|d| d.insert_temp(id, (brush, revision, t.clone())));
            t
        }
    };
    ui.painter().rect_filled(rect, 3.0, Color32::from_rgb(246, 243, 236));
    ui.painter().image(tex.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
}

#[cfg(test)]
mod tests {
    use egui::{Key, Modifiers};
    use egui_kittest::Harness;
    use egui_kittest::kittest::{By, Queryable};
    use tooncraft_brush::{BrushKind, Source, Target};

    use super::*;
    use crate::app::Tool;
    use crate::args::Cli;
    use crate::dock::Pane;

    const INK: usize = 1;

    fn harness() -> Harness<'static, ToonApp> {
        let ctx = egui::Context::default();
        let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
        let mut h = Harness::builder().with_size(egui::vec2(1500.0, 2600.0)).with_accessibility_check(true).build_ui_state(
            |ui, app: &mut ToonApp| {
                let ctx = ui.ctx().clone();
                app.tick(&ctx);
                app.draw(ui);
            },
            app,
        );
        h.state_mut().tool = Tool::Ink;
        h.state_mut().workspace.raise(Pane::Brush);
        h.run();
        h.run();
        h
    }

    /// Click a control by its label, scrolling it into view of the panel first.
    fn click(h: &mut Harness<'static, ToonApp>, label: &str) {
        h.get_by_label(label).scroll_to_me();
        h.run();
        h.get_by_label(label).click();
        h.run();
    }

    fn focus_curve(h: &mut Harness<'static, ToonApp>) {
        h.ctx.memory_mut(|m| m.request_focus(curve_id()));
        h.run();
    }

    fn pressure_size(h: &Harness<'static, ToonApp>) -> Curve {
        h.state().brushes[INK].dynamics.response.get(Target::Size, Source::Pressure).curve
    }

    #[test]
    fn the_curve_editor_is_named_and_reachable() {
        let h = harness();
        h.get(By::new().role(egui::Role::Slider).label_contains("Curve: Pressure to Size, point 1 of 2"));
        h.get_by_label("Dynamics for Opacity");
        h.get_by_label("Edit the curve of Speed");
        h.get_by_label("Linear");
        h.get_by_label("Add point");
        h.get(By::new().role(egui::Role::Image).label_contains("Brush preview"));
    }

    #[test]
    fn edit_a_curve_with_the_keyboard_only() {
        let mut h = harness();
        assert_eq!(pressure_size(&h), Curve::linear());
        focus_curve(&mut h);
        // the first point (0, 0) goes up; the end points only move vertically
        h.key_press(Key::ArrowUp);
        h.run();
        h.key_press_modifiers(Modifiers::SHIFT, Key::ArrowUp);
        h.run();
        assert!((pressure_size(&h).points()[0][1] - 0.22).abs() < 1e-4);
        assert_eq!(pressure_size(&h).points()[0][0], 0.0);
        h.key_press_modifiers(Modifiers::ALT, Key::ArrowRight);
        h.run();
        assert_eq!(pressure_size(&h).points()[0][0], 0.0);
        // next point, down a little
        h.key_press(Key::ArrowRight);
        h.run();
        h.key_press(Key::ArrowDown);
        h.run();
        assert!((pressure_size(&h).points()[1][1] - 0.98).abs() < 1e-4);
        // Insert adds a point in the widest gap and selects it; Alt+arrows slide it
        h.key_press(Key::Insert);
        h.run();
        assert_eq!(pressure_size(&h).len(), 3);
        let x = pressure_size(&h).points()[1][0];
        assert!((x - 0.5).abs() < 1e-4);
        h.key_press_modifiers(Modifiers::ALT, Key::ArrowRight);
        h.run();
        assert!((pressure_size(&h).points()[1][0] - 0.52).abs() < 1e-4);
        // Delete removes it again
        h.key_press(Key::Delete);
        h.run();
        assert_eq!(pressure_size(&h).len(), 2);
        assert!(h.ctx.memory(|m| m.has_focus(curve_id())), "focus stays on the curve");
        // the label follows the selected point
        h.get(By::new().role(egui::Role::Slider).label_contains("Curve: Pressure to Size, point 1 of 2"));
    }

    #[test]
    fn the_curve_shapes_a_real_stroke() {
        let mut h = harness();
        let width = |h: &mut Harness<'static, ToonApp>| {
            let brush = tooncraft_brush::Brush { size: 30.0, ..h.state().brushes[INK] };
            let mut s = tooncraft_raster::Surface::new(200, 60).unwrap();
            let pts: Vec<_> = (0..40).map(|i| tooncraft_brush::Sample::at(20.0 + i as f32 * 4.0, 30.0, 0.5, i as f64 * 0.01)).collect();
            tooncraft_brush::paint(&mut s, brush, &pts);
            (0..60).filter(|y| s.pixel(100, *y)[3] > 128).count()
        };
        let before = width(&mut h);
        click(&mut h, "Ease in");
        assert_eq!(pressure_size(&h), Curve::ease_in());
        assert!(width(&mut h) < before, "an ease-in curve gives a thinner line at half pressure");
        click(&mut h, "Reverse");
        assert_eq!(pressure_size(&h), Curve::falling());
        click(&mut h, "Linear");
        assert_eq!(pressure_size(&h), Curve::linear());
    }

    #[test]
    fn choose_which_dynamic_to_edit() {
        let mut h = harness();
        click(&mut h, "Dynamics for Opacity");
        click(&mut h, "Edit the curve of Speed");
        let st = state(&h.ctx);
        assert_eq!((st.target, st.source), (Target::Opacity, Source::Speed));
        h.get(By::new().role(egui::Role::Slider).label_contains("Curve: Speed to Opacity"));
        // the amount slider of that pairing, by keyboard
        let amount = h.get(By::new().role(egui::Role::Slider).label("Speed"));
        amount.focus();
        h.run();
        h.key_press(Key::ArrowRight);
        h.run();
        assert!(h.state().brushes[INK].dynamics.response.get(Target::Opacity, Source::Speed).amount > 0.0);
    }

    #[test]
    fn jitter_and_wet_mixing_sections_open_and_edit() {
        let mut h = harness();
        click(&mut h, "Jitter");
        let slider = h.get(By::new().role(egui::Role::Slider).label("Position jitter"));
        slider.focus();
        h.run();
        h.key_press(Key::ArrowRight);
        h.run();
        assert!(h.state().brushes[INK].dynamics.response.get(Target::Scatter, Source::Random).amount > 0.0);
        click(&mut h, "Wet mixing");
        let slider = h.get(By::new().role(egui::Role::Slider).label("Colour pickup"));
        slider.focus();
        h.run();
        h.key_press(Key::ArrowRight);
        h.run();
        assert!(h.state().brushes[INK].wet.pickup > 0.0);
    }

    #[test]
    fn the_preview_follows_the_brush() {
        let mut h = harness();
        let cached = |h: &Harness<'static, ToonApp>| -> Option<Brush> {
            h.ctx.data(|d| d.get_temp::<(Brush, u64, egui::TextureHandle)>(Id::unique("brush-preview").with("ink"))).map(|c| c.0)
        };
        let first = cached(&h).expect("a preview was drawn");
        h.state_mut().brushes[INK].size = 33.0;
        h.run();
        let second = cached(&h).expect("redrawn");
        assert_ne!(first.size, second.size);
        assert_eq!(second.size, 33.0);
    }

    #[test]
    fn the_new_tools_have_shortcuts_and_open_their_settings() {
        let mut h = harness();
        h.key_press(Key::W);
        h.run();
        assert_eq!(h.state().tool, Tool::Watercolour);
        h.key_press(Key::K);
        h.run();
        assert_eq!(h.state().tool, Tool::Smudge);
        assert_eq!(h.state().brush().kind, BrushKind::Smudge);
        // the smudge brush calls its pickup slider "Smear fade"
        click(&mut h, "Wet mixing");
        h.get(By::new().role(egui::Role::Slider).label("Smear fade"));
    }

    #[test]
    fn a_wet_stroke_through_the_app_leaves_one_undo_step() {
        let mut h = harness();
        h.state_mut().tool = Tool::Watercolour;
        let kind = BrushKind::Watercolour;
        let brush = h.state().brush();
        assert_eq!(brush.kind, kind);
        assert!(brush.wet.pickup > 0.0);
        let samples: Vec<_> = (0..20).map(|i| tooncraft_brush::Sample::at(100.0 + i as f32 * 5.0, 100.0, 0.7, i as f64 * 0.01)).collect();
        assert!(h.state_mut().run(tooncraft_engine::Command::Stroke { layer: 0, frame: 0, brush, samples }));
        assert_eq!(h.state().engine.undo_len(), 1);
    }
}
