//! Tests of the liquify command.

use proptest::prelude::*;
use tooncraft_doc::Project;

use crate::liquify::{LiquifyBrush, LiquifyMode};
use crate::{Command, Engine};

fn engine() -> Engine {
    Engine::new(Project::new("Liquify", 200, 200, 12, 1).unwrap())
}

fn alpha(e: &Engine, x: i64, y: i64) -> u8 {
    let p = &e.project;
    p.drawing_at(0, 0).and_then(|d| p.drawing(d)).map(|s| s.pixel(x, y)[3]).unwrap_or(0)
}

/// A red square at 80…120.
fn square(e: &mut Engine) {
    let sq = vec![(80.0, 80.0), (120.0, 80.0), (120.0, 120.0), (80.0, 120.0)];
    let c = [255, 0, 0, 255];
    e.apply(Command::Gradient {
        layer: 0,
        frame: 0,
        kind: Default::default(),
        from: (0.0, 0.0),
        to: (1.0, 0.0),
        start: c,
        end: c,
        area: Some(sq),
        area_rings: Vec::new(),
        feather: 0.0,
        stops: Vec::new(),
    })
    .unwrap();
}

fn brush(radius: f32, strength: f32) -> LiquifyBrush {
    LiquifyBrush { radius, strength }
}

#[test]
fn push_drags_paint_along_the_stroke_as_one_undo_step() {
    let mut e = engine();
    square(&mut e);
    assert_eq!(alpha(&e, 130, 100), 0);
    let before = e.undo_len();
    // push from inside the square's right edge out to the right
    let points: Vec<(f32, f32)> = (0..=20).map(|i| (110.0 + i as f32, 100.0)).collect();
    e.apply(Command::Liquify { layer: 0, frame: 0, mode: LiquifyMode::Push, brush: brush(30.0, 1.0), points }).unwrap();
    assert!(alpha(&e, 125, 100) > 0, "paint pushed past the old edge");
    assert_eq!(e.undo_len(), before + 1);
    assert_eq!(e.undo_label(), Some("Liquify"));
    e.undo().unwrap();
    assert_eq!(alpha(&e, 125, 100), 0, "undone");
}

#[test]
fn bloat_swells_pinch_shrinks_and_twirl_turns() {
    // bloat at the square's centre pushes its edge out; pinch pulls it in
    let count = |e: &Engine| (60..140).flat_map(|y| (60..140).map(move |x| (x, y))).filter(|(x, y)| alpha(e, *x, *y) > 128).count();
    let mut e = engine();
    square(&mut e);
    let base = count(&e);
    let centre = vec![(100.0, 100.0); 8];
    e.apply(Command::Liquify { layer: 0, frame: 0, mode: LiquifyMode::Bloat, brush: brush(40.0, 1.0), points: centre.clone() }).unwrap();
    assert!(count(&e) > base, "bloat: {} > {base}", count(&e));
    let mut e = engine();
    square(&mut e);
    e.apply(Command::Liquify { layer: 0, frame: 0, mode: LiquifyMode::Pinch, brush: brush(40.0, 1.0), points: centre.clone() }).unwrap();
    assert!(count(&e) < base, "pinch: {} < {base}", count(&e));
    // a twirl over a corner moves it off the square's straight edge
    let mut e = engine();
    square(&mut e);
    e.apply(Command::Liquify { layer: 0, frame: 0, mode: LiquifyMode::TwirlLeft, brush: brush(30.0, 1.0), points: vec![(100.0, 100.0); 10] }).unwrap();
    let outside = (60..140)
        .flat_map(|y| (60..140).map(move |x| (x, y)))
        .filter(|(x, y)| !(80..120).contains(x) || !(80..120).contains(y))
        .any(|(x, y)| alpha(&e, x, y) > 128);
    assert!(outside, "twirled paint leaves the old square");
    // old command lines without mode and brush read push with the default brush
    let c: Command = serde_json::from_str(r#"{"cmd":"liquify","layer":0,"frame":0,"points":[[1,2]]}"#).unwrap();
    assert!(matches!(c, Command::Liquify { mode: LiquifyMode::Push, brush, .. } if brush == LiquifyBrush::default()));
}

#[test]
fn a_hostile_stroke_is_refused_promptly() {
    let mut e = Engine::new(Project::new("Big", 2000, 2000, 12, 1).unwrap());
    let undo = e.undo_len();
    let zig: Vec<(f32, f32)> = (0..20_000).map(|i| if i % 2 == 0 { (100.0, 100.0) } else { (1500.0, 1500.0) }).collect();
    let t = std::time::Instant::now();
    let r = e.apply(Command::Liquify { layer: 0, frame: 0, mode: LiquifyMode::Push, brush: brush(500.0, 1.0), points: zig });
    assert!(matches!(r, Err(crate::EngineError::Limit(_))), "{r:?}");
    assert!(t.elapsed() < std::time::Duration::from_secs(5), "took {:?}", t.elapsed());
    assert_eq!(e.undo_len(), undo);
}

proptest! {
    #[test]
    fn hostile_liquify_never_panics(
        radius in prop_oneof![Just(f32::NAN), Just(f32::INFINITY), -5.0f32..800.0],
        strength in prop_oneof![Just(f32::NAN), -2.0f32..3.0],
        mode in 0u8..5,
        pts in proptest::collection::vec((prop_oneof![Just(f32::NAN), -1.0e6f32..1.0e6], -1.0e3f32..1.0e3), 0..8),
    ) {
        let mode = [LiquifyMode::Push, LiquifyMode::TwirlLeft, LiquifyMode::TwirlRight, LiquifyMode::Pinch, LiquifyMode::Bloat][mode as usize];
        let mut e = engine();
        square(&mut e);
        let _ = e.apply(Command::Liquify { layer: 0, frame: 0, mode, brush: LiquifyBrush { radius, strength }, points: pts });
    }
}
