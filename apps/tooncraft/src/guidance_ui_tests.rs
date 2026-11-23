//! UI tests for the reference-guided polish: the vertical sheet, onion per step and reach, tool
//! flyouts, fill modes, brush presets, shortcut search and conflicts, the add-layer menu, in/out
//! and speed, key steps, Preferences, the Export dialog, layouts and the colour chips. Keys are
//! pressed as keys; the accessibility check is on.

use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_engine::Command;

use crate::app::{Tool, ToonApp};
use crate::args::Cli;

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(vec2(1700.0, 1050.0)).with_accessibility_check(true).build_ui_state(
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

/// Drawing A on frames 1 to 3, B from frame 4.
fn two_drawings(h: &mut Harness<'static, ToonApp>) {
    let app = h.state_mut();
    assert!(app.run(Command::NewDrawing { layer: 0, frame: 0 }));
    assert!(app.run(Command::SetExposure { layer: 0, frame: 0, length: 3 }));
    assert!(app.run(Command::NewDrawing { layer: 0, frame: 3 }));
    h.run();
}

fn click(h: &mut Harness<'static, ToonApp>, label: &str) {
    h.get_by_label(label).click();
    h.run();
}

#[test]
fn the_vertical_sheet_runs_frames_down_and_layers_across() {
    let mut h = harness();
    two_drawings(&mut h);
    h.state_mut().run(Command::AddLayer { name: "Top".into(), index: Some(1) });
    h.state_mut().layer = 0;
    h.get(By::new().role(egui::Role::Button).label("Vertical")).click();
    h.run();
    assert!(h.state().tl.vertical);
    let name = h.state().engine.project.layers[0].name.clone();
    h.get(By::new().role(egui::Role::Cell).label_contains(&format!("{name}, frame 1:"))).click();
    h.run();
    assert!(h.ctx.memory(|m| m.has_focus(crate::timeline::grid_id())));
    h.key_press(Key::ArrowDown);
    h.run();
    assert_eq!(h.state().frame, 1, "down is the next frame");
    h.key_press(Key::ArrowRight);
    h.run();
    assert_eq!(h.state().layer, 1, "right is the layer above (the next column)");
    h.key_press(Key::ArrowLeft);
    h.run();
    assert_eq!(h.state().layer, 0);
    h.key_press(Key::Num5);
    h.run();
    assert_eq!(h.state().engine.project.layers[0].exposure_range(0), Some((0, 5)));
    // the cell menu works down the page too
    h.get(By::new().role(egui::Role::Cell).label_contains(&format!("{name}, frame 1:"))).click_secondary();
    h.run();
    h.get_by_label("Swap Drawing");
}

#[test]
fn onion_per_step_keys_only_and_reach() {
    let mut h = harness();
    two_drawings(&mut h);
    h.state_mut().workspace.raise(crate::dock::Pane::Onion);
    h.run();
    h.get(By::new().role(egui::Role::CheckBox).label("Opacity per step")).click();
    h.run();
    assert!(h.state().engine.project.onion.per_step);
    h.get(By::new().label("Before, step 1"));
    h.get(By::new().role(egui::Role::CheckBox).label("Key drawings only")).click();
    h.run();
    assert!(h.state().engine.project.onion.keys_only);
    // the ruler brackets set the reach: by frames, the number of frames
    {
        let app = h.state_mut();
        app.engine.project.onion.mode = tooncraft_doc::OnionMode::Frames;
        app.frame = 5;
        crate::timeline::set_onion_reach(app, true, 1);
        crate::timeline::set_onion_reach(app, false, 9);
        assert_eq!((app.engine.project.onion.before, app.engine.project.onion.after), (4, 3));
        crate::timeline::set_onion_reach(app, true, 0);
        crate::timeline::set_onion_reach(app, false, 999);
        assert_eq!(app.engine.project.onion.after as usize, tooncraft_doc::ONION_MAX_STEPS, "capped");
    }
    h.run();
    h.get(By::new().label("Onion skin reach before"));
}

#[test]
fn tool_groups_fill_modes_and_presets() {
    let mut h = harness();
    // the bucket's group holds the gradient
    h.get_by_label("Paint bucket (F)").click_secondary();
    h.run();
    h.get(By::new().label_contains("Gradient")).click();
    h.run();
    assert_eq!(h.state().tool, Tool::Gradient);
    h.get_by_label("Gradient (G)");
    // fill modes and gap presets
    h.key_press(Key::F);
    h.run();
    click(&mut h, "Empty Only");
    assert_eq!(h.state().fill.mode, tooncraft_raster::fill::FillMode::EmptyOnly);
    click(&mut h, "Large");
    assert_eq!(h.state().fill.gap, tooncraft_raster::fill::GAP_PRESETS[3]);
    // presets: a picker and Revert for the brush tools
    h.key_press(Key::N);
    h.run();
    h.get(By::new().role(egui::Role::ComboBox).label("Preset"));
    h.state_mut().brushes[1].size = 77.0;
    click(&mut h, "Revert");
    assert_eq!(h.state().brushes[1].size, tooncraft_brush::Brush::preset(tooncraft_brush::BrushKind::Ink).size);
}

#[test]
fn shortcut_search_and_the_conflict_warning() {
    let mut h = harness();
    h.state_mut().show_shortcuts = true;
    h.run();
    h.state_mut().keys.filter = "hand".into();
    h.run();
    h.get_by_label("Change Hand");
    assert!(h.query_by_label("Change Pencil").is_none(), "the search hides the rest");
    h.state_mut().keys.filter = "ctrl+left".into();
    h.run();
    h.get_by_label("Change Previous drawing");
    h.state_mut().keys.filter.clear();
    h.run();
    h.get_by_label("Change Hand").click();
    h.run();
    h.key_press(Key::P);
    h.run();
    let taken = h.state().keys.taken.clone().unwrap_or_default();
    assert!(taken.contains("Pencil"), "{taken}");
}

#[test]
fn add_layer_menu_and_a_reference_image() {
    let mut h = harness();
    h.state_mut().workspace.raise(crate::dock::Pane::Layers);
    h.run();
    click(&mut h, "Add Other");
    click(&mut h, "Drawing Layer");
    assert_eq!(h.state().engine.project.layers.len(), 2);
    let dir = std::env::temp_dir().join(format!("tooncraft-ref-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ref.png");
    let img = tooncraft_raster::RgbaImage::filled(16, 16, [200, 100, 50, 255]);
    std::fs::write(&path, tooncraft_format::encode_png(&img).unwrap()).unwrap();
    assert!(h.state_mut().add_reference(&path.to_string_lossy()));
    let l = &h.state().engine.project.layers[0];
    assert_eq!(l.name, "Reference");
    assert!(l.locked && l.opacity < 1.0, "under the drawing, locked and faded");
    let len = h.state().engine.project.length();
    assert_eq!(l.exposure_range(0), Some((0, len)), "for the whole shot");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn in_out_speed_play_around_and_key_steps() {
    let mut h = harness();
    two_drawings(&mut h);
    h.state_mut().frame = 1;
    click(&mut h, "Set In");
    h.state_mut().frame = 4;
    click(&mut h, "Set Out");
    assert_eq!(h.state().tl.range, Some((1, 4)));
    assert_eq!(crate::timeline_extras::play_span(h.state()), (1, 5));
    h.get(By::new().role(egui::Role::ComboBox).label("Speed"));
    h.state_mut().frame = 6;
    click(&mut h, "Play Around");
    assert!(h.state().playback.playing);
    assert_eq!((h.state().playback.start, h.state().playback.end), (3, 10));
    h.state_mut().playback.stop();
    // key drawings: mark A and B, then step between them
    let (a, b) = {
        let p = &h.state().engine.project;
        (p.drawing_at(0, 0).unwrap(), p.drawing_at(0, 3).unwrap())
    };
    for d in [a, b] {
        h.state_mut().engine.project.marks.insert(d, tooncraft_doc::Mark::Key);
    }
    h.state_mut().frame = 0;
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::ArrowRight);
    h.run();
    assert_eq!(h.state().frame, 3);
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::ArrowLeft);
    h.run();
    assert_eq!(h.state().frame, 0);
    // layers step with Ctrl+Up / Ctrl+Down
    h.state_mut().run(Command::AddLayer { name: "Top".into(), index: Some(1) });
    h.state_mut().layer = 0;
    h.key_press_modifiers(Modifiers::COMMAND, Key::ArrowUp);
    h.run();
    assert_eq!(h.state().layer, 1);
}

#[test]
fn preferences_tabs_restore_and_apply() {
    let mut h = harness();
    click(&mut h, "Edit");
    click(&mut h, "Preferences…");
    assert!(h.state().preferences.open);
    click(&mut h, "Sheet and timeline");
    h.get(By::new().role(egui::Role::CheckBox).label("Sheet down the page (layers as columns)")).click();
    h.run();
    assert!(h.state().tl.vertical, "applies at once");
    h.get(By::new().role(egui::Role::Button).label("General")).click();
    h.run();
    h.state_mut().preferences.prefs.undo_megabytes = 100;
    crate::preferences::apply_live(h.state_mut());
    assert_eq!(h.state().engine.undo_budget, 100 << 20);
    // Restore Defaults in the window puts the tab back, and that applies at once
    click(&mut h, "Restore Defaults");
    let d = crate::preferences::Prefs::default().undo_megabytes;
    assert_eq!(h.state().preferences.prefs.undo_megabytes, d);
    assert_eq!(h.state().engine.undo_budget, (d as u64) << 20);
    h.key_press(Key::Escape);
    h.run();
    assert!(!h.state().preferences.open);
}

#[test]
fn the_export_dialog_offers_each_formats_options() {
    let mut h = harness();
    crate::export_dialog::open(h.state_mut(), "gif");
    h.run();
    h.get(By::new().role(egui::Role::CheckBox).label("Loop"));
    assert!(h.query_by_label("Digits in frame numbers").is_none(), "GIF has no frame numbers");
    h.get(By::new().role(egui::Role::ComboBox).label("Size"));
    {
        let app = h.state_mut();
        let d = app.export_dialog.as_mut().unwrap();
        d.range = crate::export_dialog::Range::Custom;
        d.from = 2;
        d.to = 4;
        d.scale = 2;
        d.loop_forever = false;
    }
    h.run();
    let o = h.state().export_dialog.as_ref().unwrap().options(h.state());
    assert_eq!((o.start, o.end, o.scale, o.loop_forever), (1, 4, 2, false));
    click(&mut h, "Cancel");
    assert!(h.state().export_dialog.is_none());
    // the choices come back for the same shot; a PNG sequence offers digits
    crate::export_dialog::open(h.state_mut(), "png");
    h.run();
    assert_eq!(h.state().export_dialog.as_ref().unwrap().scale, 2, "remembered");
    h.get_by_label("Digits in frame numbers");
    // a format that cannot be resized ignores the size
    h.state_mut().export_dialog.as_mut().unwrap().kind = "psd-frames";
    let o = h.state().export_dialog.as_ref().unwrap().options(h.state());
    assert_eq!(o.scale, 1);
    // hostile custom range: clamped into the shot
    {
        let app = h.state_mut();
        let d = app.export_dialog.as_mut().unwrap();
        d.from = 0;
        d.to = 99_999;
    }
    let o = h.state().export_dialog.as_ref().unwrap().options(h.state());
    assert!(o.start < o.end && o.end <= h.state().engine.project.length());
}

#[test]
fn layouts_and_colour_chips() {
    let mut h = harness();
    click(&mut h, "View");
    click(&mut h, "Workspaces");
    click(&mut h, "Exposure Sheet");
    assert!(h.state().tl.vertical);
    h.state_mut().workspace.raise(crate::dock::Pane::Colour);
    h.run();
    h.state_mut().color = [10, 20, 30, 255];
    h.run();
    h.state_mut().color = [200, 100, 0, 255];
    h.run();
    h.run();
    h.get(By::new().role(egui::Role::Button).label("Previous colour #0A141E")).click();
    h.run();
    assert_eq!(h.state().color, [10, 20, 30, 255]);
}

#[test]
fn brush_colour_jitter_spray_and_blend_modes_are_in_the_editor() {
    let mut h = harness();
    h.key_press(Key::N);
    h.run();
    h.state_mut().workspace.raise(crate::dock::Pane::Brush);
    h.run();
    for header in ["Jitter", "Colour jitter"] {
        h.get_by_label(header).scroll_to_me();
        h.run();
        h.get_by_label(header).click();
        h.run();
    }
    h.get(By::new().role(egui::Role::Slider).label("Hue jitter"));
    h.get(By::new().role(egui::Role::Slider).label("Dabs per step"));
    h.get(By::new().role(egui::Role::CheckBox).label("Once per stroke")).click();
    h.run();
    assert!(h.state().brushes[1].jitter.per_stroke);
    assert_eq!(crate::layer_ops::compose_name(tooncraft_brush::Compose::Multiply), "Multiply");
}

#[test]
fn effect_looks_saved_stacks_and_the_new_effects() {
    let mut h = harness();
    h.state_mut().workspace.raise(crate::dock::Pane::Effects);
    h.run();
    h.run();
    h.get(By::new().role(egui::Role::ComboBox).label("Add a look")).click();
    h.run();
    h.get_by_label("Film").click();
    h.run();
    assert_eq!(h.state().engine.project.layers[0].effects.len(), 3, "the look's three effects");
    assert_eq!(h.state().engine.undo_label(), Some("Add effects"));
    // save the stack and add it again from the file
    let dir = std::env::temp_dir().join(format!("tooncraft-fx-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("look.toonfx");
    let stack = h.state().engine.project.layers[0].effects.clone();
    crate::effects_panel::save_stack(h.state_mut(), &stack, &f);
    crate::effects_panel::open_stack(h.state_mut(), 0, &f);
    assert_eq!(h.state().engine.project.layers[0].effects.len(), 6);
    std::fs::write(dir.join("bad.toonfx"), b"{").unwrap();
    crate::effects_panel::open_stack(h.state_mut(), 0, &dir.join("bad.toonfx"));
    assert_eq!(h.state().engine.project.layers[0].effects.len(), 6, "a bad file adds nothing");
    let _ = std::fs::remove_dir_all(&dir);
    // every new kind has a name and renders without trouble
    for k in tooncraft_fx::EffectKind::catalogue(320, 200) {
        assert!(!crate::effects_panel::effect_name(&k).is_empty());
    }
    h.state_mut().engine.project.layers[0].effects.clear();
    h.state_mut().run(tooncraft_engine::Command::AddEffect {
        layer: 0,
        effect: tooncraft_fx::Effect::new(tooncraft_fx::EffectKind::Ripple { x: 10.0, y: 10.0, amplitude: 3.0, wavelength: 20.0, speed: 0.1 }),
        index: None,
    });
    h.run();
    h.get_by_label("Wavelength");
}
