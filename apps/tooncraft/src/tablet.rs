//! Pen tablets: pressure, tilt, barrel rotation, the eraser end and the side buttons, read beside
//! winit (which drops them) through PhotoCraft's `photocraft-tablet` crate:
//!
//! - **X11** (including Xwayland): XInput2 raw events on a second X connection.
//! - **macOS**: an AppKit local event monitor.
//! - **Windows**: with Windows Ink (the default), winit reports the pen's force as touch force,
//!   which the canvas reads directly; thread-local pointer hooks add the barrel button and which
//!   end of the pen is down, so its eraser end erases and its side button works. (The hooks do
//!   not say which touch is the pen, so palm rejection needs WinTab mode there.) With Preferences ▸ Pen ▸ "WinTab" (Windows Ink off for the tablet in its driver), a
//!   WinTab 1.4 context on the window supplies pressure, tilt, rotation and the eraser end too.
//! - **Wayland-native windows** get no pen data: the tablet protocol needs `unsafe` interop on
//!   winit's own connection, which ToonCraft does not allow. So on a Wayland session with a pen
//!   attached the window opens through Xwayland instead (`display_session.rs`).
//!
//! Side buttons: X11 and macOS drivers send a pen's side buttons as the middle and right mouse
//! buttons; on Windows the barrel button comes from the pointer hook. While the pen is near, a
//! side button can run any command instead of panning or opening the canvas menu
//! ([`SideButton`], Preferences ▸ Pen).

use std::sync::{Arc, Mutex};

/// One pen reading, normalised for the brush engine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pen {
    /// 0 … 1.
    pub pressure: f32,
    /// Tilt towards +x / +y, -1 … 1.
    pub tilt_x: f32,
    pub tilt_y: f32,
    /// Barrel rotation in turns, 0 … 1 clockwise (pens with a rotating barrel; 0 otherwise).
    pub rotation: f32,
    /// The eraser end is down.
    pub eraser: bool,
}

impl Pen {
    fn from_sample(s: photocraft_tablet::Sample) -> Pen {
        let s = s.sanitized();
        Pen { pressure: s.pressure, tilt_x: s.tilt_x / 90.0, tilt_y: s.tilt_y / 90.0, rotation: s.rotation / 360.0, eraser: s.eraser }
    }
}

/// A pen side button, as the window system reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SideButton {
    /// The button drivers send as a middle click (the lower one on most pens; Windows' barrel
    /// button).
    Lower,
    /// The button drivers send as a right click (the upper one on most pens).
    Upper,
}

impl SideButton {
    pub const ALL: [SideButton; 2] = [SideButton::Lower, SideButton::Upper];

    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Default)]
pub struct Tablet {
    latest: Arc<Mutex<Option<Pen>>>,
    /// Why there is no pen input, when there is none.
    pub status: String,
    /// Which backend reads the pen (shown in Preferences ▸ Pen).
    pub backend: &'static str,
    #[cfg(target_os = "macos")]
    _monitor: Option<photocraft_tablet::macos::Monitor>,
    #[cfg(target_os = "windows")]
    _hooks: Option<photocraft_tablet::windows::Monitor>,
    #[cfg(target_os = "windows")]
    wintab: bool,
    /// The barrel button as last seen, to turn the Windows button state into presses.
    barrel_was_down: bool,
    /// Simulated Windows-style state (tests): the pen near, its eraser end, the barrel button.
    #[cfg(test)]
    sim: std::cell::Cell<(bool, bool, bool)>,
}

impl Tablet {
    /// Start listening (call once, on the UI thread).
    pub fn start(ctx: &egui::Context) -> Self {
        let mut t = Tablet::default();
        let latest = t.latest.clone();
        let ctx2 = ctx.clone();
        let _callback = move |s: Option<photocraft_tablet::Sample>| {
            let pen = s.map(Pen::from_sample);
            match latest.lock() {
                Ok(mut g) => *g = pen,
                Err(p) => *p.into_inner() = pen,
            }
            ctx2.request_repaint();
        };
        #[cfg(target_os = "linux")]
        match photocraft_tablet::x11::spawn(None, _callback) {
            Ok(_) => t.backend = "XInput2",
            Err(e) => t.status = e.to_string(),
        }
        #[cfg(target_os = "macos")]
        match photocraft_tablet::macos::Monitor::install(_callback) {
            Ok(m) => {
                t._monitor = Some(m);
                t.backend = "AppKit";
            }
            Err(e) => t.status = e.to_string(),
        }
        #[cfg(target_os = "windows")]
        match photocraft_tablet::windows::Monitor::install() {
            Ok(m) => {
                if !m.tracks_leave() {
                    log::warn!("tablet input: the WH_CALLWNDPROC hook did not install, so the pen leaving the window may be missed");
                }
                t._hooks = Some(m);
                t.backend = "Windows Ink";
            }
            Err(e) => t.status = e.to_string(),
        }
        t
    }

