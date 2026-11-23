//! Named colour palettes kept with the shot (ink-and-paint colour models). Hostile input is
//! bounded: at most [`MAX_PALETTES`] palettes of [`MAX_SWATCHES`] colours, names cut to
//! [`MAX_NAME`] characters.

use serde::{Deserialize, Serialize};

pub const MAX_PALETTES: usize = 64;
pub const MAX_SWATCHES: usize = 1024;
pub const MAX_NAME: usize = 80;

/// One named colour.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Swatch {
    #[serde(default)]
    pub name: String,
    pub rgb: [u8; 3],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
        }
        self
    }

    /// Add a colour (refused past the limit).
    pub fn push(&mut self, name: &str, rgb: [u8; 3]) -> bool {
        if self.swatches.len() >= MAX_SWATCHES {
            return false;
        }
        self.swatches.push(Swatch { name: cut(name), rgb });
        true
    }
}

/// A list of palettes within the limits.
pub fn sanitize_all(mut v: Vec<Palette>) -> Vec<Palette> {
    v.truncate(MAX_PALETTES);
    v.into_iter().map(Palette::sanitized).collect()
}
