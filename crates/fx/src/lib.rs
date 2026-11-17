//! Layer effects for ToonCraft (UI-free, no other ToonCraft dependency than the raster types).
//!
//! An [`Effect`] is one entry of a layer's effect stack: a kind with its parameters
//! ([`EffectKind`]), an on/off switch and optional keys. With keys, the parameters at a frame are
//! interpolated linearly between the surrounding keys (held before the first and after the last);
//! without keys the effect's own parameters apply to every frame.
//!
//! [`apply_stack`] runs a stack over a premultiplied RGBA8 image, top of the list first. Every
//! effect is a CPU routine over flat rows (sliding-window box blurs, a linear-time distance
//! transform, per-pixel lookup tables), split over the available cores; the results are
//! deterministic, so exports and playback show the same pixels.
//!
//! Effect parameters come from files and command lines, so they are hostile: [`Effect::sanitized`]
//! clamps every number to a documented range and drops keys of another kind.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

mod blur;
mod colour;
mod distance;
mod par;
mod shape;

use serde::{Deserialize, Serialize};
use tooncraft_raster::{BlendMode, RgbaImage};

pub use blur::gaussian;
pub use par::set_max_threads;

/// Most effects on one layer.
pub const MAX_EFFECTS: usize = 32;
/// Most keys on one effect.
pub const MAX_KEYS: usize = 4096;
/// Most points on a curve.
pub const MAX_CURVE_POINTS: usize = 32;
/// Largest blur, glow and light-wrap radius, pixels.
pub const MAX_RADIUS: f32 = 200.0;

/// The shape of a feathered mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaskShape {
    #[default]
    Rectangle,
    Ellipse,
}

/// An effect and its parameters. Distances are canvas pixels, colours straight RGB, amounts 0…1
/// unless noted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EffectKind {
    /// Soft blur (Gaussian, `radius` is its standard deviation).
    Blur { radius: f32 },
    /// Blur along one direction (motion streaks): `length` pixels at `angle` degrees.
    DirectionalBlur { length: f32, angle: f32 },
    /// Zoom blur towards a centre given as a fraction of the canvas (0.5, 0.5 is the middle).
    RadialBlur { amount: f32, centre_x: f32, centre_y: f32 },
    /// A blurred, tinted copy added on top: light from the drawing. `intensity` 0…5.
    Glow { radius: f32, intensity: f32, colour: [u8; 3] },
    /// A blurred, offset silhouette under the drawing.
    DropShadow { dx: f32, dy: f32, blur: f32, colour: [u8; 3], opacity: f32 },
    /// A solid outline `width` pixels around the drawing, under it.
    Outline { width: f32, colour: [u8; 3] },
    /// Grow (positive) or thin (negative) lines by `amount` pixels, keeping their colours.
    Thicken { amount: f32 },
    /// Input black / white points, gamma (0.1…10) and output black / white points.
    Levels { in_black: f32, in_white: f32, gamma: f32, out_black: f32, out_white: f32 },
    /// A smooth tone curve through `points` (x, y in 0…1), applied to red, green and blue.
    Curves { points: Vec<[f32; 2]> },
    /// Hue shift in degrees, saturation and lightness changes in −1…1.
    HueSaturation { hue: f32, saturation: f32, lightness: f32 },
    /// Mix the colours towards `colour` by `amount`.
    Tint { colour: [u8; 3], amount: f32 },
    /// Film grain on the drawn pixels: `amount`, grain `size` 1…16 px, new grain every frame
    /// when `animated`.
    Grain { amount: f32, size: f32, seed: u32, animated: bool },
    /// Let the light of the layers below bleed into the drawing's edges.
    LightWrap { radius: f32, intensity: f32 },
    /// Hard edges: alpha at or above `threshold` becomes opaque, the rest transparent.
    AlphaThreshold { threshold: f32 },
    /// Keep only what is inside a rectangle or ellipse centred at `x`, `y` (canvas pixels), with
    /// a soft edge `feather` pixels wide; `invert` keeps the outside.
    Mask { shape: MaskShape, x: f32, y: f32, width: f32, height: f32, feather: f32, invert: bool },
    /// Make pixels close to `colour` transparent (scanned paper, coloured backdrops): within
    /// `tolerance` (0…1 colour distance) they go, within `tolerance + softness` they fade.
    ColourKey { colour: [u8; 3], tolerance: f32, softness: f32 },
}

