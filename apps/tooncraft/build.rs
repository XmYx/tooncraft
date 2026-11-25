//! Optional build inputs, never Cargo dependencies:
//!
//! - `CRAFT_I18N_DIR`: a craft-i18n checkout. Embeds `locales/<locale>/common.tsv` and
//!   `locales/<locale>/tooncraft.tsv` for every locale that has ToonCraft's own catalog, as
//!   `CRAFT_I18N` in `OUT_DIR/craft_i18n.rs` (recipe: craft-i18n `docs/integration.md`). Unset: the
//!   table is empty and the app is English-only. `CRAFT_I18N_REQUIRED=1` (releases) turns a missing
//!   or invalid checkout into an error.
//! - `CRAFT_FONTS_DIR`: a craft-fonts checkout, embedded through `craft-fonts-build` as
//!   `OUT_DIR/craft_fonts.rs` (`CRAFT_FONTS_REQUIRED=1` for releases). Unset: egui's fonts only.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const APP: &str = "tooncraft";

fn i18n() -> Result<String, String> {
    println!("cargo::rerun-if-env-changed=CRAFT_I18N_DIR");
    println!("cargo::rerun-if-env-changed=CRAFT_I18N_REQUIRED");
    let required = std::env::var("CRAFT_I18N_REQUIRED").is_ok_and(|v| v == "1");
    let dir = std::env::var_os("CRAFT_I18N_DIR").filter(|d| !d.is_empty()).map(PathBuf::from);
    let mut out =
        String::from("/// (locale, shared catalog, ToonCraft catalog); \"\" for a missing file.\npub static CRAFT_I18N: &craft_i18n::Embedded = &[\n");
    let locales = dir.as_ref().map(|d| d.join("locales"));
    match locales.as_deref().filter(|l| l.is_dir()) {
        Some(locales) => {
            println!("cargo::rerun-if-changed={}", locales.display());
            let mut entries: Vec<PathBuf> = std::fs::read_dir(locales).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
            entries.sort();
            for loc in entries {
                let app = loc.join(format!("{APP}.tsv"));
                if !app.is_file() {
                    continue; // only locales with ToonCraft's own catalog are offered
                }
                let Some(code) = loc.file_name().and_then(|n| n.to_str()) else { continue };
                let common = loc.join("common.tsv");
                let common_src = if common.is_file() {
                    println!("cargo::rerun-if-changed={}", common.display());
                    format!("include_str!({:?})", common)
                } else {
                    "\"\"".to_string()
                };
                println!("cargo::rerun-if-changed={}", app.display());
                let _ = writeln!(out, "    ({code:?}, {common_src}, include_str!({:?})),", app);
            }
        }
        None => {
            if let Some(d) = &dir {
                let msg = format!("CRAFT_I18N_DIR={} is not a craft-i18n checkout (no locales/)", d.display());
                if required {
                    return Err(msg);
                }
                println!("cargo::warning={msg}; building English-only");
            } else if required {
                return Err("CRAFT_I18N_REQUIRED=1 but CRAFT_I18N_DIR is not set".into());
            }
        }
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
    windows_resources();
}

/// Embeds the app icon and VERSIONINFO in tooncraft.exe when targeting Windows; elsewhere it does
/// nothing. A missing resource compiler is a warning, so a cross-compile still links, unless
/// `TOONCRAFT_REQUIRE_WINRES=1` (set by `packaging/windows/package.ps1`) makes it an error.
fn windows_resources() {
    println!("cargo::rerun-if-changed=../../assets/app-icon/tooncraft.ico");
    println!("cargo::rerun-if-env-changed=TOONCRAFT_REQUIRE_WINRES");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let icon = manifest.join("../../assets/app-icon/tooncraft.ico");
    let mut res = winresource::WindowsResource::new();
    res.set_icon(&icon.to_string_lossy())
        .set("ProductName", "ToonCraft")
        .set("FileDescription", "ToonCraft frame-by-frame animation")
        .set("LegalCopyright", "Copyright (c) the ToonCraft authors. MIT OR Apache-2.0.")
        .set("OriginalFilename", "tooncraft.exe")
        .set("InternalName", "tooncraft");
    if let Err(e) = res.compile() {
        if std::env::var("TOONCRAFT_REQUIRE_WINRES").is_ok_and(|v| v == "1") {
            println!("cargo::error=embedding Windows resources failed: {e}");
        } else {
            println!("cargo::warning=tooncraft.exe built without icon/version resources: {e}");
        }
    }
}
