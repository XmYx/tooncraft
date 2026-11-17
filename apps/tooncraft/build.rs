//! Optional build inputs, never Cargo dependencies:
//!
//! - `CRAFT_I18N_DIR`: a craft-i18n checkout. Embeds `locales/<locale>/common.tsv` and
//!   `locales/<locale>/tooncraft.tsv` for every locale that has ToonCraft's own catalog, as
//!   `CRAFT_I18N` in `OUT_DIR/craft_i18n.rs` (recipe: craft-i18n `docs/integration.md`). The
//!   catalogs kept in this repository (`locales/<locale>/tooncraft.tsv`) are embedded too, for the
//!   locales the checkout has none for. Without either the table is empty and the app is
//!   English-only. `CRAFT_I18N_REQUIRED=1` (releases) turns a missing
//!   or invalid checkout into an error.
//! - `CRAFT_FONTS_DIR`: a craft-fonts checkout, embedded through `craft-fonts-build` as
//!   `OUT_DIR/craft_fonts.rs` (`CRAFT_FONTS_REQUIRED=1` for releases). Unset: egui's fonts only.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const APP: &str = "tooncraft";

/// The locales with a ToonCraft catalog under `locales_dir`: (code, shared catalog, ToonCraft catalog).
fn catalogs_in(locales_dir: &Path) -> Result<Vec<(String, Option<PathBuf>, PathBuf)>, String> {
    println!("cargo::rerun-if-changed={}", locales_dir.display());
    let mut entries: Vec<PathBuf> = std::fs::read_dir(locales_dir).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    entries.sort();
    let mut out = Vec::new();
    for loc in entries {
        let app = loc.join(format!("{APP}.tsv"));
        if !app.is_file() {
            continue; // only locales with ToonCraft's own catalog are offered
        }
        let Some(code) = loc.file_name().and_then(|n| n.to_str()) else { continue };
        let common = loc.join("common.tsv");
        out.push((code.to_string(), common.is_file().then_some(common), app));
    }
    Ok(out)
}

fn i18n() -> Result<String, String> {
    println!("cargo::rerun-if-env-changed=CRAFT_I18N_DIR");
    println!("cargo::rerun-if-env-changed=CRAFT_I18N_REQUIRED");
    let required = std::env::var("CRAFT_I18N_REQUIRED").is_ok_and(|v| v == "1");
    let dir = std::env::var_os("CRAFT_I18N_DIR").filter(|d| !d.is_empty()).map(PathBuf::from);
    let mut out =
        String::from("/// (locale, shared catalog, ToonCraft catalog); \"\" for a missing file.\npub static CRAFT_I18N: &craft_i18n::Embedded = &[\n");
    // The craft-i18n checkout first; the catalogs kept in this repository (`locales/<locale>/
    // tooncraft.tsv`) fill in the locales the checkout has no ToonCraft catalog for.
    let mut found: std::collections::BTreeMap<String, (Option<PathBuf>, PathBuf)> = std::collections::BTreeMap::new();
    match dir.as_ref().map(|d| d.join("locales")).filter(|l| l.is_dir()) {
        Some(locales) => {
            for (code, common, app) in catalogs_in(&locales)? {
                found.insert(code, (common, app));
            }
        }
        None => {
            if let Some(d) = &dir {
                let msg = format!("CRAFT_I18N_DIR={} is not a craft-i18n checkout (no locales/)", d.display());
                if required {
                    return Err(msg);
                }
                println!("cargo::warning={msg}; building with the catalogs in this repository only");
            } else if required {
                return Err("CRAFT_I18N_REQUIRED=1 but CRAFT_I18N_DIR is not set".into());
            }
        }
    }
    let own = Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from).unwrap_or_default()).join("../../locales");
    if own.is_dir() {
        for (code, common, app) in catalogs_in(&own)? {
            found.entry(code).or_insert((common, app));
        }
    }
    for (code, (common, app)) in found {
        let common_src = match common {
            Some(c) => {
                println!("cargo::rerun-if-changed={}", c.display());
                format!("include_str!({c:?})")
            }
            None => "\"\"".to_string(),
        };
        println!("cargo::rerun-if-changed={}", app.display());
        let _ = writeln!(out, "    ({code:?}, {common_src}, include_str!({app:?})),");
    }
    out.push_str("];\n");
    Ok(out)
}

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=BOARDCRAFT_BUILD_SHA");
    println!("cargo::rerun-if-env-changed=BOARDCRAFT_BUILD_DATE");
    let out_dir = std::env::var_os("OUT_DIR").map(PathBuf::from).unwrap_or_default();
    match i18n() {
        Ok(src) => {
            if let Err(e) = std::fs::write(Path::new(&out_dir).join("craft_i18n.rs"), src) {
                println!("cargo::error=craft_i18n.rs: {e}");
            }
        }
        Err(e) => println!("cargo::error={e}"),
    }
    if let Err(e) = craft_fonts_build::Embed::new().write("craft_fonts.rs") {
        println!("cargo::error={e}");
    }
}
