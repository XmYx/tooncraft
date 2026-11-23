//! The Brushes panel: the brush library (built-in brushes and the user's `.toonbrush` files) with
//! categories, favourites, a list of recent brushes and search.
//!
//! Keyboard: Ctrl+B (command `library.find`) goes to the search field from anywhere; typing filters
//! the list; Down (or Enter) in the field moves into the list; in the list Up / Down / Home / End
//! move, Enter or Space uses the brush, F marks or unmarks a favourite and Escape returns to the
//! search. The category buttons and the buttons at the bottom are ordinary focusable buttons.

use egui::{Id, Key, Sense, Ui, WidgetInfo, vec2};
use tooncraft_brush::BrushKind;
use tooncraft_engine::library::{Library, Query};

use crate::app::{Tool, ToonApp};

/// Which brushes the list shows.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    All,
    Favourites,
    Recent,
    Category(String),
}

/// The panel's state, kept in the app.
pub struct State {
    pub library: Library,
    /// The brush folder is read when the panel is first shown, not at startup.
    pub scanned: bool,
    pub text: String,
    pub filter: Filter,
    /// The id of the highlighted brush.
    pub selected: Option<String>,
    /// Move the keyboard focus to the search field / the list at the next frame.
    pub focus_search: bool,
    pub focus_list: bool,
    /// The fields of "Add Current Brush".
    pub new_name: String,
    pub new_category: String,
}

impl State {
    pub fn new(dir: Option<std::path::PathBuf>) -> Self {
        State {
            library: Library::unscanned(dir),
            scanned: false,
            text: String::new(),
            filter: Filter::All,
            selected: None,
            focus_search: false,
            focus_list: false,
            new_name: String::new(),
            new_category: String::new(),
        }
    }

    /// The ids shown for the current search and filter, in display order.
    pub fn visible(&self) -> Vec<String> {
        match &self.filter {
            Filter::Recent => {
                let words: Vec<String> = self.text.to_lowercase().split_whitespace().map(str::to_string).collect();
                self.library.recent().into_iter().filter(|e| words.iter().all(|w| e.name.to_lowercase().contains(w.as_str()))).map(|e| e.id.clone()).collect()
            }
            f => {
                let q = Query {
                    text: self.text.clone(),
                    category: if let Filter::Category(c) = f { Some(c.clone()) } else { None },
                    favourites_only: *f == Filter::Favourites,
                };
                self.library.search_by(&q, shown_name).into_iter().map(|e| e.id.clone()).collect()
            }
        }
    }
}

pub fn search_id() -> Id {
    Id::unique("library-search")
}

pub fn row_id(entry: &str) -> Id {
    Id::unique("library-row").with(entry)
}

fn filter_label(f: &Filter) -> String {
    match f {
        Filter::All => tl!("All").to_string(),
        Filter::Favourites => tl!("Favourites").to_string(),
        Filter::Recent => tl!("Recent").to_string(),
        Filter::Category(c) => category_label(c),
    }
}

/// A category as shown: the built-in names are translated, a user's folder names are not.
fn category_label(c: &str) -> String {
    match c {
        "Basics" => tl!("Basics").to_string(),
        "Pencils" => tl!("Pencils").to_string(),
        "Inks" => tl!("Inks").to_string(),
        "Airbrushes" => tl!("Airbrushes").to_string(),
        "Chalks" => tl!("Chalks").to_string(),
        "Erasers" => tl!("Erasers").to_string(),
        "Watercolours" => tl!("Watercolours").to_string(),
        "Smudges" => tl!("Smudges").to_string(),
        "My brushes" => tl!("My brushes").to_string(),
        other => other.to_string(),
    }
}