impl EffectKind {
    /// Every kind with useful starting parameters for a canvas of `w`×`h` pixels.
    pub fn catalogue(w: u32, h: u32) -> Vec<EffectKind> {
        let (w, h) = (w as f32, h as f32);
        vec![
            EffectKind::Blur { radius: 4.0 },
            EffectKind::DirectionalBlur { length: 24.0, angle: 0.0 },
            EffectKind::RadialBlur { amount: 0.15, centre_x: 0.5, centre_y: 0.5 },
            EffectKind::Glow { radius: 12.0, intensity: 1.0, colour: [255, 240, 200] },
            EffectKind::DropShadow { dx: 8.0, dy: 8.0, blur: 6.0, colour: [0, 0, 0], opacity: 0.5 },
            EffectKind::Outline { width: 3.0, colour: [255, 255, 255] },
            EffectKind::Thicken { amount: 1.0 },
            EffectKind::Levels { in_black: 0.0, in_white: 1.0, gamma: 1.0, out_black: 0.0, out_white: 1.0 },
            EffectKind::Curves { points: vec![[0.0, 0.0], [0.25, 0.2], [0.75, 0.8], [1.0, 1.0]] },
            EffectKind::HueSaturation { hue: 0.0, saturation: 0.0, lightness: 0.0 },
            EffectKind::Tint { colour: [40, 60, 140], amount: 0.3 },
            EffectKind::Grain { amount: 0.15, size: 1.0, seed: 1, animated: true },
            EffectKind::LightWrap { radius: 10.0, intensity: 0.6 },
            EffectKind::AlphaThreshold { threshold: 0.5 },
            EffectKind::Mask { shape: MaskShape::Ellipse, x: w / 2.0, y: h / 2.0, width: w * 0.6, height: h * 0.6, feather: 40.0, invert: false },
            EffectKind::ColourKey { colour: [255, 255, 255], tolerance: 0.1, softness: 0.1 },
        ]
    }

