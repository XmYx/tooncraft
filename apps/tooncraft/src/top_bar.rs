//! The bar under the menus: new, open and save; undo and redo; the workspace picker; play; the
//! current frame typed to go there; the playback start and end frames; the frame rate and the
//! shot length. Every control has a name for assistive technology; the same commands stay in the
//! menus and on their keys.

use tooncraft_engine::Command;

use crate::app::ToonApp;

/// The built-in workspaces offered in the picker: (launch name, label).
fn workspaces() -> [(&'static str, String); 4] {
    [
        ("drawing", tl!("Drawing").to_string()),
        ("timing", tl!("Timing").to_string()),
        ("ink-paint", tl!("Ink and Paint").to_string()),
        ("sheet", tl!("Exposure Sheet").to_string()),
    ]
}

pub fn show(app: &mut ToonApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if ui.button(tl!("New")).on_hover_text(tl!("A new shot (Ctrl+N)")).clicked() {
            app.new_shot = Some(app.new_shot_defaults());
        }
        if ui.button(tl!("Open")).on_hover_text(tl!("Open a shot (Ctrl+O)")).clicked() {
            app.open_dialog();
        }
        if ui.button(tl!("Save")).on_hover_text(tl!("Save the shot (Ctrl+S)")).clicked() {
            app.save();
        }
        ui.separator();
        if ui.add_enabled(app.engine.can_undo(), egui::Button::new(tl!("Undo step"))).on_hover_text(tl!("Undo (Ctrl+Z)")).clicked() {
            app.undo();
        }
        if ui.add_enabled(app.engine.can_redo(), egui::Button::new(tl!("Redo step"))).on_hover_text(tl!("Redo (Ctrl+Shift+Z)")).clicked() {
            app.redo();
        }
        ui.separator();
        let l = ui.label(tl!("Workspace"));
        egui::ComboBox::from_id_salt("top-workspace")
            .selected_text(tl!("Choose…"))
            .show_ui(ui, |ui| {
                for (id, name) in workspaces() {
                    if ui.selectable_label(false, name).clicked()
                        && let Some(layout) = crate::canvas_tools::layout_by_name(id)
                    {
                        app.workspace.layout = layout;
                        app.tl.vertical = id == "sheet";
                    }
                }
            })
            .response
            .labelled_by(l.id);
        ui.separator();
        let playing = app.playback.playing;
        if ui.selectable_label(playing, if playing { tl!("Stop") } else { tl!("Play") }).on_hover_text(tl!("Play or stop (Enter)")).clicked() {
            app.toggle_play();
        }
        let len = app.engine.project.length();
        // the current frame, 1-based: typing a number goes there
        let mut frame = app.frame + 1;
        let l = ui.label(tl!("Frame"));
        if ui.add(egui::DragValue::new(&mut frame).range(1..=len.max(1))).labelled_by(l.id).on_hover_text(tl!("Type a frame number to go there")).changed() {
            app.playback.stop();
            app.frame = frame.clamp(1, len.max(1)) - 1;
        }
        // playback start and end (the in and out points; the whole shot when not set)
        let (s0, e0) = app.tl.range.unwrap_or((0, len.saturating_sub(1)));
        let (mut s, mut e) = (s0 + 1, e0 + 1);
        let l = ui.label(tl!("Play from"));
        let rs = ui.add(egui::DragValue::new(&mut s).range(1..=len.max(1))).labelled_by(l.id).on_hover_text(tl!("Playback starts here"));
        let l = ui.label(tl!("Play to"));
        let re = ui.add(egui::DragValue::new(&mut e).range(1..=len.max(1))).labelled_by(l.id).on_hover_text(tl!("Playback loops back after this frame"));
        if rs.changed() || re.changed() {
            let (a, b) = (s.clamp(1, len.max(1)) - 1, e.clamp(1, len.max(1)) - 1);
            app.tl.range = Some(if rs.changed() { (a, b.max(a)) } else { (a.min(b), b) });
        }
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
    });
}