fn builtin_name(n: &str) -> String {
    match n {
        "Pencil" => tl!("Pencil").to_string(),
        "Ink pen" => tl!("Ink pen").to_string(),
        "Airbrush" => tl!("Airbrush").to_string(),
        "Chalk" => tl!("Chalk").to_string(),
        "Eraser" => tl!("Eraser").to_string(),
        "Soft pencil" => tl!("Soft pencil").to_string(),
        "Hard pencil" => tl!("Hard pencil").to_string(),
        "Fine liner" => tl!("Fine liner").to_string(),
        "Brush pen" => tl!("Brush pen").to_string(),
        "Soft airbrush" => tl!("Soft airbrush").to_string(),
        "Spray" => tl!("Spray").to_string(),
        "Dry chalk" => tl!("Dry chalk").to_string(),
        "Wide chalk" => tl!("Wide chalk").to_string(),
        "Hard eraser" => tl!("Hard eraser").to_string(),
        "Soft eraser" => tl!("Soft eraser").to_string(),
        "Watercolour" => tl!("Watercolour").to_string(),
        "Smudge" => tl!("Smudge").to_string(),
        "Wet wash" => tl!("Wet wash").to_string(),
        "Dry wash" => tl!("Dry wash").to_string(),
        "Soft smudge" => tl!("Soft smudge").to_string(),
        "Hard smudge" => tl!("Hard smudge").to_string(),
        other => other.to_string(),
    }
}

/// The name a brush is shown under: built-in names are translated.
pub fn shown_name(e: &tooncraft_engine::library::Entry) -> String {
    if matches!(e.source, tooncraft_engine::library::Source::Builtin(_)) { builtin_name(&e.name) } else { e.name.clone() }
}

/// Use a library brush: it replaces the current brush of its kind, and the tool switches to it.
/// A built-in brush keeps the current colour; a file brush brings its own.
pub fn use_brush(app: &mut ToonApp, id: &str) {
    let Some(entry) = app.library.library.get(id).cloned() else { return };
    match &entry.source {
        tooncraft_engine::library::Source::Builtin(b) => {
            let kind = b.kind;
            *app.brush_mut(kind) = tooncraft_brush::Brush { color: app.color, ..**b };
            if let Some(t) = Tool::ALL.into_iter().find(|t| t.brush() == Some(kind)) {
                app.tool = t;
            }
            app.set_status(tlf!("Brush {name} loaded", name = builtin_name(&entry.name)));
        }
        tooncraft_engine::library::Source::File(path) => crate::panels::use_preset(app, path),
    }
    if let Err(e) = app.library.library.mark_used(id) {
        app.set_status(e);
    }
}

/// A five-point star, filled for a favourite (drawn, so it needs no glyph).
fn draw_star(ui: &Ui, rect: egui::Rect, filled: bool, lit: bool) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.42;
    let pts: Vec<egui::Pos2> = (0..10)
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
            let rr = if i % 2 == 0 { r } else { r * 0.45 };
            c + vec2(a.cos(), a.sin()) * rr
        })
        .collect();
    let color = if lit { ui.visuals().selection.stroke.color } else { ui.visuals().text_color() };
    let fill = if filled { color } else { egui::Color32::TRANSPARENT };
    ui.painter().add(egui::Shape::Path(egui::epaint::PathShape { points: pts, closed: true, fill, stroke: egui::epaint::PathStroke::new(1.4, color) }));
}

fn toggle_favourite(app: &mut ToonApp, id: &str) {
    let on = !app.library.library.is_favourite(id);
    match app.library.library.set_favourite(id, on) {
        Ok(()) => {
            let name = app.library.library.get(id).map(shown_name).unwrap_or_default();
            app.set_status(if on { tlf!("{name} is a favourite", name = name) } else { tlf!("{name} is no longer a favourite", name = name) });
        }
        Err(e) => app.set_status(e),
    }
}

fn kind_label(k: BrushKind) -> String {
    match format!("{k:?}").as_str() {
        "Pencil" => tl!("Pencil"),
        "Ink" => tl!("Ink pen"),
        "Airbrush" => tl!("Airbrush"),
        "Chalk" => tl!("Chalk"),
        "Eraser" => tl!("Eraser"),
        "Watercolour" => tl!("Watercolour"),
        "Smudge" => tl!("Smudge"),
        other => return other.to_string(),
    }
    .to_string()
}

