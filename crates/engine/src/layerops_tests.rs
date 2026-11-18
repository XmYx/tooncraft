//! Layer operations: duplicate, merge, flatten, copy properties, locked transparency.

use super::*;
use tooncraft_brush::{BrushKind, Smoothing};
use tooncraft_doc::{BlendMode, Cell, RenderOptions};

fn engine() -> Engine {
    Engine::new(Project::new("Ops", 160, 120, 12, 6).unwrap())
}

fn pen(color: [u8; 4]) -> Brush {
    Brush { size: 30.0, hardness: 1.0, color, smoothing: Smoothing { average: 0.0, catch_up: false, ..Default::default() }, ..Brush::preset(BrushKind::Ink) }
}

fn row(y: f32) -> Vec<Sample> {
    (0..30).map(|i| Sample::at(20.0 + i as f32 * 4.0, y, 1.0, i as f64 * 0.01)).collect()
}

fn dab(e: &mut Engine, layer: usize, frame: u32, color: [u8; 4], y: f32) {
    e.apply(Command::Stroke { layer, frame, brush: pen(color), samples: row(y) }).unwrap();
}

fn frame_image(e: &Engine, frame: u32) -> Vec<u8> {
    tooncraft_doc::render_frame(&e.project, frame, RenderOptions { paper: false, ..Default::default() }).data
}

/// Two layers: red bar on 0 (frames 0..3, then a different one on 3), a blue bar on 1.
fn two() -> Engine {
    let mut e = engine();
    dab(&mut e, 0, 0, [255, 0, 0, 255], 60.0);
    e.apply(Command::ExtendHold { layer: 0, frame: 0 }).unwrap();
    e.apply(Command::ExtendHold { layer: 0, frame: 1 }).unwrap();
    dab(&mut e, 0, 3, [255, 128, 0, 255], 40.0);
    e.apply(Command::AddLayer { name: "Top".into(), index: None }).unwrap();
    dab(&mut e, 1, 0, [0, 0, 255, 255], 60.0);
    e
}

#[test]
fn duplicate_copies_look_and_drawings_and_undoes() {
    let mut e = two();
    e.apply(Command::SetLayer { layer: 0, visible: None, locked: None, opacity: Some(0.7) }).unwrap();
    e.apply(Command::SetLayerBlend { layer: 0, blend: BlendMode::Multiply }).unwrap();
    let before = e.project.clone();
    let drawings = before.drawing_count();
    e.apply(Command::DuplicateLayer { layer: 0 }).unwrap();
    let p = &e.project;
    assert_eq!(p.layers.len(), 3);
    assert_eq!(p.layers[1].name, "Layer 1 copy");
    assert_ne!(p.layers[1].id, p.layers[0].id);
    assert_eq!((p.layers[1].opacity, p.layers[1].blend), (0.7, BlendMode::Multiply));
    assert_eq!(p.drawing_count(), drawings + 2, "one copy per drawing the layer shows");
    assert_eq!(sheet(&p.layers[1].cells), sheet(&p.layers[0].cells), "same timing, holds kept");
    // private drawings: painting on the copy leaves the original alone
    let original = p.layers[0].drawing_at(0).unwrap();
    let copy = p.layers[1].drawing_at(0).unwrap();
    assert_ne!(original, copy);
    let snapshot = p.drawing(original).cloned().unwrap();
    dab(&mut e, 1, 0, [0, 255, 0, 255], 20.0);
    assert_eq!(e.project.drawing(original), Some(&snapshot));
    e.undo().unwrap();
    e.undo().unwrap();
    assert_eq!(e.project, before, "undo removes the layer and its drawings");
    e.redo().unwrap();
    assert_eq!(e.project.layers.len(), 3);
}

#[test]
fn duplicate_fails_cleanly_at_the_layer_limit() {
    let mut e = Engine::new(Project::new("Full", 32, 32, 12, 2).unwrap());
    for i in 1..tooncraft_doc::MAX_LAYERS {
        e.project.add_layer(&format!("L{i}"), i).unwrap();
    }
    let before = e.project.clone();
    assert!(e.apply(Command::DuplicateLayer { layer: 0 }).is_err());
    assert_eq!(e.project, before);
    assert!(e.apply(Command::DuplicateLayer { layer: 900 }).is_err());
}

