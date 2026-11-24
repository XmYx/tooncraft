//! Drawing aids in the app: shape modes beyond two-corner shapes (polyline, polygon, curve, filled
//! shapes), lasso fill, the gradient tool, symmetry, hold to straighten and grid snapping. The
//! engine commands are in `tooncraft_engine::drawing`; this module is their UI.
//!
//! Keyboard and non-drag paths: polyline points are placed by clicks (Enter finishes, Backspace
//! removes the last point, Escape cancels); every option is a selectable, checkbox or combo box in
//! the tool options bar.

use std::time::Instant;

use egui::{Color32, Pos2, Stroke};
use tooncraft_doc::palette::{MAX_STOPS, Stop, sanitize_stops};
use tooncraft_doc::symmetry::SymmetryMode;
use tooncraft_engine::Command;
use tooncraft_engine::drawing::GradientKind;
use tooncraft_engine::shapes::ShapeKind;

use crate::app::ToonApp;

/// What the shape tool draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeMode {
    Line,
    Rectangle,
    Ellipse,
    Polyline,
    Polygon,
    Curve,
}

/// Outline, fill or both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeFill {
    Outline,
    Filled,
    Both,
}

pub struct DrawAids {
    pub shape_mode: ShapeMode,
    pub shape_fill: ShapeFill,
    /// Points placed so far for a polyline, polygon or curve.
    pub path: Vec<(f32, f32)>,
    pub lasso_fill: bool,
    pub lasso: Vec<(f32, f32)>,
    pub gradient: GradientKind,
    pub gradient_to_transparent: bool,
    pub gradient_drag: Option<((f32, f32), (f32, f32))>,
    /// The clone stamp tool's settings and stroke.
    pub clone: crate::clone_tool::CloneTool,
    /// The gradient's own stops (from a gradient swatch or the stop editor); empty: the
    /// current colour to white or to transparent.
    pub gradient_stops: Vec<Stop>,
    pub snap_grid: bool,
    pub hold_straighten: bool,
    /// The ruler handle being dragged (ruler, handle, the rulers as they were at the press, so
    /// Escape puts them back), see `rulers_ui`.
    pub ruler_drag: Option<(usize, usize, tooncraft_doc::rulers::Rulers)>,
    /// The rulers' canvas outlines, kept until the rulers or the canvas size change.
    pub ruler_outlines: std::cell::RefCell<crate::rulers_ui::Outlines>,
    /// The text tool's style and the text box being typed (`text_tool`).
    pub text: crate::text_tool::TextTool,
    /// Points of the live stroke (for the recogniser) and when the pen last moved.
    stroke: Vec<(f32, f32)>,
    still_since: Option<(Instant, (f32, f32))>,
}

impl Default for DrawAids {
    fn default() -> Self {
        Self {
            shape_mode: ShapeMode::Line,
            shape_fill: ShapeFill::Outline,
            path: Vec::new(),
            lasso_fill: false,
            lasso: Vec::new(),
            gradient: GradientKind::Linear,
            gradient_to_transparent: true,
            gradient_drag: None,
            clone: Default::default(),
            gradient_stops: Vec::new(),
            snap_grid: false,
            hold_straighten: true,
            ruler_drag: None,
            ruler_outlines: Default::default(),
            text: Default::default(),
            stroke: Vec::new(),
            still_since: None,
        }
    }
}

fn snap(app: &ToonApp, p: (f32, f32)) -> (f32, f32) {
    if !app.aids.snap_grid {
        return p;
    }
    let (xs, ys) = tooncraft_doc::guides::grid(app.engine.project.width(), app.engine.project.height());
    tooncraft_engine::drawing::snap_to(p, &xs, &ys, 12.0 / app.view.zoom.max(0.01))
}

fn shape_brush(app: &ToonApp) -> tooncraft_brush::Brush {
    tooncraft_brush::Brush { color: app.color, ..app.brushes[1] }
}

fn fill_colour(app: &ToonApp) -> Option<[u8; 4]> {
    match app.aids.shape_fill {
        ShapeFill::Outline => None,
        _ => Some(app.color),
    }
}

