//! The text tool's engine side: fonts the engine may use, shaping a line of text (through the
//! shared `craft-font-catalog`), laying lines out with alignment and spacing, and painting the
//! glyph outlines into a drawing with antialiased coverage (`tooncraft_raster::coverage`).
//!
//! Text is painted pixels once committed (the drawing stays raster); the app keeps the text
//! editable until then. Everything is bounded: [`MAX_TEXT_CHARS`] characters, [`MAX_LINES`]
//! lines, [`MAX_GLYPHS`] glyphs, sizes from [`MIN_SIZE`] to [`MAX_SIZE`] px, and a curve
//! flattening budget per glyph. Fonts are hostile input: a face that does not parse is refused,
//! and a glyph that fails to draw is skipped.

use std::sync::Arc;

use craft_font_catalog::{FontOutline, GlyphPen, RunInput, shape_run};
use serde::{Deserialize, Serialize};
use tooncraft_raster::coverage::{self, Edge};

use crate::undo::{Entry, Part};
use crate::{Engine, EngineError, Result};

pub const MAX_TEXT_CHARS: usize = 2000;
pub const MAX_LINES: usize = 200;
pub const MAX_GLYPHS: usize = 4000;
pub const MIN_SIZE: f32 = 2.0;
pub const MAX_SIZE: f32 = 1000.0;
/// Most fonts the engine keeps.
pub const MAX_FACES: usize = 256;
/// Most flattened edges of one glyph.
const MAX_GLYPH_EDGES: usize = 200_000;
/// Most coverage cells one text command may rasterise: every glyph's box clipped to the canvas,
/// added up (overlapping boxes count twice). 32 Mi cells is about four 4K frames of glyph
/// boxes, a fraction of a second of work.
pub const MAX_TEXT_CELLS: u64 = 32 << 20;
/// Most flattened outline edges of one text command.
pub const MAX_TEXT_EDGES: usize = 2 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// How text looks. `font` names a face the engine knows ([`Fonts`]); empty is the first one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextStyle {
    pub font: String,
    /// Em size in canvas pixels.
    pub size: f32,
    /// Straight RGBA.
    pub color: [u8; 4],
    pub align: TextAlign,
    /// Line height as a multiple of the font's own.
    pub line_spacing: f32,
    /// Extra pixels after every character.
    pub letter_spacing: f32,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self { font: String::new(), size: 48.0, color: [20, 20, 20, 255], align: TextAlign::Left, line_spacing: 1.0, letter_spacing: 0.0 }
    }
}

impl TextStyle {
    pub fn sanitized(self) -> Self {
        let d = Self::default();
        let size = if self.size.is_finite() { self.size.clamp(MIN_SIZE, MAX_SIZE) } else { d.size };
        let line_spacing = if self.line_spacing.is_finite() { self.line_spacing.clamp(0.5, 4.0) } else { d.line_spacing };
        let letter_spacing = if self.letter_spacing.is_finite() { self.letter_spacing.clamp(-0.5 * size, 2.0 * size) } else { 0.0 };
        let font: String = self.font.chars().filter(|c| !c.is_control()).take(200).collect();
        Self { font, size, color: self.color, align: self.align, line_spacing, letter_spacing }
    }
}

