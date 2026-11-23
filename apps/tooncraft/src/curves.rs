//! The Curves panel: one peg value (move, rotation, scale, skew) plotted against frames, with its
//! keys. Keys are picked and dragged in time and value (one undo step per drag, Escape cancels),
//! several keys get the same ease at once, and the ease handles of the chosen keys are dragged in
//! a small timing box. Everything works from the keyboard as well:
//!
//! - Left / Right choose the previous or next key (Shift adds it to the chosen keys);
//! - Alt+Left / Alt+Right move the chosen keys a frame (Shift: ten);
//! - Up / Down change the value (Shift: ten steps);
//! - Ctrl+A chooses every key, Delete removes the chosen keys, Enter goes to the key's frame.

use std::collections::BTreeSet;

use egui::{Color32, Modifiers, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use tooncraft_doc::{Channel, Ease, Peg, PegId};
use tooncraft_engine::Command;

use crate::app::ToonApp;

/// The graph's widget id (tests and the keyboard focus it).
pub fn graph_id() -> egui::Id {
    egui::Id::unique("tooncraft-curve-graph")
}

/// The ease box's widget id.
pub fn handles_id() -> egui::Id {
    egui::Id::unique("tooncraft-curve-handles")
}

/// Key dragging starts this close to a key (screen points).
const PICK: f32 = 10.0;

/// A pointer drag in progress; nothing in the project changes until it ends.
#[derive(Clone, Copy, Debug)]
struct Drag {
    start: Pos2,
    now: Pos2,
    view: View,
}

/// A handle drag in the ease box, kept until release.
#[derive(Clone, Copy, Debug)]
struct HandleDrag {
    handle: usize,
    value: (f32, f32, f32, f32),
}

/// The frames and values the graph shows.
#[derive(Clone, Copy, Debug, PartialEq)]
struct View {
    f0: f32,
    f1: f32,
    v0: f32,
    v1: f32,
}

impl View {
    fn to_screen(self, r: Rect, f: f32, v: f32) -> Pos2 {
        let x = r.left() + (f - self.f0) / (self.f1 - self.f0).max(1e-3) * r.width();
        let y = r.bottom() - (v - self.v0) / (self.v1 - self.v0).max(1e-6) * r.height();
        pos2(x, y)
    }

    fn px_per_frame(self, r: Rect) -> f32 {
        r.width() / (self.f1 - self.f0).max(1e-3)
    }

    fn px_per_value(self, r: Rect) -> f32 {
        r.height() / (self.v1 - self.v0).max(1e-6)
    }
}

/// Sampled curves, kept until the keys, the value or the view change.
#[derive(Default)]
struct Cache {
    keys: Vec<tooncraft_doc::PegKey>,
    view: Option<View>,
    width: usize,
    curves: Vec<(Channel, Vec<(f32, f32)>)>,
}

/// Panel state that is not part of the project.
pub struct CurveUi {
    pub channel: Channel,
    /// Draw the peg's other values faintly, each scaled to the graph.
    pub overlay: bool,
    /// The chosen keys' frames, on the peg the Pegs panel has selected.
    pub chosen: BTreeSet<u32>,
    /// The key the keyboard is on.
    pub current: Option<u32>,
    peg: Option<PegId>,
    drag: Option<Drag>,
    handle_drag: Option<HandleDrag>,
    /// Handle values while their number fields are being dragged.
    typed: Option<(f32, f32, f32, f32)>,
    /// Where the graph was drawn last frame, and what it showed.
    layout: Option<(Rect, View)>,
    cache: Cache,
}

impl Default for CurveUi {
    fn default() -> Self {
        Self {
            channel: Channel::X,
            overlay: true,
            chosen: BTreeSet::new(),
            current: None,
            peg: None,
            drag: None,
            handle_drag: None,
            typed: None,
            layout: None,
            cache: Cache::default(),
        }
    }
}

#[cfg(test)]
impl CurveUi {
    /// Whether a drag is waiting for release (tests check that cancelling leaves none).
    pub fn dragging(&self) -> bool {
        self.drag.is_some() || self.handle_drag.is_some() || self.typed.is_some()
    }

    /// Where a key at `frame` with `value` was drawn last frame.
    pub fn screen_pos(&self, frame: u32, value: f32) -> Option<Pos2> {
        self.layout.map(|(r, v)| v.to_screen(r, frame as f32, value))
    }

    /// Screen points per frame and per unit of value last frame.
    pub fn scale(&self) -> Option<(f32, f32)> {
        self.layout.map(|(r, v)| (v.px_per_frame(r), v.px_per_value(r)))
    }
}

pub fn channel_label(c: Channel) -> String {
    match c {
        Channel::X => tl!("Move X"),
        Channel::Y => tl!("Move Y"),
        Channel::Angle => tl!("Rotation"),
        Channel::ScaleX => tl!("Scale X"),
        Channel::ScaleY => tl!("Scale Y"),
        Channel::Skew => tl!("Skew"),
    }
    .to_string()
}

/// The smallest value span the graph shows, so a flat curve is not blown up.
fn min_span(c: Channel) -> f32 {
    match c {
        Channel::ScaleX | Channel::ScaleY => 0.2,
        _ => 20.0,
    }
}

fn fit(peg: &Peg, channel: Channel, frame: u32, last: u32) -> View {
    let (first_key, last_key) = match (peg.keys.first(), peg.keys.last()) {
        (Some(a), Some(b)) => (a.frame, b.frame),
        _ => (frame, frame),
    };
    let lo = first_key.min(frame) as f32;
    let hi = last_key.max(frame) as f32;
    let pad = ((hi - lo) * 0.08).max(2.0);
    let (f0, f1) = ((lo - pad).max(0.0), (hi + pad).min(last.max(1) as f32).max(lo + 1.0));
    let mut v0 = f32::INFINITY;
    let mut v1 = f32::NEG_INFINITY;
    for (_, v) in peg.sample(channel, f0, f1, 256) {
        v0 = v0.min(v);
        v1 = v1.max(v);
    }
    for k in &peg.keys {
        let v = channel.get(k);
        v0 = v0.min(v);
        v1 = v1.max(v);
    }
    if !v0.is_finite() || !v1.is_finite() {
        let rest = channel.get(&tooncraft_doc::PegKey::rest(0));
        (v0, v1) = (rest, rest);
    }
    let span = (v1 - v0).max(min_span(channel));
    let mid = (v0 + v1) * 0.5;
    View { f0, f1, v0: mid - span * 0.6, v1: mid + span * 0.6 }
}

fn nearest(peg: &Peg, channel: Channel, view: &View, r: Rect, p: Pos2) -> Option<u32> {
    peg.keys
        .iter()
        .map(|k| (k.frame, view.to_screen(r, k.frame as f32, channel.get(k)).distance(p)))
        .filter(|(_, d)| *d <= PICK)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(f, _)| f)
}

