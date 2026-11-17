//! Dynamics: how pen input and chance change the dab.
//!
//! A [`Response`] holds one [`Mapping`] (an amount and a [`Curve`]) for every pairing of an input
//! [`Source`] and a dab [`Target`]. Inputs are normalised to `0 … 1`:
//!
//! | Source | Input value |
//! |---|---|
//! | `Pressure` | pen pressure |
//! | `Tilt` | how far the pen leans from upright |
//! | `Speed` | pointer speed, 0 at rest, 1 at 2500 px/s or faster |
//! | `Rotation` | the stroke's heading in turns (0 is rightwards, 0.25 downwards); a pen's barrel rotation is not part of a sample yet |
//! | `Random` | a fresh deterministic random number for every dab and target |
//!
//! `Size`, `Opacity` and `Flow` are scaled: every source contributes the factor
//! `1 − amount · (1 − curve(input))`, and the factors multiply. At amount 1 and the identity curve
//! the target follows the input (zero pressure gives zero), at amount 0 the source has no effect,
//! and a negative amount reverses the effect (`speed` with a negative amount thickens the line).
//! `Scatter` and `Angle` are added: every source contributes `amount · curve(input)`, in brush
//! diameters (scatter) or turns (angle), and the contributions add up.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::Curve;

/// What drives a mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Pressure,
    Tilt,
    Speed,
    Rotation,
    Random,
}

impl Source {
    pub const ALL: [Source; 5] = [Source::Pressure, Source::Tilt, Source::Speed, Source::Rotation, Source::Random];

    /// Stable id used in `.toonbrush` files and command files.
    pub fn id(self) -> &'static str {
        match self {
            Source::Pressure => "pressure",
            Source::Tilt => "tilt",
            Source::Speed => "speed",
            Source::Rotation => "rotation",
            Source::Random => "random",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    fn from_id(id: &str) -> Option<Source> {
        Source::ALL.into_iter().find(|s| s.id() == id)
    }

    /// The curve a fresh mapping from this source starts with: speed falls, the rest rise.
    pub fn default_curve(self) -> Curve {
        if self == Source::Speed { Curve::falling() } else { Curve::linear() }
    }
}

/// What a mapping changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    Size,
    Opacity,
    Flow,
    Scatter,
    Angle,
}

impl Target {
    pub const ALL: [Target; 5] = [Target::Size, Target::Opacity, Target::Flow, Target::Scatter, Target::Angle];

    pub fn id(self) -> &'static str {
        match self {
            Target::Size => "size",
            Target::Opacity => "opacity",
            Target::Flow => "flow",
            Target::Scatter => "scatter",
            Target::Angle => "angle",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    fn from_id(id: &str) -> Option<Target> {
        Target::ALL.into_iter().find(|t| t.id() == id)
    }

    /// Scaled targets multiply; the others add.
    pub fn is_scaled(self) -> bool {
        matches!(self, Target::Size | Target::Opacity | Target::Flow)
    }
}

/// One source driving one target.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Mapping {
    /// -1 … 1; 0 switches the mapping off.
    pub amount: f32,
    pub curve: Curve,
}

impl Default for Mapping {
    fn default() -> Self {
        Mapping { amount: 0.0, curve: Curve::linear() }
    }
}

/// The normalised input values of one dab, indexed by [`Source::index`].
pub type Inputs = [f32; 5];

/// Every mapping of a brush.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Response {
    m: [[Mapping; 5]; 5],
}

impl Default for Response {
    /// Pressure scales the size by 0.8 and the opacity by 0.2; everything else is off.
    fn default() -> Self {
        let mut r = Response::off();
        r.get_mut(Target::Size, Source::Pressure).amount = 0.8;
        r.get_mut(Target::Opacity, Source::Pressure).amount = 0.2;
        r
    }
}

impl Response {
    /// Every mapping off, with each source's starting curve.
    pub fn off() -> Self {
        let mut m = [[Mapping::default(); 5]; 5];
        for row in m.iter_mut() {
            for s in Source::ALL {
                row[s.index()].curve = s.default_curve();
            }
        }
        Response { m }
    }

    pub fn get(&self, t: Target, s: Source) -> &Mapping {
        &self.m[t.index()][s.index()]
    }

    pub fn get_mut(&mut self, t: Target, s: Source) -> &mut Mapping {
        &mut self.m[t.index()][s.index()]
    }

    /// Switch a mapping on with the identity (or, for speed, falling) curve.
    pub fn set(&mut self, t: Target, s: Source, amount: f32) {
        self.get_mut(t, s).amount = amount;
    }

    /// The product of the factors of a scaled target.
    pub fn factor(&self, t: Target, inputs: &Inputs) -> f32 {
        let mut f = 1.0f32;
        for s in Source::ALL {
            let m = &self.m[t.index()][s.index()];
            if m.amount != 0.0 {
                f *= (1.0 - m.amount * (1.0 - m.curve.eval(inputs[s.index()]))).max(0.0);
            }
        }
        f.clamp(0.0, 8.0)
    }

