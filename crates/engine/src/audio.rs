//! The soundtrack: decoding (craft-audio), waveform peaks and the PCM an export needs.

use std::sync::Arc;

use tooncraft_doc::{Project, Soundtrack};

use crate::undo::{Entry, Part};
use crate::{Engine, EngineError, Result};

/// Longest soundtrack decoded: 2 hours of 48 kHz stereo.
pub const MAX_SAMPLES: u64 = 48_000 * 2 * 7200;

/// Decoded sound: planar `f32`, at most two channels kept.
#[derive(Clone, Debug, PartialEq)]
pub struct Pcm {
    pub rate: u32,
    pub channels: Vec<Vec<f32>>,
}

impl Pcm {
    pub fn frames(&self) -> usize {
        self.channels.first().map(Vec::len).unwrap_or(0)
    }

    pub fn seconds(&self) -> f64 {
        self.frames() as f64 / self.rate.max(1) as f64
    }

    /// Mono mix for the waveform.
    pub fn mono(&self) -> Vec<f32> {
        let n = self.channels.len().max(1) as f32;
        (0..self.frames()).map(|i| self.channels.iter().map(|c| c.get(i).copied().unwrap_or(0.0)).sum::<f32>() / n).collect()
    }
}

/// Decode a sound file's bytes. The name's extension helps detect the format.
pub fn decode(bytes: &[u8], name: &str) -> std::result::Result<Pcm, String> {
    let ext = std::path::Path::new(name).extension().and_then(|e| e.to_str());
    let mut opts = craft_audio::DecodeOptions::default();
    opts.max_samples = MAX_SAMPLES;
    let (_, buf) = craft_audio::decode_with(bytes, ext, &opts).map_err(|e| e.to_string())?;
    if buf.sample_rate == 0 || buf.channels.is_empty() {
        return Err("the file has no sound".into());
    }
    let channels = buf.channels.into_iter().take(2).collect();
    Ok(Pcm { rate: buf.sample_rate, channels })
}

/// The sound under the frames `start..end` of the shot (silence where the sound has none).
pub fn segment(pcm: &Pcm, s: &Soundtrack, fps: u32, start: u32, end: u32) -> Pcm {
    let fps = fps.max(1) as f64;
    let rate = pcm.rate as f64;
    let from = ((start as f64 - s.offset as f64) / fps * rate).round() as i64;
    let len = (((end - start) as f64) / fps * rate).round().max(0.0) as usize;
    let channels = pcm
        .channels
        .iter()
        .map(|c| {
            (0..len)
                .map(|i| {
                    let at = from + i as i64;
                    if at >= 0 { c.get(at as usize).copied().unwrap_or(0.0) } else { 0.0 }
                })
                .collect()
        })
        .collect();
    Pcm { rate: pcm.rate, channels }
}

pub(crate) fn set(e: &mut Engine, path: Option<&str>, offset: i32) -> Result<()> {
    let new = match path {
        Some(p) => {
            let meta = std::fs::metadata(p).map_err(|err| EngineError::Import(format!("{p}: {err}")))?;
            if meta.len() > tooncraft_format::MAX_SOUND_BYTES {
                return Err(EngineError::Import(format!("{p}: larger than 512 MB")));
            }
            let bytes = std::fs::read(p).map_err(|err| EngineError::Import(format!("{p}: {err}")))?;
            let name = std::path::Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "sound".into());
            decode(&bytes, &name).map_err(|err| EngineError::Import(format!("{p}: {err}")))?;
            Some(Soundtrack { name, offset, bytes: Arc::new(bytes) })
        }
        None => None,
    };
    let old = std::mem::replace(&mut e.project.soundtrack, new);
    e.push(Entry { label: "Sound".into(), parts: vec![Part::Sound(old)] });
    Ok(())
}

pub(crate) fn shift(e: &mut Engine, offset: i32) -> Result<()> {
    let Some(s) = e.project.soundtrack.clone() else { return Ok(()) };
    let old = Some(s.clone());
    e.project.soundtrack = Some(Soundtrack { offset: offset.clamp(-1_000_000, 1_000_000), ..s });
    e.push(Entry { label: "Move sound".into(), parts: vec![Part::Sound(old)] });
    Ok(())
}

/// Decode the project's soundtrack, if any.
pub fn decode_project(p: &Project) -> Option<std::result::Result<Pcm, String>> {
    p.soundtrack.as_ref().map(|s| decode(&s.bytes, &s.name))
}
