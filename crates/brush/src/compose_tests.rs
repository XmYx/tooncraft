//! Painting behind existing paint and with the layer's transparency locked.

use tooncraft_raster::Surface;

use crate::{Brush, BrushKind, Compose, Sample, Smoothing, paint};

fn pen(compose: Compose, color: [u8; 4]) -> Brush {
    Brush {
        size: 30.0,
        hardness: 1.0,
        compose,
        color,
        smoothing: Smoothing { average: 0.0, catch_up: false, ..Default::default() },
        ..Brush::preset(BrushKind::Ink)
    }
}

/// A horizontal sweep at height `y`.
fn sweep(y: f32) -> Vec<Sample> {
    (0..40).map(|i| Sample::at(10.0 + i as f32 * 3.0, y, 1.0, i as f64 * 0.01)).collect()
}

fn base() -> Surface {
    // a red bar across the middle, nothing elsewhere
    let mut s = Surface::new(160, 100).unwrap();
    paint(&mut s, pen(Compose::Normal, [255, 0, 0, 255]), &sweep(50.0));
    s
}

#[test]
fn normal_paints_over_and_behind_paints_under() {
    let mut over = base();
    let mut behind = base();
    // a tall blue stroke through the red bar
    let tall: Vec<Sample> = (0..40).map(|i| Sample::at(80.0, 10.0 + i as f32 * 2.0, 1.0, i as f64 * 0.01)).collect();
    paint(&mut over, pen(Compose::Normal, [0, 0, 255, 255]), &tall);
    paint(&mut behind, pen(Compose::Behind, [0, 0, 255, 255]), &tall);
    assert_eq!(over.pixel(80, 50), [0, 0, 255, 255], "blue on top");
    assert_eq!(behind.pixel(80, 50), [255, 0, 0, 255], "the red stays on top");
    assert_eq!(behind.pixel(80, 20), [0, 0, 255, 255], "blue fills the empty paper");
    assert_eq!(over.pixel(80, 20), behind.pixel(80, 20));
}

#[test]
fn behind_fills_only_the_gaps_of_a_soft_edge() {
    let mut s = Surface::new(100, 100).unwrap();
    paint(&mut s, Brush { hardness: 0.0, ..pen(Compose::Normal, [255, 0, 0, 255]) }, &sweep(50.0));
    let before: Vec<u8> = (0..100i64).map(|y| s.pixel(60, y)[3]).collect();
    paint(&mut s, pen(Compose::Behind, [0, 0, 255, 255]), &sweep(50.0));
    for (y, a) in before.iter().enumerate() {
        assert!(s.pixel(60, y as i64)[3] >= *a, "alpha never drops at row {y}");
    }
    assert_eq!(s.pixel(60, 50)[3], 255);
}

#[test]
fn locked_transparency_recolours_only_where_there_is_paint() {
    let mut s = base();
    let (inside, outside) = (s.pixel(80, 50), s.pixel(80, 10));
    assert_eq!(inside, [255, 0, 0, 255]);
    assert_eq!(outside, [0; 4]);
    let tall: Vec<Sample> = (0..40).map(|i| Sample::at(80.0, 5.0 + i as f32 * 2.0, 1.0, i as f64 * 0.01)).collect();
    paint(&mut s, pen(Compose::KeepAlpha, [0, 0, 255, 255]), &tall);
    assert_eq!(s.pixel(80, 50), [0, 0, 255, 255], "the red bar turned blue");
    assert_eq!(s.pixel(80, 10), [0; 4], "empty paper stays empty");
    // the alpha of every pixel is what it was
    let reference = base();
    for y in 0..100 {
        for x in 0..160 {
            assert_eq!(s.pixel(x, y)[3], reference.pixel(x, y)[3], "alpha changed at {x},{y}");
        }
    }
}

#[test]
fn locked_transparency_keeps_partial_alpha_and_valid_colours() {
    let mut s = Surface::new(100, 100).unwrap();
    paint(&mut s, Brush { hardness: 0.0, opacity: 0.5, ..pen(Compose::Normal, [255, 0, 0, 255]) }, &sweep(50.0));
    let before = s.clone();
    paint(&mut s, pen(Compose::KeepAlpha, [0, 255, 0, 255]), &sweep(50.0));
    for y in 0..100 {
        for x in 0..100 {
            let (a, b) = (before.pixel(x, y), s.pixel(x, y));
            assert_eq!(a[3], b[3]);
            assert!(b[0] <= b[3] && b[1] <= b[3] && b[2] <= b[3]);
        }
    }
    let p = s.pixel(50, 50);
    assert!(p[1] > p[0], "green replaced red: {p:?}");
}

#[test]
fn locked_transparency_works_for_wet_brushes_and_the_eraser_does_nothing() {
    let mut s = base();
    let before = s.clone();
    let eraser = Brush { kind: BrushKind::Eraser, compose: Compose::KeepAlpha, size: 30.0, ..Brush::preset(BrushKind::Eraser) };
    paint(&mut s, eraser, &sweep(50.0));
    assert_eq!(s, before, "an eraser cannot remove paint on a locked layer");
    let wet = Brush { compose: Compose::KeepAlpha, color: [0, 0, 255, 255], size: 30.0, ..Brush::preset(BrushKind::Watercolour) };
    let tall: Vec<Sample> = (0..40).map(|i| Sample::at(80.0, 5.0 + i as f32 * 2.0, 1.0, i as f64 * 0.01)).collect();
    paint(&mut s, wet, &tall);
    for y in 0..100 {
        for x in 0..160 {
            assert_eq!(s.pixel(x, y)[3], before.pixel(x, y)[3], "wet paint changed alpha at {x},{y}");
        }
    }
}

#[test]
fn compose_round_trips_and_hostile_values_fall_back() {
    let b = Brush { compose: Compose::Behind, ..Brush::default() };
    let json = serde_json::to_string(&b).unwrap();
    assert!(json.contains("\"compose\":\"behind\""), "{json}");
    assert_eq!(serde_json::from_str::<Brush>(&json).unwrap().compose, Compose::Behind);
    assert!(!serde_json::to_string(&Brush::default()).unwrap().contains("compose"), "the default is not written");
    assert_eq!(serde_json::from_str::<Brush>("{}").unwrap().compose, Compose::Normal);
    assert!(serde_json::from_str::<Brush>(r#"{"compose":"nonsense"}"#).is_err());
}
