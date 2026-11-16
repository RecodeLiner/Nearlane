//! Minimal Qt boundary; protocol and authorization stay outside the UI crate.

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
        fn open_browser(url: &QUrl) -> bool;
        #[namespace = "nearlane::desktop"]
        fn create_application() -> UniquePtr<QGuiApplication>;
        #[namespace = "nearlane::desktop"]
        fn default_receive_directory() -> QString;
        #[namespace = "nearlane::desktop"]
        fn receive_by_default() -> bool;
        #[namespace = "nearlane::desktop"]
        fn confirm_received_files() -> bool;
        #[namespace = "nearlane::desktop"]
        fn saved_receive_directory() -> QString;
        #[namespace = "nearlane::desktop"]
        fn save_receive_preferences(enabled: bool, confirm: bool, directory: &QString) -> bool;
        #[namespace = "nearlane::desktop"]
        fn quit_application();
        include!("i18n.h");
        #[namespace = "nearlane::desktop"]
        fn language_preference() -> QString;
        #[namespace = "nearlane::desktop"]
        fn effective_language() -> QString;
        #[namespace = "nearlane::desktop"]
        fn install_language(preference: &QString);
        #[namespace = "nearlane::desktop"]
        fn change_language(preference: &QString) -> bool;
        #[namespace = "nearlane::desktop"]
        fn translate_text(source: &QString) -> QString;
        include!("bluetooth.h");
        #[namespace = "nearlane::desktop"]
        type BluetoothDiscovery;
        #[namespace = "nearlane::desktop"]
        fn create_bluetooth_discovery() -> UniquePtr<BluetoothDiscovery>;
        fn set_scanning(self: Pin<&mut BluetoothDiscovery>, enabled: bool);
        fn scan_status(self: &BluetoothDiscovery) -> i32;
        fn drain_advertisements(self: Pin<&mut BluetoothDiscovery>) -> QStringList;
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

use crate::i18n::{text, tr};
use cxx_qt_lib::{DateFormat, QDateTime, QString, QStringList, QTimeZone, QUrl};
use nearlane_core::Text;
use nearlane_core::Visibility;
use nearlane_service::{AccountStatus, DiscoveryStatus, LoginError, PrivateStatus, Service};
use std::pin::Pin;

/// State exposed to QML contains no credentials.
pub struct BackendRust {
    language: QString,
    effective_language: QString,
    preferences_failed: bool,
    status: QString,
    smoke_test: bool,
    tray_test: bool,
    design_preview: QString,
    account_status: QString,
    account_configured: bool,
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
    send_surface: bool,
    bluetooth: crate::bluetooth::Discovery,
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
    service: Service,
    notifications: crate::notifications::OfferNotifications,
}

impl Default for BackendRust {
    fn default() -> Self {
        let mut service = Service::default();
        let smoke_test = std::env::var_os("NEARLANE_SMOKE_TEST").is_some();
        if !smoke_test && service.account_configured() {
            let _ = service.restore_saved_sign_in();
        }
        let folder = if smoke_test {
            ffi::default_receive_directory()
        } else {
            ffi::saved_receive_directory()
        }
        .to_string();
        if !folder.is_empty() {
            service.select_receive_directory(folder.into());
        }
        service.set_continuous_discovery(!smoke_test);
        let background_receiving = !smoke_test && ffi::receive_by_default();
        let confirm_each_file = smoke_test || ffi::confirm_received_files();
        service.set_receiving_preferences(background_receiving, confirm_each_file);
        Self {
            language: ffi::language_preference(),
            effective_language: ffi::effective_language(),
            preferences_failed: false,
            status: text(&service.status_text()),
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
            account_status: text(&service.account_status().text()),
            account_configured: service.account_configured(),
            account_busy: false,
            account_authorized: false,
            private_status: text(&service.private_status().text()),
            private_busy: false,
            discovery_status: text(&service.discovery_status().text()),
            discovery_busy: false,
            discovery_available: false,
            own_devices: QStringList::default(),
            bluetooth_devices: QStringList::default(),
            bluetooth_status: QString::default(),
            send_surface: false,
            bluetooth: crate::bluetooth::Discovery::default(),
            saved_login_status: text(&service.saved_login_status().text()),
            login_remembered: service.has_saved_login(),
            login_cancellable: false,
            certificate_rows: QStringList::default(),
            selected_send_name: QString::default(),
            transfer_status: text(service.transfer_text()),
            transfer_busy: false,
            receive_directory: QString::from(service.receive_directory().as_str()),
            receive_status: text(service.receive_text()),
            incoming_offer: QString::default(),
            receiving_active: false,
            background_receiving,
            confirm_each_file,
            preferences_status: QString::default(),
            receive_pending: false,
            actionable_notification: false,
            continuous_discovery: !smoke_test,
            received_count: 0,
            service,
            notifications: crate::notifications::OfferNotifications::default(),
        }
    }
}

