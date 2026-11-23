//! Tests of the shot's drawing library: saving a run of cells, inserting it copied or linked,
//! undo, the bounds, and the round trip through a `.toon` file.

use crate::{Command, Engine, EngineError};
use tooncraft_doc::{Cell, Project};

const RED: [u8; 4] = [200, 20, 20, 255];
const BLUE: [u8; 4] = [20, 20, 200, 255];

fn rgb(c: [u8; 4]) -> Option<[u8; 3]> {
    Some([c[0], c[1], c[2]])
}

fn fill(e: &mut Engine, layer: usize, frame: u32, color: [u8; 4]) {
    e.apply(Command::Fill { layer, frame, x: 4, y: 4, color, options: Default::default(), reference: Default::default() }).unwrap();
}

/// Paint over the drawing at the frame in a new colour (a plain fill goes under paint).
fn repaint(e: &mut Engine, layer: usize, frame: u32, color: [u8; 4]) {
    let options = tooncraft_raster::fill::FillOptions { mode: tooncraft_raster::fill::FillMode::Repaint, ..Default::default() };
    e.apply(Command::Fill { layer, frame, x: 4, y: 4, color, options, reference: Default::default() }).unwrap();
    assert_eq!(colour_at(e, layer, frame), rgb(color), "the repaint took");
}

/// A shot of 12 frames with two layers: on the first, a red drawing on frames 0–1 and a blue one
/// on 2–4, blank after; the second is empty.
fn engine() -> Engine {
    let mut e = Engine::new(Project::new("Library", 32, 32, 12, 12).unwrap());
    e.apply(Command::NewDrawing { layer: 0, frame: 0 }).unwrap();
    fill(&mut e, 0, 0, RED);
    e.apply(Command::SetExposure { layer: 0, frame: 0, length: 2 }).unwrap();
    e.apply(Command::NewDrawing { layer: 0, frame: 2 }).unwrap();
    fill(&mut e, 0, 2, BLUE);
    e.apply(Command::SetExposure { layer: 0, frame: 2, length: 3 }).unwrap();
    e.apply(Command::AddLayer { name: "Two".into(), index: Some(1) }).unwrap();
    e
}

fn colour_at(e: &Engine, layer: usize, frame: u32) -> Option<[u8; 3]> {
    let d = e.project.drawing_at(layer, frame)?;
    rgb(e.project.drawing(d)?.pixel(4, 4))
}

#[test]
fn saving_copies_the_drawings_and_keeps_the_timing() {
    let mut e = engine();
    let drawings = e.project.drawing_count();
    e.apply(Command::SaveToLibrary { layer: 0, from: 1, to: 6, name: " Blink\u{7} ".into() }).unwrap();
    assert_eq!(e.undo_label(), Some("Add to library"));
    let item = e.project.library[0].clone();
    assert_eq!(item.name, "Blink");
    // frame 1 holds the red drawing, so the item starts with it; trailing blanks are dropped
    assert!(matches!(item.cells[..], [Cell::Key(_), Cell::Key(_), Cell::Hold, Cell::Hold]), "{:?}", item.cells);
    assert_eq!(e.project.drawing_count(), drawings + 2, "the item has its own copies");
    let ids = item.drawings();
    assert!(ids.iter().all(|d| e.project.drawing_at(0, 0) != Some(*d) && e.project.drawing_at(0, 2) != Some(*d)));
    // drawing on the layer afterwards leaves the item alone
    repaint(&mut e, 0, 0, BLUE);
    assert_eq!(rgb(e.project.drawing(ids[0]).unwrap().pixel(4, 4)), rgb(RED));
    // the item's drawings count as used: Purge Unused keeps them
    e.apply(Command::PurgeUnused).unwrap();
    assert!(e.project.drawing(ids[0]).is_some());
    e.undo().unwrap(); // purge
    e.undo().unwrap(); // the repaint
    e.undo().unwrap(); // one step takes the item and its drawings away
    assert!(e.project.library.is_empty());
    assert_eq!(e.project.drawing_count(), drawings);
    e.redo().unwrap();
    assert_eq!(e.project.library, vec![item]);
}

#[test]
fn nothing_to_save_is_an_error_and_changes_nothing() {
    let mut e = engine();
    let n = e.undo_len();
    assert!(matches!(e.apply(Command::SaveToLibrary { layer: 0, from: 6, to: 9, name: "x".into() }), Err(EngineError::NoDrawing(6))));
    assert!(e.apply(Command::SaveToLibrary { layer: 0, from: 0, to: 99, name: "x".into() }).is_err());
    assert!(e.apply(Command::SaveToLibrary { layer: 7, from: 0, to: 1, name: "x".into() }).is_err());
    assert_eq!(e.undo_len(), n);
    assert!(e.project.library.is_empty());
}

#[test]
fn inserting_a_copy_is_one_step_and_independent() {
    let mut e = engine();
    e.apply(Command::SaveToLibrary { layer: 0, from: 0, to: 4, name: "Walk".into() }).unwrap();
    let before = e.project.clone();
    let drawings = e.project.drawing_count();
    e.apply(Command::InsertFromLibrary { item: 0, layer: 1, frame: 3, linked: false }).unwrap();
    assert_eq!(e.undo_label(), Some("Insert from library"));
    assert_eq!(e.project.drawing_count(), drawings + 2);
    let got: Vec<_> = (0..9).map(|f| colour_at(&e, 1, f)).collect();
    let (r, b) = (rgb(RED), rgb(BLUE));
    assert_eq!(got, [None, None, None, r, r, b, b, b, None]);
    // the copies are new drawings: painting them leaves the item alone
    let item_first = e.project.library[0].drawings()[0];
    assert_ne!(e.project.drawing_at(1, 3), Some(item_first));
    repaint(&mut e, 1, 3, BLUE);
    assert_eq!(rgb(e.project.drawing(item_first).unwrap().pixel(4, 4)), r);
    e.undo().unwrap();
    e.undo().unwrap();
    assert!(e.project == before, "the insert was one undo step");
}

