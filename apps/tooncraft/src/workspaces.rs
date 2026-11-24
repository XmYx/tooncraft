//! Workspaces: Draw, Time, Ink & Paint and Sheet, the four built-in dock presets (launch names
//! `drawing`, `timing`, `ink-paint`, `sheet`). The top bar's segmented control and Window ▸
//! Workspace switch between them. Changes made to the panels while a workspace is active belong
//! to that workspace: switching away keeps them, switching back restores them, and Reset
//! Workspace returns to the preset. Save Workspace As… keeps the current layout under a name of
//! its own (the named list lives in `canvas_tools::CanvasUi`).
//!
//! The edits are saved next to the shortcuts (`workspace-edits.json`). That file is not trusted:
//! unknown workspaces and unreadable layouts are skipped, and a layout the dock cannot show falls
//! back to the default (see `dock::show`).

use std::collections::HashMap;
use std::path::PathBuf;

use craft_ui::docking::Layout;

use crate::app::ToonApp;
use crate::dock::Pane;

/// The largest edits file read (four layouts are a few kilobytes; anything bigger is not ours).
const MAX_FILE: usize = 4 << 20;

/// One of the built-in workspaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Draw,
    Time,
    InkPaint,
    Sheet,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Draw, Kind::Time, Kind::InkPaint, Kind::Sheet];

    /// The launch name (`--layout <name>`), also the key in the saved file.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Draw => "drawing",
            Kind::Time => "timing",
            Kind::InkPaint => "ink-paint",
            Kind::Sheet => "sheet",
        }
    }

    pub fn from_name(name: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.name() == name)
    }

    /// The short name on the segmented control.
    pub fn label(self) -> String {
        match self {
            Kind::Draw => tl!("Draw"),
            Kind::Time => tl!("Time"),
            Kind::InkPaint => tl!("Ink & Paint"),
            Kind::Sheet => tl!("Sheet"),
        }
        .to_string()
    }

    /// What the workspace is for (tooltip).
    pub fn hint(self) -> String {
        match self {
            Kind::Draw => tl!("Drawing: a big canvas, colour and brushes on the right, the timeline below"),
            Kind::Time => tl!("Timing: a tall timeline with the timing, peg and camera panels in front"),
            Kind::InkPaint => tl!("Ink and paint: the canvas, colour and layers, a short timeline"),
            Kind::Sheet => tl!("Exposure sheet: the sheet down the right side, layers as columns"),
        }
        .to_string()
    }

    /// The preset's panels.
    pub fn preset(self) -> Layout<Pane> {
        crate::canvas_tools::layout_by_name(self.name()).unwrap_or_else(|| crate::dock::Workspace::default().layout)
    }

    /// Whether the preset shows the timeline as a vertical exposure sheet.
    fn preset_vertical(self, preference: bool) -> bool {
        match self {
            Kind::Sheet => true,
            _ => preference,
        }
    }
}

/// A workspace as the user left it.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Edit {
    pub layout: Layout<Pane>,
    #[serde(default)]
    pub vertical: bool,
}

#[derive(Debug, Default)]
pub struct Workspaces {
    /// The built-in workspace on screen; `None` after a named layout was picked.
    pub active: Option<Kind>,
    edits: HashMap<Kind, Edit>,
    /// The name typed for Save Workspace As….
    pub save_as: String,
    /// Where edits are kept (`None`: not saved, as in tests and screenshots).
    store: Option<PathBuf>,
}

impl Workspaces {
    /// With `persist`, edits are read from and written to the user's config folder.
    pub fn new(persist: bool) -> Self {
        let store = if persist { crate::keys::overrides_path().and_then(|p| p.parent().map(|d| d.join("workspace-edits.json"))) } else { None };
        Self::with_store(store)
    }

    /// Edits kept in `store` (`None`: not saved).
    pub(crate) fn with_store(store: Option<PathBuf>) -> Self {
        let edits = store.as_ref().and_then(|p| std::fs::read(p).ok()).filter(|b| b.len() < MAX_FILE).map(|b| parse(&b)).unwrap_or_default();
        Self { active: Some(Kind::Draw), edits, save_as: String::new(), store }
    }

