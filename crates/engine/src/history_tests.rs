//! The history list and jumping through it.

use super::*;

fn engine() -> Engine {
    Engine::new(Project::new("H", 32, 32, 12, 4).unwrap())
}

#[test]
fn history_lists_steps_and_jumping_lands_on_the_exact_state() {
    let mut e = engine();
    let mut states = vec![e.project.clone()];
    for name in ["A", "B", "C", "D"] {
        e.apply(Command::AddLayer { name: name.into(), index: None }).unwrap();
        states.push(e.project.clone());
    }
    let (done, undone) = e.history();
    assert_eq!(done, vec!["Add layer"; 4]);
    assert!(undone.is_empty());
    // back to the second step
    assert_eq!(e.go_to_step(2).unwrap(), 2);
    assert_eq!(e.project, states[2]);
    let (done, undone) = e.history();
    assert_eq!((done.len(), undone.len()), (2, 2));
    // forward to the last, past the end (clamped), then all the way back
    assert_eq!(e.go_to_step(99).unwrap(), 4);
    assert_eq!(e.project, states[4]);
    assert_eq!(e.go_to_step(0).unwrap(), 0);
    assert_eq!(e.project, states[0]);
    // a new change after jumping back drops the steps that were undone
    e.go_to_step(2).unwrap();
    e.apply(Command::AddLayer { name: "X".into(), index: None }).unwrap();
    assert_eq!(e.history().1.len(), 0);
    assert_eq!(e.undo_len(), 3);
}

#[test]
fn history_order_matches_what_undo_and_redo_do() {
    let mut e = engine();
    e.apply(Command::AddLayer { name: "A".into(), index: None }).unwrap();
    e.apply(Command::RenameLayer { layer: 1, name: "B".into() }).unwrap();
    e.apply(Command::SetLayerBlend { layer: 1, blend: tooncraft_doc::BlendMode::Screen }).unwrap();
    e.undo().unwrap();
    e.undo().unwrap();
    let (done, undone) = e.history();
    assert_eq!(done, vec!["Add layer"]);
    assert_eq!(undone, vec!["Rename layer", "Blend mode"], "the next redo comes first");
    assert_eq!(e.redo_label(), Some("Rename layer"));
}

#[test]
fn jumping_is_refused_while_drawing() {
    let mut e = engine();
    e.apply(Command::AddLayer { name: "A".into(), index: None }).unwrap();
    e.begin_stroke(0, 0, Brush::default()).unwrap();
    assert_eq!(e.go_to_step(0), Err(EngineError::Busy));
    assert_eq!(e.undo_len(), 1);
    e.cancel_stroke();
    assert_eq!(e.go_to_step(0).unwrap(), 0);
}

#[test]
fn the_library_offers_blur_brushes_that_only_soften() {
    let blurs: Vec<_> = library::builtin().into_iter().filter(|(n, _, _)| n.to_lowercase().contains("blur")).collect();
    assert!(blurs.len() >= 2, "Blur and Fine blur");
    for (_, category, b) in blurs {
        assert_eq!(category, "Smudges");
        assert_eq!(b.kind, tooncraft_brush::BrushKind::Smudge);
        assert_eq!(b.wet.pickup, 1.0);
    }
}
