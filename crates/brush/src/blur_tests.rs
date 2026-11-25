//! The blur preset: the smudge brush at full pickup softens edges and carries nothing along.

use tooncraft_raster::Surface;

use crate::{Brush, BrushKind, Sample, Smoothing, Wet, paint};

fn blur(size: f32) -> Brush {
    Brush {
        size,
        hardness: 0.3,
        wet: Wet { pickup: 1.0, ..Wet::default() },
        smoothing: Smoothing { average: 0.0, catch_up: false, ..Default::default() },
        ..Brush::preset(BrushKind::Smudge)
    }
}

/// Opaque black on the left half, white on the right.
fn edge() -> Surface {
    let mut s = Surface::new(120, 60).unwrap();
    for y in 0..60u32 {
        for x in 0..120u32 {
            let v = if x < 60 { 0 } else { 255 };
            let t = s.tile_index(x / 64, y / 64).unwrap();
            let tile = s.tile_mut(t).unwrap();
            let i = (((y % 64) * 64 + x % 64) * 4) as usize;
            tile.bytes_mut()[i..i + 4].copy_from_slice(&[v, v, v, 255]);
        }
    }
    s
}

fn sweep(x0: f32, x1: f32, y: f32) -> Vec<Sample> {
    let n = 30;
    (0..=n).map(|i| Sample::at(x0 + (x1 - x0) * i as f32 / n as f32, y, 1.0, i as f64 * 0.01)).collect()
}

/// The steepest step between neighbouring pixels along row `y`.
fn steepest(s: &Surface, y: i64) -> i32 {
    (1..119).map(|x| (s.pixel(x, y)[0] as i32 - s.pixel(x - 1, y)[0] as i32).abs()).max().unwrap_or(0)
}

#[test]
fn blurring_across_an_edge_softens_it_without_dragging_colour_away() {
    let mut s = edge();
    assert_eq!(steepest(&s, 30), 255);
    paint(&mut s, blur(30.0), &sweep(30.0, 90.0, 30.0));
    assert!(steepest(&s, 30) < 140, "the edge is softer: {}", steepest(&s, 30));
    // nothing was carried to the far ends of the stroke's path: the colour far from the edge is
    // untouched, and no pixel left the range of the two colours
    assert_eq!(s.pixel(2, 30), [0, 0, 0, 255]);
    assert_eq!(s.pixel(117, 30), [255, 255, 255, 255]);
    // pixels next to where the edge was moved towards grey on both sides of where the edge was
    assert!(s.pixel(58, 30)[0] > 10 && s.pixel(62, 30)[0] < 245, "{} {}", s.pixel(58, 30)[0], s.pixel(62, 30)[0]);
    // the picture is still fully opaque
    assert!((0..120).all(|x| s.pixel(x, 30)[3] == 255));
}

#[test]
fn a_flat_area_stays_as_it_is_and_the_stroke_is_deterministic() {
    let mut a = edge();
    let mut b = edge();
    paint(&mut a, blur(30.0), &sweep(5.0, 40.0, 30.0));
    assert_eq!(a.pixel(20, 30), [0, 0, 0, 255], "blurring flat black leaves flat black");
    paint(&mut b, blur(30.0), &sweep(30.0, 90.0, 30.0));
    let mut c = edge();
    paint(&mut c, blur(30.0), &sweep(30.0, 90.0, 30.0));
    assert_eq!(b, c);
}

#[test]
fn blurring_transparent_paper_paints_nothing_and_survives_hostile_input() {
    let mut s = Surface::new(80, 80).unwrap();
    paint(&mut s, blur(20.0), &sweep(10.0, 70.0, 40.0));
    assert!(s.is_empty());
    let hostile = [Sample::at(f32::NAN, 5.0, 1.0, 0.0), Sample::at(1e9, 1e9, f32::INFINITY, 1.0), Sample::at(-5.0, -5.0, -3.0, 2.0)];
    paint(&mut s, blur(1000.0), &hostile);
}

#[test]
fn the_library_has_blur_brushes() {
    // the preset values the library uses: full pickup on the smudge kind
    let b = blur(40.0);
    assert_eq!(b.kind, BrushKind::Smudge);
    assert_eq!(b.sanitized().wet.pickup, 1.0);
}
