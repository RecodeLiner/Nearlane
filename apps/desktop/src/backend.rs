//! Qt presentation adapter. Behavior and background IO live in nearlane-application.
#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
        include!("cxx-qt-lib/qguiapplication.h");
        type QGuiApplication = cxx_qt_lib::QGuiApplication;
        include!("browser.h");
        #[namespace = "nearlane::desktop"]
        fn create_application() -> UniquePtr<QGuiApplication>;
        #[namespace = "nearlane::desktop"]
        fn application_icon_available() -> bool;
        #[namespace = "nearlane::desktop"]
        fn quit_application();
        include!("i18n.h");
        #[namespace = "nearlane::desktop"]
        fn install_language(preference: &QString);
    }
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, status)]
        #[qproperty(QString, language)]
        #[qproperty(QString, effective_language)]
        #[qproperty(bool, smoke_test)]
        #[qproperty(bool, tray_test)]
        #[qproperty(QString, design_preview)]
        #[qproperty(QString, account_status)]
        #[qproperty(bool, account_configured)]
        #[qproperty(QString, config_path)]
        #[qproperty(QString, config_status)]
        #[qproperty(bool, config_error)]
        #[qproperty(bool, account_busy)]
        #[qproperty(bool, account_authorized)]
        #[qproperty(QString, private_status)]
        #[qproperty(bool, private_busy)]
        #[qproperty(QString, discovery_status)]
        #[qproperty(bool, discovery_busy)]
        #[qproperty(bool, discovery_available)]
        #[qproperty(QStringList, own_devices)]
        #[qproperty(QStringList, bluetooth_devices)]
        #[qproperty(QString, bluetooth_status)]
        #[qproperty(QString, saved_login_status)]
        #[qproperty(bool, login_remembered)]
        #[qproperty(bool, login_cancellable)]
        #[qproperty(QStringList, certificate_rows)]
        #[qproperty(QString, selected_send_name)]
        #[qproperty(QString, transfer_status)]
        #[qproperty(bool, transfer_busy)]
        #[qproperty(QString, receive_directory)]
        #[qproperty(QString, receive_status)]
        #[qproperty(QString, incoming_offer)]
        #[qproperty(bool, receiving_active)]
        #[qproperty(bool, background_receiving)]
        #[qproperty(bool, confirm_each_file)]
        #[qproperty(QString, preferences_status)]
        #[qproperty(bool, receive_pending)]
        #[qproperty(bool, actionable_notification)]
        #[qproperty(bool, continuous_discovery)]
        #[qproperty(u32, received_count)]
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
        fn find_own_devices(self: Pin<&mut Backend>);
        #[qinvokable]
        fn cancel_discovery(self: Pin<&mut Backend>);
        #[qinvokable]
        fn restore_saved_sign_in(self: Pin<&mut Backend>);
        #[qinvokable]
        fn poll_account(self: Pin<&mut Backend>);
        #[qinvokable]
        fn load_config(self: Pin<&mut Backend>, url: &QUrl);
        #[qinvokable]
        fn select_send_file(self: Pin<&mut Backend>, url: &QUrl);
        #[qinvokable]
        fn send_selected_file(self: Pin<&mut Backend>, index: i32);
        #[qinvokable]
        fn cancel_transfer(self: Pin<&mut Backend>);
        #[qinvokable]
        fn select_receive_directory(self: Pin<&mut Backend>, url: &QUrl);
        #[qinvokable]
        fn start_receiving(self: Pin<&mut Backend>);
        #[qinvokable]
        fn stop_receiving(self: Pin<&mut Backend>);
        #[qinvokable]
        fn decide_incoming(self: Pin<&mut Backend>, accept: bool);
        #[qinvokable]
        fn set_background_discovery(self: Pin<&mut Backend>, enabled: bool);
        #[qinvokable]
        fn configure_receiving(self: Pin<&mut Backend>, enabled: bool, confirm: bool);
        #[qinvokable]
        fn set_send_surface(self: Pin<&mut Backend>, visible: bool);
        #[qinvokable]
        fn shutdown(self: Pin<&mut Backend>);
        #[qinvokable]
        fn select_language(self: Pin<&mut Backend>, language: &QString);
    }
}

