//! Lip sync as commands: a layer's mouth chart, mouth detection from the shot's sound mix, and
//! mouth timing sheets in and out. Detection and sheet imports write ordinary exposure keys (one
//! undo step), so the animator edits the result like any other timing.

use tooncraft_doc::{Cell, DrawingId, Project};
use tooncraft_lipsync::{Mouth, Settings, sheet};

use crate::undo::{Entry, Part};
use crate::{Engine, EngineError, Result};

/// The drawing to show for `m`: its own, else the nearest look-alike the chart has.
fn drawing_for(chart: &std::collections::BTreeMap<Mouth, DrawingId>, m: Mouth) -> Option<DrawingId> {
    let fallbacks: &[Mouth] = match m {
        Mouth::Rest => &[Mouth::Closed],
        Mouth::Closed => &[Mouth::Rest],
        Mouth::Open => &[Mouth::Wide, Mouth::Round],
        Mouth::Wide => &[Mouth::Open, Mouth::Consonant],
        Mouth::Round => &[Mouth::Pucker, Mouth::Open],
        Mouth::Pucker => &[Mouth::Round, Mouth::Closed],
        Mouth::Teeth => &[Mouth::Consonant, Mouth::Wide],
        Mouth::Tongue => &[Mouth::Consonant, Mouth::Wide, Mouth::Open],
        Mouth::Consonant => &[Mouth::Wide, Mouth::Tongue, Mouth::Open],
    };
    std::iter::once(&m).chain(fallbacks).chain(&[Mouth::Rest, Mouth::Closed]).find_map(|k| chart.get(k).copied())
}

/// Write `changes` (0-based frames) as keys and holds on the layer from `from` to the end.
fn write(e: &mut Engine, layer: usize, from: u32, changes: &[(u32, Mouth)], label: &str) -> Result<usize> {
    e.check_unlocked(layer)?;
    let chart = e.project.layer(layer)?.mouth_chart.clone();
    if chart.is_empty() {
        return Err(EngineError::Import("the layer has no mouth chart: pick a drawing for at least one mouth".into()));
    }
    let len = e.project.length();
    let before = e.project.layers.clone();
    let l = e.project.layer_mut(layer)?;
    let mut keys = 0;
    let mut last: Option<DrawingId> = None;
    let mut next = changes.iter().peekable();
    let mut current: Option<Mouth> = None;
    for f in from..len {
        while let Some((cf, m)) = next.peek() {
            if cf.saturating_add(from) <= f {
                current = Some(*m);
                next.next();
            } else {
                break;
            }
        }
        let d = current.and_then(|m| drawing_for(&chart, m));
        let cell = match d {
            Some(d) if last == Some(d) => Cell::Hold,
            Some(d) => {
                keys += 1;
                Cell::Key(d)
            }
            None => Cell::Blank,
        };
        if let Some(c) = l.cells.get_mut(f as usize) {
            *c = cell;
        }
        last = d;
    }
    e.push(Entry { label: label.into(), parts: vec![Part::Layers(before)] });
    Ok(keys)
}

/// Detect mouths from the shot's sound and key them on `layer` from frame `from` (the sound is
/// read from the same frame). Returns the number of keys written.
pub(crate) fn detect(e: &mut Engine, layer: usize, from: u32, settings: Settings) -> Result<usize> {
    let p = &e.project;
    let mix = match crate::audio::mix_project(p, from.min(p.length()), p.length()) {
        Some(Ok(m)) => m,
        Some(Err(err)) => return Err(EngineError::Import(err)),
        None => return Err(EngineError::Import("the shot has no sound".into())),
    };
    let mouths = tooncraft_lipsync::detect(&mix.mono(), mix.rate, p.fps(), settings);
    let changes = tooncraft_lipsync::changes(&mouths);
    write(e, layer, from, &changes, "Lip sync")
}

/// Key a mouth sheet's changes on `layer` from `from` (frames are rescaled when the sheet says
/// another frame rate).
pub(crate) fn import(e: &mut Engine, layer: usize, path: &str, from: u32) -> Result<usize> {
    let meta = std::fs::metadata(path).map_err(|err| EngineError::Import(format!("{path}: {err}")))?;
    if meta.len() > 16 << 20 {
        return Err(EngineError::Import(format!("{path}: larger than 16 MB")));
    }
    let text = std::fs::read_to_string(path).map_err(|err| EngineError::Import(format!("{path}: {err}")))?;
    let s = sheet::parse(&text).map_err(|err| EngineError::Import(format!("{path}: {err}")))?;
    let fps = e.project.fps();
    let changes: Vec<(u32, Mouth)> = match s.fps {
        Some(sf) if sf != fps => s.changes.iter().map(|(f, m)| (((*f as u64 * fps as u64 + sf as u64 / 2) / sf as u64) as u32, *m)).collect(),
        _ => s.changes,
    };
    write(e, layer, from, &changes, "Import mouth sheet")
}

