//! Recent shots (File ▸ Open Recent) and the start window shown when ToonCraft opens with no shot:
//! a new-shot form, the recent shots (missing ones marked), Open… and the demo shot.
//!
//! The recent list is a small JSON file next to the shortcuts. It is not trusted: it is read only
//! when small, every entry is checked, the list is de-duplicated and capped at
//! [`MAX_RECENT_FILES`], so a damaged file gives a shorter list, never a crash. Tests and
//! screenshots keep the list in memory only.

use std::path::{Path, PathBuf};

use crate::app::{NewShot, ToonApp};

/// Most recent shots kept.
pub const MAX_RECENT_FILES: usize = 12;
/// A recent-files file larger than this is ignored.
const MAX_STORE_BYTES: usize = 256 << 10;
/// Longer entries are dropped (no real path is this long).
const MAX_PATH_CHARS: usize = 4096;

/// The recent shots, newest first.
#[derive(Default)]
pub struct Recent {
    pub paths: Vec<PathBuf>,
    /// Where the list is saved (none in tests and screenshots).
    store: Option<PathBuf>,
}

/// The list read from a recent-files file: valid entries only, de-duplicated, capped.
pub fn parse(bytes: &[u8]) -> Vec<PathBuf> {
    if bytes.len() > MAX_STORE_BYTES {
        return Vec::new();
    }
    let raw: Vec<serde_json::Value> = serde_json::from_slice(bytes).unwrap_or_default();
    let mut out: Vec<PathBuf> = Vec::new();
    for v in raw {
        if out.len() >= MAX_RECENT_FILES {
            break;
        }
        let Some(s) = v.as_str() else { continue };
        if s.is_empty() || s.len() > MAX_PATH_CHARS || s.contains('\0') {
            continue;
        }
        let p = PathBuf::from(s);
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// The same file for the list's purposes: made absolute without touching the disk, so a missing
/// shot keeps its entry.
fn key(p: &Path) -> PathBuf {
    std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf())
}

impl Recent {
    pub fn new(persist: bool) -> Self {
        let store = if persist { crate::keys::overrides_path().and_then(|p| p.parent().map(|d| d.join("recent.json"))) } else { None };
        let paths = store.as_ref().and_then(|p| std::fs::read(p).ok()).map(|b| parse(&b)).unwrap_or_default();
        Self { paths, store }
    }

    /// Put `p` first (moving it up if it is already listed) and drop the oldest past the cap.
    pub fn note(&mut self, p: &Path) -> Result<(), String> {
        let k = key(p);
        if k.as_os_str().len() > MAX_PATH_CHARS {
            return Ok(());
        }
        self.paths.retain(|q| key(q) != k);
        self.paths.insert(0, k);
        self.paths.truncate(MAX_RECENT_FILES);
        self.save()
    }

    /// Forget every entry.
    pub fn clear(&mut self) -> Result<(), String> {
        self.paths.clear();
        self.save()
    }

    fn save(&self) -> Result<(), String> {
        let Some(path) = &self.store else { return Ok(()) };
        let list: Vec<String> = self.paths.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        let bytes = serde_json::to_vec_pretty(&list).map_err(|e| e.to_string())?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        craft_atomic::atomic_write(path, &bytes).map_err(|e| e.to_string())
    }
}

/// The start window's state.
#[derive(Default)]
pub struct Start {
    pub open: bool,
    pub form: NewShot,
    /// Put the keyboard focus in the window on its next frame.
    pub focus: bool,
}

/// Remember a shot that was just opened or saved; a failed write only gives a status message.
pub fn note(app: &mut ToonApp, p: &Path) {
    if let Err(e) = app.recent_files.note(p) {
        app.set_status(tlf!("Recent shots not saved: {error}", error = e));
    }
}

/// The name shown for a recent shot, and whether it is still there.
fn entry(p: &Path) -> (String, bool) {
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string());
    if p.is_file() { (name, true) } else { (tlf!("{name} (missing)", name = name), false) }
}

