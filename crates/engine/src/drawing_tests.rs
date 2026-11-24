//! Tests of the drawing-aid commands (symmetry, path shapes, lasso fill, gradients).

use crate::drawing::GradientKind;
use crate::{Command, Engine};
use tooncraft_brush::{Brush, BrushKind, Sample, Smoothing};
use tooncraft_doc::Project;
use tooncraft_doc::symmetry::{Symmetry, SymmetryMode};

fn engine() -> Engine {
    Engine::new(Project::new("Draw", 200, 100, 12, 4).unwrap())
}

fn ink() -> Brush {
    Brush { size: 6.0, smoothing: Smoothing { average: 0.0, ..Default::default() }, ..Brush::preset(BrushKind::Ink) }
}

const PAPER: [u8; 4] = [246, 243, 236, 255];

#[test]
fn symmetry_mirrors_live_strokes_and_commands() {
    let mut e = engine();
    e.apply(Command::SetSymmetry { symmetry: Symmetry { mode: SymmetryMode::Vertical, center: None, spokes: 6 } }).unwrap();
    let samples: Vec<Sample> = (0..20).map(|i| Sample::at(20.0, 20.0 + i as f32 * 3.0, 1.0, i as f64 * 0.01)).collect();
    e.apply(Command::Stroke { layer: 0, frame: 0, brush: ink(), samples }).unwrap();
    assert_ne!(e.pick_color(0, 20, 40), PAPER);
    assert_ne!(e.pick_color(0, 180, 40), PAPER, "the mirrored copy across x = 100");
    assert_eq!(e.undo_len(), 1, "both halves are one undo step");
    let bytes = tooncraft_format::to_bytes(&e.project).unwrap();
    assert_eq!(tooncraft_format::from_bytes(&bytes).unwrap().symmetry.mode, SymmetryMode::Vertical);
}

#[test]
fn path_shapes_outline_fill_and_lasso_fill() {
    let mut e = engine();
    let tri = vec![(20.0, 80.0), (60.0, 20.0), (100.0, 80.0)];
    let red = [220, 20, 20, 255];
    e.apply(Command::PathShape { layer: 0, frame: 0, brush: ink(), points: tri.clone(), closed: true, curve: false, outline: true, fill: Some(red) }).unwrap();
    assert_eq!(e.undo_label(), Some("Shape"));
    assert_eq!(e.undo_len(), 1, "outline and fill are one step");
    assert_eq!(e.pick_color(0, 60, 60), red, "filled inside");
    assert_ne!(e.pick_color(0, 20, 80), red, "the outline sits on the edge");
    // a lasso fill: fill only
    e.apply(Command::PathShape { layer: 0, frame: 1, brush: ink(), points: tri, closed: true, curve: true, outline: false, fill: Some(red) }).unwrap();
    assert_eq!(e.pick_color(1, 60, 60), red);
    // an open polyline outline
    e.apply(Command::PathShape {
        layer: 0,
        frame: 2,
        brush: ink(),
        points: vec![(10.0, 10.0), (190.0, 10.0)],
        closed: false,
        curve: false,
        outline: true,
        fill: None,
    })
    .unwrap();
    assert_ne!(e.pick_color(2, 100, 10), PAPER);
    assert!(
        e.apply(Command::PathShape {
            layer: 0,
            frame: 2,
            brush: ink(),
            points: vec![(f32::NAN, 0.0), (1.0, 1.0)],
            closed: false,
            curve: false,
            outline: true,
            fill: None
        })
        .is_err()
    );
    e.undo().unwrap();
    e.undo().unwrap();
    e.undo().unwrap();
    assert_eq!(e.pick_color(0, 60, 60), PAPER);
}

#[test]
fn gradients() {
    let mut e = engine();
    e.apply(Command::Gradient {
        layer: 0,
        frame: 0,
        kind: GradientKind::Linear,
        from: (0.0, 0.0),
        to: (200.0, 0.0),
        start: [0, 0, 0, 255],
        end: [255, 255, 255, 255],
        area: None,
        area_rings: Vec::new(),
    })
    .unwrap();
    let (l, m, r) = (e.pick_color(0, 1, 50)[0], e.pick_color(0, 100, 50)[0], e.pick_color(0, 198, 50)[0]);
    assert!(l < 10 && (120..=135).contains(&m) && r > 245, "{l} {m} {r}");
    e.apply(Command::Gradient {
        layer: 0,
        frame: 1,
        kind: GradientKind::Radial,
        from: (100.0, 50.0),
        to: (150.0, 50.0),
        start: [255, 0, 0, 255],
        end: [0, 0, 255, 255],
        area: Some(vec![(80.0, 30.0), (120.0, 30.0), (120.0, 70.0), (80.0, 70.0)]),
        area_rings: Vec::new(),
    })
    .unwrap();
    assert!(e.pick_color(1, 100, 50)[0] > 245, "the centre is the start colour");
    assert_eq!(e.pick_color(1, 10, 10), PAPER, "outside the area untouched");
}

