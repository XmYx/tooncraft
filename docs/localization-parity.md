# Localization parity

> **Last updated:** 2026-11-17 · **Change:** major

| | Value | Kind |
|---|---|---|
| Source texts (`locales/tooncraft.tsv`) | 525 | measured (`cargo xtask strings`) |
| Languages with a ToonCraft catalog | 0 (English only) | measured |

Every user-visible string goes through `tl!` / `tlf!` (craft-i18n; the English source text is the
key). `build.rs` embeds `locales/<locale>/common.tsv` and `locales/<locale>/tooncraft.tsv` from a
craft-i18n checkout when `CRAFT_I18N_DIR` is set (`CRAFT_I18N_REQUIRED=1` for releases); without it
the app builds in English. `locales/tooncraft.tsv` is the generated source catalog for translators;
`cargo xtask ci` fails when it is stale.

Translated catalogs live in storytold/craft-i18n (`locales/<locale>/tooncraft.tsv`, format in its
`docs/catalog-format.md`), not here. `cargo xtask strings` checks any ToonCraft catalog it can reach
(one under `locales/<locale>/` here, or in the checkout `CRAFT_I18N_DIR` points to) against the
format: three columns, valid escapes, no empty translation, the same `{placeholders}` and trailing `…`
on both sides, no duplicate rows, the language name row, the clean-room header. It prints how many
source texts each covers; rows for texts the app no longer has are counted, not an error, so a
rewording elsewhere never fails the build. Tests run in English whatever the machine's language.
Partial German and Spanish catalogs were drafted from the plain meaning of the English (338 of the
525 texts each) and wait for a maintainer's decision to propose them to craft-i18n.

Shared strings ToonCraft uses that belong in `common.tsv`: File, Edit, View, Help, Open…, Save,
Save As…, Quit, Undo, Redo, Cancel, Delete, Rename, Add, Up, Down, Done, Remove, Opacity, Size,
Red, Green, Blue, Hue, Saturation, Brightness, Hex.

Not yet localisable: tool shortcut letters shown as key names, number formats (plain), right-to-left
layout.
