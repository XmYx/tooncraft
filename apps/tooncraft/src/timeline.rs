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

const NAME_W: f32 = 132.0;
const CELL_W0: f32 = 17.0;
const ROW_H0: f32 = 22.0;
const RULER_H: f32 = 20.0;

/// A small button with a painted icon and an accessible name.
pub fn icon_button(ui: &mut egui::Ui, label: &str, selected: bool, paint: impl FnOnce(&egui::Painter, Rect, Color32)) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 22.0), Sense::click());
    let v = ui.style().interact_selectable(&resp, selected);
    ui.painter().rect(rect, 3.0, if selected || resp.hovered() { v.bg_fill } else { Color32::TRANSPARENT }, v.bg_stroke, egui::StrokeKind::Inside);
    paint(ui.painter(), rect.shrink(5.0), v.fg_stroke.color);
    if resp.has_focus() {
        ui.painter().rect_stroke(rect, 3.0, ui.visuals().selection.stroke, egui::StrokeKind::Outside);
    }
    let l = label.to_string();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, l.clone()));
    resp.on_hover_text(label)
}

fn tri(p: &egui::Painter, r: Rect, right: bool, c: Color32) {
    let pts = if right { vec![r.left_top(), r.right_center(), r.left_bottom()] } else { vec![r.right_top(), r.left_center(), r.right_bottom()] };
    p.add(egui::Shape::convex_polygon(pts, c, Stroke::NONE));
}

fn bar(p: &egui::Painter, r: Rect, x: f32, c: Color32) {
    p.rect_filled(Rect::from_min_max(pos2(x - 1.2, r.top()), pos2(x + 1.2, r.bottom())), 0.0, c);
}

fn transport(app: &mut ToonApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let len = app.engine.project.length();
        if icon_button(ui, tl!("First frame"), false, |p, r, c| {
            bar(p, r, r.left(), c);
            tri(p, Rect::from_min_max(pos2(r.left() + 3.0, r.top()), r.max), false, c);
        })
        .clicked()
        {
            app.frame = 0;
        }
        if icon_button(ui, tl!("Previous frame"), false, |p, r, c| tri(p, r.shrink2(vec2(2.0, 0.0)), false, c)).clicked() {
            app.go_to(app.frame as i64 - 1);
        }
        let playing = app.playback.playing;
        let label = if playing { tl!("Stop") } else { tl!("Play") };
        if icon_button(ui, label, playing, |p, r, c| {
            if playing {
                p.rect_filled(r.shrink(1.5), 1.0, c);
            } else {
                tri(p, r.shrink2(vec2(1.5, 0.0)), true, c);
            }
        })
        .clicked()
        {
            app.toggle_play();
        }
        if icon_button(ui, tl!("Next frame"), false, |p, r, c| tri(p, r.shrink2(vec2(2.0, 0.0)), true, c)).clicked() {
            app.go_to(app.frame as i64 + 1);
        }
        if icon_button(ui, tl!("Last frame"), false, |p, r, c| {
            tri(p, Rect::from_min_max(r.min, pos2(r.right() - 3.0, r.bottom())), true, c);
            bar(p, r, r.right(), c);
        })
        .clicked()
        {
            app.frame = len.saturating_sub(1);
        }
        let looping = app.playback.looping;
        if icon_button(ui, tl!("Loop playback"), looping, |p, r, c| {
            p.circle_stroke(r.center(), r.height() * 0.42, Stroke::new(1.6, c));
            tri(p, Rect::from_center_size(r.center_top() + vec2(2.0, 1.0), vec2(5.0, 5.0)), true, c);
        })
        .clicked()
        {
            app.playback.looping = !app.playback.looping;
        }
        ui.separator();
        ui.label(egui::RichText::new(crate::timeline_extras::position_text(app)).monospace());
        crate::timeline_extras::transport(app, ui);
        ui.separator();
        let mut fps = app.engine.project.fps();
        let l = ui.label(tl!("fps"));
        if ui.add(egui::DragValue::new(&mut fps).range(tooncraft_doc::MIN_FPS..=tooncraft_doc::MAX_FPS)).labelled_by(l.id).changed() {
            app.run(Command::SetFps { fps });
        }
        let mut frames = len;
        let l = ui.label(tl!("Frames"));
        let r = ui.add(egui::DragValue::new(&mut frames).range(1..=tooncraft_doc::MAX_FRAMES)).labelled_by(l.id);
        if r.changed() && frames != len {
            app.run(Command::SetLength { frames });
            app.clamp_selection();
        }
        ui.separator();
        let (layer, frame) = (app.layer, app.frame);
        if ui.button(tl!("New")).on_hover_text(tl!("New drawing (Shift+N)")).clicked() {
            app.run(Command::NewDrawing { layer, frame });
        }
        if ui.button(tl!("Duplicate")).on_hover_text(tl!("Duplicate drawing (Shift+D)")).clicked() {
            app.run(Command::DuplicateDrawing { layer, frame });
        }
        if ui.button(tl!("Insert")).on_hover_text(tl!("Insert frame (Insert)")).clicked() {
            app.run(Command::InsertFrame { layer: Some(layer), frame });
        }
        if ui.button(tl!("Delete")).on_hover_text(tl!("Delete frame (Shift+Delete)")).clicked() {
            app.run(Command::DeleteFrame { layer: Some(layer), frame });
        }
        if ui.button(tl!("Hold +")).on_hover_text(tl!("Show one frame longer (+)")).clicked() {
            app.run(Command::ExtendHold { layer, frame });
        }
        if ui.button(tl!("Hold −")).on_hover_text(tl!("Show one frame shorter (−)")).clicked() {
            app.run(Command::ShortenHold { layer, frame });
        }
        if ui.button(tl!("Clear")).on_hover_text(tl!("Clear frame (Delete)")).clicked() {
            app.run(Command::ClearCell { layer, frame });
        }
        ui.separator();
        let onion = app.engine.project.onion.enabled;
        if ui.selectable_label(onion, tl!("Onion")).on_hover_text(tl!("Onion skin (O)")).clicked() {
            app.engine.project.onion.enabled = !onion;
        }
        if !app.engine.project.sounds.is_empty() {
            let on = !app.sound.muted;
            if ui.selectable_label(on, tl!("Sound")).on_hover_text(tl!("Play the soundtrack")).clicked() {
                app.sound.muted = on;
            }
        }
        let lt = app.engine.project.light_table.enabled;
        if ui.selectable_label(lt, tl!("Light table")).clicked() {
            app.engine.project.light_table.enabled = !lt;
        }
        if ui.selectable_label(app.thumbnails, tl!("Thumbnails")).on_hover_text(tl!("Show drawing thumbnails in the cells")).clicked() {
            app.thumbnails = !app.thumbnails;
        }
        if ui.button(tl!("Pin")).on_hover_text(tl!("Pin this frame to the light table (Shift+L)")).clicked() {
            app.run(Command::TogglePin { layer, frame });
        }
    });
}

