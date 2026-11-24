//! Macros: the steps taken in the app, recorded as commands and played back as one undo step.
//!
//! While recording, every command applied and every finished stroke is kept as a [`Command`]
//! (a stroke as [`Command::Stroke`] with the pen samples as they came). Undoing a recorded step
//! takes it out of the macro, and redoing it puts it back. A macro holds at most [`MAX_STEPS`]
//! steps, the size of one [`Command::Batch`], so playing it back is a single undo step that
//! either all happens or changes nothing. On disk a macro is the JSON-lines command file
//! `tooncraft-cli run` reads.

use tooncraft_brush::{Brush, Sample};

use crate::{Command, Engine, EngineError, MAX_BATCH, Result};

/// Most steps one macro holds.
pub const MAX_STEPS: usize = MAX_BATCH;
/// Most pen samples kept for one recorded stroke.
pub const MAX_STROKE_SAMPLES: usize = 1 << 20;
/// Largest macro file read.
pub const MAX_FILE_BYTES: usize = 256 << 20;

/// A macro being recorded.
#[derive(Debug, Default)]
pub(crate) struct Recording {
    steps: Vec<Command>,
    /// Steps taken out by undo, to put back on redo.
    undone: Vec<Command>,
    stroke: Option<Command>,
    /// Steps dropped because the macro was full.
    dropped: usize,
}

impl Recording {
    pub(crate) fn add(&mut self, c: Command) {
        self.undone.clear();
        // a batch (a macro played while recording) is kept as its steps: batches do not nest
        if let Command::Batch { commands, .. } = c {
            for c in commands {
                self.add(c);
            }
            return;
        }
        if self.steps.len() < MAX_STEPS {
            self.steps.push(c);
        } else {
            self.dropped += 1;
        }
    }

    pub(crate) fn undone(&mut self) {
        if let Some(c) = self.steps.pop() {
            self.undone.push(c);
        }
    }

    pub(crate) fn redone(&mut self) {
        if let Some(c) = self.undone.pop() {
            self.steps.push(c);
        }
    }

    pub(crate) fn stroke_begins(&mut self, layer: usize, frame: u32, brush: Brush) {
        self.stroke = Some(Command::Stroke { layer, frame, brush, samples: Vec::new() });
    }

    pub(crate) fn sample(&mut self, s: Sample) {
        if let Some(Command::Stroke { samples, .. }) = &mut self.stroke
            && samples.len() < MAX_STROKE_SAMPLES
        {
            samples.push(s);
        }
    }

    pub(crate) fn take_stroke(&mut self) -> Option<Command> {
        self.stroke.take()
    }
}

/// What [`Engine::stop_recording`] hands back.
#[derive(Clone, Debug, PartialEq)]
pub struct Macro {
    pub steps: Vec<Command>,
    /// Steps left out because the macro was full.
    pub dropped: usize,
}

impl Engine {
    /// Start recording a macro (a recording already running starts over).
    pub fn start_recording(&mut self) {
        self.recording = Some(Recording::default());
    }

    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    /// Steps recorded so far.
    pub fn recorded_steps(&self) -> usize {
        self.recording.as_ref().map_or(0, |r| r.steps.len())
    }

    /// Stop recording and return the macro (`None` when nothing was recording).
    pub fn stop_recording(&mut self) -> Option<Macro> {
        self.recording.take().map(|r| Macro { steps: r.steps, dropped: r.dropped })
    }

    /// Play `steps` back as one undo step named `label`; on an error nothing changes.
    pub fn play_macro(&mut self, label: &str, steps: Vec<Command>) -> Result<()> {
        if steps.is_empty() {
            return Err(EngineError::Import("the macro is empty".into()));
        }
        self.apply(Command::Batch { label: label.to_string(), commands: steps })
    }
}

