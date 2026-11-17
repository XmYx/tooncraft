# Localization parity

> **Last updated:** 2026-11-17 · **Change:** major

| | Value | Kind |
|---|---|---|
| Source texts (`locales/tooncraft.tsv`) | 257 | measured (`cargo xtask strings`) |
| Languages with a ToonCraft catalog | 0 (English only) | measured |

Every user-visible string goes through `tl!` / `tlf!` (craft-i18n; the English source text is the
key). `build.rs` embeds `locales/<locale>/common.tsv` and `locales/<locale>/tooncraft.tsv` from a
craft-i18n checkout when `CRAFT_I18N_DIR` is set (`CRAFT_I18N_REQUIRED=1` for releases); without it
the app builds in English. `locales/tooncraft.tsv` is the generated source catalog for translators;
`cargo xtask ci` fails when it is stale.

Shared strings ToonCraft uses that belong in `common.tsv`: File, Edit, View, Help, Open…, Save,
Save As…, Quit, Undo, Redo, Cancel, Delete, Rename, Add, Up, Down, Done, Remove, Opacity, Size,
Red, Green, Blue, Hue, Saturation, Brightness, Hex.

Not yet localisable: tool shortcut letters shown as key names, number formats (plain), right-to-left
layout.
