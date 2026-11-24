//! Every action is reachable from the keyboard: each registry command is found in Find Command
//! (Ctrl+Shift+P) by its own name, and each panel opens from there by typing and the arrow keys,
//! with no pointer. The accessibility check is on.

use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};

use crate::app::ToonApp;
use crate::args::Cli;
use crate::command_search::{SHOW_PANEL, matches};
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

#[test]
fn every_command_and_every_panel_is_found_by_its_own_name() {
    let h = harness();
    for c in crate::commands::ALL {
        let found = matches(h.state(), &c.label());
        assert!(found.iter().any(|(id, _, _)| id == c.id), "{} is found by \"{}\"", c.id, c.label());
    }
    for pane in Pane::ALL.into_iter().filter(|p| *p != Pane::Canvas) {
        let v = crate::panel_views::view(pane);
        let want = format!("{SHOW_PANEL}{}", v.id);
        let found = matches(h.state(), &v.title());
        assert!(found.iter().any(|(id, _, _)| *id == want), "{} is found by \"{}\"", v.id, v.title());
    }
}

#[test]
fn every_panel_opens_from_find_command_without_the_pointer() {
    let mut h = harness();
    for pane in Pane::ALL.into_iter().filter(|p| *p != Pane::Canvas) {
        h.state_mut().workspace.close(pane);
        h.run();
        assert!(!h.state().workspace.is_open(pane), "{pane:?} closed first");
        let v = crate::panel_views::view(pane);
        let text = v.title();
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::P);
        h.run();
        h.get(By::new().role(egui::Role::TextInput).label("Command")).type_text(&text);
        h.run();
        // walk down the list to the panel's row with the arrow key, then Enter
        let want = format!("{SHOW_PANEL}{}", v.id);
        let at = matches(h.state(), &text).iter().position(|(id, _, _)| *id == want);
        let Some(at) = at else { panic!("{want} is listed for \"{text}\"") };
        for _ in 0..at {
            h.key_press(Key::ArrowDown);
            h.run();
        }
        h.key_press(Key::Enter);
        h.run();
        h.run();
        assert!(!h.state().find.open, "Find Command closes after {text}");
        assert!(h.state().workspace.is_open(pane), "{pane:?} opened from the keyboard");
    }
}

#[test]
fn a_command_picked_by_the_arrow_keys_runs_like_its_shortcut() {
    let mut h = harness();
    let frame = h.state().frame;
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::P);
    h.run();
    let label = crate::commands::get("frame.next").map(|c| c.label()).unwrap_or_default();
    h.get(By::new().role(egui::Role::TextInput).label("Command")).type_text(&label);
    h.run();
    let at = matches(h.state(), &label).iter().position(|(id, _, _)| id == "frame.next").unwrap_or(usize::MAX);
    assert!(at != usize::MAX, "frame.next is listed for \"{label}\"");
    for _ in 0..at {
        h.key_press(Key::ArrowDown);
        h.run();
    }
    h.key_press(Key::Enter);
    h.run();
    h.run();
    assert_eq!(h.state().frame, frame + 1);
}
