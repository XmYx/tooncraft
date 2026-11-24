//! UI tests for the Curves panel: dragging keys, the keyboard, eases on many keys.

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_doc::{Channel, Ease, PegId};
use tooncraft_engine::Command;

use crate::app::ToonApp;
use crate::args::Cli;
use crate::dock::Pane;

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(egui::vec2(1500.0, 1100.0)).with_accessibility_check(true).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    let s = h.state_mut();
    s.run(Command::AddPeg { name: "Arm".into(), parent: None });
    for (frame, x) in [(0, 0.0), (10, 50.0), (20, 20.0)] {
        s.run(Command::SetPegKey { peg: PegId(1), frame, x, y: 0.0, angle: 0.0, scale_x: 1.0, scale_y: 1.0, skew: 0.0, ease: Ease::Linear });
    }
    s.workspace.raise(Pane::Curves);
    h.run();
    h.run();
    h
}

fn keys(h: &Harness<'static, ToonApp>) -> Vec<(u32, f32)> {
    h.state().engine.project.peg(PegId(1)).map(|p| p.keys.iter().map(|k| (k.frame, k.x)).collect()).unwrap_or_default()
}

fn press(h: &mut Harness<'static, ToonApp>, m: Modifiers, k: Key) {
    h.key_press_modifiers(m, k);
    h.run();
}

#[test]
fn dragging_a_key_is_one_undo_step_and_escape_cancels_it() {
    let mut h = harness();
    h.get(By::new().role(egui::Role::Slider).label_contains("Move X curve"));
    let (ppf, ppv) = h.state().curve_ui.scale().unwrap();
    let at = h.state().curve_ui.screen_pos(10, 50.0).unwrap();
    let to = at + egui::vec2(3.0 * ppf, -5.0 * ppv);
    let depth = h.state().engine.undo_len();
    h.drag_at(at);
    h.run();
    h.hover_at(at + egui::vec2(ppf, 0.0));
    h.run();
    h.hover_at(to);
    h.run();
    assert_eq!(keys(&h)[1], (10, 50.0), "nothing changes while dragging");
    h.drop_at(to);
    h.run();
    h.run();
    let k = keys(&h);
    assert_eq!(k[1].0, 13, "{k:?}");
    assert!((k[1].1 - 55.0).abs() < 0.2, "{k:?}");
    assert_eq!(h.state().engine.undo_len(), depth + 1, "one undo step for the drag");
    assert_eq!(h.state().engine.undo_label(), Some("Move keys"));
    // a drag cancelled with Escape changes nothing and leaves no drag behind
    let at = h.state().curve_ui.screen_pos(k[1].0, k[1].1).unwrap();
    h.drag_at(at);
    h.run();
    h.hover_at(at + egui::vec2(2.0 * ppf, 30.0));
    h.run();
    h.hover_at(at + egui::vec2(4.0 * ppf, 40.0));
    h.run();
    press(&mut h, Modifiers::NONE, Key::Escape);
    h.drop_at(at + egui::vec2(4.0 * ppf, 40.0));
    h.run();
    h.run();
    assert_eq!(keys(&h), k);
    assert_eq!(h.state().engine.undo_len(), depth + 1);
    assert!(!h.state().curve_ui.dragging());
}