    /// A stable machine name (`blur`, `drop_shadow` …), as in the JSON.
    pub fn id(&self) -> &'static str {
        match self {
            EffectKind::Blur { .. } => "blur",
            EffectKind::DirectionalBlur { .. } => "directional_blur",
            EffectKind::RadialBlur { .. } => "radial_blur",
            EffectKind::Glow { .. } => "glow",
            EffectKind::DropShadow { .. } => "drop_shadow",
            EffectKind::Outline { .. } => "outline",
            EffectKind::Thicken { .. } => "thicken",
            EffectKind::Levels { .. } => "levels",
            EffectKind::Curves { .. } => "curves",
            EffectKind::HueSaturation { .. } => "hue_saturation",
            EffectKind::Tint { .. } => "tint",
            EffectKind::Grain { .. } => "grain",
            EffectKind::LightWrap { .. } => "light_wrap",
            EffectKind::AlphaThreshold { .. } => "alpha_threshold",
            EffectKind::Mask { .. } => "mask",
            EffectKind::ColourKey { .. } => "colour_key",
        }
    }

    /// Same kind (parameters may differ)?
    pub fn same_kind(&self, other: &EffectKind) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    /// Does this effect read the layers below it (light wrap)?
    pub fn needs_below(&self) -> bool {
        matches!(self, EffectKind::LightWrap { .. })
    }

    /// Every number clamped to its range (NaN becomes the range's low end or a neutral value).
    pub fn sanitized(self) -> EffectKind {
        use EffectKind::*;
        match self {
            Blur { radius } => Blur { radius: c(radius, 0.0, MAX_RADIUS, 0.0) },
            DirectionalBlur { length, angle } => DirectionalBlur { length: c(length, 0.0, 500.0, 0.0), angle: c(angle, -3600.0, 3600.0, 0.0) },
            RadialBlur { amount, centre_x, centre_y } => {
                RadialBlur { amount: c(amount, 0.0, 1.0, 0.0), centre_x: c(centre_x, -1.0, 2.0, 0.5), centre_y: c(centre_y, -1.0, 2.0, 0.5) }
            }
            Glow { radius, intensity, colour } => Glow { radius: c(radius, 0.0, MAX_RADIUS, 0.0), intensity: c(intensity, 0.0, 5.0, 0.0), colour },
            DropShadow { dx, dy, blur, colour, opacity } => DropShadow {
                dx: c(dx, -4096.0, 4096.0, 0.0),
                dy: c(dy, -4096.0, 4096.0, 0.0),
                blur: c(blur, 0.0, MAX_RADIUS, 0.0),
                colour,
                opacity: c(opacity, 0.0, 1.0, 0.0),
            },
            Outline { width, colour } => Outline { width: c(width, 0.0, 100.0, 0.0), colour },
            Thicken { amount } => Thicken { amount: c(amount, -50.0, 50.0, 0.0) },
            Levels { in_black, in_white, gamma, out_black, out_white } => {
                let in_black = c(in_black, 0.0, 1.0, 0.0);
                let in_white = c(in_white, 0.0, 1.0, 1.0).max(in_black + 1.0 / 255.0);
                Levels { in_black, in_white, gamma: c(gamma, 0.1, 10.0, 1.0), out_black: c(out_black, 0.0, 1.0, 0.0), out_white: c(out_white, 0.0, 1.0, 1.0) }
            }
            Curves { points } => {
                let mut p: Vec<[f32; 2]> = points.into_iter().take(MAX_CURVE_POINTS).map(|[x, y]| [c(x, 0.0, 1.0, 0.0), c(y, 0.0, 1.0, 0.0)]).collect();
                p.sort_by(|a, b| a[0].total_cmp(&b[0]));
                p.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-4);
                if p.len() < 2 {
                    p = vec![[0.0, 0.0], [1.0, 1.0]];
                }
                Curves { points: p }
            }
            HueSaturation { hue, saturation, lightness } => {
                HueSaturation { hue: c(hue, -180.0, 180.0, 0.0), saturation: c(saturation, -1.0, 1.0, 0.0), lightness: c(lightness, -1.0, 1.0, 0.0) }
            }
            Tint { colour, amount } => Tint { colour, amount: c(amount, 0.0, 1.0, 0.0) },
            Grain { amount, size, seed, animated } => Grain { amount: c(amount, 0.0, 1.0, 0.0), size: c(size, 1.0, 16.0, 1.0), seed, animated },
            LightWrap { radius, intensity } => LightWrap { radius: c(radius, 0.0, MAX_RADIUS, 0.0), intensity: c(intensity, 0.0, 2.0, 0.0) },
            AlphaThreshold { threshold } => AlphaThreshold { threshold: c(threshold, 0.0, 1.0, 0.5) },
            Mask { shape, x, y, width, height, feather, invert } => Mask {
                shape,
                x: c(x, -65536.0, 65536.0, 0.0),
                y: c(y, -65536.0, 65536.0, 0.0),
                width: c(width, 0.0, 65536.0, 0.0),
                height: c(height, 0.0, 65536.0, 0.0),
                feather: c(feather, 0.0, 4096.0, 0.0),
                invert,
            },
            ColourKey { colour, tolerance, softness } => ColourKey { colour, tolerance: c(tolerance, 0.0, 1.0, 0.0), softness: c(softness, 0.0, 1.0, 0.0) },
        }
    }

    /// Parameters `t` of the way from `self` to `to` (0…1). Numbers and colours interpolate;
    /// switches, seeds and shapes hold `self`'s until `to` is reached. Another kind: `self`.
    pub fn lerp(&self, to: &EffectKind, t: f32) -> EffectKind {
        use EffectKind::*;
        let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
        if t >= 1.0 && self.same_kind(to) {
            return to.clone();
        }
        let l = |a: f32, b: f32| a + (b - a) * t;
        let lc = |a: [u8; 3], b: [u8; 3]| -> [u8; 3] { [0, 1, 2].map(|i| l(a[i] as f32, b[i] as f32).round().clamp(0.0, 255.0) as u8) };
        match (self, to) {
            (Blur { radius: a }, Blur { radius: b }) => Blur { radius: l(*a, *b) },
            (DirectionalBlur { length: a, angle: aa }, DirectionalBlur { length: b, angle: ba }) => DirectionalBlur { length: l(*a, *b), angle: l(*aa, *ba) },
            (RadialBlur { amount: a, centre_x: ax, centre_y: ay }, RadialBlur { amount: b, centre_x: bx, centre_y: by }) => {
                RadialBlur { amount: l(*a, *b), centre_x: l(*ax, *bx), centre_y: l(*ay, *by) }
            }
            (Glow { radius: ar, intensity: ai, colour: ac }, Glow { radius: br, intensity: bi, colour: bc }) => {
                Glow { radius: l(*ar, *br), intensity: l(*ai, *bi), colour: lc(*ac, *bc) }
            }
            (DropShadow { dx: a1, dy: a2, blur: a3, colour: ac, opacity: a4 }, DropShadow { dx: b1, dy: b2, blur: b3, colour: bc, opacity: b4 }) => {
                DropShadow { dx: l(*a1, *b1), dy: l(*a2, *b2), blur: l(*a3, *b3), colour: lc(*ac, *bc), opacity: l(*a4, *b4) }
            }
            (Outline { width: a, colour: ac }, Outline { width: b, colour: bc }) => Outline { width: l(*a, *b), colour: lc(*ac, *bc) },
            (Thicken { amount: a }, Thicken { amount: b }) => Thicken { amount: l(*a, *b) },
            (
                Levels { in_black: a1, in_white: a2, gamma: a3, out_black: a4, out_white: a5 },
                Levels { in_black: b1, in_white: b2, gamma: b3, out_black: b4, out_white: b5 },
            ) => Levels { in_black: l(*a1, *b1), in_white: l(*a2, *b2), gamma: l(*a3, *b3), out_black: l(*a4, *b4), out_white: l(*a5, *b5) },
            (Curves { points: a }, Curves { points: b }) if a.len() == b.len() => {
                Curves { points: a.iter().zip(b).map(|(p, q)| [l(p[0], q[0]), l(p[1], q[1])]).collect() }
            }
            (HueSaturation { hue: a1, saturation: a2, lightness: a3 }, HueSaturation { hue: b1, saturation: b2, lightness: b3 }) => {
                HueSaturation { hue: l(*a1, *b1), saturation: l(*a2, *b2), lightness: l(*a3, *b3) }
            }
            (Tint { colour: ac, amount: a }, Tint { colour: bc, amount: b }) => Tint { colour: lc(*ac, *bc), amount: l(*a, *b) },
            (Grain { amount: a, size: asz, seed, animated }, Grain { amount: b, size: bsz, .. }) => {
                Grain { amount: l(*a, *b), size: l(*asz, *bsz), seed: *seed, animated: *animated }
            }
            (LightWrap { radius: ar, intensity: ai }, LightWrap { radius: br, intensity: bi }) => LightWrap { radius: l(*ar, *br), intensity: l(*ai, *bi) },
            (AlphaThreshold { threshold: a }, AlphaThreshold { threshold: b }) => AlphaThreshold { threshold: l(*a, *b) },
            (Mask { shape, x: ax, y: ay, width: aw, height: ah, feather: af, invert }, Mask { x: bx, y: by, width: bw, height: bh, feather: bf, .. }) => {
                Mask { shape: *shape, x: l(*ax, *bx), y: l(*ay, *by), width: l(*aw, *bw), height: l(*ah, *bh), feather: l(*af, *bf), invert: *invert }
            }
            (ColourKey { colour: ac, tolerance: at, softness: asf }, ColourKey { colour: bc, tolerance: bt, softness: bs }) => {
                ColourKey { colour: lc(*ac, *bc), tolerance: l(*at, *bt), softness: l(*asf, *bs) }
            }
            _ => self.clone(),
        }
    }
}