    /// Does `kind` differ from its preset (edited and kept)?
    pub fn is_edited(&self, kind: Kind) -> bool {
        self.edits.contains_key(&kind)
    }

    /// The kept edit of `kind`, if any.
    pub fn edit(&self, kind: Kind) -> Option<&Edit> {
        self.edits.get(&kind)
    }

    fn save(&self) -> Result<(), String> {
        let Some(path) = &self.store else { return Ok(()) };
        let map: serde_json::Map<String, serde_json::Value> =
            self.edits.iter().filter_map(|(k, e)| serde_json::to_value(e).ok().map(|v| (k.name().to_string(), v))).collect();
        let bytes = serde_json::to_vec_pretty(&map).map_err(|e| e.to_string())?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        craft_atomic::atomic_write(path, &bytes).map_err(|e| e.to_string())
    }

    /// Keep what is on screen as the active workspace's edit (nothing when it matches the preset).
    fn keep(&mut self, layout: &Layout<Pane>, vertical: bool, preference: bool) {
        let Some(kind) = self.active else { return };
        if *layout == kind.preset() && vertical == kind.preset_vertical(preference) {
            self.edits.remove(&kind);
        } else {
            self.edits.insert(kind, Edit { layout: layout.clone(), vertical });
        }
    }
}

/// Saved edits from a file; anything unreadable is skipped (the file is not trusted).
fn parse(bytes: &[u8]) -> HashMap<Kind, Edit> {
    let Ok(serde_json::Value::Object(map)) = serde_json::from_slice::<serde_json::Value>(bytes) else { return HashMap::new() };
    map.into_iter().filter_map(|(name, v)| Some((Kind::from_name(&name)?, serde_json::from_value::<Edit>(v).ok()?))).collect()
}

/// Switch to `kind`: the current workspace keeps its edits, and `kind` comes back as it was left
/// (or as its preset).
pub fn switch(app: &mut ToonApp, kind: Kind) {
    let preference = app.preferences.prefs.vertical_sheet;
    let layout = app.workspace.layout.clone();
    app.workspaces.keep(&layout, app.tl.vertical, preference);
    let edit = app.workspaces.edits.get(&kind).cloned().unwrap_or_else(|| Edit { layout: kind.preset(), vertical: kind.preset_vertical(preference) });
    app.workspace.layout = edit.layout;
    app.tl.vertical = edit.vertical;
    app.workspaces.active = Some(kind);
    if let Err(e) = app.workspaces.save() {
        app.set_status(tlf!("Could not keep the workspace: {error}", error = e));
    }
}

/// Keep the active workspace as it is now and write the edits (on exit, so changes made since
/// the last switch are not lost).
pub fn keep_current(app: &mut ToonApp) -> Result<(), String> {
    let preference = app.preferences.prefs.vertical_sheet;
    let layout = app.workspace.layout.clone();
    app.workspaces.keep(&layout, app.tl.vertical, preference);
    app.workspaces.save()
}

/// Use a workspace's preset without touching kept edits (launch options and screenshots).
pub fn show_preset(app: &mut ToonApp, kind: Kind) {
    app.workspace.layout = kind.preset();
    app.tl.vertical = kind.preset_vertical(app.preferences.prefs.vertical_sheet);
    app.workspaces.active = Some(kind);
}

/// Put the active workspace (Draw after a named layout) back to its preset and forget its edits.
pub fn reset(app: &mut ToonApp) {
    let kind = app.workspaces.active.unwrap_or(Kind::Draw);
    app.workspaces.edits.remove(&kind);
    show_preset(app, kind);
    match app.workspaces.save() {
        Ok(()) => app.set_status(tlf!("{workspace} is back to its first layout", workspace = kind.label())),
        Err(e) => app.set_status(tlf!("Could not keep the workspace: {error}", error = e)),
    }
}

/// Keep the current layout under `name` (Save Workspace As…).
pub fn save_as(app: &mut ToonApp, name: &str) -> bool {
    let layout = app.workspace.layout.clone();
    match app.canvas_ui.save_workspace(name, &layout) {
        Ok(()) => {
            app.set_status(tlf!("Saved workspace {name}", name = name.trim()));
            app.workspaces.save_as.clear();
            true
        }
        Err(e) => {
            app.set_status(e);
            false
        }
    }
}

