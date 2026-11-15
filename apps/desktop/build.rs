//! Compile only the Qt UI boundary and its declarative resources.

fn main() {
    cxx_qt_build::CxxQtBuilder::new_qml_module(
        cxx_qt_build::QmlModule::new("org.nearlane.app").qml_files([
            "qml/Main.qml",
            "qml/components/Theme.qml",
            "qml/components/BodyText.qml",
            "qml/components/ActionButton.qml",
            "qml/components/NavButton.qml",
            "qml/components/SettingRow.qml",
            "qml/components/Symbol.qml",
            "qml/pages/SendPage.qml",
            "qml/pages/ReceivePage.qml",
            "qml/pages/SettingsPage.qml",
        ]),
    )
    .qt_module("QuickControls2")
    .qt_module("Network")
    .qt_module("Widgets")
    .qrc("qml/resources.qrc")
    .include_dir("src")
    .file("src/backend.rs")
    .build();
}
