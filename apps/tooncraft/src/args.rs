//! Command-line arguments.

use std::path::PathBuf;

pub const USAGE: &str = "Usage: tooncraft [OPTIONS] [FILE.toon]

Options:
  --demo                     open the built-in demo shot
  --frame <N>                start on frame N (1-based)
  --screenshot <PNG>         render the window once to PNG and quit
  --panel <NAME>             bring a panel to the front (timeline, layers, brush, colour, onion, camera,
                             pegs, library, effects, sound, lipsync, timing, history, drawings,
                             curves, palette, tool-properties)
  --open <NAME>              open a window: preferences, export, shortcuts, find, new-shot, canvas-size,
                             start, scene, colour-picker, icons, radial, import-video
  --layout <NAME>            start in a layout: drawing, timing, ink-paint, sheet
  --vertical                 show the exposure sheet down the page
  --tool <NAME>              start with a tool (pencil, ink, fill, gradient, select, shape …)
  --bench-playback <SECONDS> play the shot for SECONDS, print frame timing and quit
  --select <A..B>            start with frames A to B (1-based) of the top layer selected
  --loop <A..B>              start with the loop range on frames A to B (1-based)
  --light-table              pin frame 1 of the top layer to the light table and show it
  --shift-trace              as --light-table, with the pin shifted and turned (shift and trace)
  --rulers                   start with a straight ruler and two-point perspective rulers on
  --thumbnails               start with drawing thumbnails in the timeline
  --light                    light theme
  --fx <IDS>                 add effects to the current layer (comma-separated ids such as lens_flare,
                             drawn_mask, displace; see Effects ▸ Add effect)
  --context-menu <TARGET>    open a right-click menu (for screenshots): canvas, selection, cell, ruler,
                             layer, tool, tab, swatch, brush, library, onion
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
    pub shift_trace: bool,
    pub select: Option<(u32, u32)>,
    pub thumbnails: bool,
    pub file: Option<PathBuf>,
    /// A window to open at start (for screenshots of dialogs).
    pub open: Option<String>,
    pub layout: Option<String>,
    pub vertical: bool,
    pub tool: Option<String>,
    pub rulers: bool,
    /// Effect ids added to the current layer at launch (screenshots).
    pub fx: Vec<String>,
    /// A right-click menu to open at start (for screenshots of the menus).
    pub context_menu: Option<String>,
    /// The loop (in and out) range, 0-based, for screenshots of the ruler.
    pub loop_range: Option<(u32, u32)>,
}

/// The windows `--open` knows.
pub const OPEN_NAMES: [&str; 12] =
    ["preferences", "export", "shortcuts", "find", "new-shot", "canvas-size", "start", "scene", "colour-picker", "icons", "radial", "import-video"];
/// The right-click menus `--context-menu` knows.
pub const CONTEXT_MENU_NAMES: [&str; 11] = ["canvas", "selection", "cell", "ruler", "layer", "tool", "tab", "swatch", "brush", "library", "onion"];
/// The layouts `--layout` knows.
pub const LAYOUT_NAMES: [&str; 4] = ["drawing", "timing", "ink-paint", "sheet"];

#[derive(Debug, PartialEq)]
pub enum Launch {
    /// Boxed: the options are large next to the text of `Print`.
    Run(Box<Cli>),
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
            "--shift-trace" => (cli.light_table, cli.shift_trace) = (true, true),
            "--rulers" => cli.rulers = true,
            "--thumbnails" => cli.thumbnails = true,
            "--select" => {
                let v = it.next().ok_or("--select needs A..B")?;
                let (a, b) = v.split_once("..").ok_or("--select needs A..B")?;
                let a: u32 = a.parse().map_err(|_| "--select: bad start")?;
                let b: u32 = b.parse().map_err(|_| "--select: bad end")?;
                cli.select = Some((a.max(1) - 1, b.max(1) - 1));
            }
            "--loop" => {
                let v = it.next().ok_or("--loop needs A..B")?;
                let (a, b) = v.split_once("..").ok_or("--loop needs A..B")?;
                let a: u32 = a.parse().map_err(|_| "--loop: bad start")?;
                let b: u32 = b.parse().map_err(|_| "--loop: bad end")?;
                cli.loop_range = Some((a.max(1) - 1, b.max(1) - 1));
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
            "--fx" => cli.fx.extend(it.next().ok_or("--fx needs effect ids")?.split(',').map(|v| v.trim().to_string()).filter(|v| !v.is_empty())),
            "--context-menu" => {
                let v = it.next().ok_or("--context-menu needs a target")?;
                if !CONTEXT_MENU_NAMES.contains(&v.as_str()) {
                    return Err(format!("--context-menu: unknown target {v} (one of {})", CONTEXT_MENU_NAMES.join(", ")));
                }
                cli.context_menu = Some(v);
            }
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
    Ok(Launch::Run(Box::new(cli)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses() {
        let s = |v: &[&str]| parse(v.iter().map(|s| s.to_string()));
        assert_eq!(s(&["--demo", "--frame", "3"]), Ok(Launch::Run(Box::new(Cli { demo: true, frame: Some(2), ..Default::default() }))));
        assert!(s(&["--nope"]).is_err());
        assert!(s(&["a.toon", "b.toon"]).is_err());
        assert!(matches!(s(&["--help"]), Ok(Launch::Print(_))));
        assert_eq!(
            s(&["--open", "preferences", "--layout", "sheet", "--vertical"]),
            Ok(Launch::Run(Box::new(Cli { open: Some("preferences".into()), layout: Some("sheet".into()), vertical: true, ..Default::default() })))
        );
        assert!(s(&["--open", "nope"]).is_err());
        assert_eq!(s(&["--loop", "3..9"]), Ok(Launch::Run(Box::new(Cli { loop_range: Some((2, 8)), ..Default::default() }))));
        assert!(s(&["--loop", "x"]).is_err());
        assert!(s(&["--layout", "nope"]).is_err());
        assert!(s(&["--open"]).is_err());
        assert_eq!(s(&["--context-menu", "cell"]), Ok(Launch::Run(Box::new(Cli { context_menu: Some("cell".into()), ..Default::default() }))));
        assert!(s(&["--context-menu", "nope"]).is_err());
        assert!(s(&["--context-menu"]).is_err());
        assert_eq!(
            s(&["--fx", "lens_flare, drawn_mask"]),
            Ok(Launch::Run(Box::new(Cli { fx: vec!["lens_flare".into(), "drawn_mask".into()], ..Default::default() })))
        );
        assert!(s(&["--fx"]).is_err());
    }
}
