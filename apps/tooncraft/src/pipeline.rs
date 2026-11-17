//! Pipeline menu entries: BoardCraft boards in as a layout; WebP (animated and sequences), TGA
//! and TIFF sequences and a PSD / PSB with one layer per frame out.

use tooncraft_engine::Command;
use tooncraft_interop::sequences::{self, Still};

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

/// File ▸ Export entries.
pub fn export_items(app: &mut ToonApp, ui: &mut egui::Ui) {
    ui.separator();
    if ui.button(tl!("Animated WebP…")).clicked() {
        export(app, "webp");
    }
    for (label, kind) in [(tl!("WebP Sequence…"), "webp-seq"), (tl!("TGA Sequence…"), "tga"), (tl!("TIFF Sequence…"), "tiff")] {
        if ui.button(label).clicked() {
            export(app, kind);
        }
    }
    if ui.button(tl!("PSD with a Layer per Frame…")).clicked() {
        export(app, "psd-frames");
    }
    if ui.button(tl!("PSB with a Layer per Frame (large documents)…")).clicked() {
        export(app, "psb-frames");
    }
}

fn export(app: &mut ToonApp, kind: &str) {
    let p = &app.engine.project;
    let (start, end, paper, name) = (0, p.length(), p.paper.export, p.name.clone());
    let result = match kind {
        "webp" => rfd::FileDialog::new().add_filter("WebP", &["webp"]).set_file_name(format!("{name}.webp")).save_file().map(|f| {
            sequences::animated_webp(p, start, end, paper)
                .and_then(|b| std::fs::write(&f, b).map_err(|e| tooncraft_interop::InteropError::Io(f.display().to_string(), e)))
                .map(|_| tlf!("Exported {file}", file = f.display()))
        }),
        "psd-frames" | "psb-frames" => {
            let ext = if kind == "psd-frames" { "psd" } else { "psb" };
            rfd::FileDialog::new().add_filter(ext.to_uppercase(), &[ext]).set_file_name(format!("{name}.{ext}")).save_file().map(|f| {
                sequences::frames_psd(p, start, end, paper, ext == "psb")
                    .and_then(|b| std::fs::write(&f, b).map_err(|e| tooncraft_interop::InteropError::Io(f.display().to_string(), e)))
                    .map(|_| tlf!("Exported {file}", file = f.display()))
            })
        }
        _ => {
            let format = match kind {
                "tga" => Still::Tga,
                "tiff" => Still::Tiff,
                _ => Still::WebP,
            };
            rfd::FileDialog::new().set_title(tl!("Folder for the frames")).pick_folder().map(|d| {
                sequences::write_sequence(p, &d, start, end, paper, format)
                    .map(|n| tlf!("Exported {count} frames to {folder}", count = n, folder = d.display()))
            })
        }
    };
    match result {
        Some(Ok(s)) => app.set_status(s),
        Some(Err(e)) => app.set_status(tlf!("Export failed: {error}", error = e)),
        None => {}
    }
}
