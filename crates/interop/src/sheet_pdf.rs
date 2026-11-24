//! The exposure sheet as a PDF to print: the layout of a paper sheet, one row per frame.
//!
//! Each page has a header (shot name, frame rate, length, page number) and columns for the frame
//! number, the frame notes, then one per layer, bottom layer first, named as in the XDTS export.
//! A drawing's number is written where it starts. A line down the middle of the column shows
//! that it holds, and `×` marks the start of a blank. A heavier rule marks each second and a
//! lighter one every six frames. A page holds whole seconds (96 rows at 24 fps). When there are
//! more layers than fit across, the extra columns go on further pages for the same frames.
//! Each page starts by repeating the drawing still held, in brackets, so a page reads on its own.
//!
//! [`frames`] prints frames through the camera, a grid of them per page, each with its number
//! below it, for reviews, reference and tracing.

use craft_pdf::write::{Metadata, Writer, backend};
use tooncraft_doc::Project;

use crate::common::track_names;
use crate::xdts::numbered_cells;
use crate::{InteropError, Result};

/// Most pages one sheet may print.
pub const MAX_PAGES: usize = 2000;

/// The paper the sheet is printed on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Paper {
    #[default]
    A4,
    Letter,
}

impl Paper {
    /// Width and height in points.
    pub fn size(self) -> (f32, f32) {
        match self {
            Paper::A4 => (595.28, 841.89),
            Paper::Letter => (612.0, 792.0),
        }
    }
}

const MARGIN: f32 = 36.0;
const HEADER: f32 = 34.0;
const ROW: f32 = 7.4;
const FRAME_COL: f32 = 30.0;
const NOTES_COL: f32 = 104.0;
const LAYER_COL: f32 = 44.0;
const TEXT: f32 = 6.2;

/// Most frames one PDF of printed frames may hold.
pub const MAX_PRINTED_FRAMES: u32 = 2000;
/// Pixels all printed frames together may keep in memory until the PDF is written.
pub const PRINT_PIXEL_BUDGET: u64 = 64 << 20;
/// Frames per page that [`frames`] lays out (a grid of 1×1, 1×2, 2×2, 2×3, 3×3 or 3×4).
pub const PER_PAGE: [u32; 6] = [1, 2, 4, 6, 9, 12];

/// The sheet's words, translated by the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Labels {
    /// Title of the frame-number column.
    pub frame: String,
    /// Title of the notes column.
    pub notes: String,
    /// After the length in the header ("frames").
    pub frames: String,
}

impl Default for Labels {
    fn default() -> Self {
        Self { frame: "Frame".into(), notes: "Notes".into(), frames: "frames".into() }
    }
}

/// How the sheet divides the shot: frames per page, layer columns per page, and pages.
pub fn layout(p: &Project, paper: Paper) -> (u32, usize, usize) {
    let (w, h) = paper.size();
    // one row goes to the column titles
    let fit = (((h - 2.0 * MARGIN - HEADER) / ROW).floor().max(2.0) as u32) - 1;
    let fps = p.fps().max(1);
    let rows = if fps <= fit { fps * (fit / fps) } else { fit };
    let across = (((w - 2.0 * MARGIN - FRAME_COL - NOTES_COL) / LAYER_COL).floor() as usize).max(1);
    let down = p.length().div_ceil(rows) as usize;
    let groups = p.layers.len().div_ceil(across).max(1);
    (rows, across, down.saturating_mul(groups))
}

