//! ABR brush files (`.abr`) as ToonCraft brushes: tips and basic settings.
//!
//! The file is read by the shared `craft-psd` reader (legacy v1 / v2 and the sectioned v6+
//! layout). Each brush becomes a [`Brush`] with an image tip where the file has a sampled tip, or
//! the round tip with the file's diameter, hardness, angle and roundness for a computed one;
//! spacing comes across too. Everything else in a preset (dynamics, textures, dual brushes) is left
//! out: ToonCraft's own settings start from the ink pen. Tips larger than [`MAX_TIP_SIDE`] are
//! scaled down by an integer factor (box average).

use craft_psd::abr::{AbrSample, LegacyTip};
use craft_psd::descriptor::{Descriptor, Value};
use tooncraft_brush::{Brush, BrushKind, MAX_SIZE, MAX_TIP_SIDE, Tip};

/// Most brushes taken from one file.
pub const MAX_IMPORTED: usize = 1000;

/// One brush read from a file.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedBrush {
    pub name: String,
    pub brush: Brush,
    /// The image tip, when the brush has one.
    pub tip: Option<Tip>,
}

/// What a file gave: its brushes, and the problems that did not stop the import.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AbrImport {
    pub brushes: Vec<ImportedBrush>,
    pub warnings: Vec<String>,
}

/// Read a `.abr` file's bytes. `stem` names brushes the file leaves unnamed ("stem 3").
pub fn read(bytes: &[u8], stem: &str) -> Result<AbrImport, String> {
    let file = craft_psd::abr::parse(bytes).map_err(|e| format!("not a readable brush file: {e}"))?;
    let mut out = AbrImport { warnings: file.warnings.clone(), ..Default::default() };
    let unnamed = |i: usize| format!("{stem} {}", i + 1);
    for (i, b) in file.legacy.iter().enumerate() {
        let name = if b.name.trim().is_empty() { unnamed(i) } else { clean(&b.name) };
        let spacing = spacing(f64::from(b.spacing));
        let (brush, tip) = match &b.tip {
            LegacyTip::Computed { diameter, hardness, angle, roundness } => {
                (round(f64::from(*diameter), f64::from(*hardness), f64::from(*angle), f64::from(*roundness), spacing), None)
            }
            LegacyTip::Sampled(s) => sampled(&name, s, spacing),
        };
        out.brushes.push(ImportedBrush { name, brush, tip });
    }
    if file.presets.is_empty() {
        // a v6+ file of tips only
        for (i, s) in file.samples.iter().enumerate() {
            let name = unnamed(i);
            let (brush, tip) = sampled(&name, s, spacing(25.0));
            out.brushes.push(ImportedBrush { name, brush, tip });
        }
    }
    for (i, p) in file.presets.iter().enumerate() {
        let name = text(p, "Nm  ").map(|n| clean(&n)).filter(|n| !n.is_empty()).unwrap_or_else(|| unnamed(i));
        let Some(Value::Descriptor(tip)) = p.get("Brsh") else {
            out.warnings.push(format!("{name}: no tip, skipped"));
            continue;
        };
        let space = num(tip, "Spcn").map(spacing).unwrap_or(spacing(25.0));
        let made = match text(tip, "sampledData") {
            Some(id) => match file.samples.iter().find(|s| s.id == id) {
                Some(s) => {
                    let (mut brush, t) = sampled(&name, s, space);
                    if let Some(d) = num(tip, "Dmtr") {
                        brush.size = size(d);
                    }
                    brush.angle = num(tip, "Angl").map(|a| a as f32).unwrap_or(0.0);
                    Some((brush, t))
                }
                None => {
                    out.warnings.push(format!("{name}: its tip image is missing, skipped"));
                    None
                }
            },
            None => Some((
                round(
                    num(tip, "Dmtr").unwrap_or(10.0),
                    num(tip, "Hrdn").unwrap_or(100.0),
                    num(tip, "Angl").unwrap_or(0.0),
                    num(tip, "Rndn").unwrap_or(100.0),
                    space,
                ),
                None,
            )),
        };
        if let Some((brush, tip)) = made {
            out.brushes.push(ImportedBrush { name, brush, tip });
        }
    }
    if out.brushes.len() > MAX_IMPORTED {
        out.warnings.push(format!("only the first {MAX_IMPORTED} of {} brushes were taken", out.brushes.len()));
        out.brushes.truncate(MAX_IMPORTED);
    }
    if out.brushes.is_empty() {
        return Err("the file has no brushes ToonCraft can use".into());
    }
    Ok(out)
}

