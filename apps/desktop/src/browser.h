// Original Nearlane Qt adapter: MIT OR Apache-2.0.
#pragma once

#include <QDesktopServices>
#include <QUrl>

namespace nearlane::desktop {
inline bool open_browser(const QUrl &url) {
    return QDesktopServices::openUrl(url);
}
}
