//! Vector layers through the engine: strokes, edits as undo steps, fill and saving.

use super::*;
use crate::vector::VectorEdit;
use tooncraft_raster::fill::FillOptions;
use tooncraft_vector::VPoint;

fn engine() -> Engine {
    let mut e = Engine::new(Project::new("V", 200, 200, 24, 10).unwrap());
    e.apply(Command::SetLayerVector { layer: 0, vector: true }).unwrap();
    e
}

fn line(e: &mut Engine, pts: &[(f32, f32)], w: f32) -> Result<()> {
    e.apply(Command::VectorStroke {
        layer: 0,
        frame: 0,
        color: [0, 0, 0, 255],
        points: pts.iter().map(|(x, y)| VPoint::new(*x, *y, w)).collect(),
        tolerance: 0.6,
    })
}

fn ink(e: &Engine, x: u32, y: u32) -> bool {
    e.pick_color(0, x, y)[0] < 100
}

#[test]
fn a_vector_stroke_paints_its_line_and_is_one_undo_step() {
    let mut e = engine();
    let depth = e.undo_len();
    line(&mut e, &[(20.0, 100.0), (180.0, 100.0)], 6.0).unwrap();
    assert_eq!(e.undo_len(), depth + 1);
    assert_eq!(e.undo_label(), Some("Vector stroke"));
    let d = e.project.drawing_at(0, 0).unwrap();
    assert_eq!(e.project.vectors.get(&d).map(|v| v.strokes.len()), Some(1));
    assert!(ink(&e, 100, 100) && ink(&e, 100, 102) && !ink(&e, 100, 106), "6 px wide");
    e.undo().unwrap();
    assert!(e.project.drawing_at(0, 0).is_none(), "the new drawing goes with the undo");
    assert!(e.project.vectors.is_empty());
    e.redo().unwrap();
    assert!(ink(&e, 100, 100));
}

#[test]
fn editing_a_point_moves_the_line_as_one_undo_step() {
    let mut e = engine();
    line(&mut e, &[(20.0, 100.0), (100.0, 100.0), (180.0, 100.0)], 4.0).unwrap();
    let depth = e.undo_len();
    e.apply(Command::EditVector { layer: 0, frame: 0, edit: VectorEdit::MovePoint { stroke: 0, point: 1, x: 100.0, y: 40.0 } }).unwrap();
    assert_eq!(e.undo_len(), depth + 1);
    assert!(ink(&e, 100, 40) && !ink(&e, 100, 100));
    e.apply(Command::EditVector { layer: 0, frame: 0, edit: VectorEdit::SetWidth { stroke: 0, point: None, width: 12.0 } }).unwrap();
    assert!(ink(&e, 100, 45));
    e.apply(Command::EditVector { layer: 0, frame: 0, edit: VectorEdit::Smooth { stroke: 0, strength: 1.0, passes: 2 } }).unwrap();
    assert_eq!(e.undo_len(), depth + 3);
    for _ in 0..3 {
        e.undo().unwrap();
    }
    assert!(ink(&e, 100, 100) && !ink(&e, 100, 40));
    let before = e.project.clone();
    for bad in [
        VectorEdit::MovePoint { stroke: 4, point: 0, x: 0.0, y: 0.0 },
        VectorEdit::MovePoint { stroke: 0, point: 40, x: 0.0, y: 0.0 },
        VectorEdit::DeleteStroke { stroke: 9 },
    ] {
        assert!(e.apply(Command::EditVector { layer: 0, frame: 0, edit: bad }).is_err());
    }
    assert!(e.apply(Command::EditVector { layer: 0, frame: 5, edit: VectorEdit::DeleteStroke { stroke: 0 } }).is_err());
    assert_eq!(e.project, before, "refused edits change nothing");
}

#[test]
fn fill_reads_vector_lines_and_erase_cuts_to_crossings() {
    let mut e = engine();
    // a square drawn as four lines that overshoot their corners
    for pts in [[(40.0, 50.0), (160.0, 50.0)], [(150.0, 40.0), (150.0, 160.0)], [(160.0, 150.0), (40.0, 150.0)], [(50.0, 160.0), (50.0, 40.0)]] {
        line(&mut e, &pts, 4.0).unwrap();
    }
    e.apply(Command::Fill {
        layer: 0,
        frame: 0,
        x: 100,
        y: 100,
        color: [200, 0, 0, 255],
        options: FillOptions::default(),
        reference: FillReference::AllLayers,
    })
    .unwrap();
    assert_eq!(e.pick_color(0, 100, 100), [200, 0, 0, 255], "the fill stops at the vector lines");
    assert_ne!(e.pick_color(0, 20, 20), [200, 0, 0, 255]);
    // cutting the top line's overshoot past the right-hand line
    let mut e2 = engine();
    for pts in [[(40.0, 50.0), (160.0, 50.0)], [(150.0, 40.0), (150.0, 160.0)]] {
        line(&mut e2, &pts, 4.0).unwrap();
    }
    assert!(ink(&e2, 157, 50));
    e2.apply(Command::EditVector { layer: 0, frame: 0, edit: VectorEdit::EraseToCrossings { stroke: 0, x: 157.0, y: 50.0 } }).unwrap();
    assert!(!ink(&e2, 157, 50) && ink(&e2, 100, 50), "only the overshoot went");
    assert_eq!(e2.undo_label(), Some("Erase to crossings"));
}