pub fn panel(app: &mut ToonApp, ui: &mut Ui) {
    if !app.library.scanned {
        app.library.scanned = true;
        app.library.library.rescan();
    }
    // search field: its keys are taken before the text field sees them (it would give up focus on
    // Enter and Escape itself)
    let mut surrender = false;
    if ui.memory(|m| m.has_focus(search_id())) {
        let (down, enter, escape) = ui.input_mut(|i| {
            let n = egui::Modifiers::NONE;
            (i.consume_key(n, Key::ArrowDown), i.consume_key(n, Key::Enter), i.consume_key(n, Key::Escape))
        });
        if down || enter {
            app.library.focus_list = true;
        }
        if escape {
            if app.library.text.is_empty() {
                surrender = true;
            } else {
                app.library.text.clear();
            }
        }
    }
    ui.horizontal(|ui| {
        let l = ui.label(tl!("Search brushes"));
        let r = ui.add(egui::TextEdit::singleline(&mut app.library.text).id(search_id()).hint_text(tl!("Name, category or kind")).desired_width(f32::INFINITY));
        let r = r.labelled_by(l.id);
        if r.has_focus() {
            // after the text field's own filter: Escape and Down belong to the panel
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(search_id(), egui::EventFilter { tab: false, horizontal_arrows: false, vertical_arrows: true, escape: true })
            });
        }
        if app.library.focus_search {
            r.request_focus();
            app.library.focus_search = false;
        }
        if surrender {
            r.surrender_focus();
        }
    });
    // filters
    let mut filters = vec![Filter::All, Filter::Favourites, Filter::Recent];
    filters.extend(app.library.library.categories().into_iter().map(Filter::Category));
    ui.horizontal_wrapped(|ui| {
        for f in filters {
            let selected = app.library.filter == f;
            let name = filter_label(&f);
            let r = ui.selectable_label(selected, &name);
            let label = tlf!("Show {category} brushes", category = name);
            r.widget_info(|| WidgetInfo::selected(egui::Role::Button, true, selected, label.clone()));
            if r.clicked() {
                app.library.filter = f;
            }
        }
    });
    ui.separator();
    // the list
    let ids = app.library.visible();
    if app.library.selected.as_ref().is_none_or(|s| !ids.contains(s)) {
        app.library.selected = ids.first().cloned();
    }
    let count_label = tlf!("{n} brushes", n = ids.len());
    ui.weak(count_label);
    let mut move_to: Option<usize> = None;
    let focused_row = ids.iter().position(|id| ui.memory(|m| m.has_focus(row_id(id))));
    let height = (ui.available_height() - 70.0).max(80.0);
    egui::ScrollArea::vertical().id_salt("library-list").max_height(height).show(ui, |ui| {
        for (i, id) in ids.iter().enumerate() {
            let Some(entry) = app.library.library.get(id).cloned() else { continue };
            let favourite = app.library.library.is_favourite(id);
            let selected = app.library.selected.as_deref() == Some(id.as_str());
            ui.horizontal(|ui| {
                let name = if matches!(entry.source, tooncraft_engine::library::Source::Builtin(_)) { builtin_name(&entry.name) } else { entry.name.clone() };
                let (star_rect, s) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                draw_star(ui, star_rect, favourite, s.hovered() || s.has_focus());
                let star_label = if favourite { tlf!("Remove {name} from favourites", name = name) } else { tlf!("Add {name} to favourites", name = name) };
                s.widget_info(|| WidgetInfo::selected(egui::Role::CheckBox, true, favourite, star_label.clone()));
                if s.clicked() {
                    toggle_favourite(app, id);
                }
                let r = ui.interact(ui.available_rect_before_wrap().with_max_y(ui.available_rect_before_wrap().min.y + 22.0), row_id(id), Sense::click());
                let text = format!("{name}  ·  {}", category_label(&entry.category));
                let rect = r.rect;
                let visuals = ui.style().interact_selectable(&r, selected);
                if selected || r.hovered() {
                    ui.painter().rect_filled(rect, 3.0, visuals.bg_fill);
                }
                ui.painter().text(
                    rect.left_center() + vec2(4.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    &text,
                    egui::FontId::proportional(13.0),
                    visuals.fg_stroke.color,
                );
                ui.painter().text(
                    rect.right_center() - vec2(4.0, 0.0),
                    egui::Align2::RIGHT_CENTER,
                    kind_label(entry.kind),
                    egui::FontId::proportional(11.0),
                    ui.visuals().weak_text_color(),
                );
                if r.has_focus() {
                    ui.painter().rect_stroke(rect, 3.0, ui.visuals().selection.stroke, egui::StrokeKind::Inside);
                    ui.memory_mut(|m| {
                        m.set_focus_lock_filter(r.id, egui::EventFilter { tab: false, horizontal_arrows: true, vertical_arrows: true, escape: true })
                    });
                }
                let label = if favourite {
                    tlf!("{name}, {category}, favourite", name = name, category = category_label(&entry.category))
                } else {
                    tlf!("{name}, {category}", name = name, category = category_label(&entry.category))
                };
                r.widget_info(|| WidgetInfo::selected(egui::Role::Button, true, selected, label.clone()));
                if r.clicked() {
                    app.library.selected = Some(id.clone());
                    r.request_focus();
                    use_brush(app, id);
                }
                if r.has_focus() {
                    app.library.selected = Some(id.clone());
                    ui.scroll_to_rect(rect, None);
                }
            });
            let _ = i;
        }
    });
    // keyboard in the list
    if let Some(cur) = focused_row {
        let (up, down, home, end, enter, fav, esc) = ui.input_mut(|i| {
            let n = egui::Modifiers::NONE;
            (
                i.consume_key(n, Key::ArrowUp),
                i.consume_key(n, Key::ArrowDown),
                i.consume_key(n, Key::Home),
                i.consume_key(n, Key::End),
                i.consume_key(n, Key::Enter) | i.consume_key(n, Key::Space),
                i.consume_key(n, Key::F),
                i.consume_key(n, Key::Escape),
            )
        });
        if up {
            move_to = Some(cur.saturating_sub(1));
        }
        if down {
            move_to = Some((cur + 1).min(ids.len().saturating_sub(1)));
        }
        if home {
            move_to = Some(0);
        }
        if end {
            move_to = Some(ids.len().saturating_sub(1));
        }
        if enter && let Some(id) = ids.get(cur) {
            use_brush(app, id);
        }
        if fav && let Some(id) = ids.get(cur) {
            toggle_favourite(app, id);
        }
        if esc {
            app.library.focus_search = true;
        }
    }
    if let Some(i) = move_to
        && let Some(id) = ids.get(i)
    {
        ui.memory_mut(|m| m.request_focus(row_id(id)));
    }
    if app.library.focus_list {
        app.library.focus_list = false;
        if let Some(id) = app.library.selected.clone().or_else(|| ids.first().cloned()) {
            ui.memory_mut(|m| m.request_focus(row_id(&id)));
        }
    }
    ui.separator();
    // add and import
    ui.horizontal_wrapped(|ui| {
        let l = ui.label(tl!("Name"));
        ui.add(egui::TextEdit::singleline(&mut app.library.new_name).desired_width(90.0)).labelled_by(l.id);
        let l = ui.label(tl!("Category"));
        ui.add(egui::TextEdit::singleline(&mut app.library.new_category).desired_width(90.0)).labelled_by(l.id);
        let can = app.library.library.dir().is_some();
        if ui.add_enabled(can, egui::Button::new(tl!("Add Current Brush"))).clicked() {
            add_current(app);
        }
        if ui.add_enabled(can, egui::Button::new(tl!("Import Brush…"))).clicked()
            && let Some(f) = rfd::FileDialog::new().add_filter(tl!("Brush preset"), &[tooncraft_engine::preset::EXTENSION]).pick_file()
        {
            match app.library.library.import(&f, &app.library.new_category.clone()) {
                Ok(id) => {
                    app.library.selected = Some(id);
                    app.set_status(tl!("Brush imported"));
                }
                Err(e) => app.set_status(e),
            }
        }
        if ui.button(tl!("Reload Library")).clicked() {
            app.library.library.rescan();
        }
    });
}

