use super::*;
use crate::export::{ExportOptions, apng, gif, mp4, ora_frame, png_sequence};
use tooncraft_brush::BrushKind;

fn engine() -> Engine {
    Engine::new(Project::new("Test", 320, 240, 12, 8).unwrap())
}

fn circle(cx: f32, cy: f32, r: f32) -> Vec<Sample> {
    (0..=72)
        .map(|i| {
            let a = i as f32 / 72.0 * std::f32::consts::TAU;
            Sample::at(cx + a.cos() * r, cy + a.sin() * r, 1.0, i as f64 * 0.01)
        })
        .collect()
}

fn ink() -> Brush {
    Brush { size: 4.0, smoothing: tooncraft_brush::Smoothing { average: 0.0, ..Default::default() }, ..Brush::preset(BrushKind::Ink) }
}

fn scratch(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("tooncraft-{name}-{}", std::process::id()))
}

#[test]
fn stroke_on_blank_cell_makes_a_drawing_and_undoes_cleanly() {
    let mut e = engine();
    let before = e.project.clone();
    e.apply(Command::Stroke { layer: 0, frame: 2, brush: ink(), samples: circle(100.0, 100.0, 40.0) }).unwrap();
    assert!(e.project.drawing_at(0, 2).is_some());
    assert_eq!(sheet(&e.project.layers[0].cells), "..#.....");
    assert!(e.is_dirty());
    e.undo().unwrap();
    assert_eq!(e.project, before);
    e.redo().unwrap();
    assert!(e.project.drawing_at(0, 2).is_some_and(|d| !e.project.drawing(d).is_some_and(|s| s.is_empty())));
}

#[test]
fn live_stroke_and_cancel() {
    let mut e = engine();
    e.begin_stroke(0, 0, ink()).unwrap();
    assert_eq!(e.begin_stroke(0, 0, ink()), Err(EngineError::Busy));
    for s in circle(50.0, 50.0, 20.0) {
        e.stroke_to(s).unwrap();
    }
    assert!(e.undo().is_err(), "no undo while drawing");
    e.cancel_stroke();
    assert_eq!(e.project.drawing_at(0, 0), None);
    assert_eq!(e.project.drawing_count(), 0);
}

#[test]
fn fill_with_gap_closing_inside_a_drawn_circle() {
    let mut e = engine();
    let mut pts = circle(160.0, 120.0, 60.0);
    pts.truncate(70);
    e.apply(Command::Stroke { layer: 0, frame: 0, brush: ink(), samples: pts }).unwrap();
    let red = [220, 30, 30, 255];
    e.apply(Command::Fill {
        layer: 0,
        frame: 0,
        x: 160,
        y: 120,
        color: red,
        options: FillOptions { gap: 6, ..Default::default() },
        reference: FillReference::Layer,
    })
    .unwrap();
    assert_eq!(e.pick_color(0, 160, 120), red);
    assert_ne!(e.pick_color(0, 10, 10), red, "the fill did not leak through the gap");
    assert_eq!(e.undo_label(), Some("Fill"));
}

#[test]
fn paint_under_ink_on_its_own_layer() {
    let mut e = engine();
    e.apply(Command::Stroke { layer: 0, frame: 0, brush: ink(), samples: circle(160.0, 120.0, 50.0) }).unwrap();
    e.apply(Command::AddLayer { name: "Paint".into(), index: Some(0) }).unwrap();
    e.apply(Command::Fill {
        layer: 0,
        frame: 0,
        x: 160,
        y: 120,
        color: [40, 90, 200, 255],
        options: FillOptions::default(),
        reference: FillReference::AllLayers,
    })
    .unwrap();
    assert!(e.project.drawing_at(0, 0).is_some());
    assert_eq!(e.pick_color(0, 160, 120), [40, 90, 200, 255]);
    assert_eq!(e.project.layers[1].name, "Layer 1");
}

#[test]
fn timeline_commands() {
    let mut e = engine();
    e.apply(Command::NewDrawing { layer: 0, frame: 0 }).unwrap();
    e.apply(Command::ExtendHold { layer: 0, frame: 0 }).unwrap();
    e.apply(Command::DuplicateDrawing { layer: 0, frame: 1 }).unwrap();
    assert_eq!(sheet(&e.project.layers[0].cells), "#-#.....");
    assert_ne!(e.project.drawing_at(0, 0), e.project.drawing_at(0, 2));
    e.apply(Command::InsertFrame { layer: None, frame: 1 }).unwrap();
    assert_eq!(sheet(&e.project.layers[0].cells), "#--#....");
    e.apply(Command::Retime { layer: 0, from: 0, to: 7, step: 3 }).unwrap();
    assert_eq!(sheet(&e.project.layers[0].cells), "#--#--..");
    e.apply(Command::ShortenHold { layer: 0, frame: 4 }).unwrap();
    e.apply(Command::DeleteFrame { layer: None, frame: 0 }).unwrap();
    assert_eq!(sheet(&e.project.layers[0].cells), "#-#-....");
    e.apply(Command::ClearCell { layer: 0, frame: 2 }).unwrap();
    assert_eq!(e.project.unused_drawings().len(), 1);
    e.apply(Command::PurgeUnused).unwrap();
    assert_eq!(e.project.drawing_count(), 1);
    for _ in 0..9 {
        e.undo().unwrap();
    }
    assert_eq!(sheet(&e.project.layers[0].cells), "........");
    assert!(e.undo().is_err());
}

