//! Paging drawings to disk so long shots stay memory-bounded.
//!
//! A drawing in the library is either resident (its tiles in memory) or paged: its tiles were
//! written to a page file and only a small handle stays. Reading a paged drawing loads it into a
//! cache next to the handle (so `Project::drawing` keeps returning a plain reference); paging out
//! again drops that cache. Editing a paged drawing makes it resident. The page file is the
//! process's own scratch file, removed when the last project using it is dropped; saving a shot
//! always writes every drawing into the `.toon` file, so nothing lives only in the page file.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use tooncraft_raster::{Surface, TILE_BYTES, Tile};

const MAGIC: &[u8; 4] = b"TCP1";

/// Where a paged drawing's tiles are in the page file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageKey {
    offset: u64,
    len: u64,
}

/// An append-only scratch file of drawings' tiles.
#[derive(Debug)]
pub struct PageFile {
    path: PathBuf,
    file: Mutex<(File, u64)>,
}

impl PageFile {
    /// A new page file in `dir` (the system temporary folder when `None`).
    pub fn create(dir: Option<PathBuf>) -> std::io::Result<Arc<PageFile>> {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = dir.unwrap_or_else(std::env::temp_dir);
        let path = dir.join(format!("tooncraft-pages-{}-{n}.bin", std::process::id()));
        let file = std::fs::OpenOptions::new().read(true).write(true).create_new(true).open(&path)?;
        Ok(Arc::new(PageFile { path, file: Mutex::new((file, 0)) }))
    }

    /// Bytes written so far.
    pub fn len(&self) -> u64 {
        self.file.lock().map(|g| g.1).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn write(&self, s: &Surface) -> std::io::Result<PageKey> {
        let mut blob = Vec::with_capacity(8 + s.tile_count() * (4 + TILE_BYTES));
        blob.extend_from_slice(MAGIC);
        blob.extend_from_slice(&(s.tile_count() as u32).to_le_bytes());
        for (index, tile) in s.tiles() {
            blob.extend_from_slice(&index.to_le_bytes());
            blob.extend_from_slice(tile.bytes());
        }
        let mut g = self.file.lock().map_err(|_| std::io::Error::other("page file lock poisoned"))?;
        let offset = g.1;
        g.0.seek(SeekFrom::Start(offset))?;
        g.0.write_all(&blob)?;
        g.1 = offset + blob.len() as u64;
        Ok(PageKey { offset, len: blob.len() as u64 })
    }

    fn read(&self, key: PageKey, width: u32, height: u32) -> std::io::Result<Surface> {
        let bad = |m: &str| std::io::Error::new(std::io::ErrorKind::InvalidData, m.to_string());
        if key.len < 8 || key.len > (1 << 34) {
            return Err(bad("bad page"));
        }
        let mut blob = vec![0u8; key.len as usize];
        {
            let mut g = self.file.lock().map_err(|_| std::io::Error::other("page file lock poisoned"))?;
            g.0.seek(SeekFrom::Start(key.offset))?;
            g.0.read_exact(&mut blob)?;
        }
        if &blob[..4] != MAGIC {
            return Err(bad("bad page header"));
        }
        let mut s = Surface::new(width, height).map_err(|e| bad(&e.to_string()))?;
        for chunk in blob[8..].as_chunks::<{ 4 + TILE_BYTES }>().0 {
            let index = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            let tile = Tile::from_bytes(chunk[4..].to_vec()).map_err(|e| bad(&e.to_string()))?;
            s.insert_tile(index, tile).map_err(|e| bad(&e.to_string()))?;
        }
        Ok(s)
    }
}

impl Drop for PageFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// A drawing in the library.
#[derive(Clone, Debug)]
pub(crate) enum Slot {
    Resident(Surface),
    Paged {
        file: Arc<PageFile>,
        key: PageKey,
        /// The drawing while it is loaded; `None` inside when it could not be read.
        cache: Arc<OnceLock<Option<Surface>>>,
    },
}

impl Slot {
    /// The drawing (loaded from the page file and cached when paged).
    pub(crate) fn get(&self, w: u32, h: u32) -> Option<&Surface> {
        match self {
            Slot::Resident(s) => Some(s),
            Slot::Paged { file, key, cache } => cache
                .get_or_init(|| match file.read(*key, w, h) {
                    Ok(s) => Some(s),
                    Err(e) => {
                        log::error!("could not read a paged drawing back: {e}");
                        None
                    }
                })
                .as_ref(),
        }
    }

    /// Make it resident (for editing). `None` when a paged drawing cannot be read.
    pub(crate) fn make_resident(&mut self, w: u32, h: u32) -> Option<&mut Surface> {
        if let Slot::Paged { .. } = self {
            let s = self.get(w, h)?.clone();
            *self = Slot::Resident(s);
        }
        match self {
            Slot::Resident(s) => Some(s),
            Slot::Paged { .. } => None,
        }
    }

    /// The drawing by value (a paged one is read without caching it).
    pub(crate) fn take(self, w: u32, h: u32) -> Option<Surface> {
        match self {
            Slot::Resident(s) => Some(s),
            Slot::Paged { file, key, cache } => match cache.get() {
                Some(c) => c.clone(),
                None => file.read(key, w, h).ok(),
            },
        }
    }

    /// Bytes of tiles held in memory.
    pub(crate) fn memory_bytes(&self) -> u64 {
        match self {
            Slot::Resident(s) => s.memory_bytes(),
            Slot::Paged { cache, .. } => cache.get().and_then(|c| c.as_ref()).map(Surface::memory_bytes).unwrap_or(0),
        }
    }

    pub(crate) fn is_paged(&self) -> bool {
        matches!(self, Slot::Paged { .. })
    }

    /// Write a resident drawing to the page file, or drop a paged one's cache. Returns the
    /// bytes freed.
    pub(crate) fn page_out(&mut self, file: &Arc<PageFile>) -> std::io::Result<u64> {
        let freed = self.memory_bytes();
        match self {
            Slot::Resident(s) => {
                let key = file.write(s)?;
                *self = Slot::Paged { file: Arc::clone(file), key, cache: Arc::new(OnceLock::new()) };
            }
            Slot::Paged { cache, .. } => *cache = Arc::new(OnceLock::new()),
        }
        Ok(freed)
    }
}
