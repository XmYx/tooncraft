//! The status strip under the canvas: zoom and rotation, the tool and layer, the drawing, the
//! frame, the colour and the cursor on the left; on the right the latest notice (it fades after a
//! few seconds, and is a polite live region so screen readers announce it) and the shot's size,
//! memory and timing in a weaker tone. Errors are reported here; nothing in it can stop the app.

use egui::{Align, Color32, Layout, RichText, Sense, Stroke, Ui, vec2};

use crate::app::ToonApp;

/// Seconds a notice stays at full strength, then seconds it takes to fade to the weak tone.
const HOLD_S: f64 = 4.0;
const FADE_S: f64 = 1.5;

/// How strongly a notice `age` seconds old is shown: 1 at first, falling to 0. A clock that
/// went backwards or is not a number shows the notice at full strength.
pub fn notice_strength(age: f64) -> f32 {
    if age.is_nan() || age <= HOLD_S {
        return 1.0;
    }
    (1.0 - (age - HOLD_S) / FADE_S).clamp(0.0, 1.0) as f32
}

pub fn show(app: &mut ToonApp, ui: &mut Ui) {
    let pal = crate::theme::palette(app.light_theme);
    let weak = |t: String| RichText::new(t).color(pal.weak);
    ui.horizontal(|ui| {
        // read-only text: it takes no clicks (selectable labels would catch them)
        ui.style_mut().interaction.selectable_labels = false;
        ui.spacing_mut().item_spacing.x = 10.0;
        let p = &app.engine.project;
        ui.label(weak(format!("{:.0} %  {:.0}°", app.view.zoom * 100.0, app.view.rotation_degrees())));
        let layer = p.layers.get(app.layer).map(|l| l.name.clone()).unwrap_or_default();
        ui.label(format!("{} · {layer}", app.tool.label()));
        let drawing = match p.drawing_at(app.layer, app.frame) {
            Some(d) => match p.drawing_names.get(&d) {
                Some(name) => tlf!("drawing {id}, {name}", id = d.0, name = name),
                None => tlf!("drawing {id}", id = d.0),
            },
            None => tl!("no drawing").to_string(),
        };
        ui.label(weak(drawing));
        ui.label(tlf!("Frame {frame} / {total}", frame = app.frame + 1, total = p.length()));
        let c = app.color;
        let hex = format!("{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let (r, swatch) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
            ui.painter().rect(r, 2.0, Color32::from_rgb(c[0], c[1], c[2]), Stroke::new(1.0, pal.weak), egui::StrokeKind::Inside);
            let colour = tlf!("Colour #{hex}", hex = hex.clone());
            swatch.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, colour.clone()));
            ui.label(weak(format!("#{hex}")));
        });
        if let Some((x, y)) = app.pointer {
            ui.label(weak(tlf!("x {x}  y {y}", x = x, y = y)));
        }
        // one control high: a right-to-left layout would otherwise take all the height there is
        let h = ui.spacing().interact_size.y;
        ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), h), Layout::right_to_left(Align::Center), |ui| {
            let perf = if app.playback.playing {
                tlf!("{fps} fps playback · {ms} ms/frame", fps = format!("{:.0}", app.stats.playback_fps), ms = format!("{:.1}", app.stats.frame_ms))
            } else {
                tlf!("stroke {ms} ms", ms = format!("{:.2}", app.stats.stroke_ms))
            };
            ui.label(weak(perf));
            ui.label(weak(tlf!(
                "Memory: {pixels} MB drawings, {history} MB history",
                pixels = p.memory_bytes() >> 20,
                history = app.engine.history_bytes() >> 20
            )));
            ui.label(weak(tlf!("{w}×{h} · {fps} fps", w = p.width(), h = p.height(), fps = p.fps())));
            // the notice: full strength when it changes, then it fades to the weak tone
            let now = ui.input(|i| i.time);
            let key = egui::Id::unique("tooncraft-status-notice");
            let (seen, since) = ui.data(|d| d.get_temp::<(String, f64)>(key)).unwrap_or_default();
            let since = if seen == app.status { since } else { now };
            ui.data_mut(|d| d.insert_temp(key, (app.status.clone(), since)));
            let s = notice_strength(now - since);
            // The status line is a polite live region: results of actions are announced.
            let status = ui.label(RichText::new(&app.status).color(lerp(pal.weak, pal.text, s)));
            ui.ctx().accesskit_node_builder(status.id, |n| n.set_live(egui::accesskit::Live::Polite));
            if s > 0.0 && !app.status.is_empty() {
                // repaint while the notice is still to fade
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
            }
        });
    });
}

fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 1.0 };
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_notice_holds_then_fades_and_odd_clocks_do_not_hide_it() {
        assert_eq!(notice_strength(0.0), 1.0);
        assert_eq!(notice_strength(HOLD_S), 1.0);
        let mid = notice_strength(HOLD_S + FADE_S / 2.0);
        assert!(mid > 0.4 && mid < 0.6);
        assert_eq!(notice_strength(HOLD_S + FADE_S + 1.0), 0.0);
        assert_eq!(notice_strength(f64::INFINITY), 0.0);
        assert_eq!(notice_strength(-5.0), 1.0);
        assert_eq!(notice_strength(f64::NAN), 1.0);
    }

    #[test]
    fn colours_blend_between_the_ends() {
        let (a, b) = (Color32::from_rgb(0, 0, 0), Color32::from_rgb(200, 100, 50));
        assert_eq!(lerp(a, b, 0.0), a);
        assert_eq!(lerp(a, b, 1.0), b);
        assert_eq!(lerp(a, b, f32::NAN), b);
    }
}
