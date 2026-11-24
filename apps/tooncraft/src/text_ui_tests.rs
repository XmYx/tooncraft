//! UI tests for the text tool: placing text from the keyboard (T, Enter at the keyboard pen,
//! typing, Ctrl+Enter), clicking to place and Escape to drop, the tool's options, and typed
//! letters that are tool shortcuts elsewhere staying in the text. The accessibility check is on.

use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};

use crate::app::{Tool, ToonApp};
use crate::args::Cli;

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(vec2(1600.0, 940.0)).with_accessibility_check(true).build_ui_state(
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

fn painted(h: &Harness<'static, ToonApp>) -> usize {
    let app = h.state();
    let Some(s) = app.engine.project.drawing_at(0, 0).and_then(|d| app.engine.project.drawing(d)) else { return 0 };
    s.tiles().map(|(_, t)| t.bytes().as_chunks::<4>().0.iter().filter(|p| p[3] > 0).count()).sum()
}

fn type_text(h: &mut Harness<'static, ToonApp>, text: &str) {
    h.event(egui::Event::Text(text.into()));
    h.run();
}

#[test]
fn text_is_placed_from_the_keyboard() {
    let mut h = harness();
    assert!(!h.state().engine.fonts.is_empty(), "the app registers its fonts");
    h.key_press(Key::T);
    h.run();
    assert_eq!(h.state().tool, Tool::Text);
    // focus the canvas, then Enter places a text box at the keyboard pen
    h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    h.run();
    assert!(h.state().aids.text.editing.is_some(), "Enter placed a text box");
    let field = h.get(By::new().role(egui::Role::MultilineTextInput).label("Text"));
    assert!(field.is_focused(), "the text field has the keyboard");
    // letters that are tool shortcuts (P, E, N) go into the text, not to the tool bar
    type_text(&mut h, "Pen 1");
    assert_eq!(h.state().tool, Tool::Text, "typing does not switch tools");
    assert_eq!(h.state().aids.text.editing.as_ref().map(|e| e.text.as_str()), Some("Pen 1"));
    h.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    h.run();
    assert!(h.state().aids.text.editing.is_none(), "Ctrl+Enter placed it");
    assert!(painted(&h) > 50, "the text is painted into the drawing");
    assert_eq!(h.state().engine.undo_label(), Some("Text"));
    assert_eq!(h.state().engine.undo_len(), 1, "one undo step");
}

#[test]
fn a_click_places_a_box_and_escape_drops_it() {
    let mut h = harness();
    h.key_press(Key::T);
    h.run();
    let c = h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).rect().center();
    h.hover_at(c);
    h.run();
    h.event(egui::Event::PointerButton { pos: c, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.event(egui::Event::PointerButton { pos: c, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run();
    h.run();
    assert!(h.state().aids.text.editing.is_some(), "a click placed a text box");
    type_text(&mut h, "dropped");
    h.key_press(Key::Escape);
    h.run();
    assert!(h.state().aids.text.editing.is_none());
    assert_eq!(painted(&h), 0, "nothing painted");
    assert_eq!(h.state().engine.undo_len(), 0);
}

#[test]
fn the_text_options_set_font_size_and_alignment() {
    let mut h = harness();
    h.key_press(Key::T);
    h.run();
    h.get(By::new().role(egui::Role::ComboBox).label("Font"));
    h.get_by_label("Size").focus();
    h.run();
    let before = h.state().aids.text.style.size;
    for _ in 0..3 {
        h.key_press(Key::ArrowUp);
        h.run();
    }
    assert!(h.state().aids.text.style.size > before, "arrow keys change the size");
    h.get_by_label("Align right").focus();
    h.run();
    h.key_press(Key::Space);
    h.run();
    assert_eq!(h.state().aids.text.style.align, tooncraft_engine::text::TextAlign::Right);
}
