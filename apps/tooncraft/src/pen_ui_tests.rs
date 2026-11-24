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

fn button(h: &Harness<'static, ToonApp>, pos: egui::Pos2, b: egui::PointerButton, pressed: bool) {
    h.event(egui::Event::PointerButton { pos, button: b, pressed, modifiers: egui::Modifiers::NONE });
}

#[test]
fn a_mapped_side_button_runs_its_command_instead_of_panning() {
    let mut h = harness();
    let canvas = h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).rect();
    let line = egui::Rect::from_center_size(canvas.center(), egui::vec2(200.0, 20.0));
    drag(&mut h, line);
    assert_eq!(h.state().engine.undo_len(), 1);
    h.state_mut().preferences.prefs.pen_buttons = ["edit.undo".into(), String::new()];
    // with a mouse (no pen near) the middle button still pans and undoes nothing
    let pan = h.state().view.pan;
    let c = canvas.center();
    h.hover_at(c);
    h.run();
    button(&h, c, egui::PointerButton::Middle, true);
    h.run();
    button(&h, c, egui::PointerButton::Middle, false);
    h.run();
    assert_eq!(h.state().engine.undo_len(), 1, "a mouse's middle button is not a pen button");
    // the pen near: the lower side button (a middle click) undoes, once per press, and does not pan
    h.state().tablet.set(pen(0.0));
    button(&h, c, egui::PointerButton::Middle, true);
    h.run();
    h.hover_at(c + egui::vec2(40.0, 0.0));
    h.run();
    button(&h, c + egui::vec2(40.0, 0.0), egui::PointerButton::Middle, false);
    h.run();
    assert_eq!(h.state().engine.undo_len(), 0, "the side button undid the stroke");
    assert_eq!(h.state().view.pan, pan, "and did not pan");
    // Windows reports the barrel button as a state: one push is one press
    h.state().tablet.set(None);
    h.state_mut().preferences.prefs.pen_buttons = ["edit.redo".into(), String::new()];
    h.state().tablet.simulate_windows(true, false, true);
    h.run();
    h.run();
    h.state().tablet.simulate_windows(true, false, false);
    h.run();
    assert_eq!(h.state().engine.undo_len(), 1, "the barrel button redid it");
}

#[test]
fn windows_ink_pen_touches_draw() {
    let mut h = harness();
    let canvas = h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).rect();
    // with Windows Ink the pen arrives as touches with force; the hooks only say it is near, so
    // its touches are not palms
    h.state().tablet.simulate_windows(true, false, false);
    h.run();
    let t = |h: &Harness<'static, ToonApp>, phase, pos| {
        h.event(egui::Event::Touch { device_id: egui::TouchDeviceId(0), id: egui::TouchId(1), phase, pos, force: Some(0.6) })
    };
    t(&h, egui::TouchPhase::Start, canvas.center());
    h.run();
    assert!(h.state().engine.is_stroking(), "the Windows Ink pen draws");
    t(&h, egui::TouchPhase::End, canvas.center());
    h.run();
}

#[test]
fn windows_ink_eraser_end_erases() {
    let mut h = harness();
    let canvas = h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).rect();
    let line = egui::Rect::from_center_size(canvas.center(), egui::vec2(200.0, 20.0));
    // the eraser end (from the hooks) erases with the eraser's settings
    h.state_mut().brushes[1].size = 20.0;
    drag(&mut h, line);
    let inked = ink(&h);
    assert!(inked > 100, "{inked}");
    h.state().tablet.simulate_windows(true, true, false);
    h.state_mut().mouse_pressure = 1.0;
    h.state_mut().brushes[4].size = 120.0;
    drag(&mut h, line);
    assert!(ink(&h) < inked / 4, "the eraser end erased ({inked} → {})", ink(&h));
}

fn ink(h: &Harness<'static, ToonApp>) -> usize {
    let app = h.state();
    let Some(d) = app.engine.project.drawing_at(app.layer, app.frame) else { return 0 };
    app.engine.project.drawing(d).map(|s| s.tiles().map(|(_, t)| t.bytes().as_chunks::<4>().0.iter().filter(|p| p[3] > 0).count()).sum()).unwrap_or(0)
}
