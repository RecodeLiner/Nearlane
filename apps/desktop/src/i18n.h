// Original Nearlane Qt translation boundary: MIT OR Apache-2.0.
#pragma once
#include "browser.h"
#include <QLocale>
#include <QTranslator>
#include <QQmlEngine>
#include <QSet>
#include <QWindow>
#include <QLibraryInfo>
#include <array>

namespace nearlane::desktop {
inline QString language_preference() {
    const auto value = local_preferences()->value("interface/language", "system").toString();
    return value == "ru" || value == "en" ? value : QStringLiteral("system");
}
inline QString resolved_language(const QString &preference) {
    if (preference == "ru" || preference == "en") return preference;
    for (const auto &language : QLocale::system().uiLanguages()) {
        if (language.startsWith("ru", Qt::CaseInsensitive)) return "ru";
        if (language.startsWith("en", Qt::CaseInsensitive)) return "en";
    }
    return "en";
}
inline QTranslator &application_translator() {
    // Owned until application shutdown; no translator points into a temporary.
    static QTranslator translator;
    return translator;
}
inline QString &active_language() {
    static QString language;
    return language;
}
inline std::array<QTranslator, 2> &qt_translators() {
    static std::array<QTranslator, 2> translators;
    return translators;
}
inline void install_language(const QString &preference) {
    const auto language = resolved_language(preference);
    active_language() = language;
    QCoreApplication::removeTranslator(&application_translator());
    for (auto &translator : qt_translators()) QCoreApplication::removeTranslator(&translator);
    QLocale::setDefault(preference == "system" ? QLocale::system() : QLocale(language));
    const auto qtDirectory = qEnvironmentVariable("NEARLANE_QT_TRANSLATIONS", QLibraryInfo::path(QLibraryInfo::TranslationsPath));
    const std::array<QString, 2> modules{"qtbase", "qtdeclarative"};
    for (std::size_t i = 0; i < modules.size(); ++i)
        if (qt_translators()[i].load(QLocale(language), modules[i], "_", qtDirectory))
            QCoreApplication::installTranslator(&qt_translators()[i]);
    if (application_translator().load(":/nearlane/i18n/nearlane_" + language + ".qm"))
        QCoreApplication::installTranslator(&application_translator());
}
inline QString effective_language() { return active_language(); }
inline bool change_language(const QString &preference) {
    if (preference != "system" && preference != "ru" && preference != "en") return false;
    auto settings = local_preferences();
    settings->setValue("interface/language", preference);
    settings->sync();
    install_language(preference);
    // Retranslate every existing QML binding without replacing windows or state.
    QSet<QQmlEngine *> engines;
    for (auto *window : QGuiApplication::allWindows())
        if (auto *engine = qmlEngine(window)) engines.insert(engine);
    for (auto *engine : engines) engine->retranslate();
    return settings->status() == QSettings::NoError;
}
inline QString translate_text(const QString &source) {
    const auto utf8 = source.toUtf8();
    return QCoreApplication::translate("Backend", utf8.constData());
}
}
