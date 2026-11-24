//! UI tests for several cameras and cuts in the Camera panel, from the keyboard: add a camera,
//! key it, cut to it, rename it, and remove the cut and the camera.

use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::{By, NodeT, Queryable};
use tooncraft_engine::Command;

use crate::app::ToonApp;
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
    s.run(Command::SetCameraKey { frame: 0, x: 300.0, y: 200.0, zoom: 1.5, angle: 0.0, camera: 0 });
    s.workspace.raise(crate::dock::Pane::Camera);
    h.run();
    h.run();
    h
}

fn press(h: &mut Harness<'static, ToonApp>, label: &str) {
    h.get(By::new().role(egui::Role::Button).label(label)).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
}

#[test]
fn a_second_camera_is_added_keyed_and_cut_to_from_the_keyboard() {
    let mut h = harness();
    press(&mut h, "Add Camera");
    assert_eq!(h.state().engine.project.cameras.len(), 1);
    assert_eq!(h.state().camera_edit, 1, "the new camera is the one being edited");
    assert_eq!(h.state().engine.project.cameras[0].camera.keys, h.state().engine.project.camera.keys, "it starts as a copy");
    // key the new camera from the panel's fields
    h.get_by_label("Zoom").focus();
    h.run();
    for _ in 0..5 {
        h.key_press(Key::ArrowUp);
        h.run();
    }
    h.key_press(Key::Enter);
    h.run();
    let close = h.state().engine.project.cameras[0].camera.at(0, 1, 1).zoom;
    assert!(close > 1.5, "the edited camera's zoom went up: {close}");
    assert_eq!(h.state().engine.project.camera.at(0, 1, 1).zoom, 1.5, "the main camera stays");
    // cut to it on frame 6
    h.state_mut().frame = 5;
    h.run();
    press(&mut h, "Cut Here");
    let p = &h.state().engine.project;
    assert_eq!(p.camera_cuts.iter().map(|c| (c.frame, c.camera)).collect::<Vec<_>>(), [(5, 1)]);
    assert_eq!((p.camera_index_at(4), p.camera_index_at(5)), (0, 1));
    assert_eq!(h.state().status, "Cut to “Camera 2” at frame 6");
    h.get_by_label("Frame 6: Camera 2");
    // rename it
    h.get_by_label("Camera name").focus();
    h.run();
    h.key_press(Key::End);
    for _ in 0.."Camera 2".len() {
        h.key_press(Key::Backspace);
    }
    h.get_by_label("Camera name").type_text("Close");
    h.run();
    h.key_press(Key::Tab);
    h.run();
    assert_eq!(h.state().engine.project.cameras[0].name, "Close");
    h.get_by_label("Frame 6: Close");
    // the cut and then the camera go
    press(&mut h, "Remove Cut");
    assert!(h.state().engine.project.camera_cuts.is_empty());
    press(&mut h, "Remove Camera");
    assert!(h.state().engine.project.cameras.is_empty());
    assert_eq!(h.state().camera_edit, 0);
    assert!(h.get(By::new().role(egui::Role::Button).label("Remove Camera")).accesskit_node().is_disabled(), "the main camera stays");
}

#[test]
fn the_camera_path_edits_the_picked_camera() {
    let mut h = harness();
    h.state_mut().run(Command::AddCamera { name: "Close".into(), from: None });
    h.state_mut().run(Command::SetCameraKey { frame: 0, x: 600.0, y: 300.0, zoom: 2.0, angle: 0.0, camera: 1 });
    h.state_mut().camera_edit = 1;
    h.state_mut().tool = crate::app::Tool::Transform;
    h.run();
    h.run();
    let s = h.state();
    let size = egui::vec2(s.engine.project.width() as f32, s.engine.project.height() as f32);
    let at = |x, y| s.view.to_screen(s.workspace.canvas_rect, size, egui::pos2(x, y));
    let (from, to) = (at(600.0, 300.0), at(600.0, 300.0) + egui::vec2(30.0, 0.0));
    h.drag_at(from);
    h.run();
    h.hover_at(to);
    h.run();
    h.drop_at(to);
    h.run();
    h.run();
    let p = &h.state().engine.project;
    assert!(p.cameras[0].camera.keys[0].x > 600.0, "the picked camera's key moved: {:?}", p.cameras[0].camera.keys);
    assert_eq!(p.camera.keys[0].x, 300.0, "the main camera's key stayed");
}
