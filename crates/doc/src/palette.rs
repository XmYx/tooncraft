//! Named colour palettes kept with the shot (ink-and-paint colour models). Hostile input is
//! bounded: at most [`MAX_PALETTES`] palettes of [`MAX_SWATCHES`] colours, names cut to
//! [`MAX_NAME`] characters.

use serde::{Deserialize, Serialize};

pub const MAX_PALETTES: usize = 64;
pub const MAX_SWATCHES: usize = 1024;
pub const MAX_NAME: usize = 80;
/// Most stops one gradient swatch keeps.
pub const MAX_STOPS: usize = 16;

/// One stop of a gradient swatch: where it sits (0 at the start, 1 at the end) and its
/// straight RGBA colour.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stop {
    pub at: f32,
    pub rgba: [u8; 4],
}

/// The colour of `stops` (sorted, at least one) at `t` (0…1), straight RGBA, mixed linearly
/// between the two stops around it; before the first and after the last the end colours hold.
pub fn stop_colour(stops: &[Stop], t: f32) -> [u8; 4] {
    let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
    let (Some(first), Some(last)) = (stops.first(), stops.last()) else { return [0; 4] };
    if t <= first.at {
        return first.rgba;
    }
    for w in stops.windows(2) {
        let (a, b) = (w[0], w[1]);
        if t <= b.at {
            let span = b.at - a.at;
            let f = if span > 1e-6 { (t - a.at) / span } else { 1.0 };
            let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * f).round().clamp(0.0, 255.0) as u8;
            return [mix(a.rgba[0], b.rgba[0]), mix(a.rgba[1], b.rgba[1]), mix(a.rgba[2], b.rgba[2]), mix(a.rgba[3], b.rgba[3])];
        }
    }
    last.rgba
}

/// Gradient stops within the limits: finite positions in 0…1, sorted, at most [`MAX_STOPS`];
/// fewer than two stops is no gradient (empty).
pub fn sanitize_stops(mut stops: Vec<Stop>) -> Vec<Stop> {
    stops.truncate(MAX_STOPS);
    for s in &mut stops {
        s.at = if s.at.is_finite() { s.at.clamp(0.0, 1.0) } else { 0.0 };
    }
    stops.sort_by(|a, b| a.at.total_cmp(&b.at));
    if stops.len() < 2 { Vec::new() } else { stops }
}

fn opaque() -> u8 {
    255
}

fn is_opaque(a: &u8) -> bool {
    *a == 255
}

/// One named colour, with its opacity; or a gradient (its stops, `rgb` the first one's colour).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Swatch {
    #[serde(default)]
    pub name: String,
    pub rgb: [u8; 3],
    /// Opacity (255: opaque). Files from before swatches had one read as opaque.
    #[serde(default = "opaque", skip_serializing_if = "is_opaque")]
    pub alpha: u8,
    /// A gradient swatch's stops (empty: a plain colour).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gradient: Vec<Stop>,
    /// The paint filled from this swatch: when the swatch changes, only this paint follows.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<FillLink>,
}

/// Most fills one swatch remembers (the oldest are forgotten first).
pub const MAX_LINKS: usize = 1024;
/// Most runs of pixels one fill link keeps.
pub const MAX_LINK_RUNS: usize = 200_000;
/// Most runs all the links of all palettes keep together (12 bytes each: about 24 MB). Past it
/// the oldest links are forgotten (see [`sanitize_all`]).
pub const MAX_TOTAL_LINK_RUNS: usize = 2_000_000;

/// Runs kept by every link of `palettes`.
pub fn link_runs(palettes: &[Palette]) -> usize {
    palettes.iter().flat_map(|p| &p.swatches).flat_map(|s| &s.links).map(|l| l.runs.len()).sum()
}

/// The pixels one fill painted in one drawing, as runs along rows: (y, first x, length).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FillLink {
    pub drawing: crate::DrawingId,
    pub runs: Vec<(u32, u32, u32)>,
}

impl FillLink {
    /// The link from a pixel test over a box (x0, y0, width, height); `None` when nothing is in.
    pub fn from_pixels(drawing: crate::DrawingId, (x0, y0, w, h): (u32, u32, u32, u32), inside: impl Fn(u32, u32) -> bool) -> Option<Self> {
        let mut runs = Vec::new();
        for y in y0..y0.saturating_add(h) {
            let mut start: Option<u32> = None;
            for x in x0..=x0.saturating_add(w) {
                let on = x < x0.saturating_add(w) && inside(x, y);
                match (on, start) {
                    (true, None) => start = Some(x),
                    (false, Some(s)) => {
                        if runs.len() < MAX_LINK_RUNS {
                            runs.push((y, s, x - s));
                        }
                        start = None;
                    }
                    _ => {}
                }
            }
        }
        (!runs.is_empty()).then_some(Self { drawing, runs })
    }

    /// Every linked pixel.
    pub fn pixels(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.runs.iter().flat_map(|&(y, x, n)| (x..x.saturating_add(n.min(1 << 20))).map(move |px| (px, y)))
    }
}

impl Swatch {
    /// A plain, opaque colour.
    pub fn colour(name: &str, rgb: [u8; 3]) -> Self {
        Self { name: cut(name), rgb, alpha: 255, gradient: Vec::new(), links: Vec::new() }
    }

    /// The colour as straight RGBA.
    pub fn rgba(&self) -> [u8; 4] {
        [self.rgb[0], self.rgb[1], self.rgb[2], self.alpha]
    }