/// Save the current tool's brush into the library.
fn add_current(app: &mut ToonApp) {
    let Some(kind) = app.tool.brush() else {
        app.set_status(tl!("Choose a brush tool first"));
        return;
    };
    let brush = tooncraft_brush::Brush { color: app.color, ..*app.brush_mut(kind) };
    let tip = brush.tip.and_then(|i| app.engine.project.tips.get(&i)).map(|t| (**t).clone());
    let name = if app.library.new_name.trim().is_empty() { kind_label(kind) } else { app.library.new_name.trim().to_string() };
    let category = app.library.new_category.clone();
    match app.library.library.add(&name, &category, &brush, tip.as_ref()) {
        Ok(id) => {
            app.library.selected = Some(id);
            app.set_status(tlf!("Saved brush {name}", name = name));
        }
        Err(e) => app.set_status(e),
    }
}

#[cfg(test)]
mod tests {
    use egui::{Key, Modifiers};
    use egui_kittest::Harness;
    use egui_kittest::kittest::{By, Queryable};

    use super::*;
    use crate::args::Cli;
    use crate::dock::Pane;

    fn temp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("tooncraft-ui-library-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn harness(dir: &std::path::Path) -> Harness<'static, ToonApp> {
        let mut h = harness_closed(dir);
        h.state_mut().workspace.raise(Pane::Library);
        h.run();
        h.run();
        h
    }

