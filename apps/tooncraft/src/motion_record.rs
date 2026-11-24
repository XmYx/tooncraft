//! Record motion: with recording armed and the Transform tool, moving the current layer's peg
//! while the shot plays keys every frame it passes, so a move is acted out instead of keyed.
//!
//! A drag on the canvas starts a take (and starts playback when the shot is stopped); the layer
//! follows the pointer. With the canvas focused and the shot playing, the arrow keys move it
//! (Shift: ten pixels). The take becomes one undo step ("Record motion") when the drag ends or
//! playback stops; Escape drops it and leaves the project as it was. Frames playback skipped are
//! filled in between the frames it showed.

use std::collections::BTreeMap;

use egui::{Pos2, Ui};
use tooncraft_doc::peg::{self, MAX_PEG_KEYS, PegDrag};
use tooncraft_doc::{Ease, PegId, PegKey};
use tooncraft_engine::Command;

use crate::app::ToonApp;

/// A take holds at most this many keys (a peg's own limit).
pub const MAX_TAKE: usize = MAX_PEG_KEYS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Pointer,
    Keyboard,
}

/// A take in progress: nothing changes in the project until it ends.
#[derive(Clone, Debug)]
struct Take {
    peg: PegId,
    source: Source,
    /// The frame the take began on: every key is the peg's key there, moved by `offset`.
    start: u32,
    /// Canvas pixels where it began and how far it has moved since.
    from: (f32, f32),
    offset: (f32, f32),
    /// The take has moved away from where it began at some point.
    moved: bool,
    keys: BTreeMap<u32, PegKey>,
    last: Option<u32>,
    /// The take started playback, so it stops playback when it ends.
    started_playback: bool,
}

#[derive(Default)]
pub struct Recorder {
    /// Moves on the canvas record while the shot plays.
    pub armed: bool,
    take: Option<Take>,
}

impl Recorder {
    pub fn recording(&self) -> bool {
        self.take.is_some()
    }

    #[cfg(test)]
    pub fn take_len(&self) -> usize {
        self.take.as_ref().map(|t| t.keys.len()).unwrap_or(0)
    }
}

/// Put `key` in the take at its frame. When playback skipped frames since `last` (moving
/// forward), the frames between get keys part way from the last key to this one. Stops adding
/// at [`MAX_TAKE`] keys; returns whether every key fitted.
fn record(keys: &mut BTreeMap<u32, PegKey>, last: Option<u32>, key: PegKey) -> bool {
    let fits = |k: PegKey, keys: &mut BTreeMap<u32, PegKey>| {
        if keys.len() >= MAX_TAKE && !keys.contains_key(&k.frame) {
            return false;
        }
        keys.insert(k.frame, k);
        true
    };
    if let Some(l) = last
        && key.frame > l + 1
        && let Some(a) = keys.get(&l).copied()
    {
        let span = (key.frame - l) as f32;
        for f in l + 1..key.frame {
            let t = (f - l) as f32 / span;
            let lerp = |x: f32, y: f32| x + (y - x) * t;
            let k = PegKey {
                frame: f,
                x: lerp(a.x, key.x),
                y: lerp(a.y, key.y),
                angle: lerp(a.angle, key.angle),
                scale_x: lerp(a.scale_x, key.scale_x),
                scale_y: lerp(a.scale_y, key.scale_y),
                skew: lerp(a.skew, key.skew),
                ease: Ease::Linear,
            };
            if !fits(k, keys) {
                return false;
            }
        }
    }
    fits(key, keys)
}

/// The key the take gives at `frame`: the peg's key at the take's first frame, moved by the
/// take's offset (on the canvas, whatever the parent pegs do).
fn key_at(app: &ToonApp, t: &Take, frame: u32) -> Option<PegKey> {
    let to = (t.from.0 + t.offset.0, t.from.1 + t.offset.1);
    let mut k = peg::dragged_key(&app.engine.project.pegs, t.peg, t.start, PegDrag::Move, t.from, to)?;
    k.frame = frame;
    k.ease = Ease::Linear;
    Some(k)
}