/// The sheet of `p` as PDF bytes, its text set in `font` (TrueType or OpenType bytes).
pub fn export(p: &Project, paper: Paper, font: &[u8], labels: &Labels) -> Result<Vec<u8>> {
    let (rows, across, count) = layout(p, paper);
    if count > MAX_PAGES {
        return Err(InteropError::Limit(format!("the sheet would print {count} pages; at most {MAX_PAGES}")));
    }
    let names = track_names(&p.layers);
    let cells: Vec<Vec<Option<u32>>> = p.layers.iter().map(|l| numbered_cells(l, p.length())).collect();
    craft_pdf::write::guard(|| {
        let font = craft_pdf::write::font(font, 0, None).map_err(|e| InteropError::Encode(format!("the sheet font: {e}")))?;
        let metadata = Metadata {
            creator: "ToonCraft".to_owned(),
            producer: format!("ToonCraft {}", env!("CARGO_PKG_VERSION")),
            title: Some(p.name.clone()),
            ..Metadata::default()
        };
        let mut writer = Writer::new(backend::SerializeSettings::default(), metadata);
        let (w, h) = paper.size();
        let size = backend::geom::Size::from_wh(w, h).ok_or_else(|| InteropError::Encode("the paper size is not valid".into()))?;
        let groups = p.layers.len().div_ceil(across).max(1);
        let mut number = 0;
        for start in (0..p.length()).step_by(rows as usize) {
            let end = (start + rows).min(p.length());
            for g in 0..groups {
                number += 1;
                let layers: Vec<usize> = (g * across..((g + 1) * across).min(p.layers.len())).collect();
                let mut page = writer.document_mut().start_page_with(backend::page::PageSettings::new(size));
                let mut s = page.surface();
                let sheet = Sheet { p, font: &font, names: &names, cells: &cells, layers: &layers, labels };
                sheet.draw(&mut s, start, end, (number, count));
                s.finish();
                page.finish();
            }
        }
        writer.finish().map_err(|e| InteropError::Encode(e.to_string()))
    })
}

