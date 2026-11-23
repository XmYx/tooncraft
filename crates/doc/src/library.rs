//! The shot's drawing library: named, reusable runs of drawings with their timing (a blink, a walk
//! cycle, a prop). An item keeps its own copies of the drawings in the shot's drawing table, so
//! editing the layer it came from does not change it. Inserting an item copies its cells onto a
//! layer, either as new drawings or linked (the layer shows the item's own drawings, and editing
//! them in place changes every linked use).
//!
//! Hostile input is bounded: at most [`MAX_ITEMS`] items of [`MAX_ITEM_CELLS`] cells, names cut to
//! [`MAX_NAME`] characters, and cells that point at missing drawings become blanks.

use serde::{Deserialize, Serialize};

use crate::DrawingId;
use crate::exposure::{self, Cell};

pub const MAX_ITEMS: usize = 256;
pub const MAX_ITEM_CELLS: usize = 1024;
pub const MAX_NAME: usize = 80;

/// One library item: a name and its exposure cells (the first is never a hold).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub name: String,
    pub cells: Vec<Cell>,
}

/// A name without control characters, trimmed and cut to [`MAX_NAME`]; empty becomes "Drawing".
pub fn clean_name(s: &str) -> String {
    let n: String = s.chars().filter(|c| !c.is_control()).take(MAX_NAME).collect::<String>().trim().to_string();
    if n.is_empty() { "Drawing".into() } else { n }
}

impl Item {
    /// The drawings the item uses, each once, in the order they first show.
    pub fn drawings(&self) -> Vec<DrawingId> {
        let mut out: Vec<DrawingId> = Vec::new();
        for c in &self.cells {
            if let Cell::Key(d) = c
                && !out.contains(d)
            {
                out.push(*d);
            }
        }
        out
    }

    /// Frames the item lasts.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// The item within the limits: name cleaned, cells cut, holds with nothing to hold and keys to
    /// drawings `exists` does not know become blanks, trailing blanks dropped.
    pub fn sanitized(mut self, exists: impl Fn(DrawingId) -> bool) -> Self {
        self.name = clean_name(&self.name);
        self.cells.truncate(MAX_ITEM_CELLS);
        exposure::repair(&mut self.cells, exists);
        while self.cells.last() == Some(&Cell::Blank) {
            self.cells.pop();
        }
        self
    }
}

/// A list of items within the limits (see [`Item::sanitized`]); items left empty are dropped.
pub fn sanitize_all(v: Vec<Item>, exists: impl Fn(DrawingId) -> bool) -> Vec<Item> {
    v.into_iter().take(MAX_ITEMS).map(|i| i.sanitized(&exists)).filter(|i| !i.is_empty()).collect()
}

/// The cells of `cells[from..=to]` (either order) as an item's cells: a leading hold becomes the
/// key it holds, trailing blanks are dropped. `None` when the range shows no drawing, is not inside
/// `cells`, or is longer than [`MAX_ITEM_CELLS`].
pub fn cells_from(cells: &[Cell], from: u32, to: u32) -> Option<Vec<Cell>> {
    let (a, b) = (from.min(to) as usize, from.max(to) as usize);
    if b >= cells.len() || b - a + 1 > MAX_ITEM_CELLS {
        return None;
    }
    let mut out: Vec<Cell> = cells[a..=b].to_vec();
    if out.first() == Some(&Cell::Hold) {
        out[0] = exposure::resolve(cells, a as u32).map(Cell::Key).unwrap_or(Cell::Blank);
    }
    exposure::repair(&mut out, |_| true);
    while out.last() == Some(&Cell::Blank) {
        out.pop();
    }
    if out.iter().any(|c| matches!(c, Cell::Key(_))) { Some(out) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    const D1: DrawingId = DrawingId(1);
    const D2: DrawingId = DrawingId(2);

    #[test]
    fn a_range_starting_in_a_hold_keeps_the_drawing_it_holds() {
        let cells = [Cell::Key(D1), Cell::Hold, Cell::Hold, Cell::Key(D2), Cell::Blank, Cell::Blank];
        assert_eq!(cells_from(&cells, 1, 5), Some(vec![Cell::Key(D1), Cell::Hold, Cell::Key(D2)]));
        assert_eq!(cells_from(&cells, 4, 5), None, "only blanks");
        assert_eq!(cells_from(&cells, 0, 9), None, "past the end");
        assert_eq!(cells_from(&cells, 3, 0), Some(vec![Cell::Key(D1), Cell::Hold, Cell::Hold, Cell::Key(D2)]), "either order");
    }

    #[test]
    fn hostile_items_are_cut_and_repaired() {
        let long = Item { name: "x".into(), cells: vec![Cell::Hold; MAX_ITEM_CELLS * 3] };
        let missing = Item { name: "  ".into(), cells: vec![Cell::Key(DrawingId(99)), Cell::Hold, Cell::Key(D1), Cell::Blank] };
        let many: Vec<Item> = std::iter::repeat_n(missing.clone(), MAX_ITEMS * 2).collect();
        let v = sanitize_all(vec![long, missing], |d| d == D1);
        assert_eq!(v.len(), 1, "an item of holds with nothing to hold is all blanks and dropped");
        assert_eq!(v[0].name, "Drawing");
        assert_eq!(v[0].cells, vec![Cell::Blank, Cell::Blank, Cell::Key(D1)]);
        assert_eq!(sanitize_all(many, |d| d == D1).len(), MAX_ITEMS);
        let named = Item { name: "\u{7}x".repeat(500), cells: vec![Cell::Key(D1); MAX_ITEM_CELLS + 5] }.sanitized(|_| true);
        assert_eq!(named.name.chars().count(), MAX_NAME);
        assert!(!named.name.contains('\u{7}'));
        assert_eq!(named.len(), MAX_ITEM_CELLS);
    }

    #[test]
    fn drawings_are_listed_once_in_showing_order() {
        let i = Item { name: "a".into(), cells: vec![Cell::Key(D2), Cell::Key(D1), Cell::Hold, Cell::Key(D2)] };
        assert_eq!(i.drawings(), vec![D2, D1]);
        assert_eq!(i.len(), 4);
    }
}
