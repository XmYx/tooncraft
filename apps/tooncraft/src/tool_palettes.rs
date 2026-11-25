//! Custom tool palettes and the radial menu. A palette is a named list of registry commands the
//! user puts together (tools, actions, toggles); the Tool Palette panel shows one as a grid of
//! buttons and edits it, and any palette can fill the radial menu, a ring of its commands that
//! opens around the pointer on a key (V by default). Palettes are kept in the preferences.
//!
//! Every entry is a command id from [`crate::commands`], so a palette button has the command's
//! icon, label, key and disabled reason, and runs exactly like the menu item.

use egui::{Context, Id, Key, Pos2, Rect, Sense, Ui, vec2};
use serde::{Deserialize, Serialize};

use crate::app::ToonApp;
use crate::icons::{Icon, IconButton};

/// Most palettes kept.
pub const MAX_PALETTES: usize = 16;
/// Most commands in one palette.
pub const MAX_ITEMS: usize = 32;
/// Most slices the radial menu shows (the palette's first ones).
pub const RADIAL_MAX: usize = 12;
/// Longest palette name, in characters.
pub const MAX_NAME: usize = 48;
/// Commands listed at once while adding.
const ADD_SHOWN: usize = 12;

/// One palette: a name and command ids, in order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Palette {
    pub name: String,
    pub commands: Vec<String>,
}

/// Every palette, the one the panel shows and the one the radial menu uses.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Palettes {
    pub sets: Vec<Palette>,
    pub shown: usize,
    pub radial: usize,
}

impl Default for Palettes {
    fn default() -> Self {
        let ids = |v: &[&str]| v.iter().map(|s| s.to_string()).collect();
        Self {
            sets: vec![
                Palette {
                    name: tl!("Drawing").to_string(),
                    commands: ids(&[
                        "tool.pencil",
                        "tool.ink",
                        "tool.eraser",
                        "tool.fill",
                        "tool.select",
                        "tool.picker",
                        "edit.undo",
                        "edit.redo",
                        "view.onion",
                        "view.light_table",
                        "frame.new_drawing",
                        "view.mirror",
                    ]),
                },
                Palette {
                    name: tl!("Timing").to_string(),
                    commands: ids(&[
                        "frame.play",
                        "frame.prev",
                        "frame.next",
                        "frame.hold_more",
                        "frame.hold_less",
                        "frame.insert",
                        "frame.delete",
                        "timeline.loop",
                    ]),
                },
            ],
            shown: 0,
            radial: 0,
        }
    }
}

impl Palettes {
    /// Inside the caps, every id a known command, no palette without a name (a hand-edited or
    /// damaged preferences file stays usable).
    pub fn sanitized(self) -> Self {
        let mut sets: Vec<Palette> = self
            .sets
            .into_iter()
            .take(MAX_PALETTES)
            .enumerate()
            .map(|(i, p)| {
                let mut commands: Vec<String> = Vec::new();
                for c in p.commands.into_iter().take(MAX_ITEMS * 8) {
                    if commands.len() < MAX_ITEMS && crate::commands::get(&c).is_some() && !commands.contains(&c) {
                        commands.push(c);
                    }
                }
                Palette { name: clean_name(&p.name, i), commands }
            })
            .collect();
        if sets.is_empty() {
            sets = Palettes::default().sets;
        }
        let last = sets.len() - 1;
        Self { shown: self.shown.min(last), radial: self.radial.min(last), sets }
    }

    /// The radial menu's commands (at most [`RADIAL_MAX`]).
    pub fn radial_items(&self) -> Vec<&'static str> {
        self.sets
            .get(self.radial)
            .map(|p| p.commands.iter().filter_map(|c| crate::commands::get(c).map(|c| c.id)).take(RADIAL_MAX).collect())
            .unwrap_or_default()
    }
}

fn clean_name(name: &str, index: usize) -> String {
    let n: String = name.chars().filter(|c| !c.is_control()).take(MAX_NAME).collect();
    let n = n.trim();
    if n.is_empty() { tlf!("Palette {n}", n = index + 1) } else { n.to_string() }
}

fn palettes(app: &ToonApp) -> &Palettes {
    &app.preferences.prefs.palettes
}

