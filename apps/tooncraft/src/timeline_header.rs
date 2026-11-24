//! The Timeline's header row: the icon transport (first, previous drawing, previous frame,
//! play, next frame, next drawing, last), loop and play-around, the playback speed, and inline
//! fields for the frame, the in and out points, the frame rate and the current drawing's hold.
//! Everything else the timeline does is in its `⋯` menu, so the header stays one row.

use egui::Ui;
use tooncraft_engine::Command;

use crate::app::ToonApp;
use crate::icons::{Icon, IconButton};

/// The header: one row, wrapping in a narrow timeline instead of clipping.
pub fn show(app: &mut ToonApp, ui: &mut Ui) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        transport(app, ui);
        sep(ui);
        speed(app, ui);
        sep(ui);
        fields(app, ui);
        sep(ui);
        more(app, ui);
    });
}

fn sep(ui: &mut Ui) {
    ui.add(egui::Separator::default().vertical().spacing(6.0));
}

fn button(app: &ToonApp, ui: &mut Ui, icon: Icon, label: &str, key: &str) -> egui::Response {
    let b = IconButton::new(icon, label);
    let b = if key.is_empty() { b } else { b.shortcut(app.keys.text(key)) };
    ui.add(b)
}

fn transport(app: &mut ToonApp, ui: &mut Ui) {
    if button(app, ui, Icon::First, tl!("First frame"), "frame.first").clicked() {
        app.playback.stop();
        app.frame = 0;
    }
    if button(app, ui, Icon::PrevDrawing, tl!("Previous drawing"), "frame.prev_drawing").clicked() {
        app.step_drawing(false);
    }
    if button(app, ui, Icon::PrevFrame, tl!("Previous frame"), "frame.prev").clicked() {
        app.go_to(app.frame as i64 - 1);
    }
    let playing = app.playback.playing;
    let (icon, label) = if playing { (Icon::Pause, tl!("Stop")) } else { (Icon::Play, tl!("Play")) };
    if ui.add(IconButton::new(icon, label).shortcut(app.keys.text("frame.play")).active(playing)).clicked() {
        app.toggle_play();
    }
    if button(app, ui, Icon::NextFrame, tl!("Next frame"), "frame.next").clicked() {
        app.go_to(app.frame as i64 + 1);
    }
    if button(app, ui, Icon::NextDrawing, tl!("Next drawing"), "frame.next_drawing").clicked() {
        app.step_drawing(true);
    }
    if button(app, ui, Icon::Last, tl!("Last frame"), "frame.last").clicked() {
        app.playback.stop();
        app.frame = app.engine.project.length().saturating_sub(1);
    }
    sep(ui);
    let looping = app.playback.looping;
    if ui.add(IconButton::new(Icon::Loop, tl!("Loop playback")).active(looping)).clicked() {
        app.playback.looping = !looping;
    }
    if button(app, ui, Icon::PlayAround, tl!("Play Around"), "").clicked() {
        crate::timeline_extras::play_around(app);
    }
    let l = ui.label(tl!("Around"));
    ui.add(egui::DragValue::new(&mut app.tl.around).range(1..=crate::timeline_extras::MAX_AROUND).prefix("±"))
        .labelled_by(l.id)
        .on_hover_text(tl!("Frames either side for Play Around"));
}

fn speed(app: &mut ToonApp, ui: &mut Ui) {
    let l = ui.label(tl!("Speed"));
    let speed = app.playback.speed();
    let pct = |v: f64| format!("{:.0} %", v * 100.0);
    egui::ComboBox::from_id_salt("play-speed")
        .selected_text(pct(speed))
        .width(64.0)
        .show_ui(ui, |ui| {
            for v in crate::timeline_extras::SPEEDS {
                if ui.selectable_label((speed - v).abs() < 1e-6, pct(v)).clicked() {
                    let (now, fps) = (app.now(), app.engine.project.fps());
                    app.playback.set_speed(now, fps, v);
                    if (v - 1.0).abs() > 1e-6 {
                        // the soundtrack plays only in real time
                        app.sound.stop();
                    }
                }
            }
        })
        .response
        .labelled_by(l.id)
        .on_hover_text(tl!("Playback speed; the sound plays only at 100 %"));
}

/// The loop range after typing `start` or `end` (1-based) into the In or Out field: the other
/// end gives way so the range never turns inside out.
pub fn typed_range(len: u32, start: u32, end: u32, start_changed: bool) -> Option<(u32, u32)> {
    let last = len.checked_sub(1)?;
    let top = last.saturating_add(1);
    let (a, b) = (start.clamp(1, top) - 1, end.clamp(1, top) - 1);
    Some(if start_changed { (a, b.max(a)) } else { (a.min(b), b) })
}

