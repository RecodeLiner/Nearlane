use crate::{Command, Options, notifications::OfferNotifications, radio::Radio};
use nearlane_core::{Text, Visibility};
use nearlane_platform::preferences::{PreferenceStore, Preferences};
use nearlane_service::{LoginError, Service};
use std::path::PathBuf;

pub(crate) struct Runtime {
    pub(crate) service: Service,
    pub(crate) preferences: Preferences,
    pub(crate) preferences_failed: bool,
    pub(crate) config_result: Option<Result<(), LoginError>>,
    pub(crate) config_path: Option<PathBuf>,
    pub(crate) status_override: Option<Text>,
    pub(crate) bluetooth_status: Text,
    pub(crate) notifications: OfferNotifications,
    pub(crate) devices_revision: u64,
    device_stamp: u64,
    store: Option<PreferenceStore>,
    live: bool,
    foreground: bool,
    radio: Radio,
}
impl Runtime {
    pub(crate) fn new(options: Options) -> Self {
        let loaded = options
            .preferences
            .as_ref()
            .map(PreferenceStore::load)
            .transpose();
        let preferences_failed = loaded.is_err();
        let preferences = loaded.ok().flatten().unwrap_or_default();
        let mut service = Service::from_config(options.config.as_deref());
        if options.live && service.account_configured() {
            let _ = service.restore_saved_sign_in();
        }
        if let Some(path) = preferences.destination() {
            service.select_receive_directory(path);
        }
        service.set_continuous_discovery(options.live && preferences.continuous_discovery);
        service.set_receiving_preferences(
            options.live && preferences.background_receiving,
            !options.live || preferences.confirm_each_file,
        );
        let device_stamp = service.device_list_stamp();
        Self {
            service,
            preferences,
            preferences_failed,
            config_result: None,
            config_path: options.config,
            status_override: None,
            bluetooth_status: Text::new("Bluetooth discovery is off."),
            notifications: OfferNotifications::default(),
            devices_revision: 0,
            device_stamp,
            store: options.preferences,
            live: options.live,
            foreground: false,
            radio: Radio::default(),
        }
    }
    pub(crate) fn tick(&mut self) {
        self.service.poll_account();
        self.notifications
            .update(&mut self.service, self.live, self.preferences.language);
        self.bluetooth_status = self
            .radio
            .update(&mut self.service, self.live, self.foreground);
        let stamp = self.service.device_list_stamp();
        if self.device_stamp != stamp {
            self.device_stamp = stamp;
            self.devices_revision = self.devices_revision.wrapping_add(1);
        }
    }
    pub(crate) fn command(&mut self, command: Command) {
        match command {
            Command::SignIn => match self.service.begin_sign_in() {
                Ok(url) if self.live => {
                    if nearlane_platform::open_browser(url.expose_url()).is_err() {
                        self.service.cancel_sign_in(LoginError::Browser);
                    }
                }
                Ok(_) => {}
                Err(e) if e != LoginError::Busy => self.service.cancel_sign_in(e),
                Err(_) => {}
            },
            Command::CancelSignIn => self.service.cancel_sign_in(LoginError::Cancelled),
            Command::SignOut => {
                self.radio.stop();
                self.notifications.clear();
                self.service.sign_out();
            }
            Command::RestoreSignIn if self.live => {
                let _ = self.service.restore_saved_sign_in();
            }
            Command::RestoreSignIn => {}
            Command::PrepareOwnDevices => {
                let _ = self.service.prepare_own_devices();
            }
            Command::FindOwnDevices => self.service.find_own_devices(),
            Command::CancelDiscovery => self.service.cancel_discovery(),
            Command::ImportConfiguration(path) => {
                let result = self
                    .config_path
                    .as_ref()
                    .ok_or(LoginError::ConfigurationStorage)
                    .and_then(|dest| self.service.import_config_to(&path, dest));
                self.config_result = Some(result);
                if result.is_ok() && self.live {
                    let _ = self.service.restore_saved_sign_in();
                }
            }
            Command::SelectFile(path) => self.service.select_send_file(path),
            Command::SendFile {
                index,
                devices_revision,
            } => {
                if devices_revision == self.devices_revision {
                    self.service.send_selected_file(index);
                }
            }
            Command::CancelTransfer => self.service.cancel_transfer(),
            Command::SelectDestination(path) => {
                self.service.select_receive_directory(path);
                self.preferences.receive_directory = Some(self.service.receive_directory().into());
                self.save();
            }
            Command::StartReceiving => {
                self.service.start_receiving();
                self.preferences.background_receiving = true;
                self.save();
            }
            Command::StopReceiving => {
                self.service.stop_receiving();
                self.preferences.background_receiving = false;
                self.save();
            }
            Command::DecideIncoming { request_id, accept } => {
                let _ = self.service.decide_incoming_for(request_id, accept);
            }
            Command::SetDiscovery(enabled) => {
                self.service.set_continuous_discovery(enabled);
                self.preferences.continuous_discovery = enabled;
                self.save();
            }
            Command::ConfigureReceiving { enabled, confirm } => {
                self.service.set_receiving_preferences(enabled, confirm);
                self.preferences.background_receiving = enabled;
                self.preferences.confirm_each_file = confirm;
                self.save();
            }
            Command::SetSendSurface(visible) => self.foreground = visible,
            Command::SetLanguage(language) => {
                self.preferences.language = language;
                self.notifications.clear();
                self.save();
            }
            Command::CheckContacts => {
                let available = self.service.set_visibility(Visibility::Contacts).is_ok();
                self.status_override = Some(Text::new(if available {
                    "Contacts are available."
                } else {
                    "Contacts are not available yet."
                }));
            }
        }
    }
    fn save(&mut self) {
        self.preferences_failed = self
            .store
            .as_ref()
            .is_none_or(|store| store.save(&self.preferences).is_err());
    }
    pub(crate) fn shutdown(&mut self) {
        self.radio.stop();
        self.notifications.clear();
        self.service.cancel_discovery();
        self.service.stop_receiving();
        self.service.cancel_transfer();
    }
}
