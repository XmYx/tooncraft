//! Project tasks: `cargo xtask <command>`.
//!
//! - `ci`: everything a pull request must pass: `fmt --check`, clippy with `-D warnings`, the
//!   tests, the layering check, the asset check, the progress fragments and the parity table.
//! - `layers`: the core crates (`tooncraft-raster`, `-brush`, `-doc`, `-format`, `-engine`) must
//!   not depend on any UI crate (egui, eframe, winit, rfd).
//! - `assets`: every file under `assets/`, `docs/images/` and `docs/brand/` is listed in
//!   `ATTRIBUTION.md`.
//! - `progress`: RFC-0017 fragments in `docs/progress.d/` are well formed (one non-empty entry
//!   per file, no duplicates); prints them joined, newest first.
//! - `parity`: scores `docs/target-app-parity.md` (row parity, effort parity per area and the
//!   headline weighted parity, see `parity.rs`); `--write` updates the block between the `parity:`
//!   markers. `ci` validates the rows strictly but only warns when that generated block is stale
//!   (feature PRs flip rows; a maintainer refreshes the block).
//! - `licenses`: every dependency's licence expression is satisfiable from the suite allowlist
//!   (house-rules-rust), from `cargo metadata` (the same list `deny.toml` gives cargo-deny).
//! - `strings`: extracts every `tl!`/`tlf!` source text into `locales/tooncraft.tsv` (the English
//!   catalog translators start from); `ci` fails when it is out of date. Also checks the translated
//!   catalogs `locales/<locale>/tooncraft.tsv` (kept here or in a `CRAFT_I18N_DIR` checkout) against the craft-i18n catalog format and reports how
//!   much of the source each covers.
//! - `icons`: render `assets/app-icon/tooncraft.svg` to the hicolor PNGs, the 1024 px PNG, the
//!   Windows `.ico` and the macOS `.icns` (committed, so packaging needs no tools).
//! - `bench`: the engine benchmark (`tooncraft-cli bench`, release build).
//! - `version`: the workspace version.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

mod parity;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const UI_CRATES: &[&str] = &["egui", "eframe", "egui-winit", "winit", "rfd", "egui-wgpu", "egui_glow"];
const CORE: &[&str] =
    &["tooncraft-raster", "tooncraft-fx", "tooncraft-lipsync", "tooncraft-brush", "tooncraft-doc", "tooncraft-format", "tooncraft-interop", "tooncraft-engine"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
}

fn cargo_bin() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".into())
}

fn cargo(args: &[&str]) -> Result<(), String> {
    println!("$ cargo {}", args.join(" "));
    let status = Command::new(cargo_bin()).args(args).current_dir(root()).status().map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(format!("cargo {} failed", args.join(" "))) }
}

fn layers() -> Result<(), String> {
    for krate in CORE {
        let out = Command::new(cargo_bin())
            .args(["tree", "-p", krate, "-e", "normal", "--prefix", "none", "--format", "{p}"])
            .current_dir(root())
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).into_owned());
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let bad: Vec<&str> = text.lines().filter(|l| UI_CRATES.iter().any(|c| l.split_whitespace().next() == Some(c))).collect();
        if !bad.is_empty() {
            return Err(format!("{krate} depends on UI crates: {}", bad.join(", ")));
        }
    }
    println!("layers: the core crates have no UI dependency");
    Ok(())
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn assets() -> Result<(), String> {
    let root = root();
    let index = std::fs::read_to_string(root.join("ATTRIBUTION.md")).map_err(|e| format!("ATTRIBUTION.md: {e}"))?;
    let mut files = Vec::new();
    for d in ["assets", "docs/images", "docs/brand"] {
        walk(&root.join(d), &mut files);
    }
    let missing: Vec<String> = files
        .iter()
        .filter_map(|f| f.strip_prefix(&root).ok())
        .map(|f| f.to_string_lossy().replace('\\', "/"))
        .filter(|f| !f.ends_with("README.md") && !f.ends_with("LICENSE.txt"))
        .filter(|f| !index.contains(&format!("`{f}`")))
        .collect();
    if missing.is_empty() {
        println!("assets: {} files, all attributed", files.len());
        Ok(())
    } else {
        Err(format!("not listed in ATTRIBUTION.md:\n  {}", missing.join("\n  ")))
    }
}