/// Use a named workspace saved earlier.
pub fn use_named(app: &mut ToonApp, name: &str) {
    let Some(layout) = app.canvas_ui.workspaces.iter().find(|(n, _)| n == name).map(|(_, l)| l.clone()) else { return };
    let preference = app.preferences.prefs.vertical_sheet;
    let current = app.workspace.layout.clone();
    app.workspaces.keep(&current, app.tl.vertical, preference);
    app.workspace.layout = layout;
    app.workspaces.active = None;
    if let Err(e) = app.workspaces.save() {
        app.set_status(tlf!("Could not keep the workspace: {error}", error = e));
    }
}

/// Window ▸ Workspace: the four workspaces, the named ones, Save Workspace As… and Reset
/// Workspace.
pub fn menu(app: &mut ToonApp, ui: &mut egui::Ui) {
    for kind in Kind::ALL {
        let on = app.workspaces.active == Some(kind);
        if ui.radio(on, kind.label()).on_hover_text(kind.hint()).clicked() {
            switch(app, kind);
            ui.close();
        }
    }
    let saved: Vec<String> = app.canvas_ui.workspaces.iter().map(|(n, _)| n.clone()).collect();
    if !saved.is_empty() {
        ui.separator();
    }
    for name in saved {
        ui.horizontal(|ui| {
            if ui.button(&name).clicked() {
                use_named(app, &name);
                ui.close();
            }
            if ui.small_button(tlf!("Delete {name}", name = name)).clicked()
                && let Err(e) = app.canvas_ui.delete_workspace(&name)
            {
                app.set_status(e);
            }
        });
    }
    ui.separator();
    ui.horizontal(|ui| {
        let l = ui.label(tl!("Save Workspace As…"));
        let field = ui.add(egui::TextEdit::singleline(&mut app.workspaces.save_as).hint_text(tl!("name")).desired_width(110.0)).labelled_by(l.id);
        let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (ui.button(tl!("Save")).clicked() || enter) && save_as(app, &app.workspaces.save_as.clone()) {
            ui.close();
        }
    });
    let kind = app.workspaces.active.unwrap_or(Kind::Draw);
    if ui.button(tl!("Reset Workspace")).on_hover_text(tlf!("{workspace} back to its first layout", workspace = kind.label())).clicked() {
        reset(app);
        ui.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_workspace_has_a_preset_a_name_and_a_label() {
        for k in Kind::ALL {
            assert_eq!(Kind::from_name(k.name()), Some(k));
            assert!(!k.label().is_empty() && !k.hint().is_empty());
            assert!(k.preset().contains(&Pane::Canvas), "{k:?} shows the canvas");
        }
        assert_eq!(Kind::from_name("nope"), None);
        assert!(Kind::Sheet.preset_vertical(false));
        assert!(!Kind::Draw.preset_vertical(false));
    }

    #[test]
    fn hostile_edit_files_are_skipped_not_trusted() {
        assert!(parse(b"").is_empty());
        assert!(parse(b"[1,2,3]").is_empty());
        assert!(parse(b"{\"drawing\": 7, \"nope\": {}}").is_empty());
        let good = serde_json::to_vec(&serde_json::json!({
            "timing": serde_json::to_value(Edit { layout: Kind::Sheet.preset(), vertical: true }).unwrap(),
            "elsewhere": serde_json::to_value(Edit { layout: Kind::Draw.preset(), vertical: false }).unwrap(),
        }))
        .unwrap();
        let e = parse(&good);
        assert_eq!(e.len(), 1);
        assert_eq!(e[&Kind::Time].layout, Kind::Sheet.preset());
    }

    #[test]
    fn keeping_a_preset_layout_keeps_no_edit() {
        let mut w = Workspaces::new(false);
        w.keep(&Kind::Draw.preset(), false, false);
        assert!(!w.is_edited(Kind::Draw));
        w.keep(&Kind::Time.preset(), false, false);
        assert!(w.is_edited(Kind::Draw), "Draw showing another layout is an edit");
        w.keep(&Kind::Draw.preset(), false, false);
        assert!(!w.is_edited(Kind::Draw), "back to the preset forgets the edit");
    }
}
