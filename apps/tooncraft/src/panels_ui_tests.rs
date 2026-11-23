//! UI tests for the Window menu: every panel listed with a tick, closing one and putting it back
//! where it was. The accessibility check is on.

use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};

use crate::app::ToonApp;
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
    h.run();
    h
}

fn open_window_menu(h: &mut Harness<'static, ToonApp>) {
    h.get_by_label("Window").click();
    h.run();
}

/// A panel's tick in the Window menu (the tab has the same name).
fn tick<'a>(h: &'a Harness<'static, ToonApp>, name: &'a str) -> egui_kittest::Node<'a> {
    h.get(By::new().role(egui::Role::CheckBox).label(name))
}

/// The panels sharing a tab group with `pane`.
fn group_of(app: &ToonApp, pane: Pane) -> Vec<Pane> {
    fn find(node: &craft_ui::docking::Node<Pane>, pane: Pane) -> Option<Vec<Pane>> {
        match node {
            craft_ui::docking::Node::Tabs { panels, .. } => panels.contains(&pane).then(|| panels.clone()),
            craft_ui::docking::Node::Split { first, second, .. } => find(first, pane).or_else(|| find(second, pane)),
            _ => None,
        }
    }
    app.workspace.layout.root.as_ref().and_then(|r| find(r, pane)).unwrap_or_default()
}

#[test]
fn the_window_menu_closes_a_panel_and_puts_it_back_where_it_was() {
    let mut h = harness();
    open_window_menu(&mut h);
    // every panel but the canvas is listed
    for pane in Pane::ALL.into_iter().filter(|p| *p != Pane::Canvas) {
        tick(&h, &pane.label());
    }
    assert!(h.state().workspace.is_open(Pane::Layers));
    let before = group_of(h.state(), Pane::Layers);
    assert!(before.contains(&Pane::Onion));
    tick(&h, "Layers").click();
    h.run();
    assert!(!h.state().workspace.is_open(Pane::Layers), "closed");
    assert!(group_of(h.state(), Pane::Onion).iter().all(|p| *p != Pane::Layers));
    // ticking it again puts it back in the same group, at the same place
    open_window_menu(&mut h);
    tick(&h, "Layers").click();
    h.run();
    assert!(h.state().workspace.is_open(Pane::Layers), "open again");
    assert_eq!(group_of(h.state(), Pane::Layers), before, "back where it was");
}

#[test]
fn the_canvas_cannot_be_closed_and_raising_a_closed_panel_opens_it() {
    let mut h = harness();
    assert!(!h.state_mut().workspace.close(Pane::Canvas));
    assert!(h.state().workspace.is_open(Pane::Canvas));
    assert!(h.state_mut().workspace.close(Pane::History));
    assert!(!h.state_mut().workspace.close(Pane::History), "already closed");
    h.run();
    // Find Command and --panel raise panels: a closed one comes back
    h.state_mut().workspace.raise(Pane::History);
    h.run();
    assert!(h.state().workspace.is_open(Pane::History));
    h.get_by_label("Undo History");
}

#[test]
fn reset_panels_brings_every_closed_panel_back() {
    let mut h = harness();
    for p in [Pane::Sound, Pane::Pegs, Pane::Timeline] {
        assert!(h.state_mut().workspace.close(p));
    }
    h.run();
    open_window_menu(&mut h);
    h.get_by_label("Reset Panels").click();
    h.run();
    for p in Pane::ALL {
        assert!(h.state().workspace.is_open(p), "{p:?} open after reset");
    }
}
