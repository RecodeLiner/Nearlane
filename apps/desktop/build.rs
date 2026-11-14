//! Compile only the Qt UI boundary and its declarative resources.

fn main() {
    cxx_qt_build::CxxQtBuilder::new_qml_module(
        cxx_qt_build::QmlModule::new("org.nearlane.app").qml_file("qml/Main.qml"),
    )
    .qt_module("QuickControls2")
    .qt_module("Network")
    .file("src/backend.rs")
    .build();
}