fn clean(s: &str) -> String {
    s.trim_end_matches('\0').trim().chars().filter(|c| !c.is_control()).take(200).collect()
}

fn text(d: &Descriptor, key: &str) -> Option<String> {
    match d.get(key)? {
        Value::Text(t) => Some(t.to_string_lossy().trim_end_matches('\0').to_string()),
        _ => None,
    }
}

fn num(d: &Descriptor, key: &str) -> Option<f64> {
    d.get(key).and_then(Value::as_number).filter(|v| v.is_finite())
}

fn size(d: f64) -> f32 {
    if d.is_finite() { (d as f32).clamp(0.5, MAX_SIZE) } else { 10.0 }
}

/// Spacing in percent of the diameter as ToonCraft's fraction (0 % means "off": a fine default).
fn spacing(percent: f64) -> f32 {
    if percent.is_finite() && percent > 0.0 { (percent as f32 / 100.0).clamp(0.02, 2.0) } else { 0.1 }
}

fn round(diameter: f64, hardness: f64, angle: f64, roundness: f64, spacing: f32) -> Brush {
    let f = |v: f64, d: f32| if v.is_finite() { v as f32 } else { d };
    Brush {
        size: size(diameter),
        hardness: (f(hardness, 100.0) / 100.0).clamp(0.0, 1.0),
        angle: f(angle, 0.0),
        roundness: (f(roundness, 100.0) / 100.0).clamp(0.1, 1.0),
        spacing,
        ..Brush::preset(BrushKind::Ink)
    }
    .sanitized()
}

/// A sampled tip: its coverage (the file's value is paint amount), scaled down to fit.
fn sampled(name: &str, s: &AbrSample, spacing: f32) -> (Brush, Option<Tip>) {
    let unit = s.to_unit();
    let (w, h) = (s.width as usize, s.height as usize);
    let tip = if w == 0 || h == 0 || unit.len() < w * h {
        None
    } else {
        let k = w.max(h).div_ceil(MAX_TIP_SIDE as usize).max(1);
        let (tw, th) = (w.div_ceil(k), h.div_ceil(k));
        let mut alpha = Vec::with_capacity(tw * th);
        for ty in 0..th {
            for tx in 0..tw {
                let (mut sum, mut n) = (0.0f32, 0.0f32);
                for y in ty * k..((ty + 1) * k).min(h) {
                    for x in tx * k..((tx + 1) * k).min(w) {
                        sum += unit.get(y * w + x).copied().unwrap_or(0.0);
                        n += 1.0;
                    }
                }
                alpha.push((sum / n.max(1.0) * 255.0).round().clamp(0.0, 255.0) as u8);
            }
        }
        Tip::new(name, tw as u32, th as u32, alpha)
    };
    let brush = Brush { size: size(w.max(h) as f64), hardness: 1.0, spacing, ..Brush::preset(BrushKind::Ink) }.sanitized();
    (brush, tip)
}

#[cfg(test)]
mod tests {
    use super::*;
    use craft_psd::abr::{LegacyBrush, write_v6, write_v12};
    use craft_psd::descriptor::UnicodeString;

    fn disc(n: u32) -> AbrSample {
        let c = n as f32 / 2.0;
        let data = (0..n * n).map(|i| if ((i % n) as f32 + 0.5 - c).hypot((i / n) as f32 + 0.5 - c) < c * 0.8 { 255 } else { 0 }).collect();
        AbrSample { id: String::new(), width: n, height: n, depth: 8, data }
    }

