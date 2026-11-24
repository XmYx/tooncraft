//! The timeline: transport controls and the exposure sheet (layers × frames).
//!
//! The grid is one focusable widget (role grid, "Exposure sheet") with a child node for every
//! visible cell (role cell, named "<layer>, frame <n>: key / hold / blank"), so a screen reader
//! can read the sheet. With the grid focused: ←/→ frame, ↑/↓ layer, Home/End, +/− hold, Insert,
//! Shift+Delete, Delete clear, Shift+N new drawing, Shift+D duplicate. Every grid action also has
//! a button or menu item; scrubbing by dragging the ruler has the keyboard as its alternative.

use egui::{Align2, Color32, FontId, Key, Rect, Sense, Stroke, pos2, vec2};
use tooncraft_doc::{Cell, Pin};
use tooncraft_engine::Command;

use crate::app::ToonApp;

pub fn grid_id() -> egui::Id {
    egui::Id::unique("tooncraft-exposure-sheet")
}

const NAME_W: f32 = 152.0;
const CELL_W0: f32 = 17.0;
const ROW_H0: f32 = 22.0;
const RULER_H: f32 = 20.0;

pub fn cell_text(cell: Cell) -> String {
    match cell {
        Cell::Key(d) => tlf!("drawing {id}", id = d.0),
        Cell::Hold => tl!("hold").to_string(),
        Cell::Blank => tl!("blank").to_string(),
    }
}

pub fn grid_keys(app: &mut ToonApp, ui: &egui::Ui) {
    let (layer, frame) = (app.layer, app.frame);
    let vertical = app.tl.vertical;
    let (k, shift) = ui.input(|i| {
        let arrow = |k| (i.modifiers.is_none() || i.modifiers.shift_only()) && i.key_pressed(k);
        let (l, r, u, d) = (arrow(Key::ArrowLeft), arrow(Key::ArrowRight), arrow(Key::ArrowUp), arrow(Key::ArrowDown));
        // [previous frame, next frame, layer above, layer below]; down the page, frames run
        // up and down and the top layer is the rightmost column
        let k = if vertical { [u, d, r, l] } else { [l, r, u, d] };
        (k, i.modifiers.shift)
    });
    if k.iter().any(|v| *v) {
        if shift {
            if app.sel_anchor.is_none() {
                app.sel_anchor = Some((layer, frame));
            }
        } else {
            app.sel_anchor = None;
        }
    }
    if ui.input(|i| i.key_pressed(Key::Escape)) {
        app.sel_anchor = None;
    }
    // 1 to 9: show the drawing here for that many frames
    const DIGITS: [Key; 9] = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9];
    let digit = ui.input(|i| DIGITS.iter().position(|k| i.modifiers.is_none() && i.key_pressed(*k)));
    if let Some(n) = digit
        && app.engine.project.layers.get(layer).and_then(|l| l.drawing_at(frame)).is_some()
        && app.run(Command::SetExposure { layer, frame, length: n as u32 + 1 })
    {
        app.set_status(tlf!("Held for {n} frames", n = n + 1));
    }
    if k[0] {
        app.go_to(frame as i64 - 1);
    }
    if k[1] {
        app.go_to(frame as i64 + 1);
    }
    // up / down step through the listed rows (folded folders and the label filter skip layers)
    if k[2] || k[3] {
        let rows = crate::layer_tree::rows(app);
        if let Some(i) = rows.iter().position(|&l| l == layer) {
            let j = if k[2] { i.saturating_sub(1) } else { (i + 1).min(rows.len().saturating_sub(1)) };
            app.layer = rows.get(j).copied().unwrap_or(layer);
        }
    }
}

