//! The Drawing, Play and Scene menus, and the Scene Settings window: the shot's size, frame
//! rate and length in one place, with the play range and speed. Each changed value is applied as
//! its own command (one undo step each); the canvas is resized around its centre (Canvas Size…
//! keeps another part in place).

use tooncraft_engine::Command;

use crate::app::ToonApp;
use crate::timeline_extras::SPEEDS;

/// The Scene Settings window's values while it is open.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneForm {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub frames: u32,
}

impl SceneForm {
    pub fn of(app: &ToonApp) -> Self {
        let p = &app.engine.project;
        Self { width: p.width(), height: p.height(), fps: p.fps(), frames: p.length() }
    }

    /// The commands that make the shot match the form (none for values that did not change).
    /// Values are clamped to the same ranges the commands accept.
    pub fn commands(&self, now: &SceneForm) -> Vec<Command> {
        let mut out = Vec::new();
        let (w, h) = (self.width.clamp(16, tooncraft_raster::MAX_SIDE), self.height.clamp(16, tooncraft_raster::MAX_SIDE));
        if (w, h) != (now.width, now.height) {
            out.push(Command::ResizeCanvas { width: w, height: h, anchor: (0.5, 0.5) });
        }
        let fps = self.fps.clamp(tooncraft_doc::MIN_FPS, tooncraft_doc::MAX_FPS);
        if fps != now.fps {
            out.push(Command::SetFps { fps });
        }
        let frames = self.frames.clamp(1, tooncraft_doc::MAX_FRAMES);
        if frames != now.frames {
            out.push(Command::SetLength { frames });
        }
        out
    }
}

/// Open the Scene Settings window with the shot's values.
pub fn open_settings(app: &mut ToonApp) {
    app.scene_form = Some(SceneForm::of(app));
}

/// The Drawing menu: make, step through and swap drawings on the current layer.
pub fn drawing_menu(app: &mut ToonApp, ui: &mut egui::Ui) {
    let (layer, frame) = (app.layer, app.frame);
    if ui.button(tl!("New Drawing")).clicked() {
        app.run(Command::NewDrawing { layer, frame });
    }
    if ui.button(tl!("Duplicate Drawing")).clicked() {
        app.run(Command::DuplicateDrawing { layer, frame });
    }
    ui.menu_button(tl!("Swap Drawing"), |ui| crate::context_menus::swap_grid(app, ui, layer, frame));
    ui.separator();
    if ui.button(tl!("Previous Drawing")).clicked() {
        app.step_drawing(false);
    }
    if ui.button(tl!("Next Drawing")).clicked() {
        app.step_drawing(true);
    }
    if ui.button(tl!("Previous Key Drawing")).clicked() {
        app.step_key(false);
    }
    if ui.button(tl!("Next Key Drawing")).clicked() {
        app.step_key(true);
    }
    ui.separator();
    if ui.button(tl!("Remove Unused Drawings")).on_hover_text(tl!("Drawings no frame shows")).clicked() {
        app.run(Command::PurgeUnused);
    }
}

/// The Play menu: play and stop, step, loop, ping-pong, the selection and the speed.
pub fn play_menu(app: &mut ToonApp, ui: &mut egui::Ui) {
    let play = if app.playback.playing { tl!("Stop") } else { tl!("Play") };
    if ui.button(play).clicked() {
        app.toggle_play();
    }
    if ui.add_enabled(app.has_range(), egui::Button::new(tl!("Play Selection"))).clicked() {
        crate::timeline_extras::play_selection(app);
    }
    ui.separator();
    let last = app.engine.project.length().saturating_sub(1) as i64;
    let frame = app.frame as i64;
    for (label, to) in [(tl!("First Frame"), 0), (tl!("Previous Frame"), frame - 1), (tl!("Next Frame"), frame + 1), (tl!("Last Frame"), last)] {
        if ui.button(label).clicked() {
            app.playback.stop();
            app.go_to(to);
        }
    }
    ui.separator();
    ui.checkbox(&mut app.playback.looping, tl!("Loop"));
    ui.checkbox(&mut app.playback.ping_pong, tl!("Ping-pong"));
    ui.menu_button(tl!("Speed"), |ui| {
        let speed = app.playback.speed();
        for v in SPEEDS {
            if ui.radio(speed_is(speed, v), format!("{:.0} %", v * 100.0)).clicked() {
                let (now, fps) = (app.now(), app.engine.project.fps());
                app.playback.set_speed(now, fps, v);
                if !speed_is(v, 1.0) {
                    app.sound.stop();
                }
            }
        }
    });
}

