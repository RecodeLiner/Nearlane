//! Translate only explicit message templates; device names/paths remain literal.
use crate::backend::ffi;
use cxx_qt_lib::QString;
use nearlane_core::Text;

pub(super) fn text(message: &Text) -> QString {
    QString::from(message.render(&|source| ffi::translate_text(&QString::from(source)).to_string()))
}
pub(super) fn tr(source: &'static str) -> QString {
    text(&Text::new(source))
}
