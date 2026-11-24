//! UI tests for the shell chrome (UI pass stream S2): the top bar's workspace segments, command
//! search and undo/redo; Window ▸ Workspace with Save As and Reset; the tool rail and the tool
//! options bar. Keys are pressed as keys; the accessibility check is on. Nothing is written to
//! the user's settings (workspaces and preferences are not saved in tests).

use egui::{Key, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, NodeT, Queryable};

use crate::app::{SelectMode, Tool, ToonApp};
use crate::args::Cli;
use crate::dock::Pane;
use crate::workspaces::Kind;

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(vec2(1700.0, 1050.0)).with_accessibility_check(true).with_max_steps(8).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    h.run();
    h
}

/// Focus the workspace segment `kind` and press Space on it.
fn pick_workspace(h: &mut Harness<'static, ToonApp>, kind: Kind) {
    h.get(By::new().role(egui::Role::RadioButton).label(&format!("{} workspace", kind.label()))).focus();
    h.run();
    h.key_press(Key::Space);
    h.run();
}

#[test]
fn the_workspace_segments_switch_by_keyboard_and_keep_each_workspaces_edits() {
    let mut h = harness();
    assert_eq!(h.state().workspaces.active, Some(Kind::Draw));
    let draw = h.get(By::new().role(egui::Role::RadioButton).label("Draw workspace"));
    assert_eq!(draw.accesskit_node().toggled(), Some(egui::accesskit::Toggled::True), "the active workspace reads as chosen");
    pick_workspace(&mut h, Kind::Time);
    assert_eq!(h.state().workspaces.active, Some(Kind::Time));
    assert_eq!(h.state().workspace.layout, Kind::Time.preset());
    // an edit to Time: close the pegs panel
    assert!(h.state_mut().workspace.close(Pane::Pegs));
    h.run();
    pick_workspace(&mut h, Kind::Sheet);
    assert!(h.state().tl.vertical, "the sheet workspace shows the vertical sheet");
    pick_workspace(&mut h, Kind::Draw);
    assert!(!h.state().tl.vertical, "back to the horizontal timeline");
    assert_eq!(h.state().workspace.layout, Kind::Draw.preset());
    pick_workspace(&mut h, Kind::Time);
    assert!(!h.state().workspace.is_open(Pane::Pegs), "Time comes back as it was left");
    assert!(h.state().workspaces.is_edited(Kind::Time));
    assert!(!h.state().workspaces.is_edited(Kind::Draw));
}

#[test]
fn reset_workspace_returns_to_the_preset_from_the_window_menu() {
    let mut h = harness();
    pick_workspace(&mut h, Kind::InkPaint);
    assert!(h.state_mut().workspace.close(Pane::Brush));
    h.run();
    h.get_by_label("Window").click();
    h.run();
    h.get_by_label("Workspace").click();
    h.run();
    let ink = h.get(By::new().role(egui::Role::RadioButton).label("Ink & Paint"));
    assert_eq!(ink.accesskit_node().toggled(), Some(egui::accesskit::Toggled::True), "the menu marks the active workspace");
    h.get(By::new().role(egui::Role::Button).label("Reset Workspace")).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(h.state().workspace.layout, Kind::InkPaint.preset());
    assert!(!h.state().workspaces.is_edited(Kind::InkPaint));
}

#[test]
fn save_workspace_as_keeps_the_layout_under_a_typed_name() {
    let mut h = harness();
    pick_workspace(&mut h, Kind::Time);
    assert!(h.state_mut().workspace.close(Pane::Camera));
    h.run();
    let layout = h.state().workspace.layout.clone();
    h.get_by_label("Window").click();
    h.run();
    h.get_by_label("Workspace").click();
    h.run();
    h.get(By::new().role(egui::Role::TextInput).label("Save Workspace As…")).focus();
    h.run();
    h.get(By::new().role(egui::Role::TextInput).label("Save Workspace As…")).type_text("Timing, no camera");
    h.run();
    h.key_press(Key::Enter);
    h.run();
    let saved = &h.state().canvas_ui.workspaces;
    assert!(
        saved.iter().any(|(n, l)| n == "Timing, no camera" && *l == layout),
        "saved under its name: {:?}",
        saved.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );
    // switch away and pick it by name
    pick_workspace(&mut h, Kind::Draw);
    crate::workspaces::use_named(h.state_mut(), "Timing, no camera");
    h.run();
    assert_eq!(h.state().workspace.layout, layout);
    assert_eq!(h.state().workspaces.active, None, "a named layout is not one of the four");
}

#[test]
fn the_top_bar_has_search_and_undo_redo_by_keyboard() {
    let mut h = harness();
    // undo is there and disabled with nothing to undo
    let undo = h.get(By::new().role(egui::Role::Button).label("Undo step"));
    assert!(undo.accesskit_node().is_disabled());
    assert!(h.state_mut().run(tooncraft_engine::Command::NewDrawing { layer: 0, frame: 0 }));
    h.run();
    h.get(By::new().role(egui::Role::Button).label("Undo step")).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert!(h.state().engine.can_redo(), "Enter on Undo undid the new drawing");
    h.get(By::new().role(egui::Role::Button).label("Search commands")).focus();
    h.run();
    h.key_press(Key::Space);
    h.run();
    assert!(h.state().find.open, "search opens Find Command");
}

