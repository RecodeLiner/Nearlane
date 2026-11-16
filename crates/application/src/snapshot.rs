//! Presentation data only. Tokens, authorization URLs, keys and peer addresses
//! never cross the application/frontend boundary.
use crate::{i18n, runtime::Runtime};
use chrono::{DateTime, Local};
use nearlane_core::Text;
use nearlane_service::{AccountStatus, DiscoveryStatus, PrivateStatus};

/// Immutable, localized state shared by desktop frontends.
#[derive(Clone, Default, PartialEq)]
pub struct Snapshot {
    /// Monotonic presentation revision.
    pub revision: u64,
    /// Saved language choice.
    pub language: String,
    /// Resolved catalog identifier.
    pub effective_language: String,
    /// Overall private-mode status.
    pub status: String,
    /// Account progress without credentials.
    pub account_status: String,
    /// Local application parameters are configured.
    pub account_configured: bool,
    /// Runtime JSON path, never its contents.
    pub config_path: String,
    /// Last explicit configuration import outcome.
    pub config_status: String,
    /// Import failed.
    pub config_error: bool,
    /// Account operation is running.
    pub account_busy: bool,
    /// Google granted the account permission.
    pub account_authorized: bool,
    /// Certificate preparation progress.
    pub private_status: String,
    /// Certificate operation is running.
    pub private_busy: bool,
    /// Wi-Fi discovery progress.
    pub discovery_status: String,
    /// Explicit discovery is running.
    pub discovery_busy: bool,
    /// Authenticated account and own-device certificates are ready.
    pub discovery_available: bool,
    /// Authenticated routable candidate names.
    pub own_devices: Vec<String>,
    /// Selection generation for rejecting stale send actions.
    pub devices_revision: u64,
    /// Authenticated BLE hints awaiting a Wi-Fi endpoint.
    pub bluetooth_devices: Vec<String>,
    /// Sanitized radio status.
    pub bluetooth_status: String,
    /// OS credential store progress.
    pub saved_login_status: String,
    /// An offline grant has been remembered.
    pub login_remembered: bool,
    /// Login or restore can be cancelled.
    pub login_cancellable: bool,
    /// Safe certificate inventory, with no keys or raw IDs.
    pub certificate_rows: Vec<String>,
    /// Local selected filename.
    pub selected_send_name: String,
    /// Sender progress.
    pub transfer_status: String,
    /// An outgoing transfer is running.
    pub transfer_busy: bool,
    /// Local destination folder.
    pub receive_directory: String,
    /// Receiver progress.
    pub receive_status: String,
    /// Current verified offer, formatted as plain text.
    pub incoming_offer: String,
    /// Request identity; decisions are checked again in the service.
    pub incoming_request_id: Option<u64>,
    /// A receiver worker is owned by this application.
    pub receiving_active: bool,
    /// Saved automatic private reception intent.
    pub background_receiving: bool,
    /// Saved approval intent.
    pub confirm_each_file: bool,
    /// Non-secret persistence error.
    pub preferences_status: String,
    /// An incoming offer awaits consent.
    pub receive_pending: bool,
    /// A native actionable notification handles the offer.
    pub actionable_notification: bool,
    /// Background discovery is requested.
    pub continuous_discovery: bool,
    /// Successfully received files in this process.
    pub received_count: u32,
    /// Actual current policy remains hidden until private reception is verified.
    pub hidden: bool,
}
pub(crate) fn snapshot(runtime: &Runtime) -> Snapshot {
    let service = &runtime.service;
    let language = runtime.preferences.language;
    let text = |t: &Text| i18n::render(language, t);
    let tr = |s| text(&Text::new(s));
    let account = service.account_status();
    let discovery = service.discovery_status();
    let rows = service
        .certificate_rows()
        .into_iter()
        .map(|row| {
            let date = |seconds| {
                DateTime::from_timestamp(seconds, 0)
                    .map(|t| t.with_timezone(&Local).to_rfc3339())
                    .unwrap_or_else(|| tr("Time unavailable"))
            };
            let name = if row.local {
                Text::new("This computer (Nearlane)")
            } else {
                row.device
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
        .collect();
    let offer = service.incoming_offer();
    Snapshot {
        revision: 0,
        language: language.as_str().into(),
        effective_language: i18n::effective_language(language).as_str().into(),
        status: text(
            &runtime
                .status_override
                .clone()
                .unwrap_or_else(|| service.status_text()),
        ),
        account_status: text(&account.text()),
        account_configured: service.account_configured(),
        config_path: runtime
            .config_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
        config_status: match runtime.config_result {
            None => String::new(),
            Some(Ok(())) => tr(
                "Sign-in configuration saved. It will be loaded automatically on the next start.",
            ),
            Some(Err(e)) => text(&e.text()),
        },
        config_error: runtime.config_result.is_some_and(|r| r.is_err()),
        account_busy: service.account_busy(),
        account_authorized: account == AccountStatus::Authorized,
        private_status: text(&service.private_status().text()),
        private_busy: service.private_busy(),
        discovery_status: text(&discovery.text()),
        discovery_busy: discovery == DiscoveryStatus::Searching,
        discovery_available: account == AccountStatus::Authorized
            && matches!(service.private_status(), PrivateStatus::Ready(_)),
        own_devices: service
            .own_device_names()
            .into_iter()
            .map(str::to_owned)
            .collect(),
        devices_revision: runtime.devices_revision,
        bluetooth_devices: service
            .bluetooth_device_names()
            .into_iter()
            .map(str::to_owned)
            .collect(),
        bluetooth_status: text(&runtime.bluetooth_status),
        saved_login_status: text(&service.saved_login_status().text()),
        login_remembered: service.has_saved_login(),
        login_cancellable: matches!(
            account,
            AccountStatus::WaitingForBrowser | AccountStatus::Exchanging | AccountStatus::Restoring
        ),
        certificate_rows: rows,
        selected_send_name: service.selected_send_name(),
        transfer_status: text(service.transfer_text()),
        transfer_busy: service.transfer_busy(),
        receive_directory: service.receive_directory(),
        receive_status: text(service.receive_text()),
        incoming_offer: offer
            .as_ref()
            .map(|o| {
                text(&Text::with_args(
                    "%1\n%2\n%3 bytes",
                    [
                        Text::literal(&o.sender),
                        Text::literal(&o.name),
                        Text::literal(o.bytes),
                    ],
                ))
            })
            .unwrap_or_default(),
        incoming_request_id: service.incoming_request_id(),
        receiving_active: service.receiving_active(),
        background_receiving: service.background_receiving(),
        confirm_each_file: service.confirm_each_file(),
        preferences_status: if runtime.preferences_failed {
            tr("Could not save settings. These changes apply only until the app closes.")
        } else {
            String::new()
        },
        receive_pending: offer.is_some(),
        actionable_notification: runtime.notifications.suppress_fallback(),
        continuous_discovery: service.continuous_discovery(),
        received_count: service.received_count(),
        hidden: service.visibility() == nearlane_core::Visibility::Hidden,
    }
}