fn progress() -> Result<(), String> {
    let dir = root().join("docs/progress.d");
    let mut files = Vec::new();
    walk(&dir, &mut files);
    files.sort();
    let mut seen = std::collections::BTreeSet::new();
    let mut entries = Vec::new();
    for f in &files {
        let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if name == "README.md" {
            continue;
        }
        if !name.ends_with(".md") || !name.contains('-') {
            return Err(format!("progress fragment {name}: name it <issue>-<branch-slug>.md"));
        }
        let text = std::fs::read_to_string(f).map_err(|e| format!("{name}: {e}"))?;
        let entry = text.trim();
        if entry.is_empty() {
            return Err(format!("progress fragment {name} is empty"));
        }
        if entry.lines().count() != 1 || !(entry.starts_with("- ") || entry.starts_with('|')) {
            return Err(format!("progress fragment {name}: one bullet or table row, exactly as it reads in the log"));
        }
        if !seen.insert(entry.to_string()) {
            return Err(format!("progress fragment {name} duplicates another"));
        }
        entries.push(entry.to_string());
    }
    entries.reverse();
    println!("progress: {} fragments", entries.len());
    for e in entries {
        println!("{e}");
    }
    Ok(())
}

const PARITY_START: &str = "<!-- parity:start -->\n";
const PARITY_END: &str = "<!-- parity:end -->";

/// The byte range of the generated block inside the map, between the `parity:` markers.
fn parity_block_range(text: &str) -> Result<(usize, usize), String> {
    let (Some(a), Some(b)) = (text.find(PARITY_START), text.find(PARITY_END)) else { return Err("parity markers missing".into()) };
    if a + PARITY_START.len() > b {
        return Err("parity markers out of order".into());
    }
    Ok((a + PARITY_START.len(), b))
}

fn parity(write: bool) -> Result<(), String> {
    let path = root().join("docs/target-app-parity.md");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let block = parity::score(&text)?.block();
    print!("{block}");
    let (a, b) = parity_block_range(&text)?;
    if write {
        let new = format!("{}{block}{}", &text[..a], &text[b..]);
        std::fs::write(&path, new).map_err(|e| e.to_string())?;
        println!("wrote {}", path.display());
    } else if text[a..b] != block {
        return Err("the parity headline is out of date: run `cargo xtask parity --write`".into());
    }
    Ok(())
}

/// The parity check `ci` runs. The rows are validated strictly (a malformed row, a bad status or
/// missing markers is an error), but a stale generated block only warns: rows are flipped in many
/// concurrent PRs, so committing the block there would make them all conflict. A maintainer
/// refreshes it separately with `cargo xtask parity --write`.
fn parity_ci_check(text: &str) -> Result<Option<String>, String> {
    let block = parity::score(text)?.block();
    let (a, b) = parity_block_range(text)?;
    if text[a..b] == block {
        return Ok(None);
    }
    Ok(Some(
        "the generated parity headline is out of date (not a failure; a maintainer refreshes it with `cargo xtask parity --write` in a separate PR)".into(),
    ))
}

fn parity_ci() -> Result<(), String> {
    let path = root().join("docs/target-app-parity.md");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if let Some(warning) = parity_ci_check(&text)? {
        eprintln!("xtask: warning: {warning}");
    }
    Ok(())
}

const ALLOWED: &[&str] = &[
    "ISC",
    "MIT",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "Apache-2.0",
    "Unicode-3.0",
    "OFL-1.1",
    "Ubuntu-font-1.0",
    "Bitstream-Vera",
    "CC0-1.0",
    "CC-BY-4.0",
    "CC-BY-SA-4.0",
    "Zlib",
    "BSL-1.0",
    "0BSD",
    "Unlicense",
    "IJG",
    "NCSA",
    "CDLA-Permissive-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "LicenseRef-Public-Domain",
    "MPL-2.0",
];

/// Is an SPDX expression satisfiable from the allowlist? `OR` needs one side, `AND` both.
fn spdx_ok(expr: &str) -> bool {
    let e = expr.replace(['(', ')'], " ").replace('/', " OR ");
    e.split(" OR ").any(|alt| {
        let alt = alt.trim();
        if ALLOWED.contains(&alt) {
            return true;
        }
        alt.split(" AND ").all(|part| {
            let part = part.trim();
            ALLOWED.contains(&part) || part.split(" WITH ").next().is_some_and(|base| ALLOWED.contains(&base.trim()) && part.contains("LLVM-exception"))
        })
    })
}