/// Run the path shape for the placed points.
pub fn finish_path(app: &mut ToonApp) {
    let points = std::mem::take(&mut app.aids.path);
    let min = if app.aids.shape_mode == ShapeMode::Polyline { 2 } else { 3 };
    if points.len() < min {
        return;
    }
    let (closed, curve) = match app.aids.shape_mode {
        ShapeMode::Polygon => (true, false),
        ShapeMode::Curve => (app.aids.shape_fill != ShapeFill::Outline, true),
        _ => (false, false),
    };
    let fill = if closed { fill_colour(app) } else { None };
    let outline = app.aids.shape_fill != ShapeFill::Filled || !closed;
    let (layer, frame, brush) = (app.layer, app.frame, shape_brush(app));
    app.run(Command::PathShape { layer, frame, brush, points, closed, curve, outline, fill });
}

/// Enter finishes the points placed so far, Backspace removes the last one and Escape drops them.
/// Read with the app's shortcuts, before the canvas: a focused widget takes Enter and Escape.
pub fn path_keys(app: &mut ToonApp, ctx: &egui::Context) {
    if app.aids.path.is_empty() {
        return;
    }
    let (enter, back, esc) = ctx.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Backspace), i.key_pressed(egui::Key::Escape)));
    if enter {
        finish_path(app);
    } else if back {
        app.aids.path.pop();
    } else if esc {
        app.aids.path.clear();
    }
}

/// The shape tool's canvas input.
pub fn shape_input(app: &mut ToonApp, at: Option<(f32, f32)>, resp: &egui::Response, shift: bool) {
    match app.aids.shape_mode {
        ShapeMode::Polyline | ShapeMode::Polygon | ShapeMode::Curve => {
            if let Some(p) = at {
                let p = snap(app, p);
                if resp.double_clicked() {
                    finish_path(app);
                } else if resp.clicked_by(egui::PointerButton::Primary) {
                    // a pointer click: Enter on the focused canvas also counts as a click
                    app.aids.path.push(p);
                }
            }
        }
        _ => {
            let kind = app.shape;
            if let Some(p) = at {
                let p = snap(app, p);
                if resp.drag_started() {
                    app.shape_drag = Some((p, p));
                } else if resp.dragged()
                    && let Some((a, _)) = app.shape_drag
                {
                    app.shape_drag = Some((a, p));
                }
            }
            if resp.drag_stopped()
                && let Some((from, to)) = app.shape_drag.take()
            {
                let (layer, frame, brush) = (app.layer, app.frame, shape_brush(app));
                match (kind, fill_colour(app)) {
                    (ShapeKind::Line, _) | (_, None) => {
                        app.run(Command::Shape { layer, frame, brush, shape: kind, from, to, constrain: shift });
                    }
                    (_, Some(fill)) => {
                        // a filled rectangle or ellipse is the closed path of its outline
                        let pts: Vec<(f32, f32)> =
                            tooncraft_engine::shapes::samples(kind, from, to, shift, 1.0).iter().step_by(2).map(|s| (s.x, s.y)).take(4000).collect();
                        let outline = app.aids.shape_fill == ShapeFill::Both;
                        app.run(Command::PathShape { layer, frame, brush, points: pts, closed: true, curve: false, outline, fill: Some(fill) });
                    }
                }
            }
        }
    }
}

/// Lasso fill (the fill tool with "Lasso fill" on): drag around an area to fill it.
pub fn lasso_input(app: &mut ToonApp, at: Option<(f32, f32)>, resp: &egui::Response) {
    if let Some(p) = at {
        if resp.drag_started() {
            app.aids.lasso = vec![p];
        } else if resp.dragged() && app.aids.lasso.last().is_none_or(|l| (l.0 - p.0).hypot(l.1 - p.1) > 1.5 / app.view.zoom.max(0.01)) {
            app.aids.lasso.push(p);
        }
    }
    if resp.drag_stopped() {
        let points = std::mem::take(&mut app.aids.lasso);
        if points.len() >= 3 && points.len() <= tooncraft_engine::drawing::MAX_PATH_POINTS {
            let (layer, frame, brush, color) = (app.layer, app.frame, shape_brush(app), app.color);
            app.run(Command::PathShape { layer, frame, brush, points, closed: true, curve: false, outline: false, fill: Some(color) });
        }
    }
}