/// A macro as JSON lines, one command per line.
pub fn to_lines(steps: &[Command]) -> String {
    let mut out = String::new();
    for c in steps {
        // a command is plain data; serializing it cannot fail
        if let Ok(line) = serde_json::to_string(c) {
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

/// Read a macro: one command per line; blank lines and lines starting with `#` are skipped.
pub fn from_lines(text: &str) -> Result<Vec<Command>> {
    if text.len() > MAX_FILE_BYTES {
        return Err(EngineError::Import(format!("a macro file over {} MB is not read", MAX_FILE_BYTES >> 20)));
    }
    let mut steps = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if steps.len() >= MAX_STEPS {
            return Err(EngineError::Import(format!("a macro holds at most {MAX_STEPS} steps")));
        }
        let c: Command = serde_json::from_str(t).map_err(|e| EngineError::Import(format!("line {}: {e}", n + 1)))?;
        if matches!(c, Command::Batch { .. }) {
            return Err(EngineError::Import(format!("line {}: a macro cannot hold a batch", n + 1)));
        }
        steps.push(c);
    }
    Ok(steps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tooncraft_doc::Project;

    fn engine() -> Engine {
        let mut p = Project::new("Macro", 64, 48, 12, 6).unwrap();
        p.add_layer("Ink", 1).unwrap();
        Engine::new(p)
    }

    fn pen(x: f32, y: f32) -> Sample {
        Sample { x, y, pressure: 1.0, ..Sample::default() }
    }

    #[test]
    fn recorded_steps_play_back_as_one_undo_step() {
        let mut e = engine();
        e.start_recording();
        e.apply(Command::NewDrawing { layer: 1, frame: 2 }).unwrap();
        e.begin_stroke(1, 2, Brush::default()).unwrap();
        for i in 0..10 {
            e.stroke_to(pen(5.0 + i as f32 * 3.0, 20.0)).unwrap();
        }
        e.end_stroke().unwrap();
        e.apply(Command::SetMarker { frame: 4, text: "hit".into() }).unwrap();
        assert!(e.apply(Command::NewDrawing { layer: 9, frame: 0 }).is_err(), "a failed command is not recorded");
        // undo takes the note out of the macro, redo puts it back; a cancelled stroke is not kept
        e.undo().unwrap();
        assert_eq!(e.recorded_steps(), 2);
        e.redo().unwrap();
        e.begin_stroke(1, 2, Brush::default()).unwrap();
        e.cancel_stroke();
        let m = e.stop_recording().unwrap();
        assert!(!e.is_recording());
        assert_eq!(m.steps.len(), 3);
        assert!(matches!(&m.steps[1], Command::Stroke { samples, .. } if samples.len() == 10));
        // through a file and onto a fresh shot
        let text = to_lines(&m.steps);
        let steps = from_lines(&text).unwrap();
        assert_eq!(steps, m.steps);
        let mut other = engine();
        other.play_macro("Macro", steps).unwrap();
        assert_eq!(other.project.markers.get(&4).map(String::as_str), Some("hit"));
        let d = other.project.layers[1].drawing_at(2).unwrap();
        assert!(other.project.drawing(d).unwrap().to_image().data.chunks(4).any(|p| p[3] > 0), "the stroke was drawn");
        assert_eq!(other.undo_label(), Some("Macro"));
        other.undo().unwrap();
        assert!(other.project.layers[1].drawing_at(2).is_none() && other.project.markers.is_empty(), "one step undoes it all");
    }

    #[test]
    fn a_macro_that_fails_changes_nothing() {
        let mut e = engine();
        let before = e.project.clone();
        let steps = vec![Command::SetMarker { frame: 1, text: "a".into() }, Command::NewDrawing { layer: 7, frame: 0 }];
        assert!(e.play_macro("Macro", steps).is_err());
        assert_eq!(e.project, before);
        assert!(e.play_macro("Macro", Vec::new()).is_err(), "an empty macro");
    }

    #[test]
    fn a_full_macro_drops_the_rest_and_a_played_macro_is_kept_as_its_steps() {
        let mut e = engine();
        e.start_recording();
        e.play_macro("Inner", vec![Command::SetMarker { frame: 0, text: "x".into() }, Command::SetMarker { frame: 1, text: "y".into() }]).unwrap();
        assert_eq!(e.recorded_steps(), 2, "the batch is flattened");
        for i in 0..MAX_STEPS {
            e.apply(Command::SetMarker { frame: (i % 6) as u32, text: format!("{i}") }).unwrap();
        }
        let m = e.stop_recording().unwrap();
        assert_eq!(m.steps.len(), MAX_STEPS);
        assert_eq!(m.dropped, 2);
        assert!(e.play_macro("Again", m.steps).is_ok(), "a full macro still fits one batch");
    }

    #[test]
    fn bad_files_are_refused() {
        assert!(from_lines("{\"cmd\":\"nope\"}").is_err());
        assert!(from_lines("# a note\n\n{\"cmd\":\"set_marker\",\"frame\":0,\"text\":\"a\"}\n").unwrap().len() == 1);
        assert!(from_lines("{\"cmd\":\"batch\",\"label\":\"x\",\"commands\":[]}").is_err());
        let many = "{\"cmd\":\"set_marker\",\"frame\":0,\"text\":\"a\"}\n".repeat(MAX_STEPS + 1);
        assert!(from_lines(&many).is_err());
    }
}
