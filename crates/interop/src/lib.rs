//! Shot exchange with other animation and compositing tools (UI-free).
//!
//! Every format here comes from a public, documented specification; nothing is learned from
//! another application's code. Each module is a set of pure functions: a [`Project`] or bytes or
//! paths go in, bytes, files or plain data come out.
//!
//! | module | direction | what |
//! |---|---|---|
//! | [`psd`] | export, import | one frame as a layered Photoshop document |
//! | [`ora`] | import | an OpenRaster document's layers |
//! | [`xdts`] | export, import | the exposure-sheet interchange format (`.xdts`) |
//! | [`levels`] | export, import | the sheet plus one PNG per drawing, ready to relink |
//! | [`layer_sequences`] | export | one transparent PNG sequence per layer for compositing |
//! | [`gif`] | import | the frames and delays of an animated GIF |
//! | [`prores`] | export | Apple ProRes 4444 (with alpha) or 422 HQ in a QuickTime movie |
//! | [`boards`] | import | a BoardCraft board or animatic: panels, timings and camera moves |
//! | [`palette`] | export, import | colour palettes: GIMP `.gpl` (read through craft-palette), Photoshop swatches `.aco` |
//!
//! Imported files are hostile: every reader bounds its allocations and returns an error for
//! truncated or corrupt input, never a panic. Imported pixels are premultiplied RGBA8
//! ([`RgbaImage`]) like the rest of ToonCraft; layer lists run bottom to top (index 0 is the
//! bottom of the stack), the same order as [`Project::layers`].
//!
//! [`Project`]: tooncraft_doc::Project
//! [`Project::layers`]: tooncraft_doc::Project::layers
//! [`RgbaImage`]: tooncraft_raster::RgbaImage
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

pub mod boards;
mod common;
pub mod gif;
pub mod layer_sequences;
pub mod levels;
pub mod ora;
pub mod palette;
pub mod prores;
pub mod psd;
pub mod xdts;

#[cfg(test)]
mod testutil;

pub use common::{ImportedLayer, MAX_IMPORT_BYTES, MAX_IMPORT_FILE, decode_png, safe_file_name};

/// Why an exchange failed.
#[derive(Debug, thiserror::Error)]
pub enum InteropError {
    #[error("could not read or write {0}: {1}")]
    Io(String, std::io::Error),
    #[error("could not encode: {0}")]
    Encode(String),
    #[error("the file is damaged or not supported: {0}")]
    Decode(String),
    #[error("the file is too large: {0}")]
    Limit(String),
    #[error("the frame range {0}…{1} is empty or outside the shot")]
    Range(u32, u32),
}

pub type Result<T> = std::result::Result<T, InteropError>;
