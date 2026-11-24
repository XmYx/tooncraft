//! The liquify tool: drag to push, twirl, pinch or bloat the drawing's pixels under a soft
//! round brush. The stroke is applied when it ends, as one `Liquify` command (one undo step);
//! the brush circle and the path show on the canvas meanwhile.

use egui::{Color32, Pos2, Stroke};
use tooncraft_engine::Command;
use tooncraft_engine::liquify::{LiquifyBrush, LiquifyMode, MAX_LIQUIFY_POINTS, MAX_LIQUIFY_RADIUS};

use crate::app::ToonApp;
use crate::tool_options::{Scale, scrub};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiquifyTool {
    pub mode: LiquifyMode,
    pub brush: LiquifyBrush,
    pub path: Vec<(f32, f32)>,
    pub hover: Option<(f32, f32)>,
}

pub fn input(app: &mut ToonApp, at: Option<(f32, f32)>, hover: Option<(f32, f32)>, resp: &egui::Response) {
    let l = &mut app.aids.liquify;
    l.hover = hover;
    if let Some(p) = at {
        if resp.drag_started() {
            l.path = vec![p];
        } else if resp.dragged() && !l.path.is_empty() && l.path.len() < MAX_LIQUIFY_POINTS && l.path.last() != Some(&p) {
            l.path.push(p);
        }
    }
    if resp.drag_stopped() {
        let points = std::mem::take(&mut app.aids.liquify.path);
        if points.is_empty() {
            return;
        }
        let (mode, brush, layer, frame) = (app.aids.liquify.mode, app.aids.liquify.brush, app.layer, app.frame);
        app.run(Command::Liquify { layer, frame, mode, brush, points });
    }
}

pub fn overlay(painter: &egui::Painter, app: &ToonApp, to_screen: &dyn Fn(f32, f32) -> Pos2, accent: Color32) {
    let l = &app.aids.liquify;
    let px = |r: f32, at: (f32, f32)| (to_screen(at.0 + r, at.1) - to_screen(at.0, at.1)).length();
    if l.path.len() >= 2 {
        let pts: Vec<Pos2> = l.path.iter().map(|p| to_screen(p.0, p.1)).collect();
        painter.add(egui::Shape::line(pts, Stroke::new(1.5, accent.gamma_multiply(0.6))));
    }
    if let Some(h) = l.path.last().copied().or(l.hover) {
        let r = px(l.brush.radius, h).max(2.0);
        painter.circle_stroke(to_screen(h.0, h.1), r, Stroke::new(1.5, accent));
        painter.circle_stroke(to_screen(h.0, h.1), r * (1.0 - l.brush.strength * 0.5), Stroke::new(1.0, accent.gamma_multiply(0.5)));
    }
}

fn mode_name(m: LiquifyMode) -> String {
    match m {
        LiquifyMode::Push => tl!("Push"),
        LiquifyMode::TwirlLeft => tl!("Twirl left"),
        LiquifyMode::TwirlRight => tl!("Twirl right"),
        LiquifyMode::Pinch => tl!("Pinch"),
        LiquifyMode::Bloat => tl!("Bloat"),
    }
    .to_string()
}

pub fn options(app: &mut ToonApp, ui: &mut egui::Ui) {
    let l = &mut app.aids.liquify;
    let label = ui.label(tl!("Liquify mode"));
    egui::ComboBox::from_id_salt("liquify-mode")
        .selected_text(mode_name(l.mode))
        .show_ui(ui, |ui| {
            for m in [LiquifyMode::Push, LiquifyMode::TwirlLeft, LiquifyMode::TwirlRight, LiquifyMode::Pinch, LiquifyMode::Bloat] {
                ui.selectable_value(&mut l.mode, m, mode_name(m));
            }
        })
        .response
        .labelled_by(label.id);
    scrub(ui, tl!("Size"), &mut l.brush.radius, 2.0..=MAX_LIQUIFY_RADIUS, Scale::Log, " px");
    scrub(ui, tl!("Strength"), &mut l.brush.strength, 0.0..=1.0, Scale::Percent, " %");
    ui.label(tl!("Drag over the drawing; hold still and drag again to twirl, pinch or bloat more"));
}
