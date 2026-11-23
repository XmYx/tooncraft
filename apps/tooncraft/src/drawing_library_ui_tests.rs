//! UI tests for the Drawing Library panel: add the selected frames from the keyboard, insert an
//! item copied and linked, rename, reorder and remove. The accessibility check is on.

use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use tooncraft_doc::Cell;
use tooncraft_engine::Command;

use crate::app::ToonApp;
use crate::args::Cli;
use crate::dock::Pane;

/// A blank shot with a drawing on frames 1–2 and another on 3, and the Drawing Library in front.
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
    let app = h.state_mut();
    assert!(app.run(Command::NewDrawing { layer: 0, frame: 0 }));
    assert!(app.run(Command::SetExposure { layer: 0, frame: 0, length: 2 }));
    assert!(app.run(Command::NewDrawing { layer: 0, frame: 2 }));
    app.workspace.raise(Pane::Drawings);
    h.run();
    h.run();
    h
}

fn click(h: &mut Harness<'static, ToonApp>, label: &str) {
    h.get_by_label(label).scroll_to_me();
    h.run();
    h.get_by_label(label).click();
    h.run();
}

#[test]
fn frames_are_added_and_inserted_from_the_keyboard() {
    let mut h = harness();
    // select frames 1–3 of the layer
    h.state_mut().sel_anchor = Some((0, 0));
    h.state_mut().frame = 2;
    h.run();
    h.get_by_label("New item").focus();
    h.run();
    h.get_by_label("New item").type_text("Blink");
    h.run();
    h.key_press(Key::Tab);
    h.run();
    assert!(h.get_by_label("Add Frames to Library").is_focused(), "Tab moves from the name to the Add button");
    h.key_press(Key::Enter);
    h.run();
    let lib = &h.state().engine.project.library;
    assert_eq!(lib.len(), 1);
    assert_eq!(lib[0].name, "Blink");
    assert!(matches!(lib[0].cells[..], [Cell::Key(_), Cell::Hold, Cell::Key(_)]), "{:?}", lib[0].cells);
    assert_eq!(h.state().engine.undo_label(), Some("Add to library"));
    h.get_by_label("Blink (3 frames)");
    // move the cursor and insert with the keyboard: focus Insert, press Space
    h.state_mut().sel_anchor = None;
    h.state_mut().frame = 6;
    h.run();
    h.get_by_label("Insert Item").focus();
    h.run();
    h.key_press(Key::Space);
    h.run();
    let p = &h.state().engine.project;
    assert!(p.drawing_at(0, 6).is_some() && p.drawing_at(0, 8).is_some());
    assert_ne!(p.drawing_at(0, 6), p.library[0].drawings().first().copied(), "Insert makes new drawings");
    assert_eq!(h.state().engine.undo_label(), Some("Insert from library"));
}

#[test]
fn linked_insert_rename_move_and_remove() {
    let mut h = harness();
    // no selection: Add takes the exposure at the cursor (frames 1–2)
    click(&mut h, "Add Frames to Library");
    h.state_mut().frame = 2;
    h.run();
    click(&mut h, "Add Frames to Library");
    assert_eq!(h.state().engine.project.library.iter().map(|i| i.len()).collect::<Vec<_>>(), [2, 1]);
    h.get_by_label("Drawing 1 (2 frames)");
    h.get_by_label("Drawing 2 (1 frames)");
    // the second is chosen after adding it; insert it linked at frame 10
    h.state_mut().frame = 9;
    h.run();
    click(&mut h, "Insert Item Linked");
    let p = &h.state().engine.project;
    assert_eq!(p.drawing_at(0, 9), p.library[1].drawings().first().copied(), "a linked insert shows the item's drawing");
    // rename from the keyboard: Enter confirms
    click(&mut h, "Rename Item");
    h.run();
    h.get_by_label("New name").focus();
    h.run();
    h.key_press(Key::End);
    for _ in 0.."Drawing 2".len() {
        h.key_press(Key::Backspace);
    }
    h.run();
    h.get_by_label("New name").type_text("Prop");
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(h.state().engine.project.library[1].name, "Prop");
    click(&mut h, "Move Item Up");
    assert_eq!(h.state().engine.project.library[0].name, "Prop");
    click(&mut h, "Remove from Library");
    assert_eq!(h.state().engine.project.library.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(), ["Drawing 1"]);
    // one undo brings it back
    assert!(h.state_mut().engine.undo().is_ok());
    h.run();
    assert_eq!(h.state().engine.project.library.len(), 2);
}

#[test]
fn an_empty_frame_cannot_be_added() {
    let mut h = harness();
    h.state_mut().frame = 20;
    h.run();
    h.get_by_label("No drawings yet: select frames on the timeline and add them.");
    click(&mut h, "Add Frames to Library");
    assert!(h.state().engine.project.library.is_empty(), "the button is disabled on a blank frame");
}
