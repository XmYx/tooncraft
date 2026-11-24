//! Lip-sync assist: mouth shapes from a voice recording, with no network and no trained model.
//!
//! The signal is cut into one analysis window per animation frame. For each window we measure the
//! loudness (RMS), the zero-crossing rate, how much energy sits above 3 kHz, and the first two
//! vocal-tract resonances (formants F1 and F2) as the strongest peaks of a smoothed spectrum in
//! their usual ranges. The shape follows from rules drawn from textbook phonetics (open vowels
//! have a high F1, front vowels a high F2, rounded vowels a low F2; fricatives are noisy and
//! bright; a short dip in loudness between voiced frames is a lip closure). The result is held
//! for at least `min_hold` frames, the way mouths are animated on twos, and is meant as a first
//! pass the animator edits.
//!
//! The shapes are the usual small mouth chart of hand-drawn animation (nine drawings).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

mod fft;
pub mod sheet;
pub mod text;

use serde::{Deserialize, Serialize};

/// A mouth drawing of the chart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mouth {
    /// Closed and relaxed (silence).
    Rest,
    /// Lips pressed together (M, B, P).
    Closed,
    /// Wide open (A, I).
    Open,
    /// Open and wide, teeth showing (E).
    Wide,
    /// Round and open (O).
    Round,
    /// Small and pushed forward (U, W, Q).
    Pucker,
    /// Upper teeth on the lower lip (F, V).
    Teeth,
    /// Tongue behind the teeth (L, TH).
    Tongue,
    /// Slightly open, for the other consonants (C, D, G, K, N, R, S, T …).
    Consonant,
}

impl Mouth {
    pub const ALL: [Mouth; 9] =
        [Mouth::Rest, Mouth::Closed, Mouth::Open, Mouth::Wide, Mouth::Round, Mouth::Pucker, Mouth::Teeth, Mouth::Tongue, Mouth::Consonant];

    /// The name in timing sheets (`rest`, `closed` …).
    pub fn id(self) -> &'static str {
        match self {
            Mouth::Rest => "rest",
            Mouth::Closed => "closed",
            Mouth::Open => "open",
            Mouth::Wide => "wide",
            Mouth::Round => "round",
            Mouth::Pucker => "pucker",
            Mouth::Teeth => "teeth",
            Mouth::Tongue => "tongue",
            Mouth::Consonant => "consonant",
        }
    }

    /// Our name, or one of the phoneme-group names common in lip-sync sheets
    /// (`AI`, `E`, `O`, `U`, `WQ`, `MBP`, `FV`, `L`, `etc`, `rest`), case-insensitive.
    pub fn parse(s: &str) -> Option<Mouth> {
        let s = s.trim().to_ascii_lowercase();
        Some(match s.as_str() {
            "rest" | "x" => Mouth::Rest,
            "closed" | "mbp" | "m" | "b" | "p" => Mouth::Closed,
            "open" | "ai" | "a" | "i" => Mouth::Open,
            "wide" | "e" => Mouth::Wide,
            "round" | "o" => Mouth::Round,
            "pucker" | "u" | "wq" | "w" | "q" => Mouth::Pucker,
            "teeth" | "fv" | "f" | "v" => Mouth::Teeth,
            "tongue" | "l" | "th" => Mouth::Tongue,
            "consonant" | "etc" => Mouth::Consonant,
            _ => return None,
        })
    }
}

/// Analysis settings.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Loudness, relative to the loudest frame, below which a frame is silent (0…1).
    pub silence: f32,
    /// Hold every mouth at least this many frames (1 = on ones, 2 = on twos).
    pub min_hold: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self { silence: 0.08, min_hold: 2 }
    }
}

impl Settings {
    pub fn sanitized(self) -> Settings {
        Settings { silence: if self.silence.is_finite() { self.silence.clamp(0.0, 1.0) } else { 0.08 }, min_hold: self.min_hold.clamp(1, 12) }
    }
}

/// What was measured in one frame's window.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Features {
    pub rms: f32,
    pub zcr: f32,
    /// Share of the energy above 3 kHz.
    pub bright: f32,
    pub f1: f32,
    pub f2: f32,
}

/// Longest signal analysed: one hour at 48 kHz.
pub const MAX_SAMPLES: usize = 48_000 * 3600;
const WINDOW: usize = 1024;

