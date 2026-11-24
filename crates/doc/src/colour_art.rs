//! Line and colour art in one drawing: a colour layer is linked to a line layer and shows, at
//! every frame, the colour drawing paired with the line drawing exposed there. Its exposures are
//! never edited on their own: they are worked out again from the line layer after every change,
//! so a new drawing, a hold, a re-timing or a deletion on the line layer acts on both.
//!
//! A line drawing with no colour drawing yet shows nothing on the colour layer; painting there
//! makes the colour drawing and records the pair (`tooncraft-engine`).

use crate::exposure::Cell;
use crate::{DrawingId, Project};

/// Most drawing pairs on one colour layer.
pub const MAX_PAIRS: usize = crate::MAX_DRAWINGS;

/// The colour drawing paired with `line`, if any.
pub fn paired(pairs: &[(DrawingId, DrawingId)], line: DrawingId) -> Option<DrawingId> {
    pairs.iter().find(|(l, _)| *l == line).map(|(_, c)| *c)
}

/// The colour layer's cells for the line layer's `line` cells: each line key becomes its colour
/// drawing's key (or a blank when it has none), holds follow it.
pub fn follow(line: &[Cell], pairs: &[(DrawingId, DrawingId)]) -> Vec<Cell> {
    let mut out = Vec::with_capacity(line.len());
    let mut showing = false;
    for cell in line {
        match cell {
            Cell::Key(d) => match paired(pairs, *d) {
                Some(c) => {
                    out.push(Cell::Key(c));
                    showing = true;
                }
                None => {
                    out.push(Cell::Blank);
                    showing = false;
                }
            },
            Cell::Hold if showing => out.push(Cell::Hold),
            Cell::Hold | Cell::Blank => {
                out.push(Cell::Blank);
                showing = false;
            }
        }
    }
    out
}

impl Project {
    /// The index of the line layer a colour layer follows.
    pub fn line_layer_of(&self, layer: usize) -> Option<usize> {
        let id = self.layers.get(layer)?.colour_of?;
        self.layers.iter().position(|l| l.id == id)
    }

    /// The index of the first colour layer that follows `line`.
    pub fn colour_layer_of(&self, line: usize) -> Option<usize> {
        let id = self.layers.get(line)?.id;
        self.layers.iter().position(|l| l.colour_of == Some(id))
    }

    /// Bring every colour layer's exposures in line with its line layer. A colour layer whose
    /// line layer is gone, is itself, or is another colour layer keeps its cells and loses the
    /// link. Returns whether anything changed.
    pub fn sync_colour_layers(&mut self) -> bool {
        let mut changed = false;
        for i in 0..self.layers.len() {
            let Some(id) = self.layers.get(i).and_then(|l| l.colour_of) else { continue };
            let own = self.layers.get(i).map(|l| l.id);
            let line = self.layers.iter().position(|l| l.id == id && l.colour_of.is_none()).filter(|_| own != Some(id));
            let Some(li) = line else {
                if let Some(l) = self.layers.get_mut(i) {
                    l.colour_of = None;
                    changed = true;
                }
                continue;
            };
            let cells = match (self.layers.get(li), self.layers.get(i)) {
                (Some(line), Some(colour)) => follow(&line.cells, &colour.colour_pairs),
                _ => continue,
            };
            if let Some(l) = self.layers.get_mut(i)
                && l.cells != cells
            {
                l.cells = cells;
                changed = true;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exposure::resolve;
    use proptest::prelude::*;

    fn d(n: u64) -> DrawingId {
        DrawingId(n)
    }

    #[test]
    fn colour_follows_keys_holds_and_blanks() {
        let line = [Cell::Key(d(1)), Cell::Hold, Cell::Key(d(2)), Cell::Hold, Cell::Blank, Cell::Hold, Cell::Key(d(1))];
        let pairs = [(d(1), d(11))];
        let c = follow(&line, &pairs);
        assert_eq!(c, vec![Cell::Key(d(11)), Cell::Hold, Cell::Blank, Cell::Blank, Cell::Blank, Cell::Blank, Cell::Key(d(11))]);
    }

    #[test]
    fn sync_links_and_unlinks() {
        let mut p = Project::blank("x");
        let Ok(c) = p.add_layer("Colour", 0) else { panic!("a layer") };
        let line_id = p.layers[1].id;
        p.layers[c].colour_of = Some(line_id);
        p.layers[1].cells[0] = Cell::Key(d(5));
        p.layers[1].cells[1] = Cell::Hold;
        p.layers[c].colour_pairs.push((d(5), d(6)));
        assert!(p.sync_colour_layers());
        assert_eq!(&p.layers[c].cells[..3], &[Cell::Key(d(6)), Cell::Hold, Cell::Blank]);
        assert_eq!(p.line_layer_of(c), Some(1));
        assert_eq!(p.colour_layer_of(1), Some(c));
        assert!(!p.sync_colour_layers(), "nothing more to do");
        // a link to itself is dropped, the cells stay
        p.layers[c].colour_of = Some(p.layers[c].id);
        assert!(p.sync_colour_layers());
        assert_eq!(p.layers[c].colour_of, None);
        assert_eq!(p.layers[c].cells[0], Cell::Key(d(6)));
    }

    fn cell() -> impl Strategy<Value = Cell> {
        prop_oneof![Just(Cell::Blank), Just(Cell::Hold), (1u64..6).prop_map(|n| Cell::Key(DrawingId(n)))]
    }

    proptest! {
        /// Every frame of the colour layer shows the colour drawing paired with the line drawing
        /// shown there, or nothing.
        #[test]
        fn the_colour_layer_shows_the_pair_of_the_line_drawing(line in proptest::collection::vec(cell(), 0..60),
            paired_ids in proptest::collection::btree_set(1u64..6, 0..6)) {
            let pairs: Vec<(DrawingId, DrawingId)> = paired_ids.iter().map(|n| (d(*n), d(n + 100))).collect();
            let colour = follow(&line, &pairs);
            prop_assert_eq!(colour.len(), line.len());
            for f in 0..line.len() as u32 {
                let want = resolve(&line, f).and_then(|l| paired(&pairs, l));
                prop_assert_eq!(resolve(&colour, f), want, "frame {}", f);
            }
        }
    }
}