/// Round a value change to a tenth of the value's keyboard step.
fn round_value(c: Channel, v: f32) -> f32 {
    let q = c.step() * 0.1;
    (v / q).round() * q
}

/// The frame and value change a pointer drag stands for.
fn drag_delta(d: &Drag, r: Rect, c: Channel) -> (i64, f32) {
    let df = ((d.now.x - d.start.x) / d.view.px_per_frame(r).max(1e-3)).round();
    let dv = -(d.now.y - d.start.y) / d.view.px_per_value(r).max(1e-6);
    let df = if df.is_finite() { df.clamp(-(tooncraft_doc::MAX_FRAMES as f32), tooncraft_doc::MAX_FRAMES as f32) as i64 } else { 0 };
    (df, if dv.is_finite() { round_value(c, dv) } else { 0.0 })
}

/// Move the chosen keys; on success the chosen set follows them.
fn move_chosen(app: &mut ToonApp, peg: PegId, by_frames: i64, by_value: f32) {
    let c = &app.curve_ui;
    if c.chosen.is_empty() || (by_frames == 0 && by_value == 0.0) {
        return;
    }
    let channel = (by_value != 0.0).then_some(c.channel);
    let frames: Vec<u32> = c.chosen.iter().copied().collect();
    if app.run(Command::MovePegKeys { peg, frames, by_frames, channel, by_value }) {
        let shift = |f: u32| u32::try_from(f as i64 + by_frames).unwrap_or(f);
        app.curve_ui.chosen = app.curve_ui.chosen.iter().map(|f| shift(*f)).collect();
        app.curve_ui.current = app.curve_ui.current.map(shift);
    }
}

