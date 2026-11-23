//! Command-line arguments.

use std::path::PathBuf;

pub const USAGE: &str = "Usage: tooncraft [OPTIONS] [FILE.toon]

Options:
  --demo                     open the built-in demo shot
  --frame <N>                start on frame N (1-based)
  --screenshot <PNG>         render the window once to PNG and quit
  --panel <NAME>             bring a panel to the front (timeline, layers, brush, colour, onion, camera,
                             pegs, library, effects, sound, lipsync, timing, history)
  --open <NAME>              open a window: preferences, export, shortcuts, find, new-shot, canvas-size,
                             scene, colour-picker
  --layout <NAME>            start in a layout: drawing, timing, ink-paint, sheet
  --vertical                 show the exposure sheet down the page
  --tool <NAME>              start with a tool (pencil, ink, fill, gradient, select, shape …)
  --bench-playback <SECONDS> play the shot for SECONDS, print frame timing and quit
  --select <A..B>            start with frames A to B (1-based) of the top layer selected
  --light-table              pin frame 1 of the top layer to the light table and show it
  --thumbnails               start with drawing thumbnails in the timeline
  --light                    light theme
  -h, --help                 print this help
  -V, --version              print the version
";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cli {
    pub demo: bool,
    pub frame: Option<u32>,
    pub screenshot: Option<PathBuf>,
    pub panel: Option<String>,
    pub bench_playback: Option<f64>,
    pub light: bool,
    pub light_table: bool,
    pub select: Option<(u32, u32)>,
    pub thumbnails: bool,
    pub file: Option<PathBuf>,
    /// A window to open at start (for screenshots of dialogs).
    pub open: Option<String>,
    pub layout: Option<String>,
    pub vertical: bool,
    pub tool: Option<String>,
}

/// The windows `--open` knows.
pub const OPEN_NAMES: [&str; 8] = ["preferences", "export", "shortcuts", "find", "new-shot", "canvas-size", "scene", "colour-picker"];
/// The layouts `--layout` knows.
pub const LAYOUT_NAMES: [&str; 4] = ["drawing", "timing", "ink-paint", "sheet"];

#[derive(Debug, PartialEq)]
pub enum Launch {
    Run(Cli),
    Print(String),
}

/// `ToonCraft <version>`, plus the commit and date `packaging/common.sh` exports for release builds.
pub fn version() -> String {
    let mut v = format!("ToonCraft {}", env!("CARGO_PKG_VERSION"));
    if let Some(sha) = option_env!("TOONCRAFT_BUILD_SHA").filter(|s| !s.is_empty()) {
        v.push_str(&format!(" ({})", sha.get(..12).unwrap_or(sha)));
    }
    if let Some(date) = option_env!("TOONCRAFT_BUILD_DATE").filter(|s| !s.is_empty()) {
        v.push_str(&format!(" {date}"));
    }
    v.push('\n');
    v
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Launch, String> {
    let mut cli = Cli::default();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => return Ok(Launch::Print(USAGE.to_string())),
            "-V" | "--version" => return Ok(Launch::Print(version())),
            "--demo" => cli.demo = true,
            "--light" => cli.light = true,
            "--light-table" => cli.light_table = true,
            "--thumbnails" => cli.thumbnails = true,
            "--select" => {
                let v = it.next().ok_or("--select needs A..B")?;
                let (a, b) = v.split_once("..").ok_or("--select needs A..B")?;
                let a: u32 = a.parse().map_err(|_| "--select: bad start")?;
                let b: u32 = b.parse().map_err(|_| "--select: bad end")?;
                cli.select = Some((a.max(1) - 1, b.max(1) - 1));
            }
            "--frame" => {
                let v = it.next().ok_or("--frame needs a number")?;
                let n: u32 = v.parse().map_err(|_| format!("--frame: not a number: {v}"))?;
                cli.frame = Some(n.max(1) - 1);
            }
            "--screenshot" => cli.screenshot = Some(PathBuf::from(it.next().ok_or("--screenshot needs a file")?)),
            "--panel" => cli.panel = Some(it.next().ok_or("--panel needs a name")?),
            "--open" => {
                let v = it.next().ok_or("--open needs a window name")?;
                if !OPEN_NAMES.contains(&v.as_str()) {
                    return Err(format!("--open: unknown window {v} (one of {})", OPEN_NAMES.join(", ")));
                }
                cli.open = Some(v);
            }
            "--layout" => {
                let v = it.next().ok_or("--layout needs a name")?;
                if !LAYOUT_NAMES.contains(&v.as_str()) {
                    return Err(format!("--layout: unknown layout {v} (one of {})", LAYOUT_NAMES.join(", ")));
                }
                cli.layout = Some(v);
            }
            "--vertical" => cli.vertical = true,
            "--tool" => cli.tool = Some(it.next().ok_or("--tool needs a name")?),
            "--bench-playback" => {
                let v = it.next().ok_or("--bench-playback needs seconds")?;
                let s: f64 = v.parse().map_err(|_| format!("--bench-playback: not a number: {v}"))?;
                cli.bench_playback = Some(s.clamp(1.0, 600.0));
            }
            s if s.starts_with('-') => return Err(format!("unknown option {s}")),
            _ => {
                if cli.file.is_some() {
                    return Err("only one file can be opened".into());
                }
                cli.file = Some(PathBuf::from(a));
            }
        }
    }
    Ok(Launch::Run(cli))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses() {
        let s = |v: &[&str]| parse(v.iter().map(|s| s.to_string()));
        assert_eq!(s(&["--demo", "--frame", "3"]), Ok(Launch::Run(Cli { demo: true, frame: Some(2), ..Default::default() })));
        assert!(s(&["--nope"]).is_err());
        assert!(s(&["a.toon", "b.toon"]).is_err());
        assert!(matches!(s(&["--help"]), Ok(Launch::Print(_))));
        assert_eq!(
            s(&["--open", "preferences", "--layout", "sheet", "--vertical"]),
            Ok(Launch::Run(Cli { open: Some("preferences".into()), layout: Some("sheet".into()), vertical: true, ..Default::default() }))
        );
        assert!(s(&["--open", "nope"]).is_err());
        assert!(s(&["--layout", "nope"]).is_err());
        assert!(s(&["--open"]).is_err());
    }
}
