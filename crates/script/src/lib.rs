#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
//! ToonCraft scripting: a sandboxed [Rhai](https://rhai.rs) engine over the command set.
//!
//! A script changes the shot only through [`tooncraft_engine::Command`]s, the same commands the
//! app, `tooncraft-cli run` and the command files use: `run(#{cmd: "new_drawing", layer: 0,
//! frame: 3})` applies one, written as its JSON form. It reads the shot through a few functions
//! ([`API`]) and gets its arguments as the constant map `ARGS`.
//!
//! A whole run is one undo step labelled after the script. When the script fails (an error it
//! does not catch, or a limit), the shot and its history are put back exactly as they were. A
//! script has no file, network or module access, and [`Limits`] caps operations, wall-clock
//! time, call depth, container sizes, commands and output.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rhai::{Dynamic, EvalAltResult, Map, Scope};
use tooncraft_doc::Project;
use tooncraft_engine::{Command, Engine};

/// What a run may spend. A hostile or buggy script stops at the first limit it reaches.
#[derive(Clone, Debug, PartialEq)]
pub struct Limits {
    /// Interpreter operations (every expression, call and loop turn).
    pub max_operations: u64,
    /// Wall-clock time, checked as the script runs and before every command.
    pub max_time: Duration,
    pub max_call_depth: usize,
    pub max_expr_depth: usize,
    /// Longest string, in bytes.
    pub max_string_bytes: usize,
    pub max_array_len: usize,
    pub max_map_len: usize,
    /// Commands one run may apply.
    pub max_commands: u64,
    /// Lines of output kept.
    pub max_output_lines: usize,
    /// Longest script, in bytes.
    pub max_source_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_operations: 10_000_000,
            max_time: Duration::from_secs(10),
            max_call_depth: 24,
            max_expr_depth: 48,
            max_string_bytes: 256 * 1024,
            max_array_len: 20_000,
            max_map_len: 2_000,
            max_commands: 100_000,
            max_output_lines: 5_000,
            max_source_bytes: 1 << 20,
        }
    }
}

/// Where and why a script failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptError {
    pub message: String,
    /// 1-based line and column, when the error has a place.
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.line, self.column) {
            (Some(l), Some(c)) => write!(f, "line {l}, column {c}: {}", self.message),
            (Some(l), None) => write!(f, "line {l}: {}", self.message),
            _ => f.write_str(&self.message),
        }
    }
}

/// What a run did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// `print` and `debug` output, oldest first (at most [`Limits::max_output_lines`]).
    pub output: Vec<String>,
    /// Commands applied (all undone again when the run failed).
    pub commands: u64,
    /// Why the run failed; `None` when it finished and its work was kept.
    pub error: Option<ScriptError>,
    pub elapsed: Duration,
}

/// The functions a script can call besides Rhai's own, for the editor's help and the docs.
pub const API: &[(&str, &str)] = &[
    ("run(command)", "Apply one command, written as its JSON form: #{cmd: \"new_drawing\", layer: 0, frame: 3}"),
    ("shot()", "The shot as a map: name, width, height, fps, length, layers"),
    ("layers()", "The layers, bottom first, as maps: index, name, visible, locked, opacity"),
    ("name()", "The shot's name"),
    ("width()", "Canvas width in pixels"),
    ("height()", "Canvas height in pixels"),
    ("fps()", "Frames per second"),
    ("length()", "Frames in the shot"),
    ("layer_count()", "Number of layers (0 is the bottom one)"),
    ("layer_name(layer)", "A layer's name"),
    ("layer_visible(layer)", "Whether a layer is shown"),
    ("layer_locked(layer)", "Whether a layer is locked"),
    ("drawing_at(layer, frame)", "The id of the drawing exposed there (frames from 0), or () for a blank"),
    ("note(frame)", "The note on a frame (from 0), or ()"),
    ("print(text)", "Write a line to the script log"),
    ("ARGS", "The script's arguments, a map of strings"),
];

struct State {
    engine: Engine,
    output: Vec<String>,
    commands: u64,
    limits: Limits,
    start: Instant,
}

