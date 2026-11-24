//! `tooncraft-cli brush`: work with `.toonbrush` files from the command line.
//!
//! ```text
//! tooncraft-cli brush make <kind> <file.toonbrush> [--name NAME]   write a built-in brush
//! tooncraft-cli brush info <file.toonbrush>                        every setting as JSON
//! tooncraft-cli brush preview <file.toonbrush> <out.png> [--size WxH]
//! tooncraft-cli brush import-abr <file.abr> <dir>                  every brush of a Photoshop file
//! ```

use std::path::Path;

use tooncraft_brush::{Brush, BrushKind, Images, Source, Target};
use tooncraft_engine::preset::Preset;

pub fn run(positional: &[String], name: Option<&str>, size: (u32, u32)) -> Result<(), String> {
    let usage = || "usage: tooncraft-cli brush (make <kind> <file> | info <file> | preview <file> <out.png> | import-abr <file.abr> <dir>)".to_string();
    match positional.first().map(String::as_str) {
        Some("import-abr") => {
            let (file, dir) = (Path::new(positional.get(1).ok_or_else(usage)?), Path::new(positional.get(2).ok_or_else(usage)?));
            if std::fs::metadata(file).map_err(|e| e.to_string())?.len() > 512 << 20 {
                return Err("the brush file is too large".into());
            }
            let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "brush".into());
            let got = tooncraft_interop::abr::read(&std::fs::read(file).map_err(|e| e.to_string())?, &stem)?;
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            let mut used = std::collections::HashSet::new();
            for b in &got.brushes {
                // a file name from the brush name: letters, digits, spaces and dashes, unique
                let base: String = b.name.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect();
                let base = if base.trim().is_empty() { "brush".to_string() } else { base.trim().to_string() };
                let mut name = base.clone();
                let mut n = 2;
                while !used.insert(name.to_lowercase()) {
                    name = format!("{base} {n}");
                    n += 1;
                }
                let out = dir.join(format!("{name}.{}", tooncraft_engine::preset::EXTENSION));
                Preset::new(&b.name, &b.brush, b.tip.as_ref(), None).save(&out)?;
                println!("wrote {}", out.display());
            }
            for w in &got.warnings {
                eprintln!("warning: {w}");
            }
            Ok(())
        }
        Some("make") => {
            let kind = positional
                .get(1)
                .and_then(|k| BrushKind::from_id(k))
                .ok_or_else(|| format!("brush make needs a kind: {}", BrushKind::ALL.iter().map(|k| k.id()).collect::<Vec<_>>().join(", ")))?;
            let file = positional.get(2).ok_or_else(usage)?;
            let name = name.map(str::to_string).unwrap_or_else(|| kind.id().to_string());
            Preset::new(&name, &Brush::preset(kind), None, None).save(Path::new(file))?;
            println!("wrote {file}");
            Ok(())
        }
        Some("info") => {
            let p = Preset::load(Path::new(positional.get(1).ok_or_else(usage)?))?;
            let mut mappings = Vec::new();
            for t in Target::ALL {
                for s in Source::ALL {
                    let m = p.brush.dynamics.response.get(t, s);
                    if m.amount != 0.0 {
                        mappings.push(serde_json::json!({"target": t.id(), "source": s.id(), "amount": m.amount, "curve": m.curve.points()}));
                    }
                }
            }
            let out = serde_json::json!({
                "name": p.name,
                "version": tooncraft_engine::preset::VERSION,
                "tip": p.tip.as_ref().map(|t| serde_json::json!({"name": t.name, "size": [t.width, t.height]})),
                "grain_image": p.grain.as_ref().map(|t| serde_json::json!({"name": t.name, "size": [t.width, t.height]})),
                "mappings": mappings,
                "brush": p.brush,
            });
            println!("{}", serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?);
            Ok(())
        }
        Some("preview") => {
            let p = Preset::load(Path::new(positional.get(1).ok_or_else(usage)?))?;
            let out = positional.get(2).ok_or_else(usage)?;
            let images = Images { tip: p.tip.clone().map(std::sync::Arc::new), grain: p.grain.clone().map(std::sync::Arc::new) };
            // the stroke is drawn in the brush's own colour on paper
            let img = tooncraft_engine::preview::render(p.brush, images, size.0, size.1).ok_or("the preview size must be between 8 and 4096 px")?;
            let mut sheet = tooncraft_raster::RgbaImage::filled(size.0, size.1, [246, 243, 236, 255]);
            sheet.over_image(&img);
            let png = tooncraft_format::encode_png(&sheet).map_err(|e| e.to_string())?;
            std::fs::write(out, png).map_err(|e| format!("{out}: {e}"))?;
            println!("wrote {out}");
            Ok(())
        }
        _ => Err(usage()),
    }
}
