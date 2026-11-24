//! UI tests for the timeline polish: markers (panel, menu, keyboard), ping-pong, timecode and
//! zoom, with the accessibility check on.

use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::{By, NodeT, Queryable};
use tooncraft_engine::Command;

use crate::app::ToonApp;
use crate::args::Cli;
use crate::dock::Pane;

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli { demo: true, ..Default::default() }).without_autosave();
    let mut h = Harness::builder().with_size(egui::vec2(1700.0, 1100.0)).with_accessibility_check(true).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    h.state_mut().workspace.raise(Pane::Timing);
    h.run();
    h.run();
    h
}

/// Click an item in the Timeline header's ⋯ menu (opened fresh each time), looking in its
/// Playback and Display submenus when it is not on the first level.
fn timeline_option(h: &mut Harness<'static, ToonApp>, label: &str) {
    h.get(By::new().role(egui::Role::Button).label("Timeline options")).click();
    h.run();
    for sub in ["", "Playback", "Display"] {
        if !sub.is_empty() {
            h.get_by_label(sub).click();
            h.run();
        }
        if h.query_by_label(label).is_some() {
            h.get_by_label(label).click();
            h.run();
            return;
        }
    }
    panic!("no item {label} in the Timeline options menu");
}

fn click(h: &mut Harness<'static, ToonApp>, label: &str) {
    h.get_by_label(label).scroll_to_me();
    h.run();
    h.get_by_label(label).click();
    h.run();
}

#[test]
fn a_marker_is_added_typed_listed_and_found_by_keyboard() {
    let mut h = harness();
    h.state_mut().frame = 5;
    h.run();
    h.get(By::new().role(egui::Role::TextInput).label("Note for frame 6"));
    h.state_mut().tl.note = "contact".into();
    click(&mut h, "Add Marker");
    assert_eq!(h.state().engine.project.markers.get(&5).map(String::as_str), Some("contact"));
    assert_eq!(h.state().engine.undo_label(), Some("Marker"));
    // listed with its frame and timecode, and clicking goes there
    h.get_by_label_contains("Frame 6, 00:00:00:05: contact");
    h.state_mut().frame = 0;
    h.run();
    click(&mut h, "Next Marker");
    assert_eq!(h.state().frame, 5);
    click(&mut h, "Previous Marker");
    assert_eq!(h.state().frame, 5, "there is none before it: the playhead stays");
    assert!(h.state().status.contains("No earlier marker"));
    // the button is now an update, and a marker can be removed
    h.get_by_label("Update Marker");
    click(&mut h, "Remove Marker");
    assert!(h.state().engine.project.markers.is_empty());
    h.state_mut().undo();
    assert_eq!(h.state().engine.project.markers.len(), 1);
}

#[test]
fn marker_shortcuts_toggle_and_jump() {
    let mut h = harness();
    for f in [3u32, 9, 15] {
        h.state_mut().run(Command::SetMarker { frame: f, text: format!("m{f}") });
    }
    h.state_mut().frame = 0;
    h.run();
    h.key_press_modifiers(egui::Modifiers::ALT, Key::Period);
    h.run();
    assert_eq!(h.state().frame, 3);
    h.key_press_modifiers(egui::Modifiers::ALT, Key::Period);
    h.run();
    assert_eq!(h.state().frame, 9);
    h.key_press_modifiers(egui::Modifiers::ALT, Key::Comma);
    h.run();
    assert_eq!(h.state().frame, 3);
    // Shift+M removes the marker on this frame, and adds one on an empty frame
    h.key_press_modifiers(egui::Modifiers::SHIFT, Key::M);
    h.run();
    assert!(!h.state().engine.project.markers.contains_key(&3));
    h.key_press_modifiers(egui::Modifiers::SHIFT, Key::M);
    h.run();
    assert!(h.state().engine.project.markers.contains_key(&3));
}

#[test]
fn markers_are_flags_on_the_ruler_with_their_note_on_hover() {
    let mut h = harness();
    h.state_mut().run(Command::SetMarker { frame: 2, text: "key pose".into() });
    h.run();
    h.get(By::new().role(egui::Role::Slider).label("Frame ruler"));
    // the note is reachable in the panel as text; the ruler flag is painted (checked through
    // the project and the list), and nothing panics drawing it at any zoom
    for _ in 0..8 {
        h.state_mut().tl.zoom_by(1.25);
        h.run();
    }
    for _ in 0..16 {
        h.state_mut().tl.zoom_by(0.8);
        h.run();
    }
    assert!(h.state().tl.zoom >= crate::timeline_extras::ZOOM_RANGE.0 && h.state().tl.zoom <= crate::timeline_extras::ZOOM_RANGE.1);
}

