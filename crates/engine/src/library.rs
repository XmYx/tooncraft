//! The brush library: the built-in brushes and the user's `.toonbrush` files, sorted into
//! categories, with favourites, a list of recently used brushes and search. UI-free; the Brushes
//! panel is `apps/tooncraft/src/brush_library.rs`.
//!
//! A user brush lives at `<library>/<category>/<name>.toonbrush`; a file directly in the library
//! folder is in the category "My brushes". Favourites and recents are kept in `library.json` in the
//! library folder. Everything read from disk is untrusted: the scan is bounded (two levels, 2000
//! files), category names are sanitized before they become folder names, and a damaged index is
//! ignored.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use tooncraft_brush::{Brush, BrushKind, Tip};

use crate::preset;

/// The category of the five plain built-in brushes.
pub const BASICS: &str = "Basics";
/// The category of a file directly in the library folder.
pub const MY_BRUSHES: &str = "My brushes";
/// Most recent brushes kept.
pub const MAX_RECENT: usize = 16;
const MAX_FILES: usize = 2000;
/// Paths collected before the list is sorted and cut to `MAX_FILES`.
const MAX_COLLECT: usize = MAX_FILES * 3;
const INDEX: &str = "library.json";

/// Where a library brush comes from.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Builtin(Box<Brush>),
    File(PathBuf),
}

/// One brush of the library.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Stable id: `builtin:<name>`, `file:<category>/<file name>`, or `file:<file name>` for a file
    /// directly in the library folder (so a folder that is called "My brushes" cannot clash with
    /// the loose files).
    pub id: String,
    pub name: String,
    pub category: String,
    pub kind: BrushKind,
    pub source: Source,
}

/// What a search looks for.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    /// Words that must all appear in the brush's name, category or kind (any case).
    pub text: String,
    /// Only this category.
    pub category: Option<String>,
    pub favourites_only: bool,
}

/// The built-in brushes as (name, category, brush): the five plain families in "Basics" and a few
/// variants of each in a category of their own, made from ordinary brush settings (see
/// `Brush::preset`).
pub fn builtin() -> Vec<(String, &'static str, Brush)> {
    let p = Brush::preset;
    let b = |name: &str, category: &'static str, brush: Brush| (name.to_string(), category, brush);
    vec![
        b("Pencil", BASICS, p(BrushKind::Pencil)),
        b("Ink pen", BASICS, p(BrushKind::Ink)),
        b("Airbrush", BASICS, p(BrushKind::Airbrush)),
        b("Chalk", BASICS, p(BrushKind::Chalk)),
        b("Eraser", BASICS, p(BrushKind::Eraser)),
        b("Watercolour", BASICS, p(BrushKind::Watercolour)),
        b("Smudge", BASICS, p(BrushKind::Smudge)),
        b("Soft pencil", "Pencils", Brush { size: 5.0, hardness: 0.3, grain: 0.7, opacity: 0.8, ..p(BrushKind::Pencil) }),
        b("Hard pencil", "Pencils", Brush { size: 2.0, hardness: 0.95, grain: 0.35, opacity: 1.0, ..p(BrushKind::Pencil) }),
        b("Fine liner", "Inks", Brush { size: 2.5, ..p(BrushKind::Ink) }),
        b("Brush pen", "Inks", Brush { size: 14.0, spacing: 0.05, ..p(BrushKind::Ink) }),
        b("Soft airbrush", "Airbrushes", Brush { size: 100.0, flow: 0.05, ..p(BrushKind::Airbrush) }),
        b("Spray", "Airbrushes", Brush { size: 40.0, hardness: 0.5, flow: 0.15, grain: 0.8, grain_scale: 1.5, ..p(BrushKind::Airbrush) }),
        b("Dry chalk", "Chalks", Brush { size: 14.0, grain: 1.0, hardness: 0.5, opacity: 0.85, ..p(BrushKind::Chalk) }),
        b("Wide chalk", "Chalks", Brush { size: 48.0, grain: 0.8, grain_scale: 4.0, ..p(BrushKind::Chalk) }),
        b("Hard eraser", "Erasers", Brush { size: 12.0, hardness: 1.0, ..p(BrushKind::Eraser) }),
        b("Soft eraser", "Erasers", Brush { size: 40.0, hardness: 0.2, ..p(BrushKind::Eraser) }),
        b("Wet wash", "Watercolours", Brush { size: 70.0, flow: 0.2, hardness: 0.2, wet: tooncraft_brush::Wet { pickup: 0.6 }, ..p(BrushKind::Watercolour) }),
        b("Dry wash", "Watercolours", Brush { size: 40.0, flow: 0.35, grain: 0.7, wet: tooncraft_brush::Wet { pickup: 0.15 }, ..p(BrushKind::Watercolour) }),
        b("Soft smudge", "Smudges", Brush { size: 50.0, hardness: 0.1, flow: 0.4, ..p(BrushKind::Smudge) }),
        b("Blur", "Smudges", Brush { size: 40.0, hardness: 0.3, flow: 1.0, wet: tooncraft_brush::Wet { pickup: 1.0 }, ..p(BrushKind::Smudge) }),
        b("Fine blur", "Smudges", Brush { size: 14.0, hardness: 0.5, flow: 1.0, wet: tooncraft_brush::Wet { pickup: 1.0 }, ..p(BrushKind::Smudge) }),
        b("Hard smudge", "Smudges", Brush { size: 16.0, hardness: 0.8, flow: 0.8, wet: tooncraft_brush::Wet { pickup: 0.15 }, ..p(BrushKind::Smudge) }),
    ]
}

