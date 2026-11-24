//! The interface language: picked in Preferences ▸ General from the keyboard, kept in the
//! preferences, applied at once, and a hostile code in the settings file follows the system.
//! The accessibility check is on.

use egui::{Key, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};

use crate::app::ToonApp;
use crate::args::Cli;
use crate::preferences::Prefs;

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli { open: Some("preferences".into()), ..Cli::default() }).without_autosave();
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
fn the_language_is_picked_from_the_keyboard_and_kept() {
    let mut h = harness();
    assert!(h.state().preferences.open);
    assert_eq!(h.state().preferences.prefs.language, "", "the system language by default");
    h.get(By::new().role(egui::Role::ComboBox).label("Language")).focus();
    h.run();
    h.key_press(Key::Space);
    h.run();
    h.get(By::new().label("English")).focus();
    h.run();
    h.key_press(Key::Space);
    h.run();
    h.run();
    assert_eq!(h.state().preferences.prefs.language, "en");
    assert_eq!(crate::i18n::current(), craft_i18n::Lang::EN);
    // Restore Defaults on the General tab goes back to the system language
    h.get_by_label("Restore Defaults").click();
    h.run();
    h.run();
    assert_eq!(h.state().preferences.prefs.language, "");
}

#[test]
fn a_chosen_language_reaches_the_interface_at_once() {
    let mut h = harness();
    // the pseudo locale (debug builds) marks every looked-up string, so a change shows; no frame
    // is drawn in it, since its marks have no glyph in the test fonts (kittest panics on tofu)
    h.state_mut().preferences.prefs.language = "pseudo".into();
    crate::preferences::apply_live(h.state_mut());
    assert_ne!(crate::tl!("General"), "General", "the interface follows the preference");
    h.state_mut().preferences.prefs.language = "en".into();
    crate::preferences::apply_live(h.state_mut());
    h.run();
    assert_eq!(crate::tl!("General"), "General");
}

#[test]
fn a_hostile_or_unknown_language_code_follows_the_system() {
    for bad in ["../../etc/passwd", "de\u{0}", &"x".repeat(200), "zh hant"] {
        let p = Prefs { language: bad.into(), ..Prefs::default() }.sanitized();
        assert_eq!(p.language, "", "{bad:?}");
    }
    let p = Prefs { language: " PT-BR ".into(), ..Prefs::default() }.sanitized();
    assert_eq!(p.language, "pt-br");
    // a well-formed code with no catalog is kept but shows English (craft-i18n falls back)
    crate::i18n::set_language(Some("xx"));
    assert_eq!(crate::i18n::current(), craft_i18n::Lang::EN);
    // and the setting survives a save and a load
    let json = serde_json::to_vec(&Prefs { language: "de".into(), ..Prefs::default() }).unwrap_or_default();
    let back: Prefs = serde_json::from_slice(&json).unwrap_or_default();
    assert_eq!(back.language, "de");
}