/// Clamp a hostile number; NaN and infinities become `nan`.
fn c(v: f32, lo: f32, hi: f32, nan: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { nan }
}

fn yes() -> bool {
    true
}

/// The parameters of an effect from one frame on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectKey {
    pub frame: u32,
    pub value: EffectKind,
}

/// One entry of a layer's effect stack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    #[serde(default = "yes")]
    pub enabled: bool,
    /// The parameters when the effect has no keys.
    pub kind: EffectKind,
    /// Keys sorted by frame, all of `kind`'s kind.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<EffectKey>,
    /// How the effect's result combines with what it was given (normal: replaces it).
    #[serde(default, skip_serializing_if = "is_normal")]
    pub blend: BlendMode,
    /// How much of the result is used, 0…1 (1: all of it).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub mix: f32,
}

fn is_normal(b: &BlendMode) -> bool {
    *b == BlendMode::Normal
}
fn one() -> f32 {
    1.0
}
fn is_one(v: &f32) -> bool {
    *v == 1.0
}

impl Effect {
    pub fn new(kind: EffectKind) -> Self {
        Self { enabled: true, kind: kind.sanitized(), keys: Vec::new(), blend: BlendMode::Normal, mix: 1.0 }
    }

    /// The parameters at `frame`.
    pub fn at(&self, frame: u32) -> EffectKind {
        let k = &self.keys;
        let Some(first) = k.first() else { return self.kind.clone() };
        if frame <= first.frame {
            return first.value.clone();
        }
        let i = k.partition_point(|key| key.frame <= frame);
        match (k.get(i.wrapping_sub(1)), k.get(i)) {
            (Some(a), Some(b)) if b.frame > a.frame => a.value.lerp(&b.value, (frame - a.frame) as f32 / (b.frame - a.frame) as f32),
            (Some(a), _) => a.value.clone(),
            _ => self.kind.clone(),
        }
    }

