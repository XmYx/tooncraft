//! Drawing aids beyond the brush: path shapes (polyline, polygon, smooth curve), filled shapes and
//! lasso fill, gradients, and the recogniser behind "hold to straighten" (a freehand stroke held
//! still at its end becomes a clean line or ellipse).

use serde::{Deserialize, Serialize};
use tooncraft_brush::{Brush, Sample};
use tooncraft_raster::{fill, premultiply, select, unpremultiply};

use crate::shapes::ShapeKind;
use crate::undo::{Entry, Part};
use crate::{Command, Engine, EngineError, Result};

/// Most points in one path shape.
pub const MAX_PATH_POINTS: usize = 4096;

/// Points along a Catmull–Rom curve through `pts` (closed loops wrap around), about every 2 px.
pub fn curve_points(pts: &[(f32, f32)], closed: bool) -> Vec<(f32, f32)> {
    let n = pts.len();
    if n < 3 {
        return pts.to_vec();
    }
    let get = |i: isize| -> (f32, f32) { if closed { pts[i.rem_euclid(n as isize) as usize] } else { pts[i.clamp(0, n as isize - 1) as usize] } };
    let segs = if closed { n } else { n - 1 };
    let mut out = Vec::new();
    for i in 0..segs as isize {
        let (p0, p1, p2, p3) = (get(i - 1), get(i), get(i + 1), get(i + 2));
        let len = (p2.0 - p1.0).hypot(p2.1 - p1.1);
        let steps = (len / 2.0).ceil().clamp(1.0, 2000.0) as usize;
        for k in 0..steps {
            let t = k as f32 / steps as f32;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f32, b: f32, c: f32, d: f32| 0.5 * (2.0 * b + (c - a) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (3.0 * b - a - 3.0 * c + d) * t3);
            out.push((f(p0.0, p1.0, p2.0, p3.0), f(p0.1, p1.1, p2.1, p3.1)));
        }
    }
    out.push(if closed { pts[0] } else { pts[n - 1] });
    out
}

/// The outline of a path: straight segments or a curve, closed back to the start when `closed`.
pub fn path_outline(pts: &[(f32, f32)], closed: bool, curve: bool) -> Vec<(f32, f32)> {
    if curve {
        return curve_points(pts, closed);
    }
    let mut v = pts.to_vec();
    if closed && let Some(f) = pts.first() {
        v.push(*f);
    }
    v
}

fn samples_along(line: &[(f32, f32)], pressure: f32) -> Vec<Sample> {
    let mut out = Vec::new();
    let mut time = 0.0;
    for w in line.windows(2) {
        let n = ((w[1].0 - w[0].0).hypot(w[1].1 - w[0].1) / 2.0).ceil().clamp(1.0, 20_000.0) as usize;
        for k in 0..n {
            let t = k as f32 / n as f32;
            out.push(Sample { x: w[0].0 + (w[1].0 - w[0].0) * t, y: w[0].1 + (w[1].1 - w[0].1) * t, pressure, tilt_x: 0.0, tilt_y: 0.0, time, rotation: 0.0 });
            time += 0.002;
        }
    }
    if let Some(l) = line.last() {
        out.push(Sample { x: l.0, y: l.1, pressure, tilt_x: 0.0, tilt_y: 0.0, time, rotation: 0.0 });
    }
    out
}

fn finite(pts: &[(f32, f32)]) -> bool {
    pts.iter().all(|p| p.0.is_finite() && p.1.is_finite() && p.0.abs() < 1e6 && p.1.abs() < 1e6)
}

/// Fill the inside of `outline` with straight RGBA `color` on the drawing at the frame (a new
/// drawing on a blank cell). Returns the undo parts.
fn fill_polygon(e: &mut Engine, layer: usize, frame: u32, outline: &[(f32, f32)], color: [u8; 4]) -> Result<Vec<Part>> {
    let (w, h) = (e.project.width(), e.project.height());
    let Some(mask) = select::polygon_mask(w, h, outline) else { return Ok(Vec::new()) };
    let (drawing, mut parts) = e.ensure_drawing(layer, frame)?;
    let s = e.project.drawing_mut(drawing).ok_or(EngineError::NoDrawing(frame))?;
    s.begin_capture();
    fill::paint_over(s, &mask, premultiply(color));
    parts.push(Part::Tiles { drawing, tiles: s.take_capture() });
    Ok(parts)
}

