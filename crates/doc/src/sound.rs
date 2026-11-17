//! The shot's sound: clips of sound files placed on audio tracks. A clip keeps the user's file
//! as it was imported (nothing is re-encoded until export) and says where it starts, which part
//! of the file plays, its fades and its volume (with optional keys). Tracks can be muted, soloed
//! and have their own volume.

use serde::{Deserialize, Serialize};

/// Most sound clips in one shot.
pub const MAX_SOUNDS: usize = 64;
/// Most audio tracks in one shot.
pub const MAX_AUDIO_TRACKS: usize = 16;
/// Most volume keys on one clip.
pub const MAX_VOLUME_KEYS: usize = 4096;
/// Loudest gain (×4 ≈ +12 dB).
pub const MAX_GAIN: f32 = 4.0;
/// Longest fade, trim or clip length kept, in seconds (two hours).
pub const MAX_SECONDS: f64 = 7200.0;

/// A clip's gain from a frame on.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct VolumeKey {
    pub frame: u32,
    pub gain: f32,
}

/// One sound clip.
#[derive(Clone, Debug, PartialEq)]
pub struct Soundtrack {
    /// File name it was imported from (shown in the timeline; its extension hints the decoder).
    pub name: String,
    /// Frame at which the clip starts (negative: it starts before the shot).
    pub offset: i32,
    pub bytes: std::sync::Arc<Vec<u8>>,
    /// The audio track it is on (0 is the first).
    pub track: u32,
    /// Seconds of the file skipped at its start.
    pub trim_start: f64,
    /// Seconds played after `trim_start` (`None`: to the end of the file).
    pub length: Option<f64>,
    /// Fade-in and fade-out lengths, seconds.
    pub fade_in: f64,
    pub fade_out: f64,
    /// Gain (1 = as recorded), used when the clip has no volume keys.
    pub volume: f32,
    /// Gain keys by shot frame, sorted; linear between keys, held outside them.
    pub volume_keys: Vec<VolumeKey>,
    pub muted: bool,
}

/// An audio track's switches.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioTrack {
    pub name: String,
    pub muted: bool,
    /// Soloed tracks: when any track is soloed, only soloed tracks play.
    pub solo: bool,
    pub volume: f32,
}

impl Default for AudioTrack {
    fn default() -> Self {
        Self { name: String::new(), muted: false, solo: false, volume: 1.0 }
    }
}

fn secs(v: f64) -> f64 {
    if v.is_finite() { v.clamp(0.0, MAX_SECONDS) } else { 0.0 }
}

fn gain(v: f32) -> f32 {
    if v.is_finite() { v.clamp(0.0, MAX_GAIN) } else { 1.0 }
}

impl Soundtrack {
    /// A clip of a whole file at full volume on the first track.
    pub fn new(name: &str, offset: i32, bytes: std::sync::Arc<Vec<u8>>) -> Self {
        Self {
            name: name.chars().take(200).collect(),
            offset: offset.clamp(-1_000_000, 1_000_000),
            bytes,
            track: 0,
            trim_start: 0.0,
            length: None,
            fade_in: 0.0,
            fade_out: 0.0,
            volume: 1.0,
            volume_keys: Vec::new(),
            muted: false,
        }
    }

    /// Every number clamped, keys sorted and deduplicated.
    pub fn sanitized(mut self) -> Self {
        self.name = self.name.chars().take(200).collect();
        self.offset = self.offset.clamp(-1_000_000, 1_000_000);
        self.track = self.track.min(MAX_AUDIO_TRACKS as u32 - 1);
        self.trim_start = secs(self.trim_start);
        self.length = self.length.map(secs);
        self.fade_in = secs(self.fade_in);
        self.fade_out = secs(self.fade_out);
        self.volume = gain(self.volume);
        self.volume_keys.truncate(MAX_VOLUME_KEYS);
        for k in &mut self.volume_keys {
            k.gain = gain(k.gain);
        }
        self.volume_keys.sort_by_key(|k| k.frame);
        self.volume_keys.dedup_by_key(|k| k.frame);
        self
    }