    /// Set (or replace) the key at `frame`. A value of another kind is refused.
    pub fn set_key(&mut self, frame: u32, value: EffectKind) -> bool {
        if !value.same_kind(&self.kind) {
            return false;
        }
        let value = value.sanitized();
        match self.keys.binary_search_by_key(&frame, |k| k.frame) {
            Ok(i) => {
                if let Some(k) = self.keys.get_mut(i) {
                    k.value = value;
                }
            }
            Err(i) => {
                if self.keys.len() >= MAX_KEYS {
                    return false;
                }
                self.keys.insert(i, EffectKey { frame, value });
            }
        }
        true
    }

    /// Remove the key at `frame`; true if there was one.
    pub fn remove_key(&mut self, frame: u32) -> bool {
        let before = self.keys.len();
        self.keys.retain(|k| k.frame != frame);
        self.keys.len() != before
    }

    /// Change the parameters at `frame`: with keys, set a key there; without, change the effect.
    pub fn edit(&mut self, frame: u32, value: EffectKind) -> bool {
        if !value.same_kind(&self.kind) {
            return false;
        }
        if self.keys.is_empty() {
            self.kind = value.sanitized();
            true
        } else {
            self.set_key(frame, value)
        }
    }

    /// Clamp every parameter, drop keys of another kind and duplicate frames, sort the keys.
    pub fn sanitized(mut self) -> Effect {
        self.kind = self.kind.sanitized();
        let kind = self.kind.clone();
        let mut keys: Vec<EffectKey> = self
            .keys
            .into_iter()
            .filter(|k| k.value.same_kind(&kind))
            .take(MAX_KEYS)
            .map(|k| EffectKey { frame: k.frame, value: k.value.sanitized() })
            .collect();
        keys.sort_by_key(|k| k.frame);
        keys.dedup_by_key(|k| k.frame);
        self.keys = keys;
        self.mix = c(self.mix, 0.0, 1.0, 1.0);
        self
    }
}

