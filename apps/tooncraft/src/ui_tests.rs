//! UI tests with egui_kittest: the main flows driven by the keyboard, with the accessibility
//! check on (every `run()` fails on an input widget without an accessible name).

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_engine::sheet;

use crate::app::{Tool, ToonApp};
use crate::args::Cli;

fn harness(demo: bool) -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli { demo, ..Default::default() }).without_autosave();
    Harness::builder().with_size(egui::vec2(1500.0, 940.0)).with_accessibility_check(true).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    )
}

fn focus(h: &mut Harness<'static, ToonApp>, id: egui::Id) {
    h.ctx.memory_mut(|m| m.request_focus(id));
    h.run();
}

#[test]
fn whole_window_is_accessible_and_names_the_canvas_and_sheet() {
    let mut h = harness(true);
    h.run();
    h.get(By::new().role(egui::Role::Canvas).label("Canvas: Ink, frame 1"));
    h.get(By::new().role(egui::Role::Grid).label("Exposure sheet"));
    h.get(By::new().role(egui::Role::Cell).label("Ink, frame 2: hold"));
    h.get(By::new().role(egui::Role::Cell).label_contains("Ink, frame 1: drawing"));
}

#[test]
fn tool_shortcuts() {
    let mut h = harness(false);
    h.run();
    for (key, tool) in [(Key::N, Tool::Ink), (Key::F, Tool::Fill), (Key::E, Tool::Eraser), (Key::A, Tool::Airbrush), (Key::P, Tool::Pencil)] {
        h.key_press(key);
        h.run();
        assert_eq!(h.state().tool, tool);
    }
    h.get_by_label("Ink pen (N)").click();
    h.run();
    assert_eq!(h.state().tool, Tool::Ink);
}

#[test]
fn exposure_sheet_by_keyboard() {
    let mut h = harness(false);
    h.run();
    focus(&mut h, crate::timeline::grid_id());
    h.key_press_modifiers(Modifiers::SHIFT, Key::N);
    h.run();
    h.key_press(Key::Plus);
    h.run();
    h.key_press(Key::Plus);
    h.run();
    assert_eq!(&sheet(&h.state().engine.project.layers[0].cells)[..5], "#--..");
    for _ in 0..3 {
        h.key_press(Key::ArrowRight);
        h.run();
    }
    assert_eq!(h.state().frame, 3);
    h.key_press_modifiers(Modifiers::SHIFT, Key::D);
    h.run();
    h.key_press(Key::Minus);
    h.run();
    h.key_press(Key::Delete);
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run();
    assert_eq!(h.state().engine.project.drawing_count(), 1, "undoing the duplicate removes its drawing");
    assert!(h.state().engine.can_redo());
    assert!(h.ctx.memory(|m| m.has_focus(crate::timeline::grid_id())), "focus stays on the sheet");
}

#[test]
fn play_and_onion_from_the_keyboard() {
    let mut h = harness(true);
    h.run();
    assert!(h.state().engine.project.onion.enabled);
    h.key_press(Key::O);
    h.run();
    assert!(!h.state().engine.project.onion.enabled);
    h.key_press(Key::Enter);
    h.run();
    assert!(h.state().playback.playing);
    std::thread::sleep(std::time::Duration::from_millis(250));
    h.run();
    assert!(h.state().frame > 0);
    h.key_press(Key::Enter);
    h.run();
    assert!(!h.state().playback.playing);
}

#[test]
fn keyboard_pen_draws_and_f6_moves_focus() {
    let mut h = harness(false);
    h.run();
    h.key_press(Key::F6);
    h.run();
    assert!(h.ctx.memory(|m| m.has_focus(crate::canvas::canvas_id())));
    h.key_press(Key::N);
    h.run();
    h.key_press(Key::Enter);
    h.run();
    for _ in 0..10 {
        h.key_press_modifiers(Modifiers::SHIFT, Key::ArrowRight);
        h.run();
    }
    h.key_press(Key::Enter);
    h.run();
    let app = h.state();
    let d = app.engine.project.drawing_at(0, 0).expect("a drawing was made");
    assert!(!app.engine.project.drawing(d).unwrap().is_empty());
    assert_eq!(app.engine.undo_len(), 1);
    h.key_press(Key::F6);
    h.run();
    assert!(h.ctx.memory(|m| m.has_focus(crate::timeline::grid_id())));
}

