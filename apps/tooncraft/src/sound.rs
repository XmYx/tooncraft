//! Sound playback (cpal) of the shot's sound mix (every clip on every audible track, mixed by the
//! engine): in sync with playback, and a one-frame snippet when scrubbing (stepping or dragging
//! the playhead) so lip sync can be found frame by frame. The waveform shown in the timeline comes
//! from craft-audio's peak pyramid.

use std::sync::{Arc, Mutex};

use tooncraft_engine::audio::Pcm;

#[derive(Default)]
struct Play {
    pcm: Option<Arc<Pcm>>,
    /// Position in the sound, seconds.
    pos: f64,
    playing: bool,
    /// Stop when `pos` reaches this (scrub snippets).
    stop_at: Option<f64>,
    muted: bool,
}

#[derive(Default)]
pub struct Sound {
    pub pcm: Option<Arc<Pcm>>,
    pub mono: Vec<f32>,
    pub peaks: Option<craft_audio::peaks::Peaks>,
    /// The sound settings the mix was made from.
    key: Option<u64>,
    /// Decoded clips by the address of their file bytes (clips of one file share it).
    decoded: std::collections::HashMap<usize, Arc<Pcm>>,
    state: Arc<Mutex<Play>>,
    stream: Option<cpal::Stream>,
    pub error: String,
    pub muted: bool,
}

/// A hash of everything the mix depends on (file identity, clip and track settings, timing).
fn settings_key(p: &tooncraft_doc::Project) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (p.fps(), p.length()).hash(&mut h);
    for s in &p.sounds {
        (Arc::as_ptr(&s.bytes) as usize, s.bytes.len(), &s.name, s.offset, s.track, s.muted).hash(&mut h);
        (s.trim_start.to_bits(), s.length.map(f64::to_bits), s.fade_in.to_bits(), s.fade_out.to_bits(), s.volume.to_bits()).hash(&mut h);
        for k in &s.volume_keys {
            (k.frame, k.gain.to_bits()).hash(&mut h);
        }
    }
    for t in &p.audio_tracks {
        (t.muted, t.solo, t.volume.to_bits()).hash(&mut h);
    }
    h.finish()
}

