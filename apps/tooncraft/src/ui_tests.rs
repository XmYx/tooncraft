//! UI tests with egui_kittest: the main flows driven by the keyboard, with the accessibility
//! check on (every `run()` fails on an input widget without an accessible name).

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_engine::sheet;

use crate::app::{Tool, ToonApp};
use crate::args::Cli;

fn harness(demo: bool) -> Harness<'static, ToonApp> {
    harness_sized(demo, egui::vec2(1500.0, 940.0))
}

fn harness_sized(demo: bool, size: egui::Vec2) -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli { demo, ..Default::default() }).without_autosave();
    Harness::builder().with_size(size).with_accessibility_check(true).build_ui_state(
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

#[test]
fn soundtrack_shows_a_waveform_and_scrubs() {
    let dir = std::env::temp_dir().join(format!("tooncraft-ui-sound-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let rate = 22_050u32;
    let tone: Vec<f32> = (0..rate * 2).map(|i| (i as f32 * 0.05).sin() * 0.6).collect();
    let wav = craft_audio::encode(
        &craft_audio::AudioBuffer { sample_rate: rate, channels: vec![tone] },
        &craft_audio::EncodeOptions::new(craft_audio::EncodeFormat::Wav),
    )
    .unwrap();
    let path = dir.join("voice.wav");
    std::fs::write(&path, wav).unwrap();
    let mut h = harness(false);
    h.state_mut().run(tooncraft_engine::Command::SetSound { path: Some(path.to_string_lossy().into_owned()), offset: 2 });
    h.run();
    h.get(By::new().role(egui::Role::Slider).label("Sound voice.wav, starts at frame 3"));
    assert!(h.state().sound.peaks.is_some());
    assert!(!h.state().sound.is_playing());
    h.key_press(Key::Period);
    h.run();
    assert!(h.state().sound.is_playing(), "stepping a frame plays that frame's sound");
}

fn drag_line(h: &mut Harness<'static, ToonApp>, from: egui::Pos2, to: egui::Pos2) {
    h.drag_at(from);
    h.run();
    for i in 1..=12 {
        h.hover_at(from + (to - from) * (i as f32 / 12.0));
        h.run();
    }
    h.drop_at(to);
    h.run();
}

#[test]
fn pen_eraser_end_erases() {
    let mut h = harness(false);
    h.run();
    let rect = h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).rect();
    let (a, b) = (rect.center() - egui::vec2(150.0, 0.0), rect.center() + egui::vec2(150.0, 0.0));
    h.state_mut().brushes[0].size = 30.0;
    drag_line(&mut h, a, b);
    let count = |h: &Harness<'static, ToonApp>| {
        let app = h.state();
        let d = app.engine.project.drawing_at(0, 0).unwrap();
        app.engine.project.drawing(d).unwrap().tiles().map(|(_, t)| t.bytes().as_chunks::<4>().0.iter().filter(|p| p[3] > 0).count()).sum::<usize>()
    };
    let inked = count(&h);
    assert!(inked > 100);
    // flip the pen: the eraser end is reported by the tablet reader
    h.state().tablet.set(Some(crate::tablet::Pen { pressure: 1.0, tilt_x: 0.0, tilt_y: 0.0, eraser: true }));
    h.state_mut().brushes[4].size = 120.0;
    drag_line(&mut h, a, b);
    assert!(count(&h) < inked / 4, "the eraser end removed the line ({} → {})", inked, count(&h));
}

#[test]
fn select_move_nudge_and_erase() {
    let mut h = harness(false);
    h.run();
    let rect = h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).rect();
    let c = rect.center();
    h.state_mut().brushes[1].size = 12.0;
    h.key_press(Key::N);
    h.run();
    drag_line(&mut h, c - egui::vec2(40.0, 0.0), c + egui::vec2(40.0, 0.0));
    let ink_at = |h: &Harness<'static, ToonApp>, p: egui::Pos2| {
        let app = h.state();
        let size = egui::vec2(app.engine.project.width() as f32, app.engine.project.height() as f32);
        let q = app.view.to_canvas(app.workspace.canvas_rect, size, p);
        app.engine.pick_color(0, q.x as u32, q.y as u32) != [246, 243, 236, 255]
    };
    assert!(ink_at(&h, c));
    h.key_press(Key::S);
    h.run();
    h.state_mut().select_mode = crate::app::SelectMode::Rectangle;
    drag_line(&mut h, c - egui::vec2(80.0, 40.0), c + egui::vec2(80.0, 40.0));
    assert!(h.state().selection.is_some());
    let undo_before = h.state().engine.undo_len();
    // drag inside: move 100 px down on screen
    drag_line(&mut h, c, c + egui::vec2(0.0, 100.0));
    assert_eq!(h.state().engine.undo_len(), undo_before + 1, "a live move is one undo step");
    let ink_box = |h: &Harness<'static, ToonApp>| {
        let app = h.state();
        let s = app.engine.project.drawing(app.engine.project.drawing_at(0, 0).unwrap()).unwrap();
        let mut bb = (i64::MAX, i64::MAX, i64::MIN, i64::MIN);
        for (i, t) in s.tiles() {
            let (tx, ty) = s.tile_coords(i);
            for (k, p) in t.bytes().as_chunks::<4>().0.iter().enumerate() {
                if p[3] > 0 {
                    let (x, y) = ((tx * 64) as i64 + k as i64 % 64, (ty * 64) as i64 + k as i64 / 64);
                    bb = (bb.0.min(x), bb.1.min(y), bb.2.max(x), bb.3.max(y));
                }
            }
        }
        bb
    };
    assert!(!ink_at(&h, c), "the line left its old place");
    let sel = h.state().selection.clone().unwrap();
    let (x0, y0) = (sel.iter().map(|p| p.0).fold(f32::MAX, f32::min), sel.iter().map(|p| p.1).fold(f32::MAX, f32::min));
    let (x1, y1) = (sel.iter().map(|p| p.0).fold(f32::MIN, f32::max), sel.iter().map(|p| p.1).fold(f32::MIN, f32::max));
    let bb = ink_box(&h);
    assert!(
        bb.0 as f32 >= x0 - 1.0 && bb.2 as f32 <= x1 + 1.0 && bb.1 as f32 >= y0 - 1.0 && bb.3 as f32 <= y1 + 1.0,
        "the line moved with the selection: {bb:?} in {sel:?}"
    );
    // keyboard: nudge, then erase
    focus(&mut h, crate::canvas::canvas_id());
    h.key_press(Key::ArrowRight);
    h.run();
    assert_eq!(h.state().engine.undo_len(), undo_before + 2);
    h.key_press(Key::Delete);
    h.run();
    assert_eq!(ink_box(&h).0, i64::MAX, "Delete erased the selected pixels");
    h.key_press(Key::Escape);
    h.run();
    assert!(h.state().selection.is_none());
}

