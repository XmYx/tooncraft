//! The panel registry: what each dockable panel is (its launch name, glyph, title, the commands at
//! the top of its `⋯` menu, and the body it draws). The dock, the tab chrome, the Window menu and
//! `--panel` all read it, so a new panel is one module plus one arm in [`view`].

use egui::Ui;

use crate::app::ToonApp;
use crate::dock::Pane;
use crate::icons::Icon;

/// Marks a source text for the string catalog; titles are translated when shown.
macro_rules! tlm {
    ($s:literal) => {
        $s
    };
}

/// One dockable panel.
#[derive(Clone, Copy)]
pub struct PanelView {
    /// The launch name (`--panel timeline`).
    pub id: &'static str,
    /// Other names `--panel` accepts.
    pub aliases: &'static [&'static str],
    pub icon: Icon,
    /// English source text; [`PanelView::title`] translates it.
    title: &'static str,
    /// The panel's own commands at the top of its `⋯` menu.
    pub menu: Option<fn(&mut ToonApp, &mut Ui)>,
    /// The panel's body.
    pub ui: fn(&mut ToonApp, &mut Ui),
}

impl PanelView {
    pub fn title(&self) -> String {
        tl!(self.title).to_string()
    }
}

const fn panel(id: &'static str, aliases: &'static [&'static str], icon: Icon, title: &'static str, ui: fn(&mut ToonApp, &mut Ui)) -> PanelView {
    PanelView { id, aliases, icon, title, menu: None, ui }
}

const fn with_menu(mut v: PanelView, menu: fn(&mut ToonApp, &mut Ui)) -> PanelView {
    v.menu = Some(menu);
    v
}

/// The view of `pane`. The match is exhaustive: a new `Pane` does not build until it has a view.
pub fn view(pane: Pane) -> PanelView {
    match pane {
        Pane::Canvas => panel("canvas", &[], Icon::Canvas, tlm!("Canvas"), |app, ui| {
            crate::canvas_header::strip(app, ui);
            crate::canvas::show(app, ui)
        }),
        Pane::Timeline => with_menu(panel("timeline", &[], Icon::Timeline, tlm!("Timeline"), crate::timeline::show), |app, ui| {
            ui.checkbox(&mut app.tl.vertical, tl!("Vertical Exposure Sheet")).on_hover_text(tl!("Frames down the page, layers as columns"));
        }),
        Pane::Colour => with_menu(panel("colour", &["color"], Icon::Colour, tlm!("Colour"), crate::panels::colour), |app, ui| {
            ui.checkbox(&mut app.colour.wheel, tl!("Hue Wheel"));
            ui.checkbox(&mut app.colour.window, tl!("Picker Window"));
        }),
        Pane::Brush => panel("brush", &[], Icon::Brush, tlm!("Brush"), crate::panels::brush),
        Pane::Layers => panel("layers", &[], Icon::Layers, tlm!("Layers"), crate::panels::layers),
        Pane::Onion => with_menu(panel("onion", &[], Icon::Onion, tlm!("Onion Skin"), crate::panels::onion), |app, ui| {
            ui.checkbox(&mut app.engine.project.onion.enabled, tl!("Onion Skin"));
        }),
        Pane::Camera => panel("camera", &[], Icon::Camera, tlm!("Camera"), crate::panels::camera),
        Pane::Pegs => panel("pegs", &[], Icon::Peg, tlm!("Pegs"), crate::pegs::panel),
        Pane::Library => panel("library", &["brushes"], Icon::Library, tlm!("Brushes"), crate::brush_library::panel),
        Pane::Effects => panel("effects", &[], Icon::Effects, tlm!("Effects"), crate::effects_panel::show),
        Pane::Sound => panel("sound", &[], Icon::Sound, tlm!("Sound"), crate::sound_panel::show),
        Pane::LipSync => panel("lipsync", &["lip-sync"], Icon::LipSync, tlm!("Lip Sync"), crate::lipsync_panel::show),
        Pane::Timing => panel("timing", &[], Icon::Timing, tlm!("Timing"), crate::timing_panel::panel),
        Pane::History => panel("history", &[], Icon::History, tlm!("Undo History"), crate::history_panel::panel),
        Pane::Drawings => panel("drawings", &["drawing-library"], Icon::Drawings, tlm!("Drawing Library"), crate::drawing_library::panel),
        Pane::Curves => panel("curves", &["curve"], Icon::Curves, tlm!("Curves"), crate::curves::panel),
        Pane::Palette => panel("palette", &["tool-palette"], Icon::ToolPalette, tlm!("Tool Palette"), crate::tool_palettes::panel),
        Pane::ToolProperties => panel("tool-properties", &["properties"], Icon::Settings, tlm!("Tool Properties"), crate::tool_options::panel),
    }
}

/// The panel a launch name means (its id or an alias, any case).
pub fn by_name(name: &str) -> Option<Pane> {
    let n = name.to_ascii_lowercase();
    Pane::ALL.into_iter().find(|p| {
        let v = view(*p);
        v.id == n || v.aliases.contains(&n.as_str())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_panel_has_a_view_with_its_own_name_and_glyph() {
        let mut names = HashSet::new();
        let mut titles = HashSet::new();
        for p in Pane::ALL {
            let v = view(p);
            assert!(!v.title().trim().is_empty(), "{p:?} has no title");
            assert!(titles.insert(v.title()), "{p:?}: two panels share a title");
            for n in std::iter::once(v.id).chain(v.aliases.iter().copied()) {
                assert!(names.insert(n), "{n} names two panels");
                assert_eq!(by_name(n), Some(p), "{n}");
                assert_eq!(by_name(&n.to_uppercase()), Some(p), "{n} in capitals");
            }
            assert_eq!(crate::icons::for_pane(p), v.icon);
        }
        assert_eq!(by_name("no-such-panel"), None);
    }

    #[test]
    fn the_documented_launch_names_resolve() {
        // the --panel names in args.rs USAGE
        for n in [
            "timeline",
            "layers",
            "brush",
            "colour",
            "onion",
            "camera",
            "pegs",
            "library",
            "effects",
            "sound",
            "lipsync",
            "timing",
            "history",
            "drawings",
            "curves",
            "palette",
            "tool-properties",
        ] {
            assert!(by_name(n).is_some(), "{n}");
            assert!(crate::args::USAGE.contains(n), "{n} is documented");
        }
    }

    #[test]
    fn every_panel_body_and_menu_draws_through_the_registry() {
        use egui_kittest::Harness;
        use egui_kittest::kittest::Queryable;
        let ctx = egui::Context::default();
        let app = ToonApp::with_context(&ctx, crate::args::Cli::default()).without_autosave();
        let mut h = Harness::builder().with_size(egui::vec2(900.0, 700.0)).with_accessibility_check(true).build_ui_state(
            |ui, app: &mut ToonApp| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for p in Pane::ALL.into_iter().filter(|p| *p != Pane::Canvas) {
                        let v = view(p);
                        ui.push_id(v.id, |ui| {
                            ui.heading(v.title());
                            if let Some(menu) = v.menu {
                                menu(app, ui);
                            }
                            (v.ui)(app, ui);
                        });
                    }
                });
            },
            app,
        );
        h.run();
        // the menus' own commands are there, named
        h.get_by_label("Vertical Exposure Sheet");
        h.get_by_label("Hue Wheel");
    }
}