pub fn panel(app: &mut ToonApp, ui: &mut Ui) {
    // the peg is the one the Pegs panel edits
    if app.peg_ui.selected.is_some_and(|id| app.engine.project.peg(id).is_none()) {
        app.peg_ui.selected = None;
    }
    if app.peg_ui.selected.is_none() {
        app.peg_ui.selected = app.engine.project.pegs.first().map(|p| p.id);
    }
    if app.curve_ui.peg != app.peg_ui.selected {
        app.curve_ui.peg = app.peg_ui.selected;
        app.curve_ui.chosen.clear();
        app.curve_ui.current = None;
        app.curve_ui.drag = None;
        app.curve_ui.handle_drag = None;
        app.curve_ui.typed = None;
    }
    toolbar(app, ui);
    let Some(id) = app.peg_ui.selected else {
        ui.label(tl!("No pegs yet. Add one in the Pegs panel to see its keys here."));
        return;
    };
    let Some(peg) = app.engine.project.peg(id).cloned() else { return };
    // forget chosen keys that are gone (undo, another panel)
    app.curve_ui.chosen.retain(|f| peg.has_key(*f));
    if app.curve_ui.current.is_some_and(|f| !peg.has_key(f)) {
        app.curve_ui.current = None;
    }
    ui.horizontal_top(|ui| {
        let side = 170.0;
        let w = (ui.available_width() - side - 12.0).max(160.0);
        let h = ui.available_height().clamp(120.0, 600.0);
        ui.allocate_ui(vec2(w, h), |ui| graph(app, ui, id, &peg, vec2(w, h)));
        ui.vertical(|ui| egui::ScrollArea::vertical().id_salt("curve-ease").show(ui, |ui| ease_box(app, ui, id, &peg)));
    });
}

fn toolbar(app: &mut ToonApp, ui: &mut Ui) {
    ui.horizontal_wrapped(|ui| {
        let l = ui.label(tl!("Peg"));
        let pegs: Vec<(PegId, String)> = app.engine.project.pegs.iter().map(|p| (p.id, p.name.clone())).collect();
        let mut sel = app.peg_ui.selected;
        let name = sel.and_then(|i| pegs.iter().find(|(p, _)| *p == i)).map(|(_, n)| n.clone()).unwrap_or_else(|| tl!("None").to_string());
        egui::ComboBox::from_id_salt("curve-peg")
            .selected_text(name)
            .show_ui(ui, |ui| {
                for (pid, pname) in &pegs {
                    ui.selectable_value(&mut sel, Some(*pid), pname);
                }
            })
            .response
            .labelled_by(l.id);
        if sel != app.peg_ui.selected {
            app.peg_ui.selected = sel;
        }
        ui.separator();
        for c in Channel::ALL {
            if ui.selectable_label(app.curve_ui.channel == c, channel_label(c)).on_hover_text(tl!("Show and edit this value")).clicked() {
                app.curve_ui.channel = c;
            }
        }
        ui.separator();
        ui.checkbox(&mut app.curve_ui.overlay, tl!("Other values"))
            .on_hover_text(tl!("Draw the peg's other values faintly, each scaled to fit, to compare their timing"));
        if let Some(id) = app.peg_ui.selected
            && ui.button(tl!("Key Here")).on_hover_text(tl!("Set a key at the current frame with the peg's values there")).clicked()
            && let Some(p) = app.engine.project.peg(id)
        {
            let k = p.at(app.frame);
            let frame = app.frame;
            if app.run(Command::SetPegKey {
                peg: id,
                frame,
                x: k.x,
                y: k.y,
                angle: k.angle,
                scale_x: k.scale_x,
                scale_y: k.scale_y,
                skew: k.skew,
                ease: k.ease,
            }) {
                app.curve_ui.chosen = BTreeSet::from([frame]);
                app.curve_ui.current = Some(frame);
            }
        }
    });
}

