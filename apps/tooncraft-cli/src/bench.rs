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

use tooncraft_brush::{Brush, BrushKind, Dynamics, GrainKind, Sample, Source, Target, Wet};
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

fn stroke_latency(brush: Brush) -> Result<(f64, f64, f64), String> {
    let mut e = Engine::new(Project::new("bench", 1920, 1080, 24, 1).map_err(|e| e.to_string())?);
    let kind = brush.kind;
    let mut times = Vec::new();
    for s in 0..20 {
        if matches!(kind, BrushKind::Eraser | BrushKind::Smudge) || brush.wet.pickup > 0.0 {
            // erase, smudge or mix with real ink: lay down a wide band first, outside the timing
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
                rotation: 0.0,
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
    // every dynamic switched on: a curve per source on size, opacity, flow, scatter and angle
    let mut busy = Brush { size: 6.0, ..Brush::preset(BrushKind::Ink) };
    for t in Target::ALL {
        for s in [Source::Pressure, Source::Tilt, Source::Speed, Source::Rotation, Source::Random] {
            busy.dynamics.response.set(t, s, 0.3);
            busy.dynamics.response.get_mut(t, s).curve = tooncraft_brush::Curve::s_curve();
        }
    }
    let busy = Brush { dynamics: Dynamics { min_size: 0.2, ..busy.dynamics }, ..busy };
    let ink = |size| Brush { size, ..Brush::preset(BrushKind::Ink) };
    for (name, brush) in [
        ("stroke: ink 6 px", ink(6.0)),
        ("stroke: pencil 4 px", Brush { size: 4.0, ..Brush::preset(BrushKind::Pencil) }),
        ("stroke: chalk 30 px", Brush { size: 30.0, ..Brush::preset(BrushKind::Chalk) }),
        ("stroke: airbrush 120 px", Brush { size: 120.0, ..Brush::preset(BrushKind::Airbrush) }),
        ("stroke: eraser 40 px", Brush { size: 40.0, ..Brush::preset(BrushKind::Eraser) }),
        ("stroke: ink 6 px, 25 response curves and jitter", busy),
        ("stroke: chalk 30 px, canvas-weave grain", Brush { size: 30.0, grain_kind: GrainKind::Canvas, ..Brush::preset(BrushKind::Chalk) }),
        ("stroke: watercolour 34 px (wet mixing)", Brush { size: 34.0, wet: Wet { pickup: 0.5, ..Wet::default() }, ..Brush::preset(BrushKind::Watercolour) }),
        ("stroke: smudge 30 px", Brush { size: 30.0, ..Brush::preset(BrushKind::Smudge) }),
    ] {
        let (mean, p50, p99) = stroke_latency(brush)?;
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

/// The effect stack used for the effects numbers: a typical "finished look" on the ink layer.
pub fn effect_stack() -> Vec<tooncraft_fx::Effect> {
    use tooncraft_fx::{Effect, EffectKind};
    vec![
        Effect::new(EffectKind::Outline { width: 3.0, colour: [255, 255, 255] }),
        Effect::new(EffectKind::DropShadow { dx: 12.0, dy: 12.0, blur: 8.0, colour: [0, 0, 0], opacity: 0.5 }),
        Effect::new(EffectKind::Glow { radius: 10.0, intensity: 0.8, colour: [255, 220, 160] }),
        Effect::new(EffectKind::Levels { in_black: 0.05, in_white: 0.95, gamma: 1.1, out_black: 0.0, out_white: 1.0 }),
    ]
}

/// `tooncraft-cli bench --effects`: the large shot (a held background and a drawing on every
/// frame) at `w`×`h` and `frames` long, with [`effect_stack`] on the ink layer; the CPU time to
/// composite each frame (what the app's frame worker and every export spend), and one 1080p
/// frame per effect.
pub fn effects(json: bool, w: u32, h: u32, frames: u32, file: Option<&std::path::Path>) -> Result<(), String> {
    let mut rows: Vec<(String, f64, &str)> = Vec::new();
    let e = match file {
        Some(f) => Engine::new(tooncraft_format::load(f).map_err(|e| e.to_string())?),
        None => {
            let mut e = tooncraft_engine::demo::large(w, h, frames).map_err(|e| e.to_string())?;
            for fx in effect_stack() {
                e.apply(Command::AddEffect { layer: 1, effect: fx, index: None }).map_err(|e| e.to_string())?;
            }
            e
        }
    };
    let (w, h, frames) = (e.project.width(), e.project.height(), e.project.length());
    let p = &e.project;
    let o = RenderOptions { paper: false, ..Default::default() };
    let mut times = Vec::with_capacity(frames as usize);
    let t_all = Instant::now();
    for f in 0..p.length() {
        let t0 = Instant::now();
        let _ = tooncraft_doc::render_frame(p, f, o);
        times.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    let total = t_all.elapsed().as_secs_f64();
    let mean = times.iter().sum::<f64>() / times.len().max(1) as f64;
    rows.push((format!("{w}×{h}, {frames} frames, 4-effect stack: composite per frame, mean"), mean, "ms"));
    rows.push(("composite per frame, median".into(), percentile(&mut times.clone(), 0.5), "ms"));
    rows.push(("composite per frame, 99th percentile".into(), percentile(&mut times, 0.99), "ms"));
    rows.push(("frames composited per second (one worker)".into(), p.length() as f64 / total.max(1e-9), "fps"));
    let plain = Instant::now();
    for f in 0..p.length().min(48) {
        let _ = tooncraft_doc::render_frame(p, f, RenderOptions { effects: false, ..o });
    }
    rows.push(("the same frames without effects, per frame".into(), plain.elapsed().as_secs_f64() * 1000.0 / p.length().min(48) as f64, "ms"));
    // one effect at a time on one frame
    let one = tooncraft_doc::render_frame(p, 0, RenderOptions { effects: false, ..o });
    let (w0, h0) = (one.width, one.height);
    for k in tooncraft_fx::EffectKind::catalogue(w0, h0) {
        let below = tooncraft_raster::RgbaImage::filled(w0, h0, [200, 190, 180, 255]);
        let mut img = one.clone();
        let t0 = Instant::now();
        tooncraft_fx::apply(&mut img, &k, 0, Some(&below));
        rows.push((format!("effect {} on one {w0}×{h0} layer", k.id()), t0.elapsed().as_secs_f64() * 1000.0, "ms"));
    }
    if json {
        let v: Vec<serde_json::Value> =
            rows.iter().map(|(n, v, u)| serde_json::json!({"measure": n, "value": (v * 1000.0).round() / 1000.0, "unit": u})).collect();
        println!("{}", serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?);
    } else {
        println!("| Measure | Value |\n|---|---|");
        for (n, v, u) in rows {
            println!("| {n} | {v:.2} {u} |");
        }
    }
    Ok(())
}
