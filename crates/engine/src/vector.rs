//! Vector drawings: strokes kept as editable centre lines (`tooncraft-vector`) and painted into
//! the drawing's tiles, so the compositor, fill, onion skin and every export read them like any
//! drawing. Each edit repaints the drawing from its strokes and is one undo step.

use serde::{Deserialize, Serialize};
use tooncraft_brush::{Brush, BrushKind, Dab, Sample, Stroke};
use tooncraft_doc::{DocError, DrawingId, Project};
use tooncraft_raster::Surface;
use tooncraft_raster::coverage::{self, Edge};
use tooncraft_vector::{LineStyle, VDrawing, VPoint, VStroke};

use crate::undo::{Entry, Part};
use crate::{Engine, EngineError, Result};

/// Most points one drawn stroke keeps before thinning.
pub const MAX_LIVE_POINTS: usize = 100_000;

/// A pen stroke on a vector layer, collected while it is drawn.
pub(crate) struct Live {
    layer: usize,
    frame: u32,
    brush: Brush,
    style: LineStyle,
    points: Vec<VPoint>,
}

impl Live {
    /// `Some` when the stroke lands on a vector layer's blank cell or vector drawing.
    pub(crate) fn begin(p: &Project, layer: usize, frame: u32, drawing: DrawingId, brush: Brush, style: LineStyle) -> Option<Live> {
        let on_vector_layer = p.layers.get(layer).is_some_and(|l| l.vector);
        let blank_or_vector = p.vectors.contains_key(&drawing) || p.drawing(drawing).is_some_and(|s| s.is_empty());
        (on_vector_layer && blank_or_vector).then(|| Live { layer, frame, brush, style, points: Vec::new() })
    }

    pub(crate) fn push(&mut self, s: Sample) {
        if self.points.len() < MAX_LIVE_POINTS {
            self.points.push(VPoint::new(s.x, s.y, self.brush.size * s.pressure.clamp(0.05, 1.0)));
        }
    }

    /// End the stroke: one vector stroke, or with the eraser, the line under its first point cut
    /// back to its crossings.
    pub(crate) fn finish(self, e: &mut Engine) -> Result<()> {
        let Some(first) = self.points.first().copied() else { return Ok(()) };
        if self.brush.kind == BrushKind::Eraser {
            let id = vector_drawing(e, self.layer, self.frame)?;
            let reach = (self.brush.size * 0.5).max(3.0);
            let Some(stroke) = e.project.vectors.get(&id).and_then(|v| v.stroke_at(first.x, first.y, reach)) else { return Ok(()) };
            return edit(e, self.layer, self.frame, VectorEdit::EraseToCrossings { stroke, x: first.x, y: first.y });
        }
        add_stroke(e, self.layer, self.frame, self.brush.color, &self.points, 0.6, self.style)
    }
}

/// Most dabs one repaint lays (all strokes of a drawing together).
pub const MAX_DABS: usize = 4_000_000;

/// A change to one vector stroke.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "edit", rename_all = "snake_case")]
pub enum VectorEdit {
    /// Move one point of a stroke.
    MovePoint { stroke: usize, point: usize, x: f32, y: f32 },
    /// The width at one point, or (no `point`) the widest point of the stroke, the rest in scale.
    SetWidth {
        stroke: usize,
        #[serde(default)]
        point: Option<usize>,
        width: f32,
    },
    /// Pull inner points towards their neighbours (`strength` 0…1, up to 16 `passes`).
    Smooth { stroke: usize, strength: f32, passes: u32 },
    /// Cut the stroke back to the crossings either side of the point nearest `x`, `y`.
    EraseToCrossings { stroke: usize, x: f32, y: f32 },
    /// Remove a whole stroke.
    DeleteStroke { stroke: usize },
    /// Recolour a stroke (straight RGBA).
    SetColor { stroke: usize, color: [u8; 4] },
    /// A stroke's ends and corners.
    SetStyle { stroke: usize, style: LineStyle },
}

/// Paint `v` into a new transparent surface of `w` × `h`: round, hard dabs along each curve,
/// their radius half the line width there. Work is capped at [`MAX_DABS`].
pub fn render(v: &VDrawing, w: u32, h: u32) -> Result<Surface> {
    let mut surface = Surface::new(w, h).map_err(DocError::from)?;
    let mut dabs_left = MAX_DABS;
    for s in &v.strokes {
        // other ends and corners than round: the outline, filled with exact coverage; a line
        // too big for one coverage box falls back to round dabs
        if !s.style.is_round() && outline(&mut surface, s) {
            continue;
        }
        let brush = Brush { size: 1.0, opacity: 1.0, flow: 1.0, hardness: 1.0, color: s.color, ..Brush::preset(BrushKind::Ink) };
        let mut stroke = Stroke::begin(&mut surface, brush);
        let mut dabs = Vec::new();
        let mut last: Option<VPoint> = None;
        for p in s.samples(0.5) {
            if dabs_left == 0 {
                break;
            }
            let r = (p.width * 0.5).max(0.35);
            // a dab every quarter radius keeps the edge smooth
            if let Some(q) = last
                && (p.x - q.x).hypot(p.y - q.y) < (r * 0.25).max(0.4)
            {
                continue;
            }
            last = Some(p);
            dabs.push(Dab { x: p.x, y: p.y, radius: r, alpha: 1.0, angle: 0.0, roundness: 1.0, pressure: 1.0, color: None, cell: 0, along: 0.0 });
            dabs_left -= 1;
        }
        stroke.add(&mut surface, &dabs);
        let _ = stroke.end(&mut surface);
    }
    Ok(surface)
}