/// Names Windows cannot create as a file or folder, whatever the extension.
fn is_reserved(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or("").trim_end().to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.chars().nth(3).is_some_and(|c| ('1'..='9').contains(&c)))
}

/// A name that is safe as a file or folder name everywhere: separators, characters Windows
/// refuses, control characters, leading dots and trailing dots or spaces are removed, the length
/// is capped, and a name Windows reserves (`NUL`, `COM1`, …) gets an underscore in front.
fn clean_name(name: &str, max: usize) -> String {
    let s: String = name.chars().filter(|c| !c.is_control() && !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')).collect();
    let s: String = s.trim().trim_start_matches('.').trim().chars().take(max).collect();
    let s = s.trim_end_matches(['.', ' ']).to_string();
    if is_reserved(&s) { format!("_{s}") } else { s }
}

/// A category name that is safe as a folder name: path separators, dots at the start, control
/// characters and over-long names are removed; nothing left means "My brushes".
pub fn clean_category(name: &str) -> String {
    let s = clean_name(name, 40);
    if s.is_empty() { MY_BRUSHES.to_string() } else { s }
}

fn clean_file_stem(name: &str) -> String {
    let s = clean_name(name, 80);
    if s.is_empty() { "brush".to_string() } else { s }
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
struct IndexFile {
    version: u32,
    favourites: Vec<String>,
    recent: Vec<String>,
}

/// The brush library.
#[derive(Clone, Debug, Default)]
pub struct Library {
    dir: Option<PathBuf>,
    entries: Vec<Entry>,
    favourites: BTreeSet<String>,
    recent: Vec<String>,
}

impl Library {
    /// The built-in brushes plus the brushes found in `dir` (if any).
    pub fn new(dir: Option<PathBuf>) -> Self {
        let mut l = Library::unscanned(dir);
        l.rescan();
        l
    }

    /// The built-in brushes and the saved favourites and recents, without reading the brush files
    /// yet (that is [`Library::rescan`]): opening the app does not touch the folder.
    pub fn unscanned(dir: Option<PathBuf>) -> Self {
        let mut l = Library { dir, ..Default::default() };
        l.load_index();
        l.entries = Library::builtin_entries();
        l
    }

    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    fn builtin_entries() -> Vec<Entry> {
        builtin()
            .into_iter()
            .map(|(name, category, brush)| Entry {
                id: format!("builtin:{name}"),
                category: category.to_string(),
                name,
                kind: brush.kind,
                source: Source::Builtin(Box::new(brush)),
            })
            .collect()
    }

    /// Read the library folder again.
    pub fn rescan(&mut self) {
        let mut entries = Library::builtin_entries();
        if let Some(dir) = &self.dir {
            let mut files = Vec::new();
            collect(dir, 0, &mut files);
            files.sort();
            for f in files.into_iter().take(MAX_FILES) {
                let Ok((name, kind)) = preset::peek(&f) else { continue };
                let rel = f.strip_prefix(dir).unwrap_or(&f);
                let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
                let (category, id) = match parts.as_slice() {
                    [file] => (MY_BRUSHES.to_string(), format!("file:{file}")),
                    [cat, file] => (cat.clone(), format!("file:{cat}/{file}")),
                    _ => continue,
                };
                entries.push(Entry { id, name, category, kind, source: Source::File(f) });
            }
        }
        self.entries = entries;
        // forget favourites and recents of brushes that are gone
        let ids: BTreeSet<&str> = self.entries.iter().map(|e| e.id.as_str()).collect();
        self.favourites.retain(|f| ids.contains(f.as_str()));
        self.recent.retain(|r| ids.contains(r.as_str()));
    }

    /// The categories in display order: Basics first, then the rest alphabetically.
    pub fn categories(&self) -> Vec<String> {
        let set: BTreeSet<&str> = self.entries.iter().map(|e| e.category.as_str()).collect();
        let mut v: Vec<String> = set.into_iter().filter(|c| *c != BASICS).map(str::to_string).collect();
        v.sort_by_key(|c| c.to_lowercase());
        if self.entries.iter().any(|e| e.category == BASICS) {
            v.insert(0, BASICS.to_string());
        }
        v
    }

    /// The brushes matching `q`, best matches first: a name with a word that starts with the text,
    /// then a name that contains it, then the rest in library order.
    pub fn search(&self, q: &Query) -> Vec<&Entry> {
        self.search_by(q, |e| e.name.clone())
    }

    /// [`Library::search`] with the name each brush is shown under (the translated one): the text
    /// matches that name, the stored name, the category and the kind.
    pub fn search_by(&self, q: &Query, display: impl Fn(&Entry) -> String) -> Vec<&Entry> {
        let words: Vec<String> = q.text.to_lowercase().split_whitespace().map(str::to_string).collect();
        let mut hits: Vec<(u8, usize, &Entry)> = Vec::new();
        for (i, e) in self.entries.iter().enumerate() {
            if q.category.as_deref().is_some_and(|c| c != e.category) || q.favourites_only && !self.favourites.contains(&e.id) {
                continue;
            }
            let shown = display(e).to_lowercase();
            let name = e.name.to_lowercase();
            let hay = format!("{shown} {name} {} {:?}", e.category.to_lowercase(), e.kind).to_lowercase();
            if !words.iter().all(|w| hay.contains(w.as_str())) {
                continue;
            }
            let starts = |n: &str, w: &String| n.split_whitespace().any(|x| x.starts_with(w.as_str()));
            let rank = if words.iter().all(|w| starts(&name, w) || starts(&shown, w)) {
                0
            } else if words.iter().all(|w| name.contains(w.as_str()) || shown.contains(w.as_str())) {
                1
            } else {
                2
            };
            hits.push((rank, i, e));
        }
        hits.sort_by_key(|h| (h.0, h.1));
        hits.into_iter().map(|h| h.2).collect()
    }

    /// The recently used brushes, newest first.
    pub fn recent(&self) -> Vec<&Entry> {
        self.recent.iter().filter_map(|id| self.get(id)).collect()
    }

    pub fn is_favourite(&self, id: &str) -> bool {
        self.favourites.contains(id)
    }

    pub fn favourites(&self) -> Vec<&Entry> {
        self.entries.iter().filter(|e| self.favourites.contains(&e.id)).collect()
    }

    /// Mark a brush as a favourite (or not) and save the index.
    pub fn set_favourite(&mut self, id: &str, on: bool) -> Result<(), String> {
        if self.get(id).is_none() {
            return Err("that brush is not in the library".into());
        }
        if on {
            self.favourites.insert(id.to_string());
        } else {
            self.favourites.remove(id);
        }
        self.save_index()
    }

    /// Put a brush at the front of the recent list and save the index.
    pub fn mark_used(&mut self, id: &str) -> Result<(), String> {
        if self.get(id).is_none() {
            return Err("that brush is not in the library".into());
        }
        self.recent.retain(|r| r != id);
        self.recent.insert(0, id.to_string());
        self.recent.truncate(MAX_RECENT);
        self.save_index()
    }

    /// A library brush as a preset: its name, brush and tip (built-ins have no tip).
    pub fn load(&self, id: &str) -> Result<(String, Brush, Option<Tip>), String> {
        let e = self.get(id).ok_or("that brush is not in the library")?;
        match &e.source {
            Source::Builtin(b) => Ok((e.name.clone(), **b, None)),
            Source::File(p) => preset::load(p),
        }
    }

    /// Add a brush to the library under `category` (a new folder if needed); returns its id. An
    /// existing file of that name is not overwritten: the new one gets a number.
    pub fn add(&mut self, name: &str, category: &str, brush: &Brush, tip: Option<&Tip>) -> Result<String, String> {
        let dir = self.dir.clone().ok_or("this build has no folder for brush files")?;
        let category = clean_category(category);
        let folder = if category == MY_BRUSHES { dir.clone() } else { dir.join(&category) };
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let stem = clean_file_stem(name);
        let mut path = folder.join(format!("{stem}.{}", preset::EXTENSION));
        let mut n = 2;
        while path.exists() && n < 1000 {
            path = folder.join(format!("{stem} {n}.{}", preset::EXTENSION));
            n += 1;
        }
        if path.exists() {
            return Err("too many brushes with that name".into());
        }
        preset::save(&path, name, brush, tip)?;
        self.rescan();
        let file = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        Ok(if category == MY_BRUSHES { format!("file:{file}") } else { format!("file:{category}/{file}") })
    }

    /// Copy a `.toonbrush` file into the library under `category`; returns its id.
    pub fn import(&mut self, src: &Path, category: &str) -> Result<String, String> {
        let (name, brush, tip) = preset::load(src)?;
        self.add(&name, category, &brush, tip.as_ref())
    }

    /// The file of a brush the user owns; built-in brushes have none and cannot be changed.
    pub fn file_of(&self, id: &str) -> Result<&Path, String> {
        match &self.get(id).ok_or("that brush is not in the library")?.source {
            Source::File(p) => Ok(p),
            Source::Builtin(_) => Err("built-in brushes cannot be changed; duplicate one to make your own".into()),
        }
    }

    /// Give a brush of the user's a new name (its file keeps its name).
    pub fn rename(&mut self, id: &str, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("a brush needs a name".into());
        }
        let path = self.file_of(id)?.to_path_buf();
        let (_, brush, tip) = preset::load(&path)?;
        preset::save(&path, name, &brush, tip.as_ref())?;
        self.rescan();
        Ok(())
    }

    /// A copy of a brush (built-in or the user's) in the same category, named "… copy"; returns
    /// its id. A built-in brush's copy goes to My brushes.
    pub fn duplicate(&mut self, id: &str, copy_suffix: &str) -> Result<String, String> {
        let e = self.get(id).ok_or("that brush is not in the library")?.clone();
        let (name, brush, tip) = self.load(id)?;
        let category = if matches!(e.source, Source::Builtin(_)) { MY_BRUSHES.to_string() } else { e.category };
        self.add(&format!("{name} {copy_suffix}"), &category, &brush, tip.as_ref())
    }

    /// Save `brush` into a brush of the user's, keeping its name and tip.
    pub fn overwrite(&mut self, id: &str, brush: &Brush) -> Result<(), String> {
        let path = self.file_of(id)?.to_path_buf();
        let (name, old, tip) = preset::load(&path)?;
        if old.kind != brush.kind {
            return Err("the current brush is of another kind".into());
        }
        preset::save(&path, &name, brush, tip.as_ref())?;
        self.rescan();
        Ok(())
    }

    /// Delete a brush of the user's: its file goes, and so does it from the favourites and recents.
    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        let path = self.file_of(id)?.to_path_buf();
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        self.rescan();
        self.save_index()
    }

    /// Write a brush (built-in or the user's) to `dest` as a brush file to share.
    pub fn export(&self, id: &str, dest: &Path) -> Result<(), String> {
        let (name, brush, tip) = self.load(id)?;
        preset::save(dest, &name, &brush, tip.as_ref())
    }

    fn index_path(&self) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join(INDEX))
    }

    fn load_index(&mut self) {
        let Some(p) = self.index_path() else { return };
        let Ok(meta) = std::fs::metadata(&p) else { return };
        if meta.len() > 1 << 20 {
            return;
        }
        let Ok(text) = std::fs::read_to_string(&p) else { return };
        // a damaged index is ignored: the library starts without favourites
        if let Ok(i) = serde_json::from_str::<IndexFile>(&text) {
            self.favourites = i.favourites.into_iter().take(5000).collect();
            self.recent = i.recent.into_iter().take(MAX_RECENT).collect();
        }
    }

    fn save_index(&self) -> Result<(), String> {
        let Some(p) = self.index_path() else { return Ok(()) };
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let i = IndexFile { version: 1, favourites: self.favourites.iter().cloned().collect(), recent: self.recent.clone() };
        craft_atomic::atomic_write(&p, &serde_json::to_vec_pretty(&i).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
    }
}

