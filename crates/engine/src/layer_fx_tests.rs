//! Painted layer masks and adjustment layers through engine commands.

use tooncraft_doc::{Project, RenderOptions, render_frame};
use tooncraft_fx::{Effect, EffectKind};

use crate::{Command, Engine, EngineError};

fn stroke(e: &mut Engine, layer: usize, x0: f32, x1: f32, y: f32) {
    let brush = tooncraft_brush::Brush { size: 6.0, color: [0, 0, 0, 255], ..Default::default() };
    let samples = (0..=20)
        .map(|i| tooncraft_brush::Sample {
            x: x0 + (x1 - x0) * i as f32 / 20.0,
            y,
            pressure: 1.0,
            tilt_x: 0.0,
            tilt_y: 0.0,
            time: i as f64 * 0.01,
            rotation: 0.0,
        })
        .collect();
    e.apply(Command::Stroke { layer, frame: 0, brush, samples }).unwrap();
}

fn plain() -> RenderOptions {
    RenderOptions { paper: false, ..Default::default() }
}

#[test]
fn a_painted_mask_hides_the_layer_and_undoes() {
    let mut e = Engine::new(Project::new("m", 64, 48, 24, 4).unwrap());
    stroke(&mut e, 0, 5.0, 60.0, 20.0);
    let start = e.project.clone();
    assert!(render_frame(&e.project, 0, plain()).pixel(30, 20)[3] > 200);
    assert_eq!(e.apply(Command::SetLayerMask { layer: 0, enabled: Some(false), invert: None, editing: None }), Err(EngineError::NoMask));
    e.apply(Command::AddLayerMask { layer: 0, invert: false }).unwrap();
    assert_eq!(e.apply(Command::AddLayerMask { layer: 0, invert: false }), Err(EngineError::HasMask));
    let drawing = e.project.layers[0].drawing_at(0);
    // painting now goes into the mask: a stroke across x 25…35 hides the line there
    stroke(&mut e, 0, 28.0, 32.0, 20.0);
    assert_eq!(e.project.layers[0].drawing_at(0), drawing, "the drawing itself is untouched");
    let m = e.project.layers[0].mask.unwrap();
    assert!(e.project.drawing(m.drawing).is_some_and(|s| !s.is_empty()), "the paint went into the mask");
    let masked = render_frame(&e.project, 0, plain());
    assert!(masked.pixel(30, 20)[3] < 40, "hidden under the mask paint: {:?}", masked.pixel(30, 20));
    assert!(masked.pixel(10, 20)[3] > 200, "shown elsewhere");
    // inverted: only the mask paint shows; off: the mask does nothing
    e.apply(Command::SetLayerMask { layer: 0, enabled: None, invert: Some(true), editing: Some(false) }).unwrap();
    let inv = render_frame(&e.project, 0, plain());
    assert!(inv.pixel(30, 20)[3] > 150 && inv.pixel(10, 20)[3] == 0, "{:?} {:?}", inv.pixel(30, 20), inv.pixel(10, 20));
    e.apply(Command::SetLayerMask { layer: 0, enabled: Some(false), invert: None, editing: None }).unwrap();
    assert!(render_frame(&e.project, 0, plain()).pixel(10, 20)[3] > 200);
    // the mask drawing is in use, so purging keeps it; a duplicate gets its own copy
    assert!(!e.project.unused_drawings().contains(&m.drawing));
    e.apply(Command::DuplicateLayer { layer: 0 }).unwrap();
    let copy = e.project.layers[1].mask.unwrap();
    assert_ne!(copy.drawing, m.drawing, "a private mask");
    assert!(e.project.drawing(copy.drawing).is_some());
    e.apply(Command::RemoveLayerMask { layer: 0 }).unwrap();
    assert!(e.project.layers[0].mask.is_none());
    while e.can_undo() && e.project != start {
        e.undo().unwrap();
    }
    assert_eq!(e.project, start, "every mask step undoes");
}

#[test]
fn an_adjustment_layer_changes_everything_below() {
    let mut e = Engine::new(Project::new("a", 64, 48, 24, 4).unwrap());
    stroke(&mut e, 0, 5.0, 60.0, 20.0);
    e.apply(Command::AddLayer { name: "Adjust".into(), index: None }).unwrap();
    let li = e.project.layers.len() - 1;
    // black lines tinted fully red
    e.apply(Command::AddEffect { layer: li, effect: Effect::new(EffectKind::Tint { colour: [255, 0, 0], amount: 1.0 }), index: None }).unwrap();
    let before = render_frame(&e.project, 0, plain());
    assert_eq!(before.pixel(30, 20)[0], 0, "an ordinary layer's effects act on its own (empty) drawing");
    e.apply(Command::SetLayerAdjustment { layer: li, adjustment: true }).unwrap();
    let after = render_frame(&e.project, 0, plain());
    assert!(after.pixel(30, 20)[0] > 200, "the line below is tinted: {:?}", after.pixel(30, 20));
    // with a drawing, it only acts where the drawing has paint
    stroke(&mut e, li, 5.0, 20.0, 20.0);
    let part = render_frame(&e.project, 0, plain());
    assert!(part.pixel(10, 20)[0] > 200 && part.pixel(40, 20)[0] == 0, "{:?} {:?}", part.pixel(10, 20), part.pixel(40, 20));
    // half opacity: half way
    e.apply(Command::SetLayer { layer: li, visible: None, locked: None, opacity: Some(0.5) }).unwrap();
    let half = render_frame(&e.project, 0, plain()).pixel(10, 20)[0];
    assert!(half > 90 && half < 160, "{half}");
    e.undo().unwrap();
    e.undo().unwrap();
    e.undo().unwrap();
    assert!(!e.project.layers[li].adjustment);
}
