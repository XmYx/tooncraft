//! UI tests for the effects added in pass 3: a soft mask drawn as a shape (taken from a
//! selection), a lens flare and displacement by another layer. The accessibility check is on.

use egui::{Role, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, NodeT, Queryable};
use tooncraft_engine::Command;
use tooncraft_fx::{Effect, EffectKind, MaskShape};

use crate::app::ToonApp;
use crate::args::Cli;

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
    h.state_mut().workspace.raise(crate::dock::Pane::Effects);
    h.run();
    h.run();
    h
}

fn click(h: &mut Harness<'static, ToonApp>, label: &str) {
    h.get_by_label(label).scroll_to_me();
    h.run();
    h.get_by_label(label).click();
    h.run();
}

#[test]
fn a_soft_mask_takes_its_drawn_shape_from_the_selection() {
    let mut h = harness();
    let mask = EffectKind::Mask { shape: MaskShape::Drawn, x: 0.0, y: 0.0, width: 0.0, height: 0.0, feather: 8.0, invert: false, points: Vec::new() };
    assert!(h.state_mut().run(Command::AddEffect { layer: 0, effect: Effect::new(mask), index: None }));
    h.run();
    h.get(By::new().role(Role::ComboBox).label("Mask shape"));
    h.get_by_label("No shape yet: nothing is kept");
    // without a selection the button says why it is off
    assert!(h.get_by_label("Use the Selection as the Shape").accesskit_node().is_disabled());
    h.state_mut().selection = Some(vec![(100.0, 100.0), (400.0, 120.0), (250.0, 380.0)]);
    h.run();
    click(&mut h, "Use the Selection as the Shape");
    let EffectKind::Mask { points, .. } = &h.state().engine.project.layers[0].effects[0].kind else { panic!("still a mask") };
    assert_eq!(points.len(), tooncraft_fx::MASK_SHAPE_POINTS, "resampled to the fixed count");
    assert_eq!(h.state().engine.undo_label(), Some("Effect settings"));
    h.get_by_label(&format!("{} points", tooncraft_fx::MASK_SHAPE_POINTS));
}

#[test]
fn flare_and_displacement_settings() {
    let mut h = harness();
    assert!(h.state_mut().run(Command::AddLayer { name: "Map".into(), index: None }));
    h.state_mut().layer = 0;
    h.run();
    // the add menu offers both; a displacement starts on the other layer
    h.get(By::new().role(Role::ComboBox).label("Add effect")).click();
    h.run();
    click(&mut h, "Displace by layer");
    let map_id = h.state().engine.project.layers[1].id.0;
    assert!(matches!(h.state().engine.project.layers[0].effects[0].kind, EffectKind::Displace { layer, .. } if layer == map_id));
    h.get(By::new().role(Role::ComboBox).label("Map layer"));
    h.get(By::new().role(Role::CheckBox).label("By brightness")).click();
    h.run();
    assert!(matches!(h.state().engine.project.layers[0].effects[0].kind, EffectKind::Displace { brightness: true, .. }));
    h.get(By::new().role(Role::ComboBox).label("Add effect")).click();
    h.run();
    click(&mut h, "Lens flare");
    assert_eq!(h.state().engine.project.layers[0].effects[1].kind.id(), "lens_flare");
    h.get_by_label("Rays");
    h.get_by_label("Ghosts towards X");
}