/// Measure each frame of `samples` (mono, `rate` Hz) at `fps` frames per second.
pub fn features(samples: &[f32], rate: u32, fps: u32) -> Vec<Features> {
    let (rate, fps) = (rate.max(1) as f32, fps.clamp(1, 240) as f32);
    let samples = &samples[..samples.len().min(MAX_SAMPLES)];
    let frames = (samples.len() as f32 / rate * fps).ceil() as usize;
    let hann: Vec<f32> = (0..WINDOW).map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / WINDOW as f32).cos()).collect();
    let bin_hz = rate / WINDOW as f32;
    let mut out = Vec::with_capacity(frames);
    let mut re = vec![0f32; WINDOW];
    let mut im = vec![0f32; WINDOW];
    for f in 0..frames {
        let centre = ((f as f32 + 0.5) / fps * rate) as usize;
        let start = centre.saturating_sub(WINDOW / 2);
        let win: Vec<f32> = (0..WINDOW).map(|i| samples.get(start + i).copied().filter(|v| v.is_finite()).unwrap_or(0.0)).collect();
        // loudness over the frame itself (not the whole window)
        let span = (rate / fps) as usize;
        let a = ((f as f32 / fps) * rate) as usize;
        let seg = samples.get(a..(a + span).min(samples.len())).unwrap_or(&[]);
        let rms = if seg.is_empty() { 0.0 } else { (seg.iter().map(|v| if v.is_finite() { v * v } else { 0.0 }).sum::<f32>() / seg.len() as f32).sqrt() };
        let zcr = if seg.len() < 2 { 0.0 } else { seg.windows(2).filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0)).count() as f32 / seg.len() as f32 };
        for i in 0..WINDOW {
            re[i] = win[i] * hann[i];
            im[i] = 0.0;
        }
        fft::fft(&mut re, &mut im);
        let mag: Vec<f32> = (0..WINDOW / 2).map(|i| (re[i] * re[i] + im[i] * im[i]).sqrt()).collect();
        let total: f32 = mag.iter().map(|m| m * m).sum::<f32>().max(1e-12);
        let hi = ((3000.0 / bin_hz) as usize).min(mag.len());
        let bright = mag.get(hi..).unwrap_or(&[]).iter().map(|m| m * m).sum::<f32>() / total;
        // smooth over ~180 Hz so single harmonics do not count as formants
        let k = ((180.0 / bin_hz) as usize).max(1);
        let smooth: Vec<f32> = (0..mag.len()).map(|i| mag[i.saturating_sub(k)..(i + k + 1).min(mag.len())].iter().sum::<f32>()).collect();
        let peak = |lo: f32, hi: f32| -> f32 {
            let (a, b) = (((lo / bin_hz) as usize).min(smooth.len()), ((hi / bin_hz) as usize).min(smooth.len()));
            smooth.get(a..b).unwrap_or(&[]).iter().enumerate().fold((0usize, 0f32), |best, (i, v)| if *v > best.1 { (i, *v) } else { best }).0 as f32 * bin_hz
                + lo
        };
        let f1 = peak(250.0, 1000.0);
        let f2 = peak((f1 + 350.0).max(800.0), 2900.0);
        out.push(Features { rms, zcr, bright, f1, f2 });
    }
    out
}

/// The mouth for one frame's features (`loud` is the shot's loudest frame).
pub fn classify(x: &Features, loud: f32, s: &Settings) -> Mouth {
    if loud <= 0.0 || x.rms < loud * s.silence {
        return Mouth::Rest;
    }
    // noisy and bright: a fricative
    if x.zcr > 0.25 && x.bright > 0.35 {
        return if x.rms < loud * 0.35 { Mouth::Teeth } else { Mouth::Consonant };
    }
    if x.bright > 0.5 {
        return Mouth::Consonant;
    }
    match (x.f1, x.f2) {
        (f1, _) if f1 >= 650.0 => Mouth::Open,
        (f1, f2) if f2 >= 1700.0 && f1 < 650.0 => Mouth::Wide,
        (f1, f2) if f2 < 1150.0 && f1 < 420.0 => Mouth::Pucker,
        (_, f2) if f2 < 1300.0 => Mouth::Round,
        _ => Mouth::Tongue,
    }
}

/// Mouths for every frame of a voice recording.
pub fn detect(samples: &[f32], rate: u32, fps: u32, s: Settings) -> Vec<Mouth> {
    let s = s.sanitized();
    let feats = features(samples, rate, fps);
    let loud = feats.iter().map(|f| f.rms).fold(0.0f32, f32::max);
    let mut m: Vec<Mouth> = feats.iter().map(|f| classify(f, loud, &s)).collect();
    // a short quiet dip between voiced frames is a lip closure: below the silence level, or
    // much quieter than both neighbours
    for i in 1..m.len().saturating_sub(1) {
        let r = |j: usize| feats.get(j).map_or(0.0, |f| f.rms);
        if m[i] != Mouth::Rest && r(i) < 0.3 * r(i - 1).min(r(i + 1)) {
            m[i] = Mouth::Rest;
        }
    }
    for i in 1..m.len().saturating_sub(1) {
        let voiced = |j: usize| m.get(j).is_some_and(|x| !matches!(x, Mouth::Rest | Mouth::Closed));
        if m[i] == Mouth::Rest && voiced(i - 1) && (voiced(i + 1) || (m.get(i + 1) == Some(&Mouth::Rest) && voiced(i + 2))) {
            m[i] = Mouth::Closed;
        }
    }
    hold(&mut m, s.min_hold);
    m
}

/// Make every run at least `min` frames long by extending it over the following frames.
pub fn hold(m: &mut [Mouth], min: u32) {
    let min = min.max(1) as usize;
    let mut i = 0;
    while i < m.len() {
        let v = m[i];
        let mut j = i + 1;
        while j < m.len() && m[j] == v {
            j += 1;
        }
        if j - i < min {
            let end = (i + min).min(m.len());
            for x in m.iter_mut().take(end).skip(j) {
                *x = v;
            }
            j = end;
        }
        i = j;
    }
}

/// The frames where the mouth changes, with the new mouth (the first frame always).
pub fn changes(m: &[Mouth]) -> Vec<(u32, Mouth)> {
    let mut out = Vec::new();
    let mut last = None;
    for (f, v) in m.iter().enumerate() {
        if last != Some(*v) {
            out.push((f as u32, *v));
            last = Some(*v);
        }
    }
    out
}

#[cfg(test)]
mod tests;
