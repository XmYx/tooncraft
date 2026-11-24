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
| `assets/app-icon/hicolor/scalable/apps/ai.storyteller.tooncraft.svg` | ToonCraft contributors | Copy of `assets/app-icon/tooncraft.svg` for the hicolor theme | MIT OR Apache-2.0 (`assets/app-icon/LICENSE.txt`) | Linux icon theme |
| `assets/app-icon/hicolor/16x16/apps/ai.storyteller.tooncraft.png` | ToonCraft contributors | Rendered from `assets/app-icon/tooncraft.svg` by `cargo xtask icons` | MIT OR Apache-2.0 (`assets/app-icon/LICENSE.txt`) | Linux icon theme, packages |
| `assets/app-icon/hicolor/24x24/apps/ai.storyteller.tooncraft.png` | ToonCraft contributors | Rendered from `assets/app-icon/tooncraft.svg` by `cargo xtask icons` | MIT OR Apache-2.0 (`assets/app-icon/LICENSE.txt`) | Linux icon theme, packages |
| `assets/app-icon/hicolor/32x32/apps/ai.storyteller.tooncraft.png` | ToonCraft contributors | Rendered from `assets/app-icon/tooncraft.svg` by `cargo xtask icons` | MIT OR Apache-2.0 (`assets/app-icon/LICENSE.txt`) | Linux icon theme, packages |
| `assets/app-icon/hicolor/48x48/apps/ai.storyteller.tooncraft.png` | ToonCraft contributors | Rendered from `assets/app-icon/tooncraft.svg` by `cargo xtask icons` | MIT OR Apache-2.0 (`assets/app-icon/LICENSE.txt`) | Linux icon theme, packages |
| `assets/app-icon/hicolor/64x64/apps/ai.storyteller.tooncraft.png` | ToonCraft contributors | Rendered from `assets/app-icon/tooncraft.svg` by `cargo xtask icons` | MIT OR Apache-2.0 (`assets/app-icon/LICENSE.txt`) | Linux icon theme, packages |
| `assets/app-icon/hicolor/128x128/apps/ai.storyteller.tooncraft.png` | ToonCraft contributors | Rendered from `assets/app-icon/tooncraft.svg` by `cargo xtask icons` | MIT OR Apache-2.0 (`assets/app-icon/LICENSE.txt`) | Linux icon theme, packages |
| `assets/app-icon/hicolor/256x256/apps/ai.storyteller.tooncraft.png` | ToonCraft contributors | Rendered from `assets/app-icon/tooncraft.svg` by `cargo xtask icons` | MIT OR Apache-2.0 (`assets/app-icon/LICENSE.txt`) | Linux icon theme, packages |
| `assets/app-icon/hicolor/512x512/apps/ai.storyteller.tooncraft.png` | ToonCraft contributors | Rendered from `assets/app-icon/tooncraft.svg` by `cargo xtask icons` | MIT OR Apache-2.0 (`assets/app-icon/LICENSE.txt`) | Linux icon theme, packages |
| `assets/app-icon/tooncraft-1024.png` | ToonCraft contributors | Rendered from `assets/app-icon/tooncraft.svg` by `cargo xtask icons` | MIT OR Apache-2.0 (`assets/app-icon/LICENSE.txt`) | Store listings, macOS icon source |
| `assets/app-icon/tooncraft.ico` | ToonCraft contributors | PNG-in-ICO of the rendered sizes 16–256, by `cargo xtask icons` | MIT OR Apache-2.0 (`assets/app-icon/LICENSE.txt`) | Windows installer |
| `assets/app-icon/tooncraft.icns` | ToonCraft contributors | PNG-in-ICNS of the rendered sizes 16–1024, by `cargo xtask icons` | MIT OR Apache-2.0 (`assets/app-icon/LICENSE.txt`) | macOS app bundle |
| `docs/images/tooncraft-draw.png` | ToonCraft contributors | Original work: ToonCraft screenshot (`--demo --frame 7 --layout drawing --tool ink --screenshot`) of the built-in demo shot, code-drawn in `crates/engine/src/demo.rs` | MIT OR Apache-2.0 | README |
| `docs/images/tooncraft-time.png` | ToonCraft contributors | Original work: ToonCraft screenshot (`--demo --frame 7 --layout timing --loop 4..14 --screenshot`) of the built-in demo shot | MIT OR Apache-2.0 | README |
| `docs/images/tooncraft-sheet.png` | ToonCraft contributors | Original work: ToonCraft screenshot (`--demo --frame 7 --layout sheet --screenshot`) of the built-in demo shot | MIT OR Apache-2.0 | README |
| `docs/images/tooncraft-cell-menu.png` | ToonCraft contributors | Original work: ToonCraft screenshot (`--demo --frame 7 --context-menu cell --screenshot`) of the built-in demo shot | MIT OR Apache-2.0 | README |
| `docs/images/tooncraft-onion.png` | ToonCraft contributors | Original work: ToonCraft screenshot (`--demo --frame 7 --context-menu onion --screenshot`) of the built-in demo shot | MIT OR Apache-2.0 | README |
| `docs/images/tooncraft-tool-flyout.png` | ToonCraft contributors | Original work: ToonCraft screenshot (`--demo --frame 7 --context-menu tool --screenshot`) of the built-in demo shot | MIT OR Apache-2.0 | README |

Code-drawn content (the demo shot *Bouncing ball*, the tool icons in `apps/tooncraft/src/icons.rs`
the built-in brush settings in `crates/engine/src/library.rs` and the built-in grain textures Paper,
Canvas weave, Speckle and Clouds in `crates/brush/src/grain.rs`) is original work under the project licence.
The grain textures are computed from an integer hash at stroke time: there are no grain image files, and
no third-party texture, noise table or brush file was used. Grain images a user adds (`Grain from Image…`)
stay in the user's shot or `.toonbrush` file and are not part of ToonCraft; a grain texture offered with
the app must be original or CC0 and listed in the table above in the same change.

## Bundled through dependencies

egui's default fonts (Ubuntu-Light, Ubuntu Font Licence 1.0; the egui icon font, MIT / OFL-1.1),
through `eframe`'s `default_fonts` feature.

## Embedded at build time from craft-fonts (`CRAFT_FONTS_DIR`)

None by default. With `CRAFT_FONTS_DIR` set, `craft-fonts-build` embeds the faces listed in that
checkout's manifest; their licences are in craft-fonts.