#[test]
fn drawing_with_the_pointer_and_fill() {
    let mut h = harness(false);
    h.run();
    let rect = h.get_by_label("Canvas: Layer 1, frame 1").rect();
    let c = rect.center();
    h.drag_at(c + egui::vec2(-120.0, 0.0));
    h.run();
    for i in 0..=24 {
        let a = i as f32 / 24.0 * std::f32::consts::TAU;
        h.hover_at(c + egui::vec2(a.cos() * -120.0, a.sin() * 90.0));
        h.run();
    }
    h.drop_at(c + egui::vec2(-120.0, 0.0));
    h.run();
    assert_eq!(h.state().engine.undo_len(), 1, "one stroke");
    h.key_press(Key::F);
    h.run();
    h.state_mut().color = [10, 200, 30, 255];
    h.get_by_label("Canvas: Layer 1, frame 1").click();
    h.run();
    assert_eq!(h.state().engine.undo_label(), Some("Fill"));
    let (w, hh) = (h.state().engine.project.width() / 2, h.state().engine.project.height() / 2);
    assert_eq!(h.state().engine.pick_color(0, w, hh), [10, 200, 30, 255]);
}

#[test]
fn select_copy_paste_frames_by_keyboard() {
    let mut h = harness(false);
    h.run();
    focus(&mut h, crate::timeline::grid_id());
    h.key_press_modifiers(Modifiers::SHIFT, Key::N);
    h.run();
    h.key_press(Key::Plus);
    h.run();
    // select frames 1–2 with Shift+→, copy, move to frame 5, paste
    h.key_press_modifiers(Modifiers::SHIFT, Key::ArrowRight);
    h.run();
    assert!(h.state().has_range());
    h.event(egui::Event::Copy);
    h.run();
    assert!(h.state().cell_clipboard.is_some());
    for _ in 0..3 {
        h.key_press(Key::ArrowRight);
        h.run();
    }
    assert!(!h.state().has_range(), "a plain arrow drops the selection");
    assert_eq!(h.state().frame, 4);
    h.event(egui::Event::Paste(String::new()));
    h.run();
    assert_eq!(&sheet(&h.state().engine.project.layers[0].cells)[..7], "#-..#-.");
    assert_eq!(h.state().engine.project.drawing_count(), 1, "pasted frames show the same drawing");
    // Delete clears the pasted (still selected) range
    h.key_press(Key::Delete);
    h.run();
    assert_eq!(&sheet(&h.state().engine.project.layers[0].cells)[..7], "#-.....");
    h.get(By::new().role(egui::Role::Grid).label("Exposure sheet"));
}

#[test]
fn alt_arrows_pan_and_shift_click_draws_a_line() {
    let mut h = harness(false);
    h.run();
    let before = h.state().view.pan;
    h.key_press_modifiers(Modifiers::ALT, Key::ArrowLeft);
    h.run();
    assert!(h.state().view.pan.x > before.x);
    h.state_mut().last_stroke_end = Some((200.0, 200.0));
    let rect = h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).rect();
    let at = rect.center();
    h.event(egui::Event::PointerMoved(at));
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::SHIFT });
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::SHIFT });
    h.run();
    let app = h.state();
    assert_eq!(app.engine.undo_len(), 1, "one straight-line stroke");
    let d = app.engine.project.drawing_at(0, 0).unwrap();
    let s = app.engine.project.drawing(d).unwrap();
    let end = app.last_stroke_end.unwrap();
    let mid = ((200.0 + end.0) / 2.0, (200.0 + end.1) / 2.0);
    assert!(s.pixel(mid.0 as i64, mid.1 as i64)[3] > 0, "the line passes through the midpoint");
}

#[test]
fn dragging_a_cell_changes_its_hold() {
    let mut h = harness(false);
    h.run();
    focus(&mut h, crate::timeline::grid_id());
    h.key_press_modifiers(Modifiers::SHIFT, Key::N);
    h.run();
    let first = h.get(By::new().role(egui::Role::Cell).label_contains("Layer 1, frame 1:")).rect();
    let fourth = h.get(By::new().role(egui::Role::Cell).label("Layer 1, frame 4: blank")).rect();
    h.drag_at(first.center());
    h.run();
    h.hover_at(fourth.center());
    h.run();
    h.drop_at(fourth.center());
    h.run();
    h.run();
    assert_eq!(&sheet(&h.state().engine.project.layers[0].cells)[..5], "#---.");
}
