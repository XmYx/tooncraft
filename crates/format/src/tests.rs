use super::*;
use tooncraft_doc::Cell;

fn sample() -> Project {
    let mut p = Project::new("Bounce", 300, 200, 12, 6).unwrap();
    let a = p.add_drawing(None).unwrap();
    let b = p.add_drawing(None).unwrap();
    for (d, x) in [(a, 3u32), (b, 1)] {
        let s = p.drawing_mut(d).unwrap();
        let t = s.tile_mut(x).unwrap();
        for px in t.bytes_mut().as_chunks_mut::<4>().0.iter_mut().take(500) {
            px.copy_from_slice(&[10, 20, 30, 200]);
        }
    }
    p.layers[0].cells = vec![Cell::Key(a), Cell::Hold, Cell::Key(b), Cell::Blank, Cell::Blank, Cell::Blank];
    p.add_layer("Colour", 0).unwrap();
    p.light_table.toggle(tooncraft_doc::Pin::new(p.layers[1].id, 2));
    p.light_table.pins[0].shift = tooncraft_doc::PinShift { x: 7, y: -3, turn: 150 };
    p.onion.before = 3;
    p.tips.insert(7, std::sync::Arc::new(tooncraft_brush::Tip::new("leaf", 2, 2, vec![0, 255, 128, 64]).unwrap()));
    p.camera.enabled = true;
    p.camera.set(tooncraft_doc::CameraKey { frame: 2, x: 100.0, y: 80.0, zoom: 1.5, angle: 10.0 });
    let key = tooncraft_doc::PegKey { x: 12.0, angle: 30.0, ..tooncraft_doc::PegKey::rest(0) };
    p.poses.push(tooncraft_doc::pose::Pose { name: "Wave".into(), keys: vec![(tooncraft_doc::PegId(4), key)] });
    p
}

#[test]
fn round_trip() {
    let mut p = sample();
    let file = std::sync::Arc::new(vec![1, 2, 3, 4]);
    p.sounds.push(tooncraft_doc::Soundtrack::new("dialogue.wav", -3, file.clone()));
    let mut second = tooncraft_doc::Soundtrack::new("dialogue.wav", 20, file);
    (second.track, second.trim_start, second.length, second.fade_in, second.muted) = (1, 1.5, Some(2.0), 0.25, true);
    second.set_volume_key(4, 0.5);
    p.sounds.push(second);
    p.sounds.push(tooncraft_doc::Soundtrack::new("music.ogg", 0, std::sync::Arc::new(vec![9; 10])));
    p.audio_tracks = vec![tooncraft_doc::AudioTrack { name: "Voice".into(), muted: false, solo: true, volume: 0.8 }];
    let bytes = to_bytes(&p).unwrap();
    let q = from_bytes(&bytes).unwrap();
    assert_eq!(q, p);
    assert!(std::sync::Arc::ptr_eq(&q.sounds[0].bytes, &q.sounds[1].bytes), "clips of one file share it after loading");
}

#[test]
fn reads_a_soundtrack_from_before_clips() {
    let mut p = sample();
    p.sounds.push(tooncraft_doc::Soundtrack::new("dialogue.wav", -3, std::sync::Arc::new(vec![5, 6, 7])));
    let bytes = to_bytes(&p).unwrap();
    // rewrite the file the way ToonCraft wrote it before sound clips: one `soundtrack`
    let limits = craft_zip::Limits { max_entries: 1000, max_total_bytes: 1 << 30 };
    let mut zip = craft_zip::Archive::open(&bytes, limits).unwrap();
    let names: Vec<String> = zip.names().map(|n| n.into_owned()).collect();
    let mut out = craft_zip::Writer::new();
    for n in names {
        let data = zip.read(&n, 1 << 30).unwrap().unwrap();
        let (name, data) = match n.as_str() {
            "project.json" => {
                let mut v: serde_json::Value = serde_json::from_slice(&data).unwrap();
                let obj = v.as_object_mut().unwrap();
                obj.remove("sounds");
                obj.insert("soundtrack".into(), serde_json::json!({"name": "dialogue.wav", "offset": -3, "entry": "audio/soundtrack"}));
                (n.clone(), serde_json::to_vec(&v).unwrap())
            }
            "audio/sound-0" => ("audio/soundtrack".to_string(), data),
            _ => (n.clone(), data),
        };
        let c = if name == "mimetype" { craft_zip::Compression::Stored } else { craft_zip::Compression::Deflated };
        out.add(&name, &data, c).unwrap();
    }
    let old = out.finish().unwrap();
    let q = from_bytes(&old).unwrap();
    assert_eq!(q.sounds, p.sounds);
}

