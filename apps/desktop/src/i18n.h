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
inline void retranslate_windows();
inline void install_language(const QString &preference) {
    const auto language = preference == "ru" ? QStringLiteral("ru") : QStringLiteral("en");
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
    retranslate_windows();
}
// Only presentation: preferences and locale resolution live in Rust.
inline void retranslate_windows() {
    QSet<QQmlEngine *> engines;
    for (auto *window : QGuiApplication::allWindows())
        if (auto *engine = qmlEngine(window)) engines.insert(engine);
    for (auto *engine : engines) engine->retranslate();
}
}
