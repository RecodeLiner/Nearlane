import QtQuick

QtObject {
    // Foreground roles use *Text names: onSurface/onPrimary collide with QML's
    // on<Property> syntax when matching surface/primary properties exist.
    property bool dark: false
    property real basePointSize: 12
    property real controlMaxWidth: 400
    readonly property color surface: dark ? "#111318" : "#f9f9ff"
    readonly property color surfaceLow: dark ? "#191c22" : "#f2f3fa"
    readonly property color surfaceContainer: dark ? "#1d2027" : "#eceef5"
    readonly property color surfaceHigh: dark ? "#282b32" : "#e6e8ef"
    readonly property color surfaceText: dark ? "#e2e2e9" : "#191c22"
    readonly property color surfaceVariantText: dark ? "#c3c6d1" : "#434752"
    readonly property color primary: dark ? "#adc6ff" : "#365c99"
    readonly property color primaryText: dark ? "#002e69" : "#ffffff"
    readonly property color primaryContainer: dark ? "#21457e" : "#d8e2ff"
    readonly property color primaryContainerText: dark ? "#d8e2ff" : "#001a41"
    readonly property color outline: dark ? "#8d919d" : "#737783"
    readonly property color outlineVariant: dark ? "#434752" : "#c3c6d1"
    readonly property color error: dark ? "#ffb4ab" : "#ba1a1a"
    readonly property int smallRadius: 12
    readonly property int largeRadius: 24
    readonly property int gap: 16
    readonly property real body: basePointSize
    readonly property real label: basePointSize * 0.92
    readonly property real title: basePointSize * 1.33
    readonly property real headline: basePointSize * 2
}