#[test]
fn merge_down_bakes_blend_mode_and_opacity() {
    let mut e = two();
    e.apply(Command::SetLayerBlend { layer: 1, blend: BlendMode::Multiply }).unwrap();
    e.apply(Command::SetLayer { layer: 1, visible: None, locked: None, opacity: Some(0.5) }).unwrap();
    let shown: Vec<Vec<u8>> = (0..6).map(|f| frame_image(&e, f)).collect();
    let before = e.project.clone();
    e.apply(Command::MergeDown { layer: 1 }).unwrap();
    assert_eq!(e.project.layers.len(), 1);
    let l = &e.project.layers[0];
    assert_eq!((l.blend, l.opacity, l.visible), (BlendMode::Normal, 1.0, true));
    assert_eq!(l.name, "Layer 1", "the lower layer's name stays");
    for (f, expected) in shown.iter().enumerate() {
        assert_eq!(&frame_image(&e, f as u32), expected, "frame {f} looks the same after the merge");
    }
    // frames 0..3 were a hold underneath and a hold above: one merged drawing, held
    assert_eq!(sheet(&l.cells).chars().take(3).collect::<String>(), "#..".to_string().replace('#', "#"));
    e.undo().unwrap();
    assert_eq!(e.project, before);
    e.redo().unwrap();
    assert_eq!(e.project.layers.len(), 1);
}

#[test]
fn merge_down_follows_a_peg_and_opacity_keys() {
    let mut e = two();
    e.apply(Command::SetOpacityKey { layer: 1, frame: 0, opacity: 1.0 }).unwrap();
    e.apply(Command::SetOpacityKey { layer: 1, frame: 5, opacity: 0.0 }).unwrap();
    let shown: Vec<Vec<u8>> = (0..6).map(|f| frame_image(&e, f)).collect();
    assert_ne!(shown[0], shown[4], "the fade shows");
    e.apply(Command::MergeDown { layer: 1 }).unwrap();
    for (f, expected) in shown.iter().enumerate() {
        assert_eq!(&frame_image(&e, f as u32), expected, "frame {f}");
    }
    assert!(e.project.layers[0].opacity_keys.is_empty());
}

#[test]
fn merge_down_refuses_what_it_cannot_keep_true() {
    let mut e = two();
    assert!(matches!(e.apply(Command::MergeDown { layer: 0 }), Err(EngineError::Merge(_))), "nothing below the bottom layer");
    e.apply(Command::SetLayer { layer: 1, visible: Some(false), locked: None, opacity: None }).unwrap();
    assert!(matches!(e.apply(Command::MergeDown { layer: 1 }), Err(EngineError::Merge(_))), "hidden");
    e.apply(Command::SetLayer { layer: 1, visible: Some(true), locked: Some(true), opacity: None }).unwrap();
    assert!(matches!(e.apply(Command::MergeDown { layer: 1 }), Err(EngineError::Locked(_))));
    e.apply(Command::SetLayer { layer: 1, visible: None, locked: Some(false), opacity: None }).unwrap();
    e.apply(Command::SetLayerFolder { layer: 1, folder: Some("A".into()) }).unwrap();
    assert!(matches!(e.apply(Command::MergeDown { layer: 1 }), Err(EngineError::Merge(_))), "different folders");
    e.apply(Command::SetLayerFolder { layer: 1, folder: None }).unwrap();
    e.apply(Command::SetLayerClip { layer: 0, clip: true }).unwrap();
    assert!(matches!(e.apply(Command::MergeDown { layer: 1 }), Err(EngineError::Merge(_))), "base clipped to something outside");
    e.apply(Command::SetLayerClip { layer: 0, clip: false }).unwrap();
    // a matte cutting a layer that is not part of the merge
    e.apply(Command::AddLayer { name: "Cut".into(), index: None }).unwrap();
    e.apply(Command::SetLayerMatte { layer: 2, matte: Some(tooncraft_doc::Matte::Inside) }).unwrap();
    assert!(matches!(e.apply(Command::MergeDown { layer: 2 }), Err(EngineError::Merge(_))));
}

