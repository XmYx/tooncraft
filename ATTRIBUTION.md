# Attribution

Every non-code asset in ToonCraft, with its author, source and licence. The rules are in
[AGENTS.md](AGENTS.md): no artwork from commercial animation tools, open licences only, every asset
listed here in the same change. `cargo xtask assets` checks that every file under `assets/`,
`docs/images/` and `docs/brand/` is listed. ToonCraft contains no assets from TVPaint, Toon Boom,
Celsys, Savage Interactive or any other vendor.

## In this repository

| Asset | Author | Source | Licence | Used for |
|---|---|---|---|---|
| `assets/app-icon/tooncraft.svg` | ToonCraft contributors | Original work: draft app icon (flip-book sheets, bouncing ball) | MIT OR Apache-2.0 (`assets/app-icon/LICENSE.txt`) | App icon, README |
| `docs/images/tooncraft-onion-skin.png` | ToonCraft contributors | Original work: ToonCraft screenshot (`--demo --frame 8 --screenshot`) of the built-in demo shot, code-drawn in `crates/engine/src/demo.rs` | MIT OR Apache-2.0 | README |
| `docs/images/tooncraft-timeline.png` | ToonCraft contributors | Original work: ToonCraft screenshot (`--demo --select 9..14 --light-table --thumbnails --panel camera --frame 14 --screenshot`) of the built-in demo shot | MIT OR Apache-2.0 | README |

Code-drawn content (the demo shot *Bouncing ball*, the tool icons in `apps/tooncraft/src/icons.rs`
and the paper grain in `crates/brush/src/grain.rs`) is original work under the project licence.

## Bundled through dependencies

egui's default fonts (Ubuntu-Light, Ubuntu Font Licence 1.0; the egui icon font, MIT / OFL-1.1),
through `eframe`'s `default_fonts` feature.

## Embedded at build time from craft-fonts (`CRAFT_FONTS_DIR`)

None by default. With `CRAFT_FONTS_DIR` set, `craft-fonts-build` embeds the faces listed in that
checkout's manifest; their licences are in craft-fonts.
