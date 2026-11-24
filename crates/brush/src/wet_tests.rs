//! Wet paint: water, drying, watercolour edges and bleeding.

use tooncraft_raster::{Surface, unpremultiply};

use crate::{Brush, BrushKind, Sample, Smoothing, Wet, paint};

fn wc(wet: Wet) -> Brush {
    Brush {
        size: 30.0,
        flow: 1.0,
        opacity: 1.0,
        hardness: 0.8,
        grain: 0.0,
        color: [30, 60, 200, 255],
        wet,
        smoothing: Smoothing { average: 0.0, catch_up: false, ..Default::default() },
        ..Brush::preset(BrushKind::Watercolour)
    }
}

fn row(from: f32, to: f32) -> Vec<Sample> {
    let n = ((to - from) / 2.0) as usize;
    (0..=n).map(|i| Sample::at(from + i as f32 * 2.0, 60.0, 1.0, i as f64 * 0.01)).collect()
}

fn stroke(wet: Wet, len: f32) -> Surface {
    let mut s = Surface::new(420, 120).unwrap();
    paint(&mut s, wc(wet), &row(30.0, 30.0 + len));
    s
}

const PLAIN: Wet = Wet { pickup: 0.2, dilution: 0.0, dry: 0.0, edge: 0.0, bleed: 0.0 };

#[test]
fn water_thins_the_paint() {
    let (neat, diluted) = (stroke(PLAIN, 200.0), stroke(Wet { dilution: 0.8, ..PLAIN }, 200.0));
    let (a, b) = (neat.pixel(120, 60)[3] as i32, diluted.pixel(120, 60)[3] as i32);
    assert!(b + 60 < a, "diluted {b} vs neat {a}");
    assert!(b > 0, "but still paints");
}

#[test]
fn a_drying_brush_fades_along_the_stroke() {
    let s = stroke(Wet { dry: 0.9, ..PLAIN }, 360.0);
    let (start, end) = (s.pixel(40, 60)[3] as i32, s.pixel(380, 60)[3] as i32);
    assert!(end + 80 < start, "start {start}, end {end}");
    let fresh = stroke(PLAIN, 360.0);
    assert!(fresh.pixel(380, 60)[3] as i32 > end + 60, "without drying the end keeps its paint");
}

#[test]
fn watercolour_edges_are_darker_than_the_middle() {
    let flat = stroke(PLAIN, 300.0);
    let pooled = stroke(Wet { edge: 1.0, ..PLAIN }, 300.0);
    // the stroke is about 30 px wide around y = 60: compare a point near the edge with the middle
    let edge_y = (40..60).find(|y| flat.pixel(200, *y)[3] > 150).unwrap_or(48);
    let blue = |p: [u8; 4]| unpremultiply(p)[2] as i32;
    assert!(blue(pooled.pixel(200, edge_y)) + 15 < blue(flat.pixel(200, edge_y)), "the edge pools darker");
    assert!(pooled.pixel(200, edge_y)[3] >= flat.pixel(200, edge_y)[3], "and denser");
    let mid_change = (blue(pooled.pixel(200, 60)) - blue(flat.pixel(200, 60))).abs();
    assert!(mid_change < 10, "the middle stays as it was ({mid_change})");
}

#[test]
fn bleeding_creeps_past_the_edge_and_undoes_exactly() {
    let painted = |s: &Surface| (0..420).flat_map(|x| (0..120).map(move |y| (x, y))).filter(|&(x, y)| s.pixel(x, y)[3] > 0).count();
    let plain = stroke(PLAIN, 300.0);
    let bled = stroke(Wet { bleed: 1.0, ..PLAIN }, 300.0);
    assert!(painted(&bled) > painted(&plain) + 200, "{} vs {}", painted(&bled), painted(&plain));
    assert_eq!(bled, stroke(Wet { bleed: 1.0, ..PLAIN }, 300.0), "replays exactly");
    // one undo step restores everything, the fringe too
    let mut s = Surface::new(420, 120).unwrap();
    let undo = paint(&mut s, wc(Wet { bleed: 1.0, edge: 1.0, ..PLAIN }), &row(30.0, 330.0));
    s.swap_snapshot(undo);
    s.prune();
    assert!(s.is_empty());
}

#[test]
fn hostile_wet_values_are_clamped() {
    let b = Brush { wet: Wet { pickup: f32::NAN, dilution: 9.0, dry: -3.0, edge: f32::INFINITY, bleed: f32::NAN }, ..Brush::preset(BrushKind::Watercolour) }
        .sanitized();
    assert_eq!((b.wet.dilution, b.wet.dry, b.wet.edge, b.wet.bleed), (1.0, 0.0, 0.0, 0.0));
    assert_eq!(b.wet.pickup, Brush::preset(BrushKind::Watercolour).wet.pickup);
    // old .toonbrush files have only the pickup
    let w: Wet = serde_json::from_str(r#"{"pickup": 0.4}"#).unwrap_or_default();
    assert_eq!(w, Wet { pickup: 0.4, ..Wet::default() });
}