#[allow(non_snake_case)]
pub fn show(app: &mut ToonApp, ui: &mut egui::Ui) {
    crate::timeline_header::show(app, ui);
    if app.tl.vertical {
        let (from, to) = (app.timeline_scroll.x.max(0.0) as u32, app.timeline_scroll.x.max(0.0) as u32 + 80);
        refresh_thumbnails(app, ui.ctx(), from, to);
        crate::sheet::show(app, ui);
        return;
    }
    let (CELL_W, ROW_H) = if app.thumbnails { (46.0, 42.0) } else { (CELL_W0, ROW_H0) };
    let CELL_W = CELL_W * app.tl.zoom;
    ui.add_space(2.0);
    let area = ui.available_rect_before_wrap();
    if area.width() < 40.0 || area.height() < RULER_H + 4.0 {
        return;
    }
    let pal = crate::theme::palette(app.light_theme);
    let len = app.engine.project.length();
    let rows = crate::layer_tree::rows(app);
    let n_layers = rows.len();
    let frames_area = Rect::from_min_max(pos2(area.left() + NAME_W, area.top()), area.max);
    let visible_cols = ((frames_area.width() / CELL_W).floor() as u32).max(1);
    let sound_h = if app.sound.pcm.is_some() || !app.engine.project.sounds.is_empty() { ROW_H * 1.6 } else { 0.0 };
    let visible_rows = (((area.height() - RULER_H - sound_h) / ROW_H).floor() as usize).max(1);
    // keep the current frame and layer in view
    let mut first = app.timeline_scroll.x.max(0.0) as u32;
    if app.frame < first {
        first = app.frame;
    } else if app.frame >= first + visible_cols {
        first = app.frame + 1 - visible_cols;
    }
    first = first.min(len.saturating_sub(visible_cols.min(len)));
    let top_row = rows.iter().position(|&l| l == app.layer).unwrap_or(0); // rows are drawn top layer first
    let mut row0 = app.timeline_scroll.y.max(0.0) as usize;
    if top_row < row0 {
        row0 = top_row;
    } else if top_row >= row0 + visible_rows {
        row0 = top_row + 1 - visible_rows;
    }
    // wheel scrolls frames, Shift+wheel scrolls layers
    let grid_rect = Rect::from_min_max(pos2(area.left(), area.top()), area.max);
    let grid = ui.interact(grid_rect, grid_id(), Sense::click_and_drag());
    if grid.hovered() {
        crate::timeline_extras::wheel_zoom(app, ui);
        let d = ui.input(|i| i.smooth_scroll_delta);
        if ui.input(|i| i.modifiers.shift) {
            row0 = (row0 as f32 - d.y / ROW_H).clamp(0.0, n_layers.saturating_sub(1) as f32) as usize;
        } else if d.y != 0.0 || d.x != 0.0 {
            let df = (-(d.y + d.x) / CELL_W).round() as i64;
            first = (first as i64 + df).clamp(0, len.saturating_sub(1) as i64) as u32;
        }
    }
    app.timeline_scroll = vec2(first as f32, row0 as f32);
    grid.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Grid, true, tl!("Exposure sheet")));
    ui.ctx().accesskit_node_builder(grid.id, |n| {
        n.set_row_count(n_layers);
        n.set_column_count(len as usize);
    });
    if grid.has_focus() {
        ui.memory_mut(|m| m.set_focus_lock_filter(grid.id, egui::EventFilter { tab: false, horizontal_arrows: true, vertical_arrows: true, escape: false }));
        grid_keys(app, ui);
    }
    let painter = ui.painter_at(area);
    painter.rect_filled(area, 0.0, pal.bg);
    // ruler
    let ruler = Rect::from_min_max(pos2(frames_area.left(), area.top()), pos2(area.right(), area.top() + RULER_H));
    painter.rect_filled(ruler, 0.0, pal.panel);
    let fps = app.engine.project.fps().max(1);
    let font = FontId::proportional(10.5);
    for c in 0..visible_cols.min(len - first) {
        let f = first + c;
        let x = frames_area.left() + c as f32 * CELL_W;
        let second = f.is_multiple_of(fps);
        if (f + 1).is_multiple_of(5) || f == 0 || second {
            painter.text(pos2(x + 2.0, ruler.center().y), Align2::LEFT_CENTER, format!("{}", f + 1), font.clone(), if second { pal.text } else { pal.weak });
        }
        painter.line_segment([pos2(x, ruler.bottom() - if second { 8.0 } else { 4.0 }), pos2(x, ruler.bottom())], Stroke::new(1.0, pal.weak));
    }
    // camera keys on the ruler
    for k in &app.engine.project.camera.keys {
        if k.frame >= first && k.frame < first + visible_cols {
            let x = frames_area.left() + (k.frame - first) as f32 * CELL_W + CELL_W * 0.5;
            let y = ruler.bottom() - 4.0;
            painter.add(egui::Shape::convex_polygon(vec![pos2(x, y - 4.0), pos2(x + 4.0, y), pos2(x, y + 4.0), pos2(x - 4.0, y)], pal.accent, Stroke::NONE));
        }
    }
    crate::timeline_extras::paint_flags(app, &painter, ruler, frames_area.left(), first, visible_cols, CELL_W, pal.accent);
    // frames chosen for flipping: a dot on the ruler
    for &f in &app.flip {
        if f >= first && f < first + visible_cols {
            let x = frames_area.left() + (f - first) as f32 * CELL_W + CELL_W * 0.5;
            painter.circle_filled(pos2(x, ruler.top() + 5.0), 2.5, pal.accent);
        }
    }
    // scrub on the ruler (click or drag); the loop handles and onion brackets are added after it, so they
    // sit on top and take the drag
    let ruler_resp = ui.interact(ruler, grid_id().with("ruler"), Sense::click_and_drag());
    ruler_resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Slider, true, tl!("Frame ruler")));
    let ruler_resp = match ruler_resp.hover_pos().map(|p| first as i64 + ((p.x - frames_area.left()) / CELL_W).floor() as i64) {
        Some(f) if f >= 0 => match crate::timeline_extras::note_at(app, f as u32) {
            Some(text) => ruler_resp.on_hover_text(tlf!("Frame {n}: {text}", n = f + 1, text = text)),
            None => ruler_resp,
        },
        _ => ruler_resp,
    };
    if (ruler_resp.dragged() || ruler_resp.clicked())
        && let Some(p) = ruler_resp.interact_pointer_pos()
    {
        let f = first as i64 + ((p.x - frames_area.left()) / CELL_W).floor() as i64;
        app.frame = f.clamp(0, len as i64 - 1) as u32;
        app.playback.stop();
    }
    // right-click (or the Menu key on the focused ruler): loop, onion range and Go To Frame
    let ruler_at = grid_id().with("ruler-menu-at");
    if ruler_resp.secondary_clicked()
        && let Some(p) = ruler_resp.interact_pointer_pos()
    {
        let f = (first as i64 + ((p.x - frames_area.left()) / CELL_W).floor() as i64).clamp(0, i64::from(app.engine.project.length().saturating_sub(1)));
        ui.data_mut(|d| d.insert_temp(ruler_at, f as u32));
    }
    let menu_frame: u32 = ui.data(|d| d.get_temp(ruler_at)).unwrap_or(app.frame);
    crate::context_menus::attach(ui, &ruler_resp, "ruler", |ui| crate::context_menus::ruler(app, ui, menu_frame));
    // the in and out range: a band along the top of the ruler, with a draggable handle at each end
    crate::timeline_ruler::show(app, ui, &painter, ruler, frames_area.left(), first, visible_cols, CELL_W, pal.playhead);
    // the onion skin's reach, under the ruler: before in its tint, after in its tint
    if let Some((lo, hi)) = onion_span(app) {
        let o = &app.engine.project.onion;
        let x = |f: u32| frames_area.left() + (f as f32 - first as f32) * CELL_W;
        let clip = |a: f32, b: f32| (a.max(frames_area.left()), b.min(frames_area.right()));
        let y = ruler.bottom() - 2.0;
        let (bt, at) = (o.before_tint, o.after_tint);
        for (a, b, c) in [(lo, app.frame, bt), (app.frame + 1, hi + 1, at)] {
            let (x0, x1) = clip(x(a), x(b));
            if b > a && x1 > x0 {
                painter.rect_filled(Rect::from_min_max(pos2(x0, y - 2.0), pos2(x1, y + 1.0)), 1.0, Color32::from_rgb(c[0], c[1], c[2]));
            }
        }
        // brackets at each end: drag to show more or fewer steps
        for (before, f, c) in [(true, lo, bt), (false, hi + 1, at)] {
            let bx = x(f);
            if bx < frames_area.left() - 1.0 || bx > frames_area.right() + 1.0 {
                continue;
            }
            let r = Rect::from_center_size(pos2(bx, y - 4.0), vec2(8.0, 12.0));
            let resp = ui.interact(r, grid_id().with(("onion-bracket", before)), Sense::drag());
            let name = if before { tl!("Onion skin reach before") } else { tl!("Onion skin reach after") };
            let steps = if before { app.engine.project.onion.before } else { app.engine.project.onion.after };
            resp.widget_info(|| egui::WidgetInfo::slider(true, steps as f64, name.to_string()));
            let col = Color32::from_rgb(c[0], c[1], c[2]);
            let (top, bot) = (r.top(), r.bottom());
            let dir = if before { 3.0 } else { -3.0 };
            painter.line_segment([pos2(bx, top), pos2(bx, bot)], Stroke::new(2.0, col));
            painter.line_segment([pos2(bx, top), pos2(bx + dir, top)], Stroke::new(2.0, col));
            painter.line_segment([pos2(bx, bot), pos2(bx + dir, bot)], Stroke::new(2.0, col));
            if resp.hovered() || resp.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
            if resp.dragged()
                && let Some(p) = resp.interact_pointer_pos()
            {
                let to = (first as f32 + (p.x - frames_area.left()) / CELL_W).round().max(0.0) as u32;
                set_onion_reach(app, before, to);
            }
        }
    }
    if app.thumbnails {
        refresh_thumbnails(app, ui.ctx(), first, first + visible_cols);
    }
    // rows
    let pins: Vec<Pin> = app.engine.project.light_table.pins.clone();
    let mut clicked: Option<(usize, u32)> = None;
    let mut toggles: Vec<(usize, bool)> = Vec::new();
    let mut locks: Vec<(usize, bool)> = Vec::new();
    let mut onion_toggles: Vec<usize> = Vec::new();
    let mut drag: Option<(usize, u32, u32)> = None;
    // right-click menus, shown after the rows (the rows borrow the project)
    let mut cell_menus: Vec<(usize, u32, egui::Response)> = Vec::new();
    let mut layer_menus: Vec<(usize, egui::Response)> = Vec::new();
    let mut current_cell: Option<(usize, u32, egui::Response)> = None;
    for row in 0..visible_rows.min(n_layers.saturating_sub(row0)) {
        let Some(&li) = rows.get(row0 + row) else { continue };
        let Some(layer) = app.engine.project.layers.get(li) else { continue };
        let y = area.top() + RULER_H + row as f32 * ROW_H;
        let row_rect = Rect::from_min_max(pos2(area.left(), y), pos2(area.right(), y + ROW_H));
        if li == app.layer {
            painter.rect_filled(row_rect, 0.0, pal.raised);
        }
        // the layer's label colour as a bar at the row's left edge
        if let Some(c) = layer.color {
            painter.rect_filled(
                Rect::from_min_max(pos2(area.left(), y + 1.0), pos2(area.left() + 3.0, y + ROW_H - 1.0)),
                1.0,
                Color32::from_rgb(c[0], c[1], c[2]),
            );
        }
        // name column: visibility toggle + name
        let eye = Rect::from_min_size(pos2(area.left() + 6.0, y + 3.0), vec2(16.0, 16.0));
        let eye_resp = ui.interact(eye, grid_id().with(("eye", layer.id)), Sense::click());
        let vis_label = tlf!("Show {layer}", layer = layer.name);
        eye_resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, layer.visible, vis_label.clone()));
        if eye_resp.clicked() {
            toggles.push((li, !layer.visible));
        }
        crate::timeline_rows::paint_toggle(&painter, eye, crate::icons::Icon::Eye, layer.visible, pal.text, pal.weak);
        // lock toggle beside the eye
        let lock = Rect::from_min_size(pos2(eye.right() + 3.0, y + 3.0), vec2(16.0, 16.0));
        let lock_resp = ui.interact(lock, grid_id().with(("lock", layer.id)), Sense::click());
        let lock_label = tlf!("Lock {layer}", layer = layer.name);
        lock_resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, layer.locked, lock_label.clone()));
        if lock_resp.clicked() {
            locks.push((li, !layer.locked));
        }
        crate::timeline_rows::paint_toggle(&painter, lock, crate::icons::Icon::Lock, layer.locked, pal.accent, pal.weak);
        // include in onion skin, beside the lock
        let onion = Rect::from_min_size(pos2(lock.right() + 3.0, y + 3.0), vec2(16.0, 16.0));
        let onion_resp = ui.interact(onion, grid_id().with(("onion", layer.id)), Sense::click());
        let included = crate::timeline_rows::onion_included(layer.onion);
        let onion_label = tlf!("Include {layer} in onion skin", layer = layer.name);
        onion_resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, included, onion_label.clone()));
        let onion_resp = onion_resp.on_hover_text(tl!("Show this layer's onion skin (when it is the current layer, or with onion skin on every layer)"));
        if onion_resp.clicked() {
            onion_toggles.push(li);
        }
        crate::timeline_rows::paint_toggle(&painter, onion, crate::icons::Icon::OnionInclude, included, pal.text, pal.weak);
        let name_rect = Rect::from_min_max(pos2(onion.right() + 4.0, y), pos2(area.left() + NAME_W - 4.0, y + ROW_H));
        let name_resp = ui.interact(name_rect, grid_id().with(("name", layer.id)), Sense::click());
        let name_label = tlf!("Layer {name}", name = layer.name);
        name_resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, li == app.layer, name_label.clone()));
        if name_resp.clicked() {
            clicked = Some((li, app.frame));
        }
        if crate::context_menus::wants_menu(ui, &name_resp, if li == app.layer { "layer" } else { "" }) {
            layer_menus.push((li, name_resp.clone()));
        }
        // folders: a label-coloured stripe per level, and the folder path before the name
        for (depth, f) in app.engine.project.folder_chain(layer.folder.as_deref()).iter().rev().enumerate() {
            let c = f.color.map(|c| Color32::from_rgb(c[0], c[1], c[2])).unwrap_or(pal.weak);
            let x = area.left() + 1.0 + depth as f32 * 3.0;
            painter.line_segment([pos2(x, y + 2.0), pos2(x, y + ROW_H - 2.0)], Stroke::new(2.0, c));
        }
        let mut name = crate::layer_tree::path_name(&app.engine.project, layer);
        if layer.clip {
            name = format!("{name} · {}", tl!("clipped"));
        }
        if app.engine.project.is_locked(layer) {
            name.push_str(&format!(" · {}", tl!("locked")));
        }
        painter.text(name_rect.left_center(), Align2::LEFT_CENTER, name, FontId::proportional(12.5), if li == app.layer { pal.text } else { pal.weak });
        // the frames where the layer's pegs are keyed, marked on the cells and named for readers
        let peg_keys: Vec<u32> = layer.peg.map(|id| crate::timeline_rows::key_frames(app, id, first, (first + visible_cols).min(len))).unwrap_or_default();
        // cells
        let mut f = first;
        while f < (first + visible_cols).min(len) {
            let x = frames_area.left() + (f - first) as f32 * CELL_W;
            let r = Rect::from_min_size(pos2(x, y + 2.0), vec2(CELL_W, ROW_H - 4.0));
            let cell = layer.cells.get(f as usize).copied().unwrap_or_default();
            let fade = if layer.visible { 1.0 } else { 0.45 };
            match cell {
                Cell::Key(d) => {
                    let kc = layer.color.map(|c| Color32::from_rgb(c[0], c[1], c[2])).unwrap_or(pal.cell_key);
                    painter.rect_filled(r.shrink2(vec2(1.0, 0.0)), 2.0, kc.gamma_multiply(fade));
                    if let Some((_, t)) = app.thumbs.get(&d).filter(|_| app.thumbnails) {
                        let inner = r.shrink(3.0);
                        painter.rect_filled(inner, 1.0, Color32::from_rgb(246, 243, 236));
                        painter.image(t.id(), inner, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    } else {
                        painter.circle_filled(pos2(r.left() + 6.0, r.center().y), 2.6, pal.bg);
                    }
                    if let Some(m) = app.engine.project.marks.get(&d) {
                        crate::timeline_rows::paint_mark(&painter, r, *m, pal);
                    }
                }
                Cell::Hold => {
                    let hc = layer.color.map(|c| Color32::from_rgb(c[0], c[1], c[2]).gamma_multiply(0.55)).unwrap_or(pal.cell_hold);
                    painter.rect_filled(
                        Rect::from_min_max(pos2(r.left() - 1.0, r.center().y - 3.0), pos2(r.right() - 1.0, r.center().y + 3.0)),
                        0.0,
                        hc.gamma_multiply(fade),
                    );
                }
                Cell::Blank => {
                    painter.rect_stroke(r.shrink(2.0), 1.0, Stroke::new(1.0, pal.grid), egui::StrokeKind::Inside);
                }
            }
            if pins.iter().any(|p| p.layer == layer.id && p.frame == f) {
                painter.rect_filled(Rect::from_center_size(r.center_bottom() - vec2(0.0, 3.0), vec2(5.0, 5.0)), 0.0, pal.playhead);
            }
            let resp = ui.interact(r, grid_id().with((layer.id, f)), Sense::click_and_drag());
            if resp.drag_started()
                && let Some((start, end)) = layer.exposure_range(f)
            {
                drag = Some((li, start, end - start));
            }
            let selected = li == app.layer && f == app.frame;
            let mut label = tlf!("{layer}, frame {frame}: {cell}", layer = layer.name, frame = f + 1, cell = cell_text(cell));
            if let Some(m) = cell.key().and_then(|d| app.engine.project.marks.get(&d)) {
                label = format!("{label}, {}", crate::timing_panel::mark_label(Some(*m)).to_lowercase());
            }
            if peg_keys.binary_search(&f).is_ok() {
                label = format!("{label}, {}", tl!("peg key"));
                crate::timeline_rows::paint_key(&painter, pos2(r.center().x, r.top() + 1.0), pal.accent, pal.bg);
            }
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Cell, true, selected, label.clone()));
            if cell == Cell::Hold
                && resp.hovered()
                && let Some(d) = layer.drawing_at(f)
            {
                painter.text(r.center(), Align2::CENTER_CENTER, format!("{}", d.0), FontId::proportional(9.0), pal.weak);
            }
            let resp = cell_tooltip(app, resp, cell, layer.drawing_at(f));
            if resp.clicked() {
                clicked = Some((li, f));
            }
            if resp.secondary_clicked() || resp.context_menu_opened() {
                cell_menus.push((li, f, resp.clone()));
            } else if selected {
                current_cell = Some((li, f, resp.clone()));
            }
            f += 1;
        }
        painter.line_segment([row_rect.left_bottom(), row_rect.right_bottom()], Stroke::new(1.0, pal.grid));
    }
    // sound
    if sound_h > 0.0 {
        let y = area.bottom() - sound_h;
        let row = Rect::from_min_max(pos2(area.left(), y), pos2(area.right(), area.bottom()));
        painter.rect_filled(row, 0.0, pal.panel);
        let sounds = &app.engine.project.sounds;
        let name = match sounds.len() {
            0 | 1 => sounds.first().map(|s| s.name.clone()).unwrap_or_default(),
            n => tlf!("{name} and {n} more", name = sounds.first().map(|s| s.name.clone()).unwrap_or_default(), n = n - 1),
        };
        // the waveform is the mix (from frame 0); dragging moves the first clip
        let offset = app.sound_offset();
        let first_start = sounds.first().map(|s| s.offset).unwrap_or(0);
        painter.text(pos2(area.left() + 6.0, row.center().y), Align2::LEFT_CENTER, &name, FontId::proportional(11.5), pal.weak);
        let wave = Rect::from_min_max(pos2(frames_area.left(), y + 2.0), pos2(area.right(), area.bottom() - 2.0));
        let resp = ui.interact(wave, grid_id().with("sound"), Sense::click_and_drag());
        let label = tlf!("Sound {name}, starts at frame {frame}", name = name, frame = first_start + 1);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Slider, true, label.clone()));
        if let (Some(pcm), Some(peaks)) = (&app.sound.pcm, &app.sound.peaks) {
            let fps = app.engine.project.fps().max(1) as f64;
            let per_frame = pcm.rate as f64 / fps;
            let cols = wave.width().max(1.0) as usize;
            let start = (first as f64 - offset as f64) * per_frame;
            let step = per_frame / CELL_W as f64;
            let spans = peaks.spans(&app.sound.mono, start, step, 0.0, cols);
            let mid = wave.center().y;
            let half = wave.height() * 0.5;
            for (i, sp) in spans.iter().enumerate() {
                if let Some((lo, hi)) = sp {
                    let x = wave.left() + i as f32 + 0.5;
                    painter.line_segment(
                        [pos2(x, mid - hi.clamp(-1.0, 1.0) * half), pos2(x, mid - lo.clamp(-1.0, 1.0) * half)],
                        Stroke::new(1.0, pal.playhead.gamma_multiply(0.8)),
                    );
                }
            }
        } else if !app.sound.error.is_empty() {
            painter.text(wave.left_center(), Align2::LEFT_CENTER, &app.sound.error, FontId::proportional(11.0), pal.accent);
        }
        // drag the waveform sideways to move the sound (keyboard: Frame ▸ Start Sound Here)
        if resp.dragged() {
            let d = ui.data(|m| m.get_temp::<f32>(grid_id().with("sound-drag")).unwrap_or(0.0)) + resp.drag_delta().x;
            ui.data_mut(|m| m.insert_temp(grid_id().with("sound-drag"), d));
        }
        if resp.drag_stopped() {
            let d = ui.data(|m| m.get_temp::<f32>(grid_id().with("sound-drag")).unwrap_or(0.0));
            ui.data_mut(|m| m.remove::<f32>(grid_id().with("sound-drag")));
            let frames = (d / CELL_W).round() as i32;
            if frames != 0 {
                app.run(Command::MoveSound { offset: first_start + frames, index: 0 });
            }
        }
    }
    // selection
    if app.has_range() {
        let (l0, l1, f0, f1) = app.selection();
        let fx0 = f0.max(first);
        let fx1 = f1.min(first + visible_cols.saturating_sub(1));
        let in_range: Vec<usize> = rows.iter().enumerate().filter(|(_, l)| (l0..=l1).contains(*l)).map(|(i, _)| i).collect();
        let (r0, r1) = (in_range.first().copied().unwrap_or(0), in_range.last().copied().unwrap_or(0));
        if !in_range.is_empty() && fx0 <= fx1 && r1 >= row0 && r0 < row0 + visible_rows {
            let top = area.top() + RULER_H + (r0.max(row0) - row0) as f32 * ROW_H;
            let bottom = area.top() + RULER_H + ((r1 + 1).min(row0 + visible_rows) - row0) as f32 * ROW_H;
            let rect = Rect::from_min_max(
                pos2(frames_area.left() + (fx0 - first) as f32 * CELL_W, top),
                pos2(frames_area.left() + (fx1 + 1 - first) as f32 * CELL_W, bottom),
            );
            painter.rect_filled(rect, 0.0, pal.playhead.gamma_multiply(0.18));
            painter.rect_stroke(rect, 0.0, Stroke::new(1.0, pal.playhead), egui::StrokeKind::Inside);
        }
    }
    // playhead
    if app.frame >= first && app.frame < first + visible_cols {
        let x = frames_area.left() + (app.frame - first) as f32 * CELL_W;
        let col = Rect::from_min_max(pos2(x, area.top()), pos2(x + CELL_W, area.bottom()));
        painter.rect_stroke(col, 0.0, Stroke::new(1.5, pal.playhead), egui::StrokeKind::Inside);
        painter.rect_filled(Rect::from_min_max(col.min, pos2(col.right(), area.top() + RULER_H)), 0.0, pal.playhead.gamma_multiply(0.35));
    }
    painter.line_segment([pos2(frames_area.left(), area.top()), pos2(frames_area.left(), area.bottom())], Stroke::new(1.0, pal.grid));
    if grid.has_focus() {
        painter.rect_stroke(area.shrink(1.0), 0.0, Stroke::new(2.0, pal.playhead), egui::StrokeKind::Inside);
    }
    // Dragging a drawing's cells sideways changes how long it is held (+ / − do it from the keyboard).
    let key = egui::Id::unique("tooncraft-hold-drag");
    if let Some(d) = drag {
        ui.data_mut(|m| m.insert_temp(key, d));
    }
    if let Some((li, start, len0)) = ui.data(|m| m.get_temp::<(usize, u32, u32)>(key)) {
        let (down, pos) = ui.input(|i| (i.pointer.primary_down(), i.pointer.interact_pos()));
        let target = pos.map(|p| ((p.x - frames_area.left()) / CELL_W).floor() as i64 + first as i64).unwrap_or(0);
        let len = (target - start as i64 + 1).max(1) as u32;
        if down {
            let x0 = frames_area.left() + (start as f32 - first as f32) * CELL_W;
            let row = rows.iter().position(|&l| l == li).unwrap_or(usize::MAX);
            if row >= row0 && row < row0 + visible_rows {
                let y = area.top() + RULER_H + (row - row0) as f32 * ROW_H;
                let r = Rect::from_min_size(pos2(x0, y + 1.0), vec2(len as f32 * CELL_W, ROW_H - 2.0));
                painter.rect_stroke(r, 2.0, Stroke::new(2.0, pal.accent), egui::StrokeKind::Inside);
            }
        } else {
            ui.data_mut(|m| m.remove::<(usize, u32, u32)>(key));
            if len != len0 {
                app.run(Command::SetExposure { layer: li, frame: start, length: len });
                app.set_status(tlf!("Held for {n} frames", n = len));
            }
        }
    }
    for (li, v) in toggles {
        app.run(Command::SetLayer { layer: li, visible: Some(v), locked: None, opacity: None });
    }
    for (li, v) in locks {
        app.run(Command::SetLayer { layer: li, visible: None, locked: Some(v), opacity: None });
    }
    for li in onion_toggles {
        if let Some(l) = app.engine.project.layers.get(li) {
            let onion = crate::timeline_rows::onion_include_toggled(l.onion, app.engine.project.onion);
            app.run(Command::SetLayerOnion { layer: li, onion });
        }
    }
    if let Some((li, f)) = clicked {
        if ui.input(|i| i.modifiers.shift) {
            if app.sel_anchor.is_none() {
                app.sel_anchor = Some((app.layer, app.frame));
            }
        } else {
            app.sel_anchor = None;
        }
        app.layer = li;
        app.frame = f;
        app.playback.stop();
        ui.memory_mut(|m| m.request_focus(grid_id()));
    }
    // Shift+F10 or the Menu key: the menu of the current cell, from the keyboard
    if let Some((li, f, resp)) = current_cell.take() {
        let now = crate::context_menus::launch_now(ui.ctx(), "cell") || menu_key(ui) && ui.memory(|m| m.has_focus(grid_id()));
        keyboard_menu(ui, &resp, now, |ui| crate::context_menus::cell(app, ui, li, f));
    }
    for (li, f, resp) in cell_menus {
        if resp.secondary_clicked() {
            // the menu acts on the cell it was opened on, which becomes the current one
            app.layer = li;
            app.frame = f;
            app.sel_anchor = None;
            app.playback.stop();
        }
        resp.context_menu(|ui| crate::context_menus::cell(app, ui, li, f));
    }
    for (li, resp) in layer_menus {
        let launch = if li == app.layer { "layer" } else { "" };
        crate::context_menus::attach(ui, &resp, launch, |ui| crate::context_menus::layer(app, ui, li));
    }
    ui.allocate_rect(area, Sense::hover());
}

