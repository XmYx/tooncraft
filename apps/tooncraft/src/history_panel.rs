//! The Undo History panel (every step of the shot's history, click to jump) and the recent
//! colours strip of the Colour panel.

use egui::{Color32, Sense, Stroke, Ui, vec2};

use crate::app::ToonApp;

/// Most recent colours kept.
pub const MAX_RECENT: usize = 16;

/// Remember a colour that was just painted with (most recent first, no repeats).
pub fn note_colour(recent: &mut Vec<[u8; 3]>, c: [u8; 4]) {
    let rgb = [c[0], c[1], c[2]];
    if recent.first() == Some(&rgb) {
        return;
    }
    recent.retain(|r| *r != rgb);
    recent.insert(0, rgb);
    recent.truncate(MAX_RECENT);
}

/// The strip of recent colours, newest first; click one to paint with it again.
pub fn recent(app: &mut ToonApp, ui: &mut Ui) {
    ui.label(tl!("Recent colours"));
    if app.recent.is_empty() {
        ui.weak(tl!("Colours you paint with show up here"));
        return;
    }
    let mut chosen = None;
    ui.horizontal_wrapped(|ui| {
        for s in &app.recent {
            let (rect, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
            ui.painter().rect(rect, 3.0, Color32::from_rgb(s[0], s[1], s[2]), Stroke::new(1.0, Color32::from_gray(120)), egui::StrokeKind::Inside);
            if resp.has_focus() {
                ui.painter().rect_stroke(rect, 3.0, ui.visuals().selection.stroke, egui::StrokeKind::Outside);
            }
            let label = tlf!("Recent colour #{hex}", hex = format!("{:02X}{:02X}{:02X}", s[0], s[1], s[2]));
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label.clone()));
            if resp.clicked() {
                chosen = Some(*s);
            }
        }
    });
    if let Some(s) = chosen {
        app.color = [s[0], s[1], s[2], 255];
    }
}

/// The Undo History panel.
pub fn panel(app: &mut ToonApp, ui: &mut Ui) {
    let (done, undone): (Vec<String>, Vec<String>) = {
        let (d, u) = app.engine.history();
        (d.into_iter().map(str::to_string).collect(), u.into_iter().map(str::to_string).collect())
    };
    let position = done.len();
    ui.horizontal(|ui| {
        if ui.add_enabled(app.engine.can_undo(), egui::Button::new(tl!("Undo"))).clicked() {
            app.undo();
        }
        if ui.add_enabled(app.engine.can_redo(), egui::Button::new(tl!("Redo"))).clicked() {
            app.redo();
        }
        ui.weak(tlf!("{done} steps, {undone} to redo", done = position, undone = undone.len()));
    });
    ui.separator();
    let mut jump = None;
    egui::ScrollArea::vertical().id_salt("history-scroll").auto_shrink([false, false]).show(ui, |ui| {
        // newest first, like a log; the redo steps sit above the current one, dimmed
        for (i, label) in undone.iter().enumerate().rev() {
            let step = position + 1 + (undone.len() - 1 - i);
            let text = egui::RichText::new(tlf!("Step {n}: {label}", n = step, label = label)).weak();
            if ui.selectable_label(false, text).on_hover_text(tl!("Redo up to here")).clicked() {
                jump = Some(step);
            }
        }
        for (i, label) in done.iter().enumerate().rev() {
            let step = i + 1;
            let current = step == position;
            let text = if current { tlf!("Step {n}: {label} (now)", n = step, label = label) } else { tlf!("Step {n}: {label}", n = step, label = label) };
            if ui.selectable_label(current, text).on_hover_text(tl!("Go back to just after this step")).clicked() {
                jump = Some(step);
            }
        }
        let start = if position == 0 { tl!("Start (now)").to_string() } else { tl!("Start").to_string() };
        if ui.selectable_label(position == 0, start).on_hover_text(tl!("Undo everything still remembered")).clicked() {
            jump = Some(0);
        }
    });
    if let Some(step) = jump {
        match app.engine.go_to_step(step) {
            Ok(_) => {
                app.clamp_selection();
                app.set_status(tlf!("Went to step {n}", n = step));
            }
            Err(e) => app.set_status(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_colours_are_newest_first_unique_and_bounded() {
        let mut r = Vec::new();
        note_colour(&mut r, [1, 2, 3, 255]);
        note_colour(&mut r, [4, 5, 6, 255]);
        note_colour(&mut r, [1, 2, 3, 255]);
        assert_eq!(r, vec![[1, 2, 3], [4, 5, 6]]);
        note_colour(&mut r, [1, 2, 3, 128]);
        assert_eq!(r.len(), 2, "the same colour is not repeated");
        for i in 0..100u8 {
            note_colour(&mut r, [i, 0, 0, 255]);
        }
        assert_eq!(r.len(), MAX_RECENT);
        assert_eq!(r.first(), Some(&[99, 0, 0]));
    }
}
