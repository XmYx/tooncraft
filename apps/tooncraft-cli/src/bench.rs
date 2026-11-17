//! `tooncraft-cli bench`: the engine numbers published in docs/performance.md.
//!
//! - Stroke latency: wall time from one pointer sample to the pixels being in the drawing
//!   (stabilizer, dab spacing, rasterisation), per sample, on a 1920×1080 canvas, samples 4 px
//!   apart as from a 240 Hz pen at a quick drawing speed.
//! - Fill: one paint-bucket fill with gap closing over a full-HD reference.
//! - Composite: CPU composite of the demo shot at 1920×1080 with onion skin (exports, fill
//!   references). On screen the app composites on the GPU instead (`tooncraft --bench-playback`).
//! - Save / load: the demo shot, encode + atomic journaled write, and load.

use std::time::Instant;

use tooncraft_brush::{Brush, BrushKind, Sample};
use tooncraft_doc::{Project, RenderOptions};
use tooncraft_engine::{Command, Engine, FillReference};
use tooncraft_raster::fill::FillOptions;

fn percentile(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let i = ((v.len() - 1) as f64 * p).round() as usize;
    v[i.min(v.len() - 1)]
}

fn stroke_latency(kind: BrushKind, size: f32) -> Result<(f64, f64, f64), String> {
    let mut e = Engine::new(Project::new("bench", 1920, 1080, 24, 1).map_err(|e| e.to_string())?);
    let brush = Brush { size, ..Brush::preset(kind) };
    let mut times = Vec::new();
    for s in 0..20 {
        if kind == BrushKind::Eraser {
            // erase real ink: lay down a wide band first, outside the timing
            let band: Vec<Sample> = (0..400).map(|i| Sample::at(200.0 + i as f32 * 4.0, 200.0 + s as f32 * 35.0, 1.0, i as f64 / 240.0)).collect();
            e.apply(Command::Stroke { layer: 0, frame: 0, brush: Brush { size: 90.0, ..Brush::preset(BrushKind::Ink) }, samples: band })
                .map_err(|e| e.to_string())?;
        }
        e.begin_stroke(0, 0, brush).map_err(|e| e.to_string())?;
        for i in 0..400 {
            let t = i as f32 * 0.016;
            let sample = Sample {
                x: 200.0 + i as f32 * 4.0,
                y: 200.0 + s as f32 * 35.0 + (t * 6.0).sin() * 40.0,
                pressure: 0.4 + 0.5 * (t * 3.0).sin().abs(),
                tilt_x: 0.2,
                tilt_y: 0.1,
                time: i as f64 / 240.0,
            };
            let t0 = Instant::now();
            e.stroke_to(sample).map_err(|e| e.to_string())?;
            times.push(t0.elapsed().as_secs_f64() * 1000.0);
        }
        e.end_stroke().map_err(|e| e.to_string())?;
    }
    let mean = times.iter().sum::<f64>() / times.len().max(1) as f64;
    Ok((mean, percentile(&mut times.clone(), 0.5), percentile(&mut times, 0.99)))
}

pub fn run(json: bool) -> Result<(), String> {
    let mut rows: Vec<(String, f64, &str)> = Vec::new();
    for (name, kind, size) in [
        ("stroke: ink 6 px", BrushKind::Ink, 6.0),
        ("stroke: pencil 4 px", BrushKind::Pencil, 4.0),
        ("stroke: chalk 30 px", BrushKind::Chalk, 30.0),
        ("stroke: airbrush 120 px", BrushKind::Airbrush, 120.0),
        ("stroke: eraser 40 px", BrushKind::Eraser, 40.0),
    ] {
        let (mean, p50, p99) = stroke_latency(kind, size)?;
        rows.push((format!("{name}, mean per sample"), mean, "ms"));
        rows.push((format!("{name}, median"), p50, "ms"));
        rows.push((format!("{name}, 99th percentile"), p99, "ms"));
    }
    let mut demo = tooncraft_engine::demo::demo(1920, 1080).map_err(|e| e.to_string())?;
    let t0 = Instant::now();
    let frames = 24;
    for f in 0..frames {
        let _ = tooncraft_doc::render_frame(&demo.project, f, RenderOptions { onion_layer: Some(2), ..Default::default() });
    }
    let ms = t0.elapsed().as_secs_f64() * 1000.0 / frames as f64;
    rows.push(("composite 1920×1080 with onion skin (CPU), per frame".into(), ms, "ms"));
    rows.push(("composite (CPU), frames per second".into(), 1000.0 / ms.max(1e-6), "fps"));
    let t0 = Instant::now();
    demo.apply(Command::Fill {
        layer: 1,
        frame: 1,
        x: 40,
        y: 40,
        color: [80, 160, 220, 255],
        options: FillOptions { gap: 4, ..Default::default() },
        reference: FillReference::AllLayers,
    })
    .map_err(|e| e.to_string())?;
    rows.push(("fill 1920×1080 with gap closing 4 px".into(), t0.elapsed().as_secs_f64() * 1000.0, "ms"));
    let dir = std::env::temp_dir().join(format!("tooncraft-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("bench.toon");
    let t0 = Instant::now();
    let r = tooncraft_format::save(&path, &demo.project).map_err(|e| e.to_string())?;
    rows.push(("save demo shot (encode + journaled atomic write + verify)".into(), t0.elapsed().as_secs_f64() * 1000.0, "ms"));
    rows.push(("demo shot file size".into(), r.bytes as f64 / 1024.0, "KiB"));
    let t0 = Instant::now();
    let _ = tooncraft_format::load(&path).map_err(|e| e.to_string())?;
    rows.push(("load demo shot".into(), t0.elapsed().as_secs_f64() * 1000.0, "ms"));
    let t0 = Instant::now();
    let _ = demo.project.clone();
    rows.push(("snapshot for autosave (copy-on-write clone)".into(), t0.elapsed().as_secs_f64() * 1000.0, "ms"));
    rows.push(("demo shot pixel memory".into(), demo.project.memory_bytes() as f64 / (1024.0 * 1024.0), "MiB"));
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(tooncraft_format::backup_path(&path));
    let _ = std::fs::remove_dir(&dir);
    if json {
        let v: Vec<serde_json::Value> =
            rows.iter().map(|(n, v, u)| serde_json::json!({"measure": n, "value": (v * 1000.0).round() / 1000.0, "unit": u})).collect();
        println!("{}", serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?);
    } else {
        println!("| Measure | Value |\n|---|---|");
        for (n, v, u) in rows {
            println!("| {n} | {v:.3} {u} |");
        }
    }
    Ok(())
}