/// Merge the last `n` undo entries into one with `label`.
pub(crate) fn merge_last(e: &mut Engine, n: usize, label: &str) {
    if n < 2 || e.undo.len() < n {
        if let Some(top) = e.undo.last_mut() {
            top.label = label.into();
        }
        return;
    }
    let tail: Vec<Entry> = e.undo.split_off(e.undo.len() - n);
    let parts = tail.into_iter().flat_map(|t| t.parts).collect();
    e.undo.push(Entry { label: label.into(), parts });
}

/// A path shape: outline drawn with the brush, filled, or both (one undo step).
pub(crate) fn path_shape(
    e: &mut Engine,
    layer: usize,
    frame: u32,
    brush: Brush,
    points: &[(f32, f32)],
    closed: bool,
    curve: bool,
    outline: bool,
    fill_color: Option<[u8; 4]>,
) -> Result<()> {
    if points.len() < 2 || points.len() > MAX_PATH_POINTS || !finite(points) {
        return Err(EngineError::Import("a path needs 2 to 4096 finite points".into()));
    }
    e.check_unlocked(layer)?;
    let line = path_outline(points, closed, curve);
    let before = e.undo.len();
    if let Some(c) = fill_color {
        let parts = fill_polygon(e, layer, frame, &line, c)?;
        if !parts.is_empty() {
            e.push(Entry { label: "Fill shape".into(), parts });
        }
    }
    if outline {
        let brush = Brush { smoothing: tooncraft_brush::Smoothing { average: 0.0, string: 0.0, catch_up: true }, ..brush };
        crate::command::apply(e, Command::Stroke { layer, frame, brush, samples: samples_along(&line, 1.0) })?;
    }
    let n = e.undo.len().saturating_sub(before);
    merge_last(e, n, "Shape");
    Ok(())
}

/// Linear or radial gradient between two straight RGBA colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GradientKind {
    #[default]
    Linear,
    Radial,
}

/// Paint a gradient from `from` to `to` over the drawing, limited to `area` when given (a
/// selection outline), otherwise the whole canvas.
pub(crate) fn gradient(
    e: &mut Engine,
    layer: usize,
    frame: u32,
    kind: GradientKind,
    from: (f32, f32),
    to: (f32, f32),
    stops: &[tooncraft_doc::palette::Stop],
    area: Option<&[(f32, f32)]>,
    area_rings: &[Vec<(f32, f32)>],
    feather: f32,
) -> Result<()> {
    if !finite(&[from, to]) {
        return Err(EngineError::Import("gradient ends must be finite".into()));
    }
    e.check_unlocked(layer)?;
    let (w, h) = (e.project.width(), e.project.height());
    let full = [(0.0, 0.0), (w as f32, 0.0), (w as f32, h as f32), (0.0, h as f32)];
    let outline: Vec<(f32, f32)> = match area {
        Some(a) if a.len() >= 3 && finite(a) => a.to_vec(),
        _ => full.to_vec(),
    };
    let mut rings: Vec<&[(f32, f32)]> = vec![&outline];
    if area.is_some() {
        rings.extend(area_rings.iter().filter(|r| finite(r)).map(|r| r.as_slice()).take(crate::select::MAX_RINGS));
    }
    let Some(mask) = select::rings_mask(w, h, &rings) else { return Ok(()) };
    // a feather softens the selection's edge only (a whole-canvas gradient has none)
    let Some(mask) = (if area.is_some() { select::feather(&mask, feather, w, h) } else { Some(mask) }) else { return Ok(()) };
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let len2 = (dx * dx + dy * dy).max(1e-6);
    let lerp = |t: f32| -> [u8; 4] { premultiply(tooncraft_doc::palette::stop_colour(stops, t)) };
    let (drawing, mut parts) = e.ensure_drawing(layer, frame)?;
    let s = e.project.drawing_mut(drawing).ok_or(EngineError::NoDrawing(frame))?;
    s.begin_capture();
    let t = tooncraft_raster::TILE;
    for ty in mask.y / t..=(mask.y + mask.height - 1) / t {
        for tx in mask.x / t..=(mask.x + mask.width - 1) / t {
            if !(0..t).any(|r| (0..t).any(|c| mask.get(tx * t + c, ty * t + r))) {
                continue;
            }
            let Some(index) = s.tile_index(tx, ty) else { continue };
            let Some(tile) = s.tile_mut(index) else { continue };
            let px = tile.bytes_mut();
            for r in 0..t {
                for c in 0..t {
                    let (x, y) = (tx * t + c, ty * t + r);
                    let m = mask.value(x, y);
                    if m == 0 {
                        continue;
                    }
                    let (px_x, px_y) = (x as f32 + 0.5 - from.0, y as f32 + 0.5 - from.1);
                    let f = match kind {
                        GradientKind::Linear => (px_x * dx + px_y * dy) / len2,
                        GradientKind::Radial => (px_x * px_x + px_y * px_y).sqrt() / len2.sqrt(),
                    };
                    let i = ((r * t + c) * 4) as usize;
                    let out = tooncraft_raster::over([px[i], px[i + 1], px[i + 2], px[i + 3]], tooncraft_raster::fill::weigh(lerp(f), m));
                    px[i..i + 4].copy_from_slice(&out);
                }
            }
        }
    }
    parts.push(Part::Tiles { drawing, tiles: s.take_capture() });
    e.push(Entry { label: "Gradient".into(), parts });
    Ok(())
}