/// Shift+F10 or the Menu key: open the focused thing's right-click menu.
pub fn menu_key(ui: &egui::Ui) -> bool {
    ui.input(|i| i.key_pressed(Key::Menu) || (i.modifiers.shift_only() && i.key_pressed(Key::F10)))
}

/// A widget's right-click menu opened from the keyboard (`open_now`), shown under the widget
/// (there is no pointer position to put it at) until an item is chosen or Escape.
pub fn keyboard_menu(ui: &egui::Ui, resp: &egui::Response, open_now: bool, add: impl FnOnce(&mut egui::Ui)) {
    let id = resp.id.with("keyboard-menu");
    if !open_now && !egui::Popup::is_id_open(ui.ctx(), id) {
        return;
    }
    egui::Popup::from_response(resp)
        .id(id)
        .kind(egui::PopupKind::Menu)
        .layout(egui::Layout::top_down_justified(egui::Align::Min))
        .open_memory(open_now.then_some(egui::SetOpenCommand::Bool(true)))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(add);
}

/// A hover tip on a cell: the drawing's thumbnail (when made), number and name.
pub fn cell_tooltip(app: &ToonApp, resp: egui::Response, cell: Cell, drawing: Option<tooncraft_doc::DrawingId>) -> egui::Response {
    let Some(d) = drawing else { return resp };
    let name = app.engine.project.drawing_names.get(&d).cloned();
    let thumb = app.thumbs.get(&d).map(|(_, t)| t.clone());
    let held = cell == Cell::Hold;
    resp.on_hover_ui_at_pointer(|ui| {
        if let Some(t) = &thumb {
            ui.add(egui::Image::new(t).fit_to_exact_size(vec2(120.0, 80.0)));
        }
        let mut text = tlf!("Drawing {id}", id = d.0);
        if let Some(n) = &name {
            text = format!("{text}, {n}");
        }
        if held {
            text = tlf!("{drawing} (held)", drawing = text);
        }
        ui.label(text);
    })
}

