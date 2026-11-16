// Original Nearlane Qt adapter: MIT OR Apache-2.0.
#pragma once

#include <QUrl>
#include <QApplication>
#include <QQuickStyle>
#include <QIcon>
#include <QWindow>
#include <memory>

namespace nearlane::desktop {
inline std::unique_ptr<QGuiApplication> create_application() {
    static int argc = 1;
    static char name[] = "nearlane-desktop";
    static char *argv[] = {name, nullptr};
    auto app = std::make_unique<QApplication>(argc, argv);
    app->setApplicationName("Nearlane");
    app->setDesktopFileName("org.nearlane.Nearlane");
    app->setWindowIcon(QIcon(QStringLiteral(":/nearlane/icon.svg")));
    app->setOrganizationName("Nearlane");
    app->setOrganizationDomain("nearlane.org");
    app->setQuitOnLastWindowClosed(false);
    QQuickStyle::setStyle("Material");
    return app;
}
inline bool application_icon_available() {
    if (QGuiApplication::desktopFileName() != QStringLiteral("org.nearlane.Nearlane") ||
        QGuiApplication::windowIcon().pixmap(32, 32).isNull()) return false;
    for (auto *window : QGuiApplication::allWindows())
        if (window->isVisible() && !window->icon().pixmap(32, 32).isNull()) return true;
    return false;
}
inline void quit_application() { QCoreApplication::quit(); }
}
