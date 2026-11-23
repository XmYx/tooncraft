//! The exposure sheet running down the page: layers as columns, frames as rows numbered down the
//! side, the way a paper sheet reads. It is the same grid as the timeline (same focus, keys, cell
//! names, right-click menus), transposed; View ▸ or the transport's "Vertical" switch picks it.
//! The first cell of an exposure shows the drawing's number; held cells show a line, and their
//! number faintly under the pointer.

use egui::{Align2, Color32, FontId, Rect, Sense, Stroke, pos2, vec2};
use tooncraft_doc::Cell;

use crate::app::ToonApp;
use crate::timeline::grid_id;

const ROW_H: f32 = 18.0;
const COL_W: f32 = 64.0;
const NUM_W: f32 = 44.0;
const HEAD_H: f32 = 22.0;

/// Draw the vertical sheet into the rest of `ui`.
pub fn show(app: &mut ToonApp, ui: &mut egui::Ui) {
    let area = ui.available_rect_before_wrap();
    if area.width() < NUM_W + 20.0 || area.height() < HEAD_H + ROW_H {
        return;
    }
    let pal = crate::theme::palette(app.light_theme);
    let len = app.engine.project.length();
    // columns left to right: bottom layer first, as on paper (BG on the right is also common;
    // ToonCraft keeps the timeline's order reversed so the top layer is the rightmost column)
    let mut cols = crate::layer_tree::rows(app);
    cols.reverse();
    let visible_rows = (((area.height() - HEAD_H) / ROW_H).floor() as u32).max(1);
    let visible_cols = (((area.width() - NUM_W) / COL_W).floor() as usize).max(1);
    let mut first = app.timeline_scroll.x.max(0.0) as u32;
    if app.frame < first {
        first = app.frame;
    } else if app.frame >= first + visible_rows {
        first = app.frame + 1 - visible_rows;
    }
    first = first.min(len.saturating_sub(visible_rows.min(len)));
    let cur_col = cols.iter().position(|&l| l == app.layer).unwrap_or(0);
    let mut col0 = app.timeline_scroll.y.max(0.0) as usize;
    if cur_col < col0 {
        col0 = cur_col;
    } else if cur_col >= col0 + visible_cols {
        col0 = cur_col + 1 - visible_cols;
    }
    let grid = ui.interact(area, grid_id(), Sense::click_and_drag());
    if grid.hovered() {
        let d = ui.input(|i| i.smooth_scroll_delta);
        if ui.input(|i| i.modifiers.shift) {
            col0 = (col0 as f32 - d.x.min(d.y) / COL_W).clamp(0.0, cols.len().saturating_sub(1) as f32) as usize;
        } else if d.y != 0.0 {
            first = (first as i64 + (-d.y / ROW_H).round() as i64).clamp(0, len.saturating_sub(1) as i64) as u32;
        }
    }
    app.timeline_scroll = vec2(first as f32, col0 as f32);
    grid.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Grid, true, tl!("Exposure sheet")));
    ui.ctx().accesskit_node_builder(grid.id, |n| {
        n.set_row_count(len as usize);
        n.set_column_count(cols.len());
    });
    if grid.has_focus() {
        ui.memory_mut(|m| m.set_focus_lock_filter(grid.id, egui::EventFilter { tab: false, horizontal_arrows: true, vertical_arrows: true, escape: false }));
        crate::timeline::grid_keys(app, ui);
    }
    let painter = ui.painter_at(area);
    painter.rect_filled(area, 0.0, pal.bg);
    let font = FontId::proportional(11.0);
    let fps = app.engine.project.fps().max(1);
    let mut clicked: Option<(usize, u32)> = None;
    let mut cell_menus: Vec<(usize, u32, egui::Response)> = Vec::new();
    let mut layer_menus: Vec<(usize, egui::Response)> = Vec::new();
    let mut current_cell: Option<(usize, u32, egui::Response)> = None;
    // frame numbers down the side, a stronger line at each second
    for r in 0..visible_rows.min(len - first) {
        let f = first + r;
        let y = area.top() + HEAD_H + r as f32 * ROW_H;
        let row = Rect::from_min_size(pos2(area.left(), y), vec2(area.width(), ROW_H));
        if f == app.frame {
            painter.rect_filled(row, 0.0, pal.raised);
        }
        let second = f.is_multiple_of(fps);
        painter.text(
            pos2(area.left() + NUM_W - 6.0, row.center().y),
            Align2::RIGHT_CENTER,
            format!("{}", f + 1),
            font.clone(),
            if second { pal.text } else { pal.weak },
        );
        let w = if (f + 1).is_multiple_of(fps) { 1.5 } else { 0.5 };
        painter.line_segment([row.left_bottom(), row.right_bottom()], Stroke::new(w, pal.grid));
    }
    // columns
    for (ci, &li) in cols.iter().enumerate().skip(col0).take(visible_cols) {
        let Some(layer) = app.engine.project.layers.get(li) else { continue };
        let x = area.left() + NUM_W + (ci - col0) as f32 * COL_W;
        let head = Rect::from_min_size(pos2(x, area.top()), vec2(COL_W, HEAD_H));
        let head_resp = ui.interact(head, grid_id().with(("vname", layer.id)), Sense::click());
        let label = tlf!("Layer {name}", name = layer.name);
        head_resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, li == app.layer, label.clone()));
        if head_resp.clicked() {
            clicked = Some((li, app.frame));
        }
        if head_resp.secondary_clicked() || head_resp.context_menu_opened() {
            layer_menus.push((li, head_resp));
        }
        let tint = layer.color.map(|c| Color32::from_rgb(c[0], c[1], c[2])).unwrap_or(pal.cell_key);
        painter.rect_filled(head.shrink(1.0), 2.0, if li == app.layer { pal.raised } else { pal.panel });
        painter.line_segment([head.left_bottom() + vec2(2.0, -2.0), head.right_bottom() + vec2(-2.0, -2.0)], Stroke::new(2.0, tint));
        painter.text(head.center(), Align2::CENTER_CENTER, &layer.name, font.clone(), pal.text);
        painter.line_segment([pos2(x, area.top()), pos2(x, area.bottom())], Stroke::new(1.0, pal.grid));
        for r in 0..visible_rows.min(len - first) {
            let f = first + r;
            let y = area.top() + HEAD_H + r as f32 * ROW_H;
            let rect = Rect::from_min_size(pos2(x, y), vec2(COL_W, ROW_H));
            let cell = layer.cells.get(f as usize).copied().unwrap_or_default();
            let resp = ui.interact(rect, grid_id().with((layer.id, f)), Sense::click());
            match cell {
                Cell::Key(d) => {
                    let text = match app.engine.project.drawing_names.get(&d) {
                        Some(n) => n.clone(),
                        None => format!("{}", d.0),
                    };
                    painter.text(rect.center(), Align2::CENTER_CENTER, text, font.clone(), pal.text);
                    if let Some(m) = app.engine.project.marks.get(&d) {
                        painter.text(rect.right_top() + vec2(-3.0, 1.0), Align2::RIGHT_TOP, m.letter(), FontId::proportional(9.0), tint);
                    }
                }
                Cell::Hold => {
                    painter.line_segment([pos2(rect.center().x, rect.top()), pos2(rect.center().x, rect.bottom())], Stroke::new(2.0, tint.gamma_multiply(0.7)));
                    // the held drawing's number, faintly, under the pointer
                    if resp.hovered()
                        && let Some(d) = layer.drawing_at(f)
                    {
                        painter.text(rect.center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, format!("{}", d.0), FontId::proportional(9.5), pal.weak);
                    }
                }
                Cell::Blank => {}
            }
            if li == app.layer && f == app.frame {
                painter.rect_stroke(rect.shrink(1.0), 2.0, Stroke::new(1.5, pal.accent), egui::StrokeKind::Inside);
            }
            let selected = li == app.layer && f == app.frame;
            let mut label = tlf!("{layer}, frame {frame}: {cell}", layer = layer.name, frame = f + 1, cell = crate::timeline::cell_text(cell));
            if let Some(m) = cell.key().and_then(|d| app.engine.project.marks.get(&d)) {
                label = format!("{label}, {}", crate::timing_panel::mark_label(Some(*m)).to_lowercase());
            }
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Cell, true, selected, label.clone()));
            let resp = crate::timeline::cell_tooltip(app, resp, cell, layer.drawing_at(f));
            if resp.clicked() {
                clicked = Some((li, f));
            }
            if resp.secondary_clicked() || resp.context_menu_opened() {
                cell_menus.push((li, f, resp));
            } else if selected {
                current_cell = Some((li, f, resp));
            }
        }
    }
    if let Some((li, f)) = clicked {
        app.sel_anchor = None;
        app.layer = li;
        app.frame = f;
        app.playback.stop();
        ui.memory_mut(|m| m.request_focus(grid_id()));
    }
    if let Some((li, f, resp)) = current_cell.take() {
        let now = crate::timeline::menu_key(ui) && ui.memory(|m| m.has_focus(grid_id()));
        crate::timeline::keyboard_menu(ui, &resp, now, |ui| crate::context_menus::cell(app, ui, li, f));
    }
    for (li, f, resp) in cell_menus {
        if resp.secondary_clicked() {
            app.layer = li;
            app.frame = f;
            app.sel_anchor = None;
            app.playback.stop();
        }
        resp.context_menu(|ui| crate::context_menus::cell(app, ui, li, f));
    }
    for (li, resp) in layer_menus {
        resp.context_menu(|ui| crate::context_menus::layer(app, ui, li));
    }
    ui.allocate_rect(area, Sense::hover());
}
