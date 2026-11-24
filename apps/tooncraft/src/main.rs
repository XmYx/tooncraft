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
mod brush_editor;
mod brush_library;
mod brush_presets;
mod canvas;
mod canvas_header;
mod canvas_tools;
mod color;
mod colour_layers_ui;
#[cfg(test)]
mod colour_layers_ui_tests;
mod colour_tools;
#[cfg(test)]
mod colour_ui_tests;
mod command_search;
mod commands;
mod context_menus;
#[cfg(test)]
mod context_menus_ui_tests;
mod curves;
#[cfg(test)]
mod curves_ui_tests;
mod dock;
mod draw_aids;
#[cfg(test)]
mod drawaids_ui_tests;
mod drawing_library;
#[cfg(test)]
mod drawing_library_ui_tests;
mod effects_panel;
mod export_dialog;
mod frame_cache;
#[cfg(test)]
mod fx_ui_tests;
#[cfg(test)]
mod gaps_ui_tests;
#[cfg(test)]
mod guidance_ui_tests;
mod history_panel;
#[cfg(test)]
mod history_ui_tests;
mod icons;
mod import_dialog;
mod keys;
mod layer_ops;
mod layer_tree;
mod layers_list;
mod lipsync_panel;
mod menu_actions;
#[cfg(test)]
mod menu_actions_tests;
mod menu_commands;
mod menus;
#[cfg(test)]
mod menus_ui_tests;
mod onion_popover;
mod panel_chrome;
mod panel_views;
mod panels;
#[cfg(test)]
mod panels_ui_tests;
mod peg_transform;
#[cfg(test)]
mod peg_transform_ui_tests;
mod pegs;
mod pipeline;
mod preferences;
mod rulers_ui;
#[cfg(test)]
mod rulers_ui_tests;
mod scene_menus;
mod scene_presets;
#[cfg(test)]
mod scene_ui_tests;
mod selection_tools;
#[cfg(test)]
mod selection_ui_tests;
mod sheet;
#[cfg(test)]
mod shell_chrome_ui_tests;
#[cfg(test)]
mod shell_ui_tests;
mod sound;
mod sound_panel;
mod start_screen;
mod status_strip;
#[cfg(test)]
mod swatch_ui_tests;
mod tablet;
#[cfg(test)]
mod tcui_time_ui_tests;
mod text_tool;
#[cfg(test)]
mod text_ui_tests;
mod textures;
mod theme;
mod timeline;
mod timeline_extras;
mod timeline_header;
mod timeline_rows;
#[cfg(test)]
mod timeline_rows_ui_tests;
mod timeline_ruler;
#[cfg(test)]
mod timeline_ui_tests;
mod timing_panel;
mod tool_options;
mod tool_rail;
mod top_bar;
#[cfg(test)]
mod touch_ui_tests;
#[cfg(test)]
mod ui_tests;
mod unsaved;
#[cfg(test)]
mod unsaved_ui_tests;
#[cfg(test)]
mod vector_ui_tests;
mod view;
mod workspaces;

use args::{Cli, Launch};

/// Reverse-DNS app id: Linux `.desktop` file, hicolor icon name and Wayland app id.
pub const APP_ID: &str = "ai.storyteller.tooncraft";

fn main() -> eframe::Result {
    let cli: Cli = match args::parse(std::env::args().skip(1)) {
        Ok(Launch::Run(cli)) => *cli,
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