    pub fn is_gradient(&self) -> bool {
        self.gradient.len() >= 2
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Palette {
    pub name: String,
    #[serde(default)]
    pub swatches: Vec<Swatch>,
}

fn cut(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(MAX_NAME).collect::<String>().trim().to_string()
}

impl Palette {
    pub fn new(name: &str) -> Self {
        Self { name: cut(name), swatches: Vec::new() }.sanitized()
    }

    pub fn sanitized(mut self) -> Self {
        self.name = cut(&self.name);
        if self.name.is_empty() {
            self.name = "Palette".into();
        }
        self.swatches.truncate(MAX_SWATCHES);
        for s in &mut self.swatches {
            s.name = cut(&s.name);
            s.gradient = sanitize_stops(std::mem::take(&mut s.gradient));
            // keep the newest links within the cap
            let extra = s.links.len().saturating_sub(MAX_LINKS);
            s.links = s.links.split_off(extra);
            for l in &mut s.links {
                l.runs.truncate(MAX_LINK_RUNS);
            }
            if let Some(f) = s.gradient.first() {
                s.rgb = [f.rgba[0], f.rgba[1], f.rgba[2]];
                s.alpha = f.rgba[3];
            }
        }
        self
    }

    /// Add a colour (refused past the limit).
    pub fn push(&mut self, name: &str, rgb: [u8; 3]) -> bool {
        if self.swatches.len() >= MAX_SWATCHES {
            return false;
        }
        self.swatches.push(Swatch::colour(name, rgb));
        true
    }

    /// Add a colour with its opacity (refused past the limit).
    pub fn push_rgba(&mut self, name: &str, rgba: [u8; 4]) -> bool {
        if self.swatches.len() >= MAX_SWATCHES {
            return false;
        }
        self.swatches.push(Swatch { alpha: rgba[3], ..Swatch::colour(name, [rgba[0], rgba[1], rgba[2]]) });
        true
    }

    /// Add a gradient swatch (refused past the limit, or with fewer than two usable stops).
    pub fn push_gradient(&mut self, name: &str, stops: Vec<Stop>) -> bool {
        let stops = sanitize_stops(stops);
        let Some(first) = stops.first().copied() else { return false };
        if self.swatches.len() >= MAX_SWATCHES {
            return false;
        }
        let rgb = [first.rgba[0], first.rgba[1], first.rgba[2]];
        self.swatches.push(Swatch { name: cut(name), rgb, alpha: first.rgba[3], gradient: stops, links: Vec::new() });
        true
    }
}

/// A list of palettes within the limits.
pub fn sanitize_all(mut v: Vec<Palette>) -> Vec<Palette> {
    v.truncate(MAX_PALETTES);
    let mut v: Vec<Palette> = v.into_iter().map(Palette::sanitized).collect();
    // within the project-wide budget: each swatch keeps its newest links first; swatches earlier
    // in the palettes are served first
    let mut left = MAX_TOTAL_LINK_RUNS;
    for s in v.iter_mut().flat_map(|p| p.swatches.iter_mut()) {
        let mut keep = 0;
        for l in s.links.iter().rev() {
            if l.runs.len() > left {
                break;
            }
            left -= l.runs.len();
            keep += 1;
        }
        let drop = s.links.len() - keep;
        s.links = s.links.split_off(drop);
    }
    v
}

#[cfg(test)]
mod stop_tests {
    use super::*;

    #[test]
    fn stops_mix_sanitize_and_old_swatches_read_as_opaque() {
        let s = vec![Stop { at: 1.0, rgba: [0, 0, 0, 0] }, Stop { at: 0.0, rgba: [200, 100, 0, 255] }];
        let s = sanitize_stops(s);
        assert_eq!(s[0].at, 0.0, "sorted");
        assert_eq!(stop_colour(&s, 0.5), [100, 50, 0, 128]);
        assert_eq!(stop_colour(&s, f32::NAN), [200, 100, 0, 255]);
        assert_eq!(stop_colour(&s, 9.0), [0, 0, 0, 0]);
        assert!(sanitize_stops(vec![Stop { at: 0.3, rgba: [1; 4] }]).is_empty(), "one stop is no gradient");
        let many = vec![Stop { at: f32::INFINITY, rgba: [1; 4] }; 100];
        assert_eq!(sanitize_stops(many).len(), MAX_STOPS);
        let old: Palette = serde_json::from_str(r#"{"name":"Old","swatches":[{"name":"a","rgb":[1,2,3]}]}"#).unwrap();
        assert_eq!(old.swatches[0].rgba(), [1, 2, 3, 255]);
        assert!(!old.swatches[0].is_gradient());
        let mut p = Palette::new("New");
        assert!(p.push_rgba("glass", [9, 9, 9, 40]));
        assert!(!p.push_gradient("bad", vec![Stop { at: 0.0, rgba: [1; 4] }]));
        assert!(p.push_gradient("g", vec![Stop { at: 0.0, rgba: [1, 2, 3, 4] }, Stop { at: 1.0, rgba: [5; 4] }]));
        let json = serde_json::to_string(&p).unwrap();
        let back: Palette = serde_json::from_str::<Palette>(&json).unwrap().sanitized();
        assert_eq!(back, p);
        assert_eq!(back.swatches[1].rgba(), [1, 2, 3, 4], "a gradient's colour is its first stop");
        let plain = serde_json::to_string(&Swatch::colour("x", [1, 2, 3])).unwrap();
        assert!(!plain.contains("alpha") && !plain.contains("gradient"), "{plain}");
    }
}
