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
        style: Default::default(),
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

#[test]
fn a_paged_load_keeps_vector_drawings_paged() {
    use tooncraft_doc::paging::PageFile;
    let mut e = Engine::new(Project::new("P", 400, 400, 24, 40).unwrap());
    e.apply(Command::SetLayerVector { layer: 0, vector: true }).unwrap();
    for f in 0..40u32 {
        let y = 20.0 + f as f32 * 9.0;
        e.apply(Command::VectorStroke {
            layer: 0,
            frame: f,
            color: [0, 0, 0, 255],
            points: vec![VPoint::new(10.0, y, 30.0), VPoint::new(390.0, y + 5.0, 30.0)],
            tolerance: 0.6,
            style: Default::default(),
        })
        .unwrap();
    }
    let all = e.project.memory_bytes();
    let bytes = tooncraft_format::to_bytes(&e.project).unwrap();
    let file = PageFile::create(None).unwrap();
    let paged = tooncraft_format::from_bytes_with(&bytes, Some((&file, all / 8))).unwrap();
    assert_eq!(paged.vectors.len(), 40, "every vector drawing's lines are read");
    assert!(paged.paged_count() > 0 && paged.memory_bytes() <= all / 8, "{} > {}", paged.memory_bytes(), all / 8);
}

#[test]
fn duplicate_copies_the_lines_and_purge_drops_them_with_undo() {
    let mut e = engine();
    line(&mut e, &[(20.0, 100.0), (180.0, 100.0)], 4.0).unwrap();
    let src = e.project.drawing_at(0, 0).unwrap();
    e.apply(Command::DuplicateDrawing { layer: 0, frame: 0 }).unwrap();
    let copy = e.project.drawing_at(0, 1).unwrap();
    assert_ne!(copy, src);
    assert_eq!(e.project.vectors.get(&copy), e.project.vectors.get(&src));
    e.undo().unwrap();
    assert!(!e.project.vectors.contains_key(&copy), "undo takes the copy's lines away");
    e.redo().unwrap();
    // expose nothing but blanks, then purge: the lines go with their drawings, and come back
    e.apply(Command::ClearRange { layers: vec![0], from: 0, to: 9, shift: false }).unwrap();
    e.apply(Command::PurgeUnused).unwrap();
    assert!(e.project.vectors.is_empty());
    e.undo().unwrap();
    assert_eq!(e.project.vectors.len(), 2);
}

