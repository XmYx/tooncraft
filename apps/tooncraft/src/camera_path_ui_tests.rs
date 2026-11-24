//! UI tests for the camera path on the canvas: with the Transform tool a camera key's square drags
//! to move the key, the current frame's cross drags to key the camera there, Escape cancels.

use egui::Key;
use egui_kittest::Harness;
use tooncraft_doc::CameraKey;
use tooncraft_engine::Command;

use crate::app::{Tool, ToonApp};
use crate::args::Cli;

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(egui::vec2(1500.0, 1100.0)).with_accessibility_check(true).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    let s = h.state_mut();
    s.run(Command::SetCameraKey { frame: 0, x: 300.0, y: 200.0, zoom: 1.5, angle: 0.0 });
    s.run(Command::SetCameraKey { frame: 10, x: 800.0, y: 400.0, zoom: 1.5, angle: 10.0 });
    s.tool = Tool::Transform;
    h.run();
    h.run();
    h
}

fn screen(h: &Harness<'static, ToonApp>, x: f32, y: f32) -> egui::Pos2 {
    let s = h.state();
    let size = egui::vec2(s.engine.project.width() as f32, s.engine.project.height() as f32);
    s.view.to_screen(s.workspace.canvas_rect, size, egui::pos2(x, y))
}

fn drag(h: &mut Harness<'static, ToonApp>, from: egui::Pos2, to: egui::Pos2, cancel: bool) {
    h.drag_at(from);
    h.run();
    h.hover_at(from + (to - from) * 0.5);
    h.run();
    h.hover_at(to);
    h.run();
    if cancel {
        h.key_press(Key::Escape);
        h.run();
    }
    h.drop_at(to);
    h.run();
    h.run();
}

fn keys(h: &Harness<'static, ToonApp>) -> Vec<CameraKey> {
    h.state().engine.project.camera.keys.clone()
}

/// Canvas pixels per screen point at the current view.
fn scale(h: &Harness<'static, ToonApp>) -> f32 {
    let (a, b) = (screen(h, 0.0, 0.0), screen(h, 100.0, 0.0));
    100.0 / (b.x - a.x)
}

#[test]
fn dragging_a_key_square_moves_that_key_in_one_undo_step() {
    let mut h = harness();
    h.state_mut().frame = 4;
    h.run();
    let depth = h.state().engine.undo_len();
    let s = scale(&h);
    let a = screen(&h, 800.0, 400.0);
    drag(&mut h, a, a + egui::vec2(60.0, 30.0), false);
    let k = keys(&h);
    assert_eq!(k.len(), 2);
    assert_eq!((k[0].x, k[0].y), (300.0, 200.0), "the other key stays");
    assert!((k[1].x - (800.0 + 60.0 * s)).abs() < 1.0 && (k[1].y - (400.0 + 30.0 * s)).abs() < 1.0, "{:?}", k[1]);
    assert_eq!((k[1].frame, k[1].zoom, k[1].angle), (10, 1.5, 10.0), "only the position moves");
    assert_eq!(h.state().engine.undo_len(), depth + 1, "one undo step");
    assert_eq!(h.state().engine.undo_label(), Some("Camera key"));
}

#[test]
fn escape_cancels_a_camera_drag() {
    let mut h = harness();
    let before = keys(&h);
    let depth = h.state().engine.undo_len();
    let a = screen(&h, 800.0, 400.0);
    drag(&mut h, a, a + egui::vec2(80.0, 0.0), true);
    assert_eq!(keys(&h), before);
    assert_eq!(h.state().engine.undo_len(), depth);
    assert!(h.state().camera_drag.is_none(), "no preview left behind");
}

#[test]
fn dragging_the_current_frame_keys_the_camera_there() {
    let mut h = harness();
    h.state_mut().frame = 5;
    h.run();
    let p = &h.state().engine.project;
    let here = p.camera.at(5, p.width(), p.height());
    let s = scale(&h);
    let a = screen(&h, here.x, here.y);
    drag(&mut h, a, a + egui::vec2(0.0, -40.0), false);
    let k = keys(&h);
    assert_eq!(k.len(), 3, "{k:?}");
    let new = k[1];
    assert_eq!(new.frame, 5);
    assert!((new.x - here.x).abs() < 0.5 && (new.y - (here.y - 40.0 * s)).abs() < 1.0, "{new:?}");
    assert!((new.zoom - here.zoom).abs() < 1e-4 && (new.angle - here.angle).abs() < 1e-4, "keeps the eased zoom and turn");
}

#[test]
fn away_from_the_handles_the_camera_is_left_alone() {
    let mut h = harness();
    let before = keys(&h);
    let a = screen(&h, 100.0, 600.0);
    drag(&mut h, a, a + egui::vec2(50.0, 50.0), false);
    assert_eq!(keys(&h), before);
    // hidden camera frame: no handles at all
    h.state_mut().show_camera = false;
    h.run();
    let a = screen(&h, 800.0, 400.0);
    drag(&mut h, a, a + egui::vec2(50.0, 50.0), false);
    assert_eq!(keys(&h), before);
}