use cxx_qt_lib::{QString, QStringList, QUrl};
use nearlane_application::{Application, Command, Language, Options, Snapshot};
use nearlane_platform::preferences::PreferenceStore;
use std::{pin::Pin, time::Duration};

/// Qt values only, plus an owner and a safe snapshot of the shared application.
pub struct BackendRust {
    status: QString,
    language: QString,
    effective_language: QString,
    smoke_test: bool,
    tray_test: bool,
    design_preview: QString,
    account_status: QString,
    account_configured: bool,
    config_path: QString,
    config_status: QString,
    config_error: bool,
    account_busy: bool,
    account_authorized: bool,
    private_status: QString,
    private_busy: bool,
    discovery_status: QString,
    discovery_busy: bool,
    discovery_available: bool,
    own_devices: QStringList,
    bluetooth_devices: QStringList,
    bluetooth_status: QString,
    saved_login_status: QString,
    login_remembered: bool,
    login_cancellable: bool,
    certificate_rows: QStringList,
    selected_send_name: QString,
    transfer_status: QString,
    transfer_busy: bool,
    receive_directory: QString,
    receive_status: QString,
    incoming_offer: QString,
    receiving_active: bool,
    background_receiving: bool,
    confirm_each_file: bool,
    preferences_status: QString,
    receive_pending: bool,
    actionable_notification: bool,
    continuous_discovery: bool,
    received_count: u32,
    app: Application,
    last: Snapshot,
}