fn lock(m: &Mutex<Play>) -> std::sync::MutexGuard<'_, Play> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Sound {
    /// Mix the shot's sound again when a clip, a track or the timing changed. The mix covers
    /// the whole shot from frame 0, so its offset is 0.
    pub fn sync(&mut self, p: &tooncraft_doc::Project) {
        let key = settings_key(p);
        if Some(key) == self.key {
            return;
        }
        self.key = Some(key);
        self.pcm = None;
        self.mono.clear();
        self.peaks = None;
        self.error.clear();
        let live: Vec<usize> = p.sounds.iter().map(|s| Arc::as_ptr(&s.bytes) as usize).collect();
        self.decoded.retain(|k, _| live.contains(k));
        let mut pcms: Vec<Option<Arc<Pcm>>> = Vec::with_capacity(p.sounds.len());
        for (s, ptr) in p.sounds.iter().zip(&live) {
            let pcm = match self.decoded.get(ptr) {
                Some(d) => Some(d.clone()),
                None => match tooncraft_engine::audio::decode(&s.bytes, &s.name) {
                    Ok(d) => {
                        let d = Arc::new(d);
                        self.decoded.insert(*ptr, d.clone());
                        Some(d)
                    }
                    Err(e) => {
                        self.error = format!("{}: {e}", s.name);
                        None
                    }
                },
            };
            pcms.push(pcm);
        }
        if !p.sounds.is_empty() {
            let refs: Vec<Option<&Pcm>> = pcms.iter().map(|o| o.as_deref()).collect();
            let mix = tooncraft_engine::audio::mix(p, &refs, 0, p.length());
            self.mono = mix.mono();
            self.peaks = Some(craft_audio::peaks::Peaks::build(&self.mono));
            self.pcm = Some(Arc::new(mix));
        }
        let mut st = lock(&self.state);
        st.pcm = self.pcm.clone();
        st.playing = false;
    }

    fn ensure_stream(&mut self) {
        if self.stream.is_some() || cfg!(test) {
            return;
        }
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        let result = (|| -> Result<cpal::Stream, String> {
            let dev = cpal::default_host().default_output_device().ok_or("no sound output device")?;
            let cfg = dev.default_output_config().map_err(|e| e.to_string())?;
            let channels = cfg.channels().max(1) as usize;
            let rate = cfg.sample_rate().0.max(1) as f64;
            let config: cpal::StreamConfig = cfg.clone().into();
            let state = self.state.clone();
            let fill = move |out: &mut [f32]| {
                let mut st = lock(&state);
                for frame in out.chunks_mut(channels) {
                    let mut l = 0.0;
                    let mut r = 0.0;
                    if st.playing
                        && let Some(pcm) = st.pcm.clone()
                    {
                        let x = st.pos * pcm.rate as f64;
                        if x >= 0.0 {
                            let i = x.floor() as usize;
                            let t = (x - i as f64) as f32;
                            let s = |c: &Vec<f32>| c.get(i).copied().unwrap_or(0.0) * (1.0 - t) + c.get(i + 1).copied().unwrap_or(0.0) * t;
                            l = pcm.channels.first().map(s).unwrap_or(0.0);
                            r = pcm.channels.get(1).map(s).unwrap_or(l);
                        }
                        st.pos += 1.0 / rate;
                        if st.stop_at.is_some_and(|e| st.pos >= e) || st.pos > pcm.seconds() + 1.0 {
                            st.playing = false;
                        }
                    }
                    if st.muted {
                        l = 0.0;
                        r = 0.0;
                    }
                    for (k, o) in frame.iter_mut().enumerate() {
                        *o = if k % 2 == 0 { l } else { r };
                    }
                }
            };
            let err = |e: cpal::StreamError| log::warn!("sound output: {e}");
            let stream = match cfg.sample_format() {
                cpal::SampleFormat::F32 => dev.build_output_stream(&config, move |b: &mut [f32], _| fill(b), err, None),
                cpal::SampleFormat::I16 => {
                    let mut tmp = Vec::new();
                    dev.build_output_stream(
                        &config,
                        move |b: &mut [i16], _| {
                            tmp.resize(b.len(), 0.0);
                            fill(&mut tmp);
                            for (o, s) in b.iter_mut().zip(&tmp) {
                                *o = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
                            }
                        },
                        err,
                        None,
                    )
                }
                f => return Err(format!("unsupported sound output format {f:?}")),
            }
            .map_err(|e| e.to_string())?;
            stream.play().map_err(|e| e.to_string())?;
            Ok(stream)
        })();
        match result {
            Ok(s) => self.stream = Some(s),
            Err(e) => self.error = e,
        }
    }

    /// Seconds into the sound at a shot frame.
    pub fn seconds_at(frame: f64, offset: i32, fps: u32) -> f64 {
        (frame - offset as f64) / fps.max(1) as f64
    }

    /// Start playing from shot frame `frame`.
    pub fn play(&mut self, frame: f64, offset: i32, fps: u32) {
        if self.pcm.is_none() {
            return;
        }
        self.ensure_stream();
        let mut st = lock(&self.state);
        st.pos = Self::seconds_at(frame, offset, fps);
        st.playing = true;
        st.stop_at = None;
        st.muted = self.muted;
    }

    /// Keep the sound within a short distance of the picture's clock.
    pub fn follow(&mut self, frame: f64, offset: i32, fps: u32) {
        let want = Self::seconds_at(frame, offset, fps);
        let mut st = lock(&self.state);
        st.muted = self.muted;
        if st.playing && (st.pos - want).abs() > 0.12 {
            st.pos = want;
        }
    }

    pub fn stop(&mut self) {
        lock(&self.state).playing = false;
    }

    /// Play the sound of one frame (scrubbing).
    pub fn scrub(&mut self, frame: u32, offset: i32, fps: u32) {
        if self.pcm.is_none() || self.muted {
            return;
        }
        self.ensure_stream();
        let start = Self::seconds_at(frame as f64, offset, fps);
        let mut st = lock(&self.state);
        st.pos = start;
        st.stop_at = Some(start + 1.0 / fps.max(1) as f64);
        st.playing = true;
    }

    #[cfg(test)]
    pub fn is_playing(&self) -> bool {
        lock(&self.state).playing
    }
}