fn licenses() -> Result<(), String> {
    let out = Command::new(cargo_bin()).args(["metadata", "--format-version", "1", "--locked"]).current_dir(root()).output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    // Minimal scan of the metadata JSON: each package object has "name", "version", "license".
    let mut bad = Vec::new();
    let mut count = 0;
    for chunk in text.split("{\"name\":\"").skip(1) {
        let name = chunk.split('"').next().unwrap_or_default();
        let Some(lic_at) = chunk.find("\"license\":") else { continue };
        let rest = &chunk[lic_at + 10..];
        if !rest.starts_with('"') {
            continue; // `null`: workspace crates inherit ours, checked below
        }
        let lic = rest[1..].split('"').next().unwrap_or_default();
        count += 1;
        if !spdx_ok(lic) {
            bad.push(format!("{name}: {lic}"));
        }
    }
    bad.sort();
    bad.dedup();
    if bad.is_empty() {
        println!("licenses: {count} packages, all on the allowlist");
        Ok(())
    } else {
        Err(format!("licences outside the allowlist (house-rules-rust):\n  {}", bad.join("\n  ")))
    }
}

/// Every `tl!("…")` / `tlf!("…"` source text in the app, sorted and unique.
fn source_strings() -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    walk(&root().join("apps/tooncraft/src"), &mut files);
    let mut out = std::collections::BTreeSet::new();
    for f in files.iter().filter(|f| f.extension().is_some_and(|e| e == "rs")) {
        let text = std::fs::read_to_string(f).map_err(|e| e.to_string())?;
        for marker in ["tl!(\"", "tlf!(\""] {
            for piece in text.split(marker).skip(1) {
                let mut s = String::new();
                let mut esc = false;
                for ch in piece.chars() {
                    if esc {
                        s.push(ch);
                        esc = false;
                    } else if ch == '\\' {
                        esc = true;
                    } else if ch == '"' {
                        break;
                    } else {
                        s.push(ch);
                    }
                }
                if !s.is_empty() && !s.contains("{$") {
                    out.insert(s);
                }
            }
        }
    }
    Ok(out.into_iter().collect())
}

fn strings(write: bool) -> Result<(), String> {
    let list = source_strings()?;
    let mut tsv = String::from("# ToonCraft source catalog: context<TAB>English<TAB>translation. Generated by `cargo xtask strings --write`.\n");
    for s in &list {
        tsv.push_str(&format!("tooncraft\t{s}\t\n"));
    }
    let path = root().join("locales/tooncraft.tsv");
    if write {
        std::fs::write(&path, &tsv).map_err(|e| e.to_string())?;
        println!("strings: wrote {} source texts to {}", list.len(), path.display());
        return catalogs(&list);
    }
    let current = std::fs::read_to_string(&path).unwrap_or_default();
    if current != tsv {
        return Err("locales/tooncraft.tsv is out of date: run `cargo xtask strings --write`".into());
    }
    println!("strings: {} source texts, catalog up to date", list.len());
    catalogs(&list)
}

/// The `{name}` placeholders of a catalog text, sorted.
fn placeholders(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(a) = rest.find('{') {
        let Some(b) = rest[a..].find('}') else { break };
        out.push(rest[a + 1..a + b].to_string());
        rest = &rest[a + b + 1..];
    }
    out.sort();
    out
}

/// Problems of one translated catalog and its row counts `(rows, stale)`.
fn check_catalog(code: &str, text: &str, known: &std::collections::BTreeSet<&str>) -> (usize, usize, Vec<String>) {
    let mut problems = Vec::new();
    let at = |n: usize| format!("locales/{code}/tooncraft.tsv:{}", n + 1);
    if !text.lines().any(|l| l.starts_with("# Clean room:")) {
        problems.push(format!("locales/{code}/tooncraft.tsv: the header needs the \"# Clean room:\" line"));
    }
    let (mut rows, mut stale, mut named) = (0usize, 0usize, false);
    let mut seen = std::collections::BTreeSet::new();
    for (n, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        let [ctx, en, tr] = cols.as_slice() else {
            problems.push(format!("{}: a row has three tab-separated columns", at(n)));
            continue;
        };
        if tr.is_empty() {
            problems.push(format!("{}: empty translation (leave the row out instead)", at(n)));
            continue;
        }
        if !seen.insert((ctx.to_string(), en.to_string())) {
            problems.push(format!("{}: duplicate row", at(n)));
        }
        for col in [en, tr] {
            let mut it = col.chars();
            while let Some(c) = it.next() {
                if c == '\\' && !matches!(it.next(), Some('n' | 't' | '\\')) {
                    problems.push(format!("{}: only \\n, \\t and \\\\ are escapes", at(n)));
                    break;
                }
            }
        }
        if *ctx == "@id" {
            named |= *en == "language.name";
            continue;
        }
        rows += 1;
        if !ctx.starts_with('@') && !known.contains(en) {
            stale += 1;
        }
        if placeholders(en) != placeholders(tr) {
            problems.push(format!("{}: the {{placeholders}} differ between the English and the translation", at(n)));
        }
        if en.ends_with('…') != tr.ends_with('…') {
            problems.push(format!("{}: a trailing … must be in both columns or neither", at(n)));
        }
    }
    if !named {
        problems.push(format!("locales/{code}/tooncraft.tsv: needs the row  @id<TAB>language.name<TAB>the language's own name"));
    }
    (rows, stale, problems)
}