fn graph(app: &mut ToonApp, ui: &mut Ui, id: PegId, peg: &Peg, size: egui::Vec2) {
    let channel = app.curve_ui.channel;
    let last = app.engine.project.length().saturating_sub(1);
    let (_, rect) = ui.allocate_space(size);
    let resp = ui.interact(rect, graph_id(), Sense::click_and_drag());
    let view = app.curve_ui.drag.map(|d| d.view).unwrap_or_else(|| fit(peg, channel, app.frame, last));

    // ----- pointer -----
    // a drag picks the key where the button went down, not where the pointer is now
    let origin = if resp.drag_started() { ui.input(|i| i.pointer.press_origin()) } else { None };
    if (resp.clicked() || resp.drag_started())
        && let Some(p) = origin.or(resp.interact_pointer_pos())
    {
        resp.request_focus();
        let shift = ui.input(|i| i.modifiers.shift);
        let c = &mut app.curve_ui;
        match nearest(peg, channel, &view, rect, p) {
            Some(f) => {
                if shift && resp.clicked() {
                    if !c.chosen.remove(&f) {
                        c.chosen.insert(f);
                    }
                } else if !c.chosen.contains(&f) {
                    if !shift {
                        c.chosen.clear();
                    }
                    c.chosen.insert(f);
                }
                c.current = Some(f);
                if resp.drag_started() {
                    c.drag = Some(Drag { start: p, now: p, view });
                }
            }
            None if resp.clicked() => {
                if !shift {
                    c.chosen.clear();
                }
                // a click on the graph goes to that frame
                let f = view.f0 + (p.x - rect.left()) / view.px_per_frame(rect).max(1e-3);
                if f.is_finite() {
                    app.frame = (f.round().max(0.0) as u32).min(last);
                }
            }
            None => {}
        }
    }
    if let Some(d) = app.curve_ui.drag.as_mut()
        && let Some(p) = resp.interact_pointer_pos()
    {
        d.now = p;
    }
    if app.curve_ui.drag.is_some() && ui.input_mut(|i| i.consume_key(Modifiers::NONE, egui::Key::Escape)) {
        app.curve_ui.drag = None;
    }
    if resp.drag_stopped()
        && let Some(d) = app.curve_ui.drag.take()
    {
        let (df, dv) = drag_delta(&d, rect, channel);
        move_chosen(app, id, df, dv);
    } else if app.curve_ui.drag.is_some() && !resp.dragged() && !ui.input(|i| i.pointer.any_down()) {
        // the pointer was let go somewhere this widget did not see: drop the preview
        app.curve_ui.drag = None;
    }

    // ----- keyboard -----
    if resp.has_focus() {
        ui.memory_mut(|m| m.set_focus_lock_filter(resp.id, egui::EventFilter { tab: false, horizontal_arrows: true, vertical_arrows: true, escape: false }));
        keyboard(app, ui, id, peg);
    }

    // ----- paint -----
    let Some(peg) = app.engine.project.peg(id).cloned() else { return };
    let view = app.curve_ui.drag.map(|d| d.view).unwrap_or_else(|| fit(&peg, channel, app.frame, last));
    app.curve_ui.layout = Some((rect, view));
    let visuals = ui.visuals().clone();
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, visuals.extreme_bg_color);
    let grid = Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color);
    let text = visuals.weak_text_color();
    let font = egui::FontId::proportional(10.0);
    // frame lines, at least 36 points apart
    let ppf = view.px_per_frame(rect);
    let step = [1u32, 2, 5, 10, 12, 24, 48, 100, 250, 500, 1000, 5000, 10_000].into_iter().find(|s| *s as f32 * ppf >= 36.0).unwrap_or(65_536);
    let mut f = (view.f0.max(0.0) as u32).div_ceil(step) * step;
    let mut lines = 0;
    while (f as f32) <= view.f1 && lines < 200 {
        let x = view.to_screen(rect, f as f32, 0.0).x;
        painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], grid);
        painter.text(pos2(x + 2.0, rect.bottom() - 2.0), egui::Align2::LEFT_BOTTOM, (f + 1).to_string(), font.clone(), text);
        f += step;
        lines += 1;
    }
    for k in 0..=4 {
        let v = view.v0 + (view.v1 - view.v0) * k as f32 / 4.0;
        let y = view.to_screen(rect, view.f0, v).y;
        painter.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], grid);
        painter.text(pos2(rect.left() + 2.0, y - 1.0), egui::Align2::LEFT_BOTTOM, format!("{v:.2}"), font.clone(), text);
    }
    let cur = view.to_screen(rect, app.frame as f32, 0.0).x;
    painter.line_segment([pos2(cur, rect.top()), pos2(cur, rect.bottom())], Stroke::new(1.5, visuals.selection.stroke.color.gamma_multiply(0.7)));

    // curves, sampled once per change of keys, view or width
    let width = (rect.width().max(2.0) as usize).min(tooncraft_doc::peg::MAX_SAMPLES);
    let cache = &mut app.curve_ui.cache;
    if cache.keys != peg.keys || cache.view != Some(view) || cache.width != width {
        cache.keys = peg.keys.clone();
        cache.view = Some(view);
        cache.width = width;
        cache.curves = Channel::ALL.iter().map(|c| (*c, peg.sample(*c, view.f0, view.f1, width))).collect();
    }
    let accent = visuals.selection.bg_fill.gamma_multiply(1.6);
    for (c, pts) in &app.curve_ui.cache.curves {
        if *c == channel {
            let line: Vec<Pos2> = pts.iter().map(|(f, v)| view.to_screen(rect, *f, *v)).collect();
            painter.add(egui::Shape::line(line, Stroke::new(2.0, accent)));
        } else if app.curve_ui.overlay {
            let (lo, hi) = pts.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), (_, v)| (a.min(*v), b.max(*v)));
            if !(hi - lo).is_finite() || hi - lo < 1e-6 {
                continue;
            }
            let line: Vec<Pos2> = pts
                .iter()
                .map(|(f, v)| {
                    let x = view.to_screen(rect, *f, 0.0).x;
                    pos2(x, rect.bottom() - 6.0 - (v - lo) / (hi - lo) * (rect.height() - 12.0))
                })
                .collect();
            painter.add(egui::Shape::line(line, Stroke::new(1.0, text.gamma_multiply(0.6))));
        }
    }
    // keys, and where a drag would put them
    let (df, dv) = app.curve_ui.drag.map(|d| drag_delta(&d, rect, channel)).unwrap_or((0, 0.0));
    for k in &peg.keys {
        let p = view.to_screen(rect, k.frame as f32, channel.get(k));
        let chosen = app.curve_ui.chosen.contains(&k.frame);
        diamond(&painter, p, if chosen { 6.0 } else { 4.5 }, if chosen { Color32::WHITE } else { visuals.widgets.inactive.fg_stroke.color }, accent);
        if app.curve_ui.current == Some(k.frame) {
            painter.circle_stroke(p, 9.0, Stroke::new(1.5, visuals.selection.stroke.color));
        }
        if chosen && app.curve_ui.drag.is_some() {
            let q = view.to_screen(rect, k.frame as f32 + df as f32, channel.get(k) + dv);
            diamond(&painter, q, 5.0, accent.gamma_multiply(0.6), accent);
        }
    }
    if app.curve_ui.drag.is_some() {
        painter.text(
            rect.right_top() + vec2(-6.0, 4.0),
            egui::Align2::RIGHT_TOP,
            tlf!("{f} frames, {v} (Escape cancels)", f = format!("{df:+}"), v = format!("{dv:+.2}")),
            egui::FontId::proportional(12.0),
            visuals.text_color(),
        );
    }
    painter.rect_stroke(rect, 3.0, if resp.has_focus() { visuals.selection.stroke } else { grid }, egui::StrokeKind::Inside);

    let current = app.curve_ui.current.and_then(|f| peg.keys.iter().position(|k| k.frame == f));
    let value = current.and_then(|i| peg.keys.get(i)).map(|k| channel.get(k)).unwrap_or(0.0);
    let label = match current.and_then(|i| peg.keys.get(i).map(|k| (i, k.frame))) {
        Some((i, f)) => tlf!(
            "{name} curve: key {n} of {count} at frame {frame}, value {value}; {chosen} chosen",
            name = channel_label(channel),
            n = i + 1,
            count = peg.keys.len(),
            frame = f + 1,
            value = format!("{value:.2}"),
            chosen = app.curve_ui.chosen.len()
        ),
        None => tlf!("{name} curve: {count} keys", name = channel_label(channel), count = peg.keys.len()),
    };
    resp.widget_info(|| egui::WidgetInfo::slider(true, value as f64, label.clone()));
}