impl State {
    fn say(&mut self, line: String) {
        if self.output.len() < self.limits.max_output_lines {
            self.output.push(line);
        } else if self.output.len() == self.limits.max_output_lines {
            self.output.push("… more output was dropped".into());
        }
    }
}

type Shared = Rc<RefCell<State>>;
type Fail = Box<EvalAltResult>;

/// Run `source` on `engine`'s shot with `args`. The run is one undo step labelled `label`; a
/// failed run leaves the shot and its history as they were.
pub fn run(engine: &mut Engine, source: &str, label: &str, args: &[(String, String)], limits: &Limits) -> Report {
    let start = Instant::now();
    if source.len() > limits.max_source_bytes {
        let error = ScriptError { message: format!("the script is over {} bytes", limits.max_source_bytes), line: None, column: None };
        return Report { error: Some(error), elapsed: start.elapsed(), ..Report::default() };
    }
    // stand-ins while the script holds the shot, made before it is taken so every path returns it
    let (placeholder, spare) = match (Project::new("script", 1, 1, 1, 1), Project::new("script", 1, 1, 1, 1)) {
        (Ok(a), Ok(b)) => (Engine::new(a), Engine::new(b)),
        (Err(e), _) | (_, Err(e)) => return Report { error: Some(ScriptError { message: e.to_string(), line: None, column: None }), ..Report::default() },
    };
    let mut taken = std::mem::replace(engine, placeholder);
    let group = taken.begin_group();
    let state: Shared = Rc::new(RefCell::new(State { engine: taken, output: Vec::new(), commands: 0, limits: limits.clone(), start }));
    let rhai = sandbox(&state, limits);
    let mut scope = Scope::new();
    let mut map = Map::new();
    for (k, v) in args.iter().take(limits.max_map_len) {
        map.insert(k.as_str().into(), Dynamic::from(v.clone()));
    }
    scope.push_constant("ARGS", map);
    let result = rhai.run_with_scope(&mut scope, source);
    drop(rhai);
    let State { engine: mut taken, output, commands, .. } = match Rc::try_unwrap(state) {
        Ok(cell) => cell.into_inner(),
        // the engine and its closures are dropped above, so nothing else holds the state
        Err(shared) => {
            let mut s = shared.borrow_mut();
            State {
                engine: std::mem::replace(&mut s.engine, spare),
                output: std::mem::take(&mut s.output),
                commands: s.commands,
                limits: s.limits.clone(),
                start,
            }
        }
    };
    let error = match result {
        Ok(()) => {
            taken.end_group(group, label);
            None
        }
        Err(e) => {
            taken.abandon_group(group);
            Some(script_error(&e))
        }
    };
    *engine = taken;
    Report { output, commands, error, elapsed: start.elapsed() }
}

fn script_error(e: &EvalAltResult) -> ScriptError {
    let pos = e.position();
    let message = match e {
        EvalAltResult::ErrorTerminated(..) => "stopped: the script ran out of time".to_string(),
        EvalAltResult::ErrorTooManyOperations(..) => "stopped: the script did too much work (operation limit)".to_string(),
        _ => {
            let text = e.to_string();
            // Rhai appends the place; it is reported separately
            match text.rfind(" (line ") {
                Some(i) => text[..i].to_string(),
                None => text,
            }
        }
    };
    ScriptError { message, line: pos.line(), column: pos.position() }
}

