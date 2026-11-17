//! The canvas: GPU-composited tiles under a pan / zoom / rotate / mirror view, pen input, onion
//! skin and light table.
//!
//! Every tile is drawn as a textured quad (see `textures.rs`), so rotation and zoom cost the GPU,
//! not the CPU. Pen samples are read from the frame's raw events, not from the pointer position,
//! so a fast stroke keeps every sample the window system delivered. Pressure comes from touch
//! force where the platform reports it (pens on Windows Ink, iPadOS, Android); a mouse uses the
//! "mouse pressure" setting.

use std::time::Instant;

use egui::{Color32, Event, Key, Mesh, PointerButton, Pos2, Rect, Sense, Shape, Stroke, TouchPhase, epaint::Vertex, pos2, vec2};
use tooncraft_brush::Sample;
use tooncraft_doc::DrawingId;
use tooncraft_engine::Command;
use tooncraft_raster::{Surface, TILE};

use crate::app::{KeyboardPen, Tool, ToonApp};
use crate::view::View;

pub fn canvas_id() -> egui::Id {
    egui::Id::unique("tooncraft-canvas")
}

struct Ctx<'a> {
    painter: &'a egui::Painter,
    view: View,
    rect: Rect,
    size: egui::Vec2,
}

impl Ctx<'_> {
    fn screen(&self, x: f32, y: f32) -> Pos2 {
        self.view.to_screen(self.rect, self.size, pos2(x, y))
    }
}

fn draw_surface(app_tex: &mut crate::textures::TileTextures, ctx: &egui::Context, c: &Ctx, id: DrawingId, s: &Surface, opacity: f32, tint: Option<[u8; 3]>) {
    let o = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
    if o == 0 {
        return;
    }
    let color = Color32::from_rgba_premultiplied(o, o, o, o);
    let (w, h) = (s.width(), s.height());
    for (index, tile) in s.tiles() {
        let (tx, ty) = s.tile_coords(index);
        let (x0, y0) = (tx * TILE, ty * TILE);
        let (x1, y1) = ((x0 + TILE).min(w), (y0 + TILE).min(h));
        let corners = [c.screen(x0 as f32, y0 as f32), c.screen(x1 as f32, y0 as f32), c.screen(x1 as f32, y1 as f32), c.screen(x0 as f32, y1 as f32)];
        let bb = Rect::from_points(&corners);
        if !bb.intersects(c.rect) {
            continue;
        }
        let tex = app_tex.get(ctx, id, index, tile, tint);
        let (u, v) = ((x1 - x0) as f32 / TILE as f32, (y1 - y0) as f32 / TILE as f32);
        let uvs = [pos2(0.0, 0.0), pos2(u, 0.0), pos2(u, v), pos2(0.0, v)];
        let mut mesh = Mesh::with_texture(tex);
        for (p, uv) in corners.iter().zip(uvs) {
            mesh.vertices.push(Vertex { pos: *p, uv, color });
        }
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        c.painter.add(Shape::mesh(mesh));
    }
}

/// Canvas pixel under a screen point.
fn to_canvas(app: &ToonApp, rect: Rect, p: Pos2) -> Pos2 {
    let size = vec2(app.engine.project.width() as f32, app.engine.project.height() as f32);
    app.view.to_canvas(rect, size, p)
}

fn sample(app: &ToonApp, rect: Rect, p: Pos2, pressure: f32) -> Sample {
    let c = to_canvas(app, rect, p);
    let t = app.stroke_start.map(|s| s.elapsed().as_secs_f64()).unwrap_or(0.0);
    Sample { x: c.x, y: c.y, pressure, tilt_x: 0.0, tilt_y: 0.0, time: t }
}

fn begin(app: &mut ToonApp, s: Sample) {
    let brush = app.brush();
    match app.engine.begin_stroke(app.layer, app.frame, brush) {
        Ok(()) => {
            app.stroke_start = Some(Instant::now());
            let _ = app.engine.stroke_to(Sample { time: 0.0, ..s });
        }
        Err(e) => app.set_status(e.to_string()),
    }
}

fn end(app: &mut ToonApp) {
    if let Err(e) = app.engine.end_stroke() {
        app.set_status(e.to_string());
    }
    app.stroke_start = None;
}

