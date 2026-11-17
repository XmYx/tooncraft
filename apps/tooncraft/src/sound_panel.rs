//! The Sound panel: the shot's audio tracks (mute, solo, volume) and its clips (track, start
//! frame, trim, fades, volume and volume keys, mute). Every change is an engine command.

use egui::vec2;
use tooncraft_engine::Command;

use crate::app::ToonApp;

fn num_f64(ui: &mut egui::Ui, label: &str, v: &mut f64, range: std::ops::RangeInclusive<f64>, speed: f64, suffix: &str) -> bool {
    let l = ui.label(label);
    let r = ui.add(egui::DragValue::new(v).range(range).speed(speed).max_decimals(2).suffix(suffix)).labelled_by(l.id).changed();
    ui.end_row();
    r
}

fn num_f32(ui: &mut egui::Ui, label: &str, v: &mut f32, range: std::ops::RangeInclusive<f32>) -> bool {
    let l = ui.label(label);
    let r = ui.add(egui::DragValue::new(v).range(range).speed(0.01).max_decimals(2)).labelled_by(l.id).changed();
    ui.end_row();
    r
}

/// The fields of one clip edit.
#[derive(Default)]
struct Change {
    track: Option<u32>,
    offset: Option<i32>,
    trim_start: Option<f64>,
    length: Option<f64>,
    to_end: bool,
    fade_in: Option<f64>,
    fade_out: Option<f64>,
    volume: Option<f32>,
    muted: Option<bool>,
}

impl Change {
    fn command(self, index: usize) -> Option<Command> {
        let any = self.track.is_some()
            || self.offset.is_some()
            || self.trim_start.is_some()
            || self.length.is_some()
            || self.to_end
            || self.fade_in.is_some()
            || self.fade_out.is_some()
            || self.volume.is_some()
            || self.muted.is_some();
        any.then_some(Command::SetSoundClip {
            index,
            track: self.track,
            offset: self.offset,
            trim_start: self.trim_start,
            length: self.length,
            to_end: self.to_end,
            fade_in: self.fade_in,
            fade_out: self.fade_out,
            volume: self.volume,
            muted: self.muted,
        })
    }
}

fn tracks(app: &mut ToonApp, ui: &mut egui::Ui) {
    let used = app.engine.project.sounds.iter().map(|s| s.track + 1).max().unwrap_or(0).max(app.engine.project.audio_tracks.len() as u32);
    for t in 0..used {
        let at = app.engine.project.audio_tracks.get(t as usize).cloned().unwrap_or_default();
        let name = if at.name.is_empty() { tlf!("Track {n}", n = t + 1) } else { at.name.clone() };
        ui.horizontal(|ui| {
            ui.label(&name);
            let mut m = at.muted;
            if ui.checkbox(&mut m, tlf!("Mute {track}", track = name)).changed() {
                app.run(Command::SetAudioTrack { track: t, name: None, muted: Some(m), solo: None, volume: None });
            }
            let mut s = at.solo;
            if ui.checkbox(&mut s, tlf!("Solo {track}", track = name)).changed() {
                app.run(Command::SetAudioTrack { track: t, name: None, muted: None, solo: Some(s), volume: None });
            }
            let mut v = at.volume;
            let l = ui.label(tl!("Volume"));
            if ui.add(egui::DragValue::new(&mut v).range(0.0..=4.0).speed(0.01).max_decimals(2)).labelled_by(l.id).changed() {
                app.run(Command::SetAudioTrack { track: t, name: None, muted: None, solo: None, volume: Some(v) });
            }
        });
    }
}