/// Apply an edit to the palettes, sanitize and save the preferences (not in tests).
fn edit(app: &mut ToonApp, f: impl FnOnce(&mut Palettes)) {
    let mut p = app.preferences.prefs.palettes.clone();
    f(&mut p);
    let p = p.sanitized();
    if p != app.preferences.prefs.palettes {
        app.preferences.prefs.palettes = p;
        crate::preferences::save_now(app);
    }
}

// ── the panel ──

#[derive(Clone, Default)]
struct PanelState {
    editing: bool,
    search: String,
}

fn panel_state_id() -> Id {
    Id::unique("tooncraft-tool-palette-panel")
}

/// Give a text-only button a spoken name of its own.
fn named(resp: egui::Response, name: String) -> egui::Response {
    let enabled = resp.enabled();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, name.clone()));
    resp
}

/// The Tool Palette panel: the shown palette's buttons; Edit turns it into the editor.
pub fn panel(app: &mut ToonApp, ui: &mut Ui) {
    let mut st: PanelState = ui.data(|d| d.get_temp(panel_state_id())).unwrap_or_default();
    ui.horizontal(|ui| {
        let shown = palettes(app).shown;
        let names: Vec<String> = palettes(app).sets.iter().map(|p| p.name.clone()).collect();
        let mut pick = shown;
        egui::ComboBox::from_id_salt("tool-palette-pick")
            .selected_text(names.get(shown).cloned().unwrap_or_default())
            .show_ui(ui, |ui| {
                for (i, n) in names.iter().enumerate() {
                    ui.selectable_value(&mut pick, i, n);
                }
            })
            .response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, tl!("Palette shown")));
        if pick != shown {
            edit(app, |p| p.shown = pick);
        }
        let full = (names.len() >= MAX_PALETTES).then(|| tl!("No room for more palettes").to_string());
        if ui.add(IconButton::new(Icon::Add, tl!("New palette")).disabled(full)).clicked() {
            edit(app, |p| {
                let n = p.sets.len();
                p.sets.push(Palette { name: tlf!("Palette {n}", n = n + 1), commands: Vec::new() });
                p.shown = n;
            });
            st.editing = true;
        }
        if ui.add(IconButton::new(Icon::Settings, tl!("Edit palette")).active(st.editing)).clicked() {
            st.editing = !st.editing;
        }
    });
    ui.separator();
    if st.editing {
        editor(app, ui, &mut st);
    } else {
        buttons(app, ui);
    }
    ui.data_mut(|d| d.insert_temp(panel_state_id(), st));
}

/// The shown palette as a wrapping grid of command buttons.
fn buttons(app: &mut ToonApp, ui: &mut Ui) {
    let ids: Vec<String> = palettes(app).sets.get(palettes(app).shown).map(|p| p.commands.clone()).unwrap_or_default();
    if ids.is_empty() {
        ui.weak(tl!("This palette is empty: Edit adds commands to it."));
        return;
    }
    let mut run = None;
    ui.horizontal_wrapped(|ui| {
        for id in &ids {
            let Some(c) = crate::commands::get(id) else { continue };
            let label = c.menu_label(app);
            let disabled = (c.enabled)(app).err();
            let resp = match c.icon {
                Some(icon) => {
                    let mut b = IconButton::new(icon, label).shortcut(c.key_text(app)).disabled(disabled).size(28.0);
                    if let Some(on) = c.checked {
                        b = b.active(on(app));
                    }
                    ui.add(b)
                }
                None => ui.add_enabled(disabled.is_none(), egui::Button::new(label)),
            };
            if resp.clicked() {
                run = Some(c.id);
            }
        }
    });
    if let Some(id) = run {
        let ctx = ui.ctx().clone();
        crate::commands::run(app, &ctx, id);
    }
}

