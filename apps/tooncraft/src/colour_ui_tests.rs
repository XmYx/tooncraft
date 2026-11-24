//! UI tests for colour: the hue/saturation/brightness fields typed from the keyboard, the hue
//! wheel, harmonies, shot palettes (new, add, open and save .gpl) and recolouring a layer. The
//! accessibility check is on.

use egui::{Key, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_engine::Command;

use crate::app::ToonApp;
use crate::args::Cli;

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

fn type_into(h: &mut Harness<'static, ToonApp>, label: &str, text: &str) {
    h.get(By::new().label(label).role(egui::Role::SpinButton)).click();
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
    h.run();
    h.get(By::new().label(label).role(egui::Role::SpinButton)).type_text(text);
    h.run();
    h.key_press(Key::Enter);
    h.run();
}

#[test]
fn hsv_fields_take_typed_numbers() {
    let mut h = harness();
    h.state_mut().color = [200, 40, 40, 255];
    h.run();
    type_into(&mut h, "Hue", "120");
    let c = h.state().color;
    assert!(c[1] > c[0] && c[1] > c[2], "green after hue 120: {c:?}");
    type_into(&mut h, "Saturation", "0");
    let c = h.state().color;
    assert!(c[0] == c[1] && c[1] == c[2], "grey at saturation 0: {c:?}");
    type_into(&mut h, "Brightness", "100");
    assert_eq!(h.state().color[..3], [255, 255, 255]);
}

#[test]
fn harmonies_and_the_wheel() {
    let mut h = harness();
    h.state_mut().color = [255, 0, 0, 255];
    h.run();
    // the opposite of red is cyan
    h.get(By::new().role(egui::Role::Button).label("Harmony colour #00FFFF")).click();
    h.run();
    assert_eq!(h.state().color[..3], [0, 255, 255]);
    let t = crate::colour_tools::harmony([255, 0, 0], crate::colour_tools::Harmony::Triadic);
    assert_eq!(t, vec![[255, 0, 0], [0, 255, 0], [0, 0, 255]]);
    h.get(By::new().role(egui::Role::CheckBox).label("Hue wheel")).click();
    h.run();
    h.get(By::new().role(egui::Role::Slider).label("Hue wheel"));
}

#[test]
fn shot_palettes_files_and_recolour() {
    let mut h = harness();
    h.state_mut().color = [12, 34, 56, 255];
    h.run();
    h.get_by_label("Add Current Colour").click();
    h.run();
    let p = &h.state().engine.project.palettes;
    assert_eq!(p.len(), 1, "adding on Basic makes a shot palette");
    assert_eq!(p[0].swatches[0].rgb, [12, 34, 56]);
    h.get(By::new().role(egui::Role::Button).label("Swatch #0C2238"));
    // save it and open it again: a second palette with the same colours
    let dir = std::env::temp_dir().join(format!("tooncraft-pal-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("mine.gpl");
    let shown = crate::colour_tools::shown(h.state());
    crate::colour_tools::save_file(h.state_mut(), &shown, &f);
    crate::colour_tools::open_file(h.state_mut(), &f);
    assert_eq!(h.state().engine.project.palettes.len(), 2);
    assert_eq!(h.state().engine.project.palettes[1].swatches, h.state().engine.project.palettes[0].swatches);
    // a bad file says so and changes nothing
    std::fs::write(dir.join("bad.gpl"), b"nope").unwrap();
    crate::colour_tools::open_file(h.state_mut(), &dir.join("bad.gpl"));
    assert_eq!(h.state().engine.project.palettes.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
    // recolour the layer
    h.state_mut().run(Command::Fill { layer: 0, frame: 0, x: 5, y: 5, color: [200, 20, 20, 255], options: Default::default(), reference: Default::default() });
    h.state_mut().colour.recolour_from = [200, 20, 20];
    h.state_mut().color = [20, 20, 200, 255];
    h.run();
    for label in ["Recolour Layer", "Recolour Every Drawing of the Layer"] {
        h.get_by_label(label).scroll_to_me();
        h.run();
        h.get_by_label(label).click();
        h.run();
    }
    assert_eq!(h.state().engine.undo_label(), Some("Recolour"));
    assert_eq!(h.state().engine.pick_color(0, 5, 5)[..3], [20, 20, 200]);
}

#[test]
fn paper_texture_is_chosen_from_the_keyboard_and_shows_on_the_canvas() {
    let mut h = harness();
    // open the section and the texture list with the keyboard only
    let header = || By::new().label("Paper texture");
    let list = || By::new().label("Texture").role(egui::Role::ComboBox);
    for (by, key) in [(header(), Key::Enter), (list(), Key::Space)] {
        h.get(by.clone()).scroll_to_me();
        h.run();
        h.get(by).focus();
        h.run();
        h.key_press(key);
        h.run();
    }
    h.get_by_label("Canvas weave").click();
    h.run();
    h.run();
    let t = h.state().engine.project.paper.texture;
    assert_eq!(t.kind, tooncraft_brush::GrainKind::Canvas);
    assert!(!t.is_off(), "picking a texture turns it on");
    h.get(By::new().role(egui::Role::Slider).label("Texture strength"));
    // the canvas has the repeating texture, made once, not per frame
    let uploads_before = h.state().textures.uploads;
    h.run();
    assert!(h.state().textures.uploads <= uploads_before, "the paper texture is cached");
    let (ctx, project) = (h.ctx.clone(), h.state().engine.project.clone());
    assert!(h.state_mut().textures.paper(&ctx, &project).is_some());
    // the picture with the paper (exports) shows it as well
    let img = tooncraft_doc::render_frame(&h.state().engine.project, 0, tooncraft_doc::RenderOptions::default());
    let px: std::collections::BTreeSet<[u8; 4]> = (0..60).map(|i| img.pixel(i * 7, i * 5)).collect();
    assert!(px.len() > 3, "plain paper: {px:?}");
    // and Off switches it back off
    h.get(list()).click();
    h.run();
    h.get_by_label("Off").click();
    h.run();
    assert!(h.state().engine.project.paper.texture.is_off());
}