#[test]
fn locked_layers_refuse_edits() {
    let mut e = engine();
    e.apply(Command::SetLayer { layer: 0, visible: None, locked: Some(true), opacity: None }).unwrap();
    assert!(matches!(e.apply(Command::Stroke { layer: 0, frame: 0, brush: ink(), samples: circle(5.0, 5.0, 3.0) }), Err(EngineError::Locked(_))));
    assert!(e.apply(Command::NewDrawing { layer: 0, frame: 0 }).is_err());
    assert!(e.apply(Command::Stroke { layer: 9, frame: 0, brush: ink(), samples: vec![] }).is_err());
}

#[test]
fn undo_budget_drops_oldest_steps() {
    let mut e = engine();
    e.undo_budget = 3 * tooncraft_raster::TILE_BYTES as u64;
    for i in 0..10 {
        e.apply(Command::Stroke { layer: 0, frame: 0, brush: ink(), samples: circle(20.0 + i as f32 * 25.0, 100.0, 8.0) }).unwrap();
    }
    assert!(e.undo_len() < 10);
    assert!(e.history_bytes() <= e.undo_budget || e.undo_len() == 1);
}

#[test]
fn length_and_fps_undo() {
    let mut e = engine();
    e.apply(Command::NewDrawing { layer: 0, frame: 7 }).unwrap();
    e.apply(Command::SetLength { frames: 4 }).unwrap();
    e.apply(Command::SetFps { fps: 24 }).unwrap();
    e.undo().unwrap();
    e.undo().unwrap();
    assert_eq!((e.project.length(), e.project.fps()), (8, 12));
    assert!(e.project.drawing_at(0, 7).is_some());
}

#[test]
fn exports_write_files() {
    let mut e = engine();
    for f in 0..4u32 {
        e.apply(Command::Stroke { layer: 0, frame: f, brush: ink(), samples: circle(60.0 + f as f32 * 40.0, 120.0, 30.0) }).unwrap();
    }
    let dir = scratch("export");
    let o = ExportOptions { start: 0, end: 4, paper: true };
    let files = png_sequence(&e.project, &dir.join("png"), "shot", o, |_| {}).unwrap();
    assert_eq!(files.len(), 4);
    assert!(files[0].ends_with("shot_0001.png"));
    gif(&e.project, &dir.join("shot.gif"), o, |_| {}).unwrap();
    let bytes = std::fs::read(dir.join("shot.gif")).unwrap();
    assert!(craft_gif::sniff(&bytes));
    ora_frame(&e.project, 1, &dir.join("frame.ora")).unwrap();
    assert!(craft_ora::is_ora(&std::fs::read(dir.join("frame.ora")).unwrap()));
    assert!(png_sequence(&e.project, &dir, "x", ExportOptions { start: 3, end: 2, paper: true }, |_| {}).is_err());
    mp4(&e.project, &dir.join("shot.mp4"), o, |_| {}).unwrap();
    let bytes = std::fs::read(dir.join("shot.mp4")).unwrap();
    let file = craft_media::isobmff::open(&bytes[..]).unwrap();
    assert_eq!(file.tracks.len(), 1);
    assert_eq!(file.tracks[0].samples.len(), 4, "one H.264 sample per frame");
    apng(&e.project, &dir.join("shot.png"), ExportOptions { paper: false, ..o }, |_| {}).unwrap();
    let a = std::fs::read(dir.join("shot.png")).unwrap();
    assert!(a.windows(4).any(|w| w == b"acTL"), "animated PNG");
}

