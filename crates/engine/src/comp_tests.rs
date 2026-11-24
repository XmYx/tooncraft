//! The compositing graph through engine commands: built from the layers it gives the same frame,
//! edits are undoable, and it is saved with the shot.

use tooncraft_doc::comp::NodeKind;
use tooncraft_doc::{Project, RenderOptions, render_frame};

use crate::{Command, CompEdit, Engine, EngineError};

fn stroke(e: &mut Engine, layer: usize, y: f32, colour: [u8; 4]) {
    let brush = tooncraft_brush::Brush { size: 8.0, color: colour, ..Default::default() };
    let samples = (0..=20)
        .map(|i| tooncraft_brush::Sample { x: 5.0 + 2.5 * i as f32, y, pressure: 1.0, tilt_x: 0.0, tilt_y: 0.0, time: i as f64 * 0.01, rotation: 0.0 })
        .collect();
    e.apply(Command::Stroke { layer, frame: 0, brush, samples }).unwrap();
}

fn shot() -> Engine {
    let mut e = Engine::new(Project::new("c", 64, 48, 24, 4).unwrap());
    stroke(&mut e, 0, 20.0, [200, 0, 0, 255]);
    e.apply(Command::AddLayer { name: "Top".into(), index: None }).unwrap();
    stroke(&mut e, 1, 24.0, [0, 0, 200, 255]);
    e
}

/// The largest difference of any channel (compositing over transparency first, then over the
/// paper, rounds a little differently from stacking straight onto the paper).
fn diff(a: &tooncraft_raster::RgbaImage, b: &tooncraft_raster::RgbaImage) -> u8 {
    a.data.iter().zip(&b.data).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0)
}

fn edit(e: &mut Engine, edit: CompEdit) -> Result<(), EngineError> {
    e.apply(Command::EditComp { edit })
}

/// A guard that removes a test folder when the test ends, also when it fails.
struct Dir(std::path::PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_graph_from_the_layers_matches_the_stack_and_rewires_it() {
    let mut e = shot();
    let start = e.project.clone();
    let stack = render_frame(&e.project, 0, RenderOptions::default());
    edit(&mut e, CompEdit::FromLayers).unwrap();
    assert!(e.project.comp.active());
    assert!(diff(&render_frame(&e.project, 0, RenderOptions::default()), &stack) <= 2, "the same frame as the layer stack");
    // swap the order: the bottom layer over the top one
    let blend = e.project.comp.nodes.iter().find(|n| matches!(n.kind, NodeKind::Blend { .. })).map(|n| n.id).unwrap();
    let (a, b) = (e.project.comp.source(blend, 0).unwrap(), e.project.comp.source(blend, 1).unwrap());
    edit(&mut e, CompEdit::Link { from: b, to: blend, input: 0 }).unwrap();
    edit(&mut e, CompEdit::Link { from: a, to: blend, input: 1 }).unwrap();
    let swapped = render_frame(&e.project, 0, RenderOptions::default());
    assert!(diff(&swapped, &stack) > 50, "the red line now lies over the blue one where they cross");
    // an effect node after the output's source changes the frame; a loop is refused
    let out = e.project.comp.nodes.iter().find(|n| n.kind == NodeKind::Output).map(|n| n.id).unwrap();
    edit(
        &mut e,
        CompEdit::AddNode { kind: NodeKind::Effect { effect: tooncraft_fx::Effect::new(tooncraft_fx::EffectKind::Blur { radius: 4.0 }) }, pos: [300.0, 0.0] },
    )
    .unwrap();
    let fx = e.project.comp.nodes.iter().map(|n| n.id).max().unwrap();
    edit(&mut e, CompEdit::Link { from: blend, to: fx, input: 0 }).unwrap();
    edit(&mut e, CompEdit::Link { from: fx, to: out, input: 0 }).unwrap();
    assert!(matches!(edit(&mut e, CompEdit::Link { from: fx, to: blend, input: 0 }), Err(EngineError::Comp(_))), "a loop");
    assert_ne!(render_frame(&e.project, 0, RenderOptions::default()), swapped);
    // the fill and single-layer pictures still read the layer stack
    let one = render_frame(&e.project, 0, RenderOptions { only_layer: Some(0), ..Default::default() });
    assert_eq!(one, render_frame(&start, 0, RenderOptions { only_layer: Some(0), ..Default::default() }));
    edit(&mut e, CompEdit::SetEnabled { enabled: false }).unwrap();
    assert_eq!(render_frame(&e.project, 0, RenderOptions::default()), stack, "off: the layer stack");
    while e.can_undo() && e.project != start {
        e.undo().unwrap();
    }
    assert_eq!(e.project, start, "every graph edit undoes");
}

#[test]
fn the_graph_is_saved_with_the_shot() {
    let mut e = shot();
    edit(&mut e, CompEdit::FromLayers).unwrap();
    edit(&mut e, CompEdit::MoveNode { node: 1, pos: [12.0, 34.0] }).unwrap();
    let dir = Dir(std::env::temp_dir().join(format!("tooncraft-comp-{}", std::process::id())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let path = dir.0.join("comp.toon");
    tooncraft_format::save(&path, &e.project).unwrap();
    let back = tooncraft_format::load(&path).unwrap();
    assert_eq!(back.comp, e.project.comp);
    assert_eq!(render_frame(&back, 0, RenderOptions::default()), render_frame(&e.project, 0, RenderOptions::default()));
}