fn diamond(painter: &egui::Painter, c: Pos2, r: f32, fill: Color32, edge: Color32) {
    let pts = vec![c + vec2(0.0, -r), c + vec2(r, 0.0), c + vec2(0.0, r), c + vec2(-r, 0.0)];
    painter.add(egui::Shape::convex_polygon(pts, fill, Stroke::new(1.5, edge)));
}

fn keyboard(app: &mut ToonApp, ui: &mut Ui, id: PegId, peg: &Peg) {
    use egui::Key::*;
    let (slide, extend, step, value, all, delete, go) = ui.input_mut(|i| {
        let mut n = |m: Modifiers, k| i.consume_key(m, k) as i32;
        // the most specific modifiers first: consume_key ignores extra Shift
        let slide_far = n(Modifiers::ALT | Modifiers::SHIFT, ArrowRight) - n(Modifiers::ALT | Modifiers::SHIFT, ArrowLeft);
        let slide = n(Modifiers::ALT, ArrowRight) - n(Modifiers::ALT, ArrowLeft);
        let extend = n(Modifiers::SHIFT, ArrowRight) - n(Modifiers::SHIFT, ArrowLeft);
        let step = n(Modifiers::NONE, ArrowRight) - n(Modifiers::NONE, ArrowLeft);
        let far = n(Modifiers::SHIFT, ArrowUp) - n(Modifiers::SHIFT, ArrowDown);
        let near = n(Modifiers::NONE, ArrowUp) - n(Modifiers::NONE, ArrowDown);
        let all = n(Modifiers::COMMAND, A) > 0;
        let delete = n(Modifiers::NONE, Delete) + n(Modifiers::NONE, Backspace) > 0;
        let go = n(Modifiers::NONE, Enter) > 0;
        (10 * slide_far + slide, extend, step, 10 * far + near, all, delete, go)
    });
    let frames: Vec<u32> = peg.keys.iter().map(|k| k.frame).collect();
    let c = &mut app.curve_ui;
    let walk = step + extend;
    if walk != 0 && !frames.is_empty() {
        let at = c.current.and_then(|f| frames.iter().position(|g| *g == f));
        let i = match at {
            Some(i) => (i as i64 + walk as i64).clamp(0, frames.len() as i64 - 1) as usize,
            None if walk > 0 => 0,
            None => frames.len() - 1,
        };
        if let Some(f) = frames.get(i).copied() {
            if extend == 0 {
                c.chosen.clear();
            }
            c.chosen.insert(f);
            c.current = Some(f);
        }
    }
    if all {
        c.chosen = frames.iter().copied().collect();
        if c.current.is_none() {
            c.current = frames.first().copied();
        }
    }
    if go && let Some(f) = c.current {
        app.frame = f;
    }
    if c.chosen.is_empty()
        && let Some(f) = c.current
    {
        c.chosen.insert(f);
    }
    if slide != 0 {
        move_chosen(app, id, slide as i64, 0.0);
    }
    if value != 0 {
        let by = round_value(app.curve_ui.channel, value as f32 * app.curve_ui.channel.step());
        move_chosen(app, id, 0, by);
    }
    if delete && !app.curve_ui.chosen.is_empty() {
        let frames: Vec<u32> = app.curve_ui.chosen.iter().copied().collect();
        if app.run(Command::RemovePegKeys { peg: id, frames }) {
            app.curve_ui.chosen.clear();
            app.curve_ui.current = None;
        }
    }
}

