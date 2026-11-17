use super::*;
use crate::export::{ExportOptions, gif, ora_frame, png_sequence};
use tooncraft_brush::BrushKind;

fn engine() -> Engine {
    Engine::new(Project::new("Test", 320, 240, 12, 8).unwrap())
}

fn circle(cx: f32, cy: f32, r: f32) -> Vec<Sample> {
    (0..=72)
        .map(|i| {
            let a = i as f32 / 72.0 * std::f32::consts::TAU;
            Sample::at(cx + a.cos() * r, cy + a.sin() * r, 1.0, i as f64 * 0.01)
        })
        .collect()
}

fn ink() -> Brush {
    Brush { size: 4.0, smoothing: tooncraft_brush::Smoothing { average: 0.0, ..Default::default() }, ..Brush::preset(BrushKind::Ink) }
}

fn scratch(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("tooncraft-{name}-{}", std::process::id()))
}

#[test]
fn stroke_on_blank_cell_makes_a_drawing_and_undoes_cleanly() {
    let mut e = engine();
    let before = e.project.clone();
    e.apply(Command::Stroke { layer: 0, frame: 2, brush: ink(), samples: circle(100.0, 100.0, 40.0) }).unwrap();
    assert!(e.project.drawing_at(0, 2).is_some());
    assert_eq!(sheet(&e.project.layers[0].cells), "..#.....");
    assert!(e.is_dirty());
    e.undo().unwrap();
    assert_eq!(e.project, before);
    e.redo().unwrap();
    assert!(e.project.drawing_at(0, 2).is_some_and(|d| !e.project.drawing(d).is_some_and(|s| s.is_empty())));
}

#[test]
fn live_stroke_and_cancel() {
    let mut e = engine();
    e.begin_stroke(0, 0, ink()).unwrap();
    assert_eq!(e.begin_stroke(0, 0, ink()), Err(EngineError::Busy));
    for s in circle(50.0, 50.0, 20.0) {
        e.stroke_to(s).unwrap();
    }
    assert!(e.undo().is_err(), "no undo while drawing");
    e.cancel_stroke();
    assert_eq!(e.project.drawing_at(0, 0), None);
    assert_eq!(e.project.drawing_count(), 0);
}

#[test]
fn fill_with_gap_closing_inside_a_drawn_circle() {
    let mut e = engine();
    let mut pts = circle(160.0, 120.0, 60.0);
    pts.truncate(70);
    e.apply(Command::Stroke { layer: 0, frame: 0, brush: ink(), samples: pts }).unwrap();
    let red = [220, 30, 30, 255];
    e.apply(Command::Fill {
        layer: 0,
        frame: 0,
        x: 160,
        y: 120,
        color: red,
        options: FillOptions { gap: 6, ..Default::default() },
        reference: FillReference::Layer,
    })
    .unwrap();
    assert_eq!(e.pick_color(0, 160, 120), red);
    assert_ne!(e.pick_color(0, 10, 10), red, "the fill did not leak through the gap");
    assert_eq!(e.undo_label(), Some("Fill"));
}

#[test]
fn paint_under_ink_on_its_own_layer() {
    let mut e = engine();
    e.apply(Command::Stroke { layer: 0, frame: 0, brush: ink(), samples: circle(160.0, 120.0, 50.0) }).unwrap();
    e.apply(Command::AddLayer { name: "Paint".into(), index: Some(0) }).unwrap();
    e.apply(Command::Fill {
        layer: 0,
        frame: 0,
        x: 160,
        y: 120,
        color: [40, 90, 200, 255],
        options: FillOptions::default(),
        reference: FillReference::AllLayers,
    })
    .unwrap();
    assert!(e.project.drawing_at(0, 0).is_some());
    assert_eq!(e.pick_color(0, 160, 120), [40, 90, 200, 255]);
    assert_eq!(e.project.layers[1].name, "Layer 1");
}

#[test]
fn timeline_commands() {
    let mut e = engine();
    e.apply(Command::NewDrawing { layer: 0, frame: 0 }).unwrap();
    e.apply(Command::ExtendHold { layer: 0, frame: 0 }).unwrap();
    e.apply(Command::DuplicateDrawing { layer: 0, frame: 1 }).unwrap();
    assert_eq!(sheet(&e.project.layers[0].cells), "#-#.....");
    assert_ne!(e.project.drawing_at(0, 0), e.project.drawing_at(0, 2));
    e.apply(Command::InsertFrame { layer: None, frame: 1 }).unwrap();
    assert_eq!(sheet(&e.project.layers[0].cells), "#--#....");
    e.apply(Command::Retime { layer: 0, from: 0, to: 7, step: 3 }).unwrap();
    assert_eq!(sheet(&e.project.layers[0].cells), "#--#--..");
    e.apply(Command::ShortenHold { layer: 0, frame: 4 }).unwrap();
    e.apply(Command::DeleteFrame { layer: None, frame: 0 }).unwrap();
    assert_eq!(sheet(&e.project.layers[0].cells), "#-#-....");
    e.apply(Command::ClearCell { layer: 0, frame: 2 }).unwrap();
    assert_eq!(e.project.unused_drawings().len(), 1);
    e.apply(Command::PurgeUnused).unwrap();
    assert_eq!(e.project.drawing_count(), 1);
    for _ in 0..9 {
        e.undo().unwrap();
    }
    assert_eq!(sheet(&e.project.layers[0].cells), "........");
    assert!(e.undo().is_err());
}