/// Check the translated catalogs `locales/<locale>/tooncraft.tsv` against the rules of the
/// craft-i18n catalog format (rows of three columns, valid escapes, no empty translations, the
/// same placeholders and trailing ellipsis in both columns, no duplicate rows, the language name,
/// the clean-room header) and report how much of the source catalog each one covers. Rows whose
/// English text is no longer in the app are reported, never an error: translations may lag the app.
fn catalogs(source: &[String]) -> Result<(), String> {
    // Translated catalogs live in storytold/craft-i18n; with `CRAFT_I18N_DIR` set to a checkout its
    // ToonCraft catalogs are read too. The rules are this repository's reading of craft-i18n's
    // `docs/catalog-format.md` (rows, escapes, placeholders, ellipsis, duplicates, the language-name
    // row and the `# Clean room:` header line), so a finding in another repository's file is only
    // a warning here, while one in a catalog kept in this repository fails the build.
    let mut roots = vec![(root().join("locales"), false)];
    if let Some(d) = std::env::var_os("CRAFT_I18N_DIR").filter(|d| !d.is_empty()) {
        roots.push((PathBuf::from(d).join("locales"), true));
    }
    let mut dirs: Vec<(PathBuf, bool)> = Vec::new();
    for (r, external) in roots {
        let Ok(entries) = std::fs::read_dir(&r) else { continue };
        dirs.extend(entries.flatten().map(|e| e.path()).filter(|p| p.is_dir() && p.join("tooncraft.tsv").is_file()).map(|p| (p, external)));
    }
    dirs.sort();
    let known: std::collections::BTreeSet<&str> = source.iter().map(String::as_str).collect();
    let mut problems = Vec::new();
    for (dir, external) in dirs {
        let code = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if code != code.to_lowercase() {
            problems.push(format!("locales/{code}: locale directories are lowercase"));
        }
        let Ok(text) = std::fs::read_to_string(dir.join("tooncraft.tsv")) else { continue };
        let (rows, stale, found) = check_catalog(&code, &text, &known);
        if external {
            for f in found {
                println!("strings: warning (craft-i18n checkout): {f}");
            }
        } else {
            problems.extend(found);
        }
        println!(
            "strings: {code}: {rows} of {} source texts translated{}",
            source.len(),
            if stale > 0 { format!(", {stale} rows for texts the app no longer has") } else { String::new() }
        );
    }
    if problems.is_empty() { Ok(()) } else { Err(problems.join("\n")) }
}

const ICON_ID: &str = "ai.storyteller.tooncraft";
const HICOLOR: [u32; 8] = [16, 24, 32, 48, 64, 128, 256, 512];

