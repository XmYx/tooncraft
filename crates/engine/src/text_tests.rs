use super::*;
use crate::{Command, Engine};
use proptest::prelude::*;
use tooncraft_doc::Project;

const PAPER: [u8; 4] = [246, 243, 236, 255];

fn fonts() -> Fonts {
    let mut f = Fonts::default();
    assert!(f.add("Hack Regular", Arc::new(epaint_default_fonts::HACK_REGULAR), 0));
    assert!(f.add("Ubuntu Light", Arc::new(epaint_default_fonts::UBUNTU_LIGHT), 0));
    f
}

fn engine() -> Engine {
    let mut e = Engine::new(Project::new("Text", 320, 160, 12, 2).unwrap());
    e.fonts = fonts();
    e
}

fn text(t: &str, x: f32, y: f32, style: TextStyle) -> Command {
    Command::Text { layer: 0, frame: 0, text: t.into(), x, y, style }
}

/// FNV-1a over the drawing's pixels, in tile order.
fn digest(e: &Engine) -> u64 {
    let Some(s) = e.project.drawing_at(0, 0).and_then(|d| e.project.drawing(d)) else { return 0 };
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (i, tile) in s.tiles() {
        for b in (i as u32).to_le_bytes().iter().chain(tile.bytes()) {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// The painted pixel box `(x0, y0, x1, y1)` of frame 0.
fn ink_box(e: &Engine) -> Option<(u32, u32, u32, u32)> {
    let mut b: Option<(u32, u32, u32, u32)> = None;
    for y in 0..e.project.height() {
        for x in 0..e.project.width() {
            if e.pick_color(0, x, y) != PAPER {
                b = Some(match b {
                    None => (x, y, x, y),
                    Some((a, c, d, f)) => (a.min(x), c.min(y), d.max(x), f.max(y)),
                });
            }
        }
    }
    b
}

#[test]
fn text_renders_deterministically_for_a_fixed_font() {
    let style = TextStyle { font: "Hack Regular".into(), size: 32.0, color: [200, 30, 30, 255], ..TextStyle::default() };
    let mut a = engine();
    a.apply(text("Toon 42", 20.0, 20.0, style.clone())).unwrap();
    let mut b = engine();
    b.apply(text("Toon 42", 20.0, 20.0, style.clone())).unwrap();
    assert_eq!(digest(&a), digest(&b), "the same text paints the same pixels");
    // golden: the pixels of this text in this face (a change to shaping, flattening or coverage
    // shows here)
    assert_eq!(digest(&a), GOLDEN, "golden digest");
    let (x0, y0, x1, y1) = ink_box(&a).unwrap_or_default();
    // Hack is monospaced at 0.6 em: seven characters of 19.2 px; caps start below the top
    assert!((19..=22).contains(&x0), "starts at the anchor: {x0}");
    assert!((140..=156).contains(&x1), "about seven advances wide: {x1}");
    assert!(y0 > 20 && y1 < 20 + 40, "inside the line: {y0}…{y1}");
    assert_eq!(a.undo_len(), 1);
    assert_eq!(a.undo_label(), Some("Text"));
    a.undo().unwrap();
    assert!(ink_box(&a).is_none(), "one undo step takes it all away");
}

/// Pinned from the first run of the test above (Hack Regular from egui 0.37's default fonts).
const GOLDEN: u64 = 8964701479423756612;

#[test]
fn alignment_lines_and_spacing() {
    let f = fonts();
    let face = f.get("Hack Regular").cloned().unwrap();
    let base = TextStyle { size: 20.0, ..TextStyle::default() };
    let left = layout(&face, "abc", (100.0, 10.0), &base).unwrap();
    let centre = layout(&face, "abc", (100.0, 10.0), &TextStyle { align: TextAlign::Center, ..base.clone() }).unwrap();
    let right = layout(&face, "abc", (100.0, 10.0), &TextStyle { align: TextAlign::Right, ..base.clone() }).unwrap();
    let w = left.bounds.2 - left.bounds.0;
    assert!((w - 36.0).abs() < 0.5, "three 0.6 em advances: {w}");
    assert!((left.bounds.0 - 100.0).abs() < 1e-3);
    assert!((centre.bounds.0 - (100.0 - w / 2.0)).abs() < 1e-3);
    assert!((right.bounds.2 - 100.0).abs() < 1e-3);
    let two = layout(&face, "abc\nde", (0.0, 0.0), &base).unwrap();
    let double = layout(&face, "abc\nde", (0.0, 0.0), &TextStyle { line_spacing: 2.0, ..base.clone() }).unwrap();
    assert_eq!(two.glyphs.len(), 5);
    let gap = |l: &Layout| l.glyphs[3].y - l.glyphs[0].y;
    assert!((gap(&double) - 2.0 * gap(&two)).abs() < 1e-3, "line spacing scales the line gap");
    let spaced = layout(&face, "abc", (0.0, 0.0), &TextStyle { letter_spacing: 5.0, ..base.clone() }).unwrap();
    assert!(((spaced.bounds.2 - spaced.bounds.0) - (w + 10.0)).abs() < 1e-3, "two gaps of 5 px");
}

#[test]
fn missing_fonts_and_hostile_input() {
    let mut e = Engine::new(Project::new("Text", 64, 64, 12, 1).unwrap());
    let err = e.apply(text("Hi", 4.0, 4.0, TextStyle::default())).unwrap_err();
    assert!(err.to_string().contains("no font"), "{err}");
    e.fonts = fonts();
    let err = e.apply(text("Hi", 4.0, 4.0, TextStyle { font: "Nope".into(), ..TextStyle::default() })).unwrap_err();
    assert!(err.to_string().contains("Nope"), "{err}");
    assert!(e.apply(text("Hi", f32::NAN, 4.0, TextStyle::default())).is_err());
    // empty or blank text paints nothing and makes no undo step
    e.apply(text(" \n\t ", 4.0, 4.0, TextStyle::default())).unwrap();
    assert_eq!(e.undo_len(), 0);
    assert!(e.project.drawing_at(0, 0).is_none(), "no drawing made for nothing");
    // off the canvas: never a crash
    e.apply(text("far away", 1.0e5, 1.0e5, TextStyle::default())).unwrap();
    // a huge string is capped
    let long: String = "W".repeat(100_000);
    assert_eq!(clean_text(&long).chars().count(), MAX_TEXT_CHARS);
    let lines: String = "a\n".repeat(1000);
    assert_eq!(clean_text(&lines).lines().count(), MAX_LINES);
    let face = e.fonts.get("").cloned().unwrap();
    let lay = layout(&face, &long, (0.0, 0.0), &TextStyle::default()).unwrap();
    assert!(lay.glyphs.len() <= MAX_GLYPHS);
    // a hostile style is clamped
    let s = TextStyle { size: f32::INFINITY, line_spacing: -3.0, letter_spacing: f32::NAN, font: "\u{0}x".into(), ..TextStyle::default() }.sanitized();
    assert_eq!((s.size, s.line_spacing, s.letter_spacing, s.font.as_str()), (48.0, 0.5, 0.0, "x"));
    // not a font
    let mut f = Fonts::default();
    assert!(!f.add("junk", Arc::new(b"not a font at all".to_vec()), 0));
    assert!(!f.add("hack, wrong face", Arc::new(epaint_default_fonts::HACK_REGULAR), 7));
    assert!(f.is_empty());
}

#[test]
fn text_is_a_command_in_json() {
    let line = r#"{"cmd":"text","layer":0,"frame":0,"text":"Hello","x":160,"y":10,"style":{"size":24,"align":"center"}}"#;
    let cmd: Command = serde_json::from_str(line).unwrap();
    let mut e = engine();
    e.apply(cmd).unwrap();
    let (x0, _, x1, _) = ink_box(&e).unwrap_or_default();
    assert!(x0 < 160 && x1 > 160, "centred on x = 160: {x0}…{x1}");
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]
    #[test]
    fn any_text_and_style_never_panics(t in "\\PC{0,40}", size in prop_oneof![Just(f32::NAN), -10.0f32..1200.0],
        x in -400.0f32..700.0, y in -400.0f32..500.0, ls in -50.0f32..50.0, align in 0u8..3) {
        let mut e = engine();
        let align = [TextAlign::Left, TextAlign::Center, TextAlign::Right][align as usize];
        let style = TextStyle { size, letter_spacing: ls, align, ..TextStyle::default() };
        let r = e.apply(text(&t, x, y, style));
        // only a glyph whose box passes the coverage cap may refuse
        if let Err(err) = r {
            prop_assert!(err.to_string().contains("too large"), "{}", err);
        }
    }
}