    /// Windows: read pressure, tilt, rotation and the eraser end through WinTab on the app's
    /// window (Windows Ink off for the tablet). On failure nothing changes; the reason is in
    /// [`Tablet::status`].
    #[cfg(target_os = "windows")]
    pub fn start_wintab(&mut self, cc: &eframe::CreationContext<'_>) {
        use eframe::wgpu::rwh::{HasWindowHandle, RawWindowHandle};
        let hwnd = match cc.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(h)) => h.hwnd.get(),
            _ => {
                self.status = "WinTab: no Win32 window".into();
                return;
            }
        };
        let ctx = cc.egui_ctx.clone();
        match photocraft_tablet::wintab::install(hwnd, Box::new(move || ctx.request_repaint())) {
            Ok(()) => {
                self.wintab = true;
                self.backend = "WinTab";
            }
            Err(e) => self.status = format!("WinTab: {e}"),
        }
    }

    /// Once per frame on the UI thread: WinTab's queued packets (Windows, WinTab mode).
    pub fn poll(&mut self, _focused: bool) {
        #[cfg(target_os = "windows")]
        if self.wintab {
            let latest = self.latest.clone();
            photocraft_tablet::wintab::poll(_focused, &|s| {
                let pen = s.map(Pen::from_sample);
                match latest.lock() {
                    Ok(mut g) => *g = pen,
                    Err(p) => *p.into_inner() = pen,
                }
            });
        }
    }

    /// The pen's current reading, `None` while the pointer is a mouse (and with Windows Ink,
    /// where the pressure arrives as touch force instead).
    pub fn pen(&self) -> Option<Pen> {
        match self.latest.lock() {
            Ok(g) => *g,
            Err(p) => *p.into_inner(),
        }
    }

    /// Whether a pen is near the window (hovering or touching), from a reading or, on Windows,
    /// the pointer hooks.
    pub fn pen_near(&self) -> bool {
        self.pen().is_some() || self.windows_state().0
    }

    /// Whether the pen's eraser end is the one in use.
    pub fn eraser_end(&self) -> bool {
        self.pen().is_some_and(|p| p.eraser) || self.windows_state().1
    }

    /// A side button pushed since the last call, for platforms that report a button state
    /// rather than mouse clicks (Windows' barrel button). Call once per frame.
    pub fn take_press(&mut self) -> Option<SideButton> {
        let down = self.windows_state().2;
        let pressed = down && !self.barrel_was_down;
        self.barrel_was_down = down;
        pressed.then_some(SideButton::Lower)
    }

    /// (pen near, eraser end, barrel button down) from the Windows pointer hooks.
    fn windows_state(&self) -> (bool, bool, bool) {
        #[cfg(test)]
        {
            self.sim.get()
        }
        #[cfg(all(not(test), target_os = "windows"))]
        {
            let end = photocraft_tablet::windows::pen_end();
            (end.is_some(), end == Some(true), photocraft_tablet::windows::is_barrel_button_down())
        }
        #[cfg(all(not(test), not(target_os = "windows")))]
        {
            (false, false, false)
        }
    }

    /// Set a reading by hand (tests).
    #[cfg(test)]
    pub fn set(&self, pen: Option<Pen>) {
        if let Ok(mut g) = self.latest.lock() {
            *g = pen;
        }
    }

    /// Simulate the Windows pointer hooks (tests): pen near, eraser end, barrel button down.
    #[cfg(test)]
    pub fn simulate_windows(&self, near: bool, eraser: bool, barrel: bool) {
        self.sim.set((near, eraser, barrel));
    }
}

/// What a side button press on the canvas does: `Some(command id)` to run, or `None` for the
/// usual behaviour (pan for the middle button, the canvas menu for the right one).
pub fn side_button_command(mapping: &[String; 2], b: SideButton) -> Option<&str> {
    let id = mapping.get(b.index())?.as_str();
    (!id.is_empty() && crate::commands::get(id).is_some()).then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_barrel_state_becomes_one_press_per_push() {
        let mut t = Tablet::default();
        assert_eq!(t.take_press(), None);
        t.simulate_windows(true, false, true);
        assert_eq!(t.take_press(), Some(SideButton::Lower));
        assert_eq!(t.take_press(), None, "held down is one press");
        t.simulate_windows(true, false, false);
        assert_eq!(t.take_press(), None);
        t.simulate_windows(true, true, true);
        assert_eq!(t.take_press(), Some(SideButton::Lower));
        assert!(t.pen_near() && t.eraser_end());
    }

    #[test]
    fn only_known_commands_are_mapped() {
        let m = ["edit.undo".to_string(), "no.such.command".to_string()];
        assert_eq!(side_button_command(&m, SideButton::Lower), Some("edit.undo"));
        assert_eq!(side_button_command(&m, SideButton::Upper), None);
        assert_eq!(side_button_command(&[String::new(), String::new()], SideButton::Lower), None);
    }
}
