use proptest::prelude::*;

use super::*;

const RATE: u32 = 22_050;

/// A vowel-like sound: a 120 Hz voice whose harmonics are shaped by two resonances.
fn vowel(f1: f32, f2: f32, secs: f32, amp: f32) -> Vec<f32> {
    let n = (RATE as f32 * secs) as usize;
    let f0 = 120.0;
    let gain = |f: f32| 1.0 / (1.0 + ((f - f1) / 90.0).powi(2)) + 0.7 / (1.0 + ((f - f2) / 110.0).powi(2));
    let harmonics: Vec<(f32, f32)> = (1..34).map(|k| (k as f32 * f0, gain(k as f32 * f0))).collect();
    let norm: f32 = harmonics.iter().map(|(_, g)| g).sum();
    (0..n).map(|i| harmonics.iter().map(|(f, g)| g * (std::f32::consts::TAU * f * i as f32 / RATE as f32).sin()).sum::<f32>() / norm * amp).collect()
}

fn noise(secs: f32, amp: f32) -> Vec<f32> {
    let mut s = 12345u32;
    (0..(RATE as f32 * secs) as usize)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            (s as f32 / u32::MAX as f32 * 2.0 - 1.0) * amp
        })
        .collect()
}

fn most(m: &[Mouth]) -> Mouth {
    let mut best = (Mouth::Rest, 0);
    for v in Mouth::ALL {
        let n = m.iter().filter(|x| **x == v).count();
        if n > best.1 {
            best = (v, n);
        }
    }
    best.0
}

#[test]
fn vowels_map_to_their_mouths() {
    for (f1, f2, want) in [(800.0, 1250.0, Mouth::Open), (300.0, 2300.0, Mouth::Wide), (320.0, 800.0, Mouth::Pucker), (520.0, 900.0, Mouth::Round)] {
        let s = vowel(f1, f2, 0.6, 0.5);
        let m = detect(&s, RATE, 24, Settings { min_hold: 1, ..Default::default() });
        // ignore the first and last frame (half-empty windows)
        assert_eq!(most(&m[1..m.len() - 1]), want, "F1 {f1}, F2 {f2}: {m:?}");
    }
}

#[test]
fn silence_noise_and_closures() {
    let quiet = vec![0.0; RATE as usize / 2];
    assert!(detect(&quiet, RATE, 24, Settings::default()).iter().all(|m| *m == Mouth::Rest));
    let mut s = vowel(800.0, 1250.0, 0.3, 0.5);
    s.extend(noise(0.3, 0.12));
    let m = detect(&s, RATE, 24, Settings { min_hold: 1, ..Default::default() });
    assert!(matches!(most(&m[9..m.len() - 1]), Mouth::Teeth | Mouth::Consonant), "a hiss is a fricative: {m:?}");
    // vowel, a short gap, vowel: the gap is a lip closure
    let mut s = vowel(800.0, 1250.0, 0.3, 0.5);
    s.extend(vec![0.0; (RATE as f32 * 0.08) as usize]);
    s.extend(vowel(800.0, 1250.0, 0.3, 0.5));
    let m = detect(&s, RATE, 24, Settings { min_hold: 1, ..Default::default() });
    assert!(m.contains(&Mouth::Closed), "{m:?}");
    assert!(!m[2..m.len() - 2].contains(&Mouth::Rest), "no rest inside the word: {m:?}");
}

#[test]
fn holds_and_changes() {
    let mut m = vec![Mouth::Open, Mouth::Wide, Mouth::Wide, Mouth::Wide, Mouth::Round, Mouth::Rest, Mouth::Rest];
    hold(&mut m, 2);
    assert_eq!(m, vec![Mouth::Open, Mouth::Open, Mouth::Wide, Mouth::Wide, Mouth::Round, Mouth::Round, Mouth::Rest]);
    assert_eq!(changes(&m), vec![(0, Mouth::Open), (2, Mouth::Wide), (4, Mouth::Round), (6, Mouth::Rest)]);
    for v in Mouth::ALL {
        assert_eq!(Mouth::parse(v.id()), Some(v));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    /// Any samples (NaN and huge included), any rate and frame rate: one mouth per frame, no panic.
    #[test]
    fn hostile_input(samples in proptest::collection::vec(prop_oneof![Just(f32::NAN), Just(f32::INFINITY), -1e9f32..1e9, -1.0f32..1.0], 0..6000), rate in 0u32..200_000, fps in 0u32..500, hold_frames in 0u32..40, silence in -2.0f32..2.0) {
        let m = detect(&samples, rate, fps, Settings { silence, min_hold: hold_frames });
        let frames = (samples.len().min(MAX_SAMPLES) as f32 / rate.max(1) as f32 * fps.clamp(1, 240) as f32).ceil() as usize;
        prop_assert_eq!(m.len(), frames);
    }
}
