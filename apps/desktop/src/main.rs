//! Nearlane's Qt Quick desktop shell.

mod backend;
mod bluetooth;
mod i18n;
mod notifications;

use cxx_qt::casting::Upcast as _;
use cxx_qt_lib::{QQmlApplicationEngine, QQmlEngine, QUrl};

fn main() {
    let smoke = std::env::var_os("NEARLANE_SMOKE_TEST").is_some();
    if smoke {
        eprintln!("smoke: starting Qt");
    }
    let mut application = backend::ffi::create_application();
    backend::ffi::install_language(&backend::ffi::language_preference());
    if smoke {
        eprintln!("smoke: starting QML engine");
    }
    let mut engine = QQmlApplicationEngine::new();
    let Some(mut engine_ref) = engine.as_mut() else {
        std::process::exit(1)
    };
    engine_ref
        .as_mut()
        .on_object_creation_failed(|_, _| std::process::exit(1))
        .release();
    engine_ref.load(&QUrl::from("qrc:/qt/qml/org/nearlane/app/qml/Main.qml"));
    if smoke {
        eprintln!("smoke: QML loaded");
    }
    if let Some(engine_ref) = engine.as_mut() {
        let base: std::pin::Pin<&mut QQmlEngine> = engine_ref.upcast_pin();
        base.on_quit(|_| backend::ffi::quit_application()).release();
    }
    if let Some(application) = application.as_mut() {
        application.exec();
    }
}
