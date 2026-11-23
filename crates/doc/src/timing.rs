//! Timing and planning: drawing marks (key, breakdown, inbetween), spacing charts, timing presets
//! and re-timing that keeps the keys where they are.
//!
//! A spacing chart says where the inbetweens between two drawings fall: with an even chart they
//! are equally spaced, with a slow start they bunch up near the first drawing, with a slow end
//! near the second. The same [`Ease`] curves the pegs use give the fractions.

use serde::{Deserialize, Serialize};

use crate::exposure::{ExposureError, repair, resolve};
use crate::{Cell, DrawingId, Ease, Layer};

/// What a drawing is in the plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mark {
    /// A main pose.
    Key,
    /// The drawing that decides the path between two keys.
    Breakdown,
    /// A drawing between keys and breakdowns.
    Inbetween,
}

impl Mark {
    pub const ALL: [Mark; 3] = [Mark::Key, Mark::Breakdown, Mark::Inbetween];

    /// One letter for the timeline.
    pub fn letter(self) -> &'static str {
        match self {
            Mark::Key => "K",
            Mark::Breakdown => "B",
            Mark::Inbetween => "i",
        }
    }

    /// Anchors that re-timing keeps in place.
    pub fn is_anchor(self) -> bool {
        matches!(self, Mark::Key | Mark::Breakdown)
    }
}

/// Ready-made timings: how long each drawing in a range is held.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum TimingPreset {
    /// Every drawing held `frames` frames (1 = on ones, 2 = on twos …).
    Even { frames: u32 },
    /// Long holds first, getting shorter (a move that starts slowly).
    SlowStart,
    /// Short holds first, getting longer (a move that settles).
    SlowEnd,
    /// Long at both ends, short in the middle.
    SlowBoth,
    /// These holds in order; the last repeats.
    Holds { holds: Vec<u32> },
}

/// Longest hold a preset gives.
pub const MAX_HOLD: u32 = 64;

impl TimingPreset {
    /// The hold of each of `n` drawings.
    pub fn holds(&self, n: usize) -> Vec<u32> {
        let n = n.min(crate::MAX_FRAMES as usize);
        // 3 at the slow end, 1 at the fast end, in equal thirds
        let ramp = |t: f32| 1 + ((3.0 * t.clamp(0.0, 1.0)).floor() as u32).min(2);
        let pos = |i: usize| if n > 1 { i as f32 / (n - 1) as f32 } else { 0.0 };
        match self {
            TimingPreset::Even { frames } => vec![(*frames).clamp(1, MAX_HOLD); n],
            TimingPreset::SlowStart => (0..n).map(|i| ramp(1.0 - pos(i))).collect(),
            TimingPreset::SlowEnd => (0..n).map(|i| ramp(pos(i))).collect(),
            TimingPreset::SlowBoth => (0..n).map(|i| ramp((pos(i) - 0.5).abs() * 2.0)).collect(),
            TimingPreset::Holds { holds } => {
                let last = holds.last().copied().unwrap_or(1);
                (0..n).map(|i| holds.get(i).copied().unwrap_or(last).clamp(1, MAX_HOLD)).collect()
            }
        }
    }
}

/// Where `inbetweens` drawings fall between two drawings, as fractions of the way (strictly
/// between 0 and 1, increasing) following `ease`.
pub fn spacing(inbetweens: u32, ease: Ease) -> Vec<f32> {
    let n = inbetweens.min(1000);
    let ease = if ease == Ease::Hold { Ease::Linear } else { ease };
    (1..=n).map(|i| ease.apply(i as f32 / (n + 1) as f32)).collect()
}

/// One exposure of a drawing: its first frame and how many frames it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exposure {
    pub start: u32,
    pub drawing: DrawingId,
    pub len: u32,
}

impl Layer {
    /// The exposures that start inside `from..=to` (an exposure already running at `from` counts
    /// from `from`), cut at `to`.
    pub fn exposures(&self, from: u32, to: u32) -> Vec<Exposure> {
        let mut out: Vec<Exposure> = Vec::new();
        let to = to.min((self.cells.len() as u32).saturating_sub(1));
        for f in from..=to {
            let Some(d) = resolve(&self.cells, f) else { continue };
            let starts = f == from || matches!(self.cells.get(f as usize), Some(Cell::Key(_)));
            match out.last_mut() {
                Some(e) if !starts && e.drawing == d && e.start + e.len == f => e.len += 1,
                _ => out.push(Exposure { start: f, drawing: d, len: 1 }),
            }
        }
        out
    }

