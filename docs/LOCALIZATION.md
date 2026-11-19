# Desktop localization

Nearlane supports English (`en`) and Russian (`ru`). Settings → Language offers
System default, Русский and English. Unsupported system languages use English.
The selection is stored in shared `preferences.json`, separate from account
credentials. Changes apply to the running application.

## Editing translations

- Slint: wrap static UI text in `I18n.tr("Source text")`; the callback uses the
  shared `Ui` catalog and explicitly depends on the active language.
- Rust: return `nearlane_core::Text::new("Source text")` or
  `Text::with_args("File saved: %1 bytes.", [Text::literal(bytes)])`.
  `crates/application/src/i18n.rs` renders these from the shared `Backend` catalog.
- Put untrusted names, filenames, paths and numeric values in `Text::literal`.
  Use nested Text messages for translatable arguments such as transfer stages.
  Numbered placeholders may be reordered; inserted values are never rescanned.
- Run `python3 tools/update_translations.py` to update both TS files, then edit
  `crates/application/i18n/nearlane_ru.ts` in a text editor. The English
  catalog explicitly retains the source text. Fill every unfinished entry.
- Run `python3 tools/update_translations.py --check`. It checks source coverage,
  unfinished translations and the complete multiset of numbered placeholders.
  Run this check locally before publishing translation changes.

The source collector recognizes static Text::new/with_args and Rust tr calls,
Text::new(match/if …) branches, and legacy service message literals. Keep
translatable templates static and English; avoid dynamically constructing keys.
The core Text type deliberately has no toolkit, process-global locale or I/O.
CLI messages keep their English representation.

The controller embeds the Russian TS catalog at build time and parses it on the
first Russian lookup. English uses the original strings.

## Verification

`nearlane-slint --smoke-test` exercises language switching and UI callbacks
with temporary settings and a synthetic OAuth configuration. It uses software
rendering and keeps account restoration and radios off.
`nearlane-slint --preview /absolute/path.png --language ru --page settings`
renders a synthetic PNG without a native window. Add `--compact`, `--dark` or
`--scene incoming --page receive` for other views.

OS-owned dialogs and permission prompts can use the OS language. macOS app
packaging includes localized en/ru Bluetooth usage descriptions. macOS and
Windows runtime behavior has not been tested locally.
