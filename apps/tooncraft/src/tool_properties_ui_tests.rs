//! The Tool Properties panel: the tool options bar's settings in a side panel, for a layout with
//! the properties at the side. It follows the current tool, edits the same values as the bar, and
//! both can show at once. The accessibility check is on.

use egui::{Pos2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};

use crate::app::{Tool, ToonApp};
use crate::args::Cli;
use crate::dock::Pane;

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(vec2(1600.0, 1000.0)).with_accessibility_check(true).with_max_steps(8).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    h.state_mut().workspace.raise(Pane::ToolProperties);
    h.run();
    h.run();
    h
}

/// Centres of the nodes labelled `label`, left to right.
fn centres(h: &Harness<'static, ToonApp>, label: &str) -> Vec<Pos2> {
    let mut v: Vec<Pos2> = h.query_all(By::new().label(label)).map(|n| n.rect().center()).collect();
    v.sort_by(|a, b| a.x.total_cmp(&b.x));
    v
}

#[test]
fn the_panel_shows_the_bar_settings_and_edits_the_same_brush() {
    let mut h = harness();
    h.state_mut().tool = Tool::Pencil;
    h.run();
    let bars = centres(&h, "Size bar");
    assert_eq!(bars.len(), 2, "the bar above the canvas and the panel both show the size: {bars:?}");
    let panel_bar = bars[1];
    assert!(panel_bar.x > 1250.0, "the second one is in the side panel: {panel_bar:?}");
    let kind = Tool::Pencil.brush().map(|k| h.state_mut().brush_mut(k).size);
    // a click near the right end of the panel's bar makes the pencil much bigger
    let at = pos2(panel_bar.x + 33.0, panel_bar.y);
    h.drag_at(at);
    h.drop_at(at);
    h.run();
    h.run();
    let after = Tool::Pencil.brush().map(|k| h.state_mut().brush_mut(k).size);
    assert!(after > kind, "{kind:?} -> {after:?}");
}

#[test]
fn the_panel_follows_the_current_tool() {
    let mut h = harness();
    h.state_mut().tool = Tool::Fill;
    h.run();
    h.run();
    assert_eq!(centres(&h, "Empty Only").len(), 2, "the fill options in the bar and in the panel");
    h.state_mut().tool = Tool::Select;
    h.run();
    h.run();
    assert!(h.query_all(By::new().label("Empty Only")).next().is_none(), "the fill options are gone with the fill tool");
}

#[test]
fn the_panel_opens_by_its_launch_name() {
    assert_eq!(crate::panel_views::by_name("tool-properties"), Some(Pane::ToolProperties));
    assert_eq!(crate::panel_views::by_name("properties"), Some(Pane::ToolProperties));
}