/// File ▸ Open Recent.
pub fn menu(app: &mut ToonApp, ui: &mut egui::Ui) {
    ui.menu_button(tl!("Open Recent"), |ui| {
        if app.recent_files.paths.is_empty() {
            ui.add_enabled(false, egui::Button::new(tl!("No recent shots")));
            return;
        }
        let mut pick = None;
        for p in &app.recent_files.paths {
            let (name, there) = entry(p);
            let tip = p.display().to_string();
            if ui.add_enabled(there, egui::Button::new(name)).on_hover_text(&tip).on_disabled_hover_text(&tip).clicked() {
                pick = Some(p.clone());
            }
        }
        ui.separator();
        if ui.button(tl!("Clear Recent Shots")).clicked()
            && let Err(e) = app.recent_files.clear()
        {
            app.set_status(tlf!("Recent shots not saved: {error}", error = e));
        }
        if let Some(p) = pick {
            app.open(&p);
            ui.close();
        }
    });
}

/// Should the start window open at launch? Not for a file, the demo, a screenshot or a benchmark,
/// and not when the user turned it off; `--open start` always shows it.
/// UI tests never get it unasked.
pub fn wanted_at_launch(cli: &crate::args::Cli, enabled: bool) -> bool {
    launch_rule(cli, enabled && !cfg!(test))
}

fn launch_rule(cli: &crate::args::Cli, enabled: bool) -> bool {
    cli.open.as_deref() == Some("start") || (enabled && !cli.demo && cli.file.is_none() && cli.screenshot.is_none() && cli.bench_playback.is_none())
}

/// Open the start window with the new-shot form filled from the preferences.
pub fn show(app: &mut ToonApp) {
    app.start = Start { open: true, form: app.new_shot_defaults(), focus: true };
}

enum Pick {
    Create,
    Open(PathBuf),
    Browse,
    Demo,
    Close,
}