    /// The sum of the contributions of an added target.
    pub fn offset(&self, t: Target, inputs: &Inputs) -> f32 {
        let mut o = 0.0f32;
        for s in Source::ALL {
            let m = &self.m[t.index()][s.index()];
            if m.amount != 0.0 {
                o += m.amount * m.curve.eval(inputs[s.index()]);
            }
        }
        o.clamp(-8.0, 8.0)
    }

    /// Whether any mapping of `t` is on.
    pub fn is_active(&self, t: Target) -> bool {
        self.m[t.index()].iter().any(|m| m.amount != 0.0)
    }

    /// Clamp every amount to -1 … 1 (NaN to 0).
    pub fn sanitized(mut self) -> Self {
        for row in self.m.iter_mut() {
            for m in row.iter_mut() {
                m.amount = if m.amount.is_finite() { m.amount.clamp(-1.0, 1.0) } else { 0.0 };
                m.curve = Curve::from_points(m.curve.points());
            }
        }
        self
    }
}

/// On disk: `{"size": {"pressure": {"amount": 0.8, "curve": [[0,0],[1,1]]}}}`; mappings that are
/// off with the starting curve are left out.
#[derive(Serialize, Deserialize, Default)]
struct ResponseRepr(BTreeMap<String, BTreeMap<String, Mapping>>);

impl Serialize for Response {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        let mut out = ResponseRepr::default();
        for t in Target::ALL {
            for s in Source::ALL {
                let m = self.get(t, s);
                if m.amount != 0.0 || m.curve != s.default_curve() {
                    out.0.entry(t.id().to_string()).or_default().insert(s.id().to_string(), *m);
                }
            }
        }
        out.serialize(ser)
    }
}

impl<'de> Deserialize<'de> for Response {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let repr = ResponseRepr::deserialize(de)?;
        let mut r = Response::off();
        for (t, row) in repr.0.into_iter().take(16) {
            let Some(t) = Target::from_id(&t) else { continue };
            for (s, m) in row.into_iter().take(16) {
                if let Some(s) = Source::from_id(&s) {
                    *r.get_mut(t, s) = m;
                }
            }
        }
        Ok(r.sanitized())
    }
}

/// Dynamics of a brush.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "DynamicsRepr")]
pub struct Dynamics {
    /// 0 … 1: how much pen tilt flattens and turns the dab.
    pub tilt: f32,
    /// Smallest size as a fraction of the brush size, whatever the mappings say.
    pub min_size: f32,
    pub response: Response,
}

impl Default for Dynamics {
    fn default() -> Self {
        Self { tilt: 0.0, min_size: 0.15, response: Response::default() }
    }
}

impl Dynamics {
    /// The classic three linear dynamics: pressure to size, pressure to opacity and speed to size
    /// (positive thins the line when drawing fast, negative thickens it).
    pub fn linear(pressure_size: f32, pressure_opacity: f32, velocity_size: f32, min_size: f32) -> Self {
        let mut response = Response::off();
        response.set(Target::Size, Source::Pressure, pressure_size);
        response.set(Target::Opacity, Source::Pressure, pressure_opacity);
        response.set(Target::Size, Source::Speed, velocity_size);
        Self { tilt: 0.0, min_size, response }
    }

    /// Amount of pressure → size.
    pub fn pressure_size(&self) -> f32 {
        self.response.get(Target::Size, Source::Pressure).amount
    }

    pub fn pressure_opacity(&self) -> f32 {
        self.response.get(Target::Opacity, Source::Pressure).amount
    }

    pub fn velocity_size(&self) -> f32 {
        self.response.get(Target::Size, Source::Speed).amount
    }
}

/// Dynamics as files wrote them: version-1 files hold the three linear amounts, newer files the
/// full response.
#[derive(Deserialize, Default)]
#[serde(default)]
struct DynamicsRepr {
    pressure_size: Option<f32>,
    pressure_opacity: Option<f32>,
    velocity_size: Option<f32>,
    tilt: Option<f32>,
    min_size: Option<f32>,
    response: Option<Response>,
}

impl From<DynamicsRepr> for Dynamics {
    fn from(r: DynamicsRepr) -> Self {
        let base = Dynamics::default();
        let mut response = r.response.unwrap_or(if r.pressure_size.is_some() || r.pressure_opacity.is_some() || r.velocity_size.is_some() {
            // a version-1 file: the amounts it did not name were 0 there only when written; the
            // defaults were the engine's, so keep them
            let mut v = Response::off();
            v.set(Target::Size, Source::Pressure, base.pressure_size());
            v.set(Target::Opacity, Source::Pressure, base.pressure_opacity());
            v
        } else {
            base.response
        });
        if let Some(v) = r.pressure_size {
            response.set(Target::Size, Source::Pressure, v);
        }
        if let Some(v) = r.pressure_opacity {
            response.set(Target::Opacity, Source::Pressure, v);
        }
        if let Some(v) = r.velocity_size {
            response.set(Target::Size, Source::Speed, v);
        }
        Dynamics { tilt: r.tilt.unwrap_or(base.tilt), min_size: r.min_size.unwrap_or(base.min_size), response: response.sanitized() }
    }
}

