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
    assert!(h.get_by_label("New name").is_focused(), "the field takes the focus when it opens");
    // Tab leaves the field (it does not pull the focus back), and focusing it again works
    h.key_press(Key::Tab);
    h.run();
    h.run();
    assert!(!h.get_by_label("New name").is_focused(), "Tab moves the focus on");
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

fn temp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("tooncraft-mydrawings-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn my_drawings_keep_an_item_for_other_shots() {
    let dir = temp("keep");
    let mut h = harness();
    crate::drawing_library::set_folder(&h.ctx, &dir);
    click(&mut h, "Add Frames to Library");
    // open My Drawings from the keyboard
    h.get_by_label("My Drawings").focus();
    h.run();
    h.key_press(Key::Space);
    h.run();
    h.get_by_label("No drawing files yet: save an item here to use it in other shots.");
    click(&mut h, "Save Item to My Drawings");
    let file = dir.join("Drawing 1.toondrawings");
    assert!(file.is_file(), "saved into the user's folder");
    h.run();
    h.get_by_label("File Drawing 1");
    // saving again does not overwrite: a second name
    click(&mut h, "Save Item to My Drawings");
    assert!(dir.join("Drawing 1 2.toondrawings").is_file());
    // a new shot gets it back from the folder
    h.state_mut().engine = tooncraft_engine::Engine::new(tooncraft_doc::Project::new("Next", 320, 240, 12, 24).unwrap());
    h.run();
    click(&mut h, "File Drawing 1");
    click(&mut h, "Add File to Shot Library");
    let p = &h.state().engine.project;
    assert_eq!(p.library.len(), 1);
    assert_eq!(p.library[0].name, "Drawing 1");
    assert_eq!(p.library[0].len(), 2, "the exposure of two frames came along");
    assert_eq!(h.state().engine.undo_label(), Some("Add drawing file to library"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_crowded_folder_is_listed_up_to_the_cap() {
    let dir = temp("crowded");
    for i in 0..620 {
        std::fs::write(dir.join(format!("d{i:04}.toondrawings")), b"x").unwrap();
    }
    std::fs::write(dir.join("notes.txt"), b"x").unwrap();
    let mut h = harness();
    crate::drawing_library::set_folder(&h.ctx, &dir);
    h.run();
    click(&mut h, "My Drawings");
    h.get_by_label("File d0000");
    h.get_by_label("File d0499");
    assert!(h.query_by_label("File d0500").is_none(), "at most 500 files are listed");
    assert!(h.query_by_label("File notes").is_none(), "only drawing files");
    // a damaged file says so and changes nothing
    click(&mut h, "File d0000");
    click(&mut h, "Add File to Shot Library");
    assert!(h.state().engine.project.library.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
