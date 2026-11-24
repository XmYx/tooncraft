//! Extras drawn in the timeline's layer rows: the lock toggle and, for a layer on a peg, a mark
//! at every frame where the peg (or a peg it hangs from) has a key, so the motion's timing reads
//! against the drawings; the cell's name says "peg key" for screen readers.

use egui::{Color32, Painter, Pos2, Rect, Stroke, pos2, vec2};
use tooncraft_doc::PegId;

use crate::app::ToonApp;

/// A small padlock: filled and shut when locked, open (shackle raised) when not.
pub fn paint_lock(p: &Painter, r: Rect, locked: bool, c: Color32) {
    let st = Stroke::new(1.3, c);
    let body = Rect::from_min_max(pos2(r.left() + 3.0, r.center().y), pos2(r.right() - 3.0, r.bottom() - 2.0));
    if locked {
        p.rect_filled(body, 1.5, c);
    } else {
        p.rect_stroke(body, 1.5, st, egui::StrokeKind::Middle);
    }
    let lift = if locked { 0.0 } else { 3.0 };
    let (l, rr, top) = (body.left() + 2.0, body.right() - 2.0, r.top() + 2.0 - lift);
    let shackle = vec![pos2(l, body.top() - lift), pos2(l, top + 2.0), pos2((l + rr) * 0.5, top), pos2(rr, top + 2.0), pos2(rr, body.top() - lift)];
    p.add(egui::Shape::line(shackle, st));
}

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