/// Sanitize a whole stack (at most [`MAX_EFFECTS`]).
pub fn sanitize_stack(effects: Vec<Effect>) -> Vec<Effect> {
    effects.into_iter().take(MAX_EFFECTS).map(Effect::sanitized).collect()
}

/// Does any enabled effect of the stack change pixels?
pub fn is_active(effects: &[Effect]) -> bool {
    effects.iter().any(|e| e.enabled)
}

/// Does any enabled effect read the layers below (so the composite under the layer is needed)?
pub fn needs_below(effects: &[Effect]) -> bool {
    effects.iter().any(|e| e.enabled && e.kind.needs_below())
}

/// Run the enabled effects of a stack over `img` (premultiplied, the whole canvas), in order.
/// `below` is the composite under the layer, for light wrap (without it light wrap does nothing).
pub fn apply_stack(img: &mut RgbaImage, effects: &[Effect], frame: u32, below: Option<&RgbaImage>) {
    apply_stack_at(img, (0, 0), effects, frame, below);
}

/// How far (pixels) an effect can move pixels or read its neighbours; `None`: anywhere on the
/// canvas (zoom blur), so it must run on the whole canvas.
pub fn reach(kind: &EffectKind) -> Option<u32> {
    let r = |v: f32| (v.max(0.0).ceil() as u32).saturating_add(2);
    match kind {
        EffectKind::Blur { radius } => Some(r(radius * 3.0)),
        EffectKind::DirectionalBlur { length, .. } => Some(r(length / 2.0)),
        EffectKind::RadialBlur { .. } => None,
        EffectKind::Glow { radius, .. } | EffectKind::LightWrap { radius, .. } => Some(r(radius * 3.0)),
        EffectKind::DropShadow { dx, dy, blur, .. } => Some(r(dx.abs().max(dy.abs()) + blur * 3.0)),
        EffectKind::Outline { width, .. } => Some(r(*width)),
        EffectKind::Thicken { amount } => Some(r(*amount)),
        EffectKind::Levels { .. }
        | EffectKind::Curves { .. }
        | EffectKind::HueSaturation { .. }
        | EffectKind::Tint { .. }
        | EffectKind::Grain { .. }
        | EffectKind::AlphaThreshold { .. }
        | EffectKind::Mask { .. }
        | EffectKind::ColourKey { .. } => Some(0),
    }
}

/// The reach of a whole stack at `frame` (the enabled effects' reaches add up).
pub fn stack_reach(effects: &[Effect], frame: u32) -> Option<u32> {
    effects.iter().take(MAX_EFFECTS).filter(|e| e.enabled).try_fold(0u32, |acc, e| reach(&e.at(frame).sanitized()).map(|r| acc.saturating_add(r)))
}

/// [`apply_stack`] on a part of the canvas: `img` (and `below`, the same size) sit at canvas
/// position `origin`. Pixels outside `img` must be transparent and further than the stack's
/// [`stack_reach`] from anything drawn; then the result equals running on the whole canvas.
pub fn apply_stack_at(img: &mut RgbaImage, origin: (u32, u32), effects: &[Effect], frame: u32, below: Option<&RgbaImage>) {
    for e in effects.iter().take(MAX_EFFECTS) {
        if !e.enabled || e.mix <= 0.0 {
            continue;
        }
        let k = e.at(frame).sanitized();
        if e.blend == BlendMode::Normal && (e.mix.is_nan() || e.mix >= 1.0) {
            apply_at(img, origin, &k, frame, below);
        } else {
            let input = img.clone();
            apply_at(img, origin, &k, frame, below);
            combine(img, &input, e.blend, e.mix);
        }
    }
}