#[test]
fn camera_panel_sets_keys_from_the_keyboard() {
    let mut h = harness(false);
    h.state_mut().workspace.raise(crate::dock::Pane::Camera);
    h.run();
    h.get_by_label("Set Key").click();
    h.run();
    assert_eq!(h.state().engine.project.camera.keys.len(), 1);
    assert!(h.state().engine.project.camera.enabled);
    // type a zoom
    h.get_by_label("Zoom").focus();
    h.run();
    h.key_press(Key::ArrowUp);
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert!(h.state().engine.project.camera.keys[0].zoom > 1.0);
    h.get(By::new().role(egui::Role::CheckBox).label("Export through the camera"));
    h.get_by_label("Remove Key").click();
    h.run();
    assert!(h.state().engine.project.camera.keys.is_empty());
}

#[test]
fn shapes_layer_properties_and_thumbnails() {
    let mut h = harness(false);
    h.run();
    h.key_press(Key::U);
    h.run();
    assert_eq!(h.state().tool, Tool::Shape);
    h.state_mut().shape = tooncraft_engine::shapes::ShapeKind::Rectangle;
    let rect = h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).rect();
    drag_line(&mut h, rect.center() - egui::vec2(60.0, 40.0), rect.center() + egui::vec2(60.0, 40.0));
    assert_eq!(h.state().engine.undo_label(), Some("Shape"));
    // layer properties: blend mode by keyboard-reachable combo, then thumbnails
    h.state_mut().workspace.raise(crate::dock::Pane::Layers);
    h.run();
    h.get(By::new().role(egui::Role::ComboBox).label("Blend"));
    h.state_mut().run(tooncraft_engine::Command::SetLayerBlend { layer: 0, blend: tooncraft_doc::BlendMode::Multiply });
    h.run();
    assert!(h.state().blend_cache.is_some(), "a blend mode switches the canvas to the CPU composite");
    h.get_by_label("Thumbnails").click();
    h.run();
    h.run();
    assert!(!h.state().thumbs.is_empty());
}

