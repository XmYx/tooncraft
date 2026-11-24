//! Asking before unsaved work is replaced: New Shot, Open… and Open Recent go through
//! [`request`], which acts at once when the shot is saved and otherwise asks "Save the shot
//! first?" with Save, Don't Save and Cancel. (Quitting has its own ask in `app.rs`.)

use std::path::PathBuf;

use crate::app::ToonApp;

/// Something that replaces the current shot.
#[derive(Clone, Debug, PartialEq)]
pub enum Replace {
    /// Show the New Shot dialog.
    NewShot,
    /// Pick a shot with the file chooser.
    Browse,
    /// Open this shot.
    Open(PathBuf),
}

/// Do `what` now if the shot has no unsaved changes; otherwise ask first.
pub fn request(app: &mut ToonApp, what: Replace) {
    if app.engine.is_dirty() {
        app.pending_replace = Some(what);
        app.replace_focus = true;
    } else {
        perform(app, what);
    }
}

fn perform(app: &mut ToonApp, what: Replace) {
    match what {
        Replace::NewShot => app.new_shot = Some(app.new_shot_defaults()),
        Replace::Browse => app.open_dialog(),
        Replace::Open(p) => app.open(&p),
    }
}

/// The "Unsaved changes" window while a replacement waits.
pub fn window(app: &mut ToonApp, ctx: &egui::Context) {
    let Some(what) = app.pending_replace.clone() else { return };
    #[derive(PartialEq)]
    enum Answer {
        Save,
        Discard,
        Cancel,
    }
    let mut answer = None;
    egui::Window::new(tl!("Unsaved changes"))
        .id(egui::Id::unique("unsaved-replace"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(tlf!("“{name}” has changes that are not saved. Save the shot first?", name = app.engine.project.name.clone()));
            ui.horizontal(|ui| {
                let save = ui.button(tl!("Save"));
                if save.clicked() {
                    answer = Some(Answer::Save);
                }
                if ui.button(tl!("Don’t Save")).clicked() {
                    answer = Some(Answer::Discard);
                }
                if ui.button(tl!("Cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    answer = Some(Answer::Cancel);
                }
                // the safe choice takes the keyboard focus once, when the question opens, even
                // from the canvas (whose keyboard pen would otherwise take Enter and the arrows)
                if std::mem::take(&mut app.replace_focus) {
                    save.request_focus();
                }
            });
        });
    match answer {
        Some(Answer::Save) => {
            // a cancelled or failed save keeps the question open: nothing is lost
            if app.save() {
                app.pending_replace = None;
                perform(app, what);
            }
        }
        Some(Answer::Discard) => {
            app.pending_replace = None;
            perform(app, what);
        }
        Some(Answer::Cancel) => app.pending_replace = None,
        None => {}
    }
}
