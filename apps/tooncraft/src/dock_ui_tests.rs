//! UI tests for the docked workspace: panels move, tab together, split, float and come back with
//! real pointer drags and from the keyboard, through the app's own dock (craft-ui docking under
//! the ToonCraft chrome). The accessibility check is on; nothing is written to the user's settings.

use craft_ui::docking::{DockArea, Hit, Node};
use egui::{Key, Pos2, Rect, vec2};
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

/// The docked tab stack holding `pane`, in tab order (empty when it is floating or closed).
fn docked_stack(h: &Harness<'static, ToonApp>, pane: Pane) -> Vec<Pane> {
    fn find(node: &Node<Pane>, pane: Pane) -> Option<Vec<Pane>> {
        match node {
            Node::Tabs { panels, .. } => panels.contains(&pane).then(|| panels.clone()),
            Node::Split { first, second, .. } => find(first, pane).or_else(|| find(second, pane)),
            _ => None,
        }
    }
    h.state().workspace.layout.root.as_ref().and_then(|r| find(r, pane)).unwrap_or_default()
}

fn floating_rect(h: &Harness<'static, ToonApp>, pane: Pane) -> Option<[f32; 4]> {
    h.state().workspace.layout.floating.iter().find(|w| w.contains(&pane)).map(|w| w.rect)
}

/// Where craft-ui drew `hit` in the app's dock last frame.
fn hit(h: &Harness<'static, ToonApp>, target: Hit<Pane>) -> Rect {
    DockArea::<Pane>::hit_rects(&h.ctx, crate::panel_chrome::dock_id())
        .into_iter()
        .find(|(x, _)| *x == target)
        .map(|(_, r)| r)
        .unwrap_or_else(|| panic!("{target:?} is not drawn"))
}

/// Press at `from`, move there in steps and let go at `to`, one frame per step.
fn drag(h: &mut Harness<'static, ToonApp>, from: Pos2, to: Pos2) {
    h.drag_at(from);
    h.run();
    for i in 1..=6 {
        h.hover_at(from + (to - from) * (i as f32 / 6.0));
        h.run();
    }
    h.drop_at(to);
    h.run();
    h.run();
}

#[test]
fn dragging_a_tab_onto_another_stack_tabs_the_panel_there() {
    let mut h = harness();
    assert!(!docked_stack(&h, Pane::Colour).contains(&Pane::Onion), "onion starts with the layers");
    let from = hit(&h, Hit::Tab(Pane::Onion)).center();
    let to = hit(&h, Hit::Tab(Pane::Colour)).center();
    drag(&mut h, from, to);
    let stack = docked_stack(&h, Pane::Colour);
    assert!(stack.contains(&Pane::Onion), "onion joined the colour tabs: {stack:?}");
    assert!(!docked_stack(&h, Pane::Layers).contains(&Pane::Onion), "and left its old stack");
    assert!(h.state().workspace.layout.validate().is_ok());
}

#[test]
fn dragging_a_tab_to_the_side_of_a_panel_splits_a_new_stack_there() {
    let mut h = harness();
    // the timeline's body, in its outer left third: the left drop zone
    let body = hit(&h, Hit::Tab(Pane::Timeline));
    let canvas = h.state().workspace.canvas_rect;
    let to = egui::pos2(canvas.left() + canvas.width() * 0.1, body.bottom() + 90.0);
    let from = hit(&h, Hit::Tab(Pane::Onion)).center();
    drag(&mut h, from, to);
    assert_eq!(docked_stack(&h, Pane::Onion), vec![Pane::Onion], "onion has a stack of its own");
    assert!(floating_rect(&h, Pane::Onion).is_none(), "docked, not floating");
    assert!(!docked_stack(&h, Pane::Timeline).contains(&Pane::Onion), "beside the timeline, not in its tabs");
    assert!(h.state().workspace.layout.validate().is_ok());
}

#[test]
fn a_panel_floats_from_its_stack_menu_and_its_window_moves_by_the_title_bar() {
    let mut h = harness();
    h.get(By::new().role(egui::Role::Button).label("Layers panel menu")).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    h.get(By::new().role(egui::Role::Button).label("Float")).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    h.run();
    let before = floating_rect(&h, Pane::Layers).expect("layers floats in a window of its own");
    assert!(!docked_stack(&h, Pane::Onion).contains(&Pane::Layers), "it left its stack");
    let title = hit(&h, Hit::Title(Pane::Layers)).center();
    drag(&mut h, title, title + vec2(180.0, 90.0));
    let after = floating_rect(&h, Pane::Layers).expect("still floating");
    assert!((after[0] - before[0] - 180.0).abs() < 4.0 && (after[1] - before[1] - 90.0).abs() < 4.0, "{before:?} -> {after:?}");
    // Reset Panel docks it back in its first place
    assert!(h.state_mut().workspace.reset_panel(Pane::Layers));
    h.run();
    assert!(floating_rect(&h, Pane::Layers).is_none());
    assert!(docked_stack(&h, Pane::Onion).contains(&Pane::Layers));
}

#[test]
fn move_to_puts_a_panel_beside_another_from_the_keyboard() {
    let mut h = harness();
    h.get(By::new().role(egui::Role::Button).label("Timeline panel menu")).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    h.get(By::new().role(egui::Role::Button).label("Move To")).focus();
    h.run();
    h.key_press(Key::ArrowRight);
    h.run();
    h.get(By::new().role(egui::Role::Button).label("Beside Brush")).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    h.run();
    assert!(docked_stack(&h, Pane::Brush).contains(&Pane::Timeline), "{:?}", docked_stack(&h, Pane::Brush));
    assert!(h.state().workspace.layout.validate().is_ok());
}

#[test]
fn the_canvas_cannot_be_dragged_away_or_floated() {
    let mut h = harness();
    let canvas = h.state().workspace.canvas_rect;
    // the canvas has no tab to drag (it stands alone)
    assert!(DockArea::<Pane>::hit_rects(&h.ctx, crate::panel_chrome::dock_id()).iter().all(|(x, _)| *x != Hit::Tab(Pane::Canvas)));
    let from = hit(&h, Hit::Tab(Pane::Colour)).center();
    drag(&mut h, from, canvas.center());
    // a panel dropped on the canvas floats there; the canvas takes no tabs
    assert!(floating_rect(&h, Pane::Colour).is_some(), "the colour panel floats over the canvas");
    assert!(h.state().workspace.is_open(Pane::Canvas));
    assert!(floating_rect(&h, Pane::Canvas).is_none());
    assert!(h.state().workspace.layout.validate().is_ok(), "any drop leaves a valid layout");
}
