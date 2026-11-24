//! UI tests for the timeline's layer rows: the lock toggle beside the eye and the peg-key marks.

use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_doc::{Ease, PegId};
use tooncraft_engine::Command;

use crate::app::ToonApp;
use crate::args::Cli;

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1000.0)).with_accessibility_check(true).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    h.run();
    h.run();
    h
}

#[test]
fn the_lock_beside_the_eye_locks_the_layer_with_a_click_or_the_keyboard() {
    let mut h = harness();
    let name = h.state().engine.project.layers[0].name.clone();
    let label = format!("Lock {name}");
    assert!(!h.state().engine.project.layers[0].locked);
    h.get(By::new().role(egui::Role::CheckBox).label(&label)).click();
    h.run();
    assert!(h.state().engine.project.layers[0].locked);
    assert_eq!(h.state().engine.undo_label(), Some("Layer settings"));
    h.get(By::new().role(egui::Role::CheckBox).label(&label));
    // from the keyboard: the toggle takes the focus and Space flips it
    h.get(By::new().role(egui::Role::CheckBox).label(&label)).focus();
    h.run();
    h.key_press(Key::Space);
    h.run();
    assert!(!h.state().engine.project.layers[0].locked);
}

#[test]
fn peg_keys_are_marked_and_named_in_the_layer_row() {
    let mut h = harness();
    let s = h.state_mut();
    s.run(Command::AddPeg { name: "Body".into(), parent: None });
    s.run(Command::AddPeg { name: "Hand".into(), parent: Some(PegId(1)) });
    s.run(Command::SetLayerPeg { layer: 0, peg: Some(PegId(2)) });
    let key = |peg, frame| Command::SetPegKey { peg: PegId(peg), frame, x: 0.0, y: 0.0, angle: 0.0, scale_x: 1.0, scale_y: 1.0, skew: 0.0, ease: Ease::Linear };
    s.run(key(2, 0));
    s.run(key(2, 5));
    // a key on the parent peg moves the layer too
    s.run(key(1, 8));
    h.run();
    h.run();
    let p = &h.state().engine.project;
    let cell =
        |f: usize| format!("{}, frame {}: {}", p.layers[0].name, f + 1, crate::timeline::cell_text(p.layers[0].cells.get(f).copied().unwrap_or_default()));
    let (keyed, plain): (Vec<String>, Vec<String>) = ([0, 5, 8].map(cell).into(), [3].map(cell).into());
    for c in keyed {
        h.get(By::new().role(egui::Role::Cell).label(&format!("{c}, peg key")));
    }
    for c in plain {
        h.get(By::new().role(egui::Role::Cell).label(&c));
    }
}