fn speed_is(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// The Scene menu.
pub fn scene_menu(app: &mut ToonApp, ui: &mut egui::Ui) {
    if ui.button(tl!("Scene Settings…")).clicked() {
        open_settings(app);
        ui.close();
    }
    if ui.button(tl!("Canvas Size…")).on_hover_text(tl!("Resize keeping any part of the picture in place, or crop to the camera")).clicked() {
        app.canvas_ui.resize = Some((app.engine.project.width(), app.engine.project.height(), (1, 1)));
        ui.close();
    }
}

/// The Scene Settings window.
pub fn window(app: &mut ToonApp, ctx: &egui::Context) {
    let Some(mut form) = app.scene_form.take() else { return };
    let mut open = true;
    let mut apply = false;
    let mut close = false;
    egui::Window::new(tl!("Scene Settings")).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open).show(
        ctx,
        |ui| {
            ui.label(egui::RichText::new(tl!("Size")).strong());
            crate::scene_presets::picker(&mut app.scene_presets, ui, "scene-settings", &mut form.width, &mut form.height, Some(&mut form.fps));
            egui::Grid::new("scene-settings").num_columns(2).spacing(egui::vec2(10.0, 6.0)).show(ui, |ui| {
                let l = ui.label(tl!("Width"));
                ui.add(egui::DragValue::new(&mut form.width).range(16..=tooncraft_raster::MAX_SIDE).suffix(tl!(" px"))).labelled_by(l.id);
                ui.end_row();
                let l = ui.label(tl!("Height"));
                ui.add(egui::DragValue::new(&mut form.height).range(16..=tooncraft_raster::MAX_SIDE).suffix(tl!(" px"))).labelled_by(l.id);
                ui.end_row();
                let l = ui.label(tl!("Frame rate"));
                ui.add(egui::DragValue::new(&mut form.fps).range(tooncraft_doc::MIN_FPS..=tooncraft_doc::MAX_FPS).suffix(tl!(" fps"))).labelled_by(l.id);
                ui.end_row();
                let l = ui.label(tl!("Length"));
                ui.add(egui::DragValue::new(&mut form.frames).range(1..=tooncraft_doc::MAX_FRAMES).suffix(tl!(" frames"))).labelled_by(l.id);
                ui.end_row();
                ui.label(tl!("Duration"));
                let secs = form.frames as f64 / form.fps.max(1) as f64;
                ui.label(tlf!("{seconds} s", seconds = format!("{secs:.2}")));
                ui.end_row();
            });
            ui.separator();
            ui.label(egui::RichText::new(tl!("Playback")).strong());
            ui.horizontal(|ui| {
                ui.checkbox(&mut app.playback.looping, tl!("Loop"));
                ui.checkbox(&mut app.playback.ping_pong, tl!("Ping-pong"));
                ui.separator();
                let l = ui.label(tl!("Speed"));
                let speed = app.playback.speed();
                egui::ComboBox::from_id_salt("scene-speed")
                    .selected_text(format!("{:.0} %", speed * 100.0))
                    .show_ui(ui, |ui| {
                        for v in SPEEDS {
                            if ui.selectable_label(speed_is(speed, v), format!("{:.0} %", v * 100.0)).clicked() {
                                let (now, fps) = (app.now(), app.engine.project.fps());
                                app.playback.set_speed(now, fps, v);
                                if !speed_is(v, 1.0) {
                                    app.sound.stop();
                                }
                            }
                        }
                    })
                    .response
                    .labelled_by(l.id);
            });
            ui.label(
                egui::RichText::new(tl!("The size changes around the centre; Canvas Size… keeps another part in place. Each change is one undo step.")).weak(),
            );
            ui.horizontal(|ui| {
                if ui.button(tl!("Apply")).clicked() {
                    apply = true;
                }
                if ui.button(tl!("Close")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    close = true;
                }
            });
        },
    );
    if apply {
        let now = SceneForm::of(app);
        for cmd in form.commands(&now) {
            app.run(cmd);
        }
        app.clamp_selection();
        form = SceneForm::of(app);
    }
    if open && !close {
        app.scene_form = Some(form);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_changed_values_become_commands_and_hostile_ones_are_clamped() {
        let now = SceneForm { width: 1920, height: 1080, fps: 24, frames: 48 };
        assert!(now.commands(&now).is_empty());
        let cmds = SceneForm { fps: 12, ..now.clone() }.commands(&now);
        assert!(matches!(cmds.as_slice(), [Command::SetFps { fps: 12 }]));
        let cmds = SceneForm { width: u32::MAX, height: 0, fps: 0, frames: u32::MAX }.commands(&now);
        assert_eq!(cmds.len(), 3);
        for c in &cmds {
            match c {
                Command::ResizeCanvas { width, height, .. } => {
                    assert_eq!((*width, *height), (tooncraft_raster::MAX_SIDE, 16));
                }
                Command::SetFps { fps } => assert_eq!(*fps, tooncraft_doc::MIN_FPS),
                Command::SetLength { frames } => assert_eq!(*frames, tooncraft_doc::MAX_FRAMES),
                other => panic!("unexpected {other:?}"),
            }
        }
    }
}
