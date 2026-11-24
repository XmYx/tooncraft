//! UI tests for the reference viewer: opened from the Colour panel, a picture loaded from a
//! file, a click on it picks its colour, and the keyboard cross picks with arrows and Enter.
//! The accessibility check is on.

use egui::{Key, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_doc::Project;
use tooncraft_engine::{Command, Engine};

use crate::app::ToonApp;
use crate::args::Cli;

/// A temporary folder removed when the test ends, also when it fails.
struct TempDir(std::path::PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(vec2(1700.0, 1100.0)).with_accessibility_check(true).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    h.state_mut().workspace.raise(crate::dock::Pane::Colour);
    h.run();
    h
}

/// A 40×20 PNG: red on the left half, blue on the right.
fn picture(dir: &std::path::Path) -> std::path::PathBuf {
    let mut e = Engine::new(Project::new("Ref", 40, 20, 12, 1).unwrap());
    let half = |x0: f32, x1: f32| Some(vec![(x0, 0.0), (x1, 0.0), (x1, 20.0), (x0, 20.0)]);
    for (area, c) in [(half(0.0, 20.0), [255, 0, 0, 255]), (half(20.0, 40.0), [0, 0, 255, 255])] {
        e.apply(Command::Gradient {
            layer: 0,
            frame: 0,
            kind: Default::default(),
            from: (0.0, 0.0),
            to: (1.0, 0.0),
            start: c,
            end: c,
            area,
            area_rings: Vec::new(),
            stops: Vec::new(),
            feather: 0.0,
        })
        .unwrap();
    }
    let files = tooncraft_engine::export::png_sequence(&e.project, dir, "ref", Default::default(), |_| {}).unwrap();
    files[0].clone()
}

#[test]
fn a_picture_opens_and_clicks_or_the_keyboard_pick_its_colours() {
    let dir = TempDir(std::env::temp_dir().join(format!("tooncraft-reference-viewer-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&dir.0);
    std::fs::create_dir_all(&dir.0).unwrap();
    let file = picture(&dir.0);
    let mut h = harness();
    let toggle = By::new().role(egui::Role::Button).label("Reference Viewer");
    h.get(toggle.clone()).scroll_to_me();
    h.run();
    h.get(toggle).click();
    h.run();
    assert!(h.state().colour.viewer.open, "the Colour panel opens the viewer");
    h.get(By::new().role(egui::Role::Image).label("No reference picture: open one"));
    crate::reference_viewer::load(h.state_mut(), &file);
    h.run();
    h.run();
    let pic = h.get(By::new().role(egui::Role::Image).label_contains("Reference picture ref")).rect();
    // the picture fits the area: its left quarter is red, its right quarter blue
    h.state_mut().color = [9, 9, 9, 200];
    let at = pic.center() - vec2(40.0, 0.0);
    h.hover_at(at);
    h.run();
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    h.run();
    assert_eq!(h.state().color, [255, 0, 0, 200], "red picked, the opacity kept");
    // keyboard: the cross starts where the click was; Shift+Right ×3 moves 30 px into the blue
    for _ in 0..3 {
        h.key_press_modifiers(egui::Modifiers::SHIFT, Key::ArrowRight);
        h.run();
    }
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(h.state().color, [0, 0, 255, 200], "blue picked from the keyboard");
    // a file that is not a picture says so and keeps the old one
    let bad = dir.0.join("not-a-picture.png");
    std::fs::write(&bad, b"hello").unwrap();
    crate::reference_viewer::load(h.state_mut(), &bad);
    h.run();
    assert!(h.state().status.starts_with("Could not open the reference"), "{}", h.state().status);
    assert!(h.state().colour.viewer.image.as_ref().is_some_and(|(n, _)| n.starts_with("ref")));
}
