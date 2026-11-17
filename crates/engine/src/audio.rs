//! Sound: decoding clips (craft-audio), mixing them to one stereo signal for playback and
//! export, and the sound commands.
//!
//! The mix is linear: every audible clip (not muted, its track not muted, and soloed when any
//! track is soloed) is resampled to [`MIX_RATE`] with linear interpolation, multiplied by its
//! volume (or volume keys), its fades and its track's volume, and summed; the sum is clamped to
//! ±1. Mono clips go to both channels.

use std::sync::Arc;

use tooncraft_doc::sound::{MAX_AUDIO_TRACKS, MAX_SOUNDS};
use tooncraft_doc::{AudioTrack, Project, Soundtrack};

use crate::undo::{Entry, Part};
use crate::{Engine, EngineError, Result};

/// Longest sound decoded: 2 hours of 48 kHz stereo.
pub const MAX_SAMPLES: u64 = 48_000 * 2 * 7200;
/// The rate the mix is made at.
pub const MIX_RATE: u32 = 48_000;

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

    /// Linearly interpolated sample of channel `c` at `t` seconds (silence outside).
    fn at(&self, c: usize, t: f64) -> f32 {
        let Some(ch) = self.channels.get(c.min(self.channels.len().saturating_sub(1))) else { return 0.0 };
        let x = t * self.rate as f64;
        if x.is_nan() || x < 0.0 {
            return 0.0;
        }
        let i = x.floor() as usize;
        let f = (x - i as f64) as f32;
        match (ch.get(i), ch.get(i + 1)) {
            (Some(a), Some(b)) => a + (b - a) * f,
            (Some(a), None) => *a,
            _ => 0.0,
        }
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

/// Seconds of the clip that play (after the trim, within the file).
pub fn played_seconds(clip: &Soundtrack, pcm: &Pcm) -> f64 {
    let rest = (pcm.seconds() - clip.trim_start).max(0.0);
    clip.length.map_or(rest, |l| l.min(rest))
}

/// The mix of `clips` (decoded in `pcms`, same order; `None` is skipped) under shot frames
/// `start..end`, stereo at [`MIX_RATE`].
pub fn mix(p: &Project, pcms: &[Option<&Pcm>], start: u32, end: u32) -> Pcm {
    let fps = p.fps().max(1) as f64;
    let rate = MIX_RATE as f64;
    let len = ((end.saturating_sub(start)) as f64 / fps * rate).round().clamp(0.0, MAX_SAMPLES as f64 / 2.0) as usize;
    let (mut left, mut right) = (vec![0f32; len], vec![0f32; len]);
    let t0 = start as f64 / fps;
    for (clip, pcm) in p.sounds.iter().zip(pcms) {
        let Some(pcm) = pcm else { continue };
        if !tooncraft_doc::sound::audible(clip, &p.audio_tracks) {
            continue;
        }
        let track = tooncraft_doc::sound::track_volume(clip, &p.audio_tracks);
        let begin = clip.offset as f64 / fps;
        let played = played_seconds(clip, pcm);
        // output samples the clip covers
        let first = (((begin - t0) * rate).ceil().max(0.0) as usize).min(len);
        let last = (((begin + played - t0) * rate).floor().max(0.0) as usize).min(len);
        let stereo = pcm.channels.len() > 1;
        for n in first..last {
            let t = t0 + n as f64 / rate;
            let local = t - begin;
            let g = clip.gain_at(t * fps) * clip.fade_at(local, played) * track;
            if g == 0.0 {
                continue;
            }
            let src = clip.trim_start + local;
            let l = pcm.at(0, src);
            let r = if stereo { pcm.at(1, src) } else { l };
            if let (Some(a), Some(b)) = (left.get_mut(n), right.get_mut(n)) {
                *a += l * g;
                *b += r * g;
            }
        }
    }
    for v in left.iter_mut().chain(right.iter_mut()) {
        *v = v.clamp(-1.0, 1.0);
    }
    Pcm { rate: MIX_RATE, channels: vec![left, right] }
}

/// Decode every clip of the project. `Err` names the first clip that fails.
pub fn decode_all(p: &Project) -> std::result::Result<Vec<Pcm>, String> {
    p.sounds.iter().map(|s| decode(&s.bytes, &s.name).map_err(|e| format!("{}: {e}", s.name))).collect()
}

/// The mix of the project's sound under frames `start..end`; `None` without sound clips.
pub fn mix_project(p: &Project, start: u32, end: u32) -> Option<std::result::Result<Pcm, String>> {
    if p.sounds.is_empty() {
        return None;
    }
    Some(decode_all(p).map(|pcms| {
        let refs: Vec<Option<&Pcm>> = pcms.iter().map(Some).collect();
        mix(p, &refs, start, end)
    }))
}

/// The mix as a 16-bit WAV file's bytes.
pub fn wav(pcm: &Pcm) -> std::result::Result<Vec<u8>, String> {
    let buf = craft_audio::AudioBuffer { sample_rate: pcm.rate, channels: pcm.channels.clone() };
    craft_audio::encode(&buf, &craft_audio::EncodeOptions::new(craft_audio::EncodeFormat::Wav)).map_err(|e| e.to_string())
}

/// Write the mix under frames `start..end` as a WAV file (replaced atomically).
pub fn export_wav(p: &Project, path: &std::path::Path, start: u32, end: u32) -> std::result::Result<(), String> {
    let pcm = match mix_project(p, start, end) {
        Some(r) => r?,
        None => return Err("the shot has no sound".into()),
    };
    let bytes = wav(&pcm)?;
    craft_atomic::atomic_write(path, &bytes).map_err(|e| format!("{}: {e}", path.display()))
}

fn read_clip(path: &str) -> Result<(String, Arc<Vec<u8>>)> {
    let meta = std::fs::metadata(path).map_err(|err| EngineError::Import(format!("{path}: {err}")))?;
    if meta.len() > tooncraft_format::MAX_SOUND_BYTES {
        return Err(EngineError::Import(format!("{path}: larger than 512 MB")));
    }
    let bytes = std::fs::read(path).map_err(|err| EngineError::Import(format!("{path}: {err}")))?;
    let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "sound".into());
    decode(&bytes, &name).map_err(|err| EngineError::Import(format!("{path}: {err}")))?;
    Ok((name, Arc::new(bytes)))
}