/// Key the current frame into the take.
fn sample(app: &mut ToonApp) {
    let frame = app.frame;
    let Some(t) = app.recorder.take.as_ref() else { return };
    let Some(k) = key_at(app, t, frame) else { return };
    let Some(t) = app.recorder.take.as_mut() else { return };
    let last = t.last;
    let fitted = record(&mut t.keys, last, k);
    t.last = Some(frame);
    if !fitted {
        app.set_status(tlf!("A take holds at most {n} keys: the rest was not recorded", n = MAX_TAKE));
    }
}

fn begin(app: &mut ToonApp, peg: PegId, source: Source, from: (f32, f32)) {
    let started_playback = !app.playback.playing;
    if started_playback {
        let (start, end) = crate::timeline_extras::play_span(app);
        crate::timeline_extras::play_range(app, app.frame, start, end);
    }
    app.recorder.take =
        Some(Take { peg, source, start: app.frame, from, offset: (0.0, 0.0), moved: false, keys: BTreeMap::new(), last: None, started_playback });
    sample(app);
    app.set_status(tl!("Recording motion: Escape drops the take"));
}

fn stop_playback_if_ours(app: &mut ToonApp, t: &Take) {
    if t.started_playback {
        app.playback.stop();
        app.sound.stop();
    }
}

/// End the take as one undo step.
fn commit(app: &mut ToonApp) {
    let Some(t) = app.recorder.take.take() else { return };
    stop_playback_if_ours(app, &t);
    let n = t.keys.len();
    if n == 0 || !t.moved {
        app.set_status(tl!("Nothing moved: no motion recorded"));
        return;
    }
    if app.run(Command::SetPegKeys { peg: t.peg, keys: t.keys.into_values().collect() }) {
        app.set_status(tlf!("Recorded motion on {n} frames", n = n));
    }
}

/// Drop the take: the project stays as it was.
fn discard(app: &mut ToonApp) {
    if let Some(t) = app.recorder.take.take() {
        stop_playback_if_ours(app, &t);
        app.set_status(tl!("Take dropped"));
    }
}

/// Once per UI frame, after the playback clock: key the frame shown, and end the take when
/// playback stopped, the pointer let go or the peg went away.
pub fn tick(app: &mut ToonApp, ctx: &egui::Context) {
    let Some(t) = app.recorder.take.as_ref() else { return };
    if app.engine.project.peg(t.peg).is_none() {
        app.recorder.take = None;
        return;
    }
    let released = t.source == Source::Pointer && !ctx.input(|i| i.pointer.any_down());
    if app.playback.playing {
        sample(app);
    }
    if released || !app.playback.playing {
        commit(app);
    }
}

/// Pointer input with the Transform tool. Returns whether recording took the pointer (armed,
/// and the layer hangs from a peg).
pub fn input(app: &mut ToonApp, ui: &Ui, resp: &egui::Response, canvas: &dyn Fn(Pos2) -> (f32, f32)) -> bool {
    if !app.recorder.armed && !app.recorder.recording() {
        return false;
    }
    if app.recorder.recording() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        discard(app);
        return true;
    }
    let Some(id) = crate::peg_transform::target(app) else { return false };
    if resp.drag_started()
        && !app.recorder.recording()
        && let Some(p) = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos())
    {
        begin(app, id, Source::Pointer, canvas(p));
    }
    if let Some(p) = resp.interact_pointer_pos()
        && let Some(t) = app.recorder.take.as_mut()
        && t.source == Source::Pointer
    {
        let at = canvas(p);
        t.offset = (at.0 - t.from.0, at.1 - t.from.1);
        t.moved |= t.offset != (0.0, 0.0);
    }
    true
}