    /// The window with the library panel still behind another tab.
    fn harness_closed(dir: &std::path::Path) -> Harness<'static, ToonApp> {
        let ctx = egui::Context::default();
        let mut app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
        app.library = State::new(Some(dir.to_path_buf()));
        Harness::builder().with_size(egui::vec2(1500.0, 2000.0)).with_accessibility_check(true).build_ui_state(
            |ui, app: &mut ToonApp| {
                let ctx = ui.ctx().clone();
                app.tick(&ctx);
                app.draw(ui);
            },
            app,
        )
    }

    fn names(h: &Harness<'static, ToonApp>) -> Vec<String> {
        let s = &h.state().library;
        s.visible().iter().filter_map(|id| s.library.get(id)).map(|e| e.name.clone()).collect()
    }

    fn row_focused(h: &Harness<'static, ToonApp>, id: &str) -> bool {
        h.ctx.memory(|m| m.has_focus(row_id(id)))
    }

    #[test]
    fn the_panel_lists_named_rows_and_categories() {
        let h = harness(&temp("names"));
        h.get_by_label("Search brushes");
        h.get_by_label("Show All brushes");
        h.get_by_label("Show Favourites brushes");
        h.get_by_label("Show Recent brushes");
        h.get_by_label("Show Inks brushes");
        h.get(By::new().role(egui::Role::Button).label("Fine liner, Inks"));
        h.get(By::new().role(egui::Role::CheckBox).label("Add Fine liner to favourites"));
    }

    #[test]
    fn the_brush_folder_is_read_when_the_panel_first_opens() {
        let dir = temp("lazy");
        Library::new(Some(dir.clone())).add("Mine", "Cat", &tooncraft_brush::Brush::default(), None).unwrap();
        let mut h = harness_closed(&dir);
        h.run();
        assert!(!h.state().library.scanned);
        assert!(h.state().library.library.get("file:Cat/Mine.toonbrush").is_none(), "startup does not read the folder");
        h.state_mut().workspace.raise(Pane::Library);
        h.run();
        h.run();
        assert!(h.state().library.scanned);
        h.get(By::new().role(egui::Role::Button).label("Mine, Cat"));
    }

    #[test]
    fn typing_filters_the_list() {
        let mut h = harness(&temp("search"));
        h.get_by_label("Search brushes").focus();
        h.run();
        h.event(egui::Event::Text("chalk".into()));
        h.run();
        assert_eq!(names(&h), vec!["Chalk", "Dry chalk", "Wide chalk"]);
        assert!(h.query(By::new().role(egui::Role::Button).label_contains("Hard pencil")).is_none());
        // Escape clears the text, a second Escape leaves the field
        assert!(h.ctx.memory(|m| m.has_focus(search_id())), "the search field has focus while typing");
        h.key_press(Key::Escape);
        h.run();
        assert!(h.state().library.text.is_empty());
        assert!(names(&h).len() >= 15);
    }

    #[test]
    fn find_pick_and_use_a_brush_without_the_mouse() {
        let dir = temp("keys");
        let mut h = harness(&dir);
        h.state_mut().color = [10, 20, 30, 255];
        // Ctrl+B from anywhere goes to the search field
        h.key_press_modifiers(Modifiers::CTRL, Key::B);
        h.run();
        h.run();
        assert!(h.ctx.memory(|m| m.has_focus(search_id())), "Ctrl+B focuses the search field");
        h.event(egui::Event::Text("chalk".into()));
        h.run();
        // Down enters the list on the first match; Down again moves; Enter uses the brush
        h.key_press(Key::ArrowDown);
        h.run();
        h.run();
        assert!(row_focused(&h, "builtin:Chalk"));
        h.key_press(Key::ArrowDown);
        h.run();
        h.run();
        assert!(row_focused(&h, "builtin:Dry chalk"));
        h.key_press(Key::Enter);
        h.run();
        let b = h.state().brushes[3];
        let want = tooncraft_engine::library::builtin().into_iter().find(|b| b.0 == "Dry chalk").unwrap().2;
        assert_eq!(tooncraft_brush::Brush { color: [10, 20, 30, 255], ..want }, b, "the brush replaced the chalk brush and the colour was kept");
        assert_eq!(h.state().tool, Tool::Chalk);
        assert_eq!(h.state().library.library.recent().iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), vec!["Dry chalk"]);
        // End goes to the last match, Home to the first, Escape back to the search field
        h.key_press(Key::End);
        h.run();
        h.run();
        assert!(row_focused(&h, "builtin:Wide chalk"));
        h.key_press(Key::Home);
        h.run();
        h.run();
        assert!(row_focused(&h, "builtin:Chalk"));
        h.key_press(Key::Escape);
        h.run();
        h.run();
        assert!(h.ctx.memory(|m| m.has_focus(search_id())));
    }

    #[test]
    fn favourites_by_key_and_by_click_survive_a_restart() {
        let dir = temp("fav");
        let mut h = harness(&dir);
        h.ctx.memory_mut(|m| m.request_focus(row_id("builtin:Fine liner")));
        h.run();
        h.key_press(Key::F);
        h.run();
        assert!(h.state().library.library.is_favourite("builtin:Fine liner"));
        h.get(By::new().role(egui::Role::CheckBox).label("Remove Fine liner from favourites"));
        h.get(By::new().role(egui::Role::Button).label("Fine liner, Inks, favourite"));
        h.get(By::new().role(egui::Role::CheckBox).label("Add Spray to favourites")).click();
        h.run();
        assert!(h.state().library.library.is_favourite("builtin:Spray"));
        h.get_by_label("Show Favourites brushes").click();
        h.run();
        assert_eq!(names(&h), vec!["Fine liner", "Spray"]);
        // a new library over the same folder sees them
        let again = Library::new(Some(dir));
        assert!(again.is_favourite("builtin:Fine liner") && again.is_favourite("builtin:Spray"));
    }

    #[test]
    fn categories_and_recent() {
        let dir = temp("cats");
        let mut h = harness(&dir);
        h.get_by_label("Show Inks brushes").click();
        h.run();
        assert_eq!(names(&h), vec!["Fine liner", "Brush pen"]);
        h.get(By::new().role(egui::Role::Button).label("Brush pen, Inks")).click();
        h.run();
        h.get(By::new().role(egui::Role::Button).label("Fine liner, Inks")).click();
        h.run();
        h.get_by_label("Show Recent brushes").click();
        h.run();
        assert_eq!(names(&h), vec!["Fine liner", "Brush pen"], "newest first");
        assert_eq!(h.state().tool, Tool::Ink);
        assert_eq!(h.state().brush().size, 2.5);
        // a saved brush shows up under its category
        let brush = tooncraft_brush::Brush { size: 31.0, ..tooncraft_brush::Brush::preset(BrushKind::Chalk) };
        h.state_mut().library.library.add("Dusty", "Textures", &brush, None).unwrap();
        h.run();
        h.get_by_label("Show Textures brushes").click();
        h.run();
        assert_eq!(names(&h), vec!["Dusty"]);
        h.get(By::new().role(egui::Role::Button).label("Dusty, Textures")).click();
        h.run();
        assert_eq!(h.state().brushes[3].size, 31.0);
        assert_eq!(h.state().tool, Tool::Chalk);
    }
}
