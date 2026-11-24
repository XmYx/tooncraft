//! End to end: create a shot, drive it with JSON-lines commands, export, check.

use std::process::Command;

fn cli(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_tooncraft-cli")).args(args).output().unwrap();
    (out.status.success(), format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

#[test]
fn create_script_export_check() {
    let dir = std::env::temp_dir().join(format!("tooncraft-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let shot = dir.join("shot.toon");
    let shot_s = shot.to_str().unwrap();
    let (ok, out) = cli(&["new", shot_s, "--size", "320x240", "--fps", "12", "--frames", "6"]);
    assert!(ok, "{out}");
    let cmds = dir.join("cmds.jsonl");
    std::fs::write(
        &cmds,
        r#"# a circle on frame 1, held two frames, then a fill
{"cmd":"stroke","layer":0,"frame":0,"brush":{"kind":"ink","size":5},"samples":[{"x":100,"y":60},{"x":160,"y":80},{"x":180,"y":140},{"x":120,"y":180},{"x":70,"y":130},{"x":100,"y":60}]}
{"cmd":"extend_hold","layer":0,"frame":0}
{"cmd":"fill","layer":0,"frame":0,"x":125,"y":120,"color":[230,90,70,255]}
{"cmd":"add_layer","name":"Paint"}
"#,
    )
    .unwrap();
    let (ok, out) = cli(&["run", shot_s, cmds.to_str().unwrap()]);
    assert!(ok, "{out}");
    let (ok, out) = cli(&["info", shot_s]);
    assert!(ok && out.contains("\"sheet\": \"#-....\"") && out.contains("Paint"), "{out}");
    let png = dir.join("png");
    let (ok, out) = cli(&["export", shot_s, "--png", png.to_str().unwrap(), "--range", "1..3"]);
    assert!(ok, "{out}");
    assert!(png.join("frame_0003.png").is_file());
    let gif = dir.join("shot.gif");
    assert!(cli(&["export", shot_s, "--gif", gif.to_str().unwrap()]).0);
    assert!(gif.is_file());
    let (ok, out) = cli(&["check", shot_s]);
    assert!(ok && out.contains("loads fine"), "{out}");
    // a bad command reports the line and leaves the file as it was
    std::fs::write(&cmds, "{\"cmd\":\"retime\",\"layer\":0,\"from\":0,\"to\":99,\"step\":2}\n").unwrap();
    let (ok, out) = cli(&["run", shot_s, cmds.to_str().unwrap()]);
    assert!(!ok && out.contains("line 1"), "{out}");
    let (ok, _) = cli(&["bogus"]);
    assert!(!ok);
}

#[test]
fn pegs_from_a_command_file_move_the_export() {
    let dir = std::env::temp_dir().join(format!("tooncraft-cli-pegs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let shot = dir.join("pegs.toon");
    let shot_s = shot.to_str().unwrap();
    assert!(cli(&["new", shot_s, "--size", "200x100", "--fps", "12", "--frames", "4"]).0);
    let cmds = dir.join("pegs.jsonl");
    std::fs::write(
        &cmds,
        r#"{"cmd":"stroke","layer":0,"frame":0,"brush":{"kind":"ink","size":12},"samples":[{"x":20,"y":50},{"x":30,"y":50}]}
{"cmd":"set_exposure","layer":0,"frame":0,"length":4}
{"cmd":"add_peg","name":"Body"}
{"cmd":"add_peg","name":"Arm","parent":1}
{"cmd":"set_layer_peg","layer":0,"peg":2}
{"cmd":"set_peg_key","peg":1,"frame":0,"ease":{"kind":"linear"}}
{"cmd":"set_peg_key","peg":1,"frame":3,"x":120}
{"cmd":"set_layer_depth","layer":0,"depth":0.5}
{"cmd":"add_camera_shake","shake":{"from":0,"to":3,"amplitude":3,"frequency":0.5}}
"#,
    )
    .unwrap();
    let (ok, out) = cli(&["run", shot_s, cmds.to_str().unwrap()]);
    assert!(ok, "{out}");
    let (ok, out) = cli(&["info", shot_s]);
    assert!(ok && out.contains("\"peg\": \"Arm\"") && out.contains("\"parent\": 1") && out.contains("\"camera_shakes\": 1"), "{out}");
    let png = dir.join("png");
    let (ok, out) = cli(&["export", shot_s, "--png", png.to_str().unwrap()]);
    assert!(ok, "{out}");
    assert!(png.join("frame_0004.png").is_file());
    // a loop is refused with the line number
    std::fs::write(&cmds, "{\"cmd\":\"set_peg_parent\",\"peg\":1,\"parent\":2}\n").unwrap();
    let (ok, out) = cli(&["run", shot_s, cmds.to_str().unwrap()]);
    assert!(!ok && out.contains("line 1") && out.contains("peg"), "{out}");
}

#[test]
fn nested_folders_and_clipping_from_a_command_file() {
    let dir = std::env::temp_dir().join(format!("tooncraft-cli-folders-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let shot = dir.join("f.toon");
    let shot_s = shot.to_str().unwrap();
    assert!(cli(&["new", shot_s, "--size", "64x64", "--frames", "2"]).0);
    let cmds = dir.join("f.jsonl");
    std::fs::write(
        &cmds,
        r#"{"cmd":"add_layer","name":"Shade"}
{"cmd":"add_folder","name":"Hero"}
{"cmd":"add_folder","name":"Head","parent":"Hero","layers":[0,1]}
{"cmd":"set_layer_clip","layer":1,"clip":true}
{"cmd":"set_folder_lock","name":"Hero","locked":true}
"#,
    )
    .unwrap();
    let (ok, out) = cli(&["run", shot_s, cmds.to_str().unwrap()]);
    assert!(ok, "{out}");
    let (ok, out) = cli(&["info", shot_s]);
    assert!(ok && out.contains("\"clip\": true") && out.contains("\"effectively_locked\": true") && out.contains("\"parent\": \"Hero\""), "{out}");
    std::fs::write(&cmds, "{\"cmd\":\"set_folder_parent\",\"name\":\"Hero\",\"parent\":\"Head\"}\n").unwrap();
    let (ok, out) = cli(&["run", shot_s, cmds.to_str().unwrap()]);
    assert!(!ok && out.contains("line 1"), "{out}");
}

#[test]
fn timing_commands_from_a_command_file() {
    let dir = std::env::temp_dir().join(format!("tooncraft-cli-timing-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let shot = dir.join("t.toon");
    let shot_s = shot.to_str().unwrap();
    assert!(cli(&["new", shot_s, "--size", "64x64", "--frames", "9"]).0);
    let cmds = dir.join("t.jsonl");
    std::fs::write(
        &cmds,
        r#"{"cmd":"new_drawing","layer":0,"frame":0}
{"cmd":"new_drawing","layer":0,"frame":8}
{"cmd":"set_mark","layer":0,"frame":0,"mark":"key"}
{"cmd":"set_mark","layer":0,"frame":8,"mark":"key"}
{"cmd":"add_inbetweens","layer":0,"frame":0,"count":3,"ease":{"kind":"linear"},"guides":false}
{"cmd":"retime_between_keys","layer":0,"from":0,"to":8,"ease":{"kind":"in"}}
{"cmd":"resize_canvas","width":96,"height":64,"anchor":[0,0]}
{"cmd":"crop_canvas","x":8,"y":0,"width":80,"height":48}
"#,
    )
    .unwrap();
    let (ok, out) = cli(&["run", shot_s, cmds.to_str().unwrap()]);
    assert!(ok, "{out}");
    let (ok, out) = cli(&["info", shot_s]);
    assert!(ok && out.contains("\"mark\": \"key\"") && out.contains("\"sheet\": \"##-#-#--#\"") && out.contains("\"size\": [\n    80,\n    48\n  ]"), "{out}");
}

#[test]
fn boardcraft_board_imports_as_layout() {
    let dir = std::env::temp_dir().join(format!("tooncraft-cli-pipe-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let board = dir.join("boats.boardcraft");
    std::fs::write(&board, tooncraft_interop::boards::demo().unwrap()).unwrap();
    let shot = dir.join("p.toon");
    let shot_s = shot.to_str().unwrap();
    assert!(cli(&["new", shot_s, "--size", "128x64", "--frames", "2"]).0);
    let cmds = dir.join("p.jsonl");
    std::fs::write(&cmds, format!("{{\"cmd\":\"import_boards\",\"path\":{:?}}}\n", board.to_str().unwrap())).unwrap();
    let (ok, out) = cli(&["run", shot_s, cmds.to_str().unwrap()]);
    assert!(ok, "{out}");
    let (ok, out) = cli(&["info", shot_s]);
    assert!(ok && out.contains("\"frames\": 12") && out.contains("Boards: Boats"), "{out}");
    let png = dir.join("png");
    let (ok, out) = cli(&["export", shot_s, "--png", png.to_str().unwrap(), "--range", "1..2"]);
    assert!(ok && png.join("frame_0002.png").is_file(), "{out}");
}

#[test]
fn export_webp_tga_tiff_and_layered_frames() {
    let dir = std::env::temp_dir().join(format!("tooncraft-cli-export-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let shot = dir.join("shot.toon");
    let s = shot.to_str().unwrap();
    assert!(cli(&["demo", s, "--size", "160x90"]).0);
    let d = |n: &str| dir.join(n).to_str().unwrap().to_string();
    let (ok, out) = cli(&["export", s, "--webp", &d("a.webp"), "--range", "1..4"]);
    assert!(ok, "{out}");
    let bytes = std::fs::read(dir.join("a.webp")).unwrap();
    assert_eq!(&bytes[..4], b"RIFF");
    assert!(bytes.windows(4).any(|w| w == b"ANIM"), "animated");
    let (ok, out) = cli(&["export", s, "--webp", &d("b.webp"), "--range", "1..4", "--lossy", "--quality", "70", "--transparent"]);
    assert!(ok, "{out}");
    assert!(cli(&["export", s, "--webp-frame", &d("still.webp"), "--frame", "3"]).0);
    assert_eq!(&std::fs::read(dir.join("still.webp")).unwrap()[8..12], b"WEBP");
    let (ok, out) = cli(&["export", s, "--tga", &d("tga"), "--range", "1..3"]);
    assert!(ok && out.contains("3 TGA files"), "{out}");
    assert!(dir.join("tga/frame_0002.tga").is_file());
    let (ok, out) = cli(&["export", s, "--tiff", &d("tif"), "--range", "2..3"]);
    assert!(ok && out.contains("2 TIFF files"), "{out}");
    let (ok, out) = cli(&["export", s, "--exr-layers", &d("exr"), "--range", "2..3"]);
    assert!(ok && out.contains("2 layered OpenEXR files"), "{out}");
    assert_eq!(&std::fs::read(dir.join("exr/frame_0003.exr")).unwrap()[..4], &[0x76, 0x2F, 0x31, 0x01]);
    let (ok, out) = cli(&["export", s, "--psd-frames", &d("all.psd"), "--range", "1..3"]);
    assert!(ok && out.contains("3 frames"), "{out}");
    let psd = std::fs::read(dir.join("all.psd")).unwrap();
    assert_eq!(&psd[..4], b"8BPS");
    assert_eq!(&psd[4..6], &[0, 1]);
    assert!(cli(&["export", s, "--psd-frames", &d("all.psb"), "--psb", "--range", "1..2"]).0);
    assert_eq!(&std::fs::read(dir.join("all.psb")).unwrap()[4..6], &[0, 2]);
    // bad input is an error
    assert!(!cli(&["export", s, "--webp", &d("c.webp"), "--quality", "300", "--lossy"]).0);
    assert!(!cli(&["export", s, "--tga", &d("tga2"), "--range", "9..2"]).0);
}

#[test]
fn brush_files_make_inspect_and_preview() {
    let dir = std::env::temp_dir().join(format!("tooncraft-cli-brush-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("wash.toonbrush");
    let (ok, out) = cli(&["brush", "make", "watercolour", file.to_str().unwrap(), "--name", "Wash"]);
    assert!(ok, "{out}");
    let (ok, out) = cli(&["brush", "info", file.to_str().unwrap()]);
    assert!(ok && out.contains("\"name\": \"Wash\"") && out.contains("\"kind\": \"watercolour\"") && out.contains("\"pickup\""), "{out}");
    // the dynamics are listed as mappings, curve included
    assert!(out.contains("\"target\": \"size\"") && out.contains("\"source\": \"pressure\""), "{out}");
    let png = dir.join("wash.png");
    let (ok, out) = cli(&["brush", "preview", file.to_str().unwrap(), png.to_str().unwrap(), "--size", "200x60"]);
    assert!(ok, "{out}");
    let bytes = std::fs::read(&png).unwrap();
    assert_eq!(&bytes[..4], b"\x89PNG");
    // bad input is an error, not a panic
    assert!(!cli(&["brush", "make", "nonsense", file.to_str().unwrap()]).0);
    assert!(!cli(&["brush", "info", dir.join("missing.toonbrush").to_str().unwrap()]).0);
    std::fs::write(&file, b"junk").unwrap();
    assert!(!cli(&["brush", "info", file.to_str().unwrap()]).0);
    assert!(!cli(&["brush", "preview", file.to_str().unwrap(), png.to_str().unwrap(), "--size", "1x1"]).0);
}

#[test]
fn text_commands_use_the_fonts_given() {
    let dir = std::env::temp_dir().join(format!("tooncraft-cli-text-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let shot = dir.join("shot.toon");
    let shot_s = shot.to_str().unwrap();
    assert!(cli(&["new", shot_s, "--size", "320x120", "--frames", "2"]).0);
    let font = dir.join("Hack.ttf");
    std::fs::write(&font, epaint_default_fonts::HACK_REGULAR).unwrap();
    let cmds = dir.join("text.jsonl");
    std::fs::write(&cmds, r#"{"cmd":"text","layer":0,"frame":0,"text":"Title","x":10,"y":10,"style":{"font":"Hack","size":40}}"#).unwrap();
    // without the font: an error that says why
    let (ok, out) = cli(&["run", shot_s, cmds.to_str().unwrap()]);
    assert!(!ok && out.contains("Hack"), "{out}");
    let (ok, out) = cli(&["run", shot_s, cmds.to_str().unwrap(), "--font", font.to_str().unwrap()]);
    assert!(ok, "{out}");
    let (ok, out) = cli(&["info", shot_s]);
    assert!(ok && out.contains("\"drawings\": 1"), "{out}");
    let junk = dir.join("junk.ttf");
    std::fs::write(&junk, b"no font here").unwrap();
    let (ok, out) = cli(&["run", shot_s, cmds.to_str().unwrap(), "--font", junk.to_str().unwrap()]);
    assert!(!ok && out.contains("not a font"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn export_video_codecs() {
    struct Gone(std::path::PathBuf);
    impl Drop for Gone {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let dir = std::env::temp_dir().join(format!("tooncraft-cli-codecs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let _gone = Gone(dir.clone());
    let shot = dir.join("s.toon");
    let shot_s = shot.to_str().unwrap();
    assert!(cli(&["demo", shot_s, "--size", "160x120"]).0);
    for (flag, name, codec) in [("--mov", "a.mov", "h264"), ("--mp4", "b.mp4", "h265"), ("--mp4", "c.mp4", "av1")] {
        let out = dir.join(name);
        let (ok, msg) = cli(&["export", shot_s, flag, out.to_str().unwrap(), "--codec", codec, "--range", "1..3"]);
        assert!(ok && out.is_file(), "{name}: {msg}");
    }
    let (ok, msg) = cli(&["export", shot_s, "--mp4", dir.join("d.mp4").to_str().unwrap(), "--codec", "mpeg1"]);
    assert!(!ok && msg.contains("--codec"), "{msg}");
}

/// Removes a test's folder when the test ends, also when it fails.
struct Gone(std::path::PathBuf);

impl Drop for Gone {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn import_video_round_trip() {
    let dir = std::env::temp_dir().join(format!("tooncraft-cli-video-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let _gone = Gone(dir.clone());
    let src = dir.join("src.toon");
    let src_s = src.to_str().unwrap();
    assert!(cli(&["demo", src_s, "--size", "160x120"]).0);
    let movie = dir.join("demo.mp4");
    let (ok, out) = cli(&["export", src_s, "--mp4", movie.to_str().unwrap(), "--range", "1..4"]);
    assert!(ok, "{out}");
    let shot = dir.join("roto.toon");
    let shot_s = shot.to_str().unwrap();
    assert!(cli(&["new", shot_s, "--size", "160x120", "--fps", "12", "--frames", "2"]).0);
    let (ok, out) = cli(&["import-video", shot_s, movie.to_str().unwrap(), "--every", "2"]);
    assert!(ok, "{out}");
    let (ok, out) = cli(&["info", shot_s]);
    assert!(ok && out.contains("\"demo\"") && out.contains("#-#-"), "{out}");
    let (ok, out) = cli(&["import-video", shot_s, dir.join("missing.mp4").to_str().unwrap()]);
    assert!(!ok, "{out}");
    let (ok, out) = cli(&["import-video", shot_s, movie.to_str().unwrap(), "--from", "-1"]);
    assert!(!ok && out.contains("--from"), "{out}");
}