#[test]
fn commands_are_json() {
    let c: Command =
        serde_json::from_str(r#"{"cmd":"stroke","layer":0,"frame":1,"brush":{"kind":"ink","size":6},"samples":[{"x":1,"y":2},{"x":30,"y":40,"pressure":1}]}"#)
            .unwrap();
    let mut e = engine();
    e.apply(c).unwrap();
    assert!(e.project.drawing_at(0, 1).is_some());
    let back = serde_json::to_string(&Command::Retime { layer: 0, from: 0, to: 3, step: 2 }).unwrap();
    assert_eq!(back, r#"{"cmd":"retime","layer":0,"from":0,"to":3,"step":2}"#);
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(40))]
    /// Random command sequences never panic, and undoing everything gives the start state back.
    #[test]
    fn random_commands_undo_to_the_start(ops in proptest::collection::vec((0u8..20, 0usize..3, 0u32..10, 0u32..10), 1..30)) {
        let mut e = engine();
        let start = e.project.clone();
        for (op, layer, a, b) in ops {
            let cmd = match op {
                0 => Command::Stroke { layer, frame: a, brush: ink(), samples: circle(a as f32 * 30.0, b as f32 * 20.0, 12.0) },
                1 => Command::Fill { layer, frame: a, x: a * 31, y: b * 23, color: [1, 2, 3, 255], options: FillOptions::default(), reference: FillReference::AllLayers },
                2 => Command::NewDrawing { layer, frame: a },
                3 => Command::DuplicateDrawing { layer, frame: a },
                4 => Command::InsertFrame { layer: Some(layer), frame: a },
                5 => Command::DeleteFrame { layer: None, frame: a },
                6 => Command::ExtendHold { layer, frame: a },
                7 => Command::ShortenHold { layer, frame: a },
                8 => Command::Retime { layer, from: a.min(b), to: a.max(b), step: b % 4 + 1 },
                9 => Command::AddLayer { name: "x".into(), index: Some(layer) },
                10 => Command::DeleteLayer { layer },
                11 => Command::MoveLayer { from: layer, to: a as usize },
                12 => Command::ClearDrawing { layer, frame: a },
                14 => Command::AddPeg { name: "p".into(), parent: (b > 4).then_some(tooncraft_doc::PegId(a as u64)) },
                15 => Command::SetPegKey { peg: tooncraft_doc::PegId(layer as u64 + 1), frame: a, x: b as f32 * 9.0, y: 0.0, angle: a as f32 * 20.0, scale_x: 1.0, scale_y: 1.0, skew: 0.0, ease: tooncraft_doc::Ease::Linear },
                16 => Command::SetLayerPeg { layer, peg: Some(tooncraft_doc::PegId(b as u64)) },
                17 => Command::SetPegParent { peg: tooncraft_doc::PegId(a as u64), parent: Some(tooncraft_doc::PegId(b as u64)) },
                18 => Command::DeletePeg { peg: tooncraft_doc::PegId(a as u64) },
                19 => Command::AddCameraShake { shake: tooncraft_doc::Shake { from: a, to: b, amplitude: 5.0, rotation: 1.0, frequency: 0.3, fade: 1, seed: a } },
                _ => Command::PurgeUnused,
            };
            let _ = e.apply(cmd);
        }
        while e.undo().is_ok() {}
        proptest::prop_assert_eq!(&e.project.layers, &start.layers);
        proptest::prop_assert_eq!(&e.project.pegs, &start.pegs);
        proptest::prop_assert_eq!(&e.project.camera, &start.camera);
        for f in 0..8 {
            proptest::prop_assert_eq!(tooncraft_doc::render_frame(&e.project, f, RenderOptions::default()), tooncraft_doc::render_frame(&start, f, RenderOptions::default()));
        }
    }
}

#[test]
fn paste_clear_reverse_cycle_commands() {
    use tooncraft_doc::Cell;
    let mut e = engine();
    e.apply(Command::NewDrawing { layer: 0, frame: 0 }).unwrap();
    e.apply(Command::NewDrawing { layer: 0, frame: 1 }).unwrap();
    e.apply(Command::ExtendHold { layer: 0, frame: 1 }).unwrap();
    let block = vec![e.project.layers[0].copy_cells(0, 2)];
    e.apply(Command::PasteCells { layer: 0, frame: 4, block: block.clone(), insert: false, duplicate: false }).unwrap();
    assert_eq!(sheet(&e.project.layers[0].cells), "##-.##-.");
    assert_eq!(e.project.drawing_count(), 2, "linked paste shares drawings");
    e.apply(Command::PasteCells { layer: 0, frame: 4, block, insert: false, duplicate: true }).unwrap();
    assert_eq!(e.project.drawing_count(), 4, "duplicate paste copies drawings");
    e.undo().unwrap();
    assert_eq!(e.project.drawing_count(), 2);
    e.apply(Command::Reverse { layer: 0, from: 0, to: 2 }).unwrap();
    assert_eq!(e.project.layers[0].cells[0], Cell::Key(tooncraft_doc::DrawingId(2)));
    e.apply(Command::ClearRange { layers: vec![0], from: 4, to: 7, shift: false }).unwrap();
    e.apply(Command::Cycle { layer: 0, from: 0, to: 2, times: 1 }).unwrap();
    assert_eq!(sheet(&e.project.layers[0].cells), "#-##-#..");
    e.apply(Command::ClearRange { layers: vec![0], from: 0, to: 2, shift: true }).unwrap();
    assert_eq!(sheet(&e.project.layers[0].cells), "#-#.....");
    for _ in 0..4 {
        e.undo().unwrap();
    }
    assert_eq!(sheet(&e.project.layers[0].cells), "##-.##-.");
}

#[test]
fn import_image_sequence() {
    let dir = scratch("import");
    std::fs::create_dir_all(&dir).unwrap();
    let mut paths = Vec::new();
    for i in 0..3u32 {
        let mut img = tooncraft_raster::RgbaImage::transparent(640, 100);
        for x in 0..640 {
            for y in 46..54 {
                img.set_pixel(x, y, [0, 0, 0, 255]);
            }
        }
        img.set_pixel(i, 0, [255, 0, 0, 255]);
        let p = dir.join(format!("rough_{i}.png"));
        std::fs::write(&p, tooncraft_format::encode_png(&img).unwrap()).unwrap();
        paths.push(p.to_string_lossy().into_owned());
    }
    let mut e = engine();
    e.apply(Command::ImportImages { paths: paths.clone(), frame: 1, hold: 2, name: "Roughs".into() }).unwrap();
    let l = e.project.layers.last().unwrap();
    assert_eq!(l.name, "Roughs");
    assert_eq!(sheet(&l.cells), ".#-#-#-.");
    // 640 wide on a 320-wide canvas: scaled to fit, centred
    assert_eq!(e.pick_color(1, 160, 120), [0, 0, 0, 255], "the band lands mid-canvas");
    assert_ne!(e.pick_color(1, 160, 60), [0, 0, 0, 255]);
    paths.push(dir.join("missing.png").to_string_lossy().into_owned());
    let before = e.project.clone();
    assert!(matches!(e.apply(Command::ImportImages { paths, frame: 0, hold: 1, name: "x".into() }), Err(EngineError::Import(_))));
    assert_eq!(e.project, before, "a failed import changes nothing");
    e.undo().unwrap();
    assert_eq!(e.project.layers.len(), 1);
}