#[test]
fn refuses_other_files_and_newer_versions() {
    assert!(matches!(from_bytes(b"hello"), Err(FormatError::NotToon(_))));
    let mut zip = craft_zip::Writer::new();
    zip.add("mimetype", MIMETYPE.as_bytes(), craft_zip::Compression::Stored).unwrap();
    zip.add("project.json", br#"{"format":"tooncraft","version":99}"#, craft_zip::Compression::Deflated).unwrap();
    let bytes = zip.finish().unwrap();
    assert!(matches!(from_bytes(&bytes), Err(FormatError::Newer(99))));
}

#[test]
fn hostile_sizes_are_refused() {
    let mut zip = craft_zip::Writer::new();
    zip.add("mimetype", MIMETYPE.as_bytes(), craft_zip::Compression::Stored).unwrap();
    let json = br#"{"format":"tooncraft","version":1,"project":{"name":"x","width":100000,"height":10,"fps":24,"length":3,"layers":[]},"drawings":[]}"#;
    zip.add("project.json", json, craft_zip::Compression::Deflated).unwrap();
    assert!(matches!(from_bytes(&zip.finish().unwrap()), Err(FormatError::Damaged(_))));
}

#[test]
fn journaled_save_keeps_a_backup_and_detects_interruptions() {
    let dir = std::env::temp_dir().join(format!("tooncraft-save-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("shot.toon");
    let p = sample();
    let r = save(&path, &p).unwrap();
    assert!(r.backup.is_none());
    assert!(!journal_path(&path).exists());
    let mut p2 = p.clone();
    p2.name = "Second".into();
    let r = save(&path, &p2).unwrap();
    assert_eq!(load(&r.backup.unwrap()).unwrap().name, "Bounce");
    assert_eq!(load(&path).unwrap().name, "Second");
    assert_eq!(check(&path), FileState::Clean);
    // A crash after the journal and before the file was replaced, with a torn file.
    std::fs::write(journal_path(&path), b"{}").unwrap();
    assert_eq!(check(&path), FileState::Recovered);
    std::fs::write(journal_path(&path), b"{}").unwrap();
    std::fs::write(&path, b"torn").unwrap();
    assert_eq!(check(&path), FileState::Interrupted { backup: Some(backup_path(&path)) });
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn autosave_round_trip() {
    let dir = std::env::temp_dir().join(format!("tooncraft-auto-{}", std::process::id()));
    let key = autosave::key_for(None, "My shot/1");
    assert_eq!(key, "untitled-My_shot_1");
    let p = sample();
    autosave::write(&dir, &key, &p).unwrap();
    let (path, when) = autosave::find(&dir, &key).unwrap();
    assert!(autosave::newer_than_document(when, None));
    assert_eq!(load(&path).unwrap(), p);
    autosave::discard(&dir, &key);
    assert!(autosave::find(&dir, &key).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(256))]
    /// Random corruption of a valid file never panics; it loads or gives an error.
    #[test]
    fn corrupted_files_never_panic(flips in proptest::collection::vec((0usize..1_000_000, proptest::prelude::any::<u8>()), 1..12), cut in 0usize..1_000_000) {
        let mut bytes = to_bytes(&sample()).unwrap();
        for (at, v) in flips {
            let n = bytes.len();
            bytes[at % n] ^= v | 1;
        }
        if cut % 3 == 0 {
            let n = bytes.len();
            bytes.truncate(cut % n);
        }
        let _ = from_bytes(&bytes);
    }

    /// Random JSON in project.json (valid ZIP around it) never panics.
    #[test]
    fn hostile_manifests_never_panic(w in 0u32..20000, h in 0u32..20000, fps in 0u32..500, len in 0u32..100000, cells in proptest::collection::vec(0u8..3, 0..20), tiles in 0u32..5) {
        let cells: Vec<serde_json::Value> = cells.iter().map(|c| match c { 0 => serde_json::json!("blank"), 1 => serde_json::json!("hold"), _ => serde_json::json!({"key": 1}) }).collect();
        let json = serde_json::json!({
            "format": "tooncraft", "version": 1,
            "project": {"name": "x", "width": w, "height": h, "fps": fps, "length": len,
                "layers": [{"id": 1, "name": "L", "cells": cells}]},
            "drawings": [{"id": 1, "tiles": tiles}]
        });
        let mut zip = craft_zip::Writer::new();
        zip.add("mimetype", MIMETYPE.as_bytes(), craft_zip::Compression::Stored).unwrap();
        zip.add("project.json", json.to_string().as_bytes(), craft_zip::Compression::Deflated).unwrap();
        let mut blob = b"TCT1".to_vec();
        blob.extend_from_slice(&tiles.to_le_bytes());
        zip.add("drawings/1.tiles", &blob, craft_zip::Compression::Deflated).unwrap();
        let _ = from_bytes(&zip.finish().unwrap());
    }
}

#[test]
fn version_names() {
    let dir = std::env::temp_dir().join(format!("tooncraft-ver-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(next_version_path(&dir.join("shot.toon")), dir.join("shot_v002.toon"));
    std::fs::write(dir.join("shot_v003.toon"), b"x").unwrap();
    assert_eq!(next_version_path(&dir.join("shot_v002.toon")), dir.join("shot_v004.toon"));
    assert_eq!(next_version_path(&dir.join("a_vx.toon")), dir.join("a_vx_v002.toon"));
}

#[test]
fn hostile_poses_are_capped_and_repaired_on_load() {
    let mut p = sample();
    let bad = tooncraft_doc::PegKey { x: 1.0e30, ..tooncraft_doc::PegKey::rest(9) };
    p.poses = vec![tooncraft_doc::pose::Pose { name: " Slump ".into(), keys: vec![(tooncraft_doc::PegId(1), bad)] }; tooncraft_doc::pose::MAX_POSES + 3];
    let q = from_bytes(&to_bytes(&p).unwrap()).unwrap();
    assert_eq!(q.poses.len(), tooncraft_doc::pose::MAX_POSES);
    assert_eq!(q.poses[0].name, "Slump");
    assert_eq!((q.poses[0].keys[0].1.x, q.poses[0].keys[0].1.frame), (1.0e6, 0));
}
