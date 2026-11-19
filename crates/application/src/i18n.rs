//! Shared catalogs and explicit translation boundaries for every frontend.
use nearlane_core::Text;
use nearlane_platform::preferences::Language;
use std::{collections::HashMap, sync::OnceLock};

type Catalog = HashMap<(String, String), String>;
static RUSSIAN: OnceLock<Catalog> = OnceLock::new();

/// Resolve a preference without changing the OS locale or any active workers.
pub fn effective_language(language: Language) -> Language {
    match language {
        Language::System => {
            language_for_locale(sys_locale::get_locale().as_deref().unwrap_or("en"))
        }
        _ => language,
    }
}
fn language_for_locale(value: &str) -> Language {
    if value
        .split(['-', '_', '.'])
        .next()
        .is_some_and(|s| s.eq_ignore_ascii_case("ru"))
    {
        Language::Ru
    } else {
        Language::En
    }
}
/// Translate a static UI source. Runtime names and paths must stay literal.
pub fn translate(language: Language, context: &str, source: &str) -> String {
    if effective_language(language) != Language::Ru {
        return source.to_owned();
    }
    RUSSIAN
        .get_or_init(|| catalog(include_str!("../i18n/nearlane_ru.ts")))
        .get(&(context.to_owned(), source.to_owned()))
        .cloned()
        .unwrap_or_else(|| source.to_owned())
}
/// Render nested status messages before inserting opaque values.
pub fn render(language: Language, text: &Text) -> String {
    text.render(&|source| translate(language, "Backend", source))
}
fn catalog(xml: &str) -> Catalog {
    let mut result = Catalog::new();
    let Ok(doc) = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    ) else {
        return result;
    };
    for context in doc
        .root_element()
        .children()
        .filter(|c| c.has_tag_name("context"))
    {
        let name = context
            .children()
            .find(|n| n.has_tag_name("name"))
            .and_then(|n| n.text())
            .unwrap_or_default();
        for message in context.children().filter(|m| m.has_tag_name("message")) {
            let source = message
                .children()
                .find(|n| n.has_tag_name("source"))
                .and_then(|n| n.text());
            let translated = message
                .children()
                .find(|n| n.has_tag_name("translation") && n.attribute("type").is_none())
                .and_then(|n| n.text());
            if let (Some(source), Some(translated)) = (source, translated) {
                result.insert((name.to_owned(), source.to_owned()), translated.to_owned());
            }
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_catalog_translates_status_but_preserves_opaque_values() {
        assert_eq!(translate(Language::Ru, "Ui", "Send"), "Отправить");
        assert_eq!(
            render(
                Language::Ru,
                &Text::with_args("%1 B", [Text::literal("phone %2 <b>")])
            ),
            "phone %2 <b> Б"
        );
        assert_eq!(language_for_locale("ru_RU.UTF-8"), Language::Ru);
        assert_eq!(language_for_locale("de-DE"), Language::En);
    }

    #[test]
    fn english_and_russian_catalogs_cover_the_same_sources() {
        let english = catalog(include_str!("../i18n/nearlane_en.ts"));
        let russian = catalog(include_str!("../i18n/nearlane_ru.ts"));
        assert_eq!(russian.len(), english.len());
        for (key, translation) in &english {
            assert_eq!(translation, &key.1);
            assert!(russian.contains_key(key), "Missing Russian entry {key:?}");
        }
    }
}