    /// The clip's gain at a (fractional) shot frame, before fades and the track's volume.
    pub fn gain_at(&self, frame: f64) -> f32 {
        let k = &self.volume_keys;
        let Some(first) = k.first() else { return self.volume };
        if frame <= first.frame as f64 {
            return first.gain;
        }
        let i = k.partition_point(|key| (key.frame as f64) <= frame);
        match (k.get(i.wrapping_sub(1)), k.get(i)) {
            (Some(a), Some(b)) if b.frame > a.frame => a.gain + (b.gain - a.gain) * ((frame - a.frame as f64) / (b.frame - a.frame) as f64) as f32,
            (Some(a), _) => a.gain,
            _ => self.volume,
        }
    }

    /// Set (or replace) a volume key; false when the clip has too many keys.
    pub fn set_volume_key(&mut self, frame: u32, g: f32) -> bool {
        let g = gain(g);
        match self.volume_keys.binary_search_by_key(&frame, |k| k.frame) {
            Ok(i) => {
                if let Some(k) = self.volume_keys.get_mut(i) {
                    k.gain = g;
                }
            }
            Err(i) => {
                if self.volume_keys.len() >= MAX_VOLUME_KEYS {
                    return false;
                }
                self.volume_keys.insert(i, VolumeKey { frame, gain: g });
            }
        }
        true
    }

    /// Fade envelope at `t` seconds into the played part, which is `len` seconds long.
    pub fn fade_at(&self, t: f64, len: f64) -> f32 {
        let mut g = 1.0f64;
        if self.fade_in > 0.0 {
            g = g.min(t / self.fade_in);
        }
        if self.fade_out > 0.0 {
            g = g.min((len - t) / self.fade_out);
        }
        g.clamp(0.0, 1.0) as f32
    }
}

impl AudioTrack {
    pub fn sanitized(mut self) -> Self {
        self.name = self.name.chars().take(200).collect();
        self.volume = gain(self.volume);
        self
    }
}

/// Is a clip heard, given the tracks' mute and solo switches?
pub fn audible(clip: &Soundtrack, tracks: &[AudioTrack]) -> bool {
    if clip.muted {
        return false;
    }
    let any_solo = tracks.iter().any(|t| t.solo);
    match tracks.get(clip.track as usize) {
        Some(t) => !t.muted && (!any_solo || t.solo),
        None => !any_solo,
    }
}

/// A track's volume (1 for a track with no settings).
pub fn track_volume(clip: &Soundtrack, tracks: &[AudioTrack]) -> f32 {
    tracks.get(clip.track as usize).map_or(1.0, |t| t.volume)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_fades_and_switches() {
        let mut c = Soundtrack::new("a.wav", 0, std::sync::Arc::new(Vec::new()));
        assert_eq!(c.gain_at(5.0), 1.0);
        assert!(c.set_volume_key(0, 0.0) && c.set_volume_key(10, 2.0));
        assert_eq!(c.gain_at(5.0), 1.0);
        assert_eq!(c.gain_at(20.0), 2.0);
        c.fade_in = 1.0;
        c.fade_out = 2.0;
        assert_eq!(c.fade_at(0.5, 10.0), 0.5);
        assert_eq!(c.fade_at(9.0, 10.0), 0.5);
        assert_eq!(c.fade_at(5.0, 10.0), 1.0);
        let mut tracks = vec![AudioTrack::default(), AudioTrack::default()];
        assert!(audible(&c, &tracks));
        tracks[1].solo = true;
        assert!(!audible(&c, &tracks), "another track is soloed");
        tracks[0].solo = true;
        tracks[0].muted = true;
        assert!(!audible(&c, &tracks));
        let wild = Soundtrack { volume: f32::NAN, trim_start: -4.0, fade_in: f64::INFINITY, track: 999, ..c }.sanitized();
        assert_eq!((wild.volume, wild.trim_start, wild.fade_in, wild.track), (1.0, 0.0, 0.0, MAX_AUDIO_TRACKS as u32 - 1));
    }
}
