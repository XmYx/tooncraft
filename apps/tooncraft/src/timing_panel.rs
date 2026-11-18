//! The Timing panel: mark drawings as keys, breakdowns or inbetweens; see the spacing chart of
//! the stretch around the current frame next to the chart an ease would give; add inbetweens with
//! guides; re-time between keys; apply timing presets.

use egui::{Color32, Stroke, pos2, vec2};
use tooncraft_doc::{Ease, Mark, TimingPreset};
use tooncraft_engine::Command;

use crate::app::ToonApp;

/// Panel state that is not part of the project.
pub struct TimingUi {
    pub ease: Ease,
    pub count: u32,
    pub guides: bool,
    pub preset: usize,
}

impl Default for TimingUi {
    fn default() -> Self {
        Self { ease: Ease::Linear, count: 3, guides: true, preset: 1 }
    }
}

pub fn mark_label(m: Option<Mark>) -> String {
    match m {
        Some(Mark::Key) => tl!("Key"),
        Some(Mark::Breakdown) => tl!("Breakdown"),
        Some(Mark::Inbetween) => tl!("Inbetween"),
        None => tl!("No mark"),
    }
    .to_string()
}

fn presets() -> Vec<(String, TimingPreset)> {
    vec![
        (tl!("On ones").to_string(), TimingPreset::Even { frames: 1 }),
        (tl!("On twos").to_string(), TimingPreset::Even { frames: 2 }),
        (tl!("On threes").to_string(), TimingPreset::Even { frames: 3 }),
        (tl!("On fours").to_string(), TimingPreset::Even { frames: 4 }),
        (tl!("Slow start (3s to 1s)").to_string(), TimingPreset::SlowStart),
        (tl!("Slow end (1s to 3s)").to_string(), TimingPreset::SlowEnd),
        (tl!("Slow at both ends").to_string(), TimingPreset::SlowBoth),
    ]
}

/// Frames of the stretch around the current frame: the anchor (key or breakdown, else any
/// drawing) at or before it, and the next anchor.
fn stretch(app: &ToonApp) -> Option<(u32, u32, Vec<u32>)> {
    let p = &app.engine.project;
    let l = p.layers.get(app.layer)?;
    let ex = l.exposures(0, p.length().saturating_sub(1));
    let anchored = ex.iter().any(|e| p.marks.get(&e.drawing).is_some_and(|m| m.is_anchor()));
    let is_anchor = |d| !anchored || p.marks.get(&d).is_some_and(|m: &Mark| m.is_anchor());
    let a = ex.iter().rposition(|e| e.start <= app.frame && is_anchor(e.drawing))?;
    let b = ex.iter().enumerate().skip(a + 1).find(|(_, e)| is_anchor(e.drawing)).map(|(i, _)| i)?;
    let starts = ex[a..=b].iter().map(|e| e.start).collect();
    Some((ex[a].start, ex[b].start, starts))
}

pub fn panel(app: &mut ToonApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().id_salt("timing-scroll").auto_shrink([false, false]).show(ui, |ui| panel_body(app, ui));
}

