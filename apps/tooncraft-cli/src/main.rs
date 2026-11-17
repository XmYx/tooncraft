//! `tooncraft-cli`: every ToonCraft action from the command line.
//!
//! ```text
//! tooncraft-cli new <file.toon> [--size 1920x1080] [--fps 24] [--frames 48] [--name NAME]
//! tooncraft-cli demo <file.toon> [--size 1280x720] [--large [--frames N]]
//! tooncraft-cli import <file.toon> <image>… [--frame N] [--hold N] [--name NAME]
//! tooncraft-cli info <file.toon>
//! tooncraft-cli run <file.toon> <commands.jsonl | ->      apply JSON-lines commands, then save
//! tooncraft-cli export <file.toon> (--png <dir> | --gif <file> | --ora <file> [--frame N]) [--range A..B] [--transparent]
//! tooncraft-cli export <file.toon> (--webp <file> | --webp-frame <file> [--frame N]) [--lossy [--quality 0-100]] [--range A..B] [--transparent]
//! tooncraft-cli export <file.toon> (--tga <dir> | --tiff <dir> | --psd-frames <file> [--psb]) [--range A..B] [--transparent]
//! tooncraft-cli check <file.toon>                          interrupted saves, backup, load check
//! tooncraft-cli bench [--json]                             stroke latency, fill, compositing, save
//! ```
//! Exit codes: 0 success, 1 failure, 2 usage error.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

mod bench;

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use tooncraft_doc::Project;
use tooncraft_engine::export::{self, ExportOptions};
use tooncraft_engine::{Command, Engine, sheet};

const USAGE: &str = "usage:
  tooncraft-cli new <file.toon> [--size WxH] [--fps N] [--frames N] [--name NAME]
  tooncraft-cli demo <file.toon> [--size WxH] [--large [--frames N]]
  tooncraft-cli import <file.toon> <image>… [--frame N] [--hold N] [--name NAME]
  tooncraft-cli info <file.toon>
  tooncraft-cli run <file.toon> <commands.jsonl | ->
  tooncraft-cli export <file.toon> (--png <dir> | --gif <file> | --ora <file> [--frame N]) [--range A..B] [--transparent]
  tooncraft-cli export <file.toon> (--webp <file> | --webp-frame <file> [--frame N]) [--lossy [--quality 0-100]] [--range A..B] [--transparent]
  tooncraft-cli export <file.toon> (--tga <dir> | --tiff <dir> | --psd-frames <file> [--psb]) [--range A..B] [--transparent]
  tooncraft-cli check <file.toon>
  tooncraft-cli bench [--json]
";

struct Args {
    positional: Vec<String>,
    flags: Vec<(String, Option<String>)>,
}

impl Args {
    fn parse(raw: impl Iterator<Item = String>) -> Args {
        let mut positional = Vec::new();
        let mut flags = Vec::new();
        let mut it = raw.peekable();
        while let Some(a) = it.next() {
            if let Some(name) = a.strip_prefix("--") {
                let takes = matches!(
                    name,
                    "size"
                        | "fps"
                        | "frames"
                        | "name"
                        | "png"
                        | "gif"
                        | "ora"
                        | "mp4"
                        | "apng"
                        | "frame"
                        | "range"
                        | "hold"
                        | "psd"
                        | "levels"
                        | "layers"
                        | "prores"
                        | "webp"
                        | "webp-frame"
                        | "tga"
                        | "tiff"
                        | "psd-frames"
                        | "quality"
                        | "prores-layers"
                        | "wav"
                        | "mouth-sheet"
                        | "layer"
                );
                let v = if takes { it.next() } else { None };
                flags.push((name.to_string(), v));
            } else {
                positional.push(a);
            }
        }
        Args { positional, flags }
    }

    fn flag(&self, name: &str) -> Option<&str> {
        self.flags.iter().find(|(n, _)| n == name).and_then(|(_, v)| v.as_deref())
    }

    fn has(&self, name: &str) -> bool {
        self.flags.iter().any(|(n, _)| n == name)
    }

    fn file(&self, i: usize) -> Result<PathBuf, String> {
        self.positional.get(i).map(PathBuf::from).ok_or_else(|| format!("missing file argument\n\n{USAGE}"))
    }
}

fn size(s: Option<&str>, default: (u32, u32)) -> Result<(u32, u32), String> {
    let Some(s) = s else { return Ok(default) };
    let (w, h) = s.split_once(['x', 'X']).ok_or("size is WIDTHxHEIGHT, e.g. 1920x1080")?;
    Ok((w.trim().parse().map_err(|_| "bad width")?, h.trim().parse().map_err(|_| "bad height")?))
}