fn fields(app: &mut ToonApp, ui: &mut Ui) {
    let len = app.engine.project.length();
    // the current frame, 1-based: typing a number goes there
    let l = ui.label(tl!("Frame"));
    if app.tl.timecode {
        ui.label(egui::RichText::new(crate::timeline_extras::position_text(app)).monospace()).labelled_by(l.id);
    } else {
        let mut frame = app.frame + 1;
        if ui.add(egui::DragValue::new(&mut frame).range(1..=len.max(1))).labelled_by(l.id).on_hover_text(tl!("Type a frame number to go there")).changed() {
            app.playback.stop();
            app.frame = frame.clamp(1, len.max(1)) - 1;
        }
        ui.label(egui::RichText::new(format!("/ {len}")).weak());
    }
    sep(ui);
    // playback start and end (the in and out points; the whole shot when not set)
    let (s0, e0) = app.tl.range.unwrap_or((0, len.saturating_sub(1)));
    let (mut s, mut e) = (s0 + 1, e0 + 1);
    let l = ui.label(tl!("In"));
    let rs = ui.add(egui::DragValue::new(&mut s).range(1..=len.max(1))).labelled_by(l.id).on_hover_text(tl!("Playback starts here"));
    let l = ui.label(tl!("Out"));
    let re = ui.add(egui::DragValue::new(&mut e).range(1..=len.max(1))).labelled_by(l.id).on_hover_text(tl!("Playback loops back after this frame"));
    if rs.changed() || re.changed() {
        app.tl.range = typed_range(len, s, e, rs.changed());
    }
    sep(ui);
    let mut fps = app.engine.project.fps();
    let r = ui.add(egui::DragValue::new(&mut fps).range(tooncraft_doc::MIN_FPS..=tooncraft_doc::MAX_FPS));
    let l = ui.label(tl!("fps"));
    if r.labelled_by(l.id).on_hover_text(tl!("Frames per second of the shot")).changed() {
        app.run(Command::SetFps { fps });
    }
    sep(ui);
    // the exposure length of the current drawing, editable (keys 1 to 9 in the sheet)
    let (layer, frame) = (app.layer, app.frame);
    let range = app.engine.project.layers.get(layer).and_then(|l| l.exposure_range(frame));
    let l = ui.label(tl!("Hold"));
    let (start, end) = range.unwrap_or((frame, frame + 1));
    let mut length = end - start;
    let resp = ui
        .add_enabled(range.is_some(), egui::DragValue::new(&mut length).range(1..=999).speed(0.1).suffix(tl!(" fr")))
        .labelled_by(l.id)
        .on_hover_text(tl!("How many frames this drawing is shown (1 to 9 in the sheet)"))
        .on_disabled_hover_text(tl!("No drawing on this frame to hold"));
    if resp.changed() && range.is_some() && length != end - start {
        app.run(Command::SetExposure { layer, frame, length });
    }
}

/// The `⋯` menu: the timeline's other commands and switches.
fn more(app: &mut ToonApp, ui: &mut Ui) {
    let resp = ui.add(IconButton::new(Icon::More, tl!("Timeline options")));
    // the timeline sits at the bottom of the window: the menu opens upward
    egui::Popup::menu(&resp).align(egui::RectAlign::TOP_END).show(|ui| menu(app, ui));
}

