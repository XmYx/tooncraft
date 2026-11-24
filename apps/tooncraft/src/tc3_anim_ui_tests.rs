//! UI tests for pass 3's animation stream: the onion skin's exact frames and the light table's
//! shift and trace. Keyboard tests move the focus and type.

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};

use crate::app::ToonApp;
use crate::args::Cli;
use crate::dock::Pane;

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let mut app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    app.workspace.raise(Pane::Onion);
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1200.0)).with_accessibility_check(true).build_ui_state(
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

/// Empty the focused text field from the keyboard.
fn clear(h: &mut Harness<'static, ToonApp>) {
    h.key_press(Key::End);
    for _ in 0..24 {
        h.key_press(Key::Backspace);
    }
    h.run();
}

#[test]
fn exact_frames_are_typed_in_the_onion_panel() {
    let mut h = harness();
    h.get(By::new().role(egui::Role::CheckBox).label("Exact frames")).click();
    h.run();
    assert!(h.state().engine.project.onion.pick);
    let field = By::new().role(egui::Role::TextInput).label("Offsets");
    h.get(field.clone()).click();
    h.run();
    clear(&mut h);
    h.get(field.clone()).type_text("-4 +3");
    h.run();
    assert_eq!(h.state().engine.project.onion.picked_offsets(), vec![-4, 3]);
    // a list that does not read is refused with the reason, and the last good one stays
    h.key_press(Key::End);
    h.get(field).type_text(" 0");
    h.run();
    assert_eq!(h.state().engine.project.onion.picked_offsets(), vec![-4, 3]);
    assert!(h.state().status.contains("out of range"), "{}", h.state().status);
}

#[test]
fn a_pinned_frame_is_shifted_and_turned_from_the_keyboard_without_changing_it() {
    let mut h = harness();
    h.get(By::new().role(egui::Role::Button).label("Pin This Frame")).click();
    h.run();
    assert_eq!(h.state().engine.project.light_table.pins.len(), 1);
    let undo = h.state().engine.undo_label().map(str::to_owned);
    let across = By::new().role(egui::Role::SpinButton).label("Across");
    h.get(across.clone()).click();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.get(across).type_text("25");
    h.key_press(Key::Enter);
    h.run();
    let turn = By::new().role(egui::Role::SpinButton).label("Turn");
    h.get(turn.clone()).click();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.get(turn).type_text("90");
    h.key_press(Key::Enter);
    h.run();
    h.run();
    let shift = h.state().engine.project.light_table.pins[0].shift;
    assert_eq!((shift.x, shift.y, shift.turn), (25, 0, 900));
    assert_eq!(h.state().engine.undo_label().map(str::to_owned), undo, "shifting a pin changes no drawing: no undo step");
    let reset = By::new().role(egui::Role::Button).label("Reset Shift");
    h.get(reset.clone()).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    h.run();
    assert!(h.state().engine.project.light_table.pins[0].shift.is_none(), "{:?}", h.state().engine.project.light_table.pins[0].shift);
}
