use super::*;
use proptest::prelude::*;

fn on(list: Vec<Ruler>) -> Rulers {
    Rulers { on: true, list, ..Rulers::default() }
}

fn run(r: &Rulers, pts: &[Point]) -> Vec<Point> {
    let Some(mut s) = r.snapper(1000, 800) else { return pts.to_vec() };
    let mut out: Vec<Point> = pts.iter().flat_map(|p| s.feed(*p)).collect();
    out.extend(s.finish());
    out
}

fn line_dist(a: Point, b: Point, p: Point) -> f32 {
    let d = unit(a, b);
    ((p.0 - a.0) * d.1 - (p.1 - a.1) * d.0).abs()
}

#[test]
fn a_stroke_near_a_straight_ruler_runs_along_it() {
    let r = on(vec![Ruler::Line { a: (100.0, 100.0), b: (500.0, 300.0) }]);
    let wobbly: Vec<Point> = (0..50).map(|i| (110.0 + i as f32 * 6.0, 100.0 + i as f32 * 3.0 + (i % 3) as f32 * 4.0)).collect();
    let out = run(&r, &wobbly);
    assert_eq!(out.len(), wobbly.len());
    assert!(out.iter().all(|p| line_dist((100.0, 100.0), (500.0, 300.0), *p) < 1e-2));
    // far from the ruler: freehand
    let far = run(&r, &[(100.0, 600.0), (130.0, 610.0)]);
    assert_eq!(far, vec![(100.0, 600.0), (130.0, 610.0)]);
}

#[test]
fn ellipse_and_concentric_rulers() {
    let e = on(vec![Ruler::Ellipse { center: (500.0, 400.0), rx: 200.0, ry: 100.0, angle: 0.0 }]);
    let out = run(&e, &[(705.0, 400.0), (690.0, 450.0), (500.0, 520.0)]);
    for p in &out {
        let v = ((p.0 - 500.0) / 200.0).powi(2) + ((p.1 - 400.0) / 100.0).powi(2);
        assert!((v - 1.0).abs() < 0.01, "{p:?} is on the ellipse ({v})");
    }
    // concentric: the ellipse through the first sample, at any distance
    let c = on(vec![Ruler::Concentric { center: (500.0, 400.0), ratio: 0.5, angle: 0.0 }]);
    let out = run(&c, &[(800.0, 400.0), (700.0, 520.0), (500.0, 560.0)]);
    for p in &out {
        let v = ((p.0 - 500.0) / 300.0).powi(2) + ((p.1 - 400.0) / 150.0).powi(2);
        assert!((v - 1.0).abs() < 0.01, "{p:?} is on the ellipse through the first sample ({v})");
    }
}

#[test]
fn perspective_picks_the_vanishing_point_by_direction() {
    let r = on(vec![Ruler::Perspective { points: vec![(0.0, 400.0), (1000.0, 0.0)] }]);
    // moving towards the left-hand point
    let out = run(&r, &[(500.0, 400.0), (497.0, 401.0), (490.0, 398.0), (480.0, 405.0)]);
    assert_eq!(out.len(), 4, "the held samples are released once the pen moves");
    assert!(out.iter().all(|p| (p.1 - 400.0).abs() < 1e-3), "{out:?}");
    // moving towards the upper-right point: on the line from (1000, 0) through the start
    let out = run(&r, &[(500.0, 500.0), (505.0, 496.0), (520.0, 480.0)]);
    assert!(out.iter().all(|p| line_dist((1000.0, 0.0), (500.0, 500.0), *p) < 1e-2), "{out:?}");
    // a tap: held samples come out at the end
    assert_eq!(run(&r, &[(500.0, 400.0)]).len(), 1);
}

#[test]
fn grid_snapping_for_freehand_strokes() {
    let r = Rulers { on: true, grid: true, reach: 10.0, ..Rulers::default() };
    // grid lines every 1000/24 px across; start near x = 500 (the centre line), move down
    let out = run(&r, &[(503.0, 100.0), (506.0, 120.0), (498.0, 200.0)]);
    assert!(out.iter().all(|p| (p.0 - 500.0).abs() < 1e-3), "{out:?}");
    // halfway between grid lines in both directions (beyond reach): freehand
    let (sx, sy) = (1000.0 / 24.0, 800.0 / 24.0);
    let start = (500.0 + sx / 2.0, 400.0 + sy / 2.0);
    let out = run(&r, &[start, (530.0, 430.0)]);
    assert_eq!(out, vec![start, (530.0, 430.0)]);
}