fn sandbox(state: &Shared, limits: &Limits) -> rhai::Engine {
    let mut e = rhai::Engine::new();
    e.set_max_operations(limits.max_operations);
    e.set_max_call_levels(limits.max_call_depth);
    e.set_max_expr_depths(limits.max_expr_depth, limits.max_expr_depth);
    e.set_max_string_size(limits.max_string_bytes);
    e.set_max_array_size(limits.max_array_len);
    e.set_max_map_size(limits.max_map_len);
    // no modules from disk, no eval of strings built at run time
    e.set_module_resolver(rhai::module_resolvers::DummyModuleResolver::new());
    e.disable_symbol("eval");
    let max_time = limits.max_time;
    let start = Instant::now();
    e.on_progress(move |_| if start.elapsed() > max_time { Some(Dynamic::UNIT) } else { None });
    let s = state.clone();
    e.on_print(move |text| s.borrow_mut().say(text.to_string()));
    let s = state.clone();
    e.on_debug(move |text, _, pos| s.borrow_mut().say(format!("{text} ({pos})")));

    let s = state.clone();
    e.register_fn("run", move |m: Map| -> Result<(), Fail> {
        let mut st = s.borrow_mut();
        if st.start.elapsed() > st.limits.max_time {
            return Err("stopped: the script ran out of time".into());
        }
        if st.commands >= st.limits.max_commands {
            return Err(format!("stopped: more than {} commands", st.limits.max_commands).into());
        }
        let json: serde_json::Value = rhai::serde::from_dynamic(&Dynamic::from_map(m))?;
        let cmd: Command = serde_json::from_value(json).map_err(|e| format!("not a command: {e}"))?;
        st.engine.apply(cmd).map_err(|e| e.to_string())?;
        st.commands += 1;
        Ok(())
    });
    let read = |s: &Shared, f: &dyn Fn(&Project) -> Dynamic| f(&s.borrow().engine.project);
    let s = state.clone();
    e.register_fn("shot", move || -> Map {
        let st = s.borrow();
        let p = &st.engine.project;
        let mut m = Map::new();
        m.insert("name".into(), p.name.clone().into());
        m.insert("width".into(), (p.width() as i64).into());
        m.insert("height".into(), (p.height() as i64).into());
        m.insert("fps".into(), (p.fps() as i64).into());
        m.insert("length".into(), (p.length() as i64).into());
        m.insert("layers".into(), (p.layers.len() as i64).into());
        m
    });
    let s = state.clone();
    e.register_fn("layers", move || -> rhai::Array {
        let st = s.borrow();
        st.engine
            .project
            .layers
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let mut m = Map::new();
                m.insert("index".into(), (i as i64).into());
                m.insert("name".into(), l.name.clone().into());
                m.insert("visible".into(), l.visible.into());
                m.insert("locked".into(), l.locked.into());
                m.insert("opacity".into(), (f64::from(l.opacity)).into());
                Dynamic::from_map(m)
            })
            .collect()
    });
    let s = state.clone();
    e.register_fn("name", move || read(&s, &|p| p.name.clone().into()));
    let s = state.clone();
    e.register_fn("width", move || read(&s, &|p| (p.width() as i64).into()));
    let s = state.clone();
    e.register_fn("height", move || read(&s, &|p| (p.height() as i64).into()));
    let s = state.clone();
    e.register_fn("fps", move || read(&s, &|p| (p.fps() as i64).into()));
    let s = state.clone();
    e.register_fn("length", move || read(&s, &|p| (p.length() as i64).into()));
    let s = state.clone();
    e.register_fn("layer_count", move || read(&s, &|p| (p.layers.len() as i64).into()));
    let layer = |s: &Shared, i: i64, f: &dyn Fn(&tooncraft_doc::Layer) -> Dynamic| -> Result<Dynamic, Fail> {
        let st = s.borrow();
        let l = usize::try_from(i).ok().and_then(|i| st.engine.project.layers.get(i)).ok_or_else(|| format!("no layer {i}"))?;
        Ok(f(l))
    };
    let s = state.clone();
    e.register_fn("layer_name", move |i: i64| layer(&s, i, &|l| l.name.clone().into()));
    let s = state.clone();
    e.register_fn("layer_visible", move |i: i64| layer(&s, i, &|l| l.visible.into()));
    let s = state.clone();
    e.register_fn("layer_locked", move |i: i64| layer(&s, i, &|l| l.locked.into()));
    let s = state.clone();
    e.register_fn("drawing_at", move |i: i64, f: i64| {
        let frame = u32::try_from(f).map_err(|_| format!("no frame {f}"))?;
        layer(&s, i, &|l| l.drawing_at(frame).map_or(Dynamic::UNIT, |d| Dynamic::from(i64::try_from(d.0).unwrap_or(i64::MAX))))
    });
    let s = state.clone();
    e.register_fn("note", move |f: i64| -> Dynamic {
        let st = s.borrow();
        u32::try_from(f).ok().and_then(|f| st.engine.project.markers.get(&f)).map_or(Dynamic::UNIT, |n| n.clone().into())
    });
    e
}

#[cfg(test)]
mod tests;