#[test]
fn merge_down_inside_a_folder_applies_the_folder_once() {
    let mut e = two();
    e.apply(Command::SetLayerFolder { layer: 0, folder: Some("F".into()) }).unwrap();
    e.apply(Command::SetLayerFolder { layer: 1, folder: Some("F".into()) }).unwrap();
    e.apply(Command::SetFolder { name: "F".into(), visible: None, opacity: Some(0.5), collapsed: None }).unwrap();
    let shown: Vec<Vec<u8>> = (0..6).map(|f| frame_image(&e, f)).collect();
    e.apply(Command::MergeDown { layer: 1 }).unwrap();
    assert_eq!(e.project.layers[0].folder.as_deref(), Some("F"));
    for (f, expected) in shown.iter().enumerate() {
        assert_eq!(&frame_image(&e, f as u32), expected, "frame {f}");
    }
}

#[test]
fn merge_visible_and_flatten() {
    let mut e = two();
    e.apply(Command::AddLayer { name: "Spare".into(), index: None }).unwrap();
    dab(&mut e, 2, 1, [0, 255, 0, 255], 90.0);
    e.apply(Command::SetLayer { layer: 2, visible: Some(false), locked: None, opacity: None }).unwrap();
    let shown: Vec<Vec<u8>> = (0..6).map(|f| frame_image(&e, f)).collect();
    let before = e.project.clone();

    e.apply(Command::MergeVisible).unwrap();
    assert_eq!(e.project.layers.len(), 2, "the hidden layer stays");
    assert_eq!(e.project.layers[0].name, "Merged");
    assert!(!e.project.layers[1].visible);
    for (f, expected) in shown.iter().enumerate() {
        assert_eq!(&frame_image(&e, f as u32), expected, "frame {f}");
    }
    e.undo().unwrap();
    assert_eq!(e.project, before);

    e.apply(Command::Flatten).unwrap();
    assert_eq!(e.project.layers.len(), 1, "hidden layers go too");
    for (f, expected) in shown.iter().enumerate() {
        assert_eq!(&frame_image(&e, f as u32), expected, "frame {f}");
    }
    e.undo().unwrap();
    assert_eq!(e.project, before);

    let mut lone = engine();
    assert!(matches!(lone.apply(Command::MergeVisible), Err(EngineError::Merge(_))), "one layer has nothing to merge");
}

#[test]
fn merge_through_effects_matches_the_composite() {
    let mut e = two();
    e.apply(Command::AddEffect { layer: 1, effect: tooncraft_fx::Effect::new(tooncraft_fx::EffectKind::Blur { radius: 4.0 }), index: None }).unwrap();
    let shown: Vec<Vec<u8>> = (0..6).map(|f| frame_image(&e, f)).collect();
    e.apply(Command::MergeDown { layer: 1 }).unwrap();
    assert!(e.project.layers[0].effects.is_empty());
    for (f, expected) in shown.iter().enumerate() {
        assert_eq!(&frame_image(&e, f as u32), expected, "frame {f}");
    }
}

#[test]
fn copy_properties_moves_the_look_only() {
    let mut e = two();
    e.apply(Command::SetLayer { layer: 0, visible: None, locked: None, opacity: Some(0.4) }).unwrap();
    e.apply(Command::SetLayerBlend { layer: 0, blend: BlendMode::SoftLight }).unwrap();
    e.apply(Command::SetLayerColor { layer: 0, color: Some([1, 2, 3]) }).unwrap();
    e.apply(Command::SetOpacityKey { layer: 0, frame: 2, opacity: 0.1 }).unwrap();
    let before = e.project.clone();
    e.apply(Command::CopyLayerProperties { from: 0, to: 1 }).unwrap();
    let (a, b) = (&e.project.layers[0], &e.project.layers[1]);
    assert_eq!((b.opacity, b.blend, b.color, &b.opacity_keys), (a.opacity, a.blend, a.color, &a.opacity_keys));
    assert_eq!(b.name, "Top");
    assert_eq!(b.cells, before.layers[1].cells, "drawings and timing are not copied");
    e.undo().unwrap();
    assert_eq!(e.project, before);
    e.apply(Command::SetLayer { layer: 1, visible: None, locked: Some(true), opacity: None }).unwrap();
    assert!(matches!(e.apply(Command::CopyLayerProperties { from: 0, to: 1 }), Err(EngineError::Locked(_))));
    assert!(e.apply(Command::CopyLayerProperties { from: 0, to: 9 }).is_err());
}