#[test]
fn a_rail_flyout_opens_from_the_keyboard_and_the_group_keeps_the_variant_used_last() {
    let mut h = harness();
    h.key_press(Key::P);
    h.run();
    assert_eq!(h.state().tool, Tool::Pencil);
    // Shift+F10 on the focused draw group opens its flyout; the focus moves into it
    h.get(By::new().role(egui::Role::Button).label("Pencil (P)")).focus();
    h.run();
    h.key_press_modifiers(egui::Modifiers::SHIFT, Key::F10);
    h.run();
    h.run();
    assert!(egui::Popup::is_id_open(&h.ctx, crate::tool_rail::flyout_id(2)), "the draw flyout is open");
    for name in ["Ink pen (N)", "Airbrush (A)", "Chalk (C)", "Watercolour (W)", "Reset Tool Settings", "Shortcut…"] {
        h.get(By::new().role(egui::Role::Button).label(name));
    }
    // Tab to the ink pen, Space picks it
    h.key_press(Key::Tab);
    h.run();
    h.key_press(Key::Space);
    h.run();
    assert_eq!(h.state().tool, Tool::Ink);
    assert_eq!(h.state().preferences.prefs.tool_rail.get("draw").map(String::as_str), Some("ink"));
    // another tool: the draw group still shows the ink pen, and Enter on it takes it back
    h.key_press(Key::E);
    h.run();
    assert_eq!(h.state().tool, Tool::Eraser);
    h.get(By::new().role(egui::Role::Button).label("Ink pen (N)")).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(h.state().tool, Tool::Ink);
}

#[test]
fn the_select_group_holds_the_ways_of_selecting_and_the_transform_group_moves_a_selection() {
    let mut h = harness();
    h.get(By::new().role(egui::Role::Button).label("Lasso select (S)")).focus();
    h.run();
    h.key_press(Key::Menu);
    h.run();
    h.run();
    for name in ["Rectangle select (S)", "Ellipse select (S)", "Magic wand (S)"] {
        h.get(By::new().role(egui::Role::Button).label(name));
    }
    h.get(By::new().role(egui::Role::Button).label("Ellipse select (S)")).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert_eq!((h.state().tool, h.state().select_mode), (Tool::Select, SelectMode::Ellipse));
    // the transform group: the selection transform, then the layer peg
    h.get(By::new().role(egui::Role::Button).label("Transform selection (S)")).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert!(h.state().rail.transform_only);
    assert_eq!(h.state().tool, Tool::Select);
    // the select key gives back the ordinary select tool (single keys work with nothing focused)
    h.key_press(Key::Escape);
    h.run();
    h.key_press(Key::S);
    h.run();
    assert!(!h.state().rail.transform_only);
    assert_eq!(h.state().select_mode, SelectMode::Ellipse);
}

#[test]
fn the_view_tools_zoom_and_turn_the_view() {
    let mut h = harness();
    h.key_press(Key::Z);
    h.run();
    assert_eq!(h.state().tool, Tool::Zoom);
    h.get(By::new().role(egui::Role::Button).label("Zoom view (Z)"));
    let canvas = h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).rect().center();
    let before = h.state().view.zoom;
    h.hover_at(canvas);
    h.run();
    h.drag_at(canvas);
    h.run();
    h.drop_at(canvas);
    h.run();
    assert!(h.state().view.zoom > before * 1.2, "a click zoomed in: {} → {}", before, h.state().view.zoom);
    h.key_press(Key::R);
    h.run();
    assert_eq!(h.state().tool, Tool::RotateView);
    h.hover_at(canvas + vec2(120.0, 0.0));
    h.run();
    h.drag_at(canvas + vec2(120.0, 0.0));
    h.run();
    for k in 1..=8 {
        let a = k as f32 / 8.0 * std::f32::consts::FRAC_PI_2;
        h.hover_at(canvas + vec2(120.0 * a.cos(), 120.0 * a.sin()));
        h.run();
    }
    h.drop_at(canvas + vec2(0.0, 120.0));
    h.run();
    assert!(h.state().view.rotation_degrees().abs() > 30.0, "the drag turned the view: {}", h.state().view.rotation_degrees());
}

#[test]
fn the_colour_chips_swap_back_to_the_previous_colour() {
    let mut h = harness();
    h.state_mut().color = [10, 20, 30, 255];
    h.run();
    h.state_mut().color = [200, 100, 0, 255];
    h.run();
    h.get(By::new().role(egui::Role::Button).label("Swap to the previous colour #0A141E")).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(&h.state().color[..3], &[10, 20, 30]);
}

