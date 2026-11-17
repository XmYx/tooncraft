//! Exposure-sheet cells and the edits animators make to them.

use serde::{Deserialize, Serialize};

use crate::{DrawingId, Layer, MAX_FRAMES};

/// One frame of a layer's exposure sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Cell {
    /// Nothing is shown.
    #[default]
    Blank,
    /// A drawing starts here.
    Key(DrawingId),
    /// The drawing above keeps showing (a hold).
    Hold,
}

impl Cell {
    pub fn key(self) -> Option<DrawingId> {
        match self {
            Cell::Key(d) => Some(d),
            _ => None,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ExposureError {
    #[error("frame {0} has no drawing")]
    NoDrawing(u32),
    #[error("re-timing needs a step of 1 to {0} frames")]
    BadStep(u32),
    #[error("the range {0}…{1} is not inside the shot")]
    BadRange(u32, u32),
}

/// Frame of the key whose exposure contains `frame`.
pub(crate) fn key_frame(cells: &[Cell], frame: u32) -> Option<u32> {
    let mut f = frame as usize;
    loop {
        match cells.get(f)? {
            Cell::Key(_) => return Some(f as u32),
            Cell::Blank => return None,
            Cell::Hold => {
                if f == 0 {
                    return None;
                }
                f -= 1;
            }
        }
    }
}

pub(crate) fn resolve(cells: &[Cell], frame: u32) -> Option<DrawingId> {
    key_frame(cells, frame).and_then(|f| cells.get(f as usize)).and_then(|c| c.key())
}

/// Make a loaded sheet consistent: holds with nothing to hold become blanks, keys to unknown
/// drawings become blanks.
pub(crate) fn repair(cells: &mut [Cell], exists: impl Fn(DrawingId) -> bool) {
    let mut showing = false;
    for c in cells.iter_mut() {
        match *c {
            Cell::Key(d) if exists(d) => showing = true,
            Cell::Key(_) => {
                *c = Cell::Blank;
                showing = false;
            }
            Cell::Hold if !showing => *c = Cell::Blank,
            Cell::Hold => {}
            Cell::Blank => showing = false,
        }
    }
}

impl Layer {
    /// Insert a frame at `at`: a hold when it lands inside an exposure (the drawing shows one
    /// frame longer), a blank otherwise. The last frame of the shot falls off the end.
    pub fn insert_frame(&mut self, at: u32) {
        let len = self.cells.len();
        if at as usize >= len {
            return;
        }
        let cell = if at > 0 && resolve(&self.cells, at - 1).is_some() { Cell::Hold } else { Cell::Blank };
        self.cells.insert(at as usize, cell);
        self.cells.truncate(len);
    }

    /// Delete the frame at `at`; later frames move up and a blank is added at the end.
    pub fn delete_frame(&mut self, at: u32) {
        let len = self.cells.len();
        if at as usize >= len {
            return;
        }
        let removed = self.cells.remove(at as usize);
        // Deleting a key whose exposure continues: the next hold becomes that key.
        if let (Cell::Key(d), Some(next @ Cell::Hold)) = (removed, self.cells.get_mut(at as usize)) {
            *next = Cell::Key(d);
        }
        self.cells.push(Cell::Blank);
        repair(&mut self.cells, |_| true);
    }

    /// Show the exposure containing `frame` one frame longer (shifting later frames).
    pub fn extend_hold(&mut self, frame: u32) -> Result<(), ExposureError> {
        let (_, end) = self.exposure_range(frame).ok_or(ExposureError::NoDrawing(frame))?;
        let len = self.cells.len();
        if end as usize >= len {
            // Last exposure of the shot: turn the trailing blank (if any) into a hold.
            return Ok(());
        }
        self.cells.insert(end as usize, Cell::Hold);
        self.cells.truncate(len);
        Ok(())
    }

    /// Show the exposure containing `frame` one frame shorter (never below one frame).
    pub fn shorten_hold(&mut self, frame: u32) -> Result<(), ExposureError> {
        let (start, end) = self.exposure_range(frame).ok_or(ExposureError::NoDrawing(frame))?;
        if end - start <= 1 {
            return Ok(());
        }
        self.cells.remove(end as usize - 1);
        self.cells.push(Cell::Blank);
        repair(&mut self.cells, |_| true);
        Ok(())
    }

    /// Make the exposure containing `frame` exactly `length` frames long (later cells move).
    pub fn set_exposure_length(&mut self, frame: u32, length: u32) -> Result<(), ExposureError> {
        let (start, end) = self.exposure_range(frame).ok_or(ExposureError::NoDrawing(frame))?;
        let want = length.clamp(1, self.cells.len() as u32);
        let mut have = end - start;
        let mut guard = 0;
        while have < want && guard < 70_000 {
            let before = self.cells.clone();
            self.extend_hold(start)?;
            if self.cells == before {
                break; // end of the shot
            }
            have += 1;
            guard += 1;
        }
        while have > want && guard < 140_000 {
            self.shorten_hold(start)?;
            have -= 1;
            guard += 1;
        }
        Ok(())
    }

    /// Set the cell at `frame` to start `drawing` (the old exposure there is cut at `frame`).
    pub fn set_key(&mut self, frame: u32, drawing: DrawingId) {
        if let Some(c) = self.cells.get_mut(frame as usize) {
            *c = Cell::Key(drawing);
        }
    }

    /// Make the frame blank; a hold that followed it becomes blank too (the exposure is cut).
    pub fn clear(&mut self, frame: u32) {
        if let Some(c) = self.cells.get_mut(frame as usize) {
            *c = Cell::Blank;
        }
        repair(&mut self.cells, |_| true);
    }

    /// Re-time the frames `from..=to`: every drawing exposed in the range is shown for exactly
    /// `step` frames, in order. The range keeps its length; drawings that no longer fit fall out
    /// of the range but stay in the library. Frames after the range do not move.
    pub fn retime(&mut self, from: u32, to: u32, step: u32) -> Result<(), ExposureError> {
        if step == 0 || step > 64 {
            return Err(ExposureError::BadStep(64));
        }
        let len = self.cells.len() as u32;
        if from > to || to >= len || len > MAX_FRAMES {
            return Err(ExposureError::BadRange(from, to));
        }
        let mut order: Vec<DrawingId> = Vec::new();
        for f in from..=to {
            if let Some(d) = resolve(&self.cells, f)
                && order.last() != Some(&d)
            {
                order.push(d);
            }
        }
        let mut f = from;
        for d in order {
            for k in 0..step {
                if f > to {
                    break;
                }
                self.cells[f as usize] = if k == 0 { Cell::Key(d) } else { Cell::Hold };
                f += 1;
            }
        }
        while f <= to {
            self.cells[f as usize] = Cell::Blank;
            f += 1;
        }
        // A hold right after the range now continues the range's last drawing; keep the drawing
        // that was showing there before.
        repair(&mut self.cells, |_| true);
        Ok(())
    }
}

impl Layer {
    /// The cells `from..=to` as a self-contained block: a leading hold becomes a key of the
    /// drawing it continues, so the block pastes anywhere.
    pub fn copy_cells(&self, from: u32, to: u32) -> Vec<Cell> {
        let len = self.cells.len() as u32;
        if from > to || from >= len {
            return Vec::new();
        }
        let to = to.min(len - 1);
        let mut block: Vec<Cell> = self.cells[from as usize..=to as usize].to_vec();
        if let Some(first) = block.first_mut()
            && *first == Cell::Hold
        {
            *first = resolve(&self.cells, from).map(Cell::Key).unwrap_or(Cell::Blank);
        }
        block
    }

    /// Paste a block at `at`: over the existing cells, or inserted (later cells move right; the
    /// shot keeps its length, so cells pushed past the end fall off).
    pub fn paste_cells(&mut self, at: u32, block: &[Cell], insert: bool) {
        let len = self.cells.len();
        let at = at as usize;
        if at >= len || block.is_empty() {
            return;
        }
        if insert {
            let tail = self.cells.split_off(at);
            self.cells.extend_from_slice(block);
            self.cells.extend(tail);
            self.cells.truncate(len);
        } else {
            // A hold right after the pasted block would now continue the block's last drawing;
            // keep showing what it showed before.
            let after = at + block.len();
            if after < len && self.cells[after] == Cell::Hold {
                self.cells[after] = resolve(&self.cells, after as u32).map(Cell::Key).unwrap_or(Cell::Blank);
            }
            for (i, c) in block.iter().enumerate() {
                if let Some(cell) = self.cells.get_mut(at + i) {
                    *cell = *c;
                }
            }
        }
        repair(&mut self.cells, |_| true);
    }

    /// Clear `from..=to` (blank), or remove those frames so later ones move left.
    pub fn clear_range(&mut self, from: u32, to: u32, shift: bool) {
        let len = self.cells.len() as u32;
        if from > to || from >= len {
            return;
        }
        let to = to.min(len - 1);
        if shift {
            for _ in from..=to {
                self.delete_frame(from);
            }
        } else {
            let after = to as usize + 1;
            if (after as u32) < len && self.cells[after] == Cell::Hold {
                self.cells[after] = resolve(&self.cells, after as u32).map(Cell::Key).unwrap_or(Cell::Blank);
            }
            for f in from..=to {
                self.cells[f as usize] = Cell::Blank;
            }
            repair(&mut self.cells, |_| true);
        }
    }

    /// Play `from..=to` backwards: the exposures (and blanks) in the range in reverse order, each
    /// keeping its length.
    pub fn reverse_range(&mut self, from: u32, to: u32) -> Result<(), ExposureError> {
        let len = self.cells.len() as u32;
        if from > to || to >= len {
            return Err(ExposureError::BadRange(from, to));
        }
        let block = self.copy_cells(from, to);
        // runs of (drawing or blank, length)
        let mut runs: Vec<(Option<DrawingId>, u32)> = Vec::new();
        for c in &block {
            match c {
                Cell::Key(d) => runs.push((Some(*d), 1)),
                Cell::Hold => {
                    if let Some(r) = runs.last_mut() {
                        r.1 += 1;
                    }
                }
                Cell::Blank => match runs.last_mut() {
                    Some((None, n)) => *n += 1,
                    _ => runs.push((None, 1)),
                },
            }
        }
        runs.reverse();
        let mut out = Vec::with_capacity(block.len());
        for (d, n) in runs {
            for k in 0..n {
                out.push(match (d, k) {
                    (Some(d), 0) => Cell::Key(d),
                    (Some(_), _) => Cell::Hold,
                    (None, _) => Cell::Blank,
                });
            }
        }
        self.paste_cells(from, &out, false);
        Ok(())
    }

    /// Repeat `from..=to` `times` more times right after it (later cells move right).
    pub fn cycle(&mut self, from: u32, to: u32, times: u32) -> Result<(), ExposureError> {
        let len = self.cells.len() as u32;
        if from > to || to >= len {
            return Err(ExposureError::BadRange(from, to));
        }
        let block = self.copy_cells(from, to);
        let n = block.len() as u32;
        for k in 0..times.min(64) {
            let at = to + 1 + k * n;
            if at >= len {
                break;
            }
            self.paste_cells(at, &block, true);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LayerId;

    fn layer(cells: Vec<Cell>) -> Layer {
        Layer { id: LayerId(1), name: "L".into(), visible: true, locked: false, opacity: 1.0, cells }
    }
    const A: DrawingId = DrawingId(1);
    const B: DrawingId = DrawingId(2);
    use Cell::{Blank as X, Hold as H};
    fn k(d: DrawingId) -> Cell {
        Cell::Key(d)
    }

    #[test]
    fn resolve_follows_holds() {
        let l = layer(vec![k(A), H, H, X, k(B), H]);
        assert_eq!(l.drawing_at(2), Some(A));
        assert_eq!(l.drawing_at(3), None);
        assert_eq!(l.drawing_at(5), Some(B));
        assert_eq!(l.drawing_at(9), None);
        assert_eq!(l.exposure_range(1), Some((0, 3)));
    }

    #[test]
    fn insert_and_delete_frames() {
        let mut l = layer(vec![k(A), H, k(B), H, X, X]);
        l.insert_frame(1);
        assert_eq!(l.cells, vec![k(A), H, H, k(B), H, X]);
        l.delete_frame(0);
        assert_eq!(l.cells, vec![k(A), H, k(B), H, X, X], "deleting a key promotes its hold");
        l.insert_frame(5);
        assert_eq!(l.cells, vec![k(A), H, k(B), H, X, X]);
    }

    #[test]
    fn holds_extend_and_shorten() {
        let mut l = layer(vec![k(A), k(B), H, X]);
        l.extend_hold(0).unwrap();
        assert_eq!(l.cells, vec![k(A), H, k(B), H]);
        l.shorten_hold(3).unwrap();
        assert_eq!(l.cells, vec![k(A), H, k(B), X]);
        l.shorten_hold(2).unwrap();
        assert_eq!(l.cells, vec![k(A), H, k(B), X], "an exposure keeps one frame");
        assert!(l.extend_hold(3).is_err());
    }

    #[test]
    fn retime_on_twos() {
        let mut l = layer(vec![k(A), k(B), k(DrawingId(3)), k(DrawingId(4)), X, X, X, X, k(DrawingId(5))]);
        l.retime(0, 7, 2).unwrap();
        assert_eq!(l.cells, vec![k(A), H, k(B), H, k(DrawingId(3)), H, k(DrawingId(4)), H, k(DrawingId(5))]);
        l.retime(0, 7, 3).unwrap();
        assert_eq!(l.cells[..8], [k(A), H, H, k(B), H, H, k(DrawingId(3)), H]);
        assert!(l.retime(0, 99, 2).is_err());
        assert!(l.retime(0, 3, 0).is_err());
    }

    #[test]
    fn clear_cuts_the_exposure() {
        let mut l = layer(vec![k(A), H, H]);
        l.clear(1);
        assert_eq!(l.cells, vec![k(A), X, X]);
    }

    #[test]
    fn exposure_length() {
        let mut l = layer(vec![k(A), k(B), H, X, X, X]);
        l.set_exposure_length(0, 3).unwrap();
        assert_eq!(l.cells, vec![k(A), H, H, k(B), H, X]);
        l.set_exposure_length(4, 1).unwrap();
        assert_eq!(l.cells, vec![k(A), H, H, k(B), X, X]);
        l.set_exposure_length(0, 99).unwrap();
        assert_eq!(l.cells[..3], [k(A), H, H]);
        assert!(layer(vec![X, X]).set_exposure_length(1, 2).is_err());
    }

    #[test]
    fn copy_paste_clear_reverse_cycle() {
        let c = |d| Cell::Key(DrawingId(d));
        let mut l = layer(vec![c(1), H, c(2), H, H, X, X, X, X, X]);
        let block = l.copy_cells(1, 3);
        assert_eq!(block, vec![c(1), c(2), H], "a leading hold becomes its key");
        l.paste_cells(6, &block, false);
        assert_eq!(l.cells, vec![c(1), H, c(2), H, H, X, c(1), c(2), H, X]);
        l.paste_cells(0, &[c(3)], true);
        assert_eq!(l.cells, vec![c(3), c(1), H, c(2), H, H, X, c(1), c(2), H]);
        l.clear_range(1, 3, false);
        assert_eq!(l.cells, vec![c(3), X, X, X, c(2), H, X, c(1), c(2), H], "a cut exposure keeps its drawing");
        l.clear_range(1, 3, true);
        assert_eq!(l.cells, vec![c(3), c(2), H, X, c(1), c(2), H, X, X, X]);
        l.reverse_range(0, 6).unwrap();
        assert_eq!(l.cells, vec![c(2), H, c(1), X, c(2), H, c(3), X, X, X]);
        let mut l = layer(vec![c(1), c(2), H, X, X, X, X, X]);
        l.cycle(0, 2, 2).unwrap();
        assert_eq!(l.cells, vec![c(1), c(2), H, c(1), c(2), H, c(1), c(2)]);
        assert!(l.reverse_range(3, 99).is_err());
    }

    proptest::proptest! {
        /// Any sequence of edits keeps the sheet consistent: same length, no orphan holds.
        #[test]
        fn edits_keep_the_sheet_consistent(ops in proptest::collection::vec((0u8..11, 0u32..12, 0u32..12, 1u32..4), 0..60)) {
            let mut l = layer(vec![k(A), H, k(B), X, X, k(DrawingId(3)), H, H, X, X, X, X]);
            for (op, a, b, s) in ops {
                let _ = match op {
                    0 => { l.insert_frame(a); Ok(()) }
                    1 => { l.delete_frame(a); Ok(()) }
                    2 => l.extend_hold(a),
                    3 => l.shorten_hold(a),
                    4 => { l.set_key(a, DrawingId(b as u64 + 1)); Ok(()) }
                    7 => { let blk = l.copy_cells(a.min(b), a.max(b)); l.paste_cells(s * 3, &blk, s % 2 == 0); Ok(()) }
                    8 => { l.clear_range(a.min(b), a.max(b), s % 2 == 0); Ok(()) }
                    9 => l.reverse_range(a.min(b), a.max(b)).or(Ok(())),
                    10 => l.cycle(a.min(b), a.max(b), s).or(Ok(())),
                    5 => { l.clear(a); Ok(()) }
                    _ => l.retime(a.min(b), a.max(b), s),
                };
                proptest::prop_assert_eq!(l.cells.len(), 12);
                let mut c = l.cells.clone();
                repair(&mut c, |_| true);
                proptest::prop_assert_eq!(&c, &l.cells);
            }
        }
    }
}
