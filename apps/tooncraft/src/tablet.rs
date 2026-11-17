//! Pen tablets: pressure, tilt and the eraser end, read beside winit (which drops them on Linux
//! and macOS) through PhotoCraft's `photocraft-tablet` crate: XInput2 raw events on X11
//! (including Xwayland), an AppKit event monitor on macOS. On Windows, winit reports pen force
//! as touch force, which the canvas reads directly. Wayland-native windows get no pen data yet.

use std::sync::{Arc, Mutex};

/// One pen reading, normalised for the brush engine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pen {
    /// 0 … 1.
    pub pressure: f32,
    /// Tilt towards +x / +y, -1 … 1.
    pub tilt_x: f32,
    pub tilt_y: f32,
    /// The eraser end is down.
    pub eraser: bool,
}

#[derive(Default)]
pub struct Tablet {
    latest: Arc<Mutex<Option<Pen>>>,
    /// Why there is no pen input, when there is none.
    pub status: String,
    #[cfg(target_os = "macos")]
    _monitor: Option<photocraft_tablet::macos::Monitor>,
}

impl Tablet {
    /// Start listening (call once, on the UI thread).
    pub fn start(ctx: &egui::Context) -> Self {
        let mut t = Tablet::default();
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let latest = t.latest.clone();
            let ctx = ctx.clone();
            let callback = move |s: Option<photocraft_tablet::Sample>| {
                let pen = s.map(|s| {
                    let s = s.sanitized();
                    Pen { pressure: s.pressure, tilt_x: s.tilt_x / 90.0, tilt_y: s.tilt_y / 90.0, eraser: s.eraser }
                });
                match latest.lock() {
                    Ok(mut g) => *g = pen,
                    Err(p) => *p.into_inner() = pen,
                }
                ctx.request_repaint();
            };
            #[cfg(target_os = "linux")]
            match photocraft_tablet::x11::spawn(None, callback) {
                Ok(_) => {}
                Err(e) => t.status = e.to_string(),
            }
            #[cfg(target_os = "macos")]
            match photocraft_tablet::macos::Monitor::install(callback) {
                Ok(m) => t._monitor = Some(m),
                Err(e) => t.status = e.to_string(),
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let _ = ctx;
        t
    }

    /// The pen's current state, `None` while the pointer is a mouse.
    pub fn pen(&self) -> Option<Pen> {
        match self.latest.lock() {
            Ok(g) => *g,
            Err(p) => *p.into_inner(),
        }
    }

    /// Set a reading by hand (tests).
    #[cfg(test)]
    pub fn set(&self, pen: Option<Pen>) {
        if let Ok(mut g) = self.latest.lock() {
            *g = pen;
        }
    }
}