/// Rename, reorder, remove and add commands; choose the radial menu's palette; delete.
fn editor(app: &mut ToonApp, ui: &mut Ui, st: &mut PanelState) {
    let shown = palettes(app).shown;
    let Some(pal) = palettes(app).sets.get(shown).cloned() else { return };
    let mut name = pal.name.clone();
    ui.horizontal(|ui| {
        ui.label(tl!("Name"));
        let r = ui.add(egui::TextEdit::singleline(&mut name).char_limit(MAX_NAME).desired_width(160.0));
        r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, tl!("Palette name")));
    });
    if name != pal.name {
        edit(app, |p| {
            if let Some(s) = p.sets.get_mut(shown) {
                s.name = name;
            }
        });
    }
    let mut radial = palettes(app).radial == shown;
    let hint = tlf!("Its first {n} commands, on the radial menu's key", n = RADIAL_MAX);
    if ui.checkbox(&mut radial, tl!("Use in the radial menu")).on_hover_text(hint).changed() && radial {
        edit(app, |p| p.radial = shown);
    }
    ui.separator();
    // the commands, each with earlier, later and remove
    let mut change: Option<(usize, i32)> = None;
    for (i, id) in pal.commands.iter().enumerate() {
        let Some(c) = crate::commands::get(id) else { continue };
        let label = c.label();
        ui.push_id(i, |ui| {
            ui.horizontal(|ui| {
                if let Some(icon) = c.icon {
                    let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                    crate::icons::paint(ui.painter(), r, icon, ui.visuals().text_color(), false);
                }
                ui.label(&label);
                let up = ui.add_enabled(i > 0, egui::Button::new(tl!("Up"))).on_hover_text(tl!("Earlier"));
                if named(up, tlf!("Move {command} earlier", command = label)).clicked() {
                    change = Some((i, -1));
                }
                let down = ui.add_enabled(i + 1 < pal.commands.len(), egui::Button::new(tl!("Down"))).on_hover_text(tl!("Later"));
                if named(down, tlf!("Move {command} later", command = label)).clicked() {
                    change = Some((i, 1));
                }
                if ui.add(IconButton::new(Icon::Delete, tlf!("Remove {command}", command = label))).clicked() {
                    change = Some((i, 0));
                }
            });
        });
    }
    if let Some((i, d)) = change {
        edit(app, |p| {
            if let Some(s) = p.sets.get_mut(shown) {
                if d == 0 {
                    if i < s.commands.len() {
                        s.commands.remove(i);
                    }
                } else {
                    let j = i as i64 + d as i64;
                    if j >= 0 && (j as usize) < s.commands.len() {
                        s.commands.swap(i, j as usize);
                    }
                }
            }
        });
    }
    ui.separator();
    // add a command: search by name
    let full = pal.commands.len() >= MAX_ITEMS;
    ui.add_enabled_ui(!full, |ui| {
        let r = ui.add(egui::TextEdit::singleline(&mut st.search).hint_text(tl!("Add a command…")).desired_width(200.0));
        r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::SearchInput, true, tl!("Find a command to add")));
    });
    if full {
        ui.weak(tlf!("A palette holds up to {n} commands", n = MAX_ITEMS));
    } else if !st.search.trim().is_empty() {
        let q = st.search.trim().to_lowercase();
        let mut add = None;
        for c in crate::commands::ALL.iter().filter(|c| !pal.commands.iter().any(|x| x == c.id) && c.label().to_lowercase().contains(&q)).take(ADD_SHOWN) {
            if ui.button(tlf!("Add {command}", command = c.label())).clicked() {
                add = Some(c.id);
            }
        }
        if let Some(id) = add {
            edit(app, |p| {
                if let Some(s) = p.sets.get_mut(shown) {
                    s.commands.push(id.to_string());
                }
            });
            st.search.clear();
        }
    }
    ui.separator();
    let only = palettes(app).sets.len() <= 1;
    if ui.add_enabled(!only, egui::Button::new(tl!("Delete Palette"))).on_disabled_hover_text(tl!("The last palette stays")).clicked() {
        edit(app, |p| {
            if p.sets.len() > 1 && shown < p.sets.len() {
                p.sets.remove(shown);
                p.shown = shown.saturating_sub(1);
                if p.radial == shown {
                    p.radial = 0;
                } else if p.radial > shown {
                    p.radial -= 1;
                }
            }
        });
        st.editing = false;
    }
}

// ── the radial menu ──

#[derive(Clone, Copy, Debug)]
struct Open {
    centre: Pos2,
    /// The slice the keyboard is on.
    at: usize,
    /// The first frame: focus goes to the first slice, the opening click is not "outside".
    fresh: bool,
}

fn radial_id() -> Id {
    Id::unique("tooncraft-radial-menu")
}

fn slice_id(i: usize) -> Id {
    radial_id().with(("slice", i))
}

/// Is the radial menu open?
#[cfg(test)]
pub fn radial_open(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<Open>(radial_id())).is_some()
}