fn cell_text(cell: Cell) -> String {
    match cell {
        Cell::Key(d) => tlf!("drawing {id}", id = d.0),
        Cell::Hold => tl!("hold").to_string(),
        Cell::Blank => tl!("blank").to_string(),
    }
}

fn grid_keys(app: &mut ToonApp, ui: &egui::Ui) {
    let (layer, frame) = (app.layer, app.frame);
    let (k, shift) = ui.input(|i| {
        let arrow = |k| (i.modifiers.is_none() || i.modifiers.shift_only()) && i.key_pressed(k);
        ([arrow(Key::ArrowLeft), arrow(Key::ArrowRight), arrow(Key::ArrowUp), arrow(Key::ArrowDown)], i.modifiers.shift)
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
    transport(app, ui);
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
    // scrub on the ruler (click or drag)
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
    if app.thumbnails {
        refresh_thumbnails(app, ui.ctx(), first, first + visible_cols);
    }
    // rows
    let pins: Vec<Pin> = app.engine.project.light_table.pins.clone();
    let mut clicked: Option<(usize, u32)> = None;
    let mut toggles: Vec<(usize, bool)> = Vec::new();
    let mut drag: Option<(usize, u32, u32)> = None;
    for row in 0..visible_rows.min(n_layers.saturating_sub(row0)) {
        let Some(&li) = rows.get(row0 + row) else { continue };
        let Some(layer) = app.engine.project.layers.get(li) else { continue };
        let y = area.top() + RULER_H + row as f32 * ROW_H;
        let row_rect = Rect::from_min_max(pos2(area.left(), y), pos2(area.right(), y + ROW_H));
        if li == app.layer {
            painter.rect_filled(row_rect, 0.0, pal.raised);
        }
        // name column: visibility toggle + name
        let eye = Rect::from_min_size(pos2(area.left() + 4.0, y + 3.0), vec2(16.0, 16.0));
        let eye_resp = ui.interact(eye, grid_id().with(("eye", layer.id)), Sense::click());
        let vis_label = tlf!("Show {layer}", layer = layer.name);
        eye_resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, layer.visible, vis_label.clone()));
        if eye_resp.clicked() {
            toggles.push((li, !layer.visible));
        }
        let ec = if layer.visible { pal.text } else { pal.weak };
        painter.circle_stroke(eye.center(), 5.5, Stroke::new(1.3, ec));
        if layer.visible {
            painter.circle_filled(eye.center(), 2.4, ec);
        } else {
            painter.line_segment([eye.left_bottom(), eye.right_top()], Stroke::new(1.3, ec));
        }
        let name_rect = Rect::from_min_max(pos2(eye.right() + 4.0, y), pos2(area.left() + NAME_W - 4.0, y + ROW_H));
        let name_resp = ui.interact(name_rect, grid_id().with(("name", layer.id)), Sense::click());
        let name_label = tlf!("Layer {name}", name = layer.name);
        name_resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, li == app.layer, name_label.clone()));
        if name_resp.clicked() {
            clicked = Some((li, app.frame));
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
                        painter.text(r.right_top() + vec2(-2.0, 1.0), Align2::RIGHT_TOP, m.letter(), FontId::proportional(9.5), pal.bg);
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
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Cell, true, selected, label.clone()));
            if resp.clicked() {
                clicked = Some((li, f));
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
    ui.allocate_rect(area, Sense::hover());
}

/// Thumbnails of the drawings keyed in the visible frames, a few new ones per UI frame, each
/// cached until its drawing changes.
fn refresh_thumbnails(app: &mut ToonApp, ctx: &egui::Context, from: u32, to: u32) {
    let mut budget = 6;
    let mut ids = Vec::new();
    for l in &app.engine.project.layers {
        for f in from..to.min(app.engine.project.length()) {
            if let Some(Cell::Key(d)) = l.cells.get(f as usize) {
                ids.push(*d);
            }
        }
    }
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