/// Arrow keys with the canvas focused while the shot plays and recording is armed. Returns
/// whether it used a key.
pub fn keyboard(app: &mut ToonApp, ui: &Ui) -> bool {
    if app.recorder.recording() && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        discard(app);
        return true;
    }
    if !app.recorder.armed || !app.playback.playing {
        return false;
    }
    let Some(id) = crate::peg_transform::target(app) else { return false };
    let (dx, dy) = ui.input_mut(|i| {
        use egui::Key::*;
        use egui::Modifiers as M;
        let mut n = |m: M, k| i.consume_key(m, k) as i32 as f32;
        let dx = 10.0 * (n(M::SHIFT, ArrowRight) - n(M::SHIFT, ArrowLeft)) + n(M::NONE, ArrowRight) - n(M::NONE, ArrowLeft);
        let dy = 10.0 * (n(M::SHIFT, ArrowDown) - n(M::SHIFT, ArrowUp)) + n(M::NONE, ArrowDown) - n(M::NONE, ArrowUp);
        (dx, dy)
    });
    if dx == 0.0 && dy == 0.0 {
        return false;
    }
    if !app.recorder.recording() {
        begin(app, id, Source::Keyboard, (0.0, 0.0));
    }
    if let Some(t) = app.recorder.take.as_mut() {
        t.offset = (t.offset.0 + dx, t.offset.1 + dy);
        t.moved = true;
    }
    sample(app);
    true
}

/// The pegs with the take's key at the current frame in place, while one is being recorded (the
/// canvas shows the layer where the take puts it). Only the frame shown needs it, so a long take
/// costs one key per UI frame, not the whole take.
pub fn previewed_pegs(app: &ToonApp) -> Option<Vec<tooncraft_doc::Peg>> {
    let t = app.recorder.take.as_ref()?;
    let now = key_at(app, t, app.frame)?;
    let mut pegs = app.engine.project.pegs.clone();
    pegs.iter_mut().find(|q| q.id == t.peg)?.set_key(now);
    Some(pegs)
}

/// The record mark over the canvas: dim while armed, bright while a take records.
pub fn overlay(app: &ToonApp, painter: &egui::Painter, rect: egui::Rect, text: egui::Color32, back: egui::Color32) {
    if !app.recorder.armed && !app.recorder.recording() {
        return;
    }
    let on = app.recorder.recording();
    let red = if on { egui::Color32::from_rgb(220, 40, 40) } else { egui::Color32::from_rgb(140, 60, 60) };
    let at = rect.right_top() + egui::vec2(-130.0, 18.0);
    painter.rect_filled(egui::Rect::from_min_size(at - egui::vec2(12.0, 11.0), egui::vec2(122.0, 22.0)), 11.0, back);
    painter.circle_filled(at, 6.0, red);
    let label = if on { tl!("Recording") } else { tl!("Record armed") };
    painter.text(at + egui::vec2(12.0, 0.0), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(13.0), text);
}

/// Arm or disarm recording (the registry's toggle and the tool option).
pub fn set_armed(app: &mut ToonApp, armed: bool) {
    if !armed {
        commit(app);
    }
    app.recorder.armed = armed;
    app.set_status(if armed {
        tl!("Record motion armed: with the Transform tool, drag the layer and it records while the shot plays")
    } else {
        tl!("Record motion off")
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(frame: u32, x: f32) -> PegKey {
        PegKey { x, ..PegKey::rest(frame) }
    }

    #[test]
    fn skipped_frames_are_filled_part_way_and_a_wrap_does_not_fill_backwards() {
        let mut keys = BTreeMap::new();
        assert!(record(&mut keys, None, k(2, 0.0)));
        assert!(record(&mut keys, Some(2), k(6, 40.0)));
        let xs: Vec<(u32, f32)> = keys.values().map(|k| (k.frame, k.x)).collect();
        assert_eq!(xs, [(2, 0.0), (3, 10.0), (4, 20.0), (5, 30.0), (6, 40.0)]);
        assert!(record(&mut keys, Some(6), k(0, 99.0)));
        assert_eq!(keys.len(), 6, "after a wrap only the frame shown is keyed");
        assert!(record(&mut keys, Some(0), k(2, 5.0)));
        assert_eq!(keys.get(&2).map(|k| k.x), Some(5.0), "a later pass replaces a frame");
        assert_eq!(keys.get(&1).map(|k| k.x), Some(52.0), "and fills from the frame before");
    }

    #[test]
    fn a_take_stops_at_its_key_limit() {
        let mut keys = BTreeMap::new();
        assert!(record(&mut keys, None, k(0, 0.0)));
        assert!(!record(&mut keys, Some(0), k(MAX_TAKE as u32 + 5, 1.0)));
        assert_eq!(keys.len(), MAX_TAKE);
        assert!(record(&mut keys, Some(3), k(3, 7.0)), "a frame already in the take is replaced");
    }
}
