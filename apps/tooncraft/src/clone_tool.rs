//! The clone stamp tool: Alt+click sets the point to copy from, then painting copies from there
//! (the same layer, or another layer chosen in the options). Aligned keeps the same offset for
//! every stroke; otherwise each stroke starts again from the source point. One `CloneStamp`
//! command (one undo step) per stroke.

use egui::{Color32, Pos2, Stroke};
use tooncraft_engine::Command;
use tooncraft_engine::clone::{CloneTip, MAX_CLONE_POINTS, MAX_CLONE_SIZE};

use crate::app::ToonApp;
use crate::tool_options::{Scale, scrub};

#[derive(Clone, Debug, PartialEq)]
pub struct CloneTool {
    pub tip: CloneTip,
    /// Where to copy from (canvas pixels), set with Alt+click.
    pub source: Option<(f32, f32)>,
    /// The layer to copy from; `None`: the layer being painted.
    pub source_layer: Option<usize>,
    /// Keep the offset from one stroke to the next.
    pub aligned: bool,
    /// Source minus destination, once a stroke has begun.
    pub offset: Option<(f32, f32)>,
    /// The stroke being painted.
    pub path: Vec<(f32, f32)>,
    /// The pointer over the canvas, for the overlay.
    pub hover: Option<(f32, f32)>,
}

impl Default for CloneTool {
    fn default() -> Self {
        Self { tip: CloneTip::default(), source: None, source_layer: None, aligned: true, offset: None, path: Vec::new(), hover: None }
    }
}

/// Canvas input: `at` is the pointer in canvas pixels.
pub fn input(app: &mut ToonApp, at: Option<(f32, f32)>, hover: Option<(f32, f32)>, resp: &egui::Response, alt: bool) {
    app.aids.clone.hover = hover;
    if alt {
        if resp.clicked()
            && let Some(p) = at
        {
            set_source(app, p);
        }
        return;
    }
    if resp.drag_started()
        && let Some(p) = at
    {
        let c = &mut app.aids.clone;
        let Some(src) = c.source else {
            app.set_status(tl!("Alt+click where to copy from first"));
            return;
        };
        if !(c.aligned && c.offset.is_some()) {
            c.offset = Some((src.0 - p.0, src.1 - p.1));
        }
        c.path = vec![p];
    } else if resp.dragged()
        && let Some(p) = at
    {
        let c = &mut app.aids.clone;
        if !c.path.is_empty() && c.path.len() < MAX_CLONE_POINTS && c.path.last() != Some(&p) {
            c.path.push(p);
        }
    }
    if resp.drag_stopped() {
        finish(app);
    }
}

/// Set the point to copy from; the next stroke measures its offset from there.
pub fn set_source(app: &mut ToonApp, p: (f32, f32)) {
    app.aids.clone.source = Some(p);
    app.aids.clone.offset = None;
    app.set_status(tlf!("Clone source set at {x}, {y}", x = p.0.round(), y = p.1.round()));
}

/// Paint the stroke drawn so far as one undo step.
pub fn finish(app: &mut ToonApp) {
    let c = &mut app.aids.clone;
    let points = std::mem::take(&mut c.path);
    let Some(offset) = c.offset else { return };
    if !c.aligned {
        c.offset = None;
    }
    if points.is_empty() {
        return;
    }
    let (tip, source_layer) = (c.tip, c.source_layer.filter(|l| *l < app.engine.project.layers.len()));
    let (layer, frame) = (app.layer, app.frame);
    app.run(Command::CloneStamp { layer, frame, source_layer, offset, tip, points });
}

/// The source cross and the tip outline under the pointer; while painting, the stroke so far.
pub fn overlay(painter: &egui::Painter, app: &ToonApp, to_screen: &dyn Fn(f32, f32) -> Pos2, accent: Color32) {
    let c = &app.aids.clone;
    let px = |r: f32, at: (f32, f32)| (to_screen(at.0 + r, at.1) - to_screen(at.0, at.1)).length();
    let st = Stroke::new(1.5, accent);
    let r = c.tip.size / 2.0;
    if c.path.len() >= 2 {
        let pts: Vec<Pos2> = c.path.iter().map(|p| to_screen(p.0, p.1)).collect();
        let w = c.path.first().map(|p| px(c.tip.size, *p)).unwrap_or(1.0).max(1.0);
        painter.add(egui::Shape::line(pts, Stroke::new(w, accent.gamma_multiply(0.25))));
    }
    // where the copy comes from now: the source, or the pointer plus the offset
    let from = match (c.offset, c.path.last().copied().or(c.hover)) {
        (Some(o), Some(p)) => Some((p.0 + o.0, p.1 + o.1)),
        _ => c.source,
    };
    if let Some(s) = from {
        let p = to_screen(s.0, s.1);
        let arm = px(r, s).clamp(6.0, 400.0);
        painter.line_segment([p - egui::vec2(arm, 0.0), p + egui::vec2(arm, 0.0)], st);
        painter.line_segment([p - egui::vec2(0.0, arm), p + egui::vec2(0.0, arm)], st);
        painter.circle_stroke(p, px(r, s).max(2.0), Stroke::new(1.0, accent.gamma_multiply(0.7)));
    }
    if let Some(h) = c.hover {
        painter.circle_stroke(to_screen(h.0, h.1), px(r, h).max(2.0), st);
    }
}

/// The tool's options: tip size, hardness and opacity, the layer to copy from, aligned.
pub fn options(app: &mut ToonApp, ui: &mut egui::Ui) {
    let names: Vec<String> = app.engine.project.layers.iter().map(|l| l.name.clone()).collect();
    let c = &mut app.aids.clone;
    scrub(ui, tl!("Size"), &mut c.tip.size, 1.0..=MAX_CLONE_SIZE, Scale::Log, " px");
    scrub(ui, tl!("Hardness"), &mut c.tip.hardness, 0.0..=1.0, Scale::Percent, " %");
    scrub(ui, tl!("Opacity"), &mut c.tip.opacity, 0.0..=1.0, Scale::Percent, " %");
    let this = tl!("This layer").to_string();
    let shown = c.source_layer.and_then(|l| names.get(l).cloned()).unwrap_or_else(|| this.clone());
    let l = ui.label(tl!("Copy from"));
    egui::ComboBox::from_id_salt("clone-source-layer")
        .selected_text(shown)
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut c.source_layer, None, this);
            for (i, n) in names.iter().enumerate() {
                ui.selectable_value(&mut c.source_layer, Some(i), n);
            }
        })
        .response
        .labelled_by(l.id);
    ui.checkbox(&mut c.aligned, tl!("Aligned")).on_hover_text(tl!("Keep the same offset from one stroke to the next"));
    ui.label(if c.source.is_some() { tl!("Alt+click sets a new source") } else { tl!("Alt+click where to copy from, then paint") });
}