/// `out` becomes `input` with the effect's result `out` laid over it in `mode`, `mix` of the way.
fn combine(out: &mut RgbaImage, input: &RgbaImage, mode: BlendMode, mix: f32) {
    let m = if mix.is_finite() { mix.clamp(0.0, 1.0) } else { 1.0 };
    for (o, i) in out.data.as_chunks_mut::<4>().0.iter_mut().zip(input.data.as_chunks::<4>().0.iter()) {
        let (fx, src) = ([o[0], o[1], o[2], o[3]], [i[0], i[1], i[2], i[3]]);
        let target = if mode == BlendMode::Normal { fx } else { tooncraft_raster::blend(src, fx, mode) };
        for c in 0..4 {
            o[c] = (src[c] as f32 + (target[c] as f32 - src[c] as f32) * m).round().clamp(0.0, 255.0) as u8;
        }
        let a = o[3];
        for c in 0..3 {
            o[c] = o[c].min(a);
        }
    }
}

/// Run one effect over `img` (premultiplied, the whole canvas). Parameters are expected sanitized.
pub fn apply(img: &mut RgbaImage, kind: &EffectKind, frame: u32, below: Option<&RgbaImage>) {
    apply_at(img, (0, 0), kind, frame, below);
}

fn apply_at(img: &mut RgbaImage, origin: (u32, u32), kind: &EffectKind, frame: u32, below: Option<&RgbaImage>) {
    if img.width == 0 || img.height == 0 || img.data.len() != img.width as usize * img.height as usize * 4 {
        return;
    }
    match kind {
        EffectKind::Blur { radius } => blur::gaussian(img, *radius),
        EffectKind::DirectionalBlur { length, angle } => blur::directional(img, *length, *angle),
        EffectKind::RadialBlur { amount, centre_x, centre_y } => blur::radial(img, *amount, *centre_x, *centre_y),
        EffectKind::Glow { radius, intensity, colour } => blur::glow(img, *radius, *intensity, *colour),
        EffectKind::DropShadow { dx, dy, blur, colour, opacity } => blur::drop_shadow(img, (*dx, *dy), *blur, *colour, *opacity),
        EffectKind::Outline { width, colour } => distance::outline(img, *width, *colour),
        EffectKind::Thicken { amount } => distance::thicken(img, *amount),
        EffectKind::Levels { in_black, in_white, gamma, out_black, out_white } => {
            colour::lut(img, &colour::levels_table(*in_black, *in_white, *gamma, *out_black, *out_white))
        }
        EffectKind::Curves { points } => colour::lut(img, &colour::curve_table(points)),
        EffectKind::HueSaturation { hue, saturation, lightness } => colour::hue_saturation(img, *hue, *saturation, *lightness),
        EffectKind::Tint { colour, amount } => colour::tint(img, *colour, *amount),
        EffectKind::Grain { amount, size, seed, animated } => colour::grain(img, origin, *amount, *size, *seed, if *animated { frame } else { 0 }),
        EffectKind::LightWrap { radius, intensity } => {
            if let Some(b) = below {
                blur::light_wrap(img, b, *radius, *intensity)
            }
        }
        EffectKind::AlphaThreshold { threshold } => colour::alpha_threshold(img, *threshold),
        EffectKind::Mask { shape, x, y, width, height, feather, invert } => {
            shape::mask(img, *shape, (*x - origin.0 as f32, *y - origin.1 as f32, *width, *height), *feather, *invert)
        }
        EffectKind::ColourKey { colour, tolerance, softness } => colour::key(img, *colour, *tolerance, *softness),
    }
}

#[cfg(test)]
mod tests;
