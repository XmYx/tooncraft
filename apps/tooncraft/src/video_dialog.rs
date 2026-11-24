//! File ▸ Import Video: the pictures of an MP4 or QuickTime movie as a layer of drawings, one per
//! picture, timed by the movie. The window shows the movie's size, rate and length, and asks where
//! to start, whether to take every picture or every second or third, how many at most, whether the
//! layer goes under the drawing locked and faded to trace over, and whether the shot grows to fit.
//! The import is one undo step; a movie that cannot be read changes nothing.

use tooncraft_engine::{Command, VideoImport};

use crate::app::ToonApp;

#[derive(Clone, Debug, PartialEq)]
pub struct Dialog {
    pub path: Option<String>,
    /// What [`probe`] said about `path`: its track, or why it cannot be read.
    pub info: Option<Result<tooncraft_interop::video::VideoInfo, String>>,
    pub options: VideoImport,
}

impl Dialog {
    pub fn new(path: Option<String>) -> Self {
        let mut d = Self { path: None, info: None, options: VideoImport::default() };
        if let Some(p) = path {
            d.choose(p);
        }
        d
    }

    /// Use `path` and read its video track.
    pub fn choose(&mut self, path: String) {
        self.info = Some(probe(&path));
        self.path = Some(path);
    }

    /// The command these choices make, at the current frame.
    pub fn command(&self, app: &ToonApp) -> Option<Command> {
        let path = self.path.clone()?;
        Some(Command::ImportVideo { path, frame: app.frame, options: self.options.clone() })
    }
}

fn probe(path: &str) -> Result<tooncraft_interop::video::VideoInfo, String> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    tooncraft_interop::video::probe(&f).map_err(|e| e.to_string())
}

/// Open the window, asking for a movie first.
pub fn open(app: &mut ToonApp) {
    let file = rfd::FileDialog::new().add_filter(tl!("Video"), &["mp4", "m4v", "mov"]).pick_file();
    if let Some(f) = file {
        app.video_dialog = Some(Dialog::new(Some(f.to_string_lossy().into_owned())));
    }
}

/// Run the import; true when it went through.
pub fn run(app: &mut ToonApp, d: &Dialog) -> bool {
    let Some(cmd) = d.command(app) else {
        app.set_status(tl!("Choose a video first"));
        return false;
    };
    let before = app.engine.project.drawing_count();
    if !app.run(cmd) {
        return false;
    }
    app.layer = if d.options.reference { 0 } else { app.engine.project.layers.len().saturating_sub(1) };
    let n = app.engine.project.drawing_count().saturating_sub(before);
    app.set_status(tlf!("Imported {count} video frames", count = n));
    true
}

pub fn window(app: &mut ToonApp, ctx: &egui::Context) {
    let Some(mut d) = app.video_dialog.take() else { return };
    let mut keep = true;
    let mut go = false;
    egui::Window::new(tl!("Import Video")).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
        ui.horizontal(|ui| {
            let name = d.path.as_deref().and_then(|p| std::path::Path::new(p).file_name()).map(|n| n.to_string_lossy().into_owned());
            ui.label(name.unwrap_or_else(|| tl!("No video chosen").to_string()));
            if ui.button(tl!("Choose…")).clicked()
                && let Some(f) = rfd::FileDialog::new().add_filter(tl!("Video"), &["mp4", "m4v", "mov"]).pick_file()
            {
                d.choose(f.to_string_lossy().into_owned());
            }
        });
        match &d.info {
            Some(Ok(i)) => {
                let codec = if i.codec == "hevc" { "H.265" } else { "H.264" };
                ui.label(tlf!(
                    "{w}×{h}, {fps} frames a second, {secs} seconds ({codec})",
                    w = i.width,
                    h = i.height,
                    fps = format!("{:.2}", i.fps),
                    secs = format!("{:.2}", i.seconds),
                    codec = codec
                ));
            }
            Some(Err(e)) => {
                ui.colored_label(ui.visuals().error_fg_color, tlf!("Cannot read this video: {reason}", reason = e));
            }
            None => {}
        }
        ui.label(tlf!("From frame {frame}, scaled to fit the canvas", frame = app.frame + 1));
        egui::Grid::new("video-import-grid").num_columns(2).spacing(egui::vec2(10.0, 6.0)).show(ui, |ui| {
            let l = ui.label(tl!("Start at"));
            ui.add(egui::DragValue::new(&mut d.options.from_seconds).range(0.0..=tooncraft_doc::sound::MAX_SECONDS).speed(0.05).suffix(tl!(" s")))
                .labelled_by(l.id);
            ui.end_row();
            let l = ui.label(tl!("Take every"));
            ui.add(egui::DragValue::new(&mut d.options.every).range(1..=24).suffix(tl!(" frame(s)"))).labelled_by(l.id);
            ui.end_row();
            let l = ui.label(tl!("At most (0: as many as fit)"));
            ui.add(egui::DragValue::new(&mut d.options.max_frames).range(0..=tooncraft_engine::import::MAX_IMPORT as u32)).labelled_by(l.id);
            ui.end_row();
            ui.label("");
            ui.checkbox(&mut d.options.reference, tl!("Under the drawing, locked and faded, to trace over"));
            ui.end_row();
            ui.label("");
            ui.checkbox(&mut d.options.grow, tl!("Lengthen the shot to fit the video"));
            ui.end_row();
        });
        ui.horizontal(|ui| {
            let ready = matches!(d.info, Some(Ok(_)));
            if ui.add_enabled(ready, egui::Button::new(tl!("Import"))).clicked() {
                go = true;
            }
            if ui.button(tl!("Cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                keep = false;
            }
        });
    });
    if go {
        run(app, &d);
        keep = false;
    }
    if keep {
        app.video_dialog = Some(d);
    }
}