#[test]
fn keys_are_chosen_moved_and_removed_from_the_keyboard() {
    let mut h = harness();
    h.ctx.memory_mut(|m| m.request_focus(crate::curves::graph_id()));
    h.run();
    press(&mut h, Modifiers::NONE, Key::ArrowRight);
    h.get(By::new().role(egui::Role::Slider).label("Move X curve: key 1 of 3 at frame 1, value 0.00; 1 chosen"));
    press(&mut h, Modifiers::NONE, Key::ArrowRight);
    press(&mut h, Modifiers::SHIFT, Key::ArrowRight);
    h.get(By::new().role(egui::Role::Slider).label("Move X curve: key 3 of 3 at frame 21, value 20.00; 2 chosen"));
    let depth = h.state().engine.undo_len();
    press(&mut h, Modifiers::ALT, Key::ArrowRight);
    assert_eq!(keys(&h), [(0, 0.0), (11, 50.0), (21, 20.0)]);
    press(&mut h, Modifiers::SHIFT, Key::ArrowUp);
    assert_eq!(keys(&h), [(0, 0.0), (11, 60.0), (21, 30.0)]);
    assert_eq!(h.state().engine.undo_len(), depth + 2, "one step per key press");
    // another value
    h.state_mut().curve_ui.channel = Channel::ScaleX;
    h.run();
    press(&mut h, Modifiers::NONE, Key::ArrowUp);
    let sx: Vec<f32> = h.state().engine.project.peg(PegId(1)).map(|p| p.keys.iter().map(|k| k.scale_x).collect()).unwrap_or_default();
    assert!((sx[1] - 1.01).abs() < 1e-4 && (sx[2] - 1.01).abs() < 1e-4 && sx[0] == 1.0, "{sx:?}");
    // a refused move (onto frame 0's key) changes nothing
    press(&mut h, Modifiers::NONE, Key::ArrowLeft);
    press(&mut h, Modifiers::NONE, Key::ArrowLeft);
    let before = keys(&h);
    press(&mut h, Modifiers::ALT | Modifiers::SHIFT, Key::ArrowRight);
    assert_eq!(keys(&h)[0].0, 10, "frame 0's key moves ten frames");
    press(&mut h, Modifiers::ALT, Key::ArrowRight);
    assert_eq!(keys(&h)[0].0, 10, "frame 11 is taken, so the move is refused");
    assert_ne!(keys(&h), before);
    press(&mut h, Modifiers::COMMAND, Key::A);
    press(&mut h, Modifiers::NONE, Key::Delete);
    assert!(keys(&h).is_empty());
    h.state_mut().engine.undo().unwrap();
    h.run();
    assert_eq!(keys(&h).len(), 3);
    press(&mut h, Modifiers::NONE, Key::Enter);
}

#[test]
fn one_ease_goes_on_every_chosen_key_and_typed_handles_are_clamped() {
    let mut h = harness();
    h.ctx.memory_mut(|m| m.request_focus(crate::curves::graph_id()));
    h.run();
    press(&mut h, Modifiers::COMMAND, Key::A);
    h.get_by_label("Ease for 3 chosen keys");
    let depth = h.state().engine.undo_len();
    h.get(By::new().role(egui::Role::Button).label("Hold")).click();
    h.run();
    h.run();
    let eases = |h: &Harness<'static, ToonApp>| -> Vec<Ease> {
        h.state().engine.project.peg(PegId(1)).map(|p| p.keys.iter().map(|k| k.ease).collect()).unwrap_or_default()
    };
    assert_eq!(eases(&h), [Ease::Hold; 3]);
    assert_eq!(h.state().engine.undo_len(), depth + 1, "one undo step for all of them");
    h.get(By::new().role(egui::Role::Button).label("Ease in")).click();
    h.run();
    h.run();
    h.get(By::new().role(egui::Role::SpinButton).label("Handle 1 time")).scroll_to_me();
    h.run();
    h.get(By::new().role(egui::Role::SpinButton).label("Handle 1 time")).click();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.get(By::new().role(egui::Role::SpinButton).label("Handle 1 time")).type_text("9");
    h.key_press(Key::Enter);
    h.run();
    h.run();
    let e = eases(&h);
    assert!(e.iter().all(|e| matches!(e, Ease::Curve { x1, .. } if *x1 == 1.0)), "{e:?}");
    h.get(By::new().role(egui::Role::Image).label_contains("Ease handles: 1.00, 0.00"));
}

#[test]
fn the_panel_without_pegs_says_what_to_do() {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(egui::vec2(1500.0, 1100.0)).with_accessibility_check(true).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    h.state_mut().workspace.raise(Pane::Curves);
    h.run();
    h.run();
    h.get_by_label("No pegs yet. Add one in the Pegs panel to see its keys here.");
}