#[test]
fn raster_layers_and_painted_drawings_refuse_vector_strokes() {
    let mut e = Engine::new(Project::new("R", 100, 100, 24, 4).unwrap());
    assert!(line(&mut e, &[(1.0, 1.0), (9.0, 9.0)], 2.0).is_err(), "not a vector layer");
    let samples = vec![tooncraft_brush::Sample::at(10.0, 10.0, 1.0, 0.0), tooncraft_brush::Sample::at(50.0, 50.0, 1.0, 0.1)];
    e.apply(Command::Stroke { layer: 0, frame: 0, brush: tooncraft_brush::Brush::default(), samples }).unwrap();
    e.apply(Command::SetLayerVector { layer: 0, vector: true }).unwrap();
    let before = e.project.clone();
    assert!(line(&mut e, &[(1.0, 1.0), (9.0, 9.0)], 2.0).is_err(), "the raster paint would be lost");
    assert_eq!(e.project, before);
}

#[test]
fn vector_drawings_round_trip_and_raster_shots_save_as_before() {
    let mut e = engine();
    line(&mut e, &[(20.0, 100.0), (100.0, 60.0), (180.0, 100.0)], 5.0).unwrap();
    let bytes = tooncraft_format::to_bytes(&e.project).unwrap();
    let back = tooncraft_format::from_bytes(&bytes).unwrap();
    assert_eq!(back.vectors, e.project.vectors);
    assert!(back.layers[0].vector);
    // a raster shot's project.json has no vector entries, and the vector shot's has
    let json = |bytes: &[u8]| -> String {
        let limits = craft_zip::Limits { max_entries: 1000, max_total_bytes: 1 << 30 };
        let mut zip = craft_zip::Archive::open(bytes, limits).unwrap();
        String::from_utf8(zip.read("project.json", 1 << 30).unwrap().unwrap()).unwrap()
    };
    let plain = json(&tooncraft_format::to_bytes(&Project::new("P", 64, 64, 24, 4).unwrap()).unwrap());
    assert!(!plain.contains("\"vectors\"") && !plain.contains("\"vector\""), "{plain}");
    assert!(json(&bytes).contains("\"vectors\""));
}

#[test]
fn vector_commands_read_from_json_lines() {
    let line = r#"{"cmd":"edit_vector","layer":0,"frame":2,"edit":{"edit":"move_point","stroke":1,"point":3,"x":5.0,"y":6.0}}"#;
    let cmd: Command = serde_json::from_str(line).unwrap();
    assert_eq!(cmd, Command::EditVector { layer: 0, frame: 2, edit: VectorEdit::MovePoint { stroke: 1, point: 3, x: 5.0, y: 6.0 } });
    let line = r#"{"cmd":"vector_stroke","layer":0,"frame":0,"color":[0,0,0,255],"points":[{"x":1,"y":2,"width":3}]}"#;
    assert!(matches!(serde_json::from_str::<Command>(line).unwrap(), Command::VectorStroke { tolerance, .. } if tolerance == 0.6));
}

#[test]
fn pen_strokes_on_a_vector_layer_become_vector_lines_and_the_eraser_cuts_them() {
    use tooncraft_brush::{Brush, BrushKind, Sample};
    let mut e = engine();
    let pen = |e: &mut Engine, brush: Brush, pts: &[(f32, f32)]| {
        e.begin_stroke(0, 0, brush).unwrap();
        for (i, (x, y)) in pts.iter().enumerate() {
            e.stroke_to(Sample::at(*x, *y, 1.0, i as f64 * 0.01)).unwrap();
        }
        e.end_stroke().unwrap();
    };
    let ink = Brush { size: 4.0, color: [0, 0, 0, 255], ..Brush::preset(BrushKind::Ink) };
    let depth = e.undo_len();
    pen(&mut e, ink, &(0..=60).map(|i| (40.0 + i as f32 * 2.0, 50.0)).collect::<Vec<_>>());
    pen(&mut e, ink, &(0..=60).map(|i| (150.0, 40.0 + i as f32 * 2.0)).collect::<Vec<_>>());
    assert_eq!(e.undo_len(), depth + 2, "one step per stroke");
    assert_eq!(e.undo_label(), Some("Vector stroke"));
    let d = e.project.drawing_at(0, 0).unwrap();
    let v = &e.project.vectors[&d];
    assert_eq!(v.strokes.len(), 2);
    assert!(v.strokes[0].points.len() <= 3, "a straight pen line is thinned to its ends: {:?}", v.strokes[0].points.len());
    assert!(e.pick_color(0, 157, 50)[0] < 100);
    // the eraser touching the overshoot cuts the top line back to the crossing
    pen(&mut e, Brush::preset(BrushKind::Eraser), &[(157.0, 50.0), (158.0, 50.0)]);
    assert_eq!(e.undo_label(), Some("Erase to crossings"));
    assert!(e.pick_color(0, 157, 50)[0] > 100 && e.pick_color(0, 100, 50)[0] < 100);
}
