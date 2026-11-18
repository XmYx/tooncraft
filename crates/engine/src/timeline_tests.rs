//! Timeline polish: frame markers (commands, undo, saving) and timecode through the project.

use super::*;
use tooncraft_doc::timecode;

fn engine() -> Engine {
    Engine::new(Project::new("T", 64, 64, 24, 30).unwrap())
}

#[test]
fn markers_are_commands_with_undo_and_survive_saving() {
    let mut e = engine();
    let before = e.project.clone();
    e.apply(Command::SetMarker { frame: 4, text: "anticipation".into() }).unwrap();
    e.apply(Command::SetMarker { frame: 12, text: "contact".into() }).unwrap();
    e.apply(Command::SetMarker { frame: 4, text: "pose 1".into() }).unwrap();
    assert_eq!(e.project.markers.len(), 2);
    assert_eq!(e.project.markers.get(&4).map(String::as_str), Some("pose 1"));
    assert_eq!(e.undo_label(), Some("Marker"));
    let bytes = tooncraft_format::to_bytes(&e.project).unwrap();
    let back = tooncraft_format::from_bytes(&bytes).unwrap();
    assert_eq!(back.markers, e.project.markers);
    assert_eq!(back, e.project);
    e.apply(Command::RemoveMarker { frame: 12 }).unwrap();
    assert_eq!(e.project.markers.len(), 1);
    e.apply(Command::RemoveMarker { frame: 12 }).unwrap();
    let depth = e.undo_len();
    e.apply(Command::SetMarker { frame: 4, text: String::new() }).unwrap();
    assert!(e.project.markers.is_empty(), "an empty note removes it");
    assert_eq!(e.undo_len(), depth + 1);
    while e.undo().is_ok() {}
    assert_eq!(e.project, before);
    e.redo().unwrap();
    assert_eq!(e.project.markers.len(), 1);
}

#[test]
fn bad_markers_are_refused_without_a_trace() {
    let mut e = engine();
    let steps = e.undo_len();
    assert!(e.apply(Command::SetMarker { frame: 30, text: "past the end".into() }).is_err());
    assert!(e.apply(Command::SetMarker { frame: u32::MAX, text: "x".into() }).is_err());
    assert_eq!(e.undo_len(), steps);
    assert!(e.project.markers.is_empty());
    for f in 0..tooncraft_doc::MAX_MARKERS as u32 {
        e.project.markers.insert(f, "x".into());
    }
    // a full table refuses a new frame but still edits an existing one
    let mut long = Engine::new(Project::new("L", 8, 8, 24, tooncraft_doc::MAX_FRAMES).unwrap());
    long.project.markers = e.project.markers.clone();
    assert!(long.apply(Command::SetMarker { frame: 5000, text: "one too many".into() }).is_err());
    assert!(long.apply(Command::SetMarker { frame: 7, text: "edited".into() }).is_ok());
}

#[test]
fn hostile_marker_files_are_cut_down_on_load() {
    let mut e = engine();
    e.apply(Command::SetMarker { frame: 2, text: "ok".into() }).unwrap();
    // a huge note, and a marker past the end
    let mut p = e.project.clone();
    p.markers.insert(29, "x".repeat(10_000));
    let back = tooncraft_format::from_bytes(&tooncraft_format::to_bytes(&p).unwrap()).unwrap();
    assert!(back.markers.get(&29).is_some_and(|t| t.chars().count() <= tooncraft_doc::MAX_MARKER_TEXT));
    p.markers.insert(500, "past the end".into());
    let back = tooncraft_format::from_bytes(&tooncraft_format::to_bytes(&p).unwrap()).unwrap();
    assert!(!back.markers.contains_key(&500), "markers past the end are not loaded");
}

#[test]
fn marker_commands_come_from_command_files() {
    let mut e = engine();
    for c in [r#"{"cmd":"set_marker","frame":3,"text":"hit"}"#, r#"{"cmd":"set_marker","frame":9,"text":"settle"}"#, r#"{"cmd":"remove_marker","frame":3}"#] {
        e.apply(serde_json::from_str(c).unwrap()).unwrap();
    }
    assert_eq!(e.project.markers.keys().copied().collect::<Vec<_>>(), vec![9]);
    assert_eq!(timecode(9, e.project.fps()), "00:00:00:09");
}