impl Default for BackendRust {
    fn default() -> Self {
        let smoke_test = std::env::var_os("NEARLANE_SMOKE_TEST").is_some();
        let options = if smoke_test {
            Options::isolated(
                nearlane_platform::configuration::oauth_config_path(),
                PreferenceStore::discover(),
            )
        } else {
            Options::desktop()
        };
        let app = Application::start(options)
            .unwrap_or_else(|_| panic!("Could not start application controller"));
        let snapshot = app.snapshot();
        ffi::install_language(&QString::from(snapshot.effective_language.as_str()));
        Self {
            status: QString::from(snapshot.status.as_str()),
            language: QString::from(snapshot.language.as_str()),
            effective_language: QString::from(snapshot.effective_language.as_str()),
            smoke_test,
            tray_test: smoke_test && std::env::var_os("NEARLANE_TRAY_SMOKE_TEST").is_some(),
            design_preview: QString::from(
                if smoke_test {
                    std::env::var("NEARLANE_DESIGN_PREVIEW").unwrap_or_default()
                } else {
                    String::new()
                }
                .as_str(),
            ),
            account_status: QString::from(snapshot.account_status.as_str()),
            account_configured: snapshot.account_configured,
            config_path: QString::from(snapshot.config_path.as_str()),
            config_status: QString::from(snapshot.config_status.as_str()),
            config_error: snapshot.config_error,
            account_busy: snapshot.account_busy,
            account_authorized: snapshot.account_authorized,
            private_status: QString::from(snapshot.private_status.as_str()),
            private_busy: snapshot.private_busy,
            discovery_status: QString::from(snapshot.discovery_status.as_str()),
            discovery_busy: snapshot.discovery_busy,
            discovery_available: snapshot.discovery_available,
            own_devices: snapshot
                .own_devices
                .iter()
                .map(|s| QString::from(s.as_str()))
                .collect(),
            bluetooth_devices: snapshot
                .bluetooth_devices
                .iter()
                .map(|s| QString::from(s.as_str()))
                .collect(),
            bluetooth_status: QString::from(snapshot.bluetooth_status.as_str()),
            saved_login_status: QString::from(snapshot.saved_login_status.as_str()),
            login_remembered: snapshot.login_remembered,
            login_cancellable: snapshot.login_cancellable,
            certificate_rows: snapshot
                .certificate_rows
                .iter()
                .map(|s| QString::from(s.as_str()))
                .collect(),
            selected_send_name: QString::from(snapshot.selected_send_name.as_str()),
            transfer_status: QString::from(snapshot.transfer_status.as_str()),
            transfer_busy: snapshot.transfer_busy,
            receive_directory: QString::from(snapshot.receive_directory.as_str()),
            receive_status: QString::from(snapshot.receive_status.as_str()),
            incoming_offer: QString::from(snapshot.incoming_offer.as_str()),
            receiving_active: snapshot.receiving_active,
            background_receiving: snapshot.background_receiving,
            confirm_each_file: snapshot.confirm_each_file,
            preferences_status: QString::from(snapshot.preferences_status.as_str()),
            receive_pending: snapshot.receive_pending,
            actionable_notification: snapshot.actionable_notification,
            continuous_discovery: snapshot.continuous_discovery,
            received_count: snapshot.received_count,
            last: snapshot,
            app,
        }
    }
}
impl ffi::Backend {
    fn update_account(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        let snapshot = self.rust().app.snapshot();
        if snapshot.effective_language != self.rust().effective_language.to_string() {
            ffi::install_language(&QString::from(snapshot.effective_language.as_str()));
        }
        self.as_mut()
            .set_status(QString::from(snapshot.status.as_str()));
        self.as_mut()
            .set_language(QString::from(snapshot.language.as_str()));
        self.as_mut()
            .set_effective_language(QString::from(snapshot.effective_language.as_str()));
        self.as_mut()
            .set_account_status(QString::from(snapshot.account_status.as_str()));
        self.as_mut()
            .set_account_configured(snapshot.account_configured);
        self.as_mut()
            .set_config_path(QString::from(snapshot.config_path.as_str()));
        self.as_mut()
            .set_config_status(QString::from(snapshot.config_status.as_str()));
        self.as_mut().set_config_error(snapshot.config_error);
        self.as_mut().set_account_busy(snapshot.account_busy);
        self.as_mut()
            .set_account_authorized(snapshot.account_authorized);
        self.as_mut()
            .set_private_status(QString::from(snapshot.private_status.as_str()));
        self.as_mut().set_private_busy(snapshot.private_busy);
        self.as_mut()
            .set_discovery_status(QString::from(snapshot.discovery_status.as_str()));
        self.as_mut().set_discovery_busy(snapshot.discovery_busy);
        self.as_mut()
            .set_discovery_available(snapshot.discovery_available);
        self.as_mut().set_own_devices(
            snapshot
                .own_devices
                .iter()
                .map(|s| QString::from(s.as_str()))
                .collect(),
        );
        self.as_mut().set_bluetooth_devices(
            snapshot
                .bluetooth_devices
                .iter()
                .map(|s| QString::from(s.as_str()))
                .collect(),
        );
        self.as_mut()
            .set_bluetooth_status(QString::from(snapshot.bluetooth_status.as_str()));
        self.as_mut()
            .set_saved_login_status(QString::from(snapshot.saved_login_status.as_str()));
        self.as_mut()
            .set_login_remembered(snapshot.login_remembered);
        self.as_mut()
            .set_login_cancellable(snapshot.login_cancellable);
        self.as_mut().set_certificate_rows(
            snapshot
                .certificate_rows
                .iter()
                .map(|s| QString::from(s.as_str()))
                .collect(),
        );
        self.as_mut()
            .set_selected_send_name(QString::from(snapshot.selected_send_name.as_str()));
        self.as_mut()
            .set_transfer_status(QString::from(snapshot.transfer_status.as_str()));
        self.as_mut().set_transfer_busy(snapshot.transfer_busy);
        self.as_mut()
            .set_receive_directory(QString::from(snapshot.receive_directory.as_str()));
        self.as_mut()
            .set_receive_status(QString::from(snapshot.receive_status.as_str()));
        self.as_mut()
            .set_incoming_offer(QString::from(snapshot.incoming_offer.as_str()));
        self.as_mut()
            .set_receiving_active(snapshot.receiving_active);
        self.as_mut()
            .set_background_receiving(snapshot.background_receiving);
        self.as_mut()
            .set_confirm_each_file(snapshot.confirm_each_file);
        self.as_mut()
            .set_preferences_status(QString::from(snapshot.preferences_status.as_str()));
        self.as_mut().set_receive_pending(snapshot.receive_pending);
        self.as_mut()
            .set_actionable_notification(snapshot.actionable_notification);
        self.as_mut()
            .set_continuous_discovery(snapshot.continuous_discovery);
        self.as_mut().set_received_count(snapshot.received_count);
        self.rust_mut().last = snapshot;
    }
    fn dispatch(mut self: Pin<&mut Self>, command: Command) {
        use cxx_qt::CxxQtType as _;
        let _ = self.rust().app.sender().send(command);
        if self.rust().smoke_test {
            self.rust()
                .app
                .flush(Duration::from_secs(5))
                .unwrap_or_else(|_| panic!("Isolated controller did not respond"));
        }
        self.as_mut().update_account();
    }
    pub fn sign_in(self: Pin<&mut Self>) {
        self.dispatch(Command::SignIn);
    }
    pub fn cancel_sign_in(self: Pin<&mut Self>) {
        self.dispatch(Command::CancelSignIn);
    }
    pub fn clear_account(self: Pin<&mut Self>) {
        self.dispatch(Command::SignOut);
    }
    pub fn sign_out(self: Pin<&mut Self>) {
        self.dispatch(Command::SignOut);
    }
    pub fn prepare_own_devices(self: Pin<&mut Self>) {
        self.dispatch(Command::PrepareOwnDevices);
    }
    pub fn find_own_devices(self: Pin<&mut Self>) {
        self.dispatch(Command::FindOwnDevices);
    }
    pub fn cancel_discovery(self: Pin<&mut Self>) {
        self.dispatch(Command::CancelDiscovery);
    }
    pub fn restore_saved_sign_in(self: Pin<&mut Self>) {
        self.dispatch(Command::RestoreSignIn);
    }
    pub fn cancel_transfer(self: Pin<&mut Self>) {
        self.dispatch(Command::CancelTransfer);
    }
    pub fn start_receiving(self: Pin<&mut Self>) {
        self.dispatch(Command::StartReceiving);
    }
    pub fn stop_receiving(self: Pin<&mut Self>) {
        self.dispatch(Command::StopReceiving);
    }
    pub fn poll_account(self: Pin<&mut Self>) {
        self.update_account();
    }
    pub fn load_config(self: Pin<&mut Self>, url: &QUrl) {
        if let Some(path) = url.to_local_file() {
            self.dispatch(Command::ImportConfiguration(path.to_string().into()));
        }
    }
    pub fn select_send_file(self: Pin<&mut Self>, url: &QUrl) {
        if let Some(path) = url.to_local_file() {
            self.dispatch(Command::SelectFile(path.to_string().into()));
        }
    }
    pub fn select_receive_directory(self: Pin<&mut Self>, url: &QUrl) {
        if let Some(path) = url.to_local_file() {
            self.dispatch(Command::SelectDestination(path.to_string().into()));
        }
    }
    pub fn send_selected_file(self: Pin<&mut Self>, index: i32) {
        use cxx_qt::CxxQtType as _;
        if let Ok(index) = usize::try_from(index) {
            let devices_revision = self.rust().last.devices_revision;
            self.dispatch(Command::SendFile {
                index,
                devices_revision,
            });
        }
    }
    pub fn decide_incoming(self: Pin<&mut Self>, accept: bool) {
        use cxx_qt::CxxQtType as _;
        if let Some(request_id) = self.rust().last.incoming_request_id {
            self.dispatch(Command::DecideIncoming { request_id, accept });
        }
    }
    pub fn set_background_discovery(self: Pin<&mut Self>, enabled: bool) {
        self.dispatch(Command::SetDiscovery(enabled));
    }
    pub fn configure_receiving(self: Pin<&mut Self>, enabled: bool, confirm: bool) {
        self.dispatch(Command::ConfigureReceiving { enabled, confirm });
    }
    pub fn set_send_surface(self: Pin<&mut Self>, visible: bool) {
        self.dispatch(Command::SetSendSurface(visible));
    }
    pub fn select_language(self: Pin<&mut Self>, language: &QString) {
        if let Some(language) = Language::parse(&language.to_string()) {
            self.dispatch(Command::SetLanguage(language));
        }
    }
    pub fn shutdown(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        self.as_mut().rust_mut().app.shutdown();
    }
    pub fn check_contacts(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        self.as_mut().dispatch(Command::CheckContacts);
        if self.rust().smoke_test {
            assert!(self.rust().app.snapshot().hidden);
            eprintln!("smoke: policy checked; device remains hidden");
            self.smoke_account_check();
        }
    }
    fn smoke_account_check(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        if let Ok(phase) = std::env::var("NEARLANE_CONFIG_SMOKE_PHASE") {
            if phase == "write" {
                assert!(!self.rust().last.account_configured);
                let source = std::env::var("NEARLANE_CONFIG_SMOKE_SOURCE").unwrap_or_default();
                assert!(!source.is_empty());
                self.as_mut()
                    .dispatch(Command::ImportConfiguration(source.into()));
                assert!(!self.rust().last.config_error);
            } else {
                assert_eq!(phase, "read");
            }
            assert!(self.rust().last.account_configured);
            eprintln!("smoke: persistent configuration loaded; synthetic settings only");
        }
        if let Ok(phase) = std::env::var("NEARLANE_PREFERENCES_SMOKE_TEST") {
            if phase == "write" {
                let initial = PreferenceStore::discover()
                    .and_then(|s| s.load().ok())
                    .unwrap_or_default();
                assert!(initial.background_receiving);
                assert!(!initial.confirm_each_file);
                self.as_mut().dispatch(Command::SetLanguage(Language::Ru));
                self.as_mut().dispatch(Command::ConfigureReceiving {
                    enabled: false,
                    confirm: true,
                });
                self.as_mut().dispatch(Command::SelectDestination(
                    "synthetic-preference-folder".into(),
                ));
            }
            let saved = PreferenceStore::discover()
                .and_then(|s| s.load().ok())
                .unwrap_or_default();
            assert_eq!(saved.language, Language::Ru);
            assert!(!saved.background_receiving);
            assert!(saved.confirm_each_file);
            assert_eq!(
                saved.receive_directory,
                Some("synthetic-preference-folder".into())
            );
            assert_eq!(self.rust().last.effective_language, "ru");
            eprintln!("smoke: language preference persisted; synthetic settings only");
            eprintln!("smoke: receiving preferences persisted; synthetic settings only");
        }
        assert!(self.rust().last.account_configured);
        self.as_mut().dispatch(Command::SignIn);
        assert!(self.rust().last.account_busy);
        self.as_mut().dispatch(Command::CancelSignIn);
        assert!(!self.rust().last.account_busy && !self.rust().last.account_authorized);
        assert!(self.rust().last.hidden);
        eprintln!("smoke: local sign-in cancelled; device remains hidden");
        self.as_mut().dispatch(Command::SignOut);
        assert!(self.rust().last.hidden && !self.rust().last.account_authorized);
        eprintln!("smoke: signed out; device remains hidden");
    }
}