#[test]
fn the_options_bar_scrub_fields_take_keys_and_typed_percentages() {
    let mut h = harness();
    h.key_press(Key::P);
    h.run();
    let before = h.state().brushes[0].size;
    h.get(By::new().role(egui::Role::SpinButton).label("Size")).focus();
    h.run();
    for _ in 0..3 {
        h.key_press(Key::ArrowUp);
        h.run();
    }
    assert!(h.state().brushes[0].size > before, "arrow keys grow the size");
    // opacity is typed as a percentage
    // the bar's opacity is the topmost of the "Opacity" boxes (the layers have their own)
    fn top_opacity<'h>(h: &'h Harness<'static, ToonApp>) -> egui_kittest::Node<'h> {
        h.query_all(By::new().role(egui::Role::SpinButton).label("Opacity")).min_by(|a, b| a.rect().top().total_cmp(&b.rect().top())).expect("an opacity box")
    }
    top_opacity(&h).focus();
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
    h.run();
    top_opacity(&h).type_text("40");
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert!((h.state().brushes[0].opacity - 0.4).abs() < 1e-4, "40 % → {}", h.state().brushes[0].opacity);
    // the bars are named for assistive technology too
    h.get(By::new().role(egui::Role::Slider).label("Size bar"));
    h.get(By::new().role(egui::Role::Slider).label("Smoothing bar"));
}

#[test]
fn the_options_bar_starts_with_the_tool_and_its_group() {
    let mut h = harness();
    h.key_press(Key::P);
    h.run();
    h.get(By::new().role(egui::Role::ComboBox).label("Tool: Pencil")).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    h.get_by_label_contains("Watercolour").click();
    h.run();
    assert_eq!(h.state().tool, Tool::Watercolour);
    assert_eq!(h.state().preferences.prefs.tool_rail.get("draw").map(String::as_str), Some("watercolour"));
    // the preset dropdown opens on the presets and Revert
    h.get(By::new().role(egui::Role::ComboBox).label("Preset")).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    h.get(By::new().role(egui::Role::Button).label("Revert"));
}

#[test]
fn the_bucket_options_have_mode_gaps_tolerance_and_what_to_look_at() {
    let mut h = harness();
    h.key_press(Key::F);
    h.run();
    for name in ["Fill", "Empty Only", "Repaint", "Off", "Small", "Medium", "Large"] {
        h.get(By::new().label(name).role(egui::Role::Button));
    }
    h.get(By::new().role(egui::Role::ComboBox).label("Look at"));
    h.get(By::new().role(egui::Role::SpinButton).label("Tolerance")).focus();
    h.run();
    let before = h.state().fill.tolerance;
    h.key_press(Key::ArrowUp);
    h.run();
    assert!(h.state().fill.tolerance > before);
}

#[test]
fn workspace_changes_made_before_quitting_are_kept() {
    let dir = std::env::temp_dir().join(format!("tooncraft-workspace-exit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = dir.join("workspace-edits.json");
    let mut h = harness();
    h.state_mut().workspaces = crate::workspaces::Workspaces::with_store(Some(store.clone()));
    // Draw is changed and never switched away from
    assert!(h.state_mut().workspace.close(Pane::Onion));
    h.run();
    let layout = h.state().workspace.layout.clone();
    crate::workspaces::keep_current(h.state_mut()).expect("kept");
    let next = crate::workspaces::Workspaces::with_store(Some(store));
    assert_eq!(next.edit(Kind::Draw).map(|e| &e.layout), Some(&layout), "the next launch has the change");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_shown_tab_has_room_for_its_glyph() {
    let h = harness();
    // the glyph is painted over the tab craft-ui laid out; its id has to match
    // (the canvas stands alone, with no tab strip)
    for pane in [Pane::Timeline, Pane::Colour, Pane::Layers] {
        let r = crate::panel_chrome::tab_rect(&h.ctx, pane).unwrap_or_else(|| panic!("{pane:?} has a tab"));
        assert!(r.width() > 20.0 + crate::theme::metrics::ICON, "{pane:?}: {r:?}");
    }
    // the tab's accessible name is still the panel's name
    h.get(By::new().role(egui::Role::Tab).label("Layers"));
}

#[test]
fn the_stack_menu_has_the_panels_own_commands_then_float_move_and_close() {
    let mut h = harness();
    h.get(By::new().role(egui::Role::Button).label("Timeline panel menu")).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    for name in ["Float", "Move To", "Close"] {
        h.get_by_label(name);
    }
    h.get(By::new().role(egui::Role::CheckBox).label("Vertical Exposure Sheet")).focus();
    h.run();
    h.key_press(Key::Space);
    h.run();
    assert!(h.state().tl.vertical, "the timeline's own command ran");
    // close the layers stack's panel from its menu, by keyboard
    h.key_press(Key::Escape);
    h.run();
    h.get(By::new().role(egui::Role::Button).label("Layers panel menu")).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    h.get(By::new().role(egui::Role::Button).label("Close")).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert!(!h.state().workspace.is_open(Pane::Layers), "closed from the menu");
    // the Window menu puts it back
    h.state_mut().workspace.open(Pane::Layers);
    h.run();
    assert!(h.state().workspace.is_open(Pane::Layers));
}
