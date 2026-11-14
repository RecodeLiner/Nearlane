//! Minimal Qt boundary; protocol and authorization stay outside the UI crate.

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
        include!("browser.h");
        #[namespace = "nearlane::desktop"]
        fn open_browser(url: &QUrl) -> bool;
    }
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, status)]
        #[qproperty(bool, smoke_test)]
        #[qproperty(QString, account_status)]
        #[qproperty(bool, account_configured)]
        #[qproperty(bool, account_busy)]
        #[qproperty(bool, account_authorized)]
        #[qproperty(QString, private_status)]
        #[qproperty(bool, private_busy)]
        type Backend = super::BackendRust;

        #[qinvokable]
        fn check_contacts(self: Pin<&mut Backend>);
        #[qinvokable]
        fn sign_in(self: Pin<&mut Backend>);
        #[qinvokable]
        fn cancel_sign_in(self: Pin<&mut Backend>);
        #[qinvokable]
        fn clear_account(self: Pin<&mut Backend>);
        #[qinvokable]
        fn sign_out(self: Pin<&mut Backend>);
        #[qinvokable]
        fn prepare_own_devices(self: Pin<&mut Backend>);
        #[qinvokable]
        fn poll_account(self: Pin<&mut Backend>);
        #[qinvokable]
        fn load_config(self: Pin<&mut Backend>, url: &QUrl);
    }
}

use cxx_qt_lib::{QString, QUrl};
use nearlane_core::Visibility;
use nearlane_service::{AccountStatus, LoginError, Service};
use std::pin::Pin;

/// State exposed to QML contains no credentials.
pub struct BackendRust {
    status: QString,
    smoke_test: bool,
    account_status: QString,
    account_configured: bool,
    account_busy: bool,
    account_authorized: bool,
    private_status: QString,
    private_busy: bool,
    service: Service,
}

impl Default for BackendRust {
    fn default() -> Self {
        let service = Service::default();
        Self {
            status: QString::from(service.status().as_str()),
            smoke_test: std::env::var_os("NEARLANE_SMOKE_TEST").is_some(),
            account_status: QString::from(service.account_status().message().as_str()),
            account_configured: service.account_configured(),
            account_busy: false,
            account_authorized: false,
            private_status: QString::from(service.private_status().message().as_str()),
            private_busy: false,
            service,
        }
    }
}

impl ffi::Backend {
    fn update_account(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        let account = self.rust().service.account_status();
        let configured = self.rust().service.account_configured();
        let busy = self.rust().service.account_busy();
        let private_status = self.rust().service.private_status().message();
        let private_busy = self.rust().service.private_busy();
        self.as_mut()
            .set_account_status(QString::from(account.message().as_str()));
        self.as_mut().set_account_configured(configured);
        self.as_mut().set_account_busy(busy);
        self.as_mut()
            .set_account_authorized(account == AccountStatus::Authorized);
        self.as_mut()
            .set_private_status(QString::from(private_status.as_str()));
        self.set_private_busy(private_busy);
    }

    /// Start an explicitly requested login in the system browser; QML never sees the URL.
    pub fn sign_in(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        let result = self.as_mut().rust_mut().service.begin_sign_in();
        match result {
            Ok(url) => {
                if !ffi::open_browser(&QUrl::from(url.expose_url())) {
                    self.as_mut()
                        .rust_mut()
                        .service
                        .cancel_sign_in(LoginError::Browser);
                }
            }
            Err(error) if error != LoginError::Busy => {
                self.as_mut().rust_mut().service.cancel_sign_in(error);
            }
            Err(_) => {}
        }
        self.update_account();
    }

    /// Discard this attempt; late replies cannot restore a session.
    pub fn cancel_sign_in(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        self.as_mut()
            .rust_mut()
            .service
            .cancel_sign_in(LoginError::Cancelled);
        self.update_account();
    }

    /// Sign out and request withdrawal of this device's temporary credentials.
    pub fn clear_account(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        self.as_mut().rust_mut().service.clear_account();
        self.update_account();
    }

    /// Sign out through the service so sharing state is reset before credentials.
    pub fn sign_out(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        self.as_mut().rust_mut().service.sign_out();
        self.update_account();
    }

    /// Retry certificate preparation; authentication and key material stay in Rust.
    pub fn prepare_own_devices(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        let _ = self.as_mut().rust_mut().service.prepare_own_devices();
        self.update_account();
    }

    /// Update progress on the GUI thread without doing network IO there.
    pub fn poll_account(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        self.as_mut().rust_mut().service.poll_account();
        self.update_account();
    }

    /// Read only a file selected through the local file dialog.
    pub fn load_config(mut self: Pin<&mut Self>, url: &QUrl) {
        use cxx_qt::CxxQtType as _;
        if let Some(path) = url.to_local_file() {
            let path = path.to_string();
            self.as_mut()
                .rust_mut()
                .service
                .load_account_config(std::path::Path::new(&path));
            self.update_account();
        }
    }

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
        self.as_mut().set_status(QString::from(status));
        if smoke_test {
            eprintln!("smoke: policy checked; device remains hidden");
            self.smoke_account_check();
        }
    }

    fn smoke_account_check(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        // Only called by the opt-in smoke path. Bind a real local listener but
        // never launch a browser or send a request to Google from this test.
        assert!(self.rust().service.account_configured());
        assert!(self.as_mut().rust_mut().service.begin_sign_in().is_ok());
        self.as_mut().update_account();
        assert!(self.rust().account_busy);
        self.as_mut().cancel_sign_in();
        self.as_mut().poll_account();
        assert!(!self.rust().account_busy && !self.rust().account_authorized);
        assert_eq!(self.rust().service.visibility(), Visibility::Hidden);
        eprintln!("smoke: local sign-in cancelled; device remains hidden");
        self.as_mut().sign_out();
        assert_eq!(
            self.rust().service.account_status(),
            AccountStatus::SignedOut
        );
        assert_eq!(self.rust().service.visibility(), Visibility::Hidden);
        eprintln!("smoke: signed out; device remains hidden");
    }
}