#[test]
fn a_linked_insert_shares_the_items_drawings() {
    let mut e = engine();
    e.apply(Command::SaveToLibrary { layer: 0, from: 0, to: 1, name: "Prop".into() }).unwrap();
    let drawings = e.project.drawing_count();
    e.apply(Command::InsertFromLibrary { item: 0, layer: 1, frame: 0, linked: true }).unwrap();
    e.apply(Command::InsertFromLibrary { item: 0, layer: 1, frame: 6, linked: true }).unwrap();
    assert_eq!(e.project.drawing_count(), drawings, "no new drawings");
    let d = e.project.library[0].drawings()[0];
    assert_eq!(e.project.drawing_at(1, 0), Some(d));
    assert_eq!(e.project.drawing_at(1, 7), Some(d));
    // editing one use changes the item and the other use
    repaint(&mut e, 1, 0, BLUE);
    assert_eq!(colour_at(&e, 1, 7), rgb(BLUE));
    assert_eq!(rgb(e.project.drawing(d).unwrap().pixel(4, 4)), rgb(BLUE));
}

#[test]
fn a_hold_after_the_insert_keeps_its_drawing_and_the_end_cuts_the_item() {
    let mut e = engine();
    e.apply(Command::SaveToLibrary { layer: 0, from: 0, to: 4, name: "Walk".into() }).unwrap();
    let blue = e.project.drawing_at(0, 2).unwrap();
    e.apply(Command::SetExposure { layer: 0, frame: 2, length: 6 }).unwrap(); // blue on 2..=7
    e.apply(Command::InsertFromLibrary { item: 0, layer: 0, frame: 1, linked: false }).unwrap(); // covers 1..=5
    assert_eq!(e.project.layers[0].cells[6], Cell::Key(blue), "the hold on frame 6 became a key of the drawing it showed");
    assert_eq!(e.project.drawing_at(0, 7), Some(blue));
    // near the end of the shot the item is cut
    e.apply(Command::InsertFromLibrary { item: 0, layer: 1, frame: 10, linked: false }).unwrap();
    assert_eq!(e.project.layers[1].cells.len(), 12);
    assert_eq!(colour_at(&e, 1, 11), rgb(RED));
    assert!(e.apply(Command::InsertFromLibrary { item: 0, layer: 1, frame: 12, linked: false }).is_err());
    assert!(matches!(e.apply(Command::InsertFromLibrary { item: 5, layer: 1, frame: 0, linked: false }), Err(EngineError::NoLibraryItem(5))));
}

#[test]
fn rename_move_and_remove_are_undoable() {
    let mut e = engine();
    e.apply(Command::SaveToLibrary { layer: 0, from: 0, to: 1, name: "A".into() }).unwrap();
    e.apply(Command::SaveToLibrary { layer: 0, from: 2, to: 4, name: "B".into() }).unwrap();
    e.apply(Command::RenameLibraryItem { item: 0, name: "".into() }).unwrap();
    assert_eq!(e.project.library[0].name, "Drawing", "an empty name gets the default");
    e.apply(Command::MoveLibraryItem { item: 0, to: 9 }).unwrap();
    assert_eq!(e.project.library[1].name, "Drawing");
    e.apply(Command::RemoveLibraryItem { item: 0 }).unwrap();
    assert_eq!(e.project.library.len(), 1);
    assert!(e.apply(Command::RemoveLibraryItem { item: 1 }).is_err());
    e.undo().unwrap();
    e.undo().unwrap();
    e.undo().unwrap();
    assert_eq!(e.project.library.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(), ["A", "B"]);
}

#[test]
fn the_library_is_capped() {
    let mut e = engine();
    for i in 0..tooncraft_doc::library::MAX_ITEMS {
        e.apply(Command::SaveToLibrary { layer: 0, from: 0, to: 0, name: format!("{i}") }).unwrap();
    }
    assert!(matches!(e.apply(Command::SaveToLibrary { layer: 0, from: 0, to: 0, name: "one more".into() }), Err(EngineError::Limit(_))));
    assert_eq!(e.project.library.len(), tooncraft_doc::library::MAX_ITEMS);
}

#[test]
fn the_library_round_trips_through_a_file() {
    let mut e = engine();
    e.apply(Command::SaveToLibrary { layer: 0, from: 0, to: 4, name: "Walk".into() }).unwrap();
    let bytes = tooncraft_format::to_bytes(&e.project).unwrap();
    let back = tooncraft_format::from_bytes(&bytes).unwrap();
    assert_eq!(back.library, e.project.library);
    assert!(back == e.project);
}

#[test]
fn a_hostile_library_in_a_file_is_repaired() {
    let mut e = engine();
    e.apply(Command::SaveToLibrary { layer: 0, from: 0, to: 4, name: "Walk".into() }).unwrap();
    // point the item at a drawing that is not in the file, and make it far too long
    e.project.library[0].cells.insert(0, Cell::Key(tooncraft_doc::DrawingId(77_777)));
    e.project.library[0].cells.extend(std::iter::repeat_n(Cell::Hold, tooncraft_doc::library::MAX_ITEM_CELLS * 2));
    let bytes = tooncraft_format::to_bytes(&e.project).unwrap();
    let back = tooncraft_format::from_bytes(&bytes).unwrap();
    let item = &back.library[0];
    assert_eq!(item.cells[0], Cell::Blank, "a key to a missing drawing becomes a blank");
    assert_eq!(item.len(), tooncraft_doc::library::MAX_ITEM_CELLS);
}
