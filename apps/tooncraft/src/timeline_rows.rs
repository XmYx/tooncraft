//! Extras drawn in the timeline's layer rows: the eye, lock and onion toggles (shared icons), the
//! mark dots on key cells and, for a layer on a peg, a mark at every frame where the peg (or a peg
//! it hangs from) has a key; the cell's name says "peg key" for screen readers.

use egui::{Color32, Painter, Pos2, Rect, Stroke, vec2};
use tooncraft_doc::{Mark, OnionSkin, PegId};

use crate::app::ToonApp;

/// The frames of `[from, to)` where the layer's peg chain has a key, capped at `to - from`.
pub fn key_frames(app: &ToonApp, peg: PegId, from: u32, to: u32) -> Vec<u32> {
    let pegs = &app.engine.project.pegs;
    let mut out = std::collections::BTreeSet::new();
    let mut next = Some(peg);
    let mut depth = 0;
    while let Some(id) = next {
        if depth > tooncraft_doc::peg::MAX_PEG_DEPTH {
            break;
        }
        depth += 1;
        let Some(p) = pegs.iter().find(|p| p.id == id) else { break };
        // the keys in range: binary search, then walk only those
        let start = p.keys.partition_point(|k| k.frame < from);
        for k in p.keys[start..].iter().take_while(|k| k.frame < to) {
            out.insert(k.frame);
        }
        next = p.parent;
    }
    out.into_iter().collect()
}

/// A diamond at the top of a cell.
pub fn paint_key(p: &Painter, at: Pos2, c: Color32, edge: Color32) {
    let r = 3.5;
    p.add(egui::Shape::convex_polygon(vec![at + vec2(0.0, -r), at + vec2(r, 0.0), at + vec2(0.0, r), at + vec2(-r, 0.0)], c, Stroke::new(1.0, edge)));
}

/// Whether the layer's drawings show onion skin (its own settings, or the shot's).
pub fn onion_included(own: Option<OnionSkin>) -> bool {
    own.is_none_or(|o| o.enabled)
}

/// The layer's own onion settings after flipping "Include in Onion": leaving out keeps the
/// layer's settings (or copies the shot's) with onion off; including again goes back to the
/// shot's settings when the layer's own were only a copy of them.
pub fn onion_include_toggled(own: Option<OnionSkin>, shot: OnionSkin) -> Option<OnionSkin> {
    match own {
        Some(o) if !o.enabled => {
            let on = OnionSkin { enabled: true, ..o };
            (on != OnionSkin { enabled: true, ..shot }).then_some(on)
        }
        Some(o) => Some(OnionSkin { enabled: false, ..o }),
        None => Some(OnionSkin { enabled: false, ..shot }),
    }
}

#[cfg(test)]
mod onion_include_tests {
    use super::*;

    #[test]
    fn leaving_a_layer_out_and_back_in_restores_the_shot_settings() {
        let shot = OnionSkin { enabled: true, before: 3, ..OnionSkin::default() };
        assert!(onion_included(None));
        let out = onion_include_toggled(None, shot);
        assert!(!onion_included(out));
        assert_eq!(out.map(|o| o.before), Some(3));
        // back in: the copy is dropped, the layer follows the shot again
        assert_eq!(onion_include_toggled(out, shot), None);
    }

    #[test]
    fn a_layer_with_its_own_settings_keeps_them_through_the_toggle() {
        let shot = OnionSkin::default();
        let own = OnionSkin { enabled: true, before: 7, outline: true, ..shot };
        let out = onion_include_toggled(Some(own), shot);
        assert_eq!(out, Some(OnionSkin { enabled: false, ..own }));
        assert_eq!(onion_include_toggled(out, shot), Some(own));
    }
}

/// A row toggle drawn with the shared icon: filled in `on_colour` when on, outlined in `off`.
pub fn paint_toggle(p: &Painter, r: Rect, icon: crate::icons::Icon, on: bool, on_colour: Color32, off: Color32) {
    crate::icons::paint(p, r, icon, if on { on_colour } else { off }, on);
}

/// The colour of a mark's dot.
pub fn mark_colour(m: Mark, pal: &crate::theme::Palette) -> Color32 {
    match m {
        Mark::Key => pal.mark_key,
        Mark::Breakdown => pal.mark_bd,
        Mark::Inbetween => pal.mark_ib,
    }
}

/// A cell's mark: a dot in the mark's colour at the cell's top right, ringed in the background
/// colour so it reads on a key cell's fill.
pub fn paint_mark(p: &Painter, cell: Rect, m: Mark, pal: &crate::theme::Palette) {
    let rad = (cell.height() * 0.16).clamp(2.0, 4.0);
    let at = cell.right_top() + vec2(-rad - 1.5, rad + 1.5);
    p.circle(at, rad, mark_colour(m, pal), Stroke::new(1.0, pal.bg));
}

#[cfg(test)]
mod mark_tests {
    use super::*;

    #[test]
    fn each_mark_has_its_own_colour_in_both_themes() {
        for light in [false, true] {
            let pal = crate::theme::palette(light);
            let c: Vec<Color32> = Mark::ALL.iter().map(|m| mark_colour(*m, pal)).collect();
            assert!(c[0] != c[1] && c[1] != c[2] && c[0] != c[2], "{c:?}");
        }
    }
}
