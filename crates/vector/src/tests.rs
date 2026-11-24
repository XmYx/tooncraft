use super::*;
use proptest::prelude::*;

fn line(points: &[(f32, f32)], w: f32) -> VStroke {
    VStroke { points: points.iter().map(|(x, y)| VPoint::new(*x, *y, w)).collect(), color: [0, 0, 0, 255], style: LineStyle::default() }
}

#[test]
fn drawn_points_are_thinned_but_keep_corners_and_width_changes() {
    // a straight run with a corner at (50, 0) and a width bump at (75, 25)
    let mut pts: Vec<VPoint> = (0..=50).map(|i| VPoint::new(i as f32, 0.0, 2.0)).collect();
    pts.extend((1..=50).map(|i| VPoint::new(50.0 + i as f32 * 0.5, i as f32 * 0.5, if i == 25 { 9.0 } else { 2.0 })));
    let s = VStroke::from_points(&pts, [0, 0, 0, 255], 0.5).unwrap();
    assert!(s.points.len() < 10, "{}", s.points.len());
    assert!(s.points.iter().any(|p| p.x == 50.0 && p.y == 0.0), "the corner stays");
    assert!(s.points.iter().any(|p| p.width == 9.0), "the width change stays");
    assert_eq!(s.points.first().map(|p| p.x), Some(0.0));
    assert!(VStroke::from_points(&[], [0; 4], 1.0).is_err());
}

#[test]
fn the_curve_passes_through_every_point() {
    let s = line(&[(0.0, 0.0), (40.0, 30.0), (80.0, 0.0), (120.0, 30.0)], 4.0);
    let smp = s.samples(1.0);
    for p in &s.points {
        assert!(smp.iter().any(|q| (q.x - p.x).abs() < 1e-3 && (q.y - p.y).abs() < 1e-3), "{p:?}");
    }
    // gaps between samples stay close to the step
    assert!(smp.windows(2).all(|w| (w[1].x - w[0].x).hypot(w[1].y - w[0].y) < 2.0));
    assert!(line(&[(0.0, 0.0), (1.0e6, 1.0e6)], 1.0).samples(0.0).len() <= MAX_SAMPLES);
}

#[test]
fn points_move_widths_change_and_smoothing_keeps_the_ends() {
    let mut d = VDrawing::default();
    d.add(line(&[(0.0, 0.0), (10.0, 20.0), (20.0, 0.0)], 2.0)).unwrap();
    d.move_point(0, 1, 10.0, 40.0).unwrap();
    assert_eq!(d.strokes[0].points[1], VPoint::new(10.0, 40.0, 2.0));
    assert!(d.move_point(0, 9, 0.0, 0.0).is_err() && d.move_point(3, 0, 0.0, 0.0).is_err());
    d.set_width(0, Some(1), 8.0).unwrap();
    d.set_width(0, None, 4.0).unwrap();
    assert_eq!(d.strokes[0].points.iter().map(|p| p.width).collect::<Vec<_>>(), [1.0, 4.0, 1.0]);
    d.smooth(0, 1.0, 1).unwrap();
    assert_eq!(d.strokes[0].points[0], VPoint::new(0.0, 0.0, 1.0));
    assert!(d.strokes[0].points[1].y < 40.0);
    assert_eq!(d.nearest_point(9.0, 1.0, 50.0), Some((0, 1)));
    assert_eq!(d.nearest_point(500.0, 500.0, 5.0), None);
    assert_eq!(d.stroke_at(20.0, 1.0, 2.0), Some(0));
}