/// Ease buttons for the chosen keys and a box with the two timing handles of the first one.
fn ease_box(app: &mut ToonApp, ui: &mut Ui, id: PegId, peg: &Peg) {
    let chosen: Vec<u32> = app.curve_ui.chosen.iter().copied().collect();
    let n = chosen.len();
    ui.label(if n == 0 { tl!("Choose keys to ease them").to_string() } else { tlf!("Ease for {n} chosen keys", n = n) });
    let mut set = None;
    ui.add_enabled_ui(n > 0, |ui| {
        ui.horizontal_wrapped(|ui| {
            for e in Ease::PRESETS {
                if ui.button(crate::pegs::ease_label(e)).clicked() {
                    set = Some(e);
                }
            }
        });
    });
    // the first chosen key's curve (handles for presets come from the preset)
    let first = chosen.first().and_then(|f| peg.keys.iter().find(|k| k.frame == *f)).map(|k| k.ease);
    let stored = first.and_then(|e| e.handles()).unwrap_or((0.42, 0.0, 0.58, 1.0));
    let mut h = app.curve_ui.handle_drag.map(|d| d.value).unwrap_or(stored);
    let (_, rect) = ui.allocate_space(vec2(150.0, 110.0));
    let resp = ui.interact(rect, handles_id(), if n > 0 { Sense::click_and_drag() } else { Sense::hover() });
    let inner = rect.shrink2(vec2(8.0, 22.0));
    let to = |x: f32, y: f32| pos2(inner.left() + x * inner.width(), inner.bottom() - y * inner.height());
    let from = |p: Pos2| ((p.x - inner.left()) / inner.width().max(1.0), (inner.bottom() - p.y) / inner.height().max(1.0));
    if n > 0 {
        if resp.drag_started()
            && let Some(p) = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos())
        {
            resp.request_focus();
            let d1 = to(h.0, h.1).distance(p);
            let d2 = to(h.2, h.3).distance(p);
            app.curve_ui.handle_drag = Some(HandleDrag { handle: if d1 <= d2 { 0 } else { 1 }, value: h });
        }
        if let Some(d) = app.curve_ui.handle_drag.as_mut()
            && resp.dragged()
            && let Some(p) = resp.interact_pointer_pos()
        {
            let (x, y) = from(p);
            if d.handle == 0 {
                (d.value.0, d.value.1) = (x, y);
            } else {
                (d.value.2, d.value.3) = (x, y);
            }
            // clamp the preview the way the key will be clamped
            if let Some(c) = (Ease::Curve { x1: d.value.0, y1: d.value.1, x2: d.value.2, y2: d.value.3 }).handles() {
                d.value = c;
            }
            h = d.value;
        }
        if app.curve_ui.handle_drag.is_some() && ui.input_mut(|i| i.consume_key(Modifiers::NONE, egui::Key::Escape)) {
            app.curve_ui.handle_drag = None;
            h = stored;
        }
        if resp.drag_stopped()
            && let Some(d) = app.curve_ui.handle_drag.take()
        {
            set = Some(Ease::Curve { x1: d.value.0, y1: d.value.1, x2: d.value.2, y2: d.value.3 });
        } else if app.curve_ui.handle_drag.is_some() && !resp.dragged() && !ui.input(|i| i.pointer.any_down()) {
            app.curve_ui.handle_drag = None;
        }
    }
    let visuals = ui.visuals().clone();
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, visuals.extreme_bg_color);
    painter.rect_stroke(inner, 0.0, Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color), egui::StrokeKind::Inside);
    let accent = visuals.selection.bg_fill.gamma_multiply(1.6);
    let hold = first == Some(Ease::Hold) && app.curve_ui.handle_drag.is_none();
    let curve = Ease::Curve { x1: h.0, y1: h.1, x2: h.2, y2: h.3 };
    let pts: Vec<Pos2> = (0..=40)
        .map(|i| {
            let t = i as f32 / 40.0;
            let v = if hold { if t >= 1.0 { 1.0 } else { 0.0 } } else { curve.apply(t) };
            to(t, v)
        })
        .collect();
    painter.add(egui::Shape::line(pts, Stroke::new(2.0, accent)));
    if n > 0 && !hold {
        let weak = Stroke::new(1.0, visuals.weak_text_color());
        painter.line_segment([to(0.0, 0.0), to(h.0, h.1)], weak);
        painter.line_segment([to(1.0, 1.0), to(h.2, h.3)], weak);
        painter.circle_filled(to(h.0, h.1), 4.5, Color32::WHITE);
        painter.circle_filled(to(h.2, h.3), 4.5, Color32::WHITE);
    }
    painter.rect_stroke(rect, 3.0, if resp.has_focus() { visuals.selection.stroke } else { Stroke::NONE }, egui::StrokeKind::Inside);
    let label =
        tlf!("Ease handles: {a}, {b} and {c}, {d}", a = format!("{:.2}", h.0), b = format!("{:.2}", h.1), c = format!("{:.2}", h.2), d = format!("{:.2}", h.3));
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, n > 0, label.clone()));
    // typed handle values, applied to every chosen key
    if n > 0 {
        let mut typed = app.curve_ui.typed.unwrap_or(h);
        let mut changed = false;
        let mut dragging = false;
        egui::Grid::new("curve-handles").num_columns(2).spacing(vec2(6.0, 3.0)).show(ui, |ui| {
            for (label, v) in [
                (tl!("Handle 1 time"), &mut typed.0),
                (tl!("Handle 1 value"), &mut typed.1),
                (tl!("Handle 2 time"), &mut typed.2),
                (tl!("Handle 2 value"), &mut typed.3),
            ] {
                let l = ui.label(label);
                let r = ui.add(egui::DragValue::new(v).speed(0.01).max_decimals(2)).labelled_by(l.id);
                // a pointer drag on the number is applied once, on release
                changed |= (r.changed() && !r.dragged()) || r.drag_stopped();
                dragging |= r.dragged() && !r.drag_stopped();
                ui.end_row();
            }
        });
        app.curve_ui.typed = (dragging && !changed).then_some(typed);
        if changed && typed != stored {
            set = Some(Ease::Curve { x1: typed.0, y1: typed.1, x2: typed.2, y2: typed.3 });
        }
    }
    if let Some(ease) = set
        && n > 0
    {
        app.run(Command::SetPegEases { peg: id, frames: chosen, ease });
    }
}
