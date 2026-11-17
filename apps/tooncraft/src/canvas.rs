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

/// Draw a drawing's tiles; `t` maps its pixels to canvas pixels (its peg chain). An affine map
/// keeps every tile a parallelogram, so the textured quads stay exact.
fn draw_surface(
    app_tex: &mut crate::textures::TileTextures,
    ctx: &egui::Context,
    c: &Ctx,
    id: DrawingId,
    s: &Surface,
    opacity: f32,
    tint: Option<[u8; 3]>,
    t: &tooncraft_doc::Affine,
) {
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
        let at = |x: u32, y: u32| {
            let (x, y) = t.apply(x as f32, y as f32);
            c.screen(x, y)
        };
        let corners = [at(x0, y0), at(x1, y0), at(x1, y1), at(x0, y1)];
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

/// Pixel of the current layer's drawing under a screen point (through the inverse of its peg
/// chain, so drawing on a pegged layer lands where the pen is).
fn to_canvas(app: &ToonApp, rect: Rect, p: Pos2) -> Pos2 {
    let c = to_picture(app, rect, p);
    match app.engine.project.layer_transform(app.layer, app.frame).inverse() {
        Some(inv) => {
            let (x, y) = inv.apply(c.x, c.y);
            pos2(x, y)
        }
        None => c,
    }
}

/// Canvas pixel (of the composited picture) under a screen point.
fn to_picture(app: &ToonApp, rect: Rect, p: Pos2) -> Pos2 {
    let size = vec2(app.engine.project.width() as f32, app.engine.project.height() as f32);
    app.view.to_canvas(rect, size, p)
}

fn sample(app: &ToonApp, rect: Rect, p: Pos2, pressure: f32) -> Sample {
    let c = to_canvas(app, rect, p);
    let t = app.stroke_start.map(|s| s.elapsed().as_secs_f64()).unwrap_or(0.0);
    // A tablet reading (X11 / macOS) beats touch force and the mouse setting.
    let (pressure, tilt_x, tilt_y) = match app.tablet.pen() {
        Some(pen) => (pen.pressure, pen.tilt_x, pen.tilt_y),
        None => (pressure, 0.0, 0.0),
    };
    // The view may be rotated or mirrored: tilt is in screen space, the brush works in canvas space.
    let (s, co) = (-app.view.rotation).sin_cos();
    let (mut tx, ty) = (tilt_x * co - tilt_y * s, tilt_x * s + tilt_y * co);
    if app.view.flip {
        tx = -tx;
    }
    Sample { x: c.x, y: c.y, pressure, tilt_x: tx, tilt_y: ty, time: t }
}

fn begin(app: &mut ToonApp, s: Sample) {
    let mut brush = app.brush();
    // Flipping the pen to its eraser end erases with the eraser's settings.
    if app.tablet.pen().is_some_and(|p| p.eraser) {
        brush = tooncraft_brush::Brush { color: brush.color, ..app.brushes[4] };
    }
    match app.engine.begin_stroke(app.layer, app.frame, brush) {
        Ok(()) => {
            app.stroke_start = Some(Instant::now());
            let _ = app.engine.stroke_to(Sample { time: 0.0, ..s });
        }
        Err(e) => app.set_status(e.to_string()),
    }
}

fn end(app: &mut ToonApp) {
    if let Some(s) = app.engine.live_last_sample() {
        app.last_stroke_end = Some((s.x, s.y));
    }
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
                    if app.fill_every_frame {
                        app.run(Command::FillAllFrames { layer, x: c.x as u32, y: c.y as u32, color, options, reference });
                    } else {
                        app.run(Command::Fill { layer, frame, x: c.x as u32, y: c.y as u32, color, options, reference });
                    }
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
        Tool::Select => select_input(app, rect, resp),
        Tool::Shape => {
            if let Some(p) = resp.interact_pointer_pos() {
                let c = to_canvas(app, rect, p);
                if resp.drag_started() {
                    app.shape_drag = Some(((c.x, c.y), (c.x, c.y)));
                } else if resp.dragged()
                    && let Some((a, _)) = app.shape_drag
                {
                    app.shape_drag = Some((a, (c.x, c.y)));
                }
            }
            if resp.drag_stopped()
                && let Some((from, to)) = app.shape_drag.take()
            {
                let brush = tooncraft_brush::Brush { color: app.color, ..app.brushes[1] };
                let (layer, frame, shape) = (app.layer, app.frame, app.shape);
                app.run(Command::Shape { layer, frame, brush, shape, from, to, constrain: mods.shift });
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
                    Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers, .. }
                        if !app.engine.is_stroking() && rect.contains(pos) && modifiers.shift && app.last_stroke_end.is_some() =>
                    {
                        // Shift+click: a straight line from where the last stroke ended.
                        let to = sample(app, rect, pos, force.unwrap_or(app.mouse_pressure));
                        let (x0, y0) = app.last_stroke_end.unwrap_or((to.x, to.y));
                        let n = (((to.x - x0).hypot(to.y - y0)) / 2.0).ceil().clamp(1.0, 4000.0) as usize;
                        begin(app, Sample { x: x0, y: y0, ..to });
                        for k in 1..=n {
                            let t = k as f32 / n as f32;
                            let _ = app.engine.stroke_to(Sample { x: x0 + (to.x - x0) * t, y: y0 + (to.y - y0) * t, time: t as f64 * 0.2, ..to });
                        }
                        end(app);
                        fed = true;
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

/// Handles of a selection in canvas pixels: the four corners and the rotate knob above the top edge.
fn handles(poly: &[(f32, f32)], zoom: f32) -> ([(f32, f32); 4], (f32, f32)) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in poly {
        x0 = x0.min(p.0);
        y0 = y0.min(p.1);
        x1 = x1.max(p.0);
        y1 = y1.max(p.1);
    }
    ([(x0, y0), (x1, y0), (x1, y1), (x0, y1)], ((x0 + x1) * 0.5, y0 - 28.0 / zoom.max(0.01)))
}

fn inside(poly: &[(f32, f32)], x: f32, y: f32) -> bool {
    let mut c = false;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if (a.1 > y) != (b.1 > y) && x < a.0 + (y - a.1) / (b.1 - a.1) * (b.0 - a.0) {
            c = !c;
        }
    }
    c
}

/// The select tool: drag outside the selection draws a new one (lasso or rectangle); drag inside
/// moves it, a corner scales it, the knob rotates it — shown live (the transform is re-applied
/// each frame as a single undo step) and committed on release.
fn select_input(app: &mut ToonApp, rect: Rect, resp: &egui::Response) {
    use crate::app::{SelectMode, Xform, XformKind};
    let Some(p) = resp.interact_pointer_pos() else { return };
    let c = to_canvas(app, rect, p);
    let tol = 10.0 / app.view.zoom.max(0.01);
    if resp.drag_started() {
        app.selecting.clear();
        app.xform = None;
        if let Some(poly) = app.selection.clone() {
            let (corners, knob) = handles(&poly, app.view.zoom);
            let near = |q: (f32, f32)| (q.0 - c.x).hypot(q.1 - c.y) <= tol;
            let kind = if near(knob) {
                Some(XformKind::Rotate)
            } else if corners.iter().any(|q| near(*q)) {
                Some(XformKind::Scale)
            } else if inside(&poly, c.x, c.y) {
                Some(XformKind::Move)
            } else {
                None
            };
            if let Some(kind) = kind {
                app.xform = Some(Xform { base: poly, kind, start: (c.x, c.y), applied: false, now: (0.0, 0.0, 1.0, 0.0) });
                return;
            }
        }
        app.selection = None;
        app.selecting.push((c.x, c.y));
        return;
    }
    if let Some(mut x) = app.xform.clone() {
        let pivot = tooncraft_engine::select::pivot(&x.base);
        let (dx, dy, sc, ang) = match x.kind {
            XformKind::Move => (c.x - x.start.0, c.y - x.start.1, 1.0, 0.0),
            XformKind::Scale => {
                let d0 = (x.start.0 - pivot.0).hypot(x.start.1 - pivot.1).max(1.0);
                let d1 = (c.x - pivot.0).hypot(c.y - pivot.1);
                (0.0, 0.0, (d1 / d0).clamp(0.02, 50.0), 0.0)
            }
            XformKind::Rotate => {
                let a0 = (x.start.1 - pivot.1).atan2(x.start.0 - pivot.0);
                let a1 = (c.y - pivot.1).atan2(c.x - pivot.0);
                (0.0, 0.0, 1.0, a1 - a0)
            }
        };
        if resp.dragged() && (dx, dy, sc, ang) != x.now {
            if x.applied {
                let _ = app.engine.undo();
            }
            let (layer, frame) = (app.layer, app.frame);
            x.applied = app.run(Command::TransformSelection { layer, frame, polygon: x.base.clone(), dx, dy, scale_x: sc, scale_y: sc, angle: ang });
            x.now = (dx, dy, sc, ang);
            let t = tooncraft_engine::select::affine(&x.base, dx, dy, sc, sc, ang);
            app.selection = Some(x.base.iter().map(|p| t.apply(p.0, p.1)).collect());
            app.xform = Some(x);
        }
        if resp.drag_stopped() {
            app.xform = None;
        }
        return;
    }
    if resp.dragged() {
        match app.select_mode {
            SelectMode::Lasso => {
                if app.selecting.last().is_none_or(|l| (l.0 - c.x).hypot(l.1 - c.y) > 1.5 / app.view.zoom.max(0.01)) {
                    app.selecting.push((c.x, c.y));
                }
            }
            SelectMode::Rectangle => {
                if let Some(&first) = app.selecting.first() {
                    app.selecting = vec![first, (c.x, first.1), (c.x, c.y), (first.0, c.y)];
                }
            }
        }
    }
    if resp.drag_stopped() {
        if app.selecting.len() >= 3 {
            app.selection = Some(std::mem::take(&mut app.selecting));
        }
        app.selecting.clear();
    }
    if resp.clicked() && app.selection.as_ref().is_some_and(|poly| !inside(poly, c.x, c.y)) {
        app.selection = None;
    }
}

fn draw_selection(painter: &egui::Painter, app: &ToonApp, c: &Ctx, pal: &crate::theme::Palette) {
    let t = c.view;
    let to = |p: &(f32, f32)| t.to_screen(c.rect, c.size, pos2(p.0, p.1));
    let ants = |pts: &[(f32, f32)]| {
        if pts.len() < 2 {
            return;
        }
        let mut line: Vec<Pos2> = pts.iter().map(to).collect();
        if let Some(f) = line.first().copied() {
            line.push(f);
        }
        painter.add(Shape::line(line.clone(), Stroke::new(2.5, Color32::from_black_alpha(200))));
        painter.add(Shape::dashed_line(&line, Stroke::new(1.2, Color32::WHITE), 5.0, 4.0));
    };
    ants(&app.selecting);
    if let Some(poly) = &app.selection {
        ants(poly);
        if app.tool == Tool::Select {
            let (corners, knob) = handles(poly, app.view.zoom);
            let top = to(&((corners[0].0 + corners[1].0) * 0.5, corners[0].1));
            let k = to(&knob);
            painter.line_segment([top, k], Stroke::new(1.0, pal.playhead));
            painter.circle(k, 5.0, pal.panel, Stroke::new(1.5, pal.playhead));
            for q in &corners {
                painter.rect(Rect::from_center_size(to(q), vec2(8.0, 8.0)), 1.0, pal.panel, Stroke::new(1.5, pal.playhead), egui::StrokeKind::Middle);
            }
        }
    }
}

fn pick(app: &mut ToonApp, rect: Rect, p: Pos2) {
    let c = to_picture(app, rect, p);
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
    if app.tool == Tool::Select && app.selection.is_some() {
        let (d, enter_or_esc) = ui.input(|i| {
            let step = if i.modifiers.shift { 10.0 } else { 1.0 };
            let mut d = (0.0f32, 0.0f32);
            for (k, v) in [(Key::ArrowLeft, (-step, 0.0)), (Key::ArrowRight, (step, 0.0)), (Key::ArrowUp, (0.0, -step)), (Key::ArrowDown, (0.0, step))] {
                if i.key_pressed(k) && !i.modifiers.alt {
                    d.0 += v.0;
                    d.1 += v.1;
                }
            }
            (d, i.key_pressed(Key::Escape))
        });
        if d != (0.0, 0.0) {
            app.transform_selection(d.0, d.1, (1.0, 1.0), 0.0);
        }
        if enter_or_esc {
            app.selection = None;
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
    let _ = &app.blend_cache;
    if p.light_table.enabled {
        for pin in &p.light_table.pins {
            let Some(li) = p.layer_index(pin.layer) else { continue };
            if let Some(d) = p.drawing_at(li, pin.frame)
                && let Some(s) = p.drawing(d)
            {
                draw_surface(tex, &ctx, &c, d, s, p.light_table.opacity, Some(p.light_table.tint), &p.layer_transform(li, pin.frame));
            }
        }
    }
    if p.onion.enabled && !app.playback.playing {
        let layers: Vec<usize> = if p.onion.all_layers { (0..p.layers.len()).collect() } else { vec![app.layer] };
        for li in layers {
            if !p.layers.get(li).is_some_and(|l| l.visible) {
                continue;
            }
            let t = p.layer_transform(li, app.frame);
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
                draw_surface(tex, &ctx, &c, d, s, p.onion.step_opacity(off.unsigned_abs()), tint, &t);
            }
        }
    }
    let blended =
        p.layers.iter().any(|l| (l.blend != tooncraft_doc::BlendMode::Normal || tooncraft_fx::is_active(&l.effects) || l.matte.is_some()) && p.effective(l).0);
    if blended {
        // A blend mode needs what is below it, and effects reach past their tiles: composite the
        // layers on the CPU (cached until the project or frame changes) and show that as tiles.
        let key = app.engine.changes().wrapping_mul(1_000_003) ^ (app.frame as u64).wrapping_mul(7919);
        if app.blend_cache.as_ref().is_none_or(|(k, _)| *k != key) {
            let img = tooncraft_doc::render_frame(p, app.frame, tooncraft_doc::RenderOptions { paper: false, ..Default::default() });
            if let Ok(mut s) = tooncraft_raster::Surface::new(img.width, img.height) {
                let _ = s.load_image(&img);
                app.blend_cache = Some((key, s));
            }
        }
        if let Some((_, s)) = &app.blend_cache {
            draw_surface(tex, &ctx, &c, tooncraft_doc::DrawingId(u64::MAX), s, 1.0, None, &tooncraft_doc::Affine::IDENTITY);
        }
    } else {
        for (li, layer) in p.layers.iter().enumerate() {
            let (visible, opacity) = p.effective_at(layer, app.frame);
            if !visible {
                continue;
            }
            if let Some(d) = layer.drawing_at(app.frame)
                && let Some(s) = p.drawing(d)
            {
                draw_surface(tex, &ctx, &c, d, s, opacity, None, &p.layer_transform(li, app.frame));
            }
        }
    }
    // canvas edge
    painter.add(Shape::closed_line(corners.to_vec(), Stroke::new(1.0, Color32::from_black_alpha(120))));
    draw_selection(&painter, app, &c, pal);
    if let Some((a, b)) = app.shape_drag {
        let shift = ui.input(|i| i.modifiers.shift);
        let pts = tooncraft_engine::shapes::samples(app.shape, a, b, shift, 1.0);
        let line: Vec<Pos2> = pts.iter().step_by(2).map(|s| c.screen(s.x, s.y)).collect();
        painter.add(Shape::line(line, Stroke::new(1.5, pal.accent)));
    }
    crate::pegs::draw_path(app, &painter, |x, y| c.screen(x, y), pal.accent);
    // camera frame
    let cam = &app.engine.project.camera;
    if app.show_camera && (!cam.keys.is_empty() || !cam.shakes.is_empty()) {
        let (w, h) = (app.engine.project.width(), app.engine.project.height());
        let map = cam.output_to_canvas(app.frame, w, h);
        let pts: Vec<Pos2> = [(0.0, 0.0), (w as f32, 0.0), (w as f32, h as f32), (0.0, h as f32)]
            .iter()
            .map(|(x, y)| {
                let (cx, cy) = map(*x, *y);
                c.screen(cx, cy)
            })
            .collect();
        let col = if cam.enabled { pal.accent } else { pal.weak };
        painter.add(Shape::closed_line(pts.clone(), Stroke::new(2.0, col)));
        if let Some(p0) = pts.first() {
            painter.text(*p0 + vec2(4.0, 3.0), egui::Align2::LEFT_TOP, tl!("Camera"), egui::FontId::proportional(12.0), col);
        }
    }
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