#[test]
fn set_exposure_is_one_undo_step() {
    let mut e = engine();
    e.apply(Command::NewDrawing { layer: 0, frame: 0 }).unwrap();
    e.apply(Command::NewDrawing { layer: 0, frame: 1 }).unwrap();
    e.apply(Command::SetExposure { layer: 0, frame: 0, length: 4 }).unwrap();
    assert_eq!(sheet(&e.project.layers[0].cells), "#---#...");
    e.undo().unwrap();
    assert_eq!(sheet(&e.project.layers[0].cells), "##......");
}

#[test]
fn large_demo_is_sparse() {
    let e = crate::demo::large(1920, 1080, 12).unwrap();
    assert_eq!(e.project.drawing_count(), 13);
    // a line drawing on a 1080p canvas uses a fraction of a full frame's memory
    let full = 1920u64 * 1080 * 4;
    assert!(e.project.memory_bytes() / 13 < full / 4, "{} bytes per drawing", e.project.memory_bytes() / 13);
}

#[test]
fn soundtrack_round_trip_and_mp4_with_audio() {
    let dir = scratch("sound");
    std::fs::create_dir_all(&dir).unwrap();
    let rate = 48_000u32;
    let tone: Vec<f32> = (0..rate).map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 0.5).collect();
    let wav = craft_audio::encode(
        &craft_audio::AudioBuffer { sample_rate: rate, channels: vec![tone.clone(), tone] },
        &craft_audio::EncodeOptions::new(craft_audio::EncodeFormat::Wav),
    )
    .unwrap();
    let path = dir.join("line.wav");
    std::fs::write(&path, &wav).unwrap();
    let mut e = engine();
    e.apply(Command::SetSound { path: Some(path.to_string_lossy().into_owned()), offset: 2 }).unwrap();
    let pcm = crate::audio::decode_all(&e.project).unwrap().remove(0);
    assert_eq!((pcm.rate, pcm.channels.len(), pcm.frames()), (rate, 2, rate as usize));
    let seg = crate::audio::mix_project(&e.project, 0, 4).unwrap().unwrap();
    assert_eq!(seg.frames(), rate as usize / 3);
    assert_eq!(seg.channels[0][0], 0.0, "silence before the sound starts at frame 2");
    e.apply(Command::MoveSound { offset: 0, index: 0 }).unwrap();
    e.undo().unwrap();
    assert_eq!(e.project.sounds[0].offset, 2);
    // a second clip on another track, trimmed, faded and keyed
    e.apply(Command::AddSound { path: path.to_string_lossy().into_owned(), offset: 6, track: 1 }).unwrap();
    let cmd: Command = serde_json::from_str(r#"{"cmd":"set_sound_clip","index":1,"trim_start":0.25,"length":0.5,"fade_in":0.1,"volume":0.5}"#).unwrap();
    e.apply(cmd).unwrap();
    e.apply(Command::SetVolumeKey { index: 1, frame: 6, gain: 0.0 }).unwrap();
    e.apply(Command::SetAudioTrack { track: 1, name: Some("Voice".into()), muted: None, solo: Some(true), volume: None }).unwrap();
    assert_eq!(e.project.audio_tracks.len(), 2);
    assert!(e.apply(Command::RemoveVolumeKey { index: 1, frame: 99 }).is_err());
    assert!(e.apply(Command::SetAudioTrack { track: 99, name: None, muted: None, solo: None, volume: None }).is_err());
    // round trip through the file
    let bytes = tooncraft_format::to_bytes(&e.project).unwrap();
    let back = tooncraft_format::from_bytes(&bytes).unwrap();
    assert_eq!((back.sounds.clone(), back.audio_tracks.clone()), (e.project.sounds.clone(), e.project.audio_tracks.clone()));
    // the mix as WAV
    let mixed = crate::audio::mix_project(&e.project, 0, e.project.length()).unwrap().unwrap();
    let wav = crate::audio::wav(&mixed).unwrap();
    let (_, back) = craft_audio::decode_with(&wav, Some("wav"), &craft_audio::DecodeOptions::default()).unwrap();
    assert_eq!(back.channels.len(), 2);
    e.apply(Command::RemoveSound { index: 1 }).unwrap();
    assert_eq!(e.project.sounds.len(), 1);
    // video with sound
    e.apply(Command::Stroke { layer: 0, frame: 0, brush: ink(), samples: circle(100.0, 100.0, 30.0) }).unwrap();
    let mp4 = dir.join("talk.mp4");
    crate::export::mp4(&e.project, &mp4, crate::export::ExportOptions::whole(&e.project), |_| {}).unwrap();
    let data = std::fs::read(&mp4).unwrap();
    let file = craft_media::isobmff::open(&data[..]).unwrap();
    assert_eq!(file.tracks.len(), 2, "video and sound");
    assert!(e.apply(Command::SetSound { path: Some(dir.join("nope.wav").to_string_lossy().into_owned()), offset: 0 }).is_err());
    e.apply(Command::SetSound { path: None, offset: 0 }).unwrap();
    assert!(e.project.sounds.is_empty());
}