#[test]
fn shortcuts_can_be_remapped() {
    let mut h = harness(false);
    h.state_mut().show_shortcuts = true;
    h.run();
    h.get_by_label("Change Ink pen").click();
    h.run();
    assert_eq!(h.state().keys.recording.as_deref(), Some("tool.ink"));
    h.key_press(Key::K);
    h.run();
    assert_eq!(h.state().keys.text("tool.ink"), "K");
    h.state_mut().show_shortcuts = false;
    h.run();
    h.key_press(Key::K);
    h.run();
    assert_eq!(h.state().tool, Tool::Ink);
    h.key_press(Key::P);
    h.run();
    h.key_press(Key::N);
    h.run();
    assert_eq!(h.state().tool, Tool::Pencil, "N no longer picks the ink pen");
    h.state_mut().keys.reset("tool.ink");
    h.key_press(Key::N);
    h.run();
    assert_eq!(h.state().tool, Tool::Ink);
}

#[test]
fn pegs_panel_keys_a_move_and_draws_on_the_moved_layer() {
    let mut h = harness(false);
    h.state_mut().workspace.raise(crate::dock::Pane::Pegs);
    h.run();
    h.get_by_label("Add Peg").click();
    h.run();
    assert_eq!(h.state().engine.project.pegs.len(), 1);
    let peg = h.state().engine.project.pegs[0].id;
    h.get(By::new().role(egui::Role::ComboBox).label("“Layer 1” hangs from"));
    h.state_mut().run(tooncraft_engine::Command::SetLayerPeg { layer: 0, peg: Some(peg) });
    h.run();
    // key a move by keyboard on the drag value
    h.get_by_label("Move X").focus();
    h.run();
    for _ in 0..5 {
        h.key_press(Key::ArrowUp);
        h.run();
    }
    h.key_press(Key::Enter);
    h.run();
    let key = h.state().engine.project.peg(peg).unwrap().at(0);
    assert!(key.x > 0.0, "moved by the arrow keys: {}", key.x);
    assert!(h.state().engine.project.peg(peg).unwrap().has_key(0));
    h.get(By::new().role(egui::Role::ComboBox).label("Ease to next key"));
    h.get(By::new().role(egui::Role::Image).label("Ease curve"));
    // a stroke drawn under the pointer lands in the drawing's own space
    let moved = key.x;
    let rect = h.get_by_label("Canvas: Layer 1, frame 1").rect();
    h.key_press(Key::N);
    h.run();
    drag_line(&mut h, rect.center(), rect.center() + egui::vec2(40.0, 0.0));
    let p = &h.state().engine.project;
    let d = p.drawing_at(0, 0).expect("drawn");
    let s = p.drawing(d).unwrap();
    let screen_x = rect.center().x;
    let view = h.state().view;
    let size = egui::vec2(p.width() as f32, p.height() as f32);
    let canvas_x = view.to_canvas(rect, size, egui::pos2(screen_x, rect.center().y)).x;
    let local_x = (canvas_x - moved).round() as i64;
    let y = view.to_canvas(rect, size, rect.center()).y.round() as i64;
    assert!(s.pixel(local_x + 2, y)[3] > 0, "ink at the un-moved position in the drawing");
    // the camera panel's shake list
    h.state_mut().workspace.raise(crate::dock::Pane::Camera);
    h.run();
    h.get_by_label("Add Shake Here").click();
    h.run();
    assert_eq!(h.state().engine.project.camera.shakes.len(), 1);
    h.get_by_label("Remove").click();
    h.run();
    assert!(h.state().engine.project.camera.shakes.is_empty());
}

