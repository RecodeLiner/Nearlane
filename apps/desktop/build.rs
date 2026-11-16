//! Compile only the Qt UI boundary and its declarative resources.

fn main() {
    let translations = build_translations();
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
    .qt_module("Bluetooth")
    .qt_module("Widgets")
    .qrc("qml/resources.qrc")
    .qrc(translations)
    .include_dir("src")
    .file("src/backend.rs")
    .build();
}

fn build_translations() -> std::path::PathBuf {
    use std::{env, fs, path::PathBuf, process::Command};
    println!("cargo:rerun-if-env-changed=LRELEASE");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let executable = env::var_os("LRELEASE").unwrap_or_else(|| "lrelease".into());
    let mut resources = String::from("<RCC><qresource prefix=\"/nearlane/i18n\">");
    for language in ["en", "ru"] {
        let source = format!("i18n/nearlane_{language}.ts");
        println!("cargo:rerun-if-changed={source}");
        let destination = output.join(format!("nearlane_{language}.qm"));
        let status = Command::new(&executable)
            .args(["-silent", "-nounfinished", &source, "-qm"])
            .arg(&destination)
            .status()
            .expect("Qt Linguist lrelease is required; install Qt Tools or set LRELEASE");
        assert!(status.success(), "Could not compile {source}");
        let escaped = destination
            .to_string_lossy()
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('"', "&quot;");
        resources.push_str(&format!(
            "<file alias=\"nearlane_{language}.qm\">{escaped}</file>"
        ));
    }
    resources.push_str("</qresource></RCC>");
    let resource = output.join("translations.qrc");
    fs::write(&resource, resources).expect("write translation resources");
    resource
}