/// A deterministic hash to `0 … 1` (the Random source and scatter directions).
pub fn hash01(a: u64, b: u64) -> f32 {
    let mut h = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ 0x1656_67B1_9E37_79F9;
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^= h >> 29;
    h = h.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    h ^= h >> 32;
    (h >> 40) as f32 / (1u64 << 24) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(p: f32, speed: f32) -> Inputs {
        [p, 0.0, speed, 0.0, 0.5]
    }

    #[test]
    fn scaled_targets_multiply() {
        let mut r = Response::off();
        r.set(Target::Size, Source::Pressure, 1.0);
        assert!((r.factor(Target::Size, &inputs(0.25, 0.0)) - 0.25).abs() < 1e-5);
        r.set(Target::Size, Source::Speed, 0.5);
        // speed 1 with the falling curve: 1 - 0.5 = 0.5
        assert!((r.factor(Target::Size, &inputs(1.0, 1.0)) - 0.5).abs() < 1e-5);
        // negative amounts thicken
        r.set(Target::Size, Source::Speed, -0.5);
        assert!((r.factor(Target::Size, &inputs(1.0, 1.0)) - 1.5).abs() < 1e-5);
        assert_eq!(r.factor(Target::Flow, &inputs(0.0, 1.0)), 1.0, "unmapped targets are untouched");
    }

    #[test]
    fn added_targets_add_and_follow_the_curve() {
        let mut r = Response::off();
        r.set(Target::Scatter, Source::Random, 0.5);
        r.get_mut(Target::Scatter, Source::Random).curve = Curve::from_points(&[[0.0, 0.0], [0.5, 0.0], [1.0, 1.0]]);
        let low = r.offset(Target::Scatter, &[0.0, 0.0, 0.0, 0.0, 0.25]);
        let high = r.offset(Target::Scatter, &[0.0, 0.0, 0.0, 0.0, 1.0]);
        assert!(low < 0.05 && (high - 0.5).abs() < 1e-5, "{low} {high}");
    }

    #[test]
    fn the_curve_shapes_the_response() {
        let mut flat = Response::off();
        flat.set(Target::Opacity, Source::Pressure, 1.0);
        let mut ease = flat;
        ease.get_mut(Target::Opacity, Source::Pressure).curve = Curve::ease_in();
        assert!(ease.factor(Target::Opacity, &inputs(0.5, 0.0)) < flat.factor(Target::Opacity, &inputs(0.5, 0.0)));
    }

    #[test]
    fn json_round_trip_keeps_only_what_is_on() {
        let mut d = Dynamics::default();
        d.response.set(Target::Angle, Source::Random, 0.7);
        d.response.get_mut(Target::Flow, Source::Tilt).curve = Curve::s_curve();
        let json = serde_json::to_string(&d).unwrap_or_default();
        assert!(json.contains("random") && !json.contains("rotation"));
        let back: Dynamics = serde_json::from_str(&json).unwrap_or_default();
        assert_eq!(back, d);
    }

    #[test]
    fn version_one_dynamics_still_load() {
        let old = r#"{"pressure_size":1.0,"pressure_opacity":0.0,"tilt":0.6,"velocity_size":0.25,"min_size":0.08}"#;
        let d: Dynamics = serde_json::from_str(old).unwrap_or_default();
        assert_eq!((d.pressure_size(), d.pressure_opacity(), d.velocity_size(), d.tilt, d.min_size), (1.0, 0.0, 0.25, 0.6, 0.08));
        // missing keys keep the defaults
        let d: Dynamics = serde_json::from_str(r#"{"tilt":0.5}"#).unwrap_or_default();
        assert_eq!((d.pressure_size(), d.pressure_opacity()), (0.8, 0.2));
        assert_eq!(serde_json::from_str::<Dynamics>("{}").unwrap_or_default(), Dynamics::default());
    }

    #[test]
    fn hostile_amounts_are_clamped() {
        let json = r#"{"response":{"size":{"pressure":{"amount":99,"curve":[[0,5],[9,-1]]},"bogus":{}},"nothing":{}}}"#;
        let d: Dynamics = serde_json::from_str(json).unwrap_or_default();
        assert_eq!(d.pressure_size(), 1.0);
        assert!(d.response.get(Target::Size, Source::Pressure).curve.points().iter().all(|p| (0.0..=1.0).contains(&p[1])));
    }

    #[test]
    fn hash_is_deterministic_and_spread() {
        assert_eq!(hash01(3, 4), hash01(3, 4));
        let v: Vec<f32> = (0..1000).map(|i| hash01(i, 7)).collect();
        assert!(v.iter().all(|x| (0.0..1.0).contains(x)));
        let mean = v.iter().sum::<f32>() / 1000.0;
        assert!((mean - 0.5).abs() < 0.05, "{mean}");
    }
}
