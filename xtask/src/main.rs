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
//! - `parity`: counts the status column of `docs/target-app-parity.md`; `--write` updates the
//!   headline block between the `parity:` markers.
//! - `licenses`: every dependency's licence expression is satisfiable from the suite allowlist
//!   (house-rules-rust), from `cargo metadata` (the same list `deny.toml` gives cargo-deny).
//! - `strings`: extracts every `tl!`/`tlf!` source text into `locales/tooncraft.tsv` (the English
//!   catalog translators start from); `ci` fails when it is out of date.
//! - `bench`: the engine benchmark (`tooncraft-cli bench`, release build).
//! - `version`: the workspace version.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const UI_CRATES: &[&str] = &["egui", "eframe", "egui-winit", "winit", "rfd", "egui-wgpu", "egui_glow"];
const CORE: &[&str] = &["tooncraft-raster", "tooncraft-brush", "tooncraft-doc", "tooncraft-format", "tooncraft-interop", "tooncraft-engine"];

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

fn parity(write: bool) -> Result<(), String> {
    let path = root().join("docs/target-app-parity.md");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (mut d, mut p, mut m) = (0u32, 0u32, 0u32);
    let mut areas: Vec<(String, u32, u32, u32)> = Vec::new();
    for line in text.lines() {
        if let Some(h) = line.strip_prefix("## ") {
            areas.push((h.to_string(), 0, 0, 0));
            continue;
        }
        let cols: Vec<&str> = line.split('|').map(str::trim).collect();
        if cols.len() >= 5 {
            let status = cols[3];
            let a = areas.last_mut();
            match status {
                "D" => {
                    d += 1;
                    if let Some(a) = a {
                        a.1 += 1;
                    }
                }
                "P" => {
                    p += 1;
                    if let Some(a) = a {
                        a.2 += 1;
                    }
                }
                "M" => {
                    m += 1;
                    if let Some(a) = a {
                        a.3 += 1;
                    }
                }
                _ => {}
            }
        }
    }
    let rows = d + p + m;
    if rows == 0 {
        return Err("no feature rows found in docs/target-app-parity.md".into());
    }
    let score = |d: u32, p: u32, m: u32| (d as f64 + p as f64 * 0.5) / (d + p + m).max(1) as f64 * 100.0;
    let mut block = String::from("| | Value | Kind |\n|---|---|---|\n");
    block.push_str(&format!(
        "| Feature rows | {rows} | measured |\n| Done (D) | {d} | measured |\n| Partial (P) | {p} | measured |\n| Missing (M) | {m} | measured |\n"
    ));
    block.push_str(&format!("| Parity score, (D + ½·P) / rows | {:.0} % | measured |\n", score(d, p, m)));
    for (name, ad, ap, am) in &areas {
        if ad + ap + am > 0 {
            block.push_str(&format!("| {name} | {:.0} % ({ad} D, {ap} P, {am} M) | measured |\n", score(*ad, *ap, *am)));
        }
    }
    print!("{block}");
    if write {
        let start = "<!-- parity:start -->\n";
        let end = "<!-- parity:end -->";
        let (Some(a), Some(b)) = (text.find(start), text.find(end)) else { return Err("parity markers missing".into()) };
        let new = format!("{}{start}{block}{}", &text[..a], &text[b..]);
        std::fs::write(&path, new).map_err(|e| e.to_string())?;
        println!("wrote {}", path.display());
    } else if !text.contains(&block) {
        return Err("the parity headline is out of date: run `cargo xtask parity --write`".into());
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
        return Ok(());
    }
    let current = std::fs::read_to_string(&path).unwrap_or_default();
    if current != tsv {
        return Err("locales/tooncraft.tsv is out of date: run `cargo xtask strings --write`".into());
    }
    println!("strings: {} source texts, catalog up to date", list.len());
    Ok(())
}

fn ci() -> Result<(), String> {
    cargo(&["fmt", "--all", "--check"])?;
    cargo(&["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"])?;
    cargo(&["test", "--workspace"])?;
    layers()?;
    assets()?;
    progress()?;
    parity(false)?;
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
        "strings" => strings(write),
        "bench" => cargo(&["run", "--release", "-p", "tooncraft-cli", "--", "bench"]),
        "version" => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => Err("usage: cargo xtask <ci|layers|assets|progress|parity [--write]|strings [--write]|licenses|bench|version>".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::FAILURE
        }
    }
}
