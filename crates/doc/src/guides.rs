//! Framing guides drawn over the canvas: the field guide (a grid of nested fields around the
//! centre, as on a traditional animation field chart) and the action-safe and title-safe areas.

/// Fields on the field guide (12 is the whole frame).
pub const FIELDS: u32 = 12;
/// Share of the frame inside the action-safe area.
pub const ACTION_SAFE: f32 = 0.9;
/// Share of the frame inside the title-safe area.
pub const TITLE_SAFE: f32 = 0.8;

/// A rectangle `(x0, y0, x1, y1)` in canvas pixels.
pub type Rect = (f32, f32, f32, f32);

/// The centred rectangle covering `share` of a `w`×`h` frame in each direction.
pub fn centred(w: u32, h: u32, share: f32) -> Rect {
    let share = if share.is_finite() { share.clamp(0.0, 1.0) } else { 1.0 };
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let (hw, hh) = (cx * share, cy * share);
    (cx - hw, cy - hh, cx + hw, cy + hh)
}

/// The field rectangles 1…[`FIELDS`] (field `n` covers `n / FIELDS` of the frame), smallest
/// first.
pub fn fields(w: u32, h: u32) -> Vec<Rect> {
    (1..=FIELDS).map(|n| centred(w, h, n as f32 / FIELDS as f32)).collect()
}

/// The x positions of the field guide's vertical grid lines and the y positions of its
/// horizontal ones (one per field step on each side of the centre).
pub fn grid(w: u32, h: u32) -> (Vec<f32>, Vec<f32>) {
    let step_x = w as f32 / 2.0 / FIELDS as f32;
    let step_y = h as f32 / 2.0 / FIELDS as f32;
    let xs = (0..=2 * FIELDS).map(|i| i as f32 * step_x).collect();
    let ys = (0..=2 * FIELDS).map(|i| i as f32 * step_y).collect();
    (xs, ys)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guides_are_centred_and_nested() {
        assert_eq!(centred(1920, 1080, ACTION_SAFE), (96.0, 54.0, 1824.0, 1026.0));
        assert_eq!(centred(1920, 1080, TITLE_SAFE), (192.0, 108.0, 1728.0, 972.0));
        let f = fields(1200, 600);
        assert_eq!(f.len(), 12);
        assert_eq!(f[11], (0.0, 0.0, 1200.0, 600.0));
        assert_eq!(f[5], (300.0, 150.0, 900.0, 450.0));
        let (xs, ys) = grid(1200, 600);
        assert_eq!((xs.len(), xs[12], ys[24]), (25, 600.0, 600.0));
        assert_eq!(centred(10, 10, f32::NAN), (0.0, 0.0, 10.0, 10.0));
    }
}