#[test]
fn selection_commands() {
    let mut e = engine();
    e.apply(Command::Stroke { layer: 0, frame: 0, brush: Brush { size: 20.0, ..ink() }, samples: circle(60.0, 60.0, 20.0) }).unwrap();
    let sq = vec![(20.0, 20.0), (100.0, 20.0), (100.0, 100.0), (20.0, 100.0)];
    let before = e.pick_color(0, 60, 40);
    e.apply(Command::TransformSelection { layer: 0, frame: 0, polygon: sq.clone(), dx: 150.0, dy: 40.0, scale_x: 1.0, scale_y: 1.0, angle: 0.0 }).unwrap();
    assert_eq!(e.pick_color(0, 210, 80), before, "the ring moved by (150, 40)");
    assert_ne!(e.pick_color(0, 60, 40), before);
    e.undo().unwrap();
    assert_eq!(e.pick_color(0, 60, 40), before);
    e.apply(Command::ClearSelection { layer: 0, frame: 0, polygon: sq }).unwrap();
    assert!(e.project.drawing(e.project.drawing_at(0, 0).unwrap()).unwrap().is_empty());
    e.apply(Command::PastePixels { layer: 0, frame: 3, x: 10.0, y: 10.0, width: 2, height: 1, rgba: vec![255, 0, 0, 255, 0, 0, 255, 255] }).unwrap();
    assert_eq!(e.pick_color(3, 10, 10), [255, 0, 0, 255]);
    assert!(e.apply(Command::PastePixels { layer: 0, frame: 3, x: 0.0, y: 0.0, width: 5, height: 5, rgba: vec![0; 3] }).is_err());
}

#[test]
fn camera_keys_undo_and_exports_use_them() {
    let mut e = engine();
    e.apply(Command::Stroke { layer: 0, frame: 0, brush: Brush { size: 30.0, ..ink() }, samples: circle(40.0, 40.0, 10.0) }).unwrap();
    let plain = tooncraft_doc::render_camera(&e.project, 0, RenderOptions::default());
    e.apply(Command::SetCameraKey { frame: 0, x: 40.0, y: 40.0, zoom: 4.0, angle: 0.0 }).unwrap();
    assert!(e.project.camera.enabled);
    let cam = tooncraft_doc::render_camera(&e.project, 0, RenderOptions::default());
    assert_ne!(plain, cam);
    assert!(cam.pixel(160, 120 - 40)[3] == 255 && cam.pixel(160, 120 - 40)[0] < 100, "the ring fills the view");
    e.apply(Command::RemoveCameraKey { frame: 0 }).unwrap();
    assert!(e.project.camera.keys.is_empty());
    e.undo().unwrap();
    e.undo().unwrap();
    assert!(e.project.camera.keys.is_empty() && !e.project.camera.enabled);
}

#[test]
fn shapes_and_fill_all_frames() {
    let mut e = engine();
    for f in [0u32, 2, 4] {
        e.apply(Command::Shape {
            layer: 0,
            frame: f,
            brush: ink(),
            shape: crate::shapes::ShapeKind::Ellipse,
            from: (100.0, 60.0),
            to: (220.0, 180.0),
            constrain: false,
        })
        .unwrap();
    }
    assert_eq!(e.undo_label(), Some("Shape"));
    e.apply(Command::ExtendHold { layer: 0, frame: 0 }).unwrap();
    let red = [200, 20, 20, 255];
    e.apply(Command::FillAllFrames { layer: 0, x: 160, y: 120, color: red, options: FillOptions::default(), reference: FillReference::Layer }).unwrap();
    for f in [0u32, 1, 3, 5] {
        assert_eq!(e.pick_color(f, 160, 120), red, "frame {f}");
    }
    e.undo().unwrap();
    assert_ne!(e.pick_color(5, 160, 120), red, "one undo step clears every frame's fill");
}

#[test]
fn layer_blend_colour_and_folders() {
    let mut e = engine();
    e.apply(Command::AddLayer { name: "Shade".into(), index: None }).unwrap();
    e.apply(Command::SetLayerBlend { layer: 1, blend: tooncraft_doc::BlendMode::Multiply }).unwrap();
    e.apply(Command::SetLayerColor { layer: 1, color: Some([200, 50, 50]) }).unwrap();
    e.apply(Command::SetLayerFolder { layer: 1, folder: Some("Character".into()) }).unwrap();
    e.apply(Command::SetLayerFolder { layer: 0, folder: Some("Character".into()) }).unwrap();
    assert_eq!(e.project.folders.len(), 1);
    e.apply(Command::SetFolder { name: "Character".into(), visible: Some(false), opacity: None, collapsed: Some(true) }).unwrap();
    assert!(!e.project.effective(&e.project.layers[0]).0);
    e.apply(Command::SetLayerFolder { layer: 0, folder: None }).unwrap();
    e.apply(Command::SetLayerFolder { layer: 1, folder: None }).unwrap();
    assert!(e.project.folders.is_empty(), "empty folders go away");
    let bytes = tooncraft_format::to_bytes(&e.project).unwrap();
    assert_eq!(tooncraft_format::from_bytes(&bytes).unwrap(), e.project);
    for _ in 0..7 {
        e.undo().unwrap();
    }
    assert_eq!(e.project.layers[1].blend, tooncraft_doc::BlendMode::Normal);
    assert!(e.project.folders.is_empty() && e.project.layers[1].color.is_none());
}

