//! Keeping long shots memory-bounded: drawings far from the playhead are paged out to a scratch
//! file once the drawings in memory pass [`Engine::page_budget`], and read back when shown.

use std::collections::BTreeSet;

use tooncraft_doc::DrawingId;
use tooncraft_doc::paging::PageFile;

use crate::{Engine, Result};

/// Default bytes of drawings kept in memory before paging starts.
pub const DEFAULT_PAGE_BUDGET: u64 = 1 << 30;
/// Frames before and after the playhead whose drawings stay in memory.
pub const KEEP_BEFORE: u32 = 12;
pub const KEEP_AFTER: u32 = 48;

impl Engine {
    /// Drawings shown within the kept window around `frame` (all layers), the drawing being
    /// painted, and the light table's pins.
    pub fn working_set(&self, frame: u32) -> BTreeSet<DrawingId> {
        let p = &self.project;
        let (from, to) = (frame.saturating_sub(KEEP_BEFORE), frame.saturating_add(KEEP_AFTER).min(p.length().saturating_sub(1)));
        let mut keep = BTreeSet::new();
        for layer in &p.layers {
            for f in from..=to {
                if let Some(d) = layer.drawing_at(f) {
                    keep.insert(d);
                }
            }
        }
        for pin in &p.light_table.pins {
            if let Some(d) = p.layer_index(pin.layer).and_then(|li| p.drawing_at(li, pin.frame)) {
                keep.insert(d);
            }
        }
        if let Some(d) = self.live_drawing() {
            keep.insert(d);
        }
        keep
    }

    /// Page out drawings outside the working set around `frame` while the drawings in memory
    /// take more than [`Engine::page_budget`]; goes down to three quarters of it so it does not
    /// run every frame. Returns the bytes freed (0 under budget).
    pub fn page(&mut self, frame: u32) -> Result<u64> {
        if self.project.memory_bytes() <= self.page_budget {
            return Ok(0);
        }
        let file = match &self.pages {
            Some(f) => f.clone(),
            None => {
                let f = PageFile::create(None).map_err(|e| crate::EngineError::Import(format!("page file: {e}")))?;
                self.pages = Some(f.clone());
                f
            }
        };
        let keep = self.working_set(frame);
        Ok(self.project.page_out(&file, &keep, self.page_budget / 4 * 3)?)
    }

    /// Open a `.toon` file with drawings paged out while loading, so a long shot never holds
    /// more than `budget` bytes of drawings (plus its working set once shown).
    pub fn open_paged(path: &std::path::Path, budget: u64) -> std::result::Result<Engine, tooncraft_format::FormatError> {
        let file = PageFile::create(None)?;
        let project = tooncraft_format::load_with(path, Some((&file, budget)))?;
        let mut e = Engine::new(project);
        e.page_budget = budget;
        if e.project.paged_count() > 0 {
            e.pages = Some(file);
        }
        Ok(e)
    }

    /// Bytes in the page file.
    pub fn paged_bytes(&self) -> u64 {
        self.pages.as_ref().map(|f| f.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Command;
    use tooncraft_brush::{Brush, BrushKind, Sample};
    use tooncraft_doc::{Project, RenderOptions, render_frame};

    #[test]
    fn long_shots_page_out_and_read_back_the_same() {
        let mut e = Engine::new(Project::new("Long", 512, 256, 24, 120).unwrap());
        let ink = Brush { size: 30.0, ..Brush::preset(BrushKind::Ink) };
        for f in 0..120u32 {
            let x = 20.0 + f as f32 * 4.0;
            let samples = vec![Sample::at(x, 40.0, 1.0, 0.0), Sample::at(x, 220.0, 1.0, 0.1)];
            e.apply(Command::Stroke { layer: 0, frame: f, brush: ink, samples }).unwrap();
        }
        let before: Vec<_> = (0..120).step_by(7).map(|f| render_frame(&e.project, f, RenderOptions::default())).collect();
        let all = e.project.memory_bytes();
        e.page_budget = all / 4;
        let freed = e.page(0).unwrap();
        // the working set (frames 1 to 49 here) stays, everything else may go
        let keep: u64 = e.working_set(0).iter().filter_map(|d| e.project.drawing(*d)).map(|s| s.memory_bytes()).sum();
        assert!(freed > 0 && e.project.memory_bytes() <= (all / 4).max(keep) + keep / 8, "{} of {all}, keep {keep}", e.project.memory_bytes());
        assert!(e.project.paged_count() > 60);
        assert!(e.working_set(0).iter().all(|d| e.project.drawing_owned(*d).unwrap().is_some()));
        // reading back gives the same pictures, and so does a save and load
        for (i, f) in (0..120).step_by(7).enumerate() {
            assert_eq!(render_frame(&e.project, f, RenderOptions::default()), before[i], "frame {f}");
        }
        let bytes = tooncraft_format::to_bytes(&e.project).unwrap();
        let back = tooncraft_format::from_bytes(&bytes).unwrap();
        // (transparent tiles a stroke touched are dropped on load, so compare pictures)
        let same = |x: &Project, y: &Project| {
            x.layers == y.layers && (0..120).all(|f| render_frame(x, f, RenderOptions::default()) == render_frame(y, f, RenderOptions::default()))
        };
        assert!(same(&back, &e.project));
        // painting on a paged drawing makes it resident and is undoable
        e.page(0).unwrap();
        let d = e.project.drawing_at(0, 100).unwrap();
        e.apply(Command::ClearDrawing { layer: 0, frame: 100 }).unwrap();
        assert!(e.project.drawing(d).unwrap().is_empty());
        e.undo().unwrap();
        assert_eq!(render_frame(&e.project, 100, RenderOptions::default()), render_frame(&back, 100, RenderOptions::default()));
        // a paged load stays under its budget
        let file = PageFile::create(None).unwrap();
        let paged = tooncraft_format::from_bytes_with(&bytes, Some((&file, all / 8))).unwrap();
        assert!(paged.memory_bytes() <= all / 8 && paged.paged_count() > 0, "{} > {}", paged.memory_bytes(), all / 8);
        assert!(same(&paged, &back));
    }
}
