//! Edit ▸ Record Macro / Play Macro: the steps taken, kept as a command file and played back as
//! one undo step (see `tooncraft_engine::macros`).

use std::path::Path;

use tooncraft_engine::macros;

use crate::app::ToonApp;

/// Start recording, or stop and offer to save what was recorded.
pub fn toggle(app: &mut ToonApp) {
    if !app.engine.is_recording() {
        app.engine.start_recording();
        app.set_status(tl!("Recording a macro: every step from now on is kept until you stop"));
        return;
    }
    let Some(m) = app.engine.stop_recording() else { return };
    if m.steps.is_empty() {
        app.set_status(tl!("Stopped recording: no steps were recorded"));
        return;
    }
    let name = format!("{}.jsonl", app.engine.project.name);
    if let Some(path) = rfd::FileDialog::new().add_filter(tl!("ToonCraft macro"), &["jsonl"]).set_file_name(name).save_file() {
        save(app, &m, &path);
    } else {
        app.set_status(tl!("Stopped recording; the macro was not saved"));
    }
}

/// Write a recorded macro to `path` and say so.
pub fn save(app: &mut ToonApp, m: &macros::Macro, path: &Path) {
    match craft_atomic::atomic_write(path, macros::to_lines(&m.steps).as_bytes()) {
        Ok(()) if m.dropped > 0 => app.set_status(tlf!(
            "Saved {n} steps to {file}; {dropped} more did not fit in one macro",
            n = m.steps.len(),
            file = path.display(),
            dropped = m.dropped
        )),
        Ok(()) => app.set_status(tlf!("Saved {n} steps to {file}", n = m.steps.len(), file = path.display())),
        Err(e) => app.set_status(tlf!("Could not save the macro: {error}", error = e)),
    }
}

/// Choose a macro file and play it.
pub fn open(app: &mut ToonApp) {
    if let Some(path) = rfd::FileDialog::new().add_filter(tl!("ToonCraft macro"), &["jsonl"]).pick_file() {
        play_file(app, &path);
    }
}

/// Play the macro in `path` on the shot as one undo step; on an error nothing changes.
pub fn play_file(app: &mut ToonApp, path: &Path) {
    let read = std::fs::metadata(path)
        .map_err(|e| e.to_string())
        .and_then(|m| if m.len() > macros::MAX_FILE_BYTES as u64 { Err(tl!("the file is too large").to_string()) } else { Ok(()) })
        .and_then(|()| std::fs::read_to_string(path).map_err(|e| e.to_string()));
    let label = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Macro").to_string();
    let result = read.and_then(|text| macros::from_lines(&text).map_err(|e| e.to_string())).and_then(|steps| {
        let n = steps.len();
        app.engine.play_macro(&label, steps).map(|()| n).map_err(|e| e.to_string())
    });
    match result {
        Ok(n) => app.set_status(tlf!("Played {name}: {n} steps as one undo step", name = label, n = n)),
        Err(e) => app.set_status(tlf!("Could not play {name}: {error}", name = label, error = e)),
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn record_save_and_play_a_macro() {
        struct Gone(std::path::PathBuf);
        impl Drop for Gone {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let dir = std::env::temp_dir().join(format!("tooncraft-macro-ui-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let _gone = Gone(dir.clone());
        let ctx = egui::Context::default();
        let mut app = crate::app::ToonApp::with_context(&ctx, crate::args::Cli::default()).without_autosave();
        let app = &mut app;
        app.engine.start_recording();
        app.engine.apply(tooncraft_engine::Command::SetMarker { frame: 1, text: "beat".into() }).unwrap();
        let m = app.engine.stop_recording().unwrap();
        let path = dir.join("beat.jsonl");
        super::save(app, &m, &path);
        assert!(path.is_file());
        app.engine.undo().unwrap();
        assert!(app.engine.project.markers.is_empty());
        super::play_file(app, &path);
        assert_eq!(app.engine.project.markers.get(&1).map(String::as_str), Some("beat"));
        assert_eq!(app.engine.undo_label(), Some("beat"), "named after the file");
        // a bad file says why and changes nothing
        let bad = dir.join("bad.jsonl");
        std::fs::write(&bad, "not json").unwrap();
        let before = app.engine.project.clone();
        super::play_file(app, &bad);
        assert_eq!(app.engine.project, before);
        assert!(app.status.contains("Could not play"), "{}", app.status);
    }
}
