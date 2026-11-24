//! UI tests for the unsaved-changes ask before New, Open and Open Recent replace the shot, and
//! for F6 with the Timeline closed. Keys are pressed as keys; the accessibility check is on.
//! Shots are written to a temporary folder, never the user's.

use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_engine::Command;

use crate::app::ToonApp;
use crate::args::Cli;
use crate::dock::Pane;

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
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

fn make_unsaved_change(h: &mut Harness<'static, ToonApp>) {
    let (layer, frame) = (h.state().layer, h.state().frame);
    assert!(h.state_mut().run(Command::NewDrawing { layer, frame }));
    assert!(h.state().engine.is_dirty());
    h.run();
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tooncraft-unsaved-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[test]
fn a_saved_shot_is_replaced_without_asking() {
    let mut h = harness();
    assert!(!h.state().engine.is_dirty());
    h.key_press_modifiers(Modifiers::COMMAND, Key::N);
    h.run();
    assert!(h.state().pending_replace.is_none());
    assert!(h.state().new_shot.is_some(), "the New Shot dialog opens at once");
}

#[test]
fn new_shot_with_unsaved_changes_asks_and_escape_keeps_the_work() {
    let mut h = harness();
    make_unsaved_change(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::N);
    h.run();
    assert!(h.state().new_shot.is_none(), "nothing replaced yet");
    h.get_by_label("Don’t Save");
    h.key_press(Key::Escape);
    h.run();
    assert!(h.state().pending_replace.is_none());
    assert!(h.state().new_shot.is_none());
    assert!(h.state().engine.is_dirty(), "the work is still there");
    // ask again and answer Don't Save: the New Shot dialog opens
    h.key_press_modifiers(Modifiers::COMMAND, Key::N);
    h.run();
    h.get_by_label("Don’t Save").click();
    h.run();
    assert!(h.state().pending_replace.is_none());
    assert!(h.state().new_shot.is_some());
}

#[test]
fn enter_saves_first_then_goes_on() {
    let dir = temp_dir("save");
    let path = dir.join("work.toon");
    let mut h = harness();
    assert!(h.state_mut().save_to(&path));
    make_unsaved_change(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::N);
    h.run();
    // Save has the focus, so Enter saves and then opens the New Shot dialog
    assert!(h.query_all(By::new().role(egui::Role::Button).label("Save")).any(|n| n.is_focused()), "Save has the focus");
    h.key_press(Key::Enter);
    h.run();
    assert!(!h.state().engine.is_dirty(), "saved");
    assert!(h.state().new_shot.is_some());
    let back = tooncraft_format::load(&path).expect("saved file");
    let (layer, frame) = (h.state().layer, h.state().frame);
    assert!(back.drawing_at(layer, frame).is_some(), "the new drawing is in the saved file");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn open_recent_with_unsaved_changes_asks_first() {
    let dir = temp_dir("recent");
    let other = dir.join("Other.toon");
    tooncraft_format::save(&other, &tooncraft_doc::Project::new("Other", 64, 48, 12, 6).expect("project")).expect("saved");
    let mut h = harness();
    h.state_mut().recent_files.note(&other).expect("in memory");
    make_unsaved_change(&mut h);
    h.get_by_label("File").click();
    h.run();
    h.get_by_label("Open Recent").hover();
    h.run();
    h.get_by_label("Other.toon").click();
    h.run();
    assert_ne!(h.state().engine.project.name, "Other", "not replaced before the answer");
    h.get_by_label("Don’t Save").click();
    h.run();
    assert_eq!(h.state().engine.project.name, "Other");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn f6_brings_a_closed_timeline_back_and_focuses_the_sheet() {
    let mut h = harness();
    assert!(h.state_mut().workspace.close(Pane::Timeline));
    h.run();
    h.key_press(Key::F6);
    h.run();
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(crate::canvas::canvas_id()), "first F6: the canvas");
    h.key_press(Key::F6);
    h.run();
    assert!(h.state().workspace.is_open(Pane::Timeline), "the timeline came back");
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(crate::timeline::grid_id()), "and has the focus");
}