/// What a freehand stroke looks like it meant: a straight line or an ellipse, with its two
/// corners (as the shape tools take them). `None` when it is neither.
pub fn recognize(points: &[(f32, f32)]) -> Option<(ShapeKind, (f32, f32), (f32, f32))> {
    if points.len() < 4 || !finite(points) {
        return None;
    }
    let length: f32 = points.windows(2).map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1)).sum();
    if length < 8.0 {
        return None;
    }
    let (a, b) = (points[0], points[points.len() - 1]);
    let chord = (b.0 - a.0).hypot(b.1 - a.1);
    // a line: every point close to the chord, and the path not much longer than it
    if chord > length * 0.8 {
        let (nx, ny) = ((b.1 - a.1) / chord, -(b.0 - a.0) / chord);
        let dev = points.iter().map(|p| ((p.0 - a.0) * nx + (p.1 - a.1) * ny).abs()).fold(0.0f32, f32::max);
        if dev <= (chord * 0.05).max(3.0) {
            return Some((ShapeKind::Line, a, b));
        }
    }
    // an ellipse: the path closes on itself and stays near the ellipse of its bounding box
    if chord < length * 0.2 {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in points {
            x0 = x0.min(p.0);
            y0 = y0.min(p.1);
            x1 = x1.max(p.0);
            y1 = y1.max(p.1);
        }
        let (cx, cy, rx, ry) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0, ((x1 - x0) / 2.0).max(1.0), ((y1 - y0) / 2.0).max(1.0));
        let err = points.iter().map(|p| ((((p.0 - cx) / rx).powi(2) + ((p.1 - cy) / ry).powi(2)).sqrt() - 1.0).abs()).sum::<f32>() / points.len() as f32;
        if err < 0.12 {
            return Some((ShapeKind::Ellipse, (x0, y0), (x1, y1)));
        }
    }
    None
}

/// Snap a point to the nearest grid line within `reach` pixels on each axis.
pub fn snap_to(p: (f32, f32), xs: &[f32], ys: &[f32], reach: f32) -> (f32, f32) {
    let near =
        |v: f32, lines: &[f32]| lines.iter().copied().filter(|l| (l - v).abs() <= reach).min_by(|a, b| (a - v).abs().total_cmp(&(b - v).abs())).unwrap_or(v);
    (near(p.0, xs), near(p.1, ys))
}

/// Recolour every drawing of `layer`: pixels within `tolerance` of `from` become `to`, with their
/// own opacity. One undo step; nothing to change is not an error (and no step).
pub(crate) fn recolour(e: &mut Engine, layer: usize, from: [u8; 3], to: [u8; 3], tolerance: u8) -> Result<()> {
    e.check_unlocked(layer)?;
    let drawings = layer_drawings(e, layer)?;
    let near = |p: [u8; 4]| {
        let u = unpremultiply(p);
        u[3] > 0 && (0..3).all(|k| u[k].abs_diff(from[k]) <= tolerance)
    };
    let parts = recolour_drawings(e, &drawings, near, to);
    if !parts.is_empty() {
        e.push(Entry { label: "Recolour".into(), parts });
    }
    Ok(())
}

