//! The contour tool: reshape the lines of a vector layer. Drag a line's point to move it,
//! Shift+drag it up or down to make the line thicker or thinner there; click a line to choose
//! it, and its properties (width, smoothing, colour, delete) show in the options. Every change
//! is one `edit_vector` command (one undo step).

use egui::{Color32, Pos2, Stroke};
use tooncraft_engine::Command;
use tooncraft_engine::vector::VectorEdit;
use tooncraft_vector::{MAX_WIDTH, VDrawing};

use crate::app::ToonApp;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ContourTool {
    /// The chosen line (index in the drawing).
    pub stroke: Option<usize>,
    /// A point being dragged: (stroke, point, where it is now, width change so far, Shift held).
    pub drag: Option<Drag>,
    pub hover: Option<(f32, f32)>,
    /// The width typed for the whole line.
    pub width: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    pub stroke: usize,
    pub point: usize,
    pub at: (f32, f32),
    pub start: (f32, f32),
    pub width: bool,
}

/// The vector drawing at the current cell, if the layer keeps vector lines.
fn drawing(app: &ToonApp) -> Option<&VDrawing> {
    let p = &app.engine.project;
    if !p.layers.get(app.layer).is_some_and(|l| l.vector) {
        return None;
    }
    p.drawing_at(app.layer, app.frame).and_then(|d| p.vectors.get(&d))
}

fn edit(app: &mut ToonApp, edit: VectorEdit) -> bool {
    let (layer, frame) = (app.layer, app.frame);
    app.run(Command::EditVector { layer, frame, edit })
}

/// Canvas input: `at` is the pointer in canvas pixels, `reach` the pick distance in pixels.
pub fn input(app: &mut ToonApp, at: Option<(f32, f32)>, hover: Option<(f32, f32)>, resp: &egui::Response, shift: bool, reach: f32) {
    app.aids.contour.hover = hover;
    if drawing(app).is_none() {
        if resp.clicked() || resp.drag_started() {
            app.set_status(tl!("The contour tool edits lines on a vector layer (Layers: Vector lines)"));
        }
        return;
    }
    let Some(p) = at else { return };
    if resp.drag_started() {
        let hit = drawing(app).and_then(|v| v.nearest_point(p.0, p.1, reach));
        app.aids.contour.drag = hit.map(|(stroke, point)| Drag { stroke, point, at: p, start: p, width: shift });
        if let Some((s, _)) = hit {
            app.aids.contour.stroke = Some(s);
        }
    } else if resp.dragged()
        && let Some(d) = app.aids.contour.drag.as_mut()
    {
        d.at = p;
    }
    if resp.drag_stopped()
        && let Some(d) = app.aids.contour.drag.take()
    {
        if d.width {
            let old = drawing(app).and_then(|v| v.strokes.get(d.stroke)).and_then(|s| s.points.get(d.point)).map(|q| q.width).unwrap_or(1.0);
            // up makes it thicker: half a pixel of width per pixel dragged
            let width = (old + (d.start.1 - d.at.1) * 0.5).clamp(0.0, MAX_WIDTH);
            edit(app, VectorEdit::SetWidth { stroke: d.stroke, point: Some(d.point), width });
        } else {
            edit(app, VectorEdit::MovePoint { stroke: d.stroke, point: d.point, x: d.at.0, y: d.at.1 });
        }
    } else if resp.clicked() {
        let hit = drawing(app).and_then(|v| v.stroke_at(p.0, p.1, reach));
        app.aids.contour.stroke = hit;
        if let Some(w) = hit.and_then(|s| drawing(app).and_then(|v| v.strokes.get(s))).and_then(|s| s.points.iter().map(|q| q.width).reduce(f32::max)) {
            app.aids.contour.width = w;
        }
    }
}

/// The chosen line's points, the point being dragged and the point under the pointer.
pub fn overlay(painter: &egui::Painter, app: &ToonApp, to_screen: &dyn Fn(f32, f32) -> Pos2, accent: Color32) {
    let Some(v) = drawing(app) else { return };
    let c = &app.aids.contour;
    let st = Stroke::new(1.5, accent);
    if let Some(s) = c.stroke.and_then(|s| v.strokes.get(s)) {
        let line: Vec<Pos2> = s.samples(2.0).iter().map(|q| to_screen(q.x, q.y)).collect();
        painter.add(egui::Shape::line(line, Stroke::new(1.0, accent.gamma_multiply(0.6))));
        for q in &s.points {
            let at = to_screen(q.x, q.y);
            painter.rect_filled(egui::Rect::from_center_size(at, egui::vec2(6.0, 6.0)), 1.0, accent.gamma_multiply(0.85));
        }
    }
    if let Some(d) = c.drag {
        let from = v.strokes.get(d.stroke).and_then(|s| s.points.get(d.point)).map(|q| to_screen(q.x, q.y));
        let to = to_screen(d.at.0, d.at.1);
        if let Some(f) = from {
            painter.line_segment([f, to], Stroke::new(1.0, accent));
        }
        painter.circle_stroke(to, 6.0, st);
    }
}

/// The tool's options: the chosen line's properties.
pub fn options(app: &mut ToonApp, ui: &mut egui::Ui) {
    let chosen = app.aids.contour.stroke.filter(|s| drawing(app).is_some_and(|v| *s < v.strokes.len()));
    let Some(stroke) = chosen else {
        ui.label(tl!("Drag a point to move it, Shift+drag to change the width there; click a line to choose it"));
        return;
    };
    ui.label(tlf!("Line {n}", n = stroke + 1));
    let l = ui.label(tl!("Line width"));
    let mut w = app.aids.contour.width;
    let resp = ui.add(egui::DragValue::new(&mut w).range(0.0..=MAX_WIDTH).suffix(" px")).labelled_by(l.id);
    app.aids.contour.width = w;
    if resp.lost_focus() || resp.drag_stopped() {
        edit(app, VectorEdit::SetWidth { stroke, point: None, width: w });
    }
    if ui.button(tl!("Smooth Line")).on_hover_text(tl!("Pull its points towards their neighbours")).clicked() {
        edit(app, VectorEdit::Smooth { stroke, strength: 0.5, passes: 2 });
    }
    if ui.button(tl!("Use Current Colour")).on_hover_text(tl!("Recolour the line with the current colour")).clicked() {
        let color = app.color;
        edit(app, VectorEdit::SetColor { stroke, color });
    }
    if ui.button(tl!("Delete Line")).clicked() && edit(app, VectorEdit::DeleteStroke { stroke }) {
        app.aids.contour.stroke = None;
    }
}
