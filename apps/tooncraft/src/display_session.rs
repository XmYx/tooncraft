//! Which display server the window opens on.
//!
//! Wayland compositors give a pen only to windows that bind the tablet protocol, which winit
//! 0.30 does not do (and binding it on winit's own connection would need `unsafe`, which
//! ToonCraft does not allow). Without it the pen does nothing in a Wayland-native window. Xwayland
//! binds the protocol and serves the pen as an X input device, where ToonCraft's tablet reader
//! gets pressure, tilt, rotation and the eraser end. So on a Wayland session with a pen attached
//! the window opens through Xwayland, when Xwayland is there (`DISPLAY` is set).
//! `TOONCRAFT_NATIVE_WAYLAND=1` keeps the Wayland-native window.
//!
//! The pen is found in the kernel's input device list (`/proc/bus/input/devices`): a device whose
//! key bits include the pen tool (`BTN_TOOL_PEN`, code 0x140) and whose absolute axes include
//! pressure (`ABS_PRESSURE`, code 0x18).

/// The input event code of the pen tool key.
const BTN_TOOL_PEN: u32 = 0x140;
/// The input event code of the pressure axis.
const ABS_PRESSURE: u32 = 0x18;
/// Most bytes of the device list read.
const MAX_LIST: usize = 1 << 20;

/// Whether a bitmap line of the device list (`B: KEY=…`, hex words, most significant first, 64
/// bits a word on 64-bit kernels and 32 on 32-bit ones) has bit `bit` set.
fn has_bit(words: &str, bit: u32, word_bits: u32) -> bool {
    let words: Vec<&str> = words.split_whitespace().collect();
    let (index, offset) = ((bit / word_bits) as usize, bit % word_bits);
    let Some(word) = words.len().checked_sub(index + 1).and_then(|i| words.get(i)) else { return false };
    u64::from_str_radix(word, 16).is_ok_and(|w| offset < 64 && (w >> offset) & 1 == 1)
}

/// Whether the kernel's input device list shows a pen with pressure.
pub fn has_pen(devices: &str) -> bool {
    let word_bits = if cfg!(target_pointer_width = "64") { 64 } else { 32 };
    devices.split("\n\n").take(4096).any(|block| {
        let field = |prefix: &str| block.lines().find_map(|l| l.strip_prefix(prefix)).unwrap_or("");
        has_bit(field("B: KEY="), BTN_TOOL_PEN, word_bits) && has_bit(field("B: ABS="), ABS_PRESSURE, word_bits)
    })
}

/// Whether to open the window through Xwayland: a Wayland session, Xwayland available, a pen
/// attached, and no request to stay native. `var` reads an environment variable.
pub fn use_xwayland(var: impl Fn(&str) -> Option<String>, pen: impl FnOnce() -> bool) -> bool {
    let set = |k: &str| var(k).is_some_and(|v| !v.is_empty());
    let wayland = set("WAYLAND_DISPLAY") || set("WAYLAND_SOCKET");
    let stay = var("TOONCRAFT_NATIVE_WAYLAND").is_some_and(|v| v == "1");
    wayland && set("DISPLAY") && !stay && pen()
}

/// Decide for this process (Linux): reads the environment and the device list once.
#[cfg(target_os = "linux")]
pub fn choose_xwayland() -> bool {
    let pen = || {
        use std::io::Read;
        let mut text = String::new();
        std::fs::File::open("/proc/bus/input/devices").and_then(|f| f.take(MAX_LIST as u64).read_to_string(&mut text)).is_ok() && has_pen(&text)
    };
    let chosen = use_xwayland(|k| std::env::var(k).ok(), pen);
    if chosen {
        eprintln!("tooncraft: a pen is attached; opening the window through Xwayland, where the pen works (TOONCRAFT_NATIVE_WAYLAND=1 keeps Wayland)");
    }
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A device list in the kernel's format: a mouse, then a pen with pressure and tilt.
    const LIST: &str = "I: Bus=0003 Vendor=046d Product=c077 Version=0111
N: Name=\"Some Optical Mouse\"
H: Handlers=mouse0 event3
B: EV=17
B: KEY=1f0000 0 0 0 0
B: REL=903
B: MSC=10

I: Bus=0003 Vendor=056a Product=0374 Version=0110
N: Name=\"Some Tablet Pen\"
H: Handlers=mouse1 event7
B: EV=1b
B: KEY=1c03 0 0 0 0 0
B: ABS=3000003
B: MSC=1
";

    #[test]
    fn a_pen_is_found_in_the_device_list_and_a_mouse_is_not() {
        if cfg!(target_pointer_width = "64") {
            assert!(has_pen(LIST));
            let mouse_only = LIST.split("\n\n").next().unwrap_or("");
            assert!(!has_pen(mouse_only));
        }
        // a pen key without pressure (a stylus that only clicks) does not count
        let no_pressure = LIST.replace("B: ABS=3000003", "B: ABS=3");
        assert!(!has_pen(&no_pressure));
        assert!(!has_pen(""));
        assert!(!has_pen("B: KEY=zz\nB: ABS=ffffffffffffffffffffffff"));
    }

    #[test]
    fn xwayland_only_for_a_wayland_session_with_a_pen_and_xwayland() {
        let env = |pairs: &'static [(&'static str, &'static str)]| move |k: &str| pairs.iter().find(|p| p.0 == k).map(|p| p.1.to_string());
        static WAYLAND: [(&str, &str); 2] = [("WAYLAND_DISPLAY", "wayland-0"), ("DISPLAY", ":0")];
        assert!(use_xwayland(env(&WAYLAND), || true));
        assert!(!use_xwayland(env(&WAYLAND), || false), "no pen: stay native");
        assert!(!use_xwayland(env(&[("WAYLAND_DISPLAY", "wayland-0")]), || true), "no Xwayland");
        assert!(!use_xwayland(env(&[("DISPLAY", ":0")]), || true), "already X11");
        assert!(!use_xwayland(env(&[("WAYLAND_DISPLAY", "wayland-0"), ("DISPLAY", ":0"), ("TOONCRAFT_NATIVE_WAYLAND", "1")]), || true), "asked to stay");
        // the pen is only looked for when it matters
        let looked = std::cell::Cell::new(false);
        let _ = use_xwayland(env(&[("DISPLAY", ":0")]), || {
            looked.set(true);
            true
        });
        assert!(!looked.get());
    }
}
