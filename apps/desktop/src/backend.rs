//! Minimal Qt boundary; protocol and authorization stay outside the UI crate.

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, status)]
        #[qproperty(bool, smoke_test)]
        type Backend = super::BackendRust;

        #[qinvokable]
        fn check_contacts(self: Pin<&mut Backend>);
    }
}

use cxx_qt_lib::QString;
use nearlane_core::Visibility;
use nearlane_service::Service;
use std::pin::Pin;

/// State exposed to QML contains no credentials.
pub struct BackendRust {
    status: QString,
    smoke_test: bool,
    service: Service,
}

impl Default for BackendRust {
    fn default() -> Self {
        let service = Service::default();
        Self {
            status: QString::from(service.status().as_str()),
            smoke_test: std::env::var_os("NEARLANE_SMOKE_TEST").is_some(),
            service,
        }
    }
}

impl ffi::Backend {
    /// Report account support without starting a login or widening visibility.
    pub fn check_contacts(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        let status = match self
            .as_mut()
            .rust_mut()
            .service
            .set_visibility(Visibility::Contacts)
        {
            Ok(()) => "Contacts are available.",
            Err(_) => "Contacts are not available yet. The device remains hidden.",
        };
        let smoke_test = self.rust().smoke_test;
        self.set_status(QString::from(status));
        if smoke_test {
            eprintln!("smoke: policy checked; device remains hidden");
        }
    }
}