/// The gradient tool: drag from the start to the end.
pub fn gradient_input(app: &mut ToonApp, at: Option<(f32, f32)>, resp: &egui::Response) {
    if let Some(p) = at {
        let p = snap(app, p);
        if resp.drag_started() {
            app.aids.gradient_drag = Some((p, p));
        } else if resp.dragged()
            && let Some((a, _)) = app.aids.gradient_drag
        {
            app.aids.gradient_drag = Some((a, p));
        }
    }
    if resp.drag_stopped()
        && let Some((from, to)) = app.aids.gradient_drag.take()
    {
        let (start, end) = two_colours(app);
        let stops = app.aids.gradient_stops.clone();
        let (layer, frame, kind, area, area_rings) = (app.layer, app.frame, app.aids.gradient, app.selection.clone(), crate::selection_tools::extra_rings(app));
        app.run(Command::Gradient { layer, frame, kind, from, to, start, end, area, area_rings, stops, feather: app.sel.feather });
    }
}

/// While a freehand stroke is live: keep its points, and when the pen stays still for half a
/// second, replace it with the line or ellipse it was meant to be.
pub fn track_stroke(app: &mut ToonApp, ctx: &egui::Context) {
    if !app.engine.is_stroking() {
        app.aids.stroke.clear();
        app.aids.still_since = None;
        return;
    }
    let Some(s) = app.engine.live_last_sample() else { return };
    let p = (s.x, s.y);
    if app.aids.stroke.last() != Some(&p) && app.aids.stroke.len() < 20_000 {
        app.aids.stroke.push(p);
    }
    if !app.aids.hold_straighten || app.tool.brush().is_none() {
        return;
    }
    let moved = app.aids.still_since.is_none_or(|(_, q)| (q.0 - p.0).hypot(q.1 - p.1) > 2.0 / app.view.zoom.max(0.01));
    if moved {
        app.aids.still_since = Some((Instant::now(), p));
    } else if let Some((t, _)) = app.aids.still_since
        && t.elapsed().as_secs_f32() >= 0.5
    {
        if let Some((kind, from, to)) = tooncraft_engine::drawing::recognize(&app.aids.stroke) {
            let brush = app.brush();
            app.engine.cancel_stroke();
            app.stroke_start = None;
            let (layer, frame) = (app.layer, app.frame);
            if app.run(Command::Shape { layer, frame, brush, shape: kind, from, to, constrain: false }) {
                app.set_status(tl!("Straightened"));
            }
        }
        app.aids.still_since = None;
        app.aids.stroke.clear();
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(100));
}

/// Overlays: symmetry axes, the path being placed, the lasso and the gradient line.
pub fn overlay(painter: &egui::Painter, app: &ToonApp, to_screen: &dyn Fn(f32, f32) -> Pos2, accent: Color32) {
    let sym = app.engine.project.symmetry;
    if sym.is_on() {
        let (w, h) = (app.engine.project.width() as f32, app.engine.project.height() as f32);
        let (cx, cy) = sym.center.unwrap_or((w / 2.0, h / 2.0));
        let st = Stroke::new(1.0, Color32::from_rgb(90, 170, 255).gamma_multiply(0.8));
        let r = w.hypot(h);
        let axes: Vec<f32> = match sym.mode {
            SymmetryMode::Vertical => vec![std::f32::consts::FRAC_PI_2],
            SymmetryMode::Horizontal => vec![0.0],
            SymmetryMode::Both => vec![0.0, std::f32::consts::FRAC_PI_2],
            _ => {
                (0..sym.spokes.clamp(2, 32)).map(|k| std::f32::consts::TAU * k as f32 / sym.spokes.clamp(2, 32) as f32 - std::f32::consts::FRAC_PI_2).collect()
            }
        };
        if matches!(sym.mode, SymmetryMode::Vertical | SymmetryMode::Horizontal | SymmetryMode::Both) {
            for a in axes {
                let (s, c) = a.sin_cos();
                painter.line_segment([to_screen(cx - c * r, cy - s * r), to_screen(cx + c * r, cy + s * r)], st);
            }
        } else {
            for a in axes {
                let (s, c) = a.sin_cos();
                painter.line_segment([to_screen(cx, cy), to_screen(cx + c * r, cy + s * r)], st);
            }
        }
    }
    let line = |pts: &[(f32, f32)], closed: bool| {
        let mut v: Vec<Pos2> = pts.iter().map(|p| to_screen(p.0, p.1)).collect();
        if closed && let Some(f) = v.first().copied() {
            v.push(f);
        }
        if v.len() >= 2 {
            painter.add(egui::Shape::line(v, Stroke::new(1.5, accent)));
        }
    };
    if !app.aids.path.is_empty() {
        let pts = match app.aids.shape_mode {
            ShapeMode::Curve => tooncraft_engine::drawing::curve_points(&app.aids.path, false),
            _ => app.aids.path.clone(),
        };
        line(&pts, app.aids.shape_mode == ShapeMode::Polygon);
        for p in &app.aids.path {
            painter.circle_stroke(to_screen(p.0, p.1), 3.5, Stroke::new(1.5, accent));
        }
    }
    line(&app.aids.lasso, true);
    if let Some((a, b)) = app.aids.gradient_drag {
        line(&[a, b], false);
        painter.circle_filled(to_screen(a.0, a.1), 4.0, accent);
    }
}

