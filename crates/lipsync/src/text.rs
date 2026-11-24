//! Lip-sync from the typed line of dialogue: the words turned into mouth shapes by spelling
//! rules, then laid over the voiced frames of the recording in order.
//!
//! English spelling is a loose guide to sound, so the rules are kept small and readable:
//! letter pairs first (`th`, `sh`, `ch`, `oo`, `ee`, `ou`, `ow`, `ph`, `wh`, `qu` …), then single
//! letters, with doubled letters and silent final `e` dropped. A shape repeated inside a word is
//! said once. Between words the mouth rests only where the recording is silent, the way people
//! run words together.
//!
//! Timing: the frames louder than the silence level are the voiced frames. The shapes are spread
//! over them in order, each getting an equal share (at least one frame). The words follow one
//! another across pauses, and silent frames rest. Without a recording, [`from_text_even`] gives
//! each shape a fixed number of frames. Either way the result is held on twos (or the chosen
//! hold) and is a first pass the animator edits.

use crate::{Mouth, Settings, features, hold};

/// Longest line of dialogue read, in characters.
pub const MAX_CHARS: usize = 20_000;

/// The mouth shapes of one word, by spelling rules.
pub fn word_mouths(word: &str) -> Vec<Mouth> {
    let w: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).flat_map(char::to_lowercase).collect();
    let mut out: Vec<Mouth> = Vec::new();
    let mut i = 0;
    while i < w.len() {
        let next = w.get(i + 1).copied();
        let pair = next.map(|n| (w[i], n));
        let (m, used) = match pair {
            Some(('t', 'h')) => (Some(Mouth::Tongue), 2),
            Some(('s' | 'c', 'h')) => (Some(Mouth::Consonant), 2),
            Some(('p', 'h')) => (Some(Mouth::Teeth), 2),
            Some(('w', 'h')) => (Some(Mouth::Pucker), 2),
            Some(('q', 'u')) => (Some(Mouth::Pucker), 2),
            Some(('o', 'o')) => (Some(Mouth::Pucker), 2),
            Some(('e', 'e' | 'a')) => (Some(Mouth::Wide), 2),
            Some(('o', 'u' | 'w')) => (Some(Mouth::Round), 2),
            Some(('a', 'i' | 'y')) => (Some(Mouth::Wide), 2),
            Some(('n', 'g')) => (Some(Mouth::Consonant), 2),
            Some((a, b)) if a == b && !"aeiou".contains(a) => (None, 1),
            _ => (letter(w[i], i + 1 == w.len(), w.len()), 1),
        };
        if let Some(m) = m
            && out.last() != Some(&m)
        {
            out.push(m);
        }
        i += used;
    }
    out
}

fn letter(c: char, last: bool, len: usize) -> Option<Mouth> {
    Some(match c {
        'a' | 'i' => Mouth::Open,
        // a silent final e ("time", "make"), but not in short words ("me", "the")
        'e' if last && len > 3 => return None,
        'e' | 'y' => Mouth::Wide,
        'o' => Mouth::Round,
        'u' | 'w' | 'q' => Mouth::Pucker,
        'm' | 'b' | 'p' => Mouth::Closed,
        'f' | 'v' => Mouth::Teeth,
        'l' => Mouth::Tongue,
        'h' => return None,
        c if c.is_alphabetic() => Mouth::Consonant,
        _ => return None,
    })
}

/// The shapes of a line, word after word (at most [`MAX_CHARS`] characters are read).
pub fn line_mouths(text: &str) -> Vec<Mouth> {
    let text: String = text.chars().take(MAX_CHARS).collect();
    let mut out: Vec<Mouth> = Vec::new();
    for word in text.split(|c: char| !c.is_alphabetic() && c != '\'') {
        for m in word_mouths(word) {
            if out.last() != Some(&m) {
                out.push(m);
            }
        }
    }
    out
}

