//! Line and colour art in one drawing: a colour layer follows its line layer's exposures through
//! every exposure command, undo and redo, and painting on it makes the paired colour drawing.

use proptest::prelude::*;
use tooncraft_brush::{Brush, BrushKind, Sample};
use tooncraft_doc::Project;
use tooncraft_doc::colour_art::follow;

use crate::{Command, Engine, EngineError};

fn ink() -> Brush {
    Brush { size: 6.0, ..Brush::preset(BrushKind::Ink) }
}

fn dot(layer: usize, frame: u32) -> Command {
    Command::Stroke { layer, frame, brush: ink(), samples: vec![Sample::at(20.0, 20.0, 1.0, 0.0), Sample::at(30.0, 22.0, 1.0, 0.01)] }
}

/// A shot whose layer 1 is the line layer and layer 0 its colour layer.
fn shot() -> Engine {
    let mut e = Engine::new(Project::new("Colour", 64, 64, 12, 12).unwrap());
    e.apply(Command::AddColourLayer { layer: 0 }).unwrap();
    e
}

/// The same shot with an empty history (so undo cannot take the colour layer away).
fn fresh() -> Engine {
    Engine::new(shot().project)
}

fn in_step(e: &Engine) -> bool {
    let (c, l) = (&e.project.layers[0], &e.project.layers[1]);
    c.cells == follow(&l.cells, &c.colour_pairs)
}

#[test]
fn the_colour_layer_follows_new_drawings_holds_and_deletions() {
    let mut e = shot();
    assert_eq!(e.project.layers[0].name, "Layer 1 colour");
    assert_eq!(e.project.line_layer_of(0), Some(1));
    assert_eq!(e.undo_label(), Some("Add colour layer"));
    // colour needs line art at the frame
    assert!(matches!(e.apply(dot(0, 0)), Err(EngineError::NoLineArt(1))));
    e.apply(dot(1, 0)).unwrap();
    e.apply(dot(0, 0)).unwrap();
    let c0 = e.project.drawing_at(0, 0).unwrap();
    assert_ne!(Some(c0), e.project.drawing_at(1, 0));
    // holding the line drawing holds its colour
    e.apply(Command::SetExposure { layer: 1, frame: 0, length: 4 }).unwrap();
    assert_eq!((0..4).map(|f| e.project.drawing_at(0, f)).collect::<Vec<_>>(), vec![Some(c0); 4]);
    assert_eq!(e.project.drawing_at(0, 4), None);
    // a new line drawing has no colour yet
    e.apply(Command::NewDrawing { layer: 1, frame: 2 }).unwrap();
    assert_eq!(e.project.drawing_at(0, 2), None);
    assert_eq!(e.project.drawing_at(0, 1), Some(c0));
    // deleting a frame on the line layer moves the colour with it
    e.apply(Command::DeleteFrame { layer: Some(1), frame: 0 }).unwrap();
    assert_eq!(e.project.drawing_at(0, 0), Some(c0));
    assert_eq!(e.project.drawing_at(0, 1), None);
    assert!(in_step(&e));
    // undo puts both back
    e.undo().unwrap();
    assert_eq!(e.project.drawing_at(0, 1), Some(c0));
    assert!(in_step(&e));
    // saves round-trip with the link and the pairs
    let bytes = tooncraft_format::to_bytes(&e.project).unwrap();
    let back = tooncraft_format::from_bytes(&bytes).unwrap();
    assert_eq!(back.layers[0].colour_of, e.project.layers[0].colour_of);
    assert_eq!(back.layers[0].colour_pairs, e.project.layers[0].colour_pairs);
    // unlinking keeps the cells and stops following
    e.apply(Command::SetColourOf { layer: 0, line: None }).unwrap();
    e.apply(Command::DeleteFrame { layer: Some(1), frame: 0 }).unwrap();
    assert_eq!(e.project.drawing_at(0, 0), Some(c0), "no longer follows");
}

#[test]
fn links_that_make_no_sense_are_refused() {
    let mut e = shot();
    assert!(e.apply(Command::AddColourLayer { layer: 0 }).is_err(), "a colour layer of a colour layer");
    assert!(e.apply(Command::SetColourOf { layer: 1, line: Some(1) }).is_err(), "a layer colouring itself");
    assert!(e.apply(Command::SetColourOf { layer: 1, line: Some(0) }).is_err(), "following a colour layer");
    assert!(e.apply(Command::SetColourOf { layer: 1, line: Some(9) }).is_err());
    // deleting the line layer unlinks the colour layer
    e.apply(Command::DeleteLayer { layer: 1 }).unwrap();
    assert_eq!(e.project.layers[0].colour_of, None);
}

#[test]
fn old_files_without_colour_layers_are_unchanged() {
    let mut e = Engine::new(Project::new("Old", 32, 32, 12, 4).unwrap());
    e.apply(dot(0, 0)).unwrap();
    let json = serde_json::to_string(&e.project.layers[0]).unwrap();
    assert!(!json.contains("colour_of") && !json.contains("colour_pairs"), "{json}");
}

#[derive(Clone, Debug)]
enum Op {
    Paint(usize, u32),
    New(u32),
    Extend(u32),
    Shorten(u32),
    Expose(u32, u32),
    Insert(u32),
    Delete(u32),
    Clear(u32),
    Undo,
    Redo,
}

fn op() -> impl Strategy<Value = Op> {
    let f = || 0u32..12;
    prop_oneof![
        (0usize..2, f()).prop_map(|(l, x)| Op::Paint(l, x)),
        f().prop_map(Op::New),
        f().prop_map(Op::Extend),
        f().prop_map(Op::Shorten),
        (f(), 1u32..5).prop_map(|(x, n)| Op::Expose(x, n)),
        f().prop_map(Op::Insert),
        f().prop_map(Op::Delete),
        f().prop_map(Op::Clear),
        Just(Op::Undo),
        Just(Op::Redo),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]
    /// Whatever exposure commands run on the line layer (and paint on either layer, undo, redo),
    /// the colour layer shows, at every frame, the colour of the line drawing shown there.
    #[test]
    fn every_exposure_command_keeps_the_pair_in_step(ops in proptest::collection::vec(op(), 1..40)) {
        let mut e = fresh();
        for o in ops {
            // refusals (no line art, a frame past the end, nothing to undo) are fine; drift is not
            let _ = match o {
                Op::Paint(l, f) => e.apply(dot(l, f)),
                Op::New(f) => e.apply(Command::NewDrawing { layer: 1, frame: f }),
                Op::Extend(f) => e.apply(Command::ExtendHold { layer: 1, frame: f }),
                Op::Shorten(f) => e.apply(Command::ShortenHold { layer: 1, frame: f }),
                Op::Expose(f, n) => e.apply(Command::SetExposure { layer: 1, frame: f, length: n }),
                Op::Insert(f) => e.apply(Command::InsertFrame { layer: Some(1), frame: f }),
                Op::Delete(f) => e.apply(Command::DeleteFrame { layer: Some(1), frame: f }),
                Op::Clear(f) => e.apply(Command::DeleteFrame { layer: None, frame: f }),
                Op::Undo => e.undo(),
                Op::Redo => e.redo(),
            };
            prop_assert!(in_step(&e), "after {:?}", o);
            for f in 0..e.project.length() {
                let line = e.project.drawing_at(1, f);
                let colour = e.project.drawing_at(0, f);
                prop_assert!(line.is_some() || colour.is_none(), "colour without line art at frame {}", f);
            }
        }
    }
}
