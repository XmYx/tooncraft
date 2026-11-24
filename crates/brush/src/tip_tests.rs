//! Image tips laid along the stroke: animated tips (cells), patterns and ribbons.

use std::sync::Arc;

use tooncraft_raster::Surface;

use crate::{Brush, BrushKind, Dab, Images, Sample, Smoothing, StrokeEngine, Tip, TipMode, TipOrder, paint_with};

fn plain() -> Brush {
    Brush { size: 20.0, hardness: 1.0, smoothing: Smoothing { average: 0.0, catch_up: false, ..Default::default() }, ..Brush::preset(BrushKind::Ink) }
}

fn dabs(brush: Brush, frame: u32, pts: &[Sample]) -> Vec<Dab> {
    let mut e = StrokeEngine::new(brush).at_frame(frame);
    let mut out: Vec<Dab> = pts.iter().flat_map(|s| e.feed(*s)).collect();
    out.extend(e.finish());
    out
}

fn across(n: usize) -> Vec<Sample> {
    (0..n).map(|i| Sample::at(10.0 + i as f32 * 3.0, 40.0, 1.0, i as f64 * 0.01)).collect()
}

/// A tip of `cells` square cells, cell k inked only in its column k % 8 (so cells differ).
fn strip(cells: u32) -> Arc<Tip> {
    let (w, h) = (8 * cells, 8);
    let alpha = (0..w * h).map(|i| if (i % w) / 8 == (i % w) % 8 % cells.max(1) { 255 } else { 0 }).collect();
    Arc::new(Tip::new("strip", w, h, alpha).unwrap())
}

#[test]
fn a_cell_is_read_from_its_own_part_of_the_image() {
    // two cells: the left one full, the right one empty
    let mut alpha = vec![0u8; 16 * 8];
    for y in 0..8 {
        for x in 0..8 {
            alpha[y * 16 + x] = 255;
        }
    }
    let t = Tip::new("two", 16, 8, alpha).unwrap();
    assert!(t.sample_cell(0.0, 0.0, 0, 2) > 0.99);
    assert_eq!(t.sample_cell(0.0, 0.0, 1, 2), 0.0);
    assert_eq!(t.sample_cell(0.0, 0.0, 2, 2), t.sample_cell(0.0, 0.0, 0, 2), "cells wrap");
    // more cells than pixels, or none: no panic, a sensible answer
    assert!((0.0..=1.0).contains(&t.sample_cell(0.3, 0.3, 9, 999)));
    assert_eq!(t.sample_cell(0.0, 0.0, 0, 0), t.sample(0.0, 0.0));
}

#[test]
fn cells_go_in_turn_at_random_or_by_frame() {
    let b = |order| Brush { tip_cells: 4, tip_order: order, ..plain() };
    let seq: Vec<u16> = dabs(b(TipOrder::Sequence), 0, &across(20)).iter().map(|d| d.cell).collect();
    assert_eq!(&seq[..6], &[0, 1, 2, 3, 0, 1]);
    let rnd: Vec<u16> = dabs(b(TipOrder::Random), 0, &across(40)).iter().map(|d| d.cell).collect();
    let used: std::collections::BTreeSet<u16> = rnd.iter().copied().collect();
    assert!(rnd.iter().all(|c| *c < 4) && used.len() >= 3, "{rnd:?}");
    assert!(rnd.windows(2).any(|w| w[1] != (w[0] + 1) % 4), "not in turn: {rnd:?}");
    assert_eq!(rnd, dabs(b(TipOrder::Random), 0, &across(40)).iter().map(|d| d.cell).collect::<Vec<_>>(), "replays exactly");
    let by_frame = |f| dabs(b(TipOrder::Frame), f, &across(10)).iter().map(|d| d.cell).collect::<Vec<_>>();
    assert!(by_frame(6).iter().all(|c| *c == 2) && by_frame(1).iter().all(|c| *c == 1));
    // the cells change what the stroke paints
    let img = Images { tip: Some(strip(4)), grain: None };
    let mut a = Surface::new(200, 80).unwrap();
    let mut c = Surface::new(200, 80).unwrap();
    paint_with(&mut a, b(TipOrder::Sequence), &across(40), img.clone());
    paint_with(&mut c, Brush { tip_cells: 1, ..plain() }, &across(40), img);
    assert_ne!(a, c);
}

#[test]
fn patterns_follow_the_stroke_and_lie_end_to_end() {
    let down: Vec<Sample> = (0..30).map(|i| Sample::at(40.0, 10.0 + i as f32 * 4.0, 1.0, i as f64 * 0.01)).collect();
    let d = dabs(Brush { tip_mode: TipMode::Pattern, ..plain() }, 0, &down);
    assert!(d.len() > 3);
    for w in d.windows(2).skip(1) {
        assert!((w[1].y - w[0].y - 20.0).abs() < 1.0, "one diameter apart: {}", w[1].y - w[0].y);
        assert!((w[1].angle - std::f32::consts::FRAC_PI_2).abs() < 1e-3, "turned down the stroke: {}", w[1].angle);
    }
    // a stamp keeps the pen's angle and the brush spacing
    let s = dabs(plain(), 0, &down);
    assert!(s.len() > d.len() * 4 && s.iter().all(|d| d.angle.abs() < 1e-4));
}

#[test]
fn a_ribbon_repeats_its_image_along_the_stroke_without_seams() {
    // left half ink, right half empty: a ribbon of it is a dashed band, one dash per diameter
    let mut alpha = vec![0u8; 16 * 16];
    for y in 0..16 {
        for x in 0..8 {
            alpha[y * 16 + x] = 255;
        }
    }
    let tip = Arc::new(Tip::new("half", 16, 16, alpha).unwrap());
    let mut s = Surface::new(320, 80).unwrap();
    let pts: Vec<Sample> = (0..100).map(|i| Sample::at(20.0 + i as f32 * 2.5, 40.0, 1.0, i as f64 * 0.01)).collect();
    paint_with(&mut s, Brush { tip_mode: TipMode::Ribbon, size: 20.0, ..plain() }, &pts, Images { tip: Some(tip), grain: None });
    let row: Vec<bool> = (40..240).map(|x| s.pixel(x, 40)[3] > 128).collect();
    let inked = row.iter().filter(|v| **v).count();
    assert!((70..130).contains(&inked), "about half the band is ink: {inked}");
    // dashes: runs of ink about half a diameter long, not one solid line or noise
    let changes = row.windows(2).filter(|w| w[0] != w[1]).count();
    assert!((12..30).contains(&changes), "{changes} changes along 200 px (period 20)");
}

#[test]
fn hostile_tip_settings_are_clamped() {
    let b = Brush { tip_cells: 0, ..plain() }.sanitized();
    assert_eq!(b.tip_cells, 1);
    assert_eq!(Brush { tip_cells: 255, ..plain() }.sanitized().tip_cells, crate::MAX_TIP_CELLS);
    let old: Brush = serde_json::from_str(r#"{"kind":"ink","size":5}"#).unwrap_or_default();
    assert_eq!((old.tip_mode, old.tip_cells, old.tip_order), (TipMode::Stamp, 1, TipOrder::Sequence));
}
