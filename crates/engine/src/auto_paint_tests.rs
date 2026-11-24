//! Tests of painting drawings like a painted one.

use tooncraft_doc::Project;

use crate::{Command, Engine};

const INK: [u8; 4] = [0, 0, 0, 255];
const RED: [u8; 4] = [220, 30, 30, 255];
const BLUE: [u8; 4] = [30, 30, 220, 255];

fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<(f32, f32)> {
    vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
}

/// A black ring (outer box with a hole) at `dx`, on layer 0 at `frame`, cut into two rooms by a
/// wall down the middle.
fn rooms(e: &mut Engine, frame: u32, dx: f32) {
    e.apply(Command::NewDrawing { layer: 0, frame }).unwrap();
    let paint = |area: Vec<(f32, f32)>, rings: Vec<Vec<(f32, f32)>>| Command::Gradient {
        layer: 0,
        frame,
        kind: Default::default(),
        from: (0.0, 0.0),
        to: (1.0, 0.0),
        start: INK,
        end: INK,
        area: Some(area),
        area_rings: rings,
        feather: 0.0,
        stops: Vec::new(),
    };
    e.apply(paint(rect(20.0 + dx, 10.0, 120.0 + dx, 70.0), vec![rect(24.0 + dx, 14.0, 116.0 + dx, 66.0)])).unwrap();
    e.apply(paint(rect(68.0 + dx, 14.0, 72.0 + dx, 66.0), Vec::new())).unwrap();
}

fn px(e: &Engine, frame: u32, x: i64, y: i64) -> [u8; 4] {
    let p = &e.project;
    p.drawing_at(0, frame).and_then(|d| p.drawing(d)).map(|s| tooncraft_raster::unpremultiply(s.pixel(x, y))).unwrap_or([0; 4])
}

#[test]
fn the_next_drawings_take_the_colours_of_the_painted_one() {
    let mut e = Engine::new(Project::new("Auto", 160, 80, 12, 3).unwrap());
    rooms(&mut e, 0, 0.0);
    rooms(&mut e, 1, 4.0);
    rooms(&mut e, 2, 8.0);
    // paint the reference: left room red, right room blue
    let fill = |frame, x, color| Command::Fill { layer: 0, frame, x, y: 40, color, options: Default::default(), reference: Default::default() };
    e.apply(fill(0, 40, RED)).unwrap();
    e.apply(fill(0, 95, BLUE)).unwrap();
    let before = e.undo_len();
    e.apply(Command::AutoPaint { layer: 0, from: 0, frames: vec![1, 2], options: Default::default(), reference: Default::default() }).unwrap();
    assert_eq!(e.undo_len(), before + 1, "one undo step");
    assert_eq!(e.undo_label(), Some("Paint drawings alike"));
    for f in [1, 2] {
        assert_eq!(px(&e, f, 50, 40), RED, "frame {f}: left room");
        assert_eq!(px(&e, f, 105, 40), BLUE, "frame {f}: right room");
        assert_eq!(px(&e, f, 5, 5)[3], 0, "frame {f}: outside stays empty");
    }
    e.undo().unwrap();
    assert_eq!(px(&e, 1, 50, 40)[3], 0, "undone");
}

