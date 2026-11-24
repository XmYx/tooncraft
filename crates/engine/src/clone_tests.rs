//! Tests of the clone stamp command.

use proptest::prelude::*;
use tooncraft_doc::Project;

use crate::clone::CloneTip;
use crate::{Command, Engine};

fn engine() -> Engine {
    Engine::new(Project::new("Clone", 200, 100, 12, 2).unwrap())
}

fn pixel(e: &Engine, layer: usize, x: i64, y: i64) -> [u8; 4] {
    let p = &e.project;
    p.drawing_at(layer, 0).and_then(|d| p.drawing(d)).map(|s| s.pixel(x, y)).unwrap_or([0; 4])
}

/// A red square at 20…40 on layer `layer`.
fn red_square(e: &mut Engine, layer: usize) {
    let square = vec![(20.0, 20.0), (40.0, 20.0), (40.0, 40.0), (20.0, 40.0)];
    e.apply(Command::Gradient {
        layer,
        frame: 0,
        kind: Default::default(),
        from: (0.0, 0.0),
        to: (1.0, 0.0),
        start: [255, 0, 0, 255],
        end: [255, 0, 0, 255],
        area: Some(square),
        area_rings: Vec::new(),
    })
    .unwrap();
}

fn hard(size: f32) -> CloneTip {
    CloneTip { size, hardness: 1.0, opacity: 1.0 }
}

#[test]
fn the_stamp_copies_from_the_offset_as_one_undo_step() {
    let mut e = engine();
    red_square(&mut e, 0);
    let before = e.undo_len();
    // paint at 120,30 what is at 30,30
    e.apply(Command::CloneStamp { layer: 0, frame: 0, source_layer: None, offset: (-90.0, 0.0), tip: hard(12.0), points: vec![(120.0, 30.0), (125.0, 30.0)] })
        .unwrap();
    assert_eq!(pixel(&e, 0, 120, 30), [255, 0, 0, 255], "the copy");
    assert_eq!(pixel(&e, 0, 150, 30)[3], 0, "outside the tip untouched");
    assert_eq!(e.undo_len(), before + 1);
    assert_eq!(e.undo_label(), Some("Clone"));
    e.undo().unwrap();
    assert_eq!(pixel(&e, 0, 120, 30)[3], 0, "undone");
}

#[test]
fn a_stroke_over_its_own_copy_does_not_repeat_it_and_a_soft_tip_blends() {
    let mut e = engine();
    red_square(&mut e, 0);
    // source 10 px to the left along a path that runs right: a self-feeding stamp would smear
    // the red on and on; reading the source from before the stroke keeps the copy 20 px wide
    let points: Vec<(f32, f32)> = (0..80).map(|i| (25.0 + i as f32, 30.0)).collect();
    e.apply(Command::CloneStamp { layer: 0, frame: 0, source_layer: None, offset: (-10.0, 0.0), tip: hard(4.0), points }).unwrap();
    assert_eq!(pixel(&e, 0, 45, 30), [255, 0, 0, 255], "the square, copied 10 px on");
    assert_eq!(pixel(&e, 0, 70, 30)[3], 0, "not fed by its own copy");
    let mut e = engine();
    red_square(&mut e, 0);
    e.apply(Command::CloneStamp {
        layer: 0,
        frame: 0,
        source_layer: None,
        offset: (-90.0, 0.0),
        tip: CloneTip { size: 30.0, hardness: 0.0, opacity: 0.5 },
        points: vec![(120.0, 30.0)],
    })
    .unwrap();
    let centre = pixel(&e, 0, 120, 30)[3];
    let edge = pixel(&e, 0, 133, 30)[3];
    assert!((120..=135).contains(&centre), "half opacity at the centre: {centre}");
    assert!(edge < centre, "softer at the edge: {edge} < {centre}");
}

#[test]
fn another_layer_is_a_source_and_old_files_read() {
    let mut e = engine();
    e.apply(Command::AddLayer { name: "Source".into(), index: None }).unwrap();
    assert_eq!(e.project.layers.len(), 2);
    red_square(&mut e, 1);
    e.apply(Command::CloneStamp { layer: 0, frame: 0, source_layer: Some(1), offset: (0.0, 0.0), tip: hard(10.0), points: vec![(30.0, 30.0)] }).unwrap();
    assert_eq!(pixel(&e, 0, 30, 30), [255, 0, 0, 255], "copied from the layer");
    let bad_layer = Command::CloneStamp { layer: 0, frame: 0, source_layer: Some(9), offset: (0.0, 0.0), tip: hard(10.0), points: vec![(30.0, 30.0)] };
    assert!(e.apply(bad_layer).is_err());
    let bad_offset = Command::CloneStamp { layer: 0, frame: 0, source_layer: None, offset: (f32::NAN, 0.0), tip: hard(10.0), points: vec![(30.0, 30.0)] };
    assert!(e.apply(bad_offset).is_err());
    let c: Command = serde_json::from_str(r#"{"cmd":"clone_stamp","layer":0,"frame":0,"offset":[5,5],"points":[[1,2]]}"#).unwrap();
    assert!(matches!(c, Command::CloneStamp { tip, source_layer: None, .. } if tip == CloneTip::default()));
}

proptest! {
    #[test]
    fn hostile_stamps_never_panic(
        size in prop_oneof![Just(f32::NAN), Just(f32::INFINITY), Just(-5.0f32), 0.0f32..2000.0],
        hardness in prop_oneof![Just(f32::NAN), -2.0f32..3.0],
        opacity in prop_oneof![Just(f32::NAN), -2.0f32..3.0],
        dx in prop_oneof![Just(f32::INFINITY), -1.0e9f32..1.0e9],
        pts in proptest::collection::vec((prop_oneof![Just(f32::NAN), -1.0e6f32..1.0e6], -1.0e6f32..1.0e6), 0..8),
    ) {
        let mut e = engine();
        let _ = e.apply(Command::CloneStamp { layer: 0, frame: 0, source_layer: None, offset: (dx, 3.0), tip: CloneTip { size, hardness, opacity }, points: pts });
    }
}