/// Open the radial menu around the pointer (the canvas centre when the pointer is elsewhere).
pub fn open_radial(app: &mut ToonApp, ctx: &Context) {
    if palettes(app).radial_items().is_empty() {
        app.set_status(tl!("The radial menu's palette is empty: add commands to it in the Tool Palette panel"));
        return;
    }
    let centre = ctx.pointer_latest_pos().filter(|p| ctx.content_rect().contains(*p)).unwrap_or(app.workspace.canvas_rect.center());
    ctx.data_mut(|d| d.insert_temp(radial_id(), Open { centre, at: 0, fresh: true }));
}

pub fn close_radial(ctx: &Context) {
    ctx.data_mut(|d| d.remove::<Open>(radial_id()));
}

/// The ring's radius for `n` slices: room for each slice's button.
fn radius(n: usize) -> f32 {
    (n as f32 * 46.0 / std::f32::consts::TAU).max(64.0)
}

/// Where slice `i` of `n` sits (the first at the top, then clockwise).
pub fn slice_centre(centre: Pos2, i: usize, n: usize) -> Pos2 {
    let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / n.max(1) as f32;
    centre + radius(n) * vec2(a.cos(), a.sin())
}

/// The centre the open radial menu was drawn around last frame.
#[cfg(test)]
pub fn radial_centre(ctx: &Context) -> Option<Pos2> {
    ctx.data(|d| d.get_temp::<Open>(radial_id())).map(|o| o.centre)
}

