//! Shot size and frame-rate presets for New Shot and Canvas Size: common video, cinema, square
//! and portrait sizes (named by us), plus the user's own, saved next to the shortcuts. The file is
//! not trusted: at most [`MAX_USER`] presets, every number clamped, a damaged file gives none.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Most presets of the user's own.
pub const MAX_USER: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

impl Preset {
    fn new(name: &str, width: u32, height: u32, fps: u32) -> Self {
        Self { name: name.to_string(), width, height, fps }
    }

    pub fn sanitized(self) -> Self {
        Self {
            name: self.name.chars().filter(|c| !c.is_control()).take(60).collect::<String>().trim().to_string(),
            width: self.width.clamp(16, tooncraft_raster::MAX_SIDE),
            height: self.height.clamp(16, tooncraft_raster::MAX_SIDE),
            fps: self.fps.clamp(tooncraft_doc::MIN_FPS, tooncraft_doc::MAX_FPS),
        }
    }
}

/// The built-in presets.
pub fn builtin() -> Vec<Preset> {
    vec![
        Preset::new(tl!("HD, 24 fps"), 1280, 720, 24),
        Preset::new(tl!("Full HD, 24 fps"), 1920, 1080, 24),
        Preset::new(tl!("Full HD, 25 fps"), 1920, 1080, 25),
        Preset::new(tl!("Full HD, 30 fps"), 1920, 1080, 30),
        Preset::new(tl!("UHD, 24 fps"), 3840, 2160, 24),
        Preset::new(tl!("Cinema 2K, 24 fps"), 2048, 1080, 24),
        Preset::new(tl!("Cinema 4K, 24 fps"), 4096, 2160, 24),
        Preset::new(tl!("Square, 24 fps"), 1080, 1080, 24),
        Preset::new(tl!("Portrait, 30 fps"), 1080, 1920, 30),
        Preset::new(tl!("Standard definition, 25 fps"), 720, 576, 25),
        Preset::new(tl!("Standard definition, 30 fps"), 720, 480, 30),
    ]
}

/// The width : height ratio in its simplest whole numbers (`16:9`), or a decimal when those are
/// large.
pub fn aspect(w: u32, h: u32) -> String {
    fn gcd(a: u32, b: u32) -> u32 {
        if b == 0 { a.max(1) } else { gcd(b, a % b) }
    }
    let g = gcd(w, h);
    let (a, b) = (w / g, h / g);
    if a <= 64 && b <= 64 { format!("{a}:{b}") } else { format!("{:.2}:1", w as f64 / h.max(1) as f64) }
}

pub struct State {
    pub user: Vec<Preset>,
    /// The name typed for Save as Preset.
    pub new_name: String,
    store: Option<PathBuf>,
}

impl State {
    pub fn new(persist: bool) -> Self {
        let store = if persist { crate::keys::overrides_path().and_then(|p| p.parent().map(|d| d.join("scene-presets.json"))) } else { None };
        Self { user: load(store.as_ref()), new_name: String::new(), store }
    }

    /// Save the size and rate under `name` (replacing one of that name).
    pub fn add(&mut self, name: &str, width: u32, height: u32, fps: u32) -> Result<(), String> {
        let p = Preset::new(name, width, height, fps).sanitized();
        if p.name.is_empty() {
            return Err(tl!("Give the preset a name").to_string());
        }
        if let Some(i) = self.user.iter().position(|u| u.name == p.name) {
            self.user[i] = p;
        } else if self.user.len() < MAX_USER {
            self.user.push(p);
        } else {
            return Err(tl!("Too many presets").to_string());
        }
        self.save()
    }

    fn save(&self) -> Result<(), String> {
        let Some(path) = &self.store else { return Ok(()) };
        let bytes = serde_json::to_vec_pretty(&self.user).map_err(|e| e.to_string())?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        craft_atomic::atomic_write(path, &bytes).map_err(|e| e.to_string())
    }
}

/// The user's presets from a file; a damaged or oversized file gives none.
pub fn load(path: Option<&PathBuf>) -> Vec<Preset> {
    let Some(bytes) = path.and_then(|p| std::fs::read(p).ok()).filter(|b| b.len() < 1 << 20) else { return Vec::new() };
    let Ok(list) = serde_json::from_slice::<Vec<Preset>>(&bytes) else { return Vec::new() };
    list.into_iter().take(MAX_USER).map(Preset::sanitized).filter(|p| !p.name.is_empty()).collect()
}

/// A preset picker for a size (and rate, when given) with Save as Preset; true when a value
/// changed.
pub fn picker(state: &mut State, ui: &mut egui::Ui, id: &str, width: &mut u32, height: &mut u32, mut fps: Option<&mut u32>) -> bool {
    let mut changed = false;
    let all: Vec<Preset> = builtin().into_iter().chain(state.user.iter().cloned()).collect();
    ui.horizontal(|ui| {
        let l = ui.label(tl!("Size preset"));
        egui::ComboBox::from_id_salt(("scene-preset", id))
            .selected_text(tl!("Choose…"))
            .width(220.0)
            .show_ui(ui, |ui| {
                for p in &all {
                    let text = format!("{}   {}×{}", p.name, p.width, p.height);
                    if ui.selectable_label(false, text).clicked() {
                        *width = p.width;
                        *height = p.height;
                        if let Some(f) = fps.as_deref_mut() {
                            *f = p.fps;
                        }
                        changed = true;
                    }
                }
            })
            .response
            .labelled_by(l.id);
        ui.weak(aspect(*width, *height));
    });
    ui.horizontal(|ui| {
        let l = ui.label(tl!("Preset name"));
        ui.add(egui::TextEdit::singleline(&mut state.new_name).desired_width(140.0)).labelled_by(l.id);
        if ui.button(tl!("Save as Preset")).clicked() {
            let name = state.new_name.clone();
            let rate = fps.as_deref().copied().unwrap_or(24);
            if state.add(&name, *width, *height, rate).is_ok() {
                state.new_name.clear();
            }
        }
    });
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aspect_presets_and_a_hostile_file() {
        assert_eq!(aspect(1920, 1080), "16:9");
        assert_eq!(aspect(1080, 1080), "1:1");
        assert_eq!(aspect(2048, 1080), "1.90:1");
        assert!(builtin().iter().all(|p| p.clone().sanitized() == *p), "built-ins are within the limits");
        let dir = std::env::temp_dir().join(format!("tooncraft-presets-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("scene-presets.json");
        std::fs::write(&f, b"{ nope").unwrap();
        assert!(load(Some(&f)).is_empty(), "a damaged file gives none");
        std::fs::write(&f, br#"[{"name":"Big","width":999999,"height":0,"fps":9999},{"name":"","width":10,"height":10,"fps":24}]"#).unwrap();
        let l = load(Some(&f));
        assert_eq!(l.len(), 1, "an empty name is dropped");
        assert_eq!((l[0].width, l[0].height, l[0].fps), (tooncraft_raster::MAX_SIDE, 16, tooncraft_doc::MAX_FPS));
        let mut s = State { user: Vec::new(), new_name: String::new(), store: Some(f.clone()) };
        s.add("Show A", 1998, 1080, 24).unwrap();
        s.add("Show A", 1920, 1080, 25).unwrap();
        assert_eq!(s.user.len(), 1, "the same name is replaced");
        assert_eq!(load(Some(&f))[0].fps, 25, "saved to the file");
        assert!(s.add("   ", 10, 10, 24).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