/// The drawings a layer shows, each once, in frame order.
fn layer_drawings(e: &Engine, layer: usize) -> Result<Vec<tooncraft_doc::DrawingId>> {
    let l = e.project.layer(layer)?;
    let mut drawings: Vec<tooncraft_doc::DrawingId> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for f in 0..e.project.length() {
        if let Some(d) = l.drawing_at(f)
            && seen.insert(d)
        {
            drawings.push(d);
        }
    }
    Ok(drawings)
}

/// In `drawings`, give the pixels `near` accepts (it sees premultiplied pixels) the colour `to`,
/// keeping their opacity. Returns the undo parts (empty when nothing matched). Tiles without a
/// match are only read, so the work is one pass over the drawings' painted tiles.
fn recolour_drawings(e: &mut Engine, drawings: &[tooncraft_doc::DrawingId], near: impl Fn([u8; 4]) -> bool, to: [u8; 3]) -> Vec<Part> {
    let mut parts = Vec::new();
    for d in drawings {
        let Some(s) = e.project.drawing_mut(*d) else { continue };
        let indices: Vec<u32> = s.tiles().map(|(i, _)| i).collect();
        s.begin_capture();
        for i in indices {
            let hit = s.tile(i).is_some_and(|t| t.bytes().as_chunks::<4>().0.iter().any(|p| near(*p)));
            if !hit {
                continue;
            }
            let Some(t) = s.tile_mut(i) else { continue };
            for px in t.bytes_mut().as_chunks_mut::<4>().0 {
                if near(*px) {
                    *px = premultiply([to[0], to[1], to[2], px[3]]);
                }
            }
        }
        let tiles = s.take_capture();
        if !tiles.is_empty() {
            parts.push(Part::Tiles { drawing: *d, tiles });
        }
    }
    parts
}

/// Fill with a swatch's colour and link the filled pixels to the swatch (see
/// [`tooncraft_doc::palette::FillLink`]). One undo step for the paint and the link.
#[allow(clippy::too_many_arguments)]
pub(crate) fn swatch_fill(
    e: &mut Engine,
    (palette, swatch): (usize, usize),
    layer: usize,
    frame: u32,
    at: (u32, u32),
    options: tooncraft_raster::fill::FillOptions,
    reference: crate::FillReference,
    all_frames: bool,
) -> Result<()> {
    let color = e.project.palettes.get(palette).and_then(|p| p.swatches.get(swatch)).map(|s| s.rgba()).ok_or(EngineError::NoSwatch(palette, swatch))?;
    let frames: Vec<u32> = if all_frames {
        let l = e.project.layer(layer)?;
        let mut seen = std::collections::BTreeSet::new();
        (0..e.project.length()).filter(|f| l.drawing_at(*f).is_some_and(|d| seen.insert(d))).collect()
    } else {
        vec![frame]
    };
    let mut parts = Vec::new();
    for f in frames {
        if let Some(p) = e.fill_parts_for(layer, f, at, color, options, reference)? {
            parts.extend(p);
        }
    }
    if parts.is_empty() {
        return Ok(());
    }
    // the pixels each fill changed: compare the tiles before (the undo snapshot) and now
    let mut links: Vec<tooncraft_doc::palette::FillLink> = Vec::new();
    for part in &parts {
        let Part::Tiles { drawing, tiles } = part else { continue };
        let mut runs = Vec::new();
        let Some(s) = e.project.drawing(*drawing) else { continue };
        let t = tooncraft_raster::TILE;
        for (index, old) in tiles {
            let (tx, ty) = s.tile_coords(*index);
            let now = s.tile(*index);
            let px = |tile: Option<&std::sync::Arc<tooncraft_raster::Tile>>, i: usize| {
                tile.and_then(|t| t.bytes().get(i..i + 4).map(|b| [b[0], b[1], b[2], b[3]])).unwrap_or([0; 4])
            };
            let changed = |x: u32, y: u32| {
                let i = (((y - ty * t) * t + (x - tx * t)) * 4) as usize;
                px(old.as_ref(), i) != px(now, i)
            };
            if let Some(l) = tooncraft_doc::palette::FillLink::from_pixels(*drawing, (tx * t, ty * t, t, t), changed) {
                runs.extend(l.runs);
            }
        }
        // one link per drawing and fill; runs that touch across tile edges join up
        runs.sort_unstable();
        let mut joined: Vec<(u32, u32, u32)> = Vec::with_capacity(runs.len());
        for (y, x, n) in runs {
            match joined.last_mut() {
                Some(last) if last.0 == y && last.1.saturating_add(last.2) == x => last.2 = last.2.saturating_add(n),
                _ => joined.push((y, x, n)),
            }
        }
        let mut runs = joined;
        if !runs.is_empty() {
            runs.truncate(tooncraft_doc::palette::MAX_LINK_RUNS);
            links.push(tooncraft_doc::palette::FillLink { drawing: *drawing, runs });
        }
    }
    // only this swatch's links go into the undo step (not a copy of every palette)
    let Some(s) = e.project.palettes.get_mut(palette).and_then(|p| p.swatches.get_mut(swatch)) else { return Err(EngineError::NoSwatch(palette, swatch)) };
    let before = s.links.clone();
    s.links.extend(links);
    let extra = s.links.len().saturating_sub(tooncraft_doc::palette::MAX_LINKS);
    s.links = s.links.split_off(extra);
    // the project-wide budget: drop this swatch's oldest links until everything fits
    let total = tooncraft_doc::palette::link_runs(&e.project.palettes);
    if total > tooncraft_doc::palette::MAX_TOTAL_LINK_RUNS
        && let Some(s) = e.project.palettes.get_mut(palette).and_then(|p| p.swatches.get_mut(swatch))
    {
        let mut over = total - tooncraft_doc::palette::MAX_TOTAL_LINK_RUNS;
        let mut drop = 0;
        for l in &s.links {
            if over == 0 {
                break;
            }
            over = over.saturating_sub(l.runs.len());
            drop += 1;
        }
        s.links = s.links.split_off(drop.min(s.links.len()));
    }
    parts.push(Part::SwatchLinks(palette, swatch, before));
    e.push(Entry { label: "Fill".into(), parts });
    Ok(())
}