    #[test]
    fn legacy_files_give_round_and_sampled_brushes() {
        let brushes = vec![
            LegacyBrush {
                name: String::new(),
                spacing: 30,
                anti_alias: true,
                tip: LegacyTip::Computed { diameter: 40, hardness: 50, angle: 30, roundness: 60 },
            },
            LegacyBrush { name: "Dot\0".into(), spacing: 10, anti_alias: true, tip: LegacyTip::Sampled(disc(24)) },
        ];
        for rle in [false, true] {
            let bytes = write_v12(2, &brushes, rle).unwrap();
            let got = read(&bytes, "Set").unwrap();
            assert_eq!(got.brushes.len(), 2);
            let r = &got.brushes[0];
            assert_eq!((r.name.as_str(), r.brush.size, r.brush.hardness, r.brush.roundness, r.brush.angle), ("Set 1", 40.0, 0.5, 0.6, 30.0));
            assert!((r.brush.spacing - 0.3).abs() < 1e-6 && r.tip.is_none());
            let s = &got.brushes[1];
            assert_eq!(s.name, "Dot");
            let t = s.tip.as_ref().unwrap();
            assert_eq!((t.width, t.height, s.brush.size), (24, 24, 24.0));
            assert_eq!(t.alpha[12 * 24 + 12], 255, "the middle is paint");
            assert_eq!(t.alpha[0], 0, "the corner is not");
        }
    }

    #[test]
    fn v6_presets_find_their_tips_and_settings() {
        let mut s = disc(32);
        s.id = "$tip-1".into();
        let preset = Descriptor::new("brushPreset").with("Nm  ", Value::Text(UnicodeString::new_nul("Leafy"))).with(
            "Brsh",
            Value::Descriptor(
                Descriptor::new("sampledBrush")
                    .with("sampledData", Value::Text(UnicodeString::new_nul("$tip-1")))
                    .with("Dmtr", Value::UnitFloat { unit: *b"#Pxl", value: 64.0 })
                    .with("Spcn", Value::UnitFloat { unit: *b"#Prc", value: 50.0 }),
            ),
        );
        let orphan = Descriptor::new("brushPreset")
            .with("Nm  ", Value::Text(UnicodeString::new_nul("Lost")))
            .with("Brsh", Value::Descriptor(Descriptor::new("sampledBrush").with("sampledData", Value::Text(UnicodeString::new_nul("$nope")))));
        for sub in [1, 2] {
            let bytes = write_v6(sub, std::slice::from_ref(&s), &[], &[preset.clone(), orphan.clone()], true).unwrap();
            let got = read(&bytes, "x").unwrap();
            assert_eq!(got.brushes.len(), 1, "the preset whose tip is missing is skipped");
            assert!(got.warnings.iter().any(|w| w.contains("Lost")));
            let b = &got.brushes[0];
            assert_eq!((b.name.as_str(), b.brush.size), ("Leafy", 64.0));
            assert!((b.brush.spacing - 0.5).abs() < 1e-6);
            assert_eq!(b.tip.as_ref().map(|t| t.width), Some(32));
        }
        // tips without presets still come through
        let bytes = write_v6(1, &[s], &[], &[], false).unwrap();
        assert_eq!(read(&bytes, "Tips").unwrap().brushes[0].name, "Tips 1");
    }

    #[test]
    fn huge_tips_are_scaled_down_and_junk_is_refused() {
        let big = AbrSample { id: String::new(), width: 3000, height: 1000, depth: 8, data: vec![255; 3000 * 1000] };
        let bytes = write_v12(1, &[LegacyBrush { name: String::new(), spacing: 25, anti_alias: true, tip: LegacyTip::Sampled(big) }], false).unwrap();
        let t = read(&bytes, "b").unwrap().brushes[0].tip.clone().unwrap();
        assert!(t.width <= MAX_TIP_SIDE && t.height <= MAX_TIP_SIDE && t.width == 1000, "{}×{}", t.width, t.height);
        assert!(read(b"not a brush file", "j").is_err());
        assert!(read(&[], "j").is_err());
        // a damaged file: cut short
        let ok = write_v12(2, &[LegacyBrush { name: "a".into(), spacing: 25, anti_alias: true, tip: LegacyTip::Sampled(disc(16)) }], true).unwrap();
        for n in [3, 10, ok.len() / 2] {
            let _ = read(&ok[..n], "j");
        }
    }
}
