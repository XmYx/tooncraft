//! The bar under the menus: new, open and save; undo and redo; the workspace picker. Playback,
//! the frame, the in and out points and the frame rate are in the Timeline header. Every control
//! has a name for assistive technology; the same commands stay in the menus and on their keys.

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
            crate::unsaved::request(app, crate::unsaved::Replace::NewShot);
        }
        if ui.button(tl!("Open")).on_hover_text(tl!("Open a shot (Ctrl+O)")).clicked() {
            crate::unsaved::request(app, crate::unsaved::Replace::Browse);
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
        // the transport, frame, in and out, frame rate and length are in the Timeline header
    });
}
