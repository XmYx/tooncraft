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

/// An icon button for the registry command `id`: its icon (or `icon`), its key in the tooltip,
/// on/off for toggles, disabled with the reason when it cannot run.
fn command(app: &mut ToonApp, ui: &mut Ui, id: &str, icon: Option<Icon>, label: &str) {
    let Some(c) = crate::commands::get(id) else { return };
    let Some(icon) = icon.or(c.icon) else { return };
    let mut b = IconButton::new(icon, label).shortcut(c.key_text(app)).disabled((c.enabled)(app).err());
    if let Some(on) = c.checked {
        b = b.active(on(app));
    }
    if ui.add(b).clicked() {
        let ctx = ui.ctx().clone();
        crate::commands::run(app, &ctx, c.id);
    }
}

fn transport(app: &mut ToonApp, ui: &mut Ui) {
    command(app, ui, "frame.first", None, tl!("First frame"));
    command(app, ui, "frame.prev_drawing", None, tl!("Previous drawing"));
    command(app, ui, "frame.prev", None, tl!("Previous frame"));
    let playing = app.playback.playing;
    let (icon, label) = if playing { (Icon::Pause, tl!("Stop")) } else { (Icon::Play, tl!("Play")) };
    command(app, ui, "frame.play", Some(icon), label);
    command(app, ui, "frame.next", None, tl!("Next frame"));
    command(app, ui, "frame.next_drawing", None, tl!("Next drawing"));
    command(app, ui, "frame.last", None, tl!("Last frame"));
    sep(ui);
    command(app, ui, "timeline.loop", None, tl!("Loop playback"));
    command(app, ui, "timeline.play_around", None, tl!("Play Around"));
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

/// The timeline's other commands and switches, as registry menu items: the frame commands, then
/// playback and display in a submenu each (at most 14 items a level).
pub fn menu(app: &mut ToonApp, ui: &mut Ui) {
    use crate::menus::{SEP, context};
    context(
        ui,
        app,
        &["frame.new_drawing", "frame.duplicate", "frame.insert", "frame.delete", SEP, "frame.hold_more", "frame.hold_less", "frame.clear", "view.pin"],
    );
    ui.separator();
    ui.menu_button(tl!("Playback"), |ui| {
        context(
            ui,
            app,
            &[
                "timeline.ping_pong",
                "timeline.play_selection",
                "timeline.record_motion",
                SEP,
                "timeline.set_in",
                "timeline.set_out",
                "timeline.clear_in_out",
                SEP,
                "timeline.sound",
            ],
        );
    });
    ui.menu_button(tl!("Display"), |ui| {
        context(ui, app, &["timeline.vertical", "timeline.timecode", "timeline.thumbnails", SEP, "timeline.zoom_in", "timeline.zoom_out"]);
    });
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