/// Set how many steps the onion skin shows on one side so it reaches frame `to`.
pub fn set_onion_reach(app: &mut ToonApp, before: bool, to: u32) {
    let frame = app.frame;
    let Some(l) = app.engine.project.layers.get(app.layer) else { return };
    let o = &app.engine.project.onion;
    let max = tooncraft_doc::ONION_MAX_STEPS as u32;
    let n = match o.mode {
        tooncraft_doc::OnionMode::Frames => {
            if before {
                frame.saturating_sub(to)
            } else {
                to.saturating_sub(frame + 1)
            }
        }
        tooncraft_doc::OnionMode::Drawings => {
            let keys = |a: u32, b: u32| (a..b).filter(|f| matches!(l.cells.get(*f as usize), Some(Cell::Key(_)))).count() as u32;
            if before {
                let start = l.exposure_start(frame).unwrap_or(frame);
                keys(to.min(start), start)
            } else {
                keys(frame + 1, to.max(frame + 1))
            }
        }
    }
    .min(max) as u8;
    let o = &mut app.engine.project.onion;
    if before {
        o.before = n;
    } else {
        o.after = n;
    }
}

/// The frames the onion skin reaches on the current layer, `(first, last)`, when it is on.
pub fn onion_span(app: &ToonApp) -> Option<(u32, u32)> {
    let p = &app.engine.project;
    let o = &p.onion;
    if !o.enabled {
        return None;
    }
    let len = p.length();
    let frame = app.frame;
    let l = p.layers.get(app.layer)?;
    let span = match o.mode {
        tooncraft_doc::OnionMode::Frames => (frame.saturating_sub(o.before as u32), (frame + o.after as u32).min(len.saturating_sub(1))),
        tooncraft_doc::OnionMode::Drawings => {
            let is_key = |f: u32| matches!(l.cells.get(f as usize), Some(Cell::Key(_)));
            let here = l.exposure_start(frame).unwrap_or(frame);
            let mut lo = here;
            for _ in 0..o.before {
                match (0..lo).rev().find(|f| is_key(*f)) {
                    Some(f) => lo = f,
                    None => break,
                }
            }
            let mut hi = frame;
            let mut cursor = frame;
            for _ in 0..o.after {
                match (cursor + 1..len).find(|f| is_key(*f)) {
                    Some(f) => {
                        cursor = f;
                        hi = l.exposure_range(f).map_or(f, |(_, e)| e.saturating_sub(1));
                    }
                    None => break,
                }
            }
            (lo, hi)
        }
    };
    Some(span)
}

