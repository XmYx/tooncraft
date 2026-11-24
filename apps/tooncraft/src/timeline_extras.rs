//! Timeline polish: timecode, timeline zoom, ping-pong playback, and frame markers (a note on a
//! frame, shown as a flag on the ruler, listed in the Timing panel, reachable from the keyboard).

use egui::{Color32, Rect, Ui, pos2};
use tooncraft_doc::timecode;
use tooncraft_engine::Command;

use crate::app::ToonApp;

/// Smallest and largest timeline zoom (1 is the normal cell width).
pub const ZOOM_RANGE: (f32, f32) = (0.4, 4.0);

/// Timeline state that is not part of the project.
pub struct State {
    pub zoom: f32,
    /// Show the timecode instead of the frame number.
    pub timecode: bool,
    /// The note being typed, and the frame it belongs to.
    pub note: String,
    pub note_frame: Option<u32>,
    /// The drawing name being typed, and the (layer, frame) it was loaded for.
    pub name: String,
    pub name_for: Option<(usize, u32)>,
    /// The sheet runs down the page: layers as columns, frames as rows (paper exposure sheet).
    pub vertical: bool,
    /// Frames before and after the current one for "Play Around".
    pub around: u32,
    /// The in and out frames playback loops between (inclusive), when set.
    pub range: Option<(u32, u32)>,
}

impl Default for State {
    fn default() -> Self {
        Self { zoom: 1.0, timecode: false, note: String::new(), note_frame: None, name: String::new(), name_for: None, vertical: false, around: 3, range: None }
    }
}

impl State {
    pub fn zoom_by(&mut self, factor: f32) {
        let f = if factor.is_finite() { factor } else { 1.0 };
        self.zoom = (self.zoom * f).clamp(ZOOM_RANGE.0, ZOOM_RANGE.1);
    }
}

/// The frame readout for the transport: `12 / 48`, or the timecode.
pub fn position_text(app: &ToonApp) -> String {
    let (len, fps) = (app.engine.project.length(), app.engine.project.fps());
    if app.tl.timecode {
        format!("{}  {}", timecode(app.frame, fps), tlf!("of {total}", total = timecode(len.saturating_sub(1), fps)))
    } else {
        format!("{:>4} / {}", app.frame + 1, len)
    }
}

/// Speeds offered for playback.
pub const SPEEDS: [f64; 6] = [0.25, 0.5, 0.75, 1.0, 1.5, 2.0];
/// The most frames either side for Play Around.
pub const MAX_AROUND: u32 = 48;

/// Start playback (and the sound, at real-time speed) of `start..end` from `from`.
pub fn play_range(app: &mut ToonApp, from: u32, start: u32, end: u32) {
    let now = app.now();
    app.playback.play(now, from, start, end);
    if (app.playback.speed() - 1.0).abs() < 1e-6 {
        let fps = app.engine.project.fps();
        let offset = app.sound_offset();
        app.sound.play(from as f64, offset, fps);
    }
}

/// Put the in (or out) point on the current frame; the other end stays, and the range never
/// turns inside out.
pub fn set_in_out(app: &mut ToonApp, start: bool) {
    let last = app.engine.project.length().saturating_sub(1);
    let (a, b) = app.tl.range.unwrap_or((0, last));
    let f = app.frame.min(last);
    app.tl.range = Some(if start { (f, b.max(f)) } else { (a.min(f), f) });
}

/// The frames normal playback covers: the in and out range, or the whole shot.
pub fn play_span(app: &ToonApp) -> (u32, u32) {
    let len = app.engine.project.length();
    match app.tl.range {
        Some((a, b)) if a <= b && b < len => (a, b + 1),
        _ => (0, len),
    }
}

/// Loop the frames around the current one (`around` either side), starting before it.
pub fn play_around(app: &mut ToonApp) {
    let n = app.tl.around.clamp(1, MAX_AROUND);
    let len = app.engine.project.length();
    let start = app.frame.saturating_sub(n);
    let end = (app.frame + n + 1).min(len);
    app.playback.looping = true;
    play_range(app, start, start, end);
}