fn record(e: &mut Engine, label: &str, sounds: Vec<Soundtrack>, tracks: Vec<AudioTrack>) {
    e.push(Entry { label: label.into(), parts: vec![Part::Sounds(sounds, tracks)] });
}

/// Replace every clip with one sound file (`None`: remove all sound).
pub(crate) fn set(e: &mut Engine, path: Option<&str>, offset: i32) -> Result<()> {
    let new = match path {
        Some(p) => {
            let (name, bytes) = read_clip(p)?;
            vec![Soundtrack::new(&name, offset, bytes)]
        }
        None => Vec::new(),
    };
    let old = std::mem::replace(&mut e.project.sounds, new);
    let tracks = e.project.audio_tracks.clone();
    record(e, "Sound", old, tracks);
    Ok(())
}

/// Add a clip of a sound file on `track`, starting at frame `offset`.
pub(crate) fn add(e: &mut Engine, path: &str, offset: i32, track: u32) -> Result<()> {
    if e.project.sounds.len() >= MAX_SOUNDS {
        return Err(EngineError::Import(format!("a shot can have at most {MAX_SOUNDS} sound clips")));
    }
    let (name, bytes) = read_clip(path)?;
    let mut clip = Soundtrack::new(&name, offset, bytes);
    clip.track = track.min(MAX_AUDIO_TRACKS as u32 - 1);
    let (old, tracks) = (e.project.sounds.clone(), e.project.audio_tracks.clone());
    e.project.sounds.push(clip);
    record(e, "Add sound", old, tracks);
    Ok(())
}

/// Change the clip at `index` with `f` (false: refused), as one undo step.
pub(crate) fn edit(e: &mut Engine, index: usize, label: &str, f: impl FnOnce(&mut Soundtrack) -> bool) -> Result<()> {
    let (old, tracks) = (e.project.sounds.clone(), e.project.audio_tracks.clone());
    let clip = e.project.sounds.get_mut(index).ok_or(EngineError::NoSound(index))?;
    let before = clip.clone();
    if !f(clip) {
        *clip = before;
        return Err(EngineError::NoSound(index));
    }
    *clip = clip.clone().sanitized();
    if *clip != before {
        record(e, label, old, tracks);
    }
    Ok(())
}

pub(crate) fn remove(e: &mut Engine, index: usize) -> Result<()> {
    if index >= e.project.sounds.len() {
        return Err(EngineError::NoSound(index));
    }
    let (old, tracks) = (e.project.sounds.clone(), e.project.audio_tracks.clone());
    e.project.sounds.remove(index);
    record(e, "Remove sound", old, tracks);
    Ok(())
}