/// Frames `start..end` (0-based, end exclusive) through the camera on the paper, `per_page` of
/// them (one of [`PER_PAGE`]) in a grid on each page, each with its frame number below it.
pub fn frames(p: &Project, paper: Paper, start: u32, end: u32, per_page: u32, font: &[u8], labels: &Labels) -> Result<Vec<u8>> {
    if start >= end || end > p.length() {
        return Err(InteropError::Range(start, end));
    }
    if !PER_PAGE.contains(&per_page) {
        return Err(InteropError::Encode(format!("frames per page must be one of {PER_PAGE:?}")));
    }
    let count = end - start;
    if count > MAX_PRINTED_FRAMES {
        return Err(InteropError::Limit(format!("{count} frames; print at most {MAX_PRINTED_FRAMES} at once")));
    }
    let (cols, rows) = match per_page {
        1 => (1, 1),
        2 => (1, 2),
        4 => (2, 2),
        6 => (2, 3),
        9 => (3, 3),
        _ => (3, 4),
    };
    let (pw, ph) = paper.size();
    let cell_w = (pw - 2.0 * MARGIN) / cols as f32;
    let cell_h = (ph - 2.0 * MARGIN - HEADER) / rows as f32;
    let caption = 12.0;
    // the picture fitted into its cell, above the caption
    let (iw, ih) = (p.width() as f32, p.height() as f32);
    let fit = ((cell_w - 8.0) / iw).min((cell_h - 8.0 - caption) / ih);
    let (dw, dh) = (iw * fit, ih * fit);
    // pixels: about 200 per inch on paper, and within the budget for all frames together
    let budget = (PRINT_PIXEL_BUDGET / u64::from(count)).min(((dw * 200.0 / 72.0) * (dh * 200.0 / 72.0)) as u64).max(64 * 64);
    let mut reduce = 1u32;
    while u64::from(p.width().div_ceil(reduce)) * u64::from(p.height().div_ceil(reduce)) > budget && reduce < 64 {
        reduce += 1;
    }
    craft_pdf::write::guard(|| {
        let font = craft_pdf::write::font(font, 0, None).map_err(|e| InteropError::Encode(format!("the sheet font: {e}")))?;
        let metadata = Metadata {
            creator: "ToonCraft".to_owned(),
            producer: format!("ToonCraft {}", env!("CARGO_PKG_VERSION")),
            title: Some(p.name.clone()),
            ..Metadata::default()
        };
        let mut writer = Writer::new(backend::SerializeSettings::default(), metadata);
        let size = backend::geom::Size::from_wh(pw, ph).ok_or_else(|| InteropError::Encode("the paper size is not valid".into()))?;
        let picture = backend::geom::Size::from_wh(dw, dh).ok_or_else(|| InteropError::Encode("the picture size is not valid".into()))?;
        let pages = count.div_ceil(per_page);
        for page_no in 0..pages {
            let mut page = writer.document_mut().start_page_with(backend::page::PageSettings::new(size));
            let mut s = page.surface();
            let text = |s: &mut backend::surface::Surface<'_>, x: f32, y: f32, size: f32, t: &str| {
                s.set_stroke(None);
                s.set_fill(Some(backend::paint::Fill::default()));
                s.draw_text(backend::geom::Point::from_xy(x, y), font.clone(), size, t, false, backend::text::TextDirection::LeftToRight);
            };
            text(&mut s, MARGIN, MARGIN + 10.0, 11.0, &clip(&p.name, 60));
            let facts = format!("{} fps · {}–{} · {} / {pages}", p.fps(), start + 1, end, page_no + 1);
            text(&mut s, MARGIN, MARGIN + 22.0, 7.5, &facts);
            for k in 0..per_page {
                let f = start + page_no * per_page + k;
                if f >= end {
                    break;
                }
                let (cx, cy) = (MARGIN + cell_w * (k % cols) as f32, MARGIN + HEADER + cell_h * (k / cols) as f32);
                let x = cx + (cell_w - dw) / 2.0;
                let y = cy + 4.0;
                let img = tooncraft_doc::render_camera(p, f, tooncraft_doc::RenderOptions::default()).reduced(reduce);
                let image = backend::image::Image::from_rgba8(img.to_straight(), img.width, img.height);
                s.push_transform(&backend::geom::Transform::from_translate(x, y));
                s.draw_image(image, picture);
                s.pop();
                rule(&mut s, (x, y), (x + dw, y), 0.5, 120);
                rule(&mut s, (x, y + dh), (x + dw, y + dh), 0.5, 120);
                rule(&mut s, (x, y), (x, y + dh), 0.5, 120);
                rule(&mut s, (x + dw, y), (x + dw, y + dh), 0.5, 120);
                text(&mut s, x, y + dh + 9.0, 7.5, &format!("{} {}", clip(&labels.frame, 20), f + 1));
            }
            s.finish();
            page.finish();
        }
        writer.finish().map_err(|e| InteropError::Encode(e.to_string()))
    })
}

struct Sheet<'a> {
    p: &'a Project,
    font: &'a backend::text::Font,
    names: &'a [String],
    cells: &'a [Vec<Option<u32>>],
    layers: &'a [usize],
    labels: &'a Labels,
}

