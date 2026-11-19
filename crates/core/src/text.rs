//! Deferred presentation: translate templates before inserting untrusted values.
use std::fmt;

/// UI-independent text with explicit translation boundaries. Opaque values are
/// literals, never translation keys, and inserted arguments are never rescanned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Text {
    /// A source-language template, optionally containing numbered `%1` arguments.
    Message {
        /// Static English source for the desktop translation catalog.
        source: &'static str,
        /// Nested messages or literal values to substitute after translation.
        arguments: Vec<Text>,
    },
    /// An opaque name, diagnostic code or formatted number; never translated.
    Literal(String),
}
impl Text {
    /// A translated template without arguments.
    pub fn new(source: &'static str) -> Self {
        Self::with_args(source, [])
    }
    /// A translated template with numbered arguments, starting at `%1`.
    pub fn with_args(source: &'static str, arguments: impl IntoIterator<Item = Self>) -> Self {
        Self::Message {
            source,
            arguments: arguments.into_iter().collect(),
        }
    }
    /// A value inserted verbatim, even if it contains template-looking text.
    pub fn literal(value: impl ToString) -> Self {
        Self::Literal(value.to_string())
    }
    /// Render using the caller's locale and translator.
    pub fn render(&self, translate: &impl Fn(&str) -> String) -> String {
        let Self::Message { source, arguments } = self else {
            let Self::Literal(value) = self else {
                unreachable!()
            };
            return value.clone();
        };
        let template = translate(source);
        let values: Vec<_> = arguments.iter().map(|a| a.render(translate)).collect();
        let mut rendered = String::new();
        let mut rest = template.as_str();
        while let Some(index) = rest.find('%') {
            rendered.push_str(&rest[..index]);
            rest = &rest[index + 1..];
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            let value = rest[..digits]
                .parse::<usize>()
                .ok()
                .and_then(|n| n.checked_sub(1))
                .and_then(|n| values.get(n));
            if let Some(value) = value {
                rendered.push_str(value);
                rest = &rest[digits..];
            } else {
                rendered.push('%');
            }
        }
        rendered.push_str(rest);
        rendered
    }
}
impl From<&'static str> for Text {
    fn from(source: &'static str) -> Self {
        Self::new(source)
    }
}
impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render(&str::to_owned))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn translation_precedes_substitution_and_never_translates_or_rescans_names() {
        let text = Text::with_args(
            "%1: %2",
            [Text::literal("phone %2 <b>"), Text::new("active")],
        );
        assert_eq!(
            text.render(&|s| match s {
                "%1: %2" => "%2 - %1".into(),
                "active" => "активен".into(),
                _ => panic!("unexpected translation key"),
            }),
            "активен - phone %2 <b>"
        );
    }
    #[test]
    fn numbered_arguments_support_reordering_repetition_and_two_digits() {
        let args = (1..=10).map(Text::literal);
        assert_eq!(
            Text::with_args("%10 %1 %10 %% %0 %99", args).to_string(),
            "10 1 10 %% %0 %99"
        );
    }
}