/// Change an audio track's switches (tracks are made up to `track` as needed).
pub(crate) fn set_track(e: &mut Engine, track: u32, f: impl FnOnce(&mut AudioTrack)) -> Result<()> {
    let t = track as usize;
    if t >= MAX_AUDIO_TRACKS {
        return Err(EngineError::NoSound(t));
    }
    let (old, tracks) = (e.project.sounds.clone(), e.project.audio_tracks.clone());
    if e.project.audio_tracks.len() <= t {
        e.project.audio_tracks.resize(t + 1, AudioTrack::default());
    }
    if let Some(at) = e.project.audio_tracks.get_mut(t) {
        f(at);
        *at = at.clone().sanitized();
    }
    record(e, "Audio track", old, tracks);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, secs: f32, amp: f32) -> Pcm {
        let n = (rate as f32 * secs) as usize;
        Pcm { rate, channels: vec![vec![amp; n]] }
    }

    fn shot() -> Project {
        let mut p = Project::new("s", 64, 64, 24, 48).unwrap();
        p.sounds.push(Soundtrack::new("a.wav", 0, Arc::new(Vec::new())));
        p
    }

    #[test]
    fn offset_trim_fade_and_volume() {
        let mut p = shot();
        let a = tone(24_000, 1.0, 0.5);
        p.sounds[0].offset = 12; // half a second in
        let m = mix(&p, &[Some(&a)], 0, 48);
        assert_eq!(m.frames(), 96_000);
        assert_eq!(m.channels[0][MIX_RATE as usize / 4], 0.0, "silent before the clip");
        assert!((m.channels[1][MIX_RATE as usize] - 0.5).abs() < 1e-6, "mono goes to both sides");
        assert_eq!(m.channels[0][MIX_RATE as usize * 3 / 2 + 10], 0.0, "silent after it");
        p.sounds[0].trim_start = 0.75;
        let m = mix(&p, &[Some(&a)], 0, 48);
        assert!(m.channels[0][(MIX_RATE as f64 * 0.7) as usize] > 0.0 && m.channels[0][(MIX_RATE as f64 * 0.8) as usize] == 0.0, "a quarter second plays");
        p.sounds[0].trim_start = 0.0;
        p.sounds[0].fade_in = 0.5;
        p.sounds[0].volume = 2.0;
        let m = mix(&p, &[Some(&a)], 0, 48);
        let q = m.channels[0][(MIX_RATE as f64 * 0.75) as usize];
        assert!((q - 0.5).abs() < 0.01, "half way through the fade at double volume: {q}");
        // a range of the shot starts where the shot frame is
        let part = mix(&p, &[Some(&a)], 24, 48);
        assert_eq!(part.frames(), MIX_RATE as usize);
        assert!((part.channels[0][0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn tracks_mute_solo_and_sum() {
        let mut p = shot();
        p.sounds.push(Soundtrack { track: 1, ..Soundtrack::new("b.wav", 0, Arc::new(Vec::new())) });
        let a = tone(48_000, 1.0, 0.3);
        let b = tone(48_000, 1.0, 0.4);
        let at = |p: &Project| mix(p, &[Some(&a), Some(&b)], 0, 24).channels[0][100];
        assert!((at(&p) - 0.7).abs() < 1e-6);
        p.audio_tracks = vec![AudioTrack::default(), AudioTrack { solo: true, ..Default::default() }];
        assert!((at(&p) - 0.4).abs() < 1e-6, "solo track 1");
        p.audio_tracks[1].muted = true;
        assert_eq!(at(&p), 0.0);
        p.audio_tracks = vec![AudioTrack { volume: 0.5, ..Default::default() }];
        p.sounds[1].muted = true;
        assert!((at(&p) - 0.15).abs() < 1e-6);
        p.sounds[1].muted = false;
        p.sounds[1].volume = 4.0;
        assert_eq!(at(&p), 1.0, "clamped");
    }

    #[test]
    fn resampling_and_volume_keys() {
        let mut p = shot();
        let ramp = Pcm { rate: 8_000, channels: vec![(0..8_000).map(|i| i as f32 / 8_000.0).collect(), vec![0.25; 8_000]] };
        let m = mix(&p, &[Some(&ramp)], 0, 24);
        let x = m.channels[0][24_000];
        assert!((x - 0.5).abs() < 1e-3, "linear resampling: {x}");
        assert_eq!(m.channels[1][24_000], 0.25, "right channel kept");
        p.sounds[0].set_volume_key(0, 0.0);
        p.sounds[0].set_volume_key(24, 1.0);
        let m = mix(&p, &[Some(&ramp)], 0, 24);
        assert!((m.channels[1][24_000] - 0.125).abs() < 1e-3);
    }
}
