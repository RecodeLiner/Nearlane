use crate::i18n;
use nearlane_core::Text;
use nearlane_platform::{
    notifications::{ActionNotification, FileAction, FileNotification, NotificationError},
    preferences::Language,
};
use nearlane_service::Service;

#[derive(Default)]
pub(crate) struct OfferNotifications {
    current: Option<u64>,
    job: Option<ActionNotification>,
    suppress_fallback: bool,
}
impl OfferNotifications {
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
    pub(crate) fn suppress_fallback(&self) -> bool {
        self.suppress_fallback
    }
    pub(crate) fn update(&mut self, service: &mut Service, enabled: bool, language: Language) {
        let id = service.incoming_request_id();
        if self.current != id {
            self.clear();
            self.current = id;
            if enabled && let Some(offer) = service.incoming_offer() {
                let tr = |s| i18n::translate(language, "Backend", s);
                self.job = FileNotification::new(&offer.sender, &offer.name, offer.bytes)
                    .and_then(|r| {
                        r.localized(
                            &tr("Incoming file"),
                            &tr("Accept"),
                            &tr("Decline"),
                            &i18n::render(
                                language,
                                &Text::with_args("%1 B", [Text::literal(offer.bytes)]),
                            ),
                        )
                    })
                    .and_then(ActionNotification::show)
                    .ok();
                self.suppress_fallback = self.job.is_some();
            }
        }
        if let Some(result) = self.job.as_ref().and_then(ActionNotification::poll) {
            match result {
                Ok(action) => {
                    if let Some(id) = self.current {
                        let _ = service.decide_incoming_for(id, action == FileAction::Accept);
                    }
                }
                Err(NotificationError::Unavailable | NotificationError::Invalid) => {
                    self.suppress_fallback = false
                }
                Err(NotificationError::Expired | NotificationError::Cancelled) => {}
            }
            self.job = None;
        }
    }
}