#[test]
fn effects_panel_builds_a_keyed_stack() {
    let mut h = harness_sized(true, egui::vec2(1500.0, 2600.0));
    h.state_mut().workspace.raise(crate::dock::Pane::Effects);
    h.run();
    let li = h.state().layer;
    h.get(By::new().role(egui::Role::ComboBox).label("Add effect")).click();
    h.run();
    h.get_by_label("Soft blur").click();
    h.run();
    h.get(By::new().role(egui::Role::ComboBox).label("Add effect")).click();
    h.run();
    h.get_by_label("Drop shadow").click();
    h.run();
    let ids: Vec<&str> = h.state().engine.project.layers[li].effects.iter().map(|e| e.kind.id()).collect();
    assert_eq!(ids, ["blur", "drop_shadow"]);
    assert!(h.state().blend_cache.is_some(), "effects switch the canvas to the CPU composite");
    // reorder, switch off, key
    h.get_by_label("Move Drop shadow up").click();
    h.run();
    assert_eq!(h.state().engine.project.layers[li].effects[0].kind.id(), "drop_shadow");
    h.get(By::new().role(egui::Role::CheckBox).label("Soft blur")).click();
    h.run();
    assert!(!h.state().engine.project.layers[li].effects[1].enabled);
    h.get_by_label("Key Soft blur here").click();
    h.run();
    assert_eq!(h.state().engine.project.layers[li].effects[1].keys.len(), 1);
    // a setting typed from the keyboard
    h.get_by_label("Softness").focus();
    h.run();
    h.key_press(Key::ArrowUp);
    h.run();
    h.key_press(Key::Enter);
    h.run();
    let tooncraft_fx::EffectKind::DropShadow { blur, .. } = h.state().engine.project.layers[li].effects[0].kind else { panic!("not a shadow") };
    assert!(blur > 6.0, "softness went up: {blur}");
    // the stack is taller than the panel: reach the button from the keyboard
    h.get_by_label("Remove Soft blur").focus();
    h.run();
    h.key_press(Key::Space);
    h.run();
    assert_eq!(h.state().engine.project.layers[li].effects.len(), 1, "last step: {:?}", h.state().engine.undo_label());
    h.state_mut().undo();
    h.run();
    assert_eq!(h.state().engine.project.layers[li].effects.len(), 2, "removing an effect undoes");
}

#[test]
fn effects_panel_keys_opacity_and_sets_a_matte() {
    let mut h = harness_sized(true, egui::vec2(1500.0, 2600.0));
    h.state_mut().workspace.raise(crate::dock::Pane::Effects);
    h.run();
    let li = h.state().layer;
    h.get_by_label("Key opacity here").click();
    h.run();
    assert_eq!(h.state().engine.project.layers[li].opacity_keys.len(), 1);
    h.get(By::new().role(egui::Role::ComboBox).label("Matte")).click();
    h.run();
    h.get_by_label("Matte: keep inside").click();
    h.run();
    assert_eq!(h.state().engine.project.layers[li].matte, Some(tooncraft_doc::Matte::Inside));
    assert!(h.state().blend_cache.is_some(), "a matte switches the canvas to the CPU composite");
    h.run();
    h.get(By::new().role(egui::Role::ComboBox).label("Add effect")).click();
    h.run();
    h.get_by_label("Glow").click();
    h.run();
    assert_eq!(h.state().engine.project.layers[li].effects.len(), 1, "glow added; status {:?}, undo {:?}", h.state().status, h.state().engine.undo_label());
    h.get(By::new().role(egui::Role::ComboBox).label("Result blend")).click();
    h.run();
    h.get_by_label("Multiply").click();
    h.run();
    assert_eq!(h.state().engine.project.layers[li].effects[0].blend, tooncraft_doc::BlendMode::Multiply);
}