impl Sheet<'_> {
    fn draw(&self, s: &mut backend::surface::Surface<'_>, start: u32, end: u32, (page, pages): (usize, usize)) {
        let p = self.p;
        let fps = p.fps().max(1);
        let top = MARGIN + HEADER;
        let width = FRAME_COL + NOTES_COL + LAYER_COL * self.layers.len() as f32;
        self.text(s, MARGIN, MARGIN + 10.0, 11.0, &clip(&p.name, 60));
        let facts = format!("{fps} fps · {} {} ({}) · {page} / {pages}", p.length(), clip(&self.labels.frames, 20), timecode(p.length(), fps));
        self.text(s, MARGIN, MARGIN + 22.0, 7.5, &facts);
        let title = top + ROW - 1.6;
        self.text(s, MARGIN + 2.0, title, TEXT, &clip(&self.labels.frame, 7));
        self.text(s, MARGIN + FRAME_COL + 2.0, title, TEXT, &clip(&self.labels.notes, 30));
        for (c, &l) in self.layers.iter().enumerate() {
            let name = self.names.get(l).map_or("", String::as_str);
            self.text(s, self.col(c) + 2.0, title, TEXT, &clip(name, 11));
        }
        let body = top + ROW;
        let bottom = body + ROW * (end - start) as f32;
        // a light rule every frame, heavier every six frames and at each second
        for i in 0..=(end - start) {
            let f = start + i;
            let y = body + ROW * i as f32;
            let (weight, grey) = if f.is_multiple_of(fps) || i == end - start {
                (1.0, 0)
            } else if f.is_multiple_of(6) {
                (0.5, 90)
            } else {
                (0.3, 175)
            };
            rule(s, (MARGIN, y), (MARGIN + width, y), weight, grey);
        }
        line(s, (MARGIN, top), (MARGIN + width, top), 1.0);
        let mut x = MARGIN;
        line(s, (x, top), (x, bottom), 1.0);
        for w in [FRAME_COL, NOTES_COL].into_iter().chain(self.layers.iter().map(|_| LAYER_COL)) {
            x += w;
            line(s, (x, top), (x, bottom), 0.6);
        }
        for i in 0..(end - start) {
            let f = start + i;
            let base = body + ROW * (i + 1) as f32 - 1.6;
            self.text(s, MARGIN + 2.0, base, TEXT, &(f + 1).to_string());
            if let Some(note) = p.markers.get(&f) {
                self.text(s, MARGIN + FRAME_COL + 2.0, base, TEXT, &clip(note, 30));
            }
            for (c, &l) in self.layers.iter().enumerate() {
                let Some(cells) = self.cells.get(l) else { continue };
                let now = cells.get(f as usize).copied().flatten();
                let before = f.checked_sub(1).and_then(|b| cells.get(b as usize).copied().flatten());
                let x = self.col(c);
                match (now, f == 0 || now != before, i == 0) {
                    (Some(n), true, _) => self.text(s, x + 3.0, base, TEXT, &n.to_string()),
                    (Some(n), false, true) => self.text(s, x + 3.0, base, TEXT, &format!("({n})")),
                    (Some(_), false, false) => {
                        let mid = x + LAYER_COL / 2.0;
                        let y = body + ROW * i as f32;
                        line(s, (mid, y), (mid, y + ROW), 1.4);
                    }
                    (None, true, _) => self.text(s, x + 3.0, base, TEXT, "×"),
                    (None, false, _) => {}
                }
            }
        }
    }

    fn col(&self, c: usize) -> f32 {
        MARGIN + FRAME_COL + NOTES_COL + LAYER_COL * c as f32
    }

    fn text(&self, s: &mut backend::surface::Surface<'_>, x: f32, y: f32, size: f32, t: &str) {
        if t.is_empty() {
            return;
        }
        s.set_stroke(None);
        s.set_fill(Some(backend::paint::Fill::default()));
        s.draw_text(backend::geom::Point::from_xy(x, y), self.font.clone(), size, t, false, backend::text::TextDirection::LeftToRight);
    }
}

fn line(s: &mut backend::surface::Surface<'_>, a: (f32, f32), b: (f32, f32), width: f32) {
    rule(s, a, b, width, 0);
}

/// A straight line in a grey (0 black … 255 white).
fn rule(s: &mut backend::surface::Surface<'_>, a: (f32, f32), b: (f32, f32), width: f32, grey: u8) {
    let mut path = backend::geom::PathBuilder::new();
    path.move_to(a.0, a.1);
    path.line_to(b.0, b.1);
    let Some(path) = path.finish() else { return };
    s.set_fill(None);
    s.set_stroke(Some(backend::paint::Stroke { width, paint: backend::color::luma::Color::new(grey).into(), ..Default::default() }));
    s.draw_path(&path);
}

/// At most `n` characters, control characters dropped, `…` when cut.
fn clip(t: &str, n: usize) -> String {
    let clean: String = t.chars().filter(|c| !c.is_control()).collect();
    if clean.chars().count() <= n { clean } else { clean.chars().take(n.saturating_sub(1)).chain(std::iter::once('…')).collect() }
}

