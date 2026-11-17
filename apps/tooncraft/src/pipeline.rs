//! Pipeline menu entries: BoardCraft boards in as a layout.

use tooncraft_engine::Command;

use crate::app::ToonApp;

/// File ▸ Import from Other Apps entries.
pub fn import_items(app: &mut ToonApp, ui: &mut egui::Ui) {
    if ui
        .button(tl!("BoardCraft Board or Animatic…"))
        .on_hover_text(tl!("Panels become a locked reference layer with their timings and camera moves"))
        .clicked()
        && let Some(f) = rfd::FileDialog::new().add_filter("BoardCraft", &["boardcraft"]).pick_file()
    {
        let frame = app.frame;
        if app.run(Command::ImportBoards { path: f.to_string_lossy().into_owned(), frame, camera: true }) {
            app.layer = 0;
            app.set_status(tlf!("Imported the board {file}", file = f.display()));
        }
    }
}