/// The timeline's other commands and switches, as menu items.
pub fn menu(app: &mut ToonApp, ui: &mut Ui) {
    let (layer, frame) = (app.layer, app.frame);
    let key = |app: &ToonApp, id: &str| app.keys.text(id);
    if ui.add(egui::Button::new(tl!("New")).shortcut_text(key(app, "frame.new_drawing"))).on_hover_text(tl!("New drawing")).clicked() {
        app.run(Command::NewDrawing { layer, frame });
    }
    if ui.add(egui::Button::new(tl!("Duplicate")).shortcut_text(key(app, "frame.duplicate"))).on_hover_text(tl!("Duplicate drawing")).clicked() {
        app.run(Command::DuplicateDrawing { layer, frame });
    }
    if ui.add(egui::Button::new(tl!("Insert")).shortcut_text(key(app, "frame.insert"))).on_hover_text(tl!("Insert frame")).clicked() {
        app.run(Command::InsertFrame { layer: Some(layer), frame });
    }
    if ui.add(egui::Button::new(tl!("Delete")).shortcut_text(key(app, "frame.delete"))).on_hover_text(tl!("Delete frame")).clicked() {
        app.run(Command::DeleteFrame { layer: Some(layer), frame });
    }
    if ui.add(egui::Button::new(tl!("Hold +")).shortcut_text(key(app, "frame.hold_more"))).on_hover_text(tl!("Show one frame longer")).clicked() {
        app.run(Command::ExtendHold { layer, frame });
    }
    if ui.add(egui::Button::new(tl!("Hold −")).shortcut_text(key(app, "frame.hold_less"))).on_hover_text(tl!("Show one frame shorter")).clicked() {
        app.run(Command::ShortenHold { layer, frame });
    }
    if ui.add(egui::Button::new(tl!("Clear")).shortcut_text(key(app, "frame.clear"))).on_hover_text(tl!("Clear frame")).clicked() {
        app.run(Command::ClearCell { layer, frame });
    }
    if ui.add(egui::Button::new(tl!("Pin")).shortcut_text(key(app, "view.pin"))).on_hover_text(tl!("Pin this frame to the light table")).clicked() {
        app.run(Command::TogglePin { layer, frame });
    }
    ui.separator();
    let pp = app.playback.ping_pong;
    if ui.selectable_label(pp, tl!("Ping-pong")).on_hover_text(tl!("Play forward, then backward, then forward again")).clicked() {
        app.playback.ping_pong = !pp;
    }
    let selected = app.has_range();
    if ui
        .add_enabled(selected, egui::Button::new(tl!("Play Selection")))
        .on_hover_text(tl!("Play only the selected frames"))
        .on_disabled_hover_text(tl!("Select a range of frames in the exposure sheet first"))
        .clicked()
    {
        crate::timeline_extras::play_selection(app);
    }
    if ui.button(tl!("Set In")).on_hover_text(tl!("Playback starts here")).clicked() {
        crate::timeline_extras::set_in_out(app, true);
    }
    if ui.button(tl!("Set Out")).on_hover_text(tl!("Playback loops back after this frame")).clicked() {
        crate::timeline_extras::set_in_out(app, false);
    }
    if ui.add_enabled(app.tl.range.is_some(), egui::Button::new(tl!("Clear In and Out"))).on_disabled_hover_text(tl!("No in and out range is set")).clicked() {
        app.tl.range = None;
    }
    ui.separator();
    ui.toggle_value(&mut app.tl.vertical, tl!("Vertical")).on_hover_text(tl!("Show the sheet down the page: layers as columns, frames as rows"));
    let tc = app.tl.timecode;
    if ui.selectable_label(tc, tl!("Timecode")).on_hover_text(tl!("Show the position as HH:MM:SS:FF instead of a frame number")).clicked() {
        app.tl.timecode = !tc;
    }
    if ui.selectable_label(app.thumbnails, tl!("Thumbnails")).on_hover_text(tl!("Show drawing thumbnails in the cells")).clicked() {
        app.thumbnails = !app.thumbnails;
    }
    if !app.engine.project.sounds.is_empty() {
        let on = !app.sound.muted;
        if ui.selectable_label(on, tl!("Sound")).on_hover_text(tl!("Play the soundtrack")).clicked() {
            app.sound.muted = on;
        }
    }
    if ui.add(egui::Button::new(tl!("Zoom In")).shortcut_text(key(app, "timeline.zoom_in"))).on_hover_text(tl!("Show fewer, wider frames")).clicked() {
        app.tl.zoom_by(crate::timeline_extras::ZOOM_STEP);
    }
    if ui
        .add(egui::Button::new(tl!("Zoom Out")).shortcut_text(key(app, "timeline.zoom_out")))
        .on_hover_text(tl!("Show more frames (or Ctrl and the wheel)"))
        .clicked()
    {
        app.tl.zoom_by(1.0 / crate::timeline_extras::ZOOM_STEP);
    }
    ui.separator();
    let len = app.engine.project.length();
    let mut frames = len;
    ui.horizontal(|ui| {
        let l = ui.label(tl!("Shot length"));
        let r = ui.add(egui::DragValue::new(&mut frames).range(1..=tooncraft_doc::MAX_FRAMES).suffix(tl!(" fr"))).labelled_by(l.id);
        if r.changed() && frames != len {
            app.run(Command::SetLength { frames });
            app.clamp_selection();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_in_or_out_never_turns_the_range_inside_out() {
        assert_eq!(typed_range(48, 5, 20, true), Some((4, 19)));
        assert_eq!(typed_range(48, 30, 20, true), Some((29, 29)));
        assert_eq!(typed_range(48, 30, 20, false), Some((19, 19)));
        assert_eq!(typed_range(48, 0, 900, false), Some((0, 47)));
        assert_eq!(typed_range(0, 1, 1, true), None);
        assert_eq!(typed_range(u32::MAX, u32::MAX, u32::MAX, true), Some((u32::MAX - 1, u32::MAX - 1)));
    }
}