fn num(s: Option<&str>, default: u32) -> Result<u32, String> {
    s.map(|v| v.parse::<u32>().map_err(|_| format!("not a number: {v}"))).unwrap_or(Ok(default))
}

fn save(path: &Path, e: &Engine) -> Result<(), String> {
    let r = tooncraft_format::save(path, &e.project).map_err(|e| e.to_string())?;
    println!("saved {} ({} bytes)", path.display(), r.bytes);
    Ok(())
}

fn load(path: &Path) -> Result<Engine, String> {
    if let tooncraft_format::FileState::Interrupted { backup } = tooncraft_format::check(path) {
        return Err(match backup {
            Some(b) => format!("the last save of {} was interrupted; the previous version is {}", path.display(), b.display()),
            None => format!("the last save of {} was interrupted and there is no backup", path.display()),
        });
    }
    tooncraft_format::load(path).map(Engine::new).map_err(|e| format!("{}: {e}", path.display()))
}

fn info(e: &Engine) -> serde_json::Value {
    let p = &e.project;
    serde_json::json!({
        "name": p.name,
        "size": [p.width(), p.height()],
        "fps": p.fps(),
        "frames": p.length(),
        "drawings": p.drawing_count(),
        "unused_drawings": p.unused_drawings().len(),
        "pixel_bytes": p.memory_bytes(),
        "layers": p.layers.iter().rev().map(|l| serde_json::json!({
            "name": l.name, "visible": l.visible, "locked": l.locked, "opacity": l.opacity, "sheet": sheet(&l.cells),
            "peg": l.peg.and_then(|id| p.peg(id)).map(|g| g.name.clone()), "depth": l.depth,
            "folder": l.folder, "clip": l.clip, "effectively_locked": p.is_locked(l),
        })).collect::<Vec<_>>(),
        "pegs": p.pegs.iter().map(|g| serde_json::json!({
            "id": g.id, "name": g.name, "parent": g.parent, "pivot": [g.pivot.0, g.pivot.1],
            "keys": g.keys.iter().map(|k| k.frame + 1).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "camera_shakes": p.camera.shakes.len(),
        "marks": p.marks.iter().map(|(d, m)| serde_json::json!({ "drawing": d, "mark": m })).collect::<Vec<_>>(),
        "folders": p.folders.iter().map(|f| serde_json::json!({
            "name": f.name, "parent": f.parent, "visible": f.visible, "locked": f.locked, "collapsed": f.collapsed, "color": f.color,
        })).collect::<Vec<_>>(),
    })
}

fn range(a: &Args, p: &Project) -> Result<ExportOptions, String> {
    let mut o = ExportOptions::whole(p);
    if let Some(r) = a.flag("range") {
        let (s, e) = r.split_once("..").ok_or("range is A..B (1-based, inclusive)")?;
        let s: u32 = s.parse().map_err(|_| "bad range start")?;
        let e: u32 = e.parse().map_err(|_| "bad range end")?;
        if s == 0 || e < s {
            return Err("range is A..B with 1 ≤ A ≤ B".into());
        }
        o.start = s - 1;
        o.end = e;
    }
    if a.has("transparent") {
        o.paper = false;
    }
    Ok(o)
}

fn webp_options(a: &Args) -> Result<export::WebpOptions, String> {
    let quality = num(a.flag("quality"), 80)?;
    if quality > 100 {
        return Err("--quality is 0 to 100".into());
    }
    Ok(export::WebpOptions { lossless: !a.has("lossy"), quality: quality as u8 })
}

fn run() -> Result<(), String> {
    let a = Args::parse(std::env::args().skip(1));
    if a.has("version") {
        println!("tooncraft-cli {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let cmd = a.positional.first().cloned().unwrap_or_default();
    let rest = Args { positional: a.positional.iter().skip(1).cloned().collect(), flags: a.flags.clone() };
    match cmd.as_str() {
        "new" => {
            let (w, h) = size(rest.flag("size"), (1920, 1080))?;
            let p = Project::new(rest.flag("name").unwrap_or("Shot"), w, h, num(rest.flag("fps"), 24)?, num(rest.flag("frames"), 48)?)
                .map_err(|e| e.to_string())?;
            save(&rest.file(0)?, &Engine::new(p))
        }
        "demo" => {
            let (w, h) = size(rest.flag("size"), (1280, 720))?;
            let e = if rest.has("large") {
                tooncraft_engine::demo::large(w.max(3840), h.max(2160), num(rest.flag("frames"), 240)?).map_err(|e| e.to_string())?
            } else {
                tooncraft_engine::demo::demo(w, h).map_err(|e| e.to_string())?
            };
            save(&rest.file(0)?, &e)
        }
        "info" => {
            let e = load(&rest.file(0)?)?;
            println!("{}", serde_json::to_string_pretty(&info(&e)).map_err(|e| e.to_string())?);
            Ok(())
        }
        "run" => {
            let path = rest.file(0)?;
            let mut e = load(&path)?;
            let src = rest.positional.get(1).ok_or("run needs a command file (or - for stdin)")?;
            let reader: Box<dyn BufRead> = if src == "-" {
                Box::new(std::io::BufReader::new(std::io::stdin()))
            } else {
                Box::new(std::io::BufReader::new(std::fs::File::open(src).map_err(|e| format!("{src}: {e}"))?))
            };
            for (n, line) in reader.lines().enumerate() {
                let line = line.map_err(|e| e.to_string())?;
                let t = line.trim();
                if t.is_empty() || t.starts_with('#') {
                    continue;
                }
                let c: Command = serde_json::from_str(t).map_err(|e| format!("line {}: {e}", n + 1))?;
                match t.get(..60) {
                    Some(head) => println!("{}: {head}…", n + 1),
                    None => println!("{}: {t}", n + 1),
                }
                e.apply(c).map_err(|err| format!("line {}: {err}", n + 1))?;
                if t.contains("\"undo\"") {
                    e.undo().ok();
                }
            }
            save(&path, &e)
        }
        "export" => {
            let e = load(&rest.file(0)?)?;
            let o = range(&rest, &e.project)?;
            let progress = |f: u32| eprint!("\rframe {}/{}", f + 1, o.end);
            if let Some(dir) = rest.flag("png") {
                let files = export::png_sequence(&e.project, Path::new(dir), "frame", o, progress).map_err(|e| e.to_string())?;
                eprintln!();
                println!("wrote {} PNG files to {dir}", files.len());
            } else if let Some(file) = rest.flag("gif") {
                export::gif(&e.project, Path::new(file), o, progress).map_err(|e| e.to_string())?;
                eprintln!();
                println!("wrote {file}");
            } else if let Some(file) = rest.flag("mp4") {
                export::mp4(&e.project, Path::new(file), o, progress).map_err(|e| e.to_string())?;
                eprintln!();
                println!("wrote {file}");
            } else if let Some(file) = rest.flag("apng") {
                export::apng(&e.project, Path::new(file), o, progress).map_err(|e| e.to_string())?;
                eprintln!();
                println!("wrote {file}");
            } else if let Some(file) = rest.flag("psd") {
                let frame = num(rest.flag("frame"), 1)?.max(1) - 1;
                let bytes = tooncraft_interop::psd::export_frame(&e.project, frame).map_err(|e| e.to_string())?;
                std::fs::write(file, bytes).map_err(|e| format!("{file}: {e}"))?;
                println!("wrote {file}");
            } else if let Some(dir) = rest.flag("levels") {
                let files = tooncraft_interop::levels::export(&e.project, Path::new(dir)).map_err(|e| e.to_string())?;
                println!("wrote an XDTS sheet and {} drawings to {dir}", files.len().saturating_sub(1));
            } else if let Some(dir) = rest.flag("layers") {
                let files = tooncraft_interop::layer_sequences::export(&e.project, Path::new(dir), o.start, o.end, progress).map_err(|e| e.to_string())?;
                eprintln!();
                println!("wrote {} PNG files to {dir}", files.len());
            } else if let Some(file) = rest.flag("mouth-sheet") {
                let layer = num(rest.flag("layer"), 1)?.max(1) as usize - 1;
                let text = tooncraft_engine::lipsync::sheet_text(&e.project, layer, file.ends_with(".dat"))?;
                std::fs::write(file, text).map_err(|e| format!("{file}: {e}"))?;
                println!("wrote the mouth timing of layer {} to {file}", layer + 1);
            } else if let Some(file) = rest.flag("wav") {
                tooncraft_engine::audio::export_wav(&e.project, Path::new(file), o.start, o.end)?;
                println!("wrote the sound mix to {file}");
            } else if let Some(dir) = rest.flag("prores-layers") {
                let files = tooncraft_interop::prores::export_layer_movs(&e.project, Path::new(dir), o.start, o.end).map_err(|e| e.to_string())?;
                println!("wrote {} ProRes 4444 movies to {dir}", files.len());
            } else if let Some(file) = rest.flag("prores") {
                tooncraft_interop::prores::export_mov(&e.project, Path::new(file), o.start, o.end, rest.has("alpha")).map_err(|e| e.to_string())?;
                println!("wrote {file}");
            } else if let Some(file) = rest.flag("webp") {
                export::webp_animated(&e.project, Path::new(file), o, webp_options(&rest)?, progress).map_err(|e| e.to_string())?;
                eprintln!();
                println!("wrote {file}");
            } else if let Some(file) = rest.flag("webp-frame") {
                let frame = num(rest.flag("frame"), 1)?.max(1) - 1;
                export::webp_frame(&e.project, frame, Path::new(file), webp_options(&rest)?, o.paper).map_err(|e| e.to_string())?;
                println!("wrote {file}");
            } else if let Some(dir) = rest.flag("tga") {
                let files = export::tga_sequence(&e.project, Path::new(dir), "frame", o, progress).map_err(|e| e.to_string())?;
                eprintln!();
                println!("wrote {} TGA files to {dir}", files.len());
            } else if let Some(dir) = rest.flag("tiff") {
                let files = export::tiff_sequence(&e.project, Path::new(dir), "frame", o, progress).map_err(|e| e.to_string())?;
                eprintln!();
                println!("wrote {} TIFF files to {dir}", files.len());
            } else if let Some(file) = rest.flag("psd-frames") {
                let bytes = tooncraft_interop::psd::export_frames(&e.project, o.start, o.end, rest.has("psb")).map_err(|e| e.to_string())?;
                std::fs::write(file, bytes).map_err(|e| format!("{file}: {e}"))?;
                println!("wrote {file} ({} frames, one layer group each)", o.end - o.start);
            } else if let Some(file) = rest.flag("ora") {
                let frame = num(rest.flag("frame"), 1)?.max(1) - 1;
                export::ora_frame(&e.project, frame, Path::new(file)).map_err(|e| e.to_string())?;
                println!("wrote {file}");
            } else {
                return Err(format!(
                    "export needs --png, --gif, --mp4, --apng, --prores, --prores-layers, --wav, --mouth-sheet, --psd, --layers, --levels or --ora\n\n{USAGE}"
                ));
            }
            Ok(())
        }
        "import" => {
            let path = rest.file(0)?;
            let mut e = load(&path)?;
            let images: Vec<String> = rest.positional.iter().skip(1).cloned().collect();
            if images.is_empty() {
                return Err(format!("import needs image files\n\n{USAGE}"));
            }
            let frame = num(rest.flag("frame"), 1)?.max(1) - 1;
            let hold = num(rest.flag("hold"), 1)?;
            let name = rest.flag("name").unwrap_or("Imported").to_string();
            e.apply(Command::ImportImages { paths: images, frame, hold, name }).map_err(|e| e.to_string())?;
            save(&path, &e)
        }
        "import-layers" | "import-gif" | "import-sheet" | "set-sound" => {
            let path = rest.file(0)?;
            let mut e = load(&path)?;
            let src = rest.positional.get(1).cloned().ok_or_else(|| format!("{cmd} needs a source file\n\n{USAGE}"))?;
            let frame = num(rest.flag("frame"), 1)?.max(1) - 1;
            let c = match cmd.as_str() {
                "import-layers" => Command::ImportLayers { path: src, frame },
                "import-gif" => Command::ImportGif { path: src, frame },
                "import-sheet" => Command::ImportSheet { path: src },
                _ => Command::SetSound { path: Some(src), offset: frame as i32 },
            };
            e.apply(c).map_err(|e| e.to_string())?;
            save(&path, &e)
        }
        "check" => {
            let path = rest.file(0)?;
            match tooncraft_format::check(&path) {
                tooncraft_format::FileState::Clean => println!("no interrupted save"),
                tooncraft_format::FileState::Recovered => println!("an interrupted save had completed; the stale journal was removed"),
                tooncraft_format::FileState::Interrupted { backup } => {
                    return Err(format!("the last save was interrupted; backup: {}", backup.map(|b| b.display().to_string()).unwrap_or_else(|| "none".into())));
                }
            }
            let e = load(&path)?;
            println!("loads fine: {} layers, {} drawings, {} frames", e.project.layers.len(), e.project.drawing_count(), e.project.length());
            Ok(())
        }
        "bench" => bench::run(a.has("json")),
        "" | "help" => {
            print!("{USAGE}");
            Ok(())
        }
        other => Err(format!("unknown command “{other}”\n\n{USAGE}")),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) if e.contains("usage:") => {
            eprintln!("tooncraft-cli: {e}");
            ExitCode::from(2)
        }
        Err(e) => {
            eprintln!("tooncraft-cli: {e}");
            ExitCode::FAILURE
        }
    }
}
