//! Test shots.

use tooncraft_doc::{Cell, DrawingId, Project};
use tooncraft_raster::{RgbaImage, Surface};

/// A drawing with one opaque `w`×`h` rectangle of `color` at `(x, y)`.
pub fn rect_drawing(p: &mut Project, x: u32, y: u32, w: u32, h: u32, color: [u8; 3]) -> DrawingId {
    let mut img = RgbaImage::transparent(p.width(), p.height());
    for yy in y..y + h {
        for xx in x..x + w {
            img.set_pixel(xx, yy, [color[0], color[1], color[2], 255]);
        }
    }
    let mut s = Surface::new(p.width(), p.height()).unwrap();
    s.load_image(&img).unwrap();
    p.add_drawing(Some(s)).unwrap()
}

/// 40×30, 6 frames, two layers:
/// - "Ink" (bottom): A A B B A -  (A red, B green), opacity 0.5
/// - "Colour" (top): C C C C C C (blue), hidden
pub fn shot() -> Project {
    let mut p = Project::new("Test shot", 40, 30, 12, 6).unwrap();
    p.layers[0].name = "Ink".into();
    let a = rect_drawing(&mut p, 2, 3, 10, 6, [255, 0, 0]);
    let b = rect_drawing(&mut p, 20, 10, 5, 5, [0, 255, 0]);
    p.layers[0].cells = vec![Cell::Key(a), Cell::Hold, Cell::Key(b), Cell::Hold, Cell::Key(a), Cell::Blank];
    p.layers[0].opacity = 0.5;
    p.add_layer("Colour", 1).unwrap();
    let c = rect_drawing(&mut p, 0, 0, 4, 4, [0, 0, 255]);
    p.layers[1].cells = vec![Cell::Key(c), Cell::Hold, Cell::Hold, Cell::Hold, Cell::Hold, Cell::Hold];
    p.layers[1].visible = false;
    p
}

/// A fresh scratch directory under the target directory.
pub fn temp_dir(name: &str) -> std::path::PathBuf {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/interop-tests").join(format!("{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}