#[test]
fn a_lines_ends_and_corners_are_an_undoable_edit_and_repaint_it() {
    use tooncraft_vector::{Cap, Join, LineStyle};
    let mut e = engine();
    line(&mut e, &[(40.0, 100.0), (160.0, 100.0)], 20.0).unwrap();
    // 6 px past the end: inside the round cap
    assert!(ink(&e, 166, 100), "round end");
    let style = LineStyle { cap: Cap::Flat, join: Join::Cut };
    e.apply(Command::EditVector { layer: 0, frame: 0, edit: VectorEdit::SetStyle { stroke: 0, style } }).unwrap();
    assert_eq!(e.undo_label(), Some("Line ends and corners"));
    assert!(!ink(&e, 166, 100), "flat end stops at the point");
    assert!(ink(&e, 150, 100), "the line is still there");
    e.undo().unwrap();
    assert!(ink(&e, 166, 100), "undone: round again");
    // new lines take the engine's line style
    e.line_style = style;
    let c: Command =
        serde_json::from_str(r#"{"cmd":"edit_vector","layer":0,"frame":0,"edit":{"edit":"set_style","stroke":0,"style":{"cap":"square","join":"sharp"}}}"#)
            .unwrap();
    e.apply(c).unwrap();
    assert!(ink(&e, 166, 100), "square end reaches past the point");
}

fn pen(e: &mut Engine, pts: &[(f32, f32)]) {
    use tooncraft_brush::{Brush, BrushKind, Sample};
    let brush = Brush { size: 4.0, color: [0, 0, 0, 255], ..Brush::preset(BrushKind::Ink) };
    e.begin_stroke(0, 0, brush).unwrap();
    for (i, (x, y)) in pts.iter().enumerate() {
        e.stroke_to(Sample::at(*x, *y, 1.0, i as f64 * 0.01)).unwrap();
    }
    e.end_stroke().unwrap();
}

fn lines(e: &Engine, layer: usize, frame: u32) -> Option<usize> {
    let d = e.project.drawing_at(layer, frame)?;
    e.project.vectors.get(&d).map(|v| v.strokes.len())
}

#[test]
fn a_mirrored_pen_stroke_keeps_every_copy_as_a_line_in_one_step() {
    use tooncraft_doc::symmetry::{Symmetry, SymmetryMode};
    let mut e = engine();
    e.apply(Command::SetSymmetry { symmetry: Symmetry { mode: SymmetryMode::Vertical, ..Default::default() } }).unwrap();
    let depth = e.undo_len();
    pen(&mut e, &(0..=30).map(|i| (30.0 + i as f32, 60.0 + i as f32)).collect::<Vec<_>>());
    assert_eq!(e.undo_len(), depth + 1, "one step");
    assert_eq!(lines(&e, 0, 0), Some(2), "the line and its mirror copy");
    // the copy is a line too: across the centre (x = 100) and editable
    assert!(ink(&e, 45, 75) && ink(&e, 155, 75), "both painted");
    e.apply(Command::EditVector { layer: 0, frame: 0, edit: VectorEdit::DeleteStroke { stroke: 1 } }).unwrap();
    assert!(ink(&e, 45, 75) && !ink(&e, 155, 75), "the copy goes on its own");
    e.apply(Command::SetSymmetry { symmetry: Symmetry { mode: SymmetryMode::Both, ..Default::default() } }).unwrap();
    pen(&mut e, &[(20.0, 20.0), (40.0, 30.0)]);
    assert_eq!(lines(&e, 0, 0), Some(5), "four more copies");
}

#[test]
fn copied_layers_and_pasted_copies_keep_their_lines() {
    let mut e = engine();
    line(&mut e, &[(20.0, 100.0), (180.0, 100.0)], 4.0).unwrap();
    e.apply(Command::DuplicateLayer { layer: 0 }).unwrap();
    assert_eq!(e.project.layers.len(), 2);
    assert_eq!(lines(&e, 1, 0), Some(1), "the layer copy keeps the line");
    assert_ne!(e.project.drawing_at(0, 0), e.project.drawing_at(1, 0), "its own drawing");
    // and it stays editable on its own
    e.apply(Command::EditVector { layer: 1, frame: 0, edit: VectorEdit::MovePoint { stroke: 0, point: 0, x: 20.0, y: 20.0 } }).unwrap();
    assert_eq!(lines(&e, 0, 0), Some(1));
    e.undo().unwrap();
    e.undo().unwrap();
    assert_eq!(e.project.layers.len(), 1);
    assert_eq!(e.project.vectors.len(), 1, "the copy's lines go with the undo");
    // a pasted copy of the cell
    let d = e.project.drawing_at(0, 0).unwrap();
    e.apply(Command::PasteCells { layer: 0, frame: 3, block: vec![vec![tooncraft_doc::Cell::Key(d)]], insert: false, duplicate: true }).unwrap();
    let pasted = e.project.drawing_at(0, 3).unwrap();
    assert_ne!(pasted, d);
    assert_eq!(lines(&e, 0, 3), Some(1), "the pasted copy keeps the line");
    e.undo().unwrap();
    assert!(!e.project.vectors.contains_key(&pasted));
}

#[test]
fn a_pixel_edit_turns_the_lines_into_pixels_and_undo_brings_them_back() {
    let mut e = engine();
    for pts in [[(40.0, 50.0), (160.0, 50.0)], [(150.0, 40.0), (150.0, 160.0)], [(160.0, 150.0), (40.0, 150.0)], [(50.0, 160.0), (50.0, 40.0)]] {
        line(&mut e, &pts, 4.0).unwrap();
    }
    assert!(!e.take_lines_to_pixels());
    let depth = e.undo_len();
    let fill =
        Command::Fill { layer: 0, frame: 0, x: 100, y: 100, color: [200, 0, 0, 255], options: FillOptions::default(), reference: FillReference::AllLayers };
    e.apply(fill).unwrap();
    assert_eq!(e.undo_len(), depth + 1, "one step");
    assert!(e.take_lines_to_pixels(), "the app is told");
    assert!(!e.take_lines_to_pixels(), "once");
    assert_eq!(lines(&e, 0, 0), None, "pixels now");
    // the fill is not painted over by a later edit: a vector edit is refused instead
    let edit = Command::EditVector { layer: 0, frame: 0, edit: VectorEdit::SetWidth { stroke: 0, point: None, width: 9.0 } };
    assert!(e.apply(edit.clone()).is_err());
    assert_eq!(e.pick_color(0, 100, 100), [200, 0, 0, 255], "the fill stays");
    e.undo().unwrap();
    assert_eq!(lines(&e, 0, 0), Some(4), "undo brings the lines back");
    assert_ne!(e.pick_color(0, 100, 100), [200, 0, 0, 255]);
    e.apply(edit).unwrap();
    e.redo().ok();
    // a stroke edit after the undo starts a new history: the fill's redo is gone, the lines stay lines
    assert_eq!(lines(&e, 0, 0), Some(4));
}

#[test]
fn shapes_on_a_vector_layer_are_lines() {
    let mut e = engine();
    let brush = tooncraft_brush::Brush { size: 3.0, color: [0, 0, 0, 255], ..tooncraft_brush::Brush::preset(tooncraft_brush::BrushKind::Ink) };
    e.apply(Command::Shape { layer: 0, frame: 0, brush, shape: crate::shapes::ShapeKind::Rectangle, from: (40.0, 40.0), to: (160.0, 120.0), constrain: false })
        .unwrap();
    assert_eq!(e.undo_label(), Some("Shape"));
    let d = e.project.drawing_at(0, 0).unwrap();
    let s = &e.project.vectors[&d].strokes[0];
    assert!(s.points.len() >= 5, "the corners are kept: {}", s.points.len());
    assert!(s.points.iter().any(|p| (p.x - 160.0).abs() < 1.5 && (p.y - 120.0).abs() < 1.5), "a corner point");
    assert!(ink(&e, 100, 40) && ink(&e, 160, 80) && !ink(&e, 100, 80));
    // and editable: move a corner
    let corner = s.points.iter().position(|p| (p.x - 160.0).abs() < 1.5 && (p.y - 120.0).abs() < 1.5).unwrap();
    e.apply(Command::EditVector { layer: 0, frame: 0, edit: VectorEdit::MovePoint { stroke: 0, point: corner, x: 180.0, y: 180.0 } }).unwrap();
    assert!(!ink(&e, 160, 118));
}