fn panel_body(app: &mut ToonApp, ui: &mut egui::Ui) {
    let (li, frame) = (app.layer, app.frame);
    let p = &app.engine.project;
    let current = p.drawing_at(li, frame);
    let mark = current.and_then(|d| p.marks.get(&d).copied());
    ui.label(match current {
        Some(_) => tlf!("Drawing at frame {n}: {mark}", n = frame + 1, mark = mark_label(mark)),
        None => tlf!("Frame {n} has no drawing", n = frame + 1),
    });
    ui.add_enabled_ui(current.is_some(), |ui| {
        ui.horizontal(|ui| {
            for m in [Some(Mark::Key), Some(Mark::Breakdown), Some(Mark::Inbetween), None] {
                if ui.selectable_label(mark == m, mark_label(m)).clicked() && mark != m {
                    app.run(Command::SetMark { layer: li, frame, mark: m });
                }
            }
        });
    });
    ui.separator();
    // ease used by the chart, the inbetween helper and smart re-timing
    egui::Grid::new("timing-grid").num_columns(2).spacing(vec2(8.0, 5.0)).show(ui, |ui| {
        let l = ui.label(tl!("Spacing"));
        let mut ease = app.timing_ui.ease;
        egui::ComboBox::from_id_salt("timing-ease")
            .selected_text(crate::pegs::ease_label(ease))
            .show_ui(ui, |ui| {
                for e in [Ease::Linear, Ease::In, Ease::Out, Ease::InOut] {
                    ui.selectable_value(&mut ease, e, crate::pegs::ease_label(e));
                }
            })
            .response
            .labelled_by(l.id);
        app.timing_ui.ease = ease;
        ui.end_row();
        let l = ui.label(tl!("Inbetweens"));
        ui.add(egui::DragValue::new(&mut app.timing_ui.count).range(1..=tooncraft_engine::inbetween::MAX_INBETWEENS)).labelled_by(l.id);
        ui.end_row();
    });
    chart(app, ui);
    ui.checkbox(&mut app.timing_ui.guides, tl!("Guides on a layer above"));
    ui.horizontal(|ui| {
        if ui.button(tl!("Add Inbetweens")).on_hover_text(tl!("New drawings between this drawing and the next, placed along the spacing")).clicked() {
            let (count, ease, guides) = (app.timing_ui.count, app.timing_ui.ease, app.timing_ui.guides);
            app.run(Command::AddInbetweens { layer: li, frame, count, ease, guides });
        }
        let (from, to) = range(app);
        if ui.button(tl!("Re-time Between Keys")).on_hover_text(tl!("Keys and breakdowns stay put; the drawings between them follow the spacing")).clicked() {
            let ease = app.timing_ui.ease;
            app.run(Command::RetimeBetweenKeys { layer: li, from, to, ease });
        }
    });
    ui.separator();
    let list = presets();
    ui.horizontal(|ui| {
        let l = ui.label(tl!("Timing preset"));
        let mut i = app.timing_ui.preset.min(list.len().saturating_sub(1));
        egui::ComboBox::from_id_salt("timing-preset")
            .selected_text(list.get(i).map(|p| p.0.clone()).unwrap_or_default())
            .show_ui(ui, |ui| {
                for (k, (name, _)) in list.iter().enumerate() {
                    ui.selectable_value(&mut i, k, name);
                }
            })
            .response
            .labelled_by(l.id);
        app.timing_ui.preset = i;
        let (from, to) = range(app);
        if ui.button(tl!("Apply")).on_hover_text(tl!("Re-time the selected frames (or the whole layer)")).clicked()
            && let Some((_, preset)) = list.get(i).cloned()
        {
            app.run(Command::ApplyTiming { layer: li, from, to, preset });
        }
    });
    crate::timeline_extras::names(app, ui);
    crate::timeline_extras::section(app, ui);
}

/// The selected frames on the current layer, else the whole shot.
fn range(app: &ToonApp) -> (u32, u32) {
    if app.has_range() {
        let (_, _, f0, f1) = app.selection();
        (f0, f1)
    } else {
        (0, app.engine.project.length().saturating_sub(1))
    }
}

/// The spacing chart: the drawings of the stretch as ticks along a bar (above), and where the
/// chosen ease would put the same number of inbetweens (below).
fn chart(app: &ToonApp, ui: &mut egui::Ui) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width().clamp(120.0, 280.0), 46.0), egui::Sense::hover());
    let label = match stretch(app) {
        Some((a, b, s)) => tlf!("Spacing chart: {n} drawings between frames {a} and {b}", n = s.len().saturating_sub(2), a = a + 1, b = b + 1),
        None => tl!("Spacing chart: no next drawing").to_string(),
    };
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, label.clone()));
    let painter = ui.painter_at(rect);
    let fg = ui.visuals().text_color();
    let weak = ui.visuals().weak_text_color();
    let accent = ui.visuals().selection.stroke.color;
    let Some((a, b, starts)) = stretch(app) else {
        painter.text(rect.left_center(), egui::Align2::LEFT_CENTER, tl!("No next drawing"), egui::FontId::proportional(11.0), weak);
        return;
    };
    let (x0, x1) = (rect.left() + 6.0, rect.right() - 6.0);
    let x = |t: f32| x0 + (x1 - x0) * t.clamp(0.0, 1.0);
    let span = (b - a).max(1) as f32;
    let y = rect.top() + 14.0;
    painter.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(1.0, weak));
    for (i, s) in starts.iter().enumerate() {
        let t = (s - a) as f32 / span;
        let big = i == 0 || i + 1 == starts.len();
        let h = if big { 9.0 } else { 6.0 };
        painter.line_segment([pos2(x(t), y - h), pos2(x(t), y + h)], Stroke::new(if big { 2.0 } else { 1.5 }, fg));
    }
    let n = starts.len().saturating_sub(2) as u32;
    let y2 = rect.bottom() - 10.0;
    painter.line_segment([pos2(x0, y2), pos2(x1, y2)], Stroke::new(1.0, weak));
    for t in [0.0, 1.0] {
        painter.line_segment([pos2(x(t), y2 - 7.0), pos2(x(t), y2 + 7.0)], Stroke::new(2.0, accent));
    }
    for t in tooncraft_doc::timing::spacing(if n > 0 { n } else { app.timing_ui.count.min(32) }, app.timing_ui.ease) {
        painter.line_segment([pos2(x(t), y2 - 5.0), pos2(x(t), y2 + 5.0)], Stroke::new(1.5, accent));
    }
    painter.text(pos2(x1, rect.top()), egui::Align2::RIGHT_TOP, tlf!("{n} frames", n = b - a), egui::FontId::proportional(10.0), Color32::from_gray(150));
}