/// Recolour only the pixels in `links` that are near the old colour.
fn recolour_links(e: &mut Engine, links: &[tooncraft_doc::palette::FillLink], near: impl Fn([u8; 4]) -> bool, to: [u8; 3]) -> Vec<Part> {
    let mut parts = Vec::new();
    let mut by_drawing: std::collections::BTreeMap<tooncraft_doc::DrawingId, Vec<&tooncraft_doc::palette::FillLink>> = std::collections::BTreeMap::new();
    for l in links {
        by_drawing.entry(l.drawing).or_default().push(l);
    }
    for (d, ls) in by_drawing {
        // a locked layer's drawing keeps its paint
        let locked = (0..e.project.layers.len()).any(|i| e.check_unlocked(i).is_err() && layer_drawings(e, i).is_ok_and(|v| v.contains(&d)));
        if locked {
            continue;
        }
        let Some(s) = e.project.drawing_mut(d) else { continue };
        let t = tooncraft_raster::TILE;
        s.begin_capture();
        for l in ls {
            for (x, y) in l.pixels() {
                let Some(index) = s.tile_index(x / t, y / t) else { continue };
                let Some(tile) = s.tile_mut(index) else { continue };
                let i = (((y % t) * t + (x % t)) * 4) as usize;
                if let Some(p) = tile.bytes_mut().get_mut(i..i + 4) {
                    let cur = [p[0], p[1], p[2], p[3]];
                    if near(cur) {
                        p.copy_from_slice(&premultiply([to[0], to[1], to[2], cur[3]]));
                    }
                }
            }
        }
        let tiles = s.take_capture();
        if !tiles.is_empty() {
            parts.push(Part::Tiles { drawing: d, tiles });
        }
    }
    parts
}