#[test]
fn bad_input_is_refused_or_ignored() {
    let mut e = Engine::new(Project::new("Auto", 64, 64, 12, 2).unwrap());
    // no reference drawing
    assert!(e.apply(Command::AutoPaint { layer: 0, from: 0, frames: vec![1], options: Default::default(), reference: Default::default() }).is_err());
    rooms(&mut e, 0, 0.0);
    // frames past the end, the reference itself and repeats are skipped; nothing to paint is fine
    e.apply(Command::AutoPaint { layer: 0, from: 0, frames: vec![0, 0, 5, 99, u32::MAX], options: Default::default(), reference: Default::default() }).unwrap();
    assert!(e.apply(Command::AutoPaint { layer: 9, from: 0, frames: vec![1], options: Default::default(), reference: Default::default() }).is_err());
    let c: Command = serde_json::from_str(r#"{"cmd":"auto_paint","layer":0,"from":0,"frames":[1]}"#).unwrap();
    assert!(matches!(c, Command::AutoPaint { .. }));
}

fn auto(e: &mut Engine, frames: Vec<u32>) -> crate::Result<()> {
    e.apply(Command::AutoPaint { layer: 0, from: 0, frames, options: Default::default(), reference: Default::default() })
}

#[test]
fn the_limits_are_refused_before_any_work() {
    use crate::auto_paint::{MAX_AUTO_DRAWINGS, MAX_AUTO_PIXELS, MAX_AUTO_WORK};
    // a canvas over the pixel cap
    let side = ((MAX_AUTO_PIXELS as f64).sqrt() as u32) + 100;
    let mut e = Engine::new(Project::new("Big", side, side, 12, 2).unwrap());
    e.apply(Command::NewDrawing { layer: 0, frame: 0 }).unwrap();
    e.apply(Command::NewDrawing { layer: 0, frame: 1 }).unwrap();
    let undo = e.undo_len();
    assert!(matches!(auto(&mut e, vec![1]), Err(crate::EngineError::Limit(_))));
    assert_eq!(e.undo_len(), undo);
    // more drawings than one call takes
    let n = MAX_AUTO_DRAWINGS as u32 + 2;
    let mut e = Engine::new(Project::new("Many", 16, 16, 12, n).unwrap());
    for f in 0..n {
        e.apply(Command::NewDrawing { layer: 0, frame: f }).unwrap();
    }
    let undo = e.undo_len();
    assert!(matches!(auto(&mut e, (1..n).collect()), Err(crate::EngineError::Limit(_))));
    assert_eq!(e.undo_len(), undo);
    // under both caps but over the total work budget: drawings × canvas pixels
    let side = 2000u32;
    let per = side as u64 * side as u64;
    let n = (MAX_AUTO_WORK / per) as u32 + 2;
    assert!((n as usize) <= MAX_AUTO_DRAWINGS && per <= MAX_AUTO_PIXELS);
    let mut e = Engine::new(Project::new("Work", side, side, 12, n + 1).unwrap());
    for f in 0..=n {
        e.apply(Command::NewDrawing { layer: 0, frame: f }).unwrap();
    }
    let undo = e.undo_len();
    let t = std::time::Instant::now();
    assert!(matches!(auto(&mut e, (1..=n).collect()), Err(crate::EngineError::Limit(_))));
    assert!(t.elapsed() < std::time::Duration::from_secs(2), "refused before any work: {:?}", t.elapsed());
    assert_eq!(e.undo_len(), undo);
}

#[test]
fn a_drawing_with_very_many_rooms_is_bounded() {
    use crate::auto_paint::MAX_AUTO_REGIONS;
    // the reference is solid red; the target is a grid of more rooms than one drawing fills
    let (side, step) = (420u32, 7.0f32);
    let mut e = Engine::new(Project::new("Grid", side, side, 12, 2).unwrap());
    let solid = |frame, area: Vec<(f32, f32)>, c| Command::Gradient {
        layer: 0,
        frame,
        kind: Default::default(),
        from: (0.0, 0.0),
        to: (1.0, 0.0),
        start: c,
        end: c,
        area: Some(area),
        area_rings: Vec::new(),
        feather: 0.0,
        stops: Vec::new(),
    };
    e.apply(Command::NewDrawing { layer: 0, frame: 0 }).unwrap();
    e.apply(solid(0, rect(0.0, 0.0, side as f32, side as f32), RED)).unwrap();
    e.apply(Command::NewDrawing { layer: 0, frame: 1 }).unwrap();
    let lines = (side as f32 / step) as u32;
    for k in 0..=lines {
        let at = k as f32 * step;
        e.apply(solid(1, rect(at, 0.0, at + 1.0, side as f32), INK)).unwrap();
        e.apply(solid(1, rect(0.0, at, side as f32, at + 1.0), INK)).unwrap();
    }
    let rooms = (lines as usize) * (lines as usize);
    assert!(rooms > MAX_AUTO_REGIONS, "{rooms} rooms");
    auto(&mut e, vec![1]).unwrap();
    // count the rooms that were filled: one sample pixel in each
    let filled = (0..lines)
        .flat_map(|j| (0..lines).map(move |i| (i, j)))
        .filter(|(i, j)| px(&e, 1, (*i as f32 * step + 3.5) as i64, (*j as f32 * step + 3.5) as i64) == RED)
        .count();
    assert!(filled > 0 && filled <= MAX_AUTO_REGIONS, "{filled} rooms filled");
}
