// Original Nearlane Qt adapter: MIT OR Apache-2.0.
#pragma once

#include <QDesktopServices>
#include <QUrl>
#include <QApplication>
#include <QStandardPaths>
#include <QDir>
#include <QSettings>
#include <QQuickStyle>
#include <QIcon>
#include <QWindow>
#include <memory>

namespace nearlane::desktop {
inline bool open_browser(const QUrl &url) {
    return QDesktopServices::openUrl(url);
}
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
inline QString default_receive_directory() {
    const auto downloads = QStandardPaths::writableLocation(QStandardPaths::DownloadLocation);
    return downloads.isEmpty() ? QString{} : QDir(downloads).filePath("Nearlane");
}
inline std::unique_ptr<QSettings> local_preferences() {
    const auto testDirectory = qEnvironmentVariable("NEARLANE_PREFERENCES_SMOKE_DIR");
    if (qEnvironmentVariableIsSet("NEARLANE_SMOKE_TEST") && !testDirectory.isEmpty())
        return std::make_unique<QSettings>(QDir(testDirectory).filePath("receiving.ini"), QSettings::IniFormat);
    return std::make_unique<QSettings>();
}
inline bool receive_by_default() {
    return local_preferences()->value("receiving/ownDevices", true).toBool();
}
inline bool confirm_received_files() {
    return local_preferences()->value("receiving/confirmEachFile", false).toBool();
}
inline QString saved_receive_directory() {
    return local_preferences()->value("receiving/directory", default_receive_directory()).toString();
}
inline bool save_receive_preferences(bool enabled, bool confirm, const QString &directory) {
    auto settings = local_preferences();
    settings->setValue("receiving/ownDevices", enabled);
    settings->setValue("receiving/confirmEachFile", confirm);
    settings->setValue("receiving/directory", directory);
    settings->sync();
    return settings->status() == QSettings::NoError;
}
inline void quit_application() { QCoreApplication::quit(); }
}