/// Minutes, seconds and frames: `1:04+08`.
fn timecode(frames: u32, fps: u32) -> String {
    let s = frames / fps;
    format!("{}:{:02}+{:02}", s / 60, s % 60, frames % fps)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font() -> &'static [u8] {
        epaint_default_fonts::UBUNTU_LIGHT
    }

    fn shot(layers: usize, frames: u32) -> Project {
        let mut p = Project::new("Sheet", 64, 48, 24, frames).unwrap();
        for i in 1..layers {
            p.add_layer(&format!("L{i}"), i).unwrap();
        }
        p
    }

    #[test]
    fn a_page_holds_whole_seconds_and_wide_sheets_go_across() {
        let (rows, across, pages) = layout(&shot(3, 200), Paper::A4);
        assert_eq!(rows % 24, 0, "whole seconds");
        assert_eq!(pages, 200usize.div_ceil(rows as usize));
        let (_, _, wide) = layout(&shot(across + 1, 200), Paper::A4);
        assert_eq!(wide, pages * 2, "more layers than fit go on a second set of pages");
    }

    #[test]
    fn the_sheet_is_a_pdf_with_its_pages() {
        let mut p = shot(2, 200);
        p.markers.insert(3, "Anticipation".into());
        let (_, _, want) = layout(&p, Paper::Letter);
        let pdf = export(&p, Paper::Letter, font(), &Labels::default()).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        let text = String::from_utf8_lossy(&pdf).replace("/Type /", "/Type/");
        let pages = text.matches("/Type/Page").count() - text.matches("/Type/Pages").count();
        assert_eq!(pages, want, "one PDF page per sheet page");
    }

    #[test]
    fn too_many_pages_and_a_bad_font_are_errors() {
        let p = shot(1, 24);
        assert!(export(&p, Paper::A4, b"not a font", &Labels::default()).is_err());
        let mut long = Project::new("Long", 8, 8, 1, tooncraft_doc::MAX_FRAMES).unwrap();
        for i in 1..30 {
            long.add_layer("x", i).unwrap();
        }
        let (_, _, pages) = layout(&long, Paper::A4);
        assert!(pages > MAX_PAGES, "the longest shot at 1 fps with 30 layers needs more pages than allowed");
        assert!(matches!(export(&long, Paper::A4, font(), &Labels::default()), Err(InteropError::Limit(_))));
    }

    #[test]
    fn frames_print_in_a_grid_and_refuse_bad_requests() {
        let p = shot(1, 30);
        let l = Labels::default();
        let pdf = frames(&p, Paper::A4, 0, 30, 4, font(), &l).unwrap();
        let text = String::from_utf8_lossy(&pdf).replace("/Type /", "/Type/");
        let pages = text.matches("/Type/Page").count() - text.matches("/Type/Pages").count();
        assert_eq!(pages, 8, "30 frames, 4 to a page");
        assert!(text.contains("/Subtype/Image") || text.contains("/Subtype /Image"), "the pictures are images");
        assert!(frames(&p, Paper::A4, 5, 5, 4, font(), &l).is_err(), "empty range");
        assert!(frames(&p, Paper::A4, 0, 31, 4, font(), &l).is_err(), "past the end");
        assert!(frames(&p, Paper::A4, 0, 4, 5, font(), &l).is_err(), "5 to a page is not a grid");
        let long = Project::new("Long", 8, 8, 24, MAX_PRINTED_FRAMES + 1).unwrap();
        assert!(matches!(frames(&long, Paper::A4, 0, MAX_PRINTED_FRAMES + 1, 12, font(), &l), Err(InteropError::Limit(_))));
    }

    #[test]
    fn clip_and_timecode() {
        assert_eq!(clip("abc\u{7}def", 4), "abc…");
        assert_eq!(clip("ab", 4), "ab");
        assert_eq!(timecode(24 * 64 + 8, 24), "1:04+08");
    }
}