fn brush() -> Brush {
    Brush { size: 6.0, ..Brush::preset(tooncraft_brush::BrushKind::Ink) }
}

fn dot(e: &mut Engine, layer: usize, frame: u32, x: f32) {
    e.apply(Command::Stroke { layer, frame, brush: brush(), samples: vec![tooncraft_brush::Sample::at(x, 20.0, 1.0, 0.0)] }).unwrap();
}

#[test]
fn a_named_drawing_is_exposed_again_one_frame_at_a_time() {
    let mut e = engine();
    dot(&mut e, 0, 0, 10.0); // frame 0: drawing A, held below
    e.apply(Command::ExtendHold { layer: 0, frame: 0 }).unwrap();
    e.apply(Command::ExtendHold { layer: 0, frame: 1 }).unwrap();
    e.apply(Command::ExtendHold { layer: 0, frame: 2 }).unwrap();
    dot(&mut e, 0, 6, 40.0); // frame 6: drawing B
    e.apply(Command::SetDrawingName { layer: 0, frame: 6, name: "Blink".into() }).unwrap();
    let a = e.project.drawing_at(0, 0).unwrap();
    let b = e.project.drawing_at(0, 6).unwrap();
    assert_eq!(sheet(&e.project.layers[0].cells).chars().take(8).collect::<String>(), "#---..#.");
    // substitute B into the middle of A's hold: only frame 2 changes
    e.apply(Command::ExposeNamed { layer: 0, frame: 2, name: "blink".into() }).unwrap();
    let shows: Vec<_> = (0..4).map(|f| e.project.drawing_at(0, f)).collect();
    assert_eq!(shows, vec![Some(a), Some(a), Some(b), Some(a)]);
    assert_eq!(e.undo_label(), Some("Expose drawing"));
    // and on another layer
    e.apply(Command::AddLayer { name: "Other".into(), index: None }).unwrap();
    e.apply(Command::ExposeNamed { layer: 1, frame: 4, name: "BLINK".into() }).unwrap();
    assert_eq!(e.project.drawing_at(1, 4), Some(b));
    assert_eq!(e.project.drawing_at(1, 5), None, "a blank frame stays blank after the substitution");
    // refusals leave no trace
    let steps = e.undo_len();
    assert!(e.apply(Command::ExposeNamed { layer: 0, frame: 1, name: "nobody".into() }).is_err());
    assert!(e.apply(Command::SetDrawingName { layer: 0, frame: 0, name: "blink".into() }).is_err(), "names are unique");
    assert!(e.apply(Command::SetDrawingName { layer: 0, frame: 9, name: "blank frame".into() }).is_err());
    e.apply(Command::SetLayer { layer: 1, visible: None, locked: Some(true), opacity: None }).unwrap();
    assert!(e.apply(Command::ExposeNamed { layer: 1, frame: 1, name: "blink".into() }).is_err(), "locked layers refuse");
    assert_eq!(e.undo_len(), steps + 1);
    // names are saved
    let back = tooncraft_format::from_bytes(&tooncraft_format::to_bytes(&e.project).unwrap()).unwrap();
    assert_eq!(back.drawing_by_name("Blink"), Some(b));
    assert_eq!(back, e.project);
    while e.undo().is_ok() {}
    assert!(e.project.drawing_names.is_empty());
}

#[test]
fn naming_and_exposing_work_from_command_files() {
    let mut e = engine();
    dot(&mut e, 0, 0, 12.0);
    for c in [r#"{"cmd":"set_drawing_name","layer":0,"frame":0,"name":"Open"}"#, r#"{"cmd":"expose_named","layer":0,"frame":5,"name":"open"}"#] {
        e.apply(serde_json::from_str(c).unwrap()).unwrap();
    }
    assert_eq!(e.project.drawing_at(0, 5), e.project.drawing_at(0, 0));
}