#[test]
fn drawing_commands_are_json() {
    let c: Command = serde_json::from_str(
        r#"{"cmd":"path_shape","layer":0,"frame":0,"brush":{"kind":"ink"},"points":[[0,0],[10,10],[20,0]],"closed":true,"fill":[1,2,3,255]}"#,
    )
    .unwrap();
    assert!(matches!(c, Command::PathShape { outline: true, closed: true, .. }));
    let g: Command =
        serde_json::from_str(r#"{"cmd":"gradient","layer":0,"frame":0,"from":[0,0],"to":[5,0],"start":[0,0,0,255],"end":[255,255,255,0]}"#).unwrap();
    assert!(matches!(g, Command::Gradient { kind: GradientKind::Linear, .. }));
    let s: Command = serde_json::from_str(r#"{"cmd":"set_symmetry","symmetry":{"mode":"radial","spokes":5}}"#).unwrap();
    let mut e = engine();
    e.apply(s).unwrap();
    assert_eq!(e.project.symmetry.spokes, 5);
}

#[test]
fn selection_rings_skew_distort_and_wand() {
    let mut e = engine();
    // a square of paint
    e.apply(Command::Fill { layer: 0, frame: 0, x: 5, y: 5, color: [20, 20, 20, 255], options: Default::default(), reference: Default::default() }).unwrap();
    let sq = vec![(40.0, 20.0), (80.0, 20.0), (80.0, 60.0), (40.0, 60.0)];
    let hole = vec![(50.0, 30.0), (70.0, 30.0), (70.0, 50.0), (50.0, 50.0)];
    // erasing a ring with a hole keeps the hole's paint
    e.apply(Command::ClearSelection { layer: 0, frame: 0, polygon: sq.clone(), rings: vec![hole.clone()] }).unwrap();
    assert_eq!(e.pick_color(0, 45, 25), PAPER);
    assert_ne!(e.pick_color(0, 60, 40), PAPER, "the hole is not erased");
    e.undo().unwrap();
    // skew and a resampling choice go through
    e.apply(Command::TransformSelection {
        layer: 0,
        frame: 0,
        polygon: sq.clone(),
        rings: Vec::new(),
        dx: 0.0,
        dy: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
        angle: 0.0,
        skew_x: 0.3,
        skew_y: 0.0,
        resample: tooncraft_raster::select::Resample::Nearest,
    })
    .unwrap();
    assert_eq!(e.undo_label(), Some("Transform"));
    e.undo().unwrap();
    // free distort to a smaller quad: one undo step; a degenerate quad is refused, unchanged
    e.apply(Command::DistortSelection {
        layer: 0,
        frame: 0,
        polygon: sq.clone(),
        rings: Vec::new(),
        corners: [(50.0, 30.0), (70.0, 30.0), (70.0, 50.0), (50.0, 50.0)],
        resample: Default::default(),
    })
    .unwrap();
    assert_eq!(e.undo_label(), Some("Distort"));
    assert_eq!(e.pick_color(0, 42, 22), PAPER, "the corner moved in");
    let n = e.undo_len();
    assert!(
        e.apply(Command::DistortSelection {
            layer: 0,
            frame: 0,
            polygon: sq.clone(),
            rings: Vec::new(),
            corners: [(1.0, 1.0); 4],
            resample: Default::default()
        })
        .is_err()
    );
    assert_eq!(e.undo_len(), n);
    // the magic wand picks the painted area of the drawing
    e.undo().unwrap();
    let m = crate::select::wand(&e, 0, 0, 10, 10, 30, false).expect("an area");
    assert_eq!(m.count(), 200 * 100, "the fill covers the whole drawing");
    assert!(crate::select::wand(&e, 0, 0, 9_999, 10, 30, false).is_none());
}

#[test]
fn the_wand_can_select_one_colour_everywhere() {
    let mut e = engine();
    let red = [200, 20, 20, 255];
    // two red squares apart
    for (x0, x1) in [(10.0, 30.0), (150.0, 170.0)] {
        e.apply(Command::PathShape {
            layer: 0,
            frame: 0,
            brush: ink(),
            points: vec![(x0, 10.0), (x1, 10.0), (x1, 30.0), (x0, 30.0)],
            closed: true,
            curve: false,
            outline: false,
            fill: Some(red),
        })
        .unwrap();
    }
    let one = crate::select::wand_with(&e, 0, 0, 20, 20, 10, false, true).expect("contiguous");
    let all = crate::select::wand_with(&e, 0, 0, 20, 20, 10, false, false).expect("everywhere");
    assert!(all.count() >= one.count() * 2 - 10, "{} vs {}", all.count(), one.count());
    assert!(all.get(160, 20) && !one.get(160, 20));
    assert!(crate::select::wand_with(&e, 0, 0, 999, 20, 10, false, false).is_none());
}

#[test]
fn rulers_steer_strokes_and_save_with_the_shot() {
    use tooncraft_doc::rulers::{Ruler, Rulers};
    let mut e = engine();
    let rulers = Rulers { on: true, list: vec![Ruler::Line { a: (0.0, 50.0), b: (200.0, 50.0) }], ..Rulers::default() };
    e.apply(Command::SetRulers { rulers: rulers.clone() }).unwrap();
    assert_eq!(e.undo_len(), 0, "a setting, not an undo step");
    // a wobbly stroke started near y = 50
    let samples: Vec<Sample> = (0..40).map(|i| Sample::at(20.0 + i as f32 * 4.0, 45.0 + (i % 4) as f32 * 3.0, 1.0, i as f64 * 0.01)).collect();
    e.apply(Command::Stroke { layer: 0, frame: 0, brush: ink(), samples: samples.clone() }).unwrap();
    assert_ne!(e.pick_color(0, 100, 50), PAPER, "painted on the ruler");
    for y in [40, 44, 56, 60] {
        assert_eq!(e.pick_color(0, 100, y), PAPER, "nothing off the ruler at y = {y}");
    }
    assert_eq!(e.undo_len(), 1);
    // with symmetry too: the snapped stroke is mirrored
    e.apply(Command::SetRulers { rulers: Rulers { list: vec![Ruler::Line { a: (30.0, 0.0), b: (30.0, 100.0) }], ..rulers.clone() } }).unwrap();
    e.apply(Command::SetSymmetry { symmetry: Symmetry { mode: SymmetryMode::Vertical, center: None, spokes: 6 } }).unwrap();
    let down: Vec<Sample> = (0..20).map(|i| Sample::at(26.0 + (i % 3) as f32 * 3.0, 10.0 + i as f32 * 4.0, 1.0, i as f64 * 0.01)).collect();
    e.apply(Command::Stroke { layer: 0, frame: 1, brush: ink(), samples: down }).unwrap();
    assert_ne!(e.pick_color(1, 30, 40), PAPER);
    assert_ne!(e.pick_color(1, 170, 40), PAPER, "the mirrored copy of the snapped stroke");
    assert_eq!(e.pick_color(1, 37, 40), PAPER);
    let bytes = tooncraft_format::to_bytes(&e.project).unwrap();
    assert_eq!(tooncraft_format::from_bytes(&bytes).unwrap().rulers, e.project.rulers);
    // rulers off: freehand again
    e.apply(Command::SetRulers { rulers: Rulers::default() }).unwrap();
    e.apply(Command::Stroke { layer: 0, frame: 2, brush: ink(), samples }).unwrap();
    assert_ne!(e.pick_color(2, 32, 54), PAPER, "the wobble is kept");
}

#[test]
fn a_tap_near_a_perspective_ruler_still_paints() {
    use tooncraft_doc::rulers::{Ruler, Rulers};
    let mut e = engine();
    e.apply(Command::SetRulers { rulers: Rulers { on: true, list: vec![Ruler::Perspective { points: vec![(0.0, 0.0)] }], ..Rulers::default() } }).unwrap();
    e.apply(Command::Stroke { layer: 0, frame: 0, brush: ink(), samples: vec![Sample::at(100.0, 50.0, 1.0, 0.0)] }).unwrap();
    assert_ne!(e.pick_color(0, 100, 50), PAPER);
}

#[test]
fn samples_held_by_a_perspective_ruler_keep_their_own_pressure() {
    use tooncraft_doc::rulers::{Ruler, Rulers};
    // a horizontal stroke towards a vanishing point on the left: the first samples are held
    // while the direction is decided; light ones must stay light
    let paint = |rulers: bool| {
        let mut e = engine();
        if rulers {
            e.apply(Command::SetRulers { rulers: Rulers { on: true, list: vec![Ruler::Perspective { points: vec![(0.0, 50.0)] }], ..Rulers::default() } })
                .unwrap();
        }
        // two light samples 2.5 px apart are held; the heavy one 17.5 px on decides the direction
        let mut samples = vec![Sample::at(180.0, 50.0, 0.1, 0.0), Sample::at(177.5, 50.0, 0.1, 0.01)];
        samples.extend((0..20).map(|i| Sample::at(160.0 - i as f32 * 3.0, 50.0, 1.0, 0.02 + i as f64 * 0.01)));
        e.apply(Command::Stroke { layer: 0, frame: 0, brush: Brush { size: 20.0, ..ink() }, samples }).unwrap();
        let width = |x: u32| (0..100).filter(|y| e.pick_color(0, x, *y)[0] < 128).count();
        (width(179), width(100))
    };
    let (free, snapped) = (paint(false), paint(true));
    assert!(free.0 + 5 < free.1, "the light start is thinner without rulers: {free:?}");
    assert_eq!(snapped, free, "and just as thin when a ruler held those samples");
}