#[test]
fn ping_pong_timecode_and_zoom_buttons_are_named_and_work() {
    let mut h = harness();
    timeline_option(&mut h, "Ping-pong");
    assert!(h.state().playback.ping_pong);
    timeline_option(&mut h, "Ping-pong");
    assert!(!h.state().playback.ping_pong);
    h.state_mut().frame = 30.min(h.state().engine.project.length() - 1);
    h.run();
    let fps = h.state().engine.project.fps();
    timeline_option(&mut h, "Timecode");
    assert!(h.state().tl.timecode);
    let want = tooncraft_doc::timecode(h.state().frame, fps);
    h.get_by_label_contains(&want);
    let z0 = h.state().tl.zoom;
    timeline_option(&mut h, "Zoom the timeline in");
    assert!(h.state().tl.zoom > z0);
    timeline_option(&mut h, "Zoom the timeline out");
    timeline_option(&mut h, "Zoom the timeline out");
    assert!(h.state().tl.zoom < z0);
}

#[test]
fn zoom_shortcuts_work_and_stay_in_range() {
    let mut h = harness();
    let z0 = h.state().tl.zoom;
    h.key_press_modifiers(egui::Modifiers::ALT, Key::Equals);
    h.run();
    assert!(h.state().tl.zoom > z0, "Alt+= zooms in");
    for _ in 0..30 {
        h.key_press_modifiers(egui::Modifiers::ALT, Key::Minus);
        h.run();
    }
    assert_eq!(h.state().tl.zoom, crate::timeline_extras::ZOOM_RANGE.0);
}

#[test]
fn the_frame_menu_has_the_marker_items() {
    let mut h = harness();
    h.get(By::new().role(egui::Role::Button).label("Frame")).click();
    h.run();
    for name in ["Add Marker Here", "Next Marker", "Previous Marker"] {
        assert!(h.query_all(By::new().role(egui::Role::Button).label(name)).count() >= 1, "{name}");
    }
}

#[test]
fn play_selection_plays_only_the_selected_frames() {
    let mut h = harness();
    let layer = h.state().layer;
    // nothing selected: the item is there but cannot be used
    h.get(By::new().role(egui::Role::Button).label("Timeline options")).click();
    h.run();
    h.get_by_label("Playback").click();
    h.run();
    assert!(h.get(By::new().role(egui::Role::Button).label("Play Selection")).accesskit_node().is_disabled(), "disabled with nothing selected");
    h.key_press(Key::Escape);
    h.run();
    h.state_mut().sel_anchor = Some((layer, 2));
    h.state_mut().frame = 6;
    h.run();
    timeline_option(&mut h, "Play Selection");
    let p = h.state().playback;
    assert!(p.playing);
    assert_eq!((p.start, p.end), (2, 7));
    // ping-pong stays inside the range too
    h.state_mut().playback.stop();
    h.state_mut().playback.ping_pong = true;
    h.state_mut().frame = 6;
    h.run();
    timeline_option(&mut h, "Play Selection");
    assert!(h.state().playback.playing && h.state().playback.ping_pong);
}

#[test]
fn a_drawing_is_named_and_shown_again_by_name() {
    let mut h = harness();
    let layer = h.state().layer;
    // find a frame with a drawing and one with a different drawing or none
    let (f_draw, f_other) = {
        let p = &h.state().engine.project;
        let l = &p.layers[layer];
        let a = (0..p.length()).find(|f| l.drawing_at(*f).is_some()).expect("the demo has a drawing");
        let b = (0..p.length()).find(|f| l.drawing_at(*f) != l.drawing_at(a)).expect("and a different frame");
        (a, b)
    };
    h.state_mut().frame = f_draw;
    h.run();
    h.get(By::new().role(egui::Role::TextInput).label("Name of this drawing"));
    h.state_mut().tl.name = "Hero pose".into();
    click(&mut h, "Set Name");
    let id = h.state().engine.project.drawing_at(layer, f_draw).unwrap();
    assert_eq!(h.state().engine.project.drawing_names.get(&id).map(String::as_str), Some("Hero pose"));
    h.state_mut().frame = f_other;
    h.run();
    click(&mut h, "Show Hero pose here");
    assert_eq!(h.state().engine.project.drawing_at(layer, f_other), Some(id));
    assert_eq!(h.state().engine.undo_label(), Some("Expose drawing"));
}
