//! Remappable shortcuts through craft-keymap: ToonCraft's command catalog with default keys, the
//! user's changes saved as JSON next to the app's settings, dispatch of a frame's key presses, and
//! the model behind the shortcut editor (Help ▸ Keyboard Shortcuts).

use std::path::PathBuf;

use craft_keymap::{Bindable, Binding, Chord, CommandRef, Keymap, Overrides, Platform, Policy, Scope, Scopes};

/// The remappable command ids, in the shortcut editor's order (from the command registry).
pub fn ids() -> impl Iterator<Item = &'static str> {
    crate::commands::remappable().map(|(id, _, _)| id)
}

pub fn label(id: &str) -> String {
    crate::commands::get(id).map(|c| c.label()).unwrap_or_else(|| id.to_string())
}

/// A key as the user sees it on this platform ("Ctrl+S"); the text itself when it does not parse.
pub fn chord_text(key: &str) -> String {
    match Chord::parse(key) {
        Ok(c) => Binding::new(c).text(&Scope::app()),
        Err(_) => key.to_string(),
    }
}

fn platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::Mac
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else {
        Platform::Linux
    }
}

fn catalog() -> Vec<Bindable> {
    crate::commands::remappable()
        .map(|(id, label, key)| {
            let b = Bindable::new(CommandRef::new(id), label, Scope::app());
            match Chord::parse(key) {
                Ok(c) => b.with_default(Binding::new(c)),
                Err(_) => b,
            }
        })
        .collect()
}

/// Where the user's shortcut changes are kept.
pub fn overrides_path() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("TOONCRAFT_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d).join("shortcuts.json"));
    }
    let home = |v: &str| std::env::var_os(v).filter(|d| !d.is_empty()).map(PathBuf::from);
    let base = if cfg!(target_os = "windows") {
        home("APPDATA").map(|d| d.join("ToonCraft"))
    } else if cfg!(target_os = "macos") {
        home("HOME").map(|d| d.join("Library/Application Support/ToonCraft"))
    } else {
        home("XDG_CONFIG_HOME").or_else(|| home("HOME").map(|h| h.join(".config"))).map(|d| d.join("tooncraft"))
    };
    base.map(|b| b.join("shortcuts.json"))
}

pub struct Keys {
    pub map: Keymap,
    dispatcher: craft_keymap::egui::Dispatcher,
    /// The command waiting for a new key in the editor.
    pub recording: Option<String>,
    save: bool,
    /// The shortcut editor's search: a command name or a key ("ctrl+z").
    pub filter: String,
    /// After a change: the command the new key was taken from, said in the editor.
    pub taken: Option<String>,
}

impl Keys {
    /// The catalog with the saved overrides (`load`: read them from disk).
    pub fn new(load: bool) -> Self {
        let mut map = Keymap::new(catalog(), Scopes::new(), platform());
        if load
            && let Some(p) = overrides_path()
            && let Ok(text) = std::fs::read_to_string(&p)
            && text.len() < 1 << 20
            && let Ok(o) = serde_json::from_str::<Overrides>(&text)
        {
            map.set_overrides(o);
        }
        Self { map, dispatcher: craft_keymap::egui::Dispatcher::new(), recording: None, save: load, filter: String::new(), taken: None }
    }

    /// The commands whose keys were pressed this frame and that `accept` lets through (those are
    /// consumed from the input; the rest stay for the widgets).
    pub fn dispatch_filtered(&mut self, ctx: &egui::Context, accept: impl Fn(&str) -> bool) -> Vec<String> {
        let map = &self.map;
        let d = &mut self.dispatcher;
        ctx.input_mut(|i| d.dispatch_filtered(i, map, &craft_keymap::Active::NONE, |h| accept(&h.command.key()))).into_iter().map(|h| h.command.key()).collect()
    }

    /// The keys bound to a command, as shown to the user.
    pub fn text(&self, id: &str) -> String {
        let b = self.map.bindings(&CommandRef::new(id));
        if b.is_empty() {
            return tl!("none").to_string();
        }
        b.iter().map(|b| b.text(&Scope::app())).collect::<Vec<_>>().join(", ")
    }

    /// While recording: take the next key press as the command's only key. Returns true when a
    /// key was taken (Escape cancels).
    pub fn record(&mut self, ctx: &egui::Context) -> bool {
        let Some(id) = self.recording.clone() else { return false };
        let pressed = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Key { key, modifiers, pressed: true, .. } => Some((*key, *modifiers)),
                _ => None,
            })
        });
        let Some((key, mods)) = pressed else { return false };
        self.recording = None;
        if key == egui::Key::Escape {
            return true;
        }
        let Some(k) = craft_keymap::Key::from_egui(key) else { return true };
        let chord = Chord { mods: craft_keymap::Mods::from_egui(&mods, platform()), key: k };
        let cmd = CommandRef::new(id.clone());
        // a key another command had: say which, so the user knows it moved
        let text = Binding::new(chord).text(&Scope::app());
        self.taken = ids()
            .find(|other| *other != id && self.map.bindings(&CommandRef::new(*other)).iter().any(|b| b.text(&Scope::app()) == text))
            .map(|other| tlf!("{key} was taken from “{command}”", key = text, command = label(other)));
        // The new key replaces the command's keys and is taken from any other command.
        let mut o = self.map.overrides().clone();
        o.insert(cmd.key(), Vec::new());
        self.map.set_overrides(o);
        let _ = self.map.assign(&cmd, Binding::new(chord), Policy::Steal);
        self.persist();
        true
    }

    /// Whether the command matches the shortcut editor's search: its name or one of its keys.
    pub fn matches(&self, id: &str) -> bool {
        let q = self.filter.trim().to_lowercase();
        if q.is_empty() {
            return true;
        }
        let keys = self.text(id).to_lowercase();
        label(id).to_lowercase().contains(&q) || keys.split(", ").any(|k| k == q || k.contains(&q))
    }

    pub fn reset(&mut self, id: &str) {
        self.map.reset(&CommandRef::new(id));
        self.persist();
    }

    pub fn reset_all(&mut self) {
        self.map.reset_all();
        self.persist();
    }

    fn persist(&self) {
        if !self.save {
            return;
        }
        let Some(p) = overrides_path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(self.map.overrides())
            && let Err(e) = craft_atomic::atomic_write(&p, text.as_bytes())
        {
            log::warn!("could not save shortcuts: {e}");
        }
    }
}
