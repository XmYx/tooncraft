//! Interface language: lookups through `craft-i18n` (craft-libs) over the optional craft-i18n
//! catalogs embedded by `build.rs`. The English source text is the key; every user-visible string
//! goes through [`tl!`] (or [`tlf!`] for `{name}` placeholders).

use std::cell::Cell;
use std::sync::LazyLock;

include!(concat!(env!("OUT_DIR"), "/craft_i18n.rs"));

/// Languages ToonCraft's interface is ready for (left-to-right scripts until the layout handles
/// right-to-left). Only those with a `tooncraft.tsv` in the embedded catalogs are offered.
pub const LANGUAGES: &[&str] = &["de", "es", "fr", "id", "it", "ja", "ko", "nl", "pl", "pt-br", "ru", "uk", "zh-hans", "zh-hant"];

pub static I18N: LazyLock<craft_i18n::I18n> = LazyLock::new(|| {
    let (i18n, problems) = craft_i18n::I18n::new(CRAFT_I18N, LANGUAGES);
    for p in &problems {
        log::warn!("translation catalog: {p}");
    }
    i18n
});

thread_local! {
    pub static CURRENT: Cell<craft_i18n::Lang> = const { Cell::new(craft_i18n::Lang::EN) };
}

/// Pick the interface language: `None` follows the system.
pub fn set_language(code: Option<&str>) {
    let lang = code.and_then(|c| I18N.from_code(c)).unwrap_or_else(|| I18N.system_lang());
    CURRENT.set(lang);
}

pub fn current() -> craft_i18n::Lang {
    CURRENT.get()
}

/// Translate a source text.
#[macro_export]
macro_rules! tl {
    ($s:expr) => {
        $crate::i18n::I18N.t($crate::i18n::current(), $s)
    };
}

/// Translate a source text with `{name}` placeholders: `tlf!("Layer {name}", name = n)`.
#[macro_export]
macro_rules! tlf {
    ($s:expr $(, $k:ident = $v:expr)* $(,)?) => {
        craft_i18n::fmt($crate::tl!($s), &[$((stringify!($k), &$v.to_string())),*])
    };
}

// The generated table names `EmbeddedFace` unqualified (it is empty unless the build embeds craft-fonts).
use craft_fonts_build::EmbeddedFace;
pub static CRAFT_FONTS: &[EmbeddedFace] = include!(concat!(env!("OUT_DIR"), "/craft_fonts.rs"));

/// Add the craft-fonts faces (if the build embedded any) behind egui's own, as fallbacks for
/// scripts egui's fonts lack.
pub fn install_fonts(ctx: &egui::Context) {
    if CRAFT_FONTS.is_empty() {
        return;
    }
    let mut fonts = egui::FontDefinitions::default();
    for face in CRAFT_FONTS {
        let name = face.name();
        let mut data = egui::FontData::from_static(face.bytes);
        data.index = face.index;
        fonts.font_data.insert(name.clone(), std::sync::Arc::new(data));
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts.families.entry(family).or_default().push(name.clone());
        }
    }
    ctx.set_fonts(fonts);
}