/// The radial menu, when open: a ring of the radial palette's commands. Click a slice, or move
/// round with the arrow keys and press Enter; Escape or a click outside closes it.
pub fn radial(app: &mut ToonApp, ctx: &Context) {
    let Some(mut open) = ctx.data(|d| d.get_temp::<Open>(radial_id())) else { return };
    let items = palettes(app).radial_items();
    let n = items.len();
    if n == 0 {
        close_radial(ctx);
        return;
    }
    // keep the ring on screen
    let r = radius(n) + 26.0;
    let screen = ctx.content_rect();
    let centre = Pos2::new(
        open.centre.x.clamp(screen.left() + r, (screen.right() - r).max(screen.left() + r)),
        open.centre.y.clamp(screen.top() + r, (screen.bottom() - r).max(screen.top() + r)),
    );
    let bounds = Rect::from_center_size(centre, vec2(2.0 * r, 2.0 * r));
    let (next, prev, escape) = ctx.input(|i| {
        (
            i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::ArrowDown),
            i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::ArrowUp),
            i.key_pressed(Key::Escape),
        )
    });
    if escape {
        close_radial(ctx);
        return;
    }
    let (next, prev) = (next && !open.fresh, prev && !open.fresh);
    if next {
        open.at = (open.at + 1) % n;
    }
    if prev {
        open.at = (open.at + n - 1) % n;
    }
    open.at = open.at.min(n - 1);
    let mut run = None;
    egui::Area::new(radial_id()).order(egui::Order::Foreground).fixed_pos(bounds.min).show(ctx, |ui| {
        ui.set_min_size(bounds.size());
        let pal = crate::theme::palette(!ui.visuals().dark_mode);
        let painter = ui.painter().clone();
        let ring = radius(n) + 24.0;
        painter.circle_filled(centre, ring, pal.panel.gamma_multiply(0.94));
        painter.circle_stroke(centre, ring, egui::Stroke::new(1.0, pal.line));
        let mut hot = None;
        for (i, id) in items.iter().enumerate() {
            let Some(c) = crate::commands::get(id) else { continue };
            let label = c.menu_label(app);
            let enabled = (c.enabled)(app);
            let ok = enabled.is_ok();
            let at = slice_centre(centre, i, n);
            let rect = Rect::from_center_size(at, vec2(40.0, 40.0));
            let resp = ui.interact(rect, slice_id(i), if ok { Sense::click() } else { Sense::focusable_noninteractive() });
            let name = label.clone();
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ok, name.clone()));
            if resp.has_focus() {
                // the arrows move round the ring instead of moving the focus away
                let filter = egui::EventFilter { horizontal_arrows: true, vertical_arrows: true, tab: false, escape: true };
                ui.memory_mut(|m| m.set_focus_lock_filter(resp.id, filter));
            }
            let on = resp.hovered() || resp.has_focus();
            if on {
                hot = Some((label.clone(), enabled.clone().err()));
            }
            painter.circle_filled(at, 19.0, if on && ok { pal.accent.gamma_multiply(0.25) } else { pal.raised });
            if resp.has_focus() {
                painter.circle_stroke(at, 20.0, egui::Stroke::new(2.0, pal.playhead));
            }
            let fg = if !ok {
                pal.weak.gamma_multiply(0.5)
            } else if on {
                pal.accent
            } else {
                pal.text
            };
            let checked = c.checked.is_some_and(|f| f(app));
            match c.icon {
                Some(icon) => crate::icons::paint(&painter, Rect::from_center_size(at, vec2(18.0, 18.0)), icon, fg, checked),
                None => {
                    let initial: String = label.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
                    painter.text(at, egui::Align2::CENTER_CENTER, initial, egui::FontId::proportional(14.0), fg);
                }
            }
            if resp.clicked() {
                run = Some(c.id);
            }
        }
        // the slice under the pointer or the keyboard, named in the middle
        if let Some((label, why)) = hot {
            let text = match why {
                Some(w) => format!("{label}\n{w}"),
                None => label,
            };
            painter.text(centre, egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(12.0), pal.text);
        }
    });
    // the keyboard is on slice `at`
    if open.fresh || next || prev {
        ctx.memory_mut(|m| m.request_focus(slice_id(open.at)));
    }
    if let Some(id) = run {
        close_radial(ctx);
        crate::commands::run(app, ctx, id);
        return;
    }
    let ring = radius(n) + 24.0;
    let outside = !open.fresh && ctx.input(|i| i.pointer.any_click()) && ctx.pointer_latest_pos().is_some_and(|p| (p - centre).length() > ring);
    if outside {
        close_radial(ctx);
        return;
    }
    open.fresh = false;
    open.centre = centre;
    ctx.data_mut(|d| d.insert_temp(radial_id(), open));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostile_palettes_are_capped_cleaned_and_never_empty() {
        let junk = Palettes {
            sets: (0..100)
                .map(|i| Palette {
                    name: if i % 2 == 0 { "\u{7}\n  ".into() } else { "x".repeat(500) },
                    commands: std::iter::repeat_n("tool.pencil".to_string(), 50)
                        .chain(["no.such.command".to_string(), String::new()])
                        .chain(crate::commands::ALL.iter().map(|c| c.id.to_string()))
                        .collect(),
                })
                .collect(),
            shown: usize::MAX,
            radial: 999,
        }
        .sanitized();
        assert_eq!(junk.sets.len(), MAX_PALETTES);
        assert!(junk.shown < MAX_PALETTES && junk.radial < MAX_PALETTES);
        for (i, p) in junk.sets.iter().enumerate() {
            assert!(!p.name.trim().is_empty() && p.name.chars().count() <= MAX_NAME, "{i}: {:?}", p.name);
            assert!(p.commands.len() <= MAX_ITEMS && p.commands.len() > 1);
            assert_eq!(p.commands.iter().filter(|c| *c == "tool.pencil").count(), 1, "no repeats");
            assert!(p.commands.iter().all(|c| crate::commands::get(c).is_some()), "known commands only");
        }
        assert_eq!(junk.radial_items().len(), RADIAL_MAX);
        let empty = Palettes { sets: Vec::new(), shown: 3, radial: 3 }.sanitized();
        assert_eq!(empty.sets, Palettes::default().sets, "no palettes gives the defaults");
        assert_eq!((empty.shown, empty.radial), (1, 1), "clamped to the last one");
        // a damaged file
        let bad: Palettes = serde_json::from_str(r#"{"sets": 5, "radial": -1}"#).unwrap_or_default();
        assert!(!bad.sanitized().sets.is_empty());
    }

    #[test]
    fn the_default_palettes_hold_known_commands() {
        let d = Palettes::default();
        assert_eq!(d.clone().sanitized(), d, "the defaults are already clean");
        assert!(d.radial_items().len() >= 8);
    }

    #[test]
    fn slices_sit_round_the_centre_without_overlapping() {
        let c = Pos2::new(400.0, 300.0);
        for n in 1..=RADIAL_MAX {
            let pts: Vec<Pos2> = (0..n).map(|i| slice_centre(c, i, n)).collect();
            assert!((pts[0].x - c.x).abs() < 0.01 && pts[0].y < c.y, "the first slice is at the top");
            for a in 0..n {
                for b in a + 1..n {
                    assert!((pts[a] - pts[b]).length() >= 40.0, "{n} slices: {a} and {b} overlap");
                }
            }
        }
    }
}