fn clip(app: &mut ToonApp, ui: &mut egui::Ui, i: usize) {
    let Some(c) = app.engine.project.sounds.get(i).cloned() else { return };
    let frame = app.frame;
    let title = tlf!("Clip {n}: {name}", n = i + 1, name = c.name);
    ui.horizontal(|ui| {
        ui.strong(&title);
        let mut m = c.muted;
        if ui.checkbox(&mut m, tlf!("Mute clip {n}", n = i + 1)).changed()
            && let Some(cmd) = (Change { muted: Some(m), ..Default::default() }).command(i)
        {
            app.run(cmd);
        }
        if ui.small_button(tlf!("Remove clip {n}", n = i + 1)).clicked() {
            app.run(Command::RemoveSound { index: i });
        }
    });
    let (mut track, mut start, mut trim, mut fi, mut fo) = (c.track as f64 + 1.0, c.offset as f64 + 1.0, c.trim_start, c.fade_in, c.fade_out);
    let mut len = c.length.unwrap_or(0.0);
    let mut vol = c.gain_at(frame as f64);
    let mut ch = Change::default();
    let mut key_volume = None;
    egui::Grid::new(("sound-clip", i)).num_columns(2).spacing(vec2(8.0, 4.0)).show(ui, |ui| {
        if num_f64(ui, tl!("Track"), &mut track, 1.0..=tooncraft_doc::sound::MAX_AUDIO_TRACKS as f64, 0.05, "") {
            ch.track = Some(track.round().max(1.0) as u32 - 1);
        }
        if num_f64(ui, tl!("Starts at frame"), &mut start, -1_000_000.0..=1_000_000.0, 0.2, "") {
            ch.offset = Some(start.round() as i32 - 1);
        }
        if num_f64(ui, tl!("Trim start"), &mut trim, 0.0..=7200.0, 0.01, " s") {
            ch.trim_start = Some(trim);
        }
        if num_f64(ui, tl!("Length (0: to the end)"), &mut len, 0.0..=7200.0, 0.01, " s") {
            if len <= 0.0 {
                ch.to_end = true;
            } else {
                ch.length = Some(len);
            }
        }
        if num_f64(ui, tl!("Fade in"), &mut fi, 0.0..=7200.0, 0.01, " s") {
            ch.fade_in = Some(fi);
        }
        if num_f64(ui, tl!("Fade out"), &mut fo, 0.0..=7200.0, 0.01, " s") {
            ch.fade_out = Some(fo);
        }
        if num_f32(ui, tl!("Clip volume here"), &mut vol, 0.0..=4.0) {
            if c.volume_keys.is_empty() {
                ch.volume = Some(vol);
            } else {
                key_volume = Some(vol);
            }
        }
    });
    let cmd = if let Some(gain) = key_volume { Some(Command::SetVolumeKey { index: i, frame, gain }) } else { ch.command(i) };
    if let Some(cmd) = cmd {
        app.run(cmd);
    }
    let key_here = c.volume_keys.iter().any(|k| k.frame == frame);
    ui.horizontal(|ui| {
        if ui.button(tlf!("Key clip {n} volume here", n = i + 1)).clicked() {
            app.run(Command::SetVolumeKey { index: i, frame, gain: c.gain_at(frame as f64) });
        }
        if ui.add_enabled(key_here, egui::Button::new(tlf!("Remove clip {n} volume key", n = i + 1))).clicked() {
            app.run(Command::RemoveVolumeKey { index: i, frame });
        }
        if !c.volume_keys.is_empty() {
            ui.weak(tlf!("{n} keys", n = c.volume_keys.len()));
        }
    });
}

pub fn show(app: &mut ToonApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if ui.button(tl!("Add Sound…")).clicked() {
            app.import_sound();
        }
        if ui.add_enabled(!app.engine.project.sounds.is_empty(), egui::Button::new(tl!("Export Sound Mix…"))).clicked()
            && let Some(f) = rfd::FileDialog::new().add_filter("WAV", &["wav"]).set_file_name("mix.wav").save_file()
        {
            match tooncraft_engine::audio::export_wav(&app.engine.project, &f, 0, app.engine.project.length()) {
                Ok(()) => app.set_status(tlf!("Wrote {file}", file = f.display())),
                Err(e) => app.set_status(e),
            }
        }
    });
    if !app.sound.error.is_empty() {
        ui.colored_label(egui::Color32::from_rgb(220, 120, 90), &app.sound.error);
    }
    if app.engine.project.sounds.is_empty() {
        ui.weak(tl!("No sound in this shot."));
        return;
    }
    egui::ScrollArea::vertical().id_salt("sound-panel").show(ui, |ui| {
        ui.heading(tl!("Audio tracks"));
        tracks(app, ui);
        ui.separator();
        for i in 0..app.engine.project.sounds.len() {
            ui.push_id(("clip", i), |ui| clip(app, ui, i));
            ui.separator();
        }
    });
}