impl ffi::Backend {
    fn update_account(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        let bluetooth_status = {
            let mut state = self.as_mut().rust_mut();
            let enabled = !state.smoke_test;
            let foreground = state.send_surface;
            let state = &mut *state;
            state.notifications.update(&mut state.service, enabled);
            state
                .bluetooth
                .update(&mut state.service, enabled, foreground)
        };
        let bluetooth_devices = self
            .rust()
            .service
            .bluetooth_device_names()
            .into_iter()
            .map(QString::from)
            .collect::<QStringList>();
        self.as_mut().set_bluetooth_status(text(&bluetooth_status));
        self.as_mut().set_bluetooth_devices(bluetooth_devices);
        let account = self.rust().service.account_status();
        let configured = self.rust().service.account_configured();
        let busy = self.rust().service.account_busy();
        let private_status = self.rust().service.private_status().text();
        let private_busy = self.rust().service.private_busy();
        let discovery = self.rust().service.discovery_status();
        let available = account == AccountStatus::Authorized
            && matches!(
                self.rust().service.private_status(),
                PrivateStatus::Ready(_)
            );
        let devices = self
            .rust()
            .service
            .own_device_names()
            .into_iter()
            .map(QString::from)
            .collect::<QStringList>();
        let saved = self.rust().service.saved_login_status().text();
        let remembered = self.rust().service.has_saved_login();
        let transfer_status = self.rust().service.transfer_text().clone();
        let transfer_busy = self.rust().service.transfer_busy();
        let selected_send_name = self.rust().service.selected_send_name();
        let receive_directory = self.rust().service.receive_directory();
        let receive_status = self.rust().service.receive_text().clone();
        let receiving_active = self.rust().service.receiving_active();
        let background_receiving = self.rust().service.background_receiving();
        let confirm_each_file = self.rust().service.confirm_each_file();
        let offer = self.rust().service.incoming_offer();
        let receive_pending = offer.is_some();
        let actionable_notification = self.rust().notifications.suppress_fallback();
        let continuous_discovery = self.rust().service.continuous_discovery();
        let received_count = self.rust().service.received_count();
        let incoming_offer = offer
            .map(|o| {
                text(&Text::with_args(
                    "%1\n%2\n%3 bytes",
                    [
                        Text::literal(o.sender),
                        Text::literal(o.name),
                        Text::literal(o.bytes),
                    ],
                ))
            })
            .unwrap_or_default();
        let status = self.rust().service.status_text();
        let zone = QTimeZone::system_time_zone();
        let rows = self
            .rust()
            .service
            .certificate_rows()
            .into_iter()
            .map(|row| {
                let date = |secs| {
                    zone.as_ref()
                        .map(|z| {
                            QDateTime::from_secs_since_epoch(secs, z)
                                .format_enum(DateFormat::ISODate)
                                .to_string()
                        })
                        .unwrap_or_else(|| tr("Time unavailable").to_string())
                };
                let name = if row.local {
                    Text::new("This computer (Nearlane)")
                } else {
                    row.device
                        .as_deref()
                        .map(Text::literal)
                        .unwrap_or_else(|| Text::new("Device name encrypted / not discovered"))
                };
                text(&Text::with_args(
                    "Certificate %1 - %2\n%3\n%4 → %5",
                    [
                        Text::literal(row.number),
                        Text::new(if row.active {
                            "active"
                        } else {
                            "outside current validity"
                        }),
                        name,
                        Text::literal(date(row.starts_at)),
                        Text::literal(date(row.expires_at)),
                    ],
                ))
            })
            .collect::<QStringList>();
        self.as_mut().set_account_status(text(&account.text()));
        self.as_mut().set_account_configured(configured);
        self.as_mut().set_account_busy(busy);
        self.as_mut()
            .set_account_authorized(account == AccountStatus::Authorized);
        self.as_mut().set_private_status(text(&private_status));
        self.as_mut().set_private_busy(private_busy);
        self.as_mut().set_discovery_status(text(&discovery.text()));
        self.as_mut()
            .set_discovery_busy(discovery == DiscoveryStatus::Searching);
        self.as_mut().set_discovery_available(available);
        self.as_mut().set_own_devices(devices);
        self.as_mut().set_saved_login_status(text(&saved));
        self.as_mut().set_login_remembered(remembered);
        self.as_mut().set_login_cancellable(matches!(
            account,
            AccountStatus::WaitingForBrowser | AccountStatus::Exchanging | AccountStatus::Restoring
        ));
        self.as_mut().set_certificate_rows(rows);
        self.as_mut()
            .set_selected_send_name(QString::from(selected_send_name.as_str()));
        self.as_mut().set_transfer_status(text(&transfer_status));
        self.as_mut().set_transfer_busy(transfer_busy);
        self.as_mut()
            .set_receive_directory(QString::from(receive_directory.as_str()));
        self.as_mut().set_receive_status(text(&receive_status));
        self.as_mut().set_incoming_offer(incoming_offer);
        self.as_mut().set_receiving_active(receiving_active);
        self.as_mut().set_background_receiving(background_receiving);
        self.as_mut().set_confirm_each_file(confirm_each_file);
        if !receive_pending {
            self.as_mut().set_receive_pending(false);
        }
        self.as_mut()
            .set_actionable_notification(actionable_notification);
        self.as_mut().set_receive_pending(receive_pending);
        self.as_mut().set_continuous_discovery(continuous_discovery);
        self.as_mut().set_received_count(received_count);
        self.set_status(text(&status));
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

    /// Explicitly search for certificate-matched local-network advertisements.
    pub fn find_own_devices(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        self.as_mut().rust_mut().service.find_own_devices();
        self.update_account();
    }

    /// Forget candidates and cancel the worker; no late result is applied.
    pub fn cancel_discovery(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        self.as_mut().rust_mut().service.cancel_discovery();
        self.update_account();
    }

    /// Retry the scoped system-vault flow without a browser or token exposure.
    pub fn restore_saved_sign_in(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        let _ = self.as_mut().rust_mut().service.restore_saved_sign_in();
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
    /// Native file chooser result; do not log the path or send it to a peer.
    pub fn select_send_file(mut self: Pin<&mut Self>, url: &QUrl) {
        use cxx_qt::CxxQtType as _;
        if let Some(path) = url.to_local_file() {
            self.as_mut()
                .rust_mut()
                .service
                .select_send_file(std::path::PathBuf::from(path.to_string()));
        }
        self.update_account();
    }
    /// Explicit recipient action from the current plain-text candidate list.
    pub fn send_selected_file(mut self: Pin<&mut Self>, index: i32) {
        use cxx_qt::CxxQtType as _;
        if let Ok(index) = usize::try_from(index) {
            self.as_mut().rust_mut().service.send_selected_file(index);
        }
        self.update_account();
    }
    /// Interrupt the socket and drop any late completion result.
    pub fn cancel_transfer(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        self.as_mut().rust_mut().service.cancel_transfer();
        self.update_account();
    }

    /// Explicit native destination-folder selection.
    pub fn select_receive_directory(mut self: Pin<&mut Self>, url: &QUrl) {
        use cxx_qt::CxxQtType as _;
        if let Some(path) = url.to_local_file() {
            self.as_mut()
                .rust_mut()
                .service
                .select_receive_directory(std::path::PathBuf::from(path.to_string()));
            self.as_mut().save_preferences();
        }
        self.update_account();
    }
    /// Enable only the certificate-backed private LAN receiver.
    pub fn start_receiving(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        self.as_mut().rust_mut().service.start_receiving();
        self.as_mut().save_preferences();
        self.update_account();
    }
    /// Withdraw receiver visibility and cancel established incoming IO.
    pub fn stop_receiving(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        self.as_mut().rust_mut().service.stop_receiving();
        self.as_mut().save_preferences();
        self.update_account();
    }
    /// Explicit local consent, after live private device verification.
    pub fn decide_incoming(mut self: Pin<&mut Self>, accept: bool) {
        use cxx_qt::CxxQtType as _;
        self.as_mut().rust_mut().service.decide_incoming(accept);
        self.update_account();
    }
    /// Explicitly controlled continuous private discovery, including while hidden.
    pub fn set_background_discovery(mut self: Pin<&mut Self>, enabled: bool) {
        use cxx_qt::CxxQtType as _;
        self.as_mut()
            .rust_mut()
            .service
            .set_continuous_discovery(enabled);
        self.update_account();
    }
    /// Persist only local UI preferences, never credentials or identity state.
    pub fn configure_receiving(mut self: Pin<&mut Self>, enabled: bool, confirm: bool) {
        use cxx_qt::CxxQtType as _;
        self.as_mut()
            .rust_mut()
            .service
            .set_receiving_preferences(enabled, confirm);
        self.as_mut().save_preferences();
        self.update_account();
    }
    /// Only a visible foreground Send screen may request the silent BLE hint.
    pub fn set_send_surface(mut self: Pin<&mut Self>, visible: bool) {
        use cxx_qt::CxxQtType as _;
        self.as_mut().rust_mut().send_surface = visible;
        self.update_account();
    }
    fn save_preferences(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        if self.rust().smoke_test {
            return;
        }
        let service = &self.rust().service;
        let saved = ffi::save_receive_preferences(
            service.background_receiving(),
            service.confirm_each_file(),
            &QString::from(service.receive_directory().as_str()),
        );
        self.as_mut().rust_mut().preferences_failed = !saved;
        self.update_preferences_status();
    }
    fn update_preferences_status(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        let status = if self.rust().preferences_failed {
            tr("Could not save settings. These changes apply only until the app closes.")
        } else {
            QString::default()
        };
        self.as_mut().set_preferences_status(status);
    }
    /// Persist a validated UI language and retranslate without restarting IO.
    pub fn select_language(mut self: Pin<&mut Self>, language: &QString) {
        use cxx_qt::CxxQtType as _;
        let value = language.to_string();
        if !matches!(value.as_str(), "system" | "ru" | "en") {
            return;
        }
        let saved = ffi::change_language(language);
        self.as_mut().rust_mut().preferences_failed = !saved;
        self.as_mut().set_language(language.clone());
        self.as_mut()
            .set_effective_language(ffi::effective_language());
        // Existing offers keep their identity/consent; only their presentation changes.
        self.as_mut().rust_mut().notifications.clear();
        self.as_mut().update_preferences_status();
        self.update_account();
    }

    /// Stop sharing workers on Quit without deleting the saved account grant.
    pub fn shutdown(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        self.as_mut().rust_mut().bluetooth.stop();
        self.as_mut().rust_mut().notifications.clear();
        self.as_mut().rust_mut().service.cancel_discovery();
        self.as_mut().rust_mut().service.stop_receiving();
        self.as_mut().rust_mut().service.cancel_transfer();
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
            Ok(()) => tr("Contacts are available."),
            Err(_) => tr("Contacts are not available yet."),
        };
        let smoke_test = self.rust().smoke_test;
        self.as_mut().set_status(status);
        if smoke_test {
            eprintln!("smoke: policy checked; device remains hidden");
            self.smoke_account_check();
        }
    }

    fn smoke_account_check(mut self: Pin<&mut Self>) {
        use cxx_qt::CxxQtType as _;
        if let Ok(phase) = std::env::var("NEARLANE_PREFERENCES_SMOKE_TEST") {
            if phase == "write" {
                assert!(ffi::change_language(&QString::from("ru")));
                assert!(ffi::receive_by_default());
                assert!(!ffi::confirm_received_files());
                assert!(ffi::save_receive_preferences(
                    false,
                    true,
                    &QString::from("synthetic-preference-folder")
                ));
            }
            assert_eq!(ffi::language_preference().to_string(), "ru");
            assert_eq!(ffi::effective_language().to_string(), "ru");
            self.as_mut().set_language(ffi::language_preference());
            self.as_mut()
                .set_effective_language(ffi::effective_language());
            eprintln!("smoke: language preference persisted; synthetic settings only");
            assert!(!ffi::receive_by_default());
            assert!(ffi::confirm_received_files());
            assert_eq!(
                ffi::saved_receive_directory().to_string(),
                "synthetic-preference-folder"
            );
            eprintln!("smoke: receiving preferences persisted; synthetic settings only");
        }
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
