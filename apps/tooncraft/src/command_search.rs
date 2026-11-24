//! Find Command (Ctrl+Shift+P): type part of a command's name or its key, pick it from the list
//! (arrows and Enter, or a click), and it runs exactly as its menu item or shortcut would. It lists
//! every command of the registry ([`crate::commands`]).

use crate::app::ToonApp;

#[derive(Clone, Debug, Default)]
pub struct State {
    pub open: bool,
    pub text: String,
    /// The highlighted row.
    pub at: usize,
    /// Commands picked here, run at the start of the next frame's shortcuts.
    pub pending: Vec<String>,
}

/// The prefix of the rows that show a panel: `panel.show:<launch name>`.
pub const SHOW_PANEL: &str = "panel.show:";

/// The commands whose name or key matches `text` (all of them for an empty search), as
/// (id, name, key). After the registry come one row per panel, "Window: Show <panel>", so every
/// panel is reachable by typing its name too.
pub fn matches(app: &ToonApp, text: &str) -> Vec<(String, String, String)> {
    let q = text.trim().to_lowercase();
    let window = tl!("Window").to_string();
    let commands = crate::commands::ALL.iter().map(|c| (c.id.to_string(), format!("{}: {}", c.group.label(), c.label()), c.key_text(app)));
    let panels = crate::dock::Pane::ALL.into_iter().filter(|p| *p != crate::dock::Pane::Canvas).map(|p| {
        let v = crate::panel_views::view(p);
        (format!("{SHOW_PANEL}{}", v.id), format!("{window}: {}", tlf!("Show {panel}", panel = v.title())), String::new())
    });
    commands
        .chain(panels)
        .filter(|(_, name, key)| q.is_empty() || q.split_whitespace().all(|w| name.to_lowercase().contains(w) || key.to_lowercase().contains(w)))
        .collect()
}

/// Run a row picked in Find Command: a registry command, or a panel to show.
pub fn run(app: &mut ToonApp, ctx: &egui::Context, id: &str) {
    match id.strip_prefix(SHOW_PANEL) {
        Some(name) => match crate::panel_views::by_name(name) {
            Some(pane) => app.workspace.raise(pane),
            None => log::warn!("Find Command: no panel {name}"),
        },
        None => {
            crate::commands::run(app, ctx, id);
        }
    }
}

pub fn window(app: &mut ToonApp, ctx: &egui::Context) {
    if !app.find.open {
        return;
    }
    let mut open = true;
    let list = matches(app, &app.find.text);
    app.find.at = app.find.at.min(list.len().saturating_sub(1));
    let mut run: Option<String> = None;
    egui::Window::new(tl!("Find Command")).open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_TOP, [0.0, 60.0]).show(ctx, |ui| {
        let l = ui.label(tl!("Command"));
        let r = ui.add(egui::TextEdit::singleline(&mut app.find.text).hint_text(tl!("a name or a key")).desired_width(320.0)).labelled_by(l.id);
        r.request_focus();
        let (down, up, enter) = ui.input(|i| (i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::Enter)));
        if down {
            app.find.at = (app.find.at + 1).min(list.len().saturating_sub(1));
        }
        if up {
            app.find.at = app.find.at.saturating_sub(1);
        }
        if enter && let Some((id, _, _)) = list.get(app.find.at) {
            run = Some(id.clone());
        }
        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            for (i, (id, name, key)) in list.iter().enumerate() {
                let text = format!("{name}   {key}");
                if ui.selectable_label(i == app.find.at, text).clicked() {
                    run = Some(id.clone());
                }
            }
            if list.is_empty() {
                ui.weak(tl!("No command matches"));
            }
        });
    });
    if let Some(id) = run {
        app.find.pending.push(id);
        app.find.open = false;
        app.find.text.clear();
        app.find.at = 0;
    }
    if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.find.open = false;
    }
}