/// Play the selected frames only (looping, or ping-pong, as set), from the first of them.
pub fn play_selection(app: &mut ToonApp) {
    if !app.has_range() {
        return;
    }
    let (_, _, f0, f1) = app.selection();
    let end = (f1 + 1).min(app.engine.project.length());
    play_range(app, f0, f0, end);
}

/// Zoom from the wheel or a pinch over the timeline grid.
pub fn wheel_zoom(app: &mut ToonApp, ui: &Ui) {
    let z = ui.input(|i| i.zoom_delta());
    if z != 1.0 {
        app.tl.zoom_by(z);
    }
}

/// Draw the markers' flags on the ruler (`cell_w` pixels per frame from `first`), and say which
/// marker the pointer is on.
pub fn paint_flags(app: &ToonApp, painter: &egui::Painter, ruler: Rect, frames_left: f32, first: u32, cols: u32, cell_w: f32, colour: Color32) {
    for (f, _) in app.engine.project.shown_markers().filter(|(f, _)| *f >= first && *f < first + cols) {
        let x = frames_left + (f - first) as f32 * cell_w;
        let pole = [pos2(x + 1.0, ruler.top() + 1.0), pos2(x + 1.0, ruler.bottom() - 1.0)];
        painter.line_segment(pole, egui::Stroke::new(1.5, colour));
        painter.add(egui::Shape::convex_polygon(
            vec![pos2(x + 1.0, ruler.top() + 1.0), pos2(x + 8.0, ruler.top() + 4.5), pos2(x + 1.0, ruler.top() + 8.0)],
            colour,
            egui::Stroke::NONE,
        ));
    }
}

/// The note on `frame`, if any.
pub fn note_at(app: &ToonApp, frame: u32) -> Option<String> {
    app.engine.project.markers.get(&frame).filter(|_| frame < app.engine.project.length()).cloned()
}

/// Put the playhead on the next or previous marker.
pub fn jump(app: &mut ToonApp, forward: bool) {
    let p = &app.engine.project;
    let target = if forward { p.next_marker(app.frame) } else { p.prev_marker(app.frame) };
    match target {
        Some(f) => {
            app.go_to(f as i64);
            let text = note_at(app, f).unwrap_or_default();
            app.set_status(tlf!("Marker at frame {n}: {text}", n = f + 1, text = text));
        }
        None => app.set_status(if forward { tl!("No later marker").to_string() } else { tl!("No earlier marker").to_string() }),
    }
}

/// Add a marker on the current frame (opening the Timing panel to type its note), or remove the
/// one that is there.
pub fn toggle_here(app: &mut ToonApp) {
    let frame = app.frame;
    if app.engine.project.markers.contains_key(&frame) {
        if app.run(Command::RemoveMarker { frame }) {
            app.set_status(tlf!("Marker removed from frame {n}", n = frame + 1));
        }
    } else if app.run(Command::SetMarker { frame, text: tl!("Marker").to_string() }) {
        app.tl.note_frame = None; // reload the note field with the new marker
        app.workspace.raise(crate::dock::Pane::Timing);
        app.set_status(tlf!("Marker added at frame {n}", n = frame + 1));
    }
}

