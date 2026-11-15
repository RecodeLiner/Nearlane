// Original Nearlane Qt adapter: MIT OR Apache-2.0.
#pragma once

#include <QDesktopServices>
#include <QUrl>
#include <QApplication>
#include <QStandardPaths>
#include <QDir>
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
    app->setOrganizationDomain("nearlane.org");
    app->setQuitOnLastWindowClosed(false);
    return app;
}
inline QString default_receive_directory() {
    const auto downloads = QStandardPaths::writableLocation(QStandardPaths::DownloadLocation);
    return downloads.isEmpty() ? QString{} : QDir(downloads).filePath("Nearlane");
}
inline void quit_application() { QCoreApplication::quit(); }
}