#[test]
fn image_tip_strokes_and_undo() {
    let mut e = engine();
    let alpha = vec![255u8; 8 * 8];
    e.apply(Command::AddTip { id: 1, name: "square".into(), width: 8, height: 8, alpha }).unwrap();
    let brush = Brush { tip: Some(1), size: 20.0, ..ink() };
    e.apply(Command::Stroke { layer: 0, frame: 0, brush, samples: vec![Sample::at(100.0, 100.0, 1.0, 0.0)] }).unwrap();
    // a square tip covers the corner a round dab would miss
    assert_eq!(e.pick_color(0, 92, 92), [24, 22, 30, 255]);
    let bytes = tooncraft_format::to_bytes(&e.project).unwrap();
    assert_eq!(tooncraft_format::from_bytes(&bytes).unwrap().tips, e.project.tips);
    e.undo().unwrap();
    e.undo().unwrap();
    assert!(e.project.tips.is_empty());
}

#[test]
fn interchange_round_trips_through_other_apps_formats() {
    let dir = scratch("interop");
    std::fs::create_dir_all(&dir).unwrap();
    let mut e = engine();
    e.apply(Command::Stroke { layer: 0, frame: 0, brush: Brush { size: 12.0, ..ink() }, samples: circle(100.0, 100.0, 40.0) }).unwrap();
    e.apply(Command::ExtendHold { layer: 0, frame: 0 }).unwrap();
    e.apply(Command::Stroke { layer: 0, frame: 3, brush: Brush { size: 12.0, ..ink() }, samples: circle(200.0, 120.0, 40.0) }).unwrap();
    e.apply(Command::RenameLayer { layer: 0, name: "Ink".into() }).unwrap();
    // PSD out and back in
    let psd = dir.join("frame.psd");
    std::fs::write(&psd, tooncraft_interop::psd::export_frame(&e.project, 0).unwrap()).unwrap();
    let mut b = engine();
    b.apply(Command::ImportLayers { path: psd.to_string_lossy().into_owned(), frame: 0 }).unwrap();
    assert!(b.project.layers.iter().any(|l| l.name == "Ink"));
    assert_eq!(b.pick_color(0, 140, 100), e.pick_color(0, 140, 100), "the ring came back where it was");
    // OpenRaster out and back in
    let ora = dir.join("frame.ora");
    crate::export::ora_frame(&e.project, 3, &ora).unwrap();
    let mut c = engine();
    c.apply(Command::ImportLayers { path: ora.to_string_lossy().into_owned(), frame: 2 }).unwrap();
    assert_eq!(c.pick_color(2, 240, 120), e.pick_color(3, 240, 120));
    // XDTS sheet + drawing folders out and back in: same timing
    let files = tooncraft_interop::levels::export(&e.project, &dir.join("sheet")).unwrap();
    let mut d = engine();
    d.apply(Command::ImportSheet { path: files[0].to_string_lossy().into_owned() }).unwrap();
    let ink = d.project.layers.iter().find(|l| l.name == "Ink").unwrap();
    assert_eq!(sheet(&ink.cells), sheet(&e.project.layers[0].cells));
    // GIF out and back in
    let g = dir.join("shot.gif");
    crate::export::gif(&e.project, &g, crate::export::ExportOptions::whole(&e.project), |_| {}).unwrap();
    let mut f = engine();
    f.apply(Command::ImportGif { path: g.to_string_lossy().into_owned(), frame: 0 }).unwrap();
    assert_eq!(f.project.layers.len(), 2);
    assert!(f.project.drawing_count() >= 2);
    // a damaged file changes nothing
    std::fs::write(&psd, b"8BPS broken").unwrap();
    let before = b.project.clone();
    assert!(b.apply(Command::ImportLayers { path: psd.to_string_lossy().into_owned(), frame: 0 }).is_err());
    assert_eq!(b.project, before);
}