#[test]
fn locked_layers_refuse_edits() {
    let mut e = engine();
    e.apply(Command::SetLayer { layer: 0, visible: None, locked: Some(true), opacity: None }).unwrap();
    assert!(matches!(e.apply(Command::Stroke { layer: 0, frame: 0, brush: ink(), samples: circle(5.0, 5.0, 3.0) }), Err(EngineError::Locked(_))));
    assert!(e.apply(Command::NewDrawing { layer: 0, frame: 0 }).is_err());
    assert!(e.apply(Command::Stroke { layer: 9, frame: 0, brush: ink(), samples: vec![] }).is_err());
}

#[test]
fn undo_budget_drops_oldest_steps() {
    let mut e = engine();
    e.undo_budget = 3 * tooncraft_raster::TILE_BYTES as u64;
    for i in 0..10 {
        e.apply(Command::Stroke { layer: 0, frame: 0, brush: ink(), samples: circle(20.0 + i as f32 * 25.0, 100.0, 8.0) }).unwrap();
    }
    assert!(e.undo_len() < 10);
    assert!(e.history_bytes() <= e.undo_budget || e.undo_len() == 1);
}

#[test]
fn length_and_fps_undo() {
    let mut e = engine();
    e.apply(Command::NewDrawing { layer: 0, frame: 7 }).unwrap();
    e.apply(Command::SetLength { frames: 4 }).unwrap();
    e.apply(Command::SetFps { fps: 24 }).unwrap();
    e.undo().unwrap();
    e.undo().unwrap();
    assert_eq!((e.project.length(), e.project.fps()), (8, 12));
    assert!(e.project.drawing_at(0, 7).is_some());
}

#[test]
fn exports_write_files() {
    let mut e = engine();
    for f in 0..4u32 {
        e.apply(Command::Stroke { layer: 0, frame: f, brush: ink(), samples: circle(60.0 + f as f32 * 40.0, 120.0, 30.0) }).unwrap();
    }
    let dir = scratch("export");
    let o = ExportOptions { start: 0, end: 4, paper: true };
    let files = png_sequence(&e.project, &dir.join("png"), "shot", o, |_| {}).unwrap();
    assert_eq!(files.len(), 4);
    assert!(files[0].ends_with("shot_0001.png"));
    gif(&e.project, &dir.join("shot.gif"), o, |_| {}).unwrap();
    let bytes = std::fs::read(dir.join("shot.gif")).unwrap();
    assert!(craft_gif::sniff(&bytes));
    ora_frame(&e.project, 1, &dir.join("frame.ora")).unwrap();
    assert!(craft_ora::is_ora(&std::fs::read(dir.join("frame.ora")).unwrap()));
    assert!(png_sequence(&e.project, &dir, "x", ExportOptions { start: 3, end: 2, paper: true }, |_| {}).is_err());
}

#[test]
fn commands_are_json() {
    let c: Command =
        serde_json::from_str(r#"{"cmd":"stroke","layer":0,"frame":1,"brush":{"kind":"ink","size":6},"samples":[{"x":1,"y":2},{"x":30,"y":40,"pressure":1}]}"#)
            .unwrap();
    let mut e = engine();
    e.apply(c).unwrap();
    assert!(e.project.drawing_at(0, 1).is_some());
    let back = serde_json::to_string(&Command::Retime { layer: 0, from: 0, to: 3, step: 2 }).unwrap();
    assert_eq!(back, r#"{"cmd":"retime","layer":0,"from":0,"to":3,"step":2}"#);
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(40))]
    /// Random command sequences never panic, and undoing everything gives the start state back.
    #[test]
    fn random_commands_undo_to_the_start(ops in proptest::collection::vec((0u8..14, 0usize..3, 0u32..10, 0u32..10), 1..30)) {
        let mut e = engine();
        let start = e.project.clone();
        for (op, layer, a, b) in ops {
            let cmd = match op {
                0 => Command::Stroke { layer, frame: a, brush: ink(), samples: circle(a as f32 * 30.0, b as f32 * 20.0, 12.0) },
                1 => Command::Fill { layer, frame: a, x: a * 31, y: b * 23, color: [1, 2, 3, 255], options: FillOptions::default(), reference: FillReference::AllLayers },
                2 => Command::NewDrawing { layer, frame: a },
                3 => Command::DuplicateDrawing { layer, frame: a },
                4 => Command::InsertFrame { layer: Some(layer), frame: a },
                5 => Command::DeleteFrame { layer: None, frame: a },
                6 => Command::ExtendHold { layer, frame: a },
                7 => Command::ShortenHold { layer, frame: a },
                8 => Command::Retime { layer, from: a.min(b), to: a.max(b), step: b % 4 + 1 },
                9 => Command::AddLayer { name: "x".into(), index: Some(layer) },
                10 => Command::DeleteLayer { layer },
                11 => Command::MoveLayer { from: layer, to: a as usize },
                12 => Command::ClearDrawing { layer, frame: a },
                _ => Command::PurgeUnused,
            };
            let _ = e.apply(cmd);
        }
        while e.undo().is_ok() {}
        proptest::prop_assert_eq!(&e.project.layers, &start.layers);
        for f in 0..8 {
            proptest::prop_assert_eq!(tooncraft_doc::render_frame(&e.project, f, RenderOptions::default()), tooncraft_doc::render_frame(&start, f, RenderOptions::default()));
        }
    }
}
