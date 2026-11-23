//! Colour palette files: GIMP palettes (`.gpl`, read through craft-palette, written here; the
//! shared crate has no writer yet, see `docs/gaps.md`) and Photoshop colour swatches (`.aco`,
//! from Adobe's published file-format description: version 1 records of five big-endian u16,
//! optionally followed by version 2 with UTF-16 names). Files are hostile: sizes and counts are
//! bounded, and anything malformed is an error, never a panic.

use tooncraft_doc::palette::{MAX_SWATCHES, Palette};

/// Largest palette file read.
pub const MAX_FILE: usize = 1 << 20;

/// A palette as GIMP palette text.
pub fn to_gpl(p: &Palette) -> String {
    let clean = |s: &str| s.chars().filter(|c| !c.is_control()).collect::<String>();
    let mut out = format!("GIMP Palette\nName: {}\nColumns: 8\n#\n", clean(&p.name));
    for s in &p.swatches {
        let name = if s.name.is_empty() { "Untitled".to_string() } else { clean(&s.name) };
        out.push_str(&format!("{:3} {:3} {:3}\t{}\n", s.rgb[0], s.rgb[1], s.rgb[2], name));
    }
    out
}

/// A palette from GIMP palette text.
pub fn from_gpl(text: &str) -> Result<Palette, String> {
    let g = craft_palette::parse_gpl(text)?;
    let mut p = Palette::new(g.name.trim());
    for c in g.colors.into_iter().take(MAX_SWATCHES) {
        let name = if c.name == "Untitled" { String::new() } else { c.name };
        p.push(&name, c.rgb);
    }
    Ok(p.sanitized())
}

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*b.get(i)?, *b.get(i + 1)?]))
}