#[test]
fn pegs_move_layers_undo_save_and_fill_in_local_space() {
    use tooncraft_doc::{Ease, PegId};
    let mut e = engine();
    e.apply(Command::Stroke { layer: 0, frame: 0, brush: ink(), samples: circle(60.0, 60.0, 20.0) }).unwrap();
    e.apply(Command::ExtendHold { layer: 0, frame: 0 }).unwrap();
    e.apply(Command::ExtendHold { layer: 0, frame: 0 }).unwrap();
    let before = e.project.clone();
    e.apply(Command::AddPeg { name: "Body".into(), parent: None }).unwrap();
    let body = PegId(1);
    e.apply(Command::SetLayerPeg { layer: 0, peg: Some(body) }).unwrap();
    let key = |frame, x| Command::SetPegKey { peg: body, frame, x, y: 0.0, angle: 0.0, scale_x: 1.0, scale_y: 1.0, skew: 0.0, ease: Ease::Linear };
    e.apply(key(0, 0.0)).unwrap();
    e.apply(key(2, 100.0)).unwrap();
    let dark = |c: [u8; 4]| c[0] < 128;
    assert!(dark(e.pick_color(0, 40, 60)), "the ring at rest on frame 1");
    assert!(dark(e.pick_color(2, 140, 60)), "moved 100 px by frame 3");
    assert!(!dark(e.pick_color(2, 40, 60)));
    // a fill clicked in the drawing's own space lands inside the moved ring
    e.apply(Command::Fill { layer: 0, frame: 2, x: 60, y: 60, color: [200, 0, 0, 255], options: FillOptions::default(), reference: FillReference::AllLayers })
        .unwrap();
    assert_eq!(e.pick_color(2, 160, 60), [200, 0, 0, 255]);
    // nesting, loops refused, file round-trip
    e.apply(Command::AddPeg { name: "Arm".into(), parent: Some(body) }).unwrap();
    assert!(e.apply(Command::SetPegParent { peg: body, parent: Some(PegId(2)) }).is_err(), "no loops");
    assert!(e.apply(Command::SetLayerPeg { layer: 0, peg: Some(PegId(9)) }).is_err());
    e.apply(Command::SetLayerDepth { layer: 0, depth: 2.0 }).unwrap();
    e.apply(Command::AddCameraShake { shake: tooncraft_doc::Shake { from: 0, to: 5, amplitude: 4.0, rotation: 0.0, frequency: 0.4, fade: 0, seed: 3 } })
        .unwrap();
    let bytes = tooncraft_format::to_bytes(&e.project).unwrap();
    assert_eq!(tooncraft_format::from_bytes(&bytes).unwrap(), e.project);
    let json = serde_json::to_string(&key(1, 5.0)).unwrap();
    assert!(json.contains(r#""cmd":"set_peg_key""#) && json.contains(r#""ease":{"kind":"linear"}"#), "{json}");
    for _ in 0..8 {
        e.undo().unwrap();
    }
    assert_eq!(e.project, before, "eight successful commands, eight undo steps");
    assert_eq!(e.project.layers[0].peg, None);
}

#[test]
fn nested_folders_lock_clip_and_round_trip() {
    let mut e = engine();
    e.apply(Command::AddLayer { name: "Shade".into(), index: None }).unwrap();
    let before = e.project.clone();
    e.apply(Command::AddFolder { name: "Character".into(), parent: None, layers: vec![] }).unwrap();
    e.apply(Command::AddFolder { name: "Head".into(), parent: Some("Character".into()), layers: vec![0, 1] }).unwrap();
    assert!(e.apply(Command::AddFolder { name: "Head".into(), parent: None, layers: vec![] }).is_err(), "names are unique");
    assert!(e.apply(Command::SetFolderParent { name: "Character".into(), parent: Some("Head".into()) }).is_err(), "no loops");
    e.apply(Command::SetFolderLock { name: "Character".into(), locked: true }).unwrap();
    let err = e.apply(Command::Stroke { layer: 0, frame: 0, brush: ink(), samples: circle(50.0, 50.0, 10.0) });
    assert!(matches!(err, Err(EngineError::Locked(_))), "locked through the outer folder: {err:?}");
    e.apply(Command::InsertFrame { layer: None, frame: 0 }).unwrap();
    assert_eq!(e.project.layers[0].cells, before.layers[0].cells, "insert skips locked layers");
    e.apply(Command::SetFolderLock { name: "Character".into(), locked: false }).unwrap();
    e.apply(Command::SetLayerClip { layer: 1, clip: true }).unwrap();
    e.apply(Command::SetFolderColor { name: "Head".into(), color: Some([200, 40, 40]) }).unwrap();
    e.apply(Command::RenameFolder { name: "Character".into(), to: "Hero".into() }).unwrap();
    assert_eq!(e.project.folder("Head").and_then(|f| f.parent.clone()).as_deref(), Some("Hero"));
    let bytes = tooncraft_format::to_bytes(&e.project).unwrap();
    assert_eq!(tooncraft_format::from_bytes(&bytes).unwrap(), e.project);
    // taking the layers out drops the now-empty folders
    e.apply(Command::SetLayerFolder { layer: 0, folder: None }).unwrap();
    e.apply(Command::SetLayerFolder { layer: 1, folder: None }).unwrap();
    assert!(e.project.folders.is_empty());
    while e.project != before && e.undo().is_ok() {}
    assert_eq!(e.project, before);
}

#[test]
fn marks_presets_smart_retime_and_inbetween_guides() {
    use tooncraft_doc::{Ease, Mark, TimingPreset};
    let mut e = Engine::new(Project::new("Test", 320, 240, 12, 13).unwrap());
    e.apply(Command::Stroke { layer: 0, frame: 0, brush: ink(), samples: circle(60.0, 60.0, 20.0) }).unwrap();
    e.apply(Command::Stroke { layer: 0, frame: 12, brush: ink(), samples: circle(260.0, 60.0, 20.0) }).unwrap();
    let before = e.project.clone();
    e.apply(Command::SetMark { layer: 0, frame: 0, mark: Some(Mark::Key) }).unwrap();
    e.apply(Command::SetMark { layer: 0, frame: 12, mark: Some(Mark::Key) }).unwrap();
    assert!(e.apply(Command::SetMark { layer: 0, frame: 5, mark: Some(Mark::Key) }).is_err(), "no drawing at frame 6");
    // three inbetweens on an even chart, with guides on a new layer above
    e.apply(Command::AddInbetweens { layer: 0, frame: 0, count: 3, ease: Ease::Linear, guides: true }).unwrap();
    assert_eq!(sheet(&e.project.layers[0].cells), "#--#--#--#--#");
    assert_eq!(e.project.layers.len(), 2);
    let g = &e.project.layers[1];
    assert!(g.locked && g.name.ends_with("guides"));
    let mid = e.project.drawing(g.drawing_at(6).unwrap()).unwrap();
    assert!(mid.pixel(40, 60)[3] > 0 && mid.pixel(240, 60)[3] > 0, "the guide at the middle shows both drawings");
    assert!(mid.pixel(40, 60)[3] < 200, "each at half strength");
    // smart re-time with a slow start keeps the keys
    e.apply(Command::RetimeBetweenKeys { layer: 0, from: 0, to: 12, ease: Ease::In }).unwrap();
    let s = sheet(&e.project.layers[0].cells);
    assert!(s.starts_with('#') && s.ends_with('#') && s.matches('#').count() == 5, "{s}");
    assert_ne!(s, "#--#--#--#--#");
    // a preset over the whole range
    e.apply(Command::ApplyTiming { layer: 0, from: 0, to: 12, preset: TimingPreset::Even { frames: 2 } }).unwrap();
    assert_eq!(sheet(&e.project.layers[0].cells), "#-#-#-#-#-...");
    let bytes = tooncraft_format::to_bytes(&e.project).unwrap();
    assert_eq!(tooncraft_format::from_bytes(&bytes).unwrap(), e.project);
    let json = serde_json::to_string(&Command::ApplyTiming { layer: 0, from: 0, to: 3, preset: TimingPreset::SlowStart }).unwrap();
    assert!(json.contains(r#""preset":{"kind":"slow_start"}"#), "{json}");
    for _ in 0..5 {
        e.undo().unwrap();
    }
    assert_eq!(e.project, before);
}

#[test]
fn canvas_resize_and_crop_move_drawings_without_resampling() {
    let mut e = engine();
    e.apply(Command::Stroke { layer: 0, frame: 0, brush: ink(), samples: circle(60.0, 60.0, 20.0) }).unwrap();
    e.apply(Command::AddPeg { name: "P".into(), parent: None }).unwrap();
    e.apply(Command::SetCameraKey { frame: 0, x: 160.0, y: 120.0, zoom: 1.0, angle: 0.0 }).unwrap();
    let before = e.project.clone();
    let at = |e: &Engine, x, y| e.pick_color(0, x, y);
    let ring = at(&e, 40, 60);
    // grow to the right and down, keeping the top left in place
    e.apply(Command::ResizeCanvas { width: 640, height: 480, anchor: (0.0, 0.0) }).unwrap();
    assert_eq!((e.project.width(), e.project.height()), (640, 480));
    assert_eq!(at(&e, 40, 60), ring, "pixels are moved, not resampled");
    e.undo().unwrap();
    // centred: everything moves by half the growth, pivots and camera keys too
    e.apply(Command::ResizeCanvas { width: 420, height: 240, anchor: (0.5, 0.5) }).unwrap();
    assert_eq!(at(&e, 90, 60), ring);
    assert_eq!(e.project.pegs[0].pivot, (210.0, 120.0));
    assert_eq!(e.project.camera.keys[0].x, 210.0);
    e.undo().unwrap();
    assert_eq!(e.project, before);
    e.apply(Command::CropCanvas { x: 30, y: 30, width: 100, height: 80 }).unwrap();
    assert_eq!(at(&e, 10, 30), ring);
    assert!(e.apply(Command::CropCanvas { x: 0, y: 0, width: 0, height: 10 }).is_err());
    e.undo().unwrap();
    assert_eq!(e.project, before, "undo brings the cut paint back");
}

#[test]
fn boardcraft_board_becomes_a_layout() {
    let dir = scratch("board");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("boats.boardcraft");
    std::fs::write(&path, tooncraft_interop::boards::demo().unwrap()).unwrap();
    // a 128×64 shot: the 64×32 board scales up ×2
    let mut e = Engine::new(Project::new("Test", 128, 64, 12, 4).unwrap());
    let before = e.project.clone();
    e.apply(Command::ImportBoards { path: path.to_string_lossy().into_owned(), frame: 0, camera: true }).unwrap();
    let p = &e.project;
    assert_eq!(p.length(), 12, "the shot grew to the board's 12 frames");
    let b = &p.layers[0];
    assert!(b.locked && b.name.starts_with("Boards"));
    assert_eq!(sheet(&b.cells), "#-----#---#-");
    assert!(b.cells.iter().filter_map(|c| c.key()).all(|d| p.marks.get(&d) == Some(&tooncraft_doc::Mark::Key)));
    let first = p.drawing(b.drawing_at(0).unwrap()).unwrap();
    assert_eq!(first.pixel(10, 10), [200, 0, 0, 255], "scaled ×2: board pixel (5, 5) lands at (10, 10)");
    assert!(p.camera.enabled);
    assert_eq!(p.camera.keys.len(), 2);
    let k = p.camera.keys[1];
    assert_eq!((k.frame, k.x, k.y, k.zoom, k.angle), (5, 32.0, 32.0, 2.0, 10.0));
    e.undo().unwrap();
    assert_eq!(e.project, before);
    assert!(e.apply(Command::ImportBoards { path: dir.join("missing.boardcraft").to_string_lossy().into_owned(), frame: 0, camera: true }).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
