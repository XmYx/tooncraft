//! UI tests for the Drawing, Play and Scene menus and the Scene Settings window. Menus are opened
//! with clicks; keys are pressed as keys. The accessibility check is on.

use egui::{Key, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};

use crate::app::ToonApp;
use crate::args::Cli;

fn harness(cli: Cli) -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, cli).without_autosave();
    let mut h = Harness::builder().with_size(vec2(1600.0, 1000.0)).with_accessibility_check(true).with_max_steps(8).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    h.run();
    h
}

/// Open a top menu (the menu bar comes first; the top bar also has a Play button).
fn menu(h: &mut Harness<'static, ToonApp>, name: &str) {
    h.query_all(By::new().role(egui::Role::Button).label(name)).next().expect("menu").click();
    h.run();
}

#[test]
fn scene_settings_change_size_rate_and_length_in_one_window() {
    let mut h = harness(Cli::default());
    menu(&mut h, "Scene");
    h.get_by_label("Scene Settings…").click();
    h.run();
    assert!(h.state().scene_form.is_some());
    let steps_before = h.state().engine.undo_len();
    {
        let f = h.state_mut().scene_form.as_mut().expect("open");
        f.width = 1280;
        f.height = 720;
        f.fps = 12;
        f.frames = 30;
    }
    h.run();
    h.get_by_label("Apply").click();
    h.run();
    let p = &h.state().engine.project;
    assert_eq!((p.width(), p.height(), p.fps(), p.length()), (1280, 720, 12, 30));
    assert_eq!(h.state().engine.undo_len(), steps_before + 3, "one undo step per changed value");
    // Escape closes the window, the shot keeps its settings
    h.key_press(Key::Escape);
    h.run();
    assert!(h.state().scene_form.is_none());
    assert_eq!(h.state().engine.project.fps(), 12);
}

#[test]
fn open_scene_shows_the_window_and_applying_nothing_changes_nothing() {
    let mut h = harness(Cli { open: Some("scene".into()), ..Default::default() });
    assert!(h.state().scene_form.is_some());
    let before = h.state().engine.undo_len();
    h.get_by_label("Apply").click();
    h.run();
    assert_eq!(h.state().engine.undo_len(), before, "no change: no undo step");
}

#[test]
fn the_play_menu_steps_loops_and_sets_the_speed() {
    let mut h = harness(Cli { demo: true, ..Default::default() });
    let last = h.state().engine.project.length() - 1;
    menu(&mut h, "Play");
    h.get_by_label("Last Frame").click();
    h.run();
    assert_eq!(h.state().frame, last);
    menu(&mut h, "Play");
    h.get_by_label("First Frame").click();
    h.run();
    assert_eq!(h.state().frame, 0);
    let looping = h.state().playback.looping;
    menu(&mut h, "Play");
    h.get(By::new().role(egui::Role::CheckBox).label("Loop")).click();
    h.run();
    assert_eq!(h.state().playback.looping, !looping);
    menu(&mut h, "Play");
    h.get(By::new().role(egui::Role::Button).label("Speed")).hover();
    h.run();
    h.get_by_label("50 %").click();
    h.run();
    assert!((h.state().playback.speed() - 0.5).abs() < 1e-9);
}

#[test]
fn the_drawing_menu_makes_and_steps_drawings() {
    let mut h = harness(Cli::default());
    let (layer, frame) = (h.state().layer, h.state().frame);
    assert!(h.state().engine.project.drawing_at(layer, frame).is_none());
    menu(&mut h, "Drawing");
    h.get_by_label("New Drawing").click();
    h.run();
    assert!(h.state().engine.project.drawing_at(layer, frame).is_some());
}