    fn check_range(&self, from: u32, to: u32) -> Result<(), ExposureError> {
        if from > to || to as usize >= self.cells.len() {
            return Err(ExposureError::BadRange(from, to));
        }
        Ok(())
    }

    /// Re-time `from..=to` with a preset: the drawings in the range, in order, each held as the
    /// preset says. The range keeps its length (drawings that no longer fit stay in the library);
    /// later frames do not move.
    pub fn apply_timing(&mut self, from: u32, to: u32, preset: &TimingPreset) -> Result<(), ExposureError> {
        self.check_range(from, to)?;
        let order: Vec<DrawingId> = self.exposures(from, to).iter().map(|e| e.drawing).collect();
        let holds = preset.holds(order.len());
        let mut f = from;
        for (d, h) in order.iter().zip(holds) {
            for k in 0..h {
                if f > to {
                    break;
                }
                self.cells[f as usize] = if k == 0 { Cell::Key(*d) } else { Cell::Hold };
                f += 1;
            }
        }
        while f <= to {
            self.cells[f as usize] = Cell::Blank;
            f += 1;
        }
        repair(&mut self.cells, |_| true);
        Ok(())
    }

    /// Re-space the drawings between anchors (drawings for which `anchor` is true, normally keys
    /// and breakdowns) inside `from..=to`: every anchor keeps its frame, and the drawings between
    /// two anchors are moved to the frames the spacing chart for `ease` gives. Frames before the
    /// first anchor and after the last one are left alone. A gap too short for its drawings is
    /// an error and changes nothing.
    pub fn retime_between_anchors(&mut self, from: u32, to: u32, anchor: impl Fn(DrawingId) -> bool, ease: Ease) -> Result<usize, ExposureError> {
        self.check_range(from, to)?;
        let ex = self.exposures(from, to);
        let anchors: Vec<usize> = ex.iter().enumerate().filter(|(_, e)| anchor(e.drawing)).map(|(i, _)| i).collect();
        let mut plan: Vec<(u32, u32, Vec<(u32, DrawingId)>)> = Vec::new();
        for w in anchors.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (sa, sb) = (ex[a].start, ex[b].start);
            let between: Vec<DrawingId> = ex[a + 1..b].iter().map(|e| e.drawing).collect();
            if (sb - sa) as usize <= between.len() {
                return Err(ExposureError::BadRange(sa, sb));
            }
            let gap = (sb - sa) as f32;
            let mut placed: Vec<(u32, DrawingId)> = vec![(sa, ex[a].drawing)];
            for (frac, d) in spacing(between.len() as u32, ease).into_iter().zip(between) {
                let prev = placed.last().map(|p| p.0).unwrap_or(sa);
                let at = (sa + (frac.clamp(0.0, 1.0) * gap).round() as u32).max(prev + 1);
                placed.push((at, d));
            }
            // keep every drawing strictly before the next anchor, pushing back from the end
            let mut limit = sb;
            for p in placed.iter_mut().skip(1).rev() {
                if p.0 >= limit {
                    p.0 = limit - 1;
                }
                limit = p.0;
            }
            if placed.windows(2).any(|w| w[1].0 <= w[0].0) {
                return Err(ExposureError::BadRange(sa, sb));
            }
            plan.push((sa, sb, placed));
        }
        let moved = plan.iter().map(|(_, _, p)| p.len().saturating_sub(1)).sum();
        for (sa, sb, placed) in plan {
            for f in sa..sb {
                self.cells[f as usize] = Cell::Hold;
            }
            for (f, d) in placed {
                self.cells[f as usize] = Cell::Key(d);
            }
        }
        repair(&mut self.cells, |_| true);
        Ok(moved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LayerId, Project};

    fn layer(p: &mut Project, sheet: &str) -> Layer {
        let mut cells = Vec::new();
        for c in sheet.chars() {
            cells.push(match c {
                '#' => Cell::Key(p.add_drawing(None).unwrap()),
                '-' => Cell::Hold,
                _ => Cell::Blank,
            });
        }
        Layer {
            id: LayerId(9),
            name: "L".into(),
            visible: true,
            locked: false,
            opacity: 1.0,
            blend: Default::default(),
            color: None,
            folder: None,
            peg: None,
            depth: 0.0,
            clip: false,
            lock_alpha: false,
            onion: None,
            effects: Vec::new(),
            opacity_keys: Vec::new(),
            matte: None,
            mouth_chart: Default::default(),
            cells,
        }
    }

