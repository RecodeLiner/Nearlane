//! Typed callbacks and value conversion only. No authentication, device policy,
//! settings persistence, notification decisions or radio lifetime lives here.
use crate::{Action, Actions, I18n, MainWindow, UiState};
use nearlane_application::{Command, Language, Sender, Snapshot, i18n};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::{cell::RefCell, rc::Rc};

pub(crate) type Displayed = Rc<RefCell<Snapshot>>;

pub(crate) fn translations(global: I18n<'_>) {
    global.on_lookup(|source, language| {
        i18n::translate(
            Language::parse(&language).unwrap_or(Language::En),
            "Ui",
            &source,
        )
        .into()
    });
}
pub(crate) fn actions(
    global: Actions<'_>,
    sender: Sender,
    displayed: Displayed,
    window: slint::Weak<MainWindow>,
) {
    let tx = sender.clone();
    global.on_action(move |action| {
        let command = match action {
            Action::SignIn => Command::SignIn,
            Action::CancelSignIn => Command::CancelSignIn,
            Action::SignOut => Command::SignOut,
            Action::RestoreSignIn => Command::RestoreSignIn,
            Action::PrepareDevices => Command::PrepareOwnDevices,
            Action::FindDevices => Command::FindOwnDevices,
            Action::CancelDiscovery => Command::CancelDiscovery,
            Action::CancelTransfer => Command::CancelTransfer,
            Action::StartReceiving => Command::StartReceiving,
            Action::StopReceiving => Command::StopReceiving,
            Action::CheckContacts => Command::CheckContacts,
        };
        let _ = tx.send(command);
    });
    let tx = sender.clone();
    global.on_choose_file(move || {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            let _ = tx.send(Command::SelectFile(path));
        }
    });
    let tx = sender.clone();
    global.on_choose_configuration(move || {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .pick_file()
        {
            let _ = tx.send(Command::ImportConfiguration(path));
        }
    });
    let tx = sender.clone();
    global.on_choose_destination(move || {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            let _ = tx.send(Command::SelectDestination(path));
        }
    });
    let tx = sender.clone();
    let current = Rc::clone(&displayed);
    global.on_send_file(move |index| {
        if let Ok(index) = usize::try_from(index) {
            let _ = tx.send(Command::SendFile {
                index,
                devices_revision: current.borrow().devices_revision,
            });
        }
    });
    let tx = sender.clone();
    let current = Rc::clone(&displayed);
    global.on_decide_incoming(move |accept| {
        if let Some(request_id) = current.borrow().incoming_request_id {
            let _ = tx.send(Command::DecideIncoming { request_id, accept });
        }
    });
    let tx = sender.clone();
    global.on_receiving(move |enabled, confirm| {
        let _ = tx.send(Command::ConfigureReceiving { enabled, confirm });
    });
    let tx = sender.clone();
    global.on_discovery(move |enabled| {
        let _ = tx.send(Command::SetDiscovery(enabled));
    });
    let tx = sender.clone();
    global.on_language(move |value| {
        if let Some(language) = Language::parse(&value) {
            let _ = tx.send(Command::SetLanguage(language));
        }
    });
    let tx = sender.clone();
    let weak = window.clone();
    global.on_foreground(move |enabled| {
        let visible = weak.upgrade().is_some_and(|w| w.window().is_visible());
        let _ = tx.send(Command::SetSendSurface(enabled && visible));
    });
    let tx = sender.clone();
    let weak = window.clone();
    global.on_about(move || {
        if let Some(window) = weak.upgrade()
            && window.show().is_ok()
        {
            window.invoke_show_about();
        }
    });
    global.on_open_window(move || {
        if let Some(window) = window.upgrade()
            && window.show().is_ok()
        {
            let _ = tx.send(Command::SetSendSurface(window.get_page() == 0));
        }
    });
    global.on_quit(move || {
        let _ = sender.send(Command::SetSendSurface(false));
        let _ = slint::quit_event_loop();
    });
}
fn model(values: &[String]) -> ModelRc<SharedString> {
    Rc::new(VecModel::from(
        values.iter().map(|s| s.as_str().into()).collect::<Vec<_>>(),
    ))
    .into()
}

pub(crate) fn state(snapshot: &Snapshot) -> UiState {
    UiState {
        revision: snapshot.revision as i32,
        language: snapshot.language.as_str().into(),
        effective_language: snapshot.effective_language.as_str().into(),
        status: snapshot.status.as_str().into(),
        account_status: snapshot.account_status.as_str().into(),
        account_configured: snapshot.account_configured,
        config_path: snapshot.config_path.as_str().into(),
        config_status: snapshot.config_status.as_str().into(),
        config_error: snapshot.config_error,
        account_busy: snapshot.account_busy,
        account_authorized: snapshot.account_authorized,
        private_status: snapshot.private_status.as_str().into(),
        private_busy: snapshot.private_busy,
        discovery_status: snapshot.discovery_status.as_str().into(),
        discovery_busy: snapshot.discovery_busy,
        discovery_available: snapshot.discovery_available,
        own_devices: model(&snapshot.own_devices),
        devices_revision: snapshot.devices_revision as i32,
        bluetooth_devices: model(&snapshot.bluetooth_devices),
        bluetooth_status: snapshot.bluetooth_status.as_str().into(),
        saved_login_status: snapshot.saved_login_status.as_str().into(),
        login_remembered: snapshot.login_remembered,
        login_cancellable: snapshot.login_cancellable,
        certificate_rows: model(&snapshot.certificate_rows),
        selected_send_name: snapshot.selected_send_name.as_str().into(),
        transfer_status: snapshot.transfer_status.as_str().into(),
        transfer_busy: snapshot.transfer_busy,
        receive_directory: snapshot.receive_directory.as_str().into(),
        receive_status: snapshot.receive_status.as_str().into(),
        incoming_offer: snapshot.incoming_offer.as_str().into(),
        receiving_active: snapshot.receiving_active,
        background_receiving: snapshot.background_receiving,
        confirm_each_file: snapshot.confirm_each_file,
        preferences_status: snapshot.preferences_status.as_str().into(),
        receive_pending: snapshot.receive_pending,
        actionable_notification: snapshot.actionable_notification,
        continuous_discovery: snapshot.continuous_discovery,
        received_count: snapshot.received_count as i32,
        hidden: snapshot.hidden,
    }
}