/// Thumbnails of the drawings keyed in the visible frames, a few new ones per UI frame, each
/// cached until its drawing changes.
fn refresh_thumbnails(app: &mut ToonApp, ctx: &egui::Context, from: u32, to: u32) {
    let mut ids = Vec::new();
    for l in &app.engine.project.layers {
        for f in from..to.min(app.engine.project.length()) {
            if let Some(Cell::Key(d)) = l.cells.get(f as usize) {
                ids.push(*d);
            }
        }
    }
    refresh_drawings(app, ctx, ids);
}

/// Make or refresh the thumbnails of `ids` (cached by the drawing's revision), a few a frame.
pub fn refresh_drawings(app: &mut ToonApp, ctx: &egui::Context, ids: Vec<tooncraft_doc::DrawingId>) {
    let mut budget = 6;
    for d in ids {
        let Some(s) = app.engine.project.drawing(d) else { continue };
        let rev = s.tiles().map(|(_, t)| t.revision()).max().unwrap_or(0) ^ (s.tile_count() as u64).rotate_left(40);
        if app.thumbs.get(&d).is_some_and(|(r, _)| *r == rev) || budget == 0 {
            continue;
        }
        budget -= 1;
        let img = s.to_image().downscaled(80);
        let tex = ctx.load_texture(
            format!("thumb-{}", d.0),
            egui::ColorImage::from_rgba_premultiplied([img.width as usize, img.height as usize], &img.data),
            egui::TextureOptions::LINEAR,
        );
        app.thumbs.insert(d, (rev, tex));
    }
    if budget == 0 {
        ctx.request_repaint();
    }
}

