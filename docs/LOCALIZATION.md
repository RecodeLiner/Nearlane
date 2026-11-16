# Desktop localization

Nearlane supports English (`en`) and Russian (`ru`). Settings → Language offers
System default, Русский and English. Unsupported system languages use English.
The selection is stored in QSettings `interface/language`, separate from account
credentials. No restart or network request is needed to change language.

## Editing translations

- QML: wrap static English text in `qsTranslate("Ui", "Source text")`.
- Rust: return `nearlane_core::Text::new("Source text")` or
  `Text::with_args("File saved: %1 bytes.", [Text::literal(bytes)])`.
  `apps/desktop/src/i18n.rs` renders these in the `Backend` Qt context.
- Put untrusted names, filenames, paths and numeric values in `Text::literal`.
  Use nested Text messages for translatable arguments such as transfer stages.
  Numbered placeholders may be reordered; inserted values are never rescanned.
- Run `python3 tools/update_translations.py` to update both TS files, then edit
  `apps/desktop/i18n/nearlane_ru.ts` in Qt Linguist or a text editor. The English
  catalog explicitly retains the source text. Fill every unfinished entry.
- Run `python3 tools/update_translations.py --check`. It checks source coverage,
  unfinished translations and the complete multiset of numbered placeholders.
  Run this check locally before publishing translation changes.

The source collector recognizes static Text::new/with_args and desktop tr calls,
Text::new(match/if …) branches, and legacy service message literals. Keep
translatable templates static and English; avoid dynamically constructing keys.
The core Text type deliberately has no toolkit, process-global locale or I/O.
CLI messages keep their English representation.

Qt Tools (`lrelease` on PATH, or `LRELEASE` pointing at the executable) is needed
at build time. Both compiled QM catalogs are embedded in the desktop resources,
not downloaded at runtime. Nix supplies Qt Tools and the runtime Qt module
translations. Other installations can supply Qt module catalogs through their
normal Qt translation directory or `NEARLANE_QT_TRANSLATIONS`.

## Verification

`tools/preview_desktop.py BINARY --language ru|en --output DIRECTORY` renders
synthetic, isolated UI states. The language-switch case verifies live QML and
backend translation, switching back, and rejection of an invalid preference.
`tools/smoke_desktop.py BINARY --kde` also checks persistence across processes.
These tools use temporary settings and a synthetic OAuth configuration, with no
real account grant restoration or radio activity. Native notification routing is tested
against a separate session-bus mock in both languages, without visible notices.

OS-owned dialogs and permission prompts can use the OS language. macOS app
packaging includes localized en/ru Bluetooth usage descriptions. macOS and
Windows runtime behavior has not been tested locally.