/// The text a command may paint: control characters other than line breaks dropped, tabs as
/// spaces, at most [`MAX_TEXT_CHARS`] characters and [`MAX_LINES`] lines.
pub fn clean_text(text: &str) -> String {
    let mut out = String::new();
    let mut lines = 1;
    for c in text.chars().take(MAX_TEXT_CHARS) {
        match c {
            '\n' if lines < MAX_LINES => {
                lines += 1;
                out.push('\n');
            }
            '\n' => break,
            '\t' => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// Font file bytes shared without copying (a built-in `&'static [u8]`, a loaded `Vec<u8>`, or the
/// UI toolkit's own font data).
pub type FontBytes = Arc<dyn AsRef<[u8]> + Send + Sync>;

/// One font face the engine can draw with.
#[derive(Clone)]
pub struct Face {
    pub name: String,
    pub bytes: FontBytes,
    pub index: u32,
}

impl Face {
    pub fn data(&self) -> &[u8] {
        (*self.bytes).as_ref()
    }
}

/// The fonts the engine knows, by name. Not saved with the shot: the app registers the faces it
/// ships with, the CLI the files it is given.
#[derive(Clone, Default)]
pub struct Fonts {
    faces: Vec<Face>,
}

impl Fonts {
    /// Add (or replace) a face; false when the bytes are not a font both the outline reader and
    /// the shaper accept, or the list is full.
    pub fn add(&mut self, name: &str, bytes: FontBytes, index: u32) -> bool {
        let data: &[u8] = (*bytes).as_ref();
        if FontOutline::from_bytes(data, index, &[]).is_err() || harfrust::FontRef::from_index(data, index).is_err() {
            return false;
        }
        let name: String = name.chars().filter(|c| !c.is_control()).take(200).collect();
        if let Some(f) = self.faces.iter_mut().find(|f| f.name == name) {
            *f = Face { name, bytes, index };
            return true;
        }
        if self.faces.len() >= MAX_FACES {
            return false;
        }
        self.faces.push(Face { name, bytes, index });
        true
    }

    pub fn names(&self) -> Vec<String> {
        self.faces.iter().map(|f| f.name.clone()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    /// The face called `name`, or the first face for an empty name.
    pub fn get(&self, name: &str) -> Option<&Face> {
        if name.is_empty() { self.faces.first() } else { self.faces.iter().find(|f| f.name == name) }
    }
}

/// A glyph placed on the canvas: its id and the pen position (canvas pixels, on the baseline).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub glyph: u32,
    pub x: f32,
    pub y: f32,
}

/// Laid-out text: the glyphs, the pixels per font unit and the box `(x0, y0, x1, y1)` the lines
/// take (for the app's text box).
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub glyphs: Vec<Placed>,
    pub scale: f32,
    pub bounds: (f32, f32, f32, f32),
}

/// Lay `text` out with its top at `at.1` and its anchor at `at.0` (left edge, centre or right
/// edge by the alignment).
pub fn layout(face: &Face, text: &str, at: (f32, f32), style: &TextStyle) -> Result<Layout> {
    let style = style.clone().sanitized();
    let outline = FontOutline::from_bytes(face.data(), face.index, &[]).map_err(|e| EngineError::Text(e.to_string()))?;
    let font = harfrust::FontRef::from_index(face.data(), face.index).map_err(|e| EngineError::Text(e.to_string()))?;
    let data = harfrust::ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let m = outline.metrics();
    let upem = (m.units_per_em as f32).max(16.0);
    let scale = style.size / upem;
    let (ascent, descent) = (m.ascent.abs() * scale, m.descent.abs() * scale);
    let line_h = ((m.ascent.abs() + m.descent.abs() + m.leading.max(0.0)) * scale).max(1.0) * style.line_spacing;
    let mut glyphs = Vec::new();
    let (mut lx, mut hx) = (f32::INFINITY, f32::NEG_INFINITY);
    let text = clean_text(text);
    let mut lines = 0;
    for (i, line) in text.split('\n').enumerate() {
        lines = i + 1;
        let baseline = at.1 + ascent + i as f32 * line_h;
        let input = RunInput {
            text: line,
            source_start: 0,
            pre_context: "",
            post_context: "",
            direction: harfrust::Direction::Invalid,
            language: None,
            features: &[],
            character_clusters: false,
            uppercase: false,
        };
        let run = shape_run(&shaper, &input, |c| c, |s, d, sc, l, f| Arc::new(harfrust::ShapePlan::new(s, d, sc, l, f)))
            .map_err(|e| EngineError::Text(e.to_string()))?;
        let mut pen = 0.0f32;
        let mut placed = Vec::with_capacity(run.len());
        for g in &run {
            placed.push(Placed { glyph: g.glyph_id, x: pen + g.x_offset as f32 * scale, y: baseline - g.y_offset as f32 * scale });
            pen += g.x_advance as f32 * scale + style.letter_spacing;
        }
        let width = if run.is_empty() { 0.0 } else { (pen - style.letter_spacing).max(0.0) };
        let shift = match style.align {
            TextAlign::Left => at.0,
            TextAlign::Center => at.0 - width / 2.0,
            TextAlign::Right => at.0 - width,
        };
        lx = lx.min(shift);
        hx = hx.max(shift + width);
        for p in placed {
            if glyphs.len() >= MAX_GLYPHS {
                break;
            }
            glyphs.push(Placed { x: p.x + shift, ..p });
        }
    }
    let bottom = at.1 + (lines.max(1) - 1) as f32 * line_h + ascent + descent;
    let (lx, hx) = if lx.is_finite() { (lx, hx) } else { (at.0, at.0) };
    Ok(Layout { glyphs, scale, bounds: (lx, at.1, hx, bottom) })
}

/// Flattens a glyph outline (font units, y up) into canvas-pixel edges.
struct Flatten {
    scale: f32,
    origin: (f32, f32),
    start: (f32, f32),
    cur: (f32, f32),
    edges: Vec<Edge>,
    /// Most edges this pen keeps; past it `over` is set and the rest is dropped.
    cap: usize,
    over: bool,
}

impl Flatten {
    fn map(&self, x: f32, y: f32) -> (f32, f32) {
        (self.origin.0 + x * self.scale, self.origin.1 - y * self.scale)
    }

    fn edge(&mut self, to: (f32, f32)) {
        if self.edges.len() < self.cap {
            self.edges.push((self.cur, to));
        } else {
            self.over = true;
        }
        self.cur = to;
    }

    /// Segments for a curve whose control polygon is `len` pixels long.
    fn steps(len: f32) -> usize {
        if len.is_finite() { ((len / 3.0).ceil() as usize).clamp(1, 48) } else { 1 }
    }
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

impl GlyphPen for Flatten {
    fn move_to(&mut self, x: f32, y: f32) {
        self.close();
        let p = self.map(x, y);
        self.start = p;
        self.cur = p;
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.map(x, y);
        self.edge(p);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (p0, p1, p2) = (self.cur, self.map(x1, y1), self.map(x, y));
        let n = Self::steps(dist(p0, p1) + dist(p1, p2));
        for k in 1..=n {
            let t = k as f32 / n as f32;
            let u = 1.0 - t;
            self.edge((u * u * p0.0 + 2.0 * u * t * p1.0 + t * t * p2.0, u * u * p0.1 + 2.0 * u * t * p1.1 + t * t * p2.1));
        }
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (p0, p1, p2, p3) = (self.cur, self.map(x1, y1), self.map(x2, y2), self.map(x, y));
        let n = Self::steps(dist(p0, p1) + dist(p1, p2) + dist(p2, p3));
        for k in 1..=n {
            let t = k as f32 / n as f32;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            self.edge((a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0, a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1));
        }
    }
    fn close(&mut self) {
        if self.cur != self.start {
            let s = self.start;
            self.edge(s);
        }
    }
}

/// The glyph outlines of laid-out text, flattened to edges, and the work they will take.
pub struct Prepared {
    pub glyphs: Vec<Vec<Edge>>,
    /// Coverage cells the glyph boxes cover on the `clip` canvas, added up.
    pub cells: u64,
    pub edges: usize,
}

/// The cells a glyph's coverage box takes, clipped to the canvas (as `coverage::rasterize`).
fn box_cells(edges: &[Edge], clip: (u32, u32)) -> u64 {
    let (mut lx, mut ly, mut hx, mut hy) = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for (a, b) in edges {
        lx = lx.min(a.0).min(b.0);
        ly = ly.min(a.1).min(b.1);
        hx = hx.max(a.0).max(b.0);
        hy = hy.max(a.1).max(b.1);
    }
    if !(lx.is_finite() && ly.is_finite() && hx.is_finite() && hy.is_finite()) {
        return 0;
    }
    let w = (hx.ceil() as i64).min(clip.0 as i64) - (lx.floor() as i64).max(0);
    let h = (hy.ceil() as i64).min(clip.1 as i64) - (ly.floor() as i64).max(0);
    if w <= 0 || h <= 0 { 0 } else { w as u64 * h as u64 }
}

/// Flatten every glyph and count the work; text over [`MAX_TEXT_EDGES`] edges or
/// [`MAX_TEXT_CELLS`] cells is refused before anything is painted.
pub fn prepare(face: &Face, lay: &Layout, clip: (u32, u32)) -> Result<Prepared> {
    let outline = FontOutline::from_bytes(face.data(), face.index, &[]).map_err(|err| EngineError::Text(err.to_string()))?;
    let too_large = || EngineError::Text("the text is too large to paint in one step; make it smaller or shorter".into());
    let mut out = Prepared { glyphs: Vec::with_capacity(lay.glyphs.len()), cells: 0, edges: 0 };
    for g in &lay.glyphs {
        let cap = MAX_GLYPH_EDGES.min(MAX_TEXT_EDGES - out.edges);
        let mut pen = Flatten { scale: lay.scale, origin: (g.x, g.y), start: (g.x, g.y), cur: (g.x, g.y), edges: Vec::new(), cap, over: false };
        // a glyph that fails to draw is skipped whole
        if outline.draw_to(g.glyph, &mut pen).is_err() {
            continue;
        }
        pen.close();
        if pen.over {
            return Err(too_large());
        }
        out.edges += pen.edges.len();
        out.cells = out.cells.saturating_add(box_cells(&pen.edges, clip));
        if out.cells > MAX_TEXT_CELLS {
            return Err(too_large());
        }
        if !pen.edges.is_empty() {
            out.glyphs.push(pen.edges);
        }
    }
    Ok(out)
}

/// Paint text into the drawing at the layer and frame (made on a blank cell), one undo step.
pub(crate) fn paint_text(e: &mut Engine, layer: usize, frame: u32, text: &str, at: (f32, f32), style: &TextStyle) -> Result<()> {
    e.check_unlocked(layer)?;
    if !(at.0.is_finite() && at.1.is_finite() && at.0.abs() < 1.0e6 && at.1.abs() < 1.0e6) {
        return Err(EngineError::Text("the text position is not a number".into()));
    }
    let style = style.clone().sanitized();
    let text = clean_text(text);
    if text.trim().is_empty() {
        return Ok(());
    }
    let face = e.fonts.get(&style.font).cloned().ok_or_else(|| {
        if style.font.is_empty() {
            EngineError::Text("no font is loaded".into())
        } else {
            EngineError::Text(format!("the font “{}” is not loaded", style.font))
        }
    })?;
    let lay = layout(&face, &text, at, &style)?;
    let clip = (e.project.width(), e.project.height());
    // all the work is counted (and refused) before the drawing is touched
    let prepared = prepare(&face, &lay, clip)?;
    let (drawing, parts) = e.ensure_drawing(layer, frame)?;
    let Some(s) = e.project.drawing_mut(drawing) else {
        Entry { label: String::new(), parts }.swap(&mut e.project);
        return Err(EngineError::NoDrawing(frame));
    };
    s.begin_capture();
    let mut failed = None;
    for edges in &prepared.glyphs {
        match coverage::rasterize(edges, clip) {
            Ok(Some(c)) => coverage::paint(s, &c, style.color),
            Ok(None) => {}
            Err(err) => {
                failed = Some(err);
                break;
            }
        }
    }
    let tiles = s.take_capture();
    if let Some(err) = failed {
        // put the drawing back as it was (and drop a drawing made for this text)
        s.swap_snapshot(tiles);
        Entry { label: String::new(), parts }.swap(&mut e.project);
        return Err(EngineError::Text(err.to_string()));
    }
    let mut parts = parts;
    parts.push(Part::Tiles { drawing, tiles });
    e.push(Entry { label: "Text".into(), parts });
    Ok(())
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