/// The start window.
pub fn window(app: &mut ToonApp, ctx: &egui::Context) {
    if !app.start.open {
        return;
    }
    let mut form = std::mem::take(&mut app.start.form);
    let focus = std::mem::take(&mut app.start.focus);
    let mut pick = None;
    let mut show_at_start = app.preferences.prefs.start_screen;
    egui::Window::new(tl!("Start")).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(430.0);
                ui.heading(tl!("New shot"));
                app.new_shot_fields(ui, "start-new-shot", &mut form);
                let create = ui.button(tl!("Create"));
                if create.clicked() {
                    pick = Some(Pick::Create);
                }
                if focus && app.recent_files.paths.iter().all(|p| !p.is_file()) {
                    create.request_focus();
                }
            });
            ui.add_space(24.0);
            ui.vertical(|ui| {
                ui.set_width(280.0);
                ui.heading(tl!("Recent shots"));
                if app.recent_files.paths.is_empty() {
                    ui.label(egui::RichText::new(tl!("Shots you open or save are listed here.")).weak());
                }
                let mut focused = false;
                for p in &app.recent_files.paths {
                    let (name, there) = entry(p);
                    let tip = p.display().to_string();
                    let r = ui.add_enabled(there, egui::Button::new(name)).on_hover_text(&tip).on_disabled_hover_text(&tip);
                    if focus && there && !focused {
                        r.request_focus();
                        focused = true;
                    }
                    if r.clicked() {
                        pick = Some(Pick::Open(p.clone()));
                    }
                }
                ui.separator();
                if ui.button(tl!("Open…")).clicked() {
                    pick = Some(Pick::Browse);
                }
                if ui.button(tl!("Open the Demo Shot")).on_hover_text(tl!("A short bouncing-ball shot to try things on")).clicked() {
                    pick = Some(Pick::Demo);
                }
            });
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.checkbox(&mut show_at_start, tl!("Show this window at start"));
            if ui.button(tl!("Close")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                pick = Some(Pick::Close);
            }
        });
    });
    if show_at_start != app.preferences.prefs.start_screen {
        app.preferences.prefs.start_screen = show_at_start;
        crate::preferences::save_now(app);
    }
    app.start.form = form;
    match pick {
        Some(Pick::Create) => {
            let n = app.start.form.clone();
            if app.create_shot(&n) {
                app.start.open = false;
            }
        }
        Some(Pick::Open(p)) => {
            app.open(&p);
            app.start.open = false;
        }
        Some(Pick::Browse) => {
            app.open_dialog();
            app.start.open = false;
        }
        Some(Pick::Demo) => match tooncraft_engine::demo::demo(1280, 720) {
            Ok(e) => {
                app.replace_engine(e, None);
                app.start.open = false;
            }
            Err(e) => app.set_status(e.to_string()),
        },
        Some(Pick::Close) => app.start.open = false,
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_is_bounded_and_deduplicated() {
        let mut r = Recent::default();
        for i in 0..40 {
            r.note(Path::new(&format!("/shots/s{i}.toon"))).expect("in memory");
        }
        assert_eq!(r.paths.len(), MAX_RECENT_FILES);
        assert_eq!(r.paths[0], key(Path::new("/shots/s39.toon")), "newest first");
        r.note(Path::new("/shots/s30.toon")).expect("in memory");
        assert_eq!(r.paths.len(), MAX_RECENT_FILES, "no duplicate");
        assert_eq!(r.paths[0], key(Path::new("/shots/s30.toon")), "moved to the top");
        assert_eq!(r.paths.iter().filter(|p| **p == key(Path::new("/shots/s30.toon"))).count(), 1);
        // the same file through a relative path is one entry
        let rel = Path::new("relative.toon");
        r.note(rel).expect("in memory");
        r.note(&std::path::absolute(rel).expect("absolute")).expect("in memory");
        assert_eq!(r.paths.iter().filter(|p| p.ends_with("relative.toon")).count(), 1);
        r.clear().expect("in memory");
        assert!(r.paths.is_empty());
    }

    #[test]
    fn a_damaged_list_gives_what_is_valid() {
        assert!(parse(b"not json").is_empty());
        assert!(parse(b"{\"a\": 1}").is_empty());
        assert!(parse(&vec![b' '; MAX_STORE_BYTES + 1]).is_empty(), "too large: ignored");
        let long = "x".repeat(MAX_PATH_CHARS + 1);
        let json = format!("[\"/a.toon\", 7, null, \"\", \"/a.toon\", \"{long}\", \"/b.toon\"]");
        assert_eq!(parse(json.as_bytes()), vec![PathBuf::from("/a.toon"), PathBuf::from("/b.toon")]);
        let many: Vec<String> = (0..1000).map(|i| format!("/s{i}.toon")).collect();
        assert_eq!(parse(&serde_json::to_vec(&many).expect("json")).len(), MAX_RECENT_FILES, "capped");
    }

    #[test]
    fn the_list_is_saved_and_read_back() {
        let dir = std::env::temp_dir().join(format!("tooncraft-recent-test-{}", std::process::id()));
        let store = dir.join("recent.json");
        let mut r = Recent { paths: Vec::new(), store: Some(store.clone()) };
        r.note(Path::new("/x/one.toon")).expect("saved");
        r.note(Path::new("/x/two.toon")).expect("saved");
        let back = parse(&std::fs::read(&store).expect("written"));
        assert_eq!(back, vec![key(Path::new("/x/two.toon")), key(Path::new("/x/one.toon"))]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_start_window_is_skipped_for_files_demo_and_screenshots() {
        use crate::args::Cli;
        assert!(launch_rule(&Cli::default(), true), "no file: shown");
        assert!(!launch_rule(&Cli::default(), false), "turned off in the preferences");
        assert!(launch_rule(&Cli { open: Some("start".into()), ..Default::default() }, false), "--open start always shows it");
        assert!(!launch_rule(&Cli { demo: true, ..Default::default() }, true));
        assert!(!launch_rule(&Cli { file: Some("a.toon".into()), ..Default::default() }, true));
        assert!(!launch_rule(&Cli { screenshot: Some("a.png".into()), ..Default::default() }, true));
        assert!(!launch_rule(&Cli { bench_playback: Some(1.0), ..Default::default() }, true));
        // the UI tests build the app with no file: they never get it unasked
        assert!(!wanted_at_launch(&Cli::default(), true));
    }
}
