//! The loop range on the timeline ruler: a band between the in and out frames with a handle at
//! each end. Drag a handle to move that end; with a handle focused, ← and → move it a frame
//! (Shift: five). The other end stays, and the range never turns inside out.

use egui::{Color32, Key, Modifiers, Rect, Sense, Stroke, pos2, vec2};

use crate::app::ToonApp;

/// Which end of the loop range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum End {
    In,
    Out,
}

/// The loop range after moving `end` to frame `to` (0-based) in a shot of `len` frames. With no
/// range yet, the other end starts at the start or end of the shot. `None` for an empty shot.
pub fn moved(range: Option<(u32, u32)>, end: End, to: i64, len: u32) -> Option<(u32, u32)> {
    let last = len.checked_sub(1)?;
    let (a, b) = range.map(|(a, b)| (a.min(last), b.min(last))).unwrap_or((0, last));
    let (a, b) = (a.min(b), a.max(b));
    let to = to.clamp(0, last as i64) as u32;
    Some(match end {
        End::In => (to.min(b), b),
        End::Out => (a, to.max(a)),
    })
}

/// Paint the loop range on `ruler` and handle dragging its ends. `left` is the x of frame
/// `first`, `cell_w` the width of a frame, `cols` how many frames are visible.
#[allow(clippy::too_many_arguments)]
pub fn show(app: &mut ToonApp, ui: &mut egui::Ui, painter: &egui::Painter, ruler: Rect, left: f32, first: u32, cols: u32, cell_w: f32, colour: Color32) {
    let Some((a, b)) = app.tl.range else { return };
    let len = app.engine.project.length();
    let x = |f: u32| left + (f as f32 - first as f32) * cell_w;
    let right = left + cols as f32 * cell_w;
    let (x0, x1) = (x(a).max(left), x(b + 1).min(right));
    if x1 > x0 {
        painter.rect_filled(Rect::from_min_max(pos2(x0, ruler.top()), pos2(x1, ruler.top() + 3.0)), 0.0, colour);
    }
    for (end, f, edge) in [(End::In, a, x(a)), (End::Out, b, x(b + 1))] {
        if edge < left - 1.0 || edge > ruler.right() + 1.0 {
            continue;
        }
        let r = Rect::from_center_size(pos2(edge, ruler.top() + 6.0), vec2(8.0, 12.0));
        let resp = ui.interact(r, crate::timeline::grid_id().with(("loop-handle", end)), Sense::click_and_drag());
        let name = match end {
            End::In => tl!("Loop in"),
            End::Out => tl!("Loop out"),
        };
        resp.widget_info(|| egui::WidgetInfo::slider(true, f as f64 + 1.0, name.to_string()));
        let resp = resp.on_hover_text(tl!("Drag to move this end of the loop range, or use the arrow keys when it has the focus"));
        // a flag hanging from the top of the ruler, pointing into the range
        let dir = if end == End::In { 5.0 } else { -5.0 };
        painter.line_segment([pos2(edge, ruler.top()), pos2(edge, ruler.top() + 12.0)], Stroke::new(2.0, colour));
        painter.add(egui::Shape::convex_polygon(
            vec![pos2(edge, ruler.top()), pos2(edge + dir, ruler.top() + 3.0), pos2(edge, ruler.top() + 6.0)],
            colour,
            Stroke::NONE,
        ));
        if resp.has_focus() {
            painter.rect_stroke(r.expand(1.0), 2.0, Stroke::new(2.0, colour), egui::StrokeKind::Outside);
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(resp.id, egui::EventFilter { tab: false, horizontal_arrows: true, vertical_arrows: false, escape: false })
            });
            // Shift first: a plain-key check also matches Shift+key
            let step = |ui: &egui::Ui, k: Key| {
                ui.input_mut(|i| {
                    let big = 5 * i.consume_key(Modifiers::SHIFT, k) as i64;
                    big + i.consume_key(Modifiers::NONE, k) as i64
                })
            };
            let d = step(ui, Key::ArrowRight) - step(ui, Key::ArrowLeft);
            if d != 0 {
                app.tl.range = moved(app.tl.range, end, f as i64 + d, len);
            }
        }
        if resp.hovered() || resp.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        if resp.dragged()
            && let Some(p) = resp.interact_pointer_pos()
        {
            // the in handle sits on the left edge of its frame, the out handle on the right edge
            let at = first as f32 + (p.x - left) / cell_w.max(1.0);
            let to = if end == End::In { at.round() } else { at.round() - 1.0 };
            app.tl.range = moved(app.tl.range, end, to as i64, len);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_an_end_keeps_the_other_and_never_turns_the_range_inside_out() {
        assert_eq!(moved(Some((4, 10)), End::In, 6, 48), Some((6, 10)));
        assert_eq!(moved(Some((4, 10)), End::Out, 20, 48), Some((4, 20)));
        // past the other end: the range shrinks to one frame instead of flipping
        assert_eq!(moved(Some((4, 10)), End::In, 30, 48), Some((10, 10)));
        assert_eq!(moved(Some((4, 10)), End::Out, 1, 48), Some((4, 4)));
        // clamped to the shot
        assert_eq!(moved(Some((4, 10)), End::In, -5, 48), Some((0, 10)));
        assert_eq!(moved(Some((4, 10)), End::Out, 500, 48), Some((4, 47)));
        // no range yet: the other end is the shot's start or end
        assert_eq!(moved(None, End::In, 7, 48), Some((7, 47)));
        assert_eq!(moved(None, End::Out, 7, 48), Some((0, 7)));
        assert_eq!(moved(Some((1, 2)), End::In, 0, 0), None);
    }

    #[test]
    fn hostile_ranges_and_targets_give_a_range_inside_the_shot() {
        let odd = [0u32, 1, 2, 47, 48, 1000, u32::MAX - 1, u32::MAX];
        let tos = [i64::MIN, -1, 0, 1, 46, 47, 48, i64::MAX];
        for &len in &[0u32, 1, 2, 48, u32::MAX] {
            for &a in &odd {
                for &b in &odd {
                    for &to in &tos {
                        for end in [End::In, End::Out] {
                            match moved(Some((a, b)), end, to, len) {
                                None => assert_eq!(len, 0),
                                Some((x, y)) => assert!(x <= y && y < len, "{a} {b} {to} {len} → {x} {y}"),
                            }
                        }
                    }
                }
            }
        }
    }
}