fn handle_input(app: &mut ToonApp, ui: &egui::Ui, rect: Rect, resp: &egui::Response) {
    let size = vec2(app.engine.project.width() as f32, app.engine.project.height() as f32);
    let (events, mods, hover, space, scroll, zoom, multi) =
        ui.input(|i| (i.events.clone(), i.modifiers, i.pointer.hover_pos(), i.key_down(Key::Space), i.smooth_scroll_delta, i.zoom_delta(), i.multi_touch()));
    let hovered = hover.is_some_and(|h| rect.contains(h));
    // view: wheel zooms about the pointer, Alt+wheel rotates, pinch zooms and twists
    if hovered && !app.engine.is_stroking() {
        if let Some(h) = hover {
            if mods.alt && scroll.y != 0.0 {
                app.view.rotate((scroll.y.signum() * 5.0).to_radians());
            } else if scroll.y != 0.0 && zoom == 1.0 {
                app.view.zoom_at(rect, size, h, (scroll.y * 0.0018).exp());
            }
            if zoom != 1.0 {
                app.view.zoom_at(rect, size, h, zoom);
            }
        }
        if let Some(m) = multi
            && m.rotation_delta != 0.0
        {
            app.view.rotate(m.rotation_delta);
        }
    }
    let panning = space || app.tool == Tool::Hand;
    if resp.dragged_by(PointerButton::Middle) || (panning && resp.dragged_by(PointerButton::Primary)) {
        app.view.pan += resp.drag_delta();
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        return;
    }
    if panning && hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    if app.engine.is_stroking() && ui.input(|i| i.key_pressed(Key::Escape)) {
        app.engine.cancel_stroke();
        app.stroke_start = None;
        app.set_status(tl!("Stroke cancelled"));
        return;
    }
    let (layer, frame) = (app.layer, app.frame);
    match app.tool {
        Tool::Fill => {
            if resp.clicked()
                && let Some(p) = resp.interact_pointer_pos()
            {
                let c = to_canvas(app, rect, p);
                if c.x >= 0.0 && c.y >= 0.0 {
                    let (color, options, reference) = (app.color, app.fill, app.fill_reference);
                    app.run(Command::Fill { layer, frame, x: c.x as u32, y: c.y as u32, color, options, reference });
                }
            }
        }
        Tool::Picker => {
            if resp.clicked()
                && let Some(p) = resp.interact_pointer_pos()
            {
                pick(app, rect, p);
            }
        }
        Tool::Hand => {}
        _ if mods.alt => {
            if resp.clicked()
                && let Some(p) = resp.interact_pointer_pos()
            {
                pick(app, rect, p);
            }
        }
        _ => {
            if panning {
                return;
            }
            let t0 = Instant::now();
            let mut fed = false;
            let mut force: Option<f32> = None;
            for ev in events {
                match ev {
                    Event::Touch { pos, force: f, phase, .. } => {
                        force = f.filter(|v| v.is_finite()).map(|v| v.clamp(0.0, 1.0));
                        if phase == TouchPhase::Start && !app.engine.is_stroking() && rect.contains(pos) {
                            let s = sample(app, rect, pos, force.unwrap_or(app.mouse_pressure));
                            begin(app, s);
                            fed = true;
                        }
                    }
                    Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, .. } if !app.engine.is_stroking() && rect.contains(pos) => {
                        let s = sample(app, rect, pos, force.unwrap_or(app.mouse_pressure));
                        begin(app, s);
                        fed = true;
                    }
                    Event::PointerMoved(pos) if app.engine.is_stroking() && app.keyboard_pen.is_none_or(|k| !k.down) => {
                        let s = sample(app, rect, pos, force.unwrap_or(app.mouse_pressure));
                        let _ = app.engine.stroke_to(s);
                        fed = true;
                    }
                    Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, .. }
                        if app.engine.is_stroking() && app.keyboard_pen.is_none_or(|k| !k.down) =>
                    {
                        let s = sample(app, rect, pos, force.unwrap_or(app.mouse_pressure));
                        let _ = app.engine.stroke_to(s);
                        end(app);
                    }
                    Event::PointerGone if app.engine.is_stroking() && app.keyboard_pen.is_none_or(|k| !k.down) => end(app),
                    _ => {}
                }
            }
            if fed {
                app.stats.stroke_ms = t0.elapsed().as_secs_f64() * 1000.0;
            }
        }
    }
    keyboard_pen(app, ui, resp);
}

fn pick(app: &mut ToonApp, rect: Rect, p: Pos2) {
    let c = to_canvas(app, rect, p);
    if c.x < 0.0 || c.y < 0.0 {
        return;
    }
    let col = app.engine.pick_color(app.frame, c.x as u32, c.y as u32);
    app.color = [col[0], col[1], col[2], 255];
    app.set_status(tlf!("Picked #{hex}", hex = format!("{:02X}{:02X}{:02X}", col[0], col[1], col[2])));
}