/// Change swatch `swatch` of the shot's palette `palette` (its name when given, and its colour).
/// With `update_paint`, the paint in the old colour on every unlocked layer takes the new colour
/// (matched exactly, opacity kept), so fills made from the swatch follow it. One undo step for
/// the swatch and the paint together.
pub(crate) fn edit_swatch(e: &mut Engine, palette: usize, swatch: usize, rgb: [u8; 3], name: Option<String>, update_paint: bool) -> Result<()> {
    let old = e.project.palettes.get(palette).and_then(|p| p.swatches.get(swatch)).map(|s| s.rgb).ok_or(EngineError::NoSwatch(palette, swatch))?;
    let links: Vec<tooncraft_doc::palette::FillLink> =
        e.project.palettes.get(palette).and_then(|p| p.swatches.get(swatch)).map(|s| s.links.clone()).unwrap_or_default();
    let before = e.project.palettes.clone();
    let mut parts = Vec::new();
    let near = |p: [u8; 4]| {
        let want = premultiply([old[0], old[1], old[2], p[3]]);
        p[3] > 0 && (0..3).all(|k| p[k].abs_diff(want[k]) <= 1)
    };
    if update_paint && old != rgb && !links.is_empty() {
        // linked: only the paint filled from this swatch follows
        parts = recolour_links(e, &links, near, rgb);
    } else if update_paint && old != rgb {
        let mut drawings = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for i in 0..e.project.layers.len() {
            if e.check_unlocked(i).is_err() {
                continue;
            }
            for d in layer_drawings(e, i)? {
                if seen.insert(d) {
                    drawings.push(d);
                }
            }
        }
        // the old colour as it is stored at each pixel's opacity (premultiplied, ±1 for rounding),
        // so the soft edges of a fill in that colour follow it too
        let near = |p: [u8; 4]| {
            let want = premultiply([old[0], old[1], old[2], p[3]]);
            p[3] > 0 && (0..3).all(|k| p[k].abs_diff(want[k]) <= 1)
        };
        parts = recolour_drawings(e, &drawings, near, rgb);
    }
    let mut v = before.clone();
    if let Some(s) = v.get_mut(palette).and_then(|p| p.swatches.get_mut(swatch)) {
        // without updating the paint, the linked paint keeps the old colour: it is no longer
        // this swatch's paint, so the links go
        if !update_paint && old != rgb {
            s.links.clear();
        }
        s.rgb = rgb;
        if let Some(n) = name {
            s.name = n;
        }
    }
    e.project.palettes = tooncraft_doc::palette::sanitize_all(v);
    parts.insert(0, Part::Palettes(before));
    e.push(Entry { label: "Change swatch".into(), parts });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_and_outlines() {
        let pts = [(0.0, 0.0), (50.0, 40.0), (100.0, 0.0)];
        let c = curve_points(&pts, false);
        assert_eq!(c.first(), Some(&(0.0, 0.0)));
        assert_eq!(c.last(), Some(&(100.0, 0.0)));
        assert!(c.iter().any(|p| (p.0 - 50.0).abs() < 1.5 && (p.1 - 40.0).abs() < 1.5), "passes through the middle point");
        assert_eq!(path_outline(&pts, true, false).len(), 4);
    }

    #[test]
    fn recogniser() {
        let line: Vec<(f32, f32)> = (0..50).map(|i| (i as f32 * 4.0, 100.0 + (i as f32 * 0.7).sin() * 1.5)).collect();
        assert!(matches!(recognize(&line), Some((ShapeKind::Line, _, _))));
        let circle: Vec<(f32, f32)> = (0..=64)
            .map(|i| {
                let a = i as f32 / 64.0 * std::f32::consts::TAU;
                (200.0 + a.cos() * 50.0 * (1.0 + (a * 5.0).sin() * 0.03), 200.0 + a.sin() * 30.0)
            })
            .collect();
        let r = recognize(&circle);
        assert!(matches!(r, Some((ShapeKind::Ellipse, _, _))), "{r:?}");
        let zig: Vec<(f32, f32)> = (0..40).map(|i| (i as f32 * 5.0, if i % 2 == 0 { 0.0 } else { 40.0 })).collect();
        assert!(recognize(&zig).is_none());
        assert!(recognize(&[(0.0, 0.0), (f32::NAN, 1.0), (2.0, 2.0), (3.0, 3.0)]).is_none());
    }

    #[test]
    fn snapping() {
        assert_eq!(snap_to((49.0, 77.0), &[0.0, 50.0], &[0.0, 100.0], 5.0), (50.0, 77.0));
    }
}