fn mode_name(m: ShapeMode) -> String {
    match m {
        ShapeMode::Line => tl!("Line"),
        ShapeMode::Rectangle => tl!("Rectangle"),
        ShapeMode::Ellipse => tl!("Ellipse"),
        ShapeMode::Polyline => tl!("Polyline"),
        ShapeMode::Polygon => tl!("Polygon"),
        ShapeMode::Curve => tl!("Curve"),
    }
    .to_string()
}

fn symmetry_name(m: SymmetryMode) -> String {
    match m {
        SymmetryMode::Off => tl!("No symmetry"),
        SymmetryMode::Vertical => tl!("Mirror left and right"),
        SymmetryMode::Horizontal => tl!("Mirror top and bottom"),
        SymmetryMode::Both => tl!("Mirror four ways"),
        SymmetryMode::Radial => tl!("Radial"),
        SymmetryMode::RadialMirror => tl!("Radial, mirrored"),
    }
    .to_string()
}

/// Symmetry and hold-to-straighten for the brush tools (tool options bar).
pub fn brush_options(app: &mut ToonApp, ui: &mut egui::Ui) {
    let mut sym = app.engine.project.symmetry;
    let l = ui.label(tl!("Symmetry"));
    egui::ComboBox::from_id_salt("symmetry")
        .selected_text(symmetry_name(sym.mode))
        .show_ui(ui, |ui| {
            for m in [SymmetryMode::Off, SymmetryMode::Vertical, SymmetryMode::Horizontal, SymmetryMode::Both, SymmetryMode::Radial, SymmetryMode::RadialMirror]
            {
                ui.selectable_value(&mut sym.mode, m, symmetry_name(m));
            }
        })
        .response
        .labelled_by(l.id);
    if matches!(sym.mode, SymmetryMode::Radial | SymmetryMode::RadialMirror) {
        let l = ui.label(tl!("Copies"));
        ui.add(egui::DragValue::new(&mut sym.spokes).range(2..=32)).labelled_by(l.id);
    }
    if sym != app.engine.project.symmetry {
        app.run(Command::SetSymmetry { symmetry: sym });
    }
    ui.checkbox(&mut app.aids.hold_straighten, tl!("Hold to straighten"))
        .on_hover_text(tl!("Hold the pen still at the end of a stroke to turn it into a clean line or ellipse"));
}

fn kind_of(m: ShapeMode) -> Option<ShapeKind> {
    match m {
        ShapeMode::Line => Some(ShapeKind::Line),
        ShapeMode::Rectangle => Some(ShapeKind::Rectangle),
        ShapeMode::Ellipse => Some(ShapeKind::Ellipse),
        _ => None,
    }
}

/// The shape tool's options.
pub fn shape_options(app: &mut ToonApp, ui: &mut egui::Ui) {
    for m in [ShapeMode::Line, ShapeMode::Rectangle, ShapeMode::Ellipse, ShapeMode::Polyline, ShapeMode::Polygon, ShapeMode::Curve] {
        let on = app.aids.shape_mode == m && (kind_of(m).is_none() || kind_of(m) == Some(app.shape));
        if ui.selectable_label(on, mode_name(m)).clicked() {
            app.aids.shape_mode = m;
            if let Some(k) = kind_of(m) {
                app.shape = k;
            }
            app.aids.path.clear();
        }
    }
    ui.separator();
    for (f, n) in [(ShapeFill::Outline, tl!("Outline")), (ShapeFill::Filled, tl!("Filled")), (ShapeFill::Both, tl!("Outline and fill"))] {
        ui.selectable_value(&mut app.aids.shape_fill, f, n);
    }
    ui.checkbox(&mut app.aids.snap_grid, tl!("Snap to grid"));
    if matches!(app.aids.shape_mode, ShapeMode::Polyline | ShapeMode::Polygon | ShapeMode::Curve) {
        ui.label(tl!("Click to place points; Enter or double-click finishes"));
    }
}