/// Preset files in `dir` and one level of folders below it; collecting stops at `MAX_COLLECT`
/// paths, so a crafted folder cannot make the scan list millions of entries.
fn collect(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    for entry in read.flatten().take(5000) {
        if out.len() >= MAX_COLLECT {
            return;
        }
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        if meta.is_dir() && depth == 0 {
            collect(&path, 1, out);
        } else if meta.is_file() && path.extension().is_some_and(|e| e == preset::EXTENSION) {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Removes a test's folder when the test ends, also when it fails: the bound tests write
    /// thousands of files, and leaving them behind filled a small /tmp after a few runs.
    struct Gone(PathBuf);

    impl Drop for Gone {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// An empty folder for one test, and the guard that removes it.
    fn dir(name: &str) -> (PathBuf, Gone) {
        let d = std::env::temp_dir().join(format!("tooncraft-library-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        (d.clone(), Gone(d))
    }

    #[test]
    fn test_folders_are_removed_when_the_test_ends() {
        let (d, gone) = dir("removed");
        std::fs::write(d.join("a.toonbrush"), b"").unwrap();
        assert!(d.exists());
        drop(gone);
        assert!(!d.exists(), "the folder and its files are gone");
    }

    fn q(text: &str) -> Query {
        Query { text: text.into(), ..Default::default() }
    }

    fn names(l: &Library, query: &Query) -> Vec<String> {
        l.search(query).into_iter().map(|e| e.name.clone()).collect()
    }

    #[test]
    fn built_in_brushes_are_valid_and_unique() {
        let l = Library::new(None);
        let mut ids = BTreeSet::new();
        for e in l.entries() {
            assert!(ids.insert(e.id.clone()), "duplicate id {}", e.id);
            let (name, b, tip) = l.load(&e.id).unwrap();
            assert_eq!(name, e.name);
            assert_eq!(b, b.sanitized(), "{} has out-of-range settings", e.name);
            assert_eq!(b.kind, e.kind);
            assert!(tip.is_none());
        }
        assert!(l.entries().len() >= 15);
        // every family has the plain brush and at least one variant
        for kind in BrushKind::ALL {
            assert!(l.entries().iter().filter(|e| e.kind == kind).count() >= 2, "{kind:?}");
        }
        assert_eq!(l.categories().first().map(String::as_str), Some(BASICS));
    }

    #[test]
    fn search_ranks_names_and_matches_every_word() {
        let l = Library::new(None);
        assert_eq!(names(&l, &q("")).len(), l.entries().len());
        let hits = names(&l, &q("chalk"));
        assert_eq!(hits.first().map(String::as_str), Some("Chalk"), "the plain family leads: {hits:?}");
        assert!(hits.contains(&"Dry chalk".to_string()) && !hits.contains(&"Pencil".to_string()));
        // word prefix beats substring; every word must match
        assert_eq!(names(&l, &q("soft")), vec!["Soft pencil", "Soft airbrush", "Soft eraser", "Soft smudge"]);
        assert_eq!(names(&l, &q("soft pen")), vec!["Soft pencil"]);
        assert_eq!(names(&l, &q("SOFT PEN")), vec!["Soft pencil"], "case does not matter");
        // the category and the kind are searchable too
        assert!(names(&l, &q("inks")).contains(&"Fine liner".to_string()));
        assert!(names(&l, &q("airbrush")).contains(&"Spray".to_string()));
        assert!(names(&l, &q("zzz nothing")).is_empty());
        let inks = names(&l, &Query { category: Some("Inks".into()), ..Default::default() });
        assert_eq!(inks, vec!["Fine liner", "Brush pen"]);
    }

    #[test]
    fn favourites_and_recents_persist_and_are_bounded() {
        let (d, _gone) = dir("persist");
        let mut l = Library::new(Some(d.clone()));
        assert!(l.favourites().is_empty() && l.recent().is_empty());
        l.set_favourite("builtin:Fine liner", true).unwrap();
        l.set_favourite("builtin:Spray", true).unwrap();
        l.set_favourite("builtin:Spray", false).unwrap();
        for id in ["builtin:Pencil", "builtin:Ink pen", "builtin:Pencil"] {
            l.mark_used(id).unwrap();
        }
        let again = Library::new(Some(d.clone()));
        assert_eq!(again.favourites().iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), vec!["Fine liner"]);
        assert_eq!(again.recent().iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), vec!["Pencil", "Ink pen"], "newest first, no repeats");
        assert_eq!(names(&again, &Query { favourites_only: true, ..Default::default() }), vec!["Fine liner"]);
        // the recent list is capped
        let mut l = again;
        for n in 0..3 {
            l.add(&format!("Extra {n}"), "Extras", &Brush::default(), None).unwrap();
        }
        for e in l.entries().to_vec() {
            l.mark_used(&e.id).unwrap();
        }
        assert_eq!(l.recent().len(), MAX_RECENT);
        assert!(l.set_favourite("builtin:nope", true).is_err() && l.mark_used("nope").is_err());
        // no library folder: nothing is saved, nothing fails
        let mut none = Library::new(None);
        none.set_favourite("builtin:Pencil", true).unwrap();
        assert!(none.is_favourite("builtin:Pencil"));
    }

    #[test]
    fn a_damaged_index_is_ignored() {
        let (d, _gone) = dir("damaged");
        std::fs::write(d.join(INDEX), b"{ not json").unwrap();
        let l = Library::new(Some(d.clone()));
        assert!(l.favourites().is_empty());
        std::fs::write(d.join(INDEX), vec![b'x'; 2 << 20]).unwrap();
        assert!(Library::new(Some(d.clone())).recent().is_empty());
        // ids of brushes that no longer exist are forgotten
        std::fs::write(d.join(INDEX), br#"{"version":1,"favourites":["file:Gone/x.toonbrush","builtin:Pencil"],"recent":["file:Gone/x.toonbrush"]}"#).unwrap();
        let l = Library::new(Some(d));
        assert_eq!(l.favourites().len(), 1);
        assert!(l.recent().is_empty());
    }

    #[test]
    fn files_are_found_by_folder_and_name() {
        let (d, _gone) = dir("files");
        let mut l = Library::new(Some(d.clone()));
        let brush = Brush { size: 21.0, ..Brush::preset(BrushKind::Chalk) };
        let id = l.add("Dusty", "Textures", &brush, None).unwrap();
        assert_eq!(id, "file:Textures/Dusty.toonbrush");
        let loose = l.add("Loose", "", &brush, None).unwrap();
        assert_eq!(loose, "file:Loose.toonbrush");
        assert!(d.join("Textures/Dusty.toonbrush").is_file() && d.join("Loose.toonbrush").is_file());
        // a second brush of the same name gets a number instead of overwriting
        let second = l.add("Dusty", "Textures", &Brush { size: 9.0, ..brush }, None).unwrap();
        assert_eq!(second, "file:Textures/Dusty 2.toonbrush");
        let (name, b, _) = l.load(&id).unwrap();
        assert_eq!((name.as_str(), b.size), ("Dusty", 21.0));
        assert_eq!(l.load(&second).unwrap().1.size, 9.0);
        assert!(l.categories().contains(&"Textures".to_string()) && l.categories().contains(&MY_BRUSHES.to_string()));
        assert_eq!(names(&l, &Query { category: Some("Textures".into()), ..Default::default() }), vec!["Dusty", "Dusty"]);
        // a brush file anywhere else in the tree (too deep) or with another extension is not listed
        std::fs::create_dir_all(d.join("a/b")).unwrap();
        std::fs::copy(d.join("Loose.toonbrush"), d.join("a/b/deep.toonbrush")).unwrap();
        std::fs::write(d.join("note.txt"), b"hi").unwrap();
        std::fs::write(d.join("Textures/broken.toonbrush"), b"junk").unwrap();
        l.rescan();
        assert!(l.entries().iter().all(|e| e.name != "deep" && e.name != "broken"));
        // import copies a file into a category
        let imp = l.import(&d.join("Loose.toonbrush"), "Imported").unwrap();
        assert_eq!(imp, "file:Imported/Loose.toonbrush");
        assert!(l.import(&d.join("note.txt"), "x").is_err());
    }

    #[test]
    fn hostile_names_cannot_leave_the_library_folder() {
        let (d, _gone) = dir("hostile");
        let mut l = Library::new(Some(d.clone()));
        let brush = Brush::default();
        for (name, cat) in
            [("../../evil", "../../outside"), ("a/b\\c", "/abs/path"), ("", ".."), ("con:*?\"<>|", "\u{0}nul"), (&"n".repeat(500), &"c".repeat(500))]
        {
            let id = l.add(name, cat, &brush, None).unwrap();
            let e = l.get(&id).expect("listed");
            let Source::File(p) = &e.source else { panic!("not a file") };
            assert!(p.starts_with(&d), "{} escaped {}", p.display(), d.display());
            assert!(p.file_name().unwrap().len() < 200);
        }
        assert!(!d.parent().unwrap().join("outside").exists());
        assert_eq!(clean_category("../.."), MY_BRUSHES);
        assert_eq!(clean_category("  Inks "), "Inks");
        assert!(l.add("x", "y", &brush, None).is_ok());
        // no folder, no files
        assert!(Library::new(None).add("x", "y", &brush, None).is_err());
    }

    #[test]
    fn windows_reserved_names_and_trailing_dots_are_made_safe() {
        for (name, want) in [
            ("nul", "_nul"),
            ("CON", "_CON"),
            ("com1", "_com1"),
            ("Lpt9.txt", "_Lpt9.txt"),
            ("aux ", "_aux"),
            ("COM0", "COM0"),
            ("console", "console"),
            ("name. . ", "name"),
            ("...", ""),
        ] {
            assert_eq!(clean_name(name, 40), want, "{name:?}");
        }
        assert_eq!(clean_category("nul"), "_nul");
        assert_eq!(clean_category("\u{0}nul"), "_nul");
        let (d, _gone) = dir("reserved");
        let mut l = Library::new(Some(d.clone()));
        let id = l.add("PRN", "AUX", &Brush::default(), None).unwrap();
        assert_eq!(id, "file:_AUX/_PRN.toonbrush");
        assert!(d.join("_AUX/_PRN.toonbrush").is_file());
    }

    #[test]
    fn a_folder_called_my_brushes_does_not_clash_with_loose_files() {
        let (d, _gone) = dir("clash");
        let mut l = Library::new(Some(d.clone()));
        let loose = l.add("Same", "", &Brush { size: 11.0, ..Brush::default() }, None).unwrap();
        std::fs::create_dir_all(d.join("My brushes")).unwrap();
        std::fs::copy(d.join("Same.toonbrush"), d.join("My brushes/Same.toonbrush")).unwrap();
        l.rescan();
        let same: Vec<&Entry> = l.entries().iter().filter(|e| e.name == "Same").collect();
        assert_eq!(same.len(), 2);
        assert_ne!(same[0].id, same[1].id);
        assert!(same.iter().all(|e| e.category == MY_BRUSHES));
        assert_eq!(loose, "file:Same.toonbrush");
        assert!(l.get("file:My brushes/Same.toonbrush").is_some() && l.get("file:Same.toonbrush").is_some());
    }

    #[test]
    fn search_matches_the_shown_name_as_well() {
        let l = Library::new(None);
        let q = Query { text: "bleistift".into(), ..Default::default() };
        assert!(l.search(&q).is_empty());
        let shown = |e: &Entry| match e.name.as_str() {
            "Pencil" => "Bleistift".to_string(),
            "Soft pencil" => "Weicher Bleistift".to_string(),
            n => n.to_string(),
        };
        let hits: Vec<String> = l.search_by(&q, shown).into_iter().map(|e| e.name.clone()).collect();
        assert_eq!(hits, vec!["Pencil", "Soft pencil"], "the translated word matches, the shorter name leads");
        // the stored name still matches
        assert!(!l.search_by(&Query { text: "soft pen".into(), ..Default::default() }, shown).is_empty());
    }

    #[test]
    fn opening_does_not_read_brush_files_until_asked() {
        let (d, _gone) = dir("lazy");
        let mut l = Library::new(Some(d.clone()));
        l.add("Mine", "Cat", &Brush::default(), None).unwrap();
        let fresh = Library::unscanned(Some(d));
        assert!(fresh.entries().iter().all(|e| e.id.starts_with("builtin:")), "only the built-ins before a scan");
        let mut fresh = fresh;
        fresh.rescan();
        assert!(fresh.get("file:Cat/Mine.toonbrush").is_some());
        // a large file is not listed
        let (big, _gone_big) = dir("big");
        let p = big.join("huge.toonbrush");
        std::fs::write(&p, vec![0u8; (preset::MAX_PEEK_BYTES + 1) as usize]).unwrap();
        assert!(preset::peek(&p).unwrap_err().contains("too large"));
    }

    #[test]
    fn collecting_stops_at_a_bound() {
        let (d, _gone) = dir("collect");
        for f in 0..4 {
            let sub = d.join(format!("c{f}"));
            std::fs::create_dir_all(&sub).unwrap();
            for i in 0..(MAX_COLLECT / 3 + 100) {
                std::fs::write(sub.join(format!("{i}.toonbrush")), b"").unwrap();
            }
        }
        let mut out = Vec::new();
        collect(&d, 0, &mut out);
        assert!(out.len() <= MAX_COLLECT + 1, "{}", out.len());
        assert!(out.len() >= MAX_COLLECT);
    }

    #[test]
    fn the_scan_is_bounded() {
        let (d, _gone) = dir("many");
        let b = Brush::default();
        let bytes = {
            let p = d.join("one.toonbrush");
            preset::save(&p, "One", &b, None).unwrap();
            std::fs::read(&p).unwrap()
        };
        for i in 0..(MAX_FILES + 50) {
            std::fs::write(d.join(format!("b{i:05}.toonbrush")), &bytes).unwrap();
        }
        let l = Library::new(Some(d));
        let files = l.entries().iter().filter(|e| matches!(e.source, Source::File(_))).count();
        assert!(files <= MAX_FILES, "{files}");
    }
}
