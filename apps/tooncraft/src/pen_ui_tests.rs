//! The Pen preferences through the app: the calibration pad reads the (simulated) tablet, Fit
//! Curve to My Hand sets the global pressure curve, and strokes on the canvas then see the mapped
//! pressure, while the mouse keeps its own setting.

use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};

use crate::app::{Tool, ToonApp};
use crate::args::Cli;
use crate::tablet::Pen;

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1000.0)).with_accessibility_check(true).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    h.state_mut().tool = Tool::Ink;
    h.run();
    h
}

fn pen(pressure: f32) -> Option<Pen> {
    Some(Pen { pressure, tilt_x: 0.0, tilt_y: 0.0, rotation: 0.0, eraser: false })
}

/// Drag across `rect` and return the last live stroke sample's pressure, if a stroke ran.
fn drag(h: &mut Harness<'static, ToonApp>, rect: egui::Rect) -> Option<f32> {
    let (a, b) = (rect.left_center() + egui::vec2(10.0, 0.0), rect.right_center() - egui::vec2(10.0, 0.0));
    h.drag_at(a);
    h.run();
    for i in 1..=10 {
        h.hover_at(a + (b - a) * (i as f32 / 10.0));
        h.run();
    }
    let p = h.state().engine.live_last_sample().map(|s| s.pressure);
    h.drop_at(b);
    h.run();
    p
}

#[test]
fn calibrating_a_light_hand_gives_full_pressure_on_the_canvas() {
    let mut h = harness();
    h.state_mut().preferences.open = true;
    h.run();
    h.get(By::new().label("Pen").role(egui::Role::Button)).click();
    h.run();
    h.run();
    // a light hand: the pen never goes past 0.5
    h.state().tablet.set(pen(0.5));
    let pad = h.get(By::new().label("Calibration pad")).rect();
    drag(&mut h, pad);
    drag(&mut h, pad);
    assert!(h.state().preferences.calibration.readings.len() >= 8, "{} readings", h.state().preferences.calibration.readings.len());
    h.get_by_label("Fit Curve to My Hand").click();
    h.run();
    assert!(!h.state().preferences.prefs.pressure_curve.is_linear());
    // close the window with the keyboard, then draw with the same light press
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(!h.state().preferences.open);
    let canvas = h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).rect();
    let line = egui::Rect::from_center_size(canvas.center(), egui::vec2(200.0, 20.0));
    let p = drag(&mut h, line);
    assert!(p.is_some_and(|p| p > 0.95), "the hardest press of the calibration is full pressure: {p:?}");
    // the mouse keeps its own setting, untouched by the curve
    h.state().tablet.set(None);
    h.state_mut().mouse_pressure = 0.4;
    let p = drag(&mut h, line.translate(egui::vec2(0.0, 60.0)));
    assert!(p.is_some_and(|p| (p - 0.4).abs() < 1e-4), "{p:?}");
}