/// HSB (hue in 0..360·182.04, saturation and brightness in 0..65535) to RGB.
fn hsb_to_rgb(h: u16, s: u16, v: u16) -> [u8; 3] {
    let (h, s, v) = (h as f32 / 65535.0 * 360.0, s as f32 / 65535.0, v as f32 / 65535.0);
    let c = v * s;
    let hp = (h / 60.0) % 6.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    let q = |f: f32| ((f + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    [q(r), q(g), q(b)]
}

/// A palette from Photoshop colour swatches. RGB, HSB and greyscale colours are read; colours
/// in other spaces (CMYK, Lab …) are skipped and counted in the returned number.
pub fn from_aco(bytes: &[u8], name: &str) -> Result<(Palette, usize), String> {
    if bytes.len() > MAX_FILE {
        return Err("the swatch file is too large".into());
    }
    let bad = || "this isn't a Photoshop swatch file, or it is cut short".to_string();
    let version = u16_at(bytes, 0).ok_or_else(bad)?;
    if version != 1 && version != 2 {
        return Err(bad());
    }
    let count = u16_at(bytes, 2).ok_or_else(bad)? as usize;
    let records = 4 + count * 10;
    if records > bytes.len() {
        return Err(bad());
    }
    // the colours of version 1; names come from a version 2 block after it, when present
    let mut colours: Vec<Option<[u8; 3]>> = Vec::with_capacity(count.min(MAX_SWATCHES));
    for i in 0..count.min(MAX_SWATCHES) {
        let at = 4 + i * 10;
        let space = u16_at(bytes, at).ok_or_else(bad)?;
        let (w, x, y) = (u16_at(bytes, at + 2).ok_or_else(bad)?, u16_at(bytes, at + 4).ok_or_else(bad)?, u16_at(bytes, at + 6).ok_or_else(bad)?);
        colours.push(match space {
            0 => Some([(w >> 8) as u8, (x >> 8) as u8, (y >> 8) as u8]),
            1 => Some(hsb_to_rgb(w, x, y)),
            8 => {
                // greyscale: 0 … 10000, 0 is black
                let g = (w.min(10_000) as f32 / 10_000.0 * 255.0).round() as u8;
                Some([g, g, g])
            }
            _ => None,
        });
    }
    let mut names: Vec<String> = Vec::new();
    if version == 1 && u16_at(bytes, records) == Some(2) {
        let count2 = u16_at(bytes, records + 2).unwrap_or(0) as usize;
        let mut at = records + 4;
        for _ in 0..count2.min(count).min(MAX_SWATCHES) {
            // five u16 of colour, a zero, the name length (UTF-16 units, with the final 0), the name
            let len_at = at + 10 + 2;
            let Some(len) = u16_at(bytes, len_at) else { break };
            let len = len as usize;
            let start = len_at + 2;
            if len > 256 || start + len * 2 > bytes.len() {
                break;
            }
            let units: Vec<u16> = (0..len).filter_map(|k| u16_at(bytes, start + k * 2)).take_while(|u| *u != 0).collect();
            names.push(String::from_utf16_lossy(&units));
            at = start + len * 2;
        }
    }
    let mut p = Palette::new(name);
    let mut skipped = 0;
    for (i, c) in colours.into_iter().enumerate() {
        match c {
            Some(rgb) => {
                p.push(names.get(i).map(String::as_str).unwrap_or(""), rgb);
            }
            None => skipped += 1,
        }
    }
    if p.swatches.is_empty() {
        return Err("the swatch file has no RGB, HSB or grey colours".into());
    }
    Ok((p.sanitized(), skipped))
}

/// Read a palette file by its extension (`.gpl` or `.aco`).
pub fn read_file(path: &std::path::Path) -> Result<Palette, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > MAX_FILE as u64 {
        return Err("the palette file is too large".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    match path.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref() {
        Some("aco") => from_aco(&bytes, &stem).map(|(p, _)| p),
        _ => {
            let text = String::from_utf8(bytes).map_err(|_| "the palette file is not text".to_string())?;
            let mut p = from_gpl(&text)?;
            if p.name == "Palette" && !stem.is_empty() {
                p.name = stem;
            }
            Ok(p)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aco_v1(colours: &[(u16, [u16; 4])]) -> Vec<u8> {
        let mut b = vec![0, 1];
        b.extend((colours.len() as u16).to_be_bytes());
        for (space, v) in colours {
            b.extend(space.to_be_bytes());
            for x in v {
                b.extend(x.to_be_bytes());
            }
        }
        b
    }

    #[test]
    fn gpl_round_trip_and_aco() {
        let mut p = Palette::new("Skin tones");
        p.push("light", [250, 220, 200]);
        p.push("", [120, 80, 60]);
        let text = to_gpl(&p);
        let back = from_gpl(&text).unwrap();
        assert_eq!(back.name, "Skin tones");
        assert_eq!(back.swatches, p.swatches);
        assert!(from_gpl("not a palette").is_err());
        // ACO: RGB, HSB (pure red), grey and a CMYK colour that is skipped
        let b = aco_v1(&[(0, [0xFF00, 0x8000, 0x0000, 0]), (1, [0, 65535, 65535, 0]), (8, [5000, 0, 0, 0]), (2, [1, 2, 3, 4])]);
        let (a, skipped) = from_aco(&b, "Imported").unwrap();
        assert_eq!(skipped, 1);
        assert_eq!(a.swatches.iter().map(|s| s.rgb).collect::<Vec<_>>(), vec![[255, 128, 0], [255, 0, 0], [128, 128, 128]]);
        // a version 2 block names them
        let mut b2 = aco_v1(&[(0, [0xFF00, 0, 0, 0])]);
        b2.extend([0, 2, 0, 1]);
        b2.extend([0, 0, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let name: Vec<u16> = "Red".encode_utf16().chain([0]).collect();
        b2.extend((name.len() as u16).to_be_bytes());
        for u in name {
            b2.extend(u.to_be_bytes());
        }
        assert_eq!(from_aco(&b2, "x").unwrap().0.swatches[0].name, "Red");
    }

    #[test]
    fn hostile_aco_is_an_error_not_a_panic() {
        assert!(from_aco(&[], "x").is_err());
        assert!(from_aco(&[0, 1, 0xFF, 0xFF], "x").is_err(), "a count past the end");
        assert!(from_aco(&[0, 9, 0, 0], "x").is_err());
        assert!(from_aco(&aco_v1(&[(7, [0; 4])]), "x").is_err(), "nothing readable");
        let mut cut = aco_v1(&[(0, [0; 4])]);
        cut.extend([0, 2, 0, 1, 0, 0]);
        assert!(from_aco(&cut, "x").is_ok(), "a cut-short name block keeps the colours");
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(64))]
        #[test]
        fn random_bytes_never_panic(b in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..400)) {
            let _ = from_aco(&b, "x");
            let _ = from_gpl(&String::from_utf8_lossy(&b));
        }
    }
}
