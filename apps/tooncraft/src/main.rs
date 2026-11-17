//! ToonCraft desktop app: hand-drawn, frame-by-frame animation.
//!
//! Usage: `tooncraft [--demo] [--frame N] [--screenshot <file.png>] [--bench-playback <s>] [file.toon]`;
//! `--help` prints the options.
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]
#![forbid(unsafe_code)]

#[macro_use]
mod i18n;
mod app;
mod args;
mod brush_library;
mod canvas;
mod color;
mod dock;
mod effects_panel;
mod icons;
mod keys;
mod layer_tree;
mod lipsync_panel;
mod panels;
mod pegs;
mod sound;
mod sound_panel;
mod tablet;
mod textures;
mod theme;
mod timeline;
#[cfg(test)]
mod ui_tests;
mod view;

use args::{Cli, Launch};

/// Reverse-DNS app id: Linux `.desktop` file, hicolor icon name and Wayland app id.
pub const APP_ID: &str = "ai.storyteller.tooncraft";

fn main() -> eframe::Result {
    let cli: Cli = match args::parse(std::env::args().skip(1)) {
        Ok(Launch::Run(cli)) => cli,
        Ok(Launch::Print(text)) => {
            print!("{text}");
            return Ok(());
        }
        Err(e) => {
            eprintln!("tooncraft: {e}\n\n{}", args::USAGE);
            std::process::exit(2);
        }
    };
    let size = if cli.screenshot.is_some() { [1600.0, 1000.0] } else { [1500.0, 940.0] };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("ToonCraft").with_app_id(APP_ID).with_inner_size(size).with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native("ToonCraft", options, Box::new(move |cc| Ok(Box::new(app::ToonApp::new(cc, cli)))))
}