#[test]
fn erasing_cuts_back_to_the_crossings() {
    // a horizontal line crossed by two verticals at x = 40 and x = 80
    let mut d = VDrawing::default();
    d.add(line(&[(0.0, 50.0), (120.0, 50.0)], 2.0)).unwrap();
    d.add(line(&[(40.0, 0.0), (40.0, 100.0)], 2.0)).unwrap();
    d.add(line(&[(80.0, 0.0), (80.0, 100.0)], 2.0)).unwrap();
    d.erase_to_crossings(0, 60.0, 50.0).unwrap();
    assert_eq!(d.strokes.len(), 4, "the middle piece goes, the two outer pieces stay");
    let ends: Vec<(f32, f32)> =
        d.strokes[..2].iter().map(|s| (s.points.first().map(|p| p.x).unwrap_or(-1.0), s.points.last().map(|p| p.x).unwrap_or(-1.0))).collect();
    assert!((ends[0].0 - 0.0).abs() < 0.5 && (ends[0].1 - 40.0).abs() < 0.5, "{ends:?}");
    assert!((ends[1].0 - 80.0).abs() < 0.5 && (ends[1].1 - 120.0).abs() < 0.5, "{ends:?}");
    // an end piece goes to the stroke's end
    d.erase_to_crossings(0, 10.0, 50.0).unwrap();
    assert_eq!(d.strokes.len(), 3);
    // a lone stroke goes completely
    let mut lone = VDrawing::default();
    lone.add(line(&[(0.0, 0.0), (10.0, 0.0)], 1.0)).unwrap();
    lone.erase_to_crossings(0, 5.0, 0.0).unwrap();
    assert!(lone.strokes.is_empty());
    assert!(lone.erase_to_crossings(0, 0.0, 0.0).is_err());
}

#[test]
fn the_crossing_search_is_capped() {
    let mut d = VDrawing::default();
    // two long zigzags: their sample pairs exceed the cap
    let zigzag = |y0: f32| -> Vec<(f32, f32)> { (0..3000).map(|i| (i as f32 * 3.0, y0 + if i % 2 == 0 { 0.0 } else { 3.0 })).collect() };
    d.add(line(&zigzag(0.0), 1.0)).unwrap();
    d.add(line(&zigzag(1.0), 1.0)).unwrap();
    let before = d.clone();
    assert_eq!(d.erase_to_crossings(0, 10.0, 0.0), Err(VectorError::TooMuchWork));
    assert_eq!(d, before, "a refused erase changes nothing");
}

#[test]
fn counts_are_capped() {
    let mut d = VDrawing::default();
    for _ in 0..MAX_STROKES {
        d.strokes.push(line(&[(0.0, 0.0)], 1.0));
    }
    assert_eq!(d.add(line(&[(0.0, 0.0)], 1.0)), Err(VectorError::TooManyStrokes));
    let many: Vec<VPoint> = (0..MAX_POINTS * 3).map(|i| VPoint::new(i as f32, (i % 7) as f32 * 9.0, 1.0)).collect();
    assert!(VStroke::from_points(&many, [0; 4], 0.0).unwrap().points.len() <= MAX_POINTS);
}

proptest! {
    #[test]
    fn hostile_strokes_stay_finite_and_bounded(vals in proptest::collection::vec(proptest::num::f32::ANY, 0..60), x in proptest::num::f32::ANY, y in proptest::num::f32::ANY) {
        let pts: Vec<VPoint> = vals.chunks(3).map(|c| VPoint { x: c[0], y: c.get(1).copied().unwrap_or(0.0), width: c.get(2).copied().unwrap_or(1.0) }).collect();
        let mut d = VDrawing { strokes: vec![VStroke { points: pts, color: [1, 2, 3, 4], style: LineStyle::default() }] }.sanitized();
        for s in &d.strokes {
            for p in s.samples(1.0) {
                prop_assert!(p.x.is_finite() && p.y.is_finite() && p.width.is_finite() && p.width >= 0.0);
            }
        }
        if !d.strokes.is_empty() {
            let _ = d.erase_to_crossings(0, x, y);
        }
        if !d.strokes.is_empty() {
            let _ = d.move_point(0, 0, x, y);
            let _ = d.set_width(0, None, x);
            let _ = d.smooth(0, x, 3);
        }
        for s in &d.strokes {
            prop_assert!(s.points.iter().all(|p| p.x.is_finite() && p.y.is_finite() && p.width.is_finite()));
        }
    }
}

