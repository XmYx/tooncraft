//! Tool icons, drawn with egui shapes (original, code-drawn; no icon files).

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

use crate::app::Tool;

/// Draw the icon for `tool` centred in `rect`.
pub fn tool(p: &Painter, rect: Rect, tool: Tool, color: Color32) {
    let s = rect.width().min(rect.height()) * 0.36;
    let c = rect.center();
    let st = Stroke::new(1.6, color);
    let at = |x: f32, y: f32| -> Pos2 { c + vec2(x * s, y * s) };
    match tool {
        Tool::Pencil => {
            // a slanted pencil: body, sharpened tip, lead
            let body = [at(-0.55, 0.85), at(-0.85, 0.55), at(0.55, -0.85), at(0.85, -0.55)];
            p.add(Shape::convex_polygon(vec![body[0], body[1], body[2], body[3]], Color32::TRANSPARENT, st));
            p.line_segment([at(-0.85, 0.55), at(-1.0, 1.0)], st);
            p.line_segment([at(-0.55, 0.85), at(-1.0, 1.0)], st);
            p.circle_filled(at(-0.95, 0.95), 1.6, color);
        }
        Tool::Ink => {
            // a pen nib
            let nib = vec![at(0.0, -1.0), at(0.6, -0.1), at(0.0, 1.0), at(-0.6, -0.1)];
            p.add(Shape::convex_polygon(nib, Color32::TRANSPARENT, st));
            p.line_segment([at(0.0, -0.1), at(0.0, 1.0)], st);
            p.circle_filled(at(0.0, -0.1), 2.0, color);
        }
        Tool::Airbrush => {
            p.circle_filled(c, s * 0.35, color);
            for (i, r) in [(0, 0.7f32), (1, 0.9), (2, 0.8), (3, 1.0), (4, 0.75), (5, 0.95), (6, 0.85), (7, 0.65)] {
                let a = i as f32 * std::f32::consts::TAU / 8.0 + 0.3;
                p.circle_filled(c + vec2(a.cos(), a.sin()) * s * r, 1.2, color);
            }
        }
        Tool::Chalk => {
            let r = Rect::from_center_size(c, vec2(s * 1.0, s * 1.9));
            p.rect_stroke(r, 2.0, st, egui::StrokeKind::Middle);
            for k in 0..4 {
                let y = r.top() + (k as f32 + 0.6) * r.height() / 4.0;
                p.line_segment([pos2(r.left() + 2.0, y), pos2(r.right() - 2.0, y - 2.0)], Stroke::new(1.0, color));
            }
        }
        Tool::Eraser => {
            let body = vec![at(-1.0, 0.3), at(-0.2, -0.6), at(1.0, 0.4), at(0.2, 1.0)];
            p.add(Shape::convex_polygon(body, Color32::TRANSPARENT, st));
            p.line_segment([at(-0.6, -0.15), at(0.6, 0.85)], st);
        }
        Tool::Fill => {
            // a tipped bucket with a drop
            let bucket = vec![at(-0.8, -0.3), at(0.1, -1.0), at(0.8, -0.1), at(-0.1, 0.6)];
            p.add(Shape::convex_polygon(bucket, Color32::TRANSPARENT, st));
            p.circle_filled(at(0.7, 0.75), s * 0.22, color);
        }
        Tool::Picker => {
            p.line_segment([at(-0.9, 0.9), at(0.35, -0.35)], Stroke::new(2.4, color));
            p.circle_filled(at(0.55, -0.55), s * 0.38, color);
        }
        Tool::Select => {
            let r = egui::Rect::from_center_size(c, vec2(s * 1.8, s * 1.5));
            let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
            for w in corners.windows(2) {
                let (a, b) = (w[0], w[1]);
                for k in 0..4 {
                    let t0 = k as f32 / 4.0;
                    let t1 = t0 + 0.125;
                    p.line_segment([a + (b - a) * t0, a + (b - a) * t1], st);
                }
            }
        }
        Tool::Shape => {
            p.rect_stroke(egui::Rect::from_min_max(at(-1.0, -1.0), at(0.3, 0.3)), 1.0, st, egui::StrokeKind::Middle);
            p.circle_stroke(at(0.35, 0.35), s * 0.6, st);
        }
        Tool::Hand => {
            for k in 0..4 {
                let x = -0.6 + k as f32 * 0.4;
                p.line_segment([at(x, 0.2), at(x, -0.9 + (k as f32 - 1.5).abs() * 0.15)], Stroke::new(2.0, color));
            }
            p.add(Shape::convex_polygon(vec![at(-0.75, 0.1), at(0.75, 0.1), at(0.5, 1.0), at(-0.5, 1.0)], color, Stroke::NONE));
        }
    }
}