/// Fill `s`'s outline into `surface`; false when it could not (too large) and nothing was painted.
fn outline(surface: &mut Surface, s: &VStroke) -> bool {
    let mut edges: Vec<Edge> = Vec::new();
    for poly in s.outline(1.0) {
        for (i, a) in poly.iter().enumerate() {
            if let Some(b) = poly.get((i + 1) % poly.len()) {
                edges.push((*a, *b));
            }
        }
        if edges.len() > coverage::MAX_EDGES {
            return false;
        }
    }
    match coverage::rasterize(&edges, (surface.width(), surface.height())) {
        Ok(Some(c)) => {
            coverage::paint(surface, &c, s.color);
            true
        }
        Ok(None) => true,
        Err(_) => false,
    }
}

/// The drawing at a layer's frame for a vector edit: it must be a vector drawing.
fn vector_drawing(e: &Engine, layer: usize, frame: u32) -> Result<DrawingId> {
    let d = e.project.drawing_at(layer, frame).ok_or(EngineError::NoDrawing(frame))?;
    if e.project.vectors.contains_key(&d) { Ok(d) } else { Err(EngineError::NotVector) }
}

/// Replace a drawing's strokes with `v` painted as `surface`, as one undo step with `parts`.
fn commit(e: &mut Engine, id: DrawingId, v: VDrawing, surface: Surface, label: &str, mut parts: Vec<Part>) {
    let before_surface = e.project.swap_drawing(id, Some(surface));
    let before_vectors = e.project.vectors.insert(id, v);
    parts.push(Part::Drawing { id, surface: before_surface });
    parts.push(Part::Vector { id, strokes: before_vectors });
    e.push(Entry { label: label.into(), parts });
}

/// Add a drawn stroke to the vector drawing at a layer's frame (a new one on a blank cell).
pub(crate) fn add_stroke(e: &mut Engine, layer: usize, frame: u32, color: [u8; 4], points: &[VPoint], tolerance: f32, style: LineStyle) -> Result<()> {
    if !e.project.layer(layer)?.vector {
        return Err(EngineError::NotVector);
    }
    let mut stroke = VStroke::from_points(points, color, tolerance)?;
    stroke.style = style;
    if let Some(d) = e.project.drawing_at(layer, frame)
        && !e.project.vectors.contains_key(&d)
        && e.project.drawing(d).is_some_and(|s| !s.is_empty())
    {
        // paint already on this raster drawing would be lost by a repaint
        return Err(EngineError::NotVector);
    }
    let (id, created) = e.ensure_drawing(layer, frame)?;
    let mut v = e.project.vectors.get(&id).cloned().unwrap_or_default();
    let painted = v.add(stroke).map_err(EngineError::from).and_then(|_| render(&v, e.project.width(), e.project.height()));
    match painted {
        Ok(surface) => {
            commit(e, id, v, surface, "Vector stroke", created);
            Ok(())
        }
        Err(err) => {
            // put back the drawing the stroke made for itself
            let _ = Entry { label: String::new(), parts: created }.swap(&mut e.project);
            Err(err)
        }
    }
}

/// Apply one edit to the vector drawing at a layer's frame.
pub(crate) fn edit(e: &mut Engine, layer: usize, frame: u32, edit: VectorEdit) -> Result<()> {
    e.check_unlocked(layer)?;
    let id = vector_drawing(e, layer, frame)?;
    let mut v = e.project.vectors.get(&id).cloned().unwrap_or_default();
    let label = match edit {
        VectorEdit::MovePoint { stroke, point, x, y } => {
            v.move_point(stroke, point, x, y)?;
            "Move point"
        }
        VectorEdit::SetWidth { stroke, point, width } => {
            v.set_width(stroke, point, width)?;
            "Line width"
        }
        VectorEdit::Smooth { stroke, strength, passes } => {
            v.smooth(stroke, strength, passes)?;
            "Smooth line"
        }
        VectorEdit::EraseToCrossings { stroke, x, y } => {
            v.erase_to_crossings(stroke, x, y)?;
            "Erase to crossings"
        }
        VectorEdit::DeleteStroke { stroke } => {
            if stroke >= v.strokes.len() {
                return Err(tooncraft_vector::VectorError::NoStroke(stroke).into());
            }
            v.strokes.remove(stroke);
            "Delete line"
        }
        VectorEdit::SetColor { stroke, color } => {
            v.strokes.get_mut(stroke).ok_or(tooncraft_vector::VectorError::NoStroke(stroke))?.color = color;
            "Line colour"
        }
        VectorEdit::SetStyle { stroke, style } => {
            v.strokes.get_mut(stroke).ok_or(tooncraft_vector::VectorError::NoStroke(stroke))?.style = style;
            "Line ends and corners"
        }
    };
    let surface = render(&v, e.project.width(), e.project.height())?;
    commit(e, id, v, surface, label, Vec::new());
    Ok(())
}
