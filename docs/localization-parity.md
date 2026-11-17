# Localization parity

> **Last updated:** 2026-11-17 · **Change:** major

| | Value | Kind |
|---|---|---|
| Source texts (`locales/tooncraft.tsv`) | 525 | measured (`cargo xtask strings`) |
| Languages with a ToonCraft catalog | 2: German (`de`), Spanish (`es`) | measured |
| German: texts translated | 338 of 525 (64 %) | measured (`cargo xtask strings`) |
| Spanish: texts translated | 338 of 525 (64 %) | measured (`cargo xtask strings`) |

Every user-visible string goes through `tl!` / `tlf!` (craft-i18n; the English source text is the
key). `build.rs` embeds `locales/<locale>/common.tsv` and `locales/<locale>/tooncraft.tsv` from a
craft-i18n checkout when `CRAFT_I18N_DIR` is set (`CRAFT_I18N_REQUIRED=1` for releases); without it
the app builds in English. `locales/tooncraft.tsv` is the generated source catalog for translators;
`cargo xtask ci` fails when it is stale.

## Catalogs

The translations are `locales/<locale>/tooncraft.tsv` in the craft-i18n catalog format (rows of
`context<TAB>English<TAB>translation`, empty context, the English source text as the key; the format is
in craft-i18n's `docs/catalog-format.md`). They sit in this repository so the app can ship them; each
file can be copied as it is to `locales/<locale>/tooncraft.tsv` in craft-i18n. `build.rs` embeds them
for the locales that a craft-i18n checkout (`CRAFT_I18N_DIR`) has no ToonCraft catalog for, so a plain
`cargo build` already offers German and Spanish to a user whose system language is one of them.
`cargo xtask strings` checks every catalog against the format (three columns, valid escapes, no empty
translation, the same `{placeholders}` and trailing `…` on both sides, no duplicate rows, the language
name row, the clean-room header) and prints how many source texts each covers; rows for texts the app
no longer has are counted and reported, not an error, so a rewording elsewhere never fails the build.
Tests run in English whatever the machine's language; the app has a test that shows the window in
German.

The German and Spanish texts were written from the plain meaning of the English in ordinary computing
vocabulary; no other product's translations were consulted. Still to translate: the effects panel, the
lip-sync and sound-mix panels, the peg panel, the brush editor and library, and the messages added after
the first translation pass.

Shared strings ToonCraft uses that belong in `common.tsv`: File, Edit, View, Help, Open…, Save,
Save As…, Quit, Undo, Redo, Cancel, Delete, Rename, Add, Up, Down, Done, Remove, Opacity, Size,
Red, Green, Blue, Hue, Saturation, Brightness, Hex.

Not yet localisable: tool shortcut letters shown as key names, number formats (plain), right-to-left
layout.
