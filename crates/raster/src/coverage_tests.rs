use super::*;
use proptest::prelude::*;

fn square(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<Edge> {
    vec![((x0, y0), (x1, y0)), ((x1, y0), (x1, y1)), ((x1, y1), (x0, y1)), ((x0, y1), (x0, y0))]
}

fn total(c: &Coverage) -> f32 {
    c.cells.iter().sum()
}

fn some(r: Result<Option<Coverage>, CoverageError>) -> Coverage {
    match r {
        Ok(Some(c)) => c,
        other => panic!("expected coverage, got {other:?}"),
    }
}

#[test]
fn a_square_covers_its_area_with_soft_edges() {
    let c = some(rasterize(&square(2.5, 2.0, 6.5, 5.0), (20, 20)));
    assert!((total(&c) - 12.0).abs() < 1e-3, "{}", total(&c));
    assert_eq!((c.x0, c.y0, c.width, c.height), (2, 2, 5, 3));
    assert!((c.get(0, 0) - 0.5).abs() < 1e-4, "half a pixel on the left edge");
    assert!((c.get(1, 1) - 1.0).abs() < 1e-4);
    assert!((c.get(4, 1) - 0.5).abs() < 1e-4, "half a pixel on the right edge");
}

#[test]
fn holes_and_overlaps_follow_the_non_zero_rule() {
    let mut ring = square(0.0, 0.0, 10.0, 10.0);
    // the inner square wound the other way is a hole
    ring.extend(square(3.0, 3.0, 7.0, 7.0).into_iter().map(|(a, b)| (b, a)));
    let c = some(rasterize(&ring, (20, 20)));
    assert!((total(&c) - 84.0).abs() < 1e-3);
    assert_eq!(c.get(5, 5), 0.0);
    // two squares wound the same way overlap without going over 1
    let mut two = square(0.0, 0.0, 6.0, 6.0);
    two.extend(square(3.0, 3.0, 9.0, 9.0));
    let c = some(rasterize(&two, (20, 20)));
    assert!(c.cells.iter().all(|v| *v <= 1.0));
    assert!((total(&c) - 63.0).abs() < 1e-3);
}

#[test]
fn a_triangle_and_clipping() {
    let tri = vec![((0.0, 0.0), (10.0, 0.0)), ((10.0, 0.0), (0.0, 10.0)), ((0.0, 10.0), (0.0, 0.0))];
    let c = some(rasterize(&tri, (20, 20)));
    assert!((total(&c) - 50.0).abs() < 1e-2, "{}", total(&c));
    // a square half off the canvas: only the inside part
    let c = some(rasterize(&square(-5.0, -5.0, 5.0, 5.0), (20, 20)));
    assert!((total(&c) - 25.0).abs() < 1e-3);
    assert_eq!(rasterize(&square(30.0, 30.0, 40.0, 40.0), (20, 20)), Ok(None));
    assert_eq!(rasterize(&square(0.0, 0.0, 4000.0, 4000.0), (5000, 5000)), Err(CoverageError::TooLarge));
    assert_eq!(rasterize(&[((f32::NAN, 0.0), (1.0, 1.0))], (5, 5)), Ok(None));
    let many = vec![((0.0, 0.0), (1.0, 1.0)); MAX_EDGES + 1];
    assert_eq!(rasterize(&many, (5, 5)), Err(CoverageError::TooManyEdges));
}

#[test]
fn painting_coverage_onto_a_surface() {
    let Ok(mut s) = Surface::new(64, 64) else { panic!("a 64 px surface") };
    let c = some(rasterize(&square(10.0, 10.0, 20.5, 20.0), (64, 64)));
    paint(&mut s, &c, [255, 0, 0, 255]);
    assert_eq!(s.pixel(15, 15), [255, 0, 0, 255]);
    assert_eq!(s.pixel(25, 15)[3], 0);
    let edge = s.pixel(20, 15);
    assert!(edge[3] > 100 && edge[3] < 160, "half covered: {edge:?}");
}

proptest! {
    #[test]
    fn coverage_of_a_rectangle_is_its_clipped_area(x0 in -50.0f32..80.0, y0 in -50.0f32..80.0, w in 0.0f32..60.0, h in 0.0f32..60.0) {
        let (x1, y1) = (x0 + w, y0 + h);
        let want = ((x1.min(64.0) - x0.max(0.0)).max(0.0)) * ((y1.min(64.0) - y0.max(0.0)).max(0.0));
        match rasterize(&square(x0, y0, x1, y1), (64, 64)) {
            Ok(Some(c)) => {
                prop_assert!((total(&c) - want).abs() < 1e-2 * (1.0 + want), "{} vs {}", total(&c), want);
                prop_assert!(c.cells.iter().all(|v| (0.0..=1.0).contains(v)));
            }
            Ok(None) => prop_assert!(want < 1e-3),
            Err(e) => prop_assert!(false, "{}", e),
        }
    }

    #[test]
    fn a_turned_square_keeps_its_area(cx in 20.0f32..44.0, cy in 20.0f32..44.0, r in 1.0f32..14.0, a in 0.0f32..6.3) {
        let pts: Vec<(f32, f32)> = (0..4).map(|k| {
            let t = a + std::f32::consts::FRAC_PI_2 * k as f32;
            (cx + r * t.cos(), cy + r * t.sin())
        }).collect();
        let edges: Vec<Edge> = (0..4).map(|k| (pts[k], pts[(k + 1) % 4])).collect();
        let c = some(rasterize(&edges, (64, 64)));
        let want = 2.0 * r * r;
        prop_assert!((total(&c) - want).abs() < 1e-2 * (1.0 + want), "{} vs {}", total(&c), want);
    }

    #[test]
    fn hostile_edges_never_panic(edges in proptest::collection::vec(((-1e4f32..1e4, -1e4f32..1e4), (-1e4f32..1e4, -1e4f32..1e4)), 0..40),
        nan: bool) {
        let mut edges = edges;
        if nan {
            edges.push(((f32::NAN, f32::INFINITY), (0.0, 1e30)));
        }
        if let Ok(Some(c)) = rasterize(&edges, (300, 300)) {
            prop_assert!(c.cells.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
            prop_assert!(c.width as u64 * c.height as u64 <= MAX_AREA);
        }
    }
}