/// With the canvas focused: arrow keys move a pen cursor (Shift: 10× faster), Enter puts the pen
/// down or lifts it (drawing a stroke with the current brush), the fill tool fills at the cursor
/// with Enter, Escape leaves the keyboard pen.
fn keyboard_pen(app: &mut ToonApp, ui: &egui::Ui, resp: &egui::Response) {
    if !resp.has_focus() {
        if let Some(k) = app.keyboard_pen.take()
            && k.down
        {
            end(app);
        }
        return;
    }
    let (w, h) = (app.engine.project.width() as f32, app.engine.project.height() as f32);
    let mut pen = app.keyboard_pen.unwrap_or(KeyboardPen { x: w / 2.0, y: h / 2.0, down: false });
    let (dx, dy, enter, esc, step) = ui.input(|i| {
        let step = if i.modifiers.shift { 40.0 } else { 4.0 };
        let mut d = (0.0f32, 0.0f32);
        if i.key_pressed(Key::ArrowLeft) {
            d.0 -= 1.0;
        }
        if i.key_pressed(Key::ArrowRight) {
            d.0 += 1.0;
        }
        if i.key_pressed(Key::ArrowUp) {
            d.1 -= 1.0;
        }
        if i.key_pressed(Key::ArrowDown) {
            d.1 += 1.0;
        }
        (d.0, d.1, i.key_pressed(Key::Enter), i.key_pressed(Key::Escape), step)
    });
    if esc {
        if pen.down {
            end(app);
        }
        app.keyboard_pen = None;
        return;
    }
    pen.x = (pen.x + dx * step).clamp(0.0, w - 1.0);
    pen.y = (pen.y + dy * step).clamp(0.0, h - 1.0);
    let t = app.stroke_start.map(|s| s.elapsed().as_secs_f64()).unwrap_or(0.0);
    let s = Sample { x: pen.x, y: pen.y, pressure: app.mouse_pressure, tilt_x: 0.0, tilt_y: 0.0, time: t };
    if (dx != 0.0 || dy != 0.0) && pen.down {
        let _ = app.engine.stroke_to(s);
    }
    if enter {
        match app.tool {
            Tool::Fill => {
                let (layer, frame, color, options, reference) = (app.layer, app.frame, app.color, app.fill, app.fill_reference);
                app.run(Command::Fill { layer, frame, x: pen.x as u32, y: pen.y as u32, color, options, reference });
            }
            Tool::Picker => {
                let col = app.engine.pick_color(app.frame, pen.x as u32, pen.y as u32);
                app.color = [col[0], col[1], col[2], 255];
            }
            Tool::Hand => {}
            _ if pen.down => {
                end(app);
                pen.down = false;
                app.set_status(tl!("Pen lifted"));
            }
            _ => {
                begin(app, s);
                pen.down = app.engine.is_stroking();
                if pen.down {
                    app.set_status(tl!("Pen down: arrow keys draw, Enter lifts"));
                }
            }
        }
    }
    app.keyboard_pen = Some(pen);
}

