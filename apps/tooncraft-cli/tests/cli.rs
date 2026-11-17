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