/// One mouth per frame: `text`'s shapes laid over the voiced frames of `samples` (mono, `rate`
/// Hz) at `fps`, silent frames at rest.
pub fn from_text(text: &str, samples: &[f32], rate: u32, fps: u32, s: Settings) -> Vec<Mouth> {
    let s = s.sanitized();
    let shapes = line_mouths(text);
    let feats = features(samples, rate, fps);
    let loud = feats.iter().map(|f| f.rms).fold(0.0f32, f32::max);
    let voiced: Vec<usize> = feats.iter().enumerate().filter(|(_, f)| loud > 0.0 && f.rms >= s.silence * loud).map(|(i, _)| i).collect();
    let mut m = vec![Mouth::Rest; feats.len()];
    if shapes.is_empty() || voiced.is_empty() {
        return m;
    }
    // shape k covers the voiced frames from k·V/S to (k+1)·V/S; with more shapes than frames
    // some shapes get no frame of their own
    let (v, n) = (voiced.len(), shapes.len());
    for (j, &f) in voiced.iter().enumerate() {
        let k = (j * n / v).min(n - 1);
        m[f] = shapes[k];
    }
    hold(&mut m, s.min_hold);
    m
}

/// One mouth per frame without a recording: `per_shape` frames for each shape of `text`, then
/// rest.
pub fn from_text_even(text: &str, per_shape: u32) -> Vec<Mouth> {
    let per = per_shape.clamp(1, 48) as usize;
    let mut m: Vec<Mouth> = line_mouths(text).into_iter().flat_map(|x| std::iter::repeat_n(x, per)).collect();
    m.push(Mouth::Rest);
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use Mouth::*;

    #[test]
    fn words_become_shapes() {
        assert_eq!(word_mouths("map"), [Closed, Open, Closed]);
        assert_eq!(word_mouths("Hello"), [Wide, Tongue, Round], "h is silent, ll said once");
        assert_eq!(word_mouths("the"), [Tongue, Wide]);
        assert_eq!(word_mouths("time"), [Consonant, Open, Closed], "silent final e");
        assert_eq!(word_mouths("food"), [Teeth, Pucker, Consonant]);
        assert_eq!(word_mouths("photo"), [Teeth, Round, Consonant, Round]);
        assert_eq!(word_mouths("queen"), [Pucker, Wide, Consonant]);
        assert_eq!(word_mouths("123"), Vec::<Mouth>::new());
        assert_eq!(line_mouths("Oh, hello!"), [Round, Wide, Tongue, Round]);
    }

    fn tone(len: usize, rate: u32, on: impl Fn(usize) -> bool) -> Vec<f32> {
        (0..len).map(|i| if on(i) { (i as f32 * 2.0 * std::f32::consts::PI * 220.0 / rate as f32).sin() * 0.5 } else { 0.0 }).collect()
    }

    #[test]
    fn shapes_follow_the_voice_and_rest_in_silence() {
        let rate = 16_000;
        // 2 s at 12 fps: voice from 0.5 s to 1.5 s
        let samples = tone(2 * rate as usize, rate, |i| (rate as usize / 2..3 * rate as usize / 2).contains(&i));
        let m = from_text("mama", &samples, rate, 12, Settings::default());
        assert_eq!(m.len(), 24);
        assert!(m[..5].iter().all(|x| *x == Rest), "silent start rests: {m:?}");
        assert!(m[19..].iter().all(|x| *x == Rest), "silent end rests: {m:?}");
        let spoken: Vec<Mouth> = crate::changes(&m[6..18]).into_iter().map(|(_, x)| x).collect();
        assert_eq!(spoken, [Closed, Open, Closed, Open], "in order over the voice: {m:?}");
    }

    #[test]
    fn without_sound_or_words_nothing_breaks() {
        assert_eq!(from_text_even("map", 2), [Closed, Closed, Open, Open, Closed, Closed, Rest]);
        assert!(from_text("", &[0.0; 16_000], 16_000, 12, Settings::default()).iter().all(|x| *x == Rest));
        assert!(from_text("hello", &[], 16_000, 12, Settings::default()).is_empty());
        let long = "word ".repeat(10_000);
        assert!(line_mouths(&long).len() <= MAX_CHARS);
    }

    proptest::proptest! {
        #[test]
        fn any_text_gives_shapes_without_panicking(t in "\\PC{0,300}", per in 0u32..100) {
            let _ = line_mouths(&t);
            let _ = from_text_even(&t, per);
            let _ = from_text(&t, &[0.1, -0.1, 0.2], 8000, 24, Settings::default());
        }
    }
}