pub fn show(app: &mut ToonApp, ui: &mut egui::Ui) {
    let rect = ui.available_rect_before_wrap();
    app.workspace.canvas_rect = rect;
    let size = vec2(app.engine.project.width() as f32, app.engine.project.height() as f32);
    if !app.view_fitted && rect.width() > 50.0 {
        app.view = View::fit(rect, size);
        app.view_fitted = true;
    }
    let resp = ui.interact(rect, canvas_id(), Sense::click_and_drag());
    if resp.clicked() || resp.drag_started() {
        resp.request_focus();
    }
    if app.canvas_focus_request {
        resp.request_focus();
        app.canvas_focus_request = false;
    }
    if resp.has_focus() {
        ui.memory_mut(|m| m.set_focus_lock_filter(resp.id, egui::EventFilter { tab: false, horizontal_arrows: true, vertical_arrows: true, escape: false }));
    }
    let layer_name = app.engine.project.layers.get(app.layer).map(|l| l.name.clone()).unwrap_or_default();
    let label = tlf!("Canvas: {layer}, frame {frame}", layer = layer_name, frame = app.frame + 1);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Canvas, true, label.clone()));
    let has = app.engine.project.drawing_at(app.layer, app.frame).is_some();
    let desc = if has {
        tl!("Has a drawing. Arrow keys move the keyboard pen, Enter puts it down or lifts it.")
    } else {
        tl!("Blank frame: drawing here starts a new drawing.")
    };
    ui.ctx().accesskit_node_builder(resp.id, |n| n.set_description(desc.to_string()));
    handle_input(app, ui, rect, &resp);
    // ----- drawing -----
    let pal = crate::theme::palette(app.light_theme);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, pal.workspace);
    let c = Ctx { painter: &painter, view: app.view, rect, size };
    let corners = [c.screen(0.0, 0.0), c.screen(size.x, 0.0), c.screen(size.x, size.y), c.screen(0.0, size.y)];
    let shadow: Vec<Pos2> = corners.iter().map(|p| *p + vec2(3.0, 4.0)).collect();
    painter.add(Shape::convex_polygon(shadow, Color32::from_black_alpha(90), Stroke::NONE));
    let pc = app.engine.project.paper.color;
    painter.add(Shape::convex_polygon(corners.to_vec(), Color32::from_rgb(pc[0], pc[1], pc[2]), Stroke::NONE));
    let ctx = ui.ctx().clone();
    let p = &app.engine.project;
    let tex = &mut app.textures;
    if p.light_table.enabled {
        for pin in &p.light_table.pins {
            let Some(li) = p.layer_index(pin.layer) else { continue };
            if let Some(d) = p.drawing_at(li, pin.frame)
                && let Some(s) = p.drawing(d)
            {
                draw_surface(tex, &ctx, &c, d, s, p.light_table.opacity, Some(p.light_table.tint));
            }
        }
    }
    if p.onion.enabled && !app.playback.playing {
        let layers: Vec<usize> = if p.onion.all_layers { (0..p.layers.len()).collect() } else { vec![app.layer] };
        for li in layers {
            if !p.layers.get(li).is_some_and(|l| l.visible) {
                continue;
            }
            let mut n = p.onion_drawings(li, app.frame);
            n.sort_by_key(|(_, off)| std::cmp::Reverse(off.unsigned_abs()));
            for (d, off) in n {
                let Some(s) = p.drawing(d) else { continue };
                let tint = if !p.onion.tinted {
                    None
                } else if off < 0 {
                    Some(p.onion.before_tint)
                } else {
                    Some(p.onion.after_tint)
                };
                draw_surface(tex, &ctx, &c, d, s, p.onion.step_opacity(off.unsigned_abs()), tint);
            }
        }
    }
    for layer in &p.layers {
        if !layer.visible {
            continue;
        }
        if let Some(d) = layer.drawing_at(app.frame)
            && let Some(s) = p.drawing(d)
        {
            draw_surface(tex, &ctx, &c, d, s, layer.opacity, None);
        }
    }
    // canvas edge
    painter.add(Shape::closed_line(corners.to_vec(), Stroke::new(1.0, Color32::from_black_alpha(120))));
    // cursors
    if let Some(k) = app.keyboard_pen {
        let at = c.screen(k.x, k.y);
        let col = if k.down { pal.accent } else { pal.playhead };
        painter.line_segment([at - vec2(9.0, 0.0), at + vec2(9.0, 0.0)], Stroke::new(1.5, col));
        painter.line_segment([at - vec2(0.0, 9.0), at + vec2(0.0, 9.0)], Stroke::new(1.5, col));
    }
    if let (Some(h), Some(kind)) = (ui.input(|i| i.pointer.hover_pos()), app.tool.brush())
        && rect.contains(h)
        && !ui.input(|i| i.key_down(Key::Space))
    {
        let r = (app.brushes[BRUSH_INDEX[kind as usize]].size * 0.5 * app.view.zoom).max(1.5);
        painter.circle_stroke(h, r, Stroke::new(1.0, Color32::from_black_alpha(160)));
        painter.circle_stroke(h, r + 1.0, Stroke::new(1.0, Color32::from_white_alpha(140)));
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    if resp.has_focus() {
        painter.rect_stroke(rect.shrink(1.0), 0.0, Stroke::new(2.0, pal.playhead), egui::StrokeKind::Inside);
    }
    if app.engine.project.layers.get(app.layer).is_some_and(|l| l.locked) {
        painter.text(rect.left_top() + vec2(10.0, 8.0), egui::Align2::LEFT_TOP, tl!("Layer locked"), egui::FontId::proportional(13.0), pal.accent);
    }
}

/// Index into `ToonApp::brushes` for each `BrushKind` discriminant (same order as `BrushKind::ALL`).
const BRUSH_INDEX: [usize; 5] = [0, 1, 2, 3, 4];