fn fill_at(polys: &[Vec<(f32, f32)>], p: (f32, f32)) -> bool {
    // non-zero winding over all polygons
    let mut w = 0i32;
    for poly in polys {
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            if a.1 <= p.1 {
                if b.1 > p.1 && (b.0 - a.0) * (p.1 - a.1) - (p.0 - a.0) * (b.1 - a.1) > 0.0 {
                    w += 1;
                }
            } else if b.1 <= p.1 && (b.0 - a.0) * (p.1 - a.1) - (p.0 - a.0) * (b.1 - a.1) < 0.0 {
                w -= 1;
            }
        }
    }
    w != 0
}

#[test]
fn caps_end_round_flat_or_square_and_corners_round_sharp_or_cut() {
    let mut s = line(&[(0.0, 0.0), (100.0, 0.0)], 20.0);
    // 5 px past the end: inside a round or square cap, outside a flat one
    let past = (105.0, 0.0);
    assert!(fill_at(&s.outline(1.0), past), "round");
    s.style.cap = Cap::Flat;
    assert!(!fill_at(&s.outline(1.0), past), "flat");
    assert!(fill_at(&s.outline(1.0), (95.0, 0.0)), "the line itself");
    s.style.cap = Cap::Square;
    assert!(fill_at(&s.outline(1.0), (109.0, 9.0)), "square reaches the corner");
    // a right-angle corner at (100, 0): the outer corner point (109, -9) is inside a sharp
    // corner only
    // corner point (110, -10)
    let mut c = line(&[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)], 20.0);
    let near_tip = (108.0, -8.0);
    c.style.join = Join::Sharp;
    assert!(fill_at(&c.outline(0.5), near_tip), "a sharp corner fills to its point");
    c.style.join = Join::Cut;
    assert!(!fill_at(&c.outline(0.5), near_tip), "a cut corner is cut straight across");
    assert!(fill_at(&c.outline(0.5), (103.0, -6.0)), "inside the cut");
    // with round corners the curve stays smooth: it overshoots the corner instead
    c.style.join = Join::Round;
    let smooth = c.samples(0.5);
    let sharp_c = LineStyle { join: Join::Sharp, ..c.style };
    let pointed = VStroke { style: sharp_c, ..c.clone() }.samples(0.5);
    assert!(smooth.iter().any(|p| p.x > 100.5), "round: the curve swings past the corner");
    assert!(pointed.iter().all(|p| p.x <= 100.01), "sharp: it turns at the corner");
    // old files without a style read round, and round is not written
    let old: VStroke = serde_json::from_str(r#"{"points":[{"x":1,"y":2,"width":3}],"color":[0,0,0,255]}"#).unwrap();
    assert!(old.style.is_round());
    assert!(!serde_json::to_string(&old).unwrap().contains("style"));
    let flat = VStroke { style: LineStyle { cap: Cap::Flat, join: Join::Cut }, ..old };
    let back: VStroke = serde_json::from_str(&serde_json::to_string(&flat).unwrap()).unwrap();
    assert_eq!(back.style, flat.style);
}

proptest! {
    #[test]
    fn hostile_outlines_never_panic(
        pts in proptest::collection::vec((prop_oneof![Just(f32::NAN), -1.0e7f32..1.0e7], -1.0e3f32..1.0e3, prop_oneof![Just(f32::INFINITY), -5.0f32..600.0]), 0..12),
        cap in 0u8..3,
        join in 0u8..3,
    ) {
        let style = LineStyle {
            cap: [Cap::Round, Cap::Flat, Cap::Square][cap as usize],
            join: [Join::Round, Join::Sharp, Join::Cut][join as usize],
        };
        if let Some(s) = (VStroke { points: pts.into_iter().map(|(x, y, w)| VPoint { x, y, width: w }).collect(), color: [0; 4], style }).sanitized() {
            let polys = s.outline(1.0);
            prop_assert!(polys.len() <= MAX_OUTLINE_PARTS);
        }
    }
}
