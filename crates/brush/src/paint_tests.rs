//! Tests of the blend modes, colour jitter and spray count.

use tooncraft_raster::{Surface, premultiply};

use crate::{Brush, BrushKind, ColorJitter, Compose, Sample, StrokeEngine, hsv_to_rgb, paint, rgb_to_hsv};

fn sweep(y: f32) -> Vec<Sample> {
    (0..30).map(|i| Sample::at(10.0 + i as f32 * 4.0, y, 1.0, i as f64 * 0.01)).collect()
}

fn pen(compose: Compose, color: [u8; 4]) -> Brush {
    Brush { size: 16.0, hardness: 1.0, opacity: 1.0, flow: 1.0, compose, color, ..Brush::preset(BrushKind::Ink) }.sanitized()
}

fn fill(s: &mut Surface, color: [u8; 4]) {
    for ty in 0..s.rows() {
        for tx in 0..s.cols() {
            if let Some(i) = s.tile_index(tx, ty)
                && let Some(t) = s.tile_mut(i)
            {
                for px in t.bytes_mut().as_chunks_mut::<4>().0 {
                    *px = premultiply(color);
                }
            }
        }
    }
}

#[test]
fn blend_modes_mix_with_the_paint_under() {
    let base = [200, 100, 50, 255];
    let mut m = Surface::new(160, 60).unwrap();
    fill(&mut m, base);
    paint(&mut m, pen(Compose::Multiply, [128, 128, 128, 255]), &sweep(30.0));
    let p = m.pixel(60, 30);
    assert!((p[0] as i32 - 100).abs() <= 2 && (p[1] as i32 - 50).abs() <= 2, "multiply by half: {p:?}");
    let mut sc = Surface::new(160, 60).unwrap();
    fill(&mut sc, base);
    paint(&mut sc, pen(Compose::Screen, [128, 128, 128, 255]), &sweep(30.0));
    assert!(sc.pixel(60, 30)[2] > 100, "screen lightens");
    let mut d = Surface::new(160, 60).unwrap();
    fill(&mut d, base);
    paint(&mut d, pen(Compose::Darken, [255, 0, 255, 255]), &sweep(30.0));
    assert_eq!(&d.pixel(60, 30)[..3], &[200, 0, 50], "darken keeps the darker channel");
    // on an empty layer a blend mode paints like normal
    let mut e = Surface::new(160, 60).unwrap();
    paint(&mut e, pen(Compose::Multiply, [10, 200, 30, 255]), &sweep(30.0));
    assert_eq!(e.pixel(60, 30), premultiply([10, 200, 30, 255]));
    for c in Compose::ALL {
        let _ = c.blend(0.3, 0.7);
    }
}

#[test]
fn colour_jitter_varies_per_dab_or_per_stroke_and_replays() {
    let jit = |per_stroke| Brush {
        jitter: ColorJitter { hue: 0.3, saturation: 0.0, value: 0.0, per_stroke },
        color: [220, 40, 40, 255],
        ..Brush::preset(BrushKind::Ink)
    };
    let colours = |b: Brush| {
        let mut e = StrokeEngine::new(b);
        let mut out = Vec::new();
        for s in sweep(30.0) {
            out.extend(e.feed(s));
        }
        out.extend(e.finish());
        out.into_iter().map(|d| d.color).collect::<Vec<_>>()
    };
    let per_dab = colours(jit(false));
    assert!(per_dab.iter().all(|c| c.is_some()));
    let distinct: std::collections::BTreeSet<_> = per_dab.iter().collect();
    assert!(distinct.len() > 3, "per dab: many colours");
    assert_eq!(per_dab, colours(jit(false)), "replays exactly");
    let per_stroke: std::collections::BTreeSet<_> = colours(jit(true)).into_iter().collect();
    assert_eq!(per_stroke.len(), 1, "per stroke: one colour");
    // no jitter: no colour override
    assert!(colours(Brush::preset(BrushKind::Ink)).iter().all(|c| c.is_none()));
    // hostile values are clamped
    let j = ColorJitter { hue: f32::NAN, saturation: 9.0, value: -1.0, per_stroke: false }.sanitized();
    assert_eq!((j.hue, j.saturation, j.value), (0.0, 1.0, 0.0));
    // HSV round trip
    for c in [[0, 0, 0], [255, 255, 255], [10, 200, 30], [250, 3, 99]] {
        let (h, s, v) = rgb_to_hsv(c);
        assert_eq!(hsv_to_rgb(h, s, v), c);
    }
}

#[test]
fn spray_count_lays_several_dabs_per_step() {
    let count = |n: u8| {
        let mut e = StrokeEngine::new(Brush { count: n, ..Brush::preset(BrushKind::Airbrush) });
        sweep(30.0).into_iter().map(|s| e.feed(s).len()).sum::<usize>()
    };
    let (one, five) = (count(1), count(5));
    assert!(five >= one * 4, "five per step: {one} → {five}");
    assert_eq!(Brush { count: 200, ..Brush::default() }.sanitized().count, crate::MAX_COUNT);
    assert_eq!(Brush { count: 0, ..Brush::default() }.sanitized().count, 1);
}