#[test]
fn locked_transparency_recolours_only_existing_paint_and_survives_save() {
    let mut e = engine();
    dab(&mut e, 0, 0, [255, 0, 0, 255], 60.0);
    e.apply(Command::SetLayerLockAlpha { layer: 0, lock: true }).unwrap();
    let before = e.project.drawing(e.project.drawing_at(0, 0).unwrap()).cloned().unwrap();
    // a tall stroke through the bar
    let tall: Vec<Sample> = (0..30).map(|i| Sample::at(80.0, 5.0 + i as f32 * 3.5, 1.0, i as f64 * 0.01)).collect();
    e.apply(Command::Stroke { layer: 0, frame: 0, brush: pen([0, 0, 255, 255]), samples: tall }).unwrap();
    let d = e.project.drawing(e.project.drawing_at(0, 0).unwrap()).unwrap();
    assert_eq!(d.pixel(80, 60), [0, 0, 255, 255], "the bar turned blue");
    assert_eq!(d.pixel(80, 15), [0; 4], "empty paper stays empty");
    for y in 0..120 {
        for x in 0..160 {
            assert_eq!(d.pixel(x, y)[3], before.pixel(x, y)[3], "alpha at {x},{y}");
        }
    }
    // a fill would add paint: refused
    let r = e.apply(Command::Fill {
        layer: 0,
        frame: 0,
        x: 5,
        y: 5,
        color: [0, 255, 0, 255],
        options: Default::default(),
        reference: Default::default(),
    });
    assert!(matches!(r, Err(EngineError::TransparencyLocked(_))), "{r:?}");
    // saved and reloaded
    let bytes = tooncraft_format::to_bytes(&e.project).unwrap();
    let back = tooncraft_format::from_bytes(&bytes).unwrap();
    assert!(back.layers[0].lock_alpha);
    // a locked layer's first stroke on a blank cell paints nothing (there is no paint to recolour)
    e.apply(Command::Stroke { layer: 0, frame: 3, brush: pen([0, 255, 0, 255]), samples: row(30.0) }).unwrap();
    let blank = e.project.drawing(e.project.drawing_at(0, 3).unwrap()).unwrap();
    assert!(blank.is_empty());
    e.apply(Command::SetLayerLockAlpha { layer: 0, lock: false }).unwrap();
    dab(&mut e, 0, 3, [0, 255, 0, 255], 30.0);
    assert!(!e.project.drawing(e.project.drawing_at(0, 3).unwrap()).unwrap().is_empty());
}

#[test]
fn layer_commands_work_from_command_files() {
    let cmds = [
        r#"{"cmd":"duplicate_layer","layer":0}"#,
        r#"{"cmd":"set_layer_lock_alpha","layer":1,"lock":true}"#,
        r#"{"cmd":"set_layer_blend","layer":1,"blend":"hard_mix"}"#,
        r#"{"cmd":"copy_layer_properties","from":1,"to":0}"#,
        r#"{"cmd":"merge_down","layer":1}"#,
        r#"{"cmd":"merge_visible"}"#,
        r#"{"cmd":"flatten"}"#,
    ];
    for c in cmds {
        let parsed: Command = serde_json::from_str(c).unwrap();
        assert_eq!(serde_json::to_string(&parsed).unwrap().contains("\"cmd\""), true);
    }
    let mut e = engine();
    for c in &cmds[..4] {
        e.apply(serde_json::from_str(c).unwrap()).unwrap();
    }
    assert_eq!(e.project.layers[0].blend, BlendMode::HardMix);
    assert!(e.project.layers[1].lock_alpha);
    assert!(serde_json::from_str::<Command>(r#"{"cmd":"set_layer_blend","layer":0,"blend":"nonsense"}"#).is_err());
    let _ = Cell::Blank;
}

#[test]
fn every_blend_mode_renders_and_round_trips_through_a_save() {
    let mut e = two();
    for (i, mode) in BlendMode::ALL.into_iter().enumerate() {
        e.apply(Command::SetLayerBlend { layer: 1, blend: mode }).unwrap();
        let img = frame_image(&e, 0);
        assert!(img.as_chunks::<4>().0.iter().all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]), "{mode:?} gave an invalid colour");
        let back = tooncraft_format::from_bytes(&tooncraft_format::to_bytes(&e.project).unwrap()).unwrap();
        assert_eq!(back.layers[1].blend, mode, "mode {i}");
    }
}