#[test]
fn sound_panel_mixes_tracks_and_dragging_the_playhead_scrubs() {
    let dir = std::env::temp_dir().join(format!("tooncraft-ui-mix-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let rate = 22_050u32;
    let tone: Vec<f32> = (0..rate * 2).map(|i| (i as f32 * 0.05).sin() * 0.6).collect();
    let wav = craft_audio::encode(
        &craft_audio::AudioBuffer { sample_rate: rate, channels: vec![tone] },
        &craft_audio::EncodeOptions::new(craft_audio::EncodeFormat::Wav),
    )
    .unwrap();
    let path = dir.join("line.wav").to_string_lossy().into_owned();
    std::fs::write(&path, wav).unwrap();
    let mut h = harness_sized(false, egui::vec2(1500.0, 2200.0));
    h.state_mut().run(tooncraft_engine::Command::AddSound { path: path.clone(), offset: 0, track: 0 });
    h.state_mut().run(tooncraft_engine::Command::AddSound { path, offset: 4, track: 1 });
    h.state_mut().workspace.raise(crate::dock::Pane::Sound);
    h.run();
    h.get(By::new().role(egui::Role::Slider).label("Sound line.wav and 1 more, starts at frame 1"));
    h.get(By::new().role(egui::Role::CheckBox).label("Solo Track 2")).click();
    h.run();
    assert!(h.state().engine.project.audio_tracks[1].solo);
    h.get(By::new().role(egui::Role::CheckBox).label("Mute clip 1")).click();
    h.run();
    assert!(h.state().engine.project.sounds[0].muted);
    h.get_by_label("Key clip 2 volume here").click();
    h.run();
    assert_eq!(h.state().engine.project.sounds[1].volume_keys.len(), 1);
    h.get_all(By::new().label("Fade in")).next().unwrap().focus();
    h.run();
    h.key_press(Key::ArrowUp);
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert!(h.state().engine.project.sounds[0].fade_in > 0.0);
    assert!(h.state().sound.peaks.is_some(), "the mix has a waveform");
    // drag along the frame ruler: the playhead moves and each frame plays its sound
    let ruler = h.get(By::new().role(egui::Role::Slider).label("Frame ruler")).rect();
    drag_line(&mut h, ruler.left_center() + egui::vec2(30.0, 0.0), ruler.left_center() + egui::vec2(160.0, 0.0));
    assert!(h.state().frame > 0, "the playhead followed the drag");
    assert!(h.state().sound.is_playing(), "dragging the playhead scrubs the sound");
}

#[test]
fn lip_sync_panel_builds_a_chart_and_keys_mouths_from_sound() {
    let dir = std::env::temp_dir().join(format!("tooncraft-ui-lips-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let rate = 22_050u32;
    let gain = |f: f32| 1.0 / (1.0 + ((f - 800.0) / 90.0).powi(2)) + 0.7 / (1.0 + ((f - 1250.0) / 110.0).powi(2));
    let mut voice = vec![0.0f32; rate as usize / 4];
    voice.extend(
        (0..rate as usize / 2)
            .map(|i| (1..34).map(|k| gain(k as f32 * 120.0) * (std::f32::consts::TAU * k as f32 * 120.0 * i as f32 / rate as f32).sin()).sum::<f32>() * 0.05),
    );
    voice.extend(vec![0.0; rate as usize / 2]);
    let wav = craft_audio::encode(
        &craft_audio::AudioBuffer { sample_rate: rate, channels: vec![voice] },
        &craft_audio::EncodeOptions::new(craft_audio::EncodeFormat::Wav),
    )
    .unwrap();
    let path = dir.join("hello.wav");
    std::fs::write(&path, wav).unwrap();
    let mut h = harness_sized(false, egui::vec2(1500.0, 2200.0));
    h.state_mut().run(tooncraft_engine::Command::AddSound { path: path.to_string_lossy().into_owned(), offset: 0, track: 0 });
    for frame in [0, 1] {
        h.state_mut().run(tooncraft_engine::Command::NewDrawing { layer: 0, frame });
    }
    h.state_mut().workspace.raise(crate::dock::Pane::LipSync);
    h.run();
    h.get_by_label("Use for Rest").click();
    h.run();
    h.state_mut().frame = 1;
    h.run();
    h.get_by_label("Use for Open (A I)").click();
    h.run();
    assert_eq!(h.state().engine.project.layers[0].mouth_chart.len(), 2);
    h.get_by_label("Detect Mouths from Sound").click();
    h.run();
    assert_eq!(h.state().engine.undo_label(), Some("Lip sync"));
    let p = &h.state().engine.project;
    let open = p.layers[0].mouth_chart[&tooncraft_lipsync::Mouth::Open];
    assert_eq!(p.drawing_at(0, 12), Some(open), "the vowel opens the mouth");
}