    fn sheet(l: &Layer) -> String {
        l.cells
            .iter()
            .map(|c| match c {
                Cell::Key(_) => '#',
                Cell::Hold => '-',
                Cell::Blank => '.',
            })
            .collect()
    }

    #[test]
    fn spacing_charts_bunch_where_the_move_is_slow() {
        let even = spacing(3, Ease::Linear);
        assert_eq!(even, vec![0.25, 0.5, 0.75]);
        let slow_start = spacing(3, Ease::In);
        assert!(slow_start[0] < 0.25 && slow_start.windows(2).all(|w| w[1] > w[0]));
        let slow_end = spacing(3, Ease::Out);
        assert!(slow_end[2] > 0.75);
        assert!(spacing(0, Ease::InOut).is_empty());
        assert_eq!(spacing(2, Ease::Hold).len(), 2);
    }

    #[test]
    fn presets_hold_each_drawing() {
        assert_eq!(TimingPreset::Even { frames: 2 }.holds(3), vec![2, 2, 2]);
        assert_eq!(TimingPreset::SlowStart.holds(5), vec![3, 3, 2, 1, 1]);
        assert_eq!(TimingPreset::SlowEnd.holds(5), vec![1, 1, 2, 3, 3]);
        assert_eq!(TimingPreset::SlowBoth.holds(5), vec![3, 2, 1, 2, 3]);
        assert_eq!(TimingPreset::Holds { holds: vec![4, 1, 99] }.holds(4), vec![4, 1, 64, 64]);
        assert_eq!(TimingPreset::Even { frames: 0 }.holds(1), vec![1]);
        let mut p = Project::new("t", 8, 8, 24, 12).unwrap();
        let mut l = layer(&mut p, "####........");
        l.apply_timing(0, 11, &TimingPreset::SlowEnd).unwrap();
        assert_eq!(sheet(&l), "##-#--#--...");
        assert!(l.apply_timing(5, 99, &TimingPreset::SlowEnd).is_err());
    }

    #[test]
    fn retiming_between_anchors_keeps_them_in_place() {
        let mut p = Project::new("t", 8, 8, 24, 13).unwrap();
        // keys at 0 and 12, three inbetweens bunched at the start
        let mut l = layer(&mut p, "####--------#");
        let key = |l: &Layer, f: usize| l.cells[f].key().unwrap();
        let (k1, k2) = (key(&l, 0), key(&l, 12));
        let anchors = [k1, k2];
        let n = l.retime_between_anchors(0, 12, |d| anchors.contains(&d), Ease::Linear).unwrap();
        assert_eq!(n, 3);
        assert_eq!(sheet(&l), "#--#--#--#--#");
        assert_eq!((key(&l, 0), key(&l, 12)), (k1, k2), "the keys did not move");
        l.retime_between_anchors(0, 12, |d| anchors.contains(&d), Ease::In).unwrap();
        let s = sheet(&l);
        assert_eq!((s.chars().next(), s.chars().nth(12)), (Some('#'), Some('#')));
        assert_eq!(s.matches('#').count(), 5);
        assert_eq!(s, "##--#--#----#", "a slow start bunches the inbetweens near the first key");
        // too many drawings for the gap
        let mut tight = layer(&mut p, "####.");
        let ids: Vec<DrawingId> = tight.cells.iter().filter_map(|c| c.key()).collect();
        let (a, b) = (ids[0], ids[1]);
        assert!(tight.retime_between_anchors(0, 4, |d| d == a || d == ids[3], Ease::Linear).is_ok(), "3 frames for 2 inbetweens");
        let before = tight.clone();
        assert!(tight.retime_between_anchors(0, 4, |d| d == a || d == b, Ease::Linear).is_ok());
        assert_eq!(tight, before, "adjacent anchors: nothing between them");
    }

    #[test]
    fn exposures_list_runs() {
        let mut p = Project::new("t", 8, 8, 24, 8).unwrap();
        let l = layer(&mut p, "#--#..#-");
        let e = l.exposures(1, 7);
        assert_eq!(e.iter().map(|e| (e.start, e.len)).collect::<Vec<_>>(), vec![(1, 2), (3, 1), (6, 2)]);
    }
}
