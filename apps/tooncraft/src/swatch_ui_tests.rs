//! UI tests for the colour's own opacity, editing a swatch (its paint follows) and the floating
//! picker window. The accessibility check is on.

use egui::{Key, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_doc::palette::Palette;
use tooncraft_engine::Command;

use crate::app::ToonApp;
use crate::args::Cli;

const SKIN: [u8; 3] = [250, 210, 190];

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(vec2(1700.0, 1300.0)).with_accessibility_check(true).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    h.state_mut().workspace.raise(crate::dock::Pane::Colour);
    h.run();
    h.run();
    h
}

#[test]
fn the_colour_has_its_own_opacity_set_from_the_keyboard_and_paint_uses_it() {
    let mut h = harness();
    h.state_mut().color = [200, 40, 40, 255];
    h.run();
    h.get(By::new().role(egui::Role::Slider).label("Colour opacity")).focus();
    h.run();
    for _ in 0..50 {
        h.key_press(Key::ArrowLeft);
    }
    h.run();
    let a = h.state().color[3];
    assert!((120..=135).contains(&a), "about half after 50 steps down: {a}");
    // choosing another hue keeps the opacity
    h.get(By::new().role(egui::Role::SpinButton).label("Hue")).click();
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
    h.get(By::new().role(egui::Role::SpinButton).label("Hue")).type_text("120");
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(h.state().color[3], a, "a new hue keeps the opacity");
    // a fill with the current colour is that transparent
    let c = h.state().color;
    assert!(h.state_mut().run(Command::Fill { layer: 0, frame: 0, x: 5, y: 5, color: c, options: Default::default(), reference: Default::default() }));
    let p = &h.state().engine.project;
    let px = p.drawing(p.drawing_at(0, 0).unwrap()).unwrap().pixel(5, 5);
    assert!(px[3].abs_diff(a) <= 1, "painted with the colour's opacity: {px:?}");
}

#[test]
fn editing_a_swatch_from_its_menu_recolours_its_paint() {
    let mut h = harness();
    let mut p = Palette::new("Characters");
    p.push("skin", SKIN);
    let app = h.state_mut();
    assert!(app.run(Command::SetPalettes { palettes: vec![p] }));
    assert!(app.run(Command::Fill {
        layer: 0,
        frame: 0,
        x: 5,
        y: 5,
        color: [SKIN[0], SKIN[1], SKIN[2], 255],
        options: Default::default(),
        reference: Default::default()
    }));
    app.colour.current = 1;
    h.run();
    let swatch = By::new().role(egui::Role::Button).label("Swatch skin, #FAD2BE");
    h.get(swatch.clone()).scroll_to_me();
    h.run();
    h.get(swatch).click_secondary();
    h.run();
    h.get_by_label("Edit Swatch…").click();
    h.run();
    // the editor: rename from the keyboard, pick a new colour, apply from the keyboard
    h.get_by_label("Swatch name").focus();
    h.run();
    h.key_press(Key::End);
    h.get_by_label("Swatch name").type_text(" (warm)");
    h.run();
    h.state_mut().colour.edit.as_mut().unwrap().rgb = [240, 170, 140];
    h.run();
    let n = h.state().engine.undo_len();
    h.get_by_label("Apply").focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    let app = h.state();
    assert_eq!(app.engine.undo_len(), n + 1, "one undo step");
    assert_eq!(app.engine.undo_label(), Some("Change swatch"));
    assert_eq!(app.engine.project.palettes[0].swatches[0].name, "skin (warm)");
    assert_eq!(app.engine.project.palettes[0].swatches[0].rgb, [240, 170, 140]);
    assert_eq!(app.engine.pick_color(0, 5, 5)[..3], [240, 170, 140], "the fill in the old colour follows the swatch");
    assert!(app.colour.edit.is_none(), "the editor closes");
}

#[test]
fn the_picker_window_opens_chooses_a_swatch_and_closes() {
    let mut h = harness();
    h.get(By::new().label("Picker Window")).click();
    h.run();
    assert!(h.state().colour.window);
    h.get_by_label("Colour Picker");
    h.state_mut().color = [1, 2, 3, 77];
    h.run();
    // Basic swatches show twice (panel and window): the window's is the last one
    let first = h.state().swatches[0];
    let label = format!("Swatch #{:02X}{:02X}{:02X}", first[0], first[1], first[2]);
    let n = h.query_all(By::new().role(egui::Role::Button).label(&label)).count();
    assert!(n >= 2, "the window lists the palette shown too");
    h.query_all(By::new().role(egui::Role::Button).label(&label)).last().unwrap().click();
    h.run();
    assert_eq!(h.state().color, [first[0], first[1], first[2], 77], "the swatch's colour, the opacity kept");
    h.get(By::new().label("Picker Window")).click();
    h.run();
    assert!(!h.state().colour.window);
}
