use super::*;
use tooncraft_engine::Engine;

fn shot() -> Engine {
    let mut p = Project::new("Walk", 64, 48, 12, 8).unwrap();
    p.add_layer("Ink", 1).unwrap();
    p.markers.insert(2, "contact".into());
    Engine::new(p)
}

fn go(e: &mut Engine, src: &str) -> Report {
    run(e, src, "Script", &[("count".into(), "3".into())], &Limits::default())
}

#[test]
fn commands_reads_and_one_undo_step() {
    let mut e = shot();
    let r = go(
        &mut e,
        r#"
        let n = parse_int(ARGS.count);
        for f in 0..n { run(#{cmd: "new_drawing", layer: 1, frame: f * 2}); }
        print(`${name()} ${width()}x${height()} @ ${fps()} fps, ${length()} frames, ${layer_count()} layers`);
        print(layer_name(1) + " " + layer_visible(1) + " " + layer_locked(1));
        print(note(2) + " / " + note(3));
        print(drawing_at(1, 0) != () && drawing_at(1, 2) != ());
        let ls = layers();
        print(`${shot().length} ${ls.len()} ${ls[1].name} ${ls[1].index} ${ls[0].opacity}`);
        "#,
    );
    assert_eq!(r.error, None);
    assert_eq!(r.commands, 3);
    assert_eq!(r.output, ["Walk 64x48 @ 12 fps, 8 frames, 2 layers", "Ink true false", "contact / ", "true", "8 2 Ink 1 1.0"]);
    assert!(e.project.layers[1].drawing_at(4).is_some());
    assert_eq!(e.undo_label(), Some("Script"), "the run is one step");
    e.undo().unwrap();
    assert!(e.project.layers[1].drawing_at(0).is_none(), "and undoing it takes every command back");
    assert!(!e.can_undo());
}

#[test]
fn a_failing_script_changes_nothing_and_says_where() {
    let mut e = shot();
    e.apply(Command::NewDrawing { layer: 0, frame: 0 }).unwrap();
    let before = e.project.clone();
    let r = go(&mut e, "run(#{cmd: \"new_drawing\", layer: 1, frame: 1});\n\nrun(#{cmd: \"no_such_command\"});");
    let err = r.error.unwrap();
    assert_eq!(err.line, Some(3), "{err}");
    assert!(err.message.contains("not a command"), "{err}");
    assert_eq!(e.project, before, "the first command is rolled back");
    assert_eq!(e.undo_label(), Some("New drawing").or(e.undo_label()), "history as it was");
    assert_eq!(e.undo_len(), 1);
    // a command the engine refuses is an error too
    let r = go(&mut e, "run(#{cmd: \"new_drawing\", layer: 99, frame: 0});");
    assert!(r.error.is_some());
    assert_eq!(e.project, before);
    // a parse error
    let r = go(&mut e, "let x = ;");
    assert!(r.error.unwrap().line.is_some());
}

#[test]
fn a_caught_error_keeps_the_rest() {
    let mut e = shot();
    let r = go(
        &mut e,
        r#"try { run(#{cmd: "new_drawing", layer: 99, frame: 0}); } catch (err) { print("refused"); } run(#{cmd: "new_drawing", layer: 1, frame: 5});"#,
    );
    assert_eq!(r.error, None);
    assert_eq!(r.output, ["refused"]);
    assert!(e.project.layers[1].drawing_at(5).is_some());
}

#[test]
fn limits_stop_hostile_scripts() {
    let mut e = shot();
    let before = e.project.clone();
    let tight = Limits { max_operations: 100_000, max_time: Duration::from_secs(2), max_commands: 5, max_output_lines: 3, ..Limits::default() };
    let cases = [
        "loop {}",
        "fn f(x) { f(x + 1) } f(0);",
        "let s = \"x\"; loop { s += s; }",
        "let a = []; loop { a.push(1); }",
        "for f in 0..100 { run(#{cmd: \"new_drawing\", layer: 1, frame: f % 8}); }",
        "eval(\"1\")",
        "import \"x\" as y;",
    ];
    for src in cases {
        let r = run(&mut e, src, "Script", &[], &tight);
        assert!(r.error.is_some(), "{src} must stop");
        assert_eq!(e.project, before, "{src} changed the shot");
    }
    let r = run(&mut e, "for i in 0..10 { print(i); }", "Script", &[], &tight);
    assert_eq!(r.output.len(), 4, "three lines and a note that more were dropped");
    let big = "x".repeat(tight.max_source_bytes + 1);
    assert!(run(&mut e, &big, "Script", &[], &tight).error.is_some());
}

#[test]
fn a_script_that_changes_nothing_adds_no_step() {
    let mut e = shot();
    e.apply(Command::NewDrawing { layer: 0, frame: 0 }).unwrap();
    e.undo().unwrap();
    assert!(e.can_redo());
    let r = go(&mut e, "print(length());");
    assert_eq!(r.error, None);
    assert!(e.can_redo(), "redo survives a read-only script");
    assert!(!e.can_undo());
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(64))]
    #[test]
    fn any_text_is_refused_or_run_without_panicking(src in "\\PC{0,200}") {
        let mut e = shot();
        let tight = Limits { max_operations: 20_000, max_time: Duration::from_millis(200), ..Limits::default() };
        let _ = run(&mut e, &src, "Script", &[], &tight);
    }
}