/// The Markers section of the Timing panel.
pub fn section(app: &mut ToonApp, ui: &mut Ui) {
    ui.separator();
    ui.strong(tl!("Markers"));
    let frame = app.frame;
    if app.tl.note_frame != Some(frame) {
        app.tl.note = note_at(app, frame).unwrap_or_default();
        app.tl.note_frame = Some(frame);
    }
    let here = note_at(app, frame).is_some();
    ui.horizontal_wrapped(|ui| {
        let l = ui.label(tlf!("Note for frame {n}", n = frame + 1));
        let r = ui.add(egui::TextEdit::singleline(&mut app.tl.note).desired_width(120.0).char_limit(tooncraft_doc::MAX_MARKER_TEXT)).labelled_by(l.id);
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (enter || ui.button(if here { tl!("Update Marker") } else { tl!("Add Marker") }).clicked()) && !app.tl.note.trim().is_empty() {
            let text = app.tl.note.clone();
            app.run(Command::SetMarker { frame, text });
        }
        if ui.add_enabled(here, egui::Button::new(tl!("Remove Marker"))).clicked() {
            app.run(Command::RemoveMarker { frame });
            app.tl.note_frame = None;
        }
    });
    ui.horizontal(|ui| {
        if ui.button(tl!("Previous Marker")).clicked() {
            jump(app, false);
        }
        if ui.button(tl!("Next Marker")).clicked() {
            jump(app, true);
        }
    });
    let fps = app.engine.project.fps();
    let rows: Vec<(u32, String)> = app.engine.project.shown_markers().map(|(f, t)| (f, t.to_string())).collect();
    if rows.is_empty() {
        ui.weak(tl!("No markers yet"));
    }
    egui::ScrollArea::vertical().id_salt("markers-scroll").max_height(160.0).show(ui, |ui| {
        for (f, text) in rows {
            let label = tlf!("Frame {n}, {tc}: {text}", n = f + 1, tc = timecode(f, fps), text = text);
            if ui.selectable_label(f == frame, label).clicked() {
                app.go_to(f as i64);
            }
        }
    });
}

/// The Drawing names section of the Timing panel: name the drawing at this frame, and show any
/// named drawing here (a substitution) by name.
pub fn names(app: &mut ToonApp, ui: &mut Ui) {
    ui.separator();
    ui.strong(tl!("Drawing names"));
    let (layer, frame) = (app.layer, app.frame);
    let current = app.engine.project.drawing_at(layer, frame);
    let existing = current.and_then(|d| app.engine.project.drawing_names.get(&d).cloned()).unwrap_or_default();
    if app.tl.name_for != Some((layer, frame)) {
        app.tl.name = existing.clone();
        app.tl.name_for = Some((layer, frame));
    }
    ui.add_enabled_ui(current.is_some(), |ui| {
        ui.horizontal_wrapped(|ui| {
            let l = ui.label(tl!("Name of this drawing"));
            let r = ui.add(egui::TextEdit::singleline(&mut app.tl.name).desired_width(110.0).char_limit(tooncraft_doc::MAX_NAME)).labelled_by(l.id);
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (enter || ui.button(tl!("Set Name")).clicked()) && app.tl.name.trim() != existing {
                let name = app.tl.name.clone();
                if app.run(Command::SetDrawingName { layer, frame, name }) {
                    app.tl.name_for = None;
                }
            }
        });
    });
    let named: Vec<String> = {
        let p = &app.engine.project;
        let mut v: Vec<String> = p.drawing_names.iter().filter(|(d, _)| p.drawing_ids().any(|x| x == **d)).map(|(_, n)| n.clone()).collect();
        v.sort_by_key(|n| n.to_lowercase());
        v
    };
    if named.is_empty() {
        ui.weak(tl!("No drawings have names yet"));
    }
    ui.horizontal_wrapped(|ui| {
        for n in named {
            if ui
                .button(tlf!("Show {name} here", name = n))
                .on_hover_text(tl!("Show this drawing at the current frame, leaving the frames after it as they are"))
                .clicked()
            {
                app.run(Command::ExposeNamed { layer, frame, name: n });
            }
        }
    });
}

/// Items for the Frame menu.
pub fn menu(app: &mut ToonApp, ui: &mut Ui) {
    ui.separator();
    let here = note_at(app, app.frame).is_some();
    if ui.button(if here { tl!("Remove Marker Here") } else { tl!("Add Marker Here") }).clicked() {
        toggle_here(app);
    }
    if ui.button(tl!("Next Marker")).clicked() {
        jump(app, true);
    }
    if ui.button(tl!("Previous Marker")).clicked() {
        jump(app, false);
    }
}
