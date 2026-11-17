//! Autosave: recovery copies in a separate folder, written atomically.
//!
//! The app snapshots the project (cheap: tiles are shared) and calls [`write_bytes`] with bytes
//! encoded on a background thread. After a crash, [`find`] returns the newest recovery copy for a
//! document, and the app offers it when it is newer than the document itself.

use std::path::{Path, PathBuf};

use tooncraft_doc::Project;

use crate::{Result, to_bytes};

/// A file-name-safe key for a document: its file stem, or the project name when unsaved.
pub fn key_for(document: Option<&Path>, project_name: &str) -> String {
    let raw = document.and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| format!("untitled-{project_name}"));
    let mut key: String = raw.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).take(80).collect();
    if key.is_empty() {
        key.push_str("untitled");
    }
    key
}

/// The recovery file for a key.
pub fn path_for(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{key}.autosave.{}", crate::EXTENSION))
}

/// Write a recovery copy (encodes on the calling thread).
pub fn write(dir: &Path, key: &str, p: &Project) -> Result<PathBuf> {
    write_bytes(dir, key, &to_bytes(p)?)
}

/// Write already-encoded recovery bytes atomically.
pub fn write_bytes(dir: &Path, key: &str, bytes: &[u8]) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = path_for(dir, key);
    craft_atomic::atomic_write(&path, bytes)?;
    Ok(path)
}

/// The recovery copy for a key and when it was written, if it exists and loads.
pub fn find(dir: &Path, key: &str) -> Option<(PathBuf, std::time::SystemTime)> {
    let path = path_for(dir, key);
    let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok()?;
    crate::load(&path).ok()?;
    Some((path, modified))
}

/// Should the recovery copy be offered instead of the document?
pub fn newer_than_document(recovery: std::time::SystemTime, document: Option<&Path>) -> bool {
    match document.and_then(|d| std::fs::metadata(d).and_then(|m| m.modified()).ok()) {
        Some(doc) => recovery > doc,
        None => true,
    }
}

/// Remove the recovery copy (after a successful save or when the user declines it).
pub fn discard(dir: &Path, key: &str) {
    let _ = std::fs::remove_file(path_for(dir, key));
}
