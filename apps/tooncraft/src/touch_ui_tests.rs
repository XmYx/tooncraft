//! Touch gestures and palm rejection through the app: events go in as a touch screen would send
//! them, the history and the drawing come out.

use egui::{Event, TouchDeviceId, TouchId, TouchPhase};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_engine::Command;

use crate::app::{Tool, ToonApp};
use crate::args::Cli;
use crate::tablet::Pen;

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(egui::vec2(1500.0, 940.0)).with_step_dt(0.02).with_accessibility_check(true).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    h.state_mut().tool = Tool::Ink;
    h.run();
    h.run();
    h
}

fn canvas_centre(h: &Harness<'static, ToonApp>) -> egui::Pos2 {
    h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).rect().center()
}

fn touch(h: &Harness<'static, ToonApp>, id: u64, phase: TouchPhase, pos: egui::Pos2) {
    h.event(Event::Touch { device_id: TouchDeviceId(0), id: TouchId(id), phase, pos, force: None });
}

/// Draw two strokes so there is something to undo.
fn two_strokes(h: &mut Harness<'static, ToonApp>) {
    let brush = h.state().brush();
    for y in [40.0, 80.0] {
        let samples: Vec<_> = (0..12).map(|i| tooncraft_brush::Sample::at(30.0 + i as f32 * 6.0, y, 1.0, i as f64 * 0.01)).collect();
        assert!(h.state_mut().run(Command::Stroke { layer: 0, frame: 0, brush, samples }));
    }
    h.run();
}

#[test]
fn a_two_finger_tap_undoes_and_a_three_finger_tap_redoes() {
    let mut h = harness();
    two_strokes(&mut h);
    assert_eq!(h.state().engine.undo_len(), 2);
    let c = canvas_centre(&h);
    // two fingers down and up together: undo
    touch(&h, 1, TouchPhase::Start, c);
    touch(&h, 2, TouchPhase::Start, c + egui::vec2(40.0, 0.0));
    touch(&h, 1, TouchPhase::End, c);
    touch(&h, 2, TouchPhase::End, c + egui::vec2(40.0, 0.0));
    h.run();
    assert_eq!(h.state().engine.undo_len(), 1, "undone");
    assert!(!h.state().engine.is_stroking());
    assert_eq!(h.state().engine.redo_len(), 1);
    // three fingers: redo
    for (id, dx) in [(1, 0.0), (2, 40.0), (3, 80.0)] {
        touch(&h, id, TouchPhase::Start, c + egui::vec2(dx, 20.0));
    }
    for (id, dx) in [(1, 0.0), (2, 40.0), (3, 80.0)] {
        touch(&h, id, TouchPhase::End, c + egui::vec2(dx, 20.0));
    }
    h.run();
    assert_eq!(h.state().engine.undo_len(), 2, "redone");
}

#[test]
fn a_second_finger_cancels_the_stroke_the_first_one_began() {
    let mut h = harness();
    let c = canvas_centre(&h);
    touch(&h, 1, TouchPhase::Start, c);
    h.run();
    assert!(h.state().engine.is_stroking(), "one finger draws");
    touch(&h, 2, TouchPhase::Start, c + egui::vec2(50.0, 0.0));
    h.run();
    assert!(!h.state().engine.is_stroking(), "the second finger makes it a gesture");
    assert_eq!(h.state().engine.undo_len(), 0, "and the dot it began is gone");
    // lifting both after a pinch (the fingers moved far) does not undo anything
    touch(&h, 2, TouchPhase::Move, c + egui::vec2(200.0, 0.0));
    touch(&h, 1, TouchPhase::End, c);
    touch(&h, 2, TouchPhase::End, c + egui::vec2(200.0, 0.0));
    h.run();
    assert_eq!(h.state().engine.undo_len(), 0);
}

#[test]
fn a_palm_while_the_pen_is_near_draws_nothing() {
    let mut h = harness();
    let c = canvas_centre(&h);
    h.state().tablet.set(Some(Pen { pressure: 0.0, tilt_x: 0.0, tilt_y: 0.0, rotation: 0.0, eraser: false }));
    h.run();
    touch(&h, 1, TouchPhase::Start, c);
    h.run();
    assert!(!h.state().engine.is_stroking(), "a touch next to the pen is a palm");
    touch(&h, 1, TouchPhase::End, c);
    h.run();
    assert_eq!(h.state().engine.undo_len(), 0);
    // the pen goes away; a moment later touch works again
    h.state().tablet.set(None);
    h.state_mut().touch = Default::default();
    h.run();
    touch(&h, 2, TouchPhase::Start, c);
    h.run();
    assert!(h.state().engine.is_stroking());
}