/// The icon SVG rendered at `size` px, straight RGBA.
fn render_icon(svg: &[u8], size: u32) -> Result<Vec<u8>, String> {
    use craft_svg::tree::{self, usvg};
    let tree = tree::parse(svg, &tree::Limits::default(), &usvg::Options::default()).map_err(|e| format!("icon SVG: {e:?}"))?;
    let scale = size as f32 / tree.size().width().max(1.0);
    let pixmap = tree::draw::render(&tree, usvg::Transform::from_scale(scale, scale), size, size, 0, 1 << 30).map_err(|e| format!("icon render: {e:?}"))?;
    let mut rgba = pixmap.data().to_vec();
    // tiny-skia pixmaps are premultiplied
    for px in rgba.as_chunks_mut::<4>().0.iter_mut() {
        let a = px[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut px[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    Ok(rgba)
}

fn encode_png(rgba: Vec<u8>, size: u32) -> Result<Vec<u8>, String> {
    use craft_image_codecs::{ChannelLayout, EncodeOptions, Format, Image, SampleType};
    let img = Image::from_raw(size, size, ChannelLayout::Rgba, SampleType::U8, rgba).map_err(|e| e.to_string())?;
    craft_image_codecs::encode(&img, Format::Png, &EncodeOptions::default()).map_err(|e| e.to_string())
}

/// An ICO whose images are PNGs (read by Windows Vista and later).
fn ico_file(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = vec![0, 0, 1, 0];
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (size, data) in images {
        let side = if *size >= 256 { 0 } else { *size as u8 };
        out.extend_from_slice(&[side, side, 0, 0, 1, 0, 32, 0]);
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += data.len() as u32;
    }
    for (_, data) in images {
        out.extend_from_slice(data);
    }
    out
}

/// An ICNS whose images are PNGs (macOS 10.7 and later).
fn icns_file(images: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
    let total = 8 + images.iter().map(|(_, d)| 8 + d.len()).sum::<usize>();
    let mut out = b"icns".to_vec();
    out.extend_from_slice(&(total as u32).to_be_bytes());
    for (kind, data) in images {
        out.extend_from_slice(kind);
        out.extend_from_slice(&((8 + data.len()) as u32).to_be_bytes());
        out.extend_from_slice(data);
    }
    out
}

fn icons() -> Result<(), String> {
    let dir = root().join("assets/app-icon");
    let svg = std::fs::read(dir.join("tooncraft.svg")).map_err(|e| e.to_string())?;
    let mut png = std::collections::BTreeMap::new();
    for size in HICOLOR.into_iter().chain([1024]) {
        png.insert(size, encode_png(render_icon(&svg, size)?, size)?);
    }
    let get = |s: u32| png.get(&s).cloned().ok_or_else(|| format!("no {s} px icon"));
    let write = |p: PathBuf, b: Vec<u8>| -> Result<(), String> {
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::write(&p, b).map_err(|e| format!("{}: {e}", p.display()))
    };
    for size in HICOLOR {
        write(dir.join(format!("hicolor/{size}x{size}/apps/{ICON_ID}.png")), get(size)?)?;
    }
    write(dir.join(format!("hicolor/scalable/apps/{ICON_ID}.svg")), svg.clone())?;
    write(dir.join("tooncraft-1024.png"), get(1024)?)?;
    let ico: Vec<(u32, Vec<u8>)> = [16, 24, 32, 48, 64, 128, 256].into_iter().map(|s| get(s).map(|d| (s, d))).collect::<Result<_, _>>()?;
    write(dir.join("tooncraft.ico"), ico_file(&ico))?;
    let icns = vec![
        (*b"icp4", get(16)?),
        (*b"icp5", get(32)?),
        (*b"icp6", get(64)?),
        (*b"ic07", get(128)?),
        (*b"ic08", get(256)?),
        (*b"ic09", get(512)?),
        (*b"ic10", get(1024)?),
    ];
    write(dir.join("tooncraft.icns"), icns_file(&icns))?;
    println!("icons: {} hicolor sizes, 1024 px, .ico and .icns in {}", HICOLOR.len(), dir.display());
    Ok(())
}

fn ci() -> Result<(), String> {
    cargo(&["fmt", "--all", "--check"])?;
    cargo(&["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"])?;
    cargo(&["test", "--workspace"])?;
    layers()?;
    assets()?;
    progress()?;
    parity_ci()?;
    strings(false)?;
    licenses()
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_default();
    let write = args.any(|a| a == "--write");
    let result = match cmd.as_str() {
        "ci" => ci(),
        "layers" => layers(),
        "assets" => assets(),
        "progress" => progress(),
        "parity" => parity(write),
        "licenses" => licenses(),
        "icons" => icons(),
        "strings" => strings(write),
        "bench" => cargo(&["run", "--release", "-p", "tooncraft-cli", "--", "bench"]),
        "version" => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => Err("usage: cargo xtask <ci|layers|assets|progress|parity [--write]|strings [--write]|licenses|icons|bench|version>".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parity_ci_check;

    const ROWS: &str = "## One\n\nWeight: 2\n\n| Feature | Refs | Status | Size | Notes |\n|---|---|---|---|---|\n| a | all | D | S | |\n| b | TV | P | M | |\n| c | TV | M | L | |\n";

    fn doc(block: &str, rows: &str) -> String {
        format!("# Map\n\n<!-- parity:start -->\n{block}<!-- parity:end -->\n\n{rows}")
    }

    #[test]
    fn fresh_block_is_silent() {
        let block = crate::parity::score(ROWS).map(|r| r.block()).unwrap_or_default();
        assert!(!block.is_empty());
        assert_eq!(parity_ci_check(&doc(&block, ROWS)), Ok(None));
    }

    #[test]
    fn stale_block_warns_but_does_not_fail() {
        let stale = doc("| | Value | Kind |\n|---|---|---|\n| In-scope feature rows | 1 | counted |\n", ROWS);
        let checked = parity_ci_check(&stale);
        assert!(matches!(&checked, Ok(Some(w)) if w.contains("out of date")), "{checked:?}");
    }

    #[test]
    fn malformed_rows_and_markers_still_fail() {
        let block = "x\n";
        let bad_status = ROWS.replace("| b | TV | P |", "| b | TV | Q |");
        assert!(parity_ci_check(&doc(block, &bad_status)).is_err(), "bad status");
        let bad_size = ROWS.replace("| b | TV | P | M |", "| b | TV | P | big |");
        assert!(parity_ci_check(&doc(block, &bad_size)).is_err(), "bad size");
        let short_row = ROWS.replace("| b | TV | P | M | |", "| b | TV |");
        assert!(parity_ci_check(&doc(block, &short_row)).is_err(), "row with missing cells");
        let no_weight = ROWS.replace("Weight: 2\n", "");
        assert!(parity_ci_check(&doc(block, &no_weight)).is_err(), "area without a weight");
        assert!(parity_ci_check(&doc(block, "## One\n\nno table here\n")).is_err(), "no rows");
        assert!(parity_ci_check(ROWS).is_err(), "markers missing");
        let reversed = format!("<!-- parity:end -->\n<!-- parity:start -->\n{ROWS}");
        assert!(parity_ci_check(&reversed).is_err(), "markers out of order");
    }
}

#[cfg(test)]
mod catalog_tests {
    use super::*;

    const HEAD: &str = "# Clean room: test\n@id\tlanguage.name\tDeutsch\n";

    fn check(rows: &str) -> (usize, usize, Vec<String>) {
        let known: std::collections::BTreeSet<&str> = ["Save", "Open…", "Layer {name}", "Line\\nbreak"].into_iter().collect();
        check_catalog("de", &format!("{HEAD}{rows}"), &known)
    }

    #[test]
    fn a_good_catalog_has_no_problems() {
        let (rows, stale, problems) = check(
            "\tSave\tSpeichern\n\tOpen…\tÖffnen…\n\tLayer {name}\tEbene {name}\n\tLine\\nbreak\tZeile\\numbruch\n\tGone\tWeg\n@plural\t{n} page|{n} pages\t{n} Seite|{n} Seiten\n",
        );
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!((rows, stale), (6, 1), "the row for text the app no longer has is counted, not refused");
    }

    #[test]
    fn every_rule_of_the_format_is_checked() {
        let bad = |rows: &str, needle: &str| {
            let (_, _, p) = check(rows);
            assert!(p.iter().any(|m| m.contains(needle)), "{rows:?}: {p:?}");
        };
        bad("\tSave\t\n", "empty translation");
        bad("\tSave\tSpeichern\textra\n", "three tab-separated");
        bad("\tSave\tA\n\tSave\tB\n", "duplicate");
        bad("\tLayer {name}\tEbene {title}\n", "placeholders");
        bad("\tLayer {name}\tEbene\n", "placeholders");
        bad("\tOpen…\tÖffnen\n", "trailing");
        bad("\tSave\tSpeichern…\n", "trailing");
        bad("\tSave\tSpeich\\ern\n", "escapes");
        let (_, _, p) = check_catalog("de", "@id\tlanguage.name\tDeutsch\n\tSave\tSpeichern\n", &std::collections::BTreeSet::new());
        assert!(p.iter().any(|m| m.contains("Clean room")));
        let (_, _, p) = check_catalog("de", "# Clean room: x\n\tSave\tSpeichern\n", &std::collections::BTreeSet::new());
        assert!(p.iter().any(|m| m.contains("language.name")));
    }

    #[test]
    fn placeholders_are_found_in_any_order() {
        assert_eq!(placeholders("{b} and {a} of {b}"), ["a", "b", "b"]);
        assert!(placeholders("no braces, a lone { too").is_empty());
    }

    #[test]
    fn the_catalogs_in_reach_pass() {
        let list = source_strings().unwrap();
        catalogs(&list).unwrap();
    }
}
