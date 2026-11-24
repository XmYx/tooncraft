//! Engine support for the right-click menus: `Command::Batch` (several commands, one undo step,
//! all or nothing) and selecting the pixels of one colour.

use super::*;

fn engine() -> Engine {
    Engine::new(Project::new("Test", 64, 48, 12, 6).unwrap())
}

#[test]
fn a_batch_is_one_undo_step() {
    let mut e = engine();
    let layers = e.project.layers.len();
    let batch = Command::Batch {
        label: "Place as new layer".into(),
        commands: vec![Command::AddLayer { name: "New".into(), index: Some(layers) }, Command::NewDrawing { layer: layers, frame: 2 }],
    };
    e.apply(batch).unwrap();
    assert_eq!(e.project.layers.len(), layers + 1);
    assert!(e.project.drawing_at(layers, 2).is_some());
    assert_eq!(e.undo_label(), Some("Place as new layer"));
    e.undo().unwrap();
    assert_eq!(e.project.layers.len(), layers);
    assert!(!e.can_undo());
    e.redo().unwrap();
    assert!(e.project.drawing_at(layers, 2).is_some());
}

#[test]
fn a_failing_batch_leaves_the_project_and_redo_as_they_were() {
    let mut e = engine();
    e.apply(Command::NewDrawing { layer: 0, frame: 0 }).unwrap();
    e.apply(Command::ExtendHold { layer: 0, frame: 0 }).unwrap();
    e.undo().unwrap();
    let cells = e.project.layers[0].cells.clone();
    let layers = e.project.layers.len();
    let batch = Command::Batch {
        label: "Broken".into(),
        commands: vec![Command::AddLayer { name: "New".into(), index: None }, Command::NewDrawing { layer: 99, frame: 0 }],
    };
    assert!(e.apply(batch).is_err());
    assert_eq!(e.project.layers.len(), layers);
    assert_eq!(e.project.layers[0].cells, cells);
    // the hold that was undone before can still be redone
    assert!(e.can_redo());
    assert_eq!(e.undo_len(), 1);
}

#[test]
fn an_empty_or_settings_only_batch_keeps_the_previous_label() {
    let mut e = engine();
    e.apply(Command::NewDrawing { layer: 0, frame: 0 }).unwrap();
    let label = e.undo_label().map(str::to_string);
    e.apply(Command::Batch { label: "Nothing".into(), commands: vec![] }).unwrap();
    e.apply(Command::Batch { label: "Fps".into(), commands: vec![Command::SetOnion { onion: e.project.onion }] }).unwrap();
    assert_eq!(e.undo_label().map(str::to_string), label);
}

#[test]
fn hostile_batches_are_refused() {
    let mut e = engine();
    let nested = Command::Batch { label: "Outer".into(), commands: vec![Command::Batch { label: "Inner".into(), commands: vec![] }] };
    assert!(e.apply(nested).is_err());
    let huge = Command::Batch { label: "Huge".into(), commands: vec![Command::PurgeUnused; MAX_BATCH + 1] };
    assert!(e.apply(huge).is_err());
    assert!(!e.can_undo());
}

#[test]
fn colour_mask_selects_the_pixels_of_one_colour() {
    let mut e = engine();
    e.apply(Command::NewDrawing { layer: 0, frame: 0 }).unwrap();
    let mut rgba = Vec::new();
    for i in 0..16 {
        rgba.extend_from_slice(if i < 12 { &[200, 30, 20, 255] } else { &[20, 30, 200, 128] });
    }
    e.apply(Command::PastePixels { layer: 0, frame: 0, x: 8.0, y: 4.0, width: 4, height: 4, rgba }).unwrap();
    let red = crate::select::colour_mask(&e, 0, 0, [200, 30, 20], 2).unwrap();
    assert_eq!(red.count(), 12);
    // half-clear pixels count by their colour, not their opacity
    let blue = crate::select::colour_mask(&e, 0, 0, [20, 30, 200], 4).unwrap();
    assert_eq!(blue.count(), 4);
    assert!(crate::select::colour_mask(&e, 0, 0, [0, 255, 0], 10).is_none());
    // another frame, or a layer that does not exist, has nothing of it
    assert!(crate::select::colour_mask(&e, 0, 3, [200, 30, 20], 2).is_none());
    assert!(crate::select::colour_mask(&e, 7, 0, [200, 30, 20], 2).is_none());
}