#[test]
fn the_nearest_ruler_wins_and_curves_pass_through_points() {
    let r = on(vec![
        Ruler::Line { a: (0.0, 100.0), b: (1000.0, 100.0) },
        Ruler::Line { a: (0.0, 120.0), b: (1000.0, 120.0) },
        Ruler::Curve { points: vec![(100.0, 500.0), (300.0, 600.0), (500.0, 500.0)] },
    ]);
    assert!(run(&r, &[(50.0, 117.0), (90.0, 105.0)]).iter().all(|p| (p.1 - 120.0).abs() < 1e-3));
    let out = run(&r, &[(300.0, 590.0), (300.0, 610.0)]);
    assert!(out.iter().all(|p| (p.0 - 300.0).abs() < 2.0 && (p.1 - 600.0).abs() < 1.0), "{out:?}");
}

#[test]
fn hostile_rulers_are_dropped_or_clamped() {
    let r = Rulers {
        on: true,
        reach: f32::NAN,
        grid: false,
        list: vec![
            Ruler::Line { a: (f32::NAN, 0.0), b: (1.0, 1.0) },
            Ruler::Line { a: (5.0, 5.0), b: (5.0, 5.0) },
            Ruler::Curve { points: vec![(0.0, 0.0); 500] },
            Ruler::Ellipse { center: (0.0, 0.0), rx: -1e9, ry: 0.0, angle: f32::INFINITY },
            Ruler::Ellipse { center: (0.0, 0.0), rx: -1e4, ry: 0.0, angle: 7.0 },
            Ruler::Concentric { center: (1e9, 0.0), ratio: 1.0, angle: 0.0 },
            Ruler::Perspective { points: vec![] },
            Ruler::Perspective { points: vec![(1.0, 1.0); 9] },
        ],
    }
    .sanitized();
    assert_eq!(r.reach, 24.0);
    assert_eq!(r.list.len(), 3, "{:?}", r.list);
    assert!(matches!(&r.list[0], Ruler::Curve { points } if points.len() == MAX_CURVE_POINTS));
    assert!(matches!(r.list[1], Ruler::Ellipse { rx, ry, .. } if rx == 1.0e4 && ry == 1.0));
    assert!(matches!(&r.list[2], Ruler::Perspective { points } if points.len() == MAX_VANISHING));
    let many = Rulers { list: vec![Ruler::Line { a: (0.0, 0.0), b: (1.0, 0.0) }; 100], ..Rulers::default() }.sanitized();
    assert_eq!(many.list.len(), MAX_RULERS);
    assert!(Rulers { on: false, ..many }.snapper(10, 10).is_none());
}

#[test]
fn a_stroke_that_never_moves_holds_a_bounded_number_of_samples() {
    let r = on(vec![Ruler::Perspective { points: vec![(0.0, 0.0)] }]);
    let Some(mut s) = r.snapper(100, 100) else { panic!("rulers are on") };
    let mut out = 0;
    let mut most_held = 0;
    for _ in 0..10_000 {
        out += s.feed((50.0, 50.0)).len();
        if let State::Deciding { held, .. } = &s.state {
            most_held = most_held.max(held.len());
        }
    }
    out += s.finish().len();
    assert_eq!(out, 10_000, "every sample comes out once");
    assert!(most_held < MAX_PENDING, "held at most {most_held}");
}