/// The two-colour gradient: the current colour to transparent, or to white.
fn two_colours(app: &ToonApp) -> ([u8; 4], [u8; 4]) {
    let start = app.color;
    let end = if app.aids.gradient_to_transparent { [start[0], start[1], start[2], 0] } else { [255, 255, 255, 255] };
    (start, end)
}

/// The stops the gradient tool paints with: its own, or the two-colour ones.
pub fn gradient_stops(app: &ToonApp) -> Vec<Stop> {
    if app.aids.gradient_stops.len() >= 2 {
        return app.aids.gradient_stops.clone();
    }
    let (start, end) = two_colours(app);
    vec![Stop { at: 0.0, rgba: start }, Stop { at: 1.0, rgba: end }]
}

/// The gradient tool's options.
pub fn gradient_options(app: &mut ToonApp, ui: &mut egui::Ui) {
    ui.selectable_value(&mut app.aids.gradient, GradientKind::Linear, tl!("Linear"));
    ui.selectable_value(&mut app.aids.gradient, GradientKind::Radial, tl!("Radial"));
    if app.aids.gradient_stops.is_empty() {
        ui.checkbox(&mut app.aids.gradient_to_transparent, tl!("Fade to transparent"));
        if ui.button(tl!("Edit Stops")).on_hover_text(tl!("Set the gradient's colours and where they sit")).clicked() {
            app.aids.gradient_stops = gradient_stops(app);
        }
    } else {
        stop_editor(app, ui);
    }
    if ui.button(tl!("Add Gradient to Palette")).on_hover_text(tl!("As a gradient swatch in the palette shown")).clicked() {
        let stops = gradient_stops(app);
        crate::colour_tools::add_gradient(app, stops);
    }
    ui.checkbox(&mut app.aids.snap_grid, tl!("Snap to grid"));
    ui.label(tl!("Drag from start to end; inside the selection when there is one"));
}

/// The gradient's stops: a preview strip, then each stop's colour and position.
fn stop_editor(app: &mut ToonApp, ui: &mut egui::Ui) {
    crate::colour_tools::gradient_strip(ui, &app.aids.gradient_stops, egui::vec2(120.0, 16.0), tl!("Gradient preview").to_string());
    let mut remove = None;
    let can_remove = app.aids.gradient_stops.len() > 2;
    for (i, s) in app.aids.gradient_stops.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let mut c = egui::Color32::from_rgba_unmultiplied(s.rgba[0], s.rgba[1], s.rgba[2], s.rgba[3]);
            let l = ui.label(tlf!("Stop {n}", n = i + 1));
            if ui.color_edit_button_srgba(&mut c).labelled_by(l.id).changed() {
                s.rgba = c.to_srgba_unmultiplied();
            }
            let mut pct = (s.at * 100.0).round();
            if ui.add(egui::DragValue::new(&mut pct).range(0.0..=100.0).suffix(" %")).labelled_by(l.id).changed() {
                s.at = (pct / 100.0).clamp(0.0, 1.0);
            }
            if ui.add_enabled(can_remove, egui::Button::new(tl!("Remove"))).clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        app.aids.gradient_stops.remove(i);
    }
    ui.horizontal(|ui| {
        if ui
            .add_enabled(app.aids.gradient_stops.len() < MAX_STOPS, egui::Button::new(tl!("Add Stop")))
            .on_hover_text(tl!("The current colour, half way"))
            .clicked()
        {
            app.aids.gradient_stops.push(Stop { at: 0.5, rgba: app.color });
        }
        if ui.button(tl!("Two Colours")).on_hover_text(tl!("Back to the current colour and white or transparent")).clicked() {
            app.aids.gradient_stops.clear();
        }
    });
    // keep them in order once nothing is being dragged
    if ui.ctx().dragged_id().is_none() {
        app.aids.gradient_stops = sanitize_stops(std::mem::take(&mut app.aids.gradient_stops));
    }
}