/// The layer's mouth timing as a sheet (`switch`: switch-data format), read from its cells and
/// mouth chart; drawings that are not in the chart are skipped.
pub fn sheet_text(p: &Project, layer: usize, switch: bool) -> std::result::Result<String, String> {
    let l = p.layers.get(layer).ok_or_else(|| format!("there is no layer {layer}"))?;
    let by_drawing: std::collections::BTreeMap<DrawingId, Mouth> = l.mouth_chart.iter().map(|(m, d)| (*d, *m)).collect();
    let mut changes = Vec::new();
    let mut last = None;
    for f in 0..p.length() {
        let m = l.drawing_at(f).and_then(|d| by_drawing.get(&d).copied());
        if let Some(m) = m
            && last != Some(m)
        {
            changes.push((f, m));
        }
        last = m.or(last);
    }
    Ok(if switch { sheet::write_switch(&changes) } else { sheet::write(p.fps(), &changes) })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tooncraft_doc::{Project, Soundtrack};
    use tooncraft_lipsync::Mouth;

    use crate::{Command, Engine, sheet};

    fn wav(samples: Vec<f32>, rate: u32) -> Vec<u8> {
        craft_audio::encode(
            &craft_audio::AudioBuffer { sample_rate: rate, channels: vec![samples] },
            &craft_audio::EncodeOptions::new(craft_audio::EncodeFormat::Wav),
        )
        .unwrap()
    }

    fn vowel(f1: f32, f2: f32, secs: f32, rate: u32) -> Vec<f32> {
        let n = (rate as f32 * secs) as usize;
        let gain = |f: f32| 1.0 / (1.0 + ((f - f1) / 90.0).powi(2)) + 0.7 / (1.0 + ((f - f2) / 110.0).powi(2));
        (0..n)
            .map(|i| (1..34).map(|k| gain(k as f32 * 120.0) * (std::f32::consts::TAU * k as f32 * 120.0 * i as f32 / rate as f32).sin()).sum::<f32>() * 0.05)
            .collect()
    }

    #[test]
    fn detection_writes_editable_keys_from_the_chart() {
        let mut e = Engine::new(Project::new("talk", 64, 64, 24, 48).unwrap());
        let rate = 22_050;
        let mut s = vec![0.0; rate as usize / 4];
        s.extend(vowel(800.0, 1250.0, 0.5, rate));
        s.extend(vec![0.0; rate as usize / 2]);
        e.project.sounds.push(Soundtrack::new("line.wav", 0, Arc::new(wav(s, rate))));
        e.apply(Command::AddLayer { name: "Mouth".into(), index: None }).unwrap();
        assert!(e.apply(Command::LipSync { layer: 1, from: 0, settings: Default::default() }).is_err(), "no chart yet");
        for (frame, m) in [(0, Mouth::Rest), (1, Mouth::Open)] {
            e.apply(Command::NewDrawing { layer: 1, frame }).unwrap();
            let d = e.project.drawing_at(1, frame).unwrap();
            e.apply(Command::SetMouth { layer: 1, mouth: m, drawing: Some(d) }).unwrap();
        }
        let (rest, open) = (e.project.layers[1].mouth_chart[&Mouth::Rest], e.project.layers[1].mouth_chart[&Mouth::Open]);
        e.apply(Command::LipSync { layer: 1, from: 0, settings: Default::default() }).unwrap();
        let l = &e.project.layers[1];
        assert_eq!(l.drawing_at(0), Some(rest), "silence first");
        assert_eq!(l.drawing_at(12), Some(open), "the vowel opens the mouth");
        assert_eq!(l.drawing_at(40), Some(rest), "and it closes after");
        assert!(sheet(&l.cells).contains('-'), "holds, not a key on every frame: {}", sheet(&l.cells));
        // the sheet out and back in gives the same timing
        let text = crate::lipsync::sheet_text(&e.project, 1, false).unwrap();
        assert!(text.starts_with("tooncraft-mouths 1\nfps 24\n1 rest\n"), "{text}");
        let dir = std::env::temp_dir().join(format!("tooncraft-mouths-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("line.dat");
        std::fs::write(&path, crate::lipsync::sheet_text(&e.project, 1, true).unwrap()).unwrap();
        let cells = e.project.layers[1].cells.clone();
        e.undo().unwrap();
        e.apply(Command::ImportMouthSheet { layer: 1, path: path.to_string_lossy().into_owned(), from: 0 }).unwrap();
        assert_eq!(e.project.layers[1].cells, cells);
        // a sheet at another frame rate is rescaled
        let p12 = dir.join("twelve.txt");
        std::fs::write(&p12, "tooncraft-mouths 1\nfps 12\n1 rest\n3 open\n").unwrap();
        e.apply(Command::ImportMouthSheet { layer: 1, path: p12.to_string_lossy().into_owned(), from: 0 }).unwrap();
        assert_eq!(e.project.drawing_at(1, 4), Some(open));
        assert_eq!(e.project.drawing_at(1, 3), Some(rest));
        // removing a mouth from the chart
        e.apply(Command::SetMouth { layer: 1, mouth: Mouth::Open, drawing: None }).unwrap();
        assert!(!e.project.layers[1].mouth_chart.contains_key(&Mouth::Open));
    }
}