#[test]
fn handles_edit_the_ruler() {
    let mut e = Ruler::Ellipse { center: (0.0, 0.0), rx: 10.0, ry: 5.0, angle: 0.0 };
    e.move_handle(1, (0.0, 20.0));
    assert!(matches!(e, Ruler::Ellipse { rx, angle, .. } if (rx - 20.0).abs() < 1e-4 && (angle - std::f32::consts::FRAC_PI_2).abs() < 1e-4));
    let h = e.handles();
    assert!((h[1].0).abs() < 1e-3 && (h[1].1 - 20.0).abs() < 1e-3);
    let mut l = Ruler::Line { a: (0.0, 0.0), b: (1.0, 0.0) };
    l.move_handle(1, (f32::NAN, 3.0));
    l.move_handle(7, (3.0, 3.0));
    assert_eq!(l, Ruler::Line { a: (0.0, 0.0), b: (1.0, 0.0) });
    assert!(!Ruler::Perspective { points: vec![(5.0, 5.0)] }.outline(100, 100).is_empty());
}

fn finite() -> impl Strategy<Value = f32> {
    -2000.0f32..2000.0
}

fn any_f32() -> impl Strategy<Value = f32> {
    prop_oneof![finite(), Just(f32::NAN), Just(f32::INFINITY), Just(-1e30f32), Just(0.0)]
}

fn ruler() -> impl Strategy<Value = Ruler> {
    let p = || (any_f32(), any_f32());
    prop_oneof![
        (p(), p()).prop_map(|(a, b)| Ruler::Line { a, b }),
        proptest::collection::vec(p(), 0..80).prop_map(|points| Ruler::Curve { points }),
        (p(), any_f32(), any_f32(), any_f32()).prop_map(|(center, rx, ry, angle)| Ruler::Ellipse { center, rx, ry, angle }),
        (p(), any_f32(), any_f32()).prop_map(|(center, ratio, angle)| Ruler::Concentric { center, ratio, angle }),
        proptest::collection::vec(p(), 0..6).prop_map(|points| Ruler::Perspective { points }),
    ]
}

proptest! {
    #[test]
    fn snapped_samples_lie_on_a_straight_ruler(ax in finite(), ay in finite(), bx in finite(), by in finite(),
        pts in proptest::collection::vec((finite(), finite()), 1..40)) {
        prop_assume!(dist((ax, ay), (bx, by)) > 1.0);
        let r = Rulers { reach: 500.0, ..on(vec![Ruler::Line { a: (ax, ay), b: (bx, by) }]) };
        prop_assume!(line_dist((ax, ay), (bx, by), pts[0]) <= 500.0);
        let out = run(&r, &pts);
        prop_assert_eq!(out.len(), pts.len());
        for p in out {
            prop_assert!(line_dist((ax, ay), (bx, by), p) < 0.05, "{:?}", p);
        }
    }

    #[test]
    fn snapped_samples_lie_on_an_ellipse_ruler(rx in 5.0f32..800.0, ry in 5.0f32..800.0, angle in -7.0f32..7.0,
        pts in proptest::collection::vec((-1000.0f32..1000.0, -1000.0f32..1000.0), 1..30)) {
        let r = Rulers { reach: 1.0e5, ..on(vec![Ruler::Ellipse { center: (0.0, 0.0), rx, ry, angle }]) };
        let (s, c) = (-angle).sin_cos();
        for p in run(&r, &pts) {
            let (u, v) = (p.0 * c - p.1 * s, p.0 * s + p.1 * c);
            let e = (u / rx).powi(2) + (v / ry).powi(2);
            // on the polyline through points of the ellipse: within the chord sag of 1
            prop_assert!((e - 1.0).abs() < 0.02, "{:?}: {}", p, e);
        }
    }

    #[test]
    fn hostile_rulers_and_samples_never_panic(list in proptest::collection::vec(ruler(), 0..8), grid: bool,
        pts in proptest::collection::vec((any_f32(), any_f32()), 0..30)) {
        let r = Rulers { on: true, list, grid, reach: 30.0 }.sanitized();
        let out = run(&r, &pts);
        prop_assert_eq!(out.len(), pts.len(), "every sample comes out once");
        // held samples may come out after a later non-finite one, so count rather than zip
        let good_in = pts.iter().filter(|p| ok_point(**p)).count();
        let good_out = out.iter().filter(|p| p.0.is_finite() && p.1.is_finite()).count();
        prop_assert!(good_out >= good_in, "{} finite in, {} out", good_in, good_out);
    }
}
