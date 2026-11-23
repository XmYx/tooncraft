//! UI tests for the app shell: the start window and File ▸ Open Recent. Keys are pressed as keys;
//! the accessibility check is on. Shots are written to a temporary folder, never the user's.

use std::path::PathBuf;

use egui::{Key, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, NodeT, Queryable};

use crate::app::ToonApp;
use crate::args::Cli;

fn harness_with(app: ToonApp) -> Harness<'static, ToonApp> {
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

fn app(cli: Cli) -> ToonApp {
    ToonApp::with_context(&egui::Context::default(), cli).without_autosave()
}

fn start_cli() -> Cli {
    Cli { open: Some("start".into()), ..Default::default() }
}

/// A saved shot named `name` in a fresh temporary folder.
fn saved_shot(tag: &str, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tooncraft-shell-{tag}-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{name}.toon"));
    let project = tooncraft_doc::Project::new(name, 64, 48, 12, 6).expect("project");
    tooncraft_format::save(&path, &project).expect("saved");
    path
}

#[test]
fn the_start_window_creates_a_shot_from_the_keyboard() {
    let mut h = harness_with(app(start_cli()));
    assert!(h.state().start.open);
    h.get_by_label("Recent shots");
    h.get_by_label("Open the Demo Shot");
    // no recent shots: Create has the focus, so Enter makes the shot from the form
    h.state_mut().start.form.name = "Walk cycle".into();
    h.state_mut().start.form.frames = 7;
    h.run();
    assert!(h.get_by_label("Create").is_focused(), "Create has the focus when no recent shot is there");
    h.key_press(Key::Enter);
    h.run();
    assert!(!h.state().start.open, "closed after creating");
    assert_eq!(h.state().engine.project.name, "Walk cycle");
    assert_eq!(h.state().engine.project.length(), 7);
}

#[test]
fn escape_closes_the_start_window_and_keeps_the_blank_shot() {
    let mut h = harness_with(app(start_cli()));
    let before = h.state().engine.project.name.clone();
    h.key_press(Key::Escape);
    h.run();
    assert!(!h.state().start.open);
    assert_eq!(h.state().engine.project.name, before);
}

#[test]
fn the_start_window_opens_a_recent_shot_with_enter_and_marks_missing_ones() {
    let shot = saved_shot("start", "Recent one");
    let mut a = app(start_cli());
    a.recent_files.note(&shot.with_file_name("Gone.toon")).expect("in memory");
    a.recent_files.note(&shot).expect("in memory");
    let mut h = harness_with(a);
    // the missing shot is listed, marked and cannot be picked
    assert!(h.get_by_label("Gone.toon (missing)").accesskit_node().is_disabled(), "a missing shot cannot be opened");
    // the first shot that is still there has the focus
    assert!(h.get_by_label("Recent one.toon").is_focused());
    h.key_press(Key::Enter);
    h.run();
    assert!(!h.state().start.open);
    assert_eq!(h.state().engine.project.name, "Recent one");
    assert_eq!(h.state().path.as_deref(), Some(shot.as_path()));
    let _ = std::fs::remove_dir_all(shot.parent().expect("dir"));
}

#[test]
fn file_open_recent_lists_shots_newest_first_and_opens_one() {
    let older = saved_shot("menu", "Older");
    let newer = saved_shot("menu", "Newer");
    let mut a = app(Cli::default());
    a.recent_files.note(&older).expect("in memory");
    a.recent_files.note(&newer).expect("in memory");
    assert!(!a.start.open, "the UI tests never get the start window unasked");
    let mut h = harness_with(a);
    h.get_by_label("File").click();
    h.run();
    h.get_by_label("Open Recent").hover();
    h.run();
    let items: Vec<String> =
        h.query_all(By::new().role(egui::Role::Button)).filter_map(|n| n.accesskit_node().label()).filter(|l| l.ends_with(".toon")).collect();
    assert_eq!(items, vec!["Newer.toon".to_string(), "Older.toon".to_string()], "newest first");
    h.get_by_label("Older.toon").click();
    h.run();
    assert_eq!(h.state().engine.project.name, "Older");
    // opening moved it to the top
    assert_eq!(h.state().recent_files.paths.first().and_then(|p| p.file_name()).and_then(|n| n.to_str()), Some("Older.toon"));
    let _ = std::fs::remove_dir_all(older.parent().expect("dir"));
}

#[test]
fn saving_puts_the_shot_in_the_recent_list() {
    let dir = std::env::temp_dir().join(format!("tooncraft-shell-save-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("saved.toon");
    let mut h = harness_with(app(Cli::default()));
    assert!(h.state_mut().save_to(&path));
    assert_eq!(h.state().recent_files.paths.first().map(|p| p.ends_with("saved.toon")), Some(true));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A launch with an untitled recovery copy in a temporary autosave folder.
fn with_recovery_copy(tag: &str) -> (ToonApp, PathBuf) {
    let dir = std::env::temp_dir().join(format!("tooncraft-shell-recover-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let a = app(start_cli());
    let mut lost = a.engine.project.clone();
    lost.set_length(9).expect("length");
    let key = tooncraft_format::autosave::key_for(None, &a.engine.project.name);
    tooncraft_format::autosave::write(&dir, &key, &lost).expect("recovery copy");
    let a = a.with_autosave_dir(dir.clone());
    assert!(a.recovery.is_some(), "the recovery copy is offered");
    (a, dir)
}

#[test]
fn the_start_window_waits_until_the_recovery_offer_is_answered() {
    let (a, dir) = with_recovery_copy("discard");
    let mut h = harness_with(a);
    // only the recovery offer is shown; the start window is not drawn over it
    h.get_by_label("Recover unsaved work");
    assert!(h.query_by_label("Recent shots").is_none(), "no start window while the offer stands");
    // pressing Enter does not create a shot behind the offer
    let name = h.state().engine.project.name.clone();
    h.key_press(Key::Enter);
    h.run();
    assert!(h.state().recovery.is_some());
    assert_eq!(h.state().engine.project.name, name);
    // Discard: the start window comes up
    h.get_by_label("Discard").click();
    h.run();
    assert!(h.state().recovery.is_none());
    h.get_by_label("Recent shots");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn recovering_closes_the_start_window() {
    let (a, dir) = with_recovery_copy("recover");
    let mut h = harness_with(a);
    h.get_by_label("Recover").click();
    h.run();
    assert!(h.state().recovery.is_none());
    assert!(!h.state().start.open, "the recovered work is not hidden behind the start window");
    assert!(h.query_by_label("Recent shots").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