/// The Frame menu (menu bar).
pub fn frame_menu(app: &mut ToonApp, ui: &mut egui::Ui) {
    let (layer, frame) = (app.layer, app.frame);
    let play = if app.playback.playing { tl!("Stop") } else { tl!("Play") };
    if ui.button(play).clicked() {
        app.toggle_play();
    }
    if ui.button(tl!("Previous Frame")).clicked() {
        app.go_to(frame as i64 - 1);
    }
    if ui.button(tl!("Next Frame")).clicked() {
        app.go_to(frame as i64 + 1);
    }
    if ui.button(tl!("Previous Drawing")).clicked() {
        app.step_drawing(false);
    }
    if ui.button(tl!("Next Drawing")).clicked() {
        app.step_drawing(true);
    }
    if ui.button(tl!("Add to or Remove from Flip Set")).clicked() {
        app.toggle_flip();
    }
    if ui.add_enabled(!app.flip.is_empty(), egui::Button::new(tl!("Flip to Next Chosen Frame"))).clicked() {
        app.flip_step(true);
    }
    if ui.add_enabled(!app.flip.is_empty(), egui::Button::new(tl!("Clear Flip Set"))).clicked() {
        app.flip.clear();
    }
    if ui.button(tl!("Previous Key Drawing")).clicked() {
        app.step_key(false);
    }
    if ui.button(tl!("Next Key Drawing")).clicked() {
        app.step_key(true);
    }
    ui.separator();
    let items: [(String, Command); 7] = [
        (tl!("New Drawing").to_string(), Command::NewDrawing { layer, frame }),
        (tl!("Duplicate Drawing").to_string(), Command::DuplicateDrawing { layer, frame }),
        (tl!("Insert Frame").to_string(), Command::InsertFrame { layer: Some(layer), frame }),
        (tl!("Delete Frame").to_string(), Command::DeleteFrame { layer: Some(layer), frame }),
        (tl!("Hold One Frame Longer").to_string(), Command::ExtendHold { layer, frame }),
        (tl!("Hold One Frame Shorter").to_string(), Command::ShortenHold { layer, frame }),
        (tl!("Clear Frame").to_string(), Command::ClearCell { layer, frame }),
    ];
    for (label, cmd) in items {
        if ui.button(label).clicked() {
            app.run(cmd);
        }
    }
    ui.menu_button(tl!("Swap Drawing"), |ui| crate::context_menus::swap_grid(app, ui, layer, frame));
    if ui.button(tl!("Fill Empty Frames")).clicked() {
        app.run(Command::FillEmpty { layer });
    }
    ui.separator();
    if ui.button(tl!("Copy Frames")).clicked() {
        app.copy_cells();
    }
    if ui.button(tl!("Cut Frames")).clicked() {
        app.cut_cells();
    }
    if ui.add_enabled(app.cell_clipboard.is_some(), egui::Button::new(tl!("Paste Frames"))).clicked() {
        app.paste_cells(false, false);
    }
    if ui.add_enabled(app.cell_clipboard.is_some(), egui::Button::new(tl!("Paste Frames, Moving Later Frames"))).clicked() {
        app.paste_cells(true, false);
    }
    if ui.add_enabled(app.cell_clipboard.is_some(), egui::Button::new(tl!("Paste as New Drawings"))).clicked() {
        app.paste_cells(false, true);
    }
    let (l0, _, f0, f1) = app.selection();
    if ui.add_enabled(f1 > f0, egui::Button::new(tl!("Reverse Selected Frames"))).clicked() {
        app.run(Command::Reverse { layer: l0, from: f0, to: f1 });
    }
    if ui.add_enabled(f1 > f0, egui::Button::new(tl!("Repeat Selected Frames"))).clicked() {
        app.run(Command::Cycle { layer: l0, from: f0, to: f1, times: 1 });
    }
    ui.separator();
    ui.menu_button(tl!("Re-time Layer"), |ui| {
        let to = app.engine.project.length().saturating_sub(1);
        for step in 1..=4u32 {
            if ui.button(tlf!("Each drawing on {n}s", n = step)).clicked() && app.run(Command::Retime { layer, from: 0, to, step }) {
                app.set_status(tlf!("Re-timed on {n}s; no drawing was deleted", n = step));
            }
        }
    });
    if ui.button(tl!("Insert Frame in All Layers")).clicked() {
        app.run(Command::InsertFrame { layer: None, frame });
    }
    if ui.button(tl!("Delete Frame in All Layers")).clicked() {
        app.run(Command::DeleteFrame { layer: None, frame });
    }
    if ui.add_enabled(!app.engine.project.sounds.is_empty(), egui::Button::new(tl!("Start Sound Here"))).clicked() {
        app.run(Command::MoveSound { offset: frame as i32, index: 0 });
    }
    crate::timeline_extras::menu(app, ui);
    if ui.button(tl!("Focus Exposure Sheet")).clicked() {
        ui.memory_mut(|m| m.request_focus(grid_id()));
    }
}
